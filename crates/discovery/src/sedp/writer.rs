// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! SEDP Builtin Reliable Writers — Publications + Subscriptions.
//!
//! Wrapper around [`zerodds_rtps::reliable_writer::ReliableWriter`] with
//! fixed SEDP EntityIds. The wrapper serializes
//! [`PublicationBuiltinTopicData`] or [`SubscriptionBuiltinTopicData`]
//! into the payload via PL_CDR_LE and stores it via
//! `ReliableWriter::write()`.
//!
//! **Multi-reader proxies come in externally** (via
//! [`SedpPublicationsWriter::add_reader_proxy`]). T5 wires this up automatically with SPDP
//! discovery.

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
use zerodds_rtps::topic_data::TopicBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, SequenceNumber, VendorId};

/// Default history depth for SEDP builtin writers. Spec §8.5.4.2 says
/// keep-last depth=1 for discovery (each topic is announced individually);
/// we use a more conservative 256 to handle multi-topic scenarios.
pub const SEDP_DEFAULT_DEPTH: usize = 256;

/// How many changes a SEDP writer holds at most: one per announced
/// endpoint or topic (DDSI-RTPS 2.5 §8.5.4.4, "a one-to-one correspondence
/// between each DataWriter in the participant and a data-object"), and the
/// disposes not every reader has acknowledged yet.
pub const SEDP_MAX_CHANGES: usize = 65_536;

/// Default heartbeat period for SEDP.
///
/// Why 100 ms instead of 500 ms (the old value): SEDP uses RTPS
/// reliability (HEARTBEAT/ACKNACK/resend) as its only recovery path — if
/// the initial DATA frame of `announce_subscription` is lost on
/// multicast, the heartbeat cycle must make up for it. At a 500 ms
/// heartbeat + roundtrip = ~700 ms worst case between DATA loss
/// and resend, plus the fixed reader heartbeat response delay
/// (200 ms). On loaded Linux CI runners this blows past the 5 s match
/// timeouts of the late-joiner tests (TS-1 finding 9).
///
/// 100 ms gives a worst case under ~300 ms — safely under the
/// test timeout, with negligible bandwidth overhead for
/// discovery (a few hundred bytes per hop every 100 ms). Production
/// SEDP traffic is rarely the bottleneck.
pub const SEDP_HEARTBEAT_PERIOD: Duration = Duration::from_millis(100);

/// Writer for SEDP publications (fixed EntityId
/// [`EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER`]).
#[derive(Debug)]
pub struct SedpPublicationsWriter {
    inner: ReliableWriter,
}

/// Writer for SEDP subscriptions (fixed EntityId
/// [`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER`]).
#[derive(Debug)]
pub struct SedpSubscriptionsWriter {
    inner: ReliableWriter,
}

