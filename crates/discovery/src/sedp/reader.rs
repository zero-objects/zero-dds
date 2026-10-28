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
use zerodds_rtps::topic_data::TopicBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, SequenceNumber, VendorId};
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
    /// Endpoints announced (or re-announced) ALIVE, fully decoded, each
    /// with the sequence number of its announcement.
    pub added: Vec<Announced<T>>,
    /// GUIDs of endpoints the remote disposed/unregistered (from the
    /// `PID_KEY_HASH` of the lifecycle marker — for SEDP the endpoint GUID).
    pub removed: Vec<Guid>,
}

/// One ALIVE announcement and the sequence number the announcing SEDP
/// writer gave it. The number orders it against the same announcement
/// handed over by an in-process peer
/// ([`crate::sedp::DiscoveredEndpointsCache::insert_publication_announced`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Announced<T> {
    /// The endpoint as announced.
    pub data: T,
    /// The writer's sequence number for it.
    pub sequence: SequenceNumber,
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

    /// Secure variant (DDS-Security §10.4.1.2.5): local reader
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

    /// Processes an incoming HEARTBEAT. A HEARTBEAT whose `first_sn`
    /// passes a hole releases the announcements waiting behind it — they
    /// are returned like the ones a DATA or GAP releases.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_heartbeat(
        &mut self,
        source_prefix: GuidPrefix,
        hb: &HeartbeatSubmessage,
        now: Duration,
    ) -> Result<SedpDelta<PublicationBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_heartbeat(source_prefix, hb, now);
        partition_publication_samples(samples)
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

    /// Secure variant (DDS-Security §10.4.1.2.5): local reader
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

    /// HEARTBEAT; returns what it releases, as
    /// [`SedpPublicationsReader::handle_heartbeat`] does.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_heartbeat(
        &mut self,
        source_prefix: GuidPrefix,
        hb: &HeartbeatSubmessage,
        now: Duration,
    ) -> Result<SedpDelta<SubscriptionBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_heartbeat(source_prefix, hb, now);
        partition_subscription_samples(samples)
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

/// Reader for SEDP topics (fixed EntityId
/// [`EntityId::SEDP_BUILTIN_TOPICS_READER`], DDSI-RTPS 2.5 §8.5.4.2).
///
/// Unlike the endpoint readers it takes the `vendorId` of the message a
/// submessage came in: Cyclone keys its topic announcements on a
/// parameter of its own, which means something only in a message Cyclone
/// sent ([`zerodds_rtps::topic_data::topic_key`]).
#[derive(Debug)]
pub struct SedpTopicsReader {
    inner: ReliableReader,
}

impl SedpTopicsReader {
    /// Creates a SEDP topics reader with no remote writer yet.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self {
            inner: sedp_reader(
                Guid::new(participant_prefix, EntityId::SEDP_BUILTIN_TOPICS_READER),
                vendor_id,
                Vec::new(),
            ),
        }
    }

    /// GUID of the reader.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Adds a remote SEDP topics writer.
    pub fn add_writer_proxy(&mut self, proxy: WriterProxy) {
        self.inner.add_writer_proxy(proxy);
    }

    /// Removes a remote writer.
    pub fn remove_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        self.inner.remove_writer_proxy(guid)
    }

    /// Processes an incoming DATA submessage from a message `sender` sent.
    ///
    /// # Errors
    /// [`SedpReaderError::InvalidPayload`] for an announcement that does not
    /// decode, or a dispose that names no topic.
    pub fn handle_data(
        &mut self,
        source_prefix: GuidPrefix,
        sender: VendorId,
        data: &DataSubmessage,
    ) -> Result<SedpDelta<TopicBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_data(source_prefix, data, None);
        partition_topic_samples(samples, sender)
    }

    /// Processes an incoming DATA_FRAG submessage.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_data_frag(
        &mut self,
        source_prefix: GuidPrefix,
        sender: VendorId,
        df: &DataFragSubmessage,
        now: Duration,
    ) -> Result<SedpDelta<TopicBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_data_frag(source_prefix, df, now, None);
        partition_topic_samples(samples, sender)
    }

    /// Processes an incoming GAP submessage.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_gap(
        &mut self,
        source_prefix: GuidPrefix,
        sender: VendorId,
        gap: &GapSubmessage,
    ) -> Result<SedpDelta<TopicBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_gap(source_prefix, gap);
        partition_topic_samples(samples, sender)
    }

    /// Processes an incoming HEARTBEAT.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_heartbeat(
        &mut self,
        source_prefix: GuidPrefix,
        sender: VendorId,
        hb: &HeartbeatSubmessage,
        now: Duration,
    ) -> Result<SedpDelta<TopicBuiltinTopicData>, SedpReaderError> {
        let samples = self.inner.handle_heartbeat(source_prefix, hb, now);
        partition_topic_samples(samples, sender)
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
    sedp_reader(reader_guid, vendor_id, alloc::vec![writer_proxy])
}

