// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-RPC codegen (interface operations → per-operation request/reply wire
//! structs). String smoke tests (always) + a byte-identity test that compiles
//! and runs the generated Go (gated on `go` being on PATH). Mirrors idl-rust's
//! `rpc_codegen.rs`: the request struct carries `in`+`inout`, the reply carries
//! `return_value`+`out`+`inout`, a `oneway` reply is empty. The structs are
//! ordinary `@appendable` (the default), so their XCDR2 wire is byte-identical
//! to the structs idl-rust's `emit_service_rpc` synthesises for the same IDL.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_go::{GoGenOptions, generate_go_module};

/// Service covering all four direction/return shapes:
/// * `add`       — value return, two `in` params.
/// * `transform` — `void`, one `in`, one `inout`, one `out`.
/// * `log`       — `oneway`, one `in`.
const SERVICE_IDL: &str = "@service interface Calc { \
     long add(in long a, in long b); \
     void transform(in long factor, inout long acc, out long doubled); \
     oneway void log(in string msg); \
 };";

fn emit(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::full_4_2()).expect("parse");
    generate_go_module(&ast, &GoGenOptions::default()).expect("gen")
}

/// Returns the substring of `src` starting at `header` up to the next blank line
/// gap after the type body (best-effort section slice for assertions).
fn section<'a>(src: &'a str, header: &str) -> &'a str {
    let start = src
        .find(header)
        .unwrap_or_else(|| panic!("no `{header}` in:\n{src}"));
    let rest = &src[start..];
    // Cut at the start of the next generated type declaration.
    match rest[header.len()..].find("\ntype ") {
        Some(i) => &rest[..header.len() + i],
        None => rest,
    }
}

#[test]
fn request_structs_carry_in_and_inout_params() {
    let go = emit(SERVICE_IDL);
    // add: two `in` params.
    assert!(go.contains("type Calc_add_Request struct {"), "{go}");
    let add_req = section(&go, "type Calc_add_Request struct {");
    assert!(add_req.contains("A int32"), "in a:\n{add_req}");
    assert!(add_req.contains("B int32"), "in b:\n{add_req}");

    // transform request: `in` factor + `inout` acc, NOT the `out` doubled.
    let tr_req = section(&go, "type Calc_transform_Request struct {");
    assert!(tr_req.contains("Factor int32"), "in factor:\n{tr_req}");
    assert!(tr_req.contains("Acc int32"), "inout acc:\n{tr_req}");
    assert!(
        !tr_req.contains("Doubled"),
        "out param must not be in the request:\n{tr_req}"
    );
}

#[test]
fn reply_structs_carry_return_out_and_inout() {
    let go = emit(SERVICE_IDL);
    // add reply: the return value only.
    let add_reply = section(&go, "type Calc_add_Reply struct {");
    assert!(
        add_reply.contains("Return_value int32"),
        "return value field:\n{add_reply}"
    );

    // transform reply: inout acc + out doubled, no return_value (void).
    let tr_reply = section(&go, "type Calc_transform_Reply struct {");
    assert!(
        tr_reply.contains("Acc int32"),
        "inout in reply:\n{tr_reply}"
    );
    assert!(
        tr_reply.contains("Doubled int32"),
        "out in reply:\n{tr_reply}"
    );
    assert!(
        !tr_reply.contains("Return_value"),
        "void op has no return_value:\n{tr_reply}"
    );
}

#[test]
fn oneway_reply_struct_is_empty_and_request_keeps_in_param() {
    let go = emit(SERVICE_IDL);
    // Empty reply body: `type Calc_log_Reply struct {\n}`.
    assert!(
        go.contains("type Calc_log_Reply struct {\n}"),
        "oneway reply must be empty:\n{go}"
    );
    // The oneway request still carries its `in` param.
    let log_req = section(&go, "type Calc_log_Request struct {");
    assert!(
        log_req.contains("Msg string"),
        "oneway in param:\n{log_req}"
    );
}

#[test]
fn service_name_annotation_overrides_struct_prefix() {
    // @service(name="Arith") renames the emitted request/reply structs.
    let go = emit("@service(name=\"Arith\") interface Calc { long add(in long a); };");
    assert!(go.contains("type Arith_add_Request struct {"), "{go}");
    assert!(go.contains("type Arith_add_Reply struct {"), "{go}");
    assert!(
        !go.contains("Calc_add_Request"),
        "old name must be gone:\n{go}"
    );
}

