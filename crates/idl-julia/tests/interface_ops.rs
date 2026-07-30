// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Julia backend: interface operations/attributes (#11). Previously the Julia
//! backend dropped every interface `Export::Op`/`Export::Attr`; it now emits a
//! native `<Iface>_Client` / `<Iface>_Handler` surface (abstract types + a
//! generic-function declaration per operation/attribute accessor) mirroring the
//! idl-ts / idl-swift interface surface. Operations carry no wire form — there
//! is no Julia `zerodds-rpc` runtime, so no requester/replier wrapper is
//! invented; these are string-level tests plus a `julia` conformance compile.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_julia::{JuliaGenOptions, generate_julia_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_julia_module(&ast, &JuliaGenOptions::default()).expect("gen")
}

#[test]
fn interface_emits_client_and_handler_abstract_types() {
    let s = emit("interface Calc { long add(in long a, in long b); void reset(); };");
    assert!(s.contains("abstract type Calc_Client end"), "{s}");
    assert!(s.contains("abstract type Calc_Handler end"), "{s}");
    // Operation surface: a generic function declaration per op, documented.
    assert!(s.contains("function add end"), "{s}");
    assert!(s.contains("function reset end"), "{s}");
    assert!(
        s.contains("operation: add(a::Int32, b::Int32)::Int32"),
        "{s}"
    );
    assert!(s.contains("operation: reset()::Nothing"), "{s}");
}

#[test]
fn out_and_inout_params_fold_into_return_tuple() {
    let s = emit("interface Svc { long compute(in long x, out long hi, inout long acc); };");
    // in + inout become arguments; return folds result + out + inout types.
    assert!(
        s.contains("operation: compute(x::Int32, acc::Int32)::Tuple{Int32, Int32, Int32}"),
        "{s}"
    );
    assert!(s.contains("function compute end"), "{s}");
}

#[test]
fn single_out_param_void_return_is_bare_type() {
    let s = emit("interface Q { void peek(out long v); };");
    assert!(s.contains("operation: peek()::Int32"), "{s}");
}

#[test]
fn readonly_attribute_emits_only_getter() {
    let s =
        emit("interface Sensor { readonly attribute double temperature; attribute long gain; };");
    assert!(s.contains("function get_temperature end"), "{s}");
    assert!(!s.contains("function set_temperature end"), "{s}");
    // A writable attribute gets both a getter and a setter.
    assert!(s.contains("function get_gain end"), "{s}");
    assert!(s.contains("function set_gain end"), "{s}");
}

#[test]
fn interface_base_becomes_julia_supertype() {
    // `Root`/`Sub` avoid Julia stdlib names (`Base` is escaped to `Base_`).
    let s = emit("interface Root { void ping(); }; interface Sub : Root { void go(); };");
    assert!(
        s.contains("abstract type Sub_Client <: Root_Client end"),
        "{s}"
    );
    assert!(
        s.contains("abstract type Sub_Handler <: Root_Handler end"),
        "{s}"
    );
}

#[test]
fn interface_nested_type_still_round_trips_alongside_surface() {
    // #A39: the interface's nested DATA type is still emitted (promoted to the
    // top level as `I_C`), while the operation surface emits separately.
    let s = emit("interface I { struct C { long v; }; C fetch(); };");
    assert!(s.contains("struct I_C"), "{s}");
    assert!(
        s.contains("function marshal_into!(v::I_C, w::Writer)"),
        "{s}"
    );
    assert!(s.contains("function fetch end"), "{s}");
    assert!(s.contains("operation: fetch()::I_C"), "{s}");
}

/// Julia conformance compile (gated on `julia`): the emitted Client/Handler
/// surface plus a concrete subtype and method must load and run.
#[test]
fn interface_surface_compiles_with_julia() {
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!(
            "SKIP interface_surface_compiles_with_julia: `julia` not on PATH (Linux/CI-only)"
        );
        return;
    }
    let mut src = emit("interface Calc { long add(in long a, in long b); };");
    src.push_str(
        r#"
struct MyCalc <: Calc_Handler end
add(::MyCalc, a::Int32, b::Int32)::Int32 = a + b
function main()
    println(add(MyCalc(), Int32(2), Int32(3)))
end
main()
"#,
    );
    let dir = std::env::temp_dir().join(format!("idljulia_iface_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "5");
    let _ = std::fs::remove_dir_all(&dir);
}
