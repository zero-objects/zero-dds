// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport-uds`. Safety classification: **STANDARD**.
//!
//! ZeroDDS Unix domain socket transport — container IPC for ZeroDDS.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5 §9.3.2.4** — the locator kind `LocatorKind::Uds`,
//!   `0x01F00002`, in the range that section reserves for vendor-specific
//!   kinds (`crates/rtps/src/wire_types.rs`); the spec names no UDS kind.
//! - **ZeroDDS-UDS-Transport 1.0** — vendor-specific transport spec
//!   (filesystem path resolution, abstract namespace, SOCK_DGRAM
//!   wire format, the stream mode of §3.5 and Windows of §6.2),
//!   `docs/spec-coverage/zerodds-uds-transport-1.0.md`.
//!
//! ## Note on OMG normativity
//!
//! OMG does not standardize a UDS transport for DDS. Cyclone DDS and
//! Fast DDS have no official UDS transport (they use iceoryx or shared
//! memory). ZeroDDS defines its own variant explicitly as the
//! ZeroDDS-UDS-Transport-1.0 spec.
//!
//! ## Use case
//!
//! Container IPC when multicast is blocked and POSIX SHM is impractical
//! cross-container (UID mapping, `/dev/shm` visibility, SELinux). A
//! mounted volume with UDS sockets is the realistic Docker/Kubernetes
//! pattern.
//!
//! ## Implemented
//!
//! - `SOCK_DGRAM` over filesystem sockets (default mode on Unix).
//! - `SOCK_STREAM` with length-prefixed frames ([`UdsStreamTransport`],
//!   §3.5): the only `AF_UNIX` socket type Windows has, and the UDS
//!   transport there (§6.2); on Unix the alternative to the datagram
//!   transport.
//! - Linux abstract-namespace support via the `abstract_dgram` module
//!   (datagrams without a filesystem inode).
//! - Lazy base-directory creation with a configurable mode (default
//!   `0700`); a symlink or a non-directory is refused.
//! - A configurable mode of the socket file, for a peer under another UID
//!   on a shared volume; the mode is set on the socket the bind created,
//!   never through a symlink.
//! - A receive that reports a datagram above `max_datagram` as an error
//!   instead of a cut message, and a bind that refuses a `max_datagram`
//!   the kernel's socket buffers do not carry.
//! - A send that waits at most `send_timeout` for room in a full receive
//!   queue of the peer and then fails with a named error, instead of
//!   holding the sending thread until the peer reads.
//! - Differentiated `io::Error` classification for diagnostics.
//!
//! ## Cross-vendor interop
//!
//! Not intended — UDS is intra-container/intra-host IPC. Cross-vendor
//! interop with Cyclone/Fast DDS stays in the UDP/TCP/SHM domain.
//!
//! ## Wire format
//!
//! Every datagram is a `SOCK_DGRAM` message; the kernel preserves the
//! message boundaries. In stream mode a connection begins with `ZUDS` and
//! the sender's 16-byte id, then carries each message as a four-octet
//! little-endian length and its octets. Default path resolution: 16-byte
//! `Locator` address → `<base_dir>/<lowercase-hex>.sock`. Default
//! `base_dir` = `/tmp/zerodds/uds` (on Windows the root of the current
//! drive: `\tmp\zerodds\uds`).
//!
//! ## Safety / unsafe scope
//!
//! Three `unsafe` blocks on Unix: `recv_datagram` hands an initialised
//! byte buffer to `socket2` as `[MaybeUninit<u8>]`, which it needs to pass
//! `MSG_TRUNC`; `check_own_socket` reads the effective UID with `geteuid`;
//! outside Linux, `apply_socket_mode` calls `fchmodat` with
//! `AT_SYMLINK_NOFOLLOW`, which `std` does not offer. On Windows the
//! `windows` module calls the Win32 functions that read the process's user
//! SID and create a directory with an access control list. The
//! `abstract_dgram` and `stream` modules build their addresses with
//! `socket2`; the `stream` module's accept thread waits for a connection,
//! and a connection's reader for the rest of the hello, with `poll` (on
//! Windows `WSAPoll`, its error read with
//! `WSAGetLastError`), which `socket2` does not offer, in `unsafe` blocks.

#![warn(missing_docs)]
// AF_UNIX exists on Unix and on Windows (10, build 17063 and later); on
// other targets this crate compiles to an empty library.
#![cfg(any(unix, windows))]

use std::fs;
use std::io;
#[cfg(unix)]
use std::mem::MaybeUninit;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::SendError;
#[cfg(unix)]
use zerodds_transport::{ReceivedDatagram, RecvError, Transport};

pub mod stream;
pub use stream::UdsStreamTransport;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{check_modes, ensure_base_dir};

/// Default base directory for UDS sockets. On Windows the path has no
/// drive and lies on the current drive (`\tmp\zerodds\uds`).
pub const DEFAULT_BASE_DIR: &str = "/tmp/zerodds/uds";

/// Default `max_datagram`: the largest datagram a transport sends and
/// receives. The kernel must grant socket buffers that carry it, which
/// the bind checks ([`UdsConfig::max_datagram`]).
pub const DEFAULT_MAX_DATAGRAM: usize = 65_536;

/// Default mode of a base directory the bind creates: user-private.
pub const DEFAULT_BASE_DIR_MODE: u32 = 0o700;

/// Default `send_timeout`: how long a send waits for room when the receive
/// queue of the peer is full (§4.3). A peer that reads empties its queue
/// within its scheduling latency, a few milliseconds on a loaded host; a
/// peer that has stopped reading holds each send to it this long, then the
/// send fails.
pub const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_millis(100);

/// The bits a socket-file mode may carry (`rwx` for owner, group, other).
#[cfg(unix)]
const SOCKET_MODE_BITS: u32 = 0o777;

/// The bits a directory mode may carry: the permission bits plus
/// set-user-ID, set-group-ID and sticky.
#[cfg(unix)]
const DIR_MODE_BITS: u32 = 0o7777;

/// Configuration for opening a `UdsTransport`.
#[derive(Debug, Clone)]
pub struct UdsConfig {
    /// Base directory for socket files.
    pub base_dir: PathBuf,
    /// The largest datagram this transport sends or receives. A larger
    /// payload is refused on send ([`SendError::PayloadTooLarge`]); a
    /// larger datagram from a peer is reported on receive
    /// ([`RecvError::BufferTooSmall`]), never handed on cut. The bind
    /// fails when the kernel does not grant socket buffers that carry it.
    pub max_datagram: usize,
    /// Optional recv timeout; `None` → block indefinitely.
    pub recv_timeout: Option<Duration>,
    /// How long a send waits for room when the peer's receive queue (Linux:
    /// `net.unix.max_dgram_qlen` datagrams) or this socket's send buffer
    /// is full. After it the send fails with "uds: kernel send buffer full
    /// (EAGAIN)" instead of holding the sending thread until the peer
    /// reads; [`Duration::ZERO`] does not wait. Default
    /// [`DEFAULT_SEND_TIMEOUT`]. macOS does not queue a datagram for a
    /// full receive buffer and fails the send at once with `ENOBUFS`.
    pub send_timeout: Duration,
    /// Mode of the socket file, set right after the bind. `None` keeps
    /// the mode the process umask gives. A peer may send to the socket
    /// only with write permission on the file, so a peer under another
    /// UID needs a mode with group or other write bits (`0o660` with a
    /// shared group). At most `0o777`.
    pub socket_mode: Option<u32>,
    /// Mode of `base_dir` when the bind creates it. An existing directory
    /// keeps its mode. Default [`DEFAULT_BASE_DIR_MODE`] (`0o700`); a
    /// directory shared by several UIDs needs group bits, `0o3770` with a
    /// shared group: the set-group-ID bit gives new socket files the
    /// directory's group, the sticky bit keeps one UID from removing or
    /// replacing the socket file of another. At most `0o7777`. A mode the
    /// kernel does not set as asked (it drops the set-group-ID bit when the
    /// directory's group is none of the process's groups) fails the bind.
    pub base_dir_mode: u32,
}

