// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Top-level GIOP message codec — dispatches to all 8 message types.

use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, Writer};

use crate::cancel_request::CancelRequest;
use crate::close_connection::CloseConnection;
use crate::error::{GiopError, GiopResult};
use crate::flags::Flags;
use crate::fragment::Fragment;
use crate::header::{HEADER_SIZE, MessageHeader};
use crate::locate_reply::LocateReply;
use crate::locate_request::LocateRequest;
use crate::message_error::MessageError;
use crate::message_type::MessageType;
use crate::reply::Reply;
use crate::request::Request;
use crate::version::Version;

/// Complete GIOP message — header + typed body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// Request (Spec §15.4.2).
    Request(Request),
    /// Reply (Spec §15.4.3).
    Reply(Reply),
    /// CancelRequest (Spec §15.4.4).
    CancelRequest(CancelRequest),
    /// LocateRequest (Spec §15.4.5).
    LocateRequest(LocateRequest),
    /// LocateReply (Spec §15.4.6).
    LocateReply(LocateReply),
    /// CloseConnection (Spec §15.4.7).
    CloseConnection(CloseConnection),
    /// MessageError (Spec §15.4.8).
    MessageError(MessageError),
    /// Fragment (Spec §15.4.9).
    Fragment(Fragment),
}

impl Message {
    /// Returns the `MessageType` of the variant.
    #[must_use]
    pub const fn message_type(&self) -> MessageType {
        match self {
            Self::Request(_) => MessageType::Request,
            Self::Reply(_) => MessageType::Reply,
            Self::CancelRequest(_) => MessageType::CancelRequest,
            Self::LocateRequest(_) => MessageType::LocateRequest,
            Self::LocateReply(_) => MessageType::LocateReply,
            Self::CloseConnection(_) => MessageType::CloseConnection,
            Self::MessageError(_) => MessageType::MessageError,
            Self::Fragment(_) => MessageType::Fragment,
        }
    }
}

/// Encodes a complete GIOP message including the 12-byte header.
///
/// `endianness` selects the on-wire byte order (bit 0 of the
/// `flags` octet). `more_fragments` sets the fragment bit
/// (spec §15.4.9; only allowed from GIOP 1.1 onward).
///
/// # Errors
/// Buffer error or spec violation (e.g. fragment bit in 1.0).
pub fn encode_message(
    version: Version,
    endianness: Endianness,
    more_fragments: bool,
    msg: &Message,
) -> GiopResult<Vec<u8>> {
    Ok(encode_message_with_padding(version, endianness, more_fragments, msg)?.0)
}

/// A finished GIOP message and the spans of it that are alignment fill.
///
/// See [`encode_message_with_padding`], which is the only thing that
/// produces one.
pub type MessageWithPadding = (Vec<u8>, Vec<(usize, usize)>);

