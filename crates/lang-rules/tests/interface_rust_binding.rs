// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the generated Rust binding of an interface passes to the DDS-RPC
//! run time: the topic type names, the base interfaces and the QoS the
//! interface declares.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

fn rust(idl: &str) -> String {
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    zerodds_lang_rules::generate::source(&model(idl), Language::Rust, key)
        .unwrap_or_else(|error| panic!("{error:?}"))
}

/// §7.10.1: `@Qos(RequestProfile, ReplyProfile)` is read from the
/// interface and the binding offers the constructors that apply it.
#[test]
fn the_qos_the_interface_declares_is_applied_by_a_constructor() {
    let out = rust(
        r#"module demo {
             @Qos(RequestProfile = "file:///etc/rpc/qos.xml#Calc", ReplyProfile = "file:///etc/rpc/reply.xml#Calc")
             interface Calculator { long add(in long a, in long b); };
           };"#,
    );
    assert!(out.contains("fn new_with_declared_qos("), "{out}");
    assert!(out.contains("fn serve_with_declared_qos("), "{out}");
    assert!(
        out.contains("RpcQos::from_service_profiles(")
            || out.contains("zerodds_rpc::qos_profile::RpcQos::from_service_profiles("),
        "{out}"
    );
    assert!(out.contains("file:///etc/rpc/qos.xml#Calc"), "{out}");
    assert!(out.contains("file:///etc/rpc/reply.xml#Calc"), "{out}");
}

#[test]
fn an_interface_without_qos_has_no_declared_qos_constructor() {
    let out = rust("interface Plain { void ping(); };");
    assert!(!out.contains("with_declared_qos"), "{out}");
}

/// §7.5.1.1.6/7 and §7.4.3: the endpoints are built with the type names
/// of the interface's scope and the base interfaces as aliases.
#[test]
fn the_endpoints_are_built_with_the_type_names_and_the_parents() {
    let out = rust(
        "module demo { interface Base { void ping(); };
                       interface Derived : Base { void pong(); }; };",
    );
    assert!(out.contains("RpcEndpointOptions::for_interface("), "{out}");
    assert!(out.contains("\"demo\""), "{out}");
    assert!(out.contains("with_parent(\"demo_Base\")"), "{out}");
    assert!(out.contains("with_options("), "{out}");
}
