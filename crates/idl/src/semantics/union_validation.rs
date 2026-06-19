// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Union validation (C4.6 §1.8 / spec §7.4.13.5).
//!
//! - The discriminator must be a primitive integer / char / boolean / octet /
//!   (resolved) enum. Float/double/strings are forbidden.
//! - At most one default branch.
//! - Case labels must be unique and match the discriminator type.
//! - Each member must have at least one `case` label (or default).
//!
//! The validation runs as a post-pass on a Specification — it
//! returns a list of [`UnionValidationError`].

use crate::ast::{
    CaseLabel, ConstExpr, ConstrTypeDecl, Definition, LiteralKind, Specification, SwitchTypeSpec,
    TypeDecl, UnionDcl, UnionDef,
};
use crate::errors::Span;

/// Validation error of a union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnionValidationError {
    /// The discriminator type is not permitted.
    InvalidDiscriminator {
        /// Description (e.g. `"float"`).
        kind: String,
        /// Source location.
        span: Span,
    },
    /// More than one `default` branch.
    DuplicateDefault {
        /// Source location of the second default.
        span: Span,
    },
    /// Case label duplicated (same value multiple times).
    DuplicateCaseLabel {
        /// Raw representation of the label.
        label: String,
        /// Source location.
        span: Span,
    },
    /// The case-label type does not match the discriminator.
    LabelTypeMismatch {
        /// Discriminator description.
        discriminator: String,
        /// Raw label.
        label: String,
        /// Source location.
        span: Span,
    },
    /// Member without any `case` label or `default`.
    MissingCaseLabel {
        /// Source location of the member.
        span: Span,
    },
    /// §7.4.1.4.4.4.2 — element declarators in a union must be
    /// unique.
    DuplicateElementDeclarator {
        /// Repeated member name.
        name: String,
        /// Source location of the repeated member.
        span: Span,
    },
    /// §7.4.1.4.4.4.2 — `default` is redundant if the non-default
    /// labels cover the entire discriminator range.
    DefaultLabelRedundant {
        /// Discriminator description.
        discriminator: String,
        /// Source location of the default.
        span: Span,
    },
}

/// Runs the validation for all unions in `spec`.
#[must_use]
pub fn validate_unions(spec: &Specification) -> Vec<UnionValidationError> {
    let mut errs = Vec::new();
    for d in &spec.definitions {
        walk_def(d, &mut errs);
    }
    errs
}

/// zerodds-lint: recursion-depth 32
fn walk_def(d: &Definition, errs: &mut Vec<UnionValidationError>) {
    match d {
        Definition::Module(m) => {
            for d in &m.definitions {
                walk_def(d, errs);
            }
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
            validate_union(u, errs);
        }
        _ => {}
    }
}