fn sedp_reader(
    reader_guid: Guid,
    vendor_id: VendorId,
    writer_proxies: Vec<WriterProxy>,
) -> ReliableReader {
    ReliableReader::new(ReliableReaderConfig {
        guid: reader_guid,
        vendor_id,
        writer_proxies,
        max_samples_per_proxy: SEDP_READER_MAX_SAMPLES,
        heartbeat_response_delay: DEFAULT_HEARTBEAT_RESPONSE_DELAY,
        assembler_caps: AssemblerCaps::default(),
    })
}

/// The endpoint a SEDP dispose names.
///
/// Two places carry it, and a peer need not use both. `PID_KEY_HASH` in the
/// inline QoS is the first — Fast DDS 3.6.1 and OpenDDS 3.34.0 send it. The
/// `serializedKey` behind a set K flag is the second: a `PL_CDR` parameter
/// list holding `PID_ENDPOINT_GUID`, which Cyclone 11.0.1 and OpenDDS send.
///
/// Cyclone sends **only** the second. Reading the key hash alone therefore
/// dropped every dispose a Cyclone peer sent, silently: the deleted endpoint
/// stayed matched until a liveliness timeout took it, and nothing said so.
/// Measured on domain 61, frame 21: inline QoS `PID_STATUS_INFO` only,
/// `serializedKey` `00 03 00 00 5a 00 10 00 <16 GUID octets> 01 00 00 00`.
fn disposed_endpoint(s: &DeliveredSample) -> Option<Guid> {
    if let Some(h) = s.key_hash {
        return Some(Guid::from_bytes(h));
    }
    // `serializedKey`: 4-octet encapsulation header + PL_CDR parameter list.
    let payload = s.payload.as_ref();
    let (encapsulation, body) = payload.split_at_checked(4)?;
    let little_endian = match encapsulation[..2] {
        [0x00, 0x03] => true,
        [0x00, 0x02] => false,
        _ => return None,
    };
    let pl = zerodds_rtps::parameter_list::ParameterList::from_bytes(body, little_endian).ok()?;
    let guid = pl.find(zerodds_rtps::parameter_list::pid::ENDPOINT_GUID)?;
    Some(Guid::from_bytes(guid.value.as_slice().try_into().ok()?))
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
            delta.added.push(Announced {
                data: PublicationBuiltinTopicData::from_pl_cdr_le(s.payload.as_ref())?,
                sequence: s.sequence_number,
            });
        } else if let Some(g) = disposed_endpoint(&s) {
            delta.removed.push(g);
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
            delta.added.push(Announced {
                data: SubscriptionBuiltinTopicData::from_pl_cdr_le(s.payload.as_ref())?,
                sequence: s.sequence_number,
            });
        } else if let Some(g) = disposed_endpoint(&s) {
            delta.removed.push(g);
        }
    }
    Ok(delta)
}

/// Topics counterpart of [`partition_publication_samples`]. A dispose
/// names its topic by `PID_KEY_HASH` or, in its serialized key, by what
/// [`zerodds_rtps::topic_data::topic_key`] reads; one that names none is an
/// error, not a sample to pass over.
fn partition_topic_samples<I>(
    samples: I,
    sender: VendorId,
) -> Result<SedpDelta<TopicBuiltinTopicData>, SedpReaderError>
where
    I: IntoIterator<Item = DeliveredSample>,
{
    let mut delta = SedpDelta::default();
    for s in samples {
        if s.kind.is_alive_kind() {
            delta.added.push(Announced {
                data: TopicBuiltinTopicData::from_pl_cdr(s.payload.as_ref(), sender)?,
                sequence: s.sequence_number,
            });
        } else {
            delta.removed.push(disposed_topic(&s, sender).ok_or(
                SedpReaderError::InvalidPayload {
                    reason: "topic dispose names no topic",
                },
            )?);
        }
    }
    Ok(delta)
}

