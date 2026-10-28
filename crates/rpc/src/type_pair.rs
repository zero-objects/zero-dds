// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A service defined by a pair of types (Spec §7.3.1.4).
//!
//! "A service definition may simply include a pair of types, which may be
//! the same. The request and reply types shall be marked as such using
//! the `@RPCRequestType` and `@RPCReplyType` annotations. Only struct
//! types shall be marked as request/reply types."
//!
//! "In the Basic Service Mapping, the types annotated as
//! `@RPCRequestType` shall have a member named `header` of type
//! `dds::rpc::RequestHeader` and the types annotated as `@RPCReplyType`
//! shall have a member named `header` of type `dds::rpc::RequestHeader`
//! […] When using the Enhanced Service Mapping, the annotations are not
//! necessary and the special data members must not be defined."
//!
//! [`lower_type_pairs`] reads such pairs out of a model. The topic names
//! come from `@DDSRequestTopic` / `@DDSReplyTopic` on the types
//! (§7.4.2.2) or from §7.4.1's request-reply style, which captures no
//! `<interface_name>`.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;

use crate::annotations::lower_rpc_annotations;
use crate::error::{RpcError, RpcResult};
use crate::topic_naming::{
    BindingStyle, DEFAULT_SERVICE_NAME, ServiceTopicNames, TopicNameOverrides,
};

/// The type of the `header` member of a reply type.
///
/// §7.3.1.4 prints `dds::rpc::RequestHeader` for the reply type as well
/// as for the request type; §7.5.1.1.1 defines `ReplyHeader` for the reply
/// and `RequestHeader` has no `related_request_id` to correlate it with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplyHeaderType {
    /// `dds::rpc::ReplyHeader`, the type §7.5.1.1.1 gives the reply
    /// header. The default.
    #[default]
    ReplyHeader,
    /// `dds::rpc::RequestHeader`, as the sentence of §7.3.1.4 reads
    /// literally.
    RequestHeader,
}

/// Settings of [`lower_type_pairs`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypePairOptions {
    /// The type the `header` member of the reply type must have.
    pub reply_header: ReplyHeaderType,
    /// What `ServiceParams`/`ClientParams`/`RequesterParams`/`ReplierParams`
    /// give at run time: the `<service_name>` and the topic-name suffixes
    /// (§7.4.2.3). They take precedence over the annotations on the types
    /// and over the default.
    pub overrides: TopicNameOverrides,
}

/// Which mapping the pair is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairMapping {
    /// Both types carry the `header` member (§7.3.1.4, Basic).
    Basic,
    /// Neither type does (§7.3.1.4, Enhanced).
    Enhanced,
}

/// A service defined by a pair of types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypePairService {
    /// The request type, a structure.
    pub request: Id<TypeNode>,
    /// The reply type; the same as [`Self::request`] where one type
    /// carries both annotations.
    pub reply: Id<TypeNode>,
    /// The scoped IDL name of the request type (`::`-separated).
    pub request_name: String,
    /// The scoped IDL name of the reply type.
    pub reply_name: String,
    /// The mapping the pair is written for.
    pub mapping: PairMapping,
    /// The topic names after §7.4.2.2's annotations and §7.4.1's
    /// request-reply style.
    pub topics: ServiceTopicNames,
}

fn scope_of(ast: &Ast, node: &TypeNode) -> RpcResult<Vec<String>> {
    let mut chain: Vec<String> = Vec::new();
    let mut current = Some(node.module);
    let mut depth = 0usize;
    while let Some(id) = current {
        if depth > 64 {
            return Err(RpcError::StaleHandle { what: "module" });
        }
        depth += 1;
        let module = ast
            .module(id)
            .map_err(|_| RpcError::StaleHandle { what: "module" })?;
        if !module.name.is_empty() {
            chain.push(module.name.clone());
        }
        current = module.parent;
    }
    chain.reverse();
    Ok(chain)
}

fn scoped_name(ast: &Ast, node: &TypeNode) -> RpcResult<String> {
    let mut chain = scope_of(ast, node)?;
    chain.push(node.name.clone());
    Ok(chain.join("::"))
}

/// The scoped name of the type of member `header` of structure `node`,
/// or `None` when it has none.
fn header_type(ast: &Ast, node: &TypeNode) -> RpcResult<Option<String>> {
    let TypeKind::Structure { members, .. } = &node.kind else {
        return Ok(None);
    };
    for member in members {
        let member = ast
            .member(*member)
            .map_err(|_| RpcError::StaleHandle { what: "member" })?;
        if member.name != "header" {
            continue;
        }
        let id = member.type_.resolved().ok_or(RpcError::StaleHandle {
            what: "header member type",
        })?;
        let target = ast.type_(id).map_err(|_| RpcError::StaleHandle {
            what: "header member type",
        })?;
        return Ok(Some(scoped_name(ast, target)?));
    }
    Ok(None)
}

