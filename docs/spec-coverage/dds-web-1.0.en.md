# OMG Web-Enabled DDS 1.0 — Spec Coverage

**Spec:** [OMG Web-Enabled DDS 1.0 — formal/2014-12-01 →](https://www.omg.org/spec/DDS-WEB/)

**Context:** DDS-WEB defines a WebDDS object model + a REST PSM, with which
web clients (browsers, mobile apps, cloud services) participate in DDS
topics over HTTP. ZeroDDS implements the **WebDDS object model** (§7) + the
**REST-PSM URI routing + status mapping + header set + XML element-tag
registry** (§8.3) as a pure-Rust no_std+alloc library. The
HTTP server implementation + XML body serialization are caller-layer —
realized in the daemon crate `crates/webdds-bridge` (binary
`zerodds-webdds-bridged`), which serves REST (§8.3) and SOAP (§8.4) from
one process over `crates/dcps`'s public typed API.

Implementation:

- `crates/web/` — WebDDS object model + REST PSM (URI routing, status mapping, header set, XML element-tag registry), 10 modules, 81 tests green.
- `crates/webdds-bridge/` — the `zerodds-webdds-bridged` daemon: a
  `DdsBackend` impl against the typed `zerodds-dcps` API (Gateway), type
  resolution from a config-named IDL file or TypeLookup discovery
  (TypeRegistry), an XML<->DynamicData codec driven by the type's own
  descriptor (dynxml, never an opaque byte guess), REST + SOAP dispatch, a
  hand-rolled HTTP/1.1 server (Content-Length + chunked + keep-alive), WSDL
  generation from config-named IDL interfaces
  (`zerodds_lang_rules::adapter::interface`), business-interface dispatch
  over DDS-RPC 1.0's Basic Mapping (one topic pair per interface,
  §7.5.1.1). 119 tests green (110 unit + 9 e2e, two of them against a
  real, build-time-generated `Replier` on a second, independent
  `DomainParticipant` over real RTPS/UDP).

---

## §1 Scope

### §1.1-§1.6 Overview + conformance + examples

**Spec:** §1, p. 1-4 — a DDS gateway service over HTTP/REST. §1.1/§1.2/§1.6
are purpose, background and explicitly non-normative example scenarios; the
checkable claims sit in §1.3.1 (a session whose life spans multiple physical
connections; an access control model per principal), §1.4 (the object model
extends the DDS object model with an access control model and an application
management model) and §1.5 (two platform mappings: REST and SOAP).

**Repo:** `crates/web/src/lib.rs` (crate doc + re-exports),
`crates/web/src/session.rs::SessionId`,
`crates/web/src/access_control.rs::{Permissions, Rule, Operation, Decision}`,
`crates/web/src/rest.rs::parse_route`; the SOAP mapping of §1.5 lives in
`crates/zerodds-soap/src/wsdl.rs`.

**Tests:** three tests, one per checkable claim:
`crates/web/src/lib.rs::scope_tests::scope_session_identity_survives_reconstruction_across_connections`
(§1.3.1 reason 2 — the same token string yields the same `SessionId` even
when reconstructed; a different token is a different session, an empty one is
no session),
`crates/web/src/lib.rs::scope_tests::scope_access_control_gates_per_operation_and_per_topic`
(§1.3.1 reason 3 + §1.4 — the decision depends on the operation and the
topic; the right topic with the wrong operation is denied, an unlisted topic
is denied, an empty rule set denies rather than permits),
`crates/web/src/lib.rs::scope_tests::scope_rest_mapping_addresses_the_webdds_object_model`
(§1.5 — the REST mapping addresses Application and DomainParticipant under
the §8.3.1 prefix, a URI without that prefix is not part of the platform).

**Status:** done

---

## §2 Conformance

### §2 Profile triple (REST + SIMPLE-REST + SIMPLE-WSDL-SOAP)

**Spec:** §2, p. 5 — REST `Mandatory`, SIMPLE-REST + SIMPLE-WSDL-SOAP
`Optional`.

**Repo:** ZeroDDS implements REST (mandatory) plus SOAP.

**Tests:** `crates/web/src/rest.rs::tests::rest_prefix_constant_matches_spec`,
`crates/web/src/rest.rs::tests::table_5_every_pim_operation_has_a_route`,
`crates/web/src/headers.rs::tests::validate_required_passes_with_all_three_headers`,
`crates/web/src/status.rs::tests::ok_maps_to_201_for_post_create`;
SIMPLE-WSDL-SOAP: `crates/zerodds-soap/src/wsdl.rs::tests::wsdl_11_emits_definitions_root`,
`crates/zerodds-soap/src/envelope.rs::tests::parse_round_trip`.

