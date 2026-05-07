// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Writer-Liveliness-Protocol (WLP) — DCPS-Runtime-Wiring.
//!
//! Implementiert DDSI-RTPS 2.5 §8.4.13 und §9.6.3.1 (Wire-Format der
//! `ParticipantMessageData`). Ein [`WlpEndpoint`] kapselt einen
//! WLP-Writer + WLP-Reader pro Participant. Der Writer sendet
//! periodische AUTOMATIC-Heartbeats (`lease_duration / 3`), Reader
//! aktualisiert per-Peer-Last-Seen-Timestamps.
//!
//! # Liveliness-Kinds (DDS DCPS 1.4 §2.2.3.11)
//!
//! - `AUTOMATIC` — Tick-getrieben, jeder periodische Beat. Implicit
//!   "ich lebe noch", solange irgendein Tick durchkommt.
//! - `MANUAL_BY_PARTICIPANT` — `assert_liveliness()` auf dem
//!   `DomainParticipant` triggert genau einen Wire-Send mit `kind = 1`.
//! - `MANUAL_BY_TOPIC` — `assert_liveliness()` auf dem `DataWriter`
//!   triggert einen Vendor-Kind-Send (`kind = 0x80000001`,
//!   ZeroDDS-Spezifisch) mit Topic-Token in `data`.
//!
//! # Architektur
//!
//! Der [`WlpEndpoint`] bleibt **stateless** bezgl. Reliable-State —
//! WLP-Heartbeats sind Best-Effort (Spec §8.4.13: "best effort
//! reliability is sufficient"). Der Writer ist deshalb keine
//! `ReliableWriter`, sondern ein simpler `next_sn`-Counter mit
//! `encode_data_datagram`. Das macht den Hot-Path billig und vermeidet
//! Kopplung mit der HEARTBEAT/ACKNACK-Loop des SEDP-Stacks.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram, encode_data_datagram};
use zerodds_rtps::error::WireError;
use zerodds_rtps::header::RtpsHeader;
use zerodds_rtps::participant_message_data::{
    PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
    PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
    PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC, ParticipantMessageData,
};
use zerodds_rtps::submessages::DataSubmessage;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix, SequenceNumber, VendorId};

/// DoS-Cap fuer die Manual-Pulse-Queue. Wenn ein Caller schneller
/// `assert_liveliness()` ruft als der Tick-Loop das abarbeitet, droppen
/// wir alte Pulse — ein Reader merkt sich nur den letzten Beat.
pub const MAX_QUEUED_PULSES: usize = 32;

/// DoS-Cap fuer die Anzahl bekannter remote Peer-Participants.
/// Skaliert mit Domain-Groesse; mehr als 1024 ist im Single-Domain-
/// Anti-Pattern und Hinweis auf Misuse.
pub const MAX_TRACKED_PEERS: usize = 1024;

/// State eines pendenden Manual-Pulses. Wird vom Tick verbraucht.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingPulse {
    kind: u32,
    data: Vec<u8>,
}

/// Pro-Peer-Tracking-State im Reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerLivelinessState {
    /// Letztes empfangenes WLP-Heartbeat (relative Zeit zum
    /// `WlpEndpoint`-Owner-Start).
    pub last_seen: Duration,
    /// Letzter empfangener `kind` — fuer Diagnose, ob Peer
    /// AUTOMATIC oder MANUAL geschickt hat.
    pub last_kind: u32,
}

/// WLP-Endpoint — Writer + Reader fuer DCPSParticipantMessage.
///
/// Owner ist die [`crate::runtime::DcpsRuntime`]. Der Endpoint wird unter dem Lock
/// der Runtime mutiert; alle Methoden nehmen `&mut self`.
#[derive(Debug)]
pub struct WlpEndpoint {
    /// Eigener Participant-Prefix (Source der Heartbeats).
    own_prefix: GuidPrefix,
    /// VendorId fuer den RTPS-Header.
    vendor_id: VendorId,
    /// Naechste Sequence-Number fuer den WLP-Writer.
    next_sn: i64,
    /// Tick-Periode fuer AUTOMATIC-Heartbeats (typisch
    /// `lease_duration / 3`).
    tick_period: Duration,
    /// Zeitpunkt des naechsten faelligen AUTOMATIC-Beats.
    next_tick: Duration,
    /// Manual-Pulses, die noch nicht ausgesendet sind.
    pending: VecDeque<PendingPulse>,
    /// Per-Peer-Tracking (GuidPrefix → State).
    peers: BTreeMap<GuidPrefix, PeerLivelinessState>,
}

