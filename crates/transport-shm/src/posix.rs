// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! POSIX SHM transport.
//!
//! Cross-process shared-memory transport via `shm_open` + `mmap`.
//! Linux primary, macOS supported. Windows compiles (via the
//! `shared_memory` crate, which uses platform-specific
//! `CreateFileMapping`), but our `flock` race protection and
//! `shm_unlink` cleanup are unix-only — a Windows run uses the OS's
//! handle-counted cleanup semantics, which is functionally sufficient
//! but not primarily tested.
//!
//! # Model: SpSc per reader
//!
//! One SHM segment per (writer, reader) pair, not per writer. The
//! writer allocates the segment (`open_owner`), the reader joins
//! (`open_consumer`). Lock-free single-producer-single-consumer ring
//! buffer via `AcqRel` atomics on `head`/`tail`.
//!
//! Rationale:
//! - Lock-free SpSc scales linearly with reader count, no global
//!   contention.
//! - `pthread_mutex` with `PTHREAD_PROCESS_SHARED` would be the
//!   alternative, but it is crash-recovery fragile (the robust flag is
//!   not portable everywhere, abandoned-mutex recovery is complex).
//! - SpmC (single-producer-multi-consumer like iceoryx) blocks the
//!   writer at the slowest reader — bad with heterogeneous readers.
//!
//! Price: N segments with N readers. At 100 readers × 1 MiB default
//! = 100 MiB. Acceptable; the segment size is configurable per pair.
//!
//! # Segment layout
//!
//! ```text
//!   offset 0:   magic: u32 BE   "ZSHM"
//!   offset 4:   version: u32 LE
//!   offset 8:   capacity: u64 LE (data region, without the header)
//!   offset 16:  head: AtomicU64 (next write offset, writer-owned)
//!   offset 24:  tail: AtomicU64 (next read offset, reader-owned)
//!   offset 32:  shutdown: AtomicU32 (0=active, 1=owner-gone)
//!   offset 36:  reserved (padding to the 64-byte cache line)
//!   offset 64:  data region [capacity bytes]
//! ```
//!
//! The shutdown flag is a publication mechanism from owner to
//! consumer: the owner sets it to 1 in `Drop` (release store); the
//! consumer checks it in `wait_for_frame` after each empty poll and
//! returns a targeted error (`Io{message:"shm owner terminated"}`)
//! instead of falling into the normal recv_timeout. Important for
//! cleanup diagnosis (owner crash vs. idle polling).
//!
//! The data format inside the ring region is length-prefixed:
//! `[len: u32 LE][bytes: len]`. Byte position `head % capacity`
//! marks the next free slot; wraparound is allowed but a frame is
//! **not** split — when there is not enough space at the end a padding
//! frame with `len = 0xFFFF_FFFE` is inserted and the writer jumps to
//! the start.
//!
//! # Unsafe scope
//!
//! Two `unsafe` blocks: the two `ptr::read`/`ptr::write` on the mapped
//! memory. Everything else is swapped to `AtomicU64` that may live in
//! shared memory (Rust guarantees that atomic operations are
//! well-defined across process boundaries when both processes map the
//! same address).

use std::io;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use shared_memory::{Shmem, ShmemConf, ShmemError};

#[cfg(unix)]
use std::os::fd::AsRawFd;

/// RAII wrapper for a POSIX advisory flock. The lock is released on
/// drop of the `File` (kernel-side), so holding the file handle is
/// enough.
#[cfg(unix)]
struct FlockGuard {
    #[allow(dead_code)] // hold-only; drop releases the lock
    file: std::fs::File,
}