**Status:** done — the mandatory profile (REST) and the optional
SIMPLE-WSDL-SOAP profile both implemented (see §8.4); the SIMPLE-REST
profile analogous to the REST subset.

---

## §3-§6 References + terminology

### §3 Normative references

**Spec:** §3, p. 5 — DDS 1.2, DDS-CCM, DDS-XTYPES, HTTP RFC 2616,
HTTP-Auth RFC 2617, WebSockets RFC 6455, SOAP 1.2, WSDL 1.1, XML 1.1.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — an external normative reference list;
HTTP/RFC/XML are fulfilled operationally in the respective consumer items
§7/§8.

### §4-§6 Terms + symbols + acks

**Spec:** §4-§6, p. 6-8 — terms and definitions, symbols,
acknowledgements.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary/symbols/acks; without a code
mapping.

---

## §7 WebDDS object model

### §7.1 WebDDS object model overview

**Spec:** §7.1, p. 9 — the WebDDS object model is a containment tree:
`WebDDS::Root` holds application, client, type and QoS instances, and an
application holds its DomainParticipants.

**Repo:** `crates/web/src/model.rs::{WebDdsRoot, Application,
DomainParticipant, Client}` — containment as ownership
(`WebDdsRoot::applications`, `Application::participants`,
`WebDdsRoot::clients`).

**Tests:** `crates/web/src/model.rs::tests::every_model_object_is_reachable_from_the_root`,
`crates/web/src/model.rs::tests::application_carries_participants`.

**Status:** done

### §7.2 Singleton WebDDS::Root

**Spec:** §7.2, p. 10 — `WebDDS::Root` is a singleton; all other classes are
reachable via Root.

**Repo:** `crates/web/src/model.rs::WebDdsRoot` as a singleton pattern
(instantiated via the application lifecycle).

**Tests:** `crates/web/src/model.rs::tests::root_starts_empty_and_is_the_only_container`,
`crates/web/src/model.rs::tests::root_holds_one_entry_per_application_name`,
`crates/web/src/model.rs::tests::every_model_object_is_reachable_from_the_root`.

**Status:** done

### §7.3 Access control + SessionId

**Spec:** §7.3, p. 11-13 — `AccessController` + session tracking.

**Repo:** `crates/web/src/session.rs::SessionId` +
`crates/web/src/model.rs::Client` + `crates/web/src/access_control.rs`
(permissions data model + an `evaluate(op, topic) -> Decision` engine, a
glob matcher with `*`/`prefix*`/`*suffix` patterns, the spec §7.3 decision
tree: first deny-match wins, otherwise first permit-match, otherwise
default).

**Tests:** `crates/web/src/session.rs::tests::{session_id_carries_token_string,
empty_session_id_is_invalid}`;
`crates/web/src/access_control.rs::tests::{empty_permissions_returns_default_deny,
allow_rule_grants_permit, deny_rule_overrides_allow_when_listed_first,
glob_prefix_pattern_matches, glob_suffix_pattern_matches,
operation_mismatch_falls_through_to_default,
first_matching_deny_wins_over_later_allow}` (7).

**Status:** done

### §7.3.1 Class WebDDS::Root

**Spec:** §7.3.1, p. 13-17 — Root operations: create_application,
delete_application, get_applications (with fnmatch),
create/delete/update/get_qos_library, create/delete/get_type.

**Repo:** `WebDdsRoot::{create_application, delete_application,
get_applications}` with an fnmatch wildcard subset.

**Tests:** `crates/web/src/model.rs::tests::{create_application_returns_session_id,
duplicate_application_yields_object_already_exists,
delete_application_removes_existing,
delete_unknown_application_yields_invalid_object,
get_applications_with_wildcard_matches_all,
get_applications_with_prefix_pattern_matches_subset,
get_applications_with_exact_match}`.

**Cross-ref schema storage (Phase-B cluster-8):** QosLibrary CRUD via
`crates/xml/src/qos.rs::QosLibrary`, `crates/xml/src/parser.rs::parse_xml_tree`; type CRUD via the
`crates/types/` TypeLibrary. Bridge to the DCPS API in
`crates/web/src/bridge.rs::DdsBackend` trait (see §7.4).

