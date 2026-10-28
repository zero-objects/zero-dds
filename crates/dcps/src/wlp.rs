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
//! The writer is a [`ReliableWriter`] (DDSI-RTPS 2.5 §8.4.13.1: "The protocol
//! used to communicate between built-in Endpoints is the same as used for
//! application-defined Endpoints"; §8.4.13.3: RELIABLE, TRANSIENT_LOCAL,
//! KEEP_LAST with depth 1). Its history holds the last sample of each
//! instance, the key being the participant prefix and the `kind`
//! (§8.4.13.5); it has one reader proxy for the
//! BuiltinParticipantMessageReader of each participant that announces one
//! ([`WlpEndpoint::add_peer_reader`]), sends HEARTBEATs to a reader until it
//! has acknowledged, and repairs what a reader's ACKNACK asks for
//! ([`WlpEndpoint::handle_reader_messages`]).

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::datagram::{
    ParsedDatagram, ParsedSubmessage, ReceiveContext, SubmessageFrame, decode_datagram,
    encode_datagram,
};
use zerodds_rtps::error::WireError;
use zerodds_rtps::header::RtpsHeader;
use zerodds_rtps::history_cache::HistoryKind;
use zerodds_rtps::message_builder::{DEFAULT_MTU, OutboundDatagram};
use zerodds_rtps::participant_message_data::{
    PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
    PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
    PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC, ParticipantMessageData,
};
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::reliable_writer::{DEFAULT_FRAGMENT_SIZE, ReliableWriter, ReliableWriterConfig};
use zerodds_rtps::submessages::{AckNackSubmessage, SequenceNumberSet};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, SequenceNumber, VendorId};

/// DoS cap for the manual-pulse queue. If a caller invokes
/// `assert_liveliness()` faster than the tick loop processes it, we drop
/// old pulses — a reader only remembers the last beat.
pub const MAX_QUEUED_PULSES: usize = 32;

/// DoS cap for the number of known remote peer participants.
/// Scales with domain size; more than 1024 is a single-domain
/// anti-pattern and a sign of misuse.
pub const MAX_TRACKED_PEERS: usize = 1024;

/// How often the WLP writer announces its history to a reader that has not
/// acknowledged it (DDSI-RTPS 2.5 §8.4.7.1 `heartbeatPeriod`).
pub const WLP_HEARTBEAT_PERIOD: Duration = Duration::from_millis(200);

/// What the writer knows of the WLP reader of one remote participant.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PeerReader {
    /// `builtinEndpointSet` the participant announced.
    endpoint_set: u32,
    /// Where it receives metatraffic.
    locators: Vec<Locator>,
    /// `builtinEndpointQos` it announced, if it did.
    builtin_endpoint_qos: Option<u32>,
}

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

/// What the reader side knows of one remote WLP writer, by the prefix of
/// the participant it belongs to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RemoteWlpWriter {
    /// Highest sequence number received from it (0: none yet).
    received: i64,
    /// `count` of the last ACKNACK sent to it, incremented with each one
    /// so the writer can tell a new ACKNACK from a repeated one.
    acknack_count: i32,
}

/// Most sequence numbers one ACKNACK asks for: the bitmap bound
/// `zerodds_rtps::submessages::RTPS_BITMAP_MAX_BITS`, which the decoders of
/// Cyclone and Fast DDS enforce too.
const MAX_ACKNACK_BITS: i64 = zerodds_rtps::submessages::RTPS_BITMAP_MAX_BITS as i64;

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
    /// The plain WLP writer (`BuiltinParticipantMessageWriter`): RELIABLE,
    /// TRANSIENT_LOCAL, KEEP_LAST 1 per instance.
    writer: ReliableWriter,
    /// The secure one (`BuiltinParticipantMessageSecureWriter`): another RTPS
    /// writer, with sequence numbers and a history of its own (DDS-Security
    /// 1.2 §7.5.2).
    secure_writer: ReliableWriter,
    /// The WLP readers of the remote participants, by participant prefix.
    peer_readers: BTreeMap<GuidPrefix, PeerReader>,
    /// How many times [`Self::refresh_participant`] was called.
    participant_refreshes: u64,
    /// A MANUAL_BY_PARTICIPANT sample was written since
    /// [`Self::take_manual_participant_written`].
    manual_participant_written: bool,
    /// WLP submessages addressed to another participant (INFO_DST), see
    /// [`Self::misaddressed`].
    misaddressed: u64,
    /// Tick period for AUTOMATIC heartbeats (typically
    /// `lease_duration / 3`).
    tick_period: Duration,
    /// Time of the next due AUTOMATIC beat.
    next_tick: Duration,
    /// Manual pulses that have not yet been sent.
    pending: VecDeque<PendingPulse>,
    /// Per-peer tracking (GuidPrefix → state).
    peers: BTreeMap<GuidPrefix, PeerLivelinessState>,
    /// Reader side of the reliable protocol, per remote WLP writer; see
    /// [`Self::heartbeat_responses`].
    remote_writers: BTreeMap<GuidPrefix, RemoteWlpWriter>,
    /// The plain writer is in use: some writer of the participant has a
    /// liveliness that is not protected.
    use_plain: bool,
    /// The secure writer is in use (`BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER`,
    /// 0xff0200c2, DDS-Security §10.4.1.2.5 / §7.4.7.1 Table 9): some writer's
    /// topic has `is_liveliness_protected`, or the domain-wide
    /// `liveliness_protection_kind` asks for it. Both may be in use (§7.5.2).
    use_secure: bool,
}

/// The WLP writer of `prefix`: RELIABLE, TRANSIENT_LOCAL, KEEP_LAST with depth
/// 1 per instance (DDSI-RTPS 2.5 §8.4.13.3), the plain or the secure entity.
fn make_writer(prefix: GuidPrefix, vendor_id: VendorId, secure: bool) -> ReliableWriter {
    let entity = if secure {
        EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
    } else {
        EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER
    };
    let mut writer = ReliableWriter::new(ReliableWriterConfig {
        guid: Guid::new(prefix, entity),
        vendor_id,
        reader_proxies: Vec::new(),
        max_samples: WLP_MAX_SAMPLES,
        history_kind: HistoryKind::KeepLast {
            depth: WLP_MAX_SAMPLES,
        },
        heartbeat_period: WLP_HEARTBEAT_PERIOD,
        fragment_size: DEFAULT_FRAGMENT_SIZE,
        mtu: DEFAULT_MTU,
    });
    // One sample per instance; the instances are the kinds of §8.4.13.5.
    writer.set_cache_history(
        HistoryKind::KeepLast {
            depth: WLP_MAX_SAMPLES,
        },
        WLP_MAX_SAMPLES,
        Some(1),
    );
    // The builtin EntityIds are fixed: a datagram names its participant, as
    // the SEDP writers do.
    writer.set_emit_info_dst(true);
    writer
}

