// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Error types for zerodds-types.

use core::fmt;

use zerodds_wire::WireError;

/// Error while encoding or decoding a `TypeObject` or a
/// `TypeIdentifier`.
///
/// One wire variant rather than the two the `cdr` era carried. `cdr` had
/// a `DecodeError` and an `EncodeError`; `wire` has one `WireError`, and
/// two wrappers around one type would have differed only in the word
/// this crate happened to put in front of it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypeCodecError {
    /// A read or a write the byte runtime refused.
    Wire(WireError),
    /// Unknown `TypeKind` discriminator on decode.
    UnknownTypeKind {
        /// The byte that was actually read.
        kind: u8,
    },
}

impl fmt::Display for TypeCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(e) => write!(f, "type codec wire error: {e}"),
            Self::UnknownTypeKind { kind } => {
                write!(f, "unknown TypeKind discriminator 0x{kind:02x}")
            }
        }
    }
}

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
impl std::error::Error for TypeCodecError {}

impl From<WireError> for TypeCodecError {
    fn from(e: WireError) -> Self {
        Self::Wire(e)
    }
}

impl TypeCodecError {
    /// The failure as the byte runtime would have reported it, for a
    /// caller whose own signature can only return a [`WireError`].
    ///
    /// An element function handed to a sequence decoder is the case:
    /// the element may be a whole `TypeObject`, so it can stop on a
    /// discriminator the specification does not define, and that is not
    /// a truncation. The bytes were there and said something
    /// impossible, which is [`WireError::OutOfRange`] at `at`.
    #[must_use]
    pub fn into_wire(self, at: usize) -> WireError {
        match self {
            Self::Wire(e) => e,
            Self::UnknownTypeKind { kind } => WireError::OutOfRange {
                at,
                expected: alloc::format!(
                    "a TypeKind the specification defines, and 0x{kind:02x} is not one"
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::{String, ToString};

    #[test]
    fn display_unknown_type_kind_hex_formatted() {
        let e = TypeCodecError::UnknownTypeKind { kind: 0xAB };
        let s = e.to_string();
        assert!(s.contains("0xab") || s.contains("0xAB"), "got: {s}");
    }

    #[test]
    fn a_wire_error_converts_and_keeps_its_own_text() {
        let wire = WireError::Truncated {
            at: 8,
            needed: 4,
            available: 0,
        };
        let converted = TypeCodecError::from(wire.clone());
        assert!(matches!(converted, TypeCodecError::Wire(_)));
        assert!(
            converted.to_string().contains(&wire.to_string()),
            "the wrapper must not lose what the runtime said: {converted}"
        );
    }

    #[test]
    fn display_variants_do_not_panic() {
        let _ = TypeCodecError::Wire(WireError::OutOfRange {
            at: 0,
            expected: String::from("x"),
        })
        .to_string();
        let _ = TypeCodecError::Wire(WireError::ImpossibleLength {
            at: 4,
            claimed: 1 << 30,
            available: 12,
        })
        .to_string();
    }
}
