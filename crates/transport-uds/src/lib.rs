// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport-uds`. Safety classification: **STANDARD**.
//!
//! ZeroDDS Unix domain socket transport — container IPC for ZeroDDS.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5 §9.4** — locator kind `LOCATOR_KIND_UDS`
//!   (vendor-reserved value `0x01F00002`, in
//!   `crates/rtps/src/wire_types.rs`).
//! - **ZeroDDS-UDS-Transport 1.0** — vendor-specific transport spec
//!   (filesystem path resolution, abstract namespace, SOCK_DGRAM
//!   wire format), `docs/spec-coverage/zerodds-uds-transport-1.0.md`.
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
//! ## Implemented (RC1)
//!
//! - `SOCK_DGRAM` over filesystem sockets (default mode).
//! - Linux abstract-namespace support via the `abstract_dgram` module
//!   (datagrams without a filesystem inode).
//! - Lazy base-directory creation with mode `0700`.
//! - TOCTOU-safe socket creation (path.exists + fail-fast).
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
//! message boundaries. Default path resolution: 16-byte `Locator`
//! address → `<base_dir>/<lowercase-hex>.sock`. Default `base_dir` =
//! `/tmp/zerodds/uds`.
//!
//! ## Safety / unsafe scope
//!
//! The default DGRAM module (`lib.rs`) is safe-only (`std::os::unix::net::UnixDatagram`).
//! The Linux-only `abstract_dgram` module uses `libc::sendto`/`libc::recvfrom`
//! plus raw `sockaddr_un` construction for the abstract namespace — it
//! contains limited `unsafe` blocks documented with SAFETY comments.

#![warn(missing_docs)]
// Unix domain sockets exist only on POSIX — on Windows this crate
// compiles to an empty lib. Consumers gate their uds usage with
// `#[cfg(unix)]`.
#![cfg(unix)]

use std::fs;
use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::time::Duration;

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

/// Default base directory for UDS sockets.
pub const DEFAULT_BASE_DIR: &str = "/tmp/zerodds/uds";

/// Maximum UDS datagram size we accept on receive. Linux default
/// `wmem_max` is 212992 bytes; we cap a bit below to stay under the
/// typical kernel limit and match the UDP-transport recv-buffer size.
pub const DEFAULT_MAX_DATAGRAM: usize = 65_536;

/// Configuration for opening a `UdsTransport`.
#[derive(Debug, Clone)]
pub struct UdsConfig {
    /// Base directory for socket files.
    pub base_dir: PathBuf,
    /// Max datagram we expect; determines recv-buffer size.
    pub max_datagram: usize,
    /// Optional recv timeout; `None` → block indefinitely.
    pub recv_timeout: Option<Duration>,
}

impl Default for UdsConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from(DEFAULT_BASE_DIR),
            max_datagram: DEFAULT_MAX_DATAGRAM,
            recv_timeout: None,
        }
    }
}

