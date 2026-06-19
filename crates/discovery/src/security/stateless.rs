// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Builtin endpoint `DCPSParticipantStatelessMessage` — DDS-Security 1.2
//! §7.4.4 + §10.3.4.
//!
//! Wire profile:
//! - Reliability: BestEffort (Spec §7.5.3 — "stateless").
//! - Durability:  Volatile.
//! - Topic type:  `ParticipantGenericMessage` (Spec §7.5.5), encoded with
//!   a 4-byte CDR-LE encapsulation header + XCDR1 body via
//!   [`security_runtime::builtin_topics::encode_generic_message`].
//! - EntityIds:   `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_{WRITER,READER}`.
//!
//! We do **not** use [`zerodds_rtps::ReliableWriter`], because stateless
//! has no receive status, no HEARTBEATs and no AckNack loop per spec.
//! Instead: a simple multi-reader fan-out list; each `write()` produces
//! one DATA datagram per [`ReaderProxy`].
//!
//! C3.4-c scope: no plugin pipeline logic in the reader. The reader
//! decodes the `ParticipantGenericMessage` and passes it to the caller —
//! the auth plugin hook (Spec §10.3.4.1) is installed on top by the DCPS
//! layer.

extern crate alloc;

use alloc::rc::Rc;
use alloc::vec::Vec;

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram, encode_data_datagram};
use zerodds_rtps::error::WireError;
use zerodds_rtps::fragment_assembler::{AssemblerCaps, FragmentAssembler};
use zerodds_rtps::header::RtpsHeader;
use zerodds_rtps::message_builder::OutboundDatagram;
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::submessages::{DataFragSubmessage, DataSubmessage};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, SequenceNumber, VendorId};
use zerodds_rtps::writer_proxy::WriterProxy;

use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::generic_message::ParticipantGenericMessage;

use crate::security::codec::{decode_generic_message, encode_generic_message};

/// Stateless message writer (Spec §7.4.4 + §10.3.4).
///
/// Maintains multi-reader fan-out for the
/// `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER` endpoint. Each
/// `write()` produces one [`OutboundDatagram`] per registered
/// [`ReaderProxy`] — no cache, no resend, no HEARTBEAT
/// (stateless = no receive status).
#[derive(Debug)]
pub struct StatelessMessageWriter {
    guid: Guid,
    vendor_id: VendorId,
    next_sn: i64,
    reader_proxies: Vec<ReaderProxy>,
}

impl StatelessMessageWriter {
    /// Creates a writer for the local participant.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self {
            guid: Guid::new(
                participant_prefix,
                EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
            ),
            vendor_id,
            next_sn: 1,
            reader_proxies: Vec::new(),
        }
    }

    /// GUID of the writer.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Read-only slice of the registered reader proxies.
    #[must_use]
    pub fn reader_proxies(&self) -> &[ReaderProxy] {
        &self.reader_proxies
    }

    /// Number of registered reader proxies.
    #[must_use]
    pub fn reader_proxy_count(&self) -> usize {
        self.reader_proxies.len()
    }

    /// Adds a reader proxy (idempotent: same GUID overwrites).
    pub fn add_reader_proxy(&mut self, proxy: ReaderProxy) {
        let guid = proxy.remote_reader_guid;
        if let Some(idx) = self
            .reader_proxies
            .iter()
            .position(|p| p.remote_reader_guid == guid)
        {
            self.reader_proxies[idx] = proxy;
        } else {
            self.reader_proxies.push(proxy);
        }
    }

    /// Removes a reader proxy. Returns it if present.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        let idx = self
            .reader_proxies
            .iter()
            .position(|p| p.remote_reader_guid == guid)?;
        Some(self.reader_proxies.remove(idx))
    }

    /// Sends a `ParticipantGenericMessage` to all reader proxies.
    ///
    /// Returns one datagram per proxy (or empty if none are registered).
    ///
    /// # Errors
    /// `WireError::ValueOutOfRange` on sequence-number overflow or
    /// `WireError::*` from the DATA encoding pipeline.
    pub fn write(
        &mut self,
        msg: &ParticipantGenericMessage,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        if self.reader_proxies.is_empty() {
            return Ok(Vec::new());
        }
        let payload = encode_generic_message(msg);
        let sn = SequenceNumber(self.next_sn);
        self.next_sn = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "stateless writer sequence overflow",
            })?;

        let mut out = Vec::with_capacity(self.reader_proxies.len());
        for proxy in &self.reader_proxies {
            let data = DataSubmessage {
                extra_flags: 0,
                reader_id: proxy.remote_reader_guid.entity_id,
                writer_id: self.guid.entity_id,
                writer_sn: sn,
                inline_qos: None,
                key_flag: false,
                non_standard_flag: false,
                serialized_payload: payload.clone().into(),
            };
            let header = RtpsHeader::new(self.vendor_id, self.guid.prefix);
            let bytes = encode_data_datagram(header, &[data])?;
            // Target = unicast locators of the proxy (we ignore multicast
            // for stateless: the auth handshake is always point-to-point).
            let targets = Rc::new(proxy.unicast_locators.clone());
            out.push(OutboundDatagram { bytes, targets });
        }
        Ok(out)
    }
}

