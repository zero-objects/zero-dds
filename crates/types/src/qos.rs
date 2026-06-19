// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes-related QoS policies (T17, T18).
//!
//! - [`TypeConsistencyEnforcement`] (§7.6.3.7) — strictness level for
//!   assignability checks.
//! - [`DataRepresentation`] (§7.6.3.2.1) — XCDR1/XCDR2 negotiation.

use alloc::vec::Vec;

/// Kind of the TypeConsistencyEnforcement (§7.6.3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeConsistencyKind {
    /// No type check on matching.
    DisallowTypeCoercion,
    /// Type coercion (int16→int32 etc.) allowed.
    AllowTypeCoercion,
    /// Full type validation + force all checks.
    ForceTypeValidation,
}

impl TypeConsistencyKind {
    /// Wire-encoded as u32 (§7.6.3.7).
    #[must_use]
    pub const fn to_u32(self) -> u32 {
        match self {
            Self::DisallowTypeCoercion => 0,
            Self::AllowTypeCoercion => 1,
            Self::ForceTypeValidation => 2,
        }
    }

    /// Decoder.
    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::AllowTypeCoercion,
            2 => Self::ForceTypeValidation,
            _ => Self::DisallowTypeCoercion,
        }
    }
}

/// TypeConsistencyEnforcement QoS-Policy (§7.6.3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeConsistencyEnforcement {
    /// Kind.
    pub kind: TypeConsistencyKind,
    /// IgnoreSequenceBounds — allows different bound values.
    pub ignore_sequence_bounds: bool,
    /// IgnoreStringBounds.
    pub ignore_string_bounds: bool,
    /// IgnoreMemberNames (match per @id, not per name).
    pub ignore_member_names: bool,
    /// PreventTypeWidening (Narrowing-Ergebnisse blocken).
    pub prevent_type_widening: bool,
    /// ForceTypeValidation.
    pub force_type_validation: bool,
}

impl Default for TypeConsistencyEnforcement {
    fn default() -> Self {
        Self {
            // Default per spec: AllowTypeCoercion (§7.6.3.7.1).
            kind: TypeConsistencyKind::AllowTypeCoercion,
            ignore_sequence_bounds: true,
            ignore_string_bounds: true,
            ignore_member_names: false,
            prevent_type_widening: false,
            force_type_validation: false,
        }
    }
}

impl TypeConsistencyEnforcement {
    /// Encode per XTypes §7.6.3.7.3:
    ///
    /// ```text
    /// struct TypeConsistencyEnforcementQosPolicy {
    ///     TypeConsistencyKind kind;        // u32
    ///     boolean ignore_sequence_bounds;  // 1 byte
    ///     boolean ignore_string_bounds;    // 1 byte
    ///     boolean ignore_member_names;     // 1 byte
    ///     boolean prevent_type_widening;   // 1 byte
    ///     boolean force_type_validation;   // 1 byte
    /// };
    /// ```
    ///
    /// Wire size = 4 + 5 = 9 bytes, with 3 bytes of tail padding to 12-byte
    /// alignment (the PID value is 4-byte-aligned).
    #[must_use]
    pub fn to_bytes_le(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(12);
        out.extend_from_slice(&self.kind.to_u32().to_le_bytes());
        out.push(u8::from(self.ignore_sequence_bounds));
        out.push(u8::from(self.ignore_string_bounds));
        out.push(u8::from(self.ignore_member_names));
        out.push(u8::from(self.prevent_type_widening));
        out.push(u8::from(self.force_type_validation));
        while out.len() % 4 != 0 {
            out.push(0); // padding to the 4-byte boundary.
        }
        out
    }

    /// Decode. Shorter inputs use the default for missing flags
    /// (forward-compat: older peers may send only kind + 3
    /// booleans, analogous to Cyclone DDS ≤ 0.10).
    #[must_use]
    pub fn from_bytes_le(bytes: &[u8]) -> Self {
        if bytes.len() < 4 {
            return Self::default();
        }
        // Infallible: len >= 4 checked above.
        let mut k = [0u8; 4];
        k.copy_from_slice(&bytes[..4]);
        let kind = TypeConsistencyKind::from_u32(u32::from_le_bytes(k));
        Self {
            kind,
            ignore_sequence_bounds: bytes.get(4).copied().unwrap_or(1) != 0,
            ignore_string_bounds: bytes.get(5).copied().unwrap_or(1) != 0,
            ignore_member_names: bytes.get(6).copied().unwrap_or(0) != 0,
            prevent_type_widening: bytes.get(7).copied().unwrap_or(0) != 0,
            force_type_validation: bytes.get(8).copied().unwrap_or(0) != 0,
        }
    }
}

/// Data-Representation-ID (§7.6.3.2.1).
#[repr(i16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRepresentationId {
    /// XCDR version 1 (classic).
    Xcdr1 = 0,
    /// XML representation.
    Xml = 1,
    /// XCDR version 2 (xtypes, default).
    Xcdr2 = 2,
}

impl DataRepresentationId {
    /// Tries to map an i16 to a known representation.
    #[must_use]
    pub const fn from_i16(v: i16) -> Option<Self> {
        match v {
            0 => Some(Self::Xcdr1),
            1 => Some(Self::Xml),
            2 => Some(Self::Xcdr2),
            _ => None,
        }
    }
}

