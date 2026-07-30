// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Shared, injective, reversible name encoding for generated identifiers.
//!
//! Every backend that flattens a scoped IDL name (`Module::Type`) or a file
//! basename into a single target-language identifier used to do so with an
//! ad-hoc `join("_")` / `replace("::", "_")`. That mapping is **not injective**:
//! `A::B_C` and `A_B::C` both collapse to `A_B_C`, `a-b` and `a_b` both collapse
//! to `a_b`, and a lossy sanitiser folds `Foo`/`foo` together. When two distinct
//! IDL types map to the same identifier the generated code fails to compile
//! (Rust E0428, C# CS0102, TypeScript TS2451, `javac` redefinition) or — worse —
//! silently overwrites the first definition (Python).
//!
//! This module is the single source of truth for that mapping. The encoding is
//! injective because the escape lead-in byte `_` is itself escaped, so no output
//! is ambiguous:
//!
//! | input            | output    |
//! |------------------|-----------|
//! | `[A-Za-z0-9]`    | unchanged |
//! | `_`              | `_u`      |
//! | `::` (scope sep) | `_s`      |
//! | `-`              | `_h`      |
//! | `.`              | `_d`      |
//! | leading digit    | prefix `_n` (once, at the very front) |
//! | any other byte   | `_x` + two lowercase hex digits |
//!
//! The base alphabet `[A-Za-z0-9]` is preserved and case is **never** folded
//! (folding would make `Foo` and `foo` collide). The result is always a valid
//! identifier in every target language (it starts with a letter or `_` and
//! contains only `[A-Za-z0-9_]`). Distinct examples that must stay distinct:
//! `A::B_C` -> `A_sB_uC`, `A_B::C` -> `A_uB_sC`, `a-b` -> `a_hb`,
//! `a_b` -> `a_ub`, `Foo` -> `Foo`, `foo` -> `foo`.

/// Appends the injective encoding of a single raw component (no scope
/// separator) to `out`. Used by both [`encode_ident`] and [`encode_scoped`]
/// so a component encodes identically whether it arrives as part of an FQN
/// string or as a split element.
fn encode_component_into(s: &str, out: &mut String) {
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => out.push(b as char),
            b'_' => out.push_str("_u"),
            b'-' => out.push_str("_h"),
            b'.' => out.push_str("_d"),
            other => {
                out.push_str("_x");
                out.push(hex_digit(other >> 4));
                out.push(hex_digit(other & 0x0f));
            }
        }
    }
}

/// Lowercase hex digit for a nibble `0..=15`.
fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + (nibble - 10)) as char,
    }
}

/// Prepends the leading-digit guard `_n` if `s` starts with an ASCII digit.
/// A raw leading `_` never reaches here as a literal (it is already `_u`), and
/// a raw leading `-`/`.` becomes `_h`/`_d`, so the only way an encoded string
/// can start with a digit is a genuine leading digit in the source — making the
/// `_n` prefix unambiguous.
fn guard_leading_digit(mut s: String) -> String {
    if s.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        s.insert_str(0, "_n");
    }
    s
}

/// Injective identifier from an arbitrary string, treating `::` as the IDL
/// scope separator (`Robot::Pose` -> `Robot_sPose`). Every other byte is
/// encoded per the module-level table.
#[must_use]
pub fn encode_ident(raw: &str) -> String {
    let mut out = String::new();
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => {
                out.push(b as char);
                i += 1;
            }
            b':' if i + 1 < bytes.len() && bytes[i + 1] == b':' => {
                out.push_str("_s");
                i += 2;
            }
            b'_' => {
                out.push_str("_u");
                i += 1;
            }
            b'-' => {
                out.push_str("_h");
                i += 1;
            }
            b'.' => {
                out.push_str("_d");
                i += 1;
            }
            other => {
                out.push_str("_x");
                out.push(hex_digit(other >> 4));
                out.push(hex_digit(other & 0x0f));
                i += 1;
            }
        }
    }
    guard_leading_digit(out)
}

