// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! POSIX SHM transport.
//!
//! Cross-process shared-memory transport via `shm_open` + `mmap`.
//! Linux primary, macOS supported. Windows compiles (via the
//! `shared_memory` crate, which uses platform-specific
//! `CreateFileMapping`; Unix maps through `crate::mapping`, which keeps
//! no descriptor per mapping), but our `flock` race protection and
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
//!   offset 36:  consumer_attached: AtomicU32 (consumers that have it open)
//!   offset 40:  owner_epoch: AtomicU32 (bumped by every owner that opens it)
//!   offset 44:  reserved (padding to the 64-byte cache line)
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
//! the start. A rest of 1 to 3 bytes holds no padding header; writer and
//! reader both skip it by its size.
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

use crate::mapping::{Mapping, ShmemError};

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

/// Takes the exclusive lock on a pair's `.lock` file. An owner's drop and
/// [`sweep_stale_pair_files`] remove that file; one who opened it before
/// and waited for the lock then holds the lock of a file that is gone,
/// while the next opener makes a new one and locks that. So after the lock
/// the path has to name the locked file still, or it is opened again.
fn lock_pair(lock_path: &Path) -> io::Result<FlockGuard> {
    loop {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(lock_path)?;
        acquire_flock_excl(&file)?;
        if still_named(&file, lock_path)? {
            return Ok(FlockGuard { file });
        }
    }
}

/// Whether `path` names the open `file` still.
#[cfg(unix)]
fn still_named(file: &std::fs::File, path: &Path) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let held = file.metadata()?;
    match std::fs::metadata(path) {
        Ok(named) => Ok(named.dev() == held.dev() && named.ino() == held.ino()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Not compared on Windows: `std` gives no file identity to compare the
/// name with. An owner's drop removing the lock file while an opener
/// waits on it is not covered there.
#[cfg(not(unix))]
fn still_named(_file: &std::fs::File, _path: &Path) -> io::Result<bool> {
    Ok(true)
}

/// Removes what pairs leave in `flink_dir` once their segments are gone:
/// `.lock` files without a flink beside them, and on Linux flinks whose
/// segment no longer exists in `/dev/shm` (its sweep unlinks segments, not
/// flinks). An owner's drop removed both; a process that ended without one
/// (killed), a consumer, and a superseded owner left them, and they piled
/// up (a test chain on a MacBook Air: 3 504 `.lock` files in one domain's directory). A
/// file younger than `min_age` and one whose pair lock someone holds stay.
///
/// # Errors
/// Reading `flink_dir` failed; a file that cannot be opened is skipped.
#[cfg(unix)]
pub fn sweep_stale_pair_files(flink_dir: &Path, min_age: Duration) -> io::Result<SweepReport> {
    let mut report = SweepReport::default();
    let now = std::time::SystemTime::now();
    let old = |path: &Path, report: &mut SweepReport| {
        let changed = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(now);
        let old = now.duration_since(changed).unwrap_or_default() >= min_age;
        if !old {
            report.young += 1;
        }
        old
    };
    let entries = match std::fs::read_dir(flink_dir) {
        Ok(e) => e
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect::<Vec<_>>(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(report),
        Err(e) => return Err(e),
    };
    #[cfg(target_os = "linux")]
    for flink in entries
        .iter()
        .filter(|p| p.extension().is_some_and(|x| x == "shm"))
    {
        let Ok(os_id) = std::fs::read_to_string(flink) else {
            continue;
        };
        if !os_id.starts_with("/zd-")
            || Path::new(DEV_SHM).join(&os_id[1..]).exists()
            || !old(flink, &mut report)
        {
            continue;
        }
        // Under the pair's lock: an opener that makes the segment holds it
        // between writing the flink and taking its hold.
        let Some(_pair) = try_lock_pair(&flink.with_extension("lock")) else {
            report.held += 1;
            continue;
        };
        if !Path::new(DEV_SHM).join(&os_id[1..]).exists() {
            let _ = std::fs::remove_file(flink);
            report.removed += 1;
        }
    }
    for lock in entries
        .iter()
        .filter(|p| p.extension().is_some_and(|x| x == "lock"))
    {
        if lock.with_extension("shm").exists() || !old(lock, &mut report) {
            continue;
        }
        let Some(pair) = try_lock_pair(lock) else {
            report.held += 1;
            continue;
        };
        // Removed under its own lock: a waiting opener finds the path gone
        // and opens it anew (see `lock_pair`).
        let _ = std::fs::remove_file(lock);
        drop(pair);
        report.removed += 1;
    }
    Ok(report)
}

/// The pair lock at `lock_path` if nobody holds it, without waiting.
#[cfg(unix)]
fn try_lock_pair(lock_path: &Path) -> Option<FlockGuard> {
    use std::os::unix::fs::MetadataExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path)
        .ok()?;
    // SAFETY: flock on the valid fd of `file`.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return None;
    }
    let held = file.metadata().ok()?;
    let named = std::fs::metadata(lock_path).ok()?;
    (named.dev() == held.dev() && named.ino() == held.ino()).then_some(FlockGuard { file })
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
/// misconfiguration. There is no `shm_unlink` equivalent: `shared_memory`
/// keeps a Windows mapping on a backing file that outlives every handle, and
/// removing it here could split a pair another process still uses. An
/// orphaned one goes in [`discard_orphaned_segment`].
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn shm_unlink_by_os_id(_os_id: &str) {}

/// Predictable OS id for an (owner, consumer) pair. Without an os_id
/// shared_memory would generate a random name under which crashed
/// segments cannot be found again. We force a deterministic name so
/// recovery `shm_unlink` works.
fn segment_os_id(owner_id: [u8; 16], consumer_id: [u8; 16]) -> String {
    segment_os_id_in(owner_id, consumer_id, cfg!(target_os = "macos"))
}

/// The name `/zd-<owner>-<consumer>` in hex, 69 characters; Linux allows
/// `NAME_MAX` (255).
///
/// `short`: the name has to fit macOS' `PSHMNAMLEN` (31), too short for
/// the two ids in hex. It then carries the first 13 bytes (104 bits) of
/// an FNV-1a-128 hash over both ids, 30 characters with the prefix.
/// Before, it carried the last three bytes of each id, which are the
/// entity id: every process numbers its endpoints from the same start, so
/// two pairs of different processes got one name, and a consumer read the
/// frames of the other pair (measured on macOS, 2026-10-02). The hash is
/// fixed, not the std hasher, since both sides of a pair may be built
/// with different toolchains and must arrive at the same name.
fn segment_os_id_in(owner_id: [u8; 16], consumer_id: [u8; 16], short: bool) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(70);
    s.push_str("/zd-");
    if short {
        let mut both = [0u8; 32];
        both[..16].copy_from_slice(&owner_id);
        both[16..].copy_from_slice(&consumer_id);
        for b in &fnv1a_128(&both)[..13] {
            let _ = write!(s, "{b:02x}");
        }
        return s;
    }
    for b in owner_id {
        let _ = write!(s, "{b:02x}");
    }
    s.push('-');
    for b in consumer_id {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// FNV-1a with the 128-bit parameters, big-endian result.
fn fnv1a_128(data: &[u8]) -> [u8; 16] {
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013B;
    let mut h: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for &b in data {
        h ^= u128::from(b);
        h = h.wrapping_mul(PRIME);
    }
    h.to_be_bytes()
}

/// Where Linux shows the POSIX shared-memory objects: as tmpfs files
/// named like the `os_id` without its leading `/`.
#[cfg(target_os = "linux")]
const DEV_SHM: &str = "/dev/shm";

/// Name prefix of every ZeroDDS segment in `/dev/shm` (see
/// `segment_os_id`); what [`sweep_orphaned_segments`] looks at.
#[cfg(target_os = "linux")]
pub const SEGMENT_NAME_PREFIX: &str = "zd-";

/// What [`hold_segment`] found.
#[cfg_attr(not(unix), allow(dead_code))]
enum SegmentHold {
    /// In use from here on; the file carries this process's shared lock
    /// on the segment (`None` where segments take no lock).
    Held(Option<std::fs::File>),
    /// Attached, but no process holds the segment: everyone who used it
    /// ended without unlinking it, and its ring holds their frames.
    Orphaned,
    /// The name went between mapping the segment and locking it: a
    /// sweeper removed it as orphaned.
    Vanished,
}

/// Takes this process's shared `flock` on the segment `os_id`, the mark
/// a later attacher and [`sweep_orphaned_segments`] read as "in use".
/// The kernel releases it when the process ends, by `SIGKILL` too, so a
/// segment whose every user is gone carries no lock at all. Runs inside
/// the pair's `.lock` region: no other opener of the pair runs between a
/// create and this call.
///
/// `created`: this process has just made the segment, nothing to probe.
#[cfg(target_os = "linux")]
fn hold_segment(os_id: &str, _flink: &Path, created: bool) -> io::Result<SegmentHold> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let path = Path::new(DEV_SHM).join(os_id.trim_start_matches('/'));
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(SegmentHold::Vanished),
        Err(e) => return Err(e),
    };
    let fd = file.as_raw_fd();
    if !created {
        // SAFETY: flock on the valid fd of `file`.
        if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(SegmentHold::Orphaned);
        }
        let e = io::Error::last_os_error();
        if e.raw_os_error() != Some(libc::EWOULDBLOCK) {
            return Err(e);
        }
    }
    // Held shared by a user, or exclusively by a sweeper for as long as
    // it takes to unlink it: wait that out.
    // SAFETY: flock on the valid fd of `file`.
    if unsafe { libc::flock(fd, libc::LOCK_SH) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let held = file.metadata()?;
    match std::fs::symlink_metadata(&path) {
        Ok(named) if named.dev() == held.dev() && named.ino() == held.ino() => {
            Ok(SegmentHold::Held(Some(file)))
        }
        Ok(_) => Ok(SegmentHold::Vanished),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(SegmentHold::Vanished),
        Err(e) => Err(e),
    }
}

/// Whether a process holds the segment `os_id` (see [`hold_segment`]).
#[cfg(target_os = "linux")]
fn segment_in_use(os_id: &str) -> bool {
    use std::os::unix::fs::OpenOptionsExt;
    let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(Path::new(DEV_SHM).join(os_id.trim_start_matches('/')))
    else {
        return false;
    };
    // SAFETY: flock on the valid fd of `file`; closing it releases a lock
    // this probe took.
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) != 0 }
}

#[cfg(not(target_os = "linux"))]
fn segment_in_use(_os_id: &str) -> bool {
    false
}

