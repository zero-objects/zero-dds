// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fixed-capacity stack buffer for hot-path allocations.
//!
//! `PoolBuffer<CAP>` is an on-stack array-based buffer with
//! fixed capacity `CAP`. Append operations are O(1) without
//! touching the heap; overflow is signalled as `PoolBufferError::Overflow`
//! instead of panicking. `no_std`-capable.
//!
//! ## Spec relation
//!
//! This module is not an OMG spec mapping — it is an internal
//! performance primitive for the hot path
//! `Writer::write`/`Reader::take`, which is meant to work without
//! a per-sample realloc.

#![allow(clippy::module_name_repetitions)]

// ============================================================================
// PoolBuffer<CAP> — fixed-capacity byte buffer
// ============================================================================

/// Fixed-capacity byte buffer that can be filled like `Vec<u8>`
/// — but does not do a heap realloc. Overflow is reported via
/// [`Result`], not via `panic!`.
///
/// Layout: `[u8; CAP]` + `len: u16` (max 65 535 bytes per buffer).
/// This is enough for DDS hot-path samples up to 1.5 kB; larger
/// samples continue to go through the `alloc` path.
#[derive(Debug)]
pub struct PoolBuffer<const CAP: usize> {
    bytes: [u8; CAP],
    len: u16,
}

impl<const CAP: usize> PoolBuffer<CAP> {
    /// Creates an empty buffer.
    ///
    /// `CAP` must be `<= u16::MAX as usize` — for larger
    /// values every mutation is rejected with [`PoolBufferError::CapacityTooLarge`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: [0u8; CAP],
            len: 0,
        }
    }

    /// Current length (bytes written).
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// `true` when no bytes have been written.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Static capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        CAP
    }

    /// Read-only slice over the written bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        // `len` is always <= CAP by construction — all mutation APIs
        // cap it.
        self.bytes.get(..self.len()).unwrap_or(&[])
    }

    /// Mutable slice over the uninitialized tail (capacity − len).
    #[must_use]
    pub fn spare_capacity_mut(&mut self) -> &mut [u8] {
        let start = self.len();
        self.bytes.get_mut(start..).unwrap_or(&mut [])
    }

    /// Resets the contents to `len = 0`. O(1), no memzero.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Appends `data` to the end. Errors if the buffer would be full.
    ///
    /// # Errors
    /// [`PoolBufferError::Overflow`] when `self.len() + data.len() > CAP`.
    /// [`PoolBufferError::CapacityTooLarge`] when `CAP > u16::MAX`.
    pub fn extend_from_slice(&mut self, data: &[u8]) -> Result<(), PoolBufferError> {
        if CAP > u16::MAX as usize {
            return Err(PoolBufferError::CapacityTooLarge);
        }
        let needed = self
            .len()
            .checked_add(data.len())
            .ok_or(PoolBufferError::Overflow)?;
        if needed > CAP {
            return Err(PoolBufferError::Overflow);
        }
        let start = self.len();
        let dst = self
            .bytes
            .get_mut(start..needed)
            .ok_or(PoolBufferError::Overflow)?;
        dst.copy_from_slice(data);
        // needed <= CAP <= u16::MAX, no truncation risk.
        self.len = needed as u16;
        Ok(())
    }

    /// Writes a single byte. Errors when full.
    ///
    /// # Errors
    /// [`PoolBufferError::Overflow`] when the buffer is full.
    pub fn push(&mut self, byte: u8) -> Result<(), PoolBufferError> {
        self.extend_from_slice(&[byte])
    }

    /// Sets the length explicitly. Used after a `spare_capacity_mut`
    /// write access by a codec backend.
    ///
    /// # Errors
    /// [`PoolBufferError::Overflow`] when `new_len > CAP`.
    pub fn set_len(&mut self, new_len: usize) -> Result<(), PoolBufferError> {
        if new_len > CAP || CAP > u16::MAX as usize {
            return Err(PoolBufferError::Overflow);
        }
        // The cast is safe after the range validation.
        self.len = new_len as u16;
        Ok(())
    }
}

