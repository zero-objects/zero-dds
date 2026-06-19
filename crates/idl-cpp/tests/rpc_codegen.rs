//! Integration tests for C6.1.D-cpp — DDS-RPC C++ PSM codegen.
//!
//! Spec-Referenz: OMG DDS-RPC 1.0 §10 (C++ PSM).
//!
//! The tests check the emitted headers per service type as marker-
//! Snapshots — no real C++ compile (see "non-goals" in the plan).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::ast::{
    ExceptDecl, FloatingType, Identifier, IntegerType, Member, PrimitiveType, SequenceType,
    StringType, TypeSpec,
};
use zerodds_idl::errors::Span;
use zerodds_idl_cpp::rpc::{
    emit_remote_exception_class, emit_replier_class, emit_requester_class,
    emit_rpc_runtime_headers, emit_service_full_header, emit_service_interface,
    emit_service_traits,
};
use zerodds_rpc::{MethodDef, ParamDef, ParamDirection, ServiceDef};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sp() -> Span {
    Span::SYNTHETIC
}

fn ident(t: &str) -> Identifier {
    Identifier::new(t, sp())
}

fn long_t() -> TypeSpec {
    TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long))
}

fn double_t() -> TypeSpec {
    TypeSpec::Primitive(PrimitiveType::Floating(FloatingType::Double))
}

fn string_t() -> TypeSpec {
    TypeSpec::String(StringType {
        wide: false,
        bound: None,
        span: sp(),
    })
}

fn p(name: &str, dir: ParamDirection, ty: TypeSpec) -> ParamDef {
    ParamDef {
        name: name.into(),
        direction: dir,
        type_ref: ty,
    }
}

fn meth(name: &str, params: Vec<ParamDef>, ret: Option<TypeSpec>, oneway: bool) -> MethodDef {
    MethodDef {
        name: name.into(),
        params,
        return_type: ret,
        oneway,
    }
}

fn calc_service() -> ServiceDef {
    ServiceDef {
        name: "Calculator".into(),
        methods: vec![meth(
            "add",
            vec![
                p("a", ParamDirection::In, long_t()),
                p("b", ParamDirection::In, long_t()),
            ],
            Some(long_t()),
            false,
        )],
    }
}

fn logger_service() -> ServiceDef {
    ServiceDef {
        name: "Logger".into(),
        methods: vec![meth(
            "log",
            vec![p("msg", ParamDirection::In, string_t())],
            None,
            true, // oneway
        )],
    }
}

fn swap_service() -> ServiceDef {
    ServiceDef {
        name: "Swap".into(),
        methods: vec![meth(
            "swap",
            vec![
                p("a", ParamDirection::InOut, long_t()),
                p("b", ParamDirection::InOut, long_t()),
            ],
            None,
            false,
        )],
    }
}

fn out_only_service() -> ServiceDef {
    ServiceDef {
        name: "Stat".into(),
        methods: vec![meth(
            "snapshot",
            vec![p("value", ParamDirection::Out, double_t())],
            None,
            false,
        )],
    }
}

fn empty_service() -> ServiceDef {
    ServiceDef {
        name: "Empty".into(),
        methods: vec![],
    }
}

fn member(ts: TypeSpec, name: &str) -> Member {
    use zerodds_idl::ast::Declarator;
    Member {
        type_spec: ts,
        declarators: vec![Declarator::Simple(ident(name))],
        annotations: Vec::new(),
        span: sp(),
    }
}

fn except_decl(name: &str, members: Vec<Member>) -> ExceptDecl {
    ExceptDecl {
        name: ident(name),
        members,
        annotations: Vec::new(),
        span: sp(),
    }
}

// ---------------------------------------------------------------------------
// 1. Service with 1 method — snapshot of the service interface
// ---------------------------------------------------------------------------

#[test]
fn service_interface_one_method_snapshot() {
    let s = emit_service_interface(&calc_service());
    assert!(s.contains("namespace dds { namespace rpc {"));
    assert!(s.contains("class Calculator {"));
    assert!(s.contains("virtual ~Calculator() = default;"));
    assert!(s.contains("virtual int32_t add(const int32_t& a, const int32_t& b) = 0;"));
    assert!(s.contains("virtual ::dds::rpc::Future<int32_t> add_async("));
    assert!(s.contains("class CalculatorHandlerInterface {"));
    assert!(s.contains("} } // namespace dds::rpc"));
}

#[test]
fn service_interface_methods_count_matches_signatures() {
    // 1 method → 1 sync + 1 async = 2 `virtual ... = 0` in the interface,
    // plus 1 sync im HandlerInterface. Gesamt = 3.
    let s = emit_service_interface(&calc_service());
    let count = s.matches("= 0;").count();
    assert_eq!(count, 3);
}

// ---------------------------------------------------------------------------
// 2. Service with a oneway method — no reply, no future
// ---------------------------------------------------------------------------

