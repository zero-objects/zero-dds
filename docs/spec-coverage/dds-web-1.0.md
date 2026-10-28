# OMG Web-Enabled DDS 1.0 — Spec-Coverage

**Spec:** [OMG Web-Enabled DDS 1.0 — formal/2014-12-01 →](https://www.omg.org/spec/DDS-WEB/)

**Kontext:** DDS-WEB definiert ein WebDDS-Object-Model + REST-PSM, mit
dem Web-Clients (Browser, Mobile-Apps, Cloud-Services) über HTTP an
DDS-Topics teilnehmen. ZeroDDS implementiert das **WebDDS-Object-
Model** (§7) + **REST-PSM URI-Routing + Status-Mapping + Header-Set +
XML-Element-Tag-Registry** (§8.3) als pure-Rust no_std+alloc Library.
Die HTTP-Server-Implementation + XML-Body-Serialization sind
Caller-Layer — realisiert im Daemon-Crate `crates/webdds-bridge`
(Binary `zerodds-webdds-bridged`), das REST (§8.3) und SOAP (§8.4) aus
einem Prozess über `crates/dcps`s öffentliche typisierte API bedient.

Implementation:

- `crates/web/` — WebDDS-Object-Model + REST-PSM (URI-Routing, Status-Mapping, Header-Set, XML-Element-Tag-Registry), 10 Module, 81 Tests grün.
- `crates/webdds-bridge/` — Daemon `zerodds-webdds-bridged`: `DdsBackend`-Impl
  gegen die typisierte `zerodds-dcps`-API (Gateway), Typauflösung aus
  config-benanntem IDL oder TypeLookup-Discovery (TypeRegistry), XML↔
  DynamicData-Codec über das Typ-Deskriptor (dynxml, nie Byte-Rätselraten),
  REST- + SOAP-Dispatch, ein handgeschriebener HTTP/1.1-Server
  (Content-Length + chunked + Keep-Alive), WSDL-Generierung aus
  config-benannten IDL-Interfaces (`zerodds_lang_rules::adapter::interface`),
  Business-Interface-Dispatch über DDS-RPC 1.0s Basic-Mapping (ein
  Topic-Paar pro Interface, §7.5.1.1). 119 Tests grün (110 Unit + 9 E2E,
  darunter zwei gegen einen echten, zur Build-Zeit generierten `Replier`
  auf einem zweiten, unabhängigen `DomainParticipant` über echtes
  RTPS/UDP).

---

## §1 Scope

### §1.1-§1.6 Overview + Conformance + Examples

**Spec:** §1, S. 1-4 — DDS-Gateway-Service über HTTP/REST. §1.1/§1.2/§1.6
sind Zweck, Hintergrund und ausdrücklich nicht-normative Beispielszenarien;
prüfbare Aussagen stehen in §1.3.1 (Session über mehrere physische
Verbindungen; Access-Control-Modell je Principal), §1.4 (das Objektmodell
erweitert das DDS-Objektmodell um Access-Control- und
Application-Management-Modell) und §1.5 (zwei Plattform-Mappings: REST und
SOAP).

**Repo:** `crates/web/src/lib.rs` (Crate-Doc + Re-Exports),
`crates/web/src/session.rs::SessionId`,
`crates/web/src/access_control.rs::{Permissions, Rule, Operation, Decision}`,
`crates/web/src/rest.rs::parse_route`; das SOAP-Mapping aus §1.5 liegt in
`crates/zerodds-soap/src/wsdl.rs`.

**Tests:** drei Tests, je einer pro prüfbarer Aussage:
`crates/web/src/lib.rs::scope_tests::scope_session_identity_survives_reconstruction_across_connections`
(§1.3.1 Grund 2 — dieselbe Token-Zeichenkette ergibt dieselbe `SessionId`,
auch neu konstruiert; ein anderes Token ist eine andere Session, ein leeres
ist keine),
`crates/web/src/lib.rs::scope_tests::scope_access_control_gates_per_operation_and_per_topic`
(§1.3.1 Grund 3 + §1.4 — die Entscheidung hängt an Operation und Topic;
richtige Topic mit falscher Operation wird abgelehnt, ein nicht gelistetes
Topic ebenso, ein leerer Regelsatz lehnt ab statt zu erlauben),
`crates/web/src/lib.rs::scope_tests::scope_rest_mapping_addresses_the_webdds_object_model`
(§1.5 — das REST-Mapping adressiert Application und DomainParticipant unter
dem §8.3.1-Präfix, eine URI ohne dieses Präfix gehört nicht zur Plattform).

**Status:** done

---

## §2 Conformance

### §2 Profile-Triple (REST + SIMPLE-REST + SIMPLE-WSDL-SOAP)

**Spec:** §2, S. 5 — REST `Mandatory`, SIMPLE-REST + SIMPLE-WSDL-SOAP
`Optional`.

**Repo:** ZeroDDS implementiert REST (Mandatory) plus SOAP.

**Tests:** `crates/web/src/rest.rs::tests::rest_prefix_constant_matches_spec`,
`crates/web/src/rest.rs::tests::table_5_every_pim_operation_has_a_route`,
`crates/web/src/headers.rs::tests::validate_required_passes_with_all_three_headers`,
`crates/web/src/status.rs::tests::ok_maps_to_201_for_post_create`;
SIMPLE-WSDL-SOAP: `crates/zerodds-soap/src/wsdl.rs::tests::wsdl_11_emits_definitions_root`,
`crates/zerodds-soap/src/envelope.rs::tests::parse_round_trip`.

**Status:** done — Mandatory-Profile (REST) und Optional SIMPLE-WSDL-
SOAP-Profile beide implementiert (siehe §8.4); SIMPLE-REST-Profile
analog REST-Subset.

---

## §3-§6 Referenzen + Terminologie

### §3 Normative References

**Spec:** §3, S. 5 — DDS 1.2, DDS-CCM, DDS-XTYPES, HTTP RFC 2616,
HTTP-Auth RFC 2617, WebSockets RFC 6455, SOAP 1.2, WSDL 1.1, XML 1.1.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenz-Liste; HTTP/RFC/XML werden in den jeweiligen Konsumenten-Items §7/§8 operativ erfüllt.

### §4-§6 Terms + Symbols + Acks

**Spec:** §4-§6, S. 6-8 — Terms and Definitions, Symbols,
Acknowledgements.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar/Symbole/Acks; ohne Code-Mapping.

---

## §7 WebDDS Object Model

### §7.1 WebDDS Object Model Overview

**Spec:** §7.1, S. 9 — das WebDDS-Object-Model ist ein Containment-Baum:
`WebDDS::Root` enthält Application-, Client-, Type- und QoS-Instanzen,
eine Application enthält ihre DomainParticipants.

**Repo:** `crates/web/src/model.rs::{WebDdsRoot, Application,
DomainParticipant, Client}` — Containment als Besitz-Beziehung
(`WebDdsRoot::applications`, `Application::participants`,
`WebDdsRoot::clients`).

**Tests:** `crates/web/src/model.rs::tests::every_model_object_is_reachable_from_the_root`,
`crates/web/src/model.rs::tests::application_carries_participants`.

**Status:** done

### §7.2 Singleton WebDDS::Root

**Spec:** §7.2, S. 10 — `WebDDS::Root` ist Singleton; alle anderen
Klassen sind über Root erreichbar.

**Repo:** `crates/web/src/model.rs::WebDdsRoot` als Singleton-
Pattern (instanziiert über Application-Lifecycle).

**Tests:** `crates/web/src/model.rs::tests::root_starts_empty_and_is_the_only_container`,
`crates/web/src/model.rs::tests::root_holds_one_entry_per_application_name`,
`crates/web/src/model.rs::tests::every_model_object_is_reachable_from_the_root`.

**Status:** done

### §7.3 Access Control + SessionId

**Spec:** §7.3, S. 11-13 — `AccessController` + Session-Tracking.

**Repo:** `crates/web/src/session.rs::SessionId` +
`crates/web/src/model.rs::Client` + `crates/web/src/access_control.rs`
(Permissions-Datenmodell + `evaluate(op, topic) -> Decision`-
Engine, Glob-Matcher mit `*`/`prefix*`/`*suffix`-Pattern, Spec
§7.3-Decision-Tree: erste Deny-Match wins, sonst erste Permit-Match,
sonst default).

**Tests:** `crates/web/src/session.rs::tests::{session_id_carries_token_string,
empty_session_id_is_invalid}`;
`crates/web/src/access_control.rs::tests::{empty_permissions_returns_default_deny,
allow_rule_grants_permit, deny_rule_overrides_allow_when_listed_first,
glob_prefix_pattern_matches, glob_suffix_pattern_matches,
operation_mismatch_falls_through_to_default,
first_matching_deny_wins_over_later_allow}` (7).

**Status:** done

### §7.3.1 Class WebDDS::Root

**Spec:** §7.3.1, S. 13-17 — Root-Operations: create_application,
delete_application, get_applications (mit fnmatch),
create/delete/update/get_qos_library, create/delete/get_type.

**Repo:** `WebDdsRoot::{create_application, delete_application,
get_applications}` mit fnmatch-Wildcard-Subset.

**Tests:** `crates/web/src/model.rs::tests::{create_application_returns_session_id,
duplicate_application_yields_object_already_exists,
delete_application_removes_existing,
delete_unknown_application_yields_invalid_object,
get_applications_with_wildcard_matches_all,
get_applications_with_prefix_pattern_matches_subset,
get_applications_with_exact_match}`.

**Cross-Ref Schema-Storage (Phase-B-Cluster-8):** QosLibrary CRUD
über `crates/xml/src/qos.rs::QosLibrary`, `crates/xml/src/parser.rs::parse_xml_tree`;
Type-CRUD über `crates/types/`-TypeLibrary. Bridge zur DCPS-API
in `crates/web/src/bridge.rs::DdsBackend`-Trait (siehe §7.4).

**Status:** done

### §7.4 Class-Operationen Application + Participant + …

**Spec:** §7.4, S. 17-58 — DomainParticipant, Topic, Publisher,
Subscriber, DataWriter, DataReader, WaitSet — alle CRUD.

**Repo:** Datenmodell-Stubs in `crates/web/src/model.rs::{Application, DomainParticipant}`.

**Repo:** `crates/web/src/bridge.rs::DdsBackend` Trait mit allen ~35
§7.4-Class-Operationen: create/update/delete/list für Application,
Participant, Topic, Publisher, Subscriber, DataWriter, DataReader, plus
write_sample/read_samples und create/update/wait/delete/list_waitset
(§7.4.9s Long-Poll `WaitSet::get`). QosLibrary/QosProfile/Type-CRUD
(§7.3.1, Root-Metadaten, keine DCPS-Entität) bewusst nicht im Trait.
`enforce(decision) -> Result<(), BackendError>` komponiert orthogonal
mit der AccessController-Engine aus §7.3.

**Repo:** `crates/webdds-bridge/src/gateway.rs::Gateway` — konkrete
`DdsBackend`-Impl gegen `crates/dcps/`s öffentliche typisierte API
(`DomainParticipant`/`Topic`/`Publisher`/`Subscriber`/`DataWriter`/
`DataReader`/`WaitSet`). Entity-Namen, die der Trait nicht vom Aufrufer
nimmt (Participant/Publisher/Subscriber/Writer/Reader), werden generiert
und im `Created`-Pfad zurückgegeben. `PUT`/Update auf einer DCPS-Entität
ruft `set_qos` auf der lebenden `zerodds-dcps`-Entität
(`zerodds_dcps::Entity`) — `crate::qos_update` liest das `<qos>`-Element
des PUT-Bodys und merged es auf die aktuelle QoS der Entität (nicht auf
einen frischen Default: `zerodds_qos`s eigene Policy-Defaults weichen
von `zerodds_dcps`s Entity-Defaults ab, ein "replace"-Ansatz hätte auf
Changeable=NO-Policies stets einen `ImmutablePolicy`-Fehler ausgelöst,
selbst wenn der Aufrufer sie nie erwähnt hat — gemessen, siehe
`qos_update`s Moduldoc). `UpdateWaitset` ersetzt stattdessen die
Condition-Menge (§7.4.3.5).

**Repo:** der Long-Poll eines WaitSets wartet ohne die Sperre des
Gateways (`Gateway::waitset_handle`, `WaitsetHandle::wait`); Clients
desselben WaitSets warten nacheinander, weil ein DCPS-`WaitSet` ein
zweites gleichzeitiges `wait` mit PRECONDITION_NOT_MET abweist. Vorher
auf dem Linux-Testhost gemessen: Während eines Long-Polls von 4 s dauerte eine
Liste der Applications 3,7 s, jetzt 0,4 ms. Ein `timeout`, der keine
endliche, nicht negative Sekundenzahl ist, und ein `domain_id`, der
keine Domain-ID ist, ergeben `InvalidInput` (422); vorher endete der
Verbindungsthread bei `-1`, `nan`, `1e300` ohne Antwort
(`Duration::from_secs_f64`), `abc` wartete still 10 s, und ein
unlesbarer `domain_id` ergab Domain 0. Ein fehlender `domain_id` ist
weiter Domain 0 (Spec-Text dazu nicht geprüft). Tests:
`crates/webdds-bridge/tests/daemon_e2e.rs::a_long_poll_leaves_other_requests_alone_and_a_bad_timeout_is_refused`,
`crates/webdds-bridge/src/gateway.rs::tests::{a_timeout_is_a_finite_non_negative_number_of_seconds,
a_domain_id_is_read_or_refused}`.

**Repo:** ein `<qos>` ist nicht die einzige Quelle, die §7.4-Bodys für
QoS vorsehen — `<qos_profile>name</qos_profile>` nennt ein gespeichertes
`QosLibrary`/`QosProfile`-Paar per Referenz. `crate::qos_update::
resolve_entity_qos`/`resolve_qos_profile_ref` lösen die Referenz gegen
`crate::metadata::MetadataStore` auf (`zerodds_xml::resolve_profile`)
und mergen ein zusätzlich vorhandenes inline-`<qos>` darüber (Override
gewinnt). §7.4.3.1/§7.4.3.2/§7.4.4.7/§7.4.4.8/§7.4.4.11/§7.4.4.12
erlauben außerdem, dass `create_participant`/`create_publisher`/
`create_subscriber` und ihre `update_*`-Gegenstücke verschachtelte
`<publishers>`/`<subscribers>`/`<data_writers>`/`<data_readers>`
mitbringen — `crate::contained_entities` parst sie,
`crate::gateway::{sync_publishers, sync_subscribers,
sync_data_writers, sync_data_readers}` gleichen die lebenden
DCPS-Entitäten per Name ab (anlegen, QoS neu anwenden, entfernen).
`update_*` unterscheidet dabei "Element fehlt" (unverändert lassen)
von "Element ist leer" (alle Contained-Entities dieser Art löschen) —
wörtlich, was der Spec-Satz "if the …ObjectRepresentation specifies a
set of contained entities" fordert.

**Tests:** `crates/webdds-bridge/src/qos_update.rs::tests` (u.a.
`qos_profile_reference_resolves_a_stored_profile`,
`qos_profile_reference_to_an_unknown_profile_is_a_bad_request`,
`inline_qos_overrides_the_profiles_value_for_the_same_policy`);
`crates/webdds-bridge/src/contained_entities.rs::tests` (u.a.
`nested_publishers_and_data_writers_parse`,
`a_publishers_qos_profile_reference_resolves`,
`a_contained_entity_without_a_name_is_a_bad_request`);
`crates/webdds-bridge/src/gateway.rs::tests::
update_participant_creates_and_reconciles_contained_entities` (echte
DCPS-Entitäten, Offline-Domain: Anlegen → Abgleichen →
Unverändert-lassen im vollen Zyklus).

**Tests:** `crates/web/src/bridge.rs::tests::{enforce_permit_allows_call,
enforce_deny_returns_forbidden,
permission_check_then_backend_call_chains_correctly,
create_then_delete_application_round_trip,
create_application_twice_yields_conflict}` (5);
`crates/webdds-bridge/src/gateway.rs::tests` (3) +
`crates/webdds-bridge/tests/daemon_e2e.rs::write_over_http_arrives_at_an_independent_participant`
(HTTP-Write über den Daemon, verifiziert von einem zweiten,
unabhängigen `DomainParticipant` über echtes RTPS/UDP).

**Status:** done

### §7.4.8 DataReader::read mit SampleSelector

**Spec:** §7.4.8, S. 51-53 — `read` mit `SampleSelector`-BNF-Grammar
(FilterExpression + MetadataExpression mit AND/OR-Logik).

**Repo:** `crates/web/src/sample_selector.rs::parse_sample_selector`
liefert AST (`SampleSelector` mit `filter: Option<FilterExpression>`
+ `metadata: Vec<MetadataExpression>`). Recursive-descent-Parser
mit allen 6 Vergleichs-Operatoren (=,!=,<,<=,>,>=), AND/OR-Logik,
Parentheses, Integer/String/Boolean-Literale, dotted Field-Path,
sample_state/view_state/instance_state-Metadata mit allen Werten.

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

**Spec:** §8.1, S. 59 — Content-Type `application/dds-web+xml`
(Mandatory) als XML-Format; weitere Formate optional. Volltextsuche im
PDF (§8.2, §8.3.5 Tab 7/8) findet keine zweite Repräsentation — DDS-WEB
1.0 definiert kein JSON.

**Repo:** `crates/web/src/representation.rs::{ContentType,
CONTENT_TYPE_DDS_WEB_XML}`. Der Konstantenwert selbst war bis zu diesem
Stand codebase-weit falsch geschrieben (`application/zerodds-web+xml`,
sechs Dateien) — gemessener Bug, kein Repräsentations-Fehlen.

**Tests:** `crates/web/src/representation.rs::tests::content_type_constant`.

**Status:** done

### §8.2 Representations

**Spec:** §8.2, S. 60-61 — Vier Representation-Kinds: QoS, Type,
Data, Entity (mit XML-Element-Tags pro Kind).

**Repo:** `crates/web/src/representation.rs::{RepresentationKind,
element_name}`.

**Tests:** `crates/web/src/representation.rs::tests::element_names_match_spec_tab_6`,
`crates/web/src/representation.rs::tests::types_representation_uses_dds_xtypes_root`.

**Status:** done — Element-Tag-Registry. XML-Body-Serialization
ist Caller-Layer.

### §8.3.1 Mapping zu Resource URIs

**Spec:** §8.3.1, S. 62 Tab 4 — alle URI-Patterns mit Prefix
`/dds/rest1`.

**Repo:** `crates/web/src/rest.rs::{REST_PREFIX, RestRoute,
parse_route, RestMethod}` mit allen 30+ Routes.

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

### §8.3.2 Mapping zu HTTP-Methods + Status-Codes

**Spec:** §8.3.2, S. 62-64 — Method-Mapping (POST/PUT/GET/DELETE) +
Status-Codes (201, 204, 200, 409, 422, 404, 401, 403, 500).

**Repo:** `crates/web/src/status.rs::{ReturnStatus, http_status_for}`.

**Tests:** `crates/web/src/status.rs::tests::{ok_maps_to_201_for_post_create,
ok_maps_to_204_for_delete, ok_maps_to_200_for_get,
ok_maps_to_204_for_put_update, object_already_exists_maps_to_409,
invalid_input_maps_to_422, invalid_object_maps_to_404,
access_denied_maps_to_401, permissions_error_maps_to_403,
generic_service_error_maps_to_500, dds_error_maps_to_500}` (11).

**Status:** done

### §8.3.3 Komplett-Mapping (Tab 5)

**Spec:** §8.3.3, S. 64-67 — Tab 5, "Complete mapping of WebDDS PIM
operations to REST methods": 48 Zeilen, je Operation eine HTTP-Methode und
eine URI. Dazu der Satz über der Tabelle: "the HEAD HTTP method shall be
supported on the same URIs as the GET method. The HEAD method shall behave
identically to the GET method except for it shall return no body."

**Repo:** `crates/web/src/rest.rs::RestRoute` (47 Routen-Varianten +
`Unknown`), `crates/web/src/rest.rs::parse_route`,
`crates/web/src/rest.rs::canonical_segment`.

**Tests:** die Tabelle steht als `TABLE_5` Zeile für Zeile im Test
(Operation, Methode, URI, aus dem PDF übernommen) und wird von fünf Tests
ausgewertet:
`crates/web/src/rest.rs::tests::table_5_every_pim_operation_has_a_route`
(keine Zeile fällt auf `RestRoute::Unknown` durch),
`crates/web/src/rest.rs::tests::table_5_rows_do_not_collide` (48 Zeilen
ergeben 48 verschiedene Routen — zwei Operationen dürfen nicht auf derselben
Variante landen, sonst wäre das Mapping mehrdeutig),
`crates/web/src/rest.rs::tests::table_5_alternate_segment_spellings_route_identically`
(Tab 5 schreibt dieselbe Sammlung an zwei Stellen verschieden —
`domain_participants` gegen `participants`, `datawriters`/`datareaders` gegen
`data_writers`/`data_readers`; beide Schreibweisen ergeben dieselbe Route),
`crates/web/src/rest.rs::tests::canonicalisation_is_positional_not_global`
(die Vereinheitlichung greift nur in den Sammlungs-Slots, eine Application
namens `participants` bleibt ein Name),
`crates/web/src/rest.rs::tests::head_behaves_like_get_on_the_same_uris`
(jede GET-Zeile der Tabelle routet unter HEAD identisch).

**Befund aus dem Testschreiben:** Die Tabelle war beim Anlegen dieses Tests
nicht vollständig abgebildet. 23 der 48 Zeilen fielen auf
`RestRoute::Unknown` durch — die gesamte Update/Delete/Get-Ebene für
Waitsets, registrierte Typen, Topics, Publisher, Subscriber, DataWriter und
DataReader sowie `Publisher::create_datawriter` und
`Subscriber::create_datareader`. Zusätzlich war `RestMethod::Head` zwar als
Enum-Variante vorhanden, wurde aber von `parse_route` nirgends behandelt, so
dass jede GET-URI unter HEAD auf `Unknown` fiel. Beides ist mit diesem Stand
gebaut: die 23 fehlenden Routen sind implementiert, HEAD routet wie GET, und
die zweite Schreibweise der Sammlungssegmente wird angenommen.

**Status:** done

### §8.3.4 Object-Representations (Tab 6)

**Spec:** §8.3.4, S. 68-69 Tab 6 — XML-Element-Tag-Registry.

**Repo:** `crates/web/src/representation.rs::element_name`.

**Tests:** `crates/web/src/representation.rs::tests::element_names_match_spec_tab_6`.

**Status:** done

### §8.3.5 HTTP-Headers (Tab 7+8)

**Spec:** §8.3.5, S. 70 — Required Request-/Response-Headers (Accept,
Content-Length, Content-Type, Cache-Control, OMG-DDS-API-Key,
Authentication-Info, Date, Expires, Location, Last-Modified).

**Repo:** `crates/web/src/headers.rs::{RequestHeaders,
ResponseHeaders, REQUEST_API_KEY, validate_required}`.

**Repo (Daemon):** die Library-Typen allein genügten nicht — der Daemon
rief `validate_required` bis zu diesem Stand nirgends auf, ein
Request ohne `Accept` (oder mit einem, das keine dieser
Repräsentationen nennt) wurde trotzdem bedient. `crates/webdds-bridge/
src/rest_service.rs::check_representation_headers` prüft jetzt vor
jeder REST-Operation: fehlender `Accept` → 400, ein `Accept`, das
`application/dds-web+xml` (oder ein passendes `*/*`/`application/*`)
nicht nennt → 406, ein `Content-Type`, der etwas anderes nennt → 415.

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

### §8.4 Simplified SOAP Platform

**Spec:** §8.4, S. 71+ — Optional SIMPLE-WSDL-SOAP.

**Repo:** `crates/zerodds-soap/src/{envelope,addressing,fault,mtom,
security,wsdl}.rs` (SOAP-Envelope-Codec, WS-Addressing, MTOM,
WS-Security-Hooks, WSDL-Emitter; 1218 SLOC).

**Repo (Daemon):** `crates/webdds-bridge/src/soap_service.rs` dispatcht
den SOAP-Envelope-Body (ein DDS-WEB-Objektmodell-Op-Name wie
`CreateApplication`/`WriteSample`, dieselbe Operationsmenge wie REST)
gegen denselben `Gateway` — REST und SOAP sind Schwester-Bindings
eines Objektmodells, kein zweites Protokoll. `crates/webdds-bridge/src/wsdl_bridge.rs`
generiert die WSDL sowohl für dieses feste Op-Set als auch für jedes
config-benannte IDL-`interface` (via `zerodds_lang_rules::adapter::interface`
— die im Plan §3 verlangte IDL→Operation-Brücke).

Eine Business-Interface-Operation ist WSDL-beschrieben *und*
dispatchbar: `crate::rpc_dispatch` baut, pro konfiguriertem `interface`,
DDS-RPC 1.0 §7.5.1.1s Basic-Mapping-Typen dynamisch — **ein**
Request/Reply-Topic-Paar pro Interface (`service_name` =
qualifizierter Interface-Name, z. B. `demo_Calculator`), ein
`Call`/`Return`-Union über alle Operationen (Discriminator =
`operation_hash`), pro nicht-`oneway`-Operation ein `Result`-Union
(`dds::RETCODE_OK` → `Out`-Struct, je eine `raises`-Exception als
weiterer Case, Discriminator = `exception_hash`). Jeder synthetisierte
Wire-Typ ist `@final` (klassisches CDR, kein `DHEADER`) — dieselbe Form,
die `zerodds-idlc generate --lang rust`/`--lang cpp` für ein
`interface` selbst erzeugen
(`crates/lang-rules/src/emit/interface_rust.rs`), und dieselben
`operation_hash`/`exception_hash`/Struct-Namensfunktionen aus
`zerodds_lang_rules::adapter::interface`, die diese Generatoren selbst
aufrufen — keine parallele, nur zufällig gleich rechnende Ableitung.
Eine Operation, die kein konfiguriertes Interface deklariert (und
keine DDS-WEB-Objektmodell-Operation ist), liefert weiterhin einen
benannten `env:Receiver`-Fault statt eines stillen No-Op; eine
deklarierte `raises`-Exception ebenso, benannt mit ihren Feldern
(`OperationException`), nicht ein leerer oder falscher Erfolg.

**Interop-Beleg:** `crates/webdds-bridge/tests/soap_rpc_e2e.rs` ruft
nicht gegen einen handgeschriebenen Stand-in, sondern gegen einen
**echten**, mit `zerodds_lang_rules::generate::source(..,
Language::Rust, ..)` zur Build-Zeit generierten `Replier`
(`zerodds-rpc-rust-e2e`s `Calculator`-Binding, `module demo { exception
DivByZero; @DDSService interface Calculator { … } }` — Rückgabewert,
`raises`, `oneway`, `attribute`, `inout`, jede DDS-RPC-1.0-§7.5.1.1-Form
in einem Interface) auf einem zweiten, unabhängigen
`DomainParticipant`, über echtes RTPS/UDP.

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
soap_call_to_an_unconfigured_operation_is_a_named_fault}` — die ersten
zwei gegen den echten, generierten `Replier`;
`crates/webdds-bridge/tests/daemon_e2e.rs::soap_write_sample_arrives_at_an_independent_participant`
(ein `WriteSample`, dessen `<data>` das Sample als Elemente in der
REST-Darstellung trägt, kommt bei einem zweiten, unabhängigen
`DomainParticipant` über RTPS/UDP an; vorher auf dem Linux-Testhost gemessen:
`env:Receiver` „malformed XML body: no root element“, weil der
Feldleser die Kinder von `<data>` als Liste nahm),
`crates/webdds-bridge/src/xmlfields.rs::tests::a_child_subtree_comes_back_as_xml`.

**Status:** done — SOAP-PSM als Differenzierungs-Feature für
Java-Enterprise-Bestand voll abgedeckt.

---

## Audit-Status

16 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-web` — 81 Tests grün, 0 failed.
  Module: `access_control`, `bridge`, `headers`, `model`,
  `representation`, `rest`, `sample_selector`, `session`, `status`, plus
  `scope_tests` (§1) und die Tab-5-Tests in `rest` (§8.3.3).
