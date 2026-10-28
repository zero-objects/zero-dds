// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Segment configuration and orphan sweep for the same-host SHM channels
//! (Spec `docs/specs/zerodds-zero-copy-1.0.md` §6, see
//! [`crate::same_host_channel`]).
//!
//! Only compiled with the `same-host-shm` feature enabled.
//!
//! # Path resolution
//!
//! `flink_dir = ${TMPDIR}/zerodds-shm/${host_id_hex}/d${domain_id}/`
//!
//! `host_id_hex` is the hex representation of the host id of the local
//! GuidPrefix, octets 2..6 behind the vendor id (`GuidPrefix::host_id`). This prevents same-host segments
//! from colliding across different hosts on NFS mounts. `d${domain_id}`
//! keeps two domains apart.

#![cfg(feature = "same-host-shm")]

use std::time::Duration;

use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_transport_shm::ShmConfig;

/// Default datagram limit for the same-host SHM path. More liberal than
/// the DDSI-RTPS MTU (1472 bytes) so that large fragments don't need to
/// be split up unnecessarily locally.
pub const DEFAULT_MAX_DATAGRAM: usize = crate::runtime::DEFAULT_SHM_MAX_DATAGRAM;

/// Default ring-buffer capacity (bytes). Must be `>= 2 * max_datagram`
/// (PosixShmTransport constraint).
pub const DEFAULT_CAPACITY: usize = 2 * 1024 * 1024;

/// Computes the `ShmConfig` for a same-host pair.
///
/// The resulting `flink_dir` is absolute and contains the `host_id_hex`
/// sub-folder. `${TMPDIR}` falls back to `/tmp` if the env var is not
/// set.
#[must_use]
pub fn shm_config_for_pair(local_prefix: GuidPrefix, domain_id: u32) -> ShmConfig {
    pair_config(
        local_prefix,
        domain_id,
        DEFAULT_MAX_DATAGRAM,
        DEFAULT_CAPACITY,
    )
}

/// Build a pair's configuration from the explicitly selected runtime limit.
///
/// # Errors
/// A zero limit or a capacity that cannot be represented is refused.
pub fn shm_config_for_pair_with_limit(
    local_prefix: GuidPrefix,
    domain_id: u32,
    max_datagram: usize,
) -> Result<ShmConfig, &'static str> {
    let capacity = max_datagram
        .checked_mul(2)
        .and_then(|v| v.checked_add(4096))
        .filter(|_| max_datagram > 0)
        .ok_or("shm: invalid max_datagram")?;
    Ok(pair_config(
        local_prefix,
        domain_id,
        max_datagram,
        DEFAULT_CAPACITY.max(capacity),
    ))
}

fn pair_config(
    local_prefix: GuidPrefix,
    domain_id: u32,
    max_datagram: usize,
    capacity: usize,
) -> ShmConfig {
    let host_id = local_prefix.host_id();
    let host_hex = bytes_to_hex(&host_id);
    let base = std::env::temp_dir();
    let flink_dir = base
        .join("zerodds-shm")
        .join(host_hex)
        .join(alloc::format!("d{domain_id}"));
    ShmConfig {
        capacity,
        flink_dir,
        max_datagram,
        // Data wakes the receive thread at once (futex on Linux, the poll
        // backoff elsewhere), and a thread told to stop is woken by
        // `interrupt_recv`; the timeout is only the bound for a stop that
        // races the interrupt. It was 1 ms, which woke every receive
        // thread a thousand times a second with nothing to receive: 920
        // threads of a 21-process chain on one host cost 261 % of a core
        // (measured on Linux, 2026-10-02).
        recv_timeout: Some(Duration::from_millis(250)),
    }
}

/// A segment that changed more recently than this is left to a later
/// sweep: what it covers, the gap between a segment's creation and its
/// creator's lock, is microseconds.
#[cfg(unix)]
const ORPHAN_MIN_AGE: Duration = Duration::from_secs(60);