**Status:** done

### §7.4 Class operations Application + Participant + …

**Spec:** §7.4, p. 17-58 — DomainParticipant, Topic, Publisher, Subscriber,
DataWriter, DataReader, WaitSet — all CRUD.

**Repo:** data-model stubs in `crates/web/src/model.rs::{Application, DomainParticipant}`.

**Repo:** `crates/web/src/bridge.rs::DdsBackend` trait with all ~35 §7.4
class operations: create/update/delete/list for Application, Participant,
Topic, Publisher, Subscriber, DataWriter, DataReader, plus write_sample/
read_samples and create/update/wait/delete/list_waitset (§7.4.9's
long-poll `WaitSet::get`). QosLibrary/QosProfile/Type CRUD (§7.3.1 Root
metadata, not a DCPS entity) is deliberately not in the trait.
`enforce(decision) -> Result<(), BackendError>` composes orthogonally
with the AccessController engine from §7.3.

**Repo:** `crates/webdds-bridge/src/gateway.rs::Gateway` — the concrete
`DdsBackend` impl against `crates/dcps/`'s public typed API
(`DomainParticipant`/`Topic`/`Publisher`/`Subscriber`/`DataWriter`/
`DataReader`/`WaitSet`). Entity names the trait does not take from the
caller (participant, publisher, subscriber, writer, reader) are generated
and returned in the `Created` path. `PUT`/update on a DCPS entity calls
`set_qos` on the live `zerodds-dcps` entity (`zerodds_dcps::Entity`) —
`crate::qos_update` reads the PUT body's `<qos>` element and merges it
onto the entity's current QoS (not a fresh default: `zerodds_qos`'s own
policy defaults differ from `zerodds_dcps`'s entity defaults, and a
"replace" approach tripped `ImmutablePolicy` on a Changeable=NO policy
the caller never named — measured, see `qos_update`'s module doc).
`UpdateWaitset` replaces the condition set instead (§7.4.3.5).

**Repo:** a waitset's long-poll waits without the gateway's lock
(`Gateway::waitset_handle`, `WaitsetHandle::wait`); clients of the same
waitset take turns, because a DCPS `WaitSet` refuses a second concurrent
`wait` with PRECONDITION_NOT_MET. Measured on the Linux test host before: during a
4 s long-poll a list of applications took 3.7 s, now 0.4 ms. A `timeout`
that is not a finite, non-negative number of seconds and a `domain_id`
that is not a domain id are `InvalidInput` (422); before, the
connection's thread ended without a response on `-1`, `nan`, `1e300`
(`Duration::from_secs_f64`), `abc` silently waited 10 s, and an
unreadable `domain_id` gave domain 0. A missing `domain_id` is still
domain 0 (spec text on it not checked). Tests:
`crates/webdds-bridge/tests/daemon_e2e.rs::a_long_poll_leaves_other_requests_alone_and_a_bad_timeout_is_refused`,
`crates/webdds-bridge/src/gateway.rs::tests::{a_timeout_is_a_finite_non_negative_number_of_seconds,
a_domain_id_is_read_or_refused}`.

**Repo:** an inline `<qos>` is not the only source §7.4 bodies provide
for QoS — `<qos_profile>name</qos_profile>` names a stored
`QosLibrary`/`QosProfile` pair by reference. `crate::qos_update::
resolve_entity_qos`/`resolve_qos_profile_ref` resolve the reference
against `crate::metadata::MetadataStore` (`zerodds_xml::resolve_profile`)
and merge an additionally present inline `<qos>` over it (override
wins). §7.4.3.1/§7.4.3.2/§7.4.4.7/§7.4.4.8/§7.4.4.11/§7.4.4.12 also let
`create_participant`/`create_publisher`/`create_subscriber` and their
`update_*` counterparts carry nested `<publishers>`/`<subscribers>`/
`<data_writers>`/`<data_readers>` — `crate::contained_entities` parses
them, `crate::gateway::{sync_publishers, sync_subscribers,
sync_data_writers, sync_data_readers}` reconcile the live DCPS entities
against the parsed set by name (add, re-apply QoS, or remove). `update_*`
distinguishes "the element is absent" (leave untouched) from "the
element is empty" (delete every contained entity of that kind) —
literally what the spec's own sentence ("if the
…ObjectRepresentation specifies a set of contained entities") asks for.

