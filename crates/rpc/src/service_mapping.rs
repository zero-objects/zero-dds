// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Service-IDL → typisiertes Service-Datenmodell — Spec §7.4.
//!
//! Die DDS-RPC-Spec mappt eine IDL-Service-Definition
//!
//! ```text
//! @service interface Calculator {
//!     long add(in long a, in long b);
//!     oneway void log(in string msg);
//! };
//! ```
//!
//! auf zwei Wire-Strukturen pro Methode:
//!
//! * `<Service>_<Method>_In`  — Request-Payload (alle `in`/`inout`).
//! * `<Service>_<Method>_Out` — Reply-Payload (Return + `out`/`inout`).
//!
//! Diese Foundation-Stufe (C6.1.A) stellt nur das **Datenmodell** dar
//! ([`ServiceDef`], [`MethodDef`], [`ParamDef`]). Die eigentliche
//! Codegen-Stufe (C6.1.B) konsumiert das Modell und emittiert IDL-
//! Strukturen + Rust-Bindings.
//!
//! Das Modell wird per [`lower_service`] aus einem `zerodds_idl::ast::
//! InterfaceDef` plus den bereits typisierten RPC-Annotations
//! ([`crate::annotations::LoweredRpc`]) konstruiert. Validierung in
//! dieser Stufe:
//!
//! * Service-Name ist nicht-leer + alphanumerisch + `_`.
//! * Methoden-Namen sind eindeutig.
//! * Parameter-Namen pro Methode sind eindeutig.
//! * `oneway`-Methoden haben `void`-Return und keine `out`/`inout`-Params.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_idl::ast::{Export, InterfaceDef, OpDecl, ParamAttribute, TypeSpec};

use crate::annotations::{LoweredRpc, lower_rpc_annotations};
use crate::error::{RpcError, RpcResult};
use crate::topic_naming::{ServiceTopicNames, validate_service_name};

/// Direction-Attribut eines RPC-Parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamDirection {
    /// `in` — Request-only.
    In,
    /// `out` — Reply-only.
    Out,
    /// `inout` — beides.
    InOut,
}

impl From<ParamAttribute> for ParamDirection {
    fn from(value: ParamAttribute) -> Self {
        match value {
            ParamAttribute::In => Self::In,
            ParamAttribute::Out => Self::Out,
            ParamAttribute::InOut => Self::InOut,
        }
    }
}

impl ParamDirection {
    /// `true` wenn Parameter im Request-Topic landet (`in` oder `inout`).
    #[must_use]
    pub const fn is_in(self) -> bool {
        matches!(self, Self::In | Self::InOut)
    }

    /// `true` wenn Parameter im Reply-Topic landet (`out` oder `inout`).
    #[must_use]
    pub const fn is_out(self) -> bool {
        matches!(self, Self::Out | Self::InOut)
    }
}

/// Type-Referenz im Service-Modell. In Foundation-Stufe ist das nur ein
/// Re-Use des AST-`TypeSpec` — Phase C6.1.B kann hier ein resolviertes
/// Type-Objekt einsetzen.
pub type TypeRef = TypeSpec;

/// Ein RPC-Parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDef {
    /// Parameter-Name.
    pub name: String,
    /// Direction (`in`/`out`/`inout`).
    pub direction: ParamDirection,
    /// Typ.
    pub type_ref: TypeRef,
}

/// Eine RPC-Methode.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodDef {
    /// Methoden-Name.
    pub name: String,
    /// Parameter in Deklarations-Reihenfolge.
    pub params: Vec<ParamDef>,
    /// Return-Type. `None` bei `void`.
    pub return_type: Option<TypeRef>,
    /// `true` wenn `oneway` (kein Reply, nur `in`-Params, void-Return).
    pub oneway: bool,
}

impl MethodDef {
    /// Alle `in`/`inout`-Parameter (Request-Felder).
    pub fn in_params(&self) -> impl Iterator<Item = &ParamDef> {
        self.params.iter().filter(|p| p.direction.is_in())
    }

    /// Alle `out`/`inout`-Parameter (Reply-Felder).
    pub fn out_params(&self) -> impl Iterator<Item = &ParamDef> {
        self.params.iter().filter(|p| p.direction.is_out())
    }
}

/// Eine RPC-Service-Definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceDef {
    /// Effektiver Service-Name (`@service(name="...")` oder Interface-Name).
    pub name: String,
    /// Methoden.
    pub methods: Vec<MethodDef>,
}

impl ServiceDef {
    /// Liefert die zugehoerigen Topic-Namen (`<svc>_Request` / `<svc>_Reply`).
    ///
    /// # Errors
    /// Sollte nicht passieren — `lower_service` validiert den Namen
    /// bereits. Defensiv trotzdem propagiert.
    pub fn topic_names(&self) -> RpcResult<ServiceTopicNames> {
        ServiceTopicNames::new(&self.name)
    }
}

