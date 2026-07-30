// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DynamicType ↔ TypeObject Bridge (XTypes 1.3 §7.6.3 + §7.6.4).
//!
//! This bridge is the bracket between the **wire representation**
//! (`TypeObject`, with `MinimalTypeObject` + `CompleteTypeObject` as the
//! discrimination) and the **runtime API** (`DynamicType`).
//!
//! Implements all 10 TypeObject kinds: struct + union + enum + bitmask +
//! bitset + annotation + alias plus the collection kinds sequence/array/map
//! (XTypes §7.3.4.4: `CompleteSequenceType` / `CompleteArrayType` /
//! `CompleteMapType`). Anonymous plain collections are additionally
//! referenced inline via `TypeIdentifier` (PlainCollection); `to_type_object`
//! returns the full collection `CompleteTypeObject` variant here.
//!
//! The reverse direction (`create_type_w_type_object`) resolves a top-level
//! Complete TypeObject of any of the 10 kinds; the registry-aware
//! `create_type_w_type_object_in` additionally follows composite member
//! `EquivalenceHash` references through a [`crate::resolve::TypeRegistry`] so a
//! struct/union with nested struct/union/enum/bitmask/bitset MEMBERS yields a
//! fully-populated `DynamicType`. Collections whose element/key/value is itself a
//! composite (`sequence<Struct>`, `Struct[N]`, `map<K,Struct>`, and arbitrary
//! nesting) are also fully resolved — retained via [`super::collection`] so the
//! reflective codec walks them. `map<K,V>` is codec-complete (byte-identical to
//! the compiled `BTreeMap`); composite `typedef` targets resolve transparently.
//!
//! **Minimal TypeObjects** resolve too (`resolve_minimal`): a Minimal object
//! carries the full structure — member ids, types, flags, nesting — in its shared
//! `common` fields, so a Minimal-resolved type encodes/decodes wire data
//! byte-identically to its Complete twin. The one intrinsic difference is that a
//! Minimal object holds only 4-byte name *hashes* (by XTypes design), so member /
//! type names are synthetic (`m_<hash>`) unless the Complete form is co-published.
//! (The bare, registry-less `create_type_w_type_object` still declines Minimal,
//! since nested minimal-hash members need the registry.)

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::type_identifier::{PrimitiveKind, TypeIdentifier};
use crate::type_object::common::NameHash;
use crate::type_object::common::{CommonStructMember, CommonUnionMember};
use crate::type_object::complete::{
    CompleteAliasBody, CompleteAliasHeader, CompleteAliasType, CompleteAnnotationParameter,
    CompleteAnnotationType, CompleteArrayType, CompleteBitfield, CompleteBitflag,
    CompleteBitmaskType, CompleteBitsetType, CompleteCollectionElement,
    CompleteDiscriminatorMember, CompleteEnumeratedHeader, CompleteEnumeratedLiteral,
    CompleteEnumeratedType, CompleteMapType, CompleteSequenceType, CompleteStructHeader,
    CompleteStructMember, CompleteStructType, CompleteUnionHeader, CompleteUnionMember,
    CompleteUnionType,
};
use crate::type_object::flags::{
    AliasMemberFlag, AliasTypeFlag, AnnotationParameterFlag, AnnotationTypeFlag, BitfieldFlag,
    BitflagFlag, BitmaskTypeFlag, BitsetTypeFlag, CollectionElementFlag, CollectionTypeFlag,
    EnumLiteralFlag, EnumTypeFlag, StructMemberFlag, StructTypeFlag, UnionDiscriminatorFlag,
    UnionMemberFlag, UnionTypeFlag,
};
use crate::type_object::minimal::CommonDiscriminatorMember;
use crate::type_object::minimal::{
    CommonBitfield, CommonBitflag, CommonCollectionElement, CommonEnumeratedHeader,
    CommonEnumeratedLiteral, MinimalTypeObject,
};
use crate::type_object::{CompleteTypeObject, TypeObject};

use super::builder::{DynamicTypeBuilder, DynamicTypeBuilderFactory};
use super::collection;
use super::descriptor::{
    ExtensibilityKind, MemberDescriptor, TryConstructKind, TypeDescriptor, TypeKind,
};
use super::error::DynamicError;
use super::type_::DynamicType;

// ----------------------------------------------------------------------
// DynamicType → TypeObject (spec §7.6.3 — for discovery + TypeLookup)
// ----------------------------------------------------------------------

impl DynamicType {
    /// Spec §7.6.3 — converts this DynamicType into a
    /// `CompleteTypeObject`. Always returns complete (not minimal),
    /// because DynamicType carries the names + annotations.
    ///
    /// Scope:
    /// - Struct with primitive + string + sequence/array members.
    /// - Union, enum, alias as their own helper methods (see module doc).
    ///
    /// # Errors
    /// `Unsupported` for not-yet-implemented kinds,
    /// `Inconsistent` if the type is malformed.
    pub fn to_type_object(&self) -> Result<TypeObject, DynamicError> {
        match self.kind() {
            TypeKind::Structure => Ok(TypeObject::Complete(CompleteTypeObject::Struct(
                self.to_complete_struct()?,
            ))),
            TypeKind::Union => Ok(TypeObject::Complete(CompleteTypeObject::Union(
                self.to_complete_union()?,
            ))),
            TypeKind::Enumeration => Ok(TypeObject::Complete(CompleteTypeObject::Enumerated(
                self.to_complete_enum()?,
            ))),
            TypeKind::Bitmask => Ok(TypeObject::Complete(CompleteTypeObject::Bitmask(
                self.to_complete_bitmask()?,
            ))),
            TypeKind::Alias => Ok(TypeObject::Complete(CompleteTypeObject::Alias(
                self.to_complete_alias()?,
            ))),
            TypeKind::Sequence => Ok(TypeObject::Complete(CompleteTypeObject::Sequence(
                self.to_complete_sequence()?,
            ))),
            TypeKind::Array => Ok(TypeObject::Complete(CompleteTypeObject::Array(
                self.to_complete_array()?,
            ))),
            TypeKind::Map => Ok(TypeObject::Complete(CompleteTypeObject::Map(
                self.to_complete_map()?,
            ))),
            TypeKind::Bitset => Ok(TypeObject::Complete(CompleteTypeObject::Bitset(
                self.to_complete_bitset()?,
            ))),
            TypeKind::Annotation => Ok(TypeObject::Complete(CompleteTypeObject::Annotation(
                self.to_complete_annotation()?,
            ))),
            other => Err(DynamicError::unsupported(format!(
                "to_type_object: {other:?} is a primitive/no-type kind without TypeObject",
            ))),
        }
    }

    /// XTypes §7.3.4.4 — `sequence<T>` → `CompleteSequenceType`.
    /// `bound = 0` bedeutet unbounded.
    fn to_complete_sequence(&self) -> Result<CompleteSequenceType, DynamicError> {
        use crate::type_object::common::{
            AppliedBuiltinTypeAnnotations, CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        let element = self
            .descriptor()
            .element_type
            .as_ref()
            .ok_or_else(|| DynamicError::inconsistent("sequence type missing element_type"))?;
        let bound = self.descriptor().bound.first().copied().unwrap_or(0);
        Ok(CompleteSequenceType {
            collection_flag: CollectionTypeFlag(0),
            bound,
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: self.descriptor().name.clone(),
            },
            element: complete_collection_element(element)?,
        })
    }

    /// XTypes §7.3.4.4 — `T[N]` → `CompleteArrayType`.
    /// `bound_seq` carries all array dimensions.
    fn to_complete_array(&self) -> Result<CompleteArrayType, DynamicError> {
        use crate::type_object::common::{
            AppliedBuiltinTypeAnnotations, CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        let element = self
            .descriptor()
            .element_type
            .as_ref()
            .ok_or_else(|| DynamicError::inconsistent("array type missing element_type"))?;
        let bound_seq = self.descriptor().bound.clone();
        if bound_seq.is_empty() {
            return Err(DynamicError::inconsistent(
                "array type missing dimensions (bound)",
            ));
        }
        Ok(CompleteArrayType {
            collection_flag: CollectionTypeFlag(0),
            bound_seq,
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: self.descriptor().name.clone(),
            },
            element: complete_collection_element(element)?,
        })
    }

    /// XTypes §7.3.4.4 — `map<K, V>` → `CompleteMapType`.
    /// `element_type` carries the value type, `key_element_type` the key type.
    fn to_complete_map(&self) -> Result<CompleteMapType, DynamicError> {
        use crate::type_object::common::{
            AppliedBuiltinTypeAnnotations, CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        let value =
            self.descriptor().element_type.as_ref().ok_or_else(|| {
                DynamicError::inconsistent("map type missing element_type (value)")
            })?;
        let key = self
            .descriptor()
            .key_element_type
            .as_ref()
            .ok_or_else(|| DynamicError::inconsistent("map type missing key_element_type"))?;
        let bound = self.descriptor().bound.first().copied().unwrap_or(0);
        Ok(CompleteMapType {
            collection_flag: CollectionTypeFlag(0),
            bound,
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: self.descriptor().name.clone(),
            },
            key: complete_collection_element(key)?,
            element: complete_collection_element(value)?,
        })
    }

