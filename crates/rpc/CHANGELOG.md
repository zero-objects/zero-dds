# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-rpc`-Crate.

### Spec-Referenzen

- **OMG DDS-RPC 1.0** (`formal/16-12-04`):
  - §7.3 IDL-Annotations (`@service`, `@oneway`, `@in`, `@out`, `@inout`).
  - §7.4 Service-Mapping (IDL → ServiceDef/MethodDef/ParamDef).
  - §7.5 Common-Types (RequestHeader, ReplyHeader, SampleIdentity, RemoteExceptionCode_t) + Member-Hash (§7.5.4).
  - §7.6 Evolution-Rules + Compatibility-Mappings.
  - §7.7 function_call / dispatch_request.
  - §7.8 Topic-Naming, Request-Identity, Discovery-Extensions.
  - §7.9 Requester-API.
  - §7.10 Replier-API.
  - §7.11 QoS-Profile (Foundation + Enhanced).
- Coverage-Status: `docs/spec-coverage/dds-rpc-1.0.md` — 94 done / 0 partial / 0 open / 10 n/a.

### Public-API

**Foundation:**
- `RequestHeader`, `ReplyHeader`, `SampleIdentity`, `RemoteExceptionCode`, `MAX_HEADER_BYTES`, `MAX_STRING_LEN`.
- `topic_naming::{request_topic_name, reply_topic_name, validate_service_name, REQUEST_SUFFIX, REPLY_SUFFIX, ServiceTopicNames}`.
- `annotations::{LoweredRpc, RpcAnnotation, lower_rpc_annotations}`.
- `service_mapping::{ServiceDef, MethodDef, ParamDef, ParamDirection, TypeRef, lower_service}`.
- `codegen::{ServiceLayout, RequestType, ReplyType, MethodPair, CallUnionDef, CallUnionCase, MemberType, StructMember, build_basic_pair, build_enhanced_pair, build_enhanced_all}`.
- `rpc_hash::rpc_member_hash`.

**Runtime:**
- `Requester<TIn, TOut>::{new, with_instance, send_request_blocking, send_request_async, tick}`.
- `Replier<TIn, TOut>::{new, with_instance, tick}` + `ReplierHandler`-Trait + `FnHandler`-Adapter.
- `RpcEndpointBuilder`, `RequesterEndpoint`, `ReplierEndpoint`.
- `RpcQos::{default_basic, default_enhanced, from_xml_profile, request_writer_qos, request_reader_qos, reply_writer_qos, reply_reader_qos}`.
- Konstanten: `DEFAULT_BASIC_HISTORY_DEPTH = 10`, `DEFAULT_ENHANCED_HISTORY_DEPTH = 64`, `DEFAULT_RESOURCE_LIMITS`.
- `wire_codec::{encode_request_frame, decode_request_frame, encode_reply_frame, decode_reply_frame}`.

**Cross-Cutting:**
- `discovery_ext::{PublicationBuiltinTopicDataExt, SubscriptionBuiltinTopicDataExt, ServiceMappingProfile, client_matches_service, service_matches_client}`.
- `function_call::{FunctionStub, FunctionSkeleton, OperationDescriptor, ServiceDescriptor, dispatch_request}`.
- `evolution_rules::{Evolution, Mapping, compatible_evolutions, is_compatible}`.
- `request_identity::RequestIdentity`.

**Errors:** `RpcError`, `RpcResult`.

### Implementierung

Common-Types werden via XCDR2-Final encoded (Spec §7.5.1.1) und matched byte-genau mit RTI/Cyclone-DDS-Reply-Wire. `SampleIdentity` (16-byte writer-GUID + 8-byte sequence-number) ist das Korrelations-Token zwischen Request und Reply — der Replier setzt es in `ReplyHeader::related_request_id` und der Requester routet die Antwort ueber einen `mpsc::Sender`-pending-Slot.

`Requester` ist synchron + tick-driven: `send_request_blocking` ruft `tick` in einem Polling-Loop bis Reply oder Timeout. Caller mit eigenem Event-Loop koennen `send_request_async` nutzen, der nur den Request schickt und einen `mpsc::Receiver` zurueckliefert.

`Replier` traegt einen `ReplierHandler`-Trait — Apps liefern eine Closure (`FnHandler::new(|req| reply)`) oder eine eigene Trait-Impl. `dispatch_request` (Spec §7.7) macht den function-call-Pfad vom IDL-Typ zur Handler-Methode.

`RpcQos` bringt zwei Foundation-Defaults (`default_basic` mit KeepLast(10), `default_enhanced` mit KeepLast(64)) und einen XML-Profile-Resolver — Profile unter `library::profile` werden mergt mit den Defaults, sodass nicht im XML angegebene Policies auf Spec-Default fallen.

Eine Process-globale Instance-Registry verhindert Duplikate `(participant, role, service, instance)` (Spec §7.6.2). Anonyme Instanzen (`instance_name = ""`) erlauben Mehrfach-Registrierung als Default-Instance.

`Codegen` produziert Basic + Enhanced Request/Reply-Struct-Pairs samt `CallUnion`-Diskrim-Type fuer den Server-Side-Match (Spec §7.5.1.3).

`forbid(unsafe_code)` ist gesetzt.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-dcps`, `zerodds-idl`, `zerodds-qos`, `zerodds-rtps`, `zerodds-types`, `zerodds-xml`.
- **Dependents (out):** End-User-RPC-Apps, `crates/rmw-zerodds-shim` (ROS-2-Service-Pfad), Bridges (`grpc-bridge` macht eigene Wire-Pfade).
- **Feature-Flags:** `std` (default), `alloc` (via std), `safety` (Reserve-Hook).

### Stabilitaet

Public-API + Wire-Format RC1-stabil. Major-Bumps bei Breaking-Changes der Spec-Wire-Form.
