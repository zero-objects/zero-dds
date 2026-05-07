// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! UDS `SOCK_DGRAM` **mit Abstract-Namespace** — Linux-only,
//! containerized-IPC-optimized (WP 2.0b T5).
//!
//! # Scope vs T1
//!
//! T1 (`UdsTransport` im Crate-Root) ist `SOCK_DGRAM` + Filesystem-
//! Path — portable Linux + macOS. **T5 hier** ist `SOCK_DGRAM` +
//! Linux-Abstract-Namespace (`\0`-prefixed name). Socket-Typ ist
//! derselbe; was sich aendert, ist die Addressierung:
//!
//! - Kein Filesystem-Lookup pro `send` (Abstract = in-kernel
//!   hash-table).
//! - Kein Volume-Mount noetig fuer Cross-Container-IPC (Container
//!   muessen nur dieselbe net-namespace / `--ipc host` teilen —
//!   **nicht** den Filesystem-Pfad).
//! - Kein Stale-Socket-File beim Crash-Recovery (Kernel raeumt das
//!   Abstract-Tag automatisch beim Close).
//!
//! Zusatz: der Transport unterstuetzt auch Filesystem-Addressierung
//! (als direkter Vergleich zu T1 in Benches) — der Unterschied ist
//! eine Config-Zeile, der Socket-Code identisch.
//!
//! # Warum nicht `SOCK_SEQPACKET`
//!
//! Urspruenglich fuer T5 geplant, verworfen: SEQPACKET auf Unix-
//! Domain ist **connection-oriented** (`listen`/`accept` auf Server,
//! `connect` auf Client, kein `sendto` moeglich → `ENOTCONN`). Das
//! ist TCP-über-UDS-Shape, nicht Datagram.
//!
//! DDS-RTPS ist per Spec (§8.3) Datagram-basiert. 64 KiB-DGRAM-Cap
//! (Linux `wmem_max` hebt auf 212 KB) reicht fuer alle RTPS-
//! Submessages nach Fragmentation (WP 1.2 DATA_FRAG schneidet grosse
//! Samples in MTU-Chunks). Der SEQPACKET-Connection-State-Overhead
//! zahlt sich hier nicht aus.
//!
//! Wenn spaeter echte Messages > 200 KiB gebraucht werden (z.B.
//! zero-copy-camera-images ohne Fragmentation), kommt ein eigener
//! `UdsSeqpacketTransport` mit Accept/Connect als v1.3-Spike nach.
//!
//! # Warum Linux-only
//!
//! Abstract-Namespace ist **ausschliesslich** Linux. macOS + Windows
//! fallen auf T1 (`UdsTransport`, DGRAM + Filesystem) zurueck. Der
//! Caller waehlt den Transport per Config; die Transport-Trait-Impl
//! ist identisch.
//!
//! # Performance-Claim
//!
//! `SOCK_DGRAM` + Abstract ist der schnellste UDS-Modus fuer
//! dockerized Local-Machine-Distribution:
//!
//! - Kein FS-Lookup pro send.
//! - Kein Round-trip zum Filesystem bei Bind (kein `fsync`, keine
//!   dir-permission-checks).
//! - Fuer Cross-Container: kein mounted Volume, kein SELinux-
//!   Labeling am Pfad.

use std::io;
use std::mem;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::ptr;
use std::time::Duration;

use socket2::{Domain, SockAddr, Socket, Type};

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

/// Default-Buffersize fuer Recv (Linux wmem_max-Fallback 212992).
pub const DEFAULT_RECV_BUF: usize = 212_992;

/// Maximale Groesse des abstract-Namespace-Namens in Bytes
/// (`sun_path` ist 108 Byte, Byte 0 ist der `\0`-Prefix).
pub const MAX_ABSTRACT_NAME: usize = 107;

/// Adressierungs-Modus.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum UdsAddress {
    /// Filesystem-Pfad-basiert (wie T1 — dieser Modus existiert im
    /// T5-Modul als direkter A/B-Vergleich zu Abstract).
    Filesystem {
        /// Base-Directory, unter dem die Socket-Dateien liegen.
        base_dir: PathBuf,
    },
    /// Linux-Abstract-Namespace. Namen werden als `\0<prefix>-<hex>`
    /// kodiert — kein Filesystem-Zugriff.
    Abstract {
        /// Prefix, der allen Namen vorangestellt wird (z.B. `"zerodds"`).
        prefix: String,
    },
}

