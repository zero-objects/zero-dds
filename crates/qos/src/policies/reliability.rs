// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ReliabilityQosPolicy (DDS 1.4 §2.2.3.14).
//!
//! Wire-Format (DDSI-RTPS §9.6.3.2): u32 kind + Duration max_blocking_time
//! = 4 + 8 = 12 byte.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::duration::Duration;

/// Reliability kind (DDS 1.4 §2.2.3.14).
///
/// Order per §2.2.3 Table "QoS compatibility":
/// `BestEffort < Reliable`. `offered.kind >= requested.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReliabilityKind {
    /// BestEffort (DDSI-RTPS §8.4.2).
    BestEffort = 1,
    /// Reliable (with HB+ACKNACK flow).
    Reliable = 2,
}

impl Default for ReliabilityKind {
    /// DDS 1.4 §2.2.3.14.3: default depends on the entity — reader: BestEffort,
    /// writer: Reliable. We pick the reader default here; the writer sets
    /// `ReliabilityKind::Reliable` explicitly.
    fn default() -> Self {
        Self::BestEffort
    }
}

impl ReliabilityKind {
    /// Strict mapper.
    #[must_use]
    pub const fn try_from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::BestEffort),
            2 => Some(Self::Reliable),
            _ => None,
        }
    }

    /// Forward-compatible mapper (unknown → BestEffort).
    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match v {
            2 => Self::Reliable,
            _ => Self::BestEffort,
        }
    }
}

/// ReliabilityQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReliabilityQosPolicy {
    /// Kind.
    pub kind: ReliabilityKind,
    /// Max blocking time — **writer-only semantic**. On the reader side
    /// the value is serialized (spec wire format) but ignored;
    /// DDS readers need not interpret the peer value. Cyclone and
    /// Fast-DDS behave the same — byte-wise identical, semantically
    /// a reader no-op.
    pub max_blocking_time: Duration,
}

impl Default for ReliabilityQosPolicy {
    fn default() -> Self {
        Self {
            kind: ReliabilityKind::BestEffort,
            // Spec default for a Reliable writer: 100ms.
            max_blocking_time: Duration::from_millis(100),
        }
    }
}

impl ReliabilityQosPolicy {
    /// Wire encoding: u32 kind + i32 seconds + u32 fraction = 12 bytes.
    ///
    /// # Errors
    /// Buffer overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.kind as u32)?;
        self.max_blocking_time.encode_into(w)
    }

    /// Wire decoding (strict). Unknown discriminator → `InvalidEnum`.
    ///
    /// # Errors
    /// Buffer underflow or unknown kind value.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let v = r.read_u32()?;
        let kind = ReliabilityKind::try_from_u32(v).ok_or(DecodeError::InvalidEnum {
            kind: "ReliabilityKind",
            value: v,
        })?;
        let max_blocking_time = Duration::decode_from(r)?;
        Ok(Self {
            kind,
            max_blocking_time,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn best_effort_lt_reliable() {
        assert!(ReliabilityKind::BestEffort < ReliabilityKind::Reliable);
    }

    #[test]
    fn try_from_u32_is_strict() {
        assert_eq!(ReliabilityKind::try_from_u32(0), None);
        assert_eq!(
            ReliabilityKind::try_from_u32(1),
            Some(ReliabilityKind::BestEffort)
        );
        assert_eq!(
            ReliabilityKind::try_from_u32(2),
            Some(ReliabilityKind::Reliable)
        );
        assert_eq!(ReliabilityKind::try_from_u32(3), None);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let p = ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: Duration::from_millis(250),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 12);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(ReliabilityQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn default_is_best_effort_100ms() {
        let d = ReliabilityQosPolicy::default();
        assert_eq!(d.kind, ReliabilityKind::BestEffort);
        assert_eq!(d.max_blocking_time, Duration::from_millis(100));
    }

    /// Forward-compatible mapper: unknown -> `BestEffort` (defensive).
    #[test]
    fn from_u32_forward_compatible() {
        assert_eq!(ReliabilityKind::from_u32(1), ReliabilityKind::BestEffort);
        assert_eq!(ReliabilityKind::from_u32(2), ReliabilityKind::Reliable);
        // 0 is NOT Best-Effort at the wire level (DDSI-RTPS uses 1/2),
        // but forward-compat collapses it to BestEffort.
        assert_eq!(ReliabilityKind::from_u32(0), ReliabilityKind::BestEffort);
        assert_eq!(ReliabilityKind::from_u32(99), ReliabilityKind::BestEffort);
    }

    /// Roundtrip of the BestEffort variant incl. non-default
    /// max_blocking_time (report: variant previously untested).
    #[test]
    fn best_effort_roundtrip_with_custom_blocking() {
        let p = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            max_blocking_time: Duration::from_millis(2500),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 12);
        // Wire-format spec check: first u32 is kind=1 (little-endian).
        assert_eq!(&bytes[0..4], &[1u8, 0, 0, 0]);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(ReliabilityQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    /// Debug format contains both variants and the duration.
    #[test]
    fn debug_and_clone_work() {
        let p = ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: Duration::from_secs(1),
        };
        #[allow(clippy::clone_on_copy)]
        let p2 = p.clone();
        assert_eq!(p, p2);
        let dbg = alloc::format!("{p:?}");
        assert!(dbg.contains("Reliable"), "debug: {dbg}");
    }

    /// Decode with an unknown kind discriminator → `InvalidEnum` error.
    #[test]
    fn decode_unknown_kind_errors() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(5).unwrap();
        Duration::ZERO.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let err = ReliabilityQosPolicy::decode_from(&mut r).unwrap_err();
        assert!(matches!(
            err,
            zerodds_cdr::DecodeError::InvalidEnum {
                kind: "ReliabilityKind",
                value: 5
            }
        ));
    }

    /// Decode with a short buffer (kind=Reliable, no blocking-time) →
    /// short-read error on the duration decode.
    #[test]
    fn decode_short_buffer_no_duration_errors() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(2).unwrap(); // kind=Reliable
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(ReliabilityQosPolicy::decode_from(&mut r).is_err());
    }

    /// Spec ordering: BestEffort < Reliable → `offered >= requested`
    /// compatibility logic builds on this.
    #[test]
    fn ordering_reliable_higher_than_besteffort() {
        assert!(ReliabilityKind::Reliable > ReliabilityKind::BestEffort);
    }
}
