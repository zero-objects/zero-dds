// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-webdds-bridge` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules` (`src/rpc_dispatch.rs`,
//! `src/wsdl_bridge.rs`): `adapter::interface::services` and
//! `all_operations_flattened` for the operation model the WSDL and the
//! DDS-RPC dispatch both read, and `operation_hash`, `in_struct_name`,
//! `out_struct_name` and `exception_member_name` for the names and
//! discriminators of the request and reply topic types, which have to be
//! those of a Replier `zerodds-idlc` generated for the same interface.
//!
//! Oracle: DDS-RPC 1.0 §7.5.1.1.2 (the HASH of a name: the first four
//! octets of its MD5, little endian; `add` is 0xFC78EC34, computed outside
//! this tree), §7.5.1.1.3 (an attribute implies `get_attribute_<name>` and,
//! unless read-only, `set_attribute_<name>`), §7.5.1.1.4 and §7.5.1.1.5
//! (`${interface}_${operation}_In` and `_Out`; an exception case is the
//! lower-case exception name with `_ex`), §7.4.1 (the interface name for the
//! topic is the module names and the interface joined by underscores).
#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use zerodds_lang_rules::adapter::interface::{
    all_operations_flattened, exception_member_name, in_struct_name, operation_hash,
    out_struct_name, services,
};

fn model(idl: &str) -> zerodds_ast::Ast {
    // The bridge's own loader of an IDL file (`zerodds-wire-probe`).
    zerodds_wire_probe::encode::model_from_idl(idl, "contract.idl").unwrap()
}

#[test]
fn rpc_7_5_1_1_2_the_discriminator_of_an_operation_is_the_hash_of_its_name() {
    assert_eq!(operation_hash("add") as u32, 0xFC78_EC34);
    assert_eq!(operation_hash("swap") as u32, 0xDCDF_A1F0);
}

#[test]
fn rpc_7_5_1_1_4_the_request_and_reply_structures_and_the_exception_case_are_named_by_the_clause() {
    assert_eq!(in_struct_name("Calculator", "add"), "Calculator_add_In");
    assert_eq!(out_struct_name("Calculator", "add"), "Calculator_add_Out");
    assert_eq!(exception_member_name("DivByZero"), "divbyzero_ex");
}

#[test]
fn rpc_7_5_1_1_3_an_attribute_brings_its_implied_operations_and_a_base_its_own() {
    let ast = model(
        "module robot {
           interface Base { long ping(in long x); };
           interface Calculator : Base { long add(in long a, in long b); attribute long memory; };
         };",
    );
    let services = services(&ast).unwrap();
    let calculator = services.iter().find(|s| s.name == "Calculator").unwrap();
    assert_eq!(calculator.qualified_name, "robot_Calculator", "§7.4.1");
    let names: Vec<String> = all_operations_flattened(&ast, calculator.id)
        .unwrap()
        .into_iter()
        .map(|operation| operation.name)
        .collect();
    for wanted in [
        "ping",
        "add",
        "get_attribute_memory",
        "set_attribute_memory",
    ] {
        assert!(names.iter().any(|n| n == wanted), "`{wanted}` in {names:?}");
    }
}
