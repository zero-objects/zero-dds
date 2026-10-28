// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TimeBasedFilterQosPolicy (DDS 1.4 §2.2.3.12).
//!
//! Wire-Format: Duration (8 byte).
//! Default: `ZERO`. Reader-only (the writer does not know the filter).

use zerodds_wire::{Reader, WireError, Writer};

use crate::duration::Duration;

/// TimeBasedFilterQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeBasedFilterQosPolicy {
    /// Minimum separation between received samples of the same instance.
    pub minimum_separation: Duration,
}

impl TimeBasedFilterQosPolicy {
    /// Wire-Encoding.
    pub fn encode_into(self, w: &mut Writer) {
        self.minimum_separation.encode_into(w);
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            minimum_separation: Duration::decode_from(r)?,
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
        assert_eq!(
            TimeBasedFilterQosPolicy::default().minimum_separation,
            Duration::ZERO
        );
    }

    #[test]
    fn roundtrip() {
        let p = TimeBasedFilterQosPolicy {
            minimum_separation: Duration::from_millis(100),
        };
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(TimeBasedFilterQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
