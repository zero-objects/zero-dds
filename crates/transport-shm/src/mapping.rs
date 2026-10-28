// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Named shared-memory mappings for the rings and the doorbells.
//!
//! On Unix a mapping holds no file descriptor: the object is opened, sized
//! and mapped, and its descriptor is closed at once, which leaves the
//! mapping in place (POSIX `mmap`). `shared_memory` kept the descriptor
//! until the drop, one per mapping, and a participant maps two rings and a
//! doorbell per same-host peer: at macOS' default limit of 256 descriptors
//! a process of a multi-process test chain ran out (`Too many open files`, 62 of its
//! 256 descriptors shared-memory objects). The names, the flink files and
//! who unlinks are as `shared_memory` has them, so both sides of a pair
//! find each other whichever of the two made the segment.
//!
//! Windows keeps `shared_memory`: its mapping lives on a backing file and
//! a handle, and its descriptor limit is not the one that ran out.

use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;

pub(crate) use shared_memory::ShmemError;

/// A named shared-memory object mapped into this process.
pub(crate) struct Mapping {
    #[cfg(unix)]
    inner: unix::Mapping,
    #[cfg(not(unix))]
    inner: shared_memory::Shmem,
}

impl Mapping {
    /// Makes the object `os_id` of `size` bytes, fails with
    /// `MappingIdExists` if it is there, and writes its name to `flink`.
    /// The creator is its owner: its drop unlinks the name and the flink.
    pub(crate) fn create(
        os_id: &str,
        size: usize,
        flink: Option<&Path>,
    ) -> Result<Self, ShmemError> {
        #[cfg(unix)]
        let inner = unix::Mapping::create(os_id, size, flink)?;
        #[cfg(not(unix))]
        let inner = {
            let mut conf = shared_memory::ShmemConf::new().os_id(os_id).size(size);
            if let Some(flink) = flink {
                conf = conf.flink(flink);
            }
            conf.create()?
        };
        Ok(Self { inner })
    }

    /// Maps the object `os_id`.
    pub(crate) fn open_os_id(os_id: &str) -> Result<Self, ShmemError> {
        #[cfg(unix)]
        let inner = unix::Mapping::open(os_id)?;
        #[cfg(not(unix))]
        let inner = shared_memory::ShmemConf::new().os_id(os_id).open()?;
        Ok(Self { inner })
    }

    /// Maps the object whose name `flink` holds.
    pub(crate) fn open_flink(flink: &Path) -> Result<Self, ShmemError> {
        #[cfg(unix)]
        let inner = unix::Mapping::open_flink(flink)?;
        #[cfg(not(unix))]
        let inner = shared_memory::ShmemConf::new().flink(flink).open()?;
        Ok(Self { inner })
    }

    /// Start of the mapping.
    pub(crate) fn as_ptr(&self) -> *mut u8 {
        self.inner.as_ptr()
    }

    /// Length of the mapping in bytes.
    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether this mapping's drop unlinks the name (and the flink).
    pub(crate) fn set_owner(&mut self, owner: bool) {
        let _ = self.inner.set_owner(owner);
    }
}

#[cfg(unix)]
mod unix {
    use super::{Path, PathBuf, ShmemError};
    use std::ffi::CString;
    use std::io::Write;

    pub(super) struct Mapping {
        ptr: *mut u8,
        len: usize,
        os_id: String,
        flink: Option<PathBuf>,
        owner: bool,
    }

    fn errno() -> u32 {
        std::io::Error::last_os_error()
            .raw_os_error()
            .and_then(|e| u32::try_from(e).ok())
            .unwrap_or(0)
    }

    fn c_name(os_id: &str, error: fn(u32) -> ShmemError) -> Result<CString, ShmemError> {
        CString::new(os_id).map_err(|_| error(libc::EINVAL as u32))
    }

