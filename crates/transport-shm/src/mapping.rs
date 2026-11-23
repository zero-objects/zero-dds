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
//! On Windows a segment is a file in [`backing_dir`], where `shared_memory`
//! kept it, mapped through a file mapping object without a name: the
//! views of one file are coherent across processes whichever mapping
//! object they come from (`CreateFileMapping`). `shared_memory` named the
//! object after the segment, and Windows keeps a named object while any
//! handle to it is open: a consumer still mapping a ring its owner had
//! ended made the next create of the pair map that ended ring (its header
//! with the shutdown flag set, its old `head` and `tail`) over a new,
//! empty file, and an open by name after the old object went found that
//! empty file. The unlink is the removal of the file's name
//! ([`remove_name`]); the error type stays `shared_memory`'s, which the
//! public error of the transport carries.

use std::path::{Path, PathBuf};

pub(crate) use shared_memory::ShmemError;

/// A named shared-memory object mapped into this process.
pub(crate) struct Mapping {
    #[cfg(unix)]
    inner: unix::Mapping,
    #[cfg(windows)]
    inner: windows::Mapping,
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
        #[cfg(windows)]
        let inner = windows::Mapping::create(os_id, size, flink)?;
        Ok(Self { inner })
    }

    /// Maps the object `os_id`.
    pub(crate) fn open_os_id(os_id: &str) -> Result<Self, ShmemError> {
        #[cfg(unix)]
        let inner = unix::Mapping::open(os_id)?;
        #[cfg(windows)]
        let inner = windows::Mapping::open(os_id)?;
        Ok(Self { inner })
    }

    /// Maps the object whose name `flink` holds.
    pub(crate) fn open_flink(flink: &Path) -> Result<Self, ShmemError> {
        #[cfg(unix)]
        let inner = unix::Mapping::open_flink(flink)?;
        #[cfg(windows)]
        let inner = windows::Mapping::open_flink(flink)?;
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

/// Whether `e` says there is no object under the name: `ENOENT` from
/// `shm_open` on Unix, `ERROR_FILE_NOT_FOUND` for the missing backing file
/// on Windows.
#[cfg(not(target_os = "linux"))]
pub(crate) fn is_missing(e: &ShmemError) -> bool {
    #[cfg(unix)]
    let missing = libc::ENOENT as u32;
    #[cfg(windows)]
    let missing = windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    matches!(e, ShmemError::MapOpenFailed(code) if *code == missing)
}

/// Where the segments lie on Windows: `shared_memory-rs` in the temporary
/// directory, each a file named like the segment without its leading `/`
/// (where `shared_memory` kept them, so both sides of a pair built before
/// and after find each other's).
#[cfg(windows)]
pub(crate) fn backing_dir() -> PathBuf {
    std::env::temp_dir().join("shared_memory-rs")
}

/// Removes the name `path` at once, as `unlink` does, also while other
/// handles hold the file. On Unix that is `unlink`. On Windows a removal
/// takes the name away at once only where Windows deletes the POSIX way;
/// where it deletes the old way, and under Wine, the name stays until the
/// file's last handle closes, an open of it is refused or reaches the
/// removed file, and a pair's next create or lock fails or takes a file
/// nobody else locks. So the file is first renamed to a name of its own
/// (`<name>.<pid>-<n>.gone`, which the sweeps pass over) and then removed:
/// the name is free at once, and Windows deletes the renamed file with its
/// last handle, also when that process is killed.
///
/// # Errors
/// `NotFound` when nothing has the name; else what the removal answered.
pub(crate) fn remove_name(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::remove_file(path)
    }
    #[cfg(windows)]
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let Some(name) = path.file_name() else {
            return std::fs::remove_file(path);
        };
        let mut gone = name.to_os_string();
        gone.push(format!(
            ".{}-{}.gone",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let gone = path.with_file_name(gone);
        match std::fs::rename(path, &gone) {
            Ok(()) => std::fs::remove_file(gone),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(e),
            // A rename refused (another volume, a directory): the plain
            // removal still marks it.
            Err(_) => std::fs::remove_file(path),
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::{Path, PathBuf, ShmemError};
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_INVALID_PARAMETER, HANDLE,
    };
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_TEMPORARY;
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_READ, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS,
        MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile,
    };

    pub(super) struct Mapping {
        view: *mut u8,
        len: usize,
        object: HANDLE,
        /// The segment's file, open for the life of the mapping.
        _file: std::fs::File,
        path: PathBuf,
        flink: Option<PathBuf>,
        owner: bool,
    }

    fn code(e: &std::io::Error) -> u32 {
        e.raw_os_error()
            .and_then(|c| u32::try_from(c).ok())
            .unwrap_or(0)
    }

    fn last_error() -> u32 {
        code(&std::io::Error::last_os_error())
    }

    /// The segment's file for `os_id`. A name that would leave the
    /// directory is refused.
    fn backing_file(os_id: &str) -> Option<PathBuf> {
        let name = os_id.trim_start_matches('/');
        if name.is_empty() || name.contains(['/', '\\', ':', '\0']) || name.starts_with('.') {
            return None;
        }
        Some(super::backing_dir().join(name))
    }

    /// Maps `len` bytes of `file` shared, through a mapping object without
    /// a name.
    fn map(file: &std::fs::File, len: usize) -> Result<(HANDLE, *mut u8), u32> {
        // SAFETY: the handle of the open file is valid for the call; no
        // name, no security attributes; the size is the file's.
        let object = unsafe {
            CreateFileMappingW(
                file.as_raw_handle() as HANDLE,
                core::ptr::null(),
                PAGE_READWRITE,
                0,
                0,
                core::ptr::null(),
            )
        };
        if object == 0 {
            return Err(last_error());
        }
        // SAFETY: `object` is the mapping object made above; `len` bytes
        // from offset 0 lie in the file.
        let view = unsafe { MapViewOfFile(object, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, len) };
        if view.Value.is_null() {
            let error = last_error();
            // SAFETY: ours, closed once.
            unsafe { CloseHandle(object) };
            return Err(error);
        }
        Ok((object, view.Value.cast()))
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
            let path =
                backing_file(os_id).ok_or(ShmemError::MapCreateFailed(ERROR_INVALID_PARAMETER))?;
            std::fs::create_dir_all(super::backing_dir())
                .map_err(|e| ShmemError::MapCreateFailed(code(&e)))?;
            let file = match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .attributes(FILE_ATTRIBUTE_TEMPORARY)
                .open(&path)
            {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(ShmemError::MappingIdExists);
                }
                Err(e) => return Err(ShmemError::MapCreateFailed(code(&e))),
            };
            let made = file
                .set_len(size as u64)
                .map_err(|e| code(&e))
                .and_then(|()| map(&file, size));
            let (object, view) = match made {
                Ok(made) => made,
                Err(error) => {
                    drop(file);
                    let _ = super::remove_name(&path);
                    return Err(ShmemError::MapCreateFailed(error));
                }
            };
            // From here the drop unmaps and, as owner, removes the name.
            let mut mapping = Self {
                view,
                len: size,
                object,
                _file: file,
                path,
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
            let path =
                backing_file(os_id).ok_or(ShmemError::MapOpenFailed(ERROR_INVALID_PARAMETER))?;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .map_err(|e| {
                    ShmemError::MapOpenFailed(if e.kind() == std::io::ErrorKind::NotFound {
                        ERROR_FILE_NOT_FOUND
                    } else {
                        code(&e)
                    })
                })?;
            let len = file
                .metadata()
                .map_err(|e| ShmemError::MapOpenFailed(code(&e)))
                .map(|m| usize::try_from(m.len()).unwrap_or(0))?;
            if len == 0 {
                // Made but not sized yet, as an unsized object on Unix.
                return Err(ShmemError::MapOpenFailed(ERROR_INVALID_PARAMETER));
            }
            let (object, view) = map(&file, len).map_err(ShmemError::MapOpenFailed)?;
            Ok(Self {
                view,
                len,
                object,
                _file: file,
                path,
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
            self.view
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
            // SAFETY: the view and the object are this mapping's, unmapped
            // and closed once here.
            unsafe {
                UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.view.cast(),
                });
                CloseHandle(self.object);
            }
            if self.owner {
                let _ = super::remove_name(&self.path);
                if let Some(flink) = &self.flink {
                    let _ = std::fs::remove_file(flink);
                }
            }
        }
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
    fn shm_6_1_a_mapping_holds_no_file_descriptor() {
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

    /// The probe of `shm_6_1_a_mapping_holds_no_file_descriptor`; a no-op unless
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
    fn shm_6_1_create_open_share_and_the_owner_unlinks() {
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

/// The Windows mapping, an unnamed file mapping object over the segment's
/// file in [`backing_dir`]: the same contract as the Unix one above.
#[cfg(all(test, windows))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod windows_tests {
    use super::*;

    fn name(tag: &str) -> String {
        format!("/zdm-{}-{tag}", std::process::id())
    }

    /// Both sides see each other's writes; the flink names the object, the
    /// owner's drop takes its file's name and the flink, a non-owner's drop
    /// neither. A name that is there refuses a second create.
    #[test]
    fn shm_6_2_create_open_share_and_the_owner_unlinks() {
        let dir = tempfile::tempdir().unwrap();
        let flink = dir.path().join("pair.shm");
        let os_id = name("pair");
        let backing = backing_dir().join(os_id.trim_start_matches('/'));
        let made = Mapping::create(&os_id, 4096, Some(&flink)).unwrap();
        assert!(backing.exists(), "{}", backing.display());
        assert!(matches!(
            Mapping::create(&os_id, 4096, None),
            Err(ShmemError::MappingIdExists)
        ));
        let opened = Mapping::open_flink(&flink).unwrap();
        assert_eq!(opened.len(), 4096);
        // SAFETY: both map 4096 bytes of the same file.
        unsafe {
            made.as_ptr().write(0x5A);
            assert_eq!(opened.as_ptr().read(), 0x5A);
            opened.as_ptr().add(4095).write(0xA5);
            assert_eq!(made.as_ptr().add(4095).read(), 0xA5);
        }
        let second = Mapping::open_os_id(&os_id).unwrap();
        drop(second);
        assert!(flink.exists(), "a non-owner's drop leaves the flink");
        assert!(backing.exists(), "and the file");
        drop(made);
        assert!(!flink.exists(), "the owner's drop takes the flink");
        assert!(!backing.exists(), "and the file's name");
        assert!(
            Mapping::open_os_id(&os_id).is_err_and(|e| is_missing(&e)),
            "the name is gone"
        );
        // SAFETY: the opened view stays valid after the owner's drop.
        assert_eq!(unsafe { opened.as_ptr().read() }, 0x5A);
        drop(opened);
    }

    /// A segment made anew under the name of one that another mapping still
    /// maps is a new, zeroed segment, and the old mapping keeps the old
    /// bytes. With a named mapping object, as `shared_memory` made it, the
    /// create mapped the old object again while it was open anywhere: the
    /// next owner of a pair wrote into the ring its predecessor had ended.
    #[test]
    fn shm_6_2_a_segment_made_anew_while_the_old_one_is_mapped_is_new() {
        let os_id = name("anew");
        let old = Mapping::create(&os_id, 4096, None).unwrap();
        let reader = Mapping::open_os_id(&os_id).unwrap();
        // SAFETY: the mapping is 4096 bytes long.
        unsafe { old.as_ptr().write(0x77) };
        drop(old);
        let new = Mapping::create(&os_id, 4096, None).unwrap();
        // SAFETY: both map 4096 bytes.
        unsafe {
            assert_eq!(new.as_ptr().read(), 0, "a new segment");
            assert_eq!(reader.as_ptr().read(), 0x77, "the old one as it was");
            new.as_ptr().write(0x11);
            assert_eq!(reader.as_ptr().read(), 0x77, "two segments");
        }
        let opened = Mapping::open_os_id(&os_id).unwrap();
        // SAFETY: as above.
        assert_eq!(
            unsafe { opened.as_ptr().read() },
            0x11,
            "the name is the new one's"
        );
        drop((reader, opened, new));
    }

    /// A segment's file that is made and not sized yet, as
    /// [`Mapping::create`] leaves it between its `create_new` and its
    /// `set_len`, does not open: `MapOpenFailed(ERROR_INVALID_PARAMETER)`,
    /// as an unsized object reads as `MapOpenFailed(EINVAL)` on Unix
    /// (`tests::an_unsized_object_does_not_open`). Once sized it opens.
    #[test]
    fn shm_6_2_an_unsized_segment_file_does_not_open() {
        use windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER;
        let os_id = name("unsized");
        let path = backing_dir().join(os_id.trim_start_matches('/'));
        std::fs::create_dir_all(backing_dir()).unwrap();
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        assert!(matches!(
            Mapping::open_os_id(&os_id),
            Err(ShmemError::MapOpenFailed(e)) if e == ERROR_INVALID_PARAMETER
        ));
        file.set_len(4096).unwrap();
        let opened = Mapping::open_os_id(&os_id).unwrap();
        assert_eq!(opened.len(), 4096, "sized, it opens");
        drop(opened);
        drop(file);
        remove_name(&path).unwrap();
    }
}
