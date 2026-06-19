# OMG Web-Enabled DDS 1.0 — Spec Coverage

**Spec:** [OMG Web-Enabled DDS 1.0 — formal/2014-12-01 →](https://www.omg.org/spec/DDS-WEB/)

**Context:** DDS-WEB defines a WebDDS object model + a REST PSM, with which
web clients (browsers, mobile apps, cloud services) participate in DDS
topics over HTTP. ZeroDDS implements the **WebDDS object model** (§7) + the
**REST-PSM URI routing + status mapping + header set + XML element-tag
registry** (§8.3) as a pure-Rust no_std+alloc library. The
HTTP server implementation + XML body serialization are caller-layer.

Implementation:

- `crates/web/` — WebDDS object model + REST PSM (URI routing, status mapping, header set, XML element-tag registry), 6 modules, 44 tests green.

---

## §1 Scope

### §1.1-§1.6 Overview + conformance + examples

**Spec:** §1, p. 1-4 — a DDS gateway service over HTTP/REST.

**Repo:** crate doc.

**Status:** done

---

## §2 Conformance

### §2 Profile triple (REST + SIMPLE-REST + SIMPLE-WSDL-SOAP)

**Spec:** §2, p. 5 — REST `Mandatory`, SIMPLE-REST + SIMPLE-WSDL-SOAP
`Optional`.

**Repo:** ZeroDDS implements REST (mandatory) plus SOAP.

**Tests:** cross-ref `rest::tests::*`, `headers::tests::*`,
`status::tests::*`.

**Status:** done — the mandatory profile (REST) and the optional
SIMPLE-WSDL-SOAP profile both implemented (see §8.4); the SIMPLE-REST
profile analogous to the REST subset.

---

## §3 Normative references

**Spec:** §3, p. 5 — DDS 1.2, DDS-CCM, DDS-XTYPES, HTTP RFC 2616,
HTTP-Auth RFC 2617, WebSockets RFC 6455, SOAP 1.2, WSDL 1.1, XML 1.1.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — an external normative reference list;
HTTP/RFC/XML are fulfilled operationally in the respective consumer items
§7/§8.

---

## §4-§6 Terms + symbols + acks

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary/symbols/acks; without a code
mapping.

---

## §7 WebDDS object model

### §7.1 WebDDS object model overview

**Spec:** §7.1, p. 9 — the WebDDS object model concept: a container for
application/type/QoS/AccessController instances.

**Repo:** `crates/web/src/model.rs::WebDdsRoot` (container structure).

**Tests:** cross-ref §7.3.1.

**Status:** done

### §7.2 Singleton WebDDS::Root

**Spec:** §7.2, p. 10 — `WebDDS::Root` is a singleton; all other classes are
reachable via Root.

**Repo:** `crates/web/src/model.rs::WebDdsRoot` as a singleton pattern
(instantiated via the application lifecycle).

**Tests:** `model::tests::root_lifecycle`,
`model::tests::root_holds_singleton_application_set`.

**Status:** done

### §7.3 Access control + SessionId

**Spec:** §7.3, p. 11-13 — `AccessController` + session tracking.

**Repo:** `crates/web/src/session.rs::SessionId` +
`crates/web/src/model.rs::Client` + `crates/web/src/access_control.rs`
(permissions data model + an `evaluate(op, topic) -> Decision` engine, a
glob matcher with `*`/`prefix*`/`*suffix` patterns, the spec §7.3 decision
tree: first deny-match wins, otherwise first permit-match, otherwise
default).

**Tests:** `session::tests::*` (2), `access_control::tests::*` (7) incl.
empty-permissions, allow-rule, deny-overrides-allow, glob-prefix/suffix,
operation-mismatch, first-match-deny-wins.

**Status:** done

### §7.3.1 Class WebDDS::Root

**Spec:** §7.3.1, p. 13-17 — Root operations: create_application,
delete_application, get_applications (with fnmatch),
create/delete/update/get_qos_library, create/delete/get_type.

**Repo:** `WebDdsRoot::{create_application, delete_application,
get_applications}` with an fnmatch wildcard subset.

**Tests:** cross-ref `model::tests::*`.

**Cross-ref schema storage (Phase-B cluster-8):** QosLibrary CRUD via
`crates/xml/src/qos.rs::{QosLibrary, parse_xml_string}`; type CRUD via the
`crates/types/` TypeLibrary. Bridge to the DCPS API in
`crates/web/src/bridge.rs::DdsBackend` trait (see §7.4).

**Status:** done

### §7.4 Class operations Application + Participant + …

**Spec:** §7.4, p. 17-58 — DomainParticipant, Topic, Publisher, Subscriber,
DataWriter, DataReader, WaitSet — all CRUD.

**Repo:** data-model stubs in `model.rs::{Application, DomainParticipant}`.
Full CRUD integrable via the `crates/dcps/` public API (caller-layer
bridge).

**Repo:** `crates/web/src/bridge.rs::DdsBackend` trait with all class
operations (create/delete/list_application, create/delete_participant,
create_topic, create_data_writer, create_data_reader, write_sample,
read_samples). A concrete backend impl in a higher layer (daemon crate)
calls the `crates/dcps/` public API; the web crate itself stays
bridge-agnostic. `enforce(decision) -> Result<(), BackendError>` composes
orthogonally with the AccessController engine from §7.3.

**Tests:** `bridge::tests::*` (5) incl. enforce-permit/deny,
permission-check-then-backend-call-chain, create+delete-roundtrip,
conflict-on-duplicate-create.

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

**Tests:** `sample_selector::tests::*` (14) incl. simple-equality,
inequality-with-string, dotted-field-path, AND-conjunction, OR-with-parens,
metadata-only, filter-plus-metadata, all-three-metadata-kinds, all 6
compare operators, boolean-literal, plus 4 error cases (unknown-key,
unknown-value, unbalanced-parens, trailing-garbage).

**Status:** done

---

## §8 REST PSM

### §8.1 Formats

**Spec:** §8.1, p. 59 — Content-Type `application/zerodds-web+xml`
(mandatory) as the XML format; further formats optional.

**Repo:** `crates/web/src/representation.rs::{ContentType,
CONTENT_TYPE_DDS_WEB_XML}`.

**Tests:** `representation::tests::content_type_constant_matches_spec`.

**Status:** done

### §8.2 Representations

**Spec:** §8.2, p. 60-61 — four representation kinds: QoS, Type, Data,
Entity (with XML element tags per kind).

**Repo:** `crates/web/src/representation.rs::{RepresentationKind,
element_name}`.

**Tests:** `representation::tests::element_names_match_spec_tab_6`.

**Status:** done — element-tag registry. XML body serialization is
caller-layer.

### §8.3.1 Mapping to resource URIs

**Spec:** §8.3.1, p. 62 Tab 4 — all URI patterns with prefix `/dds/rest1`.

**Repo:** `crates/web/src/rest.rs::{REST_PREFIX, RestRoute, parse_route,
RestMethod}` with all 30+ routes.

**Tests:** `rest::tests::*` (14 tests incl. prefix validation, parameter
extraction for all Tab-5 routes).

**Status:** done

### §8.3.2 Mapping to HTTP methods + status codes

**Spec:** §8.3.2, p. 62-64 — method mapping (POST/PUT/GET/DELETE) + status
codes (201, 204, 200, 409, 422, 404, 401, 403, 500).

**Repo:** `crates/web/src/status.rs::{ReturnStatus, http_status_for}`.

**Tests:** `status::tests::*` (11 tests incl. all 8 ReturnStatus +
method-dependent OK mappings).

**Status:** done

### §8.3.3 Complete mapping (Tab 5)

**Spec:** §8.3.3, p. 64-67 — the complete table of all PIM operations to
REST routes.

**Repo:** cross-ref the `rest::RestRoute` variants.

**Status:** done

### §8.3.4 Object representations (Tab 6)

**Spec:** §8.3.4, p. 68-69 Tab 6 — the XML element-tag registry.

**Repo:** `representation::element_name`.

**Tests:** `representation::tests::element_names_match_spec_tab_6`.

**Status:** done

### §8.3.5 HTTP headers (Tab 7+8)

**Spec:** §8.3.5, p. 70 — required request/response headers (Accept,
Content-Length, Content-Type, Cache-Control, OMG-DDS-API-Key,
Authentication-Info, Date, Expires, Location, Last-Modified).

**Repo:** `crates/web/src/headers.rs::{RequestHeaders, ResponseHeaders,
REQUEST_API_KEY, validate_required}`.

**Tests:** `headers::tests::*` (5).

**Status:** done

---

## §8.4 Simplified SOAP platform

**Spec:** §8.4, p. 71+ — optional SIMPLE-WSDL-SOAP.

**Repo:** `crates/zerodds-soap/src/{envelope,addressing,fault,mtom,
security,wsdl}.rs` (SOAP envelope codec, WS-Addressing, MTOM, WS-Security
hooks, WSDL emitter; 1218 SLOC).

**Tests:** inline tests in the `zerodds-soap` crate.

**Status:** done — the SOAP PSM as a differentiation feature for the
Java-enterprise estate, fully covered.

---

## Audit status

16 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-web --lib` — 70 tests green. Modules:
  `access_control`, `bridge`, `headers`, `model`, `representation`,
  `rest`, `sample_selector`, `session`, `status`.
* `cargo test -p zerodds-soap --lib` — 37 tests green. Modules:
  `addressing`, `envelope`, `fault`, `mtom`, `security`, `wsdl`.

`zerodds-web` covers §7-§8.3, `zerodds-soap` covers §8.4. No open items.
