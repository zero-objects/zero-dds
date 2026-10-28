// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The CORBA column: what the printer prints, and what it refuses.
//!
//! XCDR1 *is* CORBA CDR. The three ORBs on the Linux test host are a
//! reference for the whole XCDR1 base, and the one thing that has to be
//! right before any of their bytes mean anything is the IDL they are
//! handed: a construct that has no CORBA form must be refused **by
//! name**, never quietly reshaped into one that compiles.
//!
//! These are the tests that would fail if it started being quietly
//! reshaped. The bytes themselves are in
//! `crates/ast-layout/tests/fixtures/vendor_bytes.md` §7, because those
//! come from the ORBs and not from here.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod corba_corpus;

use zerodds_wire_probe::corba_cpp::{dump_corba_cpp, fill_corba_cpp};
use zerodds_wire_probe::corba_idl::{corba_idl, cpp_name, java_name};
use zerodds_wire_probe::corba_java::{dump_corba_java, fill_corba_java};
use zerodds_wire_probe::encode::model_from_idl;

/// The model for one snippet of IDL, and the type named `Sample`.
fn model(
    source: &str,
) -> (
    zerodds_ast::Ast,
    zerodds_ast_core::Id<zerodds_ast::TypeNode>,
) {
    let ast = model_from_idl(source, "test.idl").unwrap_or_else(|error| panic!("{error}"));
    let type_ = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![String::from(
            "Sample",
        )]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("Sample is not in the model"));
    (ast, type_)
}

fn idl_of(source: &str) -> String {
    let (ast, type_) = model(source);
    corba_idl(&ast, type_).unwrap_or_else(|error| panic!("{error}"))
}

fn refusal(source: &str) -> String {
    let (ast, type_) = model(source);
    match corba_idl(&ast, type_) {
        Ok(text) => panic!("expected a refusal, got:\n{text}"),
        Err(error) => format!("{error}"),
    }
}

#[test]
fn the_extensibility_annotations_are_dropped_and_nothing_else_changes() {
    // Rule (17) makes `@final` the members in declaration order with no
    // framing, and rule (29) makes `@appendable` the same bytes under
    // encoding version 1 — inventory row H2, three vendors, 18 bytes
    // both. A CORBA `struct` is that shape, so both annotations go and
    // the wire does not move.
    let final_ = idl_of("@final struct Sample { long a; double b; };");
    let appendable = idl_of("@appendable struct Sample { long a; double b; };");
    let bare = idl_of("struct Sample { long a; double b; };");
    assert_eq!(final_, bare);
    assert_eq!(appendable, bare);
    assert!(bare.contains("struct Sample"), "{bare}");
    assert!(!bare.contains('@'), "an annotation survived: {bare}");
}

#[test]
fn a_mutable_type_is_refused_because_its_framing_is_xtypes_and_not_corba() {
    // Rule (23)'s parameter list is XTypes'. Printing a `@mutable` type
    // as a plain CORBA `struct` would hand the ORB a different type and
    // record its bytes in a `@mutable` row.
    let message = refusal("@mutable struct Sample { @id(1) long a; };");
    assert!(message.contains("@mutable"), "{message}");
    assert!(message.contains("PL_CDR1"), "{message}");
}

#[test]
fn an_optional_member_is_refused_because_its_presence_marker_is_xtypes() {
    // Rules (19) and (20): a header of length zero under XCDR1, a
    // BOOLEAN `<is_present>` under XCDR2. CORBA has neither, and a
    // member printed without the annotation is a member that is always
    // there.
    let message = refusal("@final struct Sample { long a; @optional octet b; };");
    assert!(message.contains("@optional"), "{message}");
}

#[test]
fn the_annotations_with_no_wire_effect_are_dropped_rather_than_refused() {
    // `@key` is the key holder's business (§7.6.8) and puts nothing in
    // the payload; `@external` is given no wire effect at all by
    // §7.3.1.2.1.10; `@try_construct` and `@default` are the receiver's
    // answers and change nothing a writer emits. Refusing them would
    // cost cases for a difference that is not on the wire.
    let printed = idl_of(
        "@final struct Sample { @key long id; @external long b; \
         @try_construct(TRIM) string<4> s; @default(7) long d; };",
    );
    assert_eq!(
        printed, "struct Sample {\n  long id;\n  long b;\n  string<4> s;\n  long d;\n};\n",
        "{printed}"
    );
}

