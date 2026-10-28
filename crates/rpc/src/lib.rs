// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-rpc`. Safety classification: **STANDARD**.
//!
//! DDS-RPC 1.0 (OMG `formal/2017-04-01`) request/reply framework on
//! the ZeroDDS DCPS stack.
//!
//! ## Layer position
//!
//! Layer 4 — core services. Builds on `zerodds-dcps` (DataWriter/
//! DataReader) + `zerodds-ast` (the common AST) + `zerodds-qos` (DDS QoS) +
//! `zerodds-xml` (profile loader).
//!
//! ## Public API
//!
//! **Foundation (Spec §7.3-§7.5):**
//! * [`common_types`] — `RequestHeader`, `ReplyHeader`, `SampleIdentity`,
//!   `RemoteExceptionCode_t` (Spec §7.5.1.1.1) with XCDR2 encode/decode.
//! * [`topic_naming`] — topic naming convention `<Service>_Request` /
//!   `<Service>_Reply` (Spec §7.4.1) plus service name validation.
//! * [`annotations`] — lowering of the RPC IDL annotations `@service`,
//!   `@oneway`, `@in`, `@out`, `@inout` (Spec §7.3).
//! * [`service_mapping`] — data-model lowering of an IDL service
//!   definition to `ServiceDef`/`MethodDef`/`ParamDef` (Spec §7.4).
//! * [`java_service_def`] — a service definition written in Java rather
//!   than in IDL: the modified Java grammar and the `@in`/`@out`/
//!   `@inout`/`@oneway` annotations of Spec §7.3.2, and the pair of
//!   types of §7.3.2.2.
//! * [`language_binding`] — the exception table of Spec §7.11.1.3 and
//!   §7.11.2.1, and the class summaries of §7.11.1.4, §7.11.1.5 and
//!   §7.11.2.2.1.
//! * [`codegen`] — request/reply struct pairs (basic + enhanced) +
//!   `CallUnion` (Spec §7.5.1).
//! * [`rpc_hash`] — `rpc_member_hash` for the HASH of Spec §7.5.1.1.2.
//!
//! **Runtime (Spec §7.9-§7.11):**
//! * [`requester`] — `Requester<TIn, TOut>`: send a request, reply via
//!   `SampleIdentity` correlation, blocking + tick-driven API.
//! * [`replier`] — `Replier<TIn, TOut>` incl. the `ReplierHandler` trait +
//!   `FnHandler` adapter.
//! * [`qos_profile`] — `RpcQos` with spec defaults + XML profile
//!   resolution against `zerodds-xml::DdsXml`.
//! * [`wire_codec`] — encoder/decoder for request and reply frames
//!   (header XCDR2 + user payload).
//! * [`endpoint`] — `RpcEndpointBuilder` as a convenience constructor.
//!
//! **Cross-cutting (Spec §7.6-§7.8):**
//! * [`discovery_ext`] — the §7.6.2.1 discovery extension and the
//!   Enhanced service-matching algorithm (Figures 7.3, 7.4).
//! * [`endpoint_options`] — mapping, topic type names and discovery rules of an
//!   endpoint; [`enhanced`] — the Enhanced mapping's `remoteEx` reply.
//! * [`function_call`] — `FunctionStub` / `FunctionSkeleton` /
//!   `dispatch_request` (Spec §7.2.2.1, §7.9).
//! * [`evolution_rules`] — compatibility of service evolutions (Spec §7.7).
//! * [`request_identity`] — `RequestIdentity` (spec-layer wrapper).
//!
//! ## Example
//!
//! ```rust,ignore
//! use zerodds_rpc::{Requester, RpcEndpointBuilder};
//! // ... see README.md for a complete quickstart example.
//! ```

// Needs `std`: built on zerodds-dcps, which needs it.
#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::missing_errors_doc)]

extern crate alloc;

pub mod annotations;
pub mod codegen;
pub mod common_types;
pub mod discovery_ext;
pub mod endpoint;
pub mod endpoint_options;
pub mod enhanced;
pub mod error;
pub mod evolution_rules;
pub mod function_call;
pub mod header_pair;
pub mod java_service_def;
pub mod language_binding;
#[cfg(feature = "std")]
pub mod qos_profile;
#[cfg(feature = "std")]
pub mod replier;
pub mod request_identity;
#[cfg(feature = "std")]
pub mod requester;
pub mod rpc_hash;
pub mod service_mapping;
#[cfg(test)]
pub(crate) mod test_types;
pub mod topic_naming;
pub mod type_pair;
pub mod wire_codec;

pub use annotations::{LoweredRpc, RpcAnnotation, lower_rpc_annotations};
pub use codegen::{
    CallUnionCase, CallUnionDef, DUMMY_MEMBER, EmptyOperationStyle, EnhancedMember,
    EnhancedOperation, EnhancedOptions, EnhancedService, EnhancedStruct, ExceptionCase, MemberType,
    REMOTE_EX_MEMBER, RESULT_MEMBER, RETCODE_OK, RETURN_MEMBER, ReplyType, RequestType,
    ResultUnionDef, ServiceLayout, StructMember, UNKNOWN_OP_MEMBER, build_basic_pair,
    build_enhanced_service, build_enhanced_service_with,
};
pub use common_types::{
    MAX_HEADER_BYTES, MAX_STRING_LEN, PLACEHOLDER_OCTETS, RemoteExceptionCode, ReplyHeader,
    RequestHeader, SampleIdentity, UnknownException, UnknownOperation, UnusedMember,
};
pub use discovery_ext::{
    GUID_UNKNOWN, PublicationBuiltinTopicDataExt, SubscriptionBuiltinTopicDataExt,
    client_matching_complete, service_matching_complete,
};
pub use endpoint::{ReplierEndpoint, RequesterEndpoint, RpcEndpointBuilder};
pub use error::{RpcError, RpcResult};
pub use evolution_rules::{Evolution, Mapping, compatible_evolutions, is_compatible};
pub use function_call::{
    FunctionSkeleton, FunctionStub, OperationDescriptor, ServiceDescriptor, dispatch_request,
};
pub use java_service_def::{
    JavaAnnotation, JavaClass, JavaDecl, JavaField, JavaInterface, JavaMethod, JavaParam,
    JavaServiceDef, parse as parse_java_service_def, request_reply_pair,
};
pub use language_binding::{
    CPP_FUNCTION_CALL_CLASSES, CPP_NAMESPACES, CPP_REQUEST_REPLY_CLASSES, JAVA_FUNCTION_CALL_TYPES,
    JAVA_PACKAGES, JAVA_REQUEST_REPLY_TYPES, RemoteExceptionClass, cpp_exception_class,
    java_exception_class,
};
pub use request_identity::RequestIdentity;
pub use rpc_hash::rpc_member_hash;
pub use service_mapping::{
    MethodDef, ParamDef, ParamDirection, RaisedException, ServiceDef, TypeRef, lower_service,
};
pub use topic_naming::{
    BindingStyle, DEFAULT_SERVICE_NAME, REPLY_SUFFIX, REQUEST_SUFFIX, ServiceTopicNames,
    TopicNameOverrides, compose_topic_name, qualified_interface_name, reply_topic_name,
    request_topic_name, validate_service_name, validate_user_def_alpha_num,
};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // Smoke test: the crate compiles and the test harness runs.
    }
}
