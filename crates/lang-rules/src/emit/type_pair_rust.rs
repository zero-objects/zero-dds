// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A DDS-RPC 1.0 service defined by a pair of IDL types (§7.3.1.4), as a
//! Rust binding: the request type and the reply type become the topic
//! types of a `zerodds_rpc` `Requester` and `Replier`.
//!
//! "The request and reply types shall be marked as such using the
//! `@RPCRequestType` and `@RPCReplyType` annotations. Only struct types
//! shall be marked." In the Basic Service Mapping both types have a member
//! `header` (`dds::rpc::RequestHeader` and `dds::rpc::ReplyHeader`,
//! §7.5.1.1.1); in the Enhanced one neither does.
//!
//! What is written beside the structures and their codecs
//! (`crate::encode`):
//!
//! * `impl DdsType` for each type, named in DDS by its scoped IDL name;
//! * Basic: `RequestWithHeader` and `ReplyWithHeader`
//!   (`zerodds_rpc::header_pair`), which put the request's identity into
//!   the header the type carries;
//! * `Req::requester(participant, qos)` and `Req::replier(participant, qos,
//!   handler)`, the endpoints with the topic names the annotations
//!   (§7.4.2.2) or §7.4.1's request-reply style give.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use zerodds_ast::{Ast, Literal as AstLiteral, ModuleNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::rust;

use crate::adapter::cst::rust_type_of;
use crate::emit::interface_rust::{dds_type_impl, type_closed, xcdr1_capable_types};

/// The annotation `name` on a declaration, with a leading underscore (the
/// keyword escape of IDL 4.2 §7.2.3.2) and a scope taken off.
fn annotation<'a>(
    facets: &'a zerodds_ast::Facets,
    name: &str,
) -> Option<&'a zerodds_ast::AppliedAnnotation> {
    facets.annotations.iter().find(|candidate| {
        let tail = candidate
            .name
            .rsplit("::")
            .next()
            .unwrap_or(&candidate.name);
        tail.strip_prefix('_').unwrap_or(tail) == name
    })
}

/// The string argument of `@name(key = "…")`, or the positional one.
///
/// # Errors
/// The annotation has an argument and it is not a string literal: a topic
/// name that is not a string names no topic, and ignoring it would leave
/// the endpoints on the default names.
pub(crate) fn string_argument(
    ast: &Ast,
    facets: &zerodds_ast::Facets,
    name: &str,
    key: &str,
) -> Result<Option<String>, String> {
    let Some(applied) = annotation(facets, name) else {
        return Ok(None);
    };
    let Some(argument) = applied
        .arguments
        .iter()
        .find(|a| a.name.is_none())
        .or_else(|| {
            applied
                .arguments
                .iter()
                .find(|a| a.name.as_deref() == Some(key))
        })
    else {
        return Ok(None);
    };
    let expr = ast
        .expr(argument.value)
        .map_err(|_| format!("@{name}: a stale argument"))?;
    match &expr.expr {
        zerodds_ast::ConstExpr::Literal(AstLiteral::String(text)) => Ok(Some(text.clone())),
        _ => Err(format!(
            "@{name}({key}): the argument is not a string literal"
        )),
    }
}

/// The `<service_name>` a type gives (§7.4.1): `@service(name = "…")` of IDL
/// 4.2 or `@DDSService(name = "…")` of DDS-RPC 1.0's examples, both of which
/// the lowering keeps on the declaration's CORBA facet.
///
/// # Errors
/// The `name` argument is not a string literal.
pub(crate) fn service_name_of(
    ast: &Ast,
    facets: &zerodds_ast::Facets,
) -> Result<Option<String>, String> {
    let Some(corba) = facets.corba.as_ref().filter(|corba| corba.is_service) else {
        return Ok(None);
    };
    let Some(argument) = corba
        .service_arguments
        .iter()
        .find(|argument| argument.name.as_deref() == Some("name"))
    else {
        return Ok(None);
    };
    let expr = ast
        .expr(argument.value)
        .map_err(|_| "@service: a stale argument".to_string())?;
    match &expr.expr {
        zerodds_ast::ConstExpr::Literal(AstLiteral::String(text)) => Ok(Some(text.clone())),
        _ => Err("@service(name): the argument is not a string literal".to_string()),
    }
}