/// Samples the WLP history can hold: the instances that exist are the
/// AUTOMATIC kind, MANUAL_BY_PARTICIPANT and the vendor kind.
const WLP_MAX_SAMPLES: usize = 8;

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
            writer: make_writer(own_prefix, vendor_id, false),
            secure_writer: make_writer(own_prefix, vendor_id, true),
            peer_readers: BTreeMap::new(),
            participant_refreshes: 0,
            manual_participant_written: false,
            misaddressed: 0,
            tick_period,
            next_tick: Duration::ZERO,
            pending: VecDeque::new(),
            peers: BTreeMap::new(),
            remote_writers: BTreeMap::new(),
            use_plain: true,
            use_secure: false,
        }
    }

    /// Switches the endpoint to the secure-WLP entity (send via
    /// `BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER`). Set by the runtime
    /// when `liveliness_protection != NONE`. The receive path matches
    /// both entities anyway, independent of this flag.
    pub fn set_secure(&mut self, secure: bool) {
        self.set_channels(!secure, secure);
    }

    /// Chooses the WLP writers in use: the plain one, the secure one, or both
    /// (DDS-Security 1.2 §7.5.2: the liveliness of a writer goes over the
    /// plain or the secure ParticipantMessage writer by
    /// `is_liveliness_protected` of its topic). With neither asked for, the
    /// plain one is used. Each writer keeps its own sequence numbers and
    /// history; the reader proxies follow the choice.
    pub fn set_channels(&mut self, plain: bool, secure: bool) {
        let (plain, secure) = if plain || secure {
            (plain, secure)
        } else {
            (true, false)
        };
        if self.use_plain == plain && self.use_secure == secure {
            return;
        }
        self.use_plain = plain;
        self.use_secure = secure;
        let peers: Vec<GuidPrefix> = self.peer_readers.keys().copied().collect();
        for prefix in peers {
            self.sync_proxy(prefix);
        }
    }

    /// The remote participant `prefix` announced `endpoint_set` as its
    /// `builtinEndpointSet` and receives metatraffic at `locators`: its
    /// BuiltinParticipantMessageReader (or the secure one, with the secure
    /// endpoint) becomes a reader proxy of the WLP writer, which sends it the
    /// history and HEARTBEATs until it has acknowledged (DDSI-RTPS 2.5
    /// §8.4.13.1, §8.4.13.3). A participant that announces no such reader gets
    /// none. Called again for the same participant with other locators, it replaces them; with the same, it does nothing.
    pub fn add_peer_reader(
        &mut self,
        prefix: GuidPrefix,
        endpoint_set: u32,
        locators: Vec<Locator>,
        builtin_endpoint_qos: Option<u32>,
    ) {
        if prefix == self.own_prefix {
            return;
        }
        let peer = PeerReader {
            endpoint_set,
            locators,
            builtin_endpoint_qos,
        };
        // An unchanged record leaves the proxy, and what it has
        // acknowledged, as it is.
        if self.peer_readers.get(&prefix) == Some(&peer) {
            return;
        }
        self.peer_readers.insert(prefix, peer);
        self.sync_proxy(prefix);
    }

    /// Adds, replaces or removes the reader proxies of `prefix` after its
    /// record or the choice of the writers changed.
    fn sync_proxy(&mut self, prefix: GuidPrefix) {
        self.sync_proxy_on(prefix, false);
        self.sync_proxy_on(prefix, true);
    }

    /// [`Self::sync_proxy`] for the plain (`secure` false) or the secure
    /// writer: it has a proxy of the reader of its kind when it is in use and
    /// the participant announces that reader.
    fn sync_proxy_on(&mut self, prefix: GuidPrefix, secure: bool) {
        use zerodds_rtps::participant_data::endpoint_flag;
        let (reader_entity, bit, in_use) = if secure {
            (
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER,
                endpoint_flag::PARTICIPANT_MESSAGE_SECURE_READER,
                self.use_secure,
            )
        } else {
            (
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER,
                endpoint_flag::PARTICIPANT_MESSAGE_DATA_READER,
                self.use_plain,
            )
        };
        let reader = Guid::new(prefix, reader_entity);
        let writer = if secure {
            &mut self.secure_writer
        } else {
            &mut self.writer
        };
        match self.peer_readers.get(&prefix) {
            Some(peer) if in_use && peer.endpoint_set & bit != 0 => {
                // §8.4.13.3: "If the ParticipantProxy::builtinEndpointQos is
                // included … the BuiltinParticipantMessageWriter shall treat
                // the BuiltinParticipantMessageReader as indicated by the
                // flags. If … not included … as if it is configured with
                // RELIABLE_RELIABILITY_QOS."
                let reliable = peer.builtin_endpoint_qos.is_none_or(|qos| {
                    qos & zerodds_rtps::participant_data::builtin_endpoint_qos::
                        BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER
                        == 0
                });
                let proxy = ReaderProxy::new(reader, peer.locators.clone(), Vec::new(), reliable);
                writer.add_reader_proxy(proxy);
                // TRANSIENT_LOCAL (§8.4.13.3): the reader gets the history the
                // writer holds. It is sent to the new proxy as if the reader
                // had asked for it, the first sample held being the base: the
                // reader would ask for the same after the first HEARTBEAT,
                // and the history keeps one sample per kind, so a round trip
                // may meet a newer sample in its place. The proxy is then in
                // step, and the next sample goes to it directly.
                if reliable {
                    let cache = writer.cache();
                    if let (Some(min), Some(max)) = (cache.min_sn(), cache.max_sn()) {
                        let held: Vec<SequenceNumber> = (min.0..=max.0)
                            .map(SequenceNumber)
                            .filter(|sn| cache.get(*sn).is_some())
                            .collect();
                        writer.handle_acknack(reader, min, held);
                    }
                }
            }
            _ => {
                let _ = writer.remove_reader_proxy(reader);
            }
        }
    }

    /// How many WLP submessages (DATA and HEARTBEAT of a remote WLP writer,
    /// ACKNACK and NACK_FRAG for ours) named another participant in their
    /// INFO_DESTINATION and were ignored (DDSI-RTPS 2.5 §8.3.8.8: the
    /// submessages that follow are for that participant). Two participants
    /// that held one unicast port one after the other receive each other's.
    #[must_use]
    pub fn misaddressed(&self) -> u64 {
        self.misaddressed
    }

    /// How many times a write, `assert_liveliness`, dispose or unregister of
    /// a MANUAL_BY_PARTICIPANT writer asked for the participant's liveliness
    /// to be refreshed ([`Self::refresh_participant`]) since the start.
    #[must_use]
    pub fn participant_refreshes(&self) -> u64 {
        self.participant_refreshes
    }

    /// Whether a MANUAL_BY_PARTICIPANT sample was written since the last call;
    /// the runtime then increments the `manualLivelinessCount` of its SPDP
    /// announcement (DDSI-RTPS 2.5 Table 8.78).
    pub fn take_manual_participant_written(&mut self) -> bool {
        core::mem::take(&mut self.manual_participant_written)
    }

    /// How many remote participants the writer knows ([`Self::add_peer_reader`]),
    /// those without a message reader included.
    #[must_use]
    pub fn peer_reader_count(&self) -> usize {
        self.peer_readers.len()
    }

    /// The sequence number of the latest sample the WLP writer wrote.
    #[must_use]
    pub fn last_written_sequence_number(&self) -> SequenceNumber {
        if self.use_plain {
            self.writer.last_written_sn()
        } else {
            self.secure_writer.last_written_sn()
        }
    }

    /// The current AUTOMATIC beat period.
    #[must_use]
    pub fn tick_period(&self) -> Duration {
        self.tick_period
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

    /// A write, `assert_liveliness`, dispose or unregister of a
    /// MANUAL_BY_PARTICIPANT writer (DDSI-RTPS 2.5 §8.7.2.2.3): a
    /// MANUAL_BY_PARTICIPANT sample goes out on the next tick. One that is
    /// already waiting says the same, so it is not queued again; a writer
    /// that writes at the rate of the tick sends one sample per tick, not one
    /// per write.
    pub fn refresh_participant(&mut self) {
        self.participant_refreshes = self.participant_refreshes.saturating_add(1);
        let waiting = self.pending.iter().any(|p| {
            p.kind == PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE
        });
        if !waiting {
            self.assert_participant();
        }
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

    /// Tick. Returns the datagrams to be sent, each with the locators of the
    /// reader proxy it is for. Order:
    /// 1. The writer's own tick: HEARTBEATs to readers that have not
    ///    acknowledged, and the repairs they asked for.
    /// 2. A pending manual pulse is written (one per tick — with a full queue,
    ///    draining takes `n * tick_period`).
    /// 3. Otherwise, if `now >= next_tick` and `tick_period > 0`, an
    ///    AUTOMATIC beat is written and the next tick scheduled.
    ///
    /// The repairs come before the new sample: the history keeps one sample per
    /// kind, so a sample written first replaces the one a reader asked for
    /// since the last tick, and its repair is a GAP. With a runtime tick as
    /// long as the beat period every request met a newer sample, and the
    /// reader never got one.
    ///
    /// An empty list means: nothing to send in this tick.
    ///
    /// # Errors
    /// `WireError` if encoding fails (e.g. SN overflow).
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = self.writer.tick(now)?;
        out.extend(self.secure_writer.tick(now)?);
        if let Some(pulse) = self.pending.pop_front() {
            out.extend(self.write_pulse(&pulse)?);
        } else if !self.tick_period.is_zero() && now >= self.next_tick {
            let pulse = PendingPulse {
                kind: PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
                data: Vec::new(),
            };
            out.extend(self.write_pulse(&pulse)?);
            self.next_tick = now + self.tick_period;
        }
        Ok(out)
    }

    /// Writes `pulse` into the history of the WLP writer. The instance is
    /// `(participantGuidPrefix, kind)` (DDSI-RTPS 2.5 §8.4.13.5), so a sample
    /// replaces the earlier one of its kind and the history holds one per
    /// kind.
    fn write_pulse(&mut self, pulse: &PendingPulse) -> Result<Vec<OutboundDatagram>, WireError> {
        if pulse.kind == PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE {
            self.manual_participant_written = true;
        }
        let mut msg = ParticipantMessageData::automatic(self.own_prefix);
        msg.kind = pulse.kind;
        msg.data = pulse.data.clone();
        let payload = msg.to_cdr(true)?; // LE by default
        let mut key = [0u8; 16];
        key[..12].copy_from_slice(&self.own_prefix.to_bytes());
        key[12..].copy_from_slice(&pulse.kind.to_be_bytes());
        let mut out = Vec::new();
        if self.use_plain {
            out.extend(self.writer.write_keyed_stamped(&payload, None, Some(key))?);
        }
        if self.use_secure {
            out.extend(
                self.secure_writer
                    .write_keyed_stamped(&payload, None, Some(key))?,
            );
        }
        Ok(out)
    }

    /// The ACKNACK and NACK_FRAG submessages of `bytes` that go to the WLP
    /// writer (plain or secure), handed to it: the reader's acknowledgement
    /// and the repair it asks for (DDSI-RTPS 2.5 §8.4.7). The repairs and the
    /// HEARTBEAT an ACKNACK with the final flag clear asks for go out with the
    /// next [`Self::tick`].
    pub fn handle_reader_messages(&mut self, bytes: &[u8]) {
        self.handle_reader_messages_in(bytes, &ReceiveContext::default());
    }

    /// [`Self::handle_reader_messages`] for a datagram that arrived in
    /// `context` (DDSI-RTPS 2.5 §8.3.4).
    pub fn handle_reader_messages_in(&mut self, bytes: &[u8], context: &ReceiveContext) {
        let Ok(parsed) = decode_datagram(bytes) else {
            return;
        };
        let plain = self.writer.guid().entity_id;
        let secure = self.secure_writer.guid().entity_id;
        for (state, sub) in parsed.receiver_walk_in(self.own_prefix, context) {
            let target = match sub {
                ParsedSubmessage::AckNack(a) => Some(a.writer_id),
                ParsedSubmessage::NackFrag(n) => Some(n.writer_id),
                _ => None,
            };
            let ours = target == Some(plain) || target == Some(secure);
            let writer = if target == Some(secure) {
                &mut self.secure_writer
            } else {
                &mut self.writer
            };
            if ours && state.dest_guid_prefix != self.own_prefix {
                self.misaddressed = self.misaddressed.saturating_add(1);
                continue;
            }
            match sub {
                ParsedSubmessage::AckNack(a) if ours => {
                    let src = Guid::new(state.source_guid_prefix, a.reader_id);
                    let requested: Vec<SequenceNumber> = a.reader_sn_state.iter_set().collect();
                    writer.handle_acknack(src, a.reader_sn_state.bitmap_base, requested);
                    writer.note_acknack_final_flag(src, a.final_flag);
                }
                ParsedSubmessage::NackFrag(n) if ours => {
                    let src = Guid::new(state.source_guid_prefix, n.reader_id);
                    writer.handle_nackfrag(src, n);
                }
                _ => {}
            }
        }
    }

    /// Processes an incoming datagram. For each WLP DATA submessage
    /// (writer_id == BUILTIN_PARTICIPANT_MESSAGE_WRITER) it carries, the
    /// per-peer last-seen timestamp is updated, and the participant and the
    /// `kind` of the message are returned — the runtime hands them to its
    /// readers as liveliness assertions (DDS 1.4 §2.2.3.11). Other
    /// datagrams yield nothing.
    ///
    /// # Errors
    /// `WireError` if the outer datagram is malformed. A single
    /// malformed submessage does not cause an error — we skip it
    /// silently.
    pub fn handle_datagram(
        &mut self,
        bytes: &[u8],
        now: Duration,
    ) -> Result<Vec<(GuidPrefix, u32)>, WireError> {
        self.handle_datagram_in(bytes, &ReceiveContext::default(), now)
    }

    /// [`Self::handle_datagram`] for a datagram that arrived in `context`
    /// (DDSI-RTPS 2.5 §8.3.4).
    ///
    /// # Errors
    /// As [`Self::handle_datagram`].
    pub fn handle_datagram_in(
        &mut self,
        bytes: &[u8],
        context: &ReceiveContext,
        now: Duration,
    ) -> Result<Vec<(GuidPrefix, u32)>, WireError> {
        let parsed = decode_datagram(bytes)?;
        let mut received = Vec::new();
        for (state, sub) in parsed.receiver_walk_in(self.own_prefix, context) {
            if let ParsedSubmessage::Data(d) = sub {
                // Accept the plain AND secure WLP writer — a peer with
                // liveliness_protection publishes over 0xff0200c2; without this
                // match ZeroDDS would never see its heartbeat → a false
                // LIVELINESS_LOST/lease expire. The outer SEC_* of the secure
                // variant has already been unwrapped by the inbound path.
                if d.writer_id == EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER
                    || d.writer_id == EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
                {
                    if state.dest_guid_prefix != self.own_prefix {
                        self.misaddressed = self.misaddressed.saturating_add(1);
                        continue;
                    }
                    self.note_received(state.source_guid_prefix, d.writer_sn.0);
                    if let Ok(msg) = ParticipantMessageData::from_cdr(&d.serialized_payload) {
                        // Source: prefer the prefix from the payload (as
                        // Cyclone does), falling back to the receiver
                        // state's source prefix (DDSI-RTPS 2.5 §8.3.4,
                        // updated by InfoSource) if the payload does not
                        // carry it.
                        let src = msg.prefix();
                        let prefix = if src == GuidPrefix::UNKNOWN {
                            state.source_guid_prefix
                        } else {
                            src
                        };
                        // Our own pulse, looped back by the multicast
                        // group: this participant is not its own peer.
                        if prefix == self.own_prefix {
                            continue;
                        }
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
                        received.push((prefix, msg.kind));
                    }
                }
            }
        }
        Ok(received)
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
        self.remote_writers.remove(prefix);
        if self.peer_readers.remove(prefix).is_some() {
            let _ = self.writer.remove_reader_proxy(Guid::new(
                *prefix,
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER,
            ));
            let _ = self.secure_writer.remove_reader_proxy(Guid::new(
                *prefix,
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER,
            ));
        }
    }

    /// Records a DATA of the WLP writer of participant `source`.
    fn note_received(&mut self, source: GuidPrefix, sn: i64) {
        if source == self.own_prefix
            || (self.remote_writers.len() >= MAX_TRACKED_PEERS
                && !self.remote_writers.contains_key(&source))
        {
            return;
        }
        let writer = self.remote_writers.entry(source).or_default();
        writer.received = writer.received.max(sn);
    }

    /// The ACKNACKs that answer the HEARTBEATs of remote WLP writers in
    /// `bytes`, each with the prefix of the participant it goes to.
    ///
    /// DDSI-RTPS 2.5 §8.4.13 has the BuiltinParticipantMessageReader
    /// RELIABLE. A writer's first pulse can predate its discovery of this
    /// participant: Cyclone sends one when the participant is created, to
    /// no matched reader, and afterwards announces it in every HEARTBEAT
    /// (measured 2026-09-30 against `ddsperf sub`: `first=1, last=1`,
    /// 11 to 13 HEARTBEATs in five seconds over five runs, no second DATA). A reader that
    /// never asks for it never learns that participant is alive.
    ///
    /// The ACKNACK asks for every sequence number from the one after the
    /// highest received (or the writer's first, if later) up to its last,
    /// at most `MAX_ACKNACK_BITS`. A HEARTBEAT that announces nothing
    /// missing is answered only when its Final flag is clear, which asks
    /// the reader for an answer; the answer is then the positive
    /// acknowledgement. The reader entity is the plain or the secure WLP
    /// reader, after the writer the HEARTBEAT came from.
    pub fn heartbeat_responses(&mut self, bytes: &[u8]) -> Vec<(GuidPrefix, Vec<u8>)> {
        self.heartbeat_responses_in(bytes, &ReceiveContext::default())
    }

    /// [`Self::heartbeat_responses`] for a datagram that arrived in
    /// `context` (DDSI-RTPS 2.5 §8.3.4).
    pub fn heartbeat_responses_in(
        &mut self,
        bytes: &[u8],
        context: &ReceiveContext,
    ) -> Vec<(GuidPrefix, Vec<u8>)> {
        let Ok(parsed) = decode_datagram(bytes) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (state, sub) in parsed.receiver_walk_in(self.own_prefix, context) {
            let ParsedSubmessage::Heartbeat(hb) = sub else {
                continue;
            };
            let reader_id = if hb.writer_id == EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER {
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER
            } else if hb.writer_id == EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER {
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER
            } else {
                continue;
            };
            let source = state.source_guid_prefix;
            if state.dest_guid_prefix != self.own_prefix {
                self.misaddressed = self.misaddressed.saturating_add(1);
                continue;
            }
            if source == self.own_prefix
                || (self.remote_writers.len() >= MAX_TRACKED_PEERS
                    && !self.remote_writers.contains_key(&source))
            {
                continue;
            }
            let writer = self.remote_writers.entry(source).or_default();
            let base = hb.first_sn.0.max(writer.received.saturating_add(1)).max(1);
            let last = hb.last_sn.0.min(base.saturating_add(MAX_ACKNACK_BITS - 1));
            let missing: Vec<SequenceNumber> = (base..=last).map(SequenceNumber).collect();
            if missing.is_empty() && hb.final_flag {
                continue;
            }
            writer.acknack_count = writer.acknack_count.wrapping_add(1);
            let acknack = AckNackSubmessage {
                reader_id,
                writer_id: hb.writer_id,
                reader_sn_state: SequenceNumberSet::from_missing(SequenceNumber(base), &missing),
                count: writer.acknack_count,
                final_flag: missing.is_empty(),
            };
            let frame = |submessage| SubmessageFrame {
                little_endian: true,
                octets_elided: false,
                submessage,
            };
            let datagram = ParsedDatagram {
                header: RtpsHeader::new(self.vendor_id, self.own_prefix),
                submessages: alloc::vec![
                    frame(ParsedSubmessage::InfoDestination(source.to_bytes())),
                    frame(ParsedSubmessage::AckNack(acknack)),
                ],
                invalid_rest: None,
            };
            if let Ok(bytes) = encode_datagram(&datagram) {
                out.push((source, bytes));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use alloc::vec;
    use zerodds_rtps::datagram::encode_data_datagram;
    use zerodds_rtps::submessages::DataSubmessage;

    /// The participant the senders of these tests announce to: a receiver
    /// of prefix `[99; 12]` has a WLP reader.
    fn reader_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([99; 12])
    }

    /// `e` with a reader proxy for the WLP reader of [`reader_prefix`].
    fn with_reader(mut e: WlpEndpoint) -> WlpEndpoint {
        use zerodds_rtps::participant_data::endpoint_flag;
        // The tests drive the clock by milliseconds; the default
        // `nackResponseDelay` is the spec's 200 ms.
        e.writer.set_nack_response_delay(Duration::ZERO);
        e.add_peer_reader(
            reader_prefix(),
            endpoint_flag::PARTICIPANT_MESSAGE_DATA_READER
                | endpoint_flag::PARTICIPANT_MESSAGE_SECURE_READER,
            vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            None,
        );
        e
    }

    /// The first datagram of `out` that carries a DATA.
    fn data_datagram(out: Vec<OutboundDatagram>) -> Option<Vec<u8>> {
        out.into_iter().map(|d| d.bytes).find(|bytes| {
            decode_datagram(bytes).is_ok_and(|p| {
                p.submessages
                    .iter()
                    .any(|f| matches!(f.submessage, ParsedSubmessage::Data(_)))
            })
        })
    }

    fn ep() -> WlpEndpoint {
        with_reader(WlpEndpoint::new(
            GuidPrefix::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
            VendorId::ZERODDS,
            Duration::from_millis(300),
        ))
    }

    #[test]
    fn wlp_first_tick_emits_automatic_heartbeat() {
        let mut e = ep();
        let dg = e.tick(Duration::ZERO).unwrap();
        assert!(
            data_datagram(dg).is_some(),
            "first tick must emit AUTOMATIC beat"
        );
    }

    #[test]
    fn wlp_tick_idle_returns_none_until_period() {
        let mut e = ep();
        let _ = e.tick(Duration::ZERO).unwrap();
        // No second beat within the period.
        let dg = e.tick(Duration::from_millis(100)).unwrap();
        assert!(data_datagram(dg).is_none());
    }

    #[test]
    fn wlp_tick_emits_again_after_period() {
        let mut e = ep();
        let _ = e.tick(Duration::ZERO).unwrap();
        let dg = e.tick(Duration::from_millis(400)).unwrap();
        assert!(data_datagram(dg).is_some());
    }

    #[test]
    fn wlp_zero_period_disables_automatic_beats() {
        let mut e = WlpEndpoint::new(
            GuidPrefix::from_bytes([0xAA; 12]),
            VendorId::ZERODDS,
            Duration::ZERO,
        );
        let dg = e.tick(Duration::ZERO).unwrap();
        assert!(data_datagram(dg).is_none());
    }

    #[test]
    fn wlp_assert_participant_emits_manual_pulse() {
        // Tick period 1h so the AUTOMATIC path does not interfere.
        let mut e = with_reader(WlpEndpoint::new(
            GuidPrefix::from_bytes([1; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        ));
        // First an initial AUTOMATIC beat to initialize.
        let _ = e.tick(Duration::ZERO).unwrap();
        e.assert_participant();
        let dg = data_datagram(e.tick(Duration::from_millis(1)).unwrap()).expect("manual");
        // Decode the datagram and check the kind.
        let parsed = decode_datagram(&dg).unwrap();
        let data_sub = parsed.submessages.iter().find_map(|f| match &f.submessage {
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
        let mut e = with_reader(WlpEndpoint::new(
            GuidPrefix::from_bytes([2; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        ));
        let _ = e.tick(Duration::ZERO).unwrap();
        e.assert_topic(vec![0xAA, 0xBB]);
        let dg = data_datagram(e.tick(Duration::from_millis(1)).unwrap()).expect("manual");
        let parsed = decode_datagram(&dg).unwrap();
        let data_sub = parsed
            .submessages
            .iter()
            .find_map(|f| match &f.submessage {
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

    /// A participant's own pulse, looped back by the multicast group, is
    /// not a peer's: before, every runtime counted itself.
    #[test]
    fn wlp_handle_datagram_skips_the_own_participant() {
        let mut e = ep();
        let dg = data_datagram(e.tick(Duration::ZERO).unwrap()).unwrap();
        let updated = e.handle_datagram(&dg, Duration::from_millis(50)).unwrap();
        assert!(updated.is_empty(), "{updated:?}");
        assert_eq!(e.peer_count(), 0);
    }

    #[test]
    fn wlp_handle_datagram_updates_peer_state() {
        let mut sender = ep();
        let mut receiver = WlpEndpoint::new(
            GuidPrefix::from_bytes([99; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        let dg = data_datagram(sender.tick(Duration::ZERO).unwrap()).unwrap();
        let updated = receiver
            .handle_datagram(&dg, Duration::from_millis(50))
            .unwrap();
        assert!(!updated.is_empty());
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
        // secure-WLP entity (0xff0200c2); the receiver accepts both
        // entities. Without the dual-entity demux a peer with
        // liveliness_protection would never see the heartbeat → a false LIVELINESS_LOST.
        let mut sender = ep();
        sender.set_secure(true);
        let dg = data_datagram(sender.tick(Duration::ZERO).unwrap()).unwrap();
        // Wire: writer_id must be the secure entity.
        let parsed = decode_datagram(&dg).unwrap();
        let found_secure = parsed.submessages.iter().any(|f| {
            matches!(
                &f.submessage,
                ParsedSubmessage::Data(d)
                    if d.writer_id == EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
            )
        });
        assert!(found_secure, "secure sender must use 0xff0200c2");
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
            !updated.is_empty(),
            "secure WLP must be accepted by the dual-entity demux"
        );
        assert_eq!(receiver.peer_count(), 1);
    }

    /// DDS-Security 1.2 §7.5.2: the plain and the secure ParticipantMessage
    /// writer are two RTPS writers, each with its own sequence numbers and
    /// history; a participant with writers of both kinds beats on both, and a
    /// reader proxy of each kind belongs to the writer of its kind.
    #[test]
    fn the_plain_and_the_secure_writer_beat_with_sequence_numbers_of_their_own() {
        let mut e = ep();
        e.set_channels(true, true);
        let writers_and_sns = |out: Vec<OutboundDatagram>| {
            let mut found = Vec::new();
            for dg in out {
                for f in decode_datagram(&dg.bytes).unwrap().submessages {
                    if let ParsedSubmessage::Data(d) = f.submessage {
                        found.push((d.writer_id, d.reader_id, d.writer_sn.0));
                    }
                }
            }
            found.sort_by_key(|(w, _, _)| w.to_bytes());
            found
        };
        let first = writers_and_sns(e.tick(Duration::ZERO).unwrap());
        assert_eq!(
            first,
            vec![
                (
                    EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER,
                    EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER,
                    1
                ),
                (
                    EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER,
                    EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER,
                    1
                ),
            ]
        );
        // The secure writer alone continues its own numbers; the plain one
        // keeps its history and has no proxy any more.
        e.set_channels(false, true);
        let second = writers_and_sns(e.tick(Duration::from_millis(400)).unwrap());
        assert!(
            second
                .iter()
                .all(|d| d.0 == EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER),
            "the plain writer is out of use"
        );
        assert!(
            second.iter().any(|d| d.2 == 2),
            "the next number of the secure writer"
        );
        assert_eq!(e.last_written_sequence_number().0, 2);
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
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: vec![0u8; 8].into(),
        };
        let dg = encode_data_datagram(header, &[data]).unwrap();
        let mut e = ep();
        let updated = e.handle_datagram(&dg, Duration::from_millis(10)).unwrap();
        assert!(updated.is_empty());
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
        let dg = data_datagram(sender.tick(Duration::ZERO).unwrap()).unwrap();
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
        let dg = data_datagram(sender.tick(Duration::ZERO).unwrap()).unwrap();
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
        assert!(data_datagram(dg).is_some());
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
            data_flag: true,
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
        assert!(!updated.is_empty());
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
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: vec![0u8; 3].into(),
        };
        let dg = encode_data_datagram(header, &[data]).unwrap();
        let mut e = ep();
        let updated = e.handle_datagram(&dg, Duration::from_millis(5)).unwrap();
        assert!(updated.is_empty());
        assert_eq!(e.peer_count(), 0);
    }

    #[test]
    fn wlp_pulses_drained_one_per_tick() {
        let mut e = with_reader(WlpEndpoint::new(
            GuidPrefix::from_bytes([3; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        ));
        let _ = e.tick(Duration::ZERO).unwrap();
        e.assert_participant();
        e.assert_participant();
        let dg1 = e.tick(Duration::from_millis(1)).unwrap();
        let dg2 = e.tick(Duration::from_millis(2)).unwrap();
        let dg3 = e.tick(Duration::from_millis(3)).unwrap();
        assert!(data_datagram(dg1).is_some());
        assert!(data_datagram(dg2).is_some());
        assert!(data_datagram(dg3).is_none(), "queue empty after 2 pulses");
    }

    /// DDSI-RTPS 2.5 §8.3.8.8: the submessages after an INFO_DESTINATION are
    /// for the participant it names. A participant of another prefix — one that
    /// holds the unicast port a restarted peer held before — ignores them, and
    /// the ignore is counted: a receiver that gets nothing and counts nothing
    /// fails here.
    #[test]
    fn a_datagram_addressed_to_another_participant_is_ignored_and_counted() {
        let mut sender = ep();
        let out = sender.tick(Duration::ZERO).unwrap();
        let dg = data_datagram(out.clone()).unwrap();
        // The HEARTBEAT of the same tick, a datagram of its own.
        let hb_dg = out
            .iter()
            .map(|d| d.bytes.clone())
            .find(|b| {
                decode_datagram(b).is_ok_and(|p| {
                    p.submessages
                        .iter()
                        .any(|f| matches!(f.submessage, ParsedSubmessage::Heartbeat(_)))
                })
            })
            .expect("a HEARTBEAT");
        // The control: the participant it is addressed to takes it.
        let mut addressed = WlpEndpoint::new(
            reader_prefix(),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        let got = addressed.handle_datagram(&dg, Duration::ZERO).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(addressed.misaddressed(), 0);
        // Another prefix on the same port.
        let mut other = WlpEndpoint::new(
            GuidPrefix::from_bytes([7; 12]),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        );
        let got = other.handle_datagram(&dg, Duration::ZERO).unwrap();
        assert!(got.is_empty(), "{got:?}");
        assert_eq!(other.peer_count(), 0);
        assert_eq!(other.misaddressed(), 1, "the ignore is counted");
        // Its HEARTBEATs get no ACKNACK, also counted.
        assert_eq!(addressed.heartbeat_responses(&hb_dg).len(), 1);
        assert!(other.heartbeat_responses(&hb_dg).is_empty());
        assert_eq!(other.misaddressed(), 2);
        // The same for an ACKNACK to a writer of this prefix.
        sender.handle_reader_messages(&acknack_to(GuidPrefix::from_bytes([55; 12]), 2, &[], true));
        assert_eq!(sender.misaddressed(), 1);
    }

    // -- writer side: RELIABLE, TRANSIENT_LOCAL, KEEP_LAST 1 ---------------

    /// The sequence numbers of the DATA in `out`.
    fn data_sns(out: &[OutboundDatagram]) -> Vec<i64> {
        let mut sns = Vec::new();
        for dg in out {
            for f in decode_datagram(&dg.bytes).unwrap().submessages {
                if let ParsedSubmessage::Data(d) = f.submessage {
                    sns.push(d.writer_sn.0);
                }
            }
        }
        sns
    }

    /// The `(firstSN, lastSN)` of the HEARTBEATs in `out`.
    fn heartbeats(out: &[OutboundDatagram]) -> Vec<(i64, i64)> {
        let mut hbs = Vec::new();
        for dg in out {
            for f in decode_datagram(&dg.bytes).unwrap().submessages {
                if let ParsedSubmessage::Heartbeat(h) = f.submessage {
                    hbs.push((h.first_sn.0, h.last_sn.0));
                }
            }
        }
        hbs
    }

    /// An ACKNACK of the reader of [`reader_prefix`] to the WLP writer of
    /// `writer`: it has everything below `base` and misses `missing`.
    fn acknack_to(writer: GuidPrefix, base: i64, missing: &[i64], final_flag: bool) -> Vec<u8> {
        let missing: Vec<SequenceNumber> = missing.iter().copied().map(SequenceNumber).collect();
        let ack = AckNackSubmessage {
            reader_id: EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER,
            writer_id: EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER,
            reader_sn_state: SequenceNumberSet::from_missing(SequenceNumber(base), &missing),
            count: 1,
            final_flag,
        };
        let frame = |submessage| SubmessageFrame {
            little_endian: true,
            octets_elided: false,
            submessage,
        };
        encode_datagram(&ParsedDatagram {
            header: RtpsHeader::new(VendorId::ZERODDS, reader_prefix()),
            submessages: vec![
                frame(ParsedSubmessage::InfoDestination(writer.to_bytes())),
                frame(ParsedSubmessage::AckNack(ack)),
            ],
            invalid_rest: None,
        })
        .unwrap()
    }

    fn own() -> GuidPrefix {
        GuidPrefix::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12])
    }

    /// DDSI-RTPS 2.5 §8.4.13.3: "history.kind = KEEP_LAST_HISTORY_QOS,
    /// history.depth = 1"; §8.4.13.5: the instance is the participant prefix
    /// and the `kind`. The history holds the last sample of each kind: five
    /// AUTOMATIC and two MANUAL_BY_PARTICIPANT samples leave two.
    #[test]
    fn the_history_holds_the_last_sample_of_each_kind() {
        let mut e = ep();
        for i in 0..5 {
            e.pending.push_back(PendingPulse {
                kind: PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
                data: Vec::new(),
            });
            let _ = e.tick(Duration::from_millis(i)).unwrap();
        }
        for i in 5..7 {
            e.assert_participant();
            let _ = e.tick(Duration::from_millis(i)).unwrap();
        }
        assert_eq!(e.writer.cache().len(), 2);
        let sns: Vec<i64> = (1..=e.last_written_sequence_number().0)
            .filter(|sn| e.writer.cache().get(SequenceNumber(*sn)).is_some())
            .collect();
        assert_eq!(sns, vec![5, 7]);
    }

    /// §8.4.13.3: the writer is RELIABLE and TRANSIENT_LOCAL. A reader that
    /// comes later gets the two held samples with the first tick, and a
    /// HEARTBEAT that says what the history holds; asking again it gets the
    /// two held samples once more — and nothing of the superseded one.
    #[test]
    fn a_late_reader_is_told_by_heartbeat_and_gets_the_held_samples_on_request() {
        let mut e = WlpEndpoint::new(own(), VendorId::ZERODDS, Duration::from_secs(3600));
        e.assert_participant();
        e.assert_participant();
        for t in 0..4 {
            let out = e.tick(Duration::from_millis(t)).unwrap();
            assert!(out.is_empty(), "no reader yet: {out:?}");
        }
        // Samples 1 and 2 (MANUAL_BY_PARTICIPANT) and 3 (AUTOMATIC): the
        // history holds 2 and 3.
        let mut e = with_reader(e);
        let out = e.tick(Duration::from_millis(10)).unwrap();
        assert_eq!(heartbeats(&out), vec![(2, 3)]);
        let mut sns = data_sns(&out);
        sns.sort_unstable();
        assert_eq!(sns, vec![2, 3], "the held samples go to the new proxy");
        // A reader that lost them asks again, as it would after the HEARTBEAT.
        e.handle_reader_messages(&acknack_to(own(), 1, &[1, 2, 3], false));
        let out = e.tick(Duration::from_millis(20)).unwrap();
        let mut sns = data_sns(&out);
        sns.sort_unstable();
        assert_eq!(sns, vec![2, 3], "1 is superseded: a GAP, no DATA");
    }

    /// §8.4.2.2.3: the writer sends HEARTBEATs "until the Reader has either
    /// acknowledged receiving all available samples or has disappeared".
    #[test]
    fn the_writer_heartbeats_until_the_reader_has_acknowledged() {
        let mut e = with_reader(WlpEndpoint::new(
            own(),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        ));
        let _ = e.tick(Duration::ZERO).unwrap();
        let out = e.tick(Duration::from_millis(300)).unwrap();
        assert_eq!(heartbeats(&out), vec![(1, 1)], "unacknowledged: announced");
        e.handle_reader_messages(&acknack_to(own(), 2, &[], true));
        for t in [600, 900, 1200] {
            let out = e.tick(Duration::from_millis(t)).unwrap();
            assert!(heartbeats(&out).is_empty(), "acknowledged: {out:?}");
        }
    }

    /// §8.4.13.2: the reader proxy is the BuiltinParticipantMessageReader;
    /// a participant that announces none gets nothing, and one that is
    /// forgotten gets no more.
    #[test]
    fn only_a_participant_with_a_message_reader_gets_a_proxy() {
        use zerodds_rtps::participant_data::endpoint_flag;
        let mut e = WlpEndpoint::new(own(), VendorId::ZERODDS, Duration::from_millis(300));
        let other = GuidPrefix::from_bytes([77; 12]);
        let at = vec![Locator::udp_v4([127, 0, 0, 1], 7499)];
        e.add_peer_reader(
            other,
            endpoint_flag::PARTICIPANT_MESSAGE_DATA_WRITER,
            at.clone(),
            None,
        );
        assert!(e.tick(Duration::ZERO).unwrap().is_empty());
        e.add_peer_reader(
            other,
            endpoint_flag::PARTICIPANT_MESSAGE_DATA_READER,
            at,
            None,
        );
        let out = e.tick(Duration::from_millis(1)).unwrap();
        assert!(
            out.iter().all(|d| d.targets.iter().all(|l| l.port == 7499)),
            "{out:?}"
        );
        assert!(!out.is_empty());
        e.forget_peer(&other);
        assert!(e.tick(Duration::from_secs(10)).unwrap().is_empty());
    }

    /// §8.7.2.2.3: a write, `assert_liveliness`, dispose or unregister of a
    /// MANUAL_BY_PARTICIPANT writer sends one MANUAL_BY_PARTICIPANT sample;
    /// calls before the tick share it.
    #[test]
    fn refreshes_before_the_tick_share_one_manual_by_participant_sample() {
        let mut e = with_reader(WlpEndpoint::new(
            own(),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        ));
        let _ = e.tick(Duration::ZERO).unwrap();
        for _ in 0..10 {
            e.refresh_participant();
        }
        let out = e.tick(Duration::from_millis(1)).unwrap();
        assert_eq!(data_sns(&out), vec![2]);
        let kind = |out: &[OutboundDatagram]| {
            let dg = data_datagram(out.to_vec()).unwrap();
            let parsed = decode_datagram(&dg).unwrap();
            parsed
                .submessages
                .iter()
                .find_map(|f| match &f.submessage {
                    ParsedSubmessage::Data(d) => Some(
                        ParticipantMessageData::from_cdr(&d.serialized_payload)
                            .unwrap()
                            .kind,
                    ),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(
            kind(&out),
            PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE
        );
        assert!(data_sns(&e.tick(Duration::from_millis(2)).unwrap()).is_empty());
    }

    /// DDSI-RTPS 2.5 §8.4.13.3: with `builtinEndpointQos` announced, the
    /// writer "shall treat the BuiltinParticipantMessageReader as indicated by
    /// the flags": a reader with BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER
    /// is a best-effort proxy; without the field, or without the flag, it is
    /// RELIABLE.
    #[test]
    fn the_reader_is_treated_as_builtin_endpoint_qos_says() {
        use zerodds_rtps::participant_data::{builtin_endpoint_qos, endpoint_flag};
        let reliable_of = |qos: Option<u32>| {
            let mut e = WlpEndpoint::new(own(), VendorId::ZERODDS, Duration::from_secs(3600));
            e.add_peer_reader(
                reader_prefix(),
                endpoint_flag::PARTICIPANT_MESSAGE_DATA_READER,
                vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
                qos,
            );
            let out = e.tick(Duration::ZERO).unwrap();
            (
                e.writer.reader_proxies()[0].is_reliable,
                heartbeats(&out).len(),
            )
        };
        assert!(reliable_of(None).0);
        assert!(reliable_of(Some(0)).0);
        let (reliable, hbs) = reliable_of(Some(
            builtin_endpoint_qos::BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER,
        ));
        assert!(!reliable);
        assert_eq!(hbs, 0, "a best-effort reader is sent no HEARTBEAT");
    }

    /// A MANUAL_BY_PARTICIPANT sample written is reported once, for the
    /// `manualLivelinessCount` of the SPDP announcement.
    #[test]
    fn a_written_manual_sample_is_reported_once() {
        let mut e = with_reader(WlpEndpoint::new(
            own(),
            VendorId::ZERODDS,
            Duration::from_secs(3600),
        ));
        let _ = e.tick(Duration::ZERO).unwrap();
        assert!(!e.take_manual_participant_written(), "AUTOMATIC only");
        e.assert_participant();
        let _ = e.tick(Duration::from_millis(1)).unwrap();
        assert!(e.take_manual_participant_written());
        assert!(!e.take_manual_participant_written());
    }

    /// Two endpoints whose tick is as long as the beat period: a writer that
    /// wrote before its reader was known (the first sample goes with the
    /// proxy), and one that writes on every tick. The reader sees the writer
    /// on the first tick after the proxy exists, and again on each later one —
    /// a request that meets a newer sample would be answered with a GAP
    /// and the reader would never hear from the writer.
    #[test]
    fn a_reader_hears_a_writer_whose_tick_is_as_long_as_its_beat_period() {
        let a_prefix = own();
        let b_prefix = reader_prefix();
        let mut a = WlpEndpoint::new(a_prefix, VendorId::ZERODDS, Duration::from_millis(80));
        let mut b = WlpEndpoint::new(b_prefix, VendorId::ZERODDS, Duration::from_millis(80));
        // A wrote twice before it knew B.
        let _ = a.tick(Duration::ZERO).unwrap();
        let _ = a.tick(Duration::from_millis(100)).unwrap();
        a = with_reader(a);
        let mut heard = Vec::new();
        for step in 2..12u64 {
            let now = Duration::from_millis(step * 100);
            for dg in a.tick(now).unwrap() {
                let got = b.handle_datagram(&dg.bytes, now).unwrap();
                heard.extend(got.into_iter().map(|(p, _)| p));
                for (_, ack) in b.heartbeat_responses(&dg.bytes) {
                    a.handle_reader_messages(&ack);
                }
            }
        }
        assert!(
            heard.iter().filter(|p| **p == a_prefix).count() >= 8,
            "B heard A {} times in ten ticks",
            heard.len()
        );
    }

    // -- reader side: HEARTBEATs of remote WLP writers --------------------

    /// A remote participant's WLP writer announcing `first..=last`.
    fn heartbeat(
        from: GuidPrefix,
        writer: EntityId,
        first: i64,
        last: i64,
        final_flag: bool,
    ) -> Vec<u8> {
        use zerodds_rtps::submessages::HeartbeatSubmessage;
        let hb = HeartbeatSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: writer,
            first_sn: SequenceNumber(first),
            last_sn: SequenceNumber(last),
            count: 1,
            final_flag,
            liveliness_flag: false,
            group_info: None,
        };
        encode_datagram(&ParsedDatagram {
            header: RtpsHeader::new(VendorId::ZERODDS, from),
            submessages: vec![SubmessageFrame {
                little_endian: true,
                octets_elided: false,
                submessage: ParsedSubmessage::Heartbeat(hb),
            }],
            invalid_rest: None,
        })
        .unwrap()
    }

    /// The one ACKNACK in a response, with the prefix INFO_DST names.
    fn acknack_of(bytes: &[u8]) -> (GuidPrefix, AckNackSubmessage) {
        let parsed = decode_datagram(bytes).unwrap();
        let mut dst = None;
        let mut ack = None;
        for frame in parsed.submessages {
            match frame.submessage {
                ParsedSubmessage::InfoDestination(p) => dst = Some(GuidPrefix::from_bytes(p)),
                ParsedSubmessage::AckNack(a) => ack = Some(a),
                other => panic!("unexpected {other:?}"),
            }
        }
        (dst.unwrap(), ack.unwrap())
    }

    fn remote() -> GuidPrefix {
        GuidPrefix::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12])
    }

    fn reader() -> WlpEndpoint {
        WlpEndpoint::new(
            GuidPrefix::from_bytes([99; 12]),
            VendorId::ZERODDS,
            Duration::ZERO,
        )
    }

    #[test]
    fn a_heartbeat_announcing_a_pulse_not_received_is_answered_with_a_request_for_it() {
        // Cyclone's shape: one pulse, sent before this participant was
        // known, announced as first=1, last=1 in a Final HEARTBEAT.
        let mut r = reader();
        let hb = heartbeat(
            remote(),
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER,
            1,
            1,
            true,
        );
        let out = r.heartbeat_responses(&hb);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, remote());
        let (dst, ack) = acknack_of(&out[0].1);
        assert_eq!(dst, remote());
        assert_eq!(ack.reader_id, EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER);
        assert_eq!(ack.writer_id, EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER);
        assert_eq!(ack.reader_sn_state.bitmap_base, SequenceNumber(1));
        assert_eq!(ack.reader_sn_state.num_bits, 1);
        assert_eq!(ack.reader_sn_state.bitmap, vec![1u32 << 31]);
        assert!(!ack.final_flag);
    }

    #[test]
    fn once_the_pulse_arrived_a_final_heartbeat_gets_no_answer_and_a_plain_one_the_ack() {
        let mut sender = ep();
        let mut r = reader();
        let data = data_datagram(sender.tick(Duration::ZERO).unwrap()).unwrap();
        r.handle_datagram(&data, Duration::ZERO).unwrap();
        let w = EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER;
        assert!(
            r.heartbeat_responses(&heartbeat(remote(), w, 1, 1, true))
                .is_empty()
        );
        let out = r.heartbeat_responses(&heartbeat(remote(), w, 1, 1, false));
        assert_eq!(out.len(), 1);
        let (_, ack) = acknack_of(&out[0].1);
        assert_eq!(ack.reader_sn_state.bitmap_base, SequenceNumber(2));
        assert_eq!(ack.reader_sn_state.num_bits, 0);
        assert!(ack.final_flag);
    }

    #[test]
    fn the_request_starts_after_the_highest_pulse_received_and_counts_up() {
        let mut sender = ep();
        let mut r = reader();
        let first = data_datagram(sender.tick(Duration::ZERO).unwrap()).unwrap();
        r.handle_datagram(&first, Duration::ZERO).unwrap();
        let w = EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER;
        let (_, a) = acknack_of(&r.heartbeat_responses(&heartbeat(remote(), w, 1, 4, true))[0].1);
        assert_eq!(a.reader_sn_state.bitmap_base, SequenceNumber(2));
        assert_eq!(a.reader_sn_state.num_bits, 3);
        let (_, b) = acknack_of(&r.heartbeat_responses(&heartbeat(remote(), w, 3, 4, true))[0].1);
        assert_eq!(b.reader_sn_state.bitmap_base, SequenceNumber(3));
        assert_eq!(b.count, a.count + 1);
    }

    #[test]
    fn a_request_asks_for_at_most_the_bitmap_bound() {
        let mut r = reader();
        let w = EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER;
        let (_, a) =
            acknack_of(&r.heartbeat_responses(&heartbeat(remote(), w, 1, 10_000, true))[0].1);
        assert_eq!(i64::from(a.reader_sn_state.num_bits), MAX_ACKNACK_BITS);
    }

    #[test]
    fn the_secure_writer_is_answered_by_the_secure_reader() {
        let mut r = reader();
        let w = EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER;
        let (_, a) = acknack_of(&r.heartbeat_responses(&heartbeat(remote(), w, 1, 1, true))[0].1);
        assert_eq!(
            a.reader_id,
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER
        );
        assert_eq!(a.writer_id, w);
    }

    #[test]
    fn own_heartbeats_and_other_writers_are_not_answered() {
        let mut r = reader();
        let own = heartbeat(
            GuidPrefix::from_bytes([99; 12]),
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER,
            1,
            1,
            false,
        );
        assert!(r.heartbeat_responses(&own).is_empty());
        let sedp = heartbeat(
            remote(),
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
            1,
            1,
            false,
        );
        assert!(r.heartbeat_responses(&sedp).is_empty());
    }
}
