// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a column needs before `src/emit/<lang>.rs` can be written, and
//! which columns have it.
//!
//! `docs/language-columns-status.md` records a decision: seven of the
//! seventeen columns in `Language::ALL` are types columns and get no
//! codec backend in this pass, because three parts a backend rests on do
//! not exist for them. A decision written only in prose goes stale
//! silently — the day somebody gives a type-only CST a statement layer,
//! the paragraph explaining that it has none is wrong and nothing says
//! so. Go, D and Swift are the measured cases: each grew the three
//! parts and moved from [`TYPE_ONLY`] to [`COMPLETE`], through gate 3
//! failing first.
//!
//! So the decision is measured here instead, and the document quotes the
//! measurement. Three gates:
//!
//! 1. **Every operation the two shared walks emit exists in a runtime.**
//!    This is the size of what a new column's runtime owes, counted
//!    rather than estimated, and it is also a live guard on the walks: an
//!    operation added to `emit/encode.rs` that no runtime publishes is a
//!    generated call to a function that does not exist, and `cc` finds it
//!    in the probe rather than here. Asked twice: of `runtime/c`, which
//!    owes every one of the 66 whether a case reaches it or not, and of
//!    all twenty runtimes, of the names their own column actually writes
//!    — see [`every_runtime_publishes_the_names_its_column_writes`].
//! 2. **A codec backend never exists without the three parts it needs.**
//!    Seven columns have one and each of the seven has all three. An
//!    eighth that appeared without them would generate source with
//!    nothing to render it, nothing to link against, and no answer to
//!    Table 9.
//! 3. **The type-only columns are missing at least one of the three,
//!    each named.**
//!    The ledger. When a column stops being missing anything, this fails
//!    and says which document to move it in.
//!
//! ## Each gate rejects a deliberate error
//!
//! A gate that accepts everything is not a gate, which is the standard
//! the `<lang>_end_to_end.rs` compile gates already hold themselves to.
//! Each of the three was mutated once and seen to fail:
//!
//! - raising the floor in gate 1 to 600 reports "the scrape found only
//!   66 operations", which is both the refusal and the count;
//! - looking for `zdw_absent_<op>` instead of `zdw_<op>` reports the
//!   operations `runtime/c` does not publish;
//! - gate 1's twenty-runtime half reported the three names
//!   `runtime/kotlin/ZdWire.kt` did not publish before they were written,
//!   and reports them again with the three `fun` declarations taken back
//!   out;
//! - putting `c` into [`TYPE_ONLY`] reports `["c"] now has every part a codec
//!   backend rests on`, which is gate 3 seeing a column that has stopped
//!   being missing anything.
//!
//! ## Keyed on paths, deliberately
//!
//! The same way the guards named in `src/emit/mod.rs` are: what is being
//! checked is that a *file* exists, and a check that searched the tree
//! for its contents would pass on a copy in a test fixture.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::common::Language;

/// The repository root, from this crate's manifest.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

/// The columns with a codec: the seven that were rebuilt first, and
/// the ones that grew their three parts — a CST statement layer, a
/// `runtime/<lang>` and a `default_<lang>.rs` — and moved out of the
/// type-only tier: Go first, D after it, then Swift, Lua, Ada, Nim,
/// Zig, Julia, OCaml and Elixir, then Ada 83, Kotlin, Node and F#.
///
/// Node is the one column here with no `rule::node` catalogue: its
/// materialiser (`crates/lang-rules/src/adapter/node.rs`) declares
/// almost nothing for that system to track, because a struct, union,
/// bitset or valuetype is a plain object literal wherever it is built
/// and never a named declaration. The three parts this gate asks for
/// are a different question, and Node has all three.
const COMPLETE: [&str; 21] = [
    "ada",
    "ada83",
    "c",
    "cpp",
    "csharp",
    "d",
    "elixir",
    "fsharp",
    "go",
    "java",
    "julia",
    "kotlin",
    "lua",
    "nim",
    "node",
    "ocaml",
    "python",
    "rust",
    "swift",
    "typescript",
    "zig",
];

