// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeObject (XTypes 1.3 §7.3.4.4).
//!
//! TypeObject ist eine Union `{ MinimalTypeObject | CompleteTypeObject }`.
//! Im Wire-Format wird der Diskriminator als `EquivalenceKind` (1 byte)
//! kodiert: `EK_MINIMAL=0xF1`, `EK_COMPLETE=0xF2`.
//!
//! Beide Varianten sind vollstaendig implementiert (T2 Minimal, T3 Complete).

pub mod common;
pub mod complete;
pub mod flags;
pub mod kinds;
pub mod minimal;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError};

use crate::error::TypeCodecError;
use crate::type_identifier::kinds::{EK_COMPLETE, EK_MINIMAL};

pub use complete::CompleteTypeObject;
pub use minimal::MinimalTypeObject;

/// TypeObject-Wrapper (Minimal oder Complete).
///
/// `Complete` ist deutlich groesser als `Minimal` (Namen + Annotationen
/// laut XTypes 1.3 §7.3.1). Wire-Layout ist Spec-normativ; Boxing der
/// `Complete`-Variante wuerde ~20 Konstruktor-Callsites umbiegen ohne
/// echten Memory-Gewinn (TypeObject lebt selten auf dem Hot-Path,
/// sondern in TypeLookup-Caches).
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypeObject {
    /// Minimal-Repraesentation (hash-genau, namenlos).
    Minimal(MinimalTypeObject),
    /// Complete-Repraesentation (mit Namen + Annotationen).
    Complete(CompleteTypeObject),
}

impl TypeObject {
    /// EquivalenceKind-Byte.
    #[must_use]
    pub const fn discriminator(&self) -> u8 {
        match self {
            Self::Minimal(_) => EK_MINIMAL,
            Self::Complete(_) => EK_COMPLETE,
        }
    }

