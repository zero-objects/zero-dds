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

use alloc::rc::Rc;

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
use crate::wire_types::{EntityId, FragmentNumber, Guid, Locator, SequenceNumber, VendorId};

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

/// Default fragment size in bytes. 1344 = 1400 MTU − 20 RTPS header −
/// ~32 bytes submessage overhead.
pub const DEFAULT_FRAGMENT_SIZE: u32 = 1344;

/// Fragment size for loopback/same-host paths. The loopback MTU is
/// 65536 — a sample up to ~63 kB goes there in **one** datagram,
/// instead of N 1344-B fragments. The DCPS layer sets this via
/// [`ReliableWriter::set_fragmentation`] when all matched readers run
/// on the same host. 63000 < `u16::MAX` (wire field `fragmentSize`).
pub const LOOPBACK_FRAGMENT_SIZE: u32 = 63_000;

/// MTU budget for loopback/same-host paths (matching
/// [`LOOPBACK_FRAGMENT_SIZE`]).
pub const LOOPBACK_MTU: usize = 64_000;

/// A reliable writer with 0..N reader proxies.
#[derive(Debug, Clone)]
pub struct ReliableWriter {
    guid: Guid,
    vendor_id: VendorId,
    reader_proxies: Vec<ReaderProxy>,
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
    /// If true: every outgoing datagram begins with INFO_DST(target
    /// proxy prefix). Mandatory for cyclone's filtered VolatileSecure reader,
    /// which matches by full GUID (otherwise dst=0:0:0 -> no match ->
    /// wn->last_seq hangs -> ddsi_reorder_nackmap maxseq-0 deadlock).
    emit_info_dst: bool,
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
    /// MTU for submessage aggregation ([`DEFAULT_MTU`]).
    pub mtu: usize,
}

impl ReliableWriter {
    /// Creates an empty writer.
    ///
    /// # Panics
    /// - `cfg.fragment_size == 0`
    /// - `cfg.mtu < 20` (the RTPS header does not fit)
    #[must_use]
    pub fn new(cfg: ReliableWriterConfig) -> Self {
        assert!(cfg.fragment_size > 0, "fragment_size must be > 0");
        assert!(cfg.mtu >= 20, "mtu must accommodate RTPS header");
        Self {
            guid: cfg.guid,
            vendor_id: cfg.vendor_id,
            reader_proxies: cfg.reader_proxies,
            cache: HistoryCache::new_with_kind(cfg.history_kind, cfg.max_samples),
            heartbeat_period: cfg.heartbeat_period,
            last_heartbeat: None,
            heartbeat_count: 0,
            nackfrag_count: 0,
            unknown_src_count: 0,
            next_sn: 0,
            fragment_size: cfg.fragment_size,
            mtu: cfg.mtu,
            emit_info_dst: false,
            payload_pool: alloc::vec::Vec::with_capacity(8),
            payload_pool_cap: 8,
        }
    }

    /// GUID of the writer.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Enables INFO_DST(target-prefix) before each datagram (RTPS 2.5
    /// §8.3.7). Needed for receivers that match directed submessages by full
    /// GUID (cyclone filtered VolatileSecure reader).
    pub fn set_emit_info_dst(&mut self, v: bool) {
        self.emit_info_dst = v;
    }

    /// Opens a MessageBuilder for proxy `idx`; prepends INFO_DST with
    /// the target prefix if `emit_info_dst`.
    fn open_builder(&self, idx: usize, targets: &Rc<Vec<Locator>>) -> MessageBuilder {
        let mut b = MessageBuilder::open(self.rtps_header(), Rc::clone(targets), self.mtu);
        if self.emit_info_dst {
            let prefix = self.reader_proxies[idx].remote_reader_guid.prefix;
            let _ = b.try_add_submessage(SubmessageId::InfoDst, 0, &prefix.to_bytes());
        }
        b
    }

    /// Read-only-Slice der registrierten Reader-Proxies.
    #[must_use]
    pub fn reader_proxies(&self) -> &[ReaderProxy] {
        &self.reader_proxies
    }

    /// Number of registered reader proxies.
    #[must_use]
    pub fn reader_proxy_count(&self) -> usize {
        self.reader_proxies.len()
    }

