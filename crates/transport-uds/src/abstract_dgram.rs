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

use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use socket2::{Domain, SockAddr, Socket, Type};

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

/// Maximum size of the abstract-namespace name in bytes
/// (`sun_path` is 108 bytes, byte 0 is the `\0` prefix).
pub const MAX_ABSTRACT_NAME: usize = 107;

/// Addressing mode.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum UdsAddress {
    /// Filesystem-path based, the addressing of `UdsTransport` (§2.1):
    /// `<base_dir>/<lowercase-hex32>.sock`. This mode exists in the T5
    /// module as a direct A/B comparison to abstract, and a
    /// `UdsTransport` under the same `base_dir` reaches it.
    Filesystem {
        /// Base directory under which the socket files live.
        base_dir: PathBuf,
        /// Mode of the socket file, set right after the bind; `None`
        /// keeps the umask's ([`crate::UdsConfig::socket_mode`]).
        socket_mode: Option<u32>,
        /// Mode of `base_dir` when the bind creates it
        /// ([`crate::UdsConfig::base_dir_mode`]).
        base_dir_mode: u32,
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

    /// Default: filesystem path [`crate::DEFAULT_BASE_DIR`], the
    /// directory of `UdsTransport`, with its default modes.
    #[must_use]
    pub fn filesystem_default() -> Self {
        Self::Filesystem {
            base_dir: PathBuf::from(crate::DEFAULT_BASE_DIR),
            socket_mode: None,
            base_dir_mode: crate::DEFAULT_BASE_DIR_MODE,
        }
    }
}

/// Config for [`UdsAbstractDgramTransport`].
#[derive(Debug, Clone)]
pub struct AbstractDgramConfig {
    /// Addressing (filesystem vs. abstract).
    pub address: UdsAddress,
    /// The largest datagram this transport sends or receives, as
    /// [`crate::UdsConfig::max_datagram`]: a larger payload is refused on
    /// send, a larger datagram is reported on receive, and the bind fails
    /// when the kernel does not grant socket buffers that carry it.
    pub max_datagram: usize,
    /// Optional recv timeout.
    pub recv_timeout: Option<Duration>,
    /// How long a send waits for room in a full receive queue of the peer
    /// before it fails with "uds: kernel send buffer full (EAGAIN)", as
    /// [`crate::UdsConfig::send_timeout`]; [`Duration::ZERO`] does not wait.
    /// Default [`crate::DEFAULT_SEND_TIMEOUT`].
    pub send_timeout: Duration,
}

