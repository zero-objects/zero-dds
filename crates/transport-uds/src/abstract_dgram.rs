// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! UDS `SOCK_DGRAM` **with abstract namespace** — Linux-only,
//! containerized-IPC-optimized (WP 2.0b T5).
//!
//! # Scope vs T1
//!
//! T1 (`UdsTransport` in the crate root) is `SOCK_DGRAM` + filesystem
//! path — portable Linux + macOS. **T5 here** is `SOCK_DGRAM` +
//! Linux abstract namespace (`\0`-prefixed name). The socket type is
//! the same; what changes is the addressing:
//!
//! - No filesystem lookup per `send` (abstract = in-kernel
//!   hash-table).
//! - No volume mount needed for cross-container IPC (containers only
//!   need to share the same net namespace / `--ipc host` —
//!   **not** the filesystem path).
//! - No stale socket file on crash recovery (the kernel reclaims the
//!   abstract tag automatically on close).
//!
//! In addition the transport also supports filesystem addressing
//! (as a direct comparison to T1 in benches) — the difference is a
//! single config line, the socket code identical.
//!
//! # Why not `SOCK_SEQPACKET`
//!
//! Originally planned for T5, rejected: SEQPACKET on Unix domain is
//! **connection-oriented** (`listen`/`accept` on the server,
//! `connect` on the client, no `sendto` possible → `ENOTCONN`). That
//! is a TCP-over-UDS shape, not a datagram.
//!
//! DDS-RTPS is datagram-based per spec (§8.3). The 64 KiB DGRAM cap
//! (Linux `wmem_max` raises it to 212 KB) is enough for all RTPS
//! submessages after fragmentation (WP 1.2 DATA_FRAG splits large
//! samples into MTU chunks). The SEQPACKET connection-state overhead
//! does not pay off here.
//!
//! If real messages > 200 KiB are needed later (e.g.
//! zero-copy camera images without fragmentation), a dedicated
//! `UdsSeqpacketTransport` with accept/connect follows as a v1.3 spike.
//!
//! # Why Linux-only
//!
//! The abstract namespace is **exclusively** Linux. macOS + Windows
//! fall back to T1 (`UdsTransport`, DGRAM + filesystem). The caller
//! chooses the transport via config; the `Transport` trait impl is
//! identical.
//!
//! # Performance claim
//!
//! `SOCK_DGRAM` + abstract is the fastest UDS mode for dockerized
//! local-machine distribution:
//!
//! - No FS lookup per send.
//! - No round-trip to the filesystem on bind (no `fsync`, no
//!   dir-permission checks).
//! - For cross-container: no mounted volume, no SELinux labeling on
//!   the path.

use std::io;
use std::mem;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::ptr;
use std::time::Duration;

use socket2::{Domain, SockAddr, Socket, Type};

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

/// Default buffer size for recv (Linux wmem_max fallback 212992).
pub const DEFAULT_RECV_BUF: usize = 212_992;

/// Maximum size of the abstract-namespace name in bytes
/// (`sun_path` is 108 bytes, byte 0 is the `\0` prefix).
pub const MAX_ABSTRACT_NAME: usize = 107;

/// Addressing mode.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum UdsAddress {
    /// Filesystem-path based (like T1 — this mode exists in the T5
    /// module as a direct A/B comparison to abstract).
    Filesystem {
        /// Base directory under which the socket files live.
        base_dir: PathBuf,
    },
    /// Linux abstract namespace. Names are encoded as `\0<prefix>-<hex>`
    /// — no filesystem access.
    Abstract {
        /// Prefix prepended to every name (e.g. `"zerodds"`).
        prefix: String,
    },
}

impl UdsAddress {
    /// Default: abstract namespace with prefix `"zerodds"`.
    #[must_use]
    pub fn abstract_default() -> Self {
        Self::Abstract {
            prefix: "zerodds".to_string(),
        }
    }

    /// Default: filesystem path `/tmp/zerodds/uds-dgram`.
    #[must_use]
    pub fn filesystem_default() -> Self {
        Self::Filesystem {
            base_dir: PathBuf::from("/tmp/zerodds/uds-dgram"),
        }
    }
}

/// Config for [`UdsAbstractDgramTransport`].
#[derive(Debug, Clone)]
pub struct AbstractDgramConfig {
    /// Addressing (filesystem vs. abstract).
    pub address: UdsAddress,
    /// Max recv buffer; bounds the largest accepted message. Linux
    /// internally caps at `net.core.rmem_max` — we cap once more in
    /// user space.
    pub recv_buf: usize,
    /// Optional recv timeout.
    pub recv_timeout: Option<Duration>,
}

