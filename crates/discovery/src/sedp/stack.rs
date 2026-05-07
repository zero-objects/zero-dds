// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `SedpStack` — zusammenhaengender SEDP-Endpoint-Komplex.
//!
//! Haelt die vier SEDP-Builtin-Endpoints fuer einen Participant
//! (Publications- + Subscriptions- jeweils Writer + Reader) plus einen
//! gemeinsamen [`DiscoveredEndpointsCache`], und verdrahtet sich
//! automatisch mit neu-entdeckten Remote-Participants aus SPDP.
//!
//! # Lifecycle
//!
//! ```text
//!   let mut stack = SedpStack::new(local_prefix, VendorId::ZERODDS);
//!   // Lokale Endpoints ankuendigen
//!   stack.announce_publication(&my_pub)?;
//!   // Neuer Remote-Participant per SPDP entdeckt
//!   stack.on_participant_discovered(&remote);
//!   // ... Traffic fliesst zwischen den Proxies ...
//!   // SPDP-Lease-Timeout
//!   stack.on_participant_lost(remote_prefix);
//! ```
//!
//! Der Stack exponiert keinen Transport — er liefert Datagramme als
//! [`OutboundDatagram`] zurueck, Transport-Layer ist in
//! `crates/transport`.

extern crate alloc;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
use zerodds_rtps::error::WireError;
use zerodds_rtps::message_builder::OutboundDatagram;
use zerodds_rtps::participant_data::endpoint_flag;
use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, VendorId};
use zerodds_rtps::writer_proxy::WriterProxy;

use crate::sedp::cache::DiscoveredEndpointsCache;
use crate::sedp::reader::{SedpPublicationsReader, SedpReaderError, SedpSubscriptionsReader};
use crate::sedp::writer::{SedpPublicationsWriter, SedpSubscriptionsWriter};
use crate::spdp::DiscoveredParticipant;

/// Gesammelte Events aus einem Datagramm-Dispatch. Fuer Callers, die
/// explizit auf neue Discoveries reagieren wollen (GUI, Logging).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SedpEvents {
    /// Neu empfangene Publications (bereits im Cache abgelegt).
    pub new_publications: Vec<PublicationBuiltinTopicData>,
    /// Neu empfangene Subscriptions (bereits im Cache abgelegt).
    pub new_subscriptions: Vec<SubscriptionBuiltinTopicData>,
}

impl SedpEvents {
    /// True wenn beides leer ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.new_publications.is_empty() && self.new_subscriptions.is_empty()
    }
}

/// SEDP-Stack fuer einen Participant.
#[derive(Debug)]
pub struct SedpStack {
    local_prefix: GuidPrefix,
    pub_writer: SedpPublicationsWriter,
    pub_reader: SedpPublicationsReader,
    sub_writer: SedpSubscriptionsWriter,
    sub_reader: SedpSubscriptionsReader,
    cache: DiscoveredEndpointsCache,
}

impl SedpStack {
    /// Erzeugt einen neuen Stack fuer den lokalen Participant. Alle
    /// Endpoints sind initial ohne Remote-Proxies — die werden ueber
    /// [`on_participant_discovered`](Self::on_participant_discovered)
    /// verdrahtet.
    #[must_use]
    pub fn new(local_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        // Die Reader brauchen bei `new()` einen Writer-Proxy — wir
        // setzen einen Platzhalter mit UNKNOWN-prefix, der unmittelbar
        // wieder entfernt wird (siehe `on_participant_discovered`).
        // Eine Reader-API ohne initialen Proxy wäre kosmetisch sauberer;
        // funktional ist die Sequenz "Platzhalter rein → sofort raus"
        // äquivalent.
        let placeholder = GuidPrefix::UNKNOWN;
        let mut pub_reader =
            SedpPublicationsReader::new(local_prefix, vendor_id, placeholder, Vec::new());
        let mut sub_reader =
            SedpSubscriptionsReader::new(local_prefix, vendor_id, placeholder, Vec::new());
        // Platzhalter-Proxies sofort entfernen — sie haben keine
        // Locators, wuerden aber jeden handle_data-Call mit passender
        // EntityId abfangen. Wir entfernen sie, um clean zu starten.
        let pub_placeholder = Guid::new(placeholder, EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER);
        let sub_placeholder = Guid::new(placeholder, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER);
        pub_reader.remove_writer_proxy(pub_placeholder);
        sub_reader.remove_writer_proxy(sub_placeholder);
        Self {
            local_prefix,
            pub_writer: SedpPublicationsWriter::new(local_prefix, vendor_id),
            pub_reader,
            sub_writer: SedpSubscriptionsWriter::new(local_prefix, vendor_id),
            sub_reader,
            cache: DiscoveredEndpointsCache::default(),
        }
    }