impl Default for AbstractDgramConfig {
    fn default() -> Self {
        Self {
            address: UdsAddress::abstract_default(),
            max_datagram: crate::DEFAULT_MAX_DATAGRAM,
            recv_timeout: None,
            send_timeout: crate::DEFAULT_SEND_TIMEOUT,
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
    /// addresses a socket file is created at `base_dir/<hex>.sock`, with
    /// the base directory handled as by `UdsTransport::bind` (§2.2).
    ///
    /// # Errors
    /// `InvalidInput` for an abstract name above [`MAX_ABSTRACT_NAME`]
    /// bytes, a mode outside the permitted bits, a `base_dir` that is a
    /// symlink or not a directory, a socket path that is no longer the
    /// socket this process bound when its mode is set, or a `max_datagram`
    /// the kernel's socket buffers do not carry; `PermissionDenied` for a
    /// created `base_dir` whose mode the kernel did not set as asked;
    /// `io::Error` on socket, bind or permission failures.
    pub fn bind(local_id: [u8; 16], config: AbstractDgramConfig) -> io::Result<Self> {
        let addr = build_sockaddr(&config.address, local_id)?;
        if let UdsAddress::Filesystem {
            base_dir,
            socket_mode,
            base_dir_mode,
        } = &config.address
        {
            crate::check_modes(*socket_mode, *base_dir_mode)?;
            crate::ensure_base_dir(base_dir, *base_dir_mode)?;
            crate::remove_stale_socket(&crate::socket_path(base_dir, local_id))?;
        }
        let socket = Socket::new(Domain::UNIX, Type::DGRAM, None)?;
        socket.bind(&addr)?;
        // From here on a failure drops the transport, whose Drop removes
        // the socket file of the filesystem mode again.
        let transport = Self {
            socket,
            local_id,
            config,
        };
        if let UdsAddress::Filesystem {
            base_dir,
            socket_mode,
            ..
        } = &transport.config.address
        {
            crate::apply_socket_mode(&crate::socket_path(base_dir, local_id), *socket_mode)?;
        }
        if let Some(t) = transport.config.recv_timeout {
            transport.socket.set_read_timeout(Some(t))?;
        }
        crate::set_send_timeout(
            socket2::SockRef::from(&transport.socket),
            transport.config.send_timeout,
        )?;
        crate::size_buffers(
            socket2::SockRef::from(&transport.socket),
            transport.config.max_datagram,
        )?;
        Ok(transport)
    }

    /// Local locator.
    #[must_use]
    pub fn local_locator(&self) -> Locator {
        Locator::uds(self.local_id)
    }
}

impl Drop for UdsAbstractDgramTransport {
    fn drop(&mut self) {
        if let UdsAddress::Filesystem { base_dir, .. } = &self.config.address {
            let _ = std::fs::remove_file(crate::socket_path(base_dir, self.local_id));
        }
        // Abstract sockets clean themselves up on close (in-kernel).
    }
}

impl Transport for UdsAbstractDgramTransport {
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
        let peer_addr =
            build_sockaddr(&self.config.address, dest.address).map_err(|_| SendError::Io {
                message: "uds-dgram: addr build failed",
            })?;
        // A full receive queue of the peer waits at most `send_timeout`,
        // then the send fails with `EAGAIN` (§4.3).
        match self.socket.send_to_with_flags(
            data,
            &peer_addr,
            crate::send_flags(self.config.send_timeout),
        ) {
            Ok(_) => Ok(()),
            Err(e) => Err(classify_send_error(&e, &self.config.address)),
        }
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        // DGRAM: one recv() delivers exactly one message. Message
        // boundaries are preserved by the kernel, no user-space
        // framing needed.
        let (buf, addr) = crate::recv_datagram(
            socket2::SockRef::from(&self.socket),
            self.config.max_datagram,
            "uds-dgram: recvfrom failed",
        )?;
        let source = decode_source(&addr, &self.config.address);
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

/// §4.3 for both modes. In the abstract namespace there is no file:
/// `ECONNREFUSED` there means no socket holds the name, which the
/// filesystem classification would call a refusing socket file.
fn classify_send_error(e: &io::Error, mode: &UdsAddress) -> SendError {
    match mode {
        UdsAddress::Abstract { .. } if e.raw_os_error() == Some(libc::ECONNREFUSED) => {
            SendError::Io {
                message: "uds-dgram: no socket bound to the abstract destination name (ECONNREFUSED)",
            }
        }
        _ => crate::classify_send_error(e),
    }
}

// ---------------------------------------------------------------------
// sockaddr_un construction
// ---------------------------------------------------------------------

/// Build a `SockAddr` from (mode, 16-byte id); the same address serves
/// bind and sendto.
fn build_sockaddr(addr: &UdsAddress, id: [u8; 16]) -> io::Result<SockAddr> {
    match addr {
        UdsAddress::Filesystem { base_dir, .. } => SockAddr::unix(crate::socket_path(base_dir, id)),
        UdsAddress::Abstract { prefix } => build_abstract_sockaddr(prefix, id),
    }
}

/// The abstract name of `id`: `<prefix>-<lowercase-hex32>`, without the
/// leading null byte.
fn abstract_name(prefix: &str, id: [u8; 16]) -> String {
    format!("{prefix}-{}", crate::hex_id(id))
}

fn build_abstract_sockaddr(prefix: &str, id: [u8; 16]) -> io::Result<SockAddr> {
    // Abstract names: first byte `\0`, then up to 107 bytes of name.
    let name = abstract_name(prefix, id);
    if name.len() > MAX_ABSTRACT_NAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "abstract name exceeds 107 bytes",
        ));
    }
    // socket2 takes a path that starts with a null byte as an abstract
    // name: no terminator, length offsetof(sun_path) + 1 + name bytes.
    let mut raw = Vec::with_capacity(1 + name.len());
    raw.push(0u8);
    raw.extend_from_slice(name.as_bytes());
    SockAddr::unix(Path::new(OsStr::from_bytes(&raw)))
}

