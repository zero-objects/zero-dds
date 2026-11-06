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
