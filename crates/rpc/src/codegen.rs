// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Request/reply codegen data model — Spec §7.5.1.
//!
//! This stage (C6.1.B) derives from a [`ServiceDef`] the wire data models
//! of the request and reply topics. The spec distinguishes two layouts:
//!
//! * **Basic service** (Spec §7.5.1.1): _one_ request topic + _one_ reply
//!   topic per service type, both with an untyped discriminated union
//!   over all methods. Wire form:
//!
//!   ```text
//!   struct <Service>_Request {
//!       RequestHeader header;
//!       union <Service>_Call switch (long _d) {
//!           case OP_<m1>: <Service>_<m1>_In   m1_args;
//!           case OP_<m2>: <Service>_<m2>_In   m2_args;
//!           ...
//!       };
//!   };
//!   ```
//!
//!   `<Service>_Reply` analogously with `<Service>_<m>_Out` and `ReplyHeader`.
//!
//! * **Enhanced service** (Spec §7.5.1.2): _per method_ a dedicated
//!   request and reply topic with typed per-method structures.
//!   Wire form (per method):
//!
//!   ```text
//!   struct <Service>_<m>_In  { /* in/inout params */ };
//!   struct <Service>_<m>_Out { /* return + out/inout */ };
//!   ```
//!
//! We produce **data structures** here ([`RequestType`], [`ReplyType`])
//! with member lists — no language codegen. The language backends in
//! `crates/idl-cpp`, `idl-csharp` and `idl-java` consume the model
//! and emit bindings.
//!
//! Oneway methods:
//!
//! * In the basic layout, a oneway method appears in the request union,
//!   but is **not** included in the reply union — the reply topic
//!   is nonetheless shared.
//! * In the enhanced layout, [`build_enhanced_pair`] returns for a
//!   oneway method `Some(RequestType)` and `None` as the reply (see
//!   [`MethodPair`]).

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::{RpcError, RpcResult};
use crate::service_mapping::{MethodDef, ParamDirection, ServiceDef, TypeRef};

/// Layout variant of the service wire model (Spec §7.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceLayout {
    /// Spec §7.5.1.1 — one request/reply topic pair per service.
    Basic,
    /// Spec §7.5.1.2 — one topic pair per method.
    Enhanced,
}

/// Member of a generated wire structure.
#[derive(Debug, Clone, PartialEq)]
pub struct StructMember {
    /// Member name (e.g. `header`, `request_id`, `result`).
    pub name: String,
    /// Type reference or synthetic token (e.g. `RequestHeader`).
    pub type_ref: MemberType,
}

impl StructMember {
    /// Constructor.
    #[must_use]
    pub fn new(name: impl Into<String>, type_ref: MemberType) -> Self {
        Self {
            name: name.into(),
            type_ref,
        }
    }
}

/// Type of a struct member. Either a known RPC header type
/// (`RequestHeader`/`ReplyHeader`/`<Service>_Call`) or an IDL type
/// from `zerodds_idl::ast::TypeSpec`.
#[derive(Debug, Clone, PartialEq)]
pub enum MemberType {
    /// `RequestHeader` aus `common_types.rs`.
    RequestHeader,
    /// `ReplyHeader` aus `common_types.rs`.
    ReplyHeader,
    /// Synthetic union over all methods of a service. Carrier
    /// for the basic-layout switch.
    CallUnion(CallUnionDef),
    /// A normal IDL type.
    Idl(TypeRef),
}

/// Discriminator case of a call union (basic layout).
#[derive(Debug, Clone, PartialEq)]
pub struct CallUnionCase {
    /// Method name (= discriminator label `OP_<name>`).
    pub method: String,
    /// Discriminator value (1-based in the order of the `MethodDef` list).
    pub discriminator: u32,
    /// `<Service>_<method>_<In|Out>` type name.
    pub case_type_name: String,
    /// Member list of the case struct (`In` or `Out`).
    pub members: Vec<StructMember>,
}

/// `<Service>_Call` (request) or `<Service>_Result` (reply) union.
#[derive(Debug, Clone, PartialEq)]
pub struct CallUnionDef {
    /// Complete union type name (e.g. `Calculator_Call`).
    pub name: String,
    /// Cases. The `Reply` union contains only methods that are not
    /// `oneway`.
    pub cases: Vec<CallUnionCase>,
}