fn decode_source(addr: &SockAddr, mode: &UdsAddress) -> Locator {
    match mode {
        UdsAddress::Filesystem { base_dir, .. } => {
            crate::source_locator(addr.as_pathname(), base_dir)
        }
        UdsAddress::Abstract { prefix } => addr
            .as_abstract_namespace()
            .map_or(Locator::INVALID, |name| decode_abstract_name(name, prefix)),
    }
}

/// The id in an abstract name (without its leading null byte) of the
/// form `<prefix>-<lowercase-hex32>`; any other name, a foreign prefix
/// among them, gives `Locator::INVALID` (§3.2).
fn decode_abstract_name(name: &[u8], prefix: &str) -> Locator {
    let Ok(name) = core::str::from_utf8(name) else {
        return Locator::INVALID;
    };
    let Some(hex) = name
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('-'))
    else {
        return Locator::INVALID;
    };
    crate::parse_hex_id(hex).map_or(Locator::INVALID, Locator::uds)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn id(n: u8) -> [u8; 16] {
        let mut a = [0u8; 16];
        a[0] = 0xC0;
        a[15] = n;
        a
    }

    fn fs_address(base: &Path) -> UdsAddress {
        UdsAddress::Filesystem {
            base_dir: base.to_path_buf(),
            socket_mode: None,
            base_dir_mode: crate::DEFAULT_BASE_DIR_MODE,
        }
    }

    fn fs_cfg(base: &Path) -> AbstractDgramConfig {
        AbstractDgramConfig {
            address: fs_address(base),
            max_datagram: 8192,
            recv_timeout: Some(Duration::from_millis(500)),
            ..AbstractDgramConfig::default()
        }
    }

    fn abs_cfg(prefix: &str) -> AbstractDgramConfig {
        abs_cfg_shared(&unique_prefix(prefix))
    }

    fn mode_of(path: &Path) -> u32 {
        std::fs::symlink_metadata(path)
            .unwrap()
            .permissions()
            .mode()
            & 0o7777
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

    /// Variant of [`abs_cfg`] that reuses an already-computed
    /// `unique_prefix` string. Both transports in the same test must
    /// share the same prefix string so that their abstract-namespace
    /// names fall into the same sub-table — otherwise the sender does not
    /// see the receiver.
    fn abs_cfg_shared(unique: &str) -> AbstractDgramConfig {
        AbstractDgramConfig {
            address: UdsAddress::Abstract {
                prefix: unique.to_owned(),
            },
            max_datagram: 8192,
            recv_timeout: Some(Duration::from_millis(500)),
            ..AbstractDgramConfig::default()
        }
    }

    /// The `/proc/net/unix` lines whose path column is `path` (an abstract
    /// name shows as `@<name>`).
    fn proc_net_unix_entries(path: &str) -> usize {
        std::fs::read_to_string("/proc/net/unix")
            .unwrap()
            .lines()
            .filter(|line| line.split_whitespace().nth(7) == Some(path))
            .count()
    }

    #[test]
    fn uds_2_4_filesystem_bind_creates_socket_file() {
        let tmp = tempfile::tempdir().unwrap();
        let t = UdsAbstractDgramTransport::bind(id(1), fs_cfg(tmp.path())).unwrap();
        assert!(crate::socket_path(tmp.path(), id(1)).exists());
        drop(t);
        assert!(
            !crate::socket_path(tmp.path(), id(1)).exists(),
            "drop cleans up"
        );
    }

    /// §2.3 and §4.2: an abstract bind holds a name in the kernel's
    /// table (`/proc/net/unix` lists it as `@<name>`) and creates no
    /// file — neither under the working directory nor under `/tmp`, and
    /// no filesystem entry of that name in the table.
    #[test]
    fn uds_2_3_abstract_bind_does_not_create_file() {
        let prefix = unique_prefix("test-abs-nofs");
        let t = UdsAbstractDgramTransport::bind(id(2), abs_cfg_shared(&prefix)).unwrap();
        let name = abstract_name(&prefix, id(2));
        assert_eq!(proc_net_unix_entries(&format!("@{name}")), 1);
        assert!(!Path::new(&name).exists());
        assert!(!Path::new("/tmp").join(&name).exists());
        let as_path = std::fs::read_to_string("/proc/net/unix")
            .unwrap()
            .lines()
            .filter(|line| {
                line.split_whitespace()
                    .nth(7)
                    .is_some_and(|p| p.ends_with(&name) && !p.starts_with('@'))
            })
            .count();
        assert_eq!(as_path, 0, "no filesystem socket carries the name");
        drop(t);
        assert_eq!(proc_net_unix_entries(&format!("@{name}")), 0);
    }

    /// §2.3: the bound name is a null byte followed by
    /// `<prefix>-<lowercase-hex32>`, read back from the kernel with
    /// `getsockname`.
    #[test]
    fn uds_2_3_abstract_name_is_nul_prefix_dash_lowercase_hex32() {
        let prefix = unique_prefix("zerodds-test-name");
        let mut wide = [0u8; 16];
        wide[0] = 0xAB;
        wide[7] = 0xCD;
        wide[15] = 0xEF;
        let t = UdsAbstractDgramTransport::bind(wide, abs_cfg_shared(&prefix)).unwrap();
        let local = t.socket.local_addr().unwrap();
        let expected = format!("{prefix}-ab000000000000cd00000000000000ef");
        assert_eq!(local.as_abstract_namespace(), Some(expected.as_bytes()));
        assert_eq!(local.as_pathname(), None);
        let offset = core::mem::offset_of!(libc::sockaddr_un, sun_path);
        assert_eq!(
            local.len() as usize,
            offset + 1 + expected.len(),
            "no terminator after the name"
        );
    }

    /// §2.3: the name fits `sun_path` — 107 bytes bind, 108 are refused
    /// with `InvalidInput` before any socket exists.
    #[test]
    fn uds_2_3_abstract_name_above_107_bytes_is_refused() {
        // "<prefix>-" + 32 hex digits: a prefix of 74 bytes gives 107.
        let base = unique_prefix("zerodds-test-long");
        let fits = format!("{base:x<74}");
        assert_eq!(abstract_name(&fits, id(3)).len(), MAX_ABSTRACT_NAME);
        let _t =
            UdsAbstractDgramTransport::bind(id(3), abs_cfg_shared(&fits)).expect("107 bytes fit");
        let over = format!("{fits}x");
        let err = UdsAbstractDgramTransport::bind(id(4), abs_cfg_shared(&over))
            .err()
            .expect("108 bytes do not");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn uds_2_4_filesystem_send_recv_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let rx = UdsAbstractDgramTransport::bind(id(10), fs_cfg(tmp.path())).unwrap();
        let tx = UdsAbstractDgramTransport::bind(id(11), fs_cfg(tmp.path())).unwrap();
        tx.send(&Locator::uds(id(10)), b"hello fs-abstract_dgram")
            .unwrap();
        let got = rx.recv().unwrap();
        assert_eq!(&got.data[..], b"hello fs-abstract_dgram");
        assert_eq!(got.source, Locator::uds(id(11)));
    }

    /// §2.4: the filesystem mode binds the path of §2.1,
    /// `<base_dir>/<lowercase-hex32>.sock`, and its default directory is
    /// that of `UdsTransport`.
    #[test]
    fn uds_2_4_filesystem_mode_binds_the_path_of_2_1() {
        let tmp = tempfile::tempdir().unwrap();
        let t = UdsAbstractDgramTransport::bind(id(12), fs_cfg(tmp.path())).unwrap();
        let literal = tmp.path().join("c000000000000000000000000000000c.sock");
        assert!(literal.exists(), "{literal:?}");
        let local = t.socket.local_addr().unwrap();
        assert_eq!(local.as_pathname(), Some(literal.as_path()));
        match UdsAddress::filesystem_default() {
            UdsAddress::Filesystem {
                base_dir,
                socket_mode,
                base_dir_mode,
            } => {
                assert_eq!(base_dir, Path::new("/tmp/zerodds/uds"));
                assert_eq!(socket_mode, None);
                assert_eq!(base_dir_mode, 0o700);
            }
            UdsAddress::Abstract { .. } => panic!("filesystem_default is the filesystem mode"),
        }
    }

    /// §2.4: under one `base_dir` a `UdsTransport` and the filesystem mode
    /// of this transport reach each other in both directions, each seeing
    /// the other's locator as the source.
    #[test]
    fn uds_2_4_filesystem_mode_exchanges_datagrams_with_uds_transport() {
        let tmp = tempfile::tempdir().unwrap();
        let plain = crate::UdsTransport::bind(
            id(13),
            crate::UdsConfig {
                base_dir: tmp.path().to_path_buf(),
                max_datagram: 8192,
                recv_timeout: Some(Duration::from_millis(500)),
                ..crate::UdsConfig::default()
            },
        )
        .unwrap();
        let fs_mode = UdsAbstractDgramTransport::bind(id(14), fs_cfg(tmp.path())).unwrap();
        plain.send(&Locator::uds(id(14)), b"to fs mode").unwrap();
        let got = fs_mode.recv().unwrap();
        assert_eq!(&got.data[..], b"to fs mode");
        assert_eq!(got.source, Locator::uds(id(13)));
        fs_mode.send(&Locator::uds(id(13)), b"to plain").unwrap();
        let got = plain.recv().unwrap();
        assert_eq!(&got.data[..], b"to plain");
        assert_eq!(got.source, Locator::uds(id(14)));
    }

    /// §2.2 in the filesystem mode: a missing base directory is created
    /// with mode `0700`, or with `base_dir_mode`.
    #[test]
    fn uds_2_2_filesystem_mode_creates_base_dir_with_mode_0700() {
        let tmp = tempfile::tempdir().unwrap();
        let private = tmp.path().join("private");
        let _a = UdsAbstractDgramTransport::bind(id(15), fs_cfg(&private)).unwrap();
        assert_eq!(mode_of(&private), 0o700);
        let shared = tmp.path().join("shared");
        let config = AbstractDgramConfig {
            address: UdsAddress::Filesystem {
                base_dir: shared.clone(),
                socket_mode: None,
                base_dir_mode: 0o3770,
            },
            ..fs_cfg(&shared)
        };
        let _b = UdsAbstractDgramTransport::bind(id(16), config).unwrap();
        assert_eq!(mode_of(&shared), 0o3770);
    }

    /// §2.2 in the filesystem mode: a base directory that is a symlink is
    /// refused and the link's target keeps its mode.
    #[test]
    fn uds_2_2_filesystem_mode_rejects_a_symlink_base_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let err = UdsAbstractDgramTransport::bind(id(17), fs_cfg(&link))
            .err()
            .expect("a symlink base_dir is refused");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(mode_of(&target), 0o755);
        assert!(!crate::socket_path(&target, id(17)).exists());
        let file = tmp.path().join("file");
        std::fs::write(&file, b"").unwrap();
        let err = UdsAbstractDgramTransport::bind(id(18), fs_cfg(&file))
            .err()
            .expect("a file base_dir is refused");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn uds_2_3_abstract_send_recv_roundtrip() {
        let prefix = unique_prefix("zerodds-test-roundtrip");
        let rx = UdsAbstractDgramTransport::bind(id(20), abs_cfg_shared(&prefix)).unwrap();
        let tx = UdsAbstractDgramTransport::bind(id(21), abs_cfg_shared(&prefix)).unwrap();
        tx.send(&Locator::uds(id(20)), b"abstract-hello").unwrap();
        let got = rx.recv().unwrap();
        assert_eq!(&got.data[..], b"abstract-hello");
        assert_eq!(got.source, Locator::uds(id(21)));
    }

    #[test]
    fn uds_2_3_abstract_preserves_message_boundaries() {
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

    /// §3.2: a sender without a name yields `Locator::INVALID`.
    #[test]
    fn uds_3_2_abstract_unbound_sender_yields_locator_invalid() {
        let prefix = unique_prefix("zerodds-test-unbound");
        let rx = UdsAbstractDgramTransport::bind(id(32), abs_cfg_shared(&prefix)).unwrap();
        let anonymous = Socket::new(Domain::UNIX, Type::DGRAM, None).unwrap();
        anonymous
            .send_to(
                b"anonymous",
                &build_abstract_sockaddr(&prefix, id(32)).unwrap(),
            )
            .unwrap();
        let got = rx.recv().unwrap();
        assert_eq!(&got.data[..], b"anonymous");
        assert_eq!(got.source, Locator::INVALID);
    }

    /// §3.2: a sender whose abstract name carries another prefix (or no
    /// hex32 after the prefix) is not answerable under this prefix.
    #[test]
    fn uds_3_2_abstract_foreign_prefix_yields_locator_invalid() {
        let prefix = unique_prefix("zerodds-test-own");
        let foreign = unique_prefix("zerodds-test-foreign");
        let rx = UdsAbstractDgramTransport::bind(id(33), abs_cfg_shared(&prefix)).unwrap();
        let dest = build_abstract_sockaddr(&prefix, id(33)).unwrap();
        for sender_name in [
            build_abstract_sockaddr(&foreign, id(34)).unwrap(),
            build_abstract_sockaddr(&format!("{prefix}-x"), id(35)).unwrap(),
        ] {
            let sender = Socket::new(Domain::UNIX, Type::DGRAM, None).unwrap();
            sender.bind(&sender_name).unwrap();
            sender.send_to(b"foreign", &dest).unwrap();
            let got = rx.recv().unwrap();
            assert_eq!(got.source, Locator::INVALID);
        }
    }

    /// §3.2 in the filesystem mode: a sender outside the base directory
    /// yields `Locator::INVALID`.
    #[test]
    fn uds_3_2_filesystem_mode_sender_outside_base_dir_yields_locator_invalid() {
        let rx_dir = tempfile::tempdir().unwrap();
        let tx_dir = tempfile::tempdir().unwrap();
        let rx = UdsAbstractDgramTransport::bind(id(36), fs_cfg(rx_dir.path())).unwrap();
        let tx = UdsAbstractDgramTransport::bind(id(37), fs_cfg(tx_dir.path())).unwrap();
        tx.socket
            .send_to(
                b"from elsewhere",
                &SockAddr::unix(crate::socket_path(rx_dir.path(), id(36))).unwrap(),
            )
            .unwrap();
        let got = rx.recv().unwrap();
        assert_eq!(&got.data[..], b"from elsewhere");
        assert_eq!(got.source, Locator::INVALID);
    }

    /// §3.3: a datagram above `max_datagram` is an error with its real
    /// size, not a cut message; the next one arrives whole.
    #[test]
    fn uds_3_3_abstract_recv_above_max_datagram_is_an_error() {
        let prefix = unique_prefix("zerodds-test-trunc");
        let rx_cfg = AbstractDgramConfig {
            max_datagram: 1024,
            ..abs_cfg_shared(&prefix)
        };
        let rx = UdsAbstractDgramTransport::bind(id(38), rx_cfg).unwrap();
        let tx = UdsAbstractDgramTransport::bind(id(39), abs_cfg_shared(&prefix)).unwrap();
        tx.send(&Locator::uds(id(38)), &[0x42u8; 3000]).unwrap();
        assert_eq!(
            rx.recv(),
            Err(RecvError::BufferTooSmall {
                datagram_size: 3000,
                buffer_size: 1024
            })
        );
        tx.send(&Locator::uds(id(38)), &[0x24u8; 1024]).unwrap();
        assert_eq!(&rx.recv().unwrap().data[..], &[0x24u8; 1024]);
    }

    /// §3.3: the limit is `max_datagram`, by default 65 536 as for
    /// `UdsTransport`.
    #[test]
    fn uds_3_3_abstract_default_max_datagram_is_65536() {
        let config = AbstractDgramConfig::default();
        assert_eq!(config.max_datagram, 65_536);
        assert_eq!(config.max_datagram, crate::DEFAULT_MAX_DATAGRAM);
        let prefix = unique_prefix("zerodds-test-default-max");
        let t = UdsAbstractDgramTransport::bind(
            id(40),
            AbstractDgramConfig {
                address: UdsAddress::Abstract { prefix },
                ..AbstractDgramConfig::default()
            },
        )
        .unwrap();
        assert_eq!(Transport::max_message_size(&t), 65_536);
        assert_eq!(
            t.send(&Locator::uds(id(41)), &vec![0u8; 65_537]),
            Err(SendError::PayloadTooLarge {
                size: 65_537,
                limit: 65_536
            })
        );
    }

    /// §3.4: the bind refuses a `max_datagram` the kernel does not grant,
    /// in both modes, and the filesystem mode leaves no file behind.
    #[test]
    fn uds_3_4_abstract_bind_refuses_a_max_datagram_the_kernel_does_not_grant() {
        let tmp = tempfile::tempdir().unwrap();
        for config in [
            AbstractDgramConfig {
                max_datagram: 1 << 30,
                ..abs_cfg("zerodds-test-kernel-cap")
            },
            AbstractDgramConfig {
                max_datagram: 1 << 30,
                ..fs_cfg(tmp.path())
            },
        ] {
            let err = UdsAbstractDgramTransport::bind(id(42), config)
                .err()
                .expect("a 1 GiB datagram limit is refused");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
            assert!(err.to_string().contains("max_datagram 1073741824"), "{err}");
        }
        assert!(!crate::socket_path(tmp.path(), id(42)).exists());
    }

    /// §4.2: the kernel frees an abstract name when the last descriptor
    /// closes — a second bind of the name fails while the first holds it
    /// and succeeds after its drop, with no cleanup in between.
    #[test]
    fn uds_4_2_abstract_name_is_free_after_drop() {
        let prefix = unique_prefix("zerodds-test-free");
        let first = UdsAbstractDgramTransport::bind(id(43), abs_cfg_shared(&prefix)).unwrap();
        let err = UdsAbstractDgramTransport::bind(id(43), abs_cfg_shared(&prefix))
            .err()
            .expect("the name is held");
        assert_eq!(err.kind(), io::ErrorKind::AddrInUse);
        drop(first);
        let _again = UdsAbstractDgramTransport::bind(id(43), abs_cfg_shared(&prefix))
            .expect("the name is free after the drop");
    }

    #[test]
    fn send_rejects_non_uds_locator() {
        let t = UdsAbstractDgramTransport::bind(id(44), abs_cfg("zerodds-test-reject")).unwrap();
        let r = t.send(&Locator::udp_v4([127, 0, 0, 1], 7400), b"x");
        assert_eq!(r, Err(SendError::UnsupportedLocator));
    }

    /// §4.3: in the abstract namespace a name nobody holds is named as
    /// such, not as a refusing socket file.
    #[test]
    fn uds_4_3_abstract_send_to_missing_peer_names_the_cause() {
        let t = UdsAbstractDgramTransport::bind(id(45), abs_cfg("zerodds-test-nopeer")).unwrap();
        let r = t.send(&Locator::uds(id(99)), b"nobody");
        assert_eq!(
            r,
            Err(SendError::Io {
                message: "uds-dgram: no socket bound to the abstract destination name (ECONNREFUSED)"
            })
        );
    }

    /// §4.3 in the filesystem mode: the causes of `UdsTransport` — a
    /// missing socket file and a file whose socket is gone.
    #[test]
    fn uds_4_3_filesystem_mode_send_errors_name_the_cause() {
        let tmp = tempfile::tempdir().unwrap();
        let t = UdsAbstractDgramTransport::bind(id(46), fs_cfg(tmp.path())).unwrap();
        assert_eq!(
            t.send(&Locator::uds(id(47)), b"nobody"),
            Err(SendError::Io {
                message: "uds: destination socket missing"
            })
        );
        drop(
            std::os::unix::net::UnixDatagram::bind(crate::socket_path(tmp.path(), id(48))).unwrap(),
        );
        assert_eq!(
            t.send(&Locator::uds(id(48)), b"leftover"),
            Err(SendError::Io {
                message: "uds: peer socket exists but refused (ECONNREFUSED)"
            })
        );
    }

    /// §4.3 in both modes: a peer that does not read fills up; the kernel
    /// queues datagrams until the peer's queue (`net.unix.max_dgram_qlen +
    /// 1` datagrams) or the sender's send buffer is full, whichever comes
    /// first. The send that finds no room, and each one after it, waits
    /// `send_timeout` and fails with `EAGAIN` instead of blocking until the
    /// peer reads.
    #[test]
    fn uds_4_3_a_full_peer_queue_fails_the_send_after_send_timeout_in_both_modes() {
        let qlen: usize = std::fs::read_to_string("/proc/sys/net/unix/max_dgram_qlen")
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let prefix = unique_prefix("zerodds-test-full");
        let send_timeout = Duration::from_millis(50);
        for (rx_cfg, tx_cfg) in [
            (abs_cfg_shared(&prefix), abs_cfg_shared(&prefix)),
            (fs_cfg(tmp.path()), fs_cfg(tmp.path())),
        ] {
            let _rx = UdsAbstractDgramTransport::bind(id(62), rx_cfg).unwrap();
            let tx = UdsAbstractDgramTransport::bind(
                id(63),
                AbstractDgramConfig {
                    send_timeout,
                    ..tx_cfg
                },
            )
            .unwrap();
            let mut queued = 0usize;
            let first = loop {
                match tx.send(&Locator::uds(id(62)), b"fill") {
                    Ok(()) => queued += 1,
                    Err(e) => break e,
                }
                assert!(queued <= qlen + 1, "the kernel never refused");
            };
            assert!(queued >= 1);
            assert_eq!(
                first,
                SendError::Io {
                    message: "uds: kernel send buffer full (EAGAIN)"
                }
            );
            let before = std::time::Instant::now();
            assert_eq!(
                tx.send(&Locator::uds(id(62)), b"one more"),
                Err(SendError::Io {
                    message: "uds: kernel send buffer full (EAGAIN)"
                })
            );
            let waited = before.elapsed();
            assert!(waited >= send_timeout, "{waited:?}");
            assert!(waited < Duration::from_secs(5), "{waited:?}");
        }
    }

    #[test]
    fn uds_4_4_an_abstract_socket_recv_times_out_when_idle() {
        let t = UdsAbstractDgramTransport::bind(id(60), abs_cfg("zerodds-test-timeout")).unwrap();
        let r = t.recv();
        assert_eq!(r, Err(RecvError::Timeout));
    }

    /// §5.1 in the filesystem mode: `socket_mode` is the socket file's
    /// mode, `base_dir_mode` that of a created base directory.
    #[test]
    fn uds_5_1_filesystem_mode_applies_socket_mode_and_base_dir_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let volume = tmp.path().join("volume");
        let config = AbstractDgramConfig {
            address: UdsAddress::Filesystem {
                base_dir: volume.clone(),
                socket_mode: Some(0o660),
                base_dir_mode: 0o3770,
            },
            ..fs_cfg(&volume)
        };
        let _t = UdsAbstractDgramTransport::bind(id(61), config).unwrap();
        assert_eq!(mode_of(&volume), 0o3770);
        assert_eq!(mode_of(&crate::socket_path(&volume, id(61))), 0o660);
    }

    #[test]
    fn uds_2_3_local_locator_reflects_bind_id() {
        let t = UdsAbstractDgramTransport::bind(id(70), abs_cfg("zerodds-test-local")).unwrap();
        assert_eq!(
            <UdsAbstractDgramTransport as Transport>::local_locator(&t),
            Locator::uds(id(70))
        );
    }

    /// DDSI-RTPS 2.5 §8.4.14.1: the transport states its datagram limit.
    #[test]
    fn max_message_size_is_the_datagram_limit() {
        let t = UdsAbstractDgramTransport::bind(id(71), abs_cfg("zerodds-test-maxmsg")).unwrap();
        assert_eq!(
            <UdsAbstractDgramTransport as Transport>::max_message_size(&t),
            8192
        );
    }
}
