// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reliable XRCE stream (`stream_id >= 128`, DDS-XRCE §8.4.10/§8.4.11) for the
//! native Rust endpoint SDK — self-contained (no `zerodds-xrce` dependency), the
//! canonical state machine mirrored byte-for-byte from `crates/xrce/reliable.rs`.
//!
//! - **Sender**: [`ReliableSender`] — `submit` buffers outgoing samples,
//!   `pending_heartbeat` announces the in-flight window, `recv_acknack` clears
//!   acknowledged seqnrs and keeps the missing ones for retransmit.
//! - **Receiver**: [`ReliableReceiver`] — `recv_data` buffers out-of-order,
//!   `drain_in_order` delivers contiguously, `pending_acknack` reports the gaps.
//! - **Async-decoupled writer**: [`AsyncReliableWriter`] — a wait-free SPSC ring
//!   plus a drain thread that owns the sender + socket. The producer's `enqueue`
//!   never touches the kernel; the drain thread does the batched I/O, the
//!   HEARTBEAT timer, and the ACKNACK-driven retransmit.
//!
//! Wire (little-endian, byte-identical to `endpoints/c` + `crates/xrce`):
//! 8-byte header `[session, stream, seq_lo, seq_hi, submsg, flags, len_lo, len_hi]`
//! then body. WRITE_DATA `0x07`/`0x03` on stream `0x80` (header seq = sample seq).
//! HEARTBEAT `0x0B` / ACKNACK `0x0A` `0x01` on stream `0x00` (control), body = 5
//! bytes; the target reliable stream id travels in the body's last byte.

use std::collections::BTreeMap;
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Heartbeat period (spec recommends 100 ms; 500 ms conservative without Tx pacing).
pub const HEARTBEAT_PERIOD: Duration = Duration::from_millis(500);
/// In-flight sender window — matches the 16-bit ACKNACK bitmap.
pub const SENDER_WINDOW: usize = 16;
/// Out-of-order receiver buffer cap (DoS bound).
pub const RECEIVER_BUFFER: usize = 64;
/// Per-sample payload cap (u16 submessage length limit).
pub const MAX_PAYLOAD: usize = 65_535;

/// Reliable stream id (bit 7 set), carried in the WRITE_DATA header + control body.
pub const RELIABLE_STREAM_ID: u8 = 0x80;
const XRCE_SESSION_NOKEY: u8 = 0x80;
const XRCE_STREAM_NONE: u8 = 0x00;
const SM_WRITE_DATA: u8 = 0x07;
const SM_ACKNACK: u8 = 0x0A;
const SM_HEARTBEAT: u8 = 0x0B;
const WRITE_FLAGS: u8 = 0x03;
const CTRL_FLAGS: u8 = 0x01;

/// Errors the reliable sender can return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReliableError {
    /// Payload exceeds [`MAX_PAYLOAD`].
    PayloadTooLarge,
    /// The sender window is full — process ACKNACKs first.
    WindowFull,
    /// The receiver out-of-order buffer is full.
    BufferFull,
}

/// RFC-1982: is `a` strictly less than `b` in 16-bit serial arithmetic?
#[must_use]
pub fn seq_lt(a: u16, b: u16) -> bool {
    let diff = b.wrapping_sub(a);
    diff != 0 && diff < 0x8000
}

// ---------------------------------------------------------------------------
// Wire frames
// ---------------------------------------------------------------------------

/// HEARTBEAT body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heartbeat {
    /// First unacknowledged seqnr.
    pub first: i16,
    /// Last unacknowledged seqnr.
    pub last: i16,
    /// Target reliable stream id.
    pub stream_id: u8,
}

/// ACKNACK body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckNack {
    /// Smallest seqnr the receiver still expects.
    pub first_unacked: i16,
    /// 16-bit NACK bitmap (LE); bit `i` set ⇒ `first_unacked + i` missing.
    pub nack_bitmap: [u8; 2],
    /// Target reliable stream id.
    pub stream_id: u8,
}

