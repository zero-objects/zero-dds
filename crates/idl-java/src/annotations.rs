// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Annotation-Bridge: IDL-Builtins → Java-Source-Annotations (C5.4-b).
//!
//! Wandelt das `Lowered`-Datenmodell aus
//! [`zerodds_idl::semantics::annotations`] in Java-Annotation-Quelltext
//! um. Das ist die einzige Stelle, an der die FQN der Runtime-
//! Annotations (`org.zerodds.types.*`) gebunden ist; aendert sich der
//! Runtime-Pfad, wird hier eine Konstante geaendert und alle
//! Generierungs-Stellen profitieren davon.
//!
//! ## Mapping (vgl. `runtime/README.md`)
//!
//! | IDL                              | Java                                              | Target |
//! |----------------------------------|---------------------------------------------------|--------|
//! | `@key`                           | `@org.zerodds.types.Key`                          | FIELD  |
//! | `@id(N)`                         | `@org.zerodds.types.Id(N)`                        | FIELD  |
//! | `@optional`                      | `@org.zerodds.types.Optional`                     | FIELD  |
//! | `@must_understand`               | `@org.zerodds.types.MustUnderstand`               | FIELD  |
//! | `@external`                      | `@org.zerodds.types.External`                     | FIELD  |
//! | `@nested`                        | `@org.zerodds.types.Nested`                       | TYPE   |
//! | `@extensibility(FINAL|APPENDABLE|MUTABLE)` | `@org.zerodds.types.Extensibility(Kind.FINAL)` | TYPE   |
//! | `@final`/`@appendable`/`@mutable` | shorthand → `@Extensibility(Kind.X)`            | TYPE   |
//!
//! Andere Builtins (`@autoid`, `@unit`, `@hashid`, `@min`/`@max`,
//! `@range`, `@default`, `@topic`, `@verbatim`, `@bit_bound`,
//! `@position`, `@value`, `@default_literal`) werden in dieser
//! Foundation **nicht** auf Java-Annotations gespiegelt — entweder
//! weil sie semantisch im Code-Layout aufgehen (z.B. `@value` auf
//! Enum-Members), weil es bislang keinen Java-Konsumenten gibt
//! (`@unit`, `@range`), oder weil sie als Konstrukt-Marker im IDL
//! bleiben (`@bit_bound`, `@position`).

use zerodds_idl::ast::Annotation;
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, Lowered, lower_annotations,
};

/// Java-FQN-Praefix der Runtime-Annotations.
pub(crate) const RUNTIME_PREFIX: &str = "org.zerodds.types";

/// Lowered-View ueber eine Annotation-Liste; bei Lower-Fehlern fallen
/// wir defensiv auf `Lowered::default()` zurueck und reichen die rohen
/// Annotations als `custom` durch — der Codegen soll *robust* gegen
/// fehlerhafte IDL-Annotations sein, der Parser hat sie zuvor schon
/// strukturell akzeptiert.
pub(crate) fn lower_or_empty(anns: &[Annotation]) -> Lowered {
    lower_annotations(anns).unwrap_or_default()
}

