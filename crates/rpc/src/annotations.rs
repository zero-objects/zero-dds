// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-RPC IDL-Annotations — Spec §7.3.
//!
//! Lowering der RPC-spezifischen Annotations:
//!
//! * `@service(name="<svc>")` — markiert eine Interface-Definition als
//!   RPC-Service. Wenn `name` fehlt, wird der Interface-Name verwendet.
//! * `@oneway` — Methode ohne Reply-Erwartung. Aequivalent zum nativen
//!   IDL-`oneway`-Keyword (das im AST bereits durch `OpDecl::oneway`
//!   abgebildet ist).
//! * `@in`, `@out`, `@inout` — explizite Parameter-Direction. Aequivalent
//!   zu den nativen IDL-Direction-Keywords (`ParamAttribute::In/Out/InOut`).
//!
//! Architektur-Entscheidung: das `zerodds-idl`-Crate kennt bewusst nur die
//! Standard-XTypes-Annotations (siehe Memo `project_codegen_templates_scope`
//! — RPC-Specifics gehoeren nicht ins idl-Crate). Diese Bridge konsumiert
//! die generischen `Annotation`-Werte aus dem AST und lowert sie zu
//! `RpcAnnotation`-Variants.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_idl::ast::{Annotation, AnnotationParams, ConstExpr, LiteralKind, NamedParam};

/// Typisierte Repraesentation der RPC-spezifischen Builtin-Annotations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcAnnotation {
    /// `@service` oder `@service(name="...")`.
    Service {
        /// Optional `name="..."`-Attribut.
        name: Option<String>,
    },
    /// `@oneway` (Annotation-Form; orthogonal zum AST-`oneway`-Keyword).
    Oneway,
    /// `@in` (Annotation-Form; orthogonal zu `ParamAttribute::In`).
    In,
    /// `@out`.
    Out,
    /// `@inout`.
    InOut,
    /// `@RPCInterfaceQos(profile="<lib::profile>")` (Spec §7.10.1).
    /// Verweist auf ein QoS-Profile aus einer XML-QoS-Library, das
    /// auf Service-Endpoints (Request-Writer/Reader, Reply-
    /// Writer/Reader) angewendet wird.
    InterfaceQos {
        /// `lib::profile`-Verweis (Spec-Konvention).
        profile_ref: String,
    },
    /// `@DDSRequestTopic("<topic>")` (Spec §7.4.2.2).
    /// Override des Default-Request-Topic-Namens.
    DdsRequestTopic(String),
    /// `@DDSReplyTopic("<topic>")` (Spec §7.4.2.2).
    DdsReplyTopic(String),
    /// `@RPCRequestType` (Spec §7.3.1.4).
    /// Markiert einen IDL-Struct als Pair-of-Types Request-Type.
    RpcRequestType,
    /// `@RPCReplyType` (Spec §7.3.1.4).
    RpcReplyType,
    /// `@RPCRequest` (Spec §7.3.1.2 Enhanced).
    /// Markiert eine IDL-Operation-Inputs-Struktur.
    RpcRequest,
    /// `@RPCReply` (Spec §7.3.1.2 Enhanced).
    RpcReply,
}

/// Ergebnis eines RPC-Annotation-Lowerings — getrennte Listen fuer
/// erkannte und durchgereichte (unbekannte) Annotations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LoweredRpc {
    /// Erkannte RPC-Builtins.
    pub builtins: Vec<RpcAnnotation>,
    /// Nicht erkannt (vendor-spezifisch oder XTypes-Builtin).
    pub custom: Vec<Annotation>,
}

