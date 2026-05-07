// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds_no_alloc_in_hot_path` — flaggt Heap-Allokationen in Funktionen
//! oder Modulen, die als Hot-Path markiert sind.
//!
//! # Markierung
//!
//! Die Spec sieht `#[dds_hot_path]` als Custom-Attribut vor. Auf stable
//! Rust ohne `register_tool` ist das syntaktisch nicht ohne Proc-Macro
//! moeglich. Fuer Phase 0 nutzen wir stattdessen einen **Doc-Comment-
//! Marker**, der syntaktisch ein regulaerer `#[doc = "..."]`-Attribut
//! ist und nichts an der Kompilierung aendert:
//!
//! ```ignore
//! /// zerodds-lint: hot-path
//! fn tight_loop() { ... }
//! ```
//!
//! Ein Doc-Kommentar mit dem Marker **irgendwo** im Body markiert das
//! Item als Hot-Path. Fuer Module gilt die Markierung transitiv fuer
//! alle enthaltenen Funktionen (inkl. geschachtelter Module).
//!
//! # Erfasste Allokationen
//!
//! - `Vec::new()`, `Vec::with_capacity(...)`, `vec![...]`-Macro
//! - `Box::new(...)`, `Rc::new(...)`, `Arc::new(...)`
//! - `String::new()`, `String::from(...)`, `format!(...)`-Macro
//! - Method-Calls `.to_string()`, `.to_owned()`, `.to_vec()`, `.collect()`, `.clone()`
//! - `.push(...)` auf Collections — heuristisch flagged (false positives
//!   moeglich, aber im Hot-Path aufmerksamkeitswert)
//!
//! Spec: `docs/architecture/04_safety_by_architecture.md §3.4`.

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, ExprCall, ExprMethodCall, ItemFn, ItemMod, Macro};

use crate::diagnostic::Diagnostic;

use super::{FileLint, FileLintContext};

/// Lint-Implementierung.
pub struct NoAllocInHotPath;

const NAME: &str = "dds_no_alloc_in_hot_path";
const HOT_PATH_MARKER: &str = "zerodds-lint: hot-path";

impl FileLint for NoAllocInHotPath {
    fn name(&self) -> &'static str {
        NAME
    }

    fn check(&self, ctx: &FileLintContext<'_>) -> Vec<Diagnostic> {
        // Schnell-Check: Marker irgendwo in der Datei? Strenge Form
        // (nicht via Substring-Match), damit `hot-path-realloc-free`
        // diesen Lint nicht aktiviert.
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

/// Sucht in Doc-Kommentaren (via `#[doc = "..."]`-Attributen) nach dem
/// Hot-Path-Marker. Match ist absichtlich streng: "hot-path" muss am
/// Wort-Ende stehen, sonst matchen Verwandte wie
/// `hot-path-realloc-free` (siehe `no_realloc_in_hot_path`-Lint) den
/// strikten Marker mit.
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

/// Prueft, ob `marker` als isolierter Token in `text` vorkommt — d.h.
/// nach dem Marker steht entweder Zeilenende, Whitespace oder
/// Satzzeichen, aber kein Buchstabe oder Bindestrich.
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
            format!("Allokation `{what}` im Hot-Path"),
        ));
    }
}

const ALLOC_FREE_FNS: &[&str] = &["new", "with_capacity", "from"];
const ALLOC_FREE_TYPES: &[&str] = &["Vec", "Box", "Rc", "Arc", "String"];

fn is_alloc_call(call: &ExprCall) -> Option<String> {
    let syn::Expr::Path(p) = call.func.as_ref() else {
        return None;
    };
    // z.B. Vec::new() => segments [Vec, new]
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
