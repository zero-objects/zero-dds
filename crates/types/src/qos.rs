// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes-related QoS policies (T17, T18).
//!
//! - [`TypeConsistencyEnforcement`] (DDS-XTypes 1.3 §7.6.3.4) —
//!   strictness level for assignability checks.
//! - [`DataRepresentationId`] (§7.6.3.1) — XCDR1/XCDR2 negotiation.

use alloc::vec::Vec;

/// Kind of the TypeConsistencyEnforcement (DDS-XTypes 1.3 §7.6.3.4.1,
/// Annex D `TypeConsistencyKind`). The specification defines exactly two
/// values; `force_type_validation` is a field of the policy of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeConsistencyKind {
    /// `DISALLOW_TYPE_COERCION`: writer and reader must support the same
    /// data type.
    DisallowTypeCoercion,
    /// `ALLOW_TYPE_COERCION`: the reader's type may be assignable from
    /// the writer's type without being the same.
    AllowTypeCoercion,
}

/// A `TypeConsistencyKind` value that Annex D does not define.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownTypeConsistencyKind(pub u32);

impl core::fmt::Display for UnknownTypeConsistencyKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TypeConsistencyKind {} is not DISALLOW_TYPE_COERCION (0) or ALLOW_TYPE_COERCION (1)",
            self.0
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UnknownTypeConsistencyKind {}

impl TypeConsistencyKind {
    /// Wire value of Annex D (`DISALLOW_TYPE_COERCION` = 0,
    /// `ALLOW_TYPE_COERCION` = 1).
    #[must_use]
    pub const fn to_u32(self) -> u32 {
        match self {
            Self::DisallowTypeCoercion => 0,
            Self::AllowTypeCoercion => 1,
        }
    }

    /// Encode as the `unsigned short` XTypes 1.3 §7.6.3.4.3 declares.
    #[must_use]
    pub const fn to_u16(self) -> u16 {
        self.to_u32() as u16
    }

    /// Decode a wire value.
    ///
    /// # Errors
    /// [`UnknownTypeConsistencyKind`] for a value Annex D does not define.
    pub const fn from_u32(v: u32) -> Result<Self, UnknownTypeConsistencyKind> {
        match v {
            0 => Ok(Self::DisallowTypeCoercion),
            1 => Ok(Self::AllowTypeCoercion),
            other => Err(UnknownTypeConsistencyKind(other)),
        }
    }
}

/// TypeConsistencyEnforcement QoS-Policy (DDS-XTypes 1.3 §7.6.3.4.1).
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
    /// PreventTypeWidening (block narrowing results).
    pub prevent_type_widening: bool,
    /// `force_type_validation`: both sides must announce a TypeObject
    /// (§7.6.3.4.1).
    pub force_type_validation: bool,
}

