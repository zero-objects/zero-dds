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
use crate::sedp::reader::{
    SedpDelta, SedpPublicationsReader, SedpReaderError, SedpSubscriptionsReader,
};
use crate::sedp::writer::{SedpPublicationsWriter, SedpSubscriptionsWriter};
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
}

impl SedpEvents {
    /// True if no discovery (additions or removals) was carried.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.new_publications.is_empty()
            && self.new_subscriptions.is_empty()
            && self.removed_publications.is_empty()
            && self.removed_subscriptions.is_empty()
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
    // DDS-Security §8.4.2.4 `is_discovery_protected`: parallel secure SEDP
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
        }
    }

    /// Enables protected discovery (DDS-Security §8.4.2.4): future
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

    /// Wires up the SEDP endpoints for a newly discovered
    /// participant. Reads `p.data.builtin_endpoint_set` + unicast
    /// locators and registers the matching proxies.
    pub fn on_participant_discovered(&mut self, p: &DiscoveredParticipant) {
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

        // --- Secure SEDP (DDS-Security §8.4.2.4): proxies for the secure
        // discovery endpoints, when the peer announces them. Mirrors the
        // plaintext wiring with the SECURE EntityIds. ---
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
        delta: SedpDelta<PublicationBuiltinTopicData>,
        now: Duration,
        events: &mut SedpEvents,
    ) {
        for p in delta.added {
            self.cache.insert_publication(p.clone(), now);
            events.new_publications.push(p);
        }
        for g in delta.removed {
            self.cache.remove_publication(g);
            events.removed_publications.push(g);
        }
    }

    /// Subscriptions counterpart of [`Self::absorb_pub_delta`].
    fn absorb_sub_delta(
        &mut self,
        delta: SedpDelta<SubscriptionBuiltinTopicData>,
        now: Duration,
        events: &mut SedpEvents,
    ) {
        for s in delta.added {
            self.cache.insert_subscription(s.clone(), now);
            events.new_subscriptions.push(s);
        }
        for g in delta.removed {
            self.cache.remove_subscription(g);
            events.removed_subscriptions.push(g);
        }
    }

    /// Processes an incoming RTPS datagram into the SEDP stack.
    /// Routes the submessages by target EntityId to the matching
    /// endpoint and updates the cache. Returns the [`SedpEvents`] delta —
    /// newly discovered endpoints **and** the GUIDs of endpoints the remote
    /// disposed (deleted).
    ///
    /// # Errors
    /// Wire decode error or malformed PL_CDR payloads.
    pub fn handle_datagram(
        &mut self,
        datagram: &[u8],
        now: Duration,
    ) -> Result<SedpEvents, SedpReaderError> {
        let parsed = decode_datagram(datagram).map_err(SedpReaderError::from)?;
        // DDSI-RTPS §8.3.4: effective writer source = sourceGuidPrefix +
        // writerId. The prefix comes from the RTPS header of the datagram and
        // is passed to every reader dispatch so that multiple remote
        // writers with the same EntityId do not collide in the same proxy.
        let source_prefix = parsed.header.guid_prefix;
        let mut events = SedpEvents::default();
        for sub in parsed.submessages {
            match sub {
                ParsedSubmessage::Data(d) => {
                    // reader_id match with UNKNOWN→writer_id fallback (§8.3.7.2).
                    if routes_to_pub(d.reader_id, d.writer_id) {
                        let delta = self.pub_reader.handle_data(source_prefix, &d)?;
                        self.absorb_pub_delta(delta, now, &mut events);
                    } else if routes_to_sub(d.reader_id, d.writer_id) {
                        let delta = self.sub_reader.handle_data(source_prefix, &d)?;
                        self.absorb_sub_delta(delta, now, &mut events);
                    } else if routes_to_sec_pub(d.reader_id, d.writer_id) {
                        let delta = self.secure_pub_reader.handle_data(source_prefix, &d)?;
                        self.absorb_pub_delta(delta, now, &mut events);
                    } else if routes_to_sec_sub(d.reader_id, d.writer_id) {
                        let delta = self.secure_sub_reader.handle_data(source_prefix, &d)?;
                        self.absorb_sub_delta(delta, now, &mut events);
                    }
                }
                ParsedSubmessage::DataFrag(df) => {
                    if routes_to_pub(df.reader_id, df.writer_id) {
                        let delta = self.pub_reader.handle_data_frag(source_prefix, &df, now)?;
                        self.absorb_pub_delta(delta, now, &mut events);
                    } else if routes_to_sub(df.reader_id, df.writer_id) {
                        let delta = self.sub_reader.handle_data_frag(source_prefix, &df, now)?;
                        self.absorb_sub_delta(delta, now, &mut events);
                    } else if routes_to_sec_pub(df.reader_id, df.writer_id) {
                        let delta =
                            self.secure_pub_reader
                                .handle_data_frag(source_prefix, &df, now)?;
                        self.absorb_pub_delta(delta, now, &mut events);
                    } else if routes_to_sec_sub(df.reader_id, df.writer_id) {
                        let delta =
                            self.secure_sub_reader
                                .handle_data_frag(source_prefix, &df, now)?;
                        self.absorb_sub_delta(delta, now, &mut events);
                    }
                }
                ParsedSubmessage::Heartbeat(h) => {
                    // Cyclone often sends HEARTBEATs with reader_id=UNKNOWN and
                    // uses INFO_DST for addressing — dispatch then by
                    // writer_id to the matching SEDP reader.
                    if routes_to_pub(h.reader_id, h.writer_id) {
                        self.pub_reader.handle_heartbeat(source_prefix, &h, now);
                    } else if routes_to_sub(h.reader_id, h.writer_id) {
                        self.sub_reader.handle_heartbeat(source_prefix, &h, now);
                    } else if routes_to_sec_pub(h.reader_id, h.writer_id) {
                        self.secure_pub_reader
                            .handle_heartbeat(source_prefix, &h, now);
                    } else if routes_to_sec_sub(h.reader_id, h.writer_id) {
                        self.secure_sub_reader
                            .handle_heartbeat(source_prefix, &h, now);
                    }
                }
                ParsedSubmessage::Gap(g) => {
                    // GAP shares the UNKNOWN→writer_id fallback of the HEARTBEAT/
                    // DATA paths (H-5): without it the reliable SEDP reader
                    // stalls against cyclone, which sends GAP with reader_id=UNKNOWN.
                    if routes_to_pub(g.reader_id, g.writer_id) {
                        let delta = self.pub_reader.handle_gap(source_prefix, &g)?;
                        self.absorb_pub_delta(delta, now, &mut events);
                    } else if routes_to_sub(g.reader_id, g.writer_id) {
                        let delta = self.sub_reader.handle_gap(source_prefix, &g)?;
                        self.absorb_sub_delta(delta, now, &mut events);
                    } else if routes_to_sec_pub(g.reader_id, g.writer_id) {
                        let delta = self.secure_pub_reader.handle_gap(source_prefix, &g)?;
                        self.absorb_pub_delta(delta, now, &mut events);
                    } else if routes_to_sec_sub(g.reader_id, g.writer_id) {
                        let delta = self.secure_sub_reader.handle_gap(source_prefix, &g)?;
                        self.absorb_sub_delta(delta, now, &mut events);
                    }
                }
                ParsedSubmessage::AckNack(ack) => {
                    // Dispatch to writer by writer_id
                    let base = ack.reader_sn_state.bitmap_base;
                    let requested: Vec<_> = ack.reader_sn_state.iter_set().collect();
                    let src = Guid::new(parsed.header.guid_prefix, ack.reader_id);
                    if ack.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER {
                        self.pub_writer.handle_acknack(src, base, requested);
                    } else if ack.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER {
                        self.sub_writer.handle_acknack(src, base, requested);
                    } else if ack.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER {
                        self.secure_pub_writer.handle_acknack(src, base, requested);
                    } else if ack.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER {
                        self.secure_sub_writer.handle_acknack(src, base, requested);
                    }
                }
                ParsedSubmessage::NackFrag(nf) => {
                    let src = Guid::new(parsed.header.guid_prefix, nf.reader_id);
                    if nf.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER {
                        self.secure_pub_writer.handle_nackfrag(src, &nf);
                    } else if nf.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER {
                        self.secure_sub_writer.handle_nackfrag(src, &nf);
                    } else if nf.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER {
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
        Ok(out)
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
                participant_security_info: None,
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
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
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
}
