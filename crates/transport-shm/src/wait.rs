// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! How a consumer waits for a frame and a participant for its doorbell
//! (`zerodds-shm-transport-1.0` §4.4), and how the writer wakes it.
//!
//! The waiter reads the word it waits on (the low 32 bits of a ring's
//! `head`, a doorbell's counter) before it looks for work, and parks only
//! while that word still has the value it read, or no wake came since: a
//! wake between its look and its park is not lost. One backend per
//! platform, the same on both sides of a segment, which run on one host:
//!
//! - Linux: a shared futex on the word.
//! - macOS from 14.4: `os_sync_wait_on_address` with
//!   `OS_SYNC_WAIT_ON_ADDRESS_SHARED`; macOS 10.15 to 14.3: `__ulock_wait`
//!   with `UL_COMPARE_AND_WAIT_SHARED`. Both are looked up at run time: a
//!   binary that names them does not start on an older macOS.
//! - Every other Unix, and a macOS before 10.15: a FIFO per segment,
//!   `<name>.wake` in the pair directory (`crate::posix::pair_dir`). The
//!   writer writes a byte, the waiter polls the FIFO and drains it. A byte
//!   written between the waiter's look and its poll stays in the pipe.
//! - Windows: a named auto-reset event per segment. A set before the wait
//!   stays set.
//!
//! Before, macOS before 14.4 and the other Unixes slept 10 µs to 1 ms
//! between looks at the ring, and a doorbell 1 ms.

use std::io;
use std::time::Duration;

/// A way to wait. Every platform names all five; it makes only its own.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Backend {
    /// A shared futex (Linux).
    Futex,
    /// `os_sync_wait_on_address` (macOS from 14.4).
    OsSync,
    /// `__ulock_wait` (macOS from 10.15).
    Ulock,
    /// A FIFO per segment (every Unix).
    Fifo,
    /// A named event per segment (Windows).
    Event,
}

#[cfg(target_os = "linux")]
fn native() -> Backend {
    Backend::Futex
}