impl LoweredRpc {
    /// `true` wenn `@service` (mit oder ohne Argument) gesetzt ist.
    #[must_use]
    pub fn is_service(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::Service { .. }))
    }

    /// Liefert den expliziten `@service(name="...")`-Wert, falls gesetzt.
    #[must_use]
    pub fn service_name(&self) -> Option<&str> {
        self.builtins.iter().find_map(|a| match a {
            RpcAnnotation::Service { name: Some(n) } => Some(n.as_str()),
            _ => None,
        })
    }

    /// `true` wenn `@oneway`-Annotation gesetzt ist.
    #[must_use]
    pub fn has_oneway(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::Oneway))
    }

    /// Spec §7.10.1: liefert das `@RPCInterfaceQos(profile=...)`-Argument.
    #[must_use]
    pub fn interface_qos_profile(&self) -> Option<&str> {
        self.builtins.iter().find_map(|a| match a {
            RpcAnnotation::InterfaceQos { profile_ref } => Some(profile_ref.as_str()),
            _ => None,
        })
    }

    /// Spec §7.4.2.2: liefert den `@DDSRequestTopic`-Override.
    #[must_use]
    pub fn dds_request_topic(&self) -> Option<&str> {
        self.builtins.iter().find_map(|a| match a {
            RpcAnnotation::DdsRequestTopic(t) => Some(t.as_str()),
            _ => None,
        })
    }

    /// Spec §7.4.2.2: liefert den `@DDSReplyTopic`-Override.
    #[must_use]
    pub fn dds_reply_topic(&self) -> Option<&str> {
        self.builtins.iter().find_map(|a| match a {
            RpcAnnotation::DdsReplyTopic(t) => Some(t.as_str()),
            _ => None,
        })
    }

    /// Spec §7.3.1.4: `true` wenn `@RPCRequestType` gesetzt.
    #[must_use]
    pub fn is_rpc_request_type(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::RpcRequestType))
    }

    /// Spec §7.3.1.4: `true` wenn `@RPCReplyType` gesetzt.
    #[must_use]
    pub fn is_rpc_reply_type(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::RpcReplyType))
    }

    /// Spec §7.3.1.2: `true` wenn `@RPCRequest` gesetzt.
    #[must_use]
    pub fn is_rpc_request(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::RpcRequest))
    }

    /// Spec §7.3.1.2: `true` wenn `@RPCReply` gesetzt.
    #[must_use]
    pub fn is_rpc_reply(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::RpcReply))
    }
}

fn name_tail(a: &Annotation) -> &str {
    a.name
        .parts
        .last()
        .map(|p| p.text.as_str())
        .unwrap_or_default()
}

fn const_to_string(expr: &ConstExpr) -> Option<String> {
    if let ConstExpr::Literal(l) = expr {
        let s = l.raw.as_str();
        if matches!(l.kind, LiteralKind::String) {
            return Some(
                s.strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .unwrap_or(s)
                    .to_string(),
            );
        }
        return Some(s.to_string());
    }
    None
}

fn extract_named<'a>(named: &'a [NamedParam], key: &str) -> Option<&'a ConstExpr> {
    named.iter().find(|p| p.name.text == key).map(|p| &p.value)
}

/// Lower eine einzelne Annotation auf ihre RPC-typisierte Form.
/// Liefert `None` wenn sie kein RPC-Builtin ist (Caller stellt sie dann
/// in `LoweredRpc::custom` ab).
#[must_use]
pub fn lower_single(ann: &Annotation) -> Option<RpcAnnotation> {
    let name = name_tail(ann);
    match name {
        "service" => {
            let svc_name = match &ann.params {
                AnnotationParams::None | AnnotationParams::Empty => None,
                AnnotationParams::Single(e) => const_to_string(e),
                AnnotationParams::Named(named) => {
                    extract_named(named, "name").and_then(const_to_string)
                }
            };
            Some(RpcAnnotation::Service { name: svc_name })
        }
        "oneway" => Some(RpcAnnotation::Oneway),
        "in" => Some(RpcAnnotation::In),
        "out" => Some(RpcAnnotation::Out),
        "inout" => Some(RpcAnnotation::InOut),
        "RPCInterfaceQos" => {
            let profile_ref = match &ann.params {
                AnnotationParams::Single(e) => const_to_string(e),
                AnnotationParams::Named(named) => {
                    extract_named(named, "profile").and_then(const_to_string)
                }
                _ => None,
            }?;
            Some(RpcAnnotation::InterfaceQos { profile_ref })
        }
        "DDSRequestTopic" => {
            let topic = match &ann.params {
                AnnotationParams::Single(e) => const_to_string(e),
                AnnotationParams::Named(named) => {
                    extract_named(named, "name").and_then(const_to_string)
                }
                _ => None,
            }?;
            Some(RpcAnnotation::DdsRequestTopic(topic))
        }
        "DDSReplyTopic" => {
            let topic = match &ann.params {
                AnnotationParams::Single(e) => const_to_string(e),
                AnnotationParams::Named(named) => {
                    extract_named(named, "name").and_then(const_to_string)
                }
                _ => None,
            }?;
            Some(RpcAnnotation::DdsReplyTopic(topic))
        }
        "RPCRequestType" => Some(RpcAnnotation::RpcRequestType),
        "RPCReplyType" => Some(RpcAnnotation::RpcReplyType),
        "RPCRequest" => Some(RpcAnnotation::RpcRequest),
        "RPCReply" => Some(RpcAnnotation::RpcReply),
        _ => None,
    }
}

