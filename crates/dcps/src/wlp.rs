// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Writer Liveliness Protocol (WLP) — DCPS runtime wiring.
//!
//! Implements DDSI-RTPS 2.5 §8.4.13 and §9.6.3.1 (wire format of
//! `ParticipantMessageData`). A [`WlpEndpoint`] encapsulates one WLP
//! writer + WLP reader per participant. The writer sends periodic
//! AUTOMATIC heartbeats (`lease_duration / 3`), the reader updates
//! per-peer last-seen timestamps.
//!
//! # Liveliness kinds (DDS DCPS 1.4 §2.2.3.11)
//!
//! - `AUTOMATIC` — tick-driven, every periodic beat. Implicitly "I'm
//!   still alive" as long as any tick gets through.
//! - `MANUAL_BY_PARTICIPANT` — `assert_liveliness()` on the
//!   `DomainParticipant` triggers exactly one wire send with `kind = 1`.
//! - `MANUAL_BY_TOPIC` — `assert_liveliness()` on the `DataWriter`
//!   triggers a vendor-kind send (`kind = 0x80000001`, ZeroDDS-specific)
//!   with a topic token in `data`.
//!
//! # Architecture
//!
//! The [`WlpEndpoint`] stays **stateless** with respect to reliable
//! state — WLP heartbeats are best-effort (spec §8.4.13: "best effort
//! reliability is sufficient"). The writer is therefore not a
//! `ReliableWriter` but a simple `next_sn` counter with
//! `encode_data_datagram`. This keeps the hot path cheap and avoids
//! coupling with the SEDP stack's HEARTBEAT/ACKNACK loop.

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

/// DoS cap for the manual-pulse queue. If a caller invokes
/// `assert_liveliness()` faster than the tick loop processes it, we drop
/// old pulses — a reader only remembers the last beat.
pub const MAX_QUEUED_PULSES: usize = 32;

/// DoS cap for the number of known remote peer participants.
/// Scales with domain size; more than 1024 is a single-domain
/// anti-pattern and a sign of misuse.
pub const MAX_TRACKED_PEERS: usize = 1024;

/// State of a pending manual pulse. Consumed by the tick.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingPulse {
    kind: u32,
    data: Vec<u8>,
}

/// Per-peer tracking state in the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerLivelinessState {
    /// Last received WLP heartbeat (time relative to the
    /// `WlpEndpoint` owner's start).
    pub last_seen: Duration,
    /// Last received `kind` — for diagnostics, whether the peer sent
    /// AUTOMATIC or MANUAL.
    pub last_kind: u32,
}

/// WLP endpoint — writer + reader for DCPSParticipantMessage.
///
/// Owned by the [`crate::runtime::DcpsRuntime`]. The endpoint is mutated
/// under the runtime lock; all methods take `&mut self`.
#[derive(Debug)]
pub struct WlpEndpoint {
    /// Own participant prefix (source of the heartbeats).
    own_prefix: GuidPrefix,
    /// VendorId for the RTPS header.
    vendor_id: VendorId,
    /// Next sequence number for the WLP writer.
    next_sn: i64,
    /// Tick period for AUTOMATIC heartbeats (typically
    /// `lease_duration / 3`).
    tick_period: Duration,
    /// Time of the next due AUTOMATIC beat.
    next_tick: Duration,
    /// Manual pulses that have not yet been sent.
    pending: VecDeque<PendingPulse>,
    /// Per-peer tracking (GuidPrefix → state).
    peers: BTreeMap<GuidPrefix, PeerLivelinessState>,
    /// Sent-beat history (seq → plain DATA datagram) for reliable resend.
    /// cyclone treats the (secure-)WLP as RELIABLE and NACKs missing
    /// beats; without a resend it would never get the liveliness assertion and
    /// considers our writer not-alive. Capped to the last 32 beats.
    sent: BTreeMap<i64, Vec<u8>>,
    /// `true` if `liveliness_protection != NONE` — then the endpoint sends
    /// over the secure-WLP entity `BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER`
    /// (0xff0200c3, DDS-Security §8.4.2.4 / §7.4.7.1 Tab.7) instead of the plain
    /// entity. The runtime sets the flag when enabling the security plugin.
    secure: bool,
}

