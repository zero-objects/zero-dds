# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-rpc` crate.

### Spec references

- **OMG DDS-RPC 1.0** (`formal/16-12-04`):
  - §7.3 IDL annotations (`@service`, `@oneway`, `@in`, `@out`, `@inout`).
  - §7.4 service mapping (IDL → ServiceDef/MethodDef/ParamDef).
  - §7.5 common types (RequestHeader, ReplyHeader, SampleIdentity, RemoteExceptionCode_t) + member hash (§7.5.4).
  - §7.6 evolution rules + compatibility mappings.
  - §7.7 function_call / dispatch_request.
  - §7.8 topic naming, request identity, discovery extensions.
  - §7.9 requester API.
  - §7.10 replier API.
  - §7.11 QoS profile (foundation + enhanced).
- Coverage status: `docs/spec-coverage/dds-rpc-1.0.md` — 94 done / 0 partial / 0 open / 10 n/a.

### Public API

**Foundation:**
- `RequestHeader`, `ReplyHeader`, `SampleIdentity`, `RemoteExceptionCode`, `MAX_HEADER_BYTES`, `MAX_STRING_LEN`.
- `topic_naming::{request_topic_name, reply_topic_name, validate_service_name, REQUEST_SUFFIX, REPLY_SUFFIX, ServiceTopicNames}`.
- `annotations::{LoweredRpc, RpcAnnotation, lower_rpc_annotations}`.
- `service_mapping::{ServiceDef, MethodDef, ParamDef, ParamDirection, TypeRef, lower_service}`.
- `codegen::{ServiceLayout, RequestType, ReplyType, MethodPair, CallUnionDef, CallUnionCase, MemberType, StructMember, build_basic_pair, build_enhanced_pair, build_enhanced_all}`.
- `rpc_hash::rpc_member_hash`.

**Runtime:**
- `Requester<TIn, TOut>::{new, with_instance, send_request_blocking, send_request_async, tick}`.
- `Replier<TIn, TOut>::{new, with_instance, tick}` + `ReplierHandler` trait + `FnHandler` adapter.
- `RpcEndpointBuilder`, `RequesterEndpoint`, `ReplierEndpoint`.
- `RpcQos::{default_basic, default_enhanced, from_xml_profile, request_writer_qos, request_reader_qos, reply_writer_qos, reply_reader_qos}`.
- Constants: `DEFAULT_BASIC_HISTORY_DEPTH = 10`, `DEFAULT_ENHANCED_HISTORY_DEPTH = 64`, `DEFAULT_RESOURCE_LIMITS`.
- `wire_codec::{encode_request_frame, decode_request_frame, encode_reply_frame, decode_reply_frame}`.

**Cross-Cutting:**
- `discovery_ext::{PublicationBuiltinTopicDataExt, SubscriptionBuiltinTopicDataExt, ServiceMappingProfile, client_matches_service, service_matches_client}`.
- `function_call::{FunctionStub, FunctionSkeleton, OperationDescriptor, ServiceDescriptor, dispatch_request}`.
- `evolution_rules::{Evolution, Mapping, compatible_evolutions, is_compatible}`.
- `request_identity::RequestIdentity`.

**Errors:** `RpcError`, `RpcResult`.

### Implementation

Common types are encoded via XCDR2-Final (Spec §7.5.1.1) and match byte-exactly with the RTI/Cyclone-DDS reply wire. `SampleIdentity` (16-byte writer GUID + 8-byte sequence number) is the correlation token between request and reply — the replier sets it in `ReplyHeader::related_request_id` and the requester routes the response via an `mpsc::Sender` pending slot.

`Requester` is synchronous + tick-driven: `send_request_blocking` calls `tick` in a polling loop until reply or timeout. Callers with their own event loop can use `send_request_async`, which only sends the request and returns an `mpsc::Receiver`.

`Replier` carries a `ReplierHandler` trait — apps provide a closure (`FnHandler::new(|req| reply)`) or their own trait impl. `dispatch_request` (Spec §7.7) makes the function-call path from the IDL type to the handler method.

`RpcQos` brings two foundation defaults (`default_basic` with KeepLast(10), `default_enhanced` with KeepLast(64)) and an XML profile resolver — profiles under `library::profile` are merged with the defaults, so that policies not specified in the XML fall back to the spec default.

A process-global instance registry prevents duplicates of `(participant, role, service, instance)` (Spec §7.6.2). Anonymous instances (`instance_name = ""`) allow multiple registration as the default instance.

`Codegen` produces basic + enhanced request/reply struct pairs along with the `CallUnion` discriminator type for the server-side match (Spec §7.5.1.3).

`forbid(unsafe_code)` is set.

### Architecture

- **Layer:** 4 (core services).
- **Dependencies (in):** `zerodds-dcps`, `zerodds-idl`, `zerodds-qos`, `zerodds-rtps`, `zerodds-types`, `zerodds-xml`.
- **Dependents (out):** end-user RPC apps, `crates/rmw-zerodds-shim` (ROS-2 service path), bridges (`grpc-bridge` does its own wire paths).
- **Feature flags:** `std` (default), `alloc` (via std), `safety` (reserve hook).

### Stability

Public API + wire format RC1-stable. Major bumps for breaking changes of the spec wire form.