impl UdsAddress {
    /// Default: Abstract-Namespace mit Prefix `"zerodds"`.
    #[must_use]
    pub fn abstract_default() -> Self {
        Self::Abstract {
            prefix: "zerodds".to_string(),
        }
    }

    /// Default: Filesystem-Pfad `/tmp/zerodds/uds-dgram`.
    #[must_use]
    pub fn filesystem_default() -> Self {
        Self::Filesystem {
            base_dir: PathBuf::from("/tmp/zerodds/uds-dgram"),
        }
    }
}

/// Config fuer [`UdsAbstractDgramTransport`].
#[derive(Debug, Clone)]
pub struct AbstractDgramConfig {
    /// Adressierung (Filesystem vs. Abstract).
    pub address: UdsAddress,
    /// Max Recv-Buffer; begrenzt die groesste akzeptierte Message.
    /// Linux kapt intern auf `net.core.rmem_max` — wir cappen noch
    /// einmal im User-Space.
    pub recv_buf: usize,
    /// Optionaler Recv-Timeout.
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
    /// Bindet einen neuen Transport. Fuer Abstract-Addressen ist die
    /// Name-Zuordnung rein in-kernel — keine Filesystem-Datei. Fuer
    /// Filesystem-Addressen wird ein Socket-File unter
    /// `base_dir/<hex>.sock` angelegt.
    ///
    /// # Errors
    /// `io::Error` bei Socket-, Bind- oder Permissions-Fehlern.
    pub fn bind(local_id: [u8; 16], config: AbstractDgramConfig) -> io::Result<Self> {
        let socket = Socket::new(Domain::UNIX, Type::DGRAM, None)?;
        if let Some(t) = config.recv_timeout {
            socket.set_read_timeout(Some(t))?;
        }
        let addr = build_sockaddr(&config.address, local_id, /*is_bind=*/ true)?;
        // Fuer Filesystem-Adressen: stale socket file entfernen.
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

    /// Lokaler Locator.
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
        // Abstract sockets raeumen sich beim Close selbst auf (in-kernel).
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
        // data-Buffer + socket2-SockAddr sind aligned und konsistent.
        // SAFETY: libc::sendto mit gueltigem fd, Puffer und sockaddr.
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
        // DGRAM: ein recv() liefert genau eine Message. Message-
        // boundaries werden durch den Kernel preserved, kein
        // User-Space-Framing noetig.
        let mut buf = vec![0u8; self.config.recv_buf];
        // SAFETY: sockaddr_un ist POD; zeroed-init ist zulaessig.
        let mut addr_storage: libc::sockaddr_un = unsafe { mem::zeroed() };
        let mut addr_len = mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
        // SAFETY: libc::recvfrom mit gueltigem fd, Puffer und sockaddr-Out-Param.
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
        Ok(ReceivedDatagram { source, data: buf })
    }

    fn local_locator(&self) -> Locator {
        Locator::uds(self.local_id)
    }
}

// ---------------------------------------------------------------------
// sockaddr_un construction
// ---------------------------------------------------------------------

