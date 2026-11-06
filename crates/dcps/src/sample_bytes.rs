// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `SampleBytes` — zero-copy byte container for reader-path samples.
//!
//! Spec: `docs/specs/zerodds-zero-copy-1.0.md` §6 Wave 2.
//!
//! # Background
//!
//! `UserSample::Alive::payload` was historically a `Vec<u8>`. That forced
//! `strip_user_encap` to cut off the encap-header offset via
//! `payload[off..].to_vec()` — one heap alloc + copy per alive sample.
//!
//! `SampleBytes` replaces that Vec with an `Arc<[u8]>` + range. Strip
//! operations become pure index arithmetic (`Arc::clone` is a refcount
//! bump, not a copy). Heap allocs are eliminated on the hot path.
//!
//! # API
//!
//! - [`SampleBytes::from_vec`] — heap-Vec input (backward compat).
//! - [`SampleBytes::from_arc_slice`] — Arc + range (zero-copy path).
//! - [`SampleBytes::as_slice`] — slice read without copy.
//! - [`SampleBytes::to_vec`] — materialization at an FFI boundary.
//! - [`SampleBytes::len`] / `is_empty` — standard container API.
//! - `Clone` is O(1) — just an Arc refcount bump.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;
use zerodds_rtps::payload::Payload;

/// What the RTPS DATA submessage of a received sample says about where the
/// sample came from, beyond its payload: the identity of the sample on the
/// wire and the sample it relates to.
///
/// DDS-RPC 1.0 §7.8.2 identifies a request by the writer GUID and sequence
/// number of its DATA and a reply by `PID_RELATED_SAMPLE_IDENTITY`; neither
/// is a member of `SampleInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleWire {
    /// GUID of the writer that sent the sample.
    pub writer_guid: [u8; 16],
    /// The RTPS sequence number of the sample, `-1` where the delivery path
    /// has none.
    pub sequence_number: i64,
    /// `PID_RELATED_SAMPLE_IDENTITY` (0x0083) of the DATA, or eProsima's
    /// 0x800f from a writer of that vendor; `None` when the writer sent
    /// neither.
    pub related_sample_identity: Option<zerodds_rtps::inline_qos::SampleIdentityBytes>,
    /// The group sequence number, writer set and group coherent set of a
    /// sample from a writer of a GROUP-scope publisher (DDSI-RTPS 2.5
    /// §8.7.5, §8.7.6); `None` from any other writer. DDS 1.4 §2.2.2.5.2.10
    /// orders the readers `get_datareaders` returns by it.
    pub group: Option<zerodds_rtps::writer_group::ChangeGroupInfo>,
}

/// Refcounted byte container for reader-path samples.
///
/// Allows zero-copy slicing of the wire bytes by tracking a range over
/// an `Arc<[u8]>`. Clone is O(1).
#[derive(Debug, Clone)]
pub struct SampleBytes {
    /// The octets, a range of the buffer they arrived in — typically
    /// RTPS `DeliveredSample::payload`. Strip operations only narrow it.
    payload: Payload,
    /// Where the sample came from, if the delivery path knows.
    wire: Option<SampleWire>,
}

impl SampleBytes {
    /// Constructs from an owned `Vec<u8>`. Heap alloc for the Arc wrap.
    /// Use for test sample injection or when the bytes were freshly
    /// allocated anyway.
    #[must_use]
    pub fn from_vec(v: Vec<u8>) -> Self {
        Self::from_payload(Payload::from(v))
    }

    /// Constructs from the octets of `payload`, sharing its buffer.
    /// **Zero-copy**.
    #[must_use]
    pub fn from_payload(payload: Payload) -> Self {
        Self {
            payload,
            wire: None,
        }
    }

    /// Constructs from an `Arc<[u8]>` with the full range. **Zero-copy** —
    /// only the refcount is incremented.
    #[must_use]
    pub fn from_arc(data: Arc<[u8]>) -> Self {
        Self::from_payload(Payload::whole(data))
    }

