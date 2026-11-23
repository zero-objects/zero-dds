// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Isolation-Smoke-Test Runner (WP 2.0b T4).
//!
//! A thin CLI binary that runs as sender or receiver on
//! a selectable transport. Used by the isolation scripts
//! (`testing/isolation/`) in L1-L4 orchestrated.
//!
//! # Usage
//!
//! ```text
//! isolation-smoke --transport=udp --role=receiver \
//!     --local=127.0.0.1:7410 --count=10
//!
//! isolation-smoke --transport=udp --role=sender \
//!     --local=127.0.0.1:7411 --peer=127.0.0.1:7410 --count=10
//!
//! isolation-smoke --transport=uds --role=receiver \
//!     --local-id=01020304 --base-dir=/tmp/zds-test --count=10
//!
//! isolation-smoke --transport=uds --role=echo --local-id=80 \
//!     --base-dir=/run/zerodds-uds --socket-mode=660 --count=50
//!
//! isolation-smoke --transport=uds --role=ping --local-id=81 --peer-id=80 \
//!     --base-dir=/run/zerodds-uds --socket-mode=660 --count=50
//! ```
//!
//! # Roles
//! - `sender` / `receiver`: one direction, `--count` frames.
//! - `echo`: receives `--count` datagrams and sends each back to the
//!   locator it came from.
//! - `ping`: sends `--count` datagrams to `--peer-id`/`--peer`, one at a
//!   time, and checks that each comes back unchanged from that peer; the
//!   first send waits up to 10 s for the peer's socket to appear. Together
//!   with `echo` this carries datagrams in both directions
//!   (zerodds-uds-transport-1.0 §5.2).
//!
//! `--socket-mode=OCTAL` (UDS only) sets the mode of the socket file;
//! `--hold-ms=N` keeps the transport bound N ms after a successful run, so
//! that an observer can see the socket file.
//!
//! # Return-Codes
//! - `0`: success (all expected frames passed through).
//! - `1`: Config-/Parse error.
//! - `2`: Transport-Bind error.
//! - `3`: send/recv error during the test.
//! - `4`: payload mismatch (receiver read unexpected content).

#![allow(clippy::print_stdout, clippy::print_stderr)]
// isolation-smoke uses POSIX-only transports (UDS + POSIX-SHM). The
// entire file is therefore cfg(unix)-gated; on Windows it yields
// binary gives a hint exit.
#![cfg_attr(not(unix), allow(unused_imports, dead_code, unused_variables))]

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    eprintln!("isolation-smoke is POSIX-only (uses UDS + POSIX-SHM transports).");
    std::process::ExitCode::from(2)
}

#[cfg(unix)]
use std::env;
#[cfg(unix)]
use std::net::Ipv4Addr;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::process::ExitCode;
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use zerodds_rtps::wire_types::Locator;
#[cfg(unix)]
use zerodds_transport::Transport;
#[cfg(unix)]
use zerodds_transport_shm::{PosixShmTransport, ShmConfig};
#[cfg(unix)]
use zerodds_transport_udp::UdpTransport;
#[cfg(unix)]
use zerodds_transport_uds::{UdsConfig, UdsTransport};

#[cfg(unix)]
const PAYLOAD_PREFIX: &[u8] = b"zerodds-iso-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(unix)]
enum Role {
    Sender,
    Receiver,
    Echo,
    Ping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(unix)]
enum TransportKind {
    Udp,
    Shm,
    Uds,
    #[cfg(target_os = "linux")]
    UdsAbstract,
    // TCP omitted: TcpTransport needs an accept-loop thread with
    // shared Arc<TcpTransport> — refactor in a follow-up. Smoke-tests
    // for TCP run via the existing crate integration tests.
}

#[cfg(unix)]
fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let cfg = match parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config error: {e}");
            return ExitCode::from(1);
        }
    };
    eprintln!(
        "smoke role={:?} transport={:?} count={}",
        cfg.role, cfg.transport, cfg.count
    );
    match run(&cfg) {
        Ok(()) => ExitCode::from(0),
        Err(code) => ExitCode::from(code),
    }
}

#[derive(Debug, Clone)]
#[cfg(unix)]
struct Cfg {
    role: Role,
    transport: TransportKind,
    count: usize,
    payload_size: usize,
    // Transport-specific:
    local_addr: Option<String>,
    peer_addr: Option<String>,
    local_id: Option<[u8; 16]>,
    peer_id: Option<[u8; 16]>,
    base_dir: Option<PathBuf>,
    abstract_prefix: Option<String>,
    socket_mode: Option<u32>,
    hold: Duration,
}

