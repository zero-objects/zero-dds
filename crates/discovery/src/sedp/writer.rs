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
//! [`Self::add_reader_proxy`]). T5 wires this up automatically with SPDP
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
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, SequenceNumber, VendorId};

/// Default history depth for SEDP builtin writers. Spec §8.5.4.2 says
/// keep-last depth=1 for discovery (each topic is announced individually);
/// we use a more conservative 256 to handle multi-topic scenarios.
pub const SEDP_DEFAULT_DEPTH: usize = 256;

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

    /// Secure variant (DDS-Security §8.4.2.4 `is_discovery_protected`):
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
        self.inner.write(&payload)
    }

    /// Announces the **deletion** of a local publication: an SEDP
    /// dispose+unregister keyed on the endpoint GUID (RTPS §8.5.5.3). The
    /// remote SEDP reader removes the matched writer immediately instead of
    /// waiting for a liveliness timeout — the same signal Cyclone / Fast DDS
    /// send when a `DataWriter` is deleted.
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn dispose(&mut self, endpoint_guid: Guid) -> Result<Vec<OutboundDatagram>, WireError> {
        self.inner.write_lifecycle(
            endpoint_guid.to_bytes(),
            zerodds_rtps::inline_qos::status_info::DISPOSED
                | zerodds_rtps::inline_qos::status_info::UNREGISTERED,
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
        self.inner.write(&payload)
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

    /// Secure variant (DDS-Security §8.4.2.4): EntityId
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
        self.inner.write(&payload)
    }

    /// Announces the **deletion** of a local subscription — SEDP
    /// dispose+unregister keyed on the endpoint GUID (counterpart of the
    /// publications-writer `dispose`).
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn dispose(&mut self, endpoint_guid: Guid) -> Result<Vec<OutboundDatagram>, WireError> {
        self.inner.write_lifecycle(
            endpoint_guid.to_bytes(),
            zerodds_rtps::inline_qos::status_info::DISPOSED
                | zerodds_rtps::inline_qos::status_info::UNREGISTERED,
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
}

// ============================================================================
// Shared SEDP writer config
// ============================================================================

fn make_sedp_writer(guid: Guid, vendor_id: VendorId) -> ReliableWriter {
    ReliableWriter::new(ReliableWriterConfig {
        guid,
        vendor_id,
        reader_proxies: Vec::new(),
        max_samples: SEDP_DEFAULT_DEPTH,
        // KeepLast instead of KeepAll: a stalled remote SEDP reader must
        // not block the pipeline.
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
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
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
        // The payload is the PL_CDR_LE-encoded PublicationBuiltinTopicData.
        // We decode it back and check the topic name.
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