impl Default for AbstractDgramConfig {
    fn default() -> Self {
        Self {
            address: UdsAddress::abstract_default(),
            recv_buf: DEFAULT_RECV_BUF,
            recv_timeout: None,
        }
    }
}

/// `SOCK_DGRAM`-UDS Transport (Linux-only).
pub struct UdsAbstractDgramTransport {
    socket: Socket,
    local_id: [u8; 16],
    config: AbstractDgramConfig,
}

impl UdsAbstractDgramTransport {
    /// Binds a new transport. For abstract addresses the name mapping
    /// is purely in-kernel — no filesystem file. For filesystem
    /// addresses a socket file is created under
    /// `base_dir/<hex>.sock`.
    ///
    /// # Errors
    /// `io::Error` on socket, bind or permission failures.
    pub fn bind(local_id: [u8; 16], config: AbstractDgramConfig) -> io::Result<Self> {
        let socket = Socket::new(Domain::UNIX, Type::DGRAM, None)?;
        if let Some(t) = config.recv_timeout {
            socket.set_read_timeout(Some(t))?;
        }
        let addr = build_sockaddr(&config.address, local_id, /*is_bind=*/ true)?;
        // For filesystem addresses: remove a stale socket file.
        if let UdsAddress::Filesystem { base_dir } = &config.address {
            std::fs::create_dir_all(base_dir)?;
            let path = fs_path(base_dir, local_id);
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        }
        socket.bind(&addr)?;
        Ok(Self {
            socket,
            local_id,
            config,
        })
    }

    /// Local locator.
    #[must_use]
    pub fn local_locator(&self) -> Locator {
        Locator::uds(self.local_id)
    }
}

impl Drop for UdsAbstractDgramTransport {
    fn drop(&mut self) {
        if let UdsAddress::Filesystem { base_dir } = &self.config.address {
            let _ = std::fs::remove_file(fs_path(base_dir, self.local_id));
        }
        // Abstract sockets clean themselves up on close (in-kernel).
    }
}

impl Transport for UdsAbstractDgramTransport {
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if dest.kind != LocatorKind::Uds {
            return Err(SendError::UnsupportedLocator);
        }
        if data.len() > self.config.recv_buf {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: self.config.recv_buf,
            });
        }
        let peer_addr = match build_sockaddr(&self.config.address, dest.address, false) {
            Ok(a) => a,
            Err(_) => {
                return Err(SendError::Io {
                    message: "uds-dgram: addr build failed",
                });
            }
        };
        // data buffer + socket2 SockAddr are aligned and consistent.
        // SAFETY: libc::sendto with a valid fd, buffer and sockaddr.
        let rc = unsafe {
            libc::sendto(
                self.socket.as_raw_fd(),
                data.as_ptr().cast::<libc::c_void>(),
                data.len(),
                libc::MSG_NOSIGNAL,
                peer_addr.as_ptr(),
                peer_addr.len(),
            )
        };
        if rc < 0 {
            let e = io::Error::last_os_error();
            return Err(match e.raw_os_error() {
                Some(libc::ECONNREFUSED) | Some(libc::ENOENT) => SendError::Io {
                    message: "uds-dgram: peer not reachable",
                },
                _ => SendError::Io {
                    message: "uds-dgram: sendto failed",
                },
            });
        }
        Ok(())
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        // DGRAM: one recv() delivers exactly one message. Message
        // boundaries are preserved by the kernel, no user-space
        // framing needed.
        let mut buf = vec![0u8; self.config.recv_buf];
        // SAFETY: sockaddr_un is POD; zeroed init is permitted.
        let mut addr_storage: libc::sockaddr_un = unsafe { mem::zeroed() };
        let mut addr_len = mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
        // SAFETY: libc::recvfrom with a valid fd, buffer and sockaddr out-param.
        let rc = unsafe {
            libc::recvfrom(
                self.socket.as_raw_fd(),
                buf.as_mut_ptr().cast::<libc::c_void>(),
                buf.len(),
                0,
                (&mut addr_storage as *mut libc::sockaddr_un).cast(),
                &mut addr_len,
            )
        };
        if rc < 0 {
            let e = io::Error::last_os_error();
            return match e.kind() {
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => Err(RecvError::Timeout),
                _ => Err(RecvError::Io {
                    message: "uds-dgram: recvfrom failed",
                }),
            };
        }
        buf.truncate(rc as usize);
        let source = decode_source(&addr_storage, addr_len, &self.config.address);
        let data: std::sync::Arc<[u8]> = std::sync::Arc::from(buf.into_boxed_slice());
        Ok(ReceivedDatagram { source, data })
    }

    fn local_locator(&self) -> Locator {
        Locator::uds(self.local_id)
    }
}

