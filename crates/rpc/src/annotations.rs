// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-RPC IDL annotations — Spec §7.3.
//!
//! Lowering of the RPC-specific annotations:
//!
//! * `@service(name="<svc>")` — marks an interface definition as an
//!   RPC service. If `name` is missing, the service name is `"Service"` (§7.4.1).
//! * `@oneway` — read for the Java service definitions of §7.3.2, which admit
//!   it. An IDL interface is refused with it (§7.3.1.1 removed `oneway`).
//! * `@in`, `@out`, `@inout` — explicit parameter direction. Equivalent
//!   to the native IDL direction keywords.
//!
//! Architecture decision: the IDL frontend knows the annotations the
//! specifications define and nothing else — RPC specifics do not belong
//! in it. This bridge reads what the frontend put on a node and lowers it
//! to `RpcAnnotation` variants.
//!
//! ## Where each of them now comes from
//!
//! The common AST does not carry a list of syntactic annotations per
//! node. The ones a specification defines reach typed facet fields and
//! the rest land in [`Facets::annotations`], so this module reads two
//! places rather than one:
//!
//! | Annotation                | Where it lands                        |
//! |---------------------------|---------------------------------------|
//! | `@service`                | `CorbaFacet::is_service` + `service_arguments` |
//! | `@oneway`                 | `OperationNode::oneway` — the keyword and the annotation set the same flag |
//! | everything else here      | `Facets::annotations`                 |
//!
//! Because `@oneway` reaches the operation's own flag, a `LoweredRpc`
//! built from facets never reports it; [`LoweredRpc::has_oneway`] stays
//! for a caller holding annotations from elsewhere, and
//! [`crate::service_mapping::lower_service`] reads the operation flag,
//! which both spellings set.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{AppliedAnnotation, AppliedArgument, Ast, ConstExpr, ConstExprNode, Facets};
use zerodds_ast_core::Id;

/// Typed representation of the RPC-specific builtin annotations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcAnnotation {
    /// `@service` or `@service(name="...")`.
    Service {
        /// Optional `name="..."` attribute.
        name: Option<String>,
    },
    /// `@oneway` (annotation form; orthogonal to the AST `oneway` keyword).
    Oneway,
    /// `@in` (annotation form; orthogonal to `ParamAttribute::In`).
    In,
    /// `@out`.
    Out,
    /// `@inout`.
    InOut,
    /// `@RPCInterfaceQos(profile="<lib::profile>")` (Spec §7.10.1).
    /// References a QoS profile from an XML QoS library that is
    /// applied to service endpoints (request writer/reader, reply
    /// writer/reader).
    InterfaceQos {
        /// `lib::profile` reference (spec convention).
        profile_ref: String,
    },
    /// `@Qos(RequestProfile = "<url>", ReplyProfile = "<url>")`
    /// (Spec §7.10.1). The two XML QoS profile URLs, one for the request
    /// endpoints and one for the reply endpoints.
    Qos {
        /// `RequestProfile` URL, if written.
        request_profile: Option<String>,
        /// `ReplyProfile` URL, if written.
        reply_profile: Option<String>,
    },
    /// `@DDSRequestTopic("<topic>")` (Spec §7.4.2.2).
    /// Override of the default request topic name.
    DdsRequestTopic(String),
    /// `@DDSReplyTopic("<topic>")` (Spec §7.4.2.2).
    DdsReplyTopic(String),
    /// `@RPCRequestType` (Spec §7.3.1.4).
    /// Marks an IDL struct as a pair-of-types request type.
    RpcRequestType,
    /// `@RPCReplyType` (Spec §7.3.1.4).
    RpcReplyType,
    /// `@Mutable` on an interface operation (Spec §7.5.1.2.1.4): the
    /// synthesized `In`, `Out` and `Result` structures of the Enhanced
    /// Service Mapping are `@Extensibility(MUTABLE_EXTENSIBILITY)`.
    Mutable,
}

/// Result of an RPC annotation lowering — separate lists for
/// recognized and passed-through (unknown) annotations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LoweredRpc {
    /// Recognized RPC builtins.
    pub builtins: Vec<RpcAnnotation>,
    /// Not recognized (vendor-specific or an XTypes builtin).
    pub custom: Vec<AppliedAnnotation>,
}

