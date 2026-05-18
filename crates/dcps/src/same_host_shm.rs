// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Welle 4b.3 (Spec `docs/specs/zerodds-zero-copy-1.0.md` §6 Welle 3):
//! feature-gated Bruecke zwischen [`crate::same_host`] (no_std-Tracker)
//! und `zerodds_transport_shm::PosixShmTransport`.
//!
//! Nur kompiliert mit `same-host-shm`-Feature aktiviert. Ohne das
//! Feature bleibt der Tracker reine Pending-State-Machine und der
//! Hot-Path faellt auf UDP zurueck.
//!
//! # Pfad-Resolution
//!
//! `flink_dir = ${TMPDIR}/zerodds-shm/${host_id_hex}/`
//!
//! `host_id_hex` ist die Hex-Repraesentation der ersten 4 Bytes des
//! lokalen GuidPrefixes (Welle 4a host-id). Damit kollidieren
//! Same-Host-Segmente nicht ueber unterschiedliche Hosts auf
//! NFS-Mounts hinweg.
//!
//! Die Segment-Dateinamen leiten sich via
//! [`crate::same_host::shm_segment_filename`] vom 128-Bit-Hash des
//! `(writer_guid, reader_guid)`-Paares ab. Beide Seiten kommen ohne
//! extra Discovery-Roundtrip auf denselben Pfad.

#![cfg(feature = "same-host-shm")]

use alloc::sync::Arc;
use core::any::Any;
use std::time::Duration;

use zerodds_rtps::wire_types::{Guid, GuidPrefix};
use zerodds_transport_shm::{PosixShmTransport, ShmConfig};

use crate::same_host::Role;

/// Default-Datagram-Limit fuer den Same-Host-SHM-Pfad. Liberaler
/// als der DDSI-RTPS-MTU (1472 Byte), damit grosse Fragments lokal
/// nicht unnoetig zerlegt werden muessen.
pub const DEFAULT_MAX_DATAGRAM: usize = 64 * 1024;

/// Default-Ringbuffer-Kapazitaet (Bytes). Muss `>= 2 * max_datagram`
/// sein (PosixShmTransport-Constraint).
pub const DEFAULT_CAPACITY: usize = 2 * 1024 * 1024;

/// Berechnet den `ShmConfig` fuer ein Same-Host-Paar.
///
/// Resultat-`flink_dir` ist absolut und enthaelt den `host_id_hex`-
/// Sub-Ordner. `${TMPDIR}` faellt auf `/tmp` zurueck, wenn die
/// Env-Var nicht gesetzt ist.
#[must_use]
pub fn shm_config_for_pair(local_prefix: GuidPrefix) -> ShmConfig {
    let host_id = local_prefix.host_id();
    let host_hex = bytes_to_hex(&host_id);
    let base = std::env::temp_dir();
    let flink_dir = base.join("zerodds-shm").join(host_hex);
    ShmConfig {
        capacity: DEFAULT_CAPACITY,
        flink_dir,
        max_datagram: DEFAULT_MAX_DATAGRAM,
        recv_timeout: Some(Duration::from_millis(50)),
    }
}

/// Versucht, ein `PosixShmTransport`-Segment als `Owner` aufzusetzen.
///
/// Die Owner-Seite ist im DCPS-Modell der **Writer** — er erzeugt
/// das Segment und schreibt Sample-Datagrams hinein. Der Reader
/// attached spaeter als Consumer.
///
/// Liefert bei Erfolg ein opakes `Arc<dyn Any + Send + Sync>`, das
/// vom [`crate::same_host::SameHostTracker`] in `Bound { transport,
/// role: Owner }` gespeichert wird. Der Hot-Path-Sender (Welle 4b.4)
/// downcastet zurueck nach `PosixShmTransport`.
///
/// Bei Fehler liefert die Funktion eine statische Diagnose-String;
/// der Caller markiert den Tracker-Entry als `Failed` und faellt auf
/// UDP zurueck.
pub fn open_owner_segment(
    local_prefix: GuidPrefix,
    writer_guid: Guid,
    reader_guid: Guid,
) -> Result<Arc<dyn Any + Send + Sync>, &'static str> {
    let cfg = shm_config_for_pair(local_prefix);
    // flink_dir muss existieren; PosixShmTransport::open_owner ruft
    // intern ensure_base_dir, aber wir wollen einen klaren Fehlerpfad.
    if std::fs::create_dir_all(&cfg.flink_dir).is_err() {
        return Err("shm: flink_dir create failed");
    }
    // Owner = Writer; peer = Reader.
    let local_id = writer_guid.to_bytes();
    let peer_id = reader_guid.to_bytes();
    match PosixShmTransport::open_owner(local_id, peer_id, cfg) {
        Ok(t) => Ok(Arc::new(t) as Arc<dyn Any + Send + Sync>),
        Err(_) => Err("shm: open_owner failed"),
    }
}

