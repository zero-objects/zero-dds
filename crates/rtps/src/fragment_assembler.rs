// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fragment reassembly for DDSI-RTPS 2.5 §8.4.14 on the reader side.
//!
//! Keeps a `FragmentBuffer` per in-flight sample SN, into which DATA_FRAG
//! submessages are fed. Once all fragments are present, a
//! complete sample falls out, which the `ReliableReader` treats like a
//! regular DATA.
//!
//! # DoS posture
//!
//! The assembler must robustly process input from untrusted writers.
//! Three caps protect against pathological inputs:
//!
//! - `max_pending_sns`: maximum number of SNs simultaneously in progress.
//!   On overflow the oldest (smallest) incomplete SN is discarded.
//! - `max_sample_bytes`: upper bound for `sample_size`. DATA_FRAGs with
//!   `sample_size > cap` are discarded **without** allocation —
//!   protection against "I claim a 4 GB sample and hope you allocate".
//! - `max_fragment_size`: upper bound for `fragment_size` values from the
//!   writer. A typical MTU is < 1500; we accept up to 65535.
//!
//! Discarded fragments are counted in `drop_count` (diagnosis).

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;
use alloc::vec::Vec;

use crate::submessages::{DataFragSubmessage, FragmentNumberSet};
use crate::wire_types::{FragmentNumber, SequenceNumber};

/// Default cap for the number of simultaneously in-flight SNs.
pub const DEFAULT_MAX_PENDING_SNS: usize = 64;
/// Default cap for the maximum sample size (1 MiB). Larger samples
/// are not a use case in phase 1; DDS-Security/fragmentation on
/// large images waits for phase 2+.
pub const DEFAULT_MAX_SAMPLE_BYTES: usize = 1024 * 1024;
/// Default cap for `fragment_size` (u16 maximum per spec).
pub const DEFAULT_MAX_FRAGMENT_SIZE: u16 = u16::MAX;

/// A fully reassembled sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedSample {
    /// Writer sequence number.
    pub sequence_number: SequenceNumber,
    /// Reassembled payload (total sample bytes in original order).
    pub payload: Vec<u8>,
}

/// Configuration for the assembler.
#[derive(Debug, Clone, Copy)]
pub struct AssemblerCaps {
    /// Max. number of simultaneous SNs.
    pub max_pending_sns: usize,
    /// Max. sample_size in bytes.
    pub max_sample_bytes: usize,
    /// Max. fragment_size in bytes.
    pub max_fragment_size: u16,
}

impl Default for AssemblerCaps {
    /// Conservative defaults for typical DDS workloads (1 MiB samples,
    /// 64 simultaneous in-flight SNs, u16-max fragment size).
    fn default() -> Self {
        Self {
            max_pending_sns: DEFAULT_MAX_PENDING_SNS,
            max_sample_bytes: DEFAULT_MAX_SAMPLE_BYTES,
            max_fragment_size: DEFAULT_MAX_FRAGMENT_SIZE,
        }
    }
}

/// Per-SN ring buffer for incoming fragments.
#[derive(Debug, Clone)]
struct FragmentBuffer {
    sample_size: u32,
    fragment_size: u16,
    total_fragments: u32,
    received: BTreeSet<FragmentNumber>,
    data: Vec<u8>,
}

impl FragmentBuffer {
    fn new(sample_size: u32, fragment_size: u16) -> Self {
        let total = if fragment_size == 0 {
            0
        } else {
            sample_size.div_ceil(u32::from(fragment_size))
        };
        Self {
            sample_size,
            fragment_size,
            total_fragments: total,
            received: BTreeSet::new(),
            data: vec![0u8; sample_size as usize],
        }
    }

    fn is_complete(&self) -> bool {
        self.total_fragments > 0 && self.received.len() as u32 == self.total_fragments
    }

    fn missing(&self) -> FragmentNumberSet {
        if self.total_fragments == 0 {
            return FragmentNumberSet::from_missing(FragmentNumber(1), &[]);
        }
        let mut missing_nums = Vec::new();
        for f in 1..=self.total_fragments {
            let fnum = FragmentNumber(f);
            if !self.received.contains(&fnum) {
                missing_nums.push(fnum);
            }
        }
        let base = missing_nums
            .first()
            .copied()
            .unwrap_or(FragmentNumber(self.total_fragments.saturating_add(1)));
        FragmentNumberSet::from_missing(base, &missing_nums)
    }
}

