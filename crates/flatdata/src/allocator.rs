// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! In-Memory-Slot-Allocator als Referenz-Implementation des
//! [`SlotBackend`](crate::SlotBackend)-Traits. Single-Process
//! Same-Host-Pub/Sub ohne mmap-Overhead — fuer Tests und als
//! Default-Backend wenn `posix-mmap` deaktiviert ist. Der
//! Cross-Process-mmap-Backend lebt in `posix.rs` und implementiert
//! dieselbe Trait-API.
//!
//! Spec: zerodds-flatdata-1.0 §4.1, §5.1.

use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::sync::Mutex;

use crate::slot::{ReaderMask, SLOT_HEADER_SIZE, SlotHeader};

/// Slot-Identifikation: (segment_id, slot_index).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotHandle {
    /// Segment-ID (FNV-Hash des Segment-Pfads).
    pub segment_id: u64,
    /// Slot-Index in [0, slot_count).
    pub slot_index: u32,
}

/// Fehler beim Slot-Management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotError {
    /// Alle Slots sind belegt — keiner frei.
    NoFreeSlot,
    /// Slot-Index ausserhalb [0, slot_count).
    OutOfBounds,
    /// Sample passt nicht in Slot-Size.
    SampleTooLarge {
        /// Sample-Bytes.
        sample: usize,
        /// Konfigurierter Slot-Datenbereich.
        slot_capacity: usize,
    },
    /// Slot-Lock konnte nicht erworben werden (poisoned).
    LockPoisoned,
}