/// The columns that came back as types and stayed there.
///
/// Empty since Elixir, the last of them, grew its three parts. The tier
/// is kept rather than deleted because it is the shape the ledger is
/// written in and the next column added starts in it; and gate 3, which
/// reads it, does not go vacuous when it is empty — see
/// [`the_type_only_columns_are_each_missing_a_part_a_codec_backend_rests_on`].
const TYPE_ONLY: [&str; 0] = [];

/// The operations `emit/encode.rs` and `emit/decode.rs` name.
///
/// Every one is a string literal in those two files — the walks build no
/// operation name with `format!`, which is what makes scraping them
/// complete rather than a sample. `write_u32` and not `zdw_write_u32`:
/// the prefix is [`zerodds_lang_rules::emit::CodecTarget::runtime`]'s to
/// add.
fn operations_the_walks_emit() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for file in ["src/emit/encode.rs", "src/emit/decode.rs"] {
        let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(file))
            .unwrap_or_else(|why| panic!("{file}: {why}"));
        for literal in text.split('"').skip(1).step_by(2) {
            if is_operation(literal) {
                out.insert(literal.to_string());
            }
        }
    }
    out
}

/// Whether a string literal in a walk names a runtime operation.
///
/// The five verbs the runtime's surface is spelled with, plus the three
/// free functions that carry no verb — `strcmp`, and the two length
/// helpers a `wstring` needs. A literal that is a bare receiver (`"w"`,
/// `"r"`) or a local's name (`"zd_h"`) matches none of them.
fn is_operation(literal: &str) -> bool {
    const FREE: [&str; 5] = [
        "strcmp",
        "utf8_octets",
        "utf16_units",
        "no_dheader",
        "pid_is_user_member",
    ];
    const VERBS: [&str; 4] = ["write_", "read_", "reader_", "finish_"];
    FREE.contains(&literal)
        || (VERBS.iter().any(|verb| literal.starts_with(verb))
            && literal
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
}

/// Whether this column's CST can express a function body.
///
/// A `CodecTarget` backend's `Stmt`, `Block` and `Decl` come from
/// `zerodds_lang_cst::<lang>`, and a codec is statements. A CST that
/// declares types and nothing else has no node for a single line of one.
///
/// A `Block` **struct** is not the property; holding a sequence of
/// statements is. This asked for both and Python failed it — Python has
/// no braced block, its suite is the indented `Vec<Statement>` inside
/// `Function`, and `emit/python.rs` measures 374 of 374 corpus cases in
/// both directions through exactly that shape. A gate written from the
/// C family and applied to every column reports the one language whose
/// grammar differs, and says "no statements" about a CST whose
/// `Statement` enum has thirty variants.
fn cst_has_a_statement_layer(root: &Path, language: &str) -> bool {
    let text = std::fs::read_to_string(
        root.join("crates/lang-cst/src")
            .join(language)
            .join("mod.rs"),
    )
    .unwrap_or_else(|why| panic!("the {language} CST: {why}"));
    // `Stat` is Lua's own grammar word — the reference manual's *stat*
    // — the same way Python's suite is a `Vec<Statement>` without a
    // `Block` struct: the gate takes each language's spelling rather
    // than reporting "no statements" about an enum that holds them.
    let has_statements = text.lines().any(|line| {
        line == "pub enum Statement {" || line == "pub enum Stmt {" || line == "pub enum Stat {"
    });
    // Either a named block type, or a field that carries the sequence
    // directly — both are a body, and which one a language takes is its
    // grammar's business and not this gate's.
    let can_group = text.lines().any(|line| {
        line == "pub struct Block {"
            || line.contains("Vec<Statement>")
            || line.contains("Vec<Stmt>")
            || line.contains("Vec<Stat>")
    });
    has_statements && can_group
}

/// Whether a wire runtime exists for this column.
///
/// What the generated codec calls. `runtime/rust` is absent and Rust is
/// not a counter-example: its runtime is the `zerodds-wire` crate, and
/// its codec is `src/encode.rs` rather than a `CodecTarget` backend —
/// which is why [`a_codec_backend_never_exists_without_the_three_parts_it_needs`]
/// asks only of the columns that have a backend.
fn runtime_exists(root: &Path, language: &str) -> bool {
    // "ada83" is one exception: `runtime/ada-83`, hyphenated to match
    // `endpoints/ada-83`'s own naming — a Rust module cannot carry the
    // hyphen the directory does, so the two names differ on purpose
    // and not by drift.
    let dir = if language == "ada83" {
        "ada-83"
    } else {
        language
    };
    root.join("runtime").join(dir).is_dir()
}

/// Whether this column answers DDS-XTypes 1.3 Table 9.
///
/// What a member the stream did not carry takes. A reader without it has
/// no defined value to put at five kinds of position; see
/// `docs/language-columns-status.md`.
fn table_9_exists(root: &Path, language: &str) -> bool {
    root.join("crates/lang-rules/src")
        .join(format!("default_{language}.rs"))
        .is_file()
}

/// Whether this column has a `CodecTarget` backend.
fn backend_exists(root: &Path, language: &str) -> bool {
    root.join("crates/lang-rules/src/emit")
        .join(format!("{language}.rs"))
        .is_file()
}

#[test]
fn the_operations_the_walks_emit_all_exist_in_the_reference_runtime() {
    // `runtime/c` is the reference because the C column is the one both
    // walks were shaped from, and because it is the only runtime that
    // has generated both directions over the whole corpus. Every
    // operation is published as `zdw_<name>` in its headers.
    //
    // Two things fail here. A walk that gained an operation no runtime
    // implements — which would otherwise be a link error in a probe, one
    // build directory away from the walk that caused it. And a runtime
    // that dropped one.
    let operations = operations_the_walks_emit();
    // A floor, so a scrape that stopped matching cannot pass by finding
    // nothing. 66 at the time this was written; the floor is what stops
    // the gate going vacuously green, not a count to keep in step.
    assert!(
        operations.len() >= 60,
        "the scrape found only {} operations, which is fewer than the walks emit; \
         `is_operation` has stopped matching",
        operations.len()
    );

    let headers = std::fs::read_dir(root().join("runtime/c/include/zerodds"))
        .or_else(|_| std::fs::read_dir(root().join("runtime/c/include")))
        .expect("the C runtime's headers");
    let mut published = String::new();
    for entry in headers {
        let path = entry.expect("a header").path();
        if path.extension().is_some_and(|e| e == "h") {
            published.push_str(&std::fs::read_to_string(&path).expect("a header's text"));
        }
    }

    let missing: Vec<&String> = operations
        .iter()
        // `strcmp` is C's own and carries no runtime prefix. A target
        // whose strings compare with an operator emits no call at all —
        // see `CodecTarget::call`.
        .filter(|op| op.as_str() != "strcmp")
        .filter(|op| !published.contains(&format!("zdw_{op}")))
        .collect();
    assert!(
        missing.is_empty(),
        "the walks emit operations `runtime/c` does not publish: {missing:?}"
    );
}

#[test]
fn a_codec_backend_never_exists_without_the_three_parts_it_needs() {
    // The invariant behind the decision recorded in
    // `docs/language-columns-status.md`: a `CodecTarget` backend is the
    // last of four parts, not the first. It builds `lang_cst` statements,
    // so the CST needs a statement layer; the source it generates calls a
    // runtime, so one has to exist; and its reader has to answer Table 9,
    // so a defaulter has to exist.
    //
    // Seven columns have a backend and all seven have all three. This is
    // what would fail if a sixth arrived without them — generated source
    // with no node to build it from, no library to link against, and no
    // defined value where the stream carried nothing.
    let root = root();
    let mut faults = Vec::new();
    for language in COMPLETE.iter().chain(TYPE_ONLY.iter()) {
        if !backend_exists(&root, language) {
            continue;
        }
        if !cst_has_a_statement_layer(&root, language) {
            faults.push(format!(
                "{language}: emit/{language}.rs, but the CST has no statements"
            ));
        }
        if !runtime_exists(&root, language) {
            faults.push(format!(
                "{language}: emit/{language}.rs, but no runtime/{language}"
            ));
        }
        if !table_9_exists(&root, language) {
            faults.push(format!(
                "{language}: emit/{language}.rs, but no default_{language}.rs"
            ));
        }
    }
    assert!(faults.is_empty(), "{}", faults.join("\n"));

    // And the count, so the gate cannot pass by finding no backends at
    // all. C, C++, C#, Java, TypeScript.
    let with_backend = COMPLETE
        .iter()
        .chain(TYPE_ONLY.iter())
        .filter(|language| backend_exists(&root, language))
        .count();
    assert!(
        with_backend >= 5,
        "only {with_backend} columns have a codec backend; this gate has stopped finding them"
    );
}

#[test]
fn the_type_only_columns_are_each_missing_a_part_a_codec_backend_rests_on() {
    // The ledger `docs/language-columns-status.md` quotes. Each column
    // in the tier is missing at least one of the three, so the decision
    // not to write `emit/<lang>.rs` for it in this pass has a named
    // cause rather than a preference.
    //
    // This fails when a column stops being missing anything, which is
    // the point: at that moment the document's reason for that column no
    // longer holds and the column belongs in a different tier.
    //
    // The tier is empty since Elixir moved out of it, so the two loops
    // below range over nothing. That is why the third check exists: it
    // reads the tree rather than the list, and it is what keeps this
    // gate from passing on an empty tier by saying nothing.
    let root = root();
    let mut settled = Vec::new();
    for language in TYPE_ONLY {
        let missing: Vec<&str> = [
            (!cst_has_a_statement_layer(&root, language)).then_some("a CST statement layer"),
            (!runtime_exists(&root, language)).then_some("a wire runtime"),
            (!table_9_exists(&root, language)).then_some("Table 9 defaulters"),
        ]
        .into_iter()
        .flatten()
        .collect();
        if missing.is_empty() {
            settled.push(language);
        }
    }
    assert!(
        settled.is_empty(),
        "{settled:?} now has every part a codec backend rests on. \
         Write `crates/lang-rules/src/emit/<lang>.rs` for it, or move it out of the \
         type-only tier in `docs/language-columns-status.md` — the reason recorded \
         there for that column has stopped being true."
    );

    // And none of the type-only columns has a backend, which is the
    // state the document describes. A column that grew one is not a
    // failure of the tree, it is a ledger that has not been updated.
    let built: Vec<&str> = TYPE_ONLY
        .into_iter()
        .filter(|language| backend_exists(&root, language))
        .collect();
    assert!(
        built.is_empty(),
        "{built:?} now has a codec backend. Move it out of the type-only tier in \
         `docs/language-columns-status.md`; the tier list there says it has none."
    );

    // Read from the tree, so the two tiers together are the whole set of
    // columns and not a subset someone forgot to extend. Every directory
    // under `crates/lang-cst/src` is a column — the two files there,
    // `lib.rs` and `common.rs`, are not — and a column named in neither
    // list is one this gate never looked at.
    let mut unlisted = Vec::new();
    for entry in std::fs::read_dir(root.join("crates/lang-cst/src")).expect("the CST columns") {
        let path = entry.expect("a CST entry").path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a column's directory name")
            .to_string();
        if !COMPLETE.contains(&name.as_str()) && !TYPE_ONLY.contains(&name.as_str()) {
            unlisted.push(name);
        }
    }
    unlisted.sort();
    assert!(
        unlisted.is_empty(),
        "{unlisted:?}: a CST column in neither tier. Add it to `COMPLETE` or to \
         `TYPE_ONLY` and to `docs/language-columns-status.md` — until then no gate \
         in this file has looked at it."
    );
}

// ---------------------------------------------------------------------
// Gate 1, over every runtime and not one.
// ---------------------------------------------------------------------

/// Where each column's wire runtime lives, under `runtime/`.
///
/// Twenty entries and not twenty-one: Rust has no `runtime/rust`,
/// because its codec calls `crates/wire` directly — the same exception
/// `crates/lang-rules/tests/every_language_has_its_parts.rs::NO_RUNTIME`
/// records, and the reason [`runtime_exists`] gives above.
///
/// The directory is named rather than derived: `ada83` is `runtime/ada-83`.
const RUNTIMES: [(Language, &str); 20] = [
    (Language::C, "c"),
    (Language::Cpp, "cpp"),
    (Language::CSharp, "csharp"),
    (Language::Java, "java"),
    (Language::Python, "python"),
    (Language::TypeScript, "typescript"),
    (Language::Go, "go"),
    (Language::OCaml, "ocaml"),
    (Language::Ada, "ada"),
    (Language::Ada83, "ada-83"),
    (Language::D, "d"),
    (Language::Elixir, "elixir"),
    (Language::Julia, "julia"),
    (Language::Lua, "lua"),
    (Language::Nim, "nim"),
    (Language::Swift, "swift"),
    (Language::Zig, "zig"),
    (Language::Node, "node"),
    (Language::FSharp, "fsharp"),
    (Language::Kotlin, "kotlin"),
];

/// The corpus cases the sweep generates from.
///
/// Not the whole corpus: 418 cases against twenty columns under five
/// encodings is a minute of generation for an answer these sixteen give
/// in three seconds. What the sixteen are chosen for is *operations* and
/// not constructs — between them they reach 59 of the 66 names the two
/// walks emit, which is what the floor in
/// [`every_runtime_publishes_the_names_its_column_writes`] holds them to.
/// A case is here because it is the only one that reaches an operation:
/// `wstring-unbounded-utf8` for `write_wstring_utf8`,
/// `wstring-bounded-utf8` for `utf16_units`, `string-bounded-utf8` for
/// `utf8_octets`, `struct-mutable` for the EMHEADER and PID arms,
/// `fixed-basic` for the decimal arm.
///
/// Five of the 66 are reached by no case here and by no column:
/// `reader_save`, `reader_restore`, `reader_clear_discard`,
/// `write_pid_extended` and `write_union_overlay`. The first three belong
/// to the counting pass a `map` takes when its count is omitted, and the
/// last two to a member id or a length no corpus case carries and to the
/// Cyclone union overlay — none of which
/// [`zerodds_lang_rules::generate::source`] can be made to emit, because
/// it fixes the dialect to `Specification` on purpose (see its own doc).
/// `runtime/c` is held to all 66 by
/// [`the_operations_the_walks_emit_all_exist_in_the_reference_runtime`];
/// for the other nineteen those five are unmeasured here and recorded in
/// `docs/OPEN-ITEMS.md`.
const OPERATION_CASES: [&str; 17] = [
    "combined-everything",
    "wstring-unbounded-utf8",
    "wstring-bounded-utf8",
    "string-bounded-utf8",
    "struct-mutable",
    "recursive-optional-external",
    "map-key-long",
    "bitset-basic",
    "bitmask-bitbound-16",
    "fixed-basic",
    "longdouble-framed",
    "union-discriminator-enum",
    "enum-sparse-values",
    "sequence-primitives-nonempty",
    "array-2d-primitives",
    "primitive-mixed",
    "struct-appendable",
];

/// The extensions a runtime's own source carries, per column.
///
/// A list rather than "every text file": `runtime/typescript` holds a
/// `dist-generated/` of compiler output and `runtime/swift` a `.build/`,
/// and a name found in either says nothing about what the runtime
/// publishes.
const RUNTIME_EXTENSIONS: [&str; 23] = [
    "c", "h", "hpp", "cpp", "cs", "java", "ts", "py", "go", "ml", "mli", "ads", "adb", "d", "ex",
    "jl", "lua", "nim", "swift", "zig", "js", "fs", "kt",
];

/// Directories under `runtime/<lang>` that are not the runtime.
const NOT_RUNTIME_SOURCE: [&str; 8] = [
    "dist-generated",
    "node_modules",
    "tests",
    "test",
    "_build",
    "deps",
    "obj",
    "bin",
];

/// An identifier with the case and the underscores taken out.
///
/// The one thing the twenty columns disagree about when they spell an
/// operation: `write_wstring_utf8` is `zdw_write_wstring_utf8` in C,
/// `WriteWstringUtf8` in F#, `writeWstringUtf8` in Kotlin and
/// `Write_Wstring_Utf8` in Ada. Normalising is what lets this gate ask
/// its question without carrying a second copy of each backend's naming
/// rule — the rule that decides the spelling stays in
/// `src/emit/<lang>.rs`, and the spelling itself is read out of what the
/// generator actually wrote.
fn normalised(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Every `[A-Za-z0-9_]+` run in `text`.
fn identifiers(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else if !current.is_empty() {
            out.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.insert(current);
    }
    out
}

/// The model of one corpus case, resolved.
fn model(idl: &str, origin: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{origin} does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), origin);
    assert!(
        lowered.diagnostics.is_empty(),
        "{origin} does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name()
        .unwrap_or_else(|why| panic!("{origin} does not resolve: {why:?}"));
    ast
}

/// Every identifier the generator writes for `language` over
/// [`OPERATION_CASES`], under all five encodings.
///
/// A case a column refuses is skipped and not a failure: which
/// constructs a column declines is
/// `<lang>_end_to_end.rs`'s question, and this gate asks only about the
/// cases that did render.
fn what_the_generator_writes(root: &Path, language: Language) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for case in OPERATION_CASES {
        let path = root
            .join("tests/interop-corpus")
            .join(case)
            .join("type.idl");
        let idl = std::fs::read_to_string(&path).unwrap_or_else(|why| panic!("{case}: {why}"));
        let ast = model(&idl, case);
        for encoding in Encoding::ALL {
            let key = LayoutKey::data(encoding, Endianness::Little);
            if let Ok(text) = zerodds_lang_rules::generate::source(&ast, language, key) {
                out.extend(identifiers(&text));
            }
        }
    }
    out
}