/// Best-effort exclusive flock. On non-Unix (Windows future) it falls
/// back to a no-op — there the race is layered differently anyway via
/// `CreateFileMapping` semantics.
#[cfg(unix)]
fn acquire_flock_excl(f: &std::fs::File) -> io::Result<()> {
    let fd = f.as_raw_fd();
    // Blocks until the lock is acquired; side-effect-free w.r.t. Rust memory.
    // SAFETY: libc::flock with a valid fd + known constant (LOCK_EX).
    let rc = unsafe { libc::flock(fd, libc::LOCK_EX) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
struct FlockGuard {
    // Held so the OS lock stays until the guard drops (the handle close
    // releases the LockFileEx lock, like the unix flock on fd close).
    #[allow(dead_code)]
    file: std::fs::File,
}

/// Windows equivalent of the unix flock: a blocking exclusive lock on the
/// whole `.lock` file via `LockFileEx`. Serializes the owner-create vs
/// consumer-open race across threads AND processes, and is released
/// automatically when the handle closes (FlockGuard drop) or the process
/// dies — same crash-resilience as `flock(LOCK_EX)`.
#[cfg(windows)]
fn acquire_flock_excl(f: &std::fs::File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{LOCKFILE_EXCLUSIVE_LOCK, LockFileEx};
    use windows_sys::Win32::System::IO::OVERLAPPED;
    let handle = f.as_raw_handle();
    // SAFETY: a zeroed OVERLAPPED requests the lock starting at offset 0.
    let mut ov: OVERLAPPED = unsafe { core::mem::zeroed() };
    // SAFETY: `handle` is valid for the lifetime of the borrowed File; the
    // OVERLAPPED outlives the (blocking) call. Lock the maximal range so the
    // whole file is covered. `LockFileEx` without LOCKFILE_FAIL_IMMEDIATELY
    // blocks until the lock is granted, exactly like flock(LOCK_EX).
    let rc = unsafe {
        LockFileEx(
            handle as _,
            LOCKFILE_EXCLUSIVE_LOCK,
            0,
            u32::MAX,
            u32::MAX,
            &mut ov,
        )
    };
    if rc == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Non-unix, non-Windows std targets have no advisory file lock; the SHM
/// transport is not a real target there, so this is a documented no-op.
#[cfg(not(any(unix, windows)))]
fn acquire_flock_excl(_f: &std::fs::File) -> io::Result<()> {
    Ok(())
}

/// Crash-recovery cleanup for an SHM segment.
///
/// When an owner crashes, without this call the segment remains a
/// zombie in `/dev/shm/<name>` — system-wide, disk/RAM exhaustion on
/// the Nth restart.
///
/// We solve this with
/// (a) a predictable os_id (instead of random) on every open, and
/// (b) `shm_unlink(os_id)` before every owner `create()`.
///
/// `shm_unlink` is idempotent: ENOENT is ignored. The call runs only on
/// `cfg(unix)` and for `#[cfg(any(target_os="linux",
/// target_os="macos"))]` — other Unixes we must add on demand, Windows
/// has its own cleanup semantics.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn shm_unlink_by_os_id(os_id: &str) {
    use std::ffi::CString;
    let Ok(c) = CString::new(os_id) else {
        return;
    };
    // CString::new guards against an inline NUL; the return value does
    // not matter — we treat "segment ran" and "not found" the same.
    // SAFETY: libc::shm_unlink with a valid nul-terminated CString.
    unsafe {
        let _ = libc::shm_unlink(c.as_ptr());
    }
}

/// On non-POSIX targets `shm_unlink` is unavailable. The allocator itself is
/// cross-platform — `shared_memory` maps named segments via `CreateFileMapping`
/// on Windows — so this branch is a real, supported path, not a
/// misconfiguration. There is simply no `shm_unlink` equivalent: Windows reaps a
/// file mapping by reference count once the last handle closes, so explicit
/// crash-recovery unlinking is unnecessary there.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn shm_unlink_by_os_id(_os_id: &str) {
    // No-op: Windows handle-counted cleanup releases the mapping automatically.
}

/// Predictable OS id for an (owner, consumer) pair. Without an os_id
/// shared_memory would generate a random name under which crashed
/// segments cannot be found again. We force a deterministic name so
/// recovery `shm_unlink` works.
fn segment_os_id(owner_id: [u8; 16], consumer_id: [u8; 16]) -> String {
    // `/zerodds-<owner>-<consumer>` — max 65 chars, the Linux limit is
    // NAME_MAX (255) / macOS PSHMNAMLEN (31). We must stay under 31 for
    // macOS portability — not trivial with 2×32 hex. Fallback: on macOS
    // we truncate to the last 15 bytes of each ID, which practically
    // does not shrink the collision space (random GUIDs do not collide
    // in the last 15 bytes).
    #[cfg(target_os = "macos")]
    let (o, c) = (&owner_id[13..16], &consumer_id[13..16]);
    #[cfg(not(target_os = "macos"))]
    let (o, c) = (&owner_id[..], &consumer_id[..]);
    let mut s = String::with_capacity(64);
    s.push_str("/zd-");
    for b in o {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s.push('-');
    for b in c {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

/// Magic prefix for ZeroDDS SHM segments. `ZSHM` in big-endian.
pub const SHM_MAGIC: u32 = u32::from_be_bytes(*b"ZSHM");

/// Segment version. Bump on layout change; openers reject unknown
/// versions.
pub const SHM_VERSION: u32 = 1;

/// Fixed header overhead (magic + version + capacity + head + tail
/// + cache-line padding = 64 bytes).
pub const HEADER_BYTES: usize = 64;

/// Frame length that marks a padding frame (ring end, the writer wraps
/// around afterwards).
const PADDING_FRAME_LEN: u32 = 0xFFFF_FFFE;

/// Default capacity 1 MiB of the data region.
pub const DEFAULT_CAPACITY: usize = 1 << 20;

/// Default spin-wait cycles before the sender comes back with
/// `Backpressure`.
const SPIN_LIMIT: u32 = 1024;

/// Default base directory for OS-backed segment files (Linux:
/// automatically `/dev/shm/` via `shm_open`). This here is only the
/// sentinel directory in which we place a bookkeeping file with the
/// last sender lookup — allowing a consumer to reconstruct the path
/// from a locator without knowing the owner.
pub const DEFAULT_FLINK_DIR: &str = "/tmp/zerodds/shm";

/// Config for `PosixShmTransport`.
#[derive(Debug, Clone)]
pub struct ShmConfig {
    /// Data region size in bytes. Must be `>= max_datagram * 2` (so the
    /// ring can still hold a datagram even with wraparound padding).
    pub capacity: usize,
    /// Base directory for flink files (`shared_memory` places a meta
    /// file there that links the OS segment name).
    pub flink_dir: PathBuf,
    /// Maximum datagram size; `send` rejects larger ones with
    /// `PayloadTooLarge`.
    pub max_datagram: usize,
    /// `recv` timeout; `None` = blocking (then it spins internally with
    /// exponential backoff).
    pub recv_timeout: Option<Duration>,
}

impl Default for ShmConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_CAPACITY,
            flink_dir: PathBuf::from(DEFAULT_FLINK_DIR),
            max_datagram: 64 * 1024,
            recv_timeout: None,
        }
    }
}

/// Error when opening or operating an SHM segment.
#[derive(Debug)]
#[non_exhaustive]
pub enum PosixShmError {
    /// Shared-memory backend error (`shm_open`, `mmap`, ...).
    Shm(ShmemError),
    /// Filesystem error when creating the flink dir or the meta file.
    Io(io::Error),
    /// Segment exists, but the magic prefix/version does not match.
    InvalidHeader,
    /// Config error: `capacity < max_datagram * 2` or similar.
    InvalidConfig {
        /// Reason (static).
        reason: &'static str,
    },
}

impl core::fmt::Display for PosixShmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Shm(e) => write!(f, "shared-memory error: {e}"),
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::InvalidHeader => f.write_str("shm segment has wrong magic or version"),
            Self::InvalidConfig { reason } => write!(f, "shm config invalid: {reason}"),
        }
    }
}

impl std::error::Error for PosixShmError {}

impl From<ShmemError> for PosixShmError {
    fn from(e: ShmemError) -> Self {
        Self::Shm(e)
    }
}

impl From<io::Error> for PosixShmError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Raw pointer into the mapped segment, parked in an `unsafe`-wrapping
/// type so we can hand it around without dragging `unsafe` into every
/// call-site.
struct SegmentLayout {
    /// Base pointer, lifetime-bound to the holding `Shmem`.
    base: *mut u8,
    /// Data region length in bytes (without the 64-byte header).
    capacity: usize,
}

