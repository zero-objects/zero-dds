# `zerodds-rpc`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-rpc/badge.svg)](https://docs.rs/zerodds-rpc)

Request/Reply-Framework auf dem [ZeroDDS](https://zerodds.org)-DCPS-Stack
nach **OMG DDS-RPC 1.0** (`formal/16-12-04`). Pure-Rust + `alloc`.
Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG DDS-RPC 1.0 | §7.3 (IDL-Annotations), §7.4 (Service-Mapping), §7.5 (Common-Types + Member-Hash), §7.6 (Evolution-Rules), §7.7 (function_call/dispatch), §7.8 (Topic-Naming + Request-Identity + Discovery-Ext), §7.9 (Requester), §7.10 (Replier), §7.11 (QoS-Profile) |

Vollabdeckung der Spec laut `docs/spec-coverage/dds-rpc-1.0.md`.

## Was ist drin

**Foundation:**
- `RequestHeader`, `ReplyHeader`, `SampleIdentity`, `RemoteExceptionCode` (XCDR2-encoded).
- `topic_naming::{request_topic_name, reply_topic_name, validate_service_name, REQUEST_SUFFIX, REPLY_SUFFIX, ServiceTopicNames}`.
- `annotations::{LoweredRpc, RpcAnnotation, lower_rpc_annotations}` — IDL `@service`/`@oneway`/`@in`/`@out`/`@inout`-Lowering.
- `service_mapping::{ServiceDef, MethodDef, ParamDef, ParamDirection, TypeRef, lower_service}`.
- `codegen::{ServiceLayout, RequestType, ReplyType, MethodPair, CallUnionDef, CallUnionCase, MemberType, StructMember, build_basic_pair, build_enhanced_pair, build_enhanced_all}`.
- `rpc_hash::rpc_member_hash` — Spec §7.5.4 Member-Hash.

**Runtime:**
- `Requester<TIn, TOut>` — synchroner Client mit `send_request_blocking` + tick-driven `send_request_async`.
- `Replier<TIn, TOut>` + `ReplierHandler`-Trait + `FnHandler`-Adapter.
- `RpcEndpointBuilder`, `RequesterEndpoint`, `ReplierEndpoint`.
- `RpcQos::{default_basic, default_enhanced, from_xml_profile}` — Spec §7.11 Foundation/Enhanced + XML-Profile-Resolution.
- `wire_codec::{encode_request_frame, decode_request_frame, encode_reply_frame, decode_reply_frame}`.

**Cross-Cutting:**
- `discovery_ext::{PublicationBuiltinTopicDataExt, SubscriptionBuiltinTopicDataExt, ServiceMappingProfile, client_matches_service, service_matches_client}` — Spec §7.8.4.
- `function_call::{FunctionStub, FunctionSkeleton, OperationDescriptor, ServiceDescriptor, dispatch_request}` — Spec §7.7.
- `evolution_rules::{Evolution, Mapping, compatible_evolutions, is_compatible}` — Spec §7.6.5.
- `request_identity::RequestIdentity` — Spec §7.8.2.

**Errors:** `RpcError` / `RpcResult`.

## Schichten-Position

Layer 4 — Core Services. Konsumiert `zerodds-dcps` (Writer/Reader/QoS), `zerodds-idl` (IDL-AST), `zerodds-qos` (DDS-QoS-Policies), `zerodds-rtps` (Inline-QoS-PIDs), `zerodds-types` (DdsType-Trait), `zerodds-xml` (Profile-Loader).

## Quickstart

```rust,ignore
use zerodds_rpc::{Requester, Replier, RpcQos, FnHandler};
use zerodds_dcps::participant::DomainParticipant;

let participant = DomainParticipant::new(0)?;
let qos = RpcQos::default_basic();

// Replier: Service-Implementation
let replier = Replier::<MyRequest, MyReply>::new(
    &participant, "Calculator", &qos,
    FnHandler::new(|req: MyRequest| MyReply { sum: req.a + req.b }),
)?;

// Requester: Client
let requester = Requester::<MyRequest, MyReply>::new(&participant, "Calculator", &qos)?;
let reply = requester.send_request_blocking(&MyRequest { a: 1, b: 2 })?;
assert_eq!(reply.sum, 3);
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Threading + `Mutex`/`mpsc` fuer `Requester`/`Replier`. |
| `alloc` | ✅ via std | `Vec`/`String`. |
| `safety` | ❌ | Reserve-Hook fuer extra Defensive-Checks. |

Foundation-Layer-Module bauen auch in `no_std + alloc`. Runtime-Module (`requester`, `replier`, `qos_profile`, `endpoint`) brauchen `std`.

## Stabilitaet

`1.0.0-rc.1` materialisiert die Spec-Abdeckung von OMG DDS-RPC 1.0
vollstaendig. Public-API + Wire-Format sind RC1-stabil.

## Tests

```bash
cargo test -p zerodds-rpc
```

180 Tests gruen (171 lib + 5 + 4 integration).

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- `docs/spec-coverage/dds-rpc-1.0.md` — Spec-Coverage-Doc.
- [`zerodds-dcps`](../dcps) — DCPS-Runtime.
- [`zerodds-idl`](../idl) — IDL-AST-Parser.
- [`zerodds-xml`](../xml) — DDS-XML-Profile-Loader.
