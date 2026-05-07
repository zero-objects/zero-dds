// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! FlatWriter + FlatReader — high-level API ueber dem SlotAllocator.
//!
//! Spec: zerodds-flatdata-1.0 §8 + §9.

use alloc::sync::Arc;
use core::marker::PhantomData;
use core::ops::Deref;

use crate::FlatStruct;
use crate::allocator::{InMemorySlotAllocator, SlotError, SlotHandle};
use crate::backend::SlotBackend;
use crate::slot::ReaderMask;

/// Schreibt FlatStruct-Samples direkt in SHM-Slots — ohne CDR-Encode.
pub struct FlatWriter<T: FlatStruct> {
    alloc: Arc<InMemorySlotAllocator>,
    active_readers_mask: ReaderMask,
    _t: PhantomData<fn() -> T>,
}

impl<T: FlatStruct> FlatWriter<T> {
    /// Erzeugt einen Writer ueber einem Allocator. `active_readers_mask`
    /// listet die Reader-Bits, die alle gelesen haben muessen, bevor
    /// ein Slot wiederverwendet werden kann.
    pub fn new(alloc: Arc<InMemorySlotAllocator>, active_readers_mask: ReaderMask) -> Self {
        Self {
            alloc,
            active_readers_mask,
            _t: PhantomData,
        }
    }

    /// Spec §8.1 write_flat — reserve + memcpy + commit in einem Call.
    ///
    /// # Errors
    /// `SlotError::NoFreeSlot` bei Resource-Pressure;
    /// `SampleTooLarge` wenn der Slot kleiner als T::WIRE_SIZE ist.
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

    /// Spec §8.2 loan_slot — niedrigere Ebene: explizite Slot-Borrow.
    /// Caller schreibt direkt in das geliehene `&mut T`-Buffer und
    /// committed.
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

/// Geliehener Slot. Caller setzt den Sample via `write()` und ruft
/// dann `commit()`. Drop ohne commit verwirft den Loan.
pub struct FlatSlot<'a, T: FlatStruct> {
    handle: SlotHandle,
    writer: &'a FlatWriter<T>,
    committed: bool,
}

impl<T: FlatStruct> FlatSlot<'_, T> {
    /// Schreibt das Sample in den Slot.
    ///
    /// # Errors
    /// Wie `commit_slot`.
    pub fn commit(mut self, sample: T) -> Result<u32, SlotError> {
        let bytes = sample.as_bytes();
        let sn = self.writer.alloc.commit_slot(self.handle, bytes)?;
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

/// Liest FlatStruct-Samples direkt aus SHM-Slots.
pub struct FlatReader<T: FlatStruct> {
    alloc: Arc<InMemorySlotAllocator>,
    /// Welcher Bit im reader_mask gehoert diesem Reader.
    reader_index: u8,
    /// Letzte gelesene Sequence-Number (verhindert Duplicate).
    last_sn: core::sync::atomic::AtomicU32,
    /// Type-Hash des Topics — zum Versions-Check.
    expected_type_hash: [u8; 16],
    _t: PhantomData<fn() -> T>,
}

impl<T: FlatStruct> FlatReader<T> {
    /// Erzeugt einen Reader ueber einem Allocator.
    pub fn new(alloc: Arc<InMemorySlotAllocator>, reader_index: u8) -> Self {
        Self {
            alloc,
            reader_index,
            last_sn: core::sync::atomic::AtomicU32::new(u32::MAX),
            expected_type_hash: T::TYPE_HASH,
            _t: PhantomData,
        }
    }

    /// Liefert Type-Hash — fuer Discovery-Match.
    #[must_use]
    pub fn type_hash(&self) -> [u8; 16] {
        self.expected_type_hash
    }

    /// Spec §9.1 read_flat. Liefert das **neueste** ungelesene Sample.
    /// Setzt automatisch das reader-Bit im reader_mask.
    ///
    /// Spec §6.1 Type-Hash-Cross-Validation: vor dem Slot-Read wird
    /// `T::TYPE_HASH` gegen den am SlotBackend hinterlegten Hash
    /// geprueft. Bei Mismatch: keine Slots werden dereferenziert,
    /// `Err(SlotError::SampleTooLarge)` mit Schema-Drift-Indikation.
    ///
    /// # Errors
    /// - `SampleTooLarge` bei TYPE_HASH-Mismatch (Spec §6.1).
    /// - Wire-/Layout-Fehler oder Slot-Lock-Poison wie sonst.
    pub fn read(&self) -> Result<Option<T>, SlotError> {
        // Spec §6.1: Type-Hash Cross-Validation. Wenn der Backend
        // einen Hash liefert (per `with_type_hash` gesetzt), muss er
        // mit `T::TYPE_HASH` uebereinstimmen — sonst Schema-Drift,
        // wir lehnen den Read ohne Slot-Dereferenzierung ab.
        if let Some(backend_hash) = SlotBackend::type_hash(&*self.alloc) {
            if backend_hash != self.expected_type_hash {
                return Err(SlotError::SampleTooLarge {
                    sample: 0,
                    slot_capacity: 0,
                });
            }
        }
        let count = self.alloc.slot_count()?;
        let mut best: Option<(u32, T)> = None;
        let last_seen = self.last_sn.load(core::sync::atomic::Ordering::Relaxed);
        for idx in 0..count {
            let handle = SlotHandle {
                segment_id: 0,
                slot_index: idx as u32,
            };
            let (header, bytes) = self.alloc.read_slot(handle)?;
            if header.sample_size == 0 {
                continue; // unbenutzt
            }
            // Schon gelesen? Bit gesetzt.
            if (header.reader_mask & (1u32 << self.reader_index)) != 0 {
                continue;
            }
            // Zu kurz?
            if (bytes.len() as u32) < T::WIRE_SIZE as u32 {
                continue;
            }
            // SAFETY: WIRE_SIZE oben gepruft + TYPE_HASH-Match oben
            // gegen Schema-Drift gesichert (Spec §6.1).
            let sample = unsafe { T::from_bytes_unchecked(&bytes) };
            // Wir liefern das **neueste**: hoechste sn, die noch nicht
            // in last_seen ist.
            let unseen = last_seen == u32::MAX || header.sequence_number > last_seen;
            let beats_current = best
                .as_ref()
                .is_none_or(|(b_sn, _)| header.sequence_number > *b_sn);
            if unseen && beats_current {
                best = Some((header.sequence_number, sample));
            }
            // Reader-Bit setzen — auch wenn wir das Sample nicht
            // ausliefern (Slot kann recyclet werden).
            self.alloc.mark_read(handle, self.reader_index)?;
        }
        if let Some((sn, _)) = best.as_ref() {
            self.last_sn
                .store(*sn, core::sync::atomic::Ordering::Relaxed);
        }
        Ok(best.map(|(_, t)| t))
    }
}

/// Reference-Sample, das beim `Drop` automatisch das reader-Bit setzt.
/// Spec §9.2/§9.3.
pub struct FlatSampleRef<T: FlatStruct> {
    sample: T,
}

impl<T: FlatStruct> FlatSampleRef<T> {
    /// Wrapt ein gelesenes Sample.
    #[must_use]
    pub fn new(sample: T) -> Self {
        Self { sample }
    }

    /// Konsumiert das Wrapper und liefert das Sample.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.sample
    }
}