/// [`encode_message`], and where in the result the alignment padding is.
///
/// Each span is `(offset, length)` in the finished message's own
/// coordinates. The octets they cover carry no value: CDR fills an
/// alignment gap and says nothing about what with, and the three ORBs
/// disagree — omniORB 4.3 and JacORB 3.9 write zeros, TAO 2.5 writes
/// whatever its buffer held (measured: `01 00 00 00` before a request
/// body in `tools/wire-capture/frames/tao`).
///
/// So a message compared octet for octet against a capture has to know
/// which octets are allowed to differ. Reading that off the difference
/// itself would let any real divergence be called padding; this reads it
/// off the *layout*, from the one place that emits a fill octet.
///
/// # Errors
/// As [`encode_message`].
pub fn encode_message_with_padding(
    version: Version,
    endianness: Endianness,
    more_fragments: bool,
    msg: &Message,
) -> GiopResult<MessageWithPadding> {
    if more_fragments && !version.supports_fragments() {
        return Err(GiopError::FragmentNotSupported {
            major: version.major,
            minor: version.minor,
        });
    }
    // The body is written into the *message*, behind a reserved header,
    // and not into a buffer of its own. CDR alignment counts from byte 0
    // of the 12-byte header, not from the start of the body: a GIOP 1.2
    // body that aligned to 8 from its own start would land at absolute
    // offset ≡ 4 mod 8, which omniORB and TAO do not read
    // (CORBA §15.4.2 / §15.4.4).
    //
    // This used to be a separate writer carrying an alignment origin of
    // 12 — the same answer reached by telling the buffer a lie about
    // where it began. Writing into the message says it once, and there
    // is then no second place the origin can be forgotten.
    let mut out = Writer::new(endianness).recording_padding();
    for _ in 0..HEADER_SIZE {
        out.write_u8(0);
    }
    encode_body(version, msg, &mut out)?;
    let padding = out.padding_spans().to_vec();
    let mut bytes = out.finish();
    let body_size = u32::try_from(bytes.len() - HEADER_SIZE)
        .map_err(|_| GiopError::Malformed("body exceeds u32".into()))?;

    let mut flags = Flags::from_endianness(endianness);
    flags = flags.with_fragment(more_fragments);
    let header = MessageHeader::new(version, flags, msg.message_type(), body_size);

    let mut header_writer = Writer::new(endianness);
    header.encode(&mut header_writer)?;
    let header_bytes = header_writer.finish();
    debug_assert_eq!(header_bytes.len(), HEADER_SIZE);
    bytes[..HEADER_SIZE].copy_from_slice(&header_bytes);
    Ok((bytes, padding))
}

fn encode_body(version: Version, msg: &Message, w: &mut Writer) -> GiopResult<()> {
    match msg {
        Message::Request(r) => r.encode(version, w),
        Message::Reply(r) => r.encode(version, w),
        Message::CancelRequest(c) => c.encode(w),
        Message::LocateRequest(l) => l.encode(version, w),
        Message::LocateReply(l) => l.encode(version, w),
        Message::CloseConnection(_) | Message::MessageError(_) => Ok(()),
        Message::Fragment(f) => f.encode(version, w),
    }
}

/// Decodes a GIOP message including the header and returns the
/// remaining bytes after the message.
///
/// # Errors
/// Buffer/spec error.
pub fn decode_message(bytes: &[u8]) -> GiopResult<(Message, &[u8])> {
    let (msg, _endianness, rest) = decode_message_ctx(bytes)?;
    Ok((msg, rest))
}

/// Like [`decode_message`], but additionally returns the message's on-wire
/// byte order (from the GIOP header `byte_order` flag, spec §15.4.1).
///
/// Needed to decode the opaque `Request`/`Reply` `body` (CDR-encoded operation
/// args) in the correct order — CDR is "receiver makes it right", i.e. foreign
/// ORBs send in their native order and flag it.
///
/// # Errors
/// Buffer/spec error.
pub fn decode_message_ctx(bytes: &[u8]) -> GiopResult<(Message, zerodds_wire::Endianness, &[u8])> {
    let (header, body) = MessageHeader::decode(bytes)?;
    let endianness = header.endianness();
    let body_size = header.message_size as usize;
    if body.len() < body_size {
        return Err(GiopError::BodyTooLarge {
            body_size,
            message_size_field: header.message_size,
        });
    }
    let end = HEADER_SIZE + body_size;
    // The mirror image of the encode: one reader over the whole message,
    // stepped past the header, so alignment counts from byte 0 of the
    // message the way §15.4 says and the way the encoder wrote it.
    let mut r = Reader::new(&bytes[..end], endianness);
    r.skip_to(HEADER_SIZE)?;
    let msg = decode_body(header, &mut r)?;
    Ok((msg, endianness, &bytes[end..]))
}

