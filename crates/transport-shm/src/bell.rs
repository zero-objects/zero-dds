// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A participant's doorbell: one word in a small shared segment.
//!
//! A participant that receives over several rings (one per same-host peer)
//! parks one thread on its doorbell instead of one thread on each ring. A
//! sender pushes its frame into the ring, then rings the receiver's bell:
//! the word goes up by one and the parked thread wakes and drains every
//! ring. The receiver reads the word before it drains, and parks only while
//! the word still has that value, so a ring between its look at the rings
//! and its park is not lost.
//!
//! The wait is the one the rings use on each platform: a shared futex on
//! Linux, `os_sync_wait_on_address` on macOS from 14.4, a named event on
//! Windows, a short sleep elsewhere.
//!
//! On Linux the participant holds a shared `flock` on its bell's file in
//! `/dev/shm` for as long as the bell is open, as the users of a ring do;
//! the kernel drops it when the process ends, by `SIGKILL` too.
//! [`sweep_orphaned_doorbells`] unlinks the bells nobody holds, which a
//! process that ended without dropping its bell left behind. macOS and the
//! other Unixes list no shared-memory objects: there whoever makes a bell
//! leaves a marker `<name>.bell` in the channel's flink directory, the
//! participant holds the lock on that, and
//! `crate::posix::sweep_orphaned_flinks` (Unix other than Linux) reads it.

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use crate::mapping::{Mapping, ShmemError};

use crate::posix::PosixShmError;

/// Magic of a doorbell segment, `ZBEL` big-endian.
const BELL_MAGIC: u32 = u32::from_be_bytes(*b"ZBEL");
/// Layout version of a doorbell segment.
const BELL_VERSION: u32 = 1;
/// Magic at 0, version at 4, the word at 8.
const BELL_BYTES: usize = 64;

/// The start of every doorbell's shared-memory name.
pub const BELL_NAME_PREFIX: &str = "zdb-";