/// Lower eine Annotation-Liste in das RPC-Builtin-Modell.
#[must_use]
pub fn lower_rpc_annotations(anns: &[Annotation]) -> LoweredRpc {
    let mut out = LoweredRpc::default();
    for a in anns {
        match lower_single(a) {
            Some(b) => out.builtins.push(b),
            None => out.custom.push(a.clone()),
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use zerodds_idl::ast::{Identifier, Literal, ScopedName};
    use zerodds_idl::errors::Span;

    fn sp() -> Span {
        Span::SYNTHETIC
    }

    fn ident(t: &str) -> Identifier {
        Identifier::new(t, sp())
    }

    fn scoped(parts: &[&str]) -> ScopedName {
        ScopedName {
            absolute: false,
            parts: parts.iter().map(|p| ident(p)).collect(),
            span: sp(),
        }
    }

    fn lit(kind: LiteralKind, raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind,
            raw: raw.to_string(),
            span: sp(),
        })
    }

    fn ann(name: &str, params: AnnotationParams) -> Annotation {
        Annotation {
            name: scoped(&[name]),
            params,
            span: sp(),
        }
    }

    #[test]
    fn service_no_args_lowers_without_name() {
        let a = lower_single(&ann("service", AnnotationParams::None));
        assert_eq!(a, Some(RpcAnnotation::Service { name: None }));
    }

    #[test]
    fn service_named_arg_lowers_with_name() {
        let a = lower_single(&ann(
            "service",
            AnnotationParams::Named(alloc::vec![NamedParam {
                name: ident("name"),
                value: lit(LiteralKind::String, "\"Calculator\""),
                span: sp(),
            }]),
        ));
        assert_eq!(
            a,
            Some(RpcAnnotation::Service {
                name: Some("Calculator".into())
            })
        );
    }

    #[test]
    fn service_single_string_arg_lowers_with_name() {
        // Toleranter Pfad: @service("Foo") wird wie name="Foo" behandelt.
        let a = lower_single(&ann(
            "service",
            AnnotationParams::Single(lit(LiteralKind::String, "\"Foo\"")),
        ));
        assert_eq!(
            a,
            Some(RpcAnnotation::Service {
                name: Some("Foo".into())
            })
        );
    }

    #[test]
    fn service_named_arg_with_unknown_key_yields_none_name() {
        let a = lower_single(&ann(
            "service",
            AnnotationParams::Named(alloc::vec![NamedParam {
                name: ident("ignored"),
                value: lit(LiteralKind::String, "\"X\""),
                span: sp(),
            }]),
        ));
        assert_eq!(a, Some(RpcAnnotation::Service { name: None }));
    }

    #[test]
    fn oneway_lowers() {
        assert_eq!(
            lower_single(&ann("oneway", AnnotationParams::None)),
            Some(RpcAnnotation::Oneway)
        );
    }

    #[test]
    fn in_out_inout_lower() {
        assert_eq!(
            lower_single(&ann("in", AnnotationParams::None)),
            Some(RpcAnnotation::In)
        );
        assert_eq!(
            lower_single(&ann("out", AnnotationParams::None)),
            Some(RpcAnnotation::Out)
        );
        assert_eq!(
            lower_single(&ann("inout", AnnotationParams::None)),
            Some(RpcAnnotation::InOut)
        );
    }

    #[test]
    fn unknown_annotation_returns_none() {
        let a = lower_single(&ann("xtypes_builtin", AnnotationParams::None));
        assert!(a.is_none());
    }

    #[test]
    fn lower_rpc_annotations_splits_builtins_and_custom() {
        let anns = alloc::vec![
            ann("service", AnnotationParams::None),
            ann("topic", AnnotationParams::None), // XTypes-builtin, nicht RPC
            ann("oneway", AnnotationParams::None),
        ];
        let lowered = lower_rpc_annotations(&anns);
        assert_eq!(lowered.builtins.len(), 2);
        assert_eq!(lowered.custom.len(), 1);
        assert!(lowered.is_service());
        assert!(lowered.has_oneway());
    }

    #[test]
    fn service_name_resolved_via_helper() {
        let anns = alloc::vec![ann(
            "service",
            AnnotationParams::Named(alloc::vec![NamedParam {
                name: ident("name"),
                value: lit(LiteralKind::String, "\"Bar\""),
                span: sp(),
            }])
        )];
        let lowered = lower_rpc_annotations(&anns);
        assert_eq!(lowered.service_name(), Some("Bar"));
    }

    #[test]
    fn service_without_name_yields_none_helper() {
        let anns = alloc::vec![ann("service", AnnotationParams::None)];
        let lowered = lower_rpc_annotations(&anns);
        assert!(lowered.is_service());
        assert_eq!(lowered.service_name(), None);
    }

    // ---- §7.10.1 @RPCInterfaceQos -----------------------------------

    #[test]
    fn rpc_interface_qos_with_named_profile_lowers() {
        let a = lower_single(&ann(
            "RPCInterfaceQos",
            AnnotationParams::Named(alloc::vec![NamedParam {
                name: ident("profile"),
                value: lit(LiteralKind::String, "\"Lib1::Reliable\""),
                span: sp(),
            }]),
        ));
        assert_eq!(
            a,
            Some(RpcAnnotation::InterfaceQos {
                profile_ref: "Lib1::Reliable".into()
            })
        );
    }

    #[test]
    fn rpc_interface_qos_single_string_arg_lowers() {
        let a = lower_single(&ann(
            "RPCInterfaceQos",
            AnnotationParams::Single(lit(LiteralKind::String, "\"Lib::P\"")),
        ));
        assert_eq!(
            a,
            Some(RpcAnnotation::InterfaceQos {
                profile_ref: "Lib::P".into()
            })
        );
    }

    #[test]
    fn interface_qos_profile_resolved_via_helper() {
        let anns = alloc::vec![ann(
            "RPCInterfaceQos",
            AnnotationParams::Single(lit(LiteralKind::String, "\"Lib::Foo\""))
        )];
        let lowered = lower_rpc_annotations(&anns);
        assert_eq!(lowered.interface_qos_profile(), Some("Lib::Foo"));
    }

    // ---- §7.4.2.2 @DDSRequestTopic / @DDSReplyTopic ----------------

    #[test]
    fn dds_request_topic_lowers_and_resolves() {
        let anns = alloc::vec![ann(
            "DDSRequestTopic",
            AnnotationParams::Single(lit(LiteralKind::String, "\"MyReqTopic\""))
        )];
        let lowered = lower_rpc_annotations(&anns);
        assert_eq!(lowered.dds_request_topic(), Some("MyReqTopic"));
    }

    #[test]
    fn dds_reply_topic_lowers_and_resolves() {
        let anns = alloc::vec![ann(
            "DDSReplyTopic",
            AnnotationParams::Single(lit(LiteralKind::String, "\"MyRepTopic\""))
        )];
        let lowered = lower_rpc_annotations(&anns);
        assert_eq!(lowered.dds_reply_topic(), Some("MyRepTopic"));
    }

    // ---- §7.3.1.4 Pair of Types: @RPCRequestType / @RPCReplyType ----

    #[test]
    fn rpc_request_type_lowers_and_resolves() {
        let anns = alloc::vec![ann("RPCRequestType", AnnotationParams::None)];
        let lowered = lower_rpc_annotations(&anns);
        assert!(lowered.is_rpc_request_type());
        assert!(!lowered.is_rpc_reply_type());
    }

    #[test]
    fn rpc_reply_type_lowers_and_resolves() {
        let anns = alloc::vec![ann("RPCReplyType", AnnotationParams::None)];
        let lowered = lower_rpc_annotations(&anns);
        assert!(lowered.is_rpc_reply_type());
    }

    // ---- §7.3.1.2 Enhanced @RPCRequest / @RPCReply -----------------

    #[test]
    fn rpc_request_and_reply_lower_and_resolve() {
        let anns = alloc::vec![
            ann("RPCRequest", AnnotationParams::None),
            ann("RPCReply", AnnotationParams::None),
        ];
        let lowered = lower_rpc_annotations(&anns);
        assert!(lowered.is_rpc_request());
        assert!(lowered.is_rpc_reply());
    }
}