    fn to_complete_alias(&self) -> Result<CompleteAliasType, DynamicError> {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteTypeDetail,
            OptionalAppliedAnnotationSeq,
        };
        let header = CompleteAliasHeader {
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: self.descriptor().name.clone(),
            },
        };
        let element = self.descriptor().element_type.as_ref().ok_or_else(|| {
            DynamicError::inconsistent("alias type missing element_type (target)")
        })?;
        let related_type = descriptor_to_type_identifier(element)?;
        let body = CompleteAliasBody {
            related_flags: AliasMemberFlag(0),
            related_type,
            ann_builtin: AppliedBuiltinMemberAnnotations::default(),
            ann_custom: OptionalAppliedAnnotationSeq::default(),
        };
        Ok(CompleteAliasType {
            alias_flags: AliasTypeFlag(0),
            header,
            body,
        })
    }

    fn to_complete_enum(&self) -> Result<CompleteEnumeratedType, DynamicError> {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        let bit_bound = self.descriptor().bound.first().copied().unwrap_or(32);
        let bit_bound_u16 = u16::try_from(bit_bound).unwrap_or(32);
        let header = CompleteEnumeratedHeader {
            common: CommonEnumeratedHeader {
                bit_bound: bit_bound_u16,
            },
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: self.descriptor().name.clone(),
            },
        };
        let mut literal_seq: Vec<CompleteEnumeratedLiteral> =
            Vec::with_capacity(self.member_count() as usize);
        for m in self.members() {
            let mut flags_bits: u16 = 0;
            if m.descriptor().is_default_label {
                flags_bits |= EnumLiteralFlag::IS_DEFAULT_LITERAL;
            }
            // Enum-Ordinalwert = MemberId (Convention).
            let value = i32::try_from(m.id()).map_err(|_| {
                DynamicError::inconsistent(format!("enum literal id {} exceeds i32 range", m.id()))
            })?;
            literal_seq.push(CompleteEnumeratedLiteral {
                common: CommonEnumeratedLiteral {
                    value,
                    flags: EnumLiteralFlag(flags_bits),
                },
                detail: CompleteMemberDetail {
                    name: m.name().to_string(),
                    ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                },
            });
        }
        Ok(CompleteEnumeratedType {
            enum_flags: EnumTypeFlag(0),
            header,
            literal_seq,
        })
    }

    fn to_complete_bitmask(&self) -> Result<CompleteBitmaskType, DynamicError> {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        let bit_bound = self.descriptor().bound.first().copied().unwrap_or(32);
        let bit_bound_u16 = u16::try_from(bit_bound).unwrap_or(32);
        let detail = CompleteTypeDetail {
            ann_builtin: AppliedBuiltinTypeAnnotations::default(),
            ann_custom: OptionalAppliedAnnotationSeq::default(),
            type_name: self.descriptor().name.clone(),
        };
        let mut flag_seq: Vec<CompleteBitflag> = Vec::with_capacity(self.member_count() as usize);
        for m in self.members() {
            let position = u16::try_from(m.id()).map_err(|_| {
                DynamicError::inconsistent(format!(
                    "bitmask flag position {} exceeds u16 range",
                    m.id()
                ))
            })?;
            flag_seq.push(CompleteBitflag {
                common: CommonBitflag {
                    position,
                    flags: BitflagFlag(0),
                },
                detail: CompleteMemberDetail {
                    name: m.name().to_string(),
                    ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                },
            });
        }
        Ok(CompleteBitmaskType {
            bitmask_flags: BitmaskTypeFlag(0),
            bit_bound: bit_bound_u16,
            detail,
            flag_seq,
        })
    }

    /// Spec §7.3.4.4 — Bitset: one `CompleteBitfield` per member, with the
    /// bit start position (`MemberDescriptor.id`), width
    /// (`MemberDescriptor.bit_bound`) and holding type (the TypeKind byte of
    /// the member type, which must be a primitive integer type).
    fn to_complete_bitset(&self) -> Result<CompleteBitsetType, DynamicError> {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        let detail = CompleteTypeDetail {
            ann_builtin: AppliedBuiltinTypeAnnotations::default(),
            ann_custom: OptionalAppliedAnnotationSeq::default(),
            type_name: self.descriptor().name.clone(),
        };
        let mut field_seq: Vec<CompleteBitfield> = Vec::with_capacity(self.member_count() as usize);
        for m in self.members() {
            let position = u16::try_from(m.id()).map_err(|_| {
                DynamicError::inconsistent(format!(
                    "bitset field position {} exceeds u16 range",
                    m.id()
                ))
            })?;
            let holder_type = match descriptor_to_type_identifier(
                m.descriptor().member_type.as_ref(),
            )? {
                TypeIdentifier::Primitive(pk) => pk.to_u8(),
                other => {
                    return Err(DynamicError::inconsistent(format!(
                        "bitset field {:?} holder must be a primitive integer type, got {other:?}",
                        m.name()
                    )));
                }
            };
            let bitcount = m.descriptor().bit_bound.ok_or_else(|| {
                DynamicError::inconsistent(format!(
                    "bitset field {:?} missing bit_bound (bitfield width)",
                    m.name()
                ))
            })?;
            field_seq.push(CompleteBitfield {
                common: CommonBitfield {
                    position,
                    flags: BitfieldFlag(0),
                    bitcount,
                    holder_type,
                },
                detail: CompleteMemberDetail {
                    name: m.name().to_string(),
                    ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                },
            });
        }
        Ok(CompleteBitsetType {
            bitset_flags: BitsetTypeFlag(0),
            detail,
            field_seq,
        })
    }

    /// Spec §7.3.4.4 — Annotation: one `CompleteAnnotationParameter` per
    /// member (parameter), with the member id, parameter type, name and the
    /// default value (stored opaquely as UTF-8 bytes) from
    /// `MemberDescriptor.default_value`.
    fn to_complete_annotation(&self) -> Result<CompleteAnnotationType, DynamicError> {
        use crate::type_object::common::{
            AppliedBuiltinTypeAnnotations, CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        let detail = CompleteTypeDetail {
            ann_builtin: AppliedBuiltinTypeAnnotations::default(),
            ann_custom: OptionalAppliedAnnotationSeq::default(),
            type_name: self.descriptor().name.clone(),
        };
        let mut member_seq: Vec<CompleteAnnotationParameter> =
            Vec::with_capacity(self.member_count() as usize);
        for m in self.members() {
            let member_type_id =
                descriptor_to_type_identifier(m.descriptor().member_type.as_ref())?;
            let default_value = m
                .descriptor()
                .default_value
                .as_deref()
                .map(|s| s.as_bytes().to_vec())
                .unwrap_or_default();
            member_seq.push(CompleteAnnotationParameter {
                member_id: m.id(),
                member_flags: AnnotationParameterFlag(0),
                member_type_id,
                name: m.name().to_string(),
                default_value,
            });
        }
        Ok(CompleteAnnotationType {
            annotation_flag: AnnotationTypeFlag(0),
            detail,
            member_seq,
        })
    }

    fn to_complete_union(&self) -> Result<CompleteUnionType, DynamicError> {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        let union_flags = UnionTypeFlag(extensibility_to_flag_bits(
            self.descriptor().extensibility_kind,
        ));
        let header = CompleteUnionHeader {
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: self.descriptor().name.clone(),
            },
        };
        let disc_descriptor = self
            .descriptor()
            .discriminator_type
            .as_ref()
            .ok_or_else(|| DynamicError::inconsistent("union type missing discriminator_type"))?;
        let disc_type = descriptor_to_type_identifier(disc_descriptor)?;
        let discriminator = CompleteDiscriminatorMember {
            common: CommonDiscriminatorMember {
                member_flags: UnionDiscriminatorFlag(0),
                type_id: disc_type,
            },
            ann_builtin: AppliedBuiltinTypeAnnotations::default(),
            ann_custom: OptionalAppliedAnnotationSeq::default(),
        };
        let mut member_seq: Vec<CompleteUnionMember> =
            Vec::with_capacity(self.member_count() as usize);
        for m in self.members() {
            let mut flags_bits: u16 = 0;
            if m.descriptor().is_default_label {
                flags_bits |= UnionMemberFlag::IS_DEFAULT;
            }
            // Spec §7.3.4.5.3.2 — case labels are `long` (i32).
            let label_seq: Vec<i32> = m
                .descriptor()
                .label
                .iter()
                .map(|&v| i32::try_from(v).unwrap_or_default())
                .collect();
            let common = CommonUnionMember {
                member_id: m.id(),
                member_flags: UnionMemberFlag(flags_bits),
                type_id: descriptor_to_type_identifier(m.descriptor().member_type.as_ref())?,
                label_seq,
            };
            let detail = CompleteMemberDetail {
                name: m.name().to_string(),
                ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            };
            member_seq.push(CompleteUnionMember { common, detail });
        }
        Ok(CompleteUnionType {
            union_flags,
            header,
            discriminator,
            member_seq,
        })
    }

    fn to_complete_struct(&self) -> Result<CompleteStructType, DynamicError> {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        let struct_flags = StructTypeFlag(extensibility_to_flag_bits(
            self.descriptor().extensibility_kind,
        ));
        let header = CompleteStructHeader {
            base_type: TypeIdentifier::None,
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: self.descriptor().name.clone(),
            },
        };
        let mut member_seq: Vec<CompleteStructMember> =
            Vec::with_capacity(self.member_count() as usize);
        for m in self.members() {
            // TryConstruct bits are always present (§7.3.1.2.1.1): the default
            // DISCARD encodes as TRY_CONSTRUCT1, matching the vendors.
            let mut flags_bits: u16 =
                try_construct_to_member_flag_bits(m.descriptor().try_construct);
            if m.descriptor().is_key {
                flags_bits |= StructMemberFlag::IS_KEY;
            }
            if m.descriptor().is_optional {
                flags_bits |= StructMemberFlag::IS_OPTIONAL;
            }
            if m.descriptor().is_must_understand {
                flags_bits |= StructMemberFlag::IS_MUST_UNDERSTAND;
            }
            if m.descriptor().is_shared {
                flags_bits |= StructMemberFlag::IS_EXTERNAL;
            }
            let common = CommonStructMember {
                member_id: m.id(),
                member_flags: StructMemberFlag(flags_bits),
                member_type_id: descriptor_to_type_identifier(m.descriptor().member_type.as_ref())?,
            };
            // Carry the member default so a USE_DEFAULT reader can recover it
            // from the complete TypeObject (§7.2.4.4.4.4.9).
            let ann_builtin = AppliedBuiltinMemberAnnotations {
                default_value: m.descriptor().default_value.clone(),
                ..AppliedBuiltinMemberAnnotations::default()
            };
            let detail = CompleteMemberDetail {
                name: m.name().to_string(),
                ann_builtin,
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            };
            member_seq.push(CompleteStructMember { common, detail });
        }
        Ok(CompleteStructType {
            struct_flags,
            header,
            member_seq,
        })
    }
}

const fn extensibility_to_flag_bits(ext: ExtensibilityKind) -> u16 {
    match ext {
        ExtensibilityKind::Final => StructTypeFlag::IS_FINAL,
        ExtensibilityKind::Appendable => StructTypeFlag::IS_APPENDABLE,
        ExtensibilityKind::Mutable => StructTypeFlag::IS_MUTABLE,
    }
}

const fn flag_bits_to_extensibility(flags: u16) -> ExtensibilityKind {
    if flags & StructTypeFlag::IS_FINAL != 0 {
        ExtensibilityKind::Final
    } else if flags & StructTypeFlag::IS_MUTABLE != 0 {
        ExtensibilityKind::Mutable
    } else {
        ExtensibilityKind::Appendable
    }
}

/// Decodes the TryConstruct member-flag bits into a `TryConstructKind`
/// (XTypes 1.3 §7.3.1.2.1.1, `bits[0..2]`): USE_DEFAULT = `TRY_CONSTRUCT2`
/// (`10`), TRIM = both (`11`), everything else (incl. `01` and the undefined
/// `00`) = DISCARD. This is the decode-side counterpart to
/// `TryConstruct::to_member_flag_bits` in the builder.
const fn try_construct_from_member_flags(flags: u16) -> TryConstructKind {
    match flags & (StructMemberFlag::TRY_CONSTRUCT1 | StructMemberFlag::TRY_CONSTRUCT2) {
        StructMemberFlag::TRY_CONSTRUCT2 => TryConstructKind::UseDefault,
        x if x == StructMemberFlag::TRY_CONSTRUCT1 | StructMemberFlag::TRY_CONSTRUCT2 => {
            TryConstructKind::Trim
        }
        _ => TryConstructKind::Discard,
    }
}

/// Encodes a `TryConstructKind` back into the two member-flag bits
/// (§7.3.1.2.1.1): DISCARD = `TRY_CONSTRUCT1` (`01`), USE_DEFAULT =
/// `TRY_CONSTRUCT2` (`10`), TRIM = both (`11`).
const fn try_construct_to_member_flag_bits(kind: TryConstructKind) -> u16 {
    match kind {
        TryConstructKind::Discard => StructMemberFlag::TRY_CONSTRUCT1,
        TryConstructKind::UseDefault => StructMemberFlag::TRY_CONSTRUCT2,
        TryConstructKind::Trim => {
            StructMemberFlag::TRY_CONSTRUCT1 | StructMemberFlag::TRY_CONSTRUCT2
        }
    }
}

