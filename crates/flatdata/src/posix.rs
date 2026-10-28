// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `PosixSlotAllocator` — real cross-process zero-copy via POSIX
//! `shm_open` + `mmap` (spec §4.1, ADR-0003).
//!
//! Segment layout:
//!
//! ```text
//!   0x00 | u32 segment_magic (0x5A445332 = "ZDS2")
//!   0x04 | u32 slot_count
//!   0x08 | u32 slot_total_size
//!   0x0c | u32 next_sn (atomic counter)
//!   0x10 | u32 notify generation (futex / os_sync word, §4.2)
//!   0x14 | u32 waiters (threads parked on the generation, all processes)
//!   0x18 | padding
//!   0x20 | [slot_total_size; slot_count]   ← Slot-Array
//! ```
//!
//! A bump wakes the parked threads only when `waiters` is not zero, so a
//! write and the read of it cost no syscall while nobody waits. The magic
//! "ZDS2" names this layout. A segment of the layout without the waiter
//! count (magic "ZDSS") is refused: its readers do not count themselves,
//! and a writer that skips the wake would leave them parked until their
//! timeout.
//!
//! Per slot:
//!
//! ```text
//!   0x00 | SlotHeader (16 byte)
//!   0x10 | [u8; capacity] payload
//!   0x?? | padding bis 64-byte-Boundary
//! ```
//!
//! Atomic operations: `next_sn` is an `AtomicU32`. The `SlotHeader`
//! `reader_mask` is updated via compare-and-swap (see the
//! `mark_read` implementation). The slot `loaned` status lives in the
//! owner process's RAM (Mutex), not in the SHM — cross-process loaning
//! would require a lock-free allocator with an atomic-flag slot that
//! works across process boundaries; that is explicitly out of scope for
//! this owner-centric allocator (the loan API is therefore restricted
//! to owner-process callers — reader processes only read
//! committed samples).

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

const SEGMENT_MAGIC: u32 = 0x5A44_5332; // "ZDS2"

/// Segment header size in bytes. Layout: u32 magic, slot_count,
/// slot_total_size, next_sn (offset 0x00..0x0c), then a u32 notify generation
/// at offset 0x10 (Spec §4.2 cross-process futex word) and a u32 waiter count
/// at 0x14, padded to 0x20 so the slot array stays 8-byte aligned.
const SEGMENT_HEADER_SIZE: usize = 0x20;

/// Byte offset of the notify-generation u32 within the segment header.
const GEN_OFFSET: usize = 0x10;

/// Byte offset of the waiter count u32 within the segment header.
const WAITERS_OFFSET: usize = 0x14;

/// Error while setting up the POSIX segment.
#[derive(Debug)]
#[non_exhaustive]
pub enum PosixSlotError {
    /// Shm backend error.
    Shm(ShmemError),
    /// Slot capacity too large for u32.
    CapacityOverflow,
    /// Segment header does not match (different owner / wrong magic).
    InvalidHeader,
    /// Internal slot error (passes through).
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

/// POSIX mmap slot allocator. An owner process creates the segment;
/// consumer processes attach via `attach`.
pub struct PosixSlotAllocator {
    /// Shared-memory segment. Drop unmaps the segment.
    /// `None` only during the drop.
    shmem: Option<Shmem>,
    /// Path to the flink file (for reattachment discovery).
    flink: PathBuf,
    /// Loan tracking per slot — local to the owner process. The loan API
    /// is owner-centric (see module docs); reader processes only read
    /// committed samples.
    loaned: Mutex<Vec<bool>>,
    /// Slot count (for the bounds check, redundant with the header).
    slot_count: u32,
    /// Total slot size (header + payload + padding).
    slot_total_size: u32,
    /// Slot data capacity (without header, without padding).
    slot_capacity: u32,
}

// SAFETY: Shmem is not Sync by default; we control access
// via Mutex<loaned>. The header is modified via a *mut pointer,
// for which the atomic discipline is responsible.
unsafe impl Send for PosixSlotAllocator {}
// SAFETY: read paths use ptr::read(SlotHeader), write paths use
// AtomicU32 via a raw pointer cast (mark_read). loaned is behind a Mutex.
unsafe impl Sync for PosixSlotAllocator {}

impl PosixSlotAllocator {
    /// Creates a new POSIX SHM segment as the owner.
    ///
    /// `flink_path` is a file in the filesystem (typically
    /// `/tmp/zerodds/<segment_id>.flink`) that reveals the
    /// real OS segment name to the consumer.
    ///
    /// # Errors
    /// `Shm` on a `shm_open` error; `CapacityOverflow` when
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
        let header_size = SEGMENT_HEADER_SIZE;
        let total_size = header_size + slot_count * slot_total_size;

