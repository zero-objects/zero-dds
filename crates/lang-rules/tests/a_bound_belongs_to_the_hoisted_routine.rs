// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A collection's bound belongs to the name of the routine its codec is
//! hoisted into.
//!
//! Every column below emits one codec body per routine name, and the
//! name comes from a `type_tag`. With the bound left out of the tag,
//! `sequence<long>` and `sequence<long, 4>` are one name, so one body
//! serves both members and the member declared first decides which one
//! it is. Measured, before the fix, on
//! `@final struct S { sequence<long> b; sequence<long, 4> a; };`: the
//! generated source held no `zd_over` check at all, so `a` accepted a
//! stream of any length. With the two members the other way round both
//! got the bounded body, so `b` refused — and truncated — a length the
//! IDL puts no limit on. `string<4>` beside `string` and `wstring<4>`
//! beside `wstring` collapsed the same way, onto the runtime's
//! `utf8_octets` and `utf16_units` measure. Either way the reader
//! answered with a value the IDL does not describe and said nothing.
//!
//! Both declaration orders are here because the two produce different
//! wrong answers and only one of them is visible as a missing check.
//!
//! `a_map_over_its_bound_is_refused` holds a second and separate
//! defect: a `map`'s bound was checked in no column at all, whatever
//! the name. It runs in every column that emits a codec through
//! `emit/decode.rs` — the seventeen below plus the three under
//! [`map_check_column`], which is all twenty. The Rust column writes
//! its collections inline and is not one of them; `encode::rust_codec`
//! carries that as a reason.
//!
//! The Ada, Ada 83 and Zig columns carry the naming fix and test it in
//! `ada_end_to_end.rs`, `ada83_end_to_end.rs` and `zig_end_to_end.rs`.
//! An `array` needs nothing: its dimensions have always been in the
//! tag.
#![allow(clippy::expect_used, clippy::panic)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

/// One IDL source through one column, as the text the generator writes.
fn source(idl: &str, language: Language) -> String {
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
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");
    zerodds_lang_rules::generate::source(
        &ast,
        language,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    )
    .expect("a codec")
}

/// How often the text calls the runtime's UTF-8 measure, whatever the
/// column spells it — `utf8_octets` in most, `utf8Octets` in the ones
/// that camel-case a runtime name. Underscores and case are folded out
/// so one count reads every column.
fn utf8_measures(text: &str) -> usize {
    text.to_lowercase()
        .replace('_', "")
        .matches("utf8octets")
        .count()
}

/// The same for the UTF-16 measure a `wstring` bound is checked with.
fn utf16_measures(text: &str) -> usize {
    text.to_lowercase()
        .replace('_', "")
        .matches("utf16units")
        .count()
}

macro_rules! column {
    ($name:ident, $language:expr) => {
        mod $name {
            use super::{source, utf8_measures, utf16_measures};
            use zerodds_lang_cst::Language;

            const LANGUAGE: Language = $language;

            /// The bounded sequence keeps its length check beside an
            /// unbounded sequence of the same element, and the
            /// unbounded one gets none forced on it.
            ///
            /// `zd_over` twice is one bounded body: the guard and the
            /// narrowing that reads it. Zero of them is the bound gone
            /// altogether, which is what the first order produced.
            #[test]
            fn a_sequence_bound_survives_an_unbounded_sequence_of_the_same_element() {
                for idl in [
                    "@final struct S { sequence<long> b; sequence<long, 4> a; };",
                    "@final struct S { sequence<long, 4> a; sequence<long> b; };",
                ] {
                    let out = source(idl, LANGUAGE);
                    assert!(
                        out.contains("seq_b4_long"),
                        "the bounded sequence shares the unbounded one's routine\n{idl}\n{out}"
                    );
                    assert_eq!(
                        out.matches("zd_over").count(),
                        2,
                        "one of the two routines checks a length, and only one\n{idl}\n{out}"
                    );
                }
            }

            /// The same for a `string`, whose bound is carried the same
            /// way and checked through the runtime's UTF-8 measure.
            #[test]
            fn a_string_bound_survives_an_unbounded_string_beside_it() {
                for idl in [
                    "@final struct S { string b; string<4> a; };",
                    "@final struct S { string<4> a; string b; };",
                ] {
                    let out = source(idl, LANGUAGE);
                    assert!(
                        out.contains("string_b4"),
                        "the bounded string shares the unbounded one's routine\n{idl}\n{out}"
                    );
                    assert_eq!(
                        utf8_measures(&out),
                        1,
                        "one of the two routines measures the value, and only one\n{idl}\n{out}"
                    );
                }
            }

            /// A sequence's bound and its element's bound are two
            /// routines. `sequence<string, 4>` and `sequence<string<4> >`
            /// both answered `seq_string_b4` and shared one body, so one
            /// of the two lost its check: the length (`zd_over` twice) or
            /// the string's measure.
            #[test]
            fn a_collection_bound_and_its_element_bound_are_two_routines() {
                for idl in [
                    "@final struct S { sequence<string, 4> a; sequence<string<4> > b; };",
                    "@final struct S { sequence<string<4> > b; sequence<string, 4> a; };",
                ] {
                    let out = source(idl, LANGUAGE);
                    assert_eq!(
                        out.matches("zd_over").count(),
                        2,
                        "the sequence's length check is gone\n{idl}\n{out}"
                    );
                    assert_eq!(
                        utf8_measures(&out),
                        1,
                        "the element's measure is gone\n{idl}\n{out}"
                    );
                }
            }

            /// And for a `wstring`, whose bound counts UTF-16 code
            /// units rather than octets (DDS-XTypes 1.3 §7.4.1.1.2).
            #[test]
            fn a_wstring_bound_survives_an_unbounded_wstring_beside_it() {
                for idl in [
                    "@final struct S { wstring b; wstring<4> a; };",
                    "@final struct S { wstring<4> a; wstring b; };",
                ] {
                    let out = source(idl, LANGUAGE);
                    assert!(
                        out.contains("wstring_b4"),
                        "the bounded wstring shares the unbounded one's routine\n{idl}\n{out}"
                    );
                    assert_eq!(
                        utf16_measures(&out),
                        1,
                        "one of the two routines measures the value, and only one\n{idl}\n{out}"
                    );
                }
            }

            /// A `map` bound rides in the same tag, and the routine it
            /// now has to itself carries the length check
            /// `a_map_over_its_bound_is_refused` measures.
            #[test]
            fn a_map_bound_survives_an_unbounded_map_beside_it() {
                for idl in [
                    "@final struct S { map<long, long> b; map<long, long, 4> a; };",
                    "@final struct S { map<long, long, 4> a; map<long, long> b; };",
                ] {
                    let out = source(idl, LANGUAGE);
                    // The C column names the type `Int32Int32Map_b4`.
                    assert!(
                        out.contains("map_b4_") || out.contains("Map_b4"),
                        "the bounded map shares the unbounded one's routine\n{idl}\n{out}"
                    );
                    assert_eq!(
                        out.matches("zd_over").count(),
                        2,
                        "one of the two routines checks a length, and only one\n{idl}\n{out}"
                    );
                }
            }

            map_check!();
        }
    };
}