impl LoweredRpc {
    /// `true` if `@service` (with or without argument) is set.
    #[must_use]
    pub fn is_service(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::Service { .. }))
    }

    /// Returns the explicit `@service(name="...")` value, if set.
    #[must_use]
    pub fn service_name(&self) -> Option<&str> {
        self.builtins.iter().find_map(|a| match a {
            RpcAnnotation::Service { name: Some(n) } => Some(n.as_str()),
            _ => None,
        })
    }

    /// `true` if the `@oneway` annotation is set.
    #[must_use]
    pub fn has_oneway(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::Oneway))
    }

    /// Spec §7.10.1: returns the `@RPCInterfaceQos(profile=...)` argument.
    #[must_use]
    pub fn interface_qos_profile(&self) -> Option<&str> {
        self.builtins.iter().find_map(|a| match a {
            RpcAnnotation::InterfaceQos { profile_ref } => Some(profile_ref.as_str()),
            _ => None,
        })
    }

    /// Spec §7.10.1: the `@Qos` request/reply profile URLs, if the
    /// annotation carried a `RequestProfile`.
    #[must_use]
    pub fn qos_profiles(&self) -> Option<(&str, Option<&str>)> {
        self.builtins.iter().find_map(|a| match a {
            RpcAnnotation::Qos {
                request_profile,
                reply_profile,
            } => request_profile
                .as_deref()
                .map(|request| (request, reply_profile.as_deref())),
            _ => None,
        })
    }

    /// Spec §7.4.2.2: returns the `@DDSRequestTopic` override.
    #[must_use]
    pub fn dds_request_topic(&self) -> Option<&str> {
        self.builtins.iter().find_map(|a| match a {
            RpcAnnotation::DdsRequestTopic(t) => Some(t.as_str()),
            _ => None,
        })
    }

    /// Spec §7.4.2.2: returns the `@DDSReplyTopic` override.
    #[must_use]
    pub fn dds_reply_topic(&self) -> Option<&str> {
        self.builtins.iter().find_map(|a| match a {
            RpcAnnotation::DdsReplyTopic(t) => Some(t.as_str()),
            _ => None,
        })
    }

    /// Spec §7.3.1.4: `true` if `@RPCRequestType` is set.
    #[must_use]
    pub fn is_rpc_request_type(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::RpcRequestType))
    }

    /// Spec §7.3.1.4: `true` if `@RPCReplyType` is set.
    #[must_use]
    pub fn is_rpc_reply_type(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::RpcReplyType))
    }

    /// Spec §7.5.1.2.1.4: `true` if `@Mutable` is set.
    #[must_use]
    pub fn is_mutable(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, RpcAnnotation::Mutable))
    }
}

/// The simple name of an annotation written with a scoped name, with the
/// keyword escape off.
///
/// `@ann::mark` names `mark`. `@_in` names `in`: IDL 4.2 §7.2.3.2 makes
/// the leading underscore "a purely lexical convention that ONLY turns
/// off keyword checking", so it is not part of the identifier — and
/// `in`, `out` and `inout` are keywords, so `@in` cannot be written any
/// other way. The frontend stores the spelling as written, escape
/// included, which is why the escape comes off here rather than there.
fn name_tail(name: &str) -> &str {
    let tail = name.rsplit("::").next().unwrap_or(name);
    tail.strip_prefix('_').unwrap_or(tail)
}

/// The text of an argument expression.
///
/// The model holds a literal in evaluated form — `Literal::String`
/// already has its escapes resolved and its quotes removed — so this no
/// longer strips quotes off a raw slice, which is what the previous
/// version did and what made `@service(name = "\"X\"")` and
/// `@service(name = "X")` produce the same string.
fn const_to_string(ast: &Ast, id: Id<ConstExprNode>) -> Option<String> {
    use zerodds_ast::Literal;
    let node = ast.expr(id).ok()?;
    let ConstExpr::Literal(literal) = &node.expr else {
        return None;
    };
    match literal {
        Literal::String(s) => Some(s.clone()),
        Literal::Integer(v) => Some(v.to_string()),
        Literal::Boolean(v) => Some(v.to_string()),
        Literal::Char(c) => Some(c.to_string()),
        _ => None,
    }
}

/// The argument written `key = …`.
fn named_argument<'a>(arguments: &'a [AppliedArgument], key: &str) -> Option<&'a AppliedArgument> {
    arguments
        .iter()
        .find(|argument| argument.name.as_deref() == Some(key))
}

/// The argument written without a name, which addresses the
/// declaration's `value` member — IDL 4.2 §8.2.
fn positional_argument(arguments: &[AppliedArgument]) -> Option<&AppliedArgument> {
    arguments.iter().find(|argument| argument.name.is_none())
}

