// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Reliable-Stream-State-Machine (Spec §8.4.10/§8.4.11).
//!
//! Pro reliable Stream (StreamId mit Bit 7 gesetzt → `id >= 128`)
//! laeuft eine `ReliableStreamState`, die folgende Aufgaben hat:
//!
//! - **Sender-Seite**: `submit(seq, payload)` puffert ausgehende
//!   `WRITE_DATA`-Bodies, sendet periodisch `HEARTBEAT` (`pending_heartbeat`).
//!   `recv_acknack(...)` raeumt bestaetigte Sequence-Numbers.
//!
//! - **Receiver-Seite**: `recv_data(seq, payload)` puffert eingehende
//!   Samples in einem Out-Of-Order-Buffer und liefert sie via
//!   `drain_in_order()` ab `expected_seq` aus. `pending_acknack()`
//!   meldet die fehlenden seqnrs als 16-Bit-Bitmap.
//!
//! Der Stream nutzt RFC-1982 16-Bit-Sequence-Numbers, was 32 768
//! gleichzeitig in-flight Samples zulaesst (Spec §8.3.2.3). Wir
//! deckeln das Sender-Window mit `SENDER_WINDOW_CAP = 16` (passt zur
//! 16-Bit-Bitmap des ACKNACK).

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::time::Duration;

use crate::error::XrceError;
use crate::header::StreamId;
use crate::serial_number::SerialNumber16;
use crate::submessages::{AckNackPayload, HeartbeatPayload};

/// Default-Heartbeat-Periode (Spec-Empfehlung 100 ms; konservativ
/// hier 500 ms, weil wir keine Tx-Pacing-Schicht darunter haben).
pub const DEFAULT_HEARTBEAT_PERIOD: Duration = Duration::from_millis(500);

/// Sender-Window-Cap: 16 Samples in-flight (passt zum 16-Bit
/// `nack_bitmap` im ACKNACK-Body).
pub const SENDER_WINDOW_CAP: usize = 16;

/// Receiver-Buffer-Cap: 64 out-of-order Samples (DoS-Schutz: ein
/// boeswilliger Sender koennte sonst beliebig viele Reorder-Buckets
/// allokieren).
pub const RECEIVER_BUFFER_CAP: usize = 64;

/// Pro-Sample-Payload-Cap: 64 KiB (= u16-Submessage-Length-Limit).
pub const RELIABLE_MAX_PAYLOAD: usize = 65_535;

/// Konfiguration des Reliable-Stream.
#[derive(Debug, Clone, Copy)]
pub struct ReliableConfig {
    /// Heartbeat-Periode (Sender → Receiver).
    pub heartbeat_period: Duration,
    /// Max. Sender-Window-Groesse (in-flight unbestaetigte Samples).
    pub sender_window: usize,
    /// Max. out-of-order Samples im Receiver-Buffer.
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

/// State-Machine eines reliable XRCE-Streams.
#[derive(Debug, Clone)]
pub struct ReliableStreamState {
    stream_id: StreamId,
    config: ReliableConfig,

    // -------- Sender-State -----------
    /// Naechste auszugebende Seqnr (monoton, RFC-1982).
    next_seq: SerialNumber16,
    /// In-flight Samples: seq → payload. BTreeMap, damit das
    /// First-/Last-Pair via `keys()` einfach ist.
    in_flight: BTreeMap<u16, Vec<u8>>,
    /// Letzter gesendeter Heartbeat (`uptime`-relativ).
    last_heartbeat: Option<Duration>,

    // -------- Receiver-State ---------
    /// Naechste erwartete eingehende Seqnr.
    expected_seq: SerialNumber16,
    /// Out-of-order Buffer: seq → payload.
    received: BTreeMap<u16, Vec<u8>>,
}

impl ReliableStreamState {
    /// Konstruktor.
    ///
    /// # Panics
    /// `stream_id` muss reliable sein (`is_reliable()`).
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

    /// Anzahl in-flight Samples auf der Sender-Seite.
    #[must_use]
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Anzahl out-of-order Samples auf der Receiver-Seite.
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

    /// Submit eines neuen Sample. Liefert die zugewiesene Seqnr.
    ///
    /// # Errors
    /// - `PayloadTooLarge`, wenn `payload.len() > RELIABLE_MAX_PAYLOAD`.
    /// - `ValueOutOfRange`, wenn das Sender-Window voll ist (`sender_window`
    ///   in-flight Samples). Caller muss vorher ACKNACKs verarbeiten.
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

    /// Lookup eines in-flight Payloads (z.B. fuer Resend).
    #[must_use]
    pub fn get_in_flight(&self, seq: SerialNumber16) -> Option<&[u8]> {
        self.in_flight.get(&seq.raw()).map(Vec::as_slice)
    }

