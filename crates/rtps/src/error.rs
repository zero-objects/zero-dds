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