/// XTypes §7.3.4.4 — builds a `CompleteCollectionElement` (element or key
/// description) from a `TypeDescriptor`.
fn complete_collection_element(
    desc: &TypeDescriptor,
) -> Result<CompleteCollectionElement, DynamicError> {
    use crate::type_object::common::{
        AppliedBuiltinMemberAnnotations, OptionalAppliedAnnotationSeq,
    };
    Ok(CompleteCollectionElement {
        common: CommonCollectionElement {
            element_flags: CollectionElementFlag(0),
            type_id: descriptor_to_type_identifier(desc)?,
        },
        ann_builtin: AppliedBuiltinMemberAnnotations::default(),
        ann_custom: OptionalAppliedAnnotationSeq::default(),
    })
}

/// Maps a member `TypeDescriptor` to a matching `TypeIdentifier`. Only
/// primitive + string + sequence + array are encoded inline; composite
/// members require a TypeRegistry lookup strategy via
/// `zerodds_types::resolve::TypeRegistry`.
fn descriptor_to_type_identifier(desc: &TypeDescriptor) -> Result<TypeIdentifier, DynamicError> {
    match desc.kind {
        TypeKind::Boolean => Ok(TypeIdentifier::Primitive(PrimitiveKind::Boolean)),
        TypeKind::Byte => Ok(TypeIdentifier::Primitive(PrimitiveKind::Byte)),
        TypeKind::Int8 => Ok(TypeIdentifier::Primitive(PrimitiveKind::Int8)),
        TypeKind::UInt8 => Ok(TypeIdentifier::Primitive(PrimitiveKind::UInt8)),
        TypeKind::Int16 => Ok(TypeIdentifier::Primitive(PrimitiveKind::Int16)),
        TypeKind::UInt16 => Ok(TypeIdentifier::Primitive(PrimitiveKind::UInt16)),
        TypeKind::Int32 => Ok(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        TypeKind::UInt32 => Ok(TypeIdentifier::Primitive(PrimitiveKind::UInt32)),
        TypeKind::Int64 => Ok(TypeIdentifier::Primitive(PrimitiveKind::Int64)),
        TypeKind::UInt64 => Ok(TypeIdentifier::Primitive(PrimitiveKind::UInt64)),
        TypeKind::Float32 => Ok(TypeIdentifier::Primitive(PrimitiveKind::Float32)),
        TypeKind::Float64 => Ok(TypeIdentifier::Primitive(PrimitiveKind::Float64)),
        TypeKind::Float128 => Ok(TypeIdentifier::Primitive(PrimitiveKind::Float128)),
        TypeKind::Char8 => Ok(TypeIdentifier::Primitive(PrimitiveKind::Char8)),
        TypeKind::Char16 => Ok(TypeIdentifier::Primitive(PrimitiveKind::Char16)),
        TypeKind::String8 => {
            let bound = desc.bound.first().copied().unwrap_or(0);
            if bound <= u32::from(u8::MAX) {
                Ok(TypeIdentifier::String8Small { bound: bound as u8 })
            } else {
                Ok(TypeIdentifier::String8Large { bound })
            }
        }
        TypeKind::String16 => {
            let bound = desc.bound.first().copied().unwrap_or(0);
            if bound <= u32::from(u8::MAX) {
                Ok(TypeIdentifier::String16Small { bound: bound as u8 })
            } else {
                Ok(TypeIdentifier::String16Large { bound })
            }
        }
        TypeKind::Structure | TypeKind::Union | TypeKind::Enumeration | TypeKind::Alias => {
            // Bridge phase 1: composite members carry `TypeIdentifier::None`
            // — the TypeRegistry (C4.2) later resolves via the name
            // against an EquivalenceHash. Roundtrip tests set this
            // information separately.
            Ok(TypeIdentifier::None)
        }
        kind => Err(DynamicError::unsupported(format!(
            "descriptor_to_type_identifier: {kind:?} not yet covered"
        ))),
    }
}

// ----------------------------------------------------------------------
// TypeObject → DynamicType (Spec §7.6.4)
// ----------------------------------------------------------------------

impl DynamicTypeBuilderFactory {
    /// Spec §7.6.4 / §7.5.5.1.4 `create_type_w_type_object(type_object)`.
    ///
    /// Scope:
    /// - `CompleteStructType` with primitive + string members.
    /// - `MinimalStructType` as read-only with hash names.
    /// - Other kinds: `Unsupported`.
    ///
    /// # Errors
    /// `Unsupported` for kinds outside the current implementation scope.
    pub fn create_type_w_type_object(
        type_obj: &TypeObject,
    ) -> Result<DynamicTypeBuilder, DynamicError> {
        match type_obj {
            TypeObject::Complete(c) => match c {
                CompleteTypeObject::Struct(s) => complete_struct_to_builder(s),
                other => Err(DynamicError::unsupported(format!(
                    "complete-typeobject kind {} not yet supported",
                    other_kind_name(other)
                ))),
            },
            TypeObject::Minimal(_) => Err(DynamicError::unsupported(
                "minimal-typeobject → dynamic-type pending C4.2 TypeRegistry",
            )),
        }
    }

    /// Registry-aware `create_type_w_type_object` (XTypes §7.6.4 with a
    /// TypeLookupService). Unlike the bare overload, composite member types
    /// referenced by `EquivalenceHash` are resolved recursively against
    /// `registry`, so a struct/union with nested struct/union/enum/bitmask/
    /// bitset MEMBERS yields a fully-populated `DynamicType` (members carry
    /// their own members). Covers all 10 top-level Complete kinds.
    ///
    /// Collections whose element/key/value is composite (`sequence<NestedStruct>`,
    /// `NestedStruct[N]`, `map<K,Struct>`, and arbitrary nesting) are fully
    /// resolved too, retained via [`super::collection`] so the codec walks the
    /// nested members. Composite `typedef` targets resolve transparently, and
    /// `TypeObject::Minimal` resolves via [`resolve_minimal`] (wire-identical to
    /// Complete; only the names are synthetic hashes).
    ///
    /// # Errors
    /// `Unsupported` only for unresolvable registry hashes and minimal annotation
    /// types (annotations are metadata, not data members).
    pub fn create_type_w_type_object_in(
        type_obj: &TypeObject,
        registry: &crate::resolve::TypeRegistry,
    ) -> Result<DynamicType, DynamicError> {
        match type_obj {
            TypeObject::Complete(c) => resolve_complete(c, registry),
            TypeObject::Minimal(m) => resolve_minimal(m, registry, "MinimalType"),
        }
    }
}

/// Resolves a `TypeIdentifier` to a full `DynamicType`, following composite
/// `EquivalenceHash` references through `registry` recursively.
///
/// zerodds-lint: recursion-depth 16 (capped by TypeIdentifier decode depth +
/// registry acyclicity; a missing/cyclic hash returns `Unsupported`).
fn resolve_type_id(
    ti: &TypeIdentifier,
    registry: &crate::resolve::TypeRegistry,
) -> Result<DynamicType, DynamicError> {
    use super::builder::DynamicTypeBuilderFactory as F;
    match ti {
        TypeIdentifier::Primitive(p) => {
            let kind = primitive_kind_to_type_kind(*p);
            F::get_primitive_type(kind)
        }
        TypeIdentifier::String8Small { bound } => Ok(F::create_string_type(u32::from(*bound))),
        TypeIdentifier::String8Large { bound } => Ok(F::create_string_type(*bound)),
        TypeIdentifier::String16Small { bound } => Ok(F::create_wstring_type(u32::from(*bound))),
        TypeIdentifier::String16Large { bound } => Ok(F::create_wstring_type(*bound)),
        // Plain (anonymous) collections: resolve the element FULLY and retain it
        // via `collection::{sequence_of,array_of}` so a composite element
        // (`sequence<Struct>`, `Struct[N]`) — and arbitrary nesting like
        // `sequence<sequence<Struct>>` — carries its members for the reflective
        // codec. Scalar / string / nested-collection elements resolve through the
        // same recursion.
        TypeIdentifier::PlainSequenceSmall { bound, element, .. } => {
            let elem = resolve_type_id(element, registry)?;
            Ok(collection::sequence_of(elem, u32::from(*bound)))
        }
        TypeIdentifier::PlainSequenceLarge { bound, element, .. } => {
            let elem = resolve_type_id(element, registry)?;
            Ok(collection::sequence_of(elem, *bound))
        }
        TypeIdentifier::PlainArraySmall {
            array_bounds,
            element,
            ..
        } => {
            let elem = resolve_type_id(element, registry)?;
            let dims = array_bounds.iter().map(|b| u32::from(*b)).collect();
            Ok(collection::array_of(elem, dims, "<arr>"))
        }
        TypeIdentifier::PlainArrayLarge {
            array_bounds,
            element,
            ..
        } => {
            let elem = resolve_type_id(element, registry)?;
            Ok(collection::array_of(elem, array_bounds.clone(), "<arr>"))
        }
        // Plain map: resolve BOTH key and value fully and retain them via
        // `collection::map_of` so a composite key/value carries its members.
        TypeIdentifier::PlainMapSmall {
            bound,
            key,
            element,
            ..
        } => {
            let k = resolve_type_id(key, registry)?;
            let v = resolve_type_id(element, registry)?;
            Ok(collection::map_of(k, v, u32::from(*bound), "<map>"))
        }
        TypeIdentifier::PlainMapLarge {
            bound,
            key,
            element,
            ..
        } => {
            let k = resolve_type_id(key, registry)?;
            let v = resolve_type_id(element, registry)?;
            Ok(collection::map_of(k, v, *bound, "<map>"))
        }
        TypeIdentifier::EquivalenceHashComplete(h) => {
            let c = registry.get_complete(h).ok_or_else(|| {
                DynamicError::unsupported("composite member: EquivalenceHash not in TypeRegistry")
            })?;
            resolve_complete(c, registry)
        }
        TypeIdentifier::EquivalenceHashMinimal(h) => {
            let m = registry.get_minimal(h).ok_or_else(|| {
                DynamicError::unsupported(
                    "minimal composite member: EquivalenceHash not in registry",
                )
            })?;
            let name = format!(
                "T_{:08x}",
                u32::from_be_bytes([h.0[0], h.0[1], h.0[2], h.0[3]])
            );
            resolve_minimal(m, registry, &name)
        }
        other => Err(DynamicError::unsupported(format!(
            "resolve_type_id: {other:?} not yet supported"
        ))),
    }
}

/// Builds a full `DynamicType` from a `CompleteTypeObject`, resolving composite
/// member types through `registry`. Handles all 10 Complete kinds.
/// zerodds-lint: recursion-depth 64 (runtime DynamicData codec; bounded by type nesting).
fn resolve_complete(
    c: &CompleteTypeObject,
    registry: &crate::resolve::TypeRegistry,
) -> Result<DynamicType, DynamicError> {
    use super::builder::DynamicTypeBuilderFactory as F;
    match c {
        CompleteTypeObject::Struct(s) => complete_struct_to_builder_in(s, registry)?.build(),
        CompleteTypeObject::Union(u) => {
            let disc_kind = type_id_to_kind(&u.discriminator.common.type_id)?;
            let disc_desc = type_id_to_descriptor(&u.discriminator.common.type_id, disc_kind)?;
            let mut b = F::create_type(TypeDescriptor::union(
                u.header.detail.type_name.clone(),
                disc_desc,
            ))?;
            for m in &u.member_seq {
                let mt = resolve_type_id(&m.common.type_id, registry)?;
                let mut md = MemberDescriptor::new(
                    m.detail.name.clone(),
                    m.common.member_id,
                    mt.descriptor().clone(),
                );
                md.label = m.common.label_seq.iter().map(|&v| i64::from(v)).collect();
                md.is_default_label = (m.common.member_flags.0 & UnionMemberFlag::IS_DEFAULT) != 0;
                b.add_member_resolved(md, mt)?;
            }
            b.build()
        }
        CompleteTypeObject::Enumerated(e) => {
            let mut desc = TypeDescriptor::enumeration(e.header.detail.type_name.clone());
            desc.bound = alloc::vec![u32::from(e.header.common.bit_bound)];
            let mut b = F::create_type(desc)?;
            for lit in &e.literal_seq {
                let id = u32::try_from(lit.common.value).unwrap_or(0);
                let mut md = MemberDescriptor::new(
                    lit.detail.name.clone(),
                    id,
                    TypeDescriptor::primitive(TypeKind::Int32, "int32"),
                );
                md.is_default_label =
                    (lit.common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL) != 0;
                b.add_member(md)?;
            }
            b.build()
        }
        CompleteTypeObject::Bitmask(bm) => {
            let mut desc = bare_descriptor(TypeKind::Bitmask, bm.detail.type_name.clone());
            desc.bound = alloc::vec![u32::from(bm.bit_bound)];
            let mut b = F::create_type(desc)?;
            for f in &bm.flag_seq {
                let md = MemberDescriptor::new(
                    f.detail.name.clone(),
                    u32::from(f.common.position),
                    TypeDescriptor::primitive(TypeKind::Boolean, "boolean"),
                );
                b.add_member(md)?;
            }
            b.build()
        }
        CompleteTypeObject::Bitset(bs) => {
            let desc = bare_descriptor(TypeKind::Bitset, bs.detail.type_name.clone());
            let mut b = F::create_type(desc)?;
            for f in &bs.field_seq {
                let holder = PrimitiveKind::from_u8(f.common.holder_type).ok_or_else(|| {
                    DynamicError::inconsistent(format!(
                        "bitset field holder_type {} is not a primitive",
                        f.common.holder_type
                    ))
                })?;
                let mut md = MemberDescriptor::new(
                    f.detail.name.clone(),
                    u32::from(f.common.position),
                    TypeDescriptor::primitive(primitive_kind_to_type_kind(holder), "holder"),
                );
                md.bit_bound = Some(f.common.bitcount);
                b.add_member(md)?;
            }
            b.build()
        }
        CompleteTypeObject::Alias(a) => {
            let target_kind = type_id_to_kind(&a.body.related_type)?;
            if matches!(
                target_kind,
                TypeKind::Structure
                    | TypeKind::Union
                    | TypeKind::Sequence
                    | TypeKind::Array
                    | TypeKind::Map
            ) {
                // A typedef has the same wire form as its target, so a composite /
                // collection target is resolved TRANSPARENTLY (fully, members
                // carried) rather than kept as a shallow alias node.
                resolve_type_id(&a.body.related_type, registry)
            } else {
                // Scalar / enum / bitmask target: a shallow alias node suffices
                // (the codec rebuilds the scalar target from the descriptor).
                let target = type_id_to_descriptor(&a.body.related_type, target_kind)?;
                let mut desc = bare_descriptor(TypeKind::Alias, a.header.detail.type_name.clone());
                desc.element_type = Some(alloc::boxed::Box::new(target));
                F::create_type(desc)?.build()
            }
        }
        CompleteTypeObject::Sequence(_)
        | CompleteTypeObject::Array(_)
        | CompleteTypeObject::Map(_) => {
            // Named (non-anonymous) collection TypeObject: resolve the element
            // fully through the registry so composite elements carry their members.
            resolve_named_collection(c, registry)
        }
        CompleteTypeObject::Annotation(an) => {
            let desc = bare_descriptor(TypeKind::Annotation, an.detail.type_name.clone());
            let mut b = F::create_type(desc)?;
            for m in &an.member_seq {
                let mk = type_id_to_kind(&m.member_type_id)?;
                let mt = type_id_to_descriptor(&m.member_type_id, mk)?;
                let mut md = MemberDescriptor::new(m.name.clone(), m.member_id, mt);
                if !m.default_value.is_empty() {
                    md.default_value = Some(String::from_utf8_lossy(&m.default_value).into_owned());
                }
                b.add_member(md)?;
            }
            b.build()
        }
    }
}

/// A stable, non-empty synthetic name for a Minimal member/type: its 4-byte
/// `NameHash` rendered as hex. A Minimal TypeObject carries no readable names by
/// XTypes design — only these hashes — so this is the best name recoverable from
/// a bare Minimal form (the Complete form, if co-published, has the real names).
fn hash_name(prefix: &str, h: &NameHash) -> String {
    format!("{prefix}_{:08x}", u32::from_be_bytes(h.0))
}

/// Builds a full `DynamicType` from a `MinimalTypeObject`. A Minimal object
/// carries the FULL structure — member ids, types, flags, nesting — in its
/// shared `common` fields (identical to Complete); only the readable member/type
/// NAMES are absent (4-byte hashes). Because the reflective codec keys on member
/// ids + types, a Minimal-resolved type encodes/decodes wire data byte-identically
/// to its Complete twin; only reflection shows hash-based names.
/// zerodds-lint: recursion-depth 64 (runtime DynamicData codec; bounded by type nesting).
fn resolve_minimal(
    m: &MinimalTypeObject,
    registry: &crate::resolve::TypeRegistry,
    type_name: &str,
) -> Result<DynamicType, DynamicError> {
    use super::builder::DynamicTypeBuilderFactory as F;
    match m {
        MinimalTypeObject::Struct(s) => {
            let mut desc = TypeDescriptor::structure(type_name.to_string());
            desc.extensibility_kind = flag_bits_to_extensibility(s.struct_flags.0);
            if (s.struct_flags.0 & StructTypeFlag::IS_NESTED) != 0 {
                desc.is_nested = true;
            }
            let mut b = F::create_type(desc)?;
            for (idx, mem) in s.member_seq.iter().enumerate() {
                let mt = resolve_type_id(&mem.common.member_type_id, registry)?;
                let mut md = MemberDescriptor::new(
                    hash_name("m", &mem.detail),
                    mem.common.member_id,
                    mt.descriptor().clone(),
                );
                md.index = u32::try_from(idx).unwrap_or(u32::MAX);
                md.is_key = (mem.common.member_flags.0 & StructMemberFlag::IS_KEY) != 0;
                md.is_optional = (mem.common.member_flags.0 & StructMemberFlag::IS_OPTIONAL) != 0;
                md.is_must_understand =
                    (mem.common.member_flags.0 & StructMemberFlag::IS_MUST_UNDERSTAND) != 0;
                md.is_shared = (mem.common.member_flags.0 & StructMemberFlag::IS_EXTERNAL) != 0;
                // @try_construct bits (§7.3.1.2.1.1). A Minimal TypeObject carries
                // no member default, so USE_DEFAULT falls back to DISCARD on apply.
                md.try_construct = try_construct_from_member_flags(mem.common.member_flags.0);
                b.add_member_resolved(md, mt)?;
            }
            b.build()
        }
        MinimalTypeObject::Union(u) => {
            let disc_kind = type_id_to_kind(&u.discriminator.common.type_id)?;
            let disc_desc = type_id_to_descriptor(&u.discriminator.common.type_id, disc_kind)?;
            let mut b = F::create_type(TypeDescriptor::union(type_name.to_string(), disc_desc))?;
            for mem in &u.member_seq {
                let mt = resolve_type_id(&mem.common.type_id, registry)?;
                let mut md = MemberDescriptor::new(
                    hash_name("m", &mem.detail),
                    mem.common.member_id,
                    mt.descriptor().clone(),
                );
                md.label = mem.common.label_seq.iter().map(|&v| i64::from(v)).collect();
                md.is_default_label =
                    (mem.common.member_flags.0 & UnionMemberFlag::IS_DEFAULT) != 0;
                b.add_member_resolved(md, mt)?;
            }
            b.build()
        }
        MinimalTypeObject::Enumerated(e) => {
            let mut desc = TypeDescriptor::enumeration(type_name.to_string());
            desc.bound = alloc::vec![u32::from(e.header.common.bit_bound)];
            let mut b = F::create_type(desc)?;
            for lit in &e.literal_seq {
                let id = u32::try_from(lit.common.value).unwrap_or(0);
                let mut md = MemberDescriptor::new(
                    hash_name("e", &lit.detail),
                    id,
                    TypeDescriptor::primitive(TypeKind::Int32, "int32"),
                );
                md.is_default_label =
                    (lit.common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL) != 0;
                b.add_member(md)?;
            }
            b.build()
        }
        MinimalTypeObject::Bitmask(bm) => {
            let mut desc = bare_descriptor(TypeKind::Bitmask, type_name.to_string());
            desc.bound = alloc::vec![u32::from(bm.bit_bound)];
            let mut b = F::create_type(desc)?;
            for f in &bm.flag_seq {
                let md = MemberDescriptor::new(
                    hash_name("f", &f.detail),
                    u32::from(f.common.position),
                    TypeDescriptor::primitive(TypeKind::Boolean, "boolean"),
                );
                b.add_member(md)?;
            }
            b.build()
        }
        MinimalTypeObject::Bitset(bs) => {
            let desc = bare_descriptor(TypeKind::Bitset, type_name.to_string());
            let mut b = F::create_type(desc)?;
            for f in &bs.field_seq {
                let holder = PrimitiveKind::from_u8(f.common.holder_type).ok_or_else(|| {
                    DynamicError::inconsistent(format!(
                        "bitset field holder_type {} is not a primitive",
                        f.common.holder_type
                    ))
                })?;
                let mut md = MemberDescriptor::new(
                    hash_name("f", &f.name_hash),
                    u32::from(f.common.position),
                    TypeDescriptor::primitive(primitive_kind_to_type_kind(holder), "holder"),
                );
                md.bit_bound = Some(f.common.bitcount);
                b.add_member(md)?;
            }
            b.build()
        }
        MinimalTypeObject::Alias(a) => {
            // Typedefs are transparent (same wire form as their target).
            resolve_type_id(&a.body.common.related_type, registry)
        }
        MinimalTypeObject::Sequence(s) => {
            let elem = resolve_type_id(&s.element.common.type_id, registry)?;
            Ok(collection::sequence_named(type_name, elem, s.bound))
        }
        MinimalTypeObject::Array(a) => {
            let elem = resolve_type_id(&a.element.common.type_id, registry)?;
            Ok(collection::array_of(elem, a.bound_seq.clone(), type_name))
        }
        MinimalTypeObject::Map(mp) => {
            let key = resolve_type_id(&mp.key.common.type_id, registry)?;
            let value = resolve_type_id(&mp.element.common.type_id, registry)?;
            Ok(collection::map_of(key, value, mp.bound, type_name))
        }
        MinimalTypeObject::Annotation(_) => Err(DynamicError::unsupported(
            "minimal annotation type → dynamic-type (annotations are metadata, not data members)",
        )),
    }
}

/// Like [`complete_struct_to_builder`] but resolves each member's type fully
/// (recursively through `registry`) and attaches it via `add_member_resolved`,
/// so nested composite members carry their own members.
fn complete_struct_to_builder_in(
    s: &CompleteStructType,
    registry: &crate::resolve::TypeRegistry,
) -> Result<DynamicTypeBuilder, DynamicError> {
    let mut desc = TypeDescriptor::structure(s.header.detail.type_name.clone());
    desc.extensibility_kind = flag_bits_to_extensibility(s.struct_flags.0);
    if (s.struct_flags.0 & StructTypeFlag::IS_NESTED) != 0 {
        desc.is_nested = true;
    }
    let mut b = DynamicTypeBuilderFactory::create_type(desc)?;
    for (idx, m) in s.member_seq.iter().enumerate() {
        let mt = resolve_type_id(&m.common.member_type_id, registry)?;
        let mut md = MemberDescriptor::new(
            m.detail.name.clone(),
            m.common.member_id,
            mt.descriptor().clone(),
        );
        md.index = u32::try_from(idx).unwrap_or(u32::MAX);
        md.is_key = (m.common.member_flags.0 & StructMemberFlag::IS_KEY) != 0;
        md.is_optional = (m.common.member_flags.0 & StructMemberFlag::IS_OPTIONAL) != 0;
        md.is_must_understand =
            (m.common.member_flags.0 & StructMemberFlag::IS_MUST_UNDERSTAND) != 0;
        md.is_shared = (m.common.member_flags.0 & StructMemberFlag::IS_EXTERNAL) != 0;
        // @try_construct bits + member default (§7.3.1.2.1.1 / §7.2.4.4.4.4.9),
        // so a USE_DEFAULT reader can recover the default on apply.
        md.try_construct = try_construct_from_member_flags(m.common.member_flags.0);
        md.default_value = m.detail.ann_builtin.default_value.clone();
        b.add_member_resolved(md, mt)?;
    }
    Ok(b)
}

/// Builds a top-level named collection (Sequence/Array) `CompleteTypeObject`
/// into a `DynamicType`, resolving the element fully through `registry` (so a
/// composite element carries its members via `collection::{sequence_named,
/// array_of,map_of}`), so composite keys/values/elements carry their members.
fn resolve_named_collection(
    c: &CompleteTypeObject,
    registry: &crate::resolve::TypeRegistry,
) -> Result<DynamicType, DynamicError> {
    match c {
        CompleteTypeObject::Sequence(s) => {
            let elem = resolve_type_id(&s.element.common.type_id, registry)?;
            Ok(collection::sequence_named(
                &s.detail.type_name,
                elem,
                s.bound,
            ))
        }
        CompleteTypeObject::Array(a) => {
            let elem = resolve_type_id(&a.element.common.type_id, registry)?;
            Ok(collection::array_of(
                elem,
                a.bound_seq.clone(),
                &a.detail.type_name,
            ))
        }
        CompleteTypeObject::Map(m) => {
            let key = resolve_type_id(&m.key.common.type_id, registry)?;
            let value = resolve_type_id(&m.element.common.type_id, registry)?;
            Ok(collection::map_of(key, value, m.bound, &m.detail.type_name))
        }
        other => Err(DynamicError::unsupported(format!(
            "resolve_named_collection called on non-collection: {other:?}"
        ))),
    }
}

/// A bare `TypeDescriptor` carrying just `kind` + `name` (all structural fields
/// empty) — for the non-collection composite kinds whose body lives in their
/// members (Bitmask/Bitset/Annotation) or in `element_type` (Alias), set by the
/// caller after construction.
fn bare_descriptor(kind: TypeKind, name: impl Into<String>) -> TypeDescriptor {
    TypeDescriptor {
        kind,
        name: name.into(),
        base_type: None,
        discriminator_type: None,
        bound: Vec::new(),
        element_type: None,
        key_element_type: None,
        extensibility_kind: ExtensibilityKind::default(),
        is_nested: false,
    }
}

fn complete_struct_to_builder(s: &CompleteStructType) -> Result<DynamicTypeBuilder, DynamicError> {
    let mut desc = TypeDescriptor::structure(s.header.detail.type_name.clone());
    desc.extensibility_kind = flag_bits_to_extensibility(s.struct_flags.0);
    if (s.struct_flags.0 & StructTypeFlag::IS_NESTED) != 0 {
        desc.is_nested = true;
    }
    let mut b = DynamicTypeBuilderFactory::create_type(desc)?;
    for (idx, m) in s.member_seq.iter().enumerate() {
        let kind = type_id_to_kind(&m.common.member_type_id)?;
        let member_type = type_id_to_descriptor(&m.common.member_type_id, kind)?;
        let mut md = MemberDescriptor::new(m.detail.name.clone(), m.common.member_id, member_type);
        md.index = u32::try_from(idx).unwrap_or(u32::MAX);
        md.is_key = (m.common.member_flags.0 & StructMemberFlag::IS_KEY) != 0;
        md.is_optional = (m.common.member_flags.0 & StructMemberFlag::IS_OPTIONAL) != 0;
        md.is_must_understand =
            (m.common.member_flags.0 & StructMemberFlag::IS_MUST_UNDERSTAND) != 0;
        md.is_shared = (m.common.member_flags.0 & StructMemberFlag::IS_EXTERNAL) != 0;
        // @try_construct bits + member default (§7.3.1.2.1.1 / §7.2.4.4.4.4.9).
        md.try_construct = try_construct_from_member_flags(m.common.member_flags.0);
        md.default_value = m.detail.ann_builtin.default_value.clone();
        b.add_member(md)?;
    }
    Ok(b)
}

const fn other_kind_name(c: &CompleteTypeObject) -> &'static str {
    match c {
        CompleteTypeObject::Alias(_) => "alias",
        CompleteTypeObject::Annotation(_) => "annotation",
        CompleteTypeObject::Struct(_) => "struct",
        CompleteTypeObject::Union(_) => "union",
        CompleteTypeObject::Bitset(_) => "bitset",
        CompleteTypeObject::Sequence(_) => "sequence",
        CompleteTypeObject::Array(_) => "array",
        CompleteTypeObject::Map(_) => "map",
        CompleteTypeObject::Enumerated(_) => "enum",
        CompleteTypeObject::Bitmask(_) => "bitmask",
    }
}

