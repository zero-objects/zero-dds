// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE FRAGMENT-Reassembler (Spec §8.4.13).
//!
//! Spec §8.3.5.14 allows FRAGMENT submessages only in reliable streams.
//! Each fragment carries a body slice of the split original
//! submessage and a `LAST` flag (see `crate::submessages::fragment`).
//!
//! The reassembler is tick-free: fragments are fed in via `add_fragment`;
//! as soon as `LAST` is received and all byte ranges are gap-free,
//! `Some(Vec<u8>)` is returned. Until then the
//! reassembler keeps a single pre-allocation per `(stream, base_seq)`.
//!
//! ## DoS caps
//! - `MAX_FRAGMENTS_PER_STREAM = 256`: maximum number of fragments per
//!   in-flight reassembly. 256 covers 256·1500 ≈ 384 KiB — more than
//!   enough for the realistic use case.
//! - `MAX_TOTAL_PAYLOAD = 1 MiB`: hard cap for the reassembled
//!   payload. Also used as the pre-allocation limit (no
//!   "claimed 4 GiB" → OOM).
//! - `MAX_PENDING_STREAMS = 32`: simultaneously allowed running
//!   reassemblies. Protects against "1000 streams of 1 fragment".
//! - GC: each fragment has an `arrival` tick; old reassemblies
//!   are discarded via `gc(now)` after `gc_ttl`.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::time::Duration;

use crate::error::XrceError;

/// DoS cap: maximum fragments per in-flight reassembly.
pub const MAX_FRAGMENTS_PER_STREAM: usize = 256;
/// DoS cap: maximum total size of a reassembled submessage.
pub const MAX_TOTAL_PAYLOAD: usize = 1 << 20; // 1 MiB
/// DoS cap: maximum number of simultaneously running reassemblies.
pub const MAX_PENDING_STREAMS: usize = 32;
/// Default TTL for expired reassemblies (10 s).
pub const DEFAULT_GC_TTL: Duration = Duration::from_secs(10);

/// Key of an in-flight reassembly: `(stream_id, base_seqnr)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssemblerKey {
    /// Reliable stream id.
    pub stream_id: u8,
    /// Base sequence number of the first fragment submessage.
    pub base_seq: u16,
}

#[derive(Debug, Clone)]
struct PendingAssembly {
    /// Ordered map `offset → bytes`. As soon as `last_offset_end`
    /// is set and all slots are contiguous, the
    /// reassembly is done.
    fragments: BTreeMap<u32, Vec<u8>>,
    /// When `LAST` is received, the end offset (offset + len) of the
    /// last fragment. Marks the total length.
    final_size: Option<u32>,
    /// Tick of the last input — for GC.
    last_arrival: Duration,
}

impl PendingAssembly {
    fn new(now: Duration) -> Self {
        Self {
            fragments: BTreeMap::new(),
            final_size: None,
            last_arrival: now,
        }
    }

    fn fragment_count(&self) -> usize {
        self.fragments.len()
    }

    fn current_total(&self) -> usize {
        self.fragments.values().map(Vec::len).sum()
    }

    /// Is the reassembly gap-free and does it have the last fragment?
    fn is_complete(&self) -> bool {
        let Some(target) = self.final_size else {
            return false;
        };
        let mut cursor: u32 = 0;
        for (&offset, bytes) in &self.fragments {
            if offset != cursor {
                return false;
            }
            cursor = cursor.saturating_add(bytes.len() as u32);
        }
        cursor == target
    }

    fn assemble(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.final_size.unwrap_or(0) as usize);
        for (_, frag) in self.fragments {
            out.extend_from_slice(&frag);
        }
        out
    }
}

/// FRAGMENT reassembler over all reliable streams.
#[derive(Debug, Clone, Default)]
pub struct FragmentAssembler {
    pending: BTreeMap<AssemblerKey, PendingAssembly>,
    gc_ttl: Duration,
    drop_count: u64,
}

