// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Map-Key-Validation (XTypes 1.3 §7.2.2.4.6.6 + §7.2.4.4.8).
//!
//! Spec-Constraint: Der Key-Typ einer `map<K, V, N>` MUSS einer der
//! folgenden sein:
//!   - Integer (int8/16/32/64, uint8/16/32/64)
//!   - char, wchar, octet, boolean
//!   - Enumerated (per ScopedName-Resolution)
//!   - String / WString
//!
//! Verboten: Struct, Union, Sequence, Array, Map, Fixed, Float-/Double-,
//! ValueType, Interface, Object/Any.

use crate::ast::{
    ConstrTypeDecl, Definition, Member, PrimitiveType, ScopedName, Specification, StructDef,
    TypeDecl, TypeSpec, TypedefDecl, UnionDef,
};
use crate::errors::Span;

/// Validierungs-Fehler einer `map<K, V, N>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapValidationError {
    /// §7.2.2.4.6.6 / §7.2.4.4.8 — Map-Key-Typ ist nicht in der erlaubten
    /// Whitelist (primitive int/char/bool/string/enum).
    InvalidMapKeyType {
        /// Beschreibung des verwendeten Key-Typs.
        kind: String,
        /// Quellort der Map-Deklaration.
        span: Span,
    },
}

/// Walks alle Maps in der Specification und liefert Verletzungen.
#[must_use]
pub fn validate_maps(spec: &Specification) -> Vec<MapValidationError> {
    let mut errs = Vec::new();
    for d in &spec.definitions {
        walk_def(d, &mut errs);
    }
    errs
}

/// zerodds-lint: recursion-depth 32
fn walk_def(d: &Definition, errs: &mut Vec<MapValidationError>) {
    match d {
        Definition::Module(m) => {
            for d in &m.definitions {
                walk_def(d, errs);
            }
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(crate::ast::StructDcl::Def(
            s,
        )))) => {
            walk_struct(s, errs);
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(crate::ast::UnionDcl::Def(u)))) => {
            walk_union(u, errs);
        }
        Definition::Type(TypeDecl::Typedef(td)) => walk_typedef(td, errs),
        _ => {}
    }
}

fn walk_struct(s: &StructDef, errs: &mut Vec<MapValidationError>) {
    for m in &s.members {
        let Member { type_spec, .. } = m;
        check_type_spec(type_spec, errs);
    }
}

fn walk_union(u: &UnionDef, errs: &mut Vec<MapValidationError>) {
    for case in &u.cases {
        check_type_spec(&case.element.type_spec, errs);
    }
}

fn walk_typedef(td: &TypedefDecl, errs: &mut Vec<MapValidationError>) {
    check_type_spec(&td.type_spec, errs);
}

/// zerodds-lint: recursion-depth 32
fn check_type_spec(ts: &TypeSpec, errs: &mut Vec<MapValidationError>) {
    match ts {
        TypeSpec::Map(m) => {
            if !is_valid_map_key(&m.key) {
                errs.push(MapValidationError::InvalidMapKeyType {
                    kind: describe(&m.key),
                    span: m.span,
                });
            }
            check_type_spec(&m.key, errs);
            check_type_spec(&m.value, errs);
        }
        TypeSpec::Sequence(s) => check_type_spec(&s.elem, errs),
        _ => {}
    }
}

fn is_valid_map_key(ts: &TypeSpec) -> bool {
    match ts {
        TypeSpec::Primitive(p) => matches!(
            p,
            PrimitiveType::Integer(_)
                | PrimitiveType::Boolean
                | PrimitiveType::Char
                | PrimitiveType::WideChar
                | PrimitiveType::Octet
        ),
        TypeSpec::String(_) => true,
        // Scoped name kann ein Enum oder ein typedef-Alias sein. Ohne
        // Resolver kann der Validator das nicht abschliessend ueberpruefen
        // — wir akzeptieren scoped-name-Keys hier optimistisch und
        // verlassen uns auf den Resolver-Pass, der dann nicht-Enum
        // scoped-names mit eigener Diagnose meldet.
        TypeSpec::Scoped(_) => true,
        _ => false,
    }
}

fn describe(ts: &TypeSpec) -> String {
    match ts {
        TypeSpec::Primitive(p) => alloc::format!("{p:?}"),
        TypeSpec::String(_) => "string".into(),
        TypeSpec::Sequence(_) => "sequence".into(),
        TypeSpec::Map(_) => "map".into(),
        TypeSpec::Fixed(_) => "fixed".into(),
        TypeSpec::Scoped(s) => describe_scoped(s),
        _ => "other".into(),
    }
}

fn describe_scoped(s: &ScopedName) -> String {
    let name: alloc::vec::Vec<&str> = s.parts.iter().map(|p| p.text.as_str()).collect();
    name.join("::")
}

extern crate alloc;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn parse_to_ast(src: &str) -> Specification {
        parse(src, &ParserConfig::default()).expect("parse ok")
    }

    #[test]
    fn map_key_primitive_int_is_valid() {
        let ast = parse_to_ast("struct S { map<long, long, 10> m; };");
        let errs = validate_maps(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }

    #[test]
    fn map_key_string_is_valid() {
        let ast = parse_to_ast("struct S { map<string, long, 10> m; };");
        let errs = validate_maps(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }

    #[test]
    fn map_key_element_type_must_be_primitive() {
        // sequence<long> als Key-Typ — nicht erlaubt.
        let ast = parse_to_ast("struct S { map<sequence<long>, long, 10> m; };");
        let errs = validate_maps(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, MapValidationError::InvalidMapKeyType { .. })),
            "expected InvalidMapKeyType, got {errs:?}"
        );
    }

    #[test]
    fn map_key_nested_map_is_invalid() {
        let ast = parse_to_ast("struct S { map<map<long, long, 5>, long, 10> m; };");
        let errs = validate_maps(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, MapValidationError::InvalidMapKeyType { .. }))
        );
    }

    #[test]
    fn map_key_in_typedef_is_validated() {
        let ast = parse_to_ast("typedef map<sequence<long>, long, 10> M;");
        let errs = validate_maps(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, MapValidationError::InvalidMapKeyType { .. }))
        );
    }

    #[test]
    fn map_value_type_can_be_anything() {
        // Value-Typ ist NICHT eingeschraenkt — sequence<long> als Value
        // ist OK.
        let ast = parse_to_ast("struct S { map<long, sequence<long>, 10> m; };");
        let errs = validate_maps(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }
}