fn type_id_to_kind(ti: &TypeIdentifier) -> Result<TypeKind, DynamicError> {
    Ok(match ti {
        TypeIdentifier::None => TypeKind::NoType,
        TypeIdentifier::Primitive(p) => primitive_kind_to_type_kind(*p),
        TypeIdentifier::String8Small { .. } | TypeIdentifier::String8Large { .. } => {
            TypeKind::String8
        }
        TypeIdentifier::String16Small { .. } | TypeIdentifier::String16Large { .. } => {
            TypeKind::String16
        }
        TypeIdentifier::PlainSequenceSmall { .. } | TypeIdentifier::PlainSequenceLarge { .. } => {
            TypeKind::Sequence
        }
        TypeIdentifier::PlainArraySmall { .. } | TypeIdentifier::PlainArrayLarge { .. } => {
            TypeKind::Array
        }
        TypeIdentifier::PlainMapSmall { .. } | TypeIdentifier::PlainMapLarge { .. } => {
            TypeKind::Map
        }
        TypeIdentifier::EquivalenceHashMinimal(_) | TypeIdentifier::EquivalenceHashComplete(_) => {
            // Composite via TypeRegistry lookup — mark as Structure
            // by default; resolving it correctly is C4.2.
            TypeKind::Structure
        }
        other => {
            return Err(DynamicError::unsupported(format!(
                "type_id_to_kind: {other:?} not yet supported"
            )));
        }
    })
}

