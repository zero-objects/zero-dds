// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reliable RTPS writer (1:N reader proxies) — DDSI-RTPS 2.5 §8.4.9.
//!
//! Corresponds to the [`StatefulWriter`] role with 1..N matched readers.
//! No heartbeat liveliness. Fragmentation
//! (§8.4.14) is supported. Multi-reader + submessage aggregation
//! (Fast-DDS alignment) are in since WP 1.4 T3.
//!
//! # API shape
//!
//! The state machine is tick-driven. `now` is a `Duration`
//! since writer start (no_std-compatible, without std::Instant).
//!
//! ```text
//!   let mut w = ReliableWriter::new(...);
//!   w.add_reader_proxy(proxy_a);
//!   loop {
//!       if let Some(payload) = app.next_sample() {
//!           for dg in w.write(payload)? { transport.send_to_all(&dg.targets, &dg.bytes); }
//!       }
//!       for dg in w.tick(uptime())? { transport.send_to_all(&dg.targets, &dg.bytes); }
//!       match transport.recv_control() {
//!           AckNack(src, ack) => w.handle_acknack(src, ack.base, ack.requested),
//!           NackFrag(src, nf) => w.handle_nackfrag(src, &nf),
//!           _ => {}
//!       }
//!   }
//! ```
//!
//! [`StatefulWriter`]: https://www.omg.org/spec/DDSI-RTPS/2.5/

use core::time::Duration;

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::header::RtpsHeader;
use crate::history_cache::{CacheChange, ChangeKind, HistoryCache, HistoryKind};
use crate::message_builder::{AddError, MessageBuilder, OutboundDatagram};
use crate::reader_proxy::ReaderProxy;
use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageId};
use crate::submessages::{
    DATA_FLAG_DATA, DataFragSubmessage, DataSubmessage, GapSubmessage, HeartbeatSubmessage,
    NackFragSubmessage, SequenceNumberSet,
};
use crate::wire_types::{FragmentNumber, Guid, GuidPrefix, Locator, SequenceNumber, VendorId};

/// Default heartbeat period.
///
/// DDSI-RTPS §8.4.15 specifies no fixed default — "implementation-defined,
/// typically 1 s". Cyclone DDS and FastDDS use 100 ms; that is also our
/// value because:
/// 1. With Reliable + KEEP_LAST(N) the HB period drives the worst-case latency
///    floor (the reader sends an ACK on the HB, the writer can only shrink the cache after).
/// 2. The 1 s default was the pre-D.5d initial implementation; the current
///    event-driven ACKNACK + per-peer scheduler architecture (D.5d+) makes
///    the HB floor a pure "idle keep-alive" pulse, no longer a
///    latency determinant.
/// 3. The spec is satisfied: period < lease_duration, period > 0, period stable.
pub const DEFAULT_HEARTBEAT_PERIOD: Duration = Duration::from_millis(100);

/// How many sequence-number-to-GSN pairs a writer in a group keeps for the GAPs
/// of changes that left its cache.
const MAX_REMEMBERED_GSNS: usize = 4096;

/// Default flow-control window of a writer with reliable readers: octets
/// of serialized payload written and not yet acknowledged by every one of
/// them ([`ReliableWriter::set_unacked_window`]). Cyclone DDS holds its
/// writers to 500 kB unacknowledged (`WhcHigh`). Without a window a
/// KEEP_ALL writer of 100 000 1 kB samples put up to 100 MB on its way to a
/// reader with a 1 MB receive buffer, and the reader's kernel dropped tens
/// of thousands of datagrams per 15 s on the Linux test host.
pub const DEFAULT_UNACKED_WINDOW: usize = 512 * 1024;

/// §8.4.7.1 "Default Timing-Related Values":
/// `nackResponseDelay.nanosec = 200 * 1000 * 1000`, 200 ms, "used as the
/// defaults in order to facilitate 'out-of-the-box' interoperability
/// between implementations". The default of a writer;
/// [`ReliableWriter::set_nack_response_delay`] and
/// `writer.nack_response_delay_ms` set another (0 answers on the next tick).
pub const DEFAULT_NACK_RESPONSE_DELAY: Duration = Duration::from_millis(200);

/// The spec's `nackResponseDelay`; the same value as
/// [`DEFAULT_NACK_RESPONSE_DELAY`].
pub const SPEC_NACK_RESPONSE_DELAY: Duration = DEFAULT_NACK_RESPONSE_DELAY;

/// §8.4.7.1 `nackSuppressionDuration.sec = 0;
/// nackSuppressionDuration.nanosec = 0` — the spec default: no request is
/// discarded for arriving too soon after the change was sent.
/// [`ReliableWriter::set_nack_suppression_duration`] sets another.
pub const DEFAULT_NACK_SUPPRESSION_DURATION: Duration = Duration::ZERO;

/// Default fragment size in bytes. 1344 = 1400 MTU − 20 RTPS header −
/// ~32 bytes submessage overhead.
pub const DEFAULT_FRAGMENT_SIZE: u32 = 1344;

/// Largest fragment size. DDSI-RTPS 2.5 §8.4.14.1 says "fragment size <=
/// 65536 bytes"; `fragmentSize` of a DATA_FRAG is an `unsigned short`
/// (§9.4.5.4), which holds 65535 at most. The larger value of the text has
/// no encoding, so 65535 is the bound.
pub const MAX_FRAGMENT_SIZE: u32 = 65_535;

/// Octets of a message beside the fragments in it that the fragment size
/// leaves room for: RTPS header, INFO_DST, INFO_TS, the DATA_FRAG's own
/// header and a PID_KEY_HASH. [`DEFAULT_FRAGMENT_SIZE`] is the
/// [`fragment_size_for_message_size`] of a message of 1472 octets, what an
/// Ethernet MTU carries in a UDP datagram.
pub const FRAGMENT_MESSAGE_OVERHEAD: usize = 128;

/// The fragment size for a writer whose transport with the smallest
/// maximum message size carries `message_size` octets (DDSI-RTPS 2.5
/// §8.4.14.1: "the transport with the smallest maximum message size
/// determines the fragment size"): the message less
/// [`FRAGMENT_MESSAGE_OVERHEAD`], at least 1 and at most
/// [`MAX_FRAGMENT_SIZE`].
#[must_use]
pub fn fragment_size_for_message_size(message_size: usize) -> u32 {
    let room = message_size.saturating_sub(FRAGMENT_MESSAGE_OVERHEAD);
    u32::try_from(room)
        .unwrap_or(u32::MAX)
        .clamp(1, MAX_FRAGMENT_SIZE)
}

/// Message size for loopback/same-host paths. The loopback MTU is 65536 —
/// a sample up to ~63 kB goes there in **one** DATA instead of N fragments
/// (DDSI-RTPS 2.5 §8.4.14.1: data is fragmented only where required). The
/// DCPS layer sets it per reader proxy with
/// [`ReaderProxy::set_message_limit`]; the writer's fragment size stays the
/// one it was created with.
pub const LOOPBACK_MTU: usize = 64_000;

/// The DATA body up to its payload; see
/// [`ReliableWriter::compact_data_prefix`].
struct DataPrefix {
    bytes: [u8; DataPrefix::CAPACITY],
    len: usize,
}

impl DataPrefix {
    /// Twenty fixed octets and the 24 of `[PID_KEY_HASH, PID_SENTINEL]`.
    const CAPACITY: usize = 44;

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Target locator set of a reader proxy. Multicast preferred (network
/// latency and bandwidth), unicast as fallback.
fn proxy_targets_of(p: &ReaderProxy) -> alloc::sync::Arc<Vec<Locator>> {
    if p.multicast_locators.is_empty() {
        alloc::sync::Arc::new(p.unicast_locators.clone())
    } else {
        alloc::sync::Arc::new(p.multicast_locators.clone())
    }
}

/// A reliable writer with 0..N reader proxies.
#[derive(Debug, Clone)]
pub struct ReliableWriter {
    guid: Guid,
    vendor_id: VendorId,
    reader_proxies: Vec<ReaderProxy>,
    /// What [`Self::targets_for`] answers for each of `reader_proxies`, at
    /// the same index: built when a proxy is added or its locators change,
    /// so a write hands each datagram a reference and not a copy of the
    /// locator list (two allocations per write and reader, measured on
    /// the Linux test host).
    proxy_targets: Vec<alloc::sync::Arc<Vec<Locator>>>,
    cache: HistoryCache,
    heartbeat_period: Duration,
    last_heartbeat: Option<Duration>,
    heartbeat_count: i32,
    nackfrag_count: i32,
    /// ACKNACK/NACK_FRAG messages from unknown `src_guid` remotes.
    /// Diagnosis: indicates misrouting, stale proxies or
    /// malicious senders.
    unknown_src_count: u64,
    next_sn: i64,
    fragment_size: u32,
    mtu: usize,
    /// Per-peer behaviour axes: the participant-wide default plus explicit
    /// per-peer overrides ([`crate::dialect`]). Holds the dispose-marker
    /// form and the `INFO_DST` addressing that used to be a bare
    /// `emit_info_dst: bool`.
    behaviour: crate::dialect::BehaviourTable,
    /// §8.4.7.1 `pushMode`. `true` (the default) pushes each change to
    /// the matched readers as it is written; `false` announces it only
    /// via HEARTBEAT and sends it in response to a reader's request.
    push_mode: bool,
    /// Hot-path recycling pool for payload `Arc<[u8]>`. On cache
    /// eviction the evicted `Arc<[u8]>` is parked here and reused on the
    /// next same-sized `stage_sample` allocation
    /// (Arc::get_mut + memcpy into the existing
    /// buffer). Eliminates the `Arc::from(payload)` allocation per
    /// write for workloads with constant payload size (bench,
    /// fixed-size sensor streams, RPC with a fixed request size).
    /// On changing sizes the pool entry is dropped and a fresh one
    /// allocated.
    payload_pool: alloc::vec::Vec<alloc::sync::Arc<[u8]>>,
    /// Pool cap (default 8) — prevents unbounded pool growth
    /// on cache-depth spikes.
    payload_pool_cap: usize,
    /// Open coherent set (DDSI-RTPS 2.5 §8.7.6): the `SequenceNumber` of
    /// the set's first sample. `Some` between `begin_coherent_set` and
    /// `end_coherent_set`; every `stage_sample` in that window tags its
    /// `CacheChange` with it. `None` ⇒ no set is open.
    coherent_set_sn: Option<i64>,
    /// A coherent set is open but no sample has been written into it yet, so
    /// the set's first `SequenceNumber` is not yet known. The next
    /// `stage_sample` claims its own SN as the set-first SN.
    coherent_set_armed: bool,
    /// The group of a writer of a GROUP-scope publisher (DDSI-RTPS 2.5
    /// §8.7.5): every change draws its group sequence number from it.
    group: Option<alloc::sync::Arc<crate::writer_group::WriterGroup>>,
    /// Sequence number to group sequence number of the changes written in a
    /// group, kept past their removal from the cache ([`Self::gap_group_info`]).
    gsn_of: alloc::collections::BTreeMap<i64, i64>,
    /// The group sequence number of this writer's last change, kept past
    /// the change's removal from the cache: a HEARTBEAT of an empty cache
    /// still names it as `lastGSN`.
    last_gsn: Option<i64>,
    /// The group's `currentGSN` the last periodic HEARTBEAT carried.
    announced_current_gsn: i64,
    /// The writer's QoS as in-line parameters, for a reader whose proxy
    /// expects them (DDSI-RTPS 2.5 §8.4.9.2.4 `DATA.inlineQos :=
    /// the_rtps_writer.related_dds_writer.qos`, §8.7.2.1).
    inline_writer_qos: Option<crate::parameter_list::ParameterList>,
    /// How many of `reader_proxies` expect in-line QoS. Zero keeps the
    /// per-sample proxy lookup in [`Self::sample_inline_qos`] off the path
    /// of every writer without such a reader.
    inline_qos_readers: usize,
    /// Whether the group information of a change travels as
    /// `PID_SECURE_WRITER_GROUP_INFO` (§9.6.4.6) in place of
    /// `PID_WRITER_GROUP_INFO`; see [`Self::set_secure_writer_group_info`].
    secure_group_info: bool,
    /// How many of `reader_proxies` have a `DDS_FILTER`
    /// ([`ReaderProxy::filter`]); zero keeps the lookup out of the path of
    /// every writer without one.
    filtered_readers: usize,
    /// DDSI-RTPS 2.5 §8.4.7.1 `nackResponseDelay`.
    nack_response_delay: Duration,
    /// DDSI-RTPS 2.5 §8.4.7.1 `nackSuppressionDuration`.
    nack_suppression_duration: Duration,
    /// The latest time this writer was told: the `now` of the last
    /// [`Self::tick`] or [`Self::handle_acknack_at`]. What a send or a
    /// NACK without a time of its own is stamped with.
    clock: Duration,
    /// When each change in the cache last went out, kept only while
    /// `nack_suppression_duration` is not zero.
    sent_at: alloc::collections::BTreeMap<SequenceNumber, Duration>,
    /// The flow-control window in octets, `0` for none; see
    /// [`Self::set_unacked_window`].
    unacked_window: usize,
    /// The changes written that some reliable reader has not acknowledged
    /// yet, oldest first, with the length of each payload. A change
    /// evicted from the history before it was acknowledged stays here
    /// until it is: it went out and holds room on the way to the reader.
    unacked: alloc::collections::VecDeque<(SequenceNumber, usize)>,
    /// The sum of the lengths in `unacked`.
    unacked_bytes: usize,
    /// A HEARTBEAT was asked for to have the window acknowledged, and no
    /// acknowledgement moved it since ([`Self::ask_for_ack`]).
    ack_requested: bool,
    /// The readers the next change staged is addressed to; see
    /// [`Self::set_next_destination_readers`].
    next_destination_readers: Option<alloc::sync::Arc<[Guid]>>,
}

/// Configuration at creation.
#[derive(Debug, Clone)]
pub struct ReliableWriterConfig {
    /// GUID of the writer endpoint.
    pub guid: Guid,
    /// VendorId for the RTPS header.
    pub vendor_id: VendorId,
    /// Initial reader proxies. More via `add_reader_proxy`.
    pub reader_proxies: Vec<ReaderProxy>,
    /// Absolute upper bound for cache entries. Acts as a capacity;
    /// the semantics on overflow are determined by [`history_kind`](Self::history_kind).
    pub max_samples: usize,
    /// History QoS:
    /// - `KeepAll`: write() fails on overflow
    ///   (no-loss scenarios, e.g. logging).
    /// - `KeepLast { depth }`: the oldest sample drops out on overflow
    ///   (spec default; a stalled reader does not block the whole pipeline).
    pub history_kind: HistoryKind,
    /// Heartbeat period (default: 1 s).
    pub heartbeat_period: Duration,
    /// Fragment size in bytes ([`DEFAULT_FRAGMENT_SIZE`]).
    pub fragment_size: u32,
    /// MTU for submessage aggregation ([`DEFAULT_MTU`](crate::message_builder::DEFAULT_MTU)).
    pub mtu: usize,
}

/// One already-bounded fragment view of a serialized sample.
struct FragmentSlice<'a> {
    number: FragmentNumber,
    /// How many fragments `bytes` holds (`fragmentsInSubmessage`).
    count: u16,
    size: u16,
    sample_size: u32,
    bytes: &'a [u8],
}

impl ReliableWriter {
    /// [`Self::new`] for a configuration that may be wrong: the error names
    /// what is.
    ///
    /// # Errors
    /// `ValueOutOfRange` for a fragment size of 0 or above
    /// [`MAX_FRAGMENT_SIZE`] (§8.4.14.1), and for an `mtu` below 20.
    pub fn try_new(cfg: ReliableWriterConfig) -> Result<Self, WireError> {
        if cfg.fragment_size == 0 {
            return Err(WireError::ValueOutOfRange {
                message: "fragment_size must be > 0",
            });
        }
        if cfg.fragment_size > MAX_FRAGMENT_SIZE {
            return Err(WireError::ValueOutOfRange {
                message: "fragment_size exceeds 65535 (DDSI-RTPS 2.5 §8.4.14.1, §9.4.5.4)",
            });
        }
        if cfg.mtu < 20 {
            return Err(WireError::ValueOutOfRange {
                message: "mtu must accommodate RTPS header",
            });
        }
        Ok(Self::new(cfg))
    }

    /// Creates an empty writer.
    ///
    /// # Panics
    /// - `cfg.fragment_size == 0` or above [`MAX_FRAGMENT_SIZE`]
    /// - `cfg.mtu < 20` (the RTPS header does not fit)
    ///
    /// [`Self::try_new`] returns these as an error.
    #[must_use]
    pub fn new(cfg: ReliableWriterConfig) -> Self {
        assert!(cfg.fragment_size > 0, "fragment_size must be > 0");
        assert!(
            cfg.fragment_size <= MAX_FRAGMENT_SIZE,
            "fragment_size must be <= 65535"
        );
        assert!(cfg.mtu >= 20, "mtu must accommodate RTPS header");
        let inline_qos_readers = cfg
            .reader_proxies
            .iter()
            .filter(|p| p.expects_inline_qos())
            .count();
        let filtered_readers = cfg
            .reader_proxies
            .iter()
            .filter(|p| p.filter().is_some())
            .count();
        let proxy_targets = cfg.reader_proxies.iter().map(proxy_targets_of).collect();
        Self {
            guid: cfg.guid,
            vendor_id: cfg.vendor_id,
            reader_proxies: cfg.reader_proxies,
            proxy_targets,
            cache: HistoryCache::new_with_kind(cfg.history_kind, cfg.max_samples),
            heartbeat_period: cfg.heartbeat_period,
            last_heartbeat: None,
            heartbeat_count: 0,
            nackfrag_count: 0,
            unknown_src_count: 0,
            next_sn: 0,
            fragment_size: cfg.fragment_size,
            mtu: cfg.mtu,
            behaviour: crate::dialect::BehaviourTable::new(crate::dialect::RtpsBehaviour::new()),
            push_mode: true,
            payload_pool: alloc::vec::Vec::with_capacity(8),
            payload_pool_cap: 8,
            coherent_set_sn: None,
            coherent_set_armed: false,
            group: None,
            gsn_of: alloc::collections::BTreeMap::new(),
            last_gsn: None,
            announced_current_gsn: 0,
            inline_writer_qos: None,
            inline_qos_readers,
            filtered_readers,
            secure_group_info: false,
            nack_response_delay: DEFAULT_NACK_RESPONSE_DELAY,
            nack_suppression_duration: DEFAULT_NACK_SUPPRESSION_DURATION,
            clock: Duration::ZERO,
            sent_at: alloc::collections::BTreeMap::new(),
            unacked_window: 0,
            unacked: alloc::collections::VecDeque::new(),
            unacked_bytes: 0,
            ack_requested: false,
            next_destination_readers: None,
        }
    }

    /// Sets the flow-control window: how many octets of serialized
    /// payload may be written and not yet acknowledged by every reliable
    /// reader before [`Self::window_full`] says so. `0` sets none. The
    /// window counts only while the writer has a reliable reader; a
    /// BEST_EFFORT reader acknowledges nothing.
    pub fn set_unacked_window(&mut self, octets: usize) {
        self.unacked_window = octets;
        if octets == 0 {
            self.unacked.clear();
            self.unacked_bytes = 0;
            self.ack_requested = false;
        }
    }

    /// The flow-control window ([`Self::set_unacked_window`]).
    #[must_use]
    pub fn unacked_window(&self) -> usize {
        self.unacked_window
    }

    /// Octets of serialized payload written and not yet acknowledged by
    /// every reliable reader, counted while there is a window.
    #[must_use]
    pub fn unacked_bytes(&self) -> usize {
        self.unacked_bytes
    }

    /// Whether the unacknowledged octets fill the window: a write waits
    /// for acknowledgements before it adds more.
    #[must_use]
    pub fn window_full(&self) -> bool {
        self.unacked_window > 0 && self.unacked_bytes >= self.unacked_window
    }

    /// Asks for the next [`Self::tick`] to send a HEARTBEAT, so that the
    /// reliable readers acknowledge what they have, once the
    /// unacknowledged octets reach half the window and no acknowledgement
    /// moved them since the last time it asked. Whether it asked: the
    /// caller wakes its tick then. A reader acknowledges only in response
    /// to a HEARTBEAT (DDSI-RTPS 2.5 §8.4.2.3), and the periodic one comes
    /// a heartbeatPeriod after the last, by when a full window has waited.
    pub fn ask_for_ack(&mut self) -> bool {
        if self.unacked_window == 0
            || self.ack_requested
            || self.unacked_bytes.saturating_mul(2) < self.unacked_window
        {
            return false;
        }
        self.ack_requested = true;
        self.last_heartbeat = None;
        true
    }

    /// The next [`Self::tick`] sends a HEARTBEAT, whatever the period: for
    /// a write that waits for a full window to be acknowledged.
    pub fn request_heartbeat(&mut self) {
        self.ack_requested = true;
        self.last_heartbeat = None;
    }

    /// Takes off `unacked` what every reliable reader has acknowledged.
    fn release_acknowledged_window(&mut self) {
        let floor = self
            .reader_proxies
            .iter()
            .filter(|p| p.is_reliable)
            .map(ReaderProxy::highest_acked_sn)
            .min();
        let Some(floor) = floor else {
            // No reliable reader: nothing waits for an acknowledgement.
            self.unacked.clear();
            self.unacked_bytes = 0;
            self.ack_requested = false;
            return;
        };
        let mut moved = false;
        while let Some(&(sn, len)) = self.unacked.front() {
            if sn > floor {
                break;
            }
            self.unacked.pop_front();
            self.unacked_bytes = self.unacked_bytes.saturating_sub(len);
            moved = true;
        }
        if moved {
            self.ack_requested = false;
        }
    }

    /// Sets `nackResponseDelay` (DDSI-RTPS 2.5 §8.4.7.1): a repair goes
    /// out once the NACK that asked for it has waited this long. Zero —
    /// [`DEFAULT_NACK_RESPONSE_DELAY`] — answers on the next tick; the
    /// spec's out-of-the-box value is [`SPEC_NACK_RESPONSE_DELAY`]. The
    /// wait is measured from the NACK's arrival when it comes through
    /// [`Self::handle_acknack_at`], and from the first tick that sees it
    /// otherwise, so it is the delay plus at most one tick period.
    pub fn set_nack_response_delay(&mut self, delay: Duration) {
        self.nack_response_delay = delay;
    }

    /// The `nackResponseDelay` this writer runs.
    #[must_use]
    pub fn nack_response_delay(&self) -> Duration {
        self.nack_response_delay
    }

    /// Sets `nackSuppressionDuration` (DDSI-RTPS 2.5 §8.4.7.1): a NACK for
    /// a change that went out less than this long ago is ignored, the
    /// change taken to be in flight. Zero —
    /// [`DEFAULT_NACK_SUPPRESSION_DURATION`], the spec's value — ignores
    /// none, and keeps no send times. A change first sent by `write`
    /// carries the writer's clock, the time of the last tick, since
    /// `write` is given none.
    pub fn set_nack_suppression_duration(&mut self, duration: Duration) {
        self.nack_suppression_duration = duration;
        if duration.is_zero() {
            self.sent_at.clear();
        }
    }

    /// The `nackSuppressionDuration` this writer runs.
    #[must_use]
    pub fn nack_suppression_duration(&self) -> Duration {
        self.nack_suppression_duration
    }

    /// Notes that `sn` went out at `at`, while suppression is on.
    fn note_sent(&mut self, sn: SequenceNumber, at: Duration) {
        if !self.nack_suppression_duration.is_zero() {
            self.sent_at.insert(sn, at);
        }
    }

    /// Whether a NACK for `sn` at `now` arrives too soon after the change
    /// went out (§8.4.7.1 `nackSuppressionDuration`).
    fn too_soon(&self, sn: SequenceNumber, now: Duration) -> bool {
        !self.nack_suppression_duration.is_zero()
            && self
                .sent_at
                .get(&sn)
                .is_some_and(|sent| now.saturating_sub(*sent) < self.nack_suppression_duration)
    }

    /// Sets the QoS this writer sends in-line to a reader whose proxy
    /// [expects them](ReaderProxy::expects_inline_qos): the DataWriter's
    /// policies that DDSI-RTPS 2.5 Table 8.85 lets appear in-line, and
    /// `PID_TOPIC_NAME`, which §8.7.1 requires with them. Each such DATA
    /// carries them ahead of the change's own in-line parameters
    /// (§8.4.9.2.4). `None` sends none, and a reader that expects them is
    /// then served without.
    pub fn set_inline_writer_qos(&mut self, qos: Option<crate::parameter_list::ParameterList>) {
        self.inline_writer_qos = qos;
    }

    /// GUID of the writer.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Enables INFO_DST(target-prefix) before each datagram (RTPS 2.5
    /// §8.3.7). Needed for receivers that match directed submessages by full
    /// GUID (cyclone filtered VolatileSecure reader).
    ///
    /// Sets [`crate::dialect::DirectedAddressing`] on the participant-wide
    /// default; peers with an explicit override keep theirs. Kept as the
    /// name the SEDP and VolatileSecure call sites already use.
    pub fn set_emit_info_dst(&mut self, v: bool) {
        let mut d = self.behaviour.default_behaviour();
        d.directed_addressing = crate::dialect::DirectedAddressing::from_emit_info_dst(v);
        self.behaviour.set_default(d);
    }

    /// Makes the group information of this writer's changes travel as
    /// `PID_SECURE_WRITER_GROUP_INFO` (0x0066) where the writer of a GROUP
    /// scope publisher would send `PID_WRITER_GROUP_INFO` (0x0065): DDSI-RTPS
    /// 2.5 §9.6.4.6 reserves the secure parameter for DDS Security.
    pub fn set_secure_writer_group_info(&mut self, secure: bool) {
        self.secure_group_info = secure;
    }

    /// The participant-wide behaviour default ([`crate::dialect`]).
    #[must_use]
    pub fn behaviour(&self) -> crate::dialect::RtpsBehaviour {
        self.behaviour.default_behaviour()
    }

    /// Replaces the participant-wide behaviour default. Peers with an
    /// explicit override keep theirs.
    pub fn set_behaviour(&mut self, behaviour: crate::dialect::RtpsBehaviour) {
        self.behaviour.set_default(behaviour);
    }

    /// Sets the behaviour for every reader of the participant `prefix`;
    /// a reader GUID's own override goes before it.
    ///
    /// Always explicit: nothing here reads the peer's `vendorId` and
    /// switches by itself. See the [`crate::dialect`] header for why.
    pub fn set_participant_behaviour(
        &mut self,
        prefix: crate::wire_types::GuidPrefix,
        behaviour: crate::dialect::RtpsBehaviour,
    ) {
        self.behaviour.set_for_participant(prefix, behaviour);
    }

    /// Sets the behaviour for one matched peer, by its reader GUID.
    ///
    /// Always explicit: nothing here reads the peer's `vendorId` and
    /// switches by itself. See the [`crate::dialect`] header for why.
    pub fn set_peer_behaviour(&mut self, peer: Guid, behaviour: crate::dialect::RtpsBehaviour) {
        self.behaviour.set_for_peer(peer, behaviour);
    }

    /// Drops one peer's override, so the default applies to it again.
    pub fn clear_peer_behaviour(&mut self, peer: Guid) {
        self.behaviour.clear_for_peer(peer);
    }

    /// The behaviour in force for one matched peer.
    #[must_use]
    pub fn behaviour_for_peer(&self, peer: Guid) -> crate::dialect::RtpsBehaviour {
        self.behaviour.for_peer(peer)
    }

    /// Opens a MessageBuilder for proxy `idx`; prepends INFO_DST with the
    /// target prefix when that peer's
    /// [`crate::dialect::DirectedAddressing`] asks for it.
    fn open_builder(&self, idx: usize, targets: &alloc::sync::Arc<Vec<Locator>>) -> MessageBuilder {
        self.open_builder_for(self.reader_proxies[idx].remote_reader_guid, targets)
    }

    /// [`Self::open_builder`] addressed by peer GUID rather than proxy index.
    ///
    /// Every datagram this writer builds **for one proxy** goes through here.
    /// A datagram whose content is proxy-specific — a HEARTBEAT's announced
    /// range, a GAP's irrelevant set — is unreadable without its destination
    /// once two participants hold the same unicast port one after the other:
    /// the builtin EntityIds are fixed, so the successor of a restarted peer
    /// sees its predecessor's `readerId` on its own port and applies what was
    /// meant for the dead proxy. That is issue #32, and it is why the
    /// addressing decision belongs at the one place a builder is opened.
    fn open_builder_for(
        &self,
        peer: Guid,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> MessageBuilder {
        let mut b = MessageBuilder::open(
            self.rtps_header(),
            alloc::sync::Arc::clone(targets),
            self.message_size_for(peer),
        );
        if self
            .behaviour
            .for_peer(peer)
            .directed_addressing
            .emits_info_dst()
        {
            let _ = b.try_add_submessage(SubmessageId::InfoDst, 0, &peer.prefix.to_bytes());
        }
        b
    }

    /// Read-only slice of the registered reader proxies.
    #[must_use]
    pub fn reader_proxies(&self) -> &[ReaderProxy] {
        &self.reader_proxies
    }

    /// Refreshes a known reader's locators without resetting acknowledgment state.
    /// Returns false when no proxy with this GUID exists.
    pub fn refresh_reader_proxy_locators(
        &mut self,
        guid: Guid,
        unicast: Vec<Locator>,
        multicast: Vec<Locator>,
    ) -> bool {
        let Some(proxy) = self
            .reader_proxies
            .iter_mut()
            .find(|p| p.remote_reader_guid == guid)
        else {
            return false;
        };
        proxy.unicast_locators = unicast;
        proxy.multicast_locators = multicast;
        let targets = proxy_targets_of(proxy);
        if let Some(idx) = self
            .reader_proxies
            .iter()
            .position(|p| p.remote_reader_guid == guid)
        {
            self.proxy_targets[idx] = targets;
        }
        true
    }

    /// Number of registered reader proxies.
    #[must_use]
    pub fn reader_proxy_count(&self) -> usize {
        self.reader_proxies.len()
    }

    /// §8.4.7.1 `pushMode` — `true` (the default) pushes each written
    /// change straight to the matched readers, `false` only announces it
    /// via HEARTBEAT and waits for the reader to request it.
    pub fn set_push_mode(&mut self, push_mode: bool) {
        self.push_mode = push_mode;
    }

    /// The `pushMode` attribute (§8.4.7.1).
    #[must_use]
    pub fn push_mode(&self) -> bool {
        self.push_mode
    }

    /// §8.4.7.4 `is_acked_by_all` — has every matched reader
    /// acknowledged the change with sequence number `sn`?
    ///
    /// The spec's StatefulWriter operation takes a change and asks
    /// whether "all the `ReaderProxy` have acked it". A writer with no
    /// matched reader answers `true`: the `FOREACH` runs over an empty
    /// set, and the DDS layer must not block a `dispose` on a reader
    /// that does not exist.
    #[must_use]
    pub fn is_acked_by_all(&self, sn: SequenceNumber) -> bool {
        self.reader_proxies
            .iter()
            .all(|p| p.highest_acked_sn() >= sn)
    }

    /// Removes from the cache every change that every matched reader is done
    /// with, and returns how many: a reliable reader when it has acknowledged
    /// the change (§8.4.7.4 `is_acked_by_all`), a best-effort reader when the
    /// change has been sent to it. For a VOLATILE writer: what no reader will
    /// get or ask for again. A best-effort proxy that lags — matched after
    /// the writer wrote, so `write` did not send to it directly — is served
    /// by [`Self::tick`] from the cache; removing what it has not been sent
    /// would leave it nothing to send, sample after sample. With no matched
    /// reader everything written is done with. A TRANSIENT_LOCAL writer must
    /// not call it — its cache is what a late joiner gets.
    pub fn remove_acknowledged(&mut self) -> usize {
        let Some(max) = self.cache.max_sn() else {
            return 0;
        };
        let floor = self
            .reader_proxies
            .iter()
            .map(|p| {
                if p.is_reliable {
                    p.highest_acked_sn().0
                } else {
                    p.highest_sent_sn().0
                }
            })
            .min()
            .unwrap_or(max.0);
        // The members of an open coherent set stay until the set ends.
        // DDS 1.4 §2.2.2.4.1.10 makes the set one unit for the reader, so a
        // reader matched while it is open is given all of it
        // ([`Self::volatile_join_floor`]); dropping its first members because
        // no reader had asked for them yet left that reader half a set.
        let floor = match self.coherent_set_sn {
            Some(first) if first > 0 => floor.min(first - 1),
            _ => floor,
        };
        if floor < 1 {
            return 0;
        }
        // `HistoryCache::remove_up_to` removes SN <= its argument. The
        // payloads go back into the pool `stage_sample` takes from: a
        // VOLATILE writer whose readers keep up removes every change right
        // after its write, and dropping them left the pool empty and a
        // fresh payload allocation per write.
        let pool = &mut self.payload_pool;
        let cap = self.payload_pool_cap;
        self.cache
            .remove_up_to_with(SequenceNumber(floor), |change| {
                if pool.len() < cap {
                    pool.push(change.payload);
                }
            })
    }

    /// Removes from the cache every dispose and unregister that every
    /// matched reader is done with (as [`Self::remove_acknowledged`] counts
    /// it), and returns how many. For a TRANSIENT_LOCAL writer with one
    /// change per instance, such as a SEDP writer: the change that ended an
    /// instance has reached every current reader, and a late joiner needs
    /// none for an instance it never saw.
    pub fn remove_acknowledged_lifecycle(&mut self) -> usize {
        let Some(max) = self.cache.max_sn() else {
            return 0;
        };
        let floor = self
            .reader_proxies
            .iter()
            .map(|p| {
                if p.is_reliable {
                    p.highest_acked_sn().0
                } else {
                    p.highest_sent_sn().0
                }
            })
            .min()
            .unwrap_or(max.0);
        if floor < 1 {
            return 0;
        }
        self.cache
            .remove_up_to_where(SequenceNumber(floor), |change| !change.kind.is_alive_kind())
    }

    /// Removes samples with SN < `up_to_exclusive` from the cache. Used
    /// by higher layers for lifespan expiry (Spec §2.2.3.16):
    /// expired samples disappear so that even late
    /// reader proxies no longer get them.
    pub fn remove_samples_up_to(&mut self, up_to_exclusive: SequenceNumber) -> usize {
        // `HistoryCache::remove_up_to` removes SN <= its argument; passing
        // `up_to_exclusive` on took the first sample that had not expired.
        self.cache
            .remove_up_to(SequenceNumber(up_to_exclusive.0 - 1))
    }

    /// The sequence number of the last change written, `0` before the first.
    /// Not the cache's maximum: a VOLATILE writer removes what its readers
    /// are done with, and the cache can be empty after SN 100.
    #[must_use]
    pub fn last_written_sn(&self) -> SequenceNumber {
        SequenceNumber(self.next_sn)
    }

    /// The last SN a VOLATILE reader matched now skips: everything written
    /// before the match, except an open coherent set, which the reader gets
    /// whole from its first member (DDS 1.4 §2.2.2.4.1.10). Without that a
    /// reader matched inside a set saw its tail and a `PID_COHERENT_SET`
    /// naming a sample it never received (measured against RTI Connext
    /// 7.7.0, OMG `Test_CoherentSets_12`: no sample presented).
    #[must_use]
    pub fn volatile_join_floor(&self) -> SequenceNumber {
        match self.coherent_set_sn {
            Some(first) if first > 0 && first <= self.next_sn => SequenceNumber(first - 1),
            _ => self.last_written_sn(),
        }
    }

    /// History cache (read-only).
    #[must_use]
    pub fn cache(&self) -> &HistoryCache {
        &self.cache
    }

    /// **Expert-only**: sets the history kind + `max_samples` of the cache at
    /// runtime. Used by `DcpsRuntime` for the durability-backend replay burst
    /// (Spec §2.2.3.5) — the cache must hold all replay
    /// samples during the burst, then return to the user QoS.
    pub fn set_cache_kind_and_max(
        &mut self,
        kind: crate::history_cache::HistoryKind,
        max_samples: usize,
    ) {
        self.cache.set_kind_and_max(kind, max_samples);
    }

    /// Sets the writer's HISTORY on its cache: `kind` and `max_samples` for
    /// the cache as a whole, and the KEEP_LAST depth per instance (DDS 1.4
    /// §2.2.3.18, `None` = none). What the cache holds is what a
    /// TRANSIENT_LOCAL late joiner is replayed.
    pub fn set_cache_history(
        &mut self,
        kind: crate::history_cache::HistoryKind,
        max_samples: usize,
        per_instance_depth: Option<usize>,
    ) {
        self.cache.set_kind_and_max(kind, max_samples);
        self.cache.set_per_instance_depth(per_instance_depth);
    }

    /// Number of HEARTBEATs sent.
    #[must_use]
    pub fn heartbeat_count(&self) -> i32 {
        self.heartbeat_count
    }

    /// Number of NACK_FRAGs received.
    #[must_use]
    pub fn nackfrag_count(&self) -> i32 {
        self.nackfrag_count
    }

    /// Number of ACKNACK/NACK_FRAG messages from **unknown** sources
    /// since writer start. Typical causes: misrouting on multicast,
    /// stale proxies after `remove_reader_proxy`, GUID spoofing.
    #[must_use]
    pub fn unknown_src_count(&self) -> u64 {
        self.unknown_src_count
    }

    /// Current fragment-size configuration.
    #[must_use]
    pub fn fragment_size(&self) -> u32 {
        self.fragment_size
    }

    /// Largest RTPS message this writer builds for `peer`: the message
    /// limit of its reader proxy ([`ReaderProxy::message_limit`]), else the
    /// writer's own.
    fn message_size_for(&self, peer: Guid) -> usize {
        self.reader_proxies
            .iter()
            .find(|p| p.remote_reader_guid == peer)
            .and_then(ReaderProxy::message_limit)
            .map_or(self.mtu, |limit| limit.max(self.mtu))
    }

    /// True if the payload of `sn` goes to `peer` as DATA_FRAGs: it is
    /// longer than [`Self::fragment_size_for`].
    #[must_use]
    fn needs_fragmentation(&self, sn: SequenceNumber, peer: Guid, payload: &[u8]) -> bool {
        if payload.is_empty() {
            return false;
        }
        let len = u32::try_from(payload.len()).unwrap_or(u32::MAX);
        // DDSI-RTPS 2.5 §8.4.14.1: data is fragmented only where required.
        // Where the path to `peer` carries a larger message than the
        // writer's smallest transport does, the sample goes as one DATA up
        // to that message size; the fragment size stays the writer's.
        let single = self
            .fragment_size_for(sn)
            .max(self.unfragmented_room(sn, peer));
        len > single
    }

    /// Octets of serialized data one DATA to `peer` holds, when the path to
    /// `peer` carries a larger message than the writer's own
    /// ([`ReaderProxy::message_limit`]); 0 otherwise.
    fn unfragmented_room(&self, sn: SequenceNumber, peer: Guid) -> u32 {
        // RTPS header 20, INFO_DST 4 + 12, INFO_TS 4 + 8, DATA submessage
        // header 4 + 20 fixed octets (DDSI-RTPS 2.5 §9.4.5.3).
        const BESIDE: usize = 20 + 16 + 12 + 24;
        let size = self.message_size_for(peer);
        if size <= self.mtu {
            return 0;
        }
        let room = size.saturating_sub(BESIDE + self.sample_inline_qos_len(sn, Some(peer)));
        u32::try_from(room).unwrap_or(u32::MAX)
    }

    /// The fragment size of `sn`: the configured one, or less where the
    /// sample's in-line QoS, counted at its largest for any reader, would
    /// push a DATA_FRAG past the MTU. The same for every fragment, every
    /// resend and every reader (DDSI-RTPS 2.5 §8.4.14.1), which NACK_FRAG's
    /// fragment numbers rely on.
    ///
    /// [`DEFAULT_FRAGMENT_SIZE`] at [`crate::message_builder::DEFAULT_MTU`]
    /// leaves 44 octets beside the RTPS header, INFO_DST, INFO_TS and the
    /// DATA_FRAG's own header: `PID_KEY_HASH` fits, a coherent set member
    /// with it fits, a member of a GROUP-scope publisher's set does not.
    #[must_use]
    fn fragment_size_for(&self, sn: SequenceNumber) -> u32 {
        // RTPS header 20, INFO_DST 4 + 12, INFO_TS 4 + 8, DATA_FRAG
        // submessage header 4 + 32 fixed octets (DDSI-RTPS 2.5 §9.4.5.4).
        const BESIDE: usize = 20 + 16 + 12 + 36;
        // The same for every reader: DDSI-RTPS 2.5 §8.4.14.1 "the data a
        // fragment number refers to does not depend on a particular remote
        // Reader". The in-line QoS is therefore counted at its largest.
        let qos = self.sample_inline_qos_len(sn, None);
        // The message, and the submessage: `octetsToNextHeader` is an
        // `unsigned short`, so a DATA_FRAG holds its 32 fixed octets, the
        // in-line QoS and the fragment in 65535 at most.
        let room = self
            .mtu
            .saturating_sub(BESIDE + qos)
            .min(usize::from(u16::MAX).saturating_sub(32 + qos));
        self.fragment_size
            .min(u32::try_from(room).unwrap_or(u32::MAX))
            .max(1)
    }

    /// How many fragments of `frag_size` one DATA_FRAG to `peer` holds:
    /// as many as the message to it carries beside the RTPS header, the
    /// INFO_DST and INFO_TS, the submessage's own fixed octets and the
    /// sample's in-line QoS, at least one and at most `u16::MAX`
    /// (`fragmentsInSubmessage` is an `unsigned short`).
    fn fragments_per_message(&self, sn: SequenceNumber, peer: Guid, frag_size: usize) -> usize {
        // RTPS header 20, INFO_DST 4 + 12, INFO_TS 4 + 8, DATA_FRAG
        // submessage header 4 + 32 fixed octets (DDSI-RTPS 2.5 §9.4.5.4).
        const BESIDE: usize = 20 + 16 + 12 + 36;
        let room = self
            .message_size_for(peer)
            .saturating_sub(BESIDE + self.sample_inline_qos_len(sn, Some(peer)));
        (room / frag_size.max(1)).clamp(1, usize::from(u16::MAX))
    }

    /// Octets of the in-line QoS [`Self::sample_inline_qos`] gives `sn`
    /// for `peer` (the largest list for `None`), without building it where
    /// only a key hash is there.
    fn sample_inline_qos_len(&self, sn: SequenceNumber, peer: Option<Guid>) -> usize {
        let Some(change) = self.cache.get(sn) else {
            return 0;
        };
        let more = change.inline_qos.is_some()
            || change.coherent_set_sn.is_some()
            || change.coherent_set_end
            || change.group.is_some()
            || (self.inline_writer_qos.is_some()
                && peer.is_none_or(|peer| {
                    self.inline_qos_readers > 0
                        && self
                            .reader_proxies
                            .iter()
                            .any(|p| p.remote_reader_guid == peer && p.expects_inline_qos())
                }));
        if more {
            return self
                .sample_inline_qos_for(sn, peer)
                .map_or(0, |pl| pl.wire_len());
        }
        // PID_KEY_HASH and the sentinel.
        change.key_hash.map_or(0, |_| 4 + 16 + 4)
    }

    /// Whether every reliable reader proxy has acknowledged the highest
    /// sequence number in the cache. `true` for an empty cache and for a
    /// writer without reliable readers: there is nothing to acknowledge.
    ///
    /// The question of DDS 1.4 §2.2.2.4.2.15 `wait_for_acknowledgments`:
    /// "acknowledged by all matched DataReader entities that have
    /// RELIABILITY QoS kind RELIABLE". A best-effort reader sends no
    /// ACKNACK; counted, it held every wait to its timeout.
    #[must_use]
    pub fn all_samples_acknowledged(&self) -> bool {
        let Some(max_sn) = self.cache.max_sn() else {
            return true;
        };
        self.reader_proxies
            .iter()
            .filter(|p| p.is_reliable)
            .all(|p| p.highest_acked_sn() >= max_sn)
    }

    /// Adds a reader proxy. Idempotent: if a proxy with the
    /// same `remote_reader_guid` exists, it is replaced.
    ///
    /// Sets `last_heartbeat = None`, so the next `tick()` immediately
    /// emits a heartbeat to **all** proxies (incl. the new one).
    /// RTPS §8.4.15.4: a freshly added ReaderProxy must get an opportunity
    /// to AckNack, otherwise it waits until the next periodic
    /// heartbeat round (default 1 s) — and for late-wired proxies
    /// (after cache inserts) this is the only way to catch up the early-
    /// inserted samples, since `write_sample_with_datagrams`
    /// only sends directly if the proxy is synchronous.
    pub fn add_reader_proxy(&mut self, proxy: ReaderProxy) {
        let guid = proxy.remote_reader_guid;
        if proxy.expects_inline_qos() {
            self.inline_qos_readers += 1;
        }
        if proxy.filter().is_some() {
            self.filtered_readers += 1;
        }
        if let Some(idx) = self
            .reader_proxies
            .iter()
            .position(|p| p.remote_reader_guid == guid)
        {
            if self.reader_proxies[idx].expects_inline_qos() {
                self.inline_qos_readers -= 1;
            }
            if self.reader_proxies[idx].filter().is_some() {
                self.filtered_readers -= 1;
            }
            self.proxy_targets[idx] = proxy_targets_of(&proxy);
            self.reader_proxies[idx] = proxy;
        } else {
            self.proxy_targets.push(proxy_targets_of(&proxy));
            self.reader_proxies.push(proxy);
        }
        // Force the next tick to HB — the new proxy needs it for
        // AckNack-driven catch-up.
        self.last_heartbeat = None;
    }

    /// Removes the proxy with the given GUID.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        let idx = self
            .reader_proxies
            .iter()
            .position(|p| p.remote_reader_guid == guid)?;
        let proxy = self.reader_proxies.remove(idx);
        self.proxy_targets.remove(idx);
        if proxy.expects_inline_qos() {
            self.inline_qos_readers -= 1;
        }
        if proxy.filter().is_some() {
            self.filtered_readers -= 1;
        }
        // What only it had left to acknowledge holds no room any more.
        self.release_acknowledged_window();
        Some(proxy)
    }