/// macOS (and the other non-Linux Unixes) keeps POSIX shared-memory
/// objects out of the file system, so there is no segment file to lock.
/// The lock goes on the pair's flink instead, the file every user opens
/// the segment through: same protocol as the Linux one, and
/// [`sweep_orphaned_flinks`] reads it.
#[cfg(all(unix, not(target_os = "linux")))]
fn hold_segment(_os_id: &str, flink: &Path, created: bool) -> io::Result<SegmentHold> {
    use std::os::unix::fs::MetadataExt;
    let file = match std::fs::File::open(flink) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(SegmentHold::Vanished),
        Err(e) => return Err(e),
    };
    let fd = file.as_raw_fd();
    if !created {
        // SAFETY: flock on the valid fd of `file`.
        if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(SegmentHold::Orphaned);
        }
        let e = io::Error::last_os_error();
        if e.raw_os_error() != Some(libc::EWOULDBLOCK) {
            return Err(e);
        }
    }
    // SAFETY: flock on the valid fd of `file`.
    if unsafe { libc::flock(fd, libc::LOCK_SH) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let held = file.metadata()?;
    match std::fs::symlink_metadata(flink) {
        Ok(named) if named.dev() == held.dev() && named.ino() == held.ino() => {
            Ok(SegmentHold::Held(Some(file)))
        }
        Ok(_) => Ok(SegmentHold::Vanished),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(SegmentHold::Vanished),
        Err(e) => Err(e),
    }
}

/// Windows keeps a mapping on a backing file
/// (`%TEMP%\shared_memory-rs\<name>`, `shared_memory`'s `new_map`) that
/// outlives the process: an owner that ended without its drop left its
/// ring and its frames for the next opener (nightly `windows-smoke`,
/// `an_orphaned_segment_is_made_anew_not_attached`: 29 bytes of the ended
/// session). The lock is a `LockFileEx` on the pair's flink, released with
/// the handle or the process: the macOS protocol.
#[cfg(windows)]
fn hold_segment(_os_id: &str, flink: &Path, created: bool) -> io::Result<SegmentHold> {
    let file = match std::fs::File::open(flink) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(SegmentHold::Vanished),
        Err(e) => return Err(e),
    };
    if !created && windows_lock::try_exclusive(&file)? {
        return Ok(SegmentHold::Orphaned);
    }
    windows_lock::shared(&file)?;
    Ok(SegmentHold::Held(Some(file)))
}

#[cfg(all(not(unix), not(windows)))]
fn hold_segment(_os_id: &str, _flink: &Path, _created: bool) -> io::Result<SegmentHold> {
    Ok(SegmentHold::Held(None))
}

/// `LockFileEx` over a whole file, as `flock` on Unix.
#[cfg(windows)]
mod windows_lock {
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
    use windows_sys::Win32::Storage::FileSystem::{
        LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;

    fn lock(file: &std::fs::File, flags: u32) -> io::Result<bool> {
        // SAFETY: a zeroed OVERLAPPED requests the range from offset 0.
        let mut ov: OVERLAPPED = unsafe { core::mem::zeroed() };
        // SAFETY: the handle is valid while `file` is borrowed; the
        // OVERLAPPED outlives the call, which does not run asynchronously
        // on a handle opened without FILE_FLAG_OVERLAPPED.
        let ok = unsafe {
            LockFileEx(
                file.as_raw_handle() as _,
                flags,
                0,
                u32::MAX,
                u32::MAX,
                &mut ov,
            )
        };
        if ok != 0 {
            return Ok(true);
        }
        let e = io::Error::last_os_error();
        if e.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
            return Ok(false);
        }
        Err(e)
    }

    /// The exclusive lock if nobody holds the file, without waiting.
    pub(super) fn try_exclusive(file: &std::fs::File) -> io::Result<bool> {
        lock(file, LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY)
    }

    /// A shared lock, waiting out an exclusive one.
    pub(super) fn shared(file: &std::fs::File) -> io::Result<()> {
        lock(file, 0).map(|_| ())
    }
}

/// Removes an orphaned segment: its name on Unix, and on Windows the
/// backing file `shared_memory` keeps it on, which nothing else removes.
/// Only for a segment no process holds (see [`hold_segment`]).
fn discard_orphaned_segment(os_id: &str) {
    shm_unlink_by_os_id(os_id);
    #[cfg(windows)]
    {
        let mut backing = std::env::temp_dir();
        backing.push("shared_memory-rs");
        backing.push(os_id.trim_start_matches('/'));
        let _ = std::fs::remove_file(backing);
    }
}

/// What [`sweep_orphaned_segments`] found, segment by segment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SweepReport {
    /// Orphaned segments unlinked.
    pub removed: usize,
    /// Segments a process holds.
    pub held: usize,
    /// Segments younger than the minimum age, left alone.
    pub young: usize,
    /// Segments under the prefix whose header is not this layout
    /// version, left alone: an older ZeroDDS takes no lock, so its live
    /// segments would look orphaned.
    pub foreign: usize,
}

impl SweepReport {
    /// Both reports' counts added up.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self {
            removed: self.removed + other.removed,
            held: self.held + other.held,
            young: self.young + other.young,
            foreign: self.foreign + other.foreign,
        }
    }
}

/// Unlinks the ZeroDDS segments in `/dev/shm` no process holds: those of
/// processes that ended without dropping their transport (killed,
/// crashed, or exited with the participant still open). The memory of a
/// tmpfs object stays allocated until its name goes, so each of them kept
/// its full capacity until the next reboot.
///
/// `min_age` leaves alone what changed more recently, which covers the
/// moment between a segment's creation and its creator's lock (see
/// `hold_segment`).
///
/// # Errors
/// Reading `/dev/shm` failed; a segment that cannot be opened or locked
/// is skipped.
#[cfg(target_os = "linux")]
pub fn sweep_orphaned_segments(min_age: Duration) -> io::Result<SweepReport> {
    let current = sweep_segments(SEGMENT_NAME_PREFIX, min_age, ring_header_is_current)?;
    Ok(current.and(sweep_old_layout_segments(SEGMENT_NAME_PREFIX, min_age)?))
}

/// The rings of an earlier layout version under `prefix`: they take no
/// lock, so [`sweep_segments`] counts them `foreign` and leaves them. One
/// is unlinked when no process maps it — `/proc/<pid>/maps` names
/// `/dev/shm/<name>` for each mapping. A process whose maps this one
/// cannot read keeps them all when it is root's (root can open a 0600
/// segment of ours); another user's cannot.
///
/// Measured before this: 256 segments of 2 MiB of the earlier pair layout
/// stayed in `/dev/shm` of a host after its processes ended, 512 MiB of
/// memory until the next reboot.
#[cfg(target_os = "linux")]
fn sweep_old_layout_segments(prefix: &str, min_age: Duration) -> io::Result<SweepReport> {
    use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
    let mut report = SweepReport::default();
    let now = std::time::SystemTime::now();
    let mut old = Vec::new();
    for entry in std::fs::read_dir(DEV_SHM)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(prefix) {
            continue;
        }
        let Ok(file) = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(entry.path())
        else {
            continue;
        };
        let Ok(meta) = file.metadata() else { continue };
        let changed = std::time::UNIX_EPOCH
            + Duration::new(
                u64::try_from(meta.ctime()).unwrap_or(0),
                u32::try_from(meta.ctime_nsec()).unwrap_or(0),
            );
        if !meta.is_file() || now.duration_since(changed).unwrap_or_default() < min_age {
            continue;
        }
        let mut header = [0u8; 8];
        if file.read_exact_at(&mut header, 0).is_ok()
            && header[..4] == SHM_MAGIC.to_be_bytes()
            && header[4..] != SHM_VERSION.to_le_bytes()
        {
            old.push(name.to_owned());
        }
    }
    if old.is_empty() {
        return Ok(report);
    }
    let Some(mapped) = dev_shm_mappings() else {
        report.held += old.len();
        return Ok(report);
    };
    for name in old {
        if mapped.contains(&name) {
            report.held += 1;
        } else {
            shm_unlink_by_os_id(&format!("/{name}"));
            report.removed += 1;
        }
    }
    Ok(report)
}

/// The names under `/dev/shm` some process maps, from every
/// `/proc/<pid>/maps`; `None` when root's maps cannot be read.
#[cfg(target_os = "linux")]
fn dev_shm_mappings() -> Option<std::collections::BTreeSet<String>> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let own_uid = unsafe { libc::geteuid() };
    let mut mapped = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir("/proc").ok()? {
        let Ok(entry) = entry else { continue };
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
        {
            continue;
        }
        let maps = match std::fs::read_to_string(entry.path().join("maps")) {
            Ok(maps) => maps,
            // A process that ended since the listing has nothing mapped.
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => {
                let owner = entry.metadata().map(|m| m.uid()).unwrap_or(0);
                if owner == 0 && own_uid != 0 {
                    return None;
                }
                continue;
            }
        };
        for line in maps.lines() {
            if let Some(at) = line.find("/dev/shm/") {
                let name = line[at + "/dev/shm/".len()..].trim_end_matches(" (deleted)");
                mapped.insert(name.to_owned());
            }
        }
    }
    Some(mapped)
}

/// A ring of this layout version, which takes the lock, or a ring whose
/// creator ended between making it and writing its header: all zeros, a
/// segment no attacher accepts (`InvalidHeader`), so nothing can hold it.
#[cfg(target_os = "linux")]
fn ring_header_is_current(header: &[u8; 8]) -> bool {
    *header == [0; 8]
        || (header[..4] == SHM_MAGIC.to_be_bytes() && header[4..] == SHM_VERSION.to_le_bytes())
}

/// The sweep of [`sweep_orphaned_segments`] over the names under `prefix`
/// whose first 8 bytes `current` accepts as a layout that takes the lock.
#[cfg(target_os = "linux")]
pub(crate) fn sweep_segments(
    prefix: &str,
    min_age: Duration,
    current: impl Fn(&[u8; 8]) -> bool,
) -> io::Result<SweepReport> {
    use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
    let mut report = SweepReport::default();
    let now = std::time::SystemTime::now();
    for entry in std::fs::read_dir(DEV_SHM)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(prefix) {
            continue;
        }
        let Ok(file) = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(entry.path())
        else {
            continue;
        };
        let Ok(meta) = file.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let changed = std::time::UNIX_EPOCH
            + Duration::new(
                u64::try_from(meta.ctime()).unwrap_or(0),
                u32::try_from(meta.ctime_nsec()).unwrap_or(0),
            );
        if now.duration_since(changed).unwrap_or_default() < min_age {
            report.young += 1;
            continue;
        }
        let mut header = [0u8; 8];
        if !(file.read_exact_at(&mut header, 0).is_ok() && current(&header)) {
            report.foreign += 1;
            continue;
        }
        // SAFETY: flock on the valid fd of `file`.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            report.held += 1;
            continue;
        }
        shm_unlink_by_os_id(&format!("/{name}"));
        report.removed += 1;
    }
    Ok(report)
}

