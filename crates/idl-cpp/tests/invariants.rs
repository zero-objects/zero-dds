//! Toolchain-free structural invariants for the idl-cpp emitter.
//!
//! These assert directly on the generated C++ source string — no compiler
//! required — and pin the construct-fix campaign findings for this backend:
//!
//! - **F4** enum `@value` gaps emit explicit wire values (not plain ordinals).
//! - **F6** `const boolean = TRUE/FALSE` normalises to `true`/`false`.
//! - **F8** a string const is `constexpr std::string_view` (a literal type),
//!   never the ill-formed `constexpr std::string`.
//! - **F24** a `map<Struct, _>` key struct gets a lexicographic `operator<`.
//! - **F3** a `long double` member is rejected at codegen (no silent,
//!   platform-divergent 16-byte wire).
//!
//! Cross-cutting invariants: no duplicate top-level symbol, no C++ reserved
//! word emitted as a bare (unescaped) identifier, and none of the known
//! non-compiling literal patterns (`TRUE`/`FALSE`, ill-formed `constexpr`
//! string, an empty `enum`/`class` body).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

fn try_gen(src: &str) -> Result<String, String> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).map_err(|e| format!("{e:?}"))
}

// ---------------------------------------------------------------------------
// F4 — enum @value
// ---------------------------------------------------------------------------

#[test]
fn enum_value_gaps_emit_explicit_values() {
    // XTypes 1.3 §7.4.5.1: a gap must carry through to the C++ constant, else
    // the compiler re-derives plain ordinals and the wire value diverges.
    let cpp = emit("enum Sparse { A, @value(5) B, C, @value(100) D };");
    for tok in ["A = 0,", "B = 5,", "C = 6,", "D = 100,"] {
        assert!(cpp.contains(tok), "missing {tok:?} in:\n{cpp}");
    }
}

#[test]
fn enum_negative_value_is_signed() {
    let cpp = emit("enum N { A, @value(-3) B, C };");
    assert!(
        cpp.contains("B = -3,"),
        "negative @value not honoured:\n{cpp}"
    );
    assert!(
        cpp.contains("C = -2,"),
        "successor of negative not continued:\n{cpp}"
    );
}

#[test]
fn plain_enum_still_gets_explicit_ordinals() {
    let cpp = emit("enum Color { RED, GREEN, BLUE };");
    for tok in ["RED = 0,", "GREEN = 1,", "BLUE = 2,"] {
        assert!(cpp.contains(tok), "missing {tok:?} in:\n{cpp}");
    }
}

// ---------------------------------------------------------------------------
// F6 — boolean const TRUE/FALSE
// ---------------------------------------------------------------------------

#[test]
fn boolean_const_normalises_case() {
    let cpp = emit("const boolean FLAG_ON = TRUE; const boolean FLAG_OFF = FALSE;");
    assert!(
        cpp.contains("FLAG_ON = true"),
        "TRUE not normalised:\n{cpp}"
    );
    assert!(
        cpp.contains("FLAG_OFF = false"),
        "FALSE not normalised:\n{cpp}"
    );
    // C++ has no `TRUE`/`FALSE` token — the raw IDL spelling must not survive.
    assert!(!cpp.contains("= TRUE"), "raw TRUE literal leaked:\n{cpp}");
    assert!(!cpp.contains("= FALSE"), "raw FALSE literal leaked:\n{cpp}");
}

// ---------------------------------------------------------------------------
// F8 — string const constexpr-ness
// ---------------------------------------------------------------------------

