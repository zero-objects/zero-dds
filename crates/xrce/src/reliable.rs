// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Reliable-Stream-State-Machine (Spec §8.4.10/§8.4.11).
//!
//! Per reliable stream (StreamId with bit 7 set → `id >= 128`)
//! a `ReliableStreamState` runs, which has the following tasks:
//!
//! - **Sender side**: `submit(seq, payload)` buffers outgoing
//!   `WRITE_DATA` bodies, periodically sends `HEARTBEAT` (`pending_heartbeat`).
//!   `recv_acknack(...)` clears acknowledged sequence numbers.
//!
//! - **Receiver side**: `recv_data(seq, payload)` buffers incoming
//!   samples in an out-of-order buffer and delivers them via
//!   `drain_in_order()` starting at `expected_seq`. `pending_acknack()`
//!   reports the missing seqnrs as a 16-bit bitmap.
//!
//! The stream uses RFC-1982 16-bit sequence numbers, which allows 32,768
//! simultaneously in-flight samples (Spec §8.3.2.3). We
//! cap the sender window at `SENDER_WINDOW_CAP = 16` (matches the
//! 16-bit bitmap of the ACKNACK).

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::time::Duration;

use crate::error::XrceError;
use crate::header::StreamId;
use crate::serial_number::SerialNumber16;
use crate::submessages::{AckNackPayload, HeartbeatPayload};

/// Default heartbeat period (spec recommendation 100 ms; conservatively
/// 500 ms here, because we have no Tx pacing layer underneath).
pub const DEFAULT_HEARTBEAT_PERIOD: Duration = Duration::from_millis(500);

/// Sender window cap: 16 samples in-flight (matches the 16-bit
/// `nack_bitmap` in the ACKNACK body).
pub const SENDER_WINDOW_CAP: usize = 16;

/// Receiver buffer cap: 64 out-of-order samples (DoS protection: a
/// malicious sender could otherwise allocate arbitrarily many reorder
/// buckets).
pub const RECEIVER_BUFFER_CAP: usize = 64;

/// Per-sample payload cap: 64 KiB (= u16 submessage length limit).
pub const RELIABLE_MAX_PAYLOAD: usize = 65_535;

/// Configuration of the reliable stream.
#[derive(Debug, Clone, Copy)]
pub struct ReliableConfig {
    /// Heartbeat period (sender → receiver).
    pub heartbeat_period: Duration,
    /// Max. sender window size (in-flight unacknowledged samples).
    pub sender_window: usize,
    /// Max. out-of-order samples in the receiver buffer.
    pub receiver_buffer: usize,
}

impl Default for ReliableConfig {
    fn default() -> Self {
        Self {
            heartbeat_period: DEFAULT_HEARTBEAT_PERIOD,
            sender_window: SENDER_WINDOW_CAP,
            receiver_buffer: RECEIVER_BUFFER_CAP,
        }
    }
}

/// State machine of a reliable XRCE stream.
#[derive(Debug, Clone)]
pub struct ReliableStreamState {
    stream_id: StreamId,
    config: ReliableConfig,

    // -------- Sender state -----------
    /// Next seqnr to emit (monotonic, RFC-1982).
    next_seq: SerialNumber16,
    /// In-flight samples: seq → payload. BTreeMap so that the
    /// first/last pair via `keys()` is simple.
    in_flight: BTreeMap<u16, Vec<u8>>,
    /// Last sent heartbeat (`uptime`-relative).
    last_heartbeat: Option<Duration>,

    // -------- Receiver state ---------
    /// Next expected incoming seqnr.
    expected_seq: SerialNumber16,
    /// Out-of-order buffer: seq → payload.
    received: BTreeMap<u16, Vec<u8>>,
}

impl ReliableStreamState {
    /// Constructor.
    ///
    /// # Panics
    /// `stream_id` must be reliable (`is_reliable()`).
    #[must_use]
    pub fn new(stream_id: StreamId, config: ReliableConfig) -> Self {
        assert!(
            stream_id.is_reliable(),
            "ReliableStreamState requires reliable stream id (>=128)"
        );
        Self {
            stream_id,
            config,
            next_seq: SerialNumber16::new(0),
            in_flight: BTreeMap::new(),
            last_heartbeat: None,
            expected_seq: SerialNumber16::new(0),
            received: BTreeMap::new(),
        }
    }

    /// `StreamId`.
    #[must_use]
    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    /// Number of in-flight samples on the sender side.
    #[must_use]
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Number of out-of-order samples on the receiver side.
    #[must_use]
    pub fn out_of_order_count(&self) -> usize {
        self.received.len()
    }