/// Injective flattened identifier for a declaration `simple` nested in module
/// path `scope`. Equivalent to [`encode_ident`] applied to the components
/// joined by `::`, so an emitter that carries the scope split and a consumer
/// that carries the FQN string agree on the result.
///
/// `A::B_C` (scope `["A"]`, simple `"B_C"`) -> `A_sB_uC`;
/// `A_B::C` (scope `["A_B"]`, simple `"C"`) -> `A_uB_sC`.
#[must_use]
pub fn encode_scoped(scope: &[String], simple: &str) -> String {
    let mut out = String::new();
    for part in scope {
        encode_component_into(part, &mut out);
        out.push_str("_s");
    }
    encode_component_into(simple, &mut out);
    guard_leading_digit(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_alphabet_passthrough() {
        assert_eq!(encode_ident("Foo"), "Foo");
        assert_eq!(encode_ident("foo"), "foo");
        assert_eq!(encode_ident("Robot0"), "Robot0");
    }

    #[test]
    fn case_is_never_folded() {
        assert_ne!(encode_ident("Foo"), encode_ident("foo"));
        assert_ne!(encode_ident("A_B"), encode_ident("a_b"));
    }

    #[test]
    fn scope_and_underscore_are_distinct() {
        // The whole point: these must not collapse to the same identifier.
        assert_eq!(encode_ident("A::B_C"), "A_sB_uC");
        assert_eq!(encode_ident("A_B::C"), "A_uB_sC");
        assert_ne!(encode_ident("A::B_C"), encode_ident("A_B::C"));
    }

    #[test]
    fn punctuation_variants_are_distinct() {
        assert_eq!(encode_ident("a-b"), "a_hb");
        assert_eq!(encode_ident("a_b"), "a_ub");
        assert_eq!(encode_ident("a.b"), "a_db");
        assert_ne!(encode_ident("a-b"), encode_ident("a_b"));
        assert_ne!(encode_ident("a-b"), encode_ident("a.b"));
    }

    #[test]
    fn leading_digit_is_guarded() {
        assert_eq!(encode_ident("9lives"), "_n9lives");
        // Interior components starting with a digit need no guard (preceded
        // by the `_s` separator, which is a legal identifier continuation).
        assert_eq!(encode_ident("a::9b"), "a_s9b");
    }

    #[test]
    fn other_bytes_become_hex_escapes() {
        assert_eq!(encode_ident("a b"), "a_x20b");
        assert_eq!(encode_ident("a+b"), "a_x2bb");
        // A lone colon is not a scope separator.
        assert_eq!(encode_ident("a:b"), "a_x3ab");
    }

    #[test]
    fn encode_scoped_matches_encode_ident_of_joined() {
        for (scope, simple, fqn) in [
            (vec!["A".to_string()], "B_C", "A::B_C"),
            (vec!["A_B".to_string()], "C", "A_B::C"),
            (vec!["a".to_string(), "b".to_string()], "R", "a::b::R"),
            (vec![], "Flat", "Flat"),
            (vec!["9x".to_string()], "y", "9x::y"),
        ] {
            assert_eq!(encode_scoped(&scope, simple), encode_ident(fqn));
        }
    }

    #[test]
    fn output_is_always_a_valid_identifier() {
        for raw in ["Foo", "A::B_C", "9lives", "a-b.c", "a b", "™weird", ""] {
            let enc = encode_ident(raw);
            if enc.is_empty() {
                continue;
            }
            let first = enc.as_bytes()[0];
            assert!(
                first == b'_' || first.is_ascii_alphabetic(),
                "identifier {enc:?} must start with a letter or underscore"
            );
            assert!(
                enc.bytes().all(|b| b == b'_' || b.is_ascii_alphanumeric()),
                "identifier {enc:?} must be [A-Za-z0-9_]"
            );
        }
    }

    #[test]
    fn injective_over_a_small_corpus() {
        // A brute-force injectivity check over ambiguity-prone inputs.
        let corpus = [
            "A::B_C", "A_B::C", "A::B::C", "A_B_C", "a-b", "a_b", "a.b", "Foo", "foo", "9x", "x9",
            "A::b", "A_b", "a::B", "a_B", "m::n::p", "m_n::p", "m::n_p", "m_n_p",
        ];
        let encoded: Vec<String> = corpus.iter().map(|s| encode_ident(s)).collect();
        for i in 0..encoded.len() {
            for j in (i + 1)..encoded.len() {
                assert_ne!(
                    encoded[i], encoded[j],
                    "collision: {:?} and {:?} both -> {}",
                    corpus[i], corpus[j], encoded[i]
                );
            }
        }
    }
}
