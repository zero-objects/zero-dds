// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Top-Level GIOP-Message-Codec — dispatcht auf alle 8 Message-Types.

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

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

/// Vollstaendige GIOP-Message — Header + typisierter Body.
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
    /// Liefert das `MessageType` der Variante.
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

/// Encodiert eine vollstaendige GIOP-Message inkl. 12-Byte-Header.
///
/// `endianness` waehlt die On-Wire-Byte-Order (Bit 0 des
/// `flags`-Octets). `more_fragments` setzt das Fragment-Bit
/// (Spec §15.4.9; nur ab GIOP 1.1 erlaubt).
///
/// # Errors
/// Buffer-Fehler oder Spec-Verletzung (z.B. Fragment-Bit in 1.0).
pub fn encode_message(
    version: Version,
    endianness: Endianness,
    more_fragments: bool,
    msg: &Message,
) -> GiopResult<Vec<u8>> {
    if more_fragments && !version.supports_fragments() {
        return Err(GiopError::FragmentNotSupported {
            major: version.major,
            minor: version.minor,
        });
    }
    // Body separat encodieren, dann Header mit echter Body-Groesse.
    let mut body_writer = BufferWriter::new(endianness);
    encode_body(version, msg, &mut body_writer)?;
    let body = body_writer.into_bytes();
    let body_size =
        u32::try_from(body.len()).map_err(|_| GiopError::Malformed("body exceeds u32".into()))?;

    let mut flags = Flags::from_endianness(endianness);
    flags = flags.with_fragment(more_fragments);
    let header = MessageHeader::new(version, flags, msg.message_type(), body_size);

    let mut out = BufferWriter::with_capacity(endianness, HEADER_SIZE + body.len());
    header.encode(&mut out)?;
    out.write_bytes(&body)?;
    Ok(out.into_bytes())
}

fn encode_body(version: Version, msg: &Message, w: &mut BufferWriter) -> GiopResult<()> {
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

/// Decodiert eine GIOP-Message inkl. Header und gibt die
/// verbleibenden Bytes nach der Message zurueck.
///
/// # Errors
/// Buffer-/Spec-Fehler.
pub fn decode_message(bytes: &[u8]) -> GiopResult<(Message, &[u8])> {
    let (header, body) = MessageHeader::decode(bytes)?;
    let body_size = header.message_size as usize;
    if body.len() < body_size {
        return Err(GiopError::BodyTooLarge {
            body_size,
            message_size_field: header.message_size,
        });
    }
    let body_slice = &body[..body_size];
    let mut r = BufferReader::new(body_slice, header.endianness());
    let msg = decode_body(header, &mut r)?;
    Ok((msg, &body[body_size..]))
}

fn decode_body(header: MessageHeader, r: &mut BufferReader<'_>) -> GiopResult<Message> {
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
        // Header 12 Bytes + 0 Body.
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
        // Fragment-Bit muss im Header gesetzt sein.
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