/// Renders a locator ID (16 bytes) to a filesystem path under
/// `base_dir`.
#[must_use]
pub fn socket_path(base_dir: &Path, id: [u8; 16]) -> PathBuf {
    let mut hex = String::with_capacity(32);
    for byte in id {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    let mut p = base_dir.to_path_buf();
    p.push(format!("{hex}.sock"));
    p
}

/// UDS-based transport bound to a specific 16-byte locator ID.
pub struct UdsTransport {
    socket: UnixDatagram,
    local_id: [u8; 16],
    config: UdsConfig,
}

impl UdsTransport {
    /// Bind a new transport at `local_id`. The socket file is created
    /// under `config.base_dir`; if a stale socket file exists it is
    /// removed first (typical Unix practice after a crash).
    ///
    /// # Errors
    /// - I/O error creating `base_dir` or binding the socket.
    pub fn bind(local_id: [u8; 16], config: UdsConfig) -> io::Result<Self> {
        ensure_base_dir(&config.base_dir)?;
        let path = socket_path(&config.base_dir, local_id);
        // Remove stale socket from a prior crashed instance. UnixDatagram
        // cannot bind on top of an existing file; Linux returns EADDRINUSE.
        if path.exists() {
            fs::remove_file(&path)?;
        }
        let socket = UnixDatagram::bind(&path)?;
        if let Some(t) = config.recv_timeout {
            socket.set_read_timeout(Some(t))?;
        }
        size_buffers(socket2::SockRef::from(&socket), config.max_datagram)?;
        Ok(Self {
            socket,
            local_id,
            config,
        })
    }
}

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

/// Sizes the socket buffers for datagrams of `max_datagram` bytes.
///
/// The kernel refuses a unix datagram larger than the sender's send
/// buffer. macOS starts that buffer at `net.local.dgram.maxdgram` (2048)
/// and the receive buffer at `net.local.dgram.recvspace` (4096), so a
/// datagram above 2 KiB failed with `EMSGSIZE` there (measured on a Mac,
/// 2026-10-02) and the receiver held about one at a time. Linux starts at
/// `wmem_default`/`rmem_default` and caps a request at `wmem_max`/`rmem_max`.
pub(crate) fn size_buffers(sock: socket2::SockRef<'_>, max_datagram: usize) -> io::Result<()> {
    let send = max_datagram.saturating_mul(2);
    let recv = max_datagram.saturating_mul(8);
    if sock.send_buffer_size()? < send {
        sock.set_send_buffer_size(send)?;
    }
    if sock.recv_buffer_size()? < recv {
        sock.set_recv_buffer_size(recv)?;
    }
    Ok(())
}

fn ensure_base_dir(path: &Path) -> io::Result<()> {
    // TOCTOU fix: `path.exists()` followed by `set_permissions` is racy
    // — an attacker could place a symlink to a foreign path between the
    // check and the set, tricking us into chmod'ing 0o700 onto /etc or
    // /root.
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
            // Exists + directory + no symlink → OK, nothing to do.
            // We deliberately no longer set `0o700` on foreign dirs.
            return Ok(());
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // Path doesn't exist → create it under our ownership.
        }
        Err(e) => return Err(e),
    }
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o700);
        // Re-check: the path exists now, but run symlink_metadata once
        // more to avoid a race between create_dir_all and chmod (someone
        // could rename+symlink between the calls).
        let meta_after = fs::symlink_metadata(path)?;
        if meta_after.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "uds base_dir became a symlink between create and chmod",
            ));
        }
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

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
        match self.socket.send_to(data, &path) {
            Ok(_) => Ok(()),
            Err(e) => Err(classify_send_error(&e)),
        }
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        let mut buf = vec![0u8; self.config.max_datagram];
        match self.socket.recv_from(&mut buf) {
            Ok((len, addr)) => {
                buf.truncate(len);
                let source = source_locator(&addr, &self.config.base_dir);
                let data: std::sync::Arc<[u8]> = std::sync::Arc::from(buf.into_boxed_slice());
                Ok(ReceivedDatagram { source, data })
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
                message: "uds recv failed",
            }),
        }
    }

    fn local_locator(&self) -> Locator {
        Locator::uds(self.local_id)
    }
}

/// Differentiated `io::Error` classification.
/// Previously all errors collapsed onto a generic `SendError::Io`, so
/// that e.g. "peer not yet bound" was indistinguishable from
/// "permission denied" or "kernel buffer full".
fn classify_send_error(e: &io::Error) -> SendError {
    match e.kind() {
        io::ErrorKind::NotFound => SendError::Io {
            message: "uds: destination socket missing",
        },
        io::ErrorKind::PermissionDenied => SendError::Io {
            message: "uds: permission denied on destination",
        },
        io::ErrorKind::WouldBlock => SendError::Io {
            message: "uds: kernel send buffer full (EAGAIN)",
        },
        io::ErrorKind::ConnectionRefused => SendError::Io {
            message: "uds: peer socket exists but refused (ECONNREFUSED)",
        },
        // EMSGSIZE has no named `io::ErrorKind` on every platform, so it
        // used to land in the catch-all and read as a transient failure.
        // It is not: the kernel's per-message cap for unix datagrams can
        // sit far below `DEFAULT_MAX_DATAGRAM` (macOS
        // `net.local.dgram.maxdgram` defaults to 2048), and the caller
        // must be able to tell that apart from a full buffer.
        _ if e.raw_os_error() == Some(libc::EMSGSIZE) => SendError::Io {
            message: "uds: datagram exceeds the kernel per-message limit (EMSGSIZE)",
        },
        _ => SendError::Io {
            message: "uds: send failed",
        },
    }
}