/// The shared-memory name of the doorbell of `id`: `/zdb-` and the id in
/// hex, or on macOS, whose names stop at 31 characters, 13 bytes of an
/// FNV-1a-128 hash of it.
fn bell_os_id(id: [u8; 16]) -> String {
    use std::fmt::Write;
    let mut s = format!("/{BELL_NAME_PREFIX}");
    let bytes: Vec<u8> = if cfg!(target_os = "macos") {
        crate::posix::fnv1a_128_bytes(&id)[..13].to_vec()
    } else {
        id.to_vec()
    };
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// One participant's doorbell, mapped by the participant itself (to park
/// on it) or by a sender (to ring it).
pub struct ShmDoorbell {
    shmem: Mapping,
    os_id: String,
    /// The participant's own bell: its drop unlinks the name.
    own: bool,
    /// The shared `flock` on the bell's `/dev/shm` file (Linux) or on its
    /// marker (other Unixes) while it is the participant's own, the mark
    /// the sweep reads as "in use".
    #[cfg(unix)]
    _hold: Option<std::fs::File>,
    /// The marker of the participant's own bell, removed with it.
    #[cfg(all(unix, not(target_os = "linux")))]
    marker: Option<std::path::PathBuf>,
    #[cfg(windows)]
    event: crate::posix::RingEventHandle,
}

// SAFETY: the mapping lives as long as the struct, and every access to it
// after construction goes through the `AtomicU32` at offset 8.
unsafe impl Send for ShmDoorbell {}
// SAFETY: as above; `ring`, `sequence` and `park` only use atomics and
// the platform wait on that word.
unsafe impl Sync for ShmDoorbell {}

impl ShmDoorbell {
    /// Opens the doorbell of `id`, creating it if nobody has yet. `own`:
    /// this is the participant whose bell it is. `lock_dir`, the channel's
    /// flink directory, holds the bell's marker where the system lists no
    /// shared-memory objects (all Unixes but Linux).
    ///
    /// # Errors
    /// The segment could not be created or mapped, what is there under the
    /// name is not a doorbell of this version, or the bell could not be
    /// marked as held.
    pub fn open(id: [u8; 16], own: bool, lock_dir: &Path) -> Result<Self, PosixShmError> {
        let os_id = bell_os_id(id);
        let (mut shmem, created) = match Mapping::create(&os_id, BELL_BYTES, None) {
            Ok(s) => (s, true),
            Err(ShmemError::MappingIdExists) => (open_made_bell(&os_id)?, false),
            Err(e) => return Err(e.into()),
        };
        if shmem.len() < BELL_BYTES {
            return Err(PosixShmError::InvalidHeader);
        }
        let base = shmem.as_ptr();
        if created {
            // SAFETY: the mapping is at least BELL_BYTES long; nobody else
            // reads it before the magic is there.
            unsafe {
                core::ptr::write_unaligned(base.add(4) as *mut u32, BELL_VERSION.to_le());
                (*(base as *const AtomicU32)).store(BELL_MAGIC.to_be(), Ordering::Release);
            }
            shmem.set_owner(own);
        } else {
            // The creator writes the header right after creating; give it
            // a moment.
            let mut tries = 0;
            loop {
                // SAFETY: the mapping is at least BELL_BYTES long.
                let raw = unsafe { (*(base as *const AtomicU32)).load(Ordering::Acquire) };
                let magic = u32::from_be(raw);
                if magic == BELL_MAGIC {
                    break;
                }
                tries += 1;
                if tries > 100 {
                    return Err(PosixShmError::InvalidHeader);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            // SAFETY: the mapping is at least BELL_BYTES long.
            let raw = unsafe { core::ptr::read_unaligned(base.add(4) as *const u32) };
            let version = u32::from_le(raw);
            if version != BELL_VERSION {
                return Err(PosixShmError::InvalidHeader);
            }
        }
        #[cfg(target_os = "linux")]
        let hold = {
            let _ = lock_dir;
            if own { Some(hold_bell(&os_id)?) } else { None }
        };
        #[cfg(all(unix, not(target_os = "linux")))]
        let (hold, marker) = mark_bell(lock_dir, &os_id, own)?;
        #[cfg(not(unix))]
        let _ = lock_dir;
        #[cfg(windows)]
        let event = crate::posix::RingEventHandle::open(&os_id)?;
        Ok(Self {
            shmem,
            os_id,
            own,
            #[cfg(unix)]
            _hold: hold,
            #[cfg(all(unix, not(target_os = "linux")))]
            marker: if own { Some(marker) } else { None },
            #[cfg(windows)]
            event,
        })
    }

    fn word(&self) -> &AtomicU32 {
        // SAFETY: offset 8 lies in the mapping (BELL_BYTES), 4-aligned.
        unsafe { &*(self.shmem.as_ptr().add(8) as *const AtomicU32) }
    }

    /// The current value of the word: read it before draining the rings,
    /// park on it after.
    #[must_use]
    pub fn sequence(&self) -> u32 {
        self.word().load(Ordering::Acquire)
    }

    /// Rings: the word goes up, and the parked receiver wakes.
    pub fn ring(&self) {
        self.word().fetch_add(1, Ordering::AcqRel);
        crate::posix::park_wake(self.word().as_ptr() as *const u32);
        #[cfg(windows)]
        self.event.set();
    }

    /// Parks until the word is no longer `seen`, a ring, or `timeout`.
    pub fn park(&self, seen: u32, timeout: Option<Duration>) {
        if self.sequence() != seen {
            return;
        }
        #[cfg(windows)]
        {
            self.event.wait(timeout);
        }
        #[cfg(not(windows))]
        match crate::posix::park_fn() {
            Some(park) => park(self.word().as_ptr() as *const u32, seen, timeout),
            None => std::thread::sleep(timeout.map_or(Duration::from_millis(1), |t| {
                t.min(Duration::from_millis(1))
            })),
        }
    }
}

/// Takes a shared `flock` on the bell `os_id`'s file in `/dev/shm`.
#[cfg(target_os = "linux")]
fn hold_bell(os_id: &str) -> Result<std::fs::File, PosixShmError> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::AsRawFd;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(std::path::Path::new("/dev/shm").join(os_id.trim_start_matches('/')))
        .map_err(PosixShmError::Io)?;
    // SAFETY: flock on the valid fd of `file`.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH) } != 0 {
        return Err(PosixShmError::Io(std::io::Error::last_os_error()));
    }
    Ok(file)
}

/// Opens bell `os_id` that another process made. Its creator's `shm_open`
/// and the `ftruncate` that sizes it are two calls; in between the object
/// is 0 bytes long and mapping it fails with `EINVAL` (seen in a
/// multi-process test chain: `shm.channel_open_failed`, "os error 22"). That is waited out
/// for up to 100 ms, as the header is.
fn open_made_bell(os_id: &str) -> Result<Mapping, PosixShmError> {
    let mut tries = 0;
    loop {
        match Mapping::open_os_id(os_id) {
            Ok(shmem) if shmem.len() >= BELL_BYTES => return Ok(shmem),
            Ok(_) | Err(ShmemError::MapOpenFailed(_)) if tries < 100 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(_) => return Err(PosixShmError::InvalidHeader),
            Err(e) => return Err(e.into()),
        }
    }
}

/// The marker of bell `os_id` in `lock_dir`: made if missing, and for the
/// participant's own bell held with a shared `flock` (returned).
#[cfg(all(unix, not(target_os = "linux")))]
fn mark_bell(
    lock_dir: &Path,
    os_id: &str,
    own: bool,
) -> Result<(Option<std::fs::File>, std::path::PathBuf), PosixShmError> {
    use std::os::unix::io::AsRawFd;
    std::fs::create_dir_all(lock_dir).map_err(PosixShmError::Io)?;
    let path = bell_marker(lock_dir, os_id);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(&path)
        .map_err(PosixShmError::Io)?;
    if !own {
        return Ok((None, path));
    }
    // SAFETY: flock on the valid fd of `file`.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH) } != 0 {
        return Err(PosixShmError::Io(std::io::Error::last_os_error()));
    }
    Ok((Some(file), path))
}