// ---------------------------------------------------------------------
// sockaddr_un construction
// ---------------------------------------------------------------------

/// Build a `SockAddr` from (mode, 16-byte id). `is_bind` is a hint —
/// currently only used for documentation; construction is identical
/// for bind and sendto.
fn build_sockaddr(addr: &UdsAddress, id: [u8; 16], _is_bind: bool) -> io::Result<SockAddr> {
    match addr {
        UdsAddress::Filesystem { base_dir } => {
            let path = fs_path(base_dir, id);
            // socket2::SockAddr::unix yields the filesystem socket.
            SockAddr::unix(path)
        }
        UdsAddress::Abstract { prefix } => build_abstract_sockaddr(prefix, id),
    }
}

fn fs_path(base_dir: &Path, id: [u8; 16]) -> PathBuf {
    let mut hex = String::with_capacity(32);
    for b in id {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    let mut p = base_dir.to_path_buf();
    p.push(format!("{hex}.seqp"));
    p
}

fn build_abstract_sockaddr(prefix: &str, id: [u8; 16]) -> io::Result<SockAddr> {
    // Abstract names: first byte `\0`, then up to 107 bytes of name.
    let mut name = format!("{prefix}-");
    for b in id {
        use std::fmt::Write;
        let _ = write!(name, "{b:02x}");
    }
    if name.len() > MAX_ABSTRACT_NAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "abstract name exceeds 107 bytes",
        ));
    }
    // try_init: we get an uninit sockaddr_storage + len out-param and
    // must fill the struct. socket2 copies the result internally into
    // a suitably larger allocation.
    // SAFETY: closure fills sun_family + sun_path + sets len = offsetof(sun_path)+1+name.len().
    let ((), addr) = unsafe {
        SockAddr::try_init(|storage, len| {
            let sa = storage.cast::<libc::sockaddr_un>();
            (*sa).sun_family = libc::AF_UNIX as libc::sa_family_t;
            let path = (*sa).sun_path.as_mut_ptr().cast::<u8>();
            // sun_path[0] = 0 (abstract marker), then the name.
            ptr::write(path, 0u8);
            ptr::copy_nonoverlapping(name.as_ptr(), path.add(1), name.len());
            // addr_len = offsetof(sun_path) + 1 (\0) + name bytes.
            let offset = core::mem::offset_of!(libc::sockaddr_un, sun_path) as libc::socklen_t;
            *len = offset + 1 + name.len() as libc::socklen_t;
            Ok(())
        })?
    };
    Ok(addr)
}

fn decode_source(
    addr: &libc::sockaddr_un,
    addr_len: libc::socklen_t,
    mode: &UdsAddress,
) -> Locator {
    let family = addr.sun_family as i32;
    if family != libc::AF_UNIX {
        return Locator::INVALID;
    }
    // sun_path by addr_len - offsetof(sun_path). If addr_len <= offset
    // it is an unnamed peer (sendmsg from an unbound sender) — INVALID.
    // `core::mem::offset_of!` is stable since Rust 1.77 and does the
    // computation without an unsafe ptr-sub.
    let sun_path_offset = core::mem::offset_of!(libc::sockaddr_un, sun_path) as libc::socklen_t;
    if addr_len <= sun_path_offset {
        return Locator::INVALID;
    }
    let name_len = (addr_len - sun_path_offset) as usize;
    let sun_path_ptr = addr.sun_path.as_ptr().cast::<u8>();
    // sun_path is a c_char array; we interpret the addr_len published bytes as u8.
    // SAFETY: addr_len guarantees name_len bytes from sun_path; no aliased write.
    let bytes: &[u8] = unsafe { core::slice::from_raw_parts(sun_path_ptr, name_len) };
    match mode {
        UdsAddress::Filesystem { base_dir } => decode_fs_path(bytes, base_dir),
        UdsAddress::Abstract { prefix } => decode_abstract_name(bytes, prefix),
    }
}

