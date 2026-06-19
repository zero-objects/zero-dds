// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds_no_panic_in_safe` — block panic-producing operations in
//! [`SafetyClass::Safe`] crates.
//!
//! Captures:
//! - `.unwrap()` and `.expect(...)` method calls
//! - `panic!(...)`, `unreachable!(...)`, `todo!(...)`, `unimplemented!(...)` macros
//!
//! Exceptions:
//! - file path contains `/tests/` or `/examples/`
//! - file-level marker `zerodds-lint: allow no_panic_in_safe`
//! - item has `#[test]`, `#[cfg(test)]` or analogous test-specific cfgs
//! - item lives in a `#[cfg(test)]` module (visitor state)
//!
//! Spec: `docs/architecture/04_safety_by_architecture.md §3.4`.
//!
//! [`SafetyClass::Safe`]: crate::classification::SafetyClass::Safe

use proc_macro2::Span;
use syn::visit::{self, Visit};
use syn::{Attribute, ExprMethodCall, ItemFn, ItemImpl, ItemMod, Macro, Meta};

use crate::classification::SafetyClass;
use crate::diagnostic::Diagnostic;

use super::{FileLint, FileLintContext};

/// Lint implementation.
pub struct NoPanicInSafe;

const NAME: &str = "dds_no_panic_in_safe";
const ALLOW_MARKER: &str = "zerodds-lint: allow no_panic_in_safe";

impl FileLint for NoPanicInSafe {
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
        if is_test_path(ctx.file) {
            return Vec::new();
        }
        // Whole file marked as test via `#![cfg(test)]`? — syn collects it
        // as an inner attribute on the file.
        if has_cfg_test(&ctx.ast.attrs) {
            return Vec::new();
        }
        let mut visitor = Visitor {
            file: ctx.file,
            diagnostics: Vec::new(),
            in_test_depth: 0,
        };
        visitor.visit_file(ctx.ast);
        visitor.diagnostics
    }
}

/// True if the path is under a `tests/` or `examples/` directory.
fn is_test_path(file: &std::path::Path) -> bool {
    file.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("tests") | Some("examples") | Some("benches")
        )
    })
}

/// Searches an attribute list for `#[test]`, `#[cfg(test)]` or
/// `#[cfg(any(..., test, ...))]`/`#[cfg(all(..., test, ...))]`.
fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        if a.path().is_ident("test") {
            return true;
        }
        if a.path().is_ident("cfg") {
            return cfg_references_test(&a.meta);
        }
        false
    })
}

fn cfg_references_test(meta: &Meta) -> bool {
    // We treat every occurrence of the identifier token "test" as a
    // test indicator. This is conservative (false positives e.g. for
    // cfg(feature="testing") are possible), but in the context "allow panic
    // in tests" uncritical: when in doubt, code is not linted.
    match meta {
        Meta::Path(p) => p.is_ident("test"),
        Meta::List(l) => l
            .tokens
            .to_string()
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|t| t == "test"),
        Meta::NameValue(_) => false,
    }
}

struct Visitor<'a> {
    file: &'a std::path::Path,
    diagnostics: Vec<Diagnostic>,
    /// Non-null when we are currently under a `#[cfg(test)]` item.
    in_test_depth: usize,
}

impl Visitor<'_> {
    fn emit(&mut self, span: Span, what: &str) {
        if self.in_test_depth > 0 {
            return;
        }
        let start = span.start();
        self.diagnostics.push(Diagnostic::error(
            self.file,
            start.line,
            start.column.saturating_add(1),
            NAME,
            format!(
                "{what} in a SAFE crate; use Result/Option or \
                 mark with `zerodds-lint: allow no_panic_in_safe`"
            ),
        ));
    }
}

