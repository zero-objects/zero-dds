// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-interop-results` with `zerodds-wire-probe`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-probe`:
//! - `src/rust_column.rs`: per configuration of `matrix::CONFIGURATIONS`
//!   one model (`encode::model_from_idl_in_dialect`), a request from
//!   `Configuration::request`, `encode::encode` and `encode::hex` for the
//!   write direction, `encode::encode_with_padding` for the octets that
//!   carry no value, `decode::decode` and `canonical::{canonicalise, text}`
//!   for the read direction; `REFUSED_BY_THE_FRONT_END` for the two corpus
//!   cases the front end refuses.
//! - `src/dialect_fit.rs`: `encode::model_from_idl_with` with the front-end
//!   options spelled out, a `ProbeRequest` built field by field with
//!   `Emit::Payload` and `EncapsulationIds::Table60`.
//!
//! So the crate relies on: the write and read directions being the
//! vendors' octets for a sample the vendors agree on; the padding mask
//! marking exactly the octets no value fills; one lowering per dialect,
//! because the `@hashid` counter is decided in the front end; a refusal
//! of the front end being an `Err` with the reason, for exactly the two
//! cases `REFUSED_BY_THE_FRONT_END` names.
//!
//! Oracles: the octets Cyclone DDS, RTI Connext and Fast DDS wrote for
//! `opt_final_absent` (`crates/ast-layout/tests/fixtures/vendor_bytes.md`;
//! DDS-XTypes 1.3 §7.4.3.5.2 rule (20)); §7.4.1.1.1 (alignment, padding
//! between members); §7.3.1.2.1.1 (a `@hashid` member, the two readings of
//! the counter after it, RTI the one that does not advance); §7.4.3.4.2
//! (EMHEADER1: M_FLAG, LC, member id in the low 28 bits); §7.6.3.1.2 Table
//! 60 (the encapsulation identifiers); OMG IDL 4.2
//! §7.4.1.4.4.4.4 (a recursion needs a sequence, a map, an `@optional` or
//! an `@external` member).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{
    encode, encode_with_padding, hex, model_from_idl, model_from_idl_in_dialect,
    model_from_idl_with,
};
use zerodds_wire_probe::matrix::CONFIGURATIONS;
use zerodds_wire_probe::value::{Emit, EncapsulationIds, ProbeRequest};

const OPTIONAL: &str = "@final struct Sample { long a; @optional octet b; long c; };";
// -1234567, the absent `b` (is_present 0 and three octets of padding), -2.
const ABSENT: &str = "7929edff00000000feffffff";

fn type_in(ast: &Ast, name: &[&str]) -> Id<TypeNode> {
    ast.find_by_scoped_name(&ScopedName::absolute(
        name.iter().map(|segment| (*segment).to_string()).collect(),
    ))
    .ok()
    .and_then(zerodds_ast::AnyId::as_type)
    .expect("the type is in the model")
}