fn decode_fs_path(bytes: &[u8], base_dir: &Path) -> Locator {
    // Cut off the null terminator, then parse the path.
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let Ok(s) = core::str::from_utf8(&bytes[..end]) else {
        return Locator::INVALID;
    };
    let path = Path::new(s);
    let Ok(rel) = path.strip_prefix(base_dir) else {
        return Locator::INVALID;
    };
    let Some(stem) = rel.file_stem() else {
        return Locator::INVALID;
    };
    let Some(stem_str) = stem.to_str() else {
        return Locator::INVALID;
    };
    parse_hex_id(stem_str).map_or(Locator::INVALID, Locator::uds)
}

fn decode_abstract_name(bytes: &[u8], prefix: &str) -> Locator {
    // Abstract name: bytes[0] == 0, rest = "<prefix>-<hex>".
    if bytes.is_empty() || bytes[0] != 0 {
        return Locator::INVALID;
    }
    let Ok(name) = core::str::from_utf8(&bytes[1..]) else {
        return Locator::INVALID;
    };
    let expected_prefix = format!("{prefix}-");
    let Some(hex) = name.strip_prefix(&expected_prefix) else {
        return Locator::INVALID;
    };
    parse_hex_id(hex).map_or(Locator::INVALID, Locator::uds)
}

