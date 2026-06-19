// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Encoder and decoder errors for the OPC-UA binary wire format and the
//! UADP framing layers.
//!
//! Mirrors the split used in `zerodds-cdr`: encode errors are
//! buffer/range related, decode errors additionally cover validation
//! (truncation, invalid discriminants, bad UTF-8).

use core::fmt;

/// Error while encoding an OPC-UA / UADP value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// A length field exceeds the `i32::MAX` ceiling the OPC-UA binary
    /// encoding mandates for `String`/`ByteString`/array prefixes
    /// (OPC-UA Part 6 §5.2.2.4 — length is an `Int32`).
    LengthOverflow {
        /// The offending element kind (e.g. `"String"`, `"Array"`).
        what: &'static str,
        /// The length that could not be represented.
        len: usize,
    },
    /// A value cannot be represented on the wire — e.g. a UADP header
    /// field whose value is outside its defined range.
    ValueOutOfRange {
        /// Description of the violation.
        message: &'static str,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthOverflow { what, len } => {
                write!(f, "{what} length {len} exceeds OPC-UA Int32 ceiling")
            }
            Self::ValueOutOfRange { message } => write!(f, "value out of range: {message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

/// Error while decoding an OPC-UA / UADP value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Input ended before the expected number of bytes were available.
    UnexpectedEof {
        /// How many more bytes were needed.
        needed: usize,
        /// How many remained in the buffer.
        remaining: usize,
    },
    /// A `String` or `XmlElement` body was not valid UTF-8.
    InvalidUtf8,
    /// A discriminant byte did not match any defined case — carries the
    /// field name and the offending value for diagnosis.
    InvalidDiscriminant {
        /// Name of the field being decoded (e.g. `"NodeId encoding"`).
        field: &'static str,
        /// The value that did not match.
        value: u32,
    },
    /// A length prefix was negative but the type does not permit a null
    /// form (only `String`/`ByteString` use `-1` for null).
    NegativeLength {
        /// Name of the field being decoded.
        field: &'static str,
    },
    /// A structural invariant of a UADP message was violated (e.g. a
    /// payload header announced more DataSetMessages than the payload
    /// carried).
    MalformedMessage {
        /// Description of the violation.
        message: &'static str,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { needed, remaining } => write!(
                f,
                "unexpected end of input: needed {needed} bytes, {remaining} remaining"
            ),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in string body"),
            Self::InvalidDiscriminant { field, value } => {
                write!(f, "invalid discriminant for {field}: {value}")
            }
            Self::NegativeLength { field } => {
                write!(f, "negative length for non-nullable field {field}")
            }
            Self::MalformedMessage { message } => write!(f, "malformed UADP message: {message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}
