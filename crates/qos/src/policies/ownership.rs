// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OwnershipQosPolicy (DDS 1.4 §2.2.3.23).
//!
//! Wire-Format: u32 kind = 4 byte.
//!
//! Compatibility: offered.kind MUST = requested.kind (no ordering
//! relation; SHARED and EXCLUSIVE match only identically — §2.2.3 Table).

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// Ownership-Kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum OwnershipKind {
    /// Multiple writers allowed (default).
    #[default]
    Shared = 0,
    /// Only one writer (max strength) delivers samples.
    Exclusive = 1,
}

impl OwnershipKind {
    /// Strikter Mapper.
    #[must_use]
    pub const fn try_from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Shared),
            1 => Some(Self::Exclusive),
            _ => None,
        }
    }

    /// Forward-compatible mapper.
    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Exclusive,
            _ => Self::Shared,
        }
    }
}

/// OwnershipQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OwnershipQosPolicy {
    /// Kind.
    pub kind: OwnershipKind,
}

impl OwnershipQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.kind as u32)
    }

    /// Wire-Decoding (strict).
    ///
    /// # Errors
    /// Buffer underflow or unknown kind value.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let v = r.read_u32()?;
        let kind = OwnershipKind::try_from_u32(v).ok_or(DecodeError::InvalidEnum {
            kind: "OwnershipKind",
            value: v,
        })?;
        Ok(Self { kind })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn default_shared() {
        assert_eq!(OwnershipQosPolicy::default().kind, OwnershipKind::Shared);
    }

    #[test]
    fn try_from_u32_strict() {
        assert_eq!(OwnershipKind::try_from_u32(2), None);
    }

    #[test]
    fn roundtrip() {
        for kind in [OwnershipKind::Shared, OwnershipKind::Exclusive] {
            let p = OwnershipQosPolicy { kind };
            let mut w = BufferWriter::new(Endianness::Little);
            p.encode_into(&mut w).unwrap();
            let bytes = w.into_bytes();
            let mut r = BufferReader::new(&bytes, Endianness::Little);
            assert_eq!(OwnershipQosPolicy::decode_from(&mut r).unwrap(), p);
        }
    }

    /// `try_from_u32` accepts only 0 and 1 (strict).
    #[test]
    fn try_from_u32_valid_variants() {
        assert_eq!(OwnershipKind::try_from_u32(0), Some(OwnershipKind::Shared));
        assert_eq!(
            OwnershipKind::try_from_u32(1),
            Some(OwnershipKind::Exclusive)
        );
    }

    /// Forward-compatible mapper: unknown -> `Shared` (DDS 1.4 default).
    #[test]
    fn from_u32_forward_compatible() {
        assert_eq!(OwnershipKind::from_u32(0), OwnershipKind::Shared);
        assert_eq!(OwnershipKind::from_u32(1), OwnershipKind::Exclusive);
        assert_eq!(OwnershipKind::from_u32(2), OwnershipKind::Shared);
        assert_eq!(OwnershipKind::from_u32(999), OwnershipKind::Shared);
    }

    /// Decode with an unknown kind discriminator → `InvalidEnum` error.
    #[test]
    fn decode_unknown_kind_errors() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(42).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let err = OwnershipQosPolicy::decode_from(&mut r).unwrap_err();
        assert!(matches!(
            err,
            zerodds_cdr::DecodeError::InvalidEnum {
                kind: "OwnershipKind",
                value: 42
            }
        ));
    }

    /// Decode with an empty buffer → short read.
    #[test]
    fn decode_empty_buffer_errors() {
        let bytes: [u8; 0] = [];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(OwnershipQosPolicy::decode_from(&mut r).is_err());
    }
}
