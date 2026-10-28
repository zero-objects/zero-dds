//! TypeLookup endpoint wiring over the shared reliable RTPS transport.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::time::Duration;
use zerodds_rtps::WireError;
use zerodds_rtps::datagram::{ParsedSubmessage, ReceiveContext, decode_datagram};
use zerodds_rtps::fragment_assembler::AssemblerCaps;
use zerodds_rtps::history_cache::HistoryKind;
use zerodds_rtps::message_builder::{DEFAULT_MTU, OutboundDatagram};
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::reliable_reader::{
    DEFAULT_HEARTBEAT_RESPONSE_DELAY, DeliveredSample, ReliableReader, ReliableReaderConfig,
};
use zerodds_rtps::reliable_writer::{
    DEFAULT_HEARTBEAT_PERIOD, ReliableWriter, ReliableWriterConfig,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, VendorId};
use zerodds_rtps::writer_proxy::WriterProxy;

struct Channel {
    writer: ReliableWriter,
    reader: ReliableReader,
}

impl Channel {
    fn new(prefix: GuidPrefix, writer_id: EntityId, reader_id: EntityId) -> Self {
        Self {
            writer: ReliableWriter::new(ReliableWriterConfig {
                guid: Guid::new(prefix, writer_id),
                vendor_id: VendorId::ZERODDS,
                reader_proxies: Vec::new(),
                max_samples: 1024,
                // §7.6.3.3: the HISTORY policy of the four built-in endpoints
                // is KEEP_ALL; a request or reply is not dropped for a newer one.
                history_kind: HistoryKind::KeepAll,
                heartbeat_period: DEFAULT_HEARTBEAT_PERIOD,
                // Leave room for RPC correlation inline QoS in every fragment.
                fragment_size: 1024,
                mtu: DEFAULT_MTU,
            }),
            reader: ReliableReader::new(ReliableReaderConfig {
                guid: Guid::new(prefix, reader_id),
                vendor_id: VendorId::ZERODDS,
                writer_proxies: Vec::new(),
                max_samples_per_proxy: 32,
                // DDSI-RTPS 2.5 §8.4.10.1, as the SEDP readers.
                heartbeat_response_delay: DEFAULT_HEARTBEAT_RESPONSE_DELAY,
                assembler_caps: AssemblerCaps::default(),
            }),
        }
    }

    fn peer(&mut self, prefix: GuidPrefix, locator: Locator) {
        let reader_guid = Guid::new(prefix, self.reader.guid().entity_id);
        if !self
            .writer
            .refresh_reader_proxy_locators(reader_guid, alloc::vec![locator], Vec::new())
        {
            let mut proxy = ReaderProxy::new(
                Guid::new(prefix, self.reader.guid().entity_id),
                alloc::vec![locator],
                Vec::new(),
                true,
            );
            // XTypes 1.3 §7.6.3.3.3: the endpoints are VOLATILE. A peer that
            // matches after a request or reply was written starts behind it,
            // or the writer would push it there (DDSI-RTPS 2.5 §8.4.9.2,
            // Table 8.66 T4), as the ParticipantVolatileMessageSecure writer
            // does.
            let floor = self.writer.volatile_join_floor();
            if floor.0 > 0 {
                proxy.skip_samples_up_to(floor);
            }
            self.writer.add_reader_proxy(proxy);
        }
        self.reader.add_writer_proxy(WriterProxy::new(
            Guid::new(prefix, self.writer.guid().entity_id),
            alloc::vec![locator],
            Vec::new(),
            true,
        ));
    }
}

struct PeerObservation {
    expires_at: Option<Duration>,
    spdp: bool,
}

pub(crate) struct TypeLookupTransport {
    prefix: GuidPrefix,
    /// The writer ids of the request and reply channel: the TypeLookup
    /// endpoints of XTypes 1.3 §7.6.3.3 or, for [`Self::new_secure`], the
    /// secure ones (DDS-Security 1.2 §7.5.5.2, Table 9).
    request_writer: EntityId,
    reply_writer: EntityId,
    request: Channel,
    reply: Channel,
    peers: BTreeMap<GuidPrefix, PeerObservation>,
    fallback_lease: Duration,
}

