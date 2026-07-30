// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C4.3 — builtin-annotations apply bridge: lowers IDL annotations
//! (see [`super::annotations::BuiltinAnnotation`]) onto XTypes 1.3
//! `MemberDescriptor`/`TypeDescriptor` fields
//! (`zerodds_types::dynamic::*`).
//!
//! Spec reference: XTypes 1.3 §7.2.2.4.4.4 (builtin annotations) +
//! §7.5.4.1.2 (member-descriptor fields). The IDL-4.2 spec §1.10
//! allows the same annotation set; we map 1:1.
//!
//! # What happens here
//!
//! This bridge takes a `Lowered` collection (from the
//! annotation lowering in [`super::annotations`]) and applies it
//! to a fresh `MemberDescriptor` or `TypeDescriptor` from
//! `zerodds_types::dynamic::descriptor`. The bridge is **read-only on
//! both sides** — it constructs NEW descriptor values with the
//! annotation effects, without mutating the existing code.
//!
//! # Mapping table
//!
//! | IDL-Annotation         | Effekt im DynamicType-Descriptor          |
//! |------------------------|-------------------------------------------|
//! | `@key`                 | `MemberDescriptor.is_key = true`          |
//! | `@id(N)`               | `MemberDescriptor.id = N`                 |
//! | `@optional`            | `MemberDescriptor.is_optional = true`     |
//! | `@must_understand`     | `MemberDescriptor.is_must_understand = true` |
//! | `@external`            | `MemberDescriptor.is_shared = true`       |
//! | `@default(value)`      | `MemberDescriptor.default_value = Some(value)` |
//! | `@extensibility(...)` / `@final` / `@appendable` / `@mutable` | `TypeDescriptor.extensibility_kind` |
//! | `@nested`              | `TypeDescriptor.is_nested = true`         |
//! | `@autoid(...)`         | (not in the descriptor — builder side)    |
//! | `@unit("...")`         | TypeObject `AppliedBuiltinMemberAnnotations.unit` (complete codegen) — NOT in the runtime `MemberDescriptor` (spec §7.5.2.7) |
//! | `@hashid(hint)`        | (hash override — XTypes §7.3.1.2.1.4)     |
//! | `@bit_bound(N)`        | (bitmask-specific — separate bridge)      |
//! | `@position(N)`         | (bitmask-specific)                        |
//!
//! Annotations that do not fall into the XTypes descriptor set
//! (`@verbatim`, `@unit`, `@range`, `@min`, `@max`, `@value`,
//! `@default_literal`, `@topic`, `@autoid`) are carried along in the `MemberApplyReport`
//! / `TypeApplyReport` lists, so that code generators or
//! downstream layers can consume them.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_types::dynamic::descriptor::{
    ExtensibilityKind as DynExtKind, MemberDescriptor, TryConstructKind as DynTryConstruct,
    TypeDescriptor,
};

use super::annotations::{
    AutoidKind, BuiltinAnnotation, ExtensibilityKind as IdlExtKind, Lowered,
    TryConstructKind as IdlTryConstruct,
};

/// Report on the apply of an annotation collection to a member.
///
/// Contains the list of annotations **not** handled by the apply
/// (e.g. `@unit`, `@hashid`, `@verbatim`), which the caller can forward
/// (e.g. codegen, XML annotation section).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemberApplyReport {
    /// Annotation names not directly mapped onto MemberDescriptor fields.
    /// Order as in the Lowered.
    pub passthrough: Vec<String>,
    /// If `@autoid(SEQUENTIAL|HASH)` was set on a member container,
    /// passed through here (builder side).
    pub autoid: Option<AutoidKind>,
}

/// Report on the apply to a type.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TypeApplyReport {
    /// Annotation names not directly mapped onto TypeDescriptor fields.
    pub passthrough: Vec<String>,
    /// `@autoid` at the type level (inherited by the members).
    pub autoid: Option<AutoidKind>,
}

