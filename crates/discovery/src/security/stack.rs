// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `SecurityBuiltinStack` — bundelt die zwei Security-Builtin-Topic-
//! Endpoint-Paare in einer Struktur.
//!
//! - `DCPSParticipantStatelessMessage` (Auth-Handshake, BestEffort).
//! - `DCPSParticipantVolatileMessageSecure` (Crypto-KeyExchange, Reliable).
//!
//! Wird vom Participant-Wiring (DCPS-Layer) instanziiert, sobald ein
//! Security-Plugin registriert ist und die Discovery-Bits 22..25 im
//! `BuiltinEndpointSet` annonciert werden. Der Stack pflegt die Reader/
//! Writer-Proxies pro Remote-Participant — `handle_remote_endpoints`
//! wird vom SPDP-Hot-Path aufgerufen, sobald ein Peer mit den
//! entsprechenden Bits entdeckt wurde.

extern crate alloc;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::error::WireError;
use zerodds_rtps::message_builder::OutboundDatagram;
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, VendorId};
use zerodds_rtps::writer_proxy::WriterProxy;

use crate::capabilities::PeerCapabilities;
use crate::security::stateless::{StatelessMessageReader, StatelessMessageWriter};
use crate::security::volatile_secure::{VolatileSecureMessageReader, VolatileSecureMessageWriter};
use crate::spdp::DiscoveredParticipant;

/// Bundle aus den vier Security-Builtin-Endpoints.
#[derive(Debug)]
pub struct SecurityBuiltinStack {
    local_prefix: GuidPrefix,
    /// Stateless-Auth-Writer (Spec §7.4.4).
    pub stateless_writer: StatelessMessageWriter,
    /// Stateless-Auth-Reader.
    pub stateless_reader: StatelessMessageReader,
    /// Volatile-Secure-Writer (Spec §7.4.5).
    pub volatile_writer: VolatileSecureMessageWriter,
    /// Volatile-Secure-Reader.
    pub volatile_reader: VolatileSecureMessageReader,
}

impl SecurityBuiltinStack {
    /// Erzeugt einen frischen Stack ohne Remote-Proxies.
    #[must_use]
    pub fn new(local_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self {
            local_prefix,
            stateless_writer: StatelessMessageWriter::new(local_prefix, vendor_id),
            stateless_reader: StatelessMessageReader::new(local_prefix, vendor_id),
            volatile_writer: VolatileSecureMessageWriter::new(local_prefix, vendor_id),
            volatile_reader: VolatileSecureMessageReader::new(local_prefix, vendor_id),
        }
    }

    /// Lokaler GuidPrefix.
    #[must_use]
    pub fn local_prefix(&self) -> GuidPrefix {
        self.local_prefix
    }

