// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! 4b.5 (Spec `docs/specs/zerodds-zero-copy-1.0.md` §6 Wave 3
//! sub-sprints): UDS-datagram-based alternative same-host path.
//!
//! Active only with the `same-host-uds` feature (mutually exclusive with
//! `same-host-shm` — if both are active the SHM path wins, see the
//! hook cascade in `runtime.rs`).
//!
//! # Trade-off vs SHM
//!
//! UDS datagram is NOT true zero-copy — the kernel copies the bytes from
//! the sender's user space through the socket buffer into the receiver's
//! user space. But:
//! - eliminates IP-stack overhead (no UDP header, no routing)
//! - works in sandboxed containers without `/dev/shm`
//! - 1:N capable (multiple writers per reader endpoint, without extra
//!   segment allocation)
//!
//! # Path convention
//!
//! UDS sockets live under `${TMPDIR}/zerodds-uds/${host_hex}/`. The
//! local endpoint id is the first 16 bytes of the reader GUID (owner
//! side) or writer GUID (consumer side); a UDS `bind` creates the socket
//! path and a `sendto` addresses it directly (1:N model).

#![cfg(feature = "same-host-uds")]

use alloc::sync::Arc;
use core::any::Any;
use std::path::PathBuf;
use std::time::Duration;

use zerodds_rtps::wire_types::{Guid, GuidPrefix};
use zerodds_transport_uds::{UdsConfig, UdsTransport};

use crate::same_host::Role;

/// Default datagram limit for the UDS same-host path.
pub const DEFAULT_MAX_DATAGRAM: usize = 64 * 1024;

/// Computes the `UdsConfig` for same-host pairs.
#[must_use]
pub fn uds_config_for_pair(local_prefix: GuidPrefix) -> UdsConfig {
    let host_id = local_prefix.host_id();
    let host_hex = bytes_to_hex(&host_id);
    let base = std::env::temp_dir();
    let base_dir: PathBuf = base.join("zerodds-uds").join(host_hex);
    UdsConfig {
        base_dir,
        max_datagram: DEFAULT_MAX_DATAGRAM,
        recv_timeout: Some(Duration::from_millis(50)),
    }
}

/// Owner side: binds a UdsTransport at the reader-GUID path.
///
/// In the UDS model the reader is the owner (it binds the well-known
/// path); writers address it via `sendto`.
pub fn open_owner_segment(
    local_prefix: GuidPrefix,
    _writer_guid: Guid,
    reader_guid: Guid,
) -> Result<Arc<dyn Any + Send + Sync>, &'static str> {
    let cfg = uds_config_for_pair(local_prefix);
    if std::fs::create_dir_all(&cfg.base_dir).is_err() {
        return Err("uds: base_dir create failed");
    }
    let local_id = reader_guid.to_bytes();
    match UdsTransport::bind(local_id, cfg) {
        Ok(t) => Ok(Arc::new(t) as Arc<dyn Any + Send + Sync>),
        Err(_) => Err("uds: bind owner failed"),
    }
}

/// Consumer side: binds a separate UdsTransport at the writer-GUID path
/// and later addresses the reader via `send_to`.
pub fn open_consumer_segment(
    local_prefix: GuidPrefix,
    writer_guid: Guid,
    _reader_guid: Guid,
) -> Result<Arc<dyn Any + Send + Sync>, &'static str> {
    let cfg = uds_config_for_pair(local_prefix);
    if std::fs::create_dir_all(&cfg.base_dir).is_err() {
        return Err("uds: base_dir create failed");
    }
    let local_id = writer_guid.to_bytes();
    match UdsTransport::bind(local_id, cfg) {
        Ok(t) => Ok(Arc::new(t) as Arc<dyn Any + Send + Sync>),
        Err(_) => Err("uds: bind consumer failed"),
    }
}

/// Helper: role of the local endpoint (identical to
/// `same_host_shm::local_role_for_pair`).
#[must_use]
pub fn local_role_for_pair(local_prefix: GuidPrefix, writer: Guid, reader: Guid) -> Option<Role> {
    let is_writer = local_prefix == writer.prefix;
    let is_reader = local_prefix == reader.prefix;
    match (is_writer, is_reader) {
        (true, false) => Some(Role::Consumer),
        (false, true) => Some(Role::Owner),
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use zerodds_rtps::wire_types::EntityId;

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
    fn config_base_dir_contains_host_hex() {
        let prefix = GuidPrefix::from_bytes([0xAB, 0xCD, 0x12, 0x34, 0, 0, 0, 0, 0, 0, 0, 0]);
        let cfg = uds_config_for_pair(prefix);
        let s = cfg.base_dir.to_string_lossy();
        assert!(s.contains("zerodds-uds"));
        assert!(
            s.contains("abcd1234"),
            "base_dir should contain host_id_hex: {s}"
        );
    }

    #[test]
    fn local_role_writer_yields_consumer() {
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
        assert_eq!(role, Some(Role::Consumer));
    }

    #[test]
    fn local_role_reader_yields_owner() {
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
        assert_eq!(role, Some(Role::Owner));
    }
}