    /// Lokaler GuidPrefix dieses Stacks.
    #[must_use]
    pub fn local_prefix(&self) -> GuidPrefix {
        self.local_prefix
    }

    /// Read-only-Zugriff auf den DiscoveredEndpointsCache.
    #[must_use]
    pub fn cache(&self) -> &DiscoveredEndpointsCache {
        &self.cache
    }

    /// Mutable-Zugriff auf den Cache. Wird primaer fuer Tests
    /// (Discovery-Match injizieren ohne Live-UDP) und WP 2.5
    /// `find_topic`-Verifikation verwendet.
    pub fn cache_mut(&mut self) -> &mut DiscoveredEndpointsCache {
        &mut self.cache
    }

    /// Kuendigt eine lokale Publication an alle bereits entdeckten
    /// Remote-SEDP-Reader an.
    ///
    /// # Errors
    /// Encoder-Fehler.
    pub fn announce_publication(
        &mut self,
        p: &PublicationBuiltinTopicData,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.pub_writer.announce(p)
    }

    /// ADR-0006: Publication mit PID_SHM_LOCATOR (Vendor-PID 0x8001).
    ///
    /// # Errors
    /// Encoder-Fehler oder Inject-Fehler.
    pub fn announce_publication_with_shm_locator(
        &mut self,
        p: &PublicationBuiltinTopicData,
        locator_bytes: &[u8],
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.pub_writer.announce_with_shm_locator(p, locator_bytes)
    }

    /// Kuendigt eine lokale Subscription an.
    ///
    /// # Errors
    /// Encoder-Fehler.
    pub fn announce_subscription(
        &mut self,
        s: &SubscriptionBuiltinTopicData,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.sub_writer.announce(s)
    }

    /// Verdrahtet die SEDP-Endpoints fuer einen neu entdeckten
    /// Participant. Liest `p.data.builtin_endpoint_set` + Unicast-
    /// Locators und registriert die passenden Proxies.
    pub fn on_participant_discovered(&mut self, p: &DiscoveredParticipant) {
        let remote_prefix = p.sender_prefix;
        if remote_prefix == self.local_prefix {
            // Selbstentdeckung: kein Self-Match (Spec-gemaess).
            return;
        }
        // SEDP routet an metatraffic_unicast_locator (PID 0x0032) —
        // erst wenn der fehlt, fallback auf default_unicast_locator.
        // Cyclone/FastDDS annoncieren beide, aber strikt getrennte Rollen.
        let unicast_locators: Vec<_> = p
            .data
            .metatraffic_unicast_locator
            .or(p.data.default_unicast_locator)
            .into_iter()
            .collect();
        let flags = p.data.builtin_endpoint_set;

        // Remote hat Publications-Announcer (Writer)?
        //  → unser SedpPublicationsReader bekommt einen WriterProxy
        if flags & endpoint_flag::PUBLICATIONS_ANNOUNCER != 0 {
            self.pub_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(remote_prefix, EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }

        // Remote hat Publications-Detector (Reader)?
        //  → unser SedpPublicationsWriter bekommt einen ReaderProxy
        if flags & endpoint_flag::PUBLICATIONS_DETECTOR != 0 {
            self.pub_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(remote_prefix, EntityId::SEDP_BUILTIN_PUBLICATIONS_READER),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }

        // Subscriptions-Announcer → unser Subscriptions-Reader
        if flags & endpoint_flag::SUBSCRIPTIONS_ANNOUNCER != 0 {
            self.sub_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(remote_prefix, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }

        // Subscriptions-Detector → unser Subscriptions-Writer
        if flags & endpoint_flag::SUBSCRIPTIONS_DETECTOR != 0 {
            self.sub_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(remote_prefix, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER),
                unicast_locators,
                Vec::new(),
                true,
            ));
        }
    }