// `SegmentLayout` holds a raw pointer into memory that's mapped for
// the lifetime of the owning `Shmem`.
//
// # SAFETY — happens-before invariants
//
// Data races between the writer (Owner) and reader (Consumer) are
// avoided by the SpSc-ring discipline and `AcqRel`-atomics on
// `head`/`tail`:
//
// - Writer path (`push_frame`):
//   1. Reads `tail` with `Ordering::Acquire`. Any prior `tail`-store
//      by the reader (Release) is now visible — that store happened
//      *before* the reader finished reading previous frames out of
//      the data region. So all bytes up to `tail` are free for reuse.
//   2. Writes frame-length + payload bytes into the data region via
//      plain `ptr::write_unaligned` / `ptr::copy_nonoverlapping`.
//   3. Stores `head` with `Ordering::Release`. Any subsequent reader
//      that sees the new `head` via `Acquire` *also* sees all bytes
//      written in step 2. This is the publish edge.
//
// - Reader path (`pop_frame`) mirrors the above with head/tail swapped.
//
// The raw `ptr::read`/`ptr::write` in `SegmentLayout::{read,write}_*`
// are therefore safe **only when guarded by the ring protocol above**.
// A refactor that touches ordering (e.g. lowering to `Relaxed`,
// introducing multi-producer paths, or adding writes to data bytes
// that `head` does not already publish) invalidates these guarantees
// and must re-prove happens-before by inspection. This is what makes
// the file a safety-critical unsafe island; aarch64 in particular is
// sensitive to weaker orderings and would surface a bug that x86 hides.
// SAFETY: Send is OK because the ptr is only touched behind atomic APIs (see above).
unsafe impl Send for SegmentLayout {}
// SAFETY: Sync is OK because accesses are serialized via AcqRel atomics (see above).
unsafe impl Sync for SegmentLayout {}

// SAFETY: PosixShmTransport is Send because
//  - `Shmem` contains a `*mut u8`; the pointer is valid in the same
//    address space across a thread switch (mmap lifetime bound to the
//    struct instance) and no other thread knows of it.
//  - `SegmentLayout` (the only field that dereferences the pointer)
//    is already Send (see above).
//  - All other fields are inherently Send.
// Drop is not reentrant — the move between threads ends before
// `Drop::drop`.
unsafe impl Send for PosixShmTransport {}
// SAFETY: PosixShmTransport is Sync because
//  - `&self` methods (`send`, `recv`, `local_locator`, `peer_locator`,
//    counter reads) go exclusively through `SegmentLayout` atomics —
//    which are themselves Sync.
//  - SPSC discipline (1 sender, 1 receiver) is the spec contract of the
//    ring buffer; concurrent senders/receivers on the same
//    PosixShmTransport are UB and excluded by the caller (DcpsRuntime
//    wave 4b).
unsafe impl Sync for PosixShmTransport {}

impl SegmentLayout {
    /// # Safety
    /// `base` must point to a valid, mapped region of at least
    /// `HEADER_BYTES + capacity` bytes, and the `AtomicU64` fields
    /// referenced by `head_ptr`/`tail_ptr` must be inside that region.
    unsafe fn new(base: *mut u8, capacity: usize) -> Self {
        Self { base, capacity }
    }

    fn head(&self) -> &AtomicU64 {
        // SAFETY: base + 16 is in the mapped header, AtomicU64 is 8-byte-aligned.
        unsafe { &*(self.base.add(16) as *const AtomicU64) }
    }

    fn tail(&self) -> &AtomicU64 {
        // SAFETY: base + 24 is in the mapped header, AtomicU64 is 8-byte-aligned.
        unsafe { &*(self.base.add(24) as *const AtomicU64) }
    }

    fn shutdown(&self) -> &AtomicU32 {
        // SAFETY: base + 32 is in the mapped header, AtomicU32 is 4-byte-aligned.
        unsafe { &*(self.base.add(32) as *const AtomicU32) }
    }

    fn data_ptr(&self) -> *mut u8 {
        // SAFETY: the HEADER_BYTES offset is the start of the mapped data region.
        unsafe { self.base.add(HEADER_BYTES) }
    }

    /// Read `len` bytes from `offset` (mod capacity). Panics in debug
    /// if the read would wrap the buffer — callers must ensure
    /// single-slot reads.
    fn read_slice(&self, offset: usize, len: usize, out: &mut [u8]) {
        debug_assert!(offset + len <= self.capacity, "read would wrap");
        // SAFETY: offset + len <= capacity (debug-checked), in bounds of the data region.
        unsafe {
            ptr::copy_nonoverlapping(self.data_ptr().add(offset), out.as_mut_ptr(), len);
        }
    }

    fn write_slice(&self, offset: usize, src: &[u8]) {
        debug_assert!(offset + src.len() <= self.capacity, "write would wrap");
        // SAFETY: offset + src.len() <= capacity (debug-checked), in bounds of the data region.
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), self.data_ptr().add(offset), src.len());
        }
    }

    fn write_u32(&self, offset: usize, v: u32) {
        debug_assert!(offset + 4 <= self.capacity);
        // SAFETY: offset + 4 <= capacity (debug-checked); unaligned write is permitted.
        unsafe {
            ptr::write_unaligned(self.data_ptr().add(offset) as *mut u32, v.to_le());
        }
    }

    fn read_u32(&self, offset: usize) -> u32 {
        debug_assert!(offset + 4 <= self.capacity);
        // SAFETY: offset + 4 <= capacity (debug-checked); unaligned read is permitted.
        let v = unsafe { ptr::read_unaligned(self.data_ptr().add(offset) as *const u32) };
        u32::from_le(v)
    }
}

/// Role in the SHM pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShmRole {
    /// This transport allocates the segment (sole writer).
    Owner,
    /// This transport joins an existing segment (sole reader).
    Consumer,
}

