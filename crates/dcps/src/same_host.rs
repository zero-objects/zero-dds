// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Same-Host-Tracker fuer Welle 4 der Zero-Copy-Roadmap
//! (Spec `docs/specs/zerodds-zero-copy-1.0.md` §6 Welle 3).
//!
//! # Zweck
//!
//! Discovery erkennt via [`GuidPrefix::is_same_host`] (Welle 4a),
//! ob ein Peer auf der gleichen Maschine laeuft. Bei Match
//! registriert das DCPS-Runtime ein `(WriterGuid, ReaderGuid)`-Paar
//! in diesem Tracker. Der Tracker fuehrt eine deterministische
//! SHM-Segment-Identifikation, damit beide Seiten denselben
//! Transport-Pfad ohne extra Discovery-Roundtrip ableiten koennen.
//!
//! # Pfad-Convention
//!
//! Ein SHM-Segment fuer ein Writer-Reader-Paar wird identifiziert
//! ueber einen deterministischen 16-Byte-Hash der beiden GUIDs
//! (siehe [`shm_segment_id_for_pair`]). Beide Seiten kommen auf
//! dieselbe ID, weil der Hash symmetrisch bzw. ordered ist.
//!
//! Das `base_dir`-Default ist `${TMPDIR}/zerodds-shm` (Linux) bzw.
//! `${TEMP}\zerodds-shm` (Windows). Der `host_id_hex`-Sub-Ordner
//! verhindert Cross-Host-Kollisionen bei NFS-Mounts; sollte
//! `gethostname` fehlschlagen (Fallback in
//! `participant::host_id_bytes`), bleibt der Pfad pro Prozess
//! eindeutig.
//!
//! # Tracker-State
//!
//! Pro Paar gibt es einen [`SameHostState`]:
//!
//! - `Pending` — Match erkannt, SHM-Setup noch nicht erfolgt
//!   (z.B. weil die Welle-4b.2-Hook erst beim ersten Send das
//!   Segment auf-mappt).
//! - `Bound { transport, role }` — SHM-Transport ist initialisiert,
//!   `role` sagt welche Seite Owner (Writer-Producer) oder Consumer
//!   (Reader-Receiver) ist (siehe [`Role`]).
//! - `Failed { reason }` — Setup ist fehlgeschlagen, der Sender soll
//!   auf den klassischen UDP-Pfad zurueckfallen.
//!
//! Die konkrete Transport-Instanziierung lebt **nicht** hier (das ist
//! Sache der Welle 4b.2-Hook im `runtime`-Modul), damit dieses Modul
//! `no_std + alloc`-portierbar bleibt und nicht direkt von
//! `transport-shm` abhaengt.
//!
//! # Spec-Anker
//!
//! - `docs/specs/zerodds-zero-copy-1.0.md` §6 Welle 3 (Iceoryx-Backend-
//!   Hot-Path-Wiring; ZeroDDS reuses fuer SHM-Bytes-Pfad).
//! - RTPS 2.5 §9.4 — `LOCATOR_KIND_SHM`.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;

use zerodds_rtps::wire_types::Guid;

/// Default-Basis-Verzeichnis fuer SHM-Segmente. Wird nur in `std`-
/// Builds genutzt; im `no_std`-Profil ist der Tracker State-Only und
/// kein Pfad-Lookup noetig.
#[cfg(feature = "std")]
pub const DEFAULT_BASE_DIR_NAME: &str = "zerodds-shm";

/// Rolle des lokalen Endpunkts im SHM-Paar.
///
/// Konvention folgt `zerodds-transport-shm` (Owner = Producer):
/// im DCPS-Modell schreibt der **Writer** Samples in den
/// Ringbuffer → Writer ist der `Owner`. Der **Reader** liest sie
/// raus → Reader ist der `Consumer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Writer-Seite — erzeugt das SHM-Segment via `open_owner`
    /// und schreibt Sample-Datagrams hinein.
    Owner,
    /// Reader-Seite — attached via `open_consumer` und liest
    /// Sample-Datagrams aus dem Segment.
    Consumer,
}

/// Zustand eines Same-Host-Paares im Tracker.
#[derive(Clone)]
pub enum SameHostState {
    /// Match erkannt, SHM-Transport noch nicht aufgesetzt.
    Pending,
    /// SHM-Transport aktiv. `transport` ist ein opaker `Arc<dyn Any>`,
    /// damit dieses Modul kein direkter `transport-shm`-Abhaengiger
    /// ist. Der `runtime`-Hook downcastet zur konkreten Transport-
    /// Instanz.
    Bound {
        /// Opaker Transport-Pointer; `Arc<dyn core::any::Any + Send + Sync>`.
        transport: Arc<dyn core::any::Any + Send + Sync>,
        /// Rolle dieses Endpunkts.
        role: Role,
    },
    /// SHM-Setup ist fehlgeschlagen; UDP-Fallback aktiv.
    Failed {
        /// Diagnose-Text fuer Logs.
        reason: &'static str,
    },
}