    /// Ein Participant ist verloren (SPDP-Lease-Timeout). Cascade-
    /// Cleanup: alle Proxies mit diesem Prefix raus, Cache-Eintraege
    /// loeschen. Liefert (removed_pub_proxies, removed_sub_proxies).
    pub fn on_participant_lost(&mut self, prefix: GuidPrefix) -> (usize, usize) {
        let mut removed = 0usize;
        // Writer-seitig: remote Reader entfernen
        if self
            .pub_writer
            .remove_reader_proxy(Guid::new(
                prefix,
                EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
            ))
            .is_some()
        {
            removed += 1;
        }
        if self
            .sub_writer
            .remove_reader_proxy(Guid::new(
                prefix,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
            ))
            .is_some()
        {
            removed += 1;
        }
        // Reader-seitig: remote Writer entfernen
        self.pub_reader.remove_writer_proxy(Guid::new(
            prefix,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
        ));
        self.sub_reader.remove_writer_proxy(Guid::new(
            prefix,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
        ));
        // Cache-Cleanup
        let (pubs, subs) = self.cache.on_participant_lost(prefix);
        let _ = removed; // Reader-removal-Count derzeit nicht ausgewertet
        (pubs, subs)
    }

    /// Verarbeitet ein eingehendes RTPS-Datagramm an den SEDP-Stack.
    /// Routed die Submessages nach Ziel-EntityId an den passenden
    /// Endpoint und aktualisiert den Cache. Liefert die deltaartig
    /// neu-entdeckten Samples zurueck.
    ///
    /// # Errors
    /// Wire-Decode-Fehler oder fehlerhafte PL_CDR-Payloads.
    pub fn handle_datagram(
        &mut self,
        datagram: &[u8],
        now: Duration,
    ) -> Result<SedpEvents, SedpReaderError> {
        let parsed = decode_datagram(datagram).map_err(SedpReaderError::from)?;
        let mut events = SedpEvents::default();
        for sub in parsed.submessages {
            match sub {
                ParsedSubmessage::Data(d) => {
                    if d.reader_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_READER {
                        for p in self.pub_reader.handle_data(&d)? {
                            self.cache.insert_publication(p.clone(), now);
                            events.new_publications.push(p);
                        }
                    } else if d.reader_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER {
                        for s in self.sub_reader.handle_data(&d)? {
                            self.cache.insert_subscription(s.clone(), now);
                            events.new_subscriptions.push(s);
                        }
                    }
                }
                ParsedSubmessage::DataFrag(df) => {
                    if df.reader_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_READER {
                        for p in self.pub_reader.handle_data_frag(&df, now)? {
                            self.cache.insert_publication(p.clone(), now);
                            events.new_publications.push(p);
                        }
                    } else if df.reader_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER {
                        for s in self.sub_reader.handle_data_frag(&df, now)? {
                            self.cache.insert_subscription(s.clone(), now);
                            events.new_subscriptions.push(s);
                        }
                    }
                }
                ParsedSubmessage::Heartbeat(h) => {
                    // Cyclone schickt HEARTBEATs oft mit reader_id=UNKNOWN
                    // und nutzt INFO_DST zur Adressierung — Dispatch dann
                    // nach writer_id an den passenden SEDP-Reader.
                    let to_pub = h.reader_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_READER
                        || (h.reader_id == EntityId::UNKNOWN
                            && h.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER);
                    let to_sub = h.reader_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER
                        || (h.reader_id == EntityId::UNKNOWN
                            && h.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER);
                    if to_pub {
                        self.pub_reader.handle_heartbeat(&h, now);
                    }
                    if to_sub {
                        self.sub_reader.handle_heartbeat(&h, now);
                    }
                }
                ParsedSubmessage::Gap(g) => {
                    if g.reader_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_READER {
                        for p in self.pub_reader.handle_gap(&g)? {
                            self.cache.insert_publication(p.clone(), now);
                            events.new_publications.push(p);
                        }
                    } else if g.reader_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER {
                        for s in self.sub_reader.handle_gap(&g)? {
                            self.cache.insert_subscription(s.clone(), now);
                            events.new_subscriptions.push(s);
                        }
                    }
                }
                ParsedSubmessage::AckNack(ack) => {
                    // Dispatch auf Writer anhand writer_id
                    let base = ack.reader_sn_state.bitmap_base;
                    let requested: Vec<_> = ack.reader_sn_state.iter_set().collect();
                    let src = Guid::new(parsed.header.guid_prefix, ack.reader_id);
                    if ack.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER {
                        self.pub_writer.handle_acknack(src, base, requested);
                    } else if ack.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER {
                        self.sub_writer.handle_acknack(src, base, requested);
                    }
                }
                ParsedSubmessage::NackFrag(nf) => {
                    let src = Guid::new(parsed.header.guid_prefix, nf.reader_id);
                    if nf.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER {
                        self.pub_writer.handle_nackfrag(src, &nf);
                    } else if nf.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER {
                        self.sub_writer.handle_nackfrag(src, &nf);
                    }
                }
                ParsedSubmessage::HeartbeatFrag(_)
                | ParsedSubmessage::HeaderExtension(_)
                | ParsedSubmessage::InfoSource(_)
                | ParsedSubmessage::InfoReply(_)
                | ParsedSubmessage::InfoTimestamp(_)
                | ParsedSubmessage::Unknown { .. } => {}
            }
        }
        Ok(events)
    }

