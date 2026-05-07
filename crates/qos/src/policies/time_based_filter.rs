// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TimeBasedFilterQosPolicy (DDS 1.4 §2.2.3.12).
//!
//! Wire-Format: Duration (8 byte).
//! Default: `ZERO`. Reader-only (Writer kennt den Filter nicht).

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::duration::Duration;

/// TimeBasedFilterQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeBasedFilterQosPolicy {
    /// Minimum-Separation zwischen empfangenen Samples derselben Instance.
    pub minimum_separation: Duration,
}

impl TimeBasedFilterQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.minimum_separation.encode_into(w)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            minimum_separation: Duration::decode_from(r)?,
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
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(TimeBasedFilterQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
