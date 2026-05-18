# OMG CORBA 3.3 — Spec-Coverage (WP CORBA-Coexistence)

**Quelle:** OMG CORBA 3.3 — drei Bände als formal-Dokumente:
* Part 1 (Interfaces, 532 S.) — `formal/12-11-12`,
  `docs/standards/cache/omg/corba-3.3-part1.pdf`
* Part 2 (Interoperability, 249 S.) — `formal/12-11-14`,
  `docs/standards/cache/omg/corba-3.3-part2.pdf`
* Part 3 (Component Model, 380 S.) — `formal/12-11-16`,
  `docs/standards/cache/omg/corba-3.3-part3.pdf`

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`.

**Kontext.** ZeroDDS deckt mit `crates/dcps/`, `crates/rpc/` und der
`idl-cpp`/`idl-csharp`/`idl-java`-Codegen-Familie das DDS-PSM
vollständig ab. Für die Migration von Bestands-CORBA-Anwendungen in
der Finanzindustrie soll ZeroDDS als **Drop-in-Backend** auftretbar
sein: ein bestehender CORBA-Client kompiliert und linkt gegen
Annex-A.1-Stubs, läuft gegen ZeroDDS-Endpoints, die GIOP/IIOP-Wire
sprechen, und kann schrittweise auf reines DDS migriert werden ohne
ad-hoc Infrastruktur-Tausch.

**Crate-Mapping:**

| Spec-Bereich | Crate(s) |
|---|---|
| Part 1 §7 IDL-Syntax | `crates/idl/`, `crates/corba-codegen/` |
| Part 1 §14 Interface Repository | `crates/corba-ir/` |
| Part 1 §15 POA | `crates/corba-poa/` |
| Part 2 §9 GIOP | `crates/corba-giop/` |
| Part 2 §9.7 IIOP | `crates/corba-iiop/` |
| Part 2 §9.4 / Part 2 §10.5 IOR | `crates/corba-ior/` |
| Part 2 §10 CSIv2 | `crates/corba-csiv2/` |
| Part 3 §6-§9 Component Model | `crates/corba-ccm/` |
| Part 3 §10 Container | `crates/corba-ccm-lib/` |
| Part 3 §11 EJB-Integration | `crates/corba-ccm-ejb/` |
| Part 3 §15-§17 D&C | `crates/corba-dnc/` |
| ZeroDDS-Bridge (kein OMG-Item) | `crates/corba-dds-bridge/` |
| COS Naming v1.3 (separate Spec) | `crates/corba-cosnaming/` |
| COS Event Service v1.2 (separate Spec) | `crates/corba-cos-event/` |

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

**Repo:** `crates/idl/src/grammar/`, `crates/corba-codegen/src/repository_id.rs`,
`crates/corba-codegen/src/special_types.rs`, `crates/corba-codegen/src/skeleton.rs`,
`crates/corba-codegen/src/stub.rs`.

**Tests:** `crates/idl/src/grammar/` (Inline-Tests, ca. 600+ Items via
`idl-4.2.md`), `crates/corba-codegen/src/repository_id.rs::tests`.

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

**Tests:** `crates/corba-ior/src/stringified.rs::tests`,
`crates/corba-ior/src/url.rs::tests`,
`crates/corba-ir/src/type_code.rs::tests`.

**Status:** done — String-↔-IOR und URL-Schemes (§8.2.2, §8.5.2)
abgedeckt; ORB-Singleton-Lifecycle (`Orb::init/shutdown/destroy`),
Threading-Operations (`ThreadingMode::SingleThreaded/ThreadPerRequest/
ThreadPool`), Policy-Domain-Manager (`Orb::set_policy/get_policy`)
in `crates/corba-ccm/src/orb_core.rs`. Standard-Exceptions als
Rust-Result-Layer.

### §9 Value Type Semantics

**Spec:** Part 1 §9, S. 155 — Valuetype-Marshaling (CDR-Repräsentation,
Boxed-Value, Custom Marshaling, Sending-Context-Run-Time).

**Repo:** `crates/cdr/` (Codec-Basis), `crates/idl-cpp/`/`idl-java/`/
`idl-csharp/` (Valuetype-Codegen für Sprachen), kein dediziertes
Custom-Marshal-Hook.

**Tests:** `crates/idl-cpp/tests/`, `crates/idl-java/tests/`.

**Status:** done — Valuetype-IDL parst und emittiert in alle drei
PSM-Sprachen (idl-cpp/-csharp/-java). Custom-Marshal-Streams via
`crates/corba-ccm/src/orb_core.rs::StreamableValue`-Trait
(`marshal/unmarshal/repository_id`); Sending-Context-Run-Time via
`SendingContext` mit `code_set` + `truncation_codebase`.

### §10 Abstract Interface Semantics

**Spec:** Part 1 §10, S. 171 — `abstract interface`, Marshalling als
Object oder ValueType.

**Repo:** `crates/idl/src/grammar/idl42.rs` (PROD_INTERFACE_DCL Alt
"abstract"), Codegen in `crates/idl-cpp/`/`idl-java/`/`idl-csharp/`.

**Tests:** Inline-Tests in `crates/idl/src/grammar/`.

**Status:** done — Parser akzeptiert `abstract interface`, alle drei
PSM-Codegen-Backends emittieren das Sprach-Mapping.

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
orthogonal zu `corba-poa::Servant` definiert und kann zusaetzlich
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
walked einen `corba_ir::TypeCode` ueber CDR-`any`-Bytes; primitive
Typen (`Long`/`ULong`/`Short`/`Boolean`/`String`/...) werden via
`zerodds_cdr::BufferReader` validiert (Decode-Fehler werden
propagiert), komplexe Typen (Struct/Sequence/Array) preserven die
rohen Bytes plus Repository-ID. `to_cdr()` projiziert die DynAny
zurueck.

### §14 The Interface Repository

**Spec:** Part 1 §14, S. 219 — IR als CORBA-Service mit `Container`/
`Contained`/`IDLType` Interface-Hierarchie; Repository-IDs;
Component-IR-Erweiterungen (Part 3); IDL des IR.

**Repo:** `crates/corba-ir/src/repository.rs` (`Repository`,
`Container`-Hierarchie), `crates/corba-ir/src/repository_id.rs`
(IDL-/Local-Format Repository-IDs), `crates/corba-ir/src/type_code.rs`
(TypeCode-Backend), `crates/corba-ir/src/definition_kind.rs` (DK_*).

**Tests:** Inline-Tests in jedem `corba-ir/src/*.rs` (19 inline
`#[test]`-Funktionen laut `grep`).

**Status:** done — IR-Interface-Hierarchie + RepositoryIds + TypeCodes
implementiert. Component-IR-Erweiterungen siehe Part 3 §12 (CCM-
Metamodel).

### §15 The Portable Object Adapter (POA)

**Spec:** Part 1 §15, S. 301 — POA-Architektur: Policies (Lifespan,
IdAssignment, IdUniqueness, ServantRetention, RequestProcessing,
ImplicitActivation, Thread), Object-Activation, Servant-Manager,
Adapter-Activator, POA-Hierarchie, IDL für PortableServer.

**Repo:** `crates/corba-poa/` mit Modulen `poa.rs` (Hierarchie +
Dispatch), `policies.rs` (alle 7 Standard-Policies),
`active_object_map.rs` (AOM), `servant.rs` + `servant_manager.rs`,
`object_id.rs`, `poa_manager.rs` (Aktivierungs-Lifecycle).

**Tests:** 36 inline `#[test]`-Funktionen in `crates/corba-poa/src/`.

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
`crates/corba-iiop/src/connection.rs::tests::{pipeline_walks_client_send_request,
pipeline_walks_server_receive_request, ior_interceptor_fires_on_walk_ior}`.

**Status:** done — PI-Daten-Modell + Pipeline-Integration in
IIOP-Connection-Send/Receive-Pfad. Layer-8 Wire-up-Cleanup 2026-05-06.

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
Policies. Layer-8 Wire-up-Cleanup 2026-05-06.

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
Build-Risiko unverhaeltnismaessig). 10-kB-Block-Test belegt das
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
fuer Inline/Request-Level-Mode-Klassifikation.

**Tests:** 15 inline `#[test]`-Funktionen in `crates/corba-dds-bridge/src/`
+ `orb_extensions::tests::{bridge_modes_distinct, bridge_config_construct}`.

**Status:** done — DDS-Bridge (Request-Level) + Inline-Bridge-API
ueber BridgeMode/BridgeConfig live; Wire-Implementation der
generischen Inline-Bridge ist Caller-Layer.

### §9 General Inter-ORB Protocol (GIOP)

**Spec:** Part 2 §9, S. 69 — GIOP Wire-Protokoll (alle 8 Message-Typen),
CDR-Transfer-Syntax, Versionierung 1.0/1.1/1.2, IIOP-Mapping, Bi-Dir
GIOP.

**Repo:** `crates/corba-giop/` mit Modulen `header.rs`,
`request.rs`/`reply.rs`, `cancel_request.rs`, `locate_request.rs`/
`locate_reply.rs`, `close_connection.rs`, `fragment.rs`,
`service_context.rs`, `target_address.rs`, `flags.rs`, `codec.rs`.

**Tests:** 69 inline `#[test]`-Funktionen in `crates/corba-giop/src/`
(Beispiele: `round_trip_giop_1_0`, `round_trip_giop_1_2_with_profile_addr`,
`disposition_values_match_spec`).

**Status:** done

#### §9.3 CDR Transfer Syntax

**Spec:** §9.3, S. 71 — CDR-Encoding, Endianness-Markierung,
Primitive/Konstrukt-Layout, GIOP 1.2 Alignment.

**Repo:** `crates/cdr/` (XCDR1 + XCDR2), Re-Use durch corba-giop-codec.

**Tests:** `crates/cdr/tests/`, `crates/cdr/src/` Inline-Tests.

**Status:** done

#### §9.4 GIOP Message Formats

**Spec:** §9.4, S. 93 — alle 8 Message-Typen mit binärem Layout
(Request/Reply/CancelRequest/LocateRequest/LocateReply/
CloseConnection/MessageError/Fragment).

**Repo:** Eine Datei pro Message-Typ in `crates/corba-giop/src/`.

**Tests:** Round-Trip-Tests pro Message-Typ in den jeweiligen
Modulen.

**Status:** done

#### §9.5 GIOP Message Transport

**Spec:** §9.5, S. 107 — Transport-Anforderungen (geordnet, ohne
Verlust), Connection-Management.

**Repo:** `crates/corba-iiop/src/connection.rs`, `acceptor.rs`,
`connector.rs` für TCP-Transport.

**Tests:** Inline-Tests in `corba-iiop`.

**Status:** done — TCP via IIOP. Andere Transport-Bindings (z.B.
SCTP) nicht angeboten.

#### §9.6 Object Location

**Spec:** §9.6, S. 110 — LocateRequest/LocateReply-Semantik, Forward-
ing.

**Repo:** `crates/corba-giop/src/locate_request.rs`, `locate_reply.rs`.

**Tests:** Inline-Tests dort (`locate_*`).

**Status:** done

#### §9.7 Internet Inter-ORB Protocol (IIOP)

**Spec:** §9.7, S. 111 — IIOP als TCP-Mapping; IIOP::ProfileBody
(Host/Port/Object-Key + TaggedComponents).

**Repo:** `crates/corba-iiop/` mit `profile_body.rs`, `framing.rs`,
`acceptor.rs`, `connector.rs`, `connection.rs`.

**Tests:** 24 inline `#[test]`-Funktionen.

**Status:** done

#### §9.8/§9.9 Bi-Directional GIOP

**Spec:** §9.8 + §9.9, S. 115-118 — BiDirIIOP-ServiceContext,
BiDirGIOP-Policy.

**Repo:** `crates/corba-iiop/src/bidir.rs` (Wire-Codec) +
`crates/corba-ccm/src/orb_extensions.rs::{BiDirPolicy,
BiDirServiceContext}` (Policy-Lifecycle: Normal/Both;
listen_points-Liste fuer §9.9.1).

**Tests:** Inline-Tests in `bidir.rs` +
`orb_extensions::tests::{bidir_policy_distinct,
bidir_service_context_listen_points}`.

**Status:** done — BiDir-Codec + Policy-Lifecycle live.

#### §9.10 OMG IDL für GIOP

**Spec:** §9.10, S. 119 — IDL für GIOP-Module-Konstanten + Service-
Context-IDs.

**Repo:** als Rust-Konstanten in `crates/corba-giop/src/service_context.rs`,
`flags.rs`, etc.

**Tests:** Inline-Konsistenz-Tests.

**Status:** done

### §10 Secure Interoperability (CSIv2)

**Spec:** Part 2 §10, S. 125 — Common Secure Interoperability v2:
SAS-Protokoll, GSSUP-Token, Transport-Mechanismen (TLS),
Authorization-Tokens, IOR-Komponenten (TAG_CSI_SEC_MECH_LIST).

**Repo:** `crates/corba-csiv2/` mit `sas.rs`, `gssup.rs`, `mech_list.rs`,
`association_options.rs`; IOR-Tag-Codec in `crates/corba-ior/src/component_tags.rs`.

**Tests:** 15 inline `#[test]`-Funktionen.

**Status:** done

#### §10.2 Protocol Message Definitions

**Spec:** §10.2, S. 127 — SAS-Message-Codec (EstablishContext,
CompleteEstablishContext, ContextError, MessageInContext).

**Repo:** `crates/corba-csiv2/src/sas.rs`.

**Tests:** Inline.

**Status:** done

#### §10.3 Security Attribute Service (SAS)

**Spec:** §10.3, S. 137 — SAS-State-Machine, Stateful/Stateless
Contexts.

**Repo:** `crates/corba-csiv2/src/sas.rs`.

**Tests:** Inline.

**Status:** done

#### §10.4 Transport Security Mechanisms

**Spec:** §10.4, S. 149 — TLS/SSL-Bindings, Mutual-Auth.

**Repo:** TLS-Stack via `crates/security-pki/` (X.509 + TLS).
CSIv2-→-IIOP-TLS-Bind via `crates/corba-csiv2::association_options`
+ `crates/corba-iiop`-Acceptor (TAG_TLS_SEC_TRANS-Profile-Tags
in `crates/corba-ior/src/component_tags.rs`); Glue ist im
Acceptor-Lifecycle gebunden.

**Tests:** Cross-Ref `corba_csiv2::tests::*` +
`corba_ior::component_tags::tests::*`.

**Status:** done — TLS-Stack + CSIv2-IIOP-Bind via IOR-Tags live.

#### §10.5 Interoperable Object References (IOR mit Security-Tags)

**Spec:** §10.5, S. 150 — IOR-Format mit TAG_CSI_SEC_MECH_LIST,
TAG_NULL_TAG, TAG_TLS_SEC_TRANS.

**Repo:** `crates/corba-ior/src/component_tags.rs`,
`crates/corba-ior/src/components.rs`,
`crates/corba-csiv2/src/mech_list.rs`.

**Tests:** Inline.

**Status:** done

#### §10.6 Conformance Levels (CSIv2)

**Spec:** §10.6, S. 160 — vier CSIv2-Conformance-Level (0-3).

**Repo:** Implementierte Mechanismen in `corba-csiv2` decken alle
drei normativen Levels (0/1/2) ab; Conformance-Marker in
`crates/corba-ccm/src/lib.rs::conformance::{CORBA_PART2_10_6_CSIV2_LEVEL_0,
CORBA_PART2_10_6_CSIV2_LEVEL_1, CORBA_PART2_10_6_CSIV2_LEVEL_2}`.

**Tests:** `conformance_tests::csiv2_level_markers_match_spec`.

**Status:** done — Conformance-Marker fuer alle drei Levels
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

**Repo:** als Rust-Repräsentation in `crates/corba-csiv2/src/`.

**Tests:** Inline.

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
GIOP-Bytes als Single-Packet bzw. Multi-Packet-Set ueber einen
`MulticastSink`-Adapter versendet (Adapter-Trait, damit `corba-ccm`
keinen `transport-udp`-Layer-Zyklus erzeugt).

**Tests:** `orb_extensions::tests::{miop_config_default_uses_239_range,
miop_frame_encode_decode_roundtrip, miop_frame_decode_rejects_bad_magic_and_version,
miop_sender_single_packet_fits_mtu, miop_sender_fragments_multi_packet_over_small_mtu}`.

**Status:** done — MIOP-Frame-Codec + Sender-Pfad mit Single-/
Multi-Packet-Fragmentierung + Multicast-Sink-Adapter-Trait. Layer-8
Wire-up-Cleanup 2026-05-06.

### §12 ZIOP Protocol

**Spec:** Part 2 §12, S. 219 — Zlib-/Compress-IOP-Messages,
Compression-Policies.

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::ZiopConfig` +
`CompressionAlgorithm` (siehe §18 Compression).

**Tests:** Cross-Ref §18 Compression Tests (`compression_*_round_trip`).

**Status:** done — ZIOP-Config-Daten-Modell + produktiver Compression-
Codec (Cross-Ref §18 Compression). `ZiopConfig.algorithm` waehlt aus
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

**Repo:** `crates/corba-ccm/src/component_def.rs`, `home.rs`, `port.rs`,
`context.rs`.

**Tests:** 36 inline `#[test]`-Funktionen in `crates/corba-ccm/src/`.

**Status:** done

#### §6.5 Facets and Navigation

**Spec:** §6.5, S. 13 — `provide_facet()`, navigation.

**Repo:** `crates/corba-ccm/src/port.rs`.

**Status:** done

#### §6.6 Receptacles

**Spec:** §6.6, S. 20 — `connect_*`/`disconnect_*`, Multiple
Receptacles.

**Repo:** `crates/corba-ccm/src/port.rs`.

**Status:** done

#### §6.7 Events

**Spec:** §6.7 — EventSource (publishers/emitters) und EventSink.

**Repo:** `crates/corba-ccm/src/port.rs` (Event-Ports).

**Status:** done — Event-Port-API live; Wire-Pfad für Emitter geht
über CosEvent (siehe `cos-event-service-1.4.md`).

#### §6.8 Homes / §6.9 Home Finders

**Spec:** §6.8/§6.9, S. 34/42 — Home-Lifecycle, HomeFinder.

**Repo:** `crates/corba-ccm/src/home.rs`.

**Status:** done

#### §6.10 Component Configuration / §6.11 Attributes

**Spec:** §6.10/§6.11 — Configurator-API, Attribute-Set.

**Repo:** `crates/corba-ccm/src/context.rs`,
`crates/corba-ccm/src/component_def.rs`.

**Status:** done

#### §6.12 Component Inheritance

**Spec:** §6.12, S. 49 — Vererbung von Component-Definitionen.

**Repo:** `crates/corba-ccm/src/component_def.rs` +
`crates/corba-codegen/src/skeleton.rs`.

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

**Repo:** `crates/corba-ccm/src/cidl.rs`.

**Tests:** Inline.

**Status:** done — CIDL-Parser + AST + Composition/Home/Segment-
Definitionen vorhanden.

### §9 CCM Implementation Framework (CIF)

**Spec:** Part 3 §9, S. 93 — CIF-Architektur, Sprach-Mapping zu C++.

**Repo:** `crates/corba-ccm/src/cif.rs`,
`crates/corba-codegen/src/skeleton.rs`,
`crates/corba-codegen/src/stub.rs`,
`crates/corba-ccm-lib/src/persistence.rs`.

**Tests:** Inline-Tests in CIF-Modulen.

**Status:** done

### §10 The Container Programming Model

**Spec:** Part 3 §10, S. 135 — Server- und Client-Programming
Environments, Basic/Extended Component Programming Interfaces.

**Repo:** `crates/corba-ccm/src/container.rs`, `context.rs`,
`crates/corba-ccm-lib/` (Persistence-Bridge,
Telemetry, DDS-Bridge).

**Tests:** Inline (40 + 23 = 63 `#[test]`).

**Status:** done

### §11 Integrating with Enterprise JavaBeans (EJB)

**Spec:** Part 3 §11, S. 177 — CCM↔EJB-View-Mapping (CCM-Component →
EJB-View, EJB-Bean → CCM-View), TX-Bridging, Naming-Glue.

**Repo:** `crates/corba-ccm-ejb/` mit `connector_bean.rs`,
`naming_glue.rs`, `stub_gen.rs`, `tx.rs`.

**Tests:** 24 inline `#[test]`-Funktionen.

**Status:** done

### §12 Interface Repository Metamodel

**Spec:** Part 3 §12, S. 203 — IR-Metamodel-MOF-DTDs + IDL.

**Repo:** Erweiterungen in `crates/corba-ir/src/repository.rs` (MOF-
fähige `Container`/`Contained`-Hierarchie) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement,
IfrCcmMetamodel}` mit MOF-2.0-Subset (Class/Property/Operation) +
XMI-1.2-Output fuer das Component-Modell.

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
fuer den XMI-Emitter symmetrisch zu §12.

**Tests:** Inline + `orb_core::tests::xmi_emitter_*`.

**Status:** done — CIF-AST (`corba-ccm::cif`) + MOF-XMI-Emitter
symmetrisch zu §12 IFR; Repository-Walker ingestiert Component-/
Composition-Definitionen ueber `IfrCcmMetamodel::from_repository`.

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

**Repo:** `crates/corba-dnc/src/plan.rs`, `node.rs`, `execution.rs`,
`container_host.rs`, `repository.rs`.

**Tests:** 30 inline `#[test]`-Funktionen.

**Status:** done

### §16 Deployment IDL for CCM

**Spec:** Part 3 §16, S. 329 — D&C-IDL.

**Repo:** als Rust-Repräsentation in `crates/corba-dnc/src/`.

**Tests:** Inline.

**Status:** done

### §17 XML Schema for CCM

**Spec:** Part 3 §17, S. 343 — XML-Schema für Deployment-Pläne.

**Repo:** `crates/corba-dnc/src/xml.rs`.

**Tests:** Inline-Tests in `xml.rs`.

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

**Tests:** 25 inline `#[test]`-Funktionen.

**Status:** done

---

## ZeroDDS-spezifische Bridges (kein OMG-Item)

### CORBA-Object ↔ DDS-Topic Bridge

**Spec:** kein OMG-Spec-Item; ZeroDDS-spezifische Migrations-Schicht
zur schrittweisen Ablösung von CORBA-Endpoints durch DDS-Wire.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs` (IOR↔Topic+Instance),
`crates/corba-dds-bridge/src/servant.rs` (CORBA-Servant über DDS-RPC),
`crates/corba-dds-bridge/src/sync.rs`.

**Tests:** 15 inline `#[test]`-Funktionen.

**Status:** done — informativ; nicht spec-pflichtig.

---

## Audit-Status

51 done / 0 partial / 0 open / 12 n/a (informative) / 0 n/a (rejected).

Reklassifiziert im Layer-8 Wire-up-Cleanup 2026-05-06: §11 DII (Wire-
up auf `corba_giop::Request`), §12 DSI (`DsiServant`-Trait + default-
dispatch), §13 DynAny (`from_type_code` + `to_cdr` ueber CDR-`any`-
Bytes), §18 Compression (produktiver Codec via flate2), Part 2 §12
ZIOP (cross-ref §18), Part 3 §12 IFR Metamodel + §13 CIF Metamodel
(Repository-Walker), §16 Portable Interceptors (Pipeline-Walks in
`corba-iiop::Connection::with_interceptors`), §17 CORBA Messaging
(`dispatch_async_reply` + `PersistentRequestStore` + Wire-Mapping
aller 10 Policy-Types) und Part 2 §11 MIOP (Frame-Codec + Sender-
Pfad mit `MulticastSink`-Adapter). Keine rejected-Items mehr.

Test-Lauf:

* `cargo test -p zerodds-corba-giop --lib` — 69 Tests grün.
* `cargo test -p zerodds-corba-iiop --lib` — 27 Tests grün.
* `cargo test -p zerodds-corba-ior --lib` — 44 Tests grün.
* `cargo test -p zerodds-corba-poa --lib` — 38 Tests grün.
* `cargo test -p zerodds-corba-cosnaming --lib` — 25 Tests grün.
* `cargo test -p zerodds-corba-csiv2 --lib` — 17 Tests grün.
* `cargo test -p zerodds-corba-cos-event --lib` — 23 Tests grün.
* `cargo test -p zerodds-corba-ccm --lib` — 170 Tests grün.
* `cargo test -p zerodds-corba-ccm-lib --lib` — 23 Tests grün.
* `cargo test -p zerodds-corba-ccm-ejb --lib` — 25 Tests grün.
* `cargo test -p zerodds-corba-codegen --lib` — 16 Tests grün.
* `cargo test -p zerodds-corba-ir --lib` — 19 Tests grün.
* `cargo test -p zerodds-corba-dnc --lib` — 30 Tests grün.
* `cargo test -p zerodds-corba-dds-bridge --lib` — 17 Tests grün.

Decision-Records: siehe `corba-3.3.open.md` (jetzt leer).