**Tests:** `crates/webdds-bridge/src/qos_update.rs::tests` (incl.
`qos_profile_reference_resolves_a_stored_profile`,
`qos_profile_reference_to_an_unknown_profile_is_a_bad_request`,
`inline_qos_overrides_the_profiles_value_for_the_same_policy`);
`crates/webdds-bridge/src/contained_entities.rs::tests` (incl.
`nested_publishers_and_data_writers_parse`,
`a_publishers_qos_profile_reference_resolves`,
`a_contained_entity_without_a_name_is_a_bad_request`);
`crates/webdds-bridge/src/gateway.rs::tests::
update_participant_creates_and_reconciles_contained_entities` (real DCPS
entities, offline domain: create -> reconcile -> leave-untouched, full
cycle).

**Tests:** `crates/web/src/bridge.rs::tests::{enforce_permit_allows_call,
enforce_deny_returns_forbidden,
permission_check_then_backend_call_chains_correctly,
create_then_delete_application_round_trip,
create_application_twice_yields_conflict}` (5);
`crates/webdds-bridge/src/gateway.rs::tests` (3) +
`crates/webdds-bridge/tests/daemon_e2e.rs::write_over_http_arrives_at_an_independent_participant`
(an HTTP write through the daemon, verified by a second, independent
`DomainParticipant` over real RTPS/UDP).

**Status:** done

### §7.4.8 DataReader::read with a SampleSelector

**Spec:** §7.4.8, p. 51-53 — `read` with a `SampleSelector` BNF grammar
(FilterExpression + MetadataExpression with AND/OR logic).

**Repo:** `crates/web/src/sample_selector.rs::parse_sample_selector` returns
an AST (`SampleSelector` with `filter: Option<FilterExpression>` +
`metadata: Vec<MetadataExpression>`). A recursive-descent parser with all 6
comparison operators (=,!=,<,<=,>,>=), AND/OR logic, parentheses,
integer/string/boolean literals, a dotted field path,
sample_state/view_state/instance_state metadata with all values.

**Tests:** `crates/web/src/sample_selector.rs::tests::{parses_simple_equality_filter,
parses_inequality_with_string_literal, parses_dotted_field_path,
parses_and_conjunction, parses_or_with_parenthesis,
parses_metadata_only_expression, parses_filter_plus_metadata,
parses_all_three_metadata_kinds, comparison_operators_full_coverage,
boolean_literal_supported, rejects_unknown_metadata_key,
rejects_unknown_metadata_value, rejects_unbalanced_parenthesis,
rejects_trailing_garbage}` (14).

**Status:** done

---

## §8 REST PSM

### §8.1 Formats

**Spec:** §8.1, p. 59 — Content-Type `application/dds-web+xml`
(mandatory) as the XML format; further formats optional. A full-text
search of the PDF (§8.2, §8.3.5 Tab 7/8) finds no second
representation — DDS-WEB 1.0 defines no JSON.

**Repo:** `crates/web/src/representation.rs::{ContentType,
CONTENT_TYPE_DDS_WEB_XML}`. The constant's own value was, as of before
this round, wrong codebase-wide (`application/zerodds-web+xml`, six
files) — a measured bug, not a missing representation.

**Tests:** `crates/web/src/representation.rs::tests::content_type_constant`.

**Status:** done

### §8.2 Representations

**Spec:** §8.2, p. 60-61 — four representation kinds: QoS, Type, Data,
Entity (with XML element tags per kind).

**Repo:** `crates/web/src/representation.rs::{RepresentationKind,
element_name}`.

**Tests:** `crates/web/src/representation.rs::tests::element_names_match_spec_tab_6`,
`crates/web/src/representation.rs::tests::types_representation_uses_dds_xtypes_root`.

**Status:** done — element-tag registry. XML body serialization is
caller-layer.

### §8.3.1 Mapping to resource URIs

**Spec:** §8.3.1, p. 62 Tab 4 — all URI patterns with prefix `/dds/rest1`.

**Repo:** `crates/web/src/rest.rs::{REST_PREFIX, RestRoute, parse_route,
RestMethod}` with all 30+ routes.

