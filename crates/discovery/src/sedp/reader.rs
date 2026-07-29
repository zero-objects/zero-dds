// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! SEDP Builtin Reliable Readers — Publications + Subscriptions.
//!
//! Wrapper around [`zerodds_rtps::reliable_reader::ReliableReader`] with
//! fixed SEDP EntityIds. The wrapper deserializes incoming DATA payloads
//! as PL_CDR_LE-encoded [`PublicationBuiltinTopicData`] or
//! [`SubscriptionBuiltinTopicData`] and returns them as typed
//! samples.
//!
//! **Multi-writer support** (since WP 1.4 T4.5): the internal
//! `ReliableReader` can track multiple remote writers in parallel via
//! `add_writer_proxy` — discovery of N remote participants runs over the
//! same SEDP reader.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::error::WireError;
use zerodds_rtps::fragment_assembler::AssemblerCaps;
use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::reliable_reader::{
    DEFAULT_HEARTBEAT_RESPONSE_DELAY, DeliveredSample, ReliableReader, ReliableReaderConfig,
};
use zerodds_rtps::submessages::{
    DataFragSubmessage, DataSubmessage, GapSubmessage, HeartbeatSubmessage,
};
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, VendorId};
use zerodds_rtps::writer_proxy::WriterProxy;

/// Default capacity for the reader receive cache.
pub const SEDP_READER_MAX_SAMPLES: usize = 256;

/// SEDP reader error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SedpReaderError {
    /// PL_CDR decode of the payload failed — invalid SEDP
    /// announcement. Carries a static reason.
    InvalidPayload {
        /// Reason (e.g. "TOPIC_NAME missing").
        reason: &'static str,
    },
    /// Wire-level error in the ReliableReader (e.g. encode overflow).
    Wire(WireError),
}

impl From<WireError> for SedpReaderError {
    fn from(e: WireError) -> Self {
        match e {
            WireError::ValueOutOfRange { message } => Self::InvalidPayload { reason: message },
            other => Self::Wire(other),
        }
    }
}

/// Outcome of processing an incoming SEDP submessage.
///
/// SEDP carries endpoint lifecycle just like any keyed topic (DDSI-RTPS
/// §8.5.4): an ALIVE sample announces (or updates) a remote endpoint, while a
/// `dispose`/`unregister` marker — sent when the remote participant deletes the
/// endpoint — says "forget it". The two are split here so the discovery layer
/// can both *match* new endpoints (`added`) and *unmatch* gone ones
/// (`removed`). Before this split the readers decoded every sample as a full
/// announcement, which (a) ignored disposes entirely and (b) errored the whole
/// datagram when a vendor sent a key-only dispose payload that is not a valid
/// `PL_CDR_LE` `*BuiltinTopicData`.
#[derive(Debug)]
pub struct SedpDelta<T> {
    /// Endpoints announced (or re-announced) ALIVE, fully decoded.
    pub added: Vec<T>,
    /// GUIDs of endpoints the remote disposed/unregistered (from the
    /// `PID_KEY_HASH` of the lifecycle marker — for SEDP the endpoint GUID).
    pub removed: Vec<Guid>,
}

impl<T> Default for SedpDelta<T> {
    fn default() -> Self {
        Self {
            added: Vec::new(),
            removed: Vec::new(),
        }
    }
}

impl<T> SedpDelta<T> {
    /// `true` if neither an announcement nor a disposal was carried.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

/// Reader for SEDP publications (fixed EntityId
/// [`EntityId::SEDP_BUILTIN_PUBLICATIONS_READER`]).
#[derive(Debug)]
pub struct SedpPublicationsReader {
    inner: ReliableReader,
}

/// Reader for SEDP subscriptions (fixed EntityId
/// [`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER`]).
#[derive(Debug)]
pub struct SedpSubscriptionsReader {
    inner: ReliableReader,
}

impl SedpPublicationsReader {
    /// Creates a SEDP publications reader for the local
    /// participant. The `remote_writer_guid` must be known — in
    /// phase 1 a placeholder proxy is built with `remote_prefix` +
    /// [`EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER`].
    #[must_use]
    pub fn new(
        participant_prefix: GuidPrefix,
        vendor_id: VendorId,
        remote_writer_prefix: GuidPrefix,
        remote_metatraffic_unicast: Vec<zerodds_rtps::wire_types::Locator>,
    ) -> Self {
        Self::with_entities(
            participant_prefix,
            vendor_id,
            remote_writer_prefix,
            remote_metatraffic_unicast,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
        )
    }

