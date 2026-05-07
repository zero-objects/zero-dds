// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP-Message-Framing ueber TCP — Spec §15.7.1.
//!
//! Spec §15.7.1 normativ: "GIOP messages are sent over the TCP/IP
//! transport unmodified". Ein Frame ist also der vollstaendige
//! 12-Byte-GIOP-Header plus `header.message_size` Body-Bytes —
//! direkt back-to-back ohne Trennzeichen.
//!
//! Empfangs-Algorithmus:
//! 1. Lese exakt 12 Header-Bytes.
//! 2. Parse Header (Magic, Version, Flags, Type, Size).
//! 3. Lese genau `message_size` Body-Bytes.
//! 4. Gib (Header, Body) zurueck — Caller dispatcht in Codec.

use std::io::{Read, Write};

use zerodds_corba_giop::{GiopError, Message, Version, decode_message, encode_message};

use crate::error::IiopError;
use zerodds_cdr::Endianness;

/// Liest **eine** vollstaendige GIOP-Message vom Stream. Gibt
/// `IiopError::Closed` zurueck, wenn der Peer EOF sendet bevor 12
/// Header-Bytes gelesen wurden.
///
/// # Errors
/// `Closed` bei sauberem Connection-Ende; sonst `Io`/`Giop`.
pub fn read_giop_message<R: Read>(r: &mut R) -> Result<Message, IiopError> {
    let mut header_bytes = [0u8; 12];
    read_exact_or_closed(r, &mut header_bytes)?;
    // Header parsen, dann body_size aus dem Header lesen.
    let (header, _rest) = zerodds_corba_giop::MessageHeader::decode(&header_bytes)?;
    let body_size = header.message_size as usize;
    let mut body = alloc::vec![0u8; body_size];
    r.read_exact(&mut body)?;
    // Volle Message-Bytes = header + body.
    let mut full = alloc::vec::Vec::with_capacity(12 + body_size);
    full.extend_from_slice(&header_bytes);
    full.extend_from_slice(&body);
    let (msg, _) = decode_message(&full)?;
    Ok(msg)
}

/// Schreibt eine GIOP-Message auf den Stream.
///
/// `version` + `endianness` + `more_fragments` werden an
/// [`encode_message`] durchgereicht; siehe dort.
///
/// # Errors
/// Buffer- oder IO-Fehler.
pub fn write_giop_message<W: Write>(
    w: &mut W,
    version: Version,
    endianness: Endianness,
    more_fragments: bool,
    msg: &Message,
) -> Result<(), IiopError> {
    let bytes = encode_message(version, endianness, more_fragments, msg)?;
    w.write_all(&bytes)?;
    w.flush()?;
    Ok(())
}

fn read_exact_or_closed<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<(), IiopError> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => {
                if filled == 0 {
                    return Err(IiopError::Closed);
                }
                return Err(IiopError::Giop(GiopError::Malformed(alloc::format!(
                    "truncated header: only {filled} of {} bytes available",
                    buf.len()
                ))));
            }
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(IiopError::Io(e)),
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zerodds_corba_giop::{
        CancelRequest, CloseConnection, MessageError, Reply, ReplyStatusType, Request,
        ResponseFlags, ServiceContextList, TargetAddress, Version,
    };

    fn sample_request() -> Message {
        Message::Request(Request {
            request_id: 7,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0xab, 0xcd]),
            operation: "ping".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        })
    }

    #[test]
    fn round_trip_request_through_cursor() {
        let mut buf = alloc::vec::Vec::new();
        write_giop_message(
            &mut buf,
            Version::V1_2,
            Endianness::Big,
            false,
            &sample_request(),
        )
        .unwrap();
        let mut r = Cursor::new(&buf);
        let decoded = read_giop_message(&mut r).unwrap();
        assert_eq!(decoded, sample_request());
    }

    #[test]
    fn empty_stream_yields_closed() {
        let mut r = Cursor::new(alloc::vec::Vec::<u8>::new());
        let err = read_giop_message(&mut r).unwrap_err();
        assert!(matches!(err, IiopError::Closed));
    }

    #[test]
    fn truncated_header_is_diagnostic() {
        let mut r = Cursor::new(alloc::vec![b'G', b'I', b'O']);
        let err = read_giop_message(&mut r).unwrap_err();
        // 3 Bytes < 12 Header-Bytes -> kein Closed (filled>0), sondern
        // Giop-Malformed.
        assert!(matches!(err, IiopError::Giop(_)));
    }

    #[test]
    fn round_trip_reply_le() {
        let m = Message::Reply(Reply {
            request_id: 1,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![0xff],
        });
        let mut buf = alloc::vec::Vec::new();
        write_giop_message(&mut buf, Version::V1_2, Endianness::Little, false, &m).unwrap();
        let mut r = Cursor::new(&buf);
        let decoded = read_giop_message(&mut r).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn read_back_to_back_messages() {
        let mut buf = alloc::vec::Vec::new();
        write_giop_message(
            &mut buf,
            Version::V1_2,
            Endianness::Big,
            false,
            &Message::CancelRequest(CancelRequest { request_id: 1 }),
        )
        .unwrap();
        write_giop_message(
            &mut buf,
            Version::V1_2,
            Endianness::Big,
            false,
            &Message::CloseConnection(CloseConnection),
        )
        .unwrap();
        write_giop_message(
            &mut buf,
            Version::V1_2,
            Endianness::Big,
            false,
            &Message::MessageError(MessageError),
        )
        .unwrap();

        let mut r = Cursor::new(&buf);
        let m1 = read_giop_message(&mut r).unwrap();
        let m2 = read_giop_message(&mut r).unwrap();
        let m3 = read_giop_message(&mut r).unwrap();
        assert!(matches!(m1, Message::CancelRequest(_)));
        assert!(matches!(m2, Message::CloseConnection(_)));
        assert!(matches!(m3, Message::MessageError(_)));
        assert!(matches!(
            read_giop_message(&mut r).unwrap_err(),
            IiopError::Closed
        ));
    }
}
