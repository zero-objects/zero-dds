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

// XRCE framing — byte-identical to every endpoint SDK's `writeFrame`
// (session, stream, seq LE) + (submessage id, flags, length LE) + sample.
//
// Direction (DDS-XRCE spec §8.3.5): the two data-carrying submessages are
// direction-specific and MUST NOT be swapped:
//   * WRITE_DATA (id 0x07): endpoint (client) -> hub (agent). The endpoint
//     app builds this; this peer, standing in for the hub, PARSES it
//     ([`xrce_unframe`]).
//   * DATA       (id 0x09): hub (agent) -> endpoint (client). This peer,
//     standing in for the hub, BUILDS it ([`xrce_data_frame`]); the endpoint
//     SDK reader parses it.
const XRCE_SM_WRITE_DATA: u8 = 0x07;
/// XRCE `DATA` submessage id — hub (agent) -> endpoint (client) direction.
const XRCE_SM_DATA: u8 = 0x09;

/// Wraps a sample in an XRCE `WRITE_DATA` frame — the endpoint->hub direction
/// (what an endpoint app sends). Kept for the send side of the wire contract.
#[must_use]
pub fn xrce_frame(seq: u16, sample: &[u8]) -> Vec<u8> {
    frame_with_id(XRCE_SM_WRITE_DATA, seq, sample)
}

/// Wraps a sample in an XRCE `DATA` frame — the hub->endpoint direction. This is
/// what the peer replies (it stands in for the hub/agent); a real XRCE agent
/// pushes samples to a client with `DATA` (id 0x09), never `WRITE_DATA`.
#[must_use]
pub fn xrce_data_frame(seq: u16, sample: &[u8]) -> Vec<u8> {
    frame_with_id(XRCE_SM_DATA, seq, sample)
}

/// Shared framing for both directions (only the submessage id differs).
fn frame_with_id(submessage_id: u8, seq: u16, sample: &[u8]) -> Vec<u8> {
    let n = sample.len() as u16;
    let mut out = vec![
        0x80,
        0x01,
        seq as u8,
        (seq >> 8) as u8,
        submessage_id,
        0x03,
        n as u8,
        (n >> 8) as u8,
    ];
    out.extend_from_slice(sample);
    out
}

