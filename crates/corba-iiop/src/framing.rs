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

use zerodds_corba_giop::ziop::{ZiopCodec, ZiopSend, compress_message, decompress_message};
use zerodds_corba_giop::{
    GiopError, Magic, Message, MessageHeader, Version, ZIOP_MAGIC_BYTES, decode_message_ctx,
    encode_message,
};

use crate::error::IiopError;
use zerodds_wire::Endianness;

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
    read_giop_message_ziop(r, None)
}

/// Like [`read_giop_message_full`], and a ZIOP message (CORBA 3.3 Part 2
/// §12.1, fragments included) is decompressed with `ziop` into the GIOP
/// message it carries. Without `ziop` a ZIOP message is refused with
/// `InvalidMagic("ZIOP")`: this side did not declare ZIOP, so no peer may
/// send it one (§12.2).
///
/// # Errors
/// `Closed` on a clean connection end; otherwise `Io`/`Giop`.
pub fn read_giop_message_ziop<R: Read + ?Sized>(
    r: &mut R,
    ziop: Option<&dyn ZiopCodec>,
) -> Result<(Message, Endianness, Version), IiopError> {
    let (header, body) = read_frame(r, ziop)?;
    let version = header.version;

    // GIOP 1.2 fragmentation (§15.4.9): if the first message has the FRAGMENT flag,
    // its body is only the first fragment. Follow-up `Fragment` messages (msgtype 7)
    // carry a request_id header (4 bytes, GIOP 1.2) + continuation data;
    // reassembly happens at the raw-byte level (the partial CDR is not decodable on
    // its own) until a fragment without the FRAGMENT flag arrives.
    if header.flags.has_more_fragments() {
        let (msg, endianness) = reassemble_fragments(r, &header, body, ziop)?;
        return Ok((msg, endianness, version));
    }

    // Full message bytes = header + body.
    let mut hw = zerodds_wire::Writer::new(header.endianness());
    header.encode(&mut hw)?;
    let mut full = hw.finish();
    full.extend_from_slice(&body);
    let (msg, endianness, _) = decode_message_ctx(&full)?;
    Ok((msg, endianness, version))
}

/// Reads one message off the stream — its header and `message_size` body
/// octets — and, for a ZIOP message, the GIOP header and body it carries.
fn read_frame<R: Read + ?Sized>(
    r: &mut R,
    ziop: Option<&dyn ZiopCodec>,
) -> Result<(MessageHeader, alloc::vec::Vec<u8>), IiopError> {
    let mut header_bytes = [0u8; 12];
    read_exact_or_closed(r, &mut header_bytes)?;
    let (magic, header, _rest) = MessageHeader::decode_frame(&header_bytes)?;
    let body_size = header.message_size as usize;
    if body_size > MAX_GIOP_MESSAGE_SIZE {
        return Err(IiopError::Giop(GiopError::Malformed(alloc::format!(
            "GIOP message_size {body_size} exceeds MAX_GIOP_MESSAGE_SIZE ({MAX_GIOP_MESSAGE_SIZE})"
        ))));
    }
    let mut body = alloc::vec![0u8; body_size];
    r.read_exact(&mut body)?;
    match magic {
        Magic::Giop => Ok((header, body)),
        Magic::Ziop => {
            let Some(codec) = ziop else {
                return Err(IiopError::Giop(GiopError::InvalidMagic(ZIOP_MAGIC_BYTES)));
            };
            let mut frame = header_bytes.to_vec();
            frame.extend_from_slice(&body);
            let mut giop = decompress_message(&frame, codec, MAX_GIOP_MESSAGE_SIZE)?;
            let (giop_header, _) = MessageHeader::decode(&giop)?;
            giop.drain(..12);
            Ok((giop_header, giop))
        }
    }
}

