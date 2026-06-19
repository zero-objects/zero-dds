// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `SlotBackend` trait — abstracts the slot storage backend.
//!
//! ADR-0003: three production backends — in-memory (default for
//! tests + single-process), POSIX shm/mmap (cross-process same-host
//! via the `posix-mmap` feature), Iceoryx2 (optional bridge via the
//! `iceoryx2-bridge` feature). All three implement the same
//! trait, so that `FlatWriter`/`FlatReader` are generic over the
//! backend.

use alloc::vec::Vec;

use crate::allocator::{SlotError, SlotHandle};
use crate::slot::{ReaderMask, SlotHeader};

/// Backend trait for the SHM slot allocator.
///
/// Implementers:
/// - [`crate::InMemorySlotAllocator`] — in-memory backend, default
///   for single-process tests and as the reference implementation.
/// - `PosixSlotAllocator` — POSIX `shm_open` + `mmap`,
///   cross-process same-host (feature `posix-mmap`).
/// - `Iceoryx2SlotAdapter` — Iceoryx2 bridge (optional feature
///   `iceoryx2-bridge`).
pub trait SlotBackend: Send + Sync {
    /// Reserves a free slot. The caller then writes via
    /// `commit_slot` and thereby publishes the sample.
    ///
    /// # Errors
    /// `NoFreeSlot` when all slots are loaned or unfinished.
    fn reserve_slot(&self, active_readers_mask: ReaderMask) -> Result<SlotHandle, SlotError>;

    /// Writes the sample bytes into the slot and sets SlotHeader
    /// `{ sn, sample_size, reader_mask=0 }`. Returns the SN.
    ///
    /// # Errors
    /// `OutOfBounds`, `SampleTooLarge`, or lock poison.
    fn commit_slot(&self, handle: SlotHandle, bytes: &[u8]) -> Result<u32, SlotError>;

    /// Discards a loan without committing. The slot becomes free again.
    ///
    /// # Errors
    /// `OutOfBounds` or lock poison.
    fn discard_slot(&self, handle: SlotHandle) -> Result<(), SlotError>;

    /// True zero-copy loan: returns a writable pointer + capacity into a
    /// *currently reserved* slot's data region, so the caller can fill the
    /// sample **in place** (no staging copy) and then call
    /// [`Self::commit_in_place`]. The pointer is valid until that
    /// `commit_in_place` or a `discard_slot` for the same handle; the slot
    /// must have been obtained from [`Self::reserve_slot`] and not yet
    /// committed/discarded.
    ///
    /// # Errors
    /// `OutOfBounds` if the handle is not a live loan, lock poison, or
    /// `InPlaceUnsupported` for backends without in-place support (default).
    fn slot_data_ptr(&self, handle: SlotHandle) -> Result<(*mut u8, usize), SlotError> {
        let _ = handle;
        Err(SlotError::InPlaceUnsupported)
    }

    /// Finalizes a slot whose `len` data bytes were already written in place
    /// via [`Self::slot_data_ptr`] — sets SlotHeader `{ sn, sample_size=len,
    /// reader_mask=0 }` and releases the loan, with **no** data copy. Returns
    /// the SN. The byte-for-byte result is identical to `commit_slot(handle,
    /// &buf[..len])`, only without the staging copy.
    ///
    /// # Errors
    /// `SampleTooLarge`, `OutOfBounds`, lock poison, or `InPlaceUnsupported`
    /// (default).
    fn commit_in_place(&self, handle: SlotHandle, len: usize) -> Result<u32, SlotError> {
        let _ = (handle, len);
        Err(SlotError::InPlaceUnsupported)
    }

    /// Reads slot header + data bytes (copied).
    ///
    /// # Errors
    /// `OutOfBounds` or lock poison.
    fn read_slot(&self, handle: SlotHandle) -> Result<(SlotHeader, Vec<u8>), SlotError>;

    /// Zero-copy read: returns a **read-only** pointer + sample length into a
    /// *committed* slot's data region — the reader consumes in place without a
    /// copy (counterpart to [`Self::read_slot`], which copies). The pointer is
    /// valid until the slot is recycled (i.e. until every active reader has
    /// `mark_read` it and the writer reserves it again); a same-host reader
    /// reads it, then calls [`Self::mark_read`]. Default: `InPlaceUnsupported`.
    ///
    /// # Errors
    /// `OutOfBounds`, lock poison, or `InPlaceUnsupported`.
    fn slot_read_ptr(&self, handle: SlotHandle) -> Result<(*const u8, usize), SlotError> {
        let _ = handle;
        Err(SlotError::InPlaceUnsupported)
    }

    /// Reader-side scan: returns the handle of the next committed slot
    /// (`sample_size > 0`) that `reader_index` has not yet `mark_read`. Used
    /// with [`Self::slot_read_ptr`] for an untyped zero-copy take, then
    /// [`Self::mark_read`] to release it. Returns `Ok(None)` when nothing new
    /// is pending. Default: `Ok(None)`.
    ///
    /// # Errors
    /// Lock poison.
    fn next_unread_slot(&self, reader_index: u8) -> Result<Option<SlotHandle>, SlotError> {
        let _ = reader_index;
        Ok(None)
    }

    /// Sets the `reader_index` bit in the slot's `reader_mask`
    /// (reader has read).
    ///
    /// # Errors
    /// `OutOfBounds` or lock poison.
    fn mark_read(&self, handle: SlotHandle, reader_index: u8) -> Result<(), SlotError>;

    /// Sets the `reader_index` bit retroactively on all slots
    /// (SPDP lease-expiry).
    ///
    /// # Errors
    /// Lock poison.
    fn mark_reader_disconnected(&self, reader_index: u8) -> Result<(), SlotError>;

    /// Number of configured slots.
    ///
    /// # Errors
    /// Lock poison.
    fn slot_count(&self) -> Result<usize, SlotError>;

    /// Total slot size (header + data + padding); for discovery.
    fn slot_total_size(&self) -> usize;

    /// Data area per slot (without header, without padding).
    fn slot_capacity(&self) -> usize;

    /// Spec §6.1: TYPE_HASH of the topic type, if known to the
    /// backend. `None` = the backend tracks no hash; the caller must
    /// verify another way (e.g. via discovery). Default: None.
    fn type_hash(&self) -> Option<[u8; 16]> {
        None
    }

    /// Spec §4.2 event-driven notify — current change generation. Bumped on
    /// every `commit_slot` (new sample → wake readers) and slot-free
    /// (`mark_read`/`discard_slot`/… → wake writers). Capture this before
    /// checking a condition and pass it to [`Self::wait_for_change`] for a
    /// lost-wakeup-free wait. Default `0` for backends without notify support.
    fn notify_generation(&self) -> u64 {
        0
    }

    /// Spec §4.2 — blocks until the change generation differs from `last` or
    /// `timeout` elapses (event-driven, NO busy-poll). Default: returns
    /// immediately (a backend without notify support degrades to caller-driven
    /// polling). `InMemorySlotAllocator` uses a `Condvar`; `PosixSlotAllocator`
    /// uses a cross-process futex on a shared-memory word (Linux).
    fn wait_for_change(&self, last: u64, timeout: core::time::Duration) {
        let _ = (last, timeout);
    }
}
