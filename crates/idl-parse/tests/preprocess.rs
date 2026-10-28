// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The preprocessor and its source map. Spec §3.4.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl_cst::TokenKind;
use zerodds_idl_parse::{MemoryLoader, preprocess};

/// The kinds of the expanded stream, for asserting what survived.
fn kinds(source: &str, loader: &MemoryLoader) -> Vec<TokenKind> {
    preprocess("main.idl", source, loader)
        .tokens
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

/// The expanded text, for asserting what a branch produced.
fn text(source: &str) -> String {
    preprocess("main.idl", source, &MemoryLoader::new()).text
}

fn empty() -> MemoryLoader {
    MemoryLoader::new()
}

// -- define and undef ----------------------------------------------------

#[test]
fn a_defined_name_is_replaced_by_its_body() {
    let source = "#define MAX 64\nconst long N = MAX;\n";
    let expanded = text(source);
    assert!(expanded.contains("64"), "{expanded}");
    assert!(!expanded.contains("MAX"), "{expanded}");
}

#[test]
fn an_undefined_name_is_left_alone() {
    let source = "#define MAX 64\n#undef MAX\nconst long N = MAX;\n";
    let expanded = text(source);
    assert!(expanded.contains("MAX"), "{expanded}");
}

#[test]
fn a_redefinition_replaces_the_earlier_one() {
    let source = "#define X 1\n#define X 2\nconst long N = X;\n";
    let expanded = text(source);
    assert!(expanded.contains('2'));
    assert!(!expanded.contains('1'));
}

#[test]
fn a_define_without_a_name_is_diagnosed() {
    let result = preprocess("main.idl", "#define\n", &empty());
    assert!(!result.diagnostics.is_empty());
}

#[test]
fn a_macro_expanding_to_another_macro_is_rescanned() {
    // ISO/IEC 14882:2003 §16.3.4/1: the replacement list "is rescanned,
    // along with all subsequent preprocessing tokens of the source file,
    // for more macro names to replace". This used to be reported as a
    // limitation; it is the rule.
    let source = "#define A B\n#define B 1\nconst long N = A;\n";
    let result = preprocess("main.idl", source, &empty());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.text.contains("= 1"), "{}", result.text);
}

// -- conditionals --------------------------------------------------------

#[test]
fn ifdef_keeps_only_the_taken_branch() {
    let source = "#define ON 1\n#ifdef ON\nstruct Kept { long a; };\n#else\nstruct Dropped { long b; };\n#endif\n";
    let expanded = text(source);
    assert!(expanded.contains("Kept"), "{expanded}");
    assert!(!expanded.contains("Dropped"), "{expanded}");
}

#[test]
fn ifndef_is_the_other_way_round() {
    let source = "#ifndef ABSENT\nstruct Kept { long a; };\n#endif\n";
    assert!(text(source).contains("Kept"));

    let source = "#define PRESENT 1\n#ifndef PRESENT\nstruct Dropped { long a; };\n#endif\n";
    assert!(!text(source).contains("Dropped"));
}

#[test]
fn elif_takes_the_first_true_branch_and_no_later_one() {
    let source = r"
#define B 1
#ifdef A
struct First { long a; };
#elif defined(B)
struct Second { long b; };
#elif defined(B)
struct Third { long c; };
#else
struct Fourth { long d; };
#endif
";
    let expanded = text(source);
    assert!(expanded.contains("Second"), "{expanded}");
    assert!(!expanded.contains("First"));
    assert!(!expanded.contains("Third"), "a later branch was also taken");
    assert!(!expanded.contains("Fourth"));
}

#[test]
fn else_is_taken_only_when_nothing_before_it_was() {
    let source = "#define A 1\n#ifdef A\nstruct Kept { long a; };\n#else\nstruct Dropped { long b; };\n#endif\n";
    let expanded = text(source);
    assert!(expanded.contains("Kept"));
    assert!(!expanded.contains("Dropped"));
}