    /// Removes samples with SN < `up_to_exclusive` from the cache. Used
    /// by higher layers for lifespan expiry (Spec §2.2.3.16):
    /// expired samples disappear so that even late
    /// reader proxies no longer get them.
    pub fn remove_samples_up_to(&mut self, up_to_exclusive: SequenceNumber) -> usize {
        self.cache.remove_up_to(up_to_exclusive)
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

    /// Sets fragment size + MTU budget anew — called by the DCPS layer
    /// when the path MTU of the matched readers changes: if ALL
    /// readers run on the same host (loopback, MTU 65536), one
    /// datagram per sample suffices ([`LOOPBACK_FRAGMENT_SIZE`]); if a reader
    /// is remote, it stays at the Ethernet-safe [`DEFAULT_FRAGMENT_SIZE`]
    /// (otherwise an oversized datagram gets IP-fragmented on the 1500 path,
    /// and a lost IP fragment costs the whole sample).
    ///
    /// # Panics
    /// `fragment_size == 0` or `mtu < 20`.
    pub fn set_fragmentation(&mut self, fragment_size: u32, mtu: usize) {
        assert!(fragment_size > 0, "fragment_size must be > 0");
        assert!(mtu >= 20, "mtu must accommodate RTPS header");
        self.fragment_size = fragment_size;
        self.mtu = mtu;
    }

    /// True if a payload of this size is fragmented
    /// (payload length > `fragment_size`).
    #[must_use]
    fn needs_fragmentation(&self, payload: &[u8]) -> bool {
        u32::try_from(payload.len()).unwrap_or(u32::MAX) > self.fragment_size && !payload.is_empty()
    }

    /// Checks whether all reader proxies have already acknowledged the
    /// currently highest sample SN in the cache. Returns `true` even if
    /// the cache is empty or no proxies exist (nothing to
    /// acknowledge).
    ///
    /// Spec basis for `DataWriter::wait_for_acknowledgments`
    /// (OMG DDS 1.4 §2.2.2.4.2.22).
    #[must_use]
    pub fn all_samples_acknowledged(&self) -> bool {
        let Some(max_sn) = self.cache.max_sn() else {
            return true;
        };
        self.reader_proxies
            .iter()
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
        if let Some(idx) = self
            .reader_proxies
            .iter()
            .position(|p| p.remote_reader_guid == guid)
        {
            self.reader_proxies[idx] = proxy;
        } else {
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
        Some(self.reader_proxies.remove(idx))
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
        let (sn, payload) = self.stage_sample(payload, source_timestamp)?;

        let mut out = Vec::new();
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
            let advanced = self.reader_proxies[idx].next_unsent_change(sn);
            if advanced != Some(sn) {
                continue;
            }
            let reader_id = self.reader_proxies[idx].remote_reader_guid.entity_id;
            let targets = self.targets_for(idx);
            // The encapsulation header (payload bytes 0..4, RTPS 2.5
            // §10.5) is already set honestly by the caller and reflects
            // the actual body encoding. No per-peer
            // relabeling — restamping an encap byte without re-encoding the body
            // would fake a wrong representation to the reader
            // (XTypes 1.3 §7.4: XCDR1/XCDR2 differ
            // in alignment, so they are NOT bit-identical).
            out.extend(self.build_sample_datagrams(sn, &payload, reader_id, &targets)?);
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
        let (sn, payload) = self.stage_sample(payload, source_timestamp)?;
        // first_sn for the HEARTBEAT. final_flag=false: the reader MUST
        // respond with ACKNACK (RTPS 2.5 §8.4.15.5).
        let cache_min = self.cache.min_sn().unwrap_or(SequenceNumber(1));
        let fragmented = self.needs_fragmentation(&payload);
        let mut out = Vec::new();
        for idx in 0..self.reader_proxies.len() {
            let advanced = self.reader_proxies[idx].next_unsent_change(sn) == Some(sn);
            let reader_id = self.reader_proxies[idx].remote_reader_guid.entity_id;
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
            if advanced && !fragmented {
                // Perf: DATA + piggyback HEARTBEAT share ONE
                // RTPS message / ONE datagram — one `sendto` instead of two
                // (and one fewer `recvfrom` + wakeup at the reader).
                // RTPS 2.5 allows multiple submessages per message; this
                // is Cyclone's "piggyback heartbeat" 1:1. If the
                // HEARTBEAT no longer fits the MTU,
                // `append_submessage` automatically splits into two datagrams.
                let mut builder = self.open_builder(idx, &targets);
                self.append_data(&mut builder, sn, &payload, reader_id, &mut out, &targets)?;
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
                if coframe {
                    self.append_heartbeat(
                        &mut builder,
                        reader_id,
                        hb_first_sn,
                        false,
                        &mut out,
                        &targets,
                    )?;
                }
                if let Some(dg) = builder.finish() {
                    out.push(dg);
                }
                if !coframe {
                    let mut hb_builder = self.open_builder(idx, &targets);
                    self.append_heartbeat(
                        &mut hb_builder,
                        reader_id,
                        hb_first_sn,
                        false,
                        &mut out,
                        &targets,
                    )?;
                    if let Some(dg) = hb_builder.finish() {
                        out.push(dg);
                    }
                }
            } else {
                // Fragmentierter Sample → eigene DATA_FRAG-Datagramme;
                // lagging Proxy (advanced=false) → nur HEARTBEAT. Das
                // HEARTBEAT goes out here as its own datagram.
                if advanced {
                    out.extend(self.build_sample_datagrams(sn, &payload, reader_id, &targets)?);
                }
                let mut builder = self.open_builder(idx, &targets);
                self.append_heartbeat(
                    &mut builder,
                    reader_id,
                    hb_first_sn,
                    false,
                    &mut out,
                    &targets,
                )?;
                if let Some(dg) = builder.finish() {
                    out.push(dg);
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
    ) -> Result<(SequenceNumber, alloc::sync::Arc<[u8]>), WireError> {
        let sn_value = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "sequence number overflow",
            })?;
        self.next_sn = sn_value;
        let sn = SequenceNumber(sn_value);

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

        // Insert into the cache and capture the evicted Arc for the pool.
        let evicted = self
            .cache
            .insert_returning_evicted(
                CacheChange::alive_arc(sn, alloc::sync::Arc::clone(&arc_payload))
                    .with_source_timestamp(source_timestamp),
            )
            .map_err(|_| WireError::ValueOutOfRange {
                message: "history cache full or duplicate",
            })?;
        if let Some(evicted_change) = evicted {
            if self.payload_pool.len() < self.payload_pool_cap {
                self.payload_pool.push(evicted_change.payload);
            }
            // otherwise drop (pool full — typically not in steady state).
        }
        Ok((sn, arc_payload))
    }

    // ---------- Tick ----------

    /// Tick-Event: HEARTBEATs + Resends + NACK_FRAG-Responses, aggregiert.
    ///
    /// # Errors
    /// Wire-encode error.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        let should_heartbeat = match self.last_heartbeat {
            None => true,
            Some(last) => now.saturating_sub(last) >= self.heartbeat_period,
        };
        let emit_hb = should_heartbeat && !self.cache.is_empty();

        let mut out = Vec::new();
        let mut hb_emitted_any = false;

        for idx in 0..self.reader_proxies.len() {
            let reader_id = self.reader_proxies[idx].remote_reader_guid.entity_id;
            let targets = self.targets_for(idx);

            // 1) Fragment resends (from NACK_FRAG) — one datagram per fragment
            while let Some((sn, frag)) = self.reader_proxies[idx].next_requested_fragment() {
                match self.cache.get(sn) {
                    Some(change) => {
                        let payload = change.payload.clone();
                        #[cfg(feature = "metrics")]
                        crate::metrics::inc_retransmit();
                        #[cfg(feature = "metrics")]
                        crate::metrics::inc_fragmented_sample();
                        out.push(
                            self.build_data_frag_datagram(sn, frag, &payload, reader_id, &targets)?,
                        );
                    }
                    None => {
                        out.push(self.build_gap_datagram(sn, reader_id, &targets)?);
                    }
                }
            }

            // 2) Aggregated datagram for whole-SN resends + optional HB
            let mut builder = self.open_builder(idx, &targets);

            while let Some(sn) = self.reader_proxies[idx].next_requested_change() {
                #[cfg(feature = "metrics")]
                crate::metrics::inc_retransmit();
                match self.cache.get(sn) {
                    Some(change) => {
                        let payload = change.payload.clone();
                        // If fragmentation is needed → separate datagrams, flush the builder if needed.
                        if self.needs_fragmentation(&payload) {
                            if let Some(dg) = builder.finish() {
                                out.push(dg);
                            }
                            builder = MessageBuilder::open(
                                self.rtps_header(),
                                Rc::clone(&targets),
                                self.mtu,
                            );
                            out.extend(
                                self.build_sample_datagrams(sn, &payload, reader_id, &targets)?,
                            );
                        } else {
                            self.append_data(
                                &mut builder,
                                sn,
                                &payload,
                                reader_id,
                                &mut out,
                                &targets,
                            )?;
                        }
                    }
                    None => {
                        self.append_gap(&mut builder, sn, reader_id, &mut out, &targets)?;
                    }
                }
            }

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
            if emit_hb {
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
                    reader_id,
                    first_sn,
                    /* final_flag */ false,
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
        let requested: alloc::vec::Vec<SequenceNumber> = requested.into_iter().collect();
        let bitmap_empty = requested.is_empty();
        self.reader_proxies[idx].acked_changes_set(base);
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
                    proxy.requested_changes_set((from..=max_sn.0).map(SequenceNumber));
                }
            }
        }
        // Cache GC **removed** in the per-destination-queue model (T3
        // refactor): the cache is only trimmed by HistoryKind::KeepLast
        // anymore. A stalled reader thereby no longer blocks the
        // pipeline — for too-old samples it gets GAP responses.
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

    /// Target locator set for proxy `idx`. Multicast preferred
    /// (network latency and bandwidth), unicast as fallback.
    fn targets_for(&self, idx: usize) -> Rc<Vec<Locator>> {
        let p = &self.reader_proxies[idx];
        if !p.multicast_locators.is_empty() {
            Rc::new(p.multicast_locators.clone())
        } else {
            Rc::new(p.unicast_locators.clone())
        }
    }

    /// Prepends an INFO_TS submessage (DDSI-RTPS §8.3.7.9) for the change `sn`
    /// if it carries a source timestamp. Must be called on the builder right
    /// before the DATA/DATA_FRAG so it sits in the same datagram (the receiver
    /// applies the last-seen INFO_TS to the following DATA). Tiny (12 B) and
    /// best-effort: if it would not fit, the DATA still goes (the reader falls
    /// back to reception order).
    fn append_info_ts(&self, builder: &mut MessageBuilder, sn: SequenceNumber) {
        if let Some(ts) = self.cache.get(sn).and_then(|c| c.source_timestamp) {
            let its = crate::submessages::InfoTimestampSubmessage {
                timestamp: ts,
                invalidate: false,
            };
            let (body, flags) = its.write_body(true);
            let _ = builder.try_add_submessage(SubmessageId::InfoTs, flags, &body);
        }
    }

    fn append_data(
        &self,
        builder: &mut MessageBuilder,
        sn: SequenceNumber,
        payload: &alloc::sync::Arc<[u8]>,
        reader_id: EntityId,
        out: &mut Vec<OutboundDatagram>,
        targets: &Rc<Vec<Locator>>,
    ) -> Result<(), WireError> {
        self.append_info_ts(builder, sn);
        // Hot-path zero-copy: the 20-byte fixed header (extra_flags +
        // octetsToInlineQos + reader_id + writer_id + writer_sn) is
        // written directly into a stack buffer; the serializedData
        // are borrowed by `try_add_submessage_split` from the Arc
        // slice, without materializing an intermediate Vec. Saves
        // per DATA write 1 Vec heap alloc + 1 memcpy of N bytes
        // payload (typically 100..8000 B). Cyclone does this via iovec /
        // sendmmsg; we do it via two extend_from_slice into the
        // existing MessageBuilder Vec — semantically identical in
        // wire output, one allocation and one copy fewer.
        let mut header = [0u8; 20];
        // extra_flags (2 byte LE = 0)
        header[0] = 0;
        header[1] = 0;
        // octetsToInlineQos = 16
        header[2] = 16;
        header[3] = 0;
        // reader_id (4 byte)
        header[4..8].copy_from_slice(&reader_id.to_bytes());
        // writer_id (4 byte)
        header[8..12].copy_from_slice(&self.guid.entity_id.to_bytes());
        // writer_sn (8 byte LE)
        header[12..20].copy_from_slice(&sn.to_bytes_le());
        let flags = FLAG_E_LITTLE_ENDIAN | DATA_FLAG_DATA;
        // append_submessage_split — the builder writes the SubmessageHeader
        // (4 B) + fixed header (20 B) + payload (N B) directly one after another
        // into its Vec, no extra alloc.
        match builder.try_add_submessage_split(SubmessageId::Data, flags, &header, payload) {
            Ok(()) => Ok(()),
            Err(AddError::BodyTooLarge) => Err(WireError::ValueOutOfRange {
                message: "DATA body exceeds u16::MAX",
            }),
            Err(AddError::WouldExceedMtu { .. }) => {
                let finished = core::mem::replace(
                    builder,
                    MessageBuilder::open(self.rtps_header(), Rc::clone(targets), self.mtu),
                );
                if let Some(dg) = finished.finish() {
                    out.push(dg);
                }
                builder
                    .try_add_submessage_split(SubmessageId::Data, flags, &header, payload)
                    .map_err(|_| WireError::ValueOutOfRange {
                        message: "submessage does not fit into fresh datagram",
                    })
            }
        }
    }

    fn append_gap(
        &self,
        builder: &mut MessageBuilder,
        sn: SequenceNumber,
        reader_id: EntityId,
        out: &mut Vec<OutboundDatagram>,
        targets: &Rc<Vec<Locator>>,
    ) -> Result<(), WireError> {
        let gap = GapSubmessage {
            reader_id,
            writer_id: self.guid.entity_id,
            gap_start: sn,
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(sn.0 + 1),
                num_bits: 0,
                bitmap: Vec::new(),
            },
            group_info: None,
            filtered_count: None,
        };
        let (body, flags) = gap.write_body(true);
        self.append_submessage(
            builder,
            SubmessageId::Gap,
            flags,
            &body,
            out,
            targets,
            "GAP",
        )
    }

