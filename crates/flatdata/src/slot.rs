// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! SHM slot header (Spec §2 zerodds-flatdata-1.0).
//!
//! Layout:
//!
//! ```text
//! +--------- SHM-Slot ---------+
//! | 0x00 | u32 | sequence_number |
//! | 0x04 | u32 | sample_size     |
//! | 0x08 | u32 | reader_mask     |
//! | 0x0c | u32 | _reserved       |
//! | 0x10 | [u8]| FlatStruct-Daten |
//! +-----------------------------+
//! ```
//!
//! Header size: 16 bytes. The header lives **outside** the
//! FlatStruct data region and is not part of `FlatStruct::WIRE_SIZE`.

/// Size of the slot header in bytes.
pub const SLOT_HEADER_SIZE: usize = 16;

/// Bitmap (32 bit) — one bit per reader slot. Bit set = reader has
/// read. The slot becomes reservable again when all active reader bits
/// are set, or the timeout has elapsed.
pub type ReaderMask = u32;

/// Slot header — set by the writer in `commit_slot` and interpreted by
/// the reader in `read_flat`.
///
/// The layout is `repr(C, packed(4))` so the wire format is byte-stable
/// across all platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(4))]
pub struct SlotHeader {
    /// Writer-local sequence number.
    pub sequence_number: u32,
    /// Sample size in bytes (= `T::WIRE_SIZE` for `FlatStruct<T>`).
    pub sample_size: u32,
    /// Bitmap: which readers have read. 0 = new, all 1-bits = free.
    pub reader_mask: ReaderMask,
    /// Padding for cache-line alignment + future extensions.
    pub _reserved: u32,
}

impl SlotHeader {
    /// Constructor for a new slot on commit.
    #[must_use]
    pub const fn new(sn: u32, sample_size: u32) -> Self {
        Self {
            sequence_number: sn,
            sample_size,
            reader_mask: 0,
            _reserved: 0,
        }
    }

    /// `true` when all bits in `active_readers` are set ⇒ the slot
    /// is free (all readers have read).
    #[must_use]
    pub const fn all_read(&self, active_readers_mask: ReaderMask) -> bool {
        // The slot is free when all active readers have set their bit.
        // Inactive readers (no bit in active_readers_mask) count
        // automatically as "read".
        (self.reader_mask & active_readers_mask) == active_readers_mask
    }

    /// Sets bit `reader_index` in reader_mask (the reader has read).
    pub fn mark_read(&mut self, reader_index: u8) {
        debug_assert!(reader_index < 32, "max 32 readers per topic");
        self.reader_mask |= 1u32 << reader_index;
    }

    /// Wire encoding: 16 bytes little-endian.
    #[must_use]
    pub fn to_bytes_le(&self) -> [u8; SLOT_HEADER_SIZE] {
        let mut out = [0u8; SLOT_HEADER_SIZE];
        out[0..4].copy_from_slice(&self.sequence_number.to_le_bytes());
        out[4..8].copy_from_slice(&self.sample_size.to_le_bytes());
        out[8..12].copy_from_slice(&self.reader_mask.to_le_bytes());
        out[12..16].copy_from_slice(&self._reserved.to_le_bytes());
        out
    }

    /// Wire decoding from 16 bytes little-endian.
    ///
    /// # Errors
    /// Returns `None` when `bytes` is too short.
    #[must_use]
    pub fn from_bytes_le(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < SLOT_HEADER_SIZE {
            return None;
        }
        Some(Self {
            sequence_number: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            sample_size: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            reader_mask: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            _reserved: u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn header_size_is_16() {
        assert_eq!(SLOT_HEADER_SIZE, 16);
        assert_eq!(core::mem::size_of::<SlotHeader>(), SLOT_HEADER_SIZE);
    }

    #[test]
    fn new_header_has_zero_mask() {
        let h = SlotHeader::new(7, 24);
        assert_eq!(h.sequence_number, 7);
        assert_eq!(h.sample_size, 24);
        assert_eq!(h.reader_mask, 0);
    }

    #[test]
    fn mark_read_sets_bit() {
        let mut h = SlotHeader::new(1, 24);
        h.mark_read(0);
        h.mark_read(2);
        assert_eq!(h.reader_mask, 0b101);
    }

    #[test]
    fn all_read_with_two_active_readers() {
        let mut h = SlotHeader::new(1, 24);
        // Active readers: bit 0 + bit 1.
        let active = 0b011;
        assert!(!h.all_read(active));
        h.mark_read(0);
        assert!(!h.all_read(active));
        h.mark_read(1);
        assert!(h.all_read(active));
    }

    #[test]
    fn inactive_reader_bits_dont_block() {
        let mut h = SlotHeader::new(1, 24);
        // Only reader 0 is active. Reader 5 has not set its bit
        // either — it must not block.
        let active = 0b001;
        h.mark_read(0);
        assert!(h.all_read(active));
    }

    #[test]
    fn roundtrip_le() {
        let h = SlotHeader {
            sequence_number: 0xAABB_CCDD,
            sample_size: 24,
            reader_mask: 0b1111_0000,
            _reserved: 0,
        };
        let bytes = h.to_bytes_le();
        let h2 = SlotHeader::from_bytes_le(&bytes).expect("decode");
        assert_eq!(h, h2);
    }

    #[test]
    fn from_bytes_too_short_returns_none() {
        assert!(SlotHeader::from_bytes_le(&[0u8; 15]).is_none());
    }
}
