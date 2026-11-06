// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wireprobe-python-gen` with `zerodds-wire-probe`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-probe`:
//! - `src/bin/wireprobe-python-matrix.rs`: `matrix::{CONFIGURATIONS,
//!   Configuration, ENCODINGS, ORDERS}` for the slots it answers,
//!   `value::MapLengthPolicy` for the map length,
//!   `encode::model_from_idl_in_dialect` for one model per configuration and
//!   `canonical::{canonicalise, text}` for the canonical form of the Python
//!   column's read direction.
//!
//! So the crate relies on:
//! - the slot coordinates (twelve configurations, five encodings, two byte
//!   orders) and their names, which the Python column keys its cells by;
//! - a configuration being its dialect plus only the overrides it names;
//! - the model being lowered in the dialect, because the `@hashid` counter
//!   is the front end's;
//! - the canonical text being the one form of a value.
//!
//! Oracles:
//! DDS-XTypes 1.3 §7.6.3.1.2 Table 60 (the five encodings), §7.3.1.2.1.1
//! (the counter after a `@hashid` member, RTI the one that does not
//! advance), §7.4.3.4.2 (EMHEADER1, the member id in the low 28 bits),
//! §7.4.3.5.3 rule (15) (a delimited map's entry count, the two readings);
//! the value format of `tests/interop-corpus/README.md`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::encode::{model_from_idl, model_from_idl_in_dialect};
use zerodds_wire_probe::matrix::{CONFIGURATIONS, Configuration, ENCODINGS, ORDERS};
use zerodds_wire_probe::value::{MapLengthPolicy, ProbeRequest};

fn zerodds_wire_codec_encode(ast: &Ast, request: &ProbeRequest) -> Vec<u8> {
    zerodds_wire_probe::encode::encode(ast, request, &json!({"a": 1, "b": 2, "c": 3})).unwrap()
}

fn type_in(ast: &Ast, name: &[&str]) -> Id<TypeNode> {
    ast.find_by_scoped_name(&ScopedName::absolute(
        name.iter().map(|segment| (*segment).to_string()).collect(),
    ))
    .ok()
    .and_then(zerodds_ast::AnyId::as_type)
    .expect("the type is in the model")
}

#[test]
fn xtypes_7_6_3_1_2_the_slot_coordinates_are_twelve_configurations_five_encodings_two_orders() {
    let names: Vec<&str> = CONFIGURATIONS.iter().map(|c| c.name).collect();
    assert_eq!(
        names,
        [
            "specification",
            "cyclone",
            "rti",
            "fastdds",
            "omniorb",
            "tao",
            "jacorb",
            "opendds",
            "ros-fastrtps",
            "ros-cyclone",
            "universal-nextint",
            "rule-15-omits",
        ]
    );
    // DDS-XTypes 1.3 Table 60 names the five encodings.
    assert_eq!(
        ENCODINGS.map(|(name, _)| name),
        [
            "plain-cdr1",
            "pl-cdr1",
            "plain-cdr2",
            "delimited-cdr",
            "pl-cdr2"
        ]
    );
    assert_eq!(
        ENCODINGS.map(|(_, encoding)| encoding),
        [
            Encoding::PlainCdr1,
            Encoding::PlCdr1,
            Encoding::PlainCdr2,
            Encoding::DelimitedCdr,
            Encoding::PlCdr2,
        ]
    );
    assert_eq!(ORDERS.map(|(name, _)| name), ["le", "be"]);
    assert_eq!(
        ORDERS.map(|(_, order)| order),
        [Endianness::Little, Endianness::Big]
    );
}