impl Default for UdsConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from(DEFAULT_BASE_DIR),
            max_datagram: DEFAULT_MAX_DATAGRAM,
            recv_timeout: None,
            send_timeout: DEFAULT_SEND_TIMEOUT,
            socket_mode: None,
            base_dir_mode: DEFAULT_BASE_DIR_MODE,
        }
    }
}

/// Renders a locator ID (16 bytes) to a filesystem path under
/// `base_dir`.
#[must_use]
pub fn socket_path(base_dir: &Path, id: [u8; 16]) -> PathBuf {
    // Windows: the directory with the platform's separators
    // (`\tmp\zerodds\uds` for `/tmp/zerodds/uds`), as the system writes a
    // path; `socket2` hands the path to `AF_UNIX` as it is.
    #[cfg(windows)]
    let mut p: PathBuf = base_dir.components().collect();
    #[cfg(not(windows))]
    let mut p = base_dir.to_path_buf();
    p.push(format!("{}.sock", hex_id(id)));
    p
}

/// The 16-byte id as 32 lowercase hex digits without separators.
pub(crate) fn hex_id(id: [u8; 16]) -> String {
    let mut hex = String::with_capacity(32);
    for byte in id {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// UDS-based transport bound to a specific 16-byte locator ID, over
/// `SOCK_DGRAM` (Unix only; Windows has [`UdsStreamTransport`]).
#[cfg(unix)]
pub struct UdsTransport {
    socket: UnixDatagram,
    local_id: [u8; 16],
    config: UdsConfig,
}

#[cfg(unix)]
impl UdsTransport {
    /// Bind a new transport at `local_id`. The socket file is created
    /// under `config.base_dir`; if a stale socket file exists it is
    /// removed first (typical Unix practice after a crash).
    ///
    /// # Errors
    /// - `InvalidInput` for a mode outside the permitted bits, a
    ///   `base_dir` that is a symlink or not a directory, a socket path
    ///   that is no longer the socket this process bound when its mode is
    ///   set, or a `max_datagram` the kernel's socket buffers do not carry.
    /// - `PermissionDenied` for a created `base_dir` whose mode the kernel
    ///   did not set as `base_dir_mode` asks.
    /// - I/O error creating `base_dir`, binding the socket or setting
    ///   the mode of the socket file.
    pub fn bind(local_id: [u8; 16], config: UdsConfig) -> io::Result<Self> {
        check_modes(config.socket_mode, config.base_dir_mode)?;
        ensure_base_dir(&config.base_dir, config.base_dir_mode)?;
        let path = socket_path(&config.base_dir, local_id);
        // Remove stale socket from a prior crashed instance. UnixDatagram
        // cannot bind on top of an existing file; Linux returns EADDRINUSE.
        remove_stale_socket(&path)?;
        let socket = UnixDatagram::bind(&path)?;
        // From here on a failure drops the transport, whose Drop removes
        // the socket file again.
        let transport = Self {
            socket,
            local_id,
            config,
        };
        apply_socket_mode(&path, transport.config.socket_mode)?;
        if let Some(t) = transport.config.recv_timeout {
            transport.socket.set_read_timeout(Some(t))?;
        }
        set_send_timeout(
            socket2::SockRef::from(&transport.socket),
            transport.config.send_timeout,
        )?;
        size_buffers(
            socket2::SockRef::from(&transport.socket),
            transport.config.max_datagram,
        )?;
        Ok(transport)
    }
}

#[cfg(unix)]
impl Drop for UdsTransport {
    fn drop(&mut self) {
        // Best-effort cleanup: remove our socket file so the next bind
        // with the same id does not hit a stale file. Ignore errors —
        // a detached Docker container might have the volume
        // already unmounted.
        let path = socket_path(&self.config.base_dir, self.local_id);
        let _ = fs::remove_file(path);
    }
}

/// Refuses a socket mode above `0o777` and a directory mode above
/// `0o7777`: a bit outside them is no permission and would be passed to
/// `chmod` as something else.
#[cfg(unix)]
pub(crate) fn check_modes(socket_mode: Option<u32>, base_dir_mode: u32) -> io::Result<()> {
    if let Some(mode) = socket_mode.filter(|mode| mode & !SOCKET_MODE_BITS != 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("uds socket_mode {mode:o} has bits outside 0777"),
        ));
    }
    if base_dir_mode & !DIR_MODE_BITS != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("uds base_dir_mode {base_dir_mode:o} has bits outside 07777"),
        ));
    }
    Ok(())
}

/// Removes what a crashed predecessor left at `path`. `symlink_metadata`
/// sees a dangling symlink as well, which `Path::exists` would not.
pub(crate) fn remove_stale_socket(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Sets the mode of a freshly bound socket file. Until this call the
/// file carries the mode of the process umask; the base directory's
/// permission is what keeps other UIDs out in that interval.
///
/// The mode goes only to the socket this process bound, never through a
/// symlink: in a directory another UID may write to, that UID could have
/// replaced the socket file by a symlink to a file of this process's UID,
/// which a plain `chmod` would follow (§5.1; the sticky bit of the
/// recommended `3770` keeps it from removing the file at all). On Linux
/// the path is opened with `O_PATH | O_NOFOLLOW`, the descriptor is
/// checked to be a socket owned by the effective UID, and the mode is set
/// on exactly that inode through `/proc/self/fd`. Elsewhere the path is
/// checked the same way with `lstat` and the mode set with `fchmodat`
/// and `AT_SYMLINK_NOFOLLOW`; between the check and the call another UID
/// with write permission on a directory without the sticky bit could
/// still put a hard link there.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn apply_socket_mode(path: &Path, mode: Option<u32>) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::AsRawFd;
    let Some(mode) = mode else {
        return Ok(());
    };
    // `O_PATH` opens the inode without opening the socket; with
    // `O_NOFOLLOW` a symlink yields a descriptor of the link itself.
    let inode = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_PATH | libc::O_NOFOLLOW)
        .open(path)?;
    check_own_socket(path, &inode.metadata()?)?;
    let by_descriptor = format!("/proc/self/fd/{}", inode.as_raw_fd());
    fs::set_permissions(&by_descriptor, fs::Permissions::from_mode(mode)).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "uds socket_mode: {by_descriptor} is missing (no /proc), so the mode of {} \
                     cannot be set without following a symlink",
                    path.display()
                ),
            )
        } else {
            e
        }
    })
}

/// See the Linux variant above.
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
pub(crate) fn apply_socket_mode(path: &Path, mode: Option<u32>) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let Some(mode) = mode else {
        return Ok(());
    };
    check_own_socket(path, &fs::symlink_metadata(path)?)?;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "uds socket path holds a NUL"))?;
    // `check_modes` admitted at most 0o777, which every `mode_t` holds.
    let mode = mode as libc::mode_t;
    // SAFETY: `c_path` is a NUL-terminated string that lives across the
    // call; `fchmodat` reads it and keeps no pointer.
    let rc = unsafe {
        libc::fchmodat(
            libc::AT_FDCWD,
            c_path.as_ptr(),
            mode,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    check_own_socket(path, &fs::symlink_metadata(path)?)
}

/// Refuses an inode at `path` that is not a socket owned by the effective
/// UID: then it is not the socket this process bound.
#[cfg(unix)]
fn check_own_socket(path: &Path, meta: &fs::Metadata) -> io::Result<()> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    // SAFETY: geteuid has no preconditions and cannot fail.
    let euid = unsafe { libc::geteuid() };
    if meta.file_type().is_socket() && meta.uid() == euid {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "uds socket_mode: {} is not the socket this process bound (a socket owned by \
             uid {euid}) — refusing to set its mode",
            path.display()
        ),
    ))
}

/// Bounds how long a send waits for room in a full receive queue of the
/// peer or a full send buffer (§4.3). `SO_SNDTIMEO` of zero would mean no
/// bound, so a zero `timeout` sets nothing and [`send_flags`] makes each
/// send non-blocking instead; a timeout below the microsecond that
/// `SO_SNDTIMEO` resolves is raised to one.
#[cfg(unix)]
pub(crate) fn set_send_timeout(sock: socket2::SockRef<'_>, timeout: Duration) -> io::Result<()> {
    if timeout.is_zero() {
        return Ok(());
    }
    sock.set_write_timeout(Some(timeout.max(Duration::from_micros(1))))
}

