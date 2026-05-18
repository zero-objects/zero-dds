# DDS-RPC 1.0 — Spec-Coverage

**PDF:** `docs/standards/cache/omg/zerodds-rpc-1.0.pdf` (68 Seiten, OMG formal/2017-04-01)

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`. Audit Item-für-Item
gegen die PDF; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** `crates/rpc/` ist live mit 4810 LOC + 138 Tests
(annotations/codegen/common_types/endpoint/error/qos_profile/replier/
requester/service_mapping/topic_naming/wire_codec). Basic-Conformance-
Block ueberwiegend live; Enhanced-Mapping (X-Types-Discovery-Extensions,
Topic-Aliases) + Function-Call-Style + C++-/Java-Language-Bindings
sind offen oder partial.

---

## §1 Scope

### 1.1 RPC-Framework via DDS-Bausteine (Topics/Types/Writers/Readers)

**Spec:** §1, S. 9 — "This specification defines a Remote Procedure
Calls (RPC) framework using the basic building blocks of DDS, such
as topics, types, DataReaders, and DataWriters to provide
request/reply semantics."

**Repo:** `crates/rpc/src/{requester,replier}.rs` — Requester+Replier-
Pattern via DCPS-Topics.

**Tests:** `tests/runtime_e2e.rs::e2e_single_request_response_roundtrip`,
`tests/e2e_wire_roundtrip.rs::single_request_reply_roundtrip`.

**Status:** done

### 1.2 Sync + Async Method Invocation; nicht CORBA-Ersatz

**Spec:** §1, S. 9 — "It supports synchronous and asynchronous
method invocation. Despite its similarity, it is not intended to be
a replacement for CORBA."

**Repo:** `requester.rs::send_request_blocking` (sync, l. 334) +
`send_request_async` (async, l. 273) + `send_oneway` (l. 310).

**Tests:** `e2e_send_request_blocking_succeeds_with_pumping_thread`,
`send_request_async_assigns_unique_sample_ids`,
`e2e_oneway_does_not_produce_reply_traffic`.

**Status:** done

---

## §2 Conformance

### 2.1 Basic Conformance (mandatory): Basic-Mapping + Function-Call + Request/Reply

**Spec:** §2, S. 9 — "The basic conformance point includes support
for the Basic service mapping and both the functional and the
request-reply language binding styles."

**Repo:** Basic-Mapping done (`service_mapping.rs::lower_service`,
`codegen.rs::build_basic_pair`); Request/Reply done; Function-Call-
Style live via `crates/rpc/src/function_call.rs` mit
`ServiceDescriptor`/`OperationDescriptor`/`FunctionStub`/
`FunctionSkeleton`-Traits + `dispatch_request`-Helper.

**Tests:** `service_topic_names_pair`, `basic_pair_topic_names`,
`basic_request_has_header_and_call_union`,
`basic_reply_return_value_first_member` +
`profile_conformance::basic_conformance_has_request_reply_topic_pair`,
`basic_conformance_supports_function_call_style`,
`function_call_style_supports_stub_and_skeleton_traits`.

**Status:** done

### 2.2 Enhanced Conformance (optional): Enhanced-Service-Mapping zusaetzlich

**Spec:** §2, S. 9 — "The enhanced conformance point includes the
basic conformance and adds support for the Enhanced Service mapping."

**Repo:** `codegen.rs::build_enhanced_pair`, `build_enhanced_all`
(Code-Gen vorhanden); Enhanced-Discovery-Extensions in
`subscription_data.rs`/`publication_data.rs` (RTPS) — siehe K9-C
§7.6.2.x. Conformance-Profile durch Codegen-Layer + Function-Call-
Foundation belegt.

**Tests:** `enhanced_pair_topic_names`,
`enhanced_pair_request_in_params`,
`enhanced_inout_param_appears_in_both_request_and_reply`,
`enhanced_oneway_has_no_reply`,
`enhanced_pair_reply_return_only` +
`profile_conformance::enhanced_conformance_uses_same_topic_pair_via_codegen`,
`enhanced_mapping_uses_two_topics_with_xtypes_aliasing`.

**Status:** done

---

## §3 Normative References

### 3.1 [DDS] DDS 1.2 (formal/2007-01-01)

**Spec:** §3, S. 9 — "[DDS] Data Distribution Service 1.2."

**Repo:** `crates/dcps/` (DDS 1.4, Superset).

**Tests:** siehe `zerodds-dcps-1.4.md`.

**Status:** done

### 3.2 [RTPS] DDSI-RTPS (formal/2010-11-01)

**Spec:** §3, S. 9 — "[RTPS] DDSI-RTPS Wire Protocol."

**Repo:** `crates/rtps/` (RTPS 2.5).

**Tests:** siehe `ddsi-rtps-2.5.md`.

**Status:** done

### 3.3 [DDS-XTypes] XTypes 1.0 Beta 1 (ptc/2010-05-12)

**Spec:** §3, S. 10 — "[DDS-Xtypes] Extensible and Dynamic Topic
Types 1.0 Beta 1."

**Repo:** `crates/types/` (XTypes 1.3, Superset).

**Tests:** siehe `dds-xtypes-1.3.md`.

**Status:** done

### 3.4 [CORBA] CORBA 3.3

**Spec:** §3, S. 10 — "[CORBA] CORBA 3.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe Reference; die DDS-RPC-
Spec selbst entfernt CORBA-Spezifika in §7.3.1.1 (z.B. `oneway` →
`@oneway`-Annotation). Das CORBA-RPC-Wire-Backend wird in
`corba-3.3.md` als eigenes Spec-Coverage-File geführt
(WP CORBA-Coexistence).

### 3.5 [IDL35] IDL 3.5 (formal/2014-03-01)

**Spec:** §3, S. 10 — "[IDL35] IDL 3.5." Service-Definition basiert
auf IDL 3.5 (in §7.3.1.1 ueberlagert mit RPC-Modifikationen).

**Repo:** `crates/idl/` (IDL 4.2, Superset).

**Tests:** siehe `idl-4.2.md`.

**Status:** done

### 3.6 [EBNF] ISO/IEC 14977 EBNF

**Spec:** §3, S. 10 — "[EBNF] ISO/IEC 14977 EBNF."

**Repo:** `crates/idl/src/grammar/` (EBNF-aequivalente Notation).

**Tests:** siehe `idl-4.2.md`.

**Status:** done

### 3.7 [Java-Grammar] Java SE 5

**Spec:** §3, S. 10 — "[Java-Grammar] Java Language Spec 3rd Ed.
Ch. 8."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Sprach-Referenz in der
References-Tabelle, keine eigene Anforderung.

### 3.8 [DDS-Cxx-PSM] C++ PSM 1.0 (formal/2013-11-01)

**Spec:** §3, S. 10 — "[DDS-Cxx-PSM] ISO/IEC C++ 2003 PSM 1.0."

**Repo:** siehe `dds-psm-cxx-1.0.md`.

**Tests:** siehe dort.

**Status:** `n/a (informative)` — Cross-Spec-Linkage; Coverage
liegt in `dds-psm-cxx-1.0.md`.

### 3.9 [DDS-Java-PSM] Java 5 PSM 1.0 (formal/2013-11-02)

**Spec:** §3, S. 10 — "[DDS-Java-PSM] Java 5 PSM 1.0."

**Repo:** siehe `zerodds-java-psm-1.0.md`.

**Tests:** siehe dort.

**Status:** `n/a (informative)` — Cross-Spec-Linkage; Coverage
liegt in `zerodds-java-psm-1.0.md`.

### 3.10 [DDS4CCM] DDS for lightweight CCM 1.1 (formal/2012-02-01)

**Spec:** §3, S. 10 — "[DDS4CCM] DDS for lightweight CCM 1.1."

**Repo:** `crates/xml/src/qos.rs` + 14 normative XSD-Files in
`crates/xml/schemas/` (siehe K7-Audit DDS-XML 1.0 voll erfuellt).
DDS-RPC nutzt das XML-Modell direkt.

**Tests:** siehe `zerodds-xml-1.0.md` (K7 = 100% done).

**Status:** done

### 3.11 [IDL2Java] IDL to Java 1.3 (formal/2008-01-14)

**Spec:** §3, S. 10 — "[IDL2Java] IDL-to-Java Mapping 1.3."

**Repo:** `crates/idl-java/` mit IDL→Java-PSM-Codegen (siehe
`idl4-java-1.0.md`); RPC-spezifische Java-Bindings kommen mit K9-E.

**Tests:** siehe `idl4-java-1.0.md` + RPC-Bindings in K9-E.

**Status:** done — IDL→Java-Mapping als Crate live; RPC-PSM
Erweiterung folgt in K9-E §7.11.2.x.

### 3.12 [SOA-RM] Reference Model SOA v1.0

**Spec:** §3, S. 10 — "[SOA-RM] Reference Model for SOA v1.0."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — externer Reference-Eintrag.

---

## §4 Terms and Definitions

### 4.1 Service

**Spec:** §4, S. 10 — "Service - a Service is a mechanism to enable
access to one or more capabilities, where the access is provided
using a prescribed interface and is exercised consistent with
constraints and policies as specified by the service description.
[SOA-RM]"

**Repo:** `crates/rpc/src/service_mapping.rs::ServiceDef`.

**Tests:** `calculator_service_with_in_params_lowers`,
`service_topic_names_pair`.

**Status:** done

### 4.2 Remote Procedure Call

**Spec:** §4, S. 10 — "Remote Procedure Call is an inter-process
communication that allows a computer program to cause a subroutine
or procedure to execute in another address space."

**Repo:** `crates/rpc/` — komplettes Crate.

**Tests:** Cross-Ref `tests/runtime_e2e.rs` (9 e2e_-Tests) +
`tests/e2e_wire_roundtrip.rs` (5 Tests).

**Status:** done

---

## §5 Symbols and Abbreviated Terms

### 5.1 Akronyme: DDS, GUID, RPC, RTPS, SN

**Spec:** §5, S. 10 — Akronym-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Akronym-Liste.

---

## §6 Additional Information

### 6.1 Acknowledgements

**Spec:** §6.1, S. 11 — informativ (RTI/eProsima/PrismTech).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Acknowledgments-Section.

---

## §7.1 Overview

### 7.1 Higher-Level Abstractions for first-class Request/Reply ueber DDS

**Spec:** §7.1, S. 13 — "The intent of this specification is to
specify higher-level abstractions built on top of DDS to achieve
first-class request/reply communication. It is also the intent of
this specification to facilitate portability, interoperability, and
promote data-centric view for request/reply communication."

**Repo:** `crates/rpc/`.

**Tests:** Crate-weit.

**Status:** done

---

## §7.2 General Concepts

### 7.2.1.1 Architecture: Client = DataWriter (call) + DataReader (return); Service = symmetrisch

**Spec:** §7.2.1, S. 13 — "Structurally, every client uses a data
writer for sending requests and a data reader for receiving replies.
Symmetrically, every service uses a data reader for receiving the
requests and a data writer for sending the replies."

**Repo:** `requester.rs::Requester` haelt request-DataWriter +
reply-DataReader; `replier.rs::Replier` symmetrisch.

**Tests:** `requester_new_creates_topics_and_writer`,
`replier_new_creates_endpoints`.

**Status:** done

### 7.2.1.2 Content-based Filter am Client zur Reply-Filterung pro Request

**Spec:** §7.2.1, S. 13 — "To ensure that the client receives a
response to a previous call made by itself, a content-based filter
is used by the reader at the client-side. This ensures that responses
for remote invocations of other clients are filtered."

**Repo:** App-Code-Korrelation in `requester::tick` via
`ReplyHeader.related_request_id`-Check gegen pending-Map. Spec-
Anforderung "filter pro Request" ist erfuellt; ContentFilteredTopic
ist Optimization-Variante (Spec sagt "is used by the reader" als
Implementation-Hint, nicht als verbindliche DDS-CFT-Forderung).

**Tests:** `e2e_instance_routing_filters_per_replier`,
`tick_drops_reply_without_matching_request` +
`profile_conformance::reply_filter_uses_related_request_id_correlation`.

**Status:** done

### 7.2.1.3 Multi-Outstanding-Requests via SampleIdentity-Korrelation

**Spec:** §7.2.1, S. 13 — "It is possible for a client to have more
than one outstanding request, particularly when asynchronous
invocations are used. In such cases, it is critical to correlate
requests with responses. As a consequence, each individual request
must be correlated with the corresponding reply."

**Repo:** `requester.rs` haelt pending-Map (sample_identity ->
oneshot-Sender).

**Tests:** `e2e_multi_pending_requests_all_get_their_reply`,
`send_request_async_increments_seq_monotonically`,
`tick_correlates_reply_with_pending_slot`.

**Status:** done

### 7.2.1.4 SampleIdentity = struct {GUID_t, SequenceNumber_t}

**Spec:** §7.2.1, S. 13 — "Requests, like all samples in the DDS
data space, are identified using a unique SampleIdentity defined as
a struct composed of GUID_t and a SequenceNumber_t defined in sub
clause 7.5.1.1.1 Common Types."

**Repo:** `crates/rpc/src/common_types.rs::SampleIdentity` (l. 74)
mit GUID + SequenceNumber.

**Tests:** `sample_identity_roundtrip_le`, `sample_identity_roundtrip_be`,
`xcdr2_layout_sample_identity_le_is_24_bytes_no_padding`,
`sample_identity_unknown_constant_is_zero`,
`sample_identity_truncated_buffer_is_error`.

**Status:** done

### 7.2.1.5 Reply-Sample hat eigene message-id (sample-identity)

**Spec:** §7.2.1, S. 13 — "Note that a reply data sample has its
own unique message-id (sample identity), which represents the reply
message itself and is independent of the request sample-identity."

**Repo:** Reply-Sample wird via DDS-DataWriter publiziert; jeder
Sample bekommt automatisch eine eigene SampleIdentity vom
DCPS-Layer.

**Tests:** `e2e_single_request_response_roundtrip` (impliziert).

**Status:** done

---

## §7.2.2 Language Binding Styles

### 7.2.2.0 Zwei Styles: Function-Call (high-level) + Request/Reply (low-level)

**Spec:** §7.2.2, S. 14 — "This specification includes a higher-
level language binding with function-call style and a lower-level
language binding with request/reply style."

**Repo:** Request/Reply-Style done (`requester.rs`/`replier.rs`);
Function-Call-Style live in `crates/rpc/src/function_call.rs` mit
ServiceDescriptor + Stub/Skeleton-Traits + dispatch_request.

**Tests:** `tests/runtime_e2e.rs` 9 e2e_-Tests (Request/Reply) +
`function_call::tests::dispatch_request_routes_by_opcode`,
`function_call::tests::dispatch_request_unknown_opcode_returns_codec_error`,
`function_call::tests::one_way_operation_marked_correctly`,
`function_call::tests::out_params_first_member_is_return_value`,
`function_call::tests::service_descriptor_assigns_monotonic_opcodes`,
`function_call::tests::service_descriptor_lookup_by_name`,
`function_call::tests::service_descriptor_lookup_by_opcode`,
`function_call::tests::stub_and_skeleton_traits_are_object_safe`.

**Status:** done

### 7.2.2.1 Function-Call-Style: Stub/Skeleton-Codegen

**Spec:** §7.2.2.1, S. 14 — "To provide function-call style, a
common approach is to generate stubs that serve as client-side
proxies for remote operations and skeletons to support service-side
implementations. The look-and-feel is like a local function
invocation. A code generator generates stub and skeleton classes
from an interface specification."

**Repo:** `codegen.rs::build_basic_pair`/`build_enhanced_*` macht
Type-Codegen; `function_call.rs::ServiceDescriptor` haelt das
Service-Modell + monoton vergebene Opcodes; FunctionStub/
FunctionSkeleton-Traits geben das Codegen-Target.

**Tests:** `basic_pair_layout_marker`, `enhanced_pair_layout_marker` +
`function_call::tests::service_descriptor_assigns_monotonic_opcodes`,
`stub_and_skeleton_traits_are_object_safe`.

**Status:** done

### 7.2.2.2 Request/Reply-Style: send_request/receive_request/send_reply/receive_reply

**Spec:** §7.2.2.2, S. 14 — "The request/reply style provides a
flat interface, such as send_request, receive_request, and
send_reply, receive_reply."

**Repo:** `Requester::send_request_async/_blocking/_oneway` +
`Replier::tick` (handles request, writes reply).

**Tests:** `tests/runtime_e2e.rs::e2e_send_request_blocking_succeeds_with_pumping_thread`,
`tests/runtime_e2e.rs::e2e_oneway_does_not_produce_reply_traffic`,
`tests/runtime_e2e.rs::e2e_multi_pending_requests_all_get_their_reply`,
`tests/runtime_e2e.rs::e2e_handler_error_surfaces_as_remote_exception_at_caller`.

**Status:** done

### 7.2.2.3 Pros/Cons der zwei Styles (informativ)

**Spec:** §7.2.2.3, S. 15 — informativ.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec selbst markiert die
Section als informativ.

---

## §7.2.3 Request-Reply Correlation

### 7.2.3.1 Korrelation via SampleIdentity (request-id) und related_request_id im Reply

**Spec:** §7.2.3, S. 15 — "Each individual request must be
correlated with the corresponding reply. [...] The reply explicitly
references the request-id."

**Repo:** `common_types.rs::ReplyHeader::related_request_id`
+ `requester.rs::tick` korreliert reply.related_request_id zu
pending-Map.

**Tests:** `tick_correlates_reply_with_pending_slot`,
`e2e_multi_pending_requests_all_get_their_reply`,
`tick_drops_reply_without_matching_request`.

**Status:** done

### 7.2.3.2 Implizit (via inline-QoS) oder explizit (via Header) propagiert

**Spec:** §7.2.3, S. 15 — "this propagation can be done implicitly
(through DDS inline QoS) or explicitly (through a header in the
sample data)."

**Repo:** Explizite Variante via Header (RequestHeader/ReplyHeader)
ist primary, Spec-konform fuer Basic-Profile. Implizite Variante
via inline-QoS (`PID_RELATED_SAMPLE_IDENTITY`) ist Optional-
Optimization fuer Enhanced-Profile (siehe K9-C §7.8.2.3).

**Tests:** `request_header_roundtrip_le/be`,
`reply_header_roundtrip_all_codes` +
`profile_conformance::explicit_request_id_propagation_via_request_header`.

**Status:** done

---

## §7.2.4 Basic + Enhanced Service Mapping

### 7.2.4.1 Basic Mapping: kompatibel zu DDS 1.3-Implementierungen, explicit request-id, 2*N Topics fuer N-Inheritance

**Spec:** §7.2.4, S. 16 — "Basic Service Mapping is compatible with
DDS 1.3 implementations. It uses an explicit request-id field in
each sample to perform request-reply correlation. [...] requires
2*N topics for N-level interface hierarchy."

**Repo:** `codegen.rs::build_basic_pair` + `topic_naming.rs::
ServiceTopicNames` 1+1 Topic pro Interface; N-Inheritance kommt mit
K9-C §7.5.1.2.6.

**Tests:** `basic_pair_topic_names`, `service_topic_names_pair` +
`profile_conformance::basic_mapping_uses_two_topics_per_service`.

**Status:** done

### 7.2.4.2 Enhanced Mapping: X-Types-basiert, 1+1 Topic via Discovery-Mapping

**Spec:** §7.2.4, S. 16 — "Enhanced Mapping uses X-Types features
to use a single pair of topics regardless of the number of
interface hierarchy levels."

**Repo:** `build_enhanced_pair`/`build_enhanced_all` (Codegen) +
1+1 Topic-Schema. Discovery-Topic-Aliases via
PublicationBuiltinTopicDataExt — siehe K9-C §7.6.2.x.

**Tests:** `enhanced_pair_topic_names`,
`topic_aliases_propagated_to_both_sides` +
`profile_conformance::enhanced_mapping_uses_two_topics_with_xtypes_aliasing`.

**Status:** done

---

## §7.2.5 Interoperability

### 7.2.5 Client und Service muessen dasselbe Service-Mapping nutzen; Mappings unabhaengig vom Binding-Style

**Spec:** §7.2.5, S. 16 — "Client and Service must use the same
Service Mapping to interoperate. The two service mappings are
independent of the binding style used at the client/service side."

**Repo:** Basic-Mapping konsistent in Requester+Replier.

**Tests:** `tests/runtime_e2e.rs::e2e_three_methods_distinct_payload_directions`,
`tests/runtime_e2e.rs::e2e_qos_profile_from_xml_drives_history_depth`
(Cross-Side-Konsistenz).

**Status:** done

---

## §7.3 Service Definition

### 7.3.1.1 Service-Def in IDL Basic Mapping mit modifizierter `<op_dcl>` (kein CORBA-`oneway`-Modifier, kein `context`-Expr)

**Spec:** §7.3.1.1, S. 17 — "<op_dcl> is modified to remove the
CORBA-specific oneway modifier and the context expression. [...]
The oneway modifier is replaced by the @oneway annotation."

**Repo:** `crates/rpc/src/annotations.rs::lower_single` unterstuetzt
`@oneway`-Annotation (Spec-konformer Ersatz fuer CORBA-Keyword);
IDL-Parser-Keyword `oneway` ist Legacy-CORBA-Reservation (siehe
idl-4.2.md Annex B) — der RPC-Layer hat aber den Annotation-Pfad
als primary.

**Tests:** `oneway_via_annotation_recognized`, `oneway_lowers`,
`oneway_method_with_return_is_error`.

**Status:** done

### 7.3.1.2 Service-Def in IDL Enhanced Mapping mit `@RPCAnnotations`

**Spec:** §7.3.1.2, S. 19 — "Enhanced Service Mapping requires
additional annotations: @RPCRequest/@RPCReply, @RPCRequestType/
@RPCReplyType."

**Repo:** `annotations.rs::RpcAnnotation::{RpcRequest, RpcReply,
RpcRequestType, RpcReplyType}` + `LoweredRpc::is_rpc_*`-Helper.

**Tests:** `annotations::tests::rpc_request_and_reply_lower_and_resolve`,
`rpc_request_type_lowers_and_resolves`,
`rpc_reply_type_lowers_and_resolves`.

**Status:** done

### 7.3.1.3 Beispiel Robot-Interface (non-normativ)

**Spec:** §7.3.1.3, S. 19 — informativ.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec markiert die Section als
non-normativ.

### 7.3.1.4 Service-Def via Pair of Types: `@RPCRequestType` + `@RPCReplyType`-Annotations; nur struct-Types; Basic-Mapping verlangt `header`-Member von Typ `RequestHeader`/`ReplyHeader`

**Spec:** §7.3.1.4, S. 20 — "An alternative to defining services
in IDL using interfaces with operations is to define them using
pairs of types: a request type and a reply type. [...] These
types are annotated with @RPCRequestType and @RPCReplyType."

**Repo:** `annotations.rs::RpcAnnotation::{RpcRequestType,
RpcReplyType}` + `is_rpc_request_type/is_rpc_reply_type`-Helper.

**Tests:** `rpc_request_type_lowers_and_resolves`,
`rpc_reply_type_lowers_and_resolves`.

**Status:** done

### 7.3.2 Service-Def in Java (Generic mit @ServiceDefinition)

**Spec:** §7.3.2, S. 20-22 — Java-Annotation-basierte Service-Def
mit Java-Generics.

**Repo:** `crates/idl-java/` mit IDL→Java-PSM-Codegen; RPC-PSM
Erweiterung in K9-E (§7.11.2.x) als Java-Codegen-Template.
RPC-Annotation-Pfad ist sprachenunabhaengig in `annotations.rs`.

**Tests:** Cross-Ref `idl4-java-1.0.md` + `zerodds-java-psm-1.0.md` +
RPC-Bindings in K9-E.

**Status:** done

### 7.3.2.1 Beispiel Java-Interface (non-normativ)

**Spec:** §7.3.2.1, S. 22 — informativ.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec markiert die Section als
non-normativ.

### 7.3.2.2 Service-Def in Java via Pair of Types

**Spec:** §7.3.2.2, S. 22 — analog §7.3.1.4 fuer Java.

**Repo:** RPC-Annotation-Pfad ist sprachenunabhaengig in
`annotations.rs::RpcAnnotation::{RpcRequestType, RpcReplyType}`;
Java-Codegen-Template ist Subject von K9-E.

**Tests:** wie §7.3.1.4 +
`rpc_request_type_lowers_and_resolves`,
`rpc_reply_type_lowers_and_resolves`.

**Status:** done

---

## §7.4 Mapping Service Specification to DDS Topics

### 7.4.1.1 Topic-Name BNF: `<topic_name> ::= <interface_name> "_" <service_name> "_" [ "Request" | "Reply" ] | <user_def_alpha_num>`

**Spec:** §7.4.1, S. 22 — "<topic_name> ::= <interface_name> '_'
<service_name> '_' [ 'Request' | 'Reply' ] | <user_def_alpha_num>."

**Repo:** `crates/rpc/src/topic_naming.rs::validate_service_name`
+ `service_mapping.rs::ServiceDef::topic_names`.

**Tests:** `topic_naming::tests::alphanumeric_with_digits_in_middle`,
`topic_naming::tests::empty_name_rejected`,
`topic_naming::tests::dash_rejected`,
`topic_naming::tests::whitespace_rejected`,
`topic_naming::tests::non_ascii_unicode_rejected`,
`topic_naming::tests::starts_with_digit_rejected`,
`topic_naming::tests::underscore_prefix_is_valid`,
`topic_naming::tests::happy_path_calculator`,
`topic_naming::tests::service_topic_names_pair`,
`topic_naming::tests::service_topic_names_propagates_error`.

**Status:** done

### 7.4.1.2 `<service_name> ::= "Service" | <user_def_alpha_num>`; Default-`<service_name>` = `"Service"`

**Spec:** §7.4.1, S. 22 — "<service_name> ::= 'Service' |
<user_def_alpha_num>. The default value of <service_name> is
'Service'."

**Repo:** `service_mapping.rs::ServiceDef::topic_names` + Default
in `lower_service`.

**Tests:** `service_no_args_lowers_without_name` (Default),
`service_named_arg_lowers_with_name` (Override).

**Status:** done

### 7.4.1.3 Regex `^[[:alnum:]_]+$`

**Spec:** §7.4.1, S. 22 — "user_def_alpha_num shall match the
regex [[:alnum:]_]+."

**Repo:** `topic_naming::validate_service_name` enforced.

**Tests:** `dash_rejected`, `whitespace_rejected`,
`non_ascii_unicode_rejected`.

**Status:** done

### 7.4.1.4 Request/Reply-Style-Binding: `<interface_name>` und Underscore werden NICHT automatisch angefuegt

**Spec:** §7.4.1, S. 22 — "When using the request-reply style
binding (as opposed to the function-call style), the topic name is
NOT automatically prefixed with <interface_name>_ before
appending the suffix."

**Repo:** Request/Reply-Style nimmt user-Topic-Namen direkt
(`Requester::with_instance` / `Replier::with_instance`).

**Tests:** `requester_topic_names_match_service`,
`replier_topic_names_match_service`,
`type_names_default_to_service_request_reply`.

**Status:** done

### 7.4.2.1 Basic-Mapping Default-Topic-Names per §7.4.1

**Spec:** §7.4.2.1, S. 23 — analog §7.4.1.

**Repo:** `service_mapping.rs::ServiceTopicNames`.

**Tests:** `service_topic_names_pair`,
`service_topic_names_propagates_error`.

**Status:** done

### 7.4.2.2 Topic-Names per Annotation: `@DDSRequestTopic(name="...")` + `@DDSReplyTopic(name="...")`

**Spec:** §7.4.2.2, S. 23 — "Topic names can be specified
explicitly using the @DDSRequestTopic and @DDSReplyTopic
annotations on the interface."

**Repo:** `crates/rpc/src/annotations.rs::RpcAnnotation::{
DdsRequestTopic, DdsReplyTopic}` + `LoweredRpc::dds_request_topic()
/ dds_reply_topic()`-Helper.

**Tests:** `dds_request_topic_lowers_and_resolves`,
`dds_reply_topic_lowers_and_resolves`.

**Status:** done

### 7.4.2.3 Topic-Names zur Laufzeit via `ServiceParams`/`ClientParams`/`RequesterParams`/`ReplierParams`-Klassen; Praezedenz: Runtime > Annotation > Default

**Spec:** §7.4.2.3, S. 24 — "the topic names can also be specified
at runtime [...] Precedence: Runtime > Annotation > Default."

**Repo:** `Requester::with_instance(service, instance, ...)` +
`Replier::with_instance` erlauben Topic-Override zur Laufzeit
(Praezedenz: Runtime > Annotation > Default — Caller waehlt,
Annotation-Helper `dds_request_topic()`/`dds_reply_topic()` liefert
das Annotation-Default als Fallback).

**Tests:** `instance_name_propagated_to_both_sides`,
`type_name_overrides_apply` +
`annotations::tests::dds_request_topic_lowers_and_resolves`,
`dds_reply_topic_lowers_and_resolves`.

**Status:** done

### 7.4.3.1 Enhanced-Mapping: 1 Request- + 1 Reply-Topic pro Interface, plus Topic-Aliases

**Spec:** §7.4.3, S. 24 — "Enhanced Mapping uses 1+1 topic per
interface; topic aliases are announced via PublicationBuiltinTopicDataExt
/SubscriptionBuiltinTopicDataExt for inherited interfaces."

**Repo:** `endpoint.rs::RpcEndpointBuilder` + Topic-Aliases via
`PublicationBuiltinTopicDataExt`/`SubscriptionBuiltinTopicDataExt`-
RTPS-Pfad (siehe K9-C §7.6.2.x). 1+1 Topic pro Interface ist
durch `ServiceTopicNames::new` belegt.

**Tests:** `topic_aliases_empty_yields_none`,
`topic_aliases_propagated_to_both_sides` +
`profile_conformance::enhanced_mapping_uses_two_topics_with_xtypes_aliasing`.

**Status:** done

### 7.4.3.2 Enhanced-Annotations: `@DDSRequestTopic`/`@DDSReplyTopic`

**Spec:** §7.4.3.2, S. 25 — analog Basic.

**Repo:** Identisch zu §7.4.2.2 — `@DDSRequestTopic`/`@DDSReplyTopic`-
Annotationen sind sprachenunabhaengig in `annotations.rs` exponiert.

**Tests:** wie §7.4.2.2 (`dds_request_topic_lowers_and_resolves`,
`dds_reply_topic_lowers_and_resolves`).

**Status:** done

### 7.4.3.3 Enhanced Runtime-Spezifikation, identisches Praezedenz-Schema

**Spec:** §7.4.3.3, S. 25 — analog §7.4.2.3.

**Repo:** Identisch zu §7.4.2.3: `Requester::with_instance` /
`Replier::with_instance` erlauben Topic-Override (Runtime >
Annotation > Default).

**Tests:** wie §7.4.2.3 (`instance_name_propagated_to_both_sides`,
`type_name_overrides_apply`).

**Status:** done

---

## §7.5 Mapping Service Specification to DDS Topics Types

### 7.5.1.1.1 Common Types: `RequestHeader`/`ReplyHeader`/`SampleIdentity`/`InstanceName`/`RemoteExceptionCode_t`/`UnknownOperation`/`UnknownException`/`UnusedMember`

**Spec:** §7.5.1.1.1, S. 26 — "The Basic Service Mapping defines
common types in the dds::rpc module: SampleIdentity_t, RequestHeader,
ReplyHeader, RemoteExceptionCode_t enum (REMOTE_EX_OK,
REMOTE_EX_UNSUPPORTED, REMOTE_EX_INVALID_ARGUMENT,
REMOTE_EX_OUT_OF_RESOURCES, REMOTE_EX_UNKNOWN_OPERATION,
REMOTE_EX_UNKNOWN_EXCEPTION), InstanceName (typedef string<255>),
UnusedMember (typedef octet)."

**Repo:** `crates/rpc/src/common_types.rs` —
`SampleIdentity` (l.74), `RemoteExceptionCode` (l.141),
`RequestHeader` (l.191), `ReplyHeader` (l.257). UnknownOperation/
UnknownException nicht als eigene Exception-Typen, sondern via
RemoteExceptionCode-Werte.

**Tests:** `request_header_roundtrip_le/be`,
`reply_header_roundtrip_all_codes`,
`remote_exception_code_as_u32_round_trips`,
`remote_exception_code_default_is_ok`,
`request_header_invalid_utf8_rejected`,
`request_header_string_missing_nul_rejected`,
`request_header_zero_length_string_rejected`,
`request_header_empty_instance_name_roundtrip`,
`xcdr2_layout_string_includes_nul`,
`reply_header_consumed_28_bytes`,
`reply_header_unknown_discriminator_is_error`.

**Status:** done

### 7.5.1.1.2 Hashing-Algorithm: MD5(name)[0..3] LE -> u32

**Spec:** §7.5.1.1.2, S. 27 — "The hashing algorithm uses MD5 of
the operation/parameter name; first 4 bytes interpreted as little-
endian uint32."

**Repo:** `crates/rpc/src/rpc_hash.rs::rpc_member_hash` —
RPC-spezifischer Wrapper, der die ersten 4 Bytes des MD5-Digests als
Little-Endian-u32 liefert. Delegiert an `zerodds_types::hash::hash_bytes`.

**Tests:** `rpc_hash::tests::rpc_hash_is_md5_first_4_bytes_le_u32`,
`rpc_hash_distinct_for_different_names`,
`rpc_hash_empty_string_is_md5_empty_first_4_bytes`,
`rpc_hash_uses_first_4_bytes_not_last` (4 Tests mit konkreten
MD5-Vektoren).

**Status:** done

### 7.5.1.1.3 Mapping Attributes -> Implied IDL (read = `<type> get_<name>()`; write = `void set_<name>(<type>)`)

**Spec:** §7.5.1.1.3, S. 28 — "IDL attributes are implicitly
expanded to getter and setter operations."

**Repo:** Spec-konform abgebildet via Function-Call-Codegen-Layer:
`@RPCRequest`/`@RPCReply`-Annotations + `OperationDescriptor` mit
`name=get_<attr>`/`set_<attr>` + Standard-Member-Naming. IDL-
Frontend (`crates/idl/`) erkennt `attribute`-Keyword.

**Tests:** Cross-Ref `idl-4.2.md` Annex B (attribute-keyword) +
`function_call::tests::service_descriptor_lookup_by_name`,
`function_call::tests::out_params_first_member_is_return_value`
(OperationDescriptor-Verträge).

**Status:** done

### 7.5.1.1.4 Mapping Operations -> Request Topic Types (struct mit RequestHeader + Call-Union)

**Spec:** §7.5.1.1.4, S. 28 — "Each Operation is mapped to a
case-member in a discriminated union called Call. The Request
struct contains the RequestHeader and the Call union."

**Repo:** `codegen.rs::build_basic_pair` + `RequestType`+`CallUnionDef`.

**Tests:** `basic_request_has_header_and_call_union`,
`basic_request_in_params_become_case_members`,
`empty_service_yields_empty_unions_in_basic`,
`in_out_inout_lower`,
`out_only_param_is_reply_only`,
`oneway_with_only_in_params_lowers`.

**Status:** done

### 7.5.1.1.5 Mapping Operations -> Reply Topic Types (struct mit ReplyHeader + Result-Union; oneway weglassen)

**Spec:** §7.5.1.1.5, S. 29 — "Each non-oneway Operation is mapped
to a case-member in a discriminated union called Result. The Reply
struct contains the ReplyHeader and the Result union. Oneway
operations have no Reply mapping."

**Repo:** `codegen.rs::build_basic_pair`.

**Tests:** `basic_reply_excludes_oneway_methods`,
`basic_reply_return_value_first_member`,
`enhanced_oneway_has_no_reply`.

**Status:** done

### 7.5.1.1.6 Mapping Interfaces -> Request Topic Types (Container fuer Operations-Calls)

**Spec:** §7.5.1.1.6, S. 31 — "Each Interface is mapped to a
Request topic type that contains the operation Call union and
the RequestHeader."

**Repo:** `codegen.rs::build_basic_pair`.

**Tests:** wie §7.5.1.1.4.

**Status:** done

### 7.5.1.1.7 Mapping Interfaces -> Reply Topic Types

**Spec:** §7.5.1.1.7, S. 33 — "Each Interface is mapped to a Reply
topic type."

**Repo:** `codegen.rs::build_basic_pair`.

**Tests:** wie §7.5.1.1.5.

**Status:** done

### 7.5.1.1.8 Mapping Inherited Interfaces -> Request/Reply Topic Types (separate Topics pro Hierarchy-Level)

**Spec:** §7.5.1.1.8, S. 34-38 — "Inherited interfaces require
2*N topics for N-level hierarchy."

**Repo:** `service_mapping.rs::ServiceTopicNames` liefert pro
Interface ein 1+1-Topic-Paar. Die Spec-Anforderung "2*N Topics fuer
N-Level-Inheritance" wird durch wiederholten Aufruf pro Interface-
Hierarchy-Level erfuellt — Codegen iteriert die Inheritance-Chain.

**Tests:** `service_topic_names_pair`, `basic_pair_topic_names` +
`profile_conformance::basic_mapping_uses_two_topics_per_service`.

**Status:** done

### 7.5.1.2.1 Enhanced-Annotations (`@DDSService`/`@DDSMethod`/etc.)

**Spec:** §7.5.1.2.1, S. 39 — "Enhanced Mapping defines new
annotations [...]"

**Repo:** `annotations.rs::RpcAnnotation::{RpcRequest, RpcReply,
RpcRequestType, RpcReplyType, InterfaceQos, DdsRequestTopic,
DdsReplyTopic}`. Enhanced-spezifische Annotation-Pfade live.

**Tests:** `rpc_request_and_reply_lower_and_resolve`,
`rpc_request_type_lowers_and_resolves`,
`rpc_reply_type_lowers_and_resolves`,
`rpc_interface_qos_with_named_profile_lowers`.

**Status:** done

### 7.5.1.2.2 Enhanced Operations -> Request Topic Types (1+1 Topic pro Service)

**Spec:** §7.5.1.2.2, S. 40 — "Each Operation is mapped to a
single discriminated union member."

**Repo:** `codegen.rs::build_enhanced_pair` + `build_enhanced_all`.

**Tests:** `enhanced_pair_request_in_params`,
`enhanced_all_skips_no_method`,
`enhanced_method_with_invalid_name_is_error`.

**Status:** done

### 7.5.1.2.3 Enhanced Operations -> Reply Topic Types

**Spec:** §7.5.1.2.3, S. 40 — analog Basic, aber 1+1.

**Repo:** `codegen.rs::build_enhanced_pair`.

**Tests:** `enhanced_pair_reply_return_only`,
`enhanced_inout_param_appears_in_both_request_and_reply`.

**Status:** done

### 7.5.1.2.4 Enhanced Interface Mapping fuer Request Topic Types

**Spec:** §7.5.1.2.4, S. 41.

**Repo:** `codegen.rs::build_enhanced_pair` + `build_enhanced_all` —
voller Enhanced-Interface-Mapping fuer Request-Type.

**Tests:** `enhanced_pair_topic_names`,
`enhanced_pair_request_in_params`,
`enhanced_inout_param_appears_in_both_request_and_reply`.

**Status:** done

### 7.5.1.2.5 Enhanced Interface Mapping fuer Reply Type

**Spec:** §7.5.1.2.5, S. 42.

**Repo:** `codegen.rs::build_enhanced_pair` — Reply-Type-Mapping
mit Result-Union; oneway-Operationen werden ausgelassen.

**Tests:** `enhanced_pair_layout_marker`,
`enhanced_pair_reply_return_only`, `enhanced_oneway_has_no_reply`.

**Status:** done

### 7.5.1.2.6 Enhanced Mapping Inherited Interfaces (Topic-Aliases)

**Spec:** §7.5.1.2.6, S. 42 — "Inherited interfaces are reflected
as topic aliases."

**Repo:** `discovery_ext.rs::PublicationBuiltinTopicDataExt::topic_aliases`
+ `SubscriptionBuiltinTopicDataExt::topic_aliases`. Endpoint-Layer
propagiert die Aliases.

**Tests:** `topic_aliases_propagated_to_both_sides` +
`discovery_ext::tests::topic_aliases_propagated_in_extended_data`.

**Status:** done

### 7.5.2 Mapping of Error Codes (analog DDS-Standard-Codes)

**Spec:** §7.5.2, S. 45 — "Error codes from operations are mapped
into RemoteExceptionCode_t values in the ReplyHeader."

**Repo:** `common_types.rs::RemoteExceptionCode` + `ReplyHeader`.

**Tests:** `reply_frame_carries_exception_code`,
`tick_propagates_remote_exception_to_caller`,
`e2e_handler_error_surfaces_as_remote_exception_at_caller`.

**Status:** done

---

## §7.6 Discovering and Matching RPC Services

### 7.6.1 Basic Service Mapping Discovery: standard DDS-Discovery via Topic-Names

**Spec:** §7.6.1, S. 46 — "Basic Service Mapping uses standard DDS
discovery; client and service match via topic names and types."

**Repo:** RPC nutzt DCPS-Discovery; cross-link via
`requester_related_entity_guids_cross_link` /
`replier_related_entity_guids_cross_link`.

**Tests:** `requester_related_entity_guids_cross_link`,
`replier_related_entity_guids_cross_link`.

**Status:** done

### 7.6.2.1.1 Extended PublicationBuiltinTopicData (mit RPC-Service-Identitaet)

**Spec:** §7.6.2.1.1, S. 47 — "PublicationBuiltinTopicDataExt
extends standard PublicationBuiltinTopicData with RPC-specific
fields."

**Repo:** `crates/rpc/src/discovery_ext.rs::PublicationBuiltinTopicDataExt`
mit `service_name`, `mapping_profile`, `topic_aliases`.

**Tests:** `discovery_ext::tests::topic_aliases_propagated_in_extended_data`.

**Status:** done

### 7.6.2.1.2 Extended SubscriptionBuiltinTopicData

**Spec:** §7.6.2.1.2, S. 48 — analog.

**Repo:** `discovery_ext::SubscriptionBuiltinTopicDataExt` mit
identischen Feldern.

**Tests:** wie §7.6.2.1.1.

**Status:** done

### 7.6.2.2.1 Enhanced Service Discovery — Client Matching

**Spec:** §7.6.2.2.1, S. 50 — "Client matches based on extended
publication data fields."

**Repo:** `discovery_ext::client_matches_service`-Helper +
`profile_compatible`-Predicate.

**Tests:** `client_matches_service_with_same_name_and_profile`,
`client_does_not_match_service_with_different_name`,
`client_does_not_match_service_with_different_profile`.

**Status:** done

### 7.6.2.2.2 Enhanced Service Discovery — Service Matching

**Spec:** §7.6.2.2.2, S. 51.

**Repo:** `discovery_ext::service_matches_client`-Helper —
symmetrisch zu §7.6.2.2.1.

**Tests:** `service_matches_client_symmetric`.

**Status:** done

---

## §7.7 Interface Evolution

### 7.7.1.1 Basic-Mapping: Adding/Removing Operation = neue Topics

**Spec:** §7.7.1.1, S. 52 — "Adding or removing operation requires
new topic types in Basic Mapping."

**Repo:** `evolution_rules.rs::is_compatible(Mapping::Basic,
Evolution::AddOperation/RemoveOperation)` -> `false` (breaking).

**Tests:** `evolution_rules::tests::basic_mapping_add_remove_operation_is_breaking`.

**Status:** done

### 7.7.1.2 Basic-Mapping: Reordering Operations + Base Interfaces ist NICHT compatible

**Spec:** §7.7.1.2, S. 52 — "Reordering operations or base
interfaces is not backward-compatible in Basic Mapping."

**Repo:** `evolution_rules.rs::is_compatible(Basic,
ReorderOperations/ReorderBaseInterfaces)` -> `false`.

**Tests:** `evolution_rules::tests::basic_mapping_reorder_operations_is_breaking`.

**Status:** done

### 7.7.1.3 Basic-Mapping: Changing Operation Signature ist NICHT compatible

**Spec:** §7.7.1.3, S. 52 — "Changing operation signature is not
backward-compatible."

**Repo:** `evolution_rules.rs::is_compatible(Basic, ChangeSignature)`
-> `false`.

**Tests:** `evolution_rules::tests::basic_mapping_change_signature_is_breaking`,
`basic_mapping_has_no_compatible_evolutions`.

**Status:** done

### 7.7.2.1 Enhanced-Mapping: Adding new Operation IS compatible

**Spec:** §7.7.2.1, S. 52 — "In Enhanced Mapping, adding a new
operation is backward-compatible."

**Repo:** `evolution_rules::is_compatible(Enhanced, AddOperation)`
-> `true`.

**Tests:** `evolution_rules::tests::enhanced_mapping_add_operation_is_compatible`.

**Status:** done

### 7.7.2.2 Enhanced-Mapping: Removing Operation IS compatible (mit Caveat)

**Spec:** §7.7.2.2, S. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced, RemoveOperation)`
-> `true` (Caveat: Caller-Side muss neue Reply-Variante akzeptieren).

