// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds_no_alloc_in_hot_path` — flags heap allocations in functions
//! or modules marked as hot-path.
//!
//! # Marking
//!
//! The spec envisages `#[dds_hot_path]` as a custom attribute. On stable
//! Rust without `register_tool` this is syntactically not possible without a
//! proc-macro. For phase 0 we use a **doc-comment
//! marker** instead, which is syntactically a regular `#[doc = "..."]`
//! attribute and changes nothing about compilation:
//!
//! ```ignore
//! /// zerodds-lint: hot-path
//! fn tight_loop() { ... }
//! ```
//!
//! A doc comment with the marker **anywhere** in the body marks the
//! item as hot-path. For modules the marking applies transitively to
//! all contained functions (incl. nested modules).
//!
//! # Captured allocations
//!
//! - `Vec::new()`, `Vec::with_capacity(...)`, `vec![...]` macro
//! - `Box::new(...)`, `Rc::new(...)`, `Arc::new(...)`
//! - `String::new()`, `String::from(...)`, `format!(...)` macro
//! - method calls `.to_string()`, `.to_owned()`, `.to_vec()`, `.collect()`, `.clone()`
//! - `.push(...)` on collections — heuristically flagged (false positives
//!   possible, but worth attention in the hot path)
//!
//! Spec: `docs/architecture/04_safety_by_architecture.md §3.4`.

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, ExprCall, ExprMethodCall, ItemFn, ItemMod, Macro};

use crate::diagnostic::Diagnostic;

use super::{FileLint, FileLintContext};

/// Lint implementation.
pub struct NoAllocInHotPath;

const NAME: &str = "dds_no_alloc_in_hot_path";
const HOT_PATH_MARKER: &str = "zerodds-lint: hot-path";

impl FileLint for NoAllocInHotPath {
    fn name(&self) -> &'static str {
        NAME
    }

    fn check(&self, ctx: &FileLintContext<'_>) -> Vec<Diagnostic> {
        // Quick check: marker anywhere in the file? Strict form
        // (not via substring match), so that `hot-path-realloc-free`
        // does not activate this lint.
        if !marker_present(ctx.source, HOT_PATH_MARKER) {
            return Vec::new();
        }
        let mut visitor = Visitor {
            file: ctx.file,
            diagnostics: Vec::new(),
            hot_depth: 0,
        };
        visitor.visit_file(ctx.ast);
        visitor.diagnostics
    }
}

/// Searches doc comments (via `#[doc = "..."]` attributes) for the
/// hot-path marker. The match is intentionally strict: "hot-path" must be at
/// the word end, otherwise relatives like
/// `hot-path-realloc-free` (see the `no_realloc_in_hot_path` lint) would also
/// match the strict marker.
fn has_hot_path_marker(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("doc") {
            return false;
        }
        let syn::Meta::NameValue(nv) = &a.meta else {
            return false;
        };
        let syn::Expr::Lit(lit) = &nv.value else {
            return false;
        };
        let syn::Lit::Str(s) = &lit.lit else {
            return false;
        };
        marker_present(&s.value(), HOT_PATH_MARKER)
    })
}

/// Checks whether `marker` occurs as an isolated token in `text` — i.e.
/// after the marker there is either end-of-line, whitespace or
/// punctuation, but no letter or hyphen.
pub(crate) fn marker_present(text: &str, marker: &str) -> bool {
    text.match_indices(marker).any(|(i, _)| {
        let after = i + marker.len();
        let next = text.as_bytes().get(after).copied();
        match next {
            None => true,
            Some(b) => !(b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        }
    })
}

struct Visitor<'a> {
    file: &'a std::path::Path,
    diagnostics: Vec<Diagnostic>,
    hot_depth: usize,
}

impl Visitor<'_> {
    fn emit(&mut self, span: Span, what: &str) {
        if self.hot_depth == 0 {
            return;
        }
        let start = span.start();
        self.diagnostics.push(Diagnostic::error(
            self.file,
            start.line,
            start.column.saturating_add(1),
            NAME,
            format!("allocation `{what}` in the hot path"),
        ));
    }
}