* `cargo test -p zerodds-soap --lib` — 37 Tests grün. Module:
  `addressing`, `envelope`, `fault`, `mtom`, `security`, `wsdl`.
* `cargo test -p zerodds-webdds-bridge` — 110 Unit- + 9 E2E-Tests grün
  (der Daemon, der die zwei Libraries zusammenführt und tatsächlich
  bedient; siehe `crates/webdds-bridge/README.md`).

`zerodds-web` deckt §7-§8.3, `zerodds-soap` deckt §8.4, beide als
Bridge-agnostische Libraries. `zerodds-webdds-bridge` ist die
Caller-Layer-Realisierung (HTTP-Server, `DdsBackend`-Impl,
XML-Body-Codec) — Daemon `zerodds-webdds-bridged`. Rund 2 (DDS-XTYPES-
XML-Typdateien als Typquelle), Business-Interface-SOAP-Dispatch (echter
DDS-RPC-Request/Reply über `zerodds-rpc`s `Requester`/`Replier`,
`crate::rpc_dispatch`), automatischer TypeLookup-Poll und Admin-
`/catalog`+`/healthz` neben `/metrics` sind seither gebaut. Rund 3
schließt die drei zuvor dokumentierten offenen Punkte, gemessen (nicht
nur behauptet):

- DDS-RPC-Topic-Granularität ist jetzt ein Topic-Paar pro Interface
  (§7.5.1.1), gegen einen echten `zerodds-idlc`-generierten `Replier`
  geprüft (`tests/soap_rpc_e2e.rs`), nicht mehr ein Paar pro Operation.
- `update_*`s `<qos_profile>`-Referenz löst gegen die
  `QosLibrary`/`QosProfile`-Metadaten auf, und `create_*`/`update_*`
  von Participant/Publisher/Subscriber lesen und gleichen
  Contained-Entities ab (§7.4.3.1/§7.4.3.2/§7.4.4.7/§7.4.4.8/
  §7.4.4.11/§7.4.4.12).
- Content-Negotiation ist scharf: DDS-WEB 1.0 definiert keine zweite
  Repräsentation neben `application/dds-web+xml` (Volltext-geprüft,
  §8.2/§8.3.5) — offen war nicht ein fehlendes Format, sondern ein
  falsch geschriebener Konstantenwert (sechs Dateien) und eine
  Bibliotheksfunktion, die der Daemon nie aufrief; beides ist jetzt
  gemessen geschlossen.

Siehe `crates/webdds-bridge/README.md` "Open points, closed" für die
Belege je Punkt.
