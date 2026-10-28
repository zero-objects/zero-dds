// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim` through every layer, for the four columns that emit it.
//!
//! OMG IDL 4.2 §8.3.5.1: the annotation's `text` is copied into the
//! generated artifact unchanged, at the place its `placement` names,
//! for the backends its `language` names. Parse, lower, adapt, apply,
//! materialise, render — the same chain `c_end_to_end.rs` walks — and
//! then assert on the rendered text, because the placement is a
//! property of the output and only the output can show it.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_lang_rules::{ModelGraph, apply, rule};

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

/// IDL in, the C declaration list out, rendered.
fn c(idl: &str) -> String {
    let ast = model(idl);
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::c::catalogue()).expect("apply");
    let decls = zerodds_lang_rules::adapter::c::c_items(&ast, &applied).expect("materialise");
    zerodds_lang_render::c::file(&zerodds_lang_cst::c::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        guard: None,
        includes: Vec::new(),
        decls,
    })
}

/// IDL in, the C++ declaration list out, rendered.
fn cpp(idl: &str) -> String {
    let ast = model(idl);
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::cpp::catalogue()).expect("apply");
    let decls = zerodds_lang_rules::adapter::cpp::cpp_items(&ast, &applied).expect("materialise");
    zerodds_lang_render::cpp::file(&zerodds_lang_cst::cpp::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        pragma_once: false,
        includes: Vec::new(),
        decls,
    })
}

/// IDL in, the C# file out, rendered.
fn csharp(idl: &str) -> String {
    let ast = model(idl);
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::csharp::catalogue()).expect("apply");
    let file =
        zerodds_lang_rules::adapter::csharp::csharp_file(&ast, &applied).expect("materialise");
    zerodds_lang_render::csharp::file(&file)
}

/// IDL in, the Java file out, rendered.
fn java(idl: &str) -> String {
    // the assertions read the IDL-to-Java shape (public fields)
    zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        || java_omg(idl),
    )
}

fn java_omg(idl: &str) -> String {
    let ast = model(idl);
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::java::catalogue()).expect("apply");
    let types = zerodds_lang_rules::adapter::java::java_items(&ast, &applied).expect("materialise");
    zerodds_lang_render::java::file(&zerodds_lang_cst::java::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        package: None,
        imports: Vec::new(),
        types,
    })
}

/// The position of `needle` in `haystack`, asserted present.
fn at(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("`{needle}` is not in the output:\n{haystack}"))
}

/// All six placements on one struct, in the C++ column.
const SIX: &str = r##"
    @verbatim(language = "c++", placement = BEGIN_FILE, text = "#include <chrono> // vb-begin-file")
    @verbatim(language = "c++", placement = BEFORE_DECLARATION, text = "// vb-before")
    @verbatim(language = "c++", placement = BEGIN_DECLARATION, text = "// vb-body-open")
    @verbatim(language = "c++", placement = END_DECLARATION, text = "// vb-body-close")
    @verbatim(language = "c++", placement = AFTER_DECLARATION, text = "// vb-after")
    @verbatim(language = "c++", placement = END_FILE, text = "// vb-end-file")
    struct Pose { long x; };
"##;

#[test]
fn cpp_places_all_six_slots_in_spec_order() {
    let generated = cpp(SIX);
    let begin_file = at(&generated, "#include <chrono> // vb-begin-file");
    let before = at(&generated, "// vb-before");
    let opening = at(&generated, "struct Pose {");
    let begin = at(&generated, "// vb-body-open");
    let x = at(&generated, "std::int32_t x = {};");
    let end = at(&generated, "// vb-body-close");
    // At the start of a line: a member's `= {};` holds the same two
    // characters.
    let closing = at(&generated, "\n};");
    let after = at(&generated, "// vb-after");
    let end_file = at(&generated, "// vb-end-file");
    assert!(begin_file < before, "BEGIN_FILE is not first:\n{generated}");
    assert!(
        before < opening,
        "BEFORE_DECLARATION is not before the struct:\n{generated}"
    );
    assert!(
        opening < begin && begin < x,
        "BEGIN_DECLARATION is not at the body's start:\n{generated}"
    );
    assert!(
        x < end && end < closing,
        "END_DECLARATION is not at the body's end:\n{generated}"
    );
    assert!(
        closing < after,
        "AFTER_DECLARATION is not after the struct:\n{generated}"
    );
    assert!(after < end_file, "END_FILE is not last:\n{generated}");
    assert!(
        !generated.contains("// // vb"),
        "the text grew a comment marker:\n{generated}"
    );
}

#[test]
fn cpp_answers_to_its_alias_spellings_case_insensitively() {
    for language in ["c++", "cpp", "CXX", "C++"] {
        let generated = cpp(&format!(
            "@verbatim(language = \"{language}\", text = \"// vb-alias\")\nstruct Pose {{ long x; }};"
        ));
        assert!(
            generated.contains("// vb-alias"),
            "`{language}` did not match:\n{generated}"
        );
    }
}

