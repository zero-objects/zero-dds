// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds_bounded_recursion` — functions that call themselves (intra-file)
//! directly or via a 1-hop indirect path must carry a
//! doc-comment marker `zerodds-lint: recursion-depth N`.
//!
//! Phase-0 approximation:
//! - The call graph is built **only within the same file**.
//!   Cross-file recursion (e.g. via trait implementations
//!   or mod splits) is not detected by the lint.
//! - Only free function calls (`foo()`) and path calls (`Self::foo()`,
//!   `Type::foo()`) are analyzed; `self.foo()` method calls
//!   are skipped, because resolving them requires type info.
//! - Cycles longer than 2 (A → B → C → A) are not detected; that
//!   would be a DFS on the call graph, addable in phase 1.
//!
//! # Marker
//!
//! ```ignore
//! /// zerodds-lint: recursion-depth 8
//! fn recursive_fn() { recursive_fn(); }
//! ```
//!
//! The lint only checks the **presence** of the marker, not the stated
//! depth — the depth assumption is documentation for reviewers.
//!
//! Spec: `docs/architecture/04_safety_by_architecture.md §3.4`.

use std::collections::{HashMap, HashSet};

use proc_macro2::Span;
use syn::visit::{self, Visit};
use syn::{Attribute, ExprCall, ItemFn, ItemImpl, ItemMod, Meta};

use crate::diagnostic::Diagnostic;

use super::{FileLint, FileLintContext};

/// True if the path is under `tests/`, `examples/` or `benches/`.
fn is_test_path(file: &std::path::Path) -> bool {
    file.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("tests") | Some("examples") | Some("benches")
        )
    })
}

/// True if the attributes contain `#[test]`, `#[cfg(test)]` or `cfg(... test ...)`.
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

/// Lint implementation.
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
            // Directly recursive?
            if info.calls.contains(name) {
                diags.push(diagnostic_for(
                    ctx.file,
                    name,
                    info.span,
                    "directly recursive",
                ));
                continue;
            }
            // 1-hop indirect: does `info` call a `B` that in turn calls `name`?
            for callee in &info.calls {
                let Some(callee_info) = collector.fns.get(callee.as_str()) else {
                    continue;
                };
                if callee_info.calls.contains(name) && names.contains(callee.as_str()) {
                    diags.push(diagnostic_for(
                        ctx.file,
                        name,
                        info.span,
                        format!("indirectly recursive via `{callee}`").as_str(),
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
            "function `{fn_name}` is {why}; add a doc comment \
             `/// {MARKER_PREFIX} N` with the expected depth"
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
    /// Non-null when currently under a `#[cfg(test)]`/`#[test]` item.
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
        assert!(d[0].message.contains("directly recursive"));
    }

    #[test]
    fn indirect_one_hop_recursion_flagged() {
        let src = "fn a() { b(); }\nfn b() { a(); }\n";
        let d = run(src);
        // Both are flagged because both are part of the cycle.
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
        // a has a marker, b does not
        let d = run(src);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("`b`") || d[0].message.contains("indirekt"));
    }

    #[test]
    fn type_qualified_call_counted() {
        // `Self::a()` counts as a call to `a` — simplified assumption.
        let src = "fn a() { Self::a(); }\n";
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn three_hop_cycle_not_detected_phase0_limit() {
        // A->B->C->A is NOT detected in phase 0 (no output).
        let src = "fn a() { b(); }\nfn b() { c(); }\nfn c() { a(); }\n";
        assert!(run(src).is_empty());
    }
}
