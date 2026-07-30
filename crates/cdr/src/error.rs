// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Encoder and decoder errors.
//!
//! Deliberately separated: a value can only be encoded or decoded, but
//! the error categories differ. Encoder errors are buffer/format
//! related; decoder errors are additionally validation errors
//! (UnexpectedEof, InvalidUtf8, etc.).

use core::fmt;

/// Error while encoding a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// Write buffer too small. Cannot happen with a `Vec<u8>`-based
    /// writer; can happen with fixed buffers (no_std + static array).
    BufferTooSmall {
        /// How many additional bytes would have been needed.
        needed: usize,
        /// How many were actually available.
        available: usize,
    },
    /// Value cannot be encoded — e.g. a `String` length exceeds
    /// `u32::MAX` (XCDR limit).
    ValueOutOfRange {
        /// Description of the violation.
        message: &'static str,
    },
    /// A mutable encode omitted a non-optional member
    /// (XTypes 1.3 §7.4.1.2.3 — "the serialized representation MUST
    /// contain at least the values of all the non-optional members").
    MissingNonOptionalMember {
        /// Member ID that was missing.
        member_id: u32,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { needed, available } => write!(
                f,
                "encoder buffer too small: needed {needed} bytes, available {available}"
            ),
            Self::ValueOutOfRange { message } => write!(f, "value out of range: {message}"),
            Self::MissingNonOptionalMember { member_id } => write!(
                f,
                "mutable struct missing non-optional member: id={member_id}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

/// Error while decoding a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Input ended before the expected end.
    UnexpectedEof {
        /// How many more bytes were expected.
        needed: usize,
        /// Position in the stream where the EOF occurred.
        offset: usize,
    },
    /// UTF-8 validation for a string value failed.
    InvalidUtf8 {
        /// Position where the string began.
        offset: usize,
    },
    /// Boolean byte was neither 0 nor 1 — forbidden by the XCDR spec.
    InvalidBool {
        /// Byte actually read.
        value: u8,
        /// Position of the byte.
        offset: usize,
    },
    /// Char value is not a valid Unicode codepoint.
    InvalidChar {
        /// u32 value actually read.
        value: u32,
        /// Position.
        offset: usize,
    },
    /// Sequence/array length exceeds the bound or the remaining bytes.
    LengthExceeded {
        /// How many elements the length announces.
        announced: usize,
        /// How many are actually still readable (best-effort).
        remaining: usize,
        /// Position.
        offset: usize,
    },
    /// String-format violation (e.g. missing null terminator, length 0).
    InvalidString {
        /// Position where the string began.
        offset: usize,
        /// Short description (static).
        reason: &'static str,
    },
    /// Unknown/invalid enum discriminator — used by policy decoders that
    /// operate in strict mode (e.g. QoS enums).
    InvalidEnum {
        /// Enum name for debugging (e.g. "DurabilityKind").
        kind: &'static str,
        /// Discriminator value that was read.
        value: u32,
    },
    /// A mutable decode read a `must_understand` member with an unknown
    /// member ID (XTypes 1.3 §7.4.1.2.3 — the receiver MUST discard the
    /// message in that case).
    UnknownMustUnderstandMember {
        /// Member ID that was not recognized.
        member_id: u32,
    },
    /// A mutable decode did not find a non-optional member in the wire
    /// data (XTypes 1.3 §7.4.1.2.3).
    MissingNonOptionalMember {
        /// Member ID that is missing.
        member_id: u32,
    },
    /// A bounded `string<N>` / `wstring<N>` / `sequence<T,N>` / `map<K,V,N>`
    /// value decoded from the wire exceeds its IDL-declared bound `N`
    /// (XTypes 1.3 §7.4.3 requires the bound to be enforced on both the
    /// encode AND the decode side — regression #22). Distinct from
    /// [`DecodeError::LengthExceeded`], which is a wire-framing violation
    /// (announced length vs. remaining bytes in the buffer); this is a
    /// semantic IDL-contract violation on an otherwise well-formed decode.
    BoundExceeded {
        /// Number of elements/characters/bytes actually decoded.
        actual: usize,
        /// The IDL-declared bound `N`.
        bound: usize,
        /// Description of which bounded type was violated.
        message: &'static str,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { needed, offset } => {
                write!(
                    f,
                    "unexpected EOF: needed {needed} more bytes at offset {offset}"
                )
            }
            Self::InvalidUtf8 { offset } => write!(f, "invalid UTF-8 at offset {offset}"),
            Self::InvalidBool { value, offset } => {
                write!(f, "invalid bool byte 0x{value:02x} at offset {offset}")
            }
            Self::InvalidChar { value, offset } => {
                write!(f, "invalid char codepoint U+{value:04X} at offset {offset}")
            }
            Self::LengthExceeded {
                announced,
                remaining,
                offset,
            } => write!(
                f,
                "length {announced} at offset {offset} exceeds remaining {remaining} bytes"
            ),
            Self::InvalidString { offset, reason } => {
                write!(f, "invalid CDR string at offset {offset}: {reason}")
            }
            Self::InvalidEnum { kind, value } => {
                write!(f, "invalid {kind} discriminator: {value}")
            }
            Self::UnknownMustUnderstandMember { member_id } => {
                write!(f, "unknown must_understand member id: {member_id}")
            }
            Self::MissingNonOptionalMember { member_id } => {
                write!(f, "missing non-optional member id: {member_id}")
            }
            Self::BoundExceeded {
                actual,
                bound,
                message,
            } => write!(
                f,
                "decoded {actual} exceeds the IDL bound {bound}: {message}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "alloc")]
    extern crate alloc;
    #[cfg(feature = "alloc")]
    use alloc::format;

    #[test]
    fn encode_error_display_buffer_too_small() {
        let e = EncodeError::BufferTooSmall {
            needed: 8,
            available: 4,
        };
        let s = format!("{e}");
        assert!(s.contains("8"));
        assert!(s.contains("4"));
    }

    #[test]
    fn encode_error_display_value_out_of_range() {
        let e = EncodeError::ValueOutOfRange {
            message: "string too long",
        };
        assert!(format!("{e}").contains("string too long"));
    }

    #[test]
    fn decode_error_display_unexpected_eof() {
        let e = DecodeError::UnexpectedEof {
            needed: 4,
            offset: 12,
        };
        let s = format!("{e}");
        assert!(s.contains("4"));
        assert!(s.contains("12"));
    }

    #[test]
    fn decode_error_display_invalid_utf8() {
        let e = DecodeError::InvalidUtf8 { offset: 5 };
        assert!(format!("{e}").contains("5"));
    }

    #[test]
    fn decode_error_display_invalid_bool() {
        let e = DecodeError::InvalidBool {
            value: 0xff,
            offset: 0,
        };
        assert!(format!("{e}").contains("ff"));
    }

    #[test]
    fn decode_error_display_invalid_char() {
        let e = DecodeError::InvalidChar {
            value: 0xD800,
            offset: 0,
        };
        assert!(format!("{e}").contains("D800"));
    }

    #[test]
    fn decode_error_display_length_exceeded() {
        let e = DecodeError::LengthExceeded {
            announced: 100,
            remaining: 4,
            offset: 0,
        };
        let s = format!("{e}");
        assert!(s.contains("100"));
        assert!(s.contains("4"));
    }

    #[test]
    fn errors_are_clone_eq() {
        let e1 = EncodeError::ValueOutOfRange { message: "x" };
        let e2 = e1.clone();
        assert_eq!(e1, e2);
        let d1 = DecodeError::UnexpectedEof {
            needed: 1,
            offset: 0,
        };
        assert_eq!(d1.clone(), d1);
    }
}