/// Wandelt eine Member-Annotation-Liste in eine Liste von
/// Java-Annotation-Zeilen um (jede Zeile inkl. fuehrender `@`).
///
/// Reihenfolge ist deterministisch: `@Key` → `@Id` → `@Optional` →
/// `@MustUnderstand` → `@External` (alphabetisch nach Java-Token).
pub(crate) fn member_annotation_lines(anns: &[Annotation]) -> Vec<String> {
    let lowered = lower_or_empty(anns);
    let mut out: Vec<String> = Vec::new();
    let mut has_key = false;
    let mut has_optional = false;
    let mut has_must = false;
    let mut has_external = false;
    let mut has_shared = false;
    let mut explicit_id: Option<u32> = None;
    for b in &lowered.builtins {
        match b {
            BuiltinAnnotation::Key => has_key = true,
            BuiltinAnnotation::Id(n) => explicit_id = Some(*n),
            BuiltinAnnotation::Optional => has_optional = true,
            BuiltinAnnotation::MustUnderstand => has_must = true,
            BuiltinAnnotation::External => has_external = true,
            BuiltinAnnotation::Shared => has_shared = true,
            _ => {}
        }
    }
    if has_key {
        out.push(format!("@{RUNTIME_PREFIX}.Key"));
    }
    if let Some(n) = explicit_id {
        out.push(format!("@{RUNTIME_PREFIX}.Id({n})"));
    }
    if has_optional {
        out.push(format!("@{RUNTIME_PREFIX}.Optional"));
    }
    if has_must {
        out.push(format!("@{RUNTIME_PREFIX}.MustUnderstand"));
    }
    if has_external {
        out.push(format!("@{RUNTIME_PREFIX}.External"));
    }
    if has_shared {
        // §8.1.5 (idl4-cpp / dds-psm-cxx): @shared -> Pointer-Semantik.
        // In Java sind alle Class-Felder ohnehin Reference-Types; wir
        // emittieren das `@Shared`-Marker-Annotation analog @External
        // (Sharing-Hint fuer Codegen-Konsumenten).
        out.push(format!("@{RUNTIME_PREFIX}.Shared"));
    }
    out
}

/// Wandelt eine Type-Annotation-Liste in eine Liste von Annotationen
/// am Class/Enum-Header (TYPE-Target).
pub(crate) fn type_annotation_lines(anns: &[Annotation]) -> Vec<String> {
    let lowered = lower_or_empty(anns);
    let mut out: Vec<String> = Vec::new();
    if has_nested(&lowered) {
        out.push(format!("@{RUNTIME_PREFIX}.Nested"));
    }
    if let Some(kind) = lowered.extensibility() {
        let lit = match kind {
            ExtensibilityKind::Final => "FINAL",
            ExtensibilityKind::Appendable => "APPENDABLE",
            ExtensibilityKind::Mutable => "MUTABLE",
        };
        out.push(format!(
            "@{RUNTIME_PREFIX}.Extensibility({RUNTIME_PREFIX}.Extensibility.Kind.{lit})",
        ));
    }
    out
}

/// `true` wenn `@nested` (ohne Argument oder mit `TRUE`) vorliegt.
pub(crate) fn has_nested(lowered: &Lowered) -> bool {
    lowered
        .builtins
        .iter()
        .any(|b| matches!(b, BuiltinAnnotation::Nested))
}

