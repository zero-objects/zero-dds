// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rpc-rust-e2e` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules` (`build.rs`, a build
//! dependency): `generate::source` for the Rust binding of the `Calculator`,
//! `W`, hierarchy and nested interfaces under the Basic service mapping, and
//! `generate::source_with_mapping` with `ServiceMapping::Enhanced` and
//! `EnhancedEmptyStructures`; `src/lib.rs` includes the results as modules.
//! The crate's tests run the generated clients and dispatchers against each
//! other and against C++ and Java peers, so they rely on the generated
//! topic types of DDS-RPC 1.0 §7.5.1.1.
//!
//! Oracle: DDS-RPC 1.0 §7.5.1.1.1 (per operation a structure
//! `${interface}_${operation}_In` with the `in` and `inout` parameters as
//! members, in declaration order), §7.5.1.1.2 (the `Call` union has a case
//! per operation), §7.5.1.1.4 (per operation a `_Result` union whose
//! `raises` exceptions are cases), and DDS-XTypes 1.3 §7.4.3.4.2 with
//! §7.4.1.1.1 (the octets of an aggregate under XCDR2: the members of a
//! FINAL one alone, each `long` four octets).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_rpc_rust_e2e::calculator::demo::{Calculator_Call, Calculator_add_In, DivByZero};
use zerodds_wire::{Endianness, Reader, Writer};

#[test]
fn rpc_7_5_1_1_1_the_in_structure_holds_the_in_parameters_in_order() {
    let input = Calculator_add_In { a: 1, b: 2 };
    let mut writer = Writer::with_max_align(Endianness::Little, 4);
    input.encode(&mut writer).unwrap();
    let bytes = writer.finish();
    assert_eq!(&bytes[bytes.len() - 8..], &[1, 0, 0, 0, 2, 0, 0, 0]);
    let mut reader = Reader::new(&bytes, Endianness::Little);
    let back = Calculator_add_In::decode(&mut reader).unwrap();
    assert_eq!((back.a, back.b), (1, 2));
}

#[test]
fn rpc_7_5_1_1_2_the_call_union_has_a_case_per_operation() {
    let call = Calculator_Call::Add(Calculator_add_In { a: 3, b: 4 });
    match call {
        Calculator_Call::Add(input) => assert_eq!(input.a + input.b, 7),
        _ => panic!("the case of `add`"),
    }
}

#[test]
fn rpc_7_5_1_1_4_a_raised_exception_is_a_structure_of_its_members() {
    let exception = DivByZero {
        msg: "division by zero".into(),
    };
    assert_eq!(exception.msg, "division by zero");
}
