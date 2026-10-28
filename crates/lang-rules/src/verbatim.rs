// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim` text, sorted into the slots a backend writes.
//!
//! OMG IDL 4.2 §8.3.5.1 defines the annotation:
//!
//! ```idl
//! @annotation verbatim {
//!     enum PlacementKind {
//!         BEGIN_FILE, BEFORE_DECLARATION, BEGIN_DECLARATION,
//!         END_DECLARATION, AFTER_DECLARATION, END_FILE
//!     };
//!     string language default "*";
//!     PlacementKind placement default BEFORE_DECLARATION;
//!     string text;
//! };
//! ```
//!
//! The `text` is copied into the generated artifact unchanged, at the
//! place the `placement` names, for the backends whose language the
//! `language` value names — `"c"`, `"c++"`, `"java"`, `"idl"` or the
//! wildcard `"*"` are the values §8.3.5.1 lists, and vendor spellings
//! like `"cpp"` or `"c#"` are matched beside them so a file written for
//! another tool keeps its meaning here. DDS-XTypes 1.3 §7.2.2.4.8
//! carries the same annotation as `VerbatimText` on the TypeObject.
//!
//! The lowering has already stored every directive on the node it
//! annotates ([`zerodds_ast::XTypesFacet::verbatim`], in declaration
//! order); this module only sorts them into the six slots and answers
//! the language question. What each backend does with the slots is the
//! backend's:
//!
//! - `begin_file` / `end_file` open and close the whole generated file,
//!   wherever the annotation stood.
//! - `before` / `after` surround the annotated declaration.
//! - `begin` / `end` land **inside** the annotated declaration's scope.
//!   A backend whose tree cannot hold raw text inside the declaration
//!   in hand — the C column's `struct`, any column's `enum` — writes
//!   them adjacent to it instead: the text is never dropped, and the
//!   nearest position the tree can express is where it lands.
//! - A directive on a **member** is hoisted into its aggregate's
//!   `begin` slot (`BEGIN_FILE`/`END_FILE` keep their file meaning):
//!   the four columns declare an aggregate's body as one node, and the
//!   inside of that body is the position the tree can express today.
//!   `docs/OPEN-ITEMS.md` records exact member positioning as open.

use zerodds_ast::{Ast, TypeKind, TypeNode, VerbatimPlacement};
use zerodds_ast_core::Id;

/// The six §8.3.5.1 placements, gathered for one declaration.
///
/// Each entry is one directive's `text`, kept whole (the renderer
/// splits it on `\n`), in declaration order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Slots {
    /// `BEGIN_FILE` — the start of the generated file.
    pub begin_file: Vec<String>,
    /// `BEFORE_DECLARATION` — immediately before the declaration.
    pub before: Vec<String>,
    /// `BEGIN_DECLARATION` — inside the declaration, at its start.
    pub begin: Vec<String>,
    /// `END_DECLARATION` — inside the declaration, at its end.
    pub end: Vec<String>,
    /// `AFTER_DECLARATION` — immediately after the declaration.
    pub after: Vec<String>,
    /// `END_FILE` — the end of the generated file.
    pub end_file: Vec<String>,
}

impl Slots {
    /// Whether every slot is empty, which is the common case and the
    /// one a backend should pay nothing for.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.begin_file.is_empty()
            && self.before.is_empty()
            && self.begin.is_empty()
            && self.end.is_empty()
            && self.after.is_empty()
            && self.end_file.is_empty()
    }

    fn push(&mut self, placement: VerbatimPlacement, text: &str) {
        let slot = match placement {
            VerbatimPlacement::BeginFile => &mut self.begin_file,
            VerbatimPlacement::BeforeDeclaration => &mut self.before,
            VerbatimPlacement::BeginDeclaration => &mut self.begin,
            VerbatimPlacement::EndDeclaration => &mut self.end,
            VerbatimPlacement::AfterDeclaration => &mut self.after,
            VerbatimPlacement::EndFile => &mut self.end_file,
        };
        slot.push(text.to_string());
    }
}

/// Whether a directive written for `language` concerns a backend that
/// answers to `aliases`.
///
/// §8.3.5.1 names the values `"c"`, `"c++"`, `"java"`, `"idl"` and
/// `"*"`; the wildcard concerns every backend. The comparison is
/// case-insensitive — `"C++"` and `"c++"` name one language — and each
/// backend passes every spelling it answers to, so `"cpp"` or `"c#"`
/// from a file written against another generator still lands.
#[must_use]
pub fn language_matches(language: &str, aliases: &[&str]) -> bool {
    if language == "*" {
        return true;
    }
    aliases
        .iter()
        .any(|alias| alias.eq_ignore_ascii_case(language))
}

