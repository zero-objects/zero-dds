// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TSN roundtrip latency bench — AF_PACKET (DDS-TSN Annex A, EtherType
//! 0x88B5) against a UDP baseline over the same link.
//!
//! Measures the **host transport path latency** (sender→echo→sender) of a
//! single request/response. IMPORTANT: on a pure software link
//! (veth) or an ordinary NIC this is NOT the bounded latency
//! of a TSN network — the TSN advantage arises in the switch (taprio/ETF +
//! gPTP), not in the host socket. The bench serves as a **baseline +
//! regression guard** for the AF_PACKET path and as an honest
//! comparison to UDP on the same link.
//!
//! Modes (Linux, feature `live`, root/CAP_NET_RAW):
//! ```text
//!   tsn-pong  <iface>                       # AF_PACKET echo server
//!   tsn-ping  <iface> <peer-mac> <count>    # AF_PACKET latency client → JSON
//!   udp-pong  <bind-addr>                    # UDP echo server
//!   udp-ping  <bind-addr> <peer-addr> <count> # UDP latency client → JSON
//! ```
//! `peer-mac` as `aa:bb:cc:dd:ee:ff`. The client prints a JSON object
//! with p50/p90/p99/max/jitter (ns) to stdout, compatible with the
//! runners under `tests/perf/`.

#![cfg(all(feature = "live", target_os = "linux"))]
// CLI bench tool: stdout carries the JSON result, stderr the diagnostics.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use zerodds_rtps::wire_types::Locator;
use zerodds_transport::Transport;
use zerodds_transport_tsn::TsnTransport;

const PAYLOAD: usize = 64; // incl. 4-byte sequence number.
const WARMUP: usize = 200;
const VID: u16 = 0; // untagged: veth strips VLAN anyway (see socket docs).
const PCP: u8 = 0;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let rc = match mode {
        "tsn-pong" => tsn_pong(&args),
        "tsn-ping" => tsn_ping(&args),
        "udp-pong" => udp_pong(&args),
        "udp-ping" => udp_ping(&args),
        _ => {
            eprintln!(
                "usage: tsn_latency <tsn-pong IFACE | tsn-ping IFACE PEERMAC COUNT | \
                 udp-pong ADDR | udp-ping ADDR PEER COUNT>"
            );
            Err("unknown mode".into())
        }
    };
    if let Err(e) = rc {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

type R = Result<(), Box<dyn std::error::Error>>;

fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let mut out = [0u8; 6];
    let mut it = s.split(':');
    for slot in &mut out {
        *slot = u8::from_str_radix(it.next()?, 16).ok()?;
    }
    Some(out)
}

// ------------------------------------------------------------------ TSN.

fn tsn_pong(args: &[String]) -> R {
    let iface = args.get(2).ok_or("iface missing")?;
    let t = TsnTransport::bind(iface, VID, PCP, Some(Duration::from_millis(200)))?;
    eprintln!("[tsn-pong] echo on {iface}, mac={:02x?}", t.local_mac());
    loop {
        match t.recv() {
            Ok(d) => {
                let _ = t.send(&d.source, &d.data);
            }
            Err(_) => continue, // Timeout → keep waiting.
        }
    }
}

fn tsn_ping(args: &[String]) -> R {
    let iface = args.get(2).ok_or("iface missing")?;
    let peer = parse_mac(args.get(3).ok_or("peer-mac missing")?).ok_or("peer-mac invalid")?;
    let count: usize = args.get(4).ok_or("count missing")?.parse()?;

    let t = TsnTransport::bind(iface, VID, PCP, Some(Duration::from_millis(500)))?;
    let dest = Locator::tsn(peer, VID);
    let mut buf = [0u8; PAYLOAD];

    let mut samples = Vec::with_capacity(count);
    let mut lost = 0u64;
    for i in 0..(count + WARMUP) {
        let seq = i as u32;
        buf[..4].copy_from_slice(&seq.to_be_bytes());
        let t0 = Instant::now();
        t.send(&dest, &buf)?;
        match wait_echo_tsn(&t, seq) {
            Some(()) => {
                let rtt = t0.elapsed().as_nanos() as u64;
                if i >= WARMUP {
                    samples.push(rtt);
                }
            }
            None => lost += 1,
        }
    }
    print_json("tsn-af_packet", iface, &mut samples, lost);
    Ok(())
}

/// Waits for the echo frame with a matching sequence number. `Some(())` =
/// received, `None` = timeout/lost.
fn wait_echo_tsn(t: &TsnTransport, seq: u32) -> Option<()> {
    loop {
        match t.recv() {
            Ok(d) if d.data.len() >= 4 && d.data[..4] == seq.to_be_bytes() => return Some(()),
            Ok(_) => continue, // foreign/stale frame.
            Err(_) => return None,
        }
    }
}

// ------------------------------------------------------------------ UDP.

fn udp_pong(args: &[String]) -> R {
    let addr = args.get(2).ok_or("bind-addr missing")?;
    let sock = UdpSocket::bind(addr)?;
    eprintln!("[udp-pong] echo on {addr}");
    let mut buf = [0u8; 2048];
    loop {
        if let Ok((n, src)) = sock.recv_from(&mut buf) {
            let _ = sock.send_to(&buf[..n], src);
        }
    }
}

fn udp_ping(args: &[String]) -> R {
    let bind = args.get(2).ok_or("bind-addr missing")?;
    let peer = args.get(3).ok_or("peer-addr missing")?;
    let count: usize = args.get(4).ok_or("count missing")?.parse()?;

    let sock = UdpSocket::bind(bind)?;
    sock.set_read_timeout(Some(Duration::from_millis(500)))?;
    sock.connect(peer)?;
    let mut buf = [0u8; PAYLOAD];
    let mut rbuf = [0u8; 2048];

    let mut samples = Vec::with_capacity(count);
    let mut lost = 0u64;
    for i in 0..(count + WARMUP) {
        let seq = i as u32;
        buf[..4].copy_from_slice(&seq.to_be_bytes());
        let t0 = Instant::now();
        sock.send(&buf)?;
        match sock.recv(&mut rbuf) {
            Ok(n) if n >= 4 && rbuf[..4] == seq.to_be_bytes() => {
                if i >= WARMUP {
                    samples.push(t0.elapsed().as_nanos() as u64);
                }
            }
            _ => lost += 1,
        }
    }
    print_json("udp", peer, &mut samples, lost);
    Ok(())
}

// ------------------------------------------------------------- Reporting.

fn pct(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn print_json(transport: &str, target: &str, samples: &mut [u64], lost: u64) {
    samples.sort_unstable();
    let n = samples.len();
    let min = samples.first().copied().unwrap_or(0);
    let max = samples.last().copied().unwrap_or(0);
    let p50 = pct(samples, 50.0);
    let p90 = pct(samples, 90.0);
    let p99 = pct(samples, 99.0);
    let mean = if n > 0 {
        samples.iter().sum::<u64>() / n as u64
    } else {
        0
    };
    // Jitter = p99 - p50 (common RTT jitter measure).
    let jitter = p99.saturating_sub(p50);
    println!(
        "{{\"transport\":\"{transport}\",\"target\":\"{target}\",\"samples\":{n},\
         \"lost\":{lost},\"rtt_ns\":{{\"min\":{min},\"mean\":{mean},\"p50\":{p50},\
         \"p90\":{p90},\"p99\":{p99},\"max\":{max},\"jitter_p99_p50\":{jitter}}}}}"
    );
    eprintln!(
        "[{transport}] n={n} lost={lost} p50={p50}ns p99={p99}ns jitter={jitter}ns max={max}ns"
    );
}
