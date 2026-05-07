// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fixed-Capacity Stack-Buffer fuer Hot-Path-Allokationen.
//!
//! `PoolBuffer<CAP>` ist ein on-stack array-basierter Buffer mit
//! fester Kapazitaet `CAP`. Append-Operationen sind O(1) ohne
//! Heap-Touch; Overflow wird als `PoolBufferError::Overflow`
//! signalisiert statt zu panicen. `no_std`-tauglich.
//!
//! ## Spec-Bezug
//!
//! Dieses Modul ist kein OMG-Spec-Mapping — es ist ein internes
//! Performance-Primitive fuer den Hot-Path
//! `Writer::write`/`Reader::take`, der ohne Pro-Sample-Realloc
//! arbeiten soll.

#![allow(clippy::module_name_repetitions)]

// ============================================================================
// PoolBuffer<CAP> — fixed-capacity byte buffer
// ============================================================================

/// Fixed-Capacity Byte-Buffer, der wie `Vec<u8>` befuellt werden kann
/// — aber keinen Heap-Realloc macht. Ueberlauf wird per
/// [`Result`] gemeldet, nicht per `panic!`.
///
/// Layout: `[u8; CAP]` + `len: u16` (max 65 535 Bytes pro Buffer).
/// Fuer DDS-Hot-Path-Samples bis 1.5 kB ist das genug; groessere
/// Samples laufen weiter ueber den `alloc`-Pfad.
#[derive(Debug)]
pub struct PoolBuffer<const CAP: usize> {
    bytes: [u8; CAP],
    len: u16,
}

impl<const CAP: usize> PoolBuffer<CAP> {
    /// Erzeugt einen leeren Buffer.
    ///
    /// `CAP` muss `<= u16::MAX as usize` sein — bei groesseren
    /// Werten lehnt jede Mutation mit [`PoolBufferError::CapacityTooLarge`] ab.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: [0u8; CAP],
            len: 0,
        }
    }

    /// Aktuelle Laenge (Bytes geschrieben).
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// `true` wenn keine Bytes geschrieben sind.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Statische Kapazitaet.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        CAP
    }

    /// Lesender Slice ueber die geschriebenen Bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        // `len` ist immer <= CAP per Konstruktion — alle Mutations-APIs
        // cappen ihn.
        self.bytes.get(..self.len()).unwrap_or(&[])
    }

    /// Mutabler Slice ueber den uninitialisierten Tail (Capacity-len).
    #[must_use]
    pub fn spare_capacity_mut(&mut self) -> &mut [u8] {
        let start = self.len();
        self.bytes.get_mut(start..).unwrap_or(&mut [])
    }

    /// Setzt den Inhalt zurueck auf `len = 0`. O(1), kein Memzero.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Haengt `data` ans Ende. Fehler wenn der Buffer voll waere.
    ///
    /// # Errors
    /// [`PoolBufferError::Overflow`] wenn `self.len() + data.len() > CAP`.
    /// [`PoolBufferError::CapacityTooLarge`] wenn `CAP > u16::MAX`.
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
        // needed <= CAP <= u16::MAX, kein truncate-Risiko.
        self.len = needed as u16;
        Ok(())
    }

    /// Schreibt ein einzelnes Byte. Fehler bei Vollheit.
    ///
    /// # Errors
    /// [`PoolBufferError::Overflow`] wenn der Buffer voll ist.
    pub fn push(&mut self, byte: u8) -> Result<(), PoolBufferError> {
        self.extend_from_slice(&[byte])
    }

    /// Setzt die Laenge explizit. Genutzt nach einem `spare_capacity_mut`-
    /// Schreibzugriff durch ein Codec-Backend.
    ///
    /// # Errors
    /// [`PoolBufferError::Overflow`] wenn `new_len > CAP`.
    pub fn set_len(&mut self, new_len: usize) -> Result<(), PoolBufferError> {
        if new_len > CAP || CAP > u16::MAX as usize {
            return Err(PoolBufferError::Overflow);
        }
        // Cast ist nach der Range-Validation safe.
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

/// Fehler-Familie fuer `PoolBuffer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolBufferError {
    /// Schreibende Operation wuerde die statische Kapazitaet
    /// ueberschreiten.
    Overflow,
    /// `CAP > u16::MAX`. Diese Variante existiert weil `len` als
    /// `u16` modelliert ist.
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