        let shmem = ShmemConf::new()
            .size(total_size)
            .flink(&flink_path)
            .create()?;

        // Spec §7.1: owner-only 0600 on both the flink file and the backing
        // shm object. `shared_memory` creates the object with 0600 already
        // (`shm_open` with S_IRUSR | S_IWUSR); the flink file it writes gets
        // the umask default, typically 0644 — world-readable. A peer must be the same
        // uid to attach (cross-host/cross-uid is gated separately); 0600 stops
        // any other local user reading the zero-copy payload.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&flink_path, mode.clone());
            #[cfg(target_os = "linux")]
            {
                // shm_open names map to /dev/shm/<name> on Linux.
                let shm_path = std::path::Path::new("/dev/shm")
                    .join(shmem.get_os_id().trim_start_matches('/'));
                let _ = std::fs::set_permissions(&shm_path, mode);
            }
        }

        // Initialize the header.
        // SAFETY: as_ptr_mut points to an mmap'd region of size
        // total_size; we write the header into the first 16 byte.
        unsafe {
            let base = shmem.as_ptr();
            let p = base as *mut u32;
            p.add(0).write(SEGMENT_MAGIC);
            p.add(1).write(slot_count_u32);
            p.add(2).write(slot_total_size_u32);
            p.add(3).write(0); // next_sn = 0
            p.add(GEN_OFFSET / 4).write(0); // notify generation = 0 (§4.2)
            p.add(WAITERS_OFFSET / 4).write(0); // nobody parked yet
            // Zero the slots.
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

    /// Attaches to an existing POSIX SHM segment via the flink path.
    /// The caller becomes a consumer (not an owner — Drop only unmaps,
    /// it does not `shm_unlink`).
    ///
    /// # Errors
    /// `Shm` on an attach error; `InvalidHeader` when the magic/layout
    /// does not match.
    pub fn attach<P: Into<PathBuf>>(flink_path: P) -> Result<Self, PosixSlotError> {
        let flink_path = flink_path.into();
        let shmem = ShmemConf::new().flink(&flink_path).open()?;

        // Validate the header.
        // SAFETY: shmem.as_ptr is valid for at least 16 byte
        // (otherwise create would have failed). We read 4 u32.
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

    /// Path of the flink file (for discovery).
    #[must_use]
    pub fn flink_path(&self) -> &str {
        self.flink.to_str().unwrap_or("")
    }

    /// Returns the segment path as a string for the ShmLocator.
    /// This is what is stored in the PID_SHM_LOCATOR.
    pub fn segment_path(&self) -> String {
        self.flink_path().to_string()
    }

    /// OS shm id of the backing segment (e.g. for the §7.1 permission check).
    // Currently only exercised by the Linux permission test; kept available for
    // the production §7.1 check (unused in the non-test lib build).
    #[cfg(unix)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn shmem_os_id(&self) -> &str {
        self.shmem
            .as_ref()
            .map_or("", shared_memory::Shmem::get_os_id)
    }

    fn slot_ptr(&self, idx: u32) -> Result<*mut u8, SlotError> {
        if idx >= self.slot_count {
            return Err(SlotError::OutOfBounds);
        }
        let header_size = SEGMENT_HEADER_SIZE;
        // SAFETY: caller bound checked (idx < slot_count); the offset stays
        // within total_size, which was guaranteed at create time.
        let shmem = self.shmem.as_ref().ok_or(SlotError::LockPoisoned)?;
        let base = shmem.as_ptr();
        // SAFETY: idx < slot_count (checked above); the offset stays within
        // total_size, which was guaranteed at create time (header_size +
        // slot_count * slot_total_size).
        unsafe { Ok(base.add(header_size + (idx as usize) * (self.slot_total_size as usize))) }
    }

    fn read_header(&self, idx: u32) -> Result<SlotHeader, SlotError> {
        let p = self.slot_ptr(idx)?;
        // SAFETY: p points to SLOT_HEADER_SIZE bytes (guaranteed by the
        // slot_ptr bounds); SlotHeader is repr(C, align(4)), 16 byte.
        let header = unsafe { core::ptr::read(p as *const SlotHeader) };
        Ok(header)
    }

    fn write_header(&self, idx: u32, header: SlotHeader) -> Result<(), SlotError> {
        let p = self.slot_ptr(idx)?;
        // SAFETY: p is 4-byte-aligned (layout guarantee); 16 byte write
        // region; SlotHeader is repr(C, align(4)).
        unsafe {
            core::ptr::write(p as *mut SlotHeader, header);
        }
        Ok(())
    }

    fn next_sn_inc(&self) -> Result<u32, SlotError> {
        let shmem = self.shmem.as_ref().ok_or(SlotError::LockPoisoned)?;
        // SAFETY: next_sn is at offset 12 in the header (4th u32). The
        // Shmem is at least 16 byte. AtomicU32 + ptr::read:
        // we use the AtomicU32 directly via a raw pointer.
        let sn_ptr = unsafe { shmem.as_ptr().add(12) as *const AtomicU32 };
        // SAFETY: sn_ptr points to a 4-byte-aligned u32 in the SHM.
        let atomic = unsafe { &*sn_ptr };
        Ok(atomic.fetch_add(1, Ordering::Relaxed))
    }

    fn data_ptr(&self, idx: u32) -> Result<*mut u8, SlotError> {
        let p = self.slot_ptr(idx)?;
        // SAFETY: data follows directly after the header (offset 16).
        Ok(unsafe { p.add(SLOT_HEADER_SIZE) })
    }

    /// `&AtomicU32` view of the notify-generation word at [`GEN_OFFSET`] in the
    /// shared segment header. Shared across processes that map the same segment.
    fn gen_atomic(&self) -> Option<&AtomicU32> {
        let shmem = self.shmem.as_ref()?;
        // SAFETY: GEN_OFFSET (0x10) is inside the header (size 0x20), 4-aligned;
        // the segment outlives this borrow (owned by self).
        Some(unsafe { &*(shmem.as_ptr().add(GEN_OFFSET) as *const AtomicU32) })
    }

    /// `&AtomicU32` view of the waiter count at [`WAITERS_OFFSET`].
    fn waiters_atomic(&self) -> Option<&AtomicU32> {
        let shmem = self.shmem.as_ref()?;
        // SAFETY: WAITERS_OFFSET (0x14) is inside the header (size 0x20),
        // 4-aligned; the segment outlives this borrow (owned by self).
        Some(unsafe { &*(shmem.as_ptr().add(WAITERS_OFFSET) as *const AtomicU32) })
    }

    /// Spec §4.2: bump the shared generation and wake any cross-process waiters
    /// (futex on Linux, `os_sync_wake_by_address_all` on macOS from 14.4;
    /// elsewhere readers fall back to the caller-driven poll). The wake is a
    /// syscall, made only when a thread is parked.
    fn bump_notify(&self) {
        if let Some(g) = self.gen_atomic() {
            g.fetch_add(1, Ordering::SeqCst);
            // Pairs with the fence in `wait_for_change`: either the load below
            // sees the waiter's count, or the waiter's park, which compares the
            // generation first, sees this add.
            core::sync::atomic::fence(Ordering::SeqCst);
            let parked = self
                .waiters_atomic()
                .is_some_and(|w| w.load(Ordering::SeqCst) > 0);
            #[cfg(target_os = "linux")]
            if parked {
                futex::wake_all(g);
            }
            #[cfg(target_os = "macos")]
            if parked {
                os_sync::wake_all(g);
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            let _ = parked;
        }
    }
}

/// Linux cross-process futex helpers (Spec §4.2). A futex on the shared
/// generation word lets a reader park in the kernel until the writer wakes it —
/// no busy-poll, no UDP roundtrip. Cross-process (not `FUTEX_PRIVATE`).
#[cfg(target_os = "linux")]
mod futex {
    use core::sync::atomic::AtomicU32;

    pub(super) fn wake_all(word: &AtomicU32) {
        let ptr = core::ptr::from_ref(word).cast::<u32>().cast_mut();
        // SAFETY: ptr is a valid, aligned u32 in shared memory. FUTEX_WAKE with
        // i32::MAX wakes all waiters; extra args are ignored for WAKE.
        unsafe {
            libc::syscall(libc::SYS_futex, ptr, libc::FUTEX_WAKE, i32::MAX, 0, 0, 0);
        }
    }

    /// Parks until `*word != expected` or `timeout` elapses.
    pub(super) fn wait(word: &AtomicU32, expected: u32, timeout: core::time::Duration) {
        let ts = libc::timespec {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_nsec: libc::c_long::from(timeout.subsec_nanos().min(999_999_999) as i32),
        };
        let ptr = core::ptr::from_ref(word).cast::<u32>().cast_mut();
        // SAFETY: ptr is a valid, aligned u32 in shared memory; &ts is valid for
        // the call. FUTEX_WAIT returns immediately if *ptr != expected.
        unsafe {
            libc::syscall(
                libc::SYS_futex,
                ptr,
                libc::FUTEX_WAIT,
                expected as libc::c_int,
                core::ptr::from_ref(&ts),
                0,
                0,
            );
        }
    }
}

/// macOS cross-process wait on the shared generation word (Spec §4.2):
/// `os_sync_wait_on_address` (macOS 14.4) with the SHARED flag parks on a
/// word in memory several processes map, as the Linux futex does. Looked up
/// at run time: a binary that names the symbols does not start on an older
/// macOS; without them the reader keeps the caller-driven poll.
#[cfg(target_os = "macos")]
mod os_sync {
    use core::ffi::c_void;
    use core::sync::atomic::AtomicU32;
    use std::sync::OnceLock;

    type WaitFn = unsafe extern "C" fn(*mut c_void, u64, usize, u32, u32, u64) -> i32;
    type WakeFn = unsafe extern "C" fn(*mut c_void, usize, u32) -> i32;

    fn api() -> Option<(WaitFn, WakeFn)> {
        static API: OnceLock<Option<(WaitFn, WakeFn)>> = OnceLock::new();
        *API.get_or_init(|| {
            // SAFETY: dlsym with NUL-terminated names; a non-null result is the
            // function of that name in <os/os_sync_wait_on_address.h>, whose C
            // signature the two types above repeat.
            unsafe {
                let wait = libc::dlsym(
                    libc::RTLD_DEFAULT,
                    c"os_sync_wait_on_address_with_timeout".as_ptr(),
                );
                let wake = libc::dlsym(libc::RTLD_DEFAULT, c"os_sync_wake_by_address_all".as_ptr());
                if wait.is_null() || wake.is_null() {
                    return None;
                }
                Some((
                    core::mem::transmute::<*mut c_void, WaitFn>(wait),
                    core::mem::transmute::<*mut c_void, WakeFn>(wake),
                ))
            }
        })
    }

    pub(super) fn wake_all(word: &AtomicU32) {
        if let Some((_, wake_by_address)) = api() {
            let ptr = core::ptr::from_ref(word).cast::<c_void>().cast_mut();
            // SAFETY: ptr is a valid, aligned u32 in shared memory.
            unsafe {
                wake_by_address(ptr, 4, libc::OS_SYNC_WAKE_BY_ADDRESS_SHARED);
            }
        }
    }

    /// Parks until `*word != expected`, a wake, or `timeout`.
    pub(super) fn wait(word: &AtomicU32, expected: u32, timeout: core::time::Duration) {
        let Some((wait_on_address, _)) = api() else {
            return;
        };
        let ns = u64::try_from(timeout.as_nanos()).unwrap_or(u64::MAX).max(1);
        let ptr = core::ptr::from_ref(word).cast::<c_void>().cast_mut();
        // SAFETY: ptr is a valid, aligned u32 in shared memory; the call
        // compares it with `expected` before it sleeps.
        unsafe {
            wait_on_address(
                ptr,
                u64::from(expected),
                4,
                libc::OS_SYNC_WAIT_ON_ADDRESS_SHARED,
                libc::OS_CLOCK_MACH_ABSOLUTE_TIME,
                ns,
            );
        }
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
        // Data first, header last (release ordering).
        let dp = self.data_ptr(handle.slot_index)?;
        // SAFETY: dp is the slot data area, at least slot_capacity bytes.
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), dp, bytes.len());
        }
        self.write_header(handle.slot_index, header)?;
        // Release the loan.
        {
            let mut loaned = self.loaned.lock().map_err(|_| SlotError::LockPoisoned)?;
            loaned[handle.slot_index as usize] = false;
        }
        self.bump_notify(); // new sample → wake cross-process readers (§4.2)
        Ok(sn)
    }

    fn discard_slot(&self, handle: SlotHandle) -> Result<(), SlotError> {
        {
            let mut loaned = self.loaned.lock().map_err(|_| SlotError::LockPoisoned)?;
            if (handle.slot_index as usize) >= loaned.len() {
                return Err(SlotError::OutOfBounds);
            }
            loaned[handle.slot_index as usize] = false;
        }
        self.bump_notify();
        Ok(())
    }

    fn slot_data_ptr(&self, handle: SlotHandle) -> Result<(*mut u8, usize), SlotError> {
        {
            let loaned = self.loaned.lock().map_err(|_| SlotError::LockPoisoned)?;
            let idx = handle.slot_index as usize;
            if idx >= loaned.len() || !loaned[idx] {
                return Err(SlotError::OutOfBounds);
            }
        }
        // The slot data lives in the mmap segment at a fixed offset; the
        // pointer is stable for the whole loan.
        let dp = self.data_ptr(handle.slot_index)?;
        Ok((dp, self.slot_capacity as usize))
    }

    fn commit_in_place(&self, handle: SlotHandle, len: usize) -> Result<u32, SlotError> {
        if len > self.slot_capacity as usize {
            return Err(SlotError::SampleTooLarge {
                sample: len,
                slot_capacity: self.slot_capacity as usize,
            });
        }
        let sn = self.next_sn_inc()?;
        let sample_size = u32::try_from(len).unwrap_or(u32::MAX);
        // Data already written in place by the caller — header only (release
        // ordering, identical to commit_slot minus the copy).
        self.write_header(handle.slot_index, SlotHeader::new(sn, sample_size))?;
        {
            let mut loaned = self.loaned.lock().map_err(|_| SlotError::LockPoisoned)?;
            loaned[handle.slot_index as usize] = false;
        }
        self.bump_notify();
        Ok(sn)
    }

    fn slot_read_ptr(&self, handle: SlotHandle) -> Result<(*const u8, usize), SlotError> {
        let header = self.read_header(handle.slot_index)?;
        let n = (header.sample_size as usize).min(self.slot_capacity as usize);
        let dp = self.data_ptr(handle.slot_index)?;
        Ok((dp.cast_const(), n))
    }

    fn next_unread_slot(&self, reader_index: u8) -> Result<Option<SlotHandle>, SlotError> {
        let bit = 1u32 << reader_index;
        for idx in 0..self.slot_count {
            let header = self.read_header(idx)?;
            if header.sample_size > 0 && (header.reader_mask & bit) == 0 {
                return Ok(Some(SlotHandle {
                    segment_id: 0,
                    slot_index: idx,
                }));
            }
        }
        Ok(None)
    }

    fn read_slot(&self, handle: SlotHandle) -> Result<(SlotHeader, Vec<u8>), SlotError> {
        let header = self.read_header(handle.slot_index)?;
        let n = (header.sample_size as usize).min(self.slot_capacity as usize);
        let dp = self.data_ptr(handle.slot_index)?;
        let mut buf = alloc::vec![0u8; n];
        // SAFETY: dp is slot_capacity bytes; n <= slot_capacity.
        unsafe {
            core::ptr::copy_nonoverlapping(dp, buf.as_mut_ptr(), n);
        }
        Ok((header, buf))
    }

    fn mark_read(&self, handle: SlotHandle, reader_index: u8) -> Result<(), SlotError> {
        debug_assert!(reader_index < 32);
        // SAFETY: slot_ptr returns a pointer into the slot (bounds-
        // checked); the header starts there. reader_mask is a u32 at
        // offset 8 in the header.
        let p = self.slot_ptr(handle.slot_index)?;
        // SAFETY: p is the slot start; +8 points to the reader_mask u32.
        let mask_ptr = unsafe { p.add(8) as *const AtomicU32 };
        // SAFETY: mask_ptr points to a u32 in SHM, valid until Drop.
        let atomic = unsafe { &*mask_ptr };
        atomic.fetch_or(1u32 << reader_index, Ordering::Relaxed);
        self.bump_notify(); // slot may have freed → wake cross-process writers
        Ok(())
    }

    fn mark_reader_disconnected(&self, reader_index: u8) -> Result<(), SlotError> {
        debug_assert!(reader_index < 32);
        let bit = 1u32 << reader_index;
        for idx in 0..self.slot_count {
            let p = self.slot_ptr(idx)?;
            // SAFETY: reader_mask is at offset 8 in the header
            // (after sn:u32 + sample_size:u32). 4-byte aligned per the
            // SlotHeader layout guarantee.
            let mask_ptr = unsafe { p.add(8) as *const AtomicU32 };
            // SAFETY: mask_ptr points to a u32 in SHM, valid until Drop.
            let atomic = unsafe { &*mask_ptr };
            atomic.fetch_or(bit, Ordering::Relaxed);
        }
        self.bump_notify();
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

    fn notify_generation(&self) -> u64 {
        self.gen_atomic()
            .map_or(0, |g| u64::from(g.load(Ordering::Acquire)))
    }

    fn wait_for_change(&self, last: u64, timeout: core::time::Duration) {
        // Cross-process park on the shared generation word: futex on Linux,
        // `os_sync_wait_on_address` on macOS. Elsewhere the reader falls back
        // to the caller-driven poll (no event-driven wait).
        // The waiter count goes up before the park and down after it, so a
        // bump in between wakes; see `bump_notify` for the pairing fence.
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if let (Some(g), Some(w)) = (self.gen_atomic(), self.waiters_atomic()) {
            w.fetch_add(1, Ordering::SeqCst);
            core::sync::atomic::fence(Ordering::SeqCst);
            #[cfg(target_os = "linux")]
            futex::wait(g, last as u32, timeout);
            #[cfg(target_os = "macos")]
            os_sync::wait(g, last as u32, timeout);
            w.fetch_sub(1, Ordering::SeqCst);
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let _ = (last, timeout);
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
        // Slot total size: 16 + 64 = 80 → padded to 128.
        assert_eq!(SlotBackend::slot_total_size(&owner), 128);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn futex_notify_wakes_consumer_across_mappings() {
        // Spec §4.2: the consumer parks on a cross-process futex on the shared
        // generation word; the owner's commit wakes it. Owner (create) and
        // consumer (attach) map the SAME segment at different virtual addresses
        // but the same physical page — exactly the cross-process case.
        use alloc::sync::Arc;
        use core::time::Duration;
        let flink = unique_flink();
        let owner = Arc::new(PosixSlotAllocator::create(&flink, 4, 64).expect("create"));
        let consumer = PosixSlotAllocator::attach(&flink).expect("attach");

        let w = Arc::clone(&owner);
        let h = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let handle = w.reserve_slot(0b1).expect("reserve");
            w.commit_slot(handle, &[1, 2, 3, 4]).expect("commit"); // bumps + futex_wake
        });

        let start = std::time::Instant::now();
        let g0 = consumer.notify_generation();
        consumer.wait_for_change(g0, Duration::from_secs(5)); // futex_wait
        let woke = start.elapsed();
        assert!(
            woke < Duration::from_secs(2),
            "consumer should wake on the owner's futex_wake, not spin to timeout (waited {woke:?})"
        );
        assert!(
            consumer.notify_generation() != g0,
            "generation must have advanced"
        );
        h.join().unwrap();
    }

    /// No wake is lost between a waiter counting itself and its park: owner
    /// and consumer hand 2 000 samples through 4 slots, each parking on the
    /// shared generation whenever it has nothing to do. A lost wake leaves
    /// both parked until the 5 s timeout.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn every_bump_reaches_a_parked_peer_and_the_count_returns_to_zero() {
        use alloc::sync::Arc;
        use core::time::Duration;
        use std::time::Instant;
        const ROUNDS: usize = 2_000;
        let flink = unique_flink();
        let owner = PosixSlotAllocator::create(&flink, 4, 64).expect("create");
        let consumer = Arc::new(PosixSlotAllocator::attach(&flink).expect("attach"));

        let c = Arc::clone(&consumer);
        let reader = std::thread::spawn(move || {
            let mut slowest = Duration::ZERO;
            for _ in 0..ROUNDS {
                loop {
                    let seen = c.notify_generation();
                    if let Some(h) = c.next_unread_slot(0).expect("scan") {
                        c.mark_read(h, 0).expect("mark");
                        break;
                    }
                    let start = Instant::now();
                    c.wait_for_change(seen, Duration::from_secs(5));
                    slowest = slowest.max(start.elapsed());
                }
            }
            slowest
        });
        let mut slowest = Duration::ZERO;
        for _ in 0..ROUNDS {
            loop {
                let seen = owner.notify_generation();
                match owner.reserve_slot(0b1) {
                    Err(SlotError::NoFreeSlot) => {
                        let start = Instant::now();
                        owner.wait_for_change(seen, Duration::from_secs(5));
                        slowest = slowest.max(start.elapsed());
                    }
                    reserved => {
                        let h = reserved.expect("reserve");
                        owner.commit_slot(h, &[1, 2, 3, 4]).expect("commit");
                        break;
                    }
                }
            }
        }
        let slowest = slowest.max(reader.join().expect("reader"));
        assert!(
            slowest < Duration::from_secs(2),
            "a park lasted {slowest:?}: a wake was lost"
        );
        let waiters = owner.waiters_atomic().expect("header");
        assert_eq!(waiters.load(Ordering::SeqCst), 0);
    }

    /// A segment of the layout without the waiter count (magic "ZDSS") is
    /// refused: its readers would park without counting themselves.
    #[test]
    fn a_segment_without_the_waiter_count_is_refused() {
        let flink = unique_flink();
        let owner = PosixSlotAllocator::create(&flink, 4, 64).expect("create");
        let base = owner.shmem.as_ref().expect("mapping").as_ptr();
        // SAFETY: the header is SEGMENT_HEADER_SIZE bytes at the start of the
        // owner's mapping; the magic is its first u32.
        unsafe { base.cast::<u32>().write(0x5A44_5353) };
        assert!(matches!(
            PosixSlotAllocator::attach(&flink),
            Err(PosixSlotError::InvalidHeader)
        ));
    }

    /// Spec §7.1, portable half: the flink file that names the segment must
    /// be owner-only on every unix. The `/dev/shm` object carries the same
    /// mode but only exists on Linux — that half is
    /// `segment_is_owner_only_0600`.
    #[cfg(unix)]
    #[test]
    fn flink_file_is_owner_only_0600() {
        use std::os::unix::fs::PermissionsExt;
        let flink = unique_flink();
        let _owner = PosixSlotAllocator::create(&flink, 4, 64).expect("create");
        let mode = std::fs::metadata(&flink)
            .expect("flink stat")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "flink file must be 0600, was {mode:o}");
    }

    /// Spec §7.1 on macOS and the BSDs: the shm object has no path there,
    /// so its mode is read off a descriptor of it.
    #[cfg(all(unix, not(target_os = "linux")))]
    #[test]
    fn shm_object_is_owner_only_0600_by_descriptor() {
        let flink = unique_flink();
        let owner = PosixSlotAllocator::create(&flink, 4, 64).expect("create");
        let name = std::ffi::CString::new(owner.shmem_os_id()).unwrap();
        // SAFETY: a NUL-terminated name; read-only open of an existing object.
        let fd = unsafe { libc::shm_open(name.as_ptr(), libc::O_RDONLY) };
        assert!(fd >= 0, "shm_open: {}", std::io::Error::last_os_error());
        // SAFETY: an all-zero stat is a valid out-parameter; fd is open.
        let mut st: libc::stat = unsafe { core::mem::zeroed() };
        // SAFETY: fd is a valid descriptor and st a valid out-parameter.
        let rc = unsafe { libc::fstat(fd, &mut st) };
        // SAFETY: fd came from shm_open above and is closed once.
        unsafe { libc::close(fd) };
        assert_eq!(rc, 0, "fstat: {}", std::io::Error::last_os_error());
        let mode = u32::from(st.st_mode) & 0o777;
        assert_eq!(mode, 0o600, "shm object must be 0600, was {mode:o}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn segment_is_owner_only_0600() {
        // Spec §7.1: the flink file and the /dev/shm object must be 0600
        // (owner-only), not world-readable. Linux-only (shm path is /dev/shm).
        use std::os::unix::fs::PermissionsExt;
        let flink = unique_flink();
        let owner = PosixSlotAllocator::create(&flink, 4, 64).expect("create");
        let flink_mode = std::fs::metadata(&flink)
            .expect("flink stat")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            flink_mode, 0o600,
            "flink file must be 0600, was {flink_mode:o}"
        );
        let shm_path =
            std::path::Path::new("/dev/shm").join(owner.shmem_os_id().trim_start_matches('/'));
        let shm_mode = std::fs::metadata(&shm_path)
            .expect("shm stat")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(shm_mode, 0o600, "shm object must be 0600, was {shm_mode:o}");
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

        // Consumer marks reader 0 + reader 1 as read.
        SlotBackend::mark_read(&consumer, h, 0).expect("mark0");
        SlotBackend::mark_read(&consumer, h, 1).expect("mark1");

        // The owner sees reader_mask = 0b11 → the slot is free for reuse.
        let (header, _) = SlotBackend::read_slot(&owner, h).unwrap();
        assert_eq!(header.reader_mask, 0b011);

        // The owner can reserve the slot again.
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