/// The text of one runtime's own source files.
fn runtime_source(root: &Path, directory: &str) -> String {
    let mut out = String::new();
    let mut stack = vec![root.join("runtime").join(directory)];
    while let Some(here) = stack.pop() {
        for entry in std::fs::read_dir(&here).unwrap_or_else(|why| panic!("{directory}: {why}")) {
            let path = entry.expect("a runtime entry").path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            if path.is_dir() {
                if !name.starts_with('.') && !NOT_RUNTIME_SOURCE.contains(&name.as_str()) {
                    stack.push(path);
                }
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| RUNTIME_EXTENSIONS.contains(&e))
            {
                out.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
                out.push('\n');
            }
        }
    }
    out
}

/// Whether `name` reaches a caller outside the runtime.
///
/// A coarse screen and named as one: a name whose every line in the
/// runtime carries one of the five words a language shuts a declaration
/// with is not one a generated file can call. It does not prove the
/// converse — a name mentioned inside a private body reads as public
/// here — and it does not have to: what the generated code needs is one
/// declaration it can reach, and the compile gates in
/// `<lang>_end_to_end.rs` are what say the whole file links.
fn published(source: &str, name: &str) -> bool {
    const SHUT: [&str; 5] = ["private", "fileprivate", "internal", "defp", "local"];
    source
        .lines()
        .filter(|line| identifiers(line).contains(name))
        .any(|line| {
            let words = identifiers(line);
            !SHUT.iter().any(|shut| words.contains(*shut))
        })
}

