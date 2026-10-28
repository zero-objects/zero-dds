// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reliable RTPS reader (1:N writer proxies) — DDSI-RTPS 2.5 §8.4.10.
//!
//! Corresponds to the [`StatefulReader`] role with 1..N matched writers.
//! Fragmentation (§8.4.14) is supported. Multi-writer since WP 1.4
//! T4.5: a separate [`WriterProxyState`] per remote writer with its own
//! `received_cache`, `delivered_up_to` and `FragmentAssembler`.
//!
//! # Why per-proxy state?
//!
//! SequenceNumbers are writer-local (Spec §8.3.5.4). Two writers with
//! overlapping SN spaces would collide in a global cache
//! — hence separate buffers per proxy.
//!
//! # API-Form
//!
//! ```text
//!   let mut r = ReliableReader::new(...);
//!   r.add_writer_proxy(proxy_for_remote_A);
//!   loop {
//!       match transport.recv_submessage() {
//!           Data(d)      => for s in r.handle_data(&d) { deliver(s) },
//!           DataFrag(df) => for s in r.handle_data_frag(&df, uptime()) { deliver(s) },
//!           Heartbeat(h) => r.handle_heartbeat(&h, uptime()),
//!           Gap(g)       => for s in r.handle_gap(&g) { deliver(s) },
//!       }
//!       for dg in r.tick(uptime())? { transport.send(dg) }
//!   }
//! ```
//!
//! [`StatefulReader`]: https://www.omg.org/spec/DDSI-RTPS/2.5/

use core::time::Duration;

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::fragment_assembler::{AssemblerCaps, FragmentAssembler};
use crate::header::RtpsHeader;
use crate::history_cache::{CacheChange, ChangeKind, HistoryCache, HistoryKind};
use crate::message_builder::OutboundDatagram;
use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
use crate::submessages::{
    AckNackSubmessage, DataFragSubmessage, DataSubmessage, GapSubmessage, HeartbeatSubmessage,
    NackFragSubmessage, SequenceNumberSet,
};
use crate::wire_types::{Guid, GuidPrefix, SequenceNumber, VendorId};
use crate::writer_proxy::{ChangeFromWriterStatus, NotAvailableKind, WriterProxy};

/// §8.4.10.1 "Default Timing-Related Values":
/// `heartbeatResponseDelay.nanosec = 500 * 1000 * 1000`, 500 ms, "used as
/// the defaults in order to facilitate 'out-of-the-box' interoperability
/// between implementations". `ReliableReaderConfig::heartbeat_response_delay`
/// and `reader.heartbeat_response_delay_ms` set another (0 answers a
/// HEARTBEAT on the next tick).
pub const DEFAULT_HEARTBEAT_RESPONSE_DELAY: Duration = Duration::from_millis(500);

/// The spec's `heartbeatResponseDelay`; the same value as
/// [`DEFAULT_HEARTBEAT_RESPONSE_DELAY`].
pub const SPEC_HEARTBEAT_RESPONSE_DELAY: Duration = DEFAULT_HEARTBEAT_RESPONSE_DELAY;

/// §8.4.10.1.1 `heartbeatSuppressionDuration.sec = 0;
/// heartbeatSuppressionDuration.nanosec = 0` — the spec default, which
/// ZeroDDS also uses: no HEARTBEAT is suppressed unless the caller sets
/// [`ReliableReader::set_heartbeat_suppression_duration`].
pub const DEFAULT_HEARTBEAT_SUPPRESSION_DURATION: Duration = Duration::ZERO;

/// Per-writer state: the proxy + separate receive state.
///
/// Every remote writer has its own SN space (§8.3.5.4), so also
/// its own `received_cache`, `delivered_up_to` and
/// `FragmentAssembler`. This way two writers with colliding SNs
/// (e.g. both starting at 1) can be received in parallel without issue.
#[derive(Debug, Clone)]
pub struct WriterProxyState {
    /// Writer-proxy protocol state.
    pub proxy: WriterProxy,
    /// Receive cache for this writer.
    pub received_cache: HistoryCache,
    /// Highest SN delivered to the app.
    pub delivered_up_to: SequenceNumber,
    /// Fragment reassembly for this writer.
    pub assembler: FragmentAssembler,
    /// Time since which an ACKNACK/NACK_FRAG to this writer is
    /// pending. `None` = nothing pending.
    pub pending_acknack_since: Option<Duration>,
    /// Arrival time of the last HEARTBEAT this proxy acted on. Feeds
    /// `R::heartbeatSuppressionDuration` (§8.4.10.1): a HEARTBEAT that
    /// arrives sooner than that after the previous one is ignored.
    pub last_heartbeat_at: Option<Duration>,
    /// Whether a sample of this writer was handed out. A change missing in
    /// front of the first one is not counted lost: a reader that joins a
    /// writer late was never sent what came before.
    pub delivered_any: bool,
    /// Changes of this writer that can no longer arrive, since the last
    /// [`ReliableReader::take_lost`]: missing behind a gap a BEST_EFFORT
    /// reader stepped over, or dropped by the writer before a RELIABLE
    /// reader had them (its HEARTBEAT's `firstSN` passed them).
    pub lost_changes: u64,
    /// The writer's open coherent set, followed across the changes handed
    /// out in order (DDSI-RTPS 2.5 §8.7.6).
    coherent: Option<CoherentTrack>,
    /// The sequence numbers of this writer's changes handed out, from the
    /// writer itself or forwarded on its behalf (DDSI-RTPS 2.5 §8.7.9): a
    /// change that arrives a second time, from the writer or from a service
    /// that forwards it, is a duplicate.
    committed: SnRanges,
    /// Whether this writer's `PID_ORIGINAL_WRITER_INFO` is honoured
    /// ([`ReliableReader::set_forwarder_trusted`]).
    forwarder_trusted: bool,
    /// `WriterProxy::dataMaxSizeSerialized` (DDSI-RTPS 2.5 Table 8.72): the
    /// writer's announced bound on the size of a SerializedPayload, in
    /// octets. `None`: none announced.
    data_max_size: Option<u32>,
}

/// Sequence numbers as sorted, disjoint, non-adjacent ranges. A writer's
/// stream handed out in order is one range; what a GAP or a loss leaves out
/// splits it. At most [`SnRanges::MAX_RANGES`] ranges are kept, the lowest
/// is forgotten past it: a change below it that is forwarded once more is
/// handed out again rather than the memory growing without bound.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SnRanges {
    ranges: Vec<(i64, i64)>,
}

impl SnRanges {
    const MAX_RANGES: usize = 256;

    /// Adds `sn`; `false` when it was there.
    fn insert(&mut self, sn: i64) -> bool {
        if let Some(last) = self.ranges.last_mut() {
            if sn == last.1 + 1 {
                last.1 = sn;
                return true;
            }
        }
        let i = self.ranges.partition_point(|&(_, high)| high < sn);
        if self.ranges.get(i).is_some_and(|&(low, _)| low <= sn) {
            return false;
        }
        let joins_left = i > 0 && self.ranges[i - 1].1 + 1 == sn;
        let joins_right = self.ranges.get(i).is_some_and(|&(low, _)| low == sn + 1);
        match (joins_left, joins_right) {
            (true, true) => {
                self.ranges[i - 1].1 = self.ranges[i].1;
                self.ranges.remove(i);
            }
            (true, false) => self.ranges[i - 1].1 = sn,
            (false, true) => self.ranges[i].0 = sn,
            (false, false) => {
                self.ranges.insert(i, (sn, sn));
                if self.ranges.len() > Self::MAX_RANGES {
                    self.ranges.remove(0);
                }
            }
        }
        true
    }
}

/// A writer's open coherent set as the reader follows it: the set's first
/// sequence number (its `PID_COHERENT_SET`) and how many of the writer's
/// changes from there on the reader has, a member received or a change a
/// GAP ruled irrelevant. A change the reader stepped over as lost, or one
/// written before the reader joined, is not counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CoherentTrack {
    first: i64,
    counted: i64,
}

impl CoherentTrack {
    /// The set ends in front of `end`: complete when every change from its
    /// first up to `end` is counted.
    fn complete_before(self, end: i64) -> bool {
        self.counted == end - self.first
    }
}

impl WriterProxyState {
    fn new(proxy: WriterProxy, max_samples: usize, caps: AssemblerCaps) -> Self {
        Self {
            proxy,
            received_cache: HistoryCache::new(max_samples),
            delivered_up_to: SequenceNumber(0),
            assembler: FragmentAssembler::new(caps),
            pending_acknack_since: None,
            last_heartbeat_at: None,
            delivered_any: false,
            lost_changes: 0,
            coherent: None,
            committed: SnRanges::default(),
            forwarder_trusted: true,
            data_max_size: None,
        }
    }

    /// Follows the writer's coherent set across the change `sn` handed out
    /// as `set` (its `PID_COHERENT_SET`), `end` for an end-of-coherent-set
    /// DATA (DDSI-RTPS 2.5 §8.7.6). Returns whether the set this change
    /// ends was complete, `None` when it ends none. "The end of a Writer's
    /// coherent set is defined by the arrival of one of the following: A
    /// Data Submessage from this Writer that belongs to a new coherent set.
    /// A Data Submessage from this Writer that does not contain a coherent
    /// set in-line QoS parameter or alternatively, contains a coherent set
    /// in-line QoS parameter with value SEQUENCENUMBER_UNKNOWN." A GROUP-
    /// scope publisher's change is left to the subscriber's group order,
    /// which checks its set (§8.7.6).
    fn follow_coherent_set(
        &mut self,
        sn: i64,
        set: Option<i64>,
        end: bool,
        grouped: bool,
    ) -> Option<bool> {
        if grouped {
            return None;
        }
        if end {
            return self.coherent.take().map(|t| t.complete_before(sn));
        }
        match (self.coherent.as_mut(), set) {
            (Some(t), Some(first)) if t.first == first => {
                t.counted += 1;
                None
            }
            (_, set) => {
                let ended = self.coherent.take().map(|t| t.complete_before(sn));
                self.coherent = set.map(|first| CoherentTrack { first, counted: 1 });
                ended
            }
        }
    }

    /// A change of the writer passed without a sample: received without
    /// one to hand out, or ruled irrelevant by a GAP (§8.4.10.4.3). It
    /// counts for an open set, as one the reader does not need.
    fn follow_coherent_set_past(&mut self) {
        if let Some(t) = self.coherent.as_mut() {
            t.counted += 1;
        }
    }

    /// Counts `lost` changes, unless no sample was handed out yet.
    fn note_lost(&mut self, lost: u64) {
        if self.delivered_any {
            self.lost_changes = self.lost_changes.saturating_add(lost);
        }
    }
}

/// How many writers without a proxy a [`ReliableReader`] remembers the
/// announced SNs of; the oldest is forgotten past it.
pub const UNMATCHED_WRITERS_REMEMBERED: usize = 32;

/// A reliable reader with 0..N writer proxies.
#[derive(Debug, Clone)]
pub struct ReliableReader {
    guid: Guid,
    vendor_id: VendorId,
    writer_proxies: Vec<WriterProxyState>,
    heartbeat_response_delay: Duration,
    /// §8.4.10.1 `heartbeatSuppressionDuration`. Defaults to
    /// [`DEFAULT_HEARTBEAT_SUPPRESSION_DURATION`]; set via
    /// [`ReliableReader::set_heartbeat_suppression_duration`].
    heartbeat_suppression_duration: Duration,
    acknack_count: i32,
    nackfrag_count: i32,
    duplicate_frag_count: u64,
    /// Template for new proxies.
    max_samples_per_proxy: usize,
    assembler_caps: AssemblerCaps,
    /// Counter for submessages whose `writer_id` has no proxy.
    unknown_src_count: u64,
    /// The highest SN a writer this reader has no proxy for yet announced,
    /// by DATA or HEARTBEAT, oldest first and at most
    /// [`UNMATCHED_WRITERS_REMEMBERED`]. A remote writer can match this
    /// reader and write before this side has matched it; that DATA is
    /// dropped (§8.4.10), and without this record the reader learns of it
    /// only from the writer's next HEARTBEAT — measured three seconds with
    /// Fast DDS 3.6.1 (a ROS 2 service's first request). With it, the
    /// ACKNACK that [`Self::add_writer_proxy`] arms requests the SNs at
    /// once.
    unmatched_highest: Vec<(Guid, SequenceNumber)>,
    /// Counter for samples that arrived intact but found the per-proxy
    /// receive cache full. See [`Self::cache_full_count`].
    cache_full_count: u64,
    /// `true` ⇒ this reader's RELIABILITY QoS is BEST_EFFORT. A best-effort
    /// reader (RTPS §8.4.12.1) never blocks delivery on a missing earlier
    /// sequence number — it has no in-order-without-loss guarantee and (unlike a
    /// reliable reader) does not NACK to repair the gap. Defaults to `false`
    /// (reliable, in-order); the DCPS layer flips it via [`Self::set_best_effort`]
    /// from the reader QoS. Without this, a leading gap (e.g. SN 1 lost in
    /// transit, or a late-join against a writer whose history still starts at 1)
    /// deadlocks the best-effort reader forever.
    best_effort: bool,
    /// How many more samples this reader may hand to its consumer, or
    /// `None` for no bound. See [`Self::set_release_budget`].
    release_budget: Option<usize>,
    /// Writers that left with samples still held for the consumer: their
    /// receive state, which [`Self::release_held`] drains. See
    /// [`Self::depart_writer_proxy`].
    departed: Vec<WriterProxyState>,
    /// Departed writers whose held samples are all handed out, since the
    /// last [`Self::take_departed_done`].
    departed_done: Vec<Guid>,
    /// Changes a departed writer left missing in front of a held sample,
    /// since the last [`Self::take_lost`].
    departed_lost: u64,
    /// The changes forwarded on behalf of writers this reader has no proxy
    /// for (DDSI-RTPS 2.5 §8.7.9), oldest first and at most
    /// [`ORIGINAL_WRITERS_REMEMBERED`]; a proxy added later takes its
    /// writer's over.
    forwarded_without_proxy: Vec<(Guid, SnRanges)>,
    /// Writers whose proxy went, oldest first and at most
    /// [`ORIGINAL_WRITERS_REMEMBERED`]. A writer of the same GUID that comes
    /// back is a new message stream starting its sequence numbers anew
    /// (§8.4.10): its proxy takes nothing over, or its first changes would
    /// be dropped as duplicates of the old ones.
    ended_writers: Vec<Guid>,
    /// Changes dropped as handed out already: forwarded once more, or
    /// arriving from the writer after a forwarded copy.
    forwarded_duplicate_count: u64,
    /// Changes with `PID_ORIGINAL_WRITER_INFO` from a writer whose
    /// forwarding is not honoured, handed out as that writer's own.
    untrusted_forward_count: u64,
    /// Changes whose payload the opener refused.
    refused_payload_count: u64,
    /// Payloads beyond the writer's announced `dataMaxSizeSerialized`.
    oversize_payload_count: u64,
    /// DATA with FilteredFlag 1 (§9.6.4.9), taken but not delivered.
    filtered_sample_count: u64,
}

/// How many writers without a proxy a [`ReliableReader`] remembers the
/// forwarded changes of (DDSI-RTPS 2.5 §8.7.9).
pub const ORIGINAL_WRITERS_REMEMBERED: usize = 64;

/// Opens the SerializedPayload of a change before the reader records it as
/// received: the `CryptoContent` a writer made of it under
/// `data_protection_kind` (DDS-Security 1.2 §9.5.3.3.4.4,
/// `decode_serialized_payload`), the whole one a DATA_FRAG sequence
/// reassembles. A payload that cannot be opened is not received: it stays
/// MISSING and the writer sends it again, once the key may be there.
/// [`ReliableReader::handle_data_opening`] and
/// [`ReliableReader::handle_data_frag_opening`] take one; without one the
/// reader hands payloads out as they arrived, as the relay reader of
/// §7.1.1.4 forwards them.
pub trait PayloadOpener {
    /// The payload of a change of `writer`. `original_writer` is the writer
    /// the change was forwarded on behalf of (DDSI-RTPS 2.5 §8.7.9), when
    /// the reader honours `writer` as forwarder: its key protects the
    /// payload. `marked` is the NonStandardPayloadFlag (§9.4.5.3.1).
    fn open(
        &self,
        writer: Guid,
        original_writer: Option<Guid>,
        payload: &crate::payload::Payload,
        marked: bool,
    ) -> PayloadOpening;
}

/// What a [`PayloadOpener`] made of a payload.
#[derive(Debug, Clone)]
pub enum PayloadOpening {
    /// Not protected: hand it out as it is.
    AsIs,
    /// Opened: hand out this.
    Opened(crate::payload::Payload),
    /// Protected and not to be opened: the change is not received.
    Refused,
}

/// The [`PayloadOpener`] of a reader that opens nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct AsIs;

impl PayloadOpener for AsIs {
    fn open(
        &self,
        _writer: Guid,
        _original_writer: Option<Guid>,
        _payload: &crate::payload::Payload,
        _marked: bool,
    ) -> PayloadOpening {
        PayloadOpening::AsIs
    }
}

/// Configuration at creation.
#[derive(Debug, Clone)]
pub struct ReliableReaderConfig {
    /// GUID of the reader endpoint.
    pub guid: Guid,
    /// VendorId for the RTPS header of the ACKNACKs.
    pub vendor_id: VendorId,
    /// Initial writer proxies. More via `add_writer_proxy`.
    pub writer_proxies: Vec<WriterProxy>,
    /// Capacity of the receive cache per proxy (not global).
    pub max_samples_per_proxy: usize,
    /// Heartbeat response delay (default: 200 ms).
    pub heartbeat_response_delay: Duration,
    /// Caps for the fragment assembler (per proxy).
    pub assembler_caps: AssemblerCaps,
}

/// The change kind PID_STATUS_INFO's bits name (DDSI-RTPS 2.5 §9.6.4.9).
fn status_change_kind(bits: u32) -> ChangeKind {
    let disposed = bits & crate::inline_qos::status_info::DISPOSED != 0;
    let unregistered = bits & crate::inline_qos::status_info::UNREGISTERED != 0;
    match (disposed, unregistered) {
        (true, true) => ChangeKind::NotAliveDisposedUnregistered,
        (true, false) => ChangeKind::NotAliveDisposed,
        (false, true) => ChangeKind::NotAliveUnregistered,
        // §9.6.4.9: D = U = 0 is ALIVE; F says whether the sample passed the
        // content filter of the reader.
        (false, false) if bits & crate::inline_qos::status_info::FILTERED != 0 => {
            ChangeKind::AliveFiltered
        }
        (false, false) => ChangeKind::Alive,
    }
}

/// A sample delivered to the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredSample {
    /// GUID of the writer the sample comes from. Makes multi-writer
    /// deduplication possible in the caller.
    pub writer_guid: Guid,
    /// Sequence number in the writer.
    pub sequence_number: SequenceNumber,
    /// Serialized payload: a range of the received datagram, shared and
    /// not copied (see [`crate::payload::Payload`]).
    pub payload: crate::payload::Payload,
    /// Spec §8.2.1.2 ChangeKind — `Alive` for normal samples,
    /// `NotAliveDisposed` / `NotAliveUnregistered` /
    /// `NotAliveDisposedUnregistered` for lifecycle markers that the
    /// writer sent via `dispose`/`unregister_instance`.
    /// Spec §9.6.3.9 PID_STATUS_INFO in the inline QoS.
    pub kind: ChangeKind,
    /// `PID_KEY_HASH` from the inline QoS (Spec §9.6.4.8). For
    /// lifecycle markers this is the identity of the disposed/
    /// unregistered instance; for keyed-topic ALIVE samples optional
    /// (some vendors send an inline hash, some don't). `None`
    /// if the writer does not supply a hash inline (typical for
    /// keyless topics).
    pub key_hash: Option<[u8; 16]>,
    /// Source timestamp from the preceding INFO_TS submessage (DDSI-RTPS
    /// §8.7.3), if the writer sent one. Feeds `SampleInfo.source_timestamp` and
    /// `DESTINATION_ORDER = BY_SOURCE_TIMESTAMP`. `None` ⇒ the reader uses
    /// reception order.
    pub source_timestamp: Option<crate::header_extension::HeTimestamp>,
    /// `PID_COHERENT_SET` from the inline QoS (DDSI-RTPS 2.5 §8.7.6 /
    /// §9.6.3.9): the `SequenceNumber` of the first sample of the coherent
    /// set this sample belongs to. `None` ⇒ the sample is not part of a set.
    pub coherent_set_sn: Option<i64>,
    /// This delivery is the **end-of-coherent-set (ECS)** marker for the set
    /// named by `coherent_set_sn`, not a sample: the writer sent a DATA with
    /// the D and the K flag clear and an empty payload (§8.7.6). A consumer
    /// that buffers coherent sets releases the set on it; a consumer that
    /// does not must drop it (it carries no user data).
    pub coherent_set_end: bool,
    /// `PID_RELATED_SAMPLE_IDENTITY` (0x0083, DDS-RPC 1.0 §7.8.2) from the
    /// inline QoS: the request a reply answers.
    pub related_sample_identity: Option<crate::inline_qos::SampleIdentityBytes>,
    /// eProsima's vendor-specific form (0x800f,
    /// [`crate::inline_qos::PID_EPROSIMA_RELATED_SAMPLE_IDENTITY`]). Kept
    /// apart: the reader does not know the writer's vendor, the consumer
    /// decides whether to honour it.
    pub eprosima_related_sample_identity: Option<crate::inline_qos::SampleIdentityBytes>,
    /// The group information of a sample from a writer of a GROUP-scope
    /// publisher (DDSI-RTPS 2.5 §8.7.5, §8.7.6), from its in-line QoS.
    pub group: Option<crate::writer_group::ChangeGroupInfo>,
    /// On an end-of-coherent-set delivery: whether the writer's set it
    /// ends was complete, every change from its first up to here received
    /// or ruled irrelevant by a GAP (DDSI-RTPS 2.5 §8.7.6: "Incomplete
    /// coherent sets shall not be added to the history of the RTPS
    /// DataReaders and the corresponding CacheChanges shall be discarded").
    /// A set that ends on a sample instead, a member of a new set or one
    /// outside any, is reported by an end-of-coherent-set delivery in front
    /// of that sample when it was incomplete. `None` where the reader did
    /// not follow the set: a GROUP-scope publisher's, which the
    /// subscriber's group order checks.
    pub coherent_set_complete: Option<bool>,
    /// `PID_ORIGINAL_WRITER_INFO` of the DATA (DDSI-RTPS 2.5 §8.7.9): the
    /// writer that first generated the change and its sequence number
    /// there. The reader hands a change of a writer it honours as forwarder
    /// out as the original's, with `writer_guid` and `sequence_number` the
    /// original's and the forwarder in [`Self::forwarded_by`].
    pub original_writer: Option<(Guid, SequenceNumber)>,
    /// The writer that forwarded this change on behalf of `writer_guid`,
    /// `None` for a change from `writer_guid` itself.
    pub forwarded_by: Option<Guid>,
    /// The payload is as it arrived with the NonStandardPayloadFlag, not
    /// opened: the `CryptoContent` a relay reader forwards (DDS-Security 1.2
    /// §7.1.1.4).
    pub non_standard_payload: bool,
    /// The QoS the DATA carried in-line, which override its writer's
    /// (DDSI-RTPS 2.5 §8.3.8.2).
    pub inline_qos_override: crate::inline_qos::InlineQosOverride,
}

