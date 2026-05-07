// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ResourceLimitsQosPolicy (DDS 1.4 §2.2.3.19).
//!
//! Wire-Format (DDSI-RTPS §9.6.3.2): 3 × i32 = 12 byte.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// `LENGTH_UNLIMITED` Konstant (DDS 1.4 §2.3.3.19.1 / IDL4 `-1`).
pub const LENGTH_UNLIMITED: i32 = -1;

/// ResourceLimitsQosPolicy.
///
/// `LENGTH_UNLIMITED` (= -1) signalisiert keine harte Grenze.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimitsQosPolicy {
    /// Max-Samples.
    pub max_samples: i32,
    /// Max-Instances.
    pub max_instances: i32,
    /// Max-Samples per Instance.
    pub max_samples_per_instance: i32,
}

impl Default for ResourceLimitsQosPolicy {
    fn default() -> Self {
        Self {
            max_samples: LENGTH_UNLIMITED,
            max_instances: LENGTH_UNLIMITED,
            max_samples_per_instance: LENGTH_UNLIMITED,
        }
    }
}

impl ResourceLimitsQosPolicy {
    /// Validiert die konsistenten Bedingungen laut §2.2.3.19.3:
    /// `max_samples_per_instance <= max_samples` (falls beide gesetzt).
    /// Gibt `false` bei Verletzung, sonst `true`.
    #[must_use]
    pub fn is_consistent(self) -> bool {
        if self.max_samples == LENGTH_UNLIMITED || self.max_samples_per_instance == LENGTH_UNLIMITED
        {
            true
        } else {
            self.max_samples_per_instance <= self.max_samples
        }
    }

    /// Wire-Encoding.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.max_samples as u32)?;
        w.write_u32(self.max_instances as u32)?;
        w.write_u32(self.max_samples_per_instance as u32)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let max_samples = r.read_u32()? as i32;
        let max_instances = r.read_u32()? as i32;
        let max_samples_per_instance = r.read_u32()? as i32;
        Ok(Self {
            max_samples,
            max_instances,
            max_samples_per_instance,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn default_is_unlimited() {
        let d = ResourceLimitsQosPolicy::default();
        assert_eq!(d.max_samples, LENGTH_UNLIMITED);
        assert!(d.is_consistent());
    }

    #[test]
    fn consistency_requires_per_instance_le_total() {
        let bad = ResourceLimitsQosPolicy {
            max_samples: 100,
            max_instances: 10,
            max_samples_per_instance: 200,
        };
        assert!(!bad.is_consistent());

        let good = ResourceLimitsQosPolicy {
            max_samples: 200,
            max_instances: 10,
            max_samples_per_instance: 100,
        };
        assert!(good.is_consistent());
    }

    #[test]
    fn unlimited_passes_consistency() {
        let p = ResourceLimitsQosPolicy {
            max_samples: LENGTH_UNLIMITED,
            max_instances: 10,
            max_samples_per_instance: 1_000_000,
        };
        assert!(p.is_consistent());
    }

    #[test]
    fn encode_decode_roundtrip() {
        let p = ResourceLimitsQosPolicy {
            max_samples: 100,
            max_instances: 20,
            max_samples_per_instance: 5,
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 12);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(ResourceLimitsQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn encode_decode_negative_one() {
        let p = ResourceLimitsQosPolicy::default();
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(ResourceLimitsQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
