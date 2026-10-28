# `zerodds-rpc`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-rpc/badge.svg)](https://docs.rs/zerodds-rpc)

Request/reply framework on the [ZeroDDS](https://zerodds.org) DCPS stack
per **OMG DDS-RPC 1.0** (`formal/2017-04-01`). Pure-Rust + `alloc`.
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
- `codegen::{ServiceLayout, RequestType, ReplyType, EnhancedService, EnhancedStruct, CallUnionDef, CallUnionCase, MemberType, StructMember, build_basic_pair, build_enhanced_service}`.
- `endpoint_options::{RpcEndpointOptions, RpcMapping, TopicTypeNames, ServiceMatching, ClientMatching}` — the mapping, DDS type names and Enhanced discovery rules a `Requester`/`Replier` is built with (`with_options`).
- `enhanced::{encode_remote_ex_reply, decode_remote_ex_reply}` — the `remoteEx` member of an Enhanced reply.
- `rpc_hash::rpc_member_hash` — Spec §7.5.1.1.2 member hash.

**Runtime:**
- `Requester<TIn, TOut>` — synchronous client with `send_request_blocking` + tick-driven `send_request_async`.
- `Replier<TIn, TOut>` + `ReplierHandler` trait + `FnHandler` adapter.
- `RpcEndpointBuilder`, `RequesterEndpoint`, `ReplierEndpoint`.
- `RpcQos::{default_basic, default_enhanced, from_xml_profile}` — Spec §7.11 foundation/enhanced + XML profile resolution.
- `wire_codec::{encode_request_frame, decode_request_frame, encode_reply_frame, decode_reply_frame}`.

**Cross-Cutting:**
- `discovery_ext::{PublicationBuiltinTopicDataExt, SubscriptionBuiltinTopicDataExt, service_matching_complete, client_matching_complete}` — Spec §7.6.2.
- `function_call::{FunctionStub, FunctionSkeleton, OperationDescriptor, ServiceDescriptor, dispatch_request}` — Spec §7.5.1.1.
- `evolution_rules::{Evolution, Mapping, compatible_evolutions, is_compatible}` — Spec §7.7.
- `request_identity::RequestIdentity` — Spec §7.8.2 (`related_sample_identity`).

**Errors:** `RpcError` / `RpcResult`.

## Generated Rust clients and services

`zerodds-idlc generate --lang rust` (`crates/lang-rules/src/emit/
interface_rust.rs`) turns an IDL `interface` into code that runs on
this crate directly: the request/reply topic types of §7.5.1.1 with a
real XCDR2 codec (`impl zerodds_dcps::DdsType`), a typed
`<Iface>Client` over [`requester::Requester`], a `<Iface>` service
trait with `raises` in its signatures, and a `<Iface>Dispatcher`
implementing [`replier::ReplierHandler`] that [`replier::Replier::new`]
takes directly. A generated file therefore depends on this crate (and
on `zerodds-dcps` and `zerodds-wire`) — its own preamble says so.

`crates/rpc-rust-e2e` is where this is measured against a real
`DcpsRuntime`: `build.rs` runs the generator, `src/lib.rs` compiles the
result in, and `tests/e2e.rs` runs a client and a service — return
values, `raises`, `oneway`, an attribute's getter/setter, an `inout`
parameter, an unrecognised discriminator, and a `Derived : Base`
hierarchy served one dispatcher per level. `crates/rpc/tests/
cpp_rpc_runtime.rs` is the C++ binding's own such gate; this is Rust's.
`crates/rpc-rust-e2e/tests/wire_bytes_match_cpp.rs` holds the two
columns to the same octets one layer further in than
`cpp_rpc_runtime.rs`'s own header comparison: the generated
`Calculator_Call`/`Calculator_Return` request/reply payload for
`add(3, 4)`, encoded through the real generated Rust types on one side
and assembled by hand from §7.5.1.1.4/§7.5.1.1.5's field order — off
the same generated `Calculator_add_Hash` discriminator both columns
already share — on the other (`crates/rpc-rust-e2e/tests/cpp/
calculator_add_wire.cpp`, since the C++ column's own types have no
codec to call here).

The generated client and dispatcher read `@service(name = "…")`,
`@DDSRequestTopic` and `@DDSReplyTopic` too: [`requester::Requester::
with_topics`] and [`replier::Replier::with_topics`] take a
[`topic_naming::AnnotatedTopics`] — the interface's own three §7.4.2.2
inputs, composed via `topic_naming::ServiceTopicNames::synthesize`
(§7.4.1) — instead of always the default `Service` topic names.
[`requester::Requester::new`]/[`replier::Replier::new`] still take a
bare `<interface_name>` string and compose the default pair, for a
caller (or a generated `Calculator` with no such annotation) that has
nothing else to say; `with_topics` is additive, not a replacement.

## Layer position

Layer 4 — core services. Consumes `zerodds-dcps` (writer/reader/QoS), `zerodds-ast` (the common AST), `zerodds-qos` (DDS QoS policies), `zerodds-rtps` (inline-QoS PIDs), `zerodds-types` (DdsType trait), `zerodds-xml` (profile loader).

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

The crate needs `std`: it is built on `zerodds-dcps`, which needs it. The `std` feature gates the runtime modules (`requester`, `replier`, `qos_profile`, `endpoint`).

## Stability

`1.0.0-rc.7`, a pre-release. The public API can still change before
1.0.0; the crate's CHANGELOG lists every change.

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
- [`zerodds-ast`](../ast) — the common AST the service model is lowered from.
- [`zerodds-xml`](../xml) — DDS-XML profile loader.
