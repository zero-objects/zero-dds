// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! FlatWriter + FlatReader — high-level API over the SlotAllocator.
//!
//! Spec: zerodds-flatdata-1.0 §8 + §9.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::Deref;
use core::time::Duration;

/// Reliability policy for [`FlatWriter::write_bp`] under slot pressure
/// (Spec §10.5). `Reliable` blocks (event-driven) until a slot frees or the
/// deadline passes; `BestEffort` drops the sample immediately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reliability {
    /// Block until a slot is free (or timeout) — no sample loss within the
    /// deadline.
    Reliable,
    /// Drop the sample if no slot is free right now.
    BestEffort,
}

/// Outcome of a backpressure-aware write ([`FlatWriter::write_bp`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    /// The sample was written; carries its sequence number.
    Written(u32),
    /// `BestEffort` with no free slot: the sample was dropped.
    Dropped,
    /// `Reliable` timed out waiting for a free slot.
    TimedOut,
}

use crate::FlatStruct;
use crate::allocator::{InMemorySlotAllocator, SlotError, SlotHandle};
use crate::backend::SlotBackend;
use crate::slot::ReaderMask;

/// Writes FlatStruct samples directly into SHM slots — without CDR encoding.
pub struct FlatWriter<T: FlatStruct> {
    alloc: Arc<InMemorySlotAllocator>,
    active_readers_mask: ReaderMask,
    _t: PhantomData<fn() -> T>,
}

impl<T: FlatStruct> FlatWriter<T> {
    /// Creates a writer over an allocator. `active_readers_mask`
    /// lists the reader bits that must all have read before
    /// a slot can be reused.
    pub fn new(alloc: Arc<InMemorySlotAllocator>, active_readers_mask: ReaderMask) -> Self {
        Self {
            alloc,
            active_readers_mask,
            _t: PhantomData,
        }
    }

    /// Spec §8.1 write_flat — reserve + memcpy + commit in a single call.
    ///
    /// # Errors
    /// `SlotError::NoFreeSlot` under resource pressure;
    /// `SampleTooLarge` when the slot is smaller than T::WIRE_SIZE.
    pub fn write(&self, sample: &T) -> Result<u32, SlotError> {
        let bytes = sample.as_bytes();
        let handle = self.alloc.reserve_slot(self.active_readers_mask)?;
        match self.alloc.commit_slot(handle, bytes) {
            Ok(sn) => Ok(sn),
            Err(e) => {
                let _ = self.alloc.discard_slot(handle);
                Err(e)
            }
        }
    }

    /// Spec §10.5 backpressure-aware write. On `NoFreeSlot`: `BestEffort`
    /// drops the sample (`WriteOutcome::Dropped`); `Reliable` **blocks**
    /// event-driven on the allocator's notify until a slot frees or `timeout`
    /// elapses (`WriteOutcome::TimedOut`) — no busy-poll.
    ///
    /// # Errors
    /// Non-`NoFreeSlot` slot errors (e.g. `SampleTooLarge`, lock poison).
    pub fn write_bp(
        &self,
        sample: &T,
        reliability: Reliability,
        timeout: Duration,
    ) -> Result<WriteOutcome, SlotError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match self.write(sample) {
                Ok(sn) => return Ok(WriteOutcome::Written(sn)),
                Err(SlotError::NoFreeSlot) => {}
                Err(e) => return Err(e),
            }
            if reliability == Reliability::BestEffort {
                return Ok(WriteOutcome::Dropped);
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Ok(WriteOutcome::TimedOut);
            }
            // Capture the notify generation, re-try once (a slot may have freed
            // between the failed write and here), then block until a change.
            let g = self.alloc.notify_gen();
            match self.write(sample) {
                Ok(sn) => return Ok(WriteOutcome::Written(sn)),
                Err(SlotError::NoFreeSlot) => {}
                Err(e) => return Err(e),
            }
            self.alloc.wait_for_change(g, deadline - now);
        }
    }

    /// Spec §8.2 loan_slot — lower level: explicit slot borrow.
    /// The caller writes directly into the loaned `&mut T` buffer and
    /// commits.
    ///
    /// # Errors
    /// `NoFreeSlot`.
    pub fn loan_slot(&self) -> Result<FlatSlot<'_, T>, SlotError> {
        let handle = self.alloc.reserve_slot(self.active_readers_mask)?;
        Ok(FlatSlot {
            handle,
            writer: self,
            committed: false,
        })
    }
}