#[cfg(unix)]
fn parse_args(args: &[String]) -> Result<Cfg, String> {
    let mut cfg = Cfg {
        role: Role::Receiver,
        transport: TransportKind::Udp,
        count: 10,
        payload_size: 64,
        local_addr: None,
        peer_addr: None,
        local_id: None,
        peer_id: None,
        base_dir: None,
        abstract_prefix: None,
        socket_mode: None,
        hold: Duration::ZERO,
    };
    for arg in args.iter().skip(1) {
        let Some((k, v)) = arg.strip_prefix("--").and_then(|a| a.split_once('=')) else {
            return Err(format!("bad arg: {arg}"));
        };
        match k {
            "transport" => {
                cfg.transport = match v {
                    "udp" => TransportKind::Udp,
                    "shm" => TransportKind::Shm,
                    "uds" => TransportKind::Uds,
                    #[cfg(target_os = "linux")]
                    "uds-abstract" => TransportKind::UdsAbstract,
                    _ => return Err(format!("unknown transport: {v}")),
                };
            }
            "role" => {
                cfg.role = match v {
                    "sender" => Role::Sender,
                    "receiver" => Role::Receiver,
                    "echo" => Role::Echo,
                    "ping" => Role::Ping,
                    _ => return Err(format!("bad role: {v}")),
                };
            }
            "count" => cfg.count = v.parse().map_err(|e| format!("count: {e}"))?,
            "payload-size" => {
                cfg.payload_size = v.parse().map_err(|e| format!("payload-size: {e}"))?;
            }
            "local" => cfg.local_addr = Some(v.to_string()),
            "peer" => cfg.peer_addr = Some(v.to_string()),
            "local-id" => cfg.local_id = Some(parse_hex_id(v)?),
            "peer-id" => cfg.peer_id = Some(parse_hex_id(v)?),
            "base-dir" => cfg.base_dir = Some(validate_base_dir(v)?),
            "abstract-prefix" => cfg.abstract_prefix = Some(v.to_string()),
            "socket-mode" => {
                let mode = u32::from_str_radix(v, 8)
                    .map_err(|e| format!("socket-mode: {v:?} is no octal mode: {e}"))?;
                cfg.socket_mode = Some(mode);
            }
            "hold-ms" => {
                cfg.hold = Duration::from_millis(v.parse().map_err(|e| format!("hold-ms: {e}"))?);
            }
            _ => return Err(format!("unknown flag: {k}")),
        }
    }
    if cfg.socket_mode.is_some() && cfg.transport != TransportKind::Uds {
        return Err("--socket-mode applies to --transport=uds only".to_string());
    }
    if matches!(cfg.role, Role::Echo | Role::Ping) && cfg.transport == TransportKind::Shm {
        return Err(
            "--role=echo and --role=ping need a transport that answers the source \
                    locator; shm opens one ring per direction"
                .to_string(),
        );
    }
    Ok(cfg)
}

