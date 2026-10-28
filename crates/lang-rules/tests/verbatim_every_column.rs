// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim` in every column, through the two entry points a generator
//! calls: `generate::declarations` and `generate::source`.
//!
//! OMG IDL 4.2 §8.3.5.1: the annotation's `text` is copied into the
//! generated artifact unchanged, at the place its `placement` names, for
//! the backends its `language` names. `verbatim_end_to_end.rs` walks the
//! four columns that wove it first through their adapters; this walks
//! all twenty-one through the generator, and asserts on the rendered text
//! because the placement is a property of the output.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;
use zerodds_lang_rules::generate;

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

/// Where a column's struct keeps `BEGIN_DECLARATION` and
/// `END_DECLARATION`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Body {
    /// Inside the body, around the members.
    Around,
    /// Inside a body written after the members — Kotlin's class body
    /// follows the primary constructor that holds them.
    After,
    /// Adjacent to the declaration: its body holds no raw line.
    Adjacent,
    /// The column declares nothing for a struct — Node's values are plain
    /// objects — so the text stands where the declaration would.
    Undeclared,
}

/// Every column: its `Language`, the `language` value a directive names
/// it by, the line comment it reads, and where its struct body is.
const COLUMNS: &[(Language, &str, &str, Body)] = &[
    (Language::C, "c", "//", Body::Around),
    (Language::Cpp, "c++", "//", Body::Around),
    (Language::CSharp, "c#", "//", Body::Around),
    (Language::Java, "java", "//", Body::Around),
    (Language::Ada, "ada", "--", Body::Around),
    (Language::Ada83, "ada83", "--", Body::Around),
    (Language::D, "d", "//", Body::Around),
    (Language::Elixir, "elixir", "#", Body::Around),
    (Language::FSharp, "f#", "//", Body::Around),
    (Language::Go, "go", "//", Body::Around),
    (Language::Julia, "julia", "#", Body::Around),
    (Language::Kotlin, "kotlin", "//", Body::After),
    (Language::Lua, "lua", "--", Body::Adjacent),
    (Language::Nim, "nim", "#", Body::Around),
    (Language::Node, "javascript", "//", Body::Undeclared),
    (Language::OCaml, "ocaml", "(**)", Body::Around),
    (Language::Python, "python", "#", Body::Around),
    (Language::Rust, "rust", "//", Body::Around),
    (Language::Swift, "swift", "//", Body::Around),
    (Language::TypeScript, "typescript", "//", Body::Around),
    (Language::Zig, "zig", "//", Body::Around),
];

/// One struct carrying all six placements for `language`, each text a
/// comment in `comment` naming its slot.
fn six(language: &str, comment: &str) -> String {
    let line = |slot: &str| {
        if comment == "(**)" {
            format!("(* vb-{slot} *)")
        } else {
            format!("{comment} vb-{slot}")
        }
    };
    format!(
        r#"
        @verbatim(language = "{language}", placement = BEGIN_FILE, text = "{begin_file}")
        @verbatim(language = "{language}", placement = BEFORE_DECLARATION, text = "{before}")
        @verbatim(language = "{language}", placement = BEGIN_DECLARATION, text = "{begin}")
        @verbatim(language = "{language}", placement = END_DECLARATION, text = "{end}")
        @verbatim(language = "{language}", placement = AFTER_DECLARATION, text = "{after}")
        @verbatim(language = "{language}", placement = END_FILE, text = "{end_file}")
        struct Pose {{ long zdmark; }};
        "#,
        begin_file = line("begin-file"),
        before = line("before"),
        begin = line("body-open"),
        end = line("body-close"),
        after = line("after"),
        end_file = line("end-file"),
    )
}

/// The position of `needle` in `haystack`.
fn at(haystack: &str, needle: &str) -> Result<usize, String> {
    haystack
        .find(needle)
        .ok_or_else(|| format!("`{needle}` is not in the output"))
}

/// The first position of `needle` at or after `from`, case folded.
fn after(haystack: &str, needle: &str, from: usize) -> Result<usize, String> {
    let folded = haystack.to_ascii_lowercase();
    folded[from..]
        .find(&needle.to_ascii_lowercase())
        .map(|offset| from + offset)
        .ok_or_else(|| format!("`{needle}` is not after the BEFORE_DECLARATION text"))
}