    /// Aktuelle Empfangs-Erwartungs-Seqnr (Receiver).
    #[must_use]
    pub fn expected(&self) -> SerialNumber16 {
        self.expected_seq
    }

    // ---------------------------------------------------------------
    // Sender-Seite
    // ---------------------------------------------------------------

    /// Submits a new sample. Returns the assigned seqnr.
    ///
    /// # Errors
    /// - `PayloadTooLarge` if `payload.len() > RELIABLE_MAX_PAYLOAD`.
    /// - `ValueOutOfRange` if the sender window is full (`sender_window`
    ///   in-flight samples). The caller must process ACKNACKs first.
    pub fn submit(&mut self, payload: Vec<u8>) -> Result<SerialNumber16, XrceError> {
        if payload.len() > RELIABLE_MAX_PAYLOAD {
            return Err(XrceError::PayloadTooLarge {
                limit: RELIABLE_MAX_PAYLOAD,
                actual: payload.len(),
            });
        }
        if self.in_flight.len() >= self.config.sender_window {
            return Err(XrceError::ValueOutOfRange {
                message: "reliable sender window full",
            });
        }
        let seq = self.next_seq;
        self.in_flight.insert(seq.raw(), payload);
        self.next_seq = self.next_seq.next();
        Ok(seq)
    }

    /// Looks up an in-flight payload (e.g. for resend).
    #[must_use]
    pub fn get_in_flight(&self, seq: SerialNumber16) -> Option<&[u8]> {
        self.in_flight.get(&seq.raw()).map(Vec::as_slice)
    }

    /// Tick: returns `Some(HEARTBEAT)` if the heartbeat period
    /// has elapsed and in-flight samples exist.
    pub fn pending_heartbeat(&mut self, now: Duration) -> Option<HeartbeatPayload> {
        if self.in_flight.is_empty() {
            return None;
        }
        let due = match self.last_heartbeat {
            None => true,
            Some(t) => now.saturating_sub(t) >= self.config.heartbeat_period,
        };
        if !due {
            return None;
        }
        self.last_heartbeat = Some(now);
        let first = *self.in_flight.keys().next()?;
        let last = *self.in_flight.keys().next_back()?;
        Some(HeartbeatPayload {
            // i16 reinterpret cast — RFC-1982 comparison happens on the
            // receiver side via `wrapping_*`.
            first_unacked_seq_nr: first as i16,
            last_unacked_seq_nr: last as i16,
            stream_id: self.stream_id.0,
        })
    }

    /// Processes an incoming ACKNACK on the sender side.
    ///
    /// `first_unacked` is the smallest seqnr the receiver still
    /// expects; everything strictly before it is removed as acknowledged.
    /// `nack_bitmap` is 16-bit; bit `i` = "seqnr `first_unacked + i`
    /// still missing". So we remove all samples `< first_unacked`
    /// and all samples in `[first_unacked, first_unacked+16)` whose
    /// bit is not set.
    pub fn recv_acknack(&mut self, payload: AckNackPayload) {
        let base = payload.first_unacked_seq_num as u16;
        let bitmap = u16::from_le_bytes(payload.nack_bitmap);

        // 1) Acknowledge everything before base.
        let to_remove: Vec<u16> = self
            .in_flight
            .keys()
            .copied()
            .filter(|&k| {
                let diff = base.wrapping_sub(k);
                // k < base per RFC-1982?
                diff > 0 && diff < SerialNumber16::HALF_WINDOW
            })
            .collect();
        for k in to_remove {
            self.in_flight.remove(&k);
        }

        // 2) Within the bitmap window: bit set → missing → keep.
        //    Bit not set → acknowledged → delete.
        for i in 0u16..16 {
            let seq = base.wrapping_add(i);
            let bit = (bitmap >> i) & 1;
            if bit == 0 {
                // acknowledged
                self.in_flight.remove(&seq);
            }
        }
    }

    // ---------------------------------------------------------------
    // Receiver side
    // ---------------------------------------------------------------

    /// Receiver: a sample with `seq + payload` has arrived.
    /// Stores it in the out-of-order buffer (or discards it as a duplicate).
    ///
    /// # Errors
    /// `ValueOutOfRange` if the receiver buffer is full (DoS protection).
    pub fn recv_data(&mut self, seq: SerialNumber16, payload: Vec<u8>) -> Result<(), XrceError> {
        // Already delivered (before expected_seq)?
        if seq.wrapping_lt(self.expected_seq) {
            return Ok(()); // duplicate → silently drop
        }
        if self.received.contains_key(&seq.raw()) {
            return Ok(()); // already in the buffer
        }
        if self.received.len() >= self.config.receiver_buffer {
            return Err(XrceError::ValueOutOfRange {
                message: "reliable receiver buffer full",
            });
        }
        self.received.insert(seq.raw(), payload);
        Ok(())
    }

