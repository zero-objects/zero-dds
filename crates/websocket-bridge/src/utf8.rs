// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! UTF-8 validation per RFC 6455 §8.1 + §8.2.
//!
//! Spec: a WebSocket server MUST close the connection with status 1007 if
//! a text frame is UTF-8-invalid. Symmetric for the client.
//!
//! We validate strictly (RFC 3629), incl.:
//! - Surrogate-pair codepoints (U+D800..=U+DFFF) are forbidden.
//! - Overlong encoding is forbidden.
//! - Codepoints > U+10FFFF are forbidden.

use core::result::Result;

/// UTF-8 validation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Utf8Error {
    /// Truncated multi-byte sequence (a continuation byte is missing at the end).
    Truncated,
    /// Continuation byte without a lead byte (10xxxxxx isolated).
    UnexpectedContinuation,
    /// The lead byte is invalid (e.g. `0xC0`, `0xC1`, `0xF5`-`0xFF`).
    InvalidLeadByte,
    /// Surrogate codepoint U+D800..=U+DFFF.
    SurrogateCodepoint,
    /// Overlong encoding (e.g. `0xC0 0x80` for NUL).
    OverlongEncoding,
    /// Codepoint > U+10FFFF.
    CodepointOutOfRange,
    /// The continuation byte is not `10xxxxxx`.
    InvalidContinuation,
}

impl core::fmt::Display for Utf8Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => write!(f, "Truncated"),
            Self::UnexpectedContinuation => write!(f, "UnexpectedContinuation"),
            Self::InvalidLeadByte => write!(f, "InvalidLeadByte"),
            Self::SurrogateCodepoint => write!(f, "SurrogateCodepoint"),
            Self::OverlongEncoding => write!(f, "OverlongEncoding"),
            Self::CodepointOutOfRange => write!(f, "CodepointOutOfRange"),
            Self::InvalidContinuation => write!(f, "InvalidContinuation"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Utf8Error {}

/// Validates a complete UTF-8 byte sequence per RFC 3629.
///
/// Returns `Ok(())` if all bytes form a valid UTF-8 stream,
/// otherwise the first violation found.
///
/// # Errors
/// See [`Utf8Error`].
pub fn validate(bytes: &[u8]) -> Result<(), Utf8Error> {
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        let needed = match b0 {
            0x00..=0x7F => 0, // ASCII
            0xC0..=0xC1 => return Err(Utf8Error::OverlongEncoding),
            0xC2..=0xDF => 1, // 2-byte
            0xE0..=0xEF => 2, // 3-byte
            0xF0..=0xF4 => 3, // 4-byte
            0xF5..=0xFF => return Err(Utf8Error::InvalidLeadByte),
            0x80..=0xBF => return Err(Utf8Error::UnexpectedContinuation),
        };

        if needed == 0 {
            i += 1;
            continue;
        }

        if i + needed >= bytes.len() {
            return Err(Utf8Error::Truncated);
        }

        // Validate continuation bytes are 10xxxxxx
        for k in 1..=needed {
            if (bytes[i + k] & 0b1100_0000) != 0b1000_0000 {
                return Err(Utf8Error::InvalidContinuation);
            }
        }

        // Compute codepoint and check overlong + surrogate + range.
        let cp = match needed {
            1 => {
                let cp = (u32::from(b0 & 0b0001_1111) << 6) | u32::from(bytes[i + 1] & 0b0011_1111);
                if cp < 0x80 {
                    return Err(Utf8Error::OverlongEncoding);
                }
                cp
            }
            2 => {
                let cp = (u32::from(b0 & 0b0000_1111) << 12)
                    | (u32::from(bytes[i + 1] & 0b0011_1111) << 6)
                    | u32::from(bytes[i + 2] & 0b0011_1111);
                if cp < 0x800 {
                    return Err(Utf8Error::OverlongEncoding);
                }
                if (0xD800..=0xDFFF).contains(&cp) {
                    return Err(Utf8Error::SurrogateCodepoint);
                }
                cp
            }
            3 => {
                let cp = (u32::from(b0 & 0b0000_0111) << 18)
                    | (u32::from(bytes[i + 1] & 0b0011_1111) << 12)
                    | (u32::from(bytes[i + 2] & 0b0011_1111) << 6)
                    | u32::from(bytes[i + 3] & 0b0011_1111);
                if cp < 0x1_0000 {
                    return Err(Utf8Error::OverlongEncoding);
                }
                if cp > 0x10_FFFF {
                    return Err(Utf8Error::CodepointOutOfRange);
                }
                cp
            }
            _ => return Err(Utf8Error::InvalidLeadByte),
        };
        let _ = cp;
        i += 1 + needed;
    }
    Ok(())
}