pub(crate) fn scope_names(ast: &Ast, module: Id<ModuleNode>) -> Result<Vec<String>, String> {
    let mut chain = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
    while let Some(id) = current {
        depth += 1;
        if depth > 64 {
            return Err("the module chain is too deep".to_string());
        }
        let node = ast
            .module(id)
            .map_err(|_| "a stale module handle".to_string())?;
        if !node.name.is_empty() {
            chain.push(node.name.clone());
        }
        current = node.parent;
    }
    chain.reverse();
    Ok(chain)
}

fn is_marked(node: &TypeNode, marker: &str) -> bool {
    annotation(&node.facets, marker).is_some()
}

/// Whether the model has a type marked as a request or reply type.
#[must_use]
pub fn has_pair_types(ast: &Ast) -> bool {
    ast.types()
        .any(|(_, node)| is_marked(node, "RPCRequestType") || is_marked(node, "RPCReplyType"))
}

/// One pair, as the generator reads it.
pub(crate) struct Pair {
    pub(crate) module: Id<ModuleNode>,
    pub(crate) scope: Vec<String>,
    pub(crate) request: Id<TypeNode>,
    pub(crate) reply: Id<TypeNode>,
    /// Both types carry `header` (Basic) or neither (Enhanced).
    pub(crate) basic: bool,
}

/// The types marked in one module: the module, the request types and the
/// reply types.
type Marked = (Id<ModuleNode>, Vec<Id<TypeNode>>, Vec<Id<TypeNode>>);

pub(crate) fn collect(ast: &Ast) -> Result<Vec<Pair>, String> {
    let mut by_module: Vec<Marked> = Vec::new();
    for (id, node) in ast.types() {
        let (request, reply) = (
            is_marked(node, "RPCRequestType"),
            is_marked(node, "RPCReplyType"),
        );
        if !request && !reply {
            continue;
        }
        if !matches!(node.kind, TypeKind::Structure { .. }) {
            return Err(format!(
                "{} is marked as a request/reply type and is not a struct (DDS-RPC 1.0 §7.3.1.4)",
                node.name
            ));
        }
        let slot = match by_module.iter().position(|(m, ..)| *m == node.module) {
            Some(i) => i,
            None => {
                by_module.push((node.module, Vec::new(), Vec::new()));
                by_module.len() - 1
            }
        };
        if request {
            by_module[slot].1.push(id);
        }
        if reply {
            by_module[slot].2.push(id);
        }
    }
    let mut out = Vec::new();
    for (module, requests, replies) in by_module {
        let ([request], [reply]) = (requests.as_slice(), replies.as_slice()) else {
            return Err(format!(
                "a module with {} @RPCRequestType and {} @RPCReplyType types: a pair has one of each",
                requests.len(),
                replies.len()
            ));
        };
        let has_header = |id: Id<TypeNode>| -> Result<bool, String> {
            let node = ast
                .type_(id)
                .map_err(|_| "a stale type handle".to_string())?;
            let TypeKind::Structure { members, .. } = &node.kind else {
                return Ok(false);
            };
            Ok(members
                .iter()
                .any(|m| ast.member(*m).is_ok_and(|m| m.name == "header")))
        };
        let (request_header, reply_header) = (has_header(*request)?, has_header(*reply)?);
        if request_header != reply_header {
            return Err("only one of the two types has a `header` member; the Basic Service Mapping needs it in both and the Enhanced one in neither (DDS-RPC 1.0 §7.3.1.4)".to_string());
        }
        out.push(Pair {
            module,
            scope: scope_names(ast, module)?,
            request: *request,
            reply: *reply,
            basic: request_header,
        });
    }
    Ok(out)
}

fn header_type_text(ast: &Ast, type_: Id<TypeNode>) -> Result<String, String> {
    let node = ast
        .type_(type_)
        .map_err(|_| "a stale type handle".to_string())?;
    let TypeKind::Structure { members, .. } = &node.kind else {
        return Err("not a structure".to_string());
    };
    for member in members {
        let member = ast
            .member(*member)
            .map_err(|_| "a stale member".to_string())?;
        if member.name == "header" {
            let target = member.type_.resolved();
            let ty = rust_type_of(ast, node.module, target)
                .map_err(|error| format!("the type of `header`: {error:?}"))?;
            return Ok(zerodds_lang_render::rust::type_(&ty));
        }
    }
    Err("no `header` member".to_string())
}

fn scoped(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{}::{name}", scope.join("::"))
    }
}

fn quoted(text: &str) -> String {
    format!("{text:?}")
}

fn option_text(value: Option<&str>) -> String {
    value.map_or_else(
        || "None".to_string(),
        |text| format!("Some({}.to_string())", quoted(text)),
    )
}

