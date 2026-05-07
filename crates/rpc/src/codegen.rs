// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Request/Reply-Codegen-Datenmodell — Spec §7.5.1.
//!
//! Diese Stufe (C6.1.B) leitet aus einer [`ServiceDef`] die Wire-Datenmodelle
//! der Request- und Reply-Topics ab. Die Spec unterscheidet zwei Layouts:
//!
//! * **Basic-Service** (Spec §7.5.1.1): _ein_ Request-Topic + _ein_ Reply-
//!   Topic pro Service-Type, beide mit einer ungetypten Discriminated-Union
//!   ueber alle Methoden. Wire-Form:
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
//!   `<Service>_Reply` analog mit `<Service>_<m>_Out` und `ReplyHeader`.
//!
//! * **Enhanced-Service** (Spec §7.5.1.2): _pro Methode_ ein eigenes
//!   Request- und Reply-Topic mit typisierten Pro-Methode-Strukturen.
//!   Wire-Form (pro Methode):
//!
//!   ```text
//!   struct <Service>_<m>_In  { /* in/inout-Params */ };
//!   struct <Service>_<m>_Out { /* return + out/inout */ };
//!   ```
//!
//! Wir erzeugen hier **Daten-Strukturen** ([`RequestType`], [`ReplyType`])
//! mit Member-Listen — kein Sprach-Codegen. Die Sprach-Backends in
//! `crates/idl-cpp`, `idl-csharp` und `idl-java` konsumieren das Modell
//! und emittieren Bindings.
//!
//! Oneway-Methoden:
//!
//! * Im Basic-Layout taucht eine Oneway-Methode im Request-Union auf,
//!   wird aber **nicht** ins Reply-Union aufgenommen — der Reply-Topic
//!   wird trotzdem gemeinsam genutzt.
//! * Im Enhanced-Layout liefert [`build_enhanced_pair`] fuer eine
//!   Oneway-Methode `Some(RequestType)` und `None` als Reply (siehe
//!   [`MethodPair`]).

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::{RpcError, RpcResult};
use crate::service_mapping::{MethodDef, ParamDirection, ServiceDef, TypeRef};

/// Layout-Variante des Service-Wire-Modells (Spec §7.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceLayout {
    /// Spec §7.5.1.1 — ein Request-/Reply-Topic-Paar pro Service.
    Basic,
    /// Spec §7.5.1.2 — ein Topic-Paar pro Methode.
    Enhanced,
}

/// Member einer generierten Wire-Struktur.
#[derive(Debug, Clone, PartialEq)]
pub struct StructMember {
    /// Member-Name (z.B. `header`, `request_id`, `result`).
    pub name: String,
    /// Typ-Referenz oder synthetisches Token (z.B. `RequestHeader`).
    pub type_ref: MemberType,
}

impl StructMember {
    /// Konstruktor.
    #[must_use]
    pub fn new(name: impl Into<String>, type_ref: MemberType) -> Self {
        Self {
            name: name.into(),
            type_ref,
        }
    }
}

/// Typ eines Struct-Members. Entweder ein bekannter RPC-Header-Typ
/// (`RequestHeader`/`ReplyHeader`/`<Service>_Call`) oder ein IDL-Typ
/// aus `zerodds_idl::ast::TypeSpec`.
#[derive(Debug, Clone, PartialEq)]
pub enum MemberType {
    /// `RequestHeader` aus `common_types.rs`.
    RequestHeader,
    /// `ReplyHeader` aus `common_types.rs`.
    ReplyHeader,
    /// Synthetisches Union ueber alle Methoden eines Service. Trager
    /// fuer den Basic-Layout-Switch.
    CallUnion(CallUnionDef),
    /// Ein normaler IDL-Typ.
    Idl(TypeRef),
}

/// Discriminator-Case einer Call-Union (Basic-Layout).
#[derive(Debug, Clone, PartialEq)]
pub struct CallUnionCase {
    /// Methoden-Name (= Discriminator-Label `OP_<name>`).
    pub method: String,
    /// Diskriminator-Wert (1-basiert in Reihenfolge der `MethodDef`-Liste).
    pub discriminator: u32,
    /// `<Service>_<method>_<In|Out>`-Typname.
    pub case_type_name: String,
    /// Member-Liste der Case-Struktur (`In` bzw. `Out`).
    pub members: Vec<StructMember>,
}