impl SedpPublicationsWriter {
    /// Creates a SEDP publications writer for the given
    /// local participant. Reader proxies are added separately via
    /// [`add_reader_proxy`](Self::add_reader_proxy).
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self::with_entity(
            participant_prefix,
            vendor_id,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
        )
    }

    /// Secure variant (DDS-Security §9.4.2.7 `is_discovery_protected`):
    /// EntityId [`EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER`]. The
    /// DATA/HEARTBEAT/GAP submessages produced by the writer are additionally
    /// protected via `encode_datawriter_submessage` in the runtime send path
    /// (participant crypto) before they go to the peers' secure SEDP readers.
    #[must_use]
    pub fn new_secure(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self::with_entity(
            participant_prefix,
            vendor_id,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
        )
    }

    #[must_use]
    fn with_entity(participant_prefix: GuidPrefix, vendor_id: VendorId, entity: EntityId) -> Self {
        Self {
            inner: make_sedp_writer(Guid::new(participant_prefix, entity), vendor_id),
        }
    }

    /// GUID of this writer.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Registers a remote SEDP publications reader as a recipient.
    pub fn add_reader_proxy(&mut self, proxy: ReaderProxy) {
        self.inner.add_reader_proxy(proxy);
    }

    /// Removes a remote reader.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        self.inner.remove_reader_proxy(guid)
    }

    /// Announces a local publication via SEDP. Returns the
    /// datagram list that the transport dumps to all reader proxies.
    ///
    /// # Errors
    /// Encoder error (string too long, cache error on `KeepAll` overflow).
    pub fn announce(
        &mut self,
        p: &PublicationBuiltinTopicData,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let payload = p.to_pl_cdr_le()?;
        self.inner
            .write_keyed_stamped(&payload, None, Some(p.key.to_bytes()))
    }

    /// The sequence number of the latest sample this writer wrote.
    #[must_use]
    pub fn last_sequence_number(&self) -> SequenceNumber {
        self.inner.last_written_sn()
    }

    /// Announces the **deletion** of a local publication: an SEDP
    /// dispose+unregister keyed on the endpoint GUID (RTPS §8.5.5.3). The
    /// remote SEDP reader removes the matched writer immediately instead of
    /// waiting for a liveliness timeout — the same signal Cyclone / Fast DDS
    /// send when a `DataWriter` is deleted.
    ///
    /// The endpoint GUID goes out twice: as `PID_KEY_HASH` in the inline QoS
    /// and as the `serializedKey` behind a set K flag, which is the form
    /// Cyclone 11.0.1 and OpenDDS 3.34.0 were both measured to send. A `K=1`
    /// without that payload is what Cyclone's `validate_Data` calls
    /// malformed, discarding the whole datagram.
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn dispose(&mut self, endpoint_guid: Guid) -> Result<Vec<OutboundDatagram>, WireError> {
        let key = endpoint_guid.to_bytes();
        self.inner.write_lifecycle(
            key,
            &zerodds_rtps::parameter_list::sedp_endpoint_key_pl_cdr_le(key),
            zerodds_rtps::inline_qos::status_info::DISPOSED
                | zerodds_rtps::inline_qos::status_info::UNREGISTERED,
            None,
        )
    }

    /// ADR-0006: announces a publication AND injects
    /// PID_SHM_LOCATOR (vendor PID 0x8001) at the end of the ParameterList.
    /// Called by the DcpsRuntime when the side map carries a locator
    /// entry for the user writer (= same-host backend
    /// attached).
    ///
    /// # Errors
    /// Encoder error or inject error.
    pub fn announce_with_shm_locator(
        &mut self,
        p: &PublicationBuiltinTopicData,
        locator_bytes: &[u8],
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut payload = p.to_pl_cdr_le()?;
        zerodds_rtps::publication_data::inject_pid_shm_locator(&mut payload, locator_bytes)?;
        self.inner
            .write_keyed_stamped(&payload, None, Some(p.key.to_bytes()))
    }

    /// Tick (HEARTBEAT + resends). See [`ReliableWriter::tick`].
    ///
    /// # Errors
    /// Wire encode error.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        self.inner.tick(now)
    }

    /// Dispatch of an ACKNACK from the remote reader.
    pub fn handle_acknack(
        &mut self,
        src_guid: Guid,
        base: SequenceNumber,
        requested: impl IntoIterator<Item = SequenceNumber>,
    ) {
        self.inner.handle_acknack(src_guid, base, requested);
        // A dispose every reader has is done with.
        self.inner.remove_acknowledged_lifecycle();
    }

    /// The FinalFlag of an ACKNACK from `src_guid`; with F clear the next
    /// tick answers it with a HEARTBEAT (DDSI-RTPS 2.5 §9.4.5.3).
    pub fn note_acknack_final_flag(&mut self, src_guid: Guid, final_flag: bool) {
        self.inner.note_acknack_final_flag(src_guid, final_flag);
    }

    /// Dispatch of a NACK_FRAG from the remote reader.
    pub fn handle_nackfrag(&mut self, src_guid: Guid, nf: &NackFragSubmessage) {
        self.inner.handle_nackfrag(src_guid, nf);
    }

    /// Read-only access to the underlying `ReliableWriter`
    /// (tests/diagnostics).
    #[must_use]
    pub fn inner(&self) -> &ReliableWriter {
        &self.inner
    }

    /// Sets the participant-wide RTPS behaviour default
    /// ([`zerodds_rtps::dialect`]) — which dispose form this writer sends,
    /// and whether every datagram states its destination prefix.
    pub fn set_behaviour(&mut self, behaviour: zerodds_rtps::dialect::RtpsBehaviour) {
        self.inner
            .set_behaviour(with_directed_addressing(behaviour));
    }

    /// Sets the RTPS behaviour for one matched peer, by its SEDP reader
    /// GUID. Always explicit — nothing infers a dialect from the peer's
    /// `vendorId`.
    pub fn set_peer_behaviour(
        &mut self,
        peer: zerodds_rtps::wire_types::Guid,
        behaviour: zerodds_rtps::dialect::RtpsBehaviour,
    ) {
        self.inner
            .set_peer_behaviour(peer, with_directed_addressing(behaviour));
    }

    /// Drops one peer's override, so the participant default applies again.
    pub fn clear_peer_behaviour(&mut self, peer: zerodds_rtps::wire_types::Guid) {
        self.inner.clear_peer_behaviour(peer);
    }

    /// The behaviour in force for one matched peer.
    #[must_use]
    pub fn behaviour_for_peer(
        &self,
        peer: zerodds_rtps::wire_types::Guid,
    ) -> zerodds_rtps::dialect::RtpsBehaviour {
        self.inner.behaviour_for_peer(peer)
    }
}