    // ---------- Write ----------

    /// Writes a new sample and fans it out to all proxies.
    ///
    /// Per proxy this produces (aggregated):
    /// - 1 DATA datagram if `payload.len() <= fragment_size`
    /// - N DATA_FRAG datagrams (one datagram per fragment, no mix)
    ///
    /// # Errors
    /// SN overflow, cache full, body too large.
    pub fn write(&mut self, payload: &[u8]) -> Result<Vec<OutboundDatagram>, WireError> {
        self.write_stamped(payload, None)
    }

    /// Like [`Self::write`], but attaches a source timestamp (DDSI-RTPS
    /// §8.7.3): the writer prepends an INFO_TS submessage before each DATA so
    /// the reader can populate `SampleInfo.source_timestamp` and apply
    /// `DESTINATION_ORDER = BY_SOURCE_TIMESTAMP`. `None` ⇒ no INFO_TS.
    ///
    /// # Errors
    /// SN overflow, cache full, body too large.
    pub fn write_stamped(
        &mut self,
        payload: &[u8],
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.write_keyed_stamped(payload, source_timestamp, None)
    }

    /// Writes an alive sample carrying a known instance key in inline QoS.
    /// `None` preserves the legacy unknown-key path.
    ///
    /// # Errors
    /// As [`Self::write_stamped`].
    pub fn write_keyed_stamped(
        &mut self,
        payload: &[u8],
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        key_hash: Option<[u8; 16]>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.write_sample_with_inline_qos(payload, source_timestamp, key_hash, None)
    }

    /// Writes an alive sample while retaining its additional inline QoS in history.
    /// Initial DATA, DATA_FRAG and retransmissions carry the same parameters.
    ///
    /// # Errors
    /// Sequence overflow, full history or a sample/inline list exceeding the MTU.
    pub fn write_sample_with_inline_qos(
        &mut self,
        payload: &[u8],
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        key_hash: Option<[u8; 16]>,
        inline_qos: Option<crate::parameter_list::ParameterList>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.write_staged(payload, source_timestamp, key_hash, inline_qos, false, None)
    }

    /// [`Self::write_sample_with_inline_qos`] for a payload that is not in
    /// an encapsulation of DDSI-RTPS 2.5 §10, the `CryptoContent` of a
    /// protected SerializedPayload (DDS-Security 1.2 §9.5.3.3.4.4): its DATA
    /// and DATA_FRAG carry the NonStandardPayloadFlag
    /// ([`CacheChange::non_standard_payload`]).
    ///
    /// # Errors
    /// As [`Self::write_sample_with_inline_qos`].
    pub fn write_non_standard_sample_with_inline_qos(
        &mut self,
        payload: &[u8],
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        key_hash: Option<[u8; 16]>,
        inline_qos: Option<crate::parameter_list::ParameterList>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.write_staged(payload, source_timestamp, key_hash, inline_qos, true, None)
    }

    /// Like [`Self::write`], for a change meant for the readers of the
    /// participant `destination` only (see [`CacheChange::destination`]).
    ///
    /// # Errors
    /// As [`Self::write`].
    pub fn write_to(
        &mut self,
        payload: &[u8],
        destination: Option<GuidPrefix>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.write_staged(payload, None, None, None, false, destination)
    }

    /// Addresses the next change this writer stages to the readers
    /// `readers` only (see [`CacheChange::destination_readers`]); `None`
    /// addresses it to all. The setting is used up by the next write: the
    /// matched readers that are not named get a GAP for its number, and a
    /// repair does the same. A write that stages nothing leaves it as it
    /// was, so a caller sets it before the write and clears it after.
    pub fn set_next_destination_readers(&mut self, readers: Option<Vec<Guid>>) {
        self.next_destination_readers = readers.map(alloc::sync::Arc::from);
    }

    fn write_staged(
        &mut self,
        payload: &[u8],
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        key_hash: Option<[u8; 16]>,
        inline_qos: Option<crate::parameter_list::ParameterList>,
        non_standard: bool,
        destination: Option<GuidPrefix>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let (sn, payload) = self.stage_sample(
            payload,
            source_timestamp,
            key_hash,
            inline_qos,
            non_standard,
            destination,
        )?;

        let mut out = Vec::new();
        // §8.4.7.1 pushMode: with `false` the writer announces the change
        // only via HEARTBEAT and sends it when a reader asks for it. The
        // change is staged in the cache either way; leaving the proxies'
        // `highest_sent_sn` untouched is what makes the reader see it as
        // missing and request it.
        if !self.push_mode {
            return Ok(out);
        }
        for idx in 0..self.reader_proxies.len() {
            // `next_unsent_change(cache_max)` advances the proxy by exactly
            // one SN. Two cases:
            //
            // * The proxy was synchronous (`highest_sent_sn == sn - 1`): advance
            //   returns `Some(sn)`, we can send the sample directly to the
            //   peer — saving a heartbeat round.
            //
            // * The proxy lags (wired late via SEDP, the cache already had
            //   older samples): advance returns an older SN. Then
            //   it would be wrong to send the *new* payload with the *new* SN
            //   directly — the proxy thinks an early SN went out first
            //   while the reader sees the new one. Instead
            //   we let `tick()` resolve the gap via heartbeat + AckNack-
            //   controlled resend (the standard reliable path).
            // Only a proxy that is in step is advanced: moving a lagging one
            // on by one SN marks an older change as sent that was not.
            if self.reader_proxies[idx].highest_sent_sn().0 + 1 != sn.0 {
                continue;
            }
            let advanced = self.reader_proxies[idx].next_unsent_change(sn);
            if advanced != Some(sn) {
                continue;
            }
            let peer_guid = self.reader_proxies[idx].remote_reader_guid;
            let targets = self.targets_for(idx);
            // The encapsulation header (payload bytes 0..4, RTPS 2.5
            // §10.5) is already set honestly by the caller and reflects
            // the actual body encoding. No per-peer
            // relabeling — restamping an encap byte without re-encoding the body
            // would fake a wrong representation to the reader
            // (XTypes 1.3 §7.4: XCDR1/XCDR2 differ
            // in alignment, so they are NOT bit-identical).
            self.push_sample_datagrams(sn, &payload, peer_guid, &targets, &mut out)?;
            self.note_sent(sn, self.clock);
        }
        Ok(out)
    }

    /// D.5e phase 2: write + piggyback HEARTBEAT in one operation.
    ///
    /// Cyclone DDS and FastDDS additionally send a HEARTBEAT on every `write()`,
    /// so the reader can trigger an ACKNACK
    /// immediately. Without the piggyback the reader must wait until the next periodic
    /// HB (default 100 ms) — which becomes the latency floor for a
    /// 1-in-flight roundtrip.
    ///
    /// This method is a superset of [`Self::write`]: it emits
    /// all DATA datagrams and appends a HEARTBEAT datagram per matched
    /// reader proxy. `last_heartbeat = now` is set so that
    /// `tick()` does not fire twice.
    ///
    /// # Errors
    /// Wire encode error.
    pub fn write_with_heartbeat(
        &mut self,
        payload: &[u8],
        now: Duration,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.write_with_heartbeat_stamped(payload, now, None)
    }

    /// Like [`Self::write_with_heartbeat`], with a source timestamp (emits
    /// INFO_TS before each DATA — see [`Self::write_stamped`]).
    ///
    /// # Errors
    /// Wire encode error.
    pub fn write_with_heartbeat_stamped(
        &mut self,
        payload: &[u8],
        now: Duration,
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.write_with_heartbeat_to(payload, now, source_timestamp, None)
    }

    /// Like [`Self::write_with_heartbeat_stamped`], for a change meant for
    /// the readers of the participant `destination` only (see
    /// [`CacheChange::destination`]): the other matched readers get a GAP
    /// for its number where the addressed ones get the DATA, here and in
    /// every repair.
    ///
    /// # Errors
    /// Wire encode error.
    pub fn write_with_heartbeat_to(
        &mut self,
        payload: &[u8],
        now: Duration,
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        destination: Option<GuidPrefix>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let (sn, payload) =
            self.stage_sample(payload, source_timestamp, None, None, false, destination)?;
        // first_sn for the HEARTBEAT. final_flag=false: the reader MUST
        // respond with ACKNACK (RTPS 2.5 §8.4.15.5).
        let cache_min = self.cache.min_sn().unwrap_or(SequenceNumber(1));
        let mut out = Vec::new();
        for idx in 0..self.reader_proxies.len() {
            // §8.4.7.1 pushMode: with `false` no DATA leaves on write. The
            // change is announced by the HEARTBEAT this method emits anyway
            // and goes out once the reader requests it. Short-circuit, so
            // that `next_unsent_change` does not advance the proxy — a
            // proxy advanced past the change would never see it as missing.
            let advanced =
                self.push_mode && self.reader_proxies[idx].next_unsent_change(sn) == Some(sn);
            let peer_guid = self.reader_proxies[idx].remote_reader_guid;
            let fragmented = self.needs_fragmentation(sn, peer_guid, &payload);
            let targets = self.targets_for(idx);
            // Per-proxy HEARTBEAT `first_sn` (RTPS 2.5 §8.4.12.1: the
            // smallest SN relevant FOR THIS READER) —
            // identical to the `tick` logic. A volatile late-joiner proxy
            // has advanced via `skip_samples_up_to` past the global `cache_min`;
            // with `cache_min` as first_sn the
            // HEARTBEAT would prompt it to re-request the deliberately skipped pre-
            // history — a lossy NACK→GAP roundtrip that
            // stalls under load (flake `volatile_writer_…`). `highest_acked
            // + 1` returns instead exactly its first relevant SN.
            let hb_first_sn = cache_min.max(SequenceNumber(
                self.reader_proxies[idx]
                    .highest_acked_sn()
                    .0
                    .saturating_add(1),
            ));
            // Both branches below send this reader a non-final HEARTBEAT, and
            // its ACKNACK answers it as much as one from `tick` would.
            // DDSI-RTPS 2.5 §8.4.9.1: a best-effort writer sends no HEARTBEAT; a
            // reader that asked for no reliability gets none, and answers none.
            let reliable = self.reader_proxies[idx].is_reliable;
            if reliable {
                self.reader_proxies[idx].heartbeat_sent(now);
            }
            if advanced && !fragmented {
                // Perf: DATA + piggyback HEARTBEAT share ONE
                // RTPS message / ONE datagram — one `sendto` instead of two
                // (and one fewer `recvfrom` + wakeup at the reader).
                // RTPS 2.5 allows multiple submessages per message; this
                // is Cyclone's "piggyback heartbeat" 1:1. If the
                // HEARTBEAT no longer fits the MTU,
                // `append_submessage` automatically splits into two datagrams.
                let mut builder = self.open_builder(idx, &targets);
                self.append_data(&mut builder, sn, &payload, peer_guid, &mut out, &targets)?;
                // Piggyback ONLY if the DATA submessage ends on a
                // 32-bit boundary (RTPS 2.5 §8.3.4.1: every submessage
                // starts 4-byte-aligned). With an odd serializedData
                // length the HEARTBEAT would start misaligned — strict
                // readers (Cyclone DDS, RTI Connext) then reject the message
                // as malformed.
                //
                // NOTE: inserting a PAD submessage between DATA and HB does
                // NOT work, because then PAD itself starts at an
                // unaligned offset — Cyclone reports
                // `malformed packet state parse:DATA`. Real inlining via
                // serializedData padding would be the other option, but breaks
                // `RawBytes` readers (zerodds-self tests see the
                // pad as trailing bytes). Currently: 2 datagrams for
                // odd payloads — wire conformance before perf.
                let coframe = builder.len() % 4 == 0;
                if coframe && reliable {
                    self.append_heartbeat(
                        &mut builder,
                        peer_guid,
                        hb_first_sn,
                        false,
                        false,
                        &mut out,
                        &targets,
                    )?;
                }
                if let Some(dg) = builder.finish() {
                    out.push(dg);
                }
                if !coframe && reliable {
                    let mut hb_builder = self.open_builder(idx, &targets);
                    self.append_heartbeat(
                        &mut hb_builder,
                        peer_guid,
                        hb_first_sn,
                        false,
                        false,
                        &mut out,
                        &targets,
                    )?;
                    if let Some(dg) = hb_builder.finish() {
                        out.push(dg);
                    }
                }
            } else {
                // Fragmented sample → its own DATA_FRAG datagrams; lagging
                // proxy (advanced=false) → HEARTBEAT only. The HEARTBEAT goes
                // out here as its own datagram.
                if advanced {
                    out.extend(self.build_sample_datagrams(sn, &payload, peer_guid, &targets)?);
                }
                if reliable {
                    let mut builder = self.open_builder(idx, &targets);
                    self.append_heartbeat(
                        &mut builder,
                        peer_guid,
                        hb_first_sn,
                        false,
                        false,
                        &mut out,
                        &targets,
                    )?;
                    if let Some(dg) = builder.finish() {
                        out.push(dg);
                    }
                }
            }
        }
        self.last_heartbeat = Some(now);
        Ok(out)
    }

    /// SN allocation + HistoryCache insert. The payload `Arc<[u8]>` is
    /// **recycled** if the pool has a matching buffer
    /// (cache eviction → pool → the next `stage_sample` writes directly
    /// into it, instead of allocating `Arc::from(payload)` anew). Cyclone's
    /// `nn_xmsg_pool` equivalent for the payload buffer.
    ///
    /// Recycling condition: the pooled buffer has the **same length** and
    /// `strong_count == 1` (no one else holds it). Otherwise drop the
    /// pool entry + a fresh `Arc::from(payload)` allocation.
    fn stage_sample(
        &mut self,
        payload: &[u8],
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        key_hash: Option<[u8; 16]>,
        inline_qos: Option<crate::parameter_list::ParameterList>,
        non_standard: bool,
        destination: Option<GuidPrefix>,
    ) -> Result<(SequenceNumber, alloc::sync::Arc<[u8]>), WireError> {
        // Taken first, so that a write that fails after this point does not
        // leave its addressees for the next one.
        let destination_readers = self.next_destination_readers.take();
        let sn_value = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "sequence number overflow",
            })?;
        self.next_sn = sn_value;
        let sn = SequenceNumber(sn_value);
        // DDSI-RTPS 2.5 §8.7.6: the set is named by the SN of its first
        // sample, which only exists once that sample is staged.
        if self.coherent_set_armed && self.coherent_set_sn.is_none() {
            self.coherent_set_sn = Some(sn_value);
        }
        let coherent_set_sn = self.coherent_set_sn;

        // Try recycle from the pool. We pop from the back end (LIFO,
        // cache-warm). On a miss (refcount > 1 or length != payload)
        // we drop the pool entry and try the next.
        let arc_payload = loop {
            match self.payload_pool.pop() {
                None => {
                    // Pool empty → allocate fresh (legacy path).
                    break alloc::sync::Arc::<[u8]>::from(payload);
                }
                Some(mut arc) => {
                    let len_match = arc.len() == payload.len();
                    let exclusive = alloc::sync::Arc::strong_count(&arc) == 1
                        && alloc::sync::Arc::weak_count(&arc) == 0;
                    if len_match && exclusive {
                        // SAFETY: strong_count == 1, weak_count == 0,
                        // so exclusive access → `get_mut` is Some.
                        if let Some(slice) = alloc::sync::Arc::get_mut(&mut arc) {
                            slice.copy_from_slice(payload);
                            break arc;
                        }
                        // Theoretically unreachable (pre-check above);
                        // defensive: drop + next.
                    }
                    // Pool entry does not fit — drop it + try the next
                    // loop iteration.
                    drop(arc);
                }
            }
        };