impl TypeLookupTransport {
    pub(crate) fn new(prefix: GuidPrefix, fallback_lease: Duration) -> Self {
        Self::with_endpoints(
            prefix,
            fallback_lease,
            [
                EntityId::TL_SVC_REQ_WRITER,
                EntityId::TL_SVC_REQ_READER,
                EntityId::TL_SVC_REPLY_WRITER,
                EntityId::TL_SVC_REPLY_READER,
            ],
        )
    }

    /// The TypeLookup service on the secure builtin endpoints
    /// (DDS-Security 1.2 §7.5.5.2): the same data types and QoS, the
    /// EntityIds of Table 9.
    pub(crate) fn new_secure(prefix: GuidPrefix, fallback_lease: Duration) -> Self {
        Self::with_endpoints(
            prefix,
            fallback_lease,
            [
                EntityId::TL_SVC_REQ_SECURE_WRITER,
                EntityId::TL_SVC_REQ_SECURE_READER,
                EntityId::TL_SVC_REPLY_SECURE_WRITER,
                EntityId::TL_SVC_REPLY_SECURE_READER,
            ],
        )
    }

    fn with_endpoints(prefix: GuidPrefix, fallback_lease: Duration, ids: [EntityId; 4]) -> Self {
        let [request_writer, request_reader, reply_writer, reply_reader] = ids;
        Self {
            prefix,
            request_writer,
            reply_writer,
            peers: BTreeMap::new(),
            fallback_lease,
            request: Channel::new(prefix, request_writer, request_reader),
            reply: Channel::new(prefix, reply_writer, reply_reader),
        }
    }

    pub(crate) fn write(
        &mut self,
        request: bool,
        peer: GuidPrefix,
        locator: Locator,
        payload: &[u8],
        inline_qos: Option<zerodds_rtps::parameter_list::ParameterList>,
        now: Duration,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.observe_traffic(peer, now);
        let channel = if request {
            &mut self.request
        } else {
            &mut self.reply
        };
        channel.peer(peer, locator);
        channel
            .writer
            .write_sample_with_inline_qos(payload, None, None, inline_qos)
    }

