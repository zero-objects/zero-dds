// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds_no_realloc_in_hot_path` — flaggt **Heap-Reallokationen** in
//! Funktionen oder Modulen, die als Hot-Path-Realloc-Free markiert sind.
//!
//! # Zweck
//!
//! Dieser Lint ist eine **strikt fokussierte Untermenge** von
//! [`no_alloc_in_hot_path`](super::no_alloc_in_hot_path) — er flaggt
//! nur die Allokations-Patterns, die spezifisch im DDS-Hot-Path
//! verboten sind:
//!
//! * `Vec::with_capacity(...)` — Pro-Sample-Realloc, soll durch
//!   `PoolBuffer<CAP>` ersetzt werden.
//! * `Vec::new()` — wird in 90 % der Faelle direkt mit Push/Extend
//!   befuellt und impliziert mehrfache Reallocs.
//! * `Box::new(...)`, `Rc::new(...)`, `Arc::new(...)` — Heap-
//!   Allokation pro Hot-Path-Iteration.
//!
//! Was dieser Lint **nicht** flaggt: `String::from`, `format!`,
//! `.clone()`, `.push()`, `.to_string()`, `.collect()`. Diese sind in
//! Error-Pfaden / Slow-Paths haeufig legitim, und der striktere
//! `no_alloc_in_hot_path`-Lint deckt sie ab fuer voll-realtime-
//! kritische Loops.
//!
//! Spec: WP 5.D.1c (`docs/PHASE5_PLAN.md`).
//!
//! # Markierung
//!
//! Doc-Kommentar-Marker in einer Funktion oder einem Modul:
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

/// Lint-Implementierung.
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
            format!("Realloc `{what}` im Hot-Path (verwende `PoolBuffer<CAP>`)"),
        ));
    }
}

/// Allokations-Konstruktoren, die wir flaggen. Tuple `(typ, fn)`.
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
        // Anders als no_alloc_in_hot_path: String::from ist OK
        // (haeufig in Error-Pfaden).
        let src = concat!(
            "/// zerodds-lint: hot-path-realloc-free\n",
            "fn f() { let _ = String::from(\"x\"); }\n",
        );
        assert!(run(src).is_empty());
    }

    #[test]
    fn arc_from_not_flagged() {
        // Arc::from(slice) ist die einzige unvermeidbare Allokation
        // pro Sample fuer Cache+Fanout (Reliable-Writer). Bewusst NICHT
        // geflaggt.
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
