// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds_no_dyn_in_safe` — `dyn Trait` in [`SafetyClass::Safe`]-Crates nur
//! mit explizitem `#[allow(dds_no_dyn_in_safe)]`-Marker zulaessig.
//!
//! Spec: `docs/architecture/04_safety_by_architecture.md §3.4`.
//!
//! Pragmatischer File-Level-Allow: enthaelt der Quelltext den Marker
//! `zerodds-lint: allow no_dyn_in_safe` (in einem Kommentar irgendwo), wird
//! die Datei vom Lint ausgenommen. Damit kann z.B. `examples/spdp_demo.rs`
//! `Box<dyn std::error::Error>` weiter verwenden.
//!
//! [`SafetyClass::Safe`]: crate::classification::SafetyClass::Safe

use syn::TypeTraitObject;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

use crate::classification::SafetyClass;
use crate::diagnostic::Diagnostic;

use super::{FileLint, FileLintContext};

/// Lint-Implementierung.
pub struct NoDynInSafe;

const NAME: &str = "dds_no_dyn_in_safe";
const ALLOW_MARKER: &str = "zerodds-lint: allow no_dyn_in_safe";

impl FileLint for NoDynInSafe {
    fn name(&self) -> &'static str {
        NAME
    }

    fn check(&self, ctx: &FileLintContext<'_>) -> Vec<Diagnostic> {
        if ctx.crate_class != Some(SafetyClass::Safe) {
            return Vec::new();
        }
        if ctx.source.contains(ALLOW_MARKER) {
            return Vec::new();
        }
        let mut visitor = Visitor {
            file: ctx.file,
            diagnostics: Vec::new(),
        };
        visitor.visit_file(ctx.ast);
        visitor.diagnostics
    }
}

struct Visitor<'a> {
    file: &'a std::path::Path,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for Visitor<'_> {
    fn visit_type_trait_object(&mut self, node: &'ast TypeTraitObject) {
        let span = node.span();
        let start = span.start();
        let line = start.line;
        let col = start.column.saturating_add(1);
        self.diagnostics.push(Diagnostic::error(
            self.file,
            line,
            col,
            NAME,
            "`dyn Trait` in SAFE-Crate; nutze konkrete Generics oder \
             markiere Datei mit `zerodds-lint: allow no_dyn_in_safe`",
        ));
        visit::visit_type_trait_object(self, node);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(src: &str, class: Option<SafetyClass>) -> Vec<Diagnostic> {
        let ast = syn::parse_file(src).expect("parse");
        let lint = NoDynInSafe;
        let path = PathBuf::from("test.rs");
        let ctx = FileLintContext {
            file: &path,
            source: src,
            ast: &ast,
            crate_class: class,
            crate_name: "test",
        };
        lint.check(&ctx)
    }

    #[test]
    fn dyn_trait_in_safe_crate_fails() {
        let src = "fn f() -> Box<dyn std::error::Error> { todo!() }\n";
        let d = run(src, Some(SafetyClass::Safe));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].lint, "dds_no_dyn_in_safe");
    }

    #[test]
    fn dyn_trait_in_standard_crate_ok() {
        let src = "fn f() -> Box<dyn std::error::Error> { todo!() }\n";
        assert!(run(src, Some(SafetyClass::Standard)).is_empty());
    }

    #[test]
    fn unclassified_crate_is_skipped() {
        let src = "fn f() -> Box<dyn std::error::Error> { todo!() }\n";
        assert!(run(src, None).is_empty());
    }

    #[test]
    fn file_level_allow_marker_silences_lint() {
        let src = "// zerodds-lint: allow no_dyn_in_safe\nfn f() -> Box<dyn std::error::Error> { todo!() }\n";
        assert!(run(src, Some(SafetyClass::Safe)).is_empty());
    }

    #[test]
    fn no_dyn_no_lint() {
        let src = "fn f() -> u32 { 0 }\n";
        assert!(run(src, Some(SafetyClass::Safe)).is_empty());
    }
}