/// Public helper: validate a concrete union.
pub fn validate_union(u: &UnionDef, errs: &mut Vec<UnionValidationError>) {
    // 1. Discriminator type.
    let disc_kind = check_discriminator(&u.switch_type, errs);

    // 2. Default + case labels + type compat.
    let mut default_seen = false;
    let mut default_span: Option<Span> = None;
    let mut seen_labels: Vec<String> = Vec::new();
    let mut seen_declarators: Vec<String> = Vec::new();
    let mut bool_value_labels: Vec<bool> = Vec::new();

    for case in &u.cases {
        // §7.4.1.4.4.4.2: an element declarator must be unique within the
        // union.
        let decl_name = case.element.declarator.name().text.clone();
        if seen_declarators.iter().any(|n| n == &decl_name) {
            errs.push(UnionValidationError::DuplicateElementDeclarator {
                name: decl_name.clone(),
                span: case.element.span,
            });
        } else {
            seen_declarators.push(decl_name);
        }

        if case.labels.is_empty() {
            errs.push(UnionValidationError::MissingCaseLabel { span: case.span });
            continue;
        }
        for label in &case.labels {
            match label {
                CaseLabel::Default => {
                    if default_seen {
                        errs.push(UnionValidationError::DuplicateDefault { span: case.span });
                    }
                    default_seen = true;
                    default_span = Some(case.span);
                }
                CaseLabel::Value(expr) => {
                    let raw = const_expr_str(expr);
                    if seen_labels.iter().any(|l| l == &raw) {
                        errs.push(UnionValidationError::DuplicateCaseLabel {
                            label: raw.clone(),
                            span: case.span,
                        });
                    } else {
                        seen_labels.push(raw.clone());
                    }
                    if let Some(ref disc) = disc_kind {
                        if !label_matches_disc(expr, disc) {
                            errs.push(UnionValidationError::LabelTypeMismatch {
                                discriminator: disc.clone(),
                                label: raw.clone(),
                                span: case.span,
                            });
                        }
                        // Track bool coverage for default redundancy.
                        if disc == "boolean" {
                            if let ConstExpr::Literal(l) = expr {
                                if matches!(l.kind, LiteralKind::Boolean) {
                                    let v = l.raw == "TRUE" || l.raw == "true";
                                    if !bool_value_labels.contains(&v) {
                                        bool_value_labels.push(v);
                                    }
                                }
                            } else if let ConstExpr::Scoped(s) = expr {
                                if let Some(p) = s.parts.last() {
                                    let v = matches!(p.text.as_str(), "TRUE" | "true");
                                    if !bool_value_labels.contains(&v) {
                                        bool_value_labels.push(v);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // §7.4.1.4.4.4.2: default is redundant if the non-default labels cover
    // the entire discriminator range. Implemented for a
    // boolean discriminator (full coverage = both TRUE and FALSE
    // listed).
    if default_seen {
        if let Some(ref disc) = disc_kind {
            if disc == "boolean" && bool_value_labels.len() == 2 {
                errs.push(UnionValidationError::DefaultLabelRedundant {
                    discriminator: disc.clone(),
                    span: default_span.unwrap_or(u.span),
                });
            }
        }
    }
}

fn check_discriminator(s: &SwitchTypeSpec, errs: &mut Vec<UnionValidationError>) -> Option<String> {
    match s {
        SwitchTypeSpec::Integer(_) => Some("integer".to_string()),
        SwitchTypeSpec::Char => Some("char".to_string()),
        SwitchTypeSpec::Boolean => Some("boolean".to_string()),
        SwitchTypeSpec::Octet => Some("octet".to_string()),
        SwitchTypeSpec::Scoped(_) => Some("enum".to_string()), // best-effort
        // Floats/strings cannot currently appear as a SwitchTypeSpec
        // (the grammar rejects them); but a pseudo-path remains for
        // vendor-pragmatic modes:
        #[allow(unreachable_patterns)]
        other => {
            errs.push(UnionValidationError::InvalidDiscriminator {
                kind: format!("{other:?}"),
                span: Span::SYNTHETIC,
            });
            None
        }
    }
}

fn const_expr_str(e: &ConstExpr) -> String {
    match e {
        ConstExpr::Literal(l) => l.raw.clone(),
        ConstExpr::Scoped(s) => s
            .parts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("::"),
        ConstExpr::Unary { op, operand, .. } => format!("({op:?} {})", const_expr_str(operand)),
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            format!("({} {op:?} {})", const_expr_str(lhs), const_expr_str(rhs))
        }
    }
}

fn label_matches_disc(expr: &ConstExpr, disc: &str) -> bool {
    match (expr, disc) {
        // Integer discriminator accepts integer + scoped (enum/const).
        (ConstExpr::Literal(l), "integer" | "octet") => matches!(l.kind, LiteralKind::Integer),
        (ConstExpr::Literal(l), "char") => matches!(l.kind, LiteralKind::Char),
        (ConstExpr::Literal(l), "boolean") => matches!(l.kind, LiteralKind::Boolean),
        (ConstExpr::Scoped(_), _) => true, // Const/Enum-Reference
        // Unary/Binary: accept cautiously if the disc is integer-like.
        (ConstExpr::Unary { .. } | ConstExpr::Binary { .. }, "integer" | "octet") => true,
        _ => false,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn parse_to_ast(src: &str) -> Specification {
        parse(src, &ParserConfig::default()).expect("parse ok")
    }

    #[test]
    fn long_discriminator_with_int_labels_ok() {
        let ast = parse_to_ast(
            "union U switch (long) { case 1: long a; case 2: long b; default: long c; };",
        );
        let errs = validate_unions(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }

    #[test]
    fn duplicate_default_branch_errors() {
        let ast = parse_to_ast("union U switch (long) { default: long a; default: long b; };");
        let errs = validate_unions(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, UnionValidationError::DuplicateDefault { .. }))
        );
    }

    #[test]
    fn duplicate_case_label_errors() {
        let ast = parse_to_ast("union U switch (long) { case 1: long a; case 1: long b; };");
        let errs = validate_unions(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, UnionValidationError::DuplicateCaseLabel { .. }))
        );
    }

    #[test]
    fn boolean_discriminator_with_int_label_is_mismatch() {
        let ast = parse_to_ast("union U switch (boolean) { case 1: long a; default: long b; };");
        let errs = validate_unions(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, UnionValidationError::LabelTypeMismatch { .. }))
        );
    }

    #[test]
    fn boolean_discriminator_with_bool_labels_ok() {
        let ast =
            parse_to_ast("union U switch (boolean) { case TRUE: long a; case FALSE: long b; };");
        let errs = validate_unions(&ast);
        // FALSE/TRUE are recognized as scoped → waved through.
        assert!(errs.is_empty(), "got {errs:?}");
    }

    #[test]
    fn char_discriminator_with_char_labels_ok() {
        let ast = parse_to_ast("union U switch (char) { case 'a': long x; case 'b': long y; };");
        let errs = validate_unions(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }

    // §7.4.1.4.4.4.2 — Element-Declarator-Uniqueness

    #[test]
    fn union_with_duplicate_element_declarator_errors() {
        // Spec §7.4.1.4.4.4.2: two cases with the same member name
        // are illegal.
        let ast = parse_to_ast(
            "union U switch (long) { case 1: long a; case 2: long a; default: long b; };",
        );
        let errs = validate_unions(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, UnionValidationError::DuplicateElementDeclarator { .. })),
            "got {errs:?}"
        );
    }

    // §7.4.1.4.4.4.2 — default coverage

    #[test]
    fn union_default_redundant_for_full_boolean_coverage_errors() {
        // Spec §7.4.1.4.4.4.2: default is redundant when TRUE and FALSE
        // are both listed.
        let ast = parse_to_ast(
            "union U switch (boolean) { case TRUE: long a; case FALSE: long b; default: long c; };",
        );
        let errs = validate_unions(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, UnionValidationError::DefaultLabelRedundant { .. })),
            "got {errs:?}"
        );
    }

    #[test]
    fn union_default_required_for_partial_int_coverage_ok() {
        // Spec: with an integer discriminator a list never covers the
        // entire range — default is always allowed, not
        // redundant.
        let ast = parse_to_ast(
            "union U switch (long) { case 1: long a; case 2: long b; default: long c; };",
        );
        let errs = validate_unions(&ast);
        assert!(
            !errs
                .iter()
                .any(|e| matches!(e, UnionValidationError::DefaultLabelRedundant { .. })),
            "got {errs:?}"
        );
    }

    #[test]
    fn union_default_coverage_required_when_partial_range() {
        // §7.2.4.4.4.4 — a partial-range union WITHOUT a default is spec-compliant:
        // implicit_default_member is used. The validator MUST NOT
        // enforce that a default exists (that would be too strict).
        // The test ensures that no UnionValidationError arises
        // for a partial range without a default.
        let ast = parse_to_ast(
            "union U switch (octet) { case 1: long a; case 2: long b; case 3: long c; };",
        );
        let errs = validate_unions(&ast);
        assert!(
            errs.is_empty(),
            "partial-range no-default disallowed? errs={errs:?}"
        );
    }
}