    /// Tick ueber alle Endpoints. Liefert HEARTBEATs + Resends von den
    /// Writern plus ACKNACK/NACK_FRAG von den Readern.
    ///
    /// # Errors
    /// Wire-Encode-Fehler.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = Vec::new();
        out.extend(self.pub_writer.tick(now)?);
        out.extend(self.sub_writer.tick(now)?);
        // Reader-Ticks liefern ACKNACK/NACK_FRAG-Datagramme mit
        // WriterProxy-Unicast-Locators als Target (RTPS §8.4.15).
        // Erforderlich fuer Live-Interop: Cyclone wartet auf initiales
        // ACKNACK, bevor der Reliable-Writer DATA schickt.
        out.extend(self.pub_reader.tick_outbound(now)?);
        out.extend(self.sub_reader.tick_outbound(now)?);
        Ok(out)
    }

    /// Read-only-Zugriff auf die internen Endpoints (Diagnose/Tests).
    #[must_use]
    pub fn pub_writer(&self) -> &SedpPublicationsWriter {
        &self.pub_writer
    }

    /// dito.
    #[must_use]
    pub fn pub_reader(&self) -> &SedpPublicationsReader {
        &self.pub_reader
    }

    /// dito.
    #[must_use]
    pub fn sub_writer(&self) -> &SedpSubscriptionsWriter {
        &self.sub_writer
    }

    /// dito.
    #[must_use]
    pub fn sub_reader(&self) -> &SedpSubscriptionsReader {
        &self.sub_reader
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_rtps::participant_data::{
        Duration as DdsDuration, ParticipantBuiltinTopicData, endpoint_flag,
    };
    use zerodds_rtps::publication_data::{DurabilityKind, ReliabilityKind, ReliabilityQos};
    use zerodds_rtps::wire_types::{Locator, ProtocolVersion};

    fn remote_participant(prefix: GuidPrefix, endpoint_set: u32) -> DiscoveredParticipant {
        DiscoveredParticipant {
            sender_prefix: prefix,
            sender_vendor: VendorId::ZERODDS,
            data: ParticipantBuiltinTopicData {
                guid: Guid::new(prefix, EntityId::PARTICIPANT),
                protocol_version: ProtocolVersion::V2_5,
                vendor_id: VendorId::ZERODDS,
                default_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 99], 7411)),
                default_multicast_locator: None,
                metatraffic_unicast_locator: None,
                metatraffic_multicast_locator: None,
                domain_id: None,
                builtin_endpoint_set: endpoint_set,
                lease_duration: DdsDuration::from_secs(30),
                user_data: alloc::vec::Vec::new(),
                properties: Default::default(),
                identity_token: None,
                permissions_token: None,
                identity_status_token: None,
                sig_algo_info: None,
                kx_algo_info: None,
                sym_cipher_algo_info: None,
            },
        }
    }

    fn sample_pub() -> PublicationBuiltinTopicData {
        PublicationBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
            topic_name: "ChatterTopic".into(),
            type_name: "std_msgs::String".into(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: DdsDuration::from_secs(10),
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec::Vec::new(),
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
        }
    }

    #[test]
    fn new_stack_has_no_proxies() {
        let s = SedpStack::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        assert_eq!(s.pub_writer().inner().reader_proxy_count(), 0);
        assert_eq!(s.pub_reader().inner().writer_proxy_count(), 0);
        assert_eq!(s.sub_writer().inner().reader_proxy_count(), 0);
        assert_eq!(s.sub_reader().inner().writer_proxy_count(), 0);
    }

    #[test]
    fn discovered_participant_wires_all_four_endpoints_when_present() {
        let mut s = SedpStack::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        let remote_prefix = GuidPrefix::from_bytes([2; 12]);
        let flags = endpoint_flag::PUBLICATIONS_ANNOUNCER
            | endpoint_flag::PUBLICATIONS_DETECTOR
            | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER
            | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
        s.on_participant_discovered(&remote_participant(remote_prefix, flags));
        assert_eq!(s.pub_writer().inner().reader_proxy_count(), 1);
        assert_eq!(s.pub_reader().inner().writer_proxy_count(), 1);
        assert_eq!(s.sub_writer().inner().reader_proxy_count(), 1);
        assert_eq!(s.sub_reader().inner().writer_proxy_count(), 1);
    }

    #[test]
    fn partial_endpoint_set_wires_only_matching_sides() {
        let mut s = SedpStack::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        // Remote hat nur Publications-Seite (Announcer + Detector)
        let flags = endpoint_flag::PUBLICATIONS_ANNOUNCER | endpoint_flag::PUBLICATIONS_DETECTOR;
        s.on_participant_discovered(&remote_participant(GuidPrefix::from_bytes([2; 12]), flags));
        assert_eq!(s.pub_writer().inner().reader_proxy_count(), 1);
        assert_eq!(s.pub_reader().inner().writer_proxy_count(), 1);
        assert_eq!(s.sub_writer().inner().reader_proxy_count(), 0);
        assert_eq!(s.sub_reader().inner().writer_proxy_count(), 0);
    }

    #[test]
    fn self_discovery_is_ignored() {
        let mut s = SedpStack::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        let flags = endpoint_flag::PUBLICATIONS_ANNOUNCER;
        s.on_participant_discovered(&remote_participant(GuidPrefix::from_bytes([1; 12]), flags));
        assert_eq!(s.pub_reader().inner().writer_proxy_count(), 0);
    }

    #[test]
    fn on_participant_lost_clears_proxies_and_cache() {
        let mut s = SedpStack::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        let remote_prefix = GuidPrefix::from_bytes([2; 12]);
        let flags = endpoint_flag::PUBLICATIONS_ANNOUNCER
            | endpoint_flag::PUBLICATIONS_DETECTOR
            | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER
            | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
        s.on_participant_discovered(&remote_participant(remote_prefix, flags));
        // Vollen Zustand bestaetigen
        assert_eq!(s.pub_writer().inner().reader_proxy_count(), 1);

        s.on_participant_lost(remote_prefix);

        assert_eq!(s.pub_writer().inner().reader_proxy_count(), 0);
        assert_eq!(s.pub_reader().inner().writer_proxy_count(), 0);
        assert_eq!(s.sub_writer().inner().reader_proxy_count(), 0);
        assert_eq!(s.sub_reader().inner().writer_proxy_count(), 0);
    }

    #[test]
    fn end_to_end_discovery_between_two_stacks() {
        // Zwei Stacks auf zwei verschiedenen Prefixes, beide announcen
        // Publications. Nach on_participant_discovered + handle_datagram
        // sehen beide die jeweils fremden Publications im Cache.
        let prefix_a = GuidPrefix::from_bytes([1; 12]);
        let prefix_b = GuidPrefix::from_bytes([2; 12]);
        let flags = endpoint_flag::PUBLICATIONS_ANNOUNCER
            | endpoint_flag::PUBLICATIONS_DETECTOR
            | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER
            | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
        let mut a = SedpStack::new(prefix_a, VendorId::ZERODDS);
        let mut b = SedpStack::new(prefix_b, VendorId::ZERODDS);
        a.on_participant_discovered(&remote_participant(prefix_b, flags));
        b.on_participant_discovered(&remote_participant(prefix_a, flags));

        let now = Duration::from_secs(1);

        // A announces
        let mut pub_a = sample_pub();
        pub_a.key = Guid::new(prefix_a, EntityId::user_writer_with_key([1, 0, 0]));
        pub_a.participant_key = Guid::new(prefix_a, EntityId::PARTICIPANT);
        pub_a.topic_name = "TopicA".into();
        for dg in a.announce_publication(&pub_a).unwrap() {
            let events = b.handle_datagram(&dg.bytes, now).unwrap();
            assert!(!events.is_empty());
            assert_eq!(events.new_publications[0].topic_name, "TopicA");
        }

        // B announces
        let mut pub_b = sample_pub();
        pub_b.key = Guid::new(prefix_b, EntityId::user_writer_with_key([2, 0, 0]));
        pub_b.participant_key = Guid::new(prefix_b, EntityId::PARTICIPANT);
        pub_b.topic_name = "TopicB".into();
        for dg in b.announce_publication(&pub_b).unwrap() {
            let events = a.handle_datagram(&dg.bytes, now).unwrap();
            assert!(!events.is_empty());
            assert_eq!(events.new_publications[0].topic_name, "TopicB");
        }

        assert_eq!(a.cache().publications_len(), 1);
        assert_eq!(b.cache().publications_len(), 1);
        assert_eq!(
            a.cache().publications().next().unwrap().data.topic_name,
            "TopicB"
        );
        assert_eq!(
            b.cache().publications().next().unwrap().data.topic_name,
            "TopicA"
        );
    }
}
