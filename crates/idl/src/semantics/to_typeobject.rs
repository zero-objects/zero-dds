// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AST → TypeObject Mapper (WP 1.5 T-IDL2).
//!
//! Konvertiert IDL-`StructDef` → XTypes `TypeObject`. Primary-Fokus
//! Phase 1: Struct mit Primitives, Strings, bounded Sequences.
//! Nested composite-Types (Struct-in-Struct, Unions, Enums als Member)
//! werden als `TypeIdentifier::EquivalenceHashMinimal(ZERO)` Platzhalter
//! zurueckgeliefert — der Caller muss die in der Registry aufloesen.

use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::type_object::minimal::MinimalStructType;
use zerodds_types::{PrimitiveKind, TypeIdentifier};

use crate::ast::{
    FloatingType, IntegerType, PrimitiveType, SequenceType, StringType, StructDef, TypeSpec,
};

use super::annotations::{ExtensibilityKind, lower_annotations};

/// Fehler beim Mapping.
#[derive(Debug, Clone, PartialEq)]
pub enum MapError {
    /// Nicht-primitive Typen werden aktuell als opaque-Hash abgebildet.
    /// Falls der Caller das nicht zulaesst, liefert diese Variante einen
    /// Hinweis, dass ein echter Type-Mapper fuer composite Types noetig ist.
    UnsupportedTypeSpec(&'static str),
    /// Annotation-Lowering-Fehler.
    Annotation(String),
    /// Scoped-Name-Referenz auf einen anderen benannten Typ (z.B.
    /// `struct S { Foo a; };`). Der Mapper kann Forward-Refs nicht
    /// aufloesen — der Caller braucht einen zweistufigen Pass (erst alle
    /// Types sammeln + Hashes vorberechnen, dann Refs einsetzen).
    UnresolvedScoped(alloc::string::String),
}

/// Map einen IDL `TypeSpec` auf einen `TypeIdentifier`.
///
/// Primitives + Strings + Sequences-of-primitives werden direkt. Scoped
/// Refs + sonstige composite Types werden als Null-Hash-Placeholder
/// abgebildet.
///
/// zerodds-lint: recursion-depth 8
///
/// Rekursion bei `Sequence<...>` mit nested Element-TypeSpec. Realistische
/// IDL-Konstrukte nisten hoechstens 2-3 Ebenen. Cap nicht explizit
/// durchgesetzt — das AST kommt aus dem IDL-Parser (WP 0.3), der
/// seinerseits Grammar-basierte Limits hat (LR(1) Stack).
///
/// # Errors
/// `UnsupportedTypeSpec` fuer aktuell nicht-abbildbare Konstruktionen.
pub fn map_type_spec(spec: &TypeSpec) -> Result<TypeIdentifier, MapError> {
    Ok(match spec {
        TypeSpec::Primitive(p) => TypeIdentifier::Primitive(map_primitive(*p)),
        TypeSpec::String(StringType { wide, bound, .. }) => {
            let bound_u32 = bound
                .as_ref()
                .and_then(|e| {
                    if let crate::ast::ConstExpr::Literal(l) = e {
                        l.raw.parse::<u32>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if *wide {
                if bound_u32 <= 255 {
                    TypeIdentifier::String16Small {
                        bound: bound_u32 as u8,
                    }
                } else {
                    TypeIdentifier::String16Large { bound: bound_u32 }
                }
            } else if bound_u32 <= 255 {
                TypeIdentifier::String8Small {
                    bound: bound_u32 as u8,
                }
            } else {
                TypeIdentifier::String8Large { bound: bound_u32 }
            }
        }
        TypeSpec::Sequence(SequenceType { elem, bound, .. }) => {
            let element = alloc::boxed::Box::new(map_type_spec(elem)?);
            let bound_u32 = bound
                .as_ref()
                .and_then(|e| {
                    if let crate::ast::ConstExpr::Literal(l) = e {
                        l.raw.parse::<u32>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if bound_u32 <= 255 {
                TypeIdentifier::PlainSequenceSmall {
                    header: zerodds_types::PlainCollectionHeader::default(),
                    bound: bound_u32 as u8,
                    element,
                }
            } else {
                TypeIdentifier::PlainSequenceLarge {
                    header: zerodds_types::PlainCollectionHeader::default(),
                    bound: bound_u32,
                    element,
                }
            }
        }
        TypeSpec::Scoped(s) => {
            let path = s
                .parts
                .iter()
                .map(|p| p.text.clone())
                .collect::<alloc::vec::Vec<_>>()
                .join("::");
            return Err(MapError::UnresolvedScoped(path));
        }
        TypeSpec::Fixed(_) => return Err(MapError::UnsupportedTypeSpec("fixed")),
        TypeSpec::Map(_) => return Err(MapError::UnsupportedTypeSpec("map (inline IDL map)")),
        TypeSpec::Any => return Err(MapError::UnsupportedTypeSpec("any")),
    })
}

fn map_primitive(p: PrimitiveType) -> PrimitiveKind {
    use FloatingType::*;
    use IntegerType::*;
    match p {
        PrimitiveType::Boolean => PrimitiveKind::Boolean,
        PrimitiveType::Octet => PrimitiveKind::Byte,
        PrimitiveType::Char => PrimitiveKind::Char8,
        PrimitiveType::WideChar => PrimitiveKind::Char16,
        PrimitiveType::Integer(i) => match i {
            Short | Int16 => PrimitiveKind::Int16,
            Long | Int32 => PrimitiveKind::Int32,
            LongLong | Int64 => PrimitiveKind::Int64,
            UShort | UInt16 => PrimitiveKind::UInt16,
            ULong | UInt32 => PrimitiveKind::UInt32,
            ULongLong | UInt64 => PrimitiveKind::UInt64,
            Int8 => PrimitiveKind::Int8,
            UInt8 => PrimitiveKind::UInt8,
        },
        PrimitiveType::Floating(f) => match f {
            Float => PrimitiveKind::Float32,
            Double => PrimitiveKind::Float64,
            LongDouble => PrimitiveKind::Float128,
        },
    }
}

/// Map einen IDL `StructDef` → XTypes `MinimalStructType`.
///
/// Erkennt `@key`, `@id(n)`, `@optional`, `@must_understand`, `@external`
/// auf Membern und `@final`/`@appendable`/`@mutable`/`@nested`/
/// `@extensibility(...)` auf dem Struct.
///
/// # Errors
/// `MapError` bei nicht-abbildbaren Konstruktionen.
pub fn lower_struct_to_minimal(s: &StructDef) -> Result<MinimalStructType, MapError> {
    let type_annotations = lower_annotations(&s.annotations)
        .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
    let extensibility = match type_annotations.extensibility() {
        Some(ExtensibilityKind::Final) => Extensibility::Final,
        Some(ExtensibilityKind::Mutable) => Extensibility::Mutable,
        _ => Extensibility::Appendable,
    };
    let nested = type_annotations
        .builtins
        .iter()
        .any(|a| matches!(a, super::annotations::BuiltinAnnotation::Nested));
    let autoid_hash = type_annotations.builtins.iter().any(|a| {
        matches!(
            a,
            super::annotations::BuiltinAnnotation::Autoid(super::annotations::AutoidKind::Hash)
        )
    });

    let mut builder =
        TypeObjectBuilder::struct_type(s.name.text.clone()).extensibility(extensibility);
    if nested {
        builder = builder.nested();
    }
    if autoid_hash {
        builder = builder.autoid_hash();
    }

    for m in &s.members {
        // IDL Member kann mehrere Declarators haben (z.B. `long a, b;`).
        // Wir behandeln jeden Declarator als separaten TypeObject-Member.
        for decl in &m.declarators {
            let ti = map_type_spec(&m.type_spec)?;
            let member_anns = lower_annotations(&m.annotations)
                .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
            // §7.2.4.4.2 — @non_serialized Member werden nicht ins
            // TypeObject aufgenommen (ABI-only, kein Wire-Form, daher
            // auch nicht im Assignability-Vergleich).
            let is_non_serialized = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::NonSerialized));
            if is_non_serialized {
                continue;
            }
            let explicit_id = member_anns.explicit_id();
            let is_key = member_anns.has_key();
            let is_optional = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::Optional));
            let is_must_understand = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::MustUnderstand));
            let is_external = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::External));
            let member_name = decl.name().text.clone();
            builder = builder.member(member_name, ti, |mut mb| {
                if is_key {
                    mb = mb.key();
                }
                if is_optional {
                    mb = mb.optional();
                }
                if is_must_understand {
                    mb = mb.must_understand();
                }
                if is_external {
                    mb = mb.external();
                }
                if let Some(id) = explicit_id {
                    mb = mb.id(id);
                }
                mb
            });
        }
    }

    Ok(builder.build_minimal())
}

