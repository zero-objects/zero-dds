# `zerodds-rpc`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-rpc/badge.svg)](https://docs.rs/zerodds-rpc)

Request/reply framework on the [ZeroDDS](https://zerodds.org) DCPS stack
per **OMG DDS-RPC 1.0** (`formal/16-12-04`). Pure-Rust + `alloc`.
Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS-RPC 1.0 | §7.3 (IDL annotations), §7.4 (service mapping), §7.5 (common types + member hash), §7.6 (evolution rules), §7.7 (function_call/dispatch), §7.8 (topic naming + request identity + discovery ext), §7.9 (requester), §7.10 (replier), §7.11 (QoS profile) |

Full coverage of the spec per `docs/spec-coverage/dds-rpc-1.0.md`.

## What's inside

**Foundation:**
- `RequestHeader`, `ReplyHeader`, `SampleIdentity`, `RemoteExceptionCode` (XCDR2-encoded).
- `topic_naming::{request_topic_name, reply_topic_name, validate_service_name, REQUEST_SUFFIX, REPLY_SUFFIX, ServiceTopicNames}`.
- `annotations::{LoweredRpc, RpcAnnotation, lower_rpc_annotations}` — IDL `@service`/`@oneway`/`@in`/`@out`/`@inout` lowering.
- `service_mapping::{ServiceDef, MethodDef, ParamDef, ParamDirection, TypeRef, lower_service}`.
- `codegen::{ServiceLayout, RequestType, ReplyType, MethodPair, CallUnionDef, CallUnionCase, MemberType, StructMember, build_basic_pair, build_enhanced_pair, build_enhanced_all}`.
- `rpc_hash::rpc_member_hash` — Spec §7.5.4 member hash.

**Runtime:**
- `Requester<TIn, TOut>` — synchronous client with `send_request_blocking` + tick-driven `send_request_async`.
- `Replier<TIn, TOut>` + `ReplierHandler` trait + `FnHandler` adapter.
- `RpcEndpointBuilder`, `RequesterEndpoint`, `ReplierEndpoint`.
- `RpcQos::{default_basic, default_enhanced, from_xml_profile}` — Spec §7.11 foundation/enhanced + XML profile resolution.
- `wire_codec::{encode_request_frame, decode_request_frame, encode_reply_frame, decode_reply_frame}`.

**Cross-Cutting:**
- `discovery_ext::{PublicationBuiltinTopicDataExt, SubscriptionBuiltinTopicDataExt, ServiceMappingProfile, client_matches_service, service_matches_client}` — Spec §7.8.4.
- `function_call::{FunctionStub, FunctionSkeleton, OperationDescriptor, ServiceDescriptor, dispatch_request}` — Spec §7.7.
- `evolution_rules::{Evolution, Mapping, compatible_evolutions, is_compatible}` — Spec §7.6.5.
- `request_identity::RequestIdentity` — Spec §7.8.2.

**Errors:** `RpcError` / `RpcResult`.

## Layer position

Layer 4 — core services. Consumes `zerodds-dcps` (writer/reader/QoS), `zerodds-idl` (IDL AST), `zerodds-qos` (DDS QoS policies), `zerodds-rtps` (inline-QoS PIDs), `zerodds-types` (DdsType trait), `zerodds-xml` (profile loader).

## Quickstart

```rust,ignore
use zerodds_rpc::{Requester, Replier, RpcQos, FnHandler};
use zerodds_dcps::participant::DomainParticipant;

let participant = DomainParticipant::new(0)?;
let qos = RpcQos::default_basic();

// Replier: service implementation
let replier = Replier::<MyRequest, MyReply>::new(
    &participant, "Calculator", &qos,
    FnHandler::new(|req: MyRequest| MyReply { sum: req.a + req.b }),
)?;

// Requester: client
let requester = Requester::<MyRequest, MyReply>::new(&participant, "Calculator", &qos)?;
let reply = requester.send_request_blocking(&MyRequest { a: 1, b: 2 })?;
assert_eq!(reply.sum, 3);
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Threading + `Mutex`/`mpsc` for `Requester`/`Replier`. |
| `alloc` | ✅ via std | `Vec`/`String`. |
| `safety` | ❌ | Reserve hook for extra defensive checks. |

Foundation-layer modules also build in `no_std + alloc`. Runtime modules (`requester`, `replier`, `qos_profile`, `endpoint`) need `std`.

## Stability

`1.0.0-rc.1` fully materializes the spec coverage of OMG DDS-RPC 1.0.
Public API + wire format are RC1-stable.

## Tests

```bash
cargo test -p zerodds-rpc
```

180 tests green (171 lib + 5 + 4 integration).

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- `docs/spec-coverage/dds-rpc-1.0.md` — spec coverage doc.
- [`zerodds-dcps`](../dcps) — DCPS runtime.
- [`zerodds-idl`](../idl) — IDL AST parser.
- [`zerodds-xml`](../xml) — DDS-XML profile loader.
