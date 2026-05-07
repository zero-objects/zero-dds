// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds_bounded_recursion` — Funktionen, die sich (intra-File) direkt oder
//! ueber einen 1-Hop-Indirect-Pfad selbst aufrufen, muessen einen
//! Doc-Comment-Marker `zerodds-lint: recursion-depth N` tragen.
//!
//! Phase-0-Approximation:
//! - Nur **innerhalb derselben Datei** wird der Call-Graph aufgebaut.
//!   File-uebergreifende Rekursion (z.B. via Trait-Implementierungen
//!   oder mod-Splits) erkennt der Lint nicht.
//! - Nur freie Funktionsaufrufe (`foo()`) und Pfad-Aufrufe (`Self::foo()`,
//!   `Type::foo()`) werden analysiert; `self.foo()`-Methodenaufrufe
//!   werden uebersprungen, weil ihre Aufloesung Type-Info erfordert.
//! - Cycles laenger als 2 (A → B → C → A) werden nicht erkannt; das
//!   waere DFS auf dem Call-Graph, in Phase 1 ergaenzbar.
//!
//! # Marker
//!
//! ```ignore
//! /// zerodds-lint: recursion-depth 8
//! fn recursive_fn() { recursive_fn(); }
//! ```
//!
//! Der Lint prueft nur **Anwesenheit** des Markers, nicht die genannte
//! Tiefe — die Tiefen-Annahme ist Dokumentation fuer Reviewer.
//!
//! Spec: `docs/architecture/04_safety_by_architecture.md §3.4`.

use std::collections::{HashMap, HashSet};

use proc_macro2::Span;
use syn::visit::{self, Visit};
use syn::{Attribute, ExprCall, ItemFn, ItemImpl, ItemMod, Meta};

use crate::diagnostic::Diagnostic;

use super::{FileLint, FileLintContext};

/// True wenn Pfad unter `tests/`, `examples/` oder `benches/` liegt.
fn is_test_path(file: &std::path::Path) -> bool {
    file.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("tests") | Some("examples") | Some("benches")
        )
    })
}

/// True wenn Attribute `#[test]`, `#[cfg(test)]` oder `cfg(... test ...)` enthaelt.
fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        if a.path().is_ident("test") {
            return true;
        }
        if !a.path().is_ident("cfg") {
            return false;
        }
        match &a.meta {
            Meta::Path(p) => p.is_ident("test"),
            Meta::List(l) => l
                .tokens
                .to_string()
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|t| t == "test"),
            Meta::NameValue(_) => false,
        }
    })
}

/// Lint-Implementierung.
pub struct BoundedRecursion;

const NAME: &str = "dds_bounded_recursion";
const MARKER_PREFIX: &str = "zerodds-lint: recursion-depth";

impl FileLint for BoundedRecursion {
    fn name(&self) -> &'static str {
        NAME
    }

    fn check(&self, ctx: &FileLintContext<'_>) -> Vec<Diagnostic> {
        if is_test_path(ctx.file) {
            return Vec::new();
        }
        if has_cfg_test(&ctx.ast.attrs) {
            return Vec::new();
        }
        let mut collector = FnCollector {
            fns: HashMap::new(),
            test_depth: 0,
        };
        collector.visit_file(ctx.ast);

        let mut diags = Vec::new();
        let names: HashSet<&str> = collector.fns.keys().map(String::as_str).collect();
        for (name, info) in &collector.fns {
            if info.has_marker {
                continue;
            }
            // Direkt rekursiv?
            if info.calls.contains(name) {
                diags.push(diagnostic_for(ctx.file, name, info.span, "direkt rekursiv"));
                continue;
            }
            // 1-Hop indirekt: ruft `info` ein `B` auf, das wiederum `name` aufruft?
            for callee in &info.calls {
                let Some(callee_info) = collector.fns.get(callee.as_str()) else {
                    continue;
                };
                if callee_info.calls.contains(name) && names.contains(callee.as_str()) {
                    diags.push(diagnostic_for(
                        ctx.file,
                        name,
                        info.span,
                        format!("indirekt rekursiv via `{callee}`").as_str(),
                    ));
                    break;
                }
            }
        }
        diags
    }
}

