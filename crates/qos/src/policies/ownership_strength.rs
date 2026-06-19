// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OwnershipStrengthQosPolicy (DDS 1.4 §2.2.3.24).
//!
//! Wire format: i32 value (4 bytes). Only relevant for Ownership=Exclusive.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// OwnershipStrengthQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OwnershipStrengthQosPolicy {
    /// Strength-Value. Default 0 (§2.2.3.24.3).
    pub value: i32,
}

impl OwnershipStrengthQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.value as u32)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            value: r.read_u32()? as i32,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn default_is_zero() {
        assert_eq!(OwnershipStrengthQosPolicy::default().value, 0);
    }

    #[test]
    fn roundtrip_negative() {
        let p = OwnershipStrengthQosPolicy { value: -7 };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(OwnershipStrengthQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn roundtrip_large() {
        let p = OwnershipStrengthQosPolicy { value: i32::MAX };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(OwnershipStrengthQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