const fn primitive_kind_to_type_kind(p: PrimitiveKind) -> TypeKind {
    match p {
        PrimitiveKind::Boolean => TypeKind::Boolean,
        PrimitiveKind::Byte => TypeKind::Byte,
        PrimitiveKind::Int8 => TypeKind::Int8,
        PrimitiveKind::UInt8 => TypeKind::UInt8,
        PrimitiveKind::Int16 => TypeKind::Int16,
        PrimitiveKind::UInt16 => TypeKind::UInt16,
        PrimitiveKind::Int32 => TypeKind::Int32,
        PrimitiveKind::UInt32 => TypeKind::UInt32,
        PrimitiveKind::Int64 => TypeKind::Int64,
        PrimitiveKind::UInt64 => TypeKind::UInt64,
        PrimitiveKind::Float32 => TypeKind::Float32,
        PrimitiveKind::Float64 => TypeKind::Float64,
        PrimitiveKind::Float128 => TypeKind::Float128,
        PrimitiveKind::Char8 => TypeKind::Char8,
        PrimitiveKind::Char16 => TypeKind::Char16,
    }
}

/// zerodds-lint: recursion-depth 16
///
/// Recursive calls via the PlainSequence/PlainArray element. Depth is
/// implicitly capped by [`TypeIdentifier::MAX_DECODE_DEPTH`] — the
/// TypeIdentifier can be at most 16 layers deep on wire-decode, after
/// which decoding fails with `LengthExceeded`.
fn type_id_to_descriptor(
    ti: &TypeIdentifier,
    kind: TypeKind,
) -> Result<TypeDescriptor, DynamicError> {
    if kind.is_primitive() {
        return Ok(TypeDescriptor::primitive(
            kind,
            super::type_::primitive_name(kind).to_string(),
        ));
    }
    Ok(match ti {
        TypeIdentifier::String8Small { bound } => TypeDescriptor::string8(u32::from(*bound)),
        TypeIdentifier::String8Large { bound } => TypeDescriptor::string8(*bound),
        TypeIdentifier::String16Small { bound } => TypeDescriptor::string16(u32::from(*bound)),
        TypeIdentifier::String16Large { bound } => TypeDescriptor::string16(*bound),
        TypeIdentifier::None => TypeDescriptor {
            kind: TypeKind::Structure,
            name: String::from("<unresolved>"),
            base_type: None,
            discriminator_type: None,
            bound: Vec::new(),
            element_type: None,
            key_element_type: None,
            extensibility_kind: ExtensibilityKind::default(),
            is_nested: false,
        },
        TypeIdentifier::EquivalenceHashMinimal(_) | TypeIdentifier::EquivalenceHashComplete(_) => {
            // We only know the name after a TypeRegistry lookup
            // (C4.2). Until then: placeholder descriptor with Kind=Structure.
            TypeDescriptor {
                kind: TypeKind::Structure,
                name: String::from("<typeref>"),
                base_type: None,
                discriminator_type: None,
                bound: Vec::new(),
                element_type: None,
                key_element_type: None,
                extensibility_kind: ExtensibilityKind::default(),
                is_nested: false,
            }
        }
        TypeIdentifier::PlainSequenceSmall { bound, element, .. } => {
            let elem_kind = type_id_to_kind(element)?;
            TypeDescriptor::sequence(
                "<seq>".to_string(),
                type_id_to_descriptor(element, elem_kind)?,
                u32::from(*bound),
            )
        }
        TypeIdentifier::PlainSequenceLarge { bound, element, .. } => {
            let elem_kind = type_id_to_kind(element)?;
            TypeDescriptor::sequence(
                "<seq>".to_string(),
                type_id_to_descriptor(element, elem_kind)?,
                *bound,
            )
        }
        TypeIdentifier::PlainArraySmall {
            array_bounds,
            element,
            ..
        } => {
            let elem_kind = type_id_to_kind(element)?;
            TypeDescriptor::array(
                "<arr>".to_string(),
                type_id_to_descriptor(element, elem_kind)?,
                array_bounds.iter().map(|b| u32::from(*b)).collect(),
            )
        }
        TypeIdentifier::PlainArrayLarge {
            array_bounds,
            element,
            ..
        } => {
            let elem_kind = type_id_to_kind(element)?;
            TypeDescriptor::array(
                "<arr>".to_string(),
                type_id_to_descriptor(element, elem_kind)?,
                array_bounds.clone(),
            )
        }
        other => {
            return Err(DynamicError::unsupported(format!(
                "type_id_to_descriptor: {other:?} not yet supported"
            )));
        }
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;
    use crate::dynamic::DynamicTypeBuilderFactory;

    fn build_int_struct() -> DynamicType {
        let mut b = DynamicTypeBuilderFactory::create_struct("::Sensor");
        b.add_struct_member("id", 1, TypeDescriptor::primitive(TypeKind::Int64, "int64"))
            .unwrap();
        b.add_struct_member(
            "temp",
            2,
            TypeDescriptor::primitive(TypeKind::Float64, "double"),
        )
        .unwrap();
        b.add_struct_member("name", 3, TypeDescriptor::string8(64))
            .unwrap();
        b.build().unwrap()
    }

    #[test]
    fn dynamic_struct_to_typeobject_complete() {
        let t = build_int_struct();
        let to = t.to_type_object().unwrap();
        match to {
            TypeObject::Complete(CompleteTypeObject::Struct(s)) => {
                assert_eq!(s.header.detail.type_name, "::Sensor");
                assert_eq!(s.member_seq.len(), 3);
                assert_eq!(s.member_seq[0].detail.name, "id");
            }
            _ => panic!("to_type_object on Structure must return Complete::Struct"),
        }
    }

    #[test]
    fn typeobject_complete_struct_back_to_dynamic_type() {
        let t = build_int_struct();
        let to = t.to_type_object().unwrap();
        let b = DynamicTypeBuilderFactory::create_type_w_type_object(&to).unwrap();
        let t2 = b.build().unwrap();
        assert_eq!(t2.name(), "::Sensor");
        assert_eq!(t2.member_count(), 3);
        assert_eq!(t2.member_by_id(1).unwrap().name(), "id");
        assert_eq!(t2.member_by_id(2).unwrap().name(), "temp");
        assert_eq!(t2.member_by_id(3).unwrap().name(), "name");
    }

    #[test]
    fn roundtrip_dynamic_to_typeobject_to_dynamic_equals_logically() {
        let t = build_int_struct();
        let to = t.to_type_object().unwrap();
        let b = DynamicTypeBuilderFactory::create_type_w_type_object(&to).unwrap();
        let t2 = b.build().unwrap();
        // Deep equality on the type built by the bridge.
        assert!(t.equals(&t2), "roundtrip failed: {t:?} vs {t2:?}");
    }

    #[test]
    fn decode_reads_try_construct_bits_from_complete_typeobject() {
        use crate::builder::{TryConstruct, TypeObjectBuilder};
        // Three struct members, one per TryConstructKind, decoded through the
        // TypeObject → DynamicType bridge (§7.3.1.2.1.1).
        let co = TypeObjectBuilder::struct_type("::T")
            .member("keep", TypeIdentifier::String8Small { bound: 5 }, |m| {
                m.try_construct(TryConstruct::Trim)
            })
            .member("drop", TypeIdentifier::String8Small { bound: 5 }, |m| m)
            .member("fill", TypeIdentifier::String8Small { bound: 5 }, |m| {
                m.try_construct(TryConstruct::UseDefault)
                    .set_member_default("dflt")
            })
            .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Struct(co));
        let dt = DynamicTypeBuilderFactory::create_type_w_type_object(&to)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            dt.member_by_id(0).unwrap().descriptor().try_construct,
            TryConstructKind::Trim
        );
        assert_eq!(
            dt.member_by_id(1).unwrap().descriptor().try_construct,
            TryConstructKind::Discard
        );
        assert_eq!(
            dt.member_by_id(2).unwrap().descriptor().try_construct,
            TryConstructKind::UseDefault
        );
        // The complete TypeObject also carries the member default for USE_DEFAULT.
        assert_eq!(
            dt.member_by_id(2)
                .unwrap()
                .descriptor()
                .default_value
                .as_deref(),
            Some("dflt")
        );
    }

    #[test]
    fn decode_applies_try_construct_on_over_bound_set() {
        use crate::builder::{TryConstruct, TypeObjectBuilder};
        use crate::dynamic::DynamicData;
        // End-to-end: decode a TypeObject, then drive the apply path (data.rs
        // set → apply_try_construct) with an over-bound string per behaviour.
        let co = TypeObjectBuilder::struct_type("::T")
            .member("trim_it", TypeIdentifier::String8Small { bound: 5 }, |m| {
                m.try_construct(TryConstruct::Trim)
            })
            .member("drop_it", TypeIdentifier::String8Small { bound: 5 }, |m| {
                m.try_construct(TryConstruct::Discard)
            })
            .member(
                "default_it",
                TypeIdentifier::String8Small { bound: 5 },
                |m| {
                    m.try_construct(TryConstruct::UseDefault)
                        .set_member_default("dflt")
                },
            )
            .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Struct(co));
        let dt = DynamicTypeBuilderFactory::create_type_w_type_object(&to)
            .unwrap()
            .build()
            .unwrap();
        let mut d = DynamicData::new(dt);
        // TRIM: the 12-byte value is truncated to the 5-byte bound.
        d.set_string_value(0, "toolongvalue").unwrap();
        assert_eq!(d.get_string_value(0).unwrap(), "toolo");
        // DISCARD: set returns Ok but the member stays unset.
        d.set_string_value(1, "toolongvalue").unwrap();
        assert!(d.get_string_value(1).is_err());
        // USE_DEFAULT: the member takes the declared default.
        d.set_string_value(2, "toolongvalue").unwrap();
        assert_eq!(d.get_string_value(2).unwrap(), "dflt");
    }

    #[test]
    fn unsupported_kind_to_typeobject_yields_unsupported_error() {
        let prim = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int32).unwrap();
        let err = prim.to_type_object().unwrap_err();
        assert!(matches!(err, DynamicError::Unsupported(_)));
    }

    #[test]
    fn dynamic_alias_to_typeobject_complete() {
        // alias with element_type = int32.
        let mut desc = TypeDescriptor::primitive(TypeKind::Int32, "int32");
        desc.kind = TypeKind::Alias;
        desc.name = "::SensorId".to_string();
        desc.element_type = Some(alloc::boxed::Box::new(TypeDescriptor::primitive(
            TypeKind::Int32,
            "int32",
        )));
        let t = DynamicTypeBuilderFactory::create_type(desc)
            .unwrap()
            .build()
            .unwrap();
        let to = t.to_type_object().expect("alias bridge ok");
        match to {
            TypeObject::Complete(CompleteTypeObject::Alias(a)) => {
                assert_eq!(a.header.detail.type_name, "::SensorId");
                assert!(matches!(a.body.related_type, TypeIdentifier::Primitive(_)));
            }
            other => panic!("expected Alias, got {other:?}"),
        }
    }

    #[test]
    fn dynamic_enum_to_typeobject_complete() {
        let mut desc = TypeDescriptor::primitive(TypeKind::Enumeration, "::Color");
        desc.kind = TypeKind::Enumeration;
        desc.bound = alloc::vec![32];
        let mut b = DynamicTypeBuilderFactory::create_type(desc).unwrap();
        for (id, name) in [(0u32, "RED"), (1u32, "GREEN"), (2u32, "BLUE")] {
            let m = MemberDescriptor::new(
                name,
                id,
                TypeDescriptor::primitive(TypeKind::Int32, "int32"),
            );
            b.add_member(m).unwrap();
        }
        let t = b.build().unwrap();
        let to = t.to_type_object().expect("enum bridge ok");
        match to {
            TypeObject::Complete(CompleteTypeObject::Enumerated(e)) => {
                assert_eq!(e.literal_seq.len(), 3);
                assert_eq!(e.literal_seq[0].detail.name, "RED");
                assert_eq!(e.literal_seq[2].common.value, 2);
            }
            other => panic!("expected Enumerated, got {other:?}"),
        }
    }

    #[test]
    fn dynamic_bitmask_to_typeobject_complete() {
        let mut desc = TypeDescriptor::primitive(TypeKind::Bitmask, "::Flags");
        desc.kind = TypeKind::Bitmask;
        desc.bound = alloc::vec![8];
        let mut b = DynamicTypeBuilderFactory::create_type(desc).unwrap();
        for (pos, name) in [(0u32, "A"), (3u32, "D"), (7u32, "H")] {
            let m = MemberDescriptor::new(
                name,
                pos,
                TypeDescriptor::primitive(TypeKind::Boolean, "boolean"),
            );
            b.add_member(m).unwrap();
        }
        let t = b.build().unwrap();
        let to = t.to_type_object().expect("bitmask bridge ok");
        match to {
            TypeObject::Complete(CompleteTypeObject::Bitmask(bm)) => {
                assert_eq!(bm.bit_bound, 8);
                assert_eq!(bm.flag_seq.len(), 3);
                assert_eq!(bm.flag_seq[1].common.position, 3);
            }
            other => panic!("expected Bitmask, got {other:?}"),
        }
    }

    #[test]
    fn dynamic_bitset_to_typeobject_complete() {
        let mut desc = TypeDescriptor::primitive(TypeKind::Bitset, "::Reg");
        desc.kind = TypeKind::Bitset;
        let mut b = DynamicTypeBuilderFactory::create_type(desc).unwrap();
        for (pos, name, width) in [(0u32, "lo", 3u8), (3u32, "hi", 5u8)] {
            let mut m = MemberDescriptor::new(
                name,
                pos,
                TypeDescriptor::primitive(TypeKind::UInt8, "uint8"),
            );
            m.bit_bound = Some(width);
            b.add_member(m).unwrap();
        }
        let t = b.build().unwrap();
        let to = t.to_type_object().expect("bitset bridge ok");
        match to {
            TypeObject::Complete(CompleteTypeObject::Bitset(bs)) => {
                assert_eq!(bs.field_seq.len(), 2);
                assert_eq!(bs.field_seq[0].common.position, 0);
                assert_eq!(bs.field_seq[0].common.bitcount, 3);
                assert_eq!(bs.field_seq[0].detail.name, "lo");
                assert_eq!(bs.field_seq[1].common.position, 3);
                assert_eq!(bs.field_seq[1].common.bitcount, 5);
                assert_eq!(
                    bs.field_seq[1].common.holder_type,
                    PrimitiveKind::UInt8.to_u8()
                );
            }
            other => panic!("expected Bitset, got {other:?}"),
        }
    }

    #[test]
    fn dynamic_annotation_to_typeobject_complete() {
        let mut desc = TypeDescriptor::primitive(TypeKind::Annotation, "::Range");
        desc.kind = TypeKind::Annotation;
        let mut b = DynamicTypeBuilderFactory::create_type(desc).unwrap();
        let mut m = MemberDescriptor::new(
            "max",
            1,
            TypeDescriptor::primitive(TypeKind::Int32, "int32"),
        );
        m.default_value = Some("100".into());
        b.add_member(m).unwrap();
        let t = b.build().unwrap();
        let to = t.to_type_object().expect("annotation bridge ok");
        match to {
            TypeObject::Complete(CompleteTypeObject::Annotation(a)) => {
                assert_eq!(a.member_seq.len(), 1);
                assert_eq!(a.member_seq[0].name, "max");
                assert_eq!(a.member_seq[0].member_id, 1);
                assert_eq!(a.member_seq[0].default_value, b"100".to_vec());
                assert!(matches!(
                    a.member_seq[0].member_type_id,
                    TypeIdentifier::Primitive(PrimitiveKind::Int32)
                ));
            }
            other => panic!("expected Annotation, got {other:?}"),
        }
    }

    #[test]
    fn dynamic_sequence_to_typeobject_complete() {
        let desc = TypeDescriptor::sequence(
            "::Seq",
            TypeDescriptor::primitive(TypeKind::Int32, "int32"),
            10,
        );
        let t = DynamicTypeBuilderFactory::create_type(desc)
            .unwrap()
            .build()
            .unwrap();
        match t.to_type_object().expect("sequence bridge ok") {
            TypeObject::Complete(CompleteTypeObject::Sequence(s)) => {
                assert_eq!(s.bound, 10);
                assert_eq!(s.detail.type_name, "::Seq");
                assert!(matches!(
                    s.element.common.type_id,
                    TypeIdentifier::Primitive(PrimitiveKind::Int32)
                ));
            }
            other => panic!("expected Sequence, got {other:?}"),
        }
    }

    #[test]
    fn dynamic_array_to_typeobject_complete() {
        let desc = TypeDescriptor::array(
            "::Arr",
            TypeDescriptor::primitive(TypeKind::Float64, "double"),
            alloc::vec![3, 4],
        );
        let t = DynamicTypeBuilderFactory::create_type(desc)
            .unwrap()
            .build()
            .unwrap();
        match t.to_type_object().expect("array bridge ok") {
            TypeObject::Complete(CompleteTypeObject::Array(a)) => {
                assert_eq!(a.bound_seq, alloc::vec![3, 4]);
                assert!(matches!(
                    a.element.common.type_id,
                    TypeIdentifier::Primitive(PrimitiveKind::Float64)
                ));
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    #[test]
    fn dynamic_map_to_typeobject_complete() {
        let desc = TypeDescriptor::map(
            "::Map",
            TypeDescriptor::primitive(TypeKind::Int32, "int32"),
            TypeDescriptor::primitive(TypeKind::Boolean, "boolean"),
            7,
        );
        let t = DynamicTypeBuilderFactory::create_type(desc)
            .unwrap()
            .build()
            .unwrap();
        match t.to_type_object().expect("map bridge ok") {
            TypeObject::Complete(CompleteTypeObject::Map(m)) => {
                assert_eq!(m.bound, 7);
                assert!(matches!(
                    m.key.common.type_id,
                    TypeIdentifier::Primitive(PrimitiveKind::Int32)
                ));
                assert!(matches!(
                    m.element.common.type_id,
                    TypeIdentifier::Primitive(PrimitiveKind::Boolean)
                ));
            }
            other => panic!("expected Map, got {other:?}"),
        }
    }

    #[test]
    fn dynamic_union_to_typeobject_complete() {
        let disc = TypeDescriptor::primitive(TypeKind::Int32, "int32");
        let t = {
            let mut b = DynamicTypeBuilderFactory::create_union("::Shape", disc).unwrap();
            let mut m1 = MemberDescriptor::new(
                "circle",
                1,
                TypeDescriptor::primitive(TypeKind::Float64, "double"),
            );
            m1.label = alloc::vec![1];
            let mut m2 = MemberDescriptor::new(
                "square",
                2,
                TypeDescriptor::primitive(TypeKind::Float64, "double"),
            );
            m2.label = alloc::vec![2, 3];
            b.add_member(m1).unwrap();
            b.add_member(m2).unwrap();
            b.build().unwrap()
        };
        let to = t.to_type_object().expect("union bridge ok");
        match to {
            TypeObject::Complete(CompleteTypeObject::Union(u)) => {
                assert_eq!(u.member_seq.len(), 2);
                assert_eq!(u.member_seq[0].detail.name, "circle");
                assert_eq!(u.member_seq[1].common.label_seq, alloc::vec![2, 3]);
                assert!(matches!(
                    u.discriminator.common.type_id,
                    TypeIdentifier::Primitive(PrimitiveKind::Int32)
                ));
            }
            other => panic!("expected Union, got {other:?}"),
        }
    }

    #[test]
    fn create_type_w_minimal_typeobject_bare_needs_registry() {
        // The BARE overload has no registry to resolve nested minimal hashes, so
        // it still declines Minimal — callers use the registry-aware `_in` form.
        use crate::type_object::minimal::{MinimalStructHeader, MinimalStructType};
        let m = MinimalStructType {
            struct_flags: StructTypeFlag::default(),
            header: MinimalStructHeader {
                base_type: TypeIdentifier::None,
            },
            member_seq: alloc::vec![],
        };
        let to = TypeObject::Minimal(MinimalTypeObject::Struct(m));
        let err = DynamicTypeBuilderFactory::create_type_w_type_object(&to).unwrap_err();
        assert!(matches!(err, DynamicError::Unsupported(_)));
    }

    /// A Minimal TypeObject carries the FULL structure (member ids + types) — only
    /// the readable names are hashes. So a Minimal-resolved type encodes/decodes
    /// wire data BYTE-IDENTICALLY to its Complete twin; only reflection differs.
    #[test]
    fn minimal_resolves_wire_identical_to_complete() {
        use crate::builder::TypeObjectBuilder;
        use crate::dynamic::DynamicData;
        use crate::dynamic::codec::{decode_dynamic, encode_dynamic};

        let reg = crate::resolve::TypeRegistry::new();
        let sb = TypeObjectBuilder::struct_type("::Point")
            .member("x", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("y", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m);
        let complete = TypeObject::Complete(CompleteTypeObject::Struct(sb.build_complete()));
        let minimal = TypeObject::Minimal(MinimalTypeObject::Struct(sb.build_minimal()));

        let ct = DynamicTypeBuilderFactory::create_type_w_type_object_in(&complete, &reg).unwrap();
        let mt = DynamicTypeBuilderFactory::create_type_w_type_object_in(&minimal, &reg).unwrap();

        assert_eq!(mt.kind(), TypeKind::Structure);
        assert_eq!(mt.member_count(), 2, "minimal carries the full structure");
        assert!(
            mt.member_by_name("x").is_none(),
            "minimal has hash names, not the readable 'x'"
        );

        // Member IDs are identical between the forms → identical wire encoding.
        let cx = ct.member_by_name("x").unwrap().id();
        let cy = ct.member_by_name("y").unwrap().id();
        let mut dc = DynamicData::new(ct.clone());
        dc.set_int32_value(cx, 11).unwrap();
        dc.set_int32_value(cy, 22).unwrap();
        let mut dm = DynamicData::new(mt.clone());
        dm.set_int32_value(cx, 11).unwrap();
        dm.set_int32_value(cy, 22).unwrap();

        for xcdr2 in [true, false] {
            let bc = encode_dynamic(&dc, xcdr2, false).unwrap();
            let bm = encode_dynamic(&dm, xcdr2, false).unwrap();
            assert_eq!(
                bc, bm,
                "minimal-resolved encodes byte-identically to complete xcdr2={xcdr2}"
            );
            let back = decode_dynamic(&mt, &bm, xcdr2, false).unwrap();
            assert_eq!(back.get_int32_value(cx).unwrap(), 11);
        }
    }

    /// A Minimal struct member referenced by `EquivalenceHashMinimal` resolves
    /// fully through the registry (the old "pending C4.2" residual).
    #[test]
    fn minimal_nested_member_resolves_via_registry() {
        use crate::builder::TypeObjectBuilder;
        use crate::resolve::TypeRegistry;
        use crate::type_identifier::EquivalenceHash;

        let inner = TypeObjectBuilder::struct_type("::Point")
            .member("x", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal();
        let h = EquivalenceHash([0x7; 14]);
        let mut reg = TypeRegistry::new();
        reg.insert_minimal(h, MinimalTypeObject::Struct(inner));

        let outer = TypeObjectBuilder::struct_type("::Line")
            .member("p", TypeIdentifier::EquivalenceHashMinimal(h), |m| m)
            .build_minimal();
        let to = TypeObject::Minimal(MinimalTypeObject::Struct(outer));
        let dt = DynamicTypeBuilderFactory::create_type_w_type_object_in(&to, &reg).unwrap();

        assert_eq!(dt.member_count(), 1);
        let p = dt.member_by_index(0).unwrap();
        assert_eq!(
            p.dynamic_type().kind(),
            TypeKind::Structure,
            "nested minimal struct resolved fully"
        );
        assert_eq!(p.dynamic_type().member_count(), 1);
    }

    #[test]
    fn resolve_nested_struct_member_fully_via_registry() {
        use crate::builder::TypeObjectBuilder;
        use crate::resolve::TypeRegistry;
        use crate::type_identifier::EquivalenceHash;

        // inner: Point { x: int32, y: int32 }
        let inner = TypeObjectBuilder::struct_type("::Point")
            .member("x", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("y", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_complete();
        let h = EquivalenceHash([0x42; 14]);
        let mut reg = TypeRegistry::new();
        reg.insert_complete(h, CompleteTypeObject::Struct(inner));

        // outer: Line { start: Point (by hash), len: int32 }
        let outer = TypeObjectBuilder::struct_type("::Line")
            .member("start", TypeIdentifier::EquivalenceHashComplete(h), |m| m)
            .member(
                "len",
                TypeIdentifier::Primitive(PrimitiveKind::Int32),
                |m| m,
            )
            .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Struct(outer));

        let dt = DynamicTypeBuilderFactory::create_type_w_type_object_in(&to, &reg).unwrap();
        assert_eq!(dt.member_count(), 2);
        let start = dt.member_by_name("start").expect("start member");
        // The nested Point is FULLY resolved (not a shallow <typeref> placeholder).
        assert_eq!(start.dynamic_type().kind(), TypeKind::Structure);
        assert_eq!(start.dynamic_type().member_count(), 2);
        assert!(start.dynamic_type().member_by_name("x").is_some());
        assert!(start.dynamic_type().member_by_name("y").is_some());
        // A missing-hash reference resolves to an honest Unsupported, not a panic.
        let dangling = TypeObjectBuilder::struct_type("::Bad")
            .member(
                "ref",
                TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([0xFF; 14])),
                |m| m,
            )
            .build_complete();
        let bad = TypeObject::Complete(CompleteTypeObject::Struct(dangling));
        assert!(matches!(
            DynamicTypeBuilderFactory::create_type_w_type_object_in(&bad, &reg),
            Err(DynamicError::Unsupported(_))
        ));
    }

    // ------------------------------------------------------------------
    // O3: composite-in-collection — the element of a collection is itself a
    // composite and must resolve to a fully-populated DynamicType (its members
    // carried), not a shallow name-only descriptor.
    // ------------------------------------------------------------------

    /// Registers `::Point { x: int32, y: int32 }` and returns `(registry, hash)`.
    fn point_registry() -> (
        crate::resolve::TypeRegistry,
        crate::type_identifier::EquivalenceHash,
    ) {
        use crate::builder::TypeObjectBuilder;
        use crate::resolve::TypeRegistry;
        use crate::type_identifier::EquivalenceHash;
        let point = TypeObjectBuilder::struct_type("::Point")
            .member("x", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("y", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_complete();
        let h = EquivalenceHash([0x11; 14]);
        let mut reg = TypeRegistry::new();
        reg.insert_complete(h, CompleteTypeObject::Struct(point));
        (reg, h)
    }

    /// A struct member `field: <ti>` built through the bridge with `registry`.
    fn resolve_member(
        ti: TypeIdentifier,
        reg: &crate::resolve::TypeRegistry,
    ) -> Result<DynamicType, DynamicError> {
        use crate::builder::TypeObjectBuilder;
        let s = TypeObjectBuilder::struct_type("::Holder")
            .member("field", ti, |m| m)
            .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Struct(s));
        let dt = DynamicTypeBuilderFactory::create_type_w_type_object_in(&to, reg)?;
        Ok(dt
            .member_by_name("field")
            .expect("field member")
            .dynamic_type()
            .clone())
    }

    fn seq_of(elem: TypeIdentifier) -> TypeIdentifier {
        TypeIdentifier::PlainSequenceSmall {
            header: crate::type_identifier::PlainCollectionHeader::default(),
            bound: 0,
            element: alloc::boxed::Box::new(elem),
        }
    }

    #[test]
    fn resolve_seq_of_struct_fully_via_registry() {
        let (reg, h) = point_registry();
        let seq = resolve_member(seq_of(TypeIdentifier::EquivalenceHashComplete(h)), &reg).unwrap();
        assert_eq!(seq.kind(), TypeKind::Sequence);
        let elem = collection::resolved_element(&seq).expect("resolved composite element");
        assert_eq!(
            elem.kind(),
            TypeKind::Structure,
            "seq element is a full struct"
        );
        assert_eq!(elem.member_count(), 2, "Point's x + y survived");
        assert!(elem.member_by_name("x").is_some());
        assert!(elem.member_by_name("y").is_some());
    }

    #[test]
    fn resolve_array_of_struct_fully_via_registry() {
        let (reg, h) = point_registry();
        let arr_ti = TypeIdentifier::PlainArraySmall {
            header: crate::type_identifier::PlainCollectionHeader::default(),
            array_bounds: alloc::vec![3],
            element: alloc::boxed::Box::new(TypeIdentifier::EquivalenceHashComplete(h)),
        };
        let arr = resolve_member(arr_ti, &reg).unwrap();
        assert_eq!(arr.kind(), TypeKind::Array);
        let elem = collection::resolved_element(&arr).expect("resolved composite element");
        assert_eq!(elem.kind(), TypeKind::Structure);
        assert_eq!(elem.member_count(), 2);
    }

    #[test]
    fn resolve_seq_of_seq_of_struct_nested() {
        let (reg, h) = point_registry();
        let nested = seq_of(seq_of(TypeIdentifier::EquivalenceHashComplete(h)));
        let outer = resolve_member(nested, &reg).unwrap();
        assert_eq!(outer.kind(), TypeKind::Sequence);
        let inner = collection::resolved_element(&outer).expect("inner sequence");
        assert_eq!(
            inner.kind(),
            TypeKind::Sequence,
            "element is itself a sequence"
        );
        let leaf = collection::resolved_element(inner).expect("leaf struct");
        assert_eq!(leaf.kind(), TypeKind::Structure);
        assert_eq!(
            leaf.member_count(),
            2,
            "innermost Point still carries its members"
        );
    }

    #[test]
    fn resolve_seq_of_scalar_still_works() {
        // Regression: a scalar element must keep round-tripping (no composite).
        let reg = crate::resolve::TypeRegistry::new();
        let seq = resolve_member(
            seq_of(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            &reg,
        )
        .unwrap();
        assert_eq!(seq.kind(), TypeKind::Sequence);
        let elem = collection::resolved_element(&seq).expect("scalar element retained");
        assert_eq!(elem.kind(), TypeKind::Int32);
    }

    #[test]
    fn resolve_map_of_struct_fully_via_registry() {
        // map<int32, Point> — key stays scalar, value resolves to a full Point.
        let (reg, h) = point_registry();
        let map_ti = TypeIdentifier::PlainMapSmall {
            header: crate::type_identifier::PlainCollectionHeader::default(),
            bound: 0,
            key_flags: crate::type_identifier::CollectionElementFlag(0),
            key: alloc::boxed::Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            element: alloc::boxed::Box::new(TypeIdentifier::EquivalenceHashComplete(h)),
        };
        let map = resolve_member(map_ti, &reg).unwrap();
        assert_eq!(map.kind(), TypeKind::Map);
        let key = collection::resolved_map_key(&map).expect("resolved key");
        let val = collection::resolved_map_value(&map).expect("resolved value");
        assert_eq!(key.kind(), TypeKind::Int32);
        assert_eq!(
            val.kind(),
            TypeKind::Structure,
            "map value is a full struct"
        );
        assert_eq!(val.member_count(), 2, "Point's x + y survived");
    }

    /// End-to-end: a `sequence<Point>` type built via the TypeObject BRIDGE
    /// (not hand-assembled) is codec-usable — encode → decode round-trips
    /// byte-exact and the nested Point members survive on the wire. This is the
    /// O3 guarantee proven against the real reflective codec.
    #[test]
    fn bridge_built_seq_of_struct_roundtrips_on_the_wire() {
        use crate::builder::TypeObjectBuilder;
        use crate::dynamic::codec::{decode_dynamic, encode_dynamic};
        use crate::dynamic::{DynamicData, DynamicValue};

        let (reg, h) = point_registry();
        let path = TypeObjectBuilder::struct_type("::Path")
            .member(
                "pts",
                seq_of(TypeIdentifier::EquivalenceHashComplete(h)),
                |m| m,
            )
            .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Struct(path));
        let ty = DynamicTypeBuilderFactory::create_type_w_type_object_in(&to, &reg).unwrap();

        let pts_member = ty.member_by_name("pts").expect("pts");
        let pts_id = pts_member.id();
        let elem = collection::resolved_element(pts_member.dynamic_type()).expect("elem");
        let xid = elem.member_by_name("x").unwrap().id();
        let yid = elem.member_by_name("y").unwrap().id();

        let mut p0 = DynamicData::new(elem.clone());
        p0.set_int32_value(xid, 3).unwrap();
        p0.set_int32_value(yid, 4).unwrap();
        let mut p1 = DynamicData::new(elem.clone());
        p1.set_int32_value(xid, 5).unwrap();
        p1.set_int32_value(yid, 6).unwrap();

        let mut d = DynamicData::new(ty.clone());
        d.set_sequence_value(pts_id, alloc::vec![p0, p1]).unwrap();

        for xcdr2 in [true, false] {
            let bytes = encode_dynamic(&d, xcdr2, false).expect("encode");
            let back = decode_dynamic(&ty, &bytes, xcdr2, false).expect("decode");
            let DynamicValue::Sequence(v) = back.get_value(pts_id).unwrap() else {
                panic!("pts is a sequence")
            };
            assert_eq!(v.len(), 2);
            assert_eq!(v[0].dynamic_type().kind(), TypeKind::Structure);
            assert_eq!(v[0].get_int32_value(xid).unwrap(), 3);
            assert_eq!(v[1].get_int32_value(yid).unwrap(), 6);
            assert_eq!(
                encode_dynamic(&back, xcdr2, false).unwrap(),
                bytes,
                "byte-exact bridge-built seq<struct> xcdr2={xcdr2}"
            );
        }
    }

    /// End-to-end: a `map<i32,Point>` built via the bridge round-trips byte-exact
    /// through the shared codec, with the composite value carrying its members.
    #[test]
    fn bridge_built_map_roundtrips_on_the_wire() {
        use crate::builder::TypeObjectBuilder;
        use crate::dynamic::codec::{decode_dynamic, encode_dynamic};
        use crate::dynamic::{DynamicData, DynamicValue};

        let (reg, h) = point_registry();
        let map_ti = TypeIdentifier::PlainMapSmall {
            header: crate::type_identifier::PlainCollectionHeader::default(),
            bound: 0,
            key_flags: crate::type_identifier::CollectionElementFlag(0),
            key: alloc::boxed::Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            element: alloc::boxed::Box::new(TypeIdentifier::EquivalenceHashComplete(h)),
        };
        let holder = TypeObjectBuilder::struct_type("::Holder")
            .member("m", map_ti, |m| m)
            .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Struct(holder));
        let ty = DynamicTypeBuilderFactory::create_type_w_type_object_in(&to, &reg).unwrap();

        let m_id = ty.member_by_name("m").unwrap().id();
        let map_ty = ty.member_by_name("m").unwrap().dynamic_type().clone();
        let kt = collection::resolved_map_key(&map_ty).unwrap().clone();
        let vt = collection::resolved_map_value(&map_ty).unwrap().clone();
        let xid = vt.member_by_name("x").unwrap().id();
        let yid = vt.member_by_name("y").unwrap().id();

        let mut kd = DynamicData::new(kt.clone());
        kd.set_value_raw(0, DynamicValue::Int32(9));
        let mut vd = DynamicData::new(vt.clone());
        vd.set_int32_value(xid, 3).unwrap();
        vd.set_int32_value(yid, 4).unwrap();

        let mut d = DynamicData::new(ty.clone());
        d.set_map_value(m_id, alloc::vec![(kd, vd)]).unwrap();

        for xcdr2 in [true, false] {
            let bytes = encode_dynamic(&d, xcdr2, false).unwrap();
            let back = decode_dynamic(&ty, &bytes, xcdr2, false).unwrap();
            let DynamicValue::Map(got) = back.get_value(m_id).unwrap() else {
                panic!("m is a map")
            };
            assert_eq!(got.len(), 1);
            assert_eq!(got[0].0.get_int32_value(0).unwrap(), 9);
            assert_eq!(got[0].1.dynamic_type().kind(), TypeKind::Structure);
            assert_eq!(got[0].1.get_int32_value(xid).unwrap(), 3);
            assert_eq!(
                encode_dynamic(&back, xcdr2, false).unwrap(),
                bytes,
                "byte-exact bridge-built map<i32,struct> xcdr2={xcdr2}"
            );
        }
    }

    #[test]
    fn alias_to_composite_resolves_transparently() {
        // typedef Point PointAlias — the alias target is a composite; typedefs
        // are transparent, so resolving the alias yields the full Point struct.
        use crate::builder::TypeObjectBuilder;
        let (reg, h) = point_registry();
        let alias =
            TypeObjectBuilder::alias("::PointAlias", TypeIdentifier::EquivalenceHashComplete(h))
                .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Alias(alias));
        let dt = DynamicTypeBuilderFactory::create_type_w_type_object_in(&to, &reg).unwrap();
        assert_eq!(
            dt.kind(),
            TypeKind::Structure,
            "alias resolved transparently to its target"
        );
        assert_eq!(dt.member_count(), 2);
        assert!(dt.member_by_name("x").is_some());

        // A scalar alias (typedef int32 Id) stays a shallow Alias node.
        let scalar_alias =
            TypeObjectBuilder::alias("::Id", TypeIdentifier::Primitive(PrimitiveKind::Int32))
                .build_complete();
        let sto = TypeObject::Complete(CompleteTypeObject::Alias(scalar_alias));
        let sdt = DynamicTypeBuilderFactory::create_type_w_type_object_in(&sto, &reg).unwrap();
        assert_eq!(
            sdt.kind(),
            TypeKind::Alias,
            "scalar typedef stays an alias node"
        );
    }
}