impl SedpSubscriptionsWriter {
    /// Creates a SEDP subscriptions writer.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self::with_entity(
            participant_prefix,
            vendor_id,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
        )
    }

    /// Secure variant (DDS-Security §10.4.1.2.5): EntityId
    /// [`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER`].
    #[must_use]
    pub fn new_secure(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self::with_entity(
            participant_prefix,
            vendor_id,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
        )
    }

    #[must_use]
    fn with_entity(participant_prefix: GuidPrefix, vendor_id: VendorId, entity: EntityId) -> Self {
        Self {
            inner: make_sedp_writer(Guid::new(participant_prefix, entity), vendor_id),
        }
    }

    /// GUID.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Add a remote reader.
    pub fn add_reader_proxy(&mut self, proxy: ReaderProxy) {
        self.inner.add_reader_proxy(proxy);
    }

    /// Remove a remote reader.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        self.inner.remove_reader_proxy(guid)
    }

    /// Announces a local subscription via SEDP.
    ///
    /// # Errors
    /// Encoder error.
    pub fn announce(
        &mut self,
        s: &SubscriptionBuiltinTopicData,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let payload = s.to_pl_cdr_le()?;
        self.inner
            .write_keyed_stamped(&payload, None, Some(s.key.to_bytes()))
    }

    /// The sequence number of the latest sample this writer wrote.
    #[must_use]
    pub fn last_sequence_number(&self) -> SequenceNumber {
        self.inner.last_written_sn()
    }

    /// Announces the **deletion** of a local subscription — SEDP
    /// dispose+unregister keyed on the endpoint GUID (counterpart of the
    /// publications-writer `dispose`, and the one a peer sees on every
    /// `DataReader` shutdown).
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn dispose(&mut self, endpoint_guid: Guid) -> Result<Vec<OutboundDatagram>, WireError> {
        let key = endpoint_guid.to_bytes();
        self.inner.write_lifecycle(
            key,
            &zerodds_rtps::parameter_list::sedp_endpoint_key_pl_cdr_le(key),
            zerodds_rtps::inline_qos::status_info::DISPOSED
                | zerodds_rtps::inline_qos::status_info::UNREGISTERED,
            None,
        )
    }

    /// Tick.
    ///
    /// # Errors
    /// Wire encode error.
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
        // A dispose every reader has is done with.
        self.inner.remove_acknowledged_lifecycle();
    }

    /// The FinalFlag of an ACKNACK from `src_guid`; with F clear the next
    /// tick answers it with a HEARTBEAT (DDSI-RTPS 2.5 §9.4.5.3).
    pub fn note_acknack_final_flag(&mut self, src_guid: Guid, final_flag: bool) {
        self.inner.note_acknack_final_flag(src_guid, final_flag);
    }

    /// Dispatch NACK_FRAG.
    pub fn handle_nackfrag(&mut self, src_guid: Guid, nf: &NackFragSubmessage) {
        self.inner.handle_nackfrag(src_guid, nf);
    }

    /// Read-only access.
    #[must_use]
    pub fn inner(&self) -> &ReliableWriter {
        &self.inner
    }

    /// Sets the participant-wide RTPS behaviour default
    /// ([`zerodds_rtps::dialect`]) — which dispose form this writer sends,
    /// and whether every datagram states its destination prefix.
    pub fn set_behaviour(&mut self, behaviour: zerodds_rtps::dialect::RtpsBehaviour) {
        self.inner
            .set_behaviour(with_directed_addressing(behaviour));
    }

    /// Sets the RTPS behaviour for one matched peer, by its SEDP reader
    /// GUID. Always explicit — nothing infers a dialect from the peer's
    /// `vendorId`.
    pub fn set_peer_behaviour(
        &mut self,
        peer: zerodds_rtps::wire_types::Guid,
        behaviour: zerodds_rtps::dialect::RtpsBehaviour,
    ) {
        self.inner
            .set_peer_behaviour(peer, with_directed_addressing(behaviour));
    }

    /// Drops one peer's override, so the participant default applies again.
    pub fn clear_peer_behaviour(&mut self, peer: zerodds_rtps::wire_types::Guid) {
        self.inner.clear_peer_behaviour(peer);
    }

    /// The behaviour in force for one matched peer.
    #[must_use]
    pub fn behaviour_for_peer(
        &self,
        peer: zerodds_rtps::wire_types::Guid,
    ) -> zerodds_rtps::dialect::RtpsBehaviour {
        self.inner.behaviour_for_peer(peer)
    }
}