**Tests:** `crates/web/src/rest.rs::tests::{rest_prefix_constant_matches_spec,
missing_prefix_yields_error, create_application_route, get_applications_route,
delete_application_route_extracts_app_name,
create_participant_route_extracts_app_name,
delete_participant_route_extracts_app_and_participant,
data_writer_write_route_extracts_4_segments,
data_reader_read_route_extracts_4_segments,
qos_profile_routes_extract_lib_and_profile_name,
waitset_routes_extract_app_and_waitset,
topic_publisher_subscriber_create_routes, unknown_paths_yield_unknown_variant,
types_routes_match_root_pattern}` (14).

**Status:** done

### §8.3.2 Mapping to HTTP methods + status codes

**Spec:** §8.3.2, p. 62-64 — method mapping (POST/PUT/GET/DELETE) + status
codes (201, 204, 200, 409, 422, 404, 401, 403, 500).

**Repo:** `crates/web/src/status.rs::{ReturnStatus, http_status_for}`.

**Tests:** `crates/web/src/status.rs::tests::{ok_maps_to_201_for_post_create,
ok_maps_to_204_for_delete, ok_maps_to_200_for_get,
ok_maps_to_204_for_put_update, object_already_exists_maps_to_409,
invalid_input_maps_to_422, invalid_object_maps_to_404,
access_denied_maps_to_401, permissions_error_maps_to_403,
generic_service_error_maps_to_500, dds_error_maps_to_500}` (11).

**Status:** done

### §8.3.3 Complete mapping (Tab 5)

**Spec:** §8.3.3, p. 64-67 — Table 5, "Complete mapping of WebDDS PIM
operations to REST methods": 48 rows, an HTTP method and a URI per
operation. Plus the sentence above the table: "the HEAD HTTP method shall be
supported on the same URIs as the GET method. The HEAD method shall behave
identically to the GET method except for it shall return no body."

**Repo:** `crates/web/src/rest.rs::RestRoute` (47 route variants plus
`Unknown`), `crates/web/src/rest.rs::parse_route`,
`crates/web/src/rest.rs::canonical_segment`.

**Tests:** the table is transcribed row by row into `TABLE_5` (operation,
method, URI, taken from the PDF) and read by five tests:
`crates/web/src/rest.rs::tests::table_5_every_pim_operation_has_a_route`
(no row falls through to `RestRoute::Unknown`),
`crates/web/src/rest.rs::tests::table_5_rows_do_not_collide` (48 rows yield
48 distinct routes — two operations landing on one variant would make the
mapping ambiguous),
`crates/web/src/rest.rs::tests::table_5_alternate_segment_spellings_route_identically`
(Table 5 spells the same collection two ways — `domain_participants` against
`participants`, `datawriters`/`datareaders` against
`data_writers`/`data_readers`; both spellings reach the same route),
`crates/web/src/rest.rs::tests::canonicalisation_is_positional_not_global`
(the normalisation applies only in the collection slots, so an application
actually named `participants` stays a name),
`crates/web/src/rest.rs::tests::head_behaves_like_get_on_the_same_uris`
(every GET row of the table routes identically under HEAD).

**Finding from writing the test:** the table was not fully mapped when this
test was added. 23 of the 48 rows fell through to `RestRoute::Unknown` — the
whole update/delete/get level for waitsets, registered types, topics,
publishers, subscribers, datawriters and datareaders, plus
`Publisher::create_datawriter` and `Subscriber::create_datareader`. On top of
that `RestMethod::Head` existed as an enum variant but was handled nowhere in
`parse_route`, so every GET URI fell through to `Unknown` under HEAD. Both
are built as of this state: the 23 missing routes are implemented, HEAD
routes like GET, and the second spelling of the collection segments is
accepted.

**Status:** done

### §8.3.4 Object representations (Tab 6)

**Spec:** §8.3.4, p. 68-69 Tab 6 — the XML element-tag registry.

**Repo:** `crates/web/src/representation.rs::element_name`.

**Tests:** `crates/web/src/representation.rs::tests::element_names_match_spec_tab_6`.

**Status:** done

### §8.3.5 HTTP headers (Tab 7+8)

**Spec:** §8.3.5, p. 70 — required request/response headers (Accept,
Content-Length, Content-Type, Cache-Control, OMG-DDS-API-Key,
Authentication-Info, Date, Expires, Location, Last-Modified).

**Repo:** `crates/web/src/headers.rs::{RequestHeaders, ResponseHeaders,
REQUEST_API_KEY, validate_required}`.

