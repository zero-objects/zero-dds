//! T7.3 — vendor fixture tests for Cyclone DDS and Fast-DDS.
//!
//! Both vendors mostly use standard OMG-IDL-4.2 without
//! vendor-specific grammar extensions. These tests verify
//! that the base grammar `IDL_42` accepts representative files of these
//! vendors.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl::parser::parse;

const CYCLONE_THROUGHPUT: &str = include_str!("fixtures/cyclonedds/throughput.idl");
const CYCLONE_LISTENER: &str = include_str!("fixtures/cyclonedds/listener.idl");
const FASTDDS_HELLO_WORLD: &str = include_str!("fixtures/fastdds/hello_world.idl");
const FASTDDS_SECURITY: &str = include_str!("fixtures/fastdds/security_topic.idl");

fn check(name: &str, src: &str) {
    let cfg = ParserConfig::default();
    let res = parse(src, &cfg);
    assert!(
        res.is_ok(),
        "{name} must parse with base IDL_42, got {res:?}"
    );
}

#[test]
fn cyclone_throughput_parses_with_base() {
    check("cyclonedds/throughput.idl", CYCLONE_THROUGHPUT);
}

#[test]
fn cyclone_listener_parses_with_base() {
    check("cyclonedds/listener.idl", CYCLONE_LISTENER);
}

#[test]
fn fastdds_hello_world_parses_with_base() {
    check("fastdds/hello_world.idl", FASTDDS_HELLO_WORLD);
}

#[test]
fn fastdds_security_topic_parses_with_base() {
    check("fastdds/security_topic.idl", FASTDDS_SECURITY);
}