/// Writer for SEDP topics (fixed EntityId
/// [`EntityId::SEDP_BUILTIN_TOPICS_WRITER`], DDSI-RTPS 2.5 §8.5.4.2): one
/// sample per topic the participant created, keyed on the topic's GUID.
#[derive(Debug)]
pub struct SedpTopicsWriter {
    inner: ReliableWriter,
}

impl SedpTopicsWriter {
    /// Creates a SEDP topics writer.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self {
            inner: make_sedp_writer(
                Guid::new(participant_prefix, EntityId::SEDP_BUILTIN_TOPICS_WRITER),
                vendor_id,
            ),
        }
    }

    /// GUID of this writer.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Registers a remote SEDP topics reader as a recipient.
    pub fn add_reader_proxy(&mut self, proxy: ReaderProxy) {
        self.inner.add_reader_proxy(proxy);
    }

    /// Removes a remote reader.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        self.inner.remove_reader_proxy(guid)
    }

    /// Announces a local topic, or its changed QoS.
    ///
    /// # Errors
    /// Encoder error.
    pub fn announce(
        &mut self,
        t: &TopicBuiltinTopicData,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let payload = t.to_pl_cdr_le()?;
        self.inner
            .write_keyed_stamped(&payload, None, Some(t.key.to_bytes()))
    }

    /// Announces the deletion of a local topic: a dispose+unregister keyed
    /// on the topic GUID, in the form [`SedpPublicationsWriter::dispose`]
    /// sends — `PID_ENDPOINT_GUID` is the key of a topic as of an endpoint
    /// (DDSI-RTPS 2.5 Table 9.19).
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn dispose(&mut self, topic_guid: Guid) -> Result<Vec<OutboundDatagram>, WireError> {
        let key = topic_guid.to_bytes();
        self.inner.write_lifecycle(
            key,
            &zerodds_rtps::parameter_list::sedp_endpoint_key_pl_cdr_le(key),
            zerodds_rtps::inline_qos::status_info::DISPOSED
                | zerodds_rtps::inline_qos::status_info::UNREGISTERED,
            None,
        )
    }

    /// Tick (HEARTBEAT + resends). See [`ReliableWriter::tick`].
    ///
    /// # Errors
    /// Wire encode error.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        self.inner.tick(now)
    }

    /// Dispatch of an ACKNACK from the remote reader.
    pub fn handle_acknack(
        &mut self,
        src_guid: Guid,
        base: SequenceNumber,
        requested: impl IntoIterator<Item = SequenceNumber>,
    ) {
        self.inner.handle_acknack(src_guid, base, requested);
        // A dispose every reader has is done with.
        self.inner.remove_acknowledged_lifecycle();
    }

    /// The FinalFlag of an ACKNACK from `src_guid`; with F clear the next
    /// tick answers it with a HEARTBEAT (DDSI-RTPS 2.5 §9.4.5.3).
    pub fn note_acknack_final_flag(&mut self, src_guid: Guid, final_flag: bool) {
        self.inner.note_acknack_final_flag(src_guid, final_flag);
    }

    /// Dispatch of a NACK_FRAG from the remote reader.
    pub fn handle_nackfrag(&mut self, src_guid: Guid, nf: &NackFragSubmessage) {
        self.inner.handle_nackfrag(src_guid, nf);
    }

    /// Read-only access to the underlying `ReliableWriter`.
    #[must_use]
    pub fn inner(&self) -> &ReliableWriter {
        &self.inner
    }

    /// Sets the participant-wide RTPS behaviour, as
    /// [`SedpPublicationsWriter::set_behaviour`] does.
    pub fn set_behaviour(&mut self, behaviour: zerodds_rtps::dialect::RtpsBehaviour) {
        self.inner
            .set_behaviour(with_directed_addressing(behaviour));
    }

    /// Sets the RTPS behaviour for one matched peer, by its SEDP topics
    /// reader GUID.
    pub fn set_peer_behaviour(
        &mut self,
        peer: zerodds_rtps::wire_types::Guid,
        behaviour: zerodds_rtps::dialect::RtpsBehaviour,
    ) {
        self.inner
            .set_peer_behaviour(peer, with_directed_addressing(behaviour));
    }
}

