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

use alloc::rc::Rc;

use crate::error::WireError;
use crate::fragment_assembler::{AssemblerCaps, FragmentAssembler};
use crate::header::RtpsHeader;
use crate::history_cache::{CacheChange, ChangeKind, HistoryCache};
use crate::message_builder::OutboundDatagram;
use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
use crate::submessages::{
    AckNackSubmessage, DataFragSubmessage, DataSubmessage, GapSubmessage, HeartbeatSubmessage,
    NackFragSubmessage, SequenceNumberSet,
};
use crate::wire_types::{Guid, GuidPrefix, SequenceNumber, VendorId};
use crate::writer_proxy::WriterProxy;

/// Default heartbeat response delay.
///
/// RTPS 2.5 §8.4.15.7 allows the reader a configurable delay
/// between HEARTBEAT receipt and ACKNACK emit, to
/// batch multiple HBs. The spec specifies no fixed default — the previously
/// used 200 ms are a pre-1.0 implementation detail.
///
/// **0 ms** = synchronous ACK response. The Cyclone DDS default is also
/// 0 (`HeartbeatResponseDelay` XML default). Makes ACKNACK event-driven
/// instead of deferred-batched. No loss of correctness for reliable
/// loopback / low-loss networks; for lossy networks the value can be
/// raised via `ReliableReaderConfig::heartbeat_response_delay`.
///
/// Pre-D.5e: 200 ms — that was an implicit latency floor of 200 ms
/// per roundtrip ACK cycle.
pub const DEFAULT_HEARTBEAT_RESPONSE_DELAY: Duration = Duration::from_millis(0);

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
}

impl WriterProxyState {
    fn new(proxy: WriterProxy, max_samples: usize, caps: AssemblerCaps) -> Self {
        Self {
            proxy,
            received_cache: HistoryCache::new(max_samples),
            delivered_up_to: SequenceNumber(0),
            assembler: FragmentAssembler::new(caps),
            pending_acknack_since: None,
        }
    }
}