fn six_in_order(body: Body, generated: &str) -> Result<(), String> {
    let begin_file = at(generated, "vb-begin-file")?;
    let before = at(generated, "vb-before")?;
    let begin = at(generated, "vb-body-open")?;
    let end = at(generated, "vb-body-close")?;
    let after_ = at(generated, "vb-after")?;
    let end_file = at(generated, "vb-end-file")?;
    let order = |what: &str, a: usize, b: usize| {
        if a < b { Ok(()) } else { Err(what.to_string()) }
    };
    order("BEGIN_FILE before BEFORE_DECLARATION", begin_file, before)?;
    order("END_DECLARATION before AFTER_DECLARATION", end, after_)?;
    order("AFTER_DECLARATION before END_FILE", after_, end_file)?;
    if body == Body::Undeclared {
        order("BEFORE_DECLARATION before BEGIN_DECLARATION", before, begin)?;
        order("BEGIN_DECLARATION before END_DECLARATION", begin, end)?;
    } else {
        // The struct's own name, the first time it is written after the
        // BEFORE text, and its member.
        let name = after(generated, "Pose", before)?;
        let member = after(generated, "zdmark", before)?;
        match body {
            Body::Around => {
                order(
                    "the declaration opens before BEGIN_DECLARATION",
                    name,
                    begin,
                )?;
                order("BEGIN_DECLARATION before the member", begin, member)?;
                order("the member before END_DECLARATION", member, end)?;
            }
            Body::After => {
                order("the member before BEGIN_DECLARATION", member, begin)?;
                order("BEGIN_DECLARATION before END_DECLARATION", begin, end)?;
            }
            Body::Adjacent | Body::Undeclared => {
                order("BEFORE_DECLARATION before BEGIN_DECLARATION", before, begin)?;
                order("BEGIN_DECLARATION before the declaration", begin, name)?;
                order("the member before END_DECLARATION", member, end)?;
            }
        }
    }
    // Copied, not commented out a second time.
    if generated.contains("// // vb") || generated.contains("-- -- vb") {
        return Err("the text grew a comment marker".to_string());
    }
    Ok(())
}

/// Every failure across the columns at once, so one run names all of
/// them.
fn all_or_report(failures: Vec<String>) {
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn every_column_places_the_six_slots_in_its_declarations() {
    let mut failures = Vec::new();
    for (language, name, comment, body) in COLUMNS {
        let ast = model(&six(name, comment));
        match generate::declarations(&ast, *language) {
            Ok(generated) => {
                if let Err(why) = six_in_order(*body, &generated) {
                    failures.push(format!("{language:?}: {why}:\n{generated}"));
                }
            }
            Err(error) => failures.push(format!("{language:?}: {error:?}")),
        }
    }
    all_or_report(failures);
}

#[test]
fn every_column_ends_its_source_file_with_the_end_file_text() {
    // `generate::source` writes the codec after the declarations; the
    // `END_FILE` text belongs after it, and the `BEGIN_FILE` text before
    // everything the declarations hold.
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let mut failures = Vec::new();
    for (language, name, comment, body) in COLUMNS {
        let ast = model(&six(name, comment));
        let generated = match generate::source(&ast, *language, key) {
            Ok(generated) => generated,
            Err(error) => {
                failures.push(format!("{language:?}: {error:?}"));
                continue;
            }
        };
        if let Err(why) = six_in_order(*body, &generated) {
            failures.push(format!("{language:?}: {why}:\n{generated}"));
            continue;
        }
        let last = generated
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default();
        if !last.contains("vb-end-file") {
            failures.push(format!(
                "{language:?}: END_FILE is not the last line:\n{generated}"
            ));
        }
    }
    all_or_report(failures);
}

#[test]
fn a_directive_reaches_its_own_column_and_no_other() {
    for (language, name, comment, _) in COLUMNS {
        let marker = if *comment == "(**)" {
            "(* vb-mine *)".to_string()
        } else {
            format!("{comment} vb-mine")
        };
        let ast = model(&format!(
            "@verbatim(language = \"{name}\", text = \"{marker}\")\nstruct Pose {{ long zdmark; }};"
        ));
        for (other, ..) in COLUMNS {
            let generated = generate::declarations(&ast, *other)
                .unwrap_or_else(|error| panic!("{other:?}: {error:?}"));
            assert_eq!(
                generated.contains("vb-mine"),
                other == language,
                "a directive for {name} in the {other:?} column:\n{generated}"
            );
        }
    }
}

#[test]
fn the_wildcard_reaches_every_column() {
    let ast = model("@verbatim(text = \"// vb-everyone\")\nstruct Pose { long zdmark; };");
    for (language, ..) in COLUMNS {
        let generated = generate::declarations(&ast, *language)
            .unwrap_or_else(|error| panic!("{language:?}: {error:?}"));
        assert!(
            generated.contains("// vb-everyone"),
            "{language:?}:\n{generated}"
        );
    }
}

#[test]
fn ada_writes_its_file_text_outside_the_package() {
    // Context clauses stand before `package`; that is where an Ada file
    // starts, so it is where `BEGIN_FILE` goes.
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    for (language, name) in [(Language::Ada, "ada"), (Language::Ada83, "ada83")] {
        let ast = model(&format!(
            "@verbatim(language = \"{name}\", placement = BEGIN_FILE, text = \"with Interfaces; -- vb-with\")
             struct Pose {{ long zdmark; }};"
        ));
        let generated = generate::source(&ast, language, key)
            .unwrap_or_else(|error| panic!("{language:?}: {error:?}"));
        let with = at(&generated, "-- vb-with").expect("BEGIN_FILE text");
        let package = at(&generated, "package ").expect("the package");
        assert!(with < package, "{language:?}:\n{generated}");
    }
}
