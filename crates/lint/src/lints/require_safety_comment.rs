// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds_require_safety_comment` — jeder `unsafe`-Block, jede `unsafe fn`,
//! jedes `unsafe impl` braucht direkt davor einen `// SAFETY:`-Kommentar
//! mit mindestens einem nicht-leeren Satz.
//!
//! Fuer `pub unsafe fn` wird zusaetzlich die rustdoc-Form
//! `# Safety` (Rust API Guidelines C-FAILURE / Clippy
//! `missing_safety_doc`) akzeptiert.
//!
//! Mehrzeilige Kommentar-Bloecke werden unterstuetzt: ausgehend von
//! der Zeile direkt ueber dem `unsafe` laeuft der Matcher rueckwaerts
//! durch zusammenhaengende `//`-Zeilen, bis er entweder eine
//! `// SAFETY:`-Zeile findet oder eine Nicht-Kommentar-Zeile trifft.
//!
//! Spec: `docs/architecture/04_safety_by_architecture.md §3.4`.

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, ExprUnsafe, ItemFn, ItemImpl};

use crate::diagnostic::Diagnostic;

use super::{FileLint, FileLintContext};

/// Lint-Implementierung.
pub struct RequireSafetyComment;

const NAME: &str = "dds_require_safety_comment";

impl FileLint for RequireSafetyComment {
    fn name(&self) -> &'static str {
        NAME
    }

    fn check(&self, ctx: &FileLintContext<'_>) -> Vec<Diagnostic> {
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut visitor = Visitor {
            lines: &lines,
            file: ctx.file,
            diagnostics: Vec::new(),
        };
        visitor.visit_file(ctx.ast);
        visitor.diagnostics
    }
}

struct Visitor<'a> {
    lines: &'a [&'a str],
    file: &'a std::path::Path,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<'_> {
    /// Pruefung fuer `unsafe { ... }`-Bloecke und `unsafe impl`-Items
    /// — verlangt strikt einen `// SAFETY:`-Kommentar oberhalb.
    fn check_block_or_impl(&mut self, span: Span, what: &str) {
        let start = span.start();
        let line = start.line;
        let col = start.column.saturating_add(1);
        if !has_safety_comment_above(self.lines, line) {
            self.diagnostics.push(Diagnostic::error(
                self.file,
                line,
                col,
                NAME,
                format!("{what} ohne `// SAFETY:`-Kommentar in der Zeile davor"),
            ));
        }
    }

    /// Pruefung fuer `unsafe fn`-Deklarationen — akzeptiert beide
    /// Konventionen:
    ///
    /// 1. `// SAFETY: <text>`-Kommentar direkt davor.
    /// 2. `# Safety` als Section in der vorausgehenden rustdoc
    ///    (Rust API Guidelines C-FAILURE).
    fn check_unsafe_fn(&mut self, span: Span, attrs: &[Attribute]) {
        let start = span.start();
        let line = start.line;
        let col = start.column.saturating_add(1);
        if has_safety_comment_above(self.lines, line) {
            return;
        }
        if has_safety_section_in_doc(attrs) {
            return;
        }
        self.diagnostics.push(Diagnostic::error(
            self.file,
            line,
            col,
            NAME,
            "unsafe fn ohne `// SAFETY:`-Kommentar oder rustdoc `# Safety`-Section",
        ));
    }
}

