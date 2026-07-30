// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL `@service` interface → native Rust DDS-RPC codegen.
//!
//! Spec reference: OMG DDS-RPC 1.0 (`formal/16-12-04`). This module is the
//! Rust PSM counterpart of `idl-cpp`'s and `idl-java`'s `rpc.rs`: it takes the
//! typed service model from `zerodds-rpc`
//! ([`ServiceDef`](zerodds_rpc::service_mapping::ServiceDef) /
//! [`MethodDef`](zerodds_rpc::service_mapping::MethodDef) /
//! [`ParamDef`](zerodds_rpc::service_mapping::ParamDef)) and emits **real
//! Rust code** (not a placeholder marker) that wraps the generic
//! `zerodds_rpc::requester::Requester<TIn, TOut>` /
//! `zerodds_rpc::replier::Replier<TIn, TOut>` runtime.
//!
//! # What is emitted, per service
//!
//! Following the DDS-RPC **Enhanced** service mapping (spec §7.5.1.2 — one
//! request/reply topic pair per operation), for a service `S` with operations
//! `op`:
//!
//! * `S_op_Request` — a `#[derive(DdsType)]`-equivalent struct with the `in`
//!   and `inout` parameters (request payload `TIn`).
//! * `S_op_Reply` — struct with the return value (`return_value`) plus the
//!   `out` and `inout` parameters (reply payload `TOut`). Empty for a
//!   `oneway` operation.
//! * `SRequester` — client wrapper. Holds one
//!   `zerodds_rpc::requester::Requester<S_op_Request, S_op_Reply>` per
//!   operation (public field `op_endpoint`, so callers can drive `tick()` /
//!   inspect the DDS endpoint) plus a typed method per operation:
//!   `fn op(&self, <in/inout>, timeout) -> RpcResult<S_op_Reply>`
//!   (blocking send/receive), or `fn op(&self, <in>) -> RpcResult<()>` for a
//!   `oneway`.
//! * `SHandler` — server-side handler trait the application implements
//!   (`fn op(&self, S_op_Request) -> Result<S_op_Reply, RemoteExceptionCode>`;
//!   `fn op(&self, S_op_Request)` for `oneway`).
//! * `SReplier` — server wrapper. Holds one
//!   `zerodds_rpc::replier::Replier<S_op_Request, S_op_Reply>` per operation,
//!   each wired to the handler through a generated per-operation
//!   `ReplierHandler` adapter, plus a `tick()` that services every operation.
//!
//! The request/reply structs are emitted through the ordinary
//! [`struct_emit`](crate::struct_emit) path, so they get the full
//! `zerodds_dcps::DdsType` + `zerodds_cdr::CdrEncode`/`CdrDecode` treatment
//! and travel over the wire exactly like any other IDL type.
//!
//! # Scope
//!
//! Delivered: the core typed request/reply round-trip (value return, `in`,
//! `out`, `inout`, `oneway`). **Not** yet mapped: IDL `raises` / exception
//! payloads beyond the transport-level `RemoteExceptionCode` (a handler
//! signals failure by returning `Err(RemoteExceptionCode)`; a typed exception
//! union per operation is a larger, separate piece of work). Attributes
//! (`attribute T x;`) are not represented in `zerodds_rpc::ServiceDef` and are
//! therefore not emitted.
//!
//! In `cdr_only` mode (the CORBA/GIOP path) this module is **not** invoked —
//! that path deliberately excludes the `zerodds_dcps`/`zerodds_rpc` runtime and
//! generates interface stubs via `zerodds-corba-rust` instead.

use zerodds_idl::ast::{Declarator, Identifier, InterfaceDef, Member, StructDef, TypeSpec};
use zerodds_idl::errors::Span;
use zerodds_rpc::annotations::lower_rpc_annotations;
use zerodds_rpc::service_mapping::{MethodDef, ServiceDef, lower_service};

use crate::error::{Result, RustGenError};
use crate::type_map::{escape_keyword, rust_type_for};

/// Reply-struct field name carrying the operation return value.
const RETURN_FIELD: &str = "return_value";

