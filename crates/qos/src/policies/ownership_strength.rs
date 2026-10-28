// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OwnershipStrengthQosPolicy (DDS 1.4 §2.2.3.24).
//!
//! Wire format: i32 value (4 bytes). Only relevant for Ownership=Exclusive.

use zerodds_wire::{Reader, WireError, Writer};

/// OwnershipStrengthQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OwnershipStrengthQosPolicy {
    /// Strength-Value. Default 0 (§2.2.3.24.3).
    pub value: i32,
}

impl OwnershipStrengthQosPolicy {
    /// Wire-Encoding.
    pub fn encode_into(self, w: &mut Writer) {
        w.write_u32(self.value as u32);
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            value: r.read_u32()? as i32,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_wire::Endianness;

    #[test]
    fn default_is_zero() {
        assert_eq!(OwnershipStrengthQosPolicy::default().value, 0);
    }

    #[test]
    fn roundtrip_negative() {
        let p = OwnershipStrengthQosPolicy { value: -7 };
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(OwnershipStrengthQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn roundtrip_large() {
        let p = OwnershipStrengthQosPolicy { value: i32::MAX };
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(OwnershipStrengthQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
