// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reliable RTPS-Reader (1:N Writer-Proxies) — DDSI-RTPS 2.5 §8.4.10.
//!
//! Entspricht der [`StatefulReader`]-Rolle mit 1..N matched Writers.
//! Fragmentation (§8.4.14) ist unterstuetzt. Multi-Writer ab WP 1.4
//! T4.5: pro Remote-Writer getrennter [`WriterProxyState`] mit eigenem
//! `received_cache`, `delivered_up_to` und `FragmentAssembler`.
//!
//! # Warum pro-Proxy State?
//!
//! SequenceNumbers sind writer-lokal (Spec §8.3.5.4). Zwei Writer mit
//! ueberlappenden SN-Spaces wuerden in einem globalen Cache kollidieren
//! — daher separate Puffer pro Proxy.
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
use crate::wire_types::{Guid, SequenceNumber, VendorId};
use crate::writer_proxy::WriterProxy;

/// Default-Heartbeat-Response-Delay.
///
/// RTPS 2.5 §8.4.15.7 erlaubt dem Reader einen konfigurierbaren Delay
/// zwischen HEARTBEAT-Empfang und ACKNACK-Emit, um mehrere HBs zu
/// batchen. Spec spezifiziert keinen festen Default — die zuvor
/// verwendeten 200 ms sind ein Pre-1.0-Implementierungsdetail.
///
/// **0 ms** = synchrone ACK-Response. Cyclone DDS default ist ebenfalls
/// 0 (`HeartbeatResponseDelay`-XML-Default). Macht ACKNACK event-driven
/// statt deferred-batched. Kein Verlust an Korrektheit fuer reliable
/// loopback / low-loss-Netze; bei lossy-Netzen kann der Wert via
/// `ReliableReaderConfig::heartbeat_response_delay` hochgesetzt werden.
///
/// Pre-D.5e: 200 ms — das war ein impliziter Latency-Floor von 200 ms
/// pro Roundtrip-ACK-Cycle.
pub const DEFAULT_HEARTBEAT_RESPONSE_DELAY: Duration = Duration::from_millis(0);

/// Pro-Writer State: der Proxy + getrennter Empfangs-State.
///
/// Jeder Remote-Writer hat seinen eigenen SN-Space (§8.3.5.4), also
/// auch eigenen `received_cache`, `delivered_up_to` und
/// `FragmentAssembler`. So koennen zwei Writer mit kollidierenden SN
/// (z.B. beide starten bei 1) problemlos parallel empfangen werden.
#[derive(Debug, Clone)]
pub struct WriterProxyState {
    /// Writer-Proxy-Protokoll-State.
    pub proxy: WriterProxy,
    /// Empfangs-Cache fuer diesen Writer.
    pub received_cache: HistoryCache,
    /// Hoechste SN, die an die App ausgeliefert wurde.
    pub delivered_up_to: SequenceNumber,
    /// Fragment-Reassembly fuer diesen Writer.
    pub assembler: FragmentAssembler,
    /// Zeitpunkt, seit wann ein ACKNACK/NACK_FRAG an diesen Writer
    /// ausstehend ist. `None` = nichts ausstehend.
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

/// Ein Reliable-Reader mit 0..N Writer-Proxies.
#[derive(Debug, Clone)]
pub struct ReliableReader {
    guid: Guid,
    vendor_id: VendorId,
    writer_proxies: Vec<WriterProxyState>,
    heartbeat_response_delay: Duration,
    acknack_count: i32,
    nackfrag_count: i32,
    duplicate_frag_count: u64,
    /// Template fuer neue Proxies.
    max_samples_per_proxy: usize,
    assembler_caps: AssemblerCaps,
    /// Zaehler fuer Submessages, deren `writer_id` keinen Proxy hat.
    unknown_src_count: u64,
}

/// Konfiguration beim Anlegen.
#[derive(Debug, Clone)]
pub struct ReliableReaderConfig {
    /// GUID des Reader-Endpoints.
    pub guid: Guid,
    /// VendorId fuer den RTPS-Header der ACKNACKs.
    pub vendor_id: VendorId,
    /// Initiale Writer-Proxies. Weitere via `add_writer_proxy`.
    pub writer_proxies: Vec<WriterProxy>,
    /// Kapazitaet des Empfangs-Caches pro Proxy (nicht global).
    pub max_samples_per_proxy: usize,
    /// Heartbeat-Response-Delay (Default: 200 ms).
    pub heartbeat_response_delay: Duration,
    /// Caps fuer den Fragment-Assembler (pro Proxy).
    pub assembler_caps: AssemblerCaps,
}

/// Ein an die Applikation ausgelieferter Sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredSample {
    /// GUID des Writers, von dem der Sample kommt. Macht Multi-Writer-
    /// Deduplication im Caller moeglich.
    pub writer_guid: Guid,
    /// Sequence-Number im Writer.
    pub sequence_number: SequenceNumber,
    /// Serialisierter Payload (Zero-Copy via `Arc::clone` aus dem Cache).
    /// Nutzlast.
    pub payload: alloc::sync::Arc<[u8]>,
    /// Spec §8.2.1.2 ChangeKind — `Alive` fuer normale Samples,
    /// `NotAliveDisposed` / `NotAliveUnregistered` /
    /// `NotAliveDisposedUnregistered` fuer Lifecycle-Marker, die der
    /// Writer per `dispose`/`unregister_instance` versendet hat.
    /// Spec §9.6.3.9 PID_STATUS_INFO im Inline-QoS.
    pub kind: ChangeKind,
    /// `PID_KEY_HASH` aus dem Inline-QoS (Spec §9.6.4.8). Bei
    /// Lifecycle-Markern ist das die Identitaet der disposed/
    /// unregistered Instanz; bei keyed-Topic-ALIVE-Samples optional
    /// (manche Vendors senden Inline-Hash, manche nicht). `None`
    /// wenn der Writer keinen Hash inline mitliefert (typisch fuer
    /// keyless Topics).
    pub key_hash: Option<[u8; 16]>,
}