impl WlpEndpoint {
    /// Constructor.
    ///
    /// # Panics
    /// Never — `tick_period == ZERO` is allowed (disables AUTOMATIC
    /// beats).
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
            sent: BTreeMap::new(),
            secure: false,
        }
    }

    /// Switches the endpoint to the secure-WLP entity (send via
    /// `BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER`). Set by the runtime
    /// when `liveliness_protection != NONE`. The receive path matches
    /// both entities anyway, independent of this flag.
    pub fn set_secure(&mut self, secure: bool) {
        self.secure = secure;
    }

    /// Sets a new tick period (e.g. after a lease change).
    /// Resets the next beat slot to "immediately", so a
    /// shortening of the period takes effect at once (otherwise the
    /// endpoint still waits out the old interval).
    pub fn set_tick_period(&mut self, period: Duration) {
        self.tick_period = period;
        self.next_tick = Duration::ZERO;
    }

    /// `assert_liveliness()` on the `DomainParticipant` — enqueues a
    /// manual-by-participant pulse that goes out as WLP DATA on the next
    /// `tick()`. Idempotent over the tick slot — multiple calls within a
    /// tick period produce multiple pulses (the spec says nothing about
    /// coalescing, but we cap with [`MAX_QUEUED_PULSES`]).
    pub fn assert_participant(&mut self) {
        self.enqueue_pulse(PendingPulse {
            kind: PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
            data: Vec::new(),
        });
    }

    /// `assert_liveliness()` on a `DataWriter` — enqueues a
    /// MANUAL_BY_TOPIC pulse with the given topic token.
    /// Spec compliance: ZeroDDS vendor kind, Cyclone ignores it
    /// (MSB set → "ignore unknown kind", §9.6.3.1).
    pub fn assert_topic(&mut self, topic_token: Vec<u8>) {
        self.enqueue_pulse(PendingPulse {
            kind: PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC,
            data: topic_token,
        });
    }

    fn enqueue_pulse(&mut self, p: PendingPulse) {
        if self.pending.len() >= MAX_QUEUED_PULSES {
            // Drop oldest — the newer pulse is more relevant.
            let _ = self.pending.pop_front();
        }
        self.pending.push_back(p);
    }

    /// Tick. Returns a datagram to be sent, if any.
    /// Order:
    /// 1. Pending manual pulses are sent first (one per tick — with a
    ///    full queue, draining takes `n * tick_period`).
    /// 2. If `now >= next_tick` and `tick_period > 0`, sends an AUTOMATIC
    ///    heartbeat and schedules the next tick.
    ///
    /// `None` means: nothing to send in this tick.
    ///
    /// # Errors
    /// `WireError` if encoding fails (e.g. SN overflow).
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
        // AUTOMATIC beat
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
        let payload = msg.to_cdr(true)?; // LE by default
        let sn = SequenceNumber(self.next_sn);
        self.next_sn = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "wlp sequence overflow",
            })?;
        // Under liveliness_protection != NONE use the secure-WLP entity, so that
        // a spec-conformant peer (cyclone with is_liveliness_protected) assigns the
        // heartbeat to the protected WLP reader (§8.4.2.4 / Tab.7).
        let (writer_id, reader_id) = if self.secure {
            (
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER,
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER,
            )
        } else {
            (
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER,
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER,
            )
        };
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id,
            writer_id,
            writer_sn: sn,
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let header = RtpsHeader::new(self.vendor_id, self.own_prefix);
        let dg = encode_data_datagram(header, &[data])?;
        // Reliable-resend history: remember the plain beat under its seq.
        self.sent.insert(sn.0, dg.clone());
        while self.sent.len() > 32 {
            if let Some(&oldest) = self.sent.keys().next() {
                self.sent.remove(&oldest);
            }
        }
        Ok(dg)
    }

    /// Reliable resend: if `bytes` contains an ACKNACK to the (secure-)WLP
    /// writer, it returns the plain beat datagrams from the history that the
    /// reader still needs (>= ACKNACK base). The runtime re-protects+sends them
    /// again (protect_wlp_outbound). Without this cyclone's secure-WLP
    /// reader hangs on base 1 and our writer counts as not-alive.
    #[must_use]
    pub fn wlp_acknack_resends(&self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let Ok(parsed) = decode_datagram(bytes) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for sub in parsed.submessages {
            if let ParsedSubmessage::AckNack(a) = sub {
                if a.writer_id != EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER
                    && a.writer_id != EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
                {
                    continue;
                }
                let base = a.reader_sn_state.bitmap_base.0;
                for (seq, dg) in &self.sent {
                    if *seq >= base {
                        out.push(dg.clone());
                    }
                }
            }
        }
        out
    }

    /// Processes an incoming datagram. If it contains a WLP DATA
    /// submessage (writer_id == BUILTIN_PARTICIPANT_MESSAGE_WRITER), the
    /// per-peer last-seen timestamp is updated. Other datagrams are
    /// ignored.
    ///
    /// # Errors
    /// `WireError` if the outer datagram is malformed. A single
    /// malformed submessage does not cause an error — we skip it
    /// silently.
    pub fn handle_datagram(&mut self, bytes: &[u8], now: Duration) -> Result<bool, WireError> {
        let parsed = decode_datagram(bytes)?;
        let mut updated = false;
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                // Accept the plain AND secure WLP writer — a peer with
                // liveliness_protection publishes over 0xff0200c3; without this
                // match ZeroDDS would never see its heartbeat → a false
                // LIVELINESS_LOST/lease expire. The outer SEC_* of the secure
                // variant has already been unwrapped by the inbound path.
                if d.writer_id == EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER
                    || d.writer_id == EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
                {
                    if let Ok(msg) = ParticipantMessageData::from_cdr(&d.serialized_payload) {
                        // Source: prefer the prefix from the payload (as
                        // Cyclone does), falling back to the RTPS header
                        // prefix if the payload does not carry it.
                        let src = msg.prefix();
                        let prefix = if src == GuidPrefix::UNKNOWN {
                            parsed.header.guid_prefix
                        } else {
                            src
                        };
                        // DoS cap: limit the number of peers.
                        if self.peers.len() >= MAX_TRACKED_PEERS
                            && !self.peers.contains_key(&prefix)
                        {
                            // Drop: the known-peer list is full.
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

    /// Returns the current liveliness state of a known peer.
    /// `None` if unknown.
    #[must_use]
    pub fn peer_state(&self, prefix: &GuidPrefix) -> Option<&PeerLivelinessState> {
        self.peers.get(prefix)
    }

    /// Number of tracked peers.
    #[must_use]
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Iterates over all peers whose last-seen is older than
    /// `now - lease`. Callers use this to drive liveliness-lost events.
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

    /// Removes a peer from tracking (e.g. on SPDP lease expiry).
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
        // No second beat within the period.
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
        // Tick period 1h so the AUTOMATIC path does not interfere.
        let mut e = WlpEndpoint::new(
            GuidPrefix::from_bytes([1; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        // First an initial AUTOMATIC beat to initialize.
        let _ = e.tick(Duration::ZERO).unwrap();
        e.assert_participant();
        let dg = e.tick(Duration::from_millis(1)).unwrap().expect("manual");
        // Decode the datagram and check the kind.
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
    fn wlp_secure_sender_uses_secure_entity_and_is_received() {
        // Regression H-6: with set_secure(true) the endpoint sends over the
        // secure-WLP entity (0xff0200c3); the receiver accepts both
        // entities. Without the dual-entity demux a peer with
        // liveliness_protection would never see the heartbeat → a false LIVELINESS_LOST.
        let mut sender = ep();
        sender.set_secure(true);
        let dg = sender.tick(Duration::ZERO).unwrap().unwrap();
        // Wire: writer_id must be the secure entity.
        let parsed = decode_datagram(&dg).unwrap();
        let found_secure = parsed.submessages.iter().any(|s| {
            matches!(
                s,
                ParsedSubmessage::Data(d)
                    if d.writer_id == EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
            )
        });
        assert!(found_secure, "secure sender must use 0xff0200c3");
        // The receiver (plain config) accepts the secure WLP anyway.
        let mut receiver = WlpEndpoint::new(
            GuidPrefix::from_bytes([99; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        let updated = receiver
            .handle_datagram(&dg, Duration::from_millis(50))
            .unwrap();
        assert!(
            updated,
            "secure WLP must be accepted by the dual-entity demux"
        );
        assert_eq!(receiver.peer_count(), 1);
    }

    #[test]
    fn wlp_handle_datagram_ignores_non_wlp_traffic() {
        // An SPDP DATA submessage must NOT be counted as WLP.
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
        // now = 200 ms → 100 ms elapsed → still alive.
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
        // With the old 300 ms, 100 ms would not trigger a beat;
        // with 50 ms a beat must arrive at 100 ms.
        let dg = e.tick(Duration::from_millis(100)).unwrap();
        assert!(dg.is_some());
    }

    #[test]
    fn wlp_handle_datagram_uses_header_prefix_when_payload_unknown() {
        // If the payload GuidPrefix == UNKNOWN (zero bytes), the
        // endpoint must use the RTPS header prefix as a fallback.
        // We build a WLP datagram manually with a zero-prefix payload.
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
        // The fallback prefix from the RTPS header must be used as the
        // peer key.
        assert!(receiver.peer_state(&header_prefix).is_some());
    }

    #[test]
    fn wlp_handle_datagram_skips_malformed_cdr() {
        // WLP submessage with a 3-byte payload (too small for an
        // encapsulation header). handle_datagram must not panic, but
        // skip it silently.
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
