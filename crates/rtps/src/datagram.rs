// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Datagram encoder/decoder: combines the RTPS header and submessages
//! into a finished wire datagram (W4).

extern crate alloc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::error::WireError;
use crate::header::RtpsHeader;
use crate::header_extension::{HeaderExtension, SUBMESSAGE_ID_HEADER_EXTENSION};
use crate::receiver_state::ReceiverState;
use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
use crate::submessages::{
    ACKNACK_FLAG_FINAL, AckNackSubmessage, DATA_FRAG_FLAG_INLINE_QOS, DATA_FRAG_FLAG_KEY,
    DATA_FRAG_FLAG_NON_STANDARD, DataFragSubmessage, DataSubmessage, GAP_FLAG_FILTERED_COUNT,
    GAP_FLAG_GROUP_INFO, GapSubmessage, HEARTBEAT_FLAG_FINAL, HEARTBEAT_FLAG_GROUP_INFO,
    HEARTBEAT_FLAG_LIVELINESS, HeartbeatFragSubmessage, HeartbeatSubmessage,
    INFO_REPLY_FLAG_MULTICAST, INFO_TIMESTAMP_FLAG_INVALIDATE, InfoReplyIp4Submessage,
    InfoReplySubmessage, InfoSourceSubmessage, InfoTimestampSubmessage, NackFragSubmessage,
};
use crate::wire_types::GuidPrefix;

/// Encodes an RTPS datagram = `RtpsHeader` + a sequence of `DATA`
/// submessages. Variant: all submessages are LE; a datagram carries a
/// list of DATA bodies.
pub fn encode_data_datagram(
    header: RtpsHeader,
    data_submessages: &[DataSubmessage],
) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::new();
    out.extend_from_slice(&header.to_bytes());
    for d in data_submessages {
        let (body, flags) = d.write_body(true);
        let body_len = u16::try_from(body.len()).map_err(|_| WireError::ValueOutOfRange {
            message: "DATA submessage body exceeds u16::MAX",
        })?;
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body_len,
        };
        out.extend_from_slice(&sh.to_bytes());
        out.extend_from_slice(&body);
    }
    Ok(out)
}

/// Parsed datagram: header + all recognized submessages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDatagram {
    /// RTPS header.
    pub header: RtpsHeader,
    /// All recognized submessages in order, each with the framing its
    /// own submessage header announced.
    pub submessages: Vec<SubmessageFrame>,
    /// What made the rest of the message invalid, after the last entry of
    /// [`Self::submessages`]: DDSI-RTPS 2.5 §8.3.4.1 rules 1, 2 and 6 — a
    /// submessage header that cannot be read, a submessageLength that is
    /// invalid, a known submessage that is invalid. The submessages before
    /// it stand. `None` for a message read to its end.
    pub invalid_rest: Option<WireError>,
}

impl ParsedDatagram {
    /// The submessage contents in order, without their framing. For a
    /// caller that only asks what arrived and not how it was framed.
    pub fn contents(&self) -> impl Iterator<Item = &ParsedSubmessage> {
        self.submessages.iter().map(|f| &f.submessage)
    }

    /// Walks the datagram's submessages together with the RTPS receiver
    /// state each one is attributed to (DDSI-RTPS 2.5 §8.3.4).
    ///
    /// `dest_guid_prefix` is the receiving participant's own GuidPrefix —
    /// nothing in the wire format supplies it until an InfoDestination
    /// submessage arrives (§8.3.4), so it is the caller's own identity that
    /// seeds it, exactly as [`ReceiverState::new`] documents.
    ///
    /// The state starts at the header's `sourceVersion`/`sourceVendorId`/
    /// `sourceGuidPrefix` (§8.3.4.1) and is then updated **in place** by
    /// every InfoSource (§8.3.8.10), InfoDestination, InfoReply/InfoReplyIp4
    /// (§8.3.8.9 / PSM §9.4.5.11) and InfoTimestamp (§8.3.8.11) submessage —
    /// each such submessage governs every submessage *after* it in the same
    /// datagram, per spec, not itself.
    ///
    /// Every submessage in the datagram is yielded, including the Info*
    /// ones and HeaderExtension: their own effect on the state is already
    /// folded in by the time they are yielded, so a caller only has to act
    /// on DATA/DATA_FRAG/HEARTBEAT/HEARTBEAT_FRAG/ACKNACK/NACK_FRAG/GAP —
    /// exactly the submessages that carry a `reader_id`/`writer_id` and so
    /// need a `source_guid_prefix` to complete their GUID — and can treat
    /// the rest as before (no-ops).
    ///
    /// This is the single place that walks a datagram with receiver state
    /// in force; every consumer that attributes a submessage to a writer or
    /// reader GUID must go through it instead of `parsed.header.guid_prefix`,
    /// which is only the *datagram's* source and not necessarily the
    /// individual submessage's — the bug INFO_SRC exists to prevent.
    #[must_use]
    pub fn receiver_walk(&self, dest_guid_prefix: GuidPrefix) -> ReceiverWalk<'_> {
        let mut state = ReceiverState::new(dest_guid_prefix);
        state.init_from_header(&self.header);
        Self::walk_from(self, state, dest_guid_prefix)
    }

    /// [`Self::receiver_walk`] for a datagram whose transport is known:
    /// the reply-locator lists start as Table 8.17 gives them for a message
    /// received by `kind` from `source_address` (`None` for a transport
    /// without a source address).
    #[must_use]
    pub fn receiver_walk_from(
        &self,
        dest_guid_prefix: GuidPrefix,
        kind: crate::wire_types::LocatorKind,
        source_address: Option<[u8; 16]>,
    ) -> ReceiverWalk<'_> {
        let mut state = ReceiverState::new(dest_guid_prefix);
        state.init_from_header(&self.header);
        state.set_initial_reply_locators(kind, source_address);
        Self::walk_from(self, state, dest_guid_prefix)
    }

    /// [`Self::receiver_walk`] in the context the datagram arrived in
    /// (DDSI-RTPS 2.5 §8.3.4): its source (transport kind and address, from
    /// which Table 8.17 gives the reply-locator lists their initial values)
    /// and the reception clock. Every receive path of a participant walks
    /// its datagrams through this.
    #[must_use]
    pub fn receiver_walk_in(
        &self,
        dest_guid_prefix: GuidPrefix,
        context: &ReceiveContext,
    ) -> ReceiverWalk<'_> {
        let walk = match &context.source {
            Some(from) => self.receiver_walk_from(dest_guid_prefix, from.kind, Some(from.address)),
            None => self.receiver_walk(dest_guid_prefix),
        };
        match context.clock {
            Some(clock) => walk.with_reception_clock(clock),
            None => walk,
        }
    }

    fn walk_from(&self, state: ReceiverState, dest_guid_prefix: GuidPrefix) -> ReceiverWalk<'_> {
        ReceiverWalk {
            state,
            own_prefix: dest_guid_prefix,
            frames: self.submessages.iter(),
            reception: None,
        }
    }
}

/// What the receiver needs to record a message's time of reception
/// (§8.3.7.4: `rtpsReceptionTimestamp = GetCurrentTime()`): the clock, and
/// the threshold of `CLOCK_SKEW_DETECTED`, which the spec does not give
/// (`None`: no skew detection).
#[derive(Debug, Clone, Copy)]
pub struct ReceptionClock {
    /// The current time.
    pub now: fn() -> crate::header_extension::HeTimestamp,
    /// Largest accepted difference between reception and send time, in
    /// seconds.
    pub skew_threshold_seconds: Option<u32>,
}

/// What a receive path knows about a datagram besides its octets: where it
/// came from and the clock to stamp its reception with. `Default` knows
/// neither.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReceiveContext {
    /// Source locator of the datagram (transport kind and address).
    pub source: Option<crate::wire_types::Locator>,
    /// Reception clock; `None` records no reception time.
    pub clock: Option<ReceptionClock>,
}

/// Iterator returned by [`ParsedDatagram::receiver_walk`]. Yields
/// `(ReceiverState, &ParsedSubmessage)` pairs — the state as it stands for
/// that submessage.
#[derive(Debug, Clone)]
pub struct ReceiverWalk<'a> {
    state: ReceiverState,
    /// The receiving participant, which an InfoDestination of
    /// GUIDPREFIX_UNKNOWN names (§8.3.8.8).
    own_prefix: GuidPrefix,
    frames: core::slice::Iter<'a, SubmessageFrame>,
    reception: Option<ReceptionClock>,
}

impl<'a> ReceiverWalk<'a> {
    /// A HeaderExtension with a timestamp then sets
    /// `rtpsReceptionTimestamp` and, with a threshold,
    /// `clockSkewDetected` (§8.3.7.4); an InfoTimestamp after it is
    /// corrected by the skew (§8.3.8.11).
    #[must_use]
    pub fn with_reception_clock(mut self, clock: ReceptionClock) -> Self {
        self.reception = Some(clock);
        self
    }
}

impl<'a> Iterator for ReceiverWalk<'a> {
    type Item = (ReceiverState, &'a ParsedSubmessage);

    fn next(&mut self) -> Option<Self::Item> {
        let frame = self.frames.next()?;
        match &frame.submessage {
            // §8.3.8.10: InfoSource sets sourceVersion/sourceVendorId/
            // sourceGuidPrefix for the submessages that follow it, resets
            // haveTimestamp and both reply-locator lists. It does *not*
            // touch destGuidPrefix — that stays whatever the last
            // InfoDestination (or the datagram's receiving context) set,
            // since InfoSource changes who sent the following submessages,
            // not who they are addressed to.
            ParsedSubmessage::InfoSource(i) => {
                self.state
                    .apply_info_source(i.protocol_version, i.vendor_id, i.guid_prefix);
            }
            // InfoDestination scopes the submessages that follow it to one
            // participant (PIM §8.3.8.8, PSM §9.4.5.9): "if
            // (InfoDestination.guidPrefix != GUIDPREFIX_UNKNOWN) {
            // Receiver.destGuidPrefix = InfoDestination.guidPrefix } else {
            // Receiver.destGuidPrefix = <GuidPrefix_t of the Participant
            // receiving the_message> }".
            ParsedSubmessage::InfoDestination(gp) => {
                let gp = GuidPrefix::from_bytes(*gp);
                self.state.dest_guid_prefix = if gp == GuidPrefix::UNKNOWN {
                    self.own_prefix
                } else {
                    gp
                };
            }
            // §8.3.8.11: sets haveTimestamp/timestamp, or clears
            // haveTimestamp on the invalidate (I) flag.
            ParsedSubmessage::InfoTimestamp(i) => {
                self.state.apply_info_timestamp(i.timestamp, i.invalidate);
            }
            // §8.3.8.9: sets unicastReplyLocatorList and, with the M flag,
            // multicastReplyLocatorList.
            ParsedSubmessage::InfoReply(i) => {
                self.state
                    .apply_info_reply(i.unicast_locators.clone(), i.multicast_locators.clone());
            }
            // PSM §9.4.5.11: the IPv4-only predecessor of InfoReply, same
            // receiver-state effect with exactly one locator per list.
            ParsedSubmessage::InfoReplyIp4(i) => {
                let unicast = alloc::vec![i.unicast_locator];
                let multicast = i.multicast_locator.map(|l| alloc::vec![l]);
                self.state.apply_info_reply(unicast, multicast);
            }
            // §8.3.7.4 HeaderExtension: L/W/C/P flags update messageLength/
            // timestamp/messageChecksum/parameters.
            ParsedSubmessage::HeaderExtension(he) => {
                self.state.apply_header_extension(he);
                if let (Some(clock), Some(_)) = (self.reception, he.timestamp) {
                    self.state
                        .note_reception((clock.now)(), clock.skew_threshold_seconds);
                }
            }
            ParsedSubmessage::Data(_)
            | ParsedSubmessage::DataFrag(_)
            | ParsedSubmessage::Heartbeat(_)
            | ParsedSubmessage::HeartbeatFrag(_)
            | ParsedSubmessage::AckNack(_)
            | ParsedSubmessage::NackFrag(_)
            | ParsedSubmessage::Gap(_)
            | ParsedSubmessage::Unknown { .. } => {} // ParsedSubmessage is #[non_exhaustive] for downstream crates;
                                                     // within this crate every current variant is matched above, so
                                                     // a future addition here must be given a receiver-state effect
                                                     // (or explicitly none) rather than silently falling through.
        }
        Some((self.state.clone(), &frame.submessage))
    }
}

/// One submessage as it stood in the datagram: the two framing
/// decisions its own header carried, and the decoded body.
///
/// Both framing fields are read off the wire rather than assumed,
/// because both are a sender's decision and both are invisible in a
/// round trip against our own encoder:
///
/// * §8.3.4.1 gives every submessage its **own** byte order in its `E`
///   flag, so a datagram may mix them. An encoder told the byte order
///   by its caller rather than by the message would rewrite a
///   big-endian submessage as little-endian and change every multi-byte
///   field in it.
/// * §8.3.3.2.3 lets the **last** submessage write
///   `octetsToNextHeader = 0` for "to the end of the message". OpenDDS
///   3.34.0 writes it that way on every datagram it sends and Cyclone,
///   Fast DDS and RTI write the true length; both are conformant and
///   the difference is one the message states, not one a re-encoder may
///   pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmessageFrame {
    /// Byte order announced by this submessage's `E` flag (§8.3.4.1).
    pub little_endian: bool,
    /// `octetsToNextHeader` was written as `0` — the last-submessage
    /// form of §8.3.3.2.3. Only the final submessage of a datagram may
    /// carry it.
    pub octets_elided: bool,
    /// The decoded submessage.
    pub submessage: ParsedSubmessage,
}

