// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.5.1.1.5 rule 4a names the label constant of an
//! exception `${exceptionType}_Ex_Hash` — after the exception alone. Two
//! interfaces of one module that raise the same exception therefore share
//! one constant; declaring it once per interface declares it twice in one
//! scope.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_lang_rules::emit::{interface_cpp, interface_java};

const TWO_INTERFACES: &str = "module m {
    exception Boom { long code; };
    interface A { void f() raises (Boom); };
    interface B { void g() raises (Boom); };
};";

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

#[test]
fn java_declares_the_exception_hash_once_per_module() {
    let text = interface_java::java_interface_items(&model(TWO_INTERFACES))
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(zerodds_lang_render::java::type_decl)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(text.matches("int Boom_Ex_Hash").count(), 1, "{text}");
    assert!(
        text.contains("int A_f_Hash") && text.contains("int B_g_Hash"),
        "{text}"
    );
}

#[test]
fn cpp_declares_the_exception_hash_once_per_module() {
    let text = interface_cpp::cpp_interface_items(&model(TWO_INTERFACES))
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(zerodds_lang_render::cpp::decl)
        .collect::<Vec<_>>()
        .join("\n");
    let declarations = text
        .lines()
        .filter(|line| line.contains("Boom_Ex_Hash") && line.contains("constexpr"))
        .count();
    assert_eq!(declarations, 1, "{text}");
}
