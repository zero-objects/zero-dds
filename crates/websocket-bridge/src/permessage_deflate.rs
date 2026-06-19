// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! permessage-deflate extension — RFC 7692 §7.
//!
//! Spec §7: extension negotiation via the `Sec-WebSocket-Extensions` header.
//! Four parameters:
//! * `server_no_context_takeover` (boolean)
//! * `client_no_context_takeover` (boolean)
//! * `server_max_window_bits` (8..15)
//! * `client_max_window_bits` (8..15)
//!
//! Wire format: the RSV1 bit in the frame header indicates compressed
//! messages (Spec §6.1). The wire body is DEFLATE-compressed with the tail
//! `00 00 FF FF` stripped (Spec §7.2.1).
//!
//! We provide negotiation + frame wrapping; the DEFLATE compression
//! itself is plugged in by the caller via the `DeflateCodec` trait.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Spec §7 tail marker — stripped on send, appended on receive.
pub const DEFLATE_TAIL: [u8; 4] = [0x00, 0x00, 0xff, 0xff];

/// Negotiation parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermessageDeflateParams {
    /// `server_no_context_takeover`.
    pub server_no_takeover: bool,
    /// `client_no_context_takeover`.
    pub client_no_takeover: bool,
    /// `server_max_window_bits` (8..=15).
    pub server_max_window_bits: u8,
    /// `client_max_window_bits` (8..=15).
    pub client_max_window_bits: u8,
}

impl Default for PermessageDeflateParams {
    fn default() -> Self {
        Self {
            server_no_takeover: false,
            client_no_takeover: false,
            server_max_window_bits: 15,
            client_max_window_bits: 15,
        }
    }
}

/// Negotiation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NegotiationError {
    /// Unknown parameter.
    UnknownParam(String),
    /// Window bits outside 8..=15.
    InvalidWindowBits(u8),
    /// A boolean parameter has a value (should be parameterless).
    BooleanWithValue(String),
}

impl core::fmt::Display for NegotiationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownParam(p) => write!(f, "unknown parameter: {p}"),
            Self::InvalidWindowBits(b) => write!(f, "invalid window_bits: {b}"),
            Self::BooleanWithValue(p) => write!(f, "boolean param `{p}` has value"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for NegotiationError {}

/// Parses a `Sec-WebSocket-Extensions` value. Spec §7.1.
///
/// Format: `permessage-deflate; param1; param2=value; ...`
///
/// # Errors
/// See [`NegotiationError`].
pub fn parse_offer(offer: &str) -> Result<PermessageDeflateParams, NegotiationError> {
    let mut params = PermessageDeflateParams::default();
    for part in offer.split(';').skip(1) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((k, v)) = part.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"');
            match k {
                "server_max_window_bits" => {
                    let bits: u8 = v
                        .parse()
                        .map_err(|_| NegotiationError::InvalidWindowBits(0))?;
                    if !(8..=15).contains(&bits) {
                        return Err(NegotiationError::InvalidWindowBits(bits));
                    }
                    params.server_max_window_bits = bits;
                }
                "client_max_window_bits" => {
                    let bits: u8 = v
                        .parse()
                        .map_err(|_| NegotiationError::InvalidWindowBits(0))?;
                    if !(8..=15).contains(&bits) {
                        return Err(NegotiationError::InvalidWindowBits(bits));
                    }
                    params.client_max_window_bits = bits;
                }
                "server_no_context_takeover" | "client_no_context_takeover" => {
                    return Err(NegotiationError::BooleanWithValue(k.to_string()));
                }
                other => return Err(NegotiationError::UnknownParam(other.to_string())),
            }
        } else {
            match part {
                "server_no_context_takeover" => params.server_no_takeover = true,
                "client_no_context_takeover" => params.client_no_takeover = true,
                "client_max_window_bits" => {
                    // Client-side parameterless → server free to choose.
                    params.client_max_window_bits = 15;
                }
                other => return Err(NegotiationError::UnknownParam(other.to_string())),
            }
        }
    }
    Ok(params)
}

