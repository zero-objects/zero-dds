// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Annotation processing: `@key`, `@id`, `@extensibility`, `@nested`,
//! `@must_understand`, `@optional`, `@default`.

use zerodds_idl::ast::types::{Annotation, AnnotationParams, ConstExpr, LiteralKind};

/// Extensibility mode of a struct (XTypes 1.3 §7.4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructExtensibility {
    /// Final: wire without DHEADER, members in declaration order — the most
    /// compact form. NOTE: this is NOT the ZeroDDS default; per XTypes 1.3
    /// §7.3.3.1 an unannotated aggregate defaults to APPENDABLE (see
    /// `struct_extensibility` below, since the SX2 flip 2a571721). Use
    /// `@final` explicitly for the header-less form / Cyclone byte-parity.
    Final,
    /// Appendable: wire with DHEADER + members in declaration order.
    Appendable,
    /// Mutable: wire with DHEADER + member-id-tagged members.
    Mutable,
}

fn annotation_name(a: &Annotation) -> &str {
    a.name.parts.last().map(|p| p.text.as_str()).unwrap_or("")
}

/// Reads `@final` / `@appendable` / `@mutable` / `@extensibility(...)`.
#[must_use]
pub fn struct_extensibility(annotations: &[Annotation]) -> StructExtensibility {
    for a in annotations {
        match annotation_name(a) {
            "final" => return StructExtensibility::Final,
            "appendable" => return StructExtensibility::Appendable,
            "mutable" => return StructExtensibility::Mutable,
            "extensibility" => {
                if let Some(value) = annotation_first_param_text(a) {
                    return match value.as_str() {
                        "FINAL" | "Final" | "final" => StructExtensibility::Final,
                        "APPENDABLE" | "Appendable" | "appendable" => {
                            StructExtensibility::Appendable
                        }
                        "MUTABLE" | "Mutable" | "mutable" => StructExtensibility::Mutable,
                        _ => StructExtensibility::Final,
                    };
                }
            }
            _ => {}
        }
    }
    // SX2: spec default for an unannotated aggregate is APPENDABLE (§7.3.3.1).
    StructExtensibility::Appendable
}

/// Reads `@id(N)` from a member annotation list. If not set,
/// the caller uses an auto-ID (e.g. positional index).
#[must_use]
pub fn member_id(annotations: &[Annotation]) -> Option<u32> {
    annotations
        .iter()
        .find(|a| annotation_name(a) == "id")
        .and_then(annotation_first_param_integer)
        .and_then(|v| u32::try_from(v).ok())
}

/// Effective `@hashid` hint for a member (XTypes 1.3 §7.3.1.2.1.4): `Some(hint)`
/// for `@hashid("hint")`, `Some(member_name)` for a bare `@hashid`, `None` when
/// the member carries no `@hashid`. The returned string is what
/// `NameHash::member_id_from_name` hashes to derive the member id.
#[must_use]
pub fn member_hashid_hint(annotations: &[Annotation], member_name: &str) -> Option<String> {
    annotations
        .iter()
        .find(|a| annotation_name(a) == "hashid")
        .map(|a| annotation_first_param_text(a).unwrap_or_else(|| member_name.to_string()))
}

/// Resolves the wire member-ID of a member honoring, highest precedence first
/// (XTypes 1.3 §7.3.1.2.1): explicit `@id(n)`, then `@hashid` (per-member hash),
/// then struct-level `@autoid(HASH)` (hash of the member name), else the caller's
/// sequential positional fallback.
///
/// Broad-audit P0-3: this delegates to the ONE central resolver in the semantic
/// layer (`zerodds_idl::semantics::member_id::resolved_member_id`) so Rust, C++
/// and the TypeObject builder share a single derivation and cannot drift — the
/// EMHEADER/PID ids the data wire emits stay byte-identical to the member ids
/// the TypeObject carries (findings A31 `@hashid` + A32 `@autoid(HASH)`).
/// zerodds-lint: recursion-depth 1 (delegates to the semantic-layer resolver of
/// the same name; not self-recursive).
#[must_use]
pub fn resolved_member_id(
    struct_autoid_hash: bool,
    member_annotations: &[Annotation],
    member_name: &str,
    fallback_seq: u32,
) -> u32 {
    zerodds_idl::semantics::member_id::resolved_member_id(
        struct_autoid_hash,
        member_annotations,
        member_name,
        fallback_seq,
    )
}

