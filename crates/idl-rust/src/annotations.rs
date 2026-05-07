// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Annotation-Verarbeitung: `@key`, `@id`, `@extensibility`, `@nested`,
//! `@must_understand`, `@optional`, `@default`.

use zerodds_idl::ast::types::{Annotation, AnnotationParams, ConstExpr, LiteralKind};

/// Extensibility-Modus eines Structs (XTypes 1.3 §7.4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructExtensibility {
    /// Default in XTypes 1.3 ist `appendable`. ZeroDDS-Codegen nimmt
    /// `final` als Default — kompakter Wire, kein Header. Wer
    /// XTypes-1.3-spec-konformen Default will, annotiert `@appendable`
    /// oder setzt `@extensibility(APPENDABLE)`.
    Final,
    /// Appendable: Wire mit DHEADER + members in deklarations-Reihenfolge.
    Appendable,
    /// Mutable: Wire mit DHEADER + member-id-tagged members.
    Mutable,
}

fn annotation_name(a: &Annotation) -> &str {
    a.name.parts.last().map(|p| p.text.as_str()).unwrap_or("")
}

/// Liest `@final` / `@appendable` / `@mutable` / `@extensibility(...)`.
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
    StructExtensibility::Final
}

/// Liest `@id(N)` von einer Member-Annotation-Liste. Falls nicht
/// gesetzt: Caller nutzt eine Auto-ID (z.B. positional-index).
#[must_use]
pub fn member_id(annotations: &[Annotation]) -> Option<u32> {
    annotations
        .iter()
        .find(|a| annotation_name(a) == "id")
        .and_then(annotation_first_param_integer)
        .and_then(|v| u32::try_from(v).ok())
}

/// Liest `@must_understand` (default false).
#[must_use]
pub fn member_must_understand(annotations: &[Annotation]) -> bool {
    annotations
        .iter()
        .any(|a| annotation_name(a) == "must_understand")
}

/// Liest `@key` (default false).
#[must_use]
pub fn member_is_key(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|a| annotation_name(a) == "key")
}

/// Liest `@optional` (default false).
#[must_use]
pub fn member_is_optional(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|a| annotation_name(a) == "optional")
}

/// Liest `@nested` (default false). Ein nested-annotated struct wird
/// nicht als DDS-Topic-Type registriert (XTypes §7.4.6.3.5).
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