/// Versucht, ein `PosixShmTransport`-Segment als `Consumer` zu
/// attachen.
///
/// Der Owner (Writer-Seite) muss das Segment vorher aufgesetzt
/// haben. Bei Owner-noch-nicht-da liefert `PosixShmTransport::
/// open_consumer` einen `MapOpenFailed`, dann markiert der Caller
/// `Failed`. Spaeterer SEDP-Re-Match darf einen erneuten Versuch
/// triggern (Welle 4b.4-Folge).
pub fn open_consumer_segment(
    local_prefix: GuidPrefix,
    writer_guid: Guid,
    reader_guid: Guid,
) -> Result<Arc<dyn Any + Send + Sync>, &'static str> {
    let cfg = shm_config_for_pair(local_prefix);
    // Consumer = Reader; peer = Writer (Owner).
    let local_id = reader_guid.to_bytes();
    let peer_id = writer_guid.to_bytes();
    match PosixShmTransport::open_consumer(local_id, peer_id, cfg) {
        Ok(t) => Ok(Arc::new(t) as Arc<dyn Any + Send + Sync>),
        Err(_) => Err("shm: open_consumer failed"),
    }
}

/// Helper: `Role` fuer einen `(writer, reader)`-Pair in dem Sinne,
/// dass `local_prefix` einer der beiden Seiten ist.
///
/// Liefert `Some(Role::Owner)` wenn `local_prefix == writer.prefix`
/// (Writer-Seite produziert), `Some(Role::Consumer)` wenn
/// `local_prefix == reader.prefix` (Reader-Seite konsumiert),
/// `None` bei beidem (selbst-Match) oder keinem (Fehler im Caller).
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

// Sanity-Cross-Check: `shm_segment_filename` produziert deterministische
// Hex-Strings; das wird im no-std-Modul `same_host` getestet. Hier nur
// die Bytes-to-Hex-Variante fuer host_id.

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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

    /// Welle 4c — E2E-Test: open_owner_segment +
    /// open_consumer_segment finden einander ueber die globale
    /// Pfad-Convention (`shm_config_for_pair`, also
    /// `${TMPDIR}/zerodds-shm/${host_hex}/...`) ohne explizite
    /// Config-Sharing. Validiert dass beide Seiten den
    /// SHM-Segment-Pfad deterministisch ableiten und Bytes
    /// roundtrippen.
    ///
    /// GUIDs werden bewusst hoch-eindeutig gewaehlt (Test-uniques
    /// 0xE2E2 prefix), damit kein Konflikt mit parallel laufenden
    /// Tests auf demselben System.
    #[test]
    fn welle_4c_pair_setup_via_path_convention_roundtrip() {
        use zerodds_transport::Transport;
        // Eindeutige GuidPrefixe: gleicher host_id (4b match
        // detection), aber abweichende PIDs/Counter — analog Welle
        // 4a-Schema in participant::random_guid_prefix.
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

        // Setup wie SEDP-Hook: Writer-Seite oeffnet Owner zuerst,
        // Reader-Seite attached als Consumer.
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

    /// Writer (Owner) → Reader (Consumer) Roundtrip via tempdir.
    /// Validiert die DCPS-Rollen-Konvention: Writer schreibt,
    /// Reader liest.
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
        // Owner = Writer; Consumer = Reader.
        let owner = PosixShmTransport::open_owner(w.to_bytes(), r.to_bytes(), cfg.clone())
            .expect("open_owner");
        let consumer = PosixShmTransport::open_consumer(r.to_bytes(), w.to_bytes(), cfg)
            .expect("open_consumer");
        // Writer (Owner) sendet, Reader (Consumer) empfaengt.
        let consumer_loc = consumer.local_locator();
        owner.send(&consumer_loc, b"hello-same-host").expect("send");
        let got = consumer.recv().expect("recv");
        assert_eq!(&got.data[..], b"hello-same-host");
    }
}