#[test]
fn oneway_annotation_form_is_recognised() {
    // `@oneway` annotation is equivalent to the `oneway` keyword: empty reply.
    let go = emit("interface S { @oneway void ping(in long n); };");
    assert!(go.contains("type S_ping_Request struct {"), "{go}");
    assert!(
        go.contains("type S_ping_Reply struct {\n}"),
        "@oneway reply must be empty:\n{go}"
    );
}

#[test]
fn interface_without_operations_emits_no_rpc_section() {
    // A pure data-holding interface (only a nested type, no ops) must not emit
    // the DDS-RPC header or any `_Request`/`_Reply` struct.
    let go = emit("interface Holder { struct Inner { long x; }; };");
    assert!(!go.contains("zerodds-rpc-1.0"), "no RPC header:\n{go}");
    assert!(!go.contains("_Request"), "no request struct:\n{go}");
    // The nested data type still survives (#A39).
    assert!(go.contains("struct {"), "{go}");
}

#[test]
fn param_type_from_enclosing_module_resolves() {
    // A parameter whose type is a struct declared in the same module resolves
    // to a nested-marshal field (byte-identical to a plain struct member).
    let go = emit(
        "module m { \
           @final struct Point { long x; long y; }; \
           interface Geo { Point mid(in Point a, in Point b); }; \
         };",
    );
    // Module-nested: the `Geo_mid_Request` name is flattened under scope `m`;
    // flatten_path doubles the name's own underscores → `m_Geo__mid__Request`
    // (injective, wire-irrelevant — idl-rust emits `Geo_mid_Request` in `mod m`).
    assert!(
        go.contains("type m_Geo__mid__Request struct {"),
        "module-nested request type:\n{go}"
    );
    // The request carries the two Point params as nested struct fields, resolved
    // to the module-qualified `m_Point` Go type.
    assert!(go.contains("m_Point"), "param struct type resolved:\n{go}");
}

// ---------------------------------------------------------------------------
// Byte-identity: compile + run the generated Go and check the wire bytes.
// The request/reply structs are @appendable (default) — DHEADER(len) + body —
// exactly as idl-rust emits them, so these bytes match the Rust wire.
// ---------------------------------------------------------------------------

fn go_run_hex(idl: &str, main_body: &str, tag: &str) -> Option<Vec<String>> {
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP {tag}: `go` not on PATH (Linux/CI runs the wire check)");
        return None;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(idl, &ParserConfig::full_4_2()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(main_body);
    // Add the fmt/hex imports the harness main needs (the module imports `math`).
    let src = if src.contains("import \"math\"\n") {
        src.replacen(
            "import \"math\"\n",
            "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
            1,
        )
    } else {
        src.replacen(
            "package main\n",
            "package main\n\nimport (\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
            1,
        )
    };
    let dir = std::env::temp_dir().join(format!("idlgo_rpc_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    Some(lines)
}

#[test]
fn request_reply_wire_is_appendable_byte_identical() {
    let main_body = r#"
func main() {
	req := Calc_add_Request{A: 2, B: 3}
	fmt.Println(hex.EncodeToString(req.MarshalXCDR(Little)))
	fmt.Println(hex.EncodeToString(req.MarshalXCDR(Big)))
	// Reply of the oneway op: empty appendable struct → DHEADER(0) only.
	var empty Calc_log_Reply
	fmt.Println(hex.EncodeToString(empty.MarshalXCDR(Little)))
	// Round-trip the request.
	back := UnmarshalXCDRCalc_add_Request(req.MarshalXCDR(Little), Little)
	fmt.Printf("%d,%d\n", back.A, back.B)
}
"#;
    let Some(lines) = go_run_hex(SERVICE_IDL, main_body, "wire") else {
        return;
    };
    // @appendable XCDR2 LE: DHEADER=8 (08000000) + a=2 (02000000) + b=3 (03000000).
    assert_eq!(lines[0], "080000000200000003000000", "LE request wire");
    // BE: DHEADER=8 (00000008) + a (00000002) + b (00000003).
    assert_eq!(lines[1], "000000080000000200000003", "BE request wire");
    // Empty oneway reply: DHEADER=0 only.
    assert_eq!(lines[2], "00000000", "empty reply wire");
    // Round-trip fidelity.
    assert_eq!(lines[3], "2,3", "request round-trip");
}