// ============================================================================
// Shared SEDP writer config
// ============================================================================

/// The SEDP builtin writers always state their destination prefix.
///
/// `readerId` alone does not identify the target of a SEDP submessage: the
/// builtin EntityIds are fixed, so two participants that hold the same
/// unicast port one after another — a restarted peer reusing its
/// predecessor's participant index — present the receiver with the same
/// `readerId` on the same port. Without `INFO_DST` a HEARTBEAT built for the
/// dead predecessor's proxy is applied by the live successor: it announces
/// `first > last` (the predecessor had acked everything, so nothing is left
/// to offer), the successor takes that as "these samples are gone", acks
/// past the announcement it never received, and the writer then reports the
/// same empty range to the successor's own proxy for good. The endpoint
/// never matches and nothing on the wire says why. Measured on issue #32:
/// 632 of 634 HEARTBEATs on one port carried the dead proxy's range.
///
/// So this axis is correctness, not dialect: a caller may set every other
/// axis of [`zerodds_rtps::dialect::RtpsBehaviour`] on a SEDP writer, per
/// participant or per peer, and this one stays.
fn with_directed_addressing(
    mut behaviour: zerodds_rtps::dialect::RtpsBehaviour,
) -> zerodds_rtps::dialect::RtpsBehaviour {
    behaviour.directed_addressing = zerodds_rtps::dialect::DirectedAddressing::PrefixEveryDatagram;
    behaviour
}

