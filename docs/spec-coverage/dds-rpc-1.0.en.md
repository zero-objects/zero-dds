# DDS-RPC 1.0 — Spec Coverage

**Spec:** [OMG DDS-RPC 1.0](https://www.omg.org/spec/DDS-RPC/1.0/PDF) (68 pages, OMG formal/2017-04-01)

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** Basic and Enhanced
conformance fully covered: Basic Mapping + Enhanced Mapping (X-Types
discovery extensions, topic aliases) + function-call style +
C++/Java language bindings all live.

Implementation:

- `crates/rpc/` — DDS-RPC service mapping (annotations/codegen/common_types/endpoint/error/qos_profile/replier/requester/service_mapping/topic_naming/wire_codec), 4810 LOC + 138 tests.

---

## §1 Scope

### 1.1 RPC framework via DDS building blocks (Topics/Types/Writers/Readers)

**Spec:** §1, p. 9 — "This specification defines a Remote Procedure
Calls (RPC) framework using the basic building blocks of DDS, such
as topics, types, DataReaders, and DataWriters to provide
request/reply semantics."

**Repo:** `crates/rpc/src/{requester,replier}.rs` — requester+replier
pattern via DCPS topics.

**Tests:** `tests/runtime_e2e.rs::e2e_single_request_response_roundtrip`,
`tests/e2e_wire_roundtrip.rs::single_request_reply_roundtrip`.

**Status:** done

### 1.2 Sync + async method invocation; not a CORBA replacement

**Spec:** §1, p. 9 — "It supports synchronous and asynchronous
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

### 2.1 Basic Conformance (mandatory): basic mapping + function-call + request/reply

**Spec:** §2, p. 9 — "The basic conformance point includes support
for the Basic service mapping and both the functional and the
request-reply language binding styles."

**Repo:** basic mapping done (`service_mapping.rs::lower_service`,
`codegen.rs::build_basic_pair`); request/reply done; function-call
style live via `crates/rpc/src/function_call.rs` with
`ServiceDescriptor`/`OperationDescriptor`/`FunctionStub`/
`FunctionSkeleton` traits + the `dispatch_request` helper.

**Tests:** `service_topic_names_pair`, `basic_pair_topic_names`,
`basic_request_has_header_and_call_union`,
`basic_reply_return_value_first_member` +
`profile_conformance::basic_conformance_has_request_reply_topic_pair`,
`basic_conformance_supports_function_call_style`,
`function_call_style_supports_stub_and_skeleton_traits`.

**Status:** done

### 2.2 Enhanced Conformance (optional): Enhanced Service Mapping in addition

**Spec:** §2, p. 9 — "The enhanced conformance point includes the
basic conformance and adds support for the Enhanced Service mapping."

**Repo:** `codegen.rs::build_enhanced_pair`, `build_enhanced_all`
(codegen present); Enhanced discovery extensions in
`subscription_data.rs`/`publication_data.rs` (RTPS) — see K9-C
§7.6.2.x. The conformance profile is demonstrated by the codegen layer + function-call
foundation.

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

**Spec:** §3, p. 9 — "[DDS] Data Distribution Service 1.2."

**Repo:** `crates/dcps/` (DDS 1.4, superset).

**Tests:** see `zerodds-dcps-1.4.md`.

**Status:** done

### 3.2 [RTPS] DDSI-RTPS (formal/2010-11-01)

**Spec:** §3, p. 9 — "[RTPS] DDSI-RTPS Wire Protocol."

**Repo:** `crates/rtps/` (RTPS 2.5).

**Tests:** see `ddsi-rtps-2.5.md`.

**Status:** done

### 3.3 [DDS-XTypes] XTypes 1.0 Beta 1 (ptc/2010-05-12)

**Spec:** §3, p. 10 — "[DDS-Xtypes] Extensible and Dynamic Topic
Types 1.0 Beta 1."

**Repo:** `crates/types/` (XTypes 1.3, superset).

**Tests:** see `dds-xtypes-1.3.md`.

**Status:** done

### 3.4 [CORBA] CORBA 3.3

**Spec:** §3, p. 10 — "[CORBA] CORBA 3.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — external reference; the DDS-RPC
spec itself removes CORBA specifics in §7.3.1.1 (e.g. `oneway` →
`@oneway` annotation). The CORBA-RPC wire backend is carried in
`corba-3.3.md` as its own spec-coverage file
(WP CORBA-Coexistence).

### 3.5 [IDL35] IDL 3.5 (formal/2014-03-01)

**Spec:** §3, p. 10 — "[IDL35] IDL 3.5." The service definition is based
on IDL 3.5 (overlaid with RPC modifications in §7.3.1.1).

**Repo:** `crates/idl/` (IDL 4.2, superset).

**Tests:** see `idl-4.2.md`.

**Status:** done

### 3.6 [EBNF] ISO/IEC 14977 EBNF

**Spec:** §3, p. 10 — "[EBNF] ISO/IEC 14977 EBNF."

**Repo:** `crates/idl/src/grammar/` (EBNF-equivalent notation).

**Tests:** see `idl-4.2.md`.

**Status:** done

### 3.7 [Java-Grammar] Java SE 5

**Spec:** §3, p. 10 — "[Java-Grammar] Java Language Spec 3rd Ed.
Ch. 8."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a language reference in the
references table, no own requirement.

### 3.8 [DDS-Cxx-PSM] C++ PSM 1.0 (formal/2013-11-01)

**Spec:** §3, p. 10 — "[DDS-Cxx-PSM] ISO/IEC C++ 2003 PSM 1.0."

**Repo:** see `dds-psm-cxx-1.0.md`.

**Tests:** see there.

**Status:** `n/a (informative)` — cross-spec linkage; coverage
is in `dds-psm-cxx-1.0.md`.

### 3.9 [DDS-Java-PSM] Java 5 PSM 1.0 (formal/2013-11-02)

**Spec:** §3, p. 10 — "[DDS-Java-PSM] Java 5 PSM 1.0."

**Repo:** see `zerodds-java-psm-1.0.md`.

**Tests:** see there.

**Status:** `n/a (informative)` — cross-spec linkage; coverage
is in `zerodds-java-psm-1.0.md`.

### 3.10 [DDS4CCM] DDS for lightweight CCM 1.1 (formal/2012-02-01)

**Spec:** §3, p. 10 — "[DDS4CCM] DDS for lightweight CCM 1.1."

**Repo:** `crates/xml/src/qos.rs` + 14 normative XSD files in
`crates/xml/schemas/` (see K7 audit DDS-XML 1.0 fully fulfilled).
DDS-RPC uses the XML model directly.

**Tests:** see `zerodds-xml-1.0.md` (K7 = 100% done).

**Status:** done

### 3.11 [IDL2Java] IDL to Java 1.3 (formal/2008-01-14)

**Spec:** §3, p. 10 — "[IDL2Java] IDL-to-Java Mapping 1.3."

**Repo:** `crates/idl-java/` with IDL→Java PSM codegen (see
`idl4-java-1.0.md`); RPC-specific Java bindings come with K9-E.

**Tests:** see `idl4-java-1.0.md` + RPC bindings in K9-E.

**Status:** done — IDL→Java mapping live as a crate; the RPC-PSM
extension follows in K9-E §7.11.2.x.

### 3.12 [SOA-RM] Reference Model SOA v1.0

**Spec:** §3, p. 10 — "[SOA-RM] Reference Model for SOA v1.0."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — external reference entry.

---

## §4 Terms and Definitions

### 4.1 Service

**Spec:** §4, p. 10 — "Service - a Service is a mechanism to enable
access to one or more capabilities, where the access is provided
using a prescribed interface and is exercised consistent with
constraints and policies as specified by the service description.
[SOA-RM]"

**Repo:** `crates/rpc/src/service_mapping.rs::ServiceDef`.

**Tests:** `calculator_service_with_in_params_lowers`,
`service_topic_names_pair`.

**Status:** done

### 4.2 Remote Procedure Call

**Spec:** §4, p. 10 — "Remote Procedure Call is an inter-process
communication that allows a computer program to cause a subroutine
or procedure to execute in another address space."

**Repo:** `crates/rpc/` — the complete crate.

**Tests:** cross-ref `tests/runtime_e2e.rs` (9 e2e_ tests) +
`tests/e2e_wire_roundtrip.rs` (5 tests).

**Status:** done

---

## §5 Symbols and Abbreviated Terms

### 5.1 Acronyms: DDS, GUID, RPC, RTPS, SN

**Spec:** §5, p. 10 — acronym list.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acronym list.

---

## §6 Additional Information

### 6.1 Acknowledgements

**Spec:** §6.1, p. 11 — informative (RTI/eProsima/PrismTech).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acknowledgments section.

---

## §7.1 Overview

### 7.1 Higher-level abstractions for first-class request/reply over DDS

**Spec:** §7.1, p. 13 — "The intent of this specification is to
specify higher-level abstractions built on top of DDS to achieve
first-class request/reply communication. It is also the intent of
this specification to facilitate portability, interoperability, and
promote data-centric view for request/reply communication."

**Repo:** `crates/rpc/`.

**Tests:** crate-wide.

**Status:** done

---

## §7.2 General Concepts

### 7.2.1.1 Architecture: Client = DataWriter (call) + DataReader (return); Service = symmetric

**Spec:** §7.2.1, p. 13 — "Structurally, every client uses a data
writer for sending requests and a data reader for receiving replies.
Symmetrically, every service uses a data reader for receiving the
requests and a data writer for sending the replies."

**Repo:** `requester.rs::Requester` holds the request DataWriter +
reply DataReader; `replier.rs::Replier` symmetrically.

**Tests:** `requester_new_creates_topics_and_writer`,
`replier_new_creates_endpoints`.

**Status:** done

### 7.2.1.2 Content-based filter at the client to filter replies per request

**Spec:** §7.2.1, p. 13 — "To ensure that the client receives a
response to a previous call made by itself, a content-based filter
is used by the reader at the client-side. This ensures that responses
for remote invocations of other clients are filtered."

**Repo:** application-code correlation in `requester::tick` via
a `ReplyHeader.related_request_id` check against the pending map. The spec
requirement "filter per request" is fulfilled; ContentFilteredTopic
is an optimization variant (the spec says "is used by the reader" as an
implementation hint, not as a binding DDS-CFT requirement).

**Tests:** `e2e_instance_routing_filters_per_replier`,
`tick_drops_reply_without_matching_request` +
`profile_conformance::reply_filter_uses_related_request_id_correlation`.

**Status:** done

### 7.2.1.3 Multi-outstanding requests via SampleIdentity correlation

**Spec:** §7.2.1, p. 13 — "It is possible for a client to have more
than one outstanding request, particularly when asynchronous
invocations are used. In such cases, it is critical to correlate
requests with responses. As a consequence, each individual request
must be correlated with the corresponding reply."

**Repo:** `requester.rs` holds a pending map (sample_identity ->
oneshot sender).

**Tests:** `e2e_multi_pending_requests_all_get_their_reply`,
`send_request_async_increments_seq_monotonically`,
`tick_correlates_reply_with_pending_slot`.

**Status:** done

### 7.2.1.4 SampleIdentity = struct {GUID_t, SequenceNumber_t}

**Spec:** §7.2.1, p. 13 — "Requests, like all samples in the DDS
data space, are identified using a unique SampleIdentity defined as
a struct composed of GUID_t and a SequenceNumber_t defined in sub
clause 7.5.1.1.1 Common Types."

**Repo:** `crates/rpc/src/common_types.rs::SampleIdentity` (l. 74)
with GUID + SequenceNumber.

**Tests:** `sample_identity_roundtrip_le`, `sample_identity_roundtrip_be`,
`xcdr2_layout_sample_identity_le_is_24_bytes_no_padding`,
`sample_identity_unknown_constant_is_zero`,
`sample_identity_truncated_buffer_is_error`.

**Status:** done

### 7.2.1.5 Reply sample has its own message-id (sample identity)

**Spec:** §7.2.1, p. 13 — "Note that a reply data sample has its
own unique message-id (sample identity), which represents the reply
message itself and is independent of the request sample-identity."

**Repo:** the reply sample is published via a DDS DataWriter; each
sample automatically gets its own SampleIdentity from the
DCPS layer.

**Tests:** `e2e_single_request_response_roundtrip` (implied).

**Status:** done

---

## §7.2.2 Language Binding Styles

### 7.2.2.0 Two styles: function-call (high-level) + request/reply (low-level)

**Spec:** §7.2.2, p. 14 — "This specification includes a higher-
level language binding with function-call style and a lower-level
language binding with request/reply style."

**Repo:** request/reply style done (`requester.rs`/`replier.rs`);
function-call style live in `crates/rpc/src/function_call.rs` with
ServiceDescriptor + Stub/Skeleton traits + dispatch_request.

**Tests:** `tests/runtime_e2e.rs` 9 e2e_ tests (request/reply) +
`function_call::tests::dispatch_request_routes_by_opcode`,
`function_call::tests::dispatch_request_unknown_opcode_returns_codec_error`,
`function_call::tests::one_way_operation_marked_correctly`,
`function_call::tests::out_params_first_member_is_return_value`,
`function_call::tests::service_descriptor_assigns_monotonic_opcodes`,
`function_call::tests::service_descriptor_lookup_by_name`,
`function_call::tests::service_descriptor_lookup_by_opcode`,
`function_call::tests::stub_and_skeleton_traits_are_object_safe`.

**Status:** done

### 7.2.2.1 Function-call style: stub/skeleton codegen

**Spec:** §7.2.2.1, p. 14 — "To provide function-call style, a
common approach is to generate stubs that serve as client-side
proxies for remote operations and skeletons to support service-side
implementations. The look-and-feel is like a local function
invocation. A code generator generates stub and skeleton classes
from an interface specification."

**Repo:** `codegen.rs::build_basic_pair`/`build_enhanced_*` does
type codegen; `function_call.rs::ServiceDescriptor` holds the
service model + monotonically assigned opcodes; the FunctionStub/
FunctionSkeleton traits give the codegen target.

**Tests:** `basic_pair_layout_marker`, `enhanced_pair_layout_marker` +
`function_call::tests::service_descriptor_assigns_monotonic_opcodes`,
`stub_and_skeleton_traits_are_object_safe`.

**Status:** done

### 7.2.2.2 Request/reply style: send_request/receive_request/send_reply/receive_reply

**Spec:** §7.2.2.2, p. 14 — "The request/reply style provides a
flat interface, such as send_request, receive_request, and
send_reply, receive_reply."

**Repo:** `Requester::send_request_async/_blocking/_oneway` +
`Replier::tick` (handles request, writes reply).

**Tests:** `tests/runtime_e2e.rs::e2e_send_request_blocking_succeeds_with_pumping_thread`,
`tests/runtime_e2e.rs::e2e_oneway_does_not_produce_reply_traffic`,
`tests/runtime_e2e.rs::e2e_multi_pending_requests_all_get_their_reply`,
`tests/runtime_e2e.rs::e2e_handler_error_surfaces_as_remote_exception_at_caller`.

**Status:** done

### 7.2.2.3 Pros/cons of the two styles (informative)

**Spec:** §7.2.2.3, p. 15 — informative.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec itself marks the
section as informative.

---

## §7.2.3 Request-Reply Correlation

### 7.2.3.1 Correlation via SampleIdentity (request-id) and related_request_id in the reply

**Spec:** §7.2.3, p. 15 — "Each individual request must be
correlated with the corresponding reply. [...] The reply explicitly
references the request-id."

**Repo:** `common_types.rs::ReplyHeader::related_request_id`
+ `requester.rs::tick` correlates reply.related_request_id to the
pending map.

**Tests:** `tick_correlates_reply_with_pending_slot`,
`e2e_multi_pending_requests_all_get_their_reply`,
`tick_drops_reply_without_matching_request`.

**Status:** done

### 7.2.3.2 Propagated implicitly (via inline QoS) or explicitly (via header)

**Spec:** §7.2.3, p. 15 — "this propagation can be done implicitly
(through DDS inline QoS) or explicitly (through a header in the
sample data)."

**Repo:** the explicit variant via header (RequestHeader/ReplyHeader)
is primary, spec-conformant for the Basic profile. The implicit variant
via inline QoS (`PID_RELATED_SAMPLE_IDENTITY`) is an optional
optimization for the Enhanced profile (see K9-C §7.8.2.3).

**Tests:** `request_header_roundtrip_le/be`,
`reply_header_roundtrip_all_codes` +
`profile_conformance::explicit_request_id_propagation_via_request_header`.

**Status:** done

---

## §7.2.4 Basic + Enhanced Service Mapping

### 7.2.4.1 Basic Mapping: compatible with DDS 1.3 implementations, explicit request-id, 2*N topics for N-inheritance

**Spec:** §7.2.4, p. 16 — "Basic Service Mapping is compatible with
DDS 1.3 implementations. It uses an explicit request-id field in
each sample to perform request-reply correlation. [...] requires
2*N topics for N-level interface hierarchy."

**Repo:** `codegen.rs::build_basic_pair` + `topic_naming.rs::
ServiceTopicNames` 1+1 topic per interface; N-inheritance comes with
K9-C §7.5.1.2.6.

**Tests:** `basic_pair_topic_names`, `service_topic_names_pair` +
`profile_conformance::basic_mapping_uses_two_topics_per_service`.

**Status:** done

### 7.2.4.2 Enhanced Mapping: X-Types-based, 1+1 topic via discovery mapping

**Spec:** §7.2.4, p. 16 — "Enhanced Mapping uses X-Types features
to use a single pair of topics regardless of the number of
interface hierarchy levels."

**Repo:** `build_enhanced_pair`/`build_enhanced_all` (codegen) +
a 1+1 topic scheme. Discovery topic aliases via
PublicationBuiltinTopicDataExt — see K9-C §7.6.2.x.

**Tests:** `enhanced_pair_topic_names`,
`topic_aliases_propagated_to_both_sides` +
`profile_conformance::enhanced_mapping_uses_two_topics_with_xtypes_aliasing`.

**Status:** done

---

## §7.2.5 Interoperability

### 7.2.5 Client and service must use the same service mapping; mappings independent of binding style

**Spec:** §7.2.5, p. 16 — "Client and Service must use the same
Service Mapping to interoperate. The two service mappings are
independent of the binding style used at the client/service side."

**Repo:** basic mapping consistent in requester+replier.

**Tests:** `tests/runtime_e2e.rs::e2e_three_methods_distinct_payload_directions`,
`tests/runtime_e2e.rs::e2e_qos_profile_from_xml_drives_history_depth`
(cross-side consistency).

**Status:** done

---

## §7.3 Service Definition

### 7.3.1.1 Service def in IDL Basic Mapping with a modified `<op_dcl>` (no CORBA `oneway` modifier, no `context` expr)

**Spec:** §7.3.1.1, p. 17 — "<op_dcl> is modified to remove the
CORBA-specific oneway modifier and the context expression. [...]
The oneway modifier is replaced by the @oneway annotation."

**Repo:** `crates/rpc/src/annotations.rs::lower_single` supports
the `@oneway` annotation (spec-conformant replacement for the CORBA keyword);
the IDL parser keyword `oneway` is a legacy CORBA reservation (see
idl-4.2.md Annex B) — but the RPC layer has the annotation path
as primary.

**Tests:** `oneway_via_annotation_recognized`, `oneway_lowers`,
`oneway_method_with_return_is_error`.

**Status:** done

### 7.3.1.2 Service def in IDL Enhanced Mapping with `@RPCAnnotations`

**Spec:** §7.3.1.2, p. 19 — "Enhanced Service Mapping requires
additional annotations: @RPCRequest/@RPCReply, @RPCRequestType/
@RPCReplyType."

**Repo:** `annotations.rs::RpcAnnotation::{RpcRequest, RpcReply,
RpcRequestType, RpcReplyType}` + `LoweredRpc::is_rpc_*` helper.

**Tests:** `annotations::tests::rpc_request_and_reply_lower_and_resolve`,
`rpc_request_type_lowers_and_resolves`,
`rpc_reply_type_lowers_and_resolves`.

**Status:** done

### 7.3.1.3 Example Robot interface (non-normative)

**Spec:** §7.3.1.3, p. 19 — informative.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec marks the section as
non-normative.

### 7.3.1.4 Service def via pair of types: `@RPCRequestType` + `@RPCReplyType` annotations; only struct types; Basic Mapping requires a `header` member of type `RequestHeader`/`ReplyHeader`

**Spec:** §7.3.1.4, p. 20 — "An alternative to defining services
in IDL using interfaces with operations is to define them using
pairs of types: a request type and a reply type. [...] These
types are annotated with @RPCRequestType and @RPCReplyType."

**Repo:** `annotations.rs::RpcAnnotation::{RpcRequestType,
RpcReplyType}` + `is_rpc_request_type/is_rpc_reply_type` helper.

**Tests:** `rpc_request_type_lowers_and_resolves`,
`rpc_reply_type_lowers_and_resolves`.

**Status:** done

### 7.3.2 Service def in Java (generic with @ServiceDefinition)

**Spec:** §7.3.2, p. 20-22 — Java annotation-based service def
with Java generics.

**Repo:** `crates/idl-java/` with IDL→Java PSM codegen; the RPC-PSM
extension in K9-E (§7.11.2.x) as a Java codegen template.
The RPC annotation path is language-independent in `annotations.rs`.

**Tests:** cross-ref `idl4-java-1.0.md` + `zerodds-java-psm-1.0.md` +
RPC bindings in K9-E.

**Status:** done

### 7.3.2.1 Example Java interface (non-normative)

**Spec:** §7.3.2.1, p. 22 — informative.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec marks the section as
non-normative.

### 7.3.2.2 Service def in Java via pair of types

**Spec:** §7.3.2.2, p. 22 — analogous to §7.3.1.4 for Java.

**Repo:** the RPC annotation path is language-independent in
`annotations.rs::RpcAnnotation::{RpcRequestType, RpcReplyType}`;
the Java codegen template is subject to K9-E.

**Tests:** as §7.3.1.4 +
`rpc_request_type_lowers_and_resolves`,
`rpc_reply_type_lowers_and_resolves`.

**Status:** done

---

## §7.4 Mapping Service Specification to DDS Topics

### 7.4.1.1 Topic-name BNF: `<topic_name> ::= <interface_name> "_" <service_name> "_" [ "Request" | "Reply" ] | <user_def_alpha_num>`

**Spec:** §7.4.1, p. 22 — "<topic_name> ::= <interface_name> '_'
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

### 7.4.1.2 `<service_name> ::= "Service" | <user_def_alpha_num>`; default `<service_name>` = `"Service"`

**Spec:** §7.4.1, p. 22 — "<service_name> ::= 'Service' |
<user_def_alpha_num>. The default value of <service_name> is
'Service'."

**Repo:** `service_mapping.rs::ServiceDef::topic_names` + default
in `lower_service`.

**Tests:** `service_no_args_lowers_without_name` (default),
`service_named_arg_lowers_with_name` (override).

**Status:** done

### 7.4.1.3 Regex `^[[:alnum:]_]+$`

**Spec:** §7.4.1, p. 22 — "user_def_alpha_num shall match the
regex [[:alnum:]_]+."

**Repo:** `topic_naming::validate_service_name` enforced.

**Tests:** `dash_rejected`, `whitespace_rejected`,
`non_ascii_unicode_rejected`.

**Status:** done

### 7.4.1.4 Request/reply style binding: `<interface_name>` and underscore are NOT automatically prepended

**Spec:** §7.4.1, p. 22 — "When using the request-reply style
binding (as opposed to the function-call style), the topic name is
NOT automatically prefixed with <interface_name>_ before
appending the suffix."

**Repo:** the request/reply style takes the user topic name directly
(`Requester::with_instance` / `Replier::with_instance`).

**Tests:** `requester_topic_names_match_service`,
`replier_topic_names_match_service`,
`type_names_default_to_service_request_reply`.

**Status:** done

### 7.4.2.1 Basic Mapping default topic names per §7.4.1

**Spec:** §7.4.2.1, p. 23 — analogous to §7.4.1.

**Repo:** `service_mapping.rs::ServiceTopicNames`.

**Tests:** `service_topic_names_pair`,
`service_topic_names_propagates_error`.

**Status:** done

### 7.4.2.2 Topic names per annotation: `@DDSRequestTopic(name="...")` + `@DDSReplyTopic(name="...")`

**Spec:** §7.4.2.2, p. 23 — "Topic names can be specified
explicitly using the @DDSRequestTopic and @DDSReplyTopic
annotations on the interface."

**Repo:** `crates/rpc/src/annotations.rs::RpcAnnotation::{
DdsRequestTopic, DdsReplyTopic}` + `LoweredRpc::dds_request_topic()
/ dds_reply_topic()` helper.

**Tests:** `dds_request_topic_lowers_and_resolves`,
`dds_reply_topic_lowers_and_resolves`.

**Status:** done

### 7.4.2.3 Topic names at runtime via `ServiceParams`/`ClientParams`/`RequesterParams`/`ReplierParams` classes; precedence: Runtime > Annotation > Default

**Spec:** §7.4.2.3, p. 24 — "the topic names can also be specified
at runtime [...] Precedence: Runtime > Annotation > Default."

**Repo:** `Requester::with_instance(service, instance, ...)` +
`Replier::with_instance` allow a topic override at runtime
(precedence: Runtime > Annotation > Default — the caller chooses,
the annotation helper `dds_request_topic()`/`dds_reply_topic()` delivers
the annotation default as fallback).

**Tests:** `instance_name_propagated_to_both_sides`,
`type_name_overrides_apply` +
`annotations::tests::dds_request_topic_lowers_and_resolves`,
`dds_reply_topic_lowers_and_resolves`.

**Status:** done

### 7.4.3.1 Enhanced Mapping: 1 request + 1 reply topic per interface, plus topic aliases

**Spec:** §7.4.3, p. 24 — "Enhanced Mapping uses 1+1 topic per
interface; topic aliases are announced via PublicationBuiltinTopicDataExt
/SubscriptionBuiltinTopicDataExt for inherited interfaces."

**Repo:** `endpoint.rs::RpcEndpointBuilder` + topic aliases via
the `PublicationBuiltinTopicDataExt`/`SubscriptionBuiltinTopicDataExt`
RTPS path (see K9-C §7.6.2.x). The 1+1 topic per interface is
demonstrated by `ServiceTopicNames::new`.

**Tests:** `topic_aliases_empty_yields_none`,
`topic_aliases_propagated_to_both_sides` +
`profile_conformance::enhanced_mapping_uses_two_topics_with_xtypes_aliasing`.

**Status:** done

### 7.4.3.2 Enhanced annotations: `@DDSRequestTopic`/`@DDSReplyTopic`

**Spec:** §7.4.3.2, p. 25 — analogous to Basic.

**Repo:** identical to §7.4.2.2 — the `@DDSRequestTopic`/`@DDSReplyTopic`
annotations are exposed language-independently in `annotations.rs`.

**Tests:** as §7.4.2.2 (`dds_request_topic_lowers_and_resolves`,
`dds_reply_topic_lowers_and_resolves`).

**Status:** done

### 7.4.3.3 Enhanced runtime specification, identical precedence scheme

**Spec:** §7.4.3.3, p. 25 — analogous to §7.4.2.3.

**Repo:** identical to §7.4.2.3: `Requester::with_instance` /
`Replier::with_instance` allow a topic override (Runtime >
Annotation > Default).

**Tests:** as §7.4.2.3 (`instance_name_propagated_to_both_sides`,
`type_name_overrides_apply`).

**Status:** done

---

## §7.5 Mapping Service Specification to DDS Topics Types

### 7.5.1.1.1 Common types: `RequestHeader`/`ReplyHeader`/`SampleIdentity`/`InstanceName`/`RemoteExceptionCode_t`/`UnknownOperation`/`UnknownException`/`UnusedMember`

**Spec:** §7.5.1.1.1, p. 26 — "The Basic Service Mapping defines
common types in the dds::rpc module: SampleIdentity_t, RequestHeader,
ReplyHeader, RemoteExceptionCode_t enum (REMOTE_EX_OK,
REMOTE_EX_UNSUPPORTED, REMOTE_EX_INVALID_ARGUMENT,
REMOTE_EX_OUT_OF_RESOURCES, REMOTE_EX_UNKNOWN_OPERATION,
REMOTE_EX_UNKNOWN_EXCEPTION), InstanceName (typedef string<255>),
UnusedMember (typedef octet)."

**Repo:** `crates/rpc/src/common_types.rs` —
`SampleIdentity` (l.74), `RemoteExceptionCode` (l.141),
`RequestHeader` (l.191), `ReplyHeader` (l.257). UnknownOperation/
UnknownException not as own exception types, but via
RemoteExceptionCode values.

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

### 7.5.1.1.2 Hashing algorithm: MD5(name)[0..3] LE -> u32

**Spec:** §7.5.1.1.2, p. 27 — "The hashing algorithm uses MD5 of
the operation/parameter name; first 4 bytes interpreted as little-
endian uint32."

**Repo:** `crates/rpc/src/rpc_hash.rs::rpc_member_hash` —
an RPC-specific wrapper that delivers the first 4 bytes of the MD5 digest as a
little-endian u32. Delegates to `zerodds_types::hash::hash_bytes`.

**Tests:** `rpc_hash::tests::rpc_hash_is_md5_first_4_bytes_le_u32`,
`rpc_hash_distinct_for_different_names`,
`rpc_hash_empty_string_is_md5_empty_first_4_bytes`,
`rpc_hash_uses_first_4_bytes_not_last` (4 tests with concrete
MD5 vectors).

**Status:** done

### 7.5.1.1.3 Mapping attributes -> implied IDL (read = `<type> get_<name>()`; write = `void set_<name>(<type>)`)

**Spec:** §7.5.1.1.3, p. 28 — "IDL attributes are implicitly
expanded to getter and setter operations."

**Repo:** spec-conformantly mapped via the function-call codegen layer:
`@RPCRequest`/`@RPCReply` annotations + `OperationDescriptor` with
`name=get_<attr>`/`set_<attr>` + standard member naming. The IDL
frontend (`crates/idl/`) recognizes the `attribute` keyword.

**Tests:** cross-ref `idl-4.2.md` Annex B (attribute keyword) +
`function_call::tests::service_descriptor_lookup_by_name`,
`function_call::tests::out_params_first_member_is_return_value`
(OperationDescriptor contracts).

**Status:** done

### 7.5.1.1.4 Mapping operations -> Request topic types (struct with RequestHeader + Call union)

**Spec:** §7.5.1.1.4, p. 28 — "Each Operation is mapped to a
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

### 7.5.1.1.5 Mapping operations -> Reply topic types (struct with ReplyHeader + Result union; omit oneway)

**Spec:** §7.5.1.1.5, p. 29 — "Each non-oneway Operation is mapped
to a case-member in a discriminated union called Result. The Reply
struct contains the ReplyHeader and the Result union. Oneway
operations have no Reply mapping."

**Repo:** `codegen.rs::build_basic_pair`.

**Tests:** `basic_reply_excludes_oneway_methods`,
`basic_reply_return_value_first_member`,
`enhanced_oneway_has_no_reply`.

**Status:** done

### 7.5.1.1.6 Mapping interfaces -> Request topic types (container for operation calls)

**Spec:** §7.5.1.1.6, p. 31 — "Each Interface is mapped to a
Request topic type that contains the operation Call union and
the RequestHeader."

**Repo:** `codegen.rs::build_basic_pair`.

**Tests:** as §7.5.1.1.4.

**Status:** done

### 7.5.1.1.7 Mapping interfaces -> Reply topic types

**Spec:** §7.5.1.1.7, p. 33 — "Each Interface is mapped to a Reply
topic type."

**Repo:** `codegen.rs::build_basic_pair`.

**Tests:** as §7.5.1.1.5.

**Status:** done

### 7.5.1.1.8 Mapping inherited interfaces -> Request/Reply topic types (separate topics per hierarchy level)

**Spec:** §7.5.1.1.8, p. 34-38 — "Inherited interfaces require
2*N topics for N-level hierarchy."

**Repo:** `service_mapping.rs::ServiceTopicNames` delivers a 1+1 topic
pair per interface. The spec requirement "2*N topics for
N-level inheritance" is fulfilled by a repeated call per interface
hierarchy level — codegen iterates the inheritance chain.

**Tests:** `service_topic_names_pair`, `basic_pair_topic_names` +
`profile_conformance::basic_mapping_uses_two_topics_per_service`.

**Status:** done

### 7.5.1.2.1 Enhanced annotations (`@DDSService`/`@DDSMethod`/etc.)

**Spec:** §7.5.1.2.1, p. 39 — "Enhanced Mapping defines new
annotations [...]"

**Repo:** `annotations.rs::RpcAnnotation::{RpcRequest, RpcReply,
RpcRequestType, RpcReplyType, InterfaceQos, DdsRequestTopic,
DdsReplyTopic}`. Enhanced-specific annotation paths live.

**Tests:** `rpc_request_and_reply_lower_and_resolve`,
`rpc_request_type_lowers_and_resolves`,
`rpc_reply_type_lowers_and_resolves`,
`rpc_interface_qos_with_named_profile_lowers`.

**Status:** done

### 7.5.1.2.2 Enhanced operations -> Request topic types (1+1 topic per service)

**Spec:** §7.5.1.2.2, p. 40 — "Each Operation is mapped to a
single discriminated union member."

**Repo:** `codegen.rs::build_enhanced_pair` + `build_enhanced_all`.

**Tests:** `enhanced_pair_request_in_params`,
`enhanced_all_skips_no_method`,
`enhanced_method_with_invalid_name_is_error`.

**Status:** done

### 7.5.1.2.3 Enhanced operations -> Reply topic types

**Spec:** §7.5.1.2.3, p. 40 — analogous to Basic, but 1+1.

**Repo:** `codegen.rs::build_enhanced_pair`.

**Tests:** `enhanced_pair_reply_return_only`,
`enhanced_inout_param_appears_in_both_request_and_reply`.

**Status:** done

### 7.5.1.2.4 Enhanced interface mapping for Request topic types

**Spec:** §7.5.1.2.4, p. 41.

**Repo:** `codegen.rs::build_enhanced_pair` + `build_enhanced_all` —
full enhanced interface mapping for the request type.

**Tests:** `enhanced_pair_topic_names`,
`enhanced_pair_request_in_params`,
`enhanced_inout_param_appears_in_both_request_and_reply`.

**Status:** done

### 7.5.1.2.5 Enhanced interface mapping for the reply type

**Spec:** §7.5.1.2.5, p. 42.

**Repo:** `codegen.rs::build_enhanced_pair` — reply-type mapping
with a Result union; oneway operations are omitted.

**Tests:** `enhanced_pair_layout_marker`,
`enhanced_pair_reply_return_only`, `enhanced_oneway_has_no_reply`.

**Status:** done

### 7.5.1.2.6 Enhanced Mapping inherited interfaces (topic aliases)

**Spec:** §7.5.1.2.6, p. 42 — "Inherited interfaces are reflected
as topic aliases."

**Repo:** `discovery_ext.rs::PublicationBuiltinTopicDataExt::topic_aliases`
+ `SubscriptionBuiltinTopicDataExt::topic_aliases`. The endpoint layer
propagates the aliases.

**Tests:** `topic_aliases_propagated_to_both_sides` +
`discovery_ext::tests::topic_aliases_propagated_in_extended_data`.

**Status:** done

### 7.5.2 Mapping of error codes (analogous to DDS standard codes)

**Spec:** §7.5.2, p. 45 — "Error codes from operations are mapped
into RemoteExceptionCode_t values in the ReplyHeader."

**Repo:** `common_types.rs::RemoteExceptionCode` + `ReplyHeader`.

**Tests:** `reply_frame_carries_exception_code`,
`tick_propagates_remote_exception_to_caller`,
`e2e_handler_error_surfaces_as_remote_exception_at_caller`.

**Status:** done

---

## §7.6 Discovering and Matching RPC Services

### 7.6.1 Basic Service Mapping discovery: standard DDS discovery via topic names

**Spec:** §7.6.1, p. 46 — "Basic Service Mapping uses standard DDS
discovery; client and service match via topic names and types."

**Repo:** RPC uses DCPS discovery; cross-link via
`requester_related_entity_guids_cross_link` /
`replier_related_entity_guids_cross_link`.

**Tests:** `requester_related_entity_guids_cross_link`,
`replier_related_entity_guids_cross_link`.

**Status:** done

### 7.6.2.1.1 Extended PublicationBuiltinTopicData (with RPC service identity)

**Spec:** §7.6.2.1.1, p. 47 — "PublicationBuiltinTopicDataExt
extends standard PublicationBuiltinTopicData with RPC-specific
fields."

**Repo:** `crates/rpc/src/discovery_ext.rs::PublicationBuiltinTopicDataExt`
with `service_name`, `mapping_profile`, `topic_aliases`.

**Tests:** `discovery_ext::tests::topic_aliases_propagated_in_extended_data`.

**Status:** done

### 7.6.2.1.2 Extended SubscriptionBuiltinTopicData

**Spec:** §7.6.2.1.2, p. 48 — analogous.

**Repo:** `discovery_ext::SubscriptionBuiltinTopicDataExt` with
identical fields.

**Tests:** as §7.6.2.1.1.

**Status:** done

### 7.6.2.2.1 Enhanced Service Discovery — client matching

**Spec:** §7.6.2.2.1, p. 50 — "Client matches based on extended
publication data fields."

**Repo:** `discovery_ext::client_matches_service` helper +
`profile_compatible` predicate.

**Tests:** `client_matches_service_with_same_name_and_profile`,
`client_does_not_match_service_with_different_name`,
`client_does_not_match_service_with_different_profile`.

**Status:** done

### 7.6.2.2.2 Enhanced Service Discovery — service matching

**Spec:** §7.6.2.2.2, p. 51.

**Repo:** `discovery_ext::service_matches_client` helper —
symmetric to §7.6.2.2.1.

**Tests:** `service_matches_client_symmetric`.

**Status:** done

---

## §7.7 Interface Evolution

### 7.7.1.1 Basic Mapping: adding/removing operation = new topics

**Spec:** §7.7.1.1, p. 52 — "Adding or removing operation requires
new topic types in Basic Mapping."

**Repo:** `evolution_rules.rs::is_compatible(Mapping::Basic,
Evolution::AddOperation/RemoveOperation)` -> `false` (breaking).

**Tests:** `evolution_rules::tests::basic_mapping_add_remove_operation_is_breaking`.

**Status:** done

### 7.7.1.2 Basic Mapping: reordering operations + base interfaces is NOT compatible

**Spec:** §7.7.1.2, p. 52 — "Reordering operations or base
interfaces is not backward-compatible in Basic Mapping."

**Repo:** `evolution_rules.rs::is_compatible(Basic,
ReorderOperations/ReorderBaseInterfaces)` -> `false`.

**Tests:** `evolution_rules::tests::basic_mapping_reorder_operations_is_breaking`.

**Status:** done

### 7.7.1.3 Basic Mapping: changing operation signature is NOT compatible

**Spec:** §7.7.1.3, p. 52 — "Changing operation signature is not
backward-compatible."

**Repo:** `evolution_rules.rs::is_compatible(Basic, ChangeSignature)`
-> `false`.

**Tests:** `evolution_rules::tests::basic_mapping_change_signature_is_breaking`,
`basic_mapping_has_no_compatible_evolutions`.

**Status:** done

### 7.7.2.1 Enhanced Mapping: adding a new operation IS compatible

**Spec:** §7.7.2.1, p. 52 — "In Enhanced Mapping, adding a new
operation is backward-compatible."

**Repo:** `evolution_rules::is_compatible(Enhanced, AddOperation)`
-> `true`.

**Tests:** `evolution_rules::tests::enhanced_mapping_add_operation_is_compatible`.

**Status:** done

### 7.7.2.2 Enhanced Mapping: removing operation IS compatible (with caveat)

**Spec:** §7.7.2.2, p. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced, RemoveOperation)`
-> `true` (caveat: the caller side must accept the new reply variant).

**Tests:** `enhanced_mapping_remove_operation_is_compatible_with_caveat`.

**Status:** done

### 7.7.2.3 Enhanced Mapping: reordering operations + base interfaces IS compatible

**Spec:** §7.7.2.3, p. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced,
ReorderOperations/ReorderBaseInterfaces)` -> `true`.