    /// Constructs with an explicit range over an `Arc<[u8]>`. **Zero-copy**.
    ///
    /// # Panics
    /// If `range.end > data.len()` or `range.start > range.end`.
    #[must_use]
    pub fn from_arc_slice(data: Arc<[u8]>, range: Range<usize>) -> Self {
        assert!(
            range.end <= data.len() && range.start <= range.end,
            "SampleBytes range out of bounds"
        );
        Self::from_payload(Payload::whole(data).sub(range))
    }

    /// Creates a new `SampleBytes` with the given sub-range relative to
    /// the current view. **Zero-copy** — refcount bump.
    ///
    /// # Panics
    /// If `sub.end > self.len()`.
    #[must_use]
    pub fn slice(&self, sub: Range<usize>) -> Self {
        assert!(sub.end <= self.len(), "SampleBytes::slice out of bounds");
        Self {
            payload: self.payload.sub(sub),
            wire: self.wire,
        }
    }

    /// The same bytes, carrying where they came from.
    #[must_use]
    pub fn with_wire(mut self, wire: SampleWire) -> Self {
        self.wire = Some(wire);
        self
    }

    /// Where the sample came from, if the delivery path recorded it.
    #[must_use]
    pub fn wire(&self) -> Option<&SampleWire> {
        self.wire.as_ref()
    }

    /// Current view as `&[u8]`. No copy.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.payload
    }

    /// The octets as a [`Payload`], sharing their buffer.
    #[must_use]
    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    /// Number of bytes in the current view.
    #[must_use]
    pub fn len(&self) -> usize {
        self.payload.len()
    }

    /// `true` if the view is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }

    /// Materializes the view into a `Vec<u8>`. **Copies** — use only at
    /// FFI boundaries where owned data must be handed to C/Python/JS.
    #[must_use]
    pub fn to_vec(&self) -> Vec<u8> {
        self.as_slice().to_vec()
    }
}

impl AsRef<[u8]> for SampleBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl core::ops::Deref for SampleBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl PartialEq for SampleBytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for SampleBytes {}

impl From<Vec<u8>> for SampleBytes {
    fn from(v: Vec<u8>) -> Self {
        Self::from_vec(v)
    }
}

impl From<Arc<[u8]>> for SampleBytes {
    fn from(a: Arc<[u8]>) -> Self {
        Self::from_arc(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_is_zero_copy() {
        let arc: Arc<[u8]> = Arc::from(alloc::vec![1, 2, 3, 4, 5].into_boxed_slice());
        let s = SampleBytes::from_arc(arc);
        let inner_ptr_before = s.as_slice().as_ptr() as usize;
        let sub = s.slice(2..5);
        let inner_ptr_after = sub.as_slice().as_ptr() as usize;
        // Pointer 2 bytes further along: same Arc contents, just an offset.
        assert_eq!(inner_ptr_after - inner_ptr_before, 2);
        assert_eq!(sub.as_slice(), &[3, 4, 5]);
    }

    #[test]
    fn nested_slice_offsets_compose() {
        let s = SampleBytes::from_vec(alloc::vec![0, 1, 2, 3, 4, 5, 6, 7]);
        let s1 = s.slice(2..7); // [2,3,4,5,6]
        let s2 = s1.slice(1..4); // [3,4,5]
        assert_eq!(s2.as_slice(), &[3, 4, 5]);
    }

    #[test]
    fn clone_is_refcount_bump() {
        let s = SampleBytes::from_vec(alloc::vec![1, 2, 3]);
        let p1 = s.as_slice().as_ptr();
        let s2 = s.clone();
        let p2 = s2.as_slice().as_ptr();
        assert_eq!(p1, p2, "Clone must share backing storage");
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn slice_oob_panics() {
        let s = SampleBytes::from_vec(alloc::vec![1, 2]);
        let _ = s.slice(0..5);
    }
}
