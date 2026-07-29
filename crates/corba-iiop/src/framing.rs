// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP message framing over TCP — Spec §15.7.1.
//!
//! Spec §15.7.1 normative: "GIOP messages are sent over the TCP/IP
//! transport unmodified". A frame is therefore the complete
//! 12-byte GIOP header plus `header.message_size` body bytes —
//! directly back-to-back without a delimiter.
//!
//! Receive algorithm:
//! 1. Read exactly 12 header bytes.
//! 2. Parse the header (magic, version, flags, type, size).
//! 3. Read exactly `message_size` body bytes.
//! 4. Return (header, body) — the caller dispatches into the codec.

use std::io::{Read, Write};

use zerodds_corba_giop::{GiopError, Message, Version, decode_message_ctx, encode_message};

use crate::error::IiopError;
use zerodds_cdr::Endianness;

/// Hard cap on a single GIOP message body read off the wire (16 MiB).
///
/// GIOP over TCP (§15.7.1) is a stream protocol: the peer-announced
/// `message_size` (a `u32`, up to ~4 GiB) cannot be checked against
/// "bytes remaining" the way an in-memory buffer decode can — there is
/// no bound until this cap is applied. Mirrors the established
/// `crates/transport-tcp/src/framing.rs::MAX_FRAME_SIZE` DoS-cap
/// pattern: reject before allocating, so a peer cannot force a
/// multi-GB allocation from a 12-byte header.
const MAX_GIOP_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Reads **one** complete GIOP message from the stream. Returns
/// `IiopError::Closed` if the peer sends EOF before 12 header bytes
/// have been read.
///
/// # Errors
/// `Closed` on a clean connection end; otherwise `Io`/`Giop`.
pub fn read_giop_message<R: Read + ?Sized>(r: &mut R) -> Result<Message, IiopError> {
    Ok(read_giop_message_ctx(r)?.0)
}

/// Like [`read_giop_message`], but additionally returns the on-wire byte order
/// (GIOP `byte_order` flag) so the caller can decode the opaque Request/Reply
/// body in the correct order.
///
/// # Errors
/// `Closed` on a clean connection end; otherwise `Io`/`Giop`.
pub fn read_giop_message_ctx<R: Read + ?Sized>(
    r: &mut R,
) -> Result<(Message, Endianness), IiopError> {
    let (msg, endianness, _version) = read_giop_message_full(r)?;
    Ok((msg, endianness))
}

/// Like [`read_giop_message_ctx`], but additionally returns the **GIOP version**
/// of the received message (from the message header). Needed so the server
/// replies in the request's version (§15.4.1: never higher than the request) and
/// the client honors the target IOR profile version.
///
/// # Errors
/// `Closed` on a clean connection end; otherwise `Io`/`Giop`.
pub fn read_giop_message_full<R: Read + ?Sized>(
    r: &mut R,
) -> Result<(Message, Endianness, Version), IiopError> {
    let mut header_bytes = [0u8; 12];
    read_exact_or_closed(r, &mut header_bytes)?;
    // Parse the header, then read body_size from the header.
    let (header, _rest) = zerodds_corba_giop::MessageHeader::decode(&header_bytes)?;
    let version = header.version;
    let body_size = header.message_size as usize;
    if body_size > MAX_GIOP_MESSAGE_SIZE {
        return Err(IiopError::Giop(GiopError::Malformed(alloc::format!(
            "GIOP message_size {body_size} exceeds MAX_GIOP_MESSAGE_SIZE ({MAX_GIOP_MESSAGE_SIZE})"
        ))));
    }
    let mut body = alloc::vec![0u8; body_size];
    r.read_exact(&mut body)?;

    // GIOP 1.2 fragmentation (§15.4.9): if the first message has the FRAGMENT flag,
    // its body is only the first fragment. Follow-up `Fragment` messages (msgtype 7)
    // carry a request_id header (4 bytes, GIOP 1.2) + continuation data;
    // reassembly happens at the raw-byte level (the partial CDR is not decodable on
    // its own) until a fragment without the FRAGMENT flag arrives.
    if header.flags.has_more_fragments() {
        let (msg, endianness) = reassemble_fragments(r, &header, body)?;
        return Ok((msg, endianness, version));
    }

    // Full message bytes = header + body.
    let mut full = alloc::vec::Vec::with_capacity(12 + body_size);
    full.extend_from_slice(&header_bytes);
    full.extend_from_slice(&body);
    let (msg, endianness, _) = decode_message_ctx(&full)?;
    Ok((msg, endianness, version))
}