impl<'ast> Visit<'ast> for Visitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if has_cfg_test(&node.attrs) {
            self.in_test_depth += 1;
            visit::visit_item_fn(self, node);
            self.in_test_depth -= 1;
        } else {
            visit::visit_item_fn(self, node);
        }
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        if has_cfg_test(&node.attrs) {
            self.in_test_depth += 1;
            visit::visit_item_mod(self, node);
            self.in_test_depth -= 1;
        } else {
            visit::visit_item_mod(self, node);
        }
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        if has_cfg_test(&node.attrs) {
            self.in_test_depth += 1;
            visit::visit_item_impl(self, node);
            self.in_test_depth -= 1;
        } else {
            visit::visit_item_impl(self, node);
        }
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let name = node.method.to_string();
        if name == "unwrap" || name == "expect" {
            self.emit(node.method.span(), format!("`.{name}(...)`").as_str());
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        if let Some(ident) = node.path.get_ident() {
            let name = ident.to_string();
            if matches!(
                name.as_str(),
                "panic" | "unreachable" | "todo" | "unimplemented"
            ) {
                self.emit(ident.span(), format!("`{name}!(...)`").as_str());
            }
        }
        visit::visit_macro(self, node);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(src: &str, class: Option<SafetyClass>) -> Vec<Diagnostic> {
        run_at(src, class, &PathBuf::from("crates/foo/src/lib.rs"))
    }

    fn run_at(src: &str, class: Option<SafetyClass>, path: &std::path::Path) -> Vec<Diagnostic> {
        let ast = syn::parse_file(src).expect("parse");
        let lint = NoPanicInSafe;
        let ctx = FileLintContext {
            file: path,
            source: src,
            ast: &ast,
            crate_class: class,
            crate_name: "test",
        };
        lint.check(&ctx)
    }

    #[test]
    fn unwrap_in_safe_flagged() {
        let src = "fn f(x: Option<u32>) -> u32 { x.unwrap() }\n";
        assert_eq!(run(src, Some(SafetyClass::Safe)).len(), 1);
    }

    #[test]
    fn expect_in_safe_flagged() {
        let src = "fn f(x: Option<u32>) -> u32 { x.expect(\"reason\") }\n";
        assert_eq!(run(src, Some(SafetyClass::Safe)).len(), 1);
    }

    #[test]
    fn panic_macro_in_safe_flagged() {
        let src = "fn f() { panic!(\"oops\"); }\n";
        assert_eq!(run(src, Some(SafetyClass::Safe)).len(), 1);
    }

    #[test]
    fn unreachable_todo_unimplemented_flagged() {
        assert_eq!(
            run("fn f() { unreachable!(); }\n", Some(SafetyClass::Safe)).len(),
            1
        );
        assert_eq!(
            run("fn f() { todo!(); }\n", Some(SafetyClass::Safe)).len(),
            1
        );
        assert_eq!(
            run("fn f() { unimplemented!(); }\n", Some(SafetyClass::Safe)).len(),
            1
        );
    }

    #[test]
    fn standard_crate_is_not_linted() {
        let src = "fn f() { panic!() }\n";
        assert!(run(src, Some(SafetyClass::Standard)).is_empty());
    }

    #[test]
    fn unclassified_crate_is_not_linted() {
        let src = "fn f() { panic!() }\n";
        assert!(run(src, None).is_empty());
    }

    #[test]
    fn cfg_test_fn_is_skipped() {
        let src = "#[cfg(test)]\nfn f() { panic!() }\n";
        assert!(run(src, Some(SafetyClass::Safe)).is_empty());
    }

    #[test]
    fn test_attr_fn_is_skipped() {
        let src = "#[test]\nfn f() { panic!() }\n";
        assert!(run(src, Some(SafetyClass::Safe)).is_empty());
    }

    #[test]
    fn cfg_test_module_is_skipped() {
        let src = "#[cfg(test)]\nmod tests { fn t() { panic!(); } }\n";
        assert!(run(src, Some(SafetyClass::Safe)).is_empty());
    }

    #[test]
    fn file_under_tests_dir_is_skipped() {
        let src = "fn f() { panic!() }\n";
        let path = PathBuf::from("crates/foo/tests/it.rs");
        assert!(run_at(src, Some(SafetyClass::Safe), &path).is_empty());
    }

    #[test]
    fn file_under_examples_dir_is_skipped() {
        let src = "fn main() { panic!() }\n";
        let path = PathBuf::from("crates/foo/examples/demo.rs");
        assert!(run_at(src, Some(SafetyClass::Safe), &path).is_empty());
    }

    #[test]
    fn file_allow_marker_silences_lint() {
        let src = "// zerodds-lint: allow no_panic_in_safe\nfn f() { panic!() }\n";
        assert!(run(src, Some(SafetyClass::Safe)).is_empty());
    }

    #[test]
    fn inner_cfg_test_attr_on_file_skips_whole_file() {
        let src = "#![cfg(test)]\nfn f() { panic!() }\n";
        assert!(run(src, Some(SafetyClass::Safe)).is_empty());
    }
}
