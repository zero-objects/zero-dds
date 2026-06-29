// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Builtin endpoint `DCPSParticipantVolatileMessageSecure` — DDS-Security
//! 1.2 §7.4.5 + §10.5.4.
//!
//! Wire profile:
//! - Reliability: Reliable (Spec §7.5.4 Tab.20).
//! - Durability:  Volatile (KEEP_LAST 1 per spec, we conservatively use
//!   KEEP_LAST 16 for the re-send window).
//! - Topic type:  `ParticipantGenericMessage` (Spec §7.5.5).
//! - EntityIds:   `BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_{WRITER,READER}`.
//!
//! Wrapper around [`zerodds_rtps::reliable_writer::ReliableWriter`] +
//! [`zerodds_rtps::reliable_reader::ReliableReader`] with fixed EntityIds —
//! analogous to the SEDP endpoints, only with a different topic-type codec.

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

/// Default history depth (the spec says KEEP_LAST 1 — we conservatively
/// use 16, so that short crypto-token bursts during onboarding do not
/// drop individual tokens).
pub const VOLATILE_SECURE_DEFAULT_DEPTH: usize = 16;

/// Default HEARTBEAT period. Shorter than the SEDP default — we want
/// fast crypto-token delivery during auth onboarding.
pub const VOLATILE_SECURE_HEARTBEAT_PERIOD: Duration = Duration::from_millis(250);

/// Reader cache depth (analogous to the SEDP reader 256).
pub const VOLATILE_SECURE_READER_CAPACITY: usize = 64;

/// Writer for `DCPSParticipantVolatileMessageSecure`.
#[derive(Debug)]
pub struct VolatileSecureMessageWriter {
    inner: ReliableWriter,
}

impl VolatileSecureMessageWriter {
    /// Creates a writer for the local participant.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        let guid = Guid::new(
            participant_prefix,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
        );
        let mut inner = ReliableWriter::new(ReliableWriterConfig {
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
        });
        // cyclones filtered VolatileSecure-Reader matcht per voller GUID:
        // without INFO_DST, cyclone resolves dst=0:0:0:ff0202c4 -> no match
        // -> wn->last_seq haengt -> maxseq-0-base-N-Deadlock (Trace-belegt).
        inner.set_emit_info_dst(true);
        Self { inner }
    }

    /// GUID of the writer.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Number of registered reader proxies.
    #[must_use]
    pub fn reader_proxy_count(&self) -> usize {
        self.inner.reader_proxy_count()
    }

    /// Read-only access to the ReliableWriter (tests/diagnostics).
    #[must_use]
    pub fn inner(&self) -> &ReliableWriter {
        &self.inner
    }

    /// Adds a reader proxy.
    pub fn add_reader_proxy(&mut self, proxy: ReaderProxy) {
        self.inner.add_reader_proxy(proxy);
    }

    /// Removes a reader proxy.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        self.inner.remove_reader_proxy(guid)
    }

    /// Sends a `ParticipantGenericMessage`. Returns one datagram per
    /// reader proxy.
    ///
    /// # Errors
    /// `WireError` from the reliable writer (cache overflow on
    /// `KeepAll`, sequence overflow).
    pub fn write(
        &mut self,
        msg: &ParticipantGenericMessage,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let payload = encode_generic_message(msg);
        self.inner.write(&payload)
    }

    /// Write + piggyback HEARTBEAT in ONE operation (RTPS 2.5 §8.4.15.5).
    ///
    /// Security tokens (Kx, per-endpoint crypto) are reliable and
    /// time-critical: the remote must NACK a loss IMMEDIATELY, not only at
    /// the periodic HEARTBEAT (`VOLATILE_SECURE_HEARTBEAT_PERIOD` =
    /// 250ms). Cross-vendor, the token otherwise falls behind Cyclone's
    /// AUTHOK `delete_pending_match` and is never applied. The piggyback
    /// HEARTBEAT (final_flag=false) forces the prompt ACKNACK response —
    /// per-send, so robust for late joiners / rediscovery (no global
    /// state).
    ///
    /// # Errors
    /// Wire encode errors.
    pub fn write_with_heartbeat(
        &mut self,
        msg: &ParticipantGenericMessage,
        now: Duration,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let payload = encode_generic_message(msg);
        self.inner.write_with_heartbeat(&payload, now)
    }

    /// Tick (HEARTBEAT + resends).
    ///
    /// # Errors
    /// Wire encode errors.
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
}

/// Reader for `DCPSParticipantVolatileMessageSecure`.
#[derive(Debug)]
pub struct VolatileSecureMessageReader {
    inner: ReliableReader,
}

