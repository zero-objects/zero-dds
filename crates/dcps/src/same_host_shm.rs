// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Wave 4b.3 (Spec `docs/specs/zerodds-zero-copy-1.0.md` §6 Wave 3):
//! feature-gated bridge between [`crate::same_host`] (no_std tracker)
//! and `zerodds_transport_shm::PosixShmTransport`.
//!
//! Only compiled with the `same-host-shm` feature enabled. Without the
//! feature the tracker stays a pure pending state machine and the hot
//! path falls back to UDP.
//!
//! # Path resolution
//!
//! `flink_dir = ${TMPDIR}/zerodds-shm/${host_id_hex}/`
//!
//! `host_id_hex` is the hex representation of the first 4 bytes of the
//! local GuidPrefix (Wave 4a host-id). This prevents same-host segments
//! from colliding across different hosts on NFS mounts.
//!
//! The segment file names are derived via
//! [`crate::same_host::shm_segment_filename`] from the 128-bit hash of
//! the `(writer_guid, reader_guid)` pair. Both sides arrive at the same
//! path without an extra discovery round-trip.

#![cfg(feature = "same-host-shm")]

use alloc::sync::Arc;
use core::any::Any;
use std::time::Duration;

use zerodds_rtps::wire_types::{Guid, GuidPrefix};
use zerodds_transport_shm::{PosixShmTransport, ShmConfig};

use crate::same_host::Role;

/// Default datagram limit for the same-host SHM path. More liberal than
/// the DDSI-RTPS MTU (1472 bytes) so that large fragments don't need to
/// be split up unnecessarily locally.
pub const DEFAULT_MAX_DATAGRAM: usize = 64 * 1024;

/// Default ring-buffer capacity (bytes). Must be `>= 2 * max_datagram`
/// (PosixShmTransport constraint).
pub const DEFAULT_CAPACITY: usize = 2 * 1024 * 1024;

/// Computes the `ShmConfig` for a same-host pair.
///
/// The resulting `flink_dir` is absolute and contains the `host_id_hex`
/// sub-folder. `${TMPDIR}` falls back to `/tmp` if the env var is not
/// set.
#[must_use]
pub fn shm_config_for_pair(local_prefix: GuidPrefix) -> ShmConfig {
    let host_id = local_prefix.host_id();
    let host_hex = bytes_to_hex(&host_id);
    let base = std::env::temp_dir();
    let flink_dir = base.join("zerodds-shm").join(host_hex);
    // C3 variable zero-copy: the SHM ring buffer is length-prefixed, so
    // already **variable-size** (no Iceoryx fixed pool). The only cap is
    // `max_datagram` — but the 64 KiB default is too small for ROS
    // PointCloud2/Image (several MB) (the consumer drops
    // `len > max_datagram`). `ZERODDS_SHM_MAX_DATAGRAM` (bytes) raises it;
    // the ring capacity is scaled along accordingly (`>= 2*max + 16`).
    let max_datagram = std::env::var("ZERODDS_SHM_MAX_DATAGRAM")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_DATAGRAM);
    let capacity = DEFAULT_CAPACITY.max(max_datagram * 2 + 4096);
    ShmConfig {
        capacity,
        flink_dir,
        max_datagram,
        // Short poll timeout: with one sample per recv() the round-trip
        // latency is at most `recv_timeout`. 1ms is a compromise between
        // CPU load (busy-spin) and latency (waiting too long).
        recv_timeout: Some(Duration::from_millis(1)),
    }
}

/// Attempts to set up a `PosixShmTransport` segment as `Owner`.
///
/// The owner side is the **writer** in the DCPS model — it creates the
/// segment and writes sample datagrams into it. The reader attaches
/// later as a consumer.
///
/// On success returns an opaque `Arc<dyn Any + Send + Sync>` that is
/// stored by the [`crate::same_host::SameHostTracker`] as `Bound {
/// transport, role: Owner }`. The hot-path sender (Wave 4b.4) downcasts
/// back to `PosixShmTransport`.
///
/// On failure the function returns a static diagnostic string; the
/// caller marks the tracker entry as `Failed` and falls back to UDP.
pub fn open_owner_segment(
    local_prefix: GuidPrefix,
    writer_guid: Guid,
    reader_guid: Guid,
) -> Result<Arc<dyn Any + Send + Sync>, &'static str> {
    let cfg = shm_config_for_pair(local_prefix);
    // flink_dir must exist; PosixShmTransport::open_owner calls
    // ensure_base_dir internally, but we want a clear error path.
    if std::fs::create_dir_all(&cfg.flink_dir).is_err() {
        return Err("shm: flink_dir create failed");
    }
    // Owner = writer; peer = reader.
    let local_id = writer_guid.to_bytes();
    let peer_id = reader_guid.to_bytes();
    match PosixShmTransport::open_owner(local_id, peer_id, cfg) {
        Ok(t) => Ok(Arc::new(t) as Arc<dyn Any + Send + Sync>),
        Err(_) => Err("shm: open_owner failed"),
    }
}

