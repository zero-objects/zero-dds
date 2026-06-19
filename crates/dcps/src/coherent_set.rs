// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Coherent sets + group access (DDS DCPS 1.4 §2.2.2.4.1.8-11,
//! §2.2.2.5.2.8-11, §2.2.2.5.3.32; DDSI-RTPS 2.5 §9.6.4.2/3/4).
//!
//! When `Presentation.coherent_access = true`, a publisher can group
//! a sequence of write() calls into a **coherent set** — the
//! subscriber sees either all samples or none, never a partial set.
//! The wire mechanism is the inline-QoS PID `PID_COHERENT_SET`
//! (sequence_number of the first sample in the set).
//!
//! This module provides:
//! - [`CoherentScope`] — the state-machine tracker on the publisher /
//!   subscriber.
//! - [`CoherentSetMarker`] — the wire representation that ends up in
//!   the inline QoS.
//! - Helper functions for begin/end_coherent_changes on the publisher
//!   side and begin/end_access on the subscriber side.
//!
//! Scope: API surface + state tracking. The actual inline-QoS PID
//! wiring in the DCPS write/take path follows in the wire-up (depends
//! on the KeyHash inline-QoS wiring).

extern crate alloc;

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use crate::error::{DdsError, Result};

/// 8-byte SequenceNumber equivalent (DDSI-RTPS wire format).
pub type CoherentSequenceNumber = i64;

/// Wire representation of a `PID_COHERENT_SET` entry. Written by the
/// DataWriter into the inline QoS of a DATA submessage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoherentSetMarker {
    /// SequenceNumber of the **first** sample in the coherent set.
    pub set_first_sn: CoherentSequenceNumber,
}

impl CoherentSetMarker {
    /// 8-byte wire representation (BE — matches
    /// `SequenceNumber::to_bytes_be`).
    #[must_use]
    pub fn to_wire_bytes(&self) -> [u8; 8] {
        // RTPS SequenceNumber format: high 32 bits + low 32 bits, both BE.
        let high = (self.set_first_sn >> 32) as i32;
        let low = (self.set_first_sn & 0xFFFF_FFFF) as u32;
        let mut out = [0u8; 8];
        out[0..4].copy_from_slice(&high.to_be_bytes());
        out[4..8].copy_from_slice(&low.to_be_bytes());
        out
    }

    /// Decode from 8 bytes BE.
    #[must_use]
    pub fn from_wire_bytes(bytes: &[u8; 8]) -> Self {
        let high = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let low = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let sn = (i64::from(high) << 32) | i64::from(low);
        Self { set_first_sn: sn }
    }
}

/// State-machine tracker for the coherent set on the publisher side.
///
/// Lifecycle:
/// 1. `begin_coherent_changes` sets `active=true` and remembers the
///    next sequence number as `set_first_sn`.
/// 2. Every `write()` during the active set carries this marker.
/// 3. `end_coherent_changes` sets `active=false`. The next write
///    *without* a marker, or with a new marker, signals the end of the
///    set to the reader.
#[derive(Debug)]
pub struct CoherentScope {
    /// Currently open?
    active: AtomicBool,
    /// Sequence number of the first sample in the current set
    /// (sentinel `i64::MIN` = none set yet).
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
    /// New empty scope (in an Arc, because callers usually need to
    /// share it between the publisher inner and the write path).
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// True if the scope is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Returns the current marker if the scope is active.
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

    /// Begins a new coherent set with the given `next_sn` as the
    /// set-first SN. Spec: §2.2.2.4.1.8
    /// `Publisher::begin_coherent_changes`.
    ///
    /// # Errors
    /// `PreconditionNotMet` if a set is already active (per spec, not
    /// currently nestable).
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

    /// Ends the active coherent set. Spec: §2.2.2.4.1.9
    /// `Publisher::end_coherent_changes`.
    ///
    /// # Errors
    /// `PreconditionNotMet` if no set is active.
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

/// Subscriber-side group access (Spec §2.2.2.5.2.8/9).
///
/// When `Presentation.access_scope = GROUP` and `coherent_access` is
/// active, the subscriber MUST guarantee the atomic view across all
/// DataReaders in the subscriber between `begin_access` and
/// `end_access`.
///
/// Group-access scope with snapshot atomicity: each `begin_access`
/// increments the `snapshot_generation` counter; all DataReaders in
/// the subscriber that are read between begin and end see the SAME
/// generation and therefore, per spec, an atomic cut across all
/// topics.
#[derive(Debug, Default)]
pub struct GroupAccessScope {
    /// Counter of currently open begin_access calls (the spec allows
    /// recursive nesting).
    open_count: core::sync::atomic::AtomicU32,
    /// Snapshot generation: incremented on the first `begin()`
    /// (cur=0→1) and stays stable until the last `end()` closes it.
    /// DataReader read sites can read `current_snapshot()` to define a
    /// consistent cut.
    snapshot_generation: core::sync::atomic::AtomicU64,
}

impl GroupAccessScope {
    /// New empty scope.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// True if at least one begin_access is currently open.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.open_count.load(Ordering::Acquire) > 0
    }

    /// Current snapshot generation (see struct docs). 0 means "no
    /// snapshot ever opened".
    #[must_use]
    pub fn current_snapshot(&self) -> u64 {
        self.snapshot_generation.load(Ordering::Acquire)
    }

    /// `Subscriber::begin_access` (Spec §2.2.2.5.2.8). Idempotently
    /// nestable — each call raises the counter, each `end_access`
    /// lowers it. On the 0→1 transition the snapshot generation is
    /// also incremented (atomic cut begin).
    pub fn begin(&self) {
        let prev = self.open_count.fetch_add(1, Ordering::AcqRel);
        if prev == 0 {
            self.snapshot_generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// `Subscriber::end_access` (Spec §2.2.2.5.2.9).
    ///
    /// # Errors
    /// `PreconditionNotMet` if no begin_access is open (counter would
    /// underflow).
    pub fn end(&self) -> Result<()> {
        // Safe decrement: read+CAS loop to detect underflow.
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

    // ---- §2.2.3.6 GROUP-coherent_access snapshot generation ----

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
        // Generation persists after end() — every new begin yields a
        // new generation. This is the "atomic cut" identity.
        assert_eq!(g.current_snapshot(), 1);
        g.begin();
        assert_eq!(g.current_snapshot(), 2);
    }

    #[test]
    fn snapshot_generation_stable_during_nested_begin() {
        // Within nested begin/end the generation should stay
        // constant — we see the same cut.
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
        // Multi-reader coherent set: all DRs can see the same scope
        // via cloning → identical snapshot_generation.
        let g = GroupAccessScope::new();
        let g_for_dr1 = Arc::clone(&g);
        let g_for_dr2 = Arc::clone(&g);
        g.begin();
        // DR1 + DR2 see the same cut.
        assert_eq!(g_for_dr1.current_snapshot(), g_for_dr2.current_snapshot());
        assert_eq!(g_for_dr1.current_snapshot(), 1);
        g.end().unwrap();
    }
}
