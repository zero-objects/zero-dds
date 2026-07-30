// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Golden assertions for the native Rust DDS-RPC codegen (F.1 item 11).
//!
//! idl-rust used to drop an interface's operations behind an
//! `// UNSUPPORTED RPC` marker. It now emits real typed
//! `<Service>Requester`/`<Service>Replier` wrappers around the `zerodds_rpc`
//! runtime. These tests pin the shape of that output for a service that
//! exercises a value-return operation, an `inout`, an `out`, and a `oneway`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

/// Service covering all four direction/return shapes the codegen must handle:
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
    generate_rust_module(&ast, &RustGenOptions::default()).expect("gen")
}

fn emit_cdr_only(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::full_4_2()).expect("parse");
    generate_rust_module(
        &ast,
        &RustGenOptions {
            cdr_only: true,
            ..RustGenOptions::default()
        },
    )
    .expect("gen")
}

#[test]
fn no_unsupported_marker_is_emitted_anymore() {
    let src = emit(SERVICE_IDL);
    assert!(
        !src.contains("UNSUPPORTED RPC"),
        "the stopgap marker must be gone:\n{src}"
    );
}

#[test]
fn request_structs_carry_in_and_inout_params() {
    let src = emit(SERVICE_IDL);
    // add: two `in` params.
    assert!(src.contains("pub struct Calc_add_Request"), "{src}");
    assert!(src.contains("pub a: i32,"), "{src}");
    assert!(src.contains("pub b: i32,"), "{src}");
    // transform request: `in` factor + `inout` acc, NOT the `out` doubled.
    assert!(src.contains("pub struct Calc_transform_Request"), "{src}");
    let req = section(&src, "pub struct Calc_transform_Request");
    assert!(req.contains("pub factor: i32,"), "in param:\n{req}");
    assert!(req.contains("pub acc: i32,"), "inout param:\n{req}");
    assert!(
        !req.contains("pub doubled"),
        "out param must not be in the request:\n{req}"
    );
}

#[test]
fn reply_structs_carry_return_out_and_inout() {
    let src = emit(SERVICE_IDL);
    // add reply: the return value only.
    let add_reply = section(&src, "pub struct Calc_add_Reply");
    assert!(
        add_reply.contains("pub return_value: i32,"),
        "return value field:\n{add_reply}"
    );
    // transform reply: inout acc + out doubled, no return_value (void).
    let tr_reply = section(&src, "pub struct Calc_transform_Reply");
    assert!(
        tr_reply.contains("pub acc: i32,"),
        "inout in reply:\n{tr_reply}"
    );
    assert!(
        tr_reply.contains("pub doubled: i32,"),
        "out in reply:\n{tr_reply}"
    );
    assert!(
        !tr_reply.contains("return_value"),
        "void op has no return_value:\n{tr_reply}"
    );
}

#[test]
fn oneway_reply_struct_is_empty() {
    let src = emit(SERVICE_IDL);
    let reply = section(&src, "pub struct Calc_log_Reply");
    // Empty struct body — no fields.
    assert!(
        reply.trim_end().ends_with("pub struct Calc_log_Reply {\n}")
            || reply.contains("pub struct Calc_log_Reply {\n}"),
        "oneway reply must be empty:\n{reply}"
    );
    // The oneway request still carries its `in` param.
    assert!(src.contains("pub struct Calc_log_Request"), "{src}");
    assert!(
        section(&src, "pub struct Calc_log_Request").contains("pub msg:"),
        "oneway request keeps its in param:\n{src}"
    );
}

#[test]
fn requester_wraps_runtime_and_exposes_typed_methods() {
    let src = emit(SERVICE_IDL);
    assert!(src.contains("pub struct CalcRequester"), "{src}");
    // One runtime endpoint per operation.
    assert!(
        src.contains(
            "pub add_endpoint: zerodds_rpc::requester::Requester<Calc_add_Request, Calc_add_Reply>"
        ),
        "{src}"
    );
    // Value-return op: typed blocking method returning the reply struct.
    assert!(
        src.contains("pub fn add(&self, a: i32, b: i32, timeout: ::core::option::Option<::core::time::Duration>) -> zerodds_rpc::RpcResult<Calc_add_Reply>"),
        "{src}"
    );
    assert!(
        src.contains("self.add_endpoint.send_request_blocking(&__request, timeout)"),
        "{src}"
    );
    // inout/out op: params are the in+inout, reply carries the rest.
    assert!(
        src.contains("pub fn transform(&self, factor: i32, acc: i32, timeout:"),
        "{src}"
    );
    // Oneway op: fire-and-forget, no reply.
    assert!(
        src.contains("pub fn log(&self, msg: ") && src.contains("-> zerodds_rpc::RpcResult<()>"),
        "{src}"
    );
    assert!(
        src.contains("self.log_endpoint.send_oneway(&__request)"),
        "{src}"
    );
}