/// A recognized submessage. Supports DATA/HEARTBEAT/ACKNACK/GAP/DATA_FRAG/HEARTBEAT_FRAG/NACK_FRAG/INFO_*; others are skipped via `octets_to_next_header`
/// and recorded as [`ParsedSubmessage::Unknown`].
///
/// `#[non_exhaustive]`: DDSI-RTPS defines further submessage kinds and ZeroDDS
/// decodes more of them over time (e.g. `InfoDestination` was added for issue
/// #32). Downstream crates must include a wildcard arm when matching. This
/// follows the same policy as the other growable public wire enums in this
/// crate (see `WireError`, `HistoryCacheError`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParsedSubmessage {
    /// DATA submessage.
    Data(DataSubmessage),
    /// DATA_FRAG submessage (fragmentation).
    DataFrag(DataFragSubmessage),
    /// HEARTBEAT submessage.
    Heartbeat(HeartbeatSubmessage),
    /// HEARTBEAT_FRAG submessage.
    HeartbeatFrag(HeartbeatFragSubmessage),
    /// ACKNACK submessage.
    AckNack(AckNackSubmessage),
    /// NACK_FRAG submessage.
    NackFrag(NackFragSubmessage),
    /// GAP submessage.
    Gap(GapSubmessage),
    /// HeaderExtension-Submessage (DDSI-RTPS 2.5 §8.3.3.2).
    HeaderExtension(HeaderExtension),
    /// InfoSource-Submessage (§8.3.8.9.4).
    InfoSource(InfoSourceSubmessage),
    /// InfoReply-Submessage (§8.3.8.10.4).
    InfoReply(InfoReplySubmessage),
    /// InfoReplyIp4-Submessage (PSM §9.4.5.11): the deprecated IPv4-only
    /// predecessor of InfoReply. Updates the same receiver-state reply
    /// locator lists.
    InfoReplyIp4(InfoReplyIp4Submessage),
    /// InfoTimestamp-Submessage (§8.3.8.11).
    InfoTimestamp(InfoTimestampSubmessage),
    /// InfoDestination-Submessage (PIM §8.3.8.8, PSM §9.4.5.9): 12-byte dest
    /// GuidPrefix that scopes the following directed submessages to one
    /// participant (issue #32). A body shorter than 12 bytes is a wire error.
    InfoDestination([u8; 12]),
    /// Another submessage class (not interpreted). Carries id, flags
    /// and the body octets verbatim.
    ///
    /// The body is kept rather than dropped because §8.3.3.2 requires a
    /// receiver to *skip* a submessage it does not understand, not to
    /// erase it: anything that decodes a datagram and writes it out
    /// again — a router, a relay, a capture rewriter — would otherwise
    /// drop the content of every submessage class this crate has not
    /// grown a decoder for, and drop it silently.
    Unknown {
        /// Submessage ID byte.
        id: u8,
        /// Flag byte.
        flags: u8,
        /// The submessage body, exactly as it arrived.
        body: Vec<u8>,
    },
}

/// Decodes an RTPS datagram into header + submessage list.
///
/// `octets_to_next_header == 0` (last-submessage marker, spec §8.3.4.2)
/// is treated as: the submessage extends to the end of the datagram —
/// except for PAD and INFO_TS, whose zero length is a length (DDSI-RTPS 2.5
/// §9.4.5.1.3).
///
/// # Errors
/// `InvalidMagic`, `UnexpectedEof`, or a sub-decoder error. Unknown
/// submessage IDs are marked as `Unknown` (not an error).
pub fn decode_datagram(bytes: &[u8]) -> Result<ParsedDatagram, WireError> {
    decode_datagram_from(bytes, None)
}

/// [`decode_datagram`] for a datagram the caller holds as an `Arc`: the
/// serializedPayload of every DATA is a range of `datagram`, shared and
/// not copied out of it (see [`crate::payload::Payload`]).
///
/// # Errors
/// As [`decode_datagram`].
pub fn decode_datagram_shared(
    datagram: &alloc::sync::Arc<[u8]>,
) -> Result<ParsedDatagram, WireError> {
    decode_datagram_from(datagram, Some(datagram))
}

/// [`decode_datagram_shared`] into `frames`, which is cleared first and
/// keeps its capacity: a receive path that decodes datagram after datagram
/// hands each one the list the previous one was decoded into.
///
/// # Errors
/// As [`decode_datagram`].
pub fn decode_datagram_shared_into(
    datagram: &alloc::sync::Arc<[u8]>,
    frames: Vec<SubmessageFrame>,
) -> Result<ParsedDatagram, WireError> {
    decode_datagram_into(datagram, Some(datagram), frames)
}

/// The decoder behind both; `shared`, when given, is `bytes` as an `Arc`.
fn decode_datagram_from(
    bytes: &[u8],
    shared: Option<&alloc::sync::Arc<[u8]>>,
) -> Result<ParsedDatagram, WireError> {
    decode_datagram_into(bytes, shared, Vec::new())
}

/// Messages refused whole, see [`refused_messages`].
static REFUSED_MESSAGES: AtomicUsize = AtomicUsize::new(0);
/// Messages whose rest was invalid, see [`invalid_message_rests`].
static INVALID_MESSAGE_RESTS: AtomicUsize = AtomicUsize::new(0);
/// Submessages skipped for a must-understand PID, see
/// [`skipped_must_understand_submessages`].
static SKIPPED_MUST_UNDERSTAND: AtomicUsize = AtomicUsize::new(0);

/// Why a decoder did not take a message or a submessage whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionKind {
    /// The whole message was refused (header major version, HeaderExtension).
    Refused,
    /// A framing error or a known but invalid submessage made the rest of the
    /// message invalid (§8.3.4.1 rules 1, 2 and 6); what came before stands.
    InvalidRest,
    /// A DATA or DATA_FRAG with an unrecognized must-understand ParameterId
    /// was skipped (Table 9.6); the next submessage is read.
    MustUnderstandSkipped,
}

impl RejectionKind {
    /// The name of the kind in an observability event.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::InvalidRest => "invalid_rest",
            Self::MustUnderstandSkipped => "must_understand_skipped",
        }
    }
}

/// What [`set_rejection_hook`]'s function is told.
#[derive(Debug, Clone)]
pub struct Rejection {
    /// What happened.
    pub kind: RejectionKind,
    /// The SubmessageId octet of the submessage at fault; `None` for a
    /// refusal before any submessage (the RTPS header).
    pub submessage_id: Option<u8>,
    /// Offset of that submessage in the message.
    pub offset: usize,
    /// The VendorId of the message header.
    pub vendor_id: [u8; 2],
    /// The decoder's error.
    pub reason: WireError,
}

#[cfg(feature = "std")]
static REJECTION_HOOK: std::sync::OnceLock<fn(&Rejection)> = std::sync::OnceLock::new();

/// Installs the function that is told of every message or submessage a
/// decoder refused, invalidated or skipped (the cases the three counters
/// [`refused_messages`], [`invalid_message_rests`] and
/// [`skipped_must_understand_submessages`] count). Process-wide and set
/// once; `false` when a function was installed before. The function runs on
/// the decoding thread and has to be quick and to limit its own output.
#[cfg(feature = "std")]
pub fn set_rejection_hook(hook: fn(&Rejection)) -> bool {
    REJECTION_HOOK.set(hook).is_ok()
}

#[cfg(feature = "std")]
fn note_rejection(
    kind: RejectionKind,
    submessage_id: Option<u8>,
    offset: usize,
    header: &RtpsHeader,
    reason: &WireError,
) {
    if let Some(hook) = REJECTION_HOOK.get() {
        hook(&Rejection {
            kind,
            submessage_id,
            offset,
            vendor_id: header.vendor_id.0,
            reason: reason.clone(),
        });
    }
}

#[cfg(not(feature = "std"))]
fn note_rejection(_: RejectionKind, _: Option<u8>, _: usize, _: &RtpsHeader, _: &WireError) {}

/// How many messages the decoders of this process refused whole since it
/// started: a major version above the supported one (§8.3.6.3) or a
/// HeaderExtension that is invalid or carries an unrecognized
/// must-understand ParameterId (§8.3.7.3, Table 9.6). The counter is
/// process-wide, over every participant.
#[must_use]
pub fn refused_messages() -> usize {
    REFUSED_MESSAGES.load(Ordering::Relaxed)
}

/// How many messages had their rest invalidated by a known but invalid
/// submessage or by a framing error (§8.3.4.1 rules 1, 2 and 6): the
/// submessages before it were taken, the rest of the message was not.
/// Process-wide.
#[must_use]
pub fn invalid_message_rests() -> usize {
    INVALID_MESSAGE_RESTS.load(Ordering::Relaxed)
}

/// How many DATA and DATA_FRAG submessages were skipped for an
/// unrecognized must-understand ParameterId (Table 9.6). Process-wide.
#[must_use]
pub fn skipped_must_understand_submessages() -> usize {
    SKIPPED_MUST_UNDERSTAND.load(Ordering::Relaxed)
}

/// [`decode_datagram_from`] into `submessages`.
fn decode_datagram_into(
    bytes: &[u8],
    shared: Option<&alloc::sync::Arc<[u8]>>,
    mut submessages: Vec<SubmessageFrame>,
) -> Result<ParsedDatagram, WireError> {
    let header = RtpsHeader::from_bytes(bytes)?;
    // §8.3.6.3, third invalidity condition: the header is invalid when
    // its major protocol version exceeds the major version this
    // implementation supports. A greater major version may re-lay the
    // submessage framing itself, so the datagram cannot be skipped
    // submessage by submessage — the whole message is discarded. A
    // greater *minor* version stays readable by §8.6 and is accepted.
    if header.protocol_version.major > crate::wire_types::ProtocolVersion::CURRENT.major {
        REFUSED_MESSAGES.fetch_add(1, Ordering::Relaxed);
        let error = WireError::ValueOutOfRange {
            message: "RTPS header major version above the supported one",
        };
        note_rejection(RejectionKind::Refused, None, 0, &header, &error);
        return Err(error);
    }
    let mut pos = RtpsHeader::WIRE_SIZE;
    // Room for the submessages the datagram's size suggests: an unbatched
    // message carries one to three (DATA, DATA+HB, INFO_TS+DATA+HB), a
    // batched 1 kB stream an INFO_TS and a DATA per 1.1 kB — 26 in a
    // 14 720-octet datagram, which a list of four grew to in three steps.
    submessages.clear();
    submessages.reserve((4 + bytes.len() / 512).min(128));

    let mut invalid_rest = None;
    while pos < bytes.len() {
        // §8.3.4.1 rule 1: "If the full Submessage header cannot be read,
        // the rest of the Message is considered invalid."
        if bytes.len() < pos + SubmessageHeader::WIRE_SIZE {
            let error = WireError::UnexpectedEof {
                needed: SubmessageHeader::WIRE_SIZE,
                offset: pos,
            };
            note_rejection(RejectionKind::InvalidRest, None, pos, &header, &error);
            invalid_rest = Some(error);
            break;
        }
        // We read the submessage header first; on an unknown ID skip
        // resiliently.
        let id_byte = bytes[pos];
        let flags = bytes[pos + 1];
        let mut len_bytes = [0u8; 2];
        len_bytes.copy_from_slice(&bytes[pos + 2..pos + 4]);
        let little_endian = (flags & FLAG_E_LITTLE_ENDIAN) != 0;
        let octets = if little_endian {
            u16::from_le_bytes(len_bytes)
        } else {
            u16::from_be_bytes(len_bytes)
        };
        let body_start = pos + SubmessageHeader::WIRE_SIZE;
        // §9.4.5.1.3: a zero length makes the submessage the last one, up
        // to the end of the message — "other than PAD or INFO_TS". An
        // INFO_TS with the invalidate flag set is four octets of header
        // and nothing else, and what follows it is the next submessage;
        // read as the last one, everything after it in the datagram was
        // taken for its body and lost.
        let zero_length_is_a_length =
            id_byte == SubmessageId::Pad.as_u8() || id_byte == SubmessageId::InfoTs.as_u8();
        let body_end = if octets == 0 && !zero_length_is_a_length {
            // Last-submessage marker: to the end of the datagram.
            bytes.len()
        } else {
            body_start + octets as usize
        };
        // Rule 2: a submessageLength past the end of the message is invalid,
        // "the rest of the Message is invalid".
        if body_end > bytes.len() {
            let error = WireError::UnexpectedEof {
                needed: body_end - bytes.len(),
                offset: body_start,
            };
            note_rejection(
                RejectionKind::InvalidRest,
                Some(id_byte),
                pos,
                &header,
                &error,
            );
            invalid_rest = Some(error);
            break;
        }
        let sub = match decode_submessage(
            &header,
            &submessages,
            bytes,
            shared,
            id_byte,
            flags,
            little_endian,
            body_start,
            body_end,
        ) {
            Ok(Some(sub)) => sub,
            // Table 9.6: an unrecognized must-understand ParameterId in a
            // submessage other than the HeaderExtension — "ignore the
            // Submessage and continue with the next Submessage".
            Ok(None) => {
                SKIPPED_MUST_UNDERSTAND.fetch_add(1, Ordering::Relaxed);
                note_rejection(
                    RejectionKind::MustUnderstandSkipped,
                    Some(id_byte),
                    pos,
                    &header,
                    &WireError::ValueOutOfRange {
                        message: "unrecognized must-understand ParameterId in the inline QoS",
                    },
                );
                pos = body_end;
                continue;
            }
            Err(SubmessageError::Message(error)) => {
                REFUSED_MESSAGES.fetch_add(1, Ordering::Relaxed);
                note_rejection(RejectionKind::Refused, Some(id_byte), pos, &header, &error);
                return Err(error);
            }
            // Rule 6: "A known but invalid Submessage invalidates the rest
            // of the Message."
            Err(SubmessageError::Rest(error)) => {
                note_rejection(
                    RejectionKind::InvalidRest,
                    Some(id_byte),
                    pos,
                    &header,
                    &error,
                );
                invalid_rest = Some(error);
                break;
            }
        };
        submessages.push(SubmessageFrame {
            little_endian,
            octets_elided: octets == 0,
            submessage: sub,
        });
        pos = body_end;
    }

    if invalid_rest.is_some() {
        INVALID_MESSAGE_RESTS.fetch_add(1, Ordering::Relaxed);
    }
    Ok(ParsedDatagram {
        header,
        submessages,
        invalid_rest,
    })
}

/// DDSI-RTPS 2.5 §9.6.4 Table 9.20, note 4: "RTPS protocol versions prior to
/// 2.4 defined [PID_DIRECTED_WRITE] as a sequence<GUID_t>. … when
/// interacting with protocol versions earlier than 2.4 this parameter should
/// be ignored unless the receiver knows the format used by the vendor". A
/// sequence read as one GUID names a reader that does not exist, and the
/// sample would be absorbed as addressed to others.
fn ignore_directed_write_before_2_4(
    version: crate::wire_types::ProtocolVersion,
    inline_qos: &mut Option<crate::parameter_list::ParameterList>,
) {
    if (version.major, version.minor) >= (2, 4) {
        return;
    }
    if let Some(pl) = inline_qos {
        pl.parameters.retain(|p| {
            p.id & !(crate::parameter_list::MUST_UNDERSTAND_BIT
                | crate::parameter_list::VENDOR_SPECIFIC_BIT)
                != crate::parameter_list::pid::DIRECTED_WRITE
        });
    }
}

/// How a submessage failed: its error invalidates the rest of the message
/// (§8.3.4.1 rule 6), or the whole of it (a HeaderExtension, §8.3.7.3 and
/// Table 9.6).
enum SubmessageError {
    Rest(WireError),
    Message(WireError),
}

impl From<WireError> for SubmessageError {
    fn from(error: WireError) -> Self {
        Self::Rest(error)
    }
}