impl fmt::Debug for SameHostState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => f.write_str("Pending"),
            Self::Bound { role, .. } => f.debug_struct("Bound").field("role", role).finish(),
            Self::Failed { reason } => f.debug_struct("Failed").field("reason", reason).finish(),
        }
    }
}

/// Deterministische 16-Byte-Segment-Identifikation fuer ein
/// Writer-Reader-Paar.
///
/// Beide Seiten errechnen denselben Wert: der Hash mischt die
/// vollstaendigen Guids in fester Reihenfolge (Writer zuerst, dann
/// Reader). FNV1a-128 ist deterministisch und ohne externe Deps.
///
/// Der Wert wird typischerweise in einen Hex-String fuer den
/// Dateinamen ueberfuehrt (siehe [`shm_segment_filename`]).
#[must_use]
pub fn shm_segment_id_for_pair(writer: Guid, reader: Guid) -> [u8; 16] {
    let mut buf = [0u8; 32];
    buf[..16].copy_from_slice(&writer.to_bytes());
    buf[16..].copy_from_slice(&reader.to_bytes());
    fnv1a_128(&buf)
}

/// Hex-Repraesentation eines [`shm_segment_id_for_pair`]-Ergebnisses.
/// Liefert 32 Lowercase-Hex-Zeichen.
#[must_use]
pub fn shm_segment_filename(id: [u8; 16]) -> alloc::string::String {
    let mut s = alloc::string::String::with_capacity(32);
    for b in id {
        // SAFETY: kein unsafe; format!-Vermeidung haelt no_std-kompatibel.
        const HEX: &[u8; 16] = b"0123456789abcdef";
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0F) as usize] as char);
    }
    s
}

/// FNV1a-128 ueber `data` (deterministisch, kein externer Dep).
fn fnv1a_128(data: &[u8]) -> [u8; 16] {
    let prime: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013B;
    let offset: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    let mut h = offset;
    for &b in data {
        h ^= u128::from(b);
        h = h.wrapping_mul(prime);
    }
    h.to_le_bytes()
}

// ============================================================================
// Tracker
// ============================================================================

/// Tracker fuer alle `(WriterGuid, ReaderGuid)`-Same-Host-Paare des
/// lokalen Participants. Threadsafe via Inner-Mutex.
///
/// API ist bewusst minimal: `register_pending`, `mark_bound`,
/// `mark_failed`, `lookup`. Komplexere Operationen (Iteration,
/// Cleanup-on-Drop) liegen beim Caller.
#[cfg(feature = "std")]
pub struct SameHostTracker {
    pairs: std::sync::RwLock<alloc::collections::BTreeMap<(Guid, Guid), SameHostState>>,
}