/// Where bell `os_id`'s marker lives in `lock_dir`.
#[cfg(all(unix, not(target_os = "linux")))]
fn bell_marker(lock_dir: &Path, os_id: &str) -> std::path::PathBuf {
    lock_dir.join(format!("{}.bell", os_id.trim_start_matches('/')))
}

/// What a bell marker's segment is: a bell of this version, something
/// else, or gone.
#[cfg(all(unix, not(target_os = "linux")))]
pub(crate) enum BellSegment {
    Current,
    Other,
    Missing,
}

/// The state of the segment named `os_id`, for the flink sweep.
#[cfg(all(unix, not(target_os = "linux")))]
pub(crate) fn bell_segment(os_id: &str) -> BellSegment {
    let Ok(shmem) = Mapping::open_os_id(os_id) else {
        return BellSegment::Missing;
    };
    if shmem.len() < 8 {
        return BellSegment::Other;
    }
    let mut header = [0u8; 8];
    // SAFETY: the mapping is at least 8 bytes long.
    unsafe { core::ptr::copy_nonoverlapping(shmem.as_ptr(), header.as_mut_ptr(), 8) };
    if header[..4] == *b"ZBEL" && header[4..] == BELL_VERSION.to_le_bytes() {
        BellSegment::Current
    } else {
        BellSegment::Other
    }
}

/// Unlinks the doorbells in `/dev/shm` no participant holds, older than
/// `min_age`: those of processes that ended without dropping their bell,
/// and those a sender made for a peer that never came.
///
/// # Errors
/// Reading `/dev/shm` failed; a bell that cannot be opened or locked is
/// skipped.
#[cfg(target_os = "linux")]
pub fn sweep_orphaned_doorbells(min_age: Duration) -> std::io::Result<crate::posix::SweepReport> {
    sweep_bells(BELL_NAME_PREFIX, min_age)
}

/// The sweep of [`sweep_orphaned_doorbells`] over the names under `prefix`.
#[cfg(target_os = "linux")]
fn sweep_bells(prefix: &str, min_age: Duration) -> std::io::Result<crate::posix::SweepReport> {
    crate::posix::sweep_segments(prefix, min_age, |header| {
        header[..4] == *b"ZBEL" && header[4..] == BELL_VERSION.to_le_bytes()
    })
}

impl Drop for ShmDoorbell {
    fn drop(&mut self) {
        if self.own {
            crate::posix::unlink_os_id(&self.os_id);
            #[cfg(all(unix, not(target_os = "linux")))]
            if let Some(marker) = &self.marker {
                let _ = std::fs::remove_file(marker);
            }
        }
    }
}

impl core::fmt::Debug for ShmDoorbell {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShmDoorbell")
            .field("os_id", &self.os_id)
            .field("own", &self.own)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    fn id(n: u8) -> [u8; 16] {
        let mut a = [0u8; 16];
        a[..4].copy_from_slice(&std::process::id().to_le_bytes());
        a[15] = n;
        a
    }

    /// A ring from another mapping wakes a parked receiver at once, and a
    /// ring that came before the park is not lost.
    #[test]
    fn a_ring_wakes_the_parked_receiver_and_none_is_lost() {
        let dir = tempfile::tempdir().unwrap();
        let own = Arc::new(ShmDoorbell::open(id(1), true, dir.path()).unwrap());
        let sender = ShmDoorbell::open(id(1), false, dir.path()).unwrap();
        let seen = own.sequence();
        sender.ring();
        let start = Instant::now();
        own.park(seen, Some(Duration::from_secs(10)));
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "the earlier ring counts"
        );