/// The sample body inside a `WRITE_DATA` frame received FROM an endpoint (the
/// hub/peer side). Rejects — returns `None`, never panics — a frame that is
/// shorter than the 8-byte header, does not carry `WRITE_DATA` (id 0x07; a
/// `DATA`/0x09 frame is the wrong direction here and is refused), or whose
/// declared body length runs past the datagram (truncated / wrong length).
#[must_use]
pub fn xrce_unframe(frame: &[u8]) -> Option<&[u8]> {
    if frame.len() < 8 || frame[4] != XRCE_SM_WRITE_DATA {
        return None;
    }
    let sm_len = u16::from_le_bytes([frame[6], frame[7]]) as usize;
    if 8 + sm_len > frame.len() {
        return None;
    }
    Some(&frame[8..8 + sm_len])
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

/// Binds the Rust peer on a **fixed** loopback `port` — for the standalone
/// `peer` binary that a separately-launched language app connects to (the test
/// harness uses [`bind_peer`] with an ephemeral port instead). `None` on bind
/// failure.
#[must_use]
pub fn bind_peer_on(port: u16) -> Option<Peer> {
    let sock = UdpSocket::bind(("127.0.0.1", port)).ok()?;
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
    // 90 s, not 30: the endpoint apps compile a real toolchain at test time
    // (kotlinc/swiftc/nim/ocamlopt/…), and a GitHub runner's FIRST cold compile
    // of one of these routinely runs 30-60 s before the app can send its ping.
    // This is the external compiler's cold-start, not a ZeroDDS round-trip.
    // Julia's JIT is heavier still — it passes an even longer timeout directly.
    ping_pong_with_timeout(peer, child, label, Duration::from_secs(90));
}

/// Like [`ping_pong`] but with an explicit peer read-timeout. Interpreted
/// backends with a slow, variable cold start (Julia's first-run JIT of the
/// generated module + `Sockets` on a cold CI depot can exceed the 30 s default)
/// pass a longer timeout — this covers the language runtime's compile latency,
/// which is external to ZeroDDS, NOT a wire round-trip that got slower.
pub fn ping_pong_with_timeout(peer: &Peer, child: Child, label: &str, timeout: Duration) {
    peer.sock
        .set_read_timeout(Some(timeout))
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

    // Reply with a Pong (echo seq, different reply text) in an XRCE `DATA`
    // frame: the peer stands in for the hub/agent, and the hub->endpoint
    // direction is `DATA` (id 0x09), never `WRITE_DATA` (id 0x07). The endpoint
    // SDK reader must accept `DATA` for this to arrive.
    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    w.write_u32(seq).expect("pong seq");
    w.write_string(&format!("pong:{msg}")).expect("pong reply");
    peer.sock
        .send_to(&xrce_data_frame(1, &w.into_bytes()), app_addr)
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

/// Serves one ping-pong exchange against an **already-running external app**
/// (the caller owns the app process, so — unlike [`ping_pong`] — this does not
/// spawn a child or assert on its stdout): waits up to `timeout` for the app's
/// XRCE `Ping`, decodes it, and replies a `Pong` in a `DATA` frame. Used by the
/// standalone `peer` binary.
///
/// On success returns the received `Ping`'s `(seq, msg)`.
///
/// # Errors
/// Returns `Err` if no valid `Ping` arrives within `timeout` or the datagram is
/// not a well-formed XRCE `Ping`.
pub fn serve_ping_pong(peer: &Peer, timeout: Duration) -> Result<(u32, String), String> {
    peer.sock
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("set timeout: {e}"))?;
    let mut buf = [0u8; 4096];
    let (n, app_addr) = peer
        .sock
        .recv_from(&mut buf)
        .map_err(|e| format!("recv ping frame: {e}"))?;
    let sample = xrce_unframe(&buf[..n]).ok_or_else(|| "not an XRCE frame".to_string())?;
    let mut r = BufferReader::new(sample, Endianness::Little).xcdr2();
    let seq = r.read_u32().map_err(|e| format!("ping seq: {e}"))?;
    let msg = r.read_string().map_err(|e| format!("ping msg: {e}"))?;
    if seq != PING_SEQ {
        return Err(format!("ping seq {seq} != {PING_SEQ}"));
    }
    if msg != PING_MSG {
        return Err(format!("ping msg {msg:?} != {PING_MSG:?}"));
    }
    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    w.write_u32(seq).map_err(|e| format!("pong seq: {e}"))?;
    w.write_string(&format!("pong:{msg}"))
        .map_err(|e| format!("pong reply: {e}"))?;
    peer.sock
        .send_to(&xrce_data_frame(1, &w.into_bytes()), app_addr)
        .map_err(|e| format!("send pong frame: {e}"))?;
    Ok((seq, msg))
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

/// Binds the reliable peer on a **fixed** loopback `port` — for the standalone
/// `peer` binary (the harness uses [`bind_reliable_peer`] with an ephemeral
/// port). `drop_every` injects loss. `None` on bind failure.
#[must_use]
pub fn bind_reliable_peer_on(port: u16, drop_every: Option<usize>) -> Option<ReliablePeer> {
    let sock = UdpSocket::bind(("127.0.0.1", port)).ok()?;
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

// =====================================================================
// Conformant DDS-XRCE data submessages (§8.3.5.8/.9/.10)
// =====================================================================
//
// The frames above (`xrce_frame` / `xrce_data_frame`) are the *ZeroDDS Endpoint
// Profile*: the submessage body is the bare sample, with no object identity.
// A conformant DDS-XRCE WRITE_DATA/DATA instead prefixes a `BaseObjectRequest`
// (`request_id` + `object_id`, §7.7.8), so a foreign agent can map the frame to
// the target DataWriter/DataReader before reading the payload. The helpers
// below build and parse the conformant form with the real `crates/xrce` codec;
// the `conformant_*_roundtrip_client_agent` test drives a full client<->agent
// exchange over two UDP sockets with genuine XRCE frames.

use zerodds_xrce::header::{MessageHeader, SessionId};
use zerodds_xrce::submessages::{DataPayload, Message, SubmessageId, WriteDataPayload};
use zerodds_xrce::{BaseObjectRequest, ObjectId};

/// Builds a conformant DDS-XRCE `WRITE_DATA` message (client -> agent): a
/// best-effort `MessageHeader` plus one `WRITE_DATA` submessage whose body is
/// `BaseObjectRequest { request_id, object_id } + SampleData(serialized_data)`.
///
/// # Panics
/// Panics only on an internal encode error (impossible for in-bounds input).
#[must_use]
pub fn conformant_write_data_frame(
    request_id: [u8; 2],
    object_id: ObjectId,
    sample: &[u8],
) -> Vec<u8> {
    let header = MessageHeader::without_client_key(
        SessionId(0x80),
        StreamId::BUILTIN_BEST_EFFORT,
        SerialNumber16::new(1),
    )
    .expect("header");
    let sm = WriteDataPayload {
        base: BaseObjectRequest {
            request_id,
            object_id,
        },
        serialized_data: sample.to_vec(),
    }
    .into_submessage()
    .expect("write_data submessage");
    Message::new(header, vec![sm])
        .expect("message")
        .encode()
        .expect("encode")
}

/// Builds a conformant DDS-XRCE `DATA` message (agent -> client): the
/// `request_id` echoes the originating `READ_DATA`/`WRITE_DATA` request and the
/// `object_id` identifies the source DataReader (§8.3.5.10).
///
/// # Panics
/// Panics only on an internal encode error (impossible for in-bounds input).
#[must_use]
pub fn conformant_data_frame(request_id: [u8; 2], object_id: ObjectId, sample: &[u8]) -> Vec<u8> {
    let header = MessageHeader::without_client_key(
        SessionId(0x80),
        StreamId::BUILTIN_BEST_EFFORT,
        SerialNumber16::new(1),
    )
    .expect("header");
    let sm = DataPayload {
        base: BaseObjectRequest {
            request_id,
            object_id,
        },
        serialized_data: sample.to_vec(),
    }
    .into_submessage()
    .expect("data submessage");
    Message::new(header, vec![sm])
        .expect("message")
        .encode()
        .expect("encode")
}

/// Parses a conformant `WRITE_DATA` message, returning the `BaseObjectRequest`
/// (request/object identity) and the sample bytes. `None` on any decode error
/// or if no `WRITE_DATA` submessage is present.
#[must_use]
pub fn parse_conformant_write_data(frame: &[u8]) -> Option<(BaseObjectRequest, Vec<u8>)> {
    let msg = Message::decode(frame).ok()?;
    let sm = msg
        .submessages
        .iter()
        .find(|s| s.header.submessage_id == SubmessageId::WriteData)?;
    let wd = WriteDataPayload::try_from_submessage(sm).ok()?;
    Some((wd.base, wd.serialized_data))
}

/// Parses a conformant `DATA` message (agent -> client). `None` on any decode
/// error or if no `DATA` submessage is present.
#[must_use]
pub fn parse_conformant_data(frame: &[u8]) -> Option<(BaseObjectRequest, Vec<u8>)> {
    let msg = Message::decode(frame).ok()?;
    let sm = msg
        .submessages
        .iter()
        .find(|s| s.header.submessage_id == SubmessageId::Data)?;
    let d = DataPayload::try_from_submessage(sm).ok()?;
    Some((d.base, d.serialized_data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_xrce::object_kind::ObjectKind;

    // ---- direction contract ----

    #[test]
    fn write_data_frame_uses_id_0x07() {
        // Endpoint -> hub direction.
        let f = xrce_frame(1, &[0xAA, 0xBB]);
        assert_eq!(f[4], 0x07, "WRITE_DATA submessage id");
    }

    #[test]
    fn data_frame_uses_id_0x09() {
        // Hub -> endpoint direction (the pong the peer sends).
        let f = xrce_data_frame(1, &[0xAA, 0xBB]);
        assert_eq!(f[4], 0x09, "DATA submessage id");
    }

    #[test]
    fn unframe_roundtrips_write_data() {
        let sample = [0x01, 0x02, 0x03, 0x04];
        let f = xrce_frame(7, &sample);
        assert_eq!(xrce_unframe(&f), Some(&sample[..]));
    }

    // ---- negative frame vectors (hub-side reader rejects, never panics) ----

    #[test]
    fn unframe_rejects_wrong_direction_data_id() {
        // A DATA/0x09 frame is the hub->endpoint direction; the hub-side parser
        // must NOT accept it (it only consumes WRITE_DATA from endpoints).
        let f = xrce_data_frame(1, &[0xAA, 0xBB]);
        assert_eq!(xrce_unframe(&f), None);
    }

    #[test]
    fn unframe_rejects_unknown_submessage_id() {
        let mut f = xrce_frame(1, &[0xAA, 0xBB]);
        f[4] = 0x0A; // ACKNACK id in a data slot -> reject
        assert_eq!(xrce_unframe(&f), None);
    }

    #[test]
    fn unframe_rejects_truncated_header() {
        // Fewer than the 8 header bytes.
        for len in 0..8 {
            let f = vec![0u8; len];
            assert_eq!(xrce_unframe(&f), None, "len {len} must be rejected");
        }
    }

    #[test]
    fn unframe_rejects_length_past_end() {
        // Header claims a 100-byte body but only 2 bytes follow -> reject.
        let mut f = xrce_frame(1, &[0xAA, 0xBB]);
        f[6] = 100;
        f[7] = 0;
        assert_eq!(xrce_unframe(&f), None);
    }

    #[test]
    fn unframe_ignores_bytes_past_declared_length() {
        // Declared body is 2 bytes; a trailing appended byte is not part of the
        // sample and must not leak into the returned body.
        let mut f = xrce_frame(1, &[0xAA, 0xBB]);
        f.push(0xCC); // extra trailing byte (e.g. a second submessage)
        assert_eq!(xrce_unframe(&f), Some(&[0xAA, 0xBB][..]));
    }

    // ---- conformant DDS-XRCE (BaseObjectRequest before the sample) ----

    #[test]
    fn conformant_write_data_carries_object_identity() {
        let writer = ObjectId::new(0x001, ObjectKind::DataWriter).expect("writer id");
        let frame = conformant_write_data_frame([0x00, 0x2A], writer, &[0xDE, 0xAD]);
        let (base, sample) = parse_conformant_write_data(&frame).expect("parse");
        assert_eq!(base.request_id, [0x00, 0x2A]);
        assert_eq!(base.object_id, writer);
        assert_eq!(sample, vec![0xDE, 0xAD]);
        // The legacy profile parser must NOT accept the conformant frame: its
        // body is longer and starts with the BaseObjectRequest, not the sample.
        assert_ne!(xrce_unframe(&frame), Some(&[0xDE, 0xAD][..]));
    }

    /// Full client<->agent exchange with real DDS-XRCE frames over two UDP
    /// sockets: the client WRITE_DATAs a `Ping` (with a DataWriter object id),
    /// the agent maps the frame by `object_id`, decodes the sample, and replies
    /// a `DATA` (echoing the `request_id`, carrying the DataReader object id);
    /// the client decodes the `DATA`. Proves the conformant wire form both ways.
    #[test]
    fn conformant_write_data_then_data_roundtrip_client_agent() {
        let agent = UdpSocket::bind("127.0.0.1:0").expect("bind agent");
        let client = UdpSocket::bind("127.0.0.1:0").expect("bind client");
        agent
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("agent timeout");
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("client timeout");
        let agent_addr = agent.local_addr().expect("agent addr");

        let writer = ObjectId::new(0x001, ObjectKind::DataWriter).expect("writer id");
        let reader = ObjectId::new(0x001, ObjectKind::DataReader).expect("reader id");
        let request_id = [0x00, 0x2A];

        // client -> agent: WRITE_DATA(Ping)
        let mut w = BufferWriter::new(Endianness::Little).xcdr2();
        w.write_u32(PING_SEQ).expect("ping seq");
        w.write_string(PING_MSG).expect("ping msg");
        let ping = w.into_bytes();
        client
            .send_to(
                &conformant_write_data_frame(request_id, writer, &ping),
                agent_addr,
            )
            .expect("send write_data");

        // agent receives, maps by object_id, decodes
        let mut buf = [0u8; 4096];
        let (n, from) = agent.recv_from(&mut buf).expect("agent recv");
        let (base, sample) =
            parse_conformant_write_data(&buf[..n]).expect("agent parse write_data");
        assert_eq!(base.object_id, writer, "agent maps frame to the DataWriter");
        assert_eq!(base.request_id, request_id);
        let mut r = BufferReader::new(&sample, Endianness::Little).xcdr2();
        assert_eq!(r.read_u32().expect("seq"), PING_SEQ);
        assert_eq!(r.read_string().expect("msg"), PING_MSG);

        // agent -> client: DATA(Pong), echoing request_id, reader object id
        let mut w2 = BufferWriter::new(Endianness::Little).xcdr2();
        w2.write_u32(PING_SEQ).expect("pong seq");
        w2.write_string(&format!("pong:{PING_MSG}"))
            .expect("pong reply");
        let pong = w2.into_bytes();
        agent
            .send_to(&conformant_data_frame(base.request_id, reader, &pong), from)
            .expect("send data");

        // client receives the DATA
        let (n2, _) = client.recv_from(&mut buf).expect("client recv");
        let (rbase, rsample) = parse_conformant_data(&buf[..n2]).expect("client parse data");
        assert_eq!(rbase.request_id, request_id, "DATA echoes the request_id");
        assert_eq!(rbase.object_id, reader, "DATA carries the DataReader id");
        let mut r2 = BufferReader::new(&rsample, Endianness::Little).xcdr2();
        assert_eq!(r2.read_u32().expect("pong seq"), PING_SEQ);
        assert_eq!(
            r2.read_string().expect("pong reply"),
            format!("pong:{PING_MSG}")
        );
    }
}
