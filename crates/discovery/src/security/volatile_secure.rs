// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Builtin-Endpoint `DCPSParticipantVolatileMessageSecure` — DDS-Security
//! 1.2 §7.4.5 + §10.5.4.
//!
//! Wire-Profil:
//! - Reliability: Reliable (Spec §7.5.4 Tab.20).
//! - Durability:  Volatile (KEEP_LAST 1 nach Spec, wir nehmen
//!   konservativ KEEP_LAST 16 fuer Re-Send-Fenster).
//! - Topic-Type:  `ParticipantGenericMessage` (Spec §7.5.5).
//! - EntityIds:   `BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_{WRITER,READER}`.
//!
//! Wrapper um [`zerodds_rtps::reliable_writer::ReliableWriter`] +
//! [`zerodds_rtps::reliable_reader::ReliableReader`] mit festen EntityIds —
//! analog zu den SEDP-Endpoints, nur mit anderem Topic-Type-Codec.

extern crate alloc;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::error::WireError;
use zerodds_rtps::fragment_assembler::AssemblerCaps;
use zerodds_rtps::history_cache::HistoryKind;
use zerodds_rtps::message_builder::{DEFAULT_MTU, OutboundDatagram};
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::reliable_reader::{
    DEFAULT_HEARTBEAT_RESPONSE_DELAY, ReliableReader, ReliableReaderConfig,
};
use zerodds_rtps::reliable_writer::{DEFAULT_FRAGMENT_SIZE, ReliableWriter, ReliableWriterConfig};
use zerodds_rtps::submessages::{
    DataFragSubmessage, DataSubmessage, GapSubmessage, HeartbeatSubmessage, NackFragSubmessage,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, SequenceNumber, VendorId};
use zerodds_rtps::writer_proxy::WriterProxy;

use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::generic_message::ParticipantGenericMessage;

use crate::security::codec::{decode_generic_message, encode_generic_message};

/// Default-History-Tiefe (Spec sagt KEEP_LAST 1 — wir nehmen
/// konservativ 16, damit kurze Crypto-Token-Bursts beim Onboarding nicht
/// einzelne Tokens droppen).
pub const VOLATILE_SECURE_DEFAULT_DEPTH: usize = 16;

/// Default-HEARTBEAT-Periode. Kuerzer als SEDP-Default — wir wollen
/// schnelle Crypto-Token-Lieferung beim Auth-Onboarding.
pub const VOLATILE_SECURE_HEARTBEAT_PERIOD: Duration = Duration::from_millis(250);

/// Reader-Cache-Tiefe (analog zu SEDP-Reader 256).
pub const VOLATILE_SECURE_READER_CAPACITY: usize = 64;

/// Writer fuer `DCPSParticipantVolatileMessageSecure`.
#[derive(Debug)]
pub struct VolatileSecureMessageWriter {
    inner: ReliableWriter,
}

impl VolatileSecureMessageWriter {
    /// Erzeugt einen Writer fuer den lokalen Participant.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        let guid = Guid::new(
            participant_prefix,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
        );
        Self {
            inner: ReliableWriter::new(ReliableWriterConfig {
                guid,
                vendor_id,
                reader_proxies: Vec::new(),
                max_samples: VOLATILE_SECURE_DEFAULT_DEPTH,
                history_kind: HistoryKind::KeepLast {
                    depth: VOLATILE_SECURE_DEFAULT_DEPTH,
                },
                heartbeat_period: VOLATILE_SECURE_HEARTBEAT_PERIOD,
                fragment_size: DEFAULT_FRAGMENT_SIZE,
                mtu: DEFAULT_MTU,
            }),
        }
    }

    /// GUID des Writers.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Anzahl registrierter Reader-Proxies.
    #[must_use]
    pub fn reader_proxy_count(&self) -> usize {
        self.inner.reader_proxy_count()
    }

    /// Read-only-Zugriff auf den ReliableWriter (Tests/Diagnose).
    #[must_use]
    pub fn inner(&self) -> &ReliableWriter {
        &self.inner
    }

    /// Fuegt einen Reader-Proxy hinzu.
    pub fn add_reader_proxy(&mut self, proxy: ReaderProxy) {
        self.inner.add_reader_proxy(proxy);
    }

    /// Entfernt einen Reader-Proxy.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        self.inner.remove_reader_proxy(guid)
    }

    /// Sendet eine `ParticipantGenericMessage`. Liefert pro Reader-
    /// Proxy ein Datagramm.
    ///
    /// # Errors
    /// `WireError` aus dem Reliable-Writer (Cache-Overflow bei
    /// `KeepAll`, Sequence-Overflow).
    pub fn write(
        &mut self,
        msg: &ParticipantGenericMessage,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let payload = encode_generic_message(msg);
        self.inner.write(&payload)
    }

    /// Tick (HEARTBEAT + Resends).
    ///
    /// # Errors
    /// Wire-Encode-Fehler.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        self.inner.tick(now)
    }

    /// Dispatch eines ACKNACK vom Remote-Reader.
    pub fn handle_acknack(
        &mut self,
        src_guid: Guid,
        base: SequenceNumber,
        requested: impl IntoIterator<Item = SequenceNumber>,
    ) {
        self.inner.handle_acknack(src_guid, base, requested);
    }

    /// Dispatch eines NACK_FRAG vom Remote-Reader.
    pub fn handle_nackfrag(&mut self, src_guid: Guid, nf: &NackFragSubmessage) {
        self.inner.handle_nackfrag(src_guid, nf);
    }
}