/// Loaned slot. The caller sets the sample via `write()` and then
/// calls `commit()`. Dropping without a commit discards the loan.
pub struct FlatSlot<'a, T: FlatStruct> {
    handle: SlotHandle,
    writer: &'a FlatWriter<T>,
    committed: bool,
}

impl<T: FlatStruct> FlatSlot<'_, T> {
    /// Writes the sample into the slot.
    ///
    /// # Errors
    /// As for `commit_slot`.
    pub fn commit(mut self, sample: T) -> Result<u32, SlotError> {
        let bytes = sample.as_bytes();
        let sn = self.writer.alloc.commit_slot(self.handle, bytes)?;
        self.committed = true;
        Ok(sn)
    }

    /// True zero-copy (Spec §8.2): a `&mut T` view **directly onto the SHM
    /// slot**. The slot's `WIRE_SIZE` bytes are zeroed first (so the all-zero
    /// bit pattern is a valid `T` — `bool` fields are `false`, etc.); the
    /// caller fills the fields in place, then calls [`Self::commit_in_place`].
    /// No staging buffer and no copy on commit.
    ///
    /// # Errors
    /// `NoFreeSlot`/`OutOfBounds`/`SampleTooLarge` from the backend.
    pub fn as_mut(&mut self) -> Result<&mut T, SlotError> {
        let (ptr, cap) = self.writer.alloc.slot_data_ptr(self.handle)?;
        if cap < T::WIRE_SIZE {
            return Err(SlotError::SampleTooLarge {
                sample: T::WIRE_SIZE,
                slot_capacity: cap,
            });
        }
        // SAFETY: the slot is exclusively reserved (loaned) for this `FlatSlot`,
        // `ptr` is the slot data area with `cap >= WIRE_SIZE` bytes, and `T` is a
        // `repr(C)` `Copy` POD (FlatStruct contract). Zeroing makes the all-zero
        // bit pattern a valid `T` before forming the reference.
        unsafe {
            core::ptr::write_bytes(ptr, 0, T::WIRE_SIZE);
            Ok(&mut *ptr.cast::<T>())
        }
    }

    /// Commits a slot whose `T` was written in place via [`Self::as_mut`] —
    /// no copy (counterpart to [`Self::commit`]). Returns the SN.
    ///
    /// # Errors
    /// As for `commit_in_place`.
    pub fn commit_in_place(mut self) -> Result<u32, SlotError> {
        let sn = self
            .writer
            .alloc
            .commit_in_place(self.handle, T::WIRE_SIZE)?;
        self.committed = true;
        Ok(sn)
    }
}

impl<T: FlatStruct> Drop for FlatSlot<'_, T> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.writer.alloc.discard_slot(self.handle);
        }
    }
}

/// Reads FlatStruct samples directly from SHM slots.
pub struct FlatReader<T: FlatStruct> {
    alloc: Arc<InMemorySlotAllocator>,
    /// Which bit in reader_mask belongs to this reader.
    reader_index: u8,
    /// Last read sequence number (prevents duplicates).
    last_sn: core::sync::atomic::AtomicU32,
    /// Type hash of the topic — for the version check.
    expected_type_hash: [u8; 16],
    _t: PhantomData<fn() -> T>,
}

impl<T: FlatStruct> FlatReader<T> {
    /// Creates a reader over an allocator.
    pub fn new(alloc: Arc<InMemorySlotAllocator>, reader_index: u8) -> Self {
        Self {
            alloc,
            reader_index,
            last_sn: core::sync::atomic::AtomicU32::new(u32::MAX),
            expected_type_hash: T::TYPE_HASH,
            _t: PhantomData,
        }
    }

    /// Returns the type hash — for discovery matching.
    #[must_use]
    pub fn type_hash(&self) -> [u8; 16] {
        self.expected_type_hash
    }

    /// Spec §9.1 read_flat. Returns the **newest** unread sample.
    /// Automatically sets this reader's bit in reader_mask.
    ///
    /// Spec §6.1 type-hash cross-validation: before reading a slot,
    /// `T::TYPE_HASH` is checked against the hash stored on the
    /// SlotBackend. On mismatch: no slots are dereferenced,
    /// `Err(SlotError::SampleTooLarge)` with a schema-drift indication.
    ///
    /// # Errors
    /// - `SampleTooLarge` on TYPE_HASH mismatch (Spec §6.1).
    /// - Wire/layout errors or slot-lock poison as usual.
    pub fn read(&self) -> Result<Option<T>, SlotError> {
        // Copy-out variant: the reader bit is set on every scanned slot
        // (incl. the delivered one), so the slot can be recycled immediately.
        Ok(self.scan_best(false)?.map(|(_, _, t)| t))
    }