#[test]
fn every_runtime_publishes_the_names_its_column_writes() {
    // Gate 1 asked its question of `runtime/c` and of no other column,
    // and was green while `runtime/kotlin/ZdWire.kt` published neither
    // `writeWstringUtf8` nor `readWstringUtf8` nor `utf8Octets` — three
    // names `crates/lang-rules/src/emit/kotlin.rs` writes on every
    // `wstring` and every bounded `string`. Nineteen runtimes were
    // unchecked; this is the same question asked of all twenty.
    //
    // The question is asked of what the generator *wrote*, not of a
    // table of expected spellings: every identifier in the generated
    // source whose normalised form is an operation's — optionally
    // behind the one prefix a runtime adds — has to be an identifier
    // the runtime publishes, spelled exactly the same way.
    let root = root();
    let operations: Vec<(String, String)> = operations_the_walks_emit()
        .into_iter()
        // `strcmp` is C's own, and a column whose strings compare with
        // an operator emits no call for it at all. The same exception
        // the reference-runtime gate above makes, for the same reason.
        .filter(|op| op != "strcmp")
        .map(|op| {
            let normal = normalised(&op);
            (op, normal)
        })
        .collect();

    let mut faults: Vec<String> = Vec::new();
    let mut thin: Vec<String> = Vec::new();
    let results: Vec<(Language, Vec<String>, BTreeSet<String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = RUNTIMES
            .iter()
            .map(|(language, directory)| {
                let root = &root;
                let operations = &operations;
                scope.spawn(move || {
                    let written = what_the_generator_writes(root, *language);
                    let source = runtime_source(root, directory);
                    let names = identifiers(&source);
                    let mut missing = Vec::new();
                    let mut reached = BTreeSet::new();
                    for identifier in &written {
                        let normal = normalised(identifier);
                        for (operation, operation_normal) in operations {
                            let Some(prefix) = normal.strip_suffix(operation_normal.as_str())
                            else {
                                continue;
                            };
                            // The one thing a runtime may add in front of
                            // an operation's own name: C's single
                            // namespace (`zdw_read_u32`) and Go's package
                            // (`zdwire.Utf8Octets`). Anything else in
                            // front of it is a different name that
                            // happens to end the same way — TypeScript's
                            // `ZdReaderStatus` is a type, not a call to
                            // `reader_status`.
                            if !matches!(prefix, "" | "zdw" | "zdwire") {
                                continue;
                            }
                            if names.contains(identifier) && published(&source, identifier) {
                                reached.insert(operation.clone());
                            } else {
                                missing.push(format!("{operation} as `{identifier}`"));
                            }
                        }
                    }
                    (*language, missing, reached)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a column's sweep"))
            .collect()
    });

    for (language, missing, reached) in results {
        if !missing.is_empty() {
            faults.push(format!(
                "runtime/{} does not publish what the {} column writes: {}",
                language.as_str(),
                language.as_str(),
                missing.join(", ")
            ));
        }
        // A floor per column, so a column whose generation stopped
        // producing anything cannot pass by writing no operations at
        // all. Ada 83 is the thinnest at 47 of the 66 the walks emit.
        if reached.len() < 40 {
            thin.push(format!("{}: {}", language.as_str(), reached.len()));
        }
    }
    assert!(faults.is_empty(), "{}", faults.join("\n"));
    assert!(
        thin.is_empty(),
        "these columns reached fewer than 40 of the operations the walks emit, \
         which is a sweep that has stopped generating rather than a runtime that is \
         complete: {}",
        thin.join(", ")
    );
}
