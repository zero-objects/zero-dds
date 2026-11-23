// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! POSIX SHM transport.
//!
//! Cross-process shared-memory transport via `shm_open` + `mmap` on Unix
//! (through `crate::mapping`, which keeps no descriptor per mapping) and
//! `CreateFileMapping` without a name over a file per segment on Windows
//! (`crate::mapping` as well). Linux is the primary platform; macOS, the other
//! Unixes and Windows run the same protocol: the pair lock is `flock` on
//! Unix and `LockFileEx` on Windows (`file_lock`), and where a segment
//! is no file that takes a lock (all but Linux) its users hold its hold
//! file in [`pair_dir`].
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
//!   offset 44:  reserved
//!   offset 48:  session_head: u64 (§6.4: where the owner of a caller's
//!               region began its session; 0 in a named segment)
//!   offset 56:  reserved (padding to the 64-byte cache line)
//!   offset 64:  data region [capacity bytes]
//! ```
//!
//! The header, the frames and the checks are [`crate::ring`]'s, which the
//! `no_std` transport over a caller's region shares.
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
//! The ring's accesses are [`crate::ring`]'s. Here: the lock and identity
//! calls of the platform, the probe of a segment's header in a sweep, and
//! the `Send`/`Sync` of the transport.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[cfg(windows)]
use crate::mapping::backing_dir;
use crate::mapping::{Mapping, ShmemError, remove_name};

/// Holds the exclusive lock on a pair's `.lock` file: the kernel releases
/// it when the handle closes, on the drop of the `File` or the end of the
/// process.
pub(crate) struct FlockGuard {
    #[allow(dead_code)] // hold-only; drop releases the lock
    file: std::fs::File,
}

/// Whole-file advisory locks, released when the handle closes or the
/// process ends, by `SIGKILL` or `TerminateProcess` too: `flock` on Unix,
/// `LockFileEx` over the whole file on Windows. Both lock per open handle:
/// two handles of one process exclude each other as two processes do.
pub(crate) mod file_lock {
    use std::io;

    /// The exclusive lock, waiting until nobody else holds one.
    pub(crate) fn exclusive(file: &std::fs::File) -> io::Result<()> {
        lock(file, Kind::Exclusive, true).map(|_| ())
    }

    /// The exclusive lock if nobody holds the file, without waiting.
    pub(crate) fn try_exclusive(file: &std::fs::File) -> io::Result<bool> {
        lock(file, Kind::Exclusive, false)
    }

    /// A shared lock, waiting out an exclusive one.
    pub(crate) fn shared(file: &std::fs::File) -> io::Result<()> {
        lock(file, Kind::Shared, true).map(|_| ())
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Kind {
        Shared,
        Exclusive,
    }

    /// `Ok(false)`: `wait` was not set and someone holds a conflicting
    /// lock.
    #[cfg(unix)]
    fn lock(file: &std::fs::File, kind: Kind, wait: bool) -> io::Result<bool> {
        use std::os::fd::AsRawFd;
        let mut op = match kind {
            Kind::Shared => libc::LOCK_SH,
            Kind::Exclusive => libc::LOCK_EX,
        };
        if !wait {
            op |= libc::LOCK_NB;
        }
        loop {
            // SAFETY: flock on the valid descriptor of the borrowed file.
            if unsafe { libc::flock(file.as_raw_fd(), op) } == 0 {
                return Ok(true);
            }
            let e = io::Error::last_os_error();
            match e.raw_os_error() {
                Some(libc::EINTR) => {}
                Some(libc::EWOULDBLOCK) if !wait => return Ok(false),
                _ => return Err(e),
            }
        }
    }

    #[cfg(windows)]
    fn lock(file: &std::fs::File, kind: Kind, wait: bool) -> io::Result<bool> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
        use windows_sys::Win32::Storage::FileSystem::{
            LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx,
        };
        use windows_sys::Win32::System::IO::OVERLAPPED;
        let mut flags = 0;
        if kind == Kind::Exclusive {
            flags |= LOCKFILE_EXCLUSIVE_LOCK;
        }
        if !wait {
            flags |= LOCKFILE_FAIL_IMMEDIATELY;
        }
        // SAFETY: a zeroed OVERLAPPED requests the range from offset 0.
        let mut ov: OVERLAPPED = unsafe { core::mem::zeroed() };
        // SAFETY: the handle is valid while `file` is borrowed; the
        // OVERLAPPED outlives the call, which does not run asynchronously on
        // a handle opened without FILE_FLAG_OVERLAPPED. The maximal range
        // covers the whole file, as flock does.
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
        if !wait && e.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
            return Ok(false);
        }
        Err(e)
    }
}

/// Takes the exclusive lock on a pair's `.lock` file. An owner's drop and
/// [`sweep_stale_pair_files`] remove that file; one who opened it before
/// and waited for the lock then holds the lock of a file that is gone,
/// while the next opener makes a new one and locks that. So after the lock
/// the path has to name the locked file still, or it is opened again.
///
/// Where Windows deletes the old way, a removed lock file keeps its name
/// until its last handle closes, and an open of the name is refused
/// (`ERROR_ACCESS_DENIED`) while other openers that waited on it still hold
/// it; each of them lets go after its look at the name, so the open is
/// tried again for up to 5 s.
fn lock_pair(lock_path: &Path) -> io::Result<FlockGuard> {
    #[cfg(windows)]
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let file = match open_pair_file(lock_path) {
            Ok(file) => file,
            #[cfg(windows)]
            Err(e)
                if e.raw_os_error()
                    == Some(windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED as i32)
                    && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(e) => return Err(e),
        };
        file_lock::exclusive(&file)?;
        if still_named(&file, lock_path)? {
            return Ok(FlockGuard { file });
        }
    }
}

/// Whether `path` names the open `file` still: device and inode on Unix.
#[cfg(unix)]
fn still_named(file: &std::fs::File, path: &Path) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let held = file.metadata()?;
    match std::fs::symlink_metadata(path) {
        Ok(named) => Ok(named.dev() == held.dev() && named.ino() == held.ino()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Whether `path` names the open `file` still: the held file is not
/// marked for deletion, and the volume serial number and the file index of
/// both handles agree. A removed file keeps its name until its last handle
/// closes where Windows deletes the old way (and under Wine), and a new
/// open of that name is refused; where it deletes the POSIX way the name
/// goes at once. Before, Windows answered yes without a look, and an opener
/// that waited on a lock file an owner's drop or a sweep removed kept the
/// lock of a file nobody else locks any more.
#[cfg(windows)]
fn still_named(file: &std::fs::File, path: &Path) -> io::Result<bool> {
    use std::os::windows::fs::OpenOptionsExt;
    if delete_pending(file)? {
        return Ok(false);
    }
    // No access right is needed to read a file's identity; a reparse point
    // under the name is opened as itself, not followed.
    let named = match std::fs::OpenOptions::new()
        .access_mode(0)
        .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
    {
        Ok(f) => f,
        // Gone, or another file under the name that is marked for deletion
        // (ours is not, see above).
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            return Ok(false);
        }
        Err(e) => return Err(e),
    };
    Ok(file_identity(file)? == file_identity(&named)?)
}

/// Whether the open `file` is marked for deletion: removed, and kept until
/// its last handle closes.
#[cfg(windows)]
fn delete_pending(file: &std::fs::File) -> io::Result<bool> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_STANDARD_INFO, FileStandardInfo, GetFileInformationByHandleEx,
    };
    // SAFETY: an all-zero FILE_STANDARD_INFO is a valid value of the plain
    // struct; the call fills it.
    let mut info: FILE_STANDARD_INFO = unsafe { core::mem::zeroed() };
    // SAFETY: the handle is valid while `file` is borrowed; `info` is
    // writable for the size passed.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle() as _,
            FileStandardInfo,
            (&mut info as *mut FILE_STANDARD_INFO).cast(),
            core::mem::size_of::<FILE_STANDARD_INFO>() as u32,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info.DeletePending != 0)
}

/// The volume serial number and the 64-bit file index of an open file:
/// what tells two names of one file from two files on Windows.
#[cfg(windows)]
fn file_identity(file: &std::fs::File) -> io::Result<(u32, u32, u32)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    // SAFETY: an all-zero BY_HANDLE_FILE_INFORMATION is a valid value of
    // the plain struct; the call fills it.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { core::mem::zeroed() };
    // SAFETY: the handle is valid while `file` is borrowed, `info` is
    // writable for the call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ))
}

/// Opens or makes one of a pair's files in [`pair_dir`], never through a
/// symlink: on Unix readable and writable by this user only (on Linux the
/// directory is `/dev/shm`, which every user can write to); on Windows the
/// file takes the access list of the pair directory, and a reparse point
/// under the name (a symbolic link, a junction) is opened as itself and
/// refused.
fn open_pair_file(path: &Path) -> io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    #[cfg(windows)]
    if is_reparse_point(&file.metadata()?) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is a reparse point, not a file", path.display()),
        ));
    }
    Ok(file)
}

/// Whether `meta` is that of a reparse point: a symbolic link, a junction
/// or another object Windows resolves to something else.
#[cfg(windows)]
fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
        != 0
}

/// Where the lock that serializes the openers of a segment lies
/// (`<name>.lock`, §5.4), and, where a segment is no file that takes a
/// lock, the file its users hold while they use it (`<name>.hold`, §5.5).
/// The place depends on the segment's name alone: two users of one
/// segment that look it up in different flink directories (another
/// `TMPDIR`, a service's private `/tmp`) take the same lock. Before, the
/// lock was `<flink>.lock`, and openers in different directories were not
/// serialized at all.
///
/// Linux: `/dev/shm`, beside the segments. A segment is mode 0600, so the
/// two sides of a pair run under one user; elsewhere the directory is that
/// user's: on macOS `zerodds-shm` in the user's temporary directory as the
/// system names it (`confstr(_CS_DARWIN_USER_TEMP_DIR)`, not `TMPDIR`), on
/// the other Unixes `/tmp/zerodds-shm-<euid>`, on Windows `zerodds-shm` in
/// the temporary directory.
///
/// # Errors
/// The directory cannot be made, or one that is there is not this user's
/// alone (on Unix: another owner, a symlink, write access for group or
/// others; on Windows: a reparse point).
pub fn pair_dir() -> io::Result<PathBuf> {
    pair_dir_of_this_target()
}

#[cfg(target_os = "linux")]
fn pair_dir_of_this_target() -> io::Result<PathBuf> {
    Ok(PathBuf::from(DEV_SHM))
}

#[cfg(target_os = "macos")]
fn pair_dir_of_this_target() -> io::Result<PathBuf> {
    private_dir(&darwin_user_temp_dir()?.join("zerodds-shm"))
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn pair_dir_of_this_target() -> io::Result<PathBuf> {
    // SAFETY: geteuid has no preconditions and cannot fail.
    let uid = unsafe { libc::geteuid() };
    private_dir(Path::new(&format!("/tmp/zerodds-shm-{uid}")))
}

/// Windows: `zerodds-shm` in the temporary directory, made if it is
/// missing; one that is there must be a directory and no reparse point.
/// Its owner and access list are not checked: it takes those of the
/// temporary directory, by default the user's own in the profile, which
/// admits the user, `SYSTEM` and the administrators.
#[cfg(windows)]
fn pair_dir_of_this_target() -> io::Result<PathBuf> {
    plain_dir(&std::env::temp_dir().join("zerodds-shm"))
}

/// `dir`, made if it is missing; one that is there must be a directory
/// and no reparse point.
#[cfg(windows)]
fn plain_dir(dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let meta = std::fs::symlink_metadata(dir)?;
    if !meta.is_dir() || is_reparse_point(&meta) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a plain directory", dir.display()),
        ));
    }
    Ok(dir.to_path_buf())
}

/// The user's temporary directory as macOS names it, whatever `TMPDIR`
/// says.
#[cfg(target_os = "macos")]
fn darwin_user_temp_dir() -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let mut buf = vec![0u8; 1024];
    // SAFETY: `buf` is writable for its length; confstr writes at most
    // that many bytes, the NUL included.
    let n = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_TEMP_DIR,
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    if n == 0 || n > buf.len() {
        return Err(io::Error::other(
            "confstr(_CS_DARWIN_USER_TEMP_DIR) gave no directory",
        ));
    }
    buf.truncate(n - 1);
    Ok(PathBuf::from(std::ffi::OsString::from_vec(buf)))
}

/// `dir`, made with mode 0700 if it is missing. One that is there must be
/// a directory (not a symlink) of this user that group and others cannot
/// write to, or the pair's files there could be another user's; read and
/// search access for group and others is taken away, so that the
/// directory is private afterwards.
///
/// On macOS the pair directory is also the root of the flink directories
/// `zerodds-dcps` makes in `TMPDIR` (`zerodds-shm/<host>/d<domain>`), and
/// `create_dir_all` makes a missing root with the mode the umask leaves,
/// 0755 with 022. When a flink directory came first, every later open
/// failed for as long as the directory stayed, although no other user
/// could write to it (on the macOS runner every open failed with this
/// refusal, job 238161). The owner, the type and the mode are read from,
/// and the mode set on, the inode opened without following a symlink.
#[cfg(any(test, all(unix, not(target_os = "linux"))))]
#[cfg(unix)]
fn private_dir(dir: &Path) -> io::Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    // SAFETY: geteuid has no preconditions and cannot fail.
    let uid = unsafe { libc::geteuid() };
    let refused = || {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a directory private to user {uid}", dir.display()),
        )
    };
    // With `O_NOFOLLOW` and `O_DIRECTORY` a symlink and anything but a
    // directory fail the open; the error number differs between the
    // systems, so the path says what was there.
    let handle = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open(dir)
        .map_err(|e| match std::fs::symlink_metadata(dir) {
            Ok(meta) if !meta.is_dir() => refused(),
            _ => e,
        })?;
    let meta = handle.metadata()?;
    if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o022 != 0 {
        return Err(refused());
    }
    if meta.mode() & 0o077 != 0 {
        handle.set_permissions(std::fs::Permissions::from_mode(meta.mode() & 0o7700))?;
    }
    Ok(dir.to_path_buf())
}

/// The file `<name>.<ext>` of segment `os_id` in [`pair_dir`].
pub(crate) fn pair_file(os_id: &str, ext: &str) -> io::Result<PathBuf> {
    Ok(pair_dir()?.join(format!("{}.{ext}", os_id.trim_start_matches('/'))))
}

/// Removes what pairs leave in `flink_dir` once their segments are gone:
/// on Linux flinks whose segment no longer exists in `/dev/shm` (its sweep
/// unlinks segments, not flinks), and `.lock` files without a flink beside
/// them, which openers of the earlier protocol took there (the pair lock
/// lies in [`pair_dir`] now). A process that ended without its drop
/// (killed), a consumer, and a superseded owner left them, and they piled
/// up (a test chain on a MacBook Air: 3 504 `.lock` files in one domain's
/// directory). A file younger than `min_age` and one whose pair lock
/// someone holds stay. A flink whose content is not a segment name in the
/// shape `segment_os_id` gives counts as foreign and stays: the path of
/// the lock file is built from that name, and a name such as
/// `/zd-x/../../../<dir>/<file>` led it out of `/dev/shm` (the flink
/// directory lies in `TMPDIR`).
///
/// # Errors
/// Reading `flink_dir` failed; a file that cannot be opened is skipped.
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
        // The name goes into the path of the pair's lock file: one of
        // another shape stays, and so does whatever its path points to.
        if !is_segment_os_id(&os_id) {
            report.foreign += 1;
            continue;
        }
        if Path::new(DEV_SHM).join(&os_id[1..]).exists() || !old(flink, &mut report) {
            continue;
        }
        // Under the pair's lock: an opener that makes the segment holds it
        // between writing the flink and taking its hold.
        let Some(lock) = pair_file(&os_id, "lock").ok() else {
            continue;
        };
        let Some(pair) = try_lock_pair(&lock) else {
            report.held += 1;
            continue;
        };
        if !Path::new(DEV_SHM).join(&os_id[1..]).exists() {
            let _ = remove_name(flink);
            let _ = remove_name(&lock);
            report.removed += 1;
        }
        drop(pair);
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
        let _ = remove_name(lock);
        drop(pair);
        report.removed += 1;
    }
    Ok(report)
}

