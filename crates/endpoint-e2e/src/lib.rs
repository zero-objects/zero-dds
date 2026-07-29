// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-endpoint-e2e`. Safety classification: **STANDARD**.
//!
//! Live cross-language ping-pong end-to-end harness.
//!
//! A native endpoint **app** (the star) — built from `idlc`-generated types plus
//! the language's endpoint SDK — connects over a real UDP socket, sends a typed
//! `Ping`, and prints the `Pong` it gets back. This crate is the **Rust peer**:
//! it binds UDP, waits for the app's XRCE `WRITE_DATA` frame, decodes the sample
//! with the real ZeroDDS CDR codec ([`zerodds_cdr`]), and replies a `Pong`.
//!
//! The peer is **language-agnostic** — every endpoint language reuses it; only
//! the per-language app + build/run differs (in the `tests/`). Boundary (honest):
//! the native side is wire/TypeSupport + XRCE framing + a datagram; RTPS,
//! discovery, and QoS are the DDS participant's job (here, this Rust peer).

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::collections::HashSet;
use std::net::{SocketAddr, UdpSocket};
use std::process::Child;
use std::time::{Duration, Instant};

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_xrce::submessages::{AckNackPayload, HeartbeatPayload};

// Re-export the reliable reference types so tests + language harnesses have a
// single import surface (`zerodds_endpoint_e2e::…`).
pub use zerodds_xrce::{ReliableConfig, ReliableStreamState, SerialNumber16, StreamId};

/// The shared topic types every app uses. `@final` = inline XCDR2, no DHEADER.
pub const IDL: &str = "\
@final struct Ping { long seq; string msg; };
@final struct Pong { long seq; string reply; };";

/// The exact `Ping` every app sends (so the peer can assert it).
pub const PING_SEQ: u32 = 1;
/// The exact `Ping.msg` every app sends.
pub const PING_MSG: &str = "hello from app";

// XRCE WRITE_DATA framing — byte-identical to every endpoint SDK's `writeFrame`
// (session, stream, seq LE) + (submessage id, flags, length LE) + sample.
const XRCE_SM_WRITE_DATA: u8 = 0x07;

/// Wraps a sample in an XRCE `WRITE_DATA` frame.
#[must_use]
pub fn xrce_frame(seq: u16, sample: &[u8]) -> Vec<u8> {
    let n = sample.len() as u16;
    let mut out = vec![
        0x80,
        0x01,
        seq as u8,
        (seq >> 8) as u8,
        XRCE_SM_WRITE_DATA,
        0x03,
        n as u8,
        (n >> 8) as u8,
    ];
    out.extend_from_slice(sample);
    out
}

/// The sample body inside a received XRCE `WRITE_DATA` frame.
#[must_use]
pub fn xrce_unframe(frame: &[u8]) -> Option<&[u8]> {
    if frame.len() < 8 || frame[4] != XRCE_SM_WRITE_DATA {
        return None;
    }
    Some(&frame[8..])
}

/// The bound Rust peer: hands the app its UDP port, runs the ping-pong.
pub struct Peer {
    sock: UdpSocket,
    /// The UDP port the app must send its `Ping` to.
    pub port: u16,
}

/// Binds the Rust peer on loopback. `None` if binding fails (unusual).
#[must_use]
pub fn bind_peer() -> Option<Peer> {
    let sock = UdpSocket::bind("127.0.0.1:0").ok()?;
    let port = sock.local_addr().ok()?.port();
    Some(Peer { sock, port })
}