/// Liefert den expliziten `@value`-Ausdruck eines Enumerators als
/// String, falls vorhanden. Erwartet eine Integer-Literal-Form.
pub(crate) fn enum_value_override(anns: &[Annotation]) -> Option<String> {
    let lowered = lower_or_empty(anns);
    lowered.builtins.iter().find_map(|b| match b {
        BuiltinAnnotation::Value(s) => Some(s.clone()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use zerodds_idl::config::ParserConfig;

    fn parse(src: &str) -> zerodds_idl::ast::Specification {
        zerodds_idl::parse(src, &ParserConfig::default()).expect("parse")
    }

    fn first_member_anns(src: &str) -> Vec<Annotation> {
        let ast = parse(src);
        for d in &ast.definitions {
            if let zerodds_idl::ast::Definition::Type(zerodds_idl::ast::TypeDecl::Constr(
                zerodds_idl::ast::ConstrTypeDecl::Struct(zerodds_idl::ast::StructDcl::Def(s)),
            )) = d
            {
                if let Some(m) = s.members.first() {
                    return m.annotations.clone();
                }
            }
        }
        Vec::new()
    }

    fn struct_anns(src: &str) -> Vec<Annotation> {
        let ast = parse(src);
        for d in &ast.definitions {
            if let zerodds_idl::ast::Definition::Type(zerodds_idl::ast::TypeDecl::Constr(
                zerodds_idl::ast::ConstrTypeDecl::Struct(zerodds_idl::ast::StructDcl::Def(s)),
            )) = d
            {
                return s.annotations.clone();
            }
        }
        Vec::new()
    }

    #[test]
    fn key_emits_key_annotation() {
        let anns = first_member_anns("struct S { @key long id; };");
        assert_eq!(
            member_annotation_lines(&anns),
            vec!["@org.zerodds.types.Key".to_string()],
        );
    }

    #[test]
    fn id_emits_id_with_value() {
        let anns = first_member_anns("struct S { @id(7) long x; };");
        assert_eq!(
            member_annotation_lines(&anns),
            vec!["@org.zerodds.types.Id(7)".to_string()],
        );
    }

    #[test]
    fn optional_emits_optional_annotation() {
        let anns = first_member_anns("struct S { @optional long x; };");
        assert_eq!(
            member_annotation_lines(&anns),
            vec!["@org.zerodds.types.Optional".to_string()],
        );
    }

    #[test]
    fn must_understand_emits_marker() {
        let anns = first_member_anns("struct S { @must_understand long x; };");
        assert_eq!(
            member_annotation_lines(&anns),
            vec!["@org.zerodds.types.MustUnderstand".to_string()],
        );
    }

    #[test]
    fn external_emits_marker() {
        let anns = first_member_anns("struct S { @external long x; };");
        assert_eq!(
            member_annotation_lines(&anns),
            vec!["@org.zerodds.types.External".to_string()],
        );
    }

    #[test]
    fn key_id_optional_combine_in_deterministic_order() {
        let anns = first_member_anns("struct S { @optional @id(3) @key long x; };");
        assert_eq!(
            member_annotation_lines(&anns),
            vec![
                "@org.zerodds.types.Key".to_string(),
                "@org.zerodds.types.Id(3)".to_string(),
                "@org.zerodds.types.Optional".to_string(),
            ],
        );
    }

    #[test]
    fn nested_struct_emits_nested_type_annotation() {
        let anns = struct_anns("@nested struct S { long x; };");
        assert_eq!(
            type_annotation_lines(&anns),
            vec!["@org.zerodds.types.Nested".to_string()],
        );
    }

    #[test]
    fn final_struct_emits_extensibility_annotation() {
        let anns = struct_anns("@final struct S { long x; };");
        assert_eq!(
            type_annotation_lines(&anns),
            vec![
                "@org.zerodds.types.Extensibility(\
                  org.zerodds.types.Extensibility.Kind.FINAL)"
                    .to_string()
            ],
        );
    }

    #[test]
    fn mutable_struct_emits_extensibility_mutable() {
        let anns = struct_anns("@mutable struct S { long x; };");
        assert_eq!(
            type_annotation_lines(&anns),
            vec![
                "@org.zerodds.types.Extensibility(\
                  org.zerodds.types.Extensibility.Kind.MUTABLE)"
                    .to_string()
            ],
        );
    }

    #[test]
    fn appendable_explicit_extensibility_emits() {
        let anns = struct_anns("@extensibility(APPENDABLE) struct S { long x; };");
        assert_eq!(
            type_annotation_lines(&anns),
            vec![
                "@org.zerodds.types.Extensibility(\
                  org.zerodds.types.Extensibility.Kind.APPENDABLE)"
                    .to_string()
            ],
        );
    }

    #[test]
    fn no_annotations_yields_empty_lists() {
        let anns = first_member_anns("struct S { long x; };");
        assert!(member_annotation_lines(&anns).is_empty());
        let tann = struct_anns("struct S { long x; };");
        assert!(type_annotation_lines(&tann).is_empty());
    }

    #[test]
    fn enum_value_override_returns_literal() {
        // Synthetic: a single annotation `@value(7)` on a member
        // (testing the lowering helper, not the IDL syntax).
        let anns = first_member_anns("struct S { @value(7) long x; };");
        assert_eq!(enum_value_override(&anns), Some("7".to_string()));
    }

    #[test]
    fn enum_value_override_absent_returns_none() {
        let anns = first_member_anns("struct S { long x; };");
        assert_eq!(enum_value_override(&anns), None);
    }
}