impl Default for TypeConsistencyEnforcement {
    fn default() -> Self {
        Self {
            // Default of the policy when it is present (§7.6.3.4.1).
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
    /// The policy of an endpoint whose announcement carries none.
    ///
    /// DDS-XTypes 1.3 §7.6.3.4.1: when the Service observes in the
    /// built-in topic data of a remote endpoint "that no
    /// TypeConsistencyEnforcementQosPolicy value is provided […] it shall
    /// assume a kind of DISALLOW_TYPE_COERCION". The four options then
    /// "are treated as though prevent_type_widening is set to true and the
    /// others are set to false"; `force_type_validation` keeps its default.
    #[must_use]
    pub const fn not_announced() -> Self {
        Self {
            kind: TypeConsistencyKind::DisallowTypeCoercion,
            ignore_sequence_bounds: false,
            ignore_string_bounds: false,
            ignore_member_names: false,
            prevent_type_widening: true,
            force_type_validation: false,
        }
    }

    /// Encode per XTypes 1.3 §7.6.3.4.3:
    ///
    /// ```text
    /// struct TypeConsistencyEnforcementQosPolicy {
    ///     TypeConsistencyKind kind;        // unsigned short, 2 bytes
    ///     boolean ignore_sequence_bounds;  // 1 byte
    ///     boolean ignore_string_bounds;    // 1 byte
    ///     boolean ignore_member_names;     // 1 byte
    ///     boolean prevent_type_widening;   // 1 byte
    ///     boolean force_type_validation;   // 1 byte
    /// };
    /// ```
    ///
    /// `TypeConsistencyKind` is an `unsigned short` and not a `long`, so
    /// the body is 7 octets and PID_TYPE_CONSISTENCY_ENFORCEMENT (0x0074)
    /// carries it in a parameter of length 8.
    ///
    /// Measured rather than deduced. The recorded SEDP subscriptions of
    /// two vendors — `crates/discovery/tests/fixtures/vendor-discovery/`,
    /// RTI Connext 7.7.0 and Fast DDS — both write PID 0x0074 with
    /// length 8 and the identical body
    ///
    /// ```text
    /// 01 00  01 01 00 00 00  00
    /// kind   seq str mem wid val  pad
    /// ```
    ///
    /// which is `ALLOW_TYPE_COERCION` and the five specified defaults of
    /// §7.6.3.4.1. Reading `kind` as a `u32` — which this function did
    /// until the PID was wired up — takes `01 00 01 01` off those bytes
    /// and lands on `DisallowTypeCoercion` with the booleans shifted by
    /// two.
    #[must_use]
    pub fn to_bytes_le(self) -> Vec<u8> {
        self.to_bytes(true)
    }

    /// Encode in the given byte order. See [`Self::to_bytes_le`].
    #[must_use]
    pub fn to_bytes(self, little_endian: bool) -> Vec<u8> {
        let kind = self.kind.to_u16();
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&if little_endian {
            kind.to_le_bytes()
        } else {
            kind.to_be_bytes()
        });
        out.push(u8::from(self.ignore_sequence_bounds));
        out.push(u8::from(self.ignore_string_bounds));
        out.push(u8::from(self.ignore_member_names));
        out.push(u8::from(self.prevent_type_widening));
        out.push(u8::from(self.force_type_validation));
        out.push(0); // padding to the 4-byte boundary.
        out
    }

    /// Decode, little-endian. See [`Self::from_bytes`].
    #[must_use]
    pub fn from_bytes_le(bytes: &[u8]) -> Self {
        Self::from_bytes(bytes, true)
    }

    /// Decode in the given byte order. A field the parameter is too
    /// short to carry keeps its §7.6.3.4.1 default, which is what
    /// §9.6.3.1 says an absent policy means.
    ///
    /// A `kind` that Annex D does not define reads as
    /// `DISALLOW_TYPE_COERCION`, the stricter of the two values. A caller
    /// that has to tell the two cases apart uses
    /// [`Self::try_from_bytes`].
    #[must_use]
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Self {
        match Self::try_from_bytes(bytes, little_endian) {
            Ok(v) => v,
            Err(_) => Self {
                kind: TypeConsistencyKind::DisallowTypeCoercion,
                ..Self::default()
            },
        }
    }

    /// Decode in the given byte order and refuse a `kind` that Annex D
    /// does not define. Fields the parameter is too short to carry keep
    /// their §7.6.3.4.1 defaults.
    ///
    /// # Errors
    /// [`UnknownTypeConsistencyKind`] for a `kind` other than 0 and 1.
    pub fn try_from_bytes(
        bytes: &[u8],
        little_endian: bool,
    ) -> Result<Self, UnknownTypeConsistencyKind> {
        if bytes.len() < 2 {
            return Ok(Self::default());
        }
        // Infallible: len >= 2 checked above.
        let k = [bytes[0], bytes[1]];
        let kind = TypeConsistencyKind::from_u32(u32::from(if little_endian {
            u16::from_le_bytes(k)
        } else {
            u16::from_be_bytes(k)
        }))?;
        let d = Self::default();
        Ok(Self {
            kind,
            ignore_sequence_bounds: bytes.get(2).map_or(d.ignore_sequence_bounds, |b| *b != 0),
            ignore_string_bounds: bytes.get(3).map_or(d.ignore_string_bounds, |b| *b != 0),
            ignore_member_names: bytes.get(4).map_or(d.ignore_member_names, |b| *b != 0),
            prevent_type_widening: bytes.get(5).map_or(d.prevent_type_widening, |b| *b != 0),
            force_type_validation: bytes.get(6).map_or(d.force_type_validation, |b| *b != 0),
        })
    }
}

/// Data-Representation-ID (§7.6.3.1.1).
#[repr(i16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRepresentationId {
    /// XCDR version 1 (classic).
    Xcdr1 = 0,
    /// XML representation.
    Xml = 1,
    /// XCDR version 2. Not the default: the default is `[XCDR]`
    /// (§7.6.3.1.1).
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
///
/// **An empty list is not an empty set.** XTypes 1.3 §7.6.3.1: "The
/// default value of the `DataRepresentationQosPolicy` shall be an empty
/// list of preferences. An empty list of preferences shall be taken to
/// be equivalent to a list containing the single element XCDR."
///
/// Reading it as an empty set is what this did, and the consequence was
/// not subtle: two endpoints that both leave the policy at its default
/// offer nothing and accept nothing, so `NoOverlap` — the answer this
/// function gave for the pair the specification calls the default.
/// `XCDR` there is `DataRepresentationId::Xcdr1`, the identifier value
/// zero of Table 60.
#[must_use]
pub fn negotiate_representation(
    writer_offered: &[i16],
    reader_accepted: &[i16],
) -> RepresentationNegotiation {
    /// §7.6.3.1's substitution for an empty list of preferences.
    const DEFAULT: [i16; 1] = [DataRepresentationId::Xcdr1 as i16];
    let offered = if writer_offered.is_empty() {
        &DEFAULT[..]
    } else {
        writer_offered
    };
    let accepted = if reader_accepted.is_empty() {
        &DEFAULT[..]
    } else {
        reader_accepted
    };
    for w in offered {
        if accepted.contains(w) {
            if let Some(kind) = DataRepresentationId::from_i16(*w) {
                return RepresentationNegotiation::Accepted(kind);
            }
        }
    }
    RepresentationNegotiation::NoOverlap
}

/// Wire constraint matrix (§7.6.3.1.2, Table 60): for each DataRepresentation
/// and extensibility category, whether the pairing has an encapsulation
/// identifier. XCDR version 1 has one for APPENDABLE (and FINAL) as
/// `CDR_BE`/`CDR_LE` and for MUTABLE as `PL_CDR_BE`/`PL_CDR_LE`; version 2
/// has one for FINAL, APPENDABLE and MUTABLE.
///
/// Returns `Ok(())` if the combination is in the table; otherwise `Err`
/// with a description. No combination of the three representations is
/// outside it at present.
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
        (Xcdr1 | Xcdr2, Final | Appendable | Mutable) => Ok(()),
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

