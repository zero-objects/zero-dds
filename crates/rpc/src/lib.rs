// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-rpc`. Safety classification: **STANDARD**.
//!
//! DDS-RPC 1.0 (OMG `formal/16-12-04`) Request/Reply-Framework auf
//! dem ZeroDDS-DCPS-Stack.
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Baut auf `zerodds-dcps` (DataWriter/
//! DataReader) + `zerodds-idl` (IDL-AST) + `zerodds-qos` (DDS-QoS) +
//! `zerodds-xml` (Profile-Loader).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! **Foundation (Spec §7.3-§7.5):**
//! * [`common_types`] — `RequestHeader`, `ReplyHeader`, `SampleIdentity`,
//!   `RemoteExceptionCode_t` (Spec §7.5.1.1.1) mit XCDR2-Encode/Decode.
//! * [`topic_naming`] — Topic-Naming-Konvention `<Service>_Request` /
//!   `<Service>_Reply` (Spec §7.8.2) plus Service-Name-Validation.
//! * [`annotations`] — Lowering der RPC-IDL-Annotations `@service`,
//!   `@oneway`, `@in`, `@out`, `@inout` (Spec §7.3).
//! * [`service_mapping`] — Datenmodell-Lowering einer IDL-Service-
//!   Definition zu `ServiceDef`/`MethodDef`/`ParamDef` (Spec §7.4).
//! * [`codegen`] — Request/Reply-Struct-Pairs (Basic + Enhanced) +
//!   `CallUnion` (Spec §7.5.1).
//! * [`rpc_hash`] — `rpc_member_hash` fuer Spec-§7.5.4 Member-Hash.
//!
//! **Runtime (Spec §7.9-§7.11):**
//! * [`requester`] — `Requester<TIn, TOut>`: Request senden, Reply via
//!   `SampleIdentity`-Korrelation, blocking + tick-driven API.
//! * [`replier`] — `Replier<TIn, TOut>` inkl. `ReplierHandler`-Trait +
//!   `FnHandler`-Adapter.
//! * [`qos_profile`] — `RpcQos` mit Spec-Defaults + XML-Profile-
//!   Resolution gegen `zerodds-xml::DdsXml`.
//! * [`wire_codec`] — Encoder/Decoder fuer Request- und Reply-Frames
//!   (Header XCDR2 + User-Payload).
//! * [`endpoint`] — `RpcEndpointBuilder` als Convenience-Konstruktor.
//!
//! **Cross-Cutting (Spec §7.6-§7.8):**
//! * [`discovery_ext`] — `PublicationBuiltinTopicDataExt` + Service-Match-Logik.
//! * [`function_call`] — `FunctionStub` / `FunctionSkeleton` /
//!   `dispatch_request` (Spec §7.7).
//! * [`evolution_rules`] — Compatibility-Mappings (Spec §7.6.5).
//! * [`request_identity`] — `RequestIdentity` (Spec-Layer-Wrapper).
//!
//! ## Beispiel
//!
//! ```rust,ignore
//! use zerodds_rpc::{Requester, RpcEndpointBuilder};
//! // ... siehe README.md fuer ein vollstaendiges Quickstart-Beispiel.
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
        // Smoke-Test: Crate kompiliert und Testharness laeuft.
    }
}