/// Builds a reliable WRITE_DATA frame; the header seq carries the sample seqnr.
#[must_use]
pub fn write_frame(seq: u16, sample: &[u8]) -> Vec<u8> {
    let s = seq.to_le_bytes();
    let n = (u16::try_from(sample.len()).unwrap_or(u16::MAX)).to_le_bytes();
    let mut out = Vec::with_capacity(8 + sample.len());
    out.extend_from_slice(&[
        XRCE_SESSION_NOKEY,
        RELIABLE_STREAM_ID,
        s[0],
        s[1],
        SM_WRITE_DATA,
        WRITE_FLAGS,
        n[0],
        n[1],
    ]);
    out.extend_from_slice(sample);
    out
}

/// Parses a reliable WRITE_DATA frame into `(seq, sample)`.
#[must_use]
pub fn unframe(frame: &[u8]) -> Option<(u16, &[u8])> {
    if frame.len() < 8 || frame[4] != SM_WRITE_DATA {
        return None;
    }
    Some((u16::from_le_bytes([frame[2], frame[3]]), &frame[8..]))
}

/// Builds a HEARTBEAT control frame. `msg_seq` is the control-stream sequence
/// (the byte-golden uses `1`). Byte-identical to `golden_heartbeat_le.bin`.
#[must_use]
pub fn heartbeat_frame(hb: Heartbeat, msg_seq: u16) -> Vec<u8> {
    let m = msg_seq.to_le_bytes();
    let f = hb.first.to_le_bytes();
    let l = hb.last.to_le_bytes();
    vec![
        XRCE_SESSION_NOKEY,
        XRCE_STREAM_NONE,
        m[0],
        m[1],
        SM_HEARTBEAT,
        CTRL_FLAGS,
        5,
        0,
        f[0],
        f[1],
        l[0],
        l[1],
        hb.stream_id,
    ]
}

/// Parses a HEARTBEAT frame (keys on the submessage id; header is ignored).
#[must_use]
pub fn parse_heartbeat(frame: &[u8]) -> Option<Heartbeat> {
    if frame.len() < 13 || frame[4] != SM_HEARTBEAT {
        return None;
    }
    Some(Heartbeat {
        first: i16::from_le_bytes([frame[8], frame[9]]),
        last: i16::from_le_bytes([frame[10], frame[11]]),
        stream_id: frame[12],
    })
}

/// Builds an ACKNACK control frame. Byte-identical to `golden_acknack_le.bin`.
#[must_use]
pub fn acknack_frame(ack: AckNack, msg_seq: u16) -> Vec<u8> {
    let m = msg_seq.to_le_bytes();
    let f = ack.first_unacked.to_le_bytes();
    vec![
        XRCE_SESSION_NOKEY,
        XRCE_STREAM_NONE,
        m[0],
        m[1],
        SM_ACKNACK,
        CTRL_FLAGS,
        5,
        0,
        f[0],
        f[1],
        ack.nack_bitmap[0],
        ack.nack_bitmap[1],
        ack.stream_id,
    ]
}

/// Parses an ACKNACK frame (keys on the submessage id; header is ignored).
#[must_use]
pub fn parse_acknack(frame: &[u8]) -> Option<AckNack> {
    if frame.len() < 13 || frame[4] != SM_ACKNACK {
        return None;
    }
    Some(AckNack {
        first_unacked: i16::from_le_bytes([frame[8], frame[9]]),
        nack_bitmap: [frame[10], frame[11]],
        stream_id: frame[12],
    })
}

// ---------------------------------------------------------------------------
// Sender
// ---------------------------------------------------------------------------

/// Reliable sender: assigns seqnrs, holds in-flight samples until acknowledged,
/// emits HEARTBEATs, and clears/keeps samples on ACKNACK.
#[derive(Debug, Default)]
pub struct ReliableSender {
    next_seq: u16,
    in_flight: BTreeMap<u16, Vec<u8>>,
    last_heartbeat: Option<Instant>,
}

