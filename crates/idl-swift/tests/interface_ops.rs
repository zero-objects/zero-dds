// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Swift backend: interface operations/attributes (#11). Previously the Swift
//! backend dropped every interface `Export::Op`/`Export::Attr`; it now emits
//! native `<Iface>_Client` / `<Iface>_Handler` protocols mirroring the idl-ts /
//! idl-rust interface surface. Operations carry no wire form, so these tests are
//! string-level plus a `swiftc` conformance compile (gated on `swiftc`).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_swift::{SwiftGenOptions, generate_swift_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_swift_module(&ast, &SwiftGenOptions::default()).expect("gen")
}

#[test]
fn interface_emits_client_and_handler_protocols() {
    let s = emit("interface Calc { long add(in long a, in long b); void reset(); };");
    assert!(s.contains("public protocol Calc_Client {"), "{s}");
    assert!(s.contains("public protocol Calc_Handler {"), "{s}");
    assert!(
        s.contains("func add(a: Int32, b: Int32) throws -> Int32"),
        "{s}"
    );
    assert!(s.contains("func reset() throws\n"), "{s}");
}

#[test]
fn out_and_inout_params_fold_into_return_tuple() {
    let s = emit("interface Svc { long compute(in long x, out long hi, inout long acc); };");
    // in + inout become parameters; return folds result + out + inout.
    assert!(
        s.contains(
            "func compute(x: Int32, acc: Int32) throws -> (result: Int32, hi: Int32, acc: Int32)"
        ),
        "{s}"
    );
}

#[test]
fn single_out_param_void_return_is_bare_type() {
    let s = emit("interface Q { void peek(out long v); };");
    assert!(s.contains("func peek() throws -> Int32"), "{s}");
}

#[test]
fn attributes_emit_get_and_conditional_set() {
    let s =
        emit("interface Sensor { readonly attribute long temperature; attribute string name; };");
    assert!(s.contains("func get_temperature() throws -> Int32"), "{s}");
    assert!(
        !s.contains("func set_temperature"),
        "readonly must have no setter:\n{s}"
    );
    assert!(s.contains("func get_name() throws -> String"), "{s}");
    assert!(s.contains("func set_name(_ value: String) throws"), "{s}");
}

#[test]
fn interface_bases_become_protocol_inheritance() {
    let s = emit("interface Base { void ping(); }; interface Derived : Base { long value(); };");
    assert!(
        s.contains("public protocol Derived_Client: Base_Client {"),
        "{s}"
    );
    assert!(
        s.contains("public protocol Derived_Handler: Base_Handler {"),
        "{s}"
    );
}

#[test]
fn interface_nested_type_param_resolves_to_promoted_name() {
    // #A39: the nested struct is promoted to the scope-encoded `Reg_sEntry`; a
    // param referencing it must resolve to that same flattened name.
    let s = emit("interface Reg { struct Entry { long id; }; void put(in Entry e); };");
    assert!(s.contains("public struct Reg_sEntry {"), "{s}");
    assert!(s.contains("func put(e: Reg_sEntry) throws"), "{s}");
}

#[test]
fn generated_protocols_compile_with_swiftc() {
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP generated_protocols_compile_with_swiftc: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(
        "interface Store { \
            long put(in string key, in long value); \
            long get(in string key); \
            readonly attribute long size; \
        };",
    );
    // A concrete conformer proves the protocol is well-formed Swift.
    src.push_str(
        "\nfinal class MemStore: Store_Handler {\n\
        \x20   var m: [String: Int32] = [:]\n\
        \x20   func put(key: String, value: Int32) throws -> Int32 { m[key] = value; return value }\n\
        \x20   func get(key: String) throws -> Int32 { return m[key] ?? 0 }\n\
        \x20   func get_size() throws -> Int32 { return Int32(m.count) }\n\
        }\n\
        let s = MemStore()\n\
        _ = try s.put(key: \"a\", value: 7)\n\
        print(try s.get(key: \"a\"))\n\
        print(try s.get_size())\n",
    );
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_iface_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let sf = dir.join("main.swift");
    std::fs::write(&sf, &src).expect("write");
    let build = Command::new("swiftc")
        .arg(&sf)
        .arg("-o")
        .arg(dir.join("main_bin"))
        .output()
        .expect("swiftc");
    assert!(
        build.status.success(),
        "swiftc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(dir.join("main_bin")).output().expect("run");
    let stdout = String::from_utf8(run.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(lines.next().expect("get").trim(), "7");
    assert_eq!(lines.next().expect("size").trim(), "1");
    let _ = std::fs::remove_dir_all(&dir);
}