/// Lowert eine IDL-Service-Definition in das typisierte Service-Modell.
///
/// Erwartet, dass die Annotations des Interface bereits per
/// [`lower_rpc_annotations`] vorgelowert wurden. Wenn `@service` nicht
/// gesetzt ist, wird der Interface-Name als Service-Name benutzt.
///
/// # Errors
/// * `RpcError::InvalidServiceName` bei leerem oder ungueltigem Namen.
/// * `RpcError::InvalidMethodName` bei leerem Methoden-Namen.
/// * `RpcError::OnewayWithReturn` / `OnewayWithOutParam` bei
///   inkonsistenten oneway-Deklarationen.
/// * `RpcError::DuplicateMethod` / `DuplicateParam` bei Namens-Kollisionen.
pub fn lower_service(iface: &InterfaceDef, lowered: &LoweredRpc) -> RpcResult<ServiceDef> {
    // Service-Name aus @service(name="...") oder Interface-Name.
    let svc_name = lowered
        .service_name()
        .map(ToString::to_string)
        .unwrap_or_else(|| iface.name.text.clone());
    validate_service_name(&svc_name)?;

    let mut methods = Vec::new();
    for export in &iface.exports {
        if let Export::Op(op) = export {
            methods.push(lower_method(op)?);
        }
        // Andere Exports (Attr/Type/Const/Except) sind in C6.1.A explizit
        // out-of-scope — exceptions werden in C6.1.B als RemoteException-
        // Carrier modelliert.
    }

    // Duplicate-Method-Check.
    for i in 0..methods.len() {
        for j in (i + 1)..methods.len() {
            if methods[i].name == methods[j].name {
                return Err(RpcError::DuplicateMethod(methods[i].name.clone()));
            }
        }
    }

    Ok(ServiceDef {
        name: svc_name,
        methods,
    })
}

fn lower_method(op: &OpDecl) -> RpcResult<MethodDef> {
    let name = op.name.text.clone();
    if name.is_empty() {
        return Err(RpcError::InvalidMethodName(name));
    }

    // Annotation-Lowering der Methoden-Annotations: `@oneway` als
    // Annotation-Form muss aequivalent zum AST-`oneway`-Keyword
    // behandelt werden.
    let method_anns = lower_rpc_annotations(&op.annotations);
    let oneway = op.oneway || method_anns.has_oneway();

    let return_type = op.return_type.clone();

    if oneway && return_type.is_some() {
        return Err(RpcError::OnewayWithReturn(name));
    }

    let mut params = Vec::with_capacity(op.params.len());
    for p in &op.params {
        // Param-Annotation `@in/@out/@inout` ueberschreibt das native
        // ParamAttribute, falls explizit gesetzt.
        let p_anns = lower_rpc_annotations(&p.annotations);
        let direction = override_direction(p.attribute, &p_anns);

        if oneway && direction.is_out() {
            return Err(RpcError::OnewayWithOutParam {
                method: name.clone(),
                param: p.name.text.clone(),
            });
        }

        params.push(ParamDef {
            name: p.name.text.clone(),
            direction,
            type_ref: p.type_spec.clone(),
        });
    }

    // Duplicate-Param-Check.
    for i in 0..params.len() {
        for j in (i + 1)..params.len() {
            if params[i].name == params[j].name {
                return Err(RpcError::DuplicateParam {
                    method: name,
                    param: params[i].name.clone(),
                });
            }
        }
    }

    Ok(MethodDef {
        name,
        params,
        return_type,
        oneway,
    })
}

