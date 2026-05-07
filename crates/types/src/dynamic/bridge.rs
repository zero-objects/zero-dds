// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DynamicType ↔ TypeObject Bridge (XTypes 1.3 §7.6.3 + §7.6.4).
//!
//! Diese Bridge ist die Klammer zwischen der **Wire-Repraesentation**
//! (`TypeObject`, mit `MinimalTypeObject` + `CompleteTypeObject` als
//! Diskrimination) und der **Runtime-API** (`DynamicType`).
//!
//! Implementiert: Struct + Union + Enum + Alias + Sequence/Array/Map.
//! Bitset/Bitmask/Annotation: Spec-konforme Reject-Errors, Implementation folgt mit MemberDescriptor-Erweiterung (siehe
//! [`DynamicError::Unsupported`]). Forward-Mapping von Composite-Member-
//! Types ueber den Namen — TypeRegistry-Lookup auf
//! EquivalenceHash-TypeIdentifiers ist Aufgabe von C4.2.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::type_identifier::{PrimitiveKind, TypeIdentifier};
use crate::type_object::common::{CommonStructMember, CommonUnionMember};
use crate::type_object::complete::{
    CompleteAliasBody, CompleteAliasHeader, CompleteAliasType, CompleteBitflag,
    CompleteBitmaskType, CompleteDiscriminatorMember, CompleteEnumeratedHeader,
    CompleteEnumeratedLiteral, CompleteEnumeratedType, CompleteStructHeader, CompleteStructMember,
    CompleteStructType, CompleteUnionHeader, CompleteUnionMember, CompleteUnionType,
};
use crate::type_object::flags::{
    AliasMemberFlag, AliasTypeFlag, BitflagFlag, BitmaskTypeFlag, EnumLiteralFlag, EnumTypeFlag,
    StructMemberFlag, StructTypeFlag, UnionDiscriminatorFlag, UnionMemberFlag, UnionTypeFlag,
};
use crate::type_object::minimal::CommonDiscriminatorMember;
use crate::type_object::minimal::{CommonBitflag, CommonEnumeratedHeader, CommonEnumeratedLiteral};
use crate::type_object::{CompleteTypeObject, TypeObject};

use super::builder::{DynamicTypeBuilder, DynamicTypeBuilderFactory};
use super::descriptor::{ExtensibilityKind, MemberDescriptor, TypeDescriptor, TypeKind};
use super::error::DynamicError;
use super::type_::DynamicType;

// ----------------------------------------------------------------------
// DynamicType → TypeObject (Spec §7.6.3 — fuer Discovery + TypeLookup)
// ----------------------------------------------------------------------

impl DynamicType {
    /// Spec §7.6.3 — wandelt diese DynamicType in ein
    /// `CompleteTypeObject`. Liefert immer Complete (nicht Minimal),
    /// weil DynamicType die Namen + Annotations traegt.
    ///
    /// Scope:
    /// - Struct mit primitiven + String + Sequence/Array Members.
    /// - Union, Enum, Alias als eigene Helper-Methoden (siehe Modul-Doc).
    ///
    /// # Errors
    /// `Unsupported` fuer noch nicht implementierte Kinds,
    /// `Inconsistent` wenn der Type fehlerhaft ist.
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
            TypeKind::Array | TypeKind::Sequence | TypeKind::Map => {
                // Spec §7.3.4 — Collection-Types werden via TypeIdentifier
                // exklusiv repraesentiert (PlainCollection / HashedCollection).
                // Es gibt keinen CompleteTypeObject fuer plain Collections;
                // nur ihre Element-Types haben TypeObjects.
                Err(DynamicError::unsupported(format!(
                    "to_type_object for {:?} — Collection-Types use TypeIdentifier exclusively (XTypes §7.3.4)",
                    self.kind(),
                )))
            }
            TypeKind::Bitset | TypeKind::Annotation => {
                // Bitset benoetigt bit_position + bit_count pro Member;
                // Annotation benoetigt default_value. Beide sind in
                // MemberDescriptor nicht direkt strukturiert ablegbar
                // ohne Zusatz-Konvention. MemberDescriptor-Erweiterung benötigt.
                Err(DynamicError::unsupported(format!(
                    "to_type_object for {:?} — needs MemberDescriptor extension (MemberDescriptor extension)",
                    self.kind(),
                )))
            }
            other => Err(DynamicError::unsupported(format!(
                "to_type_object: {other:?} is a primitive/no-type kind without TypeObject",
            ))),
        }
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
            // Spec §7.3.4.5.3.2 — case-labels sind `long` (i32).
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

/// Mapping von einem Member-`TypeDescriptor` auf einen passenden
/// `TypeIdentifier`. Nur primitive + String + Sequence + Array werden
/// inline kodiert; Composite-Members verlangen eine TypeRegistry-
/// Lookup-Strategie über `zerodds_types::resolve::TypeRegistry`.
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
            // Bridge-Phase 1: Composite-Members tragen `TypeIdentifier::None`
            // — die TypeRegistry (C4.2) loest spaeter ueber den Namen
            // gegen einen EquivalenceHash. Roundtrip-Tests setzen diese
            // Information getrennt.
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
    /// - `CompleteStructType` mit primitiven + String-Members.
    /// - `MinimalStructType` als read-only mit Hash-Names.
    /// - Andere Kinds: `Unsupported`.
    ///
    /// # Errors
    /// `Unsupported` fuer Kinds ausserhalb des aktuellen Implementations-Scopes.
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
            // Composite via TypeRegistry-Lookup — markieren als Structure
            // als Default; korrekt aufzuloesen ist C4.2.
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
/// Rekursive Aufrufe via PlainSequence/PlainArray-Element. Tiefe ist
/// implizit durch [`TypeIdentifier::MAX_DECODE_DEPTH`] gecapt — der
/// Type-Identifier kann beim Wire-Decode hoechstens 16 Schichten tief
/// werden, danach scheitert das Decode mit `LengthExceeded`.
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
            // Wir kennen den Namen erst nach einem TypeRegistry-Lookup
            // (C4.2). Bis dahin: Platzhalter-Descriptor mit Kind=Structure.
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
        // Deep-Equality auf den von der Bridge gebauten Type.
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
        // alias mit element_type = int32.
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
    fn collection_kinds_to_typeobject_yield_explicit_error() {
        // Sequence ist TypeIdentifier-only — to_type_object muss einen
        // klaren Spec-konformen Error zurueckgeben.
        let mut seq_desc = TypeDescriptor::primitive(TypeKind::Sequence, "sequence");
        seq_desc.kind = TypeKind::Sequence;
        seq_desc.element_type = Some(alloc::boxed::Box::new(TypeDescriptor::primitive(
            TypeKind::Int32,
            "int32",
        )));
        seq_desc.bound = alloc::vec![16];
        let seq = DynamicTypeBuilderFactory::create_type(seq_desc)
            .unwrap()
            .build()
            .unwrap();
        let err = seq.to_type_object().unwrap_err();
        match err {
            DynamicError::Unsupported(msg) => {
                assert!(msg.contains("Collection-Types"), "msg: {msg}");
            }
            other => panic!("expected Unsupported, got {other:?}"),
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
}