/// Reassembles a fragmented GIOP 1.2 message (§15.4.9). `first_body` is the body
/// of the first (FRAGMENT-flagged) message; follow-up fragments are read and
/// their continuation data (without the 4-byte request_id header) is appended
/// until a fragment without the FRAGMENT flag arrives. Then the complete message
/// (original header with total size + cleared FRAGMENT flag + body) is decoded.
fn reassemble_fragments<R: Read + ?Sized>(
    r: &mut R,
    first_header: &zerodds_corba_giop::MessageHeader,
    first_body: alloc::vec::Vec<u8>,
) -> Result<(Message, Endianness), IiopError> {
    const FRAG_HEADER_LEN: usize = 4; // GIOP 1.2: request_id (uint32)
    let mut acc = first_body;
    loop {
        let mut fh = [0u8; 12];
        read_exact_or_closed(r, &mut fh)?;
        let (frag_header, _) = zerodds_corba_giop::MessageHeader::decode(&fh)?;
        let fsize = frag_header.message_size as usize;
        if fsize > MAX_GIOP_MESSAGE_SIZE {
            return Err(IiopError::Giop(GiopError::Malformed(alloc::format!(
                "GIOP fragment message_size {fsize} exceeds MAX_GIOP_MESSAGE_SIZE ({MAX_GIOP_MESSAGE_SIZE})"
            ))));
        }
        let mut fbody = alloc::vec![0u8; fsize];
        r.read_exact(&mut fbody)?;
        if fsize < FRAG_HEADER_LEN {
            return Err(IiopError::Giop(GiopError::Malformed(
                "fragment body shorter than request_id header".into(),
            )));
        }
        // Continuation data = fragment body without the request_id prefix.
        acc.extend_from_slice(&fbody[FRAG_HEADER_LEN..]);
        // Per-fragment size is capped above, but a peer could otherwise
        // keep the FRAGMENT flag set forever and grow `acc` unboundedly
        // across many fragments — cap the *reassembled total* too.
        if acc.len() > MAX_GIOP_MESSAGE_SIZE {
            return Err(IiopError::Giop(GiopError::Malformed(alloc::format!(
                "reassembled GIOP message exceeds MAX_GIOP_MESSAGE_SIZE ({MAX_GIOP_MESSAGE_SIZE})"
            ))));
        }
        if !frag_header.flags.has_more_fragments() {
            break;
        }
    }
    // Reconstruct the complete message: original header, but with total body
    // size and the FRAGMENT flag cleared.
    let total = u32::try_from(acc.len()).map_err(|_| {
        IiopError::Giop(GiopError::Malformed("reassembled body exceeds u32".into()))
    })?;
    let reasm_header = zerodds_corba_giop::MessageHeader::new(
        first_header.version,
        first_header.flags.with_fragment(false),
        first_header.message_type,
        total,
    );
    let mut hw = zerodds_cdr::BufferWriter::new(first_header.endianness());
    reasm_header.encode(&mut hw)?;
    let mut full = hw.into_bytes();
    full.extend_from_slice(&acc);
    let (msg, endianness, _) = decode_message_ctx(&full)?;
    Ok((msg, endianness))
}

