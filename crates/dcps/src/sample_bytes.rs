// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `SampleBytes` — Zero-Copy Byte-Container fuer Reader-Path-Samples.
//!
//! Spec: `docs/specs/zerodds-zero-copy-1.0.md` §6 Welle 2.
//!
//! # Hintergrund
//!
//! `UserSample::Alive::payload` war historisch `Vec<u8>`. Damit musste
//! `strip_user_encap` den Encap-Header-Offset durch `payload[off..].to_vec()`
//! abschneiden — ein Heap-Alloc + Copy pro Alive-Sample.
//!
//! `SampleBytes` ersetzt diesen Vec mit einer `Arc<[u8]>` + Range. Strip-
//! Operationen werden zu reinen Index-Arithmetik (`Arc::clone` ist ein
//! Refcount-Bump, kein Copy). Heap-Allocs entfallen am Hot-Path.
//!
//! # API
//!
//! - [`SampleBytes::from_vec`] — Heap-Vec-Input (Backward-Compat).
//! - [`SampleBytes::from_arc_slice`] — Arc + Range (Zero-Copy-Pfad).
//! - [`SampleBytes::as_slice`] — Slice-Read ohne Copy.
//! - [`SampleBytes::to_vec`] — Materialization an FFI-Boundary.
//! - [`SampleBytes::len`] / `is_empty` — Standard-Container-API.
//! - `Clone` ist O(1) — nur Arc-Refcount-Bump.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

/// Refcounted Byte-Container fuer Reader-Path-Samples.
///
/// Erlaubt Zero-Copy-Slicing der Wire-Bytes durch Range-Tracking auf
/// einem `Arc<[u8]>`. Clone ist O(1).
#[derive(Debug, Clone)]
pub struct SampleBytes {
    /// Refcounted Daten — kommt typisch aus RTPS-`DeliveredSample::payload`.
    data: Arc<[u8]>,
    /// Sicht-Range auf `data`. Strip-Operationen verschieben nur die Range.
    range: Range<usize>,
}

impl SampleBytes {
    /// Konstruiert aus einem owned `Vec<u8>`. Heap-Alloc fuer den Arc-Wrap.
    /// Use bei Test-Sample-Injection oder wenn die Bytes ohnehin frisch
    /// alloziert wurden.
    #[must_use]
    pub fn from_vec(v: Vec<u8>) -> Self {
        let len = v.len();
        Self {
            data: Arc::from(v.into_boxed_slice()),
            range: 0..len,
        }
    }

    /// Konstruiert aus einem `Arc<[u8]>` mit voller Range. **Zero-Copy** —
    /// es wird nur der Refcount erhoeht.
    #[must_use]
    pub fn from_arc(data: Arc<[u8]>) -> Self {
        let len = data.len();
        Self {
            data,
            range: 0..len,
        }
    }

    /// Konstruiert mit explizitem Range auf einem `Arc<[u8]>`. **Zero-Copy**.
    ///
    /// # Panics
    /// Wenn `range.end > data.len()` oder `range.start > range.end`.
    #[must_use]
    pub fn from_arc_slice(data: Arc<[u8]>, range: Range<usize>) -> Self {
        assert!(
            range.end <= data.len() && range.start <= range.end,
            "SampleBytes range out of bounds"
        );
        Self { data, range }
    }

    /// Erzeugt eine neue `SampleBytes` mit der gegebenen Sub-Range
    /// relativ zur aktuellen Sicht. **Zero-Copy** — Refcount-Bump.
    ///
    /// # Panics
    /// Wenn `sub.end > self.len()`.
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

    /// Aktuelle Sicht als `&[u8]`. Kein Copy.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[self.range.clone()]
    }

    /// Anzahl Bytes in der aktuellen Sicht.
    #[must_use]
    pub fn len(&self) -> usize {
        self.range.end - self.range.start
    }

    /// `true` wenn die Sicht leer ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    /// Materialisiert die Sicht in ein `Vec<u8>`. **Kopiert** — nur an
    /// FFI-Boundaries verwenden wo owned-Daten an C/Python/JS uebergeben
    /// werden muessen.
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
        // Pointer 2 bytes weiter: gleicher Arc-Inhalt, nur Offset.
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