fn decode_body(header: MessageHeader, r: &mut Reader<'_>) -> GiopResult<Message> {
    let v = header.version;
    Ok(match header.message_type {
        MessageType::Request => Message::Request(Request::decode(v, r)?),
        MessageType::Reply => Message::Reply(Reply::decode(v, r)?),
        MessageType::CancelRequest => Message::CancelRequest(CancelRequest::decode(r)?),
        MessageType::LocateRequest => Message::LocateRequest(LocateRequest::decode(v, r)?),
        MessageType::LocateReply => Message::LocateReply(LocateReply::decode(v, r)?),
        MessageType::CloseConnection => Message::CloseConnection(CloseConnection),
        MessageType::MessageError => Message::MessageError(MessageError),
        MessageType::Fragment => Message::Fragment(Fragment::decode(v, r)?),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::request::ResponseFlags;
    use crate::service_context::ServiceContextList;
    use crate::target_address::TargetAddress;

    fn sample_request_msg(version: Version) -> Message {
        Message::Request(Request {
            request_id: 7,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0xab, 0xcd]),
            operation: "ping".into(),
            requesting_principal: if version.uses_v1_2_request_layout() {
                None
            } else {
                Some(alloc::vec::Vec::new())
            },
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        })
    }

    #[test]
    fn round_trip_request_giop_1_0_be() {
        let m = sample_request_msg(Version::V1_0);
        let bytes = encode_message(Version::V1_0, Endianness::Big, false, &m).unwrap();
        let (decoded, rest) = decode_message(&bytes).unwrap();
        assert_eq!(decoded, m);
        assert!(rest.is_empty());
    }

    #[test]
    fn round_trip_request_giop_1_2_le() {
        let m = sample_request_msg(Version::V1_2);
        let bytes = encode_message(Version::V1_2, Endianness::Little, false, &m).unwrap();
        let (decoded, rest) = decode_message(&bytes).unwrap();
        assert_eq!(decoded, m);
        assert!(rest.is_empty());
    }

    #[test]
    fn round_trip_close_connection() {
        let m = Message::CloseConnection(CloseConnection);
        let bytes = encode_message(Version::V1_2, Endianness::Big, false, &m).unwrap();
        // Header 12 bytes + 0 body.
        assert_eq!(bytes.len(), 12);
        let (decoded, _) = decode_message(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn round_trip_message_error() {
        let m = Message::MessageError(MessageError);
        let bytes = encode_message(Version::V1_1, Endianness::Little, false, &m).unwrap();
        assert_eq!(bytes.len(), 12);
        let (decoded, _) = decode_message(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn round_trip_fragment_with_more_bit() {
        let m = Message::Fragment(Fragment {
            header: Some(crate::fragment::FragmentHeader { request_id: 3 }),
            body: alloc::vec![0; 32],
        });
        let bytes = encode_message(Version::V1_2, Endianness::Big, true, &m).unwrap();
        // Fragment bit must be set in the header.
        assert_eq!(bytes[6] & Flags::FRAGMENT_BIT, Flags::FRAGMENT_BIT);
        let (decoded, _) = decode_message(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn fragment_bit_in_giop_1_0_is_rejected() {
        let m = Message::Fragment(Fragment {
            header: None,
            body: alloc::vec::Vec::new(),
        });
        let err = encode_message(Version::V1_0, Endianness::Big, true, &m).unwrap_err();
        assert!(matches!(err, GiopError::FragmentNotSupported { .. }));
    }

    /// §15.4, normative from GIOP 1.2: the request body is aligned to
    /// eight **from the start of the message**, header included.
    ///
    /// Measured on the finished message and not on a body buffer of its
    /// own, which is the configuration that ships. A body writer that
    /// counted alignment from its own byte 0 would pass a round-trip
    /// against itself and put the body at absolute ≡ 4 mod 8, where
    /// omniORB and TAO do not read it — a green run over a message no
    /// peer accepts.
    #[test]
    fn giop_1_2_body_is_eight_aligned_from_the_message_start() {
        for endianness in [Endianness::Big, Endianness::Little] {
            let m = Message::Request(Request {
                request_id: 1,
                response_flags: ResponseFlags::SYNC_WITH_TARGET,
                target: TargetAddress::Key(alloc::vec![0xa]),
                operation: "x".into(),
                requesting_principal: None,
                service_context: ServiceContextList::default(),
                body: alloc::vec![0xff, 0xff, 0xff, 0xff],
            });
            let bytes = encode_message(Version::V1_2, endianness, false, &m).unwrap();
            let body_at = bytes.len() - 4;
            assert_eq!(
                body_at % 8,
                0,
                "{endianness:?}: the body starts at {body_at}, which is not 8-aligned"
            );
            // And the padding that got it there is padding, not a value.
            assert!(bytes[body_at..].iter().all(|b| *b == 0xff));
        }
    }

    /// The GIOP 1.2 reply body carries the same rule (§15.4.3.2).
    #[test]
    fn giop_1_2_reply_body_is_eight_aligned_from_the_message_start() {
        let m = Message::Reply(crate::reply::Reply {
            request_id: 3,
            reply_status: crate::reply::ReplyStatusType::NoException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![0x7f, 0x7f],
        });
        let bytes = encode_message(Version::V1_2, Endianness::Big, false, &m).unwrap();
        let body_at = bytes.len() - 2;
        assert_eq!(body_at % 8, 0, "reply body starts at {body_at}");
    }

    /// A GIOP 1.0 body has no such rule: it follows its header directly,
    /// and a codec that aligned it anyway would insert bytes the peer
    /// reads as content.
    #[test]
    fn giop_1_0_body_is_not_padded() {
        let m = Message::Request(Request {
            request_id: 1,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0xa]),
            operation: "x".into(),
            requesting_principal: Some(alloc::vec::Vec::new()),
            service_context: ServiceContextList::default(),
            body: alloc::vec![0xff],
        });
        let bytes = encode_message(Version::V1_0, Endianness::Big, false, &m).unwrap();
        // service_context(4) + request_id(4) + response_expected(1) +
        // pad(3) + key len(4) + key(1) + pad(3) + operation "x"(4+2) +
        // pad(2) + principal len(4) + body(1), all from offset 12.
        assert_eq!(*bytes.last().unwrap(), 0xff);
        assert_eq!(bytes.len(), 12 + 33);
    }

    /// The message the decoder rejects has to be rejected for the reason
    /// the header gives and not read as a shorter one.
    #[test]
    fn message_size_larger_than_the_buffer_is_refused() {
        let m = sample_request_msg(Version::V1_2);
        let mut bytes = encode_message(Version::V1_2, Endianness::Big, false, &m).unwrap();
        let truncated = bytes.len() - 4;
        bytes.truncate(truncated);
        assert!(matches!(
            decode_message(&bytes),
            Err(GiopError::BodyTooLarge { .. })
        ));
    }

    /// Trailing bytes after a complete message come back untouched — a
    /// TCP read hands over whatever arrived, and the next message starts
    /// in it.
    #[test]
    fn bytes_after_the_message_are_returned() {
        let m = sample_request_msg(Version::V1_2);
        let mut bytes = encode_message(Version::V1_2, Endianness::Big, false, &m).unwrap();
        bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        let (decoded, rest) = decode_message(&bytes).unwrap();
        assert_eq!(decoded, m);
        assert_eq!(rest, &[0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn header_message_size_matches_actual_body() {
        let m = sample_request_msg(Version::V1_2);
        let bytes = encode_message(Version::V1_2, Endianness::Big, false, &m).unwrap();
        let (h, body) = MessageHeader::decode(&bytes).unwrap();
        assert_eq!(h.message_size as usize, body.len());
    }

    #[test]
    fn round_trip_cancel_request() {
        let m = Message::CancelRequest(CancelRequest { request_id: 99 });
        let bytes = encode_message(Version::V1_1, Endianness::Big, false, &m).unwrap();
        let (decoded, _) = decode_message(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn round_trip_locate_request_giop_1_2() {
        let m = Message::LocateRequest(LocateRequest {
            request_id: 1,
            target: TargetAddress::Key(alloc::vec![0xa, 0xb]),
        });
        let bytes = encode_message(Version::V1_2, Endianness::Big, false, &m).unwrap();
        let (decoded, _) = decode_message(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn round_trip_locate_reply_object_here() {
        let m = Message::LocateReply(LocateReply {
            request_id: 1,
            locate_status: crate::locate_reply::LocateStatusType::ObjectHere,
            body: alloc::vec::Vec::new(),
        });
        let bytes = encode_message(Version::V1_0, Endianness::Big, false, &m).unwrap();
        let (decoded, _) = decode_message(&bytes).unwrap();
        assert_eq!(decoded, m);
    }
}
