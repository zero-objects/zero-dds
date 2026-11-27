// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Windows (`zerodds-uds-transport-1.0` §6.2): the base directory and the
//! modes.
//!
//! Windows has no mode bits. A socket file takes the access control list
//! (ACL) of its directory, and a peer connects to it only with write access
//! to the file (Microsoft, "AF_UNIX comes to Windows"). So:
//!
//! - a `base_dir` the bind creates gets a protected discretionary ACL with
//!   full access for the process's user and for `SYSTEM` and nothing else
//!   (`D:P(A;OICI;FA;;;<user SID>)(A;OICI;FA;;;SY)`, the second entry alone
//!   for a process that runs as `SYSTEM`): the directory is private to the
//!   user, as `0700` makes it on Unix (§2.2), and the socket files inherit
//!   that ACL;
//! - an existing `base_dir` keeps its ACL; one that is a reparse point (a
//!   symbolic link or a junction) is refused, as a symlink is on Unix;
//! - `socket_mode` and a `base_dir_mode` other than
//!   [`crate::DEFAULT_BASE_DIR_MODE`] are refused with `Unsupported`: they
//!   name mode bits Windows does not have. A directory several users share
//!   is prepared by the operator with an ACL that grants them write access.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{CreateDirectoryW, FILE_ATTRIBUTE_REPARSE_POINT};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::DEFAULT_BASE_DIR_MODE;

/// Refuses the modes Windows cannot carry out (§6.2).
pub(crate) fn check_modes(socket_mode: Option<u32>, base_dir_mode: u32) -> io::Result<()> {
    if let Some(mode) = socket_mode {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "uds socket_mode {mode:o}: Windows has no mode bits; the socket file takes \
                 the access control list of base_dir"
            ),
        ));
    }
    if base_dir_mode != DEFAULT_BASE_DIR_MODE {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "uds base_dir_mode {base_dir_mode:o}: Windows has no mode bits; a base_dir \
                 the bind creates is private to the user (an access control list of the \
                 user and SYSTEM), a directory shared by several users is prepared by the \
                 operator"
            ),
        ));
    }
    Ok(())
}

/// Makes sure `path` is a directory the transport may bind in, creating
/// it private to the user when it is missing (§2.2, §6.2).
pub(crate) fn ensure_base_dir(path: &Path, _mode: u32) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => return check_existing(&meta),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    match create_private_dir(path) {
        Ok(()) => {}
        // Another process created it between the check and the call: it
        // is checked as an existing directory.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    check_existing(&fs::symlink_metadata(path)?)
}

/// Refuses a reparse point and anything that is not a directory.
fn check_existing(meta: &fs::Metadata) -> io::Result<()> {
    if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "uds base_dir is a reparse point (a symbolic link or a junction) — refusing \
             (TOCTOU-hardening)",
        ));
    }
    if !meta.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "uds base_dir exists but is not a directory",
        ));
    }
    Ok(())
}

/// The SID of `SYSTEM` (`S-1-5-18`; `SY` in SDDL).
const SYSTEM_SID: &str = "S-1-5-18";

/// The SDDL of the ACL a created `base_dir` gets: protected, full access
/// for `user_sid` and `SYSTEM`, inherited by files and directories in it.
/// A process that runs as `SYSTEM` (a service; the Windows CI runner) has
/// one entry: its user is `SYSTEM`, and Windows keeps one of two equal
/// entries (the directory read back as `D:P(A;OICI;FA;;;SY)`, weekly
/// pipeline 9999, job 238934).
pub(crate) fn private_dacl_sddl(user_sid: &str) -> String {
    if user_sid == SYSTEM_SID || user_sid == "SY" {
        return String::from("D:P(A;OICI;FA;;;SY)");
    }
    format!("D:P(A;OICI;FA;;;{user_sid})(A;OICI;FA;;;SY)")
}