/// The slots one type declaration carries, for a backend named by
/// `aliases`.
///
/// Gathers the directives on the type itself and — hoisted, see the
/// module documentation — the ones on its members and union cases.
/// Directives whose `language` does not concern this backend are left
/// out here and emitted by the backend they do concern.
///
/// The type's own `BEGIN_FILE`/`END_FILE` directives appear in the
/// result too, so a single-declaration caller sees everything the
/// declaration said — but [`file_slots`] reports the same directives
/// for the whole model, and a backend that writes files takes the file
/// slots from there and only `before`/`begin`/`end`/`after` from here,
/// or it would write the text twice.
#[must_use]
pub fn declaration_slots(ast: &Ast, id: Id<TypeNode>, aliases: &[&str]) -> Slots {
    let mut slots = Slots::default();
    let Ok(node) = ast.type_(id) else {
        return slots;
    };
    gather(&node.facets, aliases, &mut slots, false);
    for member in node.kind.members() {
        if let Ok(member_node) = ast.member(*member) {
            gather(&member_node.facets, aliases, &mut slots, true);
        }
    }
    if let TypeKind::Union {
        cases,
        default_case,
        ..
    } = &node.kind
    {
        for case in cases.iter().chain(default_case.iter()) {
            let Ok(case_node) = ast.case(*case) else {
                continue;
            };
            gather(&case_node.facets, aliases, &mut slots, true);
            if let Ok(member_node) = ast.member(case_node.member) {
                gather(&member_node.facets, aliases, &mut slots, true);
            }
        }
    }
    slots
}

/// The file-level slots of the whole model: every `BEGIN_FILE` and
/// `END_FILE` directive, wherever it was written.
///
/// §8.3.5.1 places these relative to the file, not to the annotated
/// element, so they are gathered from every module, type, member and
/// case — in model order — and the declaration-relative placements are
/// left to [`declaration_slots`].
#[must_use]
pub fn file_slots(ast: &Ast, aliases: &[&str]) -> (Vec<String>, Vec<String>) {
    let mut begin = Vec::new();
    let mut end = Vec::new();
    let mut take = |facets: &zerodds_ast::Facets| {
        let Some(xtypes) = facets.xtypes.as_ref() else {
            return;
        };
        for directive in &xtypes.verbatim {
            if !language_matches(&directive.language, aliases) {
                continue;
            }
            match directive.placement {
                VerbatimPlacement::BeginFile => begin.push(directive.text.clone()),
                VerbatimPlacement::EndFile => end.push(directive.text.clone()),
                _ => {}
            }
        }
    };
    for (_, node) in ast.modules() {
        take(&node.facets);
    }
    for (_, node) in ast.types() {
        take(&node.facets);
        for member in node.kind.members() {
            if let Ok(member_node) = ast.member(*member) {
                take(&member_node.facets);
            }
        }
        if let TypeKind::Union {
            cases,
            default_case,
            ..
        } = &node.kind
        {
            for case in cases.iter().chain(default_case.iter()) {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                take(&case_node.facets);
                if let Ok(member_node) = ast.member(case_node.member) {
                    take(&member_node.facets);
                }
            }
        }
    }
    (begin, end)
}

/// A column's declaration type, as far as `@verbatim` needs it.
///
/// The C, C++, C# and Java columns weave by hand; every other column
/// implements this and calls [`weave`] and [`weave_file`], so the slot
/// rules are written once.
pub trait Carrier: Sized {
    /// A declaration that is nothing but the text, copied unchanged.
    fn verbatim(text: String) -> Self;

    /// When `self` is the body-bearing declaration of the type named
    /// `name`, moves `begin` to the start and `end` to the end of that
    /// body and answers true. A declaration with no body that can hold
    /// raw lines answers false and leaves both alone.
    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool;
}

/// Weave one type's `@verbatim` text around and into what a column just
/// declared for it: `items[start..]`.
///
/// `BEFORE_DECLARATION` goes before those items and `AFTER_DECLARATION`
/// after them. `BEGIN_DECLARATION` and `END_DECLARATION` go inside the
/// first of them that [`Carrier::take_into_body`] accepts for `name`; when
/// none does — an enumeration, an alias, a column whose declaration has
/// no body to hold raw lines — they land adjacent to the declaration,
/// `BEGIN` before it and `END` after it. Nothing is dropped.
pub fn weave<D: Carrier>(slots: Slots, name: &str, start: usize, items: &mut Vec<D>) {
    weave_with(slots, start, items, D::verbatim, |declared, begin, end| {
        for item in declared {
            if item.take_into_body(name, begin, end) {
                return;
            }
        }
    });
}