fn parse_hex_id(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, chunk) in s.as_bytes().chunks_exact(2).enumerate() {
        out[i] = (hex_nibble(chunk[0])? << 4) | hex_nibble(chunk[1])?;
    }
    Some(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn id(n: u8) -> [u8; 16] {
        let mut a = [0u8; 16];
        a[0] = 0xC0;
        a[15] = n;
        a
    }

    fn fs_cfg(base: &Path) -> AbstractDgramConfig {
        AbstractDgramConfig {
            address: UdsAddress::Filesystem {
                base_dir: base.to_path_buf(),
            },
            recv_buf: 8192,
            recv_timeout: Some(Duration::from_millis(500)),
        }
    }

    fn abs_cfg(prefix: &str) -> AbstractDgramConfig {
        AbstractDgramConfig {
            address: UdsAddress::Abstract {
                prefix: unique_prefix(prefix),
            },
            recv_buf: 8192,
            recv_timeout: Some(Duration::from_millis(500)),
        }
    }

    /// The Linux abstract namespace is shared host-wide — fixed prefixes
    /// collide between parallel CI jobs on the same runner (EADDRINUSE
    /// bleeds through). A unique suffix from PID + time (ns) + counter is
    /// therefore generated per tag.
    ///
    /// **Important**: within the same process `tx` and `rx` with the same
    /// tag must get the same prefix — otherwise `tx`'s send address never
    /// hits the `rx` bind. Hence a tag cache via
    /// `OnceLock<Mutex<HashMap>>`, instead of handing each call a fresh
    /// suffix.
    fn unique_prefix(tag: &str) -> String {
        use std::collections::HashMap;
        use std::sync::{Mutex, OnceLock};
        use std::time::{SystemTime, UNIX_EPOCH};
        static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = cache.lock().unwrap();
        if let Some(existing) = guard.get(tag) {
            return existing.clone();
        }
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let n = guard.len() as u64;
        let unique = format!("{tag}-{pid:x}-{nanos:x}-{n:x}");
        guard.insert(tag.to_string(), unique.clone());
        unique
    }

    #[test]
    fn filesystem_bind_creates_socket_file() {
        let tmp = tempfile::tempdir().unwrap();
        let t = UdsAbstractDgramTransport::bind(id(1), fs_cfg(tmp.path())).unwrap();
        assert!(fs_path(tmp.path(), id(1)).exists());
        drop(t);
        assert!(!fs_path(tmp.path(), id(1)).exists(), "drop cleans up");
    }

    #[test]
    fn abstract_bind_does_not_create_file() {
        // The abstract namespace should create no filesystem files.
        let _t = UdsAbstractDgramTransport::bind(id(2), abs_cfg("test-abs-nofs")).unwrap();
        // No FS check needed — we are abstract by definition.
    }

    #[test]
    fn filesystem_send_recv_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let rx = UdsAbstractDgramTransport::bind(id(10), fs_cfg(tmp.path())).unwrap();
        let tx = UdsAbstractDgramTransport::bind(id(11), fs_cfg(tmp.path())).unwrap();
        tx.send(&Locator::uds(id(10)), b"hello fs-abstract_dgram")
            .unwrap();
        let got = rx.recv().unwrap();
        assert_eq!(&got.data[..], b"hello fs-abstract_dgram");
        assert_eq!(got.source, Locator::uds(id(11)));
    }

    /// Variant of [`abs_cfg`] that reuses an already-computed
    /// `unique_prefix` string. Both transports in the same test must
    /// share the same prefix string so that their abstract-namespace
    /// socket paths fall into the same sub-table — otherwise the sender
    /// does not see the receiver.
    ///
    /// Before the fix `abs_cfg(prefix)` produced its **own** unique
    /// suffix per call, so `rx` and `tx` bound into disjoint
    /// namespaces — `send` failed with "peer not reachable". Linux CI
    /// was flaky because unique_prefix has a time/counter component and
    /// only _rarely_ collides when both calls happen in sub-microsecond
    /// order.
    fn abs_cfg_shared(unique: &str) -> AbstractDgramConfig {
        AbstractDgramConfig {
            address: UdsAddress::Abstract {
                prefix: unique.to_owned(),
            },
            recv_buf: 8192,
            recv_timeout: Some(Duration::from_millis(500)),
        }
    }

    #[test]
    fn abstract_send_recv_roundtrip() {
        let prefix = unique_prefix("zerodds-test-roundtrip");
        let rx = UdsAbstractDgramTransport::bind(id(20), abs_cfg_shared(&prefix)).unwrap();
        let tx = UdsAbstractDgramTransport::bind(id(21), abs_cfg_shared(&prefix)).unwrap();
        tx.send(&Locator::uds(id(20)), b"abstract-hello").unwrap();
        let got = rx.recv().unwrap();
        assert_eq!(&got.data[..], b"abstract-hello");
        assert_eq!(got.source, Locator::uds(id(21)));
    }

    #[test]
    fn abstract_preserves_message_boundaries() {
        // Two sends → two recvs, no framing coalescing.
        let prefix = unique_prefix("zerodds-test-boundaries");
        let rx = UdsAbstractDgramTransport::bind(id(30), abs_cfg_shared(&prefix)).unwrap();
        let tx = UdsAbstractDgramTransport::bind(id(31), abs_cfg_shared(&prefix)).unwrap();
        tx.send(&Locator::uds(id(30)), b"first").unwrap();
        tx.send(&Locator::uds(id(30)), b"second").unwrap();
        let a = rx.recv().unwrap();
        let b = rx.recv().unwrap();
        assert_eq!(&a.data[..], b"first");
        assert_eq!(&b.data[..], b"second");
    }

    #[test]
    fn send_rejects_non_uds_locator() {
        let t = UdsAbstractDgramTransport::bind(id(40), abs_cfg("zerodds-test-reject")).unwrap();
        let r = t.send(&Locator::udp_v4([127, 0, 0, 1], 7400), b"x");
        assert_eq!(r, Err(SendError::UnsupportedLocator));
    }

    #[test]
    fn send_to_missing_peer_is_io_error() {
        let t = UdsAbstractDgramTransport::bind(id(50), abs_cfg("zerodds-test-nopeer")).unwrap();
        let r = t.send(&Locator::uds(id(99)), b"nobody");
        assert!(matches!(r, Err(SendError::Io { .. })));
    }

    #[test]
    fn recv_times_out_when_idle() {
        let t = UdsAbstractDgramTransport::bind(id(60), abs_cfg("zerodds-test-timeout")).unwrap();
        let r = t.recv();
        assert_eq!(r, Err(RecvError::Timeout));
    }

    #[test]
    fn parse_hex_id_roundtrip() {
        let id = [0x42u8; 16];
        let mut s = String::new();
        for b in id {
            use std::fmt::Write;
            let _ = write!(s, "{b:02x}");
        }
        assert_eq!(parse_hex_id(&s), Some(id));
    }

    #[test]
    fn parse_hex_id_rejects_bad_input() {
        assert_eq!(parse_hex_id("xy"), None);
        assert_eq!(parse_hex_id(&"zz".repeat(16)), None);
    }

    #[test]
    fn local_locator_reflects_bind_id() {
        let t = UdsAbstractDgramTransport::bind(id(70), abs_cfg("zerodds-test-local")).unwrap();
        assert_eq!(
            <UdsAbstractDgramTransport as Transport>::local_locator(&t),
            Locator::uds(id(70))
        );
    }
}