/// Render the server accept header value.
#[must_use]
pub fn render_accept(params: &PermessageDeflateParams) -> String {
    let mut s = String::from("permessage-deflate");
    if params.server_no_takeover {
        s.push_str("; server_no_context_takeover");
    }
    if params.client_no_takeover {
        s.push_str("; client_no_context_takeover");
    }
    if params.server_max_window_bits != 15 {
        s.push_str(&alloc::format!(
            "; server_max_window_bits={}",
            params.server_max_window_bits
        ));
    }
    if params.client_max_window_bits != 15 {
        s.push_str(&alloc::format!(
            "; client_max_window_bits={}",
            params.client_max_window_bits
        ));
    }
    s
}

/// Append the tail after DEFLATE decompression. Spec §7.2.2.
#[must_use]
pub fn append_tail(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 4);
    out.extend_from_slice(payload);
    out.extend_from_slice(&DEFLATE_TAIL);
    out
}

/// Strip the tail before sending compressed frames. Spec §7.2.1.
#[must_use]
pub fn strip_tail(payload: &[u8]) -> &[u8] {
    if payload.ends_with(&DEFLATE_TAIL) {
        &payload[..payload.len() - DEFLATE_TAIL.len()]
    } else {
        payload
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_no_params_yields_defaults() {
        let p = parse_offer("permessage-deflate").unwrap();
        assert_eq!(p, PermessageDeflateParams::default());
    }

    #[test]
    fn parse_no_takeover_flags() {
        let p = parse_offer(
            "permessage-deflate; server_no_context_takeover; client_no_context_takeover",
        )
        .unwrap();
        assert!(p.server_no_takeover);
        assert!(p.client_no_takeover);
    }

    #[test]
    fn parse_window_bits() {
        let p =
            parse_offer("permessage-deflate; server_max_window_bits=12; client_max_window_bits=10")
                .unwrap();
        assert_eq!(p.server_max_window_bits, 12);
        assert_eq!(p.client_max_window_bits, 10);
    }

    #[test]
    fn rejects_invalid_window_bits() {
        assert!(parse_offer("permessage-deflate; server_max_window_bits=7").is_err());
        assert!(parse_offer("permessage-deflate; server_max_window_bits=16").is_err());
    }

    #[test]
    fn rejects_unknown_param() {
        assert!(matches!(
            parse_offer("permessage-deflate; foo"),
            Err(NegotiationError::UnknownParam(_))
        ));
    }

    #[test]
    fn rejects_boolean_with_value() {
        assert!(matches!(
            parse_offer("permessage-deflate; server_no_context_takeover=yes"),
            Err(NegotiationError::BooleanWithValue(_))
        ));
    }

    #[test]
    fn render_default_is_bare_extension_name() {
        let s = render_accept(&PermessageDeflateParams::default());
        assert_eq!(s, "permessage-deflate");
    }

    #[test]
    fn render_includes_params() {
        let p = PermessageDeflateParams {
            server_no_takeover: true,
            client_no_takeover: false,
            server_max_window_bits: 12,
            client_max_window_bits: 15,
        };
        let s = render_accept(&p);
        assert!(s.contains("server_no_context_takeover"));
        assert!(s.contains("server_max_window_bits=12"));
        assert!(!s.contains("client_max_window_bits"));
    }

    #[test]
    fn tail_round_trip() {
        let raw = b"hello";
        let with_tail = append_tail(raw);
        assert_eq!(with_tail, b"hello\x00\x00\xff\xff");
        let stripped = strip_tail(&with_tail);
        assert_eq!(stripped, raw);
    }

    #[test]
    fn strip_tail_no_op_when_absent() {
        assert_eq!(strip_tail(b"hello"), b"hello");
    }

    #[test]
    fn parameterless_client_max_window_bits_accepted() {
        let p = parse_offer("permessage-deflate; client_max_window_bits").unwrap();
        assert_eq!(p.client_max_window_bits, 15);
    }
}