impl WlpEndpoint {
    /// Konstruktor.
    ///
    /// # Panics
    /// Nicht — `tick_period == ZERO` ist erlaubt (deaktiviert
    /// AUTOMATIC-Beats).
    #[must_use]
    pub fn new(own_prefix: GuidPrefix, vendor_id: VendorId, tick_period: Duration) -> Self {
        Self {
            own_prefix,
            vendor_id,
            next_sn: 1,
            tick_period,
            next_tick: Duration::ZERO,
            pending: VecDeque::new(),
            peers: BTreeMap::new(),
        }
    }

    /// Setzt eine neue Tick-Periode (z.B. nach Lease-Aenderung).
    /// Setzt den naechsten Beat-Slot zurueck auf "sofort", damit eine
    /// Kuerzung der Periode unmittelbar wirkt (sonst wartet der
    /// Endpoint noch das alte Intervall ab).
    pub fn set_tick_period(&mut self, period: Duration) {
        self.tick_period = period;
        self.next_tick = Duration::ZERO;
    }

    /// `assert_liveliness()` auf dem `DomainParticipant` — enqueued
    /// einen Manual-By-Participant-Pulse, der beim naechsten
    /// `tick()` als WLP-DATA rausgeht. Idempotent ueber den
    /// Tick-Slot — Mehrfachaufruf binnen einer Tick-Periode fuehrt
    /// zu mehreren Pulses (Spec sagt nichts ueber Coalescing, aber
    /// wir cappen mit [`MAX_QUEUED_PULSES`]).
    pub fn assert_participant(&mut self) {
        self.enqueue_pulse(PendingPulse {
            kind: PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
            data: Vec::new(),
        });
    }

    /// `assert_liveliness()` auf einem `DataWriter` — enqueued einen
    /// MANUAL_BY_TOPIC-Pulse mit dem gegebenen Topic-Token.
    /// Spec-Compliance: ZeroDDS-Vendor-Kind, Cyclone ignoriert das
    /// (MSB-set → "ignore unknown kind", §9.6.3.1).
    pub fn assert_topic(&mut self, topic_token: Vec<u8>) {
        self.enqueue_pulse(PendingPulse {
            kind: PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC,
            data: topic_token,
        });
    }

    fn enqueue_pulse(&mut self, p: PendingPulse) {
        if self.pending.len() >= MAX_QUEUED_PULSES {
            // Drop oldest — neuer Pulse ist relevanter.
            let _ = self.pending.pop_front();
        }
        self.pending.push_back(p);
    }

    /// Tick. Liefert ggf. ein Datagram, das gesendet werden soll.
    /// Reihenfolge:
    /// 1. Pending Manual-Pulses werden zuerst ausgesendet (einer pro
    ///    Tick — bei voller Queue dauert die Drainage `n * tick_period`).
    /// 2. Wenn `now >= next_tick` und `tick_period > 0`, sendet einen
    ///    AUTOMATIC-Heartbeat und schedule den naechsten Tick.
    ///
    /// `None` heisst: nichts zu senden in diesem Tick.
    ///
    /// # Errors
    /// `WireError` wenn das Encoding fehlschlaegt (z.B. SN-Overflow).
    pub fn tick(&mut self, now: Duration) -> Result<Option<Vec<u8>>, WireError> {
        if let Some(pulse) = self.pending.pop_front() {
            return Ok(Some(self.encode_pulse(&pulse)?));
        }
        if self.tick_period.is_zero() {
            return Ok(None);
        }
        if now < self.next_tick {
            return Ok(None);
        }
        // AUTOMATIC-Beat
        let pulse = PendingPulse {
            kind: PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
            data: Vec::new(),
        };
        let datagram = self.encode_pulse(&pulse)?;
        self.next_tick = now + self.tick_period;
        Ok(Some(datagram))
    }