/// The flags of a send: `MSG_DONTWAIT` when `send_timeout` is zero, and on
/// Linux `MSG_NOSIGNAL`, as `std` sends there.
#[cfg(unix)]
pub(crate) fn send_flags(send_timeout: Duration) -> libc::c_int {
    let wait = if send_timeout.is_zero() {
        libc::MSG_DONTWAIT
    } else {
        0
    };
    wait | NO_SIGPIPE_FLAG
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const NO_SIGPIPE_FLAG: libc::c_int = libc::MSG_NOSIGNAL;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
const NO_SIGPIPE_FLAG: libc::c_int = 0;

/// What a unix datagram costs in the sender's send buffer beyond its
/// payload. Linux refuses a datagram with `EMSGSIZE` when it is larger
/// than `SO_SNDBUF - 32` (`unix_dgram_sendmsg`); macOS when it is larger
/// than `SO_SNDBUF`.
#[cfg(any(target_os = "linux", target_os = "android"))]
const SEND_BUFFER_OVERHEAD: usize = 32;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
const SEND_BUFFER_OVERHEAD: usize = 0;

/// The sysctl that caps a socket-buffer request, for the bind's error.
#[cfg(any(target_os = "linux", target_os = "android"))]
const BUFFER_CAP_SYSCTL: &str = "net.core.wmem_max";
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
const BUFFER_CAP_SYSCTL: &str = "kern.ipc.maxsockbuf";

/// Sizes the socket buffers for datagrams of `max_datagram` bytes and
/// checks that the kernel granted enough.
///
/// The kernel refuses a unix datagram larger than the sender's send
/// buffer. macOS starts that buffer at `net.local.dgram.maxdgram` (2048)
/// and the receive buffer at `net.local.dgram.recvspace` (4096), so a
/// datagram above 2 KiB failed with `EMSGSIZE` there and the receiver
/// held about one at a time. Linux starts at `wmem_default`/`rmem_default`
/// and caps a request at `wmem_max`/`rmem_max` without an error, so the
/// granted sizes are read back: a `max_datagram` the send buffer cannot
/// carry is an error here, at bind, and not an `EMSGSIZE` of the first
/// large sample. On macOS the receiver's buffer must hold the datagram
/// as well (the sender gets `ENOBUFS` otherwise); Linux queues unix
/// datagrams by count, not by the receive buffer.
#[cfg(unix)]
pub(crate) fn size_buffers(sock: socket2::SockRef<'_>, max_datagram: usize) -> io::Result<()> {
    // setsockopt takes an `int`.
    let cap = i32::MAX as usize;
    let needed_send = max_datagram.saturating_add(SEND_BUFFER_OVERHEAD);
    if sock.send_buffer_size()? < max_datagram.saturating_mul(2)
        && sock
            .set_send_buffer_size(max_datagram.saturating_mul(2).min(cap))
            .is_err()
    {
        // macOS refuses a request above `kern.ipc.maxsockbuf` with
        // ENOBUFS, where the datagram itself may still fit. What was
        // granted is checked below either way.
        sock.set_send_buffer_size(needed_send.min(cap)).ok();
    }
    if sock.recv_buffer_size()? < max_datagram.saturating_mul(8)
        && sock
            .set_recv_buffer_size(max_datagram.saturating_mul(8).min(cap))
            .is_err()
    {
        sock.set_recv_buffer_size(max_datagram.min(cap)).ok();
    }
    let refused = |buffer: &str, granted: usize, needed: usize| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "uds max_datagram {max_datagram} exceeds what the kernel grants: \
                 {buffer} {granted}, needed {needed} (capped by {BUFFER_CAP_SYSCTL})"
            ),
        )
    };
    let granted_send = sock.send_buffer_size()?;
    if granted_send < needed_send {
        return Err(refused("SO_SNDBUF", granted_send, needed_send));
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let granted_recv = sock.recv_buffer_size()?;
        if granted_recv < max_datagram {
            return Err(refused("SO_RCVBUF", granted_recv, max_datagram));
        }
    }
    Ok(())
}

/// The receive flag that makes `recvfrom` return the real length of a
/// datagram larger than the buffer. Linux has it; elsewhere the length
/// is cut to the buffer, which is one byte larger than `max_datagram`,
/// so a cut datagram is still recognised.
#[cfg(any(target_os = "linux", target_os = "android"))]
const RECV_TRUNC_FLAG: libc::c_int = libc::MSG_TRUNC;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
const RECV_TRUNC_FLAG: libc::c_int = 0;

/// Receives one datagram of at most `max_datagram` bytes.
///
/// The buffer has one byte more than `max_datagram`: a datagram that
/// fills it is larger than the limit and is reported as
/// [`RecvError::BufferTooSmall`] (its real size on Linux, one byte more
/// than `max_datagram` elsewhere) instead of being handed on cut, which
/// RTPS could not tell from a short message.
#[cfg(unix)]
pub(crate) fn recv_datagram(
    sock: socket2::SockRef<'_>,
    max_datagram: usize,
    io_message: &'static str,
) -> Result<(Vec<u8>, socket2::SockAddr), RecvError> {
    let mut buf = vec![0u8; max_datagram.saturating_add(1)];
    let raw = std::ptr::from_mut::<[u8]>(buf.as_mut_slice()) as *mut [MaybeUninit<u8>];
    // SAFETY: `raw` is the whole of `buf`, a live allocation that nothing
    // else touches until `uninit` is last used. `u8` and `MaybeUninit<u8>`
    // have the same layout, and `recvfrom` writes only initialised bytes
    // into the buffer, so every byte of `buf` stays initialised.
    let uninit = unsafe { &mut *raw };
    match sock.recv_from_with_flags(uninit, RECV_TRUNC_FLAG) {
        Ok((len, _)) if len > max_datagram => Err(RecvError::BufferTooSmall {
            datagram_size: len,
            buffer_size: max_datagram,
        }),
        Ok((len, addr)) => {
            buf.truncate(len);
            Ok((buf, addr))
        }
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) =>
        {
            Err(RecvError::Timeout)
        }
        Err(_) => Err(RecvError::Io {
            message: io_message,
        }),
    }
}

/// Makes sure `path` is a directory the transport may bind in, creating
/// it with `mode` when it is missing (§2.2).
#[cfg(unix)]
pub(crate) fn ensure_base_dir(path: &Path, mode: u32) -> io::Result<()> {
    // TOCTOU fix: `path.exists()` followed by `set_permissions` is racy
    // — an attacker could place a symlink to a foreign path between the
    // check and the set, tricking us into chmod'ing onto /etc or /root.
    //
    // Fix: `symlink_metadata` (does not follow links) + hard-reject if
    // the path exists AND is not a regular directory. If the path does
    // not exist, `create_dir_all` + `set_permissions` on the
    // freshly-created path (whose ownership we know, because we just
    // created it).
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "uds base_dir is a symlink — refusing (TOCTOU-hardening)",
                ));
            }
            if !meta.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "uds base_dir exists but is not a directory",
                ));
            }
            // Exists + directory + no symlink → OK, nothing to do. A
            // directory we did not create keeps its mode.
            return Ok(());
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // Path doesn't exist → create it under our ownership.
        }
        Err(e) => return Err(e),
    }
    fs::create_dir_all(path)?;
    // Re-check: the path exists now, but run symlink_metadata once more
    // to avoid a race between create_dir_all and chmod (someone could
    // rename+symlink between the calls).
    let meta_after = fs::symlink_metadata(path)?;
    if meta_after.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "uds base_dir became a symlink between create and chmod",
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    // The kernel drops the set-group-ID bit without an error when the
    // directory's group is none of the process's groups (a directory that
    // took the group of a set-group-ID parent; on macOS a new directory
    // takes the group of every parent), and macOS drops it for every
    // process in a sandbox (`sandbox-exec`, the operation
    // `file-write-setugid`). On Linux socket files would then take the
    // binder's group and a peer of the shared group would be refused on
    // its first send. The bind fails here instead, and removes the
    // directory, so that the next bind does not take it as existing.
    let granted = fs::symlink_metadata(path)?.permissions().mode() & DIR_MODE_BITS;
    if granted != mode {
        let removed = match fs::remove_dir(path) {
            Ok(()) => "removed it again".to_string(),
            Err(e) => format!("could not remove it again: {e}"),
        };
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "uds base_dir_mode {mode:o}: the kernel set {granted:o} on the created \
                 directory {} (a set-group-ID bit needs the directory's group among the \
                 process's groups, and macOS drops it for a process in a sandbox); {removed}",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(unix)]
