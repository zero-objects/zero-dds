// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! F-TYPES-3 codegen: computes a spec-conformant `TypeIdentifier`
//! (XTypes 1.3 §7.3.4.2) for an IDL struct definition and emits it as a
//! const expression in the DdsType impl.
//!
//! Strategy:
//! - Primitive member types → `TypeIdentifier::Primitive(PrimitiveKind::X)`.
//! - Strings → `TypeIdentifier::String8Small/Large`.
//! - Composite (struct/sequence/array/map) → marked as
//!   `TypeIdentifier::None`; full hash computation requires
//!   cross-crate resolution (follows in the idl-rust layer-3 review).
//!
//! The TypeIdentifier of the STRUCT itself is computed as a
//! `TypeIdentifier::EquivalenceHash` with the MD5 hash of the
//! CDR-encoded `CompleteStructType` — codegen-time deterministic.

use zerodds_idl::ast::types::{
    Declarator, IntegerType, Member, PrimitiveType, StringType, StructDef, TypeSpec,
};

/// Returns the Rust source expression for the `TypeIdentifier` const of
/// a struct definition. Format:
///
/// ```text
/// zerodds_types::TypeIdentifier::EquivalenceHash {
///     kind: zerodds_types::EquivalenceKind::Complete,
///     hash: zerodds_types::EquivalenceHash([0x.., 0x.., ...; 14]),
/// }
/// ```
///
/// If the struct contains member types whose TypeIdentifier is not
/// known at codegen time (nested composites), the function returns
/// `TypeIdentifier::None` (default path).
#[must_use]
pub fn struct_type_identifier_expr(s: &StructDef) -> String {
    match build_complete_struct_type_bytes(s) {
        Some(bytes) => {
            let hash14 = md5_truncated(&bytes);
            let mut out = String::from(
                "zerodds_types::TypeIdentifier::EquivalenceHashComplete(zerodds_types::EquivalenceHash([",
            );
            for (i, b) in hash14.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("0x{b:02x}"));
            }
            out.push_str("]))");
            out
        }
        None => "zerodds_types::TypeIdentifier::None".to_string(),
    }
}

/// MD5(`bytes`)[0..14].
fn md5_truncated(bytes: &[u8]) -> [u8; 14] {
    let full = zerodds_foundation::md5(bytes);
    let mut out = [0u8; 14];
    out.copy_from_slice(&full[0..14]);
    out
}

/// Builds the `CompleteStructType` wire bytes (CDR-LE) of the struct.
/// Returns `None` if a member type lies outside the codegen-time-known
/// range.
fn build_complete_struct_type_bytes(s: &StructDef) -> Option<Vec<u8>> {
    use zerodds_types::TypeIdentifier;
    use zerodds_types::type_object::common::{
        AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CommonStructMember,
        CompleteMemberDetail, CompleteTypeDetail, OptionalAppliedAnnotationSeq,
    };
    use zerodds_types::type_object::complete::{
        CompleteStructHeader, CompleteStructMember, CompleteStructType,
    };
    use zerodds_types::type_object::flags::{StructMemberFlag, StructTypeFlag};

    let mut member_seq: Vec<CompleteStructMember> = Vec::with_capacity(s.members.len());
    for (idx, member) in s.members.iter().enumerate() {
        let member_id = member_id_for(member, idx)?;
        let is_key = crate::annotations::member_is_key(&member.annotations);
        let mut flags_bits: u16 = 0;
        if is_key {
            flags_bits |= StructMemberFlag::IS_KEY;
        }
        let mut declarator_iter = member.declarators.iter();
        let decl = declarator_iter.next()?;
        let name = match decl {
            Declarator::Simple(n) => n.text.clone(),
            Declarator::Array(a) => a.name.text.clone(),
        };
        let member_type_id = type_spec_to_type_identifier(&member.type_spec)?;
        member_seq.push(CompleteStructMember {
            common: CommonStructMember {
                member_id,
                member_flags: StructMemberFlag(flags_bits),
                member_type_id,
            },
            detail: CompleteMemberDetail {
                name,
                ann_builtin: AppliedBuiltinMemberAnnotations {
                    unit: crate::annotations::member_unit(&member.annotations),
                    ..AppliedBuiltinMemberAnnotations::default()
                },
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            },
        });
    }

    let header = CompleteStructHeader {
        base_type: TypeIdentifier::None,
        detail: CompleteTypeDetail {
            ann_builtin: AppliedBuiltinTypeAnnotations::default(),
            ann_custom: OptionalAppliedAnnotationSeq::default(),
            type_name: s.name.text.clone(),
        },
    };
    let cs = CompleteStructType {
        struct_flags: StructTypeFlag(0),
        header,
        member_seq,
    };

    // Wrap in TypeObject::Complete for the public encode API.
    let to = zerodds_types::type_object::TypeObject::Complete(
        zerodds_types::type_object::CompleteTypeObject::Struct(cs),
    );
    to.to_bytes_le().ok()
}