/// Emits the full native DDS-RPC surface (request/reply structs + typed
/// `Requester`/`Replier` wrappers + handler trait) for an interface that
/// carries operations.
///
/// `module_path` is the enclosing IDL module scope (for `TYPE_NAME` and
/// `TYPE_IDENTIFIER` member resolution inside the emitted request/reply
/// structs).
///
/// # Errors
/// * [`RustGenError::Unsupported`] if the service model cannot be lowered
///   (e.g. a duplicate operation name — normally already rejected by the
///   parser) or a parameter/return type is outside the Rust codegen's type
///   mapping.
pub fn emit_service_rpc(
    out: &mut String,
    iface: &InterfaceDef,
    module_path: &[String],
) -> Result<()> {
    let lowered = lower_rpc_annotations(&iface.annotations);
    let svc = lower_service(iface, &lowered).map_err(|_| RustGenError::Unsupported {
        what: "DDS-RPC service definition",
        at: iface.span.start,
    })?;

    out.push_str(&format!(
        "// zerodds-rpc-1.0 — native Rust DDS-RPC codegen for service `{}` \
         (Enhanced mapping, one topic pair per operation).\n",
        svc.name
    ));

    // 1. Per-operation request + reply wire structs (real DdsType).
    for m in &svc.methods {
        crate::type_identifier::set_current_scope(module_path);
        let req = request_struct(&svc, m)?;
        crate::struct_emit::emit_struct(out, &req, module_path)?;
        out.push('\n');

        crate::type_identifier::set_current_scope(module_path);
        let reply = reply_struct(&svc, m)?;
        crate::struct_emit::emit_struct(out, &reply, module_path)?;
        out.push('\n');
    }

    // 2. Client wrapper.
    emit_requester(out, &svc)?;
    // 3. Server-side handler trait.
    emit_handler_trait(out, &svc)?;
    // 4. Server wrapper + per-operation dispatch adapters.
    emit_replier(out, &svc)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Naming helpers
// ---------------------------------------------------------------------------

fn request_type_name(svc: &ServiceDef, m: &MethodDef) -> String {
    format!("{}_{}_Request", svc.name, m.name)
}

fn reply_type_name(svc: &ServiceDef, m: &MethodDef) -> String {
    format!("{}_{}_Reply", svc.name, m.name)
}

fn dispatch_type_name(svc: &ServiceDef, m: &MethodDef) -> String {
    format!("{}_{}_Dispatch", svc.name, m.name)
}

/// Public endpoint field name on the wrapper structs (`op_endpoint`).
fn endpoint_field(m: &MethodDef) -> String {
    escape_keyword(&format!("{}_endpoint", m.name))
}

/// Per-operation service name handed to the runtime — the runtime derives the
/// `<name>_Request`/`<name>_Reply` topics from it, which yields the Enhanced
/// per-operation topic names `S_op_Request`/`S_op_Reply` (spec §7.5.1.2).
fn op_service_name(svc: &ServiceDef, m: &MethodDef) -> String {
    format!("{}_{}", svc.name, m.name)
}

// ---------------------------------------------------------------------------
// Request / reply struct synthesis
// ---------------------------------------------------------------------------

fn simple_member(name: &str, ts: &TypeSpec) -> Member {
    Member {
        type_spec: ts.clone(),
        declarators: vec![Declarator::Simple(Identifier::new(name, Span::SYNTHETIC))],
        annotations: Vec::new(),
        span: Span::SYNTHETIC,
    }
}

fn struct_def(name: String, members: Vec<Member>) -> StructDef {
    StructDef {
        name: Identifier::new(name, Span::SYNTHETIC),
        base: None,
        members,
        annotations: Vec::new(),
        span: Span::SYNTHETIC,
    }
}

/// Request struct: `in` + `inout` parameters, in declaration order.
fn request_struct(svc: &ServiceDef, m: &MethodDef) -> Result<StructDef> {
    let members = m
        .params
        .iter()
        .filter(|p| p.direction.is_in())
        .map(|p| simple_member(&p.name, &p.type_ref))
        .collect();
    Ok(struct_def(request_type_name(svc, m), members))
}

/// Reply struct: `return_value` (if the operation returns a value) followed by
/// the `out` + `inout` parameters, in declaration order. Empty for `oneway`.
fn reply_struct(svc: &ServiceDef, m: &MethodDef) -> Result<StructDef> {
    let mut members = Vec::new();
    if let Some(ret) = &m.return_type {
        members.push(simple_member(RETURN_FIELD, ret));
    }
    for p in m.params.iter().filter(|p| p.direction.is_out()) {
        members.push(simple_member(&p.name, &p.type_ref));
    }
    Ok(struct_def(reply_type_name(svc, m), members))
}

// ---------------------------------------------------------------------------
// Requester (client) wrapper
// ---------------------------------------------------------------------------

fn emit_requester(out: &mut String, svc: &ServiceDef) -> Result<()> {
    let name = format!("{}Requester", svc.name);
    out.push_str(&format!(
        "/// Typed DDS-RPC client for service `{}` (generated).\n",
        svc.name
    ));
    out.push_str(&format!("pub struct {name} {{\n"));
    for m in &svc.methods {
        out.push_str(&format!(
            "    /// DDS-RPC endpoint for operation `{}`.\n",
            m.name
        ));
        out.push_str(&format!(
            "    pub {}: zerodds_rpc::requester::Requester<{}, {}>,\n",
            endpoint_field(m),
            request_type_name(svc, m),
            reply_type_name(svc, m),
        ));
    }
    out.push_str("}\n\n");

    out.push_str(&format!("impl {name} {{\n"));
    // Constructor.
    out.push_str(
        "    /// Creates the client and its per-operation DDS endpoints on `participant`.\n",
    );
    out.push_str("    ///\n");
    out.push_str("    /// # Errors\n");
    out.push_str("    /// Propagates `zerodds_rpc::RpcError` from endpoint creation.\n");
    out.push_str(
        "    pub fn new(participant: &zerodds_dcps::participant::DomainParticipant, qos: &zerodds_rpc::qos_profile::RpcQos) -> zerodds_rpc::RpcResult<Self> {\n",
    );
    out.push_str("        ::core::result::Result::Ok(Self {\n");
    for m in &svc.methods {
        out.push_str(&format!(
            "            {}: zerodds_rpc::requester::Requester::new(participant, \"{}\", qos)?,\n",
            endpoint_field(m),
            op_service_name(svc, m),
        ));
    }
    out.push_str("        })\n");
    out.push_str("    }\n");

    // Typed methods.
    for m in &svc.methods {
        emit_requester_method(out, svc, m)?;
    }
    out.push_str("}\n\n");
    Ok(())
}

fn emit_requester_method(out: &mut String, svc: &ServiceDef, m: &MethodDef) -> Result<()> {
    let method = escape_keyword(&m.name);
    let field = endpoint_field(m);
    let req_ty = request_type_name(svc, m);

    // Parameter list (in/inout for request; for oneway there are only `in`).
    let mut params = Vec::new();
    let mut request_fields = Vec::new();
    for p in m.params.iter().filter(|p| p.direction.is_in()) {
        let pname = escape_keyword(&p.name);
        let pty = rust_type_for(&p.type_ref)?;
        params.push(format!("{pname}: {pty}"));
        request_fields.push(pname);
    }
    let request_literal = build_struct_literal(&req_ty, &request_fields);

    out.push_str(&format!("    /// Invokes operation `{}`.\n", m.name));
    if m.oneway {
        out.push_str("    ///\n    /// Oneway — fire-and-forget, no reply (spec §7.5.1).\n");
        out.push_str("    ///\n    /// # Errors\n");
        out.push_str("    /// Propagates `zerodds_rpc::RpcError` from the send.\n");
        out.push_str("    #[allow(clippy::too_many_arguments)]\n");
        out.push_str(&format!(
            "    pub fn {method}(&self, {}) -> zerodds_rpc::RpcResult<()> {{\n",
            params.join(", ")
        ));
        out.push_str(&format!("        let __request = {request_literal};\n"));
        out.push_str(&format!("        self.{field}.send_oneway(&__request)?;\n"));
        out.push_str("        ::core::result::Result::Ok(())\n");
        out.push_str("    }\n");
    } else {
        let reply_ty = reply_type_name(svc, m);
        out.push_str("    ///\n    /// Blocks until the reply arrives or `timeout` elapses.\n");
        out.push_str("    ///\n    /// # Errors\n");
        out.push_str(
            "    /// `zerodds_rpc::RpcError::Timeout` on deadline, `RemoteException` on a \
             server-side failure, or a transport error.\n",
        );
        out.push_str("    #[allow(clippy::too_many_arguments)]\n");
        let mut sig_params = params.clone();
        sig_params.push("timeout: ::core::option::Option<::core::time::Duration>".to_string());
        out.push_str(&format!(
            "    pub fn {method}(&self, {}) -> zerodds_rpc::RpcResult<{reply_ty}> {{\n",
            sig_params.join(", ")
        ));
        out.push_str(&format!("        let __request = {request_literal};\n"));
        out.push_str(&format!(
            "        self.{field}.send_request_blocking(&__request, timeout)\n"
        ));
        out.push_str("    }\n");
    }
    Ok(())
}

/// Builds a struct literal `Ty { a, b }` (field-init shorthand) or `Ty {}` when
/// there are no fields.
fn build_struct_literal(ty: &str, fields: &[String]) -> String {
    if fields.is_empty() {
        format!("{ty} {{}}")
    } else {
        format!("{ty} {{ {} }}", fields.join(", "))
    }
}

// ---------------------------------------------------------------------------
// Handler trait
// ---------------------------------------------------------------------------

fn emit_handler_trait(out: &mut String, svc: &ServiceDef) -> Result<()> {
    let name = format!("{}Handler", svc.name);
    out.push_str(&format!(
        "/// Server-side handler for service `{}`. Implement this and hand it to \
         `{}Replier::new`.\n",
        svc.name, svc.name
    ));
    out.push_str(&format!(
        "pub trait {name}: ::core::marker::Send + ::core::marker::Sync {{\n"
    ));
    for m in &svc.methods {
        let method = escape_keyword(&m.name);
        let req_ty = request_type_name(svc, m);
        out.push_str(&format!("    /// Handles operation `{}`.\n", m.name));
        if m.oneway {
            out.push_str(&format!("    fn {method}(&self, request: {req_ty});\n"));
        } else {
            let reply_ty = reply_type_name(svc, m);
            out.push_str(&format!(
                "    fn {method}(&self, request: {req_ty}) -> ::core::result::Result<{reply_ty}, zerodds_rpc::common_types::RemoteExceptionCode>;\n"
            ));
        }
    }
    out.push_str("}\n\n");
    Ok(())
}

// ---------------------------------------------------------------------------
// Replier (server) wrapper + per-operation dispatch adapters
// ---------------------------------------------------------------------------

fn emit_replier(out: &mut String, svc: &ServiceDef) -> Result<()> {
    let handler_trait = format!("{}Handler", svc.name);

    // Per-operation ReplierHandler adapters.
    for m in &svc.methods {
        let dispatch = dispatch_type_name(svc, m);
        let req_ty = request_type_name(svc, m);
        let reply_ty = reply_type_name(svc, m);
        let method = escape_keyword(&m.name);
        out.push_str(&format!(
            "/// Per-operation dispatch adapter binding `{}` to the handler.\n",
            m.name
        ));
        out.push_str(&format!(
            "struct {dispatch}(::std::sync::Arc<dyn {handler_trait}>);\n"
        ));
        out.push_str(&format!(
            "impl zerodds_rpc::replier::ReplierHandler<{req_ty}, {reply_ty}> for {dispatch} {{\n"
        ));
        out.push_str(&format!(
            "    fn handle(&self, request: {req_ty}) -> ::core::result::Result<{reply_ty}, zerodds_rpc::common_types::RemoteExceptionCode> {{\n"
        ));
        if m.oneway {
            // Oneway: invoke the handler, then hand back an empty reply. The
            // caller used `send_oneway` and never reads it (spec §7.5.1).
            out.push_str(&format!("        self.0.{method}(request);\n"));
            out.push_str(
                "        ::core::result::Result::Ok(::core::default::Default::default())\n",
            );
        } else {
            out.push_str(&format!("        self.0.{method}(request)\n"));
        }
        out.push_str("    }\n}\n\n");
    }

    let name = format!("{}Replier", svc.name);
    out.push_str(&format!(
        "/// Typed DDS-RPC server for service `{}` (generated).\n",
        svc.name
    ));
    out.push_str(&format!("pub struct {name} {{\n"));
    for m in &svc.methods {
        out.push_str(&format!(
            "    /// DDS-RPC endpoint for operation `{}`.\n",
            m.name
        ));
        out.push_str(&format!(
            "    pub {}: zerodds_rpc::replier::Replier<{}, {}>,\n",
            endpoint_field(m),
            request_type_name(svc, m),
            reply_type_name(svc, m),
        ));
    }
    out.push_str("}\n\n");

    out.push_str(&format!("impl {name} {{\n"));
    out.push_str(
        "    /// Creates the server and its per-operation DDS endpoints, all wired to `handler`.\n",
    );
    out.push_str("    ///\n    /// # Errors\n");
    out.push_str("    /// Propagates `zerodds_rpc::RpcError` from endpoint creation.\n");
    out.push_str(&format!(
        "    pub fn new(participant: &zerodds_dcps::participant::DomainParticipant, qos: &zerodds_rpc::qos_profile::RpcQos, handler: ::std::sync::Arc<dyn {handler_trait}>) -> zerodds_rpc::RpcResult<Self> {{\n"
    ));
    out.push_str("        ::core::result::Result::Ok(Self {\n");
    for m in &svc.methods {
        let dispatch = dispatch_type_name(svc, m);
        out.push_str(&format!(
            "            {}: zerodds_rpc::replier::Replier::new(participant, \"{}\", qos, ::std::sync::Arc::new({dispatch}(::std::sync::Arc::clone(&handler))))?,\n",
            endpoint_field(m),
            op_service_name(svc, m),
        ));
    }
    out.push_str("        })\n");
    out.push_str("    }\n");

    // tick(): service every operation's endpoint once, return total handled.
    out.push_str(
        "    /// Processes pending requests across all operations. Returns the number handled.\n",
    );
    out.push_str("    pub fn tick(&self) -> usize {\n");
    out.push_str("        let mut __handled = 0usize;\n");
    for m in &svc.methods {
        out.push_str(&format!(
            "        __handled += self.{}.tick();\n",
            endpoint_field(m)
        ));
    }
    out.push_str("        __handled\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
    Ok(())
}
