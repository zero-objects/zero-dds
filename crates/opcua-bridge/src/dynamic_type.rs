// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds_types::dynamic::DynamicType` → [`DdsType`], the lightweight
//! type tree [`zerodds_opcua_gateway::dds_to_ua::walker`] walks (Gateway 1.0
//! §9.2).
//!
//! `dds_to_ua::walker`'s own module doc says the caller converts its own
//! TypeObject representation into `DdsType` — this is that conversion, for
//! the representation `zerodds-dcps`'s reflective codec uses
//! ([`zerodds_types::dynamic`], §7.6.3 TypeObject ↔ DynamicType).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_opcua_gateway::dds_to_ua::walker::{
    BitmaskDef, DdsType, EnumDef, MemberDef, StructDef, UnionCase, UnionDef,
};
use zerodds_types::dynamic::{DynamicType, DynamicTypeMember, TypeKind};

/// A `DynamicType` this module cannot map onto [`DdsType`] — named rather
/// than approximated, so a config referencing such a type fails loudly at
/// load time instead of exposing a wrong or truncated AddressSpace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    /// The model reports [`DynamicError`](zerodds_types::dynamic::DynamicError)
    /// for a handle this walk needs (a stale reference, an unresolved
    /// alias/element/base type).
    Unresolved(String),
    /// A kind Gateway 1.0 §9.2 has no mapping for: `bitset` (no §9.2 rule
    /// names it — only `bitmask` does, Tab 9.11-9.14), `fixed<P,S>`,
    /// `valuetype`/`ValueBox`/`native`/`any`/an object reference, or
    /// `TK_NONE`.
    UnmappedKind(&'static str, String),
    /// Recursion past `MAX_DEPTH` — a model this deep is a caller bug, not
    /// a spec-legal type (DDS-XTypes trees are flat in practice; see the
    /// same bound in `dds_to_ua::walker`'s own recursion notes).
    TooDeep(String),
}