impl ReliableSender {
    /// New empty sender.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// In-flight (unacknowledged) sample count.
    #[must_use]
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Submits a sample, returning its assigned seqnr.
    ///
    /// # Errors
    /// [`ReliableError::PayloadTooLarge`] / [`ReliableError::WindowFull`].
    pub fn submit(&mut self, payload: Vec<u8>) -> Result<u16, ReliableError> {
        if payload.len() > MAX_PAYLOAD {
            return Err(ReliableError::PayloadTooLarge);
        }
        if self.in_flight.len() >= SENDER_WINDOW {
            return Err(ReliableError::WindowFull);
        }
        let seq = self.next_seq;
        self.in_flight.insert(seq, payload);
        self.next_seq = self.next_seq.wrapping_add(1);
        Ok(seq)
    }

    /// The in-flight payload for `seq`, if still unacknowledged (for retransmit).
    #[must_use]
    pub fn get_in_flight(&self, seq: u16) -> Option<&[u8]> {
        self.in_flight.get(&seq).map(Vec::as_slice)
    }

    /// All in-flight seqnrs, ascending (for retransmit against a NACK bitmap).
    #[must_use]
    pub fn in_flight_seqs(&self) -> Vec<u16> {
        self.in_flight.keys().copied().collect()
    }

    /// Returns a HEARTBEAT if the period elapsed and samples are in flight.
    pub fn pending_heartbeat(&mut self, now: Instant) -> Option<Heartbeat> {
        if self.in_flight.is_empty() {
            return None;
        }
        let due = match self.last_heartbeat {
            None => true,
            Some(t) => now.duration_since(t) >= HEARTBEAT_PERIOD,
        };
        if !due {
            return None;
        }
        self.last_heartbeat = Some(now);
        let first = *self.in_flight.keys().next()?;
        let last = *self.in_flight.keys().next_back()?;
        Some(Heartbeat {
            first: first as i16,
            last: last as i16,
            stream_id: RELIABLE_STREAM_ID,
        })
    }

