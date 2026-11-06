// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-webdds-bridge` with `zerodds-wire-probe`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-probe`: `encode::model_from_idl`
//! and nothing else — the IDL front end the daemon lowers its configured
//! files with: the topic types in `src/types_registry.rs`
//! (`load_idl_file`, then the type by its scoped name `Mod::Type`), the
//! interfaces of the DDS-RPC dispatchers and of the WSDL in
//! `src/daemon/server.rs`, `src/wsdl_bridge.rs` (`operations_of`),
//! `src/rpc_dispatch.rs` and `src/dynamic_rpc.rs`. A failure is reported
//! with the file name in front of the error's text.
//!
//! So the crate relies on: a type declared in a module found under its
//! scoped name; an interface's operations, also inherited ones, in the
//! model; a diagnostic refusing the whole file with a text that names
//! what is wrong.
//!
//! Oracles: OMG IDL 4.2 §7.4.1.4.4.4.1 (a structure, its members in
//! declaration order), §7.4.3.4.3.2 (a derived interface has the
//! operations of its base), §7.5.1 and §7.5.2 (a scoped name `m::T`; a
//! name that resolves to no declaration is an error).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_ast_core::ScopedName;
use zerodds_wire_probe::encode::model_from_idl;

#[test]
fn idl_7_5_1_a_type_in_a_module_is_found_by_its_scoped_name() {
    let ast = model_from_idl(
        "module shapes { @final struct Square { @key string color; long x; long y; }; };",
        "shapes.idl",
    )
    .unwrap();
    let id = ast
        .find_by_scoped_name(&ScopedName::absolute(vec![
            "shapes".to_string(),
            "Square".to_string(),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("shapes::Square is a type");
    let names: Vec<String> = ast
        .members_of(id)
        .iter()
        .map(|member| ast.member(*member).unwrap().name.clone())
        .collect();
    assert_eq!(names, ["color", "x", "y"]);
    assert!(
        ast.find_by_scoped_name(&ScopedName::absolute(vec!["Square".to_string()]))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .is_none(),
        "outside its module the bare name is not the type"
    );
}

#[test]
fn idl_7_4_3_4_3_2_an_interface_carries_its_own_and_its_inherited_operations() {
    let ast = model_from_idl(
        "module calc { interface Base { long ping(in long v); }; \
         interface Adder : Base { long add(in long a, in long b); }; };",
        "calc.idl",
    )
    .unwrap();
    let services = zerodds_webdds_bridge::wsdl_bridge::operations_of(&ast).unwrap();
    let adder = services
        .iter()
        .find(|(service, _)| service.name == "Adder")
        .expect("the derived interface is a service");
    let mut names: Vec<&str> = adder.1.iter().map(|op| op.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["add", "ping"]);
}

#[test]
fn idl_7_5_2_a_diagnostic_refuses_the_whole_file_and_names_what_is_wrong() {
    let error = model_from_idl(
        "module shapes { @final struct Square { Colour color; long x; }; };",
        "shapes.idl",
    )
    .err()
    .expect("an undeclared member type is no model");
    assert!(format!("{error}").contains("Colour"), "{error}");
}
