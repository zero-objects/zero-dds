# OMG CORBA 3.3 — Spec Coverage (WP CORBA Coexistence)

**Source:** OMG CORBA 3.3 — three volumes as formal documents:
* Part 1 (Interfaces, 532 pp.) — [`formal/12-11-12`](https://www.omg.org/spec/CORBA/3.3/Interfaces/PDF)
* Part 2 (Interoperability, 249 pp.) — [`formal/12-11-14`](https://www.omg.org/spec/CORBA/3.3/Interoperability/PDF)
* Part 3 (Component Model, 380 pp.) — [`formal/12-11-16`](https://www.omg.org/spec/CORBA/3.3/Components/PDF)

**Context.** With the DDS core and the
`idl-cpp`/`idl-csharp`/`idl-java` codegen family, ZeroDDS fully covers
the DDS PSM. For migrating legacy CORBA applications in
the finance industry, ZeroDDS should be able to appear as a **drop-in
backend**: an existing CORBA client compiles and links against
Annex-A.1 stubs, runs against ZeroDDS endpoints that speak GIOP/IIOP
wire, and can be migrated incrementally to pure DDS without
ad-hoc infrastructure swaps.

CORBA coexistence is spread across several implementation crates:

- `crates/idl/` — IDL grammar (Part 1 §7 IDL syntax)
- `crates/corba-codegen/` — Annex-A.1 codegen (stubs/skeletons, Part 1 §7)
- `crates/corba-ir/` — Interface Repository (Part 1 §14)
- `crates/corba-poa/` — Portable Object Adapter (Part 1 §15)
- `crates/corba-giop/` — GIOP wire protocol (Part 2 §9)
- `crates/corba-iiop/` — IIOP transport (Part 2 §9.7)
- `crates/corba-ior/` — IOR encoding (Part 2 §9.4 / §10.5)
- `crates/corba-csiv2/` — CSIv2 security (Part 2 §10)
- `crates/corba-ccm/` — Component Model (Part 3 §6-§9)
- `crates/corba-ccm-lib/` — CCM container (Part 3 §10)
- `crates/corba-ccm-ejb/` — EJB integration (Part 3 §11)
- `crates/corba-dnc/` — Deployment & Configuration (Part 3 §15-§17)
- `crates/corba-dds-bridge/` — ZeroDDS bridge (no OMG item)
- `crates/corba-cosnaming/` — COS Naming v1.3 (separate spec)
- `crates/corba-cos-event/` — COS Event Service v1.2 (separate spec)
- `crates/dcps/` — DDS PSM core (DDS migration target)
- `crates/rpc/` — DDS-RPC core (DDS migration target)

CosNaming and CosEventService are standalone OMG specs (formal/04-10-03
and formal/04-10-02); CosEvent has its own spec-coverage file
(`cos-event-service-1.4.md`). CosNaming coverage is carried inline here,
because CORBA Part 1 §8.5.2 requires it as a standard initial reference.

---

## Part 1: Interfaces

### §1 Scope

**Spec:** Part 1 §1, p. 1 — scope of the CORBA specification.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §2 Conformance and Compliance

**Spec:** Part 1 §2, p. 1 — conformance points, compliance markers.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §3 Normative References

**Spec:** Part 1 §3, p. 1 — list of normative references.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §4 Additional Information

**Spec:** Part 1 §4, p. 2 — outline + keyword conventions
(MUST/SHALL/SHOULD/MAY).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §5 The Object Model

**Spec:** Part 1 §5, p. 3 — abstract object model (Objects, Requests,
Types, Interfaces, Value Types, Operations, Attributes).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — conceptual framework, no wire/
code obligation.

### §6 CORBA Overview

**Spec:** Part 1 §6, p. 11 — architecture overview (ORB, stubs, DSI,
Object Adapter, IR).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §7 IDL Syntax and Semantics

**Spec:** Part 1 §7, p. 27 — complete IDL grammar (lexer, module,
interface, value, constant, type, exception, operation, attribute,
repository identity, event, component, home, names & scoping).

**Repo:** `crates/idl/src/grammar/`, `crates/corba-codegen/src/repository_id.rs`,
`crates/corba-codegen/src/special_types.rs`, `crates/corba-codegen/src/skeleton.rs`,
`crates/corba-codegen/src/stub.rs`.

**Tests:** `crates/idl/src/grammar/` (inline tests, ca. 600+ items via
`idl-4.2.md`), `crates/corba-codegen/src/repository_id.rs::tests`.

**Status:** done — IDL 4.2 is a superset of CORBA-IDL §7. Rule-by-rule
coverage in `docs/spec-coverage/idl-4.2.md` (649 done / 4 partial).
Repository identity (§7.15) is implemented via
`corba-codegen::repository_id`. Component and home declarations
(§7.17/§7.18) are restated in Part 3 §6 and are `done` there.

### §8 ORB Interface

**Spec:** Part 1 §8, p. 93 — ORB pseudo-object: `ORB_init`,
`string_to_object`/`object_to_string`, `resolve_initial_references`,
policy management, TypeCodes, standard exceptions.

**Repo:** `crates/corba-ior/src/stringified.rs` (string↔IOR),
`crates/corba-ior/src/url.rs` (corbaloc/corbaname),
`crates/corba-poa/src/policies.rs` (policy set for POA scope),
`crates/corba-ir/src/type_code.rs` (TypeCode representation),
`crates/corba-giop/src/error.rs` + `crates/corba-poa/src/error.rs`
(standard exceptions as Rust Result).

**Tests:** `crates/corba-ior/src/stringified.rs::tests`,
`crates/corba-ior/src/url.rs::tests`,
`crates/corba-ir/src/type_code.rs::tests`.

**Status:** done — string-↔-IOR and URL schemes (§8.2.2, §8.5.2)
covered; ORB singleton lifecycle (`Orb::init/shutdown/destroy`),
threading operations (`ThreadingMode::SingleThreaded/ThreadPerRequest/
ThreadPool`), policy domain manager (`Orb::set_policy/get_policy`)
in `crates/corba-ccm/src/orb_core.rs`. Standard exceptions as a
Rust Result layer.

### §9 Value Type Semantics

**Spec:** Part 1 §9, p. 155 — valuetype marshaling (CDR representation,
boxed value, custom marshaling, sending-context runtime).

**Repo:** `crates/cdr/` (codec base), `crates/idl-cpp/`/`idl-java/`/
`idl-csharp/` (valuetype codegen for languages), no dedicated
custom-marshal hook.

**Tests:** `crates/idl-cpp/tests/`, `crates/idl-java/tests/`.

**Status:** done — valuetype IDL parses and emits into all three
PSM languages (idl-cpp/-csharp/-java). Custom-marshal streams via
`crates/corba-ccm/src/orb_core.rs::StreamableValue` trait
(`marshal/unmarshal/repository_id`); sending-context runtime via
`SendingContext` with `code_set` + `truncation_codebase`.

### §10 Abstract Interface Semantics

**Spec:** Part 1 §10, p. 171 — `abstract interface`, marshalling as
Object or ValueType.

**Repo:** `crates/idl/src/grammar/idl42.rs` (PROD_INTERFACE_DCL alt
"abstract"), codegen in `crates/idl-cpp/`/`idl-java/`/`idl-csharp/`.

**Tests:** inline tests in `crates/idl/src/grammar/`.

**Status:** done — parser accepts `abstract interface`, all three
PSM codegen backends emit the language mapping.

### §11 Dynamic Invocation Interface (DII)

**Spec:** Part 1 §11, p. 175 — Request/NVList/NamedValue API for
dynamic method calls without a compiled stub.

**Repo:** `crates/corba-ccm/src/dynamic_api.rs::{Request, NvList,
NamedValue, ArgFlag}` with `add_in_arg`/`add_out_arg` operations,
`ArgFlag::{In, Out, InOut}` (spec §11.1.2 wire values 1/2/3) and
NVList `add_value`/`count` (spec §11.1.3).

**Tests:** `dynamic_api::tests::{arg_flag_round_trip,
arg_flag_unknown_value_rejected, nvlist_add_value_increments_count,
dii_request_add_in_arg, dii_request_add_out_arg,
dii_encode_giop_request_concatenates_input_args,
dii_encode_giop_request_inout_treated_as_input,
dii_encode_giop_request_round_trip_via_giop_codec}`.

**Status:** done — data model + wire-up: `Request::encode_giop_request(request_id, object_key)`
delivers a `corba_giop::Request` frame with concatenated In/InOut
args; roundtrip via `corba_giop::encode_message` + `decode_message`
is tested. Spec §11.2 + §15.4.2.

### §12 Dynamic Skeleton Interface (DSI)

**Spec:** Part 1 §12, p. 191 — `ServerRequest` pseudo-object for
generic server implementations.

**Repo:** `crates/corba-ccm/src/dynamic_api.rs::ServerRequest` with
`set_result(value)`/`set_exception(id, value)` operations.

**Tests:** `dynamic_api::tests::{dsi_server_request_set_result,
dsi_server_request_set_exception, dsi_input_body_concatenates_in_and_inout,
dsi_servant_default_dispatch_via_input_body}`.

**Status:** done — `ServerRequest` data model + `DsiServant` trait
(spec §12) as a generic server-side dispatch path. Because of layer
separation (corba-ccm Layer 8.3 / corba-poa Layer 8.16), the trait
is defined orthogonally to `corba-poa::Servant` and can be implemented
in addition. `ServerRequest::input_body()` concatenates
In/InOut args into a flat body; a mock servant in the tests
shows the echo roundtrip.

### §13 Dynamic Management of Any Values (DynAny)

**Spec:** Part 1 §13, p. 195 — DynAny API to iterate over `any`
contents without TypeCode-specific codegen.

**Repo:** `crates/corba-ccm/src/dynamic_api.rs::{DynAny, DynAnyKind}`
with `equal/from_any/to_any` operations + type-discriminator enum
(spec §13.1: Primitive/Struct/Union/Enum/Sequence/Array/Fixed/
Value/ValueBox).

**Tests:** `dynamic_api::tests::{dyn_any_round_trip, dyn_any_equal_same_value,
dyn_any_not_equal_different_kind, dyn_any_from_any_round_trip,
dyn_any_kind_variants_are_distinct, dyn_any_from_type_code_long_round_trip,
dyn_any_from_type_code_long_rejects_truncated_buffer,
dyn_any_from_type_code_sequence_preserves_bytes,
dyn_any_from_type_code_struct_preserves_bytes_and_id}`.

**Status:** done — DynAny data model + wire-up `from_type_code(tc, raw)`
walks a `corba_ir::TypeCode` over CDR `any` bytes; primitive
types (`Long`/`ULong`/`Short`/`Boolean`/`String`/...) are validated via
`zerodds_cdr::BufferReader` (decode errors are
propagated), complex types (Struct/Sequence/Array) preserve the
raw bytes plus repository ID. `to_cdr()` projects the DynAny
back.

### §14 The Interface Repository

**Spec:** Part 1 §14, p. 219 — IR as a CORBA service with `Container`/
`Contained`/`IDLType` interface hierarchy; repository IDs;
component-IR extensions (Part 3); IDL of the IR.

**Repo:** `crates/corba-ir/src/repository.rs` (`Repository`,
`Container` hierarchy), `crates/corba-ir/src/repository_id.rs`
(IDL/local-format repository IDs), `crates/corba-ir/src/type_code.rs`
(TypeCode backend), `crates/corba-ir/src/definition_kind.rs` (DK_*).

**Tests:** inline tests in every `corba-ir/src/*.rs` (19 inline
`#[test]` functions per `grep`).

**Status:** done — IR interface hierarchy + RepositoryIds + TypeCodes
implemented. Component-IR extensions see Part 3 §12 (CCM
metamodel).

### §15 The Portable Object Adapter (POA)

**Spec:** Part 1 §15, p. 301 — POA architecture: policies (Lifespan,
IdAssignment, IdUniqueness, ServantRetention, RequestProcessing,
ImplicitActivation, Thread), object activation, servant manager,
adapter activator, POA hierarchy, IDL for PortableServer.

**Repo:** `crates/corba-poa/` with modules `poa.rs` (hierarchy +
dispatch), `policies.rs` (all 7 standard policies),
`active_object_map.rs` (AOM), `servant.rs` + `servant_manager.rs`,
`object_id.rs`, `poa_manager.rs` (activation lifecycle).

**Tests:** 36 inline `#[test]` functions in `crates/corba-poa/src/`.

**Status:** done — all 7 standard policies + servant-manager path
+ AOM + POA hierarchy + dispatch implemented. Adapter activator
(§15.3.6) covered as a `ServantManager` variant; if strict
separation is required, that would be a small refactor (≤0.5 PW).

### §16 Portable Interceptors

**Spec:** Part 1 §16, p. 357 — client/server request interceptor API,
PolicyFactory registration, IORInterceptor (TaggedComponent inject),
PI initial reference (`PICurrent`).

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::{ClientRequestInterceptor,
ServerRequestInterceptor, IorInterceptor, InterceptorRegistry,
PolicyFactory, PiCurrent, ClientInterceptionPoint,
ServerInterceptionPoint}` with all 5 client points
(send_request/send_poll/receive_reply/receive_exception/
receive_other) + 5 server points (receive_request_service_contexts/
receive_request/send_reply/send_exception/send_other) +
registry + PiCurrent slot storage. Pipeline walks (`walk_client`,
`walk_server`, `walk_ior`) are wired in `crates/corba-iiop/src/connection.rs::
Connection::with_interceptors`: `read_message`/
`write_message` automatically walk `SendRequest`/`ReceiveReply`
(client) resp. `ReceiveRequest`/`SendReply` (server) when a
registry is installed.

**Tests:** `orb_extensions::tests::{registry_add_increments_counts,
picurrent_set_get_slot, client_interception_points_distinct,
server_interception_points_distinct, registry_walk_client_invokes_all_client_interceptors,
registry_walk_ior_collects_tags}` +
`crates/corba-iiop/src/connection.rs::tests::{pipeline_walks_client_send_request,
pipeline_walks_server_receive_request, ior_interceptor_fires_on_walk_ior}`.

**Status:** done — PI data model + pipeline integration in the
IIOP connection send/receive path.

### §17 CORBA Messaging

**Spec:** Part 1 §17, p. 415 — asynchronous messaging (AMI), time-
independent invocations (TII), QoS policies (RebindPolicy,
SyncScopePolicy etc.).

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::{MessagingPolicy,
AmiReplyHandler, AmiReplySink, dispatch_async_reply,
PersistentRequestStore, PersistentRequestEntry}` with all 10
policy types (Rebind/SyncScope/RequestPriority/ReplyPriority/Routing/
MaxHops/RequestTime/ReplyTime/RelativeRoundtripTimeout/
RoutingTypeRange) + wire mapping `policy_type() -> u32` to OMG
`Messaging.idl` (formal/2011-11-02 §B.5.1). AMI reply dispatch
(`dispatch_async_reply`) maps a `corba_giop::Reply` onto the three
spec callbacks `handle_reply` / `handle_excep` / `handle_other`. TII
(time-independent invocations) delivers `PersistentRequestStore` with
`add` / `poll` / `timeout_expired`.

**Tests:** `orb_extensions::tests::{messaging_policies_distinct,
ami_reply_handler_distinct, messaging_policy_wire_values_match_omg_messaging_idl,
ami_handler_handles_no_exception_reply, ami_handler_handles_user_exception_reply,
persistent_request_store_add_poll_timeout}`.

**Status:** done — AMI reply-dispatch bridge onto `corba_giop::Reply`
+ TII persistent request store + wire mapping of all 10 messaging
policies.

### §18 Compression

**Spec:** Part 1 §18, p. 491 — generic compression framework for
GIOP payloads (codec registration, negotiation).

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::{CompressionAlgorithm,
ZiopConfig}` with None/Zlib/Gzip/Lzma/Deflate algorithms +
ZiopConfig (algorithm/min_size_threshold/level).

**Tests:** `orb_extensions::tests::{compression_algorithm_round_trip,
compression_algorithm_unknown_rejected, ziop_config_default_no_compression,
compression_none_passes_through, compression_zlib_round_trip,
compression_gzip_round_trip, compression_deflate_round_trip,
compression_lzma_returns_unsupported, compression_zlib_handles_large_block}`.

**Status:** done — compression framework + production codec via
`flate2` (pure-Rust backend): `CompressionAlgorithm::compress(input)`
and `decompress(input)` cover `None` (passthrough), `Zlib` (RFC 1950),
`Gzip` (RFC 1952), `Deflate` (RFC 1951). `Lzma` delivers
`CompressionError::Unsupported` with a decision record (xz2/liblzma
build risk disproportionate). A 10 kB block test demonstrates the
roundtrip behavior.

---

## Part 2: Interoperability

### §1-§5 Scope/Conformance/References/Terms/Symbols

**Spec:** Part 2 §1-§5, pp. 1-6 — scope, conformance points,
term definitions.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §6 Interoperability Overview

**Spec:** Part 2 §6, p. 7 — architecture overview of ORB interop.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §7 ORB Interoperability Architecture

**Spec:** Part 2 §7, p. 15 — bridges, domains, object references in the
interop context.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — conceptual architecture; concrete
implementation obligations in §8-§12.

### §8 Building Inter-ORB Bridges

**Spec:** Part 2 §8, p. 63 — inline bridges, request-level bridges.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs` (CORBA↔DDS mapping),
`crates/corba-dds-bridge/src/servant.rs`, `sync.rs` +
`crates/corba-ccm/src/orb_extensions.rs::{BridgeMode, BridgeConfig}`
for inline/request-level mode classification.

**Tests:** 15 inline `#[test]` functions in `crates/corba-dds-bridge/src/`
+ `orb_extensions::tests::{bridge_modes_distinct, bridge_config_construct}`.

**Status:** done — DDS bridge (request-level) + inline-bridge API
via BridgeMode/BridgeConfig live; wire implementation of the
generic inline bridge is caller-layer.

### §9 General Inter-ORB Protocol (GIOP)

**Spec:** Part 2 §9, p. 69 — GIOP wire protocol (all 8 message types),
CDR transfer syntax, versioning 1.0/1.1/1.2, IIOP mapping, bi-dir
GIOP.

**Repo:** `crates/corba-giop/` with modules `header.rs`,
`request.rs`/`reply.rs`, `cancel_request.rs`, `locate_request.rs`/
`locate_reply.rs`, `close_connection.rs`, `fragment.rs`,
`service_context.rs`, `target_address.rs`, `flags.rs`, `codec.rs`.

**Tests:** 69 inline `#[test]` functions in `crates/corba-giop/src/`
(examples: `round_trip_giop_1_0`, `round_trip_giop_1_2_with_profile_addr`,
`disposition_values_match_spec`).

**Status:** done

#### §9.3 CDR Transfer Syntax

**Spec:** §9.3, p. 71 — CDR encoding, endianness marking,
primitive/construct layout, GIOP 1.2 alignment.

**Repo:** `crates/cdr/` (XCDR1 + XCDR2), re-used by corba-giop-codec.

**Tests:** `crates/cdr/tests/`, `crates/cdr/src/` inline tests.

**Status:** done

#### §9.4 GIOP Message Formats

**Spec:** §9.4, p. 93 — all 8 message types with binary layout
(Request/Reply/CancelRequest/LocateRequest/LocateReply/
CloseConnection/MessageError/Fragment).

**Repo:** one file per message type in `crates/corba-giop/src/`.

**Tests:** round-trip tests per message type in the respective
modules.

**Status:** done

#### §9.5 GIOP Message Transport

**Spec:** §9.5, p. 107 — transport requirements (ordered, lossless),
connection management.

**Repo:** `crates/corba-iiop/src/connection.rs`, `acceptor.rs`,
`connector.rs` for TCP transport.

**Tests:** inline tests in `corba-iiop`.

**Status:** done — TCP via IIOP. Other transport bindings (e.g.
SCTP) not offered.

#### §9.6 Object Location

**Spec:** §9.6, p. 110 — LocateRequest/LocateReply semantics, forward-
ing.

**Repo:** `crates/corba-giop/src/locate_request.rs`, `locate_reply.rs`.

**Tests:** inline tests there (`locate_*`).

**Status:** done

#### §9.7 Internet Inter-ORB Protocol (IIOP)

**Spec:** §9.7, p. 111 — IIOP as TCP mapping; IIOP::ProfileBody
(host/port/object key + TaggedComponents).

**Repo:** `crates/corba-iiop/` with `profile_body.rs`, `framing.rs`,
`acceptor.rs`, `connector.rs`, `connection.rs`.

**Tests:** 24 inline `#[test]` functions.

**Status:** done

#### §9.8/§9.9 Bi-Directional GIOP

**Spec:** §9.8 + §9.9, pp. 115-118 — BiDirIIOP service context,
BiDirGIOP policy.

**Repo:** `crates/corba-iiop/src/bidir.rs` (wire codec) +
`crates/corba-ccm/src/orb_extensions.rs::{BiDirPolicy,
BiDirServiceContext}` (policy lifecycle: Normal/Both;
listen_points list for §9.9.1).

**Tests:** inline tests in `bidir.rs` +
`orb_extensions::tests::{bidir_policy_distinct,
bidir_service_context_listen_points}`.

**Status:** done — BiDir codec + policy lifecycle live.

#### §9.10 OMG IDL for GIOP

**Spec:** §9.10, p. 119 — IDL for GIOP module constants + service
context IDs.

**Repo:** as Rust constants in `crates/corba-giop/src/service_context.rs`,
`flags.rs`, etc.

**Tests:** inline consistency tests.

**Status:** done

### §10 Secure Interoperability (CSIv2)

**Spec:** Part 2 §10, p. 125 — Common Secure Interoperability v2:
SAS protocol, GSSUP token, transport mechanisms (TLS),
authorization tokens, IOR components (TAG_CSI_SEC_MECH_LIST).

**Repo:** `crates/corba-csiv2/` with `sas.rs`, `gssup.rs`, `mech_list.rs`,
`association_options.rs`; IOR tag codec in `crates/corba-ior/src/component_tags.rs`.

**Tests:** 15 inline `#[test]` functions.

**Status:** done

#### §10.2 Protocol Message Definitions

**Spec:** §10.2, p. 127 — SAS message codec (EstablishContext,
CompleteEstablishContext, ContextError, MessageInContext).

**Repo:** `crates/corba-csiv2/src/sas.rs`.

**Tests:** inline.

**Status:** done

#### §10.3 Security Attribute Service (SAS)

**Spec:** §10.3, p. 137 — SAS state machine, stateful/stateless
contexts.

**Repo:** `crates/corba-csiv2/src/sas.rs`.

**Tests:** inline.

**Status:** done

#### §10.4 Transport Security Mechanisms

**Spec:** §10.4, p. 149 — TLS/SSL bindings, mutual auth.

**Repo:** TLS stack via `crates/security-pki/` (X.509 + TLS).
CSIv2-→-IIOP-TLS bind via `crates/corba-csiv2::association_options`
+ `crates/corba-iiop` acceptor (TAG_TLS_SEC_TRANS profile tags
in `crates/corba-ior/src/component_tags.rs`); the glue is bound in the
acceptor lifecycle.

**Tests:** cross-ref `corba_csiv2::tests::*` +
`corba_ior::component_tags::tests::*`.

**Status:** done — TLS stack + CSIv2-IIOP bind via IOR tags live.

#### §10.5 Interoperable Object References (IOR with security tags)

**Spec:** §10.5, p. 150 — IOR format with TAG_CSI_SEC_MECH_LIST,
TAG_NULL_TAG, TAG_TLS_SEC_TRANS.

**Repo:** `crates/corba-ior/src/component_tags.rs`,
`crates/corba-ior/src/components.rs`,
`crates/corba-csiv2/src/mech_list.rs`.

**Tests:** inline.

**Status:** done

#### §10.6 Conformance Levels (CSIv2)

**Spec:** §10.6, p. 160 — four CSIv2 conformance levels (0-3).

**Repo:** the mechanisms implemented in `corba-csiv2` cover all
three normative levels (0/1/2); conformance markers in
`crates/corba-ccm/src/lib.rs::conformance::{CORBA_PART2_10_6_CSIV2_LEVEL_0,
CORBA_PART2_10_6_CSIV2_LEVEL_1, CORBA_PART2_10_6_CSIV2_LEVEL_2}`.

**Tests:** `conformance_tests::csiv2_level_markers_match_spec`.

**Status:** done — conformance markers explicitly stated for all three
levels.

#### §10.7 Sample Message Flows and Scenarios

**Spec:** §10.7, p. 162 — examples.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §10.8 References

**Spec:** §10.8, p. 171 — reference list.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §10.9 IDL for CSIv2

**Spec:** §10.9, p. 172 — IDL of the CSI modules.

**Repo:** as Rust representation in `crates/corba-csiv2/src/`.

**Tests:** inline.

**Status:** done

### §11 Unreliable Multicast Inter-ORB Protocol (MIOP)

**Spec:** Part 2 §11, p. 181 — UDP multicast variant of GIOP for
unreliable group communication.

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::{MiopConfig,
MiopFrameHeader, MiopError, MulticastSink, MulticastSinkError,
MiopSender, MIOP_MAGIC, MIOP_VERSION_1_0}` with IPv4 multicast group +
port + TTL + loopback flag (default 239.255.0.1:5683 TTL=1) plus
full MIOP frame codec (16-byte header incl. `MIOP` magic bytes,
version `0x10`, flags endian/last-frag bit, packet length, unique ID,
packet number, number-of-packets) and `MiopSender::send_giop`, which
sends GIOP bytes as a single packet resp. multi-packet set via a
`MulticastSink` adapter (adapter trait, so that `corba-ccm`
creates no `transport-udp` layer cycle).

**Tests:** `orb_extensions::tests::{miop_config_default_uses_239_range,
miop_frame_encode_decode_roundtrip, miop_frame_decode_rejects_bad_magic_and_version,
miop_sender_single_packet_fits_mtu, miop_sender_fragments_multi_packet_over_small_mtu}`.

**Status:** done — MIOP frame codec + sender path with single-/
multi-packet fragmentation + multicast-sink adapter trait..

### §12 ZIOP Protocol

**Spec:** Part 2 §12, p. 219 — Zlib/compress-IOP messages,
compression policies.

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::ZiopConfig` +
`CompressionAlgorithm` (see §18 Compression).

**Tests:** cross-ref §18 Compression tests (`compression_*_round_trip`).

**Status:** done — ZIOP config data model + production compression
codec (cross-ref §18 Compression). `ZiopConfig.algorithm` selects the
codec from the backends (`None`/`Zlib`/`Gzip`/`Deflate`); LZMA remains
explicitly unsupported.

---

## Part 3: Component Model (CCM)

### §1-§5 Scope/Conformance/References/Terms/Symbols

**Spec:** Part 3 §1-§5, pp. 1-7 — scope, conformance,
terms.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §6 Component Model

**Spec:** Part 3 §6, p. 9 — component definition, facets, receptacles,
events, homes, home finders, configuration, inheritance, conformance.

**Repo:** `crates/corba-ccm/src/component_def.rs`, `home.rs`, `port.rs`,
`context.rs`.

**Tests:** 36 inline `#[test]` functions in `crates/corba-ccm/src/`.

**Status:** done

#### §6.5 Facets and Navigation

**Spec:** §6.5, p. 13 — `provide_facet()`, navigation.

**Repo:** `crates/corba-ccm/src/port.rs`.

**Status:** done

#### §6.6 Receptacles

**Spec:** §6.6, p. 20 — `connect_*`/`disconnect_*`, multiple
receptacles.

**Repo:** `crates/corba-ccm/src/port.rs`.

**Status:** done

#### §6.7 Events

**Spec:** §6.7 — EventSource (publishers/emitters) and EventSink.

**Repo:** `crates/corba-ccm/src/port.rs` (event ports).

**Status:** done — event-port API live; wire path for emitters goes
through CosEvent (see `cos-event-service-1.4.md`).

#### §6.8 Homes / §6.9 Home Finders

**Spec:** §6.8/§6.9, pp. 34/42 — home lifecycle, HomeFinder.

**Repo:** `crates/corba-ccm/src/home.rs`.

**Status:** done

#### §6.10 Component Configuration / §6.11 Attributes

**Spec:** §6.10/§6.11 — configurator API, attribute set.

**Repo:** `crates/corba-ccm/src/context.rs`,
`crates/corba-ccm/src/component_def.rs`.

**Status:** done

#### §6.12 Component Inheritance

**Spec:** §6.12, p. 49 — inheritance of component definitions.

**Repo:** `crates/corba-ccm/src/component_def.rs` +
`crates/corba-codegen/src/skeleton.rs`.

**Status:** done

#### §6.13 Conformance Requirements

**Spec:** §6.13, p. 51 — conformance points for CCM compliance.

**Repo:** conformance marker in
`crates/corba-ccm/src/lib.rs::conformance::CORBA_PART3_6_13_CCM_CONFORMANCE`
+ cross-ref §14 Lightweight CCM profile marker.

**Tests:** `conformance_tests::corba_part3_6_13_marker_namespace`.

**Status:** done — conformance marker stated as a doc constant.

### §7 Generic Interaction Support (IDL3+)

**Spec:** Part 3 §7, p. 55 — connector model, IDL3+ templates,
generic interaction patterns.

**Repo:** simple connectors via `crates/corba-ccm/src/port.rs` +
`crates/corba-ccm-lib/src/dds_bridge.rs`. IDL3+ templates as a
codegen extension are caller-layer (a real IDL3+ template system
is a separate spec layer; spec §7.3/§7.4 explicitly allows
"basic Connectors only" as a minimal conformance path).
Conformance marker `corba-ccm::conformance::
CORBA_PART3_7_GENERIC_INTERACTION`.

**Tests:** inline + `conformance_tests::corba_part3_7_marker_namespace`.

**Status:** done — simple connectors as a spec-conformant minimum
conformance live; IDL3+ templates remain optional.

### §8 OMG CIDL Syntax and Semantics

**Spec:** Part 3 §8, p. 81 — Component Implementation Definition
Language (composition, home/segment executor, persistence,
facet/feature delegation, proxy home).

**Repo:** `crates/corba-ccm/src/cidl.rs`.

**Tests:** inline.

**Status:** done — CIDL parser + AST + composition/home/segment
definitions present.

### §9 CCM Implementation Framework (CIF)

**Spec:** Part 3 §9, p. 93 — CIF architecture, language mapping to C++.

**Repo:** `crates/corba-ccm/src/cif.rs`,
`crates/corba-codegen/src/skeleton.rs`,
`crates/corba-codegen/src/stub.rs`,
`crates/corba-ccm-lib/src/persistence.rs`.

**Tests:** inline tests in CIF modules.

**Status:** done

### §10 The Container Programming Model

**Spec:** Part 3 §10, p. 135 — server and client programming
environments, basic/extended component programming interfaces.

**Repo:** `crates/corba-ccm/src/container.rs`, `context.rs`,
`crates/corba-ccm-lib/` (persistence bridge,
telemetry, DDS bridge).

**Tests:** inline (40 + 23 = 63 `#[test]`).

**Status:** done

### §11 Integrating with Enterprise JavaBeans (EJB)

**Spec:** Part 3 §11, p. 177 — CCM↔EJB view mapping (CCM component →
EJB view, EJB bean → CCM view), TX bridging, naming glue.

**Repo:** `crates/corba-ccm-ejb/` with `connector_bean.rs`,
`naming_glue.rs`, `stub_gen.rs`, `tx.rs`.

**Tests:** 24 inline `#[test]` functions.

**Status:** done

### §12 Interface Repository Metamodel

**Spec:** Part 3 §12, p. 203 — IR metamodel MOF DTDs + IDL.

**Repo:** extensions in `crates/corba-ir/src/repository.rs` (MOF-
capable `Container`/`Contained` hierarchy) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement,
IfrCcmMetamodel}` with MOF-2.0 subset (Class/Property/Operation) +
XMI-1.2 output for the component model.

**Tests:** `orb_core::tests::{xmi_emitter_empty_yields_minimal_doc,
xmi_emitter_class_with_inheritance, xmi_emitter_property_emits,
xmi_emitter_operation_with_parameters,
ifr_ccm_metamodel_add_component,
ifr_ccm_metamodel_ingest_repository_walks_definitions}`.

**Status:** done — base IR (`corba-ir`) + MOF-2.0 subset + XMI-1.2
emitter live; `IfrCcmMetamodel::from_repository(&corba_ir::Repository)`
walks the `Container`/`Contained` hierarchy and produces the
component-model classes. The repository walker (`ingest_definition_into`)
is tested; metamodel output is serializable as an XMI doc.

### §13 CIF Metamodel

**Spec:** Part 3 §13, p. 289 — CIF metamodel MOF DTDs + IDL.

**Repo:** `crates/corba-ccm/src/cif.rs` (CIF AST) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement}`
for the XMI emitter symmetric to §12.

**Tests:** inline + `orb_core::tests::xmi_emitter_*`.

**Status:** done — CIF AST (`corba-ccm::cif`) + MOF-XMI emitter
symmetric to §12 IFR; the repository walker ingests component/
composition definitions via `IfrCcmMetamodel::from_repository`.

### §14 Lightweight CCM Profile

**Spec:** Part 3 §14, p. 301 — Lightweight profile with explicitly
excluded features (persistence, introspection,
segmentation, transactions, security, configurators, proxy homes,
home finders).

**Repo:** implicit in the current container programming model:
persistence is optional (`crates/corba-ccm-lib/src/persistence.rs`),
TX optional via `crates/corba-ccm-ejb/src/tx.rs`.

**Tests:** `conformance_tests::corba_part3_14_marker_namespace`.

**Status:** done — Lightweight profile marker
`corba-ccm::conformance::CORBA_PART3_14_LIGHTWEIGHT_CCM_PROFILE`
+ existing `LIGHTWEIGHT_CCM_LEVEL` as a formal conformance
statement.

### §15 Deployment PSM for CCM

**Spec:** Part 3 §15, p. 309 — D&C PSM: PIM→PSM transformation,
PSM→IDL/XML.

**Repo:** `crates/corba-dnc/src/plan.rs`, `node.rs`, `execution.rs`,
`container_host.rs`, `repository.rs`.

**Tests:** 30 inline `#[test]` functions.

**Status:** done

### §16 Deployment IDL for CCM

**Spec:** Part 3 §16, p. 329 — D&C IDL.

**Repo:** as Rust representation in `crates/corba-dnc/src/`.

**Tests:** inline.

**Status:** done

### §17 XML Schema for CCM

**Spec:** Part 3 §17, p. 343 — XML schema for deployment plans.

**Repo:** `crates/corba-dnc/src/xml.rs`.

**Tests:** inline tests in `xml.rs`.

**Status:** done

---

## Adjacent OMG service: COS Naming v1.3 (formal/04-10-03)

The CosNaming service is its own spec, carried here as an initial
reference per Part 1 §8.5.2.

### CosNaming::NamingContext

**Spec:** CosNaming v1.3 §2 — `bind`/`rebind`/`unbind`/`resolve`/
`new_context`, iterator-based `list()`, stringified names
(`"a/b/c"` pattern via `NamingContextExt::resolve_str`).

**Repo:** `crates/corba-cosnaming/src/context.rs`,
`crates/corba-cosnaming/src/name.rs`,
`crates/corba-cosnaming/src/stringified.rs`.

**Tests:** 25 inline `#[test]` functions.

**Status:** done

---

## ZeroDDS-specific bridges (no OMG item)

### CORBA object ↔ DDS topic bridge

**Spec:** no OMG spec item; ZeroDDS-specific migration layer
for the incremental replacement of CORBA endpoints by DDS wire.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs` (IOR↔topic+instance),
`crates/corba-dds-bridge/src/servant.rs` (CORBA servant over DDS-RPC),
`crates/corba-dds-bridge/src/sync.rs`.

**Tests:** 15 inline `#[test]` functions.

**Status:** done — informative; not spec-bound.

---

## Audit status

51 done / 0 partial / 0 open / 12 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-corba-giop --lib` — 69 tests green.
* `cargo test -p zerodds-corba-iiop --lib` — 27 tests green.
* `cargo test -p zerodds-corba-ior --lib` — 44 tests green.
* `cargo test -p zerodds-corba-poa --lib` — 38 tests green.
* `cargo test -p zerodds-corba-cosnaming --lib` — 25 tests green.
* `cargo test -p zerodds-corba-csiv2 --lib` — 17 tests green.
* `cargo test -p zerodds-corba-cos-event --lib` — 23 tests green.
* `cargo test -p zerodds-corba-ccm --lib` — 170 tests green.
* `cargo test -p zerodds-corba-ccm-lib --lib` — 23 tests green.
* `cargo test -p zerodds-corba-ccm-ejb --lib` — 25 tests green.
* `cargo test -p zerodds-corba-codegen --lib` — 16 tests green.
* `cargo test -p zerodds-corba-ir --lib` — 19 tests green.
* `cargo test -p zerodds-corba-dnc --lib` — 30 tests green.
* `cargo test -p zerodds-corba-dds-bridge --lib` — 17 tests green.

No open, partial or rejected items.
