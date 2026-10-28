// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `SedpStack` — cohesive SEDP endpoint complex.
//!
//! Holds the four SEDP builtin endpoints for a participant
//! (publications + subscriptions, each writer + reader) plus a
//! shared [`DiscoveredEndpointsCache`], and wires itself up
//! automatically with newly discovered remote participants from SPDP.
//!
//! # Lifecycle
//!
//! ```text
//!   let mut stack = SedpStack::new(local_prefix, VendorId::ZERODDS);
//!   // Announce local endpoints
//!   stack.announce_publication(&my_pub)?;
//!   // New remote participant discovered via SPDP
//!   stack.on_participant_discovered(&remote);
//!   // ... traffic flows between the proxies ...
//!   // SPDP lease timeout
//!   stack.on_participant_lost(remote_prefix);
//! ```
//!
//! The stack does not expose a transport — it returns datagrams as
//! [`OutboundDatagram`]; the transport layer lives in
//! `crates/transport`.

extern crate alloc;
use alloc::vec::Vec;
use core::time::Duration;

use alloc::collections::BTreeMap;
use zerodds_rtps::datagram::{ParsedSubmessage, ReceiveContext, decode_datagram};
use zerodds_rtps::error::WireError;
use zerodds_rtps::message_builder::OutboundDatagram;
use zerodds_rtps::participant_data::endpoint_flag;
use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::reader_proxy::ReaderProxy;

use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::topic_data::TopicBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, VendorId};
use zerodds_rtps::writer_proxy::WriterProxy;

use crate::sedp::cache::DiscoveredEndpointsCache;
use crate::sedp::reader::{
    SedpDelta, SedpPublicationsReader, SedpReaderError, SedpSubscriptionsReader, SedpTopicsReader,
};
use crate::sedp::writer::{SedpPublicationsWriter, SedpSubscriptionsWriter, SedpTopicsWriter};
use crate::spdp::DiscoveredParticipant;

/// Collected events from a datagram dispatch. For callers that want
/// to react explicitly to new discoveries (GUI, logging).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SedpEvents {
    /// Newly received publications (already stored in the cache).
    pub new_publications: Vec<PublicationBuiltinTopicData>,
    /// Newly received subscriptions (already stored in the cache).
    pub new_subscriptions: Vec<SubscriptionBuiltinTopicData>,
    /// GUIDs of remote publications that were disposed/unregistered (the
    /// remote deleted its DataWriter). Already removed from the cache; the
    /// runtime unmatches them from local readers. DDSI-RTPS §8.5.4.
    pub removed_publications: Vec<Guid>,
    /// GUIDs of remote subscriptions that were disposed/unregistered.
    pub removed_subscriptions: Vec<Guid>,
    /// Endpoint GUIDs (`source_prefix` + `writer_id`) a SEDP DATA named as
    /// its writer, for which none of the four SEDP readers held a writer
    /// proxy — the announcement could not be absorbed into `new_publications`
    /// / `new_subscriptions` at all, and nothing else in [`SedpEvents`] shows
    /// that it arrived.
    ///
    /// Measured against OpenSplice 6.9: its DDSI2 network service bundles
    /// several of its own participants into one datagram and switches the
    /// effective source mid-datagram via INFO_SRC (§8.3.4). A participant
    /// this stack never ran [`SedpStack::on_participant_discovered`] for —
    /// individually, by its own SPDP GUID, not the bundling service's — has
    /// no writer proxy under that GUID, so its SEDP announcements land here
    /// unmatched instead of in `new_publications`/`new_subscriptions`. This
    /// is a `unknown participant` refusal from *this* stack's own
    /// perspective, not the RxO/partition/type match reasons the DCPS layer
    /// decides once a `DiscoveredPublication`/`DiscoveredSubscription` does
    /// exist — those two never got the chance to run.
    pub unmatched_writers: Vec<Guid>,
    /// Topics a remote SEDP topics writer announced, or announced again
    /// with a changed QoS (DDSI-RTPS 2.5 §8.5.4.2). Already stored; see
    /// [`SedpStack::discovered_topics`].
    pub new_topics: Vec<TopicBuiltinTopicData>,
    /// Topics a remote participant deleted (a dispose on its topics
    /// writer) or that left with their participant, as they were last
    /// announced. Already removed from [`SedpStack::discovered_topics`].
    pub removed_topics: Vec<TopicBuiltinTopicData>,
    /// SEDP samples that did not decode — an endpoint or topic
    /// announcement, or a dispose that names nothing — by the writer that
    /// sent them. One sample this stack cannot read does not cost the
    /// others in the same datagram, so it is reported here instead of
    /// failing [`SedpStack::handle_datagram`]. The reliable reader has
    /// already taken the sample: the writer will not send it again.
    pub refused: Vec<(Guid, SedpReaderError)>,
}

impl SedpEvents {
    /// True if no discovery (additions or removals) was carried.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.new_publications.is_empty()
            && self.new_subscriptions.is_empty()
            && self.removed_publications.is_empty()
            && self.removed_subscriptions.is_empty()
            && self.unmatched_writers.is_empty()
            && self.new_topics.is_empty()
            && self.removed_topics.is_empty()
            && self.refused.is_empty()
    }
}

/// SEDP stack for a participant.
#[derive(Debug)]
pub struct SedpStack {
    local_prefix: GuidPrefix,
    pub_writer: SedpPublicationsWriter,
    pub_reader: SedpPublicationsReader,
    sub_writer: SedpSubscriptionsWriter,
    sub_reader: SedpSubscriptionsReader,
    // DDS-Security §9.4.2.7 `is_discovery_protected`: parallel secure SEDP
    // endpoints (DCPSPublicationsSecure/DCPSSubscriptionsSecure, EntityIds
    // ff0003c2/c7 + ff0004c2/c7). Used instead of the plaintext endpoints
    // when `discovery_protected` is set, to announce secured endpoints; the
    // SEC_* protection of the generated submessages is applied by the runtime
    // send path (participant crypto). Always present, but only populated when
    // `discovery_protected`.
    secure_pub_writer: SedpPublicationsWriter,
    secure_pub_reader: SedpPublicationsReader,
    secure_sub_writer: SedpSubscriptionsWriter,
    secure_sub_reader: SedpSubscriptionsReader,
    /// `true` ⟹ secured endpoints are announced via secure SEDP (governance
    /// `discovery_protection_kind != NONE`). Default `false` (plaintext SEDP).
    discovery_protected: bool,
    cache: DiscoveredEndpointsCache,
    // DDSI-RTPS 2.5 §8.5.4.2: the SEDP topics writer and reader, and the
    // topics remote participants announced over theirs, by topic GUID.
    topics_writer: SedpTopicsWriter,
    topics_reader: SedpTopicsReader,
    topics: BTreeMap<Guid, TopicBuiltinTopicData>,
}

impl SedpStack {
    /// Creates a new stack for the local participant. All
    /// endpoints initially have no remote proxies — those are wired up
    /// via [`on_participant_discovered`](Self::on_participant_discovered).
    #[must_use]
    pub fn new(local_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        // The readers need a writer proxy at `new()` — we
        // set a placeholder with an UNKNOWN prefix that is immediately
        // removed again (see `on_participant_discovered`).
        // A reader API without an initial proxy would be cosmetically
        // cleaner; functionally the "placeholder in → immediately out"
        // sequence is equivalent.
        let placeholder = GuidPrefix::UNKNOWN;
        let mut pub_reader =
            SedpPublicationsReader::new(local_prefix, vendor_id, placeholder, Vec::new());
        let mut sub_reader =
            SedpSubscriptionsReader::new(local_prefix, vendor_id, placeholder, Vec::new());
        // Remove placeholder proxies immediately — they have no
        // locators but would intercept every handle_data call with a
        // matching EntityId. We remove them to start clean.
        let pub_placeholder = Guid::new(placeholder, EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER);
        let sub_placeholder = Guid::new(placeholder, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER);
        pub_reader.remove_writer_proxy(pub_placeholder);
        sub_reader.remove_writer_proxy(sub_placeholder);
        // Create secure readers with a placeholder analogously + remove immediately.
        let mut secure_pub_reader =
            SedpPublicationsReader::new_secure(local_prefix, vendor_id, placeholder, Vec::new());
        let mut secure_sub_reader =
            SedpSubscriptionsReader::new_secure(local_prefix, vendor_id, placeholder, Vec::new());
        secure_pub_reader.remove_writer_proxy(Guid::new(
            placeholder,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
        ));
        secure_sub_reader.remove_writer_proxy(Guid::new(
            placeholder,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
        ));
        Self {
            local_prefix,
            pub_writer: SedpPublicationsWriter::new(local_prefix, vendor_id),
            pub_reader,
            sub_writer: SedpSubscriptionsWriter::new(local_prefix, vendor_id),
            sub_reader,
            secure_pub_writer: SedpPublicationsWriter::new_secure(local_prefix, vendor_id),
            secure_pub_reader,
            secure_sub_writer: SedpSubscriptionsWriter::new_secure(local_prefix, vendor_id),
            secure_sub_reader,
            discovery_protected: false,
            cache: DiscoveredEndpointsCache::default(),
            topics_writer: SedpTopicsWriter::new(local_prefix, vendor_id),
            topics_reader: SedpTopicsReader::new(local_prefix, vendor_id),
            topics: BTreeMap::new(),
        }
    }

    /// Enables protected discovery (DDS-Security §10.4.1.2.5): future
    /// `announce_*` calls route secured endpoints into the secure SEDP writers
    /// (ff0003c2/ff0004c2) instead of the plaintext writers. Set by the
    /// DcpsRuntime when governance has `discovery_protection_kind != NONE`.
    pub fn set_discovery_protected(&mut self, protected: bool) {
        self.discovery_protected = protected;
    }

    /// `true` if protected discovery is active.
    #[must_use]
    pub fn discovery_protected(&self) -> bool {
        self.discovery_protected
    }

    /// Sets the participant-wide RTPS behaviour ([`zerodds_rtps::dialect`])
    /// on all four SEDP writers — plaintext and secure, publications and
    /// subscriptions.
    ///
    /// All four, because the axis is a property of the participant and not of
    /// one endpoint: a dispose that names its instance one way over the
    /// plaintext writer and another way over the secure one would be two
    /// dialects in one participant, decided by whether discovery happens to
    /// be protected.
    pub fn set_behaviour(&mut self, behaviour: zerodds_rtps::dialect::RtpsBehaviour) {
        self.pub_writer.set_behaviour(behaviour);
        self.sub_writer.set_behaviour(behaviour);
        self.secure_pub_writer.set_behaviour(behaviour);
        self.secure_sub_writer.set_behaviour(behaviour);
        self.topics_writer.set_behaviour(behaviour);
    }