fn make_sedp_writer(guid: Guid, vendor_id: VendorId) -> ReliableWriter {
    let mut w = ReliableWriter::new(ReliableWriterConfig {
        guid,
        vendor_id,
        reader_proxies: Vec::new(),
        max_samples: SEDP_MAX_CHANGES,
        // KeepLast instead of KeepAll: a stalled remote SEDP reader must
        // not block the pipeline.
        history_kind: HistoryKind::KeepLast {
            depth: SEDP_MAX_CHANGES,
        },
        heartbeat_period: SEDP_HEARTBEAT_PERIOD,
        fragment_size: DEFAULT_FRAGMENT_SIZE,
        mtu: DEFAULT_MTU,
    });
    // DDSI-RTPS 2.5 §8.5.4.4: one data-object per endpoint, keyed on its
    // GUID. A re-announcement or a dispose replaces the endpoint's earlier
    // change; with a depth across all endpoints instead, the 257th change
    // pushed the oldest endpoint's announcement out, and a participant that
    // joined later never matched that endpoint.
    w.set_cache_history(
        HistoryKind::KeepLast {
            depth: SEDP_MAX_CHANGES,
        },
        SEDP_MAX_CHANGES,
        Some(1),
    );
    // #32: emit INFO_DST(target participant prefix) before each directed SEDP
    // datagram (covers plaintext AND secure — this is the sole SEDP writer
    // constructor). A HEARTBEAT built for a dead predecessor is then rejected
    // by a restarted peer that reused the predecessor's unicast port instead of
    // being applied to that peer's own same-EntityId builtin reader.
    w.set_emit_info_dst(true);
    w
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
            vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
            key: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
            topic_name: "ChatterTopic".into(),
            type_name: "std_msgs::String".into(),
            durability: DurabilityKind::Volatile,
            durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
            reliability: ReliabilityQos {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: DdsDuration::from_secs(10),
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec::Vec::new(),
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
            group_guid: None,
            data_max_size_serialized: None,
            entity_name: None,
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
                if let ParsedSubmessage::Data(d) = &s.submessage {
                    Some(d)
                } else {
                    None
                }
            })
            .expect("DATA submessage");
        // The payload is the PL_CDR_LE-encoded PublicationBuiltinTopicData.
        // We decode it back and check the topic name.
        let decoded =
            PublicationBuiltinTopicData::from_pl_cdr_le(&data.serialized_payload).unwrap();
        assert_eq!(decoded.topic_name, "ChatterTopic");
        assert_eq!(decoded.type_name, "std_msgs::String");
        assert_eq!(data.writer_id, EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER);
    }

    /// DDSI-RTPS 2.5 §8.5.4.4: one data-object per endpoint. 300
    /// publications stay in the writer for a reader that matches after all
    /// of them; announcing one again replaces its change, and its dispose,
    /// once every reader acknowledged it, leaves the writer.
    #[test]
    fn every_endpoint_keeps_one_change_for_a_reader_that_matches_later() {
        let mut w = SedpPublicationsWriter::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        let mut first = None;
        for n in 0..300u32 {
            let mut p = sample_pub();
            p.key = Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([(n >> 16) as u8, (n >> 8) as u8, n as u8]),
            );
            first.get_or_insert(p.key);
            let _ = w.announce(&p).unwrap();
        }
        assert_eq!(w.inner().cache().len(), 300);
        let mut again = sample_pub();
        again.key = first.unwrap();
        let _ = w.announce(&again).unwrap();
        assert_eq!(w.inner().cache().len(), 300, "the re-announcement replaces");

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
        let _ = w.dispose(first.unwrap()).unwrap();
        assert_eq!(w.inner().cache().len(), 300, "the dispose replaces");
        let last = w.last_sequence_number();
        w.handle_acknack(remote, SequenceNumber(last.0 + 1), core::iter::empty());
        assert_eq!(
            w.inner().cache().len(),
            299,
            "an acknowledged dispose leaves"
        );
    }

    /// Scans a PL_CDR_LE parameter list for `pid`, returning its value.
    fn find_pid(payload: &[u8], pid: u16) -> Option<alloc::vec::Vec<u8>> {
        // 4-byte encapsulation header, then (pid: u16, len: u16, value).
        let mut i = 4usize;
        while i + 4 <= payload.len() {
            let p = u16::from_le_bytes([payload[i], payload[i + 1]]);
            let len = u16::from_le_bytes([payload[i + 2], payload[i + 3]]) as usize;
            i += 4;
            if p == 0x0001 {
                return None; // PID_SENTINEL
            }
            if i + len > payload.len() {
                return None;
            }
            if p == pid {
                return Some(payload[i..i + len].to_vec());
            }
            i += len;
        }
        None
    }

    /// zerodds-flatdata-1.0 §3.1 — a publication announced from a writer
    /// with a same-host backend carries PID_SHM_LOCATOR (vendor PID
    /// 0x8001) on the wire, and §4.3 — the same datagram still decodes as
    /// an ordinary `PublicationBuiltinTopicData`, which is what a foreign
    /// vendor's SEDP reader does with it.
    #[test]
    fn announce_with_shm_locator_puts_vendor_pid_on_the_wire() {
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
        // hostname_hash | uid | slot_count | slot_size | CDR string path.
        let locator: alloc::vec::Vec<u8> = alloc::vec![
            0xEF, 0xBE, 0xAD, 0xDE, // hostname hash
            0x64, 0x00, 0x00, 0x00, // uid 100
            0x08, 0x00, 0x00, 0x00, // slot_count 8
            0x80, 0x00, 0x00, 0x00, // slot_size 128
            0x03, 0x00, 0x00, 0x00, b'/', b'a', 0x00, 0x00,
        ];
        let dgs = w
            .announce_with_shm_locator(&sample_pub(), &locator)
            .unwrap();
        assert_eq!(dgs.len(), 1);
        let parsed = decode_datagram(&dgs[0].bytes).unwrap();
        let data = parsed
            .submessages
            .iter()
            .find_map(|s| {
                if let ParsedSubmessage::Data(d) = &s.submessage {
                    Some(d)
                } else {
                    None
                }
            })
            .expect("DATA submessage");

        let found =
            find_pid(&data.serialized_payload, 0x8001).expect("PID_SHM_LOCATOR on the wire");
        assert_eq!(found, locator, "the PID value must be the locator bytes");

        // §4.3: a decoder that does not know 0x8001 skips it — the vendor
        // PID carries no MUST_UNDERSTAND bit — and still sees the whole
        // publication.
        let decoded =
            PublicationBuiltinTopicData::from_pl_cdr_le(&data.serialized_payload).unwrap();
        assert_eq!(decoded.topic_name, "ChatterTopic");
        assert_eq!(decoded.type_name, "std_msgs::String");

        // An announcement without a locator carries no such PID.
        let mut w2 =
            SedpPublicationsWriter::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        w2.add_reader_proxy(ReaderProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        ));
        let plain = w2.announce(&sample_pub()).unwrap();
        let parsed_plain = decode_datagram(&plain[0].bytes).unwrap();
        let data_plain = parsed_plain
            .submessages
            .iter()
            .find_map(|s| {
                if let ParsedSubmessage::Data(d) = &s.submessage {
                    Some(d)
                } else {
                    None
                }
            })
            .expect("DATA submessage");
        assert!(find_pid(&data_plain.serialized_payload, 0x8001).is_none());
    }

    #[test]
    fn subscriptions_writer_has_expected_guid() {
        let w = SedpSubscriptionsWriter::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        assert_eq!(
            w.guid().entity_id,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
        );
    }

    /// Issue #32: a dialect set on a SEDP writer — participant-wide or per
    /// peer — must not take `INFO_DST` off its datagrams.
    ///
    /// `DcpsRuntime` calls `set_behaviour(config default)` right after
    /// construction, and the table's default axis is `ByEntityIdOnly`. That
    /// silently undid the `set_emit_info_dst(true)` the constructor sets, and
    /// a restarted peer reusing its predecessor's unicast port then read the
    /// predecessor's HEARTBEAT as its own: `first=2 last=1` on a writer that
    /// still held the sample, after which the endpoints never matched.
    #[test]
    fn a_dialect_never_takes_info_dst_off_a_sedp_writer() {
        use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
        use zerodds_rtps::dialect::{DirectedAddressing, RtpsBehaviour, RtpsDialect};
        let local = GuidPrefix::from_bytes([1; 12]);
        let peer_prefix = GuidPrefix::from_bytes([2; 12]);
        let peer = Guid::new(peer_prefix, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER);
        let plain = RtpsBehaviour {
            directed_addressing: DirectedAddressing::ByEntityIdOnly,
            ..RtpsDialect::Cyclone.behaviour()
        };
        for apply in [0u8, 1, 2] {
            let mut w = SedpSubscriptionsWriter::new(local, VendorId::ZERODDS);
            w.add_reader_proxy(ReaderProxy::new(
                peer,
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
                alloc::vec![],
                true,
            ));
            match apply {
                0 => {}
                1 => w.set_behaviour(plain),
                _ => w.set_peer_behaviour(peer, plain),
            }
            assert_eq!(
                w.behaviour_for_peer(peer).directed_addressing,
                DirectedAddressing::PrefixEveryDatagram,
                "apply={apply}: the SEDP axis must survive a dialect"
            );
            // Any write fills the cache; `tick` only heartbeats what it holds.
            let _ = w
                .dispose(Guid::new(local, EntityId::user_reader_with_key([1, 2, 3])))
                .expect("dispose");
            // The HEARTBEAT is the datagram issue #32 turned on: it is what a
            // restarted peer on the predecessor's port misread as its own.
            let dgs = w.tick(Duration::from_millis(500)).expect("tick");
            let hb_prefixes: alloc::vec::Vec<[u8; 12]> = dgs
                .iter()
                .filter_map(|dg| {
                    let d = decode_datagram(&dg.bytes).ok()?;
                    let mut sms = d.contents();
                    let first = sms.next()?;
                    let carries_hb = d
                        .contents()
                        .any(|sm| matches!(sm, ParsedSubmessage::Heartbeat(_)));
                    match first {
                        ParsedSubmessage::InfoDestination(p) if carries_hb => Some(*p),
                        _ => None,
                    }
                })
                .collect();
            assert_eq!(
                hb_prefixes,
                alloc::vec![peer_prefix.to_bytes()],
                "apply={apply}: the HEARTBEAT must open with INFO_DST(peer)"
            );
        }
    }

    /// A per-peer dialect set on the SEDP subscriptions writer reaches the
    /// dispose octets: Cyclone's peer gets `K=1` without `PID_KEY_HASH`,
    /// Fast DDS' peer gets `K=0` with it, out of the same `dispose` call.
    #[test]
    fn a_per_peer_dialect_reaches_the_sedp_dispose() {
        use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
        use zerodds_rtps::dialect::RtpsDialect;
        let local = GuidPrefix::from_bytes([1; 12]);
        let cyclone_peer = Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
        );
        let fastdds_peer = Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
        );
        let mut w = SedpSubscriptionsWriter::new(local, VendorId::ZERODDS);
        for p in [cyclone_peer, fastdds_peer] {
            w.add_reader_proxy(ReaderProxy::new(
                p,
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
                alloc::vec![],
                true,
            ));
        }
        w.set_peer_behaviour(cyclone_peer, RtpsDialect::Cyclone.behaviour());
        w.set_peer_behaviour(fastdds_peer, RtpsDialect::FastDds.behaviour());
        let deleted = Guid::new(local, EntityId::user_reader_with_key([1, 2, 3]));
        let dgs = w.dispose(deleted).expect("dispose");
        let forms: alloc::vec::Vec<(bool, bool)> = dgs
            .iter()
            .filter_map(|dg| {
                decode_datagram(&dg.bytes).ok()?.contents().find_map(|sm| {
                    if let ParsedSubmessage::Data(d) = sm {
                        Some((
                            d.key_flag,
                            d.inline_qos
                                .as_ref()
                                .and_then(zerodds_rtps::inline_qos::find_key_hash)
                                .is_some(),
                        ))
                    } else {
                        None
                    }
                })
            })
            .collect();
        assert!(
            forms.contains(&(true, false)),
            "Cyclone's peer must get K=1 without PID_KEY_HASH, got {forms:?}"
        );
        assert!(
            forms.contains(&(false, true)),
            "Fast DDS' peer must get K=0 with PID_KEY_HASH, got {forms:?}"
        );
    }
}