/// Reads `@default(value)` from a member annotation list (XTypes 1.3
/// §7.3.1.2.1.5). Returns the raw default-value expression, which the caller
/// renders as a Rust literal of the member type. Used on the decode side: a
/// non-optional member that a peer omitted (a shorter `@appendable` frame or a
/// missing `@mutable` EMHEADER) takes this value instead of failing the decode
/// (finding A33).
#[must_use]
pub fn member_default(annotations: &[Annotation]) -> Option<ConstExpr> {
    annotations
        .iter()
        .find(|a| annotation_name(a) == "default")
        .and_then(|a| single_param(&a.params))
        .cloned()
}

/// Reads `@must_understand` (default false).
#[must_use]
pub fn member_must_understand(annotations: &[Annotation]) -> bool {
    annotations
        .iter()
        .any(|a| annotation_name(a) == "must_understand")
}

/// Reads `@key` (default false).
#[must_use]
pub fn member_is_key(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|a| annotation_name(a) == "key")
}

/// Reads `@optional` (default false).
#[must_use]
pub fn member_is_optional(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|a| annotation_name(a) == "optional")
}

/// Reads `@external` (default false) — indirect (pointer/shared) member
/// storage, XTypes 1.3 §7.2.2.4.4.4.6 / `StructMemberFlag::IS_EXTERNAL`.
#[must_use]
pub fn member_is_external(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|a| annotation_name(a) == "external")
}

/// Reads `@unit("...")` — the unit of measure of the member. Lands in
/// the complete TypeObject as `AppliedBuiltinMemberAnnotations.unit`
/// (XTypes 1.3 §7.3.1.2.1.x). The runtime `MemberDescriptor` carries no
/// unit (spec §7.5.2.7), so this is the only wire location.
#[must_use]
pub fn member_unit(annotations: &[Annotation]) -> Option<String> {
    annotations
        .iter()
        .find(|a| annotation_name(a) == "unit")
        .and_then(annotation_first_param_text)
}

/// Reads an explicit `@value(N)` on an enumerator (XTypes 1.3 §7.3.1.2.1.6).
/// Returns `None` when the enumerator has no `@value`, in which case the
/// caller assigns the sequential default (previous value + 1, starting 0).
#[must_use]
pub fn enumerator_value(annotations: &[Annotation]) -> Option<i128> {
    annotations
        .iter()
        .find(|a| annotation_name(a) == "value")
        .and_then(|a| single_param(&a.params))
        .and_then(crate::type_map::const_expr_as_i128)
}

/// Effective `@bit_bound` of a bitmask — the wire holder width. XTypes 1.3
/// §7.3.1.2.1.1: the DEFAULT bit_bound for a bitmask is **32** (→ UInt32
/// holder), NOT the count of declared bits. So an unannotated bitmask is a
/// uint32 on the wire even if it declares only 3 flags.
#[must_use]
pub fn bitmask_bit_bound(annotations: &[Annotation]) -> u32 {
    annotations
        .iter()
        .find(|a| annotation_name(a) == "bit_bound")
        .and_then(annotation_first_param_integer)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(32)
}

/// Reads `@nested` (default false). A nested-annotated struct is not
/// registered as a DDS topic type (XTypes §7.4.6.3.5).
#[must_use]
pub fn struct_is_nested(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|a| annotation_name(a) == "nested")
}

/// Reads `@autoid(HASH)` on a struct/union (default false = SEQUENTIAL,
/// XTypes 1.3 §7.2.2.4.9 / `StructTypeFlag::IS_AUTOID_HASH`). When set,
/// members without an explicit `@id(n)` get a member id derived from the
/// first 4 MD5 bytes of the member name instead of the sequential
/// (declaration-order) default.
#[must_use]
pub fn struct_autoid_hash(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|a| {
        annotation_name(a) == "autoid" && annotation_first_param_text(a).as_deref() == Some("HASH")
    })
}

fn annotation_first_param_text(a: &Annotation) -> Option<String> {
    let value = single_param(&a.params)?;
    match value {
        ConstExpr::Literal(lit) if lit.kind == LiteralKind::String => {
            Some(lit.raw.trim_matches('"').to_string())
        }
        ConstExpr::Scoped(scoped) => scoped.parts.last().map(|p| p.text.clone()),
        _ => None,
    }
}

fn annotation_first_param_integer(a: &Annotation) -> Option<u64> {
    let value = single_param(&a.params)?;
    match value {
        ConstExpr::Literal(lit) if lit.kind == LiteralKind::Integer => {
            crate::type_map::const_expr_as_usize(value).map(|v| v as u64)
        }
        _ => None,
    }
}

fn single_param(params: &AnnotationParams) -> Option<&ConstExpr> {
    match params {
        AnnotationParams::Single(expr) => Some(expr),
        AnnotationParams::Named(named) => named.first().map(|np| &np.value),
        AnnotationParams::None | AnnotationParams::Empty => None,
    }
}