/// `s` as a NUL-terminated UTF-16 string.
fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// The UTF-16 string at `p` up to its NUL.
///
/// # Safety
/// `p` points to a NUL-terminated UTF-16 string.
unsafe fn from_wide(p: *const u16) -> String {
    let mut len = 0;
    // SAFETY: the caller guarantees a NUL within the string.
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` units before the NUL are part of the string.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
}

/// A buffer the system allocated with `LocalAlloc`, freed on drop.
struct Local(HLOCAL);

impl Drop for Local {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the pointer came from a call that allocates with
            // LocalAlloc and is freed once, here.
            unsafe { LocalFree(self.0) };
        }
    }
}

/// A handle closed on drop.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenProcessToken and is closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// The SID of the process's user as a string (`S-1-5-21-…`).
pub(crate) fn current_user_sid() -> io::Result<String> {
    let mut token: HANDLE = 0;
    // SAFETY: GetCurrentProcess returns a pseudo handle; `token` receives
    // the opened token.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Handle(token);
    let mut len = 0u32;
    // SAFETY: a null buffer of length 0 asks for the needed length, which
    // the call writes to `len` (it fails with ERROR_INSUFFICIENT_BUFFER).
    unsafe { GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut len) };
    if len == 0 {
        return Err(io::Error::last_os_error());
    }
    // u64 units keep the TOKEN_USER at the start aligned.
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `len` bytes.
    if unsafe { GetTokenInformation(token.0, TokenUser, buf.as_mut_ptr().cast(), len, &mut len) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call wrote a TOKEN_USER at the start of the aligned
    // buffer; its SID points into the same buffer, which outlives its use.
    let sid = unsafe { (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut text: *mut u16 = std::ptr::null_mut();
    // SAFETY: `sid` is a valid SID; the call allocates the string.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let text = Local(text.cast());
    // SAFETY: ConvertSidToStringSidW wrote a NUL-terminated string.
    Ok(unsafe { from_wide(text.0.cast()) })
}

/// Creates the directory `path` with the private ACL of
/// [`private_dacl_sddl`].
fn create_private_dir(path: &Path) -> io::Result<()> {
    let sddl = wide(OsStr::new(&private_dacl_sddl(&current_user_sid()?)));
    let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: `sddl` is NUL-terminated; the call allocates the descriptor.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let sd = Local(sd);
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd.0,
        bInheritHandle: 0,
    };
    let name = wide(path.as_os_str());
    // SAFETY: `name` is NUL-terminated and `attributes` points to a valid
    // descriptor; both live across the call.
    if unsafe { CreateDirectoryW(name.as_ptr(), &attributes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::UdsConfig;
    use crate::stream::UdsStreamTransport;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

    /// The ACL of `path` as SDDL.
    fn dacl_sddl(path: &Path) -> String {
        let name = wide(path.as_os_str());
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: `name` is NUL-terminated; the call allocates `sd`.
        let rc = unsafe {
            GetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut sd,
            )
        };
        assert_eq!(rc, 0, "GetNamedSecurityInfoW");
        let sd = Local(sd);
        let mut text: *mut u16 = std::ptr::null_mut();
        // SAFETY: `sd` is the descriptor read above; the call allocates
        // the string.
        let ok = unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                sd.0,
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut text,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(
            ok, 0,
            "ConvertSecurityDescriptorToStringSecurityDescriptorW"
        );
        let text = Local(text.cast());
        // SAFETY: the call wrote a NUL-terminated string.
        unsafe { from_wide(text.0.cast()) }
    }

    /// §6.2 and §2.2: a `base_dir` the bind creates carries the protected
    /// ACL of the user and SYSTEM, and the bound socket file in it inherits
    /// those two entries.
    #[test]
    fn uds_6_2_a_created_base_dir_is_private_to_the_user() {
        let tmp = tempfile::tempdir().unwrap();
        let config = UdsConfig {
            base_dir: tmp.path().join("uds"),
            ..UdsConfig::default()
        };
        let t = UdsStreamTransport::bind([1; 16], config.clone()).unwrap();
        let user = current_user_sid().unwrap();
        assert!(user.starts_with("S-1-5-"), "{user}");
        // SDDL writes the well-known accounts by their aliases: a runner
        // that runs as SYSTEM (S-1-5-18) reads back `SY`.
        let alias = match user.as_str() {
            "S-1-5-18" => "SY",
            "S-1-5-19" => "LS",
            "S-1-5-20" => "NS",
            sid => sid,
        };
        assert_eq!(dacl_sddl(&config.base_dir), private_dacl_sddl(alias));
        let file = dacl_sddl(t.socket_path());
        assert!(
            file.contains(&format!(";FA;;;{alias})")) && file.contains(";FA;;;SY)"),
            "{file}"
        );
    }

    /// §6.2: `socket_mode` and a `base_dir_mode` other than the default are
    /// refused with `Unsupported`, before anything is created.
    #[test]
    fn uds_6_2_mode_bits_are_refused_on_windows() {
        let tmp = tempfile::tempdir().unwrap();
        for (socket_mode, base_dir_mode) in [(Some(0o660), 0o700), (None, 0o3770)] {
            let config = UdsConfig {
                base_dir: tmp.path().join("uds"),
                socket_mode,
                base_dir_mode,
                ..UdsConfig::default()
            };
            let err = UdsStreamTransport::bind([2; 16], config.clone())
                .err()
                .expect("refused");
            assert_eq!(err.kind(), io::ErrorKind::Unsupported, "{err}");
            assert!(!config.base_dir.exists());
        }
    }

    /// §6.2 and §2.2: a junction as `base_dir` is refused.
    #[test]
    fn uds_6_2_a_junction_as_base_dir_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::create_dir(&target).unwrap();
        let junction = tmp.path().join("uds");
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .unwrap();
        assert!(out.status.success(), "mklink /J: {out:?}");
        let config = UdsConfig {
            base_dir: junction,
            ..UdsConfig::default()
        };
        let err = UdsStreamTransport::bind([3; 16], config)
            .err()
            .expect("refused");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
        assert!(err.to_string().contains("reparse point"), "{err}");
    }

    /// §6.2: on Windows the UDS transport is the stream transport; a
    /// message crosses between two of them under the default settings but
    /// the base directory.
    #[test]
    fn uds_6_2_the_stream_transport_carries_messages_on_windows() {
        use zerodds_transport::Transport;
        let tmp = tempfile::tempdir().unwrap();
        let config = UdsConfig {
            base_dir: tmp.path().join("uds"),
            recv_timeout: Some(std::time::Duration::from_secs(2)),
            ..UdsConfig::default()
        };
        let a = UdsStreamTransport::bind([4; 16], config.clone()).unwrap();
        let b = UdsStreamTransport::bind([5; 16], config).unwrap();
        a.send(&b.local_locator(), b"windows").unwrap();
        let dg = b.recv().unwrap();
        assert_eq!(&dg.data[..], b"windows");
        assert_eq!(dg.source, a.local_locator());
    }
}