/// Parses a 16-byte locator hex string.
///
/// Accepts two formats explicitly, to avoid Silent-Pad typos
/// (phase2-0-smells #11):
/// - **Exactly 32 hex chars** — full 16-byte ID (no transformation).
/// - **Exactly with a `0x` prefix OR a length that is a multiple of 2
///   and <= 32** — the input is left-padded with `0` bytes
///   padded to 16 bytes; the user sees that. For `--local-id=1`
///   etc. (short test IDs) the workflow remains.
///
/// Rejected are: odd-length strings ("abc"), non-hex chars,
/// empty string, > 32 chars. Unlike before, typos
/// like `--local-id=abz` are not silently padded to `00…00abz` +
/// error only at hex_nibble — the error now comes from
/// "length/hex check" directly.
#[cfg(unix)]
fn parse_hex_id(s: &str) -> Result<[u8; 16], String> {
    if s.is_empty() {
        return Err("hex id is empty".to_string());
    }
    if s.len() > 32 {
        return Err(format!("hex id too long ({} > 32)", s.len()));
    }
    if !s.len().is_multiple_of(2) {
        return Err(format!(
            "hex id has odd length {} — wrap with leading 0 if this was intentional",
            s.len()
        ));
    }
    // All characters must be hex — check now instead of later
    // implicitly via hex_nibble. That makes the error self-explanatory.
    for (i, b) in s.bytes().enumerate() {
        if hex_nibble(b).is_err() {
            return Err(format!(
                "hex id: non-hex char at offset {i} ({:?})",
                b as char
            ));
        }
    }
    let padded = format!("{s:0>32}");
    let mut out = [0u8; 16];
    for (i, chunk) in padded.as_bytes().chunks_exact(2).enumerate() {
        // Unwrap is safe — we validated all chars beforehand.
        out[i] = (hex_nibble(chunk[0]).map_err(|e| e.to_string())? << 4)
            | hex_nibble(chunk[1]).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Symlink guard for `--base-dir` (phase2-0-security Sec-Med-4).
/// Refuses to accept symlinks as base-dir, so that the
/// binary does not write into foreign paths. If the path does not exist,
/// we allow it (socket/segment is created under our control
/// created).
#[cfg(unix)]
fn validate_base_dir(v: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(v);
    match std::fs::symlink_metadata(&p) {
        Ok(m) if m.file_type().is_symlink() => Err(format!("base-dir is a symlink (refused): {v}")),
        Ok(m) if !m.is_dir() => Err(format!("base-dir exists but is not a directory: {v}")),
        Ok(_) => Ok(p),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(p),
        Err(e) => Err(format!("base-dir stat: {e}")),
    }
}

#[cfg(unix)]
fn hex_nibble(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("bad hex nibble: {b:#x}")),
    }
}

#[cfg(unix)]
fn parse_socket(s: &str) -> Result<(Ipv4Addr, u16), String> {
    let (a, p) = s
        .rsplit_once(':')
        .ok_or_else(|| format!("expected host:port, got {s}"))?;
    let ip: Ipv4Addr = a.parse().map_err(|e| format!("ip: {e}"))?;
    let port: u16 = p.parse().map_err(|e| format!("port: {e}"))?;
    Ok((ip, port))
}

#[cfg(unix)]
fn run(cfg: &Cfg) -> Result<(), u8> {
    match cfg.transport {
        TransportKind::Udp => run_udp(cfg),
        TransportKind::Shm => run_shm(cfg),
        TransportKind::Uds => run_uds(cfg),
        #[cfg(target_os = "linux")]
        TransportKind::UdsAbstract => run_uds_abstract(cfg),
    }
}

#[cfg(unix)]
fn run_udp(cfg: &Cfg) -> Result<(), u8> {
    let (lip, lport) = parse_socket(
        cfg.local_addr
            .as_ref()
            .ok_or_else(|| err("udp needs --local=ip:port", 1))?,
    )
    .map_err(|e| err(&e, 1))?;
    let t = UdpTransport::bind_v4(lip, lport).map_err(|e| err(&format!("{e}"), 2))?;
    let t = if cfg.role == Role::Sender {
        t
    } else {
        t.with_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| err(&format!("{e}"), 2))?
    };
    let peer = || -> Result<Locator, u8> {
        let (pip, pport) = parse_socket(
            cfg.peer_addr
                .as_ref()
                .ok_or_else(|| err("udp sender and ping need --peer=ip:port", 1))?,
        )
        .map_err(|e| err(&e, 1))?;
        Ok(Locator::udp_v4(pip.octets(), u32::from(pport)))
    };
    match cfg.role {
        Role::Receiver => recv_loop(&t, cfg),
        Role::Sender => send_loop(&t, &peer()?, cfg),
        Role::Echo => echo_loop(&t, cfg),
        Role::Ping => ping_loop(&t, &peer()?, cfg),
    }?;
    hold(cfg);
    Ok(())
}