#[test]
fn int8_and_uint8_become_octet_because_table_31_gives_all_three_one_octet() {
    // All three front ends refuse the IDL 4.2 keywords — measured
    // 2026-08-10. Table 31 gives Int8, UInt8 and Byte size 1 and
    // alignment 1, and §9.3.1.1 gives `octet` the same, so the octet on
    // the wire is the same octet whichever names it. What differs is
    // only how the value is read back, which is the fill's and the
    // dump's job.
    let printed = idl_of("@final struct Sample { int8 a; uint8 b; octet c; };");
    assert_eq!(
        printed, "struct Sample {\n  octet a;\n  octet b;\n  octet c;\n};\n",
        "{printed}"
    );
}

#[test]
fn a_signed_byte_goes_out_as_its_octet_and_comes_back_signed() {
    // The other half of the mapping above. A `-5` written as an
    // unsigned literal would be a compile error in Java and a narrowing
    // in C++; read back unsigned it would be 251, which is a value
    // nobody wrote.
    let (ast, type_) = model("@final struct Sample { int8 a; uint8 b; };");
    let values = serde_json::json!({"a": -5, "b": 251});
    let cpp = fill_corba_cpp(&ast, type_, &values).unwrap_or_else(|error| panic!("{error}"));
    assert!(cpp.contains("(CORBA::Octet)0xfb"), "{cpp}");
    let java = fill_corba_java(&ast, type_, &values).unwrap_or_else(|error| panic!("{error}"));
    assert!(java.contains("(byte)0xfb"), "{java}");

    let cpp_back = dump_corba_cpp(&ast, type_).unwrap_or_else(|error| panic!("{error}"));
    assert!(cpp_back.contains("(signed char)(sample.a)"), "{cpp_back}");
    assert!(cpp_back.contains("(unsigned char)(sample.b)"), "{cpp_back}");
    let java_back = dump_corba_java(&ast, type_).unwrap_or_else(|error| panic!("{error}"));
    assert!(
        java_back.contains("Zdj.i64((long)(sample.a))"),
        "{java_back}"
    );
    assert!(java_back.contains("0xFFL"), "{java_back}");
}

#[test]
fn an_enumeration_with_a_bit_bound_or_an_explicit_value_is_refused() {
    // CORBA 3.3 part 2 §9.3.2.5 makes an enumeration an unsigned long
    // whose value is the enumerator's ordinal position. Both `@value`
    // and `@bit_bound` change the wire — inventory rows E1 and E2 — so
    // an enumeration that uses either is refused rather than printed
    // without it.
    let bound = refusal("@bit_bound(8) enum E { A, B }; @final struct Sample { E choice; };");
    assert!(bound.contains("@bit_bound"), "{bound}");
    let value = refusal("enum E { A, @value(7) B }; @final struct Sample { E choice; };");
    assert!(value.contains("@value"), "{value}");
}

#[test]
fn the_two_xtypes_only_aggregates_are_refused_by_name_and_a_map_prints() {
    for (source, needle) in [
        (
            "@bit_bound(8) bitmask M { @position(0) LOW }; @final struct Sample { M marks; };",
            "bitmask",
        ),
        (
            "bitset B { bitfield<3> low; }; @final struct Sample { B packed; };",
            "bitset",
        ),
    ] {
        let message = refusal(source);
        assert!(message.contains(needle), "{needle}: {message}");
    }
    // `map<K, V>` is IDL 4.2 §7.4.1.4.4.4.4, not an XTypes-only
    // aggregate: it prints, and the ORB front end answers for it —
    // TAO 3.x generates `std::map`, measured over the matrix map
    // cases; the C++ backend fills and dumps it.
    let printed = idl_of("@final struct Sample { map<long, long> m; };");
    assert!(printed.contains("map<long, long>"), "{printed}");
}

#[test]
fn a_structure_with_a_base_is_refused_because_corba_idl_has_no_syntax_for_one() {
    // Measured 2026-08-10 on the Linux test host: omniidl 1.0 "Syntax
    // error in struct definition", tao_idl 2.5.24 "Missing '{' or
    // illegal syntax following struct identifier", JacORB 3.9 "Syntax
    // error". Struct inheritance is IDL 4.2.
    let message =
        refusal("@final struct Base { long a; }; @final struct Sample : Base { long b; };");
    assert!(message.contains("base"), "{message}");
}