#[test]
fn handler_trait_has_typed_signatures() {
    let src = emit(SERVICE_IDL);
    assert!(
        src.contains("pub trait CalcHandler: ::core::marker::Send + ::core::marker::Sync"),
        "{src}"
    );
    assert!(
        src.contains("fn add(&self, request: Calc_add_Request) -> ::core::result::Result<Calc_add_Reply, zerodds_rpc::common_types::RemoteExceptionCode>;"),
        "{src}"
    );
    // Oneway handler returns nothing.
    assert!(
        src.contains("fn log(&self, request: Calc_log_Request);"),
        "{src}"
    );
}

#[test]
fn replier_wraps_runtime_with_dispatch_adapters_and_tick() {
    let src = emit(SERVICE_IDL);
    assert!(src.contains("pub struct CalcReplier"), "{src}");
    // Per-operation ReplierHandler adapter.
    assert!(
        src.contains(
            "impl zerodds_rpc::replier::ReplierHandler<Calc_add_Request, Calc_add_Reply> for Calc_add_Dispatch"
        ),
        "{src}"
    );
    // Replier endpoints created with the enhanced per-operation service names.
    assert!(
        src.contains("zerodds_rpc::replier::Replier::new(participant, \"Calc_add\", qos,"),
        "{src}"
    );
    // A tick() that services every operation.
    assert!(src.contains("pub fn tick(&self) -> usize"), "{src}");
    assert!(src.contains("self.add_endpoint.tick()"), "{src}");
    assert!(src.contains("self.log_endpoint.tick()"), "{src}");
}

#[test]
fn cdr_only_mode_emits_no_dds_rpc_runtime() {
    // The CORBA/GIOP path must not pull in zerodds_dcps/zerodds_rpc.
    let src = emit_cdr_only(SERVICE_IDL);
    assert!(
        !src.contains("zerodds_rpc::"),
        "cdr_only must not reference the DDS-RPC runtime:\n{src}"
    );
    assert!(
        !src.contains("pub struct CalcRequester"),
        "cdr_only must not emit DDS-RPC wrappers:\n{src}"
    );
    assert!(
        src.contains("CORBA") && src.contains("cdr_only"),
        "cdr_only must leave a visible CORBA-path note:\n{src}"
    );
}

#[test]
fn plain_interface_without_ops_emits_no_rpc_surface() {
    let src = emit("interface JustExc { exception Oops { string what; }; };");
    assert!(
        !src.contains("Requester"),
        "no RPC without operations:\n{src}"
    );
    // The nested exception is still emitted as a data type.
    assert!(src.contains("pub struct Oops"), "nested exception:\n{src}");
}

#[test]
fn interface_inner_exceptions_still_emitted_alongside_rpc() {
    let src = emit(
        "@service interface Svc { exception BadInput { string why; }; long op(in long a) raises (BadInput); };",
    );
    assert!(
        src.contains("pub struct BadInput"),
        "inner exception:\n{src}"
    );
    assert!(
        src.contains("pub struct SvcRequester"),
        "rpc wrapper:\n{src}"
    );
}

/// Returns the substring of `src` starting at the first occurrence of
/// `anchor`, for scoped assertions (up to the next top-level `impl`/`pub
/// struct` boundary is unnecessary — the struct decl block is small).
fn section<'a>(src: &'a str, anchor: &str) -> &'a str {
    let start = src
        .find(anchor)
        .unwrap_or_else(|| panic!("missing `{anchor}`:\n{src}"));
    let rest = &src[start..];
    // Cut at the blank line that separates the struct decl from its impls.
    match rest.find("\n\n") {
        Some(end) => &rest[..end],
        None => rest,
    }
}