    /// Spec §9.2/§9.3 read_flat (reference variant). Returns the newest
    /// unread sample wrapped in a [`FlatSampleRef`] whose `Drop` sets this
    /// reader's bit — i.e. the delivered slot stays **un-recyclable** for the
    /// lifetime of the returned reference (zero-copy lifetime binding), and is
    /// released when the reference drops. All other scanned slots are marked
    /// read immediately, as in [`Self::read`].
    ///
    /// # Errors
    /// As [`Self::read`].
    pub fn read_ref(&self) -> Result<Option<FlatSampleRef<T>>, SlotError> {
        match self.scan_best(true)? {
            Some((handle, _, sample)) => {
                let concrete = Arc::clone(&self.alloc);
                let backend: Arc<dyn SlotBackend> = concrete;
                Ok(Some(FlatSampleRef::with_release(
                    sample,
                    backend,
                    handle,
                    self.reader_index,
                )))
            }
            None => Ok(None),
        }
    }

    /// Spec §4.2 event-driven read. Returns the newest unread sample, blocking
    /// on the allocator's notify (NO busy-poll) until one arrives or `timeout`
    /// elapses (then `Ok(None)`). The writer's `commit_slot` bumps the notify
    /// generation, waking this reader.
    ///
    /// # Errors
    /// As [`Self::read`].
    pub fn read_blocking(&self, timeout: Duration) -> Result<Option<T>, SlotError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(sample) = self.read()? {
                return Ok(Some(sample));
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            // Capture gen, re-check (a sample may have landed), then block until
            // a change — lost-wakeup-free.
            let g = self.alloc.notify_gen();
            if let Some(sample) = self.read()? {
                return Ok(Some(sample));
            }
            self.alloc.wait_for_change(g, deadline - now);
        }
    }

    /// Shared scan for [`Self::read`] / [`Self::read_ref`]: finds the newest
    /// unread sample and marks this reader's bit on every scanned unread slot.
    /// When `defer_best` is true the delivered ("best") slot is **not** marked
    /// here — the caller defers that to [`FlatSampleRef`]'s `Drop`.
    fn scan_best(&self, defer_best: bool) -> Result<Option<(SlotHandle, u32, T)>, SlotError> {
        // Spec §6.1: type-hash cross-validation. If the backend provides a
        // hash, it must match `T::TYPE_HASH`; otherwise it is schema drift and
        // we reject the read without dereferencing any slot.
        if let Some(backend_hash) = SlotBackend::type_hash(&*self.alloc) {
            if backend_hash != self.expected_type_hash {
                return Err(SlotError::SampleTooLarge {
                    sample: 0,
                    slot_capacity: 0,
                });
            }
        }
        let count = self.alloc.slot_count()?;
        let last_seen = self.last_sn.load(core::sync::atomic::Ordering::Relaxed);
        let mut best: Option<(SlotHandle, u32, T)> = None;
        // Slots scanned-and-unread; all get the reader bit (except the deferred
        // best one). Collected first so the best is known before marking.
        let mut to_mark: Vec<SlotHandle> = Vec::new();
        for idx in 0..count {
            let handle = SlotHandle {
                segment_id: 0,
                slot_index: idx as u32,
            };
            let (header, bytes) = self.alloc.read_slot(handle)?;
            if header.sample_size == 0 {
                continue; // unused
            }
            if (header.reader_mask & (1u32 << self.reader_index)) != 0 {
                continue; // already read
            }
            if (bytes.len() as u32) < T::WIRE_SIZE as u32 {
                continue; // too short
            }
            // SAFETY: WIRE_SIZE checked above + TYPE_HASH match above guards
            // against schema drift (Spec §6.1).
            let sample = unsafe { T::from_bytes_unchecked(&bytes) };
            to_mark.push(handle);
            let unseen = last_seen == u32::MAX || header.sequence_number > last_seen;
            let beats_current = best
                .as_ref()
                .is_none_or(|(_, b_sn, _)| header.sequence_number > *b_sn);
            if unseen && beats_current {
                best = Some((handle, header.sequence_number, sample));
            }
        }
        let best_handle = best.as_ref().map(|(h, _, _)| *h);
        for handle in to_mark {
            if defer_best && Some(handle) == best_handle {
                continue; // released later by FlatSampleRef::Drop
            }
            self.alloc.mark_read(handle, self.reader_index)?;
        }
        if let Some((_, sn, _)) = best.as_ref() {
            self.last_sn
                .store(*sn, core::sync::atomic::Ordering::Relaxed);
        }
        Ok(best)
    }
}

