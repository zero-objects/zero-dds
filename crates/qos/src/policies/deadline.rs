// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DeadlineQosPolicy (DDS 1.4 §2.2.3.7).
//!
//! Wire-Format (DDSI-RTPS §9.6.3.2): Duration (8 byte).

use zerodds_wire::{Reader, WireError, Writer};

use crate::duration::Duration;

/// DeadlineQosPolicy.
///
/// Default: `DURATION_INFINITE` (§2.2.3.7.3) — no deadline monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadlineQosPolicy {
    /// Period.
    pub period: Duration,
}

impl Default for DeadlineQosPolicy {
    fn default() -> Self {
        Self {
            period: Duration::INFINITE,
        }
    }
}

impl DeadlineQosPolicy {
    /// Wire-Encoding.
    pub fn encode_into(self, w: &mut Writer) {
        self.period.encode_into(w);
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            period: Duration::decode_from(r)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_wire::Endianness;

    #[test]
    fn default_is_infinite() {
        assert!(DeadlineQosPolicy::default().period.is_infinite());
    }

    #[test]
    fn roundtrip() {
        let p = DeadlineQosPolicy {
            period: Duration::from_millis(500),
        };
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(DeadlineQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