#[test]
fn a_directive_for_an_unknown_language_is_not_emitted() {
    // Not an error: §8.3.5.1 lets a file carry text for another tool's
    // language. It is that backend's, and no one else's output changes.
    let generated =
        cpp("@verbatim(language = \"rust\", text = \"// vb-not-here\")\nstruct Pose { long x; };");
    assert!(!generated.contains("vb-not-here"), "{generated}");
    let generated =
        c("@verbatim(language = \"java\", text = \"// vb-not-here\")\nstruct Pose { long x; };");
    assert!(!generated.contains("vb-not-here"), "{generated}");
}

#[test]
fn the_wildcard_reaches_all_four_columns() {
    let idl = "@verbatim(text = \"// vb-everyone\")\nstruct Pose { long x; };";
    for generated in [c(idl), cpp(idl), csharp(idl), java(idl)] {
        assert!(generated.contains("// vb-everyone"), "{generated}");
    }
}

#[test]
fn repeated_directives_keep_their_declaration_order() {
    let generated = cpp("@verbatim(language = \"c++\", text = \"// vb-first\")
         @verbatim(language = \"c++\", text = \"// vb-second\")
         struct Pose { long x; };");
    assert!(
        at(&generated, "// vb-first") < at(&generated, "// vb-second"),
        "{generated}"
    );
}

#[test]
fn multi_line_text_arrives_line_by_line() {
    let generated = cpp(
        "@verbatim(language = \"c++\", placement = BEGIN_FILE, text = \"#ifdef VB\\n#endif\")
         struct Pose { long x; };",
    );
    assert!(generated.contains("#ifdef VB\n"), "{generated}");
    assert!(generated.contains("#endif\n"), "{generated}");
}

#[test]
fn c_places_the_file_slots_and_the_declaration_text_inside_the_struct() {
    let generated = c(
        "@verbatim(language = \"c\", placement = BEGIN_FILE, text = \"/* vb-begin-file */\")
         @verbatim(language = \"c\", placement = BEGIN_DECLARATION, text = \"/* vb-begin */\")
         @verbatim(language = \"c\", placement = END_DECLARATION, text = \"/* vb-end */\")
         @verbatim(language = \"c\", placement = AFTER_DECLARATION, text = \"/* vb-after */\")
         struct Pose { long x; };",
    );
    let begin_file = at(&generated, "/* vb-begin-file */");
    // The struct's member list opens with BEGIN_DECLARATION and closes
    // with END_DECLARATION.
    let body = at(&generated, "struct Pose {");
    let begin = at(&generated, "/* vb-begin */");
    let field = at(&generated, "int32_t x;");
    let end = at(&generated, "/* vb-end */");
    let after = at(&generated, "/* vb-after */");
    assert!(begin_file < body, "{generated}");
    assert!(body < begin && begin < field && field < end, "{generated}");
    assert!(end < after, "{generated}");
}

#[test]
fn csharp_puts_begin_and_end_inside_the_type_body() {
    let generated = csharp(
        "@verbatim(language = \"c#\", placement = BEGIN_DECLARATION, text = \"// vb-begin\")
         @verbatim(language = \"cs\", placement = END_DECLARATION, text = \"// vb-end\")
         struct Pose { long x; };",
    );
    // An IDL struct is a C# class, and the member is PascalCased —
    // the adapter's own mapping; the IdlName attribute keeps the
    // spelling.
    let opening = at(&generated, "class Pose");
    let begin = at(&generated, "// vb-begin");
    let field = at(&generated, "public int X;");
    let end = at(&generated, "// vb-end");
    assert!(
        opening < begin && begin < field && field < end,
        "{generated}"
    );
}

#[test]
fn java_wraps_the_file_and_fills_the_class_body() {
    let generated = java(
        "@verbatim(language = \"java\", placement = BEGIN_FILE, text = \"import java.time.Instant; // vb-file\")
         @verbatim(language = \"java\", placement = BEGIN_DECLARATION, text = \"// vb-begin\")
         @verbatim(language = \"java\", placement = END_DECLARATION, text = \"// vb-end\")
         struct Pose { long x; };",
    );
    let file = at(&generated, "import java.time.Instant; // vb-file");
    let outer = at(&generated, "class ZdTypes");
    let opening = at(&generated, "class Pose");
    let begin = at(&generated, "// vb-begin");
    let field = at(&generated, "public int x;");
    let end = at(&generated, "// vb-end");
    assert!(
        file < outer,
        "BEGIN_FILE is not outside the outer class:\n{generated}"
    );
    assert!(
        opening < begin && begin < field && field < end,
        "{generated}"
    );
}

#[test]
fn a_member_directive_is_hoisted_into_its_aggregate_not_dropped() {
    let generated = cpp("struct Pose {
             @verbatim(language = \"c++\", text = \"// vb-member\")
             long x;
         };");
    let opening = at(&generated, "struct Pose {");
    let member = at(&generated, "// vb-member");
    let field = at(&generated, "std::int32_t x = {};");
    assert!(opening < member && member < field, "{generated}");
}

#[test]
fn an_enum_cannot_hold_text_inside_so_it_lands_adjacent() {
    let generated = cpp(
        "@verbatim(language = \"c++\", placement = END_DECLARATION, text = \"// vb-enum-end\")
         enum Colour { RED, GREEN };",
    );
    let closing = at(&generated, "};");
    let end = at(&generated, "// vb-enum-end");
    assert!(closing < end, "{generated}");
}