    pub(crate) fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        self.expire(now);
        let mut output = Vec::new();
        for channel in [&mut self.request, &mut self.reply] {
            output.extend(channel.writer.tick(now)?);
            output.extend(channel.reader.tick_outbound(now)?);
        }
        Ok(output)
    }

    #[cfg(test)]
    pub(crate) fn receive(
        &mut self,
        bytes: &[u8],
        locator: Locator,
        now: Duration,
    ) -> Result<(bool, Vec<DeliveredSample>), WireError> {
        self.receive_in(bytes, locator, &ReceiveContext::default(), now)
    }

    /// [`Self::receive`] for a datagram that arrived in `context`
    /// (DDSI-RTPS 2.5 §8.3.4).
    pub(crate) fn receive_in(
        &mut self,
        bytes: &[u8],
        locator: Locator,
        context: &ReceiveContext,
        now: Duration,
    ) -> Result<(bool, Vec<DeliveredSample>), WireError> {
        self.expire(now);
        let parsed = decode_datagram(bytes)?;
        let mut accepted = false;
        let mut samples = Vec::new();
        let mut accepted_prefixes: Vec<GuidPrefix> = Vec::new();
        // DDSI-RTPS 2.5 §8.3.4: `receiver_walk` gives the sourceGuidPrefix
        // and destGuidPrefix (InfoSource/InfoDestination) in force for each
        // submessage, rather than reading them once from the datagram
        // header — a bundled datagram can carry more than one source.
        for (state, sub) in parsed.receiver_walk_in(self.prefix, context) {
            let prefix = state.source_guid_prefix;
            if prefix == self.prefix {
                // Our own loopback (e.g. multicast echo) — never a peer.
                continue;
            }
            let destination = state.dest_guid_prefix == self.prefix
                || state.dest_guid_prefix == GuidPrefix::UNKNOWN;
            if !destination {
                continue;
            }
            let (writer_id, reader_id) = match sub {
                ParsedSubmessage::Data(d) => (d.writer_id, d.reader_id),
                ParsedSubmessage::DataFrag(d) => (d.writer_id, d.reader_id),
                ParsedSubmessage::Heartbeat(d) => (d.writer_id, d.reader_id),
                ParsedSubmessage::HeartbeatFrag(d) => (d.writer_id, d.reader_id),
                ParsedSubmessage::Gap(d) => (d.writer_id, d.reader_id),
                ParsedSubmessage::AckNack(d) => (d.writer_id, d.reader_id),
                ParsedSubmessage::NackFrag(d) => (d.writer_id, d.reader_id),
                _ => continue,
            };
            let channel = if writer_id == self.request_writer {
                &mut self.request
            } else if writer_id == self.reply_writer {
                &mut self.reply
            } else {
                continue;
            };
            if reader_id != EntityId::UNKNOWN && reader_id != channel.reader.guid().entity_id {
                continue;
            }
            accepted = true;
            // Observation is performed after this channel borrow ends below.
            channel.peer(prefix, locator);
            match sub {
                ParsedSubmessage::Data(d) => {
                    samples.extend(channel.reader.handle_data(prefix, d, None))
                }
                ParsedSubmessage::DataFrag(d) => {
                    samples.extend(channel.reader.handle_data_frag(prefix, d, now, None))
                }
                ParsedSubmessage::Heartbeat(d) => {
                    samples.extend(channel.reader.handle_heartbeat(prefix, d, now))
                }
                ParsedSubmessage::Gap(d) => samples.extend(channel.reader.handle_gap(prefix, d)),
                ParsedSubmessage::AckNack(d) => {
                    let src = Guid::new(prefix, d.reader_id);
                    channel.writer.handle_acknack(
                        src,
                        d.reader_sn_state.bitmap_base,
                        d.reader_sn_state.iter_set(),
                    );
                    channel.writer.note_acknack_final_flag(src, d.final_flag);
                }
                ParsedSubmessage::NackFrag(d) => channel
                    .writer
                    .handle_nackfrag(Guid::new(prefix, d.reader_id), d),
                // Fragment arrival already schedules the shared reader's NACK_FRAG.
                _ => {}
            }
            if !accepted_prefixes.contains(&prefix) {
                accepted_prefixes.push(prefix);
            }
        }
        // Observe each distinct accepted source once — a bundled datagram
        // can carry submessages from more than one (DDSI-RTPS 2.5 §8.3.4
        // InfoSource), but the common case is the single previous call.
        for prefix in accepted_prefixes {
            self.observe_traffic(prefix, now);
        }
        Ok((accepted, samples))
    }

    pub(crate) fn observe_spdp(&mut self, prefix: GuidPrefix) {
        self.peers.insert(
            prefix,
            PeerObservation {
                // The central SPDP cache alone decides known-participant expiry.
                expires_at: None,
                spdp: true,
            },
        );
    }

    pub(crate) fn remove_peer(&mut self, prefix: GuidPrefix) {
        self.peers.remove(&prefix);
        for channel in [&mut self.request, &mut self.reply] {
            channel
                .writer
                .remove_reader_proxy(Guid::new(prefix, channel.reader.guid().entity_id));
            channel
                .reader
                .remove_writer_proxy(Guid::new(prefix, channel.writer.guid().entity_id));
        }
    }

    fn observe_traffic(&mut self, prefix: GuidPrefix, now: Duration) {
        self.expire(now);
        let observation = self.peers.entry(prefix).or_insert(PeerObservation {
            expires_at: None,
            spdp: false,
        });
        if !observation.spdp {
            observation.expires_at = Some(now.saturating_add(self.fallback_lease));
        }
    }

    fn expire(&mut self, now: Duration) {
        let expired: Vec<_> = self
            .peers
            .iter()
            .filter_map(|(prefix, observation)| {
                observation
                    .expires_at
                    .filter(|deadline| *deadline <= now)
                    .map(|_| *prefix)
            })
            .collect();
        for prefix in expired {
            self.remove_peer(prefix);
        }
    }
}