/// [`weave`] for a column whose list holds something other than its
/// declaration type — a declaration kept with its module, say — or that
/// chooses among several bodies: `make` builds one raw-text item, and
/// `into_body` moves what it can place inside a body out of `begin` and
/// `end`. What it leaves lands adjacent.
pub fn weave_with<D>(
    slots: Slots,
    start: usize,
    items: &mut Vec<D>,
    make: impl Fn(String) -> D,
    into_body: impl FnOnce(&mut [D], &mut Vec<String>, &mut Vec<String>),
) {
    if slots.before.is_empty()
        && slots.begin.is_empty()
        && slots.end.is_empty()
        && slots.after.is_empty()
    {
        return;
    }
    let mut declared = items.split_off(start.min(items.len()));
    let mut begin = slots.begin;
    let mut end = slots.end;
    if !begin.is_empty() || !end.is_empty() {
        into_body(&mut declared, &mut begin, &mut end);
    }
    items.extend(slots.before.into_iter().map(&make));
    items.extend(begin.into_iter().map(&make));
    items.append(&mut declared);
    items.extend(end.into_iter().map(&make));
    items.extend(slots.after.into_iter().map(&make));
}

/// Put the model's `BEGIN_FILE` text before `items` and its `END_FILE`
/// text after them.
pub fn weave_file<D: Carrier>(ast: &Ast, aliases: &[&str], items: Vec<D>) -> Vec<D> {
    let (begin, end) = file_slots(ast, aliases);
    if begin.is_empty() && end.is_empty() {
        return items;
    }
    let mut out: Vec<D> = begin.into_iter().map(D::verbatim).collect();
    out.extend(items);
    out.extend(end.into_iter().map(D::verbatim));
    out
}

/// Sort one node's directives into `slots`.
///
/// `hoist` is the member case: the declaration-relative placements all
/// land in the aggregate's `begin` slot, and the file-relative ones are
/// **not** taken — [`file_slots`] owns those, and taking them twice
/// would write the text twice.
fn gather(facets: &zerodds_ast::Facets, aliases: &[&str], slots: &mut Slots, hoist: bool) {
    let Some(xtypes) = facets.xtypes.as_ref() else {
        return;
    };
    for directive in &xtypes.verbatim {
        if !language_matches(&directive.language, aliases) {
            continue;
        }
        match directive.placement {
            VerbatimPlacement::BeginFile | VerbatimPlacement::EndFile if hoist => {}
            _ if hoist => slots.begin.push(directive.text.clone()),
            placement => slots.push(placement, &directive.text),
        }
    }
}

/// The spellings the column for `language` answers to.
#[must_use]
pub fn aliases_of(language: zerodds_lang_cst::Language) -> &'static [&'static str] {
    use zerodds_lang_cst::Language;
    match language {
        Language::C => aliases::C,
        Language::Cpp => aliases::CPP,
        Language::Java => aliases::JAVA,
        Language::CSharp => aliases::CSHARP,
        Language::Ada => aliases::ADA,
        Language::Ada83 => aliases::ADA83,
        Language::D => aliases::D,
        Language::Elixir => aliases::ELIXIR,
        Language::FSharp => aliases::FSHARP,
        Language::Go => aliases::GO,
        Language::Julia => aliases::JULIA,
        Language::Kotlin => aliases::KOTLIN,
        Language::Lua => aliases::LUA,
        Language::Nim => aliases::NIM,
        Language::Node => aliases::NODE,
        Language::OCaml => aliases::OCAML,
        Language::Python => aliases::PYTHON,
        Language::Rust => aliases::RUST,
        Language::Swift => aliases::SWIFT,
        Language::TypeScript => aliases::TYPESCRIPT,
        Language::Zig => aliases::ZIG,
    }
}