#[cfg(unix)]
fn run_shm(cfg: &Cfg) -> Result<(), u8> {
    let local_id = cfg
        .local_id
        .ok_or_else(|| err("shm needs --local-id=HEX", 1))?;
    let peer_id = cfg
        .peer_id
        .ok_or_else(|| err("shm needs --peer-id=HEX", 1))?;
    let base = cfg
        .base_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("/tmp/zerodds/shm"));
    let shm_cfg = ShmConfig {
        capacity: 1 << 16,
        flink_dir: base,
        max_datagram: 4096,
        recv_timeout: Some(Duration::from_secs(10)),
    };
    match cfg.role {
        Role::Receiver => {
            // Consumer: wait for owner to create segment.
            let start = std::time::Instant::now();
            let t = loop {
                match PosixShmTransport::open_consumer(local_id, peer_id, shm_cfg.clone()) {
                    Ok(t) => break t,
                    Err(_) if start.elapsed() < Duration::from_secs(5) => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(e) => return Err(err(&format!("{e}"), 2)),
                }
            };
            recv_loop(&t, cfg)
        }
        Role::Sender => {
            let t = PosixShmTransport::open_owner(local_id, peer_id, shm_cfg)
                .map_err(|e| err(&format!("{e}"), 2))?;
            let peer = Locator::shm(peer_id);
            // Small delay so consumer has time to open.
            thread::sleep(Duration::from_millis(200));
            send_loop(&t, &peer, cfg)
        }
        // parse_args refuses these roles for shm.
        Role::Echo | Role::Ping => Err(err("shm has no echo or ping role", 1)),
    }
}

#[cfg(unix)]
fn run_uds(cfg: &Cfg) -> Result<(), u8> {
    let local_id = cfg
        .local_id
        .ok_or_else(|| err("uds needs --local-id=HEX", 1))?;
    let base = cfg
        .base_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("/tmp/zerodds/uds"));
    let uds_cfg = UdsConfig {
        base_dir: base,
        max_datagram: 8192,
        recv_timeout: Some(Duration::from_secs(10)),
        socket_mode: cfg.socket_mode,
        ..UdsConfig::default()
    };
    let t = UdsTransport::bind(local_id, uds_cfg).map_err(|e| err(&format!("{e}"), 2))?;
    let peer = || -> Result<Locator, u8> {
        cfg.peer_id
            .map(Locator::uds)
            .ok_or_else(|| err("uds sender and ping need --peer-id=HEX", 1))
    };
    match cfg.role {
        Role::Receiver => recv_loop(&t, cfg),
        Role::Sender => send_loop(&t, &peer()?, cfg),
        Role::Echo => echo_loop(&t, cfg),
        Role::Ping => ping_loop(&t, &peer()?, cfg),
    }?;
    hold(cfg);
    Ok(())
}

#[cfg(target_os = "linux")]
#[cfg(unix)]
fn run_uds_abstract(cfg: &Cfg) -> Result<(), u8> {
    use zerodds_transport_uds::abstract_dgram::{
        AbstractDgramConfig, UdsAbstractDgramTransport, UdsAddress,
    };
    let local_id = cfg
        .local_id
        .ok_or_else(|| err("uds-abstract needs --local-id=HEX", 1))?;
    let prefix = cfg
        .abstract_prefix
        .clone()
        .unwrap_or_else(|| "zerodds".to_string());
    let abs_cfg = AbstractDgramConfig {
        address: UdsAddress::Abstract { prefix },
        max_datagram: 8192,
        recv_timeout: Some(Duration::from_secs(10)),
        ..AbstractDgramConfig::default()
    };
    let t =
        UdsAbstractDgramTransport::bind(local_id, abs_cfg).map_err(|e| err(&format!("{e}"), 2))?;
    let peer = || -> Result<Locator, u8> {
        cfg.peer_id
            .map(Locator::uds)
            .ok_or_else(|| err("uds-abstract sender and ping need --peer-id=HEX", 1))
    };
    match cfg.role {
        Role::Receiver => recv_loop(&t, cfg),
        Role::Sender => send_loop(&t, &peer()?, cfg),
        Role::Echo => echo_loop(&t, cfg),
        Role::Ping => ping_loop(&t, &peer()?, cfg),
    }?;
    hold(cfg);
    Ok(())
}

#[cfg(unix)]
fn send_loop<T: Transport>(t: &T, peer: &Locator, cfg: &Cfg) -> Result<(), u8> {
    // Brief delay so the receiver has time to bind.
    thread::sleep(Duration::from_millis(100));
    for i in 0u32..(cfg.count as u32) {
        let buf = frame(i, cfg);
        if let Err(e) = t.send(peer, &buf) {
            eprintln!("send[{i}] failed: {e}");
            return Err(3);
        }
    }
    eprintln!("sender: {} frames sent", cfg.count);
    Ok(())
}