        let waiting = Arc::clone(&own);
        let seen = own.sequence();
        let t = std::thread::spawn(move || {
            let start = Instant::now();
            waiting.park(seen, Some(Duration::from_secs(10)));
            start.elapsed()
        });
        std::thread::sleep(Duration::from_millis(100));
        sender.ring();
        assert!(t.join().unwrap() < Duration::from_secs(5));
        assert_eq!(own.sequence(), seen.wrapping_add(1));
    }

    /// The sweep leaves a participant's own bell alone while it is open and
    /// unlinks a bell nobody holds, as a killed process or a sender that
    /// made a bell for a peer that never came leaves it.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_sweep_removes_orphaned_bells_and_keeps_held_ones() {
        let dir = tempfile::tempdir().unwrap();
        let own = ShmDoorbell::open(id(4), true, dir.path()).unwrap();
        // A sender's bell for a peer that never opened it: no lock, and its
        // drop does not unlink the name.
        drop(ShmDoorbell::open(id(5), false, dir.path()).unwrap());
        let orphan = std::path::Path::new("/dev/shm").join(&bell_os_id(id(5))[1..]);
        assert!(orphan.exists());
        // Only this process's test bells ("zdb-" and the PID's 4 bytes in
        // hex): a sweep with no minimum age over every bell could take the
        // fresh, not yet locked bell of another process on the host.
        let mine = &bell_os_id(id(0))[1..13];
        let report = sweep_bells(mine, Duration::ZERO).unwrap();
        assert!(!orphan.exists(), "the orphaned bell is unlinked");
        assert!(report.removed >= 1 && report.held >= 1, "{report:?}");
        let held = std::path::Path::new("/dev/shm").join(&bell_os_id(id(4))[1..]);
        assert!(held.exists(), "the open bell stays");
        drop(own);
        assert!(!held.exists(), "its own drop unlinks it");
    }

    /// macOS: the flink sweep unlinks a bell whose marker nobody holds and
    /// removes the marker; it keeps a held bell, and the bell's own drop
    /// takes the marker with it.
    #[cfg(all(unix, not(target_os = "linux")))]
    #[test]
    fn the_flink_sweep_removes_orphaned_bells_and_keeps_held_ones() {
        let dir = tempfile::tempdir().unwrap();
        let own = ShmDoorbell::open(id(6), true, dir.path()).unwrap();
        drop(ShmDoorbell::open(id(7), false, dir.path()).unwrap());
        let orphan = bell_marker(dir.path(), &bell_os_id(id(7)));
        let held = bell_marker(dir.path(), &bell_os_id(id(6)));
        assert!(orphan.exists() && held.exists());
        let report = crate::posix::sweep_orphaned_flinks(dir.path(), Duration::ZERO).unwrap();
        assert_eq!((report.removed, report.held), (1, 1), "{report:?}");
        assert!(!orphan.exists(), "the orphan's marker goes");
        assert!(
            matches!(bell_segment(&bell_os_id(id(7))), BellSegment::Missing),
            "the orphaned bell is unlinked"
        );
        drop(own);
        assert!(!held.exists(), "the own bell's drop removes its marker");
        assert!(matches!(
            bell_segment(&bell_os_id(id(6))),
            BellSegment::Missing
        ));
    }

    /// A sender that opens a bell between its creator's `shm_open` and the
    /// `ftruncate` that sizes it waits for the size instead of failing.
    #[cfg(unix)]
    #[test]
    fn a_bell_its_creator_has_not_sized_yet_is_waited_for() {
        let dir = tempfile::tempdir().unwrap();
        let name = std::ffi::CString::new(bell_os_id(id(8))).unwrap();
        // SAFETY: a valid NUL-terminated name; the object is new.
        let fd = unsafe {
            libc::shm_open(
                name.as_ptr(),
                libc::O_CREAT | libc::O_EXCL | libc::O_RDWR,
                0o600,
            )
        };
        assert!(fd >= 0, "{}", std::io::Error::last_os_error());
        let maker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            // SAFETY: `fd` is the open object; the mapping is BELL_BYTES
            // long once it is sized, and the header is written as `open`
            // writes it.
            unsafe {
                assert_eq!(libc::ftruncate(fd, BELL_BYTES as libc::off_t), 0);
                let base = libc::mmap(
                    core::ptr::null_mut(),
                    BELL_BYTES,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    fd,
                    0,
                )
                .cast::<u8>();
                assert_ne!(base.cast(), libc::MAP_FAILED);
                core::ptr::write_unaligned(base.add(4).cast::<u32>(), BELL_VERSION.to_le());
                (*base.cast::<AtomicU32>()).store(BELL_MAGIC.to_be(), Ordering::Release);
                libc::munmap(base.cast(), BELL_BYTES);
                libc::close(fd);
            }
        });
        let sender = ShmDoorbell::open(id(8), false, dir.path());
        maker.join().unwrap();
        // SAFETY: the name as above.
        unsafe { libc::shm_unlink(name.as_ptr()) };
        assert!(sender.is_ok(), "{:?}", sender.err());
    }

    /// The name of a doorbell differs from the rings' (`zd-`) and fits
    /// macOS' 31 characters.
    #[test]
    fn bell_names_are_apart_from_the_rings_and_fit_macos() {
        let name = bell_os_id(id(2));
        assert!(name.starts_with("/zdb-"));
        assert_ne!(name, bell_os_id(id(3)));
        #[cfg(target_os = "macos")]
        assert!(name.len() <= 31, "{name}");
    }
}