/// One submessage of [`decode_datagram_into`], `body` being
/// `bytes[body_start..body_end]`. `Ok(None)` for a DATA or DATA_FRAG that
/// carries an unrecognized must-understand ParameterId.
#[allow(clippy::too_many_arguments)]
fn decode_submessage(
    header: &RtpsHeader,
    submessages: &[SubmessageFrame],
    bytes: &[u8],
    shared: Option<&alloc::sync::Arc<[u8]>>,
    id_byte: u8,
    flags: u8,
    little_endian: bool,
    body_start: usize,
    body_end: usize,
) -> Result<Option<ParsedSubmessage>, SubmessageError> {
    let body = &bytes[body_start..body_end];
    Ok(Some(match SubmessageId::from_u8(id_byte) {
        Ok(SubmessageId::Data) => {
            let d = match shared {
                Some(datagram) => DataSubmessage::read_body_shared(
                    datagram,
                    body_start..body_end,
                    little_endian,
                    flags,
                )?,
                None => DataSubmessage::read_body_with_flags(body, little_endian, flags)?,
            };
            if d.inline_qos.as_ref().is_some_and(|pl| {
                pl.validate_must_understand_in_data_pipeline(header.vendor_id)
                    .is_err()
            }) {
                return Ok(None);
            }
            let mut d = d;
            ignore_directed_write_before_2_4(header.protocol_version, &mut d.inline_qos);
            ParsedSubmessage::Data(d)
        }
        Ok(SubmessageId::Heartbeat) => {
            let final_flag = (flags & HEARTBEAT_FLAG_FINAL) != 0;
            let liveliness_flag = (flags & HEARTBEAT_FLAG_LIVELINESS) != 0;
            let group_info_flag = (flags & HEARTBEAT_FLAG_GROUP_INFO) != 0;
            ParsedSubmessage::Heartbeat(HeartbeatSubmessage::read_body(
                body,
                little_endian,
                final_flag,
                liveliness_flag,
                group_info_flag,
            )?)
        }
        Ok(SubmessageId::AckNack) => {
            let final_flag = (flags & ACKNACK_FLAG_FINAL) != 0;
            ParsedSubmessage::AckNack(AckNackSubmessage::read_body(
                body,
                little_endian,
                final_flag,
            )?)
        }
        Ok(SubmessageId::Gap) => {
            let group_info_flag = (flags & GAP_FLAG_GROUP_INFO) != 0;
            let filtered_count_flag = (flags & GAP_FLAG_FILTERED_COUNT) != 0;
            ParsedSubmessage::Gap(GapSubmessage::read_body(
                body,
                little_endian,
                group_info_flag,
                filtered_count_flag,
            )?)
        }
        Ok(SubmessageId::DataFrag) => {
            let inline_qos = (flags & DATA_FRAG_FLAG_INLINE_QOS) != 0;
            let key = (flags & DATA_FRAG_FLAG_KEY) != 0;
            let non_standard = (flags & DATA_FRAG_FLAG_NON_STANDARD) != 0;
            let fragment =
                DataFragSubmessage::read_body(body, little_endian, inline_qos, key, non_standard)?;
            if fragment.inline_qos.as_ref().is_some_and(|qos| {
                qos.validate_must_understand_in_data_pipeline(header.vendor_id)
                    .is_err()
            }) {
                return Ok(None);
            }
            let mut fragment = fragment;
            ignore_directed_write_before_2_4(header.protocol_version, &mut fragment.inline_qos);
            ParsedSubmessage::DataFrag(fragment)
        }
        Ok(SubmessageId::HeartbeatFrag) => ParsedSubmessage::HeartbeatFrag(
            HeartbeatFragSubmessage::read_body(body, little_endian)?,
        ),
        Ok(SubmessageId::NackFrag) => {
            ParsedSubmessage::NackFrag(NackFragSubmessage::read_body(body, little_endian)?)
        }
        Ok(SubmessageId::InfoSrc) => {
            ParsedSubmessage::InfoSource(InfoSourceSubmessage::read_body(body, little_endian)?)
        }
        Ok(SubmessageId::InfoTs) => {
            let invalidate = (flags & INFO_TIMESTAMP_FLAG_INVALIDATE) != 0;
            ParsedSubmessage::InfoTimestamp(InfoTimestampSubmessage::read_body(
                body,
                little_endian,
                invalidate,
            )?)
        }
        Ok(SubmessageId::InfoDst) => {
            // DDSI-RTPS 2.5 InfoDestination carries exactly a 12-byte
            // GuidPrefix (PIM §8.3.8.8, PSM §9.4.5.9). A body shorter
            // than 12 bytes is an invalid submessage. Do not fold it into an
            // all-zero prefix: that is GUIDPREFIX_UNKNOWN, which would reset
            // the effective destination to the local participant and let
            // malformed wire data re-scope directed SEDP traffic (issue #32).
            if body.len() < 12 {
                return Err(SubmessageError::Rest(WireError::UnexpectedEof {
                    needed: 12 - body.len(),
                    offset: body_start,
                }));
            }
            let mut gp = [0u8; 12];
            gp.copy_from_slice(&body[..12]);
            ParsedSubmessage::InfoDestination(gp)
        }
        Ok(SubmessageId::InfoReply) => {
            let multicast_flag = (flags & INFO_REPLY_FLAG_MULTICAST) != 0;
            ParsedSubmessage::InfoReply(InfoReplySubmessage::read_body(
                body,
                little_endian,
                multicast_flag,
            )?)
        }
        Ok(SubmessageId::InfoReplyIp4) => {
            let multicast_flag = (flags & INFO_REPLY_FLAG_MULTICAST) != 0;
            ParsedSubmessage::InfoReplyIp4(InfoReplyIp4Submessage::read_body(
                body,
                little_endian,
                multicast_flag,
            )?)
        }
        // HeaderExtension (`RTPS_HE` = 0x00, §9.4.5.1.1), introduced in
        // RTPS 2.5: "Earlier versions of the protocol (RTPS 2.4 and
        // earlier) do not understand the HeaderExtension submessage"
        // (§8.3.3.2), so a message of an earlier version carries none.
        // It "may optionally appear immediately following the Header".
        Ok(_) | Err(WireError::UnknownSubmessageId { .. })
            if id_byte == SUBMESSAGE_ID_HEADER_EXTENSION
                && header.protocol_version
                    >= crate::wire_types::ProtocolVersion { major: 2, minor: 5 } =>
        {
            // §8.3.7.3: invalid when "The submessage is present but does not
            // immediately follow the RTPS Header". An invalid submessage
            // invalidates the rest of the message (§8.3.4.1 rule 6); the
            // submessages ahead of it stand.
            if !submessages.is_empty() {
                return Err(SubmessageError::Rest(WireError::ValueOutOfRange {
                    message: "HeaderExtension must appear directly after the RTPS header",
                }));
            }
            let he = HeaderExtension::decode_body(body, flags).map_err(SubmessageError::Message)?;
            // §8.3.7.3: invalid when "The messageLength is too small to
            // fit the RTPS Header and the HeaderExtension."
            if he
                .message_length
                .is_some_and(|len| (len as usize) < body_end)
            {
                return Err(SubmessageError::Message(WireError::ValueOutOfRange {
                    message: "HeaderExtension messageLength below the header and itself",
                }));
            }
            // §8.3.7.5: "The value of the messageLength shall indicate the
            // length of the entire RTPS Message starting from the beginning
            // of the RTPS Header."
            he.validate_message_length(u32::try_from(bytes.len()).unwrap_or(u32::MAX))
                .map_err(SubmessageError::Message)?;
            // §8.3.7.4: the messageChecksum, "computed over the entire RTPS
            // Message", detects corruption; a message whose checksum is not
            // the one its content gives is refused.
            if he.checksum.kind() != crate::header_extension::ChecksumKind::None
                && !HeaderExtension::verify_message(bytes).map_err(SubmessageError::Message)?
            {
                return Err(SubmessageError::Message(WireError::ValueOutOfRange {
                    message: "HeaderExtension messageChecksum does not match the message",
                }));
            }
            // Table 9.6: an unrecognized must-understand ParameterId
            // "in the HeaderExtension, ignore the entire RTPS message".
            if let Some(pl) = &he.parameters {
                pl.validate_must_understand_in_data_pipeline(header.vendor_id)
                    .map_err(SubmessageError::Message)?;
            }
            ParsedSubmessage::HeaderExtension(he)
        }
        // §8.3.4.1: "A Submessage with an unknown SubmessageId must be
        // ignored and parsing must continue with the next Submessage
        // … SubmessageIds in the vendor-specific range coming from a
        // vendorId that is unknown must also be ignored"; "The
        // receiver of a Submessage should ignore unknown flags." A
        // submessage has no must-understand flag.
        // Other known submessage IDs without a decoder
        // (PAD, InfoTs, …): skip and mark as Unknown.
        Ok(_) | Err(WireError::UnknownSubmessageId { .. }) => ParsedSubmessage::Unknown {
            id: id_byte,
            flags,
            body: body.to_vec(),
        },
        Err(other) => return Err(SubmessageError::Rest(other)),
    }))
}

