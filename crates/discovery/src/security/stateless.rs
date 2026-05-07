// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Builtin-Endpoint `DCPSParticipantStatelessMessage` — DDS-Security 1.2
//! §7.4.4 + §10.3.4.
//!
//! Wire-Profil:
//! - Reliability: BestEffort (Spec §7.5.3 — "stateless").
//! - Durability:  Volatile.
//! - Topic-Type:  `ParticipantGenericMessage` (Spec §7.5.5), encoded mit
//!   4-Byte CDR-LE-Encapsulation-Header + XCDR1-Body via
//!   [`security_runtime::builtin_topics::encode_generic_message`].
//! - EntityIds:   `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_{WRITER,READER}`.
//!
//! Wir nutzen **kein** [`zerodds_rtps::ReliableWriter`], weil Stateless
//! laut Spec keinen Empfangs-Status, keine HEARTBEATs und keine
//! AckNack-Loop hat. Stattdessen: simple Multi-Reader-Fan-out-Liste; pro
//! `write()` wird ein DATA-Datagramm pro [`ReaderProxy`] erzeugt.
//!
//! C3.4-c-Scope: keine Plugin-Pipeline-Logik im Reader. Der Reader
//! dekodiert die `ParticipantGenericMessage` und reicht sie an den
//! Caller — der Auth-Plugin-Hook (Spec §10.3.4.1) wird vom DCPS-Layer
//! oben drauf gesetzt.

extern crate alloc;

use alloc::rc::Rc;
use alloc::vec::Vec;

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram, encode_data_datagram};
use zerodds_rtps::error::WireError;
use zerodds_rtps::header::RtpsHeader;
use zerodds_rtps::message_builder::OutboundDatagram;
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::submessages::DataSubmessage;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, SequenceNumber, VendorId};
use zerodds_rtps::writer_proxy::WriterProxy;

use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::generic_message::ParticipantGenericMessage;

use crate::security::codec::{decode_generic_message, encode_generic_message};

/// Stateless-Message-Writer (Spec §7.4.4 + §10.3.4).
///
/// Pflegt Multi-Reader-Fan-out fuer den
/// `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER`-Endpoint. Jedes
/// `write()` erzeugt pro registriertem [`ReaderProxy`] ein
/// [`OutboundDatagram`] — kein Cache, kein Resend, kein HEARTBEAT
/// (Stateless = kein Empfangs-Status).
#[derive(Debug)]
pub struct StatelessMessageWriter {
    guid: Guid,
    vendor_id: VendorId,
    next_sn: i64,
    reader_proxies: Vec<ReaderProxy>,
}

impl StatelessMessageWriter {
    /// Erzeugt einen Writer fuer den lokalen Participant.
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

    /// GUID des Writers.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Read-only-Slice der registrierten Reader-Proxies.
    #[must_use]
    pub fn reader_proxies(&self) -> &[ReaderProxy] {
        &self.reader_proxies
    }

    /// Anzahl registrierter Reader-Proxies.
    #[must_use]
    pub fn reader_proxy_count(&self) -> usize {
        self.reader_proxies.len()
    }

    /// Fuegt einen Reader-Proxy hinzu (idempotent: gleiche GUID
    /// ueberschreibt).
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

    /// Entfernt einen Reader-Proxy. Liefert ihn zurueck, falls
    /// vorhanden.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        let idx = self
            .reader_proxies
            .iter()
            .position(|p| p.remote_reader_guid == guid)?;
        Some(self.reader_proxies.remove(idx))
    }

    /// Sendet eine `ParticipantGenericMessage` an alle Reader-Proxies.
    ///
    /// Liefert ein Datagram pro Proxy (oder leer, wenn keine
    /// registriert sind).
    ///
    /// # Errors
    /// `WireError::ValueOutOfRange` bei Sequence-Number-Overflow oder
    /// `WireError::*` aus der DATA-Encoding-Pipeline.
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
            // Ziel = Unicast-Locators des Proxies (Multicast ignorieren
            // wir bei Stateless: Auth-Handshake ist immer Punkt-zu-Punkt).
            let targets = Rc::new(proxy.unicast_locators.clone());
            out.push(OutboundDatagram { bytes, targets });
        }
        Ok(out)
    }
}

/// Stateless-Message-Reader (Spec §7.4.4 + §10.3.4).
///
/// Decoded eingehende DATA-Submessages, die an den
/// `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER` adressiert sind.
/// Stateless: kein History-Cache, kein ACKNACK, kein Heartbeat-State.
/// Ein bekannter Writer-Proxy ist fuer Source-Authenticity-Checks
/// optional — der Reader liefert die Message immer, der Auth-Plugin-
/// Hook entscheidet nach dem `source_guid`-Feld.
#[derive(Debug)]
pub struct StatelessMessageReader {
    guid: Guid,
    #[allow(dead_code)]
    vendor_id: VendorId,
    /// Bekannte Writer-Proxies. Der Reader nutzt sie nicht fuers
    /// Filtering (Stateless akzeptiert von jedem Writer), sondern fuer
    /// Caller-Diagnose (`writer_proxy_count`).
    writer_proxies: Vec<WriterProxy>,
}

impl StatelessMessageReader {
    /// Erzeugt einen Reader fuer den lokalen Participant.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self {
            guid: Guid::new(
                participant_prefix,
                EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
            ),
            vendor_id,
            writer_proxies: Vec::new(),
        }
    }

    /// GUID des Readers.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.guid
    }

    /// Anzahl registrierter Writer-Proxies.
    #[must_use]
    pub fn writer_proxy_count(&self) -> usize {
        self.writer_proxies.len()
    }

    /// Read-only-Slice der registrierten Writer-Proxies.
    #[must_use]
    pub fn writer_proxies(&self) -> &[WriterProxy] {
        &self.writer_proxies
    }

    /// Fuegt einen Writer-Proxy hinzu (idempotent).
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

    /// Entfernt einen Writer-Proxy.
    pub fn remove_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        let idx = self
            .writer_proxies
            .iter()
            .position(|p| p.remote_writer_guid == guid)?;
        Some(self.writer_proxies.remove(idx))
    }

    /// Verarbeitet eine eingehende DATA-Submessage und decoded sie zu
    /// einer `ParticipantGenericMessage`.
    ///
    /// # Errors
    /// `BadArgument` wenn die Encapsulation/CDR-Decode scheitert.
    pub fn handle_data(
        &mut self,
        data: &DataSubmessage,
    ) -> SecurityResult<ParticipantGenericMessage> {
        decode_generic_message(&data.serialized_payload)
    }

    /// Verarbeitet ein vollstaendiges RTPS-Datagram. Liefert alle
    /// dekodierten Stateless-Messages aus diesem Datagramm.
    ///
    /// # Errors
    /// - `BadArgument` wenn das Datagram nicht parst (Wire-Decoder-
    ///   Fehler) oder eine relevante DATA-Submessage einen kaputten
    ///   Generic-Message-Body hat.
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
        // Decoded SN aus den Wire-Bytes
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
        // Idempotenz
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
        // Writer baut Datagram, Reader dekodiert es zurueck.
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
