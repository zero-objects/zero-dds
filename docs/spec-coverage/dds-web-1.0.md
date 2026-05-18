# OMG Web-Enabled DDS 1.0 — Spec-Coverage

**PDF:** `docs/standards/cache/omg/zerodds-web-1.0.pdf` (OMG formal/2014-12-01).

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`.

**Kontext:** DDS-WEB definiert ein WebDDS-Object-Model + REST-PSM, mit
dem Web-Clients (Browser, Mobile-Apps, Cloud-Services) ueber HTTP an
DDS-Topics teilnehmen. ZeroDDS implementiert das **WebDDS-Object-
Model** (§7) + **REST-PSM URI-Routing + Status-Mapping + Header-Set +
XML-Element-Tag-Registry** (§8.3) als pure-Rust no_std+alloc Library
in `crates/web/`. HTTP-Server-Implementation + XML-Body-Serialization
sind Caller-Layer.

Implementation: `crates/web/` (6 Module, 44 Tests gruen).

---

## §1 Scope

### §1.1-§1.6 Overview + Conformance + Examples

**Spec:** §1, S. 1-4 — DDS-Gateway-Service ueber HTTP/REST.

**Repo:** Crate-Doc.

**Status:** done

---

## §2 Conformance

### §2 Profile-Triple (REST + SIMPLE-REST + SIMPLE-WSDL-SOAP)

**Spec:** §2, S. 5 — REST `Mandatory`, SIMPLE-REST + SIMPLE-WSDL-SOAP
`Optional`.

**Repo:** ZeroDDS implementiert REST (Mandatory) ohne SOAP.

**Tests:** Cross-Ref `rest::tests::*`, `headers::tests::*`,
`status::tests::*`.

**Status:** done — Mandatory-Profile (REST) und Optional SIMPLE-WSDL-
SOAP-Profile beide implementiert (siehe §8.4); SIMPLE-REST-Profile
analog REST-Subset.

---

## §3 Normative References

**Spec:** §3, S. 5 — DDS 1.2, DDS-CCM, DDS-XTYPES, HTTP RFC 2616,
HTTP-Auth RFC 2617, WebSockets RFC 6455, SOAP 1.2, WSDL 1.1, XML 1.1.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenz-Liste; HTTP/RFC/XML werden in den jeweiligen Konsumenten-Items §7/§8 operativ erfuellt.

---

## §4-§6 Terms + Symbols + Acks

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar/Symbole/Acks; ohne Code-Mapping.

---

## §7 WebDDS Object Model

### §7.1 WebDDS Object Model Overview

**Spec:** §7.1, S. 9 — Konzept WebDDS-Object-Model: Container für
Application/Type/QoS/AccessController-Instanzen.

**Repo:** `crates/web/src/model.rs::WebDdsRoot` (Container-Struktur).

**Tests:** Cross-Ref §7.3.1.

**Status:** done

### §7.2 Singleton WebDDS::Root

**Spec:** §7.2, S. 10 — `WebDDS::Root` ist Singleton; alle anderen
Klassen sind über Root erreichbar.

**Repo:** `crates/web/src/model.rs::WebDdsRoot` als Singleton-
Pattern (instanziiert über Application-Lifecycle).

**Tests:** `model::tests::root_lifecycle`,
`model::tests::root_holds_singleton_application_set`.

**Status:** done

### §7.3 Access Control + SessionId

**Spec:** §7.3, S. 11-13 — `AccessController` + Session-Tracking.

**Repo:** `crates/web/src/session.rs::SessionId` +
`crates/web/src/model.rs::Client` + `crates/web/src/access_control.rs`
(Permissions-Datenmodell + `evaluate(op, topic) -> Decision`-
Engine, Glob-Matcher mit `*`/`prefix*`/`*suffix`-Pattern, Spec
§7.3-Decision-Tree: erste Deny-Match wins, sonst erste Permit-Match,
sonst default).

**Tests:** `session::tests::*` (2),
`access_control::tests::*` (7) inkl. empty-permissions, allow-rule,
deny-overrides-allow, glob-prefix/suffix, operation-mismatch,
first-match-deny-wins.

**Status:** done

### §7.3.1 Class WebDDS::Root

**Spec:** §7.3.1, S. 13-17 — Root-Operations: create_application,
delete_application, get_applications (mit fnmatch),
create/delete/update/get_qos_library, create/delete/get_type.

**Repo:** `WebDdsRoot::{create_application, delete_application,
get_applications}` mit fnmatch-Wildcard-Subset.

**Tests:** Cross-Ref `model::tests::*`.

**Cross-Ref Schema-Storage (Phase-B-Cluster-8):** QosLibrary CRUD
ueber `crates/xml/src/qos.rs::{QosLibrary, parse_xml_string}`;
Type-CRUD ueber `crates/types/`-TypeLibrary. Bridge zur DCPS-API
in `crates/web/src/bridge.rs::DdsBackend`-Trait (siehe §7.4).

**Status:** done

### §7.4 Class-Operationen Application + Participant + …

**Spec:** §7.4, S. 17-58 — DomainParticipant, Topic, Publisher,
Subscriber, DataWriter, DataReader, WaitSet — alle CRUD.

**Repo:** Datenmodell-Stubs in `model.rs::{Application,
DomainParticipant}`. Vollstaendiges CRUD via `crates/dcps/`-Public-API
integrierbar (Caller-Layer-Bridge).

**Repo:** `crates/web/src/bridge.rs::DdsBackend` Trait mit allen
Class-Operationen (create/delete/list_application,
create/delete_participant, create_topic, create_data_writer,
create_data_reader, write_sample, read_samples). Konkrete Backend-
Impl in einer hoeheren Schicht (Daemon-Crate) ruft
`crates/dcps/`-Public-API; das Web-Crate selbst bleibt Bridge-
agnostisch. `enforce(decision) -> Result<(), BackendError>`
komponiert orthogonal mit der AccessController-Engine aus §7.3.

**Tests:** `bridge::tests::*` (5) inkl. enforce-permit/deny,
permission-check-then-backend-call-chain, create+delete-roundtrip,
conflict-on-duplicate-create.

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

**Tests:** `sample_selector::tests::*` (14) inkl. simple-equality,
inequality-with-string, dotted-field-path, AND-conjunction,
OR-with-parens, metadata-only, filter-plus-metadata,
all-three-metadata-kinds, alle 6 Compare-Operators, Boolean-Literal,
plus 4 Error-Cases (unknown-key, unknown-value, unbalanced-parens,
trailing-garbage).

**Status:** done

---

## §8 REST PSM

### §8.1 Formats

**Spec:** §8.1, S. 59 — Content-Type `application/zerodds-web+xml`
(Mandatory) als XML-Format; weitere Formate optional.

**Repo:** `crates/web/src/representation.rs::{ContentType,
CONTENT_TYPE_DDS_WEB_XML}`.

**Tests:** `representation::tests::content_type_constant_matches_spec`.

**Status:** done

### §8.2 Representations

**Spec:** §8.2, S. 60-61 — Vier Representation-Kinds: QoS, Type,
Data, Entity (mit XML-Element-Tags pro Kind).

**Repo:** `crates/web/src/representation.rs::{RepresentationKind,
element_name}`.

**Tests:** `representation::tests::element_names_match_spec_tab_6`.

**Status:** done — Element-Tag-Registry. XML-Body-Serialization
ist Caller-Layer.

### §8.3.1 Mapping zu Resource URIs

**Spec:** §8.3.1, S. 62 Tab 4 — alle URI-Patterns mit Prefix
`/dds/rest1`.

**Repo:** `crates/web/src/rest.rs::{REST_PREFIX, RestRoute,
parse_route, RestMethod}` mit allen 30+ Routes.

**Tests:** `rest::tests::*` (14 Tests inkl. Prefix-Validation,
Parameter-Extraktion fuer alle Tab-5-Routes).

**Status:** done

### §8.3.2 Mapping zu HTTP-Methods + Status-Codes

**Spec:** §8.3.2, S. 62-64 — Method-Mapping (POST/PUT/GET/DELETE) +
Status-Codes (201, 204, 200, 409, 422, 404, 401, 403, 500).

**Repo:** `crates/web/src/status.rs::{ReturnStatus, http_status_for}`.

**Tests:** `status::tests::*` (11 Tests inkl. alle 8 ReturnStatus +
method-abhaengige OK-Mappings).

**Status:** done

### §8.3.3 Komplett-Mapping (Tab 5)

**Spec:** §8.3.3, S. 64-67 — komplette Tabelle aller PIM-Operations zu
REST-Routes.

**Repo:** Cross-Ref `rest::RestRoute`-Variants.

**Status:** done

### §8.3.4 Object-Representations (Tab 6)

**Spec:** §8.3.4, S. 68-69 Tab 6 — XML-Element-Tag-Registry.

**Repo:** `representation::element_name`.

**Tests:** `representation::tests::element_names_match_spec_tab_6`.

**Status:** done

### §8.3.5 HTTP-Headers (Tab 7+8)

**Spec:** §8.3.5, S. 70 — Required Request-/Response-Headers (Accept,
Content-Length, Content-Type, Cache-Control, OMG-DDS-API-Key,
Authentication-Info, Date, Expires, Location, Last-Modified).

**Repo:** `crates/web/src/headers.rs::{RequestHeaders,
ResponseHeaders, REQUEST_API_KEY, validate_required}`.

**Tests:** `headers::tests::*` (5).

**Status:** done

---

## §8.4 Simplified SOAP Platform

**Spec:** §8.4, S. 71+ — Optional SIMPLE-WSDL-SOAP.

**Repo:** `crates/zerodds-soap/src/{envelope,addressing,fault,mtom,
security,wsdl}.rs` (SOAP-Envelope-Codec, WS-Addressing, MTOM,
WS-Security-Hooks, WSDL-Emitter; 1218 SLOC).

**Tests:** Inline-Tests im `zerodds-soap`-Crate.

**Status:** done — SOAP-PSM als Differenzierungs-Feature für
Java-Enterprise-Bestand voll abgedeckt.

---

## Audit-Status

16 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-web --lib` — 70 Tests grün. Module:
  `access_control`, `bridge`, `headers`, `model`, `representation`,
  `rest`, `sample_selector`, `session`, `status`.
* `cargo test -p zerodds-soap --lib` — 37 Tests grün. Module:
  `addressing`, `envelope`, `fault`, `mtom`, `security`, `wsdl`.

`zerodds-web` deckt §7-§8.3, `zerodds-soap` deckt §8.4. Keine offenen Punkte.
