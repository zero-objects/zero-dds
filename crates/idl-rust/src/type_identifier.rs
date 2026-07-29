// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! F-TYPES-3 codegen: computes a spec-conformant `TypeIdentifier`
//! (XTypes 1.3 §7.3.4.2) for an IDL struct definition and emits it as a
//! const expression in the DdsType impl.
//!
//! Strategy ("Path B" — codegen-time, per-struct):
//! - Primitive member types → `TypeIdentifier::Primitive(PrimitiveKind::X)`.
//! - Strings → `TypeIdentifier::String8Small/Large`.
//! - typedef / enum / sequence / map / nested-struct / union members →
//!   resolved via [`register_spec`]'s full-`Specification` [`NameMap`]
//!   (built once per codegen run by "Path A",
//!   `zerodds_idl::semantics::build_type_registry`) using
//!   [`zerodds_idl::semantics::map_type_spec_resolved`] — the SAME
//!   resolution the AST → `TypeObject` mapper uses, so Path B no longer
//!   reimplements a weaker subset (#24).
//! - Array declarators (`T name[N][M]`) wrap the resolved element
//!   `TypeIdentifier` in a `PlainArraySmall`/`PlainArrayLarge` via
//!   [`zerodds_idl::semantics::make_array_ti`] instead of discarding the
//!   dimensions.
//! - A member type that still cannot be resolved (e.g. it references a
//!   named type that itself contains an as-of-yet-unsupported `TypeSpec`
//!   such as `fixed<>`/`any`) nulls the *whole* struct's `TYPE_IDENTIFIER`
//!   back to `TypeIdentifier::None` — the pre-#24-fix fallback, not a
//!   regression.
//! - Struct flags (extensibility, `@nested`, `@autoid(HASH)`) and member
//!   flags (`@key`, `@optional`, `@must_understand`, `@external`, the
//!   `TRY_CONSTRUCT1` default) plus member-id assignment (incl. the
//!   `@autoid(HASH)` NameHash branch) are produced by actually calling
//!   [`zerodds_types::builder::TypeObjectBuilder`]/`StructBuilder` — the
//!   same builder "Path A" uses to lower a struct — in
//!   [`build_complete_struct_type_bytes`], not by a second hand-rolled
//!   flag-setting copy of it. (A previous version of this module hand-rolled
//!   the flags directly and silently dropped several of them; a deep review
//!   caught it before merge — see the function doc for detail.)
//!
//! The TypeIdentifier of the STRUCT itself is computed as a
//! `TypeIdentifier::EquivalenceHash` with the MD5 hash of the
//! CDR-encoded `CompleteStructType` — codegen-time deterministic.

use core::cell::RefCell;

use zerodds_idl::ast::types::{Declarator, Specification, StructDef, TypeSpec};
use zerodds_idl::semantics::NameMap;