    /// Verdrahtet Reader-/Writer-Proxies auf Basis der vom Peer
    /// annoncierten BuiltinEndpointSet-Bits (Spec §7.4.7.1):
    ///
    /// - Bits 22+23 (`PARTICIPANT_STATELESS_MESSAGE_*`) → Stateless-Slot
    /// - Bits 24+25 (`PARTICIPANT_VOLATILE_MESSAGE_SECURE_*`) → Volatile-Slot
    ///
    /// Wir routen ueber `metatraffic_unicast_locator` (PID 0x0032),
    /// fallback auf `default_unicast_locator`. Selbst-Discovery
    /// (`peer.sender_prefix == self.local_prefix`) wird ignoriert.
    pub fn handle_remote_endpoints(&mut self, peer: &DiscoveredParticipant) {
        if peer.sender_prefix == self.local_prefix {
            return;
        }
        let caps = PeerCapabilities::from_bits(peer.data.builtin_endpoint_set);
        if !caps.has_stateless_auth && !caps.has_volatile_secure {
            return;
        }
        let unicast: Vec<Locator> = peer
            .data
            .metatraffic_unicast_locator
            .or(peer.data.default_unicast_locator)
            .into_iter()
            .collect();
        let remote_prefix = peer.sender_prefix;

        if caps.has_stateless_auth {
            self.stateless_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
                ),
                unicast.clone(),
                Vec::new(),
                false,
            ));
            self.stateless_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
                ),
                unicast.clone(),
                Vec::new(),
                false,
            ));
        }

        if caps.has_volatile_secure {
            self.volatile_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
                ),
                unicast.clone(),
                Vec::new(),
                true,
            ));
            self.volatile_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
                ),
                unicast,
                Vec::new(),
                true,
            ));
        }
    }

    /// Cleanup nach SPDP-Lease-Timeout: alle Proxies dieses Prefixes
    /// entfernen. Liefert `(stateless_pairs_removed,
    /// volatile_pairs_removed)`.
    pub fn on_participant_lost(&mut self, prefix: GuidPrefix) -> (usize, usize) {
        let mut stateless = 0usize;
        let mut volatile = 0usize;
        if self
            .stateless_writer
            .remove_reader_proxy(Guid::new(
                prefix,
                EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
            ))
            .is_some()
        {
            stateless += 1;
        }
        self.stateless_reader.remove_writer_proxy(Guid::new(
            prefix,
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
        ));
        if self
            .volatile_writer
            .remove_reader_proxy(Guid::new(
                prefix,
                EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            ))
            .is_some()
        {
            volatile += 1;
        }
        self.volatile_reader.remove_writer_proxy(Guid::new(
            prefix,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
        ));
        (stateless, volatile)
    }

    /// Tick ueber alle Endpoints. Liefert HEARTBEATs/Resends vom
    /// Volatile-Writer plus ACKNACK/NACK_FRAG vom Volatile-Reader.
    /// Stateless hat keinen Tick (BestEffort, kein Resend-State).
    ///
    /// # Errors
    /// Wire-Encode-Fehler aus dem Reliable-Layer.
    pub fn poll(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = Vec::new();
        out.extend(self.volatile_writer.tick(now)?);
        out.extend(self.volatile_reader.tick_outbound(now)?);
        Ok(out)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_rtps::participant_data::{
        Duration as DdsDuration, ParticipantBuiltinTopicData, endpoint_flag,
    };
    use zerodds_rtps::wire_types::ProtocolVersion;
    use zerodds_security::generic_message::{MessageIdentity, ParticipantGenericMessage, class_id};
    use zerodds_security::token::DataHolder;

    fn local_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([1; 12])
    }
    fn remote_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([2; 12])
    }

    fn remote_with(flags: u32) -> DiscoveredParticipant {
        DiscoveredParticipant {
            sender_prefix: remote_prefix(),
            sender_vendor: VendorId::ZERODDS,
            data: ParticipantBuiltinTopicData {
                guid: Guid::new(remote_prefix(), EntityId::PARTICIPANT),
                protocol_version: ProtocolVersion::V2_5,
                vendor_id: VendorId::ZERODDS,
                default_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 99], 7411)),
                default_multicast_locator: None,
                metatraffic_unicast_locator: None,
                metatraffic_multicast_locator: None,
                domain_id: None,
                builtin_endpoint_set: flags,
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

    fn sample_stateless_msg() -> ParticipantGenericMessage {
        ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0xAA; 16],
                sequence_number: 1,
            },
            related_message_identity: MessageIdentity::default(),
            destination_participant_key: [0xBB; 16],
            destination_endpoint_key: [0; 16],
            source_endpoint_key: [0xCC; 16],
            message_class_id: class_id::AUTH_REQUEST.into(),
            message_data: alloc::vec![DataHolder::new("DDS:Auth:PKI-DH:1.2+AuthReq")],
        }
    }

    #[test]
    fn new_stack_has_zero_proxies_everywhere() {
        let s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        assert_eq!(s.stateless_writer.reader_proxy_count(), 0);
        assert_eq!(s.stateless_reader.writer_proxy_count(), 0);
        assert_eq!(s.volatile_writer.reader_proxy_count(), 0);
        assert_eq!(s.volatile_reader.writer_proxy_count(), 0);
        assert_eq!(s.local_prefix(), local_prefix());
    }

    #[test]
    fn handle_remote_endpoints_with_all_bits_wires_all_four() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER;
        s.handle_remote_endpoints(&remote_with(flags));
        assert_eq!(s.stateless_writer.reader_proxy_count(), 1);
        assert_eq!(s.stateless_reader.writer_proxy_count(), 1);
        assert_eq!(s.volatile_writer.reader_proxy_count(), 1);
        assert_eq!(s.volatile_reader.writer_proxy_count(), 1);
    }

    #[test]
    fn handle_remote_endpoints_with_only_stateless_bits_skips_volatile() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        s.handle_remote_endpoints(&remote_with(flags));
        assert_eq!(s.stateless_writer.reader_proxy_count(), 1);
        assert_eq!(s.stateless_reader.writer_proxy_count(), 1);
        assert_eq!(s.volatile_writer.reader_proxy_count(), 0);
        assert_eq!(s.volatile_reader.writer_proxy_count(), 0);
    }

    #[test]
    fn handle_remote_endpoints_with_no_security_bits_is_noop() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::ALL_STANDARD;
        s.handle_remote_endpoints(&remote_with(flags));
        assert_eq!(s.stateless_writer.reader_proxy_count(), 0);
        assert_eq!(s.volatile_writer.reader_proxy_count(), 0);
    }

    #[test]
    fn self_discovery_is_ignored() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let mut peer = remote_with(endpoint_flag::ALL_SECURE);
        peer.sender_prefix = local_prefix();
        s.handle_remote_endpoints(&peer);
        assert_eq!(s.stateless_writer.reader_proxy_count(), 0);
    }

    #[test]
    fn handle_remote_endpoints_is_idempotent_on_repeat_announcement() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        s.handle_remote_endpoints(&remote_with(flags));
        s.handle_remote_endpoints(&remote_with(flags));
        assert_eq!(s.stateless_writer.reader_proxy_count(), 1);
    }

    #[test]
    fn on_participant_lost_clears_proxies() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER;
        s.handle_remote_endpoints(&remote_with(flags));
        let (sl, vol) = s.on_participant_lost(remote_prefix());
        assert_eq!(sl, 1);
        assert_eq!(vol, 1);
        assert_eq!(s.stateless_writer.reader_proxy_count(), 0);
        assert_eq!(s.volatile_writer.reader_proxy_count(), 0);
    }

    #[test]
    fn poll_on_empty_stack_returns_no_datagrams() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let dgs = s.poll(Duration::from_secs(1)).unwrap();
        assert!(dgs.is_empty());
    }

    #[test]
    fn end_to_end_stateless_message_loopback_between_stacks() {
        let mut a = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let mut b = SecurityBuiltinStack::new(remote_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        // A entdeckt B
        a.handle_remote_endpoints(&remote_with_prefix(remote_prefix(), flags));
        // B entdeckt A
        b.handle_remote_endpoints(&remote_with_prefix(local_prefix(), flags));

        let msg = sample_stateless_msg();
        let dgs = a.stateless_writer.write(&msg).unwrap();
        assert_eq!(dgs.len(), 1);
        let received = b.stateless_reader.handle_datagram(&dgs[0].bytes).unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0], msg);
    }

    fn remote_with_prefix(prefix: GuidPrefix, flags: u32) -> DiscoveredParticipant {
        let mut peer = remote_with(flags);
        peer.sender_prefix = prefix;
        peer.data.guid = Guid::new(prefix, EntityId::PARTICIPANT);
        peer
    }

    #[test]
    fn end_to_end_volatile_secure_handshake_via_reliable_loop() {
        // A schickt eine Crypto-Token-Message ueber das VolatileSecure-
        // Topic an B. Wir simulieren den Reliable-Hop manuell:
        // 1. A.write produziert DATA + (initial pre-emptiver HEARTBEAT folgt im Tick)
        // 2. B.handle_data dekodiert die Message
        // 3. B.tick_outbound emittiert ein ACKNACK
        // 4. A.handle_acknack akzeptiert es
        let mut a = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let mut b = SecurityBuiltinStack::new(remote_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER;
        a.handle_remote_endpoints(&remote_with_prefix(remote_prefix(), flags));
        b.handle_remote_endpoints(&remote_with_prefix(local_prefix(), flags));

        let mut msg = sample_stateless_msg();
        msg.message_class_id = class_id::PARTICIPANT_CRYPTO_TOKENS.into();

        let dgs = a.volatile_writer.write(&msg).unwrap();
        assert_eq!(dgs.len(), 1, "ein Datagram pro Reader-Proxy");
        // Wire-Decode + dispatch in Bs Volatile-Reader
        let parsed = zerodds_rtps::datagram::decode_datagram(&dgs[0].bytes).unwrap();
        let mut received_msgs = Vec::new();
        for sub in parsed.submessages {
            if let zerodds_rtps::datagram::ParsedSubmessage::Data(d) = sub {
                if d.reader_id == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER {
                    received_msgs.extend(b.volatile_reader.handle_data(&d).unwrap());
                }
            }
        }
        assert_eq!(received_msgs.len(), 1);
        assert_eq!(received_msgs[0], msg);

        // ACKNACK fliesst zurueck, A muss handle_acknack akzeptieren
        let outbound = b
            .volatile_reader
            .tick_outbound(Duration::from_millis(500))
            .unwrap();
        // B kennt A als Writer-Proxy → ACKNACK-Datagrams sollten existieren
        assert!(
            !outbound.is_empty(),
            "Reader sollte initiales ACKNACK senden"
        );
    }
}
