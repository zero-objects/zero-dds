// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL → C# attribute bridge (C5.3-b).
//!
//! Maps typed `BuiltinAnnotation`s from `zerodds_idl::semantics::annotations`
//! to C# attribute strings that the emitter writes into the output.
//!
//! The C# attribute classes themselves live in the runtime lib `Omg.Types`
//! (see `runtime/Omg.Types.cs`). Generated code must have `using Omg.Types;`
//! as soon as any member or type carries an annotation.
//!
//! ## Mapping table
//!
//! | IDL                           | C# attribute                                |
//! |-------------------------------|---------------------------------------------|
//! | `@key`                        | `[Key]`                                     |
//! | `@id(N)`                      | `[Id(N)]`                                   |
//! | `@optional`                   | `[Optional]` + `T?`                         |
//! | `@must_understand`            | `[MustUnderstand]`                          |
//! | `@external`                   | `[External]`                                |
//! | `@nested`                     | `[Nested]` (type level)                     |
//! | `@extensibility(FINAL)`       | `[Extensibility(ExtensibilityKind.Final)]`  |
//! | `@extensibility(APPENDABLE)`  | `[Extensibility(ExtensibilityKind.Appendable)]` |
//! | `@extensibility(MUTABLE)`     | `[Extensibility(ExtensibilityKind.Mutable)]` |
//! | `@final` (shorthand)          | `[Extensibility(ExtensibilityKind.Final)]`  |
//! | `@appendable` (shorthand)     | `[Extensibility(ExtensibilityKind.Appendable)]` |
//! | `@mutable` (shorthand)        | `[Extensibility(ExtensibilityKind.Mutable)]` |
//!
//! ## Deliberately NOT mapped
//!
//! - `@autoid(...)`     — consumed during the TypeObject build, no
//!                        visible effect for C# codegen; phase 6+.
//! - `@topic`           — implicit via the `ITopicType<T>` marker
//!                        (see `emitter::is_topic_type`).
//! - `@unit`, `@hashid`,
//!   `@range`, `@min`, `@max`, `@value`, `@position`, `@bit_bound`,
//!   `@default`, `@default_literal`, `@verbatim` — semantic hints
//!   without a wire effect in the C# mapping; phase 6+.

use zerodds_idl::ast::Annotation;
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, lower_annotations, lower_type_annotations,
};

/// Result of the annotation bridging: strings that the emitter
/// places as a line before the member/type.
#[derive(Debug, Default, Clone)]
pub(crate) struct CsAttributes {
    /// One line per C# attribute, without a trailing newline.
    pub(crate) attrs: Vec<String>,
    /// `true` if at least one attribute was emitted — the emitter
    /// uses this to set the `using Omg.Types;` import.
    pub(crate) needs_omg_types: bool,
    /// `true` if `@optional` is set (the emitter then additionally makes
    /// the type nullable).
    pub(crate) optional: bool,
    /// `true` if `@shared` is set (the emitter wraps the type in a
    /// reference wrapper / `T?` reference).
    pub(crate) shared: bool,
}

impl CsAttributes {
    fn push(&mut self, s: impl Into<String>) {
        self.attrs.push(s.into());
        self.needs_omg_types = true;
    }
}

/// Bridge for member-level annotations (`@key`, `@id`, `@optional`,
/// `@must_understand`, `@external`).
///
/// Unknown / unmapped annotations are ignored (they are
/// already in the AST anyway).
pub(crate) fn member_attributes(anns: &[Annotation]) -> CsAttributes {
    let mut out = CsAttributes::default();
    let Ok(lowered) = lower_annotations(anns) else {
        // Lower error: silent skip — codegen is not the place for
        // annotation validation, the lowering does that itself.
        return out;
    };
    for b in &lowered.builtins {
        match b {
            BuiltinAnnotation::Key => out.push("[Key]"),
            BuiltinAnnotation::Id(n) => out.push(format!("[Id({n})]")),
            BuiltinAnnotation::Optional => {
                out.push("[Optional]");
                out.optional = true;
            }
            BuiltinAnnotation::Shared => {
                // §8.1.5 idl4-cpp / dds-psm-cxx: @shared -> Pointer.
                // C# has no direct shared_ptr equivalent; we use
                // the `[Shared]` marker attribute (annotation definition
                // in the Omg.Types runtime) and pass through the C#
                // reference-type character (class instead of struct).
                out.push("[Shared]");
                out.shared = true;
            }
            BuiltinAnnotation::MustUnderstand => out.push("[MustUnderstand]"),
            BuiltinAnnotation::External => out.push("[External]"),
            // Member level: @nested is not allowed per spec.
            // Member level: @extensibility is not allowed per spec.
            // Other annotations (default/min/max/range/...) are
            // semantic hints without a C# attribute mapping in phase 5.
            _ => {}
        }
    }
    out
}

/// Bridge for type-level annotations (`@nested`, `@extensibility`,
/// `@final`/`@appendable`/`@mutable`).
pub(crate) fn type_attributes(anns: &[Annotation]) -> CsAttributes {
    let mut out = CsAttributes::default();
    let Ok(lowered) = lower_type_annotations(anns) else {
        return out;
    };
    for b in &lowered.builtins {
        match b {
            BuiltinAnnotation::Nested => out.push("[Nested]"),
            BuiltinAnnotation::Extensibility(k) => {
                out.push(extensibility_attr(*k));
            }
            BuiltinAnnotation::Final => out.push(extensibility_attr(ExtensibilityKind::Final)),
            BuiltinAnnotation::Appendable => {
                out.push(extensibility_attr(ExtensibilityKind::Appendable));
            }
            BuiltinAnnotation::Mutable => {
                out.push(extensibility_attr(ExtensibilityKind::Mutable));
            }
            _ => {}
        }
    }
    out
}