    /// Receiver: returns all in-order available samples starting at
    /// `expected_seq` and advances `expected_seq`.
    pub fn drain_in_order(&mut self) -> Vec<(SerialNumber16, Vec<u8>)> {
        let mut out = Vec::new();
        loop {
            let key = self.expected_seq.raw();
            if let Some(payload) = self.received.remove(&key) {
                out.push((self.expected_seq, payload));
                self.expected_seq = self.expected_seq.next();
            } else {
                break;
            }
        }
        out
    }

    /// Receiver: computes the appropriate ACKNACK payload that marks the
    /// missing seqnrs starting at `expected_seq`. Returns
    /// `Some(ACKNACK)` when out-of-order samples are present OR
    /// `last_recv_seen` (HEARTBEAT hint) reveals a gap.
    #[must_use]
    pub fn pending_acknack(&self, hint_last_seen: Option<SerialNumber16>) -> AckNackPayload {
        let base = self.expected_seq;
        let mut bitmap: u16 = 0;
        // We mark ALL slots in the window as missing that are NOT in the
        // received buffer — where the window is
        // [base, base+16). If `hint_last_seen` is given,
        // slots strictly after `hint_last_seen` are treated as not-missing.
        for i in 0u16..16 {
            let seq = base.next().0.wrapping_sub(1).wrapping_add(i);
            let s = SerialNumber16::new(seq);
            // skip if > hint_last_seen
            if let Some(h) = hint_last_seen {
                if s.wrapping_gt(h) {
                    continue;
                }
            }
            // missing if not in the received buffer
            if !self.received.contains_key(&seq) {
                bitmap |= 1u16 << i;
            }
        }
        AckNackPayload {
            first_unacked_seq_num: base.raw() as i16,
            nack_bitmap: bitmap.to_le_bytes(),
            stream_id: self.stream_id.0,
        }
    }

