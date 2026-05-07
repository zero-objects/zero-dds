//! Integration-Test: parst die `zerodds_dcps.idl`-Fixture aus dem OMG-Annex
//! mit der vollstaendigen IDL-4.2-Grammar (T3.1-T3.10).
//!
//! Dieser Test ist die T3.12-Smoke-Probe: wenn er gruen ist, traegt die
//! Grammar repraesentative DDS-IDL-Konstrukte. Die Fixture ist eine
//! inhaltlich aequivalente Auswahl der OMG-DDS-1.4-Annex-IDL — siehe
//! Fixture-Header.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::engine::parse;
use zerodds_idl::grammar::idl42::IDL_42;
use zerodds_idl::lexer::Tokenizer;

const DDS_DCPS_IDL: &str = include_str!("fixtures/omg/zerodds_dcps.idl");
const DDS_SECURITY_IDL: &str = include_str!("fixtures/omg/zerodds_security.idl");
const DDS_XTYPES_IDL: &str = include_str!("fixtures/omg/dds_xtypes.idl");

fn parse_fixture(name: &str, src: &str) {
    let tokenizer = Tokenizer::for_grammar(&IDL_42);
    let stream = tokenizer
        .tokenize(src)
        .unwrap_or_else(|e| panic!("tokenize {name} failed: {e:?}"));
    let result = parse(&IDL_42, stream.tokens());
    assert!(
        result.is_ok(),
        "parse {name} failed: {result:?}\n\
         note: this fixture is hand-crafted for IDL-4.2 grammar coverage;\n\
         a parse failure means a Production is missing or buggy."
    );
}

#[test]
fn lexer_handles_dds_dcps_idl() {
    // Mit T-LIM-1 (Comment-Support) sollte das Lexen jetzt voll
    // durchlaufen — auch mit //-Kommentaren in der Fixture.
    let tokenizer = Tokenizer::for_grammar(&IDL_42);
    let result = tokenizer.tokenize(DDS_DCPS_IDL);
    assert!(result.is_ok(), "tokenize failed: {result:?}");
}

#[test]
fn parses_dds_dcps_idl_directly() {
    parse_fixture("zerodds_dcps.idl", DDS_DCPS_IDL);
}

#[test]
fn parses_dds_security_idl() {
    // T4.10 — DDS-Security-Auszug (Token-Hierarchie, Property-Lists,
    // Authentication-Interface). Validiert Annotation-Hooks (T4.4) und
    // Struct-Inheritance (T4.7) gegen realistische Multi-Modul-IDL.
    parse_fixture("zerodds_security.idl", DDS_SECURITY_IDL);
}

#[test]
fn parses_dds_xtypes_idl() {
    // T4.10 — DDS-XTypes-Auszug (bitset/bitmask/map/extensibility-
    // annotations, struct-Inheritance ueber CommonHeader-Pattern).
    // Validiert Map-Productions (T4.6), Bitset/Bitmask (T4.6) und
    // realistische Annotation-Kombinationen (T4.4).
    parse_fixture("dds_xtypes.idl", DDS_XTYPES_IDL);
}