thread_local! {
    /// Full-spec named-type index (struct/enum/union/typedef FQN →
    /// `EquivalenceHashMinimal` `TypeIdentifier`), built once per codegen
    /// run by [`register_spec`]. Consulted by
    /// [`type_spec_to_type_identifier`] to resolve typedef/enum/sequence/
    /// map/nested-struct member types that "Path A"
    /// (`zerodds_idl::semantics::build_type_registry`) already knows how
    /// to hash. Empty by default — callers that never invoke
    /// [`register_spec`] (e.g. the unit tests in this module, which
    /// construct a lone `StructDef` outside any `Specification`) keep the
    /// pre-#24-fix primitive/string-only behavior, not a panic.
    static NAME_MAP: RefCell<NameMap> = RefCell::new(NameMap::default());

    /// Module-path scope (raw IDL module names, outermost first) of the
    /// struct currently being emitted — mirrors the `path` stack
    /// `emitter.rs` walks. Needed for innermost-scope-first resolution of
    /// relative scoped member types (IDL name-lookup rule), exactly as
    /// `zerodds_idl::semantics::build_type_registry` resolves them for
    /// Path A. Set by [`set_current_scope`].
    static CURRENT_SCOPE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Builds the full-spec name index once per codegen run and stores it for
/// [`struct_type_identifier_expr`] to consult (#24 fix). Called from
/// `emitter::generate_rust_module`/`append_data_types` before any
/// definition is emitted.
///
/// A registry-build failure (e.g. a cyclic type, or a not-yet-mappable
/// `TypeSpec` such as `fixed<>`/`any` somewhere — even in a type unrelated
/// to the struct being emitted, since the registry is built for the whole
/// `Specification` at once) is NOT fatal for codegen: it only degrades the
/// affected struct(s)' `TYPE_IDENTIFIER` back to `None`, matching the
/// pre-fix behavior — it never blocks Rust source generation.
pub fn register_spec(spec: &Specification) {
    let names = zerodds_idl::semantics::build_type_registry(spec)
        .map(|lowered| lowered.names)
        .unwrap_or_default();
    NAME_MAP.with(|n| *n.borrow_mut() = names);
}

/// Sets the module-path scope of the struct about to be emitted (see
/// [`CURRENT_SCOPE`]). Called from `emitter.rs` immediately before
/// dispatching to `struct_emit`.
pub fn set_current_scope(scope: &[String]) {
    CURRENT_SCOPE.with(|s| *s.borrow_mut() = scope.to_vec());
}

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
/// range — either still genuinely unmappable (`fixed<>`/`any`) or a
/// scoped reference not present in the [`NAME_MAP`] registered via
/// [`register_spec`] (e.g. `struct_type_identifier_expr` called directly
/// in a unit test, without a full `Specification`).
///
/// The `CompleteStructType` itself is assembled via
/// `zerodds_types::builder::TypeObjectBuilder` — the SAME builder "Path A"
/// (`zerodds_idl::semantics::to_typeobject::lower_struct_to_minimal`) uses
/// — so struct flags (extensibility / `@nested` / `@autoid(HASH)`), member
/// flags (`@key` / `@optional` / `@must_understand` / `@external` +
/// the `TRY_CONSTRUCT1` default) and member-id assignment
/// (`StructBuilder::resolve_member_ids`, incl. the `@autoid(HASH)`
/// NameHash branch) are the builder's real logic, not a second hand-rolled
/// copy that can drift from it (#24 deep-review blocker: the previous
/// version here hand-rolled `CompleteStructMember`/`StructTypeFlag`
/// directly and silently dropped `IS_OPTIONAL`, `IS_MUST_UNDERSTAND`,
/// `IS_EXTERNAL`, `IS_NESTED`, `IS_AUTOID_HASH` and the `@autoid(HASH)`
/// member-id branch — emitting a wrong non-`None` hash for any annotated
/// struct instead of the spec-correct one).
fn build_complete_struct_type_bytes(s: &StructDef) -> Option<Vec<u8>> {
    use crate::annotations::StructExtensibility;
    use zerodds_types::builder::{Extensibility, TypeObjectBuilder};

    let scope = CURRENT_SCOPE.with(|c| c.borrow().clone());

    let extensibility = match crate::annotations::struct_extensibility(&s.annotations) {
        StructExtensibility::Final => Extensibility::Final,
        StructExtensibility::Appendable => Extensibility::Appendable,
        StructExtensibility::Mutable => Extensibility::Mutable,
    };
    let mut builder =
        TypeObjectBuilder::struct_type(s.name.text.clone()).extensibility(extensibility);
    if crate::annotations::struct_is_nested(&s.annotations) {
        builder = builder.nested();
    }
    if crate::annotations::struct_autoid_hash(&s.annotations) {
        builder = builder.autoid_hash();
    }

    for member in &s.members {
        let is_key = crate::annotations::member_is_key(&member.annotations);
        let is_optional = crate::annotations::member_is_optional(&member.annotations);
        let is_must_understand = crate::annotations::member_must_understand(&member.annotations);
        let is_external = crate::annotations::member_is_external(&member.annotations);
        let explicit_id = crate::annotations::member_id(&member.annotations);
        let unit = crate::annotations::member_unit(&member.annotations);
        let base_type_id = type_spec_to_type_identifier(&member.type_spec, &scope)?;

        for decl in &member.declarators {
            let name = match decl {
                Declarator::Simple(n) => n.text.clone(),
                Declarator::Array(a) => a.name.text.clone(),
            };
            // Array declarator (`T name[N][M]`) → wrap the resolved
            // element TypeIdentifier in PlainArraySmall/Large with the
            // declared dimensions (previously discarded — #24 array-bounds
            // bug: a `long grid[4][4]` member emitted a bare `Primitive`
            // TypeIdentifier, spec-wrong for a TK_ARRAY member).
            let member_type_id = match decl {
                Declarator::Simple(_) => base_type_id.clone(),
                Declarator::Array(a) => {
                    zerodds_idl::semantics::make_array_ti(base_type_id.clone(), &a.sizes).ok()?
                }
            };
            let unit = unit.clone();
            builder = builder.member(name, member_type_id, move |mut mb| {
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
                if let Some(u) = unit {
                    mb = mb.unit(u);
                }
                mb
            });
        }
    }

    let cs = builder.build_complete();

    // Wrap in TypeObject::Complete for the public encode API.
    let to = zerodds_types::type_object::TypeObject::Complete(
        zerodds_types::type_object::CompleteTypeObject::Struct(cs),
    );
    to.to_bytes_le().ok()
}

/// Map an IDL `TypeSpec` (in the module `scope` of the struct being
/// emitted) → `zerodds_types::TypeIdentifier`, resolving typedef/enum/
/// sequence/map/nested-struct/union members via the full-spec [`NAME_MAP`]
/// registered by [`register_spec`] — the same resolution
/// `zerodds_idl::semantics::build_type_registry` ("Path A") already
/// performs. `None` only for a still-genuinely-unmappable `TypeSpec`
/// (`fixed<>`/`any`) or an unresolved scoped reference (registry not
/// populated, or the referenced type itself failed to lower).
fn type_spec_to_type_identifier(
    ts: &TypeSpec,
    scope: &[String],
) -> Option<zerodds_types::TypeIdentifier> {
    NAME_MAP.with(|names| {
        zerodds_idl::semantics::map_type_spec_resolved(ts, scope, &names.borrow()).ok()
    })
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

    /// #24 deep-review blocker regression test: the `CompleteStructType`
    /// this module hashes for `@optional`/`@must_understand`/`@external`/
    /// `@nested`/`@autoid(HASH)`-annotated structs must be BYTE-IDENTICAL
    /// to the one `zerodds_types::builder::StructBuilder::build_complete()`
    /// ("Path A") produces for the equivalent hand-built spec — proving the
    /// struct/member flags and the `@autoid(HASH)` member-id branch are
    /// actually set now, not silently dropped as before the fix.
    #[test]
    fn complete_type_bytes_are_byte_identical_to_path_a_struct_builder() {
        use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
        use zerodds_types::type_object::TypeObject;
        use zerodds_types::{PrimitiveKind, TypeIdentifier};

        let src = r#"
            @nested
            @autoid(HASH)
            struct S {
                @optional long a;
                @must_understand double b;
                @external long c;
                long d;
            };
        "#;
        let s = first_struct(src);
        let actual_bytes = build_complete_struct_type_bytes(&s).expect("complete bytes");

        let expected_cs = TypeObjectBuilder::struct_type("S")
            .extensibility(Extensibility::Appendable) // XTypes 1.3 §7.3.3.1 default
            .nested()
            .autoid_hash()
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.optional()
            })
            .member(
                "b",
                TypeIdentifier::Primitive(PrimitiveKind::Float64),
                |m| m.must_understand(),
            )
            .member("c", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.external()
            })
            .member("d", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_complete();
        let expected_bytes = TypeObject::Complete(CompleteTypeObject::Struct(expected_cs))
            .to_bytes_le()
            .expect("encode expected");

        assert_eq!(
            actual_bytes, expected_bytes,
            "Path B (this module) and Path A (StructBuilder) must hash byte-identically"
        );
    }
}
