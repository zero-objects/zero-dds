//! T6.8 — end-to-end tests for the RTI Connext workflow:
//! preprocessor → tokenizer → recognizer (with RTI delta) → CST → AST.
//!
//! Ensures that the combined pipeline parses RTI-typical IDL
//! that fails without the RTI delta.

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

use zerodds_idl::ast::Definition;
use zerodds_idl::config::ParserConfig;
use zerodds_idl::grammar::deltas::RTI_CONNEXT;
use zerodds_idl::parser::{parse, parse_with_deltas};
use zerodds_idl::preprocessor::{MemoryResolver, Preprocessor};

const SENSOR_TOPIC: &str = include_str!("fixtures/rti/sensor_topic.idl");
const WITH_PREPROCESSOR: &str = include_str!("fixtures/rti/with_preprocessor.idl");
const SENSOR_COMMON: &str = include_str!("fixtures/rti/sensor_common.idl");

fn build_resolver() -> MemoryResolver {
    let mut r = MemoryResolver::new();
    r.add("sensor_common.idl", SENSOR_COMMON);
    r
}

#[test]
fn rti_keylist_parses_with_delta() {
    let cfg = ParserConfig::default();
    let ast = parse_with_deltas(SENSOR_TOPIC, &cfg, &[&RTI_CONNEXT])
        .expect("RTI delta accepts sensor_topic.idl");
    assert!(!ast.definitions.is_empty());
    match &ast.definitions[0] {
        Definition::Module(m) => assert_eq!(m.name.text, "sensors"),
        other => panic!("expected module sensors, got {other:?}"),
    }
}

#[test]
fn rti_keylist_rejected_without_delta() {
    let cfg = ParserConfig::default();
    let res = parse(SENSOR_TOPIC, &cfg);
    assert!(
        res.is_err(),
        "base IDL_42 must reject keylist syntax, got {res:?}"
    );
}

#[test]
fn rti_pipeline_with_preprocessor() {
    // Vollstaendige Pipeline: Preprocessor expandiert #include + #define,
    // then parse with the RTI delta.
    let pp = Preprocessor::new(build_resolver());
    let processed = pp
        .process("with_preprocessor.idl", WITH_PREPROCESSOR)
        .expect("preprocessor ok");
    let cfg = ParserConfig::default();
    let ast = parse_with_deltas(&processed.expanded, &cfg, &[&RTI_CONNEXT]).unwrap_or_else(|e| {
        panic!(
            "parser must accept preprocessed source: {e:?}\n\
                 expanded source:\n{}",
            processed.expanded
        )
    });
    assert!(!ast.definitions.is_empty());
}

#[test]
fn rti_pipeline_with_define_activates_debug_block() {
    // With an active ENABLE_DEBUG #define: DebugChannel + the second keylist
    // must be in the output.
    let mut src = String::from("#define ENABLE_DEBUG\n");
    src.push_str(WITH_PREPROCESSOR);
    let pp = Preprocessor::new(build_resolver());
    let processed = pp
        .process("with_define.idl", &src)
        .expect("preprocessor ok");
    assert!(
        processed.expanded.contains("DebugChannel"),
        "ENABLE_DEBUG must enable DebugChannel block:\n{}",
        processed.expanded
    );
    let ast = parse_with_deltas(
        &processed.expanded,
        &ParserConfig::default(),
        &[&RTI_CONNEXT],
    )
    .expect("parser ok");
    assert!(!ast.definitions.is_empty());
}

#[test]
fn rti_pipeline_without_define_skips_debug_block() {
    // Without ENABLE_DEBUG: the DebugChannel block is dropped.
    let pp = Preprocessor::new(build_resolver());
    let processed = pp
        .process("with_preprocessor.idl", WITH_PREPROCESSOR)
        .expect("preprocessor ok");
    assert!(
        !processed.expanded.contains("DebugChannel"),
        "without ENABLE_DEBUG, DebugChannel must be skipped"
    );
}
