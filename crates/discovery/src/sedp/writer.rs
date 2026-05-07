// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! SEDP Builtin Reliable Writers — Publications + Subscriptions.
//!
//! Wrapper um [`zerodds_rtps::reliable_writer::ReliableWriter`] mit festen
//! SEDP-EntityIds. Der Wrapper serialisiert
//! [`PublicationBuiltinTopicData`] bzw. [`SubscriptionBuiltinTopicData`]
//! via PL_CDR_LE in den Payload und legt ihn via
//! `ReliableWriter::write()` ab.
//!
//! **Multi-Reader-Proxies kommen extern rein** (via
//! [`Self::add_reader_proxy`]). T5 verdrahtet das automatisch mit SPDP-
//! Discovery.

extern crate alloc;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::error::WireError;
use zerodds_rtps::history_cache::HistoryKind;
use zerodds_rtps::message_builder::{DEFAULT_MTU, OutboundDatagram};
use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::reliable_writer::{DEFAULT_FRAGMENT_SIZE, ReliableWriter, ReliableWriterConfig};
use zerodds_rtps::submessages::NackFragSubmessage;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, SequenceNumber, VendorId};

/// Default-History-Depth fuer SEDP-Builtin-Writer. Spec §8.5.4.2 sagt
/// keep-last depth=1 fuer Discovery (jedes Topic wird einzeln announced);
/// wir nehmen konservativer 256, um Multi-Topic-Szenarien zu tragen.
pub const SEDP_DEFAULT_DEPTH: usize = 256;

/// Default-Heartbeat-Periode fuer SEDP.
///
/// Warum 100 ms statt 500 ms (alter Wert): SEDP nutzt RTPS-Reliability
/// (HEARTBEAT/ACKNACK/Resend) als einzigen Recovery-Pfad — wenn das
/// initiale DATA-Frame des `announce_subscription` auf Multicast
/// verloren geht, muss der Heartbeat-Cycle das nachholen. Bei 500 ms
/// Heartbeat + Roundtrip = ~700 ms Worst-Case zwischen DATA-Verlust
/// und Resend, plus den fixen Reader-Heartbeat-Response-Delay
/// (200 ms). Auf loaded Linux-CI-Runnern reisst das die 5-s-Match-
/// Timeouts der Late-Joiner-Tests (TS-1-Finding 9).
///
/// 100 ms gibt einen Worst-Case unter ~300 ms — sicher unter
/// Test-Timeout, vernachlaessigbarer Bandbreiten-Aufwand bei
/// Discovery (paar Hundred Byte pro Hops alle 100 ms). Production-
/// SEDP-Traffic ist selten der Bottleneck.
pub const SEDP_HEARTBEAT_PERIOD: Duration = Duration::from_millis(100);

/// Writer fuer SEDP-Publications (feste EntityId
/// [`EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER`]).
#[derive(Debug)]
pub struct SedpPublicationsWriter {
    inner: ReliableWriter,
}

/// Writer fuer SEDP-Subscriptions (feste EntityId
/// [`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER`]).
#[derive(Debug)]
pub struct SedpSubscriptionsWriter {
    inner: ReliableWriter,
}

impl SedpPublicationsWriter {
    /// Erzeugt einen SEDP-Publications-Writer fuer den gegebenen
    /// lokalen Participant. Reader-Proxies kommen separat via
    /// [`add_reader_proxy`](Self::add_reader_proxy).
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self {
            inner: make_sedp_writer(
                Guid::new(
                    participant_prefix,
                    EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
                ),
                vendor_id,
            ),
        }
    }

    /// GUID dieses Writers.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Registriert einen Remote-SEDP-Publications-Reader als Empfaenger.
    pub fn add_reader_proxy(&mut self, proxy: ReaderProxy) {
        self.inner.add_reader_proxy(proxy);
    }

    /// Entfernt einen Remote-Reader.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        self.inner.remove_reader_proxy(guid)
    }

    /// Kuendigt eine lokale Publication via SEDP an. Liefert die
    /// Datagrammliste, die der Transport an alle Reader-Proxies kippt.
    ///
    /// # Errors
    /// Encoder-Fehler (String zu lang, Cache-Fehler bei `KeepAll`-Overflow).
    pub fn announce(
        &mut self,
        p: &PublicationBuiltinTopicData,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let payload = p.to_pl_cdr_le()?;
        self.inner.write(&payload)
    }

    /// ADR-0006: kuendigt eine Publication an UND injiziert
    /// PID_SHM_LOCATOR (Vendor-PID 0x8001) ans Ende der ParameterList.
    /// Wird vom DcpsRuntime aufgerufen, wenn die Side-Map fuer den
    /// User-Writer einen Locator-Eintrag fuehrt (= Same-Host-Backend
    /// angeschlossen).
    ///
    /// # Errors
    /// Encoder-Fehler oder Inject-Fehler.
    pub fn announce_with_shm_locator(
        &mut self,
        p: &PublicationBuiltinTopicData,
        locator_bytes: &[u8],
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut payload = p.to_pl_cdr_le()?;
        zerodds_rtps::publication_data::inject_pid_shm_locator(&mut payload, locator_bytes)?;
        self.inner.write(&payload)
    }

    /// Tick (HEARTBEAT + Resends). Siehe [`ReliableWriter::tick`].
    ///
    /// # Errors
    /// Wire-Encode-Fehler.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        self.inner.tick(now)
    }

    /// Dispatch eines ACKNACK vom Remote-Reader.
    pub fn handle_acknack(
        &mut self,
        src_guid: Guid,
        base: SequenceNumber,
        requested: impl IntoIterator<Item = SequenceNumber>,
    ) {
        self.inner.handle_acknack(src_guid, base, requested);
    }

    /// Dispatch eines NACK_FRAG vom Remote-Reader.
    pub fn handle_nackfrag(&mut self, src_guid: Guid, nf: &NackFragSubmessage) {
        self.inner.handle_nackfrag(src_guid, nf);
    }

    /// Read-only-Zugriff auf den zugrundeliegenden `ReliableWriter`
    /// (Tests/Diagnose).
    #[must_use]
    pub fn inner(&self) -> &ReliableWriter {
        &self.inner
    }
}