    /// Encode als `{ octet _d; body }`.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u8(self.discriminator())?;
        match self {
            Self::Minimal(m) => m.encode_into(w),
            Self::Complete(c) => c.encode_into(w),
        }
    }

    /// Decode.
    ///
    /// # Errors
    /// `TypeCodecError::UnknownTypeKind` bei unbekanntem Equivalence-Kind.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let d = r.read_u8()?;
        match d {
            EK_MINIMAL => Ok(Self::Minimal(MinimalTypeObject::decode_from(r)?)),
            EK_COMPLETE => Ok(Self::Complete(CompleteTypeObject::decode_from(r)?)),
            other => Err(TypeCodecError::UnknownTypeKind { kind: other }),
        }
    }

    /// Convenience: LE-Bytes.
    ///
    /// # Errors
    /// Encode-Fehler.
    pub fn to_bytes_le(&self) -> Result<alloc::vec::Vec<u8>, EncodeError> {
        let mut w = BufferWriter::new(zerodds_cdr::Endianness::Little);
        self.encode_into(&mut w)?;
        Ok(w.into_bytes())
    }

    /// Convenience: aus LE-Bytes.
    ///
    /// # Errors
    /// Decode-Fehler.
    pub fn from_bytes_le(bytes: &[u8]) -> Result<Self, TypeCodecError> {
        let mut r = BufferReader::new(bytes, zerodds_cdr::Endianness::Little);
        Self::decode_from(&mut r)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::type_identifier::{PrimitiveKind, TypeIdentifier};
    use crate::type_object::common::{CommonStructMember, CommonUnionMember, NameHash};
    use crate::type_object::flags::{
        AliasMemberFlag, AliasTypeFlag, BitmaskTypeFlag, CollectionElementFlag, CollectionTypeFlag,
        EnumLiteralFlag, EnumTypeFlag, StructMemberFlag, StructTypeFlag, UnionDiscriminatorFlag,
        UnionMemberFlag, UnionTypeFlag,
    };
    use crate::type_object::minimal::{
        CommonAliasBody, CommonCollectionElement, CommonDiscriminatorMember,
        CommonEnumeratedHeader, CommonEnumeratedLiteral, MinimalAliasBody, MinimalAliasType,
        MinimalArrayType, MinimalCollectionElement, MinimalDiscriminatorMember,
        MinimalEnumeratedHeader, MinimalEnumeratedLiteral, MinimalEnumeratedType, MinimalMapType,
        MinimalSequenceType, MinimalStructHeader, MinimalStructMember, MinimalStructType,
        MinimalUnionMember, MinimalUnionType,
    };

    fn roundtrip(to: TypeObject) {
        let bytes = to.to_bytes_le().unwrap();
        let decoded = TypeObject::from_bytes_le(&bytes).unwrap();
        assert_eq!(to, decoded);
    }

    fn name_hash(bytes: [u8; 4]) -> NameHash {
        NameHash(bytes)
    }

    // ------------------------------------------------------------------
    // Struct
    // ------------------------------------------------------------------

    #[test]
    fn minimal_struct_sensor_with_two_fields() {
        let st = MinimalStructType {
            struct_flags: StructTypeFlag(StructTypeFlag::IS_APPENDABLE),
            header: MinimalStructHeader {
                base_type: TypeIdentifier::None,
            },
            member_seq: alloc::vec![
                MinimalStructMember {
                    common: CommonStructMember {
                        member_id: 1,
                        member_flags: StructMemberFlag(StructMemberFlag::IS_KEY),
                        member_type_id: TypeIdentifier::Primitive(PrimitiveKind::Int64),
                    },
                    detail: name_hash([0xde, 0xad, 0xbe, 0xef]),
                },
                MinimalStructMember {
                    common: CommonStructMember {
                        member_id: 2,
                        member_flags: StructMemberFlag::default(),
                        member_type_id: TypeIdentifier::String8Small { bound: 128 },
                    },
                    detail: name_hash([0xfe, 0xed, 0xfa, 0xce]),
                },
            ],
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Struct(st)));
    }

    #[test]
    fn minimal_struct_with_base_type() {
        let st = MinimalStructType {
            struct_flags: StructTypeFlag::empty(),
            header: MinimalStructHeader {
                base_type: TypeIdentifier::EquivalenceHashMinimal(
                    crate::type_identifier::EquivalenceHash([0x01; 14]),
                ),
            },
            member_seq: alloc::vec![],
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Struct(st)));
    }

    #[test]
    fn minimal_struct_mutable_final_flags() {
        for flags in [
            StructTypeFlag::IS_FINAL,
            StructTypeFlag::IS_MUTABLE | StructTypeFlag::IS_AUTOID_HASH,
        ] {
            let st = MinimalStructType {
                struct_flags: StructTypeFlag(flags),
                header: MinimalStructHeader {
                    base_type: TypeIdentifier::None,
                },
                member_seq: alloc::vec![],
            };
            roundtrip(TypeObject::Minimal(MinimalTypeObject::Struct(st)));
        }
    }

    // ------------------------------------------------------------------
    // Union
    // ------------------------------------------------------------------

    #[test]
    fn minimal_union_with_int_discriminator_two_cases() {
        let u = MinimalUnionType {
            union_flags: UnionTypeFlag::default(),
            discriminator: MinimalDiscriminatorMember {
                common: CommonDiscriminatorMember {
                    member_flags: UnionDiscriminatorFlag::default(),
                    type_id: TypeIdentifier::Primitive(PrimitiveKind::Int32),
                },
            },
            member_seq: alloc::vec![
                MinimalUnionMember {
                    common: CommonUnionMember {
                        member_id: 1,
                        member_flags: UnionMemberFlag::default(),
                        type_id: TypeIdentifier::Primitive(PrimitiveKind::Int64),
                        label_seq: alloc::vec![1, 2, 3],
                    },
                    detail: name_hash([0xaa, 0xbb, 0xcc, 0xdd]),
                },
                MinimalUnionMember {
                    common: CommonUnionMember {
                        member_id: 2,
                        member_flags: UnionMemberFlag(UnionMemberFlag::IS_DEFAULT),
                        type_id: TypeIdentifier::String8Small { bound: 64 },
                        label_seq: alloc::vec![],
                    },
                    detail: name_hash([0x11, 0x22, 0x33, 0x44]),
                },
            ],
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Union(u)));
    }

    // ------------------------------------------------------------------
    // Enum
    // ------------------------------------------------------------------

    #[test]
    fn minimal_enum_color_3_literals() {
        let e = MinimalEnumeratedType {
            enum_flags: EnumTypeFlag::default(),
            header: MinimalEnumeratedHeader {
                common: CommonEnumeratedHeader { bit_bound: 32 },
            },
            literal_seq: alloc::vec![
                MinimalEnumeratedLiteral {
                    common: CommonEnumeratedLiteral {
                        value: 0,
                        flags: EnumLiteralFlag(EnumLiteralFlag::IS_DEFAULT_LITERAL),
                    },
                    detail: name_hash([0x01, 0x02, 0x03, 0x04]),
                },
                MinimalEnumeratedLiteral {
                    common: CommonEnumeratedLiteral {
                        value: 1,
                        flags: EnumLiteralFlag::default(),
                    },
                    detail: name_hash([0x05, 0x06, 0x07, 0x08]),
                },
                MinimalEnumeratedLiteral {
                    common: CommonEnumeratedLiteral {
                        value: 2,
                        flags: EnumLiteralFlag::default(),
                    },
                    detail: name_hash([0x09, 0x0a, 0x0b, 0x0c]),
                },
            ],
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Enumerated(e)));
    }

    // ------------------------------------------------------------------
    // Alias
    // ------------------------------------------------------------------

    #[test]
    fn minimal_alias_int64_typedef() {
        let a = MinimalAliasType {
            alias_flags: AliasTypeFlag::default(),
            body: MinimalAliasBody {
                common: CommonAliasBody {
                    related_flags: AliasMemberFlag::default(),
                    related_type: TypeIdentifier::Primitive(PrimitiveKind::Int64),
                },
            },
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Alias(a)));
    }

    // ------------------------------------------------------------------
    // Bitmask
    // ------------------------------------------------------------------

    #[test]
    fn minimal_bitmask_3_flags() {
        use crate::type_object::minimal::{CommonBitflag, MinimalBitflag, MinimalBitmaskType};
        let b = MinimalBitmaskType {
            bitmask_flags: BitmaskTypeFlag::default(),
            bit_bound: 32,
            flag_seq: alloc::vec![
                MinimalBitflag {
                    common: CommonBitflag {
                        position: 0,
                        flags: crate::type_object::flags::BitflagFlag::default(),
                    },
                    detail: name_hash([0x10; 4]),
                },
                MinimalBitflag {
                    common: CommonBitflag {
                        position: 1,
                        flags: crate::type_object::flags::BitflagFlag::default(),
                    },
                    detail: name_hash([0x20; 4]),
                },
                MinimalBitflag {
                    common: CommonBitflag {
                        position: 2,
                        flags: crate::type_object::flags::BitflagFlag::default(),
                    },
                    detail: name_hash([0x30; 4]),
                },
            ],
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Bitmask(b)));
    }

    // ------------------------------------------------------------------
    // Sequence / Array / Map
    // ------------------------------------------------------------------

    #[test]
    fn minimal_sequence_of_floats() {
        let s = MinimalSequenceType {
            collection_flag: CollectionTypeFlag::default(),
            bound: 100,
            element: MinimalCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: TypeIdentifier::Primitive(PrimitiveKind::Float64),
                },
            },
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Sequence(s)));
    }

    #[test]
    fn minimal_array_3d_doubles() {
        let a = MinimalArrayType {
            collection_flag: CollectionTypeFlag::default(),
            bound_seq: alloc::vec![4, 4, 4],
            element: MinimalCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: TypeIdentifier::Primitive(PrimitiveKind::Float64),
                },
            },
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Array(a)));
    }

    #[test]
    fn minimal_map_string_to_int64() {
        let m = MinimalMapType {
            collection_flag: CollectionTypeFlag::default(),
            bound: 1_000,
            key: MinimalCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: TypeIdentifier::String8Small { bound: 64 },
                },
            },
            element: MinimalCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: TypeIdentifier::Primitive(PrimitiveKind::Int64),
                },
            },
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Map(m)));
    }

    // ------------------------------------------------------------------
    // Annotation
    // ------------------------------------------------------------------

    #[test]
    fn minimal_annotation_with_named_parameter() {
        use crate::type_object::flags::{AnnotationParameterFlag, AnnotationTypeFlag};
        use crate::type_object::minimal::{
            AnnotationParameterValue, CommonAnnotationParameter, MinimalAnnotationParameter,
            MinimalAnnotationType,
        };
        let a = MinimalAnnotationType {
            annotation_flag: AnnotationTypeFlag::default(),
            member_seq: alloc::vec![MinimalAnnotationParameter {
                common: CommonAnnotationParameter {
                    member_id: 1,
                    member_flags: AnnotationParameterFlag::default(),
                    member_type_id: TypeIdentifier::Primitive(PrimitiveKind::Int32),
                },
                name: alloc::string::String::from("max"),
                default_value: AnnotationParameterValue {
                    raw: alloc::vec![0, 0, 0, 10],
                },
            }],
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Annotation(a)));
    }

    // ------------------------------------------------------------------
    // Bitset
    // ------------------------------------------------------------------

    #[test]
    fn minimal_bitset_roundtrips() {
        use crate::type_object::flags::{BitfieldFlag, BitsetTypeFlag};
        use crate::type_object::minimal::{CommonBitfield, MinimalBitfield, MinimalBitsetType};
        let b = MinimalBitsetType {
            bitset_flags: BitsetTypeFlag::default(),
            field_seq: alloc::vec![MinimalBitfield {
                common: CommonBitfield {
                    position: 0,
                    flags: BitfieldFlag::default(),
                    bitcount: 3,
                    holder_type: 0x07, // TK_UINT32
                },
                name_hash: name_hash([0xab; 4]),
            }],
        };
        roundtrip(TypeObject::Minimal(MinimalTypeObject::Bitset(b)));
    }

    // ------------------------------------------------------------------
    // Complete — T3
    // ------------------------------------------------------------------

    #[test]
    fn complete_struct_sensor_with_names_and_annotations() {
        use crate::type_object::common::{
            AppliedAnnotation, AppliedAnnotationParameter, AppliedBuiltinMemberAnnotations,
            AppliedBuiltinTypeAnnotations, AppliedVerbatimAnnotation, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq, VerbatimPlacement,
        };
        use crate::type_object::complete::{
            CompleteStructHeader, CompleteStructMember, CompleteStructType,
        };
        let st = CompleteStructType {
            struct_flags: StructTypeFlag(StructTypeFlag::IS_MUTABLE),
            header: CompleteStructHeader {
                base_type: TypeIdentifier::None,
                detail: CompleteTypeDetail {
                    ann_builtin: AppliedBuiltinTypeAnnotations {
                        verbatim: Some(AppliedVerbatimAnnotation {
                            placement: VerbatimPlacement::Before,
                            language: alloc::string::String::from("c++"),
                            text: alloc::string::String::from("// hand-written"),
                        }),
                    },
                    ann_custom: OptionalAppliedAnnotationSeq(Some(alloc::vec![
                        AppliedAnnotation {
                            annotation_typeid: TypeIdentifier::EquivalenceHashComplete(
                                crate::type_identifier::EquivalenceHash([0xA0; 14]),
                            ),
                            param_seq: alloc::vec![AppliedAnnotationParameter {
                                paramname_hash: name_hash([0x01; 4]),
                                value: alloc::vec![0, 0, 0, 42],
                            }],
                        },
                    ])),
                    type_name: alloc::string::String::from("::sensors::Chatter"),
                },
            },
            member_seq: alloc::vec![CompleteStructMember {
                common: CommonStructMember {
                    member_id: 1,
                    member_flags: StructMemberFlag(StructMemberFlag::IS_KEY),
                    member_type_id: TypeIdentifier::Primitive(PrimitiveKind::Int64),
                },
                detail: CompleteMemberDetail {
                    name: alloc::string::String::from("sensor_id"),
                    ann_builtin: AppliedBuiltinMemberAnnotations {
                        unit: Some(alloc::string::String::from("celsius")),
                        min: Some(alloc::vec![0, 0, 0, 0]),
                        max: Some(alloc::vec![0xff, 0xff, 0xff, 0xff]),
                        hash_id: None,
                        default_value: None,
                    },
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                },
            }],
        };
        roundtrip(TypeObject::Complete(CompleteTypeObject::Struct(st)));
    }

    #[test]
    fn complete_union_roundtrips() {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{
            CompleteDiscriminatorMember, CompleteUnionHeader, CompleteUnionMember,
            CompleteUnionType,
        };
        let u = CompleteUnionType {
            union_flags: UnionTypeFlag::default(),
            header: CompleteUnionHeader {
                detail: CompleteTypeDetail {
                    ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                    type_name: alloc::string::String::from("::MyUnion"),
                },
            },
            discriminator: CompleteDiscriminatorMember {
                common: CommonDiscriminatorMember {
                    member_flags: UnionDiscriminatorFlag::default(),
                    type_id: TypeIdentifier::Primitive(PrimitiveKind::Int32),
                },
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            },
            member_seq: alloc::vec![CompleteUnionMember {
                common: CommonUnionMember {
                    member_id: 1,
                    member_flags: UnionMemberFlag::default(),
                    type_id: TypeIdentifier::Primitive(PrimitiveKind::Int64),
                    label_seq: alloc::vec![42],
                },
                detail: CompleteMemberDetail {
                    name: alloc::string::String::from("value"),
                    ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                },
            }],
        };
        roundtrip(TypeObject::Complete(CompleteTypeObject::Union(u)));
    }

    #[test]
    fn complete_enum_with_literal_names() {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{
            CompleteEnumeratedHeader, CompleteEnumeratedLiteral, CompleteEnumeratedType,
        };
        let e = CompleteEnumeratedType {
            enum_flags: EnumTypeFlag::default(),
            header: CompleteEnumeratedHeader {
                common: CommonEnumeratedHeader { bit_bound: 32 },
                detail: CompleteTypeDetail {
                    ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                    type_name: alloc::string::String::from("::Color"),
                },
            },
            literal_seq: alloc::vec![
                CompleteEnumeratedLiteral {
                    common: CommonEnumeratedLiteral {
                        value: 0,
                        flags: EnumLiteralFlag(EnumLiteralFlag::IS_DEFAULT_LITERAL),
                    },
                    detail: CompleteMemberDetail {
                        name: alloc::string::String::from("RED"),
                        ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                        ann_custom: OptionalAppliedAnnotationSeq::default(),
                    },
                },
                CompleteEnumeratedLiteral {
                    common: CommonEnumeratedLiteral {
                        value: 1,
                        flags: EnumLiteralFlag::default(),
                    },
                    detail: CompleteMemberDetail {
                        name: alloc::string::String::from("GREEN"),
                        ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                        ann_custom: OptionalAppliedAnnotationSeq::default(),
                    },
                },
            ],
        };
        roundtrip(TypeObject::Complete(CompleteTypeObject::Enumerated(e)));
    }

    #[test]
    fn complete_alias_typedef() {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteTypeDetail,
            OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{
            CompleteAliasBody, CompleteAliasHeader, CompleteAliasType,
        };
        let a = CompleteAliasType {
            alias_flags: AliasTypeFlag::default(),
            header: CompleteAliasHeader {
                detail: CompleteTypeDetail {
                    ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                    type_name: alloc::string::String::from("::Count"),
                },
            },
            body: CompleteAliasBody {
                related_flags: AliasMemberFlag::default(),
                related_type: TypeIdentifier::Primitive(PrimitiveKind::UInt64),
                ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            },
        };
        roundtrip(TypeObject::Complete(CompleteTypeObject::Alias(a)));
    }

    #[test]
    fn complete_array_3d_roundtrips() {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteTypeDetail,
            OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{CompleteArrayType, CompleteCollectionElement};
        let a = CompleteArrayType {
            collection_flag: CollectionTypeFlag::default(),
            bound_seq: alloc::vec![3, 4, 5],
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: alloc::string::String::from("::Cube"),
            },
            element: CompleteCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: TypeIdentifier::Primitive(PrimitiveKind::Int32),
                },
                ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            },
        };
        roundtrip(TypeObject::Complete(CompleteTypeObject::Array(a)));
    }

    #[test]
    fn complete_map_roundtrips() {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteTypeDetail,
            OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{CompleteCollectionElement, CompleteMapType};
        let m = CompleteMapType {
            collection_flag: CollectionTypeFlag::default(),
            bound: 200,
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: alloc::string::String::from("::StrIntMap"),
            },
            key: CompleteCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: TypeIdentifier::String8Small { bound: 32 },
                },
                ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            },
            element: CompleteCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: TypeIdentifier::Primitive(PrimitiveKind::Int64),
                },
                ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            },
        };
        roundtrip(TypeObject::Complete(CompleteTypeObject::Map(m)));
    }

    #[test]
    fn complete_bitmask_roundtrips() {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{CompleteBitflag, CompleteBitmaskType};
        use crate::type_object::flags::BitflagFlag;
        let b = CompleteBitmaskType {
            bitmask_flags: BitmaskTypeFlag::default(),
            bit_bound: 32,
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: alloc::string::String::from("::Perms"),
            },
            flag_seq: alloc::vec![
                CompleteBitflag {
                    common: crate::type_object::minimal::CommonBitflag {
                        position: 0,
                        flags: BitflagFlag::default(),
                    },
                    detail: CompleteMemberDetail {
                        name: alloc::string::String::from("READ"),
                        ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                        ann_custom: OptionalAppliedAnnotationSeq::default(),
                    },
                },
                CompleteBitflag {
                    common: crate::type_object::minimal::CommonBitflag {
                        position: 1,
                        flags: BitflagFlag::default(),
                    },
                    detail: CompleteMemberDetail {
                        name: alloc::string::String::from("WRITE"),
                        ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                        ann_custom: OptionalAppliedAnnotationSeq::default(),
                    },
                },
            ],
        };
        roundtrip(TypeObject::Complete(CompleteTypeObject::Bitmask(b)));
    }

    #[test]
    fn complete_bitset_roundtrips() {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{CompleteBitfield, CompleteBitsetType};
        use crate::type_object::flags::{BitfieldFlag, BitsetTypeFlag};
        let b = CompleteBitsetType {
            bitset_flags: BitsetTypeFlag::default(),
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: alloc::string::String::from("::Packed"),
            },
            field_seq: alloc::vec![CompleteBitfield {
                common: crate::type_object::minimal::CommonBitfield {
                    position: 0,
                    flags: BitfieldFlag::default(),
                    bitcount: 3,
                    holder_type: 0x07,
                },
                detail: CompleteMemberDetail {
                    name: alloc::string::String::from("a"),
                    ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                },
            }],
        };
        roundtrip(TypeObject::Complete(CompleteTypeObject::Bitset(b)));
    }

    #[test]
    fn complete_annotation_roundtrips() {
        use crate::type_object::common::{
            AppliedBuiltinTypeAnnotations, CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{CompleteAnnotationParameter, CompleteAnnotationType};
        use crate::type_object::flags::{AnnotationParameterFlag, AnnotationTypeFlag};
        let a = CompleteAnnotationType {
            annotation_flag: AnnotationTypeFlag::default(),
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: alloc::string::String::from("::my_ann"),
            },
            member_seq: alloc::vec![CompleteAnnotationParameter {
                member_id: 1,
                member_flags: AnnotationParameterFlag::default(),
                member_type_id: TypeIdentifier::Primitive(PrimitiveKind::Int32),
                name: alloc::string::String::from("max"),
                default_value: alloc::vec![0, 0, 0, 99],
            }],
        };
        roundtrip(TypeObject::Complete(CompleteTypeObject::Annotation(a)));
    }

    #[test]
    fn complete_verbatim_placement_variants_roundtrip() {
        use crate::type_object::common::{
            AppliedBuiltinTypeAnnotations, AppliedVerbatimAnnotation, CompleteTypeDetail,
            OptionalAppliedAnnotationSeq, VerbatimPlacement,
        };
        use crate::type_object::complete::{CompleteStructHeader, CompleteStructType};
        for placement in [
            VerbatimPlacement::Before,
            VerbatimPlacement::After,
            VerbatimPlacement::BeginFile,
            VerbatimPlacement::EndFile,
            VerbatimPlacement::Other(alloc::string::String::from("CUSTOM")),
        ] {
            let st = CompleteStructType {
                struct_flags: StructTypeFlag::empty(),
                header: CompleteStructHeader {
                    base_type: TypeIdentifier::None,
                    detail: CompleteTypeDetail {
                        ann_builtin: AppliedBuiltinTypeAnnotations {
                            verbatim: Some(AppliedVerbatimAnnotation {
                                placement: placement.clone(),
                                language: alloc::string::String::from("rust"),
                                text: alloc::string::String::from("// hand written"),
                            }),
                        },
                        ann_custom: OptionalAppliedAnnotationSeq::default(),
                        type_name: alloc::string::String::from("::X"),
                    },
                },
                member_seq: alloc::vec![],
            };
            roundtrip(TypeObject::Complete(CompleteTypeObject::Struct(st)));
        }
    }

    #[test]
    fn complete_sequence_of_floats() {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteTypeDetail,
            OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{CompleteCollectionElement, CompleteSequenceType};
        let s = CompleteSequenceType {
            collection_flag: CollectionTypeFlag::default(),
            bound: 128,
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: alloc::string::String::from("::Floats"),
            },
            element: CompleteCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: TypeIdentifier::Primitive(PrimitiveKind::Float64),
                },
                ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            },
        };
        roundtrip(TypeObject::Complete(CompleteTypeObject::Sequence(s)));
    }

    // ------------------------------------------------------------------
    // Unknown discriminator
    // ------------------------------------------------------------------

    #[test]
    fn unknown_equivalence_kind_fails_gracefully() {
        let bytes = [0xAB_u8];
        let err = TypeObject::from_bytes_le(&bytes).unwrap_err();
        assert!(matches!(
            err,
            TypeCodecError::UnknownTypeKind { kind: 0xAB }
        ));
    }
}