    /// Processes an ACKNACK: drops everything `< first_unacked` and every
    /// clear-bit slot in `[first_unacked, first_unacked+16)`; keeps set bits.
    pub fn recv_acknack(&mut self, ack: AckNack) {
        let base = ack.first_unacked as u16;
        let bitmap = u16::from_le_bytes(ack.nack_bitmap);
        let to_drop: Vec<u16> = self
            .in_flight
            .keys()
            .copied()
            .filter(|&k| seq_lt(k, base))
            .collect();
        for k in to_drop {
            self.in_flight.remove(&k);
        }
        for i in 0u16..16 {
            let seq = base.wrapping_add(i);
            if (bitmap >> i) & 1 == 0 {
                self.in_flight.remove(&seq);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Receiver
// ---------------------------------------------------------------------------

/// Reliable receiver: buffers out-of-order, delivers contiguously, reports gaps.
#[derive(Debug, Default)]
pub struct ReliableReceiver {
    expected: u16,
    received: BTreeMap<u16, Vec<u8>>,
}

impl ReliableReceiver {
    /// New empty receiver.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Next expected seqnr.
    #[must_use]
    pub fn expected(&self) -> u16 {
        self.expected
    }

    /// Buffered out-of-order sample count.
    #[must_use]
    pub fn out_of_order_count(&self) -> usize {
        self.received.len()
    }

    /// Accepts a sample (drops duplicates before `expected`).
    ///
    /// # Errors
    /// [`ReliableError::BufferFull`] when the reorder buffer is full.
    pub fn recv_data(&mut self, seq: u16, payload: Vec<u8>) -> Result<(), ReliableError> {
        if seq_lt(seq, self.expected) {
            return Ok(()); // duplicate before the delivery frontier
        }
        if self.received.contains_key(&seq) {
            return Ok(());
        }
        if self.received.len() >= RECEIVER_BUFFER {
            return Err(ReliableError::BufferFull);
        }
        self.received.insert(seq, payload);
        Ok(())
    }

    /// Delivers all contiguous samples from `expected`, advancing it.
    pub fn drain_in_order(&mut self) -> Vec<(u16, Vec<u8>)> {
        let mut out = Vec::new();
        while let Some(payload) = self.received.remove(&self.expected) {
            out.push((self.expected, payload));
            self.expected = self.expected.wrapping_add(1);
        }
        out
    }

    /// Computes the ACKNACK marking the missing slots in `[expected, expected+16)`.
    /// A clear bit unambiguously means "received" (`hint` unused here — full window).
    #[must_use]
    pub fn pending_acknack(&self) -> AckNack {
        let base = self.expected;
        let mut bitmap: u16 = 0;
        for i in 0u16..16 {
            let seq = base.wrapping_add(i);
            if !self.received.contains_key(&seq) {
                bitmap |= 1u16 << i;
            }
        }
        AckNack {
            first_unacked: base as i16,
            nack_bitmap: bitmap.to_le_bytes(),
            stream_id: RELIABLE_STREAM_ID,
        }
    }

    /// Clears all receiver state.
    pub fn reset(&mut self) {
        self.expected = 0;
        self.received.clear();
    }
}

// ---------------------------------------------------------------------------
// Async-decoupled writer: wait-free SPSC ring + drain thread
// ---------------------------------------------------------------------------

/// A bounded single-producer/single-consumer ring. `push`/`pop` are wait-free
/// (two atomics, no lock); `push` returns the item back on a full ring.
struct SpscRing {
    slots: Box<[std::cell::UnsafeCell<Option<Vec<u8>>>]>,
    mask: usize,
    head: AtomicUsize, // producer writes
    tail: AtomicUsize, // consumer reads
}

// SAFETY: head/tail discipline gives each slot a single writer then a single
// reader; the Vec is moved across the boundary via Acquire/Release ordering.
unsafe impl Sync for SpscRing {}
// SAFETY: the ring transfers ownership of each Vec<u8> from producer to consumer
// under the same Acquire/Release head/tail protocol, so sending it across threads
// is sound.
unsafe impl Send for SpscRing {}

impl SpscRing {
    fn new(capacity_pow2: usize) -> Self {
        let cap = capacity_pow2.next_power_of_two();
        let mut v = Vec::with_capacity(cap);
        for _ in 0..cap {
            v.push(std::cell::UnsafeCell::new(None));
        }
        Self {
            slots: v.into_boxed_slice(),
            mask: cap - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Non-consuming emptiness check (consumer side).
    fn is_empty(&self) -> bool {
        self.tail.load(Ordering::Relaxed) == self.head.load(Ordering::Acquire)
    }

    /// Producer: enqueue, or return `Err(item)` if full (backpressure).
    fn push(&self, item: Vec<u8>) -> Result<(), Vec<u8>> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= self.slots.len() {
            return Err(item);
        }
        let idx = head & self.mask;
        // SAFETY: single producer owns this slot until the Release store below.
        unsafe { *self.slots[idx].get() = Some(item) };
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Consumer: dequeue one item, or `None` if empty.
    fn pop(&self) -> Option<Vec<u8>> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let idx = tail & self.mask;
        // SAFETY: single consumer owns this slot until the Release store below.
        let item = unsafe { (*self.slots[idx].get()).take() };
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        item
    }
}

/// Handle held by the producer (aggregator). `enqueue` is wait-free and never
/// enters the kernel — the drain thread does all I/O.
#[derive(Clone)]
pub struct ReliableWriterHandle {
    ring: Arc<SpscRing>,
    running: Arc<AtomicBool>,
}

impl ReliableWriterHandle {
    /// Wait-free enqueue of one sample body. `Err` = ring full (backpressure).
    ///
    /// # Errors
    /// Returns the sample back when the ring is full.
    pub fn enqueue(&self, sample: Vec<u8>) -> Result<(), Vec<u8>> {
        self.ring.push(sample)
    }

    /// Signals the drain thread to finish (after the window empties).
    pub fn close(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// The async-decoupled reliable writer: owns the drain thread.
pub struct AsyncReliableWriter {
    handle: ReliableWriterHandle,
    drain: Option<JoinHandle<()>>,
}

impl AsyncReliableWriter {
    /// Spawns the drain thread. It owns `sock` (connected/targeting the peer),
    /// submits queued samples, sends WRITE_DATA, emits HEARTBEATs, and retransmits
    /// on ACKNACK until the ring is closed *and* the window has drained.
    #[must_use]
    pub fn start(sock: UdpSocket, ring_capacity: usize) -> Self {
        let ring = Arc::new(SpscRing::new(ring_capacity));
        let running = Arc::new(AtomicBool::new(true));
        let handle = ReliableWriterHandle {
            ring: Arc::clone(&ring),
            running: Arc::clone(&running),
        };
        let drain = thread::spawn(move || drain_loop(&sock, &ring, &running));
        Self {
            handle,
            drain: Some(drain),
        }
    }

    /// A cloneable producer handle.
    #[must_use]
    pub fn handle(&self) -> ReliableWriterHandle {
        self.handle.clone()
    }

    /// Closes the writer and joins the drain thread (blocks until the window
    /// drains). Idempotent.
    pub fn shutdown(&mut self) {
        self.handle.close();
        if let Some(j) = self.drain.take() {
            let _ = j.join();
        }
    }
}

impl Drop for AsyncReliableWriter {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// The drain thread body: the ns-decoupled producer's I/O + reliability engine.
fn drain_loop(sock: &UdpSocket, ring: &SpscRing, running: &AtomicBool) {
    sock.set_read_timeout(Some(Duration::from_millis(5))).ok();
    let mut sender = ReliableSender::new();
    let mut ctl_seq: u16 = 1;
    let mut buf = [0u8; 2048];
    // Best-effort drain deadline set when the writer is closed: shutdown() must
    // always terminate even if the peer stops ACKing (never an unbounded join).
    let mut close_deadline: Option<Instant> = None;
    loop {
        // 1) Drain the ring into the sender window and send fresh WRITE_DATA.
        while sender.in_flight_count() < SENDER_WINDOW {
            let Some(sample) = ring.pop() else { break };
            if let Ok(seq) = sender.submit(sample.clone()) {
                let _ = sock.send(&write_frame(seq, &sample));
            }
        }
        // 2) HEARTBEAT when due.
        if let Some(hb) = sender.pending_heartbeat(Instant::now()) {
            let _ = sock.send(&heartbeat_frame(hb, ctl_seq));
            ctl_seq = ctl_seq.wrapping_add(1);
        }
        // 3) Drain incoming ACKNACKs → retransmit the still-missing samples.
        while let Ok(n) = sock.recv(&mut buf) {
            if let Some(ack) = parse_acknack(&buf[..n]) {
                let base = ack.first_unacked as u16;
                let bitmap = u16::from_le_bytes(ack.nack_bitmap);
                sender.recv_acknack(ack);
                for i in 0u16..16 {
                    if (bitmap >> i) & 1 == 1 {
                        let seq = base.wrapping_add(i);
                        if let Some(p) = sender.get_in_flight(seq) {
                            let _ = sock.send(&write_frame(seq, p));
                        }
                    }
                }
            }
            if n == 0 {
                break;
            }
        }
        // 4) Exit once closed AND the window has drained (all acked) — or, as a
        //    safety valve, once the post-close drain deadline elapses (a vanished
        //    peer must never wedge shutdown()).
        let closed = !running.load(Ordering::Relaxed);
        if closed {
            if close_deadline.is_none() {
                close_deadline = Some(Instant::now() + Duration::from_secs(5));
            }
            if sender.in_flight_count() == 0 && ring.is_empty() {
                break;
            }
            if close_deadline.is_some_and(|dl| Instant::now() > dl) {
                break; // best-effort give-up; peer stopped ACKing
            }
            // Keep HEARTBEATing so the peer keeps ACKing until the window empties.
            sender.last_heartbeat = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    // ---- state machine, mirroring crates/xrce/reliable.rs ----

    #[test]
    fn submit_assigns_monotonic_seqnrs() {
        let mut s = ReliableSender::new();
        assert_eq!(s.submit(vec![1, 2]).unwrap(), 0);
        assert_eq!(s.submit(vec![3, 4]).unwrap(), 1);
        assert_eq!(s.in_flight_count(), 2);
    }

    #[test]
    fn submit_rejects_payload_too_large() {
        let mut s = ReliableSender::new();
        assert_eq!(
            s.submit(vec![0u8; MAX_PAYLOAD + 1]),
            Err(ReliableError::PayloadTooLarge)
        );
    }

    #[test]
    fn submit_rejects_when_window_full() {
        let mut s = ReliableSender::new();
        for _ in 0..SENDER_WINDOW {
            s.submit(vec![0]).unwrap();
        }
        assert_eq!(s.submit(vec![0]), Err(ReliableError::WindowFull));
    }

    #[test]
    fn heartbeat_fires_first_then_silences_until_period() {
        let mut s = ReliableSender::new();
        s.submit(vec![1]).unwrap();
        let t0 = Instant::now();
        assert!(s.pending_heartbeat(t0).is_some());
        assert!(
            s.pending_heartbeat(t0 + Duration::from_millis(100))
                .is_none()
        );
        assert!(
            s.pending_heartbeat(t0 + Duration::from_millis(600))
                .is_some()
        );
    }

    #[test]
    fn heartbeat_none_when_window_empty() {
        let mut s = ReliableSender::new();
        assert!(s.pending_heartbeat(Instant::now()).is_none());
    }

    #[test]
    fn recv_acknack_clears_acked_keeps_missing() {
        let mut s = ReliableSender::new();
        for _ in 0..3 {
            s.submit(vec![0]).unwrap();
        }
        // base=2, bit0 set → seq2 missing, 0+1 acked.
        s.recv_acknack(AckNack {
            first_unacked: 2,
            nack_bitmap: [0x01, 0x00],
            stream_id: RELIABLE_STREAM_ID,
        });
        assert_eq!(s.in_flight_count(), 1);
        assert!(s.get_in_flight(2).is_some());
    }

    #[test]
    fn recv_acknack_full_clear_when_no_bits() {
        let mut s = ReliableSender::new();
        for _ in 0..5 {
            s.submit(vec![0]).unwrap();
        }
        s.recv_acknack(AckNack {
            first_unacked: 5,
            nack_bitmap: [0, 0],
            stream_id: RELIABLE_STREAM_ID,
        });
        assert_eq!(s.in_flight_count(), 0);
    }

    #[test]
    fn recv_data_delivers_in_order() {
        let mut r = ReliableReceiver::new();
        r.recv_data(0, vec![10]).unwrap();
        r.recv_data(1, vec![11]).unwrap();
        let d = r.drain_in_order();
        assert_eq!(d.len(), 2);
        assert_eq!(r.expected(), 2);
    }

    #[test]
    fn recv_data_reorders_out_of_order() {
        let mut r = ReliableReceiver::new();
        r.recv_data(2, vec![22]).unwrap();
        r.recv_data(0, vec![20]).unwrap();
        assert_eq!(r.drain_in_order().len(), 1); // only seq0
        r.recv_data(1, vec![21]).unwrap();
        assert_eq!(r.drain_in_order().len(), 2); // seq1+2
    }

    #[test]
    fn recv_data_drops_duplicates() {
        let mut r = ReliableReceiver::new();
        r.recv_data(0, vec![1]).unwrap();
        r.drain_in_order();
        r.recv_data(0, vec![99]).unwrap();
        assert_eq!(r.out_of_order_count(), 0);
    }

    #[test]
    fn recv_data_rejects_when_buffer_full() {
        let mut r = ReliableReceiver::new();
        for i in 1..=RECEIVER_BUFFER as u16 {
            r.recv_data(i, vec![1]).unwrap();
        }
        assert_eq!(
            r.recv_data(RECEIVER_BUFFER as u16 + 1, vec![1]),
            Err(ReliableError::BufferFull)
        );
    }

    #[test]
    fn pending_acknack_marks_missing_slots() {
        let mut r = ReliableReceiver::new();
        r.recv_data(1, vec![1]).unwrap();
        r.recv_data(3, vec![3]).unwrap();
        let bm = u16::from_le_bytes(r.pending_acknack().nack_bitmap);
        assert!(bm & 1 != 0); // seq0 missing
        assert!(bm & (1 << 2) != 0); // seq2 missing
        assert!(bm & (1 << 1) == 0); // seq1 present
        assert!(bm & (1 << 3) == 0); // seq3 present
    }

    #[test]
    fn reset_clears_receiver() {
        let mut r = ReliableReceiver::new();
        r.recv_data(0, vec![1]).unwrap();
        r.reset();
        assert_eq!(r.out_of_order_count(), 0);
        assert_eq!(r.expected(), 0);
    }

    #[test]
    fn end_to_end_loss_recovery_in_process() {
        let mut sender = ReliableSender::new();
        let mut receiver = ReliableReceiver::new();
        let s0 = sender.submit(vec![10]).unwrap();
        let s1 = sender.submit(vec![11]).unwrap();
        let s2 = sender.submit(vec![12]).unwrap();
        // s1 lost.
        receiver.recv_data(s0, vec![10]).unwrap();
        receiver.recv_data(s2, vec![12]).unwrap();
        assert_eq!(receiver.drain_in_order().len(), 1);
        let ack = receiver.pending_acknack();
        sender.recv_acknack(ack);
        assert!(sender.get_in_flight(s1).is_some());
        let p = sender.get_in_flight(s1).unwrap().to_vec();
        receiver.recv_data(s1, p).unwrap();
        assert_eq!(receiver.drain_in_order().len(), 2); // s1+s2
    }

    // ---- byte-golden: identical to golden_heartbeat_le.bin / golden_acknack_le.bin ----

    #[test]
    fn heartbeat_frame_byte_golden() {
        let f = heartbeat_frame(
            Heartbeat {
                first: 1,
                last: 3,
                stream_id: 0x80,
            },
            1,
        );
        assert_eq!(f, [128, 0, 1, 0, 11, 1, 5, 0, 1, 0, 3, 0, 128]);
    }

    #[test]
    fn acknack_frame_byte_golden() {
        let f = acknack_frame(
            AckNack {
                first_unacked: 1,
                nack_bitmap: [0, 0],
                stream_id: 0x80,
            },
            1,
        );
        assert_eq!(f, [128, 0, 1, 0, 10, 1, 5, 0, 1, 0, 0, 0, 128]);
    }

    #[test]
    fn write_frame_round_trips() {
        let f = write_frame(7, b"hi");
        let (seq, body) = unframe(&f).unwrap();
        assert_eq!(seq, 7);
        assert_eq!(body, b"hi");
    }

    #[test]
    fn spsc_ring_fifo_and_backpressure() {
        let r = SpscRing::new(4);
        for i in 0..4u8 {
            r.push(vec![i]).unwrap();
        }
        assert!(r.push(vec![99]).is_err()); // full
        assert_eq!(r.pop().unwrap(), vec![0]);
        r.push(vec![4]).unwrap();
        for want in [1u8, 2, 3, 4] {
            assert_eq!(r.pop().unwrap(), vec![want]);
        }
        assert!(r.pop().is_none());
    }
}