/// Request wire data model.
///
/// * Basic layout: one RequestType per service, with `header` +
///   `union` member over all methods.
/// * Enhanced layout: one RequestType per method, with `header` +
///   typed per-method fields.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestType {
    /// Complete struct name (e.g. `Calculator_Request` or
    /// `Calculator_add_Request`).
    pub name: String,
    /// Topic name of this request wire structure.
    pub topic_name: String,
    /// Layout variant from which the struct was produced.
    pub layout: ServiceLayout,
    /// Method name for enhanced; `None` for basic.
    pub method: Option<String>,
    /// Member list (header + body).
    pub members: Vec<StructMember>,
}

/// Reply wire data model. Analogous to [`RequestType`].
#[derive(Debug, Clone, PartialEq)]
pub struct ReplyType {
    /// Complete struct name.
    pub name: String,
    /// Topic name.
    pub topic_name: String,
    /// Layout variant.
    pub layout: ServiceLayout,
    /// Method name for enhanced; `None` for basic.
    pub method: Option<String>,
    /// Member list.
    pub members: Vec<StructMember>,
}

/// Pair for an enhanced method. Reply is `None` for `oneway`.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodPair {
    /// Request wire structure of this method.
    pub request: RequestType,
    /// Reply wire structure (`None` for `oneway` methods).
    pub reply: Option<ReplyType>,
}

// ---------------------------------------------------------------------
// Basic service mapping (Spec §7.5.1.1)
// ---------------------------------------------------------------------

/// Returns the data models of the basic-layout topics
/// (`<Service>_Request` plus `<Service>_Reply`). An empty `ServiceDef`
/// returns a pair with empty unions — the caller must decide for itself
/// whether to discard it.
///
/// # Errors
/// `RpcError::InvalidServiceName` if the service name is empty (this
/// happens only for manually constructed ServiceDefs — `lower_service`
/// already validates).
pub fn build_basic_pair(svc: &ServiceDef) -> RpcResult<(RequestType, ReplyType)> {
    if svc.name.is_empty() {
        return Err(RpcError::InvalidServiceName(String::new()));
    }
    let topics = svc.topic_names()?;
    let req_union = build_call_union(svc, /*include_oneway=*/ true, /*reply=*/ false)?;
    let rep_union = build_call_union(svc, /*include_oneway=*/ false, /*reply=*/ true)?;

    let request = RequestType {
        name: format!("{}_Request", svc.name),
        topic_name: topics.request.clone(),
        layout: ServiceLayout::Basic,
        method: None,
        members: alloc::vec![
            StructMember::new("header", MemberType::RequestHeader),
            StructMember::new("call", MemberType::CallUnion(req_union)),
        ],
    };

    let reply = ReplyType {
        name: format!("{}_Reply", svc.name),
        topic_name: topics.reply,
        layout: ServiceLayout::Basic,
        method: None,
        members: alloc::vec![
            StructMember::new("header", MemberType::ReplyHeader),
            StructMember::new("result", MemberType::CallUnion(rep_union)),
        ],
    };

    Ok((request, reply))
}

fn build_call_union(
    svc: &ServiceDef,
    include_oneway: bool,
    reply: bool,
) -> RpcResult<CallUnionDef> {
    let union_name = if reply {
        format!("{}_Result", svc.name)
    } else {
        format!("{}_Call", svc.name)
    };
    let mut cases = Vec::with_capacity(svc.methods.len());
    let mut discr: u32 = 1;
    for m in &svc.methods {
        if m.oneway && !include_oneway {
            continue;
        }
        let case_type = method_struct_name(svc, m, reply);
        let members = if reply {
            method_out_members(m)
        } else {
            method_in_members(m)
        };
        cases.push(CallUnionCase {
            method: m.name.clone(),
            discriminator: discr,
            case_type_name: case_type,
            members,
        });
        discr = discr
            .checked_add(1)
            .ok_or_else(|| RpcError::Codec("more than u32::MAX methods in service".to_string()))?;
    }
    Ok(CallUnionDef {
        name: union_name,
        cases,
    })
}

// ---------------------------------------------------------------------
// Enhanced-Service-Mapping (Spec §7.5.1.2)
// ---------------------------------------------------------------------