impl Transport for UdsTransport {
    /// The datagram limit of the socket (`max_datagram`).
    fn max_message_size(&self) -> usize {
        self.config.max_datagram
    }

    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if dest.kind != LocatorKind::Uds {
            return Err(SendError::UnsupportedLocator);
        }
        if data.len() > self.config.max_datagram {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: self.config.max_datagram,
            });
        }
        let path = socket_path(&self.config.base_dir, dest.address);
        let addr = socket2::SockAddr::unix(&path).map_err(|e| classify_send_error(&e))?;
        // A full receive queue of the peer waits at most `send_timeout`
        // (`SO_SNDTIMEO`, or `MSG_DONTWAIT` for zero) and then fails with
        // `EAGAIN` (§4.3).
        match socket2::SockRef::from(&self.socket).send_to_with_flags(
            data,
            &addr,
            send_flags(self.config.send_timeout),
        ) {
            Ok(_) => Ok(()),
            Err(e) => Err(classify_send_error(&e)),
        }
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        let (buf, addr) = recv_datagram(
            socket2::SockRef::from(&self.socket),
            self.config.max_datagram,
            "uds recv failed",
        )?;
        let source = source_locator(addr.as_pathname(), &self.config.base_dir);
        let data: std::sync::Arc<[u8]> = std::sync::Arc::from(buf.into_boxed_slice());
        Ok(ReceivedDatagram {
            source,
            data: data.into(),
        })
    }

    fn local_locator(&self) -> Locator {
        Locator::uds(self.local_id)
    }
}

/// Differentiated `io::Error` classification.
/// Previously all errors collapsed onto a generic `SendError::Io`, so
/// that e.g. "peer not yet bound" was indistinguishable from
/// "permission denied" or "kernel buffer full".
pub(crate) fn classify_send_error(e: &io::Error) -> SendError {
    match e.kind() {
        io::ErrorKind::NotFound => SendError::Io {
            message: "uds: destination socket missing",
        },
        io::ErrorKind::PermissionDenied => SendError::Io {
            message: "uds: permission denied on destination",
        },
        // Windows reports an expired `SO_SNDTIMEO` as `WSAETIMEDOUT`.
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => SendError::Io {
            message: "uds: kernel send buffer full (EAGAIN)",
        },
        io::ErrorKind::ConnectionRefused => SendError::Io {
            message: "uds: peer socket exists but refused (ECONNREFUSED)",
        },
        // Stream mode (§3.5): the peer closed the connection and a new one
        // did not reach it either.
        io::ErrorKind::BrokenPipe
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted => SendError::Io {
            message: "uds: the peer closed the connection (EPIPE)",
        },
        // EMSGSIZE has no named `io::ErrorKind` on every platform, so it
        // used to land in the catch-all and read as a transient failure.
        // It is not: the kernel's per-message cap for unix datagrams can
        // sit far below `DEFAULT_MAX_DATAGRAM` (macOS
        // `net.local.dgram.maxdgram` defaults to 2048), and the caller
        // must be able to tell that apart from a full buffer.
        #[cfg(unix)]
        _ if e.raw_os_error() == Some(libc::EMSGSIZE) => SendError::Io {
            message: "uds: datagram exceeds the kernel per-message limit (EMSGSIZE)",
        },
        // No buffer space for the datagram: a full receive buffer of the
        // peer on macOS, a failed allocation on Linux.
        #[cfg(unix)]
        _ if e.raw_os_error() == Some(libc::ENOBUFS) => SendError::Io {
            message: "uds: no kernel buffer space for the datagram (ENOBUFS)",
        },
        #[cfg(windows)]
        _ if e.raw_os_error() == Some(windows_sys::Win32::Networking::WinSock::WSAENOBUFS) => {
            SendError::Io {
                message: "uds: no kernel buffer space for the datagram (ENOBUFS)",
            }
        }
        _ => SendError::Io {
            message: "uds: send failed",
        },
    }
}

/// The locator of the sender whose socket is bound at `path`.
///
/// Only a name the transport itself gives (`<base_dir>/<lowercase
/// hex32>.sock`, §2.1) resolves: a reply goes to `socket_path(base_dir,
/// id)`, so any other name would yield a locator that does not reach the
/// sender. An unnamed sender (an anonymous client that did not bind) and
/// every other name give `Locator::INVALID` — higher layers must be
/// prepared for that because RTPS cannot route responses without a peer
/// locator.
#[cfg(unix)]
pub(crate) fn source_locator(path: Option<&Path>, base_dir: &Path) -> Locator {
    let Some(path) = path else {
        return Locator::INVALID;
    };
    if path.parent() != Some(base_dir) {
        return Locator::INVALID;
    }
    if path.extension().and_then(|e| e.to_str()) != Some("sock") {
        return Locator::INVALID;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return Locator::INVALID;
    };
    parse_hex_id(stem).map_or(Locator::INVALID, Locator::uds)
}

/// Parses 32 lowercase hex digits into a 16-byte id, the inverse of
/// [`hex_id`].
#[cfg(unix)]
pub(crate) fn parse_hex_id(s: &str) -> Result<[u8; 16], ()> {
    if s.len() != 32 {
        return Err(());
    }
    let mut out = [0u8; 16];
    for (i, chunk) in s.as_bytes().chunks_exact(2).enumerate() {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

/// A lowercase hex digit. An uppercase one is no name `socket_path`
/// gives, so it does not resolve.
#[cfg(unix)]
fn hex_nibble(b: u8) -> Result<u8, ()> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(()),
    }
}