/// `PID_ORIGINAL_WRITER_INFO` from an in-line QoS (DDSI-RTPS 2.5 §8.7.9),
/// in the byte order of its submessage.
fn original_writer_of(
    inline_qos: Option<&crate::parameter_list::ParameterList>,
) -> Option<(Guid, SequenceNumber)> {
    let pl = inline_qos?;
    let (guid, sn) = crate::inline_qos::find_original_writer_info(pl, pl.little_endian)
        .ok()
        .flatten()?;
    Some((Guid::from_bytes(guid), SequenceNumber(sn)))
}

/// DDSI-RTPS 2.5 §8.7.7: whether the in-line QoS names the target readers
/// of a directed write (`PID_DIRECTED_WRITE`, one per reader) and `own` is
/// not one of them.
fn addressed_elsewhere(
    inline_qos: Option<&crate::parameter_list::ParameterList>,
    own: Guid,
) -> bool {
    inline_qos.is_some_and(|pl| {
        !crate::inline_qos::directed_write_accepts(pl, own.to_bytes()).unwrap_or(true)
    })
}

impl DeliveredSample {
    /// The end-of-coherent-set delivery the reader puts in front of `sn`,
    /// the change of `writer` that ended an incomplete set without being
    /// an end-of-coherent-set DATA itself.
    fn incomplete_set_end(writer: Guid, sn: SequenceNumber) -> Self {
        Self {
            writer_guid: writer,
            sequence_number: sn,
            payload: crate::payload::Payload::from(Vec::new()),
            kind: ChangeKind::Alive,
            key_hash: None,
            source_timestamp: None,
            coherent_set_sn: None,
            coherent_set_end: true,
            related_sample_identity: None,
            eprosima_related_sample_identity: None,
            original_writer: None,
            forwarded_by: None,
            non_standard_payload: false,
            inline_qos_override: crate::inline_qos::InlineQosOverride::default(),
            group: None,
            coherent_set_complete: Some(false),
        }
    }
}

impl ReliableReader {
    /// Creates a fresh reader.
    ///
    /// # Panics
    /// If `cfg.assembler_caps.max_pending_sns == 0`.
    #[must_use]
    pub fn new(cfg: ReliableReaderConfig) -> Self {
        assert!(
            cfg.assembler_caps.max_pending_sns > 0,
            "assembler_caps.max_pending_sns must be > 0; use a Best-Effort reader \
             or increase the cap to actually accept fragmented samples"
        );
        let proxies = cfg
            .writer_proxies
            .into_iter()
            .map(|p| WriterProxyState::new(p, cfg.max_samples_per_proxy, cfg.assembler_caps))
            .collect();
        Self {
            guid: cfg.guid,
            vendor_id: cfg.vendor_id,
            writer_proxies: proxies,
            heartbeat_response_delay: cfg.heartbeat_response_delay,
            heartbeat_suppression_duration: DEFAULT_HEARTBEAT_SUPPRESSION_DURATION,
            acknack_count: 0,
            nackfrag_count: 0,
            duplicate_frag_count: 0,
            max_samples_per_proxy: cfg.max_samples_per_proxy,
            assembler_caps: cfg.assembler_caps,
            unknown_src_count: 0,
            unmatched_highest: Vec::new(),
            cache_full_count: 0,
            best_effort: false,
            release_budget: None,
            departed: Vec::new(),
            departed_done: Vec::new(),
            departed_lost: 0,
            forwarded_without_proxy: Vec::new(),
            ended_writers: Vec::new(),
            forwarded_duplicate_count: 0,
            untrusted_forward_count: 0,
            refused_payload_count: 0,
            oversize_payload_count: 0,
            filtered_sample_count: 0,
        }
    }

    /// Records that the writer `guid`, which has no proxy here, announced
    /// changes up to `sn`.
    fn remember_unmatched(&mut self, guid: Guid, sn: SequenceNumber) {
        self.unknown_src_count = self.unknown_src_count.saturating_add(1);
        if sn.0 < 1 {
            return;
        }
        match self.unmatched_highest.iter_mut().find(|(g, _)| *g == guid) {
            Some((_, highest)) => {
                if sn > *highest {
                    *highest = sn;
                }
            }
            None => {
                if self.unmatched_highest.len() >= UNMATCHED_WRITERS_REMEMBERED {
                    self.unmatched_highest.remove(0);
                }
                self.unmatched_highest.push((guid, sn));
            }
        }
    }

    /// §8.4.10.1 `heartbeatSuppressionDuration` — HEARTBEATs from the
    /// same writer that arrive within this duration of the previous one
    /// are ignored. [`DEFAULT_HEARTBEAT_SUPPRESSION_DURATION`] (zero,
    /// the spec default) suppresses nothing.
    pub fn set_heartbeat_suppression_duration(&mut self, d: Duration) {
        self.heartbeat_suppression_duration = d;
    }

    /// The `heartbeatSuppressionDuration` attribute (§8.4.10.1).
    #[must_use]
    pub fn heartbeat_suppression_duration(&self) -> Duration {
        self.heartbeat_suppression_duration
    }

    /// The `heartbeatResponseDelay` attribute (§8.4.10.1).
    #[must_use]
    pub fn heartbeat_response_delay(&self) -> Duration {
        self.heartbeat_response_delay
    }

    /// Marks this reader as BEST_EFFORT (`true`) or RELIABLE (`false`, the
    /// default). Best-effort readers skip leading gaps instead of waiting (see
    /// the `best_effort` field).
    pub fn is_best_effort(&self) -> bool {
        self.best_effort
    }

    /// Sets the reader BEST_EFFORT (`true`) or RELIABLE (`false`).
    pub fn set_best_effort(&mut self, best_effort: bool) {
        self.best_effort = best_effort;
    }

    /// Bounds how many samples the next calls may hand to the consumer:
    /// `Some(n)` releases at most `n` more, `None` releases everything that
    /// is in order.
    ///
    /// A sample past the budget stays in its proxy's receive cache. It is
    /// received, so the next ACKNACK acknowledges it, and it is handed out
    /// in order by a later call once the consumer has room again
    /// ([`Self::release_held`]). The receive cache is bounded
    /// ([`ReliableReaderConfig::max_samples_per_proxy`]): a sample that
    /// finds it full stays MISSING and is not acknowledged, so a RELIABLE
    /// writer keeps it and, under HISTORY KEEP_ALL with finite
    /// RESOURCE_LIMITS, blocks its next write (DDS 1.4 §2.2.3.14,
    /// §2.2.2.4.2.11) — the back-pressure a full KEEP_ALL DataReader owes
    /// its writer.
    pub fn set_release_budget(&mut self, budget: Option<usize>) {
        self.release_budget = budget;
    }

    /// Sets the receive cache cap of every writer proxy, present and to come
    /// ([`ReliableReaderConfig::max_samples_per_proxy`]). A cap below what a
    /// proxy holds drops nothing: its next samples stay MISSING until it is
    /// back under the cap.
    pub fn set_max_samples_per_proxy(&mut self, max: usize) {
        self.max_samples_per_proxy = max;
        for state in &mut self.writer_proxies {
            let kind = state.received_cache.kind();
            state.received_cache.set_kind_and_max(kind, max);
        }
    }

    /// Whether a proxy holds a sample that is next in order and waits only
    /// for the consumer's room, or a writer that left holds one at all.
    #[must_use]
    pub fn holds_releasable(&self) -> bool {
        !self.departed.is_empty()
            || self.writer_proxies.iter().any(|state| {
                state
                    .received_cache
                    .get(SequenceNumber(state.delivered_up_to.0 + 1))
                    .is_some()
            })
    }

    /// Hands out, within the release budget, the samples every proxy holds
    /// in order — the ones an earlier call kept back for want of room.
    #[must_use = "the samples this call releases are delivered only through its return value"]
    pub fn release_held(&mut self) -> Vec<DeliveredSample> {
        let mut out = Vec::new();
        for state in self
            .writer_proxies
            .iter_mut()
            .chain(self.departed.iter_mut())
        {
            out.extend(Self::collect_in_order_for(
                state,
                self.best_effort,
                &mut self.release_budget,
            ));
        }
        self.commit_released(&mut out, 0);
        let mut drained = Vec::new();
        self.departed.retain(|state| {
            let held = !state.received_cache.is_empty();
            if !held {
                drained.push((state.proxy.remote_writer_guid, state.committed.clone()));
            }
            held
        });
        for (guid, _) in drained {
            self.departed_done.push(guid);
            self.note_ended(guid);
        }
        out
    }

    /// GUID.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Read-only slice of the writer-proxy states.
    #[must_use]
    pub fn writer_proxies(&self) -> &[WriterProxyState] {
        &self.writer_proxies
    }

    /// The capacity a new writer proxy's KEEP_ALL receive cache is given.
    ///
    /// Readable so a caller can assert what it configured actually arrived:
    /// the DCPS layer derives this from the reader's RESOURCE_LIMITS, and a
    /// value that silently stopped being handed through is exactly the kind
    /// of thing nothing else notices.
    #[must_use]
    pub fn max_samples_per_proxy(&self) -> usize {
        self.max_samples_per_proxy
    }

    /// Number of registered writer proxies.
    #[must_use]
    pub fn writer_proxy_count(&self) -> usize {
        self.writer_proxies.len()
    }

    /// Counter of sent ACKNACKs.
    #[must_use]
    pub fn acknack_count(&self) -> i32 {
        self.acknack_count
    }

    /// Counter of sent NACK_FRAGs.
    #[must_use]
    pub fn nackfrag_count(&self) -> i32 {
        self.nackfrag_count
    }

    /// Sum of the active (incomplete) fragment buffers across all
    /// proxies.
    #[must_use]
    pub fn pending_fragment_count(&self) -> usize {
        self.writer_proxies.iter().map(|s| s.assembler.len()).sum()
    }

    /// Number of samples that arrived intact but could not be stored because
    /// the per-proxy receive cache was full.
    ///
    /// Non-zero means the reader was handed data it then had to ask for again
    /// (RELIABLE) or drop (BEST_EFFORT). The cache fills when in-order
    /// delivery stalls behind a missing sequence number, so a single lost
    /// datagram can push several hundred later samples through this counter.
    /// The capacity is `ReliableReaderConfig::max_samples_per_proxy`.
    #[must_use]
    pub fn cache_full_count(&self) -> u64 {
        self.cache_full_count
    }

    /// Sum of the dropped fragments across all proxies
    /// (DoS / inconsistency diagnosis).
    #[must_use]
    pub fn dropped_fragment_count(&self) -> u64 {
        self.writer_proxies
            .iter()
            .map(|s| s.assembler.drop_count())
            .sum()
    }

    /// Number of DATA_FRAGs that arrived for already-known SNs
    /// (duplicate fragments, re-sends).
    #[must_use]
    pub fn duplicate_fragment_count(&self) -> u64 {
        self.duplicate_frag_count
    }

    /// Number of submessages whose `writer_id` could not be assigned to a registered
    /// proxy (misrouting / spoofing diagnosis).
    #[must_use]
    pub fn unknown_src_count(&self) -> u64 {
        self.unknown_src_count
    }

    /// Number of changes a GAP marked NOT_AVAILABLE with the sub-status
    /// `kind` (§8.4.10.4), summed over the writer proxies that are
    /// currently matched.
    #[must_use]
    pub fn not_available_count(&self, kind: NotAvailableKind) -> u64 {
        self.writer_proxies
            .iter()
            .map(|s| s.proxy.not_available_count(kind))
            .fold(0, u64::saturating_add)
    }

    /// The status of the change `sn` of the matched writer `writer`
    /// (Table 8.74 `status`); `None` when no such writer is matched or the
    /// proxy no longer keeps the status of `sn`
    /// ([`WriterProxy::change_status`]).
    #[must_use]
    pub fn change_status(
        &self,
        writer: Guid,
        sn: SequenceNumber,
    ) -> Option<ChangeFromWriterStatus> {
        self.writer_proxies
            .iter()
            .find(|s| s.proxy.remote_writer_guid == writer)
            .and_then(|s| s.proxy.change_status(sn))
    }

    /// Adds a writer proxy. Idempotent: for a known GUID the
    /// reliability state (SN bounds, received cache, delivered pointer)
    /// is **preserved** — only the locators are refreshed.
    ///
    /// Sets a preemptive ACKNACK as pending, so the writer gets a
    /// "hello, I'm here" ACKNACK on the next tick. Cyclone DDS
    /// responds with a HEARTBEAT and starts DATA resends —
    /// without this impulse the writer waits passively.
    ///
    /// Important: a renewed SPDP/SEDP announce of the same writer (Cyclone
    /// re-announces periodically) must NOT discard the reader progress.
    /// A reset would, after an already-processed HEARTBEAT, produce an empty
    /// ACKNACK ("nothing missing") → the reliable writer never delivers the
    /// DATA (cross-vendor secure-SEDP deadlock).
    pub fn add_writer_proxy(&mut self, proxy: WriterProxy) {
        let guid = proxy.remote_writer_guid;
        if let Some(idx) = self
            .writer_proxies
            .iter()
            .position(|s| s.proxy.remote_writer_guid == guid)
        {
            // Known: preserve the state, only refresh the locators.
            // DDSI-RTPS 2.5 §8.4.2.3: "In steady state, an ACKNACK Message
            // can only be sent as a response to a HEARTBEAT Message from a
            // Writer"; the exception is the Writer's first discovery, which
            // the other branch arms. A renewed announcement of a Writer
            // already known is not that.
            self.writer_proxies[idx]
                .proxy
                .refresh_locators(proxy.unicast_locators, proxy.multicast_locators);
        } else {
            let mut state =
                WriterProxyState::new(proxy, self.max_samples_per_proxy, self.assembler_caps);
            // Changes this writer sent before it was matched here are
            // available on the writer: the first ACKNACK requests them. The
            // lower bound stays where it is, so nothing is taken for lost.
            if let Some(pos) = self.unmatched_highest.iter().position(|(g, _)| *g == guid) {
                let (_, highest) = self.unmatched_highest.remove(pos);
                let first = state.proxy.first_available_sn();
                state.proxy.update_from_heartbeat(first, highest);
            }
            // What was forwarded on this writer's behalf before it had a
            // proxy counts as handed out (DDSI-RTPS 2.5 §8.7.9) — unless the
            // writer had a proxy before: then it comes back with a new
            // message stream, whose sequence numbers say nothing about the
            // old one's.
            let returning = match self.ended_writers.iter().position(|g| *g == guid) {
                Some(pos) => {
                    self.ended_writers.remove(pos);
                    true
                }
                None => false,
            };
            if let Some(pos) = self
                .forwarded_without_proxy
                .iter()
                .position(|(g, _)| *g == guid)
            {
                let forwarded = self.forwarded_without_proxy.remove(pos).1;
                if !returning {
                    state.committed = forwarded;
                }
            }
            // Duration::ZERO triggers an ACKNACK emit immediately on the next tick()
            // (now - ZERO >= heartbeat_response_delay).
            state.pending_acknack_since = Some(Duration::ZERO);
            self.writer_proxies.push(state);
        }
    }