/// Unlinks the segments of `flink_dir` no process holds: the macOS
/// counterpart of [`sweep_orphaned_segments`], which finds them through
/// their flinks, since macOS lists no shared-memory objects. A segment is
/// orphaned when nobody holds its flink's lock (see [`hold_segment`]);
/// one whose flink changed more recently than `min_age` and one of
/// another layout version are left alone. A segment whose flink went
/// stays until the next reboot: nothing names it any more.
///
/// # Errors
/// Reading `flink_dir` failed; a flink that cannot be opened is skipped.
#[cfg(all(unix, not(target_os = "linux")))]
pub fn sweep_orphaned_flinks(flink_dir: &Path, min_age: Duration) -> io::Result<SweepReport> {
    use std::io::Read;
    let mut report = SweepReport::default();
    let now = std::time::SystemTime::now();
    let entries = match std::fs::read_dir(flink_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(report),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.extension().is_some_and(|x| x == "bell") {
            sweep_bell_marker(&path, now, min_age, &mut report);
            continue;
        }
        if path.extension().is_none_or(|x| x != "shm") {
            continue;
        }
        let Ok(mut file) = std::fs::File::open(&path) else {
            continue;
        };
        let Ok(meta) = file.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let changed = meta.modified().unwrap_or(now);
        if now.duration_since(changed).unwrap_or_default() < min_age {
            report.young += 1;
            continue;
        }
        // SAFETY: flock on the valid fd of `file`.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            report.held += 1;
            continue;
        }
        let mut os_id = String::new();
        if file.read_to_string(&mut os_id).is_err() || !os_id.starts_with("/zd-") {
            report.foreign += 1;
            continue;
        }
        if !segment_is_current_version(&os_id) {
            report.foreign += 1;
            continue;
        }
        shm_unlink_by_os_id(&os_id);
        let _ = std::fs::remove_file(&path);
        report.removed += 1;
    }
    Ok(report)
}

/// One doorbell marker of [`sweep_orphaned_flinks`]: the bell it names
/// is unlinked and the marker removed when no participant holds the
/// marker's lock and it is older than `min_age`. A marker whose bell is
/// gone is removed too; one that names another layout is left.
#[cfg(all(unix, not(target_os = "linux")))]
fn sweep_bell_marker(
    path: &Path,
    now: std::time::SystemTime,
    min_age: Duration,
    report: &mut SweepReport,
) {
    use crate::bell::{BELL_NAME_PREFIX, BellSegment, bell_segment};
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    if !stem.starts_with(BELL_NAME_PREFIX) {
        report.foreign += 1;
        return;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let changed = file.metadata().and_then(|m| m.modified()).unwrap_or(now);
    if now.duration_since(changed).unwrap_or_default() < min_age {
        report.young += 1;
        return;
    }
    // SAFETY: flock on the valid fd of `file`.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        report.held += 1;
        return;
    }
    let os_id = format!("/{stem}");
    match bell_segment(&os_id) {
        BellSegment::Other => {
            report.foreign += 1;
            return;
        }
        BellSegment::Current => shm_unlink_by_os_id(&os_id),
        BellSegment::Missing => {}
    }
    let _ = std::fs::remove_file(path);
    report.removed += 1;
}

/// Whether segment `os_id` carries this layout version. A segment that
/// cannot be mapped counts as not.
#[cfg(all(unix, not(target_os = "linux")))]
fn segment_is_current_version(os_id: &str) -> bool {
    let Ok(shmem) = Mapping::open_os_id(os_id) else {
        return false;
    };
    if shmem.len() < 8 {
        return false;
    }
    // SAFETY: the mapping is at least 8 bytes long; magic and version are
    // the first two u32 of the header.
    let (magic, version) = unsafe {
        let base = shmem.as_ptr();
        (
            u32::from_be(ptr::read_unaligned(base as *const u32)),
            u32::from_le(ptr::read_unaligned(base.add(4) as *const u32)),
        )
    };
    magic == SHM_MAGIC && version == SHM_VERSION
}

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

/// Magic prefix for ZeroDDS SHM segments. `ZSHM` in big-endian.
pub const SHM_MAGIC: u32 = u32::from_be_bytes(*b"ZSHM");

/// Segment version. Bump on layout change; openers reject unknown
/// versions. 2: every user holds a shared `flock` on the segment (see
/// `hold_segment`); a version-1 user takes none, so its live segments
/// would look orphaned to version 2.
pub const SHM_VERSION: u32 = 2;

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

/// Event-driven cross-process notify for the SHM ring (Linux).
///
/// The producer advances the 64-bit `head` write-pointer on every frame and
/// then [`wake`]s; a parked consumer [`wait`]s on the **low 32 bits** of that
/// same `head` word (it changes on every push, so it doubles as the futex
/// sequence). This replaces the previous exponential-backoff sleep-poll, whose
/// 10→20→40→80 µs steps injected a bimodal 66–140 µs same-host round-trip
/// latency (vs ~24 µs for the UDP loopback it shadows). The futex is **shared**
/// (no `FUTEX_PRIVATE_FLAG`), so wait/wake key on the underlying physical page
/// and work across the two processes despite differing virtual addresses.
///
/// macOS parks through [`ring_os_sync`]; other targets keep the sleep-poll
/// in [`PosixShmTransport::wait_for_frame`]. The wire format is the same.
#[cfg(target_os = "linux")]
mod ring_futex {
    use std::time::Duration;

    /// Wake up to `n` consumers parked on `addr` (the low 32 bits of `head`).
    pub(super) fn wake(addr: *const u32, n: i32) {
        // SAFETY: `addr` points to a live `u32` inside a shared mapping that
        // outlives this call. `FUTEX_WAKE` ignores `*addr` and only signals
        // parked waiters; the trailing args are unused for WAKE.
        unsafe {
            libc::syscall(
                libc::SYS_futex,
                addr,
                libc::FUTEX_WAKE,
                n,
                core::ptr::null::<libc::timespec>(),
                core::ptr::null::<u32>(),
                0,
            );
        }
    }

    /// Park until `*addr != expected`, a [`wake`] arrives, or `timeout`
    /// elapses. `FUTEX_WAIT` re-checks `*addr == expected` atomically before
    /// sleeping, so a wake published after the caller read `expected` (but
    /// before this call) returns `EAGAIN` immediately — no lost wakeup. The
    /// caller always re-polls the ring afterwards, so `EAGAIN`/`EINTR`/
    /// `ETIMEDOUT` need no distinction.
    pub(super) fn wait(addr: *const u32, expected: u32, timeout: Option<Duration>) {
        let ts = timeout.map(|d| libc::timespec {
            tv_sec: d.as_secs() as libc::time_t,
            tv_nsec: d.subsec_nanos() as libc::c_long,
        });
        let ts_ptr = ts
            .as_ref()
            .map_or(core::ptr::null(), |t| t as *const libc::timespec);
        // SAFETY: `addr` is a live `u32` in a shared mapping; the relative
        // `timeout` (or NULL for infinite) points to a valid stack `timespec`
        // for the duration of the call.
        unsafe {
            libc::syscall(
                libc::SYS_futex,
                addr,
                libc::FUTEX_WAIT,
                expected,
                ts_ptr,
                core::ptr::null::<u32>(),
                0,
            );
        }
    }
}

/// The macOS counterpart of [`ring_futex`]: `os_sync_wait_on_address`
/// (macOS 14.4) with `OS_SYNC_WAIT_ON_ADDRESS_SHARED` parks on a word in
/// memory several processes map, as a shared futex does. Before, macOS
/// polled with a 10 µs → 1 ms backoff: 4.45 % of a core per idle receive
/// thread, and p50 177 µs against 58 µs over UDP loopback (measured on a
/// Mac in a multi-process test chain, 2026-10-02).
///
/// The functions are looked up at run time: a binary that names them
/// does not start on an older macOS. Without them the sleep-poll stays.
#[cfg(target_os = "macos")]
mod ring_os_sync {
    use core::ffi::c_void;
    use std::sync::OnceLock;
    use std::time::Duration;

    type WaitFn = unsafe extern "C" fn(*mut c_void, u64, usize, u32, u32, u64) -> i32;
    type WakeFn = unsafe extern "C" fn(*mut c_void, usize, u32) -> i32;

