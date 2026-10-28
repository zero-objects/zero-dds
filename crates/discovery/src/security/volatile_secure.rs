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

/// Limit of the writer's history. The history is KEEP_ALL (DDS-Security
/// 1.2 Table 19): a write fails when this many messages are unacknowledged
/// instead of dropping the oldest.
pub const VOLATILE_SECURE_DEFAULT_DEPTH: usize = 1024;

/// Messages the writer holds back while its KEEP_ALL history is full. A
/// write that finds the history full blocks in the sense of DCPS 2.2.2.4.2.11
/// (`max_blocking_time`): it waits here, in order, for the readers to
/// acknowledge, and goes out from [`VolatileSecureMessageWriter::tick`]. With
/// this many waiting the write fails with an error (OUT_OF_RESOURCES).
pub const VOLATILE_SECURE_PENDING_LIMIT: usize = 4096;

/// Default HEARTBEAT period. Shorter than the SEDP default — we want
/// fast crypto-token delivery during auth onboarding.
pub const VOLATILE_SECURE_HEARTBEAT_PERIOD: Duration = Duration::from_millis(250);

/// Reader cache depth (analogous to the SEDP reader 256).
pub const VOLATILE_SECURE_READER_CAPACITY: usize = 64;

/// Writer for `DCPSParticipantVolatileMessageSecure`.
#[derive(Debug)]
pub struct VolatileSecureMessageWriter {
    inner: ReliableWriter,
    /// Sequence number of the next `message_identity` (§7.5.4.3).
    next_message_sn: i64,
    /// Messages waiting for room in the KEEP_ALL history, with whether
    /// they go out with a piggyback HEARTBEAT.
    pending: alloc::collections::VecDeque<(ParticipantGenericMessage, bool)>,
    /// How many messages were held back because the history was full.
    deferred: u64,
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
            history_kind: HistoryKind::KeepAll,
            heartbeat_period: VOLATILE_SECURE_HEARTBEAT_PERIOD,
            fragment_size: DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        // cyclones filtered VolatileSecure-Reader matcht per voller GUID:
        // without INFO_DST, cyclone resolves dst=0:0:0:ff0202c4 -> no match
        // -> wn->last_seq haengt -> maxseq-0-base-N-Deadlock (Trace-belegt).
        inner.set_emit_info_dst(true);
        Self {
            inner,
            next_message_sn: 1,
            pending: alloc::collections::VecDeque::new(),
            deferred: 0,
        }
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

    /// Adds a reader proxy. DURABILITY is VOLATILE (Table 19): a reader
    /// matched now gets what is written from here on, not what was written
    /// before.
    pub fn add_reader_proxy(&mut self, mut proxy: ReaderProxy) {
        let floor = self.inner.volatile_join_floor();
        let known = self
            .inner
            .reader_proxies()
            .iter()
            .any(|p| p.remote_reader_guid == proxy.remote_reader_guid);
        if floor.0 > 0 && !known {
            proxy.skip_samples_up_to(floor);
        }
        self.inner.add_reader_proxy(proxy);
    }

    /// Removes a reader proxy.
    pub fn remove_reader_proxy(&mut self, guid: Guid) -> Option<ReaderProxy> {
        self.inner.remove_reader_proxy(guid)
    }

    /// `message` with the `message_identity` of this writer's next message:
    /// `source_guid` is the GUID of the
    /// BuiltinParticipantVolatileMessageSecureWriter, `sequence_number`
    /// starts at one and is incremented for each message (§7.5.4.3). The
    /// number is taken once the message is in the history.
    fn stamped(&self, msg: &ParticipantGenericMessage) -> ParticipantGenericMessage {
        let mut stamped = msg.clone();
        stamped.message_identity = zerodds_security::generic_message::MessageIdentity {
            source_guid: self.inner.guid().to_bytes(),
            sequence_number: self.next_message_sn,
        };
        stamped
    }

