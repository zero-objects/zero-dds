// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-rpc`. Safety classification: **STANDARD**.
//!
//! DDS-RPC 1.0 (OMG `formal/16-12-04`) request/reply framework on
//! the ZeroDDS DCPS stack.
//!
//! ## Layer position
//!
//! Layer 4 — core services. Builds on `zerodds-dcps` (DataWriter/
//! DataReader) + `zerodds-idl` (IDL AST) + `zerodds-qos` (DDS QoS) +
//! `zerodds-xml` (profile loader).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! **Foundation (Spec §7.3-§7.5):**
//! * [`common_types`] — `RequestHeader`, `ReplyHeader`, `SampleIdentity`,
//!   `RemoteExceptionCode_t` (Spec §7.5.1.1.1) with XCDR2 encode/decode.
//! * [`topic_naming`] — topic naming convention `<Service>_Request` /
//!   `<Service>_Reply` (Spec §7.8.2) plus service name validation.
//! * [`annotations`] — lowering of the RPC IDL annotations `@service`,
//!   `@oneway`, `@in`, `@out`, `@inout` (Spec §7.3).
//! * [`service_mapping`] — data-model lowering of an IDL service
//!   definition to `ServiceDef`/`MethodDef`/`ParamDef` (Spec §7.4).
//! * [`codegen`] — request/reply struct pairs (basic + enhanced) +
//!   `CallUnion` (Spec §7.5.1).
//! * [`rpc_hash`] — `rpc_member_hash` for the Spec §7.5.4 member hash.
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
//! * [`discovery_ext`] — `PublicationBuiltinTopicDataExt` + service-match logic.
//! * [`function_call`] — `FunctionStub` / `FunctionSkeleton` /
//!   `dispatch_request` (Spec §7.7).
//! * [`evolution_rules`] — compatibility mappings (Spec §7.6.5).
//! * [`request_identity`] — `RequestIdentity` (spec-layer wrapper).
//!
//! ## Example
//!
//! ```rust,ignore
//! use zerodds_rpc::{Requester, RpcEndpointBuilder};
//! // ... see README.md for a complete quickstart example.
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::missing_errors_doc)]

extern crate alloc;

pub mod annotations;
pub mod codegen;
pub mod common_types;
pub mod discovery_ext;
pub mod endpoint;
pub mod error;
pub mod evolution_rules;
pub mod function_call;
#[cfg(feature = "std")]
pub mod qos_profile;
#[cfg(feature = "std")]
pub mod replier;
pub mod request_identity;
#[cfg(feature = "std")]
pub mod requester;
pub mod rpc_hash;
pub mod service_mapping;
pub mod topic_naming;
pub mod wire_codec;

pub use annotations::{LoweredRpc, RpcAnnotation, lower_rpc_annotations};
pub use codegen::{
    CallUnionCase, CallUnionDef, MemberType, MethodPair, ReplyType, RequestType, ServiceLayout,
    StructMember, build_basic_pair, build_enhanced_all, build_enhanced_pair,
};
pub use common_types::{
    MAX_HEADER_BYTES, MAX_STRING_LEN, RemoteExceptionCode, ReplyHeader, RequestHeader,
    SampleIdentity,
};
pub use discovery_ext::{
    PublicationBuiltinTopicDataExt, ServiceMappingProfile, SubscriptionBuiltinTopicDataExt,
    client_matches_service, service_matches_client,
};
pub use endpoint::{ReplierEndpoint, RequesterEndpoint, RpcEndpointBuilder};
pub use error::{RpcError, RpcResult};
pub use evolution_rules::{Evolution, Mapping, compatible_evolutions, is_compatible};
pub use function_call::{
    FunctionSkeleton, FunctionStub, OperationDescriptor, ServiceDescriptor, dispatch_request,
};
pub use request_identity::RequestIdentity;
pub use rpc_hash::rpc_member_hash;
pub use service_mapping::{
    MethodDef, ParamDef, ParamDirection, ServiceDef, TypeRef, lower_service,
};
pub use topic_naming::{
    REPLY_SUFFIX, REQUEST_SUFFIX, ServiceTopicNames, reply_topic_name, request_topic_name,
    validate_service_name,
};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // Smoke test: the crate compiles and the test harness runs.
    }
}