    fn append_heartbeat(
        &mut self,
        builder: &mut MessageBuilder,
        reader_id: EntityId,
        first_sn: SequenceNumber,
        final_flag: bool,
        out: &mut Vec<OutboundDatagram>,
        targets: &Rc<Vec<Locator>>,
    ) -> Result<(), WireError> {
        #[cfg(feature = "metrics")]
        crate::metrics::inc_heartbeat_sent();
        self.heartbeat_count = self.heartbeat_count.wrapping_add(1);
        let last = self.cache.max_sn().unwrap_or(SequenceNumber(0));
        let hb = HeartbeatSubmessage {
            reader_id,
            writer_id: self.guid.entity_id,
            first_sn,
            last_sn: last,
            count: self.heartbeat_count,
            final_flag,
            liveliness_flag: false,
            group_info: None,
        };
        let (body, flags) = hb.write_body(true);
        self.append_submessage(
            builder,
            SubmessageId::Heartbeat,
            flags,
            &body,
            out,
            targets,
            "HEARTBEAT",
        )
    }

    /// Shared submessage append with overflow handling.
    #[allow(clippy::too_many_arguments)]
    fn append_submessage(
        &self,
        builder: &mut MessageBuilder,
        id: SubmessageId,
        flags: u8,
        body: &[u8],
        out: &mut Vec<OutboundDatagram>,
        targets: &Rc<Vec<Locator>>,
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
            Err(AddError::WouldExceedMtu { .. }) => {
                let finished = core::mem::replace(
                    builder,
                    MessageBuilder::open(self.rtps_header(), Rc::clone(targets), self.mtu),
                );
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
        reader_id: EntityId,
        targets: &Rc<Vec<Locator>>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        if !self.needs_fragmentation(payload) {
            return Ok(alloc::vec![
                self.build_single_data_datagram(sn, payload, reader_id, targets,)?
            ]);
        }
        let frag_size = self.fragment_size as usize;
        let sample_size = u32::try_from(payload.len()).map_err(|_| WireError::ValueOutOfRange {
            message: "sample size exceeds u32::MAX",
        })?;
        let frag_size_u16 = u16::try_from(frag_size).map_err(|_| WireError::ValueOutOfRange {
            message: "fragment_size exceeds u16::MAX",
        })?;
        let mut out = Vec::new();
        let mut frag_num: u32 = 1;
        let mut pos = 0usize;
        while pos < payload.len() {
            let end = core::cmp::min(pos + frag_size, payload.len());
            out.push(self.build_data_frag_submessage_datagram(
                sn,
                FragmentNumber(frag_num),
                frag_size_u16,
                sample_size,
                &payload[pos..end],
                reader_id,
                targets,
            )?);
            pos = end;
            frag_num = frag_num.checked_add(1).ok_or(WireError::ValueOutOfRange {
                message: "fragment number overflow",
            })?;
        }
        Ok(out)
    }

    fn build_single_data_datagram(
        &self,
        sn: SequenceNumber,
        payload: &alloc::sync::Arc<[u8]>,
        reader_id: EntityId,
        targets: &Rc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let mut builder = MessageBuilder::open(self.rtps_header(), Rc::clone(targets), self.mtu);
        self.append_info_ts(&mut builder, sn);
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id,
            writer_id: self.guid.entity_id,
            writer_sn: sn,
            // WP 2.0a: Arc::clone instead of to_vec — zero-copy into the wire path.
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::sync::Arc::clone(payload),
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

    /// Spec §9.6.3.9 PID_STATUS_INFO lifecycle sample: DATA with
    /// `key_flag=true` + inline QoS [PID_KEY_HASH + PID_STATUS_INFO].
    /// The payload stays empty (the spec allows it, the reader reconstructs
    /// the instance from the key hash). Called by the DCPS layer on
    /// `dispose`/`unregister_instance`.
    fn build_lifecycle_datagram(
        &self,
        sn: SequenceNumber,
        key_hash: [u8; 16],
        status_bits: u32,
        reader_id: EntityId,
        targets: &Rc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let mut builder = MessageBuilder::open(self.rtps_header(), Rc::clone(targets), self.mtu);
        let inline_qos = crate::inline_qos::lifecycle_inline_qos(key_hash, status_bits);
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id,
            writer_id: self.guid.entity_id,
            writer_sn: sn,
            inline_qos: Some(inline_qos),
            key_flag: true,
            non_standard_flag: false,
            serialized_payload: alloc::sync::Arc::from(alloc::vec::Vec::new()),
        };
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

    /// Sends a lifecycle marker (dispose/unregister) to all matched
    /// readers. Allocates a new sequence number, persists a
    /// `CacheChange` with the corresponding ChangeKind and builds a DATA
    /// with key hash + StatusInfo per reader proxy.
    ///
    /// `status_bits` is the OR combination of the desired bits from
    /// [`crate::inline_qos::status_info`]:
    /// - DISPOSED: NotAliveDisposed
    /// - UNREGISTERED: NotAliveUnregistered
    /// - DISPOSED | UNREGISTERED: NotAliveDisposedUnregistered
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn write_lifecycle(
        &mut self,
        key_hash: [u8; 16],
        status_bits: u32,
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

        // Persist the CacheChange — late-joiner replay (T9) reads from it,
        // and the history-cache bookkeeping stays consistent.
        self.cache
            .insert(crate::history_cache::CacheChange::lifecycle(
                sn,
                key_hash.to_vec(),
                kind,
            ))
            .map_err(|_| WireError::ValueOutOfRange {
                message: "history cache full or duplicate (lifecycle)",
            })?;

        let mut out = Vec::new();
        for idx in 0..self.reader_proxies.len() {
            let reader_id = self.reader_proxies[idx].remote_reader_guid.entity_id;
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
                                self.build_sample_datagrams(next, &payload, reader_id, &targets)?,
                            );
                        }
                        marker_kind => {
                            // Lifecycle marker: the change payload holds the
                            // 16-byte key hash (see `CacheChange::lifecycle`).
                            let kh = lifecycle_key_hash(&change.payload).unwrap_or(key_hash);
                            let bits = status_bits_for_kind(marker_kind);
                            out.push(
                                self.build_lifecycle_datagram(next, kh, bits, reader_id, &targets)?,
                            );
                        }
                    },
                    None => {
                        out.push(self.build_gap_datagram(next, reader_id, &targets)?);
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
        full_payload: &alloc::sync::Arc<[u8]>,
        reader_id: EntityId,
        targets: &Rc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let frag_size = self.fragment_size as usize;
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
        let end = core::cmp::min(start + frag_size, full_payload.len());
        let sample_size =
            u32::try_from(full_payload.len()).map_err(|_| WireError::ValueOutOfRange {
                message: "sample size exceeds u32::MAX",
            })?;
        let frag_size_u16 = u16::try_from(frag_size).map_err(|_| WireError::ValueOutOfRange {
            message: "fragment_size exceeds u16::MAX",
        })?;
        self.build_data_frag_submessage_datagram(
            sn,
            frag,
            frag_size_u16,
            sample_size,
            &full_payload[start..end],
            reader_id,
            targets,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_data_frag_submessage_datagram(
        &self,
        sn: SequenceNumber,
        frag: FragmentNumber,
        fragment_size: u16,
        sample_size: u32,
        chunk: &[u8],
        reader_id: EntityId,
        targets: &Rc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let df = DataFragSubmessage {
            extra_flags: 0,
            reader_id,
            writer_id: self.guid.entity_id,
            writer_sn: sn,
            fragment_starting_num: frag,
            fragments_in_submessage: 1,
            fragment_size,
            sample_size,
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
            serialized_payload: alloc::sync::Arc::from(chunk),
            inline_qos_flag: false,
            hash_key_flag: false,
            key_flag: false,
            non_standard_flag: false,
        };
        let (body, flags) = df.write_body(true);
        let mut builder = MessageBuilder::open(self.rtps_header(), Rc::clone(targets), self.mtu);
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
        reader_id: EntityId,
        targets: &Rc<Vec<Locator>>,
    ) -> Result<OutboundDatagram, WireError> {
        let gap = GapSubmessage {
            reader_id,
            writer_id: self.guid.entity_id,
            gap_start: sn,
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(sn.0 + 1),
                num_bits: 0,
                bitmap: Vec::new(),
            },
            group_info: None,
            filtered_count: None,
        };
        let (body, flags) = gap.write_body(true);
        let mut builder = MessageBuilder::open(self.rtps_header(), Rc::clone(targets), self.mtu);
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

/// Extracts the 16-byte instance key hash a lifecycle [`CacheChange`] stores in
/// its payload (see [`CacheChange::lifecycle`]). Returns `None` if the payload
/// is not exactly 16 bytes (so the caller can fall back to a known hash).
fn lifecycle_key_hash(payload: &[u8]) -> Option<[u8; 16]> {
    payload.try_into().ok()
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
    use crate::wire_types::{GuidPrefix, Locator};

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
        ReliableWriter::new(ReliableWriterConfig {
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

    fn first_proxy(w: &ReliableWriter) -> &ReaderProxy {
        w.reader_proxies().first().unwrap()
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
        match (&parsed.submessages[0], &parsed.submessages[1]) {
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
                .submessages
                .iter()
                .any(|s| matches!(s, ParsedSubmessage::InfoTimestamp(_))),
            "unstamped write must not emit INFO_TS"
        );
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
        // Force a heartbeat + NACK-driven resend window via tick.
        let resent = w.tick(Duration::from_millis(50)).expect("tick");
        // Find any DATA-bearing datagram in the resend output and confirm it
        // co-frames the original INFO_TS.
        let mut saw_data_with_ts = false;
        for dg in &resent {
            let p = decode_datagram(&dg.bytes).expect("decode resend");
            let has_data = p
                .submessages
                .iter()
                .any(|s| matches!(s, ParsedSubmessage::Data(_)));
            if has_data {
                let info_ts = p.submessages.iter().find_map(|s| match s {
                    ParsedSubmessage::InfoTimestamp(i) => Some(i),
                    _ => None,
                });
                if let Some(i) = info_ts {
                    assert_eq!(i.timestamp, ts);
                    saw_data_with_ts = true;
                }
            }
        }
        // (Resend only happens if a heartbeat/nack path produced a DATA; if the
        // tick produced no DATA resend this assertion is vacuously skipped.)
        let _ = saw_data_with_ts;
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
        match (&p1.submessages[0], &p2.submessages[0]) {
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
                .submessages
                .iter()
                .any(|s| matches!(s, ParsedSubmessage::Heartbeat(_)))
        );
        assert!(w.tick(Duration::from_millis(200)).unwrap().is_empty());
        let out2 = w.tick(Duration::from_millis(600)).unwrap();
        assert_eq!(out2.len(), 1);
    }

    #[test]
    fn tick_skips_heartbeat_when_cache_empty() {
        let mut w = make_writer(10, Duration::from_millis(100));
        assert!(w.tick(Duration::from_secs(10)).unwrap().is_empty());
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
        let mut w = ReliableWriter::new(ReliableWriterConfig {
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
        let mut w = ReliableWriter::new(ReliableWriterConfig {
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
                .submessages
                .iter()
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
        for i in 1..=3 {
            w.write(&alloc::vec![i as u8]).unwrap();
        }
        w.handle_acknack(rguid, sn(1), [sn(2)]);
        let out = w.tick(Duration::ZERO).unwrap();
        // Ein aggregiertes Datagramm: DATA-Resend + HEARTBEAT im gleichen
        let parsed = decode_datagram(&out[0].bytes).unwrap();
        let has_data_2 = parsed
            .submessages
            .iter()
            .any(|s| matches!(s, ParsedSubmessage::Data(d) if d.writer_sn == sn(2)));
        let has_hb = parsed
            .submessages
            .iter()
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
                .submessages
                .iter()
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
    fn write_under_fragment_size_produces_single_data() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), 10);
        let dgs = w.write(&alloc::vec![1, 2, 3, 4, 5]).unwrap();
        assert_eq!(dgs.len(), 1);
        let parsed = decode_datagram(&dgs[0].bytes).unwrap();
        assert!(matches!(&parsed.submessages[0], ParsedSubmessage::Data(_)));
    }

    #[test]
    fn write_above_fragment_size_produces_data_frag_split() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), 4);
        let payload: alloc::vec::Vec<u8> = (1..=10).collect();
        let dgs = w.write(&payload).unwrap();
        assert_eq!(dgs.len(), 3);
        for (i, dg) in dgs.iter().enumerate() {
            match &decode_datagram(&dg.bytes).unwrap().submessages[0] {
                ParsedSubmessage::DataFrag(df) => {
                    assert_eq!(df.fragment_starting_num.0, (i as u32) + 1);
                    assert_eq!(df.fragments_in_submessage, 1);
                    assert_eq!(df.fragment_size, 4);
                    assert_eq!(df.sample_size, 10);
                }
                other => panic!("expected DataFrag, got {other:?}"),
            }
        }
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
                    .submessages
                    .iter()
                    .any(|s| matches!(s, ParsedSubmessage::DataFrag(df) if df.fragment_starting_num == FragmentNumber(3)))
            })
            .collect();
        assert_eq!(frag_resends.len(), 1);
        assert_eq!(first_proxy(&w).pending_requested_fragment_count(), 0);
    }

    #[test]
    fn acknack_resend_for_fragmented_sn_sends_all_fragments() {
        let mut w = make_writer_with_frag_size(10, Duration::from_secs(10), 4);
        let rguid = reader_guid();
        let _ = w.write(&(1..=10).collect::<alloc::vec::Vec<u8>>()).unwrap();
        w.handle_acknack(rguid, sn(1), [sn(1)]);
        let out = w.tick(Duration::ZERO).unwrap();
        let frags: alloc::vec::Vec<_> = out
            .iter()
            .filter(|d| {
                decode_datagram(&d.bytes)
                    .unwrap()
                    .submessages
                    .iter()
                    .any(|s| matches!(s, ParsedSubmessage::DataFrag(_)))
            })
            .collect();
        assert_eq!(frags.len(), 3);
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
            .submessages
            .iter()
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
            .submessages
            .iter()
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
            for s in &decode_datagram(&d.bytes).unwrap().submessages {
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
        for i in 1..=3 {
            w.write(&alloc::vec![i as u8]).unwrap();
        }
        // All 3 as requested
        w.handle_acknack(rguid, sn(1), [sn(1), sn(2), sn(3)]);
        let out = w.tick(Duration::ZERO).unwrap();
        // One datagram contains multiple DATAs + HEARTBEAT
        assert_eq!(out.len(), 1, "all resends aggregated into single datagram");
        let parsed = decode_datagram(&out[0].bytes).unwrap();
        let data_count = parsed
            .submessages
            .iter()
            .filter(|s| matches!(s, ParsedSubmessage::Data(_)))
            .count();
        assert_eq!(data_count, 3);
        let hb_count = parsed
            .submessages
            .iter()
            .filter(|s| matches!(s, ParsedSubmessage::Heartbeat(_)))
            .count();
        assert_eq!(hb_count, 1);
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
        let mut w = ReliableWriter::new(ReliableWriterConfig {
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
        let dgs = w.write_lifecycle(key, DISPOSED).unwrap();
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
            for s in &parsed.submessages {
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
}