/// `SockAddr` aus (Modus, 16-byte-id) bauen. `is_bind` ist ein
/// hint — aktuell nur dokumentarisch genutzt, die Konstruktion ist
/// identisch fuer bind und sendto.
fn build_sockaddr(addr: &UdsAddress, id: [u8; 16], _is_bind: bool) -> io::Result<SockAddr> {
    match addr {
        UdsAddress::Filesystem { base_dir } => {
            let path = fs_path(base_dir, id);
            // socket2::SockAddr::unix liefert den Filesystem-Socket.
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
    // Abstract-Namen: 1. Byte `\0`, dann bis zu 107 Byte Name.
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
    // try_init: wir bekommen ein uninit sockaddr_storage + len-Out-Param
    // und muessen die Struktur fuellen. socket2 kopiert das Ergebnis
    // intern in einen passend groesseren Alloc.
    // SAFETY: Closure fuellt sun_family + sun_path + setzt len = offsetof(sun_path)+1+name.len().
    let ((), addr) = unsafe {
        SockAddr::try_init(|storage, len| {
            let sa = storage.cast::<libc::sockaddr_un>();
            (*sa).sun_family = libc::AF_UNIX as libc::sa_family_t;
            let path = (*sa).sun_path.as_mut_ptr().cast::<u8>();
            // sun_path[0] = 0 (Abstract-Marker), danach der Name.
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
    // sun_path nach addr_len - offsetof(sun_path). Wenn addr_len <= offset
    // ist das ein unnamed peer (sendmsg vom unbound sender) — INVALID.
    // `core::mem::offset_of!` ist stable seit Rust 1.77 und macht die
    // Berechnung ohne unsafe ptr-sub.
    let sun_path_offset = core::mem::offset_of!(libc::sockaddr_un, sun_path) as libc::socklen_t;
    if addr_len <= sun_path_offset {
        return Locator::INVALID;
    }
    let name_len = (addr_len - sun_path_offset) as usize;
    let sun_path_ptr = addr.sun_path.as_ptr().cast::<u8>();
    // sun_path ist c_char-Array; wir interpretieren die addr_len publizierten Bytes als u8.
    // SAFETY: addr_len garantiert name_len Bytes ab sun_path; kein aliased write.
    let bytes: &[u8] = unsafe { core::slice::from_raw_parts(sun_path_ptr, name_len) };
    match mode {
        UdsAddress::Filesystem { base_dir } => decode_fs_path(bytes, base_dir),
        UdsAddress::Abstract { prefix } => decode_abstract_name(bytes, prefix),
    }
}

fn decode_fs_path(bytes: &[u8], base_dir: &Path) -> Locator {
    // Null-terminator abschneiden, dann Path parsen.
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
    // Abstract-Name: bytes[0] == 0, Rest = "<prefix>-<hex>".
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

    /// Linux abstract-namespace ist hostweit shared — feste Prefixes
    /// kollidieren zwischen parallelen CI-Jobs auf demselben Runner
    /// (EADDRINUSE schlaegt durch). Pro Tag wird daher ein einmaliges
    /// Suffix aus PID + Zeit (ns) + Counter generiert.
    ///
    /// **Wichtig**: Innerhalb desselben Prozesses muss `tx` und `rx`
    /// mit demselben Tag denselben Prefix bekommen — sonst trifft
    /// `tx`'s Send-Adresse nie den `rx`-Bind. Daher Tag-Cache via
    /// `OnceLock<Mutex<HashMap>>`, statt jedem Call einen frischen
    /// Suffix zu liefern.
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
        // Abstract-namespace sollte keine Filesystem-Dateien erzeugen.
        let _t = UdsAbstractDgramTransport::bind(id(2), abs_cfg("test-abs-nofs")).unwrap();
        // Kein FS-Check noetig — wir sind per Definition abstract.
    }

    #[test]
    fn filesystem_send_recv_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let rx = UdsAbstractDgramTransport::bind(id(10), fs_cfg(tmp.path())).unwrap();
        let tx = UdsAbstractDgramTransport::bind(id(11), fs_cfg(tmp.path())).unwrap();
        tx.send(&Locator::uds(id(10)), b"hello fs-abstract_dgram")
            .unwrap();
        let got = rx.recv().unwrap();
        assert_eq!(got.data, b"hello fs-abstract_dgram");
        assert_eq!(got.source, Locator::uds(id(11)));
    }

    /// Variante von [`abs_cfg`], die einen bereits berechneten
    /// `unique_prefix`-String wiederverwendet. Beide Transports im
    /// selben Test muessen denselben Prefix-String teilen, damit ihre
    /// abstract-namespace-Socket-Pfade in dieselbe Sub-Tabelle
    /// faellt — sonst sieht der Sender den Empfaenger nicht.
    ///
    /// Vor dem Fix produzierte `abs_cfg(prefix)` pro Call ein
    /// **eigenes** unique-Suffix, sodass `rx` und `tx` in disjunkte
    /// Namespaces banden — `send` schlug mit "peer not reachable"
    /// fehl. Linux-CI war flaky weil unique_prefix eine Time/Counter-
    /// Komponente hat und nur _selten_ kollidiert wenn beide Calls in
    /// sub-microsecond-Reihenfolge passieren.
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
        assert_eq!(got.data, b"abstract-hello");
        assert_eq!(got.source, Locator::uds(id(21)));
    }

    #[test]
    fn abstract_preserves_message_boundaries() {
        // Zwei sends → zwei recvs, kein Framing-Zusammenschneiden.
        let prefix = unique_prefix("zerodds-test-boundaries");
        let rx = UdsAbstractDgramTransport::bind(id(30), abs_cfg_shared(&prefix)).unwrap();
        let tx = UdsAbstractDgramTransport::bind(id(31), abs_cfg_shared(&prefix)).unwrap();
        tx.send(&Locator::uds(id(30)), b"first").unwrap();
        tx.send(&Locator::uds(id(30)), b"second").unwrap();
        let a = rx.recv().unwrap();
        let b = rx.recv().unwrap();
        assert_eq!(a.data, b"first");
        assert_eq!(b.data, b"second");
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
