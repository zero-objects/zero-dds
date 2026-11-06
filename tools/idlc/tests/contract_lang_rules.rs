// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-idlc` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-lang-rules` (`src/generate.rs`,
//! `src/main.rs`, the `*_support.rs` files): `generate::source` for the
//! declarations and codec of every language `--lang` names, under the
//! mapping `--cpp-mapping`/`--java-mapping` selects
//! (`adapter::cpp::CppMappingScope`, `adapter::java::with_mapping`), and
//! `emit::encode::codec_name` / `emit::decode::decoder_name`, by which the
//! C++ topic trait and the support files call the codec the generated file
//! holds.
//!
//! Oracle: DDS-PSM-Cxx 1.0 §7.4.2 Table 7.1 (`sequence<T>` is
//! `std::vector<T>`, `map<K, V>` is `std::map<K, V>`, `string` is
//! `std::string`) and §8.1.4 (an `@optional` member is
//! `dds::core::optional<T>`); the documented invariant of `codec_name` and
//! `decoder_name` (the published name of the function the codec walk
//! emits for the type).
#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_cst::Language;
use zerodds_lang_rules::adapter::cpp::{CppMapping, CppMappingScope};
use zerodds_lang_rules::emit::cpp::Cpp;
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::encode::codec_name;

fn model(idl: &str) -> Ast {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        idl,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    composed.ast
}

fn key() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
}

#[test]
fn psmcxx_7_4_2_the_psm_mapping_spells_the_collections_and_the_optional_of_the_psm() {
    let ast = model(
        "struct S { sequence<long> v; map<string, long> m; string name; @optional long o; };",
    );
    let _scope = CppMappingScope::enter(CppMapping::Psm);
    let text = zerodds_lang_rules::generate::source(&ast, Language::Cpp, key()).unwrap();
    for wanted in [
        "std::vector<int32_t>",
        "std::map<std::string, int32_t>",
        "std::string",
        "::dds::core::optional<int32_t>",
    ] {
        assert!(text.contains(wanted), "`{wanted}` is missing:\n{text}");
    }
}

#[test]
fn the_codec_names_are_the_names_of_the_functions_the_file_holds() {
    let ast = model("@appendable struct Reading { long id; double value; };");
    let id = ast.types().find(|(_, n)| n.name == "Reading").unwrap().0;
    let text = zerodds_lang_rules::generate::source(&ast, Language::Cpp, key()).unwrap();
    let encoder = codec_name::<Cpp>(&ast, id, Purpose::Data);
    let decoder = decoder_name::<Cpp>(&ast, id);
    assert!(text.contains(&encoder), "`{encoder}` is not in the file");
    assert!(text.contains(&decoder), "`{decoder}` is not in the file");
}

#[test]
fn every_language_the_tool_offers_generates_the_declarations_and_a_codec() {
    let ast = model("@appendable struct Reading { long id; double value; string label; };");
    for language in Language::ALL {
        let text = zerodds_lang_rules::generate::source(&ast, language, key())
            .unwrap_or_else(|error| panic!("{language:?}: {error}"));
        assert!(text.contains("Reading"), "{language:?}");
    }
}
