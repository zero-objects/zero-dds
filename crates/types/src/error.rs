// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fehlertypen fuer zerodds-types.

use core::fmt;

use zerodds_cdr::{DecodeError, EncodeError};

/// Fehler beim Encoden/Decoden von TypeObject / TypeIdentifier.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypeCodecError {
    /// Wire-Encode-Fehler aus dem CDR-Layer.
    Encode(EncodeError),
    /// Wire-Decode-Fehler.
    Decode(DecodeError),
    /// Unbekannter TypeKind-Diskriminator beim Decoden.
    UnknownTypeKind {
        /// Tatsaechlich gelesener Byte-Wert.
        kind: u8,
    },
}

impl fmt::Display for TypeCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(e) => write!(f, "type codec encode error: {e}"),
            Self::Decode(e) => write!(f, "type codec decode error: {e}"),
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

impl From<EncodeError> for TypeCodecError {
    fn from(e: EncodeError) -> Self {
        Self::Encode(e)
    }
}

impl From<DecodeError> for TypeCodecError {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn display_unknown_type_kind_hex_formatted() {
        let e = TypeCodecError::UnknownTypeKind { kind: 0xAB };
        let s = e.to_string();
        assert!(s.contains("0xab") || s.contains("0xAB"), "got: {s}");
    }

    #[test]
    fn from_encode_and_decode_error() {
        let enc = EncodeError::ValueOutOfRange { message: "test" };
        let e1 = TypeCodecError::from(enc);
        assert!(matches!(e1, TypeCodecError::Encode(_)));

        let dec = DecodeError::UnexpectedEof {
            needed: 4,
            offset: 0,
        };
        let e2 = TypeCodecError::from(dec);
        assert!(matches!(e2, TypeCodecError::Decode(_)));
    }

    #[test]
    fn display_encode_decode_variants_dont_panic() {
        let _ = TypeCodecError::Encode(EncodeError::ValueOutOfRange { message: "x" }).to_string();
        let _ = TypeCodecError::Decode(DecodeError::UnexpectedEof {
            needed: 1,
            offset: 2,
        })
        .to_string();
    }
}