#[cfg(unix)]
fn recv_loop<T: Transport>(t: &T, cfg: &Cfg) -> Result<(), u8> {
    let mut got = 0usize;
    while got < cfg.count {
        match t.recv() {
            Ok(dg) => {
                if !dg.data.starts_with(PAYLOAD_PREFIX) {
                    eprintln!(
                        "recv[{got}] payload mismatch: first {} bytes = {:?}",
                        PAYLOAD_PREFIX.len().min(dg.data.len()),
                        &dg.data[..PAYLOAD_PREFIX.len().min(dg.data.len())],
                    );
                    return Err(4);
                }
                got += 1;
            }
            Err(e) => {
                eprintln!("recv[{got}] failed: {e}");
                return Err(3);
            }
        }
    }
    eprintln!("receiver: {got} frames got");
    Ok(())
}

/// The payload of frame `i`: the prefix, `i` big endian, padding up to
/// `--payload-size`.
#[cfg(unix)]
fn frame(i: u32, cfg: &Cfg) -> Vec<u8> {
    let pad = vec![0xAB; cfg.payload_size.saturating_sub(PAYLOAD_PREFIX.len() + 4)];
    let mut buf = Vec::with_capacity(cfg.payload_size);
    buf.extend_from_slice(PAYLOAD_PREFIX);
    buf.extend_from_slice(&i.to_be_bytes());
    buf.extend_from_slice(&pad);
    buf
}

/// The locator as `kind:hex-address`, for the log lines.
#[cfg(unix)]
fn locator_text(l: &Locator) -> String {
    let hex: String = l.address.iter().map(|b| format!("{b:02x}")).collect();
    format!("{:?}:{hex}", l.kind)
}

/// Receives `--count` datagrams and sends each back to the locator it came
/// from. A datagram without a source locator cannot be answered and fails
/// the run.
#[cfg(unix)]
fn echo_loop<T: Transport>(t: &T, cfg: &Cfg) -> Result<(), u8> {
    let mut last = Locator::INVALID;
    for i in 0..cfg.count {
        let dg = t
            .recv()
            .map_err(|e| err(&format!("echo recv[{i}] failed: {e}"), 3))?;
        if dg.source == Locator::INVALID {
            return Err(err(
                &format!("echo recv[{i}]: datagram without a source locator"),
                3,
            ));
        }
        t.send(&dg.source, &dg.data)
            .map_err(|e| err(&format!("echo send[{i}] failed: {e}"), 3))?;
        last = dg.source;
    }
    eprintln!(
        "echo: {} datagrams echoed to {}",
        cfg.count,
        locator_text(&last)
    );
    Ok(())
}

/// How long the first send of `ping` waits for the peer's socket.
#[cfg(unix)]
const PEER_WAIT: Duration = Duration::from_secs(10);

/// Sends `--count` datagrams to `peer`, one at a time, and checks that each
/// comes back unchanged from `peer`. The first send is repeated while the
/// peer's socket is missing, at most [`PEER_WAIT`]; every other error ends
/// the run with its message.
#[cfg(unix)]
fn ping_loop<T: Transport>(t: &T, peer: &Locator, cfg: &Cfg) -> Result<(), u8> {
    for i in 0..(cfg.count as u32) {
        let buf = frame(i, cfg);
        let start = std::time::Instant::now();
        loop {
            match t.send(peer, &buf) {
                Ok(()) => break,
                Err(zerodds_transport::SendError::Io { message })
                    if i == 0
                        && message == "uds: destination socket missing"
                        && start.elapsed() < PEER_WAIT =>
                {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(err(&format!("ping send[{i}] failed: {e}"), 3)),
            }
        }
        let dg = t
            .recv()
            .map_err(|e| err(&format!("ping recv[{i}] failed: {e}"), 3))?;
        if dg.source != *peer {
            return Err(err(
                &format!(
                    "ping recv[{i}]: echo from {}, expected {}",
                    locator_text(&dg.source),
                    locator_text(peer)
                ),
                4,
            ));
        }
        if dg.data[..] != buf[..] {
            return Err(err(
                &format!(
                    "ping recv[{i}]: echo of {} bytes differs from the {} bytes sent",
                    dg.data.len(),
                    buf.len()
                ),
                4,
            ));
        }
    }
    eprintln!(
        "ping: {} echoes received from {}",
        cfg.count,
        locator_text(peer)
    );
    Ok(())
}

/// Keeps the transport bound `--hold-ms` after a successful run.
#[cfg(unix)]
fn hold(cfg: &Cfg) {
    if !cfg.hold.is_zero() {
        thread::sleep(cfg.hold);
    }
}

#[cfg(unix)]
fn err(msg: &str, code: u8) -> u8 {
    eprintln!("smoke error: {msg}");
    code
}
