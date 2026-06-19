// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RTPS wire-format errors.

use core::fmt;

/// Error when encoding or decoding RTPS wire data.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WireError {
    /// Buffer is too small for the write operation.
    WriteOverflow {
        /// How many bytes would have been needed.
        needed: usize,
        /// How many were actually available.
        available: usize,
    },
    /// Input ended before the expected end.
    UnexpectedEof {
        /// How many more bytes were expected.
        needed: usize,
        /// Position in the stream.
        offset: usize,
    },
    /// Magic bytes of the RTPS header do not match ("RTPS").
    InvalidMagic {
        /// The 4 bytes actually read.
        found: [u8; 4],
    },
    /// Submessage ID is not in the supported set.
    UnknownSubmessageId {
        /// Raw submessage-ID byte.
        id: u8,
    },
    /// Locator kind has an unexpected value.
    InvalidLocatorKind {
        /// Raw i32 value.
        kind: i32,
    },
    /// Numeric value exceeds the wire field.
    ValueOutOfRange {
        /// Description.
        message: &'static str,
    },
    /// Encapsulation kind in the CDR header is not supported
    /// (e.g. PL_CDR2, CDR_LE when PL_CDR is expected).
    UnsupportedEncapsulation {
        /// The two encapsulation bytes `[kind_hi, kind_lo]`.
        kind: [u8; 2],
    },
    /// Spec-conformant wire feature that is (not yet) supported in this
    /// phase (e.g. inline QoS in DATA_FRAG).
    UnsupportedFeature {
        /// Name/short description of the feature.
        what: &'static str,
    },
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteOverflow { needed, available } => {
                write!(
                    f,
                    "RTPS write overflow: needed {needed}, available {available}"
                )
            }
            Self::UnexpectedEof { needed, offset } => {
                write!(f, "RTPS unexpected EOF: needed {needed} at offset {offset}")
            }
            Self::InvalidMagic { found } => {
                write!(f, "RTPS invalid magic: expected b\"RTPS\", got {found:?}")
            }
            Self::UnknownSubmessageId { id } => {
                write!(f, "RTPS unknown submessage id 0x{id:02x}")
            }
            Self::InvalidLocatorKind { kind } => {
                write!(f, "RTPS invalid locator kind {kind}")
            }
            Self::ValueOutOfRange { message } => write!(f, "RTPS value out of range: {message}"),
            Self::UnsupportedEncapsulation { kind } => {
                write!(
                    f,
                    "RTPS unsupported encapsulation kind 0x{:02x}{:02x}",
                    kind[0], kind[1]
                )
            }
            Self::UnsupportedFeature { what } => {
                write!(f, "RTPS unsupported feature: {what}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for WireError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[cfg(feature = "alloc")]
    extern crate alloc;
    #[cfg(feature = "alloc")]
    use alloc::format;

    #[cfg(feature = "alloc")]
    #[test]
    fn write_overflow_display() {
        let e = WireError::WriteOverflow {
            needed: 16,
            available: 4,
        };
        let s = format!("{e}");
        assert!(s.contains("16"));
        assert!(s.contains("4"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn invalid_magic_display() {
        let e = WireError::InvalidMagic {
            found: [b'X', b'X', b'X', b'X'],
        };
        let s = format!("{e}");
        assert!(s.contains("RTPS"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn unknown_submessage_id_display_uses_hex() {
        let e = WireError::UnknownSubmessageId { id: 0x42 };
        let s = format!("{e}");
        assert!(s.contains("42"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn invalid_locator_kind_display() {
        let e = WireError::InvalidLocatorKind { kind: 99 };
        assert!(format!("{e}").contains("99"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn value_out_of_range_display() {
        let e = WireError::ValueOutOfRange { message: "too big" };
        assert!(format!("{e}").contains("too big"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn unexpected_eof_display() {
        let e = WireError::UnexpectedEof {
            needed: 8,
            offset: 12,
        };
        let s = format!("{e}");
        assert!(s.contains("8"));
        assert!(s.contains("12"));
    }
}
