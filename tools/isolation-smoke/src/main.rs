// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Isolation-Smoke-Test Runner (WP 2.0b T4).
//!
//! Ein schmales CLI-Binary das als Sender oder Receiver auf
//! waehlbarem Transport laeuft. Wird von den Isolation-Scripts
//! (`testing/isolation/`) in L1-L4 orchestriert.
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
//! ```
//!
//! # Return-Codes
//! - `0`: Erfolg (alle erwarteten Frames durchgelaufen).
//! - `1`: Config-/Parse-Fehler.
//! - `2`: Transport-Bind-Fehler.
//! - `3`: Send-/Recv-Fehler waehrend des Tests.
//! - `4`: Payload-Mismatch (receiver hat unerwarteten Inhalt gelesen).

#![allow(clippy::print_stdout, clippy::print_stderr)]
// isolation-smoke nutzt POSIX-only Transports (UDS + POSIX-SHM). Das
// gesamte File ist daher cfg(unix)-gated; auf Windows liefert das
// Binary einen Hinweis-Exit.
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
            _ => return Err(format!("unknown flag: {k}")),
        }
    }
    Ok(cfg)
}

/// Parst einen 16-Byte-Locator-Hex-String.
///
/// Akzeptiert zwei Formate explizit, um Silent-Pad-Tippfehler zu
/// vermeiden (phase2-0-smells #11):
/// - **Genau 32 hex chars** — volle 16-Byte-ID (keine Transformation).
/// - **Exakt mit `0x` prefix ODER einer Laenge die Vielfaches von 2
///   und <= 32 ist** — die Eingabe wird nach links mit `0`-Bytes
///   zu 16 Byte aufgefuellt; der User sieht das. Fuer `--local-id=1`
///   etc. (kurze Test-IDs) bleibt der Workflow.
///
/// Abgelehnt werden: odd-length strings ("abc"), non-hex chars,
/// leerer String, > 32 chars. Anders als vorher werden Tippfehler
/// wie `--local-id=abz` nicht silent gepadded zu `00…00abz` +
/// Fehler erst beim hex_nibble — der Fehler kommt jetzt aus
/// "length/hex check" direkt.
#[cfg(unix)]
fn parse_hex_id(s: &str) -> Result<[u8; 16], String> {
    if s.is_empty() {
        return Err("hex id is empty".to_string());
    }
    if s.len() > 32 {
        return Err(format!("hex id too long ({} > 32)", s.len()));
    }
    if s.len() % 2 != 0 {
        return Err(format!(
            "hex id has odd length {} — wrap with leading 0 if this was intentional",
            s.len()
        ));
    }
    // Alle Zeichen muessen hex sein — jetzt checken statt spaeter
    // implizit ueber hex_nibble. Das macht den Fehler selbsterklaerend.
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
        // Unwrap sicher — wir haben alle chars vorher validiert.
        out[i] = (hex_nibble(chunk[0]).map_err(|e| e.to_string())? << 4)
            | hex_nibble(chunk[1]).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Symlink-Guard fuer `--base-dir` (phase2-0-security Sec-Med-4).
/// Weigert sich, Symlinks als base-dir zu akzeptieren, damit das
/// Binary nicht in fremde Pfade schreibt. Existiert der Pfad nicht,
/// erlauben wir ihn (Socket/Segment wird unter unserer Kontrolle
/// angelegt).
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
    let t = if cfg.role == Role::Receiver {
        t.with_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| err(&format!("{e}"), 2))?
    } else {
        t
    };
    match cfg.role {
        Role::Receiver => recv_loop(&t, cfg),
        Role::Sender => {
            let (pip, pport) = parse_socket(
                cfg.peer_addr
                    .as_ref()
                    .ok_or_else(|| err("udp sender needs --peer=ip:port", 1))?,
            )
            .map_err(|e| err(&e, 1))?;
            let peer = Locator::udp_v4(pip.octets(), u32::from(pport));
            send_loop(&t, &peer, cfg)
        }
    }
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
    };
    let t = UdsTransport::bind(local_id, uds_cfg).map_err(|e| err(&format!("{e}"), 2))?;
    match cfg.role {
        Role::Receiver => recv_loop(&t, cfg),
        Role::Sender => {
            let peer_id = cfg
                .peer_id
                .ok_or_else(|| err("uds sender needs --peer-id=HEX", 1))?;
            let peer = Locator::uds(peer_id);
            send_loop(&t, &peer, cfg)
        }
    }
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
        recv_buf: 8192,
        recv_timeout: Some(Duration::from_secs(10)),
    };
    let t =
        UdsAbstractDgramTransport::bind(local_id, abs_cfg).map_err(|e| err(&format!("{e}"), 2))?;
    match cfg.role {
        Role::Receiver => recv_loop(&t, cfg),
        Role::Sender => {
            let peer_id = cfg
                .peer_id
                .ok_or_else(|| err("uds-abstract sender needs --peer-id=HEX", 1))?;
            let peer = Locator::uds(peer_id);
            send_loop(&t, &peer, cfg)
        }
    }
}

#[cfg(unix)]
fn send_loop<T: Transport>(t: &T, peer: &Locator, cfg: &Cfg) -> Result<(), u8> {
    // Brief delay so the receiver has time to bind.
    thread::sleep(Duration::from_millis(100));
    let pad = vec![0xAB; cfg.payload_size.saturating_sub(PAYLOAD_PREFIX.len() + 4)];
    for i in 0u32..(cfg.count as u32) {
        let mut buf = Vec::with_capacity(cfg.payload_size);
        buf.extend_from_slice(PAYLOAD_PREFIX);
        buf.extend_from_slice(&i.to_be_bytes());
        buf.extend_from_slice(&pad);
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

#[cfg(unix)]
fn err(msg: &str, code: u8) -> u8 {
    eprintln!("smoke error: {msg}");
    code
}
