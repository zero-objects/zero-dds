// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XTypes 1.3 §7.5.1.2.1 (enumerated literal values) and §7.5.1.2.2
//! (bitmask types) in the C, C++ and Java columns.
//!
//! The expected texts come from the clauses: an explicit `@value` is the
//! number after `=` in C++ (§7.5.1.2.1.2), a Java literal is
//! `public static final int _<label> = <value>;` (§7.5.1.2.1.3), a bitmask
//! is "as if" an enumeration `<TypeName>Bits` with `@value(1 << position)`
//! and a typedef of the unsigned integer of Table 44.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_lang_cst::common::Language;
use zerodds_lang_rules::generate::declarations;

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("the model resolves");
    ast
}

fn text(idl: &str, language: Language) -> String {
    declarations(&model(idl), language).unwrap_or_else(|why| panic!("{language:?}: {why:?}"))
}

const MY_FLAGS: &str =
    "@bit_bound(19) bitmask MyFlags { FIRST_FLAG, @position(14) SECOND_FLAG, THIRD_FLAG };";

// -- §7.5.1.2.1.1 C ----------------------------------------------------

#[test]
fn c_defines_carry_the_explicit_and_the_implicit_value() {
    let c = text("enum Mode { IDLE, @value(4) ACTIVE, FAULT };", Language::C);
    assert!(c.contains("#define IDLE ((Mode)0)"), "{c}");
    assert!(c.contains("#define ACTIVE ((Mode)4)"), "{c}");
    assert!(c.contains("#define FAULT ((Mode)5)"), "{c}");
}

// -- §7.5.1.2.1.2 C++ --------------------------------------------------

#[test]
fn cpp_follows_an_enumerator_with_a_value_by_an_equals_sign_and_the_value() {
    let cpp = text(
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT, @value(70000) HIGH };",
        Language::Cpp,
    );
    assert!(cpp.contains("ACTIVE = 4"), "{cpp}");
    assert!(cpp.contains("HIGH = 70000"), "{cpp}");
    assert!(
        cpp.contains("FAULT = 5"),
        "the implicit value after 4: {cpp}"
    );
}

// -- §7.5.1.2.1.3 Java -------------------------------------------------

#[test]
fn java_has_an_int_constant_with_an_underscore_and_an_object_per_literal() {
    // the member is a public field: the IDL-to-Java mapping, by name
    let java = zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        || {
            text(
                "enum Mode { IDLE, @value(4) ACTIVE, FAULT };\n@final struct P { Mode m; };",
                Language::Java,
            )
        },
    );
    assert!(
        java.contains("public static final int _IDLE = 0;"),
        "{java}"
    );
    assert!(
        java.contains("public static final int _ACTIVE = 4;"),
        "{java}"
    );
    assert!(
        java.contains("public static final int _FAULT = 5;"),
        "{java}"
    );
    assert!(
        java.contains("public static final Mode ACTIVE = new Mode(_ACTIVE);"),
        "the object of [JAVA-MAP] per literal: {java}"
    );
    assert!(java.contains("public int value()"), "{java}");
    assert!(
        java.contains("public static Mode from_int(int value)"),
        "{java}"
    );
    assert!(
        java.contains("public ZdTypes.Mode m;"),
        "a member is the enumeration class (CORBA 3.3 part 3 §1.5.9): {java}"
    );
}

#[test]
fn under_the_default_the_enumeration_is_the_same_and_its_member_is_private_with_accessors() {
    let java = text(
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };\n@final struct P { Mode m; };",
        Language::Java,
    );
    assert!(
        java.contains("public static final int _IDLE = 0;"),
        "{java}"
    );
    assert!(
        java.contains("public static final int _ACTIVE = 4;"),
        "{java}"
    );
    assert!(
        java.contains("public static final int _FAULT = 5;"),
        "{java}"
    );
    assert!(
        java.contains("public static final Mode ACTIVE = new Mode(_ACTIVE);"),
        "the object of [JAVA-MAP] per literal: {java}"
    );
    assert!(java.contains("public int value()"), "{java}");
    assert!(
        java.contains("public static Mode from_int(int value)"),
        "{java}"
    );
    // DDS-Java-PSM 1.0 9.1.1: the aggregate has private state and bean accessors; the
    // enumeration itself is not addressed there and stays the class of the IDL-to-Java mapping
    assert!(java.contains("private ZdTypes.Mode m;"), "{java}");
    assert!(java.contains("public ZdTypes.Mode getM()"), "{java}");
    assert!(java.contains("public void setM(ZdTypes.Mode v)"), "{java}");
    assert!(!java.contains("public ZdTypes.Mode m;"), "{java}");
}

// -- §7.5.1.2.2 Bitmask, Table 44, the example --------------------------