/// The spellings each backend answers to.
///
/// §8.3.5.1's own values are `"c"`, `"c++"` and `"java"`; the rest are
/// the spellings other generators accept for the same languages, so a
/// file written against one of them keeps its meaning here.
pub mod aliases {
    /// The C column.
    pub const C: &[&str] = &["c"];
    /// The C++ column.
    pub const CPP: &[&str] = &["c++", "cpp", "cxx"];
    /// The Java column.
    pub const JAVA: &[&str] = &["java"];
    /// The C# column.
    pub const CSHARP: &[&str] = &["c#", "csharp", "cs"];
    /// The Ada column (Ada 2012 and later).
    pub const ADA: &[&str] = &["ada"];
    /// The Ada 83 column: its own name, because Ada 2012 text written for
    /// the other column need not compile under `-gnat83`.
    pub const ADA83: &[&str] = &["ada83", "ada-83"];
    /// The D column.
    pub const D: &[&str] = &["d", "dlang"];
    /// The Elixir column.
    pub const ELIXIR: &[&str] = &["elixir", "ex"];
    /// The F# column.
    pub const FSHARP: &[&str] = &["f#", "fsharp", "fs"];
    /// The Go column.
    pub const GO: &[&str] = &["go", "golang"];
    /// The Julia column.
    pub const JULIA: &[&str] = &["julia", "jl"];
    /// The Kotlin column.
    pub const KOTLIN: &[&str] = &["kotlin", "kt"];
    /// The Lua column.
    pub const LUA: &[&str] = &["lua"];
    /// The Nim column.
    pub const NIM: &[&str] = &["nim"];
    /// The Node column, which writes JavaScript.
    pub const NODE: &[&str] = &["javascript", "js", "node"];
    /// The OCaml column.
    pub const OCAML: &[&str] = &["ocaml", "ml"];
    /// The Python column.
    pub const PYTHON: &[&str] = &["python", "py"];
    /// The Rust column.
    pub const RUST: &[&str] = &["rust", "rs"];
    /// The Swift column.
    pub const SWIFT: &[&str] = &["swift"];
    /// The TypeScript column.
    pub const TYPESCRIPT: &[&str] = &["typescript", "ts"];
    /// The Zig column.
    pub const ZIG: &[&str] = &["zig"];
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn the_wildcard_concerns_every_backend_and_names_compare_caselessly() {
        assert!(language_matches("*", aliases::C));
        assert!(language_matches("C++", aliases::CPP));
        assert!(language_matches("CPP", aliases::CPP));
        assert!(language_matches("c#", aliases::CSHARP));
        assert!(!language_matches("java", aliases::CPP));
        assert!(!language_matches("rust", aliases::JAVA));
    }

    fn model(idl: &str) -> Ast {
        let parsed = zerodds_idl_parse::parse(idl);
        assert!(
            parsed.diagnostics().is_empty(),
            "parse: {:?}",
            parsed.diagnostics()
        );
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "lower: {:?}",
            lowered.diagnostics
        );
        let mut ast = lowered.ast;
        ast.resolve_all_by_name().expect("resolve");
        ast
    }

    fn type_id(ast: &Ast, name: &str) -> Id<TypeNode> {
        ast.types()
            .find(|(_, node)| node.name == name)
            .map(|(id, _)| id)
            .expect("the type is in the model")
    }

    #[test]
    fn every_placement_lands_in_its_own_slot() {
        let ast = model(
            "@verbatim(language = \"c\", placement = BEGIN_FILE, text = \"#include <a.h>\")
             @verbatim(language = \"c\", placement = BEFORE_DECLARATION, text = \"/* b */\")
             @verbatim(language = \"c\", placement = BEGIN_DECLARATION, text = \"/* c */\")
             @verbatim(language = \"c\", placement = END_DECLARATION, text = \"/* d */\")
             @verbatim(language = \"c\", placement = AFTER_DECLARATION, text = \"/* e */\")
             @verbatim(language = \"c\", placement = END_FILE, text = \"/* f */\")
             struct S { long x; };",
        );
        let slots = declaration_slots(&ast, type_id(&ast, "S"), aliases::C);
        assert_eq!(slots.before, vec!["/* b */"]);
        assert_eq!(slots.begin, vec!["/* c */"]);
        assert_eq!(slots.end, vec!["/* d */"]);
        assert_eq!(slots.after, vec!["/* e */"]);
        // File-relative placements belong to `file_slots`, and
        // `declaration_slots` reports them too so a single-declaration
        // caller sees everything the declaration said.
        assert_eq!(slots.begin_file, vec!["#include <a.h>"]);
        assert_eq!(slots.end_file, vec!["/* f */"]);
        let (begin, end) = file_slots(&ast, aliases::C);
        assert_eq!(begin, vec!["#include <a.h>"]);
        assert_eq!(end, vec!["/* f */"]);
    }

    #[test]
    fn a_directive_for_another_language_is_left_to_that_backend() {
        let ast = model(
            "@verbatim(language = \"java\", text = \"// java only\")
             @verbatim(text = \"// everyone\")
             struct S { long x; };",
        );
        let slots = declaration_slots(&ast, type_id(&ast, "S"), aliases::C);
        assert_eq!(slots.before, vec!["// everyone"]);
        let slots = declaration_slots(&ast, type_id(&ast, "S"), aliases::JAVA);
        assert_eq!(slots.before, vec!["// java only", "// everyone"]);
    }

    #[test]
    fn a_member_directive_is_hoisted_into_the_aggregate_and_taken_once() {
        let ast = model(
            "struct S {
                 @verbatim(language = \"c++\", placement = BEFORE_DECLARATION, text = \"// m\")
                 long x;
             };",
        );
        let slots = declaration_slots(&ast, type_id(&ast, "S"), aliases::CPP);
        assert_eq!(slots.begin, vec!["// m"]);
        assert!(slots.before.is_empty());
        let (begin, end) = file_slots(&ast, aliases::CPP);
        assert!(begin.is_empty() && end.is_empty());
    }
}