/// Stateless message reader (Spec §7.4.4 + §10.3.4).
///
/// Decodes incoming DATA submessages addressed to the
/// `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER`.
/// Stateless: no history cache, no ACKNACK, no heartbeat state.
/// A known writer proxy is optional for source-authenticity checks —
/// the reader always delivers the message, the auth plugin hook decides
/// based on the `source_guid` field.
#[derive(Debug)]
pub struct StatelessMessageReader {
    guid: Guid,
    #[allow(dead_code)]
    vendor_id: VendorId,
    /// Known writer proxies. The reader does not use them for filtering
    /// (stateless accepts from any writer), but for caller diagnostics
    /// (`writer_proxy_count`).
    writer_proxies: Vec<WriterProxy>,
    /// Fragment reassembly for LARGE stateless messages: cyclone/FastDDS
    /// RTPS-fragment the HandshakeReply/Final (DATA_FRAG), because with
    /// c.id-PEM + c.perm-p7s + c.pdata it easily exceeds the MTU.
    frag: FragmentAssembler,
}

impl StatelessMessageReader {
    /// Creates a reader for the local participant.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self {
            guid: Guid::new(
                participant_prefix,
                EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
            ),
            vendor_id,
            writer_proxies: Vec::new(),
            frag: FragmentAssembler::new(AssemblerCaps::default()),
        }
    }

    /// GUID of the reader.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Number of registered writer proxies.
    #[must_use]
    pub fn writer_proxy_count(&self) -> usize {
        self.writer_proxies.len()
    }

    /// Read-only slice of the registered writer proxies.
    #[must_use]
    pub fn writer_proxies(&self) -> &[WriterProxy] {
        &self.writer_proxies
    }

    /// Adds a writer proxy (idempotent).
    pub fn add_writer_proxy(&mut self, proxy: WriterProxy) {
        let guid = proxy.remote_writer_guid;
        if let Some(idx) = self
            .writer_proxies
            .iter()
            .position(|p| p.remote_writer_guid == guid)
        {
            self.writer_proxies[idx] = proxy;
        } else {
            self.writer_proxies.push(proxy);
        }
    }

    /// Removes a writer proxy.
    pub fn remove_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        let idx = self
            .writer_proxies
            .iter()
            .position(|p| p.remote_writer_guid == guid)?;
        Some(self.writer_proxies.remove(idx))
    }

    /// Processes an incoming DATA submessage and decodes it into a
    /// `ParticipantGenericMessage`.
    ///
    /// # Errors
    /// `BadArgument` if the encapsulation/CDR decode fails.
    pub fn handle_data(
        &mut self,
        data: &DataSubmessage,
    ) -> SecurityResult<ParticipantGenericMessage> {
        decode_generic_message(&data.serialized_payload)
    }

    /// Processes an incoming DATA_FRAG submessage. Large stateless
    /// messages (HandshakeReply/Final with cert/permissions) are
    /// RTPS-fragmented cross-vendor. Returns the decoded
    /// `ParticipantGenericMessage` once all fragments are present
    /// (otherwise empty — best-effort, no NACK).
    ///
    /// # Errors
    /// `BadArgument` if the reassembled generic-message body does not parse.
    pub fn handle_data_frag(
        &mut self,
        df: &DataFragSubmessage,
    ) -> SecurityResult<Vec<ParticipantGenericMessage>> {
        match self.frag.insert(df) {
            Some(completed) => Ok(alloc::vec![decode_generic_message(&completed.payload)?]),
            None => Ok(Vec::new()),
        }
    }

    /// Processes a complete RTPS datagram. Returns all decoded stateless
    /// messages from this datagram.
    ///
    /// # Errors
    /// - `BadArgument` if the datagram does not parse (wire decoder
    ///   error) or a relevant DATA submessage has a corrupt
    ///   generic-message body.
    pub fn handle_datagram(
        &mut self,
        datagram: &[u8],
    ) -> SecurityResult<Vec<ParticipantGenericMessage>> {
        let parsed = decode_datagram(datagram).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "stateless reader: wire decode failed",
            )
        })?;
        let mut out = Vec::new();
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                if d.reader_id == self.guid.entity_id
                    || d.writer_id == EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER
                {
                    out.push(decode_generic_message(&d.serialized_payload)?);
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;
    use zerodds_rtps::wire_types::Locator;
    use zerodds_security::generic_message::{MessageIdentity, class_id};
    use zerodds_security::token::DataHolder;

    fn sample_msg(seq: i64) -> ParticipantGenericMessage {
        ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0xAA; 16],
                sequence_number: seq,
            },
            related_message_identity: MessageIdentity::default(),
            destination_participant_key: [0xBB; 16],
            destination_endpoint_key: [0; 16],
            source_endpoint_key: [0xCC; 16],
            message_class_id: class_id::AUTH_REQUEST.into(),
            message_data: alloc::vec![DataHolder::new("DDS:Auth:PKI-DH:1.2+AuthReq")],
        }
    }

    fn local_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([1; 12])
    }

    fn remote_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([2; 12])
    }

    #[test]
    fn writer_has_expected_entity_id() {
        let w = StatelessMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        assert_eq!(
            w.guid().entity_id,
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER
        );
        assert_eq!(w.guid().prefix, local_prefix());
    }

    #[test]
    fn reader_has_expected_entity_id() {
        let r = StatelessMessageReader::new(local_prefix(), VendorId::ZERODDS);
        assert_eq!(
            r.guid().entity_id,
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER
        );
    }

    #[test]
    fn write_without_proxies_returns_empty() {
        let mut w = StatelessMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let dgs = w.write(&sample_msg(1)).unwrap();
        assert!(dgs.is_empty(), "no proxies → no fan-out");
    }

    #[test]
    fn write_to_one_proxy_produces_one_datagram() {
        let mut w = StatelessMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            false,
        ));
        let dgs = w.write(&sample_msg(1)).unwrap();
        assert_eq!(dgs.len(), 1);
        assert_eq!(dgs[0].targets.len(), 1);
    }

    #[test]
    fn write_to_two_proxies_produces_two_datagrams() {
        let mut w = StatelessMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let remote_a = Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
        );
        let remote_b = Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(
            remote_a,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            false,
        ));
        w.add_reader_proxy(ReaderProxy::new(
            remote_b,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7412)],
            alloc::vec![],
            false,
        ));
        assert_eq!(w.reader_proxy_count(), 2);
        let dgs = w.write(&sample_msg(1)).unwrap();
        assert_eq!(dgs.len(), 2);
    }

    #[test]
    fn add_reader_proxy_is_idempotent() {
        let mut w = StatelessMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            false,
        ));
        w.add_reader_proxy(ReaderProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            false,
        ));
        assert_eq!(w.reader_proxy_count(), 1);
    }

    #[test]
    fn remove_reader_proxy_returns_proxy() {
        let mut w = StatelessMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(
            remote,
            alloc::vec![],
            alloc::vec![],
            false,
        ));
        let removed = w.remove_reader_proxy(remote);
        assert!(removed.is_some());
        assert_eq!(w.reader_proxy_count(), 0);
        assert!(w.remove_reader_proxy(remote).is_none());
    }

    #[test]
    fn write_increments_sequence_number() {
        let mut w = StatelessMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            false,
        ));
        let dg1 = w.write(&sample_msg(1)).unwrap()[0].clone();
        let dg2 = w.write(&sample_msg(2)).unwrap()[0].clone();
        // Decode the SN from the wire bytes
        let p1 = decode_datagram(&dg1.bytes).unwrap();
        let p2 = decode_datagram(&dg2.bytes).unwrap();
        let sn1 = match &p1.submessages[0] {
            ParsedSubmessage::Data(d) => d.writer_sn,
            _ => unreachable!(),
        };
        let sn2 = match &p2.submessages[0] {
            ParsedSubmessage::Data(d) => d.writer_sn,
            _ => unreachable!(),
        };
        assert_eq!(sn1, SequenceNumber(1));
        assert_eq!(sn2, SequenceNumber(2));
    }

    #[test]
    fn write_carries_writer_entity_id_on_wire() {
        let mut w = StatelessMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            false,
        ));
        let dgs = w.write(&sample_msg(1)).unwrap();
        let parsed = decode_datagram(&dgs[0].bytes).unwrap();
        match &parsed.submessages[0] {
            ParsedSubmessage::Data(d) => {
                assert_eq!(
                    d.writer_id,
                    EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER
                );
                assert_eq!(
                    d.reader_id,
                    EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER
                );
            }
            _ => panic!("expected DATA"),
        }
    }

    #[test]
    fn reader_handle_data_decodes_generic_message() {
        let mut r = StatelessMessageReader::new(local_prefix(), VendorId::ZERODDS);
        let msg = sample_msg(42);
        let payload = encode_generic_message(&msg);
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
            writer_id: EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let decoded = r.handle_data(&data).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn reader_handle_data_rejects_corrupt_payload() {
        let mut r = StatelessMessageReader::new(local_prefix(), VendorId::ZERODDS);
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
            writer_id: EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::vec![0x00, 0x99, 0, 0].into(),
        };
        let err = r.handle_data(&data).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn reader_writer_proxy_management() {
        let mut r = StatelessMessageReader::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
        );
        r.add_writer_proxy(WriterProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            false,
        ));
        // Idempotency
        r.add_writer_proxy(WriterProxy::new(
            remote,
            alloc::vec![],
            alloc::vec![],
            false,
        ));
        assert_eq!(r.writer_proxy_count(), 1);
        assert!(r.remove_writer_proxy(remote).is_some());
        assert_eq!(r.writer_proxy_count(), 0);
    }

    #[test]
    fn end_to_end_writer_to_reader_loopback() {
        // Writer builds the datagram, reader decodes it back.
        let mut w = StatelessMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let mut r = StatelessMessageReader::new(remote_prefix(), VendorId::ZERODDS);
        let remote_reader_guid = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(
            remote_reader_guid,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            false,
        ));
        let msg = sample_msg(7);
        let dgs = w.write(&msg).unwrap();
        let decoded = r.handle_datagram(&dgs[0].bytes).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0], msg);
    }

    #[test]
    fn reader_handle_datagram_rejects_invalid_magic() {
        let mut r = StatelessMessageReader::new(local_prefix(), VendorId::ZERODDS);
        let err = r.handle_datagram(&[0u8; 24]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
}