/// Rejected-fragment category — for diagnostics only.
///
/// **Adding new variants**: when changing this, also adapt the
/// [`DropReason::as_str`] method (exhaustive match), otherwise
/// the build breaks. This is intentional — it prevents new
/// failure modes from being silently lost in logging/metrics paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DropReason {
    /// `sample_size` over the cap.
    SampleTooLarge,
    /// `fragment_size` over the cap or == 0.
    FragmentSizeInvalid,
    /// `fragment_starting_num == 0` (1-based expected).
    FragmentIndexZero,
    /// Fragment index beyond `total_fragments`.
    FragmentIndexOutOfRange,
    /// Payload length does not match the fragment position.
    PayloadSizeMismatch,
    /// A later DataFrag contradicts an already stored one
    /// (different `sample_size` or `fragment_size`).
    InconsistentWithBuffered,
    /// `fragments_in_submessage == 0` or inconsistent.
    FragmentsInSubmessageInvalid,
    /// The number of simultaneously managed SNs would exceed `max_pending_sns`
    /// — the oldest incomplete SN was discarded.
    PendingSnsCapExceeded,
    /// `max_pending_sns == 0` — the assembler accepts no entries.
    AssemblerDisabled,
}

impl DropReason {
    /// Stable string representation for logging/metrics. Exhaustive
    /// match — new variants intentionally break the build here.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SampleTooLarge => "sample_too_large",
            Self::FragmentSizeInvalid => "fragment_size_invalid",
            Self::FragmentIndexZero => "fragment_index_zero",
            Self::FragmentIndexOutOfRange => "fragment_index_out_of_range",
            Self::PayloadSizeMismatch => "payload_size_mismatch",
            Self::InconsistentWithBuffered => "inconsistent_with_buffered",
            Self::FragmentsInSubmessageInvalid => "fragments_in_submessage_invalid",
            Self::PendingSnsCapExceeded => "pending_sns_cap_exceeded",
            Self::AssemblerDisabled => "assembler_disabled",
        }
    }
}

impl core::fmt::Display for DropReason {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// State of a reassembler.
///
/// `FragmentAssembler::default()` returns an assembler with
/// [`AssemblerCaps::default`] — the only defaults path.
#[derive(Debug, Clone, Default)]
pub struct FragmentAssembler {
    buffers: BTreeMap<SequenceNumber, FragmentBuffer>,
    caps: AssemblerCaps,
    drop_count: u64,
    last_drop_reason: Option<DropReason>,
}

impl FragmentAssembler {
    /// Creates an assembler with the given caps.
    #[must_use]
    pub fn new(caps: AssemblerCaps) -> Self {
        Self {
            buffers: BTreeMap::new(),
            caps,
            drop_count: 0,
            last_drop_reason: None,
        }
    }

    /// Number of active SNs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Is the assembler empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    /// Number of discarded fragments since start (or since
    /// [`reset_diagnostics`](Self::reset_diagnostics)).
    #[must_use]
    pub fn drop_count(&self) -> u64 {
        self.drop_count
    }

    /// The reason for the most recently discarded fragment, if any
    /// was discarded at all. For debugging/metrics — not for
    /// control-flow decisions.
    #[must_use]
    pub fn last_drop_reason(&self) -> Option<DropReason> {
        self.last_drop_reason
    }

    /// Resets the diagnostic counters to 0. The `buffers` stay
    /// unchanged — this is pure metric hygiene (long-running readers
    /// want their delta snapshots).
    pub fn reset_diagnostics(&mut self) {
        self.drop_count = 0;
        self.last_drop_reason = None;
    }

    /// True if fragments are missing for at least one SN.
    #[must_use]
    pub fn has_gaps(&self) -> bool {
        self.buffers.values().any(|b| !b.is_complete())
    }

    /// Iterates SNs for which fragment gaps currently exist.
    pub fn incomplete_sns(&self) -> impl Iterator<Item = SequenceNumber> + '_ {
        self.buffers
            .iter()
            .filter(|(_, b)| !b.is_complete())
            .map(|(sn, _)| *sn)
    }

    /// Missing fragments for an SN. Returns an empty set if the SN is
    /// unknown or already complete.
    #[must_use]
    pub fn missing_fragments(&self, sn: SequenceNumber) -> FragmentNumberSet {
        match self.buffers.get(&sn) {
            Some(b) => b.missing(),
            None => FragmentNumberSet::from_missing(FragmentNumber(1), &[]),
        }
    }