#[cfg(all(test, unix))]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn id_for(n: u8) -> [u8; 16] {
        let mut a = [0u8; 16];
        a[15] = n;
        a
    }

    fn cfg_with_tmp(dir: &Path) -> UdsConfig {
        UdsConfig {
            base_dir: dir.to_path_buf(),
            max_datagram: 8192,
            recv_timeout: Some(Duration::from_millis(500)),
            ..UdsConfig::default()
        }
    }

    fn mode_of(path: &Path) -> u32 {
        fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
    }

    /// A temporary directory under `/tmp`, in the process's group.
    ///
    /// `sun_path` holds 104 bytes on macOS, and the per-user `TMPDIR` there
    /// (`/var/folders/<2>/<30>/T/`, 49 bytes) leaves a socket in a
    /// subdirectory of a temporary directory at 104 bytes, which the bind
    /// refuses; under `/tmp` the longest path of these tests,
    /// `/tmp/.tmpXXXXXX/volume/<hex32>.sock`, has 59. A new directory on
    /// macOS takes the group of its parent, `wheel` for `/tmp`, and a
    /// set-group-ID bit needs a group of the process, so the directory is
    /// given the process's group.
    fn short_tempdir() -> tempfile::TempDir {
        let dir = tempfile::Builder::new().tempdir_in("/tmp").unwrap();
        // SAFETY: getegid has no preconditions and cannot fail.
        let egid = unsafe { libc::getegid() };
        std::os::unix::fs::chown(dir.path(), None, Some(egid)).unwrap();
        dir
    }

    /// `net.unix.max_dgram_qlen` of this network namespace: a Linux unix
    /// socket queues at most one datagram more before a send to it waits.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn max_dgram_qlen() -> usize {
        fs::read_to_string("/proc/sys/net/unix/max_dgram_qlen")
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    }

    /// Sends datagrams of `tx` to `dest` without waiting until the kernel
    /// refuses one with `EAGAIN`, and returns how many it took: the
    /// datagrams `datagram(0)`, `datagram(1)`, … that are now queued.
    ///
    /// Linux takes a datagram while the peer's queue holds at most
    /// `net.unix.max_dgram_qlen` and the send buffer of `tx` has room; each
    /// queued datagram charges its buffer size to the sender's `SO_SNDBUF`
    /// until the peer reads it. Which is full first depends on the host: in
    /// a network namespace of its own (`max_dgram_qlen` 10) the queue, in
    /// the CI container (`max_dgram_qlen` 512, `SO_SNDBUF` 212 992) the send
    /// buffer, after 278 short datagrams.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn fill_until_refused(
        tx: &UdsTransport,
        dest: [u8; 16],
        datagram: impl Fn(usize) -> Vec<u8>,
    ) -> usize {
        let addr = socket2::SockAddr::unix(socket_path(&tx.config.base_dir, dest)).unwrap();
        let socket = socket2::SockRef::from(&tx.socket);
        let mut queued = 0;
        loop {
            match socket.send_to_with_flags(&datagram(queued), &addr, libc::MSG_DONTWAIT) {
                Ok(_) => queued += 1,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return queued,
                Err(e) => panic!("fill: {e}"),
            }
            assert!(queued <= max_dgram_qlen() + 1, "the kernel never refused");
        }
    }

    #[test]
    fn uds_2_1_socket_path_is_hex_under_base_dir() {
        let mut id = [0u8; 16];
        id[0] = 0xDE;
        id[1] = 0xAD;
        id[15] = 0xFF;
        let p = socket_path(Path::new("/tmp/xyz"), id);
        let s = p.to_string_lossy();
        assert!(s.starts_with("/tmp/xyz/dead"));
        assert!(s.ends_with("ff.sock"));
    }

    #[test]
    fn uds_2_1_parse_hex_id_roundtrip() {
        let id = [0x11u8; 16];
        let mut hex = String::new();
        for b in id {
            use std::fmt::Write;
            let _ = write!(hex, "{b:02x}");
        }
        assert_eq!(parse_hex_id(&hex), Ok(id));
    }

    #[test]
    fn uds_2_1_parse_hex_id_rejects_wrong_length() {
        assert_eq!(parse_hex_id("abc"), Err(()));
    }

    #[test]
    fn uds_2_1_parse_hex_id_rejects_non_hex() {
        assert_eq!(parse_hex_id(&"zz".repeat(16)), Err(()));
    }

    #[test]
    fn uds_4_1_bind_creates_socket_file() {
        let tmp = short_tempdir();
        let t = UdsTransport::bind(id_for(1), cfg_with_tmp(tmp.path())).unwrap();
        let expected = socket_path(tmp.path(), id_for(1));
        assert!(expected.exists(), "socket file should exist: {expected:?}");
        drop(t);
        assert!(
            !expected.exists(),
            "Drop should clean up socket file at {expected:?}"
        );
    }

    #[test]
    fn uds_4_1_bind_reuses_path_after_stale_leftover() {
        let tmp = short_tempdir();
        let path = socket_path(tmp.path(), id_for(2));
        fs::create_dir_all(tmp.path()).unwrap();
        // Simulate a leftover socket file from a crashed process.
        fs::write(&path, b"stale").unwrap();
        let _t = UdsTransport::bind(id_for(2), cfg_with_tmp(tmp.path()))
            .expect("bind must remove stale file and succeed");
        assert!(path.exists());
    }

    #[test]
    fn uds_3_1_send_and_recv_roundtrip_same_process() {
        let tmp = short_tempdir();
        let rx = UdsTransport::bind(id_for(10), cfg_with_tmp(tmp.path())).unwrap();
        let tx = UdsTransport::bind(id_for(11), cfg_with_tmp(tmp.path())).unwrap();

        tx.send(&Locator::uds(id_for(10)), b"hello zerodds")
            .unwrap();

        let got = rx.recv().expect("recv");
        assert_eq!(&got.data[..], b"hello zerodds");
        assert_eq!(got.source, Locator::uds(id_for(11)));
    }

    #[test]
    fn uds_1_1_send_rejects_non_uds_locator() {
        let tmp = short_tempdir();
        let tx = UdsTransport::bind(id_for(20), cfg_with_tmp(tmp.path())).unwrap();
        let res = tx.send(&Locator::udp_v4([127, 0, 0, 1], 7400), b"x");
        assert_eq!(res, Err(SendError::UnsupportedLocator));
    }

    #[test]
    fn uds_3_3_send_rejects_oversize_payload() {
        let tmp = short_tempdir();
        let tx = UdsTransport::bind(id_for(30), cfg_with_tmp(tmp.path())).unwrap();
        let big = vec![0u8; 10_000]; // > 8192 cfg limit
        let res = tx.send(&Locator::uds(id_for(31)), &big);
        assert!(matches!(res, Err(SendError::PayloadTooLarge { .. })));
    }

    #[test]
    fn uds_4_3_send_to_missing_peer_names_the_missing_socket() {
        let tmp = short_tempdir();
        let tx = UdsTransport::bind(id_for(40), cfg_with_tmp(tmp.path())).unwrap();
        let res = tx.send(&Locator::uds(id_for(99)), b"nobody home");
        assert_eq!(
            res,
            Err(SendError::Io {
                message: "uds: destination socket missing"
            })
        );
    }

    /// §4.3: a socket file whose socket is gone (a crashed peer that left
    /// its file) refuses the datagram with `ECONNREFUSED`, which is named
    /// apart from a missing file.
    #[test]
    fn uds_4_3_send_to_a_leftover_socket_file_names_the_refusal() {
        let tmp = short_tempdir();
        let tx = UdsTransport::bind(id_for(41), cfg_with_tmp(tmp.path())).unwrap();
        let leftover = socket_path(tmp.path(), id_for(42));
        drop(UnixDatagram::bind(&leftover).unwrap());
        assert!(leftover.exists(), "the file outlives its socket");
        let res = tx.send(&Locator::uds(id_for(42)), b"anyone?");
        assert_eq!(
            res,
            Err(SendError::Io {
                message: "uds: peer socket exists but refused (ECONNREFUSED)"
            })
        );
    }

    /// §4.3: `EMSGSIZE` and `ENOBUFS` have no `io::ErrorKind` of their own
    /// on every platform; each still gets a message that names it.
    #[test]
    fn uds_4_3_classify_send_error_names_emsgsize_and_enobufs() {
        for (errno, expected) in [
            (
                libc::EMSGSIZE,
                "uds: datagram exceeds the kernel per-message limit (EMSGSIZE)",
            ),
            (
                libc::ENOBUFS,
                "uds: no kernel buffer space for the datagram (ENOBUFS)",
            ),
            (libc::EAGAIN, "uds: kernel send buffer full (EAGAIN)"),
            (libc::EACCES, "uds: permission denied on destination"),
            (libc::ENOENT, "uds: destination socket missing"),
            (
                libc::ECONNREFUSED,
                "uds: peer socket exists but refused (ECONNREFUSED)",
            ),
        ] {
            assert_eq!(
                classify_send_error(&io::Error::from_raw_os_error(errno)),
                SendError::Io { message: expected },
                "errno {errno}"
            );
        }
    }

    #[test]
    fn uds_4_4_recv_times_out_when_idle() {
        let tmp = short_tempdir();
        let rx = UdsTransport::bind(id_for(50), cfg_with_tmp(tmp.path())).unwrap();
        let res = rx.recv();
        assert_eq!(res, Err(RecvError::Timeout));
    }

    #[test]
    fn uds_1_1_local_locator_reflects_bind_id() {
        let tmp = short_tempdir();
        let t = UdsTransport::bind(id_for(60), cfg_with_tmp(tmp.path())).unwrap();
        assert_eq!(t.local_locator(), Locator::uds(id_for(60)));
    }

    // ---- Coverage-Boost ----

    #[test]
    fn uds_4_3_classify_send_error_maps_kinds() {
        use std::io;
        let cases = [
            (io::ErrorKind::NotFound, "destination socket missing"),
            (io::ErrorKind::PermissionDenied, "permission denied"),
            (io::ErrorKind::WouldBlock, "kernel send buffer full"),
            (
                io::ErrorKind::ConnectionRefused,
                "peer socket exists but refused",
            ),
            (io::ErrorKind::Other, "send failed"),
        ];
        for (kind, expected_substr) in cases {
            let e = io::Error::new(kind, "synthetic");
            match classify_send_error(&e) {
                SendError::Io { message } => {
                    assert!(
                        message.contains(expected_substr),
                        "kind {kind:?}: got {message:?}, want substring {expected_substr:?}",
                    );
                }
                other => panic!("expected Io, got {other:?}"),
            }
        }
    }

    #[test]
    fn uds_2_2_ensure_base_dir_rejects_symlink() {
        let tmp = short_tempdir();
        let real = tmp.path().join("real");
        fs::create_dir(&real).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let err =
            ensure_base_dir(&link, DEFAULT_BASE_DIR_MODE).expect_err("symlink must be rejected");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn uds_2_2_ensure_base_dir_rejects_file_path() {
        let tmp = short_tempdir();
        let file = tmp.path().join("not-a-dir");
        fs::write(&file, b"").unwrap();
        let err = ensure_base_dir(&file, DEFAULT_BASE_DIR_MODE)
            .expect_err("regular file must be rejected");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn uds_2_2_ensure_base_dir_creates_missing_dir() {
        let tmp = short_tempdir();
        let missing = tmp.path().join("created");
        ensure_base_dir(&missing, DEFAULT_BASE_DIR_MODE).unwrap();
        assert!(missing.is_dir());
    }

    /// DDSI-RTPS 2.5 §9.3.2.4 reserves `0x00000003` to `0x01ffffff` for
    /// vendor-specific kinds.
    /// ZeroDDS takes `0x01F0_0002` for UDS; the transport binds that
    /// value, so a renumbering has to break here.
    #[test]
    fn rtps_9_3_2_4_locator_kind_is_the_vendor_reserved_uds_value() {
        let loc = Locator::uds(id_for(90));
        assert_eq!(loc.kind.as_i32(), 0x01F0_0002);
        assert!(
            (0x0000_0003..=0x01ff_ffff).contains(&loc.kind.as_i32()),
            "inside the vendor-specific range of §9.3.2.4"
        );
        assert_eq!(loc.port, 0, "the port field is unused for UDS");
        assert_eq!(loc.address, id_for(90));
    }

    /// §2.2: a base directory we create ourselves is user-private —
    /// mode `0700`, no group or other bits.
    #[test]
    fn uds_2_2_ensure_base_dir_creates_dir_with_mode_0700() {
        let tmp = short_tempdir();
        let created = tmp.path().join("private");
        ensure_base_dir(&created, DEFAULT_BASE_DIR_MODE).unwrap();
        let mode = fs::metadata(&created).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "base dir must be user-private, got {mode:o}");
    }

    /// §3: `SOCK_DGRAM` keeps message boundaries. Two sends must arrive
    /// as two receives with the original lengths — no coalescing, no
    /// framing layer needed on top.
    #[test]
    fn uds_3_1_preserves_message_boundaries_two_datagrams() {
        let tmp = short_tempdir();
        let rx = UdsTransport::bind(id_for(70), cfg_with_tmp(tmp.path())).unwrap();
        let tx = UdsTransport::bind(id_for(71), cfg_with_tmp(tmp.path())).unwrap();

        tx.send(&Locator::uds(id_for(70)), b"first").unwrap();
        tx.send(&Locator::uds(id_for(70)), b"second-and-longer")
            .unwrap();

        let a = rx.recv().expect("first datagram");
        let b = rx.recv().expect("second datagram");
        assert_eq!(&a.data[..], b"first");
        assert_eq!(&b.data[..], b"second-and-longer");
    }

    /// §3: a datagram of nearly `max_datagram` arrives whole on every
    /// platform. On macOS the send buffer started at 2048 bytes, and
    /// anything larger failed with `EMSGSIZE`.
    #[test]
    fn uds_3_4_a_datagram_near_the_configured_limit_arrives_whole() {
        let tmp = short_tempdir();
        let cfg = cfg_with_tmp(tmp.path());
        let rx = UdsTransport::bind(id_for(82), cfg.clone()).unwrap();
        let tx = UdsTransport::bind(id_for(83), cfg.clone()).unwrap();
        let big = vec![0x3Cu8; cfg.max_datagram - 512];
        for _ in 0..4 {
            tx.send(&Locator::uds(id_for(82)), &big).unwrap();
        }
        for _ in 0..4 {
            assert_eq!(&rx.recv().expect("datagram").data[..], big.as_slice());
        }
    }

    /// §3: a datagram the kernel accepts arrives byte-for-byte; one it
    /// refuses fails loudly with the EMSGSIZE classification. Never a
    /// truncated delivery — the receive buffer is `max_datagram`, so a
    /// silently clipped message would be indistinguishable from a short
    /// one at the RTPS layer.
    ///
    /// The kernel's per-message cap for unix datagrams is platform
    /// policy: Linux follows `wmem_max` (212992 by default), macOS
    /// `net.local.dgram.maxdgram` (2048 by default). The bind sizes the
    /// buffers and refuses a limit the kernel does not grant
    /// (`uds_3_4_bind_refuses_a_max_datagram_the_kernel_does_not_grant`).
    #[test]
    fn uds_3_3_oversized_for_the_kernel_fails_loudly_and_never_truncates() {
        let tmp = short_tempdir();
        let cfg = cfg_with_tmp(tmp.path());
        let limit = cfg.max_datagram;
        let rx = UdsTransport::bind(id_for(80), cfg.clone()).unwrap();
        let tx = UdsTransport::bind(id_for(81), cfg).unwrap();

        // 1 KiB is below every platform's default cap.
        let small = vec![0xA5u8; 1024];
        tx.send(&Locator::uds(id_for(80)), &small).unwrap();
        let got = rx.recv().expect("1 KiB datagram");
        assert_eq!(&got.data[..], small.as_slice(), "no truncation");

        // At the configured limit the kernel may or may not accept it.
        // Either it arrives whole, or the send reports EMSGSIZE — the
        // one outcome that must not happen is a short delivery.
        let big = vec![0x5Au8; limit];
        match tx.send(&Locator::uds(id_for(80)), &big) {
            Ok(()) => {
                let got = rx.recv().expect("max-sized datagram");
                assert_eq!(&got.data[..], big.as_slice(), "no truncation at the limit");
            }
            Err(SendError::Io { message }) => {
                assert!(
                    message.contains("EMSGSIZE"),
                    "kernel refusal must name its cause, got {message:?}"
                );
                assert_eq!(
                    rx.recv(),
                    Err(RecvError::Timeout),
                    "nothing partial arrived"
                );
            }
            other => panic!("unexpected send result: {other:?}"),
        }
    }

    /// §2.2: an existing directory keeps its mode — the bind sets a mode
    /// only on a directory it created.
    #[test]
    fn uds_2_2_ensure_base_dir_leaves_the_mode_of_an_existing_dir() {
        let tmp = short_tempdir();
        let existing = tmp.path().join("shared");
        fs::create_dir(&existing).unwrap();
        fs::set_permissions(&existing, fs::Permissions::from_mode(0o755)).unwrap();
        ensure_base_dir(&existing, DEFAULT_BASE_DIR_MODE).unwrap();
        assert_eq!(mode_of(&existing), 0o755);
        let config = UdsConfig {
            base_dir_mode: 0o3770,
            ..cfg_with_tmp(&existing)
        };
        let _t = UdsTransport::bind(id_for(100), config).unwrap();
        assert_eq!(mode_of(&existing), 0o755, "the bind leaves it as well");
    }

    /// §3.2: a sender that did not bind has no address; its datagram
    /// arrives with `Locator::INVALID`, not with a guessed locator.
    #[test]
    fn uds_3_2_unbound_sender_yields_locator_invalid() {
        let tmp = short_tempdir();
        let rx = UdsTransport::bind(id_for(101), cfg_with_tmp(tmp.path())).unwrap();
        let anonymous = UnixDatagram::unbound().unwrap();
        anonymous
            .send_to(b"anonymous", socket_path(tmp.path(), id_for(101)))
            .unwrap();
        let got = rx.recv().expect("datagram");
        assert_eq!(&got.data[..], b"anonymous");
        assert_eq!(got.source, Locator::INVALID);
    }

    /// §3.2: a sender bound under another base directory has a name the
    /// receiver cannot answer — its own `socket_path` for that id is a
    /// different file.
    #[test]
    fn uds_3_2_sender_outside_base_dir_yields_locator_invalid() {
        let rx_dir = short_tempdir();
        let tx_dir = short_tempdir();
        let rx = UdsTransport::bind(id_for(102), cfg_with_tmp(rx_dir.path())).unwrap();
        let foreign = UnixDatagram::bind(socket_path(tx_dir.path(), id_for(103))).unwrap();
        foreign
            .send_to(b"from elsewhere", socket_path(rx_dir.path(), id_for(102)))
            .unwrap();
        let got = rx.recv().expect("datagram");
        assert_eq!(&got.data[..], b"from elsewhere");
        assert_eq!(got.source, Locator::INVALID);
    }

    /// §3.2: inside the base directory only a name `socket_path` gives
    /// resolves. A name that is no lowercase hex32, an uppercase hex32, a
    /// different extension and a file in a subdirectory give
    /// `Locator::INVALID`.
    #[test]
    fn uds_3_2_a_name_socket_path_does_not_give_yields_locator_invalid() {
        let tmp = short_tempdir();
        let rx = UdsTransport::bind(id_for(104), cfg_with_tmp(tmp.path())).unwrap();
        let rx_path = socket_path(tmp.path(), id_for(104));
        let hex = hex_id(id_for(0xAB));
        fs::create_dir(tmp.path().join("sub")).unwrap();
        for name in [
            String::from("not-a-locator.sock"),
            // Another id than the control below: on a case-insensitive
            // file system the two names would be one file.
            format!("{}.sock", hex_id(id_for(0xCD)).to_uppercase()),
            format!("{hex}.seqp"),
            format!("sub/{hex}.sock"),
        ] {
            let sender = UnixDatagram::bind(tmp.path().join(&name)).unwrap();
            sender.send_to(name.as_bytes(), &rx_path).unwrap();
            let got = rx.recv().expect("datagram");
            assert_eq!(&got.data[..], name.as_bytes());
            assert_eq!(got.source, Locator::INVALID, "sender {name}");
        }
        // The control: the lowercase name of §2.1 resolves.
        let named = UnixDatagram::bind(socket_path(tmp.path(), id_for(0xAB))).unwrap();
        named.send_to(b"named", &rx_path).unwrap();
        assert_eq!(rx.recv().unwrap().source, Locator::uds(id_for(0xAB)));
    }

    /// §3.3: a datagram above the receiver's `max_datagram` (from a peer
    /// with a larger limit) is an error naming both sizes, not a message
    /// cut to `max_datagram`. The next datagram arrives whole.
    #[test]
    fn uds_3_3_recv_of_a_datagram_above_max_datagram_is_an_error_not_a_short_message() {
        let tmp = short_tempdir();
        let rx_cfg = UdsConfig {
            max_datagram: 1024,
            ..cfg_with_tmp(tmp.path())
        };
        let rx = UdsTransport::bind(id_for(105), rx_cfg).unwrap();
        let tx = UdsTransport::bind(id_for(106), cfg_with_tmp(tmp.path())).unwrap();
        tx.send(&Locator::uds(id_for(105)), &[0x77u8; 4000])
            .unwrap();
        match rx.recv() {
            Err(RecvError::BufferTooSmall {
                datagram_size,
                buffer_size,
            }) => {
                assert_eq!(buffer_size, 1024);
                #[cfg(any(target_os = "linux", target_os = "android"))]
                assert_eq!(datagram_size, 4000, "MSG_TRUNC gives the real size");
                assert!(datagram_size > buffer_size);
            }
            other => panic!("expected BufferTooSmall, got {other:?}"),
        }
        tx.send(&Locator::uds(id_for(105)), &[0x11u8; 1024])
            .unwrap();
        assert_eq!(&rx.recv().expect("next datagram").data[..], &[0x11u8; 1024]);
    }

    /// §3.4: a `max_datagram` far above any kernel's buffer cap fails the
    /// bind with a message naming the limit and the granted size, and
    /// leaves no socket file behind.
    #[test]
    fn uds_3_4_bind_refuses_a_max_datagram_the_kernel_does_not_grant() {
        let tmp = short_tempdir();
        let config = UdsConfig {
            max_datagram: 1 << 30,
            ..cfg_with_tmp(tmp.path())
        };
        let err = UdsTransport::bind(id_for(107), config)
            .err()
            .expect("a 1 GiB datagram limit is refused");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        let message = err.to_string();
        assert!(message.contains("max_datagram 1073741824"), "{message}");
        assert!(message.contains(BUFFER_CAP_SYSCTL), "{message}");
        assert!(!socket_path(tmp.path(), id_for(107)).exists());
    }

    /// §3.4 on Linux: the largest `max_datagram` is `2 * wmem_max - 32`
    /// (the kernel doubles the request it caps at `wmem_max` and refuses a
    /// datagram above `SO_SNDBUF - 32`). That one binds, one byte more
    /// does not.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn uds_3_4_bind_accepts_exactly_the_largest_max_datagram_the_kernel_grants() {
        let sysctl = |name: &str| -> usize {
            fs::read_to_string(format!("/proc/sys/net/core/{name}"))
                .unwrap()
                .trim()
                .parse()
                .unwrap()
        };
        let wmem_max = sysctl("wmem_max");
        assert!(
            sysctl("wmem_default") <= 2 * wmem_max,
            "the start size of the buffer would decide instead of the cap"
        );
        let largest = 2 * wmem_max - SEND_BUFFER_OVERHEAD;
        let tmp = short_tempdir();
        let fits = UdsConfig {
            max_datagram: largest,
            ..cfg_with_tmp(tmp.path())
        };
        let t = UdsTransport::bind(id_for(108), fits).expect("the largest limit binds");
        assert_eq!(Transport::max_message_size(&t), largest);
        let over = UdsConfig {
            max_datagram: largest + 1,
            ..cfg_with_tmp(tmp.path())
        };
        let err = UdsTransport::bind(id_for(109), over)
            .err()
            .expect("one byte more is refused");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(
            err.to_string()
                .contains(&format!("SO_SNDBUF {}", 2 * wmem_max)),
            "{err}"
        );
    }

    /// §5.1: `socket_mode` is the mode of the socket file, whatever the
    /// umask gave; it is what lets a peer under another UID write to it.
    #[test]
    fn uds_5_1_socket_mode_is_applied_to_the_socket_file() {
        let tmp = short_tempdir();
        for (n, mode) in [(110u8, 0o660), (111, 0o600), (112, 0o666)] {
            let config = UdsConfig {
                socket_mode: Some(mode),
                ..cfg_with_tmp(tmp.path())
            };
            let _t = UdsTransport::bind(id_for(n), config).unwrap();
            assert_eq!(mode_of(&socket_path(tmp.path(), id_for(n))), mode);
        }
    }

    /// §5.1: `base_dir_mode` is the mode of a base directory the bind
    /// creates, set-group-ID and sticky bit included.
    #[test]
    fn uds_5_1_base_dir_mode_is_applied_to_a_created_dir() {
        let tmp = short_tempdir();
        let shared = tmp.path().join("volume");
        let config = UdsConfig {
            base_dir_mode: 0o3770,
            ..cfg_with_tmp(&shared)
        };
        let _t = UdsTransport::bind(id_for(113), config).unwrap();
        assert_eq!(mode_of(&shared), 0o3770);
    }

    /// §5.1: the mode goes only to a socket this process bound. A symlink
    /// at the path is not followed — its target keeps its mode — and a
    /// file that is no socket is refused as well.
    #[test]
    fn uds_5_1_apply_socket_mode_refuses_a_symlink_and_a_file_that_is_no_socket() {
        let tmp = short_tempdir();
        let target = tmp.path().join("target");
        fs::write(&target, b"of this uid").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        let link = tmp.path().join("link.sock");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        for path in [&link, &target] {
            let err = apply_socket_mode(path, Some(0o666)).expect_err("refused");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{path:?}: {err}");
            assert!(err.to_string().contains("is not the socket"), "{err}");
        }
        assert_eq!(mode_of(&target), 0o600, "the symlink was not followed");
        // The control: a socket of this process takes the mode.
        let socket = tmp.path().join("own.sock");
        let _bound = UnixDatagram::bind(&socket).unwrap();
        apply_socket_mode(&socket, Some(0o620)).unwrap();
        assert_eq!(mode_of(&socket), 0o620);
    }

    /// §4.3: a peer that does not read fills up, and a send to it then
    /// fails with the full-buffer error instead of blocking the sending
    /// thread. On Linux the kernel queues datagrams until the peer's queue
    /// (`net.unix.max_dgram_qlen + 1` datagrams) or the sender's send
    /// buffer is full, whichever comes first; the next send waits
    /// `send_timeout` for room and fails with `EAGAIN`. Which of the two
    /// was full is checked with a datagram of another sender: a full queue
    /// refuses it as well, a full send buffer of the first sender does
    /// not. macOS fails the send at once with `ENOBUFS` when the receive
    /// buffer is full. What was queued arrives whole, nothing of the failed
    /// send.
    #[test]
    fn uds_4_3_a_peer_that_does_not_read_fails_the_send_with_the_full_buffer_error() {
        let tmp = short_tempdir();
        let rx = UdsTransport::bind(id_for(120), cfg_with_tmp(tmp.path())).unwrap();
        let send_timeout = Duration::from_millis(50);
        let tx = UdsTransport::bind(
            id_for(121),
            UdsConfig {
                send_timeout,
                ..cfg_with_tmp(tmp.path())
            },
        )
        .unwrap();
        let start = std::time::Instant::now();
        let mut queued = 0usize;
        let (err, waited) = loop {
            let before = std::time::Instant::now();
            match tx.send(&Locator::uds(id_for(120)), &queued.to_be_bytes()) {
                Ok(()) => queued += 1,
                Err(e) => break (e, before.elapsed()),
            }
            assert!(queued < 100_000, "the peer's buffer never filled");
        };
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            start.elapsed()
        );
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let other_queued = {
            assert_eq!(
                err,
                SendError::Io {
                    message: "uds: kernel send buffer full (EAGAIN)"
                }
            );
            assert!(
                waited >= send_timeout,
                "the send waited for room: {waited:?}"
            );
            let full_queue = max_dgram_qlen() + 1;
            assert!(
                (1..=full_queue).contains(&queued),
                "{queued} of at most {full_queue}"
            );
            let other = UdsTransport::bind(
                id_for(126),
                UdsConfig {
                    send_timeout: Duration::ZERO,
                    ..cfg_with_tmp(tmp.path())
                },
            )
            .unwrap();
            let other_sent = other.send(&Locator::uds(id_for(120)), b"other");
            if queued == full_queue {
                assert_eq!(
                    other_sent,
                    Err(SendError::Io {
                        message: "uds: kernel send buffer full (EAGAIN)"
                    }),
                    "the peer's queue was full"
                );
            } else {
                assert_eq!(
                    other_sent,
                    Ok(()),
                    "the peer's queue had room: the send buffer of the first sender was full"
                );
            }
            // Kept bound until its datagram is read.
            other_sent.is_ok().then_some(other)
        };
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let other_queued: Option<UdsTransport> = None;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            let _ = waited;
            assert_eq!(
                err,
                SendError::Io {
                    message: "uds: no kernel buffer space for the datagram (ENOBUFS)"
                }
            );
        }
        for n in 0..queued {
            assert_eq!(
                &rx.recv().expect("queued datagram").data[..],
                &n.to_be_bytes()
            );
        }
        if other_queued.is_some() {
            let dg = rx.recv().expect("the other sender's datagram");
            assert_eq!(dg.source, Locator::uds(id_for(126)));
            assert_eq!(&dg.data[..], b"other");
        }
        assert_eq!(
            rx.recv(),
            Err(RecvError::Timeout),
            "nothing of the failed send"
        );
    }

    /// §4.3: with `send_timeout` zero a send to a full queue fails at once
    /// (`MSG_DONTWAIT`); it neither waits nor blocks, as an `SO_SNDTIMEO`
    /// of zero would.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn uds_4_3_with_send_timeout_zero_a_full_queue_fails_the_send_at_once() {
        let tmp = short_tempdir();
        let _rx = UdsTransport::bind(id_for(122), cfg_with_tmp(tmp.path())).unwrap();
        let tx = UdsTransport::bind(
            id_for(123),
            UdsConfig {
                send_timeout: Duration::ZERO,
                ..cfg_with_tmp(tmp.path())
            },
        )
        .unwrap();
        // Until the peer's queue or the send buffer is full; the send that
        // finds no room fails without waiting.
        let mut queued = 0usize;
        let (err, waited) = loop {
            let before = std::time::Instant::now();
            match tx.send(&Locator::uds(id_for(122)), b"fill") {
                Ok(()) => queued += 1,
                Err(e) => break (e, before.elapsed()),
            }
            assert!(queued <= max_dgram_qlen() + 1, "the kernel never refused");
        };
        assert!(queued >= 1);
        assert_eq!(
            err,
            SendError::Io {
                message: "uds: kernel send buffer full (EAGAIN)"
            }
        );
        assert!(waited < Duration::from_secs(1), "{waited:?}");
        let before = std::time::Instant::now();
        assert_eq!(
            tx.send(&Locator::uds(id_for(122)), b"one more"),
            Err(SendError::Io {
                message: "uds: kernel send buffer full (EAGAIN)"
            })
        );
        assert!(
            before.elapsed() < Duration::from_secs(1),
            "{:?}",
            before.elapsed()
        );
    }

    /// §4.3: the wait is for room, not a loss: a send that finds the
    /// peer's queue or its own send buffer full goes out when the peer
    /// starts reading within `send_timeout`, and the peer receives every
    /// datagram in order.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn uds_4_3_a_send_to_a_full_queue_goes_out_when_the_peer_reads_within_send_timeout() {
        let tmp = short_tempdir();
        let rx = UdsTransport::bind(id_for(124), cfg_with_tmp(tmp.path())).unwrap();
        let tx = UdsTransport::bind(
            id_for(125),
            UdsConfig {
                send_timeout: Duration::from_secs(5),
                ..cfg_with_tmp(tmp.path())
            },
        )
        .unwrap();
        let full = fill_until_refused(&tx, id_for(124), |n| n.to_be_bytes().to_vec());
        assert!(full >= 1);
        let reader = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            (0..=full)
                .map(|_| rx.recv().expect("datagram").data.to_vec())
                .collect::<Vec<_>>()
        });
        let before = std::time::Instant::now();
        tx.send(&Locator::uds(id_for(124)), &full.to_be_bytes())
            .expect("the send goes out once the peer reads");
        let waited = before.elapsed();
        let got = reader.join().unwrap();
        assert!(
            waited >= Duration::from_millis(100),
            "it waited: {waited:?}"
        );
        let expected: Vec<Vec<u8>> = (0..=full).map(|n| n.to_be_bytes().to_vec()).collect();
        assert_eq!(got, expected);
    }

    /// §4.3: the default wait.
    #[test]
    fn uds_4_3_the_default_send_timeout_is_100_ms() {
        assert_eq!(DEFAULT_SEND_TIMEOUT, Duration::from_millis(100));
        assert_eq!(UdsConfig::default().send_timeout, DEFAULT_SEND_TIMEOUT);
    }

    /// §5.1: a mode with bits outside the permission bits is refused
    /// before anything is created.
    #[test]
    fn uds_5_1_a_mode_outside_the_permission_bits_is_refused() {
        let tmp = short_tempdir();
        let base = tmp.path().join("never");
        for config in [
            UdsConfig {
                socket_mode: Some(0o4660),
                ..cfg_with_tmp(&base)
            },
            UdsConfig {
                base_dir_mode: 0o17777,
                ..cfg_with_tmp(&base)
            },
        ] {
            let err = UdsTransport::bind(id_for(114), config)
                .err()
                .expect("refused");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
            assert!(!base.exists(), "nothing was created");
        }
    }

    /// DDSI-RTPS 2.5 §8.4.14.1: the transport states its datagram limit.
    #[test]
    fn max_message_size_is_the_datagram_limit() {
        let tmp = short_tempdir();
        let t = UdsTransport::bind(id_for(7), cfg_with_tmp(tmp.path())).unwrap();
        assert_eq!(Transport::max_message_size(&t), 8192);
    }
}

/// `SOCK_DGRAM` with abstract namespace (Linux-only, T5).
#[cfg(target_os = "linux")]
pub mod abstract_dgram;