    /// Secure variant (DDS-Security §8.4.2.4): local reader
    /// [`EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER`], remote writer
    /// [`EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER`]. Incoming
    /// DATA are decrypted via `decode_datawriter_submessage` earlier in
    /// the runtime receive path (participant crypto).
    #[must_use]
    pub fn new_secure(
        participant_prefix: GuidPrefix,
        vendor_id: VendorId,
        remote_writer_prefix: GuidPrefix,
        remote_metatraffic_unicast: Vec<zerodds_rtps::wire_types::Locator>,
    ) -> Self {
        Self::with_entities(
            participant_prefix,
            vendor_id,
            remote_writer_prefix,
            remote_metatraffic_unicast,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
        )
    }

    #[must_use]
    fn with_entities(
        participant_prefix: GuidPrefix,
        vendor_id: VendorId,
        remote_writer_prefix: GuidPrefix,
        remote_metatraffic_unicast: Vec<zerodds_rtps::wire_types::Locator>,
        reader_entity: EntityId,
        remote_writer_entity: EntityId,
    ) -> Self {
        let reader_guid = Guid::new(participant_prefix, reader_entity);
        let remote_writer_guid = Guid::new(remote_writer_prefix, remote_writer_entity);
        Self {
            inner: make_sedp_reader(
                reader_guid,
                vendor_id,
                remote_writer_guid,
                remote_metatraffic_unicast,
            ),
        }
    }

    /// GUID of the reader.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Adds another remote SEDP writer as a proxy
    /// (multi-participant discovery).
    pub fn add_writer_proxy(&mut self, proxy: WriterProxy) {
        self.inner.add_writer_proxy(proxy);
    }

    /// Removes a remote writer (e.g. after SPDP lease timeout).
    pub fn remove_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        self.inner.remove_writer_proxy(guid)
    }

    /// Processes an incoming DATA submessage and returns the
    /// deserialized [`PublicationBuiltinTopicData`] samples
    /// (in SN order).
    ///
    /// # Errors
    /// [`SedpReaderError::InvalidPayload`] if the payload is not
    /// valid PL_CDR_LE.
    pub fn handle_data(
        &mut self,
        source_prefix: GuidPrefix,
        data: &DataSubmessage,
    ) -> Result<SedpDelta<PublicationBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_data(source_prefix, data, None);
        partition_publication_samples(samples)
    }

    /// Processes an incoming DATA_FRAG submessage.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_data_frag(
        &mut self,
        source_prefix: GuidPrefix,
        df: &DataFragSubmessage,
        now: Duration,
    ) -> Result<SedpDelta<PublicationBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_data_frag(source_prefix, df, now, None);
        partition_publication_samples(samples)
    }

    /// Processes an incoming GAP submessage (returns any
    /// follow-on samples that can now be delivered in order).
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_gap(
        &mut self,
        source_prefix: GuidPrefix,
        gap: &GapSubmessage,
    ) -> Result<SedpDelta<PublicationBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_gap(source_prefix, gap);
        partition_publication_samples(samples)
    }

    /// Processes an incoming HEARTBEAT.
    pub fn handle_heartbeat(
        &mut self,
        source_prefix: GuidPrefix,
        hb: &HeartbeatSubmessage,
        now: Duration,
    ) {
        self.inner.handle_heartbeat(source_prefix, hb, now);
    }

    /// Tick — returns ACKNACK/NACK_FRAG datagrams when due.
    ///
    /// # Errors
    /// Wire encode error.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<Vec<u8>>, WireError> {
        self.inner.tick(now)
    }

    /// Tick with target locators per datagram.
    ///
    /// # Errors
    /// Wire encode error.
    pub fn tick_outbound(
        &mut self,
        now: Duration,
    ) -> Result<Vec<zerodds_rtps::message_builder::OutboundDatagram>, WireError> {
        self.inner.tick_outbound(now)
    }

    /// Read-only access.
    #[must_use]
    pub fn inner(&self) -> &ReliableReader {
        &self.inner
    }
}

impl SedpSubscriptionsReader {
    /// Creates a SEDP subscriptions reader.
    #[must_use]
    pub fn new(
        participant_prefix: GuidPrefix,
        vendor_id: VendorId,
        remote_writer_prefix: GuidPrefix,
        remote_metatraffic_unicast: Vec<zerodds_rtps::wire_types::Locator>,
    ) -> Self {
        Self::with_entities(
            participant_prefix,
            vendor_id,
            remote_writer_prefix,
            remote_metatraffic_unicast,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
        )
    }

