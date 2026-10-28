# OMG CORBA 3.3 — Spec Coverage (WP CORBA Coexistence)

**Source:** OMG CORBA 3.3 — three volumes as formal documents:
* Part 1 (Interfaces, 532 pp.) — [`formal/12-11-12`](https://www.omg.org/spec/CORBA/3.3/Interfaces/PDF)
* Part 2 (Interoperability, 249 pp.) — [`formal/12-11-14`](https://www.omg.org/spec/CORBA/3.3/Interoperability/PDF)
* Part 3 (Component Model, 380 pp.) — [`formal/12-11-16`](https://www.omg.org/spec/CORBA/3.3/Components/PDF)

**Context.** With the DDS core and the codegen in
`crates/lang-rules/` and `crates/corba-codegen/`, ZeroDDS fully covers
the DDS PSM. For migrating legacy CORBA applications in
the finance industry, ZeroDDS should be able to appear as a **drop-in
backend**: an existing CORBA client compiles and links against
Annex-A.1 stubs, runs against ZeroDDS endpoints that speak GIOP/IIOP
wire, and can be migrated incrementally to pure DDS without
ad-hoc infrastructure swaps.

CORBA coexistence is spread across several implementation crates:

- `crates/idl-parse/` — IDL grammar (Part 1 §7 IDL syntax)
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

**Repo:** `crates/idl-cst/src/lex.rs`, `crates/idl-parse/src/decl.rs`,
`crates/idl-lower/src/lower.rs`, `crates/corba-codegen/src/repository_id.rs`,
`crates/corba-codegen/src/special_types.rs`, `crates/corba-codegen/src/skeleton.rs`,
`crates/corba-codegen/src/stub.rs`.

**Tests:** `crates/idl-parse/tests/parse.rs::{each_declaration_form_is_recognised,
a_file_using_most_of_the_grammar_parses_cleanly_and_round_trips}`,
`crates/idl-lower/tests/conformance.rs::{every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares,
every_fixture_lowers_to_exactly_the_diagnostics_it_should}`,
`crates/corba-codegen/src/repository_id.rs::{a_nested_module_path,
splitting_takes_the_last_colon_and_checks_the_version_is_a_version}`.

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

**Tests:** `crates/corba-ior/src/stringified.rs::tests`: `round_trip_be`,
`round_trip_le`, `upper_and_lower_hex_both_decoded`,
`missing_prefix_is_diagnostic`, `odd_hex_length_is_diagnostic`,
`non_hex_char_is_diagnostic`, `nil_ior_round_trip`;
`crates/corba-ior/src/url.rs::tests`: `parse_simple_iiop_corbaloc`,
`default_port_2809_when_missing`, `rir_endpoint`, `multi_endpoint_failover`,
`corbaname_with_fragment`;
`crates/corba-ir/src/type_code.rs::tests`:
`well_known_tc_kind_values_match_spec`, `primitive_tc_has_no_body`,
`struct_tc_carries_id_and_members`, `sequence_tc_holds_element`,
`enum_tc_holds_member_names`.

**Status:** done — string-↔-IOR and URL schemes (§8.2.2, §8.5.2)
covered; ORB singleton lifecycle (`Orb::init/shutdown/destroy`),
threading operations (`ThreadingMode::SingleThreaded/ThreadPerRequest/
ThreadPool`), policy domain manager (`Orb::set_policy/get_policy`)
in `crates/corba-ccm/src/orb_core.rs`. Standard exceptions as a
Rust Result layer.

### §9 Value Type Semantics

**Spec:** Part 1 §9, p. 155 — valuetype marshaling (CDR representation,
boxed value, custom marshaling, sending-context runtime).

**Repo:** `crates/wire/src/writer.rs` + `crates/wire/src/reader.rs`
(codec base), `crates/idl-parse/src/decl.rs` (`parse_valuetype`, value
box and `custom valuetype`), `crates/lang-rules/src/coverage.rs`
(valuetype rule per target language), no dedicated custom-marshal hook
in the codegen.

**Tests:** `crates/idl-lower/tests/lowering.rs::{the_other_valuetype_forms_are_unaffected_by_the_box,
an_abstract_valuetype_parses_as_a_valuetype,
a_forward_declared_valuetype_is_completed_by_its_definition}`,
`crates/corba-codegen/tests/orb_agreement.rs::the_valuebase_holder_is_the_omgs_and_jacorbs_names_a_class_that_does_not_exist`.

**Status:** done — valuetype IDL parses, lowers and carries an emission
rule per target language in `lang-rules`. Custom-marshal streams via
`crates/corba-ccm/src/orb_core.rs::StreamableValue` trait
(`marshal/unmarshal/repository_id`); sending-context runtime via
`SendingContext` with `code_set` + `truncation_codebase`.

**Tests:** `crates/corba-codegen/tests/annex_a1_traits.rs`:
`table_a8_reads_abstract_and_truncatable_off_the_declaration`,
`table_a9_gives_a_valuebox_its_boxed_trait`,
`every_traited_type_renders_in_every_column`.

**Status:** done — a valuetype and a valuebox carry the members Annex
A.1.1 gives them in all four PSM columns.

### §10 Abstract Interface Semantics

**Spec:** Part 1 §10, p. 171 — `abstract interface`, marshalling as
Object or ValueType.

**Repo:** `crates/idl-parse/src/decl.rs` (`abstract` belongs to two
productions — the interface header (79) and `<value_abs_def>` (127);
the fork is in `parse_definition`), codegen corpus row in
`crates/corba-codegen/tests/orb-corpus/rows/03-abstractbase.idl`.

**Tests:** `crates/idl-lower/tests/lowering.rs::abstract_still_reaches_the_interface_header_it_also_belongs_to`,
`crates/idl-lower/tests/interface_as_type.rs::an_interface_used_as_a_type_resolves_in_every_spelling`
(the "an abstract interface" row).

**Status:** done — the parser accepts `abstract interface`, lowering
carries it as its own interface kind, and the codegen corpus holds the
row.

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
`zerodds_wire::Reader` (decode errors are
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

**Tests:** `crates/corba-ir/src/repository.rs::tests` (11 tests):
`scoped_lookup_descends_hierarchy`, `absolute_name_builds_scoped_path`,
`operation_def_carries_typed_signature`, `interface_def_details`,
`register_and_lookup_id`, `duplicate_id_yields_error`,
`lookup_name_at_top_level`, `nested_module_lookup_name_recurses`,
`describe_contents_filters_by_kind`, `definition_carries_type_code`,
`remove_deletes_definition`;
`crates/corba-ir/src/definition_kind.rs::tests::well_known_definition_kinds_match_spec`,
`::at_least_26_kinds_modeled`;
`crates/corba-ir/src/repository_id.rs::tests::round_trip_omg_namingcontext`,
`::missing_idl_prefix_is_invalid`, `::missing_version_is_invalid`.

**Status:** done — IR interface hierarchy + RepositoryIds + TypeCodes
implemented. Component-IR extensions see Part 3 §12 (CCM
metamodel).

### §15 The Portable Object Adapter (POA)

**Spec:** Part 1 §15, p. 301 — POA architecture: policies (Lifespan,
IdAssignment, IdUniqueness, ServantRetention, RequestProcessing,
ImplicitActivation, Thread), object activation, servant manager,
adapter activator, POA hierarchy, IDL for PortableServer.

**Repo:** `crates/corba-poa/src/poa.rs` (hierarchy + dispatch),
`crates/corba-poa/src/policies.rs` (all 7 standard policies),
`crates/corba-poa/src/active_object_map.rs` (AOM),
`crates/corba-poa/src/servant.rs` + `crates/corba-poa/src/servant_manager.rs`,
`crates/corba-poa/src/object_id.rs`,
`crates/corba-poa/src/poa_manager.rs` (activation lifecycle).

**Tests:** `crates/corba-poa/src/poa.rs::tests` (12 tests):
`poa_hierarchy_create_find_destroy`, `default_root_dispatch_via_aom`,
`user_id_activation_with_explicit_oid`,
`use_default_servant_falls_back_to_default`,
`use_servant_manager_with_activator_caches_in_aom`,
`non_retain_with_locator_calls_pre_post_per_invocation`,
`dispatch_in_holding_state_is_rejected`,
`dispatch_in_inactive_state_yields_adapter_inactive`,
`dispatch_in_discarding_state_returns_obj_adapter`,
`invalid_policy_combination_is_rejected_at_construction`;
`crates/corba-poa/src/active_object_map.rs::tests` (6),
`crates/corba-poa/src/policies.rs::tests` (8),
`crates/corba-poa/src/poa_manager.rs::tests::full_state_cycle`,
`crates/corba-poa/src/object_id.rs::tests::system_id_is_8_bytes_big_endian`,
`crates/corba-poa/src/servant.rs::tests::is_a_checks_repository_ids`,
`crates/corba-poa/src/servant_manager.rs::tests::locator_preinvoke_returns_servant_and_cookie`.

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
`crates/corba-iiop/src/connection.rs::{pipeline_walks_client_send_request,
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

**Repo:** `crates/corba-giop/src/` with one file per message type plus
`crates/corba-giop/src/header.rs`, `crates/corba-giop/src/codec.rs`,
`crates/corba-giop/src/service_context.rs`,
`crates/corba-giop/src/target_address.rs`,
`crates/corba-giop/src/flags.rs`, `crates/corba-giop/src/version.rs`.

**Tests:** `crates/corba-giop/src/codec.rs::{round_trip_request_giop_1_0_be,
round_trip_request_giop_1_2_le, giop_1_2_body_is_eight_aligned_from_the_message_start,
giop_1_0_body_is_not_padded, header_message_size_matches_actual_body,
message_size_larger_than_the_buffer_is_refused}`,
`crates/corba-giop/src/header.rs::{magic_bytes_match_spec,
header_size_is_12_bytes, round_trip_big_endian_request_header,
round_trip_little_endian_reply_header}`,
`crates/corba-giop/src/version.rs::{version_constants_match_spec,
flags_octet_replaces_byte_order_at_1_1, v1_2_request_layout_only_in_1_2_plus}`.

**Status:** done

#### §9.3 CDR Transfer Syntax

**Spec:** §9.3, p. 71 — CDR encoding, endianness marking,
primitive/construct layout, GIOP 1.2 alignment.

**Repo:** `crates/wire/src/writer.rs` + `crates/wire/src/reader.rs`
(primitives + alignment + encapsulation), re-used by the GIOP codec in
`crates/corba-giop/src/codec.rs`.

**Tests:** `crates/corba-giop/src/codec.rs::{giop_1_2_body_is_eight_aligned_from_the_message_start,
giop_1_2_reply_body_is_eight_aligned_from_the_message_start,
giop_1_0_body_is_not_padded}`,
`crates/corba-giop/src/request.rs::tests::giop_1_2_request_body_is_8_aligned`,
`crates/corba-giop/src/service_context.rs::{round_trip_with_two_entries_be,
round_trip_le_byte_order}`.

**Status:** done

#### §9.4 GIOP Message Formats

**Spec:** §9.4, p. 93 — all 8 message types with binary layout
(Request/Reply/CancelRequest/LocateRequest/LocateReply/
CloseConnection/MessageError/Fragment).

**Repo:** one file per message type in `crates/corba-giop/src/` —
`crates/corba-giop/src/request.rs`, `crates/corba-giop/src/reply.rs`,
`crates/corba-giop/src/cancel_request.rs`,
`crates/corba-giop/src/locate_request.rs`,
`crates/corba-giop/src/locate_reply.rs`,
`crates/corba-giop/src/close_connection.rs`,
`crates/corba-giop/src/message_error.rs`,
`crates/corba-giop/src/fragment.rs`.

**Tests:** one octet round trip per message type —
`crates/corba-giop/src/request.rs::{round_trip_giop_1_0_request,
round_trip_giop_1_1_request_with_reserved_bytes,
round_trip_giop_1_2_request_with_target_address}`,
`crates/corba-giop/src/reply.rs::{round_trip_giop_1_0_no_exception,
round_trip_giop_1_2_user_exception, round_trip_location_forward_perm_in_1_2}`,
`crates/corba-giop/src/cancel_request.rs::tests::round_trip`,
`crates/corba-giop/src/locate_request.rs::{round_trip_giop_1_0,
round_trip_giop_1_2_with_profile_addr}`,
`crates/corba-giop/src/locate_reply.rs::{round_trip_giop_1_0_object_here,
round_trip_giop_1_2_object_forward_perm}`,
`crates/corba-giop/src/close_connection.rs::tests::body_is_unit`,
`crates/corba-giop/src/message_error.rs::tests::body_is_unit`,
`crates/corba-giop/src/fragment.rs::{round_trip_giop_1_2_with_header,
round_trip_giop_1_1_without_header}`. All eight also through the
whole-message codec: `crates/corba-giop/src/codec.rs::{round_trip_request_giop_1_0_be,
round_trip_close_connection, round_trip_message_error,
round_trip_fragment_with_more_bit, round_trip_cancel_request,
round_trip_locate_request_giop_1_2, round_trip_locate_reply_object_here}`.
The type discriminators themselves:
`crates/corba-giop/src/message_type.rs::tests::message_type_values_match_spec_table_15_1`.

**Status:** done

#### §9.5 GIOP Message Transport

**Spec:** §9.5, p. 107 — transport requirements (ordered, lossless),
connection management.

**Repo:** `crates/corba-iiop/src/connection.rs`,
`crates/corba-iiop/src/acceptor.rs`,
`crates/corba-iiop/src/connector.rs` for TCP transport.

**Tests:** `crates/corba-iiop/src/acceptor.rs::tests::round_trip_via_acceptor_and_connector`,
`crates/corba-iiop/src/connection.rs::{round_trip_request_reply,
shutdown_propagates_eof_to_peer, close_connection_message_round_trip}`,
`crates/corba-iiop/src/framing.rs::tests::read_back_to_back_messages`
(message boundaries in the stream),
`crates/corba-iiop/src/connector.rs::{connect_reuses_pooled_connection,
invalidated_connection_is_not_returned_to_pool,
max_connections_per_endpoint_is_enforced}`.

**Status:** done — TCP via IIOP. Other transport bindings (e.g.
SCTP) not offered.

#### §9.6 Object Location

**Spec:** §9.6, p. 110 — LocateRequest/LocateReply semantics, forward-
ing.

**Repo:** `crates/corba-giop/src/locate_request.rs`,
`crates/corba-giop/src/locate_reply.rs`.

**Tests:** `crates/corba-giop/src/locate_reply.rs::{locate_status_values_match_spec,
round_trip_giop_1_0_object_here, round_trip_giop_1_2_object_forward_perm,
forward_perm_in_1_0_is_rejected}`,
`crates/corba-giop/src/locate_request.rs::{round_trip_giop_1_0,
round_trip_giop_1_2_with_profile_addr, giop_1_0_rejects_profile_target}`.

**Status:** done

#### §9.7 Internet Inter-ORB Protocol (IIOP)

**Spec:** §9.7, p. 111 — IIOP as TCP mapping; IIOP::ProfileBody
(host/port/object key + TaggedComponents).

**Repo:** `crates/corba-iiop/src/profile_body.rs`,
`crates/corba-iiop/src/framing.rs`, `crates/corba-iiop/src/acceptor.rs`,
`crates/corba-iiop/src/connector.rs`, `crates/corba-iiop/src/connection.rs`.

**Tests:** `crates/corba-iiop/src/profile_body.rs::{iiop_version_constants_match_spec,
components_only_from_iiop_1_1, round_trip_v1_0_be, round_trip_v1_0_le,
round_trip_v1_2_with_components, v1_0_does_not_emit_components_field}`,
`crates/corba-iiop/src/framing.rs::{round_trip_request_through_cursor,
round_trip_reply_le, outbound_fragmentation_be_le_many_fragments,
fragment_reassembly_three_fragments, oversized_message_size_is_rejected_cleanly}`.

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

**Repo:** as Rust constants in `crates/corba-giop/src/service_context.rs`
(`ServiceContextTag`), `crates/corba-giop/src/message_type.rs` (`MessageType`),
`crates/corba-giop/src/flags.rs` (`Flags`),
`crates/corba-giop/src/version.rs` (`Version`),
`crates/corba-giop/src/reply.rs` (`ReplyStatusType`),
`crates/corba-giop/src/locate_reply.rs` (`LocateStatusType`),
`crates/corba-giop/src/target_address.rs` (`AddressingDisposition`).

**Tests:** one value comparison against the IDL per constant group —
`crates/corba-giop/src/service_context.rs::tests::standard_tag_values_match_spec`,
`crates/corba-giop/src/message_type.rs::tests::message_type_values_match_spec_table_15_1`,
`crates/corba-giop/src/flags.rs::{fragment_bit_value_matches_spec,
byte_order_bit_value_matches_spec}`,
`crates/corba-giop/src/version.rs::tests::version_constants_match_spec`,
`crates/corba-giop/src/reply.rs::tests::reply_status_values_match_spec`,
`crates/corba-giop/src/locate_reply.rs::tests::locate_status_values_match_spec`,
`crates/corba-giop/src/target_address.rs::tests::disposition_values_match_spec`.

**Status:** done

### §10 Secure Interoperability (CSIv2)

**Spec:** Part 2 §10, p. 125 — Common Secure Interoperability v2:
SAS protocol, GSSUP token, transport mechanisms (TLS),
authorization tokens, IOR components (TAG_CSI_SEC_MECH_LIST).

**Repo:** `crates/corba-csiv2/` with `sas.rs`, `gssup.rs`, `mech_list.rs`,
`association_options.rs`; IOR tag codec in `crates/corba-ior/src/component_tags.rs`.

**Tests:** `crates/corba-csiv2/src/sas.rs::tests` (9 tests):
`sas_msg_type_values_match_spec`,
`sas_context_body_roundtrip_all_messages_be_le`,
`server_eval_decodes_foreign_gss_wrapped_establish_context`,
`identity_token_discriminator_matches_spec`,
`establish_context_holds_full_payload`, `complete_context_indicates_stateful`,
`context_error_carries_gss_status`;
`crates/corba-csiv2/src/session.rs::tests` (9), among them
`a_stateful_target_answers_establish_with_a_stateful_complete`,
`a_full_session_survives_the_wire_in_both_byte_orders`;
`crates/corba-csiv2/src/gssup.rs::tests::gssup_byte_identical_to_jacorb`,
`::initial_context_token_tag_is_0x60`,
`::encapsulation_is_wire_correct_octet_seq_len_at_offset_4`.

**Status:** done

#### §10.2 Protocol Message Definitions

**Spec:** §10.2, p. 127 — SAS message codec (EstablishContext,
CompleteEstablishContext, ContextError, MessageInContext).

**Repo:** `crates/corba-csiv2/src/sas.rs` (`SasMessage` with
`encode_encapsulation`/`decode_encapsulation`, `IdentityToken`).

**Tests:** `crates/corba-csiv2/src/sas.rs::{sas_msg_type_values_match_spec,
sas_context_body_roundtrip_all_messages_be_le,
sas_identity_token_absent_anonymous_roundtrip,
identity_token_discriminator_matches_spec, establish_context_holds_full_payload,
complete_context_indicates_stateful, message_in_context_can_request_discard,
context_error_carries_gss_status}`.

**Status:** done

#### §10.3 Security Attribute Service (SAS)

**Spec:** §10.3, p. 137 — SAS state machine, stateful/stateless
contexts.

**Repo:** `crates/corba-csiv2/src/session.rs::{SasTarget, SasClient,
TargetAction, RetainedContext, CONTEXT_ERROR_NO_CONTEXT,
CONTEXT_ERROR_INVALID_EVIDENCE}` — the target and client halves of the
state machine over the `SasMessage` types of §10.2. A stateful target
retains the context under its `ContextId` and the client then sends
`MessageInContext` instead of fresh credentials, until a
`discard_context` or a `ContextError` puts it back into the establish
state.

**Tests:** `crates/corba-csiv2/src/session.rs::{a_stateful_target_answers_establish_with_a_stateful_complete,
a_stateless_target_retains_nothing_and_says_so,
the_second_request_on_a_stateful_context_is_a_message_in_context,
a_stateless_client_re_establishes_on_every_request,
discarding_a_context_makes_the_next_reference_to_it_a_context_error,
a_message_in_context_for_an_unknown_id_is_refused,
a_context_error_puts_the_client_back_into_the_establish_state,
a_target_to_client_message_arriving_at_a_target_is_not_evidence,
a_full_session_survives_the_wire_in_both_byte_orders}`.

**Status:** done — the last test drives the whole session over the
§10.2 octets: every message crosses `encode_encapsulation` /
`decode_encapsulation` before the other half sees it.

#### §10.4 Transport Security Mechanisms

**Spec:** §10.4, p. 149 — TLS/SSL bindings, mutual auth.

**Repo:** TLS stack via `crates/security-pki/` (X.509 + TLS).
CSIv2-→-IIOP-TLS bind via `crates/corba-csiv2::association_options`
+ `crates/corba-iiop` acceptor (TAG_TLS_SEC_TRANS profile tags
in `crates/corba-ior/src/component_tags.rs`); the glue is bound in the
acceptor lifecycle.

**Tests:** `crates/corba-csiv2/src/mech_list.rs::tests`:
`build_mech_list_with_tls_and_gssup`, `empty_list_has_no_mechanisms`,
`cdr_roundtrip_compound_sec_mech_list`,
`octet_seq_seq_rejects_oversized_count`,
`compound_sec_mech_list_rejects_oversized_count`, `cdr_roundtrip_empty_list`;
`crates/corba-csiv2/src/association_options.rs::tests::flag_values_match_spec_table_24_1`,
`::has_and_with_round_trip`;
`crates/corba-ior/src/component_tags.rs::tests::well_known_component_tag_values_match_spec`,
`::round_trip_all_known_tags`, `::at_least_32_well_known_tags_modelled`.

**Status:** done — TLS stack + CSIv2-IIOP bind via IOR tags live.

#### §10.5 Interoperable Object References (IOR with security tags)

**Spec:** §10.5, p. 150 — IOR format with TAG_CSI_SEC_MECH_LIST,
TAG_NULL_TAG, TAG_TLS_SEC_TRANS.

**Repo:** `crates/corba-ior/src/component_tags.rs` (`ComponentId` with
`CsiSecMechList` = 33, `NullTag` = 34, `TlsSecTrans` = 36),
`crates/corba-ior/src/components.rs` (`StructuredComponent`),
`crates/corba-csiv2/src/mech_list.rs` (`CompoundSecMechList`).

**Tests:** `crates/corba-ior/src/ior.rs::tests::a_security_ior_carries_mech_list_tls_and_null_tag_through_a_round_trip`
— a complete IOR carrying the three tags in the IIOP profile's
component list, through `encode_encapsulation`/`decode_encapsulation`
and back into the structured form. Tag values and the component bodies
on their own:
`crates/corba-ior/src/component_tags.rs::{well_known_component_tag_values_match_spec,
round_trip_all_known_tags, at_least_32_well_known_tags_modelled}`,
`crates/corba-ior/src/components.rs::{csi_sec_mech_list_round_trip,
tls_sec_trans_with_addresses_round_trip, ssl_sec_trans_round_trip}`.

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

**Repo:** as Rust representation in
`crates/corba-csiv2/src/association_options.rs` (`AssociationOptions`),
`crates/corba-csiv2/src/mech_list.rs` (`CompoundSecMechList`,
`CompoundSecMech`, `AsContextSec`, `SasContextSec`),
`crates/corba-csiv2/src/sas.rs` (`SasMsgType`, `IdentityToken`, the
`SASContextBody` union as `SasMessage`),
`crates/corba-csiv2/src/gssup.rs` (`GssupCredentialToken`).

**Tests:** one value/layout comparison against the IDL per module —
`crates/corba-csiv2/src/association_options.rs::tests::flag_values_match_spec_table_24_1`,
`crates/corba-csiv2/src/sas.rs::{sas_msg_type_values_match_spec,
identity_token_discriminator_matches_spec}`,
`crates/corba-csiv2/src/mech_list.rs::{cdr_roundtrip_compound_sec_mech_list,
build_mech_list_with_tls_and_gssup, cdr_roundtrip_empty_list}`,
`crates/corba-csiv2/src/gssup.rs::{initial_context_token_tag_is_0x60,
gssup_oid_starts_with_der_tag, encapsulation_is_wire_correct_octet_seq_len_at_offset_4}`.

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

**Tests:** `crates/corba-ccm/src/orb_extensions.rs::tests::compression_algorithm_round_trip`,
`::compression_zlib_round_trip`, `::compression_gzip_round_trip`,
`::compression_deflate_round_trip`.

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

**Repo:** `crates/corba-ccm/src/component_def.rs`,
`crates/corba-ccm/src/home.rs`, `crates/corba-ccm/src/port.rs`,
`crates/corba-ccm/src/context.rs`, `crates/corba-ccm/src/lifecycle.rs`.

**Tests:** `crates/corba-ccm/src/component_def.rs::tests` (30 tests), among them
`trader_has_4_ports`, `keyed_component_detected`,
`provide_facet_returns_facet_by_name`, `get_all_facets_returns_complete_list`,
`get_all_publishers_excludes_emit_only_sources`,
`supports_interface_returns_true_for_listed_iface`,
`the_base_chain_runs_from_the_derived_component_to_the_root`,
`flattening_takes_over_every_inherited_feature_bases_first`,
`a_redefined_port_name_is_refused_rather_than_shadowed`,
`a_second_primary_key_in_the_chain_is_refused`;
`crates/corba-ccm/src/cidl.rs::tests::all_four_composition_categories_distinct`;
the sub-items §6.5-§6.13 name one test per clause each.

**Status:** done

#### §6.5 Facets and Navigation

**Spec:** §6.5, p. 13 — `provide_facet()`, navigation.

**Repo:** `crates/corba-ccm/src/component_def.rs`
(`provide_facet`/`get_all_facets`/`get_named_facets` — the navigation
interface), `crates/corba-ccm/src/lifecycle.rs` (`check_facet_lifetime`
— facet reference lifetime).

**Tests:** `crates/corba-ccm/src/component_def.rs::{provide_facet_returns_facet_by_name,
provide_facet_returns_none_for_unknown, get_all_facets_returns_complete_list,
get_named_facets_filters_by_names}`,
`crates/corba-ccm/src/lifecycle.rs::{facet_lifetime_passes_when_alive,
facet_lifetime_rejects_use_after_destroy, facet_lifetime_rejects_orphaned}`.

**Status:** done

#### §6.6 Receptacles

**Spec:** §6.6, p. 20 — `connect_*`/`disconnect_*`, multiple
receptacles.

**Repo:** `crates/corba-ccm/src/port.rs` (`PortRegistry::connect_receptacle`/
`disconnect_receptacle`/`get_connections`), `crates/corba-ccm/src/lifecycle.rs`
(`ReceptacleManager` with the simplex/multiplex rule),
`crates/corba-ccm/src/component_def.rs` (`ReceptacleMultiplicity`,
`get_all_receptacles`/`get_named_receptacles`).

**Tests:** `crates/corba-ccm/src/port.rs::{connect_and_get_connections,
disconnect_removes_only_target_id, disconnect_unknown_returns_false}`,
`crates/corba-ccm/src/lifecycle.rs::{receptacle_simplex_connect_then_disconnect,
receptacle_simplex_double_connect_rejected,
receptacle_multiplex_allows_multiple_connects,
receptacle_disconnect_unknown_id_rejected, receptacle_double_disconnect_rejected}`,
`crates/corba-ccm/src/component_def.rs::{receptacle_multiplicity_distinct,
get_all_receptacles_returns_complete_list, get_named_receptacles_filters}`.

**Status:** done

#### §6.7 Events

**Spec:** §6.7 — EventSource (publishers/emitters) and EventSink.

**Repo:** `crates/corba-ccm/src/port.rs` (event ports, `EventStream`),
`crates/corba-ccm/src/component_def.rs` (`EventSourceDef.emit_only`
separates `emits` from `publishes`; `EventSinkDef`).

**Tests:** `crates/corba-ccm/src/port.rs::{event_sink_subscription_round_trip,
list_event_sinks_unknown_source_empty}`,
`crates/corba-ccm/src/component_def.rs::{event_source_emit_only_default_false,
get_all_publishers_excludes_emit_only_sources,
get_all_emitters_includes_only_emit_only_sources,
get_named_publishers_respects_emit_only_flag,
get_named_emitters_excludes_publishers}`.

**Status:** done — event-port API live; wire path for emitters goes
through CosEvent (see `cos-event-service-1.4.md`).

#### §6.8 Homes / §6.9 Home Finders

**Spec:** §6.8/§6.9, pp. 34/42 — home lifecycle, HomeFinder.

**Repo:** `crates/corba-ccm/src/home.rs` (`HomeDef` with an optional
primary key, `HomeFinder` with `create`/`find_by_primary_key`/`remove`).

**Tests:** `crates/corba-ccm/src/home.rs::{home_def_default_has_no_primary_key,
keyed_home_has_primary_key, create_with_user_key_uses_key_as_id,
create_without_key_generates_system_id, find_by_primary_key_round_trip,
remove_decrements_instance_count, remove_unknown_returns_false,
home_get_component_def_returns_managed_component_id,
home_get_home_def_returns_self_repo_id}`.

**Status:** done

#### §6.10 Component Configuration / §6.11 Attributes

**Spec:** §6.10/§6.11 — configurator API, attribute set.

**Repo:** `crates/corba-ccm/src/lifecycle.rs` (`Configurator` trait,
`StandardConfigurator` over the `AttributeDef` set,
`ConfiguratorRegistry`), `crates/corba-ccm/src/context.rs`
(`ComponentContext`), `crates/corba-ccm/src/component_def.rs`
(`AttributeDef` with `readonly`/`set_raises`/`get_raises`).

**Tests:** `crates/corba-ccm/src/lifecycle.rs::{configurator_set_get_roundtrip,
configurator_rejects_unknown_attribute, configurator_rejects_readonly_attribute,
configurator_get_unknown_returns_unknown_attribute,
configurator_registry_register_and_lookup}`,
`crates/corba-ccm/src/component_def.rs::tests::attribute_readonly_skips_set_raises`,
`crates/corba-ccm/src/context.rs::{anonymous_context_returns_none,
principal_context_returns_bytes}`.

**Status:** done

#### §6.12 Component Inheritance

**Spec:** §6.12, p. 49 — inheritance of component definitions.

**Repo:** `crates/corba-ccm/src/component_def.rs::{ComponentTypeTable,
InheritanceError}` — `base_chain` walks the `base_component` links from
the derived type to the root, `is_equivalent_component_kind` checks a
repository ID against the whole chain, and `flatten` takes over the
facets, receptacles, event ports, attributes, `supports` interfaces and
primary key of the bases while refusing a redefined port or attribute
name and a second primary key.

**Tests:** `crates/corba-ccm/src/component_def.rs::{the_base_chain_runs_from_the_derived_component_to_the_root,
a_derived_component_is_also_of_every_kind_it_inherits,
flattening_takes_over_every_inherited_feature_bases_first,
an_unknown_base_is_reported_against_the_component_that_names_it,
an_unregistered_repository_id_is_not_a_component,
a_cycle_in_the_base_links_stops_the_walk,
a_redefined_port_name_is_refused_rather_than_shadowed,
a_second_primary_key_in_the_chain_is_refused,
a_keyed_base_makes_the_derived_component_keyed}`.

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

**Repo:** `crates/corba-ccm/src/cidl.rs::{Composition,
CompositionCategory, HomeExecutor, StorageType, StorageHome}` — the
data model of the CIDL constructs plus the category rule (`entity`
requires a storage home, the other three do not).

**Tests:** `crates/corba-ccm/src/cidl.rs::{session_composition_does_not_require_storage,
entity_composition_without_storage_invalid, entity_composition_with_storage_valid,
storage_type_can_inherit, home_executor_optional_component_binding,
all_four_composition_categories_distinct}`.

**Status:** partial — composition, home executor and the persistence
constructs (`storagetype`/`storagehome`) exist as an AST and are
checked through the category rule. What is missing: a CIDL parser (the
module says itself that the language is caller-layer), the segment
definition (a `segment` executor per facet group), facet/feature
delegation (`delegatesto`/`abstract storagehome`) and the proxy home.
The remainder is carried as its own item in `corba-3.3.open.en.md`.

#### §8 OMG CIDL — Remaining Constructs

**Spec:** Part 3 §8, p. 81 — the CIDL grammar itself, the `segment`
definition (one executor per facet group rather than one per
component), facet/feature delegation (`delegatesto`,
`abstract storagehome`) and the proxy home.

**Repo:** —

**Tests:** —

**Status:** open — only the data model of §8 above exists in the tree.
There is no lexer/parser for CIDL source (the module
`crates/corba-ccm/src/cidl.rs` says itself that the language is
caller-layer), no `Segment` type, no delegation clause and no proxy
home. Plan: attach the CIDL productions to the existing IDL front end
(`crates/idl-cst` + `crates/idl-parse`) and add the four constructs as
AST nodes; the data model of §8 is the target of the lowering.

### §9 CCM Implementation Framework (CIF)

**Spec:** Part 3 §9, p. 93 — CIF architecture, language mapping to C++.

**Repo:** `crates/corba-ccm/src/cif.rs::{ComponentExecutor,
SessionExecutor, KeyedExecutor, ExecutorLocator, CifError}`,
`crates/corba-codegen/src/skeleton.rs`,
`crates/corba-codegen/src/stub.rs`,
`crates/corba-ccm-lib/src/persistence.rs`.

**Tests:** `crates/corba-ccm/src/cif.rs::{lifecycle_calls_increment_counters,
cif_error_carries_diagnostic}` (executor lifecycle
`ccm_activate`/`ccm_passivate`/`ccm_remove`),
`crates/corba-codegen/src/skeleton.rs::{cpp_compares_with_strcmp_and_ends_in_bad_operation,
every_operation_reaches_the_dispatch_in_every_column}` and
`crates/corba-codegen/src/stub.rs::{a_variable_length_struct_returns_a_pointer_and_a_fixed_one_a_value,
an_object_reference_argument_is_a_ptr_and_its_out_is_an_out}`
for the C++ language mapping.

**Status:** done

### §10 The Container Programming Model

**Spec:** Part 3 §10, p. 135 — server and client programming
environments, basic/extended component programming interfaces.

**Repo:** `crates/corba-ccm/src/container.rs`,
`crates/corba-ccm/src/context.rs`, `crates/corba-ccm-lib/src/`
(persistence bridge, telemetry, DDS bridge).

**Tests:** `crates/corba-ccm/src/container.rs::tests`:
`install_and_full_lifecycle_round_trip`, `passivate_in_configured_state_invalid`,
`unknown_instance_returns_not_found`, `instance_count_tracks_install_and_remove`;
`crates/corba-ccm/src/context.rs::tests::anonymous_context_returns_none`,
`::principal_context_returns_bytes`;
`crates/corba-ccm-lib/src/dds_bridge.rs::tests` (7),
`crates/corba-ccm-lib/src/persistence.rs::tests` (8),
`crates/corba-ccm-lib/src/telemetry.rs::tests` (8).

**Status:** done

### §11 Integrating with Enterprise JavaBeans (EJB)

**Spec:** Part 3 §11, p. 177 — CCM↔EJB view mapping (CCM component →
EJB view, EJB bean → CCM view), TX bridging, naming glue.

**Repo:** `crates/corba-ccm-ejb/` with `connector_bean.rs`,
`naming_glue.rs`, `stub_gen.rs`, `tx.rs`.

**Tests:** `crates/corba-ccm-ejb/src/naming_glue.rs::tests` (8 tests):
`bind_and_lookup_round_trip`, `rebind_replaces_ior`, `unbind_removes_entry`,
`jndi_global_strips_prefix`, `jndi_comp_env_strips_prefix`,
`cos_to_jndi_round_trip_no_kind`, `cos_to_jndi_includes_kind`;
`crates/corba-ccm-ejb/src/tx.rs::tests` (8): `cos_to_jta_round_trip_for_all_states`,
`begin_then_commit_succeeds`, `rollback_only_then_commit_fails_with_rollback`;
`crates/corba-ccm-ejb/src/connector_bean.rs::tests` (4),
`crates/corba-ccm-ejb/src/stub_gen.rs::tests` (5).

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

**Tests:** `crates/corba-ccm/src/orb_core.rs::tests::xmi_emitter_empty_yields_minimal_doc`,
`::xmi_emitter_class_with_inheritance`, `::xmi_emitter_property_emits`;
`crates/corba-ccm/src/cif.rs::tests::lifecycle_calls_increment_counters`,
`::cif_error_carries_diagnostic`.

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

**Repo:** `crates/corba-dnc/src/plan.rs`, `crates/corba-dnc/src/node.rs`,
`crates/corba-dnc/src/execution.rs`, `crates/corba-dnc/src/container_host.rs`,
`crates/corba-dnc/src/repository.rs`.

**Tests:** `crates/corba-dnc/src/execution.rs::{full_lifecycle_round_trip,
invalid_plan_rejected_at_prepare, double_start_fails, destroy_before_running_fails}`
(the PIM→PSM prepare/start/destroy lifecycle),
`crates/corba-dnc/src/container_host.rs::{install_creates_container_on_demand,
install_then_activate_reaches_active, full_lifecycle_round_trip,
entity_and_session_share_host}` (the PSM→container mapping).

**Status:** done

### §16 Deployment IDL for CCM

**Spec:** Part 3 §16, p. 329 — D&C IDL.

**Repo:** as Rust representation in
`crates/corba-dnc/src/plan.rs` (`DeploymentPlan`,
`InstanceDeploymentDescription`, `PlanConnection`),
`crates/corba-dnc/src/repository.rs` (`RepositoryManager`,
`PackageConfiguration`), `crates/corba-dnc/src/node.rs`
(`NodeManager`, `NodeApplication`, `NodeApplicationManager`),
`crates/corba-dnc/src/execution.rs` (`ExecutionManager`,
`DomainApplicationManager`).

**Tests:** one test per IDL interface operation group —
`crates/corba-dnc/src/plan.rs::{valid_plan_passes_validation,
instance_with_unknown_impl_fails, connection_with_unknown_instance_fails,
co_location_with_unknown_instance_fails}`,
`crates/corba-dnc/src/repository.rs::{install_and_find_round_trip,
list_packages_returns_all_labels, delete_existing_returns_true,
delete_unknown_returns_false, install_overwrites_duplicate}`,
`crates/corba-dnc/src/node.rs::{prepare_plan_then_start_launch,
destroy_application_clears_active, register_and_unregister_application,
unregister_unknown_returns_none}`,
`crates/corba-dnc/src/execution.rs::tests::execution_manager_lists_prepared_plans`.

**Status:** done

### §17 XML Schema for CCM

**Spec:** Part 3 §17, p. 343 — XML schema for deployment plans.

**Repo:** `crates/corba-dnc/src/xml.rs`.

**Tests:** `crates/corba-dnc/src/xml.rs::{parses_minimal_plan,
parses_multiple_artifacts, validation_fails_on_unknown_impl_reference,
missing_root_tag_returns_expected_tag, co_locate_with_parsed}`.

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

**Tests:** `crates/corba-cosnaming/src/context.rs::tests` (13 tests):
`bind_and_resolve_simple`, `double_bind_yields_already_bound`,
`rebind_replaces_existing`, `nested_context_resolve`,
`resolve_to_context_returns_context`, `resolve_unknown_yields_missing_node`,
`unbind_removes_entry`, `destroy_with_bindings_yields_not_empty`,
`destroy_empty_is_ok`, `list_returns_all_bindings_with_correct_type`,
`resolve_str_via_naming_context_ext`, `to_url_constructs_corbaname`,
`empty_name_is_invalid`;
`crates/corba-cosnaming/src/stringified.rs::tests` (9),
`crates/corba-cosnaming/src/name.rs::tests` (3).

**Status:** done

---

## ZeroDDS-specific bridges (no OMG item)

### CORBA object ↔ DDS topic bridge

**Spec:** no OMG spec item; ZeroDDS-specific migration layer
for the incremental replacement of CORBA endpoints by DDS wire.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs` (IOR↔topic+instance),
`crates/corba-dds-bridge/src/servant.rs` (CORBA servant over DDS-RPC),
`crates/corba-dds-bridge/src/sync.rs`.

**Tests:** `crates/corba-dds-bridge/src/mapping.rs::tests` (6 tests):
`add_and_lookup_route`, `lookup_unknown_yields_none`,
`operation_mapping_by_name`, `add_route_replaces_existing`,
`all_routes_listing`, `direction_variants_distinct`;
`crates/corba-dds-bridge/src/locate.rs::tests` (4),
`crates/corba-dds-bridge/src/notify.rs::tests` (4),
`crates/corba-dds-bridge/src/csiv2_wire.rs::tests` (4),
`crates/corba-dds-bridge/src/bridge_security.rs::tests` (5).

**Status:** done — informative; not spec-bound.

---

## Audit status

50 done / 1 partial / 1 open / 12 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-corba-giop --lib` — 80 tests green.
* `cargo test -p zerodds-corba-iiop --lib` — 38 tests green.
* `cargo test -p zerodds-corba-ior --lib` — 53 tests green.
* `cargo test -p zerodds-corba-poa --lib` — 39 tests green.
* `cargo test -p zerodds-corba-cosnaming --lib` — 25 tests green.
* `cargo test -p zerodds-corba-csiv2 --lib` — 35 tests green.
* `cargo test -p zerodds-corba-cos-event --lib` — 23 tests green.
* `cargo test -p zerodds-corba-ccm --lib` — 181 tests green.
* `cargo test -p zerodds-corba-ccm-lib --lib` — 23 tests green.
* `cargo test -p zerodds-corba-ccm-ejb --lib` — 25 tests green.
* `cargo test -p zerodds-corba-ir --lib` — 23 tests green.
* `cargo test -p zerodds-corba-dnc --lib` — 30 tests green.
* `cargo test -p zerodds-corba-dds-bridge --lib` — 43 tests green.
* `cargo test -p zerodds-corba-codegen --tests` — 38 + 15 + 10 tests green.
* `cargo test -p zerodds-idl-parse --tests` — 36 + 32 tests green.
* `cargo test -p zerodds-idl-lower --tests` — 5 + 49 + 23 + 12 + 36 + 4 +
  82 + 3 + 62 tests green.

Open items and the reason for the `partial` status of Part 3 §8:
`corba-3.3.open.en.md`.
