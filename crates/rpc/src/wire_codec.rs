// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Wire codec for request and reply samples of the Basic Service Mapping.
//!
//! The Basic mapping's topic types are `${I}_Request { RequestHeader
//! header; ${I}_Call data; }` and `${I}_Reply { ReplyHeader header;
//! ${I}_Return data; }` (§7.5.1.1.6, §7.5.1.1.7). A sample is the CDR
//! encoding of that structure:
//!
//! ```text
//! REQUEST-Frame:
//!   RequestHeader (XCDR2-LE)  ||  pad to 4  ||  Call union
//! REPLY-Frame:
//!   ReplyHeader (XCDR2-LE)    ||  Return union
//! ```
//!
//! The `RequestHeader` ends with a variable-length string, so the `long`
//! discriminator of the union that follows starts at an offset the header
//! decides; XCDR2 aligns it to 4 octets from the start of the sample and
//! the `ReplyHeader` (28 octets) needs no padding. Every member of the
//! union is then aligned relative to the start of the sample as it is
//! relative to the start of the union, because XCDR2 aligns to 4 at most.
//!
//! Decoders must know the header format on both sides, because the DCPS
//! DataReader delivers the sample buffer as a single `Vec<u8>`.

extern crate alloc;

use alloc::vec::Vec;

use crate::common_types::{ReplyHeader, RequestHeader};
use crate::error::{RpcError, RpcResult};

/// Encodes a request frame: `RequestHeader` (XCDR2-LE), the padding that
/// aligns the `Call` union to 4 octets, and the XCDR2-encoded `Call`
/// union (§7.5.1.1.6 rule 2).
///
/// `user_payload` is the output of `T::encode`, the `Call` union starting
/// at its own offset 0, and is written after the padding.
///
/// # Errors
/// `RpcError::Codec` if `instance_name` exceeds `string<255>`
/// (Spec §7.5.1.1.1).
pub fn encode_request_frame(header: &RequestHeader, user_payload: &[u8]) -> RpcResult<Vec<u8>> {
    let mut out = header.to_cdr_le()?;
    out.resize(out.len().next_multiple_of(UNION_ALIGNMENT), 0);
    out.extend_from_slice(user_payload);
    Ok(out)
}

/// The alignment of the `long` discriminator that opens a union under
/// XCDR2 (XTypes 1.3 §7.4.1.2: at most 4 octets).
const UNION_ALIGNMENT: usize = 4;

/// What the `data` member of a reply carries when the reply reports a
/// remote exception: the `Return` union's `default` case (§7.5.1.1.7 rule
/// 3) — a `long` discriminator no operation hashes to and one octet of
/// `unknownOp`. "The remote exceptions shall not be reported back using
/// this member": `ReplyHeader.remoteEx` carries the code, and the union
/// has to be there because it is a member of the structure.
pub const ERROR_REPLY_DATA: [u8; 5] = [0, 0, 0, 0, 0];

/// Splits a request frame into `(RequestHeader, &user-payload)`.
///
/// # Errors
/// `RpcError::Codec` if the header does not parse or the payload
/// is truncated.
pub fn decode_request_frame(bytes: &[u8]) -> RpcResult<(RequestHeader, &[u8])> {
    let header = RequestHeader::from_cdr_le(bytes)?;
    let consumed = encoded_request_header_len(&header).next_multiple_of(UNION_ALIGNMENT);
    if consumed > bytes.len() {
        return Err(RpcError::codec("request frame truncated"));
    }
    Ok((header, &bytes[consumed..]))
}

/// Encodes a reply frame.
#[must_use]
pub fn encode_reply_frame(header: &ReplyHeader, user_payload: &[u8]) -> Vec<u8> {
    let mut out = header.to_cdr_le();
    out.extend_from_slice(user_payload);
    out
}

/// Splits a reply frame into `(ReplyHeader, &user-payload)`.
///
/// # Errors
/// `RpcError::Codec` if the header does not parse or the payload
/// is truncated.
pub fn decode_reply_frame(bytes: &[u8]) -> RpcResult<(ReplyHeader, &[u8])> {
    let header = ReplyHeader::from_cdr_le(bytes)?;
    let consumed = encoded_reply_header_len();
    if consumed > bytes.len() {
        return Err(RpcError::codec("reply frame truncated"));
    }
    Ok((header, &bytes[consumed..]))
}