/// Applies the lowered annotations to a MemberDescriptor.
/// Returns the new descriptor + the apply report with passthrough
/// annotations.
///
/// Member-specific annotations are mapped 1:1 onto fields;
/// type-specific annotations (`@nested`, `@extensibility`,
/// `@autoid`) are ignored (they belong to the container type).
#[must_use]
pub fn apply_to_member(
    base: MemberDescriptor,
    lowered: &Lowered,
) -> (MemberDescriptor, MemberApplyReport) {
    let mut out = base;
    let mut report = MemberApplyReport::default();

    for ann in &lowered.builtins {
        match ann {
            BuiltinAnnotation::Key => out.is_key = true,
            BuiltinAnnotation::Id(n) => out.id = *n,
            BuiltinAnnotation::Optional => out.is_optional = true,
            BuiltinAnnotation::MustUnderstand => out.is_must_understand = true,
            BuiltinAnnotation::External | BuiltinAnnotation::Shared => out.is_shared = true,
            BuiltinAnnotation::NonSerialized => {
                // §7.2.4.4.2 — during TypeObject lowering, @non_serialized
                // members are dropped. If the call still happens here
                // (e.g. via the DynamicType bridge), we
                // mark the member as non-relevant for wire +
                // assignability via passthrough.
                report.passthrough.push("non_serialized".into());
            }
            BuiltinAnnotation::IgnoreLiteralNames => {
                // Enum-type-level annotation; the member bridge only passes
                // it through.
                report.passthrough.push("ignore_literal_names".into());
            }
            BuiltinAnnotation::Default(value) => out.default_value = Some(value.clone()),
            BuiltinAnnotation::DefaultLiteral => {
                // Spec §7.3.1.2.1.16 — the member is the default branch
                // of a union. Mirrored via MemberDescriptor.is_default_label.
                out.is_default_label = true;
            }
            BuiltinAnnotation::Position(n) => {
                // Bitmask bit position: member ID = position (spec
                // §7.3.1.2.1.18). Only if no explicit `@id(...)`
                // was set — that one wins.
                if !lowered
                    .builtins
                    .iter()
                    .any(|a| matches!(a, BuiltinAnnotation::Id(_)))
                {
                    out.id = *n;
                }
            }
            // Type-container level — applied by the type bridge.
            BuiltinAnnotation::Nested
            | BuiltinAnnotation::Extensibility(_)
            | BuiltinAnnotation::Final
            | BuiltinAnnotation::Appendable
            | BuiltinAnnotation::Mutable => {}
            BuiltinAnnotation::Autoid(kind) => report.autoid = Some(*kind),
            // §7.2.4.2 — @try_construct maps 1:1 onto the MemberDescriptor
            // field (the DynamicData bridge honours it via apply_try_construct).
            BuiltinAnnotation::TryConstruct(kind) => {
                out.try_construct = match kind {
                    IdlTryConstruct::Discard => DynTryConstruct::Discard,
                    IdlTryConstruct::UseDefault => DynTryConstruct::UseDefault,
                    IdlTryConstruct::Trim => DynTryConstruct::Trim,
                };
            }
            // Pass-through: codegen-/tooling-relevant but not mapped in the
            // spec MemberDescriptor.
            BuiltinAnnotation::Topic
            | BuiltinAnnotation::Unit(_)
            | BuiltinAnnotation::HashId(_)
            | BuiltinAnnotation::Range { .. }
            | BuiltinAnnotation::Min(_)
            | BuiltinAnnotation::Max(_)
            | BuiltinAnnotation::Value(_)
            | BuiltinAnnotation::BitBound(_)
            | BuiltinAnnotation::Verbatim(_)
            | BuiltinAnnotation::Ami(_)
            | BuiltinAnnotation::Service(_)
            | BuiltinAnnotation::OnewayAnno(_) => {
                report.passthrough.push(annotation_name(ann).into());
            }
        }
    }

    // Also forwards `custom` annotations (vendor extension etc.).
    for c in &lowered.custom {
        if let Some(tail) = c.name.parts.last() {
            report.passthrough.push(tail.text.clone());
        }
    }

    (out, report)
}

/// Applies the lowered annotations to a TypeDescriptor.
#[must_use]
pub fn apply_to_type(base: TypeDescriptor, lowered: &Lowered) -> (TypeDescriptor, TypeApplyReport) {
    let mut out = base;
    let mut report = TypeApplyReport::default();

    for ann in &lowered.builtins {
        match ann {
            BuiltinAnnotation::Nested => out.is_nested = true,
            BuiltinAnnotation::Extensibility(kind) => {
                out.extensibility_kind = idl_to_dynamic_ext(*kind);
            }
            BuiltinAnnotation::Final => out.extensibility_kind = DynExtKind::Final,
            BuiltinAnnotation::Appendable => out.extensibility_kind = DynExtKind::Appendable,
            BuiltinAnnotation::Mutable => out.extensibility_kind = DynExtKind::Mutable,
            BuiltinAnnotation::Autoid(kind) => report.autoid = Some(*kind),
            // Member level — applied by the member bridge.
            BuiltinAnnotation::Key
            | BuiltinAnnotation::Id(_)
            | BuiltinAnnotation::Optional
            | BuiltinAnnotation::Shared
            | BuiltinAnnotation::MustUnderstand
            | BuiltinAnnotation::External
            | BuiltinAnnotation::NonSerialized
            | BuiltinAnnotation::IgnoreLiteralNames
            | BuiltinAnnotation::Default(_)
            | BuiltinAnnotation::DefaultLiteral
            | BuiltinAnnotation::TryConstruct(_)
            | BuiltinAnnotation::Position(_) => {}
            // Pass-Through.
            BuiltinAnnotation::Topic
            | BuiltinAnnotation::Unit(_)
            | BuiltinAnnotation::HashId(_)
            | BuiltinAnnotation::Range { .. }
            | BuiltinAnnotation::Min(_)
            | BuiltinAnnotation::Max(_)
            | BuiltinAnnotation::Value(_)
            | BuiltinAnnotation::BitBound(_)
            | BuiltinAnnotation::Verbatim(_)
            | BuiltinAnnotation::Ami(_)
            | BuiltinAnnotation::Service(_)
            | BuiltinAnnotation::OnewayAnno(_) => {
                report.passthrough.push(annotation_name(ann).into());
            }
        }
    }

    for c in &lowered.custom {
        if let Some(tail) = c.name.parts.last() {
            report.passthrough.push(tail.text.clone());
        }
    }

    (out, report)
}

