// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! LatencyBudgetQosPolicy (DDS 1.4 §2.2.3.10).
//!
//! Wire-Format: Duration (8 byte). Default: `ZERO` (§2.2.3.10.3, "as fast
//! as possible").

use zerodds_wire::{Reader, WireError, Writer};

use crate::duration::Duration;

/// LatencyBudgetQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LatencyBudgetQosPolicy {
    /// Duration (default: 0 = so fast as possible).
    pub duration: Duration,
}

impl LatencyBudgetQosPolicy {
    /// Wire-Encoding.
    pub fn encode_into(self, w: &mut Writer) {
        self.duration.encode_into(w);
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            duration: Duration::decode_from(r)?,
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
        assert_eq!(LatencyBudgetQosPolicy::default().duration, Duration::ZERO);
    }

    #[test]
    fn roundtrip() {
        let p = LatencyBudgetQosPolicy {
            duration: Duration::from_millis(50),
        };
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(LatencyBudgetQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