    /// Tick: liefert `Some(HEARTBEAT)`, falls die Heartbeat-Periode
    /// abgelaufen ist und in-flight Samples existieren.
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
            // i16 reinterpret-cast — RFC-1982-Vergleich passiert auf
            // Empfaenger-Seite via `wrapping_*`.
            first_unacked_seq_nr: first as i16,
            last_unacked_seq_nr: last as i16,
            stream_id: self.stream_id.0,
        })
    }

    /// Verarbeitet eingehendes ACKNACK auf der Sender-Seite.
    ///
    /// `first_unacked` ist die kleinste seqnr, die der Receiver noch
    /// erwartet; alles strikt davor wird als bestaetigt entfernt.
    /// `nack_bitmap` ist 16-Bit; Bit `i` = "seqnr `first_unacked + i`
    /// fehlt noch". Wir entfernen also alle Samples `< first_unacked`
    /// und alle Samples in `[first_unacked, first_unacked+16)`, deren
    /// Bit nicht gesetzt ist.
    pub fn recv_acknack(&mut self, payload: AckNackPayload) {
        let base = payload.first_unacked_seq_num as u16;
        let bitmap = u16::from_le_bytes(payload.nack_bitmap);

        // 1) Alles vor base bestaetigen.
        let to_remove: Vec<u16> = self
            .in_flight
            .keys()
            .copied()
            .filter(|&k| {
                let diff = base.wrapping_sub(k);
                // k < base nach RFC-1982?
                diff > 0 && diff < SerialNumber16::HALF_WINDOW
            })
            .collect();
        for k in to_remove {
            self.in_flight.remove(&k);
        }

        // 2) Innerhalb des Bitmap-Fensters: Bit gesetzt → fehlt → behalten.
        //    Bit nicht gesetzt → bestaetigt → loeschen.
        for i in 0u16..16 {
            let seq = base.wrapping_add(i);
            let bit = (bitmap >> i) & 1;
            if bit == 0 {
                // bestaetigt
                self.in_flight.remove(&seq);
            }
        }
    }

    // ---------------------------------------------------------------
    // Receiver-Seite
    // ---------------------------------------------------------------

    /// Receiver: ein Sample mit `seq + payload` ist eingelaufen.
    /// Speichert es im Out-Of-Order-Buffer (oder verwirft als Duplikat).
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn der Receiver-Buffer voll ist (DoS-Schutz).
    pub fn recv_data(&mut self, seq: SerialNumber16, payload: Vec<u8>) -> Result<(), XrceError> {
        // Schon zugestellt (vor expected_seq)?
        if seq.wrapping_lt(self.expected_seq) {
            return Ok(()); // Duplikat → silently drop
        }
        if self.received.contains_key(&seq.raw()) {
            return Ok(()); // schon im Buffer
        }
        if self.received.len() >= self.config.receiver_buffer {
            return Err(XrceError::ValueOutOfRange {
                message: "reliable receiver buffer full",
            });
        }
        self.received.insert(seq.raw(), payload);
        Ok(())
    }

    /// Receiver: liefert alle in-Order verfuegbaren Samples ab
    /// `expected_seq` und schiebt `expected_seq` weiter.
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

    /// Receiver: berechnet das passende ACKNACK-Payload, das die
    /// fehlenden Seqnrs ab `expected_seq` markiert. Liefert
    /// `Some(ACKNACK)`, wenn out-of-order Samples vorliegen ODER
    /// `last_recv_seen` (HEARTBEAT-Hint) eine Luecke offenlegt.
    #[must_use]
    pub fn pending_acknack(&self, hint_last_seen: Option<SerialNumber16>) -> AckNackPayload {
        let base = self.expected_seq;
        let mut bitmap: u16 = 0;
        // Wir markieren ALLE Slots im Fenster als fehlend, die NICHT im
        // received-Buffer liegen — dabei ist das Fenster
        // [base, base+16). Wenn `hint_last_seen` gegeben ist, werden
        // Slots strikt nach `hint_last_seen` als nicht-fehlend behandelt.
        for i in 0u16..16 {
            let seq = base.next().0.wrapping_sub(1).wrapping_add(i);
            let s = SerialNumber16::new(seq);
            // skip falls > hint_last_seen
            if let Some(h) = hint_last_seen {
                if s.wrapping_gt(h) {
                    continue;
                }
            }
            // missing falls nicht im received-Buffer
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

    /// Setzt den Stream-State zurueck (z.B. nach `RESET`-Submessage).
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
        // direkt danach: noch keine 500ms vergangen
        assert!(s.pending_heartbeat(Duration::from_millis(100)).is_none());
        // nach 600ms: ja
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
        // base=2, bitmap=0b0001 → seq 2 fehlt, also alles davor (0,1) bestaetigt
        // und seq 2 selbst markiert als noch fehlend
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
        // base=5, bitmap=0 → alles bestaetigt
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
        // Ohne seq 1 koennen wir nur 0 liefern, dann blockieren.
        let d1 = s.drain_in_order();
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0].0.raw(), 0);
        // seq 1 nachreichen
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
        // jetzt nochmal seq 0 → silently dropped
        s.recv_data(SerialNumber16::new(0), alloc::vec![99])
            .unwrap();
        assert_eq!(s.out_of_order_count(), 0);
    }

    #[test]
    fn recv_data_rejects_when_buffer_full() {
        let mut s = rs();
        // Buffer mit 64 OOO-Samples (seq 1..=64 → expected ist 0)
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
        // expected=0; wir kriegen seq 1 + 3 → 0 und 2 fehlen
        s.recv_data(SerialNumber16::new(1), alloc::vec![1]).unwrap();
        s.recv_data(SerialNumber16::new(3), alloc::vec![3]).unwrap();
        let ack = s.pending_acknack(Some(SerialNumber16::new(3)));
        let bitmap = u16::from_le_bytes(ack.nack_bitmap);
        // bit 0 → seq 0 fehlt, bit 2 → seq 2 fehlt
        assert!(bitmap & (1 << 0) != 0);
        assert!(bitmap & (1 << 2) != 0);
        assert!(bitmap & (1 << 1) == 0); // seq 1 da
        assert!(bitmap & (1 << 3) == 0); // seq 3 da
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

    /// Spec §8.4.14 + §9.2 — End-to-End Sender → Receiver mit
    /// Loss-Recovery via ACKNACK.
    ///
    /// Szenario:
    /// 1. Sender submit'tet 3 Payloads.
    /// 2. Receiver sieht nur seq=0 und seq=2 (seq=1 verloren).
    /// 3. Receiver berechnet pending_acknack → markiert seq=1 als
    ///    fehlend.
    /// 4. Sender ruft recv_acknack(...) auf — clearred die acked
    ///    Slots, behaelt seq=1.
    /// 5. Sender sendet seq=1 erneut, Receiver kann
    ///    drain_in_order alle 3 Samples liefern.
    #[test]
    fn end_to_end_sender_receiver_with_loss_recovery() {
        let mut sender = ReliableStreamState::new(StreamId(0x80), ReliableConfig::default());
        let mut receiver = ReliableStreamState::new(StreamId(0x80), ReliableConfig::default());

        // Sender submit'tet 3 Payloads
        let s0 = sender.submit(alloc::vec![10]).expect("submit 0");
        let s1 = sender.submit(alloc::vec![11]).expect("submit 1");
        let s2 = sender.submit(alloc::vec![12]).expect("submit 2");
        assert_eq!(sender.in_flight_count(), 3);

        // Receiver sieht s0, s2; s1 verloren
        receiver.recv_data(s0, alloc::vec![10]).expect("recv s0");
        receiver.recv_data(s2, alloc::vec![12]).expect("recv s2");

        // Drain liefert nur s0 (s2 blockt wegen fehlendem s1).
        let drained = receiver.drain_in_order();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].1, alloc::vec![10]);

        // Receiver berechnet pending_acknack → seq=1 fehlt.
        let acknack = receiver.pending_acknack(Some(s2));
        // Sender verarbeitet ACKNACK — entfernt acked, behaelt fehlende.
        sender.recv_acknack(acknack);
        // s0 ist acknowledged (drained), nur s1 + s2 sind in-flight
        // bis zum naechsten ACKNACK; nach Receiver's drain liegt s2 zwar
        // im Buffer, aber Sender kennt das nicht. Daher kann der Sender
        // hier mindestens s1 noch im in_flight haben.
        assert!(
            sender.get_in_flight(s1).is_some(),
            "s1 muss retransmittable sein"
        );

        // Sender retransmittet s1
        let s1_payload = sender.get_in_flight(s1).expect("s1 retx").to_vec();
        receiver.recv_data(s1, s1_payload).expect("recv retx s1");

        // Jetzt drain liefert s1 + s2 in-order.
        let drained2 = receiver.drain_in_order();
        assert_eq!(drained2.len(), 2);
        assert_eq!(drained2[0].1, alloc::vec![11]);
        assert_eq!(drained2[1].1, alloc::vec![12]);
    }

    /// Spec §9.2 — Remote-Configuration via CREATE/DELETE/UPDATE
    /// nutzt denselben reliable-Stream-Pfad fuer at-most-once-
    /// Delivery der Config-Submessages.
    #[test]
    fn config_submessages_delivered_in_order_via_reliable_stream() {
        // Demonstrates that the reliable-Stream is suitable for
        // CREATE/DELETE/UPDATE-Submessages (§9.2): in-order delivery
        // even when ACKNACK-Recovery erforderlich.
        let mut sender = ReliableStreamState::new(StreamId(0x80), ReliableConfig::default());
        let mut receiver = ReliableStreamState::new(StreamId(0x80), ReliableConfig::default());

        // 5 simulierte Config-Operations.
        let mut seqs = Vec::new();
        for i in 0..5u8 {
            let seq = sender.submit(alloc::vec![i]).expect("submit");
            seqs.push(seq);
        }

        // Receiver bekommt sie in zufaelliger Reihenfolge: 2, 0, 4, 1, 3.
        let order = [2usize, 0, 4, 1, 3];
        for idx in order {
            receiver
                .recv_data(seqs[idx], alloc::vec![idx as u8])
                .expect("recv");
        }

        // drain_in_order garantiert Spec-konforme Reihenfolge.
        let drained = receiver.drain_in_order();
        assert_eq!(drained.len(), 5);
        for (i, (_, payload)) in drained.iter().enumerate() {
            assert_eq!(payload, &alloc::vec![i as u8]);
        }
    }
}