#[test]
fn a_long_double_is_refused_rather_than_mapped_to_the_platforms_own() {
    // §9.3.1.3 Figure 9.2 specifies binary128 and the C++ mapping gives
    // the platform's `long double`, which on x86-64 is x87 80-bit
    // extended in a sixteen-byte object. Measured: omniORB and TAO both
    // write the x87 pattern, and JacORB maps the type to a Java
    // `double` and writes **eight** bytes. Folding that into a corpus
    // case would make every member after it unreadable.
    let message = refusal("@final struct Sample { long double d; };");
    assert!(message.contains("long double"), "{message}");
    assert!(message.contains("binary128"), "{message}");
}

#[test]
fn an_anonymous_collection_in_an_element_position_is_given_a_typedef() {
    // `sequence<sequence<long>>` is a syntax error in omniidl and in
    // JacORB alike, and `sequence<long[3]>` is not IDL at all: an array
    // puts part of itself after the name. IDL takes a name in that
    // position and nothing else, so the printer supplies one — rule (7)
    // makes a typedef transparent on the wire (row F1), so the name
    // costs no byte.
    let nested = idl_of("@final struct Sample { sequence<sequence<long>> rows; };");
    assert!(
        nested.contains("typedef sequence<long> zd_anon_1;"),
        "{nested}"
    );
    assert!(nested.contains("sequence<::zd_anon_1> rows;"), "{nested}");

    // A member whose own type is an array keeps the dimensions behind
    // the name, which is where IDL puts them, and needs no typedef.
    let member = idl_of("@final struct Sample { long items[3]; };");
    assert_eq!(
        member, "struct Sample {\n  long items[3];\n};\n",
        "{member}"
    );
}

#[test]
fn a_case_label_is_spelled_in_the_discriminators_own_language() {
    // An integer label compiles against a `long` discriminator and
    // fails against an `enum` one; against a `char` one it compiles and
    // selects a different case.
    let enumerated = idl_of(
        "enum Colour { RED, GREEN, BLUE };\n\
         union Choice switch (Colour) { case GREEN: long a; default: short b; };\n\
         @final struct Sample { Choice picked; };",
    );
    assert!(enumerated.contains("case ::GREEN:"), "{enumerated}");
    assert!(enumerated.contains("default:"), "{enumerated}");

    let boolean = idl_of(
        "union Choice switch (boolean) { case TRUE: long a; default: short b; };\n\
         @final struct Sample { Choice picked; };",
    );
    assert!(boolean.contains("case TRUE:"), "{boolean}");

    // IDL 4.2 §7.2.6.2's escapes, not Rust's `\u{e4}`, which no front
    // end reads.
    //
    // The label is written `'\xe4'` here and not `'ä'`, and that is a
    // finding of its own rather than a convenience: this project's own
    // lexer reads a raw non-ASCII character inside a `char` literal as
    // the **last byte of its UTF-8 encoding**, so `'ä'` (C3 A4) becomes
    // U+00A4 and not U+00E4. Measured 2026-08-10. It is upstream of the
    // printer — the value in the model is already wrong — and it is
    // written down in `docs/corba-column.md` rather than worked around
    // here, because a printer that "corrected" it would hide it.
    let character = idl_of(
        "union Choice switch (char) { case '\\xe4': long a; default: short b; };\n\
         @final struct Sample { Choice picked; };",
    );
    assert!(character.contains("case '\\xe4':"), "{character}");
}

#[test]
fn a_module_becomes_a_namespace_in_cpp_and_a_package_in_java() {
    let source = "module common { @final struct Sample { long a; }; };";
    let ast = model_from_idl(source, "test.idl").unwrap_or_else(|error| panic!("{error}"));
    let type_ = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            String::from("common"),
            String::from("Sample"),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("common::Sample is not in the model"));
    assert_eq!(cpp_name(&ast, type_), "common::Sample");
    assert_eq!(java_name(&ast, type_), "common.Sample");
    let printed = corba_idl(&ast, type_).unwrap_or_else(|error| panic!("{error}"));
    assert!(printed.contains("module common {"), "{printed}");
}