/// The Rust items of every pair in `ast`, each with the module it belongs
/// in.
///
/// # Errors
/// A description of what in the pair is not a service definition of
/// §7.3.1.4.
pub fn rust_type_pair_items(ast: &Ast) -> Result<Vec<(Id<ModuleNode>, rust::Item)>, String> {
    let pairs = collect(ast)?;
    if pairs.is_empty() {
        return Ok(Vec::new());
    }
    let capable = xcdr1_capable_types(ast);
    let mut out = Vec::new();
    for pair in pairs {
        let request = ast.type_(pair.request).map_err(|_| "stale".to_string())?;
        let reply = ast.type_(pair.reply).map_err(|_| "stale".to_string())?;
        let mut seen = vec![pair.request];
        if pair.reply != pair.request {
            seen.push(pair.reply);
        }
        for id in seen {
            let node = ast.type_(id).map_err(|_| "stale".to_string())?;
            let ok = type_closed(ast, id, &capable, &mut BTreeSet::new());
            out.push((
                pair.module,
                dds_type_impl(&node.name, &scoped(&pair.scope, &node.name), ok),
            ));
        }
        out.push((
            pair.module,
            rust::Item::Verbatim(glue(ast, &pair, request, reply)?),
        ));
    }
    Ok(out)
}