impl<'ast> Visit<'ast> for Visitor<'_> {
    fn visit_expr_unsafe(&mut self, node: &'ast ExprUnsafe) {
        self.check_block_or_impl(node.unsafe_token.span(), "unsafe-Block");
        visit::visit_expr_unsafe(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if let Some(unsafe_token) = node.sig.unsafety.as_ref() {
            self.check_unsafe_fn(unsafe_token.span(), &node.attrs);
        }
        visit::visit_item_fn(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        if let Some(unsafe_token) = node.unsafety.as_ref() {
            self.check_block_or_impl(unsafe_token.span(), "unsafe impl");
        }
        visit::visit_item_impl(self, node);
    }
}

/// Walker rueckwaerts durch zusammenhaengende Kommentar-Zeilen, bis ein
/// `// SAFETY:` mit Inhalt gefunden wird oder der Block bricht.
///
/// `line` ist 1-basiert (proc-macro2-Konvention).
fn has_safety_comment_above(lines: &[&str], line: usize) -> bool {
    if line < 2 {
        return false;
    }
    let mut idx = line.saturating_sub(2);
    loop {
        let cur = lines.get(idx).map(|s| s.trim()).unwrap_or("");
        // Leere Zeile zwischen Kommentar und unsafe ist nicht
        // erlaubt: bricht den Block ab.
        if cur.is_empty() {
            return false;
        }
        if !cur.starts_with("//") {
            return false;
        }
        if let Some(rest) = cur.strip_prefix("// SAFETY:") {
            return !rest.trim().is_empty();
        }
        if idx == 0 {
            return false;
        }
        idx -= 1;
    }
}

/// Prueft ob die rustdoc-Attribute eine `# Safety`-Section enthalten —
/// die kanonische Form fuer `pub unsafe fn` (Rust API Guidelines
/// C-FAILURE / Clippy `missing_safety_doc`).
fn has_safety_section_in_doc(attrs: &[Attribute]) -> bool {
    for a in attrs {
        if !a.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(nv) = &a.meta else {
            continue;
        };
        let syn::Expr::Lit(lit) = &nv.value else {
            continue;
        };
        let syn::Lit::Str(s) = &lit.lit else {
            continue;
        };
        let v = s.value();
        let trimmed = v.trim_start();
        if trimmed.starts_with("# Safety")
            || trimmed.starts_with("## Safety")
            || trimmed.starts_with("# SAFETY")
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(src: &str) -> Vec<Diagnostic> {
        let ast = syn::parse_file(src).expect("parse");
        let lint = RequireSafetyComment;
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
    fn unsafe_block_with_safety_comment_passes() {
        let src = "fn f() {\n    // SAFETY: invariants hold\n    unsafe { let _ = 1; }\n}\n";
        assert!(run(src).is_empty());
    }

    #[test]
    fn unsafe_block_without_safety_comment_fails() {
        let src = "fn f() {\n    unsafe { let _ = 1; }\n}\n";
        let d = run(src);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].lint, "dds_require_safety_comment");
    }

    #[test]
    fn unsafe_fn_requires_safety_comment_or_doc() {
        let bad = "unsafe fn f() {}\n";
        assert_eq!(run(bad).len(), 1);

        let good_safety = "// SAFETY: caller guarantees X\nunsafe fn f() {}\n";
        assert!(run(good_safety).is_empty());

        let good_doc = "/// # Safety\n/// caller guarantees X\npub unsafe fn f() {}\n";
        assert!(run(good_doc).is_empty());
    }

    #[test]
    fn unsafe_impl_requires_safety_comment() {
        let src = "unsafe impl Send for () {}\n";
        assert_eq!(run(src).len(), 1);

        let good = "// SAFETY: () is empty so Send is trivial\nunsafe impl Send for () {}\n";
        assert!(run(good).is_empty());
    }

    #[test]
    fn empty_safety_comment_is_rejected() {
        let src = "// SAFETY:\nunsafe fn f() {}\n";
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn safety_comment_with_whitespace_indent_passes() {
        let src = "fn f() {\n        // SAFETY: see RFC 0001\n        unsafe { let _ = 1; }\n}\n";
        assert!(run(src).is_empty());
    }

    #[test]
    fn multiline_safety_block_passes() {
        let src = concat!(
            "fn f() {\n",
            "    // SAFETY: invariants hold because:\n",
            "    // 1. ptr is stack-local\n",
            "    // 2. no aliasing\n",
            "    unsafe { let _ = 1; }\n",
            "}\n",
        );
        assert!(run(src).is_empty());
    }

    #[test]
    fn blank_line_between_safety_and_unsafe_breaks_block() {
        let src = concat!(
            "fn f() {\n",
            "    // SAFETY: invariants hold\n",
            "\n",
            "    unsafe { let _ = 1; }\n",
            "}\n",
        );
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn non_comment_line_between_safety_and_unsafe_breaks_block() {
        let src = concat!(
            "fn f() {\n",
            "    // SAFETY: invariants hold\n",
            "    let _x = 5;\n",
            "    unsafe { let _ = 1; }\n",
            "}\n",
        );
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn unsafe_block_inside_unsafe_fn_still_needs_safety_comment() {
        // Auch in einer unsafe fn braucht jeder bare unsafe-Block
        // einen `// SAFETY:`-Kommentar — sonst zerfallen die
        // Begruendungen ueber Funktionsgrenzen.
        let src = concat!(
            "/// # Safety\n",
            "/// caller knows.\n",
            "pub unsafe fn f(p: *mut u8) {\n",
            "    let _ = unsafe { std::ptr::read(p) };\n",
            "}\n",
        );
        assert_eq!(run(src).len(), 1);
    }
}