/// The string of `@ann("x")` or `@ann(key = "x")`, in that order of
/// preference.
fn string_argument(ast: &Ast, arguments: &[AppliedArgument], key: &str) -> Option<String> {
    positional_argument(arguments)
        .or_else(|| named_argument(arguments, key))
        .and_then(|argument| const_to_string(ast, argument.value))
}

/// Lowers `@service`'s arguments into the service-name override.
///
/// IDL 4.2 §8.3.4 declares `@service` with a `platform` member and
/// DDS-RPC 1.0 §7.3 reads a service name from the same annotation, so
/// only the explicit `name = "…"` form is read as a name here: a bare
/// `@service("DDS")` names a platform, and treating it as a service name
/// would rename every service that declares one.
#[must_use]
pub fn lower_service_annotation(ast: &Ast, arguments: &[AppliedArgument]) -> RpcAnnotation {
    let name = named_argument(arguments, "name").and_then(|a| const_to_string(ast, a.value));
    RpcAnnotation::Service { name }
}

/// Lower a single applied annotation into its RPC-typed form.
/// Returns `None` if it is not an RPC builtin (the caller then stores it
/// in `LoweredRpc::custom`).
#[must_use]
pub fn lower_single(ast: &Ast, ann: &AppliedAnnotation) -> Option<RpcAnnotation> {
    match name_tail(&ann.name) {
        "service" => Some(lower_service_annotation(ast, &ann.arguments)),
        "oneway" => Some(RpcAnnotation::Oneway),
        "in" => Some(RpcAnnotation::In),
        "out" => Some(RpcAnnotation::Out),
        "inout" => Some(RpcAnnotation::InOut),
        "RPCInterfaceQos" => Some(RpcAnnotation::InterfaceQos {
            profile_ref: string_argument(ast, &ann.arguments, "profile")?,
        }),
        // §7.10.1: `@Qos` with the two URL attributes.
        "Qos" => Some(RpcAnnotation::Qos {
            request_profile: string_argument(ast, &ann.arguments, "RequestProfile"),
            reply_profile: string_argument(ast, &ann.arguments, "ReplyProfile"),
        }),
        "DDSRequestTopic" => Some(RpcAnnotation::DdsRequestTopic(string_argument(
            ast,
            &ann.arguments,
            "name",
        )?)),
        "DDSReplyTopic" => Some(RpcAnnotation::DdsReplyTopic(string_argument(
            ast,
            &ann.arguments,
            "name",
        )?)),
        "RPCRequestType" => Some(RpcAnnotation::RpcRequestType),
        "RPCReplyType" => Some(RpcAnnotation::RpcReplyType),
        "Mutable" => Some(RpcAnnotation::Mutable),
        _ => None,
    }
}

/// Lower a list of applied annotations into the RPC builtin model.
#[must_use]
pub fn lower_applied_annotations(ast: &Ast, anns: &[AppliedAnnotation]) -> LoweredRpc {
    let mut out = LoweredRpc::default();
    for a in anns {
        match lower_single(ast, a) {
            Some(b) => out.builtins.push(b),
            None => out.custom.push(a.clone()),
        }
    }
    out
}