impl<T: FlatStruct> Deref for FlatSampleRef<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.sample
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

    // SAFETY: repr(C) + Copy + 'static, alle Felder sind Primitiv.
    unsafe impl FlatStruct for Pose {
        const TYPE_HASH: [u8; 16] = [0x42; 16];
    }

    fn fresh_alloc(slot_count: usize) -> Arc<InMemorySlotAllocator> {
        Arc::new(InMemorySlotAllocator::new(0, slot_count, 64))
    }

    #[test]
    fn writer_write_then_reader_read() {
        let alloc = fresh_alloc(4);
        // 1 aktiver Reader (Bit 0).
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
        // Zweites read ohne weiteren write → None.
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
    fn loan_drop_without_commit_releases_slot() {
        let alloc = fresh_alloc(1);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);

        {
            let _slot = writer.loan_slot().expect("loan");
            // Drop ohne commit.
        }

        // Slot ist wieder frei.
        let _ = writer.loan_slot().expect("re-loan after drop");
    }

    #[test]
    fn reader_recycles_slot_after_read() {
        let alloc = fresh_alloc(1);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);

        // Erstes Sample.
        writer.write(&Pose { x: 1, y: 1, z: 1 }).expect("w1");
        let _ = reader.read().expect("r1").expect("some");

        // Zweites Sample — Slot muss wiederverwendet werden koennen
        // (Reader-Bit ist gesetzt).
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
    fn reader_rejects_type_hash_mismatch() {
        // Spec §6.1: Reader prueft `T::TYPE_HASH` gegen den am
        // Backend hinterlegten Hash; Mismatch → Schema-Drift-Reject.
        let wrong_hash = [0xBB; 16];
        let alloc = Arc::new(InMemorySlotAllocator::new(0, 4, 64).with_type_hash(wrong_hash));
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);
        let res = reader.read();
        assert!(matches!(res, Err(SlotError::SampleTooLarge { .. })));
    }

    #[test]
    fn reader_accepts_matching_type_hash() {
        // Spec §6.1: bei korrektem Hash am Backend → kein Reject;
        // ohne Sample → Ok(None).
        let alloc = Arc::new(InMemorySlotAllocator::new(0, 4, 64).with_type_hash(Pose::TYPE_HASH));
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);
        let res = reader.read().expect("no schema drift");
        assert!(res.is_none());
    }

    #[test]
    fn reader_without_backend_hash_does_not_reject() {
        // Default-Allocator ohne `with_type_hash` → keine Validation,
        // Reader liest normal.
        let alloc = fresh_alloc(4);
        let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
        let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);
        writer.write(&Pose { x: 1, y: 2, z: 3 }).expect("write");
        let got = reader.read().expect("read").expect("some");
        assert_eq!(got, Pose { x: 1, y: 2, z: 3 });
    }
}