fn glue(ast: &Ast, pair: &Pair, request: &TypeNode, reply: &TypeNode) -> Result<String, String> {
    let (req, rep) = (request.name.as_str(), reply.name.as_str());
    let mut text = String::new();
    if pair.basic {
        let request_header = header_type_text(ast, pair.request)?;
        let _ = write!(
            text,
            "
impl zerodds_rpc::header_pair::RequestWithHeader for {req} {{
    fn with_request_header(
        &self,
        header: &zerodds_rpc::common_types::RequestHeader,
    ) -> zerodds_rpc::error::RpcResult<Self> {{
        let bytes = header.to_cdr_le()?;
        let mut reader = zerodds_wire::Reader::with_max_align(&bytes, zerodds_wire::Endianness::Little, 8);
        let decoded = <{request_header}>::decode_xcdr1(&mut reader)
            .map_err(|error| zerodds_rpc::error::RpcError::Codec(error.to_string()))?;
        let mut out = self.clone();
        out.header = decoded;
        Ok(out)
    }}
}}
"
        );
        if pair.reply != pair.request {
            let reply_header = header_type_text(ast, pair.reply)?;
            let _ = write!(
                text,
                "
impl zerodds_rpc::header_pair::ReplyWithHeader for {rep} {{
    fn with_reply_header(
        &self,
        header: &zerodds_rpc::common_types::ReplyHeader,
    ) -> zerodds_rpc::error::RpcResult<Self> {{
        let bytes = header.to_cdr_le();
        let mut reader = zerodds_wire::Reader::with_max_align(&bytes, zerodds_wire::Endianness::Little, 8);
        let decoded = <{reply_header}>::decode_xcdr1(&mut reader)
            .map_err(|error| zerodds_rpc::error::RpcError::Codec(error.to_string()))?;
        let mut out = self.clone();
        out.header = decoded;
        Ok(out)
    }}
}}
"
            );
        }
    }
    let request_topic = string_argument(ast, &request.facets, "DDSRequestTopic", "name")?;
    let reply_topic = string_argument(ast, &reply.facets, "DDSReplyTopic", "name")?;
    // §7.4.1: the annotation may stand on either type; two names for one
    // service are an error.
    let service_name = match (
        service_name_of(ast, &request.facets)?,
        service_name_of(ast, &reply.facets)?,
    ) {
        (Some(a), Some(b)) if a != b => {
            return Err(format!(
                "the request type names the service {a:?} and the reply type {b:?}"
            ));
        }
        (Some(name), _) | (None, Some(name)) => Some(name),
        (None, None) => None,
    };
    let mapping = if pair.basic { "Basic" } else { "Enhanced" };
    let (request_dds, reply_dds) = (scoped(&pair.scope, req), scoped(&pair.scope, rep));
    let construct_requester = if pair.basic {
        "zerodds_rpc::requester::Requester::with_pair"
    } else {
        "zerodds_rpc::requester::Requester::with_options"
    };
    let construct_replier = if pair.basic {
        "zerodds_rpc::replier::Replier::with_pair"
    } else {
        "zerodds_rpc::replier::Replier::with_options"
    };
    let _ = write!(
        text,
        "
impl {req} {{
    /// The topic names: `@DDSRequestTopic` and `@DDSReplyTopic` on the
    /// types (DDS-RPC 1.0 §7.4.2.2), else the request-reply style of §7.4.1.
    fn pair_topics(
        overrides: zerodds_rpc::topic_naming::TopicNameOverrides,
    ) -> zerodds_rpc::topic_naming::AnnotatedTopics {{
        zerodds_rpc::topic_naming::AnnotatedTopics {{
            service_name: {service},
            request_topic: {request_topic},
            reply_topic: {reply_topic},
            overrides,
        }}
    }}

    /// The DDS type names of the topics are the scoped names of the two
    /// types (§7.3.1.4: they are the topic types).
    fn pair_options() -> zerodds_rpc::endpoint_options::RpcEndpointOptions {{
        zerodds_rpc::endpoint_options::RpcEndpointOptions {{
            mapping: zerodds_rpc::endpoint_options::RpcMapping::{mapping},
            type_names: Some(zerodds_rpc::endpoint_options::TopicTypeNames {{
                request: {request_dds}.to_string(),
                reply: {reply_dds}.to_string(),
            }}),
            ..zerodds_rpc::endpoint_options::RpcEndpointOptions::default()
        }}
    }}

    /// A requester of this pair of types (DDS-RPC 1.0 §7.3.1.4).
    pub fn requester(
        participant: &zerodds_dcps::participant::DomainParticipant,
        qos: &zerodds_rpc::qos_profile::RpcQos,
    ) -> zerodds_rpc::error::RpcResult<zerodds_rpc::requester::Requester<{req}, {rep}>> {{
        Self::requester_with(
            participant,
            qos,
            zerodds_rpc::topic_naming::TopicNameOverrides::none(),
        )
    }}

    /// [`Self::requester`] with the service name and the topic-name
    /// suffixes given at run time, as `RequesterParams` does (DDS-RPC 1.0
    /// §7.4.2.3): they take precedence over the annotations.
    pub fn requester_with(
        participant: &zerodds_dcps::participant::DomainParticipant,
        qos: &zerodds_rpc::qos_profile::RpcQos,
        overrides: zerodds_rpc::topic_naming::TopicNameOverrides,
    ) -> zerodds_rpc::error::RpcResult<zerodds_rpc::requester::Requester<{req}, {rep}>> {{
        {construct_requester}(
            participant,
            {name},
            \"\",
            qos,
            zerodds_rpc::topic_naming::BindingStyle::RequestReply,
            &Self::pair_topics(overrides),
            &Self::pair_options(),
        )
    }}

    /// A replier of this pair of types, answering through `handler`.
    pub fn replier(
        participant: &zerodds_dcps::participant::DomainParticipant,
        qos: &zerodds_rpc::qos_profile::RpcQos,
        handler: std::sync::Arc<dyn zerodds_rpc::replier::ReplierHandler<{req}, {rep}>>,
    ) -> zerodds_rpc::error::RpcResult<zerodds_rpc::replier::Replier<{req}, {rep}>> {{
        Self::replier_with(
            participant,
            qos,
            zerodds_rpc::topic_naming::TopicNameOverrides::none(),
            handler,
        )
    }}

    /// [`Self::replier`] with the service name and the topic-name suffixes
    /// given at run time, as `ReplierParams` does (§7.4.2.3).
    pub fn replier_with(
        participant: &zerodds_dcps::participant::DomainParticipant,
        qos: &zerodds_rpc::qos_profile::RpcQos,
        overrides: zerodds_rpc::topic_naming::TopicNameOverrides,
        handler: std::sync::Arc<dyn zerodds_rpc::replier::ReplierHandler<{req}, {rep}>>,
    ) -> zerodds_rpc::error::RpcResult<zerodds_rpc::replier::Replier<{req}, {rep}>> {{
        {construct_replier}(
            participant,
            {name},
            \"\",
            qos,
            zerodds_rpc::topic_naming::BindingStyle::RequestReply,
            &Self::pair_topics(overrides),
            &Self::pair_options(),
            handler,
        )
    }}
}}
",
        service = option_text(service_name.as_deref()),
        request_topic = option_text(request_topic.as_deref()),
        reply_topic = option_text(reply_topic.as_deref()),
        request_dds = quoted(&request_dds),
        reply_dds = quoted(&reply_dds),
        name = quoted(req),
    );
    Ok(text)
}
