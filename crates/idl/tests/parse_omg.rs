//! Integration test: parses the `zerodds_dcps.idl` fixture from the OMG annex
//! with the complete IDL-4.2 grammar (T3.1-T3.10).
//!
//! This test is the T3.12 smoke probe: if it is green, the
//! grammar carries representative DDS-IDL constructs. The fixture is a
//! content-equivalent selection of the OMG-DDS-1.4 annex IDL — see the
//! fixture header.

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
    // With T-LIM-1 (comment support) the lexing should now run
    // fully through — even with //-comments in the fixture.
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
    // T4.10 — DDS-Security excerpt (token hierarchy, property lists,
    // authentication interface). Validates annotation hooks (T4.4) and
    // struct inheritance (T4.7) against realistic multi-module IDL.
    parse_fixture("zerodds_security.idl", DDS_SECURITY_IDL);
}

#[test]
fn parses_dds_xtypes_idl() {
    // T4.10 — DDS-XTypes excerpt (bitset/bitmask/map/extensibility
    // annotations, struct inheritance via the CommonHeader pattern).
    // Validates map productions (T4.6), bitset/bitmask (T4.6) and
    // realistic annotation combinations (T4.4).
    parse_fixture("dds_xtypes.idl", DDS_XTYPES_IDL);
}
