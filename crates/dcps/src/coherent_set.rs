// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Coherent-Sets + Group-Access (DDS DCPS 1.4 §2.2.2.4.1.8-11,
//! §2.2.2.5.2.8-11, §2.2.2.5.3.32; DDSI-RTPS 2.5 §9.6.4.2/3/4).
//!
//! Wenn `Presentation.coherent_access = true`, kann ein Publisher
//! eine Sequenz von write()-Aufrufen in einem **Coherent-Set**
//! gruppieren — der Subscriber sieht entweder alle Samples oder
//! keines, niemals einen Teil. Der Wire-Mechanismus ist die
//! Inline-QoS-PID `PID_COHERENT_SET` (sequence_number des ersten
//! Sample im Set).
//!
//! .9 liefert:
//! - [`CoherentScope`] — der State-Machine-Tracker im Publisher /
//!   Subscriber.
//! - [`CoherentSetMarker`] — die Wire-Repraesentation, die in der
//!   Inline-QoS landet.
//! - Hilfsfunktionen fuer begin/end_coherent_changes auf Publisher-
//!   und begin/end_access auf Subscriber-Seite.
//!
//! Scope: API-Surface + State-Tracking. Tatsaechliche
//! Inline-QoS-PID-Wiring im DCPS-write/take-Pfad folgt im Wire-Up
//! (deps auf KeyHash-Inline-QoS-Wiring).

extern crate alloc;

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use crate::error::{DdsError, Result};

/// 8-byte SequenceNumber-Aequivalent (DDSI-RTPS Wire-Format).
pub type CoherentSequenceNumber = i64;

/// Wire-Repraesentation eines `PID_COHERENT_SET`-Eintrags. Wird vom
/// DataWriter in die Inline-QoS einer DATA-Submessage geschrieben.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoherentSetMarker {
    /// SequenceNumber des **ersten** Sample im Coherent-Set.
    pub set_first_sn: CoherentSequenceNumber,
}

impl CoherentSetMarker {
    /// 8-byte Wire-Repraesentation (BE — entspricht
    /// `SequenceNumber::to_bytes_be`).
    #[must_use]
    pub fn to_wire_bytes(&self) -> [u8; 8] {
        // RTPS SequenceNumber-Format: high 32 bits + low 32 bits, beides BE.
        let high = (self.set_first_sn >> 32) as i32;
        let low = (self.set_first_sn & 0xFFFF_FFFF) as u32;
        let mut out = [0u8; 8];
        out[0..4].copy_from_slice(&high.to_be_bytes());
        out[4..8].copy_from_slice(&low.to_be_bytes());
        out
    }

    /// Decode aus 8 byte BE.
    #[must_use]
    pub fn from_wire_bytes(bytes: &[u8; 8]) -> Self {
        let high = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let low = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let sn = (i64::from(high) << 32) | i64::from(low);
        Self { set_first_sn: sn }
    }
}

/// State-Machine-Tracker fuer Coherent-Set auf Publisher-Seite.
///
/// Lifecycle:
/// 1. `begin_coherent_changes` setzt `active=true` und merkt sich
///    die naechste Sequence-Number als `set_first_sn`.
/// 2. Jede `write()` waehrend des aktiven Sets traegt diesen Marker.
/// 3. `end_coherent_changes` setzt `active=false`. Das naechste write
///    *ohne* Marker oder mit neuem Marker signalisiert dem Reader
///    das Set-Ende.
#[derive(Debug)]
pub struct CoherentScope {
    /// Aktuell offen?
    active: AtomicBool,
    /// Sequence-Number des ersten Sample im aktuellen Set
    /// (Sentinel `i64::MIN` = noch keiner gesetzt).
    set_first_sn: AtomicI64,
}

impl Default for CoherentScope {
    fn default() -> Self {
        Self {
            active: AtomicBool::new(false),
            set_first_sn: AtomicI64::new(i64::MIN),
        }
    }
}

impl CoherentScope {
    /// Neuer leerer Scope (in Arc, weil Caller den meist shared
    /// zwischen Publisher-Inner + write-Pfad halten muessen).
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// True wenn der Scope aktiv ist.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Liefert den aktuellen Marker, falls Scope aktiv ist.
    #[must_use]
    pub fn current_marker(&self) -> Option<CoherentSetMarker> {
        if self.is_active() {
            let sn = self.set_first_sn.load(Ordering::Acquire);
            if sn != i64::MIN {
                return Some(CoherentSetMarker { set_first_sn: sn });
            }
        }
        None
    }