impl core::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unresolved(m) => write!(f, "unresolved type handle: {m}"),
            Self::UnmappedKind(kind, name) => {
                write!(
                    f,
                    "`{name}` is {kind}, which Gateway 1.0 §9.2 has no mapping for"
                )
            }
            Self::TooDeep(name) => write!(f, "`{name}`: type nesting exceeds {MAX_DEPTH} levels"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConvertError {}

/// Recursion bound — mirrors `dds_to_ua::walker`'s own note: real DDS-XTypes
/// trees are well under 10 levels deep.
const MAX_DEPTH: usize = 64;

/// Converts a resolved DDS-XTypes type into the gateway walker's
/// [`DdsType`] tree.
///
/// # Errors
/// [`ConvertError`] — see its variants.
pub fn to_dds_type(ty: &DynamicType) -> Result<DdsType, ConvertError> {
    convert(ty, 0)
}

/// zerodds-lint: recursion-depth 64 (type nesting levels; every descent
/// goes through [`convert`] with a larger `depth`, and `convert` refuses a
/// type past `MAX_DEPTH`)
fn convert(ty: &DynamicType, depth: usize) -> Result<DdsType, ConvertError> {
    if depth > MAX_DEPTH {
        return Err(ConvertError::TooDeep(ty.qualified_name()));
    }
    match ty.kind() {
        TypeKind::Boolean => Ok(DdsType::Boolean),
        TypeKind::Int8 => Ok(DdsType::Int8),
        TypeKind::UInt8 | TypeKind::Byte => Ok(DdsType::UInt8),
        TypeKind::Int16 => Ok(DdsType::Int16),
        TypeKind::UInt16 => Ok(DdsType::UInt16),
        TypeKind::Int32 => Ok(DdsType::Int32),
        TypeKind::UInt32 => Ok(DdsType::UInt32),
        TypeKind::Int64 => Ok(DdsType::Int64),
        TypeKind::UInt64 => Ok(DdsType::UInt64),
        TypeKind::Float32 => Ok(DdsType::Float32),
        TypeKind::Float64 => Ok(DdsType::Float64),
        TypeKind::Char8 => Ok(DdsType::Char8),
        TypeKind::Char16 => Ok(DdsType::Char16),
        TypeKind::String8 => Ok(DdsType::String8),
        TypeKind::String16 => Ok(DdsType::String16),
        TypeKind::Enumeration => enum_def(ty).map(DdsType::Enum),
        TypeKind::Bitmask => bitmask_def(ty).map(DdsType::Bitmask),
        TypeKind::Alias => alias(ty, depth),
        TypeKind::Structure => struct_def(ty, depth).map(DdsType::Struct),
        TypeKind::Union => union_def(ty, depth).map(DdsType::Union),
        TypeKind::Array => array(ty, depth),
        TypeKind::Sequence => sequence(ty, depth),
        TypeKind::Map => map(ty, depth),
        TypeKind::Float128 => Err(ConvertError::UnmappedKind(
            "float128 (long double)",
            ty.qualified_name(),
        )),
        TypeKind::Bitset => Err(ConvertError::UnmappedKind("a bitset", ty.qualified_name())),
        TypeKind::Annotation => Err(ConvertError::UnmappedKind(
            "an annotation type",
            ty.qualified_name(),
        )),
        TypeKind::NoType => Err(ConvertError::UnmappedKind("TK_NONE", ty.qualified_name())),
        // `TypeKind` is `#[non_exhaustive]`; a kind added later refuses by
        // name rather than being silently misread as one of the above.
        other => Err(ConvertError::UnmappedKind(
            "an unrecognized kind",
            format!("{other:?} {}", ty.qualified_name()),
        )),
    }
}

fn unresolved(ty: &DynamicType, what: &str) -> ConvertError {
    ConvertError::Unresolved(format!("{}: {what}", ty.qualified_name()))
}

fn enum_def(ty: &DynamicType) -> Result<EnumDef, ConvertError> {
    let literals = members(ty)?
        .iter()
        .map(|m| Ok((m.name().to_string(), i64::from(member_ordinal(m)?))))
        .collect::<Result<Vec<_>, ConvertError>>()?;
    Ok(EnumDef {
        name: ty.qualified_name(),
        literals,
    })
}

fn bitmask_def(ty: &DynamicType) -> Result<BitmaskDef, ConvertError> {
    let bound = ty.bit_bound().unwrap_or(32);
    let bits = members(ty)?
        .iter()
        .map(|m| Ok((m.name().to_string(), member_ordinal(m)?)))
        .collect::<Result<Vec<_>, ConvertError>>()?;
    Ok(BitmaskDef {
        name: ty.qualified_name(),
        bound: u32::from(bound),
        bits,
    })
}

/// A member's effective id — for an enumerator or a bitflag this is its
/// ordinal/bit position (§7.5.2.1's `id`, which the model assigns
/// sequentially or via `@value`/`@position` the same way it assigns a
/// struct member's `@id`).
fn member_ordinal(m: &DynamicTypeMember) -> Result<u32, ConvertError> {
    // §7.5.2.7.5 gives an enumerator or a flag MEMBER_ID_INVALID as its id;
    // the value and the position are `literal_value`.
    m.literal_value().ok_or_else(|| {
        ConvertError::Unresolved(format!(
            "{}: a literal with no value in the model",
            m.name()
        ))
    })
}

/// zerodds-lint: recursion-depth 64 (type nesting levels; every descent
/// goes through [`convert`] with a larger `depth`, and `convert` refuses a
/// type past `MAX_DEPTH`)
fn alias(ty: &DynamicType, depth: usize) -> Result<DdsType, ConvertError> {
    // §7.5.2.4.1: the target of an alias is its `base_type`.
    let target = ty
        .base_type()
        .ok_or_else(|| unresolved(ty, "alias with no target"))?;
    Ok(DdsType::Alias {
        name: ty.qualified_name(),
        target: Box::new(convert(&target, depth + 1)?),
    })
}

/// zerodds-lint: recursion-depth 64 (type nesting levels; every descent
/// goes through [`convert`] with a larger `depth`, and `convert` refuses a
/// type past `MAX_DEPTH`)
fn struct_def(ty: &DynamicType, depth: usize) -> Result<StructDef, ConvertError> {
    let members = members(ty)?
        .iter()
        .map(|m| member_def(m, depth + 1))
        .collect::<Result<Vec<_>, ConvertError>>()?;
    Ok(StructDef {
        name: ty.qualified_name(),
        members,
    })
}

/// zerodds-lint: recursion-depth 64 (type nesting levels; every descent
/// goes through [`convert`] with a larger `depth`, and `convert` refuses a
/// type past `MAX_DEPTH`)
fn member_def(m: &DynamicTypeMember, depth: usize) -> Result<MemberDef, ConvertError> {
    let member_type = m
        .dynamic_type()
        .map_err(|e| ConvertError::Unresolved(format!("{}: {e:?}", m.name())))?;
    let descriptor = m
        .descriptor()
        .map_err(|e| ConvertError::Unresolved(format!("{}: {e:?}", m.name())))?;
    Ok(MemberDef {
        name: m.name().to_string(),
        member_type: convert(&member_type, depth)?,
        is_optional: descriptor.is_optional,
        is_key: descriptor.is_key,
    })
}

/// zerodds-lint: recursion-depth 64 (type nesting levels; every descent
/// goes through [`convert`] with a larger `depth`, and `convert` refuses a
/// type past `MAX_DEPTH`)
fn union_def(ty: &DynamicType, depth: usize) -> Result<UnionDef, ConvertError> {
    let discriminator = ty
        .discriminator_type()
        .ok_or_else(|| unresolved(ty, "union with no discriminator"))?;
    let cases = members(ty)?
        .iter()
        .map(|m| {
            let member_type = m
                .dynamic_type()
                .map_err(|e| ConvertError::Unresolved(format!("{}: {e:?}", m.name())))?;
            let descriptor = m
                .descriptor()
                .map_err(|e| ConvertError::Unresolved(format!("{}: {e:?}", m.name())))?;
            Ok(UnionCase {
                name: m.name().to_string(),
                member_type: convert(&member_type, depth + 1)?,
                is_default: descriptor.is_default_label,
            })
        })
        .collect::<Result<Vec<_>, ConvertError>>()?;
    Ok(UnionDef {
        name: ty.qualified_name(),
        discriminator: Box::new(convert(&discriminator, depth + 1)?),
        cases,
    })
}

/// zerodds-lint: recursion-depth 64 (type nesting levels; every descent
/// goes through [`convert`] with a larger `depth`, and `convert` refuses a
/// type past `MAX_DEPTH`)
fn array(ty: &DynamicType, depth: usize) -> Result<DdsType, ConvertError> {
    let element = ty
        .element_type()
        .ok_or_else(|| unresolved(ty, "array with no element type"))?;
    let descriptor = ty
        .descriptor()
        .map_err(|e| ConvertError::Unresolved(format!("{}: {e:?}", ty.qualified_name())))?;
    Ok(DdsType::Array {
        element: Box::new(convert(&element, depth + 1)?),
        dimensions: descriptor.bound,
    })
}

/// zerodds-lint: recursion-depth 64 (type nesting levels; every descent
/// goes through [`convert`] with a larger `depth`, and `convert` refuses a
/// type past `MAX_DEPTH`)
fn sequence(ty: &DynamicType, depth: usize) -> Result<DdsType, ConvertError> {
    let element = ty
        .element_type()
        .ok_or_else(|| unresolved(ty, "sequence with no element type"))?;
    let descriptor = ty
        .descriptor()
        .map_err(|e| ConvertError::Unresolved(format!("{}: {e:?}", ty.qualified_name())))?;
    Ok(DdsType::Sequence {
        element: Box::new(convert(&element, depth + 1)?),
        bound: descriptor.bound.first().copied(),
    })
}

/// zerodds-lint: recursion-depth 64 (type nesting levels; every descent
/// goes through [`convert`] with a larger `depth`, and `convert` refuses a
/// type past `MAX_DEPTH`)
fn map(ty: &DynamicType, depth: usize) -> Result<DdsType, ConvertError> {
    let key = ty
        .key_element_type()
        .ok_or_else(|| unresolved(ty, "map with no key type"))?;
    let value = ty
        .element_type()
        .ok_or_else(|| unresolved(ty, "map with no value type"))?;
    let descriptor = ty
        .descriptor()
        .map_err(|e| ConvertError::Unresolved(format!("{}: {e:?}", ty.qualified_name())))?;
    Ok(DdsType::Map {
        key: Box::new(convert(&key, depth + 1)?),
        value: Box::new(convert(&value, depth + 1)?),
        bound: descriptor.bound.first().copied(),
    })
}

fn members(ty: &DynamicType) -> Result<Vec<DynamicTypeMember>, ConvertError> {
    let count = ty.member_count();
    let mut out = Vec::with_capacity(count as usize);
    for index in 0..count {
        let member = ty
            .member_by_index(index)
            .ok_or_else(|| unresolved(ty, "member index out of range"))?;
        out.push(member);
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use zerodds_ast::Ast;

    fn ast_of(idl: &str) -> Arc<Ast> {
        Arc::new(
            zerodds_wire_probe::encode::model_from_idl(idl, "dynamic_type_test.idl")
                .expect("idl parses"),
        )
    }

    #[test]
    fn primitive_members_map_1_to_1() {
        let ast = ast_of("struct P { boolean a; long b; double c; string d; };");
        let dt = DynamicType::find(&ast, "P").expect("type");
        let converted = to_dds_type(&dt).expect("convert");
        let DdsType::Struct(s) = converted else {
            panic!("expected struct");
        };
        assert_eq!(s.members[0].member_type, DdsType::Boolean);
        assert_eq!(s.members[1].member_type, DdsType::Int32);
        assert_eq!(s.members[2].member_type, DdsType::Float64);
        assert_eq!(s.members[3].member_type, DdsType::String8);
    }

    #[test]
    fn key_and_optional_flags_survive() {
        let ast = ast_of("struct K { @key long id; @optional string note; };");
        let dt = DynamicType::find(&ast, "K").expect("type");
        let DdsType::Struct(s) = to_dds_type(&dt).expect("convert") else {
            panic!("expected struct");
        };
        assert!(s.members[0].is_key);
        assert!(s.members[1].is_optional);
    }

    #[test]
    fn nested_struct_recurses() {
        let ast = ast_of("struct Inner { long v; }; struct Outer { Inner i; };");
        let dt = DynamicType::find(&ast, "Outer").expect("type");
        let DdsType::Struct(s) = to_dds_type(&dt).expect("convert") else {
            panic!("expected struct");
        };
        let DdsType::Struct(inner) = &s.members[0].member_type else {
            panic!("expected nested struct");
        };
        assert_eq!(inner.name, "Inner");
    }

    #[test]
    fn enum_literals_carry_their_ordinal() {
        let ast = ast_of("enum Color { RED, GREEN, BLUE };");
        let dt = DynamicType::find(&ast, "Color").expect("type");
        let DdsType::Enum(e) = to_dds_type(&dt).expect("convert") else {
            panic!("expected enum");
        };
        assert_eq!(
            e.literals,
            alloc::vec![
                ("RED".to_string(), 0),
                ("GREEN".to_string(), 1),
                ("BLUE".to_string(), 2),
            ]
        );
    }

    #[test]
    fn sequence_of_struct_carries_bound() {
        let ast = ast_of("struct E { long v; }; struct S { sequence<E, 4> items; };");
        let dt = DynamicType::find(&ast, "S").expect("type");
        let DdsType::Struct(s) = to_dds_type(&dt).expect("convert") else {
            panic!("expected struct");
        };
        let DdsType::Sequence { element, bound } = &s.members[0].member_type else {
            panic!("expected sequence");
        };
        assert_eq!(*bound, Some(4));
        assert!(matches!(**element, DdsType::Struct(_)));
    }

    #[test]
    fn array_member_carries_dimensions() {
        let ast = ast_of("struct S { long grid[3][4]; };");
        let dt = DynamicType::find(&ast, "S").expect("type");
        let DdsType::Struct(s) = to_dds_type(&dt).expect("convert") else {
            panic!("expected struct");
        };
        let DdsType::Array { dimensions, .. } = &s.members[0].member_type else {
            panic!("expected array");
        };
        assert_eq!(*dimensions, alloc::vec![3, 4]);
    }

    #[test]
    fn alias_is_named_and_transparent() {
        let ast = ast_of("typedef long MyInt; struct S { MyInt v; };");
        let dt = DynamicType::find(&ast, "S").expect("type");
        let DdsType::Struct(s) = to_dds_type(&dt).expect("convert") else {
            panic!("expected struct");
        };
        let DdsType::Alias { name, target } = &s.members[0].member_type else {
            panic!("expected alias");
        };
        assert_eq!(name, "MyInt");
        assert_eq!(**target, DdsType::Int32);
    }

    #[test]
    fn union_carries_discriminator_and_cases() {
        let ast = ast_of("union U switch (long) { case 0: long a; case 1: default: short b; };");
        let dt = DynamicType::find(&ast, "U").expect("type");
        let DdsType::Union(u) = to_dds_type(&dt).expect("convert") else {
            panic!("expected union");
        };
        assert_eq!(*u.discriminator, DdsType::Int32);
        assert_eq!(u.cases.len(), 2);
        assert!(u.cases[1].is_default);
    }

    #[test]
    fn fixed_point_type_is_a_named_unmapped_error() {
        // `fixed<P,S>` is CORBA-only in this codebase's own IDL surface;
        // this test documents the refusal shape without depending on a
        // parseable `fixed` declaration (float128 is a stand-in that also
        // has no Gateway 1.0 §9.2 mapping).
        let ast = ast_of("struct S { long double v; };");
        let dt = DynamicType::find(&ast, "S").expect("type");
        let err = to_dds_type(&dt).expect_err("float128 has no mapping");
        assert!(matches!(err, ConvertError::UnmappedKind(_, _)));
    }
}
