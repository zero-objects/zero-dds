// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! LifespanQosPolicy (DDS 1.4 §2.2.3.16).
//!
//! Wire-Format: Duration (8 byte). Default: `INFINITE`.

use zerodds_wire::{Reader, WireError, Writer};

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
    fn default_is_infinite() {
        assert!(LifespanQosPolicy::default().duration.is_infinite());
    }

    #[test]
    fn roundtrip() {
        let p = LifespanQosPolicy {
            duration: Duration::from_secs(30),
        };
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(LifespanQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