    /// Resets the stream state (e.g. after a `RESET` submessage).
    pub fn reset(&mut self) {
        self.next_seq = SerialNumber16::new(0);
        self.in_flight.clear();
        self.last_heartbeat = None;
        self.expected_seq = SerialNumber16::new(0);
        self.received.clear();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    fn rs() -> ReliableStreamState {
        ReliableStreamState::new(StreamId::BUILTIN_RELIABLE, ReliableConfig::default())
    }

    #[test]
    fn submit_assigns_monotonic_seqnrs() {
        let mut s = rs();
        let s0 = s.submit(alloc::vec![1, 2]).unwrap();
        let s1 = s.submit(alloc::vec![3, 4]).unwrap();
        assert_eq!(s0.raw(), 0);
        assert_eq!(s1.raw(), 1);
        assert_eq!(s.in_flight_count(), 2);
    }

    #[test]
    fn submit_rejects_payload_too_large() {
        let mut s = rs();
        let huge = alloc::vec![0u8; RELIABLE_MAX_PAYLOAD + 1];
        assert!(matches!(
            s.submit(huge),
            Err(XrceError::PayloadTooLarge { .. })
        ));
    }

    #[test]
    fn submit_rejects_when_window_full() {
        let mut s = rs();
        for _ in 0..SENDER_WINDOW_CAP {
            s.submit(alloc::vec![0]).unwrap();
        }
        assert!(s.submit(alloc::vec![0]).is_err());
    }

    #[test]
    fn pending_heartbeat_fires_first_time() {
        let mut s = rs();
        s.submit(alloc::vec![1]).unwrap();
        let hb = s.pending_heartbeat(Duration::from_secs(0));
        assert!(hb.is_some());
        let h = hb.unwrap();
        assert_eq!(h.first_unacked_seq_nr, 0);
        assert_eq!(h.last_unacked_seq_nr, 0);
        assert_eq!(h.stream_id, StreamId::BUILTIN_RELIABLE.0);
    }

    #[test]
    fn pending_heartbeat_silenced_until_period_elapsed() {
        let mut s = rs();
        s.submit(alloc::vec![1]).unwrap();
        assert!(s.pending_heartbeat(Duration::from_millis(0)).is_some());
        // immediately after: no 500ms elapsed yet
        assert!(s.pending_heartbeat(Duration::from_millis(100)).is_none());
        // after 600ms: yes
        assert!(s.pending_heartbeat(Duration::from_millis(600)).is_some());
    }

    #[test]
    fn pending_heartbeat_none_when_window_empty() {
        let mut s = rs();
        assert!(s.pending_heartbeat(Duration::from_secs(0)).is_none());
    }

    #[test]
    fn recv_acknack_clears_acked_seqnrs() {
        let mut s = rs();
        s.submit(alloc::vec![0xA0]).unwrap(); // seq 0
        s.submit(alloc::vec![0xA1]).unwrap(); // seq 1
        s.submit(alloc::vec![0xA2]).unwrap(); // seq 2
        assert_eq!(s.in_flight_count(), 3);
        // base=2, bitmap=0b0001 → seq 2 missing, so everything before (0,1) acknowledged
        // and seq 2 itself marked as still missing
        let ack = AckNackPayload {
            first_unacked_seq_num: 2,
            nack_bitmap: [0x01, 0x00],
            stream_id: StreamId::BUILTIN_RELIABLE.0,
        };
        s.recv_acknack(ack);
        assert_eq!(s.in_flight_count(), 1);
        assert!(s.get_in_flight(SerialNumber16::new(2)).is_some());
    }

    #[test]
    fn recv_acknack_full_clear_when_no_bits_set() {
        let mut s = rs();
        for _ in 0..5 {
            s.submit(alloc::vec![0]).unwrap();
        }
        // base=5, bitmap=0 → everything acknowledged
        let ack = AckNackPayload {
            first_unacked_seq_num: 5,
            nack_bitmap: [0, 0],
            stream_id: 0x80,
        };
        s.recv_acknack(ack);
        assert_eq!(s.in_flight_count(), 0);
    }

    #[test]
    fn recv_data_buffers_in_order() {
        let mut s = rs();
        s.recv_data(SerialNumber16::new(0), alloc::vec![10])
            .unwrap();
        s.recv_data(SerialNumber16::new(1), alloc::vec![11])
            .unwrap();
        let drained = s.drain_in_order();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].0.raw(), 0);
        assert_eq!(drained[1].0.raw(), 1);
        assert_eq!(s.expected().raw(), 2);
    }

    #[test]
    fn recv_data_reorders_out_of_order() {
        let mut s = rs();
        s.recv_data(SerialNumber16::new(2), alloc::vec![22])
            .unwrap();
        s.recv_data(SerialNumber16::new(0), alloc::vec![20])
            .unwrap();
        // Without seq 1 we can only deliver 0, then block.
        let d1 = s.drain_in_order();
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0].0.raw(), 0);
        // deliver seq 1 belatedly
        s.recv_data(SerialNumber16::new(1), alloc::vec![21])
            .unwrap();
        let d2 = s.drain_in_order();
        assert_eq!(d2.len(), 2);
        assert_eq!(d2[0].0.raw(), 1);
        assert_eq!(d2[1].0.raw(), 2);
    }

    #[test]
    fn recv_data_drops_duplicates() {
        let mut s = rs();
        s.recv_data(SerialNumber16::new(0), alloc::vec![1]).unwrap();
        s.drain_in_order();
        // now seq 0 again → silently dropped
        s.recv_data(SerialNumber16::new(0), alloc::vec![99])
            .unwrap();
        assert_eq!(s.out_of_order_count(), 0);
    }

    #[test]
    fn recv_data_rejects_when_buffer_full() {
        let mut s = rs();
        // Buffer with 64 OOO samples (seq 1..=64 → expected is 0)
        for i in 1..=RECEIVER_BUFFER_CAP as u16 {
            s.recv_data(SerialNumber16::new(i), alloc::vec![1]).unwrap();
        }
        let res = s.recv_data(
            SerialNumber16::new(RECEIVER_BUFFER_CAP as u16 + 1),
            alloc::vec![1],
        );
        assert!(res.is_err());
    }

    #[test]
    fn pending_acknack_marks_missing_slots() {
        let mut s = rs();
        // expected=0; we get seq 1 + 3 → 0 and 2 missing
        s.recv_data(SerialNumber16::new(1), alloc::vec![1]).unwrap();
        s.recv_data(SerialNumber16::new(3), alloc::vec![3]).unwrap();
        let ack = s.pending_acknack(Some(SerialNumber16::new(3)));
        let bitmap = u16::from_le_bytes(ack.nack_bitmap);
        // bit 0 → seq 0 missing, bit 2 → seq 2 missing
        assert!(bitmap & (1 << 0) != 0);
        assert!(bitmap & (1 << 2) != 0);
        assert!(bitmap & (1 << 1) == 0); // seq 1 present
        assert!(bitmap & (1 << 3) == 0); // seq 3 present
    }

    #[test]
    fn reset_clears_state_completely() {
        let mut s = rs();
        s.submit(alloc::vec![1, 2]).unwrap();
        s.recv_data(SerialNumber16::new(0), alloc::vec![3]).unwrap();
        s.reset();
        assert_eq!(s.in_flight_count(), 0);
        assert_eq!(s.out_of_order_count(), 0);
        assert_eq!(s.expected().raw(), 0);
    }

    #[test]
    #[should_panic(expected = "reliable stream id")]
    fn constructor_panics_on_best_effort_stream() {
        let _ = ReliableStreamState::new(StreamId(1), ReliableConfig::default());
    }

    /// Spec §8.4.14 + §9.2 — end-to-end sender → receiver with
    /// Loss-Recovery via ACKNACK.
    ///
    /// Scenario:
    /// 1. Sender submits 3 payloads.
    /// 2. Receiver sees only seq=0 and seq=2 (seq=1 lost).
    /// 3. Receiver computes pending_acknack → marks seq=1 as
    ///    missing.
    /// 4. Sender calls recv_acknack(...) — clears the acked
    ///    slots, keeps seq=1.
    /// 5. Sender resends seq=1, receiver can
    ///    deliver all 3 samples via drain_in_order.
    #[test]
    fn end_to_end_sender_receiver_with_loss_recovery() {
        let mut sender = ReliableStreamState::new(StreamId(0x80), ReliableConfig::default());
        let mut receiver = ReliableStreamState::new(StreamId(0x80), ReliableConfig::default());

        // Sender submits 3 payloads
        let s0 = sender.submit(alloc::vec![10]).expect("submit 0");
        let s1 = sender.submit(alloc::vec![11]).expect("submit 1");
        let s2 = sender.submit(alloc::vec![12]).expect("submit 2");
        assert_eq!(sender.in_flight_count(), 3);

        // Receiver sees s0, s2; s1 lost
        receiver.recv_data(s0, alloc::vec![10]).expect("recv s0");
        receiver.recv_data(s2, alloc::vec![12]).expect("recv s2");

        // Drain delivers only s0 (s2 blocks due to the missing s1).
        let drained = receiver.drain_in_order();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].1, alloc::vec![10]);

        // Receiver computes pending_acknack → seq=1 missing.
        let acknack = receiver.pending_acknack(Some(s2));
        // Sender processes the ACKNACK — removes acked, keeps the missing.
        sender.recv_acknack(acknack);
        // s0 is acknowledged (drained), only s1 + s2 are in-flight
        // until the next ACKNACK; after the receiver's drain s2 is
        // in the buffer, but the sender does not know that. So the sender
        // can have at least s1 still in in_flight here.
        assert!(
            sender.get_in_flight(s1).is_some(),
            "s1 must be retransmittable"
        );

        // Sender retransmittet s1
        let s1_payload = sender.get_in_flight(s1).expect("s1 retx").to_vec();
        receiver.recv_data(s1, s1_payload).expect("recv retx s1");

        // Now drain delivers s1 + s2 in-order.
        let drained2 = receiver.drain_in_order();
        assert_eq!(drained2.len(), 2);
        assert_eq!(drained2[0].1, alloc::vec![11]);
        assert_eq!(drained2[1].1, alloc::vec![12]);
    }

    /// Spec §9.2 — Remote configuration via CREATE/DELETE/UPDATE
    /// uses the same reliable stream path for at-most-once
    /// delivery of the config submessages.
    #[test]
    fn config_submessages_delivered_in_order_via_reliable_stream() {
        // Demonstrates that the reliable-Stream is suitable for
        // CREATE/DELETE/UPDATE-Submessages (§9.2): in-order delivery
        // even when ACKNACK recovery is required.
        let mut sender = ReliableStreamState::new(StreamId(0x80), ReliableConfig::default());
        let mut receiver = ReliableStreamState::new(StreamId(0x80), ReliableConfig::default());

        // 5 simulated config operations.
        let mut seqs = Vec::new();
        for i in 0..5u8 {
            let seq = sender.submit(alloc::vec![i]).expect("submit");
            seqs.push(seq);
        }

        // Receiver gets them in random order: 2, 0, 4, 1, 3.
        let order = [2usize, 0, 4, 1, 3];
        for idx in order {
            receiver
                .recv_data(seqs[idx], alloc::vec![idx as u8])
                .expect("recv");
        }

        // drain_in_order guarantees spec-compliant order.
        let drained = receiver.drain_in_order();
        assert_eq!(drained.len(), 5);
        for (i, (_, payload)) in drained.iter().enumerate() {
            assert_eq!(payload, &alloc::vec![i as u8]);
        }
    }
}