#[test]
fn string_const_is_constexpr_string_view() {
    let cpp = emit(r#"const string GREETING = "hello";"#);
    assert!(
        cpp.contains("constexpr std::string_view GREETING"),
        "string const must be constexpr string_view:\n{cpp}"
    );
    // The ill-formed `constexpr std::string <name>` (non-literal type) must not
    // appear — matching on the trailing space avoids `std::string_view`.
    assert!(
        !cpp.contains("constexpr std::string GREETING"),
        "ill-formed constexpr std::string emitted:\n{cpp}"
    );
}

#[test]
fn wstring_const_is_constexpr_wstring_view() {
    let cpp = emit(r#"const wstring W = L"hi";"#);
    assert!(
        cpp.contains("constexpr std::wstring_view W"),
        "wstring const must be constexpr wstring_view:\n{cpp}"
    );
    assert!(
        !cpp.contains("constexpr std::wstring W"),
        "ill-formed constexpr std::wstring emitted:\n{cpp}"
    );
}

// ---------------------------------------------------------------------------
// F24 — map<Struct, _> key ordering
// ---------------------------------------------------------------------------

#[test]
fn map_struct_key_emits_operator_less() {
    let cpp = emit(
        "struct Key { long a; string b; }; \
         struct Container { map<Key, long> table; };",
    );
    assert!(cpp.contains("std::map<"), "map member missing:\n{cpp}");
    assert!(
        cpp.contains("operator<(const Key&"),
        "map-key struct must get operator<:\n{cpp}"
    );
    // The comparator must reference the members lexicographically via std::tie.
    assert!(
        cpp.contains("std::tie("),
        "operator< must use std::tie:\n{cpp}"
    );
}

#[test]
fn non_key_struct_has_no_operator_less() {
    // A struct never used as a map key must not gain a spurious operator<.
    let cpp = emit("struct Plain { long a; };");
    assert!(
        !cpp.contains("operator<(const Plain&"),
        "non-key struct must not get operator<:\n{cpp}"
    );
}

// ---------------------------------------------------------------------------
// F3 — long double reject
// ---------------------------------------------------------------------------

#[test]
fn long_double_member_rejected() {
    assert!(
        try_gen("struct F { long double v; };").is_err(),
        "long double member must be rejected at codegen"
    );
}

#[test]
fn long_double_in_sequence_rejected() {
    assert!(
        try_gen("struct F { sequence<long double> v; };").is_err(),
        "long double sequence element must be rejected at codegen"
    );
}

// ---------------------------------------------------------------------------
// Cross-cutting non-compile-pattern invariants
// ---------------------------------------------------------------------------

/// The reserved-word fixture exercises every declaration position; the emitter
/// must escape each keyword — never emit it bare — and must not collapse two
/// distinct symbols onto one name.
const RESERVED_WORD_IDL: &str = "module class { \
    enum mutable { new, delete }; \
    @final struct template { long int; long operator; }; \
    struct virtual : template { long throw; }; \
    union namespace switch (mutable) { \
        case new: long alignas; default: octet register; \
    }; \
};";

#[test]
fn no_bare_reserved_word_identifier() {
    let cpp = emit(RESERVED_WORD_IDL);
    // Illegal bare declarations that would fail to compile if the escape were
    // skipped. Each must be absent (the escaped `_`-suffixed form is present).
    for bad in [
        "namespace class {",
        "enum class mutable ",
        "class template {",
        "class virtual ",
    ] {
        assert!(
            !cpp.contains(bad),
            "unescaped reserved word {bad:?} in:\n{cpp}"
        );
    }
    for good in [
        "namespace class_ {",
        "enum class mutable_ ",
        "class template_ {",
    ] {
        assert!(
            cpp.contains(good),
            "escaped form {good:?} missing in:\n{cpp}"
        );
    }
}

#[test]
fn no_duplicate_top_level_symbol() {
    let cpp = emit(
        "struct Alpha { long a; }; \
         struct Beta { long b; }; \
         enum Gamma { G0, G1 };",
    );
    assert_eq!(
        cpp.matches("class Alpha {").count(),
        1,
        "Alpha duplicated:\n{cpp}"
    );
    assert_eq!(
        cpp.matches("class Beta {").count(),
        1,
        "Beta duplicated:\n{cpp}"
    );
    assert_eq!(
        cpp.matches("enum class Gamma ").count(),
        1,
        "Gamma duplicated:\n{cpp}"
    );
}

#[test]
fn no_empty_enum_or_class_body() {
    // A defined struct/enum must never emit a truly empty declaration body
    // (`class S {};` / `enum class E : int32_t {};`) — that is the signature of
    // a silently dropped construct. Match the specific empty-body forms rather
    // than any `{}`, which occurs legitimately (e.g. `std::array{{…}}`).
    let cpp = emit("struct S { long x; }; enum E { ONLY };");
    assert!(
        !cpp.contains("class S {};"),
        "empty struct body emitted:\n{cpp}"
    );
    assert!(
        !cpp.contains("enum class E : int32_t {};"),
        "empty enum body emitted:\n{cpp}"
    );
    // Positive: the enumerator is present with its explicit value.
    assert!(cpp.contains("ONLY = 0,"), "enumerator dropped:\n{cpp}");
    assert!(cpp.contains("class S {"), "struct dropped:\n{cpp}");
}

#[test]
fn no_raw_true_false_or_ill_formed_constexpr_string() {
    let cpp = emit(r#"const boolean B = TRUE; const string S = "x"; enum E { X };"#);
    assert!(!cpp.contains("TRUE"), "raw TRUE leaked:\n{cpp}");
    assert!(
        !cpp.contains("constexpr std::string S"),
        "ill-formed constexpr std::string:\n{cpp}"
    );
}