/// Deferred slot release carried by a [`FlatSampleRef`]: setting this reader's
/// bit on the delivered slot when the reference drops, so the slot stays
/// un-recyclable for the reference's lifetime. Holds the backend as a trait
/// object so the same reference works over the in-memory and the POSIX/DCPS
/// (`Arc<dyn SlotBackend>`) backends alike.
struct DeferredRelease {
    backend: Arc<dyn SlotBackend>,
    handle: SlotHandle,
    reader_index: u8,
}

/// Reference sample that holds its source slot for the duration of its
/// lifetime and sets this reader's bit on `Drop` (releasing the slot for
/// recycling). Spec §9.2/§9.3. Returned by [`FlatReader::read_ref`].
///
/// While a `FlatSampleRef` is alive the writer cannot reuse its slot — that is
/// the zero-copy lifetime guarantee. Dropping it (or calling
/// [`Self::into_inner`]) releases the slot.
pub struct FlatSampleRef<T: FlatStruct> {
    sample: T,
    release: Option<DeferredRelease>,
}

impl<T: FlatStruct> FlatSampleRef<T> {
    /// Wraps a read sample without a deferred release (plain owned copy).
    #[must_use]
    pub fn new(sample: T) -> Self {
        Self {
            sample,
            release: None,
        }
    }

    /// Wraps a sample together with the slot to release on `Drop`. The backend
    /// is taken as `Arc<dyn SlotBackend>` so callers over either backend can
    /// build the reference.
    #[must_use]
    pub(crate) fn with_release(
        sample: T,
        backend: Arc<dyn SlotBackend>,
        handle: SlotHandle,
        reader_index: u8,
    ) -> Self {
        Self {
            sample,
            release: Some(DeferredRelease {
                backend,
                handle,
                reader_index,
            }),
        }
    }

    /// Consumes the wrapper and returns the (copied) sample. The slot is
    /// released here (the wrapper's `Drop` runs as it goes out of scope).
    #[must_use]
    pub fn into_inner(self) -> T {
        // `T: FlatStruct: Copy`, so this copies the value out; `self` is then
        // dropped, running the deferred slot release.
        self.sample
    }
}

impl<T: FlatStruct> Deref for FlatSampleRef<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.sample
    }
}

impl<T: FlatStruct> Drop for FlatSampleRef<T> {
    fn drop(&mut self) {
        if let Some(r) = &self.release {
            // Release the slot: set this reader's bit so it can be recycled.
            let _ = r.backend.mark_read(r.handle, r.reader_index);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    #[repr(C)]
    struct Pose {
        x: i64,
        y: i64,
        z: i64,
    }

    // SAFETY: repr(C) + Copy + 'static, all fields are primitive.
    unsafe impl FlatStruct for Pose {
        const TYPE_HASH: [u8; 16] = [0x42; 16];
    }

    fn fresh_alloc(slot_count: usize) -> Arc<InMemorySlotAllocator> {
        Arc::new(InMemorySlotAllocator::new(0, slot_count, 64))
    }

    #[test]
    fn writer_write_then_reader_read() {
        let alloc = fresh_alloc(4);
        // 1 active reader (bit 0).
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);

        let p = Pose { x: 1, y: 2, z: 3 };
        let _sn = writer.write(&p).expect("write");

        let got = reader.read().expect("read").expect("some");
        assert_eq!(got, p);
    }

    #[test]
    fn reader_does_not_re_read_same_slot() {
        let alloc = fresh_alloc(4);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);

        writer.write(&Pose { x: 1, y: 2, z: 3 }).expect("write");
        let _ = reader.read().expect("first read").expect("some");
        // Second read without another write → None.
        let second = reader.read().expect("second read");
        assert!(second.is_none());
    }