    /// Beginnt einen neuen Coherent-Set mit der gegebenen
    /// `next_sn` als Set-First-SN. Spec: §2.2.2.4.1.8
    /// `Publisher::begin_coherent_changes`.
    ///
    /// # Errors
    /// `PreconditionNotMet` wenn ein Set bereits aktiv ist
    /// (Spec: nicht verschachtelbar aktuell).
    pub fn begin(&self, next_sn: CoherentSequenceNumber) -> Result<()> {
        if self.active.load(Ordering::Acquire) {
            return Err(DdsError::PreconditionNotMet {
                reason: "coherent set already active",
            });
        }
        self.set_first_sn.store(next_sn, Ordering::Release);
        self.active.store(true, Ordering::Release);
        Ok(())
    }

    /// Beendet den aktiven Coherent-Set. Spec: §2.2.2.4.1.9
    /// `Publisher::end_coherent_changes`.
    ///
    /// # Errors
    /// `PreconditionNotMet` wenn kein Set aktiv ist.
    pub fn end(&self) -> Result<CoherentSetMarker> {
        let was = self.active.swap(false, Ordering::AcqRel);
        if !was {
            return Err(DdsError::PreconditionNotMet {
                reason: "no coherent set active",
            });
        }
        let sn = self.set_first_sn.swap(i64::MIN, Ordering::AcqRel);
        Ok(CoherentSetMarker { set_first_sn: sn })
    }
}

/// Subscriber-seitiger Group-Access (Spec §2.2.2.5.2.8/9).
///
/// Wenn `Presentation.access_scope = GROUP` und `coherent_access`
/// aktiv ist, MUSS der Subscriber zwischen `begin_access` und
/// `end_access` die Atomic-Sicht ueber alle DataReader im Subscriber
/// gewaehren.
///
/// Group-Access-Scope mit Snapshot-Atomicity: jedes `begin_access`
/// inkrementiert den `snapshot_generation`-Counter; alle DataReader
/// im Subscriber, die zwischen begin und end gelesen werden, sehen
/// die SELBE Generation und daher Spec-konform einen atomic Cut
/// ueber alle Topics.
#[derive(Debug, Default)]
pub struct GroupAccessScope {
    /// Counter der noch offenen begin_access-Aufrufe (Spec erlaubt
    /// rekursives Verschachteln).
    open_count: core::sync::atomic::AtomicU32,
    /// Snapshot-Generation: wird beim ersten `begin()` (cur=0→1)
    /// inkrementiert und stays-stable bis das letzte `end()` schließt.
    /// DataReader-Read-Sites koennen `current_snapshot()` lesen, um
    /// einen konsistenten Cut zu definieren.
    snapshot_generation: core::sync::atomic::AtomicU64,
}