impl FragmentAssembler {
    /// Constructor with the default TTL.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: BTreeMap::new(),
            gc_ttl: DEFAULT_GC_TTL,
            drop_count: 0,
        }
    }

    /// With explicitly set GC TTL.
    #[must_use]
    pub fn with_gc_ttl(ttl: Duration) -> Self {
        Self {
            pending: BTreeMap::new(),
            gc_ttl: ttl,
            drop_count: 0,
        }
    }

    /// Number of concurrently running reassemblies.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Number of fragments discarded in total (DoS diagnostics).
    #[must_use]
    pub fn drop_count(&self) -> u64 {
        self.drop_count
    }

    /// Feeds in a fragment. `offset` is the byte offset of the
    /// fragment within the split original submessage.
    /// Returns `Some(payload)` when the reassembly becomes complete with
    /// this call; `None` otherwise.
    ///
    /// # Errors
    /// - `PayloadTooLarge` if the reassembly would exceed
    ///   `MAX_TOTAL_PAYLOAD`.
    /// - `ValueOutOfRange` if a cap is reached
    ///   (`MAX_FRAGMENTS_PER_STREAM`, `MAX_PENDING_STREAMS`) or a
    ///   fragment overlap occurs.
    pub fn add_fragment(
        &mut self,
        key: AssemblerKey,
        offset: u32,
        last_flag: bool,
        bytes: Vec<u8>,
        now: Duration,
    ) -> Result<Option<Vec<u8>>, XrceError> {
        let new_end = offset
            .checked_add(bytes.len() as u32)
            .ok_or(XrceError::ValueOutOfRange {
                message: "fragment offset+len overflow",
            })?;
        if (new_end as usize) > MAX_TOTAL_PAYLOAD {
            self.drop_count = self.drop_count.saturating_add(1);
            return Err(XrceError::PayloadTooLarge {
                limit: MAX_TOTAL_PAYLOAD,
                actual: new_end as usize,
            });
        }

        // Create a new pending bucket, possibly with a stream cap check.
        let entry = match self.pending.get_mut(&key) {
            Some(e) => e,
            None => {
                if self.pending.len() >= MAX_PENDING_STREAMS {
                    self.drop_count = self.drop_count.saturating_add(1);
                    return Err(XrceError::ValueOutOfRange {
                        message: "fragment assembler max-pending-streams reached",
                    });
                }
                self.pending
                    .entry(key)
                    .or_insert_with(|| PendingAssembly::new(now))
            }
        };
        entry.last_arrival = now;

        if entry.fragment_count() >= MAX_FRAGMENTS_PER_STREAM {
            self.drop_count = self.drop_count.saturating_add(1);
            return Err(XrceError::ValueOutOfRange {
                message: "fragment assembler max-fragments-per-stream reached",
            });
        }

        // Overlap check (no equal offset, no overlap).
        if entry.fragments.contains_key(&offset) {
            // duplicates dropped
            self.drop_count = self.drop_count.saturating_add(1);
            return Ok(None);
        }
        // Check whether `[offset, new_end)` overlaps with existing ranges:
        // get the largest key <= offset and the smallest key > offset.
        let prev = entry.fragments.range(..=offset).next_back();
        if let Some((&po, pb)) = prev {
            let pe = po + pb.len() as u32;
            if pe > offset {
                self.drop_count = self.drop_count.saturating_add(1);
                return Err(XrceError::ValueOutOfRange {
                    message: "overlapping fragment",
                });
            }
        }
        let next = entry.fragments.range(offset..).next();
        if let Some((&no, _)) = next {
            if new_end > no {
                self.drop_count = self.drop_count.saturating_add(1);
                return Err(XrceError::ValueOutOfRange {
                    message: "overlapping fragment",
                });
            }
        }

        // Total cap after insert?
        if entry.current_total() + bytes.len() > MAX_TOTAL_PAYLOAD {
            self.drop_count = self.drop_count.saturating_add(1);
            return Err(XrceError::PayloadTooLarge {
                limit: MAX_TOTAL_PAYLOAD,
                actual: entry.current_total() + bytes.len(),
            });
        }

        entry.fragments.insert(offset, bytes);
        if last_flag {
            entry.final_size = Some(new_end);
        }

        let complete = entry.is_complete();
        if complete {
            if let Some(done) = self.pending.remove(&key) {
                return Ok(Some(done.assemble()));
            }
        }
        Ok(None)
    }

    /// GC: discards all reassemblies whose last fragment is older than
    /// `gc_ttl`. Returns the number of discarded buckets.
    pub fn gc(&mut self, now: Duration) -> usize {
        let cutoff = now.saturating_sub(self.gc_ttl);
        let to_drop: Vec<AssemblerKey> = self
            .pending
            .iter()
            .filter(|(_, p)| p.last_arrival < cutoff)
            .map(|(k, _)| *k)
            .collect();
        let n = to_drop.len();
        for k in to_drop {
            self.pending.remove(&k);
            self.drop_count = self.drop_count.saturating_add(1);
        }
        n
    }

    /// Resets all pending buckets (e.g. on `RESET`).
    pub fn reset(&mut self) {
        let n = self.pending.len() as u64;
        self.pending.clear();
        self.drop_count = self.drop_count.saturating_add(n);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    fn k() -> AssemblerKey {
        AssemblerKey {
            stream_id: 0x80,
            base_seq: 0,
        }
    }

    #[test]
    fn happy_path_two_fragments() {
        let mut a = FragmentAssembler::new();
        let r = a
            .add_fragment(k(), 0, false, vec![1, 2, 3, 4], Duration::ZERO)
            .unwrap();
        assert!(r.is_none());
        let r = a
            .add_fragment(k(), 4, true, vec![5, 6, 7, 8], Duration::ZERO)
            .unwrap();
        assert_eq!(r.unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(a.pending_count(), 0);
    }

    #[test]
    fn out_of_order_reassembly() {
        let mut a = FragmentAssembler::new();
        a.add_fragment(k(), 4, true, vec![5, 6, 7, 8], Duration::ZERO)
            .unwrap();
        let r = a
            .add_fragment(k(), 0, false, vec![1, 2, 3, 4], Duration::ZERO)
            .unwrap();
        assert_eq!(r.unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn missing_middle_fragment_does_not_complete() {
        let mut a = FragmentAssembler::new();
        a.add_fragment(k(), 0, false, vec![1, 2], Duration::ZERO)
            .unwrap();
        let r = a
            .add_fragment(k(), 4, true, vec![5, 6], Duration::ZERO)
            .unwrap();
        assert!(r.is_none());
        assert_eq!(a.pending_count(), 1);
    }

    #[test]
    fn duplicate_offset_dropped() {
        let mut a = FragmentAssembler::new();
        a.add_fragment(k(), 0, false, vec![1, 2], Duration::ZERO)
            .unwrap();
        let before = a.drop_count();
        let r = a
            .add_fragment(k(), 0, false, vec![9, 9], Duration::ZERO)
            .unwrap();
        assert!(r.is_none());
        assert_eq!(a.drop_count(), before + 1);
    }

    #[test]
    fn overlapping_fragments_rejected() {
        let mut a = FragmentAssembler::new();
        a.add_fragment(k(), 0, false, vec![1, 2, 3, 4], Duration::ZERO)
            .unwrap();
        let res = a.add_fragment(k(), 2, false, vec![9, 9], Duration::ZERO);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn dos_cap_max_total_payload() {
        let mut a = FragmentAssembler::new();
        let res = a.add_fragment(
            k(),
            0,
            true,
            vec![0u8; MAX_TOTAL_PAYLOAD + 1],
            Duration::ZERO,
        );
        assert!(matches!(res, Err(XrceError::PayloadTooLarge { .. })));
    }

    #[test]
    fn dos_cap_max_pending_streams() {
        let mut a = FragmentAssembler::new();
        for i in 0..MAX_PENDING_STREAMS as u16 {
            let key = AssemblerKey {
                stream_id: 0x80,
                base_seq: i,
            };
            a.add_fragment(key, 0, false, vec![0u8; 4], Duration::ZERO)
                .unwrap();
        }
        let key = AssemblerKey {
            stream_id: 0x80,
            base_seq: 999,
        };
        let res = a.add_fragment(key, 0, false, vec![0u8; 4], Duration::ZERO);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn dos_cap_max_fragments_per_stream() {
        let mut a = FragmentAssembler::new();
        for i in 0..MAX_FRAGMENTS_PER_STREAM as u32 {
            a.add_fragment(k(), i * 4, false, vec![0u8; 4], Duration::ZERO)
                .unwrap();
        }
        let res = a.add_fragment(
            k(),
            (MAX_FRAGMENTS_PER_STREAM as u32) * 4,
            true,
            vec![0u8; 4],
            Duration::ZERO,
        );
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn gc_drops_stale_assemblies() {
        let mut a = FragmentAssembler::with_gc_ttl(Duration::from_secs(5));
        a.add_fragment(k(), 0, false, vec![1, 2], Duration::ZERO)
            .unwrap();
        assert_eq!(a.pending_count(), 1);
        // 4s later → not yet expired
        assert_eq!(a.gc(Duration::from_secs(4)), 0);
        assert_eq!(a.pending_count(), 1);
        // 11s later → expired
        assert_eq!(a.gc(Duration::from_secs(11)), 1);
        assert_eq!(a.pending_count(), 0);
    }

    #[test]
    fn reset_clears_all() {
        let mut a = FragmentAssembler::new();
        a.add_fragment(k(), 0, false, vec![1], Duration::ZERO)
            .unwrap();
        a.reset();
        assert_eq!(a.pending_count(), 0);
        assert!(a.drop_count() >= 1);
    }
}
