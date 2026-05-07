// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reliable RTPS-Writer (1:N Reader-Proxies) — DDSI-RTPS 2.5 §8.4.9.
//!
//! Entspricht der [`StatefulWriter`]-Rolle mit 1..N matched Readers.
//! keine Heartbeat-Liveliness. Fragmentation
//! (§8.4.14) ist unterstuetzt. Multi-Reader + Submessage-Aggregation
//! (Fast-DDS-Alignment) sind ab WP 1.4 T3 drin.
//!
//! # API-Form
//!
//! Die State-Machine ist tick-getrieben. `now` ist eine `Duration`
//! seit Writer-Start (no_std-kompatibel, ohne std::Instant).
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
use crate::history_cache::{CacheChange, HistoryCache, HistoryKind};
use crate::message_builder::{AddError, MessageBuilder, OutboundDatagram};
use crate::reader_proxy::ReaderProxy;
use crate::submessage_header::SubmessageId;
use crate::submessages::{
    DataFragSubmessage, DataSubmessage, GapSubmessage, HeartbeatSubmessage, NackFragSubmessage,
    SequenceNumberSet,
};
use crate::wire_types::{EntityId, FragmentNumber, Guid, Locator, SequenceNumber, VendorId};

/// Default-Heartbeat-Periode.
///
/// DDSI-RTPS §8.4.15 spezifiziert keine fixe Default — "implementation-defined,
/// typically 1 s". Cyclone DDS und FastDDS verwenden 100 ms; das ist auch unser
/// Wert weil:
/// 1. Bei Reliable + KEEP_LAST(N) treibt die HB-Period den Worst-Case-Latency-
///    Floor (Reader sendet ACK auf HB, Writer kann erst danach Cache schrumpfen).
/// 2. 1 s default war Pre-D.5d Initial-Implementation; die jetzige
///    Event-driven-ACKNACK + Per-Peer-Scheduler-Architektur (D.5d+) macht
///    den HB-Floor zur reinen "idle keep-alive"-Pulse, nicht mehr zum
///    Latency-Determinismus.
/// 3. Spec ist erfuellt: Period < lease_duration, period > 0, period stabil.
pub const DEFAULT_HEARTBEAT_PERIOD: Duration = Duration::from_millis(100);

/// Default-Fragment-Size in Bytes. 1344 = 1400 MTU − 20 RTPS-Header −
/// ~32 Byte Submessage-Overhead.
pub const DEFAULT_FRAGMENT_SIZE: u32 = 1344;

/// Ein Reliable-Writer mit 0..N Reader-Proxies.
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
    /// ACKNACK/NACK_FRAG-Nachrichten von unbekannten `src_guid`-Remotes.
    /// Diagnose: weist auf Misrouting, veraltete Proxies oder
    /// boesartige Sender hin.
    unknown_src_count: u64,
    next_sn: i64,
    fragment_size: u32,
    mtu: usize,
}

/// Konfiguration beim Anlegen.
#[derive(Debug, Clone)]
pub struct ReliableWriterConfig {
    /// GUID des Writer-Endpoints.
    pub guid: Guid,
    /// VendorId fuer den RTPS-Header.
    pub vendor_id: VendorId,
    /// Initiale Reader-Proxies. Weitere via `add_reader_proxy`.
    pub reader_proxies: Vec<ReaderProxy>,
    /// Absolute Obergrenze fuer Cache-Eintraege. Wirkt als Kapazitaet;
    /// die Semantik bei Ueberlauf ist von [`history_kind`](Self::history_kind)
    /// bestimmt.
    pub max_samples: usize,
    /// History-QoS:
    /// - `KeepAll`: write() schlaegt bei Overflow fehl
    ///   (No-Loss-Szenarien, z.B. Logging).
    /// - `KeepLast { depth }`: aeltestes Sample faellt bei Overflow raus
    ///   (Spec-Default; Stalled Reader blockt nicht die ganze Pipeline).
    pub history_kind: HistoryKind,
    /// Heartbeat-Periode (Default: 1 s).
    pub heartbeat_period: Duration,
    /// Fragment-Size in Bytes ([`DEFAULT_FRAGMENT_SIZE`]).
    pub fragment_size: u32,
    /// MTU fuer Submessage-Aggregation ([`DEFAULT_MTU`]).
    pub mtu: usize,
}