/// A reliable reader with 0..N writer proxies.
#[derive(Debug, Clone)]
pub struct ReliableReader {
    guid: Guid,
    vendor_id: VendorId,
    writer_proxies: Vec<WriterProxyState>,
    heartbeat_response_delay: Duration,
    acknack_count: i32,
    nackfrag_count: i32,
    duplicate_frag_count: u64,
    /// Template for new proxies.
    max_samples_per_proxy: usize,
    assembler_caps: AssemblerCaps,
    /// Counter for submessages whose `writer_id` has no proxy.
    unknown_src_count: u64,
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

/// A sample delivered to the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredSample {
    /// GUID of the writer the sample comes from. Makes multi-writer
    /// deduplication possible in the caller.
    pub writer_guid: Guid,
    /// Sequence number in the writer.
    pub sequence_number: SequenceNumber,
    /// Serialized payload (zero-copy via `Arc::clone` from the cache).
    /// Payload.
    pub payload: alloc::sync::Arc<[u8]>,
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
            acknack_count: 0,
            nackfrag_count: 0,
            duplicate_frag_count: 0,
            max_samples_per_proxy: cfg.max_samples_per_proxy,
            assembler_caps: cfg.assembler_caps,
            unknown_src_count: 0,
        }
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
            // Known: preserve the state, only refresh locators + re-arm the
            // ACKNACK (if none is pending yet).
            self.writer_proxies[idx]
                .proxy
                .refresh_locators(proxy.unicast_locators, proxy.multicast_locators);
            self.writer_proxies[idx]
                .pending_acknack_since
                .get_or_insert(Duration::ZERO);
        } else {
            let mut state =
                WriterProxyState::new(proxy, self.max_samples_per_proxy, self.assembler_caps);
            // Duration::ZERO triggers an ACKNACK emit immediately on the next tick()
            // (now - ZERO >= heartbeat_response_delay).
            state.pending_acknack_since = Some(Duration::ZERO);
            self.writer_proxies.push(state);
        }
    }

    /// Removes a writer proxy.
    pub fn remove_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        let idx = self
            .writer_proxies
            .iter()
            .position(|s| s.proxy.remote_writer_guid == guid)?;
        Some(self.writer_proxies.remove(idx).proxy)
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

    // ---------- Incoming Submessages ----------

    /// Process a DATA. Dispatch by `writer_id` to the matching
    /// proxy. Returns the reassembled samples of this proxy.
    ///
    /// Spec §9.6.3.9 PID_STATUS_INFO: with `key_flag=true` + inline QoS
    /// with STATUS_INFO set, the CacheChange is marked
    /// NotAliveDisposed / NotAliveUnregistered / NotAliveDisposedUnregistered
    /// instead of Alive.
    pub fn handle_data(
        &mut self,
        source_prefix: GuidPrefix,
        data: &DataSubmessage,
    ) -> Vec<DeliveredSample> {
        let Some(idx) = self.proxy_index_by_writer(Guid::new(source_prefix, data.writer_id)) else {
            self.unknown_src_count = self.unknown_src_count.saturating_add(1);
            return Vec::new();
        };
        let state = &mut self.writer_proxies[idx];
        let sn = data.writer_sn;
        if state.proxy.is_known(sn) || sn <= state.delivered_up_to {
            return Vec::new();
        }
        state.proxy.received_change_set(sn);
        let kind = Self::classify_change_kind(data);
        let key_hash = data
            .inline_qos
            .as_ref()
            .and_then(crate::inline_qos::find_key_hash);
        // Arc::clone instead of Vec::clone on the payload — the
        // refcount block is shared between DataSubmessage, cache and
        // DeliveredSample.
        let _ = state.received_cache.insert(CacheChange {
            sequence_number: sn,
            payload: alloc::sync::Arc::clone(&data.serialized_payload),
            kind,
            key_hash,
        });
        Self::collect_in_order_for(state)
    }

    /// Classifies an incoming DATA as Alive vs lifecycle marker.
    /// `key_flag=true` indicates a key-only payload; STATUS_INFO in the
    /// inline QoS says whether disposed/unregistered/both.
    fn classify_change_kind(data: &DataSubmessage) -> ChangeKind {
        if !data.key_flag {
            return ChangeKind::Alive;
        }
        let Some(pl) = data.inline_qos.as_ref() else {
            return ChangeKind::Alive;
        };
        let Some(bits) = crate::inline_qos::find_status_info(pl) else {
            return ChangeKind::Alive;
        };
        let disposed = bits & crate::inline_qos::status_info::DISPOSED != 0;
        let unregistered = bits & crate::inline_qos::status_info::UNREGISTERED != 0;
        match (disposed, unregistered) {
            (true, true) => ChangeKind::NotAliveDisposedUnregistered,
            (true, false) => ChangeKind::NotAliveDisposed,
            (false, true) => ChangeKind::NotAliveUnregistered,
            (false, false) => ChangeKind::Alive,
        }
    }

    /// Process a DATA_FRAG. `now` triggers NACK_FRAG scheduling
    /// directly, without waiting for a HEARTBEAT.
    pub fn handle_data_frag(
        &mut self,
        source_prefix: GuidPrefix,
        df: &DataFragSubmessage,
        now: Duration,
    ) -> Vec<DeliveredSample> {
        let Some(idx) = self.proxy_index_by_writer(Guid::new(source_prefix, df.writer_id)) else {
            self.unknown_src_count = self.unknown_src_count.saturating_add(1);
            return Vec::new();
        };
        let state = &mut self.writer_proxies[idx];
        let sn = df.writer_sn;
        if state.proxy.is_known(sn) || sn <= state.delivered_up_to {
            self.duplicate_frag_count = self.duplicate_frag_count.saturating_add(1);
            return Vec::new();
        }
        let result = if let Some(completed) = state.assembler.insert(df) {
            state.proxy.received_change_set(sn);
            let _ = state
                .received_cache
                .insert(CacheChange::alive(sn, completed.payload));
            Self::collect_in_order_for(state)
        } else {
            Vec::new()
        };
        if state.assembler.has_gaps() {
            state.pending_acknack_since.get_or_insert(now);
        }
        result
    }

    /// Process a HEARTBEAT. Dispatch by `writer_id`.
    pub fn handle_heartbeat(
        &mut self,
        source_prefix: GuidPrefix,
        hb: &HeartbeatSubmessage,
        now: Duration,
    ) -> Vec<DeliveredSample> {
        let Some(idx) = self.proxy_index_by_writer(Guid::new(source_prefix, hb.writer_id)) else {
            self.unknown_src_count = self.unknown_src_count.saturating_add(1);
            return Vec::new();
        };
        let state = &mut self.writer_proxies[idx];
        if hb.liveliness_flag {
            return Vec::new();
        }
        state.proxy.update_from_heartbeat(hb.first_sn, hb.last_sn);
        let has_missing = state.proxy.has_missing_changes();
        let has_frag_gaps = state.assembler.has_gaps();
        if !hb.final_flag || has_missing || has_frag_gaps {
            state.pending_acknack_since.get_or_insert(now);
        }
        // An HB with first_sn > delivered_up_to+1 means that samples
        // before first_sn are "lost". `collect_in_order_for` then advances
        // `delivered_up_to` to first_sn-1 and delivers samples from
        // the received_cache that were waiting on the hole fill (e.g. a
        // volatile direct send with SN > delivered_up_to+1).
        Self::collect_in_order_for(state)
    }

    /// Process a GAP. Dispatch by `writer_id`.
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
        let mut sn = gap.gap_start;
        while sn < gap.gap_list.bitmap_base {
            state.proxy.irrelevant_change_set(sn);
            state.assembler.discard(sn);
            sn = SequenceNumber(sn.0 + 1);
        }
        for sn in gap.gap_list.iter_set() {
            state.proxy.irrelevant_change_set(sn);
            state.assembler.discard(sn);
        }
        Self::collect_in_order_for(state)
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
        for idx in 0..self.writer_proxies.len() {
            let Some(since) = self.writer_proxies[idx].pending_acknack_since else {
                continue;
            };
            if now.saturating_sub(since) < self.heartbeat_response_delay {
                continue;
            }
            self.writer_proxies[idx].pending_acknack_since = None;
            let targets = Rc::new(self.writer_proxies[idx].proxy.unicast_locators.clone());

            let incomplete_sns: Vec<SequenceNumber> = self.writer_proxies[idx]
                .assembler
                .incomplete_sns()
                .collect();
            for sn in incomplete_sns {
                let bytes = self.build_nackfrag_datagram(idx, sn)?;
                out.push(OutboundDatagram {
                    bytes,
                    targets: Rc::clone(&targets),
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

    fn collect_in_order_for(state: &mut WriterProxyState) -> Vec<DeliveredSample> {
        // Typically 1 sample per recv in steady-state, occasionally a burst.
        // Pre-alloc with cap=2 eliminates the Vec::grow reallocs without
        // over-allocating on the single-sample path.
        let mut out = Vec::with_capacity(2);
        loop {
            let next = SequenceNumber(state.delivered_up_to.0 + 1);
            if let Some(change) = state.received_cache.get(next) {
                out.push(DeliveredSample {
                    writer_guid: state.proxy.remote_writer_guid,
                    sequence_number: change.sequence_number,
                    payload: change.payload.clone(),
                    kind: change.kind,
                    key_hash: change.key_hash,
                });
                state.delivered_up_to = next;
                state.received_cache.remove_up_to(next);
            } else if state.proxy.is_known(next) && state.proxy.last_available_sn() >= next {
                state.delivered_up_to = next;
            } else if next < state.proxy.first_available_sn() {
                // Writer announced first_sn > next via HEARTBEAT
                // → samples before first_available are "lost" (volatile
                // skip, historic eviction). Advance the delivery pointer
                // so that subsequent SNs in the received_cache can finally
                // be delivered. Spec §8.4.12.4.
                state.delivered_up_to = next;
            } else {
                break;
            }
        }
        out
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
        let missing = state.proxy.missing_changes(256);
        let snset = SequenceNumberSet::from_missing(base, &missing);
        self.acknack_count = self.acknack_count.wrapping_add(1);
        // final_flag=true only if we really have everything up to base-1
        // and no further writer action is needed. For the preemptive
        // AckNack (base=1, empty bitmap, proxy has seen nothing yet)
        // final must be false, otherwise the writer reads it as "reader is
        // up-to-date" and sends no durability resends (Cyclone DDS
        // then shows only HEARTBEATs, no DATA).
        let final_flag = missing.is_empty() && state.proxy.last_available_sn().0 >= 1;
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

    /// source-`guid_prefix` des Default-Writer-Proxys (single_writer_guid).
    fn p1() -> GuidPrefix {
        single_writer_guid().prefix
    }

    /// source-`guid_prefix` des zweiten Writer-Proxys (second_writer_guid).
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
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::sync::Arc::from(alloc::vec![byte]),
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
        r.handle_heartbeat(
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
        let delivered = r.handle_data(p1(), &data(w_eid, r_eid, 1, 0xAA));
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
        assert!(r.handle_data(p1(), &data(w, rd, 2, 0x22)).is_empty());
        assert!(r.handle_data(p1(), &data(w, rd, 3, 0x33)).is_empty());
        let out = r.handle_data(p1(), &data(w, rd, 1, 0x11));
        assert_eq!(
            out.iter().map(|s| s.sequence_number).collect::<Vec<_>>(),
            alloc::vec![sn(1), sn(2), sn(3)]
        );
        assert_eq!(first_state(&r).delivered_up_to, sn(3));
    }

    #[test]
    fn duplicate_data_is_rejected() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        r.handle_data(p1(), &data(w, rd, 1, 0xAA));
        let second = r.handle_data(p1(), &data(w, rd, 1, 0xAA));
        assert!(second.is_empty());
    }

    #[test]
    fn mismatched_writer_id_is_counted() {
        let mut r = make_reader(10);
        let rd = r.guid().entity_id;
        let foreign = EntityId::user_writer_with_key([0xFF, 0xFF, 0xFF]);
        assert!(r.handle_data(p1(), &data(foreign, rd, 1, 0xAA)).is_empty());
        assert_eq!(r.unknown_src_count(), 1);
    }

    // ---------- Wire-Side Lifecycle (T8) ----------

    #[test]
    fn alive_data_yields_alive_changekind() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let delivered = r.handle_data(p1(), &data(w, rd, 1, 0xAA));
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
            key_flag: true,
            non_standard_flag: false,
            serialized_payload: alloc::sync::Arc::from(alloc::vec![0u8; 0]),
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
        );
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::NotAliveDisposed);
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
        let delivered = r.handle_data(p1(), &lifecycle_data(w, rd, 1, [0xEF; 16], bits));
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::NotAliveDisposedUnregistered);
    }

    #[test]
    fn key_flag_without_status_info_falls_back_to_alive() {
        // key_flag=true without PID_STATUS_INFO is spec-borderline — to be
        // safe we fall back to Alive instead of guessing.
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let mut d = data(w, rd, 1, 0xAA);
        d.key_flag = true;
        let delivered = r.handle_data(p1(), &d);
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::Alive);
    }

    #[test]
    fn heartbeat_with_missing_triggers_acknack_after_delay() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 3, 1, false), Duration::ZERO);
        assert!(r.tick(Duration::from_millis(100)).unwrap().is_empty());
        let out = r.tick(Duration::from_millis(250)).unwrap();
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn heartbeat_without_missing_and_final_schedules_no_acknack() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        r.handle_data(p1(), &data(w, rd, 1, 0xAA));
        r.handle_heartbeat(p1(), &heartbeat(w, rd, 1, 1, 1, true), Duration::ZERO);
        assert!(r.tick(Duration::from_secs(10)).unwrap().is_empty());
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

        let d1 = r.handle_data(p1(), &data(w1, rd, 1, 0xAA));
        let d2 = r.handle_data(p2(), &data(w2, rd, 1, 0xBB));

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
        let d = r.handle_data(prefix_b, &data(eid, rd, 1, 0xBB));
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
        r.handle_heartbeat(
            p1(),
            &heartbeat(single_writer_guid().entity_id, rd, 1, 3, 1, false),
            Duration::ZERO,
        );
        r.handle_heartbeat(
            p2(),
            &heartbeat(second_writer_guid().entity_id, rd, 1, 5, 1, false),
            Duration::ZERO,
        );
        let out = r.tick(Duration::from_millis(250)).unwrap();
        // 2 ACKNACKs (one per writer)
        assert_eq!(out.len(), 2);
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
            .submessages
            .iter()
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
        match &parsed.submessages[0] {
            ParsedSubmessage::Unknown { id, .. } => assert_eq!(*id, 0x0E),
            other => panic!("expected INFO_DST first, got {other:?}"),
        }
    }

    #[test]
    fn unknown_writer_id_in_heartbeat_counts_not_crashes() {
        let mut r = make_reader(10);
        let rd = r.guid().entity_id;
        let foreign = EntityId::user_writer_with_key([0xFF, 0xFF, 0xFF]);
        r.handle_heartbeat(
            p1(),
            &heartbeat(foreign, rd, 1, 3, 1, false),
            Duration::ZERO,
        );
        assert_eq!(r.unknown_src_count(), 1);
        assert!(r.tick(Duration::from_secs(1)).unwrap().is_empty());
    }
}