**Repo (daemon):** the library types alone were not enough — the daemon
called `validate_required` nowhere, as of before this round, so a
request with no `Accept` (or one naming none of these representations)
was still served. `crates/webdds-bridge/src/rest_service.rs::
check_representation_headers` now checks before every REST operation: a
missing `Accept` -> 400, an `Accept` that names neither
`application/dds-web+xml` nor a matching `*/*`/`application/*` -> 406, a
`Content-Type` naming anything else -> 415.

**Tests:** `crates/web/src/headers.rs::tests::{request_api_key_constant_matches_spec,
validate_required_passes_with_all_three_headers, missing_api_key_rejected,
missing_accept_rejected, missing_cache_control_rejected,
response_headers_default_is_empty}` (6);
`crates/webdds-bridge/src/rest_service.rs::tests::{missing_accept_header_is_400,
accept_header_naming_an_unserved_representation_is_406,
accept_header_wildcards_are_acceptable,
content_type_naming_an_unserved_representation_is_415}` (4).

**Status:** done

---

## §8.4 SOAP PSM

### §8.4 Simplified SOAP platform

**Spec:** §8.4, p. 71+ — optional SIMPLE-WSDL-SOAP.

**Repo:** `crates/zerodds-soap/src/{envelope,addressing,fault,mtom,
security,wsdl}.rs` (SOAP envelope codec, WS-Addressing, MTOM, WS-Security
hooks, WSDL emitter; 1218 SLOC).

**Repo (daemon):** `crates/webdds-bridge/src/soap_service.rs` dispatches
the SOAP envelope's body (a DDS-WEB object-model operation name such as
`CreateApplication`/`WriteSample` — the same operation set REST exposes)
against the same `Gateway`: REST and SOAP are sibling bindings of one
object model, not a second protocol. `crates/webdds-bridge/src/wsdl_bridge.rs`
generates the WSDL both for this fixed operation set and for every
config-named IDL `interface` (via `zerodds_lang_rules::adapter::interface`
— the IDL-to-operation bridge plan §3 asked for).

A business interface's operation is WSDL-described *and* dispatchable:
`crate::rpc_dispatch` builds, per configured `interface`, DDS-RPC 1.0
§7.5.1.1's Basic Mapping types dynamically — **one** request/reply topic
pair per interface (`service_name` = the interface's qualified name,
e.g. `demo_Calculator`), one `Call`/`Return` union over every operation
(discriminator = `operation_hash`), one `Result` union per non-`oneway`
operation (`dds::RETCODE_OK` -> the `Out` struct, one case per `raises`
exception, discriminator = `exception_hash`). Every synthesized wire
type is `@final` (classic CDR, no `DHEADER`) — the same shape
`zerodds-idlc generate --lang rust`/`--lang cpp` themselves emit for an
`interface` (`crates/lang-rules/src/emit/interface_rust.rs`), and the
same `operation_hash`/`exception_hash`/struct-naming functions from
`zerodds_lang_rules::adapter::interface` those generators call
themselves — not a parallel derivation that happens to compute the same
value. An operation naming no configured interface (and no DDS-WEB
object-model operation either) still returns a named `env:Receiver`
fault instead of a silent no-op; a declared `raises` exception does too,
named with its fields (`OperationException`), not an empty or wrong
success.