    #[test]
    fn writer_loan_commit_pattern() {
        let alloc = fresh_alloc(2);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);

        let slot = writer.loan_slot().expect("loan");
        let _sn = slot.commit(Pose { x: 7, y: 8, z: 9 }).expect("commit");

        let got = reader.read().expect("read").expect("some");
        assert_eq!(got, Pose { x: 7, y: 8, z: 9 });
    }

    #[test]
    fn writer_loan_in_place_zero_copy() {
        // True zero-copy: fill the `&mut Pose` view directly in the slot,
        // commit_in_place (no staging copy), reader gets the value.
        let alloc = fresh_alloc(2);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);

        let mut slot = writer.loan_slot().expect("loan");
        {
            let p = slot.as_mut().expect("in-place view");
            p.x = 11;
            p.y = 22;
            p.z = 33;
        }
        let _sn = slot.commit_in_place().expect("commit_in_place");

        let got = reader.read().expect("read").expect("some");
        assert_eq!(
            got,
            Pose {
                x: 11,
                y: 22,
                z: 33
            }
        );
    }

    #[test]
    fn loan_drop_without_commit_releases_slot() {
        let alloc = fresh_alloc(1);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);

        {
            let _slot = writer.loan_slot().expect("loan");
            // Drop without commit.
        }

        // Slot is free again.
        let _ = writer.loan_slot().expect("re-loan after drop");
    }

    #[test]
    fn reader_recycles_slot_after_read() {
        let alloc = fresh_alloc(1);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);

        // First sample.
        writer.write(&Pose { x: 1, y: 1, z: 1 }).expect("w1");
        let _ = reader.read().expect("r1").expect("some");

        // Second sample — the slot must be reusable
        // (the reader bit is set).
        writer.write(&Pose { x: 2, y: 2, z: 2 }).expect("w2");
        let got = reader.read().expect("r2").expect("some");
        assert_eq!(got, Pose { x: 2, y: 2, z: 2 });
    }

    #[test]
    fn flat_sample_ref_deref() {
        let p = Pose { x: 1, y: 2, z: 3 };
        let r = FlatSampleRef::new(p);
        assert_eq!(r.x, 1);
        assert_eq!(r.into_inner(), p);
    }

    #[test]
    fn read_ref_holds_slot_until_drop() {
        // Spec §9.2/§9.3: read_ref defers the reader bit to FlatSampleRef::Drop,
        // so a single-slot segment stays un-recyclable while the ref is alive.
        let alloc = fresh_alloc(1);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);

        writer.write(&Pose { x: 1, y: 2, z: 3 }).expect("write");
        let sref = reader.read_ref().expect("read_ref").expect("some");
        assert_eq!(sref.x, 1);
        assert_eq!(sref.z, 3);

        // While the ref lives, the only slot is held → writer cannot reserve.
        assert!(matches!(
            writer.write(&Pose { x: 9, y: 9, z: 9 }),
            Err(SlotError::NoFreeSlot)
        ));

        // Dropping the ref sets the reader bit → slot recyclable.
        drop(sref);
        writer
            .write(&Pose { x: 4, y: 5, z: 6 })
            .expect("write after ref drop");
        let got = reader.read().expect("read").expect("some");
        assert_eq!(got, Pose { x: 4, y: 5, z: 6 });
    }

    #[test]
    fn read_ref_into_inner_releases_slot() {
        // into_inner consumes the ref → its Drop runs → slot released.
        let alloc = fresh_alloc(1);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);

        writer.write(&Pose { x: 1, y: 1, z: 1 }).expect("write");
        let sref = reader.read_ref().expect("read_ref").expect("some");
        let owned = sref.into_inner();
        assert_eq!(owned, Pose { x: 1, y: 1, z: 1 });
        // Slot released by into_inner's Drop.
        writer
            .write(&Pose { x: 2, y: 2, z: 2 })
            .expect("write after into_inner");
    }

    #[test]
    fn reader_rejects_type_hash_mismatch() {
        // Spec §6.1: the reader checks `T::TYPE_HASH` against the
        // hash stored on the backend; a mismatch → schema-drift reject.
        let wrong_hash = [0xBB; 16];
        let alloc = Arc::new(InMemorySlotAllocator::new(0, 4, 64).with_type_hash(wrong_hash));
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);
        let res = reader.read();
        assert!(matches!(res, Err(SlotError::SampleTooLarge { .. })));
    }

    #[test]
    fn reader_accepts_matching_type_hash() {
        // Spec §6.1: with a correct hash on the backend → no reject;
        // with no sample → Ok(None).
        let alloc = Arc::new(InMemorySlotAllocator::new(0, 4, 64).with_type_hash(Pose::TYPE_HASH));
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);
        let res = reader.read().expect("no schema drift");
        assert!(res.is_none());
    }

    #[test]
    fn reader_without_backend_hash_does_not_reject() {
        // Default allocator without `with_type_hash` → no validation,
        // the reader reads normally.
        let alloc = fresh_alloc(4);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);
        writer.write(&Pose { x: 1, y: 2, z: 3 }).expect("write");
        let got = reader.read().expect("read").expect("some");
        assert_eq!(got, Pose { x: 1, y: 2, z: 3 });
    }

    #[test]
    fn read_blocking_wakes_on_commit() {
        // Spec §4.2: read_blocking parks on the notify and is woken by the
        // writer's commit — no busy-poll, returns well before the 5s timeout.
        use std::time::Duration;
        let alloc = fresh_alloc(4);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);

        let w_alloc = Arc::clone(&alloc);
        let h = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            FlatWriter::<Pose>::new(w_alloc, 0b1)
                .write(&Pose { x: 7, y: 8, z: 9 })
                .expect("write");
        });
        let _ = writer; // writer used only to pin the active-mask in scope

        let start = std::time::Instant::now();
        let got = reader
            .read_blocking(Duration::from_secs(5))
            .expect("read_blocking")
            .expect("woken with a sample");
        assert_eq!(got, Pose { x: 7, y: 8, z: 9 });
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "should wake on notify, not spin to timeout"
        );
        h.join().unwrap();
    }

    #[test]
    fn read_blocking_times_out_without_writer() {
        use std::time::Duration;
        let alloc = fresh_alloc(2);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);
        let start = std::time::Instant::now();
        let got = reader.read_blocking(Duration::from_millis(60)).expect("rb");
        assert!(got.is_none());
        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn write_bp_best_effort_drops_when_full() {
        use std::time::Duration;
        let alloc = fresh_alloc(1);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        // Fill the only slot.
        assert!(matches!(
            writer
                .write_bp(
                    &Pose { x: 1, y: 1, z: 1 },
                    Reliability::BestEffort,
                    Duration::ZERO
                )
                .unwrap(),
            WriteOutcome::Written(_)
        ));
        // Next BestEffort write drops (no free slot, no reader).
        assert_eq!(
            writer
                .write_bp(
                    &Pose { x: 2, y: 2, z: 2 },
                    Reliability::BestEffort,
                    Duration::ZERO
                )
                .unwrap(),
            WriteOutcome::Dropped
        );
    }

    #[test]
    fn write_bp_reliable_blocks_until_reader_frees() {
        // Spec §10.5: Reliable parks until a reader consumes the slot.
        use std::time::Duration;
        let alloc = fresh_alloc(1);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);
        writer.write(&Pose { x: 1, y: 1, z: 1 }).expect("fill");

        let r_alloc = Arc::clone(&alloc);
        let h = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let r = FlatReader::<Pose>::new(r_alloc, 0);
            r.read().expect("read").expect("some"); // frees the slot
        });
        let _ = reader;

        let start = std::time::Instant::now();
        let outcome = writer
            .write_bp(
                &Pose { x: 2, y: 2, z: 2 },
                Reliability::Reliable,
                Duration::from_secs(5),
            )
            .expect("write_bp");
        assert!(matches!(outcome, WriteOutcome::Written(_)));
        assert!(start.elapsed() < Duration::from_secs(2));
        h.join().unwrap();
    }

    #[test]
    fn write_bp_reliable_times_out() {
        use std::time::Duration;
        let alloc = fresh_alloc(1);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        writer.write(&Pose { x: 1, y: 1, z: 1 }).expect("fill");
        let start = std::time::Instant::now();
        let outcome = writer
            .write_bp(
                &Pose { x: 2, y: 2, z: 2 },
                Reliability::Reliable,
                Duration::from_millis(60),
            )
            .expect("write_bp");
        assert_eq!(outcome, WriteOutcome::TimedOut);
        assert!(start.elapsed() >= Duration::from_millis(50));
    }
}