fn encoded_request_header_len(header: &RequestHeader) -> usize {
    // SampleIdentity (24) + uint32 length + UTF-8 bytes + NUL. The
    // identity is 4-byte aligned, so no padding precedes the length.
    24 + 4 + header.instance_name.len() + 1
}

fn encoded_reply_header_len() -> usize {
    // ReplyHeader is always 28 bytes (16 GUID + 8 SN + 4 ex-code).
    28
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::common_types::{RemoteExceptionCode, SampleIdentity};

    fn id() -> SampleIdentity {
        SampleIdentity::new([0x42; 16], 7)
    }

    #[test]
    fn request_frame_roundtrip_empty_payload() {
        let h = RequestHeader::new(id(), "");
        let frame = encode_request_frame(&h, &[]).unwrap();
        let (back, payload) = decode_request_frame(&frame).unwrap();
        assert_eq!(back, h);
        assert!(payload.is_empty());
    }

    #[test]
    fn request_frame_roundtrip_with_payload() {
        let h = RequestHeader::new(id(), "calc-A");
        let frame = encode_request_frame(&h, &[1, 2, 3, 4]).unwrap();
        let (back, payload) = decode_request_frame(&frame).unwrap();
        assert_eq!(back, h);
        assert_eq!(payload, &[1, 2, 3, 4]);
    }

    /// §7.5.1.1.6 rule 2: `data` follows `header` in the structure, and the
    /// union's `long` discriminator is aligned to 4. For the instance name
    /// `calc-A` (6 characters) the header is 24 + 4 + 7 = 35 octets, so one
    /// octet of padding precedes the union.
    #[test]
    fn the_call_union_starts_on_a_4_octet_boundary() {
        for (name, header_len, pad) in
            [("", 29, 3), ("a", 30, 2), ("calc-A", 35, 1), ("abc", 32, 0)]
        {
            let h = RequestHeader::new(id(), name);
            let frame = encode_request_frame(&h, &[0xAA; 4]).unwrap();
            assert_eq!(frame.len(), header_len + pad + 4, "{name:?}");
            assert!(frame[header_len..header_len + pad].iter().all(|b| *b == 0));
            assert_eq!(frame[header_len + pad..], [0xAA; 4]);
            assert_eq!((frame.len() - 4) % 4, 0, "{name:?}");
        }
    }

    #[test]
    fn a_request_frame_cut_inside_the_padding_is_an_error() {
        let h = RequestHeader::new(id(), "");
        let frame = encode_request_frame(&h, &[]).unwrap();
        assert!(decode_request_frame(&frame[..frame.len() - 1]).is_err());
    }

    /// §7.5.1.1.7 rule 3: an error reply still carries the `Return`
    /// union, in its `default` case.
    #[test]
    fn the_error_reply_data_is_the_default_case_of_the_union() {
        assert_eq!(ERROR_REPLY_DATA.len(), 4 + 1, "long discriminator + octet");
    }

    #[test]
    fn reply_frame_roundtrip() {
        let h = ReplyHeader::new(id(), RemoteExceptionCode::Ok);
        let frame = encode_reply_frame(&h, &[9, 8, 7]);
        let (back, payload) = decode_reply_frame(&frame).unwrap();
        assert_eq!(back, h);
        assert_eq!(payload, &[9, 8, 7]);
    }

    #[test]
    fn reply_frame_carries_exception_code() {
        let h = ReplyHeader::new(id(), RemoteExceptionCode::InvalidArgument);
        let frame = encode_reply_frame(&h, &[]);
        let (back, _) = decode_reply_frame(&frame).unwrap();
        assert_eq!(back.remote_ex, RemoteExceptionCode::InvalidArgument);
    }

    #[test]
    fn decode_request_frame_truncated_header_is_error() {
        let bytes = [0u8; 8];
        assert!(decode_request_frame(&bytes).is_err());
    }

    #[test]
    fn decode_reply_frame_truncated_is_error() {
        let bytes = [0u8; 4];
        assert!(decode_reply_frame(&bytes).is_err());
    }

    #[test]
    fn reply_header_consumed_28_bytes() {
        let h = ReplyHeader::new(SampleIdentity::UNKNOWN, RemoteExceptionCode::Ok);
        let bytes = h.to_cdr_le();
        assert_eq!(bytes.len(), 28);
    }
}
