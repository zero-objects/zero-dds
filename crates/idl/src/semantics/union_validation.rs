// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Union-Validierung (C4.6 §1.8 / Spec §7.4.13.5).
//!
//! - Discriminator muss primitiver Integer / Char / Boolean / Octet /
//!   (resolved) Enum sein. Float/Double/Strings sind verboten.
//! - Default-Branch maximal einmal.
//! - Case-Labels muessen unique sein und zum Discriminator-Type passen.
//! - Jedes Member muss mindestens ein `case`-Label haben (oder default).
//!
//! Die Validierung laeuft als Post-Pass auf einer Specification — sie
//! liefert eine Liste von [`UnionValidationError`].

use crate::ast::{
    CaseLabel, ConstExpr, ConstrTypeDecl, Definition, LiteralKind, Specification, SwitchTypeSpec,
    TypeDecl, UnionDcl, UnionDef,
};
use crate::errors::Span;

/// Validierungs-Fehler einer Union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnionValidationError {
    /// Discriminator-Typ ist nicht zulaessig.
    InvalidDiscriminator {
        /// Beschreibung (z.B. `"float"`).
        kind: String,
        /// Quellort.
        span: Span,
    },
    /// Mehr als ein `default`-Branch.
    DuplicateDefault {
        /// Quellort des zweiten Defaults.
        span: Span,
    },
    /// Case-Label dupliziert (selber Wert mehrfach).
    DuplicateCaseLabel {
        /// Roh-Repraesentation des Labels.
        label: String,
        /// Quellort.
        span: Span,
    },
    /// Case-Label-Type passt nicht zum Discriminator.
    LabelTypeMismatch {
        /// Discriminator-Beschreibung.
        discriminator: String,
        /// Roh-Label.
        label: String,
        /// Quellort.
        span: Span,
    },
    /// Member ohne irgendein `case`-Label oder `default`.
    MissingCaseLabel {
        /// Quellort des Members.
        span: Span,
    },
    /// §7.4.1.4.4.4.2 — Element-Declarators in einer Union muessen
    /// eindeutig sein.
    DuplicateElementDeclarator {
        /// Wiederholter Member-Name.
        name: String,
        /// Quellort des wiederholten Members.
        span: Span,
    },
    /// §7.4.1.4.4.4.2 — `default` ist redundant, wenn die Non-Default-
    /// Labels den gesamten Discriminator-Range abdecken.
    DefaultLabelRedundant {
        /// Discriminator-Beschreibung.
        discriminator: String,
        /// Quellort des Defaults.
        span: Span,
    },
}

/// Fuehrt die Validierung fuer alle Unions in `spec` durch.
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

/// Public-Helfer: validiere eine konkrete Union.
pub fn validate_union(u: &UnionDef, errs: &mut Vec<UnionValidationError>) {
    // 1. Discriminator-Type.
    let disc_kind = check_discriminator(&u.switch_type, errs);

    // 2. Default + Case-Labels + Type-Compat.
    let mut default_seen = false;
    let mut default_span: Option<Span> = None;
    let mut seen_labels: Vec<String> = Vec::new();
    let mut seen_declarators: Vec<String> = Vec::new();
    let mut bool_value_labels: Vec<bool> = Vec::new();

    for case in &u.cases {
        // §7.4.1.4.4.4.2: Element-Declarator muss in der Union eindeutig
        // sein.
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
                        // Bool-Coverage tracken fuer Default-Redundanz.
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

    // §7.4.1.4.4.4.2: default ist redundant wenn Non-Default-Labels den
    // gesamten Discriminator-Range abdecken. Implementiert fuer
    // Boolean-Discriminator (full coverage = beide TRUE und FALSE
    // gelistet).
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
        // Floats/Strings koennen aktuell als SwitchTypeSpec nicht auftauchen
        // (Grammar weist sie ab); aber ein Pseudo-Path bleibt fuer
        // Vendor-pragmatische Modi:
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
        // Integer-discriminator akzeptiert integer + scoped (enum/const).
        (ConstExpr::Literal(l), "integer" | "octet") => matches!(l.kind, LiteralKind::Integer),
        (ConstExpr::Literal(l), "char") => matches!(l.kind, LiteralKind::Char),
        (ConstExpr::Literal(l), "boolean") => matches!(l.kind, LiteralKind::Boolean),
        (ConstExpr::Scoped(_), _) => true, // Const/Enum-Reference
        // Unary/Binary: vorsichtig akzeptieren, wenn integer-artiger Disc.
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
        // FALSE/TRUE werden als Scoped erkannt → durchgewunken.
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
        // Spec §7.4.1.4.4.4.2: zwei Cases mit gleichem Member-Namen
        // sind illegal.
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

    // §7.4.1.4.4.4.2 — Default-Coverage

    #[test]
    fn union_default_redundant_for_full_boolean_coverage_errors() {
        // Spec §7.4.1.4.4.4.2: default redundant wenn TRUE und FALSE
        // beide gelistet sind.
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
        // Spec: bei Integer-Discriminator deckt eine Liste nie den
        // gesamten Range ab — default ist immer erlaubt, nicht
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
        // §7.2.4.4.4.4 — partial-Range Union OHNE default ist Spec-konform:
        // implicit_default_member wird benutzt. Validator MUSS NICHT
        // erzwingen, dass ein default existiert (das waere zu strikt).
        // Test stellt sicher, dass keine UnionValidationError aufkommt
        // bei partial-Range ohne Default.
        let ast = parse_to_ast(
            "union U switch (octet) { case 1: long a; case 2: long b; case 3: long c; };",
        );
        let errs = validate_unions(&ast);
        assert!(
            errs.is_empty(),
            "partial-range no-default unzulaessig? errs={errs:?}"
        );
    }
}