    /// Removes the buffer for this SN (e.g. on a GAP signal or after
    /// completion). Returns whether the buffer was present.
    pub fn discard(&mut self, sn: SequenceNumber) -> bool {
        self.buffers.remove(&sn).is_some()
    }

    /// Feeds in a DATA_FRAG. Returns the reassembled sample on
    /// completion.
    ///
    /// Inconsistent or pathological fragments are discarded and
    /// counted in `drop_count` — they cannot make the internal map
    /// grow beyond the caps.
    pub fn insert(&mut self, df: &DataFragSubmessage) -> Option<CompletedSample> {
        // --- Input validation (no-alloc gate) ----------------------
        if df.fragment_size == 0 || df.fragment_size > self.caps.max_fragment_size {
            self.record_drop(DropReason::FragmentSizeInvalid);
            return None;
        }
        if df.fragments_in_submessage == 0 {
            self.record_drop(DropReason::FragmentsInSubmessageInvalid);
            return None;
        }
        if df.sample_size as usize > self.caps.max_sample_bytes {
            self.record_drop(DropReason::SampleTooLarge);
            return None;
        }
        if df.fragment_starting_num.0 == 0 {
            self.record_drop(DropReason::FragmentIndexZero);
            return None;
        }

        // Pre-computations
        let total_fragments = df.sample_size.div_ceil(u32::from(df.fragment_size));
        let last_frag = df
            .fragment_starting_num
            .0
            .checked_add(u32::from(df.fragments_in_submessage) - 1)
            .unwrap_or(u32::MAX);
        if last_frag > total_fragments {
            self.record_drop(DropReason::FragmentIndexOutOfRange);
            return None;
        }

        // Cap: number of simultaneously in-flight SNs.
        if !self.buffers.contains_key(&df.writer_sn)
            && self.buffers.len() >= self.caps.max_pending_sns
        {
            // Discard the oldest SN — DoS protection. The affected sample
            // is gone; the reader must treat this like a GAP state.
            let Some(&oldest) = self.buffers.keys().next() else {
                // Cap == 0: nobody may enter.
                self.record_drop(DropReason::AssemblerDisabled);
                return None;
            };
            self.buffers.remove(&oldest);
            self.record_drop(DropReason::PendingSnsCapExceeded);
        }

        // Create the buffer or extend it consistently.
        let buffer = match self.buffers.get_mut(&df.writer_sn) {
            Some(existing) => {
                if existing.sample_size != df.sample_size
                    || existing.fragment_size != df.fragment_size
                {
                    self.record_drop(DropReason::InconsistentWithBuffered);
                    return None;
                }
                existing
            }
            None => {
                self.buffers.insert(
                    df.writer_sn,
                    FragmentBuffer::new(df.sample_size, df.fragment_size),
                );
                self.buffers.get_mut(&df.writer_sn)?
            }
        };

        // Write the fragment bytes to the correct position.
        let frag_size_usize = buffer.fragment_size as usize;
        let frag_count = df.fragments_in_submessage as usize;
        let first_idx = (df.fragment_starting_num.0 - 1) as usize;
        let byte_start = first_idx * frag_size_usize;
        let expected_last_frag = core::cmp::min(last_frag, buffer.total_fragments);
        // Expected payload length: frag_count-1 full fragments + possibly a
        // shortened last fragment (if last_frag == total_fragments).
        let full_portion = (frag_count - 1) * frag_size_usize;
        let tail_size = if expected_last_frag == buffer.total_fragments {
            // The last fragment of the sample may be shorter.
            buffer.sample_size as usize - ((buffer.total_fragments - 1) as usize) * frag_size_usize
        } else {
            frag_size_usize
        };
        let expected_len = full_portion + tail_size;
        // Cross-vendor: cyclone/FastDDS pad the LAST fragment of a
        // sample to alignment (e.g. 72 instead of 71 bytes). The overhang is
        // padding that does not belong to the sample (`sample_size`). We tolerate
        // a longer last fragment and copy only `expected_len`
        // sample bytes; non-last fragments stay exact (strict cap).
        let is_last = expected_last_frag == buffer.total_fragments;
        let too_short = df.serialized_payload.len() < expected_len;
        let non_last_mismatch = !is_last && df.serialized_payload.len() != expected_len;
        if too_short || non_last_mismatch {
            self.record_drop(DropReason::PayloadSizeMismatch);
            return None;
        }

        // Write — only the `expected_len` sample bytes (trailing padding of
        // the last fragment is discarded).
        let data_end = byte_start + expected_len;
        if data_end > buffer.data.len() {
            self.record_drop(DropReason::PayloadSizeMismatch);
            return None;
        }
        buffer.data[byte_start..data_end].copy_from_slice(&df.serialized_payload[..expected_len]);
        for f in 0..df.fragments_in_submessage as u32 {
            buffer
                .received
                .insert(FragmentNumber(df.fragment_starting_num.0 + f));
        }

        if buffer.is_complete() {
            // Take the buffer and return a CompletedSample.
            let buf = self.buffers.remove(&df.writer_sn)?;
            return Some(CompletedSample {
                sequence_number: df.writer_sn,
                payload: buf.data,
            });
        }
        None
    }