impl TypeLookupTransport {
    /// The locators this side's service writers address `peer` at — what
    /// `receive` and `write` last set for it.
    #[cfg(test)]
    pub(crate) fn locators_of(&self, peer: GuidPrefix) -> Vec<Locator> {
        [&self.request, &self.reply]
            .iter()
            .flat_map(|channel| channel.writer.reader_proxies().iter())
            .filter(|proxy| proxy.remote_reader_guid.prefix == peer)
            .flat_map(|proxy| proxy.unicast_locators.iter().copied())
            .collect()
    }
}

/// Whether `bytes` holds a submessage [`TypeLookupTransport::receive`]
/// acts on, read from the submessage headers alone: a DATA, DATA_FRAG,
/// HEARTBEAT, HEARTBEAT_FRAG, GAP, ACKNACK or NACK_FRAG whose writerId is
/// `TL_SVC_REQ_WRITER` or `TL_SVC_REPLY_WRITER`. The writerId follows the
/// readerId, behind extraFlags and octetsToInlineQos in DATA and DATA_FRAG
/// (DDSI-RTPS 2.5 §9.4.5.3, §9.4.5.4) and at the start of the others. A
/// message whose headers do not walk to its end answers `true`, and the
/// full decode decides.
///
/// The receive path asked `receive` about every user datagram, and
/// `receive` decodes the whole datagram before it looks at a writerId:
/// every DATA of a user stream was decoded twice, its payload and inline
/// QoS allocated and copied twice. Measured on the Linux test host, the
/// receive thread of a 1 kB BEST_EFFORT stream was saturated at 1.2 µs per
/// sample.
pub(crate) fn may_carry_type_lookup(bytes: &[u8]) -> bool {
    use zerodds_rtps::submessage_header::SubmessageId;
    const RTPS_HEADER: usize = zerodds_rtps::message_builder::RTPS_HEADER_LEN;
    if bytes.len() < RTPS_HEADER || &bytes[..4] != b"RTPS" {
        return true;
    }
    let services = [
        EntityId::TL_SVC_REQ_WRITER.to_bytes(),
        EntityId::TL_SVC_REPLY_WRITER.to_bytes(),
        EntityId::TL_SVC_REQ_SECURE_WRITER.to_bytes(),
        EntityId::TL_SVC_REPLY_SECURE_WRITER.to_bytes(),
    ];
    let mut pos = RTPS_HEADER;
    while pos < bytes.len() {
        let Some(header) = bytes.get(pos..pos + 4) else {
            return true;
        };
        let id = header[0];
        let raw = [header[2], header[3]];
        let len = usize::from(if header[1] & 0x01 != 0 {
            u16::from_le_bytes(raw)
        } else {
            u16::from_be_bytes(raw)
        });
        let body = pos + 4;
        // §9.4.5.1.3: a zero length makes every submessage but PAD and
        // INFO_TS the last one, up to the end of the message.
        let last =
            len == 0 && id != SubmessageId::Pad.as_u8() && id != SubmessageId::InfoTs.as_u8();
        let end = if last { bytes.len() } else { body + len };
        if end > bytes.len() {
            return true;
        }
        let writer_at = if id == SubmessageId::Data.as_u8() || id == SubmessageId::DataFrag.as_u8()
        {
            Some(8)
        } else if [
            SubmessageId::Heartbeat,
            SubmessageId::HeartbeatFrag,
            SubmessageId::Gap,
            SubmessageId::AckNack,
            SubmessageId::NackFrag,
        ]
        .iter()
        .any(|kind| kind.as_u8() == id)
        {
            Some(4)
        } else {
            None
        };
        if let Some(at) = writer_at {
            let Some(writer) = bytes
                .get(body + at..body + at + 4)
                .filter(|_| body + at + 4 <= end)
            else {
                return true;
            };
            if services.iter().any(|service| writer == service.as_slice()) {
                return true;
            }
        }
        if last {
            break;
        }
        pos = end;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_rtps::wire_types::SequenceNumber;

    #[test]
    fn lease_expiry_resets_peer_and_locator_refresh_preserves_ack_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let local_prefix = GuidPrefix::from_bytes([1; 12]);
        let peer_prefix = GuidPrefix::from_bytes([2; 12]);
        let old = Locator::udp_v4([127, 0, 0, 1], 10000);
        let new = Locator::udp_v4([127, 0, 0, 1], 10001);
        let mut local = TypeLookupTransport::new(local_prefix, Duration::from_secs(5));
        local.observe_spdp(peer_prefix);
        local.write(true, peer_prefix, old, &[0, 7, 0, 0], None, Duration::ZERO)?;
        local.request.writer.handle_acknack(
            Guid::new(peer_prefix, EntityId::TL_SVC_REQ_READER),
            SequenceNumber(2),
            [],
        );
        let output = local.write(
            true,
            peer_prefix,
            new,
            &[0, 7, 0, 0],
            None,
            Duration::from_secs(1),
        )?;
        assert!(!output.is_empty());
        assert!(
            output
                .iter()
                .all(|datagram| datagram.targets.as_ref() == &alloc::vec![new])
        );
        let proxy = local
            .request
            .writer
            .reader_proxies()
            .first()
            .ok_or("missing reader proxy")?;
        assert_eq!(proxy.highest_acked_sn(), SequenceNumber(1));
        assert_eq!(
            local.request.writer.cache().max_sn(),
            Some(SequenceNumber(2))
        );

        let mut remote = TypeLookupTransport::new(peer_prefix, Duration::from_secs(5));
        let reply = remote.write(
            false,
            local_prefix,
            old,
            &[0, 7, 0, 0],
            None,
            Duration::ZERO,
        )?;
        let bytes = &reply.first().ok_or("missing reply DATA")?.bytes;
        assert_eq!(
            local.receive(bytes, new, Duration::from_secs(1))?.1.len(),
            1
        );
        assert!(
            local
                .receive(bytes, new, Duration::from_secs(1))?
                .1
                .is_empty()
        );
        // The central SPDP expiry decision removes all TL state.
        local.remove_peer(peer_prefix);
        assert!(local.peers.is_empty());
        assert_eq!(local.request.writer.reader_proxy_count(), 0);
        assert_eq!(local.reply.reader.writer_proxy_count(), 0);
        // The same prefix and SN are accepted again only after expiry.
        assert_eq!(
            local.receive(bytes, new, Duration::from_secs(3))?.1.len(),
            1
        );
        local.tick(Duration::from_secs(7))?;
        assert_eq!(local.reply.reader.writer_proxy_count(), 1);
        local.tick(Duration::from_secs(8))?;
        assert_eq!(local.reply.reader.writer_proxy_count(), 0);
        assert!(local.peers.is_empty());
        Ok(())
    }

    /// DDS-Security 1.2 §7.5.5.2, Table 9: the secure TypeLookup service has
    /// its own EntityIds (`ff0300c3`, `ff0300c4`, `ff0301c3`, `ff0301c4`) and
    /// the data types and QoS of the plain one. A secure transport takes the
    /// samples of the secure writers and not those of the plain ones, and the
    /// plain one the reverse.
    #[test]
    fn the_secure_service_has_its_own_endpoints() -> Result<(), Box<dyn std::error::Error>> {
        let local_prefix = GuidPrefix::from_bytes([1; 12]);
        let peer_prefix = GuidPrefix::from_bytes([2; 12]);
        let at = Locator::udp_v4([127, 0, 0, 1], 10000);
        let lease = Duration::from_secs(5);
        let mut remote = TypeLookupTransport::new_secure(peer_prefix, lease);
        let request = remote.write(true, local_prefix, at, &[0, 7, 0, 0], None, Duration::ZERO)?;
        let bytes = &request.first().ok_or("missing request DATA")?.bytes;
        // The writer id after the RTPS header (20), the INFO_DST-less DATA
        // header (4), extraFlags/octetsToInlineQos (4) and the readerId (4).
        let wire = EntityId::TL_SVC_REQ_SECURE_WRITER.to_bytes();
        assert_eq!(wire, [0xff, 0x03, 0x00, 0xc3]);
        assert!(bytes.windows(4).any(|w| w == wire));
        assert!(may_carry_type_lookup(bytes));
        let mut secure = TypeLookupTransport::new_secure(local_prefix, lease);
        let (accepted, samples) = secure.receive(bytes, at, Duration::ZERO)?;
        assert!(accepted);
        assert_eq!(samples.len(), 1);
        assert_eq!(
            samples[0].writer_guid.entity_id,
            EntityId::TL_SVC_REQ_SECURE_WRITER
        );
        let mut plain = TypeLookupTransport::new(local_prefix, lease);
        assert!(!plain.receive(bytes, at, Duration::ZERO)?.0);
        Ok(())
    }

    /// §7.6.3.3: the history of a TypeLookup endpoint is KEEP_ALL, so 40
    /// unacknowledged samples are all held and none is replaced by a newer
    /// one (a depth of 32 would have kept the newest 32).
    #[test]
    fn the_history_of_a_type_lookup_endpoint_keeps_all() -> Result<(), Box<dyn std::error::Error>> {
        let mut channel = Channel::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([9, 9, 9]),
            EntityId::user_reader_with_key([9, 9, 8]),
        );
        for n in 0..40u8 {
            channel.writer.write(&[0, 1, 0, 0, n, 0, 0, 0])?;
        }
        assert_eq!(channel.writer.cache().len(), 40);
        Ok(())
    }

    /// DDS-Security 1.2 §7.5.5.2: "The data types and Qos policies of the Secure
    /// Type Lookup builtin shall be the same defined for the corresponding
    /// (non-secure) endpoint in DDS-XTYPES": reliable writers and readers with
    /// KEEP_ALL history (XTypes 1.3 §7.6.3.3), as the plain service has them.
    #[test]
    fn the_secure_service_has_the_qos_of_the_plain_one() -> Result<(), Box<dyn std::error::Error>> {
        let prefix = GuidPrefix::from_bytes([1; 12]);
        let lease = Duration::from_secs(5);
        let mut plain = TypeLookupTransport::new(prefix, lease);
        let mut secure = TypeLookupTransport::new_secure(prefix, lease);
        for transport in [&mut plain, &mut secure] {
            for n in 0..40u8 {
                transport.request.writer.write(&[0, 7, 0, 0, n, 0, 0, 0])?;
            }
            assert_eq!(transport.request.writer.cache().len(), 40);
        }
        assert_eq!(
            plain.reply.reader.max_samples_per_proxy(),
            secure.reply.reader.max_samples_per_proxy()
        );
        Ok(())
    }

    /// §7.6.3.3.3: "the RELIABILITY policy shall be DDS_RELIABLE_RELIABILITY_QOS,
    /// the HISTORY policy to DDS_KEEP_ALL_HISTORY_QOS and the DURABILITY policy
    /// to DDS_VOLATILE_DURABILITY_QOS." Reliable: the peer's proxy is reliable
    /// and the writer sends HEARTBEATs. Volatile: a peer that matches after a
    /// sample was written is not sent that sample, neither with the write that
    /// matches it nor later.
    #[test]
    fn the_endpoints_are_reliable_keep_all_and_volatile() -> Result<(), Box<dyn std::error::Error>>
    {
        fn holds(datagram: &OutboundDatagram, needle: &[u8]) -> bool {
            datagram
                .bytes
                .windows(needle.len())
                .any(|window| window == needle)
        }
        let early = GuidPrefix::from_bytes([2; 12]);
        let late = GuidPrefix::from_bytes([3; 12]);
        let at_early = Locator::udp_v4([127, 0, 0, 1], 10000);
        let at_late = Locator::udp_v4([127, 0, 0, 1], 10001);
        let first = [0xA1u8; 8];
        let second = [0xB2u8; 8];

        let mut service = TypeLookupTransport::new(GuidPrefix::from_bytes([1; 12]), Duration::MAX);
        let sent = service.write(true, early, at_early, &first, None, Duration::ZERO)?;
        assert!(sent.iter().any(|d| holds(d, &first)));

        let sent = service.write(true, late, at_late, &second, None, Duration::ZERO)?;
        assert!(sent.iter().any(|d| holds(d, &second)));
        assert!(
            !sent.iter().any(|d| holds(d, &first)),
            "VOLATILE: the sample written before the match is not replayed"
        );

        let ticks = service.tick(Duration::from_secs(1))?;
        assert!(
            !ticks.is_empty(),
            "RELIABLE: the writer announces its samples with HEARTBEATs"
        );
        assert!(
            !ticks
                .iter()
                .any(|d| holds(d, &first) && d.targets.contains(&at_late))
        );
        assert!(
            service
                .request
                .writer
                .reader_proxies()
                .iter()
                .all(|proxy| proxy.is_reliable)
        );
        Ok(())
    }

    /// The header walk says yes to what `receive` accepts — a reply DATA,
    /// a request DATA and the HEARTBEAT a service writer sends — and no to
    /// a user writer's DATA; a message it cannot walk goes to the full
    /// decode.
    #[test]
    fn the_header_walk_finds_what_receive_accepts() -> Result<(), Box<dyn std::error::Error>> {
        use zerodds_rtps::history_cache::HistoryKind;
        use zerodds_rtps::reliable_writer::{ReliableWriter, ReliableWriterConfig};
        let local_prefix = GuidPrefix::from_bytes([1; 12]);
        let peer_prefix = GuidPrefix::from_bytes([2; 12]);
        let at = Locator::udp_v4([127, 0, 0, 1], 10000);

        let mut remote = TypeLookupTransport::new(peer_prefix, Duration::from_secs(5));
        let reply = remote.write(false, local_prefix, at, &[0, 7, 0, 0], None, Duration::ZERO)?;
        let request = remote.write(true, local_prefix, at, &[0, 7, 0, 0], None, Duration::ZERO)?;
        let heartbeats = remote.tick(Duration::from_secs(1))?;
        let mut local = TypeLookupTransport::new(local_prefix, Duration::from_secs(5));
        for datagram in reply.iter().chain(&request).chain(&heartbeats) {
            assert!(may_carry_type_lookup(&datagram.bytes));
            assert!(
                local
                    .receive(&datagram.bytes, at, Duration::from_secs(1))?
                    .0
            );
        }

        let mut user = ReliableWriter::new(ReliableWriterConfig {
            guid: Guid::new(peer_prefix, EntityId::user_writer_with_key([1, 2, 3])),
            vendor_id: VendorId::ZERODDS,
            reader_proxies: alloc::vec![ReaderProxy::new(
                Guid::new(local_prefix, EntityId::user_reader_with_key([4, 5, 6])),
                alloc::vec![at],
                alloc::vec![],
                false,
            )],
            max_samples: 8,
            history_kind: HistoryKind::KeepLast { depth: 1 },
            heartbeat_period: Duration::from_millis(100),
            fragment_size: zerodds_rtps::reliable_writer::DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        let data = user.write_keyed_stamped(&[0, 1, 0, 0, 9, 9, 9, 9], None, Some([7; 16]))?;
        for datagram in &data {
            assert!(!may_carry_type_lookup(&datagram.bytes));
            assert!(
                !local
                    .receive(&datagram.bytes, at, Duration::from_secs(1))?
                    .0
            );
        }

        let truncated = &reply[0].bytes[..reply[0].bytes.len() - 3];
        assert!(may_carry_type_lookup(truncated));
        assert!(may_carry_type_lookup(b"not an RTPS message"));
        Ok(())
    }
}
