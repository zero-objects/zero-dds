// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! In-memory slot allocator as the reference implementation of the
//! [`SlotBackend`](crate::SlotBackend) trait. Single-process
//! same-host pub/sub without mmap overhead — for tests and as the
//! default backend when `posix-mmap` is disabled. The
//! cross-process mmap backend lives in `posix.rs` and implements
//! the same trait API.
//!
//! Spec: zerodds-flatdata-1.0 §4.1, §5.1.

use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::sync::{Condvar, Mutex};

use crate::slot::{ReaderMask, SLOT_HEADER_SIZE, SlotHeader};

/// Event-driven notify for the in-memory backend (Spec §4.2). A monotonic
/// generation counter behind a `Condvar`: bumped on every state change a waiter
/// could care about (sample committed → wakes blocked readers; slot freed →
/// wakes blocked writers). Waiters re-check their own condition after waking
/// (Condvar discipline), so a single counter serves both directions without
/// busy-polling.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
struct ChangeNotify {
    generation: Mutex<u64>,
    cvar: Condvar,
}

#[cfg(feature = "std")]
impl ChangeNotify {
    fn current(&self) -> u64 {
        self.generation.lock().map(|g| *g).unwrap_or(0)
    }

    fn bump(&self) {
        if let Ok(mut g) = self.generation.lock() {
            *g = g.wrapping_add(1);
            self.cvar.notify_all();
        }
    }

    /// Blocks until the generation differs from `last` or `timeout` elapses.
    fn wait_change(&self, last: u64, timeout: core::time::Duration) {
        if let Ok(g) = self.generation.lock() {
            // `wait_timeout_while` re-checks the predicate, absorbing spurious
            // wakeups; it returns when gen != last or the deadline passes.
            let _ = self.cvar.wait_timeout_while(g, timeout, |g| *g == last);
        }
    }
}

/// Slot identification: (segment_id, slot_index).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotHandle {
    /// Segment ID (FNV hash of the segment path).
    pub segment_id: u64,
    /// Slot index in [0, slot_count).
    pub slot_index: u32,
}