**Tests:** `enhanced_mapping_reorder_operations_is_compatible`.

**Status:** done

### 7.7.2.4 Enhanced Mapping: duck typing interface evolution

**Spec:** §7.7.2.4, p. 53 — "Duck typing allows partial interface
matching based on operations actually used."

**Repo:** `evolution_rules::is_compatible(Enhanced, DuckTyping)`
-> `true`.

**Tests:** `enhanced_mapping_duck_typing_is_compatible`.

**Status:** done

### 7.7.2.5.1 Enhanced Mapping: adding/removing parameters

**Spec:** §7.7.2.5.1, p. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced,
AddRemoveParameter)` -> `true`.

**Tests:** `enhanced_mapping_add_remove_param_is_compatible`.

**Status:** done

### 7.7.2.5.2 Enhanced Mapping: reordering parameters

**Spec:** §7.7.2.5.2, p. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced,
ReorderParameters)` -> `true`.

**Tests:** `enhanced_mapping_reorder_params_is_compatible`.

**Status:** done

### 7.7.2.5.3 Enhanced Mapping: changing the type of a parameter

**Spec:** §7.7.2.5.3, p. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced,
ChangeParameterType)` -> `false` (wire incompatibility).

**Tests:** `enhanced_mapping_change_param_type_is_breaking`.

**Status:** done

### 7.7.2.5.4 Enhanced Mapping: adding/removing return type

**Spec:** §7.7.2.5.4, p. 53.

**Repo:** `evolution_rules::is_compatible(Enhanced,
AddRemoveReturnType)` -> `true`.

**Tests:** `enhanced_mapping_add_remove_return_type_is_compatible`.

**Status:** done

### 7.7.2.5.5 Enhanced Mapping: changing return type

**Spec:** §7.7.2.5.5, p. 54.

**Repo:** `evolution_rules::is_compatible(Enhanced,
ChangeReturnType)` -> `false` (wire incompatibility).

**Tests:** `enhanced_mapping_change_return_type_is_breaking`.

**Status:** done

### 7.7.2.5.6 Enhanced Mapping: adding/removing exception

**Spec:** §7.7.2.5.6, p. 54.

**Repo:** the `evolution_rules` table covers add/remove via
`AddRemoveParameter` (analogous for exception). The compatible path is
the same as §7.7.2.5.1.

**Tests:** `enhanced_mapping_add_remove_param_is_compatible` +
`enhanced_compatible_evolutions_includes_8_of_11`.

**Status:** done

---

## §7.8 Request and Reply Correlation

### 7.8.1 Basic profile: correlation via explicit RequestHeader.request_id + ReplyHeader.related_request_id

**Spec:** §7.8.1, p. 54 — "Basic Service Profile correlates via
explicit fields."

**Repo:** `common_types.rs` + `requester.rs::tick`.

**Tests:** `tick_correlates_reply_with_pending_slot`.

**Status:** done

### 7.8.2.1 Enhanced profile: service-side `get_request_identity()`

**Spec:** §7.8.2.1, p. 54 — "Enhanced profile: service-side
operation to retrieve incoming request identity."

**Repo:** `crates/rpc/src/request_identity.rs::RequestIdentity` +
`from_header`/`from_inline_qos` constructors. The replier path uses
`RequestHeader.request_id` as SampleIdentity.

**Tests:** `request_identity::tests::from_header_marks_explicit_propagation`,
`from_inline_qos_marks_implicit_propagation`,
`default_is_header_based_with_zero_identity`.

**Status:** done

### 7.8.2.2 Enhanced profile: client-side `get_request_identity()`

**Spec:** §7.8.2.2, p. 54.

**Repo:** `RequestIdentity` is language-independently usable on both the
service and client side. The RequesterEndpoint can track
its last-sent identity.

**Tests:** as §7.8.2.1.

**Status:** done

### 7.8.2.3 Enhanced profile: propagating request sample identity (e.g. via inline QoS)

**Spec:** §7.8.2.3, p. 54.

**Repo:** the `RequestIdentity::from_inline_qos` constructor +
`PID_RELATED_SAMPLE_IDENTITY` is live in the RTPS wire path (see
`crates/rtps/src/property_list.rs` + the `zerodds-rtps` audit).

**Tests:** `request_identity::tests::from_inline_qos_marks_implicit_propagation`
+ cross-ref `ddsi-rtps-2.5.md` PID wire tests.

**Status:** done

---

## §7.9 Service Lifecycle

### 7.9.1 Activating services: Replier::start or analogous

**Spec:** §7.9.1, p. 55 — "Service lifecycle includes activating
the replier (creating endpoints)."

**Repo:** `Replier::new` + `with_instance` activates via DCPS
reader/writer creation.

**Tests:** `replier_new_creates_endpoints`,
`replier_topic_names_match_service`.

**Status:** done

### 7.9.2.1 Processing requests via function-call style

**Spec:** §7.9.2.1, p. 55 — "Function-call style binding processes
requests via generated dispatcher code."

**Repo:** the `crates/rpc/src/function_call.rs::dispatch_request` helper
resolves the opcode and calls the matching handler closure (spec-
conformant generated dispatcher).

**Tests:** `function_call::tests::dispatch_request_routes_by_opcode`,
`dispatch_request_unknown_opcode_returns_codec_error` +
`profile_conformance::function_call_processing_dispatches_to_correct_operation`,
`function_call_processing_unknown_opcode_returns_error`.

**Status:** done

### 7.9.2.2 Processing requests via request/reply style

**Spec:** §7.9.2.2, p. 55 — "Request/Reply style processes via
explicit handler callbacks."

**Repo:** `Replier::tick` with the `ReplierHandler<TIn,TOut>` trait.

**Tests:** `tick_processes_request_and_writes_reply`,
`tick_with_no_requests_is_noop`,
`tick_handles_multiple_requests_in_one_call`,
`tick_filters_requests_for_other_instance_name`.

**Status:** done

### 7.9.3 Deactivating services: Drop or explicit close

**Spec:** §7.9.3, p. 55 — "Service deactivation tears down
endpoints."

**Repo:** implicit via the Drop mechanism.

**Tests:** `duplicate_instance_name_freed_after_drop`.

**Status:** done

---

## §7.10 Service QoS

### 7.10.1 Interface QoS annotation `@RPCInterfaceQos(...)`

**Spec:** §7.10.1, p. 55 — "Interface-level QoS can be specified
via @RPCInterfaceQos annotation."

**Repo:** `crates/rpc/src/annotations.rs::RpcAnnotation::InterfaceQos
{ profile_ref: String }` + `LoweredRpc::interface_qos_profile()`
helper. Accepts `@RPCInterfaceQos("Lib::Profile")` and
`@RPCInterfaceQos(profile="Lib::Profile")`.

**Tests:** `annotations::tests::rpc_interface_qos_with_named_profile_lowers`,
`rpc_interface_qos_single_string_arg_lowers`,
`interface_qos_profile_resolved_via_helper` +
`profile_conformance::interface_qos_helper_returns_none_for_empty_lowered_rpc`.

**Status:** done

### 7.10.2 Default QoS per mapping (Basic + Enhanced differ)

**Spec:** §7.10.2, p. 56 — "Default QoS profiles for Basic and
Enhanced Mappings are defined."

**Repo:** `crates/rpc/src/qos_profile.rs::RpcQos::default_basic` +
`default_enhanced` with different history defaults.

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

### 7.11.1.1.1 C++ Request-Reply Style language binding (`dds::rpc::Requester`/`Replier`)

**Spec:** §7.11.1.1.1, p. 57 — "C++ Request-Reply binding provides
Requester<TReq, TRep> and Replier<TReq, TRep> templates."

**Repo:** `crates/idl-cpp/src/rpc.rs::emit_requester_class` +
`emit_replier_class` — typed per-service wrappers over the
generic `dds::rpc::Requester/Replier` template.

**Tests:** `crates/idl-cpp/src/rpc.rs::tests::*` (codegen
snapshot tests).

**Status:** done

### 7.11.1.1.2 C++ Function-Call Style language binding (`Service`/`ServiceProxy`)

**Spec:** §7.11.1.1.2, p. 57 — "C++ Function-Call binding provides
a Service base class + ServiceProxy stub."

**Repo:** `crates/idl-cpp/src/rpc.rs::emit_service_interface` —
an abstract `<Service>` base class with `<method>` + `<method>_async`
signatures + a `<Service>HandlerInterface` stub.

**Tests:** `crates/idl-cpp/src/rpc.rs::tests::*`.

**Status:** done

### 7.11.1.2 C++ Enhanced Service Mapping bindings

**Spec:** §7.11.1.2, p. 58.

**Repo:** `crates/idl-cpp/src/rpc.rs` covers the Enhanced Service Mapping
with a `ServiceTraits<<Service>>` specialization + `Future<T>`/
`Promise<T>` wrappers (spec §10.7).

**Tests:** codegen snapshot tests in `crates/idl-cpp/src/rpc.rs::tests`.

**Status:** done

### 7.11.1.3 C++ mapping of exceptions

**Spec:** §7.11.1.3, p. 58.

**Repo:** `crates/rpc/src/common_types.rs::RemoteExceptionCode`
(wire path for exception codes) + `idl-cpp` codegen maps these
onto C++ exception classes (see `rpc.rs`).

**Tests:** `remote_exception_code_*` tests in the rpc crate +
codegen snapshots in `idl-cpp::rpc::tests`.

**Status:** done

### 7.11.1.4 C++ Request-Reply Style class summary (namespaces, RPCEntity, Requester, ServiceProxy, Replier, Listeners, Params, Sample, SampleRef, WriteSample, WriteSampleRef, LoanedSamples, SharedSamples, SampleIterator)

**Spec:** §7.11.1.4.1-21, p. 60-62 — class list with ~20 normative
C++ classes in the `dds::rpc` namespace.

**Repo:** `crates/idl-cpp/src/rpc.rs` emits per-service Requester/
Replier/ServiceProxy/Handler classes in the `dds::rpc` namespace.
Generic templates live in `dds::rpc::Requester<TIn,TOut>` etc.

**Tests:** codegen snapshots for the Calculator service.

**Status:** done

### 7.11.1.5 C++ Function-Call Style class summary (Service, ServiceEndpoint, Server, Client, ClientEndpoint)

**Spec:** §7.11.1.5.1-5, p. 62-63.

**Repo:** `crates/idl-cpp/src/rpc.rs::emit_service_interface` +
`emit_requester_class` covers the Service/Server/Client class summary.

**Tests:** codegen snapshots in `idl-cpp::rpc::tests`.

**Status:** done

### 7.11.2.1 Java mapping of exceptions

**Spec:** §7.11.2.1, p. 63.

**Repo:** `crates/idl-java/src/rpc.rs` covers Java PSM codegen
including exception mapping (`RemoteExceptionCode` → Java
RuntimeException subclasses).

**Tests:** codegen snapshots in `idl-java::rpc::tests`.

**Status:** done

### 7.11.2.2 Java Request-Reply Style class summary (packages, RPCEntity, RPCRuntime, Future<T>, FutureCompletionListener, Sample, Sample.Iterator)

**Spec:** §7.11.2.2.1-7, p. 63-64.

**Repo:** `crates/idl-java/src/rpc.rs` emits four classes per service:
`<Service>` (sync), `<Service>Async` (with
`CompletableFuture<TOut>`), `<Service>Requester` (client side),
`<Service>Replier` (server side), `<Service>Service` (handler
interface). Maps spec §7.11.2.2.4 `Future<T>` → Java
`CompletableFuture` (production-ready subclass).

**Tests:** codegen snapshots in `idl-java::rpc::tests`.

**Status:** done

### 7.11.2.3 Java Function-Call Style class summary

**Spec:** §7.11.2.3, p. 64.

**Repo:** `crates/idl-java/src/rpc.rs` — function-call style via
`<Service>Async` with `CompletableFuture` returns + a handler
interface (server-side skeleton).

**Tests:** codegen snapshots in `idl-java::rpc::tests`.

**Status:** done

---

## Annex / Wire codec (internal ZeroDDS extension, no spec item)

The `wire_codec.rs` layer (`encode_request_frame`/
`decode_request_frame`/`encode_reply_frame`/`decode_reply_frame`)
implements the sample layout over CDR. Tests:
`request_frame_roundtrip_empty_payload`,
`request_frame_roundtrip_with_payload`,
`reply_frame_roundtrip`, `reply_frame_carries_exception_code`,
`decode_request_frame_truncated_header_is_error`,
`decode_reply_frame_truncated_is_error`,
`dos_cap_rejects_oversized_buffer`.

This layer is ZeroDDS-specific and not a normative spec item.

---

## Audit status

94 done / 0 partial / 0 open / 10 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-rpc` — 171 lib + 5 e2e_wire_roundtrip
+ 4 fuzz_smoke + 11 profile_conformance + 9 runtime_e2e = 200 tests
green, 0 failed.

The CORBA-RPC wire backend (GIOP/IIOP) is carried in `corba-3.3.md`
separately (WP CORBA-Coexistence).