fn idl_to_dynamic_ext(kind: IdlExtKind) -> DynExtKind {
    match kind {
        IdlExtKind::Final => DynExtKind::Final,
        IdlExtKind::Appendable => DynExtKind::Appendable,
        IdlExtKind::Mutable => DynExtKind::Mutable,
    }
}

fn annotation_name(ann: &BuiltinAnnotation) -> &'static str {
    match ann {
        BuiltinAnnotation::Key => "key",
        BuiltinAnnotation::Id(_) => "id",
        BuiltinAnnotation::Optional => "optional",
        BuiltinAnnotation::Shared => "shared",
        BuiltinAnnotation::MustUnderstand => "must_understand",
        BuiltinAnnotation::External => "external",
        BuiltinAnnotation::NonSerialized => "non_serialized",
        BuiltinAnnotation::IgnoreLiteralNames => "ignore_literal_names",
        BuiltinAnnotation::Default(_) => "default",
        BuiltinAnnotation::DefaultLiteral => "default_literal",
        BuiltinAnnotation::Extensibility(_) => "extensibility",
        BuiltinAnnotation::Final => "final",
        BuiltinAnnotation::Appendable => "appendable",
        BuiltinAnnotation::Mutable => "mutable",
        BuiltinAnnotation::Autoid(_) => "autoid",
        BuiltinAnnotation::TryConstruct(_) => "try_construct",
        BuiltinAnnotation::Topic => "topic",
        BuiltinAnnotation::Nested => "nested",
        BuiltinAnnotation::Unit(_) => "unit",
        BuiltinAnnotation::HashId(_) => "hashid",
        BuiltinAnnotation::Range { .. } => "range",
        BuiltinAnnotation::Min(_) => "min",
        BuiltinAnnotation::Max(_) => "max",
        BuiltinAnnotation::Value(_) => "value",
        BuiltinAnnotation::Position(_) => "position",
        BuiltinAnnotation::BitBound(_) => "bit_bound",
        BuiltinAnnotation::Verbatim(_) => "verbatim",
        BuiltinAnnotation::Ami(_) => "ami",
        BuiltinAnnotation::Service(_) => "service",
        BuiltinAnnotation::OnewayAnno(_) => "oneway",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_types::dynamic::descriptor::{TypeDescriptor, TypeKind};

    fn lowered_with(builtins: Vec<BuiltinAnnotation>) -> Lowered {
        Lowered {
            builtins,
            custom: Vec::new(),
        }
    }

    fn member(name: &str) -> MemberDescriptor {
        MemberDescriptor::new(name, 1, TypeDescriptor::primitive(TypeKind::Int32, "long"))
    }

    #[test]
    fn key_marks_member() {
        let (m, _) = apply_to_member(member("x"), &lowered_with(vec![BuiltinAnnotation::Key]));
        assert!(m.is_key);
    }

    #[test]
    fn explicit_id_overrides_default() {
        let (m, _) = apply_to_member(member("x"), &lowered_with(vec![BuiltinAnnotation::Id(42)]));
        assert_eq!(m.id, 42);
    }

    #[test]
    fn optional_must_understand_external_marks() {
        let l = lowered_with(vec![
            BuiltinAnnotation::Optional,
            BuiltinAnnotation::MustUnderstand,
            BuiltinAnnotation::External,
        ]);
        let (m, _) = apply_to_member(member("x"), &l);
        assert!(m.is_optional);
        assert!(m.is_must_understand);
        assert!(m.is_shared);
    }

    #[test]
    fn default_value_propagates() {
        let l = lowered_with(vec![BuiltinAnnotation::Default("42".into())]);
        let (m, _) = apply_to_member(member("x"), &l);
        assert_eq!(m.default_value.as_deref(), Some("42"));
    }

    #[test]
    fn default_literal_marks_default_branch() {
        let l = lowered_with(vec![BuiltinAnnotation::DefaultLiteral]);
        let (m, _) = apply_to_member(member("x"), &l);
        assert!(m.is_default_label);
    }

    #[test]
    fn position_sets_id_when_no_explicit_id() {
        let l = lowered_with(vec![BuiltinAnnotation::Position(7)]);
        let (m, _) = apply_to_member(member("x"), &l);
        assert_eq!(m.id, 7);
    }

    #[test]
    fn explicit_id_wins_over_position() {
        let l = lowered_with(vec![
            BuiltinAnnotation::Position(7),
            BuiltinAnnotation::Id(99),
        ]);
        let (m, _) = apply_to_member(member("x"), &l);
        assert_eq!(m.id, 99);
    }

    #[test]
    fn type_extensibility_short_forms() {
        for (ann, expected) in [
            (BuiltinAnnotation::Final, DynExtKind::Final),
            (BuiltinAnnotation::Appendable, DynExtKind::Appendable),
            (BuiltinAnnotation::Mutable, DynExtKind::Mutable),
        ] {
            let (t, _) = apply_to_type(TypeDescriptor::structure("S"), &lowered_with(vec![ann]));
            assert_eq!(t.extensibility_kind, expected);
        }
    }

    #[test]
    fn type_extensibility_long_form() {
        let (t, _) = apply_to_type(
            TypeDescriptor::structure("S"),
            &lowered_with(vec![BuiltinAnnotation::Extensibility(IdlExtKind::Mutable)]),
        );
        assert_eq!(t.extensibility_kind, DynExtKind::Mutable);
    }

    #[test]
    fn type_nested_marks_descriptor() {
        let (t, _) = apply_to_type(
            TypeDescriptor::structure("S"),
            &lowered_with(vec![BuiltinAnnotation::Nested]),
        );
        assert!(t.is_nested);
    }

    #[test]
    fn member_passthrough_carries_unhandled_annotations() {
        let l = lowered_with(vec![
            BuiltinAnnotation::Unit("m/s".into()),
            BuiltinAnnotation::HashId(None),
            BuiltinAnnotation::Verbatim(crate::semantics::annotations::VerbatimSpec {
                language: "*".into(),
                placement: crate::semantics::annotations::PlacementKind::AfterDeclaration,
                text: "// raw".into(),
            }),
        ]);
        let (_m, report) = apply_to_member(member("x"), &l);
        assert_eq!(
            report.passthrough,
            vec![
                String::from("unit"),
                String::from("hashid"),
                String::from("verbatim")
            ]
        );
    }

    #[test]
    fn member_apply_propagates_autoid() {
        let l = lowered_with(vec![BuiltinAnnotation::Autoid(AutoidKind::Hash)]);
        let (_m, report) = apply_to_member(member("x"), &l);
        assert_eq!(report.autoid, Some(AutoidKind::Hash));
    }

    #[test]
    fn type_apply_propagates_autoid() {
        let l = lowered_with(vec![BuiltinAnnotation::Autoid(AutoidKind::Sequential)]);
        let (_t, report) = apply_to_type(TypeDescriptor::structure("S"), &l);
        assert_eq!(report.autoid, Some(AutoidKind::Sequential));
    }

    #[test]
    fn member_apply_ignores_type_level_annotations() {
        // `@nested` on a member has no effect on the
        // member itself.
        let l = lowered_with(vec![BuiltinAnnotation::Nested]);
        let (m, report) = apply_to_member(member("x"), &l);
        assert!(!m.is_key);
        assert!(report.passthrough.is_empty());
    }

    #[test]
    fn type_apply_ignores_member_level_annotations() {
        let l = lowered_with(vec![BuiltinAnnotation::Key, BuiltinAnnotation::Optional]);
        let (t, report) = apply_to_type(TypeDescriptor::structure("S"), &l);
        assert!(!t.is_nested);
        assert!(report.passthrough.is_empty());
    }

    #[test]
    fn full_realistic_member_apply() {
        // @key @id(7) @optional @default("100") @must_understand
        let l = lowered_with(vec![
            BuiltinAnnotation::Key,
            BuiltinAnnotation::Id(7),
            BuiltinAnnotation::Optional,
            BuiltinAnnotation::Default("100".into()),
            BuiltinAnnotation::MustUnderstand,
        ]);
        let (m, _) = apply_to_member(member("count"), &l);
        assert!(m.is_key);
        assert_eq!(m.id, 7);
        assert!(m.is_optional);
        assert_eq!(m.default_value.as_deref(), Some("100"));
        assert!(m.is_must_understand);
    }
}