#[test]
fn a_union_sets_its_discriminator_after_the_member_in_both_mappings() {
    // §1.2.9 and §1.5.11 both make the one-argument modifier set the
    // discriminator to the case's *first* label. A sample whose `_d` is
    // the second label of a two-label case, or a value that reaches
    // `default:`, would otherwise carry a discriminator nobody wrote.
    let (ast, type_) = model(
        "union Choice switch (long) { case 1: long one; case 2: case 3: double two; };\n\
         @final struct Sample { Choice picked; };",
    );
    let values = serde_json::json!({"picked": {"_d": 3, "two": 1.5}});

    let cpp = fill_corba_cpp(&ast, type_, &values).unwrap_or_else(|error| panic!("{error}"));
    let member = cpp
        .find("two(")
        .unwrap_or_else(|| panic!("no member set:\n{cpp}"));
    let discriminator = cpp
        .find("_d(")
        .unwrap_or_else(|| panic!("no _d set:\n{cpp}"));
    assert!(
        member < discriminator,
        "the discriminator came first:\n{cpp}"
    );

    // Java has no `_d`; §1.5.11's second modifier takes the
    // discriminator beside the value, and it is the only way in.
    let java = fill_corba_java(&ast, type_, &values).unwrap_or_else(|error| panic!("{error}"));
    assert!(java.contains(".two(0x00000003,"), "{java}");
}

#[test]
fn a_discriminator_that_selects_no_member_still_reaches_the_union() {
    // §1.5.11 gives the union `__default(int)` for exactly this, and
    // there is no other way in — every modifier is attached to a
    // member. omniORB writes `2a000000` and nothing else for this
    // sample; measured 2026-08-10, which is what closes inventory row
    // J3.
    let (ast, type_) = model(
        "union Choice switch (long) { case 1: long one; };\n\
         @final struct Sample { Choice picked; };",
    );
    let values = serde_json::json!({"picked": {"_d": 42}});
    let java = fill_corba_java(&ast, type_, &values).unwrap_or_else(|error| panic!("{error}"));
    assert!(java.contains("__default(0x0000002a)"), "{java}");
    let cpp = fill_corba_cpp(&ast, type_, &values).unwrap_or_else(|error| panic!("{error}"));
    assert!(cpp.contains("_d(42)"), "{cpp}");
}

#[test]
fn a_wstring_goes_in_as_corba_wchar_units_and_never_as_a_wide_literal() {
    // `wchar_t` is four bytes on Linux and `CORBA::WChar` is two in
    // omniORB, so an `L"…"` literal is the wrong array against one of
    // the two ORBs — and with a cast in front of it, wrong without
    // saying so.
    let (ast, type_) = model("@final struct Sample { wstring s; };");
    let values = serde_json::json!({"s": "aä"});
    let cpp = fill_corba_cpp(&ast, type_, &values).unwrap_or_else(|error| panic!("{error}"));
    assert!(cpp.contains("static CORBA::WChar"), "{cpp}");
    assert!(cpp.contains("0x0061, 0x00e4, 0x0000"), "{cpp}");
    assert!(
        !cpp.contains("L\""),
        "a wide literal reached the emitter:\n{cpp}"
    );
}

#[test]
fn a_string_literal_escapes_every_non_ascii_byte_in_octal() {
    // `\xHH` is greedy in C++, so `"\xc3a"` reads as one character with
    // an out-of-range value: a UTF-8 string followed by a hex digit
    // would change meaning. Three-digit octal cannot run on.
    let (ast, type_) = model("@final struct Sample { string s; };");
    let values = serde_json::json!({"s": "äa"});
    let cpp = fill_corba_cpp(&ast, type_, &values).unwrap_or_else(|error| panic!("{error}"));
    assert!(cpp.contains("\"\\303\\244a\""), "{cpp}");

    // Java reads its source in the platform encoding unless told
    // otherwise, so a probe whose string depended on a locale would
    // produce different bytes on two machines.
    let java = fill_corba_java(&ast, type_, &values).unwrap_or_else(|error| panic!("{error}"));
    assert!(java.contains("\"\\u00e4a\""), "{java}");
}

#[test]
fn every_corpus_case_either_prints_or_says_which_construct_stopped_it() {
    // The whole corpus, so a construct that has no CORBA form is
    // refused by name rather than reaching an ORB's front end, which
    // would refuse it for a reason the run then misreads. The counts are
    // checked against the headline of `docs/corba-column.md` in
    // `corba_column_headline.rs`.
    let (printed, refused) = corba_corpus::print_or_refuse_every_case();
    assert!(
        printed > 0 && refused > 0,
        "the corpus pass printed {printed} and refused {refused}"
    );
}