    /// Maps `len` bytes of `fd` shared and closes `fd`, mapped or not.
    fn map_and_close(fd: libc::c_int, len: usize) -> Result<*mut u8, u32> {
        // SAFETY: `fd` is an open shared-memory object at least `len` bytes
        // long; the mapping is checked against MAP_FAILED below.
        let ptr = unsafe {
            libc::mmap(
                core::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        let error = errno();
        // SAFETY: `fd` is ours; the mapping, if any, does not need it.
        unsafe { libc::close(fd) };
        if ptr == libc::MAP_FAILED {
            return Err(error);
        }
        Ok(ptr.cast())
    }

    impl Mapping {
        pub(super) fn create(
            os_id: &str,
            size: usize,
            flink: Option<&Path>,
        ) -> Result<Self, ShmemError> {
            if size == 0 {
                return Err(ShmemError::MapSizeZero);
            }
            if flink.is_some_and(Path::is_file) {
                return Err(ShmemError::LinkExists);
            }
            let name = c_name(os_id, ShmemError::MapCreateFailed)?;
            // SAFETY: a NUL-terminated name; the result is checked.
            let fd = unsafe {
                libc::shm_open(
                    name.as_ptr(),
                    libc::O_CREAT | libc::O_EXCL | libc::O_RDWR,
                    0o600,
                )
            };
            if fd < 0 {
                let error = errno();
                return Err(if error == libc::EEXIST as u32 {
                    ShmemError::MappingIdExists
                } else {
                    ShmemError::MapCreateFailed(error)
                });
            }
            let Ok(length) = libc::off_t::try_from(size) else {
                // SAFETY: `fd` and the name are ours.
                unsafe {
                    libc::close(fd);
                    libc::shm_unlink(name.as_ptr());
                }
                return Err(ShmemError::MapCreateFailed(libc::EINVAL as u32));
            };
            // SAFETY: `fd` is the new object.
            if unsafe { libc::ftruncate(fd, length) } != 0 {
                let error = errno();
                // SAFETY: `fd` and the name are ours.
                unsafe {
                    libc::close(fd);
                    libc::shm_unlink(name.as_ptr());
                }
                return Err(ShmemError::MapCreateFailed(error));
            }
            let ptr = match map_and_close(fd, size) {
                Ok(ptr) => ptr,
                Err(error) => {
                    // SAFETY: the name is ours.
                    unsafe { libc::shm_unlink(name.as_ptr()) };
                    return Err(ShmemError::MapCreateFailed(error));
                }
            };
            // From here the drop unmaps and, as owner, unlinks.
            let mut mapping = Self {
                ptr,
                len: size,
                os_id: os_id.to_owned(),
                flink: None,
                owner: true,
            };
            if let Some(flink) = flink {
                let mut file = match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(flink)
                {
                    Ok(file) => file,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        return Err(ShmemError::LinkExists);
                    }
                    Err(e) => return Err(ShmemError::LinkCreateFailed(e)),
                };
                if let Err(e) = file.write_all(os_id.as_bytes()) {
                    let _ = std::fs::remove_file(flink);
                    return Err(ShmemError::LinkWriteFailed(e));
                }
                mapping.flink = Some(flink.to_path_buf());
            }
            Ok(mapping)
        }

        pub(super) fn open(os_id: &str) -> Result<Self, ShmemError> {
            let name = c_name(os_id, ShmemError::MapOpenFailed)?;
            // SAFETY: a NUL-terminated name; the result is checked.
            let fd = unsafe { libc::shm_open(name.as_ptr(), libc::O_RDWR, 0) };
            if fd < 0 {
                return Err(ShmemError::MapOpenFailed(errno()));
            }
            // SAFETY: a zeroed `stat` is a valid out-parameter.
            let mut stat: libc::stat = unsafe { core::mem::zeroed() };
            // SAFETY: `fd` is open and `stat` is writable.
            if unsafe { libc::fstat(fd, &mut stat) } != 0 {
                let error = errno();
                // SAFETY: `fd` is ours.
                unsafe { libc::close(fd) };
                return Err(ShmemError::MapOpenFailed(error));
            }
            let len = usize::try_from(stat.st_size).unwrap_or(0);
            if len == 0 {
                // Made but not sized yet: mapping 0 bytes fails with EINVAL,
                // which is what `shared_memory` reported here.
                // SAFETY: `fd` is ours.
                unsafe { libc::close(fd) };
                return Err(ShmemError::MapOpenFailed(libc::EINVAL as u32));
            }
            let ptr = map_and_close(fd, len).map_err(ShmemError::MapOpenFailed)?;
            Ok(Self {
                ptr,
                len,
                os_id: os_id.to_owned(),
                flink: None,
                owner: false,
            })
        }

        pub(super) fn open_flink(flink: &Path) -> Result<Self, ShmemError> {
            let os_id = std::fs::read_to_string(flink).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ShmemError::LinkOpenFailed(e)
                } else {
                    ShmemError::LinkReadFailed(e)
                }
            })?;
            let mut mapping = Self::open(&os_id)?;
            mapping.flink = Some(flink.to_path_buf());
            Ok(mapping)
        }

        pub(super) fn as_ptr(&self) -> *mut u8 {
            self.ptr
        }

        pub(super) fn len(&self) -> usize {
            self.len
        }

        pub(super) fn set_owner(&mut self, owner: bool) -> bool {
            core::mem::replace(&mut self.owner, owner)
        }
    }

    impl Drop for Mapping {
        fn drop(&mut self) {
            // SAFETY: `ptr`/`len` are this mapping's, unmapped once here.
            unsafe { libc::munmap(self.ptr.cast(), self.len) };
            if self.owner {
                if let Ok(name) = CString::new(self.os_id.as_str()) {
                    // SAFETY: a NUL-terminated name.
                    unsafe { libc::shm_unlink(name.as_ptr()) };
                }
                if let Some(flink) = &self.flink {
                    let _ = std::fs::remove_file(flink);
                }
            }
        }
    }
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn name(tag: &str) -> String {
        format!("/zdm-{}-{tag}", std::process::id())
    }

    /// A mapping keeps no descriptor: making and opening a hundred adds
    /// none to the process. Counted in a process of its own that runs only
    /// this probe, since the other tests open and close descriptors in
    /// parallel.
    #[test]
    fn a_mapping_holds_no_file_descriptor() {
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
        let out = command
            .args([
                "--exact",
                "mapping::tests::descriptor_probe",
                "--test-threads=1",
                "--nocapture",
            ])
            .env("ZERODDS_DESCRIPTOR_PROBE", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
    }

    /// The probe of `a_mapping_holds_no_file_descriptor`; a no-op unless
    /// that test runs it.
    #[test]
    fn descriptor_probe() {
        if std::env::var_os("ZERODDS_DESCRIPTOR_PROBE").is_none() {
            return;
        }
        // /dev/fd lists this process's descriptors on Linux and macOS.
        fn open_fds() -> usize {
            std::fs::read_dir("/dev/fd").unwrap().count()
        }
        let before = open_fds();
        let made: Vec<Mapping> = (0..50)
            .map(|i| Mapping::create(&name(&format!("fd{i}")), 4096, None).unwrap())
            .collect();
        let opened: Vec<Mapping> = (0..50)
            .map(|i| Mapping::open_os_id(&name(&format!("fd{i}"))).unwrap())
            .collect();
        assert_eq!(open_fds(), before, "100 mappings, no descriptor");
        drop(opened);
        drop(made);
    }

    /// Both sides see each other's writes; the flink names the object, the
    /// owner's drop takes name and flink, a non-owner's drop neither.
    #[test]
    fn create_open_share_and_the_owner_unlinks() {
        let dir = tempfile::tempdir().unwrap();
        let flink = dir.path().join("pair.shm");
        let os_id = name("pair");
        let made = Mapping::create(&os_id, 4096, Some(&flink)).unwrap();
        assert!(matches!(
            Mapping::create(&os_id, 4096, None),
            Err(ShmemError::MappingIdExists)
        ));
        let opened = Mapping::open_flink(&flink).unwrap();
        // macOS rounds the object up to its page size.
        assert!(opened.len() >= 4096, "{}", opened.len());
        // SAFETY: both map at least 4096 bytes of the same object.
        unsafe {
            made.as_ptr().write(0x5A);
            assert_eq!(opened.as_ptr().read(), 0x5A);
        }
        drop(opened);
        assert!(flink.exists(), "a non-owner's drop leaves the flink");
        drop(made);
        assert!(!flink.exists(), "the owner's drop takes the flink");
        assert!(Mapping::open_os_id(&os_id).is_err(), "and the name");
    }

    /// An object made but not sized yet reads as `MapOpenFailed(EINVAL)`,
    /// as `shared_memory` reported it.
    #[test]
    fn an_unsized_object_does_not_open() {
        let os_id = name("unsized");
        let c = std::ffi::CString::new(os_id.as_str()).unwrap();
        // SAFETY: a NUL-terminated name; the descriptor is closed below.
        let fd = unsafe { libc::shm_open(c.as_ptr(), libc::O_CREAT | libc::O_RDWR, 0o600) };
        assert!(fd >= 0);
        assert!(matches!(
            Mapping::open_os_id(&os_id),
            Err(ShmemError::MapOpenFailed(e)) if e == libc::EINVAL as u32
        ));
        // SAFETY: ours.
        unsafe {
            libc::close(fd);
            libc::shm_unlink(c.as_ptr());
        }
    }
}
