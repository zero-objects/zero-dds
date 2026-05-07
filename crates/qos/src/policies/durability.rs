// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DurabilityQosPolicy (DDS 1.4 §2.2.3.4).
//!
//! Wire-Format (DDSI-RTPS §9.6.3.2): u32 kind (4 byte).

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// Durability-Kind (DDS 1.4 §2.2.3.4).
///
/// Reihenfolge entspricht der Ordering-Relation der Compatibility-Regel:
/// `VOLATILE < TRANSIENT_LOCAL < TRANSIENT < PERSISTENT`. Spec §2.2.3
/// Table "QoS compatibility" verlangt `offered.kind >= requested.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u32)]
pub enum DurabilityKind {
    /// Samples sind fluechtig; Late-Joiners sehen nichts.
    #[default]
    Volatile = 0,
    /// Writer-Prozess haelt Samples fuer Late-Joiners.
    TransientLocal = 1,
    /// Durability-Service haelt Samples ueber Writer-Lifetime.
    Transient = 2,
    /// Persistent Storage.
    Persistent = 3,
}

impl DurabilityKind {
    /// Forward-kompatibler Mapper: unbekannte Werte werden auf `Volatile`
    /// abgebildet (default). Der stricte Mapper [`Self::try_from_u32`] gibt
    /// bei unbekannten Werten `None` zurueck.
    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::TransientLocal,
            2 => Self::Transient,
            3 => Self::Persistent,
            _ => Self::Volatile,
        }
    }

    /// Strikter Mapper (return None bei unbekanntem Wert).
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
    /// Wire-Encoding: u32 kind.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.kind as u32)
    }

    /// Wire-Decoding (strict). Unbekannter Discriminator liefert
    /// `DecodeError::InvalidEnum` — QoS-Matching darf nicht auf still
    /// gedowngradeten Werten basieren.
    ///
    /// # Errors
    /// Buffer-Underflow oder unbekannter Kind-Wert.
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
        // Strict decode: u32=99 ist Spec-Verletzung → InvalidEnum.
        // Wichtig: QoS-Matching darf nicht auf default-gedowngradeten
        // Werten basieren.
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
        // requested.kind. Pruefen via PartialOrd auf den Enum-Werten.
        use DurabilityKind::*;
        // Voller Match: gleiche Kind → kompatibel.
        assert!(Volatile >= Volatile);
        // Offered TransientLocal kompatibel mit requested Volatile.
        assert!(TransientLocal >= Volatile);
        // Offered Persistent kompatibel mit requested TransientLocal.
        assert!(Persistent >= TransientLocal);
        // Negativ: offered Volatile NICHT kompatibel mit
        // requested TransientLocal.
        assert!(Volatile < TransientLocal);
    }
}