/// Reader fuer `DCPSParticipantVolatileMessageSecure`.
#[derive(Debug)]
pub struct VolatileSecureMessageReader {
    inner: ReliableReader,
}

impl VolatileSecureMessageReader {
    /// Erzeugt einen Reader fuer den lokalen Participant.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        let guid = Guid::new(
            participant_prefix,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
        );
        Self {
            inner: ReliableReader::new(ReliableReaderConfig {
                guid,
                vendor_id,
                writer_proxies: Vec::new(),
                max_samples_per_proxy: VOLATILE_SECURE_READER_CAPACITY,
                heartbeat_response_delay: DEFAULT_HEARTBEAT_RESPONSE_DELAY,
                assembler_caps: AssemblerCaps::default(),
            }),
        }
    }

    /// GUID des Readers.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Anzahl registrierter Writer-Proxies.
    #[must_use]
    pub fn writer_proxy_count(&self) -> usize {
        self.inner.writer_proxy_count()
    }

    /// Read-only-Zugriff auf den ReliableReader (Tests/Diagnose).
    #[must_use]
    pub fn inner(&self) -> &ReliableReader {
        &self.inner
    }

    /// Fuegt einen Writer-Proxy hinzu.
    pub fn add_writer_proxy(&mut self, proxy: WriterProxy) {
        self.inner.add_writer_proxy(proxy);
    }

    /// Entfernt einen Writer-Proxy.
    pub fn remove_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        self.inner.remove_writer_proxy(guid)
    }

    /// Verarbeitet eine eingehende DATA-Submessage und liefert
    /// dekodierte Generic-Messages.
    ///
    /// # Errors
    /// `BadArgument` wenn die Encapsulation/CDR-Decode scheitert.
    pub fn handle_data(
        &mut self,
        data: &DataSubmessage,
    ) -> SecurityResult<Vec<ParticipantGenericMessage>> {
        let samples = self.inner.handle_data(data);
        decode_samples(samples.into_iter().map(|s| s.payload))
    }

    /// DATA_FRAG verarbeiten.
    ///
    /// # Errors
    /// siehe [`handle_data`](Self::handle_data).
    pub fn handle_data_frag(
        &mut self,
        df: &DataFragSubmessage,
        now: Duration,
    ) -> SecurityResult<Vec<ParticipantGenericMessage>> {
        let samples = self.inner.handle_data_frag(df, now);
        decode_samples(samples.into_iter().map(|s| s.payload))
    }

    /// GAP verarbeiten.
    ///
    /// # Errors
    /// siehe [`handle_data`](Self::handle_data).
    pub fn handle_gap(
        &mut self,
        gap: &GapSubmessage,
    ) -> SecurityResult<Vec<ParticipantGenericMessage>> {
        let samples = self.inner.handle_gap(gap);
        decode_samples(samples.into_iter().map(|s| s.payload))
    }

    /// HEARTBEAT verarbeiten.
    pub fn handle_heartbeat(&mut self, hb: &HeartbeatSubmessage, now: Duration) {
        self.inner.handle_heartbeat(hb, now);
    }

    /// Tick (ACKNACK / NACK_FRAG-Outbound).
    ///
    /// # Errors
    /// Wire-Encode-Fehler.
    pub fn tick_outbound(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        self.inner.tick_outbound(now)
    }
}

