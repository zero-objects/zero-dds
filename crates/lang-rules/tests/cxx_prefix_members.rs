// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.11.1.1.2 rule 8 for the names inside a declaration.
//!
//! "Any identifier (e.g., parameter, operation name, exception, interface,
//! module) in an IDL file that is a reserved word in C++ shall be prefixed
//! by `cxx_`." The C++ column used to append an underscore to a member, an
//! enumerator and a constant (`static_`), so the same identifier had two
//! spellings depending on whether it named a type or a member. Every
//! reference to the name has to agree with its declaration: the member
//! itself, the codec that reads and writes it, the default of an
//! enumerator, and the `_idl_names` table that maps it back to the IDL
//! spelling.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::common::Language;
use zerodds_lang_rules::generate;

const IDL: &str = "
module cx {
  const long register = 3;
  enum Colour { delete, keep };
  @final struct Settings { long static; Colour tint; };
  exception Failure { long new; };
  union Choice switch (long) { case 1: long operator; case 2: short volatile; };
};
";

fn header() -> String {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(
        parsed.diagnostics().is_empty(),
        "parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "cx.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "lower: {:?}",
        lowered.diagnostics
    );
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    generate::source(
        &ast,
        Language::Cpp,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    )
    .unwrap_or_else(|error| panic!("C++ refused the source: {error}"))
}

#[test]
fn a_member_that_is_a_reserved_word_is_prefixed_by_cxx() {
    let text = header();
    assert!(text.contains("std::int32_t cxx_static"), "{text}");
    // A union case is an accessor over the storage `zd_<case>`.
    assert!(text.contains("std::int32_t zd_cxx_operator;"), "{text}");
    assert!(text.contains("std::int16_t zd_cxx_volatile;"), "{text}");
    assert!(text.contains("std::int32_t& cxx_operator()"), "{text}");
    assert!(text.contains("std::int32_t cxx_new;"), "{text}");
    for suffixed in ["static_;", "operator_;", "volatile_;", "new_;"] {
        assert!(!text.contains(suffixed), "{suffixed} is not rule 8: {text}");
    }
}

#[test]
fn an_enumerator_and_a_constant_that_are_reserved_words_are_prefixed_by_cxx() {
    let text = header();
    assert!(text.contains("cxx_delete = 0"), "{text}");
    assert!(
        text.contains("inline constexpr std::int32_t cxx_register = 3;"),
        "{text}"
    );
    // The default of the enumerator-typed member names the same spelling.
    assert!(text.contains("::cx::Colour::cxx_delete"), "{text}");
    assert!(!text.contains("delete_"), "{text}");
    assert!(!text.contains("register_"), "{text}");
}

#[test]
fn the_codec_refers_to_the_prefixed_member() {
    let text = header();
    assert!(text.contains("zd_enc_p_long(w, v.cxx_static);"), "{text}");
    assert!(
        text.contains("zd_dec_p_long(r, v.cxx_static, zerodds::wire::TryConstruct::Discard);"),
        "{text}"
    );
    assert!(
        text.contains("zd_enc_p_long(w, v.zd_cxx_operator);"),
        "{text}"
    );
    assert!(
        text.contains("zd_enc_p_short(w, v.zd_cxx_volatile);"),
        "{text}"
    );
}

#[test]
fn the_idl_names_table_maps_the_prefixed_name_back_to_the_idl_spelling() {
    let text = header();
    assert!(
        text.contains("{{{\"cxx_static\", \"static\"}, {\"tint\", \"tint\"}}}"),
        "{text}"
    );
    assert!(
        text.contains("{{{\"cxx_delete\", \"delete\"}, {\"keep\", \"keep\"}}}"),
        "{text}"
    );
    assert!(
        text.contains("{{{\"cxx_new\", \"new\"}}}"),
        "the exception's members are in the table too: {text}"
    );
}
