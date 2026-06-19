// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds_no_realloc_in_hot_path` — flags **heap reallocations** in
//! functions or modules marked as hot-path-realloc-free.
//!
//! # Purpose
//!
//! This lint is a **strictly focused subset** of
//! [`no_alloc_in_hot_path`](super::no_alloc_in_hot_path) — it flags
//! only the allocation patterns that are specifically forbidden in the
//! DDS hot path:
//!
//! * `Vec::with_capacity(...)` — per-sample realloc, should be replaced by
//!   `PoolBuffer<CAP>`.
//! * `Vec::new()` — in 90 % of cases it is immediately filled via push/extend
//!   and implies multiple reallocs.
//! * `Box::new(...)`, `Rc::new(...)`, `Arc::new(...)` — heap
//!   allocation per hot-path iteration.
//!
//! What this lint does **not** flag: `String::from`, `format!`,
//! `.clone()`, `.push()`, `.to_string()`, `.collect()`. These are often
//! legitimate in error paths / slow paths, and the stricter
//! `no_alloc_in_hot_path` lint covers them for fully-realtime-
//! critical loops.
//!
//! Spec: WP 5.D.1c (`docs/PHASE5_PLAN.md`).
//!
//! # Marking
//!
//! Doc-comment marker in a function or a module:
//!
//! ```ignore
//! /// zerodds-lint: hot-path-realloc-free
//! fn frame_user_payload_pooled(...) { ... }
//! ```

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, ExprCall, ItemFn, ItemMod};

use crate::diagnostic::Diagnostic;

use super::{FileLint, FileLintContext};

/// Lint implementation.
pub struct NoReallocInHotPath;

const NAME: &str = "dds_no_realloc_in_hot_path";
const HOT_PATH_MARKER: &str = "zerodds-lint: hot-path-realloc-free";

impl FileLint for NoReallocInHotPath {
    fn name(&self) -> &'static str {
        NAME
    }

    fn check(&self, ctx: &FileLintContext<'_>) -> Vec<Diagnostic> {
        if !ctx.source.contains(HOT_PATH_MARKER) {
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
        s.value().contains(HOT_PATH_MARKER)
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
            format!("realloc `{what}` in the hot path (use `PoolBuffer<CAP>`)"),
        ));
    }
}

/// Allocation constructors that we flag. Tuple `(type, fn)`.
const REALLOC_CALLS: &[(&str, &str)] = &[
    ("Vec", "with_capacity"),
    ("Vec", "new"),
    ("Box", "new"),
    ("Rc", "new"),
    ("Arc", "new"),
];

fn is_realloc_call(call: &ExprCall) -> Option<String> {
    let syn::Expr::Path(p) = call.func.as_ref() else {
        return None;
    };
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
    REALLOC_CALLS
        .iter()
        .find(|(t, f)| *t == ty.as_str() && *f == func.as_str())
        .map(|(t, f)| format!("{t}::{f}"))
}

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
        if let Some(name) = is_realloc_call(node) {
            self.emit(node.span(), &name);
        }
        visit::visit_expr_call(self, node);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(src: &str) -> Vec<Diagnostic> {
        let ast = syn::parse_file(src).expect("parse");
        let lint = NoReallocInHotPath;
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
        let src = "fn f() { let _ = Vec::<u8>::with_capacity(8); }\n";
        assert!(run(src).is_empty());
    }

    #[test]
    fn vec_with_capacity_in_hot_path_flagged() {
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "fn f() { let _ = Vec::<u8>::with_capacity(8); }\n",
        );
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn vec_new_in_hot_path_flagged() {
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "fn f() { let _ = Vec::<u8>::new(); }\n",
        );
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn box_new_in_hot_path_flagged() {
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "fn f() { let _ = Box::new(42); }\n",
        );
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn arc_new_in_hot_path_flagged() {
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "fn f() { let _ = std::sync::Arc::new(42); }\n",
        );
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn string_from_not_flagged() {
        // Unlike no_alloc_in_hot_path: String::from is OK
        // (often in error paths).
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "fn f() { let _ = String::from(\"x\"); }\n",
        );
        assert!(run(src).is_empty());
    }

    #[test]
    fn arc_from_not_flagged() {
        // Arc::from(slice) is the only unavoidable allocation
        // per sample for cache+fanout (Reliable writer). Deliberately NOT
        // flagged.
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "fn f() { let _ = std::sync::Arc::<[u8]>::from(&[1u8, 2u8][..]); }\n",
        );
        assert!(run(src).is_empty());
    }

    #[test]
    fn format_macro_not_flagged() {
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "fn f() { let _ = format!(\"x\"); }\n",
        );
        assert!(run(src).is_empty());
    }

    #[test]
    fn clone_method_not_flagged() {
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "fn f(s: &String) { let _ = s.clone(); }\n",
        );
        assert!(run(src).is_empty());
    }

    #[test]
    fn module_marker_applies_to_all_fns() {
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "mod hot {\n",
            "    pub fn a() { let _ = Vec::<u8>::with_capacity(8); }\n",
            "    pub fn b() { let _ = Box::new(42); }\n",
            "}\n",
        );
        assert_eq!(run(src).len(), 2);
    }

    #[test]
    fn non_hot_sibling_not_flagged() {
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "fn hot() { let _ = Vec::<u8>::with_capacity(8); }\n",
            "fn cold() { let _ = Vec::<u8>::with_capacity(8); }\n",
        );
        assert_eq!(run(src).len(), 1);
    }
}