/// A `map` over its bound is refused, and an unbounded one is not
/// constrained.
///
/// The second defect, and separate from the naming one: `emit/decode.rs`
/// guarded a `sequence` and a `string` and walked a map's entries
/// without ever consulting its bound, in every column. Measured on
/// `@final struct S { map<long, long, 4> a; };`: the generated source
/// held no `zd_over` at all, so a stream stating any number of entries
/// was read into the sample and handed to the application with nothing
/// said about it.
///
/// Two `zd_over` are one checked routine — the guard and the narrowing
/// that reads it — which is the same count a bounded `sequence` gives.
/// The unbounded map beside it in the second case is what says the
/// check is the bound's and not something every map now carries.
macro_rules! map_check {
    () => {
        /// A collection's bound and the bound of the collection inside it
        /// are two routines. With the bound written after the element,
        /// `sequence<sequence<long>, 4>` and `sequence<sequence<long, 4> >`
        /// ended in one tag, and the member declared first decided which
        /// of the two lengths was checked. Two bounded bodies are four
        /// `zd_over`.
        #[test]
        fn a_nested_sequence_keeps_each_bound_at_its_level() {
            for idl in [
                "@final struct S { sequence<sequence<long>, 4> a; sequence<sequence<long, 4> > b; };",
                "@final struct S { sequence<sequence<long, 4> > b; sequence<sequence<long>, 4> a; };",
            ] {
                let out = source(idl, LANGUAGE);
                assert_eq!(
                    out.matches("zd_over").count(),
                    4,
                    "the outer and the inner bound share one routine\n{idl}\n{out}"
                );
            }
        }

        #[test]
        fn a_map_over_its_bound_is_refused() {
            let bounded = source("@final struct S { map<long, long, 4> a; };", LANGUAGE);
            assert_eq!(
                bounded.matches("zd_over").count(),
                2,
                "a map over its bound is read as it came\n{bounded}"
            );
            let unbounded = source("@final struct S { map<long, long> a; };", LANGUAGE);
            assert_eq!(
                unbounded.matches("zd_over").count(),
                0,
                "an unbounded map is held to a length the IDL does not state\n{unbounded}"
            );
        }
    };
}

/// The map check alone, for a column whose tag spells a collection
/// differently — the Ada pair write `Seq_Of_Long_B4` — or which the
/// naming half of this file already covers elsewhere.
macro_rules! map_check_column {
    ($name:ident, $language:expr) => {
        mod $name {
            use super::source;
            use zerodds_lang_cst::Language;

            const LANGUAGE: Language = $language;

            map_check!();
        }
    };
}

column!(go, Language::Go);
column!(d, Language::D);
column!(nim, Language::Nim);
column!(lua, Language::Lua);
column!(elixir, Language::Elixir);
column!(swift, Language::Swift);
column!(ocaml, Language::OCaml);
column!(node, Language::Node);
column!(julia, Language::Julia);
column!(c, Language::C);
column!(cpp, Language::Cpp);
column!(csharp, Language::CSharp);
column!(java, Language::Java);
column!(kotlin, Language::Kotlin);
column!(python, Language::Python);
column!(typescript, Language::TypeScript);
column!(fsharp, Language::FSharp);

map_check_column!(ada, Language::Ada);
map_check_column!(ada83, Language::Ada83);
map_check_column!(zig, Language::Zig);