#[test]
fn oneway_method_has_no_async_in_interface() {
    let s = emit_service_interface(&logger_service());
    // sync log() is there, but no `log_async` (oneway).
    assert!(s.contains("virtual void log("));
    assert!(!s.contains("log_async"));
}

#[test]
fn oneway_method_emits_fire_and_forget_in_requester() {
    let s = emit_requester_class(&logger_service());
    assert!(s.contains("class Logger_Requester"));
    // Oneway: void return, no future, no promise.
    assert!(s.contains("virtual void log("));
    assert!(!s.contains("Promise<"));
    assert!(s.contains("// oneway: fire-and-forget"));
}

#[test]
fn oneway_replier_dispatch_invokes_handler_without_return() {
    let s = emit_replier_class(&logger_service());
    assert!(s.contains("if (method_name == \"log\")"));
    // Oneway: no `(void)handler_->log` cast — direct call.
    assert!(s.contains("handler_->log(msg);"));
}

// ---------------------------------------------------------------------------
// 3. Service with raises — RemoteException hierarchy
// ---------------------------------------------------------------------------

#[test]
fn idl_exception_emits_remote_exception_subclass() {
    let e = except_decl("NotFound", vec![member(string_t(), "what_")]);
    let s = emit_remote_exception_class(&e).unwrap();
    assert!(s.contains("class NotFound : public ::dds::rpc::RemoteException"));
    assert!(s.contains("std::string what__{}"));
    assert!(s.contains("explicit NotFound(std::string msg)"));
}

#[test]
fn empty_idl_exception_still_inherits_remote_exception() {
    let e = except_decl("EmptyEx", vec![]);
    let s = emit_remote_exception_class(&e).unwrap();
    assert!(s.contains("class EmptyEx : public ::dds::rpc::RemoteException"));
    // No `private:` block, since there are no members.
    assert!(!s.contains("private:"));
}

#[test]
fn idl_exception_with_reserved_name_is_rejected() {
    let e = except_decl("class", vec![]);
    let res = emit_remote_exception_class(&e);
    assert!(res.is_err());
}

#[test]
fn remote_exception_member_getters_and_setters() {
    let e = except_decl("BadInput", vec![member(long_t(), "code")]);
    let s = emit_remote_exception_class(&e).unwrap();
    assert!(s.contains("const int32_t& code() const noexcept"));
    assert!(s.contains("void code(const int32_t& v)"));
}

// ---------------------------------------------------------------------------
// 4. Service with out/inout params — reference mapping
// ---------------------------------------------------------------------------

#[test]
fn inout_params_emit_nonconst_reference() {
    let s = emit_service_interface(&swap_service());
    // inout → `int32_t&`; not `const int32_t&`.
    assert!(s.contains("virtual void swap(int32_t& a, int32_t& b) = 0;"));
    assert!(!s.contains("const int32_t& a"));
}

#[test]
fn out_only_param_emits_nonconst_reference() {
    let s = emit_service_interface(&out_only_service());
    assert!(s.contains("virtual void snapshot(double& value) = 0;"));
}

#[test]
fn in_param_emits_const_reference() {
    let s = emit_service_interface(&calc_service());
    assert!(s.contains("const int32_t& a"));
    assert!(s.contains("const int32_t& b"));
}

// ---------------------------------------------------------------------------
// 5. ServiceTraits-Spezialisierung pro Service
// ---------------------------------------------------------------------------

#[test]
fn service_traits_carries_topic_names() {
    let s = emit_service_traits(&calc_service());
    assert!(s.contains("template <>"));
    assert!(s.contains("struct ServiceTraits<Calculator>"));
    assert!(s.contains("\"Calculator_Request\""));
    assert!(s.contains("\"Calculator_Reply\""));
    assert!(s.contains("static constexpr bool is_specialized = true;"));
}

#[test]
fn service_traits_links_typedefs_for_endpoints() {
    let s = emit_service_traits(&calc_service());
    assert!(s.contains("using requester_type = Calculator_Requester;"));
    assert!(s.contains("using replier_type = Calculator_Replier;"));
    assert!(s.contains("using handler_interface_type = CalculatorHandlerInterface;"));
}

#[test]
fn service_traits_default_mapping_is_basic() {
    let s = emit_service_traits(&calc_service());
    assert!(s.contains("ServiceMapping::Basic"));
}

// ---------------------------------------------------------------------------
// 6. Multi-service in one IDL file / namespace hierarchy
// ---------------------------------------------------------------------------

#[test]
fn full_header_for_one_service_contains_pragma_once() {
    let s = emit_service_full_header(&calc_service());
    assert!(s.contains("#pragma once"));
}

#[test]
fn full_header_emits_all_four_blocks() {
    let s = emit_service_full_header(&calc_service());
    // Interface
    assert!(s.contains("class Calculator {"));
    // Requester
    assert!(s.contains("class Calculator_Requester"));
    // Replier
    assert!(s.contains("class Calculator_Replier"));
    // ServiceTraits
    assert!(s.contains("ServiceTraits<Calculator>"));
}