    /// Whether the KEEP_ALL history has no room for another message.
    fn history_full(&self) -> bool {
        self.inner.cache().len() >= VOLATILE_SECURE_DEFAULT_DEPTH
    }

    /// Writes `msg` into the history now (the history has room).
    fn write_now(
        &mut self,
        msg: &ParticipantGenericMessage,
        heartbeat_at: Option<Duration>,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        let msg = self.stamped(msg);
        let payload = encode_generic_message(&msg);
        let out = match heartbeat_at {
            Some(now) => {
                self.inner
                    .write_with_heartbeat_to(&payload, now, None, destination_of(&msg))?
            }
            None => self.inner.write_to(&payload, destination_of(&msg))?,
        };
        self.next_message_sn = self.next_message_sn.saturating_add(1);
        Ok(out)
    }

    /// Holds `msg` back until the history has room.
    fn defer(
        &mut self,
        msg: &ParticipantGenericMessage,
        heartbeat: bool,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        if self.pending.len() >= VOLATILE_SECURE_PENDING_LIMIT {
            return Err(WireError::ValueOutOfRange {
                message: "volatile secure writer: KEEP_ALL history and waiting queue are full",
            });
        }
        self.pending.push_back((msg.clone(), heartbeat));
        self.deferred = self.deferred.saturating_add(1);
        Ok(Vec::new())
    }

    /// How many messages wait for room in the history.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// How many messages were held back so far because the KEEP_ALL history
    /// was full.
    #[must_use]
    pub fn deferred_count(&self) -> u64 {
        self.deferred
    }

