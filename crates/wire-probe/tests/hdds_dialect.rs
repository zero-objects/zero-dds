// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! **Counter-check.** `Dialect::Hdds` reproduces each of the four
//! recorded departures from XTypes 1.3, and the axis that carries it is
//! the only thing that does.
//!
//! Each test names the recorded `.bin` it reads, encodes the case under
//! `Dialect::Hdds` and holds the result against those octets, and then
//! encodes the same case under `Dialect::Specification` and asserts the
//! two differ — so an axis that stopped reaching the wire fails here as
//! a recorded row that no longer comes out, rather than passing on a
//! constant.
//!
//! The cases and their samples are `tests/interop-corpus/<case>/`; the
//! octets are `tests/interop-results/<case>/hdds/`.
#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The octets `tests/interop-results/<case>/hdds/<slot>.bin` records.
fn recorded(case: &str, slot: &str) -> Vec<u8> {
    let path = repository_root()
        .join("tests")
        .join("interop-results")
        .join(case)
        .join("hdds")
        .join(format!("{slot}.bin"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// One case from `tests/interop-corpus/<case>/`, encoded under `dialect`.
fn encode_case(case: &str, slot: &str, dialect: Dialect) -> Vec<u8> {
    let directory = repository_root()
        .join("tests")
        .join("interop-corpus")
        .join(case);
    let idl = std::fs::read_to_string(directory.join("type.idl")).expect("the IDL");
    let type_name = std::fs::read_to_string(directory.join("type")).expect("the type name");
    let type_name = type_name.trim();
    let values: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.join("values.json")).expect("the values"),
    )
    .expect("the values parse");
    let ast = model_from_idl_in_dialect(&idl, &format!("{case}/type.idl"), dialect)
        .unwrap_or_else(|error| panic!("{case}: {error}"));
    let encoding = if slot.starts_with("plain-cdr1") {
        Encoding::PlainCdr1
    } else {
        Encoding::PlainCdr2
    };
    let endianness = if slot.ends_with("le") {
        Endianness::Little
    } else {
        Endianness::Big
    };
    let request = ProbeRequest::in_dialect(type_name, encoding, endianness, dialect);
    encode(&ast, &request, &values).unwrap_or_else(|error| panic!("{case}: {error}"))
}

/// The writer under `Dialect::Hdds` and the recorded bytes are the same,
/// and `Dialect::Specification` — the clause — is not.
fn hdds_reproduces(case: &str, slot: &str) {
    let expected = recorded(case, slot);
    let mine = encode_case(case, slot, Dialect::Hdds);
    assert_eq!(
        hex(&mine),
        hex(&expected),
        "{case}/{slot}: `Dialect::Hdds` does not write tests/interop-results/{case}/hdds/{slot}.bin"
    );
    let clause = encode_case(case, slot, Dialect::Specification);
    assert_ne!(
        clause, mine,
        "{case}/{slot}: the specification and the dialect agree, so the row does not pin the axis"
    );
}

/// [`ElementPadding::Packed`]: the second `long` of a `Cell items[2]`
/// follows the first element's `short` with no padding to a four-octet
/// boundary — the `01000000 0200 03000000` of the recorded row.
///
/// [`ElementPadding::Packed`]: zerodds_ast_layout::ElementPadding::Packed
#[test]
fn no_alignment_padding_between_elements() {
    hdds_reproduces("array-1d-struct-final", "plain-cdr1.le");
}

/// [`AggregateDelimiter::Never`]: an `@appendable` sample takes no
/// DHEADER under XCDR2, where §7.4.3.5.3 rule (2) writes `0e000000`.
///
/// [`AggregateDelimiter::Never`]: zerodds_ast_layout::AggregateDelimiter::Never
#[test]
fn no_dheader_or_emheader_in_front_of_an_aggregate() {
    hdds_reproduces("struct-appendable", "plain-cdr2.le");
}

/// [`SequenceElementDelimiter::BeforeEachNonFinalElement`]: the `06 00
/// 00 00` in front of each `@appendable Cell` element of a `sequence`,
/// under XCDR1 as well as XCDR2.
///
/// [`SequenceElementDelimiter::BeforeEachNonFinalElement`]: zerodds_ast_layout::SequenceElementDelimiter::BeforeEachNonFinalElement
#[test]
fn a_sequence_element_carries_its_own_length() {
    hdds_reproduces("sequence-struct-appendable", "plain-cdr1.le");
}

/// [`WstringCodeUnits::Utf8`]: `w = "abc"` goes out as `04 00 00 00 61
/// 62 63 00`, three UTF-8 octets and a NUL inside the declared length.
///
/// [`WstringCodeUnits::Utf8`]: zerodds_ast_layout::WstringCodeUnits::Utf8
#[test]
fn a_wstring_is_written_in_utf8() {
    hdds_reproduces("wstring-member", "plain-cdr1.le");
}