/// Map IDL TypeSpec → zerodds_types::TypeIdentifier (for codegen-time-
/// known primitive/string members). Composite/sequence/array → None.
fn type_spec_to_type_identifier(ts: &TypeSpec) -> Option<zerodds_types::TypeIdentifier> {
    use zerodds_types::TypeIdentifier;
    match ts {
        TypeSpec::Primitive(p) => primitive_to_type_identifier(*p).map(TypeIdentifier::Primitive),
        TypeSpec::String(s) => string_type_to_type_identifier(s),
        // Composite/Constructed/Scoped → None (cross-type resolution needed).
        _ => None,
    }
}

fn primitive_to_type_identifier(p: PrimitiveType) -> Option<zerodds_types::PrimitiveKind> {
    use zerodds_types::PrimitiveKind;
    match p {
        PrimitiveType::Boolean => Some(PrimitiveKind::Boolean),
        PrimitiveType::Octet => Some(PrimitiveKind::Byte),
        PrimitiveType::Char => Some(PrimitiveKind::Char8),
        PrimitiveType::WideChar => Some(PrimitiveKind::Char16),
        PrimitiveType::Floating(f) => Some(match f {
            zerodds_idl::ast::types::FloatingType::Float => PrimitiveKind::Float32,
            zerodds_idl::ast::types::FloatingType::Double => PrimitiveKind::Float64,
            zerodds_idl::ast::types::FloatingType::LongDouble => PrimitiveKind::Float128,
        }),
        PrimitiveType::Integer(i) => Some(match i {
            IntegerType::Short | IntegerType::Int16 => PrimitiveKind::Int16,
            IntegerType::Long | IntegerType::Int32 => PrimitiveKind::Int32,
            IntegerType::LongLong | IntegerType::Int64 => PrimitiveKind::Int64,
            IntegerType::UShort | IntegerType::UInt16 => PrimitiveKind::UInt16,
            IntegerType::ULong | IntegerType::UInt32 => PrimitiveKind::UInt32,
            IntegerType::ULongLong | IntegerType::UInt64 => PrimitiveKind::UInt64,
            IntegerType::Int8 => PrimitiveKind::Int8,
            IntegerType::UInt8 => PrimitiveKind::UInt8,
        }),
    }
}

fn string_type_to_type_identifier(s: &StringType) -> Option<zerodds_types::TypeIdentifier> {
    use zerodds_types::TypeIdentifier;
    // bound = 0 = unbounded; otherwise a codegen-time-evaluable literal value.
    let bound_usize = match &s.bound {
        Some(expr) => crate::type_map::const_expr_as_usize(expr).unwrap_or(0),
        None => 0,
    };
    let bound = u32::try_from(bound_usize).ok()?;
    if s.wide {
        if bound <= u32::from(u8::MAX) {
            Some(TypeIdentifier::String16Small { bound: bound as u8 })
        } else {
            Some(TypeIdentifier::String16Large { bound })
        }
    } else if bound <= u32::from(u8::MAX) {
        Some(TypeIdentifier::String8Small { bound: bound as u8 })
    } else {
        Some(TypeIdentifier::String8Large { bound })
    }
}

fn member_id_for(member: &Member, fallback_index: usize) -> Option<u32> {
    if let Some(id) = crate::annotations::member_id(&member.annotations) {
        return Some(id);
    }
    u32::try_from(fallback_index).ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_idl::ast::{ConstrTypeDecl, Definition, StructDcl, TypeDecl};
    use zerodds_idl::config::ParserConfig;
    use zerodds_types::type_object::{CompleteTypeObject, TypeObject};

    fn first_struct(src: &str) -> StructDef {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
        for d in &ast.definitions {
            if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) = d
            {
                return s.clone();
            }
        }
        panic!("no struct found");
    }

    /// Builds the complete-TypeObject bytes, decodes them back and returns
    /// the unit of the first member — the round-trip proves that `@unit`
    /// actually lands in the wire format.
    fn member0_unit_via_wire(src: &str) -> Option<String> {
        let s = first_struct(src);
        let bytes = build_complete_struct_type_bytes(&s).expect("complete bytes");
        let to = TypeObject::from_bytes_le(&bytes).expect("decode");
        let TypeObject::Complete(CompleteTypeObject::Struct(cs)) = to else {
            panic!("expected Complete struct TypeObject");
        };
        cs.member_seq[0].detail.ann_builtin.unit.clone()
    }

    #[test]
    fn unit_annotation_lands_in_complete_typeobject() {
        assert_eq!(
            member0_unit_via_wire("struct S { @unit(\"meters\") double d; };"),
            Some("meters".to_string())
        );
    }

    #[test]
    fn absent_unit_leaves_wire_field_empty() {
        assert_eq!(member0_unit_via_wire("struct S { double d; };"), None);
    }
}