    /// Sets the RTPS behaviour for one matched peer, by its participant
    /// `GuidPrefix`. The override reaches the SEDP reader GUID of that peer
    /// on each of the four writers, which is the GUID the writers key their
    /// table on.
    ///
    /// Always explicit — nothing infers a dialect from a `vendorId`.
    pub fn set_peer_behaviour(
        &mut self,
        peer: GuidPrefix,
        behaviour: zerodds_rtps::dialect::RtpsBehaviour,
    ) {
        use zerodds_rtps::wire_types::{EntityId, Guid};
        let pubs = Guid::new(peer, EntityId::SEDP_BUILTIN_PUBLICATIONS_READER);
        let subs = Guid::new(peer, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER);
        let sec_pubs = Guid::new(peer, EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER);
        let sec_subs = Guid::new(peer, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER);
        self.pub_writer.set_peer_behaviour(pubs, behaviour);
        self.sub_writer.set_peer_behaviour(subs, behaviour);
        self.secure_pub_writer
            .set_peer_behaviour(sec_pubs, behaviour);
        self.secure_sub_writer
            .set_peer_behaviour(sec_subs, behaviour);
        self.topics_writer.set_peer_behaviour(
            Guid::new(peer, EntityId::SEDP_BUILTIN_TOPICS_READER),
            behaviour,
        );
    }

    /// Local GuidPrefix of this stack.
    #[must_use]
    pub fn local_prefix(&self) -> GuidPrefix {
        self.local_prefix
    }

    /// Read-only access to the DiscoveredEndpointsCache.
    #[must_use]
    pub fn cache(&self) -> &DiscoveredEndpointsCache {
        &self.cache
    }

    /// Mutable access to the cache. Used primarily for tests
    /// (inject discovery match without live UDP) and WP 2.5
    /// `find_topic` verification.
    pub fn cache_mut(&mut self) -> &mut DiscoveredEndpointsCache {
        &mut self.cache
    }

    /// Announces a local publication to all already-discovered
    /// remote SEDP readers.
    ///
    /// # Errors
    /// Encoder error.
    pub fn announce_publication(
        &mut self,
        p: &PublicationBuiltinTopicData,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        if self.discovery_protected {
            self.secure_pub_writer.announce(p)
        } else {
            self.pub_writer.announce(p)
        }
    }

    /// Disposes a deleted local publication (SEDP dispose+unregister keyed on
    /// the endpoint GUID) — routed to the secure or plain publications writer
    /// exactly like `announce_publication`.
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn dispose_publication(
        &mut self,
        endpoint_guid: Guid,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        if self.discovery_protected {
            self.secure_pub_writer.dispose(endpoint_guid)
        } else {
            self.pub_writer.dispose(endpoint_guid)
        }
    }

    /// ADR-0006: publication with PID_SHM_LOCATOR (vendor PID 0x8001).
    ///
    /// # Errors
    /// Encoder error or inject error.
    pub fn announce_publication_with_shm_locator(
        &mut self,
        p: &PublicationBuiltinTopicData,
        locator_bytes: &[u8],
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        if self.discovery_protected {
            self.secure_pub_writer
                .announce_with_shm_locator(p, locator_bytes)
        } else {
            self.pub_writer.announce_with_shm_locator(p, locator_bytes)
        }
    }

    /// The sequence number of the latest publication announcement — the
    /// secure writer's under protected discovery. An in-process peer is
    /// handed the announcement with it
    /// ([`crate::sedp::DiscoveredEndpointsCache::insert_publication_in_process`]).
    #[must_use]
    pub fn last_publication_sequence(&self) -> zerodds_rtps::wire_types::SequenceNumber {
        if self.discovery_protected {
            self.secure_pub_writer.last_sequence_number()
        } else {
            self.pub_writer.last_sequence_number()
        }
    }

    /// The subscription counterpart of [`Self::last_publication_sequence`].
    #[must_use]
    pub fn last_subscription_sequence(&self) -> zerodds_rtps::wire_types::SequenceNumber {
        if self.discovery_protected {
            self.secure_sub_writer.last_sequence_number()
        } else {
            self.sub_writer.last_sequence_number()
        }
    }

    /// Announces a local subscription.
    ///
    /// # Errors
    /// Encoder error.
    pub fn announce_subscription(
        &mut self,
        s: &SubscriptionBuiltinTopicData,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        if self.discovery_protected {
            self.secure_sub_writer.announce(s)
        } else {
            self.sub_writer.announce(s)
        }
    }

    /// Disposes a deleted local subscription (counterpart of
    /// `dispose_publication`).
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn dispose_subscription(
        &mut self,
        endpoint_guid: Guid,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        if self.discovery_protected {
            self.secure_sub_writer.dispose(endpoint_guid)
        } else {
            self.sub_writer.dispose(endpoint_guid)
        }
    }

    /// Announces a local topic, or its changed QoS, over the SEDP topics
    /// writer (DDSI-RTPS 2.5 §8.5.4.2, Table 8.84).
    ///
    /// Under protected discovery nothing is written: DDS-Security 1.2
    /// defines no secure counterpart of the topics endpoints, and a topic
    /// announced in the clear would name what the governance protects.
    ///
    /// # Errors
    /// Encoder error.
    pub fn announce_topic(
        &mut self,
        t: &TopicBuiltinTopicData,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        if self.discovery_protected {
            return Ok(Vec::new());
        }
        self.topics_writer.announce(t)
    }

    /// Disposes a deleted local topic, keyed on its GUID; nothing under
    /// protected discovery, as for [`Self::announce_topic`].
    ///
    /// # Errors
    /// Wire encode error or sequence-number overflow.
    pub fn dispose_topic(&mut self, topic_guid: Guid) -> Result<Vec<OutboundDatagram>, WireError> {
        if self.discovery_protected {
            return Ok(Vec::new());
        }
        self.topics_writer.dispose(topic_guid)
    }

