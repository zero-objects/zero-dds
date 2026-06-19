// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Wire codec for request and reply samples (foundation C6.1.C).
//!
//! At this stage requester/replier transport their samples over
//! generic `RawBytes` topics — the DDS-RPC wire frame is therefore
//! visible on the application side:
//!
//! ```text
//! REQUEST-Frame:
//!   RequestHeader (XCDR2-LE)  ||  user-payload-bytes
//! REPLY-Frame:
//!   ReplyHeader (XCDR2-LE)    ||  user-payload-bytes
//! ```
//!
//! Decoders must know the header format on both sides, because the DCPS
//! DataReader delivers the sample buffer as a single `Vec<u8>`. The wire frame
//! is compatible with the XCDR2 encodings from [`crate::common_types`].

extern crate alloc;

use alloc::vec::Vec;

use crate::common_types::{ReplyHeader, RequestHeader};
use crate::error::{RpcError, RpcResult};

/// Encodes a request frame: `RequestHeader` (XCDR2-LE) followed by the
/// XCDR2-encoded user payload bytes.
///
/// `user_payload` is the output of `T::encode` and is written **byte-identically**
/// after the header — no additional padding.
#[must_use]
pub fn encode_request_frame(header: &RequestHeader, user_payload: &[u8]) -> Vec<u8> {
    let mut out = header.to_cdr_le();
    out.extend_from_slice(user_payload);
    out
}

/// Splits a request frame into `(RequestHeader, &user-payload)`.
///
/// # Errors
/// `RpcError::Codec` if the header does not parse or the payload
/// is truncated.
pub fn decode_request_frame(bytes: &[u8]) -> RpcResult<(RequestHeader, &[u8])> {
    let header = RequestHeader::from_cdr_le(bytes)?;
    let consumed = encoded_request_header_len(&header);
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
    // We compute the length via re-encode — cheap (24-30 bytes typical),
    // robust against padding changes in the encoder.
    header.to_cdr_le().len()
}

fn encoded_reply_header_len() -> usize {
    // ReplyHeader ist immer 28 byte (16 GUID + 8 SN + 4 ex-code).
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
        let frame = encode_request_frame(&h, &[]);
        let (back, payload) = decode_request_frame(&frame).unwrap();
        assert_eq!(back, h);
        assert!(payload.is_empty());
    }

    #[test]
    fn request_frame_roundtrip_with_payload() {
        let h = RequestHeader::new(id(), "calc-A");
        let frame = encode_request_frame(&h, &[1, 2, 3, 4]);
        let (back, payload) = decode_request_frame(&frame).unwrap();
        assert_eq!(back, h);
        assert_eq!(payload, &[1, 2, 3, 4]);
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