    fn record_drop(&mut self, reason: DropReason) {
        self.drop_count = self.drop_count.saturating_add(1);
        self.last_drop_reason = Some(reason);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::wire_types::EntityId;

    fn wid() -> EntityId {
        EntityId::user_writer_with_key([0x10, 0x20, 0x30])
    }
    fn rid() -> EntityId {
        EntityId::user_reader_with_key([0x40, 0x50, 0x60])
    }

    fn df(
        sn: i64,
        starting: u32,
        count: u16,
        frag_size: u16,
        sample_size: u32,
        payload: Vec<u8>,
    ) -> DataFragSubmessage {
        DataFragSubmessage {
            extra_flags: 0,
            reader_id: rid(),
            writer_id: wid(),
            writer_sn: SequenceNumber(sn),
            fragment_starting_num: FragmentNumber(starting),
            fragments_in_submessage: count,
            fragment_size: frag_size,
            sample_size,
            serialized_payload: alloc::sync::Arc::from(payload),
            inline_qos_flag: false,
            hash_key_flag: false,
            key_flag: false,
            non_standard_flag: false,
        }
    }

    #[test]
    fn single_fragment_sample_completes_immediately() {
        let mut a = FragmentAssembler::default();
        // sample_size=4, frag_size=4 → 1 fragment
        let res = a.insert(&df(1, 1, 1, 4, 4, vec![1, 2, 3, 4]));
        assert!(res.is_some());
        let s = res.unwrap();
        assert_eq!(s.sequence_number, SequenceNumber(1));
        assert_eq!(s.payload, vec![1, 2, 3, 4]);
        assert_eq!(a.len(), 0);
    }

    #[test]
    fn two_fragments_complete_in_order() {
        let mut a = FragmentAssembler::default();
        assert!(a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4])).is_none());
        let res = a.insert(&df(1, 2, 1, 4, 8, vec![5, 6, 7, 8])).unwrap();
        assert_eq!(res.payload, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn fragments_complete_out_of_order() {
        let mut a = FragmentAssembler::default();
        // 2 first, then 1, then 3
        assert!(a.insert(&df(1, 2, 1, 4, 10, vec![5, 6, 7, 8])).is_none());
        assert!(a.insert(&df(1, 1, 1, 4, 10, vec![1, 2, 3, 4])).is_none());
        let res = a.insert(&df(1, 3, 1, 4, 10, vec![9, 10])).unwrap();
        assert_eq!(res.payload, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn last_fragment_shorter_than_fragment_size() {
        let mut a = FragmentAssembler::default();
        assert!(a.insert(&df(1, 1, 1, 4, 10, vec![1, 2, 3, 4])).is_none());
        assert!(a.insert(&df(1, 2, 1, 4, 10, vec![5, 6, 7, 8])).is_none());
        let res = a.insert(&df(1, 3, 1, 4, 10, vec![9, 10])).unwrap();
        assert_eq!(res.payload.len(), 10);
    }

    #[test]
    fn duplicate_fragment_is_idempotent() {
        let mut a = FragmentAssembler::default();
        assert!(a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4])).is_none());
        assert!(a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4])).is_none());
        assert_eq!(a.missing_fragments(SequenceNumber(1)).num_bits, 1);
    }

    #[test]
    fn missing_fragments_enumerates_gaps() {
        let mut a = FragmentAssembler::default();
        // Fragment 2 is missing
        assert!(a.insert(&df(1, 1, 1, 4, 10, vec![1, 2, 3, 4])).is_none());
        assert!(a.insert(&df(1, 3, 1, 4, 10, vec![9, 10])).is_none());
        let ms = a.missing_fragments(SequenceNumber(1));
        let collected: Vec<_> = ms.iter_set().collect();
        assert_eq!(collected, vec![FragmentNumber(2)]);
    }

    #[test]
    fn inconsistent_sample_size_drops_fragment() {
        let mut a = FragmentAssembler::default();
        assert!(a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4])).is_none());
        // Second fragment reports sample_size=12 instead of 8 → discarded
        let res = a.insert(&df(1, 2, 1, 4, 12, vec![5, 6, 7, 8]));
        assert!(res.is_none());
        assert_eq!(a.drop_count(), 1);
        // sn=1 is still in-flight with the old sample_size=8
        assert_eq!(a.missing_fragments(SequenceNumber(1)).num_bits, 1);
    }

    #[test]
    fn sample_too_large_drops_without_alloc() {
        let caps = AssemblerCaps {
            max_sample_bytes: 16,
            ..AssemblerCaps::default()
        };
        let mut a = FragmentAssembler::new(caps);
        // sample_size=100 > cap=16 → discarded
        assert!(a.insert(&df(1, 1, 1, 4, 100, vec![1, 2, 3, 4])).is_none());
        assert!(a.is_empty());
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn fragment_size_zero_dropped() {
        let mut a = FragmentAssembler::default();
        // frag_size=0 → avoid div by 0
        assert!(a.insert(&df(1, 1, 1, 0, 4, vec![1, 2, 3, 4])).is_none());
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn fragment_index_zero_dropped() {
        let mut a = FragmentAssembler::default();
        assert!(a.insert(&df(1, 0, 1, 4, 4, vec![1, 2, 3, 4])).is_none());
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn fragment_index_out_of_range_dropped() {
        let mut a = FragmentAssembler::default();
        // sample_size=4, frag_size=4 → total=1, but index 2 requested
        assert!(a.insert(&df(1, 2, 1, 4, 4, vec![0])).is_none());
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn payload_size_mismatch_dropped() {
        let mut a = FragmentAssembler::default();
        // frag_size=4 but payload is only 2 bytes → mismatch
        assert!(a.insert(&df(1, 1, 1, 4, 8, vec![1, 2])).is_none());
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn max_pending_sns_evicts_oldest() {
        let caps = AssemblerCaps {
            max_pending_sns: 2,
            ..AssemblerCaps::default()
        };
        let mut a = FragmentAssembler::new(caps);
        // SN 1, 2 open (only fragment 1 of 2 received each)
        a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        a.insert(&df(2, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert_eq!(a.len(), 2);
        // SN 3 pushes SN 1 out
        a.insert(&df(3, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert_eq!(a.len(), 2);
        assert!(a.buffers.contains_key(&SequenceNumber(2)));
        assert!(a.buffers.contains_key(&SequenceNumber(3)));
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn has_gaps_flips_to_false_after_completion() {
        let mut a = FragmentAssembler::default();
        a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert!(a.has_gaps());
        a.insert(&df(1, 2, 1, 4, 8, vec![5, 6, 7, 8]));
        assert!(!a.has_gaps());
    }

    #[test]
    fn incomplete_sns_enumerates_in_order() {
        let mut a = FragmentAssembler::default();
        a.insert(&df(5, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        a.insert(&df(2, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        let sns: Vec<_> = a.incomplete_sns().collect();
        assert_eq!(sns, vec![SequenceNumber(2), SequenceNumber(5)]);
    }

    #[test]
    fn discard_removes_buffer() {
        let mut a = FragmentAssembler::default();
        a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert!(a.discard(SequenceNumber(1)));
        assert!(a.is_empty());
        assert!(!a.discard(SequenceNumber(1)));
    }

    #[test]
    fn missing_for_unknown_sn_is_empty() {
        let a = FragmentAssembler::default();
        assert_eq!(a.missing_fragments(SequenceNumber(42)).num_bits, 0);
    }

    // ---- fragments_in_submessage > 1 (bundle decode) ----

    #[test]
    fn bundled_fragments_all_full() {
        // 3 fragments in one submessage, all full (no tail).
        // sample_size=18, frag_size=4, total=5. We bundle fragments 1-3.
        let mut a = FragmentAssembler::default();
        let payload = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let res = a.insert(&df(1, 1, 3, 4, 18, payload.clone()));
        assert!(res.is_none(), "not yet complete");
        // Fragments 4, 5 are still missing
        let ms: Vec<_> = a.missing_fragments(SequenceNumber(1)).iter_set().collect();
        assert_eq!(ms, vec![FragmentNumber(4), FragmentNumber(5)]);
    }

    #[test]
    fn bundled_fragments_including_last_with_tail() {
        // 2 fragments in one submessage, incl. the last (tail shortened).
        // sample_size=10, frag_size=4, total=3. Bundle: fragments 2-3.
        let mut a = FragmentAssembler::default();
        // First present fragment 1
        assert!(
            a.insert(&df(1, 1, 1, 4, 10, vec![0xA, 0xB, 0xC, 0xD]))
                .is_none()
        );
        // Now bundle 2+3 (4 + 2 bytes = 6)
        let bundle = vec![5, 6, 7, 8, 9, 10];
        let res = a.insert(&df(1, 2, 2, 4, 10, bundle));
        assert!(res.is_some());
        let s = res.unwrap();
        assert_eq!(s.payload, vec![0xA, 0xB, 0xC, 0xD, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn bundled_fragments_payload_size_mismatch_rejected() {
        // Bundle with a claimed 3 fragments of 4 bytes = 12, but
        // only 10 bytes delivered.
        let mut a = FragmentAssembler::default();
        let payload = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert!(a.insert(&df(1, 1, 3, 4, 20, payload)).is_none());
        assert_eq!(a.drop_count(), 1);
        assert_eq!(a.last_drop_reason(), Some(DropReason::PayloadSizeMismatch));
    }

    // ---- last_drop_reason diagnosis ----

    #[test]
    fn last_drop_reason_tracks_most_recent() {
        let mut a = FragmentAssembler::default();
        assert_eq!(a.last_drop_reason(), None);
        a.insert(&df(1, 0, 1, 4, 4, vec![1, 2, 3, 4]));
        assert_eq!(a.last_drop_reason(), Some(DropReason::FragmentIndexZero));
        a.insert(&df(1, 1, 1, 0, 4, vec![1, 2, 3, 4]));
        assert_eq!(a.last_drop_reason(), Some(DropReason::FragmentSizeInvalid));
    }

    #[test]
    fn pending_sns_cap_exceeded_uses_dedicated_reason() {
        let caps = AssemblerCaps {
            max_pending_sns: 1,
            ..AssemblerCaps::default()
        };
        let mut a = FragmentAssembler::new(caps);
        a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        a.insert(&df(2, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert_eq!(
            a.last_drop_reason(),
            Some(DropReason::PendingSnsCapExceeded)
        );
    }

    #[test]
    fn default_assembler_uses_default_caps() {
        // B2 regression: the Default trait must return a working
        // assembler (not just zero state, but correct caps).
        let mut a = FragmentAssembler::default();
        assert!(a.is_empty());
        // Typical case: 1-fragment sample complete
        let res = a.insert(&df(1, 1, 1, 4, 4, vec![1, 2, 3, 4]));
        assert!(res.is_some());
    }

    #[test]
    fn reset_diagnostics_clears_counters_but_keeps_buffers() {
        // B8 regression: reset_diagnostics should only zero the metric
        // state, in-flight buffers are kept.
        let mut a = FragmentAssembler::default();
        a.insert(&df(1, 0, 1, 4, 4, vec![1, 2, 3, 4])); // FragmentIndexZero → drop
        a.insert(&df(2, 1, 1, 4, 8, vec![1, 2, 3, 4])); // partial buffer
        assert_eq!(a.drop_count(), 1);
        assert_eq!(a.len(), 1);
        a.reset_diagnostics();
        assert_eq!(a.drop_count(), 0);
        assert!(a.last_drop_reason().is_none());
        assert_eq!(a.len(), 1, "buffers must stay intact");
    }

    #[test]
    fn max_pending_sns_zero_rejects_with_assembler_disabled() {
        let caps = AssemblerCaps {
            max_pending_sns: 0,
            ..AssemblerCaps::default()
        };
        let mut a = FragmentAssembler::new(caps);
        a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert_eq!(a.last_drop_reason(), Some(DropReason::AssemblerDisabled));
        assert!(a.is_empty());
    }
}