    fn encode_pulse(&mut self, pulse: &PendingPulse) -> Result<Vec<u8>, WireError> {
        let mut msg = ParticipantMessageData::automatic(self.own_prefix);
        msg.kind = pulse.kind;
        msg.data = pulse.data.clone();
        let payload = msg.to_cdr(true)?; // LE per Default
        let sn = SequenceNumber(self.next_sn);
        self.next_sn = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "wlp sequence overflow",
            })?;
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER,
            writer_id: EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER,
            writer_sn: sn,
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let header = RtpsHeader::new(self.vendor_id, self.own_prefix);
        encode_data_datagram(header, &[data])
    }

    /// Verarbeitet ein eingehendes Datagram. Wenn es eine WLP-DATA-
    /// Submessage enthaelt (writer_id == BUILTIN_PARTICIPANT_MESSAGE_
    /// WRITER), wird der Per-Peer-Last-Seen-Timestamp aktualisiert.
    /// Andere Datagramme werden ignoriert.
    ///
    /// # Errors
    /// `WireError` wenn das aeussere Datagram malformed ist. Eine
    /// einzelne malformed Submessage fuehrt nicht zum Fehler — wir
    /// skippen sie still.
    pub fn handle_datagram(&mut self, bytes: &[u8], now: Duration) -> Result<bool, WireError> {
        let parsed = decode_datagram(bytes)?;
        let mut updated = false;
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                if d.writer_id == EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER {
                    if let Ok(msg) = ParticipantMessageData::from_cdr(&d.serialized_payload) {
                        // Quelle: bevorzugt der Prefix aus dem Payload
                        // (wie Cyclone macht), fallback der RTPS-Header-
                        // Prefix wenn der Payload den nicht traegt.
                        let src = msg.prefix();
                        let prefix = if src == GuidPrefix::UNKNOWN {
                            parsed.header.guid_prefix
                        } else {
                            src
                        };
                        // DoS-Cap: Anzahl Peers begrenzen.
                        if self.peers.len() >= MAX_TRACKED_PEERS
                            && !self.peers.contains_key(&prefix)
                        {
                            // Drop: bekannten Peer-Liste ist voll.
                            continue;
                        }
                        self.peers.insert(
                            prefix,
                            PeerLivelinessState {
                                last_seen: now,
                                last_kind: msg.kind,
                            },
                        );
                        updated = true;
                    }
                }
            }
        }
        Ok(updated)
    }

    /// Liefert den aktuellen Liveliness-State eines bekannten Peers.
    /// `None` wenn unbekannt.
    #[must_use]
    pub fn peer_state(&self, prefix: &GuidPrefix) -> Option<&PeerLivelinessState> {
        self.peers.get(prefix)
    }

    /// Anzahl getrackter Peers.
    #[must_use]
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Iter ueber alle Peers, deren Last-Seen aelter als `now - lease`
    /// ist. Caller nutzt das, um Liveliness-Lost-Events zu treiben.
    pub fn lost_peers(
        &self,
        now: Duration,
        lease: Duration,
    ) -> impl Iterator<Item = (&GuidPrefix, &PeerLivelinessState)> + '_ {
        self.peers.iter().filter(move |(_, s)| {
            now.checked_sub(s.last_seen)
                .is_some_and(|elapsed| elapsed > lease)
        })
    }

    /// Entfernt einen Peer aus dem Tracking (z.B. bei SPDP-Lease-
    /// Expire).
    pub fn forget_peer(&mut self, prefix: &GuidPrefix) {
        self.peers.remove(prefix);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use alloc::vec;

    fn ep() -> WlpEndpoint {
        WlpEndpoint::new(
            GuidPrefix::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
            VendorId::ZERODDS,
            Duration::from_millis(300),
        )
    }

    #[test]
    fn wlp_first_tick_emits_automatic_heartbeat() {
        let mut e = ep();
        let dg = e.tick(Duration::ZERO).unwrap();
        assert!(dg.is_some(), "first tick must emit AUTOMATIC beat");
    }

    #[test]
    fn wlp_tick_idle_returns_none_until_period() {
        let mut e = ep();
        let _ = e.tick(Duration::ZERO).unwrap();
        // Kein zweiter Beat innerhalb der Periode.
        let dg = e.tick(Duration::from_millis(100)).unwrap();
        assert!(dg.is_none());
    }

    #[test]
    fn wlp_tick_emits_again_after_period() {
        let mut e = ep();
        let _ = e.tick(Duration::ZERO).unwrap();
        let dg = e.tick(Duration::from_millis(400)).unwrap();
        assert!(dg.is_some());
    }

    #[test]
    fn wlp_zero_period_disables_automatic_beats() {
        let mut e = WlpEndpoint::new(
            GuidPrefix::from_bytes([0xAA; 12]),
            VendorId::ZERODDS,
            Duration::ZERO,
        );
        let dg = e.tick(Duration::ZERO).unwrap();
        assert!(dg.is_none());
    }

    #[test]
    fn wlp_assert_participant_emits_manual_pulse() {
        // Tick-Period 1h damit der AUTOMATIC-Pfad nicht stoert.
        let mut e = WlpEndpoint::new(
            GuidPrefix::from_bytes([1; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        // Erstmal initialer AUTOMATIC-Beat zum Initialisieren.
        let _ = e.tick(Duration::ZERO).unwrap();
        e.assert_participant();
        let dg = e.tick(Duration::from_millis(1)).unwrap().expect("manual");
        // Datagram dekodieren und kind pruefen.
        let parsed = decode_datagram(&dg).unwrap();
        let data_sub = parsed.submessages.iter().find_map(|s| match s {
            ParsedSubmessage::Data(d) => Some(d),
            _ => None,
        });
        let payload = &data_sub.expect("DATA").serialized_payload;
        let m = ParticipantMessageData::from_cdr(payload).unwrap();
        assert_eq!(
            m.kind,
            PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE
        );
    }

    #[test]
    fn wlp_assert_topic_emits_vendor_kind_with_token() {
        let mut e = WlpEndpoint::new(
            GuidPrefix::from_bytes([2; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        let _ = e.tick(Duration::ZERO).unwrap();
        e.assert_topic(vec![0xAA, 0xBB]);
        let dg = e.tick(Duration::from_millis(1)).unwrap().expect("manual");
        let parsed = decode_datagram(&dg).unwrap();
        let data_sub = parsed
            .submessages
            .iter()
            .find_map(|s| match s {
                ParsedSubmessage::Data(d) => Some(d),
                _ => None,
            })
            .unwrap();
        let m = ParticipantMessageData::from_cdr(&data_sub.serialized_payload).unwrap();
        assert_eq!(
            m.kind,
            PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC
        );
        assert_eq!(m.data, vec![0xAA, 0xBB]);
    }

    #[test]
    fn wlp_pending_queue_caps_at_max() {
        let mut e = ep();
        for _ in 0..(MAX_QUEUED_PULSES + 10) {
            e.assert_participant();
        }
        assert_eq!(e.pending.len(), MAX_QUEUED_PULSES);
    }

    #[test]
    fn wlp_handle_datagram_updates_peer_state() {
        let mut sender = ep();
        let mut receiver = WlpEndpoint::new(
            GuidPrefix::from_bytes([99; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        let dg = sender.tick(Duration::ZERO).unwrap().unwrap();
        let updated = receiver
            .handle_datagram(&dg, Duration::from_millis(50))
            .unwrap();
        assert!(updated);
        assert_eq!(receiver.peer_count(), 1);
        let state = receiver
            .peer_state(&GuidPrefix::from_bytes([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
            ]))
            .unwrap();
        assert_eq!(
            state.last_kind,
            PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE
        );
        assert_eq!(state.last_seen, Duration::from_millis(50));
    }

    #[test]
    fn wlp_handle_datagram_ignores_non_wlp_traffic() {
        // Eine SPDP-DATA-Submessage darf NICHT als WLP gewertet werden.
        let header = RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::from_bytes([5; 12]));
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SPDP_BUILTIN_PARTICIPANT_READER,
            writer_id: EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: vec![0u8; 8].into(),
        };
        let dg = encode_data_datagram(header, &[data]).unwrap();
        let mut e = ep();
        let updated = e.handle_datagram(&dg, Duration::from_millis(10)).unwrap();
        assert!(!updated);
        assert_eq!(e.peer_count(), 0);
    }

    #[test]
    fn wlp_lost_peers_returns_only_expired() {
        let mut sender = ep();
        let mut receiver = WlpEndpoint::new(
            GuidPrefix::from_bytes([99; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        let dg = sender.tick(Duration::ZERO).unwrap().unwrap();
        receiver
            .handle_datagram(&dg, Duration::from_millis(100))
            .unwrap();
        // Lease 200 ms. now = 250 ms → 150 ms elapsed → lost.
        let lost: Vec<_> = receiver
            .lost_peers(Duration::from_millis(350), Duration::from_millis(200))
            .collect();
        assert_eq!(lost.len(), 1);
        // now = 200 ms → 100 ms elapsed → noch alive.
        let alive: Vec<_> = receiver
            .lost_peers(Duration::from_millis(200), Duration::from_millis(200))
            .collect();
        assert_eq!(alive.len(), 0);
    }

    #[test]
    fn wlp_forget_peer_removes_state() {
        let mut sender = ep();
        let mut receiver = WlpEndpoint::new(
            GuidPrefix::from_bytes([99; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        let dg = sender.tick(Duration::ZERO).unwrap().unwrap();
        receiver
            .handle_datagram(&dg, Duration::from_millis(0))
            .unwrap();
        let prefix = GuidPrefix::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        receiver.forget_peer(&prefix);
        assert!(receiver.peer_state(&prefix).is_none());
    }

    #[test]
    fn wlp_set_tick_period_takes_effect() {
        let mut e = ep();
        let _ = e.tick(Duration::ZERO).unwrap();
        e.set_tick_period(Duration::from_millis(50));
        // Mit alten 300 ms wuerde 100 ms keinen Beat triggern;
        // mit 50 ms muss bei 100 ms ein Beat kommen.
        let dg = e.tick(Duration::from_millis(100)).unwrap();
        assert!(dg.is_some());
    }

    #[test]
    fn wlp_handle_datagram_uses_header_prefix_when_payload_unknown() {
        // Wenn der Payload-GuidPrefix == UNKNOWN ist (zero-bytes),
        // muss der Endpoint den RTPS-Header-Prefix als Fallback
        // benutzen.
        // Wir bauen ein WLP-Datagram manuell mit Zero-Prefix-Payload.
        let mut msg = ParticipantMessageData::automatic(GuidPrefix::UNKNOWN);
        msg.kind = PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE;
        let payload = msg.to_cdr(true).unwrap();
        let header_prefix = GuidPrefix::from_bytes([0x77; 12]);
        let header = RtpsHeader::new(VendorId::ZERODDS, header_prefix);
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER,
            writer_id: EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let dg = encode_data_datagram(header, &[data]).unwrap();

        let mut receiver = WlpEndpoint::new(
            GuidPrefix::from_bytes([99; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        let updated = receiver
            .handle_datagram(&dg, Duration::from_millis(7))
            .unwrap();
        assert!(updated);
        // Fallback-Prefix aus RTPS-Header muss als Peer-Key verwendet
        // sein.
        assert!(receiver.peer_state(&header_prefix).is_some());
    }

    #[test]
    fn wlp_handle_datagram_skips_malformed_cdr() {
        // WLP-Submessage mit 3-Byte-Payload (zu klein fuer
        // Encapsulation-Header). handle_datagram darf nicht
        // panicen, sondern still skippen.
        let header = RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::from_bytes([0xAB; 12]));
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER,
            writer_id: EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: vec![0u8; 3].into(),
        };
        let dg = encode_data_datagram(header, &[data]).unwrap();
        let mut e = ep();
        let updated = e.handle_datagram(&dg, Duration::from_millis(5)).unwrap();
        assert!(!updated);
        assert_eq!(e.peer_count(), 0);
    }

    #[test]
    fn wlp_pulses_drained_one_per_tick() {
        let mut e = WlpEndpoint::new(
            GuidPrefix::from_bytes([3; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        let _ = e.tick(Duration::ZERO).unwrap();
        e.assert_participant();
        e.assert_participant();
        let dg1 = e.tick(Duration::from_millis(1)).unwrap();
        let dg2 = e.tick(Duration::from_millis(2)).unwrap();
        let dg3 = e.tick(Duration::from_millis(3)).unwrap();
        assert!(dg1.is_some());
        assert!(dg2.is_some());
        assert!(dg3.is_none(), "queue empty after 2 pulses");
    }
}
