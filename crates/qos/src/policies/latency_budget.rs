// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! LatencyBudgetQosPolicy (DDS 1.4 §2.2.3.10).
//!
//! Wire-Format: Duration (8 byte). Default: `ZERO` (§2.2.3.10.3, "as fast
//! as possible").

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::duration::Duration;

/// LatencyBudgetQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LatencyBudgetQosPolicy {
    /// Duration (default: 0 = so fast as possible).
    pub duration: Duration,
}

impl LatencyBudgetQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.duration.encode_into(w)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            duration: Duration::decode_from(r)?,
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
        assert_eq!(LatencyBudgetQosPolicy::default().duration, Duration::ZERO);
    }

    #[test]
    fn roundtrip() {
        let p = LatencyBudgetQosPolicy {
            duration: Duration::from_millis(50),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(LatencyBudgetQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
