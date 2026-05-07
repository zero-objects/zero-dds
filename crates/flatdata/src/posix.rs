// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `PosixSlotAllocator` — echter Cross-Process-Zero-Copy via POSIX
//! `shm_open` + `mmap` (Spec §4.1, ADR-0003).
//!
//! Layout des Segments:
//!
//! ```text
//!   0x00 | u32 segment_magic (0x5A445353 = "ZDSS")
//!   0x04 | u32 slot_count
//!   0x08 | u32 slot_total_size
//!   0x0c | u32 next_sn (atomic counter)
//!   0x10 | [slot_total_size; slot_count]   ← Slot-Array
//! ```
//!
//! Pro Slot:
//!
//! ```text
//!   0x00 | SlotHeader (16 byte)
//!   0x10 | [u8; capacity] payload
//!   0x?? | padding bis 64-byte-Boundary
//! ```
//!
//! Atomare Operationen: `next_sn` ist `AtomicU32`. Der `SlotHeader`
//! `reader_mask` wird via Compare-and-Swap aktualisiert (siehe
//! `mark_read` Implementation). Slot-`loaned`-Status liegt im Owner-
//! Process im RAM (Mutex), nicht im SHM — Cross-Process-Loaning
//! erforderte einen Lock-Free-Allocator mit atomic-flag-Slot, der
//! ueber Process-Boundaries lauft; das ist explizit nicht im Scope
//! dieses Owner-zentrischen Allocators (Loan-API ist deshalb auf
//! Owner-Process-Caller beschraenkt — Reader-Processes lesen nur
//! committet Samples).

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use std::path::PathBuf;
use std::sync::Mutex;

use shared_memory::{Shmem, ShmemConf, ShmemError};

use crate::allocator::{SlotError, SlotHandle};
use crate::backend::SlotBackend;
use crate::slot::{ReaderMask, SLOT_HEADER_SIZE, SlotHeader};

const SEGMENT_MAGIC: u32 = 0x5A44_5353; // "ZDSS"

/// Fehler beim Aufbau des POSIX-Segments.
#[derive(Debug)]
#[non_exhaustive]
pub enum PosixSlotError {
    /// Shm-Backend-Fehler.
    Shm(ShmemError),
    /// Slot-Capacity zu gross fuer u32.
    CapacityOverflow,
    /// Segment-Header passt nicht (anderer Owner / wrong Magic).
    InvalidHeader,
    /// Internal slot-error (passes through).
    Slot(SlotError),
}