fn decode_samples<B, I>(payloads: I) -> SecurityResult<Vec<ParticipantGenericMessage>>
where
    B: AsRef<[u8]>,
    I: IntoIterator<Item = B>,
{
    let mut out = Vec::new();
    for p in payloads {
        // Original-SecurityError direkt durchreichen — der Detail-String
        // im Codec ist bereits aussagekraeftig genug, kein Topic-spezi-
        // fischer Wrapper noetig (Spec §7.5.5 macht keine Unterscheidung
        // pro Topic auf der Codec-Ebene).
        out.push(decode_generic_message(p.as_ref())?);
    }
    Ok(out)
}

// Marker, dass beide Imports oben dauerhaft genutzt werden.
const _: Option<SecurityErrorKind> = None;
const _: Option<SecurityError> = None;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_rtps::wire_types::Locator;
    use zerodds_security::generic_message::{MessageIdentity, class_id};
    use zerodds_security::token::DataHolder;

    fn local_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([1; 12])
    }
    fn remote_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([2; 12])
    }

    fn sample_msg() -> ParticipantGenericMessage {
        ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0xAA; 16],
                sequence_number: 1,
            },
            related_message_identity: MessageIdentity::default(),
            destination_participant_key: [0xBB; 16],
            destination_endpoint_key: [0; 16],
            source_endpoint_key: [0xCC; 16],
            message_class_id: class_id::PARTICIPANT_CRYPTO_TOKENS.into(),
            message_data: alloc::vec![DataHolder::new("DDS:Crypto:AES-GCM-GMAC")],
        }
    }

    #[test]
    fn writer_has_expected_entity_id() {
        let w = VolatileSecureMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        assert_eq!(
            w.guid().entity_id,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
        );
    }

    #[test]
    fn reader_has_expected_entity_id() {
        let r = VolatileSecureMessageReader::new(local_prefix(), VendorId::ZERODDS);
        assert_eq!(
            r.guid().entity_id,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER
        );
    }

    #[test]
    fn writer_starts_with_zero_proxies() {
        let w = VolatileSecureMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        assert_eq!(w.reader_proxy_count(), 0);
    }

    #[test]
    fn reader_starts_with_zero_proxies() {
        let r = VolatileSecureMessageReader::new(local_prefix(), VendorId::ZERODDS);
        assert_eq!(r.writer_proxy_count(), 0);
    }

    #[test]
    fn write_without_proxies_returns_empty_datagrams() {
        let mut w = VolatileSecureMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let dgs = w.write(&sample_msg()).unwrap();
        assert!(dgs.is_empty());
    }

    #[test]
    fn write_with_one_proxy_produces_one_datagram() {
        let mut w = VolatileSecureMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        ));
        let dgs = w.write(&sample_msg()).unwrap();
        assert_eq!(dgs.len(), 1);
    }

    #[test]
    fn add_remove_reader_proxy_roundtrip() {
        let mut w = VolatileSecureMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(remote, alloc::vec![], alloc::vec![], true));
        assert_eq!(w.reader_proxy_count(), 1);
        assert!(w.remove_reader_proxy(remote).is_some());
        assert_eq!(w.reader_proxy_count(), 0);
    }

    #[test]
    fn add_remove_writer_proxy_roundtrip() {
        let mut r = VolatileSecureMessageReader::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
        );
        r.add_writer_proxy(WriterProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        ));
        assert_eq!(r.writer_proxy_count(), 1);
        assert!(r.remove_writer_proxy(remote).is_some());
        assert_eq!(r.writer_proxy_count(), 0);
    }

    #[test]
    fn reader_decodes_data_with_known_writer() {
        let mut r = VolatileSecureMessageReader::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
        );
        r.add_writer_proxy(WriterProxy::new(
            remote,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        ));
        let msg = sample_msg();
        let payload = encode_generic_message(&msg);
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            writer_id: EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let out = r.handle_data(&data).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], msg);
    }

    #[test]
    fn reader_drops_data_from_unknown_writer() {
        let mut r = VolatileSecureMessageReader::new(local_prefix(), VendorId::ZERODDS);
        // Kein Writer-Proxy registriert → handle_data muss leer liefern
        let msg = sample_msg();
        let payload = encode_generic_message(&msg);
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            writer_id: EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let out = r.handle_data(&data).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn reader_rejects_corrupt_payload() {
        let mut r = VolatileSecureMessageReader::new(local_prefix(), VendorId::ZERODDS);
        let remote = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
        );
        r.add_writer_proxy(WriterProxy::new(remote, alloc::vec![], alloc::vec![], true));
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            writer_id: EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::vec![0x00, 0x99, 0, 0].into(),
        };
        let err = r.handle_data(&data).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
}