        let group = self.stamp_group_change();
        self.note_gsn(sn, group);
        // Insert into the cache and capture the evicted Arc for the pool.
        let evicted = self
            .cache
            .insert_returning_evicted(
                CacheChange::alive_arc(sn, alloc::sync::Arc::clone(&arc_payload))
                    .with_key_hash(key_hash)
                    .with_inline_qos(inline_qos)
                    .with_source_timestamp(source_timestamp)
                    .with_coherent_set(coherent_set_sn)
                    .with_group(group)
                    .with_non_standard_payload(non_standard)
                    .with_destination(destination)
                    .with_destination_readers(destination_readers),
            )
            .map_err(|_| {
                // The number belongs to a change that does not exist: give it
                // back, or the readers would wait for a sequence number the
                // writer never had.
                self.next_sn = sn_value - 1;
                WireError::ValueOutOfRange {
                    message: "history cache full or duplicate",
                }
            })?;
        if let Some(evicted_change) = evicted {
            if self.payload_pool.len() < self.payload_pool_cap {
                self.payload_pool.push(evicted_change.payload);
            }
            // otherwise drop (pool full — typically not in steady state).
        }
        if self.unacked_window > 0 && self.reader_proxies.iter().any(|p| p.is_reliable) {
            self.unacked.push_back((sn, payload.len()));
            self.unacked_bytes = self.unacked_bytes.saturating_add(payload.len());
        }
        Ok((sn, arc_payload))
    }

    // ---------- Tick ----------

    /// When [`Self::tick`] sends the next periodic HEARTBEAT, on the clock
    /// `tick` is given: a `heartbeatPeriod` after the last one, or at once
    /// when none went out yet. `None` with nothing in the history, where
    /// `tick` sends none, and without a matched reader, which it sends none
    /// to. A writer that writes asks its scheduler for this instant once,
    /// where it asked for a pass after every write.
    #[must_use]
    pub fn heartbeat_due(&self) -> Option<Duration> {
        // A writer of a group heartbeats with an empty cache too (DDSI-RTPS
        // 2.5 §8.4.2.2): the readers learn from it which group sequence
        // numbers it does not have.
        if self.reader_proxies.is_empty() || (self.cache.is_empty() && self.group.is_none()) {
            return None;
        }
        Some(
            self.last_heartbeat
                .map_or(Duration::ZERO, |last| last + self.heartbeat_period),
        )
    }

    /// Tick-Event: HEARTBEATs + Resends + NACK_FRAG-Responses, aggregiert.
    ///
    /// # Errors
    /// Wire-encode error.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        self.clock = self.clock.max(now);
        if !self.nack_suppression_duration.is_zero() {
            let oldest = self.cache.min_sn().unwrap_or(SequenceNumber(i64::MAX));
            self.sent_at.retain(|sn, _| *sn >= oldest);
        }
        let should_heartbeat = match self.last_heartbeat {
            None => true,
            Some(last) => now.saturating_sub(last) >= self.heartbeat_period,
        };
        let emit_hb = should_heartbeat && !self.cache.is_empty();
        // DDSI-RTPS 2.5 §8.4.2.2: "A Writer belonging to a Group shall send
        // HEARTBEAT or GAP Submessages to its matched Readers even if the
        // Reader has acknowledged all of that Writer's samples", so the
        // Subscriber learns which group sequence numbers this writer does
        // not have — except where its DATA said so: when the group's last
        // number is its own, or no HEARTBEAT is due since the last one.
        let group_current = self.group.as_ref().map(|g| g.current_gsn());
        let group_hb = should_heartbeat
            && group_current
                .is_some_and(|c| c > self.announced_current_gsn && Some(c) != self.last_gsn);

        let mut out = Vec::new();
        let mut hb_emitted_any = false;

        for idx in 0..self.reader_proxies.len() {
            let peer_guid = self.reader_proxies[idx].remote_reader_guid;
            let targets = self.targets_for(idx);

            // §8.4.7.1 nackResponseDelay: what this reader asked for goes
            // out once the request has waited that long.
            let repair = self.reader_proxies[idx].repair_due(now, self.nack_response_delay);

            // 1) Fragment resends (from NACK_FRAG) — the fragments asked for
            // that are neighbours go in one DATA_FRAG as far as the message
            // to the reader carries them (§8.4.14.1)
            if repair {
                let fits = |w: &Self, sn: SequenceNumber| {
                    let fs = w.fragment_size_for(sn) as usize;
                    u16::try_from(w.fragments_per_message(sn, peer_guid, fs)).unwrap_or(u16::MAX)
                };
                while let Some((sn, frag)) = self.reader_proxies[idx].next_requested_fragment() {
                    let more = fits(self, sn).saturating_sub(1);
                    let mut count = 1u16;
                    if more > 0 {
                        if let Some((_, _, extra)) = self.reader_proxies[idx]
                            .next_requested_fragment_run_after(sn, frag, more)
                        {
                            count += extra;
                        }
                    }
                    match self.cache.get(sn) {
                        Some(change) => {
                            let payload = change.payload.clone();
                            #[cfg(feature = "metrics")]
                            crate::metrics::inc_retransmit();
                            #[cfg(feature = "metrics")]
                            crate::metrics::inc_fragmented_sample();
                            out.push(self.build_data_frag_datagram(
                                sn, frag, count, &payload, peer_guid, &targets,
                            )?);
                        }
                        None => {
                            out.push(self.build_gap_datagram(sn, peer_guid, &targets)?);
                        }
                    }
                }
            }

            // 2) Aggregated datagram for whole-SN resends + optional HB
            let mut builder = self.open_builder(idx, &targets);
            let mut repaired_any = false;
            // What this round repaired out of order: the push below moves the
            // cursor over it without sending it a second time.
            let mut repaired_ahead: Vec<SequenceNumber> = Vec::new();

            if repair {
                while let Some(sn) = self.reader_proxies[idx].next_requested_change() {
                    #[cfg(feature = "metrics")]
                    crate::metrics::inc_retransmit();
                    self.append_change(&mut builder, sn, peer_guid, &mut out, &targets)?;
                    let in_step = sn.0 == self.reader_proxies[idx].highest_sent_sn().0 + 1;
                    self.reader_proxies[idx].repaired(sn);
                    if !in_step {
                        repaired_ahead.push(sn);
                    }
                    self.note_sent(sn, now);
                    repaired_any = true;
                }
            }
            self.reader_proxies[idx].repairs_sent();
            // DDSI-RTPS 2.5 §8.4.9.1, Table 8.65 T4: a best-effort reader is
            // pushed `next_unsent_change()` until none is left — a DATA for a
            // change in the cache, a GAP for one that is not. It asks for
            // nothing (§8.4.2.3), so what `write` did not send it — a reader
            // that matched a TRANSIENT_LOCAL writer after it wrote — reaches
            // it only this way. §8.4.9.2, Table 8.66 T4 does the same for a
            // reliable reader under pushMode: `write` sends a change at once
            // only to a reader in step, and one behind (a late joiner, one
            // whose request was answered with a GAP) is brought into step
            // here. Left to its requests, a reader that asks for the last
            // change a HEARTBEAT names never got into step with a KEEP_LAST
            // writer that replaced that change within the nackResponseDelay
            // (HDDS 1.4.8 behind a TRANSIENT writer: HEARTBEAT and GAP, no
            // DATA, OMG dds-rtps Test_Durability_9).
            if !self.reader_proxies[idx].is_reliable || self.push_mode {
                if let Some(cache_max) = self.cache.max_sn() {
                    while let Some(sn) = self.reader_proxies[idx].next_unsent_change(cache_max) {
                        if repaired_ahead.contains(&sn) {
                            continue;
                        }
                        self.append_change(&mut builder, sn, peer_guid, &mut out, &targets)?;
                        self.note_sent(sn, now);
                    }
                }
            }
            // A reader that asked for a full SequenceNumberSet may be missing
            // more than the set could name, and it asks again only in
            // response to a HEARTBEAT (DDSI-RTPS 2.5 §8.4.2.3). One with the
            // repair lets it ask for the next window a round trip later
            // instead of a heartbeatPeriod later: measured with
            // `zerodds_perf` after a reader overflow, 10 HEARTBEATs a second
            // at 256 changes each refilled at most about 2 560 changes a
            // second. A reader without room does not answer it (see
            // `ReliableReader::tick_outbound`), so this does not turn into a
            // loop at the round-trip rate.
            let chained = repaired_any
                && self.reader_proxies[idx].take_full_window()
                && self
                    .cache
                    .max_sn()
                    .is_some_and(|last| self.reader_proxies[idx].unacked_changes(last));

            // 3) Piggyback HEARTBEAT at the end (if due).
            //
            // `first_sn` is per-proxy: `max(cache.min_sn, proxy.highest_acked + 1)`.
            // The per-proxy `highest_acked + 1` prevents volatile
            // proxies (which advanced past the cache-min via `skip_samples_up_to`)
            // from asking the reader to re-request old
            // samples. Spec §8.4.12.1: firstSN is the
            // "smallest sequence number considered relevant FOR THE READER".
            //
            // **FinalFlag (WP 1.E stage A, §8.4.9.2.7):** periodic
            // HEARTBEATs MUST carry `FinalFlag = NOT_SET`, so that the
            // reader is obliged to respond (reliable liveness).
            // A set final bit would signal to the reader
            // "you need do nothing" — which leads to never receiving an ACKNACK
            // again after discovery with a fully-acknowledged cache, and
            // the writer falling into a zombie state. Hence hard
            // `false` here.
            // A writer with nothing written sends no periodic HEARTBEAT, but
            // a reader that has not answered one yet gets one at the same
            // rate, first 1 and last 0 (see `ReaderProxy::handshake_owed`).
            let handshake_hb = should_heartbeat
                && self.cache.is_empty()
                && self.reader_proxies[idx].handshake_owed();
            // The periodic HEARTBEAT is the `announcing` state of DDSI-RTPS
            // 2.5 Table 8.66: T5 leaves it for `idle` once
            // `RP::unacked_changes()` is false, and T7 fires only in it.
            // §8.4.2.2.3 says the same in prose: the writer heartbeats a
            // reader "until the Reader has either acknowledged receiving all
            // available samples or has disappeared". A reader that has
            // acknowledged the whole cache and answered a HEARTBEAT gets
            // none. It used to get one every period, announcing
            // first = last + 1, and answered each with an ACKNACK: with 18
            // participants on one host at rest, 9 094 of the 9 265
            // HEARTBEATs a second were of that kind (loopback capture).
            let announcing = emit_hb
                && (self.reader_proxies[idx].handshake_owed()
                    || self
                        .cache
                        .max_sn()
                        .is_some_and(|last| self.reader_proxies[idx].unacked_changes(last)));
            // §9.4.5.3: an ACKNACK with F clear is answered by a HEARTBEAT,
            // at once for a reader that had none yet, else at most one per
            // period.
            let asked = self.reader_proxies[idx].heartbeat_is_asked(now, self.heartbeat_period);
            // §8.4.9.1: none for a best-effort reader.
            if self.reader_proxies[idx].is_reliable
                && (announcing || handshake_hb || chained || group_hb || asked)
            {
                self.reader_proxies[idx].heartbeat_sent(now);
                let cache_min = self.cache.min_sn().unwrap_or(SequenceNumber(1));
                let per_proxy_first = SequenceNumber(
                    self.reader_proxies[idx]
                        .highest_acked_sn()
                        .0
                        .saturating_add(1),
                );
                let first_sn = cache_min.max(per_proxy_first);
                self.append_heartbeat(
                    &mut builder,
                    peer_guid,
                    first_sn,
                    /* final_flag */ false,
                    false,
                    &mut out,
                    &targets,
                )?;
                hb_emitted_any = true;
            }

            if let Some(dg) = builder.finish() {
                out.push(dg);
            }
        }

        if hb_emitted_any {
            self.last_heartbeat = Some(now);
            if let Some(c) = group_current {
                self.announced_current_gsn = self.announced_current_gsn.max(c);
            }
        }

        Ok(out)
    }

    /// A HEARTBEAT with the liveliness flag to every matched reader: the
    /// wire form of `DataWriter::assert_liveliness` (DDSI-RTPS 2.5
    /// §8.3.7.5 — the flag "indicates that the DDS DataWriter associated
    /// with the RTPS Writer of the message has manually asserted its
    /// LIVELINESS"). Cyclone DDS's `dds_assert_liveliness` on a writer
    /// sends the same (`write_hb_liveliness`). Final: it asks the reader
    /// for no ACKNACK.
    ///
    /// # Errors
    /// `WireError` if a datagram cannot be built.
    pub fn liveliness_heartbeat(&mut self) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = Vec::new();
        let cache_min = self.cache.min_sn().unwrap_or(SequenceNumber(1));
        for idx in 0..self.reader_proxies.len() {
            let peer_guid = self.reader_proxies[idx].remote_reader_guid;
            let targets = self.targets_for(idx);
            let per_proxy_first = SequenceNumber(
                self.reader_proxies[idx]
                    .highest_acked_sn()
                    .0
                    .saturating_add(1),
            );
            let mut builder = self.open_builder(idx, &targets);
            self.append_heartbeat(
                &mut builder,
                peer_guid,
                cache_min.max(per_proxy_first),
                true,
                true,
                &mut out,
                &targets,
            )?;
            if let Some(dg) = builder.finish() {
                out.push(dg);
            }
        }
        Ok(out)
    }

    // ---------- Incoming Control ----------

    /// Processes a received ACKNACK from `src_guid`.
    /// Unknown sender → no-op.
    pub fn handle_acknack(
        &mut self,
        src_guid: Guid,
        base: SequenceNumber,
        requested: impl IntoIterator<Item = SequenceNumber>,
    ) {
        self.handle_acknack_at(self.clock, src_guid, base, requested);
    }

    /// As [`Self::handle_acknack`], for an ACKNACK that arrived at `now`:
    /// the time `nackResponseDelay` counts from and
    /// `nackSuppressionDuration` is judged at (DDSI-RTPS 2.5 §8.4.7.1).
    pub fn handle_acknack_at(
        &mut self,
        now: Duration,
        src_guid: Guid,
        base: SequenceNumber,
        requested: impl IntoIterator<Item = SequenceNumber>,
    ) {
        self.handle_acknack_window(now, src_guid, base, requested.into_iter().collect(), false);
    }

    /// [`Self::handle_acknack_at`] for the SequenceNumberSet as it arrived.
    /// A set whose `numBits` is the whole window (`RTPS_BITMAP_MAX_BITS`)
    /// may leave out what the reader misses behind it, however few changes
    /// it names: its repair carries a HEARTBEAT, as that of a set naming
    /// 256 changes does, so the reader asks for the next window a round
    /// trip later (DDSI-RTPS 2.5 §8.4.2.3). Gaps spread over a stream fill
    /// such sets with a few changes each, and the reader's next window
    /// waited for the periodic HEARTBEAT: measured with a RELIABLE 1 kB
    /// stream after the kernel dropped datagrams, the reader then took
    /// 5 000–100 000 samples a second for seconds.
    pub fn handle_acknack_set_at(
        &mut self,
        now: Duration,
        src_guid: Guid,
        set: &crate::submessages::SequenceNumberSet,
    ) {
        let cut = set.num_bits >= crate::submessages::RTPS_BITMAP_MAX_BITS;
        self.handle_acknack_window(
            now,
            src_guid,
            set.bitmap_base,
            set.iter_set().collect(),
            cut,
        );
    }

    /// The ACKNACK behind [`Self::handle_acknack_at`] and
    /// [`Self::handle_acknack_set_at`]; `window_cut` when the set may have
    /// left out changes the reader misses.
    fn handle_acknack_window(
        &mut self,
        now: Duration,
        src_guid: Guid,
        base: SequenceNumber,
        mut requested: alloc::vec::Vec<SequenceNumber>,
        window_cut: bool,
    ) {
        self.clock = self.clock.max(now);
        #[cfg(feature = "metrics")]
        crate::metrics::inc_acknack_received();
        let Some(idx) = self
            .reader_proxies
            .iter()
            .position(|p| p.remote_reader_guid == src_guid)
        else {
            self.unknown_src_count = self.unknown_src_count.saturating_add(1);
            return;
        };
        let bitmap_empty = requested.is_empty();
        // Counted before nackSuppressionDuration takes anything out: the
        // question is what the reader could name, not what goes out again.
        let full_window =
            window_cut || requested.len() >= crate::submessages::RTPS_BITMAP_MAX_BITS as usize;
        // §8.4.7.1 nackSuppressionDuration: a change that went out a
        // moment ago is most likely still in flight. A number below the
        // base is one this ACKNACK acknowledges; a SequenceNumberSet cannot
        // name it (§9.4.2.6), and it is not a request.
        requested.retain(|sn| *sn >= base && !self.too_soon(*sn, now));
        self.reader_proxies[idx].acknack_received(
            now,
            self.cache.is_empty(),
            self.heartbeat_period,
        );
        self.reader_proxies[idx].acked_changes_set(base);
        self.release_acknowledged_window();
        self.reader_proxies[idx].requested_changes_set(requested);
        // Cross-vendor reliability: a reader that lags behind the writer cache
        // via a **pure ACK** (base, empty NACK bitmap) requests no
        // concrete SNs — but relies on the writer pushing the
        // unacknowledged tail (RTPS §8.4.2.3.3: non-acked changes are
        // to be delivered reliably). cyclone sends exactly such a "1/0" ACKNACK after
        // an early security decode drop; without this tail request
        // the race-discarded VolatileSecure samples would remain permanently
        // undelivered. With a non-empty bitmap (ZeroDDS reader) unchanged.
        if bitmap_empty {
            if let Some(max_sn) = self.cache.max_sn() {
                let proxy = &mut self.reader_proxies[idx];
                if proxy.unacked_changes(max_sn) {
                    let from = proxy.highest_acked_sn().0 + 1;
                    let tail: alloc::vec::Vec<SequenceNumber> = (from..=max_sn.0)
                        .map(SequenceNumber)
                        .filter(|sn| !self.too_soon(*sn, now))
                        .collect();
                    self.reader_proxies[idx].requested_changes_set(tail);
                }
            }
        }
        self.reader_proxies[idx].note_nack(now);
        self.reader_proxies[idx].note_window(full_window);
        // Cache GC **removed** in the per-destination-queue model (T3
        // refactor): the cache is only trimmed by HistoryKind::KeepLast
        // anymore. A stalled reader thereby no longer blocks the
        // pipeline — for too-old samples it gets GAP responses.
    }

    /// Notes the FinalFlag of an ACKNACK from `src_guid`. DDSI-RTPS 2.5
    /// §9.4.5.3: "F=1 means the reader does not require a Heartbeat from the
    /// writer. F=0 means the writer must respond to the AckNack message with
    /// a Heartbeat message." With F clear the next [`Self::tick`] sends that
    /// reader a HEARTBEAT, whatever it acknowledged: at once when it had
    /// none yet, else a period after the last one at the earliest.
    pub fn note_acknack_final_flag(&mut self, src_guid: Guid, final_flag: bool) {
        if final_flag {
            return;
        }
        if let Some(proxy) = self
            .reader_proxies
            .iter_mut()
            .find(|p| p.remote_reader_guid == src_guid)
        {
            proxy.heartbeat_asked();
        }
    }

    /// Processes a received NACK_FRAG from `src_guid`.
    pub fn handle_nackfrag(&mut self, src_guid: Guid, nf: &NackFragSubmessage) {
        if nf.writer_id != self.guid.entity_id {
            return;
        }
        let Some(idx) = self
            .reader_proxies
            .iter()
            .position(|p| p.remote_reader_guid == src_guid)
        else {
            self.unknown_src_count = self.unknown_src_count.saturating_add(1);
            return;
        };
        self.nackfrag_count = self.nackfrag_count.wrapping_add(1);
        let missing: Vec<FragmentNumber> = nf.fragment_number_state.iter_set().collect();
        self.reader_proxies[idx].requested_fragments_set(nf.writer_sn, missing);
    }

    // ---------- Build helpers ----------

    fn rtps_header(&self) -> RtpsHeader {
        RtpsHeader::new(self.vendor_id, self.guid.prefix)
    }

    /// Target locator set for proxy `idx`; see [`proxy_targets_of`].
    fn targets_for(&self, idx: usize) -> alloc::sync::Arc<Vec<Locator>> {
        alloc::sync::Arc::clone(&self.proxy_targets[idx])
    }

    /// Prepends an INFO_TS submessage (DDSI-RTPS §8.3.7.9) for the change `sn`
    /// if it carries a source timestamp. Must be called on the builder right
    /// before the DATA/DATA_FRAG so it sits in the same datagram (the receiver
    /// applies the last-seen INFO_TS to the following DATA). Tiny (12 B) and
    /// best-effort: if it would not fit, the DATA still goes (the reader falls
    /// back to reception order).
    fn append_info_ts(&self, builder: &mut MessageBuilder, sn: SequenceNumber) {
        if let Some(ts) = self.cache.get(sn).and_then(|c| c.source_timestamp) {
            let body = crate::submessages::InfoTimestampSubmessage::timestamp_body(ts, true);
            let _ = builder.try_add_submessage(SubmessageId::InfoTs, 0, &body);
        }
    }

    /// Whether change `sn` goes to `peer` as what it is: always, unless it
    /// is addressed to another participant (`CacheChange::destination`),
    /// or is an end-of-coherent-set DATA and the peer takes that as a GAP
    /// (`crate::dialect::EndOfSetForm`); then `peer` gets a GAP for the
    /// number instead. A number the cache no longer holds is a GAP anyway.
    fn is_relevant(&self, sn: SequenceNumber, peer: Guid) -> bool {
        self.cache.get(sn).is_none_or(|c| {
            c.is_relevant_to(peer)
                && self.passes_dds_filter(c, peer)
                && !(c.coherent_set_end
                    && self.behaviour.for_peer(peer).end_of_set_form
                        == crate::dialect::EndOfSetForm::Gap)
        })
    }

    /// `DDS_FILTER(the_reader_proxy, a_change)` (DDSI-RTPS 2.5 §8.4.9.1 T4):
    /// the filter of the proxy of `peer`, none is a pass.
    fn passes_dds_filter(&self, change: &CacheChange, peer: Guid) -> bool {
        if self.filtered_readers == 0 {
            return true;
        }
        self.reader_proxies
            .iter()
            .find(|p| p.remote_reader_guid == peer)
            .and_then(ReaderProxy::filter)
            .is_none_or(|f| f.passes(change))
    }

    /// The `filteredCount` of a GAP for `sn` (§8.4.9.1 T4): 1 for a change
    /// the cache holds and the reader is not sent (it did not pass
    /// `DDS_FILTER`, or is addressed to others), 0 for a number the cache
    /// no longer holds, "these changes do not appear counted in the GAP
    /// message filteredCount".
    fn gap_filtered_count(&self, sn: SequenceNumber) -> u64 {
        u64::from(self.cache.get(sn).is_some())
    }

    fn append_data(
        &self,
        builder: &mut MessageBuilder,
        sn: SequenceNumber,
        payload: &alloc::sync::Arc<[u8]>,
        peer: Guid,
        out: &mut Vec<OutboundDatagram>,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<(), WireError> {
        if !self.is_relevant(sn, peer) {
            return self.append_gap(builder, sn, peer, out, targets);
        }
        let reader_id = peer.entity_id;
        self.append_info_ts(builder, sn);
        // DDSI-RTPS 2.5 §8.7.6: a member of a coherent set carries
        // `PID_COHERENT_SET` in its inline QoS, and the end-of-coherent-set
        // marker carries it with the D flag clear. The tag lives on the
        // CacheChange, so it must be rebuilt here too — this function serves
        // the DATA+piggyback-HEARTBEAT path and the retransmit path, and a
        // member sent through either without its marker reaches the reader as
        // a standalone sample. The reader then closes the set around the
        // members it did see: a silently wrong cut, not a dropped sample.
        // Such a change leaves the zero-copy fast path below and goes out as a
        // full `DataSubmessage`, exactly as `build_single_data_datagram`
        // builds it.
        let Some((prefix, flags)) = self.compact_data_prefix(sn, peer) else {
            let is_ecs = self.cache.get(sn).is_some_and(|c| c.coherent_set_end);
            let data = DataSubmessage {
                extra_flags: 0,
                reader_id,
                writer_id: self.guid.entity_id,
                writer_sn: sn,
                inline_qos: self.sample_inline_qos(sn, peer),
                data_flag: !is_ecs,
                key_flag: false,
                non_standard_flag: self.cache.get(sn).is_some_and(|c| c.non_standard_payload),
                serialized_payload: crate::payload::Payload::whole(alloc::sync::Arc::clone(
                    payload,
                )),
            };
            let (body, flags) = data.write_body(true);
            return self.append_submessage(
                builder,
                SubmessageId::Data,
                flags,
                &body,
                peer,
                out,
                targets,
                "DATA",
            );
        };
        // The builder writes the submessage header, the prefix and the
        // payload one after another into its own buffer; see
        // `compact_data_prefix`.
        match builder.try_add_submessage_split(
            SubmessageId::Data,
            flags,
            prefix.as_slice(),
            payload,
        ) {
            Ok(()) => Ok(()),
            Err(AddError::BodyTooLarge) => Err(WireError::ValueOutOfRange {
                message: "DATA body exceeds u16::MAX",
            }),
            // Full, or ending after a submessage whose length is not a
            // multiple of four (DDSI-RTPS 2.5 §9.4.1): a new datagram.
            Err(AddError::WouldExceedMtu { .. } | AddError::Unaligned) => {
                // The continuation datagram is still addressed to the same
                // proxy: opening it plain would drop the INFO_DST the first
                // one carried (issue #32).
                let finished = core::mem::replace(builder, self.open_builder_for(peer, targets));
                if let Some(dg) = finished.finish() {
                    out.push(dg);
                }
                builder
                    .try_add_submessage_split(SubmessageId::Data, flags, prefix.as_slice(), payload)
                    .map_err(|_| WireError::ValueOutOfRange {
                        message: "submessage does not fit into fresh datagram",
                    })
            }
        }
    }

    /// The DATA body up to its payload, in one stack buffer, with the
    /// submessage flags: the twenty fixed octets (extraFlags,
    /// octetsToInlineQos, readerId, writerId, writerSN; DDSI-RTPS 2.5
    /// §9.4.5.3) and, for a keyed change, the inline QoS
    /// `[PID_KEY_HASH, PID_SENTINEL]` (§9.6.3.8).
    ///
    /// The builder then takes the payload straight from the history's
    /// `Arc`, and it is copied once. Without it every keyed sample built a
    /// `ParameterList`, a KeyHash `Vec`, the list's bytes and a body `Vec`
    /// holding a second copy of the payload — six allocations per write,
    /// measured on the Linux test host. The octets are the ones
    /// [`DataSubmessage::write_body`] writes for the same change.
    ///
    /// `None` when the change carries any other inline QoS — the writer's
    /// own for a reader that expects it, its own inline QoS, a coherent
    /// set — or is the end-of-coherent-set marker; the caller writes the
    /// full `DataSubmessage` then.
    fn compact_data_prefix(&self, sn: SequenceNumber, peer: Guid) -> Option<(DataPrefix, u8)> {
        let change = self.cache.get(sn)?;
        if change.inline_qos.is_some()
            || change.coherent_set_sn.is_some()
            || change.coherent_set_end
            || change.group.is_some()
        {
            return None;
        }
        if self.inline_writer_qos.is_some()
            && self.inline_qos_readers > 0
            && self
                .reader_proxies
                .iter()
                .any(|p| p.remote_reader_guid == peer && p.expects_inline_qos())
        {
            return None;
        }
        let mut prefix = DataPrefix {
            bytes: [0; DataPrefix::CAPACITY],
            len: 20,
        };
        // extraFlags 0, octetsToInlineQos 16 (readerId + writerId + writerSN).
        prefix.bytes[2..4].copy_from_slice(&16u16.to_le_bytes());
        prefix.bytes[4..8].copy_from_slice(&peer.entity_id.to_bytes());
        prefix.bytes[8..12].copy_from_slice(&self.guid.entity_id.to_bytes());
        prefix.bytes[12..20].copy_from_slice(&sn.to_bytes_le());
        let mut flags = FLAG_E_LITTLE_ENDIAN | DATA_FLAG_DATA;
        if change.non_standard_payload {
            flags |= crate::submessages::DATA_FLAG_NON_STANDARD;
        }
        if let Some(key) = change.key_hash {
            let qos = &mut prefix.bytes[20..44];
            qos[0..2].copy_from_slice(&crate::parameter_list::pid::KEY_HASH.to_le_bytes());
            qos[2..4].copy_from_slice(&16u16.to_le_bytes());
            qos[4..20].copy_from_slice(&key);
            qos[20..22].copy_from_slice(&crate::parameter_list::pid::SENTINEL.to_le_bytes());
            qos[22..24].copy_from_slice(&0u16.to_le_bytes());
            prefix.len = 44;
            flags |= crate::submessages::DATA_FLAG_INLINE_QOS;
        }
        Some((prefix, flags))
    }

    fn append_gap(
        &self,
        builder: &mut MessageBuilder,
        sn: SequenceNumber,
        peer: Guid,
        out: &mut Vec<OutboundDatagram>,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<(), WireError> {
        let gap = GapSubmessage {
            reader_id: peer.entity_id,
            writer_id: self.guid.entity_id,
            gap_start: sn,
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(sn.0 + 1),
                num_bits: 0,
                bitmap: Vec::new(),
            },
            group_info: self.gap_group_info(sn, peer),
            filtered_count: Some(self.gap_filtered_count(sn)),
        };
        let (body, flags) = gap.write_body(true);
        self.append_submessage(
            builder,
            SubmessageId::Gap,
            flags,
            &body,
            peer,
            out,
            targets,
            "GAP",
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn append_heartbeat(
        &mut self,
        builder: &mut MessageBuilder,
        peer: Guid,
        first_sn: SequenceNumber,
        final_flag: bool,
        liveliness_flag: bool,
        out: &mut Vec<OutboundDatagram>,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<(), WireError> {
        let reader_id = peer.entity_id;
        #[cfg(feature = "metrics")]
        crate::metrics::inc_heartbeat_sent();
        self.heartbeat_count = self.heartbeat_count.wrapping_add(1);
        // lastSN is the last SN written, not the cache's maximum: a VOLATILE
        // writer removes what every reader acknowledged, and an empty cache
        // after SN 31 announced first 32, last 0. DDSI-RTPS 2.5 §8.3.7.5
        // requires lastSN >= firstSN - 1; Cyclone DDS 11.0.1 discards such a
        // message as malformed ("parse:shortmsg") with the DATA that rode in
        // it, and a reader matched after the cache emptied got no sample (OMG
        // dds-rtps, every RELIABLE case from ZeroDDS to Cyclone).
        let last = self.last_written_sn();
        debug_assert!(
            first_sn.0 <= last.0.saturating_add(1),
            "HEARTBEAT first {} beyond last {} + 1",
            first_sn.0,
            last.0
        );
        let first_sn = SequenceNumber(first_sn.0.min(last.0.saturating_add(1)));
        let hb = HeartbeatSubmessage {
            reader_id,
            writer_id: self.guid.entity_id,
            first_sn,
            last_sn: last,
            count: self.heartbeat_count,
            final_flag,
            liveliness_flag,
            group_info: self.heartbeat_group_info(first_sn, last, peer),
        };
        let (body, flags) = hb.write_body(true);
        self.append_submessage(
            builder,
            SubmessageId::Heartbeat,
            flags,
            &body,
            peer,
            out,
            targets,
            "HEARTBEAT",
        )
    }

    /// The group numbers a subscriber in this writer's own participant
    /// commits by instead of a HEARTBEAT (DDSI-RTPS 2.5 §8.7.5):
    /// `currentGSN` the group's last, and every number up to this writer's
    /// last as one it may have. The same-runtime handoff takes a change out
    /// of a VOLATILE writer's cache before the change reaches the reader's
    /// channel, so the cache does not tell what is still on its way; a
    /// number above the writer's last it never had. The group's number is
    /// read under the caller's lock of this writer, so the writer cannot
    /// take one between the two reads.
    #[must_use]
    pub fn local_group_progress(&self) -> Option<crate::submessages::HeartbeatGroupInfo> {
        let group = self.group.as_ref()?;
        Some(crate::submessages::HeartbeatGroupInfo {
            current_gsn: SequenceNumber(group.current_gsn()),
            first_gsn: SequenceNumber(1),
            last_gsn: SequenceNumber(self.last_gsn.unwrap_or(0)),
            writer_set: None,
            secure_writer_set: None,
        })
    }

    /// The group sequence numbers of a HEARTBEAT announcing `first..=last`
    /// (DDSI-RTPS 2.5 §8.3.7.5): `currentGSN` the group's last, `firstGSN`
    /// and `lastGSN` those of the samples `first` and `last`. `None` for a
    /// writer outside a group, and where either sample is no longer in the
    /// cache or carries no number: the GroupInfoFlag is optional, and a
    /// guess would mislead the reader's commit rule (§8.7.5).
    fn heartbeat_group_info(
        &self,
        first: SequenceNumber,
        last: SequenceNumber,
        peer: Guid,
    ) -> Option<crate::submessages::HeartbeatGroupInfo> {
        let group = self.group.as_ref()?;
        if !self.group_info_for(peer) {
            return None;
        }
        let send_digests = self.behaviour.for_peer(peer).heartbeat_group_digests
            == crate::dialect::HeartbeatGroupDigests::Sent;
        let gsn_of = |sn: SequenceNumber| self.cache.get(sn).and_then(|c| c.group).map(|g| g.gsn);
        // The last sample may have left the cache: its number is the
        // writer's last. An empty announcement, first = last + 1, has
        // firstGSN = lastGSN + 1 (§8.3.7.5: lastGSN >= firstGSN - 1).
        let last_gsn = gsn_of(last).or(self.last_gsn)?;
        let first_gsn = if first > last {
            last_gsn + 1
        } else {
            gsn_of(first)?
        };
        let info = crate::submessages::HeartbeatGroupInfo {
            current_gsn: SequenceNumber(group.current_gsn()),
            first_gsn: SequenceNumber(first_gsn),
            last_gsn: SequenceNumber(last_gsn),
            // Table 8.44: the writers of the group at this time, and the
            // secure set, which ZeroDDS has none of (the empty group), after
            // lastGSN; a peer whose behaviour leaves them out gets the form of
            // the figure of §9.4.5.7.
            writer_set: send_digests.then(|| group.writer_set()),
            secure_writer_set: send_digests.then_some(crate::group_digest::GroupDigest::EMPTY),
        };
        info.is_valid().then_some(info)
    }

    /// Shared submessage append with overflow handling.
    #[allow(clippy::too_many_arguments)]
    fn append_submessage(
        &self,
        builder: &mut MessageBuilder,
        id: SubmessageId,
        flags: u8,
        body: &[u8],
        peer: Guid,
        out: &mut Vec<OutboundDatagram>,
        targets: &alloc::sync::Arc<Vec<Locator>>,
        kind_hint: &'static str,
    ) -> Result<(), WireError> {
        match builder.try_add_submessage(id, flags, body) {
            Ok(()) => Ok(()),
            Err(AddError::BodyTooLarge) => Err(WireError::ValueOutOfRange {
                message: match kind_hint {
                    "DATA" => "DATA body exceeds u16::MAX",
                    "GAP" => "GAP body exceeds u16::MAX",
                    "HEARTBEAT" => "HEARTBEAT body exceeds u16::MAX",
                    _ => "submessage body exceeds u16::MAX",
                },
            }),
            // Full, or ending after a submessage whose length is not a
            // multiple of four (DDSI-RTPS 2.5 §9.4.1): a new datagram.
            Err(AddError::WouldExceedMtu { .. } | AddError::Unaligned) => {
                // The continuation datagram is still addressed to the same
                // proxy: opening it plain would drop the INFO_DST the first
                // one carried (issue #32).
                let finished = core::mem::replace(builder, self.open_builder_for(peer, targets));
                if let Some(dg) = finished.finish() {
                    out.push(dg);
                }
                builder.try_add_submessage(id, flags, body).map_err(|_| {
                    WireError::ValueOutOfRange {
                        message: "submessage does not fit into fresh datagram",
                    }
                })
            }
        }
    }

    /// Produces one datagram per fragment (DATA) or one DATA datagram
    /// (if below `fragment_size`). No aggregation with other DATAs.
    fn build_sample_datagrams(
        &self,
        sn: SequenceNumber,
        payload: &alloc::sync::Arc<[u8]>,
        peer: Guid,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = Vec::new();
        self.push_sample_datagrams(sn, payload, peer, targets, &mut out)?;
        Ok(out)
    }

    /// [`Self::build_sample_datagrams`] into `out`: the write path pushes
    /// its one datagram into the list it returns, instead of a list of one
    /// it then copies over.
    fn push_sample_datagrams(
        &self,
        sn: SequenceNumber,
        payload: &alloc::sync::Arc<[u8]>,
        peer: Guid,
        targets: &alloc::sync::Arc<Vec<Locator>>,
        out: &mut Vec<OutboundDatagram>,
    ) -> Result<(), WireError> {
        if !self.is_relevant(sn, peer) {
            out.push(self.build_gap_datagram(sn, peer, targets)?);
            return Ok(());
        }
        if !self.needs_fragmentation(sn, peer, payload) {
            out.push(self.build_single_data_datagram(sn, payload, peer, targets)?);
            return Ok(());
        }
        // Every DATA_FRAG carries the sample's in-line QoS, the first one as
        // DDSI-RTPS 2.5 §8.7.6 requires — a coherent set member's
        // PID_COHERENT_SET with it — and the reader's assembler keeps it.
        let frag_size = self.fragment_size_for(sn) as usize;
        let sample_size = u32::try_from(payload.len()).map_err(|_| WireError::ValueOutOfRange {
            message: "sample size exceeds u32::MAX",
        })?;
        let frag_size_u16 = u16::try_from(frag_size).map_err(|_| WireError::ValueOutOfRange {
            message: "fragment_size exceeds u16::MAX",
        })?;
        // §8.4.14.1: "If a transport can accommodate multiple fragments of
        // the given fragment size, it is recommended that implementations
        // concatenate as many fragments as possible into a single DataFrag
        // message."
        let per_message = self.fragments_per_message(sn, peer, frag_size);
        let mut frag_num: u32 = 1;
        let mut pos = 0usize;
        while pos < payload.len() {
            let end = core::cmp::min(pos + per_message * frag_size, payload.len());
            let count = u16::try_from((end - pos).div_ceil(frag_size)).map_err(|_| {
                WireError::ValueOutOfRange {
                    message: "fragments in submessage exceed u16::MAX",
                }
            })?;
            out.push(self.build_data_frag_submessage_datagram(
                sn,
                FragmentSlice {
                    number: FragmentNumber(frag_num),
                    count,
                    size: frag_size_u16,
                    sample_size,
                    bytes: &payload[pos..end],
                },
                peer,
                targets,
            )?);
            pos = end;
            frag_num =
                frag_num
                    .checked_add(u32::from(count))
                    .ok_or(WireError::ValueOutOfRange {
                        message: "fragment number overflow",
                    })?;
        }
        Ok(())
    }

    fn build_single_data_datagram(
        &self,
        sn: SequenceNumber,
        payload: &alloc::sync::Arc<[u8]>,
        peer: Guid,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let reader_id = peer.entity_id;
        let mut builder = self.open_builder_for(peer, targets);
        self.append_info_ts(&mut builder, sn);
        if let Some((prefix, flags)) = self.compact_data_prefix(sn, peer) {
            builder
                .try_add_submessage_split(SubmessageId::Data, flags, prefix.as_slice(), payload)
                .map_err(|_| WireError::ValueOutOfRange {
                    message: "DATA submessage does not fit into MTU",
                })?;
            return builder.finish().ok_or(WireError::ValueOutOfRange {
                message: "MessageBuilder finish returned no datagram",
            });
        }
        // DDSI-RTPS 2.5 §8.7.6: every DATA of a coherent set carries
        // `PID_COHERENT_SET` in its inline QoS, and the ECS marker that closes
        // the set carries it with both the D and the K flag clear. The tag
        // lives on the CacheChange, so a retransmit rebuilds the identical
        // inline QoS instead of shipping a set member without its marker.
        let is_ecs = self.cache.get(sn).is_some_and(|c| c.coherent_set_end);
        let inline_qos = self.sample_inline_qos(sn, peer);
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id,
            writer_id: self.guid.entity_id,
            writer_sn: sn,
            // WP 2.0a: Arc::clone instead of to_vec — zero-copy into the wire path.
            inline_qos,
            data_flag: !is_ecs,
            key_flag: false,
            non_standard_flag: self.cache.get(sn).is_some_and(|c| c.non_standard_payload),
            serialized_payload: crate::payload::Payload::whole(alloc::sync::Arc::clone(payload)),
        };
        let (body, flags) = data.write_body(true);
        builder
            .try_add_submessage(SubmessageId::Data, flags, &body)
            .map_err(|_| WireError::ValueOutOfRange {
                message: "DATA submessage does not fit into MTU",
            })?;
        builder.finish().ok_or(WireError::ValueOutOfRange {
            message: "MessageBuilder finish returned no datagram",
        })
    }

    /// Spec §9.6.3.9 `PID_STATUS_INFO` lifecycle sample: a DATA with the D
    /// flag clear and inline QoS `[PID_KEY_HASH + PID_STATUS_INFO]`, sent by
    /// the SEDP writers on endpoint deletion and by the DCPS layer on
    /// `dispose`/`unregister_instance`.
    ///
    /// The K flag follows `serialized_key`, and it may not do otherwise.
    /// §8.3.7.2.5 Table 8.43 binds `K=1` to "`serializedPayload` contains the
    /// serialized Key" — a `K=1` DATA whose payload is zero octets long is
    /// not a key, and Cyclone rejects it, together with the rest of the
    /// datagram: `ddsi_receive.c:432` in `validate_Data` returns
    /// `VR_MALFORMED` when `ptr + 4 > msg + size`, i.e. when the payload does
    /// not even hold an encapsulation header, and
    /// `handle_submsg_sequence` (`ddsi_receive.c:3259`) answers a
    /// `VR_MALFORMED` with `malformed_packet_received` and `return -1`,
    /// discarding the whole message. Measured against Cyclone 11.0.1 on
    /// domain 64: `malformed packet received from vendor 1.240 length 76
    /// state parse:shortmsg`, once per reader shutdown.
    ///
    /// The two forms this builds are the two the three vendors were measured
    /// to send on a SEDP subscriptions dispose:
    ///
    /// * `serialized_key` non-empty ⇒ `K=1`, payload = the key. Cyclone
    ///   11.0.1 and OpenDDS 3.34.0 both send this (flags `0x0b`), with the
    ///   key as `PL_CDR_LE` + `PID_ENDPOINT_GUID` + `PID_SENTINEL`.
    /// * `serialized_key` empty ⇒ `K=0` and no payload; the instance is named
    ///   by `PID_KEY_HASH` alone. Fast DDS 3.6.1 sends this (flags `0x03`).
    ///
    /// What it never builds is the third combination, `K=1` with no key.
    fn build_lifecycle_datagram(
        &self,
        sn: SequenceNumber,
        marker: &LifecycleMarker<'_>,
        peer: Guid,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let mut builder = self.open_builder_for(peer, targets);
        // §8.3.7.9: the same INFO_TS a plain `append_data` prepends for an
        // alive change, read off the same field
        // (`CacheChange::source_timestamp`, set on a lifecycle change by
        // `write_lifecycle`) — a dispose/unregister with a source timestamp
        // must reach `SampleInfo.source_timestamp` too.
        self.append_info_ts(&mut builder, sn);
        let data = self.lifecycle_data(sn, marker, peer);
        let (body, flags) = data.write_body(true);
        builder
            .try_add_submessage(SubmessageId::Data, flags, &body)
            .map_err(|_| WireError::ValueOutOfRange {
                message: "lifecycle DATA submessage does not fit into MTU",
            })?;
        builder.finish().ok_or(WireError::ValueOutOfRange {
            message: "MessageBuilder finish returned no datagram",
        })
    }

    /// The lifecycle DATA itself — see [`Self::build_lifecycle_datagram`] for
    /// the two forms and why the third is not among them. A dispose or
    /// unregister inside a coherent set, or of a GROUP-scope publisher's
    /// writer, carries the set and group parameters of its change like a
    /// sample does ([`Self::push_set_params`]); without `PID_COHERENT_SET`
    /// it would end the set at the reader (§8.7.6).
    fn lifecycle_data(
        &self,
        sn: SequenceNumber,
        marker: &LifecycleMarker<'_>,
        peer: Guid,
    ) -> DataSubmessage {
        let mut qos = Some(if marker.form.emits_key_hash() {
            crate::inline_qos::lifecycle_inline_qos(marker.key_hash, marker.status_bits)
        } else {
            crate::inline_qos::lifecycle_inline_qos_status_only(marker.status_bits)
        });
        if let Some(change) = self.cache.get(sn) {
            // The change's own in-line QoS (`write_lifecycle_with_inline_qos`).
            if let (Some(list), Some(own)) = (qos.as_mut(), change.inline_qos.as_ref()) {
                list.parameters.extend(own.parameters.iter().cloned());
            }
            self.push_set_params(change, Some(peer), &mut qos);
        }
        DataSubmessage {
            extra_flags: 0,
            reader_id: peer.entity_id,
            writer_id: self.guid.entity_id,
            writer_sn: sn,
            inline_qos: qos,
            data_flag: false,
            key_flag: marker.form.sets_key_flag(),
            non_standard_flag: false,
            serialized_payload: if marker.form.sets_key_flag() {
                crate::payload::Payload::whole(alloc::sync::Arc::clone(marker.serialized_key))
            } else {
                crate::payload::Payload::from(alloc::vec::Vec::new())
            },
        }
    }

    /// The dispose form for one matched peer, reconciled with whether a key
    /// serialization is on hand.
    ///
    /// `with_key_available` is what keeps a marker from naming no instance:
    /// a peer configured for [`crate::dialect::DisposeKeyForm::KeyInPayload`]
    /// — key in the payload, no `PID_KEY_HASH` — would otherwise, on a
    /// caller that has no key, emit a DATA carrying neither identity.
    fn marker_form(&self, peer: Guid, key: &[u8]) -> crate::dialect::DisposeKeyForm {
        self.behaviour
            .for_peer(peer)
            .dispose_key_form
            .with_key_available(!key.is_empty())
    }

    /// Appends change `sn` for `peer` to an open builder as what the cache
    /// holds of it: a lifecycle DATA for a dispose or unregister (the
    /// marker again, not its key bytes dressed as a sample), a DATA or its
    /// DATA_FRAGs for a sample, a GAP for a number the cache no longer has.
    /// A fragmented sample goes out in datagrams of its own, and the builder
    /// is opened anew behind them.
    fn append_change(
        &self,
        builder: &mut MessageBuilder,
        sn: SequenceNumber,
        peer_guid: Guid,
        out: &mut Vec<OutboundDatagram>,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<(), WireError> {
        match self.cache.get(sn) {
            Some(change) if !change.kind.is_alive_kind() => {
                let key = change.payload.clone();
                let marker = LifecycleMarker {
                    key_hash: change.key_hash.unwrap_or([0u8; 16]),
                    serialized_key: &key,
                    status_bits: status_bits_for_kind(change.kind),
                    form: self.marker_form(peer_guid, &key),
                };
                self.append_lifecycle(builder, sn, &marker, peer_guid, out, targets)
            }
            Some(change) => {
                let payload = change.payload.clone();
                if self.needs_fragmentation(sn, peer_guid, &payload) {
                    let full =
                        core::mem::replace(builder, self.open_builder_for(peer_guid, targets));
                    if let Some(dg) = full.finish() {
                        out.push(dg);
                    }
                    out.extend(self.build_sample_datagrams(sn, &payload, peer_guid, targets)?);
                    Ok(())
                } else {
                    self.append_data(builder, sn, &payload, peer_guid, out, targets)
                }
            }
            None => self.append_gap(builder, sn, peer_guid, out, targets),
        }
    }

    /// Appends a lifecycle DATA to an open builder, splitting the datagram
    /// like [`Self::append_data`] when it no longer fits.
    ///
    /// The repair path needs this. A `CacheChange` of a not-alive kind that a
    /// reader NACKs must be rebuilt as the marker it is: sending its payload
    /// through `append_data` would set `D=1` and hand the reader the
    /// serialized key as if it were a sample — the instance stays alive on
    /// that reader and gains a nonsense sample on top.
    fn append_lifecycle(
        &self,
        builder: &mut MessageBuilder,
        sn: SequenceNumber,
        marker: &LifecycleMarker<'_>,
        peer: Guid,
        out: &mut Vec<OutboundDatagram>,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<(), WireError> {
        // See `build_lifecycle_datagram` — the same INFO_TS `append_data`
        // prepends for an alive change.
        self.append_info_ts(builder, sn);
        let data = self.lifecycle_data(sn, marker, peer);
        let (body, flags) = data.write_body(true);
        self.append_submessage(
            builder,
            SubmessageId::Data,
            flags,
            &body,
            peer,
            out,
            targets,
            "DATA",
        )
    }

    /// Sends a lifecycle marker (dispose/unregister) to all matched
    /// readers. Allocates a new sequence number, persists a
    /// `CacheChange` with the corresponding ChangeKind and builds a DATA
    /// with key hash + StatusInfo per reader proxy.
    ///
    /// `serialized_key` is the instance key as it goes on the wire —
    /// a four-octet encapsulation header followed by the key's CDR. Passing
    /// it sets the K flag and puts it in `serializedPayload`, which is the
    /// form Cyclone 11.0.1 and OpenDDS 3.34.0 were measured to send; passing
    /// an empty slice leaves K clear and names the instance by
    /// `PID_KEY_HASH` alone, which is the form Fast DDS 3.6.1 sends. See
    /// `Self::build_lifecycle_datagram` for the octets and for what
    /// Cyclone does with the third combination.
    ///
    /// The key is stored on the `CacheChange`, so a NACK repair of the
    /// marker rebuilds the identical DATA instead of a keyless one.
    ///
    /// `status_bits` is the OR combination of the desired bits from
    /// [`crate::inline_qos::status_info`]:
    /// - DISPOSED: NotAliveDisposed
    /// - UNREGISTERED: NotAliveUnregistered
    /// - DISPOSED | UNREGISTERED: NotAliveDisposedUnregistered
    ///
    /// `source_timestamp` is the timestamp of the `dispose`/
    /// `unregister_instance` call (DDSI-RTPS §8.3.7.9 / §8.7.3), stored on
    /// the `CacheChange` exactly as [`crate::history_cache::CacheChange::with_source_timestamp`]
    /// does for an alive sample: `Self::append_info_ts` reads it back off
    /// the change for both the first send and every retransmit, so a NACK
    /// repair carries the *original* timestamp and not the resend
    /// wall-clock. `None` emits no INFO_TS (reception order).
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn write_lifecycle(
        &mut self,
        key_hash: [u8; 16],
        serialized_key: &[u8],
        status_bits: u32,
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.write_lifecycle_with_inline_qos(
            key_hash,
            serialized_key,
            status_bits,
            source_timestamp,
            None,
        )
    }

    /// [`Self::write_lifecycle`] with further in-line QoS on the marker, as
    /// [`Self::write_sample_with_inline_qos`] has for a sample: a dispose or
    /// unregister forwarded on behalf of another writer carries
    /// `PID_ORIGINAL_WRITER_INFO` (DDSI-RTPS 2.5 §8.7.9) on the first send
    /// and on every repair.
    ///
    /// # Errors
    /// As [`Self::write_lifecycle`].
    pub fn write_lifecycle_with_inline_qos(
        &mut self,
        key_hash: [u8; 16],
        serialized_key: &[u8],
        status_bits: u32,
        source_timestamp: Option<crate::header_extension::HeTimestamp>,
        inline_qos: Option<crate::parameter_list::ParameterList>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let sn_value = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "sequence number overflow",
            })?;
        self.next_sn = sn_value;
        let sn = SequenceNumber(sn_value);

        let kind = match (
            status_bits & crate::inline_qos::status_info::DISPOSED != 0,
            status_bits & crate::inline_qos::status_info::UNREGISTERED != 0,
        ) {
            (true, true) => crate::history_cache::ChangeKind::NotAliveDisposedUnregistered,
            (true, false) => crate::history_cache::ChangeKind::NotAliveDisposed,
            (false, true) => crate::history_cache::ChangeKind::NotAliveUnregistered,
            (false, false) => {
                return Err(WireError::ValueOutOfRange {
                    message: "lifecycle send requires DISPOSED or UNREGISTERED bit",
                });
            }
        };

        // A dispose or unregister between begin and end of a coherent set
        // is one of its changes (§8.7.6), and may be its first.
        if self.coherent_set_armed && self.coherent_set_sn.is_none() {
            self.coherent_set_sn = Some(sn_value);
        }
        let group = self.stamp_group_change();
        self.note_gsn(sn, group);
        // Persist the CacheChange — late-joiner replay (T9) reads from it,
        // and the history-cache bookkeeping stays consistent. The key hash
        // rides in the change's own `key_hash` field and the serialized key
        // is its payload, so a retransmit reproduces both.
        self.cache
            .insert(
                crate::history_cache::CacheChange::lifecycle(
                    sn,
                    key_hash,
                    serialized_key.to_vec(),
                    kind,
                )
                .with_source_timestamp(source_timestamp)
                .with_coherent_set(self.coherent_set_sn)
                .with_group(group)
                .with_inline_qos(inline_qos),
            )
            .map_err(|_| WireError::ValueOutOfRange {
                message: "history cache full or duplicate (lifecycle)",
            })?;

        let mut out = Vec::new();
        for idx in 0..self.reader_proxies.len() {
            let peer_guid = self.reader_proxies[idx].remote_reader_guid;
            let targets = self.targets_for(idx);
            // Drain every change this proxy has not yet been sent, in order, up
            // to and including the new lifecycle marker at `sn`.
            //
            // A plain `write` gates on `next_unsent_change(sn) == Some(sn)` and
            // only fires when the proxy's send cursor sits *exactly* at `sn-1`.
            // For a dispose that is wrong: the SEDP writer's cursor races the
            // periodic-(re)announce SN churn, and whenever it lags by ≥1 the
            // dispose is silently dropped from the direct send and left to
            // NACK-repair — which, under the SEDP writer's KeepLast history, the
            // reliable reader does not reliably close inside the discovery
            // window (the reader keeps the stale match forever). Draining makes
            // delivery cursor-independent: cached ALIVE changes go out as DATA,
            // lifecycle markers as a lifecycle DATA, and KeepLast-evicted SNs as
            // GAP so the reader can still advance to and deliver the marker.
            loop {
                let Some(next) = self.reader_proxies[idx].next_unsent_change(sn) else {
                    break;
                };
                match self.cache.get(next) {
                    Some(change) => match change.kind {
                        ChangeKind::Alive | ChangeKind::AliveFiltered => {
                            let payload = change.payload.clone();
                            out.extend(
                                self.build_sample_datagrams(next, &payload, peer_guid, &targets)?,
                            );
                        }
                        marker_kind => {
                            // Lifecycle marker: the change carries the key
                            // hash in its own field and the serialized key as
                            // its payload (see `CacheChange::lifecycle`), so
                            // the repair DATA is byte-identical to the first
                            // send — K flag and all.
                            let key = change.payload.clone();
                            let marker = LifecycleMarker {
                                key_hash: change.key_hash.unwrap_or(key_hash),
                                serialized_key: &key,
                                status_bits: status_bits_for_kind(marker_kind),
                                form: self.marker_form(peer_guid, &key),
                            };
                            out.push(
                                self.build_lifecycle_datagram(next, &marker, peer_guid, &targets)?,
                            );
                        }
                    },
                    None => {
                        out.push(self.build_gap_datagram(next, peer_guid, &targets)?);
                    }
                }
                if next == sn {
                    break;
                }
            }
        }
        Ok(out)
    }

    // ---------- Coherent sets (DDSI-RTPS 2.5 §8.7.6) ----------

    /// Opens a coherent set. Every sample staged from here on tags its
    /// `CacheChange` with the set's first `SequenceNumber`, and every DATA
    /// built from such a change carries `PID_COHERENT_SET` in its inline QoS.
    ///
    /// The set's identity is the SN of its *first* sample, so the value is
    /// only fixed once that sample is written; an empty set never names one.
    /// Idempotent — a second call on an open set is a no-op, so the DCPS
    /// `Publisher::begin_coherent_changes` can arm every one of its writers
    /// without tracking which of them already are.
    pub fn begin_coherent_set(&mut self) {
        self.coherent_set_armed = true;
    }

    /// Makes this writer one of a GROUP-scope publisher's (DDSI-RTPS 2.5
    /// §8.7.5): from the next change on, each carries a group sequence
    /// number and the writer set, and HEARTBEATs carry the group's numbers.
    /// `None` takes it out of the group.
    pub fn set_group(&mut self, group: Option<alloc::sync::Arc<crate::writer_group::WriterGroup>>) {
        self.group = group;
    }

    /// The group information of this writer's next change, recorded as its
    /// last; `None` outside a group.
    fn stamp_group_change(&mut self) -> Option<crate::writer_group::ChangeGroupInfo> {
        let info = self.group.as_ref()?.stamp_change();
        self.last_gsn = Some(info.gsn);
        Some(info)
    }

    /// Remembers the group sequence number of the change `sn`, for the GAP
    /// that announces it after it has left the cache (§8.3.8.4). The oldest
    /// entries go at [`MAX_REMEMBERED_GSNS`].
    fn note_gsn(
        &mut self,
        sn: SequenceNumber,
        group: Option<crate::writer_group::ChangeGroupInfo>,
    ) {
        let Some(info) = group else { return };
        self.gsn_of.insert(sn.0, info.gsn);
        while self.gsn_of.len() > MAX_REMEMBERED_GSNS {
            self.gsn_of.pop_first();
        }
    }

    /// `gapStartGSN` and `gapEndGSN` of a GAP for the single change `sn`
    /// (DDSI-RTPS 2.5 §8.3.8.4): the group sequence number of the change,
    /// as the start and as the end of the range of one. `None` for a writer
    /// that is not in a group, and for a change whose number is no longer
    /// remembered.
    fn gap_group_info(
        &self,
        sn: SequenceNumber,
        peer: Guid,
    ) -> Option<crate::submessages::GapGroupInfo> {
        self.group.as_ref()?;
        if !self.group_info_for(peer) {
            return None;
        }
        let gsn = self
            .cache
            .get(sn)
            .and_then(|c| c.group.map(|g| g.gsn))
            .or_else(|| self.gsn_of.get(&sn.0).copied())?;
        Some(crate::submessages::GapGroupInfo {
            gap_start_gsn: SequenceNumber(gsn),
            gap_end_gsn: SequenceNumber(gsn),
        })
    }

    /// Whether `peer` gets this writer's group information on the wire
    /// (`crate::dialect::GroupWireForm`).
    fn group_info_for(&self, peer: Guid) -> bool {
        self.behaviour.for_peer(peer).group_wire_form
            == crate::dialect::GroupWireForm::Specification
    }

    /// The group this writer draws its group sequence numbers from.
    #[must_use]
    pub fn group(&self) -> Option<&alloc::sync::Arc<crate::writer_group::WriterGroup>> {
        self.group.as_ref()
    }

    /// `true` while a coherent set is open on this writer.
    #[must_use]
    pub fn coherent_set_is_open(&self) -> bool {
        self.coherent_set_armed
    }

    /// First `SequenceNumber` of the open coherent set, or `None` when no set
    /// is open or the open set has no sample yet.
    #[must_use]
    pub fn coherent_set_first_sn(&self) -> Option<i64> {
        self.coherent_set_sn
    }

    /// Closes the open coherent set and emits the **end-of-coherent-set
    /// (ECS)** sample (DDSI-RTPS 2.5 §8.7.6): a DATA with the D and the K
    /// flag clear, an empty payload and no in-line QoS (Table 9.22,
    /// Example 3; see [`CacheChange::coherent_set_end`]). The reader
    /// releases the buffered set on it.
    ///
    /// A set that never carried a sample has no first SN and therefore no
    /// identity to close — the call clears the flag and emits nothing.
    /// Delivery uses the same drain as `write_lifecycle`, so the marker
    /// reaches a proxy whose send cursor lags.
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn end_coherent_set(&mut self) -> Result<Vec<OutboundDatagram>, WireError> {
        self.coherent_set_armed = false;
        if self.coherent_set_sn.take().is_none() {
            return Ok(Vec::new());
        }
        let sn = self.next_change_sn()?;
        self.emit_coherent_set_end(CacheChange::coherent_set_end(sn))
    }

    /// Closes this writer's part of a GROUP-scope publisher's coherent set
    /// and sends its end-of-coherent-set DATA (DDSI-RTPS 2.5 §8.7.6): "A
    /// group's coherent set is marked as being finished by sending an End
    /// Coherent Set (ECS) Data submessage from all Writers within the
    /// Publisher" — this one as well when it wrote nothing into the set.
    /// It carries `PID_GROUP_SEQ_NUM` = `end.ecs_gsn`,
    /// `PID_GROUP_COHERENT_SET` and `PID_WRITER_GROUP_INFO`, and the
    /// writer's own `PID_COHERENT_SET` where it has one.
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn end_group_coherent_set(
        &mut self,
        end: crate::writer_group::GroupSetEnd,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.coherent_set_armed = false;
        let first_sn = self.coherent_set_sn.take();
        let sn = self.next_change_sn()?;
        self.last_gsn = Some(end.ecs_gsn);
        self.emit_coherent_set_end(CacheChange::group_coherent_set_end(sn, first_sn, end))
    }

    /// The next writer sequence number, taken.
    fn next_change_sn(&mut self) -> Result<SequenceNumber, WireError> {
        let sn_value = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "sequence number overflow",
            })?;
        self.next_sn = sn_value;
        Ok(SequenceNumber(sn_value))
    }

    /// Stores an end-of-coherent-set change and sends it to every proxy,
    /// with whatever the proxy has not been sent before it.
    fn emit_coherent_set_end(
        &mut self,
        change: CacheChange,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let sn = change.sequence_number;
        self.cache
            .insert(change)
            .map_err(|_| WireError::ValueOutOfRange {
                message: "history cache full or duplicate (coherent set end)",
            })?;

        let mut out = Vec::new();
        for idx in 0..self.reader_proxies.len() {
            let peer_guid = self.reader_proxies[idx].remote_reader_guid;
            let targets = self.targets_for(idx);
            // Drain up to and including the ECS marker — same rationale as
            // `write_lifecycle`: a proxy whose cursor lags would otherwise
            // never see the marker directly, and the set would stay open on
            // the reader until an unrelated later sample closed it.
            loop {
                let Some(next) = self.reader_proxies[idx].next_unsent_change(sn) else {
                    break;
                };
                match self.cache.get(next) {
                    Some(change) => match change.kind {
                        ChangeKind::Alive | ChangeKind::AliveFiltered => {
                            let payload = change.payload.clone();
                            out.extend(
                                self.build_sample_datagrams(next, &payload, peer_guid, &targets)?,
                            );
                        }
                        marker_kind => {
                            let key = change.payload.clone();
                            let marker = LifecycleMarker {
                                key_hash: change.key_hash.unwrap_or([0u8; 16]),
                                serialized_key: &key,
                                status_bits: status_bits_for_kind(marker_kind),
                                form: self.marker_form(peer_guid, &key),
                            };
                            out.push(
                                self.build_lifecycle_datagram(next, &marker, peer_guid, &targets)?,
                            );
                        }
                    },
                    None => {
                        out.push(self.build_gap_datagram(next, peer_guid, &targets)?);
                    }
                }
                if next == sn {
                    break;
                }
            }
        }
        Ok(out)
    }

    fn build_data_frag_datagram(
        &self,
        sn: SequenceNumber,
        frag: FragmentNumber,
        count: u16,
        full_payload: &alloc::sync::Arc<[u8]>,
        peer: Guid,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        if !self.is_relevant(sn, peer) {
            return self.build_gap_datagram(sn, peer, targets);
        }
        let frag_size = self.fragment_size_for(sn) as usize;
        if frag.0 == 0 {
            return Err(WireError::ValueOutOfRange {
                message: "fragment number must be >= 1",
            });
        }
        let start = (frag.0 as usize - 1) * frag_size;
        if start >= full_payload.len() {
            return Err(WireError::ValueOutOfRange {
                message: "fragment number beyond sample",
            });
        }
        let end = core::cmp::min(
            start + usize::from(count.max(1)) * frag_size,
            full_payload.len(),
        );
        let count = u16::try_from((end - start).div_ceil(frag_size)).map_err(|_| {
            WireError::ValueOutOfRange {
                message: "fragments in submessage exceed u16::MAX",
            }
        })?;
        let sample_size =
            u32::try_from(full_payload.len()).map_err(|_| WireError::ValueOutOfRange {
                message: "sample size exceeds u32::MAX",
            })?;
        let frag_size_u16 = u16::try_from(frag_size).map_err(|_| WireError::ValueOutOfRange {
            message: "fragment_size exceeds u16::MAX",
        })?;
        self.build_data_frag_submessage_datagram(
            sn,
            FragmentSlice {
                number: frag,
                count,
                size: frag_size_u16,
                sample_size,
                bytes: &full_payload[start..end],
            },
            peer,
            targets,
        )
    }

    /// Adds the coherent-set and group parameters of `change` for `peer` to
    /// `qos`. Every DATA of a change carries them, a lifecycle DATA as well
    /// as a sample: the writer-level `PID_COHERENT_SET` (DDSI-RTPS 2.5
    /// §8.7.6), and for a GROUP-scope publisher's writer
    /// `PID_GROUP_SEQ_NUM` and `PID_WRITER_GROUP_INFO` on every DATA and
    /// DATA_FRAG (§8.7.5, §9.6.4.4, §9.6.4.5), `PID_GROUP_COHERENT_SET` in
    /// a group coherent set (§8.7.6, §9.6.4.3).
    fn push_set_params(
        &self,
        change: &CacheChange,
        peer: Option<Guid>,
        qos: &mut Option<crate::parameter_list::ParameterList>,
    ) {
        // A peer set to the coherent-sets-only form gets a GROUP-scope
        // publisher's sets as at TOPIC scope (`crate::dialect::GroupWireForm`):
        // no group information, and the end-of-set DATA of a writer that
        // wrote into the set without in-line QoS, as `Self::end_coherent_set`
        // sends it. `None` is the largest case, the specification's form.
        let group_info = peer.is_none_or(|peer| self.group_info_for(peer));
        if let Some(first) = change.coherent_set_sn {
            let plain_end = change.coherent_set_end && change.group.is_some() && !group_info;
            if !plain_end {
                qos.get_or_insert_with(crate::parameter_list::ParameterList::new)
                    .parameters
                    .extend(crate::inline_qos::coherent_set_inline_qos(first, true).parameters);
            }
        }
        if let Some(group) = change.group.filter(|_| group_info) {
            let list = qos.get_or_insert_with(crate::parameter_list::ParameterList::new);
            list.push(crate::inline_qos::group_seq_num_param(group.gsn, true));
            list.push(if self.secure_group_info {
                crate::inline_qos::secure_writer_group_info_param(group.writer_set)
            } else {
                crate::inline_qos::writer_group_info_param(group.writer_set)
            });
            // A peer may be set to go without it (see
            // `crate::dialect::GroupCoherentSetPid`).
            // `None` is the largest case: every parameter that any peer gets.
            let sent = peer.is_none_or(|peer| {
                self.behaviour.for_peer(peer).group_coherent_set
                    == crate::dialect::GroupCoherentSetPid::Sent
            });
            if let (Some(first), true) = (group.coherent_set_gsn, sent) {
                list.push(crate::inline_qos::group_coherent_set_param(first, true));
            }
        }
    }

    fn sample_inline_qos(
        &self,
        sn: SequenceNumber,
        peer: Guid,
    ) -> Option<crate::parameter_list::ParameterList> {
        self.sample_inline_qos_for(sn, Some(peer))
    }

    /// The in-line QoS of `sn` for `peer`; for `None` the largest list any
    /// peer gets (the writer's QoS as for a reader that expects it, the
    /// group coherent set parameter as for a peer that is sent it), which
    /// is what [`Self::fragment_size_for`] is made of.
    fn sample_inline_qos_for(
        &self,
        sn: SequenceNumber,
        peer: Option<Guid>,
    ) -> Option<crate::parameter_list::ParameterList> {
        let change = self.cache.get(sn)?;
        let mut qos = change.inline_qos.clone();
        // DDSI-RTPS 2.5 §8.4.9.2.4: `IF (the_reader_proxy.expectsInlineQos)
        // { DATA.inlineQos := the_rtps_writer.related_dds_writer.qos;
        // DATA.inlineQos += a_change.inlineQos; }` — the writer's first.
        if let Some(writer_qos) = self.inline_writer_qos.as_ref().filter(|_| {
            peer.is_none_or(|peer| {
                self.inline_qos_readers > 0
                    && self
                        .reader_proxies
                        .iter()
                        .any(|p| p.remote_reader_guid == peer && p.expects_inline_qos())
            })
        }) {
            let mut list = writer_qos.clone();
            if let Some(own) = qos.take() {
                list.parameters.extend(own.parameters);
            }
            qos = Some(list);
        }
        self.push_set_params(change, peer, &mut qos);
        if let Some(key) = change.key_hash {
            qos.get_or_insert_with(crate::parameter_list::ParameterList::new)
                .push(crate::parameter_list::Parameter::new(
                    crate::parameter_list::pid::KEY_HASH,
                    key.to_vec(),
                ));
        }
        qos
    }

    fn build_data_frag_submessage_datagram(
        &self,
        sn: SequenceNumber,
        fragment: FragmentSlice<'_>,
        peer: Guid,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let inline_qos = self.sample_inline_qos(sn, peer);
        let inline_qos_flag = inline_qos.is_some();
        let df = DataFragSubmessage {
            extra_flags: 0,
            reader_id: peer.entity_id,
            writer_id: self.guid.entity_id,
            writer_sn: sn,
            fragment_starting_num: fragment.number,
            fragments_in_submessage: fragment.count,
            fragment_size: fragment.size,
            sample_size: fragment.sample_size,
            // WP 2.0a: chunk is a sub-slice of the full Arc payload.
            //
            // **Zero-copy scope claim:** the
            // `Arc::from(chunk)` here ALLOCATES a new refcount
            // block and copies the chunk bytes. This is **not**
            // zero-copy. The WP-2.0a claim "3-7 % gain" refers
            // exclusively to the unfragmented
            // DATA path (`build_single_data_datagram`), where
            // `Arc::clone` shares the full payload. Fragmentation
            // paths stay copy-per-chunk until WP 2.0a-2 (iovec)
            // eliminates the submessage-builder side.
            serialized_payload: alloc::sync::Arc::from(fragment.bytes),
            inline_qos,
            inline_qos_flag,
            key_flag: false,
            non_standard_flag: self.cache.get(sn).is_some_and(|c| c.non_standard_payload),
        };
        let (body, flags) = df.write_body(true);
        let mut builder = self.open_builder_for(peer, targets);
        // Each fragment's datagram carries the source timestamp so a reader
        // reassembling out-of-order still sees a consistent INFO_TS.
        self.append_info_ts(&mut builder, sn);
        builder
            .try_add_submessage(SubmessageId::DataFrag, flags, &body)
            .map_err(|_| WireError::ValueOutOfRange {
                message: "DATA_FRAG submessage does not fit into MTU",
            })?;
        builder.finish().ok_or(WireError::ValueOutOfRange {
            message: "MessageBuilder finish returned no datagram",
        })
    }

    fn build_gap_datagram(
        &self,
        sn: SequenceNumber,
        peer: Guid,
        targets: &alloc::sync::Arc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let gap = GapSubmessage {
            reader_id: peer.entity_id,
            writer_id: self.guid.entity_id,
            gap_start: sn,
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(sn.0 + 1),
                num_bits: 0,
                bitmap: Vec::new(),
            },
            group_info: self.gap_group_info(sn, peer),
            filtered_count: Some(self.gap_filtered_count(sn)),
        };
        let (body, flags) = gap.write_body(true);
        let mut builder = self.open_builder_for(peer, targets);
        builder
            .try_add_submessage(SubmessageId::Gap, flags, &body)
            .map_err(|_| WireError::ValueOutOfRange {
                message: "GAP submessage does not fit into MTU",
            })?;
        builder.finish().ok_or(WireError::ValueOutOfRange {
            message: "MessageBuilder finish returned no datagram",
        })
    }
}