    /// Sends a `ParticipantGenericMessage`. Returns one datagram per
    /// reader proxy. KEEP_ALL (Table 19): when the history is full the
    /// message is held back, in order, until the readers have
    /// acknowledged enough (see [`VOLATILE_SECURE_PENDING_LIMIT`]); it is
    /// never dropped.
    ///
    /// # Errors
    /// `WireError` from the reliable writer, or when the waiting queue is
    /// full as well.
    pub fn write(
        &mut self,
        msg: &ParticipantGenericMessage,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.inner.remove_acknowledged();
        if !self.pending.is_empty() || self.history_full() {
            return self.defer(msg, false);
        }
        self.write_now(msg, None)
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
    /// state). A full history holds the message back as for [`Self::write`].
    ///
    /// # Errors
    /// As [`Self::write`].
    pub fn write_with_heartbeat(
        &mut self,
        msg: &ParticipantGenericMessage,
        now: Duration,
    ) -> Result<Vec<OutboundDatagram>, WireError> {
        self.inner.remove_acknowledged();
        if !self.pending.is_empty() || self.history_full() {
            return self.defer(msg, true);
        }
        self.write_now(msg, Some(now))
    }

    /// Tick (HEARTBEAT + resends).
    ///
    /// # Errors
    /// Wire encode errors.
    pub fn tick(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        // VOLATILE: what every matched reader has acknowledged is not kept;
        // the room that makes lets the held-back messages go, in order.
        self.inner.remove_acknowledged();
        let mut out = Vec::new();
        while !self.history_full() {
            let Some((msg, heartbeat)) = self.pending.pop_front() else {
                break;
            };
            out.extend(self.write_now(&msg, heartbeat.then_some(now))?);
        }
        out.extend(self.inner.tick(now)?);
        Ok(out)
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

/// The participant a message is meant for: its `destination_participant_guid`
/// unless that is `GUID_UNKNOWN` (DDS-Security 1.2 §7.5.4.4). The writer gives
/// every other matched reader a GAP for it, the content filter §7.5.4.4
/// implies applied on the writer's side.
fn destination_of(msg: &ParticipantGenericMessage) -> Option<GuidPrefix> {
    if msg.destination_participant_key == [0; 16] {
        return None;
    }
    let mut prefix = [0u8; 12];
    prefix.copy_from_slice(&msg.destination_participant_key[..12]);
    Some(GuidPrefix::from_bytes(prefix))
}

/// Whether the reader of the participant `local` takes `msg`: the content
/// filter DDS-Security 1.2 §7.5.4.4 implies on the
/// BuiltinParticipantVolatileMessageSecureReader, `destination_participant_guid
/// == GUID_UNKNOWN || destination_participant_guid == reader.participant.guid`,
/// and a `destination_endpoint_guid` other than `GUID_UNKNOWN` naming an
/// endpoint of that participant.
#[must_use]
pub fn is_addressed_to(msg: &ParticipantGenericMessage, local: GuidPrefix) -> bool {
    let participant = Guid::new(local, EntityId::PARTICIPANT).to_bytes();
    let to_participant = msg.destination_participant_key == [0; 16]
        || msg.destination_participant_key == participant;
    let to_endpoint = msg.destination_endpoint_key == [0; 16]
        || msg.destination_endpoint_key[..12] == local.to_bytes();
    to_participant && to_endpoint
}

/// Reader for `DCPSParticipantVolatileMessageSecure`.
#[derive(Debug)]
pub struct VolatileSecureMessageReader {
    inner: ReliableReader,
    /// Messages the reader received and did not take, being addressed to
    /// another participant or to an endpoint of one (see [`is_addressed_to`]).
    not_addressed: u64,
}

impl VolatileSecureMessageReader {
    /// Creates a reader for the local participant.
    #[must_use]
    pub fn new(participant_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        let guid = Guid::new(
            participant_prefix,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
        );
        let inner = ReliableReader::new(ReliableReaderConfig {
            guid,
            vendor_id,
            writer_proxies: Vec::new(),
            max_samples_per_proxy: VOLATILE_SECURE_READER_CAPACITY,
            heartbeat_response_delay: DEFAULT_HEARTBEAT_RESPONSE_DELAY,
            assembler_caps: AssemblerCaps::default(),
        });
        Self {
            inner,
            not_addressed: 0,
        }
    }

    /// How many received messages the reader did not take because they
    /// were addressed elsewhere (DDS-Security 1.2 §7.5.4.4).
    #[must_use]
    pub fn not_addressed_count(&self) -> u64 {
        self.not_addressed
    }

    /// The decoded messages of `payloads` that are addressed to this
    /// reader's participant; the others are counted.
    fn take_addressed<B, I>(
        &mut self,
        payloads: I,
    ) -> SecurityResult<Vec<ParticipantGenericMessage>>
    where
        B: AsRef<[u8]>,
        I: IntoIterator<Item = B>,
    {
        let local = self.inner.guid().prefix;
        let mut msgs = decode_samples(payloads)?;
        let before = msgs.len();
        msgs.retain(|m| is_addressed_to(m, local));
        self.not_addressed = self
            .not_addressed
            .saturating_add((before - msgs.len()) as u64);
        Ok(msgs)
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

    /// Processes an incoming DATA submessage and returns the decoded
    /// generic messages addressed to this participant (DDS-Security 1.2
    /// §7.5.4.4, see [`is_addressed_to`]).
    ///
    /// # Errors
    /// `BadArgument` if the encapsulation/CDR decode fails.
    pub fn handle_data(
        &mut self,
        source_prefix: GuidPrefix,
        data: &DataSubmessage,
    ) -> SecurityResult<Vec<ParticipantGenericMessage>> {
        let samples = self.inner.handle_data(source_prefix, data, None);
        self.take_addressed(samples.into_iter().map(|s| s.payload))
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
        self.take_addressed(samples.into_iter().map(|s| s.payload))
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
        self.take_addressed(samples.into_iter().map(|s| s.payload))
    }

    /// Process a HEARTBEAT. A HEARTBEAT whose `first_sn` passes a hole
    /// releases the messages waiting behind it; they are returned like the
    /// ones a DATA or GAP releases.
    ///
    /// # Errors
    /// see [`handle_data`](Self::handle_data).
    pub fn handle_heartbeat(
        &mut self,
        source_prefix: GuidPrefix,
        hb: &HeartbeatSubmessage,
        now: Duration,
    ) -> SecurityResult<Vec<ParticipantGenericMessage>> {
        let samples = self.inner.handle_heartbeat(source_prefix, hb, now);
        self.take_addressed(samples.into_iter().map(|s| s.payload))
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
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
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
            destination_participant_key: [0; 16],
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
                .any(|s| matches!(s.submessage, ParsedSubmessage::Data(_))),
            "DATA submessage missing"
        );
        assert!(
            parsed
                .submessages
                .iter()
                .any(|s| matches!(s.submessage, ParsedSubmessage::Heartbeat(_))),
            "piggyback HEARTBEAT missing — Volatile token would otherwise wait 250ms for periodic HB"
        );
    }

    /// DDS-Security 1.2 §7.5.4.3: `source_guid` is the GUID of the
    /// BuiltinParticipantVolatileMessageSecureWriter and `sequence_number`
    /// starts at one and rises by one for each message, whatever the caller
    /// put there. Table 19: HISTORY is KEEP_ALL and DURABILITY VOLATILE, so
    /// a reader matched after two messages gets the third and neither of the
    /// first two.
    #[test]
    fn identity_is_the_writers_and_a_late_reader_gets_no_old_message() {
        use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
        let mut w = VolatileSecureMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let proxy = |prefix: GuidPrefix| {
            ReaderProxy::new(
                Guid::new(
                    prefix,
                    EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
                ),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
                alloc::vec![],
                true,
            )
        };
        w.add_reader_proxy(proxy(remote_prefix()));
        let identities = |dgs: &[OutboundDatagram]| {
            let mut out = Vec::new();
            for dg in dgs {
                for frame in decode_datagram(&dg.bytes).unwrap().submessages {
                    if let ParsedSubmessage::Data(d) = frame.submessage {
                        let m = decode_generic_message(d.serialized_payload.as_ref()).unwrap();
                        out.push((d.writer_sn.0, m.message_identity));
                    }
                }
            }
            out
        };
        let first = identities(&w.write(&sample_msg()).unwrap());
        let second = identities(&w.write(&sample_msg()).unwrap());
        assert_eq!(first[0].1.source_guid, w.guid().to_bytes());
        assert_eq!(first[0].1.sequence_number, 1);
        assert_eq!(second[0].1.sequence_number, 2);
        w.add_reader_proxy(proxy(GuidPrefix::from_bytes([3; 12])));
        let third = w.write(&sample_msg()).unwrap();
        let late = Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
        );
        let mut seen_by_late = Vec::new();
        for dg in &third {
            let parsed = decode_datagram(&dg.bytes).unwrap();
            for frame in parsed.submessages {
                if let ParsedSubmessage::Data(d) = frame.submessage {
                    if d.reader_id == late.entity_id {
                        seen_by_late.push(d.writer_sn.0);
                    }
                }
            }
        }
        assert!(!seen_by_late.contains(&1) && !seen_by_late.contains(&2));
    }

    /// DDS-Security 1.2 Table 19 (KEEP_ALL), DCPS 2.2.2.4.2.11: a write into
    /// a full history is not dropped. It waits, in order, behind the ones
    /// before it, goes out from `tick` once the reader acknowledged room, with
    /// the next message number; only a full waiting queue is an error.
    #[test]
    fn a_full_keep_all_history_holds_messages_back_and_drops_none() {
        use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
        let mut w = VolatileSecureMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let reader = Guid::new(
            remote_prefix(),
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
        );
        w.add_reader_proxy(ReaderProxy::new(
            reader,
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        ));
        for _ in 0..VOLATILE_SECURE_DEFAULT_DEPTH {
            assert_eq!(w.write(&sample_msg()).unwrap().len(), 1);
        }
        assert_eq!(w.pending_count(), 0);
        // The next ones wait; none is written and none is refused.
        for n in 1..=3usize {
            assert!(w.write(&sample_msg()).unwrap().is_empty());
            assert_eq!(w.pending_count(), n);
        }
        assert_eq!(w.deferred_count(), 3);
        // Nothing acknowledged: a tick has nothing to release.
        let idle = w.tick(Duration::from_secs(1)).unwrap();
        assert_eq!(w.pending_count(), 3);
        for dg in &idle {
            for f in decode_datagram(&dg.bytes).unwrap().submessages {
                if let ParsedSubmessage::Data(d) = f.submessage {
                    assert!(d.writer_sn.0 <= VOLATILE_SECURE_DEFAULT_DEPTH as i64);
                }
            }
        }
        // The reader acknowledges everything written: the waiting ones go out
        // in order with the next numbers.
        let acked = SequenceNumber(VOLATILE_SECURE_DEFAULT_DEPTH as i64 + 1);
        w.handle_acknack(reader, acked, alloc::vec![]);
        let released = w.tick(Duration::from_secs(2)).unwrap();
        assert_eq!(w.pending_count(), 0);
        let mut seen = Vec::new();
        for dg in &released {
            for f in decode_datagram(&dg.bytes).unwrap().submessages {
                if let ParsedSubmessage::Data(d) = f.submessage {
                    let m = decode_generic_message(d.serialized_payload.as_ref()).unwrap();
                    seen.push((d.writer_sn.0, m.message_identity.sequence_number));
                }
            }
        }
        let first = VOLATILE_SECURE_DEFAULT_DEPTH as i64 + 1;
        for i in 0..3 {
            assert!(seen.contains(&(first + i, first + i)), "{seen:?}");
        }
    }

    /// With the history and the waiting queue full, the write is an error
    /// the caller sees (OUT_OF_RESOURCES), not a silent drop.
    #[test]
    fn a_full_waiting_queue_is_an_error() {
        let mut w = VolatileSecureMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        w.add_reader_proxy(ReaderProxy::new(
            Guid::new(
                remote_prefix(),
                EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            ),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        ));
        for _ in 0..VOLATILE_SECURE_DEFAULT_DEPTH + VOLATILE_SECURE_PENDING_LIMIT {
            w.write(&sample_msg()).unwrap();
        }
        assert_eq!(w.pending_count(), VOLATILE_SECURE_PENDING_LIMIT);
        assert!(w.write(&sample_msg()).is_err());
        assert!(
            w.write_with_heartbeat(&sample_msg(), Duration::ZERO)
                .is_err()
        );
        assert_eq!(w.pending_count(), VOLATILE_SECURE_PENDING_LIMIT);
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
            data_flag: true,
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
            data_flag: true,
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
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::vec![0x00, 0x99, 0, 0].into(),
        };
        let err = r.handle_data(remote_prefix(), &data).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    fn participant_guid(prefix: GuidPrefix) -> [u8; 16] {
        Guid::new(prefix, EntityId::PARTICIPANT).to_bytes()
    }

    /// DDS-Security 1.2 §7.5.12: a datagram of the volatile writer holds
    /// submessages of that writer only — DATA, HEARTBEAT, GAP — and no
    /// submessage of another builtin or of an application topic.
    #[test]
    fn a_volatile_datagram_holds_the_submessages_of_its_writer_only() {
        use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
        let mut w = VolatileSecureMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        w.add_reader_proxy(ReaderProxy::new(
            Guid::new(
                remote_prefix(),
                EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            ),
            alloc::vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
            alloc::vec![],
            true,
        ));
        let dgs = w
            .write_with_heartbeat(&sample_msg(), Duration::ZERO)
            .unwrap();
        assert!(!dgs.is_empty());
        for dg in dgs {
            for sm in decode_datagram(&dg.bytes).unwrap().submessages {
                let writer = match sm.submessage {
                    ParsedSubmessage::Data(d) => Some(d.writer_id),
                    ParsedSubmessage::Heartbeat(h) => Some(h.writer_id),
                    ParsedSubmessage::Gap(g) => Some(g.writer_id),
                    ParsedSubmessage::InfoDestination(_) | ParsedSubmessage::InfoTimestamp(_) => {
                        None
                    }
                    other => panic!("a submessage of another topic: {other:?}"),
                };
                if let Some(writer) = writer {
                    assert_eq!(
                        writer,
                        EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
                    );
                }
            }
        }
    }

    /// DDS-Security 1.2 §7.5.4.4: a message with a destination participant
    /// goes as DATA to that participant's reader only; the writer's other
    /// readers get a GAP for its number.
    #[test]
    fn writer_sends_a_directed_message_to_its_destination_only() {
        use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
        let mut w = VolatileSecureMessageWriter::new(local_prefix(), VendorId::ZERODDS);
        let third = GuidPrefix::from_bytes([3; 12]);
        for (prefix, port) in [(remote_prefix(), 7411), (third, 7412)] {
            w.add_reader_proxy(ReaderProxy::new(
                Guid::new(
                    prefix,
                    EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
                ),
                alloc::vec![Locator::udp_v4([127, 0, 0, 1], port)],
                alloc::vec![],
                true,
            ));
        }
        let mut msg = sample_msg();
        msg.destination_participant_key = participant_guid(third);
        let dgs = w.write_with_heartbeat(&msg, Duration::ZERO).unwrap();
        let carries = |port: u32| {
            let mut data = false;
            let mut gap = false;
            for d in dgs
                .iter()
                .filter(|d| d.targets.iter().any(|t| t.port == port))
            {
                for sm in decode_datagram(&d.bytes).unwrap().submessages {
                    match sm.submessage {
                        ParsedSubmessage::Data(_) => data = true,
                        ParsedSubmessage::Gap(_) => gap = true,
                        _ => {}
                    }
                }
            }
            (data, gap)
        };
        assert_eq!(
            carries(7412),
            (true, false),
            "the destination gets the DATA"
        );
        assert_eq!(carries(7411), (false, true), "the other reader gets a GAP");
    }

    #[test]
    fn the_implied_content_filter() {
        let local = local_prefix();
        let mut m = sample_msg();
        assert!(
            is_addressed_to(&m, local),
            "GUID_UNKNOWN: every participant"
        );
        m.destination_participant_key = participant_guid(local);
        assert!(is_addressed_to(&m, local));
        m.destination_participant_key = participant_guid(remote_prefix());
        assert!(!is_addressed_to(&m, local), "another participant");
        m.destination_participant_key = participant_guid(local);
        m.destination_endpoint_key =
            Guid::new(remote_prefix(), EntityId::user_reader_with_key([1, 2, 3])).to_bytes();
        assert!(
            !is_addressed_to(&m, local),
            "an endpoint of another participant"
        );
        m.destination_endpoint_key =
            Guid::new(local, EntityId::user_reader_with_key([1, 2, 3])).to_bytes();
        assert!(is_addressed_to(&m, local));
    }

    /// The reader takes what is addressed to its participant and counts
    /// the rest, which a writer without the filter still sends it.
    #[test]
    fn reader_does_not_take_a_message_for_another_participant() {
        let mut r = VolatileSecureMessageReader::new(local_prefix(), VendorId::ZERODDS);
        r.add_writer_proxy(WriterProxy::new(
            Guid::new(
                remote_prefix(),
                EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            ),
            alloc::vec![],
            alloc::vec![],
            true,
        ));
        let mut msg = sample_msg();
        msg.destination_participant_key = participant_guid(GuidPrefix::from_bytes([3; 12]));
        let data = |sn: i64, m: &ParticipantGenericMessage| DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            writer_id: EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            writer_sn: SequenceNumber(sn),
            inline_qos: None,
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: encode_generic_message(m).into(),
        };
        assert!(
            r.handle_data(remote_prefix(), &data(1, &msg))
                .unwrap()
                .is_empty()
        );
        assert_eq!(r.not_addressed_count(), 1);
        msg.destination_participant_key = participant_guid(local_prefix());
        assert_eq!(
            r.handle_data(remote_prefix(), &data(2, &msg)).unwrap(),
            alloc::vec![msg]
        );
        assert_eq!(r.not_addressed_count(), 1);
    }
}
