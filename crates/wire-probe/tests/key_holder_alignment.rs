// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! How each dialect aligns a key holder, against what each vendor computed.
//!
//! DDS-XTypes 1.3 §7.6.8 serialises the holder as PLAIN_CDR2 big-endian,
//! which aligns every member to its size capped at four. Measured on
//! The Linux test host with `tools/keyhash-probe/run-all.sh` for the types of
//! `tools/keyhash-probe/opensplice/keyhash.idl` (the same shapes through
//! `@key` in `../keyhash.idl`):
//!
//! - OpenSplice 6.9, on the wire: every user DATA carries `PID_KEY_HASH`,
//!   with no padding between key fields of different widths.
//! - RTI Connext 7.7.0 and Cyclone 11.0.1 under XCDR1 align to eight, the
//!   XCDR1 `MAXALIGN`; under XCDR2 to four, as the clause says.
//! - Fast DDS 3.6.1 aligns to four, as the clause says. Its probe prints
//!   one KeyHash per type, so which encoding version it computed under is
//!   not recorded; only its XCDR2 dialect is asserted here.
//!
//! `BigKey` is the control: twenty octets with no padding in any reading.
//! The IDL is the probes' own `@key` form (`tools/keyhash-probe/keyhash.idl`)
//! — the OpenSplice column writes the same types through `#pragma keylist`,
//! whose lowering to these keys `idl-lower`'s `keys_and_inheritance` tests —
//! so a change to what is measured changes what is asserted.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::{Emit, ProbeRequest};

const IDL: &str = include_str!("../../../tools/keyhash-probe/keyhash.idl");

fn key_hash(
    dialect: Dialect,
    encoding: Encoding,
    type_name: &str,
    values: &serde_json::Value,
) -> String {
    let ast = model_from_idl_in_dialect(IDL, "keyhash.idl", dialect).expect("the model");
    let request = ProbeRequest::in_dialect(type_name, encoding, Endianness::Little, dialect)
        .emitting(Emit::KeyHash);
    hex(&encode(&ast, &request, values).expect("the probe"))
}

/// The samples the probes wrote. An integer past 2^53 is a JSON string,
/// which the probe requires: a JSON number cannot carry it exactly.
fn sample(type_name: &str) -> serde_json::Value {
    match type_name {
        "PadKey" => json!({"a": 0x11, "b": 0x2233_4455, "v": 1}),
        "WideKey" => json!({"s": 0x0102, "ll": "217304205466536208", "v": 1}),
        "CharKey" => json!({"o": 0x11, "s": 0x2233, "v": 1}),
        // `@key PathLeaf nested` with `@key octet b` in `PathLeaf`, then
        // `@key long x`: the path `nested.b x` of the OpenSplice column.
        "PathKey" => json!({"nested": {"a": 0x55, "b": 0x66}, "x": 0x7777_8888_u32, "v": 1}),
        "StrKey" => json!({"name": "ab", "id": 0x0a0b_0c0d, "v": 1}),
        "BigKey" => json!({"a": "72340172838076673", "b": "144680345676153346",
                           "c": 0x0303_0303, "v": 1}),
        other => panic!("no sample for {other}"),
    }
}

/// The clause's KeyHash: aligned to at most four. Fast DDS, and Cyclone
/// and RTI under XCDR2, computed exactly these.
const CLAUSE: [(&str, &str); 6] = [
    ("PadKey", "11000000223344550000000000000000"),
    ("WideKey", "01020000030405060708091000000000"),
    ("CharKey", "11002233000000000000000000000000"),
    ("PathKey", "66000000777788880000000000000000"),
    // MD5 over `00000003 61620000 0a0b0c0d`: an unbounded string makes the
    // holder's maximum unbounded.
    ("StrKey", "7b4e6d689588c5b24e9869ce93caec4c"),
    ("BigKey", "fb4e4b6ea54f6a57c9649eaf885d064f"),
];

/// OpenSplice's `PID_KEY_HASH`: the key fields back to back.
const OPENSPLICE: [(&str, &str); 6] = [
    ("PadKey", "11223344550000000000000000000000"),
    ("WideKey", "01020304050607080910000000000000"),
    ("CharKey", "11223300000000000000000000000000"),
    ("PathKey", "66777788880000000000000000000000"),
    // MD5 over `00000003 616200 0a0b0c0d`, eleven octets.
    ("StrKey", "1fbfe9f281baa6ca435e7d88ddde0c45"),
    ("BigKey", "fb4e4b6ea54f6a57c9649eaf885d064f"),
];

/// Cyclone's and RTI's under XCDR1: the eight-octet key aligned to eight.
/// Only `WideKey` has a member eight wide behind a narrower one.
const XCDR1_ALIGNED: [(&str, &str); 5] = [
    ("PadKey", "11000000223344550000000000000000"),
    ("WideKey", "01020000000000000304050607080910"),
    ("CharKey", "11002233000000000000000000000000"),
    ("PathKey", "66000000777788880000000000000000"),
    ("StrKey", "7b4e6d689588c5b24e9869ce93caec4c"),
];

#[test]
fn the_opensplice_dialect_hashes_the_key_fields_back_to_back() {
    for (type_name, expected) in OPENSPLICE {
        assert_eq!(
            key_hash(
                Dialect::OpenSplice,
                Encoding::PlainCdr1,
                type_name,
                &sample(type_name)
            ),
            expected,
            "{type_name}"
        );
    }
}

#[test]
fn the_rti_and_cyclone_dialects_align_the_holder_to_eight_under_xcdr1() {
    for dialect in [Dialect::Rti, Dialect::Cyclone] {
        for (type_name, expected) in XCDR1_ALIGNED {
            assert_eq!(
                key_hash(dialect, Encoding::PlainCdr1, type_name, &sample(type_name)),
                expected,
                "{dialect:?} {type_name}"
            );
        }
    }
}

#[test]
fn the_clause_holds_for_the_specification_and_for_every_measured_xcdr2_holder() {
    for dialect in [
        Dialect::Specification,
        Dialect::FastDds,
        Dialect::Rti,
        Dialect::Cyclone,
    ] {
        for (type_name, expected) in CLAUSE {
            assert_eq!(
                key_hash(dialect, Encoding::PlainCdr2, type_name, &sample(type_name)),
                expected,
                "{dialect:?} {type_name}"
            );
        }
    }
}

/// `CombSample` with `names = []`: a `@key` keyless `@appendable` level
/// whose `sequence<string>` goes into the holder. Cyclone 11.0.1 and Fast
/// DDS 3.6.1 computed the MD5 of `ffed2979 00000004 00000000`, the
/// sequence with its DHEADER; RTI 7.7.0 under XCDR2 that of
/// `ffed2979 00000000`, without it (its collection delimiter,
/// `vendor_bytes.md`). With two names Fast DDS aborts in `compute_key`, so
/// the empty sequence is the one shape all three computed.
#[test]
fn a_sequence_of_strings_in_the_holder_keeps_its_dheader_except_under_rti() {
    let empty = json!({"middle": {"leaf": {"k": -1_234_567, "v": -2}, "names": []}, "tail": -1});
    for dialect in [Dialect::Specification, Dialect::FastDds, Dialect::Cyclone] {
        assert_eq!(
            key_hash(dialect, Encoding::PlainCdr2, "CombSample", &empty),
            "c14ff8ae1e3185d9e12d88af6b1a1458",
            "{dialect:?}"
        );
    }
    assert_eq!(
        key_hash(Dialect::Rti, Encoding::PlainCdr2, "CombSample", &empty),
        "81485919ce1f729031bd5438ec2372f2"
    );
}