/// `true` if a type is *not* marked `@nested` (i.e. topic-capable).
/// Default: top-level structs are topic types unless explicitly
/// annotated `@nested`.
pub(crate) fn is_nested_type(anns: &[Annotation]) -> bool {
    let Ok(lowered) = lower_type_annotations(anns) else {
        return false;
    };
    lowered
        .builtins
        .iter()
        .any(|b| matches!(b, BuiltinAnnotation::Nested))
}

fn extensibility_attr(k: ExtensibilityKind) -> String {
    let kind = match k {
        ExtensibilityKind::Final => "Final",
        ExtensibilityKind::Appendable => "Appendable",
        ExtensibilityKind::Mutable => "Mutable",
    };
    format!("[Extensibility(ExtensibilityKind.{kind})]")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use zerodds_idl::config::ParserConfig;

    fn parse_first_member_anns(src: &str) -> Vec<Annotation> {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
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

    fn parse_struct_anns(src: &str) -> Vec<Annotation> {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
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
    fn key_maps_to_key_attribute() {
        let anns = parse_first_member_anns("struct S { @key long id; };");
        let cs = member_attributes(&anns);
        assert_eq!(cs.attrs, vec!["[Key]"]);
        assert!(cs.needs_omg_types);
        assert!(!cs.optional);
    }

    #[test]
    fn id_maps_to_id_attribute_with_value() {
        let anns = parse_first_member_anns("struct S { @id(42) long x; };");
        let cs = member_attributes(&anns);
        assert_eq!(cs.attrs, vec!["[Id(42)]"]);
    }

    #[test]
    fn optional_sets_optional_flag() {
        let anns = parse_first_member_anns("struct S { @optional long x; };");
        let cs = member_attributes(&anns);
        assert_eq!(cs.attrs, vec!["[Optional]"]);
        assert!(cs.optional);
    }

    #[test]
    fn must_understand_maps() {
        let anns = parse_first_member_anns("struct S { @must_understand long x; };");
        let cs = member_attributes(&anns);
        assert_eq!(cs.attrs, vec!["[MustUnderstand]"]);
    }

    #[test]
    fn external_maps() {
        let anns = parse_first_member_anns("struct S { @external long x; };");
        let cs = member_attributes(&anns);
        assert_eq!(cs.attrs, vec!["[External]"]);
    }

    #[test]
    fn member_attributes_empty_when_no_annotations() {
        let anns = parse_first_member_anns("struct S { long x; };");
        let cs = member_attributes(&anns);
        assert!(cs.attrs.is_empty());
        assert!(!cs.needs_omg_types);
    }

    #[test]
    fn nested_maps_to_nested_attribute() {
        let anns = parse_struct_anns("@nested struct S { long x; };");
        let cs = type_attributes(&anns);
        assert_eq!(cs.attrs, vec!["[Nested]"]);
    }

    #[test]
    fn final_shorthand_maps_to_extensibility_final() {
        let anns = parse_struct_anns("@final struct S { long x; };");
        let cs = type_attributes(&anns);
        assert_eq!(
            cs.attrs,
            vec!["[Extensibility(ExtensibilityKind.Final)]".to_string()]
        );
    }

    #[test]
    fn appendable_shorthand_maps() {
        let anns = parse_struct_anns("@appendable struct S { long x; };");
        let cs = type_attributes(&anns);
        assert_eq!(
            cs.attrs,
            vec!["[Extensibility(ExtensibilityKind.Appendable)]".to_string()]
        );
    }

    #[test]
    fn mutable_shorthand_maps() {
        let anns = parse_struct_anns("@mutable struct S { long x; };");
        let cs = type_attributes(&anns);
        assert_eq!(
            cs.attrs,
            vec!["[Extensibility(ExtensibilityKind.Mutable)]".to_string()]
        );
    }

    #[test]
    fn extensibility_full_form_maps() {
        let anns = parse_struct_anns("@extensibility(MUTABLE) struct S { long x; };");
        let cs = type_attributes(&anns);
        assert_eq!(
            cs.attrs,
            vec!["[Extensibility(ExtensibilityKind.Mutable)]".to_string()]
        );
    }

    #[test]
    fn is_nested_true_for_nested_struct() {
        let anns = parse_struct_anns("@nested struct S { long x; };");
        assert!(is_nested_type(&anns));
    }

    #[test]
    fn is_nested_false_for_plain_struct() {
        let anns = parse_struct_anns("struct S { long x; };");
        assert!(!is_nested_type(&anns));
    }

    #[test]
    fn unrelated_annotation_is_ignored() {
        let anns = parse_first_member_anns("struct S { @vendor_tag long x; };");
        let cs = member_attributes(&anns);
        assert!(cs.attrs.is_empty());
    }

    #[test]
    fn key_and_id_combine_in_order() {
        let anns = parse_first_member_anns("struct S { @key @id(7) long id; };");
        let cs = member_attributes(&anns);
        // The order matches the source order.
        assert_eq!(cs.attrs, vec!["[Key]".to_string(), "[Id(7)]".to_string()]);
    }
}
