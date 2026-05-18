// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! 4b.5 (Spec `docs/specs/zerodds-zero-copy-1.0.md` §6 Welle 3
//! Sub-Sprints): UDS-Datagram-basierter alternativer Same-Host-Pfad.
//!
//! Aktiv nur mit Feature `same-host-uds` (mutually exclusive zu
//! `same-host-shm` — bei beiden aktiv gewinnt SHM-Pfad, siehe
//! `runtime.rs`-Hook-Cascade).
//!
//! # Trade-off vs SHM
//!
//! UDS-Datagram ist KEIN echtes Zero-Copy — der Kernel kopiert die
//! Bytes von Sender-User-Space ueber Socket-Buffer in den Receiver-
//! User-Space. Aber:
//! - eliminiert IP-Stack-Overhead (kein UDP-Header, kein routing)
//! - funktioniert in Sandboxed-Containern ohne `/dev/shm`
//! - 1:N-fähig (mehrere Writer pro Reader-Endpoint, ohne extra
//!   Segment-Allokation)
//!
//! # Pfad-Convention
//!
//! UDS-Sockets liegen unter `${TMPDIR}/zerodds-uds/${host_hex}/`.
//! Der lokale Endpoint-Id ist die ersten 16 Bytes der Reader-GUID
//! (Owner-Seite) bzw. Writer-GUID (Consumer-Seite); ein UDS-`bind`
//! erstellt den Socket-Pfad und ein `sendto` adressiert ihn direkt
//! (1:N-Modell).

#![cfg(feature = "same-host-uds")]

use alloc::sync::Arc;
use core::any::Any;
use std::path::PathBuf;
use std::time::Duration;

use zerodds_rtps::wire_types::{Guid, GuidPrefix};
use zerodds_transport_uds::{UdsConfig, UdsTransport};

use crate::same_host::Role;

/// Default-Datagram-Limit fuer den UDS-Same-Host-Pfad.
pub const DEFAULT_MAX_DATAGRAM: usize = 64 * 1024;

/// Berechnet den `UdsConfig` fuer Same-Host-Paare.
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

/// Owner-Side: bindet einen UdsTransport am Reader-GUID-Pfad.
///
/// Im UDS-Modell ist der Reader der Owner (er bindet den
/// well-known Pfad), Writer adressieren ihn via `sendto`.
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

/// Consumer-Side: bindet einen separaten UdsTransport am Writer-
/// GUID-Pfad und adressiert spaeter den Reader via `send_to`.
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

/// Helper: Rolle des lokalen Endpunkts (identisch zu
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