/// The pair lock at `lock_path` if nobody holds it, without waiting; the
/// file is made if it is missing.
fn try_lock_pair(lock_path: &Path) -> Option<FlockGuard> {
    let file = open_pair_file(lock_path).ok()?;
    if !file_lock::try_exclusive(&file).ok()? {
        return None;
    }
    still_named(&file, lock_path)
        .ok()?
        .then_some(FlockGuard { file })
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
/// `shm_unlink` is idempotent: ENOENT is ignored. Every Unix has it (on
/// the Unixes but Linux and macOS this was a no-op, and only the path of
/// §5.5 removed a zombie there); Windows has its own cleanup semantics.
#[cfg(unix)]
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

/// Windows has no `shm_unlink`: a segment is a file in [`backing_dir`]
/// (`crate::mapping`), which outlives every handle, and an opener finds the
/// segment by that file. Removing the file's name is the unlink
/// ([`remove_name`]): the name goes at once, the mappings stay until their
/// handles close. Before, this was a no-op: the segment of an ended owner,
/// of an owner that had attached instead of made it, and of a
/// participant's doorbell a sender had made stayed on disk with the frames
/// in it.
#[cfg(windows)]
fn shm_unlink_by_os_id(os_id: &str) {
    let _ = remove_name(&backing_dir().join(os_id.trim_start_matches('/')));
}

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

/// Whether `os_id` has the shape [`segment_os_id_in`] gives a name: `/zd-`
/// and the two ids in lowercase hex (`<32>-<32>`), or the 26 hex digits of
/// the short form. A flink holds the name of its segment, and the sweeps
/// build the paths of the pair's files from it; a name of another shape
/// (`/zd-x/../../../<dir>/<file>`) could lead such a path out of its
/// directory.
fn is_segment_os_id(os_id: &str) -> bool {
    let Some(rest) = os_id
        .strip_prefix('/')
        .and_then(|s| s.strip_prefix(SEGMENT_PREFIX))
    else {
        return false;
    };
    let hex = |s: &str, digits: usize| {
        s.len() == digits && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    };
    match rest.split_once('-') {
        Some((owner, consumer)) => hex(owner, 32) && hex(consumer, 32),
        None => hex(rest, 26),
    }
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
enum SegmentHold {
    /// In use from here on; the file carries this process's shared lock
    /// on the segment.
    Held(std::fs::File),
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
fn hold_segment(os_id: &str, created: bool) -> io::Result<SegmentHold> {
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
    if !created && file_lock::try_exclusive(&file)? {
        return Ok(SegmentHold::Orphaned);
    }
    // Held shared by a user, or exclusively by a sweeper for as long as
    // it takes to unlink it: wait that out.
    file_lock::shared(&file)?;
    let held = file.metadata()?;
    match std::fs::symlink_metadata(&path) {
        Ok(named) if named.dev() == held.dev() && named.ino() == held.ino() => {
            Ok(SegmentHold::Held(file))
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
    // Closing the file releases a lock this probe took.
    !file_lock::try_exclusive(&file).unwrap_or(true)
}

/// Whether a process holds the segment `os_id`: its hold file in
/// [`pair_dir`] carries a lock (see [`hold_segment`]). A lock that cannot
/// be probed counts as held.
#[cfg(not(target_os = "linux"))]
fn segment_in_use(os_id: &str) -> bool {
    let Ok(path) = pair_file(os_id, "hold") else {
        return false;
    };
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    // Closing the file releases a lock this probe took.
    !file_lock::try_exclusive(&file).unwrap_or(false)
}

/// Where a segment is no file that takes a lock — macOS and the other
/// Unixes keep POSIX shared-memory objects out of the file system, and on
/// Windows a segment is a file in [`backing_dir`] that outlives the process
/// and is open while it is mapped — the lock goes on the segment's hold file
/// `<name>.hold` in [`pair_dir`], which every user finds by the segment's
/// name whatever its flink directory is: same protocol as the Linux one,
/// and [`sweep_pair_dir`] and [`sweep_orphaned_flinks`] read it. Before,
/// it lay on the pair's flink, and a user that looked the segment up in
/// another flink directory saw it as orphaned; on Windows an owner that
/// ended without its drop left its ring and its frames for the next
/// opener (nightly `windows-smoke`,
/// `shm_5_5_an_orphaned_segment_is_made_anew_not_attached`: 29 bytes of
/// the ended session).
#[cfg(not(target_os = "linux"))]
fn hold_segment(os_id: &str, created: bool) -> io::Result<SegmentHold> {
    let path = pair_file(os_id, "hold")?;
    let file = open_pair_file(&path)?;
    if !created && file_lock::try_exclusive(&file)? {
        return Ok(SegmentHold::Orphaned);
    }
    file_lock::shared(&file)?;
    // An owner's drop removes the hold file; one held after that names
    // nothing a later opener finds.
    if still_named(&file, &path)? {
        Ok(SegmentHold::Held(file))
    } else {
        Ok(SegmentHold::Vanished)
    }
}

#[cfg(test)]
thread_local! {
    /// Segments [`PosixShmTransport::open`] discarded as orphaned on this
    /// thread (§5.5): what tells the unlink before a create (§5.2) from
    /// that path, which reaches the same fresh ring.
    static ORPHANS_DISCARDED: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

/// The producer lock of the owners of segment `os_id` in this process.
/// A channel closed and opened again while a sender still held the old
/// one has two owners of one segment here (§5.3), and the ring has one
/// producer: the two share one lock, so the later one's epoch bump and
/// the earlier one's pushes do not interleave. Owners in different
/// processes would be two participants with one GUID prefix, which DDS
/// rules out.
fn owner_producer_lock(os_id: &str) -> std::sync::Arc<std::sync::Mutex<()>> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};
    static LOCKS: OnceLock<Mutex<HashMap<String, Weak<Mutex<()>>>>> = OnceLock::new();
    let mut locks = LOCKS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(lock) = locks.get(os_id).and_then(Weak::upgrade) {
        return lock;
    }
    // The entries of segments without an owner here go as well.
    locks.retain(|_, lock| lock.strong_count() > 0);
    let lock = Arc::new(Mutex::new(()));
    locks.insert(os_id.to_owned(), Arc::downgrade(&lock));
    lock
}

/// Removes an orphaned segment: its name, on Windows its backing file.
/// Only for a segment no process holds (see [`hold_segment`]).
fn discard_orphaned_segment(os_id: &str) {
    shm_unlink_by_os_id(os_id);
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
    /// segments would look orphaned. Also the flinks whose content is no
    /// segment name in the shape this version gives, left alone.
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
    let old = sweep_old_layout_segments(SEGMENT_NAME_PREFIX, min_age)?;
    Ok(current.and(old).and(sweep_pair_dir(min_age)?))
}

/// Removes what pairs left in [`pair_dir`] once their segments are gone or
/// orphaned, each under the pair's lock, so no opener is between its create
/// and its hold; a file younger than `min_age` stays. On Linux the `.lock`
/// files and the wait FIFOs (`.wake`, §4.4) whose segment is no longer in
/// `/dev/shm` (its sweep unlinks the segments). Elsewhere the `.hold` files
/// nobody holds, with the segment they name when it carries this layout
/// version (an orphan, §5.5; one of another version stays and counts as
/// `foreign`), the `.lock` files without a `.hold` beside them, and the
/// FIFOs whose segment is gone. The same for the doorbells' files. An
/// owner's drop removes them; a process that ended without it left them.
///
/// On Windows the backing files of the rings as well
/// (`sweep_backing_files`): a ring whose creator ended before it took
/// its hold has no hold file to be found by.
///
/// # Errors
/// [`pair_dir`] cannot be read; a file that cannot be opened is skipped.
pub fn sweep_pair_dir(min_age: Duration) -> io::Result<SweepReport> {
    let rings = sweep_pair_files(SEGMENT_PREFIX, min_age)?;
    let all = rings.and(sweep_pair_files(crate::bell::BELL_NAME_PREFIX, min_age)?);
    #[cfg(windows)]
    let all = all.and(sweep_backing_files(min_age)?);
    Ok(all)
}

/// The names of the ring segments start so (see `segment_os_id`).
const SEGMENT_PREFIX: &str = "zd-";

/// Windows: removes the rings in [`backing_dir`] no process holds, the
/// counterpart of the sweep over `/dev/shm` on Linux. A ring's backing
/// file outlives every handle (§6.3): one whose users all ended without
/// their drop keeps its frames and its size on disk until it is removed.
/// [`sweep_pair_dir`] finds such a ring through its hold file; this finds
/// the ring whose creator ended between making it and taking its hold,
/// which left no hold file. Each one is looked at under its pair's lock,
/// which an opener holds from before its create until after its hold. A
/// ring of another layout version stays (`foreign`), so does one younger
/// than `min_age`, and so do the files whose name was removed while a
/// handle held them: [`remove_name`] renames them to
/// `<name>.<pid>-<n>.gone`, `shared_memory` renamed them to
/// `<name>_deleted`; Windows deletes them with their last handle.
///
/// # Errors
/// [`backing_dir`] cannot be read; a file that cannot be opened is
/// skipped.
#[cfg(windows)]
pub fn sweep_backing_files(min_age: Duration) -> io::Result<SweepReport> {
    sweep_backing_files_under(SEGMENT_PREFIX, min_age)
}

/// [`sweep_backing_files`] over the rings whose name starts with `prefix`.
#[cfg(windows)]
fn sweep_backing_files_under(prefix: &str, min_age: Duration) -> io::Result<SweepReport> {
    let mut report = SweepReport::default();
    let now = std::time::SystemTime::now();
    let entries = match std::fs::read_dir(backing_dir()) {
        Ok(e) => e
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect::<Vec<_>>(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(report),
        Err(e) => return Err(e),
    };
    for path in entries {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with(SEGMENT_PREFIX)
            || !name.starts_with(prefix)
            || name.ends_with("_deleted")
            || name.contains(['.', '/', '\\'])
        {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_file() || is_reparse_point(&meta) {
            continue;
        }
        let changed = meta.modified().unwrap_or(now);
        if now.duration_since(changed).unwrap_or_default() < min_age {
            report.young += 1;
            continue;
        }
        let os_id = format!("/{name}");
        if segment_in_use(&os_id) {
            report.held += 1;
            continue;
        }
        let Ok(lock) = pair_file(&os_id, "lock") else {
            continue;
        };
        let Some(pair) = try_lock_pair(&lock) else {
            report.held += 1;
            continue;
        };
        if segment_in_use(&os_id) {
            report.held += 1;
            continue;
        }
        match segment_state(&os_id) {
            SegmentState::Other => {
                report.foreign += 1;
                continue;
            }
            SegmentState::Current => discard_orphaned_segment(&os_id),
            SegmentState::Missing => {}
        }
        if let Ok(hold) = pair_file(&os_id, "hold") {
            let _ = remove_name(&hold);
        }
        let _ = remove_name(&lock);
        drop(pair);
        report.removed += 1;
    }
    Ok(report)
}

/// [`sweep_pair_dir`] over the files whose name starts with `prefix`.
fn sweep_pair_files(prefix: &str, min_age: Duration) -> io::Result<SweepReport> {
    let dir = pair_dir()?;
    let mut report = SweepReport::default();
    let now = std::time::SystemTime::now();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect::<Vec<_>>(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(report),
        Err(e) => return Err(e),
    };
    for path in entries {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some((stem, ext)) = name.rsplit_once('.') else {
            continue;
        };
        if !name.starts_with(prefix) || !matches!(ext, "lock" | "hold" | "wake") {
            continue;
        }
        #[cfg(target_os = "linux")]
        let live = ext == "hold" || Path::new(DEV_SHM).join(stem).exists();
        #[cfg(not(target_os = "linux"))]
        let live = match ext {
            "lock" => dir.join(format!("{stem}.hold")).exists(),
            "wake" => segment_exists(&format!("/{stem}")),
            _ => false,
        };
        // Gone since the listing: removed with its pair's other file.
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if live || !is_pair_file_type(&meta, ext) {
            continue;
        }
        let changed = meta.modified().unwrap_or(now);
        if now.duration_since(changed).unwrap_or_default() < min_age {
            report.young += 1;
            continue;
        }
        let lock = dir.join(format!("{stem}.lock"));
        let Some(pair) = try_lock_pair(&lock) else {
            report.held += 1;
            continue;
        };
        // Looked at again under the lock.
        #[cfg(target_os = "linux")]
        if Path::new(DEV_SHM).join(stem).exists() {
            report.held += 1;
            continue;
        }
        #[cfg(not(target_os = "linux"))]
        {
            let os_id = format!("/{stem}");
            match ext {
                "hold" => {
                    if segment_in_use(&os_id) {
                        report.held += 1;
                        continue;
                    }
                    match segment_state(&os_id) {
                        SegmentState::Other => {
                            report.foreign += 1;
                            continue;
                        }
                        SegmentState::Current => discard_orphaned_segment(&os_id),
                        SegmentState::Missing => {}
                    }
                }
                "wake" if segment_exists(&os_id) => {
                    report.held += 1;
                    continue;
                }
                _ => {}
            }
        }
        if ext != "lock" {
            let _ = remove_name(&path);
        }
        // Removed under its own lock: a waiting opener finds the path gone
        // and opens it anew (see `lock_pair`).
        let _ = remove_name(&lock);
        drop(pair);
        report.removed += 1;
    }
    Ok(report)
}

/// Whether `meta` is of the type the pair file `<name>.<ext>` is made as:
/// a FIFO for `.wake` (§4.4, made with `mkfifo` by the FIFO backend of
/// `crate::wait`; Windows has none), a regular file for `.lock` and
/// `.hold`. Before, the sweep took regular files only and passed over
/// every wait FIFO, so the FIFO of a pair whose processes ended without
/// their drop stayed in the pair directory. Anything else under such a
/// name (a link, a directory) is not this crate's and stays.
fn is_pair_file_type(meta: &std::fs::Metadata, ext: &str) -> bool {
    match ext {
        #[cfg(unix)]
        "wake" => {
            use std::os::unix::fs::FileTypeExt;
            meta.file_type().is_fifo()
        }
        #[cfg(not(unix))]
        "wake" => false,
        _ => meta.is_file(),
    }
}

/// What a segment name names, for a sweep: a ring of this layout version,
/// something else, or nothing.
#[cfg(not(target_os = "linux"))]
enum SegmentState {
    Current,
    Other,
    Missing,
}

/// Whether a segment or doorbell `os_id` exists.
#[cfg(not(target_os = "linux"))]
fn segment_exists(os_id: &str) -> bool {
    !matches!(segment_state(os_id), SegmentState::Missing)
}

/// The state of segment `os_id`. One that exists but cannot be mapped
/// (another user's) counts as `Other`.
#[cfg(not(target_os = "linux"))]
fn segment_state(os_id: &str) -> SegmentState {
    match Mapping::open_os_id(os_id) {
        Err(e) if crate::mapping::is_missing(&e) => SegmentState::Missing,
        Err(_) => SegmentState::Other,
        Ok(_) if segment_is_current_version(os_id) => SegmentState::Current,
        Ok(_) => SegmentState::Other,
    }
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
        // A name with a dot is a pair's file beside the segments (§5.4).
        if !name.starts_with(prefix) || name.contains('.') {
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
        // A name with a dot is a pair's file beside the segments (§5.4).
        if !name.starts_with(prefix) || name.contains('.') {
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
        if !file_lock::try_exclusive(&file).unwrap_or(false) {
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
/// orphaned when nobody holds its hold file in [`pair_dir`] (see
/// [`hold_segment`]); it is unlinked and its flink, hold and lock files
/// removed under the pair's lock. A flink whose segment is gone is
/// removed as well. One whose flink changed more recently than `min_age`
/// and one of another layout version are left alone. A segment whose
/// flink went is found by [`sweep_pair_dir`] through its hold file.
///
/// # Errors
/// Reading `flink_dir` failed; a flink that cannot be opened is skipped.
#[cfg(not(target_os = "linux"))]
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
        let mut os_id = String::new();
        // The name goes into the paths of its pair files.
        if file.read_to_string(&mut os_id).is_err() || !is_segment_os_id(&os_id) {
            report.foreign += 1;
            continue;
        }
        if segment_in_use(&os_id) {
            report.held += 1;
            continue;
        }
        // Under the pair's lock: no opener is between its create and its
        // hold.
        let Ok(lock) = pair_file(&os_id, "lock") else {
            continue;
        };
        let Some(pair) = try_lock_pair(&lock) else {
            report.held += 1;
            continue;
        };
        if segment_in_use(&os_id) {
            report.held += 1;
            continue;
        }
        match segment_state(&os_id) {
            SegmentState::Other => {
                report.foreign += 1;
                continue;
            }
            SegmentState::Current => discard_orphaned_segment(&os_id),
            SegmentState::Missing => {}
        }
        let _ = remove_name(&path);
        if let Ok(hold) = pair_file(&os_id, "hold") {
            let _ = remove_name(&hold);
        }
        let _ = remove_name(&lock);
        drop(pair);
        report.removed += 1;
    }
    Ok(report)
}

/// One doorbell marker of [`sweep_orphaned_flinks`]: the bell it names
/// is unlinked and the marker removed when no participant holds the
/// marker's lock and it is older than `min_age`. A marker whose bell is
/// gone is removed too; one that names another layout is left.
#[cfg(not(target_os = "linux"))]
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
    if !file_lock::try_exclusive(&file).unwrap_or(false) {
        report.held += 1;
        return;
    }
    let os_id = format!("/{stem}");
    match bell_segment(&os_id) {
        BellSegment::Other => {
            report.foreign += 1;
            return;
        }
        BellSegment::Current => discard_orphaned_segment(&os_id),
        BellSegment::Missing => {}
    }
    let _ = remove_name(path);
    report.removed += 1;
}

/// Whether segment `os_id` carries this layout version. A segment that
/// cannot be mapped counts as not.
#[cfg(not(target_os = "linux"))]
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
            u32::from_be(core::ptr::read_unaligned(base as *const u32)),
            u32::from_le(core::ptr::read_unaligned(base.add(4) as *const u32)),
        )
    };
    magic == SHM_MAGIC && version == SHM_VERSION
}

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

pub use crate::ring::{HEADER_BYTES, MAX_CAPACITY, SHM_MAGIC, SHM_VERSION, ShmRole};
use crate::ring::{Next, Push, Ring};

/// Default capacity 1 MiB of the data region.
pub const DEFAULT_CAPACITY: usize = 1 << 20;

/// What a waiting `recv` answers when the frame it took off the ring was
/// corrupt (§3.3): the call cannot hand up a frame, and says why instead of
/// waiting on as if nothing had come.
const CORRUPT_FRAME_DROPPED: RecvError = RecvError::Io {
    message: "shm corrupt frame dropped",
};

/// What a push answers when a ring position it computed lies outside the
/// data region: not reached while `head` holds what this writer stored.
const OUTSIDE_THE_RING: SendError = SendError::Io {
    message: "shm ring position outside the ring",
};

/// Default spin-wait cycles before the sender comes back with
/// `Backpressure`.
const SPIN_LIMIT: u32 = 1024;

/// For [`crate::bell`]: the hash of the short names and
/// [`shm_unlink_by_os_id`].
pub(crate) fn fnv1a_128_bytes(data: &[u8]) -> [u8; 16] {
    fnv1a_128(data)
}

pub(crate) fn unlink_os_id(os_id: &str) {
    shm_unlink_by_os_id(os_id);
}

/// The pair lock of the segment or doorbell `os_id` (§5.4), for a doorbell
/// that makes its FIFO under it.
pub(crate) fn lock_pair_of(os_id: &str) -> io::Result<FlockGuard> {
    lock_pair(&pair_file(os_id, "lock")?)
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
    /// Data region size in bytes. Must be `>= max_datagram * 2 + 16` (so
    /// the ring can still hold a datagram and its frame header even with
    /// wraparound padding); `open_owner` and `open_consumer` refuse less.
    pub capacity: usize,
    /// Base directory for flink files (a meta file per segment that
    /// holds the OS segment name).
    pub flink_dir: PathBuf,
    /// Maximum datagram size; `send` rejects larger ones with
    /// `PayloadTooLarge`.
    pub max_datagram: usize,
    /// `recv` timeout. `None`: `recv` parks (§4.4) until a frame arrives,
    /// the owner's drop ends the segment, or
    /// [`PosixShmTransport::interrupt_recv`] is called.
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
    /// Config error: `capacity < max_datagram * 2 + 16`, or a size the
    /// layout cannot hold.
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

// SAFETY: PosixShmTransport is Send because
//  - `Mapping` contains a `*mut u8`; the pointer is valid in the same
//    address space across a thread switch (mmap lifetime bound to the
//    struct instance) and no other thread knows of it.
//  - `Ring` (the only field that dereferences the pointer) is Send.
//  - All other fields are inherently Send.
// Drop is not reentrant — the move between threads ends before
// `Drop::drop`.
unsafe impl Send for PosixShmTransport {}
// SAFETY: PosixShmTransport is Sync because
//  - `&self` methods (`send`, `recv`, `local_locator`, `peer_locator`,
//    counter reads) go exclusively through the `Ring`'s atomics and its
//    bounded copies, ordered by `head` and `tail` (see `crate::ring`).
//  - SPSC discipline (1 sender, 1 receiver) is the spec contract of the
//    ring buffer; concurrent senders/receivers on the same
//    PosixShmTransport are UB and excluded by the caller (DcpsRuntime
//    wave 4b).
unsafe impl Sync for PosixShmTransport {}

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
/// protocol via `Ring` is already secured by AcqRel atomics on
/// `head` and `tail` and the SPSC discipline (see `crate::ring`), and
/// all other fields (`role`, locators, `PathBuf`, `String`,
/// `AtomicU64`) are inherently `Send + Sync`. The `Mapping` pointer
/// itself stays valid as long as the `PosixShmTransport` object lives
/// (Drop does `munmap` exactly once). Cross-thread move or cross-thread
/// `&self` access are therefore safe, as long as the caller keeps the
/// SPSC discipline (max one sender thread, max one receiver thread;
/// that is the DCPS wave-4b architecture: hot-path sender in the tick
/// worker, recv worker in the dedicated `recv_user_shm_loop`).
pub struct PosixShmTransport {
    /// Keeps the mapping alive. `Some` for the life of the transport: an
    /// owner's drop takes it under the pair lock (see `Drop`).
    _shmem: Option<Mapping>,
    /// The shared lock on the segment that tells a sweeper this segment
    /// is in use (see `hold_segment`); released by the kernel when this
    /// process ends, however it ends.
    _liveness: std::fs::File,
    /// How the consumer waits on `head` and the owner wakes it (§4.4).
    waiter: crate::wait::Waiter,
    /// Parks of the consumer, for the tests that tell a park from a poll.
    #[cfg(test)]
    parks: core::sync::atomic::AtomicU64,
    /// The owner epoch this owner opened the segment under. Its drop ends
    /// the segment only while no later owner has opened it since.
    owner_epoch: u32,
    /// Serializes the senders of this process: the ring has one producer,
    /// and a same-host channel sends from the tick, the receive threads
    /// and the application's writers at once. Unserialized, two pushes
    /// raced on `head` (measured: `used > capacity` in a reliable burst).
    /// The owners of one segment in this process share it (see
    /// [`owner_producer_lock`]).
    producer: std::sync::Arc<std::sync::Mutex<()>>,
    /// `false`: a push does not wake a consumer parked on `head`, because
    /// the consumer parks on a [`crate::bell::ShmDoorbell`] instead.
    wake_on_push: core::sync::atomic::AtomicBool,
    layout: Ring,
    role: ShmRole,
    local_locator: Locator,
    peer_locator: Locator,
    config: ShmConfig,
    /// For the owner: the flink path we unlink on drop.
    /// The consumer leaves the path alone (the owner owns it).
    flink_path: PathBuf,
    /// The pair's lock file in [`pair_dir`], removed by the owner's drop.
    lock_path: PathBuf,
    /// OS id of the `/dev/shm` segment — we keep a copy so `Drop` can
    /// still call `shm_unlink` even on an internal mapping failure
    /// (crash-recovery defense-in-depth).
    os_id: String,
    /// Consumed padding frames since bind. See
    /// [`padding_frames_seen`](Self::padding_frames_seen).
    padding_counter: core::sync::atomic::AtomicU64,
    /// Corrupt-frame drops (§3.3, see [`Self::corrupt_frames_seen`]): the
    /// consumer counts the frame and resumes at `head`.
    corrupt_frame_counter: core::sync::atomic::AtomicU64,
    /// Set by [`Self::interrupt_recv`]: the next or the waiting `recv`
    /// returns `Timeout` at once. Process-local, not in the segment.
    interrupted: core::sync::atomic::AtomicBool,
    /// Set by the waiter from before its last look at `interrupted` until
    /// its park has returned. [`Self::interrupt_recv`] wakes until it is
    /// clear or the flag is taken. Process-local as well.
    parking: core::sync::atomic::AtomicBool,
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
            // The mapping and the hold go with the fields; a consumer
            // that made the segment does not unlink it (see `open`).
            return;
        }
        // An owner decides and ends under the pair lock (§5.3, §5.4): an
        // opener of the pair runs before all of it or after all of it.
        // Without the lock, a next owner that attached between the look at
        // `owner_epoch` and the unlink wrote into a ring this drop ended,
        // and one that made the ring anew in between lost its name to the
        // mapping's own drop, which ran after this body (measured on the
        // Linux test host: 16 of 20 runs of
        // `shm_5_3_a_drop_racing_the_next_owner_leaves_it_a_ring_that_is_read`
        // failed, 500 rounds each). A lock that cannot be taken (its
        // directory gone) leaves the drop as it was without it: `Drop`
        // cannot report.
        let _pair = lock_pair(&self.lock_path).ok();
        // An owner that re-opened the pair (a channel closed and opened
        // again while this one was still in use) attached to this very
        // segment and carries it on: ending it here would set the shutdown
        // flag and unlink the name under the live owner, which then writes
        // into a ring nobody reads.
        let ends = !self.is_superseded();
        if !ends {
            // Nor may the mapping unlink it, as the creator's would.
            if let Some(mapping) = self._shmem.as_mut() {
                mapping.set_owner(false);
            }
        } else {
            // 0. Set the shutdown flag before we tear down the
            //    resources. A consumer in a wait_for_frame loop sees the
            //    release store and returns a targeted error instead of
            //    running into the normal recv_timeout. One that opens the
            //    pair again waits for the pair lock and finds no ring.
            self.layout
                .shutdown()
                .store(1, core::sync::atomic::Ordering::Release);
            // Wake any consumer parked in `wait_for_frame` so it observes the
            // shutdown flag immediately instead of waiting out its
            // recv_timeout. The futex keys on `head` (the wait address); a
            // bare wake with no head change still releases the waiter, which
            // then re-checks shutdown. Then the FIFO goes, if this backend
            // keeps one: the consumer's descriptor still reads the byte.
            self.wake_ring();
            self.waiter.remove_files();
            // 1. Remove the flink file (lookup file, not a segment).
            let _ = remove_name(&self.flink_path);
            // 2. Where the segment is no file, remove its hold file: a user
            //    that still holds it keeps the lock on a name a later
            //    opener no longer finds.
            #[cfg(not(target_os = "linux"))]
            if let Ok(hold) = pair_file(&self.os_id, "hold") {
                let _ = remove_name(&hold);
            }
            // 3. Unlink the /dev/shm segment — defense-in-depth.
            //    The mapping does this itself when it is the owner (set
            //    in the `open()` path after `create`), but an explicit
            //    call is idempotent.
            shm_unlink_by_os_id(&self.os_id);
        }
        // The mapping goes while the lock is held: the creator's mapping
        // unlinks the name and removes the flink once more on its drop
        // (on Windows it marks the backing file for deletion), and after
        // the lock that reached a ring a next opener had made under the
        // same name.
        drop(self._shmem.take());
        // 4. The pair's lock file goes last, still held: an opener that
        //    comes now makes a new one and runs at once, and before the
        //    rest of the drop was done it ran beside it (the first form of
        //    this lock removed the file in step 2 and lost 5 of 30 runs of
        //    the test above). An opener waiting on the file finds it gone
        //    once it has the lock, and takes a new one (`lock_pair`).
        if ends {
            let _ = remove_name(&self.lock_path);
        }
    }
}

impl core::fmt::Debug for PosixShmTransport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PosixShmTransport")
            .field("role", &self.role)
            .field("local_locator", &self.local_locator)
            .field("peer_locator", &self.peer_locator)
            .field("capacity", &self.layout.capacity())
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
        if config.capacity < config.max_datagram.saturating_mul(2).saturating_add(16) {
            return Err(PosixShmError::InvalidConfig {
                reason: "capacity must be >= 2 * max_datagram + 16",
            });
        }
        if config.capacity > MAX_CAPACITY {
            return Err(PosixShmError::InvalidConfig {
                reason: "capacity must be <= 2^31 (positions compare by their low 32 bits)",
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
        // 1. flock excl on the pair's lock file in `pair_dir`, named after
        //    the segment alone → serializes every opener of the segment,
        //    whichever flink directory it looks in (§5.4)
        // 2. Try opening the existing flink → ATTACH (we are not creator)
        // 3. No flink, but a process holds the segment (its users found it
        //    through another flink directory) → ATTACH by name
        // 4. Otherwise → remove a zombie, CREATE + initialize the header
        // 5. Hold the segment (§5.5); flock release at the end of the block
        //
        // The role parameter is orthogonal: it only determines send/recv
        // semantics (the owner may send, the consumer may recv), not
        // who creates the segment.
        let lock_path = pair_file(&os_id, "lock")?;
        let _guard = lock_pair(&lock_path)?;

        // A segment no process holds any more is discarded and made
        // again, once; see `hold_segment`.
        let mut discarded = false;
        let requested = config;
        let (shmem, liveness, config, layout) = loop {
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
                // on the old ring and this one on a new one. Under the
                // pair lock no opener is between its create and its hold,
                // so a segment nobody holds here is orphaned.
                Err(_) if segment_in_use(&os_id) => (Mapping::open_os_id(&os_id)?, false),
                Err(_) => {
                    // Crash recovery: remove a zombie shm file from a
                    // previous crashed process.
                    shm_unlink_by_os_id(&os_id);
                    let _ = remove_name(&flink);
                    let fresh =
                        Mapping::create(&os_id, HEADER_BYTES + config.capacity, Some(&flink));
                    match fresh {
                        Ok(fresh) => (fresh, true),
                        // The name is there although nobody holds it and
                        // the unlink above ran: on Windows a kernel mapping
                        // object of that name lives until its last handle
                        // closes, whatever happened to its backing file, and
                        // a user of an earlier protocol takes no hold.
                        //
                        // That is not a failure. It says the segment exists,
                        // which is the same thing the `open` above was
                        // looking for and missed only because it looks by
                        // flink. Attach by os_id, and attach as the
                        // **non-creator**: something else made it and will
                        // unlink it, and a second owner would unlink twice.
                        // The header check and the hold below decide whether
                        // the ring is taken.
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

            let ring = if is_creator {
                // SAFETY: the mapping is `total_size` bytes long and lives
                // with `shmem`; it is zero as `create` made it, and we are
                // alone in the pair lock.
                let ring = unsafe { Ring::create_in_zeroed(base, total_size, config.capacity) }
                    .map_err(|_| PosixShmError::InvalidConfig {
                        reason: "mapped segment cannot hold the requested capacity",
                    })?;
                // Only an owner-role creator is the mapping's owner, which
                // unlinks the segment on drop.
                // A consumer that made the segment first must not: its drop
                // took the name from under the live owner, and a consumer
                // that opened the pair again made a ring of its own beside
                // the owner's. The attacher never does (double unlink).
                shmem.set_owner(role == ShmRole::Owner);
                ring
            } else {
                // Magic, version and a capacity the mapping holds. The ring
                // is as large as its creator made it. Both sides used their
                // own configured size before, and two sides that configured
                // different ones (`transport.shm_max_datagram` per
                // participant) either failed to attach or ran the ring with
                // two different capacities.
                // SAFETY: the mapping is `total_size` bytes long and lives
                // with `shmem`.
                let ring = unsafe { Ring::attach(base, total_size) }
                    .map_err(|_| PosixShmError::InvalidHeader)?;
                config.capacity = ring.capacity();
                if config.max_datagram.saturating_mul(2).saturating_add(16) > config.capacity {
                    config.max_datagram = (config.capacity - 16) / 2;
                }
                ring
            };

            match hold_segment(&os_id, is_creator)? {
                SegmentHold::Held(file) => break (shmem, file, config, ring),
                SegmentHold::Orphaned | SegmentHold::Vanished if !discarded => {
                    // Its ring still holds the frames of a session that is
                    // over; a consumer attaching to it would deliver them.
                    drop(shmem);
                    discard_orphaned_segment(&os_id);
                    let _ = remove_name(&flink);
                    discarded = true;
                    #[cfg(test)]
                    ORPHANS_DISCARDED.with(|n| n.set(n.get() + 1));
                }
                SegmentHold::Orphaned | SegmentHold::Vanished => {
                    return Err(PosixShmError::Io(io::Error::other(
                        "shm segment vanished again after it was made anew",
                    )));
                }
            }
        };
        // Under the pair lock: a sweep removes a FIFO whose segment is gone
        // under it. Before the two counts below: an open that fails here
        // leaves neither an attach nor an epoch behind.
        let waiter = crate::wait::Waiter::open(&os_id)?;
        if role == ShmRole::Consumer {
            // A count, not a flag: a consumer that re-opens the ring before
            // its old handle is gone would otherwise be marked detached by
            // that handle's drop.
            layout.consumer_attached().fetch_add(1, Ordering::AcqRel);
        }
        // The owners of the segment in this process share the producer
        // lock, and the epoch goes up under it: a send of the owner this
        // one supersedes ends before the bump, or sees it and refuses.
        let producer = if role == ShmRole::Owner {
            owner_producer_lock(&os_id)
        } else {
            std::sync::Arc::new(std::sync::Mutex::new(()))
        };
        let owner_epoch = if role == ShmRole::Owner {
            let _sends = producer
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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

        Ok(Self {
            _shmem: Some(shmem),
            _liveness: liveness,
            waiter,
            #[cfg(test)]
            parks: core::sync::atomic::AtomicU64::new(0),
            owner_epoch,
            producer,
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
            parking: core::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Makes the `recv` waiting on this segment, or the next one, return
    /// `Timeout` at once, so a receive thread that is told to stop does
    /// not wait out its `recv_timeout`. With it the timeout is no longer
    /// what bounds the stop, and can be long: a receive thread with a 1 ms
    /// timeout woke a thousand times a second with nothing to receive.
    pub fn interrupt_recv(&self) {
        self.interrupted.store(true, Ordering::SeqCst);
        // The waiter parks on `head`; a wake without a change of `head`
        // releases it to re-check, and it finds the flag.
        self.wake_ring();
        // The flag is not the word the waiter parks on, so one wake is not
        // enough: a waiter that looked at the flag before the store and
        // parks after the wake sleeps on, with no timeout for good. It
        // announces that span in `parking`; the wake is repeated until the
        // waiter has left the span or taken the flag.
        while self.parking.load(Ordering::SeqCst) && self.interrupted.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_micros(50));
            self.wake_ring();
        }
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
        // The segment is a later owner's (§5.3); a push of this one would
        // race that owner's on `head`.
        if self.is_superseded() {
            return Err(SendError::Io {
                message: "shm owner superseded: a later owner sends on this segment",
            });
        }
        self.push_frame(prefix, data)
    }

    /// Whether a later owner opened this owner's segment (`owner_epoch`,
    /// §2.1, §5.3): the segment is the later one's, a send of this one
    /// returns an error, and its drop leaves the segment alone. Always
    /// `false` for a consumer.
    #[must_use]
    pub fn is_superseded(&self) -> bool {
        self.role == ShmRole::Owner
            && self.layout.owner_epoch().load(Ordering::Acquire) != self.owner_epoch
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
        self.layout.tail_position()
    }

    /// Number of bytes in the ring not yet consumed by the reader.
    /// Diagnostic for backpressure monitoring.
    #[must_use]
    pub fn occupied_bytes(&self) -> u64 {
        self.layout.occupied()
    }

    fn push_frame(&self, prefix: &[u8], data: &[u8]) -> Result<(), SendError> {
        let mut spins: u32 = 0;
        loop {
            let full = match self.layout.try_push(prefix, data) {
                Push::Pushed => break,
                Push::Full => "shm ring full, reader too slow",
                Push::FullAtWrap => "shm ring full near wraparound",
                Push::OutsideTheRing => return Err(OUTSIDE_THE_RING),
                Push::TooLarge => {
                    return Err(SendError::PayloadTooLarge {
                        size: prefix.len() + data.len(),
                        limit: self.config.max_datagram,
                    });
                }
            };
            // The reader has not consumed enough: spin, then give up.
            spins = spins.saturating_add(1);
            if spins > SPIN_LIMIT {
                return Err(SendError::Io { message: full });
            }
            core::hint::spin_loop();
        }
        // Event-driven notify: the `head` store is published; wake a
        // consumer parked in `wait_for_frame` on the low 32 bits of `head`.
        if self.wake_on_push.load(Ordering::Relaxed) {
            self.wake_ring();
        }
        Ok(())
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

    /// Number of frames the consumer dropped as corrupt (§3.3): a length
    /// above half the ring, a frame that would cross the ring's end or the
    /// published `head`, a skip past `head`, or a `head` more than the ring
    /// ahead of `tail`. Non-zero indicates a broken or malicious owner; the
    /// DCPS receive paths file each drop as the event `shm.corrupt_frame`.
    /// Atomic, Relaxed ordering.
    #[must_use]
    pub fn corrupt_frames_seen(&self) -> u64 {
        self.corrupt_frame_counter
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    /// The name of the shared-memory object behind this transport, as
    /// `shm_open` takes it (`/zd-…`; on Linux the file `/dev/shm/zd-…`).
    #[must_use]
    pub fn segment_name(&self) -> &str {
        &self.os_id
    }

    /// Takes the next frame off the ring. Every value read from the ring
    /// (`head`, the length, a padding marker) is checked against what a
    /// writer of this protocol can publish before a byte is copied: the
    /// other side of the segment can write anything there (§3.3,
    /// [`Ring::next`]).
    fn pop_frame(&self) -> Option<Vec<u8>> {
        let mut paddings = 0;
        let next = self.layout.next(&mut paddings);
        if paddings > 0 {
            self.padding_counter
                .fetch_add(u64::from(paddings), Ordering::Relaxed);
        }
        match next {
            Next::Empty => None,
            Next::Corrupt => {
                self.corrupt_frame_counter.fetch_add(1, Ordering::Relaxed);
                None
            }
            Next::Frame { offset, len, end } => {
                let mut out = vec![0u8; len];
                if !self.layout.read(offset, &mut out) {
                    let _ = self.layout.drop_published();
                    self.corrupt_frame_counter.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
                self.layout.release(end);
                Some(out)
            }
        }
    }

    fn wait_for_frame(&self) -> Result<Vec<u8>, RecvError> {
        let deadline = self.config.recv_timeout.map(|t| Instant::now() + t);
        self.wait_parked(deadline)
    }

    /// Wakes the consumer parked on this segment.
    fn wake_ring(&self) {
        self.waiter
            .wake(self.layout.head_word().as_ptr().cast_const());
    }

    /// Event-driven wait (§4.4): park on the low 32 bits of `head` and let
    /// the producer's wake (in `push_frame`) deliver the frame at once. The
    /// `head` value is read **before** `pop_frame`, so a frame published
    /// between the read and the park makes the park return at once (the
    /// word differs, or the wake is already there): no lost wakeup.
    fn wait_parked(&self, deadline: Option<Instant>) -> Result<Vec<u8>, RecvError> {
        let head_word = self.layout.head_word().as_ptr().cast_const();
        loop {
            let expected = self.layout.head_word().load(Ordering::Acquire);
            let corrupt = self.corrupt_frames_seen();
            if let Some(frame) = self.pop_frame() {
                return Ok(frame);
            }
            if self.corrupt_frames_seen() != corrupt {
                return Err(CORRUPT_FRAME_DROPPED);
            }
            // Targeted owner-gone signal: the owner sets shutdown=1 (and
            // wakes us) on drop. Checked after pop_frame, so frames
            // published before the drop still arrive.
            if self.layout.shutdown().load(Ordering::Acquire) != 0 {
                return Err(RecvError::Io {
                    message: "shm owner terminated",
                });
            }
            // Announced before the look at the flag: `interrupt_recv` stores
            // the flag and then reads `parking`, so one of the two sees the
            // other (see there).
            self.parking.store(true, Ordering::SeqCst);
            if self.interrupted.swap(false, Ordering::SeqCst) {
                self.parking.store(false, Ordering::SeqCst);
                return Err(RecvError::Timeout);
            }
            let remaining = match deadline {
                Some(d) => {
                    let now = Instant::now();
                    if now >= d {
                        self.parking.store(false, Ordering::SeqCst);
                        return Err(RecvError::Timeout);
                    }
                    Some(d - now)
                }
                None => None,
            };
            #[cfg(test)]
            self.parks.fetch_add(1, Ordering::Relaxed);
            self.waiter.park(head_word, expected, remaining);
            self.parking.store(false, Ordering::SeqCst);
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
            data: data.into(),
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
    use crate::ring::PADDING_FRAME_LEN;
    use std::ptr;
    use std::sync::atomic::AtomicU64;

    /// A GUID for the tests, distinct in every process.
    ///
    /// `segment_os_id` is deterministic in (owner_id, consumer_id) and
    /// the segment it names lives in `/dev/shm`, which is the whole
    /// host's. Two test binaries — or two checkouts' runs — that both
    /// wrote `id(10)` therefore opened the *same* segment, and the
    /// second one read the first one's header: measured as
    /// `shm_4_2_consumer_cannot_send: Err(InvalidHeader)` and as a `recv()`
    /// timeout in `shm_2_3_open_concurrent_two_threads_both_bound`.
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
    fn shm_3_1_open_owner_then_consumer_roundtrip_single_frame() {
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
    fn shm_4_3_open_consumer_first_then_owner_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        // Unique ids 220/221: `segment_os_id` is deterministic in
        // (owner_id, consumer_id) and LIVES GLOBALLY in /dev/shm.
        // Earlier this test and `shm_4_2_owner_cannot_recv` shared ids 20/21 →
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
    fn shm_2_3_open_concurrent_two_threads_both_bound() {
        use std::sync::Arc as StdArc;
        // Unique ids 230/231 — see the id-collision note in
        // shm_4_3_open_consumer_first_then_owner_roundtrip above.
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
    fn shm_4_2_consumer_cannot_send() {
        let tmp = tempfile::tempdir().unwrap();
        let _owner =
            PosixShmTransport::open_owner(id(10), id(11), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(11), id(10), cfg_tmp(tmp.path(), 4096)).unwrap();

        let res = consumer.send(&Locator::shm(id(10)), b"x");
        assert!(matches!(res, Err(SendError::Io { .. })));
    }

    #[test]
    fn shm_4_2_owner_cannot_recv() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(20), id(21), cfg_tmp(tmp.path(), 4096)).unwrap();
        let res = owner.recv();
        assert!(matches!(res, Err(RecvError::Io { .. })));
    }

    #[test]
    fn shm_1_1_send_rejects_non_shm_locator() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(30), id(31), cfg_tmp(tmp.path(), 4096)).unwrap();
        let res = owner.send(&Locator::udp_v4([127, 0, 0, 1], 7400), b"x");
        assert_eq!(res, Err(SendError::UnsupportedLocator));
    }

    #[test]
    fn shm_4_1_send_rejects_wrong_peer() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(40), id(41), cfg_tmp(tmp.path(), 4096)).unwrap();
        // The owner talks only to peer 41, not 99.
        let res = owner.send(&Locator::shm(id(99)), b"x");
        assert_eq!(res, Err(SendError::UnsupportedLocator));
    }

    #[test]
    fn shm_3_3_send_rejects_oversize_payload() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(50), id(51), cfg_tmp(tmp.path(), 4096)).unwrap();
        let big = vec![0u8; 4096]; // > max_datagram (4096/2 - 16)
        let res = owner.send(&Locator::shm(id(51)), &big);
        assert!(matches!(res, Err(SendError::PayloadTooLarge { .. })));
    }

    #[test]
    fn shm_4_4_recv_times_out_when_idle() {
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
    fn shm_2_3_many_frames_roundtrip_with_wraparound() {
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
    fn shm_3_2_a_remainder_too_small_for_a_padding_header_is_skipped() {
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
    fn shm_3_2_frames_of_every_length_cross_the_ring_end_whole_and_in_order() {
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
    fn shm_4_3_consumer_open_succeeds_without_owner_creates_segment() {
        let tmp = tempfile::tempdir().unwrap();
        let res = PosixShmTransport::open_consumer(id(80), id(81), cfg_tmp(tmp.path(), 4096));
        assert!(
            res.is_ok(),
            "consumer-first must create the segment (idempotent open)"
        );
    }

    #[test]
    fn shm_1_1_local_locator_is_shm_with_own_id() {
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
    fn shm_2_1_invalid_config_capacity_too_small_for_max_datagram() {
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
    fn shm_3_1_occupied_bytes_reflects_send_state() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(110), id(111), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(owner.occupied_bytes(), 0);
        owner.send(&Locator::shm(id(111)), &[0u8; 64]).unwrap();
        // 4 byte length-prefix + 64 byte payload.
        assert_eq!(owner.occupied_bytes(), 68);
    }

    #[test]
    fn shm_3_1_recv_after_send_drains_occupied_bytes() {
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
    fn shm_4_4_owner_drop_signals_consumer_via_shutdown_flag() {
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
    fn shm_4_5_full_ring_returns_an_error_instead_of_dropping() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 512,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 128,
            recv_timeout: Some(Duration::from_millis(200)),
        };
        // 202/203 and not 220/221: `shm_4_3_open_consumer_first_then_owner_roundtrip`
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
    fn shm_3_3_corrupt_length_is_dropped_and_counted() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_tmp(tmp.path(), 4096);
        // No frame of an owner is longer than half the ring.
        let bound = cfg.capacity / 2;
        let owner = PosixShmTransport::open_owner(id(210), id(211), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(211), id(210), cfg).unwrap();

        // Write a bogus length header straight into the ring and publish
        // it, the way a corrupt owner would.
        assert!(owner.layout.write_u32(0, u32::try_from(bound + 1).unwrap()));
        owner.layout.store_head(4 + (bound as u64) + 1);

        assert!(consumer.pop_frame().is_none(), "no frame is handed up");
        assert_eq!(consumer.corrupt_frames_seen(), 1, "the drop is counted");
    }

    /// Positions an empty ring at `at`: `head` and `tail` both there, as a
    /// ring that has carried `at` bytes looks.
    fn place_empty_ring(owner: &PosixShmTransport, at: u64) {
        owner.layout.store_tail(at);
        owner.layout.store_head(at);
    }

    /// §3.3: a length below half the ring whose frame would end past the
    /// ring's end is dropped and counted; no byte past the data region is
    /// read. A writer of this protocol pads instead. Before, the read went
    /// past the mapping in a release build (the bound was a
    /// `debug_assert`).
    #[test]
    fn shm_3_3_a_length_past_the_ring_end_is_dropped_and_counted() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_tmp(tmp.path(), 4096);
        let owner = PosixShmTransport::open_owner(id(104), id(105), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(105), id(104), cfg).unwrap();
        // 100 bytes before the ring's end, a length of 200 (below the
        // bound of 2048), published whole.
        place_empty_ring(&owner, 4096 - 100);
        assert!(owner.layout.write_u32(4096 - 100, 200));
        owner.layout.store_head(4096 - 100 + 204);
        assert!(consumer.pop_frame().is_none(), "no frame is handed up");
        assert_eq!(consumer.corrupt_frames_seen(), 1);
        assert_eq!(consumer.occupied_bytes(), 0, "the reader resumes at head");
    }

    /// §3.3: a length whose frame ends past the published `head` is dropped
    /// and counted. Before, the reader handed up bytes the writer had not
    /// published and moved `tail` past `head`.
    #[test]
    fn shm_3_3_a_length_past_head_is_dropped_and_counted() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_tmp(tmp.path(), 4096);
        let owner = PosixShmTransport::open_owner(id(106), id(107), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(107), id(106), cfg).unwrap();
        place_empty_ring(&owner, 1000);
        assert!(owner.layout.write_u32(1000, 100));
        // 8 bytes published, the length announces 104.
        owner.layout.store_head(1008);
        assert!(consumer.pop_frame().is_none(), "no frame is handed up");
        assert_eq!(consumer.corrupt_frames_seen(), 1);
        assert_eq!(consumer.consumed_bytes(), 1008, "tail stops at head");
    }

    /// §3.3: a padding marker, a remainder skip or a `head` that claim more
    /// than the writer published, and a `head` more than the ring ahead of
    /// `tail`, are dropped and counted each; `tail` never passes `head`.
    #[test]
    fn shm_3_3_skips_and_heads_beyond_what_was_published_are_dropped() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_tmp(tmp.path(), 4096);
        let owner = PosixShmTransport::open_owner(id(108), id(109), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(109), id(108), cfg).unwrap();
        // A padding marker 100 bytes before the end with 8 bytes published.
        place_empty_ring(&owner, 4096 - 100);
        assert!(owner.layout.write_u32(4096 - 100, PADDING_FRAME_LEN));
        owner.layout.store_head(4096 - 92);
        assert!(consumer.pop_frame().is_none());
        assert_eq!(consumer.corrupt_frames_seen(), 1, "padding past head");
        assert_eq!(consumer.padding_frames_seen(), 0);
        // A remainder of 2 bytes with 1 byte published.
        place_empty_ring(&owner, 2 * 4096 - 2);
        owner.layout.store_head(2 * 4096 - 1);
        assert!(consumer.pop_frame().is_none());
        assert_eq!(consumer.corrupt_frames_seen(), 2, "remainder past head");
        // 2 bytes published where a frame needs 4 for its length.
        place_empty_ring(&owner, 3 * 4096);
        owner.layout.store_head(3 * 4096 + 2);
        assert!(consumer.pop_frame().is_none());
        assert_eq!(consumer.corrupt_frames_seen(), 3, "a length not published");
        // A head more than the ring ahead, and one behind the tail.
        place_empty_ring(&owner, 4 * 4096);
        owner.layout.store_head(6 * 4096);
        assert!(consumer.pop_frame().is_none());
        assert_eq!(consumer.corrupt_frames_seen(), 4, "head a ring ahead");
        place_empty_ring(&owner, 7 * 4096);
        owner.layout.store_head(7 * 4096 - 8);
        assert!(consumer.pop_frame().is_none());
        assert_eq!(consumer.corrupt_frames_seen(), 5, "head behind tail");
        assert_eq!(consumer.occupied_bytes(), 0, "tail at head each time");
    }

    /// §3.3: after a corrupt frame the reader resumes at `head`: the waiting
    /// `recv` returns the drop as an error, it is counted once, not again
    /// at every poll, and the writer's next frames arrive. Before, `tail` jumped to the ring's end past `head`, the
    /// occupancy wrapped, and the writer computed room from it.
    #[test]
    fn shm_3_3_the_ring_carries_frames_again_after_a_corrupt_one() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_tmp(tmp.path(), 4096);
        let owner = PosixShmTransport::open_owner(id(112), id(113), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(113), id(112), cfg).unwrap();
        let to = Locator::shm(id(113));
        owner.send(&to, b"before").unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"before");
        // A length of 3000, above half the ring, 10 bytes published.
        let at = owner.layout.head_position();
        assert!(owner.layout.write_u32(at as usize, 3000));
        owner.layout.store_head(at + 10);
        // A waiting recv returns the drop as an error, not as a frame and
        // not by waiting on.
        match consumer.recv() {
            Err(RecvError::Io { message }) => assert!(message.contains("corrupt"), "{message}"),
            other => panic!("expected the corrupt-frame error, got {other:?}"),
        }
        assert!(consumer.pop_frame().is_none());
        assert_eq!(consumer.corrupt_frames_seen(), 1, "counted once");
        assert_eq!(owner.occupied_bytes(), 0);
        for i in 0..200u8 {
            owner.send(&to, &[i; 100]).unwrap();
            assert_eq!(&consumer.recv().unwrap().data[..], &[i; 100][..]);
        }
        assert_eq!(consumer.corrupt_frames_seen(), 1);
    }

    #[test]
    fn shm_3_2_corrupt_frame_counter_default_zero() {
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
    fn rtps_9_3_2_4_locator_kind_is_the_vendor_reserved_shm_value() {
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
    fn shm_2_1_owner_writes_magic_and_version_into_the_header() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(160), id(161), cfg_tmp(tmp.path(), 4096)).unwrap();
        let base = owner.layout.base();
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
    fn shm_2_2_attach_rejects_foreign_magic() {
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
    fn shm_2_2_attach_rejects_unknown_version() {
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
    fn shm_5_2_open_recovers_from_stale_flink_of_crashed_owner() {
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
        #[cfg(windows)]
        {
            let name = segment_os_id(owner, consumer);
            let backing = backing_dir().join(name.trim_start_matches('/'));
            assert!(
                backing.exists(),
                "the ended process left {} behind",
                backing.display()
            );
        }
    }

    /// Runs `body` once for every wait backend this host has (§4.4), with
    /// that backend forced for the transports it opens on this thread.
    fn for_each_backend(body: impl Fn(crate::wait::Backend)) {
        let all = crate::wait::available();
        assert!(!all.is_empty());
        for backend in all {
            crate::wait::force(Some(backend));
            body(backend);
            crate::wait::force(None);
        }
    }

    /// Waits until `consumer` has parked at least `n` times.
    fn wait_for_parks(consumer: &PosixShmTransport, n: u64) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while consumer.parks.load(Ordering::Relaxed) < n {
            assert!(Instant::now() < deadline, "the consumer did not park");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// §4.4: an idle consumer parks once for its whole `recv_timeout`, on
    /// every backend of the host, instead of looking at the ring again and
    /// again. Before, every Unix but Linux and macOS from 14.4 slept 10 µs
    /// to 1 ms between looks: a few hundred looks in 300 ms.
    #[test]
    fn shm_4_4_the_consumer_parks_instead_of_polling() {
        for_each_backend(|backend| {
            let tmp = tempfile::tempdir().unwrap();
            let mut cfg = cfg_tmp(tmp.path(), 4096);
            cfg.recv_timeout = Some(Duration::from_millis(300));
            let _owner = PosixShmTransport::open_owner(id(138), id(139), cfg.clone()).unwrap();
            let consumer = PosixShmTransport::open_consumer(id(139), id(138), cfg).unwrap();
            assert_eq!(consumer.waiter.backend(), backend);
            let start = Instant::now();
            assert_eq!(consumer.recv(), Err(RecvError::Timeout));
            let waited = start.elapsed();
            let parks = consumer.parks.load(Ordering::Relaxed);
            assert!(
                waited >= Duration::from_millis(290),
                "{backend:?}: {waited:?}"
            );
            assert!(parks <= 3, "{backend:?}: {parks} parks in {waited:?}");
        });
    }

    /// §4.4: a push wakes a consumer that is parked on the ring at once, on
    /// every backend of the host. A wake that does not reach it leaves it
    /// parked until its 20 s timeout.
    #[test]
    fn shm_4_4_a_push_wakes_a_parked_consumer() {
        for_each_backend(|backend| {
            let tmp = tempfile::tempdir().unwrap();
            let mut cfg = cfg_tmp(tmp.path(), 4096);
            cfg.recv_timeout = Some(Duration::from_secs(20));
            let owner = PosixShmTransport::open_owner(id(154), id(155), cfg.clone()).unwrap();
            let consumer = std::sync::Arc::new(
                PosixShmTransport::open_consumer(id(155), id(154), cfg).unwrap(),
            );
            let waiting = std::sync::Arc::clone(&consumer);
            let t = std::thread::spawn(move || {
                let start = Instant::now();
                (waiting.recv().map(|d| d.data.to_vec()), start.elapsed())
            });
            wait_for_parks(&consumer, 1);
            std::thread::sleep(Duration::from_millis(50));
            owner.send(&Locator::shm(id(155)), b"wake").unwrap();
            let (got, waited) = t.join().unwrap();
            assert_eq!(got, Ok(b"wake".to_vec()), "{backend:?}");
            assert!(waited < Duration::from_secs(5), "{backend:?}: {waited:?}");
        });
    }

    /// §4.4: no wake is lost between the consumer's look at `head` and its
    /// park. 20 000 rounds per backend; the writer sends the next frame only
    /// once the consumer took the last, so every frame finds the consumer
    /// about to park, parking or parked. A lost wake leaves one `recv`
    /// waiting out its 3 s timeout.
    #[test]
    fn shm_4_4_no_wakeup_is_lost_between_the_look_and_the_park() {
        for_each_backend(|backend| {
            let tmp = tempfile::tempdir().unwrap();
            let mut cfg = cfg_tmp(tmp.path(), 4096);
            cfg.recv_timeout = Some(Duration::from_secs(3));
            let owner = PosixShmTransport::open_owner(id(142), id(143), cfg.clone()).unwrap();
            let consumer = PosixShmTransport::open_consumer(id(143), id(142), cfg).unwrap();
            const ROUNDS: u32 = 20_000;
            let producer = std::thread::spawn(move || {
                for round in 0..ROUNDS {
                    owner
                        .send(&Locator::shm(id(143)), &round.to_le_bytes())
                        .unwrap();
                    let deadline = Instant::now() + Duration::from_secs(10);
                    while owner.occupied_bytes() > 0 {
                        assert!(Instant::now() < deadline, "round {round} not taken");
                        std::thread::yield_now();
                    }
                }
                owner
            });
            let mut slowest = Duration::ZERO;
            for round in 0..ROUNDS {
                let start = Instant::now();
                let got = consumer.recv().unwrap_or_else(|e| {
                    panic!("{backend:?}, round {round}: {e:?}");
                });
                slowest = slowest.max(start.elapsed());
                assert_eq!(got.data[..], round.to_le_bytes(), "{backend:?}");
            }
            drop(producer.join().unwrap());
            assert!(
                slowest < Duration::from_millis(1500),
                "{backend:?}: a recv took {slowest:?}"
            );
        });
    }

    /// §4.4, §5.3: the owner's drop wakes a consumer that is parked on the
    /// ring, on every backend of the host; it returns the owner-gone error
    /// at once instead of after its 20 s timeout.
    #[test]
    fn shm_4_4_owner_drop_wakes_a_waiting_consumer() {
        for_each_backend(|backend| {
            let tmp = tempfile::tempdir().unwrap();
            let mut cfg = cfg_tmp(tmp.path(), 4096);
            cfg.recv_timeout = Some(Duration::from_secs(20));
            let owner = PosixShmTransport::open_owner(id(144), id(145), cfg.clone()).unwrap();
            let consumer = std::sync::Arc::new(
                PosixShmTransport::open_consumer(id(145), id(144), cfg).unwrap(),
            );
            let waiting = std::sync::Arc::clone(&consumer);
            let t = std::thread::spawn(move || {
                let start = Instant::now();
                (waiting.recv(), start.elapsed())
            });
            wait_for_parks(&consumer, 1);
            std::thread::sleep(Duration::from_millis(50));
            drop(owner);
            let (got, waited) = t.join().unwrap();
            match got {
                Err(RecvError::Io { message }) => {
                    assert!(message.contains("owner"), "{backend:?}: {message}")
                }
                other => panic!("{backend:?}: expected the owner-gone error, got {other:?}"),
            }
            assert!(waited < Duration::from_secs(5), "{backend:?}: {waited:?}");
        });
    }

    /// The owner sees whether a consumer has the segment open; a frame
    /// sent with a prefix arrives as one datagram.
    #[test]
    fn shm_2_1_the_owner_sees_the_consumer_attach_and_leave() {
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

    /// §2.1: `consumer_attached` is the `u32` at offset 36 and `owner_epoch`
    /// the `u32` at offset 40, in the host's byte order (both sides of a
    /// segment run on one host); the rest of the header up to 64 stays
    /// zero. Read from the raw mapping, not through the accessors.
    #[test]
    fn shm_2_1_attach_and_epoch_words_sit_at_offsets_36_and_40() {
        let tmp = tempfile::tempdir().unwrap();
        let first =
            PosixShmTransport::open_owner(id(114), id(115), cfg_tmp(tmp.path(), 4096)).unwrap();
        let raw = |at: usize| {
            // SAFETY: the owner holds the mapping; `at + 4 <= 64` lies in
            // the header.
            unsafe { ptr::read_unaligned(first.layout.base().add(at) as *const u32) }
        };
        assert_eq!((raw(36), raw(40)), (0, 1), "one owner, no consumer");
        let consumer =
            PosixShmTransport::open_consumer(id(115), id(114), cfg_tmp(tmp.path(), 4096)).unwrap();
        let again =
            PosixShmTransport::open_consumer(id(115), id(114), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(raw(36), 2, "two consumer handles");
        let second =
            PosixShmTransport::open_owner(id(114), id(115), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(raw(40), 2, "a second owner bumps the epoch");
        drop(again);
        assert_eq!(raw(36), 1);
        for at in (44..64).step_by(4) {
            assert_eq!(raw(at), 0, "offset {at} is reserved");
        }
        drop((consumer, second));
    }

    /// §5.2: a zombie under the pair's name with no flink beside it and no
    /// process holding it — a ring with a valid header and a published
    /// frame of the ended session — is unlinked before the create: the
    /// pair opens with an empty ring and carries only this session's frame.
    #[test]
    fn shm_5_2_a_zombie_segment_without_a_flink_is_unlinked_before_the_create() {
        let tmp = tempfile::tempdir().unwrap();
        let os_id = segment_os_id(id(116), id(117));
        shm_unlink_by_os_id(&os_id);
        let mut zombie = Mapping::create(&os_id, HEADER_BYTES + 4096, None).unwrap();
        let stale = b"frame of a crashed session";
        // SAFETY: the mapping is HEADER_BYTES + 4096 bytes long; the header
        // and one frame at the start of the data region are written as a
        // creator and a push write them.
        unsafe {
            let base = zombie.as_ptr();
            ptr::write_unaligned(base as *mut u32, SHM_MAGIC.to_be());
            ptr::write_unaligned(base.add(4) as *mut u32, SHM_VERSION.to_le());
            ptr::write_unaligned(base.add(8) as *mut u64, 4096u64.to_le());
            ptr::write_unaligned(
                base.add(HEADER_BYTES) as *mut u32,
                (stale.len() as u32).to_le(),
            );
            ptr::copy_nonoverlapping(stale.as_ptr(), base.add(HEADER_BYTES + 4), stale.len());
            (*(base.add(16) as *const AtomicU64)).store(4 + stale.len() as u64, Ordering::Release);
        }
        // What a crash leaves: the name stays, nobody maps or holds it.
        zombie.set_owner(false);
        drop(zombie);
        assert!(Mapping::open_os_id(&os_id).is_ok(), "the zombie is there");
        assert!(!segment_flink(tmp.path(), id(116), id(117)).exists());

        let discarded = ORPHANS_DISCARDED.with(core::cell::Cell::get);
        let owner =
            PosixShmTransport::open_owner(id(116), id(117), cfg_tmp(tmp.path(), 4096)).unwrap();
        // The unlink before the create took the zombie, not the path of
        // §5.5, which ends at the same fresh ring: without the unlink the
        // create finds the name, the open attaches, finds no holder and
        // discards the segment. On Windows the unlink removes the backing
        // file (before, it was a no-op there, and that path was the one).
        assert_eq!(
            ORPHANS_DISCARDED.with(core::cell::Cell::get),
            discarded,
            "the zombie went before the create, not as an orphan"
        );
        let consumer =
            PosixShmTransport::open_consumer(id(117), id(116), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(consumer.occupied_bytes(), 0, "a fresh ring");
        owner.send(&Locator::shm(id(117)), b"this session").unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"this session");
    }

    /// §5.3: frames the owner published before its drop still arrive, in
    /// order; the next `recv` returns the owner-gone error at once.
    #[test]
    fn shm_5_3_frames_published_before_the_drop_still_arrive() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = cfg_tmp(tmp.path(), 4096);
        cfg.recv_timeout = Some(Duration::from_secs(10));
        let owner = PosixShmTransport::open_owner(id(118), id(119), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(119), id(118), cfg).unwrap();
        for frame in [&b"one"[..], b"two", b"three"] {
            owner.send(&Locator::shm(id(119)), frame).unwrap();
        }
        drop(owner);
        for frame in [&b"one"[..], b"two", b"three"] {
            assert_eq!(&consumer.recv().unwrap().data[..], frame);
        }
        let start = Instant::now();
        match consumer.recv() {
            Err(RecvError::Io { message }) => assert!(message.contains("owner"), "{message}"),
            other => panic!("expected the owner-gone error, got {other:?}"),
        }
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    /// An owner dropped after a second owner opened the same segment
    /// leaves it to the second: no shutdown flag, the name stays, and the
    /// consumer keeps receiving from the second owner.
    #[test]
    fn shm_5_3_a_superseded_owner_leaves_the_segment_to_its_successor() {
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

    /// §5.3: the drop of an owner and the open of the pair's next owner do
    /// not interleave. Whichever comes first, the next owner writes into a
    /// ring its consumer reads: the old one, which it carries on, or a new
    /// one the consumer opens after the old ring's shutdown flag. Before,
    /// the drop decided outside the pair lock that it was the last owner,
    /// and a next owner that attached between that decision and the unlink
    /// wrote into a ring nobody read; so did one that made the ring anew
    /// before the old mapping's own drop unlinked the name once more.
    #[test]
    fn shm_5_3_a_drop_racing_the_next_owner_leaves_it_a_ring_that_is_read() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = cfg_tmp(tmp.path(), 4096);
        cfg.recv_timeout = Some(Duration::from_secs(1));
        for round in 0..500u32 {
            let first = PosixShmTransport::open_owner(id(146), id(147), cfg.clone()).unwrap();
            let mut consumer =
                PosixShmTransport::open_consumer(id(147), id(146), cfg.clone()).unwrap();
            let start = std::sync::Arc::new(std::sync::Barrier::new(2));
            let dropper = {
                let start = std::sync::Arc::clone(&start);
                std::thread::spawn(move || {
                    start.wait();
                    drop(first);
                })
            };
            start.wait();
            let next = PosixShmTransport::open_owner(id(146), id(147), cfg.clone()).unwrap();
            dropper.join().unwrap();
            // A consumer whose owner ended the ring opens the pair again,
            // as the receive thread of a same-host channel does.
            if consumer.try_recv().is_err() {
                consumer = PosixShmTransport::open_consumer(id(147), id(146), cfg.clone()).unwrap();
            }
            assert!(
                next.peer_attached(),
                "round {round}: the consumer reads the next owner's ring"
            );
            next.send(&Locator::shm(id(147)), &round.to_le_bytes())
                .unwrap();
            assert_eq!(
                &consumer.recv().expect("the frame of the next owner").data[..],
                &round.to_le_bytes()[..],
                "round {round}"
            );
        }
    }

    /// §5.3: an owner whose segment a later owner opened sends no more:
    /// the ring has one producer. Its send returns an error, and the frame
    /// it sent before is followed in the ring by the later owner's alone.
    #[test]
    fn shm_5_3_a_superseded_owner_refuses_to_send() {
        let tmp = tempfile::tempdir().unwrap();
        let first =
            PosixShmTransport::open_owner(id(148), id(149), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(149), id(148), cfg_tmp(tmp.path(), 4096)).unwrap();
        first.send(&Locator::shm(id(149)), b"before").unwrap();
        assert!(!first.is_superseded());
        let second =
            PosixShmTransport::open_owner(id(148), id(149), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert!(first.is_superseded() && !second.is_superseded());
        assert!(!consumer.is_superseded(), "a consumer is never superseded");
        match first.send(&Locator::shm(id(149)), b"after") {
            Err(SendError::Io { message }) => assert!(message.contains("superseded"), "{message}"),
            other => panic!("expected the superseded error, got {other:?}"),
        }
        second
            .send(&Locator::shm(id(149)), b"from the successor")
            .unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"before");
        assert_eq!(&consumer.recv().unwrap().data[..], b"from the successor");
        assert!(matches!(consumer.try_recv(), Ok(None)));
    }

    /// §5.3: an owner that keeps sending while a later owner of its segment
    /// in the same process opens and sends does not push at the same time:
    /// every frame a send reported as sent arrives whole and in order, and
    /// the earlier owner's sends end at the open. Before, each owner had a
    /// producer lock of its own, and the two raced on `head`.
    #[test]
    fn shm_5_3_two_owners_of_one_segment_never_push_at_once() {
        use std::sync::atomic::AtomicBool;
        use std::sync::{Arc, Mutex};
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = cfg_tmp(tmp.path(), 65536);
        cfg.recv_timeout = Some(Duration::from_millis(300));
        let frame = |sender: u8, seq: u32| {
            let mut f = vec![sender; 64];
            f[..4].copy_from_slice(&seq.to_le_bytes());
            f
        };
        let consumer = PosixShmTransport::open_consumer(id(157), id(156), cfg.clone()).unwrap();
        let first = PosixShmTransport::open_owner(id(156), id(157), cfg.clone()).unwrap();
        let senders_done = Arc::new(AtomicBool::new(false));
        let received = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let reader = {
            let (done, received) = (Arc::clone(&senders_done), Arc::clone(&received));
            std::thread::spawn(move || {
                loop {
                    match consumer.recv() {
                        Ok(dg) => received.lock().unwrap().push(dg.data.to_vec()),
                        Err(_) if done.load(Ordering::Acquire) => break,
                        Err(_) => {}
                    }
                }
                consumer.corrupt_frames_seen()
            })
        };
        let first_started = Arc::new(AtomicBool::new(false));
        let earlier = {
            let started = Arc::clone(&first_started);
            std::thread::spawn(move || {
                let mut sent = 0u32;
                loop {
                    match first.send(&Locator::shm(id(157)), &frame(1, sent)) {
                        Ok(()) => {
                            sent += 1;
                            started.store(true, Ordering::Release);
                        }
                        Err(SendError::Io { message }) if message.contains("superseded") => {
                            break;
                        }
                        Err(_) => std::thread::yield_now(),
                    }
                    if sent >= 200_000 {
                        break;
                    }
                }
                sent
            })
        };
        while !first_started.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        let second = PosixShmTransport::open_owner(id(156), id(157), cfg).unwrap();
        let mut sent_second = 0u32;
        while sent_second < 5000 {
            match second.send(&Locator::shm(id(157)), &frame(2, sent_second)) {
                Ok(()) => sent_second += 1,
                Err(_) => std::thread::yield_now(),
            }
        }
        let sent_first = earlier.join().unwrap();
        senders_done.store(true, Ordering::Release);
        let corrupt = reader.join().unwrap();
        let received = received.lock().unwrap();
        assert_eq!(corrupt, 0, "no frame broken by two pushes at once");
        let mut next = [0u32; 2];
        for f in received.iter() {
            assert_eq!(f.len(), 64);
            let sender = f[4];
            assert!(sender == 1 || sender == 2, "a whole frame: {f:?}");
            assert!(f[4..].iter().all(|b| *b == sender), "a whole frame: {f:?}");
            let seq = u32::from_le_bytes([f[0], f[1], f[2], f[3]]);
            let slot = usize::from(sender - 1);
            assert_eq!(seq, next[slot], "sender {sender} in order, none lost");
            next[slot] += 1;
        }
        assert_eq!(next, [sent_first, sent_second], "every frame sent arrived");
        assert!(
            sent_first < 200_000,
            "the earlier owner's sends ended at the open"
        );
    }

    /// §2.1: an attacher whose `max_datagram` does not fit twice in the
    /// creator's ring lowers it to `(capacity - 16) / 2`. As owner it
    /// refuses a frame above that with `PayloadTooLarge` and sends one of
    /// exactly that size; as consumer it reports the same limit.
    #[test]
    fn shm_2_1_an_attacher_lowers_its_max_datagram_to_the_creators_ring() {
        let tmp = tempfile::tempdir().unwrap();
        let small = cfg_tmp(tmp.path(), 4096);
        let large = cfg_tmp(tmp.path(), 65536);
        assert!(large.max_datagram > (4096 - 16) / 2);
        let limit = (4096 - 16) / 2;
        // The consumer with the small ring creates, the owner attaches.
        let consumer = PosixShmTransport::open_consumer(id(153), id(152), small.clone()).unwrap();
        let owner = PosixShmTransport::open_owner(id(152), id(153), large.clone()).unwrap();
        assert_eq!(owner.max_message_size(), limit);
        match owner.send(&Locator::shm(id(153)), &vec![1u8; limit + 1]) {
            Err(SendError::PayloadTooLarge { size, limit: l }) => {
                assert_eq!((size, l), (limit + 1, limit));
            }
            other => panic!("expected PayloadTooLarge, got {other:?}"),
        }
        owner
            .send(&Locator::shm(id(153)), &vec![2u8; limit])
            .unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], &vec![2u8; limit][..]);
        // The owner with the small ring creates, the consumer attaches.
        let owner = PosixShmTransport::open_owner(id(158), id(159), small).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(159), id(158), large).unwrap();
        assert_eq!(consumer.max_message_size(), limit);
        assert_eq!(
            owner.max_message_size(),
            cfg_tmp(tmp.path(), 4096).max_datagram
        );
    }

    /// A consumer that made the segment before its owner leaves the name
    /// to the owner: a consumer that opens the pair again finds the live
    /// ring, not a new one of its own.
    #[test]
    fn shm_4_3_a_consumer_that_made_the_segment_leaves_it_to_the_owner() {
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
    fn shm_2_1_sides_with_different_ring_sizes_share_the_creators_ring() {
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
    fn shm_4_4_interrupt_recv_returns_a_waiting_recv_at_once() {
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

    /// `interrupt_recv` ends a receive loop without a timeout wherever the
    /// loop is: before its look at the flag, between that look and the
    /// park, or parked. With one wake, a loop between the look and the park
    /// slept on for good.
    #[test]
    fn shm_4_4_interrupt_recv_reaches_a_recv_about_to_park() {
        use std::sync::atomic::AtomicBool;
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = cfg_tmp(tmp.path(), 4096);
        cfg.recv_timeout = None;
        let _owner = PosixShmTransport::open_owner(id(222), id(223), cfg.clone()).unwrap();
        let consumer =
            std::sync::Arc::new(PosixShmTransport::open_consumer(id(223), id(222), cfg).unwrap());
        for round in 0..2000u32 {
            let stop = std::sync::Arc::new(AtomicBool::new(false));
            let (done_tx, done_rx) = std::sync::mpsc::channel();
            let (waiting, stopped) = (
                std::sync::Arc::clone(&consumer),
                std::sync::Arc::clone(&stop),
            );
            let t = std::thread::spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    let _ = waiting.recv();
                }
                let _ = done_tx.send(());
            });
            // A different point of the loop each round.
            for _ in 0..(round % 64) * 40 {
                std::hint::spin_loop();
            }
            stop.store(true, Ordering::Relaxed);
            consumer.interrupt_recv();
            assert!(
                done_rx.recv_timeout(Duration::from_secs(5)).is_ok(),
                "round {round}: the receive loop did not end"
            );
            t.join().unwrap();
            // A flag the loop did not take is not left for the next round.
            consumer.interrupted.store(false, Ordering::SeqCst);
        }
    }

    /// §5.4: two users that look the segment up in different flink
    /// directories (another TMPDIR, a service's private /tmp) share it.
    /// Before, the second found no flink, unlinked the live segment and
    /// made its own: the owner wrote into one ring, the consumer read
    /// another. On every platform: before, the repair was Linux's alone.
    #[test]
    fn shm_5_4_users_in_different_flink_directories_share_the_segment() {
        let (here, there) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let owner =
            PosixShmTransport::open_owner(id(240), id(241), cfg_tmp(here.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(241), id(240), cfg_tmp(there.path(), 4096))
                .unwrap();
        owner.send(&Locator::shm(id(241)), b"one ring").unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"one ring");
    }

    /// §5.4: the lock that serializes the openers of a segment is
    /// `<name>.lock` in the pair directory, named after the segment alone;
    /// the flink directory holds no lock file. Where a segment is no file
    /// (all but Linux), its users hold `<name>.hold` beside it.
    #[test]
    fn shm_5_4_the_pair_lock_lies_in_the_pair_dir_not_in_the_flink_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(122), id(123), cfg_tmp(tmp.path(), 4096)).unwrap();
        let os_id = segment_os_id(id(122), id(123));
        let lock = pair_file(&os_id, "lock").unwrap();
        assert!(lock.exists(), "{}", lock.display());
        assert_eq!(lock.parent(), Some(pair_dir().unwrap().as_path()));
        assert!(
            !segment_flink(tmp.path(), id(122), id(123))
                .with_extension("lock")
                .exists()
        );
        #[cfg(target_os = "linux")]
        assert_eq!(pair_dir().unwrap(), Path::new(DEV_SHM));
        #[cfg(not(target_os = "linux"))]
        {
            assert!(segment_in_use(&os_id), "the owner holds <name>.hold");
            assert!(pair_file(&os_id, "hold").unwrap().exists());
        }
        #[cfg(all(unix, not(target_os = "linux")))]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = std::fs::symlink_metadata(pair_dir().unwrap()).unwrap();
            assert_eq!(meta.mode() & 0o077, 0, "the pair directory is private");
        }
        drop(owner);
        assert!(!lock.exists(), "the owner's drop removes the lock file");
    }

    /// §5.4: a pair file is never opened through a symlink. A symlink
    /// under the name of a pair's lock file makes the open fail, and the
    /// file it points to stays as it was.
    #[cfg(unix)]
    #[test]
    fn shm_5_4_a_pair_file_behind_a_symlink_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("elsewhere");
        std::fs::write(&target, b"not a lock").unwrap();
        let os_id = segment_os_id(id(162), id(163));
        let lock = pair_file(&os_id, "lock").unwrap();
        std::os::unix::fs::symlink(&target, &lock).unwrap();
        let opened = PosixShmTransport::open_owner(id(162), id(163), cfg_tmp(tmp.path(), 4096));
        let _ = std::fs::remove_file(&lock);
        assert!(opened.is_err(), "the open went through the symlink");
        assert_eq!(std::fs::read(&target).unwrap(), b"not a lock");
    }

    /// §5.4 outside Linux (`private_dir`, here on every Unix): a missing
    /// pair directory is made 0700. One of this user that group and others
    /// may only read and search (0755, as `create_dir_all` makes the root
    /// of the flink directories) is taken and made 0700. One they may
    /// write to is refused and keeps its mode, and so are a symlink to a
    /// private directory and a file.
    #[cfg(unix)]
    #[test]
    fn shm_5_4_a_pair_dir_others_may_only_read_is_made_private_one_they_may_write_is_refused() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let tmp = tempfile::tempdir().unwrap();
        let mode = |p: &Path| std::fs::symlink_metadata(p).unwrap().mode() & 0o7777;
        let made = tmp.path().join("made");
        assert_eq!(private_dir(&made).unwrap(), made);
        assert_eq!(mode(&made), 0o700);
        let readable = tmp.path().join("readable");
        std::fs::create_dir(&readable).unwrap();
        std::fs::set_permissions(&readable, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(private_dir(&readable).unwrap(), readable);
        assert_eq!(mode(&readable), 0o700, "made private");
        for writable in [0o720, 0o702, 0o775, 0o1777] {
            let dir = tmp.path().join(format!("w{writable:o}"));
            std::fs::create_dir(&dir).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(writable)).unwrap();
            let err = private_dir(&dir).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::PermissionDenied, "{writable:o}");
            assert!(err.to_string().contains("not a directory private to user"));
            assert_eq!(mode(&dir), writable, "left as it was");
        }
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&made, &link).unwrap();
        let file = tmp.path().join("file");
        std::fs::write(&file, b"no directory").unwrap();
        for path in [&link, &file] {
            let err = private_dir(path).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::PermissionDenied, "{path:?}");
        }
    }

    /// §5.4 on macOS and the other Unixes: a pair directory that a flink
    /// directory made first, with the 0755 the umask leaves, does not fail
    /// the open (every open failed so on the macOS runner, job 238161); the
    /// open makes it private.
    #[cfg(all(unix, not(target_os = "linux")))]
    #[test]
    fn shm_5_4_an_open_takes_a_pair_dir_made_0755_and_makes_it_private() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let dir = pair_dir().unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(182), id(183), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(183), id(182), cfg_tmp(tmp.path(), 4096)).unwrap();
        owner.send(&Locator::shm(id(183)), b"private").unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"private");
        let meta = std::fs::symlink_metadata(&dir).unwrap();
        assert_eq!(meta.mode() & 0o777, 0o700);
    }

    /// §5.4 on Windows: a pair directory that is a junction is refused, a
    /// plain one is taken.
    #[cfg(windows)]
    #[test]
    fn shm_5_4_a_pair_dir_that_is_a_junction_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let plain = tmp.path().join("plain");
        assert_eq!(plain_dir(&plain).unwrap(), plain);
        let junction = tmp.path().join("junction");
        let made = std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(&junction)
            .arg(&plain)
            .status()
            .unwrap();
        assert!(made.success(), "mklink /J");
        let refused = plain_dir(&junction).unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::PermissionDenied);
    }

    /// §5.4: an owner and a consumer that look the pair up in different
    /// flink directories and open it at the same moment end on one ring,
    /// round after round. Before, on Linux the second could find the
    /// segment between the first one's create and its hold, take it for
    /// an orphan and make its own; elsewhere it always did.
    #[test]
    fn shm_5_4_openers_in_different_flink_directories_race_to_one_segment() {
        let (here, there) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        for round in 0..200u32 {
            let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
            let (cfg_o, cfg_c) = (cfg_tmp(here.path(), 4096), cfg_tmp(there.path(), 4096));
            let g = std::sync::Arc::clone(&gate);
            let o = std::thread::spawn(move || {
                g.wait();
                PosixShmTransport::open_owner(id(124), id(125), cfg_o)
            });
            let g = std::sync::Arc::clone(&gate);
            let c = std::thread::spawn(move || {
                g.wait();
                PosixShmTransport::open_consumer(id(125), id(124), cfg_c)
            });
            let owner = o.join().unwrap().expect("owner");
            let consumer = c.join().unwrap().expect("consumer");
            let frame = round.to_le_bytes();
            owner.send(&Locator::shm(id(125)), &frame).unwrap();
            assert_eq!(
                consumer.recv().map(|d| d.data.to_vec()),
                Ok(frame.to_vec()),
                "round {round}: one ring"
            );
            drop(owner);
            drop(consumer);
        }
    }

    /// The environment variable that turns [`pair_opener`] from a no-op
    /// into the other process of
    /// `shm_5_4_two_processes_open_the_pair_at_once`.
    const OPENER_SPEC: &str = "ZERODDS_SHM_TEST_OPENER";

    /// Not a test of its own: the body of the process that
    /// `shm_5_4_two_processes_open_the_pair_at_once` starts. At the moment
    /// it is given (milliseconds since the epoch) it opens the owner side in
    /// its own flink directory, waits for the consumer, sends one frame and
    /// waits until it is taken.
    #[test]
    fn pair_opener() {
        let Ok(spec) = std::env::var(OPENER_SPEC) else {
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
        let at: u128 = parts[3].parse().unwrap();
        wait_until_epoch_ms(at);
        let t = PosixShmTransport::open_owner(owner, consumer, cfg_tmp(Path::new(parts[2]), 4096))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !t.peer_attached() {
            assert!(Instant::now() < deadline, "no consumer attached");
            std::thread::sleep(Duration::from_millis(1));
        }
        t.send(&Locator::shm(consumer), b"from the other process")
            .unwrap();
        while t.occupied_bytes() > 0 {
            assert!(Instant::now() < deadline, "the frame was not taken");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn epoch_ms() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }

    fn wait_until_epoch_ms(at: u128) {
        let now = epoch_ms();
        if at > now {
            std::thread::sleep(Duration::from_millis(u64::try_from(at - now).unwrap()));
        }
    }

    /// §5.4 across processes: a child process opens the owner side in its
    /// flink directory at the same moment as this one opens the consumer
    /// side in another; the frame the child sends arrives, round after
    /// round.
    #[test]
    fn shm_5_4_two_processes_open_the_pair_at_once() {
        let (here, there) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let exe = std::env::current_exe().unwrap();
        for round in 0..10 {
            let at = epoch_ms() + 300;
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
            let child = command
                .args(["--exact", "posix::tests::pair_opener", "--test-threads=1"])
                .env(
                    OPENER_SPEC,
                    format!(
                        "{};{};{};{at}",
                        hex16(id(126)),
                        hex16(id(127)),
                        there.path().display()
                    ),
                )
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap();
            wait_until_epoch_ms(at);
            let mut cfg = cfg_tmp(here.path(), 4096);
            cfg.recv_timeout = Some(Duration::from_secs(10));
            let consumer = PosixShmTransport::open_consumer(id(127), id(126), cfg).unwrap();
            assert_eq!(
                consumer.recv().map(|d| d.data.to_vec()),
                Ok(b"from the other process".to_vec()),
                "round {round}"
            );
            let status = child.wait_with_output().unwrap().status;
            assert!(
                status.success(),
                "round {round}: the other process: {status}"
            );
        }
    }

    /// §5.5: a consumer that finds the segment of an ended session under
    /// its pair's name makes it anew. Before, it attached and delivered the
    /// frame the ended owner had left in the ring. On Windows (§6.3) the
    /// mapping stays on its backing file after the ended process; nobody
    /// holds its hold file, and the opener discards it as everywhere.
    #[test]
    fn shm_5_5_an_orphaned_segment_is_made_anew_not_attached() {
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

    /// §6.4: the ring of a named segment and the ring over a caller's
    /// region are one layout: a region consumer over the bytes of a POSIX
    /// owner's mapping receives its frames, and a POSIX consumer receives
    /// what a region owner writes into its mapping; across the ring's end.
    #[test]
    fn shm_6_4_region_and_posix_transport_share_one_layout() {
        use crate::region::{RegionConfig, RegionShmTransport, SpinWait};
        let frame = |i: u32| {
            let mut f = i.to_le_bytes().to_vec();
            f.resize(4 + (i as usize * 29) % 500, i as u8);
            f
        };
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_tmp(tmp.path(), 4096);
        let wait = SpinWait {
            spins_per_ms: 100_000,
        };
        let region_cfg = RegionConfig {
            max_datagram: 1024,
            recv_timeout: Some(Duration::from_secs(2)),
        };
        let owner = PosixShmTransport::open_owner(id(174), id(175), cfg.clone()).unwrap();
        let len = HEADER_BYTES + owner.layout.capacity();
        // SAFETY: the owner's mapping is `len` bytes long and outlives the
        // consumer; nobody else reads the ring.
        let consumer = unsafe {
            RegionShmTransport::open_consumer(
                owner.layout.base(),
                len,
                id(175),
                id(174),
                region_cfg,
                wait,
            )
        }
        .unwrap();
        assert!(
            owner.peer_attached(),
            "the region consumer counts as attached"
        );
        for i in 0..300u32 {
            owner.send(&Locator::shm(id(175)), &frame(i)).unwrap();
            assert_eq!(
                &consumer.recv().unwrap().data[..],
                &frame(i)[..],
                "frame {i}"
            );
        }
        assert!(consumer.padding_frames_seen() > 0);
        drop(consumer);
        drop(owner);

        let consumer = PosixShmTransport::open_consumer(id(177), id(176), cfg).unwrap();
        let len = HEADER_BYTES + consumer.layout.capacity();
        // SAFETY: as above, the consumer's mapping.
        let owner = unsafe {
            RegionShmTransport::open_owner(
                consumer.layout.base(),
                len,
                id(176),
                id(177),
                region_cfg,
                wait,
            )
        }
        .unwrap();
        for i in 0..300u32 {
            owner.send(&Locator::shm(id(177)), &frame(i)).unwrap();
            assert_eq!(
                &consumer.recv().unwrap().data[..],
                &frame(i)[..],
                "frame {i}"
            );
        }
        assert!(consumer.padding_frames_seen() > 0);
        drop(owner);
        assert!(matches!(
            consumer.recv(),
            Err(RecvError::Io {
                message: "shm owner terminated"
            })
        ));
    }

    /// §6.3: what an owner that ended without its drop leaves on Windows —
    /// the ring's backing file with the frame of its session, its flink,
    /// its hold and its lock file — goes with the sweeps a participant runs
    /// at its start when nobody opens the pair again: the backing file, the
    /// hold and the lock with the sweep of the pair directory, the flink
    /// with the flink sweep. A ring whose creator ended before it took its
    /// hold (no hold file, no flink) goes with the sweep of the backing
    /// files. A held pair stays and carries a frame after. Before, nothing
    /// swept on Windows, and each such ring stayed on disk for good.
    #[cfg(windows)]
    #[test]
    fn shm_6_3_the_sweep_removes_what_an_ended_owner_left_on_windows() {
        let tmp = tempfile::tempdir().unwrap();
        leave_orphan(id(164), id(165), tmp.path());
        let orphan = segment_os_id(id(164), id(165));
        let backing = backing_dir().join(&orphan[1..]);
        let hold = pair_file(&orphan, "hold").unwrap();
        let lock = pair_file(&orphan, "lock").unwrap();
        let flink = segment_flink(tmp.path(), id(164), id(165));
        assert!(hold.exists() && lock.exists() && flink.exists());
        let report = sweep_pair_files(&orphan[1..], Duration::ZERO).unwrap();
        assert_eq!(report.removed, 1, "{report:?}");
        assert!(!backing.exists(), "the ring with the ended session's frame");
        assert!(!hold.exists() && !lock.exists());
        let report = sweep_orphaned_flinks(tmp.path(), Duration::ZERO).unwrap();
        assert_eq!(report.removed, 1, "{report:?}");
        assert!(!flink.exists());

        // A ring made and left before its creator took the hold.
        let early = segment_os_id(id(166), id(167));
        let mut ring = Mapping::create(&early, HEADER_BYTES + 4096, None).unwrap();
        // SAFETY: the mapping is HEADER_BYTES + 4096 bytes long.
        unsafe {
            ptr::write_unaligned(ring.as_ptr() as *mut u32, SHM_MAGIC.to_be());
            ptr::write_unaligned(ring.as_ptr().add(4) as *mut u32, SHM_VERSION.to_le());
        }
        ring.set_owner(false);
        drop(ring);
        let early_file = backing_dir().join(&early[1..]);
        assert!(early_file.exists() && !pair_file(&early, "hold").unwrap().exists());
        let report = sweep_backing_files_under(&early[1..], Duration::from_secs(3600)).unwrap();
        assert_eq!((report.young, report.removed), (1, 0), "{report:?}");
        let report = sweep_backing_files_under(&early[1..], Duration::ZERO).unwrap();
        assert_eq!(report.removed, 1, "{report:?}");
        assert!(!early_file.exists());

        let owner =
            PosixShmTransport::open_owner(id(168), id(169), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(169), id(168), cfg_tmp(tmp.path(), 4096)).unwrap();
        let live = segment_os_id(id(168), id(169));
        let pairs = sweep_pair_files(&live[1..], Duration::ZERO).unwrap();
        let rings = sweep_backing_files_under(&live[1..], Duration::ZERO).unwrap();
        let flinks = sweep_orphaned_flinks(tmp.path(), Duration::ZERO).unwrap();
        assert_eq!(
            (pairs.removed, rings.removed, rings.held, flinks.removed),
            (0, 0, 1, 0),
            "{pairs:?} {rings:?} {flinks:?}"
        );
        assert!(backing_dir().join(&live[1..]).exists());
        owner.send(&Locator::shm(id(169)), b"still bound").unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"still bound");
    }

    /// A `.lock` file without a flink beside it goes once nobody holds it;
    /// a young one and one whose pair lock is held stay, and so does the
    /// lock file of an open pair.
    #[test]
    fn shm_5_5_the_pair_file_sweep_removes_lone_lock_files_and_keeps_the_rest() {
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
            pair_file(&segment_os_id(id(24), id(25)), "lock")
                .unwrap()
                .exists()
        );
        owner
            .send(&Locator::shm(id(25)), b"after the sweep")
            .unwrap();
        assert_eq!(&consumer.recv().unwrap().data[..], b"after the sweep");
    }

    /// §5.5 on Linux: the sweep of a flink directory builds the path of a
    /// pair's lock file from the name a flink holds. A flink whose name
    /// climbs out of `/dev/shm` (`/zd-<dir>/../../../<path>/secret`) counts
    /// as foreign and stays, and so does the file it points to. Before, the
    /// sweep removed `<path>/secret.lock` in any directory the user may
    /// write to.
    #[cfg(target_os = "linux")]
    #[test]
    fn shm_5_5_the_flink_sweep_leaves_a_flink_whose_name_points_outside_dev_shm() {
        let flinks = tempfile::tempdir().unwrap();
        let victim = tempfile::tempdir().unwrap();
        let kept = victim.path().join("secret.lock");
        std::fs::write(&kept, b"kept").unwrap();
        // `..` resolves only behind a directory that exists.
        let step = format!("zd-sweep-probe-{}", std::process::id());
        let step_dir = Path::new(DEV_SHM).join(&step);
        std::fs::create_dir(&step_dir).unwrap();
        let name = format!("/{step}/../../..{}/secret", victim.path().display());
        let flink = flinks.path().join("x.shm");
        std::fs::write(&flink, &name).unwrap();
        let report = sweep_stale_pair_files(flinks.path(), Duration::ZERO);
        let _ = std::fs::remove_dir(&step_dir);
        let report = report.unwrap();
        assert!(kept.exists(), "{name}: the sweep removed it ({report:?})");
        assert_eq!((report.removed, report.foreign), (0, 1), "{report:?}");
        assert!(flink.exists(), "a foreign flink stays");
    }

    /// §5.5 on every platform but Linux, the counterpart of the test
    /// above: [`sweep_orphaned_flinks`] builds the paths of a pair's lock
    /// and hold files in [`pair_dir`] from the name a flink holds. A flink
    /// whose name climbs out of the pair directory
    /// (`/zd-<dir>/../../<victim>/secret`) counts as foreign and stays; the
    /// sweep neither makes nor removes a file where the name leads. Without
    /// the shape check it took the pair lock on `<victim>/secret.lock`,
    /// which it made for that, before the mapping refused the name.
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn shm_5_5_the_flink_sweep_leaves_a_flink_whose_name_points_outside_the_pair_dir() {
        let dir = pair_dir().unwrap();
        let victim = tempfile::tempdir_in(dir.parent().unwrap()).unwrap();
        let hold = victim.path().join("secret.hold");
        let lock = victim.path().join("secret.lock");
        std::fs::write(&hold, b"kept").unwrap();
        // `..` resolves only behind a directory that exists (Unix).
        let step = format!("zd-sweep-probe-{}", std::process::id());
        let step_dir = dir.join(&step);
        std::fs::create_dir(&step_dir).unwrap();
        let victim_name = victim.path().file_name().unwrap().to_str().unwrap();
        let name = format!("/{step}/../../{victim_name}/secret");
        let flinks = tempfile::tempdir().unwrap();
        let flink = flinks.path().join("x.shm");
        std::fs::write(&flink, &name).unwrap();
        // The name does lead there: the test checks the sweep, not a path
        // that misses.
        let led_to = pair_file(&name, "hold").unwrap().canonicalize();
        let report = sweep_orphaned_flinks(flinks.path(), Duration::ZERO);
        let _ = std::fs::remove_dir(&step_dir);
        assert_eq!(led_to.unwrap(), hold.canonicalize().unwrap());
        let report = report.unwrap();
        assert!(hold.exists(), "{name}: the sweep removed it ({report:?})");
        assert!(
            !lock.exists(),
            "{name}: the sweep made {} ({report:?})",
            lock.display()
        );
        assert_eq!((report.removed, report.foreign), (0, 1), "{report:?}");
        assert!(flink.exists(), "a foreign flink stays");
    }

    /// The sweeps take a name from a flink only in the shape
    /// `segment_os_id` gives it: `/zd-` and the two ids in lowercase hex,
    /// or the 26 hex digits of the short form; nothing that would lead a
    /// path out of its directory.
    #[test]
    fn shm_5_5_a_name_from_a_flink_has_the_shape_segment_os_id_gives() {
        // Fixed ids, no segment: the names are only looked at.
        let (owner, consumer) = ([0xab; 16], [0xcd; 16]);
        for short in [false, true] {
            let name = segment_os_id_in(owner, consumer, short);
            assert!(is_segment_os_id(&name), "{name}");
        }
        let long = segment_os_id_in(owner, consumer, false);
        for name in [
            String::new(),
            "/zd-".to_owned(),
            long[1..].to_owned(),
            format!("/zd-{}", long[4..].to_uppercase()),
            format!("{long}0"),
            format!("{long}\n"),
            long.replacen("-", "/", 2),
            format!("/zd-{}", "0".repeat(25)),
            format!("/zd-{}.", "0".repeat(25)),
            format!("/zd-{}-{}", "0".repeat(32), "0".repeat(31)),
            format!("/zd-x/../../../tmp/{}", "0".repeat(26)),
            format!("/zd-{}/../{}", "0".repeat(26), "0".repeat(26)),
        ] {
            assert!(!is_segment_os_id(&name), "{name:?}");
        }
    }

    /// An opener that waited on a lock file someone removed meanwhile does
    /// not keep that lock: it locks the file now under the name. On Windows
    /// too, where before the name was not compared with the locked file. The
    /// file goes as an owner's drop and the sweeps remove it
    /// ([`remove_name`]).
    #[test]
    fn shm_5_5_a_lock_taken_on_a_removed_lock_file_is_taken_anew() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("pair.lock");
        let first = lock_pair(&path).unwrap();
        let waiter = {
            let path = path.clone();
            std::thread::spawn(move || lock_pair(&path).map(|g| g.file))
        };
        std::thread::sleep(Duration::from_millis(50));
        remove_name(&path).unwrap();
        drop(first);
        let file = waiter.join().unwrap().unwrap();
        assert!(path.exists(), "the waiter made the lock file anew");
        assert!(
            still_named(&file, &path).unwrap(),
            "and holds the file under the name"
        );
        // The check can fail: a file the name no longer names is told apart,
        // one that was removed and one that was moved away.
        let gone = tmp.path().join("gone.lock");
        let old = open_pair_file(&gone).unwrap();
        remove_name(&gone).unwrap();
        assert!(!still_named(&old, &gone).unwrap(), "a removed file");
        let named = tmp.path().join("named.lock");
        let moved = tmp.path().join("moved.lock");
        let held = open_pair_file(&named).unwrap();
        std::fs::rename(&named, &moved).unwrap();
        let _other = open_pair_file(&named).unwrap();
        assert!(
            !still_named(&held, &named).unwrap(),
            "another file, same name"
        );
        assert!(still_named(&held, &moved).unwrap(), "its new name");
    }

    /// A pair file or segment file that is removed frees its name at once,
    /// while another handle still holds it: a new file can be made under
    /// the name, and the old handle keeps its own bytes and its lock. Where
    /// Windows deletes the old way, and under Wine, a plain removal kept the
    /// name until the last handle closed.
    #[test]
    fn shm_5_4_a_removed_pair_file_frees_its_name_at_once() {
        use std::io::{Read, Seek, Write};
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("pair.hold");
        let mut old = open_pair_file(&path).unwrap();
        old.write_all(b"old").unwrap();
        file_lock::shared(&old).unwrap();
        remove_name(&path).unwrap();
        assert!(!path.exists(), "the name is gone");
        let mut new = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        new.write_all(b"new").unwrap();
        assert!(file_lock::try_exclusive(&new).unwrap(), "a file of its own");
        let mut bytes = String::new();
        old.rewind().unwrap();
        old.read_to_string(&mut bytes).unwrap();
        assert_eq!(bytes, "old");
        assert!(
            remove_name(&tmp.path().join("missing"))
                .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
        );
        drop(old);
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .filter(|n| n != "pair.hold")
            .collect();
        assert!(
            leftovers.is_empty(),
            "the removed file goes with its handle: {leftovers:?}"
        );
    }

    /// §5.5: the sweep unlinks a segment no process holds, leaves a
    /// young one alone, and keeps one that is held.
    #[cfg(target_os = "linux")]
    #[test]
    fn shm_5_5_the_sweep_removes_orphaned_segments_and_keeps_held_ones() {
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
    fn shm_5_5_a_ring_of_an_earlier_layout_goes_unless_a_process_maps_it() {
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

    /// A FIFO at `path`, as the FIFO backend makes a segment's `.wake`
    /// (§4.4): `std::fs::write` would make a regular file.
    #[cfg(unix)]
    fn make_fifo(path: &Path) {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::FileTypeExt;
        let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: a NUL-terminated path; the result is checked.
        let made = unsafe { libc::mkfifo(name.as_ptr(), 0o600) };
        assert_eq!(
            made,
            0,
            "{}: {}",
            path.display(),
            io::Error::last_os_error()
        );
        let meta = std::fs::symlink_metadata(path).unwrap();
        assert!(meta.file_type().is_fifo() && !meta.is_file());
    }

    /// §5.5: the sweep of the pair directory removes the files of pairs
    /// whose segments are gone, the wait FIFOs of §4.4 among them, and,
    /// where a segment is no file, the orphaned segment its hold file
    /// names; it keeps what a process holds and what is younger than the
    /// minimum age.
    #[test]
    fn shm_5_5_the_pair_dir_sweep_removes_what_gone_segments_left() {
        #[cfg(target_os = "linux")]
        {
            let prefix = format!("{SEGMENT_PREFIX}pairsweep-{}-", std::process::id());
            let dir = pair_dir().unwrap();
            let lone = dir.join(format!("{prefix}a.lock"));
            let live = dir.join(format!("{prefix}b.lock"));
            let segment = dir.join(format!("{prefix}b"));
            let held = dir.join(format!("{prefix}c.lock"));
            let fifo = dir.join(format!("{prefix}d.wake"));
            let live_fifo = dir.join(format!("{prefix}b.wake"));
            for file in [&lone, &live, &segment, &held] {
                std::fs::write(file, b"").unwrap();
            }
            make_fifo(&fifo);
            make_fifo(&live_fifo);
            let guard = lock_pair(&held).unwrap();
            let report = sweep_pair_files(&prefix, Duration::from_secs(3600)).unwrap();
            assert_eq!((report.young, report.removed), (3, 0), "{report:?}");
            let report = sweep_pair_files(&prefix, Duration::ZERO).unwrap();
            assert_eq!((report.removed, report.held), (2, 1), "{report:?}");
            assert!(!lone.exists(), "the lock file of a gone segment goes");
            assert!(
                std::fs::symlink_metadata(&fifo).is_err(),
                "and its wait FIFO"
            );
            assert!(live.exists() && held.exists());
            assert!(
                std::fs::symlink_metadata(&live_fifo).is_ok(),
                "the wait FIFO of a segment that is there stays"
            );
            drop(guard);
            for file in [&live, &segment, &held, &live_fifo] {
                std::fs::remove_file(file).unwrap();
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let tmp = tempfile::tempdir().unwrap();
            leave_orphan(id(128), id(129), tmp.path());
            let orphan = segment_os_id(id(128), id(129));
            let hold = pair_file(&orphan, "hold").unwrap();
            assert!(hold.exists(), "the ended process left its hold file");
            let report = sweep_pair_files(&orphan[1..], Duration::from_secs(3600)).unwrap();
            assert_eq!((report.young, report.removed), (1, 0), "{report:?}");
            let report = sweep_pair_files(&orphan[1..], Duration::ZERO).unwrap();
            assert_eq!(report.removed, 1, "{report:?}");
            assert!(!hold.exists());
            assert!(
                Mapping::open_os_id(&orphan).is_err(),
                "the segment it named is unlinked"
            );
            let owner =
                PosixShmTransport::open_owner(id(134), id(135), cfg_tmp(tmp.path(), 4096)).unwrap();
            let live = segment_os_id(id(134), id(135));
            // The wait FIFO of §4.4 beside the live segment, as an owner
            // on the FIFO backend makes it; the owner here waits on its
            // own backend and does not remove it.
            #[cfg(unix)]
            let fifo = pair_file(&live, "wake").unwrap();
            #[cfg(unix)]
            make_fifo(&fifo);
            let report = sweep_pair_files(&live[1..], Duration::ZERO).unwrap();
            assert_eq!((report.removed, report.held), (0, 1), "{report:?}");
            #[cfg(unix)]
            assert!(
                std::fs::symlink_metadata(&fifo).is_ok(),
                "the wait FIFO of a segment that is there stays"
            );
            drop(owner);
            // The owner's drop unlinked the segment and left the FIFO, as a
            // process on the FIFO backend that ended without its drop does.
            #[cfg(unix)]
            {
                let report = sweep_pair_files(&live[1..], Duration::ZERO).unwrap();
                assert_eq!(report.removed, 1, "{report:?}");
                assert!(
                    std::fs::symlink_metadata(&fifo).is_err(),
                    "the wait FIFO of a gone segment goes"
                );
            }
        }
    }

    /// §5.5 on every platform but Linux: the flink sweep unlinks a segment
    /// no process holds, leaves a young one alone, keeps one that is held
    /// and one of another layout version.
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn shm_5_5_the_flink_sweep_removes_orphaned_segments_and_keeps_held_ones() {
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

        // A ring of layout version 1 nobody holds: the sweep cannot tell
        // whether a user of that version still maps it and leaves it.
        let foreign = tempfile::tempdir().unwrap();
        let os_id = segment_os_id(id(136), id(137));
        let flink = segment_flink(foreign.path(), id(136), id(137));
        let mut v1 = Mapping::create(&os_id, HEADER_BYTES + 4096, Some(&flink)).unwrap();
        // SAFETY: the mapping is at least HEADER_BYTES long.
        unsafe {
            ptr::write_unaligned(v1.as_ptr() as *mut u32, SHM_MAGIC.to_be());
            ptr::write_unaligned(v1.as_ptr().add(4) as *mut u32, 1u32.to_le());
        }
        v1.set_owner(false);
        drop(v1);
        let report = sweep_orphaned_flinks(foreign.path(), Duration::ZERO).unwrap();
        assert_eq!((report.removed, report.foreign), (0, 1), "{report:?}");
        assert!(flink.exists());
        assert!(Mapping::open_os_id(&os_id).is_ok(), "the ring stays");
        shm_unlink_by_os_id(&os_id);
        let _ = std::fs::remove_file(pair_file(&os_id, "lock").unwrap());
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
    /// between `shm_4_5_full_ring_returns_an_error_instead_of_dropping` and
    /// `shm_4_3_open_consumer_first_then_owner_roundtrip`, which only Windows ever
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
            if let Some(rest) = t.strip_prefix("fn ")
                && let Some(name) = rest.split('(').next()
            {
                current = name.to_string();
                consts.clear();
            }
            if let Some(rest) = t.strip_prefix("const ")
                && let Some((name, value)) = rest.split_once(": u8 = ")
                && let Ok(v) = value.trim_end_matches(';').trim().parse::<u32>()
            {
                consts.insert(name.trim().to_string(), v);
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
                if let Some(v) = value
                    && !current.is_empty()
                {
                    per_test.entry(current.clone()).or_default().insert(v);
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
    fn shm_5_1_short_names_of_pairs_that_differ_only_in_the_prefix_differ() {
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
    fn shm_5_1_segment_os_id_is_deterministic_and_within_platform_limit() {
        let a = segment_os_id(id(190), id(191));
        assert_eq!(a, segment_os_id(id(190), id(191)), "same pair, same name");
        assert_ne!(a, segment_os_id(id(191), id(190)), "direction matters");
        assert!(a.starts_with('/'), "posix shm name starts with a slash");
        #[cfg(target_os = "macos")]
        assert!(a.len() <= 31, "PSHMNAMLEN is 31, got {} in {a}", a.len());
    }

    #[test]
    fn shm_3_2_fill_ring_triggers_wraparound_padding() {
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
        // Frames 0 to 3 fill 0..256; the fifth does not fit the 44 bytes
        // left and goes to 0, behind a padding marker at 256.
        for i in 0..5u8 {
            let p = vec![i; 60];
            owner.send(&Locator::shm(id(131)), &p).unwrap();
            if i == 4 {
                assert_eq!(
                    owner.layout.read_u32(256),
                    Some(0xFFFF_FFFE),
                    "the writer marks the rest of the ring as padding"
                );
                assert_eq!(consumer.padding_frames_seen(), 0);
            }
            let got = consumer.recv().unwrap();
            assert_eq!(&got.data[..], p.as_slice());
        }
        assert_eq!(consumer.padding_frames_seen(), 1, "the reader counts it");
    }

    /// §3.2: the reader counts each padding frame once. 64-byte frames in a
    /// 300-byte ring wrap after every fourth frame with 44 bytes of
    /// padding: 201 frames cross the ring's end 50 times.
    #[test]
    fn shm_3_2_a_padding_frame_is_counted_once_per_wrap() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 300,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 100,
            recv_timeout: Some(Duration::from_secs(1)),
        };
        let owner = PosixShmTransport::open_owner(id(132), id(133), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(133), id(132), cfg).unwrap();
        for i in 0..201u32 {
            let p = vec![(i % 251) as u8; 60];
            owner.send(&Locator::shm(id(133)), &p).unwrap();
            assert_eq!(&consumer.recv().unwrap().data[..], p.as_slice());
        }
        assert_eq!(consumer.padding_frames_seen(), 50);
        assert_eq!(consumer.corrupt_frames_seen(), 0);
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
