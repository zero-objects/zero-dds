// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL `interface` operations → native Go DDS-RPC wire codegen.
//!
//! Spec reference: OMG DDS-RPC 1.0 (`formal/16-12-04`), **Enhanced** service
//! mapping (§7.5.1.2 — one request/reply topic pair per operation). This module
//! is the Go PSM counterpart of `idl-rust`'s and `idl-cpp`'s `rpc.rs`: it takes
//! the typed service model from `zerodds-rpc`
//! ([`ServiceDef`](zerodds_rpc::service_mapping::ServiceDef) /
//! [`MethodDef`](zerodds_rpc::service_mapping::MethodDef) /
//! [`ParamDef`](zerodds_rpc::service_mapping::ParamDef)) — the **same** lowering
//! `idl-rust` uses, so `@service(name=...)`, the `@oneway` annotation form, and
//! the `@in`/`@out`/`@inout` direction overrides are handled identically — and
//! emits, per operation of a service `S` with operation `op`:
//!
//! * `S_op_Request` — a Go struct + `MarshalXCDR`/`UnmarshalXCDR` carrying the
//!   `in` and `inout` parameters (request payload).
//! * `S_op_Reply` — a Go struct with the return value (`return_value`) followed
//!   by the `out` and `inout` parameters (reply payload). Empty for a `oneway`
//!   operation.
//!
//! The request/reply structs are emitted through the ordinary
//! [`emit_struct`](crate::emitter::emit_struct) path, so their XCDR2 wire bytes
//! are **byte-identical** to the structs `idl-rust`'s `emit_service_rpc`
//! synthesises for the same interface (field composition and order match: `in`
//! + `inout` for the request; `return_value` + `out` + `inout` for the reply).
//!
//! # Scope
//!
//! Delivered: the per-operation request/reply **wire** structs — the part of
//! the RPC surface that travels over DDS and must match the Rust goldens. The
//! typed `<Service>Requester` / `<Service>Replier` client/server wrappers that
//! `idl-rust` emits wrap the `zerodds_rpc` runtime; Go's runtime counterpart
//! lives in `endpoints/go`, which is outside this crate. Emitting wrappers
//! against a non-existent Go runtime would produce code that does not compile,
//! so they are deliberately not generated here. `raises` / exception payloads
//! and `attribute`s are out of scope in `idl-rust` too and are likewise not
//! emitted.

use std::collections::{HashMap, HashSet};

use zerodds_idl::ast::types::{Declarator, Identifier, InterfaceDef, Member, StructDef, TypeSpec};
use zerodds_idl::errors::Span;
use zerodds_rpc::annotations::lower_rpc_annotations;
use zerodds_rpc::service_mapping::{MethodDef, ServiceDef, lower_service};

use crate::emitter::emit_struct;
use crate::error::{IdlGoError, Result};

/// Reply-struct field name carrying the operation return value (matches
/// `idl-rust`'s `RETURN_FIELD`).
const RETURN_FIELD: &str = "return_value";

/// Emits the per-operation request/reply wire structs for an interface that
/// carries operations. `scope` is the enclosing module scope (the flattened Go
/// type name of each struct is `qualify(scope, "<svc>_<op>_Request/Reply")`).
///
/// # Errors
/// * [`IdlGoError::Unsupported`] if the service model cannot be lowered (e.g. a
///   duplicate operation name — normally already rejected by the parser) or a
///   parameter/return type is outside the Go codegen's type mapping.
pub fn emit_service_rpc(
    out: &mut String,
    iface: &InterfaceDef,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    structs: &HashMap<String, &StructDef>,
) -> Result<()> {
    let lowered = lower_rpc_annotations(&iface.annotations);
    let svc = lower_service(iface, &lowered).map_err(|e| {
        IdlGoError::Unsupported(format!(
            "DDS-RPC service definition for interface `{}`: {e}",
            iface.name.text
        ))
    })?;

    let _ = write_header(out, &svc);

    for m in &svc.methods {
        let req = request_struct(&svc, m);
        emit_struct(
            out,
            &req,
            scope,
            enum_names,
            struct_names,
            typedefs,
            structs,
        )?;
        out.push('\n');

        let reply = reply_struct(&svc, m);
        emit_struct(
            out,
            &reply,
            scope,
            enum_names,
            struct_names,
            typedefs,
            structs,
        )?;
        out.push('\n');
    }

    Ok(())
}

fn write_header(out: &mut String, svc: &ServiceDef) -> std::fmt::Result {
    use std::fmt::Write;
    writeln!(
        out,
        "\n// zerodds-rpc-1.0 — DDS-RPC request/reply wire structs for service `{}` \
         (Enhanced mapping, one topic pair per operation). Typed client/server \
         wrappers are provided by the Go runtime, not generated here.",
        svc.name
    )
}

// ---------------------------------------------------------------------------
// Naming helpers (identical to idl-rust's rpc.rs).
// ---------------------------------------------------------------------------

fn request_type_name(svc: &ServiceDef, m: &MethodDef) -> String {
    format!("{}_{}_Request", svc.name, m.name)
}

fn reply_type_name(svc: &ServiceDef, m: &MethodDef) -> String {
    format!("{}_{}_Reply", svc.name, m.name)
}

// ---------------------------------------------------------------------------
// Request / reply struct synthesis (identical composition to idl-rust).
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
fn request_struct(svc: &ServiceDef, m: &MethodDef) -> StructDef {
    let members = m
        .in_params()
        .map(|p| simple_member(&p.name, &p.type_ref))
        .collect();
    struct_def(request_type_name(svc, m), members)
}

/// Reply struct: `return_value` (if the operation returns a value) followed by
/// the `out` + `inout` parameters, in declaration order. Empty for `oneway`.
fn reply_struct(svc: &ServiceDef, m: &MethodDef) -> StructDef {
    let mut members = Vec::new();
    if let Some(ret) = &m.return_type {
        members.push(simple_member(RETURN_FIELD, ret));
    }
    for p in m.out_params() {
        members.push(simple_member(&p.name, &p.type_ref));
    }
    struct_def(reply_type_name(svc, m), members)
}
