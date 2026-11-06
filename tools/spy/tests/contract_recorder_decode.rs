// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-spy` with `zerodds-recorder-decode`
//! (internal/release/rc8-test-suite.md, "Crate tier"). What the tool uses:
//! `TypeBook` to load an IDL file and resolve a type by name, `encoding_of`
//! to know which encoding a received sample of it is in, and `data_to_json`
//! to print a decoded value. What it relies on: DDS-XTypes 1.3 §7.4.1.1 and
//! §7.4.2 (under XCDR1 an appendable struct is PLAIN_CDR and a mutable one
//! PL_CDR; under XCDR2 DELIMITED_CDR and PL_CDR2), IDL 4.2 §7.5.2 (a scoped
//! name with or without the leading `::`), and RFC 8259 for the printed
//! value (NaN is no number).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_ast_layout::key::Encoding;
use zerodds_recorder_decode::json_sink::data_to_json;
use zerodds_recorder_decode::type_source::TypeBook;
use zerodds_recorder_decode::wire::encoding_of;
use zerodds_types::dynamic::DynamicData;

#[test]
fn xtypes_7_4_the_encoding_of_a_resolved_type_follows_its_extensibility() {
    let book = TypeBook::from_idl(
        "module shapes { @appendable struct Point { long x; }; @mutable struct Pose { long x; }; };",
    )
    .unwrap();
    let a = book.resolve("::shapes::Point").unwrap();
    assert_eq!(encoding_of(&a, false), Encoding::PlainCdr1);
    assert_eq!(encoding_of(&a, true), Encoding::DelimitedCdr);
    let m = book.resolve("shapes::Pose").unwrap();
    assert_eq!(encoding_of(&m, false), Encoding::PlCdr1);
    assert_eq!(encoding_of(&m, true), Encoding::PlCdr2);
}

#[test]
fn rfc_8259_a_decoded_value_prints_as_json() {
    let value = DynamicData::Aggregate(vec![
        (
            String::from("id"),
            DynamicData::Integer {
                kind: None,
                value: 42,
            },
        ),
        (
            String::from("name"),
            DynamicData::Text(String::from("bandit")),
        ),
        (
            String::from("x"),
            DynamicData::Float {
                kind: None,
                value: f64::NAN,
            },
        ),
    ]);
    assert_eq!(
        data_to_json(&value).to_string(),
        r#"{"id":42,"name":"bandit","x":null}"#
    );
}