#[test]
fn multi_service_concatenation_keeps_namespaces_balanced() {
    let mut concat = String::new();
    concat.push_str(&emit_service_full_header(&calc_service()));
    concat.push_str(&emit_service_full_header(&logger_service()));
    let opens = concat.matches("namespace dds { namespace rpc {").count();
    let closes = concat.matches("} } // namespace dds::rpc").count();
    assert_eq!(opens, closes);
    assert!(opens >= 8); // 4 Bloecke pro Service * 2 Services
}

// ---------------------------------------------------------------------------
// Runtime-Header Sanity (templates-eingebettet)
// ---------------------------------------------------------------------------

#[test]
fn runtime_header_has_pragma_once_and_includes() {
    let s = emit_rpc_runtime_headers();
    assert!(s.contains("#pragma once"));
    assert!(s.contains("#include <future>"));
    assert!(s.contains("#include <string>"));
    assert!(s.contains("#include <dds/core/exceptions.hpp>"));
}

#[test]
fn runtime_header_exposes_remote_exception_subclasses() {
    let s = emit_rpc_runtime_headers();
    assert!(s.contains("class RemoteException"));
    assert!(s.contains("class UnknownOperationError"));
    assert!(s.contains("class UnknownExceptionError"));
    assert!(s.contains("class InvalidArgumentRemoteError"));
    assert!(s.contains("class OutOfResourcesRemoteError"));
    assert!(s.contains("class UnsupportedRemoteError"));
}

#[test]
fn runtime_header_defines_future_and_promise_templates() {
    let s = emit_rpc_runtime_headers();
    assert!(s.contains("class Future {"));
    assert!(s.contains("class Promise {"));
    assert!(s.contains("std::future<T>"));
    assert!(s.contains("std::promise<T>"));
}

#[test]
fn runtime_header_defines_generic_requester_and_replier() {
    let s = emit_rpc_runtime_headers();
    assert!(s.contains("class Requester {"));
    assert!(s.contains("class Replier {"));
    assert!(s.contains("send_request_async"));
    assert!(s.contains("dispatch_to_handler"));
}

#[test]
fn runtime_header_service_mapping_enum() {
    let s = emit_rpc_runtime_headers();
    assert!(s.contains("enum class ServiceMapping"));
    assert!(s.contains("Basic = 0,"));
    assert!(s.contains("Enhanced = 1,"));
}

// ---------------------------------------------------------------------------
// Edge-Cases
// ---------------------------------------------------------------------------

#[test]
fn empty_service_emits_minimal_interface_and_traits() {
    let s = emit_service_full_header(&empty_service());
    assert!(s.contains("class Empty {"));
    assert!(s.contains("class Empty_Requester"));
    assert!(s.contains("class Empty_Replier"));
    assert!(s.contains("ServiceTraits<Empty>"));
    // The replier has no if-branch.
    assert!(s.contains("(void)method_name;"));
}

#[test]
fn replier_with_unknown_method_throws() {
    let s = emit_replier_class(&calc_service());
    assert!(s.contains("throw ::dds::rpc::UnknownOperationError"));
}

#[test]
fn doxygen_oneway_marker_in_interface() {
    let s = emit_service_interface(&logger_service());
    assert!(s.contains("@oneway"));
}

#[test]
fn doxygen_param_directions_documented() {
    let s = emit_service_interface(&swap_service());
    assert!(s.contains("@param a (inout)"));
    assert!(s.contains("@param b (inout)"));
}

#[test]
fn requester_async_for_void_returning_method() {
    let svc = ServiceDef {
        name: "Notifier".into(),
        methods: vec![meth(
            "ping",
            vec![p("seq", ParamDirection::In, long_t())],
            None,
            false,
        )],
    };
    let s = emit_requester_class(&svc);
    // Non-oneway, void return → `Future<void>` async + sync `void ping(...)`.
    assert!(s.contains("::dds::rpc::Future<void> ping_async"));
    // The sync variant must not have `return`.
    assert!(s.contains("ping_async(seq).get();"));
    assert!(!s.contains("return ping_async"));
}

#[test]
fn requester_async_for_value_returning_method_uses_return() {
    let s = emit_requester_class(&calc_service());
    assert!(s.contains("return add_async(a, b).get();"));
}

#[test]
fn service_interface_sequence_param_emits_vector() {
    let seq_ty = TypeSpec::Sequence(SequenceType {
        elem: Box::new(long_t()),
        bound: None,
        span: sp(),
    });
    let svc = ServiceDef {
        name: "Bulk".into(),
        methods: vec![meth(
            "process",
            vec![p("data", ParamDirection::In, seq_ty)],
            Some(long_t()),
            false,
        )],
    };
    let s = emit_service_interface(&svc);
    assert!(s.contains("const std::vector<int32_t>& data"));
}