    /// Secure variant (DDS-Security §8.4.2.4): local reader
    /// [`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER`], remote writer
    /// [`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER`].
    #[must_use]
    pub fn new_secure(
        participant_prefix: GuidPrefix,
        vendor_id: VendorId,
        remote_writer_prefix: GuidPrefix,
        remote_metatraffic_unicast: Vec<zerodds_rtps::wire_types::Locator>,
    ) -> Self {
        Self::with_entities(
            participant_prefix,
            vendor_id,
            remote_writer_prefix,
            remote_metatraffic_unicast,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
        )
    }

    #[must_use]
    fn with_entities(
        participant_prefix: GuidPrefix,
        vendor_id: VendorId,
        remote_writer_prefix: GuidPrefix,
        remote_metatraffic_unicast: Vec<zerodds_rtps::wire_types::Locator>,
        reader_entity: EntityId,
        remote_writer_entity: EntityId,
    ) -> Self {
        let reader_guid = Guid::new(participant_prefix, reader_entity);
        let remote_writer_guid = Guid::new(remote_writer_prefix, remote_writer_entity);
        Self {
            inner: make_sedp_reader(
                reader_guid,
                vendor_id,
                remote_writer_guid,
                remote_metatraffic_unicast,
            ),
        }
    }

    /// GUID.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Adds another remote SEDP writer as a proxy.
    pub fn add_writer_proxy(&mut self, proxy: WriterProxy) {
        self.inner.add_writer_proxy(proxy);
    }

    /// Removes a remote writer.
    pub fn remove_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        self.inner.remove_writer_proxy(guid)
    }

    /// Processes a DATA submessage.
    ///
    /// # Errors
    /// see [`SedpPublicationsReader::handle_data`].
    pub fn handle_data(
        &mut self,
        source_prefix: GuidPrefix,
        data: &DataSubmessage,
    ) -> Result<SedpDelta<SubscriptionBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_data(source_prefix, data, None);
        partition_subscription_samples(samples)
    }

    /// DATA_FRAG.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_data_frag(
        &mut self,
        source_prefix: GuidPrefix,
        df: &DataFragSubmessage,
        now: Duration,
    ) -> Result<SedpDelta<SubscriptionBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_data_frag(source_prefix, df, now, None);
        partition_subscription_samples(samples)
    }

    /// GAP.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_gap(
        &mut self,
        source_prefix: GuidPrefix,
        gap: &GapSubmessage,
    ) -> Result<SedpDelta<SubscriptionBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_gap(source_prefix, gap);
        partition_subscription_samples(samples)
    }

    /// HEARTBEAT.
    pub fn handle_heartbeat(
        &mut self,
        source_prefix: GuidPrefix,
        hb: &HeartbeatSubmessage,
        now: Duration,
    ) {
        self.inner.handle_heartbeat(source_prefix, hb, now);
    }

    /// Tick.
    ///
    /// # Errors
    /// Wire encode error.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<Vec<u8>>, WireError> {
        self.inner.tick(now)
    }

    /// Tick with target locators.
    ///
    /// # Errors
    /// Wire encode error.
    pub fn tick_outbound(
        &mut self,
        now: Duration,
    ) -> Result<Vec<zerodds_rtps::message_builder::OutboundDatagram>, WireError> {
        self.inner.tick_outbound(now)
    }

    /// Read-only access.
    #[must_use]
    pub fn inner(&self) -> &ReliableReader {
        &self.inner
    }
}

// ============================================================================
// Shared SEDP reader helpers
// ============================================================================

fn make_sedp_reader(
    reader_guid: Guid,
    vendor_id: VendorId,
    remote_writer_guid: Guid,
    remote_metatraffic_unicast: Vec<zerodds_rtps::wire_types::Locator>,
) -> ReliableReader {
    let writer_proxy = WriterProxy::new(
        remote_writer_guid,
        remote_metatraffic_unicast,
        Vec::new(),
        true,
    );
    ReliableReader::new(ReliableReaderConfig {
        guid: reader_guid,
        vendor_id,
        writer_proxies: alloc::vec![writer_proxy],
        max_samples_per_proxy: SEDP_READER_MAX_SAMPLES,
        heartbeat_response_delay: DEFAULT_HEARTBEAT_RESPONSE_DELAY,
        assembler_caps: AssemblerCaps::default(),
    })
}

/// Splits delivered SEDP samples into ALIVE announcements (decoded) and
/// dispose/unregister markers (collected by endpoint GUID). The decode is
/// gated on `is_alive_kind` so a key-only lifecycle payload — which is *not* a
/// valid `PL_CDR_LE` `PublicationBuiltinTopicData` — never errors the datagram.
fn partition_publication_samples<I>(
    samples: I,
) -> Result<SedpDelta<PublicationBuiltinTopicData>, SedpReaderError>
where
    I: IntoIterator<Item = DeliveredSample>,
{
    let mut delta = SedpDelta::default();
    for s in samples {
        if s.kind.is_alive_kind() {
            delta
                .added
                .push(PublicationBuiltinTopicData::from_pl_cdr_le(
                    s.payload.as_ref(),
                )?);
        } else if let Some(h) = s.key_hash {
            delta.removed.push(Guid::from_bytes(h));
        }
    }
    Ok(delta)
}