/// Streaming UTF-8 validator for fragmented text frames.
///
/// WebSocket spec §6.2 may split text frames into multiple DATA frames
/// (FIN=0) — the UTF-8 stream may be interrupted in the middle of a
/// multi-byte sequence. The validator keeps the state of the code point
/// that has been started and reports a stream as malformed with the octet
/// that makes it so: the second octet of a surrogate, of an overlong form
/// or of a code point above U+10FFFF, not the end of its sequence.
#[derive(Debug, Default, Clone)]
pub struct StreamingValidator {
    /// The lead octet of the code point in progress; meaningful while
    /// `needed` is not zero.
    lead: u8,
    /// The continuation octets the code point in progress still lacks.
    needed: u8,
    /// The next octet is the first one behind the lead octet, whose range
    /// depends on the lead (RFC 3629 §4).
    second: bool,
}

impl StreamingValidator {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check a chunk of bytes. Returns `Ok(())` if (together with the
    /// previous state) everything so far is valid.
    ///
    /// # Errors
    /// See [`Utf8Error`].
    pub fn feed(&mut self, chunk: &[u8]) -> Result<(), Utf8Error> {
        for &octet in chunk {
            if self.needed == 0 {
                self.needed = match octet {
                    0x00..=0x7F => 0,
                    0xC0..=0xC1 => return Err(Utf8Error::OverlongEncoding),
                    0xC2..=0xDF => 1,
                    0xE0..=0xEF => 2,
                    0xF0..=0xF4 => 3,
                    0xF5..=0xFF => return Err(Utf8Error::InvalidLeadByte),
                    0x80..=0xBF => return Err(Utf8Error::UnexpectedContinuation),
                };
                self.lead = octet;
                self.second = true;
                continue;
            }
            if (octet & 0b1100_0000) != 0b1000_0000 {
                return Err(Utf8Error::InvalidContinuation);
            }
            if self.second {
                // RFC 3629 §4: the second octet of a sequence decides
                // whether the form is the shortest one, a surrogate, or
                // above U+10FFFF.
                match (self.lead, octet) {
                    (0xE0, 0x80..=0x9F) | (0xF0, 0x80..=0x8F) => {
                        return Err(Utf8Error::OverlongEncoding);
                    }
                    (0xED, 0xA0..=0xBF) => return Err(Utf8Error::SurrogateCodepoint),
                    (0xF4, 0x90..=0xBF) => return Err(Utf8Error::CodepointOutOfRange),
                    _ => {}
                }
                self.second = false;
            }
            self.needed -= 1;
        }
        Ok(())
    }

    /// `true` if the stream ends on a codepoint boundary (no
    /// pending byte). A text frame with FIN=1 MUST reach this state —
    /// otherwise a truncated error.
    ///
    /// # Errors
    /// `Utf8Error::Truncated` if pending bytes remain.
    pub fn finalize(self) -> Result<(), Utf8Error> {
        if self.needed == 0 {
            Ok(())
        } else {
            Err(Utf8Error::Truncated)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_8_1_empty_is_valid() {
        assert!(validate(b"").is_ok());
    }

    #[test]
    fn ws_8_1_ascii_is_valid() {
        assert!(validate(b"hello world").is_ok());
    }

    #[test]
    fn ws_8_1_valid_2_byte_codepoint() {
        // U+00E9 (é) = 0xC3 0xA9
        assert!(validate(&[0xC3, 0xA9]).is_ok());
    }

    #[test]
    fn ws_8_1_valid_3_byte_codepoint() {
        // U+20AC (€) = 0xE2 0x82 0xAC
        assert!(validate(&[0xE2, 0x82, 0xAC]).is_ok());
    }

    #[test]
    fn ws_8_1_valid_4_byte_codepoint() {
        // U+1F600 (😀) = 0xF0 0x9F 0x98 0x80
        assert!(validate(&[0xF0, 0x9F, 0x98, 0x80]).is_ok());
    }

    #[test]
    fn ws_8_1_rejects_overlong_2_byte_for_ascii() {
        // 0xC0 0x80 would encode U+0000 in 2 bytes (overlong)
        assert_eq!(validate(&[0xC0, 0x80]), Err(Utf8Error::OverlongEncoding));
    }

    #[test]
    fn ws_8_1_rejects_unexpected_continuation_byte() {
        assert_eq!(validate(&[0x80]), Err(Utf8Error::UnexpectedContinuation));
    }

    #[test]
    fn ws_8_1_rejects_invalid_lead_byte() {
        assert_eq!(validate(&[0xFF]), Err(Utf8Error::InvalidLeadByte));
    }

    #[test]
    fn ws_8_1_rejects_truncated_2_byte() {
        // 0xC3 (lead) without continuation
        assert_eq!(validate(&[0xC3]), Err(Utf8Error::Truncated));
    }

    #[test]
    fn ws_8_1_rejects_truncated_3_byte() {
        assert_eq!(validate(&[0xE2, 0x82]), Err(Utf8Error::Truncated));
    }

    #[test]
    fn ws_8_1_rejects_invalid_continuation() {
        // 0xC3 0x00 — second byte is not 10xxxxxx
        assert_eq!(validate(&[0xC3, 0x00]), Err(Utf8Error::InvalidContinuation));
    }

    #[test]
    fn ws_8_1_rejects_surrogate_codepoint() {
        // U+D800 = 0xED 0xA0 0x80 — surrogate
        assert_eq!(
            validate(&[0xED, 0xA0, 0x80]),
            Err(Utf8Error::SurrogateCodepoint)
        );
    }

    #[test]
    fn ws_8_1_rejects_codepoint_above_max() {
        // 0xF4 0x90 0x80 0x80 = U+110000 (just out of range)
        assert_eq!(
            validate(&[0xF4, 0x90, 0x80, 0x80]),
            Err(Utf8Error::CodepointOutOfRange)
        );
    }

    #[test]
    fn ws_8_1_streaming_handles_split_codepoint() {
        // U+20AC split: 0xE2 in chunk 1, 0x82 0xAC in chunk 2
        let mut v = StreamingValidator::new();
        assert!(v.feed(&[0xE2]).is_ok());
        assert!(v.feed(&[0x82, 0xAC]).is_ok());
        assert!(v.finalize().is_ok());
    }

    #[test]
    fn ws_8_1_streaming_agrees_with_validate_at_every_split() {
        // Every one- to four-octet sequence class of RFC 3629 §4, valid
        // and malformed: fed whole, octet by octet and cut at each
        // position, the stream is valid exactly when `validate` says so,
        // with the same error.
        let samples: [&[u8]; 22] = [
            b"plain",
            &[0xc2, 0x80],
            &[0xdf, 0xbf],
            &[0xe0, 0xa0, 0x80],
            &[0xed, 0x9f, 0xbf],
            &[0xee, 0x80, 0x80],
            &[0xef, 0xbf, 0xbf],
            &[0xf0, 0x90, 0x80, 0x80],
            &[0xf4, 0x8f, 0xbf, 0xbf],
            &[b'a', 0xe2, 0x82, 0xac, b'b', 0xf0, 0x9f, 0x98, 0x80],
            &[0xc0, 0x80],
            &[0xc1, 0xbf],
            &[0xe0, 0x9f, 0xbf],
            &[0xed, 0xa0, 0x80],
            &[0xed, 0xbf, 0xbf],
            &[0xf0, 0x8f, 0xbf, 0xbf],
            &[0xf4, 0x90, 0x80, 0x80],
            &[0xf5, 0x80, 0x80, 0x80],
            &[0x80],
            &[0xc2, 0x41],
            &[0xe2, 0x82, 0x41],
            &[0xf0, 0x9f, 0x98, 0x41],
        ];
        for sample in samples {
            let expected = validate(sample);
            for cut in 0..=sample.len() {
                let mut v = StreamingValidator::new();
                let got = v
                    .feed(&sample[..cut])
                    .and_then(|()| v.feed(&sample[cut..]))
                    .and_then(|()| v.finalize());
                assert_eq!(got, expected, "{sample:02x?} cut at {cut}");
            }
        }
    }

    #[test]
    fn ws_8_1_streaming_fails_with_the_octet_that_makes_the_stream_malformed() {
        // The second octet decides: no valid sequence begins with these
        // pairs, so the stream is malformed before the sequence is
        // complete.
        for (pair, error) in [
            ([0xe0u8, 0x9f], Utf8Error::OverlongEncoding),
            ([0xed, 0xa0], Utf8Error::SurrogateCodepoint),
            ([0xf0, 0x8f], Utf8Error::OverlongEncoding),
            ([0xf4, 0x90], Utf8Error::CodepointOutOfRange),
        ] {
            let mut v = StreamingValidator::new();
            assert_eq!(v.feed(&pair[..1]), Ok(()));
            assert_eq!(v.feed(&pair[1..]), Err(error), "{pair:02x?}");
        }
        // The pairs at the edges of the valid ranges are the start of a
        // code point.
        for pair in [[0xe0u8, 0xa0], [0xed, 0x9f], [0xf0, 0x90], [0xf4, 0x8f]] {
            let mut v = StreamingValidator::new();
            assert_eq!(v.feed(&pair), Ok(()), "{pair:02x?}");
            assert_eq!(v.finalize(), Err(Utf8Error::Truncated));
        }
    }

    #[test]
    fn ws_8_1_streaming_finalize_with_pending_is_truncated() {
        let mut v = StreamingValidator::new();
        assert!(v.feed(&[0xE2, 0x82]).is_ok());
        assert_eq!(v.finalize(), Err(Utf8Error::Truncated));
    }

    #[test]
    fn ws_8_1_streaming_complete_codepoint_in_one_chunk() {
        let mut v = StreamingValidator::new();
        assert!(v.feed(b"hello").is_ok());
        assert!(v.finalize().is_ok());
    }
}