/// Cross-process SHM transport via `shm_open` + `mmap`.
///
/// One transport object binds **one** segment in a fixed role. For
/// multi-reader-writer, multiple transports must be instantiated (their
/// own segments).
///
/// # Thread safety
///
/// `Send + Sync` are impl'd manually (see below). Background:
/// `shared_memory::Shmem` contains a `*mut u8` (pointer to the mmap'd
/// segment) that does not get `Send/Sync` automatically. The ring
/// protocol via `SegmentLayout` is already secured by AcqRel atomics
/// and SPSC discipline (see `unsafe impl Send for SegmentLayout`), and
/// all other fields (`role`, locators, `PathBuf`, `String`,
/// `AtomicU64`) are inherently `Send + Sync`. The `Shmem` pointer
/// itself stays valid as long as the `PosixShmTransport` object lives
/// (Drop does `munmap` exactly once). Cross-thread move or cross-thread
/// `&self` access are therefore safe, as long as the caller keeps the
/// SPSC discipline (max one sender thread, max one receiver thread;
/// that is the DCPS wave-4b architecture: hot-path sender in the tick
/// worker, recv worker in the dedicated `recv_user_shm_loop`).
pub struct PosixShmTransport {
    _shmem: Shmem, // keeps the mapping alive; Drop unmaps
    layout: SegmentLayout,
    role: ShmRole,
    local_locator: Locator,
    peer_locator: Locator,
    config: ShmConfig,
    /// For the owner: the flink path we unlink on drop.
    /// The consumer leaves the path alone (the owner owns it).
    flink_path: PathBuf,
    /// For the owner: the `.lock` path cleaned up on drop.
    lock_path: PathBuf,
    /// OS id of the `/dev/shm` segment — we keep a copy so `Drop` can
    /// still call `shm_unlink` even on an internal `shared_memory`
    /// failure (crash-recovery defense-in-depth).
    os_id: String,
    /// Consumed padding frames since bind. See
    /// [`padding_frames_seen`](Self::padding_frames_seen).
    padding_counter: core::sync::atomic::AtomicU64,
    /// Corrupt-frame drops: the owner wrote a length > max_datagram.
    /// The consumer drops the frame + skips to the ring end.
    corrupt_frame_counter: core::sync::atomic::AtomicU64,
}

impl Drop for PosixShmTransport {
    fn drop(&mut self) {
        // Crash-resilient cleanup:
        //
        // - The owner created the `/dev/shm/<os_id>` segment, the flink
        //   file and the `.lock` file. All three must go, otherwise the
        //   system overflows after N restarts.
        // - The consumer does NOT clean up — a consumer drop does not
        //   mean the owner is gone. The owner owns the resources.
        // - All calls are best-effort (with `_ =`), because `Drop` must
        //   not panic and ENOENT (already gone) is okay.
        if self.role == ShmRole::Owner {
            // 0. Set the shutdown flag before we tear down the
            //    resources. A consumer in a wait_for_frame loop sees the
            //    release store and returns a targeted error instead of
            //    running into the normal recv_timeout.
            self.layout
                .shutdown()
                .store(1, core::sync::atomic::Ordering::Release);
            // 1. Remove the flink file (lookup file, not a segment).
            let _ = std::fs::remove_file(&self.flink_path);
            // 2. Remove the lock file.
            let _ = std::fs::remove_file(&self.lock_path);
            // 3. Unlink the /dev/shm segment — defense-in-depth.
            //    `shared_memory` does this internally when set_owner(true)
            //    is set (set in the `open()` path via `.create()`), but
            //    an explicit call is idempotent.
            shm_unlink_by_os_id(&self.os_id);
        }
    }
}

impl core::fmt::Debug for PosixShmTransport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PosixShmTransport")
            .field("role", &self.role)
            .field("local_locator", &self.local_locator)
            .field("peer_locator", &self.peer_locator)
            .field("capacity", &self.layout.capacity)
            .finish()
    }
}

impl PosixShmTransport {
    /// Owner-side instantiation. `local_id` identifies the writer
    /// endpoint, `peer_id` the one allowed reader. The segment is placed
    /// at the path `<flink_dir>/<hex(local)>-<hex(peer)>.shm`.
    ///
    /// # Errors
    /// [`PosixShmError`].
    pub fn open_owner(
        local_id: [u8; 16],
        peer_id: [u8; 16],
        config: ShmConfig,
    ) -> Result<Self, PosixShmError> {
        Self::open(local_id, peer_id, config, ShmRole::Owner)
    }

    /// Consumer-side instantiation. The owner must have created the
    /// segment beforehand — otherwise `Shm(ShmemError::MapOpenFailed)`.
    ///
    /// # Errors
    /// [`PosixShmError`].
    pub fn open_consumer(
        local_id: [u8; 16],
        peer_id: [u8; 16],
        config: ShmConfig,
    ) -> Result<Self, PosixShmError> {
        // Consumer perspective: `peer_id` is the owner/writer,
        // `local_id` our reader endpoint. The segment path is derived
        // from (owner, consumer) — same as for the owner, only the
        // roles are flipped.
        Self::open(peer_id, local_id, config, ShmRole::Consumer)
    }