/// Subscription counterpart of [`partition_publication_samples`].
fn partition_subscription_samples<I>(
    samples: I,
) -> Result<SedpDelta<SubscriptionBuiltinTopicData>, SedpReaderError>
where
    I: IntoIterator<Item = DeliveredSample>,
{
    let mut delta = SedpDelta::default();
    for s in samples {
        if s.kind.is_alive_kind() {
            delta
                .added
                .push(SubscriptionBuiltinTopicData::from_pl_cdr_le(
                    s.payload.as_ref(),
                )?);
        } else if let Some(h) = s.key_hash {
            delta.removed.push(Guid::from_bytes(h));
        }
    }
    Ok(delta)
}

// Reference String to avoid an unused-import warning (for
// SedpReaderError::InvalidPayload.reason = String-ish).
#[allow(dead_code)]
const _: Option<String> = None;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_rtps::participant_data::Duration as DdsDuration;
    use zerodds_rtps::publication_data::{DurabilityKind, ReliabilityKind, ReliabilityQos};
    use zerodds_rtps::submessages::DataSubmessage;
    use zerodds_rtps::wire_types::{Locator, SequenceNumber};

    fn sample_pub() -> PublicationBuiltinTopicData {
        PublicationBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([2; 12]),
                EntityId::user_writer_with_key([0xAA, 0xBB, 0xCC]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
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
            presentation: zerodds_qos::PresentationQosPolicy::default(),
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

    fn make_reader() -> SedpPublicationsReader {
        SedpPublicationsReader::new(
            GuidPrefix::from_bytes([1; 12]),
            VendorId::ZERODDS,
            GuidPrefix::from_bytes([2; 12]),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
        )
    }

    #[test]
    fn reader_has_expected_guid() {
        let r = make_reader();
        assert_eq!(
            r.guid().entity_id,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_READER
        );
    }

    #[test]
    fn handle_data_decodes_publication_payload() {
        let mut r = make_reader();
        let p = sample_pub();
        let payload = p.to_pl_cdr_le().unwrap();
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
            writer_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let out = r
            .handle_data(GuidPrefix::from_bytes([2; 12]), &data)
            .unwrap();
        assert_eq!(out.added.len(), 1);
        assert!(out.removed.is_empty());
        assert_eq!(out.added[0].topic_name, "ChatterTopic");
        assert_eq!(out.added[0].type_name, "std_msgs::String");
    }

    #[test]
    fn handle_data_reports_dispose_as_removed() {
        let mut r = make_reader();
        // The disposed endpoint's GUID is carried in PID_KEY_HASH; SEDP uses
        // the endpoint GUID as the instance key.
        let disposed = Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_writer_with_key([0xAA, 0xBB, 0xCC]),
        );
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
            writer_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: Some(zerodds_rtps::inline_qos::lifecycle_inline_qos(
                disposed.to_bytes(),
                zerodds_rtps::inline_qos::status_info::DISPOSED
                    | zerodds_rtps::inline_qos::status_info::UNREGISTERED,
            )),
            // Key-only dispose: no valid PL_CDR BuiltinTopicData payload.
            key_flag: true,
            non_standard_flag: false,
            serialized_payload: alloc::vec::Vec::new().into(),
        };
        let out = r
            .handle_data(GuidPrefix::from_bytes([2; 12]), &data)
            .unwrap();
        assert!(out.added.is_empty());
        assert_eq!(out.removed, alloc::vec![disposed]);
    }

    #[test]
    fn handle_data_rejects_invalid_payload() {
        let mut r = make_reader();
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
            writer_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
            writer_sn: SequenceNumber(1),
            // Encapsulation + malformed ParameterList
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::vec![0x00, 0x03, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF].into(),
        };
        let res = r.handle_data(GuidPrefix::from_bytes([2; 12]), &data);
        assert!(matches!(res, Err(SedpReaderError::InvalidPayload { .. })));
    }

    #[test]
    fn subscriptions_reader_has_expected_guid() {
        let r = SedpSubscriptionsReader::new(
            GuidPrefix::from_bytes([1; 12]),
            VendorId::ZERODDS,
            GuidPrefix::from_bytes([2; 12]),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
        );
        assert_eq!(
            r.guid().entity_id,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER
        );
    }
}