#[test]
fn conditionals_nest() {
    let source = r"
#define OUTER 1
#define INNER 1
#ifdef OUTER
  #ifdef INNER
struct Both { long a; };
  #endif
  #ifdef ABSENT
struct OnlyOuter { long b; };
  #endif
#endif
#ifdef ABSENT
  #ifdef INNER
struct Neither { long c; };
  #endif
#endif
";
    let expanded = text(source);
    assert!(expanded.contains("Both"), "{expanded}");
    assert!(!expanded.contains("OnlyOuter"));
    assert!(!expanded.contains("Neither"));
}

#[test]
fn a_define_inside_a_dropped_branch_does_not_take_effect() {
    let source = "#ifdef ABSENT\n#define SNEAKY 1\n#endif\n#ifdef SNEAKY\nstruct Dropped { long a; };\n#endif\n";
    assert!(!text(source).contains("Dropped"));
}

#[test]
fn an_if_with_a_literal_works_and_an_unevaluable_one_is_reported() {
    assert!(text("#if 1\nstruct Kept { long a; };\n#endif\n").contains("Kept"));
    assert!(!text("#if 0\nstruct Dropped { long a; };\n#endif\n").contains("Dropped"));

    // §16.1/4: an identifier left after replacement is 0, so the
    // branch is dropped — and each such name is reported as a warning,
    // because a misspelt macro takes exactly this path.
    let result = preprocess(
        "main.idl",
        "#if A + B > 2\nstruct S { long a; };\n#endif\n",
        &empty(),
    );
    assert!(!result.text.contains("struct"), "{}", result.text);
    let zeroed: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("names no macro"))
        .collect();
    assert_eq!(zeroed.len(), 2, "{:?}", result.diagnostics);
    assert!(
        zeroed
            .iter()
            .all(|d| d.severity == zerodds_idl_parse::Severity::Warning),
        "{zeroed:?}"
    );
}

#[test]
fn an_unmatched_endif_or_else_is_diagnosed() {
    for source in ["#endif\n", "#else\n", "#elif 1\n"] {
        let result = preprocess("main.idl", source, &empty());
        assert!(!result.diagnostics.is_empty(), "{source:?} was accepted");
    }
}

#[test]
fn a_conditional_left_open_is_diagnosed() {
    let result = preprocess("main.idl", "#ifdef A\nstruct S { long a; };\n", &empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("left open")),
        "{:?}",
        result.diagnostics
    );
}

// -- include -------------------------------------------------------------

#[test]
fn an_included_file_contributes_its_declarations() {
    let loader = empty().with("common.idl", "struct Common { long a; };\n");
    let source = "#include \"common.idl\"\nstruct Local { long b; };\n";
    let result = preprocess("main.idl", source, &loader);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.text.contains("Common"));
    assert!(result.text.contains("Local"));
}

#[test]
fn both_include_spellings_resolve() {
    let loader = empty().with("common.idl", "struct Common { long a; };\n");
    for source in ["#include \"common.idl\"\n", "#include <common.idl>\n"] {
        let result = preprocess("main.idl", source, &loader);
        assert!(result.text.contains("Common"), "{source:?}");
    }
}

#[test]
fn a_missing_include_is_diagnosed_and_parsing_continues() {
    let source = "#include \"absent.idl\"\nstruct Local { long b; };\n";
    let result = preprocess("main.idl", source, &empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("absent.idl"))
    );
    // The rest of the file still arrived.
    assert!(result.text.contains("Local"));
}

#[test]
fn an_include_cycle_is_diagnosed_and_not_followed() {
    // Following it would not terminate. Spec §3.4.
    let loader = empty()
        .with("a.idl", "#include \"b.idl\"\nstruct A { long a; };\n")
        .with("b.idl", "#include \"a.idl\"\nstruct B { long b; };\n");
    let result = preprocess("a.idl", "#include \"b.idl\"\n", &loader);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("includes itself")),
        "{:?}",
        result.diagnostics
    );
    // And it terminated, which is the actual assertion.
    assert!(result.text.contains('B'));
}

#[test]
fn an_include_guard_works_the_way_idl_files_use_it() {
    let loader = empty().with(
        "guarded.idl",
        "#ifndef GUARDED_IDL\n#define GUARDED_IDL\nstruct Once { long a; };\n#endif\n",
    );
    let source = "#include \"guarded.idl\"\n#include \"guarded.idl\"\n";
    let result = preprocess("main.idl", source, &loader);
    let occurrences = result.text.matches("Once").count();
    assert_eq!(occurrences, 1, "the guard did not hold: {}", result.text);
}