impl core::fmt::Display for SlotError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoFreeSlot => f.write_str("no free slot in segment"),
            Self::OutOfBounds => f.write_str("slot index out of bounds"),
            Self::SampleTooLarge {
                sample,
                slot_capacity,
            } => write!(
                f,
                "sample {sample} byte does not fit in slot capacity {slot_capacity}"
            ),
            Self::LockPoisoned => f.write_str("slot lock poisoned"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SlotError {}

/// In-Memory-Slot-Allocator. Vermittelt das gleiche
/// [`SlotBackend`](crate::SlotBackend)-Interface wie der
/// POSIX-mmap-Backend (`posix.rs`), aber liegt im Process-Heap —
/// Single-Process-Pub/Sub und Test-Setups ohne mmap-Dep.
#[cfg(feature = "std")]
pub struct InMemorySlotAllocator {
    slots: Arc<Mutex<Vec<Slot>>>,
    segment_id: u64,
    slot_capacity: usize,
    next_sn: Arc<core::sync::atomic::AtomicU32>,
    /// Optionaler Type-Hash fuer Cross-Validation (Spec §6.1).
    type_hash: Option<[u8; 16]>,
}

#[derive(Debug, Clone)]
struct Slot {
    header: SlotHeader,
    data: Vec<u8>,
    /// `true` wenn aktuell ein Loan aktiv ist (Writer hat reserviert,
    /// noch kein commit/discard).
    loaned: bool,
}

#[cfg(feature = "std")]
impl crate::backend::SlotBackend for InMemorySlotAllocator {
    fn reserve_slot(&self, active_readers_mask: ReaderMask) -> Result<SlotHandle, SlotError> {
        Self::reserve_slot(self, active_readers_mask)
    }
    fn commit_slot(&self, handle: SlotHandle, bytes: &[u8]) -> Result<u32, SlotError> {
        Self::commit_slot(self, handle, bytes)
    }
    fn discard_slot(&self, handle: SlotHandle) -> Result<(), SlotError> {
        Self::discard_slot(self, handle)
    }
    fn read_slot(&self, handle: SlotHandle) -> Result<(SlotHeader, Vec<u8>), SlotError> {
        Self::read_slot(self, handle)
    }
    fn mark_read(&self, handle: SlotHandle, reader_index: u8) -> Result<(), SlotError> {
        Self::mark_read(self, handle, reader_index)
    }
    fn mark_reader_disconnected(&self, reader_index: u8) -> Result<(), SlotError> {
        Self::mark_reader_disconnected(self, reader_index)
    }
    fn slot_count(&self) -> Result<usize, SlotError> {
        Self::slot_count(self)
    }
    fn slot_total_size(&self) -> usize {
        Self::slot_total_size(self)
    }
    fn slot_capacity(&self) -> usize {
        Self::slot_capacity(self)
    }
    fn type_hash(&self) -> Option<[u8; 16]> {
        self.type_hash
    }
}

#[cfg(feature = "std")]
impl InMemorySlotAllocator {
    /// Erzeugt einen neuen Allocator mit `slot_count` Slots zu je
    /// `slot_capacity` Bytes Daten-Bereich.
    #[must_use]
    pub fn new(segment_id: u64, slot_count: usize, slot_capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            slots.push(Slot {
                header: SlotHeader::new(0, 0),
                data: alloc::vec![0u8; slot_capacity],
                loaned: false,
            });
        }
        Self {
            slots: Arc::new(Mutex::new(slots)),
            segment_id,
            slot_capacity,
            next_sn: Arc::new(core::sync::atomic::AtomicU32::new(0)),
            type_hash: None,
        }
    }

    /// Spec §6.1: Allocator mit verbundenem Type-Hash. Reader, der
    /// gegen diesen Backend liest, prueft den Hash gegen
    /// `T::TYPE_HASH` und droppt bei Mismatch.
    #[must_use]
    pub fn with_type_hash(mut self, hash: [u8; 16]) -> Self {
        self.type_hash = Some(hash);
        self
    }

    /// Spec §4.1 reserve_slot. Sucht einen freien Slot (alle aktiven
    /// Reader haben gelesen) oder den ersten unbenutzten.
    ///
    /// # Errors
    /// `NoFreeSlot` wenn alle Slots geloant oder unfertig.
    pub fn reserve_slot(&self, active_readers_mask: ReaderMask) -> Result<SlotHandle, SlotError> {
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        for (idx, slot) in slots.iter_mut().enumerate() {
            if slot.loaned {
                continue;
            }
            // Slot frei wenn (a) leer (sample_size==0) oder (b) alle
            // Reader gelesen haben.
            if slot.header.sample_size == 0 || slot.header.all_read(active_readers_mask) {
                slot.loaned = true;
                return Ok(SlotHandle {
                    segment_id: self.segment_id,
                    slot_index: idx as u32,
                });
            }
        }
        Err(SlotError::NoFreeSlot)
    }

    /// Spec §4.1 commit_slot. Schreibt sample-bytes in den Slot und
    /// setzt SlotHeader { sn, sample_size, reader_mask=0 }.
    ///
    /// # Errors
    /// `OutOfBounds`, `SampleTooLarge`, oder Lock-Poison.
    pub fn commit_slot(&self, handle: SlotHandle, bytes: &[u8]) -> Result<u32, SlotError> {
        if bytes.len() > self.slot_capacity {
            return Err(SlotError::SampleTooLarge {
                sample: bytes.len(),
                slot_capacity: self.slot_capacity,
            });
        }
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        let idx = handle.slot_index as usize;
        if idx >= slots.len() {
            return Err(SlotError::OutOfBounds);
        }
        let sn = self
            .next_sn
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        let slot = &mut slots[idx];
        let sample_size = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        slot.header = SlotHeader::new(sn, sample_size);
        slot.data[..bytes.len()].copy_from_slice(bytes);
        slot.loaned = false;
        Ok(sn)
    }

    /// Spec §4.1 release_slot (kein commit; verwirft Loan).
    ///
    /// # Errors
    /// `OutOfBounds` oder Lock-Poison.
    pub fn discard_slot(&self, handle: SlotHandle) -> Result<(), SlotError> {
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        let idx = handle.slot_index as usize;
        if idx >= slots.len() {
            return Err(SlotError::OutOfBounds);
        }
        slots[idx].loaned = false;
        Ok(())
    }

    /// Reader-Side: liest Slot-Header + bytes.
    ///
    /// # Errors
    /// `OutOfBounds`.
    pub fn read_slot(&self, handle: SlotHandle) -> Result<(SlotHeader, Vec<u8>), SlotError> {
        let slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        let idx = handle.slot_index as usize;
        if idx >= slots.len() {
            return Err(SlotError::OutOfBounds);
        }
        let slot = &slots[idx];
        let n = slot.header.sample_size as usize;
        Ok((slot.header, slot.data[..n.min(slot.data.len())].to_vec()))
    }

    /// Reader-Side: setzt `reader_index`-Bit im reader_mask.
    ///
    /// # Errors
    /// `OutOfBounds` oder Lock-Poison.
    pub fn mark_read(&self, handle: SlotHandle, reader_index: u8) -> Result<(), SlotError> {
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        let idx = handle.slot_index as usize;
        if idx >= slots.len() {
            return Err(SlotError::OutOfBounds);
        }
        slots[idx].header.mark_read(reader_index);
        Ok(())
    }

    /// Spec §5.2 Reader-Disconnect retroaktiv.
    ///
    /// Wird vom Caller (SPDP-Lease-Expiry-Hook) gerufen wenn ein
    /// Reader gestorben ist. Setzt sein Bit auf **allen** Slots,
    /// damit der Slot-Allocator ihn als "hat gelesen" sieht und
    /// belegte Slots wieder freigibt.
    ///
    /// # Errors
    /// Lock-Poison.
    pub fn mark_reader_disconnected(&self, reader_index: u8) -> Result<(), SlotError> {
        debug_assert!(reader_index < 32);
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        for slot in slots.iter_mut() {
            slot.header.reader_mask |= 1u32 << reader_index;
        }
        Ok(())
    }

    /// Slot-Capacity (data-Bereich; ohne Header).
    #[must_use]
    pub fn slot_capacity(&self) -> usize {
        self.slot_capacity
    }

    /// Anzahl konfigurierter Slots.
    pub fn slot_count(&self) -> Result<usize, SlotError> {
        Ok(self
            .slots
            .lock()
            .map_err(|_| SlotError::LockPoisoned)?
            .len())
    }

    /// Berechnet die Gesamt-Slot-Groesse (Header + Daten, gepaddet auf
    /// 64-byte Cache-Line) — fuer SEDP-Discovery.
    #[must_use]
    pub fn slot_total_size(&self) -> usize {
        let raw = SLOT_HEADER_SIZE + self.slot_capacity;
        (raw + 63) & !63
    }
}