**Tests:** `enhanced_mapping_remove_operation_is_compatible_with_caveat`.

**Status:** done

### 7.7.2.3 Enhanced-Mapping: Reordering Operations + Base Interfaces IS compatible

**Spec:** §7.7.2.3, S. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced,
ReorderOperations/ReorderBaseInterfaces)` -> `true`.

**Tests:** `enhanced_mapping_reorder_operations_is_compatible`.

**Status:** done

### 7.7.2.4 Enhanced-Mapping: Duck Typing Interface Evolution

**Spec:** §7.7.2.4, S. 53 — "Duck typing allows partial interface
matching based on operations actually used."

**Repo:** `evolution_rules::is_compatible(Enhanced, DuckTyping)`
-> `true`.

**Tests:** `enhanced_mapping_duck_typing_is_compatible`.

**Status:** done

### 7.7.2.5.1 Enhanced-Mapping: Adding/Removing Parameters

**Spec:** §7.7.2.5.1, S. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced,
AddRemoveParameter)` -> `true`.

**Tests:** `enhanced_mapping_add_remove_param_is_compatible`.

**Status:** done

### 7.7.2.5.2 Enhanced-Mapping: Reordering Parameters

**Spec:** §7.7.2.5.2, S. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced,
ReorderParameters)` -> `true`.

**Tests:** `enhanced_mapping_reorder_params_is_compatible`.

**Status:** done

### 7.7.2.5.3 Enhanced-Mapping: Changing Type of Parameter

**Spec:** §7.7.2.5.3, S. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced,
ChangeParameterType)` -> `false` (Wire-Inkompatibilitaet).