impl core::fmt::Display for PosixSlotError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Shm(e) => write!(f, "shm error: {e}"),
            Self::CapacityOverflow => f.write_str("slot capacity overflows u32"),
            Self::InvalidHeader => f.write_str("segment magic/version mismatch"),
            Self::Slot(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PosixSlotError {}

impl From<ShmemError> for PosixSlotError {
    fn from(e: ShmemError) -> Self {
        Self::Shm(e)
    }
}

impl From<SlotError> for PosixSlotError {
    fn from(e: SlotError) -> Self {
        Self::Slot(e)
    }
}

/// POSIX-mmap Slot-Allocator. Ein Owner-Process erzeugt das Segment;
/// Consumer-Processes attachen via `attach`.
pub struct PosixSlotAllocator {
    /// Shared-memory-Segment. Drop unmappt das Segment.
    /// `None` nur waehrend des Drops.
    shmem: Option<Shmem>,
    /// Pfad zur flink-Datei (zur Reattachment-Discovery).
    flink: PathBuf,
    /// Loan-Tracking pro Slot — lokal im Owner-Process. Loan-API
    /// ist Owner-zentrisch (siehe Modul-Doc); Reader-Processes lesen
    /// nur committet Samples.
    loaned: Mutex<Vec<bool>>,
    /// Slot-Anzahl (zur Bounds-Check, redundant zum Header).
    slot_count: u32,
    /// Slot-Total-Size (Header + Payload + Padding).
    slot_total_size: u32,
    /// Slot-Daten-Capacity (ohne Header, ohne Padding).
    slot_capacity: u32,
}

// SAFETY: Shmem ist nicht Sync per default; wir kontrollieren den
// Zugriff via Mutex<loaned>. Der Header wird via *mut-Pointer modifiziert,
// dafuer ist die Atomic-Disziplin verantwortlich.
unsafe impl Send for PosixSlotAllocator {}
// SAFETY: Read-Pfade nutzen ptr::read(SlotHeader), Write-Pfade nutzen
// AtomicU32 via raw pointer cast (mark_read). loaned ist hinter Mutex.
unsafe impl Sync for PosixSlotAllocator {}

impl PosixSlotAllocator {
    /// Erzeugt ein neues POSIX-SHM-Segment als Owner.
    ///
    /// `flink_path` ist eine Datei im Filesystem (typisch
    /// `/tmp/zerodds/<segment_id>.flink`), die dem Consumer den
    /// realen OS-Segment-Namen verraet.
    ///
    /// # Errors
    /// `Shm` bei `shm_open`-Fehler; `CapacityOverflow` wenn
    /// `slot_capacity > u32::MAX`.
    pub fn create<P: Into<PathBuf>>(
        flink_path: P,
        slot_count: usize,
        slot_capacity: usize,
    ) -> Result<Self, PosixSlotError> {
        let flink_path = flink_path.into();
        if let Some(parent) = flink_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let slot_capacity_u32 =
            u32::try_from(slot_capacity).map_err(|_| PosixSlotError::CapacityOverflow)?;
        let slot_count_u32 =
            u32::try_from(slot_count).map_err(|_| PosixSlotError::CapacityOverflow)?;
        let slot_total_size = align_up(SLOT_HEADER_SIZE + slot_capacity, 64);
        let slot_total_size_u32 =
            u32::try_from(slot_total_size).map_err(|_| PosixSlotError::CapacityOverflow)?;
        let header_size = 0x10usize;
        let total_size = header_size + slot_count * slot_total_size;

        let shmem = ShmemConf::new()
            .size(total_size)
            .flink(&flink_path)
            .create()?;

        // Header initialisieren.
        // SAFETY: as_ptr_mut zeigt auf einen mmap'd Region der Groesse
        // total_size; wir schreiben in die ersten 16 byte den Header.
        unsafe {
            let base = shmem.as_ptr();
            let p = base as *mut u32;
            p.add(0).write(SEGMENT_MAGIC);
            p.add(1).write(slot_count_u32);
            p.add(2).write(slot_total_size_u32);
            p.add(3).write(0); // next_sn = 0
            // Slots zeroen.
            core::ptr::write_bytes(base.add(header_size), 0u8, slot_count * slot_total_size);
        }

        Ok(Self {
            shmem: Some(shmem),
            flink: flink_path,
            loaned: Mutex::new(alloc::vec![false; slot_count]),
            slot_count: slot_count_u32,
            slot_total_size: slot_total_size_u32,
            slot_capacity: slot_capacity_u32,
        })
    }

    /// Attached an ein bestehendes POSIX-SHM-Segment via flink-Pfad.
    /// Der Caller wird Consumer (kein Owner — Drop unmappt nur, nicht
    /// `shm_unlink`).
    ///
    /// # Errors
    /// `Shm` bei attach-Fehler; `InvalidHeader` wenn Magic/Layout
    /// nicht stimmt.
    pub fn attach<P: Into<PathBuf>>(flink_path: P) -> Result<Self, PosixSlotError> {
        let flink_path = flink_path.into();
        let shmem = ShmemConf::new().flink(&flink_path).open()?;

        // Header validieren.
        // SAFETY: shmem.as_ptr ist valide fuer mindestens 16 byte
        // (sonst waere create gescheitert). Wir lesen 4 u32.
        let (magic, slot_count, slot_total_size, _next_sn) = unsafe {
            let p = shmem.as_ptr() as *const u32;
            (
                p.add(0).read(),
                p.add(1).read(),
                p.add(2).read(),
                p.add(3).read(),
            )
        };
        if magic != SEGMENT_MAGIC {
            return Err(PosixSlotError::InvalidHeader);
        }

        let slot_capacity = slot_total_size.saturating_sub(SLOT_HEADER_SIZE as u32);

        Ok(Self {
            shmem: Some(shmem),
            flink: flink_path,
            loaned: Mutex::new(alloc::vec![false; slot_count as usize]),
            slot_count,
            slot_total_size,
            slot_capacity,
        })
    }

    /// Pfad der flink-Datei (fuer Discovery).
    #[must_use]
    pub fn flink_path(&self) -> &str {
        self.flink.to_str().unwrap_or("")
    }

    /// Liefert den Segment-Pfad als String fuer den ShmLocator.
    /// Dies ist das, was im PID_SHM_LOCATOR steht.
    pub fn segment_path(&self) -> String {
        self.flink_path().to_string()
    }

    fn slot_ptr(&self, idx: u32) -> Result<*mut u8, SlotError> {
        if idx >= self.slot_count {
            return Err(SlotError::OutOfBounds);
        }
        let header_size = 0x10usize;
        // SAFETY: caller-Bound geprueft (idx < slot_count); offset bleibt
        // im total_size, der bei create gesichert wurde.
        let shmem = self.shmem.as_ref().ok_or(SlotError::LockPoisoned)?;
        let base = shmem.as_ptr();
        // SAFETY: idx < slot_count (oben gepruft); offset bleibt im
        // total_size, der bei create gesichert wurde (header_size +
        // slot_count * slot_total_size).
        unsafe { Ok(base.add(header_size + (idx as usize) * (self.slot_total_size as usize))) }
    }

    fn read_header(&self, idx: u32) -> Result<SlotHeader, SlotError> {
        let p = self.slot_ptr(idx)?;
        // SAFETY: p zeigt auf SLOT_HEADER_SIZE-Bytes (durch slot_ptr-
        // Bounds garantiert); SlotHeader ist repr(C, align(4)), 16 byte.
        let header = unsafe { core::ptr::read(p as *const SlotHeader) };
        Ok(header)
    }

    fn write_header(&self, idx: u32, header: SlotHeader) -> Result<(), SlotError> {
        let p = self.slot_ptr(idx)?;
        // SAFETY: p ist 4-byte-aligned (Layout-Garantie); 16 byte Schreib-
        // Region; SlotHeader ist repr(C, align(4)).
        unsafe {
            core::ptr::write(p as *mut SlotHeader, header);
        }
        Ok(())
    }

    fn next_sn_inc(&self) -> Result<u32, SlotError> {
        let shmem = self.shmem.as_ref().ok_or(SlotError::LockPoisoned)?;
        // SAFETY: next_sn liegt bei Offset 12 im Header (4. u32). Der
        // Shmem ist mindestens 16 byte. AtomicU32 + ptr::read:
        // wir nutzen direkt den AtomicU32 ueber raw pointer.
        let sn_ptr = unsafe { shmem.as_ptr().add(12) as *const AtomicU32 };
        // SAFETY: sn_ptr zeigt auf 4-byte-aligned u32 im SHM.
        let atomic = unsafe { &*sn_ptr };
        Ok(atomic.fetch_add(1, Ordering::Relaxed))
    }

    fn data_ptr(&self, idx: u32) -> Result<*mut u8, SlotError> {
        let p = self.slot_ptr(idx)?;
        // SAFETY: data folgt direkt nach Header (Offset 16).
        Ok(unsafe { p.add(SLOT_HEADER_SIZE) })
    }
}

impl SlotBackend for PosixSlotAllocator {
    fn reserve_slot(&self, active_readers_mask: ReaderMask) -> Result<SlotHandle, SlotError> {
        let mut loaned = self.loaned.lock().map_err(|_| SlotError::LockPoisoned)?;
        for idx in 0..self.slot_count {
            if loaned[idx as usize] {
                continue;
            }
            let header = self.read_header(idx)?;
            if header.sample_size == 0 || header.all_read(active_readers_mask) {
                loaned[idx as usize] = true;
                return Ok(SlotHandle {
                    segment_id: 0,
                    slot_index: idx,
                });
            }
        }
        Err(SlotError::NoFreeSlot)
    }

    fn commit_slot(&self, handle: SlotHandle, bytes: &[u8]) -> Result<u32, SlotError> {
        if bytes.len() > self.slot_capacity as usize {
            return Err(SlotError::SampleTooLarge {
                sample: bytes.len(),
                slot_capacity: self.slot_capacity as usize,
            });
        }
        let sn = self.next_sn_inc()?;
        let sample_size = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        let header = SlotHeader::new(sn, sample_size);
        // Daten zuerst, Header zuletzt (release-Reihenfolge).
        let dp = self.data_ptr(handle.slot_index)?;
        // SAFETY: dp ist Slot-Daten-Bereich, mindestens slot_capacity Bytes.
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), dp, bytes.len());
        }
        self.write_header(handle.slot_index, header)?;
        // Loan freigeben.
        let mut loaned = self.loaned.lock().map_err(|_| SlotError::LockPoisoned)?;
        loaned[handle.slot_index as usize] = false;
        Ok(sn)
    }

    fn discard_slot(&self, handle: SlotHandle) -> Result<(), SlotError> {
        let mut loaned = self.loaned.lock().map_err(|_| SlotError::LockPoisoned)?;
        if (handle.slot_index as usize) >= loaned.len() {
            return Err(SlotError::OutOfBounds);
        }
        loaned[handle.slot_index as usize] = false;
        Ok(())
    }

    fn read_slot(&self, handle: SlotHandle) -> Result<(SlotHeader, Vec<u8>), SlotError> {
        let header = self.read_header(handle.slot_index)?;
        let n = (header.sample_size as usize).min(self.slot_capacity as usize);
        let dp = self.data_ptr(handle.slot_index)?;
        let mut buf = alloc::vec![0u8; n];
        // SAFETY: dp ist slot_capacity Bytes; n <= slot_capacity.
        unsafe {
            core::ptr::copy_nonoverlapping(dp, buf.as_mut_ptr(), n);
        }
        Ok((header, buf))
    }

    fn mark_read(&self, handle: SlotHandle, reader_index: u8) -> Result<(), SlotError> {
        debug_assert!(reader_index < 32);
        // SAFETY: slot_ptr returnt einen Pointer in den Slot (bounds-
        // checked); Header startet dort. reader_mask ist u32 bei
        // Offset 8 im Header.
        let p = self.slot_ptr(handle.slot_index)?;
        // SAFETY: p ist Slot-Beginn; +8 zeigt auf reader_mask u32.
        let mask_ptr = unsafe { p.add(8) as *const AtomicU32 };
        // SAFETY: mask_ptr zeigt auf u32 im SHM, gültig bis Drop.
        let atomic = unsafe { &*mask_ptr };
        atomic.fetch_or(1u32 << reader_index, Ordering::Relaxed);
        Ok(())
    }

    fn mark_reader_disconnected(&self, reader_index: u8) -> Result<(), SlotError> {
        debug_assert!(reader_index < 32);
        let bit = 1u32 << reader_index;
        for idx in 0..self.slot_count {
            let p = self.slot_ptr(idx)?;
            // SAFETY: reader_mask liegt bei Offset 8 im Header
            // (nach sn:u32 + sample_size:u32). 4-byte aligned per
            // SlotHeader-Layout-Garantie.
            let mask_ptr = unsafe { p.add(8) as *const AtomicU32 };
            // SAFETY: mask_ptr zeigt auf u32 im SHM, gültig bis Drop.
            let atomic = unsafe { &*mask_ptr };
            atomic.fetch_or(bit, Ordering::Relaxed);
        }
        Ok(())
    }

    fn slot_count(&self) -> Result<usize, SlotError> {
        Ok(self.slot_count as usize)
    }

    fn slot_total_size(&self) -> usize {
        self.slot_total_size as usize
    }

    fn slot_capacity(&self) -> usize {
        self.slot_capacity as usize
    }
}

