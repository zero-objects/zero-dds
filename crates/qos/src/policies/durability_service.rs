// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DurabilityServiceQosPolicy (DDS 1.4 §2.2.3.5).
//!
//! Wire-Format: Duration (8) + u32 history_kind (4) + i32 history_depth (4)
//! + 3 × i32 resource_limits (12) = 28 byte.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::duration::Duration;
use crate::policies::history::HistoryKind;
use crate::policies::resource_limits::LENGTH_UNLIMITED;

/// DurabilityServiceQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurabilityServiceQosPolicy {
    /// Cleanup-Delay.
    pub service_cleanup_delay: Duration,
    /// History-Kind.
    pub history_kind: HistoryKind,
    /// History-Depth.
    pub history_depth: i32,
    /// Max-Samples.
    pub max_samples: i32,
    /// Max-Instances.
    pub max_instances: i32,
    /// Max-Samples per Instance.
    pub max_samples_per_instance: i32,
}

impl Default for DurabilityServiceQosPolicy {
    fn default() -> Self {
        Self {
            service_cleanup_delay: Duration::ZERO,
            history_kind: HistoryKind::KeepLast,
            history_depth: 1,
            max_samples: LENGTH_UNLIMITED,
            max_instances: LENGTH_UNLIMITED,
            max_samples_per_instance: LENGTH_UNLIMITED,
        }
    }
}

impl DurabilityServiceQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.service_cleanup_delay.encode_into(w)?;
        w.write_u32(self.history_kind as u32)?;
        w.write_u32(self.history_depth as u32)?;
        w.write_u32(self.max_samples as u32)?;
        w.write_u32(self.max_instances as u32)?;
        w.write_u32(self.max_samples_per_instance as u32)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let service_cleanup_delay = Duration::decode_from(r)?;
        let v = r.read_u32()?;
        let history_kind = HistoryKind::try_from_u32(v).ok_or(DecodeError::InvalidEnum {
            kind: "HistoryKind",
            value: v,
        })?;
        let history_depth = r.read_u32()? as i32;
        let max_samples = r.read_u32()? as i32;
        let max_instances = r.read_u32()? as i32;
        let max_samples_per_instance = r.read_u32()? as i32;
        Ok(Self {
            service_cleanup_delay,
            history_kind,
            history_depth,
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
    fn default_matches_spec() {
        let d = DurabilityServiceQosPolicy::default();
        assert_eq!(d.service_cleanup_delay, Duration::ZERO);
        assert_eq!(d.history_kind, HistoryKind::KeepLast);
        assert_eq!(d.history_depth, 1);
        assert_eq!(d.max_samples, LENGTH_UNLIMITED);
    }

    #[test]
    fn roundtrip() {
        let p = DurabilityServiceQosPolicy {
            service_cleanup_delay: Duration::from_secs(60),
            history_kind: HistoryKind::KeepAll,
            history_depth: -1,
            max_samples: 100,
            max_instances: 10,
            max_samples_per_instance: 5,
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 28);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(DurabilityServiceQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