impl SedpSubscriptionsWriter {
    /// Erzeugt einen SEDP-Subscriptions-Writer.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self {
            inner: make_sedp_writer(
                Guid::new(
                    participant_prefix,
                    EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
                ),
                vendor_id,
            ),
        }
    }

    /// GUID.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Remote-Reader hinzufuegen.
    pub fn add_reader_proxy(&mut self, proxy: ReaderProxy) {
        self.inner.add_reader_proxy(proxy);
    }

    /// Remote-Reader entfernen.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        self.inner.remove_reader_proxy(guid)
    }

    /// Kuendigt eine lokale Subscription via SEDP an.
    ///
    /// # Errors
    /// Encoder-Fehler.
    pub fn announce(
        &mut self,
        s: &SubscriptionBuiltinTopicData,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let payload = s.to_pl_cdr_le()?;
        self.inner.write(&payload)
    }

    /// Tick.
    ///
    /// # Errors
    /// Wire-Encode-Fehler.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        self.inner.tick(now)
    }

    /// Dispatch ACKNACK.
    pub fn handle_acknack(
        &mut self,
        src_guid: Guid,
        base: SequenceNumber,
        requested: impl IntoIterator<Item = SequenceNumber>,
    ) {
        self.inner.handle_acknack(src_guid, base, requested);
    }

    /// Dispatch NACK_FRAG.
    pub fn handle_nackfrag(&mut self, src_guid: Guid, nf: &NackFragSubmessage) {
        self.inner.handle_nackfrag(src_guid, nf);
    }

    /// Read-only-Zugriff.
    #[must_use]
    pub fn inner(&self) -> &ReliableWriter {
        &self.inner
    }
}

// ============================================================================
// Shared SEDP-Writer-Config
// ============================================================================

fn make_sedp_writer(guid: Guid, vendor_id: VendorId) -> ReliableWriter {
    ReliableWriter::new(ReliableWriterConfig {
        guid,
        vendor_id,
        reader_proxies: Vec::new(),
        max_samples: SEDP_DEFAULT_DEPTH,
        // KeepLast statt KeepAll: stalled Remote-SEDP-Reader darf die
        // Pipeline nicht blockieren.
        history_kind: HistoryKind::KeepLast {
            depth: SEDP_DEFAULT_DEPTH,
        },
        heartbeat_period: SEDP_HEARTBEAT_PERIOD,
        fragment_size: DEFAULT_FRAGMENT_SIZE,
        mtu: DEFAULT_MTU,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
    use zerodds_rtps::participant_data::Duration as DdsDuration;
    use zerodds_rtps::publication_data::{DurabilityKind, ReliabilityKind, ReliabilityQos};
    use zerodds_rtps::wire_types::Locator;

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
    fn writer_has_expected_guid() {
        let w = SedpPublicationsWriter::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        assert_eq!(
            w.guid().entity_id,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER
        );
    }

    #[test]
    fn announce_without_proxies_returns_no_datagrams() {
        let mut w = SedpPublicationsWriter::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        let dgs = w.announce(&sample_pub()).unwrap();
        assert!(dgs.is_empty(), "no proxies → no fan-out");
    }

    #[test]
    fn announce_with_one_proxy_produces_one_datagram_with_cdr_body() {
        let mut w = SedpPublicationsWriter::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        let remote = Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        ));
        let dgs = w.announce(&sample_pub()).unwrap();
        assert_eq!(dgs.len(), 1);
        let parsed = decode_datagram(&dgs[0].bytes).unwrap();
        let data = parsed
            .submessages
            .iter()
            .find_map(|s| {
                if let ParsedSubmessage::Data(d) = s {
                    Some(d)
                } else {
                    None
                }
            })
            .expect("DATA submessage");
        // Der Payload ist die PL_CDR_LE-enkodierte PublicationBuiltinTopicData.
        // Wir dekodieren ihn zurueck und pruefen Topic-Name.
        let decoded =
            PublicationBuiltinTopicData::from_pl_cdr_le(&data.serialized_payload).unwrap();
        assert_eq!(decoded.topic_name, "ChatterTopic");
        assert_eq!(decoded.type_name, "std_msgs::String");
        assert_eq!(data.writer_id, EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER);
    }

    #[test]
    fn subscriptions_writer_has_expected_guid() {
        let w = SedpSubscriptionsWriter::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        assert_eq!(
            w.guid().entity_id,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
        );
    }
}