#[cfg(target_os = "macos")]
fn native() -> Backend {
    if os_sync::available() {
        Backend::OsSync
    } else if ulock::available() {
        Backend::Ulock
    } else {
        Backend::Fifo
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn native() -> Backend {
    Backend::Fifo
}

#[cfg(windows)]
fn native() -> Backend {
    Backend::Event
}

#[cfg(test)]
thread_local! {
    /// The backend the transports and doorbells opened on this thread use,
    /// when a test sets one; see [`force`].
    static FORCED: core::cell::Cell<Option<Backend>> = const { core::cell::Cell::new(None) };
}

/// Makes the transports and doorbells this thread opens from here on use
/// `backend` (`None`: the platform's), so one host runs every backend it
/// has.
#[cfg(test)]
pub(crate) fn force(backend: Option<Backend>) {
    FORCED.with(|f| f.set(backend));
}

/// Every backend this host can run.
#[cfg(test)]
pub(crate) fn available() -> Vec<Backend> {
    [
        #[cfg(target_os = "linux")]
        Some(Backend::Futex),
        #[cfg(target_os = "macos")]
        os_sync::available().then_some(Backend::OsSync),
        #[cfg(target_os = "macos")]
        ulock::available().then_some(Backend::Ulock),
        #[cfg(unix)]
        Some(Backend::Fifo),
        #[cfg(windows)]
        Some(Backend::Event),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The backend a segment or doorbell opened now uses.
pub(crate) fn chosen() -> Backend {
    #[cfg(test)]
    if let Some(forced) = FORCED.with(core::cell::Cell::get) {
        return forced;
    }
    native()
}

/// The waiting side and the waking side of one segment or doorbell.
pub(crate) struct Waiter {
    inner: Inner,
}

enum Inner {
    #[cfg(target_os = "linux")]
    Futex,
    #[cfg(target_os = "macos")]
    OsSync,
    #[cfg(target_os = "macos")]
    Ulock,
    #[cfg(unix)]
    Fifo(Fifo),
    #[cfg(windows)]
    Event(Event),
}

impl Waiter {
    /// The waiter of the segment or doorbell `os_id`, with the backend
    /// [`chosen`] names. The FIFO backend makes or opens `<name>.wake` in
    /// the pair directory; the caller holds the pair's lock, under which a
    /// sweep removes a FIFO whose segment is gone.
    ///
    /// # Errors
    /// The backend is not available on this host, or its FIFO or event
    /// could not be made or opened.
    pub(crate) fn open(os_id: &str) -> io::Result<Self> {
        let inner = match chosen() {
            #[cfg(target_os = "linux")]
            Backend::Futex => Inner::Futex,
            #[cfg(target_os = "macos")]
            Backend::OsSync if os_sync::available() => Inner::OsSync,
            #[cfg(target_os = "macos")]
            Backend::Ulock if ulock::available() => Inner::Ulock,
            #[cfg(unix)]
            Backend::Fifo => Inner::Fifo(Fifo::open(os_id)?),
            #[cfg(windows)]
            Backend::Event => Inner::Event(Event::open(os_id)?),
            #[allow(unreachable_patterns)]
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!("shm wait backend {other:?} is not available on this host"),
                ));
            }
        };
        Ok(Self { inner })
    }

    /// The backend this waiter uses.
    #[cfg(test)]
    pub(crate) fn backend(&self) -> Backend {
        match self.inner {
            #[cfg(target_os = "linux")]
            Inner::Futex => Backend::Futex,
            #[cfg(target_os = "macos")]
            Inner::OsSync => Backend::OsSync,
            #[cfg(target_os = "macos")]
            Inner::Ulock => Backend::Ulock,
            #[cfg(unix)]
            Inner::Fifo(_) => Backend::Fifo,
            #[cfg(windows)]
            Inner::Event(_) => Backend::Event,
        }
    }

    /// Parks until `*word != expected`, a [`Self::wake`] since the caller
    /// read `expected`, or `timeout`. A spurious return is harmless: the
    /// caller looks at its work again whatever the return was.
    pub(crate) fn park(&self, word: *const u32, expected: u32, timeout: Option<Duration>) {
        let _ = (word, expected);
        match &self.inner {
            #[cfg(target_os = "linux")]
            Inner::Futex => futex::wait(word, expected, timeout),
            #[cfg(target_os = "macos")]
            Inner::OsSync => os_sync::wait(word, expected, timeout),
            #[cfg(target_os = "macos")]
            Inner::Ulock => ulock::wait(word, expected, timeout),
            #[cfg(unix)]
            Inner::Fifo(fifo) => fifo.park(timeout),
            #[cfg(windows)]
            Inner::Event(event) => event.wait(timeout),
        }
    }

    /// Wakes the waiters parked on `word`; call it after the word changed.
    pub(crate) fn wake(&self, word: *const u32) {
        let _ = word;
        match &self.inner {
            #[cfg(target_os = "linux")]
            Inner::Futex => futex::wake(word),
            #[cfg(target_os = "macos")]
            Inner::OsSync => os_sync::wake(word),
            #[cfg(target_os = "macos")]
            Inner::Ulock => ulock::wake(word),
            #[cfg(unix)]
            Inner::Fifo(fifo) => fifo.wake(),
            #[cfg(windows)]
            Inner::Event(event) => event.set(),
        }
    }

    /// Removes what this backend keeps in the file system for the segment
    /// (the FIFO): called by the one that ends the segment.
    pub(crate) fn remove_files(&self) {
        match &self.inner {
            #[cfg(unix)]
            Inner::Fifo(fifo) => {
                let _ = std::fs::remove_file(&fifo.path);
            }
            // The other backends keep nothing in the file system; on the
            // Unixes but Linux and macOS the FIFO is the only one.
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            _ => {}
        }
    }
}

/// A shared futex on the word (Linux). Shared (no `FUTEX_PRIVATE_FLAG`):
/// wait and wake key on the physical page and work across the processes
/// that map it at different addresses.
#[cfg(target_os = "linux")]
mod futex {
    use std::time::Duration;

    pub(super) fn wake(addr: *const u32) {
        // SAFETY: `addr` points to a live `u32` inside a shared mapping that
        // outlives this call. `FUTEX_WAKE` ignores `*addr` and only signals
        // parked waiters; the trailing args are unused for WAKE.
        unsafe {
            libc::syscall(
                libc::SYS_futex,
                addr,
                libc::FUTEX_WAKE,
                i32::MAX,
                core::ptr::null::<libc::timespec>(),
                core::ptr::null::<u32>(),
                0,
            );
        }
    }

    /// `FUTEX_WAIT` compares `*addr == expected` atomically before it
    /// sleeps, so a wake after the caller read `expected` returns `EAGAIN`
    /// at once.
    pub(super) fn wait(addr: *const u32, expected: u32, timeout: Option<Duration>) {
        let ts = timeout.map(|d| libc::timespec {
            tv_sec: libc::time_t::try_from(d.as_secs()).unwrap_or(libc::time_t::MAX),
            // Below 10^9: fits a `c_long` of 32 bits as well.
            tv_nsec: d.subsec_nanos() as libc::c_long,
        });
        let ts_ptr = ts
            .as_ref()
            .map_or(core::ptr::null(), |t| t as *const libc::timespec);
        // SAFETY: `addr` is a live `u32` in a shared mapping; the relative
        // `timeout` (or NULL for infinite) points to a valid stack
        // `timespec` for the duration of the call.
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

/// `os_sync_wait_on_address` (macOS 14.4) with
/// `OS_SYNC_WAIT_ON_ADDRESS_SHARED`: parks on a word several processes map,
/// as a shared futex does. Measured on macOS 26 before the other backends
/// came: 100 waiting consumers 0.3 % of a core; with the sleep-poll it was
/// 4.45 % of a core per consumer.
#[cfg(target_os = "macos")]
mod os_sync {
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

    pub(super) fn available() -> bool {
        api().is_some()
    }

    pub(super) fn wake(addr: *const u32) {
        if let Some((_, wake_all)) = api() {
            // SAFETY: `addr` is a live, aligned `u32` in a shared mapping
            // that outlives this call.
            unsafe {
                wake_all(addr as *mut c_void, 4, libc::OS_SYNC_WAKE_BY_ADDRESS_SHARED);
            }
        }
    }

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

/// `__ulock_wait` / `__ulock_wake` with `UL_COMPARE_AND_WAIT_SHARED` (xnu
/// `sys/ulock.h`): the macOS wait on a shared word before
/// `os_sync_wait_on_address`, which is built on it. The shared operation
/// came with macOS 10.15 (Darwin 19); before that the FIFO stays. Rust's
/// `std` waits through `__ulock_wait` on the macOS versions without
/// `os_sync` as well.
#[cfg(target_os = "macos")]
mod ulock {
    use core::ffi::c_void;
    use std::sync::OnceLock;
    use std::time::Duration;

    /// `int __ulock_wait(uint32_t operation, void *addr, uint64_t value,
    /// uint32_t timeout_us)`; a timeout of 0 waits without one.
    type WaitFn = unsafe extern "C" fn(u32, *mut c_void, u64, u32) -> i32;
    /// `int __ulock_wake(uint32_t operation, void *addr, uint64_t
    /// wake_value)`.
    type WakeFn = unsafe extern "C" fn(u32, *mut c_void, u64) -> i32;

    const UL_COMPARE_AND_WAIT_SHARED: u32 = 3;
    const ULF_WAKE_ALL: u32 = 0x0000_0100;
    const ULF_NO_ERRNO: u32 = 0x0100_0000;

    fn api() -> Option<(WaitFn, WakeFn)> {
        static API: OnceLock<Option<(WaitFn, WakeFn)>> = OnceLock::new();
        *API.get_or_init(|| {
            if darwin_major()? < 19 {
                return None;
            }
            // SAFETY: dlsym with NUL-terminated names. A non-null result is
            // the libSystem function of that name, whose signature the two
            // types above repeat.
            unsafe {
                let wait = libc::dlsym(libc::RTLD_DEFAULT, c"__ulock_wait".as_ptr());
                let wake = libc::dlsym(libc::RTLD_DEFAULT, c"__ulock_wake".as_ptr());
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

    /// The major number of the running Darwin kernel (`uname -r`).
    fn darwin_major() -> Option<u32> {
        // SAFETY: a zeroed `utsname` is a valid out-parameter.
        let mut name: libc::utsname = unsafe { core::mem::zeroed() };
        // SAFETY: `name` is writable.
        if unsafe { libc::uname(&mut name) } != 0 {
            return None;
        }
        // SAFETY: uname wrote a NUL-terminated release string.
        let release = unsafe { core::ffi::CStr::from_ptr(name.release.as_ptr()) };
        release.to_str().ok()?.split('.').next()?.parse().ok()
    }

    pub(super) fn available() -> bool {
        api().is_some()
    }

    pub(super) fn wake(addr: *const u32) {
        if let Some((_, ulock_wake)) = api() {
            // SAFETY: `addr` is a live, aligned `u32` in a shared mapping
            // that outlives this call.
            unsafe {
                ulock_wake(
                    UL_COMPARE_AND_WAIT_SHARED | ULF_WAKE_ALL | ULF_NO_ERRNO,
                    addr as *mut c_void,
                    0,
                );
            }
        }
    }

    pub(super) fn wait(addr: *const u32, expected: u32, timeout: Option<Duration>) {
        let Some((ulock_wait, _)) = api() else {
            return;
        };
        // 0 waits without a timeout, so a timeout rounds up to 1 µs.
        let us = timeout.map_or(0, |d| {
            u32::try_from(d.as_micros()).unwrap_or(u32::MAX).max(1)
        });
        // SAFETY: as in `wake`; the kernel compares the `u32` at `addr`
        // with `expected` before it sleeps.
        unsafe {
            ulock_wait(
                UL_COMPARE_AND_WAIT_SHARED | ULF_NO_ERRNO,
                addr as *mut c_void,
                u64::from(expected),
                us,
            );
        }
    }
}

/// The FIFO of a segment: `<name>.wake` in the pair directory, opened for
/// reading and writing without blocking by both sides.
#[cfg(unix)]
struct Fifo {
    file: std::fs::File,
    path: std::path::PathBuf,
}

#[cfg(unix)]
impl Fifo {
    fn open(os_id: &str) -> io::Result<Self> {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
        let path = crate::posix::pair_file(os_id, "wake")?;
        let name = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(io::Error::other)?;
        // SAFETY: a NUL-terminated path; the result is checked.
        if unsafe { libc::mkfifo(name.as_ptr(), 0o600) } != 0 {
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::AlreadyExists {
                return Err(e);
            }
        }
        // Read and write: the writer's byte never meets a FIFO without a
        // reader, and the waiter's poll never sees the end of a pipe
        // without writers.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)?;
        if !file.metadata()?.file_type().is_fifo() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} is not a FIFO", path.display()),
            ));
        }
        Ok(Self { file, path })
    }

    /// One byte into the pipe. A full pipe (`EAGAIN`) holds wakes enough.
    fn wake(&self) {
        use std::io::Write;
        let _ = (&self.file).write(&[1]);
    }

    /// Polls until a byte is in the pipe or `timeout`, then drains it.
    fn park(&self, timeout: Option<Duration>) {
        use std::io::Read;
        use std::os::fd::AsRawFd;
        // Rounded up: a remainder under a millisecond polled with 0 and
        // returned at once, a busy loop until the deadline.
        let ms = timeout.map_or(-1, |d| {
            libc::c_int::try_from(d.as_nanos().div_ceil(1_000_000)).unwrap_or(libc::c_int::MAX)
        });
        let mut fd = libc::pollfd {
            fd: self.file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd; an interrupted poll returns early, which
        // the caller treats as any wake.
        unsafe {
            libc::poll(&mut fd, 1, ms);
        }
        let mut drain = [0u8; 64];
        while matches!((&self.file).read(&mut drain), Ok(n) if n > 0) {}
    }
}

/// Windows has no wait on an address across processes (`WaitOnAddress`
/// keys on the process's own address space), so every segment and
/// doorbell has a named auto-reset event: the writer sets it after each
/// frame, the waiter waits on it.
#[cfg(windows)]
struct Event(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Event {
    /// Creates the event of `os_id`, or opens it if the other side made it
    /// first.
    fn open(os_id: &str) -> io::Result<Self> {
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

    fn set(&self) {
        // SAFETY: a live event handle owned by `self`.
        unsafe {
            windows_sys::Win32::System::Threading::SetEvent(self.0);
        }
    }

    fn wait(&self, timeout: Option<Duration>) {
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
impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: the handle `open` returned, closed once.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