    fn open(
        owner_id: [u8; 16],
        consumer_id: [u8; 16],
        config: ShmConfig,
        role: ShmRole,
    ) -> Result<Self, PosixShmError> {
        if config.capacity < config.max_datagram * 2 + 16 {
            return Err(PosixShmError::InvalidConfig {
                reason: "capacity must be >= 2 * max_datagram + 16",
            });
        }
        match std::fs::symlink_metadata(&config.flink_dir) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(PosixShmError::InvalidConfig {
                    reason: "flink_dir is a symlink — refused",
                });
            }
            Ok(m) if !m.is_dir() => {
                return Err(PosixShmError::InvalidConfig {
                    reason: "flink_dir exists but is not a directory",
                });
            }
            Ok(_) | Err(_) => { /* create_dir_all deals with missing */ }
        }
        std::fs::create_dir_all(&config.flink_dir)?;
        let flink = segment_flink(&config.flink_dir, owner_id, consumer_id);
        let os_id = segment_os_id(owner_id, consumer_id);

        // **Idempotent open_or_create** (bug fix 2026-05-19):
        // In real DDS systems the SEDP match order is not predictable —
        // the reader side could match + open first, before the writer
        // side has created. Both sides must be able to open the segment
        // regardless of the role parameter.
        //
        // Algorithm:
        // 1. flock excl on the .lock file → serializes both sides
        // 2. Try opening the existing flink → ATTACH (we are not creator)
        // 3. If open fails (ENOENT) → CREATE + initialize the header
        // 4. flock release at the end of the block
        //
        // The role parameter is orthogonal: it only determines send/recv
        // semantics (the owner may send, the consumer may recv), not
        // who creates the segment.
        let lock_path = flink.with_extension("lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)?;
        acquire_flock_excl(&lock_file)?;
        let _guard = FlockGuard { file: lock_file };

        // Try attach to existing segment first.
        let (mut shmem, is_creator) = match ShmemConf::new().flink(&flink).open() {
            Ok(s) => (s, false),
            Err(_) => {
                // Crash recovery: remove a zombie shm file from a
                // previous crashed process.
                shm_unlink_by_os_id(&os_id);
                let _ = std::fs::remove_file(&flink);
                let fresh = ShmemConf::new()
                    .os_id(&os_id)
                    .size(HEADER_BYTES + config.capacity)
                    .flink(&flink)
                    .create()?;
                (fresh, true)
            }
        };

        let base = shmem.as_ptr();
        let total_size = shmem.len();
        if total_size < HEADER_BYTES + config.capacity {
            return Err(PosixShmError::InvalidConfig {
                reason: "mapped segment smaller than requested capacity",
            });
        }

        if is_creator {
            // SAFETY: segment >= HEADER_BYTES, zero-init by create();
            // we are alone in the flock region.
            unsafe {
                ptr::write_unaligned(base as *mut u32, SHM_MAGIC.to_be());
                ptr::write_unaligned(base.add(4) as *mut u32, SHM_VERSION.to_le());
                ptr::write_unaligned(base.add(8) as *mut u64, (config.capacity as u64).to_le());
            }
            // Only the creator is the owner in the shared_memory sense —
            // it unlinks the segment on drop. The attacher must NOT
            // set_owner(true) (double unlink).
            shmem.set_owner(true);
        } else {
            // SAFETY: segment >= HEADER_BYTES bytes, offset 0 holds magic.
            let magic_be = unsafe { ptr::read_unaligned(base as *const u32) };
            if u32::from_be(magic_be) != SHM_MAGIC {
                return Err(PosixShmError::InvalidHeader);
            }
            // SAFETY: segment >= HEADER_BYTES bytes; offset 4 holds the
            // version field directly behind the magic.
            let version = unsafe { ptr::read_unaligned(base.add(4) as *const u32) };
            if u32::from_le(version) != SHM_VERSION {
                return Err(PosixShmError::InvalidHeader);
            }
        }

        // SAFETY: base is valid for the lifetime of shmem; layout holds only refs of the same lifetime.
        let layout = unsafe { SegmentLayout::new(base, config.capacity) };

        let local_locator = Locator::shm(if role == ShmRole::Owner {
            owner_id
        } else {
            consumer_id
        });
        let peer_locator = Locator::shm(if role == ShmRole::Owner {
            consumer_id
        } else {
            owner_id
        });

        let lock_path = flink.with_extension("lock");
        Ok(Self {
            _shmem: shmem,
            layout,
            role,
            local_locator,
            peer_locator,
            config,
            flink_path: flink,
            lock_path,
            os_id,
            padding_counter: core::sync::atomic::AtomicU64::new(0),
            corrupt_frame_counter: core::sync::atomic::AtomicU64::new(0),
        })
    }

    /// Number of bytes in the ring not yet consumed by the reader.
    /// Diagnostic for backpressure monitoring.
    #[must_use]
    pub fn occupied_bytes(&self) -> u64 {
        let h = self.layout.head().load(Ordering::Acquire);
        let t = self.layout.tail().load(Ordering::Acquire);
        h.wrapping_sub(t)
    }

    fn push_frame(&self, data: &[u8]) -> Result<(), SendError> {
        let needed = (data.len() + 4) as u64; // length-prefix
        let cap = self.config.capacity as u64;

        let mut spins: u32 = 0;
        loop {
            let h = self.layout.head().load(Ordering::Relaxed);
            let t = self.layout.tail().load(Ordering::Acquire);
            let used = h.wrapping_sub(t);
            let free = cap - used;

            let h_mod = (h % cap) as usize;
            let tail_space = self.config.capacity - h_mod; // up to ring end

            if needed > free {
                // The reader has not consumed enough.
                spins = spins.saturating_add(1);
                if spins > SPIN_LIMIT {
                    return Err(SendError::Io {
                        message: "shm ring full, reader too slow",
                    });
                }
                core::hint::spin_loop();
                continue;
            }

            if (needed as usize) > tail_space {
                // The fragment would wrap → first write a padding frame,
                // then retry. We need at least 4 bytes for the padding
                // header; if tail_space < 4 it no longer fits, then we
                // set only a sentinel-byte padding via a head bump
                // (readable as "no frame, skip to start").
                //
                // The condition `needed + tail_space <= free` is
                // CORRECT: after the padding `used` rises by
                // `tail_space` (the reader sees the padding marker and
                // consumes it in `pop_frame`; until it does, the bytes
                // are ring occupancy). So before the wrap it must hold
                // that `cap - used >= tail_space + needed`, i.e.
                // `free >= tail_space + needed`.
                if tail_space >= 4 && (needed + (tail_space as u64)) <= free {
                    self.layout.write_u32(h_mod, PADDING_FRAME_LEN);
                    self.layout
                        .head()
                        .store(h + tail_space as u64, Ordering::Release);
                    continue;
                }
                // Not enough space for padding + frame — the reader must
                // consume further first. Spin.
                spins = spins.saturating_add(1);
                if spins > SPIN_LIMIT {
                    return Err(SendError::Io {
                        message: "shm ring full near wraparound",
                    });
                }
                core::hint::spin_loop();
                continue;
            }

            // Happy path: the frame fits in one piece from h_mod.
            let len_u32 = u32::try_from(data.len()).map_err(|_| SendError::PayloadTooLarge {
                size: data.len(),
                limit: self.config.max_datagram,
            })?;
            self.layout.write_u32(h_mod, len_u32);
            self.layout.write_slice(h_mod + 4, data);
            self.layout.head().store(h + needed, Ordering::Release);
            return Ok(());
        }
    }

    /// Number of padding frames observed since bind. Rises every time
    /// the writer had to insert a ring wrap. A conspicuously high rate
    /// is a symptom of too small a `capacity` or too large a message-
    /// size variance — a diagnostic aid, not a safety property.
    ///
    /// The counter is consumer-side (`pop_frame` counts). Owner-side
    /// counting would be duplication — the consumer view is enough for
    /// the diagnostic function (the padding rate is a local phenomenon).
    #[must_use]
    pub fn padding_frames_seen(&self) -> u64 {
        self.padding_counter
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    /// Number of frames the consumer dropped due to the length sanity
    /// check (`len > max_datagram`). Non-zero indicates a broken or
    /// malicious owner — an ops-signal candidate. Atomic, Relaxed
    /// ordering.
    #[must_use]
    pub fn corrupt_frames_seen(&self) -> u64 {
        self.corrupt_frame_counter
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    fn pop_frame(&self) -> Option<Vec<u8>> {
        // Iterative instead of recursion.
        // A ring with continuous padding frames (e.g. a tiny capacity +
        // constant wrap cycles, or a malicious owner that deliberately
        // writes only padding) could drive the original
        // `return self.pop_frame()` recursion to a stack overflow. The
        // loop here terminates in at most `capacity / 4` iterations
        // (each padding skip advances tail by `tail_space` >= 4, until
        // `h == t`).
        loop {
            let t = self.layout.tail().load(Ordering::Relaxed);
            let h = self.layout.head().load(Ordering::Acquire);
            if h == t {
                return None;
            }

            let cap = self.config.capacity as u64;
            let t_mod = (t % cap) as usize;
            let tail_space = self.config.capacity - t_mod;

            // tail_space < 4 means: the writer left a tiny remainder at
            // the ring end (no more length headers fit). Skip to the
            // segment start.
            if tail_space < 4 {
                self.layout
                    .tail()
                    .store(t + tail_space as u64, Ordering::Release);
                continue;
            }

            let len = self.layout.read_u32(t_mod);
            if len == PADDING_FRAME_LEN {
                // Padding — skip the rest of the region. The counter
                // enables diagnosis.
                self.padding_counter
                    .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                self.layout
                    .tail()
                    .store(t + tail_space as u64, Ordering::Release);
                continue;
            }

            // DoS guard:
            // a malicious or corrupt owner could write a `len` larger
            // than the datagram config limit; without a check we would
            // allocate a huge Vec. Drop + bump the counter (visible via
            // `corrupt_frames_seen()`).
            if len as usize > self.config.max_datagram {
                self.corrupt_frame_counter
                    .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                self.layout
                    .tail()
                    .store(t + tail_space as u64, Ordering::Release);
                return None;
            }

            let len = len as usize;
            let mut out = vec![0u8; len];
            self.layout.read_slice(t_mod + 4, len, &mut out);
            self.layout
                .tail()
                .store(t + (len as u64) + 4, Ordering::Release);
            return Some(out);
        }
    }

    fn wait_for_frame(&self) -> Result<Vec<u8>, RecvError> {
        // Sleep-poll model. Instead of a real futex/eventfd notify the
        // reader polls in exponential-backoff steps. The trade-off is
        // deliberate:
        //
        // - **Low tail latency**: max_backoff capped at 1 ms; the worst
        //   case between frame arrival and delivery is one millisecond
        //   instead of the previous 10 ms.
        // - **CPU cost when idle**: sleep(1 ms) in a loop = ~1000
        //   polls/s. An empty receiver on an idle host uses <0.1 % CPU —
        //   acceptable.
        // - **Full futex/eventfd**: deferred to v1.3 (`eventfd(2)` per
        //   segment + epoll_wait). It breaks our cross-platform
        //   abstraction (Linux-only) and only pays off once we hit
        //   1M+ messages/s.
        let deadline = self.config.recv_timeout.map(|t| Instant::now() + t);
        let mut backoff = Duration::from_micros(10);
        let max_backoff = Duration::from_millis(1);
        loop {
            if let Some(frame) = self.pop_frame() {
                return Ok(frame);
            }
            // Targeted owner-gone signal:
            // when the owner drops, it sets shutdown=1. We check only
            // after pop_frame — so frames published before the drop
            // still arrive. The acquire load pairs with the owner's
            // release store.
            if self.layout.shutdown().load(Ordering::Acquire) != 0 {
                return Err(RecvError::Io {
                    message: "shm owner terminated",
                });
            }
            if let Some(d) = deadline {
                if Instant::now() >= d {
                    return Err(RecvError::Timeout);
                }
            }
            std::thread::sleep(backoff);
            backoff = (backoff * 2).min(max_backoff);
        }
    }
}

/// Path of the flink file for an owner/consumer pair. Symmetric in the
/// roles — `(A,B)` and `(B,A)` generate different paths, because the
/// owner role must be unambiguous.
fn segment_flink(base_dir: &Path, owner_id: [u8; 16], consumer_id: [u8; 16]) -> PathBuf {
    let mut s = String::with_capacity(32 + 1 + 32 + 4);
    for b in owner_id {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s.push('-');
    for b in consumer_id {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s.push_str(".shm");
    let mut p = base_dir.to_path_buf();
    p.push(s);
    p
}

impl Transport for PosixShmTransport {
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if self.role != ShmRole::Owner {
            return Err(SendError::Io {
                message: "shm consumer cannot send on this segment",
            });
        }
        if dest.kind != LocatorKind::Shm {
            return Err(SendError::UnsupportedLocator);
        }
        if *dest != self.peer_locator {
            // This transport talks to only **one** peer. For
            // multi-reader the caller must hold multiple transports.
            return Err(SendError::UnsupportedLocator);
        }
        if data.len() > self.config.max_datagram {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: self.config.max_datagram,
            });
        }
        self.push_frame(data)
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        if self.role != ShmRole::Consumer {
            return Err(RecvError::Io {
                message: "shm owner cannot recv on this segment",
            });
        }
        let data = self.wait_for_frame()?;
        Ok(ReceivedDatagram {
            source: self.peer_locator,
            data: std::sync::Arc::from(data.into_boxed_slice()),
        })
    }

    fn local_locator(&self) -> Locator {
        self.local_locator
    }
}

