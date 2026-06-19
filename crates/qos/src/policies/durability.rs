// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DurabilityQosPolicy (DDS 1.4 §2.2.3.4).
//!
//! Wire format (DDSI-RTPS §9.6.3.2): u32 kind (4 bytes).

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// Durability kind (DDS 1.4 §2.2.3.4).
///
/// The order matches the ordering relation of the compatibility rule:
/// `VOLATILE < TRANSIENT_LOCAL < TRANSIENT < PERSISTENT`. Spec §2.2.3
/// Table "QoS compatibility" requires `offered.kind >= requested.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u32)]
pub enum DurabilityKind {
    /// Samples are volatile; late joiners see nothing.
    #[default]
    Volatile = 0,
    /// The writer process holds samples for late joiners.
    TransientLocal = 1,
    /// A durability service holds samples beyond the writer's lifetime.
    Transient = 2,
    /// Persistent storage.
    Persistent = 3,
}

impl DurabilityKind {
    /// Forward-compatible mapper: unknown values are mapped to `Volatile`
    /// (default). The strict mapper [`Self::try_from_u32`] returns
    /// `None` for unknown values.
    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::TransientLocal,
            2 => Self::Transient,
            3 => Self::Persistent,
            _ => Self::Volatile,
        }
    }

    /// Strict mapper (returns None for an unknown value).
    #[must_use]
    pub const fn try_from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Volatile),
            1 => Some(Self::TransientLocal),
            2 => Some(Self::Transient),
            3 => Some(Self::Persistent),
            _ => None,
        }
    }
}

/// `DurabilityQosPolicy` (§2.2.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DurabilityQosPolicy {
    /// Kind.
    pub kind: DurabilityKind,
}

impl DurabilityQosPolicy {
    /// Wire encoding: u32 kind.
    ///
    /// # Errors
    /// Buffer overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.kind as u32)
    }

    /// Wire decoding (strict). An unknown discriminator returns
    /// `DecodeError::InvalidEnum` — QoS matching must not be based on
    /// silently downgraded values.
    ///
    /// # Errors
    /// Buffer underflow or unknown kind value.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let v = r.read_u32()?;
        let kind = DurabilityKind::try_from_u32(v).ok_or(DecodeError::InvalidEnum {
            kind: "DurabilityKind",
            value: v,
        })?;
        Ok(Self { kind })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn default_is_volatile() {
        assert_eq!(
            DurabilityQosPolicy::default().kind,
            DurabilityKind::Volatile
        );
    }

    #[test]
    fn ordering_matches_spec() {
        use DurabilityKind::*;
        assert!(Volatile < TransientLocal);
        assert!(TransientLocal < Transient);
        assert!(Transient < Persistent);
    }

    #[test]
    fn try_from_u32_is_strict() {
        assert_eq!(
            DurabilityKind::try_from_u32(0),
            Some(DurabilityKind::Volatile)
        );
        assert_eq!(
            DurabilityKind::try_from_u32(3),
            Some(DurabilityKind::Persistent)
        );
        assert_eq!(DurabilityKind::try_from_u32(4), None);
    }

    #[test]
    fn from_u32_is_forward_compat() {
        assert_eq!(DurabilityKind::from_u32(99), DurabilityKind::Volatile);
    }

    #[test]
    fn encode_decode_roundtrip() {
        for kind in [
            DurabilityKind::Volatile,
            DurabilityKind::TransientLocal,
            DurabilityKind::Transient,
            DurabilityKind::Persistent,
        ] {
            let p = DurabilityQosPolicy { kind };
            let mut w = BufferWriter::new(Endianness::Little);
            p.encode_into(&mut w).unwrap();
            let bytes = w.into_bytes();
            assert_eq!(bytes.len(), 4);
            let mut r = BufferReader::new(&bytes, Endianness::Little);
            let back = DurabilityQosPolicy::decode_from(&mut r).unwrap();
            assert_eq!(back, p);
        }
    }

    #[test]
    fn unknown_kind_decode_rejected() {
        // Strict decode: u32=99 is a spec violation → InvalidEnum.
        // Important: QoS matching must not be based on default-downgraded
        // values.
        let bytes = [99u8, 0, 0, 0];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = DurabilityQosPolicy::decode_from(&mut r);
        assert!(matches!(
            res,
            Err(DecodeError::InvalidEnum {
                kind: "DurabilityKind",
                ..
            })
        ));
    }

    #[test]
    fn compatibility_offered_ge_requested() {
        // Spec §2.2.3 Tab. "QoS compatibility" — offered.kind >=
        // requested.kind. Check via PartialOrd on the enum values.
        use DurabilityKind::*;
        // Full match: same kind → compatible.
        assert!(Volatile >= Volatile);
        // Offered TransientLocal compatible with requested Volatile.
        assert!(TransientLocal >= Volatile);
        // Offered Persistent compatible with requested TransientLocal.
        assert!(Persistent >= TransientLocal);
        // Negative: offered Volatile NOT compatible with
        // requested TransientLocal.
        assert!(Volatile < TransientLocal);
    }
}