/// `<Service>_Call` (Request) bzw. `<Service>_Result` (Reply) Union.
#[derive(Debug, Clone, PartialEq)]
pub struct CallUnionDef {
    /// Vollstaendiger Union-Typname (z.B. `Calculator_Call`).
    pub name: String,
    /// Cases. `Reply`-Union enthaelt nur Methoden, die nicht `oneway`
    /// sind.
    pub cases: Vec<CallUnionCase>,
}

/// Request-Wire-Datenmodell.
///
/// * Basic-Layout: ein RequestType pro Service, mit `header` +
///   `union`-Member ueber alle Methoden.
/// * Enhanced-Layout: ein RequestType pro Methode, mit `header` +
///   typisierten Pro-Methode-Feldern.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestType {
    /// Vollstaendiger Strukturname (z.B. `Calculator_Request` oder
    /// `Calculator_add_Request`).
    pub name: String,
    /// Topic-Name dieser Request-Wire-Struktur.
    pub topic_name: String,
    /// Layout-Variante, aus der die Struktur erzeugt wurde.
    pub layout: ServiceLayout,
    /// Methoden-Name fuer Enhanced; `None` bei Basic.
    pub method: Option<String>,
    /// Member-Liste (Header + Body).
    pub members: Vec<StructMember>,
}

/// Reply-Wire-Datenmodell. Analog zu [`RequestType`].
#[derive(Debug, Clone, PartialEq)]
pub struct ReplyType {
    /// Vollstaendiger Strukturname.
    pub name: String,
    /// Topic-Name.
    pub topic_name: String,
    /// Layout-Variante.
    pub layout: ServiceLayout,
    /// Methoden-Name fuer Enhanced; `None` bei Basic.
    pub method: Option<String>,
    /// Member-Liste.
    pub members: Vec<StructMember>,
}

/// Pair fuer eine Enhanced-Methode. Reply ist `None` bei `oneway`.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodPair {
    /// Request-Wire-Struktur dieser Methode.
    pub request: RequestType,
    /// Reply-Wire-Struktur (`None` bei `oneway`-Methoden).
    pub reply: Option<ReplyType>,
}

// ---------------------------------------------------------------------
// Basic-Service-Mapping (Spec §7.5.1.1)
// ---------------------------------------------------------------------

/// Liefert die Datenmodelle der Basic-Layout-Topics
/// (`<Service>_Request` plus `<Service>_Reply`). Ein leerer `ServiceDef`
/// liefert ein Pair mit leeren Unions — der Caller muss selbst entscheiden,
/// ob er das verwirft.
///
/// # Errors
/// `RpcError::InvalidServiceName` wenn der Service-Name leer ist (das
/// passiert nur bei manuell konstruierten ServiceDefs — `lower_service`
/// validiert bereits).
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

/// Liefert das Pro-Methode-Pair (Request + optional Reply).
///
/// Reply ist `None`, wenn die Methode `oneway` ist.
///
/// # Errors
/// `RpcError::InvalidServiceName` bzw. `RpcError::InvalidMethodName`
/// bei inkonsistenten ServiceDefs.
pub fn build_enhanced_pair(svc: &ServiceDef, method: &MethodDef) -> RpcResult<MethodPair> {
    if svc.name.is_empty() {
        return Err(RpcError::InvalidServiceName(String::new()));
    }
    if method.name.is_empty() {
        return Err(RpcError::InvalidMethodName(String::new()));
    }
    // Enhanced-Topic-Naming: `<Service>_<Method>_Request` /
    // `<Service>_<Method>_Reply` (Spec §7.5.1.2 + §7.8.2 erlaubt Vendor-
    // Erweiterung; Cyclone/FastDDS verwenden diese Form).
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

/// Liefert alle Enhanced-Layout-Pairs eines Service.
///
/// # Errors
/// Siehe [`build_enhanced_pair`].
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
        // `out` und `inout` landen beide im Reply.
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
        // Beide Methoden auch oneway-`log` muessen im Request-Union sein.
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
        // `log` ist oneway → nicht im Reply.
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
        // Request-Members: header + msg.
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
        assert!(pairs[1].reply.is_none()); // log ist oneway.
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