impl PosixShmTransport {
    /// Peer locator (1:1 pair). `send` validates that the target
    /// locator matches it — the DCPS hot path needs this getter to pass
    /// the right target.
    #[must_use]
    pub fn peer_locator(&self) -> Locator {
        self.peer_locator
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn id(n: u8) -> [u8; 16] {
        let mut a = [0u8; 16];
        a[15] = n;
        a
    }

    fn cfg_tmp(base: &Path, cap: usize) -> ShmConfig {
        ShmConfig {
            capacity: cap,
            flink_dir: base.to_path_buf(),
            max_datagram: cap / 2 - 16,
            recv_timeout: Some(Duration::from_secs(2)),
        }
    }

    #[test]
    fn open_owner_then_consumer_roundtrip_single_frame() {
        let tmp = tempfile::tempdir().unwrap();
        let owner = PosixShmTransport::open_owner(id(1), id(2), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(2), id(1), cfg_tmp(tmp.path(), 4096)).unwrap();

        owner.send(&Locator::shm(id(2)), b"hello shm").unwrap();
        let got = consumer.recv().unwrap();
        assert_eq!(&got.data[..], b"hello shm");
        assert_eq!(got.source, Locator::shm(id(1)));
    }

    /// **Architecture requirement (real-life DDS, 2026-05-19)**:
    /// In real systems we do not know the SEDP match order. The reader
    /// side could match + call open_consumer first, BEFORE the writer
    /// side did open_owner. The transport must support this idempotently
    /// — whoever comes first creates the segment, whoever comes later
    /// attaches.
    ///
    /// Today this test shows the **bug** (open_consumer fails on a
    /// non-existent segment). After the fix it goes green.
    #[test]
    fn open_consumer_first_then_owner_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        // Unique ids 220/221: `segment_os_id` is deterministic in
        // (owner_id, consumer_id) and LIVES GLOBALLY in /dev/shm.
        // Earlier this test and `owner_cannot_recv` shared ids 20/21 →
        // under llvm-cov (3-5x slower drop cleanup) the previous segment
        // was still there when the next test started, race → recv error.
        const OWNER: u8 = 220;
        const CONSUMER: u8 = 221;
        // The reader hook runs first → open_consumer without the owner
        // having created the segment yet.
        let consumer =
            PosixShmTransport::open_consumer(id(CONSUMER), id(OWNER), cfg_tmp(tmp.path(), 4096))
                .expect("consumer-first must succeed (idempotent open)");
        // The writer hook later → open_owner attaches to the existing segment.
        let owner =
            PosixShmTransport::open_owner(id(OWNER), id(CONSUMER), cfg_tmp(tmp.path(), 4096))
                .expect("owner-after-consumer must succeed");

        // The sample roundtrip must work.
        owner
            .send(&Locator::shm(id(CONSUMER)), b"consumer-first")
            .unwrap();
        let got = consumer
            .recv()
            .unwrap_or_else(|e| panic!("consumer.recv() failed: {e:?}"));
        assert_eq!(&got.data[..], b"consumer-first");
    }