impl<const CAP: usize> Default for PoolBuffer<CAP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CAP: usize> AsRef<[u8]> for PoolBuffer<CAP> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

// ============================================================================
// Error
// ============================================================================

/// Error family for `PoolBuffer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolBufferError {
    /// A write operation would exceed the static capacity.
    Overflow,
    /// `CAP > u16::MAX`. This variant exists because `len` is
    /// modeled as a `u16`.
    CapacityTooLarge,
}

impl core::fmt::Display for PoolBufferError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Overflow => f.write_str("pool buffer overflow"),
            Self::CapacityTooLarge => f.write_str("CAP exceeds u16::MAX"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PoolBufferError {}

// ============================================================================
// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_has_len_zero() {
        let b: PoolBuffer<128> = PoolBuffer::new();
        assert_eq!(b.len(), 0);
        assert!(b.is_empty());
        assert_eq!(b.capacity(), 128);
        assert_eq!(b.as_slice(), &[]);
    }

    #[test]
    fn extend_appends_and_tracks_len() {
        let mut b: PoolBuffer<16> = PoolBuffer::new();
        b.extend_from_slice(b"hello").unwrap();
        assert_eq!(b.len(), 5);
        assert_eq!(b.as_slice(), b"hello");
        b.extend_from_slice(b" world").unwrap();
        assert_eq!(b.as_slice(), b"hello world");
    }

    #[test]
    fn extend_overflow_returns_error_no_partial_write() {
        let mut b: PoolBuffer<8> = PoolBuffer::new();
        b.extend_from_slice(b"1234").unwrap();
        let err = b.extend_from_slice(b"56789").unwrap_err();
        assert_eq!(err, PoolBufferError::Overflow);
        assert_eq!(b.as_slice(), b"1234");
    }

    #[test]
    fn push_byte_works() {
        let mut b: PoolBuffer<4> = PoolBuffer::new();
        b.push(0xCA).unwrap();
        b.push(0xFE).unwrap();
        assert_eq!(b.as_slice(), &[0xCA, 0xFE]);
    }

    #[test]
    fn push_overflow_errors() {
        let mut b: PoolBuffer<2> = PoolBuffer::new();
        b.push(1).unwrap();
        b.push(2).unwrap();
        assert_eq!(b.push(3).unwrap_err(), PoolBufferError::Overflow);
    }

    #[test]
    fn clear_resets_len_only() {
        let mut b: PoolBuffer<16> = PoolBuffer::new();
        b.extend_from_slice(b"abc").unwrap();
        b.clear();
        assert_eq!(b.len(), 0);
        assert_eq!(b.as_slice(), &[]);
        b.extend_from_slice(b"xyz").unwrap();
        assert_eq!(b.as_slice(), b"xyz");
    }

    #[test]
    fn spare_capacity_then_set_len() {
        let mut b: PoolBuffer<32> = PoolBuffer::new();
        let spare = b.spare_capacity_mut();
        assert_eq!(spare.len(), 32);
        spare[0..3].copy_from_slice(&[1, 2, 3]);
        b.set_len(3).unwrap();
        assert_eq!(b.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn set_len_overflow_errors() {
        let mut b: PoolBuffer<8> = PoolBuffer::new();
        assert_eq!(b.set_len(9).unwrap_err(), PoolBufferError::Overflow);
    }

    #[test]
    fn cap_above_u16_max_extend_errors() {
        let mut b: PoolBuffer<{ u16::MAX as usize + 1 }> = PoolBuffer::new();
        let err = b.extend_from_slice(&[0u8]).unwrap_err();
        assert_eq!(err, PoolBufferError::CapacityTooLarge);
    }

    #[test]
    fn error_display_strings() {
        let s = std::format!("{}", PoolBufferError::Overflow);
        assert!(s.contains("overflow"));
        let s = std::format!("{}", PoolBufferError::CapacityTooLarge);
        assert!(s.contains("u16"));
    }
}
