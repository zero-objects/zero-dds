// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-routing-service` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`:
//! - `src/bin/zerodds-router.rs`: `cli::Configured::from_args` over the
//!   arguments, `configured.args` (everything that is not a configuration
//!   flag, in order, its own `run`/`validate` command line),
//!   `configured.runtime`, `check_config`, and `cli::HELP` in its usage text.
//! - `src/engine.rs`: `RuntimeConfig::from_standard_paths` and
//!   `create_participant_with_config` per domain, `UserReaderConfig` and
//!   `UserWriterConfig` filled field by field over `Default`,
//!   `register_user_reader_kind`/`register_user_writer_kind`, and
//!   `serialized_type::SerializedTypeSupport::from_complete_type_objects` on
//!   a keyed endpoint's `type_file`, refused unless `is_keyed()`.
//! - `src/forwarding.rs`: `UserSample` (`Alive` with its payload,
//!   `representation_identifier`, source timestamp, writer GUID and
//!   sequence number; `Lifecycle`; `CoherentSetEnd`), and the output
//!   writer's `set_user_writer_data_rep_override`,
//!   `set_user_writer_wire_extensibility`,
//!   `set_user_writer_byte_order_override`, `write_user_sample_*` and
//!   `write_user_lifecycle*`.
//!
//! What it relies on and is checked here, without a socket: the
//! configuration flags of the command line and their precedence (the
//! rustdoc of `zerodds_dcps::cli`: flags above files), the arguments that
//! are no flags handed back in order, and that a type support built from a
//! COMPLETE TypeObject says whether the type is keyed and derives the key
//! hash of DDS-XTypes 1.3 §7.6.8 from a body. Oracles: Cyclone DDS 11.0.1's
//! `idlc` octets of `@final struct Plain { long a; short b; }` (recorded in
//! `crates/types/tests/typeobject_vendor_dheader.rs`), DDS-XTypes 1.3
//! §7.6.8 (a key holder of at most 16 octets, big endian, zero padded, is
//! the key hash). The framing of the forwarded samples needs a bound
//! runtime; `tests/encapsulation_forwarding.rs` checks it in the
//! integration pillar's form.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use zerodds_dcps::cli::{Configured, HELP};
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_types::type_object::TypeObject;

/// Cyclone DDS 11.0.1 `idlc`: `module to { @final struct Plain { long a;
/// short b; }; }`, COMPLETE, XCDR2 little endian.
const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// An empty runtime configuration file of this test alone, so that no
/// system file takes part.
fn empty_config(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zerodds-router-contract-{}-{name}.yaml",
        std::process::id()
    ));
    std::fs::write(&path, "\n").unwrap();
    path
}

#[test]
fn the_router_command_line_comes_back_without_the_configuration_flags() {
    let config = empty_config("args");
    let configured = Configured::from_args(strings(&[
        "run",
        "--config",
        config.to_str().unwrap(),
        "--routes",
        "routes.json",
        "--set",
        "discovery.domain_tag=router-a",
        "--types",
        "shapes.json",
    ]))
    .unwrap();
    assert_eq!(
        configured.args,
        strings(&["run", "--routes", "routes.json", "--types", "shapes.json"]),
        "the router's own arguments, in order"
    );
    assert_eq!(
        configured.runtime.domain_tag, "router-a",
        "--set reaches the runtime"
    );
    assert!(!configured.checking());
    assert!(configured.check_config().is_none());
    let _ = std::fs::remove_file(config);
}

#[test]
fn a_set_flag_ranks_above_the_configuration_file() {
    let config = empty_config("precedence");
    std::fs::write(&config, "discovery:\n  domain_tag: from-file\n").unwrap();
    let from_file =
        Configured::from_args(strings(&["--config", config.to_str().unwrap()])).unwrap();
    assert_eq!(from_file.runtime.domain_tag, "from-file");
    let flag = Configured::from_args(strings(&[
        "--config",
        config.to_str().unwrap(),
        "--set",
        "discovery.domain_tag=from-flag",
    ]))
    .unwrap();
    assert_eq!(flag.runtime.domain_tag, "from-flag");
    let _ = std::fs::remove_file(config);
}

#[test]
fn an_unknown_set_path_is_a_configuration_error() {
    let config = empty_config("unknown");
    // The router answers a configuration error with exit code 2.
    assert!(
        Configured::from_args(strings(&[
            "--config",
            config.to_str().unwrap(),
            "--set",
            "discovery.no_such_key=1",
        ]))
        .is_err()
    );
    let _ = std::fs::remove_file(config);
}

#[test]
fn the_help_text_names_the_flags_the_router_passes_on() {
    for flag in ["--config", "--set", "--check-config"] {
        assert!(HELP.contains(flag), "{flag}");
    }
}

#[test]
fn xtypes_7_3_4_5_a_type_without_a_key_member_is_not_keyed() {
    let TypeObject::Complete(complete) = TypeObject::from_bytes_le(&unhex(PLAIN_COMPLETE)).unwrap()
    else {
        panic!("EK_COMPLETE");
    };
    let support =
        SerializedTypeSupport::from_complete_type_objects("to::Plain", complete, Vec::new())
            .unwrap();
    assert!(!support.is_keyed(), "a keyed route refuses it");
}

#[test]
fn xtypes_7_6_8_a_keyed_type_gives_the_padded_key_holder_as_its_key_hash() {
    let ast = Arc::new(
        zerodds_wire_probe::encode::model_from_idl(
            "@final struct KeyedFinal { @key long id; long value; };",
            "keyed-final.idl",
        )
        .unwrap(),
    );
    let type_ = zerodds_types::dynamic::DynamicType::find(&ast, "KeyedFinal").unwrap();
    let complete = zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap();
    let support =
        SerializedTypeSupport::from_complete_type_objects("KeyedFinal", complete, Vec::new())
            .unwrap();
    assert!(support.is_keyed());
    // id = 0x01020304, value = 7, PLAIN_CDR2 little endian.
    let body = [0x04, 0x03, 0x02, 0x01, 0x07, 0x00, 0x00, 0x00];
    let sample = support.read_body(&body, 2, false).unwrap();
    let (hash, holder) = support.key_material(&sample).unwrap();
    // §7.6.8: the key holder is the key members in big-endian PLAIN_CDR2;
    // one of at most 16 octets is the key hash, padded with zeros.
    assert_eq!(holder, [0x01, 0x02, 0x03, 0x04]);
    assert_eq!(hash, [1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}