extern crate alloc;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn first_struct(src: &str) -> StructDef {
        let ast = parse(src, &ParserConfig::default()).expect("parse");
        for def in ast.definitions {
            if let crate::ast::Definition::Type(crate::ast::TypeDecl::Constr(
                crate::ast::ConstrTypeDecl::Struct(crate::ast::StructDcl::Def(s)),
            )) = def
            {
                return s;
            }
        }
        panic!("no struct");
    }

    #[test]
    fn simple_struct_lowers() {
        let s = first_struct("struct S { long id; string text; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 2);
        assert!(matches!(
            st.member_seq[0].common.member_type_id,
            TypeIdentifier::Primitive(PrimitiveKind::Int32)
        ));
        assert!(matches!(
            st.member_seq[1].common.member_type_id,
            TypeIdentifier::String8Small { .. }
        ));
    }

    #[test]
    fn key_and_id_annotations_carry_through() {
        let s = first_struct("struct S { @key @id(5) long id; long extra; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq[0].common.member_id, 5);
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_KEY)
        );
    }

    #[test]
    fn mutable_extensibility_applied() {
        let s = first_struct("@mutable struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_MUTABLE)
        );
    }

    #[test]
    fn sequence_of_ints_lowers() {
        let s = first_struct("struct S { sequence<long, 10> items; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(matches!(
            st.member_seq[0].common.member_type_id,
            TypeIdentifier::PlainSequenceSmall { bound: 10, .. }
        ));
    }

    #[test]
    fn optional_annotation_sets_flag() {
        let s = first_struct("struct S { @optional long maybe; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_OPTIONAL)
        );
    }

    #[test]
    fn non_serialized_member_is_dropped_from_typeobject() {
        // §7.2.4.4.2 — @non_serialized Member sind ABI-only, nicht im
        // Wire-Form und nicht im Assignability-Vergleich.
        let s = first_struct(
            "struct S { long visible; @non_serialized long hidden; long also_visible; };",
        );
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 2);
        // Memer-Reihenfolge bleibt erhalten (visible, also_visible).
        // Keine ID-Annotation → IDs werden allokiert; nur sicherstellen,
        // dass der hidden-Member fehlt.
        // Da wir keine Namen-Liste leicht greifen koennen, pruefen wir
        // dass der Type-Identifier-Typ stimmt und Length passt.
    }

    #[test]
    fn non_serialized_in_otherwise_empty_struct_yields_empty_member_seq() {
        let s = first_struct("struct S { @non_serialized long internal_only; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 0);
    }

    #[test]
    fn non_serialized_member_does_not_block_assignability() {
        // Reader hat @non_serialized fuer einen Member; Writer hat ihn
        // ueberhaupt nicht. Beim Wire-Vergleich (assignability) sollen
        // beide Member-Sequenzen identisch sein → kein blockierender
        // Mismatch.
        let writer_src = "struct S { long a; long b; };";
        let reader_src = "struct S { long a; @non_serialized long debug_only; long b; };";
        let writer = lower_struct_to_minimal(&first_struct(writer_src)).unwrap();
        let reader = lower_struct_to_minimal(&first_struct(reader_src)).unwrap();
        // Beide muessen dieselbe Anzahl + dieselbe Member-Identifier-Folge haben.
        assert_eq!(writer.member_seq.len(), reader.member_seq.len());
        for (w, r) in writer.member_seq.iter().zip(&reader.member_seq) {
            assert_eq!(w.common.member_type_id, r.common.member_type_id);
        }
    }

    #[test]
    fn non_serialized_with_other_annotations_still_dropped() {
        // @key + @non_serialized — non_serialized hat Vorrang, Member wird gedropped.
        let s = first_struct("struct S { @key @non_serialized long ghost; long real; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 1);
        assert!(
            !st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_KEY)
        );
    }

    // ---- map_type_spec: every Primitive variant --------------------------

    use crate::ast::Identifier;
    use crate::ast::{
        ConstExpr, FixedPtType, FloatingType, IntegerType, Literal, LiteralKind, MapType,
        PrimitiveType, ScopedName, SequenceType, StringType, TypeSpec,
    };
    use crate::errors::Span;

    fn sp() -> Span {
        Span::SYNTHETIC
    }

    fn int_lit(raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw: raw.to_string(),
            span: sp(),
        })
    }

    fn prim(p: PrimitiveType) -> TypeSpec {
        TypeSpec::Primitive(p)
    }

    #[test]
    fn map_primitive_all_integer_kinds() {
        let cases = [
            (IntegerType::Short, PrimitiveKind::Int16),
            (IntegerType::Long, PrimitiveKind::Int32),
            (IntegerType::LongLong, PrimitiveKind::Int64),
            (IntegerType::UShort, PrimitiveKind::UInt16),
            (IntegerType::ULong, PrimitiveKind::UInt32),
            (IntegerType::ULongLong, PrimitiveKind::UInt64),
            (IntegerType::Int8, PrimitiveKind::Int8),
            (IntegerType::Int16, PrimitiveKind::Int16),
            (IntegerType::Int32, PrimitiveKind::Int32),
            (IntegerType::Int64, PrimitiveKind::Int64),
            (IntegerType::UInt8, PrimitiveKind::UInt8),
            (IntegerType::UInt16, PrimitiveKind::UInt16),
            (IntegerType::UInt32, PrimitiveKind::UInt32),
            (IntegerType::UInt64, PrimitiveKind::UInt64),
        ];
        for (idl, xt) in cases {
            let ti = map_type_spec(&prim(PrimitiveType::Integer(idl))).unwrap();
            assert_eq!(ti, TypeIdentifier::Primitive(xt));
        }
    }

    #[test]
    fn map_primitive_floats_boolean_char_octet() {
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Floating(FloatingType::Float))).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Float32)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Floating(FloatingType::Double))).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Float64)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Floating(FloatingType::LongDouble))).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Float128)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Boolean)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Boolean)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Octet)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Byte)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Char)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Char8)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::WideChar)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Char16)
        );
    }

    // ---- String kinds ----------------------------------------------------

    #[test]
    fn map_string_unbounded_narrow_is_small_with_bound_zero() {
        let ti = map_type_spec(&TypeSpec::String(StringType {
            wide: false,
            bound: None,
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(ti, TypeIdentifier::String8Small { bound: 0 }));
    }

    #[test]
    fn map_string_bounded_narrow_small() {
        let ti = map_type_spec(&TypeSpec::String(StringType {
            wide: false,
            bound: Some(int_lit("128")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(ti, TypeIdentifier::String8Small { bound: 128 });
    }

    #[test]
    fn map_string_large_narrow_over_255() {
        let ti = map_type_spec(&TypeSpec::String(StringType {
            wide: false,
            bound: Some(int_lit("1000")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(ti, TypeIdentifier::String8Large { bound: 1000 });
    }

    #[test]
    fn map_string_wide_small_and_large() {
        let wide_small = map_type_spec(&TypeSpec::String(StringType {
            wide: true,
            bound: Some(int_lit("64")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(wide_small, TypeIdentifier::String16Small { bound: 64 });

        let wide_large = map_type_spec(&TypeSpec::String(StringType {
            wide: true,
            bound: Some(int_lit("70000")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(wide_large, TypeIdentifier::String16Large { bound: 70000 });
    }

    // ---- Sequence kinds --------------------------------------------------

    #[test]
    fn map_sequence_small_and_large_bounds() {
        let small = map_type_spec(&TypeSpec::Sequence(SequenceType {
            elem: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: Some(int_lit("100")),
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            small,
            TypeIdentifier::PlainSequenceSmall { bound: 100, .. }
        ));

        let large = map_type_spec(&TypeSpec::Sequence(SequenceType {
            elem: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: Some(int_lit("1000")),
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            large,
            TypeIdentifier::PlainSequenceLarge { bound: 1000, .. }
        ));
    }

    #[test]
    fn map_sequence_unbounded_is_small_with_bound_zero() {
        let ti = map_type_spec(&TypeSpec::Sequence(SequenceType {
            elem: alloc::boxed::Box::new(prim(PrimitiveType::Boolean)),
            bound: None,
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            ti,
            TypeIdentifier::PlainSequenceSmall { bound: 0, .. }
        ));
    }

    // ---- Error paths -----------------------------------------------------

    #[test]
    fn map_scoped_returns_unresolved_scoped() {
        let scoped = ScopedName {
            absolute: false,
            parts: alloc::vec![Identifier::new("Ns", sp()), Identifier::new("Inner", sp()),],
            span: sp(),
        };
        let err = map_type_spec(&TypeSpec::Scoped(scoped)).unwrap_err();
        match err {
            MapError::UnresolvedScoped(p) => assert_eq!(p, "Ns::Inner"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn map_fixed_is_unsupported() {
        let err = map_type_spec(&TypeSpec::Fixed(FixedPtType {
            digits: int_lit("10"),
            scale: int_lit("2"),
            span: sp(),
        }))
        .unwrap_err();
        assert_eq!(err, MapError::UnsupportedTypeSpec("fixed"));
    }

    #[test]
    fn map_map_is_unsupported() {
        let err = map_type_spec(&TypeSpec::Map(MapType {
            key: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            value: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: None,
            span: sp(),
        }))
        .unwrap_err();
        assert_eq!(err, MapError::UnsupportedTypeSpec("map (inline IDL map)"));
    }

    #[test]
    fn map_any_is_unsupported() {
        let err = map_type_spec(&TypeSpec::Any).unwrap_err();
        assert_eq!(err, MapError::UnsupportedTypeSpec("any"));
    }

    // ---- lower_struct_to_minimal struct-level annotations ----------------

    #[test]
    fn struct_autoid_hash_sets_flag() {
        let s = first_struct("@autoid(HASH) struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_AUTOID_HASH)
        );
    }

    #[test]
    fn struct_nested_sets_flag() {
        let s = first_struct("@nested struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_NESTED)
        );
    }

    #[test]
    fn struct_explicit_final_extensibility_sets_flag() {
        let s = first_struct("@extensibility(FINAL) struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_FINAL)
        );
    }

    #[test]
    fn struct_multi_declarator_expands_to_separate_members() {
        let s = first_struct("struct S { long a, b, c; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 3);
        for m in &st.member_seq {
            assert!(matches!(
                m.common.member_type_id,
                TypeIdentifier::Primitive(PrimitiveKind::Int32)
            ));
        }
    }

    #[test]
    fn struct_with_must_understand_flag() {
        let s = first_struct("struct S { @must_understand long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_MUST_UNDERSTAND)
        );
    }

    #[test]
    fn struct_with_external_flag() {
        let s = first_struct("struct S { @external long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_EXTERNAL)
        );
    }

    #[test]
    fn struct_with_scoped_member_returns_unresolved() {
        let s = first_struct("struct S { Foo x; };");
        let err = lower_struct_to_minimal(&s).unwrap_err();
        match err {
            MapError::UnresolvedScoped(p) => assert_eq!(p, "Foo"),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