/// Reassembles a fragmented GIOP 1.2 message (§15.4.9). `first_body` is the body
/// of the first (FRAGMENT-flagged) message; follow-up fragments are read and
/// their continuation data (without the 4-byte request_id header) is appended
/// until a fragment without the FRAGMENT flag arrives. Then the complete message
/// (original header with total size + cleared FRAGMENT flag + body) is decoded.
/// Each fragment may arrive as a ZIOP message of its own.
fn reassemble_fragments<R: Read + ?Sized>(
    r: &mut R,
    first_header: &zerodds_corba_giop::MessageHeader,
    first_body: alloc::vec::Vec<u8>,
    ziop: Option<&dyn ZiopCodec>,
) -> Result<(Message, Endianness), IiopError> {
    const FRAG_HEADER_LEN: usize = 4; // GIOP 1.2: request_id (uint32)
    let mut acc = first_body;
    loop {
        let (frag_header, fbody) = read_frame(r, ziop)?;
        let fsize = fbody.len();
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
    let mut hw = zerodds_wire::Writer::new(first_header.endianness());
    reasm_header.encode(&mut hw)?;
    let mut full = hw.finish();
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
    for frame in encode_giop_frames(version, endianness, msg, Some(max_fragment_payload))? {
        w.write_all(&frame)?;
    }
    w.flush()?;
    Ok(())
}

/// Writes a GIOP message as ZIOP messages where `send` allows it (CORBA 3.3
/// Part 2 §12.1): the message is fragmented as in
/// [`write_giop_message_fragmented`] when `max_fragment_payload` is given,
/// and each message on the wire — the whole message or each fragment — is
/// compressed with the compressor of `send` unless its low value or minimum
/// ratio keeps it a GIOP message
/// ([`zerodds_corba_giop::ziop::compress_message`]). Returns how many of
/// the written messages are ZIOP messages.
///
/// # Errors
/// Encode/I/O error; `Giop(Malformed)` when the compressor fails.
pub fn write_giop_message_ziop<W: Write + ?Sized>(
    w: &mut W,
    version: Version,
    endianness: Endianness,
    msg: &Message,
    max_fragment_payload: Option<usize>,
    send: &ZiopSend,
    codec: &dyn ZiopCodec,
) -> Result<usize, IiopError> {
    let mut compressed = 0;
    for frame in encode_giop_frames(version, endianness, msg, max_fragment_payload)? {
        match compress_message(&frame, send, codec)? {
            Some(z) => {
                compressed += 1;
                w.write_all(&z)?;
            }
            None => w.write_all(&frame)?,
        }
    }
    w.flush()?;
    Ok(compressed)
}

/// The GIOP messages that carry `msg`: one, or the fragments of
/// [`write_giop_message_fragmented`] when `max_fragment_payload` is given
/// and the message is fragmentable and longer.
fn encode_giop_frames(
    version: Version,
    endianness: Endianness,
    msg: &Message,
    max_fragment_payload: Option<usize>,
) -> Result<alloc::vec::Vec<alloc::vec::Vec<u8>>, IiopError> {
    let full = encode_message(version, endianness, false, msg)?;
    let Some(max_fragment_payload) = max_fragment_payload else {
        return Ok(alloc::vec![full]);
    };
    let body = &full[12..];
    // 8-aligned fragment size (except the last); round the threshold down to a multiple of 8.
    let frag = max_fragment_payload & !7;
    let req_id = message_request_id(msg);

    // No splitting: too small, not fragmentable, not 1.2, or threshold < 8.
    let Some(req_id) = req_id else {
        return Ok(alloc::vec![full]);
    };
    if frag == 0 || !version.supports_fragments() || body.len() <= frag {
        return Ok(alloc::vec![full]);
    }

    let header = &full[..12];
    let (header_struct, _) = zerodds_corba_giop::MessageHeader::decode(header)?;
    let msg_type = header_struct.message_type;

    let mut frames = alloc::vec::Vec::new();
    let mut off = 0usize;
    let mut first = true;
    while off < body.len() {
        let remaining = body.len() - off;
        let is_last = remaining <= frag;
        let chunk_len = if is_last { remaining } else { frag };
        let chunk = &body[off..off + chunk_len];
        let mut out = alloc::vec::Vec::new();

        if first {
            // First frame: original type + FRAGMENT flag (if more follows).
            let h = zerodds_corba_giop::MessageHeader::new(
                version,
                zerodds_corba_giop::Flags::from_endianness(endianness).with_fragment(!is_last),
                msg_type,
                u32::try_from(chunk_len).map_err(|_| overflow())?,
            );
            write_header(&mut out, &h, endianness)?;
            out.extend_from_slice(chunk);
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
            write_header(&mut out, &h, endianness)?;
            let mut idw = zerodds_wire::Writer::new(endianness);
            idw.write_u32(req_id);
            out.extend_from_slice(&idw.finish());
            out.extend_from_slice(chunk);
        }
        frames.push(out);
        off += chunk_len;
    }
    Ok(frames)
}

fn write_header<W: Write + ?Sized>(
    w: &mut W,
    h: &zerodds_corba_giop::MessageHeader,
    endianness: Endianness,
) -> Result<(), IiopError> {
    let mut hw = zerodds_wire::Writer::new(endianness);
    h.encode(&mut hw)?;
    w.write_all(&hw.finish())?;
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
        let mut w = zerodds_wire::Writer::new(endian);
        header.encode(&mut w).unwrap();
        buf.extend_from_slice(&w.finish());
        buf.extend_from_slice(body);
    }

    /// Fragment continuation body: request_id(4 bytes, GIOP 1.2) + data.
    fn frag_body(endian: Endianness, data: &[u8]) -> alloc::vec::Vec<u8> {
        let mut fb = alloc::vec::Vec::new();
        let mut idw = zerodds_wire::Writer::new(endian);
        idw.write_u32(7); // == sample_request().request_id
        fb.extend_from_slice(&idw.finish());
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
        let mut w = zerodds_wire::Writer::new(Endianness::Big);
        header.encode(&mut w).unwrap();
        // No body bytes follow — read_giop_message_full must reject
        // before attempting to read/allocate `message_size` bytes.
        let mut r = Cursor::new(w.finish());
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
        let mut fw = zerodds_wire::Writer::new(Endianness::Big);
        frag_header.encode(&mut fw).unwrap();
        wire.extend_from_slice(&fw.finish());

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

    /// A run-length "compressor" under zlib's id for the framing tests:
    /// (count, octet) pairs. The zlib of CORBA 3.3 Part 1 §18 is tested in
    /// zerodds-corba-ccm and through the ORB in zerodds-corba-interop.
    struct Rle;

    impl ZiopCodec for Rle {
        fn supports(&self, compressor: u16) -> bool {
            compressor == 4
        }
        fn compress(
            &self,
            _: zerodds_corba_giop::ziop::CompressorIdLevel,
            body: &[u8],
        ) -> Result<alloc::vec::Vec<u8>, zerodds_corba_giop::ziop::ZiopError> {
            let mut out = alloc::vec::Vec::new();
            let mut i = 0;
            while i < body.len() {
                let b = body[i];
                let mut n = 1usize;
                while i + n < body.len() && body[i + n] == b && n < 255 {
                    n += 1;
                }
                out.push(n as u8);
                out.push(b);
                i += n;
            }
            Ok(out)
        }
        fn decompress(
            &self,
            _: u16,
            data: &[u8],
            _: u32,
        ) -> Result<alloc::vec::Vec<u8>, zerodds_corba_giop::ziop::ZiopError> {
            let mut out = alloc::vec::Vec::new();
            for pair in data.chunks(2) {
                out.extend(core::iter::repeat_n(pair[1], pair[0] as usize));
            }
            Ok(out)
        }
    }

    fn zeros_request(payload: usize) -> Message {
        Message::Request(Request {
            request_id: 42,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0x01, 0x02, 0x03, 0x04]),
            operation: "big_op".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![0; payload],
        })
    }

    fn zlib_send() -> ZiopSend {
        ZiopSend {
            compressor: zerodds_corba_giop::ziop::DEFAULT_COMPRESSOR,
            low_value: 0,
            min_ratio: None,
        }
    }

    /// The magics of the messages in `wire`, in order.
    fn magics(wire: &[u8]) -> alloc::vec::Vec<[u8; 4]> {
        let mut out = alloc::vec::Vec::new();
        let mut off = 0;
        while off < wire.len() {
            let (_, h, _) = MessageHeader::decode_frame(&wire[off..]).unwrap();
            out.push([wire[off], wire[off + 1], wire[off + 2], wire[off + 3]]);
            off += 12 + h.message_size as usize;
        }
        out
    }

    #[test]
    fn corba_p2_12_1_a_ziop_request_is_read_back_as_its_giop_message() {
        for endian in [Endianness::Big, Endianness::Little] {
            let msg = zeros_request(4096);
            let mut wire = alloc::vec::Vec::new();
            let n = write_giop_message_ziop(
                &mut wire,
                Version::V1_2,
                endian,
                &msg,
                None,
                &zlib_send(),
                &Rle,
            )
            .unwrap();
            assert_eq!(n, 1);
            assert_eq!(magics(&wire), alloc::vec![*b"ZIOP"]);
            assert!(wire.len() < 200, "{} octets", wire.len());
            let (decoded, e, v) =
                read_giop_message_ziop(&mut Cursor::new(&wire), Some(&Rle)).unwrap();
            assert_eq!(decoded, msg);
            assert_eq!(e, endian);
            assert_eq!(v, Version::V1_2);
        }
    }

    #[test]
    fn corba_p2_12_1_each_fragment_is_a_ziop_message_of_its_own() {
        let msg = zeros_request(4096);
        let mut wire = alloc::vec::Vec::new();
        let n = write_giop_message_ziop(
            &mut wire,
            Version::V1_2,
            Endianness::Little,
            &msg,
            Some(1024),
            &zlib_send(),
            &Rle,
        )
        .unwrap();
        let m = magics(&wire);
        assert!(m.len() > 3, "{} messages", m.len());
        assert_eq!(n, m.len());
        assert!(m.iter().all(|x| x == b"ZIOP"));
        let (decoded, _, _) = read_giop_message_ziop(&mut Cursor::new(&wire), Some(&Rle)).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn corba_p2_12_1_a_fragment_the_low_value_keeps_giop_is_mixed_with_ziop_ones() {
        // 4096 octets in fragments of 1024: the last fragment is short.
        let msg = zeros_request(4096);
        let send = ZiopSend {
            low_value: 1000,
            ..zlib_send()
        };
        let mut wire = alloc::vec::Vec::new();
        let n = write_giop_message_ziop(
            &mut wire,
            Version::V1_2,
            Endianness::Big,
            &msg,
            Some(1024),
            &send,
            &Rle,
        )
        .unwrap();
        let m = magics(&wire);
        assert_eq!(m.last(), Some(b"GIOP"));
        assert_eq!(n, m.iter().filter(|x| *x == b"ZIOP").count());
        assert!(n >= 1);
        let (decoded, _, _) = read_giop_message_ziop(&mut Cursor::new(&wire), Some(&Rle)).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn corba_p2_12_2_a_ziop_message_without_a_codec_is_refused() {
        let mut wire = alloc::vec::Vec::new();
        write_giop_message_ziop(
            &mut wire,
            Version::V1_2,
            Endianness::Big,
            &zeros_request(4096),
            None,
            &zlib_send(),
            &Rle,
        )
        .unwrap();
        let err = read_giop_message_ziop(&mut Cursor::new(&wire), None).unwrap_err();
        assert!(
            matches!(err, IiopError::Giop(GiopError::InvalidMagic(m)) if m == *b"ZIOP"),
            "{err:?}"
        );
        // GIOP 1.1 is never compressed.
        let mut v11 = alloc::vec::Vec::new();
        let n = write_giop_message_ziop(
            &mut v11,
            Version::V1_1,
            Endianness::Big,
            &zeros_request(4096),
            None,
            &zlib_send(),
            &Rle,
        )
        .unwrap();
        assert_eq!(n, 0);
        assert_eq!(magics(&v11), alloc::vec![*b"GIOP"]);
    }
}