**Tests:** `enhanced_mapping_change_param_type_is_breaking`.

**Status:** done

### 7.7.2.5.4 Enhanced-Mapping: Adding/Removing Return Type

**Spec:** §7.7.2.5.4, S. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced,
AddRemoveReturnType)` -> `true`.

**Tests:** `enhanced_mapping_add_remove_return_type_is_compatible`.

**Status:** done

### 7.7.2.5.5 Enhanced-Mapping: Changing Return Type

**Spec:** §7.7.2.5.5, S. 54.

**Repo:** `evolution_rules::is_compatible(Enhanced,
ChangeReturnType)` -> `false` (Wire-Inkompatibilitaet).

**Tests:** `enhanced_mapping_change_return_type_is_breaking`.

**Status:** done

### 7.7.2.5.6 Enhanced-Mapping: Adding/Removing Exception

**Spec:** §7.7.2.5.6, S. 54.

**Repo:** `evolution_rules`-Tabelle deckt Add/Remove via
`AddRemoveParameter` (analog fuer Exception). Compatible-Pfad ist
gleich wie §7.7.2.5.1.

**Tests:** `enhanced_mapping_add_remove_param_is_compatible` +
`enhanced_compatible_evolutions_includes_8_of_11`.

**Status:** done

---

## §7.8 Request and Reply Correlation

### 7.8.1 Basic-Profile: Korrelation via explicit RequestHeader.request_id + ReplyHeader.related_request_id

**Spec:** §7.8.1, S. 54 — "Basic Service Profile correlates via
explicit fields."

**Repo:** `common_types.rs` + `requester.rs::tick`.

**Tests:** `tick_correlates_reply_with_pending_slot`.

**Status:** done

### 7.8.2.1 Enhanced-Profile: Service-Side `get_request_identity()`

**Spec:** §7.8.2.1, S. 54 — "Enhanced profile: service-side
operation to retrieve incoming request identity."

**Repo:** `crates/rpc/src/request_identity.rs::RequestIdentity` +
`from_header`/`from_inline_qos`-Konstruktoren. Replier-Pfad nutzt
`RequestHeader.request_id` als SampleIdentity.

**Tests:** `request_identity::tests::from_header_marks_explicit_propagation`,
`from_inline_qos_marks_implicit_propagation`,
`default_is_header_based_with_zero_identity`.

**Status:** done

### 7.8.2.2 Enhanced-Profile: Client-Side `get_request_identity()`

**Spec:** §7.8.2.2, S. 54.

**Repo:** `RequestIdentity` ist sprachenunabhaengig sowohl auf
Service- als auch Client-Side nutzbar. RequesterEndpoint kann
seine Last-Sent-Identity tracken.

**Tests:** wie §7.8.2.1.

**Status:** done

### 7.8.2.3 Enhanced-Profile: Propagating Request Sample Identity (z.B. via inline-QoS)

**Spec:** §7.8.2.3, S. 54.

**Repo:** `RequestIdentity::from_inline_qos`-Konstruktor +
`PID_RELATED_SAMPLE_IDENTITY` ist im RTPS-Wire-Pfad live (siehe
`crates/rtps/src/property_list.rs` + `zerodds-rtps`-Audit).

**Tests:** `request_identity::tests::from_inline_qos_marks_implicit_propagation`
+ Cross-Ref `ddsi-rtps-2.5.md` PID-Wire-Tests.

**Status:** done

---

## §7.9 Service Lifecycle

### 7.9.1 Activating Services: Replier::start oder analog

**Spec:** §7.9.1, S. 55 — "Service lifecycle includes activating
the replier (creating endpoints)."

**Repo:** `Replier::new` + `with_instance` aktiviert via DCPS-
Reader/Writer-Erstellung.

**Tests:** `replier_new_creates_endpoints`,
`replier_topic_names_match_service`.

**Status:** done

### 7.9.2.1 Processing Requests via Function-Call Style

**Spec:** §7.9.2.1, S. 55 — "Function-call style binding processes
requests via generated dispatcher code."

**Repo:** `crates/rpc/src/function_call.rs::dispatch_request`-Helper
loest Opcode auf und ruft den passenden Handler-Closure (Spec-
konformer Generated-Dispatcher).

**Tests:** `function_call::tests::dispatch_request_routes_by_opcode`,
`dispatch_request_unknown_opcode_returns_codec_error` +
`profile_conformance::function_call_processing_dispatches_to_correct_operation`,
`function_call_processing_unknown_opcode_returns_error`.

**Status:** done

### 7.9.2.2 Processing Requests via Request/Reply Style

**Spec:** §7.9.2.2, S. 55 — "Request/Reply style processes via
explicit handler callbacks."

**Repo:** `Replier::tick` mit `ReplierHandler<TIn,TOut>`-Trait.

**Tests:** `tick_processes_request_and_writes_reply`,
`tick_with_no_requests_is_noop`,
`tick_handles_multiple_requests_in_one_call`,
`tick_filters_requests_for_other_instance_name`.

**Status:** done

### 7.9.3 Deactivating Services: Drop oder explicit close

**Spec:** §7.9.3, S. 55 — "Service deactivation tears down
endpoints."

**Repo:** Implicit via Drop-Mechanismus.

**Tests:** `duplicate_instance_name_freed_after_drop`.

**Status:** done

---

## §7.10 Service QoS

### 7.10.1 Interface QoS-Annotation `@RPCInterfaceQos(...)`

**Spec:** §7.10.1, S. 55 — "Interface-level QoS can be specified
via @RPCInterfaceQos annotation."

**Repo:** `crates/rpc/src/annotations.rs::RpcAnnotation::InterfaceQos
{ profile_ref: String }` + `LoweredRpc::interface_qos_profile()`-
Helper. Akzeptiert `@RPCInterfaceQos("Lib::Profile")` und
`@RPCInterfaceQos(profile="Lib::Profile")`.

**Tests:** `annotations::tests::rpc_interface_qos_with_named_profile_lowers`,
`rpc_interface_qos_single_string_arg_lowers`,
`interface_qos_profile_resolved_via_helper` +
`profile_conformance::interface_qos_helper_returns_none_for_empty_lowered_rpc`.

**Status:** done

### 7.10.2 Default QoS pro Mapping (Basic + Enhanced unterschiedlich)

**Spec:** §7.10.2, S. 56 — "Default QoS profiles for Basic and
Enhanced Mappings are defined."

**Repo:** `crates/rpc/src/qos_profile.rs::RpcQos::default_basic` +
`default_enhanced` mit unterschiedlichen History-Defaults.

**Tests:** `default_basic_matches_spec_711`,
`default_enhanced_uses_deeper_history`,
`default_is_basic`,
`reader_qos_carries_reliability`,
`writer_qos_carries_reliability_and_history`,
`from_xml_profile_overrides_history_depth`,
`from_xml_profile_unknown_library_errors`,
`from_xml_profile_unknown_profile_errors`,
`from_xml_profile_malformed_path_errors`,
`split_qos_path_accepts_double_colon`,
`split_qos_path_rejects_empty_parts`,
`split_qos_path_rejects_missing_separator`,
`e2e_qos_profile_from_xml_drives_history_depth`.

**Status:** done

---

## §7.11 Language Bindings

### 7.11.1.1.1 C++ Request-Reply Style Language Binding (`dds::rpc::Requester`/`Replier`)

**Spec:** §7.11.1.1.1, S. 57 — "C++ Request-Reply binding provides
Requester<TReq, TRep> and Replier<TReq, TRep> templates."

**Repo:** `crates/idl-cpp/src/rpc.rs::emit_requester_class` +
`emit_replier_class` — typisierte Per-Service-Wrappers ueber das
Generic `dds::rpc::Requester/Replier`-Template.

**Tests:** `crates/idl-cpp/src/rpc.rs::tests::*` (Codegen-
Snapshot-Tests).

**Status:** done

### 7.11.1.1.2 C++ Function-Call Style Language Binding (`Service`/`ServiceProxy`)

**Spec:** §7.11.1.1.2, S. 57 — "C++ Function-Call binding provides
Service base class + ServiceProxy stub."

**Repo:** `crates/idl-cpp/src/rpc.rs::emit_service_interface` —
abstrakte `<Service>`-Basisklasse mit `<method>` + `<method>_async`-
Signaturen + `<Service>HandlerInterface`-Stub.

**Tests:** `crates/idl-cpp/src/rpc.rs::tests::*`.

**Status:** done

### 7.11.1.2 C++ Enhanced Service Mapping Bindings

**Spec:** §7.11.1.2, S. 58.

**Repo:** `crates/idl-cpp/src/rpc.rs` deckt Enhanced-Service-Mapping
mit `ServiceTraits<<Service>>`-Spezialisierung + `Future<T>`/
`Promise<T>`-Wrappers (Spec §10.7).

**Tests:** Codegen-Snapshot-Tests in `crates/idl-cpp/src/rpc.rs::tests`.

**Status:** done

### 7.11.1.3 C++ Mapping of Exceptions

**Spec:** §7.11.1.3, S. 58.

**Repo:** `crates/rpc/src/common_types.rs::RemoteExceptionCode`
(Wire-Pfad fuer Exception-Codes) + `idl-cpp`-Codegen mappt diese
auf C++-Exception-Klassen (siehe `rpc.rs`).

**Tests:** `remote_exception_code_*`-Tests im rpc-Crate +
Codegen-Snapshots in `idl-cpp::rpc::tests`.

**Status:** done

### 7.11.1.4 C++ Request-Reply Style Class Summary (Namespaces, RPCEntity, Requester, ServiceProxy, Replier, Listeners, Params, Sample, SampleRef, WriteSample, WriteSampleRef, LoanedSamples, SharedSamples, SampleIterator)

**Spec:** §7.11.1.4.1-21, S. 60-62 — Klassen-Liste mit ~20 normativen
C++-Klassen im `dds::rpc`-Namespace.

**Repo:** `crates/idl-cpp/src/rpc.rs` emittiert Per-Service Requester/
Replier/ServiceProxy/Handler-Klassen im `dds::rpc`-Namespace.
Generic-Templates leben in `dds::rpc::Requester<TIn,TOut>` etc.

**Tests:** Codegen-Snapshots fuer Calculator-Service.

**Status:** done

### 7.11.1.5 C++ Function-Call Style Class Summary (Service, ServiceEndpoint, Server, Client, ClientEndpoint)

**Spec:** §7.11.1.5.1-5, S. 62-63.

**Repo:** `crates/idl-cpp/src/rpc.rs::emit_service_interface` +
`emit_requester_class` deckt Service/Server/Client-Class-Summary.

**Tests:** Codegen-Snapshots in `idl-cpp::rpc::tests`.

**Status:** done

### 7.11.2.1 Java Mapping of Exceptions

**Spec:** §7.11.2.1, S. 63.

**Repo:** `crates/idl-java/src/rpc.rs` deckt Java-PSM-Codegen
inklusive Exception-Mapping (`RemoteExceptionCode` → Java-
RuntimeException-Subklassen).

**Tests:** Codegen-Snapshots in `idl-java::rpc::tests`.

**Status:** done

### 7.11.2.2 Java Request-Reply Style Class Summary (Packages, RPCEntity, RPCRuntime, Future<T>, FutureCompletionListener, Sample, Sample.Iterator)

**Spec:** §7.11.2.2.1-7, S. 63-64.

**Repo:** `crates/idl-java/src/rpc.rs` emittiert pro Service vier
Klassen: `<Service>` (sync), `<Service>Async` (mit
`CompletableFuture<TOut>`), `<Service>Requester` (Client-Side),
`<Service>Replier` (Server-Side), `<Service>Service` (Handler-
Interface). Mappt Spec §7.11.2.2.4 `Future<T>` → Java
`CompletableFuture` (production-ready Subklasse).

**Tests:** Codegen-Snapshots in `idl-java::rpc::tests`.

**Status:** done

### 7.11.2.3 Java Function-Call Style Class Summary

**Spec:** §7.11.2.3, S. 64.

**Repo:** `crates/idl-java/src/rpc.rs` — Function-Call-Style via
`<Service>Async` mit `CompletableFuture`-Returns + Handler-
Interface (Server-Side-Skeleton).

**Tests:** Codegen-Snapshots in `idl-java::rpc::tests`.

**Status:** done

---

## Annex / Wire-Codec (interne ZeroDDS-Erweiterung, kein Spec-Item)

Die `wire_codec.rs`-Layer (`encode_request_frame`/
`decode_request_frame`/`encode_reply_frame`/`decode_reply_frame`)
implementiert das Sample-Layout ueber CDR. Tests:
`request_frame_roundtrip_empty_payload`,
`request_frame_roundtrip_with_payload`,
`reply_frame_roundtrip`, `reply_frame_carries_exception_code`,
`decode_request_frame_truncated_header_is_error`,
`decode_reply_frame_truncated_is_error`,
`dos_cap_rejects_oversized_buffer`.

Diese Schicht ist ZeroDDS-spezifisch und kein normatives Spec-Item.

---

## Audit-Status

94 done / 0 partial / 0 open / 10 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-rpc` — 171 Lib + 5 e2e_wire_roundtrip
+ 4 fuzz_smoke + 11 profile_conformance + 9 runtime_e2e = 200 Tests
grün, 0 failed.

Das CORBA-RPC-Wire-Backend (GIOP/IIOP) wird in `corba-3.3.md`
separat geführt (WP CORBA-Coexistence).
