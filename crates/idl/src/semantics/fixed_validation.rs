// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! §7.4.1.4.4.3.4 — Fixed-Praezisions-Constraints.
//!
//! Spec: `fixed<P, S>` mit `P <= 31` (max 31 total digits) und
//! `S <= P` (scale darf total nicht ueberschreiten).

use crate::ast::{
    ConstExpr, ConstrTypeDecl, Definition, FixedPtType, LiteralKind, Specification, TypeDecl,
    TypeSpec, TypedefDecl,
};
use crate::errors::Span;

/// Fixed-Type-Validierungs-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedValidationError {
    /// `fixed<P, S>` mit `P > 31`.
    TotalDigitsExceeded {
        /// Tatsaechlicher Wert P.
        digits: u64,
        /// Quellort.
        span: Span,
    },
    /// `fixed<P, S>` mit `S > P`.
    ScaleExceedsDigits {
        /// Wert P.
        digits: u64,
        /// Wert S.
        scale: u64,
        /// Quellort.
        span: Span,
    },
}

/// Top-Level: alle Fixed-Type-Verwendungen in der Spec validieren.
#[must_use]
pub fn validate_fixed_types(spec: &Specification) -> Vec<FixedValidationError> {
    let mut errs = Vec::new();
    for d in &spec.definitions {
        walk(d, &mut errs);
    }
    errs
}

/// zerodds-lint: recursion-depth 32
fn walk(d: &Definition, errs: &mut Vec<FixedValidationError>) {
    match d {
        Definition::Module(m) => {
            for inner in &m.definitions {
                walk(inner, errs);
            }
        }
        Definition::Type(TypeDecl::Typedef(td)) => {
            check_typedef(td, errs);
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(crate::ast::StructDcl::Def(
            s,
        )))) => {
            for m in &s.members {
                check_type_spec(&m.type_spec, errs);
            }
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(crate::ast::UnionDcl::Def(u)))) => {
            for case in &u.cases {
                check_type_spec(&case.element.type_spec, errs);
            }
        }
        _ => {}
    }
}

fn check_typedef(td: &TypedefDecl, errs: &mut Vec<FixedValidationError>) {
    check_type_spec(&td.type_spec, errs);
}

/// zerodds-lint: recursion-depth 32
fn check_type_spec(ts: &TypeSpec, errs: &mut Vec<FixedValidationError>) {
    match ts {
        TypeSpec::Fixed(f) => check_fixed(f, errs),
        TypeSpec::Sequence(s) => check_type_spec(&s.elem, errs),
        TypeSpec::Map(m) => {
            check_type_spec(&m.key, errs);
            check_type_spec(&m.value, errs);
        }
        _ => {}
    }
}

fn check_fixed(f: &FixedPtType, errs: &mut Vec<FixedValidationError>) {
    let digits = const_to_u64(&f.digits);
    let scale = const_to_u64(&f.scale);
    if let Some(d) = digits {
        if d > 31 {
            errs.push(FixedValidationError::TotalDigitsExceeded {
                digits: d,
                span: f.span,
            });
        }
        if let Some(s) = scale {
            if s > d {
                errs.push(FixedValidationError::ScaleExceedsDigits {
                    digits: d,
                    scale: s,
                    span: f.span,
                });
            }
        }
    }
}

fn const_to_u64(e: &ConstExpr) -> Option<u64> {
    if let ConstExpr::Literal(l) = e {
        if matches!(l.kind, LiteralKind::Integer) {
            return l.raw.parse::<u64>().ok();
        }
    }
    None
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
    fn fixed_within_31_digits_ok() {
        let ast = parse_to_ast("typedef fixed<31, 10> F;");
        let errs = validate_fixed_types(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }

    #[test]
    fn fixed_with_total_over_31_errors() {
        let ast = parse_to_ast("typedef fixed<32, 5> F;");
        let errs = validate_fixed_types(&ast);
        assert!(
            errs.iter().any(|e| matches!(
                e,
                FixedValidationError::TotalDigitsExceeded { digits: 32, .. }
            )),
            "got {errs:?}"
        );
    }

    #[test]
    fn fixed_with_scale_greater_than_total_errors() {
        let ast = parse_to_ast("typedef fixed<5, 7> F;");
        let errs = validate_fixed_types(&ast);
        assert!(
            errs.iter().any(|e| matches!(
                e,
                FixedValidationError::ScaleExceedsDigits {
                    digits: 5,
                    scale: 7,
                    ..
                }
            )),
            "got {errs:?}"
        );
    }

    #[test]
    fn fixed_in_struct_member_validates() {
        let ast = parse_to_ast("struct S { fixed<35, 2> price; };");
        let errs = validate_fixed_types(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, FixedValidationError::TotalDigitsExceeded { .. })),
            "got {errs:?}"
        );
    }
}
