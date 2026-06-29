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
//! fully-populated `DynamicType`. Residual (honest `Unsupported`): a COLLECTION
//! whose element is itself a composite keeps a shallow element descriptor (the
//! `TypeDescriptor::element_type` model holds a descriptor, not a full
//! `DynamicType`), Minimal TypeObjects, and alias-to-composite targets.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::type_identifier::{PrimitiveKind, TypeIdentifier};
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
    CommonEnumeratedLiteral,
};
use crate::type_object::{CompleteTypeObject, TypeObject};

use super::builder::{DynamicTypeBuilder, DynamicTypeBuilderFactory};
use super::descriptor::{ExtensibilityKind, MemberDescriptor, TypeDescriptor, TypeKind};
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
    /// `bound_seq` traegt alle Array-Dimensionen.
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
    /// `element_type` traegt den Value-, `key_element_type` den Key-Typ.
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

    /// Spec §7.3.4.4 — Bitset: ein `CompleteBitfield` je Member, mit
    /// Bit-Startposition (`MemberDescriptor.id`), Breite
    /// (`MemberDescriptor.bit_bound`) und Holding-Type (TypeKind-Byte des
    /// Member-Typs, der ein primitiver Integer-Typ sein muss).
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

    /// Spec §7.3.4.4 — Annotation: ein `CompleteAnnotationParameter` je
    /// Member (Parameter), mit Member-Id, Parameter-Typ, Name und dem
    /// (opaque als UTF-8-Bytes abgelegten) Default-Value aus
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
            let mut flags_bits: u16 = 0;
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
            let detail = CompleteMemberDetail {
                name: m.name().to_string(),
                ann_builtin: AppliedBuiltinMemberAnnotations::default(),
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
    /// Residual (honest `Unsupported`, not silent): a COLLECTION whose element
    /// is itself a composite keeps a shallow element descriptor — the
    /// `TypeDescriptor::element_type` model holds a descriptor, not a full
    /// `DynamicType`, so `sequence<NestedStruct>` cannot yet carry the nested
    /// members (needs a model change); likewise Minimal TypeObjects and
    /// alias-to-composite targets.
    ///
    /// # Errors
    /// `Unsupported` for the residual cases above and unresolvable hashes.
    pub fn create_type_w_type_object_in(
        type_obj: &TypeObject,
        registry: &crate::resolve::TypeRegistry,
    ) -> Result<DynamicType, DynamicError> {
        match type_obj {
            TypeObject::Complete(c) => resolve_complete(c, registry),
            TypeObject::Minimal(_) => Err(DynamicError::unsupported(
                "minimal-typeobject → dynamic-type pending C4.2 TypeRegistry",
            )),
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
        // Plain collections: build the collection DynamicType. A composite
        // element keeps a shallow element descriptor (model residual above).
        TypeIdentifier::PlainSequenceSmall { .. }
        | TypeIdentifier::PlainSequenceLarge { .. }
        | TypeIdentifier::PlainArraySmall { .. }
        | TypeIdentifier::PlainArrayLarge { .. }
        | TypeIdentifier::PlainMapSmall { .. }
        | TypeIdentifier::PlainMapLarge { .. } => {
            let kind = type_id_to_kind(ti)?;
            let desc = type_id_to_descriptor(ti, kind)?;
            F::create_type(desc)?.build()
        }
        TypeIdentifier::EquivalenceHashComplete(h) => {
            let c = registry.get_complete(h).ok_or_else(|| {
                DynamicError::unsupported("composite member: EquivalenceHash not in TypeRegistry")
            })?;
            resolve_complete(c, registry)
        }
        TypeIdentifier::EquivalenceHashMinimal(_) => Err(DynamicError::unsupported(
            "minimal composite member → dynamic-type pending C4.2",
        )),
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
            // Leaf alias targets resolve fully; a composite target keeps a
            // shallow descriptor (model residual).
            let target_kind = type_id_to_kind(&a.body.related_type)?;
            let target = type_id_to_descriptor(&a.body.related_type, target_kind)?;
            let mut desc = bare_descriptor(TypeKind::Alias, a.header.detail.type_name.clone());
            desc.element_type = Some(alloc::boxed::Box::new(target));
            F::create_type(desc)?.build()
        }
        CompleteTypeObject::Sequence(_)
        | CompleteTypeObject::Array(_)
        | CompleteTypeObject::Map(_) => {
            // Named (non-anonymous) collection TypeObject: build via descriptor;
            // composite elements stay shallow (model residual).
            resolve_named_collection(c)
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
        b.add_member_resolved(md, mt)?;
    }
    Ok(b)
}

/// Builds a top-level named collection (Sequence/Array/Map) `CompleteTypeObject`
/// into a `DynamicType` via its descriptor.
fn resolve_named_collection(c: &CompleteTypeObject) -> Result<DynamicType, DynamicError> {
    use super::builder::DynamicTypeBuilderFactory as F;
    let desc = match c {
        CompleteTypeObject::Sequence(s) => {
            let ek = type_id_to_kind(&s.element.common.type_id)?;
            let ed = type_id_to_descriptor(&s.element.common.type_id, ek)?;
            TypeDescriptor::sequence(s.detail.type_name.clone(), ed, s.bound)
        }
        CompleteTypeObject::Array(a) => {
            let ek = type_id_to_kind(&a.element.common.type_id)?;
            let ed = type_id_to_descriptor(&a.element.common.type_id, ek)?;
            TypeDescriptor::array(a.detail.type_name.clone(), ed, a.bound_seq.clone())
        }
        CompleteTypeObject::Map(m) => {
            let kk = type_id_to_kind(&m.key.common.type_id)?;
            let kd = type_id_to_descriptor(&m.key.common.type_id, kk)?;
            let vk = type_id_to_kind(&m.element.common.type_id)?;
            let vd = type_id_to_descriptor(&m.element.common.type_id, vk)?;
            TypeDescriptor::map(m.detail.type_name.clone(), kd, vd, m.bound)
        }
        other => {
            return Err(DynamicError::unsupported(format!(
                "resolve_named_collection called on non-collection: {other:?}"
            )));
        }
    };
    F::create_type(desc)?.build()
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
    fn create_type_w_minimal_typeobject_is_unsupported() {
        use crate::type_object::MinimalTypeObject;
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
}
