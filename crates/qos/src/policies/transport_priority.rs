// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TransportPriorityQosPolicy (DDS 1.4 §2.2.3.15).
//!
//! Wire format: i32 value (4 bytes). A hint for transport; no
//! match effect.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// TransportPriorityQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransportPriorityQosPolicy {
    /// Priority-Value. Default 0.
    pub value: i32,
}

impl TransportPriorityQosPolicy {
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
        assert_eq!(TransportPriorityQosPolicy::default().value, 0);
    }

    #[test]
    fn roundtrip() {
        let p = TransportPriorityQosPolicy { value: 42 };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(TransportPriorityQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
