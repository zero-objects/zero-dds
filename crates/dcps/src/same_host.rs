// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Same-host tracker for Wave 4 of the zero-copy roadmap
//! (Spec `docs/specs/zerodds-zero-copy-1.0.md` §6 Wave 3).
//!
//! # Purpose
//!
//! Discovery detects via [`GuidPrefix::is_same_host`] (Wave 4a) whether
//! a peer runs on the same machine. On a match, the DCPS runtime
//! registers a `(WriterGuid, ReaderGuid)` pair in this tracker. The
//! tracker keeps a deterministic SHM-segment identification so that both
//! sides can derive the same transport path without an extra discovery
//! round-trip.
//!
//! # Path convention
//!
//! An SHM segment for a writer-reader pair is identified by a
//! deterministic 16-byte hash of the two GUIDs (see
//! [`shm_segment_id_for_pair`]). Both sides arrive at the same id because
//! the hash is symmetric / ordered.
//!
//! The `base_dir` default is `${TMPDIR}/zerodds-shm` (Linux) or
//! `${TEMP}\zerodds-shm` (Windows). The `host_id_hex` sub-folder prevents
//! cross-host collisions on NFS mounts; should `gethostname` fail
//! (fallback in `participant::host_id_bytes`), the path remains unique
//! per process.
//!
//! # Tracker state
//!
//! Each pair has a [`SameHostState`]:
//!
//! - `Pending` — match detected, SHM setup not yet performed (e.g.
//!   because the Wave-4b.2 hook only maps the segment on the first send).
//! - `Bound { transport, role }` — SHM transport is initialized; `role`
//!   says which side is Owner (writer producer) or Consumer (reader
//!   receiver) (see [`Role`]).
//! - `Failed { reason }` — setup failed; the sender should fall back to
//!   the classic UDP path.
//!
//! The concrete transport instantiation does **not** live here (that is
//! the job of the Wave-4b.2 hook in the `runtime` module), so this module
//! stays `no_std + alloc` portable and does not depend directly on
//! `transport-shm`.
//!
//! # Spec anchors
//!
//! - `docs/specs/zerodds-zero-copy-1.0.md` §6 Wave 3 (Iceoryx backend
//!   hot-path wiring; ZeroDDS reuses it for the SHM-bytes path).
//! - RTPS 2.5 §9.4 — `LOCATOR_KIND_SHM`.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;

use zerodds_rtps::wire_types::Guid;

/// Default base directory for SHM segments. Used only in `std` builds;
/// in the `no_std` profile the tracker is state-only and no path lookup
/// is needed.
#[cfg(feature = "std")]
pub const DEFAULT_BASE_DIR_NAME: &str = "zerodds-shm";

/// Role of the local endpoint in the SHM pair.
///
/// The convention follows `zerodds-transport-shm` (Owner = producer):
/// in the DCPS model the **writer** writes samples into the ring buffer
/// → the writer is the `Owner`. The **reader** reads them out → the
/// reader is the `Consumer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Writer side — creates the SHM segment via `open_owner` and writes
    /// sample datagrams into it.
    Owner,
    /// Reader side — attaches via `open_consumer` and reads sample
    /// datagrams out of the segment.
    Consumer,
}

/// State of a same-host pair in the tracker.
#[derive(Clone)]
pub enum SameHostState {
    /// Match detected, SHM transport not yet set up.
    Pending,
    /// SHM transport active. `transport` is an opaque `Arc<dyn Any>` so
    /// that this module does not depend directly on `transport-shm`. The
    /// `runtime` hook downcasts to the concrete transport instance.
    Bound {
        /// Opaque transport pointer; `Arc<dyn core::any::Any + Send + Sync>`.
        transport: Arc<dyn core::any::Any + Send + Sync>,
        /// Role of this endpoint.
        role: Role,
    },
    /// SHM setup failed; UDP fallback active.
    Failed {
        /// Diagnostic text for logs.
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

/// Deterministic 16-byte segment identification for a writer-reader
/// pair.
///
/// Both sides compute the same value: the hash mixes the complete GUIDs
/// in a fixed order (writer first, then reader). FNV1a-128 is
/// deterministic and has no external deps.
///
/// The value is typically converted into a hex string for the file name
/// (see [`shm_segment_filename`]).
#[must_use]
pub fn shm_segment_id_for_pair(writer: Guid, reader: Guid) -> [u8; 16] {
    let mut buf = [0u8; 32];
    buf[..16].copy_from_slice(&writer.to_bytes());
    buf[16..].copy_from_slice(&reader.to_bytes());
    fnv1a_128(&buf)
}

/// Hex representation of a [`shm_segment_id_for_pair`] result. Returns
/// 32 lowercase hex characters.
#[must_use]
pub fn shm_segment_filename(id: [u8; 16]) -> alloc::string::String {
    let mut s = alloc::string::String::with_capacity(32);
    for b in id {
        // SAFETY: no unsafe; avoiding format! keeps this no_std compatible.
        const HEX: &[u8; 16] = b"0123456789abcdef";
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0F) as usize] as char);
    }
    s
}

/// FNV1a-128 over `data` (deterministic, no external dep).
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

/// Tracker for all `(WriterGuid, ReaderGuid)` same-host pairs of the
/// local participant. Thread-safe via an inner mutex.
///
/// The API is deliberately minimal: `register_pending`, `mark_bound`,
/// `mark_failed`, `lookup`. More complex operations (iteration,
/// cleanup-on-drop) are left to the caller.
#[cfg(feature = "std")]
pub struct SameHostTracker {
    pairs: std::sync::RwLock<alloc::collections::BTreeMap<(Guid, Guid), SameHostState>>,
}

#[cfg(feature = "std")]
impl SameHostTracker {
    /// New empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pairs: std::sync::RwLock::new(alloc::collections::BTreeMap::new()),
        }
    }

    /// Registers a pair in the `Pending` state. Idempotent — an existing
    /// entry is **not** overwritten (so that a `Bound` state is not
    /// accidentally reset).
    pub fn register_pending(&self, writer: Guid, reader: Guid) {
        if let Ok(mut g) = self.pairs.write() {
            g.entry((writer, reader)).or_insert(SameHostState::Pending);
        }
    }

    /// Marks a pair as `Bound` with a transport handle and role.
    /// Overwrites any previous state.
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

    /// Marks a pair as `Failed`. The caller (hot path) should then choose
    /// the UDP fallback.
    pub fn mark_failed(&self, writer: Guid, reader: Guid, reason: &'static str) {
        if let Ok(mut g) = self.pairs.write() {
            g.insert((writer, reader), SameHostState::Failed { reason });
        }
    }

    /// Returns a copy of the state for a pair.
    #[must_use]
    pub fn lookup(&self, writer: Guid, reader: Guid) -> Option<SameHostState> {
        self.pairs.read().ok()?.get(&(writer, reader)).cloned()
    }

    /// Removes an entry (e.g. when the match is dissolved).
    pub fn remove(&self, writer: Guid, reader: Guid) {
        if let Ok(mut g) = self.pairs.write() {
            g.remove(&(writer, reader));
        }
    }

    /// Number of currently registered pairs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.read().map(|g| g.len()).unwrap_or(0)
    }

    /// `true` if the tracker is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Snapshot of all pairs. Mainly for debug/diagnostics.
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
        // (writer, reader) must differ from (reader, writer); otherwise
        // the Owner/Consumer roles are ambiguous.
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
        // Re-register must NOT reset the Bound state back to Pending.
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