fn override_direction(native: ParamAttribute, anns: &LoweredRpc) -> ParamDirection {
    use crate::annotations::RpcAnnotation;
    for a in &anns.builtins {
        match a {
            RpcAnnotation::In => return ParamDirection::In,
            RpcAnnotation::Out => return ParamDirection::Out,
            RpcAnnotation::InOut => return ParamDirection::InOut,
            _ => {}
        }
    }
    native.into()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use zerodds_idl::ast::{
        Annotation, AnnotationParams, Identifier, IntegerType, InterfaceKind, OpDecl, ParamDecl,
        PrimitiveType, ScopedName, StringType, TypeSpec,
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

    fn op(
        name: &str,
        oneway: bool,
        ret: Option<TypeSpec>,
        params: Vec<ParamDecl>,
        anns: Vec<Annotation>,
    ) -> OpDecl {
        OpDecl {
            name: ident(name),
            oneway,
            return_type: ret,
            params,
            raises: Vec::new(),
            annotations: anns,
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

    fn iface(name: &str, exports: Vec<Export>, anns: Vec<Annotation>) -> InterfaceDef {
        InterfaceDef {
            kind: InterfaceKind::Plain,
            name: ident(name),
            bases: Vec::new(),
            exports,
            annotations: anns,
            span: sp(),
        }
    }

    fn ann_simple(name: &str) -> Annotation {
        Annotation {
            name: ScopedName {
                absolute: false,
                parts: vec![ident(name)],
                span: sp(),
            },
            params: AnnotationParams::None,
            span: sp(),
        }
    }

    #[test]
    fn calculator_service_with_in_params_lowers() {
        let add = op(
            "add",
            false,
            Some(long_t()),
            vec![
                param("a", ParamAttribute::In, long_t()),
                param("b", ParamAttribute::In, long_t()),
            ],
            Vec::new(),
        );
        let i = iface(
            "Calculator",
            vec![Export::Op(add)],
            vec![ann_simple("service")],
        );
        let lowered = lower_rpc_annotations(&i.annotations);
        let svc = lower_service(&i, &lowered).unwrap();
        assert_eq!(svc.name, "Calculator");
        assert_eq!(svc.methods.len(), 1);
        let m = &svc.methods[0];
        assert_eq!(m.name, "add");
        assert!(!m.oneway);
        assert_eq!(m.params.len(), 2);
        assert_eq!(m.in_params().count(), 2);
        assert_eq!(m.out_params().count(), 0);
        assert_eq!(svc.topic_names().unwrap().request, "Calculator_Request");
    }

    #[test]
    fn oneway_method_with_return_is_error() {
        let bad = op(
            "log",
            true,
            Some(long_t()),
            vec![param("msg", ParamAttribute::In, string_t())],
            Vec::new(),
        );
        let i = iface("Logger", vec![Export::Op(bad)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let err = lower_service(&i, &lowered).unwrap_err();
        assert!(matches!(err, RpcError::OnewayWithReturn(_)));
    }

    #[test]
    fn oneway_method_with_out_param_is_error() {
        let bad = op(
            "fire",
            true,
            None,
            vec![param("result", ParamAttribute::Out, long_t())],
            Vec::new(),
        );
        let i = iface("Svc", vec![Export::Op(bad)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let err = lower_service(&i, &lowered).unwrap_err();
        assert!(matches!(err, RpcError::OnewayWithOutParam { .. }));
    }

    #[test]
    fn oneway_method_with_inout_param_is_error() {
        let bad = op(
            "fire",
            true,
            None,
            vec![param("v", ParamAttribute::InOut, long_t())],
            Vec::new(),
        );
        let i = iface("Svc", vec![Export::Op(bad)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let err = lower_service(&i, &lowered).unwrap_err();
        assert!(matches!(err, RpcError::OnewayWithOutParam { .. }));
    }

    #[test]
    fn oneway_with_only_in_params_lowers() {
        let m = op(
            "log",
            true,
            None,
            vec![param("msg", ParamAttribute::In, string_t())],
            Vec::new(),
        );
        let i = iface("Logger", vec![Export::Op(m)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let svc = lower_service(&i, &lowered).unwrap();
        assert!(svc.methods[0].oneway);
        assert_eq!(svc.methods[0].in_params().count(), 1);
        assert_eq!(svc.methods[0].out_params().count(), 0);
    }

    #[test]
    fn oneway_via_annotation_recognized() {
        // Native oneway=false, aber @oneway-Annotation gesetzt.
        let m = op("ping", false, None, Vec::new(), vec![ann_simple("oneway")]);
        let i = iface("Svc", vec![Export::Op(m)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let svc = lower_service(&i, &lowered).unwrap();
        assert!(svc.methods[0].oneway);
    }

    #[test]
    fn duplicate_method_detected() {
        let m1 = op("foo", false, None, Vec::new(), Vec::new());
        let m2 = op("foo", false, None, Vec::new(), Vec::new());
        let i = iface("Svc", vec![Export::Op(m1), Export::Op(m2)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let err = lower_service(&i, &lowered).unwrap_err();
        assert_eq!(err, RpcError::DuplicateMethod("foo".into()));
    }

    #[test]
    fn duplicate_param_detected() {
        let m = op(
            "add",
            false,
            Some(long_t()),
            vec![
                param("x", ParamAttribute::In, long_t()),
                param("x", ParamAttribute::In, long_t()),
            ],
            Vec::new(),
        );
        let i = iface("Svc", vec![Export::Op(m)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let err = lower_service(&i, &lowered).unwrap_err();
        assert!(matches!(err, RpcError::DuplicateParam { .. }));
    }

    #[test]
    fn empty_method_name_rejected() {
        let m = op("", false, None, Vec::new(), Vec::new());
        let i = iface("Svc", vec![Export::Op(m)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let err = lower_service(&i, &lowered).unwrap_err();
        assert!(matches!(err, RpcError::InvalidMethodName(_)));
    }

    #[test]
    fn invalid_service_name_rejected() {
        let i = iface("Bad-Name", Vec::new(), Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let err = lower_service(&i, &lowered).unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    #[test]
    fn service_name_from_annotation_overrides_iface_name() {
        // @service(name="OuterName") gewinnt ueber den Interface-Namen.
        let i = iface(
            "InternalIface",
            Vec::new(),
            vec![Annotation {
                name: ScopedName {
                    absolute: false,
                    parts: vec![ident("service")],
                    span: sp(),
                },
                params: AnnotationParams::Named(vec![zerodds_idl::ast::NamedParam {
                    name: ident("name"),
                    value: zerodds_idl::ast::ConstExpr::Literal(zerodds_idl::ast::Literal {
                        kind: zerodds_idl::ast::LiteralKind::String,
                        raw: "\"OuterName\"".into(),
                        span: sp(),
                    }),
                    span: sp(),
                }]),
                span: sp(),
            }],
        );
        let lowered = lower_rpc_annotations(&i.annotations);
        let svc = lower_service(&i, &lowered).unwrap();
        assert_eq!(svc.name, "OuterName");
    }

    #[test]
    fn inout_param_appears_in_both_directions() {
        let m = op(
            "swap",
            false,
            None,
            vec![param("v", ParamAttribute::InOut, long_t())],
            Vec::new(),
        );
        let i = iface("Svc", vec![Export::Op(m)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let svc = lower_service(&i, &lowered).unwrap();
        let m = &svc.methods[0];
        assert_eq!(m.in_params().count(), 1);
        assert_eq!(m.out_params().count(), 1);
    }

    #[test]
    fn out_only_param_is_reply_only() {
        let m = op(
            "result",
            false,
            None,
            vec![param("v", ParamAttribute::Out, long_t())],
            Vec::new(),
        );
        let i = iface("Svc", vec![Export::Op(m)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let svc = lower_service(&i, &lowered).unwrap();
        let m = &svc.methods[0];
        assert_eq!(m.in_params().count(), 0);
        assert_eq!(m.out_params().count(), 1);
    }

    #[test]
    fn param_annotation_in_overrides_native_attr() {
        // ParamAttribute::Out, aber @in-Annotation -> @in gewinnt.
        let mut p = param("v", ParamAttribute::Out, long_t());
        p.annotations.push(ann_simple("in"));
        let m = op("foo", false, None, vec![p], Vec::new());
        let i = iface("Svc", vec![Export::Op(m)], Vec::new());
        let lowered = lower_rpc_annotations(&i.annotations);
        let svc = lower_service(&i, &lowered).unwrap();
        assert_eq!(svc.methods[0].params[0].direction, ParamDirection::In);
    }

    #[test]
    fn non_op_exports_are_ignored() {
        // Const-Exports sollen das Service-Modell nicht stoeren — sie
        // werden in C6.1.A explizit nicht abgebildet.
        let const_decl = zerodds_idl::ast::ConstDecl {
            name: ident("MAX"),
            type_: zerodds_idl::ast::ConstType::Integer(IntegerType::Long),
            value: zerodds_idl::ast::ConstExpr::Literal(zerodds_idl::ast::Literal {
                kind: zerodds_idl::ast::LiteralKind::Integer,
                raw: "10".into(),
                span: sp(),
            }),
            annotations: Vec::new(),
            span: sp(),
        };
        let m = op("foo", false, None, Vec::new(), Vec::new());
        let i = iface(
            "Svc",
            vec![Export::Const(const_decl), Export::Op(m)],
            Vec::new(),
        );
        let lowered = lower_rpc_annotations(&i.annotations);
        let svc = lower_service(&i, &lowered).unwrap();
        assert_eq!(svc.methods.len(), 1);
    }

    #[test]
    fn param_direction_helpers() {
        assert!(ParamDirection::In.is_in());
        assert!(!ParamDirection::In.is_out());
        assert!(!ParamDirection::Out.is_in());
        assert!(ParamDirection::Out.is_out());
        assert!(ParamDirection::InOut.is_in());
        assert!(ParamDirection::InOut.is_out());
    }

    #[test]
    fn param_direction_from_param_attribute() {
        assert_eq!(ParamDirection::from(ParamAttribute::In), ParamDirection::In);
        assert_eq!(
            ParamDirection::from(ParamAttribute::Out),
            ParamDirection::Out
        );
        assert_eq!(
            ParamDirection::from(ParamAttribute::InOut),
            ParamDirection::InOut
        );
    }
}