#[test]
fn table_44_one_to_eight_bits_is_an_8_bit_unsigned_integer() {
    let src = "@bit_bound(8) bitmask F { A, @position(7) B };";
    let c = text(src, Language::C);
    assert!(c.contains("typedef uint8_t F;"), "{c}");
    assert!(c.contains("typedef uint8_t FBits;"), "{c}");
    let cpp = text(src, Language::Cpp);
    assert!(cpp.contains("F = std::uint8_t"), "{cpp}");
    assert!(cpp.contains("enum class FBits : std::uint8_t"), "{cpp}");
    let java = text(src, Language::Java);
    assert!(java.contains("class FBits"), "{java}");
}

#[test]
fn table_44_nine_to_sixteen_bits_is_an_unsigned_short() {
    let src = "@bit_bound(16) bitmask F { A, @position(15) B };";
    assert!(text(src, Language::C).contains("typedef uint16_t F;"));
    assert!(text(src, Language::Cpp).contains("F = std::uint16_t"));
}

#[test]
fn table_44_seventeen_to_thirty_two_bits_is_an_unsigned_long() {
    let src = "@bit_bound(32) bitmask F { A, @position(31) B };";
    let c = text(src, Language::C);
    assert!(c.contains("typedef uint32_t F;"), "{c}");
    assert!(
        c.contains("#define B ((FBits)1 << 31)"),
        "bit 31 in the 32-bit holder: {c}"
    );
    let cpp = text(src, Language::Cpp);
    assert!(cpp.contains("F = std::uint32_t"), "{cpp}");
    assert!(cpp.contains("enum class FBits : std::uint32_t"), "{cpp}");
    assert!(cpp.contains("B = 2147483648"), "{cpp}");
    let java = text(src, Language::Java);
    assert!(
        java.contains("public static final long _B = 1L << 31;"),
        "Java holds a 32-bit unsigned in a long: {java}"
    );
}

#[test]
fn table_44_thirty_three_to_sixty_four_bits_is_an_unsigned_long_long() {
    let src = "@bit_bound(64) bitmask F { A, @position(63) B };";
    let c = text(src, Language::C);
    assert!(c.contains("typedef uint64_t F;"), "{c}");
    assert!(c.contains("#define B ((FBits)1 << 63)"), "{c}");
    let cpp = text(src, Language::Cpp);
    assert!(cpp.contains("F = std::uint64_t"), "{cpp}");
    assert!(cpp.contains("enum class FBits : std::uint64_t"), "{cpp}");
    assert!(cpp.contains("B = 9223372036854775808u"), "{cpp}");
}

#[test]
fn the_example_of_the_clause_myflags_binds_as_the_text_shows() {
    // "@bit_bound(19) bitmask MyFlags { FIRST_FLAG, @position(14)
    // SECOND_FLAG, THIRD_FLAG };" is bound "as if" the enumeration
    // MyFlagsBits with the values 1 << 0, 1 << 14 and 1 << 15 and
    // "typedef unsigned long MyFlags;" (19 bits fall in 17-32).
    let c = text(MY_FLAGS, Language::C);
    assert!(c.contains("typedef uint32_t MyFlags;"), "{c}");
    assert!(c.contains("typedef uint32_t MyFlagsBits;"), "{c}");
    assert!(
        c.contains("#define FIRST_FLAG ((MyFlagsBits)1 << 0)"),
        "{c}"
    );
    assert!(
        c.contains("#define SECOND_FLAG ((MyFlagsBits)1 << 14)"),
        "{c}"
    );
    assert!(
        c.contains("#define THIRD_FLAG ((MyFlagsBits)1 << 15)"),
        "{c}"
    );

    let cpp = text(MY_FLAGS, Language::Cpp);
    assert!(cpp.contains("MyFlags = std::uint32_t"), "{cpp}");
    assert!(
        cpp.contains("enum class MyFlagsBits : std::uint32_t"),
        "{cpp}"
    );
    assert!(cpp.contains("FIRST_FLAG = 1"), "{cpp}");
    assert!(cpp.contains("SECOND_FLAG = 16384"), "1 << 14: {cpp}");
    assert!(cpp.contains("THIRD_FLAG = 32768"), "1 << 15: {cpp}");

    let java = text(MY_FLAGS, Language::Java);
    assert!(java.contains("class MyFlagsBits"), "{java}");
    assert!(
        java.contains("public static final long _SECOND_FLAG = 1L << 14;"),
        "{java}"
    );
    assert!(
        java.contains("public static final long _THIRD_FLAG = 1L << 15;"),
        "{java}"
    );
    assert!(
        java.contains("public static final MyFlagsBits THIRD_FLAG = new MyFlagsBits(_THIRD_FLAG);"),
        "{java}"
    );
}

#[test]
fn a_type_named_like_the_flag_enumeration_is_a_collision_in_every_column() {
    let src = "bitmask F { A, B };\nstruct FBits { long a; };";
    for language in [Language::C, Language::Cpp, Language::Java] {
        let result = declarations(&model(src), language);
        assert!(
            result.is_err(),
            "{language:?} wrote two declarations named FBits"
        );
    }
}