#[test]
fn xtypes_7_4_3_5_2_both_directions_are_the_vendors_octets() {
    let ast = model_from_idl(OPTIONAL, "optional.idl").unwrap();
    let request = ProbeRequest::new("Sample", Encoding::PlainCdr2, Endianness::Little);
    assert_eq!(request.emit, Emit::Payload);
    let sample = json!({"a": -1_234_567, "c": -2});
    let bytes = encode(&ast, &request, &sample).unwrap();
    assert_eq!(hex(&bytes), ABSENT);
    let read = decode(&ast, &request, &bytes).unwrap();
    let expected = canonicalise(&ast, type_in(&ast, &["Sample"]), &sample, "sample").unwrap();
    assert_eq!(text(&read), text(&expected));
    assert_eq!(text(&read), r#"{"a":-1234567,"c":-2}"#);
}

#[test]
fn xtypes_7_4_1_1_1_the_padding_mask_is_the_octets_no_value_fills() {
    let ast = model_from_idl(OPTIONAL, "optional.idl").unwrap();
    let request = ProbeRequest::new("Sample", Encoding::PlainCdr2, Endianness::Little);
    let (bytes, mask) =
        encode_with_padding(&ast, &request, &json!({"a": -1_234_567, "c": -2})).unwrap();
    assert_eq!(hex(&bytes), ABSENT);
    assert_eq!(mask.len(), bytes.len(), "one flag per octet");
    // a: 0..4, is_present: 4, padding to the next long: 5..8, c: 8..12.
    let padding: Vec<usize> = mask
        .iter()
        .enumerate()
        .filter(|(_, padded)| **padded)
        .map(|(offset, _)| offset)
        .collect();
    assert_eq!(padding, [5, 6, 7]);
}

#[test]
fn xtypes_7_3_1_2_1_1_the_hashid_counter_is_decided_per_dialect_in_the_front_end() {
    // After a `@hashid` member, Cyclone continues from the hashed id and
    // RTI from the last sequential one: the id of `c` differs, and only a
    // model lowered in the dialect carries it.
    let idl = "@mutable struct H { long a; @hashid long b; long c; };";
    let id_of_c = |dialect: Dialect| -> (u32, u32) {
        let ast = model_from_idl_in_dialect(idl, "hash.idl", dialect).unwrap();
        let request = ProbeRequest::in_dialect("H", Encoding::PlCdr2, Endianness::Little, dialect);
        let bytes = encode(&ast, &request, &json!({"a": 1, "b": 2, "c": 3})).unwrap();
        // DHEADER, then EMHEADER1 and value per member (LC 2: four octets).
        let word =
            |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) & 0x0fff_ffff;
        (word(12), word(20))
    };
    let (b_cyclone, c_cyclone) = id_of_c(Dialect::Cyclone);
    let (b_rti, c_rti) = id_of_c(Dialect::Rti);
    assert_eq!(b_cyclone, b_rti, "the hashed id itself is the same");
    assert_eq!(
        c_cyclone,
        b_cyclone + 1,
        "Cyclone continues after the hashed id"
    );
    assert_eq!(c_rti, 1, "RTI continues after `a`");
    // The configurations lower per dialect: the RTI one is RTI's.
    assert!(
        CONFIGURATIONS
            .iter()
            .any(|c| c.name == "rti" && c.dialect == Dialect::Rti)
    );
}

#[test]
fn xtypes_7_6_3_1_2_a_request_built_field_by_field_takes_the_table_60_identifiers() {
    let mut request = ProbeRequest::new("Sample", Encoding::PlainCdr2, Endianness::Little);
    request.emit = Emit::PayloadWithEncapsulation;
    request.encapsulation_ids = EncapsulationIds::Table60;
    let ast = model_from_idl_with(
        OPTIONAL,
        "optional.idl",
        zerodds_idl_lower::Options::default(),
    )
    .unwrap();
    let bytes = encode(&ast, &request, &json!({"a": -1_234_567, "c": -2})).unwrap();
    // DDS-XTypes 1.3 Table 60: CDR2_LE is 0x0007 for a final type, the
    // options zero (the payload ends on a four-octet boundary).
    assert_eq!(hex(&bytes), format!("00070000{ABSENT}"));
}

#[test]
fn idl_7_4_1_4_4_4_4_the_front_end_refuses_the_two_named_cases_with_a_reason() {
    assert_eq!(
        zerodds_wire_probe::REFUSED_BY_THE_FRONT_END,
        ["recursive-array", "resolve-module-named-as-type"]
    );
    let corpus =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus");
    for case in zerodds_wire_probe::REFUSED_BY_THE_FRONT_END {
        let idl = std::fs::read_to_string(corpus.join(case).join("type.idl")).unwrap();
        let error = model_from_idl(&idl, &format!("{case}/type.idl"))
            .err()
            .expect("the front end refuses the case");
        assert!(format!("{error}").len() > 30, "{case}: {error}");
    }
    let recursion =
        "module m { struct Node; @appendable struct Node { long value; Node pair[2]; }; };";
    let error = format!("{}", model_from_idl(recursion, "array.idl").err().unwrap());
    assert!(error.contains("7.4.1.4.4.4.4"), "{error}");
}