/// Error during slot management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotError {
    /// All slots are occupied — none free.
    NoFreeSlot,
    /// Slot index outside [0, slot_count).
    OutOfBounds,
    /// Sample does not fit in the slot size.
    SampleTooLarge {
        /// Sample bytes.
        sample: usize,
        /// Configured slot data area.
        slot_capacity: usize,
    },
    /// Slot lock could not be acquired (poisoned).
    LockPoisoned,
    /// The backend does not support in-place loan
    /// (`slot_data_ptr`/`commit_in_place`).
    InPlaceUnsupported,
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
            Self::InPlaceUnsupported => f.write_str("backend does not support in-place loan"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SlotError {}

/// In-memory slot allocator. Provides the same
/// [`SlotBackend`](crate::SlotBackend) interface as the
/// POSIX mmap backend (`posix.rs`), but lives on the process heap —
/// single-process pub/sub and test setups without an mmap dep.
#[cfg(feature = "std")]
pub struct InMemorySlotAllocator {
    slots: Arc<Mutex<Vec<Slot>>>,
    segment_id: u64,
    slot_capacity: usize,
    next_sn: Arc<core::sync::atomic::AtomicU32>,
    /// Optional type-hash for cross-validation (spec §6.1).
    type_hash: Option<[u8; 16]>,
    /// Event-driven notify (Spec §4.2): bumped on commit (wake readers) and on
    /// slot-free (wake writers in backpressure). Shared via `Arc` so clones
    /// observe the same generation.
    notify: Arc<ChangeNotify>,
}

#[derive(Debug, Clone)]
struct Slot {
    header: SlotHeader,
    data: Vec<u8>,
    /// `true` when a loan is currently active (writer has reserved,
    /// not yet committed/discarded).
    loaned: bool,
    /// Wall-clock instant the current sample was committed — writer-side,
    /// out-of-band (not in the wire header). Drives the §5.1 timeout-eviction
    /// of slots held hostage by a hung reader. `None` = never committed.
    committed_at: Option<std::time::Instant>,
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
    fn slot_data_ptr(&self, handle: SlotHandle) -> Result<(*mut u8, usize), SlotError> {
        Self::slot_data_ptr(self, handle)
    }
    fn commit_in_place(&self, handle: SlotHandle, len: usize) -> Result<u32, SlotError> {
        Self::commit_in_place(self, handle, len)
    }
    fn slot_read_ptr(&self, handle: SlotHandle) -> Result<(*const u8, usize), SlotError> {
        Self::slot_read_ptr(self, handle)
    }
    fn next_unread_slot(&self, reader_index: u8) -> Result<Option<SlotHandle>, SlotError> {
        Self::next_unread_slot(self, reader_index)
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
    fn notify_generation(&self) -> u64 {
        self.notify_gen()
    }
    fn wait_for_change(&self, last: u64, timeout: core::time::Duration) {
        InMemorySlotAllocator::wait_for_change(self, last, timeout);
    }
}

#[cfg(feature = "std")]
impl InMemorySlotAllocator {
    /// Creates a new allocator with `slot_count` slots, each with a
    /// `slot_capacity`-byte data area.
    #[must_use]
    pub fn new(segment_id: u64, slot_count: usize, slot_capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            slots.push(Slot {
                header: SlotHeader::new(0, 0),
                data: alloc::vec![0u8; slot_capacity],
                loaned: false,
                committed_at: None,
            });
        }
        Self {
            slots: Arc::new(Mutex::new(slots)),
            segment_id,
            slot_capacity,
            next_sn: Arc::new(core::sync::atomic::AtomicU32::new(0)),
            type_hash: None,
            notify: Arc::new(ChangeNotify::default()),
        }
    }

    /// Current notify generation. Capture it before checking a condition and
    /// pass it to [`Self::wait_for_change`] to avoid a lost wakeup.
    #[must_use]
    pub fn notify_gen(&self) -> u64 {
        self.notify.current()
    }

    /// Blocks until the notify generation differs from `last` or `timeout`
    /// elapses (event-driven, Spec §4.2 — no busy-poll). Pair with
    /// [`Self::notify_gen`] for lost-wakeup-free waiting.
    pub fn wait_for_change(&self, last: u64, timeout: core::time::Duration) {
        self.notify.wait_change(last, timeout);
    }

    /// Spec §6.1: allocator with an associated type-hash. A reader
    /// reading against this backend checks the hash against
    /// `T::TYPE_HASH` and drops on mismatch.
    #[must_use]
    pub fn with_type_hash(mut self, hash: [u8; 16]) -> Self {
        self.type_hash = Some(hash);
        self
    }

    /// Spec §4.1 reserve_slot. Finds a free slot (all active
    /// readers have read) or the first unused one.
    ///
    /// # Errors
    /// `NoFreeSlot` when all slots are loaned or unfinished.
    pub fn reserve_slot(&self, active_readers_mask: ReaderMask) -> Result<SlotHandle, SlotError> {
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        for (idx, slot) in slots.iter_mut().enumerate() {
            if slot.loaned {
                continue;
            }
            // Slot is free if (a) empty (sample_size==0) or (b) all
            // readers have read.
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

    /// Spec §4.1 commit_slot. Writes the sample bytes into the slot
    /// and sets SlotHeader { sn, sample_size, reader_mask=0 }.
    ///
    /// # Errors
    /// `OutOfBounds`, `SampleTooLarge`, or lock poison.
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
        slot.committed_at = Some(std::time::Instant::now());
        drop(slots);
        self.notify.bump(); // new sample → wake blocked readers (§4.2)
        Ok(sn)
    }

    /// In-place loan: writable pointer + capacity into a reserved slot's data
    /// area (see [`SlotBackend::slot_data_ptr`](crate::SlotBackend::slot_data_ptr)).
    ///
    /// # Errors
    /// `OutOfBounds` if the slot is not a live loan, or lock poison.
    pub fn slot_data_ptr(&self, handle: SlotHandle) -> Result<(*mut u8, usize), SlotError> {
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        let idx = handle.slot_index as usize;
        let slot = slots.get_mut(idx).ok_or(SlotError::OutOfBounds)?;
        if !slot.loaned {
            return Err(SlotError::OutOfBounds);
        }
        Ok((slot.data.as_mut_ptr(), self.slot_capacity))
    }

    /// Finalizes an in-place loan (no copy) — sets the header + releases the
    /// loan (see
    /// [`SlotBackend::commit_in_place`](crate::SlotBackend::commit_in_place)).
    ///
    /// # Errors
    /// `SampleTooLarge`, `OutOfBounds`, or lock poison.
    pub fn commit_in_place(&self, handle: SlotHandle, len: usize) -> Result<u32, SlotError> {
        if len > self.slot_capacity {
            return Err(SlotError::SampleTooLarge {
                sample: len,
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
        let sample_size = u32::try_from(len).unwrap_or(u32::MAX);
        slot.header = SlotHeader::new(sn, sample_size);
        slot.loaned = false;
        slot.committed_at = Some(std::time::Instant::now());
        drop(slots);
        self.notify.bump();
        Ok(sn)
    }

    /// Zero-copy read pointer + sample length into a committed slot (see
    /// [`SlotBackend::slot_read_ptr`](crate::SlotBackend::slot_read_ptr)).
    ///
    /// # Errors
    /// `OutOfBounds` or lock poison.
    pub fn slot_read_ptr(&self, handle: SlotHandle) -> Result<(*const u8, usize), SlotError> {
        let slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        let idx = handle.slot_index as usize;
        let slot = slots.get(idx).ok_or(SlotError::OutOfBounds)?;
        Ok((slot.data.as_ptr(), slot.header.sample_size as usize))
    }

    /// Next committed slot not yet read by `reader_index` (see
    /// [`SlotBackend::next_unread_slot`](crate::SlotBackend::next_unread_slot)).
    ///
    /// # Errors
    /// Lock poison.
    pub fn next_unread_slot(&self, reader_index: u8) -> Result<Option<SlotHandle>, SlotError> {
        let slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        let bit = 1u32 << reader_index;
        for (idx, slot) in slots.iter().enumerate() {
            if slot.header.sample_size > 0 && (slot.header.reader_mask & bit) == 0 {
                return Ok(Some(SlotHandle {
                    segment_id: self.segment_id,
                    slot_index: idx as u32,
                }));
            }
        }
        Ok(None)
    }

    /// Spec §5.1 timeout-eviction. Force-frees every committed, not-yet-fully-
    /// read, not-currently-loaned slot whose sample was committed more than
    /// `max_age` ago — by setting all `active_readers_mask` bits so the slot
    /// counts as read and becomes reservable again. Guards against a single
    /// hung/dead reader holding a slot hostage forever (the SPDP lease path
    /// handles clean disconnects; this is the backstop for a stuck-but-alive
    /// reader). Returns the number of slots evicted. The writer side calls this
    /// periodically (e.g. from the DCPS tick).
    ///
    /// # Errors
    /// Lock poison.
    pub fn evict_stale(
        &self,
        max_age: core::time::Duration,
        active_readers_mask: ReaderMask,
    ) -> Result<usize, SlotError> {
        let now = std::time::Instant::now();
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        let mut evicted = 0;
        for slot in slots.iter_mut() {
            if slot.loaned || slot.header.sample_size == 0 {
                continue;
            }
            if slot.header.all_read(active_readers_mask) {
                continue; // already free
            }
            let stale = slot
                .committed_at
                .is_some_and(|t| now.saturating_duration_since(t) >= max_age);
            if stale {
                // Force-free: mark all active readers as read.
                slot.header.reader_mask |= active_readers_mask;
                evicted += 1;
            }
        }
        drop(slots);
        if evicted > 0 {
            self.notify.bump(); // slots freed → wake blocked writers (§4.2/§10.5)
        }
        Ok(evicted)
    }

    /// Spec §4.1 release_slot (no commit; discards the loan).
    ///
    /// # Errors
    /// `OutOfBounds` or lock poison.
    pub fn discard_slot(&self, handle: SlotHandle) -> Result<(), SlotError> {
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        let idx = handle.slot_index as usize;
        if idx >= slots.len() {
            return Err(SlotError::OutOfBounds);
        }
        slots[idx].loaned = false;
        drop(slots);
        self.notify.bump(); // loan released → wake blocked writers
        Ok(())
    }

    /// Reader side: reads slot header + bytes.
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

    /// Reader side: sets the `reader_index` bit in reader_mask.
    ///
    /// # Errors
    /// `OutOfBounds` or lock poison.
    pub fn mark_read(&self, handle: SlotHandle, reader_index: u8) -> Result<(), SlotError> {
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        let idx = handle.slot_index as usize;
        if idx >= slots.len() {
            return Err(SlotError::OutOfBounds);
        }
        slots[idx].header.mark_read(reader_index);
        drop(slots);
        self.notify.bump(); // reader consumed slot → may free it → wake writers
        Ok(())
    }

    /// Spec §5.2 reader disconnect, applied retroactively.
    ///
    /// Called by the caller (SPDP lease-expiry hook) when a
    /// reader has died. Sets its bit on **all** slots, so that
    /// the slot allocator sees it as "has read" and frees up
    /// occupied slots again.
    ///
    /// # Errors
    /// Lock poison.
    pub fn mark_reader_disconnected(&self, reader_index: u8) -> Result<(), SlotError> {
        debug_assert!(reader_index < 32);
        let mut slots = self.slots.lock().map_err(|_| SlotError::LockPoisoned)?;
        for slot in slots.iter_mut() {
            slot.header.reader_mask |= 1u32 << reader_index;
        }
        drop(slots);
        self.notify.bump(); // dead reader freed slots → wake blocked writers
        Ok(())
    }

    /// Slot capacity (data area; without header).
    #[must_use]
    pub fn slot_capacity(&self) -> usize {
        self.slot_capacity
    }

    /// Number of configured slots.
    pub fn slot_count(&self) -> Result<usize, SlotError> {
        Ok(self
            .slots
            .lock()
            .map_err(|_| SlotError::LockPoisoned)?
            .len())
    }

    /// Computes the total slot size (header + data, padded to a
    /// 64-byte cache line) — for SEDP discovery.
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
    fn in_place_loan_writes_without_staging_copy() {
        // Reserve → write directly into the slot via the raw pointer →
        // commit_in_place: the result must be byte-identical to commit_slot.
        let alloc = InMemorySlotAllocator::new(0, 2, 64);
        let h = alloc.reserve_slot(0).unwrap();
        let (ptr, cap) = alloc.slot_data_ptr(h).unwrap();
        assert!(cap >= 3);
        // SAFETY: slot is reserved (loaned); ptr is the slot data area, cap bytes.
        unsafe {
            ptr.add(0).write(1);
            ptr.add(1).write(2);
            ptr.add(2).write(3);
        }
        let sn = alloc.commit_in_place(h, 3).unwrap();
        assert_eq!(sn, 0);
        let (header, bytes) = alloc.read_slot(h).unwrap();
        assert_eq!(header.sequence_number, 0);
        assert_eq!(header.sample_size, 3);
        assert_eq!(bytes, vec![1, 2, 3]);

        // The loan was released → the slot is reservable again.
        let h2 = alloc.reserve_slot(0).unwrap();
        assert_eq!(alloc.commit_in_place(h2, 0).unwrap(), 1);
    }

    #[test]
    fn slot_data_ptr_rejects_unreserved_slot() {
        let alloc = InMemorySlotAllocator::new(0, 2, 64);
        // Slot 0 was never reserved → no live loan.
        let h = SlotHandle {
            segment_id: 0,
            slot_index: 0,
        };
        assert_eq!(alloc.slot_data_ptr(h), Err(SlotError::OutOfBounds));
    }

    #[test]
    fn commit_in_place_too_large_returns_error() {
        let alloc = InMemorySlotAllocator::new(0, 2, 8);
        let h = alloc.reserve_slot(0).unwrap();
        assert!(matches!(
            alloc.commit_in_place(h, 9),
            Err(SlotError::SampleTooLarge { .. })
        ));
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
        // The slot can be reserved again.
        let _ = alloc.reserve_slot(0).unwrap();
    }

    #[test]
    fn slot_recyclable_after_all_readers_marked() {
        let alloc = InMemorySlotAllocator::new(0, 1, 64);
        // 2 active readers: bit 0 + bit 1.
        let active = 0b011;
        let h = alloc.reserve_slot(active).unwrap();
        alloc.commit_slot(h, &[0xAA]).unwrap();

        // Reservation fails — the slot has not been read by everyone yet.
        assert_eq!(alloc.reserve_slot(active), Err(SlotError::NoFreeSlot));

        alloc.mark_read(h, 0).unwrap();
        assert_eq!(alloc.reserve_slot(active), Err(SlotError::NoFreeSlot));

        alloc.mark_read(h, 1).unwrap();
        // Now free.
        let _ = alloc.reserve_slot(active).unwrap();
    }

    #[test]
    fn slot_total_size_is_cache_line_padded() {
        let alloc = InMemorySlotAllocator::new(0, 4, 100);
        // Header(16) + Data(100) = 116 → padded to 128.
        assert_eq!(alloc.slot_total_size(), 128);
    }

    #[test]
    fn evict_stale_frees_slot_held_by_hung_reader() {
        // Spec §5.1: a committed slot a reader never read stays blocked; after
        // `max_age` the writer's timeout-eviction force-frees it.
        let alloc = InMemorySlotAllocator::new(0, 1, 64);
        let active = 0b1; // one active reader, never reads.
        let h = alloc.reserve_slot(active).unwrap();
        alloc.commit_slot(h, &[0xAA]).unwrap();

        // Blocked: the reader hasn't read.
        assert_eq!(alloc.reserve_slot(active), Err(SlotError::NoFreeSlot));

        // A future deadline evicts nothing.
        assert_eq!(
            alloc
                .evict_stale(core::time::Duration::from_secs(3600), active)
                .unwrap(),
            0
        );
        assert_eq!(alloc.reserve_slot(active), Err(SlotError::NoFreeSlot));

        // A zero deadline (everything is already older) evicts the slot.
        assert_eq!(
            alloc
                .evict_stale(core::time::Duration::ZERO, active)
                .unwrap(),
            1
        );
        // Now reservable again.
        let _ = alloc.reserve_slot(active).unwrap();
    }

    #[test]
    fn evict_stale_leaves_read_slots_untouched() {
        let alloc = InMemorySlotAllocator::new(0, 1, 64);
        let active = 0b1;
        let h = alloc.reserve_slot(active).unwrap();
        alloc.commit_slot(h, &[0xAA]).unwrap();
        alloc.mark_read(h, 0).unwrap(); // reader consumed it.
        // Already free → nothing to evict even at zero deadline.
        assert_eq!(
            alloc
                .evict_stale(core::time::Duration::ZERO, active)
                .unwrap(),
            0
        );
    }

    #[test]
    fn reader_disconnect_frees_blocked_slots() {
        // Spec §5.2: reader lease-expiry triggers mark_reader_disconnected;
        // slots that were waiting on the dead reader become free.
        let alloc = InMemorySlotAllocator::new(0, 1, 64);
        // Active readers: 0 + 1.
        let active = 0b011;
        let h = alloc.reserve_slot(active).unwrap();
        alloc.commit_slot(h, &[0xAA]).unwrap();

        // Only reader 0 has read, reader 1 has not.
        alloc.mark_read(h, 0).unwrap();
        assert_eq!(alloc.reserve_slot(active), Err(SlotError::NoFreeSlot));

        // Reader 1 disconnected — the slot becomes reservable again.
        alloc.mark_reader_disconnected(1).unwrap();
        let _ = alloc.reserve_slot(active).expect("free after disconnect");
    }
}