#[cfg(feature = "std")]
impl SameHostTracker {
    /// Neuer leerer Tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pairs: std::sync::RwLock::new(alloc::collections::BTreeMap::new()),
        }
    }

    /// Registriert ein Paar im `Pending`-State. Idempotent — ein
    /// existierender Eintrag wird **nicht** ueberschrieben (damit
    /// `Bound`-State nicht versehentlich zurueckgesetzt wird).
    pub fn register_pending(&self, writer: Guid, reader: Guid) {
        if let Ok(mut g) = self.pairs.write() {
            g.entry((writer, reader)).or_insert(SameHostState::Pending);
        }
    }

    /// Markiert ein Paar als `Bound` mit Transport-Handle und Rolle.
    /// Ueberschreibt jeden vorherigen State.
    pub fn mark_bound(
        &self,
        writer: Guid,
        reader: Guid,
        transport: Arc<dyn core::any::Any + Send + Sync>,
        role: Role,
    ) {
        if let Ok(mut g) = self.pairs.write() {
            g.insert((writer, reader), SameHostState::Bound { transport, role });
        }
    }

    /// Markiert ein Paar als `Failed`. Caller (Hot-Path) soll
    /// dann UDP-Fallback waehlen.
    pub fn mark_failed(&self, writer: Guid, reader: Guid, reason: &'static str) {
        if let Ok(mut g) = self.pairs.write() {
            g.insert((writer, reader), SameHostState::Failed { reason });
        }
    }

    /// Liefert eine Kopie des State fuer ein Paar.
    #[must_use]
    pub fn lookup(&self, writer: Guid, reader: Guid) -> Option<SameHostState> {
        self.pairs.read().ok()?.get(&(writer, reader)).cloned()
    }

    /// Entfernt einen Eintrag (z.B. wenn der Match aufgeloest wird).
    pub fn remove(&self, writer: Guid, reader: Guid) {
        if let Ok(mut g) = self.pairs.write() {
            g.remove(&(writer, reader));
        }
    }

    /// Anzahl der aktuell registrierten Paare.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.read().map(|g| g.len()).unwrap_or(0)
    }

    /// `true` wenn der Tracker leer ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Snapshot aller Paare. Hauptsaechlich fuer Debug/Diagnose.
    #[must_use]
    pub fn snapshot(&self) -> Vec<(Guid, Guid, SameHostState)> {
        self.pairs
            .read()
            .map(|g| {
                g.iter()
                    .map(|(&(w, r), s)| (w, r, s.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }
}

#[cfg(feature = "std")]
impl Default for SameHostTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl fmt::Debug for SameHostTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SameHostTracker")
            .field("len", &self.len())
            .finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

    fn writer_guid(seed: u8) -> Guid {
        Guid::new(
            GuidPrefix::from_bytes([seed; 12]),
            EntityId::user_writer_with_key([seed, seed, seed]),
        )
    }

    fn reader_guid(seed: u8) -> Guid {
        Guid::new(
            GuidPrefix::from_bytes([seed; 12]),
            EntityId::user_reader_with_key([seed, seed, seed]),
        )
    }

    #[test]
    fn segment_id_is_deterministic_for_same_input() {
        let w = writer_guid(0xAA);
        let r = reader_guid(0xBB);
        let a = shm_segment_id_for_pair(w, r);
        let b = shm_segment_id_for_pair(w, r);
        assert_eq!(a, b, "same input → same ID");
    }

    #[test]
    fn segment_id_differs_for_swapped_pair() {
        // (writer, reader) muss sich von (reader, writer) unterscheiden;
        // sonst Owner/Consumer-Rollen unklar.
        let w = writer_guid(0xAA);
        let r = reader_guid(0xBB);
        let a = shm_segment_id_for_pair(w, r);
        let b = shm_segment_id_for_pair(r, w);
        assert_ne!(a, b, "ordered hash: pair (w,r) ≠ (r,w)");
    }

    #[test]
    fn segment_id_differs_for_different_pairs() {
        let a = shm_segment_id_for_pair(writer_guid(1), reader_guid(2));
        let b = shm_segment_id_for_pair(writer_guid(1), reader_guid(3));
        assert_ne!(a, b);
    }

    #[test]
    fn segment_filename_is_32_lowercase_hex_chars() {
        let id = shm_segment_id_for_pair(writer_guid(7), reader_guid(8));
        let name = shm_segment_filename(id);
        assert_eq!(name.len(), 32);
        assert!(
            name.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
        );
    }

    #[test]
    fn tracker_register_then_lookup_pending() {
        let t = SameHostTracker::new();
        let w = writer_guid(1);
        let r = reader_guid(2);
        t.register_pending(w, r);
        assert!(matches!(t.lookup(w, r), Some(SameHostState::Pending)));
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn tracker_mark_bound_overwrites_pending() {
        let t = SameHostTracker::new();
        let w = writer_guid(1);
        let r = reader_guid(2);
        t.register_pending(w, r);
        let dummy: Arc<dyn core::any::Any + Send + Sync> = Arc::new(42u32);
        t.mark_bound(w, r, dummy, Role::Owner);
        let st = t.lookup(w, r).expect("entry");
        match st {
            SameHostState::Bound { role, .. } => assert_eq!(role, Role::Owner),
            other => panic!("expected Bound, got {other:?}"),
        }
    }

    #[test]
    fn tracker_register_pending_is_idempotent() {
        let t = SameHostTracker::new();
        let w = writer_guid(1);
        let r = reader_guid(2);
        let dummy: Arc<dyn core::any::Any + Send + Sync> = Arc::new(42u32);
        t.mark_bound(w, r, dummy, Role::Consumer);
        // Re-register sollte den Bound-State NICHT auf Pending zuruecksetzen.
        t.register_pending(w, r);
        assert!(matches!(t.lookup(w, r), Some(SameHostState::Bound { .. })));
    }

    #[test]
    fn tracker_mark_failed_signals_udp_fallback() {
        let t = SameHostTracker::new();
        let w = writer_guid(1);
        let r = reader_guid(2);
        t.mark_failed(w, r, "shm_open ENOENT");
        match t.lookup(w, r) {
            Some(SameHostState::Failed { reason }) => assert!(reason.contains("ENOENT")),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn tracker_remove_drops_entry() {
        let t = SameHostTracker::new();
        let w = writer_guid(1);
        let r = reader_guid(2);
        t.register_pending(w, r);
        t.remove(w, r);
        assert!(t.lookup(w, r).is_none());
        assert!(t.is_empty());
    }
}