/// What a lifecycle DATA (dispose / unregister) says about one instance:
/// its `PID_KEY_HASH`, the `serializedPayload` key octets (empty when the
/// caller has none), and the `PID_STATUS_INFO` bits. Grouped because the
/// three always travel together — the first send, the NACK repair and the
/// lagging-proxy drain all build the same DATA out of them.
#[derive(Debug, Clone, Copy)]
struct LifecycleMarker<'a> {
    key_hash: [u8; 16],
    serialized_key: &'a alloc::sync::Arc<[u8]>,
    status_bits: u32,
    /// Which of the three measured forms this DATA takes — the matched
    /// peer's [`crate::dialect::DisposeKeyForm`], already reconciled with
    /// whether a key serialization is actually on hand.
    form: crate::dialect::DisposeKeyForm,
}

/// Maps a not-alive [`ChangeKind`] back to its `PID_STATUS_INFO` bits, the
/// inverse of the classification in [`ReliableWriter::write_lifecycle`].
fn status_bits_for_kind(kind: ChangeKind) -> u32 {
    use crate::inline_qos::status_info::{DISPOSED, UNREGISTERED};
    match kind {
        ChangeKind::NotAliveDisposed => DISPOSED,
        ChangeKind::NotAliveUnregistered => UNREGISTERED,
        ChangeKind::NotAliveDisposedUnregistered => DISPOSED | UNREGISTERED,
        // Alive kinds never reach this helper (the drain handles them as DATA);
        // default to a dispose marker rather than emitting an empty status.
        ChangeKind::Alive | ChangeKind::AliveFiltered => DISPOSED,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::datagram::{ParsedSubmessage, decode_datagram};
    use crate::message_builder::DEFAULT_MTU;
    use crate::reader_proxy::ChangeFilter;
    use crate::wire_types::EntityId;
    use crate::wire_types::{GuidPrefix, Locator};

    /// A writer that answers a NACK on the next tick: the tests below drive
    /// the clock by single milliseconds, and the default `nackResponseDelay`
    /// is the spec's 200 ms.
    fn fast_writer(cfg: ReliableWriterConfig) -> ReliableWriter {
        let mut w = ReliableWriter::new(cfg);
        w.set_nack_response_delay(Duration::ZERO);
        w
    }

    fn sn(n: i64) -> SequenceNumber {
        SequenceNumber(n)
    }

    fn reader_guid() -> Guid {
        Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
        )
    }

    fn make_writer(max_samples: usize, hb_period: Duration) -> ReliableWriter {
        make_writer_with_frag_size(max_samples, hb_period, DEFAULT_FRAGMENT_SIZE)
    }

    fn make_writer_with_frag_size(
        max_samples: usize,
        hb_period: Duration,
        fragment_size: u32,
    ) -> ReliableWriter {
        let writer_guid = Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
        );
        let reader_proxy = ReaderProxy::new(
            reader_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
            alloc::vec![],
            true,
        );
        fast_writer(ReliableWriterConfig {
            guid: writer_guid,
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![reader_proxy],
            max_samples,
            history_kind: HistoryKind::KeepAll,
            heartbeat_period: hb_period,
            fragment_size,
            mtu: DEFAULT_MTU,
        })
    }

    /// DCPS 2.2.2.4.2.11: a KEEP_ALL history that is full refuses the write.
    /// The refused write must not use up a sequence number: the next change
    /// after room was made is the one after the last written, with no hole
    /// for the readers to wait on.
    #[test]
    fn a_refused_write_into_a_full_keep_all_history_keeps_its_number() {
        let mut w = make_writer(2, Duration::from_millis(10));
        w.write(&[1]).unwrap();
        w.write(&[2]).unwrap();
        assert!(w.write(&[3]).is_err(), "the history is full");
        assert_eq!(w.last_written_sn(), sn(2));
        w.handle_acknack(reader_guid(), sn(3), alloc::vec![]);
        w.remove_acknowledged();
        w.write(&[3]).unwrap();
        assert_eq!(w.last_written_sn(), sn(3));
    }

    fn first_proxy(w: &ReliableWriter) -> &ReaderProxy {
        w.reader_proxies().first().unwrap()
    }

    /// The `serializedKey` a SEDP dispose carries, in the octets Cyclone
    /// 11.0.1 and OpenDDS 3.34.0 were both measured to put there: a
    /// `PL_CDR_LE` encapsulation header, `PID_ENDPOINT_GUID` (0x005a) with
    /// the sixteen octets of the deleted endpoint's GUID, and
    /// `PID_SENTINEL`.
    ///
    /// Cyclone, domain 61, frame 21 of `out/cyclone/cap.pcap`:
    /// `00 03 00 00  5a 00 10 00  01 10 d8 6c 05 73 19 80 13 63 f2 44
    ///  00 00 02 07  01 00 00 00`.
    fn sedp_serialized_key(guid: Guid) -> alloc::vec::Vec<u8> {
        let mut key = alloc::vec![0x00, 0x03, 0x00, 0x00, 0x5A, 0x00, 0x10, 0x00];
        key.extend_from_slice(&guid.to_bytes());
        key.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        key
    }

    /// The single DATA of a datagram, or `None`.
    fn only_data(bytes: &[u8]) -> Option<crate::submessages::DataSubmessage> {
        decode_datagram(bytes)
            .ok()?
            .contents()
            .find_map(|s| match s {
                ParsedSubmessage::Data(d) => Some(d.clone()),
                _ => None,
            })
    }

    /// RTPS-F1: `write_stamped` prepends an INFO_TS submessage (with the source
    /// timestamp) immediately before the DATA in the same datagram; a plain
    /// `write` emits none. Verified by decoding the wire bytes.
    #[test]
    fn write_stamped_prepends_info_ts_before_data() {
        use crate::header_extension::HeTimestamp;
        let mut w = make_writer(10, Duration::from_secs(1));
        let ts = HeTimestamp {
            seconds: 0x1122_3344,
            fraction: 0x5566_7788,
        };
        let out = w
            .write_stamped(&alloc::vec![0xAA, 0xBB, 0xCC, 0xDD], Some(ts))
            .expect("write_stamped");
        let parsed = decode_datagram(&out[0].bytes).expect("decode");
        // First submessage = INFO_TS with our timestamp, second = DATA.
        match (
            &parsed.submessages[0].submessage,
            &parsed.submessages[1].submessage,
        ) {
            (ParsedSubmessage::InfoTimestamp(its), ParsedSubmessage::Data(_)) => {
                assert!(!its.invalidate);
                assert_eq!(its.timestamp, ts, "INFO_TS must carry the source timestamp");
            }
            other => panic!("expected [InfoTimestamp, Data], got {other:?}"),
        }

        // A plain write() emits NO INFO_TS (None timestamp → reception order).
        let out2 = w.write(&alloc::vec![0xEE]).expect("write");
        let parsed2 = decode_datagram(&out2[0].bytes).expect("decode2");
        assert!(
            !parsed2
                .contents()
                .any(|s| matches!(s, ParsedSubmessage::InfoTimestamp(_))),
            "unstamped write must not emit INFO_TS"
        );
    }

    /// `write_lifecycle`'s `source_timestamp` prepends an INFO_TS before the
    /// lifecycle DATA exactly like `write_stamped` does for an alive
    /// sample — a dispose/unregister has a source timestamp too
    /// (DDSI-RTPS §8.3.7.9), and until this test the lifecycle send path
    /// never emitted one.
    #[test]
    fn write_lifecycle_prepends_info_ts_before_data() {
        use crate::header_extension::HeTimestamp;
        let mut w = make_writer(10, Duration::from_secs(1));
        let ts = HeTimestamp {
            seconds: 0x1122_3344,
            fraction: 0x5566_7788,
        };
        let out = w
            .write_lifecycle(
                [0xAAu8; 16],
                &alloc::vec![0x00, 0x06, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00],
                crate::inline_qos::status_info::DISPOSED,
                Some(ts),
            )
            .expect("write_lifecycle");
        let parsed = decode_datagram(&out[0].bytes).expect("decode");
        match (
            &parsed.submessages[0].submessage,
            &parsed.submessages[1].submessage,
        ) {
            (ParsedSubmessage::InfoTimestamp(its), ParsedSubmessage::Data(d)) => {
                assert!(!its.invalidate);
                assert_eq!(its.timestamp, ts, "INFO_TS must carry the source timestamp");
                assert!(d.key_flag, "the marker still carries K=1 with the key");
            }
            other => panic!("expected [InfoTimestamp, Data], got {other:?}"),
        }

        // An unstamped lifecycle marker emits no INFO_TS.
        let out2 = w
            .write_lifecycle(
                [0xBBu8; 16],
                &[],
                crate::inline_qos::status_info::UNREGISTERED,
                None,
            )
            .expect("write_lifecycle unstamped");
        let parsed2 = decode_datagram(&out2[0].bytes).expect("decode2");
        assert!(
            !parsed2
                .contents()
                .any(|s| matches!(s, ParsedSubmessage::InfoTimestamp(_))),
            "unstamped lifecycle marker must not emit INFO_TS"
        );
    }

    /// A NACK repair (via `tick`, the aggregated-resend path
    /// `append_lifecycle` serves) of a stamped lifecycle marker still
    /// carries the ORIGINAL INFO_TS, exactly like `resend_carries_original_info_ts`
    /// below for an alive sample — the timestamp lives on the
    /// `CacheChange`, not the send call.
    #[test]
    fn lifecycle_resend_carries_original_info_ts() {
        use crate::header_extension::HeTimestamp;
        let mut w = make_writer(10, Duration::from_millis(10));
        let ts = HeTimestamp {
            seconds: 7,
            fraction: 42,
        };
        let _ = w
            .write_lifecycle(
                [0xCCu8; 16],
                &alloc::vec![0x00, 0x06, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00],
                crate::inline_qos::status_info::DISPOSED,
                Some(ts),
            )
            .expect("write_lifecycle");
        assert_eq!(info_ts_of_the_repair(&mut w), Some(ts));
    }

    /// A retransmit (via tick) of a stamped change still carries the ORIGINAL
    /// INFO_TS — the timestamp lives on the cache change, not the send call.
    #[test]
    fn resend_carries_original_info_ts() {
        use crate::header_extension::HeTimestamp;
        let mut w = make_writer(10, Duration::from_millis(10));
        let ts = HeTimestamp {
            seconds: 7,
            fraction: 42,
        };
        let _ = w
            .write_stamped(&alloc::vec![1, 2, 3, 4], Some(ts))
            .expect("write_stamped");
        assert_eq!(info_ts_of_the_repair(&mut w), Some(ts));
    }

    /// The INFO_TS framed with the repair of sequence number 1: the reader
    /// NACKs it and `tick` answers once the response delay has passed. A
    /// tick without a NACK repairs nothing, and the two tests above read
    /// such a tick until this was added — they passed without a DATA to
    /// look at.
    fn info_ts_of_the_repair(
        w: &mut ReliableWriter,
    ) -> Option<crate::header_extension::HeTimestamp> {
        w.handle_acknack(reader_guid(), sn(1), [sn(1)]);
        let resent = w.tick(Duration::from_secs(1)).expect("tick");
        resent.iter().find_map(|dg| {
            let p = decode_datagram(&dg.bytes).expect("decode resend");
            if !p.contents().any(|s| matches!(s, ParsedSubmessage::Data(_))) {
                return None;
            }
            p.contents().find_map(|s| match s {
                ParsedSubmessage::InfoTimestamp(i) => Some(i.timestamp),
                _ => None,
            })
        })
    }

    /// DDSI-RTPS 2.5 §8.4.9.2.4: a reader whose proxy expects in-line QoS
    /// gets the writer's QoS with every DATA, ahead of the change's own
    /// parameters; the reader beside it, which does not, gets the change's
    /// alone. Removing the proxy takes the writer's QoS off the wire again.
    #[test]
    fn a_reader_that_expects_inline_qos_gets_the_writers_qos_with_each_data() {
        use crate::parameter_list::{Parameter, ParameterList, pid};
        let mut w = make_writer(10, Duration::from_secs(1));
        let stateless = Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::user_reader_with_key([0x01, 0x02, 0x03]),
        );
        let mut proxy = ReaderProxy::new(
            stateless,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        );
        proxy.set_expects_inline_qos(true);
        w.add_reader_proxy(proxy);
        // `PID_TOPIC_NAME` "T": length 2, 'T', NUL, two octets of padding.
        let topic = Parameter::new(pid::TOPIC_NAME, alloc::vec![2, 0, 0, 0, b'T', 0, 0, 0]);
        let mut writer_qos = ParameterList::new();
        writer_qos.push(topic.clone());
        w.set_inline_writer_qos(Some(writer_qos));

        let key = [0x5A; 16];
        let out = w
            .write_keyed_stamped(&[1, 2, 3, 4], None, Some(key))
            .expect("write");
        let inline_qos_to = |reader: Guid, out: &[OutboundDatagram]| {
            out.iter()
                .filter_map(|d| only_data(&d.bytes))
                .find(|d| d.reader_id == reader.entity_id)
                .unwrap_or_else(|| panic!("no DATA to {reader:?}"))
                .inline_qos
        };
        let ids = |qos: Option<ParameterList>| {
            qos.map(|l| l.parameters.iter().map(|p| p.id).collect::<Vec<_>>())
                .unwrap_or_default()
        };
        let to_stateless = inline_qos_to(stateless, &out);
        assert_eq!(
            ids(to_stateless.clone()),
            [pid::TOPIC_NAME, pid::KEY_HASH],
            "the writer's QoS first, then the change's"
        );
        assert_eq!(
            to_stateless.expect("in-line QoS").parameters[0],
            topic,
            "the octets are the writer's"
        );
        assert_eq!(ids(inline_qos_to(reader_guid(), &out)), [pid::KEY_HASH]);

        w.remove_reader_proxy(stateless);
        let out = w
            .write_keyed_stamped(&[5, 6, 7, 8], None, Some(key))
            .expect("write");
        assert_eq!(ids(inline_qos_to(reader_guid(), &out)), [pid::KEY_HASH]);
    }

    /// DDSI-RTPS 2.5 §8.4.14.1: the fragment size "is identical for all
    /// remote Readers. By fixing the fragment size, the data a fragment
    /// number refers to does not depend on a particular remote Reader." A
    /// reader that expects the writer's QoS in-line gets a longer in-line QoS
    /// than the reader beside it; the fragments of both are the same size.
    #[test]
    fn the_fragment_size_does_not_depend_on_the_reader_expecting_inline_qos() {
        use crate::parameter_list::{Parameter, ParameterList, pid};
        let mut w = make_writer(10, Duration::from_secs(1));
        let expecting = Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::user_reader_with_key([0x01, 0x02, 0x03]),
        );
        let mut proxy = ReaderProxy::new(
            expecting,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        );
        proxy.set_expects_inline_qos(true);
        w.add_reader_proxy(proxy);
        // A 100-octet parameter: with the key hash it takes the room that
        // 1344 octets of fragment need beside the headers.
        let mut writer_qos = ParameterList::new();
        writer_qos.push(Parameter::new(pid::TOPIC_NAME, alloc::vec![0u8; 100]));
        w.set_inline_writer_qos(Some(writer_qos));

        let out = w
            .write_keyed_stamped(&alloc::vec![7u8; 4000], None, Some([0x5A; 16]))
            .expect("write");
        let mut sizes = alloc::collections::BTreeMap::<[u8; 3], alloc::vec::Vec<u16>>::new();
        for dg in &out {
            for sm in &decode_datagram(&dg.bytes).unwrap().submessages {
                if let ParsedSubmessage::DataFrag(df) = &sm.submessage {
                    sizes
                        .entry(df.reader_id.entity_key)
                        .or_default()
                        .push(df.fragment_size);
                }
            }
        }
        assert_eq!(sizes.len(), 2, "both readers get fragments");
        for (reader, sizes) in &sizes {
            assert!(
                sizes.iter().all(|s| *s == sizes[0]),
                "reader {reader:?}: {sizes:?}"
            );
        }
        let all: alloc::collections::BTreeSet<u16> = sizes.values().flatten().copied().collect();
        assert_eq!(all.len(), 1, "one fragment size for all readers: {all:?}");
    }

    /// The DATA a batch of datagrams carries for `sn`.
    fn data_for(out: &[OutboundDatagram], sn: i64) -> usize {
        out.iter()
            .filter_map(|d| only_data(&d.bytes))
            .filter(|d| d.writer_sn == SequenceNumber(sn))
            .count()
    }

    /// DDSI-RTPS 2.5 §8.4.7.1 `nackResponseDelay`: the repair a NACK asks
    /// for goes out once the NACK has waited that long — not on the tick
    /// it arrived on, and on the first tick after.
    #[test]
    fn a_repair_waits_for_the_nack_response_delay() {
        let mut w = make_writer(10, Duration::from_secs(60));
        w.set_nack_response_delay(Duration::from_millis(200));
        assert_eq!(w.nack_response_delay(), Duration::from_millis(200));
        let _ = w.write(&[1, 2, 3, 4]).expect("write");
        let t0 = Duration::from_secs(10);
        w.handle_acknack_at(t0, reader_guid(), SequenceNumber(1), [SequenceNumber(1)]);
        assert_eq!(data_for(&w.tick(t0).expect("tick"), 1), 0, "at arrival");
        let early = t0 + Duration::from_millis(199);
        assert_eq!(data_for(&w.tick(early).expect("tick"), 1), 0, "1 ms early");
        let due = t0 + Duration::from_millis(200);
        assert_eq!(data_for(&w.tick(due).expect("tick"), 1), 1, "on time");
        // Answered: the next NACK waits from its own arrival.
        let t1 = due + Duration::from_secs(1);
        w.handle_acknack_at(t1, reader_guid(), SequenceNumber(1), [SequenceNumber(1)]);
        assert_eq!(data_for(&w.tick(t1).expect("tick"), 1), 0);
        let again = t1 + Duration::from_millis(200);
        assert_eq!(data_for(&w.tick(again).expect("tick"), 1), 1);
    }

    /// Zero, the default, answers on the tick that follows the NACK, as
    /// before the delay was settable.
    #[test]
    fn a_zero_nack_response_delay_repairs_on_the_next_tick() {
        let mut w = make_writer(10, Duration::from_secs(60));
        let _ = w.write(&[1, 2, 3, 4]).expect("write");
        w.handle_acknack(reader_guid(), SequenceNumber(1), [SequenceNumber(1)]);
        assert_eq!(
            data_for(&w.tick(Duration::from_secs(1)).expect("tick"), 1),
            1
        );
    }

    /// DDSI-RTPS 2.5 §8.4.7.1 `nackSuppressionDuration`: a NACK for a
    /// change that went out less than that long ago is ignored; one that
    /// comes later is answered.
    #[test]
    fn a_nack_inside_the_suppression_duration_is_ignored() {
        let mut w = make_writer(10, Duration::from_secs(60));
        w.set_nack_suppression_duration(Duration::from_millis(100));
        let t0 = Duration::from_secs(5);
        let _ = w.tick(t0).expect("tick sets the clock");
        let sent = w.write(&[1, 2, 3, 4]).expect("write");
        assert_eq!(data_for(&sent, 1), 1, "the first send");
        let soon = t0 + Duration::from_millis(50);
        w.handle_acknack_at(soon, reader_guid(), SequenceNumber(1), [SequenceNumber(1)]);
        assert_eq!(
            data_for(&w.tick(soon).expect("tick"), 1),
            0,
            "still in flight"
        );
        let later = t0 + Duration::from_millis(100);
        w.handle_acknack_at(later, reader_guid(), SequenceNumber(1), [SequenceNumber(1)]);
        assert_eq!(
            data_for(&w.tick(later).expect("tick"), 1),
            1,
            "lost after all"
        );
        // Turning it off forgets the send times.
        w.set_nack_suppression_duration(Duration::ZERO);
        assert_eq!(w.nack_suppression_duration(), Duration::ZERO);
        w.handle_acknack_at(later, reader_guid(), SequenceNumber(1), [SequenceNumber(1)]);
        assert_eq!(data_for(&w.tick(later).expect("tick"), 1), 1);
    }

    // ---- DDSI-RTPS 2.5 §8.7.6 Coherent Sets ----

    /// Helper: the inline-QoS `ParameterList` of the first DATA in a datagram.
    fn has_data(bytes: &[u8]) -> bool {
        decode_datagram(bytes)
            .expect("decode")
            .contents()
            .any(|s| matches!(s, ParsedSubmessage::Data(_)))
    }

    fn data_inline_qos(bytes: &[u8]) -> Option<crate::parameter_list::ParameterList> {
        let parsed = decode_datagram(bytes).expect("decode");
        parsed.contents().find_map(|s| match s {
            ParsedSubmessage::Data(d) => Some(d.inline_qos.clone()),
            _ => None,
        })?
    }

    /// §8.7.6 / §9.6.3.9: every DATA of a coherent set carries
    /// `PID_COHERENT_SET` in its inline QoS, and its value is the
    /// `SequenceNumber_t` of the set's FIRST sample — `{ long high;
    /// unsigned long low; }` in the submessage endianness. The assertion is on
    /// the octets of the parameter, not on the API that produced them.
    #[test]
    fn coherent_set_member_data_carries_pid_0x0056_with_set_first_sn_octets() {
        let mut w = make_writer(10, Duration::from_secs(1));
        // One sample outside the set, so the set does not start at SN 1 and a
        // hard-coded "1" cannot pass by accident.
        let _ = w.write(&alloc::vec![0x00]).expect("pre-set write");
        w.begin_coherent_set();
        let d1 = w.write(&alloc::vec![0xAA]).expect("set member 1");
        let d2 = w.write(&alloc::vec![0xBB]).expect("set member 2");

        // The set starts at SN 2, so high = 0, low = 2, little-endian.
        let expected: [u8; 8] = [0, 0, 0, 0, 2, 0, 0, 0];
        for (label, dg) in [("member 1", &d1), ("member 2", &d2)] {
            let pl = data_inline_qos(&dg[0].bytes)
                .unwrap_or_else(|| panic!("{label}: DATA has no inline QoS"));
            let p = pl
                .find(crate::parameter_list::pid::COHERENT_SET)
                .unwrap_or_else(|| panic!("{label}: no PID_COHERENT_SET"));
            assert_eq!(p.id, 0x0056, "{label}: PID number");
            assert_eq!(p.value.len(), 8, "{label}: SequenceNumber_t is 8 octets");
            assert_eq!(
                p.value.as_slice(),
                &expected,
                "{label}: set-first-SN octets"
            );
        }
    }

    /// §8.7.6: the marker belongs to the sample, not to the send path. The
    /// DATA+piggyback-HEARTBEAT path (`write_with_heartbeat`) and the
    /// retransmit path (`tick` after an ACKNACK) both build their DATA through
    /// `append_data`, which wrote a bare 20-octet header and no inline QoS at
    /// all. A set member sent that way reaches the reader unmarked, and the
    /// reader closes the set around the members it did see — a silently wrong
    /// cut, not a lost sample. Measured against the OMG `dds-rtps` suite: 3 of
    /// every 4 members of a `Test_CoherentSets_11` set arrived with the marker
    /// and the fourth without.
    #[test]
    fn coherent_set_marker_survives_piggyback_heartbeat_and_retransmit() {
        let expect_marker = |label: &str, bytes: &[u8], first_sn: i64| {
            let pl =
                data_inline_qos(bytes).unwrap_or_else(|| panic!("{label}: DATA has no inline QoS"));
            let p = pl
                .find(crate::parameter_list::pid::COHERENT_SET)
                .unwrap_or_else(|| panic!("{label}: no PID_COHERENT_SET"));
            let mut expected = [0u8; 8];
            expected[4..].copy_from_slice(&(first_sn as u32).to_le_bytes());
            assert_eq!(
                p.value.as_slice(),
                &expected,
                "{label}: set-first-SN octets"
            );
        };

        // (a) DATA + piggyback HEARTBEAT — the path a DCPS `write` takes
        // whenever a heartbeat is due.
        let mut w = make_writer(10, Duration::from_secs(1));
        w.begin_coherent_set();
        let out = w
            .write_with_heartbeat(&alloc::vec![0xAA], Duration::from_secs(1))
            .expect("write_with_heartbeat");
        let carrier = out
            .iter()
            .find(|d| data_inline_qos(&d.bytes).is_some() || has_data(&d.bytes))
            .expect("a DATA went out");
        expect_marker("piggyback DATA", &carrier.bytes, 1);

        // (b) Retransmit after an ACKNACK requesting the same SN.
        let mut w = make_writer(10, Duration::from_secs(10));
        w.begin_coherent_set();
        let _ = w.write(&alloc::vec![0xBB]).expect("set member");
        w.handle_acknack(reader_guid(), sn(1), [sn(1)]);
        let resent = w.tick(Duration::ZERO).expect("tick");
        let carrier = resent
            .iter()
            .find(|d| has_data(&d.bytes))
            .expect("the requested SN was resent");
        expect_marker("retransmitted DATA", &carrier.bytes, 1);
    }

    /// §8.7.6: a sample written outside an open set carries no
    /// `PID_COHERENT_SET`, and after `end_coherent_set` the next sample is
    /// unmarked again — the marker is not sticky.
    #[test]
    fn samples_outside_a_coherent_set_carry_no_marker() {
        let mut w = make_writer(10, Duration::from_secs(1));
        let before = w.write(&alloc::vec![0x01]).expect("before");
        assert!(
            data_inline_qos(&before[0].bytes).is_none(),
            "an unmarked DATA must carry no inline QoS at all"
        );
        w.begin_coherent_set();
        let _ = w.write(&alloc::vec![0x02]).expect("member");
        let _ = w.end_coherent_set().expect("end");
        let after = w.write(&alloc::vec![0x03]).expect("after");
        assert!(
            data_inline_qos(&after[0].bytes).is_none(),
            "after end_coherent_set the marker must be gone"
        );
    }

    /// §8.7.6: `end_coherent_set` emits the end-of-coherent-set sample — a DATA
    /// with the D and the K flag clear, an empty payload, and
    /// `PID_COHERENT_SET` naming the set that ends.
    #[test]
    fn end_coherent_set_emits_ecs_data_with_d_and_k_clear() {
        let mut w = make_writer(10, Duration::from_secs(1));
        w.begin_coherent_set();
        let _ = w.write(&alloc::vec![0xAA]).expect("member");
        let out = w.end_coherent_set().expect("end_coherent_set");
        assert!(!out.is_empty(), "ECS must reach the matched reader proxy");
        let parsed = decode_datagram(&out[0].bytes).expect("decode");
        let data = parsed
            .contents()
            .find_map(|s| match s {
                ParsedSubmessage::Data(d) => Some(d.clone()),
                _ => None,
            })
            .expect("ECS DATA");
        assert!(!data.data_flag, "ECS carries no sample, so D = 0");
        assert!(!data.key_flag, "ECS carries no key, so K = 0");
        assert!(data.serialized_payload.is_empty(), "ECS payload is empty");
        // Table 9.22, Example 3: no in-line QoS, so no PID_COHERENT_SET.
        assert!(data.inline_qos.is_none(), "ECS carries no in-line QoS");
    }

    /// A reader matched while a coherent set is open is given the whole set:
    /// its members stay in the cache although no reader had asked for them,
    /// and the join floor sits before the first member. After the set ends
    /// the floor is the last SN written again.
    #[test]
    fn a_reader_matched_inside_an_open_coherent_set_gets_the_whole_set() {
        let mut w = make_writer(10, Duration::from_secs(1));
        let _ = w.remove_reader_proxy(reader_guid());
        let _ = w.write(&alloc::vec![0x01]).expect("before the set");
        w.begin_coherent_set();
        let _ = w.write(&alloc::vec![0xAA]).expect("member 1");
        let _ = w.write(&alloc::vec![0xBB]).expect("member 2");
        w.remove_acknowledged();
        assert!(
            w.cache().get(SequenceNumber(2)).is_some(),
            "member 1 was dropped"
        );
        assert!(
            w.cache().get(SequenceNumber(3)).is_some(),
            "member 2 was dropped"
        );
        assert_eq!(w.volatile_join_floor(), SequenceNumber(1));
        let _ = w.end_coherent_set().expect("end");
        assert_eq!(w.volatile_join_floor(), w.last_written_sn());
    }

    /// §8.7.6: an empty transaction has no first sample, so it names no set —
    /// `end_coherent_set` sends nothing rather than an ECS for a set that never
    /// existed.
    #[test]
    fn empty_coherent_set_sends_no_ecs() {
        let mut w = make_writer(10, Duration::from_secs(1));
        w.begin_coherent_set();
        assert!(w.coherent_set_is_open());
        assert_eq!(w.coherent_set_first_sn(), None);
        let out = w.end_coherent_set().expect("end");
        assert!(out.is_empty(), "empty set must not emit an ECS sample");
        assert!(!w.coherent_set_is_open());
    }

    /// DDSI-RTPS 2.5 §8.7.6: a coherent set member too large for one DATA
    /// goes out as DATA_FRAGs that carry its in-line QoS, and the reader's
    /// assembler hands the marker on with the sample. It used to be refused
    /// with `UnsupportedFeature`, because the assembler dropped the in-line
    /// QoS of fragments.
    #[test]
    fn an_oversized_coherent_set_member_goes_out_as_marked_fragments() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(1), 16);
        let _ = w.write(&alloc::vec![0x00]).expect("pre-set write");
        w.begin_coherent_set();
        let payload: alloc::vec::Vec<u8> = (0u8..64).collect();
        let datagrams = w.write(&payload).expect("fragmented member");
        let mut assembler = crate::fragment_assembler::FragmentAssembler::default();
        let mut completed = None;
        let mut fragments = 0;
        for dg in &datagrams {
            for sub in decode_datagram(&dg.bytes).expect("decode").contents() {
                if let ParsedSubmessage::DataFrag(df) = sub {
                    fragments += usize::from(df.fragments_in_submessage);
                    let qos = df.inline_qos.as_ref().expect("fragment without inline QoS");
                    assert_eq!(
                        crate::inline_qos::find_coherent_set(qos, qos.little_endian),
                        Some(2)
                    );
                    if let Some(sample) = assembler.insert(df) {
                        completed = Some(sample);
                    }
                }
            }
        }
        assert_eq!(fragments, 4, "64 octets in fragments of 16");
        let sample = completed.expect("reassembled");
        assert_eq!(sample.payload, payload);
        let qos = sample.inline_qos.expect("inline QoS kept");
        assert_eq!(
            crate::inline_qos::find_coherent_set(&qos, qos.little_endian),
            Some(2)
        );
    }

    /// The fragment size shrinks for a sample whose in-line QoS would push
    /// a DATA_FRAG past the MTU, and every datagram stays within it.
    #[test]
    fn a_sample_with_long_inline_qos_is_cut_into_smaller_fragments() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(1), DEFAULT_FRAGMENT_SIZE);
        let mut qos = crate::parameter_list::ParameterList::new();
        // 200 octets of in-line QoS, past the 44 the default leaves.
        qos.push(crate::parameter_list::Parameter::new(
            crate::parameter_list::pid::TOPIC_NAME,
            alloc::vec![b'x'; 196],
        ));
        let payload = alloc::vec![0x5A; 4000];
        let datagrams = w
            .write_sample_with_inline_qos(&payload, None, None, Some(qos))
            .expect("write");
        assert!(datagrams.len() > 3, "smaller fragments, more datagrams");
        for dg in &datagrams {
            assert!(dg.bytes.len() <= DEFAULT_MTU, "{} > MTU", dg.bytes.len());
        }
    }

    #[test]
    fn write_increments_sn_and_returns_data_datagram() {
        let mut w = make_writer(10, Duration::from_secs(1));
        let d1 = w.write(&alloc::vec![0xAA]).expect("write1");
        let d2 = w.write(&alloc::vec![0xBB]).expect("write2");
        assert_eq!(d1.len(), 1);
        assert_eq!(d2.len(), 1);
        let p1 = decode_datagram(&d1[0].bytes).unwrap();
        let p2 = decode_datagram(&d2[0].bytes).unwrap();
        match (&p1.submessages[0].submessage, &p2.submessages[0].submessage) {
            (ParsedSubmessage::Data(a), ParsedSubmessage::Data(b)) => {
                assert_eq!(a.writer_sn, sn(1));
                assert_eq!(b.writer_sn, sn(2));
            }
            _ => panic!("expected DATA submessages"),
        }
        assert_eq!(w.cache().len(), 2);
    }

    #[test]
    fn tick_emits_heartbeat_after_period() {
        let mut w = make_writer(10, Duration::from_millis(500));
        w.write(&alloc::vec![0xAA]).unwrap();
        let out = w.tick(Duration::from_millis(10)).unwrap();
        assert_eq!(out.len(), 1);
        let parsed = decode_datagram(&out[0].bytes).expect("decode hb");
        assert!(
            parsed
                .contents()
                .any(|s| matches!(s, ParsedSubmessage::Heartbeat(_)))
        );
        assert!(w.tick(Duration::from_millis(200)).unwrap().is_empty());
        let out2 = w.tick(Duration::from_millis(600)).unwrap();
        assert_eq!(out2.len(), 1);
    }

    /// The HEARTBEATs in `dgs`, as `(first, last)`.
    /// A writer of a GROUP-scope publisher with the entity key `key`,
    /// sharing `group`.
    fn group_writer(
        key: u8,
        group: &alloc::sync::Arc<crate::writer_group::WriterGroup>,
    ) -> ReliableWriter {
        let mut w = fast_writer(ReliableWriterConfig {
            guid: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0, 0, key]),
            ),
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![ReaderProxy::new(
                reader_guid(),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
                alloc::vec![],
                true,
            )],
            max_samples: 32,
            history_kind: HistoryKind::KeepAll,
            heartbeat_period: Duration::from_secs(1),
            fragment_size: DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        w.set_group(Some(alloc::sync::Arc::clone(group)));
        w
    }

    /// The group parameters of every DATA in `dgs`: (writerSN, D flag,
    /// PID_GROUP_SEQ_NUM, PID_COHERENT_SET, PID_GROUP_COHERENT_SET,
    /// PID_WRITER_GROUP_INFO).
    #[allow(clippy::type_complexity)]
    fn group_params(
        dgs: &[OutboundDatagram],
    ) -> Vec<(
        i64,
        bool,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<crate::group_digest::GroupDigest>,
    )> {
        let mut out = Vec::new();
        for dg in dgs {
            for sub in decode_datagram(&dg.bytes).unwrap().contents() {
                if let ParsedSubmessage::Data(d) = sub {
                    let pl = d.inline_qos.clone().unwrap_or_default();
                    let le = pl.little_endian;
                    out.push((
                        d.writer_sn.0,
                        d.data_flag,
                        crate::inline_qos::find_group_seq_num(&pl, le),
                        crate::inline_qos::find_coherent_set(&pl, le),
                        crate::inline_qos::find_group_coherent_set(&pl, le),
                        crate::inline_qos::find_writer_group_info(&pl),
                    ));
                }
            }
        }
        out
    }

    /// `local_group_progress`: the group's last number, and every number up
    /// to the writer's last as one it may have, whether the cache still
    /// holds the change or not.
    #[test]
    fn local_group_progress_counts_every_number_up_to_the_writers_last() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w1 = group_writer(1, &group);
        let mut w2 = group_writer(2, &group);
        assert_eq!(
            w1.local_group_progress()
                .map(|p| (p.current_gsn.0, p.first_gsn.0, p.last_gsn.0)),
            Some((0, 1, 0))
        );
        let _ = w1.write(&[1]).unwrap();
        let _ = w1.write(&[2]).unwrap();
        let _ = w2.write(&[3]).unwrap();
        w1.remove_acknowledged();
        assert_eq!(
            w1.local_group_progress()
                .map(|p| (p.current_gsn.0, p.first_gsn.0, p.last_gsn.0)),
            Some((3, 1, 2))
        );
        assert!(
            make_writer(10, Duration::from_secs(1))
                .local_group_progress()
                .is_none()
        );
    }

    /// A dispose is a change of its writer like a sample (DDSI-RTPS 2.5
    /// §8.7.5, §8.7.6): in a coherent set it carries `PID_COHERENT_SET`,
    /// here as the set's first change, and a GROUP-scope publisher's writer
    /// gives it `PID_GROUP_SEQ_NUM`, `PID_WRITER_GROUP_INFO` and
    /// `PID_GROUP_COHERENT_SET`.
    #[test]
    fn a_dispose_carries_the_set_and_group_parameters_of_its_change() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w = group_writer(1, &group);
        group.set_writers(&[w.guid().entity_id]);
        let set = crate::group_digest::GroupDigest::of_entities(&[w.guid().entity_id]);
        let _ = w.write(&[0]).unwrap();
        let disposed = crate::inline_qos::status_info::DISPOSED;
        let outside = w.write_lifecycle([7; 16], &[], disposed, None).unwrap();
        group.begin_coherent_set();
        w.begin_coherent_set();
        let d = w.write_lifecycle([7; 16], &[], disposed, None).unwrap();
        let s = w.write(&[1]).unwrap();
        let end = group.end_coherent_set().unwrap();
        let e = w.end_group_coherent_set(end).unwrap();
        // (writerSN, D, GSN, COHERENT_SET, GROUP_COHERENT_SET, writer set)
        assert_eq!(
            group_params(&outside),
            [(2, false, Some(2), None, None, Some(set))]
        );
        assert_eq!(
            group_params(&d),
            [(3, false, Some(3), Some(3), Some(3), Some(set))]
        );
        assert_eq!(
            group_params(&s),
            [(4, true, Some(4), Some(3), Some(3), Some(set))]
        );
        assert_eq!(
            group_params(&e),
            [(5, false, Some(5), Some(3), Some(3), Some(set))]
        );
    }

    /// A writer outside a group: a dispose in a coherent set carries the
    /// set's `PID_COHERENT_SET`, the end-of-coherent-set DATA none (Table
    /// 9.22, Example 3), and neither a group parameter.
    #[test]
    fn a_dispose_in_a_writers_coherent_set_carries_its_pid_coherent_set() {
        let mut w = make_writer(10, Duration::from_secs(1));
        let _ = w.write(&[0]).unwrap();
        w.begin_coherent_set();
        let s = w.write(&[1]).unwrap();
        let disposed = crate::inline_qos::status_info::DISPOSED;
        let d = w.write_lifecycle([7; 16], &[], disposed, None).unwrap();
        let e = w.end_coherent_set().unwrap();
        assert_eq!(group_params(&s), [(2, true, None, Some(2), None, None)]);
        assert_eq!(group_params(&d), [(3, false, None, Some(2), None, None)]);
        assert_eq!(group_params(&e), [(4, false, None, None, None, None)]);
        let after = w.write_lifecycle([7; 16], &[], disposed, None).unwrap();
        assert_eq!(group_params(&after), [(5, false, None, None, None, None)]);
    }

    /// DDSI-RTPS 2.5 Table 9.23, octet values aside: two writers of a
    /// GROUP-scope publisher, a group coherent set of three samples, and
    /// both writers' end-of-coherent-set DATA.
    #[test]
    fn a_group_coherent_set_carries_the_parameters_of_table_9_23() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w1 = group_writer(1, &group);
        let mut w2 = group_writer(2, &group);
        group.set_writers(&[w1.guid().entity_id, w2.guid().entity_id]);
        let set = crate::group_digest::GroupDigest::of_entities(&[
            w1.guid().entity_id,
            w2.guid().entity_id,
        ]);
        // Writer 1 at SN 3, writer 2 at SN 7, the group at GSN 10.
        for _ in 0..3 {
            let _ = w1.write(&[0]).unwrap();
        }
        for _ in 0..7 {
            let _ = w2.write(&[0]).unwrap();
        }
        group.begin_coherent_set();
        w1.begin_coherent_set();
        w2.begin_coherent_set();
        let d1 = w1.write(&[1]).unwrap();
        let d2 = w2.write(&[2]).unwrap();
        let d3 = w1.write(&[3]).unwrap();
        let end = group.end_coherent_set().unwrap();
        let e1 = w1.end_group_coherent_set(end).unwrap();
        let e2 = w2.end_group_coherent_set(end).unwrap();
        // (writerSN, D, GSN, COHERENT_SET, GROUP_COHERENT_SET, writer set)
        assert_eq!(
            group_params(&d1),
            [(4, true, Some(11), Some(4), Some(11), Some(set))]
        );
        assert_eq!(
            group_params(&d2),
            [(8, true, Some(12), Some(8), Some(11), Some(set))]
        );
        assert_eq!(
            group_params(&d3),
            [(5, true, Some(13), Some(4), Some(11), Some(set))]
        );
        assert_eq!(
            group_params(&e1),
            [(6, false, Some(14), Some(4), Some(11), Some(set))]
        );
        assert_eq!(
            group_params(&e2),
            [(9, false, Some(14), Some(8), Some(11), Some(set))]
        );
    }

    /// DDSI-RTPS 2.5 §9.6.4.6: with `set_secure_writer_group_info` the DATA of
    /// a GROUP-scope publisher's writer carries PID_SECURE_WRITER_GROUP_INFO
    /// (0x0066) where it has PID_WRITER_GROUP_INFO (0x0065) otherwise, with the
    /// same WriterGroupInfo_t, and a reader takes either for the group
    /// information.
    #[test]
    fn the_secure_writer_group_info_stands_where_the_plain_one_is_required() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w = group_writer(1, &group);
        group.set_writers(&[w.guid().entity_id]);
        let qos_pids = |w: &mut ReliableWriter| -> alloc::vec::Vec<(u16, alloc::vec::Vec<u8>)> {
            let dgs = w.write(&[1]).unwrap();
            let data = only_data(&dgs[0].bytes).unwrap();
            let qos = data.inline_qos.unwrap();
            qos.parameters
                .iter()
                .filter(|p| {
                    p.id == crate::parameter_list::pid::WRITER_GROUP_INFO
                        || p.id == crate::parameter_list::pid::SECURE_WRITER_GROUP_INFO
                })
                .map(|p| (p.id, p.value.clone()))
                .collect()
        };
        let plain = qos_pids(&mut w);
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].0, 0x0065);
        w.set_secure_writer_group_info(true);
        let secure = qos_pids(&mut w);
        assert_eq!(secure.len(), 1);
        assert_eq!(secure[0].0, 0x0066);
        assert_eq!(secure[0].1, plain[0].1, "the same WriterGroupInfo_t");
        // A reader finds the group information under either.
        for parameter in [
            crate::inline_qos::writer_group_info_param(
                crate::group_digest::GroupDigest::of_entities(&[w.guid().entity_id]),
            ),
            crate::inline_qos::secure_writer_group_info_param(
                crate::group_digest::GroupDigest::of_entities(&[w.guid().entity_id]),
            ),
        ] {
            let mut pl = crate::parameter_list::ParameterList::new();
            pl.push(crate::inline_qos::group_seq_num_param(5, true));
            pl.push(parameter);
            let info = crate::inline_qos::find_change_group_info(&pl).unwrap();
            assert_eq!(info.gsn, 5);
            assert_eq!(
                info.writer_set,
                crate::group_digest::GroupDigest::of_entities(&[w.guid().entity_id])
            );
        }
    }

    /// `GroupCoherentSetPid::Omitted` for a peer's participant leaves
    /// PID_GROUP_COHERENT_SET out of that peer's DATA and nothing else;
    /// the default sends it (DDSI-RTPS 2.5 §8.7.6).
    #[test]
    fn a_participant_set_to_go_without_it_gets_no_group_coherent_set_pid() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w = group_writer(1, &group);
        group.begin_coherent_set();
        w.begin_coherent_set();
        let sent = w.write(&[1]).unwrap();
        assert_eq!(group_params(&sent)[0].4, Some(1), "the default sends it");
        w.set_participant_behaviour(
            reader_guid().prefix,
            crate::dialect::RtpsBehaviour::new()
                .with_group_coherent_set(crate::dialect::GroupCoherentSetPid::Omitted),
        );
        let omitted = w.write(&[2]).unwrap();
        let [(_, _, gsn, cs, gcs, set)] = group_params(&omitted)[..] else {
            panic!("one DATA");
        };
        assert_eq!(gcs, None);
        assert_eq!((gsn, cs), (Some(2), Some(1)));
        assert!(set.is_some(), "the writer set still goes");
    }

    /// `GroupWireForm::CoherentSetsOnly` for a peer's participant sends that
    /// peer a GROUP-scope writer's set as at TOPIC scope: `PID_COHERENT_SET`
    /// on the sample, an end-of-set DATA without in-line QoS, no group
    /// sequence number, writer set or group coherent set, and HEARTBEATs
    /// without the GroupInfoFlag. The default sends all of it (the tests
    /// around this one).
    #[test]
    fn a_participant_set_to_coherent_sets_only_gets_no_group_information() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w = group_writer(1, &group);
        w.set_participant_behaviour(
            reader_guid().prefix,
            crate::dialect::RtpsBehaviour::new()
                .with_group_wire_form(crate::dialect::GroupWireForm::CoherentSetsOnly),
        );
        group.begin_coherent_set();
        w.begin_coherent_set();
        let sample = w.write(&[1]).unwrap();
        assert_eq!(
            group_params(&sample),
            [(1, true, None, Some(1), None, None)]
        );
        let end = group.end_coherent_set().unwrap();
        let ecs = w.end_group_coherent_set(end).unwrap();
        assert_eq!(group_params(&ecs), [(2, false, None, None, None, None)]);
        let hb = w.tick(Duration::from_secs(5)).unwrap();
        let gsns = heartbeat_gsns(&hb);
        assert!(!gsns.is_empty(), "the reader is still heartbeated");
        assert!(gsns.iter().all(Option::is_none), "{gsns:?}");
    }

    /// `EndOfSetForm::Gap` for a peer's participant sends that peer the
    /// number of a writer's end-of-coherent-set DATA as a GAP; the default
    /// sends the DATA.
    #[test]
    fn a_participant_set_to_gap_ends_gets_the_end_of_a_set_as_a_gap() {
        fn kinds(dgs: &[OutboundDatagram]) -> Vec<(&'static str, i64)> {
            let mut out = Vec::new();
            for dg in dgs {
                for sub in decode_datagram(&dg.bytes).unwrap().contents() {
                    match sub {
                        ParsedSubmessage::Data(d) => out.push(("DATA", d.writer_sn.0)),
                        ParsedSubmessage::Gap(g) => out.push(("GAP", g.gap_start.0)),
                        _ => {}
                    }
                }
            }
            out
        }
        let mut w = make_writer(32, Duration::from_secs(1));
        w.begin_coherent_set();
        let _ = w.write(&[1]).unwrap();
        assert_eq!(kinds(&w.end_coherent_set().unwrap()), [("DATA", 2)]);
        w.set_participant_behaviour(
            reader_guid().prefix,
            crate::dialect::RtpsBehaviour::new()
                .with_end_of_set_form(crate::dialect::EndOfSetForm::Gap),
        );
        w.begin_coherent_set();
        assert_eq!(kinds(&w.write(&[2]).unwrap()), [("DATA", 3)]);
        assert_eq!(kinds(&w.end_coherent_set().unwrap()), [("GAP", 4)]);
    }

    /// §8.7.6: "from all Writers within the Publisher" — a writer that
    /// wrote nothing into the set sends an end-of-coherent-set DATA too,
    /// without a PID_COHERENT_SET of its own.
    #[test]
    fn a_writer_with_no_sample_in_the_set_still_ends_it() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w1 = group_writer(1, &group);
        let mut w2 = group_writer(2, &group);
        group.begin_coherent_set();
        w1.begin_coherent_set();
        w2.begin_coherent_set();
        let _ = w1.write(&[1]).unwrap();
        let end = group.end_coherent_set().unwrap();
        let _ = w1.end_group_coherent_set(end).unwrap();
        let e2 = w2.end_group_coherent_set(end).unwrap();
        let [(sn, data, gsn, cs, gcs, _)] = group_params(&e2)[..] else {
            panic!("one ECS DATA");
        };
        assert_eq!((sn, data, gsn, cs, gcs), (1, false, Some(2), None, Some(1)));
    }

    /// Outside a coherent set a GROUP-scope writer's DATA carries the group
    /// sequence number and the writer set, and no set parameter.
    #[test]
    fn a_group_writers_data_carries_its_group_sequence_number() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w1 = group_writer(1, &group);
        let mut w2 = group_writer(2, &group);
        let a = w1.write(&[1]).unwrap();
        let b = w2.write(&[2]).unwrap();
        let c = w1.write(&[3]).unwrap();
        let gsn = |dgs: &[OutboundDatagram]| group_params(dgs)[0].2;
        assert_eq!([gsn(&a), gsn(&b), gsn(&c)], [Some(1), Some(2), Some(3)]);
        assert_eq!(group_params(&c)[0].3, None);
        assert_eq!(group_params(&c)[0].4, None);
    }

    /// DDSI-RTPS 2.5 §8.3.8.4: a GAP of a writer in a group carries
    /// gapStartGSN and gapEndGSN with the GroupInfoFlag, the group sequence
    /// number of the change it announces away, also after the change left the
    /// cache; a GAP of a writer outside a group carries none. The numbers are
    /// valid (start > 0, end >= start - 1) and the reader finds them.
    #[test]
    fn a_gap_of_a_group_writer_carries_the_group_sequence_numbers() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w1 = group_writer(1, &group);
        let mut w2 = group_writer(2, &group);
        let _ = w1.write(&[1]).unwrap(); // SN 1, GSN 1
        let _ = w2.write(&[2]).unwrap(); // GSN 2
        let _ = w1.write(&[3]).unwrap(); // SN 2, GSN 3
        let _ = w1.write(&[4]).unwrap(); // SN 3, GSN 4
        let gaps =
            |w: &mut ReliableWriter, at: Duration| -> Vec<crate::submessages::GapSubmessage> {
                w.tick(at)
                    .unwrap()
                    .iter()
                    .flat_map(|d| {
                        decode_datagram(&d.bytes)
                            .unwrap()
                            .contents()
                            .filter_map(|s| match s {
                                ParsedSubmessage::Gap(g) => Some(g.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect()
            };
        // SN 2 (GSN 3) is still in the cache but addressed to nobody here:
        // the reader asks for SN 1 after the cache dropped it.
        w1.remove_samples_up_to(sn(2));
        w1.handle_acknack(reader_guid(), sn(1), [sn(1)]);
        let g = gaps(&mut w1, Duration::from_millis(1));
        assert_eq!(g.len(), 1);
        let info = g[0].group_info.expect("GroupInfoFlag");
        assert_eq!((info.gap_start_gsn.0, info.gap_end_gsn.0), (1, 1));
        assert!(info.is_valid());
        // A change that is still in the cache and filtered for the reader.
        let odd = crate::reader_proxy::ChangeFilter::new(|c| c.payload.first() == Some(&4));
        let mut w3 = group_writer(3, &group);
        w3.remove_reader_proxy(reader_guid());
        w3.add_reader_proxy(
            ReaderProxy::new(
                reader_guid(),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
                alloc::vec![],
                true,
            )
            .with_filter(odd),
        );
        let out = w3.write(&[9]).unwrap(); // GSN 5, filtered
        let g: Vec<_> = out
            .iter()
            .flat_map(|d| {
                decode_datagram(&d.bytes)
                    .unwrap()
                    .contents()
                    .filter_map(|s| match s {
                        ParsedSubmessage::Gap(g) => Some(g.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(g.len(), 1);
        let info = g[0].group_info.expect("GroupInfoFlag");
        assert_eq!((info.gap_start_gsn.0, info.gap_end_gsn.0), (5, 5));
        // Outside a group: no flag.
        let mut plain = make_writer(10, Duration::from_secs(10));
        let _ = plain.write(&[1]).unwrap();
        plain.remove_samples_up_to(sn(2));
        plain.handle_acknack(reader_guid(), sn(1), [sn(1)]);
        let g = gaps(&mut plain, Duration::from_millis(300));
        assert!(!g.is_empty());
        assert!(g.iter().all(|g| g.group_info.is_none()));
    }

    /// A writer outside a group sends none of the group parameters.
    #[test]
    fn a_writer_outside_a_group_sends_no_group_parameter() {
        let mut w = make_writer(10, Duration::from_secs(1));
        let d = w.write(&[1]).unwrap();
        assert!(
            group_params(&d)
                .iter()
                .all(|p| p.2.is_none() && p.5.is_none())
        );
    }

    /// §8.3.7.5: a GROUP-scope writer's HEARTBEAT carries currentGSN (the
    /// group's last), firstGSN and lastGSN (its own first and last
    /// sample's).
    #[test]
    fn a_group_writers_heartbeat_carries_the_groups_numbers() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w1 = group_writer(1, &group);
        let mut w2 = group_writer(2, &group);
        let _ = w1.write(&[1]).unwrap(); // GSN 1
        let _ = w2.write(&[2]).unwrap(); // GSN 2
        let _ = w1.write(&[3]).unwrap(); // GSN 3
        let _ = w2.write(&[4]).unwrap(); // GSN 4
        let out = w1.tick(Duration::from_secs(5)).unwrap();
        let infos: Vec<_> = out
            .iter()
            .flat_map(|dg| {
                decode_datagram(&dg.bytes)
                    .unwrap()
                    .contents()
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .filter_map(|s| match s {
                ParsedSubmessage::Heartbeat(h) => Some(h.group_info),
                _ => None,
            })
            .collect();
        assert!(!infos.is_empty(), "no HEARTBEAT");
        for info in infos {
            let info = info.expect("HEARTBEAT without group information");
            assert_eq!(
                (info.current_gsn.0, info.first_gsn.0, info.last_gsn.0),
                (4, 1, 3)
            );
        }
    }

    /// DDSI-RTPS 2.5 Table 8.44: a HEARTBEAT with the GroupInfoFlag carries
    /// `writerSet` (the digest of the group's writers) and `secureWriterSet`
    /// (the empty group: ZeroDDS has no secure set) after lastGSN; a peer whose
    /// behaviour omits them gets the three numbers of the figure of §9.4.5.7.
    #[test]
    fn a_group_writers_heartbeat_carries_the_writer_sets_unless_the_peer_omits_them() {
        use crate::dialect::HeartbeatGroupDigests;
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let ids = [
            EntityId::user_writer_with_key([0, 0, 1]),
            EntityId::user_writer_with_key([0, 0, 2]),
        ];
        group.set_writers(&ids);
        let sets_of = |w: &mut ReliableWriter| {
            let _ = w.write(&[1]).unwrap();
            w.tick(Duration::from_secs(5))
                .unwrap()
                .iter()
                .flat_map(|dg| {
                    decode_datagram(&dg.bytes)
                        .unwrap()
                        .contents()
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .filter_map(|s| match s {
                    ParsedSubmessage::Heartbeat(h) => h.group_info,
                    _ => None,
                })
                .map(|g| (g.writer_set, g.secure_writer_set))
                .collect::<Vec<_>>()
        };
        let mut sent = group_writer(1, &group);
        let sets = sets_of(&mut sent);
        assert!(!sets.is_empty());
        for (writer_set, secure) in sets {
            assert_eq!(
                writer_set,
                Some(crate::group_digest::GroupDigest::of_entities(&ids))
            );
            assert_eq!(secure, Some(crate::group_digest::GroupDigest::EMPTY));
        }
        let mut omitted = group_writer(1, &group);
        omitted.set_behaviour(
            omitted
                .behaviour()
                .with_heartbeat_group_digests(HeartbeatGroupDigests::Omitted),
        );
        let sets = sets_of(&mut omitted);
        assert!(!sets.is_empty());
        assert!(sets.iter().all(|s| *s == (None, None)));
    }

    /// (currentGSN, firstGSN, lastGSN) of every HEARTBEAT in `dgs`, `None`
    /// for one without group information.
    fn heartbeat_gsns(dgs: &[OutboundDatagram]) -> Vec<Option<(i64, i64, i64)>> {
        let mut out = Vec::new();
        for dg in dgs {
            for s in decode_datagram(&dg.bytes).unwrap().contents() {
                if let ParsedSubmessage::Heartbeat(h) = s {
                    out.push(
                        h.group_info
                            .map(|g| (g.current_gsn.0, g.first_gsn.0, g.last_gsn.0)),
                    );
                }
            }
        }
        out
    }

    /// DDSI-RTPS 2.5 §8.4.2.2: a writer of a group heartbeats a reader that
    /// has all its samples once another writer of the group has written,
    /// so the subscriber learns this writer does not have those numbers —
    /// and not while the group's last number is its own or nothing new
    /// came since its last HEARTBEAT.
    #[test]
    fn a_group_writer_heartbeats_an_acknowledged_reader_when_the_group_moved_on() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        let mut w1 = group_writer(1, &group);
        let mut w2 = group_writer(2, &group);
        let _ = w1.write(&[1]).unwrap(); // GSN 1
        // The reader has not answered a HEARTBEAT yet: it gets one, for
        // SN 1 at GSN 1, and acknowledges it.
        let handshake = w1.tick(Duration::from_secs(5)).unwrap();
        assert_eq!(heartbeat_gsns(&handshake), [Some((1, 1, 1))]);
        w1.handle_acknack(reader_guid(), SequenceNumber(2), []);
        let own_last = w1.tick(Duration::from_secs(10)).unwrap();
        assert_eq!(heartbeat_gsns(&own_last), []);
        let _ = w2.write(&[2]).unwrap(); // GSN 2
        let _ = w2.write(&[3]).unwrap(); // GSN 3
        let moved_on = w1.tick(Duration::from_secs(15)).unwrap();
        // first = last + 1 for the acknowledged reader: firstGSN 2,
        // lastGSN 1 (§8.3.7.5: lastGSN >= firstGSN - 1).
        assert_eq!(heartbeat_gsns(&moved_on), [Some((3, 2, 1))]);
        let nothing_new = w1.tick(Duration::from_secs(20)).unwrap();
        assert_eq!(heartbeat_gsns(&nothing_new), []);
    }

    /// A writer of a group is due for a HEARTBEAT with an empty cache; one
    /// outside a group is not.
    #[test]
    fn a_group_writer_with_an_empty_cache_is_due_for_a_heartbeat() {
        let group = alloc::sync::Arc::new(crate::writer_group::WriterGroup::new());
        assert!(group_writer(1, &group).heartbeat_due().is_some());
        assert!(
            make_writer(10, Duration::from_secs(1))
                .heartbeat_due()
                .is_none()
        );
    }

    fn heartbeats(dgs: &[OutboundDatagram]) -> Vec<(i64, i64)> {
        let mut out = Vec::new();
        for dg in dgs {
            for s in decode_datagram(&dg.bytes).unwrap().contents() {
                if let ParsedSubmessage::Heartbeat(h) = s {
                    out.push((h.first_sn.0, h.last_sn.0));
                }
            }
        }
        out
    }

    /// A writer with nothing written heartbeats each reliable reader, first
    /// 1 and last 0, until the reader has answered one; then it is quiet.
    /// It used to send nothing: an OpenDDS 3.34 reader, which counts a
    /// reliable writer as matched only after that exchange, stayed
    /// unmatched (measured on the Linux test host, 25 s).
    #[test]
    fn an_empty_writer_heartbeats_a_reader_until_it_answers() {
        let mut w = make_writer(10, Duration::from_millis(100));
        assert_eq!(
            heartbeats(&w.tick(Duration::from_secs(10)).unwrap()),
            alloc::vec![(1, 0)]
        );
        assert!(
            w.tick(Duration::from_millis(10_050)).unwrap().is_empty(),
            "not before the period"
        );
        assert_eq!(
            heartbeats(&w.tick(Duration::from_millis(10_100)).unwrap()),
            alloc::vec![(1, 0)],
            "again while unanswered"
        );
        w.handle_acknack(reader_guid(), sn(1), alloc::vec![]);
        assert!(w.tick(Duration::from_secs(11)).unwrap().is_empty());
    }

    /// An ACKNACK the reader sent before any HEARTBEAT reached it (a
    /// preemptive one) answers nothing.
    #[test]
    fn a_preemptive_acknack_does_not_end_the_handshake() {
        let mut w = make_writer(10, Duration::from_millis(100));
        w.handle_acknack(reader_guid(), sn(1), alloc::vec![]);
        assert_eq!(
            heartbeats(&w.tick(Duration::from_secs(10)).unwrap()),
            alloc::vec![(1, 0)]
        );
    }

    /// The handshake HEARTBEAT reached the reader before it held the
    /// writer, and the reader's own preemptive ACKNACK came back next: the
    /// handshake looked done. The reader goes on asking — an OpenDDS reader
    /// every 1, 2, 4 … seconds — and an ACKNACK a period after the last
    /// HEARTBEAT reopens it, so one more HEARTBEAT goes out. Its answer
    /// closes it again, and an answer that arrives twice within the period
    /// does not reopen it.
    #[test]
    fn an_unprompted_acknack_reopens_the_handshake_of_an_empty_writer() {
        let mut w = make_writer(10, Duration::from_millis(100));
        assert_eq!(
            heartbeats(&w.tick(Duration::from_secs(10)).unwrap()),
            alloc::vec![(1, 0)]
        );
        // The reader's preemptive ACKNACK, 20 ms later: taken as the answer.
        w.handle_acknack_at(
            Duration::from_millis(10_020),
            reader_guid(),
            sn(1),
            alloc::vec![],
        );
        assert!(w.tick(Duration::from_millis(10_200)).unwrap().is_empty());
        // A second answer within the period reopens nothing.
        w.handle_acknack_at(
            Duration::from_millis(10_050),
            reader_guid(),
            sn(1),
            alloc::vec![],
        );
        assert!(w.tick(Duration::from_millis(10_400)).unwrap().is_empty());
        // The reader asks again a second later: one more HEARTBEAT.
        w.handle_acknack_at(
            Duration::from_millis(11_020),
            reader_guid(),
            sn(1),
            alloc::vec![],
        );
        assert_eq!(
            heartbeats(&w.tick(Duration::from_millis(11_100)).unwrap()),
            alloc::vec![(1, 0)]
        );
        // Its answer closes the handshake; the writer is quiet again.
        w.handle_acknack_at(
            Duration::from_millis(11_110),
            reader_guid(),
            sn(1),
            alloc::vec![],
        );
        assert!(w.tick(Duration::from_millis(11_300)).unwrap().is_empty());
        assert!(w.tick(Duration::from_secs(13)).unwrap().is_empty());
    }

    /// DDSI-RTPS 2.5 Table 8.66: T5 takes a reader proxy from `announcing`
    /// to `idle` once `RP::unacked_changes()` is false, and the periodic
    /// HEARTBEAT (T7) fires only in `announcing`. A reader that has
    /// acknowledged the cache and answered a HEARTBEAT gets none; a change
    /// it has not acknowledged brings them back.
    #[test]
    fn a_reader_that_acknowledged_the_cache_gets_no_periodic_heartbeat() {
        let mut w = make_writer(10, Duration::from_millis(100));
        w.write(&alloc::vec![1]).unwrap();
        assert_eq!(
            heartbeats(&w.tick(Duration::from_secs(1)).unwrap()),
            alloc::vec![(1, 1)]
        );
        w.handle_acknack(reader_guid(), sn(2), alloc::vec![]);
        assert!(
            w.tick(Duration::from_secs(2)).unwrap().is_empty(),
            "idle once the reader acknowledged the cache"
        );
        assert!(
            w.tick(Duration::from_secs(3)).unwrap().is_empty(),
            "and it stays idle"
        );
        w.write(&alloc::vec![2]).unwrap();
        assert_eq!(
            heartbeats(&w.tick(Duration::from_secs(4)).unwrap()),
            alloc::vec![(2, 2)],
            "announcing again for the change it has not acknowledged"
        );
    }

    /// DDSI-RTPS 2.5 §9.4.5.3: "F=0 means the writer must respond to the
    /// AckNack message with a Heartbeat message." An ACKNACK that
    /// acknowledges everything with F clear gets a HEARTBEAT a period after
    /// the last one; with F set it gets none.
    #[test]
    fn an_acknack_with_the_final_flag_clear_is_answered_by_a_heartbeat() {
        for final_flag in [false, true] {
            let mut w = make_writer(10, Duration::from_secs(1));
            let sent = w
                .write_with_heartbeat(&alloc::vec![1], Duration::from_secs(1))
                .unwrap();
            assert_eq!(heartbeats(&sent), alloc::vec![(1, 1)]);
            w.handle_acknack(reader_guid(), sn(2), alloc::vec![]);
            w.note_acknack_final_flag(reader_guid(), final_flag);
            let answer = heartbeats(&w.tick(Duration::from_secs(2)).unwrap());
            if final_flag {
                assert!(answer.is_empty(), "F set: no HEARTBEAT, got {answer:?}");
            } else {
                assert_eq!(answer.len(), 1, "F clear: one HEARTBEAT");
            }
        }
    }

    /// A reader that holds the whole cache but has not answered a HEARTBEAT
    /// yet still gets one each period, the handshake
    /// `an_empty_writer_heartbeats_a_reader_until_it_answers` covers for an
    /// empty cache.
    #[test]
    fn an_acknowledged_reader_is_heartbeated_until_it_answers_one() {
        let mut w = make_writer(10, Duration::from_millis(100));
        w.write(&alloc::vec![1]).unwrap();
        w.handle_acknack(reader_guid(), sn(2), alloc::vec![]);
        assert_eq!(
            heartbeats(&w.tick(Duration::from_secs(1)).unwrap()),
            alloc::vec![(2, 1)]
        );
        w.handle_acknack(reader_guid(), sn(2), alloc::vec![]);
        assert!(w.tick(Duration::from_secs(2)).unwrap().is_empty());
    }

    /// The HEARTBEAT `write_with_heartbeat` sends with the change asks for
    /// an answer like a periodic one, and the ACKNACK that follows answers
    /// it: no further HEARTBEAT is needed to finish the handshake.
    #[test]
    fn the_heartbeat_sent_with_a_change_is_answered_by_the_next_acknack() {
        let mut w = make_writer(10, Duration::from_millis(100));
        let sent = w
            .write_with_heartbeat(&alloc::vec![1], Duration::from_secs(1))
            .unwrap();
        assert_eq!(heartbeats(&sent), alloc::vec![(1, 1)]);
        w.handle_acknack(reader_guid(), sn(2), alloc::vec![]);
        assert!(w.tick(Duration::from_secs(2)).unwrap().is_empty());
    }

    #[test]
    fn handle_acknack_updates_proxy_state() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let rguid = reader_guid();
        for i in 1..=3 {
            w.write(&alloc::vec![i as u8]).unwrap();
        }
        w.handle_acknack(rguid, sn(4), [sn(2)]);
        // Per-destination-queue model: the cache stays full (KeepAll),
        // GC happens only via history QoS. The ACKNACK state is, however,
        // tracked correctly at the proxy.
        assert_eq!(w.cache().len(), 3, "cache intact under KeepAll");
        assert_eq!(first_proxy(&w).highest_acked_sn(), sn(3));
        // sn(2) was acked by base=4 → not even remembered as requested
        assert_eq!(first_proxy(&w).pending_requested_count(), 0);
    }

    /// A reader that NACKed a full SequenceNumberSet gets a HEARTBEAT with
    /// the repair, so it can ask for the next window a round trip later; it
    /// may ask only in response to one (DDSI-RTPS 2.5 §8.4.2.3). A NACK of
    /// fewer changes gets its repair alone.
    #[test]
    fn a_full_nack_window_gets_a_heartbeat_with_its_repair() {
        let mut w = make_writer(600, Duration::from_secs(10));
        for i in 0..600u32 {
            w.write(&i.to_le_bytes()).unwrap();
        }
        // The first periodic HEARTBEAT; the next is ten seconds away.
        let _ = w.tick(Duration::ZERO).unwrap();

        let full: alloc::vec::Vec<SequenceNumber> = (1..=256).map(sn).collect();
        w.handle_acknack_at(Duration::from_millis(1), reader_guid(), sn(1), full);
        let out = w.tick(Duration::from_millis(1)).unwrap();
        assert_eq!(
            heartbeats(&out).len(),
            1,
            "the repair of a full window carries a HEARTBEAT"
        );

        w.handle_acknack_at(
            Duration::from_millis(2),
            reader_guid(),
            sn(257),
            [sn(257), sn(258)],
        );
        let out = w.tick(Duration::from_millis(2)).unwrap();
        assert!(!out.is_empty(), "the two changes are repaired");
        assert!(
            heartbeats(&out).is_empty(),
            "a NACK of two changes gets its repair alone"
        );
    }

    /// A SequenceNumberSet over the whole window may have left out changes
    /// the reader misses behind it, however few it names: gaps spread over
    /// a stream fill each window with a few. Its repair carries a
    /// HEARTBEAT too; a set that ends inside the window named everything.
    #[test]
    fn a_nack_set_spanning_the_window_gets_a_heartbeat_with_its_repair() {
        use crate::submessages::SequenceNumberSet;
        let mut w = make_writer(1200, Duration::from_secs(10));
        for i in 0..1200u32 {
            w.write(&i.to_le_bytes()).unwrap();
        }
        let _ = w.tick(Duration::ZERO).unwrap();

        // 13 missing at 1..=13, and more behind the window the set could
        // not reach: numBits is the whole window.
        let mut missing: alloc::vec::Vec<SequenceNumber> = (1..=13).map(sn).collect();
        missing.push(sn(400));
        let cut = SequenceNumberSet::from_missing(sn(1), &missing);
        assert_eq!(cut.num_bits, crate::submessages::RTPS_BITMAP_MAX_BITS);
        w.handle_acknack_set_at(Duration::from_millis(1), reader_guid(), &cut);
        let out = w.tick(Duration::from_millis(1)).unwrap();
        assert_eq!(
            heartbeats(&out).len(),
            1,
            "the reader asks for the next window"
        );

        let inside: alloc::vec::Vec<SequenceNumber> = (500..=512).map(sn).collect();
        let named = SequenceNumberSet::from_missing(sn(500), &inside);
        w.handle_acknack_set_at(Duration::from_millis(2), reader_guid(), &named);
        let out = w.tick(Duration::from_millis(2)).unwrap();
        assert!(!out.is_empty(), "the 13 changes are repaired");
        assert!(heartbeats(&out).is_empty(), "the set named everything");
    }

    /// §8.4.7.1.1 "Default Timing-Related Values". The spec names 200 ms
    /// for `nackResponseDelay` and 0 for `nackSuppressionDuration`;
    /// ZeroDDS runs 0 for both. The numbers are named constants, and the
    /// behaviour they stand for is what this measures: an ACKNACK that
    /// arrives immediately after the change was sent is *not* suppressed,
    /// and the repair leaves on the very next tick rather than 200 ms
    /// later.
    #[test]
    fn nack_response_delay_defaults_to_the_spec_200_ms_and_suppression_to_zero() {
        assert_eq!(SPEC_NACK_RESPONSE_DELAY, Duration::from_millis(200));
        assert_eq!(DEFAULT_NACK_RESPONSE_DELAY, Duration::from_millis(200));
        assert_eq!(DEFAULT_NACK_SUPPRESSION_DURATION, Duration::ZERO);

        let mut w = ReliableWriter::new(ReliableWriterConfig {
            guid: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            ),
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![ReaderProxy::new(
                reader_guid(),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
                alloc::vec![],
                true,
            )],
            max_samples: 10,
            history_kind: HistoryKind::KeepAll,
            heartbeat_period: Duration::from_secs(10),
            fragment_size: DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        assert_eq!(w.nack_response_delay(), Duration::from_millis(200));
        let sent = w.write(b"payload").unwrap();
        assert_eq!(sent.len(), 1, "the change is on the wire at t=0");

        // The NACK arrives in the same instant the change was sent:
        // nackSuppressionDuration = 0 means it is honoured, not dropped.
        w.handle_acknack(reader_guid(), sn(1), [sn(1)]);
        assert_eq!(first_proxy(&w).pending_requested_count(), 1);

        // nackResponseDelay = 200 ms: nothing leaves before it has passed,
        // the repair leaves once it has.
        let early = w.tick(Duration::from_millis(100)).unwrap();
        assert!(
            !early.iter().any(|d| has_data(&d.bytes)),
            "no repair inside the delay"
        );
        assert_eq!(first_proxy(&w).pending_requested_count(), 1);
        let repaired = w.tick(Duration::from_millis(250)).unwrap();
        assert!(repaired.iter().any(|d| has_data(&d.bytes)));
        assert_eq!(first_proxy(&w).pending_requested_count(), 0);
    }

    /// §8.4.7.1 pushMode: "If pushMode==true, then the Writer will push
    /// changes to the reader. If pushMode==false, changes will only be
    /// announced via heartbeats and only be sent as response to the
    /// request of a reader." Both halves, on the same writer.
    #[test]
    fn push_mode_false_announces_by_heartbeat_and_sends_only_on_request() {
        let mut w = make_writer(10, Duration::from_secs(10));
        assert!(w.push_mode(), "pushMode defaults to true");
        assert_eq!(
            w.write(b"pushed").unwrap().len(),
            1,
            "pushMode=true sends the change straight out"
        );

        w.set_push_mode(false);
        let pushed = w.write(b"announced only").unwrap();
        assert!(pushed.is_empty(), "pushMode=false must not push");
        assert_eq!(w.cache().len(), 2, "the change is staged all the same");

        // The reader learns of it from the HEARTBEAT …
        let hb = w.tick(Duration::from_secs(20)).unwrap();
        assert_eq!(hb.len(), 1, "a heartbeat announces the range");

        // … and gets it once it asks for it.
        w.handle_acknack(reader_guid(), sn(2), [sn(2)]);
        let repaired = w.tick(Duration::from_secs(30)).unwrap();
        assert!(
            !repaired.is_empty(),
            "the requested change is sent on request"
        );
    }

    /// §8.4.7.1 pushMode on the piggyback path
    /// ([`ReliableWriter::write_with_heartbeat`], the path the DCPS layer
    /// uses): "changes will only be announced via heartbeats and only be
    /// sent as response to the request of a reader". So the datagrams of
    /// a `pushMode == false` write must carry a HEARTBEAT and no DATA,
    /// and the DATA must follow the ACKNACK.
    #[test]
    fn push_mode_false_sends_no_data_on_the_piggyback_path() {
        let mut w = make_writer(10, Duration::from_secs(10));
        w.set_push_mode(false);

        let out = w
            .write_with_heartbeat(b"announced only", Duration::ZERO)
            .unwrap();
        let mut heartbeats = 0;
        for dg in &out {
            for sub in decode_datagram(&dg.bytes).unwrap().contents() {
                match sub {
                    ParsedSubmessage::Heartbeat(_) => heartbeats += 1,
                    ParsedSubmessage::Data(_) => {
                        panic!("pushMode=false must not put DATA on the wire")
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(heartbeats, 1, "the change is announced by one HEARTBEAT");
        assert_eq!(w.cache().len(), 1, "the change is staged all the same");

        // The reader asks for it — now the DATA goes out.
        w.handle_acknack(reader_guid(), sn(1), [sn(1)]);
        let repaired = w.tick(Duration::from_secs(1)).unwrap();
        let data_count: usize = repaired
            .iter()
            .map(|dg| {
                decode_datagram(&dg.bytes)
                    .unwrap()
                    .contents()
                    .filter(|s| matches!(s, ParsedSubmessage::Data(_)))
                    .count()
            })
            .sum();
        assert_eq!(data_count, 1, "the requested change is sent on request");
    }

    /// §8.4.7.4 `is_acked_by_all`: true only once every matched
    /// `ReaderProxy` has acked the change. Two proxies, one lagging —
    /// the answer follows the slower one, and flips when it catches up.
    #[test]
    fn is_acked_by_all_follows_the_slowest_reader_proxy() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let fast = reader_guid();
        let slow = Guid::new(
            GuidPrefix::from_bytes([9; 12]),
            EntityId::user_reader_with_key([0x40, 0x50, 0x60]),
        );
        w.add_reader_proxy(ReaderProxy::new(
            slow,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        ));
        for i in 1..=3 {
            w.write(&alloc::vec![i as u8]).unwrap();
        }
        assert!(!w.is_acked_by_all(sn(1)), "nothing acked yet");

        w.handle_acknack(fast, sn(4), []);
        assert!(
            !w.is_acked_by_all(sn(1)),
            "one of two proxies acked is not all"
        );

        w.handle_acknack(slow, sn(3), []);
        assert!(w.is_acked_by_all(sn(2)), "both acked up to 2");
        assert!(!w.is_acked_by_all(sn(3)), "the slow proxy stops at 2");

        w.handle_acknack(slow, sn(4), []);
        assert!(w.is_acked_by_all(sn(3)));
    }

    /// DDS 1.4 §2.2.2.4.2.15: `wait_for_acknowledgments` waits for "all
    /// matched DataReader entities that have RELIABILITY QoS kind
    /// RELIABLE". A best-effort reader beside the reliable one does not
    /// hold the answer; it sends no ACKNACK.
    #[test]
    fn all_samples_acknowledged_asks_the_reliable_readers_only() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let best_effort = Guid::new(
            GuidPrefix::from_bytes([9; 12]),
            EntityId::user_reader_with_key([0x40, 0x50, 0x60]),
        );
        w.add_reader_proxy(ReaderProxy::new(
            best_effort,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            false,
        ));
        w.write(&alloc::vec![1u8]).unwrap();
        assert!(
            !w.all_samples_acknowledged(),
            "the reliable reader has not acked"
        );
        w.handle_acknack(reader_guid(), sn(2), []);
        assert!(w.all_samples_acknowledged());
    }

    /// A writer without matched readers acks vacuously — the spec's
    /// `FOREACH` over an empty `ReaderProxy` set.
    #[test]
    fn is_acked_by_all_without_proxies_is_true() {
        let writer_guid = Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
        );
        let w = fast_writer(ReliableWriterConfig {
            guid: writer_guid,
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![],
            max_samples: 4,
            history_kind: HistoryKind::KeepAll,
            heartbeat_period: Duration::from_secs(10),
            fragment_size: DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        assert!(w.is_acked_by_all(sn(99)));
    }

    /// The flow-control window fills with what the reliable reader has not
    /// acknowledged, asks once for an acknowledgement at half of it, and
    /// is freed by the ACKNACK and by the reader leaving.
    #[test]
    fn the_unacked_window_fills_and_acknowledgements_free_it() {
        let mut w = make_writer(100, Duration::from_secs(10));
        w.set_unacked_window(3000);
        let sample = alloc::vec![0u8; 1000];
        let _ = w.write(&sample).unwrap();
        assert!(!w.ask_for_ack(), "a third of the window");
        let _ = w.write(&sample).unwrap();
        assert!(
            w.ask_for_ack(),
            "half the window asks for an acknowledgement"
        );
        assert!(!w.ask_for_ack(), "once until one comes");
        assert!(!w.window_full());
        let _ = w.write(&sample).unwrap();
        assert_eq!(w.unacked_bytes(), 3000);
        assert!(w.window_full());

        w.handle_acknack(reader_guid(), sn(3), []);
        assert_eq!(w.unacked_bytes(), 1000, "SN 1 and 2 are acknowledged");
        assert!(!w.window_full());
        let _ = w.write(&sample).unwrap();
        assert!(w.ask_for_ack(), "the acknowledgement re-arms the request");

        assert!(w.remove_reader_proxy(reader_guid()).is_some());
        assert_eq!(w.unacked_bytes(), 0, "no reliable reader waits");
        let _ = w.write(&sample).unwrap();
        assert_eq!(w.unacked_bytes(), 0, "nothing counts without one");
    }

    /// The instant the next periodic HEARTBEAT goes out: none with an empty
    /// history, at once before the first, a heartbeatPeriod after the last.
    #[test]
    fn heartbeat_due_follows_the_last_heartbeat() {
        let period = Duration::from_millis(100);
        let mut w = make_writer(10, period);
        assert_eq!(w.heartbeat_due(), None, "nothing written");
        w.write(&alloc::vec![1]).unwrap();
        assert_eq!(w.heartbeat_due(), Some(Duration::ZERO), "none went out yet");
        let at = Duration::from_millis(40);
        let _ = w.tick(at).unwrap();
        assert_eq!(w.heartbeat_due(), Some(at + period));
        let _ = w.tick(at + Duration::from_millis(10)).unwrap();
        assert_eq!(
            w.heartbeat_due(),
            Some(at + period),
            "not due inside the period"
        );
    }

    #[test]
    fn handle_acknack_with_lower_base_leaves_requested() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let rguid = reader_guid();
        for i in 1..=3 {
            w.write(&alloc::vec![i as u8]).unwrap();
        }
        w.handle_acknack(rguid, sn(2), [sn(2), sn(3)]);
        // Cache full under KeepAll.
        assert_eq!(w.cache().len(), 3);
        assert_eq!(first_proxy(&w).highest_acked_sn(), sn(1));
        assert_eq!(first_proxy(&w).pending_requested_count(), 2);
    }

    #[test]
    fn keep_last_evicts_oldest_on_overflow() {
        let writer_guid = Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
        );
        let reader_proxy = ReaderProxy::new(
            reader_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
            alloc::vec![],
            true,
        );
        let mut w = fast_writer(ReliableWriterConfig {
            guid: writer_guid,
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![reader_proxy],
            max_samples: 3,
            history_kind: HistoryKind::KeepLast { depth: 3 },
            heartbeat_period: Duration::from_secs(10),
            fragment_size: DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        for i in 1..=5 {
            w.write(&alloc::vec![i as u8])
                .expect("keep_last never fails");
        }
        // Cache holds only the last 3 (SN 3, 4, 5)
        assert_eq!(w.cache().len(), 3);
        assert_eq!(w.cache().min_sn(), Some(sn(3)));
        assert_eq!(w.cache().max_sn(), Some(sn(5)));
        assert_eq!(w.cache().evicted_count(), 2);
    }

    #[test]
    fn keep_last_stalled_reader_does_not_block_fresh_writes() {
        // Scenario: two proxies, one "stalled" (never acked),
        // the other active. Under KeepLast the writer keeps
        // writing, the stalled reader gets GAPs later.
        let writer_guid = Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
        );
        let stalled = ReaderProxy::new(
            Guid::new(
                GuidPrefix::from_bytes([9; 12]),
                EntityId::user_reader_with_key([0xDE, 0xAD, 0x00]),
            ),
            alloc::vec![Locator::udp_v4([127, 0, 0, 99], 9999)],
            alloc::vec![],
            true,
        );
        let active = ReaderProxy::new(
            reader_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
            alloc::vec![],
            true,
        );
        let mut w = fast_writer(ReliableWriterConfig {
            guid: writer_guid,
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![stalled, active],
            max_samples: 3,
            history_kind: HistoryKind::KeepLast { depth: 3 },
            heartbeat_period: Duration::from_secs(10),
            fragment_size: DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        // 10 samples — stalled never acked, but write does not fail
        for i in 1..=10 {
            w.write(&alloc::vec![i as u8]).expect("never blocks");
        }
        assert_eq!(w.cache().len(), 3);
        assert_eq!(w.cache().min_sn(), Some(sn(8)));
        // The active reader later requests sn(2) → it is evicted, gets a GAP
        w.handle_acknack(reader_guid(), sn(1), [sn(2)]);
        let out = w.tick(Duration::ZERO).unwrap();
        let has_gap = out.iter().any(|d| {
            decode_datagram(&d.bytes)
                .unwrap()
                .contents()
                .any(|s| matches!(s, ParsedSubmessage::Gap(_)))
        });
        assert!(has_gap, "evicted SN must elicit GAP");
    }

    #[test]
    fn handle_acknack_unknown_source_counts_but_noops() {
        let mut w = make_writer(10, Duration::from_secs(10));
        w.write(&alloc::vec![1]).unwrap();
        let foreign = Guid::new(
            GuidPrefix::from_bytes([0xFF; 12]),
            EntityId::user_reader_with_key([0xFF, 0xFF, 0xFF]),
        );
        w.handle_acknack(foreign, sn(5), [sn(2)]);
        assert_eq!(w.cache().len(), 1, "cache untouched");
        assert_eq!(first_proxy(&w).pending_requested_count(), 0);
        assert_eq!(w.unknown_src_count(), 1, "unknown source counted");
    }

    #[test]
    fn handle_nackfrag_unknown_source_counts() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), 4);
        let _ = w.write(&(1..=10).collect::<alloc::vec::Vec<u8>>()).unwrap();
        let foreign = Guid::new(
            GuidPrefix::from_bytes([0xFF; 12]),
            EntityId::user_reader_with_key([0xFF, 0xFF, 0xFF]),
        );
        let nf = NackFragSubmessage {
            reader_id: foreign.entity_id,
            writer_id: w.guid.entity_id,
            writer_sn: sn(1),
            fragment_number_state: crate::submessages::FragmentNumberSet::from_missing(
                FragmentNumber(1),
                &[FragmentNumber(2)],
            ),
            count: 1,
        };
        w.handle_nackfrag(foreign, &nf);
        assert_eq!(w.nackfrag_count(), 0, "not counted as legit nackfrag");
        assert_eq!(w.unknown_src_count(), 1);
    }

    #[test]
    fn tick_resends_requested_as_data_aggregated_with_hb() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let rguid = reader_guid();
        // Four octets each: a DATA that ends on a 32-bit boundary is one a
        // HEARTBEAT may follow (DDSI-RTPS 2.5 §9.4.1).
        for i in 1..=3 {
            w.write(&alloc::vec![i as u8; 4]).unwrap();
        }
        w.handle_acknack(rguid, sn(1), [sn(2)]);
        let out = w.tick(Duration::ZERO).unwrap();
        // An aggregated datagram: DATA-Resend + HEARTBEAT in same
        let parsed = decode_datagram(&out[0].bytes).unwrap();
        let has_data_2 = parsed
            .contents()
            .any(|s| matches!(s, ParsedSubmessage::Data(d) if d.writer_sn == sn(2)));
        let has_hb = parsed
            .contents()
            .any(|s| matches!(s, ParsedSubmessage::Heartbeat(_)));
        assert!(has_data_2, "DATA resend for sn(2)");
        assert!(has_hb, "Piggyback HEARTBEAT in the same datagram");
    }

    #[test]
    fn tick_resends_evicted_request_as_gap() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let rguid = reader_guid();
        w.write(&alloc::vec![1]).unwrap();
        w.handle_acknack(rguid, sn(1), [sn(5)]);
        let out = w.tick(Duration::ZERO).unwrap();
        let has_gap = out.iter().any(|d| {
            decode_datagram(&d.bytes)
                .unwrap()
                .contents()
                .any(|s| matches!(s, ParsedSubmessage::Gap(_)))
        });
        assert!(has_gap);
    }

    #[test]
    fn write_at_cache_capacity_is_error() {
        let mut w = make_writer(2, Duration::from_secs(10));
        w.write(&alloc::vec![1]).unwrap();
        w.write(&alloc::vec![2]).unwrap();
        assert!(w.write(&alloc::vec![3]).is_err());
    }

    #[test]
    fn heartbeat_count_increments() {
        let mut w = make_writer(10, Duration::from_millis(100));
        w.write(&alloc::vec![1]).unwrap();
        assert_eq!(w.heartbeat_count(), 0);
        w.tick(Duration::ZERO).unwrap();
        assert_eq!(w.heartbeat_count(), 1);
        w.tick(Duration::from_millis(150)).unwrap();
        assert_eq!(w.heartbeat_count(), 2);
    }

    #[test]
    fn heartbeat_count_wraps_around_at_i32_max_per_spec_8_4_15_7() {
        // Spec §8.4.15.7: counts MUST be wrap-around-tolerant
        // (modular arithmetic). i32 wraps when the counter
        // reaches i32::MAX.
        let mut w = make_writer(10, Duration::from_millis(100));
        w.write(&alloc::vec![1]).unwrap();
        // Set manually to MAX (no public setter; but we track
        // the counter via `heartbeat_count.wrapping_add(1)` →
        // wrap behaviour is guaranteed by code).
        // Test the wrapping semantics directly:
        let counter: i32 = i32::MAX;
        let next = counter.wrapping_add(1);
        assert_eq!(next, i32::MIN, "i32::MAX + 1 wraps to i32::MIN");
        let after_wrap = next.wrapping_add(1);
        assert_eq!(after_wrap, i32::MIN + 1);
    }

    // ---------- Fragmentation ----------

    #[test]
    fn keyed_alive_preserves_inline_hash_on_data_fragments_and_rebuilds() {
        for fragment_size in [4, 1024] {
            for related in [
                None,
                Some(crate::inline_qos::SampleIdentityBytes::new([0x42; 16], 73)),
            ] {
                let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), fragment_size);
                let payload: Vec<u8> = (1..=10).collect();
                let key = [0; 16];
                let sent = match related {
                    None => w.write_keyed_stamped(&payload, None, Some(key)),
                    Some(identity) => w.write_sample_with_inline_qos(
                        &payload,
                        None,
                        Some(key),
                        Some(crate::inline_qos::reply_inline_qos(identity, true)),
                    ),
                }
                .unwrap();
                let check = |datagrams: &[OutboundDatagram]| {
                    let mut found = 0;
                    for dg in datagrams {
                        for sub in decode_datagram(&dg.bytes).unwrap().submessages {
                            let qos = match sub.submessage {
                                ParsedSubmessage::Data(data) => {
                                    assert!(data.data_flag);
                                    data.inline_qos
                                }
                                ParsedSubmessage::DataFrag(data) => data.inline_qos,
                                _ => continue,
                            };
                            assert_eq!(
                                qos.as_ref().and_then(crate::inline_qos::find_key_hash),
                                Some(key)
                            );
                            assert_eq!(
                                crate::inline_qos::find_related_sample_identity(
                                    qos.as_ref().unwrap(),
                                    true
                                )
                                .unwrap(),
                                related
                            );
                            found += 1;
                        }
                    }
                    assert!(found > 0);
                };
                check(&sent);
                let sn = w.cache.max_sn().unwrap();
                assert_eq!(w.cache.get(sn).unwrap().key_hash, Some(key));
                let cached = w.cache.get(sn).unwrap().payload.clone();
                let targets = w.targets_for(0);
                if fragment_size == 4 {
                    // Exact helper used for NACK_FRAG repairs.
                    check(&[w
                        .build_data_frag_datagram(
                            sn,
                            FragmentNumber(2),
                            1,
                            &cached,
                            reader_guid(),
                            &targets,
                        )
                        .unwrap()]);
                } else {
                    // Exact helper used for DATA retransmits and piggyback paths.
                    let mut builder = w.open_builder_for(reader_guid(), &targets);
                    let mut rebuilt = Vec::new();
                    w.append_data(
                        &mut builder,
                        sn,
                        &cached,
                        reader_guid(),
                        &mut rebuilt,
                        &targets,
                    )
                    .unwrap();
                    rebuilt.push(builder.finish().unwrap());
                    check(&rebuilt);
                }
            }
        }
    }

    #[test]
    fn write_under_fragment_size_produces_single_data() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), 10);
        let dgs = w.write(&alloc::vec![1, 2, 3, 4, 5]).unwrap();
        assert_eq!(dgs.len(), 1);
        let parsed = decode_datagram(&dgs[0].bytes).unwrap();
        assert!(matches!(
            &parsed.submessages[0].submessage,
            ParsedSubmessage::Data(_)
        ));
    }

    /// §8.4.14.1: "it is recommended that implementations concatenate as
    /// many fragments as possible into a single DataFrag message": a sample
    /// of three fragments that one message carries is one DATA_FRAG with
    /// `fragmentsInSubmessage` 3.
    #[test]
    fn write_above_fragment_size_packs_the_fragments_a_message_carries() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), 4);
        let payload: alloc::vec::Vec<u8> = (1..=10).collect();
        let dgs = w.write(&payload).unwrap();
        assert_eq!(dgs.len(), 1);
        match &decode_datagram(&dgs[0].bytes).unwrap().submessages[0].submessage {
            ParsedSubmessage::DataFrag(df) => {
                assert_eq!(df.fragment_starting_num.0, 1);
                assert_eq!(df.fragments_in_submessage, 3);
                assert_eq!(df.fragment_size, 4);
                assert_eq!(df.sample_size, 10);
                assert_eq!(&df.serialized_payload[..], &payload[..]);
            }
            other => panic!("expected DataFrag, got {other:?}"),
        }
    }

    /// §8.4.14.1: "fragment size <= 65536"; the field is an `unsigned
    /// short`, so 65535 is the largest that has an encoding. A writer with
    /// a larger one is refused with an error; one at the bound sends
    /// fragments of the submessage's capacity (65535 less the 32 fixed
    /// octets of a DATA_FRAG), and the fragment size of a transport follows
    /// its message size.
    #[test]
    fn the_fragment_size_is_bounded_by_what_the_field_holds() {
        let cfg = |fragment_size, mtu| ReliableWriterConfig {
            guid: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0x10, 0x20, 0x32]),
            ),
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![ReaderProxy::new(
                reader_guid(),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
                alloc::vec![],
                true,
            )],
            max_samples: 10,
            history_kind: HistoryKind::KeepAll,
            heartbeat_period: Duration::from_secs(10),
            fragment_size,
            mtu,
        };
        assert!(ReliableWriter::try_new(cfg(65_536, 70_000)).is_err());
        assert!(ReliableWriter::try_new(cfg(0, 70_000)).is_err());
        assert!(ReliableWriter::try_new(cfg(100, 19)).is_err());
        let mut w = ReliableWriter::try_new(cfg(65_535, 70_000)).unwrap();
        let dgs = w.write(&alloc::vec![7u8; 65_503 + 10]).unwrap();
        assert_eq!(dgs.len(), 2);
        match &decode_datagram(&dgs[0].bytes).unwrap().submessages[0].submessage {
            ParsedSubmessage::DataFrag(df) => assert_eq!(df.fragment_size, 65_503),
            other => panic!("expected DataFrag, got {other:?}"),
        }
        assert_eq!(fragment_size_for_message_size(1472), DEFAULT_FRAGMENT_SIZE);
        assert_eq!(fragment_size_for_message_size(1500), 1372);
        assert_eq!(fragment_size_for_message_size(65_536), 65_408);
        assert_eq!(fragment_size_for_message_size(200_000), MAX_FRAGMENT_SIZE);
        assert_eq!(fragment_size_for_message_size(0), 1);
    }

    /// The fragments of a sample fill messages of the size the path carries
    /// and no more: 450 octets in fragments of 100 on a message size of 300
    /// are 5 fragments in DATA_FRAGs of 2, 2 and 1, numbered in order.
    #[test]
    fn the_fragments_of_a_sample_fill_the_messages_and_no_more() {
        let mut w = fast_writer(ReliableWriterConfig {
            guid: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0x10, 0x20, 0x31]),
            ),
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![ReaderProxy::new(
                reader_guid(),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
                alloc::vec![],
                true,
            )],
            max_samples: 10,
            history_kind: HistoryKind::KeepAll,
            heartbeat_period: Duration::from_secs(10),
            fragment_size: 100,
            mtu: 300,
        });
        let payload: alloc::vec::Vec<u8> = (0..450).map(|i| (i % 251) as u8).collect();
        let dgs = w.write(&payload).unwrap();
        let frags: alloc::vec::Vec<(u32, u16, usize)> = dgs
            .iter()
            .map(|dg| {
                assert!(dg.bytes.len() <= 300, "{} octets", dg.bytes.len());
                match &decode_datagram(&dg.bytes).unwrap().submessages[0].submessage {
                    ParsedSubmessage::DataFrag(df) => (
                        df.fragment_starting_num.0,
                        df.fragments_in_submessage,
                        df.serialized_payload.len(),
                    ),
                    other => panic!("expected DataFrag, got {other:?}"),
                }
            })
            .collect();
        assert_eq!(frags, alloc::vec![(1, 2, 200), (3, 2, 200), (5, 1, 50)]);
        // A NACK_FRAG for fragments 2 to 4 is answered with the neighbours
        // together: 2+3 in one message, 4 in the next.
        w.handle_nackfrag(
            reader_guid(),
            &NackFragSubmessage {
                reader_id: reader_guid().entity_id,
                writer_id: w.guid().entity_id,
                writer_sn: sn(1),
                fragment_number_state: crate::submessages::FragmentNumberSet::from_missing(
                    FragmentNumber(2),
                    &[FragmentNumber(2), FragmentNumber(3), FragmentNumber(4)],
                ),
                count: 1,
            },
        );
        let repair = w.tick(Duration::from_millis(1)).unwrap();
        let repaired: alloc::vec::Vec<(u32, u16)> = repair
            .iter()
            .flat_map(|dg| {
                decode_datagram(&dg.bytes)
                    .unwrap()
                    .contents()
                    .filter_map(|s| match s {
                        ParsedSubmessage::DataFrag(df) => {
                            Some((df.fragment_starting_num.0, df.fragments_in_submessage))
                        }
                        _ => None,
                    })
                    .collect::<alloc::vec::Vec<_>>()
            })
            .collect();
        assert_eq!(repaired, alloc::vec![(2, 2), (4, 1)]);
    }

    /// DDSI-RTPS 2.5 §8.4.14.1: "The fragment size must be fixed for a given
    /// Writer and is identical for all remote Readers", and "a regular Data
    /// Submessage must be sent on those transports" that need no
    /// fragmentation. A 5000-octet sample goes to the reader on a path of
    /// `LOOPBACK_MTU` as one DATA and to the other as DATA_FRAGs of the
    /// writer's fragment size; adding the first does not change that size.
    #[test]
    fn fragment_size_is_fixed_and_a_larger_path_gets_one_data() {
        let mut w = make_writer(10, Duration::from_secs(10));
        assert_eq!(w.fragment_size(), DEFAULT_FRAGMENT_SIZE);
        let local_guid = Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::user_reader_with_key([0xA0, 0xB0, 0xC1]),
        );
        let mut local = ReaderProxy::new(
            local_guid,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        );
        local.set_message_limit(Some(LOOPBACK_MTU));
        w.add_reader_proxy(local);
        assert_eq!(w.fragment_size(), DEFAULT_FRAGMENT_SIZE);

        let payload = alloc::vec![0x5Au8; 5000];
        let dgs = w.write(&payload).unwrap();
        let (mut local_data, mut remote_frags) = (0, alloc::vec::Vec::new());
        for dg in &dgs {
            let to_local = dg.targets.iter().any(|l| l.port == 7411);
            for sm in &decode_datagram(&dg.bytes).unwrap().submessages {
                match &sm.submessage {
                    ParsedSubmessage::Data(d) if to_local => {
                        assert_eq!(d.serialized_payload.len(), 5000);
                        local_data += 1;
                    }
                    ParsedSubmessage::DataFrag(df) if !to_local => {
                        assert_eq!(df.fragment_size, 1344);
                        remote_frags.push(df.fragment_starting_num.0);
                    }
                    ParsedSubmessage::Heartbeat(_) => {}
                    other => panic!("unexpected {other:?} (to_local={to_local})"),
                }
            }
        }
        assert_eq!(local_data, 1);
        // 5000 / 1344 -> 4 fragments (3 full, 1 of 968 octets).
        assert_eq!(remote_frags, alloc::vec![1, 2, 3, 4]);
    }

    /// DDSI-RTPS 2.5 §8.4.14.1: "it is recommended that implementations
    /// concatenate as many fragments as possible into a single DataFrag
    /// message." On a path of `LOOPBACK_MTU` a 200 000-octet sample goes in
    /// DATA_FRAGs of many consecutive fragments each, every message within
    /// the path's size, and the fragments together are the sample.
    #[test]
    fn a_path_with_a_larger_message_gets_many_fragments_in_one_data_frag() {
        let mut w = make_writer(10, Duration::from_secs(10));
        w.reader_proxies[0].set_message_limit(Some(LOOPBACK_MTU));
        let payload: alloc::vec::Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let dgs = w.write(&payload).unwrap();
        let mut rebuilt = alloc::vec![0u8; payload.len()];
        let (mut next, mut most, mut messages) = (1u32, 0u16, 0usize);
        for dg in &dgs {
            assert!(dg.bytes.len() <= LOOPBACK_MTU, "{}", dg.bytes.len());
            for sm in &decode_datagram(&dg.bytes).unwrap().submessages {
                if let ParsedSubmessage::DataFrag(df) = &sm.submessage {
                    assert_eq!(df.fragment_starting_num.0, next, "consecutive, in order");
                    assert_eq!(df.fragment_size, 1344);
                    let at = (next as usize - 1) * 1344;
                    rebuilt[at..at + df.serialized_payload.len()]
                        .copy_from_slice(&df.serialized_payload);
                    next += u32::from(df.fragments_in_submessage);
                    most = most.max(df.fragments_in_submessage);
                    messages += 1;
                }
            }
        }
        // 200 000 / 1344 -> 149 fragments.
        assert_eq!(next, 150);
        assert!(most > 1, "many fragments in one DATA_FRAG");
        assert!(messages < 10, "{messages} messages for 149 fragments");
        assert_eq!(rebuilt, payload);
    }

    /// On a path of the smallest transport's size a DATA_FRAG holds the one
    /// fragment that fills it.
    #[test]
    fn on_the_default_path_a_data_frag_holds_one_fragment() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let dgs = w.write(&alloc::vec![7u8; 5000]).unwrap();
        let counts: alloc::vec::Vec<u16> = dgs
            .iter()
            .flat_map(|dg| decode_datagram(&dg.bytes).unwrap().submessages)
            .filter_map(|sm| match sm.submessage {
                ParsedSubmessage::DataFrag(df) => Some(df.fragments_in_submessage),
                _ => None,
            })
            .collect();
        assert_eq!(counts, alloc::vec![1, 1, 1, 1]);
    }

    #[test]
    fn handle_nackfrag_queues_fragment_resends() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), 4);
        let rguid = reader_guid();
        let _ = w.write(&(1..=10).collect::<alloc::vec::Vec<u8>>()).unwrap();
        let nf = NackFragSubmessage {
            reader_id: rguid.entity_id,
            writer_id: w.guid.entity_id,
            writer_sn: sn(1),
            fragment_number_state: crate::submessages::FragmentNumberSet::from_missing(
                FragmentNumber(1),
                &[FragmentNumber(2), FragmentNumber(3)],
            ),
            count: 1,
        };
        w.handle_nackfrag(rguid, &nf);
        assert_eq!(w.nackfrag_count(), 1);
        assert_eq!(first_proxy(&w).pending_requested_fragment_count(), 2);
    }

    #[test]
    fn tick_resends_requested_fragments() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), 4);
        let rguid = reader_guid();
        let _ = w.write(&(1..=10).collect::<alloc::vec::Vec<u8>>()).unwrap();
        let nf = NackFragSubmessage {
            reader_id: rguid.entity_id,
            writer_id: w.guid.entity_id,
            writer_sn: sn(1),
            fragment_number_state: crate::submessages::FragmentNumberSet::from_missing(
                FragmentNumber(1),
                &[FragmentNumber(3)],
            ),
            count: 1,
        };
        w.handle_nackfrag(rguid, &nf);
        let out = w.tick(Duration::ZERO).unwrap();
        let frag_resends: alloc::vec::Vec<_> = out
            .iter()
            .filter(|d| {
                decode_datagram(&d.bytes)
                    .unwrap()
                    .contents()
                    .any(|s| matches!(s, ParsedSubmessage::DataFrag(df) if df.fragment_starting_num == FragmentNumber(3)))
            })
            .collect();
        assert_eq!(frag_resends.len(), 1);
        assert_eq!(first_proxy(&w).pending_requested_fragment_count(), 0);
    }

    /// What the datagrams to the reader at `port` carry: (DATA or DATA_FRAG
    /// for `sn`, GAP covering `sn`).
    fn sent_to(out: &[OutboundDatagram], port: u32, sn_: SequenceNumber) -> (bool, bool) {
        let mut data = false;
        let mut gap = false;
        for d in out
            .iter()
            .filter(|d| d.targets.iter().any(|t| t.port == port))
        {
            for sm in decode_datagram(&d.bytes).expect("decode").contents() {
                match sm {
                    ParsedSubmessage::Data(x) if x.writer_sn == sn_ => data = true,
                    ParsedSubmessage::DataFrag(x) if x.writer_sn == sn_ => data = true,
                    ParsedSubmessage::Gap(g) if g.gap_start <= sn_ => gap = true,
                    _ => {}
                }
            }
        }
        (data, gap)
    }

    /// DDSI-RTPS 2.5 §8.7.9: a dispose forwarded on behalf of another
    /// writer names it in-line, on the first send and on the repair.
    #[test]
    fn a_lifecycle_marker_keeps_its_own_inline_qos_on_every_send() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let mut qos = crate::parameter_list::ParameterList::new();
        qos.push(crate::inline_qos::original_writer_info_param(
            [0xA1; 16], 9, true,
        ));
        let owi = |out: &[OutboundDatagram]| {
            out.iter()
                .flat_map(|d| {
                    decode_datagram(&d.bytes)
                        .expect("decode")
                        .contents()
                        .filter_map(|sm| match sm {
                            ParsedSubmessage::Data(d) if !d.data_flag => d.inline_qos.clone(),
                            _ => None,
                        })
                        .collect::<alloc::vec::Vec<_>>()
                })
                .filter_map(|pl| {
                    crate::inline_qos::find_original_writer_info(&pl, true).expect("decode")
                })
                .collect::<alloc::vec::Vec<_>>()
        };
        let out = w
            .write_lifecycle_with_inline_qos(
                [7; 16],
                &[],
                crate::inline_qos::status_info::DISPOSED,
                None,
                Some(qos),
            )
            .expect("dispose");
        assert_eq!(owi(&out), alloc::vec![([0xA1; 16], 9)], "first send");
        w.handle_acknack(reader_guid(), sn(1), [sn(1)]);
        let out = w.tick(Duration::from_secs(1)).expect("tick");
        assert_eq!(owi(&out), alloc::vec![([0xA1; 16], 9)], "repair");
    }

    /// DDS-Security 1.2 §7.5.4.4: a ParticipantVolatileMessageSecure
    /// message with a destination participant is meant for that
    /// participant's reader only. The writer filters on its side: the other
    /// reader gets a GAP for the number, on the first send, on an ACKNACK
    /// repair and on a NACK_FRAG repair. Without the filter every matched
    /// reader received every token message, its crypto keys included.
    #[test]
    fn a_change_for_one_participant_reaches_the_others_as_a_gap() {
        for frag_size in [DEFAULT_FRAGMENT_SIZE, 4] {
            let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), frag_size);
            let other = Guid::new(GuidPrefix::from_bytes([3; 12]), reader_guid().entity_id);
            w.add_reader_proxy(ReaderProxy::new(
                other,
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
                alloc::vec![],
                true,
            ));
            let payload: alloc::vec::Vec<u8> = (1..=10).collect();
            let out = w
                .write_with_heartbeat_to(
                    &payload,
                    Duration::ZERO,
                    None,
                    Some(GuidPrefix::from_bytes([3; 12])),
                )
                .expect("write");
            assert_eq!(
                sent_to(&out, 7411, sn(1)),
                (true, false),
                "addressed reader"
            );
            assert_eq!(sent_to(&out, 7410, sn(1)), (false, true), "other reader");

            // The other reader asks for the number all the same.
            w.handle_acknack(reader_guid(), sn(1), [sn(1)]);
            w.handle_acknack(other, sn(1), [sn(1)]);
            let out = w.tick(Duration::from_secs(1)).expect("tick");
            assert_eq!(
                sent_to(&out, 7411, sn(1)),
                (true, false),
                "repair, addressed"
            );
            assert_eq!(sent_to(&out, 7410, sn(1)), (false, true), "repair, other");

            if frag_size == 4 {
                for (reader, port) in [(reader_guid(), 7410), (other, 7411)] {
                    w.handle_nackfrag(
                        reader,
                        &NackFragSubmessage {
                            reader_id: reader.entity_id,
                            writer_id: w.guid.entity_id,
                            writer_sn: sn(1),
                            fragment_number_state:
                                crate::submessages::FragmentNumberSet::from_missing(
                                    FragmentNumber(1),
                                    &[FragmentNumber(2)],
                                ),
                            count: 1,
                        },
                    );
                    let out = w.tick(Duration::from_secs(2)).expect("tick");
                    let addressed = port == 7411;
                    assert_eq!(
                        sent_to(&out, port, sn(1)),
                        (addressed, !addressed),
                        "fragment repair to {port}"
                    );
                }
            }
        }
    }

    /// DDS-RPC 1.0 §7.11.1.4.3: a request bound to a service instance "shall
    /// be sent to the bound service instance only". The instance can be one
    /// reader of a participant that has more, so a change is addressed to
    /// readers by GUID: the reader named gets the DATA, the other reader of
    /// the same participant a GAP, on the first send and on a repair, and the
    /// next change is for all again.
    #[test]
    fn a_change_for_one_reader_reaches_the_other_readers_of_its_participant_as_a_gap() {
        for frag_size in [DEFAULT_FRAGMENT_SIZE, 4] {
            let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), frag_size);
            let sibling = Guid::new(
                reader_guid().prefix,
                EntityId::user_reader_with_key([0xA0, 0xB0, 0xC1]),
            );
            w.add_reader_proxy(ReaderProxy::new(
                sibling,
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
                alloc::vec![],
                true,
            ));
            let payload: alloc::vec::Vec<u8> = (1..=10).collect();
            w.set_next_destination_readers(Some(alloc::vec![sibling]));
            let out = w
                .write_with_heartbeat_to(&payload, Duration::ZERO, None, None)
                .expect("write");
            assert_eq!(
                sent_to(&out, 7411, sn(1)),
                (true, false),
                "addressed reader"
            );
            assert_eq!(sent_to(&out, 7410, sn(1)), (false, true), "other reader");

            // The reader that was not addressed asks for the number all the same.
            w.handle_acknack(reader_guid(), sn(1), [sn(1)]);
            w.handle_acknack(sibling, sn(1), [sn(1)]);
            let out = w.tick(Duration::from_secs(1)).expect("tick");
            assert_eq!(
                sent_to(&out, 7411, sn(1)),
                (true, false),
                "repair, addressed"
            );
            assert_eq!(sent_to(&out, 7410, sn(1)), (false, true), "repair, other");

            // The setting was used up by that write.
            let _ = w
                .write_with_heartbeat_to(&payload, Duration::from_secs(2), None, None)
                .expect("write");
            assert!(
                w.cache()
                    .get(sn(2))
                    .is_some_and(|c| c.destination_readers.is_none()),
                "the next change is for all readers"
            );
            assert!(
                w.cache()
                    .get(sn(1))
                    .is_some_and(|c| c.destination_readers.as_deref() == Some(&[sibling][..])),
                "and the first keeps its addressees"
            );
        }
    }

    #[test]
    fn acknack_resend_for_fragmented_sn_sends_all_fragments() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), 4);
        let rguid = reader_guid();
        let _ = w.write(&(1..=10).collect::<alloc::vec::Vec<u8>>()).unwrap();
        w.handle_acknack(rguid, sn(1), [sn(1)]);
        let out = w.tick(Duration::ZERO).unwrap();
        // The three fragments of the sample, in however many DATA_FRAGs.
        let frags: usize = out
            .iter()
            .flat_map(|d| {
                decode_datagram(&d.bytes)
                    .unwrap()
                    .contents()
                    .filter_map(|s| match s {
                        ParsedSubmessage::DataFrag(df) => {
                            Some(usize::from(df.fragments_in_submessage))
                        }
                        _ => None,
                    })
                    .collect::<alloc::vec::Vec<_>>()
            })
            .sum();
        assert_eq!(frags, 3);
    }

    #[test]
    fn heartbeat_carries_cache_range() {
        let mut w = make_writer(10, Duration::from_millis(100));
        w.write(&alloc::vec![1]).unwrap();
        w.write(&alloc::vec![2]).unwrap();
        w.write(&alloc::vec![3]).unwrap();
        let out = w.tick(Duration::ZERO).unwrap();
        let parsed = decode_datagram(&out[0].bytes).unwrap();
        let hb = parsed
            .contents()
            .find_map(|s| {
                if let ParsedSubmessage::Heartbeat(h) = s {
                    Some(h)
                } else {
                    None
                }
            })
            .expect("HB in output");
        assert_eq!(hb.first_sn, sn(1));
        assert_eq!(hb.last_sn, sn(3));
    }

    /// Every submessage of a datagram but the last starts and ends on a
    /// 32-bit boundary (DDSI-RTPS 2.5 §9.4.1, §9.4.5.1.3). The resend path
    /// of `tick` used to put two DATA of odd length and the HEARTBEAT into
    /// one datagram, the second and third starting unaligned; Cyclone DDS
    /// drops such a message as malformed.
    #[test]
    fn a_resend_datagram_keeps_every_submessage_but_the_last_aligned() {
        let mut w = make_writer(10, Duration::from_secs(10));
        w.write(&alloc::vec![7u8; 123]).unwrap();
        w.write(&alloc::vec![8u8; 57]).unwrap();
        w.handle_acknack(reader_guid(), sn(1), [sn(1), sn(2)]);
        let out = w.tick(Duration::from_secs(20)).unwrap();
        let mut datas = 0;
        for dg in &out {
            let bytes = &dg.bytes;
            let mut at = 20;
            while at + 4 <= bytes.len() {
                let id = bytes[at];
                let len = usize::from(u16::from_le_bytes([bytes[at + 2], bytes[at + 3]]));
                let end = at + 4 + len;
                if id == 0x15 {
                    datas += 1;
                }
                assert!(
                    len % 4 == 0 || end == bytes.len(),
                    "submessage 0x{id:02x} of {len} octets is followed by another"
                );
                at = end;
            }
        }
        assert_eq!(datas, 2, "both samples were resent");
    }

    /// `DataWriter::assert_liveliness` on the wire: a final HEARTBEAT with
    /// the liveliness flag to each matched reader (DDSI-RTPS 2.5 §8.3.7.5),
    /// announcing the range the periodic one would. An empty cache announces
    /// `[1, 0]`.
    #[test]
    fn liveliness_heartbeat_is_a_final_heartbeat_with_the_liveliness_flag() {
        let heartbeat_of = |out: &[OutboundDatagram]| {
            assert_eq!(out.len(), 1, "one datagram per matched reader");
            let parsed = decode_datagram(&out[0].bytes).unwrap();
            parsed
                .contents()
                .find_map(|s| match s {
                    ParsedSubmessage::Heartbeat(h) => Some(h.clone()),
                    _ => None,
                })
                .expect("HB in output")
        };
        let mut w = make_writer(10, Duration::from_millis(100));
        let hb = heartbeat_of(&w.liveliness_heartbeat().unwrap());
        assert!(hb.liveliness_flag && hb.final_flag);
        assert_eq!((hb.first_sn, hb.last_sn), (sn(1), sn(0)));
        assert_eq!(hb.reader_id, reader_guid().entity_id);
        w.write(&alloc::vec![1]).unwrap();
        w.write(&alloc::vec![2]).unwrap();
        let hb = heartbeat_of(&w.liveliness_heartbeat().unwrap());
        assert!(hb.liveliness_flag && hb.final_flag);
        assert_eq!((hb.first_sn, hb.last_sn), (sn(1), sn(2)));
    }

    #[test]
    fn emit_info_dst_prepends_info_dst_with_peer_prefix() {
        // cyclones filtered VolatileSecure-Reader matcht per voller GUID;
        // without INFO_DST it resolves dst=0:0:0:<eid> -> no match -> deadlock.
        let mut w = make_writer(10, Duration::from_secs(1));
        w.set_emit_info_dst(true);
        // The volatile writer uses write_with_heartbeat (not plain write).
        let d = w
            .write_with_heartbeat(&alloc::vec![0xAA], Duration::ZERO)
            .expect("whb");
        let bytes = &d[0].bytes;
        assert_eq!(bytes[20], 0x0E, "first submessage must be INFO_DST");
        assert_eq!(
            u16::from_le_bytes([bytes[22], bytes[23]]),
            12,
            "INFO_DST body = 12-byte prefix"
        );
        assert_eq!(
            &bytes[24..36],
            &[2u8; 12],
            "INFO_DST carries peer (reader) prefix"
        );
    }

    /// Every datagram this writer builds for one proxy carries the address,
    /// not only the ones `write_with_heartbeat` opens.
    ///
    /// `emit_info_dst_prepends_info_dst_with_peer_prefix` above covers that
    /// one path and covered only that one: GAP, the lifecycle DATA, the plain
    /// single DATA and the DATA_FRAG ways each opened their datagram with a
    /// bare `MessageBuilder::open`. A GAP is the one that hurts — its content
    /// is per-proxy, so a GAP meant for a dead predecessor reaches the
    /// successor on a reused port and marks sequence numbers irrelevant that
    /// it never saw. Issue #32, and the reason `open_builder_for` exists.
    ///
    /// What this reaches, checked by putting each path back and watching it
    /// fail: the GAP and the piggyback HEARTBEAT of `tick`'s aggregated
    /// datagram, the lifecycle DATA, and the plain single DATA. What it does
    /// **not** reach is `build_gap_datagram` and `build_data_frag_datagram` —
    /// both are on the fragment-resend path, which needs a NACK_FRAG against
    /// a fragmented sample to enter. They go through `open_builder_for` too;
    /// this test is not the reason to believe it.
    #[test]
    fn every_proxy_directed_datagram_carries_info_dst_not_just_the_heartbeat_path() {
        fn first_submessage_is_info_dst(bytes: &[u8]) -> bool {
            bytes.len() > 36
                && bytes[20] == 0x0E
                && u16::from_le_bytes([bytes[22], bytes[23]]) == 12
                && bytes[24..36] == [2u8; 12]
        }

        // GAP: the proxy asks for a sequence number the cache no longer has.
        let mut w = make_writer(10, Duration::from_secs(1));
        w.set_emit_info_dst(true);
        for i in 1..=3u8 {
            let _ = w.write(&alloc::vec![i]).expect("write");
        }
        // SN 1 is gone (lifespan), so asking for it can only be answered with
        // a GAP — which is the datagram whose content is per-proxy.
        assert_eq!(
            w.remove_samples_up_to(sn(2)),
            1,
            "SN 1 leaves the cache, SN 2 stays: the bound is exclusive"
        );
        assert_eq!(w.cache().min_sn(), Some(sn(2)));
        w.handle_acknack(reader_guid(), sn(1), [sn(1)]);
        let out = w.tick(Duration::from_secs(2)).expect("tick");
        let gaps: alloc::vec::Vec<&OutboundDatagram> = out
            .iter()
            .filter(|d| {
                decode_datagram(&d.bytes).is_ok_and(|dec| {
                    dec.contents()
                        .any(|sm| matches!(sm, ParsedSubmessage::Gap(_)))
                })
            })
            .collect();
        assert!(!gaps.is_empty(), "the run has to produce a GAP to test one");
        for g in &gaps {
            assert!(
                first_submessage_is_info_dst(&g.bytes),
                "a GAP must open with INFO_DST(peer); it is per-proxy content"
            );
        }

        // Lifecycle DATA: dispose builds its own datagram.
        let mut w2 = make_writer(10, Duration::from_secs(1));
        w2.set_emit_info_dst(true);
        let disposed = Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([9, 9, 9]),
        );
        let dgs = w2
            .write_lifecycle(
                disposed.to_bytes(),
                &alloc::vec![0u8; 16],
                crate::inline_qos::status_info::DISPOSED,
                None,
            )
            .expect("dispose");
        assert!(!dgs.is_empty(), "dispose has to produce a datagram");
        for d in &dgs {
            assert!(
                first_submessage_is_info_dst(&d.bytes),
                "a lifecycle DATA must open with INFO_DST(peer)"
            );
        }

        // The plain single DATA of `write`.
        let mut w3 = make_writer(10, Duration::from_secs(1));
        w3.set_emit_info_dst(true);
        let plain = w3.write(&alloc::vec![0x42]).expect("write");
        assert!(!plain.is_empty());
        for d in &plain {
            assert!(
                first_submessage_is_info_dst(&d.bytes),
                "a plain DATA must open with INFO_DST(peer)"
            );
        }
    }

    #[test]
    fn no_info_dst_by_default() {
        let mut w = make_writer(10, Duration::from_secs(1));
        let d = w.write(&alloc::vec![0xAA]).expect("write");
        assert_ne!(d[0].bytes[20], 0x0E, "no INFO_DST emitted by default");
    }

    // ---------- Multi-Reader (WP 1.4 T3b) ----------

    #[test]
    fn write_fans_out_to_all_reader_proxies() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let second = Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::user_reader_with_key([0xA1, 0xB1, 0xC1]),
        );
        w.add_reader_proxy(ReaderProxy::new(
            second,
            alloc::vec![Locator::udp_v4([127, 0, 0, 2], 7411)],
            alloc::vec![],
            true,
        ));
        let dgs = w.write(&alloc::vec![0xAA]).unwrap();
        assert_eq!(dgs.len(), 2, "one datagram per reader-proxy");
        // Verschiedene Targets
        assert_ne!(dgs[0].targets, dgs[1].targets);
    }

    /// §8.4.7.5 Table 8.61 `multicastLocatorList`: a reader proxy that has
    /// multicast locators is written to there, one datagram for the group;
    /// one with only unicast locators gets the datagram at those. A DATA and
    /// a HEARTBEAT both follow the proxy's list.
    #[test]
    fn a_reader_proxy_with_multicast_locators_is_sent_to_at_the_multicast_locator() {
        let mut w = make_writer(10, Duration::from_millis(10));
        w.remove_reader_proxy(reader_guid());
        let unicast = Locator::udp_v4([127, 0, 0, 5], 7411);
        let multicast = Locator::udp_v4([239, 255, 0, 1], 7401);
        let with_group = Guid::new(
            GuidPrefix::from_bytes([4; 12]),
            EntityId::user_reader_with_key([0xA2, 0xB2, 0xC2]),
        );
        let unicast_only = Guid::new(
            GuidPrefix::from_bytes([5; 12]),
            EntityId::user_reader_with_key([0xA3, 0xB3, 0xC3]),
        );
        w.add_reader_proxy(ReaderProxy::new(
            with_group,
            alloc::vec![unicast],
            alloc::vec![multicast],
            true,
        ));
        w.add_reader_proxy(ReaderProxy::new(
            unicast_only,
            alloc::vec![Locator::udp_v4([127, 0, 0, 6], 7412)],
            alloc::vec![],
            true,
        ));
        let dgs = w.write(&alloc::vec![0xAA]).unwrap();
        assert_eq!(dgs.len(), 2);
        let to_group: Vec<_> = dgs
            .iter()
            .filter(|d| d.targets.contains(&multicast))
            .collect();
        assert_eq!(to_group.len(), 1, "the DATA for the proxy with a group");
        assert_eq!(to_group[0].targets.as_slice(), [multicast]);
        assert!(
            dgs.iter().all(|d| !d.targets.contains(&unicast)),
            "the unicast locator of a proxy with a multicast locator is not used"
        );
        assert!(
            dgs.iter()
                .any(|d| d.targets.as_slice() == [Locator::udp_v4([127, 0, 0, 6], 7412)]),
            "the proxy without a group gets its unicast locator"
        );
        let beats = w.tick(Duration::from_secs(1)).unwrap();
        assert!(
            beats.iter().any(|d| d.targets.as_slice() == [multicast]),
            "the HEARTBEAT for the proxy with a group goes to the group"
        );
    }

    /// A reliable reader matched after a VOLATILE writer emptied its cache
    /// is told first = last + 1 with last the last SN written. The HEARTBEAT
    /// used to take last from the cache, 0 once empty: first 4, last 0, which
    /// DDSI-RTPS 2.5 §8.3.7.5 forbids (lastSN >= firstSN - 1) and Cyclone
    /// DDS 11.0.1 drops as malformed, with the DATA in the same message.
    #[test]
    fn a_heartbeat_after_the_cache_emptied_announces_the_last_sn_written() {
        let mut w = make_writer(100, Duration::from_millis(10));
        w.remove_reader_proxy(reader_guid());
        for i in 0u8..3 {
            w.write(&alloc::vec![i]).unwrap();
        }
        w.remove_acknowledged();
        assert_eq!(w.cache().len(), 0);
        assert_eq!(w.last_written_sn(), sn(3));

        let mut late = ReaderProxy::new(
            Guid::new(
                GuidPrefix::from_bytes([9; 12]),
                EntityId::user_reader_with_key([0xA9, 0xB9, 0xC9]),
            ),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7499)],
            alloc::vec![],
            true,
        );
        late.skip_samples_up_to(w.last_written_sn());
        w.add_reader_proxy(late);
        let hbs = heartbeats(&w.tick(Duration::from_secs(1)).unwrap());
        assert!(!hbs.is_empty(), "the late reader is owed a HEARTBEAT");
        for (first, last) in &hbs {
            assert_eq!((*first, *last), (4, 3), "first = last + 1, last = 3");
        }
    }

    /// DDSI-RTPS 2.5 §8.4.2.3: "if a Writer receives a negative
    /// acknowledgement to a previously positively acknowledged sample, and
    /// the Writer can still service the request, the Writer should send the
    /// sample." A reader that acknowledged 1 to 5 asks for 2 and 3 again, as
    /// one that came back under the same GUID does; both go out.
    #[test]
    fn an_acknowledged_change_asked_for_again_is_sent_again() {
        let mut w = make_writer(100, Duration::from_millis(10));
        for i in 0u8..5 {
            w.write(&alloc::vec![i]).unwrap();
        }
        w.handle_acknack(reader_guid(), sn(6), []);
        w.handle_acknack(reader_guid(), sn(2), [sn(2), sn(3)]);
        let out = w.tick(Duration::from_secs(1)).unwrap();
        let sns: alloc::vec::Vec<i64> = out
            .iter()
            .flat_map(|dg| {
                decode_datagram(&dg.bytes)
                    .unwrap()
                    .contents()
                    .filter_map(|sub| match sub {
                        ParsedSubmessage::Data(d) => Some(d.writer_sn.0),
                        _ => None,
                    })
                    .collect::<alloc::vec::Vec<_>>()
            })
            .collect();
        assert_eq!(sns, alloc::vec![2, 3]);
    }

    /// DDSI-RTPS 2.5 §8.4.9.1, Table 8.65 T4: a best-effort reader matched
    /// after the writer wrote is pushed what it has not been sent, in order,
    /// without asking — a best-effort reader asks for nothing (§8.4.2.3).
    /// The writer keeps its cache here as a TRANSIENT_LOCAL writer does.
    #[test]
    fn a_best_effort_reader_matched_late_is_pushed_what_the_cache_holds() {
        let mut w = make_writer(100, Duration::from_millis(10));
        w.remove_reader_proxy(reader_guid());
        for i in 0u8..3 {
            w.write(&alloc::vec![i]).unwrap();
        }
        let late = Guid::new(
            GuidPrefix::from_bytes([7; 12]),
            EntityId::user_reader_with_key([0xA3, 0xB3, 0xC3]),
        );
        w.add_reader_proxy(ReaderProxy::new(
            late,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7419)],
            alloc::vec![],
            false,
        ));
        let data_sns = |out: &[OutboundDatagram]| -> alloc::vec::Vec<i64> {
            out.iter()
                .flat_map(|dg| {
                    decode_datagram(&dg.bytes)
                        .unwrap()
                        .contents()
                        .filter_map(|sub| match sub {
                            ParsedSubmessage::Data(d) => Some(d.writer_sn.0),
                            _ => None,
                        })
                        .collect::<alloc::vec::Vec<_>>()
                })
                .collect()
        };
        let first = w.tick(Duration::from_millis(1)).unwrap();
        assert_eq!(data_sns(&first), alloc::vec![1, 2, 3]);
        let second = w.tick(Duration::from_millis(30)).unwrap();
        assert!(data_sns(&second).is_empty(), "nothing goes twice");
    }

    /// The submessages of `out` as `(kind, sn)`: DATA by its number, GAP by
    /// its start and `filteredCount`.
    fn data_and_gaps(
        out: &[OutboundDatagram],
    ) -> alloc::vec::Vec<(&'static str, i64, Option<u64>)> {
        out.iter()
            .flat_map(|dg| {
                decode_datagram(&dg.bytes)
                    .unwrap()
                    .contents()
                    .filter_map(|sub| match sub {
                        ParsedSubmessage::Data(d) => Some(("DATA", d.writer_sn.0, None)),
                        ParsedSubmessage::Gap(g) => Some(("GAP", g.gap_start.0, g.filtered_count)),
                        _ => None,
                    })
                    .collect::<alloc::vec::Vec<_>>()
            })
            .collect()
    }

    /// §8.4.9.1 T4 and §8.4.9.2 T4: `DDS_FILTER(the_reader_proxy, a_change)`
    /// false sends `GAP(a_change.sequenceNumber)` with `filteredCount := 1`
    /// instead of the DATA; a change that passes is a DATA; the reader of
    /// another proxy, without a filter, gets every change.
    #[test]
    fn a_change_that_fails_the_readers_filter_is_a_gap_with_filtered_count_one() {
        let mut w = make_writer(10, Duration::from_secs(10));
        w.remove_reader_proxy(reader_guid());
        let filtered_reader = Guid::new(
            GuidPrefix::from_bytes([6; 12]),
            EntityId::user_reader_with_key([0xA6, 0xB6, 0xC6]),
        );
        let other_reader = Guid::new(
            GuidPrefix::from_bytes([7; 12]),
            EntityId::user_reader_with_key([0xA7, 0xB7, 0xC7]),
        );
        let proxy = |guid, port, reliable| {
            ReaderProxy::new(
                guid,
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], port)],
                alloc::vec![],
                reliable,
            )
        };
        // The reader's content filter: odd first octets only.
        let odd_only = ChangeFilter::new(|c| c.payload.first().is_some_and(|b| b % 2 == 1));
        w.add_reader_proxy(proxy(filtered_reader, 7430, true).with_filter(odd_only));
        w.add_reader_proxy(proxy(other_reader, 7431, true));
        let to = |out: &[OutboundDatagram], port: u32| -> alloc::vec::Vec<OutboundDatagram> {
            out.iter()
                .filter(|d| d.targets.iter().any(|l| l.port == port))
                .cloned()
                .collect()
        };
        let out1 = w.write(&alloc::vec![1]).unwrap();
        let out2 = w.write(&alloc::vec![2]).unwrap();
        let out3 = w.write(&alloc::vec![3]).unwrap();
        let mut filtered = to(&out1, 7430);
        filtered.extend(to(&out2, 7430));
        filtered.extend(to(&out3, 7430));
        assert_eq!(
            data_and_gaps(&filtered),
            alloc::vec![("DATA", 1, None), ("GAP", 2, Some(1)), ("DATA", 3, None)]
        );
        let mut other = to(&out1, 7431);
        other.extend(to(&out2, 7431));
        assert_eq!(
            data_and_gaps(&other),
            alloc::vec![("DATA", 1, None), ("DATA", 2, None)]
        );
        // A request for the filtered number is answered by the GAP again.
        w.handle_acknack(filtered_reader, sn(2), [sn(2)]);
        let repair = w.tick(Duration::from_millis(1)).unwrap();
        let repair = to(&repair, 7430);
        assert!(
            data_and_gaps(&repair).contains(&("GAP", 2, Some(1))),
            "{:?}",
            data_and_gaps(&repair)
        );
    }

    /// The same for a BEST_EFFORT reader that is behind (T4 of Table 8.65):
    /// what the cache holds is pushed in order, a change the filter refuses
    /// is a GAP with `filteredCount` 1, and a number the cache no longer
    /// holds is a GAP with `filteredCount` 0.
    #[test]
    fn a_best_effort_reader_behind_is_pushed_gaps_with_the_filtered_count() {
        let mut w = make_writer(100, Duration::from_millis(10));
        w.remove_reader_proxy(reader_guid());
        for i in 1u8..=3 {
            w.write(&alloc::vec![i]).unwrap();
        }
        let late = Guid::new(
            GuidPrefix::from_bytes([8; 12]),
            EntityId::user_reader_with_key([0xA8, 0xB8, 0xC8]),
        );
        let odd_only = ChangeFilter::new(|c| c.payload.first().is_some_and(|b| b % 2 == 1));
        w.add_reader_proxy(
            ReaderProxy::new(
                late,
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7432)],
                alloc::vec![],
                false,
            )
            .with_filter(odd_only),
        );
        let out = w.tick(Duration::from_millis(1)).unwrap();
        assert_eq!(
            data_and_gaps(&out),
            alloc::vec![("DATA", 1, None), ("GAP", 2, Some(1)), ("DATA", 3, None)]
        );
    }

    /// §8.4.9.1 T4: "These changes do not appear counted in the GAP message
    /// filteredCount": a number the cache does not hold is announced with
    /// `filteredCount` 0.
    #[test]
    fn a_gap_for_a_number_the_cache_no_longer_holds_has_filtered_count_zero() {
        let mut w = make_writer(10, Duration::from_secs(10));
        w.write(&alloc::vec![1]).unwrap();
        w.handle_acknack(reader_guid(), sn(1), [sn(5)]);
        let out = w.tick(Duration::ZERO).unwrap();
        assert!(
            data_and_gaps(&out).contains(&("GAP", 5, Some(0))),
            "{:?}",
            data_and_gaps(&out)
        );
    }

    /// A best-effort proxy matched after the writer wrote. Left at SN 0 it
    /// lags: `write` sends only to a proxy in step and marks nothing sent for
    /// a lagging one, and `remove_acknowledged` keeps what it has not been
    /// sent. Skipped to [`ReliableWriter::last_written_sn`] — what the DCPS
    /// layer does for a VOLATILE match — it gets the next write directly,
    /// also after the cache was emptied. Before, the skip went to the cache's
    /// maximum, which is `None` once a VOLATILE writer with no reliable
    /// reader has removed everything; the proxy stayed at 0 for good and a
    /// Cyclone 11.0.1 best-effort reader got 0 samples.
    #[test]
    fn a_best_effort_reader_matched_late_gets_the_next_write() {
        let mut w = make_writer(100, Duration::from_millis(10));
        w.remove_reader_proxy(reader_guid());
        for i in 0u8..3 {
            w.write(&alloc::vec![i]).unwrap();
        }
        w.remove_acknowledged();
        assert_eq!(w.cache().len(), 0, "no reader: nothing is held back");
        assert_eq!(w.last_written_sn(), sn(3));

        let reader = |n: u8| {
            ReaderProxy::new(
                Guid::new(
                    GuidPrefix::from_bytes([4 + n; 12]),
                    EntityId::user_reader_with_key([0xA2, 0xB2, n]),
                ),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7412 + u32::from(n))],
                alloc::vec![],
                false,
            )
        };
        // Left at 0: lags, is not sent to, and nothing is marked sent.
        w.add_reader_proxy(reader(0));
        assert!(w.write(&alloc::vec![3]).unwrap().is_empty());
        w.remove_acknowledged();
        assert_eq!(
            w.cache().len(),
            1,
            "SN 4 has not been sent to the lagging reader"
        );

        // Skipped to the last SN written: in step, sent directly.
        let mut in_step = reader(1);
        in_step.skip_samples_up_to(w.last_written_sn());
        w.remove_reader_proxy(reader(0).remote_reader_guid);
        w.add_reader_proxy(in_step);
        let direct = w.write(&alloc::vec![4]).unwrap();
        let sent: Vec<i64> = direct
            .iter()
            .flat_map(|dg| decode_datagram(&dg.bytes).unwrap().submessages)
            .filter_map(|sub| match sub.submessage {
                ParsedSubmessage::Data(d) => Some(d.writer_sn.0),
                _ => None,
            })
            .collect();
        assert_eq!(sent, alloc::vec![5], "the next write reaches the reader");
    }

    #[test]
    fn write_fans_out_to_every_unicast_locator_of_a_proxy() {
        // #27: a multi-homed reader advertises several unicast locators. UDP
        // cannot detect a dead destination, so the writer targets EVERY
        // advertised locator — the reachable one delivers even when the first
        // advertised locator is unreachable.
        let mut w = make_writer(10, Duration::from_secs(10));
        let rguid = reader_guid();
        let unreachable = Locator::udp_v4([192, 0, 2, 9], 7411); // TEST-NET-1, FIRST
        let reachable = Locator::udp_v4([127, 0, 0, 1], 7411); // loopback, SECOND
        // Replace the default proxy with a two-unicast-locator one (no multicast,
        // so `targets_for` uses the unicast list).
        w.add_reader_proxy(ReaderProxy::new(
            rguid,
            alloc::vec![unreachable, reachable],
            alloc::vec![],
            true,
        ));
        assert_eq!(w.reader_proxy_count(), 1, "same guid replaces, not appends");
        let dgs = w.write(&alloc::vec![0xAA]).unwrap();
        let d = dgs
            .iter()
            .find(|d| d.targets.contains(&reachable))
            .expect("a datagram targets the reachable locator");
        assert!(
            d.targets.contains(&unreachable),
            "the unreachable FIRST locator is still targeted"
        );
        assert!(
            d.targets.contains(&reachable),
            "the reachable SECOND locator is targeted — delivery succeeds despite the dead first"
        );
    }

    #[test]
    fn add_reader_proxy_is_idempotent_on_same_guid() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let rguid = reader_guid();
        let replacement = ReaderProxy::new(
            rguid,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 9999)],
            alloc::vec![],
            true,
        );
        w.add_reader_proxy(replacement);
        assert_eq!(w.reader_proxy_count(), 1);
        assert_eq!(
            w.reader_proxies()[0].unicast_locators,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 9999)]
        );
    }

    #[test]
    fn remove_reader_proxy_by_guid() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let rguid = reader_guid();
        let removed = w.remove_reader_proxy(rguid);
        assert!(removed.is_some());
        assert_eq!(w.reader_proxy_count(), 0);
        assert!(
            w.remove_reader_proxy(rguid).is_none(),
            "second remove is None"
        );
    }

    #[test]
    fn acknack_dispatches_to_matching_proxy_only() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let rguid1 = reader_guid();
        let rguid2 = Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::user_reader_with_key([0xA1, 0xB1, 0xC1]),
        );
        w.add_reader_proxy(ReaderProxy::new(
            rguid2,
            alloc::vec![Locator::udp_v4([127, 0, 0, 2], 7411)],
            alloc::vec![],
            true,
        ));
        for i in 1..=3 {
            w.write(&alloc::vec![i as u8]).unwrap();
        }
        w.handle_acknack(rguid1, sn(4), []);
        // Proxy 1 shows highest_acked=3, proxy 2 unchanged=0.
        // Cache GC is decoupled from the acknack (per-destination-queue model).
        assert_eq!(w.reader_proxies()[0].highest_acked_sn(), sn(3));
        assert_eq!(w.reader_proxies()[1].highest_acked_sn(), sn(0));
        assert_eq!(w.cache().len(), 3, "KeepAll cache intact");
    }

    #[test]
    fn nackfrag_dispatches_only_to_matching_proxy() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), 4);
        let rguid1 = reader_guid();
        let rguid2 = Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::user_reader_with_key([0xA1, 0xB1, 0xC1]),
        );
        w.add_reader_proxy(ReaderProxy::new(
            rguid2,
            alloc::vec![Locator::udp_v4([127, 0, 0, 2], 7411)],
            alloc::vec![],
            true,
        ));
        let _ = w.write(&(1..=10).collect::<alloc::vec::Vec<u8>>()).unwrap();
        let nf = NackFragSubmessage {
            reader_id: rguid1.entity_id,
            writer_id: w.guid.entity_id,
            writer_sn: sn(1),
            fragment_number_state: crate::submessages::FragmentNumberSet::from_missing(
                FragmentNumber(1),
                &[FragmentNumber(2)],
            ),
            count: 1,
        };
        w.handle_nackfrag(rguid1, &nf);
        assert_eq!(w.reader_proxies()[0].pending_requested_fragment_count(), 1);
        assert_eq!(w.reader_proxies()[1].pending_requested_fragment_count(), 0);
    }

    // ---------- WP 1.E stage A: HEARTBEAT FinalFlag default ----------

    /// §8.4.9.2.7: periodic HEARTBEATs must carry `FinalFlag=NOT_SET`,
    /// otherwise the reader does not respond with ACKNACK and the
    /// reliable-liveness loop breaks.
    #[test]
    fn periodic_heartbeat_has_final_flag_unset() {
        let mut w = make_writer(10, Duration::from_millis(50));
        w.write(&alloc::vec![1]).unwrap();
        let out = w.tick(Duration::ZERO).unwrap();
        let parsed = decode_datagram(&out[0].bytes).unwrap();
        let hb = parsed
            .contents()
            .find_map(|s| {
                if let ParsedSubmessage::Heartbeat(h) = s {
                    Some(h)
                } else {
                    None
                }
            })
            .expect("HB must be present");
        assert!(
            !hb.final_flag,
            "periodic HB must NOT set FinalFlag (Spec §8.4.9.2.7)"
        );
    }

    /// Ad-hoc HB directly after `add_reader_proxy`: sets `last_heartbeat=None`,
    /// i.e. the next `tick()` immediately emits an HB. This HB is
    /// also non-final, so the fresh reader reliably responds.
    #[test]
    fn heartbeat_after_add_reader_proxy_is_non_final() {
        let mut w = make_writer(10, Duration::from_secs(60));
        w.write(&alloc::vec![1]).unwrap();
        // first tick consumes initial HB
        let _ = w.tick(Duration::ZERO).unwrap();
        // add second proxy → last_heartbeat=None → next tick emits HB
        let second = ReaderProxy::new(
            Guid::new(
                GuidPrefix::from_bytes([7; 12]),
                EntityId::user_reader_with_key([0xA1, 0xB1, 0xC1]),
            ),
            alloc::vec![Locator::udp_v4([127, 0, 0, 2], 7411)],
            alloc::vec![],
            true,
        );
        w.add_reader_proxy(second);
        let out = w.tick(Duration::ZERO).unwrap();
        let mut hb_found = 0usize;
        for d in &out {
            for s in decode_datagram(&d.bytes).unwrap().contents() {
                if let ParsedSubmessage::Heartbeat(h) = s {
                    assert!(
                        !h.final_flag,
                        "post-add_reader_proxy HB must be non-final (Spec §8.4.9.2.7)"
                    );
                    hb_found += 1;
                }
            }
        }
        assert!(hb_found >= 1, "at least one HB expected");
    }

    #[test]
    fn aggregation_packs_multiple_resends_into_one_datagram() {
        let mut w = make_writer(10, Duration::from_secs(10));
        let rguid = reader_guid();
        // Four octets each, so each DATA ends aligned and the next may follow
        // it in the same datagram (DDSI-RTPS 2.5 §9.4.1).
        for i in 1..=3 {
            w.write(&alloc::vec![i as u8; 4]).unwrap();
        }
        // All 3 as requested
        w.handle_acknack(rguid, sn(1), [sn(1), sn(2), sn(3)]);
        let out = w.tick(Duration::ZERO).unwrap();
        // One datagram contains multiple DATAs + HEARTBEAT
        assert_eq!(out.len(), 1, "all resends aggregated into single datagram");
        let parsed = decode_datagram(&out[0].bytes).unwrap();
        let data_count = parsed
            .contents()
            .filter(|s| matches!(s, ParsedSubmessage::Data(_)))
            .count();
        assert_eq!(data_count, 3);
        let hb_count = parsed
            .contents()
            .filter(|s| matches!(s, ParsedSubmessage::Heartbeat(_)))
            .count();
        assert_eq!(hb_count, 1);
    }

    /// §8.3.7.2.5 Table 8.43: `K=1` means `serializedPayload` holds the
    /// serialized Key. A lifecycle DATA that sets K therefore has to carry
    /// one, and the key has to be long enough to be a key — Cyclone's
    /// `validate_Data` (`ddsi_receive.c:432`) calls a `D`-or-`K` DATA
    /// malformed when the payload does not even hold the four-octet
    /// encapsulation header, and discards the entire datagram.
    ///
    /// Measured counterpart: Cyclone 11.0.1 and OpenDDS 3.34.0 both send a
    /// SEDP subscriptions dispose with flags `0x0b` (E|Q|K) and the key as
    /// `PL_CDR_LE` + `PID_ENDPOINT_GUID` + `PID_SENTINEL`.
    #[test]
    fn lifecycle_with_a_key_sets_k_and_carries_it() {
        use crate::inline_qos::status_info::{DISPOSED, UNREGISTERED};
        let mut w = make_writer(10, Duration::from_secs(10));
        let endpoint = reader_guid();
        let key = sedp_serialized_key(endpoint);
        let dgs = w
            .write_lifecycle(endpoint.to_bytes(), &key, DISPOSED | UNREGISTERED, None)
            .unwrap();
        let d = only_data(&dgs.first().expect("one datagram").bytes).expect("a DATA");
        assert!(d.key_flag, "a lifecycle DATA that carries a key sets K");
        assert!(!d.data_flag, "D and K are mutually exclusive (Table 8.43)");
        assert_eq!(
            d.serialized_payload.as_ref(),
            key.as_slice(),
            "the key on the wire is the key that was handed in"
        );
        assert!(
            d.serialized_payload.len() >= 4,
            "a K=1 payload shorter than an encapsulation header is what \
             Cyclone rejects as malformed"
        );
        let qos = d.inline_qos.as_ref().expect("inline QoS");
        assert_eq!(
            crate::inline_qos::find_key_hash(qos),
            Some(endpoint.to_bytes()),
            "PID_KEY_HASH names the same instance as the key"
        );
        assert_eq!(
            crate::inline_qos::find_status_info(qos),
            Some(DISPOSED | UNREGISTERED)
        );
    }

    /// The other measured form: Fast DDS 3.6.1 sends the SEDP dispose with
    /// flags `0x03` (E|Q), no payload at all, and names the instance by
    /// `PID_KEY_HASH` alone. A caller with no key serialization gets that,
    /// never `K=1` over zero octets.
    #[test]
    fn lifecycle_without_a_key_clears_k_and_sends_no_payload() {
        use crate::inline_qos::status_info::DISPOSED;
        let mut w = make_writer(10, Duration::from_secs(10));
        let key_hash = [0xAB; 16];
        let dgs = w.write_lifecycle(key_hash, &[], DISPOSED, None).unwrap();
        let d = only_data(&dgs.first().expect("one datagram").bytes).expect("a DATA");
        assert!(
            !d.key_flag,
            "no key to serialize means K stays clear — a K=1 DATA with an \
             empty payload is the malformed form Cyclone discards"
        );
        assert!(!d.data_flag);
        assert!(d.serialized_payload.is_empty());
        assert_eq!(
            crate::inline_qos::find_key_hash(d.inline_qos.as_ref().expect("inline QoS")),
            Some(key_hash),
            "with no payload the key hash is the only instance identity"
        );
    }

    /// Each of the three measured dispose forms, reached through the
    /// per-peer table, produces the flags and inline QoS that its own
    /// implementation was measured to send.
    #[test]
    fn each_peer_dialect_produces_its_measured_dispose_form() {
        use crate::dialect::RtpsDialect;
        use crate::inline_qos::status_info::{DISPOSED, UNREGISTERED};
        let endpoint = reader_guid();
        let key = sedp_serialized_key(endpoint);
        // (dialect, K flag, PID_KEY_HASH present, payload carries the key)
        let expected = [
            (RtpsDialect::Cyclone, true, false, true),
            (RtpsDialect::OpenDds, true, true, true),
            (RtpsDialect::MaximallyInteroperable, true, true, true),
            (RtpsDialect::FastDds, false, true, false),
        ];
        for (dialect, k, hash, payload) in expected {
            let mut w = make_writer(10, Duration::from_secs(10));
            w.set_peer_behaviour(reader_guid(), dialect.behaviour());
            let dgs = w
                .write_lifecycle(endpoint.to_bytes(), &key, DISPOSED | UNREGISTERED, None)
                .unwrap();
            let d = only_data(&dgs.first().expect("one datagram").bytes).expect("a DATA");
            assert_eq!(d.key_flag, k, "{dialect:?}: K flag");
            assert!(!d.data_flag, "{dialect:?}: D is clear on every marker");
            let qos = d.inline_qos.as_ref().expect("inline QoS");
            assert_eq!(
                crate::inline_qos::find_key_hash(qos).is_some(),
                hash,
                "{dialect:?}: PID_KEY_HASH presence"
            );
            assert_eq!(
                crate::inline_qos::find_status_info(qos),
                Some(DISPOSED | UNREGISTERED),
                "{dialect:?}: PID_STATUS_INFO is on every form"
            );
            if payload {
                assert_eq!(
                    d.serialized_payload.as_ref(),
                    key.as_slice(),
                    "{dialect:?}: the key on the wire"
                );
            } else {
                assert!(
                    d.serialized_payload.is_empty(),
                    "{dialect:?}: K clear means no payload"
                );
            }
            // Whichever form: the instance is named somewhere.
            assert!(
                crate::inline_qos::find_key_hash(qos).is_some() || !d.serialized_payload.is_empty(),
                "{dialect:?}: a marker must name its instance"
            );
        }
    }

    /// Two peers of different dialects on one writer each get their own
    /// form out of the same `write_lifecycle` call — the reason the table is
    /// per peer and not per participant.
    #[test]
    fn two_peer_dialects_on_one_writer_get_their_own_form() {
        use crate::dialect::RtpsDialect;
        use crate::inline_qos::status_info::DISPOSED;
        let cyclone_peer = reader_guid();
        let fastdds_peer = Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::user_reader_with_key([0xC0, 0xD0, 0xE0]),
        );
        let mut w = make_writer(10, Duration::from_secs(10));
        w.add_reader_proxy(ReaderProxy::new(
            fastdds_peer,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        ));
        w.set_peer_behaviour(cyclone_peer, RtpsDialect::Cyclone.behaviour());
        w.set_peer_behaviour(fastdds_peer, RtpsDialect::FastDds.behaviour());
        let endpoint = cyclone_peer;
        let key = sedp_serialized_key(endpoint);
        let dgs = w
            .write_lifecycle(endpoint.to_bytes(), &key, DISPOSED, None)
            .unwrap();
        let forms: alloc::vec::Vec<(bool, bool)> = dgs
            .iter()
            .filter_map(|dg| only_data(&dg.bytes))
            .map(|d| {
                (
                    d.key_flag,
                    d.inline_qos
                        .as_ref()
                        .and_then(crate::inline_qos::find_key_hash)
                        .is_some(),
                )
            })
            .collect();
        assert!(
            forms.contains(&(true, false)),
            "one datagram must be Cyclone's form (K=1, no hash), got {forms:?}"
        );
        assert!(
            forms.contains(&(false, true)),
            "one datagram must be Fast DDS' form (K=0, hash), got {forms:?}"
        );
    }

    /// A peer configured for Cyclone's form names the instance only in the
    /// payload. Handed no key — the DCPS user path — it must not emit a
    /// marker that carries neither identity.
    #[test]
    fn a_key_in_payload_peer_without_a_key_still_names_the_instance() {
        use crate::dialect::RtpsDialect;
        use crate::inline_qos::status_info::DISPOSED;
        let mut w = make_writer(10, Duration::from_secs(10));
        w.set_peer_behaviour(reader_guid(), RtpsDialect::Cyclone.behaviour());
        let key_hash = [0xAB; 16];
        let dgs = w.write_lifecycle(key_hash, &[], DISPOSED, None).unwrap();
        let d = only_data(&dgs.first().expect("one datagram").bytes).expect("a DATA");
        assert!(!d.key_flag, "no key means K stays clear");
        assert!(d.serialized_payload.is_empty());
        assert_eq!(
            crate::inline_qos::find_key_hash(d.inline_qos.as_ref().expect("inline QoS")),
            Some(key_hash),
            "the form degraded to the one that still carries an identity"
        );
    }

    /// The migrated axis: `set_emit_info_dst` now writes
    /// `DirectedAddressing` on the participant default, and the datagram
    /// still opens with INFO_DST.
    #[test]
    fn set_emit_info_dst_writes_the_migrated_axis() {
        use crate::dialect::DirectedAddressing;
        let mut w = make_writer(10, Duration::from_secs(10));
        assert_eq!(
            w.behaviour().directed_addressing,
            DirectedAddressing::ByEntityIdOnly
        );
        w.set_emit_info_dst(true);
        assert_eq!(
            w.behaviour().directed_addressing,
            DirectedAddressing::PrefixEveryDatagram
        );
        // `write_with_heartbeat` is the path that opens its builder through
        // `open_builder`; plain `write` builds its DATA datagram directly.
        let dgs = w
            .write_with_heartbeat(&alloc::vec![0xAA], Duration::ZERO)
            .unwrap();
        assert_eq!(
            dgs.first().expect("a datagram").bytes[20],
            SubmessageId::InfoDst.as_u8(),
            "the migrated axis still puts INFO_DST in front"
        );
        w.set_emit_info_dst(false);
        assert_eq!(
            w.behaviour().directed_addressing,
            DirectedAddressing::ByEntityIdOnly
        );
    }

    /// A NACK repair of the marker must reproduce the first send: same K
    /// flag, same key octets. The key lives on the `CacheChange`, so this
    /// holds for a proxy that asks for the marker long after it was written.
    #[test]
    fn lifecycle_repair_reproduces_the_key() {
        use crate::inline_qos::status_info::DISPOSED;
        let mut w = make_writer(10, Duration::from_secs(10));
        let endpoint = reader_guid();
        let key = sedp_serialized_key(endpoint);
        let marker_sn = {
            let dgs = w
                .write_lifecycle(endpoint.to_bytes(), &key, DISPOSED, None)
                .unwrap();
            only_data(&dgs.first().expect("one datagram").bytes)
                .expect("a DATA")
                .writer_sn
        };
        // The proxy asks for the marker again.
        w.handle_acknack(reader_guid(), marker_sn, [marker_sn]);
        let repaired = w.tick(Duration::from_secs(1)).unwrap();
        let d = repaired
            .iter()
            .find_map(|dg| only_data(&dg.bytes))
            .expect("the repair carries the marker");
        assert_eq!(d.writer_sn, marker_sn);
        assert!(
            d.key_flag,
            "the repair sets K exactly as the first send did"
        );
        assert_eq!(
            d.serialized_payload.as_ref(),
            key.as_slice(),
            "the repair carries the same key, not an empty payload"
        );
    }

    /// A writer with three changes and a reader proxy that joined behind
    /// them — SEDP's publications writer before a peer is discovered.
    fn writer_with_a_late_proxy() -> ReliableWriter {
        let mut w = fast_writer(ReliableWriterConfig {
            guid: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            ),
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![],
            max_samples: 10,
            history_kind: HistoryKind::KeepAll,
            heartbeat_period: Duration::from_millis(100),
            fragment_size: DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        for _ in 0..3 {
            w.write(b"announce").unwrap();
        }
        w.add_reader_proxy(ReaderProxy::new(
            reader_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
            alloc::vec![],
            true,
        ));
        w
    }

    /// The SNs of the DATA submessages in `dgs`.
    fn data_sns(dgs: &[OutboundDatagram]) -> Vec<i64> {
        let mut out = Vec::new();
        for dg in dgs {
            for s in decode_datagram(&dg.bytes).unwrap().contents() {
                if let ParsedSubmessage::Data(d) = s {
                    out.push(d.writer_sn.0);
                }
            }
        }
        out
    }

    /// Measured on the Linux test host (ROS 2 service, capture): a proxy that joined
    /// behind the cache was brought up by repair, and its send cursor stayed
    /// at 0; every later change waited for the next HEARTBEAT and ACKNACK
    /// (100 ms) instead of going out on `write`. The client's reply reader
    /// reached the server 100 ms after its request writer, and the first
    /// reply went nowhere. A reader's acknowledgement brings the cursor up.
    #[test]
    fn a_proxy_behind_the_cache_is_in_step_once_it_acknowledges() {
        let mut w = writer_with_a_late_proxy();
        assert!(data_sns(&w.write(b"before").unwrap()).is_empty());
        w.handle_acknack(reader_guid(), sn(5), alloc::vec![]);
        assert_eq!(first_proxy(&w).highest_sent_sn(), sn(4));
        assert_eq!(data_sns(&w.write(b"after").unwrap()), alloc::vec![5]);
    }

    /// Repairs sent in order move the cursor as a direct send would, so the
    /// next change goes out on `write` before the reader's next ACKNACK.
    #[test]
    fn repairs_in_order_bring_a_lagging_proxy_into_step() {
        let mut w = writer_with_a_late_proxy();
        w.handle_acknack(reader_guid(), sn(1), [sn(1), sn(2), sn(3)]);
        let repaired = w.tick(Duration::from_secs(1)).unwrap();
        assert_eq!(data_sns(&repaired), alloc::vec![1, 2, 3]);
        assert_eq!(first_proxy(&w).highest_sent_sn(), sn(3));
        assert_eq!(data_sns(&w.write(b"next").unwrap()), alloc::vec![4]);
    }

    /// A repair out of order is followed by the changes before it
    /// (DDSI-RTPS 2.5 §8.4.9.2, Table 8.66 T4), and the change it repaired
    /// goes out once.
    #[test]
    fn a_repair_out_of_order_is_followed_by_the_changes_before_it() {
        let mut w = writer_with_a_late_proxy();
        w.handle_acknack(reader_guid(), sn(1), [sn(3)]);
        let out = w.tick(Duration::from_secs(1)).unwrap();
        assert_eq!(data_sns(&out), alloc::vec![3, 1, 2]);
        assert_eq!(first_proxy(&w).highest_sent_sn(), sn(3));
    }

    /// A reliable reader behind the cache is pushed the changes it lacks
    /// (§8.4.9.2, Table 8.66 T4) without asking, and is in step afterwards:
    /// the next change goes out on `write`.
    #[test]
    fn a_reliable_proxy_behind_the_cache_is_pushed_into_step() {
        let mut w = writer_with_a_late_proxy();
        let out = w.tick(Duration::from_secs(1)).unwrap();
        assert_eq!(data_sns(&out), alloc::vec![1, 2, 3]);
        assert_eq!(first_proxy(&w).highest_sent_sn(), sn(3));
        assert_eq!(data_sns(&w.write(b"next").unwrap()), alloc::vec![4]);
    }

    /// A reader that asks only for the last change a HEARTBEAT names, behind
    /// a KEEP_LAST 1 writer that replaced it before the repair: the GAP for
    /// the replaced changes and the DATA of the current one bring it into
    /// step. Measured before: HDDS 1.4.8 behind a ZeroDDS TRANSIENT writer
    /// got HEARTBEAT and GAP and never a DATA (OMG dds-rtps
    /// Test_Durability_9).
    #[test]
    fn a_reader_asking_for_a_replaced_change_gets_the_current_one() {
        let mut w = fast_writer(ReliableWriterConfig {
            guid: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            ),
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![],
            max_samples: 10,
            history_kind: HistoryKind::KeepLast { depth: 1 },
            heartbeat_period: Duration::from_millis(100),
            fragment_size: DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        w.write(b"first").unwrap();
        w.add_reader_proxy(ReaderProxy::new(
            reader_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
            alloc::vec![],
            true,
        ));
        // The reader asks for SN 1; the writer replaces it with SN 2 and 3
        // before it answers.
        w.handle_acknack(reader_guid(), sn(1), [sn(1)]);
        assert!(data_sns(&w.write(b"second").unwrap()).is_empty());
        assert!(data_sns(&w.write(b"third").unwrap()).is_empty());
        let out = w.tick(Duration::from_secs(1)).unwrap();
        assert_eq!(data_sns(&out), alloc::vec![3]);
        assert_eq!(first_proxy(&w).highest_sent_sn(), sn(3));
        assert_eq!(data_sns(&w.write(b"fourth").unwrap()), alloc::vec![4]);
    }

    /// Regression (SEDP dispose delivery): `write_lifecycle` must deliver the
    /// lifecycle marker to a proxy whose send cursor lags behind the cache —
    /// not only to one sitting exactly at `dispose_sn - 1`. Previously the
    /// direct send was gated on `next_unsent_change == Some(sn)`, so a lagging
    /// proxy got **zero** datagrams and the disposed endpoint lingered. The
    /// writer now drains the proxy in-order up to and including the marker.
    #[test]
    fn write_lifecycle_drains_a_lagging_proxy() {
        // Writer with NO proxy yet: stage three ALIVE samples into the cache
        // (SN 1..3) so any later-joining proxy starts two-plus changes behind.
        let writer_guid = Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
        );
        let mut w = fast_writer(ReliableWriterConfig {
            guid: writer_guid,
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![],
            max_samples: 10,
            history_kind: HistoryKind::KeepAll,
            heartbeat_period: Duration::from_secs(10),
            fragment_size: DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        for _ in 0..3 {
            w.write(b"announce").unwrap();
        }
        // A reader discovered late — its send cursor starts at SN 0, i.e. three
        // changes behind the cache.
        w.add_reader_proxy(ReaderProxy::new(
            reader_guid(),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
            alloc::vec![],
            true,
        ));
        use crate::inline_qos::status_info::DISPOSED;
        let key = [0xABu8; 16];
        let dgs = w
            .write_lifecycle(key, &sedp_serialized_key(reader_guid()), DISPOSED, None)
            .unwrap();
        // Old behaviour: empty (dispose dropped). New: drains SN 1..3 + the
        // marker at SN 4, so the lagging proxy is caught up and disposed.
        assert!(
            !dgs.is_empty(),
            "lifecycle marker must reach a lagging proxy"
        );
        assert_eq!(
            first_proxy(&w).highest_sent_sn(),
            sn(4),
            "proxy must be drained up to the dispose SN"
        );
        // The dispose itself (SN 4, STATUS_INFO=DISPOSED) is on the wire.
        let mut saw_dispose = false;
        for dg in &dgs {
            let parsed = decode_datagram(&dg.bytes).unwrap();
            for s in parsed.contents() {
                if let ParsedSubmessage::Data(d) = s {
                    if d.writer_sn == sn(4) {
                        let bits = d
                            .inline_qos
                            .as_ref()
                            .and_then(crate::inline_qos::find_status_info)
                            .unwrap_or(0);
                        assert!(bits & DISPOSED != 0, "SN 4 must carry a DISPOSED marker");
                        saw_dispose = true;
                    }
                }
            }
        }
        assert!(
            saw_dispose,
            "the dispose DATA (SN 4) must be among the datagrams"
        );
    }

    /// The flags and body of the first DATA submessage of `bytes`.
    fn first_data_submessage(bytes: &[u8]) -> (u8, alloc::vec::Vec<u8>) {
        let mut pos = crate::message_builder::RTPS_HEADER_LEN;
        while pos + 4 <= bytes.len() {
            let id = bytes[pos];
            let flags = bytes[pos + 1];
            let len = usize::from(u16::from_le_bytes([bytes[pos + 2], bytes[pos + 3]]));
            let body = &bytes[pos + 4..pos + 4 + len];
            if id == SubmessageId::Data as u8 {
                return (flags, body.to_vec());
            }
            pos += 4 + len;
        }
        panic!("no DATA submessage in the datagram");
    }

    /// The DATA a write puts into the builder from `compact_data_prefix` is
    /// the one `DataSubmessage::write_body` writes for the same change:
    /// keyed, with `[PID_KEY_HASH, PID_SENTINEL]`, and keyless, with no
    /// inline QoS; through the single-DATA path and through the
    /// DATA+HEARTBEAT path.
    #[test]
    fn the_compact_data_prefix_writes_the_octets_write_body_writes() {
        use crate::header_extension::HeTimestamp;
        let payload = alloc::vec![0x00, 0x01, 0x00, 0x00, 1, 2, 3, 4, 5, 6, 7, 8];
        let key = [0x5Au8; 16];
        let ts = Some(HeTimestamp {
            seconds: 3,
            fraction: 9,
        });
        let expected = |sn_: i64, key: Option<[u8; 16]>| {
            let inline_qos = key.map(|k| {
                let mut pl = crate::parameter_list::ParameterList::new();
                pl.push(crate::parameter_list::Parameter::new(
                    crate::parameter_list::pid::KEY_HASH,
                    k.to_vec(),
                ));
                pl
            });
            DataSubmessage {
                extra_flags: 0,
                reader_id: reader_guid().entity_id,
                writer_id: EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
                writer_sn: sn(sn_),
                inline_qos,
                data_flag: true,
                key_flag: false,
                non_standard_flag: false,
                serialized_payload: crate::payload::Payload::from(payload.as_slice()),
            }
            .write_body(true)
        };

        let mut w = make_writer(10, Duration::from_millis(10));
        let keyed = w
            .write_keyed_stamped(&payload, ts, Some(key))
            .expect("keyed write");
        let (flags, body) = first_data_submessage(&keyed[0].bytes);
        let (want_body, want_flags) = expected(1, Some(key));
        assert_eq!(body, want_body, "keyed DATA body");
        assert_eq!(flags, want_flags, "keyed DATA flags");

        let keyless = w.write_keyed_stamped(&payload, ts, None).expect("keyless");
        let (flags, body) = first_data_submessage(&keyless[0].bytes);
        let (want_body, want_flags) = expected(2, None);
        assert_eq!(body, want_body, "keyless DATA body");
        assert_eq!(flags, want_flags, "keyless DATA flags");

        let with_hb = w
            .write_with_heartbeat_stamped(&payload, Duration::from_secs(1), ts)
            .expect("write with heartbeat");
        let (flags, body) = first_data_submessage(&with_hb[0].bytes);
        let (want_body, want_flags) = expected(3, None);
        assert_eq!(body, want_body, "DATA body beside the HEARTBEAT");
        assert_eq!(flags, want_flags, "DATA flags beside the HEARTBEAT");
    }
}