/// Unlinks the segments no process holds any more and files what it
/// removed: segments of processes that ended without dropping their
/// transport keep their memory until their name goes. On Linux once per
/// process over `/dev/shm`
/// (`zerodds_transport_shm::posix::sweep_orphaned_segments`); elsewhere
/// once per flink directory, which is per host and domain
/// (`sweep_orphaned_flinks`), since macOS lists no shared-memory objects.
/// Then, once per flink directory on every Unix, the `.lock` files and
/// stale flinks the pairs left (`sweep_stale_pair_files`).
#[cfg(unix)]
pub fn sweep_orphaned_segments_once(
    sink: &zerodds_foundation::observability::SharedSink,
    local_prefix: GuidPrefix,
    domain_id: u32,
) {
    use zerodds_foundation::observability::{Component, Event, Level};
    use zerodds_transport_shm::posix::SweepReport;
    let dir = shm_config_for_pair(local_prefix, domain_id).flink_dir;
    // Once per flink directory, which is per host and domain.
    static SWEPT: std::sync::Mutex<alloc::collections::BTreeSet<std::path::PathBuf>> =
        std::sync::Mutex::new(alloc::collections::BTreeSet::new());
    let first_in_dir = SWEPT
        .lock()
        .map(|mut s| s.insert(dir.clone()))
        .unwrap_or(false);
    #[cfg(target_os = "linux")]
    let segments: Option<std::io::Result<SweepReport>> = {
        // /dev/shm holds every domain's segments: once per process.
        static ONCE: std::sync::Once = std::sync::Once::new();
        let mut result = None;
        ONCE.call_once(|| {
            use zerodds_transport_shm::{bell, posix};
            // The rings and the doorbells: both stay in /dev/shm when a
            // process ends without dropping them.
            result = Some(
                posix::sweep_orphaned_segments(ORPHAN_MIN_AGE).and_then(|rings| {
                    bell::sweep_orphaned_doorbells(ORPHAN_MIN_AGE).map(|bells| rings.and(bells))
                }),
            );
        });
        result
    };
    #[cfg(not(target_os = "linux"))]
    let segments: Option<std::io::Result<SweepReport>> = first_in_dir
        .then(|| zerodds_transport_shm::posix::sweep_orphaned_flinks(&dir, ORPHAN_MIN_AGE));
    // After the segments: the `.lock` files (and on Linux the flinks) of
    // the pairs whose segments are gone.
    let pair_files = first_in_dir
        .then(|| zerodds_transport_shm::posix::sweep_stale_pair_files(&dir, ORPHAN_MIN_AGE));
    let swept = match (segments, pair_files) {
        (None, None) => None,
        (Some(r), None) | (None, Some(r)) => Some(r),
        (Some(a), Some(b)) => Some(a.and_then(|a| b.map(|b| a.and(b)))),
    };
    let event = match swept {
        None | Some(Ok(SweepReport { removed: 0, .. })) => return,
        Some(Ok(r)) => Event::new(
            Level::Info,
            Component::Transport,
            "shm.orphaned_segments_removed",
        )
        .with_attr("removed", r.removed.to_string())
        .with_attr("held", r.held.to_string())
        .with_attr("young", r.young.to_string())
        .with_attr("foreign", r.foreign.to_string()),
        Some(Err(e)) => Event::new(Level::Warn, Component::Transport, "shm.orphan_sweep_failed")
            .with_attr("error", e.to_string()),
    };
    sink.record(&event);
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

    #[test]
    fn shm_config_satisfies_capacity_constraint_and_default() {
        // B8 / C3 variable zero-copy: the length-prefixed ring is
        // variable-size; `capacity >= 2*max_datagram + 16` is the
        // PosixShmTransport invariant. Default path (no env).
        let cfg = shm_config_for_pair(GuidPrefix::from_bytes([7u8; 12]), 0);
        assert_eq!(cfg.max_datagram, DEFAULT_MAX_DATAGRAM);
        assert!(
            cfg.capacity >= cfg.max_datagram * 2 + 16,
            "capacity {} < 2*max_datagram {} + 16",
            cfg.capacity,
            cfg.max_datagram
        );
        // Exercise the real constructor with a large explicit runtime limit.
        let big = 8 * 1024 * 1024usize;
        let large =
            shm_config_for_pair_with_limit(GuidPrefix::from_bytes([7u8; 12]), 0, big).unwrap();
        assert_eq!(large.max_datagram, big);
        assert!(large.capacity >= big * 2 + 16);
        assert!(shm_config_for_pair_with_limit(GuidPrefix::from_bytes([7u8; 12]), 0, 0).is_err());
        assert!(
            shm_config_for_pair_with_limit(GuidPrefix::from_bytes([7u8; 12]), 0, usize::MAX)
                .is_err()
        );
    }

    /// A pair's segment file is named after the two GUIDs; the directory
    /// carries the domain, so two participants in different domains that
    /// share a GUID prefix never share a segment.
    #[test]
    fn two_domains_never_share_a_segment_directory() {
        let prefix = GuidPrefix::from_bytes([7u8; 12]);
        let a = shm_config_for_pair(prefix, 3);
        let b = shm_config_for_pair(prefix, 4);
        assert_ne!(a.flink_dir, b.flink_dir);
        assert!(a.flink_dir.ends_with("d3"), "{}", a.flink_dir.display());
    }

    #[test]
    fn config_flink_dir_contains_host_hex() {
        // The host id is octets 2..6, after the vendor id.
        let prefix = GuidPrefix::zerodds([0xAB, 0xCD, 0x12, 0x34], [0; 6]);
        let cfg = shm_config_for_pair(prefix, 0);
        let s = cfg.flink_dir.to_string_lossy();
        assert!(s.contains("zerodds-shm"));
        assert!(
            s.contains("abcd1234"),
            "flink_dir should contain host_id_hex: {s}"
        );
    }
}