/// Returns the per-method pair (request + optional reply).
///
/// Reply is `None` if the method is `oneway`.
///
/// # Errors
/// `RpcError::InvalidServiceName` resp. `RpcError::InvalidMethodName`
/// on inconsistent ServiceDefs.
pub fn build_enhanced_pair(svc: &ServiceDef, method: &MethodDef) -> RpcResult<MethodPair> {
    if svc.name.is_empty() {
        return Err(RpcError::InvalidServiceName(String::new()));
    }
    if method.name.is_empty() {
        return Err(RpcError::InvalidMethodName(String::new()));
    }
    // Enhanced-Topic-Naming: `<Service>_<Method>_Request` /
    // `<Service>_<Method>_Reply` (Spec §7.5.1.2 + §7.8.2 allows vendor-
    // extension; Cyclone/FastDDS use this form).
    let request_topic = format!(
        "{}_{}{}",
        svc.name,
        method.name,
        crate::topic_naming::REQUEST_SUFFIX
    );
    let reply_topic = format!(
        "{}_{}{}",
        svc.name,
        method.name,
        crate::topic_naming::REPLY_SUFFIX
    );

    let mut req_members = alloc::vec![StructMember::new("header", MemberType::RequestHeader)];
    req_members.extend(method_in_members(method));
    let request = RequestType {
        name: format!("{}_{}_Request", svc.name, method.name),
        topic_name: request_topic,
        layout: ServiceLayout::Enhanced,
        method: Some(method.name.clone()),
        members: req_members,
    };

    let reply = if method.oneway {
        None
    } else {
        let mut rep_members = alloc::vec![StructMember::new("header", MemberType::ReplyHeader)];
        rep_members.extend(method_out_members(method));
        Some(ReplyType {
            name: format!("{}_{}_Reply", svc.name, method.name),
            topic_name: reply_topic,
            layout: ServiceLayout::Enhanced,
            method: Some(method.name.clone()),
            members: rep_members,
        })
    };

    Ok(MethodPair { request, reply })
}

