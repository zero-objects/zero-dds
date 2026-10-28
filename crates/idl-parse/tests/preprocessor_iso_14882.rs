// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The preprocessor IDL 4.2 §7.3 adopts, ISO/IEC 14882:2003 §16, as far
//! as `docs/spec-coverage/idl-4.2.gaps.md` A.5 recorded it missing:
//! function-like macros, `#`, `##`, the rescan, `#if` over a full
//! constant expression, `__LINE__`, `__FILE__`, `#line`, and lines
//! continued with a backslash.
//!
//! Where the standard gives an example, the test is that example.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl_parse::{DiagnosticCode, MemoryLoader, Preprocessed, Severity, preprocess};

fn run(source: &str) -> Preprocessed {
    preprocess("main.idl", source, &MemoryLoader::new())
}

/// The expanded token texts, joined by single spaces.
fn expanded(source: &str) -> String {
    let result = run(source);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{source:?}: {errors:?}");
    result
        .tokens
        .iter()
        .map(|token| token.span.slice(&result.text).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

fn errors(source: &str) -> Vec<String> {
    run(source)
        .diagnostics
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{} {}", d.code.as_str(), d.message))
        .collect()
}

// -- §16.3: function-like macros -------------------------------------------

#[test]
fn a_function_like_macro_substitutes_its_arguments() {
    assert_eq!(
        expanded("#define TWICE(x) ((x) * 2)\nconst long K = TWICE(3);\n"),
        "const long K = ( ( 3 ) * 2 ) ;"
    );
}

#[test]
fn a_comma_inside_parentheses_does_not_split_an_argument() {
    assert_eq!(
        expanded("#define FIRST(a, b) a\nconst long K = FIRST((1, 2), 3);\n"),
        "const long K = ( 1 , 2 ) ;"
    );
}

#[test]
fn a_function_like_name_without_parentheses_is_not_an_invocation() {
    // §16.3/10: only a `(` after the name makes it an invocation.
    assert_eq!(
        expanded("#define F(x) x\nconst long F = 1;\n"),
        "const long F = 1 ;"
    );
}

#[test]
fn a_space_before_the_parenthesis_makes_an_object_like_macro() {
    assert_eq!(
        expanded("#define P (1)\nconst long K = P;\n"),
        "const long K = ( 1 ) ;"
    );
}

#[test]
fn an_invocation_may_span_lines() {
    assert_eq!(
        expanded("#define ADD(a, b) a + b\nconst long K = ADD(1,\n  2);\n"),
        "const long K = 1 + 2 ;"
    );
}

#[test]
fn a_wrong_number_of_arguments_is_an_error() {
    let found = errors("#define ADD(a, b) a + b\nconst long K = ADD(1);\n");
    assert!(
        found
            .iter()
            .any(|e| e.contains("IDL0022") && e.contains("2 argument")),
        "{found:?}"
    );
}

#[test]
fn an_unclosed_invocation_is_an_error() {
    let found = errors("#define F(x) x\nconst long K = F(1;\n");
    assert!(found.iter().any(|e| e.contains("not closed")), "{found:?}");
}

// -- §16.3.2: `#` ------------------------------------------------------------

#[test]
fn stringize_spells_the_argument() {
    assert_eq!(
        expanded("#define STR(x) #x\nconst string K = STR(ab);\n"),
        "const string K = \"ab\" ;"
    );
    // One space where white space stood, none at the ends, and the
    // quotes and backslashes of a literal in the argument escaped.
    assert_eq!(
        expanded("#define STR(x) #x\nconst string K = STR(  a   +  \"q\\n\"  );\n"),
        "const string K = \"a + \\\"q\\\\n\\\"\" ;"
    );
}

#[test]
fn a_hash_not_followed_by_a_parameter_is_refused_at_the_define() {
    let found = errors("#define BAD(x) # y\n");
    assert!(
        found
            .iter()
            .any(|e| e.contains("not followed by a parameter")),
        "{found:?}"
    );
}

// -- §16.3.3: `##` -----------------------------------------------------------

#[test]
fn token_paste_joins_two_tokens() {
    assert_eq!(
        expanded("#define CAT(a, b) a##b\nstruct CAT(S, a) { long m; };\n"),
        "struct Sa { long m ; } ;"
    );
}

#[test]
fn an_empty_argument_pastes_as_nothing() {
    // §16.3.3/2: the placemarker.
    assert_eq!(
        expanded("#define CAT(a, b) a##b\nconst long K = CAT(, 7);\n"),
        "const long K = 7 ;"
    );
}

#[test]
fn a_paste_that_is_not_one_token_is_an_error() {
    let found = errors("#define CAT(a, b) a##b\nconst long K = CAT(+, -);\n");
    assert!(found.iter().any(|e| e.contains("not one")), "{found:?}");
}

#[test]
fn a_paste_at_either_end_is_refused_at_the_define() {
    assert!(!errors("#define BAD(x) ## x\n").is_empty());
    assert!(!errors("#define BAD x ##\n").is_empty());
}

// -- §16.3.4: rescan and its limit -------------------------------------------

#[test]
fn a_macro_does_not_expand_inside_its_own_expansion() {
    // `#define X X` would loop forever without §16.3.4/2.
    assert_eq!(
        expanded("#define X X + 1\nconst long X = 0;\n"),
        "const long X + 1 = 0 ;"
    );
}

#[test]
fn the_rescan_reaches_the_tokens_after_the_invocation() {
    // §16.3.5/5, the second part of the standard's own example:
    // `#define f(a) a*g` and `#define g(a) f(a)`, then `f(2)(9)`,
    // gives `2*9*g`.
    assert_eq!(
        expanded("#define f(a) a*g\n#define g(a) f(a)\nconst long K = f(2)(9);\n"),
        "const long K = 2 * 9 * g ;"
    );
}

#[test]
fn an_argument_is_expanded_before_it_is_substituted() {
    assert_eq!(
        expanded("#define ONE 1\n#define ID(x) x\nconst long K = ID(ONE);\n"),
        "const long K = 1 ;"
    );
    // …but not next to `#`, which spells it as written.
    assert_eq!(
        expanded("#define ONE 1\n#define STR(x) #x\nconst string K = STR(ONE);\n"),
        "const string K = \"ONE\" ;"
    );
}

// -- §16.1: `#if` ----------------------------------------------------------

#[test]
fn if_evaluates_an_arithmetic_expression() {
    assert!(expanded("#if 2 * 3 > 5\nstruct Kept { long a; };\n#endif\n").contains("Kept"));
    assert!(!expanded("#if 2 * 3 > 6\nstruct Gone { long a; };\n#endif\n").contains("Gone"));
}

#[test]
fn if_replaces_macros_and_answers_defined_first() {
    let source = "#define V 3\n#if defined(V) && V >= 3 && !defined NOPE\nstruct Kept { long a; };\n#endif\n";
    assert!(expanded(source).contains("Kept"));
}

#[test]
fn a_function_like_macro_works_in_a_condition() {
    let source = "#define MAX(a, b) ((a) > (b) ? (a) : (b))\n#if MAX(2, 7) == 7\nstruct Kept { long a; };\n#endif\n";
    assert!(expanded(source).contains("Kept"));
}

#[test]
fn an_if_inside_a_dropped_group_is_not_evaluated() {
    // `1 / 0` would be an error; inside `#if 0` it is never reached.
    let source = "#if 0\n#if 1 / 0\n#endif\n#endif\nstruct S { long a; };\n";
    assert!(errors(source).is_empty(), "{:?}", errors(source));
}

#[test]
fn a_division_by_zero_in_a_live_condition_is_an_error() {
    let found = errors("#if 1 / 0\n#endif\n");
    assert!(
        found.iter().any(|e| e.contains("division by zero")),
        "{found:?}"
    );
}

// -- §16.8: predefined macros, and §16.4: `#line` -----------------------------

#[test]
fn line_and_file_name_where_they_stand() {
    assert_eq!(
        expanded("\n\nconst long K = __LINE__;\n"),
        "const long K = 3 ;"
    );
    assert_eq!(
        expanded("const string K = __FILE__;\n"),
        "const string K = \"main.idl\" ;"
    );
}

#[test]
fn line_inside_a_macro_is_the_line_of_the_invocation() {
    assert_eq!(
        expanded("#define HERE __LINE__\n\n\nconst long K = HERE;\n"),
        "const long K = 4 ;"
    );
}

#[test]
fn hash_line_renumbers_and_renames() {
    assert_eq!(
        expanded("#line 100 \"robot.idl\"\nconst long K = __LINE__;\nconst string F = __FILE__;\n"),
        "const long K = 100 ; const string F = \"robot.idl\" ;"
    );
}

#[test]
fn the_predefined_names_cannot_be_defined() {
    let found = errors("#define __LINE__ 7\n");
    assert!(
        found.iter().any(|e| e.contains("cannot be defined")),
        "{found:?}"
    );
}

// -- §2.1 phase 2: line continuation ------------------------------------------

#[test]
fn a_backslash_continues_a_directive() {
    assert_eq!(
        expanded("#define NN \\\n4\nconst long K = NN;\n"),
        "const long K = 4 ;"
    );
    assert_eq!(
        expanded("#define ADD(a, \\\n  b) a + b\nconst long K = ADD(1, 2);\n"),
        "const long K = 1 + 2 ;"
    );
}

// -- what the parser sees ------------------------------------------------------

#[test]
fn an_expansion_that_is_not_an_idl_token_is_reported() {
    let found = errors("#define AND &&\nconst long K = 1 AND 2;\n");
    assert!(
        found.iter().any(|e| e.contains("not an IDL token")),
        "{found:?}"
    );
}

#[test]
fn the_diagnostic_code_is_stable() {
    assert_eq!(DiagnosticCode::Preprocessing.as_str(), "IDL0022");
}
