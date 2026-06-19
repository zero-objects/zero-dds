// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! LivelinessQosPolicy (DDS 1.4 §2.2.3.11).
//!
//! Wire-Format (DDSI-RTPS §9.6.3.2): u32 kind + Duration lease = 12 byte.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::duration::Duration;

/// Liveliness-Kind.
///
/// Ordering per §2.2.3 Table "QoS compatibility":
/// `AUTOMATIC < MANUAL_BY_PARTICIPANT < MANUAL_BY_TOPIC`.
/// `offered.kind >= requested.kind` for compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u32)]
pub enum LivelinessKind {
    /// DDS stack asserts liveliness automatically (default).
    #[default]
    Automatic = 0,
    /// Participant assertion propagates to all writers.
    ManualByParticipant = 1,
    /// Writer-granulare manuelle Assertion.
    ManualByTopic = 2,
}

impl LivelinessKind {
    /// Strikter Mapper.
    #[must_use]
    pub const fn try_from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Automatic),
            1 => Some(Self::ManualByParticipant),
            2 => Some(Self::ManualByTopic),
            _ => None,
        }
    }

    /// Forward-compatible mapper (unknown → Automatic).
    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::ManualByParticipant,
            2 => Self::ManualByTopic,
            _ => Self::Automatic,
        }
    }
}

/// LivelinessQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivelinessQosPolicy {
    /// Kind.
    pub kind: LivelinessKind,
    /// Lease-Duration.
    pub lease_duration: Duration,
}

impl Default for LivelinessQosPolicy {
    fn default() -> Self {
        Self {
            kind: LivelinessKind::Automatic,
            lease_duration: Duration::INFINITE,
        }
    }
}

impl LivelinessQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.kind as u32)?;
        self.lease_duration.encode_into(w)
    }

    /// Wire-Decoding (strict).
    ///
    /// # Errors
    /// Buffer underflow or unknown kind value.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let v = r.read_u32()?;
        let kind = LivelinessKind::try_from_u32(v).ok_or(DecodeError::InvalidEnum {
            kind: "LivelinessKind",
            value: v,
        })?;
        let lease_duration = Duration::decode_from(r)?;
        Ok(Self {
            kind,
            lease_duration,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn default_automatic_infinite() {
        let d = LivelinessQosPolicy::default();
        assert_eq!(d.kind, LivelinessKind::Automatic);
        assert!(d.lease_duration.is_infinite());
    }

    #[test]
    fn kind_ordering() {
        use LivelinessKind::*;
        assert!(Automatic < ManualByParticipant);
        assert!(ManualByParticipant < ManualByTopic);
    }

    #[test]
    fn try_from_u32_strict() {
        assert_eq!(LivelinessKind::try_from_u32(3), None);
        assert_eq!(
            LivelinessKind::try_from_u32(2),
            Some(LivelinessKind::ManualByTopic)
        );
    }

    #[test]
    fn roundtrip() {
        for kind in [
            LivelinessKind::Automatic,
            LivelinessKind::ManualByParticipant,
            LivelinessKind::ManualByTopic,
        ] {
            let p = LivelinessQosPolicy {
                kind,
                lease_duration: Duration::from_millis(1234),
            };
            let mut w = BufferWriter::new(Endianness::Little);
            p.encode_into(&mut w).unwrap();
            let bytes = w.into_bytes();
            assert_eq!(bytes.len(), 12);
            let mut r = BufferReader::new(&bytes, Endianness::Little);
            assert_eq!(LivelinessQosPolicy::decode_from(&mut r).unwrap(), p);
        }
    }

    /// Forward-compatible mapper: unknown -> `Automatic`.
    #[test]
    fn from_u32_forward_compatible() {
        assert_eq!(LivelinessKind::from_u32(0), LivelinessKind::Automatic);
        assert_eq!(
            LivelinessKind::from_u32(1),
            LivelinessKind::ManualByParticipant
        );
        assert_eq!(LivelinessKind::from_u32(2), LivelinessKind::ManualByTopic);
        assert_eq!(LivelinessKind::from_u32(3), LivelinessKind::Automatic);
        assert_eq!(
            LivelinessKind::from_u32(u32::MAX),
            LivelinessKind::Automatic
        );
    }

    /// Decode with an unknown kind discriminator → `InvalidEnum` error.
    #[test]
    fn decode_unknown_kind_errors() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(7).unwrap();
        // valid lease, irrelevant since kind-validate failt first.
        Duration::from_millis(10).encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let err = LivelinessQosPolicy::decode_from(&mut r).unwrap_err();
        assert!(matches!(
            err,
            zerodds_cdr::DecodeError::InvalidEnum {
                kind: "LivelinessKind",
                value: 7
            }
        ));
    }

    /// Decode with a short buffer (kind ok, lease missing) → short read.
    #[test]
    fn decode_short_buffer_no_lease_errors() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(0).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(LivelinessQosPolicy::decode_from(&mut r).is_err());
    }

    /// Default is `Automatic` with an `INFINITE` lease (§2.2.3.11.4).
    #[test]
    fn default_debug_contains_infinite() {
        let d = LivelinessQosPolicy::default();
        assert!(d.lease_duration.is_infinite());
        let dbg = alloc::format!("{d:?}");
        assert!(dbg.contains("Automatic"), "debug: {dbg}");
    }

    /// Copy semantics for policy and kind.
    #[test]
    fn policy_is_copy() {
        let p = LivelinessQosPolicy::default();
        let q = p;
        assert_eq!(p, q);
    }
}