    fn api() -> Option<(WaitFn, WakeFn)> {
        static API: OnceLock<Option<(WaitFn, WakeFn)>> = OnceLock::new();
        *API.get_or_init(|| {
            // SAFETY: dlsym with NUL-terminated names. A non-null result is
            // the function of that name in <os/os_sync_wait_on_address.h>,
            // whose C signature the two types above repeat.
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

    /// Whether this macOS has the functions.
    pub(super) fn available() -> bool {
        api().is_some()
    }

    /// Wake every consumer parked on `addr`.
    pub(super) fn wake(addr: *const u32) {
        if let Some((_, wake_all)) = api() {
            // SAFETY: `addr` is a live, aligned `u32` in a shared mapping
            // that outlives this call.
            unsafe {
                wake_all(addr as *mut c_void, 4, libc::OS_SYNC_WAKE_BY_ADDRESS_SHARED);
            }
        }
    }

    /// Park until `*addr != expected`, a [`wake`], or `timeout`. The call
    /// compares `*addr` with `expected` before it sleeps, so a wake after
    /// the caller read `expected` is not lost; the caller re-polls the ring
    /// whatever the return was.
    pub(super) fn wait(addr: *const u32, expected: u32, timeout: Option<Duration>) {
        let Some((wait_on_address, _)) = api() else {
            return;
        };
        // No deadline: an hour, and the caller's loop parks again.
        let ns = timeout
            .map_or(3_600_000_000_000, |d| {
                u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
            })
            .max(1);
        // SAFETY: as in `wake`; the value and size describe that `u32`.
        unsafe {
            wait_on_address(
                addr as *mut c_void,
                u64::from(expected),
                4,
                libc::OS_SYNC_WAIT_ON_ADDRESS_SHARED,
                libc::OS_CLOCK_MACH_ABSOLUTE_TIME,
                ns,
            );
        }
    }
}

/// How a consumer parks on `head` on this target: the futex on Linux,
/// `os_sync_wait_on_address` on a macOS that has it.
/// `None` leaves the sleep-poll in [`PosixShmTransport::wait_for_frame`].
#[cfg_attr(windows, allow(dead_code))]
fn ring_park() -> Option<fn(*const u32, u32, Option<Duration>)> {
    #[cfg(target_os = "linux")]
    return Some(ring_futex::wait);
    #[cfg(target_os = "macos")]
    return ring_os_sync::available().then_some(ring_os_sync::wait as fn(_, _, _));
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return None;
}

/// For [`crate::bell`]: [`ring_park`], [`ring_wake`], the hash of the
/// short names, [`shm_unlink_by_os_id`] and the Windows event.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn park_fn() -> Option<fn(*const u32, u32, Option<Duration>)> {
    ring_park()
}

pub(crate) fn park_wake(addr: *const u32) {
    ring_wake(addr);
}

pub(crate) fn fnv1a_128_bytes(data: &[u8]) -> [u8; 16] {
    fnv1a_128(data)
}

pub(crate) fn unlink_os_id(os_id: &str) {
    shm_unlink_by_os_id(os_id);
}

#[cfg(windows)]
pub(crate) type RingEventHandle = RingEvent;

/// Wakes the consumers [`ring_park`] parked on `addr`.
fn ring_wake(addr: *const u32) {
    #[cfg(target_os = "linux")]
    ring_futex::wake(addr, i32::MAX);
    #[cfg(target_os = "macos")]
    ring_os_sync::wake(addr);
    let _ = addr;
}

/// Windows has no wait on an address across processes (`WaitOnAddress`
/// keys on the process's own address space), so every segment has a
/// named auto-reset event: the owner sets it after each frame, the
/// consumer waits on it. A set before the wait stays set, so a wake
/// between the consumer's look at the ring and its wait is not lost.
/// Before, Windows polled with a 10 µs → 1 ms backoff.
#[cfg(windows)]
pub(crate) struct RingEvent(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl RingEvent {
    /// Creates the event of segment `os_id`, or opens it if the other
    /// side made it first.
    pub(crate) fn open(os_id: &str) -> io::Result<Self> {
        use windows_sys::Win32::System::Threading::CreateEventW;
        let name: Vec<u16> = format!("Local\\zd-ev-{}", os_id.trim_start_matches('/'))
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: `name` is NUL-terminated UTF-16 that outlives the call;
        // no security attributes; auto-reset, initially not set.
        let handle = unsafe { CreateEventW(core::ptr::null(), 0, 0, name.as_ptr()) };
        if handle == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(handle))
    }

    pub(crate) fn set(&self) {
        // SAFETY: a live event handle owned by `self`.
        unsafe {
            windows_sys::Win32::System::Threading::SetEvent(self.0);
        }
    }

    pub(crate) fn wait(&self, timeout: Option<Duration>) {
        use windows_sys::Win32::System::Threading::{INFINITE, WaitForSingleObject};
        let ms = timeout.map_or(INFINITE, |d| {
            u32::try_from(d.as_millis())
                .unwrap_or(INFINITE - 1)
                .clamp(1, INFINITE - 1)
        });
        // SAFETY: a live event handle owned by `self`.
        unsafe {
            WaitForSingleObject(self.0, ms);
        }
    }
}

#[cfg(windows)]
impl Drop for RingEvent {
    fn drop(&mut self) {
        // SAFETY: the handle `open` returned, closed once.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

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
    /// Base directory for flink files (a meta file per segment that
    /// holds the OS segment name).
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
    /// Base pointer, lifetime-bound to the holding `Mapping`.
    base: *mut u8,
    /// Data region length in bytes (without the 64-byte header).
    capacity: usize,
}

// `SegmentLayout` holds a raw pointer into memory that's mapped for
// the lifetime of the owning `Mapping`.
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
//  - `Mapping` contains a `*mut u8`; the pointer is valid in the same
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

    /// The low 32 bits of `head`, the word a consumer parks on: it changes
    /// with every frame. Byte 16 on a little-endian target, byte 20 on a
    /// big-endian one; the futex path was little-endian-only for that
    /// reason, and a big-endian Linux polled.
    fn head_word(&self) -> *const u32 {
        let offset = if cfg!(target_endian = "big") { 20 } else { 16 };
        // SAFETY: base + 16 .. base + 24 is `head` in the mapped header.
        unsafe { self.base.add(offset) as *const u32 }
    }

    fn tail(&self) -> &AtomicU64 {
        // SAFETY: base + 24 is in the mapped header, AtomicU64 is 8-byte-aligned.
        unsafe { &*(self.base.add(24) as *const AtomicU64) }
    }

    fn shutdown(&self) -> &AtomicU32 {
        // SAFETY: base + 32 is in the mapped header, AtomicU32 is 4-byte-aligned.
        unsafe { &*(self.base.add(32) as *const AtomicU32) }
    }

    fn owner_epoch(&self) -> &AtomicU32 {
        // SAFETY: base + 40 is in the mapped header (formerly reserved,
        // zero in a fresh segment), AtomicU32 is 4-byte-aligned.
        unsafe { &*(self.base.add(40) as *const AtomicU32) }
    }

    fn consumer_attached(&self) -> &AtomicU32 {
        // SAFETY: base + 36 is in the mapped header (formerly reserved,
        // zero in a fresh segment), AtomicU32 is 4-byte-aligned.
        unsafe { &*(self.base.add(36) as *const AtomicU32) }
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
/// `Mapping` contains a `*mut u8` (pointer to the mmap'd
/// segment) that does not get `Send/Sync` automatically. The ring
/// protocol via `SegmentLayout` is already secured by AcqRel atomics
/// and SPSC discipline (see `unsafe impl Send for SegmentLayout`), and
/// all other fields (`role`, locators, `PathBuf`, `String`,
/// `AtomicU64`) are inherently `Send + Sync`. The `Mapping` pointer
/// itself stays valid as long as the `PosixShmTransport` object lives
/// (Drop does `munmap` exactly once). Cross-thread move or cross-thread
/// `&self` access are therefore safe, as long as the caller keeps the
/// SPSC discipline (max one sender thread, max one receiver thread;
/// that is the DCPS wave-4b architecture: hot-path sender in the tick
/// worker, recv worker in the dedicated `recv_user_shm_loop`).
pub struct PosixShmTransport {
    _shmem: Mapping, // keeps the mapping alive; Drop unmaps
    /// The shared `flock` on the segment that tells a sweeper this
    /// segment is in use (see `hold_segment`); released by the kernel
    /// when this process ends, however it ends. `None` where the segment
    /// is not a file that takes a lock.
    _liveness: Option<std::fs::File>,
    /// Windows: the segment's named event a consumer waits on.
    #[cfg(windows)]
    ring_event: RingEvent,
    /// The owner epoch this owner opened the segment under. Its drop ends
    /// the segment only while no later owner has opened it since.
    owner_epoch: u32,
    /// Serializes the senders of this process: the ring has one producer,
    /// and a same-host channel sends from the tick, the receive threads
    /// and the application's writers at once. Unserialized, two pushes
    /// raced on `head` (measured: `used > capacity` in a reliable burst).
    producer: std::sync::Mutex<()>,
    /// `false`: a push does not wake a consumer parked on `head`, because
    /// the consumer parks on a [`crate::bell::ShmDoorbell`] instead.
    wake_on_push: core::sync::atomic::AtomicBool,
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
    /// still call `shm_unlink` even on an internal mapping failure
    /// (crash-recovery defense-in-depth).
    os_id: String,
    /// Consumed padding frames since bind. See
    /// [`padding_frames_seen`](Self::padding_frames_seen).
    padding_counter: core::sync::atomic::AtomicU64,
    /// Corrupt-frame drops: the owner wrote a length > max_datagram.
    /// The consumer drops the frame + skips to the ring end.
    corrupt_frame_counter: core::sync::atomic::AtomicU64,
    /// Set by [`Self::interrupt_recv`]: the next or the waiting `recv`
    /// returns `Timeout` at once. Process-local, not in the segment.
    interrupted: core::sync::atomic::AtomicBool,
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
        if self.role == ShmRole::Consumer {
            let _ = self.layout.consumer_attached().fetch_update(
                Ordering::AcqRel,
                Ordering::Acquire,
                |n| n.checked_sub(1),
            );
        }
        // An owner that re-opened the pair (a channel closed and opened
        // again while this one was still in use) attached to this very
        // segment and carries it on: ending it here would set the shutdown
        // flag and unlink the name under the live owner, which then writes
        // into a ring nobody reads.
        let superseded = self.role == ShmRole::Owner
            && self.layout.owner_epoch().load(Ordering::Acquire) != self.owner_epoch;
        if superseded {
            // Nor may the mapping unlink it, as the creator's would.
            self._shmem.set_owner(false);
        }
        if self.role == ShmRole::Owner && !superseded {
            // 0. Set the shutdown flag before we tear down the
            //    resources. A consumer in a wait_for_frame loop sees the
            //    release store and returns a targeted error instead of
            //    running into the normal recv_timeout.
            self.layout
                .shutdown()
                .store(1, core::sync::atomic::Ordering::Release);
            // Wake any consumer parked in `wait_for_frame` so it observes the
            // shutdown flag immediately instead of waiting out its
            // recv_timeout. The futex keys on `head` (the wait address); a
            // bare wake with no head change still releases the waiter, which
            // then re-checks shutdown.
            self.wake_ring();
            // 1. Remove the flink file (lookup file, not a segment).
            let _ = std::fs::remove_file(&self.flink_path);
            // 2. Remove the lock file.
            let _ = std::fs::remove_file(&self.lock_path);
            // 3. Unlink the /dev/shm segment — defense-in-depth.
            //    The mapping does this itself when it is the owner (set
            //    in the `open()` path after `create`), but an explicit
            //    call is idempotent.
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
        let _guard = lock_pair(&flink.with_extension("lock"))?;

        // A segment no process holds any more is discarded and made
        // again, once; see `hold_segment`.
        let mut discarded = false;
        let requested = config;
        let (shmem, liveness, config) = loop {
            // What this side asked for; an attacher takes the ring's size
            // from the segment instead (see below).
            let mut config = requested.clone();
            // Try attach to existing segment first.
            let (mut shmem, is_creator) = match Mapping::open_flink(&flink) {
                Ok(s) => (s, false),
                // No flink here, but a process holds a segment under the
                // pair's name: its users found it through another flink
                // directory (another TMPDIR, a service's private /tmp).
                // Unlinking it, as the branch below does, left that side
                // on the old ring and this one on a new one.
                Err(_) if segment_in_use(&os_id) => (Mapping::open_os_id(&os_id)?, false),
                Err(_) => {
                    // Crash recovery: remove a zombie shm file from a
                    // previous crashed process.
                    shm_unlink_by_os_id(&os_id);
                    let _ = std::fs::remove_file(&flink);
                    let fresh =
                        Mapping::create(&os_id, HEADER_BYTES + config.capacity, Some(&flink));
                    match fresh {
                        Ok(fresh) => (fresh, true),
                        // The mapping is there and the flink is not.
                        //
                        // On Windows that is a state the two steps above
                        // cannot leave: `shm_unlink_by_os_id` is a no-op
                        // there — the documented one, forty lines up — and
                        // removing the flink file does not touch the kernel
                        // object, which lives until the last handle closes.
                        // So a segment another handle still holds survives
                        // both, and `create` answers `MappingIdExists`.
                        //
                        // That is not a failure. It says the segment exists,
                        // which is the same thing the `open` above was
                        // looking for and missed only because it looks by
                        // flink. Attach by os_id, and attach as the
                        // **non-creator**: something else made it and will
                        // unlink it, and a second owner would unlink twice.
                        Err(ShmemError::MappingIdExists) => (Mapping::open_os_id(&os_id)?, false),
                        Err(other) => return Err(other.into()),
                    }
                }
            };

            let base = shmem.as_ptr();
            let total_size = shmem.len();
            if is_creator && total_size < HEADER_BYTES + config.capacity {
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
                // Only an owner-role creator is the mapping's owner, which
                // unlinks the segment on drop.
                // A consumer that made the segment first must not: its drop
                // took the name from under the live owner, and a consumer
                // that opened the pair again made a ring of its own beside
                // the owner's. The attacher never does (double unlink).
                shmem.set_owner(role == ShmRole::Owner);
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
                // The ring is as large as its creator made it. Both sides
                // used their own configured size before, and two sides that
                // configured different ones (`transport.shm_max_datagram`
                // per participant) either failed to attach or ran the ring
                // with two different capacities.
                // SAFETY: segment >= HEADER_BYTES bytes; offset 8 holds the
                // capacity the creator wrote.
                let raw = unsafe { ptr::read_unaligned(base.add(8) as *const u64) };
                let capacity = usize::try_from(u64::from_le(raw)).unwrap_or(usize::MAX);
                if capacity < 32 || total_size < HEADER_BYTES.saturating_add(capacity) {
                    return Err(PosixShmError::InvalidHeader);
                }
                config.capacity = capacity;
                if config.max_datagram.saturating_mul(2).saturating_add(16) > capacity {
                    config.max_datagram = (capacity - 16) / 2;
                }
            }

            match hold_segment(&os_id, &flink, is_creator)? {
                SegmentHold::Held(file) => break (shmem, file, config),
                SegmentHold::Orphaned | SegmentHold::Vanished if !discarded => {
                    // Its ring still holds the frames of a session that is
                    // over; a consumer attaching to it would deliver them.
                    drop(shmem);
                    discard_orphaned_segment(&os_id);
                    let _ = std::fs::remove_file(&flink);
                    discarded = true;
                }
                SegmentHold::Orphaned | SegmentHold::Vanished => {
                    return Err(PosixShmError::Io(io::Error::other(
                        "shm segment vanished again after it was made anew",
                    )));
                }
            }
        };
        let base = shmem.as_ptr();

        // SAFETY: base is valid for the lifetime of shmem; layout holds only refs of the same lifetime.
        let layout = unsafe { SegmentLayout::new(base, config.capacity) };
        if role == ShmRole::Consumer {
            // A count, not a flag: a consumer that re-opens the ring before
            // its old handle is gone would otherwise be marked detached by
            // that handle's drop.
            layout.consumer_attached().fetch_add(1, Ordering::AcqRel);
        }
        let owner_epoch = if role == ShmRole::Owner {
            layout
                .owner_epoch()
                .fetch_add(1, Ordering::AcqRel)
                .wrapping_add(1)
        } else {
            0
        };

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
        #[cfg(windows)]
        let ring_event = RingEvent::open(&os_id)?;
        Ok(Self {
            _shmem: shmem,
            _liveness: liveness,
            #[cfg(windows)]
            ring_event,
            owner_epoch,
            producer: std::sync::Mutex::new(()),
            wake_on_push: core::sync::atomic::AtomicBool::new(true),
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
            interrupted: core::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Makes the `recv` waiting on this segment, or the next one, return
    /// `Timeout` at once, so a receive thread that is told to stop does
    /// not wait out its `recv_timeout`. With it the timeout is no longer
    /// what bounds the stop, and can be long: a receive thread with a 1 ms
    /// timeout woke a thousand times a second with nothing to receive.
    pub fn interrupt_recv(&self) {
        self.interrupted.store(true, Ordering::Release);
        // The waiter parks on `head`; a wake without a change of `head`
        // releases it to re-check, and it finds the flag.
        self.wake_ring();
    }

    /// Whether a consumer has the segment open, from the owner's side. An
    /// owner that sends before that writes into a ring nobody reads: a
    /// peer that never attaches (an older ZeroDDS, a peer that left) would
    /// lose everything sent there.
    #[must_use]
    pub fn peer_attached(&self) -> bool {
        self.layout.consumer_attached().load(Ordering::Acquire) > 0
    }

    /// Sends `prefix` and `data` as one frame, without joining them first.
    ///
    /// # Errors
    /// As [`Transport::send`].
    pub fn send_with_prefix(
        &self,
        dest: &Locator,
        prefix: &[u8],
        data: &[u8],
    ) -> Result<(), SendError> {
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
        let size = prefix.len() + data.len();
        if size > self.config.max_datagram {
            return Err(SendError::PayloadTooLarge {
                size,
                limit: self.config.max_datagram,
            });
        }
        let _one_producer = self.producer.lock().map_err(|_| SendError::Io {
            message: "shm producer lock poisoned",
        })?;
        self.push_frame(prefix, data)
    }

    /// Whether a push wakes a consumer parked on this ring's `head`. A
    /// consumer that parks on a doorbell for several rings does not park
    /// here, and the wake would be a wasted syscall per frame.
    pub fn set_wake_on_push(&self, wake: bool) {
        self.wake_on_push.store(wake, Ordering::Relaxed);
    }

    /// The next frame, without waiting: `Ok(None)` on an empty ring.
    ///
    /// # Errors
    /// The owner dropped the segment (`Io`), or this is the owner's side.
    pub fn try_recv(&self) -> Result<Option<ReceivedDatagram>, RecvError> {
        if self.role != ShmRole::Consumer {
            return Err(RecvError::Io {
                message: "shm owner cannot recv on this segment",
            });
        }
        if let Some(data) = self.pop_frame() {
            return Ok(Some(ReceivedDatagram {
                source: self.peer_locator,
                data: data.into(),
            }));
        }
        if self.layout.shutdown().load(Ordering::Acquire) != 0 {
            return Err(RecvError::Io {
                message: "shm owner terminated",
            });
        }
        Ok(None)
    }

    /// How far the reader has consumed the ring, in bytes since it was
    /// made: it moves while the reader drains and stands still when it
    /// does not.
    #[must_use]
    pub fn consumed_bytes(&self) -> u64 {
        self.layout.tail().load(Ordering::Acquire)
    }

    /// Number of bytes in the ring not yet consumed by the reader.
    /// Diagnostic for backpressure monitoring.
    #[must_use]
    pub fn occupied_bytes(&self) -> u64 {
        let h = self.layout.head().load(Ordering::Acquire);
        let t = self.layout.tail().load(Ordering::Acquire);
        h.wrapping_sub(t)
    }

    fn push_frame(&self, prefix: &[u8], data: &[u8]) -> Result<(), SendError> {
        let len = prefix.len() + data.len();
        let needed = (len + 4) as u64; // length-prefix
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
                // The frame would wrap → first fill the rest of the ring,
                // then retry from its start. A rest of 4 bytes or more
                // gets a padding header; a rest of 1 to 3 bytes holds no
                // header and is skipped by its size alone, which is how
                // `pop_frame` reads it. Without that skip a frame ending
                // 1 to 3 bytes before the end left the writer waiting for
                // room in an empty ring, and every later send failed.
                //
                // The condition `needed + tail_space <= free` is
                // CORRECT: after the padding `used` rises by
                // `tail_space` (the reader skips it in `pop_frame`; until
                // it does, the bytes are ring occupancy). So before the
                // wrap it must hold that `cap - used >= tail_space +
                // needed`, i.e. `free >= tail_space + needed`.
                if (needed + (tail_space as u64)) <= free {
                    if tail_space >= 4 {
                        self.layout.write_u32(h_mod, PADDING_FRAME_LEN);
                    }
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
            let len_u32 = u32::try_from(len).map_err(|_| SendError::PayloadTooLarge {
                size: len,
                limit: self.config.max_datagram,
            })?;
            self.layout.write_u32(h_mod, len_u32);
            self.layout.write_slice(h_mod + 4, prefix);
            self.layout.write_slice(h_mod + 4 + prefix.len(), data);
            self.layout.head().store(h + needed, Ordering::Release);
            // Event-driven notify: the `head` store above is published; wake a
            // consumer parked in `wait_for_frame` on the low 32 bits of `head`.
            // Wake-all (typically a single consumer) keeps it correct if a
            // future revision maps several readers onto one segment.
            if self.wake_on_push.load(Ordering::Relaxed) {
                self.wake_ring();
            }
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
            // The bound is the ring's, not this side's `max_datagram`: the
            // owner may have configured a larger one, and its frames are
            // whole up to half the ring.
            if len as usize > self.config.capacity / 2 {
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
        let deadline = self.config.recv_timeout.map(|t| Instant::now() + t);
        #[cfg(windows)]
        return self.wait_parked(deadline, |_, _, timeout| self.ring_event.wait(timeout));
        #[cfg(not(windows))]
        match ring_park() {
            Some(park) => self.wait_parked(deadline, park),
            None => self.wait_polling(deadline),
        }
    }

    /// Wakes the consumer parked on this segment.
    fn wake_ring(&self) {
        ring_wake(self.layout.head_word());
        #[cfg(windows)]
        self.ring_event.set();
    }

    /// Event-driven wait: park on the low 32 bits of `head` and let the
    /// producer's wake (in `push_frame`) deliver the frame at once. The
    /// `head` value is read **before** `pop_frame`, so a frame published
    /// between the read and the park makes `*addr != expected`, and the
    /// park returns at once: no lost wakeup.
    fn wait_parked(
        &self,
        deadline: Option<Instant>,
        park: impl Fn(*const u32, u32, Option<Duration>),
    ) -> Result<Vec<u8>, RecvError> {
        let head_word = self.layout.head_word();
        loop {
            let expected = self.layout.head().load(Ordering::Acquire) as u32;
            if let Some(frame) = self.pop_frame() {
                return Ok(frame);
            }
            // Targeted owner-gone signal: the owner sets shutdown=1 (and
            // wakes us) on drop. Checked after pop_frame, so frames
            // published before the drop still arrive.
            if self.layout.shutdown().load(Ordering::Acquire) != 0 {
                return Err(RecvError::Io {
                    message: "shm owner terminated",
                });
            }
            if self.interrupted.swap(false, Ordering::AcqRel) {
                return Err(RecvError::Timeout);
            }
            let remaining = match deadline {
                Some(d) => {
                    let now = Instant::now();
                    if now >= d {
                        return Err(RecvError::Timeout);
                    }
                    Some(d - now)
                }
                None => None,
            };
            park(head_word, expected, remaining);
        }
    }

    /// Sleep-poll for a target [`ring_park`] has nothing for: exponential
    /// backoff 10 µs → 1 ms. Higher tail latency and a wakeup every
    /// millisecond while idle; identical on the wire.
    #[cfg_attr(windows, allow(dead_code))]
    fn wait_polling(&self, deadline: Option<Instant>) -> Result<Vec<u8>, RecvError> {
        let mut backoff = Duration::from_micros(10);
        let max_backoff = Duration::from_millis(1);
        loop {
            if let Some(frame) = self.pop_frame() {
                return Ok(frame);
            }
            if self.layout.shutdown().load(Ordering::Acquire) != 0 {
                return Err(RecvError::Io {
                    message: "shm owner terminated",
                });
            }
            if self.interrupted.swap(false, Ordering::AcqRel) {
                return Err(RecvError::Timeout);
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
    /// The datagram limit of the segment ([`ShmConfig::max_datagram`]).
    fn max_message_size(&self) -> usize {
        self.config.max_datagram
    }

    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        self.send_with_prefix(dest, &[], data)
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

    /// A GUID for the tests, distinct in every process.
    ///
    /// `segment_os_id` is deterministic in (owner_id, consumer_id) and
    /// the segment it names lives in `/dev/shm`, which is the whole
    /// host's. Two test binaries — or two checkouts' runs — that both
    /// wrote `id(10)` therefore opened the *same* segment, and the
    /// second one read the first one's header: measured as
    /// `consumer_cannot_send: Err(InvalidHeader)` and as a `recv()`
    /// timeout in `open_concurrent_two_threads_both_bound`.
    ///
    /// The process id goes into the leading bytes, so the name is still
    /// deterministic inside one process — the tests pair an owner and a
    /// consumer by number — and unique between processes.
    ///
    /// The process id alone is not enough, and two measurements say so.
    /// `segment_os_id` took only bytes 13..16 on macOS, where the name
    /// has to stay under `PSHMNAMLEN`, so the leading four bytes were not
    /// in the name there at all (it hashes both ids now). And Windows
    /// reuses process ids quickly:
    /// on the CI runner this test answered first `Shm(MappingIdExists)`
    /// and then, once attaching was allowed,
    /// `InvalidConfig { "mapped segment smaller than requested
    /// capacity" }` — a segment under this very name, alive, and not
    /// this run's.
    ///
    /// So bytes 13 and 14 carry a value that is new for every process,
    /// and they are inside what every platform puts in the name. Byte 15
    /// stays the test's own number, which is what pairs an owner with
    /// its consumer.
    fn id(n: u8) -> [u8; 16] {
        use std::sync::OnceLock;
        static RUN: OnceLock<u16> = OnceLock::new();
        let run = *RUN.get_or_init(|| {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            // The pid rotates, the clock does not; together they do not
            // repeat within the lifetime of a mapping.
            (nanos as u16) ^ (std::process::id() as u16)
        });
        let mut a = [0u8; 16];
        a[..4].copy_from_slice(&std::process::id().to_le_bytes());
        a[13..15].copy_from_slice(&run.to_le_bytes());
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

    /// A frame that ends 1 to 3 bytes before the ring's end leaves room for
    /// no padding header. The writer skips that remainder, as the reader
    /// does; it used to wait for the reader to make room in an empty ring,
    /// and every later send failed with "near wraparound".
    #[test]
    fn a_remainder_too_small_for_a_padding_header_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 256,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 120,
            recv_timeout: Some(Duration::from_secs(2)),
        };
        let owner = PosixShmTransport::open_owner(id(74), id(75), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(75), id(74), cfg).unwrap();
        let to = Locator::shm(id(75));
        // Frames of 4 + 80, 4 + 81, 4 + 81 bytes: the head stops 2 bytes
        // short of 256.
        for (fill, len) in [(1u8, 80), (2, 81), (3, 81)] {
            owner.send(&to, &vec![fill; len]).unwrap();
            assert_eq!(consumer.recv().unwrap().data[..], vec![fill; len][..]);
        }
        assert_eq!(owner.occupied_bytes(), 0, "the ring is empty");
        for fill in 4u8..10 {
            owner
                .send(&to, &[fill; 10])
                .expect("a frame fits an empty ring");
            assert_eq!(consumer.recv().unwrap().data[..], [fill; 10]);
        }
    }

    /// Frames of every length from 1 to `max_datagram`, in a fixed but
    /// irregular order, end at every offset of the ring, so the writer
    /// meets every remainder at the ring's end; all of them arrive whole
    /// and in order.
    #[test]
    fn frames_of_every_length_cross_the_ring_end_whole_and_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        // Three frames of up to 204 bytes and a padding of up to 203 fit.
        let cfg = ShmConfig {
            capacity: 1024,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 200,
            recv_timeout: Some(Duration::from_secs(2)),
        };
        let owner = PosixShmTransport::open_owner(id(76), id(77), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(77), id(76), cfg).unwrap();
        let to = Locator::shm(id(77));
        let mut x: u32 = 0x9E37_79B9;
        for round in 0..5_000u32 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let len = 1 + (x % 200) as usize;
            let fill = (round % 251) as u8;
            // Up to three frames in the ring before the reader takes them.
            let burst = 1 + (x >> 8) as usize % 3;
            for _ in 0..burst {
                let sent = owner.send(&to, &vec![fill; len]);
                assert!(sent.is_ok(), "round {round}, len {len}: {sent:?}");
            }
            for _ in 0..burst {
                let got = consumer.recv().unwrap();
                assert_eq!(got.data.len(), len, "round {round}");
                assert!(got.data.iter().all(|b| *b == fill), "round {round}");
            }
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
        // `Mapping` is not Send, so we can move neither owner nor consumer
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

    /// Backpressure must be loud. A consumer that never drains fills the
    /// ring; the writer then returns a named error after `SPIN_LIMIT`
    /// spins instead of blocking forever or dropping the frame quietly.
    #[test]
    fn full_ring_returns_an_error_instead_of_dropping() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 512,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 128,
            recv_timeout: Some(Duration::from_millis(200)),
        };
        // 202/203 and not 220/221: `open_consumer_first_then_owner_roundtrip`
        // has that pair, `segment_os_id` is a function of the pair alone, and
        // the two ask for different capacities — whichever ran first fixed the
        // size and the other got `mapped segment smaller than requested
        // capacity`. Windows caught it; Linux and macOS happened to interleave
        // the other way. `test_ids_are_not_shared_between_tests` below now
        // fails on a repeat instead of leaving it to a scheduler.
        let owner = PosixShmTransport::open_owner(id(202), id(203), cfg.clone()).unwrap();
        let _consumer = PosixShmTransport::open_consumer(id(203), id(202), cfg).unwrap();

        // 128-byte frames cost 132 bytes each; nothing is consumed, so
        // the fifth send at the latest has to hit the wall.
        let payload = vec![0x7Eu8; 128];
        let dest = Locator::shm(id(203));
        let mut sent = 0usize;
        let err = loop {
            match owner.send(&dest, &payload) {
                Ok(()) => {
                    sent += 1;
                    assert!(sent < 32, "the ring must fill, not swallow frames");
                }
                Err(e) => break e,
            }
        };
        match err {
            SendError::Io { message } => assert!(
                message.contains("ring full"),
                "backpressure must name itself, got {message:?}"
            ),
            other => panic!("expected a ring-full error, got {other:?}"),
        }
        assert!(sent >= 1, "at least one frame fits before the ring is full");
    }

    /// A length beyond `max_datagram` is a broken or hostile owner. The
    /// consumer must drop the frame **and count it** — never allocate the
    /// announced size, never hand the ring up as data.
    #[test]
    fn corrupt_length_is_dropped_and_counted() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_tmp(tmp.path(), 4096);
        // No frame of an owner is longer than half the ring.
        let bound = cfg.capacity / 2;
        let owner = PosixShmTransport::open_owner(id(210), id(211), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(211), id(210), cfg).unwrap();

        // Write a bogus length header straight into the ring and publish
        // it, the way a corrupt owner would.
        owner.layout.write_u32(0, u32::try_from(bound + 1).unwrap());
        owner
            .layout
            .head()
            .store(4 + (bound as u64) + 1, Ordering::Release);

        assert!(consumer.pop_frame().is_none(), "no frame is handed up");
        assert_eq!(consumer.corrupt_frames_seen(), 1, "the drop is counted");
    }

    #[test]
    fn corrupt_frame_counter_default_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let t = PosixShmTransport::open_owner(id(150), id(151), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(t.corrupt_frames_seen(), 0);
        assert_eq!(t.padding_frames_seen(), 0);
    }

    /// DDSI-RTPS 2.5 §9.3.2.4 reserves `0x00000003` to `0x01ffffff` for
    /// vendor-specific kinds.
    /// ZeroDDS takes `0x01F0_0001` for shared memory; the transport
    /// binds that value, so a renumbering has to break here.
    #[test]
    fn locator_kind_is_the_vendor_reserved_shm_value() {
        let loc = Locator::shm(id(200));
        assert_eq!(loc.kind.as_i32(), 0x01F0_0001);
        assert!(
            (0x0000_0003..=0x01ff_ffff).contains(&loc.kind.as_i32()),
            "inside the vendor-specific range of §9.3.2.4"
        );
        assert_eq!(loc.port, 0, "the port field is unused for SHM");
        assert_eq!(loc.address, id(200));
    }

    /// §2 header: the magic is written big-endian at offset 0 and the
    /// version little-endian at offset 4 — read straight out of the
    /// mapping the owner created, not through the accessors.
    #[test]
    fn owner_writes_magic_and_version_into_the_header() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(160), id(161), cfg_tmp(tmp.path(), 4096)).unwrap();
        let base = owner.layout.base;
        // SAFETY: the owner holds the mapping; offsets 0 and 4 are inside
        // the 64-byte header.
        let (magic, version, capacity) = unsafe {
            (
                u32::from_be(ptr::read_unaligned(base as *const u32)),
                u32::from_le(ptr::read_unaligned(base.add(4) as *const u32)),
                u64::from_le(ptr::read_unaligned(base.add(8) as *const u64)),
            )
        };
        assert_eq!(magic, SHM_MAGIC, "magic is \"ZSHM\" big-endian");
        assert_eq!(magic.to_be_bytes(), *b"ZSHM");
        assert_eq!(version, SHM_VERSION);
        assert_eq!(capacity, 4096);
    }

    /// A segment carrying a foreign magic must be refused on attach
    /// rather than interpreted as a ring. Builds the segment through the
    /// same mapping primitives the transport uses, then attaches.
    #[test]
    fn attach_rejects_foreign_magic() {
        let tmp = tempfile::tempdir().unwrap();
        let flink = segment_flink(tmp.path(), id(170), id(171));
        let os_id = segment_os_id(id(170), id(171));
        shm_unlink_by_os_id(&os_id);
        let foreign = Mapping::create(&os_id, HEADER_BYTES + 4096, Some(&flink)).unwrap();
        // SAFETY: the mapping is at least HEADER_BYTES long.
        unsafe {
            ptr::write_unaligned(foreign.as_ptr() as *mut u32, u32::from_be_bytes(*b"IOX2"));
            ptr::write_unaligned(foreign.as_ptr().add(4) as *mut u32, SHM_VERSION.to_le());
        }

        let err = PosixShmTransport::open_consumer(id(171), id(170), cfg_tmp(tmp.path(), 4096))
            .expect_err("foreign magic must be refused");
        assert!(matches!(err, PosixShmError::InvalidHeader), "got {err:?}");
    }

    /// Same for an unknown layout version: a bumped header version means
    /// a different ring layout, so attaching would misread the frames.
    #[test]
    fn attach_rejects_unknown_version() {
        let tmp = tempfile::tempdir().unwrap();
        let flink = segment_flink(tmp.path(), id(172), id(173));
        let os_id = segment_os_id(id(172), id(173));
        shm_unlink_by_os_id(&os_id);
        let future = Mapping::create(&os_id, HEADER_BYTES + 4096, Some(&flink)).unwrap();
        // SAFETY: the mapping is at least HEADER_BYTES long.
        unsafe {
            ptr::write_unaligned(future.as_ptr() as *mut u32, SHM_MAGIC.to_be());
            ptr::write_unaligned(
                future.as_ptr().add(4) as *mut u32,
                (SHM_VERSION + 1).to_le(),
            );
        }

        let err = PosixShmTransport::open_consumer(id(173), id(172), cfg_tmp(tmp.path(), 4096))
            .expect_err("unknown version must be refused");
        assert!(matches!(err, PosixShmError::InvalidHeader), "got {err:?}");
    }

    /// §5.2 crash recovery: a flink file left behind by a crashed owner
    /// points at a segment that is no longer there. The next open must
    /// unlink the zombie, create a fresh segment and carry a frame —
    /// not fail and not leak the stale name.
    #[test]
    fn open_recovers_from_stale_flink_of_crashed_owner() {
        let tmp = tempfile::tempdir().unwrap();
        let flink = segment_flink(tmp.path(), id(180), id(181));
        std::fs::create_dir_all(tmp.path()).unwrap();
        // What a crashed owner leaves: a flink naming a segment that
        // `shm_open` no longer resolves.
        std::fs::write(&flink, b"/zerodds-zombie-that-is-gone").unwrap();
        assert!(flink.exists());

        let owner = PosixShmTransport::open_owner(id(180), id(181), cfg_tmp(tmp.path(), 4096))
            .expect("stale flink must be recovered, not fatal");
        let consumer =
            PosixShmTransport::open_consumer(id(181), id(180), cfg_tmp(tmp.path(), 4096)).unwrap();
        owner
            .send(&Locator::shm(id(181)), b"after recovery")
            .unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"after recovery");
    }

    /// The environment variable that turns [`orphan_maker`] from a no-op
    /// into the process that leaves an orphaned segment behind.
    const ORPHAN_SPEC: &str = "ZERODDS_SHM_TEST_ORPHAN";

    fn hex16(b: [u8; 16]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// Not a test of its own: the body of the process that
    /// [`leave_orphan`] starts. It opens the owner side, publishes one
    /// frame and ends without dropping the transport, as a killed or
    /// crashed process ends.
    #[test]
    fn orphan_maker() {
        let Ok(spec) = std::env::var(ORPHAN_SPEC) else {
            return;
        };
        let parts: Vec<&str> = spec.split(';').collect();
        let bytes = |h: &str| -> [u8; 16] {
            let mut b = [0u8; 16];
            for (i, chunk) in b.iter_mut().enumerate() {
                *chunk = u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap();
            }
            b
        };
        let (owner, consumer) = (bytes(parts[0]), bytes(parts[1]));
        let t = PosixShmTransport::open_owner(owner, consumer, cfg_tmp(Path::new(parts[2]), 4096))
            .unwrap();
        t.send(&Locator::shm(consumer), b"frame of an ended session")
            .unwrap();
        std::process::exit(0);
    }

    /// Runs [`orphan_maker`] in a process of its own and checks that the
    /// segment outlived it. `ZERODDS_TEST_RUNNER` names a program to start
    /// the test binary through, the way cargo's target runner does: under
    /// qemu-user without binfmt the binary does not start on its own.
    fn leave_orphan(owner: [u8; 16], consumer: [u8; 16], dir: &Path) {
        let exe = std::env::current_exe().unwrap();
        let runner = std::env::var("ZERODDS_TEST_RUNNER").unwrap_or_default();
        let mut runner = runner.split_whitespace();
        let mut command = match runner.next() {
            Some(program) => {
                let mut c = std::process::Command::new(program);
                c.args(runner).arg(&exe);
                c
            }
            None => std::process::Command::new(&exe),
        };
        let status = command
            .args(["--exact", "posix::tests::orphan_maker", "--test-threads=1"])
            .env(
                ORPHAN_SPEC,
                format!("{};{};{}", hex16(owner), hex16(consumer), dir.display()),
            )
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "the orphan maker failed: {status}");
        let flink = segment_flink(dir, owner, consumer);
        assert!(
            flink.exists(),
            "the ended process left {} behind",
            flink.display()
        );
        #[cfg(target_os = "linux")]
        {
            let name = segment_os_id(owner, consumer);
            assert!(
                Path::new(DEV_SHM)
                    .join(name.trim_start_matches('/'))
                    .exists(),
                "the ended process left {name} behind"
            );
        }
    }

    /// §4.4: on Linux and on macOS from 14.4 a consumer
    /// parks on `head` instead of polling. On macOS it polled every 10 µs
    /// to 1 ms before.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn the_consumer_parks_instead_of_polling() {
        assert!(ring_park().is_some());
    }

    /// The owner sees whether a consumer has the segment open; a frame
    /// sent with a prefix arrives as one datagram.
    #[test]
    fn the_owner_sees_the_consumer_attach_and_leave() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(246), id(247), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert!(!owner.peer_attached());
        let consumer =
            PosixShmTransport::open_consumer(id(247), id(246), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert!(owner.peer_attached());
        owner
            .send_with_prefix(&Locator::shm(id(247)), &[7], b"payload")
            .unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"\x07payload");
        drop(consumer);
        assert!(!owner.peer_attached());
    }

    /// An owner dropped after a second owner opened the same segment
    /// leaves it to the second: no shutdown flag, the name stays, and the
    /// consumer keeps receiving from the second owner.
    #[test]
    fn a_superseded_owner_leaves_the_segment_to_its_successor() {
        let tmp = tempfile::tempdir().unwrap();
        let first =
            PosixShmTransport::open_owner(id(248), id(249), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(249), id(248), cfg_tmp(tmp.path(), 4096)).unwrap();
        let second =
            PosixShmTransport::open_owner(id(248), id(249), cfg_tmp(tmp.path(), 4096)).unwrap();
        drop(first);
        second
            .send(&Locator::shm(id(249)), b"from the successor")
            .unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"from the successor");
        drop(second);
        assert!(
            matches!(consumer.recv(), Err(RecvError::Io { .. })),
            "the last owner's drop ends the segment"
        );
    }

    /// A consumer that made the segment before its owner leaves the name
    /// to the owner: a consumer that opens the pair again finds the live
    /// ring, not a new one of its own.
    #[test]
    fn a_consumer_that_made_the_segment_leaves_it_to_the_owner() {
        let tmp = tempfile::tempdir().unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(251), id(250), cfg_tmp(tmp.path(), 4096)).unwrap();
        let owner =
            PosixShmTransport::open_owner(id(250), id(251), cfg_tmp(tmp.path(), 4096)).unwrap();
        drop(consumer);
        assert!(!owner.peer_attached());
        let again =
            PosixShmTransport::open_consumer(id(251), id(250), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert!(owner.peer_attached(), "the same ring");
        owner.send(&Locator::shm(id(251)), b"same ring").unwrap();
        assert_eq!(&again.recv().unwrap().data[..], b"same ring");
    }

    /// Two sides that configured different ring sizes share the ring the
    /// creator made, each way round, and a frame larger than the
    /// consumer's own `max_datagram` still arrives.
    #[test]
    fn sides_with_different_ring_sizes_share_the_creators_ring() {
        let tmp = tempfile::tempdir().unwrap();
        let small = cfg_tmp(tmp.path(), 4096);
        let large = cfg_tmp(tmp.path(), 65536);
        // The owner with the large ring creates; the consumer with the
        // small one attaches.
        let owner = PosixShmTransport::open_owner(id(252), id(253), large.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(253), id(252), small.clone()).unwrap();
        let big = vec![0x42u8; 8000];
        owner.send(&Locator::shm(id(253)), &big).unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], big.as_slice());
        // The consumer with the large ring creates; the owner with the
        // small one attaches and sends what its own limit allows.
        let consumer = PosixShmTransport::open_consumer(id(255), id(254), large).unwrap();
        let owner = PosixShmTransport::open_owner(id(254), id(255), small).unwrap();
        owner.send(&Locator::shm(id(255)), &[7u8; 1000]).unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], &[7u8; 1000][..]);
    }

    /// `interrupt_recv` returns a waiting `recv` at once instead of after
    /// its timeout, and a frame that is there is still delivered first.
    #[test]
    fn interrupt_recv_returns_a_waiting_recv_at_once() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = cfg_tmp(tmp.path(), 4096);
        cfg.recv_timeout = Some(Duration::from_secs(20));
        let owner = PosixShmTransport::open_owner(id(238), id(239), cfg.clone()).unwrap();
        let consumer =
            std::sync::Arc::new(PosixShmTransport::open_consumer(id(239), id(238), cfg).unwrap());
        let waiting = std::sync::Arc::clone(&consumer);
        let t = std::thread::spawn(move || {
            let start = Instant::now();
            (waiting.recv().map(|d| d.data), start.elapsed())
        });
        std::thread::sleep(Duration::from_millis(100));
        consumer.interrupt_recv();
        let (got, waited) = t.join().unwrap();
        assert_eq!(got, Err(RecvError::Timeout));
        assert!(waited < Duration::from_secs(5), "waited {waited:?}");
        owner.send(&Locator::shm(id(239)), b"after").unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"after");
    }

    /// §5.4: two users that look the segment up in different flink
    /// directories (another TMPDIR, a service's private /tmp) share it.
    /// Before, the second found no flink, unlinked the live segment and
    /// made its own: the owner wrote into one ring, the consumer read
    /// another.
    #[cfg(target_os = "linux")]
    #[test]
    fn users_in_different_flink_directories_share_the_segment() {
        let (here, there) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let owner =
            PosixShmTransport::open_owner(id(240), id(241), cfg_tmp(here.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(241), id(240), cfg_tmp(there.path(), 4096))
                .unwrap();
        owner.send(&Locator::shm(id(241)), b"one ring").unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"one ring");
    }

    /// §5.5: a consumer that finds the segment of an ended session under
    /// its pair's name makes it anew. Before, it attached and delivered the
    /// frame the ended owner had left in the ring. On Windows (§6.3) the
    /// mapping goes with the ended process's last handle, and the same
    /// name opens fresh.
    #[test]
    fn an_orphaned_segment_is_made_anew_not_attached() {
        let tmp = tempfile::tempdir().unwrap();
        leave_orphan(id(236), id(237), tmp.path());
        let consumer =
            PosixShmTransport::open_consumer(id(237), id(236), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(consumer.occupied_bytes(), 0, "a fresh ring");
        let owner =
            PosixShmTransport::open_owner(id(236), id(237), cfg_tmp(tmp.path(), 4096)).unwrap();
        owner.send(&Locator::shm(id(237)), b"this session").unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"this session");
    }

    /// A `.lock` file without a flink beside it goes once nobody holds it;
    /// a young one and one whose pair lock is held stay, and so does the
    /// lock file of an open pair.
    #[cfg(unix)]
    #[test]
    fn the_pair_file_sweep_removes_lone_lock_files_and_keeps_the_rest() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let lone = dir.join("aa-bb.lock");
        std::fs::write(&lone, b"").unwrap();
        let held = dir.join("cc-dd.lock");
        std::fs::write(&held, b"").unwrap();
        let held_guard = lock_pair(&held).unwrap();

        let report = sweep_stale_pair_files(dir, Duration::from_secs(3600)).unwrap();
        assert_eq!(report.removed, 0, "{report:?}");
        let report = sweep_stale_pair_files(dir, Duration::ZERO).unwrap();
        assert!(!lone.exists(), "{report:?}");
        assert!(held.exists(), "{report:?}");
        assert_eq!(report.held, 1, "{report:?}");
        drop(held_guard);

        // The live pair: its lock file stays while the pair is open, and an
        // open after the sweep took a lone lock file works.
        let owner = PosixShmTransport::open_owner(id(24), id(25), cfg_tmp(dir, 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(25), id(24), cfg_tmp(dir, 4096)).unwrap();
        sweep_stale_pair_files(dir, Duration::ZERO).unwrap();
        assert!(
            segment_flink(dir, id(24), id(25))
                .with_extension("lock")
                .exists()
        );
        owner
            .send(&Locator::shm(id(25)), b"after the sweep")
            .unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"after the sweep");
    }

    /// An opener that waited on a lock file someone removed meanwhile does
    /// not keep that lock: it locks the file now under the name.
    #[cfg(unix)]
    #[test]
    fn a_lock_taken_on_a_removed_lock_file_is_taken_anew() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("pair.lock");
        let first = lock_pair(&path).unwrap();
        let waiter = {
            let path = path.clone();
            std::thread::spawn(move || lock_pair(&path).map(|g| g.file))
        };
        std::thread::sleep(Duration::from_millis(50));
        std::fs::remove_file(&path).unwrap();
        drop(first);
        let file = waiter.join().unwrap().unwrap();
        use std::os::unix::fs::MetadataExt;
        let held = file.metadata().unwrap();
        let named = std::fs::metadata(&path).unwrap();
        assert_eq!((held.dev(), held.ino()), (named.dev(), named.ino()));
    }

    /// §5.5: the sweep unlinks a segment no process holds, leaves a
    /// young one alone, and keeps one that is held.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_sweep_removes_orphaned_segments_and_keeps_held_ones() {
        let tmp = tempfile::tempdir().unwrap();
        leave_orphan(id(232), id(233), tmp.path());
        let orphan = format!("{SEGMENT_NAME_PREFIX}{}", hex16(id(232)));
        let report =
            sweep_segments(&orphan, Duration::from_secs(3600), ring_header_is_current).unwrap();
        assert_eq!((report.young, report.removed), (1, 0), "{report:?}");
        let report = sweep_segments(&orphan, Duration::ZERO, ring_header_is_current).unwrap();
        assert_eq!((report.removed, report.held), (1, 0), "{report:?}");
        let name = segment_os_id(id(232), id(233));
        assert!(!Path::new(DEV_SHM).join(&name[1..]).exists());

        let owner =
            PosixShmTransport::open_owner(id(234), id(235), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(235), id(234), cfg_tmp(tmp.path(), 4096)).unwrap();
        let held = format!("{SEGMENT_NAME_PREFIX}{}", hex16(id(234)));
        let report = sweep_segments(&held, Duration::ZERO, ring_header_is_current).unwrap();
        assert_eq!((report.removed, report.held), (0, 1), "{report:?}");
        owner.send(&Locator::shm(id(235)), b"still bound").unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"still bound");

        // A segment whose creator ended before it wrote the header.
        let blank = format!("{SEGMENT_NAME_PREFIX}{}-blank", hex16(id(236)));
        std::fs::write(Path::new(DEV_SHM).join(&blank), [0u8; 64]).unwrap();
        let report = sweep_segments(&blank, Duration::ZERO, ring_header_is_current).unwrap();
        assert_eq!((report.removed, report.foreign), (1, 0), "{report:?}");
        assert!(!Path::new(DEV_SHM).join(&blank).exists());
    }

    /// A ring of an earlier layout version takes no lock: it is unlinked
    /// when no process maps it and stays while one does.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_ring_of_an_earlier_layout_goes_unless_a_process_maps_it() {
        use std::os::unix::io::AsRawFd;
        let prefix = format!("{SEGMENT_NAME_PREFIX}oldlayout-{}-", std::process::id());
        let mut header = [0u8; 64];
        header[..4].copy_from_slice(&SHM_MAGIC.to_be_bytes());
        header[4..8].copy_from_slice(&1u32.to_le_bytes());
        let orphan = Path::new(DEV_SHM).join(format!("{prefix}a"));
        let mapped = Path::new(DEV_SHM).join(format!("{prefix}b"));
        std::fs::write(&orphan, header).unwrap();
        std::fs::write(&mapped, header).unwrap();
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(&mapped)
            .unwrap();
        // SAFETY: a shared read-only mapping of a 64-byte file this test
        // owns, unmapped below.
        let at = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                64,
                libc::PROT_READ,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            )
        };
        assert_ne!(at, libc::MAP_FAILED);

        let report = sweep_old_layout_segments(&prefix, Duration::ZERO).unwrap();
        if dev_shm_mappings().is_none() {
            // Not root, on a host where a root process's maps cannot be
            // read (a GitHub runner): the sweep cannot tell whether a ring
            // is mapped and keeps both. The removal is measured where the
            // tests run as root (the GitLab image).
            assert_eq!((report.removed, report.held), (0, 2), "{report:?}");
            assert!(orphan.exists() && mapped.exists());
            // SAFETY: the mapping made above.
            unsafe { libc::munmap(at, 64) };
            std::fs::remove_file(&orphan).unwrap();
            std::fs::remove_file(&mapped).unwrap();
            return;
        }
        assert_eq!((report.removed, report.held), (1, 1), "{report:?}");
        assert!(!orphan.exists());
        assert!(mapped.exists());

        // SAFETY: the mapping made above.
        unsafe { libc::munmap(at, 64) };
        let report = sweep_old_layout_segments(&prefix, Duration::ZERO).unwrap();
        assert_eq!((report.removed, report.held), (1, 0), "{report:?}");
        assert!(!mapped.exists());
    }

    /// §5.5 on macOS: the flink sweep unlinks a segment no process holds,
    /// leaves a young one alone, and keeps one that is held.
    #[cfg(all(unix, not(target_os = "linux")))]
    #[test]
    fn the_flink_sweep_removes_orphaned_segments_and_keeps_held_ones() {
        let orphaned = tempfile::tempdir().unwrap();
        leave_orphan(id(242), id(243), orphaned.path());
        let report = sweep_orphaned_flinks(orphaned.path(), Duration::from_secs(3600)).unwrap();
        assert_eq!((report.young, report.removed), (1, 0), "{report:?}");
        let report = sweep_orphaned_flinks(orphaned.path(), Duration::ZERO).unwrap();
        assert_eq!((report.removed, report.held), (1, 0), "{report:?}");
        assert!(!segment_flink(orphaned.path(), id(242), id(243)).exists());
        assert!(
            Mapping::open_os_id(&segment_os_id(id(242), id(243))).is_err(),
            "the segment is unlinked"
        );

        let held = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(244), id(245), cfg_tmp(held.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(245), id(244), cfg_tmp(held.path(), 4096)).unwrap();
        let report = sweep_orphaned_flinks(held.path(), Duration::ZERO).unwrap();
        assert_eq!((report.removed, report.held), (0, 1), "{report:?}");
        owner.send(&Locator::shm(id(245)), b"still bound").unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"still bound");
    }

    /// §5.1: the os_id is a pure function of the pair and stays inside
    /// the platform's name limit (macOS `PSHMNAMLEN` = 31).
    /// No two tests may name the same `id()` pair.
    ///
    /// `segment_os_id` is a function of the pair alone and the segment lives
    /// outside the process, so two tests holding the same pair share one
    /// mapping. They then race over its capacity: the first fixes the size and
    /// the second gets `mapped segment smaller than requested capacity`. It has
    /// happened twice — 20/21 between this file's early tests, and 220/221
    /// between `full_ring_returns_an_error_instead_of_dropping` and
    /// `open_consumer_first_then_owner_roundtrip`, which only Windows ever
    /// failed on. Both times a reader had to find it in a scheduler's timing.
    ///
    /// Reading the source is the only place the question can be asked: the ids
    /// are literals spread over sixty tests, and nothing at run time sees more
    /// than its own.
    #[test]
    fn test_ids_are_not_shared_between_tests() {
        use std::collections::{BTreeMap, BTreeSet};
        let src = include_str!("posix.rs");
        let mut per_test: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
        let mut consts: BTreeMap<String, u32> = BTreeMap::new();
        let mut current = String::new();
        for line in src.lines() {
            let t = line.trim_start();
            if let Some(rest) = t.strip_prefix("fn ") {
                if let Some(name) = rest.split('(').next() {
                    current = name.to_string();
                    consts.clear();
                }
            }
            if let Some(rest) = t.strip_prefix("const ") {
                if let Some((name, value)) = rest.split_once(": u8 = ") {
                    if let Ok(v) = value.trim_end_matches(';').trim().parse::<u32>() {
                        consts.insert(name.trim().to_string(), v);
                    }
                }
            }
            let mut rest = line;
            while let Some(at) = rest.find("id(") {
                rest = &rest[at + 3..];
                let Some(close) = rest.find(')') else { break };
                let token = rest[..close].trim();
                let value = token
                    .parse::<u32>()
                    .ok()
                    .or_else(|| consts.get(token).copied());
                if let Some(v) = value {
                    if !current.is_empty() {
                        per_test.entry(current.clone()).or_default().insert(v);
                    }
                }
                rest = &rest[close..];
            }
        }
        let mut by_ids: BTreeMap<Vec<u32>, Vec<String>> = BTreeMap::new();
        for (test, ids) in &per_test {
            if ids.is_empty() {
                continue;
            }
            by_ids
                .entry(ids.iter().copied().collect())
                .or_default()
                .push(test.clone());
        }
        let shared: Vec<String> = by_ids
            .iter()
            .filter(|(_, tests)| tests.len() > 1)
            .map(|(ids, tests)| format!("{ids:?} in {}", tests.join(", ")))
            .collect();
        assert!(
            shared.is_empty(),
            "these id sets are named by more than one test: {}",
            shared.join("; ")
        );
        assert!(
            per_test.len() > 20,
            "the scan found only {} tests — the parser stopped seeing them",
            per_test.len()
        );
    }

    /// §5.1: the short name (macOS) depends on every byte of both ids.
    /// Two pairs of different participants whose endpoints carry the same
    /// entity ids got one name before and shared a segment.
    #[test]
    fn short_names_of_pairs_that_differ_only_in_the_prefix_differ() {
        let guid = |prefix: u8, entity: [u8; 4]| {
            let mut g = [prefix; 16];
            g[12..].copy_from_slice(&entity);
            g
        };
        let (writer, reader) = ([0, 0, 1, 3], [0, 0, 1, 4]);
        let first = segment_os_id_in(guid(1, writer), guid(2, reader), true);
        let second = segment_os_id_in(guid(3, writer), guid(4, reader), true);
        assert_ne!(first, second, "same entity ids, other participants");
        assert_eq!(
            first,
            segment_os_id_in(guid(1, writer), guid(2, reader), true)
        );
        assert_ne!(
            first,
            segment_os_id_in(guid(2, reader), guid(1, writer), true),
            "direction matters"
        );
        assert!(first.len() <= 31, "PSHMNAMLEN is 31, got {first}");
        assert!(first.starts_with("/zd-"));
        let long = segment_os_id_in(guid(1, writer), guid(2, reader), false);
        assert_eq!(long.len(), 4 + 32 + 1 + 32);
    }

    #[test]
    fn segment_os_id_is_deterministic_and_within_platform_limit() {
        let a = segment_os_id(id(190), id(191));
        assert_eq!(a, segment_os_id(id(190), id(191)), "same pair, same name");
        assert_ne!(a, segment_os_id(id(191), id(190)), "direction matters");
        assert!(a.starts_with('/'), "posix shm name starts with a slash");
        #[cfg(target_os = "macos")]
        assert!(a.len() <= 31, "PSHMNAMLEN is 31, got {} in {a}", a.len());
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

    /// DDSI-RTPS 2.5 §8.4.14.1: the transport states its datagram limit.
    #[test]
    fn max_message_size_is_the_datagram_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_tmp(tmp.path(), 4096);
        let want = cfg.max_datagram;
        let owner = PosixShmTransport::open_owner(id(41), id(42), cfg).unwrap();
        assert_eq!(Transport::max_message_size(&owner), want);
    }
}