    /// Concurrent open from two threads — flock must serialize, both
    /// end up validly bound. Today fail-prone due to a race in the
    /// open_owner remove-then-create path.
    #[test]
    fn open_concurrent_two_threads_both_bound() {
        use std::sync::Arc as StdArc;
        // Unique ids 230/231 — see the id-collision note in
        // open_consumer_first_then_owner_roundtrip above.
        const OWNER: u8 = 230;
        const CONSUMER: u8 = 231;
        let tmp = StdArc::new(tempfile::tempdir().unwrap());
        let cfg_path = tmp.path().to_path_buf();
        let cfg = move || cfg_tmp(&cfg_path, 4096);

        let cfg_o = cfg();
        let cfg_c = cfg();
        let h_o = std::thread::spawn(move || {
            PosixShmTransport::open_owner(id(OWNER), id(CONSUMER), cfg_o)
        });
        let h_c = std::thread::spawn(move || {
            PosixShmTransport::open_consumer(id(CONSUMER), id(OWNER), cfg_c)
        });
        let owner = h_o.join().unwrap().expect("owner thread");
        let consumer = h_c.join().unwrap().expect("consumer thread");

        owner
            .send(&Locator::shm(id(CONSUMER)), b"both-bound")
            .unwrap();
        let got = consumer
            .recv()
            .unwrap_or_else(|e| panic!("consumer.recv() failed: {e:?}"));
        assert_eq!(&got.data[..], b"both-bound");
    }

    #[test]
    fn consumer_cannot_send() {
        let tmp = tempfile::tempdir().unwrap();
        let _owner =
            PosixShmTransport::open_owner(id(10), id(11), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(11), id(10), cfg_tmp(tmp.path(), 4096)).unwrap();

        let res = consumer.send(&Locator::shm(id(10)), b"x");
        assert!(matches!(res, Err(SendError::Io { .. })));
    }

