// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Annotation processing: `@key`, `@id`, `@extensibility`, `@nested`,
//! `@must_understand`, `@optional`, `@default`.

use zerodds_idl::ast::types::{Annotation, AnnotationParams, ConstExpr, LiteralKind};

/// Extensibility mode of a struct (XTypes 1.3 §7.4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructExtensibility {
    /// Default in XTypes 1.3 is `appendable`. ZeroDDS codegen uses
    /// `final` as default — more compact wire format, no header. To get
    /// the XTypes-1.3-spec-conformant default, annotate `@appendable`
    /// or set `@extensibility(APPENDABLE)`.
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

/// Effective `@bit_bound` of an enum — selects the signed-integer wire width
/// (XTypes 1.3 §7.3.1.2.1.2 + §7.4.5.1): the DEFAULT enum bit_bound is **32**.
/// `@bit_bound(N)` (1..=32) narrows the holder: N≤8 → 1 octet, N≤16 → 2 octets,
/// else 4 octets. Cyclone honours this; matching it is spec-faithful.
#[must_use]
pub fn enum_bit_bound(annotations: &[Annotation]) -> u32 {
    annotations
        .iter()
        .find(|a| annotation_name(a) == "bit_bound")
        .and_then(annotation_first_param_integer)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|&v| (1..=32).contains(&v))
        .unwrap_or(32)
}

/// Wire width in octets (1/2/4) for an enum `@bit_bound`.
#[must_use]
pub fn enum_wire_octets(bit_bound: u32) -> u8 {
    if bit_bound <= 8 {
        1
    } else if bit_bound <= 16 {
        2
    } else {
        4
    }
}

/// Reads `@nested` (default false). A nested-annotated struct is not
/// registered as a DDS topic type (XTypes §7.4.6.3.5).
#[must_use]
pub fn struct_is_nested(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|a| annotation_name(a) == "nested")
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