fn diagnostic_for(file: &std::path::Path, fn_name: &str, span: Span, why: &str) -> Diagnostic {
    let start = span.start();
    Diagnostic::error(
        file,
        start.line,
        start.column.saturating_add(1),
        NAME,
        format!(
            "Funktion `{fn_name}` ist {why}; ergaenze Doc-Kommentar \
             `/// {MARKER_PREFIX} N` mit erwarteter Tiefe"
        ),
    )
}

#[derive(Debug)]
struct FnInfo {
    span: Span,
    calls: HashSet<String>,
    has_marker: bool,
}

struct FnCollector {
    fns: HashMap<String, FnInfo>,
    /// Nicht-null wenn aktuell unter einem `#[cfg(test)]`/`#[test]`-Item.
    test_depth: usize,
}

impl<'ast> Visit<'ast> for FnCollector {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let in_test = has_cfg_test(&node.attrs);
        if in_test {
            self.test_depth += 1;
        }
        if self.test_depth == 0 {
            let name = node.sig.ident.to_string();
            let span = node.sig.ident.span();
            let mut call_visitor = CallCollector {
                calls: HashSet::new(),
            };
            call_visitor.visit_block(&node.block);
            self.fns.insert(
                name,
                FnInfo {
                    span,
                    calls: call_visitor.calls,
                    has_marker: has_recursion_marker(&node.attrs),
                },
            );
        }
        visit::visit_item_fn(self, node);
        if in_test {
            self.test_depth -= 1;
        }
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let in_test = has_cfg_test(&node.attrs);
        if in_test {
            self.test_depth += 1;
        }
        visit::visit_item_mod(self, node);
        if in_test {
            self.test_depth -= 1;
        }
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        let in_test = has_cfg_test(&node.attrs);
        if in_test {
            self.test_depth += 1;
        }
        visit::visit_item_impl(self, node);
        if in_test {
            self.test_depth -= 1;
        }
    }
}

fn has_recursion_marker(attrs: &[Attribute]) -> bool {
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
        s.value().contains(MARKER_PREFIX)
    })
}

struct CallCollector {
    calls: HashSet<String>,
}

impl<'ast> Visit<'ast> for CallCollector {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let syn::Expr::Path(p) = node.func.as_ref() {
            if let Some(last) = p.path.segments.last() {
                self.calls.insert(last.ident.to_string());
            }
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
        let lint = BoundedRecursion;
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
    fn no_recursion_no_findings() {
        let src = "fn a() { b(); }\nfn b() {}\n";
        assert!(run(src).is_empty());
    }

    #[test]
    fn direct_recursion_flagged() {
        let src = "fn a() { a(); }\n";
        let d = run(src);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("direkt rekursiv"));
    }

    #[test]
    fn indirect_one_hop_recursion_flagged() {
        let src = "fn a() { b(); }\nfn b() { a(); }\n";
        let d = run(src);
        // Beide werden geflaggt, weil beide Teil des Cycles sind.
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn marker_silences_lint() {
        let src = "/// zerodds-lint: recursion-depth 5\nfn a() { a(); }\n";
        assert!(run(src).is_empty());
    }

    #[test]
    fn marker_on_one_of_two_partially_silences() {
        let src = concat!(
            "/// zerodds-lint: recursion-depth 5\n",
            "fn a() { b(); }\n",
            "fn b() { a(); }\n",
        );
        // a hat Marker, b nicht
        let d = run(src);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("`b`") || d[0].message.contains("indirekt"));
    }

    #[test]
    fn type_qualified_call_counted() {
        // `Self::a()` zaehlt als Aufruf von `a` — vereinfachte Annahme.
        let src = "fn a() { Self::a(); }\n";
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn three_hop_cycle_not_detected_phase0_limit() {
        // A->B->C->A wird in Phase 0 NICHT erkannt (keine Ausgabe).
        let src = "fn a() { b(); }\nfn b() { c(); }\nfn c() { a(); }\n";
        assert!(run(src).is_empty());
    }
}