    #[test]
    fn owner_cannot_recv() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(20), id(21), cfg_tmp(tmp.path(), 4096)).unwrap();
        let res = owner.recv();
        assert!(matches!(res, Err(RecvError::Io { .. })));
    }

    #[test]
    fn send_rejects_non_shm_locator() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(30), id(31), cfg_tmp(tmp.path(), 4096)).unwrap();
        let res = owner.send(&Locator::udp_v4([127, 0, 0, 1], 7400), b"x");
        assert_eq!(res, Err(SendError::UnsupportedLocator));
    }

    #[test]
    fn send_rejects_wrong_peer() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(40), id(41), cfg_tmp(tmp.path(), 4096)).unwrap();
        // The owner talks only to peer 41, not 99.
        let res = owner.send(&Locator::shm(id(99)), b"x");
        assert_eq!(res, Err(SendError::UnsupportedLocator));
    }

    #[test]
    fn send_rejects_oversize_payload() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(50), id(51), cfg_tmp(tmp.path(), 4096)).unwrap();
        let big = vec![0u8; 4096]; // > max_datagram (4096/2 - 16)
        let res = owner.send(&Locator::shm(id(51)), &big);
        assert!(matches!(res, Err(SendError::PayloadTooLarge { .. })));
    }

    #[test]
    fn recv_times_out_when_idle() {
        let tmp = tempfile::tempdir().unwrap();
        let _owner =
            PosixShmTransport::open_owner(id(60), id(61), cfg_tmp(tmp.path(), 4096)).unwrap();
        let mut cfg = cfg_tmp(tmp.path(), 4096);
        cfg.recv_timeout = Some(Duration::from_millis(200));
        let consumer = PosixShmTransport::open_consumer(id(61), id(60), cfg).unwrap();
        let res = consumer.recv();
        assert_eq!(res, Err(RecvError::Timeout));
    }

    #[test]
    fn many_frames_roundtrip_with_wraparound() {
        // cap=256 with max_datagram=112 -> ~2 frames until wraparound
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 256,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 80,
            recv_timeout: Some(Duration::from_secs(2)),
        };
        let owner = PosixShmTransport::open_owner(id(70), id(71), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(71), id(70), cfg).unwrap();

        for i in 0..20u8 {
            let payload = vec![i; 60];
            owner.send(&Locator::shm(id(71)), &payload).unwrap();
            let got = consumer.recv().unwrap();
            assert_eq!(got.data.len(), 60);
            assert_eq!(got.data[0], i);
        }
    }

    /// Updated contract (2026-05-19): open_consumer is idempotent — on
    /// a missing owner segment the consumer creates it itself (whatever
    /// role order in real-life DDS). The owner comes later and attaches.
    #[test]
    fn consumer_open_succeeds_without_owner_creates_segment() {
        let tmp = tempfile::tempdir().unwrap();
        let res = PosixShmTransport::open_consumer(id(80), id(81), cfg_tmp(tmp.path(), 4096));
        assert!(
            res.is_ok(),
            "consumer-first must create the segment (idempotent open)"
        );
    }

    #[test]
    fn local_locator_is_shm_with_own_id() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(90), id(91), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(owner.local_locator(), Locator::shm(id(90)));
        let consumer =
            PosixShmTransport::open_consumer(id(91), id(90), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(consumer.local_locator(), Locator::shm(id(91)));
    }

    // ---- Coverage-Boost: Error-Matrix ----

    #[test]
    fn invalid_config_capacity_too_small_for_max_datagram() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 100,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 200, // 2 * 200 + 16 = 416 > 100
            recv_timeout: None,
        };
        let res = PosixShmTransport::open_owner(id(100), id(101), cfg);
        assert!(matches!(res, Err(PosixShmError::InvalidConfig { .. })));
    }

    #[test]
    fn occupied_bytes_reflects_send_state() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(110), id(111), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(owner.occupied_bytes(), 0);
        owner.send(&Locator::shm(id(111)), &[0u8; 64]).unwrap();
        // 4 byte length-prefix + 64 byte payload.
        assert_eq!(owner.occupied_bytes(), 68);
    }

    #[test]
    fn recv_after_send_drains_occupied_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(120), id(121), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(121), id(120), cfg_tmp(tmp.path(), 4096)).unwrap();
        owner.send(&Locator::shm(id(121)), b"abc").unwrap();
        let _ = consumer.recv().unwrap();
        assert_eq!(owner.occupied_bytes(), 0);
    }

    #[test]
    fn posix_shm_error_display_covers_all_variants() {
        // Smoke: every variant has a readable Display impl (prevents
        // future panic-in-Display regressions).
        use std::io;
        let variants = [
            PosixShmError::InvalidHeader,
            PosixShmError::InvalidConfig { reason: "x" },
            PosixShmError::Io(io::Error::other("test")),
        ];
        for v in &variants {
            let s = format!("{v}");
            assert!(!s.is_empty(), "empty display for {v:?}");
        }
    }

    #[test]
    fn owner_drop_signals_consumer_via_shutdown_flag() {
        // The consumer must not hang in recv_timeout when the owner has
        // crashed/dropped.
        //
        // `Shmem` is not Send, so we can move neither owner nor consumer
        // across thread boundaries. We simulate the owner drop directly
        // via the shutdown bit instead: a producer thread accesses the
        // shutdown flag via a second consumer join (which maps the same
        // segment).
        //
        // Simpler: synchronous main thread: drop the owner before
        // consumer.recv().
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 4096,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 1024,
            recv_timeout: Some(Duration::from_secs(10)),
        };
        let owner = PosixShmTransport::open_owner(id(140), id(141), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(141), id(140), cfg).unwrap();

        // Drop the owner NOW (sets shutdown=1), then recv — must come
        // back immediately with an owner-gone error instead of waiting.
        // The recv timeout is 10 s; the measured elapsed must be
        // significantly smaller.
        drop(owner);
        let start = std::time::Instant::now();
        let res = consumer.recv();
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(200),
            "recv should shortcut on shutdown, took {elapsed:?}",
        );
        match res {
            Err(RecvError::Io { message }) => {
                assert!(message.contains("owner"), "unexpected msg: {message}");
            }
            other => panic!("expected Io(owner-gone), got {other:?}"),
        }
    }

    #[test]
    fn corrupt_frame_counter_default_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let t = PosixShmTransport::open_owner(id(150), id(151), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(t.corrupt_frames_seen(), 0);
        assert_eq!(t.padding_frames_seen(), 0);
    }

    #[test]
    fn fill_ring_triggers_wraparound_padding() {
        // A small ring + large frames forces wraparound with padding.
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 300,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 100,
            recv_timeout: Some(Duration::from_secs(1)),
        };
        let owner = PosixShmTransport::open_owner(id(130), id(131), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(131), id(130), cfg).unwrap();
        // 5 frames of 60 bytes = 5 * 64 bytes; wraparound needed for ring 300.
        for i in 0..5u8 {
            let p = vec![i; 60];
            owner.send(&Locator::shm(id(131)), &p).unwrap();
            let got = consumer.recv().unwrap();
            assert_eq!(&got.data[..], p.as_slice());
        }
    }
}
