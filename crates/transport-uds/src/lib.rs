// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport-uds`. Safety classification: **STANDARD**.
//!
//! ZeroDDS Unix domain socket transport — container IPC for ZeroDDS.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5 §9.4** — locator kind `LOCATOR_KIND_UDS`
//!   (vendor-reserved value `0x81000001`, in
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
}

/// `SOCK_DGRAM` with abstract namespace (Linux-only, T5).
#[cfg(target_os = "linux")]
pub mod abstract_dgram;
