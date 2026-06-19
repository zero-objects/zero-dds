// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Custom-Metadata encoding per the gRPC spec.
//!
//! Spec: `protocol-http2.md` + `protocol-web.md` —
//! "Custom-Metadata is an arbitrary set of key-value pairs defined
//! by the application layer. [...] Header names that end with `-bin`
//! are interpreted as binary and base64-encoded values, otherwise
//! header values are ASCII."
//!
//! Plus the gRPC-Web specifics for `application/grpc-web-text`:
//! the entire LPM body is base64-encoded.

use alloc::string::String;
use alloc::vec::Vec;

/// Suffix for binary headers per the gRPC spec.
pub const BIN_SUFFIX: &str = "-bin";

/// Standard Content-Type header values.
pub mod content_types {
    /// Mandatory for gRPC-over-HTTP/2.
    pub const GRPC: &str = "application/grpc";
    /// gRPC-over-HTTP/2 with a sub-format.
    pub const GRPC_PROTO: &str = "application/grpc+proto";
    /// gRPC-Web binary format (browser-compatible).
    pub const GRPC_WEB: &str = "application/grpc-web";
    /// gRPC-Web base64 text format (browser-compatible without
    /// HTTP trailers).
    pub const GRPC_WEB_TEXT: &str = "application/grpc-web-text";
}

/// Standard required request headers (HTTP/2 pseudo-headers + gRPC).
pub mod request_headers {
    /// HTTP/2 pseudo-header `:method` — gRPC requires POST.
    pub const METHOD: &str = ":method";
    /// HTTP/2 pseudo-header `:scheme` — http or https.
    pub const SCHEME: &str = ":scheme";
    /// HTTP/2 pseudo-header `:path` — `/<service>/<method>`.
    pub const PATH: &str = ":path";
    /// HTTP/2 pseudo-header `:authority` — server hostname.
    pub const AUTHORITY: &str = ":authority";
    /// `te: trailers` — gRPC requires a trailer-support signal.
    pub const TE: &str = "te";
    /// `content-type` — see [`super::content_types`].
    pub const CONTENT_TYPE: &str = "content-type";
    /// `grpc-encoding` — compression (e.g. "identity", "gzip").
    pub const GRPC_ENCODING: &str = "grpc-encoding";
    /// `grpc-accept-encoding` — list of accepted compressions.
    pub const GRPC_ACCEPT_ENCODING: &str = "grpc-accept-encoding";
    /// `user-agent` — recommended.
    pub const USER_AGENT: &str = "user-agent";
    /// `grpc-timeout` — see the `timeout` module.
    pub const GRPC_TIMEOUT: &str = "grpc-timeout";
    /// `grpc-message-type` — fully-qualified name of the request type.
    pub const GRPC_MESSAGE_TYPE: &str = "grpc-message-type";
}

/// Standard required response headers + trailers.
pub mod response_headers {
    /// HTTP/2 pseudo-header `:status` — HTTP status.
    pub const STATUS: &str = ":status";
    /// `content-type` (see request).
    pub const CONTENT_TYPE: &str = "content-type";
    /// `grpc-encoding`.
    pub const GRPC_ENCODING: &str = "grpc-encoding";
    /// Trailer: `grpc-status` (numeric, 0..=16).
    pub const GRPC_STATUS: &str = "grpc-status";
    /// Trailer: `grpc-message` (percent-encoded).
    pub const GRPC_MESSAGE: &str = "grpc-message";
}

// ---------------------------------------------------------------------------
// -bin header classification
// ---------------------------------------------------------------------------

/// `true` if the header name carries the `-bin` suffix.
#[must_use]
pub fn is_binary_header(name: &str) -> bool {
    name.len() > BIN_SUFFIX.len() && name.ends_with(BIN_SUFFIX)
}

// ---------------------------------------------------------------------------
// Base64 for binary headers + grpc-web-text
// ---------------------------------------------------------------------------