impl ReliableReader {
    /// Erzeugt einen frischen Reader.
    ///
    /// # Panics
    /// Wenn `cfg.assembler_caps.max_pending_sns == 0`.
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

    /// Read-only-Slice der Writer-Proxy-States.
    #[must_use]
    pub fn writer_proxies(&self) -> &[WriterProxyState] {
        &self.writer_proxies
    }

    /// Anzahl registrierter Writer-Proxies.
    #[must_use]
    pub fn writer_proxy_count(&self) -> usize {
        self.writer_proxies.len()
    }

    /// Zaehler der gesendeten ACKNACKs.
    #[must_use]
    pub fn acknack_count(&self) -> i32 {
        self.acknack_count
    }

    /// Zaehler der gesendeten NACK_FRAGs.
    #[must_use]
    pub fn nackfrag_count(&self) -> i32 {
        self.nackfrag_count
    }

    /// Summe der aktiven (unvollstaendigen) Fragment-Buffer ueber alle
    /// Proxies.
    #[must_use]
    pub fn pending_fragment_count(&self) -> usize {
        self.writer_proxies.iter().map(|s| s.assembler.len()).sum()
    }

    /// Summe der verworfenen Fragmente ueber alle Proxies
    /// (DoS-/Inkonsistenz-Diagnose).
    #[must_use]
    pub fn dropped_fragment_count(&self) -> u64 {
        self.writer_proxies
            .iter()
            .map(|s| s.assembler.drop_count())
            .sum()
    }

    /// Anzahl DATA_FRAGs, die fuer bereits-bekannte SNs eintrafen
    /// (Duplicate-Fragments, Re-Sends).
    #[must_use]
    pub fn duplicate_fragment_count(&self) -> u64 {
        self.duplicate_frag_count
    }

    /// Anzahl Submessages, deren `writer_id` keinem registrierten
    /// Proxy zuzuordnen war (Misrouting / Spoofing-Diagnose).
    #[must_use]
    pub fn unknown_src_count(&self) -> u64 {
        self.unknown_src_count
    }

    /// Fuegt einen Writer-Proxy hinzu. Idempotent: gleiche GUID ersetzt.
    ///
    /// Setzt sofort ein preemptives ACKNACK als pending, damit der Writer
    /// beim naechsten Tick ein "Hallo, ich bin hier"-ACKNACK bekommt.
    /// Cyclone DDS reagiert darauf mit einem HEARTBEAT und beginnt mit
    /// DATA-Resends — ohne diesen Impuls wartet der Writer passiv.
    pub fn add_writer_proxy(&mut self, proxy: WriterProxy) {
        let guid = proxy.remote_writer_guid;
        let mut state =
            WriterProxyState::new(proxy, self.max_samples_per_proxy, self.assembler_caps);
        // Duration::ZERO triggert beim naechsten tick() sofort einen
        // ACKNACK-Emit (now - ZERO >= heartbeat_response_delay).
        state.pending_acknack_since = Some(Duration::ZERO);
        if let Some(idx) = self
            .writer_proxies
            .iter()
            .position(|s| s.proxy.remote_writer_guid == guid)
        {
            self.writer_proxies[idx] = state;
        } else {
            self.writer_proxies.push(state);
        }
    }