**Interop evidence:** `crates/webdds-bridge/tests/soap_rpc_e2e.rs` calls
not a hand-written stand-in but a **real** `Replier`, generated at build
time with `zerodds_lang_rules::generate::source(.., Language::Rust, ..)`
(`zerodds-rpc-rust-e2e`'s `Calculator` binding — `module demo { exception
DivByZero; @DDSService interface Calculator { … } }`, a return value,
`raises`, `oneway`, an `attribute`, `inout` — every DDS-RPC 1.0 §7.5.1.1
construct in one interface), on a second, independent
`DomainParticipant`, over real RTPS/UDP.

**Tests:** `crates/zerodds-soap/src/envelope.rs::tests::{build_simple_body_only,
build_with_header, parse_round_trip, parse_missing_envelope_rejected,
parse_missing_body_rejected, build_includes_namespace_declaration}`;
`crates/zerodds-soap/src/addressing.rs::tests::{to_and_action_are_emitted,
message_id_and_relates_to_round_trip,
reply_to_and_fault_to_emit_address_subelement, ns_constant_is_w3c_2005_08}`;
`crates/zerodds-soap/src/fault.rs::tests::{fault_codes_have_correct_qnames,
fault_xml_contains_required_elements, fault_with_detail_includes_detail_block,
fault_reason_is_xml_escaped}`;
`crates/zerodds-soap/src/wsdl.rs::tests::{wsdl_11_emits_definitions_root,
wsdl_20_emits_description_root, wsdl_11_includes_all_operation_messages,
wsdl_11_binding_uses_soap_http_transport, wsdl_20_binding_targets_soap_protocol,
endpoint_address_is_included}`;
`crates/zerodds-soap/src/security.rs::tests::{username_token_text_password_emits_correct_type,
username_token_digest_uses_digest_uri, timestamp_emits_created_and_expires,
x509_token_default_uses_v3_value_type, password_type_uris_match_spec}`;
`crates/zerodds-soap/src/mtom.rs::tests::{package_starts_with_xop_root_part,
package_includes_attachment_part, package_terminates_with_closing_boundary,
xop_include_namespace_is_correct}`;
`crates/webdds-bridge/src/rpc_dispatch.rs::tests::{
finds_a_declared_operation_across_flattened_services,
an_operation_with_no_params_gets_a_dummy_member,
every_synthetic_type_is_final_not_appendable,
builds_one_interface_wide_call_and_return_union}`;
`crates/webdds-bridge/tests/soap_rpc_e2e.rs::{
soap_add_reaches_a_generated_replier_and_returns_its_reply,
soap_div_by_zero_surfaces_the_declared_exception,
soap_call_to_an_unconfigured_operation_is_a_named_fault}` — the first
two against the real, generated `Replier`;
`crates/webdds-bridge/tests/daemon_e2e.rs::soap_write_sample_arrives_at_an_independent_participant`
(a `WriteSample` whose `<data>` holds the sample as elements in the REST
representation arrives at a second, independent `DomainParticipant` over
RTPS/UDP; measured on the Linux test host before: `env:Receiver` "malformed XML
body: no root element", because the field reader took the children of
`<data>` for a list),
`crates/webdds-bridge/src/xmlfields.rs::tests::a_child_subtree_comes_back_as_xml`.

**Status:** done — the SOAP PSM as a differentiation feature for the
Java-enterprise estate, fully covered.

---

## Audit status

16 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-web` — 81 tests green, 0 failed.
  Modules: `access_control`, `bridge`, `headers`, `model`,
  `representation`, `rest`, `sample_selector`, `session`, `status`, plus
  `scope_tests` (§1) and the Table-5 tests in `rest` (§8.3.3).
* `cargo test -p zerodds-soap --lib` — 37 tests green. Modules:
  `addressing`, `envelope`, `fault`, `mtom`, `security`, `wsdl`.
* `cargo test -p zerodds-webdds-bridge` — 110 unit + 9 e2e tests green
  (the daemon that combines the two libraries and actually serves
  them; see `crates/webdds-bridge/README.md`).

`zerodds-web` covers §7-§8.3, `zerodds-soap` covers §8.4, both as
bridge-agnostic libraries. `zerodds-webdds-bridge` is the caller-layer
realization (HTTP server, `DdsBackend` impl, XML body codec) — the
`zerodds-webdds-bridged` daemon. DDS-XTYPES XML type files as a type
source, business-interface SOAP dispatch (a real DDS-RPC request/reply
over `zerodds-rpc`'s `Requester`/`Replier`, `crate::rpc_dispatch`), an
automatic TypeLookup poll, and an admin `/catalog`+`/healthz` beside
`/metrics` have since been built. This round closes the three
previously documented open points, measured (not merely claimed):

- DDS-RPC topic granularity is now one topic pair per interface
  (§7.5.1.1), checked against a real `zerodds-idlc`-generated `Replier`
  (`tests/soap_rpc_e2e.rs`), no longer one pair per operation.
- `update_*`'s `<qos_profile>` reference resolves against the
  `QosLibrary`/`QosProfile` metadata, and `create_*`/`update_*` of
  Participant/Publisher/Subscriber read and reconcile contained
  entities (§7.4.3.1/§7.4.3.2/§7.4.4.7/§7.4.4.8/§7.4.4.11/§7.4.4.12).
- Content negotiation is enforced: DDS-WEB 1.0 defines no second
  representation beside `application/dds-web+xml` (full-text checked,
  §8.2/§8.3.5) — what was open was not a missing format but a
  misspelled constant value (six files) and a library function the
  daemon never called; both are now measurably closed.

See `crates/webdds-bridge/README.md` "Open points, closed" for the
evidence behind each one.