/// Attempts to attach a `PosixShmTransport` segment as `Consumer`.
///
/// The owner (writer side) must have set up the segment beforehand. If
/// the owner is not yet present, `PosixShmTransport::open_consumer`
/// returns a `MapOpenFailed`, and the caller then marks `Failed`. A
/// later SEDP re-match may trigger another attempt (Wave 4b.4 follow-up).
pub fn open_consumer_segment(
    local_prefix: GuidPrefix,
    writer_guid: Guid,
    reader_guid: Guid,
) -> Result<Arc<dyn Any + Send + Sync>, &'static str> {
    let cfg = shm_config_for_pair(local_prefix);
    // Consumer = reader; peer = writer (Owner).
    let local_id = reader_guid.to_bytes();
    let peer_id = writer_guid.to_bytes();
    match PosixShmTransport::open_consumer(local_id, peer_id, cfg) {
        Ok(t) => Ok(Arc::new(t) as Arc<dyn Any + Send + Sync>),
        Err(_) => Err("shm: open_consumer failed"),
    }
}

/// Helper: `Role` for a `(writer, reader)` pair in the sense that
/// `local_prefix` is one of the two sides.
///
/// Returns `Some(Role::Owner)` if `local_prefix == writer.prefix`
/// (writer side produces), `Some(Role::Consumer)` if
/// `local_prefix == reader.prefix` (reader side consumes), `None` for
/// both (self-match) or neither (error in the caller).
#[must_use]
pub fn local_role_for_pair(local_prefix: GuidPrefix, writer: Guid, reader: Guid) -> Option<Role> {
    let is_writer = local_prefix == writer.prefix;
    let is_reader = local_prefix == reader.prefix;
    match (is_writer, is_reader) {
        (true, false) => Some(Role::Owner),
        (false, true) => Some(Role::Consumer),
        _ => None,
    }
}

fn bytes_to_hex(bytes: &[u8]) -> alloc::string::String {
    let mut s = alloc::string::String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0F) as usize] as char);
    }
    s
}

