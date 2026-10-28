// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.3.1.4 in the generated Rust binding: a pair of types
//! marked `@RPCRequestType` / `@RPCReplyType`.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

fn rust(idl: &str) -> Result<String, String> {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "pair.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    zerodds_lang_rules::generate::source(&ast, Language::Rust, key).map_err(|e| e.to_string())
}

const HEADERS: &str = "module dds { module rpc {
    struct RequestHeader { long id; };
    struct ReplyHeader { long id; };
}; };";

#[test]
fn a_pair_with_headers_gets_the_header_traits_and_the_endpoints() {
    let out = rust(&format!(
        "{HEADERS}
         module robot {{
           @RPCRequestType @DDSRequestTopic(name=\"MoveRequests\")
           struct Req {{ dds::rpc::RequestHeader header; long x; }};
           @RPCReplyType struct Rep {{ dds::rpc::ReplyHeader header; long y; }};
         }};"
    ))
    .unwrap();
    assert!(
        out.contains("impl zerodds_rpc::header_pair::RequestWithHeader for Req"),
        "{out}"
    );
    assert!(
        out.contains("impl zerodds_rpc::header_pair::ReplyWithHeader for Rep"),
        "{out}"
    );
    assert!(out.contains("pub fn requester("), "{out}");
    assert!(out.contains("pub fn replier("), "{out}");
    assert!(out.contains("Requester::with_pair"), "{out}");
    assert!(out.contains("\"MoveRequests\""), "{out}");
    // The topic types are named by their scoped names.
    assert!(out.contains("\"robot::Req\""), "{out}");
    assert!(out.contains("\"robot::Rep\""), "{out}");
    assert!(out.contains("RpcMapping::Basic"), "{out}");
}

/// "When using the Enhanced Service Mapping, the annotations are not
/// necessary and the special data members must not be defined": without
/// `header` the pair is the Enhanced mapping and needs no header traits.
#[test]
fn a_pair_without_headers_is_the_enhanced_mapping() {
    let out = rust(
        "module robot {
           @RPCRequestType struct Req { long x; };
           @RPCReplyType struct Rep { long y; };
         };",
    )
    .unwrap();
    assert!(!out.contains("RequestWithHeader"), "{out}");
    assert!(out.contains("RpcMapping::Enhanced"), "{out}");
    assert!(out.contains("Requester::with_options"), "{out}");
}

#[test]
fn a_header_in_only_one_type_is_refused() {
    let err = rust(&format!(
        "{HEADERS}
         @RPCRequestType struct Req {{ dds::rpc::RequestHeader header; long x; }};
         @RPCReplyType struct Rep {{ long y; }};"
    ))
    .unwrap_err();
    assert!(err.contains("header"), "{err}");
}

#[test]
fn a_marked_type_that_is_not_a_struct_is_refused() {
    let err = rust("@RPCRequestType enum E { A, B };").unwrap_err();
    assert!(err.contains("not a struct"), "{err}");
}

/// A topic annotation whose argument is not a string names no topic;
/// ignoring it would leave the endpoints on the default names.
#[test]
fn a_topic_annotation_that_is_not_a_string_is_refused() {
    let err = rust(
        "@RPCRequestType @DDSRequestTopic(name=7) struct Req { long x; };
         @RPCReplyType struct Rep { long y; };",
    )
    .unwrap_err();
    assert!(err.contains("DDSRequestTopic"), "{err}");
}

/// §7.4.1: the `<service_name>` is user-supplied, by `@service(name=…)` or the
/// `@DDSService(name=…)` of the specification's examples, on either type; at
/// run time the endpoint takes it from the params (§7.4.2.3).
#[test]
fn the_service_name_is_read_from_either_annotation_and_can_be_given_at_run_time() {
    for annotation in ["@service(name=\"Move\")", "@DDSService(name=\"Move\")"] {
        let out = rust(&format!(
            "{annotation} @RPCRequestType struct Req {{ long x; }};
             @RPCReplyType struct Rep {{ long y; }};"
        ))
        .unwrap();
        assert!(
            out.contains("service_name: Some(\"Move\".to_string())"),
            "{annotation}: {out}"
        );
        assert!(out.contains("pub fn requester_with("), "{out}");
        assert!(out.contains("pub fn replier_with("), "{out}");
    }
    let err = rust(
        "@RPCRequestType @service(name=\"A\") struct Req { long x; };
         @RPCReplyType @DDSService(name=\"B\") struct Rep { long y; };",
    )
    .unwrap_err();
    assert!(err.contains("\"A\"") && err.contains("\"B\""), "{err}");
}