/// Lower everything a node carries: the typed `@service` fields plus the
/// annotations that had no typed field.
///
/// This is the entry point a caller holding a model node uses.
/// `@service` is read from [`zerodds_ast::CorbaFacet`] because that is
/// where the frontend puts it; the rest come from
/// [`Facets::annotations`].
#[must_use]
pub fn lower_rpc_annotations(ast: &Ast, facets: &Facets) -> LoweredRpc {
    let mut out = lower_applied_annotations(ast, &facets.annotations);
    if let Some(corba) = facets.corba.as_ref()
        && corba.is_service
    {
        out.builtins
            .insert(0, lower_service_annotation(ast, &corba.service_arguments));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Parse, lower, resolve — the frontend the annotations arrive
    /// through.
    ///
    /// The previous version of this module built `Annotation` values by
    /// hand, which meant every test agreed with itself about what the
    /// frontend produces and none of them could disagree with it. These
    /// run real IDL through `zerodds-idl-parse` and `zerodds-idl-lower`,
    /// so where an annotation lands is measured rather than assumed.
    fn model(idl: &str) -> Ast {
        let parsed = zerodds_idl_parse::parse(idl);
        assert!(
            parsed.diagnostics().iter().all(|d| !d.is_error()),
            "the source does not parse: {:?}",
            parsed.diagnostics()
        );
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "rpc_annotations.idl");
        assert!(
            lowered.diagnostics.iter().all(|d| !d.is_error()),
            "the source does not lower: {:?}",
            lowered.diagnostics
        );
        let mut ast = lowered.ast;
        let report = ast.resolve_all_by_name().expect("a resolve pass");
        assert!(
            report.broken.is_empty(),
            "every name must resolve: {:?}",
            report.broken
        );
        ast
    }

    /// The declarations of the RPC annotations that no specification the
    /// frontend knows declares. Without them the uses below resolve to
    /// nothing and the resolve pass reports them broken.
    const RPC_ANNOTATION_DECLS: &str = r#"
        @annotation RPCInterfaceQos { string profile; };
        @annotation DDSRequestTopic { string name; };
        @annotation DDSReplyTopic { string name; };
        @annotation RPCRequestType { boolean value default TRUE; };
        @annotation RPCReplyType { boolean value default TRUE; };
    "#;

    fn interface_facets<'a>(ast: &'a Ast, name: &str) -> &'a Facets {
        let (_, node) = ast
            .interfaces()
            .find(|(_, node)| node.name == name)
            .unwrap_or_else(|| panic!("no interface named {name}"));
        &node.facets
    }

    fn lowered_for(idl: &str, interface: &str) -> (Ast, LoweredRpc) {
        let ast = model(idl);
        let lowered = lower_rpc_annotations(&ast, interface_facets(&ast, interface));
        (ast, lowered)
    }

    #[test]
    fn service_no_args_lowers_without_name() {
        let (_ast, lowered) = lowered_for("@service interface Calc { void ping(); };", "Calc");
        assert!(lowered.is_service());
        assert_eq!(lowered.service_name(), None);
        assert_eq!(
            lowered.builtins.first(),
            Some(&RpcAnnotation::Service { name: None })
        );
    }

    #[test]
    fn service_named_arg_lowers_with_name() {
        let (_ast, lowered) = lowered_for(
            r#"@service(name = "Calculator") interface Calc { void ping(); };"#,
            "Calc",
        );
        assert_eq!(lowered.service_name(), Some("Calculator"));
    }

    #[test]
    fn service_positional_arg_is_a_platform_and_not_a_name() {
        // IDL 4.2 §8.3.4 declares `@service` with a `platform` member, so
        // the positional form names a platform. The previous version read
        // it as a service name, which renamed every service that declared
        // the platform it runs on.
        let (_ast, lowered) = lowered_for(
            r#"@service("DDS") interface Calc { void ping(); };"#,
            "Calc",
        );
        assert!(lowered.is_service());
        assert_eq!(lowered.service_name(), None);
    }

    #[test]
    fn service_named_arg_with_unknown_key_yields_none_name() {
        let (_ast, lowered) = lowered_for(
            r#"@service(platform = "DDS") interface Calc { void ping(); };"#,
            "Calc",
        );
        assert!(lowered.is_service());
        assert_eq!(lowered.service_name(), None);
    }

    #[test]
    fn oneway_keyword_reaches_the_operation_flag_not_the_annotation_list() {
        // `@oneway` and the `oneway` keyword set the same field on the
        // operation, so an interface's `LoweredRpc` never carries the
        // annotation. `lower_service` reads the flag.
        let ast = model("interface Log { oneway void write(in string msg); };");
        let (_, iface) = ast.interfaces().next().expect("an interface");
        let op = ast.operation(iface.operations[0]).expect("an operation");
        assert!(op.oneway);
        assert!(!lower_rpc_annotations(&ast, &op.facets).has_oneway());
    }

    #[test]
    fn unknown_annotation_lands_in_custom() {
        let (_ast, lowered) = lowered_for(
            r"@annotation vendor_thing { boolean value default TRUE; };
              @vendor_thing interface Calc { void ping(); };",
            "Calc",
        );
        assert!(lowered.builtins.is_empty());
        assert_eq!(lowered.custom.len(), 1);
        assert_eq!(lowered.custom[0].name, "vendor_thing");
    }

    #[test]
    fn lower_rpc_annotations_splits_builtins_and_custom() {
        let idl = alloc::format!(
            r#"{RPC_ANNOTATION_DECLS}
               @annotation vendor_thing {{ boolean value default TRUE; }};
               @service
               @RPCInterfaceQos(profile = "Lib1::Reliable")
               @vendor_thing
               interface Calc {{ void ping(); }};"#
        );
        let (_ast, lowered) = lowered_for(&idl, "Calc");
        assert_eq!(lowered.builtins.len(), 2);
        assert_eq!(lowered.custom.len(), 1);
        assert!(lowered.is_service());
        assert_eq!(lowered.interface_qos_profile(), Some("Lib1::Reliable"));
    }

    // ---- §7.10.1 @RPCInterfaceQos -----------------------------------

    #[test]
    fn rpc_interface_qos_with_named_profile_lowers() {
        let idl = alloc::format!(
            r#"{RPC_ANNOTATION_DECLS}
               @RPCInterfaceQos(profile = "Lib1::Reliable")
               interface Calc {{ void ping(); }};"#
        );
        let (_ast, lowered) = lowered_for(&idl, "Calc");
        assert_eq!(
            lowered.builtins.first(),
            Some(&RpcAnnotation::InterfaceQos {
                profile_ref: "Lib1::Reliable".into()
            })
        );
    }

    #[test]
    fn rpc_interface_qos_positional_arg_lowers() {
        let idl = alloc::format!(
            r#"{RPC_ANNOTATION_DECLS}
               @RPCInterfaceQos("Lib::P") interface Calc {{ void ping(); }};"#
        );
        let (_ast, lowered) = lowered_for(&idl, "Calc");
        assert_eq!(lowered.interface_qos_profile(), Some("Lib::P"));
    }

    // ---- §7.4.2.2 @DDSRequestTopic / @DDSReplyTopic ----------------

    #[test]
    fn dds_request_and_reply_topics_lower_and_resolve() {
        let idl = alloc::format!(
            r#"{RPC_ANNOTATION_DECLS}
               @DDSRequestTopic("MyReqTopic")
               @DDSReplyTopic(name = "MyRepTopic")
               interface Calc {{ void ping(); }};"#
        );
        let (_ast, lowered) = lowered_for(&idl, "Calc");
        assert_eq!(lowered.dds_request_topic(), Some("MyReqTopic"));
        assert_eq!(lowered.dds_reply_topic(), Some("MyRepTopic"));
    }

    // ---- §7.3.1.4 Pair of Types: @RPCRequestType / @RPCReplyType ----
    #[test]
    fn the_pair_annotations_lower_and_resolve() {
        let idl = alloc::format!(
            r"{RPC_ANNOTATION_DECLS}
              @RPCRequestType @RPCReplyType
              interface Calc {{ void ping(); }};"
        );
        let (_ast, lowered) = lowered_for(&idl, "Calc");
        assert!(lowered.is_rpc_request_type());
        assert!(lowered.is_rpc_reply_type());
    }

    #[test]
    fn the_spec_annotations_need_no_declaration() {
        // DDS-RPC 1.0 §7.3.1.4, §7.4.2.2, §7.10.1: `@DDSRequestTopic`,
        // `@DDSReplyTopic` and `@Qos` are defined by the specification, so
        // a file uses them without declaring them. Before they were
        // builtins the resolve pass reported every use broken, which is
        // why the tests had to carry `RPC_ANNOTATION_DECLS`.
        let idl = r#"
            @service
            @DDSRequestTopic("RobotRequestTopic")
            @DDSReplyTopic(name = "RobotReplyTopic")
            @Qos(RequestProfile = "file:///qos.xml#req", ReplyProfile = "file:///qos.xml#rep")
            interface RobotControl { void command(in long c); };
        "#;
        let (_ast, lowered) = lowered_for(idl, "RobotControl");
        assert_eq!(lowered.dds_request_topic(), Some("RobotRequestTopic"));
        assert_eq!(lowered.dds_reply_topic(), Some("RobotReplyTopic"));
        assert_eq!(
            lowered.qos_profiles(),
            Some(("file:///qos.xml#req", Some("file:///qos.xml#rep")))
        );
    }

    #[test]
    fn a_string_literal_keeps_its_escapes_resolved_and_its_quotes_off() {
        // The model stores `Literal::String` evaluated. The previous
        // version stripped one leading and one trailing quote off the raw
        // slice, so a literal containing a quote came out with it still
        // attached at one end.
        let idl = alloc::format!(
            r#"{RPC_ANNOTATION_DECLS}
               @DDSRequestTopic("a\"b") interface Calc {{ void ping(); }};"#
        );
        let (_ast, lowered) = lowered_for(&idl, "Calc");
        assert_eq!(lowered.dds_request_topic(), Some("a\"b"));
    }
}