fn align_up(x: usize, n: usize) -> usize {
    debug_assert!(n.is_power_of_two());
    (x + n - 1) & !(n - 1)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU64, Ordering};

    fn unique_flink() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let n = N.fetch_add(1, Ordering::Relaxed);
        let mut p = std::env::temp_dir();
        p.push(alloc::format!("zerodds-flatdata-test-{pid}-{n}"));
        p
    }

    #[test]
    fn create_attach_roundtrip() {
        let flink = unique_flink();
        let owner = PosixSlotAllocator::create(&flink, 4, 64).expect("create");
        let consumer = PosixSlotAllocator::attach(&flink).expect("attach");
        assert_eq!(SlotBackend::slot_count(&owner).unwrap(), 4);
        assert_eq!(SlotBackend::slot_count(&consumer).unwrap(), 4);
        // Slot-Total-Size: 16 + 64 = 80 → padded auf 128.
        assert_eq!(SlotBackend::slot_total_size(&owner), 128);
    }

    #[test]
    fn write_read_through_shm() {
        let flink = unique_flink();
        let owner = PosixSlotAllocator::create(&flink, 4, 64).expect("create");
        let consumer = PosixSlotAllocator::attach(&flink).expect("attach");

        let h = SlotBackend::reserve_slot(&owner, 0b1).expect("reserve");
        let _sn = SlotBackend::commit_slot(&owner, h, &[1, 2, 3, 4]).expect("commit");

        let (header, bytes) = SlotBackend::read_slot(&consumer, h).expect("read");
        assert_eq!(header.sample_size, 4);
        assert_eq!(bytes, vec![1, 2, 3, 4]);
    }

    #[test]
    fn mark_read_visible_to_owner() {
        let flink = unique_flink();
        let owner = PosixSlotAllocator::create(&flink, 1, 64).expect("create");
        let consumer = PosixSlotAllocator::attach(&flink).expect("attach");

        let h = SlotBackend::reserve_slot(&owner, 0b011).expect("reserve");
        SlotBackend::commit_slot(&owner, h, &[0xFF]).expect("commit");

        // Consumer markiert Reader 0 + Reader 1 als gelesen.
        SlotBackend::mark_read(&consumer, h, 0).expect("mark0");
        SlotBackend::mark_read(&consumer, h, 1).expect("mark1");

        // Owner sieht reader_mask = 0b11 → Slot ist frei fuer Reuse.
        let (header, _) = SlotBackend::read_slot(&owner, h).unwrap();
        assert_eq!(header.reader_mask, 0b011);

        // Owner kann Slot wieder reservieren.
        let _ = SlotBackend::reserve_slot(&owner, 0b011).expect("reuse");
    }

    #[test]
    fn next_sn_increments_atomically() {
        let flink = unique_flink();
        let owner = PosixSlotAllocator::create(&flink, 4, 64).expect("create");

        let h0 = SlotBackend::reserve_slot(&owner, 0b1).unwrap();
        let sn0 = SlotBackend::commit_slot(&owner, h0, &[0]).unwrap();
        let h1 = SlotBackend::reserve_slot(&owner, 0b1).unwrap();
        let sn1 = SlotBackend::commit_slot(&owner, h1, &[1]).unwrap();
        assert!(sn1 > sn0);
    }
}