// Sanity cross-check: `shm_segment_filename` produces deterministic hex
// strings; that is tested in the no_std module `same_host`. Here only the
// bytes-to-hex variant for host_id.

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use zerodds_rtps::wire_types::EntityId;

    #[test]
    fn shm_config_satisfies_capacity_constraint_and_default() {
        // B8 / C3 variable zero-copy: the length-prefixed ring is
        // variable-size; `capacity >= 2*max_datagram + 16` is the
        // PosixShmTransport invariant. Default path (no env).
        let cfg = shm_config_for_pair(GuidPrefix::from_bytes([7u8; 12]));
        assert_eq!(cfg.max_datagram, DEFAULT_MAX_DATAGRAM);
        assert!(
            cfg.capacity >= cfg.max_datagram * 2 + 16,
            "capacity {} < 2*max_datagram {} + 16",
            cfg.capacity,
            cfg.max_datagram
        );
        // The capacity formula scales correctly with a large max_datagram:
        // at 8 MiB max_datagram this yields >= 16 MiB capacity.
        let big = 8 * 1024 * 1024usize;
        assert!(DEFAULT_CAPACITY.max(big * 2 + 4096) >= big * 2 + 16);
    }

    fn writer_guid(seed: u8, host: [u8; 4]) -> Guid {
        let mut p = [0u8; 12];
        p[..4].copy_from_slice(&host);
        p[4..].copy_from_slice(&[seed; 8]);
        Guid::new(
            GuidPrefix::from_bytes(p),
            EntityId::user_writer_with_key([seed, seed, seed]),
        )
    }

    fn reader_guid(seed: u8, host: [u8; 4]) -> Guid {
        let mut p = [0u8; 12];
        p[..4].copy_from_slice(&host);
        p[4..].copy_from_slice(&[seed; 8]);
        Guid::new(
            GuidPrefix::from_bytes(p),
            EntityId::user_reader_with_key([seed, seed, seed]),
        )
    }

    #[test]
    fn config_flink_dir_contains_host_hex() {
        let prefix = GuidPrefix::from_bytes([0xAB, 0xCD, 0x12, 0x34, 0, 0, 0, 0, 0, 0, 0, 0]);
        let cfg = shm_config_for_pair(prefix);
        let s = cfg.flink_dir.to_string_lossy();
        assert!(s.contains("zerodds-shm"));
        assert!(
            s.contains("abcd1234"),
            "flink_dir should contain host_id_hex: {s}"
        );
    }

    #[test]
    fn local_role_writer_yields_owner() {
        let host = [1u8, 2, 3, 4];
        let mut other = host;
        other[0] = 9;
        let w_local = writer_guid(1, host);
        let r_remote = reader_guid(2, other);
        let role = local_role_for_pair(
            GuidPrefix::from_bytes(w_local.prefix.to_bytes()),
            w_local,
            r_remote,
        );
        assert_eq!(role, Some(Role::Owner));
    }

    #[test]
    fn local_role_reader_yields_consumer() {
        let host = [1u8, 2, 3, 4];
        let mut other = host;
        other[0] = 9;
        let w_remote = writer_guid(1, other);
        let r_local = reader_guid(2, host);
        let role = local_role_for_pair(
            GuidPrefix::from_bytes(r_local.prefix.to_bytes()),
            w_remote,
            r_local,
        );
        assert_eq!(role, Some(Role::Consumer));
    }

    #[test]
    fn local_role_neither_side_returns_none() {
        let elsewhere = [9u8, 9, 9, 9];
        let w = writer_guid(1, elsewhere);
        let r = reader_guid(2, elsewhere);
        let role = local_role_for_pair(GuidPrefix::from_bytes([0; 12]), w, r);
        assert!(role.is_none());
    }

    /// Wave 4c — E2E test: open_owner_segment + open_consumer_segment
    /// find each other via the global path convention
    /// (`shm_config_for_pair`, i.e. `${TMPDIR}/zerodds-shm/${host_hex}/...`)
    /// without explicit config sharing. Validates that both sides derive
    /// the SHM-segment path deterministically and that bytes round-trip.
    ///
    /// The GUIDs are deliberately chosen to be highly unique (test-unique
    /// 0xE2E2 prefix) so there is no conflict with tests running in
    /// parallel on the same system.
    #[test]
    fn welle_4c_pair_setup_via_path_convention_roundtrip() {
        use zerodds_transport::Transport;
        // Unique GuidPrefixes: same host_id (4b match detection), but
        // diverging PIDs/counters — analogous to the Wave 4a scheme in
        // participant::random_guid_prefix.
        let mut writer_prefix_bytes = [0u8; 12];
        writer_prefix_bytes[..4].copy_from_slice(&[0xE2, 0xE2, 0xC4, 0x01]);
        writer_prefix_bytes[4..8].copy_from_slice(&0x11111111u32.to_le_bytes());
        writer_prefix_bytes[8..].copy_from_slice(&[0x77, 0x77, 0x77, 0x77]);
        let mut reader_prefix_bytes = writer_prefix_bytes;
        reader_prefix_bytes[4..8].copy_from_slice(&0x22222222u32.to_le_bytes());
        reader_prefix_bytes[8..].copy_from_slice(&[0x88, 0x88, 0x88, 0x88]);

        let writer = Guid::new(
            GuidPrefix::from_bytes(writer_prefix_bytes),
            EntityId::user_writer_with_key([0xC4, 0x01, 0x01]),
        );
        let reader = Guid::new(
            GuidPrefix::from_bytes(reader_prefix_bytes),
            EntityId::user_reader_with_key([0xC7, 0x01, 0x01]),
        );

        // Setup like the SEDP hook: writer side opens Owner first, reader
        // side attaches as Consumer.
        let owner_any = open_owner_segment(writer.prefix, writer, reader)
            .expect("open_owner via path convention");
        let consumer_any = open_consumer_segment(reader.prefix, writer, reader)
            .expect("open_consumer via path convention");
        let owner = owner_any
            .downcast::<PosixShmTransport>()
            .expect("owner downcast");
        let consumer = consumer_any
            .downcast::<PosixShmTransport>()
            .expect("consumer downcast");

        let consumer_loc = consumer.local_locator();
        owner
            .send(&consumer_loc, b"welle-4c-e2e")
            .expect("owner.send");
        let got = consumer.recv().expect("consumer.recv");
        assert_eq!(&got.data[..], b"welle-4c-e2e");
    }

    /// Writer (Owner) → Reader (Consumer) round-trip via tempdir.
    /// Validates the DCPS role convention: the writer writes, the reader
    /// reads.
    #[test]
    fn writer_owner_to_reader_consumer_roundtrip() {
        use zerodds_transport::Transport;
        use zerodds_transport_shm::PosixShmTransport;
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: DEFAULT_CAPACITY,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: DEFAULT_MAX_DATAGRAM,
            recv_timeout: Some(Duration::from_millis(100)),
        };
        let host = [0xAA, 0xBB, 0xCC, 0xDD];
        let w = writer_guid(1, host);
        let r = reader_guid(2, host);
        // Owner = writer; Consumer = reader.
        let owner = PosixShmTransport::open_owner(w.to_bytes(), r.to_bytes(), cfg.clone())
            .expect("open_owner");
        let consumer = PosixShmTransport::open_consumer(r.to_bytes(), w.to_bytes(), cfg)
            .expect("open_consumer");
        // Writer (Owner) sends, reader (Consumer) receives.
        let consumer_loc = consumer.local_locator();
        owner.send(&consumer_loc, b"hello-same-host").expect("send");
        let got = consumer.recv().expect("recv");
        assert_eq!(&got.data[..], b"hello-same-host");
    }
}