/// Completes one ping-pong against an already-spawned app `child`: waits for the
/// app's XRCE `Ping`, decodes + asserts it, replies a `Pong`, then asserts the
/// app printed `PONG seq=<n> reply=pong:<msg>` on stdout. `label` names the case
/// in failure messages.
///
/// # Panics
/// Panics (fails the test) if the app does not send the exact `Ping` or does not
/// print the exact `Pong`.
pub fn ping_pong(peer: &Peer, child: Child, label: &str) {
    peer.sock
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("set timeout");
    let mut buf = [0u8; 4096];
    let (n, app_addr) = peer.sock.recv_from(&mut buf).expect("recv ping frame");
    let sample = xrce_unframe(&buf[..n]).unwrap_or_else(|| panic!("{label}: not an XRCE frame"));

    // Decode the Ping with the real ZeroDDS CDR codec.
    let mut r = BufferReader::new(sample, Endianness::Little).xcdr2();
    let seq = r.read_u32().expect("ping seq");
    let msg = r.read_string().expect("ping msg");
    assert_eq!(seq, PING_SEQ, "{label}: ping seq");
    assert_eq!(msg, PING_MSG, "{label}: ping msg");

    // Reply with a Pong (echo seq, different reply text), XRCE-framed.
    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    w.write_u32(seq).expect("pong seq");
    w.write_string(&format!("pong:{msg}")).expect("pong reply");
    peer.sock
        .send_to(&xrce_frame(1, &w.into_bytes()), app_addr)
        .expect("send pong frame");

    let out = child.wait_with_output().expect("app wait");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let want = format!("PONG seq={PING_SEQ} reply=pong:{PING_MSG}");
    assert!(
        stdout.contains(&want),
        "{label}: app did not print `{want}`\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// =====================================================================
// Reliable stream (XRCE `stream_id >= 128`, spec §8.4.10/§8.4.11)
// =====================================================================
//
// The reliable peer is the RECEIVER: it drives the reference
// [`ReliableStreamState`] from `zerodds-xrce`, injects loss, reorders +
// de-duplicates incoming samples, and replies `ACKNACK` so the sending app must
// retransmit. All three reliable frames are byte-identical to the C SDK
// (`endpoints/c`: `zdw_xrce_write_frame` / `zdw_xrce_acknack_frame` /
// `zdw_xrce_heartbeat_read`) and to `crates/xrce`.
//
// Frame layout (8-byte header + body), all little-endian:
//   [0] session (0x80), [1] stream (reliable id, 0x80),
//   [2..4] sequence LE, [4] submessage id, [5] flags, [6..8] body-len LE, [8..] body
//   WRITE_DATA id=0x07 flags=0x03 body=sample; header seq = the sample's RFC-1982 seq
//   ACKNACK   id=0x0A flags=0x01 body(5)= first_unacked i16 LE, nack[0], nack[1], stream
//   HEARTBEAT id=0x0B flags=0x01 body(5)= first i16 LE, last i16 LE, stream

/// XRCE session id (no client key).
const XRCE_SESSION_NOKEY: u8 = 0x80;
/// Reliable stream id (bit 7 set — RFC-1982 window, spec §8.3.2.2).
pub const RELIABLE_STREAM_ID: u8 = 0x80;
const XRCE_SM_ACKNACK: u8 = 0x0A;
const XRCE_SM_HEARTBEAT: u8 = 0x0B;
/// WRITE_DATA flags (E-flag little-endian + data-present), matches the C SDK.
const XRCE_WRITE_FLAGS: u8 = 0x03;
/// Control-message flags: E-flag little-endian only, matches `zdw_xrce_acknack_frame`.
const XRCE_FLAG_E_LE: u8 = 0x01;

/// Builds a reliable `WRITE_DATA` frame. The 2-byte stream-header sequence
/// carries the RFC-1982 sample sequence number.
#[must_use]
pub fn reliable_write_frame(seq: u16, sample: &[u8]) -> Vec<u8> {
    let s = seq.to_le_bytes();
    let n = (sample.len() as u16).to_le_bytes();
    let mut out = vec![
        XRCE_SESSION_NOKEY,
        RELIABLE_STREAM_ID,
        s[0],
        s[1],
        XRCE_SM_WRITE_DATA,
        XRCE_WRITE_FLAGS,
        n[0],
        n[1],
    ];
    out.extend_from_slice(sample);
    out
}

/// Parses a reliable `WRITE_DATA` frame into `(seq, sample)`.
#[must_use]
pub fn reliable_unframe(frame: &[u8]) -> Option<(u16, &[u8])> {
    if frame.len() < 8 || frame[4] != XRCE_SM_WRITE_DATA {
        return None;
    }
    Some((u16::from_le_bytes([frame[2], frame[3]]), &frame[8..]))
}

/// Builds an `ACKNACK` frame (byte-identical to `zdw_xrce_acknack_frame`).
#[must_use]
pub fn acknack_frame(ack: AckNackPayload) -> Vec<u8> {
    let f = ack.first_unacked_seq_num.to_le_bytes();
    vec![
        XRCE_SESSION_NOKEY,
        RELIABLE_STREAM_ID,
        0,
        0,
        XRCE_SM_ACKNACK,
        XRCE_FLAG_E_LE,
        5,
        0,
        f[0],
        f[1],
        ack.nack_bitmap[0],
        ack.nack_bitmap[1],
        ack.stream_id,
    ]
}

/// Parses an `ACKNACK` frame into an [`AckNackPayload`].
#[must_use]
pub fn parse_acknack(frame: &[u8]) -> Option<AckNackPayload> {
    if frame.len() < 13 || frame[4] != XRCE_SM_ACKNACK {
        return None;
    }
    Some(AckNackPayload {
        first_unacked_seq_num: i16::from_le_bytes([frame[8], frame[9]]),
        nack_bitmap: [frame[10], frame[11]],
        stream_id: frame[12],
    })
}

/// Builds a `HEARTBEAT` frame (mirrors the `zdw_xrce_heartbeat_read` layout).
#[must_use]
pub fn heartbeat_frame(hb: HeartbeatPayload) -> Vec<u8> {
    let f = hb.first_unacked_seq_nr.to_le_bytes();
    let l = hb.last_unacked_seq_nr.to_le_bytes();
    vec![
        XRCE_SESSION_NOKEY,
        RELIABLE_STREAM_ID,
        0,
        0,
        XRCE_SM_HEARTBEAT,
        XRCE_FLAG_E_LE,
        5,
        0,
        f[0],
        f[1],
        l[0],
        l[1],
        hb.stream_id,
    ]
}

/// Parses a `HEARTBEAT` frame into a [`HeartbeatPayload`].
#[must_use]
pub fn parse_heartbeat(frame: &[u8]) -> Option<HeartbeatPayload> {
    if frame.len() < 13 || frame[4] != XRCE_SM_HEARTBEAT {
        return None;
    }
    Some(HeartbeatPayload {
        first_unacked_seq_nr: i16::from_le_bytes([frame[8], frame[9]]),
        last_unacked_seq_nr: i16::from_le_bytes([frame[10], frame[11]]),
        stream_id: frame[12],
    })
}

/// A reliable Rust peer: binds UDP, drives a receiver [`ReliableStreamState`],
/// injects loss, and replies `ACKNACK` so the sending app must retransmit.
pub struct ReliablePeer {
    sock: UdpSocket,
    /// UDP port the app sends its reliable `WRITE_DATA` to.
    pub port: u16,
    /// Drop each n-th distinct incoming sample **once** (forces one retransmit
    /// per victim, so the run still converges). `None` = lossless baseline.
    pub drop_every: Option<usize>,
}

/// Binds the reliable peer on loopback. `drop_every` injects loss. `None` on
/// bind failure (unusual).
#[must_use]
pub fn bind_reliable_peer(drop_every: Option<usize>) -> Option<ReliablePeer> {
    let sock = UdpSocket::bind("127.0.0.1:0").ok()?;
    let port = sock.local_addr().ok()?.port();
    Some(ReliablePeer {
        sock,
        port,
        drop_every,
    })
}

/// Drives the reliable receiver against a sending app: receives `WRITE_DATA`,
/// injects loss per `drop_every`, reorders + de-duplicates via the reference
/// [`ReliableStreamState`], and replies `ACKNACK` on every data packet and on
/// each `HEARTBEAT`. Returns the gap-free, in-order delivered payloads once
/// `expected` have arrived — loss having been recovered by the app's retransmit.
///
/// # Panics
/// Panics (fails the test) if fewer than `expected` samples are delivered
/// gap-free within the timeout.
#[must_use]
pub fn reliable_collect(peer: &ReliablePeer, label: &str, expected: usize) -> Vec<Vec<u8>> {
    let mut state = ReliableStreamState::new(StreamId::BUILTIN_RELIABLE, ReliableConfig::default());
    let mut delivered: Vec<Vec<u8>> = Vec::new();
    let mut dropped_once: HashSet<u16> = HashSet::new();
    let mut recv_count = 0usize;
    let mut last_addr: Option<SocketAddr> = None;

    peer.sock
        .set_read_timeout(Some(Duration::from_millis(200)))
        .expect("set timeout");
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut buf = [0u8; 8192];

    while delivered.len() < expected {
        assert!(
            Instant::now() < deadline,
            "{label}: only {}/{expected} samples delivered before timeout",
            delivered.len()
        );
        let (n, app_addr) = match peer.sock.recv_from(&mut buf) {
            Ok(x) => x,
            Err(_) => continue, // read timeout → re-check deadline
        };
        last_addr = Some(app_addr);
        let frame = &buf[..n];

        if let Some((seq, sample)) = reliable_unframe(frame) {
            // Loss injection: drop each n-th distinct sample exactly once.
            if let Some(every) = peer.drop_every {
                recv_count += 1;
                if recv_count % every == 0 && dropped_once.insert(seq) {
                    continue; // silently drop → the app must retransmit
                }
            }
            let s = SerialNumber16::new(seq);
            let _ = state.recv_data(s, sample.to_vec());
            for (_seq, payload) in state.drain_in_order() {
                delivered.push(payload);
            }
            // `None` hint: mark every not-yet-received slot in the window as
            // missing, so a clear bit unambiguously means "received". The sender
            // may then safely purge clear-bit samples and retransmit set ones. A
            // hinted ACKNACK would leave beyond-hint slots clear, and the sender
            // would wrongly treat still-in-flight (lost) samples as acked.
            let ack = state.pending_acknack(None);
            let _ = peer.sock.send_to(&acknack_frame(ack), app_addr);
        } else if parse_heartbeat(frame).is_some() {
            let ack = state.pending_acknack(None);
            let _ = peer.sock.send_to(&acknack_frame(ack), app_addr);
        }
    }

    // Final "all-acked" ACKNACKs (base = expected, bitmap = 0) so the app's
    // send window drains and it can exit its retransmit loop.
    if let Some(addr) = last_addr {
        let ack = state.pending_acknack(None);
        for _ in 0..3 {
            let _ = peer.sock.send_to(&acknack_frame(ack), addr);
        }
    }
    delivered
}

/// Runs [`reliable_collect`] against a spawned sending app `child`, then reaps
/// the child (surfacing its stderr on a non-zero exit). Returns the delivered
/// payloads.
///
/// # Panics
/// Panics via [`reliable_collect`] if delivery is incomplete.
pub fn reliable_receive(
    peer: &ReliablePeer,
    child: Child,
    label: &str,
    expected: usize,
) -> Vec<Vec<u8>> {
    let got = reliable_collect(peer, label, expected);
    let out = child.wait_with_output().expect("app wait");
    if !out.status.success() {
        eprintln!(
            "{label}: app exited {:?}\nstderr: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    got
}