    /// Removes a writer proxy.
    ///
    /// What the writer handed out is not kept: a writer of the same GUID
    /// that comes back starts a new message stream (see
    /// [`Self::add_writer_proxy`]).
    pub fn remove_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        let idx = self
            .writer_proxies
            .iter()
            .position(|s| s.proxy.remote_writer_guid == guid)?;
        let state = self.writer_proxies.remove(idx);
        self.note_ended(guid);
        Some(state.proxy)
    }

    /// Removes the proxy of a writer that left, and keeps what it delivered.
    ///
    /// A sample in the receive cache is received and acknowledged: the
    /// writer may have discarded it, and it waits only for the consumer's
    /// room ([`Self::set_release_budget`]). Removed with the proxy, it was
    /// gone without a trace. Measured with `zerodds_perf` and a reader that
    /// takes every 3 s: the writer process ended once
    /// `wait_for_acknowledgments` returned, and 85 248 of 485 249 samples
    /// never reached the reader. The samples stay, and
    /// [`Self::release_held`] hands them out in order. A change still
    /// missing in front of one of them can no longer arrive: it is counted
    /// ([`Self::take_lost`]) and delivery steps past it. Once all
    /// are handed out, [`Self::take_departed_done`] names the writer.
    ///
    /// `None` when `guid` has no proxy.
    pub fn depart_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        let idx = self.proxy_index_by_writer(guid)?;
        let mut state = self.writer_proxies.remove(idx);
        let proxy = state.proxy.clone();
        if let Some(highest) = state.received_cache.max_sn() {
            let first = state.proxy.first_available_sn();
            let mut sn = SequenceNumber(state.delivered_up_to.0 + 1);
            while sn < highest {
                if sn >= first
                    && state.received_cache.get(sn).is_none()
                    && !state.proxy.is_known(sn)
                {
                    state.proxy.irrelevant_change_set(sn);
                    self.departed_lost = self.departed_lost.saturating_add(1);
                }
                sn = SequenceNumber(sn.0 + 1);
            }
            self.departed.push(state);
        } else {
            self.note_ended(guid);
        }
        Some(proxy)
    }

    /// Whether a writer that left still has samples held for the consumer
    /// ([`Self::depart_writer_proxy`]).
    #[must_use]
    pub fn holds_departed(&self, guid: Guid) -> bool {
        self.departed
            .iter()
            .any(|s| s.proxy.remote_writer_guid == guid)
    }

    /// The writers that left whose held samples have all been handed out
    /// since the last call.
    pub fn take_departed_done(&mut self) -> Vec<Guid> {
        core::mem::take(&mut self.departed_done)
    }

    /// How many changes can no longer arrive, since the last call (DDS 1.4
    /// §2.2.4.1 SAMPLE_LOST): what writers that left had missing in front
    /// of a held sample, and per writer [`WriterProxyState::lost_changes`].
    /// A BEST_EFFORT reader's samples lost in transit counted nowhere: the
    /// reader stepped over the gap, and SAMPLE_LOST stayed 0.
    pub fn take_lost(&mut self) -> u64 {
        let mut lost = core::mem::take(&mut self.departed_lost);
        for state in self
            .writer_proxies
            .iter_mut()
            .chain(self.departed.iter_mut())
        {
            lost = lost.saturating_add(core::mem::take(&mut state.lost_changes));
        }
        lost
    }

    /// Zeroes all diagnostic counters. Touches no state machine.
    pub fn reset_diagnostics(&mut self) {
        self.acknack_count = 0;
        self.nackfrag_count = 0;
        self.duplicate_frag_count = 0;
        self.unknown_src_count = 0;
        for s in &mut self.writer_proxies {
            s.assembler.reset_diagnostics();
        }
    }

    /// Whether the `PID_ORIGINAL_WRITER_INFO` of the writer `writer` is
    /// honoured (DDSI-RTPS 2.5 §8.7.9). A writer is trusted when its proxy
    /// is added; DDS-Security 1.2 §7.1.1.4 accepts a relayed sample only
    /// from a participant allowed to relay, and the security layer clears
    /// the trust of the others. A change of an untrusted writer with the
    /// parameter is handed out as its own. No-op without a proxy.
    pub fn set_forwarder_trusted(&mut self, writer: Guid, trusted: bool) {
        if let Some(idx) = self.proxy_index_by_writer(writer) {
            self.writer_proxies[idx].forwarder_trusted = trusted;
        }
    }

    /// DATA with FilteredFlag 1 (§9.6.4.9): known, not delivered.
    #[must_use]
    pub fn filtered_sample_count(&self) -> u64 {
        self.filtered_sample_count
    }

    /// Records the `dataMaxSizeSerialized` a matched writer announced
    /// (DDSI-RTPS 2.5 Table 8.72): "the maximum size of any SerializedPayload
    /// that may be sent by the matched Writer". A payload beyond it is
    /// refused (see [`Self::oversize_payload_count`]). `None` removes the
    /// bound; a negative announced value is no bound. No-op without a proxy.
    pub fn set_writer_data_max_size(&mut self, writer: Guid, max: Option<i32>) {
        if let Some(idx) = self.proxy_index_by_writer(writer) {
            self.writer_proxies[idx].data_max_size = max.and_then(|m| u32::try_from(m).ok());
        }
    }

    /// Payloads larger than the `dataMaxSizeSerialized` their writer
    /// announced; each was refused and its sequence number taken as not
    /// available.
    #[must_use]
    pub fn oversize_payload_count(&self) -> u64 {
        self.oversize_payload_count
    }

    /// Changes whose payload the [`PayloadOpener`] refused: not received,
    /// asked for again.
    #[must_use]
    pub fn refused_payload_count(&self) -> u64 {
        self.refused_payload_count
    }

    /// Changes dropped as handed out already (DDSI-RTPS 2.5 §8.7.9: "it
    /// can treat them as duplicates").
    #[must_use]
    pub fn forwarded_duplicate_count(&self) -> u64 {
        self.forwarded_duplicate_count
    }

    /// Changes with `PID_ORIGINAL_WRITER_INFO` from a writer whose
    /// forwarding is not honoured ([`Self::set_forwarder_trusted`]).
    #[must_use]
    pub fn untrusted_forward_count(&self) -> u64 {
        self.untrusted_forward_count
    }

    /// The receive state of `writer`, matched or departed with samples held.
    fn state_of_mut(&mut self, writer: Guid) -> Option<&mut WriterProxyState> {
        self.writer_proxies
            .iter_mut()
            .chain(self.departed.iter_mut())
            .find(|s| s.proxy.remote_writer_guid == writer)
    }

    /// Records that the proxy of `writer` went (see `ended_writers`).
    fn note_ended(&mut self, writer: Guid) {
        self.ended_writers.retain(|g| *g != writer);
        if self.ended_writers.len() >= ORIGINAL_WRITERS_REMEMBERED {
            self.ended_writers.remove(0);
        }
        self.ended_writers.push(writer);
    }

    /// Keeps what was forwarded on behalf of `writer`, which has no proxy.
    fn remember_forwarded(&mut self, writer: Guid, committed: SnRanges) {
        if committed.ranges.is_empty() {
            return;
        }
        self.forwarded_without_proxy.retain(|(g, _)| *g != writer);
        if self.forwarded_without_proxy.len() >= ORIGINAL_WRITERS_REMEMBERED {
            self.forwarded_without_proxy.remove(0);
        }
        self.forwarded_without_proxy.push((writer, committed));
    }

    /// Records the change `sn` of `original`, forwarded on its behalf;
    /// `false` when it was handed out already or waits in the receive cache
    /// of the original's proxy, where it is delivered from.
    fn commit_forwarded(&mut self, original: Guid, sn: SequenceNumber) -> bool {
        if let Some(state) = self.state_of_mut(original) {
            if state.received_cache.get(sn).is_some() {
                return false;
            }
            return state.committed.insert(sn.0);
        }
        if let Some((_, committed)) = self
            .forwarded_without_proxy
            .iter_mut()
            .find(|(g, _)| *g == original)
        {
            return committed.insert(sn.0);
        }
        let mut committed = SnRanges::default();
        committed.insert(sn.0);
        self.remember_forwarded(original, committed);
        true
    }

    /// A change of `original`, numbered `sn` there, that a writer of this
    /// reader's own participant forwards on its behalf and hands over past
    /// the RTPS receive path (a durability or routing service in the same
    /// runtime): whether it is new to the reader (DDSI-RTPS 2.5 §8.7.9). One
    /// the reader handed out already, from the original or a forwarder, or
    /// one waiting in the original's receive cache, is a duplicate and
    /// counted as one ([`Self::forwarded_duplicate_count`]); a change the
    /// reader then receives from the original or another forwarder is the
    /// duplicate.
    pub fn accept_forwarded(&mut self, original: Guid, sn: SequenceNumber) -> bool {
        let fresh = self.commit_forwarded(original, sn);
        if !fresh {
            self.forwarded_duplicate_count = self.forwarded_duplicate_count.saturating_add(1);
        }
        fresh
    }

    /// Commits the samples released into `out[from..]` (DDSI-RTPS 2.5
    /// §8.7.9). "When a RTPS Reader receives this information, it will treat
    /// it as a normal CacheChange, but once the CacheChange is ready to be
    /// committed to the DDS DataReader, it will not commit it. Instead, it
    /// will hand if off to the HistoryCache of the RTPS Reader that is
    /// communicating with the RTPS Writer indicated in the
    /// ORIGINAL_WRITER_INFO in-line QoS and treat is as having the sequence
    /// number which appears there." A forwarded change is handed out as
    /// the original's; one handed out already, from the original or a
    /// forwarder, is dropped as the duplicate it is. End-of-coherent-set
    /// deliveries carry no sample and are left as they are.
    fn commit_released(&mut self, out: &mut Vec<DeliveredSample>, from: usize) {
        let mut i = from;
        while i < out.len() {
            if out[i].coherent_set_end {
                i += 1;
                continue;
            }
            let source = out[i].writer_guid;
            let sn = out[i].sequence_number;
            let trusted = self
                .state_of_mut(source)
                .is_some_and(|state| state.forwarder_trusted);
            let keep = match out[i].original_writer {
                Some((original, original_sn)) if trusted && original != source => {
                    let fresh = self.commit_forwarded(original, original_sn);
                    if fresh {
                        let sample = &mut out[i];
                        sample.writer_guid = original;
                        sample.sequence_number = original_sn;
                        sample.forwarded_by = Some(source);
                    }
                    fresh
                }
                forwarded => {
                    if forwarded.is_some_and(|(original, _)| original != source) {
                        self.untrusted_forward_count =
                            self.untrusted_forward_count.saturating_add(1);
                    }
                    self.state_of_mut(source)
                        .is_none_or(|state| state.committed.insert(sn.0))
                }
            };
            if keep {
                i += 1;
            } else {
                self.forwarded_duplicate_count = self.forwarded_duplicate_count.saturating_add(1);
                out.remove(i);
            }
        }
    }

    // ---------- Incoming Submessages ----------

    /// Process a DATA. Dispatch by `writer_id` to the matching
    /// proxy. Returns the reassembled samples of this proxy.
    ///
    /// Spec §9.6.3.9 PID_STATUS_INFO: with `key_flag=true` + inline QoS
    /// with STATUS_INFO set, the CacheChange is marked
    /// NotAliveDisposed / NotAliveUnregistered / NotAliveDisposedUnregistered
    /// instead of Alive.
    #[must_use = "the samples this call releases are delivered only through its return value"]
    pub fn handle_data(
        &mut self,
        source_prefix: GuidPrefix,
        data: &DataSubmessage,
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
    ) -> Vec<DeliveredSample> {
        let mut out = Vec::new();
        self.handle_data_into(source_prefix, data, source_timestamp, &mut out);
        out
    }

    /// [`Self::handle_data`], appending the released samples to `out`: a
    /// receive path that hands a reader a run of DATA uses one list for
    /// the run, where a list per DATA was an allocation per received
    /// sample.
    pub fn handle_data_into(
        &mut self,
        source_prefix: GuidPrefix,
        data: &DataSubmessage,
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        out: &mut Vec<DeliveredSample>,
    ) {
        self.handle_data_opening(source_prefix, data, source_timestamp, out, None::<&AsIs>);
    }

    /// [`Self::handle_data_into`], the payload opened by `opener` before
    /// the change is received ([`PayloadOpener`]).
    pub fn handle_data_opening<O: PayloadOpener>(
        &mut self,
        source_prefix: GuidPrefix,
        data: &DataSubmessage,
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        out: &mut Vec<DeliveredSample>,
        opener: Option<&O>,
    ) {
        let from = out.len();
        self.receive_data(source_prefix, data, source_timestamp, out, opener);
        self.commit_released(out, from);
    }

    /// [`Self::handle_data_opening`] without the commit of what it releases.
    fn receive_data<O: PayloadOpener>(
        &mut self,
        source_prefix: GuidPrefix,
        data: &DataSubmessage,
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        out: &mut Vec<DeliveredSample>,
        opener: Option<&O>,
    ) {
        let Some(idx) = self.proxy_index_by_writer(Guid::new(source_prefix, data.writer_id)) else {
            self.remember_unmatched(Guid::new(source_prefix, data.writer_id), data.writer_sn);
            return;
        };
        let own = self.guid;
        let state = &mut self.writer_proxies[idx];
        let sn = data.writer_sn;
        if state.proxy.is_known(sn) || sn <= state.delivered_up_to {
            return;
        }
        let kind = Self::classify_change_kind(data);
        // §9.6.4.9: F = 1 says the writer wrote a sample that did not pass
        // the content filter of this reader. The change is known (the
        // writer must not repeat it) and is not delivered; it is counted.
        if kind == ChangeKind::AliveFiltered {
            self.filtered_sample_count = self.filtered_sample_count.saturating_add(1);
            state
                .proxy
                .not_available_change_set(sn, NotAvailableKind::Filtered);
            Self::collect_in_order_into(state, self.best_effort, &mut self.release_budget, out);
            return;
        }
        // Key-only ALIVE sample (K-flag set, D-flag clear): the payload holds
        // only the @key fields — an instance registration (e.g. OpenDDS
        // `register_instance`), NOT a full sample. Full-decoding its key-only
        // payload fails with a spurious cross-vendor decode error (OpenDDS sends
        // one key-only DATA per data DATA). Mark the SN received so reliability
        // advances past it (collect_in_order_for skips a known-but-uncached SN),
        // but do not deliver it: the actual data arrives in the D-flag samples.
        // Dispose/unregister key-only markers (kind != Alive) DO carry lifecycle
        // semantics and fall through to be delivered below.
        if data.key_flag && kind == ChangeKind::Alive {
            // Treat like a GAP for this one SN: mark it irrelevant, so the
            // reliable reader does not stall waiting to "fill" a registration
            // that will never be delivered. The subsequent D-flag data SN then
            // flows.
            state.proxy.irrelevant_change_set(sn);
            Self::collect_in_order_into(state, self.best_effort, &mut self.release_budget, out);
            return;
        }
        // DDSI-RTPS 2.5 §8.7.7: "When a writer sends a directed sample, only
        // recipients with a matching GUID accept the sample; all other
        // recipients acknowledge but absorb the sample, as if it were a GAP
        // message."
        if addressed_elsewhere(data.inline_qos.as_ref(), own) {
            state.proxy.irrelevant_change_set(sn);
            Self::collect_in_order_into(state, self.best_effort, &mut self.release_budget, out);
            return;
        }
        let original_writer = original_writer_of(data.inline_qos.as_ref());
        let payload = match opener.map(|opener| {
            opener.open(
                state.proxy.remote_writer_guid,
                original_writer
                    .filter(|_| state.forwarder_trusted)
                    .map(|(guid, _)| guid),
                &data.serialized_payload,
                data.non_standard_flag,
            )
        }) {
            None | Some(PayloadOpening::AsIs) => data.serialized_payload.clone(),
            Some(PayloadOpening::Opened(payload)) => payload,
            Some(PayloadOpening::Refused) => {
                self.refused_payload_count = self.refused_payload_count.saturating_add(1);
                return;
            }
        };
        // `dataMaxSizeSerialized` bounds the payload as the writer produced
        // it, so it is checked on the opened one.
        if state
            .data_max_size
            .is_some_and(|max| payload.len() as u64 > u64::from(max))
        {
            self.oversize_payload_count = self.oversize_payload_count.saturating_add(1);
            state.proxy.irrelevant_change_set(sn);
            Self::collect_in_order_into(state, self.best_effort, &mut self.release_budget, out);
            return;
        }
        let opened = opener.is_some();
        state.proxy.received_change_set(sn);
        let key_hash = data
            .inline_qos
            .as_ref()
            .and_then(crate::inline_qos::find_key_hash);
        // DDSI-RTPS 2.5 §8.7.6: `PID_COHERENT_SET` names the set the sample
        // belongs to. The value is a `SequenceNumber_t` in the byte order of
        // the submessage, which the parameter list keeps.
        let coherent_set_sn = data
            .inline_qos
            .as_ref()
            .and_then(|pl| crate::inline_qos::find_coherent_set(pl, pl.little_endian));
        // The ECS marker is a DATA with D and K both clear (§8.7.6). It has no
        // payload and must not be delivered as a sample, but it must reach the
        // consumer so the buffered set can be released. Table 9.22 gives it
        // `PID_COHERENT_SET` = `SEQUENCENUMBER_UNKNOWN` (this stack) or no
        // such PID (RTI Connext 7.7.0); HDDS 1.4.8 sends the set's last SN,
        // a value of no open set, so a new set that ends the open one
        // (measured on the OMG dds-rtps suite). Each empty DATA is an ECS
        // and ends the writer's open set, whatever it names.
        //
        // But a dispose/unregister marker takes the SAME wire shape — an empty
        // DATA with D and K both clear, carrying `PID_STATUS_INFO` and
        // `PID_KEY_HASH` (the form `write_user_lifecycle` sends, §9.6.4.8). It
        // is NOT a coherent terminator: `classify_change_kind` already read its
        // status bits into `kind`, so an empty DATA closes a coherent set only
        // when it carries no lifecycle status (`kind == Alive`); otherwise it
        // falls through and is delivered as the NotAlive* marker it is.
        let coherent_set_end = !data.data_flag && !data.key_flag && kind == ChangeKind::Alive;
        let group = data
            .inline_qos
            .as_ref()
            .and_then(crate::inline_qos::find_change_group_info);
        // The next sample in order, nothing waiting before it and room at
        // the consumer: it is delivered as it is, without going into the
        // receive cache and straight out again. That round trip was a cache
        // node, a copy of the inline QoS and a removal per received sample;
        // what is delivered is the same `DeliveredSample`.
        //
        // A BEST_EFFORT reader takes any sample past the last it delivered:
        // it waits for no missing one (§8.4.12.1), and through the cache a
        // sample behind a gap was stored, then delivered by skipping to it.
        // Everything before it is lost then; the proxy forgets it, or its
        // received set would keep every SN behind every gap, and a
        // BEST_EFFORT writer sends no HEARTBEAT that would cut it.
        let gap_skipped = sn.0 > state.delivered_up_to.0 + 1;
        if state.received_cache.is_empty()
            && (!gap_skipped || self.best_effort)
            && self.release_budget != Some(0)
        {
            Self::hand_out(
                state,
                DeliveredSample {
                    writer_guid: state.proxy.remote_writer_guid,
                    sequence_number: sn,
                    payload: payload.clone(),
                    kind,
                    key_hash,
                    source_timestamp,
                    coherent_set_sn,
                    coherent_set_end,
                    related_sample_identity: data.inline_qos.as_ref().and_then(|pl| {
                        crate::inline_qos::find_related_sample_identity(pl, pl.little_endian)
                            .ok()
                            .flatten()
                    }),
                    eprosima_related_sample_identity: data.inline_qos.as_ref().and_then(|pl| {
                        crate::inline_qos::find_eprosima_related_sample_identity(
                            pl,
                            pl.little_endian,
                        )
                    }),
                    group,
                    coherent_set_complete: None,
                    original_writer,
                    forwarded_by: None,
                    non_standard_payload: data.non_standard_flag && !opened,
                    inline_qos_override: crate::inline_qos::InlineQosOverride::of(
                        data.inline_qos.as_ref(),
                    ),
                },
                out,
            );
            state.delivered_up_to = sn;
            if gap_skipped {
                let lost = state.proxy.lost_changes_update(SequenceNumber(sn.0 + 1));
                state.note_lost(lost);
            }
            state.delivered_any = true;
            if let Some(left) = self.release_budget.as_mut() {
                *left -= 1;
            }
            // With the cache empty there is only more to hand out when the
            // SNs right after this one are known already (a GAP) or no
            // longer available.
            let next = SequenceNumber(sn.0 + 1);
            if !state.received_cache.is_empty()
                || state.proxy.is_known(next)
                || next < state.proxy.first_available_sn()
            {
                Self::collect_in_order_into(state, self.best_effort, &mut self.release_budget, out);
            }
            return;
        }
        // Arc::clone instead of Vec::clone on the payload — the
        // refcount block is shared between DataSubmessage, cache and
        // DeliveredSample.
        if !Self::store_received(
            state,
            CacheChange {
                inline_qos: data.inline_qos.clone(),
                sequence_number: sn,
                // The receive cache holds payloads of their own: a change
                // that waits there must not keep the whole datagram alive.
                payload: payload.to_arc(),
                kind,
                key_hash,
                // From the preceding INFO_TS submessage (DDSI-RTPS §8.7.3).
                source_timestamp,
                coherent_set_sn,
                coherent_set_end,
                group,
                non_standard_payload: data.non_standard_flag && !opened,
                destination: None,
                destination_readers: None,
            },
        ) {
            self.cache_full_count = self.cache_full_count.saturating_add(1);
            return;
        }
        Self::collect_in_order_into(state, self.best_effort, &mut self.release_budget, out);
    }

    /// Classifies an incoming DATA as Alive vs lifecycle marker.
    ///
    /// §9.6.3.9 puts the lifecycle semantics in `PID_STATUS_INFO`, not in the
    /// flags: D and K say what `serializedPayload` holds (§8.3.7.2.5 Table
    /// 8.43), and a marker may hold a serialized key (`K=1`) or nothing at
    /// all (`K=0`). Both forms are on the wire — Cyclone 11.0.1 and OpenDDS
    /// 3.34.0 send the first, Fast DDS 3.6.1 the second.
    ///
    /// Keying the classification on `key_flag` therefore read Fast DDS's
    /// dispose as an ALIVE sample with an empty payload; on SEDP that made
    /// the announcement decoder fail on the whole datagram instead of
    /// unmatching the endpoint. The gate is `!data_flag`: a DATA that carries
    /// no sample body and says `disposed`/`unregistered` is a marker.
    fn classify_change_kind(data: &DataSubmessage) -> ChangeKind {
        if data.data_flag {
            // A sample body: D and U name nothing here (see above), F still
            // says the sample did not pass the reader's filter (§9.6.4.9).
            let filtered = data
                .inline_qos
                .as_ref()
                .and_then(crate::inline_qos::find_status_info)
                .is_some_and(|bits| status_change_kind(bits) == ChangeKind::AliveFiltered);
            return if filtered {
                ChangeKind::AliveFiltered
            } else {
                ChangeKind::Alive
            };
        }
        data.inline_qos
            .as_ref()
            .and_then(crate::inline_qos::find_status_info)
            .map_or(ChangeKind::Alive, status_change_kind)
    }

    /// Process a DATA_FRAG. `now` triggers NACK_FRAG scheduling
    /// directly, without waiting for a HEARTBEAT.
    #[must_use = "the samples this call releases are delivered only through its return value"]
    pub fn handle_data_frag(
        &mut self,
        source_prefix: GuidPrefix,
        df: &DataFragSubmessage,
        now: Duration,
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
    ) -> Vec<DeliveredSample> {
        self.handle_data_frag_opening(source_prefix, df, now, source_timestamp, None::<&AsIs>)
    }

    /// [`Self::handle_data_frag`], the reassembled payload opened by
    /// `opener` before the change is received ([`PayloadOpener`]). `_now`
    /// is the time of arrival; a fragment arms no response (only a
    /// HEARTBEAT or HEARTBEAT_FRAG does), so nothing here reads it.
    #[must_use = "the samples this call releases are delivered only through its return value"]
    pub fn handle_data_frag_opening<O: PayloadOpener>(
        &mut self,
        source_prefix: GuidPrefix,
        df: &DataFragSubmessage,
        _now: Duration,
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        opener: Option<&O>,
    ) -> Vec<DeliveredSample> {
        let Some(idx) = self.proxy_index_by_writer(Guid::new(source_prefix, df.writer_id)) else {
            self.remember_unmatched(Guid::new(source_prefix, df.writer_id), df.writer_sn);
            return Vec::new();
        };
        let state = &mut self.writer_proxies[idx];
        let sn = df.writer_sn;
        if state.proxy.is_known(sn) || sn <= state.delivered_up_to {
            self.duplicate_frag_count = self.duplicate_frag_count.saturating_add(1);
            return Vec::new();
        }
        let mut cache_was_full = false;
        let own = self.guid;
        let mut result = if let Some(mut completed) = state.assembler.insert(df) {
            // The whole payload a DATA_FRAG sequence carries is what the
            // writer protected (DDS-Security 1.2 §9.5.3.3.4.4): it is opened
            // once reassembled. Refused, the change is not received; the
            // writer sends its fragments again.
            let original_writer = original_writer_of(completed.inline_qos.as_ref())
                .filter(|_| state.forwarder_trusted)
                .map(|(guid, _)| guid);
            if let Some(opener) = opener {
                let whole = crate::payload::Payload::from(core::mem::take(&mut completed.payload));
                match opener.open(
                    state.proxy.remote_writer_guid,
                    original_writer,
                    &whole,
                    df.non_standard_flag,
                ) {
                    PayloadOpening::AsIs => completed.payload = whole.as_ref().to_vec(),
                    PayloadOpening::Opened(opened) => completed.payload = opened.as_ref().to_vec(),
                    PayloadOpening::Refused => {
                        self.refused_payload_count = self.refused_payload_count.saturating_add(1);
                        return Vec::new();
                    }
                }
            }
            if state
                .data_max_size
                .is_some_and(|max| completed.payload.len() as u64 > u64::from(max))
            {
                self.oversize_payload_count = self.oversize_payload_count.saturating_add(1);
                state.proxy.irrelevant_change_set(sn);
                let mut released =
                    Self::collect_in_order_for(state, self.best_effort, &mut self.release_budget);
                self.commit_released(&mut released, 0);
                return released;
            }
            let non_standard = df.non_standard_flag && opener.is_none();
            // §8.3.8.3: "Once the serializedData is re-assembled by the RTPS
            // Reader, the interpretation of the DataFrag Submessages is
            // identical to that of the Data Submessage". The first
            // fragment's in-line QoS is the sample's. A serialized key with
            // PID_STATUS_INFO is the dispose or unregister it names; F = 1
            // with D = U = 0 is a sample that did not pass the reader's
            // filter (§9.6.4.9), with or without the key flag.
            let status_kind = completed
                .inline_qos
                .as_ref()
                .and_then(crate::inline_qos::find_status_info)
                .map_or(ChangeKind::Alive, status_change_kind);
            let kind = if completed.key_flag || status_kind == ChangeKind::AliveFiltered {
                status_kind
            } else {
                ChangeKind::Alive
            };
            if kind == ChangeKind::AliveFiltered {
                self.filtered_sample_count = self.filtered_sample_count.saturating_add(1);
                state
                    .proxy
                    .not_available_change_set(sn, NotAvailableKind::Filtered);
                let mut released =
                    Self::collect_in_order_for(state, self.best_effort, &mut self.release_budget);
                self.commit_released(&mut released, 0);
                return released;
            }
            // A key-only ALIVE sample is an instance registration, absorbed
            // as for a DATA (see `receive_data`).
            if completed.key_flag && kind == ChangeKind::Alive {
                state.proxy.irrelevant_change_set(sn);
                let mut released =
                    Self::collect_in_order_for(state, self.best_effort, &mut self.release_budget);
                self.commit_released(&mut released, 0);
                return released;
            }
            state.proxy.received_change_set(sn);
            // §8.7.7: a directed sample for another reader, absorbed as by
            // a GAP; the first fragment's in-line QoS is the sample's.
            if addressed_elsewhere(completed.inline_qos.as_ref(), own) {
                state.proxy.irrelevant_change_set(sn);
                let released =
                    Self::collect_in_order_for(state, self.best_effort, &mut self.release_budget);
                let mut released = released;
                self.commit_released(&mut released, 0);
                return released;
            }
            // The first fragment's in-line QoS is the sample's: a coherent
            // set member travels as DATA_FRAG once it outgrows a DATA
            // (DDSI-RTPS 2.5 §8.7.6).
            let coherent_set_sn = completed
                .inline_qos
                .as_ref()
                .and_then(|pl| crate::inline_qos::find_coherent_set(pl, pl.little_endian));
            let mut change = CacheChange::alive(sn, completed.payload);
            change.kind = kind;
            if !Self::store_received(
                state,
                change
                    .with_key_hash(completed.key_hash)
                    .with_group(
                        completed
                            .inline_qos
                            .as_ref()
                            .and_then(crate::inline_qos::find_change_group_info),
                    )
                    .with_inline_qos(completed.inline_qos)
                    .with_source_timestamp(source_timestamp)
                    .with_coherent_set(coherent_set_sn)
                    .with_non_standard_payload(non_standard),
            ) {
                cache_was_full = true;
                Vec::new()
            } else {
                Self::collect_in_order_for(state, self.best_effort, &mut self.release_budget)
            }
        } else {
            Vec::new()
        };
        if cache_was_full {
            self.cache_full_count = self.cache_full_count.saturating_add(1);
        }
        // No NACK_FRAG and no ACKNACK for the fragments that are still
        // missing: DDSI-RTPS 2.5 §8.4.14.1 "A NackFrag Submessage can only be
        // sent in response to a Heartbeat or HeartbeatFrag submessage" and
        // §8.4.2.3 allows an ACKNACK in steady state only as the response to
        // a HEARTBEAT. `handle_heartbeat` and `handle_heartbeat_frag` arm
        // the response; the fragments in flight are not yet missing.
        self.commit_released(&mut result, 0);
        result
    }

    /// Process a HEARTBEAT_FRAG (DDSI-RTPS 2.5 §8.3.8.7): "The RTPS Reader
    /// will respond by sending a NackFrag message, but only if it is
    /// missing any of the available fragments" — fragments up to
    /// `lastFragmentNum` of a change it holds part of. The NACK_FRAG goes
    /// out on the next [`Self::tick_outbound`]. Returns whether one is due.
    pub fn handle_heartbeat_frag(
        &mut self,
        source_prefix: GuidPrefix,
        hbf: &crate::submessages::HeartbeatFragSubmessage,
        now: Duration,
    ) -> bool {
        let Some(idx) = self.proxy_index_by_writer(Guid::new(source_prefix, hbf.writer_id)) else {
            self.unknown_src_count = self.unknown_src_count.saturating_add(1);
            return false;
        };
        if self.best_effort || hbf.writer_sn.0 <= 0 || hbf.last_fragment_num.0 == 0 {
            return false;
        }
        let state = &mut self.writer_proxies[idx];
        let missing = state
            .assembler
            .missing_fragments(hbf.writer_sn)
            .iter_set()
            .any(|f| f.0 <= hbf.last_fragment_num.0);
        if missing {
            state.pending_acknack_since.get_or_insert(now);
        }
        missing
    }

    /// Process a HEARTBEAT. Dispatch by `writer_id`. A HEARTBEAT whose
    /// `first_sn` passes a hole releases the samples waiting behind it;
    /// they are returned here like the ones a DATA or GAP releases.
    #[must_use = "the samples this call releases are delivered only through its return value"]
    pub fn handle_heartbeat(
        &mut self,
        source_prefix: GuidPrefix,
        hb: &HeartbeatSubmessage,
        now: Duration,
    ) -> Vec<DeliveredSample> {
        let Some(idx) = self.proxy_index_by_writer(Guid::new(source_prefix, hb.writer_id)) else {
            if !hb.liveliness_flag {
                self.remember_unmatched(Guid::new(source_prefix, hb.writer_id), hb.last_sn);
            } else {
                self.unknown_src_count = self.unknown_src_count.saturating_add(1);
            }
            return Vec::new();
        };
        let suppression = self.heartbeat_suppression_duration;
        let state = &mut self.writer_proxies[idx];
        // §8.4.10.1 `heartbeatSuppressionDuration`: ignore a HEARTBEAT
        // that arrives "too soon" after the previous one from this
        // writer. Zero (the spec default) suppresses nothing.
        if !suppression.is_zero() {
            if let Some(prev) = state.last_heartbeat_at {
                if now.saturating_sub(prev) < suppression {
                    return Vec::new();
                }
            }
        }
        state.last_heartbeat_at = Some(now);
        let lost = state.proxy.update_from_heartbeat(hb.first_sn, hb.last_sn);
        state.note_lost(lost);
        let has_missing = state.proxy.has_missing_changes();
        let has_frag_gaps = state.assembler.has_gaps();
        // Table 8.77 T2: "if (HB.FinalFlag == NOT_SET) then must_send_ack
        // else if (HB.LivelinessFlag == NOT_SET) then may_send_ack else
        // waiting"; T7 takes firstSN and lastSN in either case.
        if !hb.final_flag || (!hb.liveliness_flag && (has_missing || has_frag_gaps)) {
            state.pending_acknack_since.get_or_insert(now);
        }
        // An HB with first_sn > delivered_up_to+1 means that samples
        // before first_sn are "lost". `collect_in_order_for` then advances
        // `delivered_up_to` to first_sn-1 and delivers samples from
        // the received_cache that were waiting on the hole fill (e.g. a
        // volatile direct send with SN > delivered_up_to+1).
        let mut released =
            Self::collect_in_order_for(state, self.best_effort, &mut self.release_budget);
        self.commit_released(&mut released, 0);
        released
    }

    /// Process a GAP. Dispatch by `writer_id`.
    #[must_use = "the samples this call releases are delivered only through its return value"]
    pub fn handle_gap(
        &mut self,
        source_prefix: GuidPrefix,
        gap: &GapSubmessage,
    ) -> Vec<DeliveredSample> {
        let Some(idx) = self.proxy_index_by_writer(Guid::new(source_prefix, gap.writer_id)) else {
            self.unknown_src_count = self.unknown_src_count.saturating_add(1);
            return Vec::new();
        };
        let state = &mut self.writer_proxies[idx];
        // §8.4.10.4 `not_available_change_set(set, filteredCount)`: the set
        // is gapStart..gapList.bitmapBase-1 plus the bits of gapList. A GAP
        // without filteredCount (flag F clear) counts as 0: the changes
        // were removed from the writer cache.
        let below_base = u64::try_from(gap.gap_list.bitmap_base.0 - gap.gap_start.0).unwrap_or(0);
        let total = below_base.saturating_add(gap.gap_list.iter_set().count() as u64);
        let kind = NotAvailableKind::for_gap(total, gap.filtered_count.unwrap_or(0));
        let mut sn = gap.gap_start;
        while sn < gap.gap_list.bitmap_base {
            state.proxy.not_available_change_set(sn, kind);
            state.assembler.discard(sn);
            sn = SequenceNumber(sn.0 + 1);
        }
        for sn in gap.gap_list.iter_set() {
            state.proxy.not_available_change_set(sn, kind);
            state.assembler.discard(sn);
        }
        let mut released =
            Self::collect_in_order_for(state, self.best_effort, &mut self.release_budget);
        self.commit_released(&mut released, 0);
        released
    }

    /// Tick: returns due ACKNACK/NACK_FRAG datagrams **across all
    /// proxies**. Per proxy its own ACKNACK/NACK_FRAG, because
    /// SN spaces are per writer.
    ///
    /// # Errors
    /// Wire encode error.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<Vec<u8>>, WireError> {
        Ok(self
            .tick_outbound(now)?
            .into_iter()
            .map(|d| d.bytes)
            .collect())
    }

    /// Like [`Self::tick`], but with target locators for each datagram.
    /// Preferred for transport integration, because each AckNack must go to
    /// the concrete writer-proxy unicast locator.
    ///
    /// # Errors
    /// `WireError::ValueOutOfRange` for an overlong submessage body.
    pub fn tick_outbound(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = Vec::new();
        // DDSI-RTPS 2.5 §8.4.2.3: "A best-effort Reader is completely
        // passive as it only receives data and does not send messages
        // itself." No ACKNACK and no NACK_FRAG, whatever a HEARTBEAT or a
        // new writer asked for.
        if self.best_effort {
            for state in &mut self.writer_proxies {
                state.pending_acknack_since = None;
            }
            return Ok(out);
        }
        for idx in 0..self.writer_proxies.len() {
            let Some(since) = self.writer_proxies[idx].pending_acknack_since else {
                continue;
            };
            if now.saturating_sub(since) < self.heartbeat_response_delay {
                continue;
            }
            // DDSI-RTPS 2.5 §8.4.2.3: a reader reports missing samples "only
            // if the Reader can accommodate these missing samples in its
            // cache", and "the response may be delayed". A receive cache
            // that is full of samples the consumer has not taken yet keeps
            // the response pending until it takes; the ACKNACK it would send
            // now would name changes it could not hold, and an empty one
            // reads to a writer as a pure ACK that asks for the whole tail.
            // A cache that is full behind a missing change is not held back:
            // that change is what lets the cache drain.
            if Self::full_for_the_consumer(&self.writer_proxies[idx]) {
                continue;
            }
            self.writer_proxies[idx].pending_acknack_since = None;
            let targets =
                alloc::sync::Arc::new(self.writer_proxies[idx].proxy.unicast_locators.clone());

            let incomplete_sns: Vec<SequenceNumber> = self.writer_proxies[idx]
                .assembler
                .incomplete_sns()
                .collect();
            for sn in incomplete_sns {
                let bytes = self.build_nackfrag_datagram(idx, sn)?;
                out.push(OutboundDatagram {
                    bytes,
                    targets: alloc::sync::Arc::clone(&targets),
                });
            }
            let bytes = self.build_acknack_datagram(idx)?;
            out.push(OutboundDatagram { bytes, targets });
        }
        Ok(out)
    }

    // ---------- Internal ----------

    /// Finds the writer proxy by the **full writer GUID** (source
    /// `guid_prefix` from the RTPS header + `writerId` of the submessage).
    /// DDSI-RTPS 2.5 §8.3.4: the effective source of a writer submessage is
    /// `Receiver.sourceGuidPrefix` + `writerId`; the EntityId alone
    /// does NOT uniquely identify a remote writer (multiple participants
    /// share the same per-participant base-assigned EntityId), otherwise
    /// DATA/HB/GAP of two writers end up in the same proxy state.
    fn proxy_index_by_writer(&self, guid: Guid) -> Option<usize> {
        self.writer_proxies
            .iter()
            .position(|s| s.proxy.remote_writer_guid == guid)
    }

    fn collect_in_order_for(
        state: &mut WriterProxyState,
        best_effort: bool,
        budget: &mut Option<usize>,
    ) -> Vec<DeliveredSample> {
        // Typically 1 sample per recv in steady-state, occasionally a burst.
        // Pre-alloc with cap=2 eliminates the Vec::grow reallocs without
        // over-allocating on the single-sample path.
        let mut out = Vec::with_capacity(2);
        Self::collect_in_order_into(state, best_effort, budget, &mut out);
        out
    }

    /// [`Self::collect_in_order_for`] into `out`.
    fn collect_in_order_into(
        state: &mut WriterProxyState,
        best_effort: bool,
        budget: &mut Option<usize>,
        out: &mut Vec<DeliveredSample>,
    ) {
        loop {
            let next = SequenceNumber(state.delivered_up_to.0 + 1);
            if let Some(change) = state.received_cache.get(next) {
                // The consumer has no room: `next` stays in the receive cache,
                // received and acknowledged, and everything after it waits
                // behind it. See `set_release_budget`.
                if *budget == Some(0) {
                    break;
                }
                // Inline-QoS values follow the submessage endianness; the DATA
                // bodies this stack decodes are little-endian (see
                // `coherent_set_sn` in `on_data`).
                let related = change.inline_qos.as_ref().and_then(|pl| {
                    crate::inline_qos::find_related_sample_identity(pl, pl.little_endian)
                        .ok()
                        .flatten()
                });
                let eprosima_related = change.inline_qos.as_ref().and_then(|pl| {
                    crate::inline_qos::find_eprosima_related_sample_identity(pl, pl.little_endian)
                });
                let sample = DeliveredSample {
                    writer_guid: state.proxy.remote_writer_guid,
                    sequence_number: change.sequence_number,
                    payload: crate::payload::Payload::whole(change.payload.clone()),
                    kind: change.kind,
                    key_hash: change.key_hash,
                    source_timestamp: change.source_timestamp,
                    coherent_set_sn: change.coherent_set_sn,
                    coherent_set_end: change.coherent_set_end,
                    group: change.group,
                    related_sample_identity: related,
                    eprosima_related_sample_identity: eprosima_related,
                    coherent_set_complete: None,
                    original_writer: original_writer_of(change.inline_qos.as_ref()),
                    forwarded_by: None,
                    non_standard_payload: change.non_standard_payload,
                    inline_qos_override: crate::inline_qos::InlineQosOverride::of(
                        change.inline_qos.as_ref(),
                    ),
                };
                Self::hand_out(state, sample, out);
                state.delivered_up_to = next;
                state.delivered_any = true;
                state.received_cache.remove_up_to(next);
                if let Some(left) = budget.as_mut() {
                    *left -= 1;
                }
            } else if state.proxy.is_known(next) {
                // Received without a sample to hand out (a key-only
                // registration) or made irrelevant by a GAP, which
                // §8.4.10.4.3 records as RECEIVED. Either way `next` no
                // longer holds delivery up, announced by a HEARTBEAT or not.
                state.delivered_up_to = next;
                state.follow_coherent_set_past();
            } else if next < state.proxy.first_available_sn() {
                // Writer announced first_sn > next via HEARTBEAT
                // → samples before first_available are "lost" (volatile
                // skip, historic eviction). Advance the delivery pointer
                // so that subsequent SNs in the received_cache can finally
                // be delivered. Spec §8.4.12.4.
                state.delivered_up_to = next;
            } else if best_effort {
                // BEST_EFFORT reader (RTPS §8.4.12.1): never block on a missing
                // `next`. A best-effort reader makes no in-order-without-loss
                // guarantee and does not NACK to repair the gap, so waiting for
                // `next` (a sample lost in transit, or one the writer still lists
                // in its HEARTBEAT but we never received) would deadlock delivery
                // forever. Skip ahead to the lowest cached SN and deliver from
                // there. Cross-vendor: an OpenDDS/RTI writer whose first sample to
                // us is mid-stream lands here.
                match state.received_cache.min_sn() {
                    Some(low) if low.0 > next.0 => {
                        // What it steps over is lost, and NOT_AVAILABLE so
                        // that a HEARTBEAT does not count it again.
                        let lost = state.proxy.lost_changes_update(low);
                        state.note_lost(lost);
                        state.delivered_up_to = SequenceNumber(low.0 - 1);
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }
    }

    /// Hands `sample` out to `out`, the writer's coherent set followed
    /// across it: an end-of-coherent-set delivery says whether its set was
    /// complete, and a sample that ends an incomplete set otherwise gets
    /// one in front of it (see [`DeliveredSample::coherent_set_complete`]).
    fn hand_out(
        state: &mut WriterProxyState,
        mut sample: DeliveredSample,
        out: &mut Vec<DeliveredSample>,
    ) {
        let ended = state.follow_coherent_set(
            sample.sequence_number.0,
            sample.coherent_set_sn,
            sample.coherent_set_end,
            sample.group.is_some(),
        );
        if sample.coherent_set_end {
            sample.coherent_set_complete = ended;
        } else if ended == Some(false) {
            out.push(DeliveredSample::incomplete_set_end(
                state.proxy.remote_writer_guid,
                sample.sequence_number,
            ));
        }
        out.push(sample);
    }

    /// Stores a change its proxy has marked received in the receive cache,
    /// and returns whether it is there.
    ///
    /// A full KEEP_ALL cache gives up its highest change for one that comes
    /// before it, and that change is MISSING again. The cache then holds
    /// the changes closest to delivery: one full of changes behind a
    /// missing change still takes the missing one, which is what lets it
    /// drain. Refused, the missing change could never be stored while the
    /// changes waiting for it filled the cache, and nothing behind it was
    /// delivered again.
    ///
    /// A change that is not stored has its received mark taken back, or
    /// the reader would report an SN it does not hold and never ask for it
    /// again (see `WriterProxy::unreceive_change_set`). The same holds for
    /// the change it displaces.
    fn store_received(state: &mut WriterProxyState, change: CacheChange) -> bool {
        let sn = change.sequence_number;
        let cache = &mut state.received_cache;
        if cache.kind() == HistoryKind::KeepAll
            && cache.len() >= cache.capacity()
            && cache.max_sn().is_some_and(|highest| highest > sn)
        {
            if let Some(displaced) = cache.remove_highest() {
                state.proxy.unreceive_change_set(displaced.sequence_number);
            }
        }
        if state.received_cache.insert(change).is_err() {
            state.proxy.unreceive_change_set(sn);
            return false;
        }
        true
    }

    /// Whether the change after the last one delivered is in the receive
    /// cache, so that what the cache holds waits for the consumer and not
    /// for a change still missing in front of it.
    fn next_change_is_cached(state: &WriterProxyState) -> bool {
        state
            .received_cache
            .get(SequenceNumber(state.delivered_up_to.0 + 1))
            .is_some()
    }

    /// The receive cache is full of samples waiting for the consumer, and
    /// the writer has more the reader is missing: the case in which
    /// [`Self::tick_outbound`] holds the ACKNACK back.
    fn full_for_the_consumer(state: &WriterProxyState) -> bool {
        state.received_cache.len() >= state.received_cache.capacity()
            && Self::next_change_is_cached(state)
            && state.proxy.has_missing_changes()
    }

    fn build_nackfrag_datagram(
        &mut self,
        proxy_idx: usize,
        sn: SequenceNumber,
    ) -> Result<Vec<u8>, WireError> {
        let missing = self.writer_proxies[proxy_idx]
            .assembler
            .missing_fragments(sn);
        self.nackfrag_count = self.nackfrag_count.wrapping_add(1);
        let writer_guid = self.writer_proxies[proxy_idx].proxy.remote_writer_guid;
        let nf = NackFragSubmessage {
            reader_id: self.guid.entity_id,
            writer_id: writer_guid.entity_id,
            writer_sn: sn,
            fragment_number_state: missing,
            count: self.nackfrag_count,
        };
        let (body, mut flags) = nf.write_body(true);
        flags |= FLAG_E_LITTLE_ENDIAN;
        self.wrap_to_writer(writer_guid.prefix, SubmessageId::NackFrag, flags, &body)
    }

    fn build_acknack_datagram(&mut self, proxy_idx: usize) -> Result<Vec<u8>, WireError> {
        let state = &self.writer_proxies[proxy_idx];
        let base = state.proxy.acknack_base();
        // No more than the receive cache can hold (§8.4.2.3, see
        // `tick_outbound`) while what it holds waits for the consumer, and
        // never more than one SequenceNumberSet.
        let window = crate::submessages::RTPS_BITMAP_MAX_BITS as usize;
        let limit = if Self::next_change_is_cached(state) {
            state
                .received_cache
                .capacity()
                .saturating_sub(state.received_cache.len())
                .min(window)
        } else {
            window
        };
        let missing = state.proxy.missing_changes(limit);
        // §8.4.14.1: "a sequence number must be negatively acknowledged
        // only when all fragments are missing." A partly received sample
        // is asked for by NACK_FRAG, fragment by fragment.
        let partial: alloc::collections::BTreeSet<SequenceNumber> =
            state.assembler.incomplete_sns().collect();
        let requested: Vec<SequenceNumber> = missing
            .iter()
            .copied()
            .filter(|sn| !partial.contains(sn))
            .collect();
        let snset = SequenceNumberSet::from_missing(base, &requested);
        self.acknack_count = self.acknack_count.wrapping_add(1);
        // final_flag=true only if we really have everything up to base-1
        // and no further writer action is needed. For the preemptive
        // AckNack (base=1, empty bitmap, proxy has seen nothing yet)
        // final must be false, otherwise the writer reads it as "reader is
        // up-to-date" and sends no durability resends (Cyclone DDS
        // then shows only HEARTBEATs, no DATA). A reader that took a
        // HEARTBEAT from the writer knows what it has, also when that is
        // nothing (first 1, last 0): F clear would ask the writer for a
        // HEARTBEAT it already sent (DDSI-RTPS 2.5 §9.4.5.3), and each answer
        // for the next.
        let final_flag = missing.is_empty()
            && (state.proxy.last_available_sn().0 >= 1 || state.last_heartbeat_at.is_some());
        let writer_guid = state.proxy.remote_writer_guid;
        let ack = AckNackSubmessage {
            reader_id: self.guid.entity_id,
            writer_id: writer_guid.entity_id,
            reader_sn_state: snset,
            count: self.acknack_count,
            final_flag,
        };
        let (body, mut flags) = ack.write_body(true);
        flags |= FLAG_E_LITTLE_ENDIAN;
        self.wrap_to_writer(writer_guid.prefix, SubmessageId::AckNack, flags, &body)
    }

    /// Packs `Header + INFO_DST(writer_prefix) + Submessage` into a
    /// datagram. INFO_DST is mandatory: without it the effective
    /// destination prefix = UNKNOWN, and receivers (e.g. Cyclone DDS)
    /// discard the submessage as "not a connection" (RTPS 2.5 §8.3.7.6).
    fn wrap_to_writer(
        &self,
        writer_prefix: crate::wire_types::GuidPrefix,
        id: SubmessageId,
        flags: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, WireError> {
        let header = RtpsHeader::new(self.vendor_id, self.guid.prefix);
        let mut out = Vec::new();
        out.extend_from_slice(&header.to_bytes());

        // INFO_DST: target writer's GuidPrefix (12 byte body).
        let info_dst_header = SubmessageHeader {
            submessage_id: SubmessageId::InfoDst,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: 12,
        };
        out.extend_from_slice(&info_dst_header.to_bytes());
        out.extend_from_slice(&writer_prefix.to_bytes());

        // Eigentliche Submessage (ACKNACK / NACK_FRAG).
        let body_len = u16::try_from(body.len()).map_err(|_| WireError::ValueOutOfRange {
            message: "submessage body exceeds u16::MAX",
        })?;
        let sh = SubmessageHeader {
            submessage_id: id,
            flags,
            octets_to_next_header: body_len,
        };
        out.extend_from_slice(&sh.to_bytes());
        out.extend_from_slice(body);
        Ok(out)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::datagram::{ParsedSubmessage, decode_datagram};
    use crate::wire_types::{EntityId, GuidPrefix, Locator};

    fn single_writer_guid() -> Guid {
        Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
        )
    }

    fn make_reader(max_samples: usize) -> ReliableReader {
        let reader_guid = Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
        );
        let writer_proxy = WriterProxy::new(
            single_writer_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
            alloc::vec![],
            true,
        );
        ReliableReader::new(ReliableReaderConfig {
            guid: reader_guid,
            vendor_id: VendorId::ZERODDS,
            writer_proxies: alloc::vec![writer_proxy],
            max_samples_per_proxy: max_samples,
            heartbeat_response_delay: Duration::from_millis(200),
            assembler_caps: AssemblerCaps::default(),
        })
    }

    fn sn(n: i64) -> SequenceNumber {
        SequenceNumber(n)
    }

    /// source `guid_prefix` of the default writer proxy (single_writer_guid).
    fn p1() -> GuidPrefix {
        single_writer_guid().prefix
    }

    /// source `guid_prefix` of the second writer proxy (second_writer_guid).
    fn p2() -> GuidPrefix {
        second_writer_guid().prefix
    }

    fn data(wid: EntityId, rid: EntityId, n: i64, byte: u8) -> DataSubmessage {
        DataSubmessage {
            extra_flags: 0,
            reader_id: rid,
            writer_id: wid,
            writer_sn: sn(n),
            inline_qos: None,
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from(alloc::vec![byte]),
        }
    }

    fn heartbeat(
        wid: EntityId,
        rid: EntityId,
        first: i64,
        last: i64,
        count: i32,
        final_flag: bool,
    ) -> HeartbeatSubmessage {
        HeartbeatSubmessage {
            reader_id: rid,
            writer_id: wid,
            first_sn: sn(first),
            last_sn: sn(last),
            count,
            final_flag,
            liveliness_flag: false,
            group_info: None,
        }
    }

    fn first_state(r: &ReliableReader) -> &WriterProxyState {
        &r.writer_proxies()[0]
    }

    /// Measured on the Linux test host (ROS 2 service, Fast DDS 3.6.1 client, capture):
    /// the client's request writer matched this reader and wrote SN 1 before
    /// this side knew the writer; the DATA was dropped, and the first
    /// ACKNACK after the match requested nothing, so SN 1 came with the
    /// writer's next HEARTBEAT three seconds later. The SNs such a writer
    /// announced before the match are requested by the first ACKNACK.
    #[test]
    fn changes_a_writer_sent_before_the_match_are_requested_at_once() {
        let mut r = make_reader(10);
        let w = second_writer_guid();
        let rd = r.guid().entity_id;
        assert!(
            r.handle_data(w.prefix, &data(w.entity_id, rd, 1, 1), None)
                .is_empty()
        );
        assert!(
            r.handle_data(w.prefix, &data(w.entity_id, rd, 2, 2), None)
                .is_empty()
        );
        assert_eq!(r.unknown_src_count(), 2);
        r.add_writer_proxy(WriterProxy::new(
            w,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7422)],
            alloc::vec![],
            true,
        ));
        let state = r
            .writer_proxies()
            .iter()
            .find(|s| s.proxy.remote_writer_guid == w)
            .expect("the new proxy");
        assert_eq!(state.proxy.missing_changes(16), alloc::vec![sn(1), sn(2)]);
        // The reader's heartbeat_response_delay is 200 ms.
        let acks: Vec<_> = r
            .tick(Duration::from_millis(200))
            .unwrap()
            .iter()
            .flat_map(|dg| {
                decode_datagram(dg)
                    .unwrap()
                    .contents()
                    .filter_map(|s| match s {
                        ParsedSubmessage::AckNack(a) if a.writer_id == w.entity_id => {
                            Some(a.reader_sn_state.iter_set().collect::<Vec<_>>())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(acks, alloc::vec![alloc::vec![sn(1), sn(2)]]);
    }

    /// Only a bounded number of unmatched writers is remembered.
    #[test]
    fn unmatched_writers_are_remembered_up_to_a_bound() {
        let mut r = make_reader(10);
        let rd = r.guid().entity_id;
        for n in 0..(UNMATCHED_WRITERS_REMEMBERED + 5) {
            let key = u8::try_from(n).unwrap();
            let w = Guid::new(
                GuidPrefix::from_bytes([9; 12]),
                EntityId::user_writer_with_key([key, 0, 0]),
            );
            let _ = r.handle_data(w.prefix, &data(w.entity_id, rd, 1, 1), None);
        }
        assert_eq!(r.unmatched_highest.len(), UNMATCHED_WRITERS_REMEMBERED);
    }

    /// A reliable reader whose per-proxy receive cache is full must not lose
    /// the sample it could not store: the SN has to fall back to MISSING so
    /// the next ACKNACK asks for it again.
    ///
    /// Setup: SN 1 never arrives, so nothing can be delivered in order and
    /// every later sample stays in the cache. With a capacity of 3, SNs 2..4
    /// fill it and SN 5 finds it full. Before the rollback, SN 5 was marked
    /// received, dropped, and never requested again — a permanent, silent hole
    /// in a RELIABLE stream.
    #[test]
    fn sample_rejected_by_a_full_cache_stays_missing_for_the_acknack() {
        let mut r = make_reader(3);
        let w_eid = single_writer_guid().entity_id;
        let r_eid = r.guid().entity_id;
        // The proxy only knows which SNs exist once a HEARTBEAT has announced
        // the range; `missing_changes` reports nothing outside it.
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w_eid, r_eid, 1, 5, 1, false),
            Duration::from_millis(0),
        );
        // SN 1 is the gap that blocks in-order delivery.
        for n in 2..=4i64 {
            let out = r.handle_data(p1(), &data(w_eid, r_eid, n, n as u8), None);
            assert!(out.is_empty(), "SN {n} waits behind the missing SN 1");
        }
        assert_eq!(r.cache_full_count(), 0, "3 samples fit into a cache of 3");

        let out = r.handle_data(p1(), &data(w_eid, r_eid, 5, 5), None);
        assert!(out.is_empty());
        assert_eq!(r.cache_full_count(), 1, "SN 5 found the cache full");

        let missing = first_state(&r).proxy.missing_changes(16);
        assert!(
            missing.contains(&sn(1)),
            "the real gap is still missing: {missing:?}"
        );
        assert!(
            missing.contains(&sn(5)),
            "the sample that did not fit must be requested again: {missing:?}"
        );
        assert!(
            !first_state(&r).proxy.is_known(sn(5)),
            "SN 5 must not count as received without a sample behind it"
        );
    }

    /// A full receive cache takes the change that blocks it: SN 1 arrives
    /// at a cache of 3 that holds SNs 2..4 behind it, SN 4 makes room and
    /// is MISSING again, and SNs 1..3 are delivered. Refused, SN 1 could
    /// never arrive while the cache held what waits for it.
    #[test]
    fn a_full_cache_takes_the_change_in_front_of_what_it_holds() {
        let mut r = make_reader(3);
        let w_eid = single_writer_guid().entity_id;
        let r_eid = r.guid().entity_id;
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w_eid, r_eid, 1, 4, 1, false),
            Duration::from_millis(0),
        );
        for n in 2..=4i64 {
            let out = r.handle_data(p1(), &data(w_eid, r_eid, n, n as u8), None);
            assert!(out.is_empty(), "SN {n} waits behind the missing SN 1");
        }
        let out = r.handle_data(p1(), &data(w_eid, r_eid, 1, 1), None);
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            [sn(1), sn(2), sn(3)]
        );
        assert_eq!(r.cache_full_count(), 0, "SN 1 was stored");
        assert!(
            !first_state(&r).proxy.is_known(sn(4)),
            "the displaced SN 4 does not count as received"
        );
        assert_eq!(first_state(&r).proxy.missing_changes(16), [sn(4)]);
        let out = r.handle_data(p1(), &data(w_eid, r_eid, 4, 4), None);
        assert_eq!(out.len(), 1, "the repaired SN 4 follows");
    }

    /// A writer that leaves while its samples wait for the consumer's room
    /// leaves them with the reader: they were acknowledged, and the writer
    /// may have discarded them. They are handed out in order once there is
    /// room, and the writer is named as done after the last one.
    #[test]
    fn a_departed_writer_leaves_its_held_samples_with_the_reader() {
        let mut r = make_reader(4);
        let w = single_writer_guid();
        let rd = r.guid().entity_id;
        r.set_release_budget(Some(0));
        for n in 1..=3i64 {
            let out = r.handle_data(p1(), &data(w.entity_id, rd, n, n as u8), None);
            assert!(out.is_empty(), "SN {n} waits for room");
        }
        assert!(r.depart_writer_proxy(w).is_some());
        assert_eq!(r.writer_proxy_count(), 0, "the proxy is gone");
        assert!(r.holds_departed(w));
        assert!(r.holds_releasable());
        assert_eq!(r.take_lost(), 0, "nothing was missing");

        r.set_release_budget(Some(2));
        let out = r.release_held();
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            [sn(1), sn(2)]
        );
        assert!(r.take_departed_done().is_empty(), "SN 3 is still held");
        r.set_release_budget(Some(2));
        let out = r.release_held();
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            [sn(3)]
        );
        assert_eq!(r.take_departed_done(), [w]);
        assert!(!r.holds_departed(w));
        assert!(!r.holds_releasable());
    }

    /// A change missing in front of a held sample cannot arrive once its
    /// writer has left: it is counted as lost (DDS 1.4 §2.2.4.1
    /// SAMPLE_LOST) and delivery steps past it.
    #[test]
    fn a_departed_writer_counts_what_is_missing_in_front_of_its_held_samples() {
        let mut r = make_reader(4);
        let w = single_writer_guid();
        let rd = r.guid().entity_id;
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w.entity_id, rd, 1, 5, 1, false),
            Duration::from_millis(0),
        );
        for n in [2i64, 4] {
            let out = r.handle_data(p1(), &data(w.entity_id, rd, n, n as u8), None);
            assert!(out.is_empty(), "SN {n} waits behind SN 1");
        }
        assert!(r.depart_writer_proxy(w).is_some());
        assert_eq!(
            r.take_lost(),
            2,
            "SN 1 and SN 3; SN 5 is behind every held sample"
        );
        let out = r.release_held();
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            [sn(2), sn(4)]
        );
        assert_eq!(r.take_departed_done(), [w]);
    }

    /// DDS 1.4 §2.2.4.1 SAMPLE_LOST: what a BEST_EFFORT reader steps over
    /// never arrives, and is counted; what came before the first sample it
    /// took was never sent to it, and is not.
    #[test]
    fn a_best_effort_reader_counts_the_samples_a_gap_lost() {
        let mut r = make_reader(4);
        r.set_best_effort(true);
        let w = single_writer_guid();
        let rd = r.guid().entity_id;
        let deliver = |r: &mut ReliableReader, n: i64| {
            r.handle_data(p1(), &data(w.entity_id, rd, n, n as u8), None)
                .iter()
                .map(|s| s.sequence_number.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(deliver(&mut r, 5), [5], "a late joiner starts at SN 5");
        assert_eq!(r.take_lost(), 0, "SN 1 to 4 were not this reader's to lose");
        assert_eq!(deliver(&mut r, 6), [6]);
        assert_eq!(deliver(&mut r, 9), [9]);
        assert_eq!(r.take_lost(), 2, "SN 7 and 8");
        assert_eq!(r.take_lost(), 0, "counted once");
        assert_eq!(deliver(&mut r, 8), Vec::<i64>::new(), "too late");
        assert_eq!(r.take_lost(), 0);
    }

    /// A RELIABLE reader loses what its writer dropped before it arrived:
    /// the HEARTBEAT's firstSN passed it. Before the first sample is taken,
    /// a firstSN past what the reader has is a late join.
    #[test]
    fn a_reliable_reader_counts_the_changes_its_writer_dropped() {
        let mut r = make_reader(16);
        let w = single_writer_guid();
        let rd = r.guid().entity_id;
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w.entity_id, rd, 3, 4, 1, false),
            Duration::from_millis(0),
        );
        assert_eq!(r.take_lost(), 0, "SN 1 and 2 left before this reader came");
        for n in [3i64, 4] {
            let out = r.handle_data(p1(), &data(w.entity_id, rd, n, n as u8), None);
            assert_eq!(out.len(), 1, "SN {n}");
        }
        let out = r.handle_data(p1(), &data(w.entity_id, rd, 6, 6), None);
        assert!(out.is_empty(), "SN 6 waits for SN 5");
        let out = r.handle_heartbeat(
            p1(),
            &heartbeat(w.entity_id, rd, 6, 8, 2, false),
            Duration::from_secs(1),
        );
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            [sn(6)],
            "the writer no longer has SN 5"
        );
        assert_eq!(r.take_lost(), 1, "SN 5");
    }

    /// A writer that leaves with nothing held is not kept.
    #[test]
    fn a_departed_writer_with_nothing_held_is_not_kept() {
        let mut r = make_reader(4);
        let w = single_writer_guid();
        let rd = r.guid().entity_id;
        let out = r.handle_data(p1(), &data(w.entity_id, rd, 1, 1), None);
        assert_eq!(out.len(), 1);
        assert!(r.depart_writer_proxy(w).is_some());
        assert!(!r.holds_departed(w));
        assert!(!r.holds_releasable());
        assert!(
            r.depart_writer_proxy(w).is_none(),
            "the proxy is already gone"
        );
    }

    /// A consumer without room holds samples back in the receive cache, not
    /// in an unbounded queue: what fits there is acknowledged and released in
    /// order once there is room, and what does not fit stays MISSING, so the
    /// writer keeps it (DDS 1.4 §2.2.3.14).
    #[test]
    fn a_spent_release_budget_holds_samples_back_in_order() {
        let mut r = make_reader(2);
        let w_eid = single_writer_guid().entity_id;
        let r_eid = r.guid().entity_id;
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w_eid, r_eid, 1, 4, 1, false),
            Duration::from_millis(0),
        );
        r.set_release_budget(Some(1));
        let out = r.handle_data(p1(), &data(w_eid, r_eid, 1, 1), None);
        assert_eq!(out.len(), 1, "SN 1 fits the budget");
        for n in 2..=3i64 {
            let out = r.handle_data(p1(), &data(w_eid, r_eid, n, n as u8), None);
            assert!(out.is_empty(), "SN {n} waits for room");
        }
        assert!(r.holds_releasable());
        assert!(first_state(&r).proxy.is_known(sn(2)));
        assert!(first_state(&r).proxy.is_known(sn(3)));
        let out = r.handle_data(p1(), &data(w_eid, r_eid, 4, 4), None);
        assert!(out.is_empty());
        assert_eq!(r.cache_full_count(), 1, "SN 4 found the receive cache full");
        assert!(
            first_state(&r).proxy.missing_changes(16).contains(&sn(4)),
            "SN 4 is not acknowledged"
        );

        assert!(r.release_held().is_empty(), "the budget is still spent");
        r.set_release_budget(Some(1));
        let out = r.release_held();
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            [sn(2)]
        );
        r.set_release_budget(None);
        let out = r.release_held();
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            [sn(3)]
        );
        assert!(!r.holds_releasable());
        let out = r.handle_data(p1(), &data(w_eid, r_eid, 4, 4), None);
        assert_eq!(out.len(), 1, "the repaired SN 4 follows");
    }

    /// The rollback must not fire on the normal path: a sample that fits is
    /// received, stays known, and is not counted.
    #[test]
    fn sample_that_fits_stays_received_and_is_not_counted() {
        let mut r = make_reader(8);
        let w_eid = single_writer_guid().entity_id;
        let r_eid = r.guid().entity_id;
        let out = r.handle_data(p1(), &data(w_eid, r_eid, 1, 1), None);
        assert_eq!(out.len(), 1, "SN 1 is in order and is delivered");
        assert_eq!(r.cache_full_count(), 0);
        assert!(first_state(&r).proxy.is_known(sn(1)));
    }

    // ---- DDSI-RTPS 2.5 §8.7.6 Coherent Sets, reader side ----

    /// §8.7.6: the reader reads `PID_COHERENT_SET` off the inline QoS and
    /// carries the set-first `SequenceNumber` on the delivered sample, so the
    /// DCPS layer can tell the members of one set apart.
    #[test]
    fn delivered_sample_carries_the_coherent_set_marker() {
        let mut r = make_reader(10);
        let w_eid = single_writer_guid().entity_id;
        let r_eid = r.guid().entity_id;
        let mut d = data(w_eid, r_eid, 1, 0xAA);
        d.inline_qos = Some(crate::inline_qos::coherent_set_inline_qos(1, true));
        let out = r.handle_data(p1(), &d, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].coherent_set_sn, Some(1));
        assert!(!out[0].coherent_set_end);
    }

    /// §8.7.6: a DATA without the PID names no set.
    #[test]
    fn delivered_sample_without_marker_has_no_coherent_set() {
        let mut r = make_reader(10);
        let w_eid = single_writer_guid().entity_id;
        let r_eid = r.guid().entity_id;
        let out = r.handle_data(p1(), &data(w_eid, r_eid, 1, 0xAA), None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].coherent_set_sn, None);
        assert!(!out[0].coherent_set_end);
    }

    /// §8.7.6: the end-of-coherent-set sample is a DATA with the D and the K
    /// flag clear. The reader delivers it as the set's terminator, not as a
    /// sample with an empty payload — the DCPS layer releases the buffered
    /// set on it. Each form ends the set: Table 9.22 Example 2
    /// (`SEQUENCENUMBER_UNKNOWN`, no set named) and Example 3 (no inline
    /// QoS), the set's last SN (HDDS 1.4.8) and the set's first SN (this
    /// stack before Table 9.22, and Table 9.23 in a GROUP set).
    #[test]
    fn end_of_coherent_set_data_is_delivered_as_a_terminator() {
        let unknown = crate::wire_types::SequenceNumber::UNKNOWN.0;
        for (form, value, named) in [
            ("Example 2", Some(unknown), None),
            ("Example 3", None, None),
            ("last SN", Some(2), Some(2)),
            ("first SN", Some(1), Some(1)),
        ] {
            let mut r = make_reader(10);
            let w_eid = single_writer_guid().entity_id;
            let r_eid = r.guid().entity_id;
            for n in 1..=2 {
                let mut member = data(w_eid, r_eid, n, 0xAA);
                member.inline_qos = Some(crate::inline_qos::coherent_set_inline_qos(1, true));
                let _ = r.handle_data(p1(), &member, None);
            }

            let ecs = DataSubmessage {
                extra_flags: 0,
                reader_id: r_eid,
                writer_id: w_eid,
                writer_sn: sn(3),
                inline_qos: value.map(|v| crate::inline_qos::coherent_set_inline_qos(v, true)),
                data_flag: false,
                key_flag: false,
                non_standard_flag: false,
                serialized_payload: crate::payload::Payload::from(alloc::vec::Vec::new()),
            };
            let out = r.handle_data(p1(), &ecs, None);
            assert_eq!(out.len(), 1, "{form}");
            assert!(out[0].coherent_set_end, "{form}: D=0, K=0 ends the set");
            assert_eq!(out[0].coherent_set_sn, named, "{form}");
            assert!(out[0].payload.is_empty(), "{form}");
        }
    }

    /// DDSI-RTPS 2.5 §9.6.4.2 Table 9.22: a set of the samples 3 to 6
    /// (PID_COHERENT_SET 3) ends with each of the three DATA the table
    /// gives: Example 1, sample 7 with data that opens a new set (PID 7);
    /// Example 2, no data and PID_COHERENT_SET SEQUENCENUMBER_UNKNOWN;
    /// Example 3, no data and no in-line QoS. Every one ends the set 3, which
    /// arrived whole.
    #[test]
    fn table_9_22_each_form_that_ends_a_coherent_set_ends_it() {
        let unknown = crate::wire_types::SequenceNumber::UNKNOWN.0;
        for (form, ecs_pid, data_flag) in [
            ("Example 1", Some(7), true),
            ("Example 2", Some(unknown), false),
            ("Example 3", None, false),
        ] {
            let mut r = make_reader(20);
            let w = single_writer_guid().entity_id;
            let rd = r.guid().entity_id;
            for n in 1..=2 {
                let _ = r.handle_data(p1(), &data(w, rd, n, 0x11), None);
            }
            for n in 3..=6 {
                let _ = r.handle_data(p1(), &member(&r, n, Some(3)), None);
            }
            let mut seventh = data(w, rd, 7, 0xAA);
            seventh.data_flag = data_flag;
            if !data_flag {
                seventh.serialized_payload = crate::payload::Payload::from(alloc::vec::Vec::new());
            }
            seventh.inline_qos =
                ecs_pid.map(|v| crate::inline_qos::coherent_set_inline_qos(v, true));
            let out = r.handle_data(p1(), &seventh, None);
            assert_eq!(out.len(), 1, "{form}");
            assert_eq!(out[0].sequence_number, sn(7), "{form}");
            if data_flag {
                // A sample of the new set 7: the old one is complete, and the
                // sample is the first of the next.
                assert!(!out[0].coherent_set_end, "{form}");
                assert_eq!(out[0].coherent_set_sn, Some(7), "{form}");
            } else {
                assert!(out[0].coherent_set_end, "{form}");
                assert_eq!(out[0].coherent_set_complete, Some(true), "{form}");
            }
        }
    }

    /// DDSI-RTPS 2.5 §9.6.4.3 Table 9.23: the DATA of two writers in a group
    /// coherent set carry PID_GROUP_SEQ_NUM, PID_COHERENT_SET,
    /// PID_GROUP_COHERENT_SET and (end samples) PID_WRITER_GROUP_INFO as the
    /// table gives them; the reader hands out the group numbers of each.
    #[test]
    fn table_9_23_the_data_of_a_group_coherent_set_reach_the_consumer_with_their_group_numbers() {
        let mut r = make_reader(20);
        r.set_best_effort(true);
        add_second_writer(&mut r);
        let w1 = single_writer_guid();
        let w2 = second_writer_guid();
        let rd = r.guid().entity_id;
        let set = crate::group_digest::GroupDigest::of_entities(&[w1.entity_id, w2.entity_id]);
        // (writer, writerSN, DataFlag, GROUP_SEQ_NUM, COHERENT_SET)
        let rows = [
            (w1, 4, true, 11, 4),
            (w2, 8, true, 12, 8),
            (w1, 5, true, 13, 4),
            (w1, 6, false, 14, 4),
            (w2, 9, false, 14, 8),
        ];
        for (w, writer_sn, data_flag, gsn, cs) in rows {
            let mut d = data(w.entity_id, rd, writer_sn, 0x01);
            d.data_flag = data_flag;
            if !data_flag {
                d.serialized_payload = crate::payload::Payload::from(alloc::vec::Vec::new());
            }
            let mut pl = crate::inline_qos::coherent_set_inline_qos(cs, true);
            pl.push(crate::inline_qos::group_seq_num_param(gsn, true));
            pl.push(crate::inline_qos::group_coherent_set_param(11, true));
            pl.push(crate::inline_qos::writer_group_info_param(set));
            d.inline_qos = Some(pl);
            let out = r.handle_data(w.prefix, &d, None);
            assert_eq!(out.len(), 1, "writerSN {writer_sn}");
            assert_eq!(out[0].coherent_set_sn, Some(cs), "writerSN {writer_sn}");
            assert_eq!(
                out[0].group,
                Some(crate::writer_group::ChangeGroupInfo {
                    gsn,
                    writer_set: set,
                    coherent_set_gsn: Some(11),
                }),
                "writerSN {writer_sn}"
            );
            assert_eq!(out[0].coherent_set_end, !data_flag, "writerSN {writer_sn}");
        }
    }

    /// A member of the set starting at `first`, or a DATA outside any set.
    fn member(r: &ReliableReader, n: i64, first: Option<i64>) -> DataSubmessage {
        let mut d = data(single_writer_guid().entity_id, r.guid().entity_id, n, 0xAA);
        d.inline_qos = first.map(|f| crate::inline_qos::coherent_set_inline_qos(f, true));
        d
    }

    /// The end-of-coherent-set DATA at `n` (Table 9.22, Example 2).
    fn set_end(r: &ReliableReader, n: i64) -> DataSubmessage {
        let mut d = member(r, n, Some(crate::wire_types::SequenceNumber::UNKNOWN.0));
        d.data_flag = false;
        d.serialized_payload = crate::payload::Payload::from(alloc::vec::Vec::new());
        d
    }

    /// `(SN, end-of-coherent-set, coherent_set_complete)` of what was
    /// handed out.
    fn ends(out: &[DeliveredSample]) -> Vec<(i64, bool, Option<bool>)> {
        out.iter()
            .map(|s| {
                (
                    s.sequence_number.0,
                    s.coherent_set_end,
                    s.coherent_set_complete,
                )
            })
            .collect()
    }

    /// §8.7.6: every change of the set from its first up to its end
    /// arrived; the end says so.
    #[test]
    fn a_set_that_arrived_whole_ends_complete() {
        let mut r = make_reader(10);
        let _ = r.handle_data(p1(), &member(&r, 1, Some(1)), None);
        let _ = r.handle_data(p1(), &member(&r, 2, Some(1)), None);
        let out = r.handle_data(p1(), &set_end(&r, 3), None);
        assert_eq!(ends(&out), [(3, true, Some(true))]);
    }

    /// A change of the set a GAP ruled irrelevant is not one the reader
    /// needs (§8.7.6 removes filtered changes from the relevant set).
    #[test]
    fn a_change_a_gap_ruled_out_keeps_the_set_complete() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let _ = r.handle_data(p1(), &member(&r, 1, Some(1)), None);
        let _ = r.handle_gap(p1(), &gap(w, rd, 2, 3));
        let _ = r.handle_data(p1(), &member(&r, 3, Some(1)), None);
        let out = r.handle_data(p1(), &set_end(&r, 4), None);
        assert_eq!(ends(&out), [(4, true, Some(true))]);
    }

    /// A reader that joins inside a set has its tail only: the set's first
    /// change was written before it, and the set is incomplete. A
    /// BEST_EFFORT reader that loses a member is in the same place.
    #[test]
    fn a_set_short_of_a_change_ends_incomplete() {
        let mut r = make_reader(10);
        r.set_best_effort(true);
        let _ = r.handle_data(p1(), &member(&r, 3, Some(2)), None);
        let out = r.handle_data(p1(), &set_end(&r, 4), None);
        assert_eq!(
            ends(&out),
            [(4, true, Some(false))],
            "joined inside the set"
        );

        let _ = r.handle_data(p1(), &member(&r, 5, Some(5)), None);
        let _ = r.handle_data(p1(), &member(&r, 7, Some(5)), None);
        let out = r.handle_data(p1(), &set_end(&r, 8), None);
        assert_eq!(ends(&out), [(8, true, Some(false))], "SN 6 lost");
    }

    /// A set that ends on a sample — one outside any set here — and was
    /// incomplete gets an end-of-coherent-set delivery in front of that
    /// sample; a complete one needs none.
    #[test]
    fn a_set_ended_by_a_sample_is_reported_in_front_of_it_when_incomplete() {
        let mut r = make_reader(10);
        r.set_best_effort(true);
        let _ = r.handle_data(p1(), &member(&r, 1, Some(1)), None);
        let out = r.handle_data(p1(), &member(&r, 2, None), None);
        assert_eq!(ends(&out), [(2, false, None)], "complete, nothing in front");

        // SN 4 is lost: it may have been set 3's or its end, so set 3 is
        // not known complete.
        let _ = r.handle_data(p1(), &member(&r, 3, Some(3)), None);
        let out = r.handle_data(p1(), &member(&r, 5, Some(5)), None);
        assert_eq!(
            ends(&out),
            [(5, true, Some(false)), (5, false, None)],
            "SN 4 lost"
        );
        let _ = r.handle_data(p1(), &member(&r, 6, Some(5)), None);
        let out = r.handle_data(p1(), &member(&r, 7, Some(7)), None);
        assert_eq!(ends(&out), [(7, false, None)], "set 5 complete");
    }

    /// A GROUP-scope publisher's set is left to the subscriber's group
    /// order.
    #[test]
    fn a_group_set_is_not_followed() {
        let mut r = make_reader(10);
        r.set_best_effort(true);
        let mut d = member(&r, 3, Some(2));
        let mut pl = d.inline_qos.take().unwrap_or_default();
        pl.push(crate::inline_qos::group_seq_num_param(9, true));
        d.inline_qos = Some(pl);
        let _ = r.handle_data(p1(), &d, None);
        let mut e = set_end(&r, 4);
        let mut pl = e.inline_qos.take().unwrap_or_default();
        pl.push(crate::inline_qos::group_seq_num_param(10, true));
        e.inline_qos = Some(pl);
        let out = r.handle_data(p1(), &e, None);
        assert_eq!(ends(&out), [(4, true, None)]);
    }

    /// §8.4.14.1: "a sequence number must be negatively acknowledged only
    /// when all fragments are missing." SN 1 has its first fragment of
    /// two, SN 2 nothing: the ACKNACK asks for SN 2, a NACK_FRAG for SN 1's
    /// second fragment.
    #[test]
    fn a_partly_received_sample_is_asked_for_by_nack_frag_not_acknack() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let df = crate::submessages::DataFragSubmessage {
            inline_qos: None,
            extra_flags: 0,
            reader_id: rd,
            writer_id: w,
            writer_sn: sn(1),
            fragment_starting_num: crate::wire_types::FragmentNumber(1),
            fragments_in_submessage: 1,
            fragment_size: 4,
            sample_size: 8,
            serialized_payload: alloc::sync::Arc::from(alloc::vec![1u8, 2, 3, 4].as_slice()),
            inline_qos_flag: false,
            key_flag: false,
            non_standard_flag: false,
        };
        let _ = r.handle_data_frag(p1(), &df, Duration::ZERO, None);
        let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 2, 1, false), Duration::ZERO);
        let mut acked = alloc::vec::Vec::new();
        let mut frags = alloc::vec::Vec::new();
        for dg in r.tick_outbound(Duration::from_secs(10)).unwrap() {
            for sub in crate::datagram::decode_datagram(&dg.bytes)
                .unwrap()
                .contents()
            {
                match sub {
                    crate::datagram::ParsedSubmessage::AckNack(a) => {
                        acked.extend(a.reader_sn_state.iter_set().map(|s| s.0));
                    }
                    crate::datagram::ParsedSubmessage::NackFrag(n) => {
                        frags.extend(
                            n.fragment_number_state
                                .iter_set()
                                .map(|f| (n.writer_sn.0, f.0)),
                        );
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(
            acked,
            [2],
            "the ACKNACK asks for the sample with no fragment"
        );
        assert_eq!(frags, [(1, 2)], "the NACK_FRAG for the missing fragment");
    }

    /// A DATA_FRAG of two fragments, the first with `inline_qos`.
    fn two_fragment_sample(
        w: EntityId,
        rd: EntityId,
        n: i64,
        inline_qos: Option<crate::parameter_list::ParameterList>,
        key_flag: bool,
    ) -> [crate::submessages::DataFragSubmessage; 2] {
        let part =
            |starting: u32, bytes: &[u8], qos: Option<crate::parameter_list::ParameterList>| {
                crate::submessages::DataFragSubmessage {
                    extra_flags: 0,
                    reader_id: rd,
                    writer_id: w,
                    writer_sn: sn(n),
                    fragment_starting_num: crate::wire_types::FragmentNumber(starting),
                    fragments_in_submessage: 1,
                    fragment_size: 4,
                    sample_size: 8,
                    serialized_payload: alloc::sync::Arc::from(bytes),
                    inline_qos_flag: qos.is_some(),
                    inline_qos: qos,
                    key_flag,
                    non_standard_flag: false,
                }
            };
        [
            part(1, &[1, 2, 3, 4], inline_qos),
            part(2, &[5, 6, 7, 8], None),
        ]
    }

    /// §8.3.8.3 and §9.6.4.9: the interpretation of a re-assembled sample is
    /// that of a DATA. F = 1 with D = U = 0 in the first fragment's in-line
    /// QoS is a sample that did not pass the reader's filter: it is known (the
    /// next sample is in order), not delivered, counted, and NA_FILTERED.
    #[test]
    fn a_reassembled_sample_with_the_filtered_flag_is_known_but_not_delivered() {
        use crate::writer_proxy::ChangeFromWriterStatus as S;
        let mut r = make_reader(10);
        let w = single_writer_guid();
        let rd = r.guid().entity_id;
        let mut qos = crate::parameter_list::ParameterList::new();
        qos.push(crate::inline_qos::status_info_param(
            crate::inline_qos::status_info::FILTERED,
        ));
        let [a, b] = two_fragment_sample(w.entity_id, rd, 1, Some(qos), false);
        assert!(
            r.handle_data_frag(p1(), &a, Duration::ZERO, None)
                .is_empty()
        );
        assert!(
            r.handle_data_frag(p1(), &b, Duration::ZERO, None)
                .is_empty()
        );
        assert_eq!(r.filtered_sample_count(), 1);
        assert_eq!(
            r.change_status(w, sn(1)),
            Some(S::NotAvailable(NotAvailableKind::Filtered))
        );
        let out = r.handle_data(p1(), &data(w.entity_id, rd, 2, 0x22), None);
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            alloc::vec![sn(2)]
        );
        // The same sample without the flag is delivered whole.
        let [a, b] = two_fragment_sample(w.entity_id, rd, 3, None, false);
        assert!(
            r.handle_data_frag(p1(), &a, Duration::ZERO, None)
                .is_empty()
        );
        let out = r.handle_data_frag(p1(), &b, Duration::ZERO, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].payload.as_ref(), &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(out[0].kind, ChangeKind::Alive);
    }

    /// §8.3.8.3: a re-assembled serialized key with PID_STATUS_INFO is the
    /// dispose it names, and one without status is the key-only registration
    /// a DATA with the key flag would be: absorbed, not delivered.
    #[test]
    fn a_reassembled_key_is_a_dispose_or_a_registration_as_a_data_is() {
        let mut r = make_reader(10);
        let w = single_writer_guid();
        let rd = r.guid().entity_id;
        let mut qos = crate::parameter_list::ParameterList::new();
        qos.push(crate::inline_qos::status_info_param(
            crate::inline_qos::status_info::DISPOSED,
        ));
        let [a, b] = two_fragment_sample(w.entity_id, rd, 1, Some(qos), true);
        assert!(
            r.handle_data_frag(p1(), &a, Duration::ZERO, None)
                .is_empty()
        );
        let out = r.handle_data_frag(p1(), &b, Duration::ZERO, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, ChangeKind::NotAliveDisposed);
        let [a, b] = two_fragment_sample(w.entity_id, rd, 2, None, true);
        assert!(
            r.handle_data_frag(p1(), &a, Duration::ZERO, None)
                .is_empty()
        );
        assert!(
            r.handle_data_frag(p1(), &b, Duration::ZERO, None)
                .is_empty()
        );
        let out = r.handle_data(p1(), &data(w.entity_id, rd, 3, 0x33), None);
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            alloc::vec![sn(3)],
            "the registration is known, so SN 3 is in order"
        );
    }

    /// DDSI-RTPS 2.5 Table 8.77: a HEARTBEAT with the LivelinessFlag is a
    /// HEARTBEAT — T7 takes its firstSN and lastSN, and with the FinalFlag
    /// clear T2 makes the reader answer; with both flags set it waits.
    #[test]
    fn a_heartbeat_with_the_liveliness_flag_is_taken_as_a_heartbeat() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let _ = r.tick_outbound(Duration::ZERO).unwrap();
        let mut hb = heartbeat(w, rd, 1, 3, 1, true);
        hb.liveliness_flag = true;
        let _ = r.handle_heartbeat(p1(), &hb, Duration::from_secs(1));
        assert_eq!(first_state(&r).proxy.last_available_sn(), sn(3), "T7");
        assert!(
            first_state(&r).pending_acknack_since.is_none(),
            "F and L set: waiting"
        );
        hb.final_flag = false;
        hb.count = 2;
        let _ = r.handle_heartbeat(p1(), &hb, Duration::from_secs(2));
        assert!(
            first_state(&r).pending_acknack_since.is_some(),
            "F clear: must_send_ack"
        );
    }

    /// §8.4.2.3: "A best-effort Reader is completely passive as it only
    /// receives data and does not send messages itself." Neither the
    /// writer's arrival nor a HEARTBEAT with F clear and samples missing
    /// makes it send an ACKNACK; the same reader reliable does.
    #[test]
    fn a_best_effort_reader_sends_no_acknack() {
        for best_effort in [true, false] {
            let mut r = make_reader(10);
            r.set_best_effort(best_effort);
            let w = single_writer_guid().entity_id;
            let rd = r.guid().entity_id;
            // A writer that arrives: §8.4.2.3 lets a reliable reader answer
            // it at once. Past the heartbeatResponseDelay of `make_reader`.
            r.add_writer_proxy(WriterProxy::new(
                second_writer_guid(),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7421)],
                alloc::vec![],
                true,
            ));
            let on_arrival = r.tick_outbound(Duration::from_secs(1)).unwrap();
            let hb = heartbeat(w, rd, 1, 3, 1, false);
            let _ = r.handle_heartbeat(p1(), &hb, Duration::from_secs(2));
            let after_hb = r.tick_outbound(Duration::from_secs(3)).unwrap();
            if best_effort {
                assert!(on_arrival.is_empty() && after_hb.is_empty());
            } else {
                assert_eq!(on_arrival.len(), 1, "the ACKNACK to the new writer");
                assert_eq!(after_hb.len(), 1, "the ACKNACK the HEARTBEAT asked for");
            }
        }
    }

    /// §8.3.8.7: a HEARTBEAT_FRAG asks for a NACK_FRAG when fragments up
    /// to its lastFragmentNum are missing, and for nothing otherwise.
    #[test]
    fn a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let _ = r.tick_outbound(Duration::ZERO).unwrap();
        // Fragment 2 of 3 of SN 1 arrived.
        let df = crate::submessages::DataFragSubmessage {
            inline_qos: None,
            extra_flags: 0,
            reader_id: rd,
            writer_id: w,
            writer_sn: sn(1),
            fragment_starting_num: crate::wire_types::FragmentNumber(2),
            fragments_in_submessage: 1,
            fragment_size: 4,
            sample_size: 12,
            serialized_payload: alloc::sync::Arc::from(alloc::vec![1u8, 2, 3, 4].as_slice()),
            inline_qos_flag: false,
            key_flag: false,
            non_standard_flag: false,
        };
        let _ = r.handle_data_frag(p1(), &df, Duration::ZERO, None);
        let _ = r.tick_outbound(Duration::from_secs(1)).unwrap();
        let hbf = |last: u32| crate::submessages::HeartbeatFragSubmessage {
            reader_id: rd,
            writer_id: w,
            writer_sn: sn(1),
            last_fragment_num: crate::wire_types::FragmentNumber(last),
            count: 1,
        };
        assert!(r.handle_heartbeat_frag(p1(), &hbf(1), Duration::from_secs(2)));
        let out = r.tick_outbound(Duration::from_secs(3)).unwrap();
        let nack_frags = out
            .iter()
            .flat_map(|dg| {
                crate::datagram::decode_datagram(&dg.bytes)
                    .unwrap()
                    .submessages
            })
            .filter(|f| matches!(f.submessage, crate::datagram::ParsedSubmessage::NackFrag(_)))
            .count();
        assert_eq!(nack_frags, 1, "fragment 1 is available and missing");
        // Unknown SN: nothing of it is held, nothing to answer.
        let mut other = hbf(3);
        other.writer_sn = sn(7);
        assert!(!r.handle_heartbeat_frag(p1(), &other, Duration::from_secs(4)));
    }

    /// DDSI-RTPS 2.5 §8.4.2.3: "In steady state, an ACKNACK Message can only
    /// be sent as a response to a HEARTBEAT Message from a Writer. ACKNACK
    /// Messages can be sent from a Reader when it first discovers a Writer as
    /// an optimization."; §8.4.14.1: "A NackFrag Submessage can only be sent in
    /// response to a Heartbeat or HeartbeatFrag submessage." Neither a
    /// fragment that leaves a hole nor a renewed announcement of a known
    /// writer makes the reader send; the HEARTBEAT that follows does.
    #[test]
    fn acknack_and_nack_frag_wait_for_a_heartbeat() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        // The first discovery of a writer: the allowed ACKNACK.
        r.add_writer_proxy(WriterProxy::new(
            second_writer_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7421)],
            alloc::vec![],
            true,
        ));
        assert_eq!(r.tick_outbound(Duration::from_secs(1)).unwrap().len(), 1);
        let df = crate::submessages::DataFragSubmessage {
            inline_qos: None,
            extra_flags: 0,
            reader_id: rd,
            writer_id: w,
            writer_sn: sn(1),
            fragment_starting_num: crate::wire_types::FragmentNumber(2),
            fragments_in_submessage: 1,
            fragment_size: 4,
            sample_size: 12,
            serialized_payload: alloc::sync::Arc::from(alloc::vec![1u8, 2, 3, 4].as_slice()),
            inline_qos_flag: false,
            key_flag: false,
            non_standard_flag: false,
        };
        let _ = r.handle_data_frag(p1(), &df, Duration::from_secs(2), None);
        r.add_writer_proxy(WriterProxy::new(
            single_writer_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
            alloc::vec![],
            true,
        ));
        assert!(
            r.tick_outbound(Duration::from_secs(10)).unwrap().is_empty(),
            "fragment 1 and 3 of SN 1 are missing, and no HEARTBEAT has asked"
        );
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w, rd, 1, 1, 1, false),
            Duration::from_secs(11),
        );
        let out = r.tick_outbound(Duration::from_secs(12)).unwrap();
        let kinds: alloc::vec::Vec<_> = out
            .iter()
            .flat_map(|dg| {
                crate::datagram::decode_datagram(&dg.bytes)
                    .unwrap()
                    .submessages
            })
            .filter_map(|f| match f.submessage {
                crate::datagram::ParsedSubmessage::NackFrag(_) => Some("NACK_FRAG"),
                crate::datagram::ParsedSubmessage::AckNack(_) => Some("ACKNACK"),
                _ => None,
            })
            .collect();
        assert!(
            kinds.contains(&"NACK_FRAG") && kinds.contains(&"ACKNACK"),
            "{kinds:?}"
        );
    }

    /// §8.3.8.2: a DATA's in-line OWNERSHIP_STRENGTH and LIFESPAN reach the
    /// consumer as overrides of its writer's QoS; a DATA without them
    /// carries none.
    #[test]
    fn a_data_hands_on_the_in_line_strength_and_lifespan() {
        use crate::parameter_list::{Parameter, ParameterList, pid};
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let mut qos = ParameterList::new();
        qos.push(Parameter::new(
            pid::OWNERSHIP_STRENGTH,
            (-7i32).to_le_bytes().to_vec(),
        ));
        let mut lifespan = 2i32.to_le_bytes().to_vec();
        lifespan.extend_from_slice(&(1u32 << 31).to_le_bytes());
        qos.push(Parameter::new(pid::LIFESPAN, lifespan));
        let mut with = data(w, rd, 1, 0x11);
        with.inline_qos = Some(qos);
        let out = r.handle_data(p1(), &with, None);
        assert_eq!(out.len(), 1);
        let o = out[0].inline_qos_override;
        assert_eq!(o.ownership_strength, Some(-7));
        assert_eq!(
            o.lifespan,
            Some(zerodds_qos::Duration {
                seconds: 2,
                fraction: 1 << 31
            })
        );
        let out = r.handle_data(p1(), &data(w, rd, 2, 0x22), None);
        assert_eq!(out[0].inline_qos_override, Default::default());
    }

    /// §8.3.8.3: one DATA_FRAG carries `fragmentsInSubmessage` consecutive
    /// fragments, the last of the sample shorter; the reader reassembles
    /// the sample from them.
    #[test]
    fn a_data_frag_with_many_fragments_is_reassembled() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let frag = |start: u32, n: u16, bytes: &[u8]| crate::submessages::DataFragSubmessage {
            inline_qos: None,
            extra_flags: 0,
            reader_id: rd,
            writer_id: w,
            writer_sn: sn(1),
            fragment_starting_num: crate::wire_types::FragmentNumber(start),
            fragments_in_submessage: n,
            fragment_size: 4,
            sample_size: 14,
            serialized_payload: alloc::sync::Arc::from(bytes),
            inline_qos_flag: false,
            key_flag: false,
            non_standard_flag: false,
        };
        // 14 octets in fragments of 4: fragments 1 and 2 in one submessage,
        // then 3 and 4 (the last holds 2 octets).
        let first = r.handle_data_frag(
            p1(),
            &frag(1, 2, &[1, 2, 3, 4, 5, 6, 7, 8]),
            Duration::from_secs(1),
            None,
        );
        assert!(first.is_empty(), "not complete yet");
        let done = r.handle_data_frag(
            p1(),
            &frag(3, 2, &[9, 10, 11, 12, 13, 14]),
            Duration::from_secs(2),
            None,
        );
        assert_eq!(done.len(), 1);
        assert_eq!(
            &done[0].payload[..],
            &(1..=14u8).collect::<alloc::vec::Vec<u8>>()[..]
        );
    }

    /// §8.3.8.6: "if the FinalFlag is not set, then the Reader must send an
    /// AckNack message indicating which CacheChange changes it has received,
    /// even if the AckNack indicates it has received all CacheChange changes
    /// in the writer's HistoryCache." With the LivelinessFlag too.
    #[test]
    fn a_heartbeat_without_the_final_flag_is_answered_when_everything_is_received() {
        for liveliness in [false, true] {
            let mut r = make_reader(10);
            let w = single_writer_guid().entity_id;
            let rd = r.guid().entity_id;
            r.add_writer_proxy(WriterProxy::new(
                single_writer_guid(),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
                alloc::vec![],
                true,
            ));
            assert_eq!(r.handle_data(p1(), &data(w, rd, 1, 0x11), None).len(), 1);
            let mut hb = heartbeat(w, rd, 1, 1, 1, false);
            hb.liveliness_flag = liveliness;
            let _ = r.handle_heartbeat(p1(), &hb, Duration::from_secs(5));
            let out = r.tick_outbound(Duration::from_secs(6)).unwrap();
            let acks: alloc::vec::Vec<_> = out
                .iter()
                .flat_map(|dg| {
                    crate::datagram::decode_datagram(&dg.bytes)
                        .unwrap()
                        .submessages
                })
                .filter_map(|f| match f.submessage {
                    crate::datagram::ParsedSubmessage::AckNack(a) => Some(a),
                    _ => None,
                })
                .collect();
            assert_eq!(acks.len(), 1, "liveliness={liveliness}");
            assert_eq!(
                acks[0].reader_sn_state.bitmap_base,
                sn(2),
                "all up to SN 1 received"
            );
            assert_eq!(
                acks[0].reader_sn_state.iter_set().count(),
                0,
                "nothing missing"
            );
        }
    }

    /// §8.3.8.1 Table 8.40 and §8.3.8.12 Table 8.50: `count` is incremented
    /// each time a new ACKNACK, and a new NACK_FRAG, is sent, so that the
    /// writer can tell a new message from a duplicate.
    #[test]
    fn the_counts_of_acknack_and_nack_frag_rise_with_each_message() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        r.add_writer_proxy(WriterProxy::new(
            single_writer_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
            alloc::vec![],
            true,
        ));
        // Fragment 2 of three of SN 1 arrives: fragments 1 and 3 are missing.
        let df = crate::submessages::DataFragSubmessage {
            inline_qos: None,
            extra_flags: 0,
            reader_id: rd,
            writer_id: w,
            writer_sn: sn(1),
            fragment_starting_num: crate::wire_types::FragmentNumber(2),
            fragments_in_submessage: 1,
            fragment_size: 4,
            sample_size: 12,
            serialized_payload: alloc::sync::Arc::from(alloc::vec![1u8, 2, 3, 4].as_slice()),
            inline_qos_flag: false,
            key_flag: false,
            non_standard_flag: false,
        };
        let _ = r.handle_data_frag(p1(), &df, Duration::from_secs(1), None);
        let mut acknack_counts = alloc::vec::Vec::new();
        let mut nackfrag_counts = alloc::vec::Vec::new();
        for round in 1..=3i32 {
            let at = Duration::from_secs(10 * u64::try_from(round).unwrap());
            let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 1, round, false), at);
            for dg in r.tick_outbound(at + Duration::from_secs(1)).unwrap() {
                for frame in crate::datagram::decode_datagram(&dg.bytes)
                    .unwrap()
                    .submessages
                {
                    match frame.submessage {
                        crate::datagram::ParsedSubmessage::AckNack(a) => {
                            acknack_counts.push(a.count)
                        }
                        crate::datagram::ParsedSubmessage::NackFrag(n) => {
                            nackfrag_counts.push(n.count)
                        }
                        _ => {}
                    }
                }
            }
        }
        assert_eq!(acknack_counts, alloc::vec![1, 2, 3]);
        assert_eq!(nackfrag_counts, alloc::vec![1, 2, 3]);
    }

    /// DDSI-RTPS 2.5 §8.3.8.3: a dispose whose serialized key came in two
    /// DATA_FRAGs (K flag, PID_STATUS_INFO on the first) is delivered as the
    /// dispose it is, not as an ALIVE sample.
    #[test]
    fn a_fragmented_dispose_is_delivered_as_a_dispose() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let frag = |n: u32, bytes: [u8; 4]| crate::submessages::DataFragSubmessage {
            inline_qos: (n == 1).then(|| {
                crate::inline_qos::lifecycle_inline_qos(
                    [7; 16],
                    crate::inline_qos::status_info::DISPOSED,
                )
            }),
            extra_flags: 0,
            reader_id: rd,
            writer_id: w,
            writer_sn: sn(1),
            fragment_starting_num: crate::wire_types::FragmentNumber(n),
            fragments_in_submessage: 1,
            fragment_size: 4,
            sample_size: 8,
            serialized_payload: alloc::sync::Arc::from(bytes.as_slice()),
            inline_qos_flag: n == 1,
            key_flag: true,
            non_standard_flag: false,
        };
        assert!(
            r.handle_data_frag(p1(), &frag(1, [1, 2, 3, 4]), Duration::ZERO, None)
                .is_empty()
        );
        let out = r.handle_data_frag(p1(), &frag(2, [5, 6, 7, 8]), Duration::ZERO, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, ChangeKind::NotAliveDisposed);
        assert_eq!(out[0].key_hash, Some([7; 16]));
    }

    #[test]
    fn re_adding_known_writer_preserves_reliability_state() {
        // RTPS: a renewed SPDP/SEDP announce of the same writer must NOT
        // discard the reader reliability state. Otherwise, after a
        // HEARTBEAT(first=1,last=1), the reader falsely reports "nothing missing"
        // (empty ACKNACK) and the reliable writer never delivers the DATA —
        // exactly the cross-vendor secure-SEDP deadlock against Cyclone DDS.
        let mut r = make_reader(10);
        let w_eid = single_writer_guid().entity_id;
        let r_eid = r.guid().entity_id;
        // Writer announces seq 1 (not yet delivered).
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w_eid, r_eid, 1, 1, 1, false),
            Duration::ZERO,
        );
        assert!(first_state(&r).proxy.has_missing_changes());
        assert_eq!(first_state(&r).proxy.last_available_sn(), sn(1));
        // Re-discovery: the same writer proxy is added again.
        r.add_writer_proxy(WriterProxy::new(
            single_writer_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
            alloc::vec![],
            true,
        ));
        // seq 1 must still be known as missing.
        assert!(
            first_state(&r).proxy.has_missing_changes(),
            "re-add must not reset last_available/missing"
        );
        assert_eq!(first_state(&r).proxy.last_available_sn(), sn(1));
    }

    #[test]
    fn in_order_data_delivered_immediately() {
        let mut r = make_reader(10);
        let w_eid = single_writer_guid().entity_id;
        let r_eid = r.guid().entity_id;
        let delivered = r.handle_data(p1(), &data(w_eid, r_eid, 1, 0xAA), None);
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].payload.as_ref(), &[0xAA][..]);
        assert_eq!(delivered[0].writer_guid, single_writer_guid());
        assert_eq!(first_state(&r).delivered_up_to, sn(1));
    }

    #[test]
    fn out_of_order_data_buffered_until_gap_filled() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        assert!(r.handle_data(p1(), &data(w, rd, 2, 0x22), None).is_empty());
        assert!(r.handle_data(p1(), &data(w, rd, 3, 0x33), None).is_empty());
        let out = r.handle_data(p1(), &data(w, rd, 1, 0x11), None);
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            alloc::vec![sn(1), sn(2), sn(3)]
        );
        assert_eq!(first_state(&r).delivered_up_to, sn(3));
    }

    fn gap(wid: EntityId, rid: EntityId, start: i64, list_base: i64) -> GapSubmessage {
        GapSubmessage {
            reader_id: rid,
            writer_id: wid,
            gap_start: sn(start),
            gap_list: crate::submessages::SequenceNumberSet {
                bitmap_base: sn(list_base),
                num_bits: 0,
                bitmap: alloc::vec![],
            },
            group_info: None,
            filtered_count: None,
        }
    }

    /// A GAP makes its SNs irrelevant, which DDSI-RTPS 2.5 §8.4.10.4.3 records
    /// as RECEIVED; the SN after them is then in order, whether or not a
    /// HEARTBEAT has announced anything yet.
    ///
    /// A late-joining reader meets exactly this: the writer's first HEARTBEAT
    /// arrives before the SEDP match and is dropped, and the writer answers the
    /// reader's first ACKNACK with GAP 1..16 and DATA 17 in one datagram
    /// (measured with a ZeroDDS `ros_discovery_info` writer). Held until the
    /// next HEARTBEAT, SN 17 left the reader on the HEARTBEAT path instead.
    #[test]
    fn gap_before_any_heartbeat_lets_the_next_data_through() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        for n in 1..=16 {
            assert!(r.handle_gap(p1(), &gap(w, rd, n, n + 1)).is_empty());
        }
        let out = r.handle_data(p1(), &data(w, rd, 17, 0x17), None);
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            alloc::vec![sn(17)],
            "SNs 1..16 are irrelevant, so SN 17 is the next in order"
        );
        assert_eq!(first_state(&r).delivered_up_to, sn(17));
    }

    /// A GAP that arrives after the DATA it clears the way for releases that
    /// DATA, again without a HEARTBEAT.
    #[test]
    fn gap_after_the_held_data_releases_it() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        assert!(r.handle_data(p1(), &data(w, rd, 3, 0x33), None).is_empty());
        let out = r.handle_gap(p1(), &gap(w, rd, 1, 3));
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            alloc::vec![sn(3)]
        );
        assert_eq!(first_state(&r).delivered_up_to, sn(3));
    }

    /// §8.4.10.4 `not_available_change_set`: filteredCount equal to the set
    /// size gives NA_FILTERED, 0 gives NA_REMOVED, anything between gives
    /// NA_UNSPECIFIED. A GAP without the filteredCount flag counts as 0.
    #[test]
    fn a_gap_sorts_its_changes_by_the_filtered_count() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let count = |r: &ReliableReader| {
            (
                r.not_available_count(NotAvailableKind::Filtered),
                r.not_available_count(NotAvailableKind::Removed),
                r.not_available_count(NotAvailableKind::Unspecified),
            )
        };
        // SNs 1..3 (three changes), filteredCount 3: all filtered.
        let mut g = gap(w, rd, 1, 4);
        g.filtered_count = Some(3);
        let _ = r.handle_gap(p1(), &g);
        assert_eq!(count(&r), (3, 0, 0));
        // SNs 4..5, filteredCount 0: removed.
        let mut g = gap(w, rd, 4, 6);
        g.filtered_count = Some(0);
        let _ = r.handle_gap(p1(), &g);
        assert_eq!(count(&r), (3, 2, 0));
        // SNs 6..9, filteredCount 1 of 4: unspecified.
        let mut g = gap(w, rd, 6, 10);
        g.filtered_count = Some(1);
        let _ = r.handle_gap(p1(), &g);
        assert_eq!(count(&r), (3, 2, 4));
        // No filteredCount: removed; a repeated GAP counts nothing again.
        let _ = r.handle_gap(p1(), &gap(w, rd, 10, 12));
        let _ = r.handle_gap(p1(), &gap(w, rd, 10, 12));
        assert_eq!(count(&r), (3, 4, 4));
    }

    /// Table 8.74 `status`, per change: a GAP stores NA_FILTERED, NA_REMOVED
    /// or NA_UNSPECIFIED for each of its changes (§8.4.12.1 T4, §8.4.12.2 T9),
    /// a DATA with the FilteredFlag stores NA_FILTERED, a HEARTBEAT with a
    /// larger `firstSN` stores NA_REMOVED for what the reader still missed.
    #[test]
    fn the_reader_keeps_the_status_of_each_change() {
        use crate::writer_proxy::ChangeFromWriterStatus as S;
        let mut r = make_reader(10);
        let w = single_writer_guid();
        let rd = r.guid().entity_id;
        let status = |r: &ReliableReader, n: i64| r.change_status(w, sn(n));
        let na = |k| Some(S::NotAvailable(k));
        let mut g = gap(w.entity_id, rd, 1, 3);
        g.filtered_count = Some(2);
        let _ = r.handle_gap(p1(), &g);
        assert_eq!(status(&r, 1), na(NotAvailableKind::Filtered));
        assert_eq!(status(&r, 2), na(NotAvailableKind::Filtered));
        let mut g = gap(w.entity_id, rd, 3, 5);
        g.filtered_count = Some(0);
        let _ = r.handle_gap(p1(), &g);
        assert_eq!(status(&r, 3), na(NotAvailableKind::Removed));
        assert_eq!(status(&r, 4), na(NotAvailableKind::Removed));
        let mut g = gap(w.entity_id, rd, 5, 7);
        g.filtered_count = Some(1);
        let _ = r.handle_gap(p1(), &g);
        assert_eq!(status(&r, 5), na(NotAvailableKind::Unspecified));
        assert_eq!(status(&r, 6), na(NotAvailableKind::Unspecified));
        // DATA 7 with F = 1: filtered; DATA 8 plain: received.
        let mut marker = lifecycle_data(
            w.entity_id,
            rd,
            7,
            [0xAB; 16],
            crate::inline_qos::status_info::FILTERED,
        );
        marker.key_flag = false;
        let _ = r.handle_data(p1(), &marker, None);
        assert_eq!(status(&r, 7), na(NotAvailableKind::Filtered));
        let _ = r.handle_data(p1(), &data(w.entity_id, rd, 8, 0x08), None);
        assert_eq!(status(&r, 8), Some(S::Received));
        // HEARTBEAT 1..12 announces 9..12, firstSN 11 makes 9 and 10 gone.
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w.entity_id, rd, 1, 12, 1, false),
            Duration::ZERO,
        );
        assert_eq!(status(&r, 9), Some(S::Missing));
        assert_eq!(status(&r, 13), Some(S::Unknown));
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w.entity_id, rd, 11, 12, 2, false),
            Duration::ZERO,
        );
        assert_eq!(status(&r, 9), na(NotAvailableKind::Removed));
        assert_eq!(status(&r, 10), na(NotAvailableKind::Removed));
        assert_eq!(status(&r, 11), Some(S::Missing));
        assert_eq!(r.not_available_count(NotAvailableKind::Removed), 4);
        assert_eq!(r.not_available_count(NotAvailableKind::Filtered), 3);
        assert_eq!(r.not_available_count(NotAvailableKind::Unspecified), 2);
        // A writer that is not matched has no status.
        assert_eq!(r.change_status(Guid::UNKNOWN, sn(1)), None);
    }

    /// A BEST_EFFORT reader (RTPS §8.4.12.1) must NOT block on a leading gap —
    /// it does not NACK to repair it, so waiting would deadlock forever. It
    /// delivers from the lowest received SN. Regression for the cross-vendor
    /// OpenDDS/RTI case where the writer's first sample to us is mid-stream.
    #[test]
    fn best_effort_reader_skips_leading_gap() {
        let mut r = make_reader(10);
        r.set_best_effort(true);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        // SN 1 never arrives. A reliable reader would buffer SN 2/3 forever
        // (see `out_of_order_data_buffered_until_gap_filled`); a best-effort
        // reader delivers SN 2 immediately, skipping the missing SN 1.
        let out = r.handle_data(p1(), &data(w, rd, 2, 0x22), None);
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            alloc::vec![sn(2)],
            "best-effort reader must deliver SN 2 despite the missing SN 1"
        );
        assert_eq!(first_state(&r).delivered_up_to, sn(2));
        // Subsequent in-order samples keep flowing.
        let out3 = r.handle_data(p1(), &data(w, rd, 3, 0x33), None);
        assert_eq!(out3.len(), 1);
        assert_eq!(out3[0].sequence_number, sn(3));
    }

    #[test]
    fn duplicate_data_is_rejected() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let _ = r.handle_data(p1(), &data(w, rd, 1, 0xAA), None);
        let second = r.handle_data(p1(), &data(w, rd, 1, 0xAA), None);
        assert!(second.is_empty());
    }

    #[test]
    fn mismatched_writer_id_is_counted() {
        let mut r = make_reader(10);
        let rd = r.guid().entity_id;
        let foreign = EntityId::user_writer_with_key([0xFF, 0xFF, 0xFF]);
        assert!(
            r.handle_data(p1(), &data(foreign, rd, 1, 0xAA), None)
                .is_empty()
        );
        assert_eq!(r.unknown_src_count(), 1);
    }

    // ---------- Wire-Side Lifecycle (T8) ----------

    #[test]
    fn alive_data_yields_alive_changekind() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let delivered = r.handle_data(p1(), &data(w, rd, 1, 0xAA), None);
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::Alive);
    }

    fn lifecycle_data(
        wid: EntityId,
        rid: EntityId,
        n: i64,
        key_hash: [u8; 16],
        status_bits: u32,
    ) -> DataSubmessage {
        DataSubmessage {
            extra_flags: 0,
            reader_id: rid,
            writer_id: wid,
            writer_sn: sn(n),
            inline_qos: Some(crate::inline_qos::lifecycle_inline_qos(
                key_hash,
                status_bits,
            )),
            data_flag: false,
            key_flag: true,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from(alloc::vec![0u8; 0]),
        }
    }

    #[test]
    fn dispose_data_yields_not_alive_disposed() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let delivered = r.handle_data(
            p1(),
            &lifecycle_data(
                w,
                rd,
                1,
                [0xAB; 16],
                crate::inline_qos::status_info::DISPOSED,
            ),
            None,
        );
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::NotAliveDisposed);
    }

    /// Fast DDS 3.6.1's measured dispose form: `D=0`, `K=0`, no payload,
    /// `PID_KEY_HASH` + `PID_STATUS_INFO` in the inline QoS. It is a marker,
    /// not an ALIVE sample with an empty body.
    #[test]
    fn keyless_dispose_marker_yields_not_alive_disposed() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let mut marker = lifecycle_data(
            w,
            rd,
            1,
            [0xAB; 16],
            crate::inline_qos::status_info::DISPOSED | crate::inline_qos::status_info::UNREGISTERED,
        );
        marker.key_flag = false;
        let delivered = r.handle_data(p1(), &marker, None);
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::NotAliveDisposedUnregistered);
        assert_eq!(delivered[0].key_hash, Some([0xAB; 16]));
    }

    /// Table 8.72 `dataMaxSizeSerialized`: a payload beyond the bound the
    /// writer announced is refused and counted, and the next one is in order.
    #[test]
    fn a_payload_beyond_the_announced_maximum_is_refused_and_counted() {
        let mut r = make_reader(10);
        let w = single_writer_guid();
        let rd = r.guid().entity_id;
        r.set_writer_data_max_size(w, Some(1));
        // `data()` carries one octet: within the bound.
        let out = r.handle_data(p1(), &data(w.entity_id, rd, 1, 0x11), None);
        assert_eq!(out.len(), 1);
        let mut big = data(w.entity_id, rd, 2, 0x22);
        big.serialized_payload = crate::payload::Payload::from(alloc::vec![0x22, 0x23]);
        assert!(r.handle_data(p1(), &big, None).is_empty());
        assert_eq!(r.oversize_payload_count(), 1);
        let out = r.handle_data(p1(), &data(w.entity_id, rd, 3, 0x33), None);
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            alloc::vec![sn(3)]
        );
        // No bound announced, or a negative one: none.
        r.set_writer_data_max_size(w, None);
        let out = r.handle_data(p1(), &big_with(w.entity_id, rd, 4), None);
        assert_eq!(out.len(), 1);
        assert_eq!(r.oversize_payload_count(), 1);
    }

    fn big_with(wid: EntityId, rid: EntityId, n: i64) -> DataSubmessage {
        let mut d = data(wid, rid, n, 0x44);
        d.serialized_payload = crate::payload::Payload::from(alloc::vec![0x44; 64]);
        d
    }

    /// §9.6.4.9: F = 1 with D = U = 0 is a sample that did not pass the
    /// reader's filter. It is known, so the next sample is in order, and it
    /// is not delivered. With D or U set, the lifecycle marker wins.
    #[test]
    fn a_sample_with_the_filtered_flag_is_known_but_not_delivered() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let filtered = crate::inline_qos::status_info::FILTERED;
        let mut marker = lifecycle_data(w, rd, 1, [0xAB; 16], filtered);
        marker.key_flag = false;
        assert!(r.handle_data(p1(), &marker, None).is_empty());
        assert_eq!(r.filtered_sample_count(), 1);
        let out = r.handle_data(p1(), &data(w, rd, 2, 0x22), None);
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            alloc::vec![sn(2)]
        );
        let disposed = r.handle_data(
            p1(),
            &lifecycle_data(
                w,
                rd,
                3,
                [0xAB; 16],
                filtered | crate::inline_qos::status_info::DISPOSED,
            ),
            None,
        );
        assert_eq!(disposed.len(), 1);
        assert_eq!(disposed[0].kind, ChangeKind::NotAliveDisposed);
        assert_eq!(r.filtered_sample_count(), 1);
    }

    #[test]
    fn unregister_data_yields_not_alive_unregistered() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let delivered = r.handle_data(
            p1(),
            &lifecycle_data(
                w,
                rd,
                1,
                [0xCD; 16],
                crate::inline_qos::status_info::UNREGISTERED,
            ),
            None,
        );
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::NotAliveUnregistered);
    }

    #[test]
    fn dispose_and_unregister_combined() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let bits =
            crate::inline_qos::status_info::DISPOSED | crate::inline_qos::status_info::UNREGISTERED;
        let delivered = r.handle_data(p1(), &lifecycle_data(w, rd, 1, [0xEF; 16], bits), None);
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::NotAliveDisposedUnregistered);
    }

    #[test]
    fn key_only_alive_registration_is_acked_but_not_delivered() {
        // A key_flag=true DATA without a DISPOSED/UNREGISTERED status is a
        // key-only *instance registration* (e.g. OpenDDS register_instance):
        // the payload holds only the @key fields, not a full sample. It must
        // NOT be delivered to the application — full-decoding the key-only
        // payload would raise a spurious cross-vendor decode error. The SN is
        // still acknowledged so the reliable protocol advances; the actual data
        // arrives in the D-flag samples.
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let mut d = data(w, rd, 1, 0xAA);
        d.key_flag = true;
        let delivered = r.handle_data(p1(), &d, None);
        assert!(
            delivered.is_empty(),
            "key-only ALIVE registration must not be delivered"
        );
        // A subsequent full (D-flag) sample at SN 2 is delivered: the reader
        // advanced past the registration without stalling.
        let d2 = data(w, rd, 2, 0xBB);
        let delivered2 = r.handle_data(p1(), &d2, None);
        assert_eq!(delivered2.len(), 1);
        assert_eq!(delivered2[0].sequence_number, SequenceNumber(2));
    }

    #[test]
    fn heartbeat_with_missing_triggers_acknack_after_delay() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 3, 1, false), Duration::ZERO);
        assert!(r.tick(Duration::from_millis(100)).unwrap().is_empty());
        let out = r.tick(Duration::from_millis(250)).unwrap();
        assert_eq!(out.len(), 1);
    }

    /// §8.4.10.1: `heartbeatSuppressionDuration` lets the reader "ignore
    /// HEARTBEATs that arrive 'too soon' after a previous HEARTBEAT was
    /// received". The second HEARTBEAT arrives 30 ms after the first
    /// with a wider range; under a 100 ms suppression the reader must
    /// not take its range, and the one at 150 ms must.
    #[test]
    fn heartbeat_suppression_duration_ignores_a_heartbeat_that_arrives_too_soon() {
        let mut r = make_reader(10);
        r.set_heartbeat_suppression_duration(Duration::from_millis(100));
        assert_eq!(
            r.heartbeat_suppression_duration(),
            Duration::from_millis(100)
        );
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;

        let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 2, 1, false), Duration::ZERO);
        assert_eq!(first_state(&r).proxy.last_available_sn(), sn(2));

        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w, rd, 1, 9, 2, false),
            Duration::from_millis(30),
        );
        assert_eq!(
            first_state(&r).proxy.last_available_sn(),
            sn(2),
            "the suppressed HEARTBEAT must not widen the range"
        );

        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(w, rd, 1, 9, 3, false),
            Duration::from_millis(150),
        );
        assert_eq!(first_state(&r).proxy.last_available_sn(), sn(9));
    }

    /// The spec default is zero, and zero suppresses nothing: two
    /// HEARTBEATs in the same instant both take effect.
    #[test]
    fn default_heartbeat_suppression_duration_is_the_spec_zero() {
        assert_eq!(
            DEFAULT_HEARTBEAT_SUPPRESSION_DURATION,
            Duration::ZERO,
            "§8.4.10.1.1 heartbeatSuppressionDuration"
        );
        assert_eq!(
            SPEC_HEARTBEAT_RESPONSE_DELAY,
            Duration::from_millis(500),
            "§8.4.10.1.1 heartbeatResponseDelay"
        );
        let mut r = make_reader(10);
        assert_eq!(r.heartbeat_suppression_duration(), Duration::ZERO);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 2, 1, false), Duration::ZERO);
        let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 7, 2, false), Duration::ZERO);
        assert_eq!(first_state(&r).proxy.last_available_sn(), sn(7));
    }

    #[test]
    fn heartbeat_without_missing_and_final_schedules_no_acknack() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let _ = r.handle_data(p1(), &data(w, rd, 1, 0xAA), None);
        let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 1, 1, true), Duration::ZERO);
        assert!(r.tick(Duration::from_secs(10)).unwrap().is_empty());
    }

    /// Measured against HDDS 1.4.8 (domain 0, `tshark -V`, frame 24): a Final
    /// HEARTBEAT `[1,1]` from a writer this reader has never received data
    /// from must produce an ACKNACK that actually names SN 1 as missing —
    /// not an empty bitmap — and that ACKNACK must not claim `final` (it
    /// requests real data, so the writer must answer).
    #[test]
    fn final_heartbeat_from_unknown_to_data_writer_requests_the_missing_sn() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 1, 1, true), Duration::ZERO);
        let out = r.tick(Duration::from_millis(200)).unwrap();
        assert_eq!(
            out.len(),
            1,
            "a HEARTBEAT over an entirely unreceived range must be ACKed"
        );
        let parsed = decode_datagram(&out[0]).unwrap();
        let ack = parsed
            .contents()
            .find_map(|s| {
                if let ParsedSubmessage::AckNack(a) = s {
                    Some(a)
                } else {
                    None
                }
            })
            .expect("ACKNACK in datagram");
        assert_eq!(ack.reader_sn_state.bitmap_base, sn(1));
        assert_eq!(
            ack.reader_sn_state.num_bits, 1,
            "SN 1 must be named as missing, not an empty bitmap"
        );
        assert!(
            !ack.final_flag,
            "a NACK that requests real data must not claim final"
        );
    }

    /// A remote writer that keeps re-announcing the same unfulfilled range on
    /// every packet — measured: HDDS 1.4.8 re-sent a Final HEARTBEAT `[1,1]`
    /// roughly every 1.5 ms without ever delivering SN 1, 8509 of them
    /// against 8510 ACKNACKs in 13 s while the reader answered at once —
    /// must not turn this reader into a 1:1 ACKNACK echo. Under the spec's
    /// timing (heartbeatResponseDelay 500 ms, heartbeatSuppressionDuration
    /// 0, §8.4.10.1) the response waits for the first HEARTBEAT that asked
    /// for it, not the last, so the reader answers once per delay however
    /// fast the writer re-heartbeats; it still keeps asking (it never gives
    /// up on a RELIABLE writer that still claims to hold the data).
    #[test]
    fn heartbeat_storm_from_a_writer_that_never_fulfils_a_nack_cannot_arise() {
        let mut r = ReliableReader::new(ReliableReaderConfig {
            guid: Guid::new(
                GuidPrefix::from_bytes([2; 12]),
                EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
            ),
            vendor_id: VendorId::ZERODDS,
            writer_proxies: alloc::vec![WriterProxy::new(
                single_writer_guid(),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
                alloc::vec![],
                true,
            )],
            max_samples_per_proxy: 10,
            heartbeat_response_delay: DEFAULT_HEARTBEAT_RESPONSE_DELAY,
            assembler_caps: AssemblerCaps::default(),
        });
        assert_eq!(r.heartbeat_suppression_duration(), Duration::ZERO);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let mut acknacks = 0usize;
        // ~1.3 s of HEARTBEATs every 650 µs (HDDS-measured cadence); SN 1
        // never arrives.
        for step in 0..2000i32 {
            let now = Duration::from_micros(u64::from(step.unsigned_abs()) * 650);
            let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 1, step + 1, true), now);
            acknacks += r.tick(now).unwrap().len();
        }
        assert!(
            acknacks <= 3,
            "2000 identical HEARTBEATs over ~1.3 s get one response per \
             500 ms, at most 3, got {acknacks}"
        );
        assert!(
            acknacks > 0,
            "the reader must still eventually re-request SN 1, got {acknacks}"
        );
    }

    // ---------- Multi-writer (T4.5) ----------

    fn second_writer_guid() -> Guid {
        Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::user_writer_with_key([0x40, 0x50, 0x60]),
        )
    }

    fn add_second_writer(r: &mut ReliableReader) {
        r.add_writer_proxy(WriterProxy::new(
            second_writer_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 2], 7420)],
            alloc::vec![],
            true,
        ));
    }

    #[test]
    fn add_writer_proxy_increases_count() {
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        assert_eq!(r.writer_proxy_count(), 2);
    }

    #[test]
    fn two_writers_with_overlapping_sn_spaces_both_delivered() {
        // Core regression: both writers use SN 1. Without per-proxy
        // state the second `handle_data` would be rejected as a duplicate.
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        let w1 = single_writer_guid().entity_id;
        let w2 = second_writer_guid().entity_id;
        let rd = r.guid().entity_id;

        let d1 = r.handle_data(p1(), &data(w1, rd, 1, 0xAA), None);
        let d2 = r.handle_data(p2(), &data(w2, rd, 1, 0xBB), None);

        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0].payload.as_ref(), &[0xAA][..]);
        assert_eq!(d1[0].writer_guid, single_writer_guid());
        assert_eq!(d2.len(), 1);
        assert_eq!(d2[0].payload.as_ref(), &[0xBB][..]);
        assert_eq!(d2[0].writer_guid, second_writer_guid());

        assert_eq!(r.writer_proxies()[0].delivered_up_to, sn(1));
        assert_eq!(r.writer_proxies()[1].delivered_up_to, sn(1));
    }

    #[test]
    fn same_entity_id_different_prefix_not_confused() {
        // Regression H-1/H-2: two remote writers with the SAME entity_id but
        // different guid_prefix (the normal case for fan-in — entity keys are
        // assigned per-participant from a low base, the first user writer
        // of each participant shares the same entity_id). A sample from writer B
        // must NOT be attributed to writer A (first proxy).
        let mut r = make_reader(10);
        let eid = single_writer_guid().entity_id; // identical to proxy 0
        let prefix_b = GuidPrefix::from_bytes([9; 12]);
        let guid_b = Guid::new(prefix_b, eid);
        r.add_writer_proxy(WriterProxy::new(
            guid_b,
            alloc::vec![Locator::udp_v4([127, 0, 0, 9], 7420)],
            alloc::vec![],
            true,
        ));
        assert_eq!(r.writer_proxy_count(), 2);
        let rd = r.guid().entity_id;
        // Sample from B: must be assigned to the B proxy, not A.
        let d = r.handle_data(prefix_b, &data(eid, rd, 1, 0xBB), None);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].writer_guid, guid_b, "sample misattributed");
        // The A proxy (proxy 0) must NOT have advanced.
        assert_eq!(r.writer_proxies()[0].delivered_up_to, sn(0));
        assert_eq!(r.writer_proxies()[1].delivered_up_to, sn(1));
    }

    #[test]
    fn remove_writer_proxy_drops_its_state() {
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        let removed = r.remove_writer_proxy(single_writer_guid());
        assert!(removed.is_some());
        assert_eq!(r.writer_proxy_count(), 1);
        assert_eq!(
            r.writer_proxies()[0].proxy.remote_writer_guid,
            second_writer_guid()
        );
    }

    #[test]
    fn tick_emits_one_acknack_per_writer_with_missing() {
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        let rd = r.guid().entity_id;
        // Both writers send an HB with a missing SN
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(single_writer_guid().entity_id, rd, 1, 3, 1, false),
            Duration::ZERO,
        );
        let _ = r.handle_heartbeat(
            p2(),
            &heartbeat(second_writer_guid().entity_id, rd, 1, 5, 1, false),
            Duration::ZERO,
        );
        let out = r.tick(Duration::from_millis(250)).unwrap();
        // 2 ACKNACKs (one per writer)
        assert_eq!(out.len(), 2);
    }

    /// DDSI-RTPS 2.5 §8.4.2.3: a reader reports missing samples "only if
    /// the Reader can accommodate these missing samples in its cache", and
    /// the response "may be delayed". A receive cache full of samples the
    /// consumer has not taken holds the ACKNACK back; once the consumer
    /// takes one, the ACKNACK names no more changes than there is room for.
    #[test]
    fn a_cache_full_for_the_consumer_holds_the_acknack_until_it_takes() {
        let mut r = make_reader(2);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        r.set_release_budget(Some(0));
        let _ = r.handle_data(p1(), &data(w, rd, 1, 1), None);
        let _ = r.handle_data(p1(), &data(w, rd, 2, 2), None);
        // No room for SN 3: it stays missing.
        let _ = r.handle_data(p1(), &data(w, rd, 3, 3), None);
        let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 6, 1, false), Duration::ZERO);
        assert!(
            r.tick(Duration::from_millis(250)).unwrap().is_empty(),
            "a full cache holds the ACKNACK back"
        );

        r.set_release_budget(Some(1));
        assert_eq!(r.release_held().len(), 1, "the consumer takes SN 1");
        let out = r.tick(Duration::from_millis(500)).unwrap();
        assert_eq!(out.len(), 1, "the held-back ACKNACK goes out");
        let parsed = decode_datagram(&out[0]).unwrap();
        let ack = parsed
            .contents()
            .find_map(|s| {
                if let ParsedSubmessage::AckNack(a) = s {
                    Some(a)
                } else {
                    None
                }
            })
            .expect("ACKNACK in datagram");
        assert_eq!(ack.reader_sn_state.bitmap_base, sn(3));
        assert_eq!(
            ack.reader_sn_state
                .iter_set()
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![sn(3)],
            "one free slot, one change named of the four missing"
        );
    }

    /// The other full cache: full behind a change still missing in front
    /// of it. Holding the ACKNACK back there would never let the cache
    /// drain, so the missing change is named.
    #[test]
    fn a_cache_full_behind_a_missing_change_still_names_it() {
        let mut r = make_reader(2);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let _ = r.handle_data(p1(), &data(w, rd, 2, 2), None);
        let _ = r.handle_data(p1(), &data(w, rd, 3, 3), None);
        let _ = r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 3, 1, false), Duration::ZERO);
        let out = r.tick(Duration::from_millis(250)).unwrap();
        assert_eq!(out.len(), 1, "the ACKNACK goes out");
        let parsed = decode_datagram(&out[0]).unwrap();
        let ack = parsed
            .contents()
            .find_map(|s| {
                if let ParsedSubmessage::AckNack(a) = s {
                    Some(a)
                } else {
                    None
                }
            })
            .expect("ACKNACK in datagram");
        assert_eq!(
            ack.reader_sn_state
                .iter_set()
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![sn(1)]
        );
    }

    // ---------- WP 1.E stage B: pre-emptive ACKNACK ----------

    /// §8.4.2.3.4: when matching a new writer proxy the reader sends
    /// **proactively** an ACKNACK with `bitmap_base=1, num_bits=0,
    /// final_flag=false` — this speeds up the first data flow by
    /// exactly one HB period (typ. 1 s).
    #[test]
    fn pre_emptive_acknack_emitted_after_add_writer_proxy() {
        let reader_guid = Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
        );
        let mut r = ReliableReader::new(ReliableReaderConfig {
            guid: reader_guid,
            vendor_id: VendorId::ZERODDS,
            writer_proxies: alloc::vec![],
            max_samples_per_proxy: 10,
            heartbeat_response_delay: Duration::from_millis(200),
            assembler_caps: AssemblerCaps::default(),
        });
        r.add_writer_proxy(WriterProxy::new(
            single_writer_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
            alloc::vec![],
            true,
        ));
        // Values from add_writer_proxy: pending_acknack_since=Duration::ZERO
        // → tick(>=delay) yields the pre-emptive AckNack.
        let out = r.tick(Duration::from_millis(250)).unwrap();
        assert_eq!(out.len(), 1, "exactly one Pre-Emptive ACKNACK expected");
        let parsed = decode_datagram(&out[0]).unwrap();
        let ack = parsed
            .contents()
            .find_map(|s| {
                if let ParsedSubmessage::AckNack(a) = s {
                    Some(a)
                } else {
                    None
                }
            })
            .expect("ACKNACK in datagram");
        assert_eq!(ack.reader_sn_state.bitmap_base, sn(1));
        assert_eq!(ack.reader_sn_state.num_bits, 0);
        assert!(
            !ack.final_flag,
            "Pre-Emptive ACKNACK must be non-final (force HB-response)"
        );
    }

    /// Pre-emptive ACKNACK does NOT happen if `add_writer_proxy` was never
    /// called (defensive sanity check for the default reader).
    #[test]
    fn no_pre_emptive_acknack_without_proxy() {
        let reader_guid = Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
        );
        let mut r = ReliableReader::new(ReliableReaderConfig {
            guid: reader_guid,
            vendor_id: VendorId::ZERODDS,
            writer_proxies: alloc::vec![],
            max_samples_per_proxy: 10,
            heartbeat_response_delay: Duration::from_millis(200),
            assembler_caps: AssemblerCaps::default(),
        });
        // No proxies → no ACKNACK
        assert!(r.tick(Duration::from_secs(10)).unwrap().is_empty());
    }

    /// Initial proxies from `ReliableReaderConfig.writer_proxies` get
    /// **no** automatic pre-emptive — only via `add_writer_proxy`.
    /// This is consistent with the DCPS integration: the discovery layer calls
    /// `add_writer_proxy` as soon as the SEDP match is established.
    #[test]
    fn initial_proxy_from_config_does_not_send_pre_emptive() {
        // make_reader() uses config.writer_proxies, not add_writer_proxy
        let mut r = make_reader(10);
        // Before add: no pre-emptive even after a long tick
        assert!(
            r.tick(Duration::from_secs(10)).unwrap().is_empty(),
            "initial proxy from config must not emit Pre-Emptive"
        );
    }

    #[test]
    fn pre_emptive_acknack_carries_info_dst() {
        // The pre-emptive ACKNACK MUST be wrapped in INFO_DST(writer_prefix),
        // otherwise Cyclone/Fast-DDS discard the submessage as
        // "not for me" (Spec §8.3.7.6 / §8.3.8.7).
        let reader_guid = Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
        );
        let mut r = ReliableReader::new(ReliableReaderConfig {
            guid: reader_guid,
            vendor_id: VendorId::ZERODDS,
            writer_proxies: alloc::vec![],
            max_samples_per_proxy: 10,
            heartbeat_response_delay: Duration::from_millis(200),
            assembler_caps: AssemblerCaps::default(),
        });
        r.add_writer_proxy(WriterProxy::new(
            single_writer_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
            alloc::vec![],
            true,
        ));
        let out = r.tick(Duration::from_millis(250)).unwrap();
        assert_eq!(out.len(), 1);
        let parsed = decode_datagram(&out[0]).unwrap();
        // submessages[0] = INFO_DST (Unknown in the decoder, because InfoDst
        // is not unpacked by the decoder), [1] = ACKNACK
        assert!(parsed.submessages.len() >= 2, "INFO_DST + ACKNACK");
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::InfoDestination(gp) => {
                assert_eq!(*gp, single_writer_guid().prefix.to_bytes())
            }
            other => panic!("expected INFO_DST first, got {other:?}"),
        }
    }

    #[test]
    fn unknown_writer_id_in_heartbeat_counts_not_crashes() {
        let mut r = make_reader(10);
        let rd = r.guid().entity_id;
        let foreign = EntityId::user_writer_with_key([0xFF, 0xFF, 0xFF]);
        let _ = r.handle_heartbeat(
            p1(),
            &heartbeat(foreign, rd, 1, 3, 1, false),
            Duration::ZERO,
        );
        assert_eq!(r.unknown_src_count(), 1);
        assert!(r.tick(Duration::from_secs(1)).unwrap().is_empty());
    }

    /// A DATA of `forwarder`'s writer with `PID_ORIGINAL_WRITER_INFO`
    /// naming the change `original_sn` of `original`.
    fn forwarded(n: i64, original: Guid, original_sn: i64, byte: u8) -> DataSubmessage {
        let mut d = data(second_writer_guid().entity_id, EntityId::UNKNOWN, n, byte);
        let mut pl = crate::parameter_list::ParameterList::new();
        pl.push(crate::inline_qos::original_writer_info_param(
            original.to_bytes(),
            original_sn,
            true,
        ));
        d.inline_qos = Some(pl);
        d
    }

    fn sns(out: &[DeliveredSample]) -> Vec<(Guid, i64, Option<Guid>)> {
        out.iter()
            .map(|s| (s.writer_guid, s.sequence_number.0, s.forwarded_by))
            .collect()
    }

    /// DDSI-RTPS 2.5 §8.7.9: a forwarded change is handed out as the
    /// original writer's, with its sequence number; the copy from the
    /// original afterwards is a duplicate.
    #[test]
    fn a_forwarded_change_is_the_originals_and_its_direct_copy_a_duplicate() {
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        let alice = single_writer_guid();
        let trent = second_writer_guid();
        let rd = r.guid().entity_id;
        let out = r.handle_data(p2(), &forwarded(1, alice, 1, 0xA1), None);
        assert_eq!(sns(&out), alloc::vec![(alice, 1, Some(trent))]);
        assert_eq!(out[0].payload.as_ref(), &[0xA1]);
        let out = r.handle_data(p1(), &data(alice.entity_id, rd, 1, 0xA1), None);
        assert!(out.is_empty(), "the original's copy is a duplicate");
        let out = r.handle_data(p1(), &data(alice.entity_id, rd, 2, 0xA2), None);
        assert_eq!(sns(&out), alloc::vec![(alice, 2, None)]);
        assert_eq!(r.forwarded_duplicate_count(), 1);
    }

    /// A change a writer of the reader's own participant forwards past the
    /// receive path counts against the original's stream like one that
    /// arrived forwarded: new once, a duplicate after, and the original's
    /// own copy that follows is dropped (DDSI-RTPS 2.5 §8.7.9).
    #[test]
    fn a_change_forwarded_inside_the_participant_is_deduplicated_with_the_original() {
        let mut r = make_reader(10);
        let alice = single_writer_guid();
        let rd = r.guid().entity_id;
        assert!(r.accept_forwarded(alice, SequenceNumber(1)));
        assert!(
            !r.accept_forwarded(alice, SequenceNumber(1)),
            "a second copy"
        );
        assert!(
            r.handle_data(p1(), &data(alice.entity_id, rd, 1, 1), None)
                .is_empty(),
            "the original's copy after the forwarded one"
        );
        assert_eq!(
            sns(&r.handle_data(p1(), &data(alice.entity_id, rd, 2, 2), None)),
            alloc::vec![(alice, 2, None)]
        );
        assert!(
            !r.accept_forwarded(alice, SequenceNumber(2)),
            "forwarded after the original's"
        );
        // A writer the reader has no proxy for is remembered as well.
        let carol = Guid::new(GuidPrefix::from_bytes([7; 12]), alice.entity_id);
        assert!(r.accept_forwarded(carol, SequenceNumber(5)));
        assert!(!r.accept_forwarded(carol, SequenceNumber(5)));
        assert_eq!(r.forwarded_duplicate_count(), 4);
    }

    /// The other order: the original's change first, the forwarded copy
    /// is dropped, and so is a second forwarded copy.
    #[test]
    fn a_change_from_the_original_is_not_handed_out_again_when_forwarded() {
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        let alice = single_writer_guid();
        let rd = r.guid().entity_id;
        assert_eq!(
            r.handle_data(p1(), &data(alice.entity_id, rd, 1, 1), None)
                .len(),
            1
        );
        assert!(
            r.handle_data(p2(), &forwarded(1, alice, 1, 1), None)
                .is_empty()
        );
        assert!(
            r.handle_data(p2(), &forwarded(2, alice, 1, 1), None)
                .is_empty()
        );
        assert_eq!(r.forwarded_duplicate_count(), 2);
    }

    /// A change the original's proxy holds in its receive cache, behind a
    /// missing one, is delivered from there; the forwarded copy is dropped.
    #[test]
    fn a_forwarded_change_waiting_in_the_originals_cache_is_a_duplicate() {
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        let alice = single_writer_guid();
        let rd = r.guid().entity_id;
        assert!(
            r.handle_data(p1(), &data(alice.entity_id, rd, 2, 2), None)
                .is_empty()
        );
        assert!(
            r.handle_data(p2(), &forwarded(1, alice, 2, 2), None)
                .is_empty()
        );
        let out = r.handle_data(p1(), &data(alice.entity_id, rd, 1, 1), None);
        assert_eq!(
            sns(&out),
            alloc::vec![(alice, 1, None), (alice, 2, None)],
            "the original delivers both, once"
        );
    }

    /// A late joiner: the original made its first changes irrelevant for
    /// this reader with a GAP; forwarded, they are handed out (the
    /// durability service's case, §8.7.9).
    #[test]
    fn a_forwarded_change_the_original_gapped_is_handed_out() {
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        let alice = single_writer_guid();
        let rd = r.guid().entity_id;
        assert!(
            r.handle_gap(p1(), &gap(alice.entity_id, rd, 1, 3))
                .is_empty()
        );
        assert_eq!(
            r.handle_data(p1(), &data(alice.entity_id, rd, 3, 3), None)
                .len(),
            1
        );
        let out = r.handle_data(p2(), &forwarded(1, alice, 1, 1), None);
        assert_eq!(
            sns(&out),
            alloc::vec![(alice, 1, Some(second_writer_guid()))]
        );
        assert!(
            r.handle_data(p2(), &forwarded(2, alice, 3, 3), None)
                .is_empty()
        );
    }

    /// Without a proxy for the original the forwarded changes are
    /// remembered; a proxy added later does not deliver them again.
    #[test]
    fn changes_forwarded_before_the_original_matched_are_not_delivered_again() {
        let alice = single_writer_guid();
        let reader_guid = Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
        );
        let mut r = ReliableReader::new(ReliableReaderConfig {
            guid: reader_guid,
            vendor_id: VendorId::ZERODDS,
            writer_proxies: alloc::vec![],
            max_samples_per_proxy: 10,
            heartbeat_response_delay: Duration::from_millis(200),
            assembler_caps: AssemblerCaps::default(),
        });
        add_second_writer(&mut r);
        let rd = r.guid().entity_id;
        assert_eq!(
            r.handle_data(p2(), &forwarded(1, alice, 1, 1), None).len(),
            1
        );
        assert!(
            r.handle_data(p2(), &forwarded(2, alice, 1, 1), None)
                .is_empty()
        );
        r.add_writer_proxy(WriterProxy::new(
            alice,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
            alloc::vec![],
            true,
        ));
        assert!(
            r.handle_data(p1(), &data(alice.entity_id, rd, 1, 1), None)
                .is_empty()
        );
        assert_eq!(
            r.handle_data(p1(), &data(alice.entity_id, rd, 2, 2), None)
                .len(),
            1
        );
    }

    /// A writer of the same GUID that comes back after its proxy went is a
    /// new message stream: its sequence numbers start anew and are no
    /// duplicates of the old ones, forwarded or not.
    #[test]
    fn a_writer_that_comes_back_starts_a_new_stream() {
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        let alice = single_writer_guid();
        let rd = r.guid().entity_id;
        assert_eq!(
            r.handle_data(p1(), &data(alice.entity_id, rd, 1, 1), None)
                .len(),
            1
        );
        r.remove_writer_proxy(alice);
        assert!(
            r.handle_data(p2(), &forwarded(1, alice, 1, 1), None).len() <= 1,
            "a forwarded old change, once at most"
        );
        r.add_writer_proxy(WriterProxy::new(
            alice,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
            alloc::vec![],
            true,
        ));
        let out = r.handle_data(p1(), &data(alice.entity_id, rd, 1, 9), None);
        assert_eq!(out.len(), 1, "the new stream's first change is delivered");
        assert_eq!(out[0].payload.as_ref(), &[9]);
    }

    /// A writer whose forwarding is not honoured hands its change out as
    /// its own (DDS-Security 1.2 §7.1.1.4: only a participant allowed to
    /// relay).
    #[test]
    fn an_untrusted_forwarder_delivers_as_itself() {
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        let alice = single_writer_guid();
        let trent = second_writer_guid();
        r.set_forwarder_trusted(trent, false);
        let out = r.handle_data(p2(), &forwarded(1, alice, 7, 1), None);
        assert_eq!(sns(&out), alloc::vec![(trent, 1, None)]);
        assert_eq!(r.untrusted_forward_count(), 1);
    }

    /// DDSI-RTPS 2.5 §8.7.7: a directed sample reaches its target readers,
    /// every one of the PID_DIRECTED_WRITE parameters; another reader
    /// absorbs it as after a GAP, and the next sample is in order.
    #[test]
    fn a_directed_sample_is_absorbed_by_a_reader_it_does_not_name() {
        let mut r = make_reader(10);
        let alice = single_writer_guid();
        let rd = r.guid().entity_id;
        let directed = |n: i64, targets: &[[u8; 16]]| {
            let mut d = data(alice.entity_id, rd, n, n as u8);
            let mut pl = crate::parameter_list::ParameterList::new();
            for t in targets {
                pl.push(crate::inline_qos::directed_write_param(*t));
            }
            d.inline_qos = Some(pl);
            d
        };
        let other = [0x77; 16];
        assert!(r.handle_data(p1(), &directed(1, &[other]), None).is_empty());
        assert!(first_state(&r).proxy.is_known(sn(1)), "acknowledged");
        let own = r.guid().to_bytes();
        assert_eq!(
            r.handle_data(p1(), &directed(2, &[other, own]), None).len(),
            1
        );
        assert_eq!(
            r.handle_data(p1(), &data(alice.entity_id, rd, 3, 3), None)
                .len(),
            1
        );
    }

    /// The ranges merge, split and stay bounded.
    #[test]
    fn sequence_number_ranges_merge_and_stay_bounded() {
        let mut set = SnRanges::default();
        for sn in [1, 2, 3, 7, 5, 6] {
            assert!(set.insert(sn));
        }
        assert_eq!(set.ranges, alloc::vec![(1, 3), (5, 7)]);
        assert!(!set.insert(6));
        assert!(set.insert(4));
        assert_eq!(set.ranges, alloc::vec![(1, 7)]);
        let mut sparse = SnRanges::default();
        for k in 0..(SnRanges::MAX_RANGES as i64 + 10) {
            sparse.insert(k * 2);
        }
        assert_eq!(sparse.ranges.len(), SnRanges::MAX_RANGES);
        assert_eq!(sparse.ranges[0], (20, 20), "the lowest are forgotten");
    }
}