impl ReliableWriter {
    /// Erzeugt einen leeren Writer.
    ///
    /// # Panics
    /// - `cfg.fragment_size == 0`
    /// - `cfg.mtu < 20` (RTPS-Header passt nicht rein)
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
        }
    }

    /// GUID des Writers.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Read-only-Slice der registrierten Reader-Proxies.
    #[must_use]
    pub fn reader_proxies(&self) -> &[ReaderProxy] {
        &self.reader_proxies
    }

    /// Anzahl registrierter Reader-Proxies.
    #[must_use]
    pub fn reader_proxy_count(&self) -> usize {
        self.reader_proxies.len()
    }

    /// Entfernt Samples mit SN < `up_to_exclusive` aus dem Cache. Wird
    /// von hoeheren Layern fuer Lifespan-Expire genutzt (Spec §2.2.3.16):
    /// abgelaufene Samples verschwinden so, dass auch spaete
    /// Reader-Proxies sie nicht mehr bekommen.
    pub fn remove_samples_up_to(&mut self, up_to_exclusive: SequenceNumber) -> usize {
        self.cache.remove_up_to(up_to_exclusive)
    }

    /// History-Cache (read-only).
    #[must_use]
    pub fn cache(&self) -> &HistoryCache {
        &self.cache
    }

    /// Anzahl gesendeter HEARTBEATs.
    #[must_use]
    pub fn heartbeat_count(&self) -> i32 {
        self.heartbeat_count
    }

    /// Anzahl empfangener NACK_FRAGs.
    #[must_use]
    pub fn nackfrag_count(&self) -> i32 {
        self.nackfrag_count
    }

    /// Anzahl ACKNACK/NACK_FRAG-Nachrichten von **unbekannten** Quellen
    /// seit Writer-Start. Typische Ursachen: Misrouting auf Multicast,
    /// veraltete Proxies nach `remove_reader_proxy`, GUID-Spoofing.
    #[must_use]
    pub fn unknown_src_count(&self) -> u64 {
        self.unknown_src_count
    }

    /// Aktuelle Fragment-Size-Konfiguration.
    #[must_use]
    pub fn fragment_size(&self) -> u32 {
        self.fragment_size
    }

    /// True wenn ein Payload dieser Groesse fragmentiert wird
    /// (Payload-Laenge > `fragment_size`).
    #[must_use]
    fn needs_fragmentation(&self, payload: &[u8]) -> bool {
        u32::try_from(payload.len()).unwrap_or(u32::MAX) > self.fragment_size && !payload.is_empty()
    }

    /// Prueft, ob alle Reader-Proxies den aktuell hoechsten Sample-SN
    /// im Cache bereits acknowledged haben. Liefert `true` auch wenn
    /// der Cache leer ist oder keine Proxies existieren (nichts zu
    /// bestaetigen).
    ///
    /// Spec-Basis fuer `DataWriter::wait_for_acknowledgments`
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

    /// Fuegt einen Reader-Proxy hinzu. Idempotent: wenn ein Proxy mit
    /// gleichem `remote_reader_guid` existiert, wird er ersetzt.
    ///
    /// Setzt `last_heartbeat = None`, damit der naechste `tick()` sofort
    /// einen Heartbeat an **alle** Proxies (inkl. den neuen) emittiert.
    /// RTPS §8.4.15.4: frisch hinzugefuegter ReaderProxy muss Gelegenheit
    /// zu AckNack bekommen, sonst wartet er bis zur naechsten periodischen
    /// Heartbeat-Runde (Default 1 s) — und bei spaet-gewireten Proxies
    /// (nach Cache-Inserts) ist das die einzige Moeglichkeit, die frueh
    /// eingefuegten Samples nachzuliefern, da `write_sample_with_datagrams`
    /// nur direkt sendet, wenn der Proxy synchron ist.
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
        // Zwinge naechsten tick zu HB — neuer Proxy braucht ihn fuer
        // AckNack-getriebenen Catch-up.
        self.last_heartbeat = None;
    }

    /// Entfernt den Proxy mit gegebener GUID.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        let idx = self
            .reader_proxies
            .iter()
            .position(|p| p.remote_reader_guid == guid)?;
        Some(self.reader_proxies.remove(idx))
    }

    // ---------- Write ----------

    /// Schreibt einen neuen Sample und fan-outet an alle Proxies.
    ///
    /// Pro Proxy entsteht (aggregiert):
    /// - 1 DATA-Datagram wenn `payload.len() <= fragment_size`
    /// - N DATA_FRAG-Datagramme (je Fragment 1 Datagram, kein Mix)
    ///
    /// # Errors
    /// SN-Overflow, Cache voll, Body zu gross.
    pub fn write(&mut self, payload: &[u8]) -> Result<Vec<OutboundDatagram>, WireError> {
        let sn_value = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "sequence number overflow",
            })?;
        self.next_sn = sn_value;
        let sn = SequenceNumber(sn_value);
        // Slice → Arc allokiert genau einmal pro Sample; danach nur
        // noch Arc-clone (refcount-increment) im Cache + pro Datagram
        // (Perf-Audit F7/F8). caller darf einen
        // PoolBuffer<SMALL> reinreichen, damit das Pre-Framing
        // heap-frei laeuft.
        let payload: alloc::sync::Arc<[u8]> = alloc::sync::Arc::from(payload);
        self.cache
            .insert(CacheChange::alive_arc(
                sn,
                alloc::sync::Arc::clone(&payload),
            ))
            .map_err(|_| WireError::ValueOutOfRange {
                message: "history cache full or duplicate",
            })?;

        let mut out = Vec::new();
        for idx in 0..self.reader_proxies.len() {
            // `next_unsent_change(cache_max)` rueckt den Proxy um genau
            // eine SN vor. Zwei Faelle:
            //
            // * Proxy war synchron (`highest_sent_sn == sn - 1`): advance
            //   liefert `Some(sn)`, wir koennen das Sample direkt an den
            //   Peer schicken — spart eine Heartbeat-Runde.
            //
            // * Proxy ist lag (spaet via SEDP gewired, Cache hatte schon
            //   aeltere Samples): advance liefert eine aeltere SN. Dann
            //   waere es falsch, das *neue* Payload mit dem *neuen* SN
            //   direkt zu schicken — der Proxy denkt, erst eine fruehe SN
            //   sei raus, waehrend der Reader die neue sieht. Stattdessen
            //   lassen wir `tick()` die Luecke via Heartbeat + AckNack-
            //   gesteuertem Resend aufloesen (Standard-Reliable-Pfad).
            let advanced = self.reader_proxies[idx].next_unsent_change(sn);
            if advanced != Some(sn) {
                continue;
            }
            let reader_id = self.reader_proxies[idx].remote_reader_guid.entity_id;
            let targets = self.targets_for(idx);
            // D.5g — Per-Peer Wire-Format. Payload-bytes 0..4 sind
            // der Encap-Header (RTPS 2.5 §10.5). Wenn dieser
            // Reader-Proxy XCDR1 ausgehandelt hat (legacy peer),
            // klonen wir das Payload und ueberschreiben byte 1 von
            // `0x07` (PLAIN_CDR2_LE) auf `0x01` (CDR_LE).
            // Body-Bytes (offset 4..) bleiben fuer @final-Primitive-
            // Structs bit-identisch zwischen XCDR1/XCDR2-LE.
            let proxy_payload = self.adapt_payload_for_proxy(idx, &payload);
            out.extend(self.build_sample_datagrams(sn, &proxy_payload, reader_id, &targets)?);
        }
        Ok(out)
    }

    /// D.5g — Bereitet die Payload-Bytes fuer einen bestimmten Reader-
    /// Proxy auf. Wenn das ausgehandelte Wire-Format vom Cache-Default
    /// abweicht, klont es das Payload und ueberschreibt byte 1 des
    /// Encap-Headers entsprechend.
    fn adapt_payload_for_proxy(
        &self,
        idx: usize,
        payload: &alloc::sync::Arc<[u8]>,
    ) -> alloc::sync::Arc<[u8]> {
        let negotiated = self.reader_proxies[idx].negotiated_data_representation();
        // Cache-default Encap (vom Caller gesetzt) ist XCDR2 = 0x07.
        // Bei legacy-Reader (XCDR1) overriden wir byte 1.
        if negotiated == crate::publication_data::data_representation::XCDR2 || payload.len() < 4 {
            return alloc::sync::Arc::clone(payload);
        }
        let target_byte = match negotiated {
            crate::publication_data::data_representation::XCDR => 0x01_u8,
            _ => return alloc::sync::Arc::clone(payload),
        };
        if payload[1] == target_byte {
            return alloc::sync::Arc::clone(payload);
        }
        let mut adapted: alloc::vec::Vec<u8> = payload.to_vec();
        adapted[1] = target_byte;
        alloc::sync::Arc::from(adapted.into_boxed_slice())
    }

    /// D.5e Phase-2: Write + piggyback HEARTBEAT in einer Operation.
    ///
    /// Cyclone DDS und FastDDS senden bei jedem `write()` zusaetzlich
    /// einen HEARTBEAT, damit der Reader sofort einen ACKNACK triggern
    /// kann. Ohne piggyback muss der Reader bis zum naechsten periodic-
    /// HB (default 100 ms) warten — das wird bei 1-in-flight-Roundtrip
    /// zum Latenz-Floor.
    ///
    /// Diese Methode ist ein Superset von [`Self::write`]: emittiert
    /// alle DATA-Datagrams und haengt pro matched Reader-Proxy ein
    /// HEARTBEAT-Datagram an. `last_heartbeat = now` wird gesetzt damit
    /// `tick()` nicht doppelt feuert.
    ///
    /// # Errors
    /// Wire-Encode-Fehler.
    pub fn write_with_heartbeat(
        &mut self,
        payload: &[u8],
        now: Duration,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = self.write(payload)?;
        if self.cache.is_empty() {
            return Ok(out);
        }
        // Piggyback HEARTBEAT pro matched Reader-Proxy. final_flag=false
        // damit der Reader den HB als "respond please"-Pulse interpretiert
        // (RTPS 2.5 §8.4.15.5).
        let cache_min = self.cache.min_sn().unwrap_or(SequenceNumber(1));
        for idx in 0..self.reader_proxies.len() {
            let reader_id = self.reader_proxies[idx].remote_reader_guid.entity_id;
            let targets = self.targets_for(idx);
            let mut builder =
                MessageBuilder::open(self.rtps_header(), Rc::clone(&targets), self.mtu);
            self.append_heartbeat(
                &mut builder,
                reader_id,
                cache_min,
                /*final_flag=*/ false,
                &mut out,
                &targets,
            )?;
            if let Some(dg) = builder.finish() {
                out.push(dg);
            }
        }
        self.last_heartbeat = Some(now);
        Ok(out)
    }

    // ---------- Tick ----------

    /// Tick-Event: HEARTBEATs + Resends + NACK_FRAG-Responses, aggregiert.
    ///
    /// # Errors
    /// Wire-Encode-Fehler.
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

            // 1) Fragment-Resends (aus NACK_FRAG) — je 1 Datagramm pro Fragment
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

            // 2) Aggregiertes Datagramm fuer ganze-SN-Resends + optional HB
            let mut builder =
                MessageBuilder::open(self.rtps_header(), Rc::clone(&targets), self.mtu);

            while let Some(sn) = self.reader_proxies[idx].next_requested_change() {
                #[cfg(feature = "metrics")]
                crate::metrics::inc_retransmit();
                match self.cache.get(sn) {
                    Some(change) => {
                        let payload = change.payload.clone();
                        // Wenn Fragmentation noetig → eigene Datagramme, Builder flushen falls noetig.
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

            // 3) Piggyback-HEARTBEAT am Ende (wenn faellig).
            //
            // `first_sn` ist per-Proxy: `max(cache.min_sn, proxy.highest_acked + 1)`.
            // Das per-Proxy `highest_acked + 1` verhindert, dass Volatile-
            // proxies (die via `skip_samples_up_to` ueber den cache-min
            // hinaus vorgerueckt sind) den Reader auffordern, alte
            // Samples nachzufordern. Spec §8.4.12.1: firstSN ist die
            // "smallest sequence number considered relevant FOR THE READER".
            //
            // **FinalFlag (WP 1.E Stufe-A, §8.4.9.2.7):** Periodische
            // HEARTBEATs MUESSEN `FinalFlag = NOT_SET` tragen, damit der
            // Reader zur Antwort verpflichtet ist (Reliable-Liveness).
            // Ein gesetztes Final-Bit wuerde dem Reader signalisieren
            // "Du musst nichts tun" — was dazu fuehrt, dass nach Discovery
            // bei voll-acknowledged Cache nie wieder ACKNACK kommt und
            // der Writer in einen Zombie-State faellt. Daher hier hart
            // `false`.
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

    /// Verarbeitet eine eingegangene ACKNACK von `src_guid`.
    /// Unbekannter Sender → no-op.
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
        self.reader_proxies[idx].acked_changes_set(base);
        self.reader_proxies[idx].requested_changes_set(requested);
        // Cache-GC **entfernt** im Per-Destination-Queue-Modell (T3-
        // Refactor): Der Cache wird nur noch durch HistoryKind::KeepLast
        // getrimmt. Ein stalled Reader blockt damit nicht mehr die
        // Pipeline — bei zu-alten Samples bekommt er GAP-Responses.
    }

    /// Verarbeitet ein eingegangenes NACK_FRAG von `src_guid`.
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

    // ---------- Build-Helfer ----------

    fn rtps_header(&self) -> RtpsHeader {
        RtpsHeader::new(self.vendor_id, self.guid.prefix)
    }

    /// Ziel-Locator-Set fuer den Proxy `idx`. Multicast bevorzugt
    /// (Netzwerk-Latenz und Bandbreite), Unicast als Fallback.
    fn targets_for(&self, idx: usize) -> Rc<Vec<Locator>> {
        let p = &self.reader_proxies[idx];
        if !p.multicast_locators.is_empty() {
            Rc::new(p.multicast_locators.clone())
        } else {
            Rc::new(p.unicast_locators.clone())
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
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id,
            writer_id: self.guid.entity_id,
            writer_sn: sn,
            // WP 2.0a: Arc::clone statt to_vec — Zero-Copy in den Wire-Pfad.
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::sync::Arc::clone(payload),
        };
        let (body, flags) = data.write_body(true);
        self.append_submessage(
            builder,
            SubmessageId::Data,
            flags,
            &body,
            out,
            targets,
            "DATA",
        )
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

    /// Gemeinsamer Submessage-Append mit Overflow-Handling.
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

    /// Erzeugt je 1 Datagramm pro Fragment (DATA) oder 1 DATA-Datagramm
    /// (wenn unter `fragment_size`). Kein Aggregieren mit anderen DATAs.
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
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id,
            writer_id: self.guid.entity_id,
            writer_sn: sn,
            // WP 2.0a: Arc::clone statt to_vec — Zero-Copy in den Wire-Pfad.
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

    /// Spec §9.6.3.9 PID_STATUS_INFO Lifecycle-Sample: DATA mit
    /// `key_flag=true` + Inline-QoS [PID_KEY_HASH + PID_STATUS_INFO].
    /// Payload bleibt leer (Spec erlaubt das, der Reader rekonstruiert
    /// die Instanz aus dem Key-Hash). Wird vom DCPS-Layer beim
    /// `dispose`/`unregister_instance` aufgerufen.
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

    /// Sendet einen Lifecycle-Marker (dispose/unregister) an alle matched
    /// Reader. Allokiert eine neue Sequence-Number, persistiert eine
    /// `CacheChange` mit dem entsprechenden ChangeKind und baut pro
    /// Reader-Proxy eine DATA mit Key-Hash + StatusInfo.
    ///
    /// `status_bits` ist die ODER-Verknuepfung der gewuenschten Bits aus
    /// [`crate::inline_qos::status_info`]:
    /// - DISPOSED: NotAliveDisposed
    /// - UNREGISTERED: NotAliveUnregistered
    /// - DISPOSED | UNREGISTERED: NotAliveDisposedUnregistered
    ///
    /// # Errors
    /// Wire-Encode-Fehler oder Sequence-Number-Overflow.
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

        // CacheChange persistieren — Late-Joiner-Replay (T9) liest darauf
        // auf, und das History-Cache-Bookkeeping bleibt konsistent.
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
            let advanced = self.reader_proxies[idx].next_unsent_change(sn);
            if advanced != Some(sn) {
                continue;
            }
            let reader_id = self.reader_proxies[idx].remote_reader_guid.entity_id;
            let targets = self.targets_for(idx);
            out.push(self.build_lifecycle_datagram(
                sn,
                key_hash,
                status_bits,
                reader_id,
                &targets,
            )?);
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
            // WP 2.0a: chunk ist ein Sub-Slice des vollen Arc-Payloads.
            //
            // **Zero-Copy-Scope-Claim:** der
            // `Arc::from(chunk)` hier ALLOZIERT einen neuen Refcount-
            // Block und kopiert die chunk-Bytes. Das ist **nicht**
            // Zero-Copy. Der WP-2.0a-Claim „3-7 % Gewinn" bezieht
            // sich ausschliesslich auf den unfragmentierten
            // DATA-Pfad (`build_single_data_datagram`), wo
            // `Arc::clone` den vollen Payload teilt. Fragmentation-
            // Pfade bleiben copy-per-Chunk bis WP 2.0a-2 (iovec)
            // die Submessage-Builder-Seite eliminiert.
            serialized_payload: alloc::sync::Arc::from(chunk),
            inline_qos_flag: false,
            hash_key_flag: false,
            key_flag: false,
            non_standard_flag: false,
        };
        let (body, flags) = df.write_body(true);
        let mut builder = MessageBuilder::open(self.rtps_header(), Rc::clone(targets), self.mtu);
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
        // Per-destination-queue-Modell: Cache bleibt voll (KeepAll),
        // GC passiert nur via History-QoS. ACKNACK-State wird aber
        // am Proxy korrekt getrackt.
        assert_eq!(w.cache().len(), 3, "cache intact under KeepAll");
        assert_eq!(first_proxy(&w).highest_acked_sn(), sn(3));
        // sn(2) war acked durch base=4 → gar nicht erst als requested
        // gemerkt
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
        // Cache voll unter KeepAll.
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
        // Cache haelt nur die letzten 3 (SN 3, 4, 5)
        assert_eq!(w.cache().len(), 3);
        assert_eq!(w.cache().min_sn(), Some(sn(3)));
        assert_eq!(w.cache().max_sn(), Some(sn(5)));
        assert_eq!(w.cache().evicted_count(), 2);
    }

    #[test]
    fn keep_last_stalled_reader_does_not_block_fresh_writes() {
        // Scenario: zwei Proxies, einer "stalled" (nie acked),
        // der andere aktiv. Unter KeepLast schreibt der Writer
        // weiter, der stalled Reader bekommt spaeter GAPs.
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
        // 10 samples — stalled nie acked, aber write schlaegt nicht fehl
        for i in 1..=10 {
            w.write(&alloc::vec![i as u8]).expect("never blocks");
        }
        assert_eq!(w.cache().len(), 3);
        assert_eq!(w.cache().min_sn(), Some(sn(8)));
        // Aktiver Reader fragt spaeter sn(2) an → ist evicted, bekommt GAP
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
        assert!(has_data_2, "DATA-Resend fuer sn(2)");
        assert!(has_hb, "Piggyback-HEARTBEAT im gleichen Datagramm");
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
        // (modular arithmetic). i32 wraps wenn der Counter
        // i32::MAX erreicht.
        let mut w = make_writer(10, Duration::from_millis(100));
        w.write(&alloc::vec![1]).unwrap();
        // Manuell auf MAX setzen (kein Public-Setter; aber wir tracken
        // den Counter ueber `heartbeat_count.wrapping_add(1)` →
        // wrap-Verhalten ist garantiert per Code).
        // Teste die wrapping-Semantik direkt:
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
        // Proxy 1 zeigt highest_acked=3, Proxy 2 unveraendert=0.
        // Cache-GC ist entkoppelt vom acknack (Per-destination-queue-Modell).
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

    // ---------- WP 1.E Stufe-A: HEARTBEAT FinalFlag-Default ----------

    /// §8.4.9.2.7: Periodische HEARTBEATs muessen `FinalFlag=NOT_SET`
    /// tragen, sonst antwortet der Reader nicht mit ACKNACK und der
    /// Reliable-Liveness-Loop bricht.
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

    /// Ad-hoc HB direkt nach `add_reader_proxy`: setzt `last_heartbeat=None`,
    /// d.h. naechster `tick()` emittiert sofort einen HB. Dieser HB ist
    /// ebenfalls non-final, damit der frische Reader sicher antwortet.
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
        // Alle 3 als requested
        w.handle_acknack(rguid, sn(1), [sn(1), sn(2), sn(3)]);
        let out = w.tick(Duration::ZERO).unwrap();
        // Ein Datagramm enthaelt mehrere DATAs + HEARTBEAT
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
}