impl GroupAccessScope {
    /// Neuer leerer Scope.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// True wenn aktuell mindestens ein begin_access offen ist.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.open_count.load(Ordering::Acquire) > 0
    }

    /// Aktuelle Snapshot-Generation (siehe Struct-Doku). 0 bedeutet
    /// "kein Snapshot je geoeffnet".
    #[must_use]
    pub fn current_snapshot(&self) -> u64 {
        self.snapshot_generation.load(Ordering::Acquire)
    }

    /// `Subscriber::begin_access` (Spec §2.2.2.5.2.8). Idempotent
    /// nestable — jeder Aufruf erhoeht den Counter, jedes
    /// `end_access` erniedrigt. Beim Uebergang von 0→1 wird zusaetzlich
    /// die Snapshot-Generation inkrementiert (atomic Cut-Begin).
    pub fn begin(&self) {
        let prev = self.open_count.fetch_add(1, Ordering::AcqRel);
        if prev == 0 {
            self.snapshot_generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// `Subscriber::end_access` (Spec §2.2.2.5.2.9).
    ///
    /// # Errors
    /// `PreconditionNotMet` wenn kein begin_access offen ist (Counter
    /// unterläuft).
    pub fn end(&self) -> Result<()> {
        // Sicherer Decrement: read+CAS-loop um Underflow zu erkennen.
        loop {
            let cur = self.open_count.load(Ordering::Acquire);
            if cur == 0 {
                return Err(DdsError::PreconditionNotMet {
                    reason: "end_access without begin_access",
                });
            }
            if self
                .open_count
                .compare_exchange(cur, cur - 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn marker_wire_roundtrip() {
        let m = CoherentSetMarker {
            set_first_sn: 0x0123_4567_89AB_CDEF,
        };
        let bytes = m.to_wire_bytes();
        let back = CoherentSetMarker::from_wire_bytes(&bytes);
        assert_eq!(m, back);
    }

    #[test]
    fn marker_wire_zero() {
        let m = CoherentSetMarker { set_first_sn: 0 };
        assert_eq!(m.to_wire_bytes(), [0u8; 8]);
    }

    #[test]
    fn coherent_scope_starts_inactive() {
        let s = CoherentScope::new();
        assert!(!s.is_active());
        assert!(s.current_marker().is_none());
    }

    #[test]
    fn begin_end_lifecycle() {
        let s = CoherentScope::new();
        s.begin(42).unwrap();
        assert!(s.is_active());
        let m = s.current_marker().expect("active should have marker");
        assert_eq!(m.set_first_sn, 42);
        let end = s.end().unwrap();
        assert_eq!(end.set_first_sn, 42);
        assert!(!s.is_active());
    }

    #[test]
    fn double_begin_is_error() {
        let s = CoherentScope::new();
        s.begin(1).unwrap();
        let err = s.begin(2).unwrap_err();
        assert!(matches!(err, DdsError::PreconditionNotMet { .. }));
    }

    #[test]
    fn end_without_begin_is_error() {
        let s = CoherentScope::new();
        let err = s.end().unwrap_err();
        assert!(matches!(err, DdsError::PreconditionNotMet { .. }));
    }

    #[test]
    fn group_access_nesting() {
        let g = GroupAccessScope::new();
        assert!(!g.is_active());
        g.begin();
        assert!(g.is_active());
        g.begin();
        g.end().unwrap();
        assert!(g.is_active(), "still nested");
        g.end().unwrap();
        assert!(!g.is_active());
    }

    #[test]
    fn group_access_underflow_is_error() {
        let g = GroupAccessScope::new();
        let err = g.end().unwrap_err();
        assert!(matches!(err, DdsError::PreconditionNotMet { .. }));
    }

    // ---- §2.2.3.6 GROUP-coherent_access Snapshot-Generation ----

    #[test]
    fn snapshot_generation_starts_zero() {
        let g = GroupAccessScope::new();
        assert_eq!(g.current_snapshot(), 0);
    }

    #[test]
    fn snapshot_generation_increments_on_begin_from_zero() {
        let g = GroupAccessScope::new();
        g.begin();
        assert_eq!(g.current_snapshot(), 1);
        g.end().unwrap();
        // Generation bleibt nach end() — jedes neue begin gibt eine
        // neue Generation. Das ist die "atomic cut"-Identitaet.
        assert_eq!(g.current_snapshot(), 1);
        g.begin();
        assert_eq!(g.current_snapshot(), 2);
    }

    #[test]
    fn snapshot_generation_stable_during_nested_begin() {
        // Innerhalb verschachtelter begin/end soll die Generation
        // konstant bleiben — wir sehen denselben Cut.
        let g = GroupAccessScope::new();
        g.begin();
        let g1 = g.current_snapshot();
        g.begin();
        let g2 = g.current_snapshot();
        assert_eq!(g1, g2, "nested begin must keep snapshot stable");
        g.end().unwrap();
        let g3 = g.current_snapshot();
        assert_eq!(g1, g3, "snapshot stays stable until last end");
        g.end().unwrap();
    }

    #[test]
    fn cross_topic_consistent_snapshot_via_clone() {
        // Multi-Reader-Coherent-Set: alle DR koennen via cloning
        // dieselbe Scope sehen → identische snapshot_generation.
        let g = GroupAccessScope::new();
        let g_for_dr1 = Arc::clone(&g);
        let g_for_dr2 = Arc::clone(&g);
        g.begin();
        // DR1 + DR2 sehen denselben Cut.
        assert_eq!(g_for_dr1.current_snapshot(), g_for_dr2.current_snapshot());
        assert_eq!(g_for_dr1.current_snapshot(), 1);
        g.end().unwrap();
    }
}