/// Writes a parsed datagram back out: the RTPS header, then every
/// submessage with the framing its [`SubmessageFrame`] carries.
///
/// The inverse of [`decode_datagram`] for every submessage class this
/// crate decodes, and octet-for-octet so for a datagram whose fields
/// are all ones the decoder reads. Two things it does not restore, and
/// both are the sender's and not the message's:
///
/// * flag bits the submessage class does not define. §8.3.3.2 says a
///   receiver ignores them, so they are not read and not written.
/// * whatever a sender left in alignment padding inside a payload —
///   `serializedPayload` is carried verbatim, so this only concerns
///   padding *between* submessages, which RTPS does not have: every
///   submessage is a whole number of four-octet words by §8.3.3.2.3.
///
/// # Errors
/// `ValueOutOfRange` when a submessage body exceeds `u16::MAX`, when a
/// submessage other than the last one claims `octets_elided`, or when
/// [`HeaderExtension::encode_body`] refuses.
pub fn encode_datagram(datagram: &ParsedDatagram) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::new();
    out.extend_from_slice(&datagram.header.to_bytes());
    let last = datagram.submessages.len().saturating_sub(1);
    for (index, frame) in datagram.submessages.iter().enumerate() {
        let le = frame.little_endian;
        let (id, body, flags) = match &frame.submessage {
            ParsedSubmessage::Data(d) => {
                let (body, flags) = d.write_body(le);
                (SubmessageId::Data.as_u8(), body, flags)
            }
            ParsedSubmessage::DataFrag(d) => {
                let (body, flags) = d.write_body(le);
                (SubmessageId::DataFrag.as_u8(), body, flags)
            }
            ParsedSubmessage::Heartbeat(h) => {
                let (body, flags) = h.write_body(le);
                (SubmessageId::Heartbeat.as_u8(), body, flags)
            }
            ParsedSubmessage::HeartbeatFrag(h) => {
                let (body, flags) = (*h).write_body(le);
                (SubmessageId::HeartbeatFrag.as_u8(), body, flags)
            }
            ParsedSubmessage::AckNack(a) => {
                let (body, flags) = a.write_body(le);
                (SubmessageId::AckNack.as_u8(), body, flags)
            }
            ParsedSubmessage::NackFrag(n) => {
                let (body, flags) = n.write_body(le);
                (SubmessageId::NackFrag.as_u8(), body, flags)
            }
            ParsedSubmessage::Gap(g) => {
                let (body, flags) = g.write_body(le);
                (SubmessageId::Gap.as_u8(), body, flags)
            }
            ParsedSubmessage::InfoSource(i) => {
                let (body, flags) = i.write_body(le);
                (SubmessageId::InfoSrc.as_u8(), body, flags)
            }
            ParsedSubmessage::InfoReply(i) => {
                let (body, flags) = i.write_body(le);
                (SubmessageId::InfoReply.as_u8(), body, flags)
            }
            ParsedSubmessage::InfoReplyIp4(i) => {
                let (body, flags) = i.write_body(le);
                (SubmessageId::InfoReplyIp4.as_u8(), body, flags)
            }
            ParsedSubmessage::InfoTimestamp(i) => {
                let (body, flags) = i.write_body(le);
                (SubmessageId::InfoTs.as_u8(), body, flags)
            }
            ParsedSubmessage::InfoDestination(prefix) => {
                let flags = if le { FLAG_E_LITTLE_ENDIAN } else { 0 };
                (SubmessageId::InfoDst.as_u8(), prefix.to_vec(), flags)
            }
            ParsedSubmessage::HeaderExtension(he) => (
                SUBMESSAGE_ID_HEADER_EXTENSION,
                he.encode_body()?,
                he.flag_byte(),
            ),
            ParsedSubmessage::Unknown { id, flags, body } => (*id, body.clone(), *flags),
        };
        if frame.octets_elided && index != last {
            return Err(WireError::ValueOutOfRange {
                message: "octetsToNextHeader = 0 is the last submessage's form only",
            });
        }
        let octets = if frame.octets_elided {
            0u16
        } else {
            u16::try_from(body.len()).map_err(|_| WireError::ValueOutOfRange {
                message: "submessage body exceeds u16::MAX",
            })?
        };
        out.push(id);
        out.push(flags);
        out.extend_from_slice(&if le {
            octets.to_le_bytes()
        } else {
            octets.to_be_bytes()
        });
        out.extend_from_slice(&body);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use crate::wire_types::{EntityId, GuidPrefix, SequenceNumber, VendorId};
    use alloc::vec;

    fn header() -> RtpsHeader {
        RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::from_bytes([1; 12]))
    }

    fn data_msg(sn: i64, payload: &[u8]) -> DataSubmessage {
        DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([0xA, 0xB, 0xC]),
            writer_id: EntityId::user_writer_with_key([0x1, 0x2, 0x3]),
            writer_sn: SequenceNumber(sn),
            inline_qos: None,
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from(payload),
        }
    }

    /// §9.4.5.1.3: a zero `octetsToNextHeader` makes a submessage the last
    /// one, "other than PAD or INFO_TS". An INFO_TS with the invalidate
    /// flag set has no body, and the DATA behind it is decoded.
    #[test]
    fn an_info_ts_without_a_body_is_not_the_last_submessage() {
        use crate::message_builder::MessageBuilder;
        use crate::submessage_header::SubmessageId;

        let mut b = MessageBuilder::open(header(), alloc::sync::Arc::new(vec![]), 1400);
        b.try_add_submessage(
            SubmessageId::InfoTs,
            crate::submessages::INFO_TIMESTAMP_FLAG_INVALIDATE,
            &[],
        )
        .expect("info_ts fits");
        let (body, flags) = data_msg(1, b"behind-1").write_body(true);
        b.try_add_submessage(SubmessageId::Data, flags, &body)
            .expect("data fits");
        let bytes = b.finish().map(|d| d.bytes).expect("datagram");

        let parsed = decode_datagram(&bytes).expect("decode");
        let data: alloc::vec::Vec<&DataSubmessage> = parsed
            .contents()
            .filter_map(|s| match s {
                ParsedSubmessage::Data(d) => Some(d),
                _ => None,
            })
            .collect();
        assert_eq!(data.len(), 1, "the DATA behind the INFO_TS");
        assert_eq!(&*data[0].serialized_payload, b"behind-1");
    }

    /// The shared decode gives the submessages the copying one gives, and
    /// each DATA's payload is a range of the datagram itself.
    #[test]
    fn the_shared_decode_gives_ranges_of_the_datagram() {
        use crate::message_builder::MessageBuilder;
        use crate::submessage_header::SubmessageId;

        let mut b = MessageBuilder::open(header(), alloc::sync::Arc::new(vec![]), 1400);
        for (sn, payload) in [
            (1, &b"first-01"[..]),
            (2, &b"second-2"[..]),
            (3, &b"third"[..]),
        ] {
            let (body, flags) = data_msg(sn, payload).write_body(true);
            b.try_add_submessage(SubmessageId::Data, flags, &body)
                .expect("data fits");
        }
        let datagram: alloc::sync::Arc<[u8]> =
            alloc::sync::Arc::from(b.finish().map(|d| d.bytes).expect("datagram"));

        let copied = decode_datagram(&datagram).expect("decode");
        let shared = decode_datagram_shared(&datagram).expect("decode shared");
        // Into a list that held another datagram's frames: they are gone,
        // and the list's allocation is the one handed in.
        let mut reused = decode_datagram_shared(&datagram)
            .expect("decode shared")
            .submessages;
        reused.reserve(64);
        let (at, capacity) = (reused.as_ptr(), reused.capacity());
        let into = decode_datagram_shared_into(&datagram, reused).expect("decode into");
        assert_eq!(
            (into.submessages.as_ptr(), into.submessages.capacity()),
            (at, capacity)
        );
        let copied: alloc::vec::Vec<&ParsedSubmessage> = copied.contents().collect();
        let into: alloc::vec::Vec<&ParsedSubmessage> = into.contents().collect();
        let shared: alloc::vec::Vec<&ParsedSubmessage> = shared.contents().collect();
        assert_eq!(copied, shared);
        assert_eq!(into, shared);
        for s in shared {
            if let ParsedSubmessage::Data(d) = s {
                assert!(alloc::sync::Arc::ptr_eq(
                    d.serialized_payload.buffer(),
                    &datagram
                ));
            }
        }
    }

    /// §8.3.8.13 Pad: "the contents of the Submessage are ignored" — it
    /// exists solely to align what follows. A PAD between two DATA
    /// submessages must therefore neither fail the decode nor swallow
    /// the DATA behind it, and its body must not be interpreted.
    #[test]
    fn pad_submessage_is_skipped_and_the_data_behind_it_still_decodes() {
        use crate::message_builder::MessageBuilder;
        use crate::submessage_header::SubmessageId;

        let mut b = MessageBuilder::open(header(), alloc::sync::Arc::new(vec![]), 1400);
        let add_data = |b: &mut MessageBuilder, sn, payload: &[u8]| {
            let (body, flags) = data_msg(sn, payload).write_body(true);
            b.try_add_submessage(SubmessageId::Data, flags, &body)
                .expect("data fits");
        };
        // Eight octets, so the DATA ends on a 32-bit boundary and the PAD
        // behind it starts on one (DDSI-RTPS 2.5 §9.4.1).
        add_data(&mut b, 1, b"first-01");
        // A 4-octet PAD whose body is deliberately shaped like a
        // submessage header, so that "ignored" is more than "empty".
        b.try_add_submessage(
            SubmessageId::Pad,
            FLAG_E_LITTLE_ENDIAN,
            &[0x15, 0x05, 0x40, 0x00],
        )
        .expect("pad fits");
        add_data(&mut b, 2, b"second");
        let bytes = b.finish().map(|d| d.bytes).expect("datagram");

        let parsed = decode_datagram(&bytes).expect("decode");
        let ids: alloc::vec::Vec<u8> = parsed
            .contents()
            .map(|s| match s {
                ParsedSubmessage::Data(_) => SubmessageId::Data.as_u8(),
                ParsedSubmessage::Unknown { id, .. } => *id,
                other => panic!("unexpected submessage {other:?}"),
            })
            .collect();
        assert_eq!(
            ids,
            vec![
                SubmessageId::Data.as_u8(),
                SubmessageId::Pad.as_u8(),
                SubmessageId::Data.as_u8()
            ]
        );
        let payloads: alloc::vec::Vec<&[u8]> = parsed
            .contents()
            .filter_map(|s| match s {
                ParsedSubmessage::Data(d) => Some(&*d.serialized_payload),
                _ => None,
            })
            .collect();
        assert_eq!(payloads, vec![&b"first-01"[..], &b"second"[..]]);
    }

    /// Assembles `RTPS header + one submessage` byte-for-byte, without going
    /// through [`crate::message_builder::MessageBuilder`].
    ///
    /// The INFO_DST decoder tests below need exactly the message the builder
    /// refuses to produce: a message whose only submessage is framing. That
    /// refusal is deliberate on the send path (such a message tells the
    /// receiver nothing), but the *decoder* still has to read one correctly,
    /// because a peer may put one on the wire. So the fixture is written here
    /// rather than borrowed from the sender.
    fn framing_only_bytes(id: crate::submessage_header::SubmessageId, body: &[u8]) -> Vec<u8> {
        use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader};
        let hdr = RtpsHeader::new(
            crate::wire_types::VendorId::ZERODDS,
            crate::wire_types::GuidPrefix::from_bytes([1; 12]),
        );
        let mut out = alloc::vec::Vec::new();
        out.extend_from_slice(&hdr.to_bytes());
        let sh = SubmessageHeader {
            submessage_id: id,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: u16::try_from(body.len()).expect("body fits u16"),
        };
        out.extend_from_slice(&sh.to_bytes());
        out.extend_from_slice(body);
        out
    }

    #[test]
    fn info_dst_decodes_to_typed_destination_prefix() {
        use crate::submessage_header::SubmessageId;
        let dst = [0xAB_u8; 12];
        let bytes = framing_only_bytes(SubmessageId::InfoDst, &dst);
        let parsed = decode_datagram(&bytes).expect("decode");
        match parsed.contents().next() {
            Some(ParsedSubmessage::InfoDestination(gp)) => assert_eq!(*gp, dst),
            other => panic!("expected InfoDestination, got {other:?}"),
        }
    }

    #[test]
    fn info_dst_body_shorter_than_twelve_is_a_wire_error() {
        use crate::submessage_header::SubmessageId;
        // DDSI-RTPS 2.5: an InfoDestination shorter than a full 12-byte
        // GuidPrefix (PIM 8.3.8.8, PSM 9.4.5.9) is invalid and must not be
        // normalised to GUIDPREFIX_UNKNOWN (issue #32). Cover every deficient
        // length 0..=11, not just a sample.
        // §8.3.4.1 rule 6: the invalid submessage invalidates the rest of
        // the message; it is not taken in.
        for len in 0usize..12 {
            let body = alloc::vec![0x07u8; len];
            let bytes = framing_only_bytes(SubmessageId::InfoDst, &body);
            let parsed = decode_datagram(&bytes).unwrap();
            assert!(parsed.submessages.is_empty(), "INFO_DST body len {len}");
            assert!(
                matches!(parsed.invalid_rest, Some(WireError::UnexpectedEof { .. })),
                "INFO_DST body len {len} must invalidate the rest, got {:?}",
                parsed.invalid_rest
            );
        }
    }

    #[test]
    fn info_dst_body_of_exactly_twelve_decodes() {
        use crate::submessage_header::SubmessageId;
        let dst = [0x5A_u8; 12];
        let bytes = framing_only_bytes(SubmessageId::InfoDst, &dst);
        let parsed = decode_datagram(&bytes).expect("decode");
        assert!(matches!(
            parsed.contents().next(),
            Some(ParsedSubmessage::InfoDestination(gp)) if *gp == dst
        ));
    }

    #[test]
    fn info_dst_explicit_zero_prefix_is_preserved_as_unknown() {
        use crate::submessage_header::SubmessageId;
        // A deliberately transmitted all-zero prefix is the spec
        // GUIDPREFIX_UNKNOWN reset and must decode as [0; 12], distinct from a
        // malformed short body which is rejected above.
        let dst = [0u8; 12];
        let bytes = framing_only_bytes(SubmessageId::InfoDst, &dst);
        let parsed = decode_datagram(&bytes).expect("decode");
        assert!(matches!(
            parsed.contents().next(),
            Some(ParsedSubmessage::InfoDestination(gp)) if *gp == [0u8; 12]
        ));
    }

    #[test]
    fn encode_decode_single_data_datagram() {
        let h = header();
        let d = data_msg(1, b"hello");
        let bytes = encode_data_datagram(h, &[d.clone()]).unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.header, h);
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::Data(decoded) => assert_eq!(decoded, &d),
            other => panic!("expected Data, got {other:?}"),
        }
    }

    #[test]
    fn encode_decode_two_data_submessages() {
        let h = header();
        let d1 = data_msg(1, b"first");
        let d2 = data_msg(2, b"second-payload");
        let bytes = encode_data_datagram(h, &[d1.clone(), d2.clone()]).unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 2);
        match (
            &parsed.submessages[0].submessage,
            &parsed.submessages[1].submessage,
        ) {
            (ParsedSubmessage::Data(a), ParsedSubmessage::Data(b)) => {
                assert_eq!(a, &d1);
                assert_eq!(b, &d2);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn encode_decode_empty_payload() {
        let h = header();
        let d = data_msg(42, b"");
        let bytes = encode_data_datagram(h, &[d.clone()]).unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::Data(decoded) => {
                assert!(decoded.serialized_payload.is_empty());
                assert_eq!(decoded.writer_sn, SequenceNumber(42));
            }
            other => panic!("expected Data, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_invalid_magic() {
        let mut bytes = vec![0u8; 32];
        bytes[..4].copy_from_slice(b"XXXX");
        let res = decode_datagram(&bytes);
        assert!(matches!(res, Err(WireError::InvalidMagic { .. })));
    }

    #[test]
    fn pad_submessage_is_skipped_and_re_encoded_byte_for_byte() {
        // §8.3.8.13: PAD carries no fields. A receiver walks over its
        // body via octetsToNextHeader and reads the next submessage;
        // the body itself must survive the round trip untouched.
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let pad_body = [0xAAu8, 0xBB, 0xCC, 0xDD];
        let pad_header = SubmessageHeader {
            submessage_id: SubmessageId::Pad,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: u16::try_from(pad_body.len()).unwrap(),
        };
        bytes.extend_from_slice(&pad_header.to_bytes());
        bytes.extend_from_slice(&pad_body);
        let d = data_msg(9, b"after-pad");
        let (body, flags) = d.write_body(true);
        let data_header = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: u16::try_from(body.len()).unwrap(),
        };
        bytes.extend_from_slice(&data_header.to_bytes());
        bytes.extend_from_slice(&body);

        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 2);
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::Unknown { id, body: kept, .. } => {
                assert_eq!(*id, SubmessageId::Pad.as_u8());
                assert_eq!(kept.as_slice(), &pad_body);
            }
            other => panic!("expected the PAD, got {other:?}"),
        }
        match &parsed.submessages[1].submessage {
            ParsedSubmessage::Data(decoded) => {
                assert_eq!(decoded.writer_sn, SequenceNumber(9));
            }
            other => panic!("expected the DATA behind the PAD, got {other:?}"),
        }
        assert_eq!(encode_datagram(&parsed).unwrap(), bytes);
    }

    #[test]
    fn decode_rejects_major_version_above_supported() {
        // §8.3.6.3: a header whose major version exceeds the supported
        // one is invalid and the message is discarded.
        let h = header();
        let d = data_msg(1, b"payload");
        let mut bytes = encode_data_datagram(h, &[d]).unwrap();
        assert_eq!(bytes[4], 2, "the fixture header is major version 2");
        bytes[4] = 3;
        assert!(matches!(
            decode_datagram(&bytes),
            Err(WireError::ValueOutOfRange { .. })
        ));
    }

    #[test]
    fn decode_accepts_minor_version_above_supported() {
        // §8.6: a greater *minor* version keeps the framing, so the
        // datagram stays readable and is not rejected by §8.3.6.3.
        let h = header();
        let d = data_msg(1, b"payload");
        let mut bytes = encode_data_datagram(h, &[d]).unwrap();
        bytes[5] = 9;
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.header.protocol_version.minor, 9);
        assert_eq!(parsed.submessages.len(), 1);
    }

    #[test]
    fn decode_rejects_datagram_shorter_than_the_header() {
        // §8.3.6.3, first invalidity condition: fewer octets than a
        // full RTPS header.
        let bytes = b"RTPS\x02\x05".to_vec();
        assert!(matches!(
            decode_datagram(&bytes),
            Err(WireError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn decode_handles_last_submessage_zero_length() {
        // Construct manually: header + DATA-SH with octets=0
        // Body is DATA format (at least 20 bytes + payload).
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let d = data_msg(7, b"X");
        let (body, flags) = d.write_body(true);
        // Submessage header with octets=0 (last marker)
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: 0,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::Data(decoded) => {
                assert_eq!(decoded, &d);
            }
            other => panic!("expected Data, got {other:?}"),
        }
    }

    #[test]
    fn decode_marks_unknown_submessage_id_without_failing() {
        // Header + sub-header with ID 0x01 (PAD submessage). We use
        // PAD as a stand-in, because InfoTs is now typed-decoded (R3).
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let body = [0u8; 0]; // PAD has no body
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Pad,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::Unknown { id, flags, body } => {
                assert_eq!(*id, 0x01);
                assert_eq!(*flags, FLAG_E_LITTLE_ENDIAN);
                assert!(body.is_empty(), "PAD carries no body");
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn decode_heartbeat_preserves_final_and_liveliness_flags() {
        // Regression for the WP-1.1 finding: the F/L flag from the
        // submessage header must be available in HeartbeatSubmessage.
        let h = header();
        let hb = HeartbeatSubmessage {
            reader_id: crate::wire_types::EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: crate::wire_types::EntityId::user_writer_with_key([4, 5, 6]),
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(7),
            count: 42,
            final_flag: true,
            liveliness_flag: true,
            group_info: None,
        };
        let (body, flags) = hb.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Heartbeat,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::Heartbeat(decoded) => {
                assert_eq!(decoded, &hb);
                assert!(decoded.final_flag);
                assert!(decoded.liveliness_flag);
            }
            other => panic!("expected Heartbeat, got {other:?}"),
        }
    }

    #[test]
    fn decode_acknack_preserves_final_flag() {
        let h = header();
        let ack = AckNackSubmessage {
            reader_id: crate::wire_types::EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: crate::wire_types::EntityId::user_writer_with_key([4, 5, 6]),
            reader_sn_state: crate::submessages::SequenceNumberSet {
                bitmap_base: SequenceNumber(1),
                num_bits: 0,
                bitmap: vec![],
            },
            count: 3,
            final_flag: true,
        };
        let (body, flags) = ack.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::AckNack,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::AckNack(decoded) => {
                assert_eq!(decoded, &ack);
                assert!(decoded.final_flag);
            }
            other => panic!("expected AckNack, got {other:?}"),
        }
    }

    #[test]
    fn data_frag_spec_and_cyclone_flag_oracle() {
        // DDSI-RTPS 2.5 §9.4.5.5: K=0x04, N=0x08, unlike DATA.
        // Cyclone 11.0.1 ddsi__protocol.h: DDSI_DATAFRAG_FLAG_KEYFLAG=0x04u.
        // Literal wire flags exercise dispatch independently of our constants.
        for le in [false, true] {
            let word = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
            let short = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
            let mut body = Vec::new();
            body.extend_from_slice(&short(0));
            body.extend_from_slice(&short(28));
            body.extend_from_slice(&[0, 0, 1, 7, 0, 0, 2, 2]);
            for value in [0, 7, 1] {
                body.extend_from_slice(&word(value));
            }
            body.extend_from_slice(&short(1));
            body.extend_from_slice(&short(4));
            body.extend_from_slice(&word(4));
            body.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
            for (wire_flags, key, non_standard) in [
                (0x00, false, false),
                (0x04, true, false),
                (0x08, false, true),
                (0x0c, true, true),
            ] {
                let flags = wire_flags | u8::from(le);
                let mut bytes = header().to_bytes().to_vec();
                bytes.extend_from_slice(&[0x16, flags]);
                bytes.extend_from_slice(&short(body.len() as u16));
                bytes.extend_from_slice(&body);
                let parsed = decode_datagram(&bytes).unwrap();
                let ParsedSubmessage::DataFrag(decoded) = &parsed.submessages[0].submessage else {
                    panic!("expected DATA_FRAG");
                };
                assert_eq!(decoded.key_flag, key);
                assert_eq!(decoded.non_standard_flag, non_standard);
                assert_eq!(&*decoded.serialized_payload, &[0xaa, 0xbb, 0xcc, 0xdd]);
                let (encoded, encoded_flags) = decoded.write_body(le);
                assert_eq!(encoded_flags, flags);
                assert_eq!(encoded, body);
            }
        }
    }

    #[test]
    fn decode_data_frag_preserves_flags_and_payload() {
        let h = header();
        let df = DataFragSubmessage {
            inline_qos: None,
            extra_flags: 0,
            reader_id: crate::wire_types::EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: crate::wire_types::EntityId::user_writer_with_key([4, 5, 6]),
            writer_sn: SequenceNumber(7),
            fragment_starting_num: crate::wire_types::FragmentNumber(1),
            fragments_in_submessage: 1,
            fragment_size: 4,
            sample_size: 12,
            serialized_payload: alloc::sync::Arc::<[u8]>::from([0xAA, 0xBB, 0xCC, 0xDD].as_slice()),
            inline_qos_flag: false,
            key_flag: true,
            non_standard_flag: false,
        };
        let (body, flags) = df.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::DataFrag,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::DataFrag(decoded) => {
                assert_eq!(decoded, &df);
                assert!(decoded.key_flag);
                assert!(!decoded.inline_qos_flag);
            }
            other => panic!("expected DataFrag, got {other:?}"),
        }
    }

    #[test]
    fn decode_heartbeat_frag_roundtrip() {
        let h = header();
        let hf = HeartbeatFragSubmessage {
            reader_id: crate::wire_types::EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: crate::wire_types::EntityId::user_writer_with_key([4, 5, 6]),
            writer_sn: SequenceNumber(42),
            last_fragment_num: crate::wire_types::FragmentNumber(8),
            count: 3,
        };
        let (body, flags) = hf.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::HeartbeatFrag,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::HeartbeatFrag(decoded) => assert_eq!(decoded, &hf),
            other => panic!("expected HeartbeatFrag, got {other:?}"),
        }
    }

    #[test]
    fn decode_nack_frag_roundtrip() {
        let h = header();
        let nf = NackFragSubmessage {
            reader_id: crate::wire_types::EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: crate::wire_types::EntityId::user_writer_with_key([4, 5, 6]),
            writer_sn: SequenceNumber(5),
            fragment_number_state: crate::submessages::FragmentNumberSet {
                bitmap_base: crate::wire_types::FragmentNumber(1),
                num_bits: 4,
                bitmap: vec![0b1010_0000_0000_0000_0000_0000_0000_0000],
            },
            count: 1,
        };
        let (body, flags) = nf.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::NackFrag,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::NackFrag(decoded) => assert_eq!(decoded, &nf),
            other => panic!("expected NackFrag, got {other:?}"),
        }
    }

    // ---- WP 1.E stage E/F: InfoSource + InfoReply via datagram ----

    #[test]
    fn decode_info_source_via_datagram() {
        use crate::wire_types::{GuidPrefix, ProtocolVersion as PV, VendorId};
        let h = header();
        let info = InfoSourceSubmessage {
            unused: 0,
            protocol_version: PV::V2_5,
            vendor_id: VendorId([0xAB, 0xCD]),
            guid_prefix: GuidPrefix::from_bytes([3; 12]),
        };
        let (body, flags) = info.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::InfoSrc,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::InfoSource(decoded) => assert_eq!(decoded, &info),
            other => panic!("expected InfoSource, got {other:?}"),
        }
    }

    #[test]
    fn decode_info_reply_with_multicast_via_datagram() {
        use crate::wire_types::Locator;
        let h = header();
        let info = InfoReplySubmessage {
            unicast_locators: alloc::vec![Locator::udp_v4([10, 1, 2, 3], 7411)],
            multicast_locators: Some(alloc::vec![Locator::udp_v4([239, 255, 0, 1], 7400)]),
        };
        let (body, flags) = info.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::InfoReply,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::InfoReply(decoded) => assert_eq!(decoded, &info),
            other => panic!("expected InfoReply, got {other:?}"),
        }
    }

    /// §8.3.4.1 rule 1: "If the full Submessage header cannot be read, the
    /// rest of the Message is considered invalid."
    #[test]
    fn a_truncated_submessage_header_invalidates_the_rest() {
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        bytes.extend_from_slice(&[0u8, 0, 0]); // only 3 bytes of sub-header instead of 4
        let parsed = decode_datagram(&bytes).unwrap();
        assert!(parsed.submessages.is_empty());
        assert!(matches!(
            parsed.invalid_rest,
            Some(WireError::UnexpectedEof { .. })
        ));
    }

    /// §8.3.4.1 rule 2: a submessageLength that reaches past the end of the
    /// message is invalid, "the rest of the Message is invalid"; the INFO_TS
    /// ahead of it stands.
    #[test]
    fn a_submessage_length_past_the_end_invalidates_the_rest() {
        use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
        let mut bytes = framing_only_bytes(SubmessageId::InfoTs, &[1, 0, 0, 0, 0, 0, 0, 0]);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: 200,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&[0u8; 24]);
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        assert!(matches!(
            parsed.submessages[0].submessage,
            ParsedSubmessage::InfoTimestamp(_)
        ));
        assert!(matches!(
            parsed.invalid_rest,
            Some(WireError::UnexpectedEof { .. })
        ));
    }

    fn message_with_he_length(declared: u32) -> Vec<u8> {
        let he = crate::header_extension::HeaderExtension {
            little_endian: true,
            message_length: Some(declared),
            ..crate::header_extension::HeaderExtension::default()
        };
        let mut bytes = header().to_bytes().to_vec();
        bytes.extend_from_slice(&he.encode().unwrap());
        push_submessage(
            &mut bytes,
            SubmessageId::InfoTs,
            &[1, 0, 0, 0, 0, 0, 0, 0],
            0,
        );
        bytes
    }

    /// §8.3.7.3: a HeaderExtension is invalid when "The messageLength is too
    /// small to fit the RTPS Header and the HeaderExtension"; the message is
    /// invalid with it (the HeaderExtension is the first submessage).
    #[test]
    fn a_header_extension_message_length_too_small_for_header_and_extension_is_invalid() {
        let whole = message_with_he_length(0).len() as u32;
        // 20 octets of header and 8 of HeaderExtension: 28 is the least.
        for declared in [0u32, 8, 20, 27] {
            assert!(
                decode_datagram(&message_with_he_length(declared)).is_err(),
                "{declared}"
            );
        }
        let ok = decode_datagram(&message_with_he_length(whole)).unwrap();
        assert_eq!(ok.submessages.len(), 2);
        assert!(ok.invalid_rest.is_none());
    }

    /// §8.3.7.5: the messageLength is the length of the entire message from
    /// the beginning of the RTPS Header, so it is compared with the message
    /// as received: one octet more or fewer is a message that is not what it
    /// says.
    #[test]
    fn a_header_extension_message_length_counts_from_the_start_of_the_rtps_header() {
        let whole = message_with_he_length(0).len() as u32;
        assert_eq!(whole, 20 + 8 + 12);
        let ok = decode_datagram(&message_with_he_length(whole)).unwrap();
        assert!(ok.invalid_rest.is_none());
        // Counted from the first submessage (20 less) is no valid length.
        for declared in [whole - 20, whole - 1, whole + 1, whole + 20] {
            assert!(
                decode_datagram(&message_with_he_length(declared)).is_err(),
                "{declared}"
            );
        }
    }

    /// §8.3.4.1 rule 4: "The receiver of a Submessage should ignore unknown
    /// flags." The top three bits of the flags octet are defined by no
    /// submessage; set, they change nothing in what is read. One case per
    /// kind that has a constructor here; unknown kinds are in
    /// `decode_skips_an_unknown_submessage_whatever_its_flags`.
    #[test]
    fn unknown_flags_of_a_known_submessage_are_ignored() {
        use crate::submessages::{
            AckNackSubmessage, GapSubmessage, HeartbeatSubmessage, InfoTimestampSubmessage,
        };
        let sn = |n: i64| crate::wire_types::SequenceNumber(n);
        let set = |first: i64| crate::submessages::SequenceNumberSet {
            bitmap_base: sn(first),
            num_bits: 0,
            bitmap: Vec::new(),
        };
        let mut cases: Vec<(SubmessageId, Vec<u8>, u8)> = Vec::new();
        let (b, f) = data_msg(1, b"x").write_body(true);
        cases.push((SubmessageId::Data, b, f));
        let (b, f) = HeartbeatSubmessage {
            reader_id: crate::wire_types::EntityId::UNKNOWN,
            writer_id: crate::wire_types::EntityId::UNKNOWN,
            first_sn: sn(1),
            last_sn: sn(3),
            count: 1,
            final_flag: true,
            liveliness_flag: false,
            group_info: None,
        }
        .write_body(true);
        cases.push((SubmessageId::Heartbeat, b, f));
        let (b, f) = AckNackSubmessage {
            reader_id: crate::wire_types::EntityId::UNKNOWN,
            writer_id: crate::wire_types::EntityId::UNKNOWN,
            reader_sn_state: set(4),
            count: 1,
            final_flag: true,
        }
        .write_body(true);
        cases.push((SubmessageId::AckNack, b, f));
        let (b, f) = GapSubmessage {
            reader_id: crate::wire_types::EntityId::UNKNOWN,
            writer_id: crate::wire_types::EntityId::UNKNOWN,
            gap_start: sn(1),
            gap_list: set(3),
            group_info: None,
            filtered_count: None,
        }
        .write_body(true);
        cases.push((SubmessageId::Gap, b, f));
        let (b, f) = InfoTimestampSubmessage {
            timestamp: crate::header_extension::HeTimestamp {
                seconds: 5,
                fraction: 0,
            },
            invalidate: false,
        }
        .write_body(true);
        cases.push((SubmessageId::InfoTs, b, f));
        cases.push((
            SubmessageId::InfoDst,
            alloc::vec![7u8; 12],
            FLAG_E_LITTLE_ENDIAN,
        ));
        for (id, body, flags) in cases {
            let read = |flags: u8| {
                let mut bytes = header().to_bytes().to_vec();
                let sh = SubmessageHeader {
                    submessage_id: id,
                    flags,
                    octets_to_next_header: body.len() as u16,
                };
                bytes.extend_from_slice(&sh.to_bytes());
                bytes.extend_from_slice(&body);
                let parsed = decode_datagram(&bytes).unwrap();
                assert!(parsed.invalid_rest.is_none(), "{id:?}");
                alloc::format!("{:?}", parsed.contents().collect::<Vec<_>>())
            };
            assert_eq!(read(flags | 0xE0), read(flags), "{id:?}");
        }
    }

    /// §8.3.8.1 to §8.3.8.12 "Validity": a submessage whose `submessageLength`
    /// is too small for its body is invalid, and with it "the rest of the
    /// Message" (§8.3.4.1 rule 6); the INFO_TS ahead of it stands and the
    /// submessage behind it is not read. One case per submessage kind.
    #[test]
    fn a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before() {
        use crate::submessage_header::SubmessageId;
        let kinds = [
            SubmessageId::AckNack,
            SubmessageId::Data,
            SubmessageId::DataFrag,
            SubmessageId::Gap,
            SubmessageId::Heartbeat,
            SubmessageId::HeartbeatFrag,
            SubmessageId::InfoDst,
            SubmessageId::InfoReply,
            SubmessageId::InfoSrc,
            SubmessageId::InfoTs,
            SubmessageId::NackFrag,
        ];
        for id in kinds {
            let mut bytes = framing_only_bytes(SubmessageId::InfoTs, &[1, 0, 0, 0, 0, 0, 0, 0]);
            // Two octets: shorter than the fixed part of every kind above.
            push_submessage(&mut bytes, id, &[0, 0], 0);
            // A valid INFO_TS behind it, which must not be read.
            push_submessage(
                &mut bytes,
                SubmessageId::InfoTs,
                &[2, 0, 0, 0, 0, 0, 0, 0],
                0,
            );
            let parsed = decode_datagram(&bytes).unwrap();
            assert_eq!(
                parsed.submessages.len(),
                1,
                "{id:?}: what came before stands"
            );
            assert!(
                matches!(
                    parsed.submessages[0].submessage,
                    ParsedSubmessage::InfoTimestamp(_)
                ),
                "{id:?}"
            );
            assert!(parsed.invalid_rest.is_some(), "{id:?}: the rest is invalid");
        }
    }

    /// §8.3.4.1 rules 2 and 6: an invalid submessage invalidates "the rest
    /// of the Message"; the submessages ahead of it stand. An INFO_TS, an
    /// INFO_DST four octets short of a GuidPrefix, a HEARTBEAT behind it:
    /// the INFO_TS is read, the HEARTBEAT is not.
    #[test]
    fn the_submessages_before_an_invalid_one_stand() {
        use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
        let mut bytes = framing_only_bytes(SubmessageId::InfoTs, &[1, 0, 0, 0, 0, 0, 0, 0]);
        for (id, body) in [
            (SubmessageId::InfoDst, alloc::vec![7u8; 8]),
            (SubmessageId::Heartbeat, alloc::vec![0u8; 28]),
        ] {
            let sh = SubmessageHeader {
                submessage_id: id,
                flags: FLAG_E_LITTLE_ENDIAN,
                octets_to_next_header: u16::try_from(body.len()).unwrap(),
            };
            bytes.extend_from_slice(&sh.to_bytes());
            bytes.extend_from_slice(&body);
        }
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1, "{parsed:?}");
        assert!(matches!(
            parsed.submessages[0].submessage,
            ParsedSubmessage::InfoTimestamp(_)
        ));
        assert!(matches!(
            parsed.invalid_rest,
            Some(WireError::UnexpectedEof { .. })
        ));
    }

    /// A message rebuilt from a received one, as after its protected
    /// submessages were opened, takes the HeaderExtension with its own
    /// messageLength and without the checksum, once the received message
    /// passed both (`HeaderExtension::restate_for_rebuilt_message`). Kept
    /// as it was, the extension names the received length and the decoder
    /// refuses the rebuilt message: every opened message of CoreDX 6.14.1,
    /// which sends a messageLength, its crypto tokens among them. A received
    /// message whose messageLength is not its length is refused.
    #[test]
    fn a_rebuilt_message_takes_the_header_extension_with_its_own_length() {
        use crate::header_extension::{ChecksumValue, HeaderExtension};
        // PAD (0x01, little-endian) with `n` octets of body.
        let pad = |n: u16| {
            let mut s = vec![0x01, 0x01];
            s.extend_from_slice(&n.to_le_bytes());
            s.extend(core::iter::repeat_n(0u8, usize::from(n)));
            s
        };
        let mut he = HeaderExtension {
            little_endian: true,
            message_length: Some(0),
            checksum: ChecksumValue::Crc32c(0),
            ..HeaderExtension::default()
        };
        let he_len = he.encode().unwrap().len();
        he.message_length = Some(u32::try_from(20 + he_len + pad(16).len()).unwrap());
        let mut received = header().to_bytes().to_vec();
        received.extend_from_slice(&he.encode().unwrap());
        received.extend_from_slice(&pad(16));
        HeaderExtension::seal_message(&mut received).unwrap();
        assert!(decode_datagram(&received).is_ok());

        let mut rebuilt = received[..20 + he_len].to_vec();
        rebuilt.extend_from_slice(&pad(4));
        assert!(
            decode_datagram(&rebuilt).is_err(),
            "it names the received length"
        );
        HeaderExtension::restate_for_rebuilt_message(&received, &mut rebuilt).unwrap();
        let parsed = decode_datagram(&rebuilt).unwrap();
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::HeaderExtension(restated) => {
                assert_eq!(
                    restated.message_length,
                    Some(u32::try_from(rebuilt.len()).unwrap())
                );
                assert_eq!(restated.checksum, ChecksumValue::None);
            }
            other => panic!("{other:?}"),
        }

        let mut longer = received.clone();
        longer.extend_from_slice(&pad(0));
        let mut rebuilt = longer[..20 + he_len].to_vec();
        rebuilt.extend_from_slice(&pad(4));
        assert!(HeaderExtension::restate_for_rebuilt_message(&longer, &mut rebuilt).is_err());
    }

    #[test]
    fn decode_header_extension_in_datagram() {
        let h = header();
        // The message is the header and the HeaderExtension: 20 + 4 + body.
        let mut he = crate::header_extension::HeaderExtension {
            little_endian: true,
            message_length: Some(0),
            timestamp: Some(crate::header_extension::HeTimestamp {
                seconds: 1,
                fraction: 2,
            }),
            // §8.3.7.4: the checksum of the entire message, its own bytes
            // zeroed; the message is sealed with it below.
            checksum: crate::header_extension::ChecksumValue::Crc32c(0),
            ..crate::header_extension::HeaderExtension::default()
        };
        he.message_length = Some(20 + he.encode().unwrap().len() as u32);
        let mut bytes = h.to_bytes().to_vec();
        bytes.extend_from_slice(&he.encode().unwrap());
        crate::header_extension::HeaderExtension::seal_message(&mut bytes).unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::HeaderExtension(decoded) => {
                let mut sealed = he.clone();
                sealed.checksum = decoded.checksum.clone();
                assert_eq!(decoded, &sealed);
                assert_ne!(
                    decoded.checksum,
                    crate::header_extension::ChecksumValue::Crc32c(0)
                );
            }
            other => panic!("expected HE, got {other:?}"),
        }
    }

    /// §8.3.4.1 rules 3 and 4: an unknown submessage is skipped whatever
    /// its flags, and parsing goes on with the next one.
    #[test]
    fn decode_skips_an_unknown_submessage_whatever_its_flags() {
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let body = [0u8; 4];
        for id in [0x7E, 0x80, 0x81, 0xFF] {
            let sh = SubmessageHeader {
                submessage_id: SubmessageId::Pad, // the ID value is overwritten below
                flags: FLAG_E_LITTLE_ENDIAN | 0x80,
                octets_to_next_header: body.len() as u16,
            };
            let mut sh_bytes = sh.to_bytes();
            sh_bytes[0] = id;
            bytes.extend_from_slice(&sh_bytes);
            bytes.extend_from_slice(&body);
        }
        let pad = SubmessageHeader {
            submessage_id: SubmessageId::Pad,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: 0,
        };
        bytes.extend_from_slice(&pad.to_bytes());
        let parsed = decode_datagram(&bytes).unwrap();
        let ids: Vec<u8> = parsed
            .submessages
            .iter()
            .filter_map(|f| match &f.submessage {
                ParsedSubmessage::Unknown { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(ids, [0x7E, 0x80, 0x81, 0xFF, SubmessageId::Pad.as_u8()]);
    }

    #[test]
    fn decode_skips_unknown_submessage_without_must_understand() {
        // Without the Must-Understand bit: skip + mark as Unknown.
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let body = [0u8; 4];
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Pad,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: body.len() as u16,
        };
        let mut sh_bytes = sh.to_bytes();
        sh_bytes[0] = 0x7E;
        bytes.extend_from_slice(&sh_bytes);
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::Unknown { id, .. } => assert_eq!(*id, 0x7E),
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn decode_data_after_header_extension() {
        // Wire-Layout: RtpsHeader || HE || DATA, messageLength the whole
        // message's (§8.3.3.2).
        let h = header();
        let d = data_msg(7, b"after-he");
        let (body, flags) = d.write_body(true);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        let with_length = |len: u32| {
            let he = crate::header_extension::HeaderExtension {
                little_endian: true,
                message_length: Some(len),
                ..crate::header_extension::HeaderExtension::default()
            };
            let mut bytes = h.to_bytes().to_vec();
            bytes.extend_from_slice(&he.encode().unwrap());
            bytes.extend_from_slice(&sh.to_bytes());
            bytes.extend_from_slice(&body);
            bytes
        };
        let total = with_length(0).len() as u32;
        let bytes = with_length(total);
        // §8.3.7.3: "The messageLength is too small to fit the RTPS Header
        // and the HeaderExtension" makes it invalid.
        assert!(decode_datagram(&with_length(20)).is_err());
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 2);
        assert!(matches!(
            &parsed.submessages[0].submessage,
            ParsedSubmessage::HeaderExtension(_)
        ));
        assert!(matches!(
            &parsed.submessages[1].submessage,
            ParsedSubmessage::Data(_)
        ));
    }

    #[test]
    fn decode_rejects_header_extension_after_data_submessage() {
        // Spec §8.3.7.3: HE MUST appear directly after the RTPS header.
        // If another submessage was parsed before it, the HE is invalid: it
        // and the rest of the message are, the DATA ahead of it stands.
        // Wire layout: RtpsHeader || DATA || HE.
        let h = header();
        let d = data_msg(7, b"first");
        let he = crate::header_extension::HeaderExtension {
            little_endian: true,
            message_length: Some(0),
            ..crate::header_extension::HeaderExtension::default()
        };
        let mut bytes = h.to_bytes().to_vec();
        let (dbody, dflags) = d.write_body(true);
        let dsh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags: dflags,
            octets_to_next_header: dbody.len() as u16,
        };
        bytes.extend_from_slice(&dsh.to_bytes());
        bytes.extend_from_slice(&dbody);
        bytes.extend_from_slice(&he.encode().unwrap());
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        assert!(matches!(
            parsed.submessages[0].submessage,
            ParsedSubmessage::Data(_)
        ));
        assert!(matches!(
            parsed.invalid_rest,
            Some(WireError::ValueOutOfRange { .. })
        ));
    }

    /// DDSI-RTPS 2.5 §8.3.7.4: a message whose `messageChecksum` is not the
    /// one its content gives is refused (and counted); the sealed one is read.
    #[test]
    fn a_message_with_a_wrong_checksum_is_refused_and_counted() {
        use crate::header_extension::{ChecksumKind, ChecksumValue, HeaderExtension};
        let he = HeaderExtension {
            little_endian: true,
            checksum: ChecksumValue::Crc32c(0),
            ..HeaderExtension::default()
        };
        let mut bytes = header().to_bytes().to_vec();
        bytes.extend_from_slice(&he.encode().unwrap());
        bytes.extend_from_slice(&[0x09, 0x01, 8, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
        HeaderExtension::seal_message(&mut bytes).unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 2);
        let ParsedSubmessage::HeaderExtension(read) = &parsed.submessages[0].submessage else {
            panic!("expected HE");
        };
        assert_eq!(read.checksum.kind(), ChecksumKind::Crc32c);
        let before = refused_messages();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x80;
        let res = decode_datagram(&bytes);
        assert!(
            matches!(res, Err(WireError::ValueOutOfRange { .. })),
            "{res:?}"
        );
        assert!(refused_messages() > before);
    }

    // ---- §9.4.2.11.2 Must-Understand-Bit reject path ----

    /// Table 9.6: an unrecognized must-understand ParameterId in a DATA —
    /// "ignore the Submessage and continue with the next Submessage in the
    /// RTPS message, if any". The DATA is left out, the INFO_TS behind it is
    /// read.
    #[test]
    fn a_data_with_an_unknown_must_understand_pid_is_skipped() {
        use crate::parameter_list::{MUST_UNDERSTAND_BIT, Parameter, ParameterList};
        let h = header();
        // Inline QoS with an unknown MU PID 0x3500 (not a standard PID).
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | 0x3500,
            vec![1, 2, 3, 4],
        ));
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([0xA, 0xB, 0xC]),
            writer_id: EntityId::user_writer_with_key([0x1, 0x2, 0x3]),
            writer_sn: SequenceNumber(1),
            inline_qos: Some(pl),
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from([] as [u8; 0]),
        };
        let mut bytes = h.to_bytes().to_vec();
        let (body, flags) = d.write_body(true);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let ts = SubmessageHeader {
            submessage_id: SubmessageId::InfoTs,
            flags: crate::submessage_header::FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: 8,
        };
        bytes.extend_from_slice(&ts.to_bytes());
        bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1, "{parsed:?}");
        assert!(matches!(
            parsed.submessages[0].submessage,
            ParsedSubmessage::InfoTimestamp(_)
        ));
        assert_eq!(parsed.invalid_rest, None);
    }

    #[test]
    fn decode_accepts_data_with_known_must_understand_pid_in_inline_qos() {
        use crate::parameter_list::{MUST_UNDERSTAND_BIT, Parameter, ParameterList, pid};
        let h = header();
        let mut pl = ParameterList::new();
        // KEY_HASH is a standard PID, MU bit allowed.
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | pid::KEY_HASH,
            vec![0; 16],
        ));
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([0xA, 0xB, 0xC]),
            writer_id: EntityId::user_writer_with_key([0x1, 0x2, 0x3]),
            writer_sn: SequenceNumber(2),
            inline_qos: Some(pl),
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from([] as [u8; 0]),
        };
        let mut bytes = h.to_bytes().to_vec();
        let (body, flags) = d.write_body(true);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        decode_datagram(&bytes).expect("known MU PID should pass");
    }

    #[test]
    fn a_data_with_a_vendor_specific_must_understand_pid_is_skipped() {
        use crate::parameter_list::{
            MUST_UNDERSTAND_BIT, Parameter, ParameterList, VENDOR_SPECIFIC_BIT,
        };
        let h = header();
        let mut pl = ParameterList::new();
        // Table 9.6: not recognized, so an error; the DATA is skipped.
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT | 0x3500,
            vec![0xCA, 0xFE, 0xBA, 0xBE],
        ));
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([0xA, 0xB, 0xC]),
            writer_id: EntityId::user_writer_with_key([0x1, 0x2, 0x3]),
            writer_sn: SequenceNumber(3),
            inline_qos: Some(pl),
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from([] as [u8; 0]),
        };
        let mut bytes = h.to_bytes().to_vec();
        let (body, flags) = d.write_body(true);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        assert!(parsed.submessages.is_empty(), "{parsed:?}");
        assert_eq!(parsed.invalid_rest, None);
    }

    #[test]
    fn rtps_2_1_treats_0x80_as_vendor_specific_not_header_extension() {
        use crate::wire_types::ProtocolVersion;
        // Spec §8.3.7.3: HeaderExtension (0x80) is a 2.5 submessage.
        // Older vendors (e.g. Cyclone 2.1) may use 0x80 as
        // vendor-specific. We do NOT discard, but skip.
        let mut h = header();
        h.protocol_version = ProtocolVersion::V2_1;
        let mut bytes = h.to_bytes().to_vec();
        // First a real DATA submessage (so `submessages` becomes
        // non-empty), then 0x80 as a vendor submessage appended.
        let d = data_msg(1, b"x");
        let (body, flags) = d.write_body(true);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        // Vendor submessage 0x80 with a 4-byte body, no MU bit.
        bytes.extend_from_slice(&[0x80, FLAG_E_LITTLE_ENDIAN, 4, 0]);
        bytes.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let parsed = decode_datagram(&bytes).expect("0x80 under RTPS 2.1 must skip");
        assert!(matches!(
            parsed.contents().last(),
            Some(ParsedSubmessage::Unknown { id: 0x80, .. })
        ));
    }

    #[cfg(feature = "std")]
    static SEEN_REJECTIONS: std::sync::Mutex<Vec<Rejection>> = std::sync::Mutex::new(Vec::new());

    #[cfg(feature = "std")]
    /// A known but invalid submessage (GAP with gapStart 0) and a vendor
    /// submessage 0x80 in one message: the hook is told of the GAP with its
    /// id, offset, vendor and reason, the 0x80 submessage in front of it is
    /// skipped without a report, and `invalid_rest` carries the same error.
    #[test]
    fn a_rejected_submessage_is_reported_to_the_hook() {
        assert!(
            set_rejection_hook(|r| {
                if r.vendor_id == [0xAB, 0xCD] {
                    SEEN_REJECTIONS.lock().unwrap().push(r.clone());
                }
            }) || REJECTION_HOOK.get().is_some()
        );
        let mut h = header();
        h.vendor_id = crate::wire_types::VendorId([0xAB, 0xCD]);
        let mut bytes = h.to_bytes().to_vec();
        bytes.extend_from_slice(&[0x80, FLAG_E_LITTLE_ENDIAN, 4, 0, 1, 2, 3, 4]);
        let at = bytes.len();
        let mut gap = alloc::vec![0u8; 8];
        gap.extend_from_slice(&0i64.to_le_bytes());
        gap.extend_from_slice(&1i64.to_le_bytes());
        gap.extend_from_slice(&0u32.to_le_bytes());
        push_submessage(&mut bytes, SubmessageId::Gap, &gap, 0);
        let parsed = decode_datagram(&bytes).unwrap();
        assert!(parsed.invalid_rest.is_some());
        let seen = SEEN_REJECTIONS.lock().unwrap();
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert_eq!(seen[0].kind, RejectionKind::InvalidRest);
        assert_eq!(seen[0].submessage_id, Some(SubmessageId::Gap.as_u8()));
        assert_eq!(seen[0].offset, at);
        assert_eq!(Some(&seen[0].reason), parsed.invalid_rest.as_ref());
    }

    // ---- receiver_walk (DDSI-RTPS 2.5 §8.3.4) ----

    fn push_submessage(bytes: &mut Vec<u8>, id: SubmessageId, body: &[u8], extra_flags: u8) {
        let sh = SubmessageHeader {
            submessage_id: id,
            flags: FLAG_E_LITTLE_ENDIAN | extra_flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(body);
    }

    #[test]
    fn receiver_walk_without_info_source_attributes_to_header_prefix() {
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let (body, flags) = data_msg(1, b"a").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &body,
            flags & !FLAG_E_LITTLE_ENDIAN,
        );
        let parsed = decode_datagram(&bytes).unwrap();
        let dest = GuidPrefix::from_bytes([9; 12]);
        let walked: Vec<_> = parsed.receiver_walk(dest).collect();
        assert_eq!(walked.len(), 1);
        assert_eq!(walked[0].0.source_guid_prefix, h.guid_prefix);
        assert_eq!(walked[0].0.dest_guid_prefix, dest);
    }

    /// Regression for the bug this walker fixes: a datagram whose header
    /// carries one participant's prefix but whose submessages, separated by
    /// INFO_SRC, actually originate from two other participants — as
    /// OpenSplice's DDSI2 service does when it bundles the submessages of
    /// several participants it hosts into one datagram.
    #[test]
    fn receiver_walk_attributes_submessages_after_info_source_to_the_new_prefix() {
        use crate::wire_types::{ProtocolVersion as PV, VendorId};
        let header_prefix = GuidPrefix::from_bytes([0xAA; 12]);
        let source_a = GuidPrefix::from_bytes([0x11; 12]);
        let source_b = GuidPrefix::from_bytes([0x22; 12]);
        let h = RtpsHeader::new(VendorId::ZERODDS, header_prefix);
        let mut bytes = h.to_bytes().to_vec();

        // Submessage before any INFO_SRC: attributed to the header prefix.
        let (d0_body, d0_flags) = data_msg(1, b"pre").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &d0_body,
            d0_flags & !FLAG_E_LITTLE_ENDIAN,
        );

        // INFO_SRC → source_a
        let info_a = InfoSourceSubmessage {
            unused: 0,
            protocol_version: PV::V2_5,
            vendor_id: VendorId([9, 9]),
            guid_prefix: source_a,
        };
        let (ia_body, ia_flags) = info_a.write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::InfoSrc,
            &ia_body,
            ia_flags & !FLAG_E_LITTLE_ENDIAN,
        );
        let (d1_body, d1_flags) = data_msg(2, b"a").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &d1_body,
            d1_flags & !FLAG_E_LITTLE_ENDIAN,
        );

        // INFO_SRC → source_b
        let info_b = InfoSourceSubmessage {
            unused: 0,
            protocol_version: PV::V2_5,
            vendor_id: VendorId([9, 9]),
            guid_prefix: source_b,
        };
        let (ib_body, ib_flags) = info_b.write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::InfoSrc,
            &ib_body,
            ib_flags & !FLAG_E_LITTLE_ENDIAN,
        );
        let (d2_body, d2_flags) = data_msg(3, b"b").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &d2_body,
            d2_flags & !FLAG_E_LITTLE_ENDIAN,
        );

        let parsed = decode_datagram(&bytes).unwrap();
        let dest = GuidPrefix::from_bytes([1; 12]);
        let data_states: Vec<GuidPrefix> = parsed
            .receiver_walk(dest)
            .filter_map(|(state, sub)| match sub {
                ParsedSubmessage::Data(_) => Some(state.source_guid_prefix),
                _ => None,
            })
            .collect();
        assert_eq!(data_states, vec![header_prefix, source_a, source_b]);
    }

    /// InfoSource must not reset destGuidPrefix — an InfoDestination that
    /// scoped the datagram to one participant stays in force across an
    /// intervening InfoSource, exactly as OpenSplice's captures rely on
    /// (INFO_DST(pub) precedes both services' INFO_SRC blocks in the same
    /// datagram).
    #[test]
    fn receiver_walk_info_source_does_not_reset_dest_guid_prefix() {
        use crate::wire_types::{ProtocolVersion as PV, VendorId};
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let dst = [0x77u8; 12];
        push_submessage(&mut bytes, SubmessageId::InfoDst, &dst, 0);
        let info = InfoSourceSubmessage {
            unused: 0,
            protocol_version: PV::V2_5,
            vendor_id: VendorId([1, 1]),
            guid_prefix: GuidPrefix::from_bytes([0x33; 12]),
        };
        let (ibody, iflags) = info.write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::InfoSrc,
            &ibody,
            iflags & !FLAG_E_LITTLE_ENDIAN,
        );
        let (dbody, dflags) = data_msg(1, b"x").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &dbody,
            dflags & !FLAG_E_LITTLE_ENDIAN,
        );

        let parsed = decode_datagram(&bytes).unwrap();
        let (state, sub) = parsed
            .receiver_walk(GuidPrefix::from_bytes([0; 12]))
            .find(|(_, s)| matches!(s, ParsedSubmessage::Data(_)))
            .unwrap();
        assert!(matches!(sub, ParsedSubmessage::Data(_)));
        assert_eq!(state.dest_guid_prefix, GuidPrefix::from_bytes(dst));
        assert_eq!(state.source_guid_prefix, GuidPrefix::from_bytes([0x33; 12]));
    }

    #[test]
    fn receiver_walk_info_timestamp_invalidate_clears_have_timestamp() {
        // An invalidating InfoTimestamp has an empty body, and §8.3.3.2.3
        // lets `octetsToNextHeader = 0` mean "read to end of datagram" only
        // for the *last* submessage — so this fixture, like a real sender,
        // places the invalidating InfoTs last. The Data submessage before
        // it still gets the earlier, valid timestamp.
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let ts = InfoTimestampSubmessage {
            timestamp: crate::header_extension::HeTimestamp {
                seconds: 5,
                fraction: 0,
            },
            invalidate: false,
        };
        let (tbody, tflags) = ts.write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::InfoTs,
            &tbody,
            tflags & !FLAG_E_LITTLE_ENDIAN,
        );
        let (d1body, d1flags) = data_msg(1, b"a").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &d1body,
            d1flags & !FLAG_E_LITTLE_ENDIAN,
        );

        let invalidate = InfoTimestampSubmessage {
            timestamp: crate::header_extension::HeTimestamp::default(),
            invalidate: true,
        };
        let (ivbody, ivflags) = invalidate.write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::InfoTs,
            &ivbody,
            ivflags & !FLAG_E_LITTLE_ENDIAN,
        );

        let parsed = decode_datagram(&bytes).unwrap();
        let states: Vec<(bool, &ParsedSubmessage)> = parsed
            .receiver_walk(GuidPrefix::from_bytes([0; 12]))
            .map(|(state, sub)| (state.have_timestamp, sub))
            .collect();
        assert!(matches!(states[1].1, ParsedSubmessage::Data(_)));
        assert!(
            states[1].0,
            "Data after the valid InfoTs must see haveTimestamp=true"
        );
        assert!(matches!(states[2].1, ParsedSubmessage::InfoTimestamp(_)));
        assert!(
            !states[2].0,
            "the invalidating InfoTs itself must already show haveTimestamp=false"
        );
    }

    #[test]
    fn receiver_walk_info_reply_sets_reply_locators() {
        use crate::wire_types::Locator;
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let reply = InfoReplySubmessage {
            unicast_locators: vec![Locator::udp_v4([10, 0, 0, 1], 7411)],
            multicast_locators: Some(vec![Locator::udp_v4([239, 255, 0, 1], 7400)]),
        };
        let (rbody, rflags) = reply.write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::InfoReply,
            &rbody,
            rflags & !FLAG_E_LITTLE_ENDIAN,
        );
        let (dbody, dflags) = data_msg(1, b"x").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &dbody,
            dflags & !FLAG_E_LITTLE_ENDIAN,
        );

        let parsed = decode_datagram(&bytes).unwrap();
        let (state, _) = parsed
            .receiver_walk(GuidPrefix::from_bytes([0; 12]))
            .find(|(_, s)| matches!(s, ParsedSubmessage::Data(_)))
            .unwrap();
        assert_eq!(
            state.unicast_reply_locator_list,
            vec![Locator::udp_v4([10, 0, 0, 1], 7411)]
        );
        assert_eq!(
            state.multicast_reply_locator_list,
            vec![Locator::udp_v4([239, 255, 0, 1], 7400)]
        );
    }

    fn skewed_message() -> ParsedDatagram {
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let he = crate::header_extension::HeaderExtension {
            little_endian: true,
            timestamp: Some(crate::header_extension::HeTimestamp {
                seconds: 1000,
                fraction: 0,
            }),
            ..crate::header_extension::HeaderExtension::default()
        };
        bytes.extend_from_slice(&he.encode().unwrap());
        let ts = InfoTimestampSubmessage {
            timestamp: crate::header_extension::HeTimestamp {
                seconds: 990,
                fraction: 0,
            },
            invalidate: false,
        };
        let (tbody, tflags) = ts.write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::InfoTs,
            &tbody,
            tflags & !FLAG_E_LITTLE_ENDIAN,
        );
        let (dbody, dflags) = data_msg(1, b"x").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &dbody,
            dflags & !FLAG_E_LITTLE_ENDIAN,
        );
        decode_datagram(&bytes).unwrap()
    }

    fn clock_at_1100() -> crate::header_extension::HeTimestamp {
        crate::header_extension::HeTimestamp {
            seconds: 1100,
            fraction: 0,
        }
    }

    /// §8.3.8.11: "The reception of the InfoTimestamp submessage sets the
    /// source timestamp associated with any subsequent Data and DataFrag
    /// submessages." The time of a HeaderExtension is the time the message
    /// was sent (`rtpsSendTimestamp`, §8.3.7.4), not a source timestamp: a
    /// DATA behind it, with no INFO_TS, has none.
    #[test]
    fn a_header_extension_timestamp_is_no_source_timestamp() {
        let he = crate::header_extension::HeaderExtension {
            little_endian: true,
            timestamp: Some(crate::header_extension::HeTimestamp {
                seconds: 1000,
                fraction: 0,
            }),
            ..crate::header_extension::HeaderExtension::default()
        };
        let mut bytes = header().to_bytes().to_vec();
        bytes.extend_from_slice(&he.encode().unwrap());
        let (dbody, dflags) = data_msg(1, b"x").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &dbody,
            dflags & !FLAG_E_LITTLE_ENDIAN,
        );
        let parsed = decode_datagram(&bytes).unwrap();
        let (state, _) = parsed
            .receiver_walk(GuidPrefix::from_bytes([0; 12]))
            .find(|(_, s)| matches!(s, ParsedSubmessage::Data(_)))
            .unwrap();
        assert!(!state.have_timestamp, "no source timestamp");
        assert_eq!(state.rtps_send_timestamp.map(|t| t.seconds), Some(1000));
    }

    /// §8.3.7.4 and §8.3.8.11: with a reception clock and a threshold, the
    /// message with a send time 100 s behind the clock is a skewed one and
    /// its InfoTimestamp is moved by the difference.
    #[test]
    fn the_walk_with_a_reception_clock_detects_a_skew_and_corrects_the_timestamp() {
        let parsed = skewed_message();
        let on = ReceptionClock {
            now: clock_at_1100,
            skew_threshold_seconds: Some(30),
        };
        let (state, _) = parsed
            .receiver_walk(GuidPrefix::from_bytes([0; 12]))
            .with_reception_clock(on)
            .find(|(_, s)| matches!(s, ParsedSubmessage::Data(_)))
            .unwrap();
        assert!(state.clock_skew_detected);
        assert_eq!(state.rtps_reception_timestamp, Some(clock_at_1100()));
        assert_eq!(state.timestamp.seconds, 1090);
        // No threshold: the time of reception is kept, nothing is detected
        // and the timestamp stays as sent.
        let off = ReceptionClock {
            now: clock_at_1100,
            skew_threshold_seconds: None,
        };
        let (state, _) = parsed
            .receiver_walk(GuidPrefix::from_bytes([0; 12]))
            .with_reception_clock(off)
            .find(|(_, s)| matches!(s, ParsedSubmessage::Data(_)))
            .unwrap();
        assert!(!state.clock_skew_detected);
        assert_eq!(state.rtps_reception_timestamp, Some(clock_at_1100()));
        assert_eq!(state.timestamp.seconds, 990);
        // Without a clock the reception time stays TIME_INVALID.
        let (state, _) = parsed
            .receiver_walk(GuidPrefix::from_bytes([0; 12]))
            .find(|(_, s)| matches!(s, ParsedSubmessage::Data(_)))
            .unwrap();
        assert_eq!(state.rtps_reception_timestamp, None);
    }

    /// `receiver_walk_in` gives a path the source and the clock of its
    /// datagram in one argument; the default context knows neither.
    #[test]
    fn receiver_walk_in_applies_the_source_and_the_clock_of_the_context() {
        use crate::wire_types::{Locator, LocatorKind};
        let parsed = skewed_message();
        let mut from = [0u8; 16];
        from[12..].copy_from_slice(&[10, 1, 2, 3]);
        let context = ReceiveContext {
            source: Some(Locator {
                kind: LocatorKind::UdpV4,
                port: 4000,
                address: from,
            }),
            clock: Some(ReceptionClock {
                now: clock_at_1100,
                skew_threshold_seconds: Some(30),
            }),
        };
        let (state, _) = parsed
            .receiver_walk_in(GuidPrefix::from_bytes([0; 12]), &context)
            .find(|(_, s)| matches!(s, ParsedSubmessage::Data(_)))
            .unwrap();
        assert_eq!(state.unicast_reply_locator_list[0].address, from);
        assert_eq!(
            state.unicast_reply_locator_list[0].port,
            Locator::PORT_INVALID
        );
        assert!(state.clock_skew_detected);
        assert_eq!(state.timestamp.seconds, 1090);
        let (state, _) = parsed
            .receiver_walk_in(GuidPrefix::from_bytes([0; 12]), &ReceiveContext::default())
            .find(|(_, s)| matches!(s, ParsedSubmessage::Data(_)))
            .unwrap();
        assert_eq!(state.unicast_reply_locator_list, vec![Locator::INVALID]);
        assert!(!state.clock_skew_detected);
    }

    /// Table 8.17: with the transport known, a submessage before any
    /// InfoReply sees the sender's address and the invalid port.
    #[test]
    fn receiver_walk_from_starts_the_reply_lists_at_the_transport_values() {
        use crate::wire_types::{Locator, LocatorKind};
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let (dbody, dflags) = data_msg(1, b"x").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &dbody,
            dflags & !FLAG_E_LITTLE_ENDIAN,
        );
        let parsed = decode_datagram(&bytes).unwrap();
        let mut from = [0u8; 16];
        from[12..].copy_from_slice(&[192, 168, 1, 9]);
        let (state, _) = parsed
            .receiver_walk_from(
                GuidPrefix::from_bytes([0; 12]),
                LocatorKind::UdpV4,
                Some(from),
            )
            .find(|(_, s)| matches!(s, ParsedSubmessage::Data(_)))
            .unwrap();
        assert_eq!(state.unicast_reply_locator_list.len(), 1);
        assert_eq!(state.unicast_reply_locator_list[0].address, from);
        assert_eq!(
            state.unicast_reply_locator_list[0].port,
            Locator::PORT_INVALID
        );
        assert_eq!(state.multicast_reply_locator_list.len(), 1);
        assert_eq!(
            state.multicast_reply_locator_list[0].address,
            Locator::ADDRESS_INVALID
        );
    }

    #[test]
    fn receiver_walk_info_reply_ip4_sets_reply_locators() {
        use crate::wire_types::Locator;
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let reply = InfoReplyIp4Submessage {
            unicast_locator: Locator::udp_v4([10, 0, 0, 2], 7412),
            multicast_locator: Some(Locator::udp_v4([239, 255, 0, 1], 7400)),
        };
        let (rbody, rflags) = reply.write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::InfoReplyIp4,
            &rbody,
            rflags & !FLAG_E_LITTLE_ENDIAN,
        );
        let (dbody, dflags) = data_msg(1, b"x").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &dbody,
            dflags & !FLAG_E_LITTLE_ENDIAN,
        );

        let parsed = decode_datagram(&bytes).unwrap();
        let (state, _) = parsed
            .receiver_walk(GuidPrefix::from_bytes([0; 12]))
            .find(|(_, s)| matches!(s, ParsedSubmessage::Data(_)))
            .unwrap();
        assert_eq!(
            state.unicast_reply_locator_list,
            vec![Locator::udp_v4([10, 0, 0, 2], 7412)]
        );
        assert_eq!(
            state.multicast_reply_locator_list,
            vec![Locator::udp_v4([239, 255, 0, 1], 7400)]
        );
    }

    #[test]
    fn receiver_walk_yields_every_submessage_including_info_ones() {
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let info = InfoSourceSubmessage {
            unused: 0,
            protocol_version: crate::wire_types::ProtocolVersion::V2_5,
            vendor_id: crate::wire_types::VendorId([1, 1]),
            guid_prefix: GuidPrefix::from_bytes([2; 12]),
        };
        let (ibody, iflags) = info.write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::InfoSrc,
            &ibody,
            iflags & !FLAG_E_LITTLE_ENDIAN,
        );
        let (dbody, dflags) = data_msg(1, b"x").write_body(true);
        push_submessage(
            &mut bytes,
            SubmessageId::Data,
            &dbody,
            dflags & !FLAG_E_LITTLE_ENDIAN,
        );

        let parsed = decode_datagram(&bytes).unwrap();
        let count = parsed
            .receiver_walk(GuidPrefix::from_bytes([0; 12]))
            .count();
        assert_eq!(count, parsed.submessages.len());
    }

    /// The submessages of one of every known kind, encoded little-endian:
    /// `(id, body, flags, flags defined for it)`.
    fn one_of_every_known_submessage() -> Vec<(SubmessageId, Vec<u8>, u8, u8)> {
        use crate::submessages::{
            ACKNACK_FLAG_FINAL, AckNackSubmessage, GAP_FLAG_FILTERED_COUNT, GAP_FLAG_GROUP_INFO,
            GapSubmessage, HEARTBEAT_FLAG_FINAL, HEARTBEAT_FLAG_GROUP_INFO,
            HEARTBEAT_FLAG_LIVELINESS, INFO_REPLY_FLAG_MULTICAST, INFO_TIMESTAMP_FLAG_INVALIDATE,
            InfoReplyIp4Submessage, InfoReplySubmessage, InfoSourceSubmessage,
            InfoTimestampSubmessage, NackFragSubmessage, SequenceNumberSet,
        };
        use crate::wire_types::{FragmentNumber, Locator, ProtocolVersion};
        let reader = EntityId::user_reader_with_key([1, 2, 3]);
        let writer = EntityId::user_writer_with_key([4, 5, 6]);
        let e = FLAG_E_LITTLE_ENDIAN;
        let mut out = Vec::new();
        let (b, f) = HeartbeatSubmessage {
            reader_id: reader,
            writer_id: writer,
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(7),
            count: 42,
            final_flag: true,
            liveliness_flag: true,
            group_info: None,
        }
        .write_body(true);
        out.push((
            SubmessageId::Heartbeat,
            b,
            f,
            e | HEARTBEAT_FLAG_FINAL | HEARTBEAT_FLAG_LIVELINESS | HEARTBEAT_FLAG_GROUP_INFO,
        ));
        let (b, f) = AckNackSubmessage {
            reader_id: reader,
            writer_id: writer,
            reader_sn_state: SequenceNumberSet::from_missing(
                SequenceNumber(3),
                &[SequenceNumber(3), SequenceNumber(5)],
            ),
            count: 9,
            final_flag: true,
        }
        .write_body(true);
        out.push((SubmessageId::AckNack, b, f, e | ACKNACK_FLAG_FINAL));
        let (b, f) = GapSubmessage {
            reader_id: reader,
            writer_id: writer,
            gap_start: SequenceNumber(2),
            gap_list: SequenceNumberSet::from_missing(SequenceNumber(4), &[SequenceNumber(5)]),
            group_info: None,
            filtered_count: Some(1),
        }
        .write_body(true);
        out.push((
            SubmessageId::Gap,
            b,
            f,
            e | GAP_FLAG_GROUP_INFO | GAP_FLAG_FILTERED_COUNT,
        ));
        let (b, f) = HeartbeatFragSubmessage {
            reader_id: reader,
            writer_id: writer,
            writer_sn: SequenceNumber(42),
            last_fragment_num: FragmentNumber(8),
            count: 3,
        }
        .write_body(true);
        out.push((SubmessageId::HeartbeatFrag, b, f, e));
        let (b, f) = NackFragSubmessage {
            reader_id: reader,
            writer_id: writer,
            writer_sn: SequenceNumber(5),
            fragment_number_state: crate::submessages::FragmentNumberSet {
                bitmap_base: FragmentNumber(1),
                num_bits: 4,
                bitmap: vec![0b1010_0000_0000_0000_0000_0000_0000_0000],
            },
            count: 1,
        }
        .write_body(true);
        out.push((SubmessageId::NackFrag, b, f, e));
        let (b, f) = InfoTimestampSubmessage {
            timestamp: crate::header_extension::HeTimestamp {
                seconds: 5,
                fraction: 6,
            },
            invalidate: false,
        }
        .write_body(true);
        out.push((
            SubmessageId::InfoTs,
            b,
            f,
            e | INFO_TIMESTAMP_FLAG_INVALIDATE,
        ));
        let (b, f) = InfoSourceSubmessage {
            unused: 0,
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId([0xAB, 0xCD]),
            guid_prefix: GuidPrefix::from_bytes([3; 12]),
        }
        .write_body(true);
        out.push((SubmessageId::InfoSrc, b, f, e));
        let (b, f) = InfoReplySubmessage {
            unicast_locators: vec![Locator::udp_v4([10, 1, 2, 3], 7411)],
            multicast_locators: Some(vec![Locator::udp_v4([239, 255, 0, 1], 7400)]),
        }
        .write_body(true);
        out.push((SubmessageId::InfoReply, b, f, e | INFO_REPLY_FLAG_MULTICAST));
        let (b, f) = InfoReplyIp4Submessage {
            unicast_locator: Locator::udp_v4([10, 1, 2, 3], 7411),
            multicast_locator: Some(Locator::udp_v4([239, 255, 0, 1], 7400)),
        }
        .write_body(true);
        out.push((
            SubmessageId::InfoReplyIp4,
            b,
            f,
            e | INFO_REPLY_FLAG_MULTICAST,
        ));
        out
    }

    fn message_of(id: SubmessageId, body: &[u8], flags: u8) -> Vec<u8> {
        let mut bytes = header().to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: id,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(body);
        bytes
    }

    /// DDSI-RTPS 2.5 §8.6.1: "Additional fields can be added to the end of
    /// a Submessage that was already defined in the current minor version.
    /// An implementation should skip over additional fields using the
    /// submessageLength field": each known submessage decodes to the same
    /// value with eight further octets at the end of its body.
    #[test]
    fn additional_fields_at_the_end_of_a_known_submessage_are_skipped() {
        for (id, body, flags, _) in one_of_every_known_submessage() {
            let plain = decode_datagram(&message_of(id, &body, flags)).unwrap();
            let mut longer = body.clone();
            longer.extend_from_slice(&[0xEE; 8]);
            let extended = decode_datagram(&message_of(id, &longer, flags)).unwrap();
            assert_eq!(extended.submessages.len(), 1, "{id:?}");
            assert_eq!(
                alloc::format!("{:?}", extended.submessages[0].submessage),
                alloc::format!("{:?}", plain.submessages[0].submessage),
                "{id:?}"
            );
        }
        // INFO_DST: the 12 octets of the prefix, then more.
        let mut dst = vec![7u8; 12];
        dst.extend_from_slice(&[0xEE; 4]);
        let parsed = decode_datagram(&message_of(
            SubmessageId::InfoDst,
            &dst,
            FLAG_E_LITTLE_ENDIAN,
        ))
        .unwrap();
        match &parsed.submessages[0].submessage {
            ParsedSubmessage::InfoDestination(prefix) => assert_eq!(prefix, &[7u8; 12]),
            other => panic!("expected InfoDestination, got {other:?}"),
        }
    }

    /// DDSI-RTPS 2.5 §9.4.5.1.2: the X flags "should be set to zero when
    /// sending and ignored when receiving". Every encoder sets the E flag and
    /// the flags defined for its submessage, no other; every decoder gives the
    /// same value with all the other flag bits set.
    #[test]
    fn x_flags_are_zero_when_sending_and_ignored_when_receiving() {
        for (id, body, flags, defined) in one_of_every_known_submessage() {
            assert_eq!(flags & !defined, 0, "{id:?} sends X flags: {flags:#04x}");
            let plain = decode_datagram(&message_of(id, &body, flags)).unwrap();
            let x_flags = !defined;
            let noisy = decode_datagram(&message_of(id, &body, flags | x_flags)).unwrap();
            assert_eq!(
                alloc::format!("{:?}", noisy.submessages[0].submessage),
                alloc::format!("{:?}", plain.submessages[0].submessage),
                "{id:?} reads the X flags"
            );
        }
        // DATA and DATA_FRAG, which the other submessages above do not cover.
        let data = data_msg(7, b"x-flags");
        let (body, flags) = data.write_body(true);
        assert_eq!(
            flags
                & !(FLAG_E_LITTLE_ENDIAN
                    | crate::submessages::DATA_FLAG_INLINE_QOS
                    | crate::submessages::DATA_FLAG_DATA
                    | crate::submessages::DATA_FLAG_KEY
                    | crate::submessages::DATA_FLAG_NON_STANDARD),
            0
        );
        let plain = decode_datagram(&message_of(SubmessageId::Data, &body, flags)).unwrap();
        let defined = FLAG_E_LITTLE_ENDIAN
            | crate::submessages::DATA_FLAG_INLINE_QOS
            | crate::submessages::DATA_FLAG_DATA
            | crate::submessages::DATA_FLAG_KEY
            | crate::submessages::DATA_FLAG_NON_STANDARD;
        // The flags defined for DATA are all of 0x1F; X is bits 5 to 7.
        let noisy =
            decode_datagram(&message_of(SubmessageId::Data, &body, flags | !defined)).unwrap();
        assert_eq!(
            alloc::format!("{:?}", noisy.submessages[0].submessage),
            alloc::format!("{:?}", plain.submessages[0].submessage),
        );
    }

    /// DDSI-RTPS 2.5 Table 9.20 note 4: PID_DIRECTED_WRITE of a peer of a
    /// protocol version before 2.4 (a sequence<GUID_t> there) is ignored; one
    /// of 2.4 or later (a GUID_t) is kept.
    #[test]
    fn pid_directed_write_of_a_peer_before_2_4_is_ignored() {
        use crate::wire_types::ProtocolVersion;
        let target = crate::wire_types::Guid::new(
            GuidPrefix::from_bytes([9; 12]),
            EntityId::user_reader_with_key([7, 7, 7]),
        );
        let mut d = data_msg(3, b"directed");
        let mut pl = crate::parameter_list::ParameterList::new();
        pl.push(crate::inline_qos::directed_write_param(target.to_bytes()));
        d.inline_qos = Some(pl);
        let (body, flags) = d.write_body(true);
        for (minor, kept) in [(3u8, false), (4, true), (5, true)] {
            let mut h = header();
            h.protocol_version = ProtocolVersion { major: 2, minor };
            let mut bytes = h.to_bytes().to_vec();
            let sh = SubmessageHeader {
                submessage_id: SubmessageId::Data,
                flags,
                octets_to_next_header: body.len() as u16,
            };
            bytes.extend_from_slice(&sh.to_bytes());
            bytes.extend_from_slice(&body);
            let parsed = decode_datagram(&bytes).unwrap();
            let ParsedSubmessage::Data(data) = &parsed.submessages[0].submessage else {
                panic!("expected Data");
            };
            let found = data
                .inline_qos
                .as_ref()
                .and_then(|pl| crate::inline_qos::find_directed_write(pl).unwrap());
            assert_eq!(found.is_some(), kept, "protocol version 2.{minor}");
        }
    }

    /// DDSI-RTPS 2.5 §9.3.2.7: `MESSAGE_LENGTH_INVALID` is 0, and a
    /// HeaderExtension with that `messageLength` is invalid (§8.3.7.3: "The
    /// messageLength is too small to fit the RTPS Header and the
    /// HeaderExtension"); the whole message is refused.
    #[test]
    fn a_header_extension_announcing_message_length_invalid_refuses_the_message() {
        assert_eq!(crate::header_extension::MESSAGE_LENGTH_INVALID, 0);
        let (dbody, dflags) = data_msg(1, b"x").write_body(true);
        let message = |length: u32| {
            let he = crate::header_extension::HeaderExtension {
                little_endian: true,
                message_length: Some(length),
                ..crate::header_extension::HeaderExtension::default()
            };
            let mut bytes = header().to_bytes().to_vec();
            bytes.extend_from_slice(&he.encode().unwrap());
            let sh = SubmessageHeader {
                submessage_id: SubmessageId::Data,
                flags: dflags,
                octets_to_next_header: dbody.len() as u16,
            };
            bytes.extend_from_slice(&sh.to_bytes());
            bytes.extend_from_slice(&dbody);
            bytes
        };
        let probe = message(0);
        let real = u32::try_from(probe.len()).unwrap();
        assert!(
            decode_datagram(&message(crate::header_extension::MESSAGE_LENGTH_INVALID)).is_err()
        );
        assert!(decode_datagram(&message(real)).is_ok());
    }
}