const B64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encoding of a binary header value per RFC 4648 §4 (with padding).
#[must_use]
pub fn encode_base64(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= input.len() {
        let b0 = input[i];
        let b1 = input[i + 1];
        let b2 = input[i + 2];
        out.push(B64_TABLE[(b0 >> 2) as usize] as char);
        out.push(B64_TABLE[(((b0 & 0b11) << 4) | (b1 >> 4)) as usize] as char);
        out.push(B64_TABLE[(((b1 & 0b1111) << 2) | (b2 >> 6)) as usize] as char);
        out.push(B64_TABLE[(b2 & 0b111111) as usize] as char);
        i += 3;
    }
    let rest = input.len() - i;
    if rest == 1 {
        let b0 = input[i];
        out.push(B64_TABLE[(b0 >> 2) as usize] as char);
        out.push(B64_TABLE[((b0 & 0b11) << 4) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let b0 = input[i];
        let b1 = input[i + 1];
        out.push(B64_TABLE[(b0 >> 2) as usize] as char);
        out.push(B64_TABLE[(((b0 & 0b11) << 4) | (b1 >> 4)) as usize] as char);
        out.push(B64_TABLE[((b1 & 0b1111) << 2) as usize] as char);
        out.push('=');
    }
    out
}

/// Decoding of a base64-encoded binary header value.
///
/// # Errors
/// `MetadataError::InvalidBase64` on non-base64 characters or
/// malformed padding.
pub fn decode_base64(input: &str) -> Result<Vec<u8>, MetadataError> {
    let bytes = input.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err(MetadataError::InvalidBase64);
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let mut buf = [0u8; 4];
    let mut buf_len;
    let mut i = 0;
    while i < bytes.len() {
        buf_len = 0;
        let mut padding = 0;
        for j in 0..4 {
            let c = bytes[i + j];
            if c == b'=' {
                padding += 1;
                buf[j] = 0;
            } else {
                buf[j] = decode_b64_char(c).ok_or(MetadataError::InvalidBase64)?;
                buf_len += 1;
            }
        }
        if padding > 0 && i + 4 != bytes.len() {
            return Err(MetadataError::InvalidBase64);
        }
        out.push((buf[0] << 2) | (buf[1] >> 4));
        if buf_len > 2 {
            out.push((buf[1] << 4) | (buf[2] >> 2));
        }
        if buf_len > 3 {
            out.push((buf[2] << 6) | buf[3]);
        }
        i += 4;
    }
    Ok(out)
}

fn decode_b64_char(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Metadata encoding error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataError {
    /// Base64 input is invalid (padding, characters, length).
    InvalidBase64,
    /// Header name carries no `-bin` suffix, but the value is non-ASCII.
    NonAsciiInTextHeader,
}

impl core::fmt::Display for MetadataError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidBase64 => write!(f, "InvalidBase64"),
            Self::NonAsciiInTextHeader => write!(f, "NonAsciiInTextHeader"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MetadataError {}

// ---------------------------------------------------------------------------
// High-Level Helpers
// ---------------------------------------------------------------------------

/// Encodes a header value according to the header name:
/// `-bin` suffix → base64; otherwise ASCII (plain).
///
/// # Errors
/// `MetadataError::NonAsciiInTextHeader` if the header has *no*
/// `-bin` suffix but `value` contains non-ASCII bytes.
pub fn encode_header_value(name: &str, value: &[u8]) -> Result<String, MetadataError> {
    if is_binary_header(name) {
        Ok(encode_base64(value))
    } else if value.iter().all(|b| b.is_ascii() && *b != 0) {
        Ok(String::from_utf8(value.to_vec()).map_err(|_| MetadataError::NonAsciiInTextHeader)?)
    } else {
        Err(MetadataError::NonAsciiInTextHeader)
    }
}

/// Decodes a header value into raw bytes according to the header name.
///
/// # Errors
/// `MetadataError::InvalidBase64` on base64 errors.
pub fn decode_header_value(name: &str, encoded: &str) -> Result<Vec<u8>, MetadataError> {
    if is_binary_header(name) {
        decode_base64(encoded)
    } else {
        Ok(encoded.as_bytes().to_vec())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn is_binary_header_recognizes_bin_suffix() {
        assert!(is_binary_header("trace-bin"));
        assert!(is_binary_header("custom-meta-bin"));
    }

    #[test]
    fn is_binary_header_rejects_text_headers() {
        assert!(!is_binary_header("custom-key"));
        assert!(!is_binary_header(""));
        assert!(!is_binary_header("-bin"));
    }

    #[test]
    fn encode_base64_empty() {
        assert_eq!(encode_base64(b""), "");
    }

    #[test]
    fn encode_base64_one_byte_pads_two() {
        assert_eq!(encode_base64(b"f"), "Zg==");
    }

    #[test]
    fn encode_base64_two_bytes_pads_one() {
        assert_eq!(encode_base64(b"fo"), "Zm8=");
    }

    #[test]
    fn encode_base64_three_bytes_no_padding() {
        assert_eq!(encode_base64(b"foo"), "Zm9v");
    }

    #[test]
    fn encode_base64_known_vector() {
        // RFC 4648 Test-Vector
        assert_eq!(encode_base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn decode_base64_round_trip() {
        for input in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"] {
            let encoded = encode_base64(input);
            let decoded = decode_base64(&encoded).expect("decode");
            assert_eq!(decoded, input);
        }
    }

    #[test]
    fn decode_base64_rejects_invalid_chars() {
        assert!(decode_base64("Zm**").is_err());
    }

    #[test]
    fn decode_base64_rejects_bad_padding_length() {
        assert!(decode_base64("Zm9").is_err());
    }

    #[test]
    fn encode_header_value_text_passes_ascii() {
        let v = encode_header_value("custom-key", b"hello").expect("ok");
        assert_eq!(v, "hello");
    }

    #[test]
    fn encode_header_value_text_rejects_non_ascii() {
        // 0xff is invalid ASCII
        let r = encode_header_value("custom-key", &[0xff]);
        assert!(r.is_err());
    }

    #[test]
    fn encode_header_value_bin_uses_base64() {
        let v = encode_header_value("trace-bin", &[0xde, 0xad, 0xbe, 0xef]).expect("ok");
        // 0xdeadbeef → 3q2+7w==
        assert_eq!(v, "3q2+7w==");
    }

    #[test]
    fn decode_header_value_round_trip_bin() {
        let original = vec![0x01, 0x02, 0x03, 0x04, 0xff];
        let encoded = encode_header_value("trace-bin", &original).expect("encode");
        let decoded = decode_header_value("trace-bin", &encoded).expect("decode");
        assert_eq!(decoded, original);
    }

    #[test]
    fn decode_header_value_text_passes_through() {
        let decoded = decode_header_value("user-agent", "grpc-rust/1.0").expect("ok");
        assert_eq!(decoded, b"grpc-rust/1.0");
    }

    #[test]
    fn content_types_match_spec_strings() {
        assert_eq!(content_types::GRPC, "application/grpc");
        assert_eq!(content_types::GRPC_PROTO, "application/grpc+proto");
        assert_eq!(content_types::GRPC_WEB, "application/grpc-web");
        assert_eq!(content_types::GRPC_WEB_TEXT, "application/grpc-web-text");
    }

    #[test]
    fn request_headers_constants_match_spec() {
        assert_eq!(request_headers::METHOD, ":method");
        assert_eq!(request_headers::PATH, ":path");
        assert_eq!(request_headers::AUTHORITY, ":authority");
        assert_eq!(request_headers::TE, "te");
        assert_eq!(request_headers::GRPC_TIMEOUT, "grpc-timeout");
    }

    #[test]
    fn response_headers_constants_match_spec() {
        assert_eq!(response_headers::STATUS, ":status");
        assert_eq!(response_headers::GRPC_STATUS, "grpc-status");
        assert_eq!(response_headers::GRPC_MESSAGE, "grpc-message");
    }
}