    /// The topics remote participants announced over their SEDP topics
    /// writers and have not deleted.
    pub fn discovered_topics(&self) -> impl Iterator<Item = &TopicBuiltinTopicData> + '_ {
        self.topics.values()
    }

    /// The discovered topics of one remote participant, as
    /// [`SedpEvents::removed_topics`] reports them when it is lost.
    #[must_use]
    pub fn topics_of(&self, prefix: GuidPrefix) -> Vec<TopicBuiltinTopicData> {
        self.topics
            .values()
            .filter(|t| t.key.prefix == prefix)
            .cloned()
            .collect()
    }

    /// Wires up the SEDP endpoints for a newly discovered
    /// participant. Reads `p.data.builtin_endpoint_set` + unicast
    /// locators and registers the matching proxies: the regular and the
    /// secure ones.
    pub fn on_participant_discovered(&mut self, p: &DiscoveredParticipant) {
        self.on_participant_discovered_regular(p);
        self.on_participant_discovered_secure(p);
    }

    /// The regular SEDP endpoints of `p` (DCPSPublications,
    /// DCPSSubscriptions, DCPSTopics): with DDS-Security, all an
    /// unauthenticated participant is matched with, and an authenticated
    /// one besides the secure ones (DDS-Security 1.2 §9.8.3).
    pub fn on_participant_discovered_regular(&mut self, p: &DiscoveredParticipant) {
        let remote_prefix = p.sender_prefix;
        if remote_prefix == self.local_prefix {
            // Self-discovery: no self-match (per spec).
            return;
        }
        // SEDP routes to metatraffic_unicast_locator (PID 0x0032) —
        // only if that is missing does it fall back to default_unicast_locator.
        // Cyclone/FastDDS announce both, but with strictly separate roles.
        let unicast_locators: Vec<_> = p.data.metatraffic_or_default_unicast_locators();
        let flags = p.data.builtin_endpoint_set;

        // Does the remote have a publications announcer (writer)?
        //  → our SedpPublicationsReader gets a WriterProxy
        if flags & endpoint_flag::PUBLICATIONS_ANNOUNCER != 0 {
            self.pub_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(remote_prefix, EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }

        // Does the remote have a publications detector (reader)?
        //  → our SedpPublicationsWriter gets a ReaderProxy
        if flags & endpoint_flag::PUBLICATIONS_DETECTOR != 0 {
            self.pub_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(remote_prefix, EntityId::SEDP_BUILTIN_PUBLICATIONS_READER),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }

        // Subscriptions announcer → our subscriptions reader
        if flags & endpoint_flag::SUBSCRIPTIONS_ANNOUNCER != 0 {
            self.sub_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(remote_prefix, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }

        // Subscriptions detector → our subscriptions writer
        if flags & endpoint_flag::SUBSCRIPTIONS_DETECTOR != 0 {
            self.sub_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(remote_prefix, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }

        // Topics announcer → our topics reader; topics detector → our topics
        // writer (DDSI-RTPS 2.5 §8.5.5.1).
        if flags & endpoint_flag::TOPICS_ANNOUNCER != 0 {
            self.topics_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(remote_prefix, EntityId::SEDP_BUILTIN_TOPICS_WRITER),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }
        if flags & endpoint_flag::TOPICS_DETECTOR != 0 {
            self.topics_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(remote_prefix, EntityId::SEDP_BUILTIN_TOPICS_READER),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }
    }

    /// The secure SEDP endpoints of `p` (DDS-Security 1.2 §10.4.1.2.5), when
    /// it announces them; matched once `p` is authenticated (§9.8.3).
    /// Mirrors the plaintext wiring with the SECURE EntityIds.
    pub fn on_participant_discovered_secure(&mut self, p: &DiscoveredParticipant) {
        let remote_prefix = p.sender_prefix;
        if remote_prefix == self.local_prefix {
            return;
        }
        let unicast_locators: Vec<_> = p.data.metatraffic_or_default_unicast_locators();
        let flags = p.data.builtin_endpoint_set;
        if flags & endpoint_flag::PUBLICATIONS_SECURE_WRITER != 0 {
            self.secure_pub_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
                ),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }
        if flags & endpoint_flag::PUBLICATIONS_SECURE_READER != 0 {
            self.secure_pub_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
                ),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }
        if flags & endpoint_flag::SUBSCRIPTIONS_SECURE_WRITER != 0 {
            self.secure_sub_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
                ),
                unicast_locators.clone(),
                Vec::new(),
                true,
            ));
        }
        if flags & endpoint_flag::SUBSCRIPTIONS_SECURE_READER != 0 {
            self.secure_sub_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
                ),
                unicast_locators,
                Vec::new(),
                true,
            ));
        }
    }

    /// A participant has been lost (SPDP lease timeout). Cascade
    /// cleanup: remove all proxies with this prefix, delete cache
    /// entries. Returns (removed_pub_proxies, removed_sub_proxies).
    pub fn on_participant_lost(&mut self, prefix: GuidPrefix) -> (usize, usize) {
        let mut removed = 0usize;
        // Writer side: remove remote reader
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
        // Reader side: remove remote writer
        self.pub_reader.remove_writer_proxy(Guid::new(
            prefix,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
        ));
        self.sub_reader.remove_writer_proxy(Guid::new(
            prefix,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
        ));
        // Remove secure SEDP proxies analogously.
        self.secure_pub_writer.remove_reader_proxy(Guid::new(
            prefix,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
        ));
        self.secure_sub_writer.remove_reader_proxy(Guid::new(
            prefix,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
        ));
        self.secure_pub_reader.remove_writer_proxy(Guid::new(
            prefix,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
        ));
        self.secure_sub_reader.remove_writer_proxy(Guid::new(
            prefix,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
        ));
        self.topics_writer
            .remove_reader_proxy(Guid::new(prefix, EntityId::SEDP_BUILTIN_TOPICS_READER));
        self.topics_reader
            .remove_writer_proxy(Guid::new(prefix, EntityId::SEDP_BUILTIN_TOPICS_WRITER));
        self.topics.retain(|key, _| key.prefix != prefix);
        // Cache cleanup
        let (pubs, subs) = self.cache.on_participant_lost(prefix);
        let _ = removed; // reader-removal count not currently evaluated
        (pubs, subs)
    }

    /// Folds a publications [`SedpDelta`] into the cache + the event set:
    /// ALIVE announcements are stored and reported as `new_publications`,
    /// dispose/unregister GUIDs are evicted from the cache and reported as
    /// `removed_publications`. Shared by the DATA/DATA_FRAG/GAP dispatch arms
    /// for both the plain and the secure publications readers.
    fn absorb_pub_delta(
        &mut self,
        writer: Guid,
        delta: Result<SedpDelta<PublicationBuiltinTopicData>, SedpReaderError>,
        now: Duration,
        events: &mut SedpEvents,
    ) {
        let delta = match delta {
            Ok(delta) => delta,
            Err(e) => {
                events.refused.push((writer, e));
                return;
            }
        };
        for p in delta.added {
            // Left out when an in-process peer already handed over a newer
            // announcement; see `insert_publication_announced`.
            if self
                .cache
                .insert_publication_announced(p.data.clone(), now, p.sequence)
                .is_some()
            {
                events.new_publications.push(p.data);
            }
        }
        for g in delta.removed {
            self.cache.remove_publication(g);
            events.removed_publications.push(g);
        }
    }

    /// Subscriptions counterpart of [`Self::absorb_pub_delta`].
    fn absorb_sub_delta(
        &mut self,
        writer: Guid,
        delta: Result<SedpDelta<SubscriptionBuiltinTopicData>, SedpReaderError>,
        now: Duration,
        events: &mut SedpEvents,
    ) {
        let delta = match delta {
            Ok(delta) => delta,
            Err(e) => {
                events.refused.push((writer, e));
                return;
            }
        };
        for s in delta.added {
            if self
                .cache
                .insert_subscription_announced(s.data.clone(), now, s.sequence)
                .is_some()
            {
                events.new_subscriptions.push(s.data);
            }
        }
        for g in delta.removed {
            self.cache.remove_subscription(g);
            events.removed_subscriptions.push(g);
        }
    }

    /// Topics counterpart of [`Self::absorb_pub_delta`]: an announcement
    /// is stored under its key and reported; a dispose of a topic this
    /// stack holds removes it and reports what it was. A dispose of a
    /// topic it never held reports nothing — there is nothing to retract.
    fn absorb_topic_delta(
        &mut self,
        writer: Guid,
        delta: Result<SedpDelta<TopicBuiltinTopicData>, SedpReaderError>,
        events: &mut SedpEvents,
    ) {
        let delta = match delta {
            Ok(delta) => delta,
            Err(e) => {
                events.refused.push((writer, e));
                return;
            }
        };
        for t in delta.added {
            self.topics.insert(t.data.key, t.data.clone());
            events.new_topics.push(t.data);
        }
        for g in delta.removed {
            if let Some(gone) = self.topics.remove(&g) {
                events.removed_topics.push(gone);
            }
        }
    }

    /// Processes an incoming RTPS datagram into the SEDP stack.
    /// Routes the submessages by target EntityId to the matching
    /// endpoint and updates the cache. Returns the [`SedpEvents`] delta —
    /// newly discovered endpoints **and** the GUIDs of endpoints the remote
    /// disposed (deleted).
    ///
    /// # Errors
    /// A datagram that does not decode as RTPS at all. A sample inside a
    /// well-formed datagram that does not decode is in
    /// [`SedpEvents::refused`] instead.
    pub fn handle_datagram(
        &mut self,
        datagram: &[u8],
        now: Duration,
    ) -> Result<SedpEvents, SedpReaderError> {
        self.handle_datagram_in(datagram, &ReceiveContext::default(), now)
    }

    /// [`Self::handle_datagram`] for a datagram that arrived in `context`:
    /// its source and reception clock (DDSI-RTPS 2.5 §8.3.4).
    ///
    /// # Errors
    /// As [`Self::handle_datagram`].
    pub fn handle_datagram_in(
        &mut self,
        datagram: &[u8],
        context: &ReceiveContext,
        now: Duration,
    ) -> Result<SedpEvents, SedpReaderError> {
        let parsed = decode_datagram(datagram).map_err(SedpReaderError::from)?;
        let mut events = SedpEvents::default();
        // DDSI-RTPS 2.5 §8.3.4: effective writer/reader source =
        // sourceGuidPrefix + entityId. A datagram may carry the submessages
        // of several source participants behind InfoSource — OpenSplice's
        // DDSI2 service does exactly this when it bundles several
        // participants it hosts into one datagram — so the source prefix is
        // recomputed per submessage via `receiver_walk` rather than taken
        // once from the header.
        for (state, sub) in parsed.receiver_walk_in(self.local_prefix, context) {
            // A non-UNKNOWN destGuidPrefix that isn't ours scopes this
            // submessage to a different participant on the same host/socket.
            if state.dest_guid_prefix != self.local_prefix
                && state.dest_guid_prefix != GuidPrefix::UNKNOWN
            {
                continue;
            }
            let source_prefix = state.source_guid_prefix;
            let sender = state.source_vendor_id;
            match sub {
                ParsedSubmessage::Data(d) => {
                    // reader_id match with UNKNOWN→writer_id fallback (§8.3.7.2).
                    if routes_to_pub(d.reader_id, d.writer_id) {
                        let writer_guid = Guid::new(source_prefix, d.writer_id);
                        if !has_writer_proxy(self.pub_reader.inner(), writer_guid) {
                            events.unmatched_writers.push(writer_guid);
                        }
                        let delta = self.pub_reader.handle_data(source_prefix, d);
                        let writer = Guid::new(source_prefix, d.writer_id);
                        self.absorb_pub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sub(d.reader_id, d.writer_id) {
                        let writer_guid = Guid::new(source_prefix, d.writer_id);
                        if !has_writer_proxy(self.sub_reader.inner(), writer_guid) {
                            events.unmatched_writers.push(writer_guid);
                        }
                        let delta = self.sub_reader.handle_data(source_prefix, d);
                        let writer = Guid::new(source_prefix, d.writer_id);
                        self.absorb_sub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sec_pub(d.reader_id, d.writer_id) {
                        let delta = self.secure_pub_reader.handle_data(source_prefix, d);
                        let writer = Guid::new(source_prefix, d.writer_id);
                        self.absorb_pub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sec_sub(d.reader_id, d.writer_id) {
                        let delta = self.secure_sub_reader.handle_data(source_prefix, d);
                        let writer = Guid::new(source_prefix, d.writer_id);
                        self.absorb_sub_delta(writer, delta, now, &mut events);
                    } else if routes_to_topics(d.reader_id, d.writer_id) {
                        let writer_guid = Guid::new(source_prefix, d.writer_id);
                        if !has_writer_proxy(self.topics_reader.inner(), writer_guid) {
                            events.unmatched_writers.push(writer_guid);
                        }
                        let delta = self.topics_reader.handle_data(source_prefix, sender, d);
                        self.absorb_topic_delta(writer_guid, delta, &mut events);
                    }
                }
                ParsedSubmessage::DataFrag(df) => {
                    if routes_to_pub(df.reader_id, df.writer_id) {
                        let delta = self.pub_reader.handle_data_frag(source_prefix, df, now);
                        let writer = Guid::new(source_prefix, df.writer_id);
                        self.absorb_pub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sub(df.reader_id, df.writer_id) {
                        let delta = self.sub_reader.handle_data_frag(source_prefix, df, now);
                        let writer = Guid::new(source_prefix, df.writer_id);
                        self.absorb_sub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sec_pub(df.reader_id, df.writer_id) {
                        let delta = self
                            .secure_pub_reader
                            .handle_data_frag(source_prefix, df, now);
                        let writer = Guid::new(source_prefix, df.writer_id);
                        self.absorb_pub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sec_sub(df.reader_id, df.writer_id) {
                        let delta = self
                            .secure_sub_reader
                            .handle_data_frag(source_prefix, df, now);
                        let writer = Guid::new(source_prefix, df.writer_id);
                        self.absorb_sub_delta(writer, delta, now, &mut events);
                    } else if routes_to_topics(df.reader_id, df.writer_id) {
                        let delta =
                            self.topics_reader
                                .handle_data_frag(source_prefix, sender, df, now);
                        let writer = Guid::new(source_prefix, df.writer_id);
                        self.absorb_topic_delta(writer, delta, &mut events);
                    }
                }
                ParsedSubmessage::Heartbeat(h) => {
                    // Cyclone often sends HEARTBEATs with reader_id=UNKNOWN and
                    // uses INFO_DST for addressing — dispatch then by
                    // writer_id to the matching SEDP reader.
                    // A HEARTBEAT whose first_sn passes a hole releases the
                    // announcements buffered behind it (OpenSplice sends a
                    // subscription's DATA by multicast and the GAP for the
                    // sequence number before it by unicast; a HEARTBEAT that
                    // arrives between them releases the DATA). They are
                    // absorbed like a DATA's.
                    if routes_to_pub(h.reader_id, h.writer_id) {
                        let delta = self.pub_reader.handle_heartbeat(source_prefix, h, now);
                        let writer = Guid::new(source_prefix, h.writer_id);
                        self.absorb_pub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sub(h.reader_id, h.writer_id) {
                        let delta = self.sub_reader.handle_heartbeat(source_prefix, h, now);
                        let writer = Guid::new(source_prefix, h.writer_id);
                        self.absorb_sub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sec_pub(h.reader_id, h.writer_id) {
                        let delta = self
                            .secure_pub_reader
                            .handle_heartbeat(source_prefix, h, now);
                        let writer = Guid::new(source_prefix, h.writer_id);
                        self.absorb_pub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sec_sub(h.reader_id, h.writer_id) {
                        let delta = self
                            .secure_sub_reader
                            .handle_heartbeat(source_prefix, h, now);
                        let writer = Guid::new(source_prefix, h.writer_id);
                        self.absorb_sub_delta(writer, delta, now, &mut events);
                    } else if routes_to_topics(h.reader_id, h.writer_id) {
                        let delta =
                            self.topics_reader
                                .handle_heartbeat(source_prefix, sender, h, now);
                        let writer = Guid::new(source_prefix, h.writer_id);
                        self.absorb_topic_delta(writer, delta, &mut events);
                    }
                }
                ParsedSubmessage::Gap(g) => {
                    // GAP shares the UNKNOWN→writer_id fallback of the HEARTBEAT/
                    // DATA paths (H-5): without it the reliable SEDP reader
                    // stalls against cyclone, which sends GAP with reader_id=UNKNOWN.
                    if routes_to_pub(g.reader_id, g.writer_id) {
                        let delta = self.pub_reader.handle_gap(source_prefix, g);
                        let writer = Guid::new(source_prefix, g.writer_id);
                        self.absorb_pub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sub(g.reader_id, g.writer_id) {
                        let delta = self.sub_reader.handle_gap(source_prefix, g);
                        let writer = Guid::new(source_prefix, g.writer_id);
                        self.absorb_sub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sec_pub(g.reader_id, g.writer_id) {
                        let delta = self.secure_pub_reader.handle_gap(source_prefix, g);
                        let writer = Guid::new(source_prefix, g.writer_id);
                        self.absorb_pub_delta(writer, delta, now, &mut events);
                    } else if routes_to_sec_sub(g.reader_id, g.writer_id) {
                        let delta = self.secure_sub_reader.handle_gap(source_prefix, g);
                        let writer = Guid::new(source_prefix, g.writer_id);
                        self.absorb_sub_delta(writer, delta, now, &mut events);
                    } else if routes_to_topics(g.reader_id, g.writer_id) {
                        let delta = self.topics_reader.handle_gap(source_prefix, sender, g);
                        let writer = Guid::new(source_prefix, g.writer_id);
                        self.absorb_topic_delta(writer, delta, &mut events);
                    }
                }
                ParsedSubmessage::AckNack(ack) => {
                    // Dispatch to writer by writer_id
                    let base = ack.reader_sn_state.bitmap_base;
                    let requested: Vec<_> = ack.reader_sn_state.iter_set().collect();
                    let src = Guid::new(source_prefix, ack.reader_id);
                    let f = ack.final_flag;
                    if ack.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER {
                        self.pub_writer.handle_acknack(src, base, requested);
                        self.pub_writer.note_acknack_final_flag(src, f);
                    } else if ack.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER {
                        self.sub_writer.handle_acknack(src, base, requested);
                        self.sub_writer.note_acknack_final_flag(src, f);
                    } else if ack.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER {
                        self.secure_pub_writer.handle_acknack(src, base, requested);
                        self.secure_pub_writer.note_acknack_final_flag(src, f);
                    } else if ack.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER {
                        self.secure_sub_writer.handle_acknack(src, base, requested);
                        self.secure_sub_writer.note_acknack_final_flag(src, f);
                    } else if ack.writer_id == EntityId::SEDP_BUILTIN_TOPICS_WRITER {
                        self.topics_writer.handle_acknack(src, base, requested);
                        self.topics_writer.note_acknack_final_flag(src, f);
                    }
                }
                ParsedSubmessage::NackFrag(nf) => {
                    let src = Guid::new(source_prefix, nf.reader_id);
                    if nf.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER {
                        self.secure_pub_writer.handle_nackfrag(src, nf);
                    } else if nf.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER {
                        self.secure_sub_writer.handle_nackfrag(src, nf);
                    } else if nf.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER {
                        self.pub_writer.handle_nackfrag(src, nf);
                    } else if nf.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER {
                        self.sub_writer.handle_nackfrag(src, nf);
                    } else if nf.writer_id == EntityId::SEDP_BUILTIN_TOPICS_WRITER {
                        self.topics_writer.handle_nackfrag(src, nf);
                    }
                }
                ParsedSubmessage::InfoDestination(_)
                | ParsedSubmessage::HeartbeatFrag(_)
                | ParsedSubmessage::HeaderExtension(_)
                | ParsedSubmessage::InfoSource(_)
                | ParsedSubmessage::InfoReply(_)
                | ParsedSubmessage::InfoReplyIp4(_)
                | ParsedSubmessage::InfoTimestamp(_)
                | ParsedSubmessage::Unknown { .. } => {}
                // ParsedSubmessage is #[non_exhaustive]: ignore future kinds.
                _ => {}
            }
        }
        Ok(events)
    }

    /// Tick over all endpoints. Returns HEARTBEATs + resends from the
    /// writers plus ACKNACK/NACK_FRAG from the readers.
    ///
    /// # Errors
    /// Wire encode error.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = Vec::new();
        out.extend(self.pub_writer.tick(now)?);
        out.extend(self.sub_writer.tick(now)?);
        // Reader ticks return ACKNACK/NACK_FRAG datagrams with the
        // WriterProxy unicast locators as target (RTPS §8.4.15).
        // Required for live interop: Cyclone waits for the initial
        // ACKNACK before the reliable writer sends DATA.
        out.extend(self.pub_reader.tick_outbound(now)?);
        out.extend(self.sub_reader.tick_outbound(now)?);
        // Secure SEDP endpoints (proxies only present if a peer announced
        // secure SEDP). The HEARTBEAT/DATA outputs of the secure writers
        // are SEC_*-protected in the runtime send path; ACKNACKs of the
        // secure readers are submessages that Cyclone also accepts
        // unprotected ("clear submsg from protected src"), but we protect
        // them too for consistency via the writer_id-driven path.
        out.extend(self.secure_pub_writer.tick(now)?);
        out.extend(self.secure_sub_writer.tick(now)?);
        out.extend(self.secure_pub_reader.tick_outbound(now)?);
        out.extend(self.secure_sub_reader.tick_outbound(now)?);
        out.extend(self.topics_writer.tick(now)?);
        out.extend(self.topics_reader.tick_outbound(now)?);
        Ok(out)
    }

    /// Whether the regular and whether the secure DCPSPublications reader of
    /// the participant `prefix` is matched with this side's writer: which of
    /// its SEDP endpoints are matched (DDS-Security 1.2 §9.8.3).
    #[must_use]
    pub fn publications_readers_matched(&self, prefix: GuidPrefix) -> (bool, bool) {
        let has = |writer: &zerodds_rtps::reliable_writer::ReliableWriter, eid: EntityId| {
            let guid = Guid::new(prefix, eid);
            writer
                .reader_proxies()
                .iter()
                .any(|p| p.remote_reader_guid == guid)
        };
        (
            has(
                self.pub_writer.inner(),
                EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
            ),
            has(
                self.secure_pub_writer.inner(),
                EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
            ),
        )
    }

    /// Read-only access to the internal endpoints (diagnostics/tests).
    #[must_use]
    pub fn pub_writer(&self) -> &SedpPublicationsWriter {
        &self.pub_writer
    }

    /// ditto.
    #[must_use]
    pub fn pub_reader(&self) -> &SedpPublicationsReader {
        &self.pub_reader
    }

    /// ditto.
    #[must_use]
    pub fn sub_writer(&self) -> &SedpSubscriptionsWriter {
        &self.sub_writer
    }

    /// ditto.
    #[must_use]
    pub fn sub_reader(&self) -> &SedpSubscriptionsReader {
        &self.sub_reader
    }
}

/// `true` if `reader`'s `ReliableReader` already holds a writer proxy for
/// `writer_guid` — i.e. [`SedpStack::on_participant_discovered`] ran for
/// this exact GUID prefix. Backs [`SedpEvents::unmatched_writers`]: checked
/// *before* dispatch, so it reports the gap the same tick the DATA arrived
/// unmatched, not after `handle_data` has already no-op'd it into the
/// `ReliableReader`'s bare `unknown_src_count`.
fn has_writer_proxy(
    reader: &zerodds_rtps::reliable_reader::ReliableReader,
    writer_guid: Guid,
) -> bool {
    reader
        .writer_proxies()
        .iter()
        .any(|s| s.proxy.remote_writer_guid == writer_guid)
}

/// Routes an incoming writer submessage (DATA/DATA_FRAG/GAP/HEARTBEAT) to
/// the matching SEDP reader. Match by `reader_id`; for
/// `reader_id == ENTITYID_UNKNOWN` (cyclone/FastDDS address via INFO_DST,
/// DDSI-RTPS §8.3.7.2) it is resolved via the `writer_id`. Without the
/// UNKNOWN fallback the reader dispatch silently discards DATA/GAP of such peers →
/// the reliable SEDP reader stalls → endpoint discovery stays incomplete.
fn routes_to_reader(
    reader_id: EntityId,
    writer_id: EntityId,
    reader: EntityId,
    writer: EntityId,
) -> bool {
    reader_id == reader || (reader_id == EntityId::UNKNOWN && writer_id == writer)
}

fn routes_to_pub(reader_id: EntityId, writer_id: EntityId) -> bool {
    routes_to_reader(
        reader_id,
        writer_id,
        EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
        EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
    )
}

fn routes_to_sub(reader_id: EntityId, writer_id: EntityId) -> bool {
    routes_to_reader(
        reader_id,
        writer_id,
        EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
        EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
    )
}

fn routes_to_sec_pub(reader_id: EntityId, writer_id: EntityId) -> bool {
    routes_to_reader(
        reader_id,
        writer_id,
        EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
        EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
    )
}

fn routes_to_topics(reader_id: EntityId, writer_id: EntityId) -> bool {
    routes_to_reader(
        reader_id,
        writer_id,
        EntityId::SEDP_BUILTIN_TOPICS_READER,
        EntityId::SEDP_BUILTIN_TOPICS_WRITER,
    )
}

fn routes_to_sec_sub(reader_id: EntityId, writer_id: EntityId) -> bool {
    routes_to_reader(
        reader_id,
        writer_id,
        EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
        EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::vec;
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
                default_unicast_locators: vec![Locator::udp_v4([127, 0, 0, 99], 7411)],
                default_multicast_locators: Vec::new(),
                metatraffic_unicast_locators: Vec::new(),
                metatraffic_multicast_locators: Vec::new(),
                domain_id: None,
                domain_tag: Default::default(),
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
                available_builtin_endpoints_ext: 0,
                participant_security_info: None,
                expects_inline_qos: false,
                manual_liveliness_count: None,
                builtin_endpoint_qos: None,
                entity_name: None,
            },
        }
    }

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

    fn sample_sub(prefix: GuidPrefix, topic: &str) -> SubscriptionBuiltinTopicData {
        SubscriptionBuiltinTopicData {
            content_filter_unreadable: false,
            key: Guid::new(prefix, EntityId::user_reader_with_key([0x70, 0x80, 0x90])),
            participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
            topic_name: topic.into(),
            type_name: "std_msgs::String".into(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: DdsDuration::from_secs(10),
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: zerodds_rtps::publication_data::PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec::Vec::new(),
            content_filter: None,
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_consistency: Default::default(),
            vendor_id: Some(zerodds_rtps::wire_types::VendorId([0x01, 0x02])),
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
            expects_inline_qos: false,
            group_guid: None,
            type_consistency_announced: true,
            entity_name: None,
        }
    }

    /// Reproduces OpenSplice 6.9's measured DDSI2-bundling datagram shape
    /// (`ospl-sedp-sub-to-zd.txt`): one datagram, one outer source
    /// (`bundling_prefix`, the DDSI2 network service), carrying
    /// `INFO_DST(local) + HEARTBEAT + INFO_SRC(owner_prefix) + GAP +
    /// HEARTBEAT + INFO_SRC(owner_prefix) + DATA(r)` — the actual
    /// subscription's owner is a *different* participant than the one the
    /// outer RTPS header names, switched in mid-datagram.
    ///
    /// `owner_prefix` is the SEDP subscriptions writer's effective source
    /// after INFO_SRC — i.e. the GUID `on_participant_discovered` would need
    /// to have run for, for [`SedpStack::handle_datagram`] to have a writer
    /// proxy under that exact prefix.
    fn ospl_bundled_sub_datagram(
        bundling_prefix: GuidPrefix,
        local: GuidPrefix,
        owner_prefix: GuidPrefix,
        topic: &str,
    ) -> alloc::vec::Vec<u8> {
        use zerodds_rtps::message_builder::MessageBuilder;
        use zerodds_rtps::submessages::{
            GapSubmessage, HeartbeatSubmessage, InfoSourceSubmessage, SequenceNumberSet,
        };
        use zerodds_rtps::wire_types::{ProtocolVersion, SequenceNumber};
        use zerodds_rtps::{RtpsHeader, SubmessageId};

        // The DATA itself, correctly encoded via a throwaway stack scoped to
        // `owner_prefix` — same technique as `sedp_pub_datagram`. A dummy
        // reader proxy is required for `announce_subscription` to actually
        // produce a datagram at all (nothing to address it to otherwise).
        let mut owner = SedpStack::new(owner_prefix, VendorId::ZERODDS);
        owner
            .on_participant_discovered(&remote_participant(GuidPrefix::from_bytes([250; 12]), DET));
        let sub = sample_sub(owner_prefix, topic);
        let ann = owner.announce_subscription(&sub).unwrap();
        let decoded = zerodds_rtps::datagram::decode_datagram(&ann[0].bytes).unwrap();
        let data = decoded
            .contents()
            .find_map(|sm| match sm {
                ParsedSubmessage::Data(d) => Some(d),
                _ => None,
            })
            .expect("the announce carries a DATA submessage");
        let (data_body, data_flags) = data.write_body(true);

        let hdr = RtpsHeader::new(VendorId::ZERODDS, bundling_prefix);
        let mut b = MessageBuilder::open(hdr, alloc::sync::Arc::new(alloc::vec![]), 1400);
        b.try_add_submessage(SubmessageId::InfoDst, 0, &local.to_bytes())
            .unwrap();
        // HEARTBEAT from the bundling prefix's own subscriptions writer —
        // frame 253's leading submessage, unrelated to `owner_prefix`.
        let hb0 = HeartbeatSubmessage {
            reader_id: EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
            writer_id: EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(1),
            count: 1,
            final_flag: true,
            liveliness_flag: false,
            group_info: None,
        };
        let (hb0_body, hb0_flags) = hb0.write_body(true);
        b.try_add_submessage(SubmessageId::Heartbeat, hb0_flags, &hb0_body)
            .unwrap();
        // INFO_SRC switches the effective source to `owner_prefix`.
        let info = InfoSourceSubmessage {
            unused: 0,
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId([0x01, 0x02]),
            guid_prefix: owner_prefix,
        };
        let (info_body, info_flags) = info.write_body(true);
        b.try_add_submessage(SubmessageId::InfoSrc, info_flags, &info_body)
            .unwrap();
        // GAP for `owner_prefix`'s subscriptions writer — measured shape
        // (frame 253: gapStart=1, bitmapBase=2), but a no-op range here
        // (gapStart == bitmapBase, no bits set): it must not retire the SN
        // the DATA below actually carries.
        let gap = GapSubmessage {
            reader_id: EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
            writer_id: EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
            gap_start: SequenceNumber(1),
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(1),
                num_bits: 0,
                bitmap: alloc::vec::Vec::new(),
            },
            group_info: None,
            filtered_count: None,
        };
        let (gap_body, gap_flags) = gap.write_body(true);
        b.try_add_submessage(SubmessageId::Gap, gap_flags, &gap_body)
            .unwrap();
        // HEARTBEAT for `owner_prefix`'s subscriptions writer, announcing
        // SN 1 — the SN `announce_subscription` gave the DATA below (a
        // fresh `SedpStack`'s first sample on that writer).
        let hb1 = HeartbeatSubmessage {
            reader_id: EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
            writer_id: EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(1),
            count: 1,
            final_flag: true,
            liveliness_flag: false,
            group_info: None,
        };
        let (hb1_body, hb1_flags) = hb1.write_body(true);
        b.try_add_submessage(SubmessageId::Heartbeat, hb1_flags, &hb1_body)
            .unwrap();
        // INFO_SRC(owner_prefix) again — real captures repeat it right
        // before the DATA it scopes (frame 268's per-sample INFO_TS/DATA
        // pairs each follow their own INFO_SRC in the same way).
        b.try_add_submessage(SubmessageId::InfoSrc, info_flags, &info_body)
            .unwrap();
        b.try_add_submessage(SubmessageId::Data, data_flags, &data_body)
            .unwrap();
        b.finish().map(|d| d.bytes).unwrap()
    }

    /// OpenSplice CE 6.9 sends a new subscription's DATA by multicast and the
    /// GAP for the sequence number before it by unicast. When the HEARTBEAT
    /// that names the DATA's sequence number as the first available one
    /// arrives before that GAP, the HEARTBEAT is what releases the buffered
    /// DATA. The SEDP reader threw away what a HEARTBEAT released: the
    /// subscription counted as received (ACKNACK past it), was never
    /// discovered, and a ZeroDDS writer never matched the OpenSplice reader
    /// (dds-rtps Test_Ownership_3 against OpenSplice, READER_NOT_MATCHED in
    /// about half the runs, captured on the Linux host).
    #[test]
    fn a_heartbeat_past_a_hole_releases_the_buffered_subscription() {
        use zerodds_rtps::message_builder::MessageBuilder;
        use zerodds_rtps::submessages::HeartbeatSubmessage;
        use zerodds_rtps::wire_types::SequenceNumber;
        use zerodds_rtps::{RtpsHeader, SubmessageId};

        let local = GuidPrefix::from_bytes([1; 12]);
        let owner_prefix = GuidPrefix::from_bytes([0xd0; 12]);
        let mut r = SedpStack::new(local, VendorId::ZERODDS);
        r.on_participant_discovered(&remote_participant(owner_prefix, DET));

        // The owner's second subscription, SN 2; SN 1 never reaches us.
        let mut owner = SedpStack::new(owner_prefix, VendorId::ZERODDS);
        owner
            .on_participant_discovered(&remote_participant(GuidPrefix::from_bytes([250; 12]), DET));
        let _first = owner
            .announce_subscription(&sample_sub(owner_prefix, "Other"))
            .unwrap();
        let mut second = sample_sub(owner_prefix, "Square");
        second.key = Guid::new(owner_prefix, EntityId::user_reader_with_key([0, 0, 2]));
        let ann = owner.announce_subscription(&second).unwrap();
        let decoded = zerodds_rtps::datagram::decode_datagram(&ann[0].bytes).unwrap();
        let data = decoded
            .contents()
            .find_map(|sm| match sm {
                ParsedSubmessage::Data(d) => Some(d),
                _ => None,
            })
            .expect("the announce carries a DATA submessage");
        assert_eq!(data.writer_sn, SequenceNumber(2));
        let (data_body, data_flags) = data.write_body(true);
        let mut b = MessageBuilder::open(
            RtpsHeader::new(VendorId::ZERODDS, owner_prefix),
            alloc::sync::Arc::new(alloc::vec![]),
            1400,
        );
        b.try_add_submessage(SubmessageId::Data, data_flags, &data_body)
            .unwrap();
        let multicast_data = b.finish().map(|d| d.bytes).unwrap();
        let ev = r
            .handle_datagram(&multicast_data, Duration::from_secs(1))
            .unwrap();
        assert!(ev.new_subscriptions.is_empty(), "SN 2 waits for SN 1");

        let hb = HeartbeatSubmessage {
            reader_id: EntityId::UNKNOWN,
            writer_id: EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
            first_sn: SequenceNumber(2),
            last_sn: SequenceNumber(2),
            count: 1,
            final_flag: false,
            liveliness_flag: false,
            group_info: None,
        };
        let (hb_body, hb_flags) = hb.write_body(true);
        let mut b = MessageBuilder::open(
            RtpsHeader::new(VendorId::ZERODDS, owner_prefix),
            alloc::sync::Arc::new(alloc::vec![]),
            1400,
        );
        b.try_add_submessage(SubmessageId::Heartbeat, hb_flags, &hb_body)
            .unwrap();
        let heartbeat = b.finish().map(|d| d.bytes).unwrap();
        let ev = r
            .handle_datagram(&heartbeat, Duration::from_secs(1))
            .unwrap();
        let topics: alloc::vec::Vec<&str> = ev
            .new_subscriptions
            .iter()
            .map(|s| s.topic_name.as_str())
            .collect();
        assert_eq!(topics, alloc::vec!["Square"]);
    }

    /// The owner participant was discovered individually (its own SPDP GUID
    /// ran through `on_participant_discovered`, giving the SEDP
    /// subscriptions reader a writer proxy for it) — the bundled DATA must
    /// be absorbed normally, `unmatched_writers` stays empty.
    #[test]
    fn ospl_bundled_sub_datagram_with_prior_discovery_matches() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let bundling_prefix = GuidPrefix::from_bytes([0x81; 12]);
        let owner_prefix = GuidPrefix::from_bytes([0x04; 12]);
        let mut r = SedpStack::new(local, VendorId::ZERODDS);
        r.on_participant_discovered(&remote_participant(bundling_prefix, DET));
        r.on_participant_discovered(&remote_participant(owner_prefix, DET));
        let dg = ospl_bundled_sub_datagram(bundling_prefix, local, owner_prefix, "Shape");
        let ev = r.handle_datagram(&dg, Duration::from_secs(1)).unwrap();
        assert_eq!(
            ev.new_subscriptions.len(),
            1,
            "the owner's DATA must be absorbed once it was individually discovered"
        );
        assert_eq!(ev.new_subscriptions[0].topic_name, "Shape");
        assert!(
            ev.unmatched_writers.is_empty(),
            "a discovered owner must not be reported unmatched: {:?}",
            ev.unmatched_writers
        );
    }

    /// The owner participant was **not** discovered individually — only the
    /// DDSI2 bundling service's own prefix went through
    /// `on_participant_discovered` (e.g. because only the bundling
    /// service's own SPDP announcement ever reached this participant). The
    /// SEDP subscriptions reader then has no writer proxy for
    /// `owner_prefix`, and the DATA that names it after INFO_SRC is dropped
    /// by `ReliableReader::handle_data`'s `unknown_src_count` path — with no
    /// GUID, no topic, nothing but a bare counter. `unmatched_writers` is
    /// the one place that gap now surfaces the specific writer GUID.
    ///
    /// This does not by itself prove this is *the* OpenSplice non-match —
    /// it shows the shape *can* silently drop a subscription in this stack
    /// when discovery only ever sees the bundling participant, and gives a
    /// concrete GUID to check against on the Linux host.
    #[test]
    fn ospl_bundled_sub_datagram_without_prior_discovery_is_unmatched() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let bundling_prefix = GuidPrefix::from_bytes([0x81; 12]);
        let owner_prefix = GuidPrefix::from_bytes([0x04; 12]);
        let mut r = SedpStack::new(local, VendorId::ZERODDS);
        // Only the bundling participant was ever discovered — `owner_prefix`
        // never got its own on_participant_discovered call.
        r.on_participant_discovered(&remote_participant(bundling_prefix, DET));
        let dg = ospl_bundled_sub_datagram(bundling_prefix, local, owner_prefix, "Shape");
        let ev = r.handle_datagram(&dg, Duration::from_secs(1)).unwrap();
        assert!(
            ev.new_subscriptions.is_empty(),
            "an undiscovered owner's DATA cannot be absorbed: {:?}",
            ev.new_subscriptions
        );
        assert_eq!(
            ev.unmatched_writers,
            alloc::vec![Guid::new(
                owner_prefix,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
            )],
            "the undiscovered owner's writer GUID must be named, not just silently dropped"
        );
    }

    #[test]
    fn routing_matches_exact_reader_id() {
        // Exakter reader_id-Match (writer_id irrelevant).
        assert!(routes_to_pub(
            EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
            EntityId::UNKNOWN
        ));
        assert!(routes_to_sub(
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
            EntityId::UNKNOWN
        ));
        assert!(routes_to_sec_pub(
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
            EntityId::UNKNOWN
        ));
        assert!(routes_to_sec_sub(
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
            EntityId::UNKNOWN
        ));
    }

    #[test]
    fn routing_resolves_unknown_reader_via_writer_id() {
        // Regression H-5/L-1: reader_id=UNKNOWN (cyclone INFO_DST) is resolved
        // via writer_id — applies to DATA/DATA_FRAG/GAP/HEARTBEAT.
        assert!(routes_to_pub(
            EntityId::UNKNOWN,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER
        ));
        assert!(routes_to_sub(
            EntityId::UNKNOWN,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
        ));
        assert!(routes_to_sec_pub(
            EntityId::UNKNOWN,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER
        ));
        assert!(routes_to_sec_sub(
            EntityId::UNKNOWN,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER
        ));
    }

    #[test]
    fn routing_unknown_with_wrong_writer_does_not_match() {
        // UNKNOWN reader_id + wrong writer_id → no match (no misrouting).
        assert!(!routes_to_pub(
            EntityId::UNKNOWN,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
        ));
        assert!(!routes_to_sub(
            EntityId::UNKNOWN,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER
        ));
        // An unknown (user) writer with UNKNOWN reader_id matches none.
        let foreign = EntityId::user_writer_with_key([0xAA, 0xBB, 0xCC]);
        assert!(!routes_to_pub(EntityId::UNKNOWN, foreign));
        assert!(!routes_to_sub(EntityId::UNKNOWN, foreign));
        assert!(!routes_to_sec_pub(EntityId::UNKNOWN, foreign));
        assert!(!routes_to_sec_sub(EntityId::UNKNOWN, foreign));
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
        // Remote has only the publications side (announcer + detector)
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
    fn two_live_participants_sharing_a_locator_keep_both_proxies() {
        // DDSI-RTPS invariant: a ReaderProxy is identified by its FULL GUID; a
        // locator is a transport target, not endpoint identity. Two distinct
        // LIVE participants announcing the same unicast locator (relay /
        // discovery-server / NAT / container) must BOTH stay matched. INFO_DST
        // preserves this (removes no proxies); locator-collision supersession
        // would violate it (this test failed with that approach).
        let mut s = SedpStack::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        let flags = endpoint_flag::PUBLICATIONS_DETECTOR | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
        s.on_participant_discovered(&remote_participant(GuidPrefix::from_bytes([2; 12]), flags));
        s.on_participant_discovered(&remote_participant(GuidPrefix::from_bytes([3; 12]), flags));
        assert_eq!(s.sub_writer().inner().reader_proxy_count(), 2);
        assert_eq!(s.pub_writer().inner().reader_proxy_count(), 2);
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
        // Confirm full state
        assert_eq!(s.pub_writer().inner().reader_proxy_count(), 1);

        s.on_participant_lost(remote_prefix);

        assert_eq!(s.pub_writer().inner().reader_proxy_count(), 0);
        assert_eq!(s.pub_reader().inner().writer_proxy_count(), 0);
        assert_eq!(s.sub_writer().inner().reader_proxy_count(), 0);
        assert_eq!(s.sub_reader().inner().writer_proxy_count(), 0);
    }

    #[test]
    fn end_to_end_discovery_between_two_stacks() {
        // Two stacks on two different prefixes, both announce
        // publications. After on_participant_discovered + handle_datagram
        // both see each other's publications in the cache.
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

    // --- issue #32: direct handle_datagram INFO_DST receiver-state tests ---
    // These assert state transitions (discovery events + cache), not just the
    // decoded enum value. DATA is exercised across all five scope cases; the
    // guard is byte-identical for DATA_FRAG/HEARTBEAT/GAP/ACKNACK/NACK_FRAG.

    const DET: u32 = endpoint_flag::PUBLICATIONS_DETECTOR
        | endpoint_flag::SUBSCRIPTIONS_DETECTOR
        | endpoint_flag::PUBLICATIONS_ANNOUNCER
        | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER;

    /// One SEDP publications DATA datagram from `src`, optionally prefixed with
    /// INFO_DST(`target`). The DATA payload comes from a real announce and is
    /// re-wrapped so the test controls the destination scope independently of
    /// the DATA bytes. `secure` routes it through the secure SEDP writer.
    fn sedp_pub_datagram(
        src: GuidPrefix,
        target: Option<[u8; 12]>,
        topic: &str,
        wkey: [u8; 3],
        secure: bool,
    ) -> alloc::vec::Vec<u8> {
        use zerodds_rtps::message_builder::MessageBuilder;
        use zerodds_rtps::{RtpsHeader, SubmessageId};
        let mut s = SedpStack::new(src, VendorId::ZERODDS);
        if secure {
            s.set_discovery_protected(true);
        }
        s.on_participant_discovered(&remote_participant(GuidPrefix::from_bytes([200; 12]), DET));
        let mut pd = sample_pub();
        pd.key = Guid::new(src, EntityId::user_writer_with_key(wkey));
        pd.participant_key = Guid::new(src, EntityId::PARTICIPANT);
        pd.topic_name = topic.into();
        let ann = s.announce_publication(&pd).unwrap();
        // The DATA is taken out of the announce by structure, not by offset:
        // a SEDP datagram opens with INFO_DST (issue #32), so the submessage
        // does not sit at a fixed place in the buffer.
        let decoded = zerodds_rtps::datagram::decode_datagram(&ann[0].bytes).unwrap();
        let data = decoded
            .contents()
            .find_map(|sm| match sm {
                zerodds_rtps::datagram::ParsedSubmessage::Data(d) => Some(d),
                _ => None,
            })
            .expect("the announce carries a DATA submessage");
        let (data_body, data_flags) = data.write_body(true);
        let hdr = RtpsHeader::new(VendorId::ZERODDS, src);
        let mut b = MessageBuilder::open(hdr, alloc::sync::Arc::new(alloc::vec![]), 1400);
        if let Some(dst) = target {
            b.try_add_submessage(SubmessageId::InfoDst, 0, &dst)
                .unwrap();
        }
        b.try_add_submessage(SubmessageId::Data, data_flags, &data_body)
            .unwrap();
        b.finish().map(|d| d.bytes).unwrap()
    }

    fn receiver(local: GuidPrefix, src: GuidPrefix, secure: bool) -> SedpStack {
        let mut r = SedpStack::new(local, VendorId::ZERODDS);
        if secure {
            r.set_discovery_protected(true);
        }
        r.on_participant_discovered(&remote_participant(src, DET));
        r
    }

    #[test]
    fn handle_datagram_without_info_dst_processes_for_local_receiver() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let src = GuidPrefix::from_bytes([2; 12]);
        let mut r = receiver(local, src, false);
        let dg = sedp_pub_datagram(src, None, "NoDst", [1, 0, 0], false);
        let ev = r.handle_datagram(&dg, Duration::from_secs(1)).unwrap();
        assert_eq!(ev.new_publications.len(), 1);
        assert_eq!(ev.new_publications[0].topic_name, "NoDst");
        assert_eq!(r.cache().publications_len(), 1);
    }

    #[test]
    fn handle_datagram_info_dst_matching_local_is_processed() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let src = GuidPrefix::from_bytes([2; 12]);
        let mut r = receiver(local, src, false);
        let dg = sedp_pub_datagram(src, Some(local.to_bytes()), "Local", [1, 0, 0], false);
        let ev = r.handle_datagram(&dg, Duration::from_secs(1)).unwrap();
        assert_eq!(ev.new_publications.len(), 1);
        assert_eq!(r.cache().publications_len(), 1);
    }

    #[test]
    fn handle_datagram_foreign_info_dst_is_fully_ignored() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let src = GuidPrefix::from_bytes([2; 12]);
        let foreign = [0x55u8; 12];
        let mut r = receiver(local, src, false);
        let before = r.cache().publications_len();
        let dg = sedp_pub_datagram(src, Some(foreign), "Foreign", [1, 0, 0], false);
        let ev = r.handle_datagram(&dg, Duration::from_secs(1)).unwrap();
        assert!(ev.is_empty(), "foreign INFO_DST must produce no events");
        assert_eq!(
            r.cache().publications_len(),
            before,
            "cache must not change"
        );
    }

    #[test]
    fn handle_datagram_unknown_info_dst_resets_to_local() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let src = GuidPrefix::from_bytes([2; 12]);
        let mut r = receiver(local, src, false);
        let dg = sedp_pub_datagram(src, Some([0u8; 12]), "Unknown", [1, 0, 0], false);
        let ev = r.handle_datagram(&dg, Duration::from_secs(1)).unwrap();
        assert_eq!(
            ev.new_publications.len(),
            1,
            "GUIDPREFIX_UNKNOWN resets to local"
        );
        assert_eq!(r.cache().publications_len(), 1);
    }

    #[test]
    fn handle_datagram_sequential_info_dst_scopes_only_following_submessages() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let src = GuidPrefix::from_bytes([2; 12]);
        let foreign = [0x55u8; 12];
        let mut r = receiver(local, src, false);
        // header + INFO_DST(foreign) + DATA_A + INFO_DST(local) + DATA_B
        let a = sedp_pub_datagram(src, Some(foreign), "SeqA", [1, 0, 0], false);
        let bpart = sedp_pub_datagram(src, Some(local.to_bytes()), "SeqB", [2, 0, 0], false);
        let mut dg = a.clone();
        dg.extend_from_slice(&bpart[20..]);
        let ev = r.handle_datagram(&dg, Duration::from_secs(1)).unwrap();
        assert_eq!(
            ev.new_publications.len(),
            1,
            "only the locally-scoped DATA is processed"
        );
        assert_eq!(ev.new_publications[0].topic_name, "SeqB");
        assert!(
            !r.cache()
                .publications()
                .any(|x| x.data.topic_name == "SeqA")
        );
    }

    /// Feed one HEARTBEAT for (`writer_id`,`reader_id`) scoped by INFO_DST(`dst`)
    /// and return how many samples the SEDP reader NACKs on the next tick. A
    /// processed HEARTBEAT (samples 1..=3 announced, none held) NACKs three; an
    /// ignored one leaves only the pre-emptive ACKNACK with zero NACKed samples.
    fn hb_nack_count(
        local: GuidPrefix,
        src: GuidPrefix,
        writer_id: EntityId,
        reader_id: EntityId,
        dst: [u8; 12],
        discover_flags: u32,
        protected: bool,
    ) -> usize {
        use zerodds_rtps::message_builder::MessageBuilder;
        use zerodds_rtps::submessages::HeartbeatSubmessage;
        use zerodds_rtps::wire_types::SequenceNumber;
        use zerodds_rtps::{RtpsHeader, SubmessageId};
        let mut r = SedpStack::new(local, VendorId::ZERODDS);
        if protected {
            r.set_discovery_protected(true);
        }
        r.on_participant_discovered(&remote_participant(src, discover_flags));
        let hb = HeartbeatSubmessage {
            reader_id,
            writer_id,
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(3),
            count: 1,
            final_flag: false,
            liveliness_flag: false,
            group_info: None,
        };
        let (body, flags) = hb.write_body(true);
        let hdr = RtpsHeader::new(VendorId::ZERODDS, src);
        let mut b = MessageBuilder::open(hdr, alloc::sync::Arc::new(alloc::vec![]), 1400);
        b.try_add_submessage(SubmessageId::InfoDst, 0, &dst)
            .unwrap();
        b.try_add_submessage(SubmessageId::Heartbeat, flags, &body)
            .unwrap();
        let dg = b.finish().map(|d| d.bytes).unwrap();
        r.handle_datagram(&dg, Duration::from_secs(1)).unwrap();
        let out = r.tick(Duration::from_secs(2)).unwrap();
        let mut nacked = 0;
        for od in &out {
            let parsed = zerodds_rtps::datagram::decode_datagram(&od.bytes).unwrap();
            for sm in parsed.submessages {
                if let ParsedSubmessage::AckNack(a) = sm.submessage {
                    if a.writer_id == writer_id {
                        nacked = nacked.max(a.reader_sn_state.iter_set().count());
                    }
                }
            }
        }
        nacked
    }

    #[test]
    fn handle_datagram_heartbeat_is_gated_by_info_dst() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let src = GuidPrefix::from_bytes([2; 12]);
        let foreign = [0x55u8; 12];
        // Local scope: the SEDP subscriptions reader reacts and NACKs 1..=3.
        assert_eq!(
            hb_nack_count(
                local,
                src,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
                local.to_bytes(),
                DET,
                false,
            ),
            3
        );
        // Foreign scope: ignored, no reactive NACK.
        assert_eq!(
            hb_nack_count(
                local,
                src,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
                foreign,
                DET,
                false,
            ),
            0
        );
    }

    #[test]
    fn handle_datagram_secure_sedp_heartbeat_is_gated_by_info_dst() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let src = GuidPrefix::from_bytes([2; 12]);
        let foreign = [0x55u8; 12];
        let flags = DET | endpoint_flag::ALL_SECURE;
        // Secure SEDP subscriptions builtin, local scope -> processed.
        assert_eq!(
            hb_nack_count(
                local,
                src,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
                local.to_bytes(),
                flags,
                true,
            ),
            3
        );
        // Foreign scope -> ignored on the secure endpoint too.
        assert_eq!(
            hb_nack_count(
                local,
                src,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
                foreign,
                flags,
                true,
            ),
            0
        );
    }

    // --- SEDP topics (DDSI-RTPS 2.5 §8.5.4.2) ---

    const TOPICS: u32 = endpoint_flag::TOPICS_ANNOUNCER | endpoint_flag::TOPICS_DETECTOR;

    #[test]
    fn the_topics_bits_wire_the_topics_writer_and_reader() {
        let mut s = SedpStack::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        let remote = GuidPrefix::from_bytes([2; 12]);
        s.on_participant_discovered(&remote_participant(remote, TOPICS));
        assert_eq!(s.topics_writer.inner().reader_proxy_count(), 1);
        assert_eq!(s.topics_reader.inner().writer_proxy_count(), 1);
        s.on_participant_lost(remote);
        assert_eq!(s.topics_writer.inner().reader_proxy_count(), 0);
        assert_eq!(s.topics_reader.inner().writer_proxy_count(), 0);
    }

    #[test]
    fn two_stacks_exchange_a_topic_its_new_qos_and_its_dispose() {
        let prefix_a = GuidPrefix::from_bytes([1; 12]);
        let prefix_b = GuidPrefix::from_bytes([2; 12]);
        let mut a = SedpStack::new(prefix_a, VendorId::ZERODDS);
        let mut b = SedpStack::new(prefix_b, VendorId::ZERODDS);
        a.on_participant_discovered(&remote_participant(prefix_b, TOPICS));
        b.on_participant_discovered(&remote_participant(prefix_a, TOPICS));
        let now = Duration::from_secs(1);

        let key = Guid::new(prefix_a, EntityId::topic([0, 0, 1]));
        let mut topic = TopicBuiltinTopicData::new(key, "Square", "ShapeType");
        let mut seen = Vec::new();
        for dg in a.announce_topic(&topic).unwrap() {
            seen.extend(b.handle_datagram(&dg.bytes, now).unwrap().new_topics);
        }
        assert_eq!(seen, alloc::vec![topic.clone()]);

        // A QoS change is the same instance announced again (Table 8.84).
        topic.history.depth = 9;
        let mut seen = Vec::new();
        for dg in a.announce_topic(&topic).unwrap() {
            seen.extend(b.handle_datagram(&dg.bytes, now).unwrap().new_topics);
        }
        assert_eq!(seen, alloc::vec![topic.clone()]);
        assert_eq!(b.discovered_topics().count(), 1);
        assert_eq!(b.discovered_topics().next().unwrap().history.depth, 9);

        let mut gone = Vec::new();
        for dg in a.dispose_topic(key).unwrap() {
            gone.extend(b.handle_datagram(&dg.bytes, now).unwrap().removed_topics);
        }
        assert_eq!(gone, alloc::vec![topic]);
        assert_eq!(b.discovered_topics().count(), 0);
    }

    #[test]
    fn protected_discovery_announces_no_topic() {
        let mut a = SedpStack::new(GuidPrefix::from_bytes([1; 12]), VendorId::ZERODDS);
        a.on_participant_discovered(&remote_participant(GuidPrefix::from_bytes([2; 12]), TOPICS));
        a.set_discovery_protected(true);
        let key = Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::topic([0, 0, 1]));
        let topic = TopicBuiltinTopicData::new(key, "Secret", "T");
        assert!(a.announce_topic(&topic).unwrap().is_empty());
        assert!(a.dispose_topic(key).unwrap().is_empty());
    }

    /// One DATA from `src`'s SEDP topics writer in a message whose header
    /// names `vendor`.
    fn topics_data_datagram(
        vendor: VendorId,
        src: GuidPrefix,
        sn: i64,
        key_flag: bool,
        inline_qos: Option<zerodds_rtps::parameter_list::ParameterList>,
        payload: alloc::vec::Vec<u8>,
    ) -> alloc::vec::Vec<u8> {
        use zerodds_rtps::message_builder::MessageBuilder;
        use zerodds_rtps::submessages::DataSubmessage;
        use zerodds_rtps::wire_types::SequenceNumber;
        use zerodds_rtps::{RtpsHeader, SubmessageId};
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SEDP_BUILTIN_TOPICS_READER,
            writer_id: EntityId::SEDP_BUILTIN_TOPICS_WRITER,
            writer_sn: SequenceNumber(sn),
            inline_qos,
            data_flag: !key_flag,
            key_flag,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let (body, flags) = data.write_body(true);
        let mut b = MessageBuilder::open(
            RtpsHeader::new(vendor, src),
            alloc::sync::Arc::new(alloc::vec![]),
            1400,
        );
        b.try_add_submessage(SubmessageId::Data, flags, &body)
            .unwrap();
        b.finish().map(|d| d.bytes).unwrap()
    }

    /// A topic announcement in Cyclone's form: the key in
    /// `PID_CYCLONE_TOPIC_GUID` (`0x801b`), no `PID_ENDPOINT_GUID`.
    fn cyclone_topic_payload(key: Guid) -> alloc::vec::Vec<u8> {
        use zerodds_rtps::parameter_list::{Parameter, ParameterList, pid};
        let mut pl = ParameterList::new();
        for (id, text) in [
            (pid::TOPIC_NAME, "TopicProbe"),
            (pid::TYPE_NAME, "tp::Probe"),
        ] {
            let mut v = alloc::vec::Vec::new();
            v.extend_from_slice(&(u32::try_from(text.len() + 1).unwrap()).to_le_bytes());
            v.extend_from_slice(text.as_bytes());
            v.push(0);
            while v.len() % 4 != 0 {
                v.push(0);
            }
            pl.push(Parameter::new(id, v));
        }
        pl.push(Parameter::new(
            zerodds_rtps::topic_data::PID_CYCLONE_TOPIC_GUID,
            key.to_bytes().to_vec(),
        ));
        pl.push(Parameter::new(
            pid::VENDOR_ID,
            alloc::vec![0x01, 0x10, 0, 0],
        ));
        let mut out = alloc::vec![0x00, 0x03, 0x00, 0x00];
        out.extend_from_slice(&pl.to_bytes(true));
        out
    }

    #[test]
    fn a_cyclone_topic_and_its_dispose_are_read_by_cyclones_own_key() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let cyclone = GuidPrefix::from_bytes([
            0x01, 0x10, 0xd5, 0xff, 0x26, 0x58, 0x3a, 0xe1, 0xaf, 0x16, 0x5a, 0xb9,
        ]);
        let mut s = SedpStack::new(local, VendorId::ZERODDS);
        s.on_participant_discovered(&remote_participant(cyclone, TOPICS));
        let key = Guid::new(cyclone, EntityId::topic([0, 0, 1]));
        let now = Duration::from_secs(1);

        let alive = topics_data_datagram(
            VendorId::CYCLONE,
            cyclone,
            1,
            false,
            None,
            cyclone_topic_payload(key),
        );
        let ev = s.handle_datagram(&alive, now).unwrap();
        assert_eq!(ev.new_topics.len(), 1);
        assert_eq!(ev.new_topics[0].key, key);
        assert_eq!(ev.new_topics[0].topic_name, "TopicProbe");

        // Cyclone's dispose: K=1, the serialized key a PL_CDR_BE list holding
        // PID_CYCLONE_TOPIC_GUID (`serdata_plist_from_keyhash`), the inline
        // QoS PID_STATUS_INFO alone.
        let mut serialized_key = alloc::vec![0x00, 0x02, 0x00, 0x00, 0x80, 0x1b, 0x00, 0x10];
        serialized_key.extend_from_slice(&key.to_bytes());
        serialized_key.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
        let mut inline_qos = zerodds_rtps::parameter_list::ParameterList::new();
        inline_qos.push(zerodds_rtps::inline_qos::status_info_param(
            zerodds_rtps::inline_qos::status_info::DISPOSED
                | zerodds_rtps::inline_qos::status_info::UNREGISTERED,
        ));
        let dead = topics_data_datagram(
            VendorId::CYCLONE,
            cyclone,
            2,
            true,
            Some(inline_qos),
            serialized_key,
        );
        let ev = s.handle_datagram(&dead, now).unwrap();
        assert_eq!(ev.removed_topics.len(), 1);
        assert_eq!(ev.removed_topics[0].key, key);
        assert_eq!(s.discovered_topics().count(), 0);
    }

    #[test]
    fn cyclones_key_parameter_from_another_vendor_is_refused_without_failing_the_datagram() {
        let local = GuidPrefix::from_bytes([1; 12]);
        let other = GuidPrefix::from_bytes([3; 12]);
        let mut s = SedpStack::new(local, VendorId::ZERODDS);
        s.on_participant_discovered(&remote_participant(other, TOPICS));
        let key = Guid::new(other, EntityId::topic([0, 0, 1]));
        let dg = topics_data_datagram(
            VendorId::FASTDDS,
            other,
            1,
            false,
            None,
            cyclone_topic_payload(key),
        );
        let ev = s.handle_datagram(&dg, Duration::from_secs(1)).unwrap();
        assert!(ev.new_topics.is_empty());
        assert_eq!(ev.refused.len(), 1);
        assert_eq!(
            ev.refused[0].0,
            Guid::new(other, EntityId::SEDP_BUILTIN_TOPICS_WRITER)
        );
        assert!(matches!(
            ev.refused[0].1,
            SedpReaderError::InvalidPayload { .. }
        ));
    }

    #[test]
    fn a_lost_participant_takes_its_topics_with_it() {
        let prefix_a = GuidPrefix::from_bytes([1; 12]);
        let prefix_b = GuidPrefix::from_bytes([2; 12]);
        let mut a = SedpStack::new(prefix_a, VendorId::ZERODDS);
        let mut b = SedpStack::new(prefix_b, VendorId::ZERODDS);
        a.on_participant_discovered(&remote_participant(prefix_b, TOPICS));
        b.on_participant_discovered(&remote_participant(prefix_a, TOPICS));
        let topic = TopicBuiltinTopicData::new(
            Guid::new(prefix_a, EntityId::topic([0, 0, 1])),
            "T",
            "M::T",
        );
        for dg in a.announce_topic(&topic).unwrap() {
            b.handle_datagram(&dg.bytes, Duration::from_secs(1))
                .unwrap();
        }
        assert_eq!(b.topics_of(prefix_a), alloc::vec![topic]);
        b.on_participant_lost(prefix_a);
        assert!(b.topics_of(prefix_a).is_empty());
        assert_eq!(b.discovered_topics().count(), 0);
    }

    /// A publication announcement that does not decode is refused by its
    /// writer's GUID, and the subscription announcement in the same
    /// datagram is still read. Before, the first one's error failed the
    /// whole datagram, and the runtime dropped every event in it.
    #[test]
    fn a_sample_that_does_not_decode_costs_only_itself() {
        use zerodds_rtps::message_builder::MessageBuilder;
        use zerodds_rtps::submessages::DataSubmessage;
        use zerodds_rtps::wire_types::SequenceNumber;
        use zerodds_rtps::{RtpsHeader, SubmessageId};

        let local = GuidPrefix::from_bytes([1; 12]);
        let remote = GuidPrefix::from_bytes([0xd1; 12]);
        let mut r = SedpStack::new(local, VendorId::ZERODDS);
        r.on_participant_discovered(&remote_participant(remote, DET));

        // A valid subscription DATA, taken from a real announcement.
        let mut owner = SedpStack::new(remote, VendorId::ZERODDS);
        owner
            .on_participant_discovered(&remote_participant(GuidPrefix::from_bytes([250; 12]), DET));
        let ann = owner
            .announce_subscription(&sample_sub(remote, "StillRead"))
            .unwrap();
        let decoded = zerodds_rtps::datagram::decode_datagram(&ann[0].bytes).unwrap();
        let sub_data = decoded
            .contents()
            .find_map(|sm| match sm {
                ParsedSubmessage::Data(d) => Some(d),
                _ => None,
            })
            .expect("the announce carries a DATA submessage");
        let (sub_body, sub_flags) = sub_data.write_body(true);

        // A publication DATA whose ParameterList does not decode.
        let bad = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
            writer_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::vec![0x00, 0x03, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF].into(),
        };
        let (bad_body, bad_flags) = bad.write_body(true);

        let mut b = MessageBuilder::open(
            RtpsHeader::new(VendorId::ZERODDS, remote),
            alloc::sync::Arc::new(alloc::vec![]),
            1400,
        );
        b.try_add_submessage(SubmessageId::Data, bad_flags, &bad_body)
            .unwrap();
        b.try_add_submessage(SubmessageId::Data, sub_flags, &sub_body)
            .unwrap();
        let datagram = b.finish().map(|d| d.bytes).unwrap();

        let ev = r
            .handle_datagram(&datagram, Duration::from_secs(1))
            .unwrap();
        assert_eq!(ev.refused.len(), 1);
        assert_eq!(
            ev.refused[0].0,
            Guid::new(remote, EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER)
        );
        assert!(matches!(
            ev.refused[0].1,
            SedpReaderError::InvalidPayload { .. }
        ));
        let topics: Vec<&str> = ev
            .new_subscriptions
            .iter()
            .map(|s| s.topic_name.as_str())
            .collect();
        assert_eq!(topics, alloc::vec!["StillRead"]);
    }
}
