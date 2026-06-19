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

/// Refcounted byte container for reader-path samples.
///
/// Allows zero-copy slicing of the wire bytes by tracking a range over
/// an `Arc<[u8]>`. Clone is O(1).
#[derive(Debug, Clone)]
pub struct SampleBytes {
    /// Refcounted data — typically comes from RTPS `DeliveredSample::payload`.
    data: Arc<[u8]>,
    /// View range over `data`. Strip operations only shift the range.
    range: Range<usize>,
}

impl SampleBytes {
    /// Constructs from an owned `Vec<u8>`. Heap alloc for the Arc wrap.
    /// Use for test sample injection or when the bytes were freshly
    /// allocated anyway.
    #[must_use]
    pub fn from_vec(v: Vec<u8>) -> Self {
        let len = v.len();
        Self {
            data: Arc::from(v.into_boxed_slice()),
            range: 0..len,
        }
    }

    /// Constructs from an `Arc<[u8]>` with the full range. **Zero-copy** —
    /// only the refcount is incremented.
    #[must_use]
    pub fn from_arc(data: Arc<[u8]>) -> Self {
        let len = data.len();
        Self {
            data,
            range: 0..len,
        }
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
        Self { data, range }
    }

    /// Creates a new `SampleBytes` with the given sub-range relative to
    /// the current view. **Zero-copy** — refcount bump.
    ///
    /// # Panics
    /// If `sub.end > self.len()`.
    #[must_use]
    pub fn slice(&self, sub: Range<usize>) -> Self {
        assert!(sub.end <= self.len(), "SampleBytes::slice out of bounds");
        let start = self.range.start + sub.start;
        let end = self.range.start + sub.end;
        Self {
            data: Arc::clone(&self.data),
            range: start..end,
        }
    }

    /// Current view as `&[u8]`. No copy.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[self.range.clone()]
    }

    /// Number of bytes in the current view.
    #[must_use]
    pub fn len(&self) -> usize {
        self.range.end - self.range.start
    }

    /// `true` if the view is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
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
    fn from_vec_roundtrip() {
        let s = SampleBytes::from_vec(alloc::vec![1, 2, 3, 4]);
        assert_eq!(s.len(), 4);
        assert_eq!(s.as_slice(), &[1, 2, 3, 4]);
        assert_eq!(s.to_vec(), alloc::vec![1, 2, 3, 4]);
    }

    #[test]
    fn from_arc_full_range() {
        let arc: Arc<[u8]> = Arc::from(alloc::vec![10, 20, 30].into_boxed_slice());
        let s = SampleBytes::from_arc(arc);
        assert_eq!(s.as_slice(), &[10, 20, 30]);
    }

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
    fn empty_after_full_strip() {
        let s = SampleBytes::from_vec(alloc::vec![1, 2, 3]);
        let empty = s.slice(3..3);
        assert!(empty.is_empty());
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn slice_oob_panics() {
        let s = SampleBytes::from_vec(alloc::vec![1, 2]);
        let _ = s.slice(0..5);
    }
}