#[test]
fn pragma_once_holds_the_way_an_ifndef_guard_does() {
    let loader = empty().with("guarded.idl", "#pragma once\nstruct Once { long a; };\n");
    let source = "#include \"guarded.idl\"\n#include \"guarded.idl\"\n";
    let result = preprocess("main.idl", source, &loader);
    let occurrences = result.text.matches("Once").count();
    assert_eq!(
        occurrences, 1,
        "`#pragma once` did not hold: {}",
        result.text
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

/// A file that says `#pragma once` and is included once has to arrive
/// whole: the skip is about the second reading and nothing else.
#[test]
fn pragma_once_does_not_swallow_the_first_reading() {
    let loader = empty().with("guarded.idl", "#pragma once\nstruct Once { long a; };\n");
    let result = preprocess("main.idl", "#include \"guarded.idl\"\n", &loader);
    assert_eq!(result.text.matches("Once").count(), 1, "{}", result.text);
}

/// `#pragma once` is answered here and is not handed on as trivia: the
/// lowering reports every pragma it cannot interpret, and a directive
/// this stage has acted on is not one of those.
#[test]
fn pragma_once_does_not_survive_into_the_token_stream() {
    let result = preprocess(
        "main.idl",
        "#pragma once\nstruct S { long x; };\n",
        &empty(),
    );
    let directives: Vec<&str> = result
        .tokens
        .iter()
        .flat_map(|token| token.leading.iter())
        .filter(|trivia| trivia.kind == zerodds_idl_cst::TriviaKind::Directive)
        .filter_map(|trivia| trivia.span.slice("#pragma once\nstruct S { long x; };\n"))
        .collect();
    assert!(directives.is_empty(), "{directives:?}");
}

/// The pragma inside a branch the conditionals cut out was never read,
/// so it cannot guard anything.
#[test]
fn a_pragma_once_in_a_branch_that_is_cut_out_does_not_guard() {
    let loader = empty().with(
        "guarded.idl",
        "#ifdef NEVER\n#pragma once\n#endif\nstruct Once { long a; };\n",
    );
    let source = "#include \"guarded.idl\"\n#include \"guarded.idl\"\n";
    let result = preprocess("main.idl", source, &loader);
    assert_eq!(result.text.matches("Once").count(), 2, "{}", result.text);
}

/// Every other pragma still reaches the lowering as trivia: the arm
/// above is one directive and not a change to how pragmas travel.
#[test]
fn a_pragma_the_preprocessor_does_not_own_still_reaches_the_stream() {
    let source = "struct Reading { long id; };\n#pragma keylist Reading id\n";
    let result = preprocess("main.idl", source, &empty());
    let kept = result
        .tokens
        .iter()
        .flat_map(|token| token.leading.iter())
        .filter(|trivia| trivia.kind == zerodds_idl_cst::TriviaKind::Directive)
        .count();
    assert_eq!(kept, 1, "the keylist pragma did not survive");
}

// -- the source map ------------------------------------------------------

#[test]
fn a_literal_token_names_its_file_and_range() {
    let source = "struct S { long x; };\n";
    let result = preprocess("main.idl", source, &empty());
    let origin = result.map.origin(0).expect("an origin for the first token");
    assert_eq!(result.map.path(origin.file), Some("main.idl"));
    assert!(!origin.is_expanded());
    // The range points into the original file, not into the expanded
    // text — that is the whole point of the map.
    assert_eq!(origin.span.slice(source), Some("struct"));
}

#[test]
fn an_expanded_token_names_both_the_text_and_the_macro_that_produced_it() {
    // Spec §3.4: a diagnostic inside an expansion has to be able to say
    // both.
    let source = "#define MAX 64\nconst long N = MAX;\n";
    let result = preprocess("main.idl", source, &empty());

    let expanded: Vec<_> = (0..result.map.len())
        .filter_map(|i| result.map.origin(i))
        .filter(|o| o.is_expanded())
        .collect();
    assert_eq!(expanded.len(), 1, "expected exactly the expanded 64");

    let origin = expanded[0];
    // Where a reader should look: the use site, not the definition.
    assert_eq!(origin.span.slice(source), Some("MAX"));
    // And which macro put it there.
    assert_eq!(
        origin.macro_chain.first().map(|u| u.name.as_str()),
        Some("MAX")
    );
}

#[test]
fn tokens_from_an_included_file_name_that_file_not_the_includer() {
    let loader = empty().with("common.idl", "struct Common { long a; };\n");
    let source = "#include \"common.idl\"\nstruct Local { long b; };\n";
    let result = preprocess("main.idl", source, &loader);

    let files: Vec<&str> = (0..result.map.len())
        .filter_map(|i| result.map.origin(i))
        .filter_map(|o| result.map.path(o.file))
        .collect();
    assert!(files.contains(&"common.idl"));
    assert!(files.contains(&"main.idl"));
}

#[test]
fn the_map_covers_every_token_of_the_stream() {
    // A token without an origin would be one no diagnostic could point
    // at.
    let loader = empty().with("common.idl", "struct Common { long a; };\n");
    let source = "#define MAX 8\n#include \"common.idl\"\nconst long N = MAX;\n";
    let result = preprocess("main.idl", source, &loader);
    assert_eq!(result.map.len(), result.tokens.len());
    for index in 0..result.tokens.len() {
        assert!(
            result.map.origin(index).is_some(),
            "token {index} has no origin"
        );
    }
}

// -- pragmas -------------------------------------------------------------

#[test]
fn a_pragma_is_not_the_preprocessors_business() {
    // It carries declaration metadata and reaches the lowering through
    // the trivia it sits in, so the preprocessor neither acts on it nor
    // complains about it.
    let result = preprocess(
        "main.idl",
        "#pragma prefix \"omg.org\"\nstruct S { long a; };\n",
        &empty(),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.text.contains('S'));
}

#[test]
fn the_expanded_stream_carries_no_directive_tokens() {
    // Directives are consumed by the preprocessor, not handed on: a `#`
    // reaching the parser would be a leak.
    let source = "#define MAX 8
#ifdef MAX
struct S { long a; };
#endif
";
    let found = kinds(source, &empty());
    assert!(
        !found.contains(&TokenKind::Hash),
        "a directive leaked through"
    );
    assert!(found.contains(&TokenKind::Struct));
}

// -- termination ---------------------------------------------------------

#[test]
fn an_empty_file_preprocesses_to_nothing() {
    let result = preprocess("main.idl", "", &empty());
    assert!(result.tokens.is_empty());
    assert!(result.diagnostics.is_empty());
}

proptest::proptest! {
    /// Whatever the directives look like, preprocessing terminates and
    /// every token it emits has an origin.
    #[test]
    fn any_input_terminates_with_a_complete_map(source in "\\PC{0,200}") {
        let result = preprocess("main.idl", &source, &MemoryLoader::new());
        proptest::prop_assert_eq!(result.map.len(), result.tokens.len());
    }
}

// -- the preprocessor feeding the parser, spec §3 ------------------------

#[test]
fn an_included_file_reaches_the_tree() {
    // The point of wiring the two together: a type declared in the
    // included file is in the parsed result, not merely mentioned.
    let loader = MemoryLoader::default().with(
        "common.idl",
        "module Shared { struct Header { long id; }; };",
    );
    let source = "#include \"common.idl\"\nmodule App { struct Body { long value; }; };";
    let pre = preprocess("app.idl", source, &loader);
    assert!(pre.diagnostics.is_empty(), "{:?}", pre.diagnostics);

    let parsed = zerodds_idl_parse::parse_preprocessed(&pre);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let text = parsed.syntax().text();
    assert!(
        text.contains("Header"),
        "the include did not reach the tree"
    );
    assert!(text.contains("Body"));
}

#[test]
fn a_macro_is_expanded_before_the_parser_sees_it() {
    let source = "#define LIMIT 8\nstruct S { sequence<long, LIMIT> items; };";
    let pre = preprocess("m.idl", source, &MemoryLoader::default());
    let parsed = zerodds_idl_parse::parse_preprocessed(&pre);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let text = parsed.syntax().text();
    assert!(text.contains('8'), "the macro was not expanded: {text}");
    assert!(!text.contains("LIMIT"), "the macro name survived: {text}");
}

#[test]
fn a_conditional_branch_that_is_off_does_not_reach_the_tree() {
    let source = "struct Kept { long a; };\n#ifdef ENABLE\nstruct Cut { long b; };\n#endif";
    let pre = preprocess("c.idl", source, &MemoryLoader::default());
    let parsed = zerodds_idl_parse::parse_preprocessed(&pre);
    let text = parsed.syntax().text();
    assert!(text.contains("Kept"));
    assert!(!text.contains("Cut"), "a disabled branch reached the tree");
}

#[test]
fn the_expanded_text_is_itself_lexable() {
    // Tokens concatenated without a separator would turn `struct S` into
    // one identifier for anything reading the text rather than the
    // stream.
    let pre = preprocess("s.idl", "struct S { long x; };", &MemoryLoader::default());
    let reparsed = zerodds_idl_parse::parse(&pre.text);
    assert!(
        reparsed.diagnostics().is_empty(),
        "the expanded text does not parse: {:?} — {:?}",
        pre.text,
        reparsed.diagnostics()
    );
}

/// A chain that is merely very long stops too. Spec §3.4 bounds the
/// nesting; DDS 1.4 §1.1 is why there is a bound at all — "avoid
/// properties that may require the use of unbounded or hard-to-predict
/// resources". `MAX_INCLUDE_DEPTH` is 64, so a chain of 64 links is
/// preprocessed and one of 70 is refused at the 65th.
#[test]
fn an_include_chain_past_the_nesting_bound_is_refused() {
    fn chain(links: usize) -> (MemoryLoader, String) {
        let mut loader = empty();
        for i in 0..links {
            let body = if i + 1 < links {
                format!(
                    "#include \"link{}.idl\"\nstruct L{i} {{ long a; }};\n",
                    i + 1
                )
            } else {
                format!("struct L{i} {{ long a; }};\n")
            };
            loader = loader.with(&format!("link{i}.idl"), &body);
        }
        (loader, "#include \"link0.idl\"\n".to_owned())
    }

    let (loader, source) = chain(64);
    let within = preprocess("main.idl", &source, &loader);
    assert!(
        !within
            .diagnostics
            .iter()
            .any(|d| d.message.contains("include nesting past")),
        "64 links are within the bound: {:?}",
        within.diagnostics
    );
    assert!(within.text.contains("L63"), "the last link did not arrive");

    let (loader, source) = chain(70);
    let past = preprocess("main.idl", &source, &loader);
    assert!(
        past.diagnostics
            .iter()
            .any(|d| d.message.contains("include nesting past 64 levels")),
        "the bound did not bite: {:?}",
        past.diagnostics
    );
    // It stopped at the bound instead of walking the rest of the chain.
    assert!(!past.text.contains("L69"), "the chain was followed past 64");
}

#[test]
fn a_preprocessor_diagnostic_comes_before_the_parsers() {
    let source = "#include \"missing.idl\"\nstruct S { long x; };";
    let pre = preprocess("p.idl", source, &MemoryLoader::default());
    assert!(
        !pre.diagnostics.is_empty(),
        "a missing include went unreported"
    );

    let parsed = zerodds_idl_parse::parse_preprocessed(&pre);
    assert_eq!(
        parsed.diagnostics().first().map(|d| d.message.clone()),
        pre.diagnostics.first().map(|d| d.message.clone()),
        "the preprocessor's diagnostic did not come first"
    );
}

// -- the suffix annotation form through the preprocessor -----------------

/// A `//@` comment survives preprocessing.
///
/// The preprocessor emits no comments, so `//@key` reached the parser as
/// nothing at all: `zerodds-idl-compose` — the path `zerodds-idlc` and
/// every `build.rs` take — produced a `ShapeType` with no key and said
/// nothing about it. DDS-XTypes 1.3 §7.3.1.2.5 makes the comment an
/// annotation, so it is held like a `#pragma` and attached to the token
/// that follows.
#[test]
fn a_suffix_annotation_survives_preprocessing() {
    let source = "\
#define COLOR_MAX 64
struct ShapeType {
    string<COLOR_MAX> color; //@key
    long x;
};
";
    let pre = preprocess("shapes.idl", source, &MemoryLoader::new());
    assert!(pre.diagnostics.is_empty(), "{:?}", pre.diagnostics);
    let parsed = zerodds_idl_parse::parse_preprocessed(&pre);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert!(
        parsed
            .syntax()
            .descendants()
            .iter()
            .any(|node| node.kind() == zerodds_idl_cst::CstKind::Annotation),
        "the annotation did not survive: {}",
        parsed.syntax().text()
    );
}

/// One written after the last declaration of the unit survives too.
///
/// §7.3.1.2.3's own example ends the file with `}; //@my_type_annotation`,
/// and an annotation is read from the leading trivia of the token *after*
/// the declaration — which at the end of a unit is the end of file.
#[test]
fn a_suffix_annotation_at_the_end_of_the_unit_survives_preprocessing() {
    let source = "struct Gadget { long my_integer; }; //@nested\n";
    let pre = preprocess("g.idl", source, &MemoryLoader::new());
    let parsed = zerodds_idl_parse::parse_preprocessed(&pre);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert!(
        parsed
            .syntax()
            .descendants()
            .iter()
            .any(|node| node.kind() == zerodds_idl_cst::CstKind::Annotation),
        "the annotation did not survive: {}",
        parsed.syntax().text()
    );
    assert_eq!(pre.map.len(), pre.tokens.len(), "every token has an origin");
}

/// A comment that is not the suffix form is still dropped, and a `//@`
/// in a branch the preprocessor cut goes with the branch.
#[test]
fn an_ordinary_comment_and_a_cut_branch_carry_nothing_through() {
    let source = "\
struct S {
    long x; // an ordinary comment
};
#ifdef ENABLE
struct Cut { long y; }; //@nested
#endif
";
    let pre = preprocess("c.idl", source, &MemoryLoader::new());
    assert!(
        !pre.text.contains("ordinary"),
        "a comment reached the stream: {}",
        pre.text
    );
    assert!(
        !pre.text.contains("//@"),
        "a `//@` from a cut branch reached the stream: {}",
        pre.text
    );
    let parsed = zerodds_idl_parse::parse_preprocessed(&pre);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
}

/// `#error` produces a diagnostic. ISO/IEC 14882:2003 §16.5, which
/// IDL 4.2 §7.3 adopts: "#error causes the implementation to produce a
/// diagnostic message that includes the specified sequence of
/// preprocessing tokens."
///
/// It was read and dropped. A file whose whole purpose was to refuse an
/// unsupported configuration compiled clean and emitted an empty
/// translation unit — the directive that exists to stop a build was the
/// one directive that could not.
#[test]
fn an_error_directive_is_a_diagnostic_and_carries_its_text() {
    let out = preprocess("main.idl", "#error stop here\n", &empty());
    let found = out
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "IDL0020")
        .unwrap_or_else(|| panic!("no IDL0020: {:?}", out.diagnostics));
    assert!(
        found.message.contains("stop here"),
        "the clause requires the tokens in the message: {}",
        found.message
    );
}

#[test]
fn an_error_directive_in_a_branch_that_is_cut_out_says_nothing() {
    // The whole subtlety of the rule. An `#error` inside a conditional
    // the preprocessor did not take is text that was never reached, and
    // reporting it would refuse every file that guards one — which is
    // nearly every file that has one at all.
    for source in [
        "#ifdef NOPE\n#error stop here\n#endif\nstruct Sa { long m; };\n",
        "#ifndef NOPE\nstruct Sa { long m; };\n#else\n#error stop here\n#endif\n",
        "#if 0\n#error stop here\n#endif\nstruct Sa { long m; };\n",
    ] {
        let out = preprocess("main.idl", source, &empty());
        assert!(
            !out.diagnostics.iter().any(|d| d.code.as_str() == "IDL0020"),
            "{source:?} reached no #error: {:?}",
            out.diagnostics
        );
    }
}

/// A bare `#error` with no tokens after it is still a diagnostic.
#[test]
fn a_bare_error_directive_is_still_a_diagnostic() {
    let out = preprocess("main.idl", "#error\n", &empty());
    assert!(
        out.diagnostics.iter().any(|d| d.code.as_str() == "IDL0020"),
        "{:?}",
        out.diagnostics
    );
}