/// Writes a GIOP message to the stream.
///
/// `version` + `endianness` + `more_fragments` are passed through to
/// [`encode_message`]; see there.
///
/// # Errors
/// Buffer or I/O error.
pub fn write_giop_message<W: Write + ?Sized>(
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

/// Extracts the `request_id` from a fragmentable message (Request/Reply).
/// Follow-up fragment messages (GIOP 1.2) carry this ID in the 4-byte header.
fn message_request_id(msg: &Message) -> Option<u32> {
    match msg {
        Message::Request(r) => Some(r.request_id),
        Message::Reply(r) => Some(r.request_id),
        _ => None,
    }
}

/// Writes a GIOP message and **fragments** it (§15.4.9) if its body exceeds
/// `max_fragment_payload`: first message (Request/Reply, FRAGMENT flag set,
/// partial body) + follow-up `Fragment` messages (msgtype 7, each with a
/// request_id header (4 bytes) + continuation), the last fragment without the
/// FRAGMENT flag. Fragments other than the last are multiples of 8 bytes (spec
/// requirement).
///
/// Non-fragmentable messages (CancelRequest/Locate*/Close/MessageError),
/// GIOP < 1.2 or bodies ≤ the threshold go out unchanged via
/// [`write_giop_message`]. Counterpart: [`read_giop_message_ctx`] reassembles.
///
/// # Errors
/// Encode/I/O error; `Giop(Malformed)` if the threshold is < 8.
pub fn write_giop_message_fragmented<W: Write + ?Sized>(
    w: &mut W,
    version: Version,
    endianness: Endianness,
    msg: &Message,
    max_fragment_payload: usize,
) -> Result<(), IiopError> {
    let full = encode_message(version, endianness, false, msg)?;
    let body = &full[12..];
    // 8-aligned fragment size (except the last); round the threshold down to a multiple of 8.
    let frag = max_fragment_payload & !7;
    let req_id = message_request_id(msg);

    // No splitting: too small, not fragmentable, not 1.2, or threshold < 8.
    if frag == 0 || req_id.is_none() || !version.supports_fragments() || body.len() <= frag {
        w.write_all(&full)?;
        w.flush()?;
        return Ok(());
    }
    let req_id = req_id.unwrap_or(0);

    let header = &full[..12];
    let (header_struct, _) = zerodds_corba_giop::MessageHeader::decode(header)?;
    let msg_type = header_struct.message_type;

    let mut off = 0usize;
    let mut first = true;
    while off < body.len() {
        let remaining = body.len() - off;
        let is_last = remaining <= frag;
        let chunk_len = if is_last { remaining } else { frag };
        let chunk = &body[off..off + chunk_len];

        if first {
            // First frame: original type + FRAGMENT flag (if more follows).
            let h = zerodds_corba_giop::MessageHeader::new(
                version,
                zerodds_corba_giop::Flags::from_endianness(endianness).with_fragment(!is_last),
                msg_type,
                u32::try_from(chunk_len).map_err(|_| overflow())?,
            );
            write_header(w, &h, endianness)?;
            w.write_all(chunk)?;
            first = false;
        } else {
            // Follow-up fragment: msgtype Fragment, body = request_id(4) + data.
            let payload_len = 4 + chunk_len;
            let h = zerodds_corba_giop::MessageHeader::new(
                version,
                zerodds_corba_giop::Flags::from_endianness(endianness).with_fragment(!is_last),
                zerodds_corba_giop::MessageType::Fragment,
                u32::try_from(payload_len).map_err(|_| overflow())?,
            );
            write_header(w, &h, endianness)?;
            let mut idw = zerodds_cdr::BufferWriter::new(endianness);
            idw.write_u32(req_id).map_err(|_| overflow())?;
            w.write_all(&idw.into_bytes())?;
            w.write_all(chunk)?;
        }
        off += chunk_len;
    }
    w.flush()?;
    Ok(())
}

fn write_header<W: Write + ?Sized>(
    w: &mut W,
    h: &zerodds_corba_giop::MessageHeader,
    endianness: Endianness,
) -> Result<(), IiopError> {
    let mut hw = zerodds_cdr::BufferWriter::new(endianness);
    h.encode(&mut hw)?;
    w.write_all(&hw.into_bytes())?;
    Ok(())
}

fn overflow() -> IiopError {
    IiopError::Giop(GiopError::Malformed("fragment size exceeds u32".into()))
}

fn read_exact_or_closed<R: Read + ?Sized>(r: &mut R, buf: &mut [u8]) -> Result<(), IiopError> {
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
        // 3 bytes < 12 header bytes -> not Closed (filled>0), but
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

    /// Builds a GIOP 1.2 message frame (12-byte header + body) onto the buffer.
    fn push_frame(
        buf: &mut alloc::vec::Vec<u8>,
        endian: Endianness,
        more: bool,
        mtype: zerodds_corba_giop::MessageType,
        body: &[u8],
    ) {
        let header = zerodds_corba_giop::MessageHeader::new(
            Version::V1_2,
            zerodds_corba_giop::Flags::from_endianness(endian).with_fragment(more),
            mtype,
            body.len() as u32,
        );
        let mut w = zerodds_cdr::BufferWriter::new(endian);
        header.encode(&mut w).unwrap();
        buf.extend_from_slice(&w.into_bytes());
        buf.extend_from_slice(body);
    }

    /// Fragment continuation body: request_id(4 bytes, GIOP 1.2) + data.
    fn frag_body(endian: Endianness, data: &[u8]) -> alloc::vec::Vec<u8> {
        let mut fb = alloc::vec::Vec::new();
        let mut idw = zerodds_cdr::BufferWriter::new(endian);
        idw.write_u32(7).unwrap(); // == sample_request().request_id
        fb.extend_from_slice(&idw.into_bytes());
        fb.extend_from_slice(data);
        fb
    }

    /// Fragmented request sequence (§15.4.9): a canonically encoded message is
    /// split at the 8-byte boundary into a first fragment + follow-up `Fragment`;
    /// the reassembler must return the original.
    fn assert_fragment_roundtrip(endian: Endianness) {
        let mut canonical = alloc::vec::Vec::new();
        write_giop_message(
            &mut canonical,
            Version::V1_2,
            endian,
            false,
            &sample_request(),
        )
        .unwrap();
        let full_body = &canonical[12..];
        assert!(full_body.len() > 8, "body too small to fragment");
        let (first, rest) = full_body.split_at(8); // 8-aligned cut (≠ last fragment)

        let mut wire = alloc::vec::Vec::new();
        push_frame(
            &mut wire,
            endian,
            true,
            zerodds_corba_giop::MessageType::Request,
            first,
        );
        push_frame(
            &mut wire,
            endian,
            false,
            zerodds_corba_giop::MessageType::Fragment,
            &frag_body(endian, rest),
        );

        let mut r = Cursor::new(&wire);
        let decoded = read_giop_message(&mut r).unwrap();
        assert_eq!(
            decoded,
            sample_request(),
            "reassembly mismatch for {endian:?}"
        );
        assert!(matches!(
            read_giop_message(&mut r).unwrap_err(),
            IiopError::Closed
        ));
    }

    fn big_request(payload: usize) -> Message {
        Message::Request(Request {
            request_id: 42,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0x01, 0x02, 0x03, 0x04]),
            operation: "big_op".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: (0..payload).map(|i| (i % 251) as u8).collect(),
        })
    }

    /// Outbound fragmentation → reassembly round-trip: a large request is written
    /// in many 8-aligned fragments and reconstructed by the reader.
    fn assert_outbound_fragment_roundtrip(endian: Endianness, payload: usize, frag: usize) {
        let msg = big_request(payload);
        let mut wire = alloc::vec::Vec::new();
        write_giop_message_fragmented(&mut wire, Version::V1_2, endian, &msg, frag).unwrap();
        let mut r = Cursor::new(&wire);
        let decoded = read_giop_message(&mut r).unwrap();
        assert_eq!(
            decoded, msg,
            "reassembled mismatch ({endian:?}, payload={payload}, frag={frag})"
        );
        // Stream fully consumed.
        assert!(matches!(
            read_giop_message(&mut r).unwrap_err(),
            IiopError::Closed
        ));
    }

    #[test]
    fn outbound_fragmentation_be_le_many_fragments() {
        // 4 kB payload in 64-byte fragments → many fragments, both orders.
        assert_outbound_fragment_roundtrip(Endianness::Big, 4096, 64);
        assert_outbound_fragment_roundtrip(Endianness::Little, 4096, 64);
        // Payload exactly on the fragment boundary + 1 byte (remainder fragment).
        assert_outbound_fragment_roundtrip(Endianness::Big, 129, 64);
    }

    #[test]
    fn outbound_no_fragmentation_when_small_or_unfragmentable() {
        // Body ≤ threshold → one frame.
        let small = big_request(8);
        let mut wire = alloc::vec::Vec::new();
        write_giop_message_fragmented(&mut wire, Version::V1_2, Endianness::Big, &small, 4096)
            .unwrap();
        let mut r = Cursor::new(&wire);
        assert_eq!(read_giop_message(&mut r).unwrap(), small);
        assert!(matches!(
            read_giop_message(&mut r).unwrap_err(),
            IiopError::Closed
        ));
        // CancelRequest is not fragmentable → unchanged, even with a tiny threshold.
        let cancel = Message::CancelRequest(CancelRequest { request_id: 9 });
        let mut wire2 = alloc::vec::Vec::new();
        write_giop_message_fragmented(&mut wire2, Version::V1_2, Endianness::Big, &cancel, 8)
            .unwrap();
        let mut r2 = Cursor::new(&wire2);
        assert!(matches!(
            read_giop_message(&mut r2).unwrap(),
            Message::CancelRequest(_)
        ));
    }

    #[test]
    fn fragment_reassembly_big_endian() {
        assert_fragment_roundtrip(Endianness::Big);
    }

    #[test]
    fn fragment_reassembly_little_endian() {
        assert_fragment_roundtrip(Endianness::Little);
    }

    #[test]
    fn fragment_reassembly_three_fragments() {
        let endian = Endianness::Big;
        let mut canonical = alloc::vec::Vec::new();
        write_giop_message(
            &mut canonical,
            Version::V1_2,
            endian,
            false,
            &sample_request(),
        )
        .unwrap();
        let full_body = &canonical[12..];
        assert!(
            full_body.len() >= 16,
            "need ≥16 body bytes, got {}",
            full_body.len()
        );

        let mut wire = alloc::vec::Vec::new();
        push_frame(
            &mut wire,
            endian,
            true,
            zerodds_corba_giop::MessageType::Request,
            &full_body[..8],
        );
        push_frame(
            &mut wire,
            endian,
            true,
            zerodds_corba_giop::MessageType::Fragment,
            &frag_body(endian, &full_body[8..16]),
        );
        push_frame(
            &mut wire,
            endian,
            false,
            zerodds_corba_giop::MessageType::Fragment,
            &frag_body(endian, &full_body[16..]),
        );

        let mut r = Cursor::new(&wire);
        let decoded = read_giop_message(&mut r).unwrap();
        assert_eq!(decoded, sample_request());
    }

    // -------------------------------------------------------------
    // Buffer-cap hardening — a peer-announced `message_size` (or
    // fragment `message_size`) that exceeds MAX_GIOP_MESSAGE_SIZE must
    // be rejected cleanly (no multi-GB allocation attempt) right after
    // the 12-byte header is parsed, before the body is read/allocated.
    // Mirrors the established
    // `crates/transport-tcp/src/framing.rs::MAX_FRAME_SIZE` guard.
    // -------------------------------------------------------------

    #[test]
    fn oversized_message_size_is_rejected_cleanly() {
        let header = zerodds_corba_giop::MessageHeader::new(
            Version::V1_2,
            zerodds_corba_giop::Flags::from_endianness(Endianness::Big),
            zerodds_corba_giop::MessageType::Request,
            u32::MAX, // far beyond MAX_GIOP_MESSAGE_SIZE
        );
        let mut w = zerodds_cdr::BufferWriter::new(Endianness::Big);
        header.encode(&mut w).unwrap();
        // No body bytes follow — read_giop_message_full must reject
        // before attempting to read/allocate `message_size` bytes.
        let mut r = Cursor::new(w.into_bytes());
        let err = read_giop_message(&mut r).unwrap_err();
        assert!(
            matches!(err, IiopError::Giop(GiopError::Malformed(_))),
            "expected clean Malformed rejection, got {err:?}"
        );
    }

    #[test]
    fn oversized_fragment_message_size_is_rejected_cleanly() {
        let mut wire = alloc::vec::Vec::new();
        // First fragment: small, valid, FRAGMENT flag set.
        push_frame(
            &mut wire,
            Endianness::Big,
            true,
            zerodds_corba_giop::MessageType::Request,
            &[0, 0, 0, 0, 0, 0, 0, 8],
        );
        // Follow-up fragment header announces a huge message_size, with
        // no body bytes following it.
        let frag_header = zerodds_corba_giop::MessageHeader::new(
            Version::V1_2,
            zerodds_corba_giop::Flags::from_endianness(Endianness::Big).with_fragment(false),
            zerodds_corba_giop::MessageType::Fragment,
            u32::MAX,
        );
        let mut fw = zerodds_cdr::BufferWriter::new(Endianness::Big);
        frag_header.encode(&mut fw).unwrap();
        wire.extend_from_slice(&fw.into_bytes());

        let mut r = Cursor::new(&wire);
        let err = read_giop_message(&mut r).unwrap_err();
        assert!(
            matches!(err, IiopError::Giop(GiopError::Malformed(_))),
            "expected clean Malformed rejection, got {err:?}"
        );
    }

    #[test]
    fn within_bound_messages_still_round_trip() {
        // Regression guard: the new cap must not reject ordinary
        // messages (well below MAX_GIOP_MESSAGE_SIZE).
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
        assert_eq!(read_giop_message(&mut r).unwrap(), sample_request());
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