/// The topic a SEDP topics dispose names; see [`partition_topic_samples`].
fn disposed_topic(s: &DeliveredSample, sender: VendorId) -> Option<Guid> {
    if let Some(h) = s.key_hash {
        return Some(Guid::from_bytes(h));
    }
    let (encapsulation, body) = s.payload.as_ref().split_at_checked(4)?;
    let little_endian = match encapsulation[..2] {
        [0x00, 0x03] => true,
        [0x00, 0x02] => false,
        _ => return None,
    };
    let pl = zerodds_rtps::parameter_list::ParameterList::from_bytes(body, little_endian).ok()?;
    zerodds_rtps::topic_data::topic_key(&pl, sender)
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
            vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
            key: Guid::new(
                GuidPrefix::from_bytes([2; 12]),
                EntityId::user_writer_with_key([0xAA, 0xBB, 0xCC]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
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
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let out = r
            .handle_data(GuidPrefix::from_bytes([2; 12]), &data)
            .unwrap();
        assert_eq!(out.added.len(), 1);
        assert!(out.removed.is_empty());
        assert_eq!(out.added[0].data.topic_name, "ChatterTopic");
        assert_eq!(out.added[0].data.type_name, "std_msgs::String");
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
            // Key-only dispose in Fast DDS 3.6.1's measured form: K clear,
            // no payload, the instance named by PID_KEY_HASH alone.
            data_flag: false,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::vec::Vec::new().into(),
        };
        let out = r
            .handle_data(GuidPrefix::from_bytes([2; 12]), &data)
            .unwrap();
        assert!(out.added.is_empty());
        assert_eq!(out.removed, alloc::vec![disposed]);
    }

    /// Cyclone 11.0.1's measured SEDP dispose: `K=1`, `serializedKey` =
    /// `PL_CDR_LE` + `PID_ENDPOINT_GUID` + `PID_SENTINEL`, and inline QoS
    /// holding `PID_STATUS_INFO` **and nothing else** — no `PID_KEY_HASH`.
    ///
    /// The octets below are frame 21 of a domain-61 capture, byte for byte
    /// from `writerEntityId 0x000004c2` down: reader GUID
    /// `0110d86c 05731980 1363f244 00000207`.
    ///
    /// Reading only the key hash dropped this sample and left the deleted
    /// endpoint matched until a liveliness timeout — with no error anywhere.
    #[test]
    fn handle_data_reports_a_cyclone_dispose_without_key_hash_as_removed() {
        let mut r = make_reader();
        let guid_octets = [
            0x01, 0x10, 0xd8, 0x6c, 0x05, 0x73, 0x19, 0x80, 0x13, 0x63, 0xf2, 0x44, 0x00, 0x00,
            0x02, 0x07,
        ];
        let disposed = Guid::from_bytes(guid_octets);
        let mut inline_qos = zerodds_rtps::parameter_list::ParameterList::new();
        inline_qos.push(zerodds_rtps::inline_qos::status_info_param(
            zerodds_rtps::inline_qos::status_info::DISPOSED
                | zerodds_rtps::inline_qos::status_info::UNREGISTERED,
        ));
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::UNKNOWN,
            writer_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: Some(inline_qos),
            data_flag: false,
            key_flag: true,
            non_standard_flag: false,
            serialized_payload: alloc::vec![
                0x00, 0x03, 0x00, 0x00, // PL_CDR_LE, options 0
                0x5a, 0x00, 0x10, 0x00, // PID_ENDPOINT_GUID, length 16
                0x01, 0x10, 0xd8, 0x6c, 0x05, 0x73, 0x19, 0x80, 0x13, 0x63, 0xf2, 0x44, 0x00, 0x00,
                0x02, 0x07, // the endpoint GUID
                0x01, 0x00, 0x00, 0x00, // PID_SENTINEL
            ]
            .into(),
        };
        let out = r
            .handle_data(GuidPrefix::from_bytes([2; 12]), &data)
            .unwrap();
        assert!(out.added.is_empty());
        assert_eq!(
            out.removed,
            alloc::vec![disposed],
            "a dispose that names its endpoint only in the serialized key \
             must still unmatch it"
        );
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
            data_flag: true,
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

    /// Regression for the measured HDDS 1.4.8 storm (domain 0, `tshark -V`):
    /// its SEDP publications writer re-sent a Final HEARTBEAT `[1,1]`
    /// roughly every 1.5 ms without ever delivering SN 1 — 8509 HEARTBEATs
    /// against 8510 ACKNACKs in 13 s. This reader's *default* config (no
    /// test-only override) must not echo that 1:1: the spec's
    /// heartbeatResponseDelay of 500 ms (§8.4.10.1) runs from the first
    /// HEARTBEAT that asked for a response, so one goes out per delay.
    #[test]
    fn sedp_reader_does_not_storm_against_a_writer_that_never_delivers() {
        let mut r = make_reader();
        let remote_prefix = GuidPrefix::from_bytes([2; 12]);
        let w = EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER;
        let rd = r.guid().entity_id;
        let mut acknacks = 0usize;
        for step in 0..2000i32 {
            let now = Duration::from_micros(u64::from(step.unsigned_abs()) * 650);
            let _ = r.handle_heartbeat(
                remote_prefix,
                &HeartbeatSubmessage {
                    reader_id: rd,
                    writer_id: w,
                    first_sn: SequenceNumber(1),
                    last_sn: SequenceNumber(1),
                    count: step + 1,
                    final_flag: true,
                    liveliness_flag: false,
                    group_info: None,
                },
                now,
            );
            acknacks += r.tick(now).unwrap().len();
        }
        assert!(
            acknacks < 100,
            "2000 identical HEARTBEATs over ~1.3 s must not produce a \
             near-1:1 ACKNACK echo, got {acknacks}"
        );
        assert!(
            acknacks > 0,
            "the reader must still eventually re-request SN 1, got {acknacks}"
        );
    }
}