    /// Entfernt einen Writer-Proxy.
    pub fn remove_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        let idx = self
            .writer_proxies
            .iter()
            .position(|s| s.proxy.remote_writer_guid == guid)?;
        Some(self.writer_proxies.remove(idx).proxy)
    }

    /// Nulliert alle Diagnose-Zaehler. Beruehrt keine State-Maschine.
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

    /// DATA verarbeiten. Dispatch nach `writer_id` auf den passenden
    /// Proxy. Liefert die reassemblierten Samples dieses Proxies.
    ///
    /// Spec §9.6.3.9 PID_STATUS_INFO: bei `key_flag=true` + Inline-QoS
    /// mit gesetztem STATUS_INFO wird der CacheChange mit
    /// NotAliveDisposed / NotAliveUnregistered / NotAliveDisposedUnregistered
    /// markiert, statt Alive.
    pub fn handle_data(&mut self, data: &DataSubmessage) -> Vec<DeliveredSample> {
        let Some(idx) = self.proxy_index_by_writer_id(data.writer_id) else {
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
        // Arc::clone statt Vec::clone auf dem Payload — der
        // Refcount-Block wird zwischen DataSubmessage, Cache und
        // DeliveredSample geteilt.
        let _ = state.received_cache.insert(CacheChange {
            sequence_number: sn,
            payload: alloc::sync::Arc::clone(&data.serialized_payload),
            kind,
            key_hash,
        });
        Self::collect_in_order_for(state)
    }

    /// Klassifiziert eine eingehende DATA als Alive vs Lifecycle-Marker.
    /// `key_flag=true` zeigt Key-Only-Payload an; STATUS_INFO im
    /// Inline-QoS sagt, ob disposed/unregistered/beides.
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

    /// DATA_FRAG verarbeiten. `now` triggert NACK_FRAG-Scheduling
    /// direkt, ohne auf HEARTBEAT zu warten.
    pub fn handle_data_frag(
        &mut self,
        df: &DataFragSubmessage,
        now: Duration,
    ) -> Vec<DeliveredSample> {
        let Some(idx) = self.proxy_index_by_writer_id(df.writer_id) else {
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

    /// HEARTBEAT verarbeiten. Dispatch nach `writer_id`.
    pub fn handle_heartbeat(
        &mut self,
        hb: &HeartbeatSubmessage,
        now: Duration,
    ) -> Vec<DeliveredSample> {
        let Some(idx) = self.proxy_index_by_writer_id(hb.writer_id) else {
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
        // Ein HB mit first_sn > delivered_up_to+1 bedeutet, dass Samples
        // vor first_sn "lost" sind. `collect_in_order_for` rueckt dann
        // `delivered_up_to` bis first_sn-1 vor und liefert Samples aus
        // dem received_cache, die auf den Hole-Fill warteten (z.B. ein
        // Volatile-direkt-send mit SN > delivered_up_to+1).
        Self::collect_in_order_for(state)
    }

    /// GAP verarbeiten. Dispatch nach `writer_id`.
    pub fn handle_gap(&mut self, gap: &GapSubmessage) -> Vec<DeliveredSample> {
        let Some(idx) = self.proxy_index_by_writer_id(gap.writer_id) else {
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

    /// Tick: liefert faellige ACKNACK/NACK_FRAG-Datagramme **ueber alle
    /// Proxies hinweg**. Pro Proxy ein eigenes ACKNACK/NACK_FRAG, weil
    /// SN-Spaces pro Writer sind.
    ///
    /// # Errors
    /// Wire-Encode-Fehler.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<Vec<u8>>, WireError> {
        Ok(self
            .tick_outbound(now)?
            .into_iter()
            .map(|d| d.bytes)
            .collect())
    }

    /// Wie [`Self::tick`], aber mit Ziel-Locators fuer jedes Datagram.
    /// Bevorzugt fuer Transport-Integration, weil jeder AckNack an den
    /// konkreten Writer-Proxy-Unicast-Locator gehen muss.
    ///
    /// # Errors
    /// `WireError::ValueOutOfRange` bei ueberlangem Submessage-Body.
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

    // ---------- Intern ----------

    fn proxy_index_by_writer_id(&self, writer_id: crate::wire_types::EntityId) -> Option<usize> {
        self.writer_proxies
            .iter()
            .position(|s| s.proxy.remote_writer_guid.entity_id == writer_id)
    }

    fn collect_in_order_for(state: &mut WriterProxyState) -> Vec<DeliveredSample> {
        let mut out = Vec::new();
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
                // Writer hat via HEARTBEAT first_sn > next angekuendigt
                // → Samples vor first_available sind "lost" (Volatile-
                // Skip, Historic-Eviction). Delivery-Pointer weiterruecken,
                // damit nachfolgende SNs im received_cache endlich geliefert
                // werden koennen. Spec §8.4.12.4.
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
        // final_flag=true nur, wenn wir wirklich alles bis base-1 haben
        // und keine weitere Writer-Aktion noetig ist. Beim preemptive
        // AckNack (base=1, leere bitmap, Proxy noch nichts gesehen)
        // muss final=false, sonst liest der Writer es als "Reader ist
        // up-to-date" und schickt keine durability-Resends (Cyclone DDS
        // zeigt dann nur HEARTBEATs, keine DATA).
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

    /// Packt `Header + INFO_DST(writer_prefix) + Submessage` in ein
    /// Datagramm. INFO_DST ist zwingend: ohne ihn ist der effektive
    /// Destination-Prefix = UNKNOWN, und Receiver (z.B. Cyclone DDS)
    /// verwerfen die Submessage als "not a connection" (RTPS 2.5 §8.3.7.6).
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
    fn in_order_data_delivered_immediately() {
        let mut r = make_reader(10);
        let w_eid = single_writer_guid().entity_id;
        let r_eid = r.guid().entity_id;
        let delivered = r.handle_data(&data(w_eid, r_eid, 1, 0xAA));
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
        assert!(r.handle_data(&data(w, rd, 2, 0x22)).is_empty());
        assert!(r.handle_data(&data(w, rd, 3, 0x33)).is_empty());
        let out = r.handle_data(&data(w, rd, 1, 0x11));
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
        r.handle_data(&data(w, rd, 1, 0xAA));
        let second = r.handle_data(&data(w, rd, 1, 0xAA));
        assert!(second.is_empty());
    }

    #[test]
    fn mismatched_writer_id_is_counted() {
        let mut r = make_reader(10);
        let rd = r.guid().entity_id;
        let foreign = EntityId::user_writer_with_key([0xFF, 0xFF, 0xFF]);
        assert!(r.handle_data(&data(foreign, rd, 1, 0xAA)).is_empty());
        assert_eq!(r.unknown_src_count(), 1);
    }

    // ---------- Wire-Side Lifecycle (T8) ----------

    #[test]
    fn alive_data_yields_alive_changekind() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let delivered = r.handle_data(&data(w, rd, 1, 0xAA));
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
        let delivered = r.handle_data(&lifecycle_data(
            w,
            rd,
            1,
            [0xAB; 16],
            crate::inline_qos::status_info::DISPOSED,
        ));
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::NotAliveDisposed);
    }

    #[test]
    fn unregister_data_yields_not_alive_unregistered() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let delivered = r.handle_data(&lifecycle_data(
            w,
            rd,
            1,
            [0xCD; 16],
            crate::inline_qos::status_info::UNREGISTERED,
        ));
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
        let delivered = r.handle_data(&lifecycle_data(w, rd, 1, [0xEF; 16], bits));
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::NotAliveDisposedUnregistered);
    }

    #[test]
    fn key_flag_without_status_info_falls_back_to_alive() {
        // key_flag=true ohne PID_STATUS_INFO ist Spec-grenzwertig — wir
        // fallen sicherheitshalber auf Alive zurueck statt zu raten.
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        let mut d = data(w, rd, 1, 0xAA);
        d.key_flag = true;
        let delivered = r.handle_data(&d);
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].kind, ChangeKind::Alive);
    }

    #[test]
    fn heartbeat_with_missing_triggers_acknack_after_delay() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        r.handle_heartbeat(&heartbeat(w, rd, 1, 3, 1, false), Duration::ZERO);
        assert!(r.tick(Duration::from_millis(100)).unwrap().is_empty());
        let out = r.tick(Duration::from_millis(250)).unwrap();
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn heartbeat_without_missing_and_final_schedules_no_acknack() {
        let mut r = make_reader(10);
        let w = single_writer_guid().entity_id;
        let rd = r.guid().entity_id;
        r.handle_data(&data(w, rd, 1, 0xAA));
        r.handle_heartbeat(&heartbeat(w, rd, 1, 1, 1, true), Duration::ZERO);
        assert!(r.tick(Duration::from_secs(10)).unwrap().is_empty());
    }

    // ---------- Multi-Writer (T4.5) ----------

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
        // Kern-Regression: beide Writer benutzen SN 1. Ohne per-Proxy-
        // State wuerde das zweite `handle_data` als Duplicate abgelehnt.
        let mut r = make_reader(10);
        add_second_writer(&mut r);
        let w1 = single_writer_guid().entity_id;
        let w2 = second_writer_guid().entity_id;
        let rd = r.guid().entity_id;

        let d1 = r.handle_data(&data(w1, rd, 1, 0xAA));
        let d2 = r.handle_data(&data(w2, rd, 1, 0xBB));

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
        // Beide Writer schicken HB mit missing-SN
        r.handle_heartbeat(
            &heartbeat(single_writer_guid().entity_id, rd, 1, 3, 1, false),
            Duration::ZERO,
        );
        r.handle_heartbeat(
            &heartbeat(second_writer_guid().entity_id, rd, 1, 5, 1, false),
            Duration::ZERO,
        );
        let out = r.tick(Duration::from_millis(250)).unwrap();
        // 2 ACKNACKs (pro Writer einen)
        assert_eq!(out.len(), 2);
    }

    // ---------- WP 1.E Stufe-B: Pre-Emptive ACKNACK ----------

    /// §8.4.2.3.4: Beim Match eines neuen Writer-Proxies sendet der
    /// Reader **proaktiv** einen ACKNACK mit `bitmap_base=1, num_bits=0,
    /// final_flag=false` — das beschleunigt den ersten Datenfluss um
    /// genau eine HB-Periode (typ. 1 s).
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
        // Werte aus add_writer_proxy: pending_acknack_since=Duration::ZERO
        // → tick(>=delay) liefert Pre-Emptive AckNack.
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

    /// Pre-Emptive ACKNACK passiert NICHT, wenn `add_writer_proxy` nie
    /// aufgerufen wurde (defensiver Sanity-Check fuer den Default-Reader).
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
        // Keine Proxies → kein ACKNACK
        assert!(r.tick(Duration::from_secs(10)).unwrap().is_empty());
    }

    /// Initiale Proxies aus `ReliableReaderConfig.writer_proxies` bekommen
    /// **kein** automatisches Pre-Emptive — nur via `add_writer_proxy`.
    /// Das ist konsistent mit der DCPS-Integration: Discovery-Layer ruft
    /// `add_writer_proxy` auf, sobald SEDP-Match steht.
    #[test]
    fn initial_proxy_from_config_does_not_send_pre_emptive() {
        // make_reader() nutzt config.writer_proxies, kein add_writer_proxy
        let mut r = make_reader(10);
        // Vor add: auch nach langem tick kein Pre-Emptive
        assert!(
            r.tick(Duration::from_secs(10)).unwrap().is_empty(),
            "initial proxy from config must not emit Pre-Emptive"
        );
    }

    #[test]
    fn pre_emptive_acknack_carries_info_dst() {
        // Pre-Emptive ACKNACK MUSS in INFO_DST(writer_prefix) gewrappt
        // sein, sonst verwerfen Cyclone/Fast-DDS die Submessage als
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
        // submessages[0] = INFO_DST (Unknown im Decoder, weil InfoDst
        // im Decoder nicht ausgepackt wird), [1] = ACKNACK
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
        r.handle_heartbeat(&heartbeat(foreign, rd, 1, 3, 1, false), Duration::ZERO);
        assert_eq!(r.unknown_src_count(), 1);
        assert!(r.tick(Duration::from_secs(1)).unwrap().is_empty());
    }
}