/// Returns all enhanced-layout pairs of a service.
///
/// # Errors
/// See [`build_enhanced_pair`].
pub fn build_enhanced_all(svc: &ServiceDef) -> RpcResult<Vec<MethodPair>> {
    let mut out = Vec::with_capacity(svc.methods.len());
    for m in &svc.methods {
        out.push(build_enhanced_pair(svc, m)?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn method_struct_name(svc: &ServiceDef, m: &MethodDef, reply: bool) -> String {
    if reply {
        format!("{}_{}_Out", svc.name, m.name)
    } else {
        format!("{}_{}_In", svc.name, m.name)
    }
}

fn method_in_members(m: &MethodDef) -> Vec<StructMember> {
    m.params
        .iter()
        .filter(|p| p.direction.is_in())
        .map(|p| StructMember::new(p.name.clone(), MemberType::Idl(p.type_ref.clone())))
        .collect()
}

fn method_out_members(m: &MethodDef) -> Vec<StructMember> {
    let mut out = Vec::new();
    if let Some(ret) = &m.return_type {
        out.push(StructMember::new("_return", MemberType::Idl(ret.clone())));
    }
    for p in m.params.iter().filter(|p| p.direction.is_out()) {
        // `out` and `inout` both land in the reply.
        let _ = ParamDirection::Out;
        out.push(StructMember::new(
            p.name.clone(),
            MemberType::Idl(p.type_ref.clone()),
        ));
    }
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;
    use crate::annotations::lower_rpc_annotations;
    use crate::service_mapping::{ParamDef, lower_service};
    use zerodds_idl::ast::{
        Annotation, AnnotationParams, Export, Identifier, IntegerType, InterfaceDef, InterfaceKind,
        OpDecl, ParamAttribute, ParamDecl, PrimitiveType, ScopedName, StringType, TypeSpec,
    };
    use zerodds_idl::errors::Span;

    fn sp() -> Span {
        Span::SYNTHETIC
    }

    fn ident(t: &str) -> Identifier {
        Identifier::new(t, sp())
    }

    fn long_t() -> TypeSpec {
        TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long))
    }

    fn string_t() -> TypeSpec {
        TypeSpec::String(StringType {
            wide: false,
            bound: None,
            span: sp(),
        })
    }

    fn op(name: &str, oneway: bool, ret: Option<TypeSpec>, params: Vec<ParamDecl>) -> OpDecl {
        OpDecl {
            name: ident(name),
            oneway,
            return_type: ret,
            params,
            raises: Vec::new(),
            context: Vec::new(),
            annotations: Vec::new(),
            span: sp(),
        }
    }

    fn param(name: &str, attr: ParamAttribute, ty: TypeSpec) -> ParamDecl {
        ParamDecl {
            attribute: attr,
            type_spec: ty,
            name: ident(name),
            annotations: Vec::new(),
            span: sp(),
        }
    }

    fn ann_simple(name: &str) -> Annotation {
        Annotation {
            name: ScopedName {
                absolute: false,
                parts: alloc::vec![ident(name)],
                span: sp(),
            },
            params: AnnotationParams::None,
            span: sp(),
        }
    }

    fn calc_service() -> ServiceDef {
        let add = op(
            "add",
            false,
            Some(long_t()),
            alloc::vec![
                param("a", ParamAttribute::In, long_t()),
                param("b", ParamAttribute::In, long_t()),
            ],
        );
        let log = op(
            "log",
            true, // oneway
            None,
            alloc::vec![param("msg", ParamAttribute::In, string_t())],
        );
        let i = InterfaceDef {
            kind: InterfaceKind::Plain,
            name: ident("Calculator"),
            bases: Vec::new(),
            exports: alloc::vec![Export::Op(add), Export::Op(log)],
            annotations: alloc::vec![ann_simple("service")],
            span: sp(),
        };
        let lowered = lower_rpc_annotations(&i.annotations);
        lower_service(&i, &lowered).unwrap()
    }

    #[test]
    fn basic_pair_topic_names() {
        let svc = calc_service();
        let (req, rep) = build_basic_pair(&svc).unwrap();
        assert_eq!(req.topic_name, "Calculator_Request");
        assert_eq!(rep.topic_name, "Calculator_Reply");
    }

    #[test]
    fn basic_pair_layout_marker() {
        let svc = calc_service();
        let (req, rep) = build_basic_pair(&svc).unwrap();
        assert_eq!(req.layout, ServiceLayout::Basic);
        assert_eq!(rep.layout, ServiceLayout::Basic);
        assert_eq!(req.method, None);
        assert_eq!(rep.method, None);
    }

    #[test]
    fn basic_request_has_header_and_call_union() {
        let svc = calc_service();
        let (req, _) = build_basic_pair(&svc).unwrap();
        assert_eq!(req.members.len(), 2);
        assert_eq!(req.members[0].name, "header");
        assert!(matches!(req.members[0].type_ref, MemberType::RequestHeader));
        assert_eq!(req.members[1].name, "call");
        let call_union = match &req.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => panic!("expected CallUnion"),
        };
        assert_eq!(call_union.name, "Calculator_Call");
        // Both methods including oneway `log` must be in the request union.
        assert_eq!(call_union.cases.len(), 2);
        assert_eq!(call_union.cases[0].method, "add");
        assert_eq!(call_union.cases[0].discriminator, 1);
        assert_eq!(call_union.cases[0].case_type_name, "Calculator_add_In");
        assert_eq!(call_union.cases[1].method, "log");
        assert_eq!(call_union.cases[1].discriminator, 2);
    }

    #[test]
    fn basic_reply_excludes_oneway_methods() {
        let svc = calc_service();
        let (_, rep) = build_basic_pair(&svc).unwrap();
        let result_union = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => panic!("expected CallUnion"),
        };
        assert_eq!(result_union.name, "Calculator_Result");
        // `log` is oneway → not in the reply.
        assert_eq!(result_union.cases.len(), 1);
        assert_eq!(result_union.cases[0].method, "add");
        assert_eq!(result_union.cases[0].case_type_name, "Calculator_add_Out");
    }

    #[test]
    fn basic_request_in_params_become_case_members() {
        let svc = calc_service();
        let (req, _) = build_basic_pair(&svc).unwrap();
        let call_union = match &req.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let add_case = &call_union.cases[0];
        assert_eq!(add_case.members.len(), 2);
        assert_eq!(add_case.members[0].name, "a");
        assert_eq!(add_case.members[1].name, "b");
    }

    #[test]
    fn basic_reply_return_value_first_member() {
        let svc = calc_service();
        let (_, rep) = build_basic_pair(&svc).unwrap();
        let result_union = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let add_case = &result_union.cases[0];
        assert_eq!(add_case.members.len(), 1);
        assert_eq!(add_case.members[0].name, "_return");
    }

    #[test]
    fn enhanced_pair_topic_names() {
        let svc = calc_service();
        let pair = build_enhanced_pair(&svc, &svc.methods[0]).unwrap();
        assert_eq!(pair.request.topic_name, "Calculator_add_Request");
        assert_eq!(
            pair.reply.as_ref().unwrap().topic_name,
            "Calculator_add_Reply"
        );
    }

    #[test]
    fn enhanced_pair_layout_marker() {
        let svc = calc_service();
        let pair = build_enhanced_pair(&svc, &svc.methods[0]).unwrap();
        assert_eq!(pair.request.layout, ServiceLayout::Enhanced);
        assert_eq!(pair.request.method, Some("add".to_string()));
    }

    #[test]
    fn enhanced_oneway_has_no_reply() {
        let svc = calc_service();
        let log = svc.methods.iter().find(|m| m.oneway).unwrap();
        let pair = build_enhanced_pair(&svc, log).unwrap();
        assert!(pair.reply.is_none());
        // Request members: header + msg.
        assert_eq!(pair.request.members.len(), 2);
        assert_eq!(pair.request.members[0].name, "header");
        assert_eq!(pair.request.members[1].name, "msg");
    }

    #[test]
    fn enhanced_pair_request_in_params() {
        let svc = calc_service();
        let pair = build_enhanced_pair(&svc, &svc.methods[0]).unwrap();
        // header + a + b
        assert_eq!(pair.request.members.len(), 3);
        assert_eq!(pair.request.members[0].name, "header");
        assert_eq!(pair.request.members[1].name, "a");
        assert_eq!(pair.request.members[2].name, "b");
    }

    #[test]
    fn enhanced_pair_reply_return_only() {
        let svc = calc_service();
        let pair = build_enhanced_pair(&svc, &svc.methods[0]).unwrap();
        let rep = pair.reply.as_ref().unwrap();
        // header + _return
        assert_eq!(rep.members.len(), 2);
        assert_eq!(rep.members[0].name, "header");
        assert_eq!(rep.members[1].name, "_return");
    }

    #[test]
    fn enhanced_inout_param_appears_in_both_request_and_reply() {
        let m = op(
            "swap",
            false,
            None,
            alloc::vec![param("v", ParamAttribute::InOut, long_t())],
        );
        let svc = ServiceDef {
            name: "Swap".into(),
            methods: alloc::vec![MethodDef {
                name: "swap".into(),
                params: alloc::vec![ParamDef {
                    name: "v".into(),
                    direction: ParamDirection::InOut,
                    type_ref: long_t(),
                }],
                return_type: None,
                oneway: false,
            }],
        };
        let _ = m;
        let pair = build_enhanced_pair(&svc, &svc.methods[0]).unwrap();
        assert!(pair.request.members.iter().any(|m| m.name == "v"));
        let rep = pair.reply.as_ref().unwrap();
        assert!(rep.members.iter().any(|m| m.name == "v"));
    }

    #[test]
    fn enhanced_all_skips_no_method() {
        let svc = calc_service();
        let pairs = build_enhanced_all(&svc).unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].request.method, Some("add".to_string()));
        assert_eq!(pairs[1].request.method, Some("log".to_string()));
        assert!(pairs[1].reply.is_none()); // log is oneway.
    }

    #[test]
    fn empty_service_yields_empty_unions_in_basic() {
        let svc = ServiceDef {
            name: "Empty".into(),
            methods: Vec::new(),
        };
        let (req, rep) = build_basic_pair(&svc).unwrap();
        let req_u = match &req.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let rep_u = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        assert_eq!(req_u.cases.len(), 0);
        assert_eq!(rep_u.cases.len(), 0);
    }

    #[test]
    fn invalid_service_name_is_error_in_codegen() {
        let svc = ServiceDef {
            name: String::new(),
            methods: Vec::new(),
        };
        let err = build_basic_pair(&svc).unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    #[test]
    fn enhanced_method_with_invalid_name_is_error() {
        let svc = ServiceDef {
            name: "S".into(),
            methods: alloc::vec![MethodDef {
                name: String::new(),
                params: Vec::new(),
                return_type: None,
                oneway: false,
            }],
        };
        let err = build_enhanced_pair(&svc, &svc.methods[0]).unwrap_err();
        assert!(matches!(err, RpcError::InvalidMethodName(_)));
    }
}
