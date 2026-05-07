// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! LifespanQosPolicy (DDS 1.4 §2.2.3.16).
//!
//! Wire-Format: Duration (8 byte). Default: `INFINITE`.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::duration::Duration;

/// LifespanQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifespanQosPolicy {
    /// Duration.
    pub duration: Duration,
}

impl Default for LifespanQosPolicy {
    fn default() -> Self {
        Self {
            duration: Duration::INFINITE,
        }
    }
}

impl LifespanQosPolicy {
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
    fn default_is_infinite() {
        assert!(LifespanQosPolicy::default().duration.is_infinite());
    }

    #[test]
    fn roundtrip() {
        let p = LifespanQosPolicy {
            duration: Duration::from_secs(30),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(LifespanQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