fn invalid(module: &str, reason: impl Into<String>) -> RpcError {
    RpcError::InvalidTypePair {
        module: module.to_string(),
        reason: reason.into(),
    }
}

/// Every service the model defines by a pair of types.
///
/// The types marked in one module form that module's pair: one
/// `@RPCRequestType` and one `@RPCReplyType` (one type may carry both).
///
/// # Errors
/// `RpcError::InvalidTypePair` for a marked type that is not a
/// structure, a module with more than one request type or more than one
/// reply type, one half without the other, a pair of which only one type
/// has a `header` member, or a `header` of the wrong type.
pub fn lower_type_pairs(ast: &Ast, options: &TypePairOptions) -> RpcResult<Vec<TypePairService>> {
    #[derive(Default)]
    struct Marked {
        requests: Vec<Id<TypeNode>>,
        replies: Vec<Id<TypeNode>>,
    }
    let mut by_module: Vec<(String, Marked)> = Vec::new();
    for (id, node) in ast.types() {
        let lowered = lower_rpc_annotations(ast, &node.facets);
        let (is_request, is_reply) = (lowered.is_rpc_request_type(), lowered.is_rpc_reply_type());
        if !is_request && !is_reply {
            continue;
        }
        let module = scope_of(ast, node)?.join("::");
        if !matches!(node.kind, TypeKind::Structure { .. }) {
            return Err(invalid(
                &module,
                alloc::format!(
                    "{} is not a struct; only struct types are request/reply types",
                    node.name
                ),
            ));
        }
        let slot = match by_module.iter().position(|(name, _)| *name == module) {
            Some(index) => index,
            None => {
                by_module.push((module.clone(), Marked::default()));
                by_module.len() - 1
            }
        };
        if is_request {
            by_module[slot].1.requests.push(id);
        }
        if is_reply {
            by_module[slot].1.replies.push(id);
        }
    }

    let mut out = Vec::new();
    for (module, marked) in by_module {
        let [request] = marked.requests.as_slice() else {
            return Err(invalid(
                &module,
                alloc::format!(
                    "{} types are marked @RPCRequestType; a pair has one",
                    marked.requests.len()
                ),
            ));
        };
        let [reply] = marked.replies.as_slice() else {
            return Err(invalid(
                &module,
                alloc::format!(
                    "{} types are marked @RPCReplyType; a pair has one",
                    marked.replies.len()
                ),
            ));
        };
        let request_node = ast.type_(*request).map_err(|_| RpcError::StaleHandle {
            what: "request type",
        })?;
        let reply_node = ast
            .type_(*reply)
            .map_err(|_| RpcError::StaleHandle { what: "reply type" })?;

        let request_header = header_type(ast, request_node)?;
        let reply_header = header_type(ast, reply_node)?;
        let mapping = match (&request_header, &reply_header) {
            (Some(request_type), Some(reply_type)) => {
                if request_type != "dds::rpc::RequestHeader" {
                    return Err(invalid(
                        &module,
                        alloc::format!(
                            "the header of {} is {request_type}, not dds::rpc::RequestHeader",
                            request_node.name
                        ),
                    ));
                }
                let wanted = match options.reply_header {
                    ReplyHeaderType::ReplyHeader => "dds::rpc::ReplyHeader",
                    ReplyHeaderType::RequestHeader => "dds::rpc::RequestHeader",
                };
                if reply_type != wanted {
                    return Err(invalid(
                        &module,
                        alloc::format!(
                            "the header of {} is {reply_type}, not {wanted}",
                            reply_node.name
                        ),
                    ));
                }
                PairMapping::Basic
            }
            (None, None) => PairMapping::Enhanced,
            _ => {
                return Err(invalid(
                    &module,
                    "only one of the two types has a `header` member; the Basic Service Mapping \
                     needs it in both and the Enhanced Service Mapping in neither",
                ));
            }
        };

        // §7.4.2.2: the topic annotations on the request and reply type.
        let request_annotations = lower_rpc_annotations(ast, &request_node.facets);
        let reply_annotations = lower_rpc_annotations(ast, &reply_node.facets);
        // §7.4.1: the `<service_name>` is "user-supplied"; the annotation
        // may stand on either type, and the two may not name two services.
        let service_name = match (
            request_annotations.service_name(),
            reply_annotations.service_name(),
        ) {
            (Some(request), Some(reply)) if request != reply => {
                return Err(invalid(
                    &module,
                    alloc::format!(
                        "the request type names the service {request:?} and the reply type {reply:?}"
                    ),
                ));
            }
            (Some(name), _) | (None, Some(name)) => name,
            (None, None) => DEFAULT_SERVICE_NAME,
        };
        let topics = ServiceTopicNames::synthesize(
            BindingStyle::RequestReply,
            &request_node.name,
            &[],
            service_name,
            request_annotations.dds_request_topic(),
            reply_annotations.dds_reply_topic(),
            &options.overrides,
        )?;
        out.push(TypePairService {
            request: *request,
            reply: *reply,
            request_name: scoped_name(ast, request_node)?,
            reply_name: scoped_name(ast, reply_node)?,
            mapping,
            topics,
        });
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn model(idl: &str) -> Ast {
        let parsed = zerodds_idl_parse::parse(idl);
        assert!(
            parsed.diagnostics().iter().all(|d| !d.is_error()),
            "{:?}",
            parsed.diagnostics()
        );
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "type_pair.idl");
        assert!(
            lowered.diagnostics.iter().all(|d| !d.is_error()),
            "{:?}",
            lowered.diagnostics
        );
        let mut ast = lowered.ast;
        let report = ast.resolve_all_by_name().expect("a resolve pass");
        assert!(report.broken.is_empty(), "{:?}", report.broken);
        ast
    }

    const HEADERS: &str = "module dds { module rpc {
        struct RequestHeader { long id; };
        struct ReplyHeader { long id; };
    }; };";

    fn with_headers(body: &str) -> Ast {
        model(&alloc::format!("{HEADERS}\n{body}"))
    }

    /// §7.3.1.4 Basic: a request type and a reply type, each with a
    /// `header`, form a service.
    #[test]
    fn a_marked_pair_with_headers_is_a_basic_service() {
        let ast = with_headers(
            "module robot {
               @RPCRequestType struct Req { dds::rpc::RequestHeader header; long a; };
               @RPCReplyType struct Rep { dds::rpc::ReplyHeader header; long r; };
             };",
        );
        let pairs = lower_type_pairs(&ast, &TypePairOptions::default()).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].mapping, PairMapping::Basic);
        assert_eq!(pairs[0].request_name, "robot::Req");
        assert_eq!(pairs[0].reply_name, "robot::Rep");
        assert_ne!(pairs[0].request, pairs[0].reply);
        // §7.4.1: the request-reply style captures no interface name.
        assert_eq!(pairs[0].topics.request, "Service_Request");
        assert_eq!(pairs[0].topics.reply, "Service_Reply");
    }

    /// "A pair of types, which may be the same."
    #[test]
    fn one_type_may_be_both_halves() {
        let ast = with_headers(
            "@RPCRequestType @RPCReplyType struct Both { dds::rpc::RequestHeader header; long a; };",
        );
        // The one type has one header, of the request type: the reply
        // reading of the type is `RequestHeader` literally.
        let options = TypePairOptions {
            reply_header: ReplyHeaderType::RequestHeader,
            ..TypePairOptions::default()
        };
        let pairs = lower_type_pairs(&ast, &options).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].request, pairs[0].reply);
    }

    /// §7.3.1.4 Enhanced: "the annotations are not necessary and the
    /// special data members must not be defined."
    #[test]
    fn a_marked_pair_without_headers_is_an_enhanced_service() {
        let ast = model(
            "@RPCRequestType struct Req { long a; };
             @RPCReplyType struct Rep { long r; };",
        );
        let pairs = lower_type_pairs(&ast, &TypePairOptions::default()).unwrap();
        assert_eq!(pairs[0].mapping, PairMapping::Enhanced);
    }

    #[test]
    fn a_header_in_only_one_type_is_refused() {
        let ast = with_headers(
            "@RPCRequestType struct Req { dds::rpc::RequestHeader header; long a; };
             @RPCReplyType struct Rep { long r; };",
        );
        let err = lower_type_pairs(&ast, &TypePairOptions::default()).unwrap_err();
        assert!(matches!(err, RpcError::InvalidTypePair { .. }), "{err:?}");
    }

    #[test]
    fn a_header_of_the_wrong_type_is_refused() {
        let ast = with_headers(
            "@RPCRequestType struct Req { dds::rpc::ReplyHeader header; long a; };
             @RPCReplyType struct Rep { dds::rpc::ReplyHeader header; long r; };",
        );
        let err = lower_type_pairs(&ast, &TypePairOptions::default()).unwrap_err();
        assert!(
            matches!(&err, RpcError::InvalidTypePair { reason, .. } if reason.contains("RequestHeader")),
            "{err:?}"
        );
    }

    /// The sentence of §7.3.1.4 that prints `RequestHeader` for the reply
    /// is the named alternative.
    #[test]
    fn the_reply_header_type_is_a_setting() {
        let ast = with_headers(
            "@RPCRequestType struct Req { dds::rpc::RequestHeader header; long a; };
             @RPCReplyType struct Rep { dds::rpc::RequestHeader header; long r; };",
        );
        assert!(lower_type_pairs(&ast, &TypePairOptions::default()).is_err());
        let literal = TypePairOptions {
            reply_header: ReplyHeaderType::RequestHeader,
            ..TypePairOptions::default()
        };
        assert_eq!(lower_type_pairs(&ast, &literal).unwrap().len(), 1);
    }

    /// "Only struct types shall be marked as request/reply types."
    #[test]
    fn a_marked_type_that_is_not_a_struct_is_refused() {
        let ast = model("@RPCRequestType enum E { A, B };");
        let err = lower_type_pairs(&ast, &TypePairOptions::default()).unwrap_err();
        assert!(
            matches!(&err, RpcError::InvalidTypePair { reason, .. } if reason.contains("not a struct")),
            "{err:?}"
        );
    }

    #[test]
    fn a_request_type_without_a_reply_type_is_refused() {
        let ast = model("@RPCRequestType struct Req { long a; };");
        assert!(lower_type_pairs(&ast, &TypePairOptions::default()).is_err());
    }

    /// §7.4.2.2: the topic annotations may stand on the types.
    #[test]
    fn the_topic_annotations_on_the_types_name_the_topics() {
        let ast = model(
            r#"@RPCRequestType @DDSRequestTopic(name="RobotRequestTopic") struct Req { long a; };
               @RPCReplyType @DDSReplyTopic(name="RobotReplyTopic") struct Rep { long r; };"#,
        );
        let pairs = lower_type_pairs(&ast, &TypePairOptions::default()).unwrap();
        assert_eq!(pairs[0].topics.request, "RobotRequestTopic");
        assert_eq!(pairs[0].topics.reply, "RobotReplyTopic");
    }

    #[test]
    fn an_idl_without_marked_types_defines_no_pair() {
        let ast = model("struct Plain { long a; };");
        assert!(
            lower_type_pairs(&ast, &TypePairOptions::default())
                .unwrap()
                .is_empty()
        );
    }

    /// §7.4.1: `<service_name>` is user-supplied; `@service(name=…)` and the
    /// `@DDSService(name=…)` of the specification's examples both give it.
    #[test]
    fn the_service_name_comes_from_either_annotation_on_either_type() {
        for (request, reply) in [
            ("@service(name=\"Move\")", ""),
            ("@DDSService(name=\"Move\")", ""),
            ("", "@DDSService(name=\"Move\")"),
            ("@service(name=\"Move\")", "@DDSService(name=\"Move\")"),
        ] {
            let ast = model(&alloc::format!(
                "@RPCRequestType {request} struct Req {{ long a; }};
                 @RPCReplyType {reply} struct Rep {{ long r; }};"
            ));
            let pairs = lower_type_pairs(&ast, &TypePairOptions::default()).unwrap();
            assert_eq!(pairs[0].topics.service, "Move", "{request} {reply}");
            assert_eq!(pairs[0].topics.request, "Move_Request");
            assert_eq!(pairs[0].topics.reply, "Move_Reply");
        }
    }

    #[test]
    fn two_names_for_the_service_are_refused() {
        let ast = model(
            "@RPCRequestType @service(name=\"A\") struct Req { long a; };
             @RPCReplyType @DDSService(name=\"B\") struct Rep { long r; };",
        );
        assert!(matches!(
            lower_type_pairs(&ast, &TypePairOptions::default()),
            Err(RpcError::InvalidTypePair { .. })
        ));
    }

    /// §7.4.2.3: what the params give at run time wins over the annotation,
    /// which wins over the default; a topic name written out wins over the
    /// service name (§7.4.1).
    #[test]
    fn the_run_time_service_name_takes_precedence() {
        let ast = model(
            "@RPCRequestType @DDSService(name=\"Move\") struct Req { long a; };
             @RPCReplyType struct Rep { long r; };",
        );
        let options = TypePairOptions {
            overrides: TopicNameOverrides::with_service_name("Live"),
            ..TypePairOptions::default()
        };
        let pairs = lower_type_pairs(&ast, &options).unwrap();
        assert_eq!(pairs[0].topics.request, "Live_Request");
        assert_eq!(pairs[0].topics.reply, "Live_Reply");
    }
}
