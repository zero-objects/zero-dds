// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE wire format errors.
//!
//! Analogous to `zerodds_rtps::WireError`, but with XRCE-specific
//! variants (e.g. for a missing ClientKey presence).

use core::fmt;

/// Error while encoding or decoding XRCE wire data.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum XrceError {
    /// Output buffer too small.
    WriteOverflow {
        /// How many bytes would have been needed.
        needed: usize,
        /// How many were actually available.
        available: usize,
    },
    /// Input ended before the expected end.
    UnexpectedEof {
        /// How many bytes were still expected.
        needed: usize,
        /// Position in the stream at the time of the error.
        offset: usize,
    },
    /// Submessage ID is not in 0..=15 (Spec §8.3.5).
    UnknownSubmessageId {
        /// Raw submessage ID byte.
        id: u8,
    },
    /// The SubmessageHeader length points to a body that would extend
    /// beyond the end of the datagram (truncation).
    TruncatedSubmessageBody {
        /// Header value `submessage_length`.
        declared: u16,
        /// Actually available bytes from the body start.
        available: usize,
    },
    /// The number of submessages in a message exceeds
    /// `DOSC_MAX_SUBMESSAGES` — DoS protection.
    TooManySubmessages {
        /// Fixed limit.
        limit: usize,
    },
    /// Payload size exceeds `DOSC_MAX_PAYLOAD_SIZE` —
    /// DoS protection.
    PayloadTooLarge {
        /// Limit.
        limit: usize,
        /// Actual size.
        actual: usize,
    },
    /// SubmessageLength does not fit the 4-byte alignment
    /// (§8.3.3: the offset of each submessage must be a multiple of 4).
    UnalignedSubmessage {
        /// Offset at which the problem occurred.
        offset: usize,
    },
    /// Numeric value exceeds the wire field.
    ValueOutOfRange {
        /// Description.
        message: &'static str,
    },
}

impl fmt::Display for XrceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteOverflow { needed, available } => write!(
                f,
                "xrce write overflow: needed {needed}, available {available}"
            ),
            Self::UnexpectedEof { needed, offset } => {
                write!(f, "xrce unexpected EOF: needed {needed} at offset {offset}")
            }
            Self::UnknownSubmessageId { id } => {
                write!(f, "xrce unknown submessage id 0x{id:02x}")
            }
            Self::TruncatedSubmessageBody {
                declared,
                available,
            } => write!(
                f,
                "xrce submessage body truncated: declared {declared}, available {available}"
            ),
            Self::TooManySubmessages { limit } => {
                write!(f, "xrce too many submessages: limit {limit}")
            }
            Self::PayloadTooLarge { limit, actual } => {
                write!(f, "xrce payload too large: limit {limit}, actual {actual}")
            }
            Self::UnalignedSubmessage { offset } => {
                write!(f, "xrce submessage not 4-byte aligned at offset {offset}")
            }
            Self::ValueOutOfRange { message } => {
                write!(f, "xrce value out of range: {message}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for XrceError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[cfg(feature = "alloc")]
    extern crate alloc;
    #[cfg(feature = "alloc")]
    use alloc::format;

    #[cfg(feature = "alloc")]
    #[test]
    fn write_overflow_display_includes_numbers() {
        let e = XrceError::WriteOverflow {
            needed: 16,
            available: 4,
        };
        let s = format!("{e}");
        assert!(s.contains("16") && s.contains("4"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn unknown_submessage_id_display_uses_hex() {
        let e = XrceError::UnknownSubmessageId { id: 0xAB };
        let s = format!("{e}");
        assert!(s.contains("ab"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn truncated_body_display_includes_values() {
        let e = XrceError::TruncatedSubmessageBody {
            declared: 100,
            available: 50,
        };
        let s = format!("{e}");
        assert!(s.contains("100") && s.contains("50"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn too_many_submessages_display_mentions_limit() {
        let e = XrceError::TooManySubmessages { limit: 64 };
        assert!(format!("{e}").contains("64"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn payload_too_large_display_includes_both() {
        let e = XrceError::PayloadTooLarge {
            limit: 1024,
            actual: 2048,
        };
        let s = format!("{e}");
        assert!(s.contains("1024") && s.contains("2048"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn unaligned_display_mentions_offset() {
        let e = XrceError::UnalignedSubmessage { offset: 17 };
        assert!(format!("{e}").contains("17"));
    }
}
