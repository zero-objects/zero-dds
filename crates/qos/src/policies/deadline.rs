// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DeadlineQosPolicy (DDS 1.4 §2.2.3.7).
//!
//! Wire-Format (DDSI-RTPS §9.6.3.2): Duration (8 byte).

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

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
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.period.encode_into(w)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            period: Duration::decode_from(r)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn default_is_infinite() {
        assert!(DeadlineQosPolicy::default().period.is_infinite());
    }

    #[test]
    fn roundtrip() {
        let p = DeadlineQosPolicy {
            period: Duration::from_millis(500),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(DeadlineQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
