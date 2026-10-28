# OMG CORBA 3.3 — Spec-Coverage (WP CORBA-Coexistence)

**Quelle:** OMG CORBA 3.3 — drei Bände als formal-Dokumente:
* Part 1 (Interfaces, 532 S.) — [`formal/12-11-12`](https://www.omg.org/spec/CORBA/3.3/Interfaces/PDF)
* Part 2 (Interoperability, 249 S.) — [`formal/12-11-14`](https://www.omg.org/spec/CORBA/3.3/Interoperability/PDF)
* Part 3 (Component Model, 380 S.) — [`formal/12-11-16`](https://www.omg.org/spec/CORBA/3.3/Components/PDF)

**Kontext.** ZeroDDS deckt mit dem DDS-Kern und dem Codegen aus
`crates/lang-rules/` und `crates/corba-codegen/` das DDS-PSM
vollständig ab. Für die Migration von Bestands-CORBA-Anwendungen in
der Finanzindustrie soll ZeroDDS als **Drop-in-Backend** auftretbar
sein: ein bestehender CORBA-Client kompiliert und linkt gegen
Annex-A.1-Stubs, läuft gegen ZeroDDS-Endpoints, die GIOP/IIOP-Wire
sprechen, und kann schrittweise auf reines DDS migriert werden ohne
ad-hoc Infrastruktur-Tausch.

Die CORBA-Coexistence ist über mehrere Implementierungs-Crates verteilt:

- `crates/idl-parse/` — IDL-Grammatik (Part 1 §7 IDL-Syntax)
- `crates/corba-codegen/` — Annex-A.1-Codegen (Stubs/Skeletons, Part 1 §7)
- `crates/corba-ir/` — Interface Repository (Part 1 §14)
- `crates/corba-poa/` — Portable Object Adapter (Part 1 §15)
- `crates/corba-giop/` — GIOP-Wire-Protokoll (Part 2 §9)
- `crates/corba-iiop/` — IIOP-Transport (Part 2 §9.7)
- `crates/corba-ior/` — IOR-Kodierung (Part 2 §9.4 / §10.5)
- `crates/corba-csiv2/` — CSIv2-Security (Part 2 §10)
- `crates/corba-ccm/` — Component Model (Part 3 §6-§9)
- `crates/corba-ccm-lib/` — CCM-Container (Part 3 §10)
- `crates/corba-ccm-ejb/` — EJB-Integration (Part 3 §11)
- `crates/corba-dnc/` — Deployment & Configuration (Part 3 §15-§17)
- `crates/corba-dds-bridge/` — ZeroDDS-Bridge (kein OMG-Item)
- `crates/corba-cosnaming/` — COS Naming v1.3 (separate Spec)
- `crates/corba-cos-event/` — COS Event Service v1.2 (separate Spec)
- `crates/dcps/` — DDS-PSM-Kern (DDS-Migrationsziel)
- `crates/rpc/` — DDS-RPC-Kern (DDS-Migrationsziel)

CosNaming und CosEventService sind eigenständige OMG-Specs (formal/04-10-03
und formal/04-10-02); CosEvent hat eine eigene Spec-Coverage-Datei
(`cos-event-service-1.4.md`). CosNaming-Coverage wird hier inline mitgeführt,
weil CORBA Part 1 §8.5.2 sie als Standard-Initial-Reference verlangt.

---

## Part 1: Interfaces

### §1 Scope

**Spec:** Part 1 §1, S. 1 — Geltungsbereich der CORBA-Spezifikation.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §2 Conformance and Compliance

**Spec:** Part 1 §2, S. 1 — Konformitätspunkte, Compliance-Marker.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §3 Normative References

**Spec:** Part 1 §3, S. 1 — Liste normativer Referenzen.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §4 Additional Information

**Spec:** Part 1 §4, S. 2 — Outline + Keyword-Konventionen
(MUST/SHALL/SHOULD/MAY).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §5 The Object Model

**Spec:** Part 1 §5, S. 3 — abstraktes Objektmodell (Objects, Requests,
Types, Interfaces, Value Types, Operations, Attributes).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — konzeptioneller Rahmen, keine Wire-/
Code-Pflicht.

### §6 CORBA Overview

**Spec:** Part 1 §6, S. 11 — Architektur-Überblick (ORB, Stubs, DSI,
Object Adapter, IR).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §7 IDL Syntax and Semantics

**Spec:** Part 1 §7, S. 27 — komplette IDL-Grammatik (Lexer, Module,
Interface, Value, Constant, Type, Exception, Operation, Attribute,
Repository-Identity, Event, Component, Home, Names & Scoping).

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

**Status:** done — IDL 4.2 ist Superset zu CORBA-IDL §7. Rule-für-Rule
Coverage in `docs/spec-coverage/idl-4.2.md` (649 done / 4 partial).
Repository-Identity (§7.15) ist via `corba-codegen::repository_id`
implementiert. Component- und Home-Declaration (§7.17/§7.18) werden
in Part 3 §6 erneut gefasst und sind dort `done`.

### §8 ORB Interface

**Spec:** Part 1 §8, S. 93 — ORB-Pseudo-Objekt: `ORB_init`,
`string_to_object`/`object_to_string`, `resolve_initial_references`,
Policy-Management, TypeCodes, Standard-Exceptions.

**Repo:** `crates/corba-ior/src/stringified.rs` (string↔IOR),
`crates/corba-ior/src/url.rs` (corbaloc/corbaname),
`crates/corba-poa/src/policies.rs` (Policy-Set für POA-Scope),
`crates/corba-ir/src/type_code.rs` (TypeCode-Repräsentation),
`crates/corba-giop/src/error.rs` + `crates/corba-poa/src/error.rs`
(Standard-Exceptions als Rust-Result).

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

**Status:** done — String-↔-IOR und URL-Schemes (§8.2.2, §8.5.2)
abgedeckt; ORB-Singleton-Lifecycle (`Orb::init/shutdown/destroy`),
Threading-Operations (`ThreadingMode::SingleThreaded/ThreadPerRequest/
ThreadPool`), Policy-Domain-Manager (`Orb::set_policy/get_policy`)
in `crates/corba-ccm/src/orb_core.rs`. Standard-Exceptions als
Rust-Result-Layer.

### §9 Value Type Semantics

**Spec:** Part 1 §9, S. 155 — Valuetype-Marshaling (CDR-Repräsentation,
Boxed-Value, Custom Marshaling, Sending-Context-Run-Time).

**Repo:** `crates/wire/src/writer.rs` + `crates/wire/src/reader.rs`
(Codec-Basis), `crates/idl-parse/src/decl.rs` (`parse_valuetype`,
Value-Box und `custom valuetype`), `crates/lang-rules/src/coverage.rs`
(Valuetype-Regel je Zielsprache), kein dediziertes Custom-Marshal-Hook
im Codegen.

**Tests:** `crates/idl-lower/tests/lowering.rs::{the_other_valuetype_forms_are_unaffected_by_the_box,
an_abstract_valuetype_parses_as_a_valuetype,
a_forward_declared_valuetype_is_completed_by_its_definition}`,
`crates/corba-codegen/tests/orb_agreement.rs::the_valuebase_holder_is_the_omgs_and_jacorbs_names_a_class_that_does_not_exist`.

**Status:** done — Valuetype-IDL parst, lowert und trägt in
`lang-rules` eine Emissions-Regel je Zielsprache. Custom-Marshal-Streams via
`crates/corba-ccm/src/orb_core.rs::StreamableValue`-Trait
(`marshal/unmarshal/repository_id`); Sending-Context-Run-Time via
`SendingContext` mit `code_set` + `truncation_codebase`.

**Tests:** `crates/corba-codegen/tests/annex_a1_traits.rs`:
`table_a8_reads_abstract_and_truncatable_off_the_declaration`,
`table_a9_gives_a_valuebox_its_boxed_trait`,
`every_traited_type_renders_in_every_column`.

**Status:** done — Valuetype und Valuebox tragen in allen vier PSM-Spalten
die Member, die Annex A.1.1 ihnen gibt.

### §10 Abstract Interface Semantics

**Spec:** Part 1 §10, S. 171 — `abstract interface`, Marshalling als
Object oder ValueType.

**Repo:** `crates/idl-parse/src/decl.rs` (`abstract` gehört zu zwei
Produktionen — Interface-Header (79) und `<value_abs_def>` (127); die
Weiche steht in `parse_definition`), Codegen-Zeile in
`crates/corba-codegen/tests/orb-corpus/rows/03-abstractbase.idl`.

**Tests:** `crates/idl-lower/tests/lowering.rs::abstract_still_reaches_the_interface_header_it_also_belongs_to`,
`crates/idl-lower/tests/interface_as_type.rs::an_interface_used_as_a_type_resolves_in_every_spelling`
(Zeile „an abstract interface").

**Status:** done — Parser akzeptiert `abstract interface`, das Lowering
führt es als eigenen Interface-Kind, und der Codegen-Korpus enthält die
Zeile.

### §11 Dynamic Invocation Interface (DII)

**Spec:** Part 1 §11, S. 175 — Request/NVList/NamedValue-API für
dynamische Methoden-Aufrufe ohne kompilierten Stub.

**Repo:** `crates/corba-ccm/src/dynamic_api.rs::{Request, NvList,
NamedValue, ArgFlag}` mit `add_in_arg`/`add_out_arg`-Operations,
`ArgFlag::{In, Out, InOut}` (Spec §11.1.2 Wire-Werte 1/2/3) und
NVList-`add_value`/`count` (Spec §11.1.3).

**Tests:** `dynamic_api::tests::{arg_flag_round_trip,
arg_flag_unknown_value_rejected, nvlist_add_value_increments_count,
dii_request_add_in_arg, dii_request_add_out_arg,
dii_encode_giop_request_concatenates_input_args,
dii_encode_giop_request_inout_treated_as_input,
dii_encode_giop_request_round_trip_via_giop_codec}`.

**Status:** done — Daten-Modell + Wire-up: `Request::encode_giop_request(request_id, object_key)`
liefert ein `corba_giop::Request`-Frame mit konkatenierten In/InOut-
Args; Roundtrip via `corba_giop::encode_message` + `decode_message`
ist getestet. Spec §11.2 + §15.4.2.

### §12 Dynamic Skeleton Interface (DSI)

**Spec:** Part 1 §12, S. 191 — `ServerRequest`-Pseudo-Objekt für
generische Server-Implementations.

**Repo:** `crates/corba-ccm/src/dynamic_api.rs::ServerRequest` mit
`set_result(value)`/`set_exception(id, value)`-Operations.

**Tests:** `dynamic_api::tests::{dsi_server_request_set_result,
dsi_server_request_set_exception, dsi_input_body_concatenates_in_and_inout,
dsi_servant_default_dispatch_via_input_body}`.

**Status:** done — `ServerRequest`-Daten-Modell + `DsiServant`-Trait
(Spec §12) als generischer Server-Side-Dispatch-Pfad. Wegen Layer-
Trennung (corba-ccm Layer 8.3 / corba-poa Layer 8.16) ist der Trait
orthogonal zu `corba-poa::Servant` definiert und kann zusätzlich
implementiert werden. `ServerRequest::input_body()` konkateniert
In/InOut-Args zu einem flachen Body; ein Mock-Servant in den Tests
zeigt den Echo-Roundtrip.

### §13 Dynamic Management of Any Values (DynAny)

**Spec:** Part 1 §13, S. 195 — DynAny-API zum Iterieren über `any`-
Inhalte ohne TypeCode-spezifische Codegen.

**Repo:** `crates/corba-ccm/src/dynamic_api.rs::{DynAny, DynAnyKind}`
mit `equal/from_any/to_any`-Operations + Type-Discriminator-Enum
(Spec §13.1: Primitive/Struct/Union/Enum/Sequence/Array/Fixed/
Value/ValueBox).

**Tests:** `dynamic_api::tests::{dyn_any_round_trip, dyn_any_equal_same_value,
dyn_any_not_equal_different_kind, dyn_any_from_any_round_trip,
dyn_any_kind_variants_are_distinct, dyn_any_from_type_code_long_round_trip,
dyn_any_from_type_code_long_rejects_truncated_buffer,
dyn_any_from_type_code_sequence_preserves_bytes,
dyn_any_from_type_code_struct_preserves_bytes_and_id}`.

**Status:** done — DynAny-Daten-Modell + Wire-up `from_type_code(tc, raw)`
walked einen `corba_ir::TypeCode` über CDR-`any`-Bytes; primitive
Typen (`Long`/`ULong`/`Short`/`Boolean`/`String`/...) werden via
`zerodds_wire::Reader` validiert (Decode-Fehler werden
propagiert), komplexe Typen (Struct/Sequence/Array) preserven die
rohen Bytes plus Repository-ID. `to_cdr()` projiziert die DynAny
zurück.

### §14 The Interface Repository

**Spec:** Part 1 §14, S. 219 — IR als CORBA-Service mit `Container`/
`Contained`/`IDLType` Interface-Hierarchie; Repository-IDs;
Component-IR-Erweiterungen (Part 3); IDL des IR.

**Repo:** `crates/corba-ir/src/repository.rs` (`Repository`,
`Container`-Hierarchie), `crates/corba-ir/src/repository_id.rs`
(IDL-/Local-Format Repository-IDs), `crates/corba-ir/src/type_code.rs`
(TypeCode-Backend), `crates/corba-ir/src/definition_kind.rs` (DK_*).

**Tests:** `crates/corba-ir/src/repository.rs::tests` (11 Tests):
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

**Status:** done — IR-Interface-Hierarchie + RepositoryIds + TypeCodes
implementiert. Component-IR-Erweiterungen siehe Part 3 §12 (CCM-
Metamodel).

### §15 The Portable Object Adapter (POA)

**Spec:** Part 1 §15, S. 301 — POA-Architektur: Policies (Lifespan,
IdAssignment, IdUniqueness, ServantRetention, RequestProcessing,
ImplicitActivation, Thread), Object-Activation, Servant-Manager,
Adapter-Activator, POA-Hierarchie, IDL für PortableServer.

**Repo:** `crates/corba-poa/src/poa.rs` (Hierarchie + Dispatch),
`crates/corba-poa/src/policies.rs` (alle 7 Standard-Policies),
`crates/corba-poa/src/active_object_map.rs` (AOM),
`crates/corba-poa/src/servant.rs` + `crates/corba-poa/src/servant_manager.rs`,
`crates/corba-poa/src/object_id.rs`,
`crates/corba-poa/src/poa_manager.rs` (Aktivierungs-Lifecycle).

**Tests:** `crates/corba-poa/src/poa.rs::tests` (12 Tests):
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

**Status:** done — alle 7 Standard-Policies + Servant-Manager-Pfad
+ AOM + POA-Hierarchie + Dispatch implementiert. Adapter-Activator
(§15.3.6) als `ServantManager`-Variante mitabgedeckt; falls strikte
Trennung verlangt, wäre das ein kleiner Refactor (≤0.5 PW).

### §16 Portable Interceptors

**Spec:** Part 1 §16, S. 357 — Client-/Server-Request-Interceptor-API,
PolicyFactory-Registrierung, IORInterceptor (TaggedComponent-Inject),
PI-Initial-Reference (`PICurrent`).

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::{ClientRequestInterceptor,
ServerRequestInterceptor, IorInterceptor, InterceptorRegistry,
PolicyFactory, PiCurrent, ClientInterceptionPoint,
ServerInterceptionPoint}` mit allen 5 Client-Points
(send_request/send_poll/receive_reply/receive_exception/
receive_other) + 5 Server-Points (receive_request_service_contexts/
receive_request/send_reply/send_exception/send_other) +
Registry + PiCurrent-Slot-Storage. Pipeline-Walks (`walk_client`,
`walk_server`, `walk_ior`) sind in `crates/corba-iiop/src/connection.rs::
Connection::with_interceptors` verdrahtet: `read_message`/
`write_message` walken automatisch `SendRequest`/`ReceiveReply`
(Client) bzw. `ReceiveRequest`/`SendReply` (Server) wenn eine
Registry installiert ist.

**Tests:** `orb_extensions::tests::{registry_add_increments_counts,
picurrent_set_get_slot, client_interception_points_distinct,
server_interception_points_distinct, registry_walk_client_invokes_all_client_interceptors,
registry_walk_ior_collects_tags}` +
`crates/corba-iiop/src/connection.rs::{pipeline_walks_client_send_request,
pipeline_walks_server_receive_request, ior_interceptor_fires_on_walk_ior}`.

**Status:** done — PI-Daten-Modell + Pipeline-Integration in
IIOP-Connection-Send/Receive-Pfad.

### §17 CORBA Messaging

**Spec:** Part 1 §17, S. 415 — Asynchrone Messaging (AMI), Time-
Independent Invocations (TII), QoS-Policies (RebindPolicy,
SyncScopePolicy etc.).

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::{MessagingPolicy,
AmiReplyHandler, AmiReplySink, dispatch_async_reply,
PersistentRequestStore, PersistentRequestEntry}` mit allen 10
Policy-Types (Rebind/SyncScope/RequestPriority/ReplyPriority/Routing/
MaxHops/RequestTime/ReplyTime/RelativeRoundtripTimeout/
RoutingTypeRange) + Wire-Mapping `policy_type() -> u32` nach OMG
`Messaging.idl` (formal/2011-11-02 §B.5.1). AMI-Reply-Dispatch
(`dispatch_async_reply`) mappt einen `corba_giop::Reply` auf die drei
Spec-Callbacks `handle_reply` / `handle_excep` / `handle_other`. TII
(Time-Independent-Invocations) liefert `PersistentRequestStore` mit
`add` / `poll` / `timeout_expired`.

**Tests:** `orb_extensions::tests::{messaging_policies_distinct,
ami_reply_handler_distinct, messaging_policy_wire_values_match_omg_messaging_idl,
ami_handler_handles_no_exception_reply, ami_handler_handles_user_exception_reply,
persistent_request_store_add_poll_timeout}`.

**Status:** done — AMI-Reply-Dispatch-Bridge auf `corba_giop::Reply`
+ TII Persistent-Request-Store + Wire-Mapping aller 10 Messaging-
Policies.

### §18 Compression

**Spec:** Part 1 §18, S. 491 — Generic Compression Framework für
GIOP-Payloads (Codec-Registrierung, Negotiation).

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::{CompressionAlgorithm,
ZiopConfig}` mit None/Zlib/Gzip/Lzma/Deflate-Algorithmen +
ZiopConfig (algorithm/min_size_threshold/level).

**Tests:** `orb_extensions::tests::{compression_algorithm_round_trip,
compression_algorithm_unknown_rejected, ziop_config_default_no_compression,
compression_none_passes_through, compression_zlib_round_trip,
compression_gzip_round_trip, compression_deflate_round_trip,
compression_lzma_returns_unsupported, compression_zlib_handles_large_block}`.

**Status:** done — Compression-Framework + produktiver Codec via
`flate2` (Pure-Rust-Backend): `CompressionAlgorithm::compress(input)`
und `decompress(input)` decken `None` (passthrough), `Zlib` (RFC 1950),
`Gzip` (RFC 1952), `Deflate` (RFC 1951) ab. `Lzma` liefert
`CompressionError::Unsupported` mit Decision-Record (xz2/liblzma-
Build-Risiko unverhältnismäßig). 10-kB-Block-Test belegt das
Roundtrip-Verhalten.

---

## Part 2: Interoperability

### §1-§5 Scope/Conformance/References/Terms/Symbols

**Spec:** Part 2 §1-§5, S. 1-6 — Geltungsbereich, Konformitätspunkte,
Begriffsdefinitionen.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §6 Interoperability Overview

**Spec:** Part 2 §6, S. 7 — Architektur-Überblick zur ORB-Interop.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §7 ORB Interoperability Architecture

**Spec:** Part 2 §7, S. 15 — Bridges, Domains, Object References im
Interop-Kontext.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — konzeptionelle Architektur; konkrete
Implementations-Pflichten in §8-§12.

### §8 Building Inter-ORB Bridges

**Spec:** Part 2 §8, S. 63 — Inline-Bridges, Request-Level-Bridges.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs` (CORBA↔DDS Mapping),
`crates/corba-dds-bridge/src/servant.rs`, `sync.rs` +
`crates/corba-ccm/src/orb_extensions.rs::{BridgeMode, BridgeConfig}`
für Inline/Request-Level-Mode-Klassifikation.

**Tests:** 15 inline `#[test]`-Funktionen in `crates/corba-dds-bridge/src/`
+ `orb_extensions::tests::{bridge_modes_distinct, bridge_config_construct}`.

**Status:** done — DDS-Bridge (Request-Level) + Inline-Bridge-API
über BridgeMode/BridgeConfig live; Wire-Implementation der
generischen Inline-Bridge ist Caller-Layer.

### §9 General Inter-ORB Protocol (GIOP)

**Spec:** Part 2 §9, S. 69 — GIOP Wire-Protokoll (alle 8 Message-Typen),
CDR-Transfer-Syntax, Versionierung 1.0/1.1/1.2, IIOP-Mapping, Bi-Dir
GIOP.

**Repo:** `crates/corba-giop/src/` mit einer Datei je Message-Typ plus
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

**Spec:** §9.3, S. 71 — CDR-Encoding, Endianness-Markierung,
Primitive/Konstrukt-Layout, GIOP 1.2 Alignment.

**Repo:** `crates/wire/src/writer.rs` + `crates/wire/src/reader.rs`
(Primitive + Alignment + Encapsulation), Re-Use durch den
GIOP-Codec in `crates/corba-giop/src/codec.rs`.

**Tests:** `crates/corba-giop/src/codec.rs::{giop_1_2_body_is_eight_aligned_from_the_message_start,
giop_1_2_reply_body_is_eight_aligned_from_the_message_start,
giop_1_0_body_is_not_padded}`,
`crates/corba-giop/src/request.rs::tests::giop_1_2_request_body_is_8_aligned`,
`crates/corba-giop/src/service_context.rs::{round_trip_with_two_entries_be,
round_trip_le_byte_order}`.

**Status:** done

#### §9.4 GIOP Message Formats

**Spec:** §9.4, S. 93 — alle 8 Message-Typen mit binärem Layout
(Request/Reply/CancelRequest/LocateRequest/LocateReply/
CloseConnection/MessageError/Fragment).

**Repo:** Eine Datei pro Message-Typ in `crates/corba-giop/src/` —
`crates/corba-giop/src/request.rs`, `crates/corba-giop/src/reply.rs`,
`crates/corba-giop/src/cancel_request.rs`,
`crates/corba-giop/src/locate_request.rs`,
`crates/corba-giop/src/locate_reply.rs`,
`crates/corba-giop/src/close_connection.rs`,
`crates/corba-giop/src/message_error.rs`,
`crates/corba-giop/src/fragment.rs`.

**Tests:** Ein Oktett-Round-Trip je Message-Typ —
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
round_trip_giop_1_1_without_header}`. Alle acht auch über den
Gesamt-Codec: `crates/corba-giop/src/codec.rs::{round_trip_request_giop_1_0_be,
round_trip_close_connection, round_trip_message_error,
round_trip_fragment_with_more_bit, round_trip_cancel_request,
round_trip_locate_request_giop_1_2, round_trip_locate_reply_object_here}`.
Die Typ-Diskriminatoren selbst:
`crates/corba-giop/src/message_type.rs::tests::message_type_values_match_spec_table_15_1`.

**Status:** done

#### §9.5 GIOP Message Transport

**Spec:** §9.5, S. 107 — Transport-Anforderungen (geordnet, ohne
Verlust), Connection-Management.

**Repo:** `crates/corba-iiop/src/connection.rs`,
`crates/corba-iiop/src/acceptor.rs`,
`crates/corba-iiop/src/connector.rs` für TCP-Transport.

**Tests:** `crates/corba-iiop/src/acceptor.rs::tests::round_trip_via_acceptor_and_connector`,
`crates/corba-iiop/src/connection.rs::{round_trip_request_reply,
shutdown_propagates_eof_to_peer, close_connection_message_round_trip}`,
`crates/corba-iiop/src/framing.rs::tests::read_back_to_back_messages`
(Message-Grenzen im Strom),
`crates/corba-iiop/src/connector.rs::{connect_reuses_pooled_connection,
invalidated_connection_is_not_returned_to_pool,
max_connections_per_endpoint_is_enforced}`.

**Status:** done — TCP via IIOP. Andere Transport-Bindings (z.B.
SCTP) nicht angeboten.

#### §9.6 Object Location

**Spec:** §9.6, S. 110 — LocateRequest/LocateReply-Semantik, Forward-
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

**Spec:** §9.7, S. 111 — IIOP als TCP-Mapping; IIOP::ProfileBody
(Host/Port/Object-Key + TaggedComponents).

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

**Spec:** §9.8 + §9.9, S. 115-118 — BiDirIIOP-ServiceContext,
BiDirGIOP-Policy.

**Repo:** `crates/corba-iiop/src/bidir.rs` (Wire-Codec) +
`crates/corba-ccm/src/orb_extensions.rs::{BiDirPolicy,
BiDirServiceContext}` (Policy-Lifecycle: Normal/Both;
listen_points-Liste für §9.9.1).

**Tests:** Inline-Tests in `bidir.rs` +
`orb_extensions::tests::{bidir_policy_distinct,
bidir_service_context_listen_points}`.

**Status:** done — BiDir-Codec + Policy-Lifecycle live.

#### §9.10 OMG IDL für GIOP

**Spec:** §9.10, S. 119 — IDL für GIOP-Module-Konstanten + Service-
Context-IDs.

**Repo:** als Rust-Konstanten in `crates/corba-giop/src/service_context.rs`
(`ServiceContextTag`), `crates/corba-giop/src/message_type.rs` (`MessageType`),
`crates/corba-giop/src/flags.rs` (`Flags`),
`crates/corba-giop/src/version.rs` (`Version`),
`crates/corba-giop/src/reply.rs` (`ReplyStatusType`),
`crates/corba-giop/src/locate_reply.rs` (`LocateStatusType`),
`crates/corba-giop/src/target_address.rs` (`AddressingDisposition`).

**Tests:** Ein Wert-Vergleich gegen die IDL je Konstanten-Gruppe —
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

**Spec:** Part 2 §10, S. 125 — Common Secure Interoperability v2:
SAS-Protokoll, GSSUP-Token, Transport-Mechanismen (TLS),
Authorization-Tokens, IOR-Komponenten (TAG_CSI_SEC_MECH_LIST).

**Repo:** `crates/corba-csiv2/` mit `sas.rs`, `gssup.rs`, `mech_list.rs`,
`association_options.rs`; IOR-Tag-Codec in `crates/corba-ior/src/component_tags.rs`.

**Tests:** `crates/corba-csiv2/src/sas.rs::tests` (9 Tests):
`sas_msg_type_values_match_spec`,
`sas_context_body_roundtrip_all_messages_be_le`,
`server_eval_decodes_foreign_gss_wrapped_establish_context`,
`identity_token_discriminator_matches_spec`,
`establish_context_holds_full_payload`, `complete_context_indicates_stateful`,
`context_error_carries_gss_status`;
`crates/corba-csiv2/src/session.rs::tests` (9), darunter
`a_stateful_target_answers_establish_with_a_stateful_complete`,
`a_full_session_survives_the_wire_in_both_byte_orders`;
`crates/corba-csiv2/src/gssup.rs::tests::gssup_byte_identical_to_jacorb`,
`::initial_context_token_tag_is_0x60`,
`::encapsulation_is_wire_correct_octet_seq_len_at_offset_4`.

**Status:** done

#### §10.2 Protocol Message Definitions

**Spec:** §10.2, S. 127 — SAS-Message-Codec (EstablishContext,
CompleteEstablishContext, ContextError, MessageInContext).

**Repo:** `crates/corba-csiv2/src/sas.rs` (`SasMessage` mit
`encode_encapsulation`/`decode_encapsulation`, `IdentityToken`).

**Tests:** `crates/corba-csiv2/src/sas.rs::{sas_msg_type_values_match_spec,
sas_context_body_roundtrip_all_messages_be_le,
sas_identity_token_absent_anonymous_roundtrip,
identity_token_discriminator_matches_spec, establish_context_holds_full_payload,
complete_context_indicates_stateful, message_in_context_can_request_discard,
context_error_carries_gss_status}`.

**Status:** done

#### §10.3 Security Attribute Service (SAS)

**Spec:** §10.3, S. 137 — SAS-State-Machine, Stateful/Stateless
Contexts.

**Repo:** `crates/corba-csiv2/src/session.rs::{SasTarget, SasClient,
TargetAction, RetainedContext, CONTEXT_ERROR_NO_CONTEXT,
CONTEXT_ERROR_INVALID_EVIDENCE}` — Target- und Client-Hälfte der
State-Machine über den `SasMessage`-Typen aus §10.2; ein
stateful Target hält den Kontext unter der `ContextId` und der Client
schickt danach `MessageInContext` statt neuer Credentials, bis ein
`discard_context` oder ein `ContextError` ihn zurück in den
Establish-Zustand setzt.

**Tests:** `crates/corba-csiv2/src/session.rs::{a_stateful_target_answers_establish_with_a_stateful_complete,
a_stateless_target_retains_nothing_and_says_so,
the_second_request_on_a_stateful_context_is_a_message_in_context,
a_stateless_client_re_establishes_on_every_request,
discarding_a_context_makes_the_next_reference_to_it_a_context_error,
a_message_in_context_for_an_unknown_id_is_refused,
a_context_error_puts_the_client_back_into_the_establish_state,
a_target_to_client_message_arriving_at_a_target_is_not_evidence,
a_full_session_survives_the_wire_in_both_byte_orders}`.

**Status:** done — der letzte Test fährt die volle Sitzung über die
§10.2-Oktette: jede Nachricht kreuzt `encode_encapsulation` /
`decode_encapsulation`, bevor die Gegenseite sie sieht.

#### §10.4 Transport Security Mechanisms

**Spec:** §10.4, S. 149 — TLS/SSL-Bindings, Mutual-Auth.

**Repo:** TLS-Stack via `crates/security-pki/` (X.509 + TLS).
CSIv2-→-IIOP-TLS-Bind via `crates/corba-csiv2::association_options`
+ `crates/corba-iiop`-Acceptor (TAG_TLS_SEC_TRANS-Profile-Tags
in `crates/corba-ior/src/component_tags.rs`); Glue ist im
Acceptor-Lifecycle gebunden.

**Tests:** `crates/corba-csiv2/src/mech_list.rs::tests`:
`build_mech_list_with_tls_and_gssup`, `empty_list_has_no_mechanisms`,
`cdr_roundtrip_compound_sec_mech_list`,
`octet_seq_seq_rejects_oversized_count`,
`compound_sec_mech_list_rejects_oversized_count`, `cdr_roundtrip_empty_list`;
`crates/corba-csiv2/src/association_options.rs::tests::flag_values_match_spec_table_24_1`,
`::has_and_with_round_trip`;
`crates/corba-ior/src/component_tags.rs::tests::well_known_component_tag_values_match_spec`,
`::round_trip_all_known_tags`, `::at_least_32_well_known_tags_modelled`.

**Status:** done — TLS-Stack + CSIv2-IIOP-Bind via IOR-Tags live.

#### §10.5 Interoperable Object References (IOR mit Security-Tags)

**Spec:** §10.5, S. 150 — IOR-Format mit TAG_CSI_SEC_MECH_LIST,
TAG_NULL_TAG, TAG_TLS_SEC_TRANS.

**Repo:** `crates/corba-ior/src/component_tags.rs` (`ComponentId` mit
`CsiSecMechList` = 33, `NullTag` = 34, `TlsSecTrans` = 36),
`crates/corba-ior/src/components.rs` (`StructuredComponent`),
`crates/corba-csiv2/src/mech_list.rs` (`CompoundSecMechList`).

**Tests:** `crates/corba-ior/src/ior.rs::tests::a_security_ior_carries_mech_list_tls_and_null_tag_through_a_round_trip`
— ein vollständiger IOR mit den drei Tags in der IIOP-Profile-
Component-Liste, über `encode_encapsulation`/`decode_encapsulation`
und zurück in die strukturierte Form. Tag-Werte und die
Component-Bodies einzeln:
`crates/corba-ior/src/component_tags.rs::{well_known_component_tag_values_match_spec,
round_trip_all_known_tags, at_least_32_well_known_tags_modelled}`,
`crates/corba-ior/src/components.rs::{csi_sec_mech_list_round_trip,
tls_sec_trans_with_addresses_round_trip, ssl_sec_trans_round_trip}`.

**Status:** done

#### §10.6 Conformance Levels (CSIv2)

**Spec:** §10.6, S. 160 — vier CSIv2-Conformance-Level (0-3).

**Repo:** Implementierte Mechanismen in `corba-csiv2` decken alle
drei normativen Levels (0/1/2) ab; Conformance-Marker in
`crates/corba-ccm/src/lib.rs::conformance::{CORBA_PART2_10_6_CSIV2_LEVEL_0,
CORBA_PART2_10_6_CSIV2_LEVEL_1, CORBA_PART2_10_6_CSIV2_LEVEL_2}`.

**Tests:** `conformance_tests::csiv2_level_markers_match_spec`.

**Status:** done — Conformance-Marker für alle drei Levels
explizit ausgewiesen.

#### §10.7 Sample Message Flows and Scenarios

**Spec:** §10.7, S. 162 — Beispiele.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §10.8 References

**Spec:** §10.8, S. 171 — Referenzliste.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §10.9 IDL für CSIv2

**Spec:** §10.9, S. 172 — IDL der CSI-Module.

**Repo:** als Rust-Repräsentation in
`crates/corba-csiv2/src/association_options.rs` (`AssociationOptions`),
`crates/corba-csiv2/src/mech_list.rs` (`CompoundSecMechList`,
`CompoundSecMech`, `AsContextSec`, `SasContextSec`),
`crates/corba-csiv2/src/sas.rs` (`SasMsgType`, `IdentityToken`,
`SASContextBody`-Union als `SasMessage`),
`crates/corba-csiv2/src/gssup.rs` (`GssupCredentialToken`).

**Tests:** Ein Wert-/Layout-Vergleich gegen die IDL je Modul —
`crates/corba-csiv2/src/association_options.rs::tests::flag_values_match_spec_table_24_1`,
`crates/corba-csiv2/src/sas.rs::{sas_msg_type_values_match_spec,
identity_token_discriminator_matches_spec}`,
`crates/corba-csiv2/src/mech_list.rs::{cdr_roundtrip_compound_sec_mech_list,
build_mech_list_with_tls_and_gssup, cdr_roundtrip_empty_list}`,
`crates/corba-csiv2/src/gssup.rs::{initial_context_token_tag_is_0x60,
gssup_oid_starts_with_der_tag, encapsulation_is_wire_correct_octet_seq_len_at_offset_4}`.

**Status:** done

### §11 Unreliable Multicast Inter-ORB Protocol (MIOP)

**Spec:** Part 2 §11, S. 181 — UDP-Multicast-Variante von GIOP für
unreliable group communication.

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::{MiopConfig,
MiopFrameHeader, MiopError, MulticastSink, MulticastSinkError,
MiopSender, MIOP_MAGIC, MIOP_VERSION_1_0}` mit IPv4-Multicast-Group +
Port + TTL + Loopback-Flag (Default 239.255.0.1:5683 TTL=1) plus
voller MIOP-Frame-Codec (16-Byte-Header inkl. `MIOP`-Magic-Bytes,
Version `0x10`, Flags-Endian/Last-Frag-Bit, Packet-Length, Unique-ID,
Packet-Number, Number-of-Packets) und `MiopSender::send_giop` der
GIOP-Bytes als Single-Packet bzw. Multi-Packet-Set über einen
`MulticastSink`-Adapter versendet (Adapter-Trait, damit `corba-ccm`
keinen `transport-udp`-Layer-Zyklus erzeugt).

**Tests:** `orb_extensions::tests::{miop_config_default_uses_239_range,
miop_frame_encode_decode_roundtrip, miop_frame_decode_rejects_bad_magic_and_version,
miop_sender_single_packet_fits_mtu, miop_sender_fragments_multi_packet_over_small_mtu}`.

**Status:** done — MIOP-Frame-Codec + Sender-Pfad mit Single-/
Multi-Packet-Fragmentierung + Multicast-Sink-Adapter-Trait..

### §12 ZIOP Protocol

**Spec:** Part 2 §12, S. 219 — Zlib-/Compress-IOP-Messages,
Compression-Policies.

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::ZiopConfig` +
`CompressionAlgorithm` (siehe §18 Compression).

**Tests:** `crates/corba-ccm/src/orb_extensions.rs::tests::compression_algorithm_round_trip`,
`::compression_zlib_round_trip`, `::compression_gzip_round_trip`,
`::compression_deflate_round_trip`.

**Status:** done — ZIOP-Config-Daten-Modell + produktiver Compression-
Codec (Cross-Ref §18 Compression). `ZiopConfig.algorithm` wählt aus
den Backends (`None`/`Zlib`/`Gzip`/`Deflate`) den Codec; LZMA bleibt
explizit unsupported.

---

## Part 3: Component Model (CCM)

### §1-§5 Scope/Conformance/References/Terms/Symbols

**Spec:** Part 3 §1-§5, S. 1-7 — Geltungsbereich, Konformität,
Begriffe.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §6 Component Model

**Spec:** Part 3 §6, S. 9 — Component-Definition, Facets, Receptacles,
Events, Homes, Home-Finders, Configuration, Inheritance, Conformance.

**Repo:** `crates/corba-ccm/src/component_def.rs`,
`crates/corba-ccm/src/home.rs`, `crates/corba-ccm/src/port.rs`,
`crates/corba-ccm/src/context.rs`, `crates/corba-ccm/src/lifecycle.rs`.

**Tests:** `crates/corba-ccm/src/component_def.rs::tests` (30 Tests), darunter
`trader_has_4_ports`, `keyed_component_detected`,
`provide_facet_returns_facet_by_name`, `get_all_facets_returns_complete_list`,
`get_all_publishers_excludes_emit_only_sources`,
`supports_interface_returns_true_for_listed_iface`,
`the_base_chain_runs_from_the_derived_component_to_the_root`,
`flattening_takes_over_every_inherited_feature_bases_first`,
`a_redefined_port_name_is_refused_rather_than_shadowed`,
`a_second_primary_key_in_the_chain_is_refused`;
`crates/corba-ccm/src/cidl.rs::tests::all_four_composition_categories_distinct`;
die Unter-Items §6.5-§6.13 nennen je einen Test pro Klausel.

**Status:** done

#### §6.5 Facets and Navigation

**Spec:** §6.5, S. 13 — `provide_facet()`, navigation.

**Repo:** `crates/corba-ccm/src/component_def.rs`
(`provide_facet`/`get_all_facets`/`get_named_facets` — Navigations-
Interface), `crates/corba-ccm/src/lifecycle.rs` (`check_facet_lifetime`
— Facet-Referenz-Lebensdauer).

**Tests:** `crates/corba-ccm/src/component_def.rs::{provide_facet_returns_facet_by_name,
provide_facet_returns_none_for_unknown, get_all_facets_returns_complete_list,
get_named_facets_filters_by_names}`,
`crates/corba-ccm/src/lifecycle.rs::{facet_lifetime_passes_when_alive,
facet_lifetime_rejects_use_after_destroy, facet_lifetime_rejects_orphaned}`.

**Status:** done

#### §6.6 Receptacles

**Spec:** §6.6, S. 20 — `connect_*`/`disconnect_*`, Multiple
Receptacles.

**Repo:** `crates/corba-ccm/src/port.rs` (`PortRegistry::connect_receptacle`/
`disconnect_receptacle`/`get_connections`), `crates/corba-ccm/src/lifecycle.rs`
(`ReceptacleManager` mit Simplex-/Multiplex-Regel),
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

**Spec:** §6.7 — EventSource (publishers/emitters) und EventSink.

**Repo:** `crates/corba-ccm/src/port.rs` (Event-Ports, `EventStream`),
`crates/corba-ccm/src/component_def.rs` (`EventSourceDef.emit_only`
trennt `emits` von `publishes`; `EventSinkDef`).

**Tests:** `crates/corba-ccm/src/port.rs::{event_sink_subscription_round_trip,
list_event_sinks_unknown_source_empty}`,
`crates/corba-ccm/src/component_def.rs::{event_source_emit_only_default_false,
get_all_publishers_excludes_emit_only_sources,
get_all_emitters_includes_only_emit_only_sources,
get_named_publishers_respects_emit_only_flag,
get_named_emitters_excludes_publishers}`.

**Status:** done — Event-Port-API live; Wire-Pfad für Emitter geht
über CosEvent (siehe `cos-event-service-1.4.md`).

#### §6.8 Homes / §6.9 Home Finders

**Spec:** §6.8/§6.9, S. 34/42 — Home-Lifecycle, HomeFinder.

**Repo:** `crates/corba-ccm/src/home.rs` (`HomeDef` mit optionalem
Primary Key, `HomeFinder` mit `create`/`find_by_primary_key`/`remove`).

**Tests:** `crates/corba-ccm/src/home.rs::{home_def_default_has_no_primary_key,
keyed_home_has_primary_key, create_with_user_key_uses_key_as_id,
create_without_key_generates_system_id, find_by_primary_key_round_trip,
remove_decrements_instance_count, remove_unknown_returns_false,
home_get_component_def_returns_managed_component_id,
home_get_home_def_returns_self_repo_id}`.

**Status:** done

#### §6.10 Component Configuration / §6.11 Attributes

**Spec:** §6.10/§6.11 — Configurator-API, Attribute-Set.

**Repo:** `crates/corba-ccm/src/lifecycle.rs` (`Configurator`-Trait,
`StandardConfigurator` über den `AttributeDef`-Satz,
`ConfiguratorRegistry`), `crates/corba-ccm/src/context.rs`
(`ComponentContext`), `crates/corba-ccm/src/component_def.rs`
(`AttributeDef` mit `readonly`/`set_raises`/`get_raises`).

**Tests:** `crates/corba-ccm/src/lifecycle.rs::{configurator_set_get_roundtrip,
configurator_rejects_unknown_attribute, configurator_rejects_readonly_attribute,
configurator_get_unknown_returns_unknown_attribute,
configurator_registry_register_and_lookup}`,
`crates/corba-ccm/src/component_def.rs::tests::attribute_readonly_skips_set_raises`,
`crates/corba-ccm/src/context.rs::{anonymous_context_returns_none,
principal_context_returns_bytes}`.

**Status:** done

#### §6.12 Component Inheritance

**Spec:** §6.12, S. 49 — Vererbung von Component-Definitionen.

**Repo:** `crates/corba-ccm/src/component_def.rs::{ComponentTypeTable,
InheritanceError}` — `base_chain` läuft die `base_component`-Kette vom
abgeleiteten Typ zur Wurzel, `is_equivalent_component_kind` prüft eine
Repository-ID gegen die ganze Kette, `flatten` übernimmt Facets,
Receptacles, Event-Ports, Attribute, `supports`-Interfaces und den
Primary Key der Basen und weist eine redefinierte Port- oder
Attribut-Bezeichnung sowie einen zweiten Primary Key zurück.

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

**Spec:** §6.13, S. 51 — Conformance-Punkte für CCM-Compliance.

**Repo:** Conformance-Marker in
`crates/corba-ccm/src/lib.rs::conformance::CORBA_PART3_6_13_CCM_CONFORMANCE`
+ Cross-Ref §14 Lightweight CCM Profile-Marker.

**Tests:** `conformance_tests::corba_part3_6_13_marker_namespace`.

**Status:** done — Conformance-Marker als Doc-Constant ausgewiesen.

### §7 Generic Interaction Support (IDL3+)

**Spec:** Part 3 §7, S. 55 — Connector-Modell, IDL3+-Templates,
Generic Interaction Patterns.

**Repo:** Simple Connectors via `crates/corba-ccm/src/port.rs` +
`crates/corba-ccm-lib/src/dds_bridge.rs`. IDL3+-Templates als
Codegen-Erweiterung sind Caller-Layer (echtes IDL3+-Template-System
ist eine separate Spec-Schicht; Spec §7.3/§7.4 erlaubt explizit
"basic Connectors only" als minimalen Conformance-Pfad).
Conformance-Marker `corba-ccm::conformance::
CORBA_PART3_7_GENERIC_INTERACTION`.

**Tests:** Inline + `conformance_tests::corba_part3_7_marker_namespace`.

**Status:** done — Simple Connectors als Spec-konforme Mindest-
Conformance live; IDL3+-Templates bleiben optional.

### §8 OMG CIDL Syntax and Semantics

**Spec:** Part 3 §8, S. 81 — Component Implementation Definition
Language (Composition, Home/Segment Executor, Persistence,
Facet/Feature Delegation, Proxy Home).

**Repo:** `crates/corba-ccm/src/cidl.rs::{Composition,
CompositionCategory, HomeExecutor, StorageType, StorageHome}` —
Datenmodell der CIDL-Konstrukte plus die Kategorie-Regel
(`entity` verlangt eine Storage-Home, die übrigen drei nicht).

**Tests:** `crates/corba-ccm/src/cidl.rs::{session_composition_does_not_require_storage,
entity_composition_without_storage_invalid, entity_composition_with_storage_valid,
storage_type_can_inherit, home_executor_optional_component_binding,
all_four_composition_categories_distinct}`.

**Status:** partial — Composition, Home-Executor und die Persistence-
Konstrukte (`storagetype`/`storagehome`) sind als AST vorhanden und
über die Kategorie-Regel geprüft. Es fehlen: ein CIDL-Parser (das Modul
sagt selbst, die Sprache sei Caller-Layer), die Segment-Definition
(`segment`-Executor je Facet-Gruppe), die Facet-/Feature-Delegation
(`delegatesto`/`abstract storagehome`) und der Proxy Home. Der Rest
steht als eigenes Item in `corba-3.3.open.md`.

#### §8 OMG CIDL — verbleibende Konstrukte

**Spec:** Part 3 §8, S. 81 — die CIDL-Grammatik selbst, die
`segment`-Definition (ein Executor je Facet-Gruppe statt einer je
Komponente), die Facet-/Feature-Delegation (`delegatesto`,
`abstract storagehome`) und der Proxy Home.

**Repo:** —

**Tests:** —

**Status:** open — im Baum existiert nur das Datenmodell aus §8 oben.
Es gibt keinen Lexer/Parser für CIDL-Quelltext (das Modul
`crates/corba-ccm/src/cidl.rs` sagt selbst, die Sprache sei
Caller-Layer), keinen `Segment`-Typ, keine Delegations-Klausel und
keinen Proxy Home. Plan: die CIDL-Produktionen an den bestehenden
IDL-Frontend (`crates/idl-cst` + `crates/idl-parse`) anhängen und die
vier Konstrukte als AST-Knoten ergänzen; das Datenmodell aus §8 ist
das Ziel des Lowerings.

### §9 CCM Implementation Framework (CIF)

**Spec:** Part 3 §9, S. 93 — CIF-Architektur, Sprach-Mapping zu C++.

**Repo:** `crates/corba-ccm/src/cif.rs::{ComponentExecutor,
SessionExecutor, KeyedExecutor, ExecutorLocator, CifError}`,
`crates/corba-codegen/src/skeleton.rs`,
`crates/corba-codegen/src/stub.rs`,
`crates/corba-ccm-lib/src/persistence.rs`.

**Tests:** `crates/corba-ccm/src/cif.rs::{lifecycle_calls_increment_counters,
cif_error_carries_diagnostic}` (Executor-Lifecycle
`ccm_activate`/`ccm_passivate`/`ccm_remove`),
`crates/corba-codegen/src/skeleton.rs::{cpp_compares_with_strcmp_and_ends_in_bad_operation,
every_operation_reaches_the_dispatch_in_every_column}` und
`crates/corba-codegen/src/stub.rs::{a_variable_length_struct_returns_a_pointer_and_a_fixed_one_a_value,
an_object_reference_argument_is_a_ptr_and_its_out_is_an_out}`
für das C++-Sprach-Mapping.

**Status:** done

### §10 The Container Programming Model

**Spec:** Part 3 §10, S. 135 — Server- und Client-Programming
Environments, Basic/Extended Component Programming Interfaces.

**Repo:** `crates/corba-ccm/src/container.rs`,
`crates/corba-ccm/src/context.rs`, `crates/corba-ccm-lib/src/`
(Persistence-Bridge, Telemetry, DDS-Bridge).

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

**Spec:** Part 3 §11, S. 177 — CCM↔EJB-View-Mapping (CCM-Component →
EJB-View, EJB-Bean → CCM-View), TX-Bridging, Naming-Glue.

**Repo:** `crates/corba-ccm-ejb/` mit `connector_bean.rs`,
`naming_glue.rs`, `stub_gen.rs`, `tx.rs`.

**Tests:** `crates/corba-ccm-ejb/src/naming_glue.rs::tests` (8 Tests):
`bind_and_lookup_round_trip`, `rebind_replaces_ior`, `unbind_removes_entry`,
`jndi_global_strips_prefix`, `jndi_comp_env_strips_prefix`,
`cos_to_jndi_round_trip_no_kind`, `cos_to_jndi_includes_kind`;
`crates/corba-ccm-ejb/src/tx.rs::tests` (8): `cos_to_jta_round_trip_for_all_states`,
`begin_then_commit_succeeds`, `rollback_only_then_commit_fails_with_rollback`;
`crates/corba-ccm-ejb/src/connector_bean.rs::tests` (4),
`crates/corba-ccm-ejb/src/stub_gen.rs::tests` (5).

**Status:** done

### §12 Interface Repository Metamodel

**Spec:** Part 3 §12, S. 203 — IR-Metamodel-MOF-DTDs + IDL.

**Repo:** Erweiterungen in `crates/corba-ir/src/repository.rs` (MOF-
fähige `Container`/`Contained`-Hierarchie) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement,
IfrCcmMetamodel}` mit MOF-2.0-Subset (Class/Property/Operation) +
XMI-1.2-Output für das Component-Modell.

**Tests:** `orb_core::tests::{xmi_emitter_empty_yields_minimal_doc,
xmi_emitter_class_with_inheritance, xmi_emitter_property_emits,
xmi_emitter_operation_with_parameters,
ifr_ccm_metamodel_add_component,
ifr_ccm_metamodel_ingest_repository_walks_definitions}`.

**Status:** done — Basis-IR (`corba-ir`) + MOF-2.0-Subset + XMI-1.2-
Emitter live; `IfrCcmMetamodel::from_repository(&corba_ir::Repository)`
walked die `Container`/`Contained`-Hierarchie und produziert die
Component-Modell-Klassen. Repository-Walker (`ingest_definition_into`)
ist getestet; Metamodel-Output ist als XMI-Doc serialisierbar.

### §13 CIF Metamodel

**Spec:** Part 3 §13, S. 289 — CIF-Metamodel-MOF-DTDs + IDL.

**Repo:** `crates/corba-ccm/src/cif.rs` (CIF-AST) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement}`
für den XMI-Emitter symmetrisch zu §12.

**Tests:** `crates/corba-ccm/src/orb_core.rs::tests::xmi_emitter_empty_yields_minimal_doc`,
`::xmi_emitter_class_with_inheritance`, `::xmi_emitter_property_emits`;
`crates/corba-ccm/src/cif.rs::tests::lifecycle_calls_increment_counters`,
`::cif_error_carries_diagnostic`.

**Status:** done — CIF-AST (`corba-ccm::cif`) + MOF-XMI-Emitter
symmetrisch zu §12 IFR; Repository-Walker ingestiert Component-/
Composition-Definitionen über `IfrCcmMetamodel::from_repository`.

### §14 Lightweight CCM Profile

**Spec:** Part 3 §14, S. 301 — Lightweight-Profile mit explizit
ausgenommenen Features (Persistence, Introspection,
Segmentation, Transactions, Security, Configurators, Proxy Homes,
Home Finders).

**Repo:** Implizit im aktuellen Container-Programming-Model:
Persistence ist optional (`crates/corba-ccm-lib/src/persistence.rs`),
TX optional via `crates/corba-ccm-ejb/src/tx.rs`.

**Tests:** `conformance_tests::corba_part3_14_marker_namespace`.

**Status:** done — Lightweight-Profile-Marker
`corba-ccm::conformance::CORBA_PART3_14_LIGHTWEIGHT_CCM_PROFILE`
+ vorhandene `LIGHTWEIGHT_CCM_LEVEL` als formales Conformance-
Statement.

### §15 Deployment PSM for CCM

**Spec:** Part 3 §15, S. 309 — D&C-PSM: PIM→PSM-Transformation,
PSM→IDL/XML.

**Repo:** `crates/corba-dnc/src/plan.rs`, `crates/corba-dnc/src/node.rs`,
`crates/corba-dnc/src/execution.rs`, `crates/corba-dnc/src/container_host.rs`,
`crates/corba-dnc/src/repository.rs`.

**Tests:** `crates/corba-dnc/src/execution.rs::{full_lifecycle_round_trip,
invalid_plan_rejected_at_prepare, double_start_fails, destroy_before_running_fails}`
(PIM→PSM-Lebenszyklus prepare/start/destroy),
`crates/corba-dnc/src/container_host.rs::{install_creates_container_on_demand,
install_then_activate_reaches_active, full_lifecycle_round_trip,
entity_and_session_share_host}` (PSM→Container-Abbildung).

**Status:** done

### §16 Deployment IDL for CCM

**Spec:** Part 3 §16, S. 329 — D&C-IDL.

**Repo:** als Rust-Repräsentation in
`crates/corba-dnc/src/plan.rs` (`DeploymentPlan`,
`InstanceDeploymentDescription`, `PlanConnection`),
`crates/corba-dnc/src/repository.rs` (`RepositoryManager`,
`PackageConfiguration`), `crates/corba-dnc/src/node.rs`
(`NodeManager`, `NodeApplication`,
`NodeApplicationManager`), `crates/corba-dnc/src/execution.rs`
(`ExecutionManager`, `DomainApplicationManager`).

**Tests:** Ein Test je IDL-Interface-Operation-Gruppe —
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

**Spec:** Part 3 §17, S. 343 — XML-Schema für Deployment-Pläne.

**Repo:** `crates/corba-dnc/src/xml.rs`.

**Tests:** `crates/corba-dnc/src/xml.rs::{parses_minimal_plan,
parses_multiple_artifacts, validation_fails_on_unknown_impl_reference,
missing_root_tag_returns_expected_tag, co_locate_with_parsed}`.

**Status:** done

---

## Beigeordnete OMG-Service: COS Naming v1.3 (formal/04-10-03)

CosNaming-Service ist eigene Spec, wird hier als Initial-Reference
gemäß Part 1 §8.5.2 mitgeführt.

### CosNaming::NamingContext

**Spec:** CosNaming v1.3 §2 — `bind`/`rebind`/`unbind`/`resolve`/
`new_context`, Iterator-basiertes `list()`, stringified Names
(`"a/b/c"`-Pattern via `NamingContextExt::resolve_str`).

**Repo:** `crates/corba-cosnaming/src/context.rs`,
`crates/corba-cosnaming/src/name.rs`,
`crates/corba-cosnaming/src/stringified.rs`.

**Tests:** `crates/corba-cosnaming/src/context.rs::tests` (13 Tests):
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

## ZeroDDS-spezifische Bridges (kein OMG-Item)

### CORBA-Object ↔ DDS-Topic Bridge

**Spec:** kein OMG-Spec-Item; ZeroDDS-spezifische Migrations-Schicht
zur schrittweisen Ablösung von CORBA-Endpoints durch DDS-Wire.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs` (IOR↔Topic+Instance),
`crates/corba-dds-bridge/src/servant.rs` (CORBA-Servant über DDS-RPC),
`crates/corba-dds-bridge/src/sync.rs`.

**Tests:** `crates/corba-dds-bridge/src/mapping.rs::tests` (6 Tests):
`add_and_lookup_route`, `lookup_unknown_yields_none`,
`operation_mapping_by_name`, `add_route_replaces_existing`,
`all_routes_listing`, `direction_variants_distinct`;
`crates/corba-dds-bridge/src/locate.rs::tests` (4),
`crates/corba-dds-bridge/src/notify.rs::tests` (4),
`crates/corba-dds-bridge/src/csiv2_wire.rs::tests` (4),
`crates/corba-dds-bridge/src/bridge_security.rs::tests` (5).

**Status:** done — informativ; nicht spec-pflichtig.

---

## Audit-Status

50 done / 1 partial / 1 open / 12 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-corba-giop --lib` — 80 Tests grün.
* `cargo test -p zerodds-corba-iiop --lib` — 38 Tests grün.
* `cargo test -p zerodds-corba-ior --lib` — 53 Tests grün.
* `cargo test -p zerodds-corba-poa --lib` — 39 Tests grün.
* `cargo test -p zerodds-corba-cosnaming --lib` — 25 Tests grün.
* `cargo test -p zerodds-corba-csiv2 --lib` — 35 Tests grün.
* `cargo test -p zerodds-corba-cos-event --lib` — 23 Tests grün.
* `cargo test -p zerodds-corba-ccm --lib` — 181 Tests grün.
* `cargo test -p zerodds-corba-ccm-lib --lib` — 23 Tests grün.
* `cargo test -p zerodds-corba-ccm-ejb --lib` — 25 Tests grün.
* `cargo test -p zerodds-corba-ir --lib` — 23 Tests grün.
* `cargo test -p zerodds-corba-dnc --lib` — 30 Tests grün.
* `cargo test -p zerodds-corba-dds-bridge --lib` — 43 Tests grün.
* `cargo test -p zerodds-corba-codegen --tests` — 38 + 15 + 10 Tests grün.
* `cargo test -p zerodds-idl-parse --tests` — 36 + 32 Tests grün.
* `cargo test -p zerodds-idl-lower --tests` — 5 + 49 + 23 + 12 + 36 + 4 +
  82 + 3 + 62 Tests grün.

Offene Punkte und der Grund für den `partial`-Status von Part 3 §8:
`corba-3.3.open.md`.