/// Extract the peer's locator ID from a `SocketAddr`. If the address
/// is unnamed (anonymous client that did not bind), we return
/// `Locator::INVALID` — higher layers must be prepared for that
/// because RTPS cannot route responses without a peer locator.
fn source_locator(addr: &std::os::unix::net::SocketAddr, base_dir: &Path) -> Locator {
    let Some(path) = addr.as_pathname() else {
        return Locator::INVALID;
    };
    let Ok(rel) = path.strip_prefix(base_dir) else {
        return Locator::INVALID;
    };
    let Some(stem) = rel.file_stem() else {
        return Locator::INVALID;
    };
    let Some(stem_str) = stem.to_str() else {
        return Locator::INVALID;
    };
    let Ok(id) = parse_hex_id(stem_str) else {
        return Locator::INVALID;
    };
    Locator::uds(id)
}

fn parse_hex_id(s: &str) -> Result<[u8; 16], ()> {
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

fn hex_nibble(b: u8) -> Result<u8, ()> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(()),
    }
}

#[cfg(test)]
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
        }
    }

    #[test]
    fn socket_path_is_hex_under_base_dir() {
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
    fn parse_hex_id_roundtrip() {
        let id = [0x11u8; 16];
        let mut hex = String::new();
        for b in id {
            use std::fmt::Write;
            let _ = write!(hex, "{b:02x}");
        }
        assert_eq!(parse_hex_id(&hex), Ok(id));
    }

    #[test]
    fn parse_hex_id_rejects_wrong_length() {
        assert_eq!(parse_hex_id("abc"), Err(()));
    }

    #[test]
    fn parse_hex_id_rejects_non_hex() {
        assert_eq!(parse_hex_id(&"zz".repeat(16)), Err(()));
    }

    #[test]
    fn bind_creates_socket_file() {
        let tmp = tempfile::tempdir().unwrap();
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
    fn bind_reuses_path_after_stale_leftover() {
        let tmp = tempfile::tempdir().unwrap();
        let path = socket_path(tmp.path(), id_for(2));
        fs::create_dir_all(tmp.path()).unwrap();
        // Simulate a leftover socket file from a crashed process.
        fs::write(&path, b"stale").unwrap();
        let _t = UdsTransport::bind(id_for(2), cfg_with_tmp(tmp.path()))
            .expect("bind must remove stale file and succeed");
        assert!(path.exists());
    }

    #[test]
    fn send_and_recv_roundtrip_same_process() {
        let tmp = tempfile::tempdir().unwrap();
        let rx = UdsTransport::bind(id_for(10), cfg_with_tmp(tmp.path())).unwrap();
        let tx = UdsTransport::bind(id_for(11), cfg_with_tmp(tmp.path())).unwrap();

        tx.send(&Locator::uds(id_for(10)), b"hello zerodds")
            .unwrap();

        let got = rx.recv().expect("recv");
        assert_eq!(&got.data[..], b"hello zerodds");
        assert_eq!(got.source, Locator::uds(id_for(11)));
    }

    #[test]
    fn send_rejects_non_uds_locator() {
        let tmp = tempfile::tempdir().unwrap();
        let tx = UdsTransport::bind(id_for(20), cfg_with_tmp(tmp.path())).unwrap();
        let res = tx.send(&Locator::udp_v4([127, 0, 0, 1], 7400), b"x");
        assert_eq!(res, Err(SendError::UnsupportedLocator));
    }

    #[test]
    fn send_rejects_oversize_payload() {
        let tmp = tempfile::tempdir().unwrap();
        let tx = UdsTransport::bind(id_for(30), cfg_with_tmp(tmp.path())).unwrap();
        let big = vec![0u8; 10_000]; // > 8192 cfg limit
        let res = tx.send(&Locator::uds(id_for(31)), &big);
        assert!(matches!(res, Err(SendError::PayloadTooLarge { .. })));
    }

    #[test]
    fn send_to_missing_peer_is_io_error() {
        let tmp = tempfile::tempdir().unwrap();
        let tx = UdsTransport::bind(id_for(40), cfg_with_tmp(tmp.path())).unwrap();
        let res = tx.send(&Locator::uds(id_for(99)), b"nobody home");
        assert!(matches!(res, Err(SendError::Io { .. })));
    }

    #[test]
    fn recv_times_out_when_idle() {
        let tmp = tempfile::tempdir().unwrap();
        let rx = UdsTransport::bind(id_for(50), cfg_with_tmp(tmp.path())).unwrap();
        let res = rx.recv();
        assert_eq!(res, Err(RecvError::Timeout));
    }

    #[test]
    fn local_locator_reflects_bind_id() {
        let tmp = tempfile::tempdir().unwrap();
        let t = UdsTransport::bind(id_for(60), cfg_with_tmp(tmp.path())).unwrap();
        assert_eq!(t.local_locator(), Locator::uds(id_for(60)));
    }

    // ---- Coverage-Boost ----

    #[test]
    fn classify_send_error_maps_kinds() {
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

    #[cfg(unix)]
    #[test]
    fn ensure_base_dir_rejects_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        fs::create_dir(&real).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let err = ensure_base_dir(&link).expect_err("symlink must be rejected");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn ensure_base_dir_rejects_file_path() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("not-a-dir");
        fs::write(&file, b"").unwrap();
        let err = ensure_base_dir(&file).expect_err("regular file must be rejected");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn ensure_base_dir_creates_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("created");
        ensure_base_dir(&missing).unwrap();
        assert!(missing.is_dir());
    }

    /// DDSI-RTPS 2.5 §9.3.2.4 reserves `0x00000003` to `0x01ffffff` for
    /// vendor-specific kinds.
    /// ZeroDDS takes `0x01F0_0002` for UDS; the transport binds that
    /// value, so a renumbering has to break here.
    #[test]
    fn locator_kind_is_the_vendor_reserved_uds_value() {
        let loc = Locator::uds(id_for(90));
        assert_eq!(loc.kind.as_i32(), 0x01F0_0002);
        assert!(
            (0x0000_0003..=0x01ff_ffff).contains(&loc.kind.as_i32()),
            "inside the vendor-specific range of §9.3.2.4"
        );
        assert_eq!(loc.port, 0, "the port field is unused for UDS");
        assert_eq!(loc.address, id_for(90));
    }

    /// §2.1: a base directory we create ourselves is user-private —
    /// mode `0700`, no group or other bits.
    #[cfg(unix)]
    #[test]
    fn ensure_base_dir_creates_dir_with_mode_0700() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let created = tmp.path().join("private");
        ensure_base_dir(&created).unwrap();
        let mode = fs::metadata(&created).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "base dir must be user-private, got {mode:o}");
    }

    /// §3: `SOCK_DGRAM` keeps message boundaries. Two sends must arrive
    /// as two receives with the original lengths — no coalescing, no
    /// framing layer needed on top.
    #[test]
    fn preserves_message_boundaries_two_datagrams() {
        let tmp = tempfile::tempdir().unwrap();
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
    fn a_datagram_near_the_configured_limit_arrives_whole() {
        let tmp = tempfile::tempdir().unwrap();
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
    /// policy, not ours: Linux follows `wmem_max` (212992 by default),
    /// macOS `net.local.dgram.maxdgram` (2048 by default). The
    /// configured `max_datagram` is therefore an upper bound we enforce,
    /// not a guarantee the OS honours — see the `partial` remainder of
    /// the §3 spec item.
    #[test]
    fn oversized_for_the_kernel_fails_loudly_and_never_truncates() {
        let tmp = tempfile::tempdir().unwrap();
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

    /// DDSI-RTPS 2.5 §8.4.14.1: the transport states its datagram limit.
    #[test]
    fn max_message_size_is_the_datagram_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let t = UdsTransport::bind(id_for(7), cfg_with_tmp(tmp.path())).unwrap();
        assert_eq!(Transport::max_message_size(&t), 8192);
    }
}

/// `SOCK_DGRAM` with abstract namespace (Linux-only, T5).
#[cfg(target_os = "linux")]
pub mod abstract_dgram;