/// Result of a data representation negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepresentationNegotiation {
    /// Common value found.
    Accepted(DataRepresentationId),
    /// No overlap.
    NoOverlap,
}

/// As DCPS §2.2.3 describes: the writer offers representations, the reader
/// accepts a list. Match = the first writer choice present in the reader
/// list.
#[must_use]
pub fn negotiate_representation(
    writer_offered: &[i16],
    reader_accepted: &[i16],
) -> RepresentationNegotiation {
    for w in writer_offered {
        if reader_accepted.contains(w) {
            if let Some(kind) = DataRepresentationId::from_i16(*w) {
                return RepresentationNegotiation::Accepted(kind);
            }
        }
    }
    RepresentationNegotiation::NoOverlap
}

/// Wire constraint matrix (§7.6.3.1 Tab.59): for each DataRepresentation and
/// extensibility category, the spec defines whether the pairing is allowed.
/// XCDR1 supports FINAL and MUTABLE (with PL_CDR1); XCDR2 supports
/// FINAL, APPENDABLE and MUTABLE.
///
/// Returns `Ok(())` if the combination is in the spec matrix;
/// otherwise `Err` with a description.
///
/// # Errors
/// `&'static str` with the violation.
pub fn check_data_repr_extensibility(
    repr: DataRepresentationId,
    ext: ExtensibilityForRepr,
) -> Result<(), &'static str> {
    use DataRepresentationId::*;
    use ExtensibilityForRepr::*;
    match (repr, ext) {
        // XCDR1 allows FINAL + MUTABLE (via PL_CDR1) — APPENDABLE is NOT
        // directly defined (the spec falls back to FINAL handling;
        // the validator here treats it as a violation).
        (Xcdr1, Final) | (Xcdr1, Mutable) => Ok(()),
        (Xcdr1, Appendable) => Err(
            "Tab.59 §7.6.3.1: XCDR1 does not support APPENDABLE directly — the encoder must choose FINAL",
        ),
        // XCDR2 supports all three categories (FINAL, APPENDABLE, MUTABLE).
        (Xcdr2, _) => Ok(()),
        // XML has its own representation, no direct extensibility
        // constraint in the same sense.
        (Xml, _) => Ok(()),
    }
}

/// Extensibility category for the DataRepresentation constraint matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensibilityForRepr {
    /// `@final`.
    Final,
    /// `@appendable`.
    Appendable,
    /// `@mutable`.
    Mutable,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn consistency_roundtrip_default() {
        let c = TypeConsistencyEnforcement::default();
        let bytes = c.to_bytes_le();
        let decoded = TypeConsistencyEnforcement::from_bytes_le(&bytes);
        assert_eq!(decoded, c);
    }

    #[test]
    fn consistency_flags_all_set() {
        let c = TypeConsistencyEnforcement {
            kind: TypeConsistencyKind::ForceTypeValidation,
            ignore_sequence_bounds: false,
            ignore_string_bounds: false,
            ignore_member_names: true,
            prevent_type_widening: true,
            force_type_validation: true,
        };
        let bytes = c.to_bytes_le();
        let decoded = TypeConsistencyEnforcement::from_bytes_le(&bytes);
        assert_eq!(decoded, c);
    }

    #[test]
    fn negotiate_xcdr2_preferred() {
        let result = negotiate_representation(&[2, 0], &[0, 2]);
        assert_eq!(
            result,
            RepresentationNegotiation::Accepted(DataRepresentationId::Xcdr2)
        );
    }

    #[test]
    fn negotiate_fallback_to_xcdr1() {
        let result = negotiate_representation(&[0], &[0, 2]);
        assert_eq!(
            result,
            RepresentationNegotiation::Accepted(DataRepresentationId::Xcdr1)
        );
    }

    #[test]
    fn negotiate_no_overlap() {
        let result = negotiate_representation(&[2], &[0]);
        assert_eq!(result, RepresentationNegotiation::NoOverlap);
    }

    // ---- §7.6.3.1 Tab.59 DataRepresentation/Extensibility-Matrix ----

    #[test]
    fn xcdr1_final_combination_is_allowed() {
        assert!(
            check_data_repr_extensibility(DataRepresentationId::Xcdr1, ExtensibilityForRepr::Final)
                .is_ok()
        );
    }

    #[test]
    fn xcdr1_mutable_combination_is_allowed() {
        // XCDR1 + Mutable via PL_CDR1.
        assert!(
            check_data_repr_extensibility(
                DataRepresentationId::Xcdr1,
                ExtensibilityForRepr::Mutable
            )
            .is_ok()
        );
    }

    #[test]
    fn xcdr1_appendable_is_disallowed() {
        let res = check_data_repr_extensibility(
            DataRepresentationId::Xcdr1,
            ExtensibilityForRepr::Appendable,
        );
        assert!(res.is_err());
    }

    #[test]
    fn xcdr2_supports_all_three_extensibilities() {
        for ext in [
            ExtensibilityForRepr::Final,
            ExtensibilityForRepr::Appendable,
            ExtensibilityForRepr::Mutable,
        ] {
            assert!(
                check_data_repr_extensibility(DataRepresentationId::Xcdr2, ext).is_ok(),
                "XCDR2 + {ext:?} must be allowed"
            );
        }
    }
}