#[test]
fn a_configuration_is_its_dialect_and_only_the_overrides_it_names() {
    for settings in &CONFIGURATIONS[..10] {
        let settings: &Configuration = settings;
        assert_eq!(
            settings.flags(),
            [
                "--dialect".to_string(),
                settings.dialect.as_str().to_string()
            ],
            "{} carries no override",
            settings.name
        );
        let request = settings.request("T", Encoding::PlainCdr2, Endianness::Little);
        let plain = ProbeRequest::in_dialect(
            "T",
            Encoding::PlainCdr2,
            Endianness::Little,
            settings.dialect,
        );
        assert_eq!(request.length_codes, plain.length_codes);
        assert_eq!(request.map_length, plain.map_length);
    }
    let nextint = &CONFIGURATIONS[10];
    assert_eq!(
        nextint.flags()[..2],
        ["--dialect".to_string(), "specification".to_string()]
    );
    assert!(nextint.flags().contains(&"--length-codes".to_string()));
    let rule15 = &CONFIGURATIONS[11];
    assert_eq!(
        rule15.flags()[2..],
        ["--map-length".to_string(), "rule-15-omits".to_string()]
    );
    assert_eq!(
        rule15
            .request("T", Encoding::PlainCdr2, Endianness::Little)
            .map_length,
        MapLengthPolicy::Rule15Omits
    );
}

#[test]
fn xtypes_7_4_3_5_3_the_two_readings_of_a_delimited_maps_length_by_name() {
    // Rule (15) as written omits the entry count behind a DHEADER; the
    // default keeps it, the same shape as the sequence §7.4.1.1.4 makes a
    // map equivalent to.
    assert_eq!(
        MapLengthPolicy::default(),
        MapLengthPolicy::SequenceEquivalent
    );
    assert!(MapLengthPolicy::SequenceEquivalent.is_default());
    for policy in MapLengthPolicy::ALL {
        assert_eq!(MapLengthPolicy::parse(policy.as_str()), Some(policy));
    }
    assert_eq!(
        MapLengthPolicy::SequenceEquivalent.as_str(),
        "sequence-equivalent"
    );
    assert_eq!(MapLengthPolicy::Rule15Omits.as_str(), "rule-15-omits");
    assert_eq!(
        MapLengthPolicy::parse("rule-15-omit"),
        None,
        "a typo is no policy"
    );
}

#[test]
fn the_canonical_text_sorts_the_keys_and_writes_a_64_bit_integer_as_a_string() {
    let idl = "enum Colour { RED, GREEN };\n\
               @final struct V { long long w; Colour c; sequence<short> q; boolean f; };";
    let ast = model_from_idl(idl, "canonical.idl").unwrap();
    let value = json!({"w": -3, "c": "GREEN", "q": [1, -1], "f": true});
    let canonical = canonicalise(&ast, type_in(&ast, &["V"]), &value, "sample").unwrap();
    assert_eq!(
        text(&canonical),
        r#"{"c":"GREEN","f":true,"q":[1,-1],"w":"-3"}"#
    );
    // A value that is not one of the type's is refused with its path.
    let error = canonicalise(
        &ast,
        type_in(&ast, &["V"]),
        &json!({"w": 1, "c": "BLUE", "q": [], "f": true}),
        "sample",
    )
    .err()
    .unwrap();
    assert!(format!("{error}").contains("sample"), "{error}");
}

#[test]
fn xtypes_7_3_1_2_1_1_the_model_is_lowered_in_the_dialect() {
    // After a `@hashid` member, Cyclone continues from the hashed id and
    // RTI from the last sequential one, and that is decided by the front
    // end: the model of each dialect numbers `c` differently.
    let idl = "@mutable struct H { long a; @hashid long b; long c; };";
    let id_of_c = |dialect: Dialect| -> u32 {
        let ast = model_from_idl_in_dialect(idl, "hash.idl", dialect).unwrap();
        let request = ProbeRequest::in_dialect("H", Encoding::PlCdr2, Endianness::Little, dialect);
        let bytes = zerodds_wire_codec_encode(&ast, &request);
        u32::from_le_bytes(bytes[20..24].try_into().unwrap()) & 0x0fff_ffff
    };
    assert_eq!(id_of_c(Dialect::Rti), 1, "RTI continues after `a`");
    assert_ne!(
        id_of_c(Dialect::Cyclone),
        1,
        "Cyclone continues after the hashed id"
    );
}