impl VolatileSecureMessageReader {
    /// Creates a reader for the local participant.
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

    /// GUID of the reader.
    #[must_use]
    pub fn guid(&self) -> Guid {
        self.inner.guid()
    }

    /// Number of registered writer proxies.
    #[must_use]
    pub fn writer_proxy_count(&self) -> usize {
        self.inner.writer_proxy_count()
    }

    /// Read-only access to the ReliableReader (tests/diagnostics).
    #[must_use]
    pub fn inner(&self) -> &ReliableReader {
        &self.inner
    }

    /// Adds a writer proxy.
    pub fn add_writer_proxy(&mut self, proxy: WriterProxy) {
        self.inner.add_writer_proxy(proxy);
    }

    /// Removes a writer proxy.
    pub fn remove_writer_proxy(&mut self, guid: Guid) -> Option<WriterProxy> {
        self.inner.remove_writer_proxy(guid)
    }

    /// Processes an incoming DATA submessage and returns decoded
    /// generic messages.
    ///
    /// # Errors
    /// `BadArgument` if the encapsulation/CDR decode fails.
    pub fn handle_data(
        &mut self,
        source_prefix: GuidPrefix,
        data: &DataSubmessage,
    ) -> SecurityResult<Vec<ParticipantGenericMessage>> {
        let samples = self.inner.handle_data(source_prefix, data, None);
        decode_samples(samples.into_iter().map(|s| s.payload))
    }

    /// Process DATA_FRAG.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_data_frag(
        &mut self,
        source_prefix: GuidPrefix,
        df: &DataFragSubmessage,
        now: Duration,
    ) -> SecurityResult<Vec<ParticipantGenericMessage>> {
        let samples = self.inner.handle_data_frag(source_prefix, df, now, None);
        decode_samples(samples.into_iter().map(|s| s.payload))
    }

    /// Process GAP.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_gap(
        &mut self,
        source_prefix: GuidPrefix,
        gap: &GapSubmessage,
    ) -> SecurityResult<Vec<ParticipantGenericMessage>> {
        let samples = self.inner.handle_gap(source_prefix, gap);
        decode_samples(samples.into_iter().map(|s| s.payload))
    }

    /// HEARTBEAT verarbeiten.
    pub fn handle_heartbeat(
        &mut self,
        source_prefix: GuidPrefix,
        hb: &HeartbeatSubmessage,
        now: Duration,
    ) {
        self.inner.handle_heartbeat(source_prefix, hb, now);
    }

    /// Tick (ACKNACK / NACK_FRAG outbound).
    ///
    /// # Errors
    /// Wire encode errors.
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
        // Propagate the original SecurityError directly — the detail string
        // in the codec is already descriptive enough, no topic-specific
        // wrapper needed (Spec §7.5.5 makes no per-topic distinction at the
        // codec level).
        out.push(decode_generic_message(p.as_ref())?);
    }
    Ok(out)
}

// Marker that both imports above are permanently used.
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
    fn write_with_heartbeat_piggybacks_heartbeat_submessage() {
        use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
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
        // §8.4.15.5: token delivery must be prompt. A plain `write()`
        // makes the reader wait until the periodic HEARTBEAT (250ms) before
        // it NACKs a loss — cross-vendor the endpoint crypto token thereby
        // falls behind Cyclone's AUTHOK match. A piggyback HEARTBEAT
        // directly with the DATA solves this per-send (robust for late joiners).
        let dgs = w
            .write_with_heartbeat(&sample_msg(), Duration::from_millis(0))
            .unwrap();
        assert_eq!(
            dgs.len(),
            1,
            "DATA + piggyback HEARTBEAT share one datagram"
        );
        let parsed = decode_datagram(&dgs[0].bytes).expect("decode");
        assert!(
            parsed
                .submessages
                .iter()
                .any(|s| matches!(s, ParsedSubmessage::Data(_))),
            "DATA submessage missing"
        );
        assert!(
            parsed
                .submessages
                .iter()
                .any(|s| matches!(s, ParsedSubmessage::Heartbeat(_))),
            "piggyback HEARTBEAT missing — Volatile token would otherwise wait 250ms for periodic HB"
        );
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
        let out = r.handle_data(remote_prefix(), &data).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], msg);
    }

    #[test]
    fn reader_drops_data_from_unknown_writer() {
        let mut r = VolatileSecureMessageReader::new(local_prefix(), VendorId::ZERODDS);
        // No writer proxy registered → handle_data must return empty
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
        let out = r.handle_data(remote_prefix(), &data).unwrap();
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
        let err = r.handle_data(remote_prefix(), &data).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
}