#[cfg(all(test, feature = "std"))]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn reserve_returns_first_free_slot() {
        let alloc = InMemorySlotAllocator::new(0, 4, 64);
        let h0 = alloc.reserve_slot(0).expect("reserve 0");
        assert_eq!(h0.slot_index, 0);
        let h1 = alloc.reserve_slot(0).expect("reserve 1");
        assert_eq!(h1.slot_index, 1);
    }

    #[test]
    fn reserve_returns_no_free_slot_when_all_loaned() {
        let alloc = InMemorySlotAllocator::new(0, 2, 64);
        let _h0 = alloc.reserve_slot(0).unwrap();
        let _h1 = alloc.reserve_slot(0).unwrap();
        assert_eq!(alloc.reserve_slot(0), Err(SlotError::NoFreeSlot));
    }

    #[test]
    fn commit_writes_bytes_and_increments_sn() {
        let alloc = InMemorySlotAllocator::new(0, 2, 64);
        let h = alloc.reserve_slot(0).unwrap();
        let sn = alloc.commit_slot(h, &[1, 2, 3]).unwrap();
        assert_eq!(sn, 0);
        let (header, bytes) = alloc.read_slot(h).unwrap();
        assert_eq!(header.sequence_number, 0);
        assert_eq!(header.sample_size, 3);
        assert_eq!(bytes, vec![1, 2, 3]);

        let h2 = alloc.reserve_slot(0).unwrap();
        let sn2 = alloc.commit_slot(h2, &[9]).unwrap();
        assert_eq!(sn2, 1);
    }

    #[test]
    fn commit_too_large_returns_error() {
        let alloc = InMemorySlotAllocator::new(0, 2, 8);
        let h = alloc.reserve_slot(0).unwrap();
        let err = alloc.commit_slot(h, &[0u8; 16]).unwrap_err();
        assert!(matches!(
            err,
            SlotError::SampleTooLarge {
                sample: 16,
                slot_capacity: 8
            }
        ));
    }

    #[test]
    fn discard_frees_slot_for_reuse() {
        let alloc = InMemorySlotAllocator::new(0, 1, 64);
        let h = alloc.reserve_slot(0).unwrap();
        alloc.discard_slot(h).unwrap();
        // Slot kann wieder reserviert werden.
        let _ = alloc.reserve_slot(0).unwrap();
    }

    #[test]
    fn slot_recyclable_after_all_readers_marked() {
        let alloc = InMemorySlotAllocator::new(0, 1, 64);
        // 2 aktive Reader: Bit 0 + Bit 1.
        let active = 0b011;
        let h = alloc.reserve_slot(active).unwrap();
        alloc.commit_slot(h, &[0xAA]).unwrap();

        // Reservation scheitert — Slot ist noch nicht von allen gelesen.
        assert_eq!(alloc.reserve_slot(active), Err(SlotError::NoFreeSlot));

        alloc.mark_read(h, 0).unwrap();
        assert_eq!(alloc.reserve_slot(active), Err(SlotError::NoFreeSlot));

        alloc.mark_read(h, 1).unwrap();
        // Jetzt frei.
        let _ = alloc.reserve_slot(active).unwrap();
    }

    #[test]
    fn slot_total_size_is_cache_line_padded() {
        let alloc = InMemorySlotAllocator::new(0, 4, 100);
        // Header(16) + Data(100) = 116 → padded auf 128.
        assert_eq!(alloc.slot_total_size(), 128);
    }

    #[test]
    fn reader_disconnect_frees_blocked_slots() {
        // Spec §5.2: Reader-Lease-Expiry triggert mark_reader_disconnected;
        // Slots die auf den toten Reader warteten, werden frei.
        let alloc = InMemorySlotAllocator::new(0, 1, 64);
        // Aktive Reader: 0 + 1.
        let active = 0b011;
        let h = alloc.reserve_slot(active).unwrap();
        alloc.commit_slot(h, &[0xAA]).unwrap();

        // Nur Reader 0 hat gelesen, Reader 1 nicht.
        alloc.mark_read(h, 0).unwrap();
        assert_eq!(alloc.reserve_slot(active), Err(SlotError::NoFreeSlot));

        // Reader 1 disconnected — Slot wird wieder reservierbar.
        alloc.mark_reader_disconnected(1).unwrap();
        let _ = alloc.reserve_slot(active).expect("free after disconnect");
    }
}