const ALLOC_FREE_FNS: &[&str] = &["new", "with_capacity", "from"];
const ALLOC_FREE_TYPES: &[&str] = &["Vec", "Box", "Rc", "Arc", "String"];

fn is_alloc_call(call: &ExprCall) -> Option<String> {
    let syn::Expr::Path(p) = call.func.as_ref() else {
        return None;
    };
    // e.g. Vec::new() => segments [Vec, new]
    let segs: Vec<String> = p
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    if segs.len() < 2 {
        return None;
    }
    let ty = &segs[segs.len() - 2];
    let func = &segs[segs.len() - 1];
    if ALLOC_FREE_TYPES.contains(&ty.as_str()) && ALLOC_FREE_FNS.contains(&func.as_str()) {
        Some(format!("{ty}::{func}"))
    } else {
        None
    }
}

const ALLOC_METHOD_NAMES: &[&str] = &[
    "to_string",
    "to_owned",
    "to_vec",
    "collect",
    "clone",
    "push",
];

const ALLOC_MACRO_NAMES: &[&str] = &["vec", "format"];

impl<'ast> Visit<'ast> for Visitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let marked = has_hot_path_marker(&node.attrs);
        if marked {
            self.hot_depth += 1;
        }
        visit::visit_item_fn(self, node);
        if marked {
            self.hot_depth -= 1;
        }
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let marked = has_hot_path_marker(&node.attrs);
        if marked {
            self.hot_depth += 1;
        }
        visit::visit_item_mod(self, node);
        if marked {
            self.hot_depth -= 1;
        }
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Some(name) = is_alloc_call(node) {
            self.emit(node.span(), &name);
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let m = node.method.to_string();
        if ALLOC_METHOD_NAMES.iter().any(|&n| n == m) {
            self.emit(node.method.span(), format!(".{m}(...)").as_str());
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        if let Some(ident) = node.path.get_ident() {
            let name = ident.to_string();
            if ALLOC_MACRO_NAMES.contains(&name.as_str()) {
                self.emit(ident.span(), format!("{name}!(...)").as_str());
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

    fn run(src: &str) -> Vec<Diagnostic> {
        let ast = syn::parse_file(src).expect("parse");
        let lint = NoAllocInHotPath;
        let path = PathBuf::from("test.rs");
        let ctx = FileLintContext {
            file: &path,
            source: src,
            ast: &ast,
            crate_class: None,
            crate_name: "test",
        };
        lint.check(&ctx)
    }

    #[test]
    fn no_marker_no_findings() {
        let src = "fn f() { let _ = Vec::<u8>::new(); }\n";
        assert!(run(src).is_empty());
    }

    #[test]
    fn vec_new_in_hot_path_flagged() {
        let src = "/// zerodds-lint: hot-path\nfn f() { let _ = Vec::<u8>::new(); }\n";
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn box_new_in_hot_path_flagged() {
        let src = "/// zerodds-lint: hot-path\nfn f() { let _ = Box::new(42); }\n";
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn format_macro_in_hot_path_flagged() {
        let src = "/// zerodds-lint: hot-path\nfn f() { let _ = format!(\"x\"); }\n";
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn to_string_in_hot_path_flagged() {
        let src = "/// zerodds-lint: hot-path\nfn f(s: &str) { let _ = s.to_string(); }\n";
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn module_marker_applies_to_all_fns() {
        let src = "/// zerodds-lint: hot-path\nmod hot {\n    pub fn a() { let _ = Vec::<u8>::new(); }\n    pub fn b() { format!(\"x\"); }\n}\n";
        assert_eq!(run(src).len(), 2);
    }

    #[test]
    fn non_hot_sibling_not_flagged() {
        let src = concat!(
            "/// zerodds-lint: hot-path\n",
            "fn hot() { let _ = Vec::<u8>::new(); }\n",
            "fn cold() { let _ = Vec::<u8>::new(); }\n",
        );
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn multiple_allocations_counted_separately() {
        let src = concat!(
            "/// zerodds-lint: hot-path\n",
            "fn f() {\n    let _ = Vec::<u8>::new();\n    let _ = Box::new(1);\n    let _ = format!(\"x\");\n}\n",
        );
        assert_eq!(run(src).len(), 3);
    }
}