    /// XTypes 1.3 §7.6.3.1: "The default value of the
    /// `DataRepresentationQosPolicy` shall be an empty list of
    /// preferences. An empty list of preferences shall be taken to be
    /// equivalent to a list containing the single element XCDR."
    ///
    /// Both directions and the pair of defaults. This used to answer
    /// `NoOverlap` for all three, which means two endpoints that touch
    /// neither policy do not match through this function.
    #[test]
    fn an_empty_list_of_preferences_is_the_single_element_xcdr() {
        let xcdr = DataRepresentationId::Xcdr1 as i16;
        assert_eq!(
            negotiate_representation(&[], &[xcdr]),
            RepresentationNegotiation::Accepted(DataRepresentationId::Xcdr1)
        );
        assert_eq!(
            negotiate_representation(&[xcdr], &[]),
            RepresentationNegotiation::Accepted(DataRepresentationId::Xcdr1)
        );
        assert_eq!(
            negotiate_representation(&[], &[]),
            RepresentationNegotiation::Accepted(DataRepresentationId::Xcdr1)
        );
        // And the substitution is XCDR, not "anything": an empty offer
        // against a reader that takes only XCDR2 still finds nothing.
        assert_eq!(
            negotiate_representation(&[], &[DataRepresentationId::Xcdr2 as i16]),
            RepresentationNegotiation::NoOverlap
        );
    }
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
            kind: TypeConsistencyKind::AllowTypeCoercion,
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

    /// The octets two vendors actually write. RTI Connext 7.7.0 and
    /// Fast DDS both announce PID_TYPE_CONSISTENCY_ENFORCEMENT with a
    /// body of eight and this content —
    /// `crates/discovery/tests/fixtures/vendor-discovery/{rti,fastdds}.frames`.
    #[test]
    fn the_default_policy_is_the_octets_rti_and_fastdds_write() {
        assert_eq!(
            TypeConsistencyEnforcement::default().to_bytes_le(),
            alloc::vec![0x01, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00],
        );
        assert_eq!(
            TypeConsistencyEnforcement::from_bytes_le(&[
                0x01, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00
            ]),
            TypeConsistencyEnforcement::default(),
        );
    }

    /// A body written big-endian reads back as the same policy.
    #[test]
    fn the_policy_survives_a_big_endian_parameter_list() {
        let c = TypeConsistencyEnforcement {
            kind: TypeConsistencyKind::DisallowTypeCoercion,
            ignore_sequence_bounds: false,
            ignore_string_bounds: true,
            ignore_member_names: true,
            prevent_type_widening: false,
            force_type_validation: true,
        };
        assert_eq!(
            TypeConsistencyEnforcement::from_bytes(&c.to_bytes(false), false),
            c
        );
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

    // ---- §7.6.3.1.2 Table 60 DataRepresentation/Extensibility-Matrix ----

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

    /// Table 60 lists XCDR version 1 with APPENDABLE (`CDR_BE`, `CDR_LE`).
    #[test]
    fn xcdr1_appendable_is_allowed() {
        let res = check_data_repr_extensibility(
            DataRepresentationId::Xcdr1,
            ExtensibilityForRepr::Appendable,
        );
        assert!(res.is_ok());
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

    /// Annex D: `TypeConsistencyKind` has the two values
    /// `DISALLOW_TYPE_COERCION` and `ALLOW_TYPE_COERCION`. A third value
    /// on the wire is refused and not mapped to one of them.
    #[test]
    fn type_consistency_kind_has_the_two_values_of_annex_d() {
        assert_eq!(
            TypeConsistencyKind::from_u32(0),
            Ok(TypeConsistencyKind::DisallowTypeCoercion)
        );
        assert_eq!(
            TypeConsistencyKind::from_u32(1),
            Ok(TypeConsistencyKind::AllowTypeCoercion)
        );
        assert_eq!(
            TypeConsistencyKind::from_u32(2),
            Err(UnknownTypeConsistencyKind(2))
        );
        let body = [0x02, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(
            TypeConsistencyEnforcement::try_from_bytes(&body, true),
            Err(UnknownTypeConsistencyKind(2))
        );
        // The lenient decoder reads the stricter value.
        assert_eq!(
            TypeConsistencyEnforcement::from_bytes(&body, true).kind,
            TypeConsistencyKind::DisallowTypeCoercion
        );
    }
}
