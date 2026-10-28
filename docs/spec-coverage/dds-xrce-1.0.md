# DDS-XRCE 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xrce-1.0.pdf` (166 Seiten, OMG formal/2020-02-01) —
[OMG DDS-XRCE 1.0](https://www.omg.org/spec/DDS-XRCE/1.0/)

Audit Item für Item gegen die PDF nach `internal/spec-coverage/PROCESS.md`.
Seitenangaben sind PDF-Seiten (PDF-Seite = gedruckte Seite + 8). Jedes Feld
der Typen von §7.7 und Annex A, jede Submessage mit Flags und Payload, jede
Prüfung und jeder Rückgabewert der Operationen von §7.8, jede Regel der
Reliable Streams, jede Zeile von Table 5, Table 6 und Table 15 und jede
Abbildung auf einen Transport ist ein eigenes Item. `done` steht nur, wo Code
und Test die Anforderung abdecken; ein zitierter Test nennt die Testfunktion als
`datei::modul::name`. Eine Fundstelle im Repo nennt Datei, Symbol und, wo es
hilft, die Zeile (Z.); die Zeilen beziehen sich auf den Stand, mit dem diese
Datei eingecheckt wurde (Zeilenangaben sind nur noch dort genannt, wo sie
helfen). Jede Aussage über Verhalten ist aus dem Code gelesen; die zitierten
Tests liefen auf dem Linux-Testhost, dieses Audit führt keinen Test aus. Wo
Code eine Anforderung abbildet, aber kein Test sie prüft, ist das Item
`partial` und die Notiz nennt, welcher Test fehlt.

**Kontext:** DDS-XRCE ist in ZeroDDS über diese Teile verteilt:

- `crates/xrce/` — Codec der Nachrichten (`MessageHeader`, `Submessage`, die 16
  Payloads), die Typen von §7.7 (`ObjectId`, `ObjectVariant`, `ResultStatus`,
  `ObjectInfo`, `SampleInfo`, `TransportLocator`), `CLIENT_Representation` und
  `AGENT_Representation` (`handshake`), Reliable Streams (`reliable`),
  Fragmentierung (`fragment`), fortgesetztes Lesen (`continuous_read`), der
  Objektspeicher je Client (`object_store`), Transporte für UDP, TCP und eine
  serielle Leitung (`transport_*`), DTLS und TLS hinter den Features `dtls` und
  `tls`, ein Erzeuger von CREATE-Nachrichten aus einer XML-Datei
  (`xml_config`).
- `crates/xrce-agent/` — `XrceAgent` (Registry der Clients, Objektspeicher,
  Warteschlange der Samples je Reader); unter dem Feature `daemon` die
  Sitzungslogik (`daemon::session::SessionEngine`), das Lesen der
  Repräsentationen (`daemon::entity_repr`), Profile (`daemon::profiles`), die
  Anbindung an `zerodds-dcps` (`daemon::dcps_bridge::RealDdsPort`), Listener
  für UDP, TCP und seriell (`daemon::transport`), Discovery
  (`daemon::discovery`) und das Binary `zerodds-xrce-agentd`.
- `crates/xrce-client/` — `XrceClient`, ein synchroner Client über dem Trait
  `ClientTransport`.
- `crates/endpoint-e2e/tests/xrce_write_framing.rs` — prüft die
  WRITE_DATA-Rahmen der Endpoint-SDKs weiterer Sprachen gegen den Codec von
  `crates/xrce`.

Die Spec definiert keine Programmierschnittstelle des Clients (§8.1, S. 57:
"Do not have a standard API"). Die Endpoint-SDKs der Sprachen sind deshalb
nicht als Items geführt; die C-API von ZeroDDS hat keine Funktionen für XRCE.

Mehrere Strukturen haben in §7/§8 und in Annex A verschiedene Formen
(`CLIENT_Representation`, `SampleInfo`, die Binärformen der Objekte u. a.). Das
Item nennt die Form, nach der der Code richtet; die Form der anderen Stelle
steht als eigenes Item. Wo der Code der Form folgt, die eProsima
Micro-XRCE-DDS 3.0.1 sendet, beruht das auf dem Mitschnitt unter
`tools/wire-capture/xrce-sessions/`; ein Lauf gegen einen eProsima-Agent oder
-Client ist nicht Teil dieses Audits.

Die Spec nennt `dds_xrce_types.idl` und `dds_xrce_model.xmi` als normative
maschinenlesbare Dateien (S. 1). Sie liegen nicht im Cache; ein Abgleich mit
ihnen fehlt.

**Offene Punkte:** keine.

---

## §1 Scope

### §1 Scope

**Spec:** §1, S. 11 — "This specification defines an XRCE Protocol between a
resource constrained, low-powered device (client) and an Agent (the server).
[...] The specification's purpose and scope are to ensure that applications
based on different vendors' implementations of the XRCE Protocol and XRCE Agent
are compatible and interoperable."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; Protokoll und Agent sind ab §7
einzeln geführt.

---

## §2 Conformance

### §2 Zehn Profile als Konformitätspunkte

**Spec:** §2, S. 12 — "This specification defines ten profiles. Each
constitutes a separate conformance point".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; ob ein Profil erfüllt ist, folgt
aus dem Status der zugeordneten Items. Die Tests in
`crates/xrce/tests/profile_conformance.rs` prüfen nur, dass die Werte der
`SubmessageId` dekodieren; sie belegen kein Verhalten.

### §2 Read Access profile

**Spec:** §2, S. 12 — "Provides the clients the ability to read data on
pre-configured Topics with pre-configured QoS policies. Requires implementation
of all submessage types except for CREATE, INFO, WRITE_DATA, and DELETE,
including the associated behaviors."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: Items zu §8.3.5 (alle Submessages
außer CREATE, INFO, WRITE_DATA, DELETE), §7.8.5, §8.4.3, §8.4.14. GET_INFO ohne
Sitzung und InfoMask (65cd1820a04), die Formate von DATA (d5fbdbe672e),
`MAX_SAMPLES_ZERO` (2fa0800f55a) und vorkonfigurierte DataReader (b6e55cfa859)
sind in diesen Items geführt.

### §2 Write Access profile

**Spec:** §2, S. 12 — "Provides the clients the ability to write data on
pre-configured Topics with pre-configured QoS policies. Requires implementation
of all submessage types except for CREATE, INFO, READ_DATA, DATA, and DELETE,
including the associated behaviors."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: Items zu §8.3.5, §7.8.4, §8.4.2,
§8.4.14. STATUS auf WRITE_DATA (dc0209b38fe), die Sequenzformate von WRITE_DATA
(4f1fb9f5f4f) und vorkonfigurierte DataWriter (b6e55cfa859) sind in diesen Items
geführt.

### §2 Configure Entities profile

**Spec:** §2, S. 12 — "Provides the clients the ability define
DomainParticipant, Topic, Publisher, Subscriber, DataWriter, and DataReader
entities using pre-configured QoS policies and data-types. Requires
implementation of the CREATE_CLIENT, DELETE_CLIENT, CREATE, and DELETE
submessage and the associated behaviors."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: Items zu §7.8.2, §7.8.3, §7.7.3 und
Table 6. Eine Submessage DELETE_CLIENT gibt es in Table 10 nicht;
`delete_client` läuft über DELETE (§8.3.5.4). Die Prüfung des Participant für
Publisher und Subscriber (dd55e9c9df1) und die DDS-XML-Formen der
Repräsentationen (3aea52dd70a, 9239562286c) sind in diesen Items geführt.

### §2 Configure QoS profile

**Spec:** §2, S. 12 — "Provides client the ability to define QoS profiles to
be used by DDS entities. Requires implementation of the CREATE submessage and
the associated behaviors for object kind OBJK_QOSPROFILE."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: Items zu §7.7.3.2, Table 6
OBJK_QOSPROFILE und §8.4.7. Die DDS-XML-Form des QosProfile ist umgesetzt
(3aea52dd70a, 9239562286c); welche Policies der Agent anwendet, steht in den
Items zu §7.1.

### §2 Configure types profile

**Spec:** §2, S. 12 — "Provides client the ability to explicitly define data
types to be used for DDS Topics. Requires implementation of the CREATE
submessage and the associated behaviors for object kind OBJK_TYPE."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: Items zu §7.7.3.3, Table 6
OBJK_TYPE und §8.4.8. Der Verweis eines `<types>` auf Typen, die der Agent schon
kennt (§7.7.3.3.2), ist dort `done`.

### §2 Discovery access profile

**Spec:** §2, S. 12 — "Provides the clients the ability to discover the Topics
and Types available on the DDS Global Data Space. Requires implementation of
the GET_INFO and INFO submessage and the associated behaviors."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: Items zu §7.8.2.2, §7.8.3.3,
§8.3.5.3, §8.3.5.7 und §7.7.13. GET_INFO liefert den Agent (auch ohne Sitzung),
die Objekte des Client und die für ihn konfigurierten Objekte (65cd1820a04,
b6e55cfa859). Eine Abfrage der Topics und Typen des Global Data Space hat der
Agent nicht; in `session.rs`, `dds_port.rs` und `dcps_bridge.rs` ist kein Code
dafür.

### §2 File based configuration profile

**Spec:** §2, S. 12 — "Provides a standard way to configure the Agent using
XML files. Requires implementation of the file-based configuration mechanism
described in clause 9.3".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: Items zu §9.3 und Table 15.

### §2 UDP Transport profile

**Spec:** §2, S. 12 — "Implements the mapping of the protocol to the UDP
transport. Requires implementing the mechanisms described in clause 11.2 (UDP
Transport)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: Items zu §11.2.

### §2 TCP Transport profile

**Spec:** §2, S. 12 — "Implements the mapping of the protocol to the TCP
transport. Requires implementing the mechanisms described in clause 11.3 (TCP
Transport)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: Items zu §11.3.

### §2 Complete profile

**Spec:** §2, S. 12 — "Complete profile. Requires implementation of the
complete specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: alle Items dieses Dokuments.

---

## §3 References

### §3.1 [IETF RFC-1982] Serial Number Arithmetic

**Spec:** §3.1, S. 13 — "[IETF RFC-1982] Serial Number Arithmetic."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die Anforderung steht im Item zu
§8.3.2.3.

### §3.1 [IDL] IDL 4.2

**Spec:** §3.1, S. 13 — "[IDL] Interface Definition Language (IDL), version
4.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die Typen von Annex A sind in IDL
geschrieben und als Items zu §7.7 und Annex A geführt.

### §3.1 [DDS] DDS 1.4

**Spec:** §3.1, S. 13 — "[DDS] Data Distribution Service for Real-Time
Systems Specification, version 1.4".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; DDS-Entitäten in den Items zu
Table 6, die Syntax der Filter im Item zu Table 9.

### §3.1 [DDS-XML] DDS Consolidated XML Syntax 1.0

**Spec:** §3.1, S. 13 — "[DDS-XML] DDS Consolidated XML Syntax, version 1.0".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die XML-Formen in den Items zu
§7.7.3 und §9.3.

### §3.1 [DDS-XTYPES] XTypes 1.2

**Spec:** §3.1, S. 13 — "[DDS-XTYPES] Extensible And Dynamic Topic Types for
DDS, version 1.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; XCDR2 im Item zu §7.7.

### §3.1 [UML] 2.5

**Spec:** §3.1, S. 13 — "[UML] Unified Modeling Language, version 2.5".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur für die Diagramme.

### §3.1 [UDP] RFC 768

**Spec:** §3.1, S. 13 — "[UDP] User Datagram Protocol, IETF RFC 768".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die Abbildung in den Items zu
§11.2.

### §3.1 [TCP] RFC 793

**Spec:** §3.1, S. 13 — "[TCP] Transmission Control Protocol, STD 7, IETF RFC
793".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die Abbildung in den Items zu
§11.3.

### §3.1 [DTLS] RFC 6347

**Spec:** §3.1, S. 13 — "[DTLS] Datagram Transport Layer Security, version
1.2, IETF RFC 6347".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die Spec nennt DTLS in §11.2
(S. 99) als Möglichkeit. `crates/xrce/src/transport_dtls.rs` (Feature `dtls`)
ist im Item zu §11.2 genannt.

### §3.1 [TLS] RFC 5246

**Spec:** §3.1, S. 13 — "[TLS] The Transport Layer Security (TLS) Protocol,
version 1.2, IETF RFC 5246".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die Spec nennt TLS in §11.3
(S. 100–101) als Möglichkeit. `crates/xrce/src/transport_tls.rs` (Feature
`tls`) ist im Item zu §11.3 genannt.

### §3.1 [IETF RFC-1662] PPP in HDLC-like Framing

**Spec:** §3.1, S. 13 — "[IETF RFC-1662] PPP in HDLC-like Framing."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die Rahmung in den Items zu
Annex C.

### §3.2 [SMART] Smart Transducers 1.0

**Spec:** §3.2, S. 13 — "[SMART] Smart Transducers Specification, version
1.0".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — nicht normative Literatur.

---

## §4 Terms and Definitions

### §4 Begriffe

**Spec:** §4, S. 14 — Definitionen von "Data Distribution Service (DDS)",
"DDS Domain", "DDS DomainParticipant", "DDS Global Data Space", "GUID".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffe.

---

## §5 Symbols

### §5 Abkürzungen

**Spec:** §5, S. 15 — DDS, IDL, RTPS, XRCE.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Abkürzungen.

---

## §6 Additional Information

### §6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, S. 16 — "This specification does not change any adopted OMG
specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis.

### §6.2 Acknowledgements

**Spec:** §6.2, S. 16 — einreichende Firmen (Real-Time Innovations,
eProsima, TwinOaks Computing).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Danksagung.

---

## §7.1 General

### §7.1 Der Agent ist ein DDS-Participant

**Spec:** §7.1, S. 17 — "The XRCE Agent is a DDS Participant in the DDS Global
Data Space. The DDS-XRCE protocol allows the client to use the XRCE Agent as a
proxy in order to produce and consume data in the DDS Global Data Space."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort`,
`runtime_for`: ein `DcpsRuntime` je Domain)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** `done` — der Test schreibt und liest über einen `DomainParticipant`
von `zerodds-dcps` ohne XRCE auf der anderen Seite.

### §7.1 UML-Modell des Agent

**Spec:** §7.1, S. 17 — "this specification defines a UML model for the XRCE
Agent. This model, called the DDS-XRCE Object Model, defines the objects,
interfaces, and operations to be implemented by the agent."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; die Objekte stehen in den Items zu
§7.4 bis §7.6, die Operationen in §7.8.

### §7.1 Jedes Topic mit jeder QoS
**Spec:** §7.1, S. 17 — "Despite being simpler, the DDS-XRCE Object Model
provides XRCE clients complete access to the DDS Global Data space. Any DDS
Topic may be published or subscribed to on any DDS with any QoS."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_qos_profile`, `qos_policy`, `TopicQos`, `participant_qos_of`); `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`create_writer`, `create_reader`, `create_participant_with`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_qos_of_a_topic_is_the_default_of_its_endpoints`, `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_qos_of_an_xml_participant_is_applied`, `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_participant_qos_profile_reference_is_applied`, `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_participant_qos_is_applied_to_the_participant`

**Status:** done — jede Policy, die die DDS-Entität hat, wird angewandt: am Endpunkt RELIABILITY, DURABILITY, DURABILITY_SERVICE, HISTORY, DEADLINE, LATENCY_BUDGET, LIFESPAN, LIVELINESS, OWNERSHIP, OWNERSHIP_STRENGTH, DESTINATION_ORDER, TRANSPORT_PRIORITY, TIME_BASED_FILTER, PARTITION, USER_DATA, TOPIC_DATA, RESOURCE_LIMITS; am Publisher und Subscriber PARTITION, GROUP_DATA, PRESENTATION, ENTITY_FACTORY; am Topic als Vorgabe seiner Endpunkte (`TopicQos`); am DomainParticipant USER_DATA (eigene Runtime) und ENTITY_FACTORY, aus dem `qos_profile_reference` der Binärform und aus `<domain_participant_qos>` der XML-Form (b4cd825c268, 2c715803843). Was die Runtime nicht kann, wird nicht angenommen und ignoriert: `STATUS_ERR_INCOMPATIBLE`, es sei denn `unsupported_qos: ignore` (Einstellung für die Profildateien anderer Hersteller).
### §7.1 QoS, die der Agent nicht anwenden kann
**Spec:** §7.1, S. 17 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_not_applied`, `create_core`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_qos_profile`); `crates/xrce-agent/src/daemon/config.rs` (`unsupported_qos`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::qos_the_agent_cannot_apply_is_refused_unless_ignoring_is_named`, `crates/xrce-agent/src/daemon/config.rs::tests::unsupported_qos_is_refused_unless_ignoring_is_named`

**Status:** done — eine QoS, die der Agent nicht anwenden kann (WRITER_DATA_LIFECYCLE mit autodispose aus, READER_DATA_LIFECYCLE mit Autopurge-Verzögerung, ein Element eines Herstellerdialekts), macht das CREATE zu `STATUS_ERR_INCOMPATIBLE`; nichts wird gespeichert. `unsupported_qos: ignore` legt das Objekt ohne sie an und meldet, was fehlt (`SessionEvent::RepresentationNotApplied`). Eine Policy ohne Bedeutung für die Art des Endpunkts (TIME_BASED_FILTER eines Writers) gehört nicht zu dessen QoS und wird nicht gemeldet. 2c715803843.

---

## §7.2 XRCE Client

### §7.2 Gründe für ein eigenes Objektmodell

**Spec:** §7.2, S. 18 — drei Gründe: (1) "These clients expect a simplified
interface with no callbacks, and use parameters encoded in text." (2) "The
DDS-XRCE DDS Object Model must overcome intermittent connectivity by
introducing a 'session,' which can exist across repeated sleep-wakeup cycles by
a device." (3) "it is desirable to have an access control model that
authenticates each client application/principal".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung. Die Sitzung steht in den Items zu
§8.2.2 und §7.8.2.1, die Zugriffskontrolle im Item zu §7.4 AccessController.

### §7.2 Arten von Clients

**Spec:** §7.2, S. 19 — einfache Geräte ohne Discovery, Geräte, die "data
push" aktivieren und deaktivieren, Clients, die Objekte selbst anlegen, und
Clients, die Sequenznummern und Zeitstempel nutzen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung; das Ein- und Ausschalten der
Lieferung steht im Item zu Table 9 `max_samples`.

---

## §7.3 XRCE Agent

### §7.3 Präsenz des Client im DDS-Datenraum

**Spec:** §7.3, S. 19 — "The purpose of the DDS-XRCE Agent (XRCE Agent) is to
establish and maintain the presence of the XRCE Client in the DDS data-space."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine`,
`apply_dds_create`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::create_writer`, `create_reader`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** `done`

### §7.3 Vorkonfigurierte und vom Client angelegte Ressourcen

**Spec:** §7.3, S. 19 — "Resources may be preconfigured with well-known names,
or dynamically created by an XRCE Client."

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::predefined_objects`);
`crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions`);
`crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::ensure_predefined`,
`make_client_applications`); `crates/xrce-agent/src/daemon/config.rs` (Schlüssel
`xml_profile_files`, `client_applications`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_reference_is_read_like_the_profile_it_names`

**Status:** `done` — vom Client angelegte Ressourcen: CREATE. Konfigurierte
Ressourcen: die Dateien nach §9.3 (`<types>`, `<qos_library>`,
`<domain_library>`, `<domain_participant_library>`, `<application_library>`) aus
`xml_profile_files:` werden unter der ObjectId aus dem MD5 ihres Namens
bereitgestellt (03371b4965c, dc63d65691f, 3aea52dd70a, b6e55cfa859).

### §7.3 Schreiben über eine vorkonfigurierte Ressource ohne CREATE

**Spec:** §7.3, S. 19 — "a Client that has access to this resource can write
data using this resource simply by referring to the existing 'MySquareWriter'.
The Client does not have to create a resource."

**Repo:** `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::ensure_predefined`, aufgerufen von `handle_write_data`,
`handle_read_data`, `handle_get_info` und `handle_create`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_reads_through_a_preconfigured_data_reader_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_preconfigured_object_is_made_only_for_a_client_allowed_to_create_it`

**Status:** `done` — WRITE_DATA auf die ObjectId eines konfigurierten DataWriter
legt den DomainParticipant mit allem darin für den Client an (b6e55cfa859); die
Zugriffsregeln gelten dabei. Eine ObjectId ohne Konfiguration ergibt
`STATUS_ERR_UNKNOWN_REFERENCE`.

### §7.3 Abfrage durch den Client

**Spec:** §7.3, S. 19 — "This specification enables Clients to be in charge of
when data is received, and to request the XRCE Agent to return data that
matches a set of constraints. [...] an XRCE Client queries the XRCE Agent when
it wakes up."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`pump_read`); `crates/xrce-agent/src/lib.rs` (`XrceAgent::pull_sample`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_elapsed_time_is_measured_from_the_real_request_time`,
`crates/xrce-agent/src/lib.rs::tests::after_submit_pull_returns_sample_in_fifo_order`

**Status:** `done`

### §7.3 DDS-RTPS zu den übrigen Participants

**Spec:** §7.3, S. 20 — "the standard interactions occurring with the XRCE
Client use the DDS-XRCE protocol, while interactions with other DDS domain
participants use the DDS-RTPS protocol."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** `done`

### §7.3 Geteilte oder eigene DDS-Entitäten

**Spec:** §7.3, S. 19 — "a resource 'MySquareWriter' may be associated with a
DDS DataWriter shared by many DDS-XRCE clients, or an XRCE Client may have its
own dedicated 'MySquareWriter'".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Freiheit der Implementierung; die Pflicht zum
Verhalten „als ob“ steht im Item zu §10.3.

---

## §7.4 Model Overview

### §7.4 Root als Singleton und Fabrik

**Spec:** §7.4, S. 21 — "The Root singleton is the entry point for the service.
It functions as a factory for all the Objects managed by the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine`);
`crates/xrce-agent/src/lib.rs` (`XrceAgent`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`

**Status:** `done`

### §7.4 ProxyClient

**Spec:** §7.4, S. 21 — "The ProxyClient class models the XRCE Client
application that interacts with the XRCE Agent using the XRCE protocol."

**Repo:** `crates/xrce-agent/src/lib.rs` (`XrceAgent::register_client`, ein
`ObjectStore` je `ClientKey`)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::multiple_clients_isolated`

**Status:** `done`

### §7.4 Application gehört zu einem ProxyClient

**Spec:** §7.4, S. 21 — "Each Application object is associated with a single
XRCE ProxyClient and gets its access rights from those assigned to the XRCE
Client."

**Repo:** `crates/xrce-agent/src/lib.rs` (`XrceAgent::create_object`);
`crates/xrce-agent/src/daemon/access.rs` (`AccessController::permits`, Regeln je
ClientKey, ObjectKind `application`); `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::permit`)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::create_application_object_via_objk_application`,
`crates/xrce-agent/src/lib.rs::tests::multiple_clients_isolated`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_without_permission_is_denied_and_stores_nothing`

**Status:** `done` — eine Application liegt im Objektspeicher ihres Client; ihre
Rechte sind die Regeln des Client im AccessController (d66ada44616,
515bcc677f3). Aus dem Code gelesen: die Prüfung nimmt den ClientKey und den
ObjectKind des Objekts; ein Test mit einer Application als verweigertem Objekt
gibt es nicht.

### §7.4 Application mit DomainParticipants

**Spec:** §7.4, S. 21 — "The Application class models a software application
that connects with the XRCE Client and manages the DDS objects needed to
publish and subscribe data on one or more DDS Domains. An XRCE Application can
be associated with zero of more DomainParticipant objects."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create` gibt
`DdsCreate::Application` zurück, `create_nested` legt die DomainParticipants an,
je unter der ObjectId ihres Namens);
`crates/xrce-agent/src/daemon/definitions.rs` (`ApplicationDef`,
`ParticipantDef`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_creates_the_objects_inside_it`

**Status:** `done` — eine Application aus `<application_library>` legt ihre
DomainParticipants mit allem darin an, auch für einen Client, dem sie in
`client_applications:` zugeordnet ist (3aea52dd70a, b6e55cfa859).

### §7.4 AccessController

**Spec:** §7.4, S. 21 — "The AccessController is responsible for making
decisions regarding the resources and operations a particular XRCE ProxyClient
is allowed to perform. It contains rules that associate a Client with
privileges, which determine which DDS domain an application executing on
behalf of a client may join, which DDS Topics the application can read and
write, and so on."

**Repo:** `crates/xrce-agent/src/daemon/access.rs` (`AccessController`, `Rule`,
`Subject`, `Operation`); `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::permit`, `permits`, `deny_create_client`);
`crates/xrce-agent/src/daemon/config.rs` (Schlüssel `authorized_client_keys`,
`required_client_properties`, `access_rules`)

**Tests:** `crates/xrce-agent/src/daemon/access.rs::tests::a_controller_without_rules_restricts_nothing`,
`crates/xrce-agent/src/daemon/access.rs::tests::the_first_matching_rule_decides_and_no_match_denies`,
`crates/xrce-agent/src/daemon/access.rs::tests::domains_kinds_and_topic_prefixes_narrow_a_rule`,
`crates/xrce-agent/src/daemon/access.rs::tests::only_configured_client_keys_are_authorized`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_key_not_configured_in_the_agent_is_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_without_permission_is_denied_and_stores_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::write_and_read_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_rule_can_limit_the_domains_a_client_joins`

**Status:** `done` — Regeln je Client für Operation, ObjectKind, DDS-Domain und
Topic; eine Operation ohne passende Regel wird mit `STATUS_ERR_DENIED`
beantwortet, sobald eine Regel existiert, ohne Regeln beschränkt der Controller
nichts (d66ada44616, 515bcc677f3).

### §7.4 DomainParticipant als Proxy

**Spec:** §7.4, S. 21 — "The DDS-XRCE DomainParticipant is a proxy for a DDS
DomainParticipant and models the association with a DDS domain and the
capability of the Application to publish and subscribe to Topics on that
domain."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::create_participant_with`, `runtime_for`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::full_chain_against_a_real_leased_domain`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::two_large_samples_reach_a_default_reader_on_the_same_agent`

**Status:** `done` — alle XRCE-DomainParticipants einer Domain teilen einen
`DcpsRuntime`, ausgenommen ein Participant mit USER_DATA (eigener Runtime je
Domain und Wert); §10.3 erlaubt das, solange sich das Verhalten so zeigt, als
hätte jeder Client eigene Entitäten.

---

## §7.5 XRCE DDS Proxy Objects

### §7.5 Proxy-Objekte delegieren an DDS-Objekte

**Spec:** §7.5, S. 22 — "These proxy objects allow the client application to
participate as first-class citizens on the DDS network by delegating the actual
DDS behavior and DDS-RTPS protocol implementation to the proxy DDS objects."
Figure 5: DomainParticipant, Publisher, Subscriber, DataWriter, DataReader,
Topic, QosProfile und Qos als Werte.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`apply_dds_create`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::create_participant_with`, `create_publisher`,
`create_subscriber`, `create_writer`, `create_reader`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_publisher_is_a_dds_entity_of_its_participants_domain`

**Status:** `done` — DataWriter und DataReader sind DDS-Endpunkte im
`DcpsRuntime`, Publisher und Subscriber haben je eine eigene DDS-Entität im
Participant (dd55e9c9df1), das Topic liegt als Name und Typ im Zustand des
`RealDdsPort`, QosProfile und Qos gehen als Werte ein.

### §7.5 Publisher und Subscriber als DDS-Entität

**Spec:** §7.5, S. 22 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/dds_port.rs`
(`DdsPort::create_publisher`, `create_subscriber`, `ContainerSpec`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::create_container`,
`container_for`); `crates/xrce-agent/src/daemon/session.rs`
(`DdsCreate::Container`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_publisher_is_a_dds_entity_of_its_participants_domain`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_and_a_subscriber_get_a_dds_entity_with_their_policies`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_or_subscriber_needs_an_existing_participant`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::publisher_and_subscriber_need_their_participant_and_endpoints_need_them`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::a_publisher_that_does_not_autoenable_refuses_its_writers`

**Status:** `done` — Publisher und Subscriber sind eine Gruppen-Entität im
Participant ihrer Domain; PRESENTATION und ENTITY_FACTORY aus den XML-Formen
wirken (ENTITY_FACTORY `autoenable_created_entities: false` verweigert die
Endpunkte darunter), PARTITION und GROUP_DATA erreichen die Endpunkte über deren
`EndpointQos`. Ein Participant, der nicht existiert, ergibt
`STATUS_ERR_UNKNOWN_REFERENCE` (dd55e9c9df1). GROUP_DATA wird mit dem Endpunkt
angekündigt, nicht mit einer eigenen Ankündigung des Publisher.

---

## §7.6 XRCE Object Identification

### §7.6 ObjectId eindeutig je Agent und ClientKey

**Spec:** §7.6, S. 22 — "This implies that the ObjectId shall be unique within
the scope of an Agent and a ClientKey."

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore`);
`crates/xrce-agent/src/lib.rs` (`XrceAgent::register_client`, ein Objektspeicher
je `ClientKey`)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_strict_on_existing_returns_conflict`,
`crates/xrce-agent/src/lib.rs::tests::multiple_clients_isolated`

**Status:** `done`

### §7.6 ObjectId im Agent konfiguriert oder vom Client gewählt

**Spec:** §7.6, S. 22 — "The value of the ObjectId for a particular object
shall be configured on the XRCE Agent or specified by the XRCE Client at the
time the object is created."

**Repo:** `crates/xrce/src/object_id.rs` (`ObjectId::from_reference`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create`, `ensure_predefined`)

**Tests:** `crates/xrce/src/object_id.rs::tests::from_reference_matches_the_examples_of_the_spec`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_object_id_is_the_prefix_and_the_kind_of_the_representation`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done` — die vom Client gewählte `ObjectId` wird übernommen; die im
Agent konfigurierte `ObjectId` ist die aus dem MD5 des Namens nach §9.3 und
Table 16 (03371b4965c, b6e55cfa859).

### §7.6 OBJECTID_INVALID {0x00, 0x00}

**Spec:** §7.6, S. 23 — "The value {0x00, 0x00} is referred as
OBJECTID_INVALID and represents an invalid object."

**Repo:** `crates/xrce/src/object_id.rs` (`OBJECTID_INVALID` = 0x0000,
`OBJECTID_SESSION` = 0xFFFF); `crates/xrce/src/object_store.rs`
(`ObjectStore::create` weist `OBJECTID_INVALID` ab)

**Tests:** `crates/xrce/src/object_id.rs::tests::invalid_object_id_is_all_zeros`,
`crates/xrce/src/object_id.rs::tests::reserved_object_ids_per_spec_7_6_and_annex_a`,
`crates/xrce/src/object_store.rs::tests::create_invalid_object_id_rejected`

**Status:** `done` — `OBJECTID_INVALID` ist `{0x00, 0x00}`, `0xFFFF` ist
`OBJECTID_SESSION` aus Annex A (f63fa2d1218); die Konstanten der SDKs in
`endpoints/` folgen (5530a71bb29).

### §7.6 OBJECTID_CLIENT {0xFF, 0xFE}

**Spec:** §7.6, S. 23 — "The value {0xFF, 0xFE} is referred as OBJECTID_CLIENT
and represents the XRCE ProxyClient object."

**Repo:** `crates/xrce/src/object_id.rs` (`OBJECTID_CLIENT`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_delete`)

**Tests:** `crates/xrce/src/object_id.rs::tests::client_singleton_has_kind_client`,
`crates/xrce-client/src/lib.rs::tests::disconnect_sends_delete_client_and_clears_state`

**Status:** `done`

### §7.6 Zwei reservierte Werte

**Spec:** §7.6, S. 23 — "There are two reserved values for ObjectId."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Annex A (S. 103) nennt vier Werte
(`OBJECTID_INVALID`, `OBJECTID_AGENT`, `OBJECTID_CLIENT`, `OBJECTID_SESSION`);
die beiden zusätzlichen sind als Items zu Annex A geführt.

### §7.6 Name der Ressource statt ObjectId

**Spec:** §7.6, S. 23 — "Alternatively, objects may also be identified by a
string resourceName. The format of this name depends on the resource and
provides a way to refer to a resource configured on the agent using a
configuration file or similar means."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`referenced`);
`crates/xrce-agent/src/daemon/profiles.rs` (`AgentProfiles`);
`crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_is_created_by_name_or_by_register_type_ref`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_reference_is_read_like_the_profile_it_names`

**Status:** `done` — ein Name löst sich gegen die Dateien des Agent auf:
`profile_name` einer Profildatei und die Namen nach DDS-XML (`Library::Element`,
Table 15) der Bibliotheken von §9.3 (dc63d65691f, 3aea52dd70a).

---

## §7.7 Data types used to model operations on XRCE Objects

### §7.7 IDL nach Annex A, Kodierung XCDR2

**Spec:** §7.7, S. 23 — "The IDL definitions for the data types shall be as
specified in Annex A IDL Types. When serializing these types into a binary
representation the encoding shall follow the rules defined in in
[DDS-XTYPES] for XCDR version 2 encoding."

**Repo:** `crates/xrce/src/` (Codecs je Typ, siehe die folgenden Items;
`encoding.rs`: `UnannotatedTypes`, `RepresentationTimestamps`);
`crates/xrce-agent/src/daemon/binary_forms.rs` (XCDR2-Leser der Binärformen)

**Tests:** `crates/xrce/tests/foreign_frames.rs::every_client_datagram_decodes_and_writes_back`,
`crates/xrce/tests/foreign_frames.rs::every_agent_datagram_decodes_and_writes_back`,
`crates/xrce/tests/unannotated_types.rs::the_default_is_appendable_with_a_dheader`

**Status:** `done` — die Payloads der Submessages und die Typen von Annex A
werden nach XCDR2 gelesen und geschrieben. Typen ohne `@extensibility` sind
APPENDABLE mit DHEADER; die FINAL-Form der Referenzimplementierung ist die
Einstellung `unannotated_types: final` (f2ace917b5a). Erweiterungen über Annex A
hinaus stehen in den Items der einzelnen Typen, darunter `requested_mtu` hinter
`properties` der CLIENT_Representation. Die klassische CDR-Form der Binärobjekte
des eProsima-Client bleibt als Rückfall (`entity_repr.rs`, `Cdr`).

### §7.7 Ausrichtung, DHEADER und Byte-Reihenfolge der Annex-A-Typen

**Spec:** §7.7, S. 23 — wie oben.

**Repo:** `crates/xrce/src/handshake.rs` (`encode_properties`,
`decode_properties`); `crates/xrce/src/transport_locator.rs`;
`crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/object_info.rs`;
`crates/xrce-agent/src/daemon/binary_forms.rs`

**Tests:** `crates/xrce/src/handshake.rs::tests::padding_counts_from_the_start_of_the_payload`,
`crates/xrce/src/handshake.rs::tests::a_big_endian_client_representation_with_properties_reads_back`,
`crates/xrce/src/transport_locator.rs::tests::padding_is_counted_from_the_start_of_the_payload`,
`crates/xrce/src/transport_locator.rs::tests::by_default_the_union_and_its_case_have_a_dheader`,
`crates/xrce/src/submessages/read_data.rs::tests::the_content_filter_is_a_string_with_a_nul_and_an_aligned_length`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`,
`crates/xrce/src/object_info.rs::tests::agent_activity_info_has_a_dheader_and_a_locator_sequence`,
`crates/xrce/src/object_info.rs::tests::object_info_puts_the_activity_before_the_config`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::annex_a_topic_has_a_dheader`,
`crates/xrce/tests/unannotated_types.rs::both_forms_are_read_in_either_setting`

**Status:** `done` — die Abweichungen sind behoben: `PropertySeq` mit auf 4
ausgerichteten Längen (65cd1820a04); `TransportLocator` mit Ausrichtung, DHEADER
und numerischen Ports (c304e509660); `content_filter_expression` als
XCDR2-String und `DataDeliveryControl` mit DHEADER (1663f8f6240);
`AGENT_ActivityInfo`, `DATAREADER_ActivityInfo`, `DATAWRITER_ActivityInfo`,
`ObjectInfo` und `ActivityInfoVariant` nach Annex A (65cd1820a04); die
Binärformen von Annex A als XCDR2 mit DHEADER (c67ce23c724);
`CLIENT_Representation`, `AGENT_Representation`, `ObjectInfo` und die Locatoren
in der Byte-Reihenfolge des E-Flag (b43d5e6b848).

---

## §7.7.1 Data and Samples

### §7.7.1 Fünf Formate und die Werte von DataFormat

**Spec:** §7.7.1, S. 23 — "The five possible representation are: SampleData,
Sample, SampleDataSeq, SampleSeq, and SamplePackedData. They respectively
correspond to the DataFormat values FORMAT_DATA, FORMAT_DATA_SEQ,
FORMAT_SAMPLE, FORMAT_SAMPLE_SEQ, and FORMAT_PACKED."

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`DataFormat`,
`AnyWriteData`); `crates/xrce/src/submessages/data.rs` (`AnyData`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::any_write_data_dispatches_every_format`,
`crates/xrce/src/submessages/data.rs::tests::any_data_dispatches_every_format`,
`crates/xrce/src/submessages/read_data.rs::tests::the_data_format_octet_has_the_values_of_annex_a`

**Status:** `done` — der Code ordnet wie Table 1 und Annex A zu (`SampleDataSeq`
zu FORMAT_DATA_SEQ, `Sample` zu FORMAT_SAMPLE; Werte 0x00, 0x02, 0x08, 0x0A,
0x0E). Die Aufzählung „respectively“ dieses Satzes ordnet `Sample`
FORMAT_DATA_SEQ zu und nennt `SamplePackedData` und `FORMAT_PACKED`, die es in
Annex A nicht gibt.

### §7.7.1 SampleData

**Spec:** §7.7.1, S. 23 — "@extensibility(FINAL) struct SampleData {
XCDRSerializedBuffer serialized_data; }; In this structure the
XCDRSerializedBuffer represents the bytes resulting from serializing the
application-specific data type that is being sent using the XCDR version 2
rules defined in clause 7.4 of [DDS-XTYPES]."

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayload::encode_body`, `try_from_submessage`);
`crates/xrce/src/submessages/data.rs` (`DataPayload`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_matches_the_annex_b_example`,
`crates/xrce/src/submessages/data.rs::tests::data_base_object_request_precedes_the_sample`

**Status:** done — `serialized_data` läuft ohne eigene Länge bis zum Ende der
Submessage, wie in Annex B.3.1 (`submessageLength = 6`). Annex A definiert
`XCDRSerializedBuffer` nicht.

### §7.7.1 SampleInfoFlags

**Spec:** §7.7.1, S. 23 — "@bit_bound(8) bitmask SampleInfoFlags {
@position(0) INSTANCE_STATE_UNREGISTERED, @position(1)
INSTANCE_STATE_DISPOSED, @position(2) VIEW_STATE_NEW, @position(3)
SAMPLE_STATE_READ, };"

**Repo:** `crates/xrce/src/sample_info.rs` (`SampleInfoFlags`)

**Tests:** `crates/xrce/src/sample_info.rs::tests::sample_info_flags_bit_positions`,
`crates/xrce/src/sample_info.rs::tests::sample_info_flags_all_four_bits_independent`

**Status:** done

### §7.7.1 SampleInfo mit sequence_number und session_time_offset

**Spec:** §7.7.1, S. 24 — "@extensibility(FINAL) struct SampleInfo {
SampleInfoFlags state; unsigned long sequence_number; unsigned long
session_time_offset; // milliseconds up to 53 days };"

**Repo:** `crates/xrce/src/sample_info.rs` (`SampleInfo`,
`WIRE_SIZE = 12`)

**Tests:** `crates/xrce/src/sample_info.rs::tests::sample_info_matches_annex_b_5_3_layout`,
`crates/xrce/src/sample_info.rs::tests::sample_info_roundtrip_be`

**Status:** done — `state`, 3 Byte Füllung, zwei `unsigned long`. Annex A
(S. 115) definiert `SampleInfo` anders; das ist ein eigenes Item zu Annex A.

### §7.7.1 Sample

**Spec:** §7.7.1, S. 24 — "@extensibility(FINAL) struct Sample { SampleInfo
info; SampleData data; };"

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayloadSample`); `crates/xrce/src/submessages/data.rs`
(`DataPayloadSample`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_sample_matches_annex_b_5_3_layout`,
`crates/xrce/src/submessages/data.rs::tests::data_sample_matches_annex_b_5_3_layout`

**Status:** done

### §7.7.1 DeciSecond und SampleInfoDelta

**Spec:** §7.7.1, S. 24 — "typedef unsigned short DeciSecond; // 10e-1
seconds @extensibility(FINAL) struct SampleInfoDelta { SampleInfoFlags state;
octet seq_number_delta; DeciSecond timestamp_delta; // In 1/10 of seconds };"

**Repo:** `crates/xrce/src/sample_info.rs` (`SampleInfoDelta`,
`WIRE_SIZE = 4`)

**Tests:** `crates/xrce/src/sample_info.rs::tests::sample_info_delta_roundtrip`,
`crates/xrce/src/sample_info.rs::tests::sample_info_delta_decode_short_is_eof`

**Status:** done

### §7.7.1 SampleDelta

**Spec:** §7.7.1, S. 24 — "@extensibility(FINAL) struct SampleDelta {
SampleInfoDelta info_delta; SampleData data; };"

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayloadPackedSamples::elements`); `crates/xrce/src/sample_info.rs`
(`SampleInfoDelta::decode`); `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::split_write_items`, `SessionEngine::data_submessages`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_packed_samples_matches_annex_b_5_5_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::packed_samples_resolve_their_deltas_against_the_base`

**Status:** done — das Layout je Element ist abgebildet; im Codec bleiben die Elemente
ein Block (`elements`), weil `SampleData` keine Länge trägt. Der Agent trennt
sie beim WRITE_DATA nach dem Typ des Topic (`split_write_items`, 4f1fb9f5f4f)
und schreibt sie beim READ_DATA (`data_submessages`, d5fbdbe672e).

### §7.7.1 PackedSamples

**Spec:** §7.7.1, S. 24 — "@extensibility(FINAL) struct PackedSamples {
SampleInfo info_base; sequence<SampleDelta> sample_delta_seq; };"

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayloadPackedSamples`); `crates/xrce/src/submessages/data.rs`
(`DataPayloadPackedSamples`)

**Tests:** `crates/xrce/src/submessages/data.rs::tests::data_packed_samples_matches_annex_b_5_5_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_packed_samples_is_answered_with_one_packed_samples`,
`crates/xrce-agent/src/daemon/session.rs::tests::packed_samples_resolve_their_deltas_against_the_base`

**Status:** done — der Agent liest die Form beim WRITE_DATA (4f1fb9f5f4f) und schreibt sie
beim READ_DATA (d5fbdbe672e).

---

## §7.7.2 DataRepresentation

### §7.7.2 DataRepresentation als Union über DataFormat

**Spec:** §7.7.2, S. 24 und S. 26 — "The DataRepresentation is defined as a
union discriminated by a DataFormat. Depending on the discriminator it selects
one of the formats defined in clause 7.7.1." "union DataRepresentation
switch(DataFormat) { case FORMAT_DATA: SampleData data; case FORMAT_SAMPLE:
Sample sample; case FORMAT_DATA_SEQ: SampleDataSeq data_seq; case
FORMAT_SAMPLE_SEQ: SampleSeq sample_seq; case FORMAT_PACKED_SAMPLES:
PackedSamples packed_samples; };"

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`AnyWriteData`,
Diskriminator aus den Flags über `DataFormat::from_flag_bits`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::any_write_data_dispatches_every_format`

**Status:** done — der Diskriminator steht in den Flags der Submessage
(§8.3.5.8.1), nicht im Payload.

### §7.7.2 Table 1 FORMAT_DATA

**Spec:** §7.7.2, Table 1, S. 25 — "FORMAT_DATA struct SampleData [...]
Contains the data for a single sample without additional sample information."

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`WriteDataPayload`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_roundtrip_format_data`

**Status:** done

### §7.7.2 Table 1 FORMAT_DATA_SEQ

**Spec:** §7.7.2, Table 1, S. 25 — "FORMAT_DATA_SEQ struct SampleDataSeq [...]
Contains a sequence of data samples."

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayloadDataSeq`); `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::split_write_items`, `SessionEngine::data_submessages`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_data_seq_matches_annex_b_5_2_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_data_seq_is_split_by_the_type_and_written_sample_by_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_data_seq_is_answered_with_one_data_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_sequence_formats_split_at_the_size_of_a_submessage`

**Status:** done — Zahl der Elemente und die Elemente als ein Block. Der Agent trennt die
Elemente beim WRITE_DATA nach dem Typ des Topic und schreibt jedes einzeln
(4f1fb9f5f4f); beim READ_DATA schreibt er eine DATA_SEQ je Submessage
(d5fbdbe672e). Annahme, nicht gemessen: die Elemente stehen ohne Füllung
hintereinander, jedes ab Ausrichtungsursprung 0 gelesen (Annex B.5.2 zeigt nur
Elemente von 4 Byte).

### §7.7.2 Table 1 FORMAT_SAMPLE mit Zeitstempel relativ zur Sitzung
**Spec:** §7.7.2, Table 1, S. 25 — "The SampleInfo holds the DDS
InstanceState, SampleState, and ViewState of the corresponding DDS Sample. It
also contains the sample sequence number and timestamp. The timestamp is
represented as an offset relative to the session timestamp established when
the session was created. The session timestamp corresponds to the
client_timestamp attribute in CLIENT_Representation".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`data_submessages`, `source_offset_ms`, `handle_write_data`); `crates/xrce/src/sample_info.rs` (`SampleInfo`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_sample_info_is_what_the_datareader_gave`, `crates/xrce-agent/src/daemon/session.rs::tests::the_client_timestamp_is_the_session_timestamp`

**Status:** done — Zustand, Sequenznummer und Versatz werden übertragen. Beim WRITE_DATA ist der Versatz die Zeit nach dem Beginn der Sitzung und wird zum Quellzeitstempel (4f1fb9f5f4f). Beim READ_DATA sendet der Agent FORMAT_SAMPLE mit dem Quellzeitstempel des Writers als Versatz und dem Zustand des DataReaders (8be8c6101ca). Der Beginn der Sitzung ist das `client_timestamp` des CREATE_CLIENT (0f32e029a31).
### §7.7.2 Inhalt der SampleInfo und Bezugspunkt des Zeitstempels
**Spec:** §7.7.2, Table 1, S. 25 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/dds_port.rs` (`PolledSample`, `DdsPort::poll_samples`); `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`spawn_reader_pump`, `ReaderQueue::push`); `crates/xrce-agent/src/daemon/session.rs` (`data_submessages`, `session_start`); `crates/xrce-client/src/lib.rs` (`session_time_offset_ms`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_sample_info_is_what_the_datareader_gave`, `crates/xrce-agent/src/daemon/session.rs::tests::the_client_timestamp_is_the_session_timestamp`

**Status:** done — `DdsPort::poll_samples` liefert `PolledSample` (Nutzdaten, `SampleInfoFlags`, Quellzeitstempel): `VIEW_STATE_NEW` für das erste Sample einer Instanz, `INSTANCE_STATE_DISPOSED`/`_UNREGISTERED` für ein dispose oder unregister, das der DataReader meldet (ein Sample ohne Daten), `SAMPLE_STATE_READ` nie (ein Sample wird einmal übergeben). FORMAT_DATA und FORMAT_DATA_SEQ haben keine SampleInfo und senden keine Marker. Bezugspunkt ist das `client_timestamp` (`representation_timestamps: present`) oder die Uhr des Agent; der Client schreibt die Millisekunden seit `connect` als Versatz. 8be8c6101ca, 0f32e029a31.
### §7.7.2 Table 1 FORMAT_SAMPLE_SEQ

**Spec:** §7.7.2, Table 1, S. 25 — "FORMAT_SAMPLE_SEQ struct SampleSeq [...]
Contains a sequence of samples, each containing both the data and the
additional sample information."

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayloadSampleSeq`); `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::split_write_items`, `SessionEngine::data_submessages`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_sample_seq_matches_annex_b_5_4_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_seq_carries_state_and_time_per_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_seq_is_answered_with_one_sample_seq`

**Status:** done — der Agent liest die Form beim WRITE_DATA (Zustand und Zeit je Sample,
4f1fb9f5f4f) und schreibt sie beim READ_DATA (d5fbdbe672e). Was die
SampleInfo beim Lesen enthält, steht im `open`-Item zu §7.7.2.

### §7.7.2 Table 1 FORMAT_PACKED_SAMPLES und resulting_info

**Spec:** §7.7.2, Table 1, S. 25 — "This representation is limited to samples
that are close in sequence number (no more than 256 apart) and timestamp (100
minutes). [...] resulting_info.state := info_delta.state
resulting_info.sequence_number := info_base.sequence_number +
info_delta.seq_number_delta resulting_info. session_time_offset :=
info_base. session_time_offset + info_delta.timestamp_delta"

**Repo:** `crates/xrce/src/sample_info.rs` (`SampleInfoDelta::resolve`);
`crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::data_submessages`)

**Tests:** `crates/xrce/src/sample_info.rs::tests::sample_info_delta_resolve_adds_to_base`,
`crates/xrce/src/sample_info.rs::tests::sample_info_delta_resolve_wraps_on_overflow`,
`crates/xrce-agent/src/daemon/session.rs::tests::packed_samples_resolve_their_deltas_against_the_base`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_packed_samples_is_answered_with_one_packed_samples`

**Status:** done — `resolve` rechnet `timestamp_delta` (Zehntelsekunden) mit 100 in
Millisekunden um; die Formel der Spec addiert Größen in Millisekunden und
Zehntelsekunden ohne Umrechnung. Der Agent liest die Form beim WRITE_DATA und
schreibt sie beim READ_DATA mit höchstens 256 Elementen je Submessage (Bereich
von `seq_number_delta`, ein `octet`); `timestamp_delta` und `state` schreibt er
als 0 (4f1fb9f5f4f, d5fbdbe672e).

---

## §7.7.3 ObjectVariant

### §7.7.3 ObjectVariant als Union über ObjectKind

**Spec:** §7.7.3, S. 26 — "The ObjectVariant type is defined as a union
discriminated by ObjectKind. Each value of the discriminator selects an
appropriate object representation for that kind."

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::encode_for_kind`, `decode_for_kind`, `ObjectVariantExtra`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`,
`crates/xrce/src/object_repr.rs::tests::participant_representation_matches_the_captured_create_bytes`,
`crates/xrce/src/object_repr.rs::tests::subscriber_representation_matches_the_captured_create_bytes`

**Status:** done — `ObjectKind` als Oktett, danach die Repräsentation der Art
und ihr nachgestelltes Feld (`domain_id` oder die `ObjectId` des Containers).

### §7.7.3 Mehrere Formate je Art

**Spec:** §7.7.3, S. 26 — "For a given ObjectKind the, ObjectVariant type
also supports multiple representation formats. Each format is identified by a
value of the RepresentationFormat."

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant`,
`encode_with_pad`, `decode_with_pad`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::by_reference_roundtrip_le`,
`crates/xrce/src/object_repr.rs::tests::by_xml_string_roundtrip_be`,
`crates/xrce/src/object_repr.rs::tests::in_binary_roundtrip`

**Status:** done

---

## §7.7.3.1 Object Representation Formats

### §7.7.3.1 Drei Werte von RepresentationFormat

**Spec:** §7.7.3.1, S. 26 — "There are three RepresentationFormat values:
REPRESENTATION_BY_REFERENCE, REPRESENTATION_AS_XML_STRING, and
REPRESENTATION_IN_BINARY." Annex A, S. 106: 0x01, 0x02, 0x03.

**Repo:** `crates/xrce/src/object_repr.rs` (`repr_disc`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::discriminator_returns_correct_byte`,
`crates/xrce/src/object_repr.rs::tests::unknown_discriminator_rejected`

**Status:** done

### §7.7.3.1 Drei oder zwei Formate je Art

**Spec:** §7.7.3.1, S. 26 — "Some object kinds support all three formats; in
this case the corresponding representation extends the type struct
OBJK_Representation3_Base. Other object kinds support only two formats, and
therefore, extend the type struct OBJK_RepresentationRefAndXML_Base or the type
OBJK_RepresentationBinAndXML_Base."

**Repo:** `crates/xrce/src/object_repr.rs` (`decode_for_kind`); `crates/xrce-agent/src/daemon/session.rs`
(`format_is_allowed`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`

**Status:** done — `decode_for_kind` liest alle drei Diskriminatoren; der Agent lässt je
Art nur die Formate der Spec zu: Participant und Topic alle drei, Type,
QosProfile und Application Referenz und XML, Publisher, Subscriber, DataWriter
und DataReader Binär und XML. Ein anderes Format ist STATUS_ERR_INVALID_DATA
(bdf3f6ee966). Die Einstellung steht im Item „Erlaubte Formate je Art“.

### §7.7.3.1 Erlaubte Formate je Art

**Spec:** §7.7.3.1, S. 26 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`format_is_allowed`, Prüfung in `SessionEngine::handle_create`);
`crates/xrce-agent/src/daemon/config.rs` (Schlüssel `representation_formats`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`,
`crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_representation_formats_can_be_widened`

**Status:** done — normative Lesart (§7.7.3.1, Annex A) als Vorgabe: ein CREATE in einem
Format, das die Union der Art nicht hat, ist STATUS_ERR_INVALID_DATA mit einem
Ereignis `CreateRefused`. Die andere Lesart ist die Einstellung
`representation_formats: spec | any` (Vorgabe `spec`); `any` lässt alle Formate
zu, etwa für den Micro-XRCE-DDS-Client, der Publisher, Subscriber, DataWriter
und DataReader per `*_ref` anlegt (nicht gemessen, der Mitschnitt hat nur XML
und Binär). Beide Lesarten sind getestet (bdf3f6ee966).

### §7.7.3.1 OBJK_Representation3Formats

**Spec:** §7.7.3.1, S. 26–27 — "union OBJK_Representation3Formats
switch(RepresentationFormat) { case REPRESENTATION_BY_REFERENCE :
string<REFERENCE_MAX_LEN> object_reference case
REPRESENTATION_AS_XML_STRING : string xml_string_representation; case
REPRESENTATION_IN_BINARY : sequence<octet> binary_representation; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant`,
`encode_with_pad`: Länge, Zeichen und NUL für die Zeichenketten, Länge
und Oktette für die Sequenz)

**Tests:** `crates/xrce/src/object_repr.rs::tests::topic_representation_matches_the_captured_create_bytes`,
`crates/xrce/src/object_repr.rs::tests::in_binary_roundtrip`

**Status:** done — die Grenze `REFERENCE_MAX_LEN` ist ein eigenes Item.

### §7.7.3.1 OBJK_RepresentationRefAndXMLFormats

**Spec:** §7.7.3.1, S. 27 — "union OBJK_RepresentationRefAndXMLFormats
switch(RepresentationFormat) { case REPRESENTATION_BY_REFERENCE :
string<REFERENCE_MAX_LEN> object_reference; case REPRESENTATION_AS_XML_STRING
: string string_representation; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::ByReference`,
`ByXmlString`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::object_variant_xml_form_supports_topic_qosprofile_application`

**Status:** done — die Kodierung der beiden Fälle; dass `IN_BINARY` hier nicht
vorkommen darf, steht im `open`-Item „Erlaubte Formate je Art nicht geprüft“.

### §7.7.3.1 OBJK_RepresentationBinAndXMLFormats

**Spec:** §7.7.3.1, S. 27 — "union OBJK_RepresentationBinAndXMLFormats
switch(RepresentationFormat) { case REPRESENTATION_IN_BINARY :
sequence<octet> binary_representation; case REPRESENTATION_AS_XML_STRING :
string string_representation; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::InBinary`,
`ByXmlString`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::subscriber_representation_matches_the_captured_create_bytes`

**Status:** done — wie oben für `BY_REFERENCE`.

### §7.7.3.1 Basistypen mit dem Member representation

**Spec:** §7.7.3.1, S. 27 — "struct OBJK_RepresentationRefAndXML_Base {
OBJK_RepresentationRefAndXMLFormats representation; }; struct
OBJK_RepresentationBinAndXML_Base { OBJK_RepresentationBinAndXMLFormats
representation; }; struct OBJK_Representation3_Base {
OBJK_Representation3Formats representation; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`encode_for_kind`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`

**Status:** done

### §7.7.3.1 REFERENCE_MAX_LEN = 128

**Spec:** §7.7.3.1, S. 26 — "const long REFERENCE_MAX_LEN = 128;" mit
`string<REFERENCE_MAX_LEN> object_reference`.

**Repo:** `crates/xrce/src/object_repr.rs` (`REFERENCE_MAX_LEN`, Prüfung in
`ObjectVariant::encode_with_pad` und `decode_with_pad`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::a_reference_longer_than_reference_max_len_is_refused`

**Status:** done — Lesen und Schreiben weisen eine Referenz mit mehr als 128 Zeichen ab;
128 Zeichen sind erlaubt (d96b535312c).

### §7.7.3.1 Weitere Formate wie JSON

**Spec:** §7.7.3.1, S. 27–28 — "It is expected that additional
representations may be added after they are defined in other OMG
specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Ausblick.

### §7.7.3.1.1 Referenz auf eine im Agent bekannte Beschreibung

**Spec:** §7.7.3.1.1, S. 28 — "The REPRESENTATION_BY_REFERENCE represents the
object using an object_reference encoded in a string. The string shall refer by
name to a description already known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`referenced`, `participant`,
`participant_definition`, `application_definition`); `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::entity`, `AgentProfiles::qos_profile`); `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions`); `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_reference_is_read_like_the_profile_it_names`

**Status:** done — Participant, Topic, Publisher, Subscriber, DataWriter und DataReader
lösen sich gegen `profile_name` einer Profildatei im Format von Fast DDS auf;
QosProfile (`Lib::Profile`), Application (`Lib::App`) und DomainParticipant
(`Lib::Participant`) gegen die Bibliotheken der Dateien und gegen die
Definitionen anderer Clients (dc63d65691f, 3aea52dd70a, b6e55cfa859). Eine
unbekannte Referenz ist STATUS_ERR_UNKNOWN_REFERENCE. Publisher, Subscriber,
DataWriter und DataReader per Referenz gibt es nur mit
`representation_formats: any`. Typ und Domain stehen in den Items zu
§7.7.3.3.1 und §7.7.3.4.1.

### §7.7.3.1.1 object_reference nach elementNameReference von DDS-XML
**Spec:** §7.7.3.1.1, S. 28 — "The object_reference shall be a string
formatted as defined by the XSD simpleType elementNameReference defined in the
[DDS-XML] specification file dds-xml_domain_definitions_nonamespace.xsd."

**Repo:** `crates/xrce/src/object_repr.rs` (`element_name_reference`); `crates/xrce-agent/src/daemon/session.rs` (`handle_create`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::element_name_references_follow_the_xsd_pattern`, `crates/xrce-agent/src/daemon/session.rs::tests::an_object_reference_must_be_an_element_name_reference`

**Status:** done — der Agent prüft den `object_reference` eines CREATE nach dem XSD-Typ `elementNameReference` (`dds-xml_qos_definitions_nonamespace.xsd`): `whiteSpace collapse`, dann das Muster `((::)?[a-zA-Z0-9_.])+`. Eine Zeichenkette anderer Form ist STATUS_ERR_INVALID_DATA, eine wohlgeformte, die der Agent nicht kennt, STATUS_ERR_UNKNOWN_REFERENCE; Referenzen der Form `Library::Name` werden gegen die Bibliotheken von DDS-XML aufgelöst (3aea52dd70a, dc63d65691f). a515c614d98.
### §7.7.3.1.1 Vorkonfiguration offline oder über das Protokoll

**Spec:** §7.7.3.1.1, S. 28 — "The pre-configuration may be done off-line
prior to starting the XRCE Agent or may be done on-line using the DDS-XRCE
protocol in combination with the REPRESENTATION_AS_XML_STRING."

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs` (`AgentProfiles::load`, `AgentProfiles::add_profiles`);
`crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::definition_to_register`, `rebuild_profiles`)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::the_eprosima_refs_file_indexes_every_entity_profile`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::types_and_qos_libraries_in_a_dds_document_are_read`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`

**Status:** done — offline über `xml_profile_files:` (`<types>`, `<qos_library>`,
`<domain_library>`, `<domain_participant_library>`, `<application_library>`;
dc63d65691f). Über das Protokoll: ein CREATE mit XML von QosProfile, Type,
Application oder DomainParticipant legt eine Definition an, die andere Clients
per Referenz nutzen, solange das Objekt des anlegenden Clients besteht
(3aea52dd70a).

### §7.7.3.1.1 Beispiel einer Referenz

**Spec:** §7.7.3.1.1, S. 28 — "The following string is an example of an
object_reference used to represent an XRCE QosProfile:
'MyQosLibrary::MyQosProfile'."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel.

### §7.7.3.1.1 Arten mit Referenz

**Spec:** §7.7.3.1.1, S. 28 — "This format is available for the XRCE Object
kinds that can be configured as libraries in the XRCE Agent. These are XRCE
Type, QosProfile, Domain, DomainParticipan, and Application."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`format_is_allowed`); `crates/xrce-agent/src/daemon/config.rs` (Schlüssel
`representation_formats`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`,
`crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_representation_formats_can_be_widened`

**Status:** done — Vorgabe ist die Lesart von Annex A: Referenz für Participant und Topic
(`OBJK_Representation3_Base`) sowie Type, QosProfile und Application
(`OBJK_RepresentationRefAndXML_Base`); Publisher, Subscriber, DataWriter und
DataReader haben nur Binär und XML. Die Spec widerspricht sich: OBJK_TOPIC_Representation
hat eine Referenz (§7.7.3.7.1, S. 33), §8.4.10 und §8.4.11 (S. 82–83) nennen sie
auch für DataWriter und DataReader. Die andere Lesart ist die Einstellung
`representation_formats: any` (bdf3f6ee966); beide sind getestet.

### §7.7.3.1.2 XML nach DDS-XML
**Spec:** §7.7.3.1.2, S. 28–29 — "The string shall contain an XML element
formatted according to the [DDS-XML] specification. The format of the string
is defined for each Object kind in clauses 7.7.3.2 to 7.7.3.11."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs`; `crates/xrce-agent/src/daemon/definitions.rs`

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_type_by_reference_names_a_known_type_by_its_fully_qualified_name`, `crates/xrce-agent/tests/spec_items_session.rs::a_subscriber_in_xml_has_its_qos_and_its_nested_data_reader`

**Status:** done — Type, QosProfile, Application, DomainParticipant (mit den enthaltenen Objekten), Topic, Publisher, Subscriber, DataWriter und DataReader werden nach DDS-XML gelesen (3aea52dd70a, 9239562286c). Ein `<types>` darf auf bereits bekannte Typen verweisen (Item zu §7.7.3.3.2). Eine Domain mit CREATE anzulegen definiert die Spec nicht (§7.7.3.4).
### §7.7.3.1.2 Formen von DDS-XML für Entitäten
**Spec:** §7.7.3.1.2, S. 28–29 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint_from_xml`, `container_from_xml`, `topic_from_xml`, `apply_qos_profile`); `crates/xrce-agent/src/daemon/definitions.rs`

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_subscriber_in_xml_has_its_qos_and_its_nested_data_reader`, `crates/xrce-agent/tests/spec_items_session.rs::a_data_reader_in_xml_has_its_qos_and_its_content_filter`, `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_qos_of_a_topic_is_the_default_of_its_endpoints`

**Status:** done — gelesen und getestet: `register_type_ref`, `topic_ref` als Name des Topic, `<datawriter_qos>`/`<datareader_qos>` mit `base_name`, `qos_profile_ref`, `<publisher_qos>`, `<subscriber_qos>`, `<topic_qos>`, `<domain_participant_qos>`, `<content_filter>` und die Bibliotheken `<qos_library>`, `<domain_library>`, `<domain_participant_library>`, `<application_library>` (3aea52dd70a, 9239562286c, 2c715803843, 0f85937a436).
### §7.7.3.1.2 XML für die Konfiguration aus der Ferne

**Spec:** §7.7.3.1.2, S. 29 — "This format is intended for remotely
configuring the agent. Typically it will not be used by the XRCE Clients except
in deployments where the client-to-agent connection has sufficient
bandwidth."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis. Das Beispiel darunter (S. 29) hat
falsch geschlossene Elemente (`<reliability>`, `<data_reader_qos>`,
`<qos_library>`).

### §7.7.3.1.3 Binär als XCDR2 der Struktur der Art

**Spec:** §7.7.3.1.3, S. 28 — "The REPRESENTATION_IN_BINARY represents objects
using a binary_representation octet sequence. The octet sequence is the result
of serializing an IDL-defined data-structure that depends on the kind of object
using the XCDR version 2 format defined in [DDS-XTYPES]."

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`participant`, `topic`, `container`, `endpoint`:
XCDR2-Leser); `crates/xrce-agent/src/daemon/entity_repr.rs` (`Cdr`: die Strukturen des
eProsima-Micro-XRCE-DDS-Client 3.0.1 als weitere Lesart)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::annex_a_topic_has_a_dheader`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::the_36_octet_example_of_the_clause_is_a_topic`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_topic_names_its_topic_and_type`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_big_endian_binary_topic_reads_back`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_blob_of_no_form_is_refused_naming_each_reading`

**Status:** done — die `sequence<octet>` wird als XCDR2 der Struktur der Art gelesen, in
beiden Formen der Spec (Annex A mit DHEADER, die Abschnitte als FINAL); ein
Blob, der keine der Formen ist, wird mit Nennung jeder Lesart abgewiesen. Die
Strukturen von eProsima (klassisches CDR) bleiben als weitere Lesart
(c67ce23c724).

### §7.7.3.1.3 Binärformen nach §7.7.3 und Annex A

**Spec:** §7.7.3.1.3, S. 28 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`participant`, `topic`, `container`,
`endpoint`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`, `topic`, `container`,
`endpoint`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::participant_binary_in_both_forms`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::annex_a_topic_has_a_dheader`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::publisher_binary_of_annex_a`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::reader_binary_of_the_clause`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_writer_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_clause_binary_reader_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_participant_is_read`

**Status:** done — Participant, Topic, Publisher, Subscriber, DataWriter und DataReader
werden in der Form von Annex A (DHEADER) und der Form der Abschnitte gelesen:
`string topic_name` bei DataWriter und DataReader, `TypeIdentifier` und
`type_reference` beim Topic, die Binärform des Participant (c67ce23c724).

### §7.7.3.1.3 Beispiel OBJK_Topic_Binary mit 36 Byte

**Spec:** §7.7.3.1.3, S. 29 — Topic "Square", `type_reference`
"MyTypes::ShapeType", 36 Byte little-endian.

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`topic`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`topic`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::the_36_octet_example_of_the_clause_is_a_topic`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_topic_names_its_topic_and_type`

**Status:** done — der Test dekodiert die 36 Byte des Beispiels als Topic mit
`type_reference` „MyTypes::ShapeType“. Die Zeichen des Beispiels lauten
'S','q','u','e','r','e'; `binary_forms` liest sie so (92633850a97), der Text der
Spec nennt das Topic „Square“. Zusätzlich liest `entity_repr` dieselbe Struktur
mit „Square“ als 36-Byte-Blob (c67ce23c724).

---

## §7.7.3.2 XRCE QosProfile

### §7.7.3.2 OBJK_QOSPROFILE_Representation

**Spec:** §7.7.3.2, S. 29 — "struct OBJK_QOSPROFILE_Representation :
OBJK_RepresentationRefAndXML_Base { };"

**Repo:** `crates/xrce/src/object_repr.rs` (`decode_for_kind`, kein
nachgestelltes Feld für `ObjectKind::QosProfile`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`

**Status:** done — Kodierung; die Wirkung steht in den folgenden Items.

### §7.7.3.2.1 Referenz auf ein QosProfile „Lib::Profile“

**Spec:** §7.7.3.2.1, S. 29 — "the object_reference field shall contain the
fully qualified name of a QosProfile known to the XRCE Agent. The fully
qualified name is composed of the name of the Qos library and the name of the
QosProfile within the library."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, Zweig OBJK_QOSPROFILE); `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::qos_profile`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`

**Status:** done — CREATE(`OBJK_QOSPROFILE`) mit REPRESENTATION_BY_REFERENCE löst „Lib::Profile“
gegen die QoS-Bibliotheken der Dateien und der Definitionen anderer Clients
auf; ein Name in keiner Bibliothek ist STATUS_ERR_UNKNOWN_REFERENCE
(3aea52dd70a).

### §7.7.3.2.2 XML mit einer qos_library und einem qos_profile

**Spec:** §7.7.3.2.2, S. 29 — "the string_representation field shall contain a
single <qos_library> top-level XML element with the syntax defined by the XSD
complexType qosLibrary [...]. The <qos_library> element shall contain a single
<qos_profile> child element."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::definition_to_register`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`

**Status:** done — die `<qos_library>` mit genau einem `<qos_profile>` wird gelesen und als
Definition gespeichert; eine Bibliothek mit zwei Profilen ist
STATUS_ERR_INVALID_DATA (9239562286c).

### §7.7.3.2.2 Verweis auf andere QosProfiles

**Spec:** §7.7.3.2.2, S. 29–30 — "The REPRESENTATION_AS_XML_STRING
representation may reference other QoS profiles already known to the Agent.
[...] <qos_profile name="ModifiedProfile" base_name="MyQosLib:MyQosProfile">"

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::definition_to_register`); `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::qos_profile`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`

**Status:** done — `base_name` mit einem Doppelpunkt („Q:One“) wird wie „Q::One“ gelesen;
das Profil erbt vom Profil, das der Agent kennt (9239562286c). Das Beispiel
schreibt `base_name` mit einem Doppelpunkt, §7.7.3.2.1 verlangt „::“.

---

## §7.7.3.3 XRCE Type

### §7.7.3.3 OBJK_TYPE_Representation

**Spec:** §7.7.3.3, S. 30 — "struct OBJK_TYPE_Representation :
OBJK_RepresentationRefAndXML_Base { };"

**Repo:** `crates/xrce/src/object_repr.rs` (`decode_for_kind`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_binary_complete_type_object_is_named_by_its_own_type_name`

**Status:** done — Kodierung. `REPRESENTATION_IN_BINARY` für `OBJK_TYPE` ist mit der
Vorgabe `representation_formats: spec` STATUS_ERR_INVALID_DATA (Item „Erlaubte
Formate je Art“); mit `any` liest `entity_repr::types` einen
`TypeObject`.

### §7.7.3.3.1 Referenz auf einen Typ mit Modulen
**Spec:** §7.7.3.3.1, S. 30 — "the object_reference field shall contain the
fully qualified name of an XRCE Type known to the XRCE Agent. The fully
qualified name is composed of the name of the type prepended by the names of
the enclosing modules."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, `find_type_definition`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_type_by_reference_names_a_known_type_by_its_fully_qualified_name`

**Status:** done — CREATE(`OBJK_TYPE`) mit REPRESENTATION_BY_REFERENCE löst den Namen (`MyModule::ShapeType`) gegen die Typen des Clients, der Dateien (`<types>`), der Definitionen anderer Clients und von `type_files:` auf; ein unbekannter Name, auch ohne Modul, ist STATUS_ERR_UNKNOWN_REFERENCE (3aea52dd70a, 0f85937a436).
### §7.7.3.3.2 XML mit types, ein Typ mit nested=false

**Spec:** §7.7.3.3.2, S. 30 — "the string_representation field shall contain a
single <types> top-level XML element [...]. Within the <types> element there
may be multiple types defined. In this case only one type shall have the
nested annotation (see [DDS-XTYPES]) set to false. This corresponds to the
XRCE Type being created."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`types`, `nested_flags`);
`crates/xrce-agent/src/daemon/session.rs` (`DdsCreate::Types`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_gets_the_type_its_type_name_names`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_xml_type_library_compiles_to_named_complete_types`

**Status:** done — `<types>` wird nach DDS-XML gelesen und übersetzt; alle benannten Typen
werden registriert. Bei mehreren Typen muss genau einer `nested` nicht tragen,
sonst STATUS_ERR_INVALID_DATA (9239562286c). Ein Topic findet seinen Typ über
den Namen (`SessionEngine::resolve_type`).

### §7.7.3.3.2 Annotation nested

**Spec:** §7.7.3.3.2, S. 30 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`types`, `nested_flags`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`

**Status:** done — `nested_flags` liest die Annotation je Typ (auch in Modulen); zwei Typen
ohne `nested="true"` sind STATUS_ERR_INVALID_DATA, ein Typ ohne und einer mit
`nested="true"` ist STATUS_OK (9239562286c).

### §7.7.3.3.2 Verweis auf bekannte Typen
**Spec:** §7.7.3.3.2, S. 30 — "The string_representation may reference other
Types already known to the Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`types`); `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, `ObjectKind::Type`); `crates/xml/src/typeobject_bridge.rs` (`bridge_library_complete_with_known`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_types_string_may_reference_types_the_agent_knows`, `crates/xrce-agent/src/daemon/session.rs::tests::a_types_string_references_a_type_created_before`

**Status:** done — `types` übergibt `zerodds_xml::bridge_library_complete_with_known(lib, known)` die dem Agenten bekannten Typen: die vom selben Client angelegten, die aus `type_files:` und die der Profildateien. Ein Verweis auf einen von ihnen wird zu dessen `EquivalenceHashComplete`, der bekannte Typ wird nicht noch einmal definiert. Ein Name, der weder in der Zeichenkette steht noch bekannt ist, ist STATUS_ERR_UNKNOWN_REFERENCE. Nicht geprüft: ob ein Verweis auf einen Typ gelingt, den ein anderer Client binär (nicht als XML `<types>`) angelegt hat; die Typen aus XML-`<types>` gehören den Definitionen des Agenten (§9.2).

---

## §7.7.3.4 XRCE Domain

### §7.7.3.4 OBJK_DOMAIN_Representation
**Spec:** §7.7.3.4, S. 30–31 — "struct OBJK_DOMAIN_Representation :
OBJK_RepresentationRefAndXML_Base { };"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Die Spec gibt Domain keinen Wert von `ObjectKind` und keinen Fall in `ObjectVariant` (§7.7.5, S. 39; Annex A, S. 103 und S. 111): ein CREATE kann keine Domain benennen, es gibt nichts, was erfüllt werden kann. Kind 0x0F ist `OBJK_OTHER` (f63fa2d1218). Domains kommen aus `<domain_library>` der Dateien und sind als `Library::Domain` referenzierbar (`domain_ref`, `domain_reference`).
### §7.7.3.4.1 Referenz auf eine Domain „Lib::Domain“
**Spec:** §7.7.3.4.1, S. 31 — "the object_reference field shall contain the
fully qualified name of an XRCE Domain definition known to the Agent."

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`domain`, `domain_scope`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_domain_reference_is_resolved_against_the_domain_libraries`

**Status:** done — eine Domain ist als „Lib::Domain“ referenzierbar: `domain_ref` des DomainParticipant in XML und `domain_reference` der Binärform werden gegen die `<domain_library>` der Dateien aufgelöst (dc63d65691f, b4cd825c268). Als `object_reference` eines CREATE kann sie nicht stehen, weil die Spec der Domain keine ObjectKind gibt (§7.7.3.4).
### §7.7.3.4.2 XML mit domain_library und einer domain
**Spec:** §7.7.3.4.2, S. 31 — "The XML shall contain a single
<domain_library> top-level XML element [...]. The <domain_library> element
shall contain a single <domain> child element. [...] The string_representation
may reference Types already known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`add_domain_library`)

**Tests:** `crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`

**Status:** done — die `<domain_library>` mit ihren `<domain>` wird aus den Dateien gelesen (auch als Wurzel; `base_name`, `register_type`, `topic`; dc63d65691f). Eine Domain als Inhalt eines CREATE definiert die Spec nicht (§7.7.3.4).

---

## §7.7.3.5 XRCE Application

### §7.7.3.5 OBJK_APPLICATION_Representation

**Spec:** §7.7.3.5, S. 31 — "The OBJK_TYPE_Representation supports the
REPRESENTATION_BY_REFERENCE and REPRESENTATION_AS_XML_STRING formats. [...]
struct OBJK_APPLICATION_Representation : OBJK_RepresentationRefAndXML_Base {
};"

**Repo:** `crates/xrce/src/object_repr.rs` (`decode_for_kind`);
`crates/xrce-agent/src/lib.rs` (`XrceAgent::create_object`)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::create_application_object_via_objk_application`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`

**Status:** done — Kodierung und Speicherung. Der Satz nennt `OBJK_TYPE_Representation`
statt `OBJK_APPLICATION_Representation`. Ein CREATE in Binär ist
STATUS_ERR_INVALID_DATA (Item „Erlaubte Formate je Art“).

### §7.7.3.5.1 Referenz auf eine Application „Lib::App“

**Spec:** §7.7.3.5.1, S. 31 — "the object_reference field shall contain the
fully qualified name of an XRCE Application definition known to the Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`application_definition`); `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::application`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`

**Status:** done — „Lib::App“ wird gegen die `<application_library>` der Dateien und der
Definitionen anderer Clients aufgelöst; das CREATE legt den DomainParticipant
der Application mit allem darin an. Eine unbekannte Application ist
STATUS_ERR_UNKNOWN_REFERENCE (3aea52dd70a, b6e55cfa859).

### §7.7.3.5.2 XML mit application_library und einer application

**Spec:** §7.7.3.5.2, S. 31–32 — "The XML shall contain a single
<application_library> top-level XML element [...]. This element shall contain
a single <application> child element. [...] The string_representation may
reference XRCE Types, Qos Profiles, Domains, or DomainParticipants already
known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`application_definition`); `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::add_application_library`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::what_cannot_be_planned_is_refused`

**Status:** done — die `<application_library>` mit einer `<application>` wird gelesen; ihre
Participants verweisen auf Typen, QoS-Profile, Domains und DomainParticipants
der Definitionen (3aea52dd70a). Das Beispiel (S. 32) legt im Subscriber ein
`<data_writer name="MyReader" topic_ref="Circle">` an, obwohl nur „Square“
definiert ist; der Code weist ein `topic_ref` auf ein Topic, das der
Participant nicht hat, ab.

---

## §7.7.3.6 XRCE DomainParticipant

### §7.7.3.6 OBJK_PARTICIPANT_Representation mit domain_id

**Spec:** §7.7.3.6, S. 32 — "struct OBJK_PARTICIPANT_Representation :
OBJK_Representation3_Base { short domain_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`encode_for_kind`:
Ausrichtung auf 2 vor `domain_id`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::participant_representation_matches_the_captured_create_bytes`

**Status:** done

### §7.7.3.6.1 Referenz auf einen DomainParticipant

**Spec:** §7.7.3.6.1, S. 32 — "the object_reference field shall contain the
fully qualified name of an XRCE DomainParticipant definition known to the
Agent. The fully qualified name is composed of the name of the
DomainParticipant library and the name of the DomainParticipant within the
library."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`, `participant_definition`); `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::participant`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_reference_is_read_like_the_profile_it_names`

**Status:** done — die Referenz löst sich gegen `<participant profile_name>` einer
Profildatei und gegen die `<domain_participant_library>` auf („Lib::Participant“,
bei einer Application „Lib::App::Participant“); die Domain kommt aus der
Definition (`domain_id` oder `domain_ref`) oder aus dem Profil
(3aea52dd70a, b6e55cfa859).

### §7.7.3.6.2 XML mit domain_participant_library

**Spec:** §7.7.3.6.2, S. 32 — "the string_representation field shall contain a
single <domain_participant_library> top-level XML element [...]. This element
shall contain a single <domain_participant> child element. [...] The
string_representation may reference XRCE Types, Qos Profiles, Domains, or
DomainParticipants already known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant_definition`, `participant_source`);
`crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions::add_participant_library`, `plan_participant`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_creates_the_objects_inside_it`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_with_a_broken_part_creates_nothing`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`

**Status:** done — die `<domain_participant_library>` mit einem `<domain_participant>` wird
gelesen; das CREATE legt die enthaltenen Topics, Publisher, Subscriber und
Endpunkte mit an (ObjectId aus dem Namen) und aktiviert sie mit dem Participant.
Schlägt ein Teil fehl, bleibt nichts zurück (3aea52dd70a).

### §7.7.3.6.3 OBJK_DomainParticipant_Binary

**Spec:** §7.7.3.6.3, S. 33 — "struct OBJK_DomainParticipant_Binary { long
domain_id; @optional string<128> domain_reference; @optional string<128>
qos_profile_reference; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`participant`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::participant_binary_in_both_forms`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_participant_is_read`

**Status:** done — drei Lesarten des Blobs: Annex A (DHEADER, ohne `domain_id`), die Form
der Abschnitte (`long domain_id` zuerst, FINAL) und die zwei Oktette `00 00` des
eProsima-Client. `domain_id` der Binärform und die der Repräsentation (`short`)
müssen übereinstimmen, sonst STATUS_ERR_INVALID_DATA (c67ce23c724).

### §7.7.3.6.3 domain_reference
**Spec:** §7.7.3.6.3, S. 33 — "Any XRCE Topic and Type definitions contained in
the referenced domain are considered defined within the scope of the XRCE
DomainParticipant and become available as references to construct XRCE
objects contained by the DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`); `crates/xrce-agent/src/daemon/definitions.rs` (`domain_scope`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_domain_reference_is_resolved_against_the_domain_libraries`

**Status:** done — `domain_reference` wird gegen die Domains der `<domain_library>` aufgelöst; die `domain_id` der Domain gilt für den Participant und muss mit `long domain_id` der Binärform übereinstimmen; ein Name in keiner Bibliothek ist STATUS_ERR_UNKNOWN_REFERENCE (b4cd825c268). Topics und Typen der Domain stehen im Geltungsbereich des Participant (4a57a1fa4ce, siehe das nächste Item).
### §7.7.3.6.3 Topics und Typen der Domain bei der Binärform
**Spec:** §7.7.3.6.3, S. 33 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`ParticipantDescription::scope`); `crates/xrce-agent/src/daemon/session.rs` (`participant_scopes`); `crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions::domain_scope`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_domain_reference_is_resolved_against_the_domain_libraries`

**Status:** done — bei der Binärform mit `domain_reference` stehen die `register_type` und Topics der Domain im Geltungsbereich des Participant wie beim XML-Participant mit `domain_ref`; ein Topic des Participant findet sie über den Namen (4a57a1fa4ce).
### §7.7.3.6.3 qos_profile_reference

**Spec:** §7.7.3.6.3, S. 33 — "If specified, the corresponding DDS
DomainPaticipant shall be created using that Qos. Otherwise, the
DomainPaticipant shall be created using the DDS default Qos."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`, `participant_qos_of`); `crates/xrce-agent/src/daemon/dds_port.rs`
(`DdsPort::create_participant_with`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_participant_qos_profile_reference_is_applied`

**Status:** done — das Participant-QoS des Profils (`<domain_participant_qos>`) gilt je
Participant: ENTITY_FACTORY beim Participant (bei `autoenable_created_entities`
false bleiben seine Publisher, Subscriber und Topics deaktiviert), USER_DATA über
eine eigene DCPS-Laufzeit je Domain und Wert. Eine andere Policy im Participant-QoS
steht nicht im XSD von DDS-XML 1.0: `crates/xml` weist die Datei beim Lesen ab
(Test `a_participant_policy_outside_the_schema_is_refused`); trägt ein aufgelöstes
Profil sie trotzdem, ist es STATUS_ERR_INCOMPATIBLE. Es gibt keine Policy, die das
Schema im Participant-QoS erlaubt und die der Agent nicht anwenden kann (USER_DATA
und ENTITY_FACTORY werden beide angewendet). Ein Profil ohne Participant-QoS wendet nichts an;
ohne Referenz gilt das DDS-Default-QoS (b4cd825c268).

---

## §7.7.3.7 XRCE Topic

### §7.7.3.7 OBJK_TOPIC_Representation mit participant_id

**Spec:** §7.7.3.7, S. 33 — "struct OBJK_TOPIC_Representation :
OBJK_Representation3_Base { ObjectId participant_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`,
`ObjectVariantExtra::parent`: `ObjectId` ohne Ausrichtung hinter der Variante)

**Tests:** `crates/xrce/src/object_repr.rs::tests::topic_representation_matches_the_captured_create_bytes`,
`crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`

**Status:** done

### §7.7.3.7 Topic gehört zum DomainParticipant

**Spec:** §7.7.3.7, S. 33 — "Independent of the representation format, the
field participant_id shall contain the ObjectId of an XRCE DomainParticipant
object. The referenced or created Topic will belong to the specified
DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::plan_dds_create`,
Zweig `ObjectKind::Topic`; `participant_of_create`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_is_created_by_name_or_by_register_type_ref`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::null_port_rejects_topic_before_participant`

**Status:** done — Art und Existenz des Participant werden geprüft (falsche Art:
STATUS_ERR_INVALID_DATA, fehlender Participant: STATUS_ERR_UNKNOWN_REFERENCE),
das Topic wird mit ihm angelegt. Getestet sind das Anlegen unter dem Participant
und die Reihenfolge auf Port-Ebene; einen Test des Fehlerwegs eines Topic auf
Sitzungsebene gibt es nicht (dd55e9c9df1).

### §7.7.3.7.1 Referenz auf den Namen eines Topic im Participant

**Spec:** §7.7.3.7.1, S. 33 — "the object_reference field shall contain the
bare name of an XRCE Topic defined in XRCE DomainParticipant identified by the
participant_id. The Topic could be defined directly on the XRCE
DomainParticipant, or else in the XRCE Domain associated with the
DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`topic`, Zweig BY_REFERENCE:
`ParticipantScope::topic`, danach `<topic profile_name>` der Profildateien);
`crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions::scope_of`:
Topics des Participant und seiner Domain)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_topic_is_created_by_name_or_by_register_type_ref`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_reference_is_read_like_the_profile_it_names`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`

**Status:** done — der Name wird gegen die Topics des Participant und seiner
Domain aufgelöst; ein Name, den beide und die Profildateien nicht haben, ergibt
STATUS_ERR_UNKNOWN_REFERENCE. Der CREATE-Test nennt ein Topic des Participant;
die Topics der Domain prüft der Test von `definitions.rs` an der Planung eines
Participant (3aea52dd70a).

### §7.7.3.7.2 XML mit einem topic
**Spec:** §7.7.3.7.2, S. 34 — "the string_representation field shall contain a
single <topic> top-level XML element with the syntax defined by the XSD
complexType topic [...]. <topic name="Square" register_type_ref="ShapeType"
/> The string_representation may reference XRCE Types or QosProfiles already
known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`topic_from_xml`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_qos_of_a_topic_is_the_default_of_its_endpoints`

**Status:** done — `<topic name=… register_type_ref=…/>` (das Beispiel der Spec) wird gelesen: die Registrierung wird im Participant und in seiner Domain gesucht, ihr Typ unter den dem Agent bekannten Typen; eine Registrierung, die der Participant nicht hat, ergibt STATUS_ERR_UNKNOWN_REFERENCE. Die Attributform (`type_name`) und `<dataType>` von Fast DDS bleiben lesbar. QoS und QosProfile-Verweise des Topic werden angewandt (nächstes Item). 3aea52dd70a.
### §7.7.3.7.2 QoS eines Topic und QosProfile-Verweise
**Spec:** §7.7.3.7.2, S. 34 — "The string_representation may reference XRCE Types or
QosProfiles already known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`TopicQos`, `topic_from_xml`, `endpoint_for_topic`); `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_qos_of_a_topic_is_the_default_of_its_endpoints`

**Status:** done — `<topic_qos base_name=…>` und `qos_profile`/`qos_profile_ref` des `<topic>` werden gelesen; was das Topic sagt, ist die Vorgabe der Endpunkte, die für es angelegt werden (DDS 1.4 §2.2.2.3.1), die Darstellung des Endpunkts selbst hat Vorrang; TOPIC_DATA gehört dazu. 2c715803843.
### §7.7.3.7.3 OBJK_Topic_Binary

**Spec:** §7.7.3.7.3, S. 34 — "struct OBJK_Topic_Binary { string<256>
topic_name; @optional string<256> type_reference; @optional
DDS:XTypes::TypeIdentifier type_identifier; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`topic`, `TopicBinary`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`topic`, Zweig `InBinary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::the_36_octet_example_of_the_clause_is_a_topic`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::annex_a_topic_has_a_dheader`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::string_bounds_are_checked`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_topic_names_its_topic_and_type`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_topic_names_its_topic_and_type`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_big_endian_binary_topic_reads_back`

**Status:** done — XCDR2 nach §7.7.3.1.3: `topic_name`, optionales
`type_reference`, optionales `type_identifier` (`TypeIdentifier`), `string<256>`
geprüft. Die Spec gibt die Struktur doppelt an: Annex A als APPENDABLE mit
DHEADER, die Klausel als FINAL ohne. Beide Fassungen werden gelesen (Annex A
zuerst, die Form des Blobs entscheidet, kein Schalter), danach die Struktur des
eProsima-Client. Die 36 Oktette des Beispiels der Klausel schreiben „Squere“
(der Text: „Square“); der Test erwartet die Oktette. c67ce23c724, 92633850a97.

### §7.7.3.7.3 type_identifier

**Spec:** §7.7.3.7.3, S. 34 — "The type_identifier, if present, shall contain
the DDS-XTYPES TypeIdentifier for the data-type. See clause 7.3.2 of
[DDS-XTYPES]."

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`topic`:
`TypeIdentifier::decode_from`); `crates/xrce-agent/src/daemon/session.rs`
(`type_name_of`, `plan_dds_create`, Zweig Topic)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::a_topic_with_a_type_identifier_reads_it_and_one_without_a_type_is_refused`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::string_bounds_are_checked`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_topic_may_name_its_type_by_identifier`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_can_name_its_type_by_type_identifier`

**Status:** done — ein vorhandenes `type_identifier` wird als
XTypes-`TypeIdentifier` gelesen und mit den COMPLETE-Identifiern der dem Agent
bekannten Typen (`OBJK_TYPE` des Client, `type_files:`) verglichen; ein
Identifier ohne Treffer ergibt STATUS_ERR_UNKNOWN_REFERENCE. Die Grenzen
`string<256>` werden geprüft. c67ce23c724.

### §7.7.3.7.3 Nicht beide fehlen, type_identifier hat Vorrang
**Spec:** §7.7.3.7.3, S. 34 — "Either type_reference or type_identifier may be
used to identify the XRCE Type associated with the Topic. Either member may be
omitted, but not both. If both are present the type_identifier shall take
precedence."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`); `crates/xrce-agent/src/daemon/binary_forms.rs` (`topic`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_binary_topic_takes_its_type_from_the_type_identifier_and_needs_one_of_the_two`

**Status:** done — fehlen beide Member, ergibt das STATUS_ERR_INVALID_DATA (f87b3c3d0b2, c67ce23c724); mit `type_identifier` allein wird der Typ gefunden; sind beide da, hat der Identifier Vorrang (0f85937a436).
### §7.7.3.7.3 type_reference auf einen bekannten Typ

**Spec:** §7.7.3.7.3, S. 34 — "The type_reference, if present, shall contain
the fully qualified name of the type [...]. The referenced type shall be known
to the XRCE Agent either via pre-configuration, or as a result of a prior
create operation executed on the XRCE ProxyClient; see 7.8.3.1."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::resolve_type`,
`find_type_definition`, `apply_dds_create`, Zweig Topic)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_topic_gets_the_type_its_type_name_names`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_configured_type_is_known_to_every_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_of_an_unknown_type_is_refused_when_the_agent_is_told_to`

**Status:** done — der Typ wird zuerst unter den `OBJK_TYPE` des Client
(früheres CREATE), dann unter `type_files:` (Vorkonfiguration) gesucht; ein
Topic erhält die Definition des Typs, den sein Name nennt (Test mit zwei Typen
des Client). Ein unbekannter Typ ergibt STATUS_ERR_UNKNOWN_REFERENCE (Item
unten). e4d37a6aff6.

### §7.7.3.7.3 Topic mit unbekanntem Typ

**Spec:** §7.7.3.7.3, S. 34 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`apply_dds_create`, Zweig Topic;
`SessionEngineConfig::forward_unknown_topic_types`);
`crates/xrce-agent/src/daemon/config.rs` (Schlüssel `unknown_topic_types`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_topic_of_an_unknown_type_is_refused_when_the_agent_is_told_to`,
`crates/xrce-agent/src/daemon/config.rs::tests::unknown_topic_types_are_refused_unless_forwarding_is_named`

**Status:** done — Standard ist die Spec: ein Topic, dessen Typ der Agent nicht
kennt, ergibt STATUS_ERR_UNKNOWN_REFERENCE, nichts wird abgelegt. Benannte
Einstellung: Agent-Schlüssel `unknown_topic_types: forward` legt das Topic an
und meldet `SessionEvent::UnknownTypeForwarding`; Samples gehen dann ohne
Schlüssel und ohne `TypeInformation` hinaus (der eProsima-Client nennt Typen,
die der Agent nicht hält). e4d37a6aff6, 0da6aef56b5.

---

## §7.7.3.8 XRCE Publisher

### §7.7.3.8 OBJK_PUBLISHER_Representation mit participant_id

**Spec:** §7.7.3.8, S. 34 — "struct OBJK_PUBLISHER_Representation :
OBJK_RepresentationBinAndXML_Base { ObjectId participant_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`,
`ObjectVariantExtra::parent`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`

**Status:** done — Kodierung; die Prüfung steht im nächsten Item.

### §7.7.3.8 Publisher gehört zum DomainParticipant

**Spec:** §7.7.3.8, S. 34 — "the member participant_id shall contain the
ObjectId of an XRCE DomainParticipant object. The referenced or created
Publisher shall belong to the specified DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, Zweig
Publisher/Subscriber; `participant_of_create`; `apply_dds_create`:
`DdsPort::create_publisher`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_or_subscriber_needs_an_existing_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_and_a_subscriber_get_a_dds_entity_with_their_policies`

**Status:** done — Art und Existenz des Participant werden geprüft (fehlender
Participant: STATUS_ERR_UNKNOWN_REFERENCE, andere Art: STATUS_ERR_INVALID_DATA,
nichts wird abgelegt); der Publisher wird als DDS-Publisher unter dem
Participant angelegt. dd55e9c9df1.

### §7.7.3.8.1 XML mit einem publisher

**Spec:** §7.7.3.8.1, S. 34–35 — "the string_representation field shall
contain a single <publisher> top-level XML element [...]. Note that the XML
representation of a Publisher allows specifying Qos policies and including
nested DataWriter objects. [...] Any referenced object must have been
previously created or configured on the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`container`,
`container_from_xml`: `<publisher_qos>`, `<qos>`, eingebettete `<data_writer>`);
`crates/xrce-agent/src/daemon/session.rs` (`create_embedded`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_empty_container_xml_is_a_default_container`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_container_reads_presentation_and_entity_factory`

**Status:** done — `<publisher>` mit `<publisher_qos base_name=…>` (PARTITION,
GROUP_DATA, PRESENTATION, ENTITY_FACTORY werden angewandt) und `<qos>` von Fast
DDS; Richtlinien, die ein Publisher nicht hat, werden in `not_applied` benannt.
Das Beispiel (S. 35) schließt `<publisher name="MyPublisher"/>` vor seinen
Kindelementen. 3aea52dd70a, 9239562286c.

### §7.7.3.8.1 publisher_qos und eingebettete DataWriter

**Spec:** §7.7.3.8.1, S. 35 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`container_from_xml`:
`ContainerDescription::embedded`); `crates/xrce-agent/src/daemon/session.rs`
(`create_embedded`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`

**Status:** done — eingebettete `<data_writer>` werden mit der ObjectId ihres
Namens (§9.3) unter dem Publisher angelegt; scheitert einer (im Test: ein Topic,
das der Participant nicht hat, STATUS_ERR_UNKNOWN_REFERENCE), scheitert das
CREATE des Publisher und es bleibt nichts zurück. 9239562286c.

### §7.7.3.8.2 OBJK_Publisher_Binary

**Spec:** §7.7.3.8.2, S. 35 — "struct OBJK_PUBLISHER_QosBinary { @optional
sequence<string> partitions; @optional sequence<octet> group_data; }; struct
OBJK_Publisher_Binary { @optional string publisher_name; @optional
OBJK_PUBLISHER_QosBinary qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`container`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`container`, Zweig `InBinary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::publisher_binary_of_annex_a`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_publisher_gives_its_partitions`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_publisher_has_default_qos`

**Status:** done — `partitions` und `group_data` werden angewandt,
`publisher_name` wird gelesen und nicht verwendet. Annex A (APPENDABLE, DHEADER)
wird zuerst gelesen, danach die Fassung der Klausel ohne DHEADER, die auch der
eProsima-Client sendet. c67ce23c724.

---

## §7.7.3.9 XRCE Subscriber

### §7.7.3.9 OBJK_SUBSCRIBER_Representation mit participant_id

**Spec:** §7.7.3.9, S. 35 — "struct OBJK_SUBSCRIBER_Representation :
OBJK_RepresentationBinAndXML_Base { ObjectId participant_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::subscriber_representation_matches_the_captured_create_bytes`

**Status:** done — Kodierung.

### §7.7.3.9 Subscriber gehört zum DomainParticipant

**Spec:** §7.7.3.9, S. 35 — "the member participant_id shall contain the
ObjectId of an XRCE DomainParticipant object. The referenced or created
Subscriber shall belong to the specified DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, Zweig
Publisher/Subscriber; `participant_of_create`; `apply_dds_create`:
`DdsPort::create_subscriber`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_or_subscriber_needs_an_existing_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_and_a_subscriber_get_a_dds_entity_with_their_policies`

**Status:** done — wie beim Publisher (derselbe Zweig, beide Arten in den
Tests). dd55e9c9df1.

### §7.7.3.9.1 XML mit einem subscriber
**Spec:** §7.7.3.9.1, S. 35–36 — "the string_representation field shall
contain a single <subscriber> top-level XML element [...]. Note that the XML
representation of a Subscriber allows specifying Qos policies and including
nested DataReader objects."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`container_from_xml`); `crates/xrce-agent/src/daemon/session.rs` (`create_embedded`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_subscriber_in_xml_has_its_qos_and_its_nested_data_reader`

**Status:** done — der Code liest `<subscriber>` in denselben Zweigen wie `<publisher>` (3aea52dd70a); der Test legt ein `<subscriber>` mit `<subscriber_qos>` und Inhalt an (0f85937a436).
### §7.7.3.9.1 subscriber_qos und eingebettete DataReader
**Spec:** §7.7.3.9.1, S. 36 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_embedded`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`container_from_xml`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_subscriber_in_xml_has_its_qos_and_its_nested_data_reader`

**Status:** done — der Code legt die `<data_reader>` eines `<subscriber>` wie die `<data_writer>` eines `<publisher>` an (9239562286c); der Test legt ein Subscriber mit eingebettetem `<data_reader>` per CREATE an und prüft Partition und Reader (0f85937a436).
### §7.7.3.9.2 OBJK_Subscriber_Binary

**Spec:** §7.7.3.9.2, S. 36 — "struct OBJK_SUBSCRIBER_QosBinary { @optional
sequence<string> partitions; @optional sequence<octet> group_data; }; struct
OBJK_Subscriber_Binary { @optional string subscriber_name; @optional
OBJK_SUBSCRIBER_QosBinary qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`container`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`container`, dieselbe
Leseroutine wie für den Publisher)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::publisher_binary_of_annex_a`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_publisher_gives_its_partitions`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_publisher_has_default_qos`

**Status:** done — die Tests lesen die Form des Publisher, die Leseroutine ist
dieselbe. c67ce23c724.

---

## §7.7.3.10 XRCE DataWriter

### §7.7.3.10 DATAWRITER_Representation mit publisher_id

**Spec:** §7.7.3.10, S. 36 — "struct DATAWRITER_Representation :
OBJK_RepresentationBinAndXML_Base { ObjectId publisher_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`,
`crates/xrce/src/object_repr.rs::tests::annex_b_2_3_writer_variant_reads_as_the_blob_and_its_publisher`

**Status:** done — Kodierung.

### §7.7.3.10 DataWriter gehört zum Publisher

**Spec:** §7.7.3.10, S. 36 — "the member publisher_id shall contain the
ObjectId of an XRCE Publisher object. The referenced or created DataWriter
shall belong to the specified Publisher."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, Zweig
DataWriter/DataReader)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_endpoint_and_its_topic_belong_to_one_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`

**Status:** done — Art und Existenz des Publisher werden geprüft (fehlender
Publisher: STATUS_ERR_UNKNOWN_REFERENCE), DataWriter und Topic müssen zu einem
Participant gehören; der DataWriter erbt PARTITION und GROUP_DATA des Publisher.

### §7.7.3.10.1 XML mit einem data_writer

**Spec:** §7.7.3.10.1, S. 36–37 — "the string_representation field shall
contain a single <data_writer> top-level XML element [...]. <data_writer
name="MySquareWriter" topic_ref="Square"/> The referenced Topic must have been
previously created or configured on the XRCE DomainParticipant [...]. The XML
representation of a DataWriter allows specifying Qos policies."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`,
`endpoint_from_xml`: Attributform, Form von Fast DDS, `topic_ref` als Name,
`<datawriter_qos>` / `<data_writer_qos>`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_data_writer_states_its_qos_over_a_profile`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_attribute_form_still_reads`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_eprosima_element_form_names_topic_type_and_kind`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_qos_profile_reference_applies_the_profile`

**Status:** done — gelesen werden die Form von DDS-XML (`<data_writer name=…
topic_ref=…>`, `<datawriter_qos base_name=…>`), die Attributform (`topic_ref`
als ObjectId in Hex, `qos_profile`, `reliability`) und die Form von Fast DDS.
Eine BY_REFERENCE-Darstellung hat ein DataWriter nach §7.7.3.1 nicht; der
Agent-Schlüssel `representation_formats: any` lässt sie zu (der eProsima-Client
benutzt sie). 3aea52dd70a, bdf3f6ee966.

### §7.7.3.10.1 topic_ref als Name und data_writer_qos

**Spec:** §7.7.3.10.1, S. 36–37 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint_from_xml`: `topic_ref`,
`datawriter_qos` / `data_writer_qos`); `crates/xrce-agent/src/daemon/session.rs`
(`resolve_topic`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_data_writer_states_its_qos_over_a_profile`

**Status:** done — `topic_ref="Square"` wird als Name eines Topic des
Participant aufgelöst (`0x…` bleibt die ObjectId); `<datawriter_qos
base_name=…>` wendet das Profil an, Richtlinien des Elements gehen darüber. Die
Schreibweise `<data_writer_qos>` der Klausel liest der Code ebenfalls; die Tests
benutzen `<datawriter_qos>`. Das Beispiel (S. 36) nennt den Container „XRCE
Susbcriber“. 3aea52dd70a.

### §7.7.3.10.2 OBJK_DataWriter_Binary

**Spec:** §7.7.3.10.2, S. 37 — "struct OBJK_DataWriter_Binary { string
topic_name; OBJK_Endpoint_QosBinary endpoint_qos; @optional unsigned long
ownership_strength; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint`, `endpoint_qos`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`, Zweig `InBinary`,
`apply_endpoint_binary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_writer_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_writer_is_reliable_keep_last_transient_local`,
`crates/xrce-agent/src/daemon/session.rs::tests::annex_b_2_3_create_of_a_binary_data_writer`,
`crates/xrce-agent/src/daemon/session.rs::tests::annex_b_2_4_create_of_a_binary_data_writer_with_qos`

**Status:** done — `topic_name`, `endpoint_qos` und `ownership_strength`
(`unsigned long`). Annex A (APPENDABLE, `qos` optional) und die Klausel (FINAL,
`endpoint_qos` Pflicht) werden beide gelesen, Annex A zuerst, die Form des Blobs
entscheidet (kein Schalter); danach die Struktur des eProsima-Client (`ObjectId
topic_id`, `ownership_strength` mit 64 Bit). Die Beispiele Annex B.2.3 und B.2.4
sind Byte für Byte getestet. Die Fassung der Klausel ist für den DataWriter
nicht eigens getestet, für den DataReader (Item unten) schon. c67ce23c724,
33df0df0945.

### §7.7.3.10.2 EndpointQosFlags

**Spec:** §7.7.3.10.2, S. 37 — "@bit_bound(16) bitmask EndpointQosFlags {
@position(0) is_reliable, @position(1) is_history_keep_last, @position(2)
is_ownership_exclusive, @position(3) is_durability_transient_local,
@position(4) is_durability_transient, @position(5) is_durability_persistent,
};"

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`spec_flags`,
`apply_endpoint_binary`; `flags`, `endpoint_qos_from_binary`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_writer_is_reliable_keep_last_transient_local`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_binary_writer_with_every_optional_member_reads_back`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_writer_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_clause_binary_reader_names_its_topic_and_qos`

**Status:** done — Bit 1 ist je Fassung `is_history_keep_last` (Klausel,
eProsima-Client) oder `is_history_keep_all` (Annex A, S. 110); die Fassung des
Blobs entscheidet (`EndpointBinary::bit1_is_keep_all`). Die Tests prüfen beide
Lesarten. c67ce23c724.

### §7.7.3.10.2 OBJK_Endpoint_QosBinary

**Spec:** §7.7.3.10.2, S. 37 — "struct OBJK_Endpoint_QosBinary {
EndpointQosFlags qos_flags; @optional unsigned short history_depth; @optional
unsigned long deadline_msec; @optional unsigned long lifespan_msec; @optional
sequence<octet> user_data; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint_qos`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_endpoint_binary`,
`endpoint_qos_from_binary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_binary_writer_with_every_optional_member_reads_back`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_binary_endpoint_without_qos_gets_the_role_defaults`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`

**Status:** done — `qos_flags`, `history_depth`, `deadline_msec`,
`lifespan_msec` und `user_data` (alle außer den Flags optional). c67ce23c724.

---

## §7.7.3.11 XRCE DataReader

### §7.7.3.11 DATAREADER_Representation mit subscriber_id

**Spec:** §7.7.3.11, S. 38 — "struct DATAREADER_Representation :
OBJK_RepresentationBinAndXML_Base { ObjectId subscriber_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`,
`crates/xrce/src/object_repr.rs::tests::annex_b_2_6_reader_with_default_qos`

**Status:** done — Kodierung.

### §7.7.3.11 DataReader gehört zum Subscriber

**Spec:** §7.7.3.11, S. 38 — "the member subscriber_id shall contain the
ObjectId of an XRCE Subscriber object. The referenced or created DataReader
will belong to the specified Subscriber."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, Zweig
DataWriter/DataReader)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_reads_through_a_preconfigured_data_reader_without_a_create`

**Status:** done — Art und Existenz des Subscriber werden geprüft; der
DataReader erbt PARTITION und GROUP_DATA des Subscriber.

### §7.7.3.11.1 XML mit einem data_reader und Filtern
**Spec:** §7.7.3.11.1, S. 38 — "the string_representation field shall contain
a single <data_reader> top-level XML element [...]. The XML representation of
a DataReader allows specifying Qos policies. [...] The XML representation of a
DataReader may also contain time-based and content-based filters."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint_from_xml`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_data_reader_in_xml_has_its_qos_and_its_content_filter`

**Status:** done — getestet sind die Attributform, die Form von Fast DDS mit `timeBasedFilter`, ein `<data_reader topic_ref=…>` aus der Konfigurationsdatei und `<data_reader_qos>` mit `<content_filter>` (0f85937a436). Eine BY_REFERENCE-Darstellung hat ein DataReader nach §7.7.3.1 nicht; `representation_formats: any` lässt sie zu.
### §7.7.3.11.1 data_reader_qos und content_filter
**Spec:** §7.7.3.11.1, S. 38 — "<content_filter name="MyFilter">
<expression> x &gt; 5 </expression> </content_filter>"

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint_from_xml`, `apply_qos_profile`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_data_reader_in_xml_has_its_qos_and_its_content_filter`

**Status:** done — `<content_filter><expression>` setzt den Filter des Reader (das Attribut `name` wird nicht verwendet), `<datareader_qos>`/`<data_reader_qos>` wenden Profil und eigene Richtlinien an; der Test liest beide aus XML und prüft QoS und Filter am DDS-Port (0f85937a436, 3aea52dd70a).
### §7.7.3.11.2 OBJK_DataReader_Binary

**Spec:** §7.7.3.11.2, S. 38–39 — "struct OBJK_DataReader_Binary { string
topic_name; OBJK_Endpoint_QosBinary endpoint_qos; @optional unsigned long
timebasedfilter_msec; @optional string contentbased_filter; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint`, `endpoint_qos`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`, Zweig `InBinary`,
`apply_endpoint_binary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::reader_binary_of_the_clause`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_clause_binary_reader_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_reader_is_reliable_keep_last_transient_local`

**Status:** done — `topic_name`, `endpoint_qos`, `timebasedfilter_msec` und
`contentbased_filter` (als Filter des Reader angewandt). Fassung der Klausel und
Annex A werden wie beim DataWriter gelesen; Annex B.2.6 und B.2.7 sind Byte für
Byte getestet. Im Beispiel B.2.7 nennt der Text fünf Minuten, die Tabelle 180000
ms; gelesen werden die Oktette. c67ce23c724, 33df0df0945.

---

## §7.7.4 ObjectId

### §7.7.4 ObjectId als octet[2] je Client und Agent

**Spec:** §7.7.4, S. 39 — "Each ObjectId is scoped to an XRCE Client and Agent
pair. [...] typedef octet ObjectId [2];"

**Repo:** `crates/xrce/src/object_id.rs` (`ObjectId::to_bytes`, `ObjectId::from_bytes`)

**Tests:** `crates/xrce/src/object_id.rs::tests::bytes_are_big_endian`,
`crates/xrce/src/object_info.rs::tests::base_object_request_roundtrip`

**Status:** done — die beiden Oktette in Reihenfolge, unabhängig vom E-Flag.

---

## §7.7.5 ObjectKind

### §7.7.5 Werte von ObjectKind

**Spec:** §7.7.5, S. 39 — "XRCE objects are classified into 14 kinds. [...]
typedef octet ObjectKind; const ObjectKind OBJK_INVALID = 0x00; [...]
OBJK_PARTICIPANT = 0x01; OBJK_TOPIC = 0x02; OBJK_PUBLISHER = 0x03;
OBJK_SUBSCRIBER = 0x04; OBJK_DATAWRITER = 0x05; OBJK_DATAREADER = 0x06;
OBJK_TYPE = 0x0A; OBJK_QOSPROFILE = 0x0B; OBJK_APPLICATION = 0x0C; OBJK_AGENT =
0x0D; OBJK_CLIENT = 0x0E;"

**Repo:** `crates/xrce/src/object_kind.rs` (Konstanten `OBJK_*`, `ObjectKind::from_u8`)

**Tests:** `crates/xrce/src/object_kind.rs::tests::all_spec_kinds_roundtrip`,
`crates/xrce/src/object_kind.rs::tests::raw_const_values_match_spec`,
`crates/xrce/src/object_kind.rs::tests::unknown_byte_rejected`

**Status:** done — 13 Werte: die zwölf des Textes und `OBJK_OTHER = 0x0F` (Annex
A). Der Text nennt 14 Arten und listet 12. f63fa2d1218.

---

## §7.7.6 ObjectIdPrefix

### §7.7.6 ObjectId aus Präfix und ObjectKind

**Spec:** §7.7.6, S. 39 — "The ObjectId of an object is composed combining 12
bits from the ObjectIdPrefix and four bits from the ObjectKind. [...] typedef
octet ObjectIdPrefix [2]; [...] object_id[0] = objectid_prefix[0]
object_id[1] = (objectid_prefix[1]&0xF0) + object_kind"

**Repo:** `crates/xrce/src/object_id.rs` (`ObjectId::new`, `ObjectId::kind`,
`ObjectId::from_reference`)

**Tests:** `crates/xrce/src/object_id.rs::tests::new_packs_kind_into_lower_4_bits`,
`crates/xrce/src/object_id.rs::tests::bytes_are_big_endian`,
`crates/xrce/src/object_id.rs::tests::from_reference_matches_the_examples_of_the_spec`

**Status:** done — der Präfix ist im Code die 12-Bit-Zahl `raw_id`; ein Typ
`ObjectIdPrefix` fehlt. `ObjectId::from_reference` bildet den Präfix aus den
ersten 12 Bit des MD5 (§9.3, 03371b4965c). `ObjectId::new_with_mask` und
`kind_mask` deuten Bit 15 als „vom Client vergeben“; das steht nicht in der Spec
und wird von der Sitzung nicht benutzt.

---

## §7.7.7 ResultStatus

### §7.7.7 StatusValue

**Spec:** §7.7.7, S. 40 — "@bit_bound(8) enum StatusValue { @value(0x00)
STATUS_OK, @value(0x01) STATUS_OK_MATCHED, @value(0x80) STATUS_ERR_DDS_ERROR,
@value(0x81) STATUS_ERR_MISMATCH, @value(0x82) STATUS_ERR_ALREADY_EXISTS,
@value(0x83) STATUS_ERR_DENIED, @value(0x84) STATUS_ERR_UNKNOWN_REFERENCE,
@value(0x85) STATUS_ERR_INVALID_DATA, @value(0x86) STATUS_ERR_INCOMPATIBLE,
@value(0x87) STATUS_ERR_RESOURCES };"

**Repo:** `crates/xrce/src/object_info.rs` (`ResultStatusCode`)

**Tests:** `crates/xrce/src/object_info.rs::tests::result_status_code_all_ten_spec_values_roundtrip`,
`crates/xrce/src/object_info.rs::tests::result_status_code_unknown_byte_rejected`

**Status:** done

### §7.7.7 ResultStatus

**Spec:** §7.7.7, S. 40 — "struct ResultStatus { StatusValue status; octet
implementation_status; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ResultStatus`, `encode_into`, `decode_at`);
`crates/xrce/src/encoding.rs` (`UnannotatedTypes`)

**Tests:** `crates/xrce/src/object_info.rs::tests::result_status_roundtrip`,
`crates/xrce/src/object_info.rs::tests::result_status_decode_short_returns_eof`,
`crates/xrce/tests/unannotated_types.rs::the_default_is_appendable_with_a_dheader`,
`crates/xrce/tests/unannotated_types.rs::the_final_setting_writes_the_reference_implementations_octets`,
`crates/xrce/tests/unannotated_types.rs::both_forms_are_read_in_either_setting`

**Status:** done — Annex A gibt `ResultStatus` kein `@extensibility`: Standard
ist APPENDABLE mit DHEADER nach DDS-XTypes 1.3 §7.3.1.2.1.8 (6 Oktette, im
`BaseObjectReply` 10). Die Oktette der Referenzimplementierung (2 und 6, ohne
DHEADER) schreibt die Einstellung `encoding::UnannotatedTypes::Final`
(Agent-Schlüssel `unannotated_types: final`); beide Fassungen werden in jeder
Einstellung gelesen. f2ace917b5a, 3874ad8d6a6.

### §7.7.7 Table 2 STATUS_OK

**Spec:** §7.7.7, Table 2, S. 41 — "Indicates a successful execution of the
operation".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`status_reply`, `agent_status_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_successful_write_is_answered_with_status_ok`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done — auch ein erfolgreiches WRITE_DATA wird mit STATUS_OK
beantwortet (dc0209b38fe); der eProsima-Agent antwortet darauf nicht.

### §7.7.7 Table 2 STATUS_OK_MATCHED

**Spec:** §7.7.7, Table 2, S. 41 — "Indicates a successful execution of a
create or update operation on a resource when the resource already existed on
the Agent and the resource state already matched the one requested by the
operation."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Reused`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create`)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_reuse_returns_equal_marker`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`

**Status:** done — eine Darstellung, die der des vorhandenen Objekts gleicht,
ergibt bei `reuse` STATUS_OK_MATCHED und ändert nichts. Table 5 schreibt
STATUS_OK_MATCHES, `StatusValue` in Annex A und im Code heißt STATUS_OK_MATCHED
(nur der Name). 8b949a13ef5.

### §7.7.7 Table 2 STATUS_ERR_DDS_ERROR

**Spec:** §7.7.7, Table 2, S. 41 — "Indicates a failure in the execution of
the operation caused by an error when creating or operating on the DDS
resource related to the operation."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`dds_port_error_status`,
`apply_dds_create`); `crates/xrce-agent/src/daemon/dds_port.rs`
(`DdsPortError::DdsError`, `DdsPort::enable`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_dds_entity_is_enabled_when_the_creation_has_succeeded`

**Status:** done — kann die DDS-Seite eines CREATE nicht freigegeben (`enable`)
oder angelegt werden, ist die Antwort STATUS_ERR_DDS_ERROR, nichts wird abgelegt
und die DDS-Seite gelöscht. e4d37a6aff6.

### §7.7.7 Table 2 STATUS_ERR_MISMATCH

**Spec:** §7.7.7, Table 2, S. 41 — "Indicates a failure in the execution of a
create or update operation on a resource when the resource already existed on
the Agent, the state did not match the one requested by the operation, and it
was not possible to change the state of the resource."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Reused { equal: false }`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create`)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_reuse_returns_equal_marker`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`

**Status:** done — `reuse` ohne `replace` und eine ungleiche Darstellung ergeben
STATUS_ERR_MISMATCH; mit `replace` wird das Objekt ersetzt (Table 5).
8b949a13ef5.

### §7.7.7 Table 2 STATUS_ERR_ALREADY_EXISTS

**Spec:** §7.7.7, Table 2, S. 41 — "Indicates a failure in the execution of a
create operation due to the fact that the resource already existed."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Conflict`)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_strict_on_existing_returns_conflict`

**Status:** done

### §7.7.7 Table 2 STATUS_ERR_DENIED

**Spec:** §7.7.7, Table 2, S. 41 — "Indicates a failure in the execution of an
operation due to lack of permissions."

**Repo:** `crates/xrce-agent/src/daemon/access.rs` (`AccessController`);
`crates/xrce-agent/src/daemon/session.rs` (Prüfung vor CREATE, DELETE,
GET_INFO, WRITE_DATA, READ_DATA)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_key_not_configured_in_the_agent_is_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_without_permission_is_denied_and_stores_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::write_and_read_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::client_properties_are_checked_against_what_the_agent_requires`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_update_needs_the_permission_to_update`

**Status:** done — ein ClientKey, der nicht in `authorized_client_keys` steht,
eine fehlende Client-Property und eine Operation ohne Recht ergeben
STATUS_ERR_DENIED; nichts wird abgelegt. d66ada44616, 515bcc677f3.

### §7.7.7 Table 2 STATUS_ERR_UNKNOWN_REFERENCE

**Spec:** §7.7.7, Table 2, S. 41 — "Indicates a failure in the execution of an
operation due to the fact that the referenced resource is not known to the
Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`dds_port_error_status`,
`handle_delete`, `handle_get_info`, `participant_of_create`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::full_chain_against_a_real_leased_domain`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::null_port_rejects_topic_before_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_or_subscriber_needs_an_existing_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** done — die Tests prüfen `DdsPortError::UnknownReference`, einen
fehlenden Participant, ein fehlendes Objekt in GET_INFO und das Löschen des
Client ohne ProxyClient; `dds_port_error_status` bildet den Port-Fehler auf den
Wert ab.

### §7.7.7 Table 2 STATUS_ERR_INVALID_DATA

**Spec:** §7.7.7, Table 2, S. 41 — "Indicates a failure in the execution due
to wrong on invalid input parameter data."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`,
`handle_read_data`, `refuse_unreadable`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_request_that_cannot_be_read_is_answered_invalid_data`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_without_a_base_object_request_is_answered`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`

**Status:** done — ein nicht lesbarer Body von CREATE, READ_DATA, WRITE_DATA,
DELETE und GET_INFO, eine Darstellung in einem Format, das die Art nicht hat,
und ein nicht erfüllbarer Filter ergeben STATUS_ERR_INVALID_DATA. 133a0db3c4f,
bdf3f6ee966.

### §7.7.7 Table 2 STATUS_ERR_INCOMPATIBLE

**Spec:** §7.7.7, Table 2, S. 41 — "Indicates a failure in the execution of an
operation due to an incompatibility between the Client and the Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`,
`handle_get_info`: Version in `client_info`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_rejects_wrong_major_version`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_checks_the_version_in_client_info`

**Status:** done

### §7.7.7 Table 2 STATUS_ERR_RESOURCES
**Spec:** §7.7.7, Table 2, S. 41 — "Indicates a failure in the execution of an
operation due to a resource error on the Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`, `create_core`, `handle_message`, `emit_possibly_fragmented`, `data_submessages`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::create_client_over_max_clients_is_a_resource_error`, `crates/xrce-agent/tests/spec_items_session.rs::create_over_max_objects_per_client_is_a_resource_error`, `crates/xrce-agent/tests/spec_items_session.rs::a_full_reorder_buffer_of_a_reliable_stream_is_a_resource_error`, `crates/xrce-agent/tests/spec_items_session.rs::a_fragment_run_over_its_limit_is_a_resource_error`, `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_too_large_for_data_is_reported`

**Status:** done — der Agent sendet den Wert bei `max_clients`, `max_objects_per_client`, vollem Puffer des Empfängers, einer FRAGMENT-Zusammensetzung über der Grenze und einem Sample, das nicht in ein DATA passt; jeder dieser Wege hat einen Test (2fa0800f55a, 0f85937a436). Ein INFO, das nicht kodiert werden kann, hat keinen Test.

---

## §7.7.8 BaseObjectRequest

### §7.7.8 BaseObjectRequest

**Spec:** §7.7.8, S. 41 — "@extensibility(FINAL) struct BaseObjectRequest {
RequestId request_id; ObjectId object_id; };"

**Repo:** `crates/xrce/src/object_info.rs` (`BaseObjectRequest`)

**Tests:** `crates/xrce/src/object_info.rs::tests::base_object_request_roundtrip`,
`crates/xrce/src/object_info.rs::tests::base_object_request_decode_short_returns_eof`

**Status:** done — FINAL, 4 Oktette ohne DHEADER.

### §7.7.8 request_id verknüpft Antwort und Anfrage

**Spec:** §7.7.8, S. 41–42 — "request_id (RequestId) identifies each request.
It is used to correlate a reply with the related request. It is scoped to each
XRCE Client and Agent pair. Note that it is possible to reuse a value of the
request_id for future requests as long as the previous request with that value
is known by Client and Agent to no longer be active."

**Repo:** `crates/xrce-client/src/lib.rs` (`allocate_request_id`, `request_id_octets`);
`crates/xrce-agent/src/daemon/session.rs` (`status_reply`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::full_lifecycle_creates_unique_request_ids_and_real_submessages`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §7.7.8 object_id als Ziel und Präfix

**Spec:** §7.7.8, S. 42 — "object_id (ObjectId) the ObjectId that is the target
of the request. For requests that create objects, the object_id conveys the
ObjectIdPrefix for the created object."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`: `ObjectId` aus
Präfix der Anfrage und Art der Darstellung)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_object_id_is_the_prefix_and_the_kind_of_the_representation`

**Status:** done — für die Bildung der `ObjectId` beim CREATE siehe das Item zu
§7.8.3.1 „input_objectid“. e4d37a6aff6.

---

## §7.7.9 BaseObjectReply

### §7.7.9 BaseObjectReply

**Spec:** §7.7.9, S. 42 — "@extensibility(FINAL) struct BaseObjectReply {
BaseObjectRequest related_request; ResultStatus result; };"

**Repo:** `crates/xrce/src/object_info.rs` (`BaseObjectReply`)

**Tests:** `crates/xrce/src/object_info.rs::tests::base_object_reply_roundtrip`,
`crates/xrce/tests/unannotated_types.rs::both_forms_are_read_in_either_setting`

**Status:** done — FINAL: `related_request` (4 Oktette) und `result`; das
`ResultStatus` darin trägt seinen DHEADER (Item `ResultStatus`).

### §7.7.9 related_request

**Spec:** §7.7.9, S. 42 — "related_request contains the request_id and
object_id of the request that caused the reply to be sent [...]. For requests
that create objects, the object_id conveys the desired ObjectId for the
created object."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`status_reply`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §7.7.9 status bei Erfolg und Fehler

**Spec:** §7.7.9, S. 42 — "If the operation succeeded the StatusValue shall be
set to STATUS_OK or STATUS_OK_MATCHED. If it failed it shall be set to the
value that corresponds to the type of error encountered."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`: Abbildung von
`CreateOutcome` auf den Status)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`

**Status:** done — der Text nennt die Member `status` und
`implementation_status`; die Struktur hat `result`.

### §7.7.9 implementation_status

**Spec:** §7.7.9, S. 42 — "implementation_status (octet) provides an
implementation-specific (vendor-specific) return status. The value is scoped by
the XrceVendorId of the Agent. It shall only be interpreted by clients that
understand the implementation status values of the XrceVendorId of the Agent
that returned it."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`status_reply`: Wert 0);
`crates/xrce-client/src/lib.rs` (`handle_status` wertet ihn nicht aus)

**Tests:** `crates/xrce/src/object_info.rs::tests::result_status_roundtrip`

**Status:** done

---

## §7.7.10 RelatedObjectRequest

### §7.7.10 RelatedObjectRequest als typedef

**Spec:** §7.7.10, S. 42 — "typedef RelatedObjectRequest BaseObjectRequest; The
interpretation is the same as for the related_request that appears in the
BaseObjectReply".

**Repo:** `crates/xrce/src/object_info.rs` (`RelatedObjectRequest`)

**Tests:** `crates/xrce/src/object_info.rs::tests::related_object_request_is_base_object_request`

**Status:** done — die Spec schreibt den `typedef` in umgekehrter Reihenfolge;
gemeint ist `typedef BaseObjectRequest RelatedObjectRequest`.

---

## §7.7.11 CreationMode

### §7.7.11 CreationMode

**Spec:** §7.7.11, S. 43 — "struct CreationMode { boolean reuse; boolean
replace; };"

**Repo:** `crates/xrce/src/object_store.rs` (`CreationMode`);
`crates/xrce/src/submessages/create.rs` (`CREATE_FLAG_REUSE`,
`CREATE_FLAG_REPLACE`)

**Tests:** `crates/xrce/src/submessages/create.rs::tests::create_roundtrip_carries_reuse_replace_flags`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`

**Status:** done — auf der Leitung als Flags von CREATE (§8.3.5.2.1); `reuse`
und `replace` wirken nach Table 5 (8b949a13ef5).

---

## §7.7.12 ActivityInfoVariant

### §7.7.12 InfoMask

**Spec:** §7.7.12, S. 43 — "bitmask InfoMask { @position(0)
INFO_CONFIGURATION, @position(1) INFO_ACTIVITY };"

**Repo:** `crates/xrce/src/object_info.rs` (`InfoMask`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`, `object_info`:
Configuration und Activity nach der Maske)

**Tests:** `crates/xrce/src/object_info.rs::tests::info_mask_bits_and_wire_form`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_writer_reports_its_activity`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_an_info_mask_is_invalid_data`

**Status:** done — `InfoMask` ist eine 32-Bit-Bitmaske (`INFO_CONFIGURATION` Bit
0, `INFO_ACTIVITY` Bit 1, ohne `@bit_bound` nach XTypes 32 Bit) und wird aus dem
GET_INFO gelesen; ein GET_INFO ohne Maske ergibt STATUS_ERR_INVALID_DATA.
65cd1820a04.

### §7.7.12 AGENT_ActivityInfo

**Spec:** §7.7.12, S. 43 — "@extensibility(APPENDABLE) struct
AGENT_ActivityInfo { short availability; TransportLocatorSeq address_seq; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ActivityInfoVariant::Agent`);
`crates/xrce-agent/src/daemon/session.rs` (`agent_object_info`)

**Tests:** `crates/xrce/src/object_info.rs::tests::agent_activity_info_has_a_dheader_and_a_locator_sequence`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_a_session_is_answered_for_the_agent_only`

**Status:** done — `@extensibility(APPENDABLE)`: Diskriminator `OBJK_AGENT`
(0x0D; Annex A schreibt `OBJECTID_AGENT`, einen ObjectId mit zwei Oktetten und
keinen Wert von `ObjectKind`), DHEADER, `short availability` und
`TransportLocatorSeq` aus den Listenern des Agent. 65cd1820a04, c304e509660.

### §7.7.12 DATAREADER_ActivityInfo

**Spec:** §7.7.12, S. 43 — "@extensibility(APPENDABLE) struct
DATAREADER_ActivityInfo { short highest_acked_num; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ActivityInfoVariant::DataReader`
Z. 524, `encode_into` Z. 574, `decode_at` Z. 633);
`crates/xrce-agent/src/daemon/session.rs` (`object_info` Z. 3002)

**Tests:** `crates/xrce/src/object_info.rs::tests::data_reader_activity_info_is_a_dheader_and_a_short`,
`crates/xrce/src/object_info.rs::tests::appended_members_of_an_activity_are_skipped`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_reader_reports_its_activity`

**Status:** `done` — DHEADER und `short highest_acked_num`; Member, die eine
neuere Fassung anhängt, überspringt der Leser über das DHEADER. Der Agent sendet
die Aktivität im INFO auf ein GET_INFO eines DataReader: `highest_acked_num` ist
die höchste vom Client bestätigte Sequenznummer des Reliable Streams, auf dem
das READ_DATA liefert, sonst 0. Die Spec definiert den Wert nicht; gegen keinen
anderen Agent gemessen. Commit 65cd1820a04.

### §7.7.12 DATAWRITER_ActivityInfo

**Spec:** §7.7.12, S. 43 — "@extensibility(APPENDABLE) struct
DATAWRITER_ActivityInfo { unsigned long long sample_seq_num; short
stream_seq_num; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ActivityInfoVariant::DataWriter`
Z. 529, `encode_into` Z. 574, `decode_at` Z. 633);
`crates/xrce-agent/src/daemon/session.rs` (`object_info` Z. 3002,
`dds_write_and_report` Z. 3282)

**Tests:** `crates/xrce/src/object_info.rs::tests::data_writer_activity_info_is_a_dheader_a_long_long_and_a_short`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_writer_reports_its_activity`

**Status:** `done` — DHEADER, `unsigned long long sample_seq_num` (auf 4
ausgerichtet) und `short stream_seq_num`. Der Agent zählt je DataWriter die
geschriebenen Samples (`sample_seq_num`) und merkt sich die Sequenznummer im
Header der letzten WRITE_DATA-Nachricht (`stream_seq_num`). Die Spec definiert
beide Werte nicht; gegen keinen anderen Agent gemessen. Commit 65cd1820a04.

### §7.7.12 ActivityInfoVariant als Union

**Spec:** §7.7.12, S. 43–44 — "union ActivityInfoVariant (ObjectKind) { case
OBJK_DATAWRITER : DATAWRITER_ActivityInfo data_writer; case OBJK_DATAREADER :
DATAREADER_ActivityInfo data_reader; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ActivityInfoVariant` Z. 512,
`discriminator` Z. 540, `encode_into` Z. 574, `decode_at` Z. 633)

**Tests:** `crates/xrce/src/object_info.rs::tests::agent_activity_info_has_a_dheader_and_a_locator_sequence`,
`crates/xrce/src/object_info.rs::tests::activity_info_decode_unknown_discriminator_rejected`,
`crates/xrce/src/object_info.rs::tests::activity_info_decode_truncated_rejected`,
`crates/xrce/src/object_info.rs::tests::appended_members_of_an_activity_are_skipped`

**Status:** `done` — der Diskriminator ist das Oktett der `ObjectKind`
(OBJK_DATAWRITER 0x05, OBJK_DATAREADER 0x06); die Member tragen das DHEADER; ein
unbekannter Diskriminator ist ein Fehler. Annex A (S. 113) hat zusätzlich den
Fall des Agent und schreibt `OBJECTID_AGENT`, eine ObjectId aus zwei Oktetten
und kein Wert von `ObjectKind`; der Code nimmt `OBJK_AGENT` (0x0D) als
Diskriminator von `AGENT_ActivityInfo`. Der IDL der Klausel fehlt das Wort
`switch`. Commit 65cd1820a04.

---

## §7.7.13 ObjectInfo

### §7.7.13 ObjectInfo mit activity und config

**Spec:** §7.7.13, S. 44 — "@extensibility(FINAL) struct ObjectInfo {
@optional ActivityInfoVariant activity; @optional ObjectVariant config; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ObjectInfo` Z. 760, `encode_into`
Z. 794, `decode_at` Z. 834, `ObjectConfig` Z. 690);
`crates/xrce-agent/src/daemon/session.rs` (`object_info` Z. 3002, `answer_info`
Z. 2962)

**Tests:** `crates/xrce/src/object_info.rs::tests::object_info_puts_the_activity_before_the_config`,
`crates/xrce/src/object_info.rs::tests::the_config_of_a_topic_is_the_object_variant_it_was_created_with`,
`crates/xrce/src/object_info.rs::tests::the_config_of_the_agent_is_its_agent_representation`,
`crates/xrce/src/object_info.rs::tests::an_empty_object_info_is_two_presence_octets`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`

**Status:** `done` — `activity` vor `config`, je ein Merker-Oktett für die
beiden `@optional`; `config` ist ein `ObjectVariant` mit seiner `ObjectKind`
(beim Agent die `AGENT_Representation`, bei einem Objekt die Repräsentation, mit
der es angelegt wurde). Die Ausrichtung zählt ab dem Beginn der Payload, die
Byte-Reihenfolge folgt dem E-Flag. Commits 65cd1820a04, b43d5e6b848.

---

## §7.7.14 ReadSpecification

### §7.7.14 DataDeliveryControl

**Spec:** §7.7.14, S. 44 — "@extensibility(APPENDABLE) struct
DataDeliveryControl { unsigned short max_samples; unsigned short
max_elapsed_time; unsigned short max_bytes_per_second; unsigned short
min_pace_period; // milliseconds };"

**Repo:** `crates/xrce/src/submessages/read_data.rs` (`DataDeliveryControl`
Z. 60, `encode_into` Z. 79, `decode` Z. 93, `DeliveryControlForm` Z. 127);
`crates/xrce-client/src/lib.rs` (`XrceClient::set_delivery_control_form` Z. 372)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::read_data_matches_the_captured_read_data_example`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`

**Status:** `done` — vier `unsigned short` in dieser Reihenfolge, mit DHEADER
(APPENDABLE). Die Form ohne DHEADER, die der Mitschnitt von eProsima zeigt, ist
die benannte Einstellung `DeliveryControlForm::Bare`
(`XrceClient::set_delivery_control_form`); der Leser nimmt beide Formen. Commit
1663f8f6240.

### §7.7.14 DHEADER von DataDeliveryControl

**Spec:** §7.7.14, S. 44 — wie oben; Annex B.4.2, S. 151–152, zeigt das
DHEADER.

**Repo:** `crates/xrce/src/submessages/read_data.rs`
(`ReadSpecification::encode_into` Z. 151, `decode` Z. 204)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`,
`crates/xrce/src/submessages/read_data.rs::tests::read_data_matches_the_captured_read_data_example`

**Status:** `done` — geschrieben wird ein DHEADER mit der Länge 8, auf 4
ausgerichtet. Annex B.4.2 setzt im DHEADER zusätzlich das Bit 31 (0x80000008),
das XCDR2 im DHEADER nicht kennt; der Leser nimmt es nicht als Teil der Länge.
Gelesen wird mit und ohne DHEADER. Commit 1663f8f6240.

### §7.7.14 ReadSpecification

**Spec:** §7.7.14, S. 44 — "@extensibility(FINAL) struct ReadSpecification {
StreamId preferred_stream_id; DataFormat data_format; @optional string
content_filter_expression; @optional DataDeliveryControl delivery_control;
};"

**Repo:** `crates/xrce/src/submessages/read_data.rs` (`ReadSpecification`
Z. 114, `encode_into` Z. 151, `decode` Z. 204, `ReadDataPayload` Z. 312)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::read_data_matches_the_captured_read_data_example`,
`crates/xrce/src/submessages/read_data.rs::tests::read_data_full_spec_roundtrip`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`

**Status:** `done` — Reihenfolge der Felder, Merker der beiden `@optional`,
Werte von `data_format` nach Annex A und XCDR2-Zeichenkette (siehe die Items zu
data_format, content_filter_expression und DataDeliveryControl). Commit
1663f8f6240.

### §7.7.14 Werte von data_format

**Spec:** §7.7.14, S. 44; Annex A, S. 113–114 — "const DataFormat FORMAT_DATA
= 0x00; const DataFormat FORMAT_SAMPLE = 0x02; const DataFormat
FORMAT_DATA_SEQ = 0x08; const DataFormat FORMAT_SAMPLE_SEQ = 0x0A; const
DataFormat FORMAT_PACKED_SAMPLES = 0x0E;"

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`DataFormat` Z. 52, `raw`
Z. 69, `from_raw` Z. 78); `crates/xrce/src/submessages/read_data.rs`
(`ReadSpecification::encode_into` Z. 151);
`crates/xrce-agent/src/daemon/session.rs` (`data_submessages` Z. 3581)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::the_data_format_octet_has_the_values_of_annex_a`,
`crates/xrce/src/submessages/read_data.rs::tests::read_spec_rejects_reserved_data_format`,
`crates/xrce/src/submessages/write_data.rs::tests::data_format_raw_roundtrip`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_data_seq_is_answered_with_one_data_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_is_answered_with_a_sample_each`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_seq_is_answered_with_one_sample_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_packed_samples_is_answered_with_one_packed_samples`

**Status:** `done` — `DataFormat::raw` gibt 0x00, 0x02, 0x08, 0x0A und 0x0E;
`from_raw` liest dieselben Werte, Bits außerhalb von FORMAT_MASK (0x0E) und
reservierte Kombinationen sind `ValueOutOfRange`. Der Agent liefert DATA im
verlangten Format. Commits 1663f8f6240, d5fbdbe672e.

### §7.7.14 Kodierung von content_filter_expression

**Spec:** §7.7.14, S. 44 — "@optional string content_filter_expression;"
(XCDR2 nach §7.7); Annex B.4.2, S. 151–152: Füllbyte 15, Länge 6 für "x>100".

**Repo:** `crates/xrce/src/submessages/read_data.rs`
(`ReadSpecification::encode_into` Z. 151, `decode` Z. 204)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::the_content_filter_is_a_string_with_a_nul_and_an_aligned_length`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`

**Status:** `done` — XCDR2-Zeichenkette: Länge als `unsigned long`, auf 4
ausgerichtet (ab Beginn der Payload), zählt das NUL mit; das NUL folgt den
Zeichen. Annex B.4.2 (Füllbyte 15, Länge 6 für "x>100") ist durch einen Test mit
den Bytes festgehalten. Commit 1663f8f6240.

---

## §7.8.1 Use of the ClientKey

### §7.8.1 ClientKey identifiziert den Client und trägt Rechte

**Spec:** §7.8.1, S. 44 — "The ClientKey is assigned to each client. The
ClientKey uniquely identifies the client to a particular agent. The ClientKey
is associated with a set of permissions for the client within the agent."

**Repo:** `crates/xrce-agent/src/lib.rs` (`XrceAgent::clients` Z. 102);
`crates/xrce-agent/src/daemon/transport.rs` (`PeerTable::resolve` Z. 111);
`crates/xrce-agent/src/daemon/access.rs` (`AccessController::permits` Z. 212,
`Rule` Z. 125)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::multiple_clients_isolated`,
`crates/xrce-agent/src/daemon/transport.rs::tests::peer_table_resolves_header_client_key_directly`,
`crates/xrce-agent/src/daemon/access.rs::tests::a_controller_without_rules_restricts_nothing`,
`crates/xrce-agent/src/daemon/access.rs::tests::the_first_matching_rule_decides_and_no_match_denies`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_without_permission_is_denied_and_stores_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::write_and_read_without_permission_are_denied`

**Status:** `done` — der ClientKey identifiziert den Client. Die Rechte stehen
im AccessController (§7.4): Regeln je ClientKey für Operation, ObjectKind,
DDS-Domain und Topic (Konfigurationsschlüssel `access_rules`); die erste
passende Regel entscheidet, ohne Treffer wird verweigert, ohne Regelwerk ist
alles erlaubt. Eine Operation ohne Recht ergibt STATUS_ERR_DENIED. Die Regeln
werden beim Start gelesen; ein Neuladen mit SIGHUP ändert sie nicht. Commits
d66ada44616, 515bcc677f3.

### §7.8.1 ClientKey ist geheim und in Client und Agent konfiguriert

**Spec:** §7.8.1, S. 45 — "The ClientKey shall be considered secret. It must be
configured both on the Client and in the Agent. The creation and configuration
are outside the scope of this specification."

**Repo:** `crates/xrce-agent/src/daemon/access.rs`
(`AccessController::client_authorized` Z. 193, `parse_client_key` Z. 227);
`crates/xrce-agent/src/daemon/config.rs` (`authorized_client_keys`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create_client` Z. 1227)

**Tests:** `crates/xrce-agent/src/daemon/access.rs::tests::only_configured_client_keys_are_authorized`,
`crates/xrce-agent/src/daemon/access.rs::tests::a_client_key_is_read_in_wire_order`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_key_not_configured_in_the_agent_is_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::without_configured_client_keys_every_key_is_accepted`

**Status:** `done` — Konfigurationsschlüssel `authorized_client_keys` (vier
Oktette in der Reihenfolge der Übertragung); ein ClientKey außerhalb der Liste
ergibt STATUS_ERR_DENIED und keine Sitzung, ohne Liste nimmt der Agent jeden
Schlüssel an. Der Client trägt seinen ClientKey ein (`XrceClient::new`). Der
Schutz des ClientKey auf dem Transport nach §8.3.2.4 sind die Listener `kind:
dtls` und `kind: tls`. Commits d66ada44616, 515bcc677f3, 01470e273bc.

### §7.8.1 ClientKey wird nicht interpretiert

**Spec:** §7.8.1, S. 45 — "The ClientKey shall not be interpreted."

**Repo:** `crates/xrce/src/header.rs` (`ClientKey` Z. 110)

**Tests:** `crates/xrce/src/handshake.rs::tests::client_representation_decodes_the_captured_create_client_prefix`

**Status:** `done`

### §7.8.1 Operationen nur mit bestehendem ProxyClient

**Spec:** §7.8.1, S. 45 — "With the exception of the operations create_client
and get_info on the XRCE Root, all other operations expect that the ClientKey
references an already exiting XRCE ProxyClient. If this is not the case, the
operation shall fail."

**Repo:** `crates/xrce-agent/src/lib.rs` (`XrceAgent::create_object` Z. 240,
`pull_sample` Z. 313: `AgentError::UnknownClient`);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` Z. 1583,
`handle_delete` Z. 2733)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::create_object_for_unknown_client_rejected`,
`crates/xrce-agent/src/lib.rs::tests::pull_sample_unknown_client_rejected`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** `done` — ein Vorgang für einen ClientKey ohne ProxyClient wird nicht
ausgeführt; die Sitzung antwortet mit `STATUS_ERR_UNKNOWN_REFERENCE`.

### §7.8.1 STATUS_ERR_NOCLIENT oder stilles Verwerfen
**Spec:** §7.8.1, S. 45 — "To avoid information leakage that could compromise
security, the failure to locate a ClientKey may in some cases result in a
returnValue having STATUS_ERR_NOCLIENT while in others it may silently drop the
connection to the client."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_message`, `drops_for_unknown_client`, `SessionEvent::NoClient`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_message_to_an_unknown_client_key_is_dropped_and_counted`

**Status:** done — §7.8.1 lässt zwei Wege: `STATUS_ERR_NOCLIENT` oder stilles Verwerfen. `StatusValue` (Annex A) hat den Wert nicht, der Agent verwirft die Nachricht ohne Antwort und meldet `SessionEvent::NoClient`; `create_client` und `get_info` sind ausgenommen, `delete_client` bekommt seine Antwort nach §7.8.2.3. 7356cbaaa8c.
### §7.8.1 Zähler für STATUS_ERR_NOCLIENT und Schließen der Verbindung
**Spec:** §7.8.1, S. 45 — "The Agent shall maintain a counter on the number of
times the STATUS_ERR_NOCLIENT was sent on an established connection, and once
a certain threshold is crossed it shall close the connection. The Agent may
subsequently refuse or throttle new connections originating from the same
client transport endpoint that was previously closed."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`PeerTable::strike`, `PeerTable::is_blocked`, `apply_events`); `crates/xrce-agent/src/daemon/config.rs` (`no_client_threshold`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::messages_to_an_unknown_client_key_close_the_connection_at_the_threshold`, `crates/xrce-agent/src/daemon/config.rs::tests::the_no_client_threshold_can_be_set`

**Status:** done — der Agent zählt die Nachrichten je Peer an einen unbekannten ClientKey; bei `no_client_threshold` (Vorgabe 16, 0 nie) wird die TCP-Verbindung geschlossen und ein UDP-Peer nicht mehr gelesen. 7356cbaaa8c.

---

## §7.8.2 XRCE Root

### §7.8.2 Root authentifiziert und legt ProxyClients an

**Spec:** §7.8.2, S. 45 — "The XRCE Root object represents the Agent. An XRCE
Agent is a singleton object that all agents shall instantiate. The XRCE Root
is responsible for authenticating client applications and creating the XRCE
ProxyClient object associated with each client."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine`,
`handle_create_client` Z. 1168; Prüfungen Z. 1227 und Z. 1240);
`crates/xrce-agent/src/daemon/access.rs` (`AccessController`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_key_not_configured_in_the_agent_is_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::client_properties_are_checked_against_what_the_agent_requires`

**Status:** `done` — der ProxyClient wird angelegt. Authentifiziert wird durch
den AccessController: konfigurierte ClientKeys (`authorized_client_keys`) und
geforderte Properties mit Name und Wert (`required_client_properties`), sonst
STATUS_ERR_DENIED und keine Sitzung. Commits d66ada44616, 515bcc677f3.

### §7.8.2 Table 3 Operationen der Root

**Spec:** §7.8.2, Table 3, S. 45 — `create_client`, `get_info`,
`delete_client`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Operationen stehen einzeln in
§7.8.2.1 bis §7.8.2.3.

### §7.8.2.1 create_client: Ein- und Ausgaben

**Spec:** §7.8.2.1, S. 45–46 — "client_representation (CLIENT_Representation):
a representation of the Client. [...] returnValue (ResultStatus) [...]
agent_info (AGENT_Representation): a representation of the Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
Z. 1168, `agent_status_reply` Z. 4331)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`

**Status:** `done` — STATUS_AGENT trägt `ResultStatus` und
`AGENT_Representation` (Item zu §8.3.5.5).

### §7.8.2.1 CLIENT_Representation mit client_timestamp

**Spec:** §7.8.2.1, S. 46 — "struct CLIENT_Representation { XrceCookie
xrce_cookie; XrceVersion xrce_version; XrceVendorId xrce_vendor_id; Time_t
client_timestamp; ClientKey client_key; SessionId session_id; @optional
PropertySeq properties; };"

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation` Z. 328,
`encode_form` Z. 407, `decode_form` Z. 463); `crates/xrce/src/encoding.rs`
(`RepresentationTimestamps` Z. 214); `crates/xrce-agent/src/daemon/config.rs`
(`representation_timestamps`); `crates/xrce-client/src/lib.rs`
(`XrceClient::set_representation_timestamps` Z. 364)

**Tests:** `crates/xrce/src/handshake.rs::tests::by_default_the_representations_have_no_timestamp`,
`crates/xrce/src/handshake.rs::tests::the_prose_form_has_a_time_t_after_the_vendor_id`,
`crates/xrce/src/handshake.rs::tests::client_representation_decodes_the_captured_create_client_prefix`,
`crates/xrce-client/src/lib.rs::tests::the_representation_timestamp_is_an_option_of_the_client`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_form_of_the_representations_can_be_set`

**Status:** `done` — die Spec widerspricht sich: der Text (§7.8.2.1, §8.3.5.1.2)
nennt `Time_t client_timestamp` nach `xrce_vendor_id`; das IDL von Annex A
(S. 105–106), die Bytes von Annex B.1 und der Mitschnitt von eProsima (16
Oktette) haben es nicht. Die Formen sind an den Bytes nicht zu unterscheiden.
Standard ist die Form des IDL; die Form mit Zeitstempel ist die benannte
Einstellung `representation_timestamps: absent | present` (Agent-Schlüssel),
`encoding::RepresentationTimestamps` (Rust-API),
`XrceClient::set_representation_timestamps` und die Option `representation
timestamps` im XRCE-Client jedes SDK. Der `Time_t` folgt `UnannotatedTypes` (im
Standard ein DHEADER mit der Länge 8). Die Tests prüfen die Bytes beider Formen.
Commits 711f93fd460, a8e8f6953a4.

### §7.8.2.1 AGENT_Representation mit agent_timestamp

**Spec:** §7.8.2.1, S. 46 — "struct AGENT_Representation { XrceCookie
xrce_cookie; XrceVersion xrce_version; XrceVendorId xrce_vendor_id; Time_t
agent_timestamp; @optional PropertySeq properties; };"

**Repo:** `crates/xrce/src/handshake.rs` (`AgentRepresentation` Z. 356,
`encode_form` Z. 554, `decode_form` Z. 599); `crates/xrce/src/encoding.rs`
(`RepresentationTimestamps` Z. 214)

**Tests:** `crates/xrce/src/handshake.rs::tests::the_agent_representation_follows_the_setting_the_same_way`,
`crates/xrce/src/handshake.rs::tests::agent_representation_decodes_the_captured_status_agent_body`,
`crates/xrce-client/src/lib.rs::tests::the_representation_timestamp_is_an_option_of_the_client`

**Status:** `done` — wie beim Item zu `client_timestamp`: Standard ist die Form
von Annex A (S. 106) ohne `agent_timestamp`; die Form mit dem Zeitstempel ist
die Einstellung `representation_timestamps: present`. Der Agent liest die
Einstellung beim Start (`server.rs`, `set_representation_timestamps`). Commits
711f93fd460, a8e8f6953a4.

### §7.8.2.1 Prüfung von xrce_cookie

**Spec:** §7.8.2.1, S. 46 — "Check the xrce_cookie to ensure it matches the
predefined XRCE_COOKIE constant. If it does not match the creation shall fail
and set the returnValue StatusValue to STATUS_ERR_INVALID_DATA."

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation::decode_at`
Z. 455); `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
Z. 1175)

**Tests:** `crates/xrce/src/submessages/create_client.rs::tests::refuses_a_payload_whose_cookie_is_not_xrce`,
`crates/xrce/src/handshake.rs::tests::client_representation_rejects_wrong_cookie`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`

**Status:** `done` — das Cookie wird geprüft; ein falsches ergibt ein
STATUS_AGENT mit STATUS_ERR_INVALID_DATA, es wird kein Client registriert.
Commit ed2ca7b49a9.

### §7.8.2.1 Antwort auf ein falsches Cookie als STATUS

**Spec:** §7.8.2.1, S. 46 — wie oben; §8.3.5.1, S. 63: "The XRCE Agent shall
send a STATUS_AGENT message in response".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
Z. 1175–1192, `agent_status_reply` Z. 4331)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_unreadable_property_seq_is_refused_with_invalid_data`

**Status:** `done` — die Antwort ist ein STATUS_AGENT (§8.3.5.1) mit dem
ClientKey des Headers, wenn der Header einen hat, sonst aus Nullen. Commit
ed2ca7b49a9.

### §7.8.2.1 Prüfung der Hauptversion

**Spec:** §7.8.2.1, S. 46 — "Check that the major version (xrce_version[0])
matches the XRCE_VERSION_MAJOR. If it does not match, the creation shall fail
and set the returnValue StatusValue to STATUS_ERR_INCOMPATIBLE."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
Z. 1212)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_rejects_wrong_major_version`

**Status:** `done`

### §7.8.2.1 client_key berechtigt

**Spec:** §7.8.2.1, S. 46 — "Check that the Client identified by the
client_key is authorized to connect to the XRCE Agent. If this check fails the
operation shall fail and set the returnValue StatusValue to
STATUS_ERR_DENIED."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
Z. 1227, `deny_create_client` Z. 1337); `crates/xrce-agent/src/daemon/access.rs`
(`AccessController::client_authorized` Z. 193)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_key_not_configured_in_the_agent_is_denied`,
`crates/xrce-agent/src/daemon/access.rs::tests::only_configured_client_keys_are_authorized`

**Status:** `done` — ein ClientKey außerhalb von `authorized_client_keys` ergibt
STATUS_ERR_DENIED; es entsteht keine Sitzung. Commits d66ada44616, 515bcc677f3.

### §7.8.2.1 Prüfung der properties

**Spec:** §7.8.2.1, S. 46 — "Check the Client properties, if present. These
may contain vendor-specific information that may prevent the Agent from
accepting the connection from the Client. [...] If this check fails the
operation shall fail and set the returnValue StatusValue to the appropriate
value."

**Repo:** `crates/xrce-agent/src/daemon/access.rs`
(`AccessController::properties_accepted` Z. 201); `crates/xrce/src/handshake.rs`
(`decode_properties` Z. 266); `crates/xrce-agent/src/daemon/session.rs`
(`handle_create_client` Z. 1240)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::client_properties_are_checked_against_what_the_agent_requires`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_unreadable_property_seq_is_refused_with_invalid_data`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_big_endian_create_client_with_properties_is_read_right`,
`crates/xrce/src/handshake.rs::tests::a_malformed_property_seq_is_an_error_not_absent_properties`

**Status:** `done` — verlangt der Agent Properties
(`required_client_properties`, Name und Wert), antwortet er ohne sie oder mit
anderem Wert mit STATUS_ERR_DENIED. Eine nicht lesbare PropertySeq ist ein
Fehler und wird mit STATUS_ERR_INVALID_DATA beantwortet; sie gilt nicht als
"keine Properties". Commits 515bcc677f3, 65cd1820a04.

### §7.8.2.1 Bestehender ProxyClient mit gleicher session_id

**Spec:** §7.8.2.1, S. 47 — "If a ProxyClient exists and has the same
session_id, then the operation shall not perform any action and shall set the
returnValue StatusValue to STATUS_OK."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
Z. 1274 `same_session`, Z. 1302–1332)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_same_session_id_reconnect_keeps_object_store`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_the_same_session_leaves_the_streams_alone`

**Status:** `done` — Objektspeicher und Streams bleiben, die Antwort ist
STATUS_OK. Die Variante, die die Streams zurücksetzt, ist die Einstellung des
folgenden Items. Commit ed2ca7b49a9.

### §7.8.2.1 Streams bei gleicher session_id

**Spec:** §7.8.2.1, S. 47 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
Z. 1290, `SessionEngineConfig::restart_on_same_session`);
`crates/xrce-agent/src/daemon/config.rs` (`same_session_create_client`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_the_same_session_leaves_the_streams_alone`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_the_same_session_can_restart_the_streams`,
`crates/xrce-agent/src/daemon/config.rs::tests::a_create_client_for_the_same_session_can_restart_the_streams`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done` — Standard `keep` (Spec): ein CREATE_CLIENT mit der
session_id des bestehenden ProxyClient "shall not perform any action", die
Streams laufen weiter. Benannte Einstellung `same_session_create_client:
restart`: der Agent verwirft die Zustände der Reliable Streams, der
Fragmentierung, den Rückstau und die Zähler der Best-Effort-Streams. Das
brauchen die Prozesse von eProsima, die ClientKey und `session_id`
wiederverwenden und bei der Sequenznummer 0 beginnen (Mitschnitt; der
Replay-Test setzt `restart`). Commit ed2ca7b49a9.

### §7.8.2.1 Bestehender ProxyClient mit anderer session_id

**Spec:** §7.8.2.1, S. 47 — "If a ProxyClient exists and has a different
session_id then the operation shall delete the existing XRCE ProxyClient
object and subsequently take the same actions as if there had not been a
ProxyClient associated with the client_key."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
Z. 1275)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_different_session_id_resets_object_store`

**Status:** `done`

### §7.8.2.1 Ausreichende Ressourcen
**Spec:** §7.8.2.1, S. 47 — "Check that there are sufficient internal
resources to complete the create operation. If there are not, then the
operation shall fail and set the returnValue StatusValue to
STATUS_ERR_RESOURCES."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::create_client_over_max_clients_is_a_resource_error`

**Status:** done — `handle_create_client` prüft `max_clients` und antwortet mit STATUS_ERR_RESOURCES; ein erneutes CREATE_CLIENT eines vorhandenen Clients bleibt STATUS_OK (0f85937a436).
### §7.8.2.1 Löschen setzt den Zustand der Kommunikation zurück

**Spec:** §7.8.2.1, S. 47 — "deletion of an existing ProxyClient resets any
prior communication state between the client and the agent. Any messages that
were cached pending acknowledgments shall be discarded."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
Z. 1275–1301, `handle_delete` Z. 2733)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_different_session_id_resets_object_store`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done`

### §7.8.2.1 Neuer ProxyClient: session_id, Streams ab 0, STATUS_OK

**Spec:** §7.8.2.1, S. 47 — "If the Agent creates a ProxyClient object it
shall: Initialize its state to have the specified session_id. Initialize the
built-in streams with sequence number 0. Set the returnValue StatusValue to
STATUS_OK. Return a representation of the XRCE Agent in the agent_info."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
Z. 1302–1332); `crates/xrce/src/reliable.rs` (`ReliableStreamState::new`:
`next_seq = 0`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::reliable_outbound_messages_get_real_incrementing_sequence_numbers`

**Status:** `done`

### §7.8.2.1 Zeitstempel für die Synchronisation

**Spec:** §7.8.2.1, S. 47 — "The Agent and Client may use the client_timestamp
and agent_timestamp to detect time-synchronization differences [...]. The use
of this information is left outside the scope of this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — außerhalb der Spec.

### §7.8.2.1 XrceVersion und XrceVendorId zur Konfiguration

**Spec:** §7.8.2.1, S. 47 — "The Agent and Client may use the XrceVersion and
XrceVendorId to further configure their protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis.

### §7.8.2.2 get_info der Root: Ein- und Ausgaben

**Spec:** §7.8.2.2, S. 47 — "info_mask (InfoMask): selects the kind of
information to retrieve. client_info (ObjectInfo): a representation of the
Client. [...] returnValue (ResultStatus) [...] agent_info (ObjectInfo): a
representation of the Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info` Z. 2838,
`agent_object_info` Z. 4242, `client_info_status` Z. 242);
`crates/xrce/src/object_info.rs` (`InfoMask` Z. 449);
`crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply` Z. 107);
`crates/xrce/src/submessages/get_info.rs` (`GetInfoPayload::with_client_info`
Z. 45)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_checks_the_version_in_client_info`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_info_mask_selects_activity_and_configuration`,
`crates/xrce/src/object_info.rs::tests::info_mask_bits_and_wire_form`

**Status:** `done` — INFO mit `ResultStatus` und dem `ObjectInfo` des Agent.
`info_mask` ist ein 32-Bit-Merker (`InfoMask`), den der Agent aus dem GET_INFO
liest; er wählt `activity` und `config`. `client_info`: `GET_INFO_Payload`
(§8.3.5.3.2, Annex A) hat dafür kein Member, §7.8.2.2 und Figure 10 (§8.4.4)
nennen es. Standard ist die Payload von Annex A;
`GetInfoPayload::with_client_info` hängt `client_info` hinter die InfoMask, der
Agent liest es, wenn es da ist (Item zu `client_info`). Commits 65cd1820a04,
5798b040952.

### §7.8.2.2 get_info vor create_client und über eine andere Verbindung

**Spec:** §7.8.2.2, S. 47 — "The operation get_info returns information on the
XRCE Agent and may be used prior to establishing a Session with the XRCE Agent
[...]. The operation get_info may be used over a different transport or
connection".

**Repo:** `crates/xrce-agent/src/daemon/discovery.rs` (`run_discovery_responder`
Z. 43); `crates/xrce-agent/src/daemon/session.rs` (`refusal` Z. 936,
`handle_submessage` Z. 1141, `handle_get_info` Z. 2838)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_tcp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_a_session_is_answered_for_the_agent_only`

**Status:** `done` — der Responder auf dem Port der Discovery und die
Sitzungs-Engine auf den Listenern für UDP und TCP beantworten
GET_INFO(OBJK_AGENT) ohne Sitzung; auch ein anderer Transport als der der
folgenden Sitzung ist möglich. Für den Listener der seriellen Leitung gibt es
keinen eigenen Test. Commits 65cd1820a04, 67ead4d9708.

### §7.8.2.2 GET_INFO ohne Sitzung an einem Listener

**Spec:** §7.8.2.2, S. 47 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`refusal` Z. 936,
`handle_submessage` Z. 1141, `handle_get_info` Z. 2838)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_tcp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_a_session_is_answered_for_the_agent_only`

**Status:** `done` — `handle_submessage` reicht ein GET_INFO ohne aufgelösten
Client oder mit Header ohne Sitzung an `handle_get_info(None, ...)`;
`GET_INFO(OBJK_AGENT)` vor CREATE_CLIENT wird auf den Listenern für UDP und TCP
beantwortet, jede andere Art mit `STATUS_ERR_UNKNOWN_REFERENCE`. Commits
65cd1820a04, 67ead4d9708.

### §7.8.2.2 client_info mit CLIENT_Representation und CLIENTKEY_INVALID

**Spec:** §7.8.2.2, S. 47–48 — "The ObjectVariant member within client_info
shall contain a CLIENT_Representation, which provides information on the XRCE
Client that makes the request. [...] The client_key field of
CLIENT_Representation shall be set to the value CLIENTKEY_INVALID".

**Repo:** `crates/xrce/src/submessages/get_info.rs`
(`GetInfoPayload::with_client_info` Z. 45, `client_info` Z. 62);
`crates/xrce-agent/src/daemon/session.rs` (`client_info_status` Z. 242);
`crates/xrce-client/src/discovery.rs` (`AgentDiscovery::with_client_info`
Z. 113); `crates/xrce-client/src/lib.rs` (`XrceClient::request_agent_info`
Z. 622)

**Tests:** `crates/xrce/src/submessages/get_info.rs::tests::client_info_follows_the_info_mask`,
`crates/xrce-client/src/discovery.rs::tests::client_info_names_the_version_and_hides_the_client_key`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_checks_the_version_in_client_info`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::a_client_of_another_major_version_is_answered_incompatible`,
`crates/xrce-client/src/lib.rs::tests::request_agent_info_works_before_the_session_and_sends_the_mask`

**Status:** `done` — `client_info` ist ein `ObjectInfo` hinter der InfoMask,
dessen `config` eine `CLIENT_Representation` mit `client_key` CLIENTKEY_INVALID
(Nullen) ist. Annex A hat dafür kein Member in `GET_INFO_Payload`; ein
Empfänger, der es nicht liest, beantwortet das GET_INFO wie ohne. Der Agent
liest es: eine andere Hauptversion ergibt INFO mit STATUS_ERR_INCOMPATIBLE
(Figure 10), eine nicht lesbare oder keine CLIENT_Representation
STATUS_ERR_INVALID_DATA. Der Client sendet es nur auf
`AgentDiscovery::with_client_info`; `XrceClient::request_agent_info` sendet das
GET_INFO von Annex A. Commits 65cd1820a04, 5798b040952.

### §7.8.2.2 agent_info mit AGENT_Representation und AGENT_ActivityInfo

**Spec:** §7.8.2.2, S. 48 — "The ObjectVariant member within agent_info shall
contain an AGENT_ActivityInfo [...]. The ActivityInfoVariant member within
agent_info shall contain an AGENT_Representation".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`agent_object_info`
Z. 4242); `crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply`
Z. 107)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_info_mask_selects_activity_and_configuration`

**Status:** `done` — `activity` trägt `ActivityInfoVariant::Agent`
(`AGENT_ActivityInfo`, Layout siehe §7.7.12), `config` die
`AGENT_Representation`. Der Text vertauscht die beiden Member: `config` ist
`ObjectVariant`, `activity` `ActivityInfoVariant`; der Code folgt Annex A.
Commit 65cd1820a04.

### §7.8.2.2 AGENT_Representation in agent_info

**Spec:** §7.8.2.2, S. 48 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`agent_object_info`
Z. 4242, `handle_get_info` Z. 2838); `crates/xrce-agent/src/daemon/discovery.rs`
(`build_info_reply` Z. 107)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_info_mask_selects_activity_and_configuration`

**Status:** `done` — `config` ist `ObjectConfig::Agent(AgentRepresentation)` mit
Version und Vendor des Agent, soweit die InfoMask `config` verlangt; damit wählt
ein Client nach §8.4.4 einen Agent aus. `properties` ist leer. Commit
65cd1820a04.

### §7.8.2.2 address_seq meldet die Adressen
**Spec:** §7.8.2.2, S. 48 — "ActivityInfoVariant member address_seq shall be
used to inform the XRCE Client of the transport addresses over which it can be
reached and can receive calls to create_client."

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`, `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_names_the_secured_and_the_serial_listeners`

**Status:** done — `address_seq` trägt die Adressen der Listener: UDP und TCP mit IPv4 als `ADDRESS_FORMAT_MEDIUM`, mit IPv6 als `ADDRESS_FORMAT_LARGE` (§11.2.1, §11.3.1), als `TransportLocatorSeq` im `AGENT_ActivityInfo` mit DHEADER; DTLS, TLS und eine serielle Leitung als `ADDRESS_FORMAT_STRING` (nächstes Item). c304e509660, 65cd1820a04, 8d2b85bfeb6.
### §7.8.2.2 address_seq nennt die Listener mit DTLS und TLS
**Spec:** §7.8.2.2, S. 48 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_names_the_secured_and_the_serial_listeners`

**Status:** done — die Listener `kind: dtls` und `kind: tls` werden als `ADDRESS_FORMAT_STRING`-Locator gemeldet (`dtls://host:port`, `tls://host:port`), eine serielle Leitung als `serial://device?address=n`: `TransportLocatorFormat` hat dafür keinen eigenen Wert, und ein einfacher Locator desselben Hosts brächte einen Client dazu, ohne den Schutz nach §8.3.2.4 zu verbinden. 8d2b85bfeb6.
### §7.8.2.2 properties für eine Authentifizierung

**Spec:** §7.8.2.2, S. 48 — "The properties field of type PropertySeq [...] may
be used to implement an authentication protocol for the XRCE Agent. The
specific mechanism is outside the scope of this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — außerhalb der Spec.

### §7.8.2.3 delete_client löscht den ProxyClient

**Spec:** §7.8.2.3, S. 48 — "If the object is found it shall be delete and
returnValue StatusValue shall be set to STATUS_OK."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete` Z. 2733)

**Tests:** `crates/xrce-client/src/lib.rs::tests::disconnect_sends_delete_client_and_clears_state`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done`

### §7.8.2.3 delete_client ohne ProxyClient

**Spec:** §7.8.2.3, S. 48 — "The XRCE Agent shall check the ClientKey to
locate an existing XRCE: ProxyClient. If the object is not found the operation
shall fail and returnValue StatusValue shall be set to
STATUS_ERR_UNKNOWN_REFERENCE."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete` Z. 2733:
`deletes_client`, `deleted_clients`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** `done` — `DELETE(OBJECTID_CLIENT)` für einen ClientKey ohne
ProxyClient ergibt `STATUS_ERR_UNKNOWN_REFERENCE`. Dieselbe Anfrage noch einmal,
die die Sitzung beendet hat, ist eine Wiederholung und bekommt `STATUS_OK`.
Commits ed2ca7b49a9, ffa31fc59af.

---

## §7.8.3 XRCE ProxyClient

### §7.8.3 ProxyClient über den ClientKey

**Spec:** §7.8.3, S. 48 — "The XRCE ProxyClient object represents a specific
XRCE Client inside a concrete XRCE Agent. The ProxyClient object is identified
by the ClientKey."

**Repo:** `crates/xrce-agent/src/lib.rs` (`XrceAgent::clients` Z. 102)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::register_client_idempotent`

**Status:** `done`

### §7.8.3 Table 4 Operationen des ProxyClient

**Spec:** §7.8.3, Table 4, S. 48–49 — `create`, `update`, `get_info`,
`delete`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Operationen stehen einzeln in
§7.8.3.1 bis §7.8.3.4.

### §7.8.3.1 create: Ein- und Ausgaben

**Spec:** §7.8.3.1, S. 49 — "creation_mode (CreationMode) [...] objectid_prefix
(ObjectIdPrefix) [...] object_representation (ObjectVariant) [...] The
object_id in the returnValue shall be derived from the object_prefix input
parameter."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create` Z. 1452,
`create_core` Z. 1583, `status_reply` Z. 4293)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done`

### §7.8.3.1 input_objectkind und input_objectid

**Spec:** §7.8.3.1, S. 49 — "We will refer to this ObjectKind as the
'input_objectkind'. [...] The combination of the objectid_prefix and the
ObjectKind contained in the object_representation discriminator shall be used
to construct the 'input' ObjectId."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create` Z. 1531:
`ObjectId::new(request.object_id.raw_id_12(), kind)`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_object_id_is_the_prefix_and_the_kind_of_the_representation`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done` — die `ObjectId` besteht aus dem Präfix des Requests (12 Bit)
und der Art des Diskriminators der Repräsentation; das STATUS nennt sie. Commit
e4d37a6aff6.

### §7.8.3.1 object_id mit anderem Halbbyte

**Spec:** §7.8.3.1, S. 49 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`
Z. 1531–1544)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_object_id_is_the_prefix_and_the_kind_of_the_representation`

**Status:** `done` — das untere Halbbyte der `object_id` des Requests wird nicht
gelesen: auch mit einer anderen Art darin legt `handle_create` das Objekt der
Art des Diskriminators unter demselben Präfix an (im Test ein Participant unter
der `object_id` einer Publisher-Art). Commit e4d37a6aff6.

### §7.8.3.1 Table 5: Objekt existiert nicht

**Spec:** §7.8.3.1, Table 5, S. 50 — "Don't care / Don't care / NO — Create
object according to Table 6."

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore::create` Z. 152,
`CreateOutcome::Created` Z. 175)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_inserts_new_object`

**Status:** `done`

### §7.8.3.1 Table 5: reuse=FALSE, replace=FALSE, existiert

**Spec:** §7.8.3.1, Table 5, S. 50 — "FALSE / FALSE / YES — No action taken.
Set the StatusValue within returnValue to: STATUS_ERR_ALREADY_EXISTS."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Conflict` Z. 208);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` Z. 1692)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_strict_on_existing_returns_conflict`

**Status:** `done`

### §7.8.3.1 Table 5: reuse=FALSE, replace=TRUE, existiert

**Spec:** §7.8.3.1, Table 5, S. 50 — "FALSE / TRUE / YES — Delete existing
object as specified by the delete operation. Create object according to Table
6. Set the StatusValue within returnValue to: STATUS_OK."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Replaced` Z. 206);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` Z. 1700)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_replace_increments_version`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_replace_that_fails_returns_the_object_to_its_previous_configuration`

**Status:** `done` — dazu gehört, dass ein fehlschlagendes Ersetzen das Objekt
in die vorige Konfiguration zurückführt (§7.8.3.2). Commit e4d37a6aff6.

### §7.8.3.1 Table 5: reuse=TRUE, replace=FALSE, existiert

**Spec:** §7.8.3.1, Table 5, S. 50 — "TRUE / FALSE / YES — Check if
object_representation matches the existing Object: If it matches no action is
taken [...] STATUS_OK_MATCHES. If it does not match no action is taken [...]
STATUS_ERR_MISMATCH."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Reused` Z. 180);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` Z. 1688–1693)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_reuse_returns_equal_marker`,
`crates/xrce-agent/src/daemon/session.rs::tests::reuse_compares_the_container_of_the_representation`

**Status:** `done` — gleich: `STATUS_OK_MATCHED`, ungleich:
`STATUS_ERR_MISMATCH`, in beiden Fällen ohne Änderung. Table 5 schreibt
`STATUS_OK_MATCHES`; `StatusValue` in Annex A und `ResultStatusCode::OkMatched`
(0x01) heißen `STATUS_OK_MATCHED`. Verglichen werden Repräsentation, Container
und Domain (Item unten). Commit e4d37a6aff6.

### §7.8.3.1 Vergleich mit participant_id, publisher_id, domain_id

**Spec:** §7.8.3.1, Table 5, S. 50 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core` Z. 1671:
`same_extra`, `ObjectVariantExtra`); `crates/xrce/src/object_store.rs`
(`ObjectStore::create` Z. 152)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::reuse_compares_the_container_of_the_representation`

**Status:** `done` — der Vergleich bei `reuse` schließt den Container
(participant_id, publisher_id, subscriber_id) und die domain_id ein, die der
Objektspeicher nicht hält (`ObjectVariantExtra` je Objekt in der Sitzung).
Dieselbe XML-Zeichenkette unter einem anderen Container ergibt
`STATUS_ERR_MISMATCH`. Commit e4d37a6aff6.

### §7.8.3.1 Table 5: reuse=TRUE, replace=TRUE, existiert

**Spec:** §7.8.3.1, Table 5, S. 50 — "TRUE / TRUE / YES — Check if
object_representation matches the existing Object: If it matches, no action is
taken [...] STATUS_OK_MATCHES. If it does not match, delete existing object as
specified by the delete operation and then create a new object according to
Table 6. Set the StatusValue within returnValue to: STATUS_OK."

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore::create` Z. 177–192);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` Z. 1672–1681)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_reuse_and_replace_replaces_an_unequal_object`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`

**Status:** `done` — bei Gleichheit `STATUS_OK_MATCHED` ohne Änderung, bei
Ungleichheit wird das Objekt gelöscht und neu angelegt (`STATUS_OK`), die
DDS-Seite eingeschlossen. Commit 8b949a13ef5.

### §7.8.3.1 reuse und replace ersetzen ein ungleiches Objekt

**Spec:** §7.8.3.1, Table 5, S. 50 — wie oben.

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore::create` Z. 177–192);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` Z. 1672–1681)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_reuse_and_replace_replaces_an_unequal_object`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_update_of_a_missing_object_creates_it_or_is_refused`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_update_needs_the_permission_to_update`

**Status:** `done` — `ObjectStore::create` behandelt `reuse` mit und ohne
`replace` verschieden: ohne `replace` ergibt ein ungleiches Objekt
`STATUS_ERR_MISMATCH`, mit `replace` ersetzt es das Objekt. Das ist auch die
Operation `update` (§7.8.3.2, Table 14, §8.4.13): `XrceClient::update_object`,
Berechtigung `update` im AccessController, ohne vorhandenes Objekt wird angelegt
(Table 5); `update_of_missing_object: refuse` liefert
`STATUS_ERR_UNKNOWN_REFERENCE` (§7.8.3.2). Die beiden Lesarten widersprechen
sich in der Spec; Standard ist Table 5, getestet in
`the_update_of_a_missing_object_creates_it_or_is_refused`. Commit 8b949a13ef5.

### §7.8.3.1 Alle Repräsentationen mit Referenz und XML

**Spec:** §7.8.3.1, S. 50 — "the representations for the different kinds of
objects all derive from either OBJK_Representation2_Base or
OBJK_Representation3_Base. Therefore, they all have at least the
REPRESENTATION_BY_REFERENCE and the REPRESENTATION_AS_XML_STRING."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`format_is_allowed` Z. 279,
`handle_create` Z. 1508); `crates/xrce-agent/src/daemon/config.rs`
(`representation_formats`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_representation_formats_can_be_widened`

**Status:** `done` — die Spec widerspricht sich: der Text sagt, alle
Repräsentationen hätten `REPRESENTATION_BY_REFERENCE` und
`REPRESENTATION_AS_XML_STRING`; §7.7.3.1 und Annex A geben Type, QosProfile,
Application und Domain Referenz oder XML (`OBJK_RepresentationRefAndXML_Base`),
Publisher, Subscriber, DataWriter und DataReader binär oder XML
(`OBJK_RepresentationBinAndXML_Base`), DomainParticipant und Topic alle drei.
`OBJK_Representation2_Base` gibt es nicht. Standard ist Annex A
(`representation_formats: spec`): ein anderes Format ergibt
STATUS_ERR_INVALID_DATA. Die benannte Einstellung `representation_formats: any`
nimmt jedes Format an, für Clients, die Publisher, Subscriber, DataWriter und
DataReader per Referenz anlegen; gegen einen solchen Client nicht gemessen.
Commits bdf3f6ee966, f0995555d4a.

### §7.8.3.1 Eingebettete Objekte

**Spec:** §7.8.3.1, S. 50 — "Certain representations support the
representation of nested objects. [...] In this case, the creation of the XRCE
object shall also create the nested objects and the failure to create any
nested entity shall be considered a failure to create the contained entity as
well."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_nested` Z. 1833,
`create_embedded` Z. 1785, `create_core` Z. 1743–1759)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_creates_the_objects_inside_it`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_with_a_broken_part_creates_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`

**Status:** `done` — ein DomainParticipant oder eine Application legt die
Objekte an, die ihre Definition enthält (DDS-XML), ebenso ein Publisher oder
Subscriber seine eingebetteten DataWriter und DataReader. Schlägt ein Kind fehl,
scheitert das Ganze: was angelegt war, wird entfernt (`remove_with_nested`) und
das vorige Objekt kommt zurück. Commits 3aea52dd70a, 9239562286c.
### §7.8.3.1 Proxy legt die DDS-Entität an

**Spec:** §7.8.3.1, S. 50 — "Some of the XRCE objects may be defined by this
specification as proxies for DDS entities. In this case the creation of the
XRCE Object will automatically trigger the creation of the proxy DDS Entity."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`apply_dds_create`); `crates/xrce-agent/src/daemon/dds_port.rs`
(`DdsPort::create_participant_with`, `DdsPort::create_topic`,
`DdsPort::create_publisher`, `DdsPort::create_subscriber`,
`DdsPort::create_writer`, `DdsPort::create_reader`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_and_a_subscriber_get_a_dds_entity_with_their_policies`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_publisher_is_a_dds_entity_of_its_participants_domain`

**Status:** `done` — Commits `dd55e9c9df1`. Participant, Topic, Publisher, Subscriber,
DataWriter und DataReader haben eine DDS-Entität. Publisher und Subscriber legen
`DdsPort::create_publisher` und `create_subscriber` an (PRESENTATION,
ENTITY_FACTORY, PARTITION, GROUP_DATA werden angewendet). QosProfile, Type und
Application haben nach Table 6 keine DDS-Entität.

### §7.8.3.1 Scheitern der DDS-Entität ist Scheitern des Objekts

**Spec:** §7.8.3.1, S. 50 — "Failure to create a DDS Entity shall be
considered a failure to create the proxy XRCE object as well."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core`,
`apply_dds_create`, `remove_with_nested`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_dds_entity_is_enabled_when_the_creation_has_succeeded`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_of_an_unknown_type_is_refused_when_the_agent_is_told_to`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_with_a_broken_part_creates_nothing`

**Status:** `done` — Commits `e4d37a6aff6`, `3aea52dd70a`. Scheitert `apply_dds_create`
oder `DdsPort::enable`, löscht `create_core` die DDS-Seite und den
Speichereintrag und antwortet mit dem Code des Fehlers. Scheitert ein
eingebettetes Objekt, entfällt das ganze Objekt (`remove_with_nested`).

### §7.8.3.1 Kein Discovery-Verkehr bei Fehlschlag, Entitäten deaktiviert
**Spec:** §7.8.3.1, S. 50 — "If the creation of the XRCE object fails then
there should be no associated DDS-RTPS discovery traffic generated by the
Agent. This means that all DDS entities shall be created disabled [...] and
enabled (if so configured by their QoS) only after it has been determined that
the creation has succeeded."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`enable`, `create_participant_with`); `crates/xrce-agent/src/daemon/session.rs` (`create_core`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_participant_qos_is_applied_to_the_participant`, `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::an_endpoint_is_announced_only_when_it_is_enabled`

**Status:** done — DataWriter, DataReader und DomainParticipant werden erst mit `DdsPort::enable` beim `DcpsRuntime` registriert und damit entdeckt; `create_core` ruft `enable` auf, wenn das Objekt gespeichert ist und nichts mehr scheitern kann. Ein Objekt, das beim Anlegen oder in `enable` scheitert, wird nie angekündigt. e4d37a6aff6, 232eb724129.
### §7.8.3.1 DomainParticipant wird deaktiviert angelegt
**Spec:** §7.8.3.1, S. 50 — "This means that all DDS entities shall be created
disabled [...] and enabled (if so configured by their QoS) only after it has
been determined that the creation has succeeded."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`create_participant_with`, `enable`, `runtime_for`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_participant_qos_is_applied_to_the_participant`

**Status:** done — `create_participant_with` startet keinen Runtime mehr; es merkt sich den Participant (bei USER_DATA den Platz eines eigenen Runtime), `enable` startet ihn, wenn das CREATE gelingt. Eine Domain-ID über 232 wird beim CREATE abgewiesen. 232eb724129.
### §7.8.3.1 Erfolg ergibt STATUS_OK

**Spec:** §7.8.3.1, S. 50 — "If the creation succeeds the Agent shall set the
StatusValue within returnStatus to STATUS_OK.."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`,
`create_core`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done — Eine erfolgreiche Anlage ist STATUS_OK; bei `reuse` mit
gleicher Darstellung STATUS_OK_MATCHED (Table 5 schreibt STATUS_OK_MATCHES,
unterschiedlich ist nur der Name im Code).

### §7.8.3.1 Table 6 OBJK_QOSPROFILE

**Spec:** §7.8.3.1, Table 6, S. 51 — "The agent shall use that definition to
create an XRCE QosProfile in accordance to the representation defined in
7.7.3.2."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`definition_to_register`, `rebuild_profiles`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`

**Status:** `done` — Commits `3aea52dd70a`, `9239562286c`. Das Objekt hat keine
DDS-Entität. Ein `<qos_library>` mit einem `<qos_profile>` ist dem Agenten als
Definition bekannt, solange das Objekt besteht; andere Clients legen es per
REPRESENTATION_BY_REFERENCE an. Eine Referenz löst sich gegen die Profildateien
und diese Definitionen auf.

### §7.8.3.1 Table 6 OBJK_TYPE
**Spec:** §7.8.3.1, Table 6, S. 51 — "The agent shall use that definition to
create an XRCE Type in accordance to the representation defined in 7.7.3.3."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`types`); `crates/xrce-agent/src/daemon/session.rs` (`find_type_definition`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_type_by_reference_names_a_known_type_by_its_fully_qualified_name`

**Status:** done — XML `<types>` (genau ein nicht verschachtelter Typ) und ein binäres `TypeObject` legen Typen an; eine Referenz löst sich gegen die dem Agenten bekannten Typen auf, und ein Typ darf auf einen bereits bekannten Typ verweisen (Item zu §7.7.3.3.2).
### §7.8.3.1 Table 6 OBJK_APPLICATION

**Spec:** §7.8.3.1, Table 6, S. 51 — "The agent shall use that definition to
create an XRCE Application with all the contained entities found within the
definition in accordance to the representation defined in 7.7.3.5."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`create_nested`, `ensure_predefined`);
`crates/xrce-agent/src/daemon/definitions.rs` (`plan_participant`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`

**Status:** `done` — Commits `9239562286c`, `b6e55cfa859`. `create_nested` legt die
DomainParticipants der Application mit den Objekten in ihnen unter den ObjectIds
ihrer Namen an (Table 16); ein zweites Anlegen verwendet die vorhandenen Objekte
wieder.

### §7.8.3.1 Table 6 OBJK_PARTICIPANT

**Spec:** §7.8.3.1, Table 6, S. 51 — "The agent shall use that definition to
create an XRCE DomainParticipant and an associated DDS DomainParticipant with
all the contained entities found within the definition in accordance to the
representation defined in 7.7.3.6."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`create_core`, `create_nested`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::create_participant_with`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`);
`crates/xrce-agent/src/daemon/binary_forms.rs` (`participant`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_creates_the_objects_inside_it`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_with_a_broken_part_creates_nothing`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::full_chain_against_a_real_leased_domain`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_participant_qos_is_applied_to_the_participant`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_participant_domain_comes_from_the_wire_unless_the_xml_names_one`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_domain_reference_is_resolved_against_the_domain_libraries`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_participant_qos_profile_reference_is_applied`

**Status:** `done` — Commits `3aea52dd70a`, `b4cd825c268`. Der Participant bindet den
`DcpsRuntime` seiner Domain. Die Entitäten in der Definition werden mit ihm
angelegt; scheitert eine, entfällt der ganze Participant. `domain_reference` und
`qos_profile_reference` der Binärform werden aufgelöst.

### §7.8.3.1 Table 6 OBJK_TOPIC

**Spec:** §7.8.3.1, Table 6, S. 51 — "The agent shall locate the XRCE
DomainParticipant identified by the participant_id. If this object is not found
the operation shall fail and return STATUS_ERR_UNKNOWN_REFERENCE. The agent
shall use the definition to create an XRCE Topic [...] and an associated DDS
Topic. The DDS Topic shall be created using the DomainParticipant identified
by the participant_id."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`participant_of_create`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::create_topic`)

**Tests:** `crates/xrce-agent/src/daemon/dds_port.rs::tests::null_port_rejects_topic_before_participant`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_is_created_by_name_or_by_register_type_ref`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_of_an_unknown_type_is_refused_when_the_agent_is_told_to`

**Status:** `done` — Commits `0da6aef56b5`. Das Topic ist im `DcpsRuntime` Name und Typ
seiner Endpunkte. Ein Typ, den der Agent nicht kennt, folgt dem Agent-Schlüssel
`unknown_topic_types`: `refuse` (Standard) antwortet
STATUS_ERR_UNKNOWN_REFERENCE und legt nichts an, `forward` legt das Topic an und
leitet die Samples undekodiert weiter.

### §7.8.3.1 Table 6 OBJK_PUBLISHER

**Spec:** §7.8.3.1, Table 6, S. 51–52 — "The agent shall locate the XRCE
DomainParticipant identified by the participant_id. If this object is not found
the operation shall fail and return STATUS_ERR_UNKNOWN_REFERENCE. [...] an
associated DDS Publisher. The DDS Publisher shall be created using the
DomainParticipant identified by the participant_id."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`participant_of_create`, `DdsCreate::Container`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::create_publisher`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_or_subscriber_needs_an_existing_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_and_a_subscriber_get_a_dds_entity_with_their_policies`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_publisher_is_a_dds_entity_of_its_participants_domain`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::publisher_and_subscriber_need_their_participant_and_endpoints_need_them`

**Status:** `done` — Commits `dd55e9c9df1`. Ein Participant, den der Client nicht hat,
ergibt STATUS_ERR_UNKNOWN_REFERENCE; eine `participant_id` anderer Art
STATUS_ERR_INVALID_DATA; nichts wird gespeichert. Der DDS-Publisher entsteht
unter dem Participant mit PRESENTATION, ENTITY_FACTORY, PARTITION und
GROUP_DATA.

### §7.8.3.1 Table 6 OBJK_SUBSCRIBER

**Spec:** §7.8.3.1, Table 6, S. 51–52 — wie beim Publisher für den Subscriber.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`participant_of_create`, `DdsCreate::Container`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::create_subscriber`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_or_subscriber_needs_an_existing_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_and_a_subscriber_get_a_dds_entity_with_their_policies`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::publisher_and_subscriber_need_their_participant_and_endpoints_need_them`

**Status:** `done` — Commits `dd55e9c9df1`. Wie beim Publisher, für den Subscriber.

### §7.8.3.1 Table 6 OBJK_DATAWRITER

**Spec:** §7.8.3.1, Table 6, S. 52 — "The agent shall locate the XRCE
Publisher identified by the publisher_id. If this object is not found the
operation shall fail and return STATUS_ERR_UNKNOWN_REFERENCE. [...] an
associated DDS DataWriter. The DDS DataWriter shall be created using the
Publisher identified by the publisher_id."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`DdsCreate::Endpoint`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::create_writer`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::create_writer_before_topic_rejected`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::annex_b_2_3_create_of_a_binary_data_writer`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_endpoint_and_its_topic_belong_to_one_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_data_writer_states_its_qos_over_a_profile`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::publisher_and_subscriber_need_their_participant_and_endpoints_need_them`

**Status:** done — Der Writer erbt PARTITION und GROUP_DATA des Publisher. Ein
Publisher, den der Client nicht hat, ergibt STATUS_ERR_UNKNOWN_REFERENCE;
Publisher und Topic müssen zum selben Participant gehören.

### §7.8.3.1 Table 6 OBJK_DATAREADER

**Spec:** §7.8.3.1, Table 6, S. 52 — "OBJK_DATEREADER The ObjectVariant is a
DATAWRITER_Representation [...]. The agent shall locate the XRCE Subscriber
identified by the subscriber_id. If this object is not found the operation
shall fail and return STATUS_ERR_UNKNOWN_REFERENCE."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`DdsCreate::Endpoint`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::create_reader`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_endpoint_and_its_topic_belong_to_one_participant`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::publisher_and_subscriber_need_their_participant_and_endpoints_need_them`

**Status:** done — Die Zeile der Spec schreibt `OBJK_DATEREADER` und
`DATAWRITER_Representation`; gemeint sind `OBJK_DATAREADER` und
`DATAREADER_Representation`. Der Reader erbt PARTITION und GROUP_DATA des
Subscriber; ein fehlender Subscriber ergibt STATUS_ERR_UNKNOWN_REFERENCE.

### §7.8.3.2 update eines bestehenden Objekts

**Spec:** §7.8.3.2, S. 52 — "This operation shall attempt to update an
existing object in the XRCE Agent. If the object exists and the update is
successful STATUS_OK shall be returned".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core`);
`crates/xrce/src/object_store.rs` (`ObjectStore::create`);
`crates/xrce-client/src/lib.rs` (`XrceClient::update_object`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`,
`crates/xrce-client/src/lib.rs::tests::update_object_sends_a_create_with_reuse_and_replace`

**Status:** `done` — Commits `8b949a13ef5`. Table 14 und §8.4.13 bilden update auf CREATE
mit `reuse` und `replace` ab. Eine gleiche Darstellung (Container und
`domain_id` eingeschlossen) ergibt STATUS_OK_MATCHED und ändert nichts; eine
abweichende ersetzt das Objekt samt DDS-Seite (STATUS_OK).

### §7.8.3.2 update ohne Objekt

**Spec:** §7.8.3.2, S. 52 — "If the object does not already exist
STATUS_ERR_UNKNOWN_REFERENCE shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core`);
`crates/xrce-agent/src/daemon/config.rs` (`update_of_missing_object`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_update_of_a_missing_object_creates_it_or_is_refused`

**Status:** `done` — Commits `8b949a13ef5`. Die Spec widerspricht sich: §7.8.3.2 verlangt
STATUS_ERR_UNKNOWN_REFERENCE, Table 5 und §8.4.13 lassen dasselbe CREATE das
Objekt anlegen. Standard ist Table 5 (anlegen); der Agent-Schlüssel
`update_of_missing_object: refuse` liefert STATUS_ERR_UNKNOWN_REFERENCE.

### §7.8.3.2 update mit ungültigen Parametern, Rückkehr zur alten Konfiguration

**Spec:** §7.8.3.2, S. 52 — "If the update was unsuccessful due to invalid
parameters, STATUS_ERR_INVALID_DATA shall be returned. If an update is
unsuccessful the referenced object shall return to its previous
configuration."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core`,
`refuse_create`, `restore_previous`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_replace_that_fails_returns_the_object_to_its_previous_configuration`

**Status:** `done` — Commits `e4d37a6aff6`. Eine Darstellung, aus der sich kein Objekt
bilden lässt, wird vor jeder Änderung mit ihrem STATUS abgewiesen
(STATUS_ERR_INVALID_DATA für eine fehlerhafte Darstellung,
STATUS_ERR_UNKNOWN_REFERENCE für eine Referenz, die sich nicht auflöst).
Scheitert die neue DDS-Seite, stellt `restore_previous` Speichereintrag und
DDS-Seite aus dem gespeicherten Plan wieder her.

### §7.8.3.2 update ohne Rechte

**Spec:** §7.8.3.2, S. 52 — "If the object cannot be updated due to permission
restrictions, STATUS_ERR_DENIED shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core`, `permits`);
`crates/xrce-agent/src/daemon/access.rs` (`Operation::Update`,
`AccessController::permits`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::an_update_needs_the_permission_to_update`,
`crates/xrce-agent/src/daemon/session.rs::tests::replacing_an_object_needs_the_permission_to_delete_it`

**Status:** `done` — Commits `515bcc677f3`, `8b949a13ef5`. Ein update (CREATE mit `reuse`
und `replace`) braucht die Berechtigung `Operation::Update`, ein replace ohne
`reuse` die Berechtigung `Operation::Delete`; sonst STATUS_ERR_DENIED.

### §7.8.3.3 get_info eines Objekts

**Spec:** §7.8.3.3, S. 52 — "object_info (ObjectInfo): contains the current
activity and configuration of the specified object. This operation returns the
configuration and activity data for an existing object."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`,
`object_info`, `answer_info`); `crates/xrce/src/object_info.rs` (`ObjectInfo`);
`crates/xrce/src/submessages/info.rs` (`InfoPayload`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_writer_reports_its_activity`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_reader_reports_its_activity`,
`crates/xrce-agent/src/daemon/session.rs::tests::any_object_id_of_kind_agent_is_the_root_in_a_session`

**Status:** `done` — Commits `65cd1820a04`. Die Antwort ist ein INFO mit `activity`
(DataWriter, DataReader) und `config` (die ObjectVariant, mit der das Objekt
angelegt wurde); die InfoMask wählt aus. `sample_seq_num`, `stream_seq_num` und
`highest_acked_num` zählt der Agent selbst; die Spec definiert die Werte nicht,
ein Vergleich mit einem anderen Agent wurde nicht gemessen.

### §7.8.3.3 get_info ohne Objekt

**Spec:** §7.8.3.3, S. 53 — "If the object does not already exist
STATUS_ERR_UNKNOWN_REFERENCE shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`,
`answer_info`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`

**Status:** `done` — Commits `65cd1820a04`. Der Agent antwortet mit einem INFO, dessen
ResultStatus STATUS_ERR_UNKNOWN_REFERENCE ist und das `request_id` und
`object_id` der Anfrage trägt (§8.3.5.7.2: die Antwort auf GET_INFO ist INFO).

### §7.8.3.3 get_info ohne Rechte

**Spec:** §7.8.3.3, S. 53 — "If the object cannot be accessed due to
permission restrictions STATUS_ERR_DENIED shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`);
`crates/xrce-agent/src/daemon/access.rs` (`Operation::GetInfo`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`

**Status:** `done` — Commits `515bcc677f3`. Ohne Berechtigung `Operation::GetInfo`
antwortet der Agent mit einem INFO mit STATUS_ERR_DENIED; das Objekt bleibt.

### §7.8.3.4 delete eines Objekts

**Spec:** §7.8.3.4, S. 53 — "This operation deletes an existing object. If the
object is successfully deleted STATUS_OK shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::delete_fixture_decodes_and_removes_the_object_create_added`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** done — Die Spec gibt dem Eingang `object_id` den Typ
`ObjectIdPrefix`. DELETE(OBJECTID_CLIENT) beendet die Sitzung und löscht den
ProxyClient (`ed2ca7b49a9`).

### §7.8.3.4 delete ohne Objekt
**Spec:** §7.8.3.4, S. 53 — "If the object does not exist
STATUS_ERR_UNKNOWN_REFERENCE shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::delete_of_an_object_that_does_not_exist_is_an_unknown_reference`, `crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** done — `handle_delete` antwortet STATUS_ERR_UNKNOWN_REFERENCE, wenn kein Objekt entfernt wurde; für DELETE(OBJECTID_CLIENT) ohne ProxyClient (`ed2ca7b49a9`) und für ein gewöhnliches Objekt (0f85937a436).
### §7.8.3.4 delete ohne Rechte

**Spec:** §7.8.3.4, S. 53 — "If the object cannot be deleted due to permission
restrictions, STATUS_ERR_DENIED shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`);
`crates/xrce-agent/src/daemon/access.rs` (`Operation::Delete`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::replacing_an_object_needs_the_permission_to_delete_it`

**Status:** `done` — Commits `515bcc677f3`. Ohne Berechtigung `Operation::Delete`
antwortet der Agent STATUS_ERR_DENIED; das Objekt bleibt.

---

## §7.8.4 XRCE DataWriter

### §7.8.4 Table 7 Operation write

**Spec:** §7.8.4, Table 7, S. 53 — `write` mit `object_id` und `data`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht.

### §7.8.4.1 write schreibt ein oder mehrere Samples

**Spec:** §7.8.4.1, S. 53 — "This operation writes one or more samples using
the XRCE DataWriter identified by the object_id. [...] The DataRepresentation
type (see 7.7.2) supports multiple DataFormats. This allows sending single data
items (FORMAT_DATA) as well as sequences (batches) of data items
(FORMAT_SAMPLE_SEQ)."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`,
`split_write_items`, `dds_write_and_report`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`,
`RealDdsPort::sample_length`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/tests/endpoint_client_fixtures.rs::write_data_fixture_decodes_into_the_sample_bytes_the_client_sent`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_data_seq_is_split_by_the_type_and_written_sample_by_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_seq_carries_state_and_time_per_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::packed_samples_resolve_their_deltas_against_the_base`

**Status:** `done` — Commits `4f1fb9f5f4f`. FORMAT_DATA, FORMAT_SAMPLE, FORMAT_DATA_SEQ,
FORMAT_SAMPLE_SEQ und FORMAT_PACKED_SAMPLES werden geschrieben. Die Elemente
einer Sequenz liegen ohne Länge hintereinander (Annex A `sequence<SampleData>`);
der Typ des Topic (`DdsPort::sample_length`) trennt sie. Annahme, nicht gegen
einen anderen Agent gemessen: zwischen den Elementen steht kein Padding, jedes
wird ab Ausrichtungsursprung 0 gelesen (Annex B.5.2 zeigt nur 4-Byte-Elemente).

### §7.8.4.1 Sequenzformate beim Schreiben

**Spec:** §7.8.4.1, S. 53 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`split_write_items`,
`handle_write_data`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::sample_length`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_data_seq_is_split_by_the_type_and_written_sample_by_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sequence_that_does_not_split_writes_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sequence_of_a_topic_of_unknown_type_is_refused`

**Status:** `done` — Commits `4f1fb9f5f4f`. Die drei Sequenzformate werden Sample für
Sample geschrieben. Eine Sequenz, die sich nicht trennen lässt (Anzahl größer
als die Bytes, Bytes hinter dem letzten Sample, abgeschnittenes Sample),
schreibt nichts und ergibt STATUS_ERR_INVALID_DATA. Dasselbe gilt für ein Topic
unbekannten Typs (nur mit `unknown_topic_types: forward` möglich), dessen
Samples sich nicht trennen lassen.

### §7.8.4.1 Erfolg ergibt STATUS_OK

**Spec:** §7.8.4.1, S. 53 — "If the data is successfully written STATUS_OK
shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_successful_write_is_answered_with_status_ok`

**Status:** `done` — Commits `dc0209b38fe`. Ein erfolgreiches WRITE_DATA wird mit einem
STATUS beantwortet, der `request_id` und DataWriter der Anfrage nennt; mehrere
Samples in einer Anfrage ergeben einen STATUS. Der Mitschnitt des Agents von
eProsima enthält keine Antwort auf WRITE_DATA; der Text der Spec (§8.3.5.8,
Table 14) wird befolgt.

### §7.8.4.1 Unbekannter DataWriter

**Spec:** §7.8.4.1, S. 53 — "If the XRCE DataWriter object identified by the
object_id does not exist, the ResultStatus STATUS_ERR_UNKNOWN_REFERENCE shall
be returned."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`);
`crates/xrce-agent/src/daemon/session.rs` (`dds_write_and_report`,
`dds_port_error_status`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::full_chain_against_a_real_leased_domain`

**Status:** done — `RealDdsPort::write` gibt für einen Writer, den es nicht
gibt, `DdsPortError::UnknownReference` zurück; die Sitzung antwortet
STATUS_ERR_UNKNOWN_REFERENCE.

### §7.8.4.1 write ohne Rechte

**Spec:** §7.8.4.1, S. 53 — "If the client is not allowed to write data using
the referenced object_id due to permission restrictions, the ResultStatus
STATUS_ERR_DENIED shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`,
`permit`); `crates/xrce-agent/src/daemon/access.rs` (`Operation::WriteData`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::write_and_read_without_permission_are_denied`

**Status:** `done` — Commits `515bcc677f3`. Ohne Berechtigung `Operation::WriteData`
antwortet der Agent STATUS_ERR_DENIED; nichts erreicht die DDS-Seite.

### §7.8.4.1 Ungültige Daten

**Spec:** §7.8.4.1, S. 53 — "If the data could not be written successfully
due, for example invalid data format, the ResultStatus
STATUS_ERR_INVALID_DATA shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`,
`refuse_unreadable`, `dds_port_error_status`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_request_that_cannot_be_read_is_answered_invalid_data`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sequence_that_does_not_split_writes_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_dispose_the_agent_cannot_identify_is_answered_with_a_status`

**Status:** `done` — Commits `133a0db3c4f`, `4f1fb9f5f4f`. Ein WRITE_DATA, dessen Body
sich nicht lesen lässt, eine Sequenz, die sich nicht trennen lässt, und ein
dispose ohne bestimmbare Instanz ergeben STATUS_ERR_INVALID_DATA. Liest der
bekannte Typ des Topic einen Payload nicht, gibt `RealDdsPort::write`
`DdsPortError::InvalidType` zurück (ebenfalls STATUS_ERR_INVALID_DATA); dieser
Zweig hat keinen eigenen Test.

### §7.8.4.1 Zeitstempel in SampleInfo

**Spec:** §7.8.4.1, S. 53 — "The DataRepresentation type also supports sending
sample information in addition to the data. This is encoded in the SampleInfo
type (see 7.7.1) allowing sending timestamps and also notifications of dispose
and unregister."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`split_write_items`,
`source_time_of`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::write`, `he_timestamp`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_is_stamped_with_the_session_time_plus_its_offset`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_seq_carries_state_and_time_per_sample`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::known_keyed_type_gets_a_real_keyhash_and_honors_dispose`

**Status:** `done` — Commits `4f1fb9f5f4f`. Die Merker für dispose und unregister und
`session_time_offset` werden aus SampleInfo (FORMAT_SAMPLE, FORMAT_SAMPLE_SEQ)
und SampleInfoDelta (FORMAT_PACKED_SAMPLES) gelesen; der Zeitstempel erreicht
den DDS-Writer als Quellzeit.

### §7.8.4.1 session_time_offset als Quellzeit

**Spec:** §7.8.4.1, S. 53 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`source_time_of`,
`handle_create_client`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::write`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_is_stamped_with_the_session_time_plus_its_offset`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_seq_carries_state_and_time_per_sample`

**Status:** `done` — Commits `4f1fb9f5f4f`. Die Quellzeit ist die Sitzungszeit plus
`session_time_offset`. Die Sitzungszeit ist die Uhr des Agents, als die Sitzung
angelegt wurde: Annex A `CLIENT_Representation` hat kein `client_timestamp`. Mit
`representation_timestamps: present` liest der Agent ein `client_timestamp`,
verwendet es aber nicht als Sitzungszeit. FORMAT_DATA trägt keine SampleInfo und
stempelt nicht.

### §7.8.4.1 dispose
**Spec:** §7.8.4.1, S. 54 — "If the DataRepresentation contains a Sample where
the SampleInfo has the 'dispose' flag set, the XRCE Agent shall call the
dispose operation on the corresponding DDS DataWriter for the instance
identified in the associated data."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`); `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::dispose_on_a_topic_of_unknown_type_is_refused`, `crates/xrce-agent/src/daemon/session.rs::tests::a_dispose_the_agent_cannot_identify_is_answered_with_a_status`, `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done — für einen bekannten Typ mit Schlüssel ruft der Agent `write_user_lifecycle` mit dem Merker DISPOSED und dem echten KeyHash auf (4f1fb9f5f4f). Die Spec verlangt das dispose „for the instance identified in the associated data“; ein Typ ohne Schlüssel oder ein unbekannter Typ identifiziert keine Instanz, die Daten sind für den Agent nicht deutbar: STATUS_ERR_INVALID_DATA (nächstes Item).
### §7.8.4.1 dispose ohne bekannten Typ
**Spec:** §7.8.4.1, S. 54 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`); `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::dispose_on_a_topic_of_unknown_type_is_refused`, `crates/xrce-agent/src/daemon/session.rs::tests::a_dispose_the_agent_cannot_identify_is_answered_with_a_status`, `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done — für ein Topic unbekannten Typs oder eines Typs ohne `@key` kann der Agent die Instanz nicht aus den Daten bestimmen (`SerializedTypeSupport::key_material` ist `None`); eine Instanz, die die Spec identifiziert sehen will, gibt es nicht. `RealDdsPort::write` schreibt nichts und gibt `DdsPortError::InvalidType` zurück; die Sitzung antwortet STATUS_ERR_INVALID_DATA, den Status der Spec für Daten, die der Agent nicht deuten kann, und meldet `SessionEvent::DdsPortFailed` (4f1fb9f5f4f).
### §7.8.4.1 unregister
**Spec:** §7.8.4.1, S. 54 — "Similarly there is a Sample where the SampleInfo
has the 'unregister' flag set, the XRCE Agent shall call the unregister
operation on the corresponding DDS DataWriter for the instance identified in
the associated data."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`); `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::dispose_on_a_topic_of_unknown_type_is_refused`, `crates/xrce-agent/src/daemon/session.rs::tests::a_dispose_the_agent_cannot_identify_is_answered_with_a_status`, `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done — für einen bekannten Typ mit Schlüssel ruft der Agent `write_user_lifecycle` mit dem Merker UNREGISTERED auf; der native Reader des e2e-Tests sieht die Abmeldung (4f1fb9f5f4f). Für einen Typ ohne Schlüssel oder ohne bekannten Typ gilt wie beim dispose STATUS_ERR_INVALID_DATA.

---

## §7.8.5 XRCE DataReader

### §7.8.5 Table 8 Operation read

**Spec:** §7.8.5, Table 8, S. 54 — `read` mit `object_id`,
`read_specification` und Ausgabe `read_data`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht.

### §7.8.5.1 read liest Samples

**Spec:** §7.8.5.1, S. 54 — "This operation reads one or more samples from the
XRCE DataReader identified by the object_id. If the data is successfully read
STATUS_OK shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`pump_read`, `data_submessages`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::read_data_elapsed_time_is_measured_from_the_real_request_time`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_data_seq_is_answered_with_one_data_seq`

**Status:** `done` — Commits `2fa0800f55a`, `d5fbdbe672e`. Der Erfolg ist DATA
(§8.3.5.10) im verlangten `data_format`.

### §7.8.5.1 read ohne Objekt

**Spec:** §7.8.5.1, S. 54 — "If the object does not exist
STATUS_ERR_UNKNOWN_REFERENCE shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`);
`crates/xrce-agent/src/daemon/dds_port.rs` (`NullDdsPort::set_read_filter`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::set_read_filter`)

**Tests:** `crates/xrce-agent/src/daemon/dds_port.rs::tests::null_port_rejects_content_filter_but_accepts_clearing_it`

**Status:** done — Ein Reader, den es nicht gibt, ergibt aus
`DdsPort::set_read_filter` `DdsPortError::UnknownReference`; `handle_read_data`
antwortet STATUS_ERR_UNKNOWN_REFERENCE und startet kein Lesen.

### §7.8.5.1 read ohne Rechte

**Spec:** §7.8.5.1, S. 54 — "If the client is not allowed to read data using
the referenced object_id due to permission restrictions, STATUS_ERR_DENIED
shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`permit`); `crates/xrce-agent/src/daemon/access.rs` (`Operation::ReadData`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::write_and_read_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::read_with_permission_is_not_denied`

**Status:** `done` — Commits `515bcc677f3`. Ohne Berechtigung `Operation::ReadData`
antwortet der Agent STATUS_ERR_DENIED und startet kein Lesen.

### §7.8.5.1 Table 9 preferred_stream_id

**Spec:** §7.8.5.1, Table 9, S. 54 — "Specifies the StreamId the Agent should
use to send the Samples to the Client. The special value STREAMID_NONE
indicates that Agent can choose the StreamId unconstrained. The Agent is
allowed to use an StreamId different from the preferred_stream_id if it
determines that using the preferred_stream_id could cause problems with its
resource-management."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`pump_read`, `ActiveRead`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_above_the_clients_mtu_on_a_reliable_stream_splits_into_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::streamid_none_lets_the_agent_choose_the_stream`

**Status:** `done` — Commits `2fa0800f55a`. DATA geht auf dem gewünschten Stream; für
STREAMID_NONE wählt der Agent STREAMID_BUILTIN_BEST_EFFORTS.

### §7.8.5.1 STREAMID_NONE als gewünschter Stream

**Spec:** §7.8.5.1, Table 9, S. 54 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::streamid_none_lets_the_agent_choose_the_stream`

**Status:** `done` — Commits `2fa0800f55a`. Der Agent wählt den eingebauten
Best-Effort-Stream 0x01; DATA geht nicht auf STREAMID_NONE.

### §7.8.5.1 Table 9 data_format

**Spec:** §7.8.5.1, Table 9, S. 55 — "data_format DataFormat Selects one the
data formats. See 7.7.1"; S. 55: "The setting of the data_format controls
whether the read operation returns a single sample per message or a collection
of samples. It also determines whether the data or it includes the additional
information that appears in the SampleInfo".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`data_submessages`,
`pump_read`, `ActiveRead`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_data_seq_is_answered_with_one_data_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_is_answered_with_a_sample_each`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_seq_is_answered_with_one_sample_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_packed_samples_is_answered_with_one_packed_samples`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_sequence_formats_split_at_the_size_of_a_submessage`,
`crates/xrce-client/src/lib.rs::tests::poll_hands_over_the_data_of_every_format`

**Status:** `done` — Commits `d5fbdbe672e`. DATA kommt in FORMAT_DATA, FORMAT_SAMPLE,
FORMAT_DATA_SEQ, FORMAT_SAMPLE_SEQ oder FORMAT_PACKED_SAMPLES, wie verlangt; die
Sequenzformate teilen sich bei der Größe einer Submessage (PACKED_SAMPLES
höchstens 256 Samples). Die SampleInfo-Werte kennt nur der Agent:
`sequence_number` zählt die gelieferten Samples, `session_time_offset` ist der
Lieferzeitpunkt, `state` ist 0, weil `DdsPort::poll_samples` nur Payload-Bytes
übergibt (kein Quellzeitstempel, kein Instanz-, View- oder Sample-Zustand).

### §7.8.5.1 Table 9 content_filter_expression

**Spec:** §7.8.5.1, Table 9, S. 55 — "A content filter expression selecting
which data to read. The syntax shall be as specified in Annex B (Syntax for
Queries and Filters) of the DDS specification [DDS]." S. 55: "Only samples for
which the filter evaluates to TRUE shall be returned to the XRCE Client."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::set_read_filter`, `CompiledReadFilter`, `drain_buffer`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`);
`crates/xrce/src/submessages/read_data.rs` (`ReadSpecification`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_content_filter_admits_an_xcdr2_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`,
`crates/xrce/src/submessages/read_data.rs::tests::the_content_filter_is_a_string_with_a_nul_and_an_aligned_length`

**Status:** `done` — Commits `1663f8f6240`. Die Kodierung auf der Leitung ist ein
XCDR2-String mit NUL. Ein Filter, den der Agent nicht anwenden kann, ergibt ein
STATUS und liefert keine ungefilterten Daten.

### §7.8.5.1 Table 9 max_samples

**Spec:** §7.8.5.1, Table 9, S. 55 — "Maximum number of samples to return as a
result of the read. The special value MAX_SAMPLES_ZERO =0 is used to cancel any
prior read operation that may still be active. The special value
MAX_SAMPLES_UNLIMITED =0xffff is used to indicate that there is limit on the
number of samples returned."

**Repo:** `crates/xrce/src/continuous_read.rs`
(`ReadStream::pull_pending_samples`, `ReadStream::capacity`);
`crates/xrce-agent/src/daemon/session.rs` (`pump_read`)

**Tests:** `crates/xrce/src/continuous_read.rs::tests::max_samples_cap_enforced`,
`crates/xrce/src/continuous_read.rs::tests::the_capacity_is_what_the_stream_still_delivers`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_takes_only_the_samples_it_delivers`

**Status:** done — Die Grenze wird gezählt; der Agent nimmt aus dem DataReader
nur die Samples, die das Lesen noch liefert. Der Text zu `MAX_SAMPLES_UNLIMITED`
sagt „there is limit“; §8.3.5.9.2 (S. 71) sagt „no limit“, dem folgt der Code.

### §7.8.5.1 MAX_SAMPLES_ZERO und MAX_SAMPLES_UNLIMITED

**Spec:** §7.8.5.1, Table 9, S. 55; §8.3.5.9.2, S. 71 — wie oben.

**Repo:** `crates/xrce/src/continuous_read.rs` (`MAX_SAMPLES_ZERO`,
`MAX_SAMPLES_UNLIMITED`); `crates/xrce-agent/src/daemon/session.rs`
(`handle_read_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::max_samples_zero_cancels_the_active_read`,
`crates/xrce/src/continuous_read.rs::tests::max_samples_zero_ends_the_read_and_unlimited_has_no_limit`

**Status:** `done` — Commits `2fa0800f55a`. 0 beendet das laufende Lesen des DataReader
ohne weiteres DATA; 0xFFFF liest ohne Grenze.

### §7.8.5.1 Table 9 max_elapsed_time
**Spec:** §7.8.5.1, Table 9, S. 55 — "Maximum amount of time in seconds that
may be spent delivering the samples from the read operation. [...] The special
value MAX_ELAPSED_TIME_UNLIMITED = 0 indicates there is no maximum".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`elapsed_time_limit`, `handle_read_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::max_elapsed_time_is_in_seconds_or_in_the_milliseconds_of_the_example`

**Status:** done — der Agent liest `max_elapsed_time` als Sekunden (Table 9, die Definition; 0 ist MAX_ELAPSED_TIME_UNLIMITED) und misst ab dem Zeitpunkt der Anfrage. Annex B.4.2 (Beispiel) schreibt für 30 Sekunden 30000; diese Lesart ist die Einstellung `max_elapsed_time_unit: milliseconds`. e94545ab3bd.
### §7.8.5.1 max_elapsed_time in Millisekunden nach Annex B.4.2
**Spec:** Annex B.4.2, S. 152 — "over a time window not to exceed 30 seconds"
mit `max_elapsed_time = 30000 = 0x7530`; dagegen Table 9, S. 55: "Maximum
amount of time in seconds".

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`max_elapsed_time_unit`); `crates/xrce-agent/src/daemon/session.rs` (`SessionEngineConfig::max_elapsed_time_in_milliseconds`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::max_elapsed_time_is_in_seconds_or_in_the_milliseconds_of_the_example`, `crates/xrce-agent/src/daemon/config.rs::tests::the_unit_of_max_elapsed_time_can_be_set`

**Status:** done — die Lesart des Annex-B-Beispiels (Millisekunden) ist die Einstellung `max_elapsed_time_unit: milliseconds`. e94545ab3bd.
### §7.8.5.1 Table 9 max_bytes_per_sec

**Spec:** §7.8.5.1, Table 9, S. 55 — "Maximum rate in bytes per second at which
the data may be returned to the read operation."

**Repo:** `crates/xrce/src/continuous_read.rs` (`ReadStream`,
`ReadStream::pull_pending_samples`)

**Tests:** `crates/xrce/src/continuous_read.rs::tests::rate_limit_partitions_samples_over_time`

**Status:** done — Das Feld heißt im IDL `max_bytes_per_second`.

### §7.8.5.1 Table 9 min_pace_period

**Spec:** §7.8.5.1, Table 9, S. 55 — "Minimum separation between data messages
returned from the read operation in milliseconds." S. 55: "This period makes
it possible for the client to go into a sleep cycle between messages."

**Repo:** `crates/xrce/src/continuous_read.rs`
(`ReadStream::pull_pending_samples`)

**Tests:** `crates/xrce/src/continuous_read.rs::tests::pacing_throttles_per_period`

**Status:** done

---

## §8.1 General

### §8.1 Eigenschaften des Protokolls

**Spec:** §8.1, S. 57 — Bandbreite 40–100 Kbit/s, Schlafphasen, Unabhängigkeit
von der Sprache, minimale Discovery, voller Zugang zu DDS, Updates mehrerer
Topics, Lieferung verlässlich und best-effort, Sicherheit auf der Ebene des
Transports, Zugang zu allen Daten, "Provide a full implementation requiring
less than 100KB of code."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Ziele des Entwurfs; die Größe des Codes ist
nicht gemessen.

### §8.1 XRCE-Clients ohne Standard-API und ohne Peer-to-Peer

**Spec:** §8.1, S. 57 — "In contrast to applications that use the DDS API
directly, XRCE Clients: Do not have a standard API [...]. Cannot operate
without infrastructure support. [...] Do not communicate directly
peer-to-peer."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung.

---

## §8.2 Definitions

### §8.2 Begriffe client, agent, session, message

**Spec:** §8.2, S. 57 — "At a high level, a client communicates with an agent
using the DDS-XRCE protocol, exchanging messages on a stream belonging to a
session."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffe.

### §8.2.1 Eine Transportnachricht trägt eine XRCE-Nachricht

**Spec:** §8.2.1, S. 57 — "The underlying XRCE Transport shall transfer each
message as a unit. A single XRCE Transport 'message' shall transport a single
XRCE message."

**Repo:** `crates/xrce/src/transport_udp.rs` (`send_message`, `recv_message`);
`crates/xrce/src/transport_tcp.rs` (`XrceTcpClient::send_message`,
`recv_message`)

**Tests:** `crates/xrce/src/transport_udp.rs::tests::rfc768_udp_delivers_one_datagram_per_message_with_boundaries_intact`,
`crates/xrce/src/transport_tcp.rs::tests::rfc793_byte_stream_is_delimited_by_the_length_prefix_not_by_writes`

**Status:** done

### §8.2.1 Ausrichtung ab dem ersten Byte der Nachricht

**Spec:** §8.2.1, S. 57 — "XRCE messages shall be encoded assuming the first
byte has a 16-byte alignment. Therefore, the encoding is independent of any
transport heading or prefix that may precede it."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`Message::encode`,
`Message::decode`); `crates/xrce-agent/src/daemon/binary_forms.rs`
(`participant`, `topic`, `container`, `endpoint`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`Cdr`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::message_two_submessages_padded_correctly`

**Status:** done — Die Codecs der Payloads richten relativ zum Beginn ihrer
Struktur aus; diese Beginne liegen auf Vielfachen von 4. Die Binärformen nach
§7.7.3 liest `binary_forms.rs` als XCDR2 (Ausrichtung bis 4) relativ zum Beginn
der `sequence<octet>`, die Strukturen von eProsima liest `entity_repr.rs::Cdr`
als klassisches CDR (Ausrichtung bis 8) ab demselben Beginn.

### §8.2.2 Sitzung mit Handshake

**Spec:** §8.2.2, S. 58 — "A session defines a bi-directional connection
between a client and an agent that has been established with a handshake."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`);
`crates/xrce-client/src/lib.rs` (`XrceClient::connect`, `handle_status_agent`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::udp_listener_answers_create_client_over_loopback`,
`crates/xrce-client/src/lib.rs::tests::connect_sends_a_real_create_client_and_moves_to_connecting`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`

**Status:** done — Der Agent beantwortet CREATE_CLIENT mit STATUS_AGENT; ein
falsches Cookie ergibt STATUS_AGENT mit STATUS_ERR_INVALID_DATA (`ed2ca7b49a9`).

### §8.2.2 Bis zu 256 Streams je Sitzung

**Spec:** §8.2.2, S. 58 — "A session can contain independent, reliable, and
best-effort message streams. Each session may have up to 256 streams."

**Repo:** `crates/xrce/src/header.rs` (`StreamId`);
`crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::reliable`)

**Tests:** `crates/xrce/src/header.rs::tests::stream_id_classification`,
`crates/xrce-agent/src/daemon/session.rs::tests::reset_clears_reads_backlog_best_effort_counters_and_every_reliable_stream`

**Status:** `done` — Commits `619184de8a1`. `StreamId` hat 256 Werte; der Agent führt den
Zustand je `(client, stream)`, auch für Stream 255 (RESET löscht alle).

### §8.2.2 Höchstens eine aktive Sitzung

**Spec:** §8.2.2, S. 58 — "There can be at most one active session between an
XRCE client and an XRCE agent. Creation of a new session closes any previous
sessions."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`,
`refusal`, `session_ids`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_different_session_id_resets_object_store`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_message_of_a_replaced_session_is_not_processed`

**Status:** `done` — Commits `619184de8a1`. Eine neue `session_id` ersetzt die alte
Sitzung; Nachrichten mit der alten `session_id` werden nicht verarbeitet (siehe
das nächste Item).

### §8.2.2 Nachrichten einer beendeten Sitzung

**Spec:** §8.2.2, S. 58 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::refusal`,
`handle_message`, `SessionEvent::MessageRefused`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_message_of_a_replaced_session_is_not_processed`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_message_without_a_session_is_not_processed_unless_it_is_a_create_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_a_session_id_of_no_session_is_refused`

**Status:** `done` — Commits `619184de8a1`. Eine Nachricht, deren `session_id` nicht die
aktive Sitzung des Clients ist, wird nicht verarbeitet und nicht beantwortet;
der Agent meldet `SessionEvent::MessageRefused`. Die Werte 0x00 und 0x80 (keine
Sitzung) nimmt der Agent nur für CREATE_CLIENT und für GET_INFO (§7.8.2.2) an.

### §8.2.3 Streams mit eigenen Sequenznummern

**Spec:** §8.2.3, S. 58 — "A stream represents an independent ordered flow of
messages within a session. Messages are ordered within a stream by means of a
sequence number. The sequence numbers used by different streams are
independent of each other."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`reliable`,
`best_effort_seq` je `(client, stream)`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::best_effort_messages_get_increasing_sequence_numbers`

**Status:** done

### §8.2.3 Konstante Byte-Reihenfolge je Stream

**Spec:** §8.2.3, S. 58 — "Streams can be reliable or best efforts. Each stream
uses a constant endianness to encode the data in the message/submessage
headers and payload."

**Repo:** `crates/xrce/src/submessages/` (`into_submessage` aller Payloads mit
`FLAG_E_LITTLE_ENDIAN`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_flags_are_e_le_and_format_data`

**Status:** done — ZeroDDS sendet little-endian; beim Empfang folgt jede Payload dem
E-Flag ihrer Submessage (`b43d5e6b848`, `c304e509660`). Die Gleichheit der
Byte-Reihenfolge innerhalb eines Stream prüft der Code nicht.

### §8.2.4 Client beginnt die Sitzung

**Spec:** §8.2.4, S. 58 — "An XRCE client is the entity that initiates the
establishment of a session with an XRCE agent."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::connect`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::connect_sends_a_real_create_client_and_moves_to_connecting`

**Status:** done

### §8.2.5 Agent nimmt Sitzungen an

**Spec:** §8.2.5, S. 58 — "An XRCE agent is the entity that listens to and
accepts requests to establish sessions from XRCE clients."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`run_udp_listener`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::udp_listener_answers_create_client_over_loopback`

**Status:** done

---

## §8.3 Message Structure

### §8.3.1 Header und eine oder mehrere Submessages

**Spec:** §8.3.1, S. 58 — "An XRCE message is composed of a message Header
followed by one or more Submessages and shall be transferred as a unit by the
underlying XRCE Transport."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`Message`, `Message::new`, `encode`,
`decode`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::message_encode_decode_roundtrip_with_key`,
`crates/xrce/tests/foreign_frames.rs::every_client_datagram_decodes_and_writes_back`,
`crates/xrce/tests/wire_roundtrips.rs::a_message_without_a_submessage_is_refused`

**Status:** done — Header und Submessages. Eine Nachricht ohne Submessage lehnen
`Message::new`, `encode` und `decode` ab (`619184de8a1`, siehe das Item
unten).

### §8.3.1 Nachricht ohne Submessage

**Spec:** §8.3.1, S. 58 — wie oben.

**Repo:** `crates/xrce/src/submessages/mod.rs` (`Message::new`, `Message::encode`,
`Message::decode`; Fehler `ValueOutOfRange`)

**Tests:** `crates/xrce/tests/wire_roundtrips.rs::a_message_without_a_submessage_is_refused`

**Status:** done — `Message::new` und `encode` mit einer leeren Liste sowie `decode` eines
reinen Header liefern einen Fehler (`619184de8a1`). Die beiden Tests, die den
früheren Zustand festschrieben, prüfen jetzt die Ablehnung.

### §8.3.2 Layout des Header

**Spec:** §8.3.2, S. 59 — `sessionId` (8 Bit), `streamId` (8 Bit),
`sequenceNr` (16 Bit), `clientKey` (32 Bit, "if sessionId <= 127").

**Repo:** `crates/xrce/src/header.rs` (`MessageHeader`, `write_to`, `read_from`)

**Tests:** `crates/xrce/src/header.rs::tests::header_layout_bytes_no_key`,
`crates/xrce/src/header.rs::tests::header_layout_bytes_with_key`,
`crates/xrce/src/header.rs::tests::header_decode_truncated_with_key`

**Status:** done

### §8.3.2.1 sessionId mit 8 Bit, eindeutig je Client

**Spec:** §8.3.2.1, S. 59 — "A session is identified by an 8-bit sessionId. The
sessionId is unique to an XRCE Agent for a given XRCE Client. The sessionId
also determines whether the Header includes a clientKey or not."

**Repo:** `crates/xrce/src/header.rs` (`SessionId`);
`crates/xrce-agent/src/daemon/session.rs` (`session_ids`)

**Tests:** `crates/xrce/src/header.rs::tests::session_id_127_carries_client_key`

**Status:** done

### §8.3.2.1 sessionId 0 bis 127 mit clientKey

**Spec:** §8.3.2.1, S. 59 — "If the sessionId is between 0 and 127 (0x00 to
0x7f), both included, then the Header shall include the clientKey and the
sessionId is scoped by the clientKey."

**Repo:** `crates/xrce/src/header.rs` (`SessionId::carries_client_key`,
`MessageHeader::with_client_key`)

**Tests:** `crates/xrce/src/header.rs::tests::header_with_client_key_wire_size_8`,
`crates/xrce/src/header.rs::tests::header_constructor_rejects_inconsistent_with_key`

**Status:** done

### §8.3.2.1 sessionId 128 bis 255 ohne clientKey

**Spec:** §8.3.2.1, S. 59 — "If the sessionId is between 128 and 255 (0x80 to
0xff), both included, then the Header shall not include the clientKey and the
sessionId is scoped by the source address of the message."

**Repo:** `crates/xrce/src/header.rs` (`MessageHeader::without_client_key`);
`crates/xrce-agent/src/daemon/transport.rs` (`PeerTable`)

**Tests:** `crates/xrce/src/header.rs::tests::header_no_client_key_wire_size_4`,
`crates/xrce-agent/src/daemon/transport.rs::tests::peer_table_resolves_bound_address`

**Status:** done

### §8.3.2.1 clientKey aus der Quelladresse

**Spec:** §8.3.2.1, S. 59 — "If the clientKey does not appear explicitly in the
message header, the XRCE Agent must be able to locate it from the source
address of the message (see clause 8.3.2.4)."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`PeerTable::resolve`, `bind`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::peer_table_bootstraps_from_create_client_payload`,
`crates/xrce-agent/src/daemon/transport.rs::tests::a_client_registering_again_is_answered_at_its_new_address`

**Status:** done

### §8.3.2.1 SESSION_ID_NONE_WITH_CLIENT_KEY = 0x00

**Spec:** §8.3.2.1, S. 59 — "The value 0 (0x00) shall be used to indicate the
lack of a session within a Header containing a clientKey. This value is
referred to as SESSION_ID_NONE_WITH_CLIENT_KEY."

**Repo:** `crates/xrce/src/header.rs` (`SESSION_ID_NONE_WITH_CLIENT_KEY`,
`SessionId::is_none`, `SessionId::none_like`);
`crates/xrce-agent/src/daemon/session.rs` (`refusal`, `handle_create_client`);
`crates/xrce-client/src/lib.rs` (`XrceClient::connect`)

**Tests:** `crates/xrce/src/header.rs::tests::only_0x00_and_0x80_are_the_no_session_values`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_message_without_a_session_is_not_processed_unless_it_is_a_create_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_a_session_id_of_no_session_is_refused`,
`crates/xrce-client/src/lib.rs::tests::create_client_travels_in_a_header_without_a_session`

**Status:** done — der Client sendet CREATE_CLIENT in einem Header mit 0x00 (Sitzung
0 bis 127) oder 0x80; der Agent verarbeitet in einem solchen Header nur
CREATE_CLIENT (und GET_INFO, §7.8.2.2) und verweigert ein CREATE_CLIENT, das
0x00 oder 0x80 als `session_id` erbittet (`619184de8a1`).

### §8.3.2.1 SESSION_ID_NONE_WITHOUT_CLIENT_KEY = 0x80

**Spec:** §8.3.2.1, S. 59 — "The value 128 (0x80) shall be used to indicate the
lack of a session within a Header that does not contain a clientKey. This
value is referred to as SESSION_ID_NONE_WITHOUT_CLIENT_KEY."

**Repo:** `crates/xrce/src/header.rs` (`SESSION_ID_NONE_WITHOUT_CLIENT_KEY`,
`SessionId::is_none`, `SessionId::none_like`);
`crates/xrce-agent/src/daemon/session.rs` (`refusal`);
`crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply`)

**Tests:** `crates/xrce/src/header.rs::tests::only_0x00_and_0x80_are_the_no_session_values`,
`crates/xrce/src/header.rs::tests::header_roundtrip_no_key`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_message_without_a_session_is_not_processed_unless_it_is_a_create_client`,
`crates/xrce-client/src/lib.rs::tests::create_client_travels_in_a_header_without_a_session`

**Status:** done — CREATE_CLIENT eines Client mit `session_id` 128 bis 255 geht in einem
Header mit 0x80 ohne `clientKey`; INFO der Discovery nutzt 0x80
(`619184de8a1`).

### §8.3.2.1 Werte für „keine Sitzung“ im Header

**Spec:** §8.3.2.1, S. 59 — wie oben.

**Repo:** `crates/xrce/src/header.rs` (`SessionId::is_none`, `SessionId::none_like`);
`crates/xrce-agent/src/daemon/session.rs` (`refusal`, `handle_create_client`,
`SessionEvent::MessageRefused`);
`crates/xrce-client/src/lib.rs` (`XrceClient::connect`, `send_control`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_message_without_a_session_is_not_processed_unless_it_is_a_create_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_message_of_a_replaced_session_is_not_processed`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_a_session_id_of_no_session_is_refused`,
`crates/xrce-client/src/lib.rs::tests::create_client_travels_in_a_header_without_a_session`

**Status:** done — `XrceClient::connect` sendet CREATE_CLIENT mit `SESSION_ID_NONE_*` im
Header (0x00 mit `clientKey`, 0x80 ohne); der Agent verarbeitet in einem
Header ohne Sitzung nur CREATE_CLIENT und GET_INFO, verweigert 0x00 und 0x80
als erbetene `session_id` und verwirft Nachrichten einer ersetzten Sitzung
(`619184de8a1`).

### §8.3.2.2 Streams im Rahmen der Sitzung

**Spec:** §8.3.2.2, S. 59–60 — "Each XRCE message belongs to a single stream.
[...] Streams are scoped by the session they belong to."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`reliable`,
`best_effort_seq` mit dem Schlüssel `(client, stream)`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::best_effort_messages_get_increasing_sequence_numbers`

**Status:** done

### §8.3.2.2 STREAMID_NONE

**Spec:** §8.3.2.2, S. 60 — "The streamId with value 0 (0x00) is referred as
STREAMID_NONE. This stream is used for messages exclusively containing
submessages that do not belong to any stream."

**Repo:** `crates/xrce/src/header.rs` (`StreamId::NONE`)

**Tests:** `crates/xrce/src/header.rs::tests::stream_id_classification`

**Status:** done — die Regeln je Submessage stehen bei §8.3.5.11, §8.3.5.12,
§8.3.5.15 und §8.3.5.16.

### §8.3.2.2 streamId 1 bis 127 best-effort

**Spec:** §8.3.2.2, S. 60 — "The streams with streamId between 1 (0x01) and 127
(0x7F), both included, shall be best-effort streams."

**Repo:** `crates/xrce/src/header.rs` (`StreamId::is_best_effort`)

**Tests:** `crates/xrce/src/header.rs::tests::stream_id_classification`

**Status:** done

### §8.3.2.2 streamId 128 bis 255 reliable, oberstes Bit als Merker

**Spec:** §8.3.2.2, S. 60 — "The streams with streamId between 128 (0x80) and
255 (0xFF), both included, shall be reliable streams. Based on the rules above
if the streamId is not STREAMID_NONE, then the leading bit of the streamId can
be interpreted as a flag that indicates the reliability of the stream."

**Repo:** `crates/xrce/src/header.rs` (`StreamId::is_reliable`)

**Tests:** `crates/xrce/src/header.rs::tests::stream_id_classification`

**Status:** done

### §8.3.2.2 Eingebaute Streams 0x01 und 0x80

**Spec:** §8.3.2.2, S. 60 — "There are two built-in streams that are created
whenever a session is created: A built-in best-effort stream identified by a
streamId with value 1 (0x01). [...] A built-in reliable stream identified by a
streamId with value 128 (0x80)."

**Repo:** `crates/xrce/src/header.rs` (`StreamId::BUILTIN_BEST_EFFORT`,
`BUILTIN_RELIABLE`); `crates/xrce-client/src/lib.rs`
(`send_maybe_fragmented`: Stream 0x80 oder 0x01)

**Tests:** `crates/xrce/src/header.rs::tests::stream_id_classification`,
`crates/xrce-agent/src/daemon/session.rs::tests::reliable_outbound_messages_get_real_incrementing_sequence_numbers`

**Status:** done — der Zustand eines Stream entsteht beim ersten Gebrauch.

### §8.3.2.3 STREAMID_NONE ohne Reihenfolge

**Spec:** §8.3.2.3, S. 60 — "For the stream with streamId STREAMID_NONE, the
sequenceNr does not impose any order; however, it still may be used to discard
duplicate messages."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_message`: Prüfung der
Reihenfolge nur für Best-Effort- und Reliable-Streams)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::best_effort_streams_drop_duplicates_and_older_messages`

**Status:** done — auf STREAMID_NONE prüft der Agent weder Reihenfolge noch Duplikate;
die Spec erlaubt das Verwerfen von Duplikaten dort, verlangt es nicht
(`619184de8a1`).

### §8.3.2.3 Reihenfolge und Duplikate auf den übrigen Streams

**Spec:** §8.3.2.3, S. 60 — "For the stream with streamId different from
STREAMID_NONE, the sequenceNr imposes an order. Messages within a stream shall
not be delivered out of order. In addition, duplicate messages shall be
discarded."

**Repo:** `crates/xrce/src/reliable.rs` (`recv_data`, `drain_in_order`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_message`,
`take_best_effort`); `crates/xrce-client/src/lib.rs` (`ingest_message`)

**Tests:** `crates/xrce/src/reliable.rs::tests::recv_data_reorders_out_of_order`,
`crates/xrce/src/reliable.rs::tests::recv_data_drops_duplicates`,
`crates/xrce-agent/src/daemon/session.rs::tests::out_of_order_reliable_messages_wait_for_the_gap_and_report_a_truthful_acknack`,
`crates/xrce-agent/src/daemon/session.rs::tests::best_effort_streams_drop_duplicates_and_older_messages`,
`crates/xrce-client/src/lib.rs::tests::a_best_effort_stream_discards_duplicate_and_older_messages`

**Status:** done — Reliable Streams ordnen und verwerfen Duplikate über `recv_data` und
`drain_in_order`; Best-Effort-Streams verwerfen Duplikate und ältere
Nachrichten im Agent und im Client (`619184de8a1`, siehe das Item unten).

### §8.3.2.3 Reihenfolge und Duplikate auf Best-Effort-Streams

**Spec:** §8.3.2.3, S. 60 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`take_best_effort`,
`best_effort_in`, `SessionEvent::BestEffortMessageDiscarded`);
`crates/xrce-client/src/lib.rs` (`ingest_message`, `best_effort_in`,
`ClientEvent::BestEffortMessageDiscarded`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::best_effort_streams_drop_duplicates_and_older_messages`,
`crates/xrce-client/src/lib.rs::tests::a_best_effort_stream_discards_duplicate_and_older_messages`

**Status:** done — eine Nachricht, die nicht neuer ist als die letzte des Stream (RFC 1982,
16 Bit, auch über den Zählerüberlauf), wird verworfen und als Ereignis
gemeldet; jeder Stream zählt für sich (`619184de8a1`).

### §8.3.2.3 Serial Number Arithmetic mit 16 Bit

**Spec:** §8.3.2.3, S. 60 — "Addition and comparison of sequence numbers shall
use Serial Number Arithmetic as defined by [IETF RFC-1982] with SERIAL_BITS
set to 16. This implies that the maximum number of outstanding
(unacknowledged) messages for a specific client session stream is limited to
2 15 —that is, 32768."

**Repo:** `crates/xrce/src/serial_number.rs` (`SerialNumber16`,
`wrapping_lt`, `wrapping_cmp`)

**Tests:** `crates/xrce/src/serial_number.rs::tests::lt_across_wrap_boundary`,
`crates/xrce/src/serial_number.rs::tests::undefined_pair_at_exactly_half_window`,
`crates/xrce/src/serial_number.rs::tests::wrap_around_does_not_break_lt_for_consecutive_numbers`

**Status:** done — der Sender hält höchstens 16 Nachrichten offen
(`SENDER_WINDOW_CAP`).

### §8.3.2.3 sequenceNr little-endian

**Spec:** §8.3.2.3, S. 60 — "The sequenceNr shall be encoded using little
endian format."

**Repo:** `crates/xrce/src/header.rs` (`write_to`)

**Tests:** `crates/xrce/src/header.rs::tests::header_layout_bytes_no_key`

**Status:** done

### §8.3.2.4 clientKey bei sessionId 0 bis 127

**Spec:** §8.3.2.4, S. 60 — "The clientKey shall be present on the Header if
the sessionId is between 0 and 127. [...] If the clientKey is present, it shall
contain the ClientKey associated with the XRCE Client."

**Repo:** `crates/xrce-client/src/lib.rs` (`header_client_key`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::every_fixture_shares_the_same_client_key_and_session_id`

**Status:** done

### §8.3.2.4 ClientKey aus der Quelladresse ableiten

**Spec:** §8.3.2.4, S. 60 — "If the clientKey is not present, the XRCE Agent
shall be able to derive the ClientKey associated with the XRCE Client from the
source address of the message. This means that the ClientKey has either been
pre-configured on the XRCE Agent for that particular source address, or it has
been exchanged as part of the session establishment."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`PeerTable::bind`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::peer_table_bootstraps_from_create_client_payload`

**Status:** done — über den Austausch im CREATE_CLIENT; eine Vorkonfiguration
je Quelladresse gibt es nicht.

### §8.3.2.4 Schutz des ClientKey durch den Transport

**Spec:** §8.3.2.4, S. 60 — "Any exchange of the clientKey is protected by the
security mechanisms provided by the XRCE transport."

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`ListenerConfig::Dtls`,
`ListenerConfig::Tls`); `crates/xrce-agent/src/daemon/transport/secure.rs`;
`crates/xrce/src/transport_dtls.rs`, `crates/xrce/src/transport_tls.rs`

**Tests:** `crates/xrce-agent/tests/secure_listeners.rs::tls::a_client_that_trusts_the_agents_certificate_opens_a_session`,
`crates/xrce-agent/tests/secure_listeners.rs::tls::a_plain_text_create_client_on_the_tls_port_is_not_answered`,
`crates/xrce-agent/tests/secure_listeners.rs::dtls::a_client_that_trusts_the_agents_certificate_opens_a_session`,
`crates/xrce-agent/tests/secure_listeners.rs::dtls::a_plain_text_create_client_on_the_dtls_port_is_not_answered`

**Status:** done — Listener `kind: dtls` und `kind: tls` mit Zertifikat und Schlüssel aus
PEM-Dateien (`01470e273bc`); auf diesem Port wird Klartext nicht beantwortet.
Die Features `dtls` und `tls` gehören nicht zu `daemon`; ein Binary braucht
`--features dtls,tls`. Gesicherte Listener erscheinen nicht in GET_INFO(AGENT)
und der Multicast-Discovery. Der DTLS-Handshake eines Peer läuft im
Accept-Task des Listeners, einer nach dem anderen.

### §8.3.3 Submessage mit Header und Payload bis 64 KB

**Spec:** §8.3.3, S. 60 — "Following the message header there shall be one or
more submessages. A Submessage shall be composed of a SubmessageHeader and a
payload." Payload "up to to 64 KB".

**Repo:** `crates/xrce/src/submessages/mod.rs` (`Submessage`,
`Submessage::new`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::message_decode_rejects_truncated_body`,
`crates/xrce/src/submessages/mod.rs::tests::message_encode_decode_roundtrip_no_key_single_sm`

**Status:** done

### §8.3.3 Submessages auf Vielfachen von 4

**Spec:** §8.3.3, S. 60 — "Submessages shall start at an offset that is a
multiple of 4 relative to the beginning of the Message. This means that
additional padding may be added between the end of a submessage and the
beginning of the next submessage."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`Message::encode`,
`Message::decode`: `UnalignedSubmessage`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::message_two_submessages_padded_correctly`,
`crates/xrce/tests/foreign_frames.rs::every_client_datagram_decodes_and_writes_back`

**Status:** done — nach der letzten Submessage schreibt der Code keine
Füllung, eProsima schreibt sie; der Decoder nimmt beides an.

### §8.3.4 Layout des SubmessageHeader

**Spec:** §8.3.4, S. 61 — `submessageId` (8 Bit), `flags` (8 Bit),
`submessageLength` (16 Bit).

**Repo:** `crates/xrce/src/submessages/mod.rs` (`SubmessageHeader`,
`to_bytes`, `from_bytes`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::submessage_header_roundtrip`

**Status:** done

### §8.3.4.1 submessageId

**Spec:** §8.3.4.1, S. 61 — "The submessageId identifies the kind of
submessage. The kinds of submessages are defined in 8.3.5."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`SubmessageId::from_u8`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::submessage_id_rejects_unknown_byte`

**Status:** done — eine unbekannte `submessageId` verwirft die ganze
Nachricht.

### §8.3.4.2 Endianness-Bit in den flags

**Spec:** §8.3.4.2, S. 61 — "Bit 0, the 'Endianness' bit, shall indicate the
endianness used to encode the submessage header and payload. If the Endianness
bit is set to 0, the encoding shall be big endian and otherwise little endian.
The flags field for all submessage kinds shall have the Endianness bit.
Specific submessage kinds may define additional flag bits."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`FLAG_E_LITTLE_ENDIAN`,
`SubmessageHeader::body_endianness`); `crates/xrce/src/handshake.rs`,
`crates/xrce/src/object_info.rs`, `crates/xrce/src/transport_locator.rs`
(`encode_with`/`decode_with`); `crates/xrce-agent/src/daemon/session.rs`,
`crates/xrce-client/src/lib.rs` (lesen mit `body_endianness`)

**Tests:** `crates/xrce/src/submessages/acknack.rs::tests::acknack_be_roundtrip`,
`crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_be_roundtrip`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_big_endian_binary_topic_reads_back`,
`crates/xrce/src/submessages/create_client.rs::tests::create_client_follows_the_e_flag`

**Status:** done — alle Payloads folgen dem Bit (`b43d5e6b848`, `c304e509660`, siehe das
Item unten). Der Agent schreibt seine eigenen Antworten little-endian mit
E=1; jede Submessage nennt ihre Byte-Reihenfolge selbst.

### §8.3.4.2 Payloads und das Endianness-Bit

**Spec:** §8.3.4.2, S. 61 — wie oben.

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation`,
`AgentRepresentation`, `PropertySeq`: `encode_with`/`decode_with`);
`crates/xrce/src/object_info.rs` (`ObjectInfo`, `ActivityInfoVariant`);
`crates/xrce/src/transport_locator.rs` (Ports in der Byte-Reihenfolge des
Flags); `crates/xrce/src/submessages/create_client.rs`
(`CreateClientPayload::into_submessage_with`)

**Tests:** `crates/xrce/src/handshake.rs::tests::a_big_endian_client_representation_with_properties_reads_back`,
`crates/xrce/src/handshake.rs::tests::a_big_endian_agent_representation_with_properties_reads_back`,
`crates/xrce/src/submessages/create_client.rs::tests::create_client_follows_the_e_flag`,
`crates/xrce/src/transport_locator.rs::tests::medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_big_endian_create_client_with_properties_is_read_right`,
`crates/xrce-client/src/lib.rs::tests::a_big_endian_status_agent_is_read_with_its_properties`

**Status:** done — `ClientRepresentation`, `AgentRepresentation`, `PropertySeq`,
`ObjectInfo`, `ActivityInfoVariant` und die Ports der `TransportLocator` werden
in der Byte-Reihenfolge des E-Flag gelesen und geschrieben (`b43d5e6b848`,
`c304e509660`). Der Agent schreibt das INFO, das er baut, weiter
little-endian mit E=1.

### §8.3.4.3 submessageLength ohne Header, little-endian

**Spec:** §8.3.4.3, S. 61 — "The submessageLength indicates the length of the
Submessage (excluding the Submessage header). The submessageLength shall be
encoded using little endian format, independent of the value of the flags."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`SubmessageHeader::to_bytes`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::submessage_header_length_is_always_le_even_with_be_body`

**Status:** done

### §8.3.4.4 payload mit den Typen von §7.7

**Spec:** §8.3.4.4, S. 61 — "The definition of the payload shall use the data
types defined in clause 7.7."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufbau der Spec; die Typen stehen in den
Items zu §7.7.

---

## §8.3.5 Submessage Types

### §8.3.5 Table 10 Werte der SubmessageId

**Spec:** §8.3.5, Table 10, S. 62–63 — CREATE_CLIENT 0, CREATE 1, GET_INFO 2,
DELETE 3, STATUS_AGENT 4, STATUS 5, INFO 6, WRITE_DATA 7, READ_DATA 8, DATA 9,
ACKNACK 10, HEARTBEAT 11, RESET 12, FRAGMENT 13, TIMESTAMP 14, TIMESTAMP_REPLY
15.

**Repo:** `crates/xrce/src/submessages/mod.rs` (`SubmessageId`)

**Tests:** `crates/xrce/tests/profile_conformance.rs::spec_1_2_submessage_ids_match_spec_assignment`,
`crates/xrce/src/submessages/mod.rs::tests::submessage_id_roundtrip_all_16_values`

**Status:** done — der Text davor nennt 13 Arten, Table 10 und Figure 7
zeigen 16.

### §8.3.5.1 CREATE_CLIENT vom Client

**Spec:** §8.3.5.1, S. 63 — "The CREATE_CLIENT submessage shall be sent by the
XRCE Client to create an XRCE ProxyClient."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::connect`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::connect_sends_a_real_create_client_and_moves_to_connecting`

**Status:** done

### §8.3.5.1 Empfang ruft create_client

**Spec:** §8.3.5.1, S. 63 — "Reception of this submessage shall result in the
XRCE Agent calling the create_client operation on the XRCE Root object, see
7.8.2.1. The parameters to this operation are obtained from the payload."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`

**Status:** done

### §8.3.5.1 STATUS_AGENT als Antwort

**Spec:** §8.3.5.1, S. 63 — "The XRCE Agent shall send a STATUS_AGENT message
in response, see 8.3.5.5."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`agent_status_reply`,
`handle_create_client`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_client_rejects_wrong_major_version`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`

**Status:** done — jeder Ausgang antwortet mit STATUS_AGENT; ein falsches Cookie oder eine
nicht lesbare Payload ergibt STATUS_AGENT mit `STATUS_ERR_INVALID_DATA`
(`ed2ca7b49a9`).

### §8.3.5.1 Flags von CREATE_CLIENT

**Spec:** §8.3.5.1.1, S. 63 — "The CREATE_CLIENT submessage does not define
any additional flag bits beyond the common ones specified in 8.3.4.2."

**Repo:** `crates/xrce/src/submessages/create_client.rs`
(`CreateClientPayload::into_submessage`)

**Tests:** `crates/xrce/src/submessages/create_client.rs::tests::create_client_roundtrip_via_submessage`,
`crates/xrce/src/submessages/create_client.rs::tests::annex_b_1_create_client_example_bytes`

**Status:** done — der Code setzt nur das E-Flag. Annex B.1 nennt 0x07 („reuse, replace“),
die CREATE_CLIENT nicht hat; der Test zu Annex B.1 (`d4d590690f6`) prüft die
Bytes der `CLIENT_Representation`, nicht dieses Flag.

### §8.3.5.1 Payload CREATE_CLIENT_Payload

**Spec:** §8.3.5.1.2, S. 64 — "struct CLIENT_Representation { XrceCookie
xrce_cookie; XrceVersion xrce_version; XrceVendorId xrce_vendor_id; ClientKey
client_key; SessionId session_id; @optional PropertySeq properties; }; struct
CREATE_CLIENT_Payload { CLIENT_Representation client_representation; };"

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation`, `encode_form`,
`decode_form`); `crates/xrce/src/encoding.rs`
(`RepresentationTimestamps`); `crates/xrce/src/submessages/create_client.rs`;
Agent-Schlüssel `representation_timestamps` in
`crates/xrce-agent/src/daemon/config.rs`; `XrceClient::set_representation_timestamps`

**Tests:** `crates/xrce/src/submessages/create_client.rs::tests::decodes_the_committed_fixture_member_by_member`,
`crates/xrce/src/submessages/create_client.rs::tests::annex_b_1_create_client_example_bytes`,
`crates/xrce/src/handshake.rs::tests::client_representation_decodes_the_captured_create_client_prefix`,
`crates/xrce/src/handshake.rs::tests::by_default_the_representations_have_no_timestamp`,
`crates/xrce/src/handshake.rs::tests::the_prose_form_has_a_time_t_after_the_vendor_id`,
`crates/xrce-client/src/lib.rs::tests::the_representation_timestamp_is_an_option_of_the_client`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_form_of_the_representations_can_be_set`

**Status:** done — Annex A und die Bytes von Annex B.1 haben kein `client_timestamp`; das
ist die Voreinstellung. Die Prosa von §7.8.2.1 und §8.3.5.1.2 nennt ein
`Time_t` hinter `xrce_vendor_id`; diese Form ist die Einstellung
`representation_timestamps: present` (Rust:
`encoding::RepresentationTimestamps::Present`; SDKs: Option `representation
timestamps`) (`711f93fd460`, `a8e8f6953a4`). Beide Formen sind in `zerodds-xrce`
und im Client mit Bytes getestet. Zwei Byte nach `properties` liest und
schreibt der Code als `requested_mtu`, wie eProsima sie sendet; die Spec
kennt das Feld nicht. `PropertySeq` ist nach Annex A ausgerichtet
(`65cd1820a04`).

### §8.3.5.2 CREATE vom Client

**Spec:** §8.3.5.2, S. 64 — "The CREATE submessage shall be sent by the XRCE
Client to create an XRCE Object."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::create_object`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::create_object_builds_the_real_two_level_object_variant`

**Status:** done

### §8.3.5.2 Empfang ruft create mit Flags und Payload

**Spec:** §8.3.5.2, S. 64 — "Reception of this submessage shall result in the
XRCE Agent calling the create operation on the XRCE ProxyClient object [...].
The parameters to this operation shall be obtained from the SubmessageHeader
flags and payload."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §8.3.5.2 STATUS als Antwort auf CREATE

**Spec:** §8.3.5.2, S. 64 — "The XRCE Agent shall send a STATUS submessage in
response, see 8.3.5.6."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`, `status_reply`,
`refuse_unreadable`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_without_a_base_object_request_is_answered`

**Status:** done — jede Anfrage erhält STATUS: eine Payload ohne `BaseObjectRequest` und
eine nicht lesbare Darstellung ergeben `STATUS_ERR_INVALID_DATA`
(`e4d37a6aff6`, `133a0db3c4f`).

### §8.3.5.2 CREATE ohne BaseObjectRequest

**Spec:** §8.3.5.2, S. 64 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`,
`SessionEvent::CreateRefused`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_without_a_base_object_request_is_answered`

**Status:** done — ein CREATE von weniger als 4 Byte ergibt STATUS mit
`STATUS_ERR_INVALID_DATA` für `OBJECTID_INVALID` (mit dem `request_id`, wenn
seine zwei Byte da sind) und ein Ereignis `CreateRefused` (`e4d37a6aff6`).

### §8.3.5.2 Flags reuse und replace

**Spec:** §8.3.5.2.1, S. 64 — "Bit 1, the 'Reuse' bit, encodes the value of the
CreationMode reuse field. Bit 2, the 'Replace' bit, encodes the value of the
CreationMode replace field."

**Repo:** `crates/xrce/src/submessages/create.rs` (`CREATE_FLAG_REUSE`,
`CREATE_FLAG_REPLACE`)

**Tests:** `crates/xrce/src/submessages/create.rs::tests::create_roundtrip_carries_reuse_replace_flags`,
`crates/xrce/src/submessages/create.rs::tests::create_default_flags_have_no_reuse_no_replace`

**Status:** done

### §8.3.5.2 Payload CREATE_Payload

**Spec:** §8.3.5.2.2, S. 64–65 — "struct CREATE_Payload : BaseObjectRequest {
ObjectVariant object_representation; };"

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`:
`BaseObjectRequest::decode`, `ObjectVariant::decode_for_kind`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::create_fixture_decodes_into_an_object_the_agent_accepts`,
`crates/xrce/src/object_repr.rs::tests::participant_representation_matches_the_captured_create_bytes`

**Status:** done

### §8.3.5.3 GET_INFO vom Client

**Spec:** §8.3.5.3, S. 65 — "The GET_INFO submessage shall be sent by the XRCE
Client to get information about a resource identified by its object_id."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::request_get_info`,
`XrceClient::request_agent_info`, `ClientEvent::Info`,
`ClientEvent::InfoUnreadable`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::request_agent_info_works_before_the_session_and_sends_the_mask`,
`crates/xrce-client/src/lib.rs::tests::request_get_info_needs_a_session_and_names_the_object`,
`crates/xrce-client/src/lib.rs::tests::an_info_is_read_into_an_event`

**Status:** done — `request_get_info` fragt nach einem Objekt der Sitzung,
`request_agent_info` nach dem Agent, auch ohne Sitzung; ein INFO wird als
`ClientEvent::Info` ausgewertet (`65cd1820a04`).

### §8.3.5.3 Ziel nach der ObjectKind der object_id

**Spec:** §8.3.5.3, S. 65 — "The targeted XRCE Object shall depend on the
ObjectKind encoded in the last 4 bits of the object_id. If the ObjectKind is
set to OBJK_AGENT, then it shall result in the XRCE Agent calling the get_info
operation on the XRCE Root object [...]. If the ObjectKind is set to one of
OBJK_PARTICIPANT, [...] That is to a value between 0x01 and 0x0c (both
included), then it shall result in the XRCE Agent calling the get_info
operation on the XRCE ProxyClient object".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`: Art
`OBJK_AGENT` ist die Root, 0x01 bis 0x0C der ProxyClient, `OBJK_CLIENT` die
Sitzung); `crates/xrce-agent/src/daemon/discovery.rs`
(`run_discovery_responder`)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_a_session_is_answered_for_the_agent_only`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`

**Status:** done — die Root antwortet auf jede `object_id` der Art `OBJK_AGENT`, auch
ohne Sitzung; die Arten 0x01 bis 0x0C gehen an den ProxyClient
(`65cd1820a04`). Die Spec verweist für die Root auf §7.8.3.3 statt §7.8.2.2,
die Liste nennt „OBJK“ statt `OBJK_TOPIC`.

### §8.3.5.3 Unterscheidung nach dem Halbbyte der Art

**Spec:** §8.3.5.3, S. 65 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`);
`crates/xrce-agent/src/daemon/discovery.rs` (`run_discovery_responder`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::any_object_id_of_kind_agent_is_the_root_in_a_session`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_tcp_listener_answers_get_info_before_a_session`

**Status:** done — `handle_get_info` und der Responder prüfen die Art der `object_id` und
nicht den Wert 0xFFFD; eine `ObjectId` `0x010` der Art `OBJK_AGENT` wird von
der Root beantwortet (`65cd1820a04`, `67ead4d9708`).

### §8.3.5.3 INFO als Antwort auf GET_INFO

**Spec:** §8.3.5.3, S. 65 — "The XRCE Agent shall send an INFO submessage in
response to this message, see 8.3.5.6."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`, `answer_info`,
`InfoPayload`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`

**Status:** done — jede Antwort auf GET_INFO ist ein INFO; für ein unbekanntes Objekt
trägt es `STATUS_ERR_UNKNOWN_REFERENCE`, ohne Recht `STATUS_ERR_DENIED`
(`65cd1820a04`). Der Verweis lautet auf §8.3.5.6 statt §8.3.5.7.

### §8.3.5.3 Flags von GET_INFO

**Spec:** §8.3.5.3.1, S. 65 — "The GET_INFO submessage does not define any
additional flag bits beyond the common ones specified in 8.3.4.2."

**Repo:** `crates/xrce/src/submessages/get_info.rs`
(`GetInfoPayload::into_submessage`)

**Tests:** `crates/xrce/src/submessages/get_info.rs::tests::get_info_roundtrip`

**Status:** done

### §8.3.5.3 Payload GET_INFO_Payload

**Spec:** §8.3.5.3.2, S. 65 — "struct GET_INFO_Payload : BaseObjectRequest {
InfoMask info_mask; };"

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`:
`BaseObjectRequest::decode`, `GetInfoPayload::parse`);
`crates/xrce/src/object_info.rs` (`InfoMask`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_an_info_mask_is_invalid_data`,
`crates/xrce/src/object_info.rs::tests::info_mask_bits_and_wire_form`,
`crates/xrce/src/submessages/get_info.rs::tests::get_info_roundtrip`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`

**Status:** done — der Agent liest `BaseObjectRequest` und `info_mask`; fehlt die Maske,
antwortet er mit INFO und `STATUS_ERR_INVALID_DATA` (`65cd1820a04`). Das
`client_info` aus Figure 10 (§8.4.4) steht hinter der `InfoMask`
(`GetInfoPayload::with_client_info`), weil Annex A dafür kein Mitglied hat.

### §8.3.5.4 DELETE vom Client

**Spec:** §8.3.5.4, S. 65 — "The DELETE submessage shall be sent by the XRCE
Client to delete the XRCE:ProxyClient or any other XRCE Object".

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::delete_object`,
`disconnect`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::disconnect_sends_delete_client_and_clears_state`

**Status:** done

### §8.3.5.4 delete_client bei OBJK_CLIENT, sonst delete

**Spec:** §8.3.5.4, S. 65–66 — "The related XRCE Object is identified by the
object_id field in the payload. If the ObjectVariant contained within the
payload has ObjectKind set to OBJK_CLIENT, then the XRCE Agent shall call the
delete_client operation. Otherwise it shall call the delete operation."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** done — der Code nimmt die Art aus der `object_id`; ein `ObjectVariant` hat
DELETE_Payload nicht. `delete_client` ohne ProxyClient ergibt
`STATUS_ERR_UNKNOWN_REFERENCE` (`ed2ca7b49a9`).

### §8.3.5.4 STATUS als Antwort auf DELETE

**Spec:** §8.3.5.4, S. 66 — "The XRCE Agent shall send a STATUS submessage in
response, see 8.3.5.6."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`, `status_reply`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done — der Code antwortet auf jedes DELETE mit STATUS, auch auf
`delete_client`, wie der Text der Klausel es sagt. Table 14 (S. 86) nennt für
`delete_client` STATUS_AGENT; dafür gibt es keine Einstellung.

### §8.3.5.4 Flags und Payload DELETE_Payload

**Spec:** §8.3.5.4.1–§8.3.5.4.2, S. 66 — keine zusätzlichen Flags; "struct
DELETE_Payload : BaseObjectRequest { };"

**Repo:** `crates/xrce/src/submessages/delete.rs` (`DeletePayload`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::delete_fixture_decodes_and_removes_the_object_create_added`

**Status:** done

### §8.3.5.5 STATUS_AGENT auf CREATE_CLIENT

**Spec:** §8.3.5.5, S. 66 — "The STATUS_AGENT submessage shall be sent by the
XRCE Agent in response to a CREATE_CLIENT submessage. The submessage shall
contain the returnStatus to the create_client operation invocation".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`agent_status_reply`; `SessionEngine::handle_create_client`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_client_rejects_wrong_major_version`

**Status:** `done` — Commits `ed2ca7b49a9`. auch jeder Fehler von CREATE_CLIENT (falsches
Cookie, `session_id` ohne Sitzung, falsche Hauptversion, ClientKey nicht
zugelassen, keine Ressourcen) wird als STATUS_AGENT beantwortet, nicht als
STATUS.

### §8.3.5.5 Flags von STATUS_AGENT

**Spec:** §8.3.5.5.1, S. 66 — keine zusätzlichen Flags.

**Repo:** `crates/xrce/src/submessages/status_agent.rs`
(`StatusAgentPayload::into_submessage`)

**Tests:** `crates/xrce/src/submessages/status_agent.rs::tests::status_agent_roundtrip`,
`crates/xrce-client/src/lib.rs::tests::a_big_endian_status_agent_is_read_with_its_properties`

**Status:** done — der Agent schreibt die Submessage mit gesetztem E-Flag und
ohne weiteres Flag; der Client liest auch E=0 (§8.3.4.2, `b43d5e6b848`).

### §8.3.5.5 Payload STATUS_AGENT_Payload
**Spec:** §8.3.5.5.2, S. 66–67 — "struct STATUS_AGENT_Payload {
AGENT_Representation agent_info; }; If the operation fails, the
STATUS_AGENT_Payload shall have the ResultStatus within the BaseObjectReply set
to with the StatusValue that corresponds to the type of error encountered."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`agent_status_reply`); `crates/xrce/src/submessages/status_agent.rs` (`StatusAgentPayload::result_and_position`); `crates/xrce-client/src/lib.rs` (`handle_status_agent`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_status_agent_payload_is_annex_a_unless_the_result_status_is_named`, `crates/xrce-agent/src/daemon/config.rs::tests::the_payload_of_status_agent_can_be_set`

**Status:** done — die IDL von Annex A ist `STATUS_AGENT_Payload { AGENT_Representation agent_info; }`: ein erfolgreiches CREATE_CLIENT wird mit der `AGENT_Representation` allein beantwortet (`agent_timestamp` nur mit `representation_timestamps: present`). Die Form des Textes und des Mitschnitts von eProsima mit einem `ResultStatus` davor ist die Einstellung `status_agent: with_result_status`; ein Fehler trägt das `ResultStatus`, das der Text ihm gibt, in beiden Formen, weil die IDL keinen Platz dafür hat. Der Client liest beide (am Cookie unterschieden); der Replay setzt die Einstellung. f514e3eae4b.
### §8.3.5.5 request_id und object_id im STATUS_AGENT
**Spec:** §8.3.5.5.2, S. 67 — "The request_id and object_id within the
BaseObjectReply shall match the namesake fields in the BaseObjectRequest of the
corresponding CREATE_CLIENT message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Der Text verlangt, dass `request_id` und `object_id` im `BaseObjectReply` des STATUS_AGENT denen des `BaseObjectRequest` des CREATE_CLIENT gleichen. `CREATE_CLIENT_Payload` hat in Annex A kein `BaseObjectRequest` (nur die `CLIENT_Representation`), also gibt es nichts zu gleichen, und `STATUS_AGENT_Payload` hat kein `BaseObjectReply` (siehe das Item zur Nutzlast). Es gibt keine erfüllbare Anforderung.
### §8.3.5.5 xrce_cookie XRCE

**Spec:** §8.3.5.5.2, S. 67 — "The xrce_cookie shall be set to the four bytes
{'X', 'R', 'C', 'E'}."

**Repo:** `crates/xrce/src/handshake.rs` (`XRCE_COOKIE`,
`AgentRepresentation::encode_into`)

**Tests:** `crates/xrce/src/handshake.rs::tests::agent_representation_roundtrip_no_properties`,
`crates/xrce/src/handshake.rs::tests::agent_representation_decodes_the_captured_status_agent_body`

**Status:** done

### §8.3.5.5 xrce_version des Agent

**Spec:** §8.3.5.5.2, S. 67 — "The xrce_version shall be set to the version of
the XRCE protocol that the Agent will implement in its connection to the
Client."

**Repo:** `crates/xrce/src/handshake.rs` (`XRCE_VERSION`);
`crates/xrce-agent/src/daemon/session.rs` (`agent_status_reply`)

**Tests:** `crates/xrce/src/handshake.rs::tests::agent_representation_decodes_the_captured_status_agent_body`

**Status:** done

### §8.3.5.6 STATUS auf CREATE und DELETE

**Spec:** §8.3.5.6, S. 67 — "The STATUS submessage shall be sent by the XRCE
Agent in response to a CREATE or DELETE."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`status_reply`; `handle_create`, `handle_delete`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_without_a_base_object_request_is_answered`

**Status:** done

### §8.3.5.6 STATUS auf READ_DATA bei Fehler

**Spec:** §8.3.5.6, S. 67 — "The STATUS submessage shall also be sent by the
XRCE Agent in response to a READ_DATA submessage when the returnStatus to the
read_data operation is anything other than STATUS_OK."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`; `refuse_unreadable`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_request_that_cannot_be_read_is_answered_invalid_data`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_too_large_for_data_is_reported`

**Status:** done — Fehler beim Setzen des Filters, eine nicht lesbare Anfrage
(`133a0db3c4f`) und ein Sample, das in keine Submessage passt, werden als STATUS
beantwortet.

### §8.3.5.6 STATUS trägt den Rückgabewert der Operation

**Spec:** §8.3.5.6, S. 67 — "The STATUS submessage shall contain the
returnStatus to the operation that was triggered by the corresponding request
message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`status_reply`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §8.3.5.6 Flags und Payload STATUS_Payload

**Spec:** §8.3.5.6.1–§8.3.5.6.2, S. 67 — keine zusätzlichen Flags; "struct
STATUS_Payload : BaseObjectReply { }; [...] The request_id and object_id
within the BaseObjectReply shall match the namesake fields in the
corresponding request message."

**Repo:** `crates/xrce/src/object_info.rs` (`BaseObjectReply`, `ResultStatus`);
`crates/xrce/src/submessages/status.rs` (`StatusPayload`)

**Tests:** `crates/xrce/src/object_info.rs::tests::base_object_reply_roundtrip`,
`crates/xrce/tests/unannotated_types.rs::the_final_setting_writes_the_reference_implementations_octets`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done — `ResultStatus` hat in Annex A keine `@extensibility` und ist
APPENDABLE: in der Vorgabe trägt es ein DHEADER (`BaseObjectReply` 10 Byte), mit
`unannotated_types: final` sind es 6 Byte wie im Mitschnitt; beide Formen werden
gelesen (`f2ace917b5a`, `3874ad8d6a6`).

### §8.3.5.7 INFO auf GET_INFO

**Spec:** §8.3.5.7, S. 67 — "The INFO submessage shall be sent by the XRCE
Agent to the XRCE Client in response to a GET_INFO message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`, `answer_info`);
`crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply`)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_tcp_listener_answers_get_info_before_a_session`

**Status:** `done` — Commits `65cd1820a04`, `67ead4d9708`. in der Sitzung, vor einer
Sitzung an UDP- und TCP-Listener und über die Discovery-Gruppe.

### §8.3.5.7 Flags von INFO

**Spec:** §8.3.5.7.1, S. 67 — keine zusätzlichen Flags.

**Repo:** `crates/xrce/src/submessages/info.rs` (`InfoPayload::into_submessage_with`)

**Tests:** `crates/xrce/src/submessages/info.rs::tests::info_roundtrip`,
`crates/xrce/src/submessages/info.rs::tests::info_payload_is_the_reply_and_the_object_info`

**Status:** done — das E-Flag nennt die Byte-Reihenfolge der Payload; der Agent
schreibt E=1 (`65cd1820a04`).

### §8.3.5.7 Payload INFO_Payload

**Spec:** §8.3.5.7.2, S. 67–68 — "struct INFO_Payload : BaseObjectReply {
ObjectInfo object_info; }; [...] The activity and config within members within
the INFO_Payload shall contain the value of the identically named output
parameters of the get_info operation."

**Repo:** `crates/xrce/src/submessages/info.rs` (`InfoPayload::new`, `InfoPayload::parse`);
`crates/xrce/src/object_info.rs` (`BaseObjectReply`, `ObjectInfo`, `ActivityInfoVariant`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`, `object_info`, `agent_object_info`)

**Tests:** `crates/xrce/src/submessages/info.rs::tests::info_payload_is_the_reply_and_the_object_info`,
`crates/xrce/src/object_info.rs::tests::object_info_puts_the_activity_before_the_config`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_writer_reports_its_activity`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_reader_reports_its_activity`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_info_mask_selects_activity_and_configuration`

**Status:** `done` — Commits `65cd1820a04`. `BaseObjectReply` und `ObjectInfo` (activity
vor config, `@optional` als Präsenz-Oktett) nach Annex A; `activity` und
`config` folgen der `InfoMask`. Die Werte `highest_acked_num` (DataReader) sowie
`stream_seq_num` und `sample_seq_num` (DataWriter) zählt der Agent selbst; die
Spec definiert sie nicht, gegen einen anderen Agent nicht gemessen.

### §8.3.5.7 Fehler im INFO

**Spec:** §8.3.5.7.2, S. 68 — "If the operation fails the ResultStatus within
the BaseObjectReply shall be set to the StatusValue that corresponds to the
type of error encountered."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`, `answer_info`);
`crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_an_info_mask_is_invalid_data`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::a_get_info_without_a_mask_is_answered_with_an_error_info`

**Status:** `done` — Commits `65cd1820a04`. ein Fehler von GET_INFO (UNKNOWN_REFERENCE,
INVALID_DATA, DENIED) wird als INFO mit dem Status gesendet, nicht als STATUS.

### §8.3.5.7 request_id und object_id des GET_INFO

**Spec:** §8.3.5.7.2, S. 68 — "The request_id and object_id within the
BaseObjectReply shall match the identically named fields in the
BaseObjectRequest of the corresponding GET_INFO message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`: `InfoRequest::base`, `answer_info`);
`crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/session.rs::tests::any_object_id_of_kind_agent_is_the_root_in_a_session`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::any_object_id_of_kind_agent_is_the_root`

**Status:** `done` — Commits `65cd1820a04`. die Tests prüfen `request_id` und `object_id`
im INFO, auch für eine `object_id` der Art OBJK_AGENT außer 0xFFFD.

### §8.3.5.8 WRITE_DATA vom Client

**Spec:** §8.3.5.8, S. 68 — "The WRITE_DATA submessage is used by the XRCE
Client to write data using an XRCE DataWriter object within the XRCE Agent."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::request_write`,
`XrceClient::request_write_sample`); `crates/xrce/src/submessages/write_data.rs`

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_matches_the_annex_b_example`,
`crates/endpoint-e2e/tests/xrce_write_framing.rs::rust_endpoint_frames_conformant_write_data`

**Status:** done — `xrce_write_framing.rs` prüft die Rahmen der Endpoint-SDKs
weiterer Sprachen gegen denselben Decoder.

### §8.3.5.8 Empfang ruft write, STATUS als Antwort

**Spec:** §8.3.5.8, S. 68 — "Reception of this submessage shall result in the
XRCE Agent calling the write operation on an XRCE DataWriter object (see
7.8.4.1). The XRCE Agent shall respond with a STATUS submessage."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_successful_write_is_answered_with_status_ok`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** `done` — Commits `dc0209b38fe`. `write` wird aufgerufen; ein Schreiben ohne
Fehler wird mit STATUS_OK beantwortet, ein Fehler mit dessen Status. Die Antwort
folgt dem Text von §8.3.5.8; der Mitschnitt des eProsima-Agent zeigt keine
Antwort auf WRITE_DATA, eine Einstellung dafür gibt es nicht.

### §8.3.5.8 DataWriter aus object_id, Daten aus dem Payload

**Spec:** §8.3.5.8, S. 68 — "The data parameter to the write operation shall be
obtained from the payload. The related XRCE DataWriter is identified by the
object_id field in the payload. Upon reception of this message the XRCE Agent
shall located the XRCE DataWriter identified by the object_id and use it to
write the data to the DDS domain."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`, `dds_write_and_report`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`

**Status:** done

### §8.3.5.8 Flags: Endianness und DataFormat in Bit 1 bis 3

**Spec:** §8.3.5.8.1, S. 68 — "The WRITE_DATA sub-message uses the lowest
order 4 bits of the flags: Bit 0 indicates the 'Endianness' [...]. Bits 1, 2,
and 3 shall be set to indicate the DataFormat used for the payload."

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`DataFormat::to_flag_bits`, `DataFormat::from_flag_bits`, `DataFormat::FORMAT_MASK`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_sample_flags_carry_format_sample`,
`crates/xrce/src/submessages/write_data.rs::tests::write_data_rejects_reserved_format`

**Status:** done

### §8.3.5.8 Table 11 Werte der Flags

**Spec:** §8.3.5.8.1, Table 11, S. 68–69 — FORMAT_DATA 0x0/0x1, FORMAT_SAMPLE
0x2/0x3, FORMAT_DATA_SEQ 0x8/0x9, FORMAT_SAMPLE_SEQ 0xA/0xB,
FORMAT_PACKED_SAMPLES 0xE/0xF; Beispiel 0x09 für FORMAT_DATA_SEQ
little-endian.

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`DataFormat`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_flags_are_e_le_and_format_data`,
`crates/xrce/src/submessages/write_data.rs::tests::data_format_raw_roundtrip`,
`crates/xrce/src/submessages/write_data.rs::tests::any_write_data_dispatches_every_format`

**Status:** done

### §8.3.5.8 Table 12 Payload je DataFormat

**Spec:** §8.3.5.8.2, Table 12, S. 69–70 — `WRITE_DATA_Payload_Data`,
`_Sample`, `_DataSeq`, `_SampleSeq`, `_PackedSamples`; "All the WRITE_DATA
payload representations extend BaseObjectRequest".

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`WriteDataPayload`,
`WriteDataPayloadSample`, `WriteDataPayloadDataSeq`, `WriteDataPayloadSampleSeq`,
`WriteDataPayloadPackedSamples`);
`crates/xrce-agent/src/daemon/session.rs` (`split_write_items`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_base_object_request_precedes_the_sample`,
`crates/xrce/src/submessages/write_data.rs::tests::write_data_sample_seq_matches_annex_b_5_4_layout`,
`crates/xrce/src/submessages/write_data.rs::tests::write_data_packed_samples_matches_annex_b_5_5_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_data_seq_is_split_by_the_type_and_written_sample_by_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_seq_carries_state_and_time_per_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::packed_samples_resolve_their_deltas_against_the_base`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sequence_that_does_not_split_writes_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sequence_of_a_topic_of_unknown_type_is_refused`

**Status:** `done` — Commits `4f1fb9f5f4f`. Codec und Verarbeitung im Agent: die Samples
der drei Sequenzformate werden nach dem Typ des Topics getrennt und einzeln
geschrieben, mit `SampleInfo` je Sample. Für ein Topic unbekannten Typs ist eine
Sequenz nicht teilbar und wird mit STATUS_ERR_INVALID_DATA beantwortet, ohne
etwas zu schreiben. Annahme, nicht gegen einen anderen Agent gemessen: die
Samples stehen ohne Füllbytes hintereinander, jedes ab Ausrichtung 0 (Annex
B.5.2 zeigt nur 4-Byte-Elemente).

### §8.3.5.9 READ_DATA vom Client

**Spec:** §8.3.5.9, S. 70 — "The READ_DATA submessage is used by the XRCE
Client to initiate a reception (read) of data from an XRCE DataReader object
within the XRCE Agent."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::request_read`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::read_data_fixture_decodes_into_a_real_read_specification`

**Status:** done

### §8.3.5.9 Empfang ruft read, DATA oder STATUS

**Spec:** §8.3.5.9, S. 70 — "Reception of this submessage shall result in the
XRCE Agent calling the read operation on an XRCE DataReader object (see
7.8.5.1) one or more times. Depending on the returnStatus, the XRCE Agent may
respond with a DATA submessages or a STATUS submessage. The read_specification
parameters to the read operation shall be obtained from the payload."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`, `pump_read`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_elapsed_time_is_measured_from_the_real_request_time`,
`crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`

**Status:** done

### §8.3.5.9 Ein oder mehrere DATA, DataReader aus object_id

**Spec:** §8.3.5.9, S. 70 — "The XRCE Agent shall send one or more DATA
submessages in response to this message, see 8.3.5.10. The related XRCE
DataReader is identified by the object_id field in the payload."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`pump_read`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done

### §8.3.5.9 DATA bis zum Endkriterium

**Spec:** §8.3.5.9, S. 70 — "After reception of this message, the XRCE Agent
shall continue to send DATA submessages to the client until either the 'end
criteria' specified in the payload read_specification and
continuous_read_options attained".

**Repo:** `crates/xrce/src/continuous_read.rs` (`ReadStream`);
`crates/xrce-agent/src/daemon/session.rs` (`tick`, `pump_read`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_elapsed_time_is_measured_from_the_real_request_time`,
`crates/xrce/src/continuous_read.rs::tests::max_samples_cap_enforced`

**Status:** `done` — Commits `2fa0800f55a`. `continuous_read_options` gibt es in der IDL
nicht; das Gegenstück ist `delivery_control` (Item „Lesen ohne und mit
continuous_read_options“ unten).

### §8.3.5.9 Neues READ_DATA für dieselbe object_id

**Spec:** §8.3.5.9, S. 70 — "[...] or else a new READ_DATA message for the same
object_id is received from the XRCE Client."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`: `self.reads.insert`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_new_read_data_replaces_the_active_read`

**Status:** done — das neue Lesen ersetzt das laufende; DATA trägt danach nur die
`request_id` des neuen.

### §8.3.5.9 Steuerung des Zeitpunkts der Lieferung

**Spec:** §8.3.5.9, S. 71 — "The read operation also allows an XRCE Client to
control when data may be sent by the XRCE Agent so that the Agent does not
unnecessarily wake up the Client during its sleep cycle."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweck; die Felder stehen in den Items zu
Table 9.

### §8.3.5.9 Flags von READ_DATA

**Spec:** §8.3.5.9.1, S. 71 — keine zusätzlichen Flags.

**Repo:** `crates/xrce/src/submessages/read_data.rs`
(`ReadDataPayload::into_submessage`, `ReadDataPayload::into_submessage_with`)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::read_data_minimal_roundtrip`

**Status:** done

### §8.3.5.9 Payload READ_DATA_Payload

**Spec:** §8.3.5.9.2, S. 71 — "struct READ_DATA_Payload : BaseObjectRequest {
ReadSpecification read_specification; };"

**Repo:** `crates/xrce/src/submessages/read_data.rs` (`ReadDataPayload`,
`ReadSpecification`, `DataDeliveryControl`, `DeliveryControlForm`);
`crates/xrce-client/src/lib.rs` (`XrceClient::set_delivery_control_form`)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::read_data_base_object_request_precedes_the_spec`,
`crates/xrce/src/submessages/read_data.rs::tests::read_data_matches_the_captured_read_data_example`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`,
`crates/xrce/src/submessages/read_data.rs::tests::the_content_filter_is_a_string_with_a_nul_and_an_aligned_length`,
`crates/xrce/src/submessages/read_data.rs::tests::the_data_format_octet_has_the_values_of_annex_a`

**Status:** `done` — Commits `1663f8f6240`. `BaseObjectRequest` und `ReadSpecification`
nach Annex A (`data_format` 0x00/0x02/0x08/0x0A/0x0E,
`content_filter_expression` als XCDR2-String, `DataDeliveryControl` mit
DHEADER). Der Widerspruch zwischen Annex B.4.2 (DHEADER) und dem Mitschnitt von
eProsima (ohne DHEADER) ist eine benannte Einstellung:
`XrceClient::set_delivery_control_form(DeliveryControlForm::Bare)` schreibt die
Form ohne DHEADER; der Agent liest beide.

### §8.3.5.9 Lesen ohne und mit continuous_read_options

**Spec:** §8.3.5.9.2, S. 71–72 — "If the continuous_read_options member is not
present, then the Agent shall call the read operation just once. As a result
the only data returned will be the one already in the DDS DataReader cache. If
the continuous_read_options member is present, then the Agent shall call the
read operation multiple times. The period of calling shall be no faster than
the pace_period. [...] The Agent shall stop calling the read operation once
either max_total_samples have been returned, or else max_total_elapsed_time has
elapsed."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`);
`crates/xrce/src/continuous_read.rs` (`ReadStream::once`, `ReadStream::new`, `ReadStream::pull_pending_samples`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_read_without_delivery_control_returns_the_cache_once`,
`crates/xrce/src/continuous_read.rs::tests::a_read_without_delivery_control_returns_the_cache_once`,
`crates/xrce-agent/src/daemon/session.rs::tests::next_sample_waits_for_one_sample`,
`crates/xrce/src/continuous_read.rs::tests::pacing_throttles_per_period`,
`crates/xrce/src/continuous_read.rs::tests::max_elapsed_time_finalizes`,
`crates/xrce/src/continuous_read.rs::tests::max_samples_cap_enforced`

**Status:** `done` — Commits `2fa0800f55a`. die IDL kennt `continuous_read_options`,
`pace_period`, `max_total_samples` und `max_total_elapsed_time` nicht; der Code
nimmt das Vorhandensein von `delivery_control` als Merker (`max_samples`,
`max_elapsed_time`, `min_pace_period`). Mit `delivery_control` liest er
fortgesetzt; ohne liest er einmal und liefert, was im Cache liegt. Der
Widerspruch zu Micro-XRCE-DDS 3.0.1 (`DataReader::read`, aus dem Quelltext
gelesen: nächstes Sample ohne Zeitgrenze) ist die benannte Einstellung
`read_without_delivery_control: once | next-sample` (Vorgabe `once`).

### §8.3.5.9 Lesen ohne delivery_control liest den Cache einmal

**Spec:** §8.3.5.9.2, S. 71–72 — wie oben.

**Repo:** `crates/xrce/src/continuous_read.rs` (`ReadStream::pull_pending_samples`);
`crates/xrce-agent/src/daemon/session.rs` (`pump_read`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_read_without_delivery_control_returns_the_cache_once`,
`crates/xrce-agent/src/daemon/session.rs::tests::next_sample_waits_for_one_sample`

**Status:** `done` — Commits `2fa0800f55a`. ohne `delivery_control` liefert der Agent
alle Samples im Cache und beendet das Lesen; ein später eintreffendes Sample
liefert er nicht (Vorgabe `once`). Mit `read_without_delivery_control:
next-sample` liefert er das nächste eintreffende Sample.

### §8.3.5.9 MIN_PACE_PERIOD_NONE

**Spec:** §8.3.5.9.2, S. 72 — "The member min_pace_period may take the special
value MIN_PACE_PERIOD_NONE. This value shall indicate that there is no minimum
time interval between samples."

**Repo:** `crates/xrce/src/continuous_read.rs` (`ReadStream::capacity`,
`ReadStream::pull_pending_samples`: 0 ohne Mindestabstand)

**Tests:** `crates/xrce/src/continuous_read.rs::tests::max_samples_cap_enforced`,
`crates/xrce/src/continuous_read.rs::tests::rate_limit_partitions_samples_over_time`,
`crates/xrce/src/continuous_read.rs::tests::pacing_throttles_per_period`

**Status:** done — die Spec gibt `MIN_PACE_PERIOD_NONE` keinen Wert; der Code
nimmt 0.

### §8.3.5.10 DATA bei STATUS_OK, sonst STATUS

**Spec:** §8.3.5.10, S. 72 — "The DATA submessage shall be sent by the XRCE
Agent to the XRCE Client in response to a READ_DATA message when the read
operation performed by the XRCE Agent returns STATUS_OK. If the read operation
returns any other status the XRCE Agent shall send a STATUS message, not a
DATA message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`pump_read`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_too_large_for_data_is_reported`

**Status:** done

### §8.3.5.10 Folge von DATA bis zum Ende oder Abbruch

**Spec:** §8.3.5.10, S. 72 — "A single READ_DATA message may result on
multiple, possible an open-ended sequence, of DATA submessages sent as a
response by the XRCE Agent. The DATA messages will continue to be sent until
the one of the terminating conditions on the READ_DATA operation is reached,
or until it is explicitly cancelled."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`tick`, `pump_read`, `handle_read_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_elapsed_time_is_measured_from_the_real_request_time`,
`crates/xrce-agent/src/daemon/session.rs::tests::max_samples_zero_cancels_the_active_read`

**Status:** done — bis zur Endbedingung oder zum Abbruch mit `MAX_SAMPLES_ZERO`.

### §8.3.5.10 request_id und object_id des READ_DATA

**Spec:** §8.3.5.10, S. 72 — "The request_id and object_id within the DATA
payload shall match the namesake fields in the corresponding READ_DATA
message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`pump_read`: `ActiveRead::request_id`;
`data_submessages`: `BaseObjectRequest`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce/src/submessages/data.rs::tests::data_base_object_request_precedes_the_sample`

**Status:** done

### §8.3.5.10 Flags von DATA

**Spec:** §8.3.5.10.1, S. 72 — "The flags shall be interpreted the same way as
the flags of the WRITE_DATA submessage."

**Repo:** `crates/xrce/src/submessages/data.rs` (`DataPayload::flags`,
`DataPayloadSample::flags`, `DataPayloadDataSeq::flags`, `DataPayloadSampleSeq::flags`,
`DataPayloadPackedSamples::flags`)

**Tests:** `crates/xrce/src/submessages/data.rs::tests::data_uses_id_9`,
`crates/xrce/src/submessages/data.rs::tests::data_sample_rejects_non_sample_format`

**Status:** done

### §8.3.5.10 Format von DATA wie im READ_DATA verlangt

**Spec:** §8.3.5.10.2, S. 72 — "The format the payload shall match the one
requested in the READ_DATA message having the matching request_id. It shall
also match the DataFormat encoded in the flags".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`: `ActiveRead::format`;
`data_submessages`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_data_seq_is_answered_with_one_data_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_is_answered_with_a_sample_each`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_seq_is_answered_with_one_sample_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_packed_samples_is_answered_with_one_packed_samples`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_sequence_formats_split_at_the_size_of_a_submessage`

**Status:** `done` — Commits `d5fbdbe672e`. der Agent sendet DATA in dem `data_format`,
das das READ_DATA verlangt (FORMAT_DATA, _SAMPLE, _DATA_SEQ, _SAMPLE_SEQ,
_PACKED_SAMPLES); die Sequenzformate teilen sich an der Größe einer Submessage.

### §8.3.5.10 Table 13 Payload je DataFormat

**Spec:** §8.3.5.10.2, Table 13, S. 72–73 — `DATA_Payload_Data`, `_Sample`,
`_DataSeq`, `_SampleSeq`, `_PackedSamples`; "All the DATA payload
representations extend RelatedObjectRequest."

**Repo:** `crates/xrce/src/submessages/data.rs` (`DataPayload`,
`DataPayloadSample`, `DataPayloadDataSeq`, `DataPayloadSampleSeq`,
`DataPayloadPackedSamples`, `AnyData`);
`crates/xrce-agent/src/daemon/session.rs` (`data_submessages`)

**Tests:** `crates/xrce/src/submessages/data.rs::tests::any_data_dispatches_every_format`,
`crates/xrce/src/submessages/data.rs::tests::data_sample_seq_matches_annex_b_5_4_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_seq_is_answered_with_one_sample_seq`

**Status:** `done` — Commits `d5fbdbe672e`. Codec und Agent. In der Liste fehlt nach
`DATA_Payload_Sample` die schließende Klammer (S. 73). Die `SampleInfo` füllt
der Agent mit dem, was er kennt: `sequence_number` zählt die gelieferten
Samples, `session_time_offset` ist der Lieferzeitpunkt, `state` ist 0, weil
`DdsPort::poll_samples` nur die Nutzdaten liefert (Quellzeit, Instanz-, View-
und Sample-Zustand fehlen).

### §8.3.5.11 ACKNACK für ein Protokoll unabhängig vom Transport

**Spec:** §8.3.5.11, S. 74 — "The ACKNACK submessage is used to enable a
transport independent reliability protocol to be implemented. If the
transport used for a session is able to reliably send messages [...] then
these messages may not be required."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweck; auch über TCP benutzt der Code
ACKNACK.

### §8.3.5.11 Agent sendet ACKNACK nur auf HEARTBEAT

**Spec:** §8.3.5.11, S. 74 — "in general it is expected that it is the XRCE
Client that initiates any synchronization, and therefore, the XRCE Agent will
only send ACKNACK messages in response to HEARTBEAT messages."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_heartbeat`;
keine andere Stelle des Agent erzeugt ACKNACK)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::out_of_order_reliable_messages_wait_for_the_gap_and_report_a_truthful_acknack`

**Status:** done

### §8.3.5.11 ACKNACK auf STREAMID_NONE
**Spec:** §8.3.5.11, S. 74 — "The ACKNACK submessage does not belong to any
stream, for this reason the MessageHeader shall have the streamId set to
STREAM_ID_NONE".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_heartbeat`); `crates/xrce-client/src/lib.rs` (`handle_heartbeat`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::the_acknack_of_the_agent_is_on_streamid_none`

**Status:** done — `handle_heartbeat` des Agent und `XrceClient::handle_heartbeat` setzen `StreamId::NONE` im Header des ACKNACK, der Payload nennt den Stream (0f85937a436).
### §8.3.5.11 Flags von ACKNACK

**Spec:** §8.3.5.11.1, S. 74 — keine zusätzlichen Flags.

**Repo:** `crates/xrce/src/submessages/acknack.rs`
(`AckNackPayload::into_submessage`)

**Tests:** `crates/xrce/src/submessages/acknack.rs::tests::acknack_roundtrip_via_submessage`

**Status:** done

### §8.3.5.11 Payload ACKNACK_Payload

**Spec:** §8.3.5.11.2, S. 74 — "struct ACKNACK_Payload { short
first_unacked_seq_num; octet[2] nack_bitmap; octet stream_id; };"

**Repo:** `crates/xrce/src/submessages/acknack.rs` (`AckNackPayload`,
`ACKNACK_BODY_SIZE` = 5 Oktette der Member; `encode_body`, `decode_body`)

**Tests:** `crates/xrce/src/submessages/acknack.rs::tests::acknack_le_roundtrip`,
`crates/xrce/src/submessages/acknack.rs::tests::acknack_be_roundtrip`,
`crates/xrce/tests/unannotated_types.rs::the_default_is_appendable_with_a_dheader`,
`crates/xrce/tests/unannotated_types.rs::the_final_setting_writes_the_reference_implementations_octets`,
`crates/xrce/tests/unannotated_types.rs::both_forms_are_read_in_either_setting`

**Status:** `done` — Commits `f2ace917b5a`. `ACKNACK_Payload` hat in Annex A keine
`@extensibility` und ist APPENDABLE (DDS-XTypes 1.3 §7.3.1.2.1.8): in der
Vorgabe DHEADER (4 Byte) und 5 Byte Member; mit `unannotated_types: final` sind
es 5 Byte wie im Mitschnitt von eProsima; beide Formen werden gelesen, in allen
SDKs (`a293d8fc5e9`, `b590f6539d0`). Der Text nennt `short`, Annex A `unsigned
short` für `first_unacked_seq_num`; beide haben 2 Oktette.

### §8.3.5.11 first_unacked_seq_num

**Spec:** §8.3.5.11.2, S. 74 — "The first_unacked_seq_num shall indicate that
all sequence numbers up to but not including it have been received."

**Repo:** `crates/xrce/src/reliable.rs` (`ReliableStreamState::pending_acknack`:
`expected_seq`; `ReliableStreamState::recv_acknack`)

**Tests:** `crates/xrce/src/reliable.rs::tests::pending_acknack_marks_missing_slots`,
`crates/xrce/src/reliable.rs::tests::recv_acknack_clears_acked_seqnrs`

**Status:** done — das Beispiel darunter sagt „up to and including 99“ für
den Wert 100.

### §8.3.5.11 nack_bitmap

**Spec:** §8.3.5.11.2, S. 74 — "The nack_bitmap shall indicate missing sequence
numbers, starting from first_unacked_seq_num. [...] first_unacked_seq_num set
to 100 and nack_bitmap set to 0x4009 [...] would indicate that sequence
numbers 100, 103, and 114 are missing."

**Repo:** `crates/xrce/src/reliable.rs` (`ReliableStreamState::pending_acknack`,
`ReliableStreamState::recv_acknack`: `u16::from_le_bytes(nack_bitmap)`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_acknack`);
`crates/xrce-client/src/lib.rs` (`XrceClient::handle_acknack`)

**Tests:** `crates/xrce/src/reliable.rs::tests::end_to_end_sender_receiver_with_loss_recovery`,
`crates/xrce-agent/src/daemon/session.rs::tests::acknack_nack_triggers_retransmission_of_the_exact_buffered_bytes`

**Status:** done — Bit i steht für `first_unacked_seq_num + i`, `nack_bitmap[0]`
für die Bits 0 bis 7. Die Spec legt nicht fest, welches Oktett von `octet[2]`
die niedrigen Bits trägt; alle ACKNACK im Mitschnitt von eProsima haben
`nack_bitmap = 0x0000` (mit einem Skript über die Datei gezählt), die
Reihenfolge ist daran nicht prüfbar.

### §8.3.5.11 stream_id des ACKNACK

**Spec:** §8.3.5.11.2, S. 74 — "The stream_id indicates the stream to which the
ACKNACK is directed."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_acknack`:
`self.reliable.get_mut(&(client, ack.stream_id))`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::acknack_nack_triggers_retransmission_of_the_exact_buffered_bytes`

**Status:** done

### §8.3.5.12 HEARTBEAT für ein Protokoll unabhängig vom Transport

**Spec:** §8.3.5.12, S. 74 — "The HEARTBEAT submessage is used to enable a
transport independent reliability protocol to be implemented. [...] If a
session transport is able to reliably send messages [...] then these messages
may not be required."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweck.

### §8.3.5.12 HEARTBEAT nur bei aktivem Client

**Spec:** §8.3.5.12, S. 74 — "This specification does not dictate the timing
of HEARTBEAT messages. However, in general it is expected that it is the XRCE
Agent will only send HEARTBEAT messages when it has some indication that the
XRCE Client is active and not in a sleep cycle."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erwartung. Der Code sendet HEARTBEAT in `tick`
alle `DEFAULT_HEARTBEAT_PERIOD` (500 ms), solange unbestätigte Nachrichten
bestehen und die Sitzung nicht abgelaufen ist, ohne Rücksicht auf eine
Ruhephase.

### §8.3.5.12 HEARTBEAT auf STREAMID_NONE

**Spec:** §8.3.5.12, S. 74 — "The HEARTBEAT submessage does not belong to any
stream, for this reason the MessageHeader shall have the stream_id set to
STREAM_ID_NONE".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`tick`);
`crates/xrce-client/src/lib.rs` (`XrceClient::tick`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_heartbeat_of_the_agent_is_on_streamid_none`,
`crates/xrce-client/src/lib.rs::tests::a_heartbeat_of_the_client_is_on_streamid_none`

**Status:** done — der Header von HEARTBEAT trägt STREAMID_NONE, die Payload den
Stream, den es meldet.

### §8.3.5.12 Flags von HEARTBEAT

**Spec:** §8.3.5.12.1, S. 74 — keine zusätzlichen Flags.

**Repo:** `crates/xrce/src/submessages/heartbeat.rs`
(`HeartbeatPayload::into_submessage`)

**Tests:** `crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_roundtrip_via_submessage`

**Status:** done

### §8.3.5.12 Payload HEARTBEAT_Payload

**Spec:** §8.3.5.12.2, S. 75 — "struct HEARTBEAT_Payload { short
first_unacked_seq_nr; short last_unacked_seq_nr; octet stream_id; };"

**Repo:** `crates/xrce/src/submessages/heartbeat.rs` (`HeartbeatPayload`,
`HEARTBEAT_BODY_SIZE` = 5)

**Tests:** `crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_le_roundtrip`,
`crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_be_roundtrip`

**Status:** done — Annex A schreibt `@extensibility(FINAL)`: 5 Byte ohne
DHEADER. Annex A nennt die Member `first_unacked_seq_num` und
`last_unacked_seq_num` (`unsigned short`), der Text `first_unacked_seq_nr` und
`last_unacked_seq_nr` (`short`); der Code heißt `_nr` und hat `i16`, beide haben
2 Oktette.

### §8.3.5.12 first_unacked_seq_nr und last_unacked_seq_nr

**Spec:** §8.3.5.12.2, S. 75 — "The first_unacked_seq_nr indicates the first
available message sequence number on the sending side. The
last_unacked_seq_nr indicates the first available message sequence number on
the sending side."

**Repo:** `crates/xrce/src/reliable.rs` (`ReliableStreamState::pending_heartbeat`:
`window_base` und das größte offene `seq`)

**Tests:** `crates/xrce/src/reliable.rs::tests::pending_heartbeat_fires_first_time`,
`crates/xrce/src/reliable.rs::tests::heartbeat_window_end_with_hole_is_serial_max_not_count`

**Status:** done — der Satz zu `last_unacked_seq_nr` wiederholt den zu
`first_unacked_seq_nr`; der Code setzt die letzte offene Nummer.

### §8.3.5.12 stream_id des HEARTBEAT

**Spec:** §8.3.5.12.2, S. 75 — "The stream_id indicates the stream to which the
HEARTBEAT is directed."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_heartbeat`:
Zustand je `hb.stream_id`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::out_of_order_reliable_messages_wait_for_the_gap_and_report_a_truthful_acknack`

**Status:** done

### §8.3.5.13 RESET setzt den Zustand der Sitzung zurück

**Spec:** §8.3.5.13, S. 75 — "The RESET submessage shall be used to reset and
re-establish a session. It contains no payload. It shall cause the XRCE Agent
to reset all state associated with the session_id indicated in the submessage
header."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_reset`);
`crates/xrce/src/reliable.rs` (`ReliableStreamState::reset`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::reset_clears_reads_backlog_best_effort_counters_and_every_reliable_stream`,
`crates/xrce/src/reliable.rs::tests::reset_clears_state_completely`

**Status:** `done` — Commits `619184de8a1`. RESET setzt zurück: alle Reliable Streams
(auch Stream 255), die Reassemblierung von FRAGMENT, die laufenden Lesevorgänge,
den Rückstau auf volle Sender-Fenster und die Zähler der Best-Effort-Streams,
ein- und ausgehend. Die Objekte des ProxyClient und seine `session_id` bleiben;
ob „all state“ sie meint, legt die Spec nicht fest.

### §8.3.5.13 Laufendes Lesen und Zähler nach RESET

**Spec:** §8.3.5.13, S. 75 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_reset`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::reset_clears_reads_backlog_best_effort_counters_and_every_reliable_stream`

**Status:** `done` — Commits `619184de8a1`. `handle_reset` beendet laufende READ_DATA,
leert den Rückstau und setzt die Zähler der Best-Effort-Streams zurück; der Test
ruft RESET über die Sitzung.

### §8.3.5.13 Flags und leerer Payload von RESET

**Spec:** §8.3.5.13.1–§8.3.5.13.2, S. 75 — keine zusätzlichen Flags; "The
RESET submessage shall have an empty payload."

**Repo:** `crates/xrce/src/submessages/reset.rs` (`ResetPayload`)

**Tests:** `crates/xrce/src/submessages/reset.rs::tests::reset_roundtrip_empty_body`,
`crates/xrce/src/submessages/reset.rs::tests::reset_rejects_nonempty_body`

**Status:** done

### §8.3.5.14 FRAGMENT für Submessages über der MTU

**Spec:** §8.3.5.14, S. 75 — "The FRAGMENT submessage is used to enable sending
of other submessages whose length exceeds the transport MTU."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`emit_possibly_fragmented`);
`crates/xrce-client/src/lib.rs` (`XrceClient::send_maybe_fragmented`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_above_the_clients_mtu_on_a_reliable_stream_splits_into_fragments`,
`crates/xrce-client/src/lib.rs::tests::write_above_mtu_splits_into_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_fragment_run_without_a_submessage_is_answered_invalid_data`

**Status:** done — die MTU kommt aus dem Feld `requested_mtu`, das eProsima
anhängt; fehlt es, gelten 508 Byte. FRAGMENT nur auf Reliable Streams
(§8.3.5.14): auf einem Best-Effort-Stream sendet der Agent eine zu große
Submessage ungeteilt und meldet `SessionEvent::ReliableSendOverflowed`. Ein Lauf
ohne Submessage wird abgelehnt (`1933142acaf`).

### §8.3.5.14 FRAGMENT nur auf Reliable Streams

**Spec:** §8.3.5.14, S. 75 — "The FRAGMENT message shall only be sent within
reliable streams."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`emit_possibly_fragmented`:
auf Best-Effort-Streams ganz gesendet); `crates/xrce-client/src/lib.rs`
(`send_maybe_fragmented`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_above_the_clients_mtu_on_a_reliable_stream_splits_into_fragments`,
`crates/xrce-client/src/lib.rs::tests::write_above_mtu_splits_into_fragments`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done — Agent und Client zerlegen nur auf Reliable Streams; eine
Submessage über der MTU geht auf einem Best-Effort-Stream ganz hinaus (Ereignis
`ReliableSendOverflowed`). Beim Empfang setzt der Code FRAGMENT auf jedem Stream
zusammen. Der Best-Effort-Zweig hat keinen eigenen Test.

### §8.3.5.14 Bit Last Fragment

**Spec:** §8.3.5.14, S. 75 — "all FRAGMENT submessage except for the last
shall have the 'Last Fragment' bit in the flags set to 0. The last FRAGMENT
submessage shall have the 'Last Fragment' flag set 1."

**Repo:** `crates/xrce/src/submessages/fragment.rs` (`FRAGMENT_FLAG_LAST`)

**Tests:** `crates/xrce/src/submessages/fragment.rs::tests::fragment_roundtrip_carries_last_flag`,
`crates/xrce/src/submessages/fragment.rs::tests::fragment_intermediate_has_no_last_flag`

**Status:** done

### §8.3.5.14 Zusammensetzen und Auslegen als Submessages

**Spec:** §8.3.5.14, S. 75 — "Upon reception of the last fragment submessage
the Agent shall concatenate the payload bytes of all FRAGMENT messages for
that Stream in the order of the stream sequence number without sequence number
gaps. The concatenated payloads shall be interpreted as XRCE submessages as if
they had been received following the HEADER that came with the last
fragment."

**Repo:** `crates/xrce-agent/src/daemon/session.rs`
(`process_message_submessages`); `crates/xrce/src/fragment.rs`
(`FragmentAssembler::add_fragment`); `crates/xrce/src/submessages/mod.rs`
(`decode_submessages`)

**Tests:**
`crates/xrce-agent/src/daemon/session.rs::tests::multi_fragment_create_reassembles_correctly`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_second_fragmented_message_reassembles_like_the_first`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_fragmented_run_of_two_submessages_runs_both`

**Status:** done — die Bytes werden in der Reihenfolge des Stream
zusammengesetzt und unter dem Header der Nachricht mit dem letzten Fragment
verarbeitet, jede Submessage des Laufs (9b18d126ef3).

### §8.3.5.14 Alle Submessages der zusammengesetzten Folge

**Spec:** §8.3.5.14, S. 75 — wie oben.

**Repo:** `crates/xrce/src/submessages/mod.rs` (`decode_submessages`);
`crates/xrce-agent/src/daemon/session.rs` (`process_message_submessages`);
`crates/xrce-client/src/lib.rs` (`XrceClient`, `ClientEvent::FragmentDiscarded`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::a_submessage_run_decodes_every_submessage_past_the_padding`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_fragmented_run_of_two_submessages_runs_both`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_fragment_run_without_a_submessage_is_answered_invalid_data`,
`crates/xrce-client/src/lib.rs::tests::a_fragment_run_without_a_submessage_is_reported`

**Status:** done — Agent und Client nehmen jede Submessage des zusammengesetzten
Laufs, auf Vielfachen von 4 (§8.3.3) (9b18d126ef3). Ein Lauf ohne Submessage
oder ohne Folge von Submessages ergibt beim Agent STATUS
`STATUS_ERR_INVALID_DATA`, beim Client `ClientEvent::FragmentDiscarded`
(1933142acaf).

### §8.3.5.14 Flags von FRAGMENT

**Spec:** §8.3.5.14.1, S. 75 — "Bit 0 indicates the 'Endianness' [...]. Bit 1,
the 'Last Fragment' bit, indicates the last fragment in the sequence."

**Repo:** `crates/xrce/src/submessages/fragment.rs` (`FragmentPayload::flags`)

**Tests:** `crates/xrce/src/submessages/fragment.rs::tests::fragment_roundtrip_carries_last_flag`

**Status:** done

### §8.3.5.14 Payload von FRAGMENT im Puffer

**Spec:** §8.3.5.14.2, S. 75 — "The payload of the FRAGMENT submessage is
opaque. The Agent shall cache the payload bytes of all FRAGMENT submessages for
a Stream in the order of the stream sequence number until the last FRAGMENT
submessage is received."

**Repo:** `crates/xrce/src/fragment.rs` (`FragmentAssembler`);
`crates/xrce-agent/src/daemon/session.rs` (`FragmentRun`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_tick_between_fragments_keeps_the_reassembly`,
`crates/xrce/src/fragment.rs::tests::out_of_order_reassembly`

**Status:** done — ein unvollständiger Lauf wird nach 10 s verworfen
(`DEFAULT_GC_TTL`) und als Ereignis gemeldet.

### §8.3.5.15 TIMESTAMP senden

**Spec:** §8.3.5.15, S. 76 — "The TIMESTAMP submessage is used to send
timestamp information. [...] The TIMESTAMP submessage does not belong to any
stream, for this reason the MessageHeader shall have the streamId set to
STREAM_ID_NONE". §8.3.5.15.2: "The transmit_timestamp shall contain the
timestamp of the sender of the message at the time the message is sent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::request_timestamp`, `tick` mit `timestamp_interval`,
`handle_timestamp_reply`); `crates/xrce-client/src/lib.rs`
(`XrceClient::send_timestamp`, `send_timestamp_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_agent_sends_a_timestamp_and_reads_the_reply`,
`crates/xrce-agent/src/daemon/session.rs::tests::tick_sends_a_timestamp_every_interval`,
`crates/xrce-client/src/lib.rs::tests::the_client_sends_and_reads_timestamps`

**Status:** done — Agent und Client senden TIMESTAMP auf `STREAMID_NONE` mit der
Uhrzeit des Senders (2b718de0166). Der Agent sendet ungefragt im Abstand des
Agent-Schlüssels `timestamp_interval_secs`; Vorgabe 0 (keine), weil die
eProsima-Clients keine TIMESTAMP erwarten. Die TIMESTAMP_REPLY des Client ergibt
das Ereignis `ClockSynchronised`.

### §8.3.5.15 Flags und Payload TIMESTAMP_Payload

**Spec:** §8.3.5.15.1–§8.3.5.15.2, S. 76 — keine zusätzlichen Flags; "struct
TIMESTAMP_Payload { Time_t transmit_timestamp; };"

**Repo:** `crates/xrce/src/submessages/timestamp.rs` (`TimestampPayload`,
`TimePoint`, `TimePoint::encode_into`, `TimePoint::decode_at`)

**Tests:** `crates/xrce/src/submessages/timestamp.rs::tests::timestamp_roundtrip_via_submessage`,
`crates/xrce/src/submessages/timestamp.rs::tests::time_t_roundtrip_be`,
`crates/xrce/src/submessages/timestamp.rs::tests::a_timestamp_body_has_the_dheader_of_an_appendable_time_t`

**Status:** done — `TIMESTAMP_Payload` ist `@extensibility(FINAL)`; `Time_t` hat
in Annex A keine Angabe und ist APPENDABLE (Body: DHEADER 8 und 8 Oktette). Die
8 Oktette der Referenzimplementierung schreibt die Einstellung
`unannotated_types: final` (Rust `UnannotatedTypes::Final`); beide Formen werden
gelesen (f2ace917b5a, 3874ad8d6a6).

### §8.3.5.16 TIMESTAMP_REPLY als Antwort

**Spec:** §8.3.5.16, S. 76 — "The TIMESTAMP_REPLY submessage is used to reply
to a timestamp information message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_timestamp`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::timestamp_reply_echoes_originate_and_uses_real_wall_clock`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_agent_sends_a_timestamp_and_reads_the_reply`,
`crates/xrce-client/src/lib.rs::tests::the_client_sends_and_reads_timestamps`

**Status:** done — der Agent beantwortet TIMESTAMP, der Client beantwortet die
TIMESTAMP des Agent (`send_timestamp_reply`).

### §8.3.5.16 TIMESTAMP_REPLY auf STREAMID_NONE
**Spec:** §8.3.5.16, S. 76 — "The TIMESTAMP_REPLY submessage does not belong
to any stream, for this reason the MessageHeader shall have the streamId set to
STREAM_ID_NONE".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_timestamp`); `crates/xrce-client/src/lib.rs` (`send_control`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::the_timestamp_reply_of_the_agent_is_on_streamid_none`

**Status:** done — der Code setzt `StreamId::NONE` in der Antwort des Agent und des Client; der Test prüft den Header der TIMESTAMP_REPLY des Agent (0f85937a436).
### §8.3.5.16 Flags und Payload TIMESTAMP_REPLY_Payload

**Spec:** §8.3.5.16.1–§8.3.5.16.2, S. 76 — keine zusätzlichen Flags; "struct
TIMESTAMP_REPLY_Payload { Time_t transmit_timestamp; Time_t receive_timestamp;
Time_t originate_timestamp; };"

**Repo:** `crates/xrce/src/submessages/timestamp_reply.rs`
(`TimestampReplyPayload`)

**Tests:** `crates/xrce/src/submessages/timestamp_reply.rs::tests::timestamp_reply_roundtrip_le`,
`crates/xrce/src/submessages/timestamp_reply.rs::tests::timestamp_reply_roundtrip_via_submessage`,
`crates/xrce/src/transport_udp.rs::tests::loopback_roundtrip_timestamp_reply`

**Status:** done — `TIMESTAMP_REPLY_Payload` ist `@extensibility(FINAL)` mit
drei `Time_t`, jedes APPENDABLE (DHEADER 8, zusammen 36 Oktette); mit
`unannotated_types: final` drei mal 8 Oktette; beide Formen werden gelesen
(f2ace917b5a).

### §8.3.5.16 Bedeutung der drei Zeitstempel

**Spec:** §8.3.5.16.2, S. 76 — "The transmit_timestamp shall contain the
timestamp of the sender of the TIMESTAMP_REPLY message at the time the message
is sent. The originate_timestamp shall contain the transmit_timestamp of the
related TIMESTAMP [...]. The receive_timestamp the shall contain the timestamp
at which the related TIMESTAMP message was received."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_timestamp`,
`wall_clock_now`, `handle_timestamp_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::timestamp_reply_echoes_originate_and_uses_real_wall_clock`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_agent_sends_a_timestamp_and_reads_the_reply`

**Status:** done — `originate_timestamp` ist die `transmit_timestamp` der
TIMESTAMP; `transmit_timestamp` und `receive_timestamp` der Antwort des Agent
stammen aus einer Ablesung der Uhr. Aus den vier Zeiten einer TIMESTAMP_REPLY
des Client berechnet der Agent Versatz und Umlaufzeit.

---

## §8.4 Interaction Model

### §8.4.1 Typische Abläufe, teils durch Konfiguration ersetzbar

**Spec:** §8.4.1, S. 77 — "This section describes typical message flows. [...]
some of the message flows are optional and may be replaced by out-of-band
configuration of the XRCE Client and Agent."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung; die Abläufe stehen einzeln in
den folgenden Items.

### §8.4.2 Senden über einen vorkonfigurierten DataWriter

**Spec:** §8.4.2, S. 77 — "The XRCE Agent has been pre-configured to create an
XRCE Application containing a DomainParticipant, Publisher and DataWriter. The
DataWriter pre-configured object_id is known to the XRCE Client. [...] The
reception of the CREATE_CLIENT triggers the creation or reuse of the
corresponding XRCE objects. [...] Subsequent WRITE_DATA messages reference the
ObjectId of those DataWriters". Figure 8: CREATE_CLIENT(reuse = 1),
STATUS_AGENT, WRITE_DATA, DELETE.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`make_client_applications`,
`ensure_predefined`); `crates/xrce-agent/src/daemon/config.rs`
(`client_applications`); `crates/xrce/src/object_id.rs`
(`ObjectId::from_reference`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_preconfigured_object_is_made_only_for_a_client_allowed_to_create_it`,
`crates/xrce-agent/src/daemon/config.rs::tests::client_applications_are_read`,
`crates/xrce/src/object_id.rs::tests::from_reference_matches_the_examples_of_the_spec`

**Status:** done — WRITE_DATA auf die `ObjectId` einer konfigurierten Ressource
(MD5 des Namens, §9.3) legt den DomainParticipant mit allem darin für den Client
an; der AccessController (§7.4) gilt dabei. Der Text nennt ein
`application_object_id` in CREATE_CLIENT und ein Flag `reuse`;
`CLIENT_Representation` (Annex A) und §8.3.5.1.1 haben beides nicht. Auslöser
ist der Agent-Schlüssel `client_applications:` (ClientKey, Application- oder
DomainParticipant-Referenz); das CREATE_CLIENT legt die Objekte an, ein zweites
CREATE_CLIENT derselben Sitzung nutzt sie weiter (03371b4965c, b6e55cfa859).

### §8.4.3 Empfangen über einen vorkonfigurierten DataReader

**Spec:** §8.4.3, S. 77–78 — "The XRCE Agent has been pre-configured to create
an XRCE Application containing a DomainParticipant, Subscriber and DataReader.
The DataReader pre-configured object_id is known to the XRCE Client. [...] A
subsequent READ messag e references the ObjectId of those DataReaders".
Figure 9: CREATE_CLIENT(reuse=1), STATUS_AGENT, READ_DATA, DATA, DELETE.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`ensure_predefined`, `make_client_applications`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_reads_through_a_preconfigured_data_reader_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`

**Status:** done — READ_DATA auf die `ObjectId` des konfigurierten DataReader
legt ihn an, DATA kommt mit der `request_id` des READ_DATA. Dieselbe Abweichung
vom Text (`application_object_id`, `reuse`) und derselbe Agent-Schlüssel
`client_applications:` wie bei §8.4.2 (b6e55cfa859).

### §8.4.4 Discovery eines Agent über GET_INFO

**Spec:** §8.4.4, S. 78–79 — "This flow is only required when the Client is not
pre-configured with the TransportLocator of the XRCE Agent. It allows an XRCE
Client to be configured to content one or more TransportLocators (which may
include multicast addresses) in order to dynamically discover the presence and
actual Address of the Agents." Figure 10: GET_INFO(OBJECTID_AGENT,
CLIENT_Representation), INFO(AGENT_Representation): STATUS_OK, INFO():
STATUS_ERR_INCOMPATIBLE, dann CREATE_CLIENT und STATUS_AGENT.

**Repo:** `crates/xrce-agent/src/daemon/discovery.rs`
(`run_discovery_responder`, `build_info_reply`, `bind_discovery_socket`);
`crates/xrce-agent/src/daemon/session.rs` (`client_info_status`,
`handle_get_info`); `crates/xrce/src/discovery.rs` (`MulticastDiscovery`)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_info_mask_selects_activity_and_configuration`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::a_client_of_another_major_version_is_answered_incompatible`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_discovery_group_is_the_one_given`,
`crates/xrce/src/discovery.rs::tests::loopback_send_recv_roundtrip`

**Status:** done — der Agent beantwortet `GET_INFO(OBJECTID_AGENT)` auf dem Port
der Discovery mit INFO (AGENT_Representation und AGENT_ActivityInfo nach der
InfoMask), mit und ohne Sitzung; die Multicast-Adresse ist einstellbar
(`discovery.address:`). Annex A hat in `GET_INFO_Payload` kein Feld für die
`CLIENT_Representation` der Figure 10; Annex A ist die Vorgabe, die Figure 10
ist eine Erweiterung hinter der InfoMask (`GetInfoPayload::with_client_info`,
Client: `AgentDiscovery::with_client_info`), die ein Agent ohne sie überliest
(5798b040952, 65cd1820a04).

### §8.4.4 Discovery auf Seite des Client

**Spec:** §8.4.4, S. 78–79 — wie oben.

**Repo:** `crates/xrce-client/src/discovery.rs` (`AgentDiscovery`,
`discover_over_udp`, `connect_over_tcp`); `crates/xrce-client/src/lib.rs`
(`XrceClient::request_agent_info`)

**Tests:** `crates/xrce-client/src/discovery.rs::tests::get_info_goes_to_the_candidates_every_period`,
`crates/xrce-client/src/discovery.rs::tests::a_suitable_answer_ends_the_search`,
`crates/xrce-client/src/discovery.rs::tests::answers_that_are_not_suitable_are_passed_over`,
`crates/xrce-client/src/discovery.rs::tests::the_search_over_udp_finds_the_agent_that_answers`,
`crates/xrce-client/src/discovery.rs::tests::the_search_over_tcp_connects_to_the_agent_that_accepts`,
`crates/xrce-client/src/lib.rs::tests::request_agent_info_works_before_the_session_and_sends_the_mask`,
`crates/xrce-client/src/lib.rs::tests::an_info_is_read_into_an_event`

**Status:** done — der Client sendet GET_INFO an eine Liste von Adressen im
festen Takt, wertet das INFO aus und verbindet sich mit dem ersten Agent, der
passt (UDP, TCP). Die Tests erreichen die Agents über Unicast auf Loopback; der
Empfang auf einer Multicast-Gruppe ist nicht getestet (5798b040952).

### §8.4.4 INFO mit STATUS_ERR_INCOMPATIBLE

**Spec:** §8.4.4, Figure 10, S. 79 — "INFO(): STATUS_ERR_INCOMPATIBLE".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`client_info_status`,
`handle_get_info`); `crates/xrce-agent/src/daemon/discovery.rs`
(`build_info_reply`)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::a_client_of_another_major_version_is_answered_incompatible`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_checks_the_version_in_client_info`,
`crates/xrce-client/src/discovery.rs::tests::client_info_names_the_version_and_hides_the_client_key`

**Status:** done — enthält das GET_INFO eine `CLIENT_Representation` mit anderer
Hauptversion als die des Agent, ist die Antwort ein INFO mit
`STATUS_ERR_INCOMPATIBLE`; eine unlesbare Angabe ergibt
`STATUS_ERR_INVALID_DATA`. Die `client_info` ist die Erweiterung hinter der
InfoMask des Items oben (5798b040952).

### §8.4.4 Auswahl unter mehreren Agents

**Spec:** §8.4.4, S. 78 — "the XRCE Client may discover more than one XRCE
Agent. In that case it may use the information received about the XRCE Agent
configuration [...] and the XRCE Agent activity [...] to select the most
appropriate XRCE Agent and even connect to more than one XRCE Agents."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis für den Client.

### §8.4.5 Verbindung mit CREATE_CLIENT und STATUS_AGENT

**Spec:** §8.4.5, S. 79–80 — "An XRCE Client connects to an Agent using the
CREATE_CLIENT message. The Agent responds with a STATUS_AGENT indicating whether
the connection succeeded and the ClientProxy was created on behalf of the XRCE
Client."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::connect`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`,
`agent_status_reply`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::full_lifecycle_creates_unique_request_ids_and_real_submessages`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`

**Status:** done

### §8.4.6 Anlegen einer vollständigen Application

**Spec:** §8.4.6, S. 80 — "An XRCE Client uses the CREATE message to create an
XRCE Application. The CREATE message carries a CREATE_Payload containing an
ObjectVariant with ObjectKind set to OBJK_APPLICATION. The corresponding
OBJK_APPLICATION_Representation may use the REPRESENTATION_BY_REFERENCE to
refer to an Application pre-configured in the Agent or it may use the
REPRESENTATION_AS_XML_STRING to fully describe the Application including any
necessary Types, Qos, and DDS Entities."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, Zweig
`ObjectKind::Application`); `crates/xrce-agent/src/daemon/entity_repr.rs`
(`application_definition`); `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::add_application_library`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`

**Status:** done — CREATE(OBJK_APPLICATION) mit einer `<application_library>`
legt die DomainParticipants der Application mit Topics, Publishern, Subscribern
und Endpunkten an; BY_REFERENCE nennt eine Application der Definitionen des
Agent (3aea52dd70a, b6e55cfa859).

### §8.4.7 Festlegen eines QosProfile

**Spec:** §8.4.7, S. 80 — "An XRCE Client uses the CREATE message to define Qos
Profile. The CREATE message carries a CREATE_Payload containing an
ObjectVariant with ObjectKind set to OBJK_QOSPROFILE. The corresponding
OBJK_QOSPROFILE_Representation may use the REPRESENTATION_AS_XML_STRING to
fully describe the Qos Profile." Mit diesem Profil werden später andere
Objekte angelegt.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, Zweig
`ObjectKind::QosProfile`, `definition_to_register`, `rebuild_profiles`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_qos_profile`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_qos_profile_reference_applies_the_profile`

**Status:** done — das Profil gehört zu den Definitionen des Agent, solange das
Objekt besteht; ein späteres CREATE nennt es als `Bibliothek::Profil`
(`qos_profile_ref`, `base_name`). In Figure 13 sind Client und Agent vertauscht
(3aea52dd70a).

### §8.4.8 Festlegen von Typen
**Spec:** §8.4.8, S. 81 — "An XRCE Client uses the CREATE message to create an
XRCE Type. The CREATE message carries a CREATE_Payload containing an
ObjectVariant with ObjectKind set to OBJK_TYPE. The corresponding
OBJK_TYPE_Representation may use the REPRESENTATION_AS_XML_STRING to fully
describe the DDS-XTYPES Type including any referenced types."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`types`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_type_by_reference_names_a_known_type_by_its_fully_qualified_name`

**Status:** done — ein Typ als `<types>` wird übersetzt und ist unter seinem Namen bekannt; er darf auf Typen verweisen, die der Agent schon kennt (Item zu §7.7.3.3.2 „Verweis auf bekannte Typen“).
### §8.4.9 Anlegen eines Topic

**Spec:** §8.4.9, S. 81 — "An XRCE Client uses the CREATE message to create an
XRCE Topic. The CREATE message carries a CREATE_Payload containing an
ObjectVariant with ObjectKind set to OBJK_TOPIC. The corresponding
OBJK_TOPIC_Representation may use the REPRESENTATION_IN_BINARY or the
REPRESENTATION_AS_XML_STRING to fully define the Topic."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`topic`,
`topic_from_xml`); `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_topic_gets_the_type_its_type_name_names`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_of_an_unknown_type_is_refused_when_the_agent_is_told_to`

**Status:** done — die XML-Form im ersten Test, die binäre Form aus dem
eProsima-Mitschnitt im zweiten. Ein Topic eines Typs, den der Agent nicht kennt,
ergibt `STATUS_ERR_UNKNOWN_REFERENCE` (Agent-Schlüssel `unknown_topic_types:
refuse`, Vorgabe; `forward` legt es an), dritter Test (0da6aef56b5).

### §8.4.10 Anlegen eines DataWriter und Schreiben

**Spec:** §8.4.10, S. 82 — "An XRCE Client uses the CREATE message to create an
XRCE DataWriter. [...] The corresponding DATAREADER_Representation may use the
REPRESENTATION_IN_BINARY or the REPRESENTATION_AS_XML_STRING to fully define
the DataWriter. Both these representations allow specification of the
DataWriter Qos. The DATAREADER_Representation may also use the
REPRESENTATION_BY_REFERENCE to refer to a DataWriter definition known to the
Agent." Figure 16: CREATE, STATUS, WRITE_DATA, DELETE_RESOURCE(DataWriter,
Session).

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`,
`endpoint_from_xml`); `crates/xrce-agent/src/daemon/session.rs`
(`plan_dds_create`, `format_is_allowed`, `handle_write_data`, `handle_delete`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`

**Status:** done — XML im ersten Test, die binäre Form im zweiten. Annex A und
§7.7.3.1 geben dem DataWriter nur Binär und XML; §8.4.10 nennt auch
`REPRESENTATION_BY_REFERENCE`. Annex A ist die Vorgabe (die Referenz ergibt
`STATUS_ERR_INVALID_DATA`, vierter Test); der Agent-Schlüssel
`representation_formats: any` lässt die Referenz zu (dritter Test) (bdf3f6ee966,
f0995555d4a). Der Text schreibt `DATAREADER_Representation` für den DataWriter;
Figure 16 nennt `DELETE_RESOURCE`, eine solche Submessage gibt es nicht (gemeint
ist DELETE).

### §8.4.11 Anlegen eines DataReader und Lesen

**Spec:** §8.4.11, S. 82–83 — "An XRCE Client uses the CREATE message to create
an XRCE DataReader. [...] The corresponding OBJK_DATAREADER_Representation may
use the REPRESENTATION_IN_BINARY or the REPRESENTATION_AS_XML_STRING to fully
define the DataReader. [...] may also use the REPRESENTATION_BY_REFERENCE".
Figure 17: CREATE, STATUS, READ, DATA.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`,
`endpoint_from_xml`); `crates/xrce-agent/src/daemon/session.rs`
(`plan_dds_create`, `format_is_allowed`, `handle_read_data`, `pump_read`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`

**Status:** done — Binär und XML wie beim DataWriter; die Referenz auf einen
DataReader ist nach Annex A und §7.7.3.1 nicht vorgesehen und nur mit dem
Agent-Schlüssel `representation_formats: any` zulässig (dritter und vierter
Test) (bdf3f6ee966). Die Einleitung sagt „to publish data“ für den DataReader.

### §8.4.12 Information über eine Ressource

**Spec:** §8.4.12, S. 83 — "An XRCE Client may use this mechanism to determine
the QoS of any of the DDS proxy entities that the XRCE Agent manages on behalf
of the XRCE Client. It may also be used to read QoS profiles and type
declarations that are known to the XRCE Agent. [...] The XRCE Agent responds
with an INFO message containing an ObjectVariant. The ObjectKind of the
ObjectVariant is the appropriate for the specified ObjectId."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`,
`object_info`, `ensure_predefined`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_writer_reports_its_activity`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_reader_reports_its_activity`

**Status:** done — GET_INFO auf ein Objekt des Client ergibt ein INFO mit der
`ObjectVariant`, mit der es angelegt wurde (mit `ObjectKind`), und der Aktivität
eines DataWriter oder DataReader nach der InfoMask (65cd1820a04). GET_INFO auf
die `ObjectId` eines QoS-Profils oder Typs aus den Definitionen des Agent legt
das Objekt für den Client an (`ensure_predefined`, b6e55cfa859); dieser Zweig
hat keinen eigenen Test.

### §8.4.13 Ändern einer Ressource

**Spec:** §8.4.13, S. 84 — "An XRCE Client uses the CREATE message with the
attribute reuse set to TRUE and the attribute replace set to TRUE to indicate
it wants to update the Object identified by the ObjectId. [...] The XRCE Agent
updates the Object using the new configuration contained in the ObjectVarian t
and responds with a STATUS message."

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore::create`,
`CreateOutcome::Replaced`); `crates/xrce-agent/src/daemon/session.rs`
(`handle_create`); `crates/xrce-client/src/lib.rs` (`XrceClient::update_object`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_update_of_a_missing_object_creates_it_or_is_refused`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_replace_that_fails_returns_the_object_to_its_previous_configuration`,
`crates/xrce/src/object_store.rs::tests::create_reuse_and_replace_replaces_an_unequal_object`,
`crates/xrce-client/src/lib.rs::tests::update_object_sends_a_create_with_reuse_and_replace`

**Status:** done — CREATE mit `reuse` und `replace` ersetzt ein ungleiches
Objekt (Table 5); schlägt die DDS-Seite fehl, gilt wieder die alte
Konfiguration. Das Update eines fehlenden Objekts widerspricht sich in der Spec:
§7.8.3.2 verlangt `STATUS_ERR_UNKNOWN_REFERENCE`, Table 5 und §8.4.13 legen es
an. Vorgabe ist Table 5; der Agent-Schlüssel `update_of_missing_object: refuse`
gibt `STATUS_ERR_UNKNOWN_REFERENCE` (8b949a13ef5, e4d37a6aff6).

### §8.4.14 Zuverlässigkeit je Stream, nur auf zuverlässigen Streams

**Spec:** §8.4.14, S. 84 — "Reliability is implemented separately for each
Stream, and only for the reliable streams which are identified by the stream_id
value being between 0x80 and 0xFF."

**Repo:** `crates/xrce/src/reliable.rs` (`ReliableStreamState::new`);
`crates/xrce-agent/src/daemon/session.rs` (`reliable` je `(client, stream)`,
`handle_message`)

**Tests:** `crates/xrce/src/reliable.rs::tests::constructor_panics_on_best_effort_stream`,
`crates/xrce-agent/src/daemon/session.rs::tests::best_effort_messages_get_increasing_sequence_numbers`

**Status:** done

### §8.4.14 Zwei Endpunkte mit je eigener Zustandsmaschine

**Spec:** §8.4.14, S. 84 — "A Stream has exactly two endpoints, the sending
endpoint and the receiving endpoint. [...] The sender and receiver endpoint on
a Stream each execute its own protocol state machine."

**Repo:** `crates/xrce/src/reliable.rs` (`ReliableStreamState`: Sender
`submit`, `pending_heartbeat`, `recv_acknack`; Empfänger `recv_data`,
`drain_in_order`, `pending_acknack`)

**Tests:** `crates/xrce/src/reliable.rs::tests::end_to_end_sender_receiver_with_loss_recovery`

**Status:** done — beide Hälften liegen in einem Zustand je Stream; Agent und
Client benutzen je Richtung eine davon.

### §8.4.14 Serial Number Arithmetic

**Spec:** §8.4.14, S. 84 — "Sequence number arithmetic and comparisons shall
use Serial Number Arithmetic as specified in clause 8.3.2.3 sequenceNr."

**Repo:** `crates/xrce/src/reliable.rs` (`serial_min_in_flight`,
`serial_max_in_flight`); `crates/xrce/src/serial_number.rs`

**Tests:** `crates/xrce/src/reliable.rs::tests::heartbeat_window_across_wrap_reports_rfc1982_order`,
`crates/xrce/src/reliable.rs::tests::acknack_retransmit_of_lost_sample_across_wrap`

**Status:** done

### §8.4.14.1 Zustand des Senders

**Spec:** §8.4.14.1, S. 85 — "The sender maintains two state variables
associated with the stream. The HighestSentSequenecNumber and the
HighestAcknowledgedSequenceNumber. Each time a message is sent the
HighestSentSequenecNumber is increased. The reception of ACKNACK messages
updates the HiguestAcknowledgedSequenceNumber." Figure 20: "receive(ACKNACK)
/save HigestAcknowledgedSeqNumber repair SequenceNumber in NACK".

**Repo:** `crates/xrce/src/reliable.rs` (`submit`, `recv_acknack`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_acknack`)

**Tests:** `crates/xrce/src/reliable.rs::tests::submit_assigns_monotonic_seqnrs`,
`crates/xrce/src/reliable.rs::tests::recv_acknack_clears_acked_seqnrs`,
`crates/xrce-agent/src/daemon/session.rs::tests::acknack_nack_triggers_retransmission_of_the_exact_buffered_bytes`

**Status:** done — der Sender hält die offenen Nachrichten statt zweier
Zahlen; `window_base` entspricht der höchsten bestätigten Nummer plus eins.

### §8.4.14.1 HEARTBEAT bis zur vollständigen Bestätigung

**Spec:** §8.4.14.1, S. 85 — "While the HighestAcknowledgedSequenceNumber is
less than the HighestSentSequenceNumber the sender sends HeartBeat messages
that announce the HighestSentSequenecNumber to the receiver. These HeartBeat
messages may be periodic or optimized using on vendor specific mechanism. The
requirement is that they are sent at some rate until
HighestAcknowledgedSequenceNumber matches the HighestSentSequenceNumber."

**Repo:** `crates/xrce/src/reliable.rs` (`pending_heartbeat`,
`DEFAULT_HEARTBEAT_PERIOD`); `crates/xrce-agent/src/daemon/session.rs`
(`tick`); `crates/xrce-client/src/lib.rs` (`XrceClient::tick`)

**Tests:** `crates/xrce/src/reliable.rs::tests::pending_heartbeat_fires_first_time`,
`crates/xrce/src/reliable.rs::tests::pending_heartbeat_silenced_until_period_elapsed`,
`crates/xrce/src/reliable.rs::tests::pending_heartbeat_none_when_window_empty`

**Status:** done — alle 500 ms. Figure 20 schreibt den Übergang nach
„Initial“ mit `HiguestReceivedSeqNumber`, der Text mit der höchsten bestätigten
Nummer.

### §8.4.14.2 Zustand des Empfängers

**Spec:** §8.4.14.2, S. 85–86 — "The receiver maintains two state variables
associated with the stream. The HighestReceivedSequenceNumber and the
HighestAnnouncedSequenceNumber. Each time a Message is received the
HighestReceivedSequenceNumber may be updated (assuming all previous messages
have been received). [...] Each time a HEARTBEAT is received the
HighestAnnouncedSequenceNumber may be adjusted."

**Repo:** `crates/xrce/src/reliable.rs` (`recv_data`, `drain_in_order`,
`pending_acknack`)

**Tests:** `crates/xrce/src/reliable.rs::tests::recv_data_buffers_in_order`,
`crates/xrce/src/reliable.rs::tests::pending_acknack_marks_missing_slots`

**Status:** done — die höchste angekündigte Nummer wird nicht gespeichert; sie
geht als `hint_last_seen` in das ACKNACK auf den HEARTBEAT ein.

### §8.4.14.2 Client sendet ACKNACK, solange Nachrichten fehlen

**Spec:** §8.4.14.2, S. 86 — "If the receiver is a n XRCE Client, then while
the HiguestReceivedSequenceNumber is less than the
HighestAnnouncedSequenceNumber, the received sends ACKNACK messages to request
the messages corresponding to the missing sequence numbers. These ACKNACK
messages may be periodic or optimized using on vendor specific mechanism."

**Repo:** `crates/xrce-client/src/lib.rs` (`handle_heartbeat`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::heartbeat_is_answered_with_acknack`

**Status:** done — der Client sendet ACKNACK auf jeden HEARTBEAT, nicht nach
einem eigenen Takt (Figure 21: `NackPeriod`); der Agent sendet HEARTBEAT alle
500 ms, solange Nachrichten offen sind.

### §8.4.14.2 Agent sendet ACKNACK nur auf HEARTBEAT

**Spec:** §8.4.14.2, S. 86 — "If the receiver is the XRCE Agent, then it only
sends ACKNACK messages in response to receiving a HEARTBEAT."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_heartbeat`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::out_of_order_reliable_messages_wait_for_the_gap_and_report_a_truthful_acknack`

**Status:** done — wie im Item zu §8.3.5.11.

---

## §8.5 XRCE Object Operation Traceability

### §8.5 Table 14 create_client

**Spec:** §8.5, Table 14, S. 86 — "XRCE Root / create_client / CREATE_CLIENT /
STATUS_AGENT".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`,
`agent_status_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_client_rejects_wrong_major_version`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`

**Status:** done — ein falsches Cookie wird mit STATUS_AGENT und
`STATUS_ERR_INVALID_DATA` beantwortet (ed2ca7b49a9). Die Überschrift der Table
14 lautet „Predefined XRCE Objects from parsing the Example XML configuration
XML file“, wie die von Table 16.

### §8.5 Table 14 get_info der Root

**Spec:** §8.5, Table 14, S. 86 — "XRCE Root / get_info / GET_INFO / INFO".

**Repo:** `crates/xrce-agent/src/daemon/discovery.rs`
(`run_discovery_responder`); `crates/xrce-agent/src/daemon/session.rs`
(`handle_get_info`)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_a_session_is_answered_for_the_agent_only`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`

**Status:** done

### §8.5 Table 14 delete_client mit STATUS_AGENT
**Spec:** §8.5, Table 14, S. 86 — "XRCE Root / delete_client / DELETE /
STATUS_AGENT".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`delete_client_reply`); `crates/xrce-agent/src/daemon/config.rs` (`delete_client_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::delete_client_is_answered_with_status_unless_status_agent_is_named`, `crates/xrce-agent/src/daemon/config.rs::tests::the_reply_to_delete_client_can_be_set`

**Status:** done — `DELETE(OBJECTID_CLIENT)` wird mit STATUS beantwortet (§8.3.5.4; in der IDL von Annex A trägt nur STATUS ein `ResultStatus`); das STATUS_AGENT von Table 14 ist die Einstellung `delete_client_reply: status_agent`. Der Client nimmt ein STATUS_AGENT, das ohne ausstehendes CREATE_CLIENT eintrifft, für die Antwort auf sein DELETE. a44974c49d7.
### §8.5 Table 14 create

**Spec:** §8.5, Table 14, S. 86 — "XRCE ProxyClient / create / CREATE (flags for
creation) / STATUS".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §8.5 Table 14 update

**Spec:** §8.5, Table 14, S. 86 — "XRCE ProxyClient / update / CREATE (flags for
reuse) / STATUS".

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore::create`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create`);
`crates/xrce-client/src/lib.rs` (`XrceClient::update_object`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`,
`crates/xrce-client/src/lib.rs::tests::update_object_sends_a_create_with_reuse_and_replace`,
`crates/xrce/src/object_store.rs::tests::create_reuse_and_replace_replaces_an_unequal_object`

**Status:** done — wie im Item zu §8.4.13, einschließlich der Einstellung
`update_of_missing_object`. Die Table nennt nur „flags for reuse“, §8.4.13 setzt
`reuse` und `replace`; der Update ist CREATE mit beiden (8b949a13ef5).

### §8.5 Table 14 get_info des ProxyClient

**Spec:** §8.5, Table 14, S. 86 — "XRCE ProxyClient / get_info / GET_INFO /
INFO".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`,
`object_info`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_writer_reports_its_activity`

**Status:** done — GET_INFO auf ein Objekt ergibt INFO, auf eine unbekannte
`ObjectId` ein INFO mit `STATUS_ERR_UNKNOWN_REFERENCE`, ohne Recht
`STATUS_ERR_DENIED` (65cd1820a04, d7338ca58f2).

### §8.5 Table 14 delete

**Spec:** §8.5, Table 14, S. 86 — "XRCE ProxyClient / delete / DELETE /
STATUS".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::delete_fixture_decodes_and_removes_the_object_create_added`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §8.5 Table 14 write

**Spec:** §8.5, Table 14, S. 86 — "XRCE DataWriter / write / WRITE_DATA,
FRAGMENT / STATUS".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`,
`process_message_submessages`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_successful_write_is_answered_with_status_ok`

**Status:** done — WRITE_DATA direkt und in FRAGMENT; ein erfolgreiches
WRITE_DATA ergibt STATUS mit `STATUS_OK`, ein Fehler den Fehlercode
(dc0209b38fe). Der eProsima-Agent antwortet auf ein erfolgreiches WRITE_DATA
nicht; der Agent folgt dem Text der Spec.

### §8.5 Table 14 read

**Spec:** §8.5, Table 14, S. 86 — "XRCE DataReader / read / READ_DATA / DATA,
FRAGMENT, STATUS".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`pump_read`, `emit_possibly_fragmented`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_above_the_clients_mtu_on_a_reliable_stream_splits_into_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`

**Status:** done

---

## §9 XRCE Agent Configuration

### §9.1 Vorwissen des Agent für REPRESENTATION_BY_REFERENCE

**Spec:** §9.1, S. 87 — "The XRCE Agent may be configured such that it has a
priori knowledge XRCE Objects. This allows XRCE Clients to reference and create
XRCE Objects in a very compact manner using the representation format
REPRESENTATION_BY_REFERENCE, see clause 7.7.3.3.1 REPRESENTATION_BY_REFERENCE
format."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis. Der Abschnitt mit dem Titel
„REPRESENTATION_BY_REFERENCE format“ ist §7.7.3.1.1; §7.7.3.3.1 gehört zu
XRCE Type.

### §9.1 Zwei Standardmechanismen, weitere erlaubt

**Spec:** §9.1, S. 87 — "This specification provides two standard mechanism s
to configure the XRCE Agent. Implementations may also provide additional
mechanisms: Remote configuration using the XRCE Protocol; Local file-based
configuration".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Überblick; die beiden Mechanismen stehen in
den Items zu §9.2 und §9.3. Die Profildateien im Format von Fast DDS
(`xml_profile_files:`) sind ein zusätzlicher Mechanismus.

### §9.2 ConfigurationClient mit dem gewöhnlichen Protokoll

**Spec:** §9.2, S. 87 — "An application may use a n XRCE Client with the only
purpose of defining and creating XRCE Objects that are intended for other
applications. [...] The protocol used by the XRCE ConfigurationClient is the
same used by any other XRCE Client. The only difference is that an XRCE
ConfigurationClient never uses the READ_DATA or WRITE messages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung einer Rolle des Client; sie
braucht keine eigene Nachricht.

### §9.2 Objekte des ConfigurationClient für andere Clients

**Spec:** §9.2, S. 87–88 — "Any other XRCE Client can reference XRCE Objects
created by an XRCE ConfigurationClient." Figure 22: der ConfigurationClient legt
mit CREATE an, ein anderer Client mit CREATE(REPRESENTATION_BY_REFERENCE)
und liest mit READ_DATA. "These XRCE Objects are later referenced by a
different XRCE Client using the representation formats
REPRESENTATION_BY_REFERENCE."

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions`);
`crates/xrce-agent/src/daemon/session.rs` (`definition_to_register`,
`rebuild_profiles`, `ensure_predefined`);
`crates/xrce-agent/src/daemon/profiles.rs` (`AgentProfiles::predefined_objects`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`

**Status:** done — ein ConfigurationClient legt QosProfile, Application und
DomainParticipant mit CREATE (XML) an; ein anderer Client erzeugt sie mit
`REPRESENTATION_BY_REFERENCE`, solange das Objekt des ConfigurationClient
besteht (der Test deckt QosProfile und Application). Type und DomainParticipant
laufen über denselben Weg (`definition_to_register`), ohne eigenen Test. Ein
Domain hat keine `ObjectKind` (§7.7.5, Annex A) und kommt aus den Dateien.
Figure 22 nennt die Antwort auf CREATE_CLIENT `STATUS_CLIENT`; die Submessage
heißt STATUS_AGENT (3aea52dd70a, b6e55cfa859).

### §9.2 Anderes Netz oder anderer Transport

**Spec:** §9.2, S. 87 — "Note that the XRCE ConfigurationClient may be
communicating with the Agent using a different network or transport, which may
not have the same constraints as a typical XRCE Client."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis.

### §9.3 Laden einer XML-Datei nach dds-xml_system_example.xsd

**Spec:** §9.3, S. 88 — "The XRCE Agent shall provide a configuration or
run-time option to load a n XML file formatted according to the schema defined
in the [DDS-XML] machine-readable file dds-xml_system_example.xsd."

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`xml_profile_files`,
`qos_library_files`); `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::load`, `AgentProfiles::add_profiles`);
`crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::add_document`)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::types_and_qos_libraries_in_a_dds_document_are_read`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`

**Status:** done — `xml_profile_files:` nimmt ein Dokument mit der Wurzel
`<dds>` (oder eine Bibliothek als Wurzel) und liest `<types>`, `<qos_library>`,
`<domain_library>`, `<domain_participant_library>` und `<application_library>`
(dc63d65691f, 3aea52dd70a). Die Schemadatei liegt nicht im Cache; geprüft ist
gegen §9.3.1.

### §9.3 Domain-, Application- und DomainParticipant-Libraries der Konfigurationsdatei

**Spec:** §9.3, S. 88 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::add_profiles`); `crates/xrce-agent/src/daemon/definitions.rs`
(`add_domain_library`, `add_participant_library`, `add_application_library`)

**Tests:** `crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::a_library_can_be_the_root_and_a_clash_of_ids_is_refused`

**Status:** done — die drei Libraries werden gelesen, auch als Wurzel des
Dokuments, mit `base_name`-Vererbung. Andere Elemente von `<dds>` stehen nur im
Startprotokoll (`AgentProfiles::unused`) (dc63d65691f, 3aea52dd70a).

### §9.3 Ein XRCE-Objekt je Element, für alle Clients per Referenz

**Spec:** §9.3, S. 88 — "The XRCE Agent shall parse the XML file and for each of
the elements defined in Table 15, it shall construct the corresponding XRCE
Object specified in Table 15. All the created XRCE Objects shall be made
available to XRCE clients such that they may refer to them using the
representation format REPRESENTATION_BY_REFERENCE."

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::predefined_objects`); `crates/xrce-agent/src/daemon/session.rs`
(`ensure_predefined`, `predefined_index`); `crates/xrce/src/object_id.rs`
(`ObjectId::from_reference`)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::a_library_can_be_the_root_and_a_clash_of_ids_is_refused`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce/src/object_id.rs::tests::from_reference_matches_the_examples_of_the_spec`

**Status:** done — der Agent kennt jedes Element von Table 15 als Objekt mit der
`ObjectId` aus dem MD5 der Referenz (§9.3, Table 16); jeder Client erhält es mit
CREATE und `REPRESENTATION_BY_REFERENCE` oder beim ersten Zugriff auf die
`ObjectId`. Zwei Objekte oberster Ebene mit derselben `ObjectId` weist das Laden
der Datei ab (03371b4965c, b6e55cfa859).

### §9.3 Table 15 <types>

**Spec:** §9.3, Table 15, S. 88 — "<types> / XRCE Type. / The created XRCE Types
shall be referenceable using their fully qualified name, which includes the
names of enclosing modules. For example: 'MyModule::MyNestedModule::MyStructType'"

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs` (`add_types`,
`AgentProfiles::resolve_type_ref`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`types`);
`crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, Zweig
`ObjectKind::Type`)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::types_and_qos_libraries_in_a_dds_document_are_read`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_xml_type_library_compiles_to_named_complete_types`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_configured_type_is_known_to_every_client`

**Status:** done — die Typen sind für alle Clients unter ihrem vollständigen
Namen mit Modulen bekannt (`ShapesDemoTypes::ShapeType`); ein Topic findet sie
über `type_name`. CREATE mit `OBJK_TYPE` und `REPRESENTATION_BY_REFERENCE` nimmt
einen bekannten Namen an (`plan_dds_create`, ohne eigenen Test). Verweise eines
`<types>` auf andere bekannte Typen: Item zu §7.7.3.3.2 (dc63d65691f,
3aea52dd70a).

### §9.3 Table 15 <qos_profile>

**Spec:** §9.3, Table 15, S. 88 — "<qos_profile> (Child of <qos_library>) /
XRCE QosProfile. / The created XRCE Types shall be referenceable using their
fully qualified name, which includes the names of enclosing Qos Profile
Library. For example: 'MyProfileLibrary::MyQosProfile'"

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::qos_profile`, `AgentProfiles::predefined_objects`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_qos_profile`);
`crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, Zweig
`ObjectKind::QosProfile`)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::types_and_qos_libraries_in_a_dds_document_are_read`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_qos_profile_reference_applies_the_profile`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`

**Status:** done — ein Endpunkt nennt das Profil mit `Bibliothek::Profil`;
CREATE mit `OBJK_QOSPROFILE` legt ein Profil an, das spätere CREATE benutzen
(3aea52dd70a). Die Zeile schreibt „XRCE Types“ für das QosProfile.

### §9.3 Table 15 <domain>

**Spec:** §9.3, Table 15, S. 89 — "<domain> (Child of <domain_library>) / XRCE
Domain. / The created XRCE Domain shall be referenceable using their fully
qualified name, which includes the names of enclosing Domain Library. For
example: 'MyDomainLibrary::MyDomain'"

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`DomainDef`,
`SystemDefinitions::add_domain_library`, `SystemDefinitions::domain`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`: `domain_ref`,
`domain_reference`)

**Tests:** `crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_domain_reference_is_resolved_against_the_domain_libraries`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`

**Status:** done — `<domain_library>` wird gelesen; ein Domain ist unter
`Bibliothek::Domain` benannt und wird von `domain_ref` (XML) und
`domain_reference` (binär) eines DomainParticipant aufgelöst. Der Domain hat
keine `ObjectKind` (§7.7.5, Annex A): ein CREATE kann ihn nicht nennen, er hat
keine `ObjectId` (dc63d65691f, b4cd825c268).

### §9.3 Table 15 <topic> in <domain>

**Spec:** §9.3, Table 15, S. 89 — "<topic> (Child of <domain>) / XRCE Topic /
The created XRCE Topic shall be referenceable using its name from any
DomainParticipant that references the Domain where the Topic is defined. For
example: 'ExampleTopic'"

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`DomainDef::topics`,
`SystemDefinitions::scope_of`, `ParticipantScope::topic`)

**Tests:** `crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`

**Status:** done — die Topics eines Domain stehen im Geltungsbereich jedes
DomainParticipant, der den Domain nennt (§7.7.3.7.1); ein DataWriter oder
DataReader findet sie unter ihrem Namen, auch über `base_name` eines Domain
(3aea52dd70a).

### §9.3 Table 15 <application>

**Spec:** §9.3, Table 15, S. 89 — "<application> (Child of
<application_library>) / XRCE Application. / The created XRCE Application shall
be referenceable using their fully qualified name, which includes the names of
enclosing Application Library. For example:
'MyApplicationLibrary::MyApplication'"

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::add_application_library`, `application`);
`crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::predefined_objects`); `crates/xrce/src/object_id.rs`
(`ObjectId::from_reference`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`

**Status:** `done` — Commits `dc63d65691f`, `b6e55cfa859`. `<application_library>` wird
gelesen, auch als Wurzel einer Datei. Die Application ist unter
„Bibliothek::Application“ bekannt (Beispiel „Apps::App“), ihre ObjectId stammt
aus dem MD5 dieser Zeichenkette. Ein anderer Client erzeugt sie mit
REPRESENTATION_BY_REFERENCE, solange das Objekt besteht, das sie definiert hat.

### §9.3 Table 15 <domain_participant>

**Spec:** §9.3, Table 15, S. 89 — "<domain_participant> (Child of
<domain_participant _library>) / XRCE DomainParticipant / The created XRCE
DomainParticipant shall be referenceable using their fully qualified name,
which includes the names of enclosing DomainParticipant Library. For example:
'MyParticipantLibrary::MyParticipant'"

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::add_participant_library`, `participant`,
`plan_participant`); `crates/xrce-agent/src/daemon/session.rs` (`create_nested`,
`ensure_predefined`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_creates_the_objects_inside_it`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::a_library_can_be_the_root_and_a_clash_of_ids_is_refused`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`

**Status:** `done` — Commits `dc63d65691f`, `3aea52dd70a`, `b6e55cfa859`.
`<domain_participant_library>` wird gelesen, auch als Wurzel einer Datei. Der
Participant ist unter „Bibliothek::Participant“ bekannt, in einer Application
unter „Bibliothek::Application::Participant“ (Table 16:
„MyApplications::ShapesDemoApp::MyParticipant“); seine ObjectId stammt aus dem
MD5 der Zeichenkette. Die Form `<participant profile_name="…">` einer
Profildatei im Format von Fast DDS bleibt daneben lesbar.

### §9.3 Table 15 <topic> in <domain_participant>

**Spec:** §9.3, Table 15, S. 89 — "<topic> (Child of <domain_participant>) /
XRCE Topic / The created XRCE Topic shall be referenceable using its name from
any objects in the same DomainParticipant. For example: 'ExampleTopic'"

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`plan_participant`: die
Topics des Participant und seiner Domain);
`crates/xrce-agent/src/daemon/session.rs` (`create_nested`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_topic_is_created_by_name_or_by_register_type_ref`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`

**Status:** `done` — Commits `3aea52dd70a`, `9239562286c`. Ein Topic im Participant ist
ein Objekt mit der ObjectId seines Namens (Beispiel „Square“ {0xce, 0xb2}). Die
Objekte desselben Participant nennen es mit diesem Namen (`topic_ref="Square"`
eines DataWriter wird zur ObjectId des Topic); ein Name, den der Participant
nicht hält, wird abgewiesen. Die Form `<topic profile_name="…">` im Format von
Fast DDS bleibt daneben lesbar.

### §9.3 Table 15 <publisher> und <subscriber>

**Spec:** §9.3, Table 15, S. 89 — "<publisher> <subscriber> (Child of
<domain_participant>) / XRCE Publisher, XRCE Subscriber / The created XRCE
Publisher or Subscriber shall be referenceable using their name. No
qualification is necessary since these entities are always referenced within
the scope of a DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`plan_participant`);
`crates/xrce-agent/src/daemon/session.rs` (`create_nested`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`container_for` prüft, dass Topic
und Publisher/Subscriber zum selben Participant gehören)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::what_cannot_be_planned_is_refused`

**Status:** `done` — Commits `3aea52dd70a`, `9239562286c`. Publisher und Subscriber
sind Objekte mit der ObjectId ihres Namens (Beispiel „MyPublisher“ {0x13,
0xe3}), ohne Bibliothek davor, und gelten im Bereich ihres Participant. Zwei
Participants desselben Client, die eine Entität gleichen Namens definieren,
leiten dieselbe ObjectId ab (die Spec leitet nur aus dem Namen ab); die zweite
ist `STATUS_ERR_ALREADY_EXISTS` (Befund aus dem Code, nicht durch einen Test
belegt).

### §9.3 Table 15 <data_writer> und <data_reader>

**Spec:** §9.3, Table 15, S. 89 — "<data_writer> < data_reader> (Child of
<domain_participant>) / XRCE DataWriter, XRCE DataReader / The created XRCE
DataWriter or DataReader sha ll be referenceable using their name. No
qualification is necessary since these entities are always referenced within
the scope of a Publisher or Subscriber."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_embedded`,
`create_nested`, `ensure_predefined`);
`crates/xrce-agent/src/daemon/definitions.rs` (`plan_participant`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_reads_through_a_preconfigured_data_reader_without_a_create`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`

**Status:** `done` — Commits `3aea52dd70a`, `9239562286c`, `b6e55cfa859`. DataWriter
und DataReader sind Objekte mit der ObjectId ihres Namens (Beispiel
„MySquareWriter“ {0x1c, 0xc5}) und hängen unter ihrem Publisher oder Subscriber
(§9.3.1; Table 15 nennt `<domain_participant>` als Elternelement). Ein Client
schreibt und liest über sie ohne CREATE.

### §9.3 ObjectId aus dem MD5 der Referenz

**Spec:** §9.3, S. 89 — "The XRCE Objects created from the file-based
configuration shall have their ObjectId automatically derived from the
REPRESENTATION_BY_REFERENCE string. Specifically, the ObjectIdPrefix (see
7.7.6) shall be set to the first 2 bytes of the MD5 hash computed on the
REPRESENTATION_BY_REFERENCE string. The MD5 treats each string character as a
byte and does not include the NUL terminating character of the string."
Beispiel: "MyWriter", MD5 0x03e26181adfef529038bf0dce7cab871,
ObjectIdPrefix {0x03, 0xe2}.

**Repo:** `crates/xrce/src/object_id.rs` (`ObjectId::from_reference`: MD5 der
Zeichenkette ohne NUL, die ersten zwei Bytes, die unteren 4 Bit durch die
ObjectKind ersetzt)

**Tests:** `crates/xrce/src/object_id.rs::tests::from_reference_matches_the_examples_of_the_spec`

**Status:** `done` — Commits `03371b4965c`. „MyWriter“ ergibt den MD5
0x03e26181adfef529038bf0dce7cab871 (Test mit dem Crate `md-5`; nachgerechnet
auch mit Python `hashlib`, gemessen), das ObjectIdPrefix {0x03, 0xe2} und mit
der Art DataWriter die ObjectId {0x03, 0xe5}. Der letzte Satz der Spec schreibt
„The ObjectIdPrefix shall be computed by combining the ObjectIdPrefix with the
ObjectKind“; gemeint ist die ObjectId. Der Kommentar in
`crates/xrce/src/xml_config.rs` verweist auf `ObjectId::from_reference`.

### §9.3.1 Beispieldatei

**Spec:** §9.3.1, S. 90–91 — "The following XML file could be used to configure
an XRCE Agent." `<types>` mit Modul `ShapesDemoTypes`, `<qos_library
name="MyQosLibrary">`, `<application_library name="MyApplications">` mit
`<application name="ShapesDemoApp">`, Participant `MyParticipant`, drei Topics,
`MyPublisher` mit `MySquareWriter` und `MyCircleWriter`, `MySubscriber` mit
`MyTriangleRdr`.

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::add_profiles` liest die Datei)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`

**Status:** `n/a (informative)` — Beispiel. Der Test lädt die Datei mit
ergänzter Domain und ergänztem Typ. `domain_ref` nennt
„ShapesDomainLibrary::ShapesDomain“, das die Datei nicht definiert;
`register_type type_ref="ShapeType"` nennt den Typ ohne sein Modul
`ShapesDemoTypes`.

### §9.3.1 Table 16 vordefinierte Objekte

**Spec:** §9.3.1, Table 16, S. 92 — "An XRCE Agent loading the above
configuration file would have the pre-defined XRCE Objects shown in Table 16."
Zwölf Zeilen mit Referenz, ObjectPrefix und ObjectId, z. B.
„ShapesDemoTypes::ShapeType“ {0x59, 0x51} {0x59, 0x5a}.

**Repo:** `crates/xrce/src/object_id.rs` (`ObjectId::from_reference`);
`crates/xrce-agent/src/daemon/profiles.rs` (`AgentProfiles::predefined_objects`)

**Tests:** `crates/xrce/src/object_id.rs::tests::from_reference_matches_the_examples_of_the_spec`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`

**Status:** `done` — Commits `03371b4965c`, `dc63d65691f`. Der Test in `object_id.rs`
rechnet alle zwölf Zeilen der Table 16 aus ihrer eigenen Zeichenkette nach
(Prefix, ObjectId, Art im unteren Halbbyte). Die Tests des Agent prüfen die
ObjectIds der Objekte, die er aus der Datei ableitet. Zwei Zeichenketten der
Table 16 weichen von der Datei in §9.3.1 ab:
„MyApplications::SimpleShapesDemoApplication“ statt
„MyApplications::ShapesDemoApp“ (MD5-Prefix von „ShapesDemoApp“: {0xeb, 0x1d})
und „MyTriangleReader“ statt „MyTriangleRdr“ (MD5-Prefix von „MyTriangleRdr“:
{0x50, 0x38}). Der Agent leitet nach der Regel aus §9.3 aus den Namen der Datei
ab; die ObjectIds dieser beiden Zeilen weichen deshalb von der Tabelle ab. Ein
Schalter für die andere Lesart besteht nicht. Der Participant heißt
„MyApplications::ShapesDemoApp::MyParticipant“, nicht nach dem Muster der Table
15.

---

## §10 XRCE Deployments

### §10 Operationen mit einem Agent, Austausch über DDS

**Spec:** §10, S. 93 — "All the operations described in the DDS-XRCE PIM
pertain to the interaction of a client application with a single DDS-XRCE
Agent. [...] client applications may interact with each other despite
connecting to different DDS-XRCE Agents. These interactions would happen as a
consequence of the DDS-XRCE Agents creating and performing operations on DDS
DomainParticipant entities".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung.

### §10.1 Client über den Agent zu jedem DDS-Participant

**Spec:** §10.1, S. 93 — "The DDS DomainParticipant will discover the proxy DDS
Entities that the XRCE Agent creates on behalf of the Client and with use the
standard DDS-RTPS Interoperability protocol to communicate wit the Agent. The
XRCE Client will communicate with the XRCE Agent using the XRCE Protocol. Using
this protocol it can direct the XRCE Agent to create new DDS entities and use
these entities to read and write data on the DDS Global Data Space."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort` Z. 310,
`runtime_for` Z. 396)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::full_chain_against_a_real_leased_domain`

**Status:** done — der erste Test schreibt und liest gegen einen eigenen
DCPS-Participant im selben Prozess.

### §10.2 Clients an verschiedenen Agents über DDS
**Spec:** §10.2, S. 93 — "XRCE Agents appear as DDS DomainParticipants in the
DDS Dom ain. For this reason XRCE Client applications that are connected to
different XRCE Agents will communicate with each other without the need for
further configuration."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`runtime_instance`, `runtime_for`)

**Tests:** `crates/xrce-agent/tests/two_agents.rs::two_clients_on_two_agents_exchange_a_sample_through_dds`

**Status:** done — zwei Daemons in einer Domain, ein DataWriter über den ersten, ein DataReader über den zweiten: das Sample kommt mit gleichen Bytes an. Die GUID-Präfixe der Runtimes waren aus Domain und Host allein abgeleitet, zwei Agents auf einem Host entdeckten sich nicht; sie tragen Prozess und Zähler (3068f2df50c). Der Test ist geschrieben, nicht ausgeführt.
### §10.3 Mehrere Clients an einem Agent wie mit eigenen Entitäten

**Spec:** §10.3, S. 94 — "Multiple XRCE Client applications may be connected to
the same XRCE Agent. In it up to the implementation of the XRCE Agent whether
the DDS Entities it creates are exclusive to each XRCE Client or alternatively
are shared across XRCE Clients. However, the behavior observable by the XRCE
Client shall be as if the DDS XRCE Agent creates separate DDS Objects exclusive
to each XRCE Client. [...] If the XRCE Agent shares DDS entities among
different XRCE Clients, then the requiremen t to behave 'as if' each had its
own separate entities requires that the local DDS DataWriter entities discover
and match the local DDS DataReader entities in the same DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`runtime_for` Z. 396:
ein `DcpsRuntime` je Domain für alle Clients, ein weiterer je Domain und
USER_DATA über `own_runtime_slot`; `create_writer` Z. 654, `create_reader` Z.
716)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::two_large_samples_reach_a_default_reader_on_the_same_agent`

**Status:** `done` — der Writer eines Client erreicht den Reader eines anderen
Client im selben `DcpsRuntime`. Eigene GUID und QoS des Participant sind je
Client nicht geprüft; ein Participant mit eigenem USER_DATA bekommt nach dem
Code einen eigenen `DcpsRuntime` (gelesen, nicht durch einen Test belegt).

### §10.3 Abkürzung zwischen lokalem Writer und Reader

**Spec:** §10.3, S. 94 — "An implementation of an XRCE Agent may choose to
create faster communication path between the local XRCE DataWriter and
DataReader objects [...]. This 'shortcut' can be implemented as an
optimization".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis; der Agent geht über den
`DcpsRuntime`.

### §10.4 Föderation von Agents

**Spec:** §10.4, S. 95 — "The specification supports federated deployments
where XRCE Agents appear as Clients to other XRCE Agents. In other to support
these deployments the XRCE Agent implementation must implement the client -side
of the XRCE Protocol in addition to the server part. Supporting this kind of
deployment is an implementation decision".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Entscheidung der Implementierung; der Agent
hat keine Seite als Client.

### §10.5 Direkte Kommunikation zwischen Anwendungen

**Spec:** §10.5, S. 96 — "The specification supports applications having
direct communications using only the XRCE Protoco l. In order to do this each
application must implement both the XRCE Client and the XRCE Agent part of the
protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Möglichkeit der Bereitstellung.

### §10.6 Kombinierte Bereitstellung

**Spec:** §10.6, S. 97–98 — "Figure 28 below illustrates a scenario where the
different deployments are combined into a single system."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Abbildung.

---

## §11 Transport Mappings

### §11.1 Nicht an einen Transport gebunden

**Spec:** §11.1, S. 99 — "The XRCE protocol is not limited to any specific
transports. It can be mapped to most existing network transports such as UDP,
TCP and low bandwidth transports such as Bluetooth, ZigBee and 6LoWPAN."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung; ZeroDDS hat UDP, TCP und eine
serielle Leitung (Items zu §11.2, §11.3 und Annex C).

### §11.1 Erwartete Eigenschaften (1) bis (4)

**Spec:** §11.1, S. 99 — "To run without additional overhead, it is expected
that the transport supports the following functionality: (1) Deliver messages
of at least 64 bytes. (2) Handle the integrity of messages, dropping any
messages that are corru pted. [...] (3) Provide the size of the received
message as well as the source address. [...] (4) Support bi-directional
communication."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erwartung an den Transport. Die serielle
Leitung ergänzt Länge, Adressen und CRC (Items zu Annex C).

### §11.1 Erwartete Eigenschaft (5) Sicherheit

**Spec:** §11.1, S. 99 — "(5) Provide transport-level security, specifically the
means for the Client to authenticate the Agent and the means for secure
(encrypted and authenticated) message exchange. Alternatively , the XRCE Agent
and Client can be deployed on top of a secure network layer (e.g. an encrypted
VPN)."

**Repo:** `crates/xrce-agent/src/daemon/transport/secure.rs`
(`run_tls_listener`, `run_dtls_listener`);
`crates/xrce-agent/src/daemon/config.rs` (`ListenerConfig::Tls`,
`ListenerConfig::Dtls`)

**Tests:** `crates/xrce-agent/tests/secure_listeners.rs::tls::a_client_that_trusts_the_agents_certificate_opens_a_session`,
`crates/xrce-agent/tests/secure_listeners.rs::dtls::a_client_that_trusts_the_agents_certificate_opens_a_session`

**Status:** `n/a (informative)` — Erwartung an den Transport. Der Agent hat
unter den Features `tls` und `dtls` die Listener `kind: tls` und `kind: dtls`
mit Zertifikat und Schlüssel des Agent (`01470e273bc`): der Client prüft den
Agent, die Nachrichten laufen verschlüsselt (Item zu §8.3.2.4 „Schutz des
ClientKey durch den Transport“). Die Listener `udp`, `tcp` und `serial` sind
nicht gesichert. Die Tests laufen nur mit dem jeweiligen Feature.

### §11.1 Nicht verlangt: Zuverlässigkeit, Reihenfolge, Meldung von Verlusten

**Spec:** §11.1, S. 99 — "The following functionality is explicitly not
required from the transport: (1) It does not need to provide reliability. [...]
(2) It does not need to provide ordering. [...] (3) It does not need to provide
notification of dropped messages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis; Zuverlässigkeit und Reihenfolge
stehen in den Items zu §8.3.2.3 und §8.4.14.

### §11.1 Hülle für fehlende Eigenschaften

**Spec:** §11.1, S. 99 — "Transports that do not meet some of the above
pre-requisites may still be used by adding the missing information as an
envelope around the XRCE message. [...] If the transport does not support
integrity a CRC suffix could be added to the XRCE message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Möglichkeit; die Hüllen für TCP und die
serielle Leitung stehen in den Items zu §11.3.3 und Annex C.

### §11.2 UDP ohne Sicherheit, DTLS oder VPN

**Spec:** §11.2, S. 99 — "The UDP transport meets all the functionality listed
in clause 11.1. Except that it does not provide security. For applications
requiring security there is the 'Datagram Transport Layer Security' (DTLS)
standard [DTLS] that provides security in top of UDP/IP. Alternatively, UDP mat
be deployed on a private network (VPN)".

**Repo:** `crates/xrce-agent/src/daemon/transport/secure.rs`
(`run_dtls_listener`); `crates/xrce/src/transport_dtls.rs` (`WebrtcDtls`,
`WebrtcDtlsServer`)

**Tests:** `crates/xrce-agent/tests/secure_listeners.rs::dtls::a_client_that_trusts_the_agents_certificate_opens_a_session`,
`crates/xrce-agent/tests/secure_listeners.rs::dtls::a_plain_text_create_client_on_the_dtls_port_is_not_answered`,
`crates/xrce/tests/dtls_e2e.rs::dtls_handshake_and_roundtrip`

**Status:** `n/a (informative)` — Hinweis. Der Agent hat unter dem Feature
`dtls` den Listener `kind: dtls` (DTLS 1.2 mit dem Zertifikat des Agent,
`01470e273bc`); ein CREATE_CLIENT im Klartext auf diesem Port wird nicht
beantwortet. Die Tests laufen nur mit diesem Feature.

### §11.2.1 UDP v4 mit ADDRESS_FORMAT_MEDIUM

**Spec:** §11.2.1, S. 99 — "When XRCE is mapped to the UDP v4, the
TransportLocator union shall use the TransportLocatorFormat discriminator
ADDRESS_FORMAT_MEDIUM. This selects the member medium_locator of type
TransportLocatorMedium [...] struct TransportLocatorMedium { octet address[4];
unsigned short port; };"

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators` Z. 438);
`crates/xrce/src/transport_locator.rs` (`TransportLocatorMedium`,
`TransportLocator::encode_member`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`,
`crates/xrce/src/transport_locator.rs::tests::medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag`,
`crates/xrce/src/transport_locator.rs::tests::a_udp_locator_carries_the_default_agent_port`

**Status:** `done` — Commits `c304e509660`. ein UDP-Listener mit IPv4-Adresse wird als
`TransportLocator::Medium` (Diskriminator ADDRESS_FORMAT_MEDIUM) gemeldet: vier
Oktette Adresse, ein Füllbyte, der Port als `unsigned short` in der
Byte-Reihenfolge des E-Flags.

### §11.2.1 UDP v6 mit ADDRESS_FORMAT_LARGE

**Spec:** §11.2.1, S. 99–100 — "When XRCE is mapped to the UDP v6, the
TransportLocator union shall use the TransportLocatorFormat discriminator
ADDRESS_FORMAT_LARGE. [...] struct TransportLocatorLarge { octet address[16];
unsigned long port; }; The address field shall contain the IP v6 address and the
port field shall contain the UDP/IP v6 port number."

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators` Z. 438);
`crates/xrce/src/transport_locator.rs` (`TransportLocatorLarge`,
`TransportLocator::encode_member`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`,
`crates/xrce/src/transport_locator.rs::tests::large_has_three_padding_octets_and_a_32_bit_port`

**Status:** `done` — Commits `c304e509660`. ein UDP-Listener mit IPv6-Adresse wird als
`TransportLocator::Large` gemeldet: 16 Oktette Adresse, drei Füllbytes, der Port
als `unsigned long`.

### §11.2.2 Agent als UDP-Server, Client als UDP-Client

**Spec:** §11.2.2, S. 100 — "The UDP Server sends replies back the Client using
the address and port received in the message. [...] When communicating over UDP
the XRCE Agent shall behave as an UDP Server and the XRCE Client as the UDP
Client."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`run_udp_listener` Z.
428, `PeerTable`, `Route::Udp`); `crates/xrce/src/transport_udp.rs`
(`XrceUdpSender::bind` Z. 54)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::udp_listener_answers_create_client_over_loopback`,
`crates/xrce-agent/src/daemon/transport.rs::tests::a_client_registering_again_is_answered_at_its_new_address`

**Status:** done — die Antwort geht an die Adresse, unter der sich der Client
zuletzt mit CREATE_CLIENT gemeldet hat; ohne Eintrag an die Quelladresse.

### §11.2.2 Port des Agent vorkonfiguriert

**Spec:** §11.2.2, S. 100 — "The XRCE Agent shall be pre-configured with the
port number it shall listen to."

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`ListenerConfig::Udp`,
Vorgabe `0.0.0.0:2018`)

**Tests:** `crates/xrce-agent/src/daemon/config.rs::tests::default_config_has_one_udp_listener_on_2018`,
`crates/xrce-agent/src/daemon/config.rs::tests::a_listener_can_be_written_as_one_string`

**Status:** done — die Spec nennt keinen Standardport. `agent_default_port`
in `crates/xrce/src/transport_udp.rs` (Z. 27) rechnet 7400 + 4 · Domain; der
Agent benutzt die Funktion nicht.

### §11.2.2 Adresse und Port des Agent im Client vorkonfiguriert

**Spec:** §11.2.2, S. 100 — "The XRCE Client shall be pre-configured with the
UDP/IP address and port of the XRCE Agent."

**Repo:** `crates/xrce/src/transport_udp.rs` (`XrceUdpSender::bind` Z. 54);
`crates/xrce-client/src/lib.rs` (`ClientTransport` Z. 89)

**Tests:** `crates/xrce/src/transport_udp.rs::tests::loopback_roundtrip_create_client`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done — `XrceClient` bekommt die Adresse über seinen
`ClientTransport`; `crates/xrce-client` selbst hat keinen UDP-Transport, der
Test baut ihn in `e2e_udp_loopback.rs`.

### §11.2.3 Keine Hülle, genau eine Nachricht je Datagramm

**Spec:** §11.2.3, S. 100 — "The mapping of the XRCE Protocol to UDP/IP does not
add any additional envelopes around the XRCE message. The UDP/IP payload shall
contain exactly one XRCE message."

**Repo:** `crates/xrce/src/transport_udp.rs` (`send_message` Z. 67,
`recv_message` Z. 89)

**Tests:** `crates/xrce/src/transport_udp.rs::tests::rfc768_udp_delivers_one_datagram_per_message_with_boundaries_intact`,
`crates/xrce/src/transport_udp.rs::tests::rfc768_datagram_larger_than_udp_payload_limit_is_rejected`

**Status:** done

### §11.2.4 Multicast-Adresse und Port des Agent

**Spec:** §11.2.4, S. 100 — "XRCE Agent discovery may be done using UDP/IP
multicast. The XRCE Agents shall be pre-configured with the multicast address
and port number they shall listen to. By default, they shall be the address
239.255.0.2 and the port 7400."

**Repo:** `crates/xrce/src/discovery.rs` (`XRCE_DISCOVERY_GROUP`,
`XRCE_DISCOVERY_PORT`); `crates/xrce-agent/src/daemon/discovery.rs`
(`bind_discovery_socket` Z. 146); `crates/xrce-agent/src/daemon/config.rs`
(`discovery_address`, `discovery_port`)

**Tests:** `crates/xrce/src/discovery.rs::tests::discovery_constants_match_spec`,
`crates/xrce/src/discovery.rs::tests::the_discovery_port_is_bound_shared`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_discovery_group_is_the_one_given`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_discovery_group_can_be_set`

**Status:** `done` — Commits `5798b040952`. Vorgabe Gruppe 239.255.0.2 und Port 7400;
`discovery.address:` und `discovery.port:` stellen beide ein (Item unten).

### §11.2.4 Einstellung der Multicast-Adresse

**Spec:** §11.2.4, S. 100 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`discovery_address`,
Schlüssel `discovery.address`); `crates/xrce-agent/src/daemon/discovery.rs`
(`bind_discovery_socket` Z. 146); `crates/xrce-agent/src/daemon/server.rs`
(Aufruf Z. 400)

**Tests:** `crates/xrce-agent/src/daemon/config.rs::tests::the_discovery_group_can_be_set`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_discovery_group_is_the_one_given`

**Status:** `done` — Commits `5798b040952`. der Agent tritt der eingestellten Gruppe
bei; eine Adresse, die keine IPv4-Multicast-Adresse ist, lehnen die
Konfiguration und `bind_discovery_socket` ab. Beim Client ist die
Multicast-Adresse ein Eintrag der Kandidatenliste von `discover_over_udp`.

### §11.2.4 Periodisches GET_INFO des Client an die Multicast-Adresse

**Spec:** §11.2.4, S. 100 — "To discover Agents via multicast the XRCE Client
shall send the GET_INFO message (see 8.3.5.3) periodically to the configured
multicast address and port. [...] The XRCE Client shall stop sending the
periodic message once it receives a suitable response from an Agent."

**Repo:** `crates/xrce-client/src/discovery.rs` (`AgentDiscovery::poll`,
`on_datagram`, `discover_over_udp`); `crates/xrce/src/discovery.rs`
(`MulticastDiscovery::send_multicast`)

**Tests:** `crates/xrce-client/src/discovery.rs::tests::get_info_goes_to_the_candidates_every_period`,
`crates/xrce-client/src/discovery.rs::tests::a_suitable_answer_ends_the_search`,
`crates/xrce-client/src/discovery.rs::tests::the_search_over_udp_finds_the_agent_that_answers`

**Status:** `done` — Commits `5798b040952`. `GET_INFO(OBJECTID_AGENT)` ohne Sitzung und
ohne ClientKey geht beim ersten Aufruf und danach je Periode an alle Adressen
der Liste (Multicast-Adresse und Port oder Unicast-Kandidaten); eine passende
Antwort (INFO mit `STATUS_OK` und gleicher Hauptversion) beendet das Senden, ein
`STATUS_ERR_INCOMPATIBLE` oder eine andere Hauptversion nicht. Der Test mit
Sockets sendet an Unicast-Kandidaten auf Loopback; der Empfang an einer echten
Multicast-Gruppe ist nicht geprüft.

### §11.2.4 Antwort des Agent mit seinen TransportLocators

**Spec:** §11.2.4, S. 100 — "This message shall invoke the get_info operation
(see 7.8.2.2) on the XRCE Agent, which shall respond and include its
TransportLocators."

**Repo:** `crates/xrce-agent/src/daemon/discovery.rs` (`run_discovery_responder`
Z. 43, `build_info_reply` Z. 107); `crates/xrce-agent/src/daemon/server.rs`
(`listener_locators` Z. 438)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`

**Status:** `done` — Commits `65cd1820a04`, `c304e509660`. INFO mit
`AGENT_ActivityInfo.address_seq` als `TransportLocatorSeq` der UDP- und
TCP-Listener (Medium für IPv4, Large für IPv6) und mit der
`AGENT_Representation`. Listener mit DTLS oder TLS und die serielle Leitung
meldet der Agent nicht; Annex A hat dafür kein `TransportLocatorFormat`.

### §11.2.4 Unicast-Discovery über eine Liste von Adressen

**Spec:** §11.2.4, S. 100 — "XRCE Agent discovery may be done using UDP/IP
unicast. The XRCE Clients shall be pre-configured with a list of candidate UDP
addresses and ports where XRCE Agents may be located. To discover Agents via
unicast the XRCE Client shall send the GET_INFO message (see 8.3.5.3)
periodically to the configured addresses and ports. [...] The XRCE Client shall
stop sending the periodic message once it receives a suitable response from an
Agent."

**Repo:** `crates/xrce-client/src/discovery.rs`
(`AgentDiscovery::new(candidates, period)`, `discover_over_udp`)

**Tests:** `crates/xrce-client/src/discovery.rs::tests::get_info_goes_to_the_candidates_every_period`,
`crates/xrce-client/src/discovery.rs::tests::the_search_over_udp_finds_the_agent_that_answers`

**Status:** `done` — Commits `5798b040952`. der Client bekommt eine Liste von
Kandidatenadressen, sendet `GET_INFO` periodisch an alle und hört bei einer
passenden Antwort auf.

### §11.3 TCP ohne Sicherheit und Länge, TLS oder VPN

**Spec:** §11.3, S. 100–101 — "The TCP transport meets all the functionality
listed in clause 11.1. except it does not provide security nor information on
the message size. For applications requiring security there is the 'Transport
Layer Security (TLS)' standard [TLS] [...]. The message size shall be added as a
prefix ahead of the XRCE message as defined in 11.3.3."

**Repo:** `crates/xrce-agent/src/daemon/transport/secure.rs`
(`run_tls_listener`); `crates/xrce/src/transport_tls.rs` (`RustlsTlsClient`,
`RustlsTlsServer`)

**Tests:** `crates/xrce-agent/tests/secure_listeners.rs::tls::a_client_that_trusts_the_agents_certificate_opens_a_session`,
`crates/xrce-agent/tests/secure_listeners.rs::tls::a_plain_text_create_client_on_the_tls_port_is_not_answered`,
`crates/xrce/tests/tls_e2e.rs::tls_handshake_and_message_roundtrip`

**Status:** `n/a (informative)` — Hinweis; die Länge steht im Item zu §11.3.3.
Der Agent hat unter dem Feature `tls` den Listener `kind: tls` (TLS mit dem
Zertifikat des Agent, `01470e273bc`); das 2-Byte-Längenpräfix steht im
verschlüsselten Strom. Die Tests laufen nur mit diesem Feature.

### §11.3.1 TCP v4 mit ADDRESS_FORMAT_MEDIUM

**Spec:** §11.3.1, S. 101 — "When XRCE is mapped to the TCP/IP version 4, the
TransportLocator union shall use the TransportLocatorFormat discriminator
ADDRESS_FORMAT_MEDIUM."

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators` Z. 438);
`crates/xrce/src/transport_locator.rs` (`TransportLocatorMedium`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`,
`crates/xrce/src/transport_locator.rs::tests::medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag`

**Status:** `done` — Commits `c304e509660`. ein TCP-Listener mit IPv4-Adresse wird als
`TransportLocator::Medium` (ADDRESS_FORMAT_MEDIUM) gemeldet; der Diskriminator
0x04 besteht nicht mehr.

### §11.3.1 TCP v6 mit ADDRESS_FORMAT_LARGE

**Spec:** §11.3.1, S. 101 — "When XRCE is mapped to the TCP/IP version 6, the
TransportLocator union shall use the TransportLocatorFormat discriminator
ADDRESS_FORMAT_LARGE. [...] The address field shall contain the IP v6 address
and the port field shall contain the TCP/IP v6 port number."

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators` Z. 438);
`crates/xrce/src/transport_locator.rs` (`TransportLocatorLarge`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`,
`crates/xrce/src/transport_locator.rs::tests::large_has_three_padding_octets_and_a_32_bit_port`

**Status:** `done` — Commits `c304e509660`. ein TCP-Listener mit IPv6-Adresse wird als
`TransportLocator::Large` gemeldet, mit der Portnummer in `locator_port`.

### §11.3.2 Agent als TCP-Server, Client als TCP-Client

**Spec:** §11.3.2, S. 101 — "The TCP Server accepts the connection from the
Client. This establishes a bi-directional communication channel. [...] The TCP
Server can send and receive messages to and from the Client. When
communicating over TCP the XRCE Agent shall behave as a TCP Server and the XRCE
Client as the TCP Client."

**Repo:** `crates/xrce/src/transport_tcp.rs` (`XrceTcpServer::accept` Z. 160,
`XrceTcpClient::connect` Z. 52); `crates/xrce-agent/src/daemon/transport.rs`
(`run_tcp_listener` Z. 550, `run_tcp_connection` Z. 485)

**Tests:** `crates/xrce/src/transport_tcp.rs::tests::tcp_loopback_create_client_roundtrip`,
`crates/xrce/src/transport_tcp.rs::tests::tcp_loopback_three_message_chain`,
`crates/xrce-agent/src/daemon/transport.rs::tests::a_tcp_client_gets_the_messages_of_the_agent_over_its_connection`

**Status:** done — die Rollen sind im Transport und am TCP-Listener des Agent
geprüft; der Agent sendet über die Verbindung auch ohne Anfrage (Items unten).

### §11.3.2 TCP-Listener des Agent

**Spec:** §11.3.2, S. 101 — wie oben.

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`run_tcp_listener` Z.
550, `run_tcp_connection` Z. 485)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::a_tcp_client_gets_the_messages_of_the_agent_over_its_connection`

**Status:** done — der Test startet den Listener, registriert einen Client über
TCP und erhält STATUS_AGENT. `XrceTcpServer` benutzt der Agent nicht.

### §11.3.2 Nachrichten des Agent ohne Anfrage über TCP

**Spec:** §11.3.2, S. 101 — "The TCP Server can send and receive messages to
and from the Client."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`run_ticker` Z. 410,
`deliver` Z. 367, `Route`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::a_tcp_client_gets_the_messages_of_the_agent_over_its_connection`

**Status:** `done` — Commits `2d3b8106888`. ein Thread (`run_ticker`, transport.rs
Z. 410) ruft `tick` für alle Listener auf; `deliver` (Z. 367) sendet jede
Nachricht über den Weg, auf dem sich ihr Client registriert hat: den Socket
eines UDP-Listeners oder die Warteschlange des Schreibers einer TCP-Verbindung
oder seriellen Leitung. Mit der Verbindung gehen ihre Wege. Der Test erhält
über die Verbindung eine Nachricht, die der Agent ohne Anfrage sendet.

### §11.3.2 Port des Agent vorkonfiguriert

**Spec:** §11.3.2, S. 101 — "The XRCE Agent shall be pre-configured with the
port number it shall listen to."

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`ListenerConfig::Tcp`)

**Tests:** `crates/xrce-agent/src/daemon/config.rs::tests::a_listener_can_be_written_as_one_string`

**Status:** done

### §11.3.2 Adresse und Port des Agent im Client vorkonfiguriert

**Spec:** §11.3.2, S. 101 — "The XRCE Client shall be pre-configured with the
TCP/IP address and port of the XRCE Agent."

**Repo:** `crates/xrce/src/transport_tcp.rs` (`XrceTcpClient::connect` Z. 52);
`crates/xrce-client/src/discovery.rs` (`TcpTransport`, `connect_over_tcp`)

**Tests:** `crates/xrce/src/transport_tcp.rs::tests::tcp_loopback_create_client_roundtrip`,
`crates/xrce-client/src/discovery.rs::tests::the_search_over_tcp_connects_to_the_agent_that_accepts`

**Status:** `done` — Commits `5798b040952`. `crates/xrce-client` hat mit `TcpTransport`
einen `ClientTransport` über TCP (Feature `std`); `connect_over_tcp` verbindet
sich mit den vorkonfigurierten Adressen und sendet CREATE_CLIENT.

### §11.3.3 Länge in zwei Byte, little-endian

**Spec:** §11.3.3, S. 101 — "The mapping of the XRCE Protocol to TCP/IP adds a
2-byte prefix as an envelope the XRCE message. The 2-byte prefix shall contain
the length of the XRCE message that follows encoded as little endian."

**Repo:** `crates/xrce/src/transport_tcp.rs` (`TCP_LENGTH_PREFIX_SIZE` Z. 37,
`XrceTcpClient::send_message` Z. 70, `recv_message` Z. 103)

**Tests:** `crates/xrce/src/transport_tcp.rs::tests::tcp_length_prefix_size_constant`,
`crates/xrce/src/transport_tcp.rs::tests::rfc793_byte_stream_is_delimited_by_the_length_prefix_not_by_writes`,
`crates/xrce/src/transport_tcp.rs::tests::rfc793_frame_split_across_two_writes_is_reassembled`

**Status:** done

### §11.3.3 Genau eine Nachricht, Ausrichtung unverändert

**Spec:** §11.3.3, S. 101 — "After the 2-byte envelope the TCP/IP payload shall
contain exactly one XRCE message. The alignment of the XRCE message shall not be
changed by the added 2-byte prefix. Stated differently the XRCE message shall
consider its first byte to be aligned to an 8-byte (XCDR maximum alignment)
boundary."

**Repo:** `crates/xrce/src/transport_tcp.rs` (`send_message`: `msg.encode()`
vor der Länge)

**Tests:** `crates/xrce/src/transport_tcp.rs::tests::tcp_loopback_three_message_chain`,
`crates/xrce/src/submessages/mod.rs::tests::message_two_submessages_padded_correctly`

**Status:** done — die Nachricht wird ohne Kenntnis der Länge kodiert. §8.2.1
nennt für den Anfang der Nachricht eine Ausrichtung auf 16 Byte, §11.3.3 eine
auf 8 Byte.

### §11.3.4 Multicast-Discovery auch für TCP

**Spec:** §11.3.4, S. 102 — "XRCE Agent discovery may be done using UDP/IP
multicast even if the communication will be over TCP. The XRCE Agents may be
pre-configured with the multicast address and port number they shall listen
to. By default they shall be the address 239.255.0.2 and the port 7400."

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (Responder unabhängig von der
Art der Listener); `crates/xrce-agent/src/daemon/discovery.rs`
(`bind_discovery_socket` Z. 146)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce/src/discovery.rs::tests::discovery_constants_match_spec`

**Status:** done — der Locator eines TCP-Listener in der Antwort ist im Item zu
§11.3.1 behandelt.

### §11.3.4 Periodisches GET_INFO des Client für TCP

**Spec:** §11.3.4, S. 102 — "To discover Agents via multicast the XRCE Client
shall send the GET_INFO message (see 8.3.5.3) periodically to the configured
multicast address and port. [...] The XRCE Client shall stop sending the
periodic message once it receives a suitable response from an Agent."

**Repo:** `crates/xrce-client/src/discovery.rs` (`AgentDiscovery`,
`discover_over_udp`)

**Tests:** `crates/xrce-client/src/discovery.rs::tests::get_info_goes_to_the_candidates_every_period`,
`crates/xrce-client/src/discovery.rs::tests::a_suitable_answer_ends_the_search`,
`crates/xrce-client/src/discovery.rs::tests::the_search_over_udp_finds_the_agent_that_answers`

**Status:** `done` — Commits `5798b040952`. wie im Item zu §11.2.4: der Client sucht
den Agent mit `GET_INFO` an die Multicast-Adresse und verbindet sich danach über
TCP (Item unten).

### §11.3.4 Discovery über TCP-Verbindungen zu einer Liste von Adressen

**Spec:** §11.3.4, S. 102 — "The XRCE Clients shall be pre-configured with a
list of candidate TCP addresses and ports where XRCE Agents may be located. To
discover Agents via unicast the XRCE Client shall periodically attempt to
establish TCP connections to the configured addresses and ports. One a
connection is established it shall send the CREATE_CLIENT message [...]. The
XRCE Client shall stop making periodic connection attempts once it receives a
suitable response from an Agent."

**Repo:** `crates/xrce-client/src/discovery.rs` (`connect_over_tcp`)

**Tests:** `crates/xrce-client/src/discovery.rs::tests::the_search_over_tcp_connects_to_the_agent_that_accepts`

**Status:** `done` — Commits `5798b040952`. der Client versucht je Periode, sich mit
jeder Adresse der Liste zu verbinden; auf eine Verbindung sendet er
CREATE_CLIENT und hört bei einem `STATUS_AGENT` mit `STATUS_OK` auf.

### §11.4 Weitere Transporte

**Spec:** §11.4, S. 102 — "The XRCE Protocol is well suited to be mapped to
other transports [...]. it is expected that future revisions of this
specification will provide additional transport mappings. An example Serial
Transport mapping can be found in Annex C.1"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Ausblick; die serielle Leitung steht in den
Items zu Annex C.

---

## Annex A IDL Types

### Annex A Typen, die §7.7 und §8.3.5 gleichen

**Spec:** Annex A, S. 103–120 — "module dds { module xrce { [...] }; };" mit
ClientKey, ObjectKind, ObjectId, XrceCookie, XrceVersion, XrceVendorId, Time_t,
SessionId, StreamId, RepresentationFormat und den Repräsentationen der Objekte,
CreationMode, RequestId, StatusValue, ResultStatus, InfoMask, den
ActivityInfo-Typen, ObjectInfo, BaseObjectRequest, BaseObjectReply,
DataFormat, DataDeliveryControl, ReadSpecification, SampleInfoFlags,
SampleInfoDelta, SampleData, Sample, SampleDelta, PackedSamples,
DataRepresentation, den Payloads der Submessages und SubmessageId.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung: wo Annex A dieselbe Form hat wie
§7.7 oder §8.3.5, steht die Anforderung im Item dort. Abweichende und
zusätzliche Formen stehen in den folgenden Items. Annex A schreibt
`SESSIONID_NONE_WITH_CLIENT_KEY`, §8.3.2.1 `SESSION_ID_NONE_WITH_CLIENT_KEY`;
Annex A schreibt `STREAMID_BUILTIN_BEST_EFFORTS`.

### Annex A Typen ohne @extensibility

**Spec:** Annex A, S. 104–112, S. 119 — "struct Time_t { long seconds; unsigned
long nanoseconds; };", "struct TransportLocatorMedium { [...] };", "struct
Property { string name; string value; };", "struct ResultStatus { StatusValue
status; octet implementation_status; };", "struct ACKNACK_Payload { [...] };"
ohne Annotation; DDS-XTypes 1.3 §7.3.1.2.1.8, S. 96: "In the event that the
representation of a given type does not indicate the type's extensibility kind,
the type shall be considered appendable."

**Repo:** `crates/xrce/src/encoding.rs` (`UnannotatedTypes`,
`with_unannotated_types`, `begin_unannotated`, `end_unannotated`);
`crates/xrce-agent/src/daemon/config.rs` (`unannotated_types`)

**Tests:** `crates/xrce/tests/unannotated_types.rs::the_default_is_appendable_with_a_dheader`,
`crates/xrce/tests/unannotated_types.rs::the_final_setting_writes_the_reference_implementations_octets`,
`crates/xrce/tests/unannotated_types.rs::both_forms_are_read_in_either_setting`,
`crates/xrce/tests/unannotated_types.rs::the_reference_implementations_octets_are_read_in_the_default_setting`,
`crates/xrce/src/transport_locator.rs::tests::by_default_the_union_and_its_case_have_a_dheader`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_extensibility_of_unannotated_types_can_be_set`

**Status:** `done` — Commits `f2ace917b5a`, `3874ad8d6a6`, `cd542d21c14`. Ohne
`@extensibility` gilt APPENDABLE (DDS-XTypes 1.3 §7.3.1.2.1.8): `Time_t`,
`ResultStatus`, `Property`, `TransportLocator` und `ACKNACK_Payload` tragen in
XCDR2 einen DHEADER; das ist die Vorgabe. Die Oktette des Mitschnitts von
eProsima (STATUS 6 Byte Payload, ACKNACK 5 Byte, aus
`tools/wire-capture/xrce-sessions/agent-to-client.hex` gelesen) sind die
benannte Einstellung FINAL: Agent-Schlüssel `unannotated_types: final`, Rust-API
`zerodds_xrce::encoding::UnannotatedTypes::Final`; jedes SDK hat die Option
„unannotated types“ (`b590f6539d0`). Die Leser nehmen beide Formen an.

### Annex A OBJK_OTHER = 0x0F

**Spec:** Annex A, S. 103 — "const ObjectKind OBJK_OTHER = 0x0F;"

**Repo:** `crates/xrce/src/object_kind.rs` (`OBJK_OTHER` Z. 43,
`ObjectKind::Other`, `ObjectKind::from_u8`)

**Tests:** `crates/xrce/src/object_kind.rs::tests::all_spec_kinds_roundtrip`,
`crates/xrce/src/object_kind.rs::tests::raw_const_values_match_spec`

**Status:** `done` — Commits `f63fa2d1218`. der Wert 0x0F heißt `OBJK_OTHER` wie in
Annex A; `OBJK_DOMAIN` besteht nicht mehr. Die Spec gibt der Domain keine
ObjectKind (Item zu §7.7.3.4).

### Annex A OBJECTID_AGENT {0xFF, 0xFD}

**Spec:** Annex A, S. 103 — "#define OBJECTID_AGENT {0xFF,0xFD}"

**Repo:** `crates/xrce/src/object_id.rs` (`OBJECTID_AGENT` Z. 44)

**Tests:** `crates/xrce/src/object_id.rs::tests::agent_singleton_has_kind_agent`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`

**Status:** done

### Annex A OBJECTID_SESSION {0xFF, 0xFF}

**Spec:** Annex A, S. 103 — "#define OBJECTID_SESSION {0xFF,0xFF}"

**Repo:** `crates/xrce/src/object_id.rs` (`OBJECTID_SESSION` Z. 42,
`OBJECTID_INVALID` Z. 40)

**Tests:** `crates/xrce/src/object_id.rs::tests::session_object_id_is_all_ones`,
`crates/xrce/src/object_id.rs::tests::reserved_object_ids_per_spec_7_6_and_annex_a`

**Status:** `done` — Commits `f63fa2d1218`. `OBJECTID_SESSION` ist {0xFF, 0xFF};
`OBJECTID_INVALID` ist {0x00, 0x00} (§7.6). Die Spec sagt nicht, wofür
`OBJECTID_SESSION` steht; der Code benutzt die Konstante nicht.

### Annex A XRCE_VENDOR_INVALID

**Spec:** Annex A, S. 104 — "typedef octet XrceVendorId[2]; #define
XRCE_VENDOR_INVALID1 0x00 #define XRCE_VENDOR_INVALID1 0x00"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Konstanten ohne Regel. Die Zeile steht
zweimal mit demselben Namen; gemeint sind vermutlich die beiden Oktette eines
ungültigen Vendor.

### Annex A TransportLocatorFormat

**Spec:** Annex A, S. 104 — "@bit_bound(8) enum TransportLocatorFormat {
ADDRESS_FORMAT_SMALL, ADDRESS_FORMAT_MEDIUM, ADDRESS_FORMAT_LARGE,
ADDRESS_FORMAT_STRING };" und S. 105 "union TransportLocator switch
(TransportLocatorFormat) { [...] case ADDRESS_FORMAT_LARGE:
TransportLocatorLarge medium_locator; [...] };"

**Repo:** `crates/xrce/src/transport_locator.rs` (`fmt_disc` Z. 50,
`TransportLocator::encode_into`, `decode_at`)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag`,
`crates/xrce/src/transport_locator.rs::tests::large_has_three_padding_octets_and_a_32_bit_port`,
`crates/xrce/src/transport_locator.rs::tests::small_is_two_address_octets_and_a_one_octet_port`,
`crates/xrce/src/transport_locator.rs::tests::string_length_is_aligned_to_four`,
`crates/xrce/src/transport_locator.rs::tests::unknown_discriminator_rejected`

**Status:** `done` — Commits `c304e509660`. die vier Werte 0 bis 3; der Diskriminator
ist ein Oktett vor dem Member, in der Vorgabe hinter dem DHEADER der Union (Item
zu Annex A „Typen ohne @extensibility“). Annex A nennt das Member für LARGE
`medium_locator`; auf der Leitung hat der Name keine Wirkung.

### Annex A Diskriminatoren 0x04 und 0x05

**Spec:** Annex A, S. 104 — wie oben.

**Repo:** `crates/xrce/src/transport_locator.rs` (`fmt_disc` hat nur 0x00 bis
0x03)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::unknown_discriminator_rejected`

**Status:** `done` — Commits `c304e509660`. `fmt_disc::TCP` und `fmt_disc::SERIAL`
sowie `TransportLocator::Tcp` und `Serial` sind entfernt; der Test weist die
Diskriminatoren 4 und 5 ab. TCP-Listener werden als Medium oder Large gemeldet,
die serielle Leitung gar nicht.

### Annex A TransportLocatorSmall

**Spec:** Annex A, S. 104–105 — "struct TransportLocatorSmall { octet
address[2]; octet locator_port; };"

**Repo:** `crates/xrce/src/transport_locator.rs` (`TransportLocatorSmall`:
`address: [u8; 2]`, `port: u8`)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::small_is_two_address_octets_and_a_one_octet_port`

**Status:** `done` — Commits `c304e509660`. zwei Oktette Adresse und ein Oktett Port,
ohne Füllbytes.

### Annex A TransportLocatorMedium

**Spec:** Annex A, S. 105 — "struct TransportLocatorMedium { octet address[4];
unsigned short locator_port; };"

**Repo:** `crates/xrce/src/transport_locator.rs` (`TransportLocatorMedium`,
`encode_member`)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag`,
`crates/xrce/src/transport_locator.rs::tests::a_udp_locator_carries_the_default_agent_port`

**Status:** `done` — Commits `c304e509660`. wie im Item zu §11.2.1: ein Füllbyte nach
der Adresse, der Port als Zahl in der Byte-Reihenfolge des E-Flags.

### Annex A TransportLocatorLarge

**Spec:** Annex A, S. 105 — "struct TransportLocatorLarge { octet address[16];
unsigned long locator_port; };"

**Repo:** `crates/xrce/src/transport_locator.rs` (`TransportLocatorLarge`,
`TransportLocator::encode_into`, `TransportLocator::decode_at`)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::large_has_three_padding_octets_and_a_32_bit_port`,
`crates/xrce/src/transport_locator.rs::tests::a_locator_sequence_is_a_count_and_aligned_locators`

**Status:** `done` — Commits `c304e509660`, `f2ace917b5a`. 16 Adressoktette, drei
Füllbytes, der Port als `unsigned long` in der Byte-Reihenfolge des E-Flags.
Union und Struktur des Falls tragen ohne `@extensibility` je einen DHEADER
(APPENDABLE nach DDS-XTypes 1.3 §7.3.1.2.1.8); der Agent-Schlüssel
`unannotated_types: final` schreibt sie ohne. Der Test der Füllbytes läuft in
der Einstellung `Final`, der Test der Folge in der Vorgabe.

### Annex A TransportLocatorString

**Spec:** Annex A, S. 105 — "struct TransportLocatorString { string value; };"

**Repo:** `crates/xrce/src/transport_locator.rs` (`TransportLocator::String`,
`TRANSPORT_LOCATOR_STRING_MAX`, `TransportLocator::encode_into`)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::string_length_is_aligned_to_four`,
`crates/xrce/src/transport_locator.rs::tests::a_long_string_locator_is_not_cut_at_256`,
`crates/xrce/src/transport_locator.rs::tests::a_string_without_its_nul_is_refused`

**Status:** `done` — Commits `c304e509660`. die Länge ist ein `unsigned long`, auf 4
ausgerichtet und in der Byte-Reihenfolge des E-Flags; danach die Zeichen mit
NUL. Die Obergrenze beträgt 65 535 Byte (die Größe einer Submessage), nicht
mehr 256.

### Annex A Property und PropertySeq

**Spec:** Annex A, S. 105 — "struct Property { string name; string value; };
typedef sequence<Property> PropertySeq;"

**Repo:** `crates/xrce/src/handshake.rs` (`Property`, `encode_properties`,
`decode_properties`, `decode_property`, `MAX_PROPERTIES`)

**Tests:** `crates/xrce/src/handshake.rs::tests::a_property_has_a_dheader_by_default`,
`crates/xrce/src/handshake.rs::tests::a_big_endian_client_representation_with_properties_reads_back`,
`crates/xrce/src/handshake.rs::tests::a_big_endian_agent_representation_with_properties_reads_back`,
`crates/xrce/src/handshake.rs::tests::padding_counts_from_the_start_of_the_payload`,
`crates/xrce/src/handshake.rs::tests::max_properties_cap_enforced_on_decode`,
`crates/xrce/src/handshake.rs::tests::a_malformed_property_seq_is_an_error_not_absent_properties`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_unreadable_property_seq_is_refused_with_invalid_data`

**Status:** `done` — Commits `65cd1820a04`, `f2ace917b5a`, `b43d5e6b848`. die Zahl der
Elemente und die Längen der Zeichenketten sind `unsigned long`, ab dem Anfang
der Payload auf 4 ausgerichtet; die Zahl liegt in `CLIENT_Representation` auf
Versatz 16. Jede `Property` trägt ohne `@extensibility` einen DHEADER (Vorgabe),
`unannotated_types: final` schreibt sie ohne; beide Formen werden gelesen. Die
Byte-Reihenfolge folgt dem E-Flag. Eine Folge, die sich nicht lesen lässt, ist
ein Fehler und gilt nicht als nicht vorhanden; der Agent antwortet darauf mit
`STATUS_ERR_INVALID_DATA`. Obergrenze beim Lesen: 64 Elemente.

### Annex A CLIENT_Representation

**Spec:** Annex A, S. 105–106 — "@extensibility(FINAL) struct
CLIENT_Representation { XrceCookie xrce_cookie; XrceVersion xrce_version;
XrceVendorId xrce_vendor_id; ClientKey client_key; SessionId session_id;
@optional PropertySeq properties; };"

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation`, `encode_with`,
`decode_with`, `encode_into`, `decode_at`)

**Tests:** `crates/xrce/src/handshake.rs::tests::client_representation_decodes_the_captured_create_client_prefix`,
`crates/xrce/src/handshake.rs::tests::client_representation_roundtrip_with_properties`,
`crates/xrce/src/handshake.rs::tests::by_default_the_representations_have_no_timestamp`,
`crates/xrce/src/handshake.rs::tests::the_prose_form_has_a_time_t_after_the_vendor_id`

**Status:** `done` — Commits `65cd1820a04`, `b43d5e6b848`, `711f93fd460`. der Code folgt
der Form von Annex A; `@optional` ist ein Oktett 0 oder 1, `properties` liegt
wie im Item oben. Der Text in §7.8.2.1 nennt zusätzlich ein `Time_t
client_timestamp` hinter `xrce_vendor_id`; IDL und Bytes von Annex B.1 haben es
nicht. Die Form von Annex A ist die Vorgabe; die Form des Textes ist die
benannte Einstellung `RepresentationTimestamps::Present` (Agent-Schlüssel
`representation_timestamps: present`, `XrceClient::set_representation_timestamps`,
Option `representation timestamps` in jedem SDK). Die zwei Oktette nach
`properties` (`requested_mtu`) sind eine Erweiterung von eProsima, auf 2
ausgerichtet.

### Annex A AGENT_Representation

**Spec:** Annex A, S. 106 — "@extensibility(FINAL) struct AGENT_Representation
{ XrceCookie xrce_cookie; XrceVersion xrce_version; XrceVendorId
xrce_vendor_id; @optional PropertySeq properties; };"

**Repo:** `crates/xrce/src/handshake.rs` (`AgentRepresentation`, `encode_with`,
`decode_with`, `encode_into`, `decode_at`)

**Tests:** `crates/xrce/src/handshake.rs::tests::agent_representation_decodes_the_captured_status_agent_body`,
`crates/xrce/src/handshake.rs::tests::agent_representation_roundtrip_no_properties`,
`crates/xrce/src/handshake.rs::tests::the_agent_representation_follows_the_setting_the_same_way`

**Status:** `done` — Commits `65cd1820a04`, `b43d5e6b848`, `711f93fd460`. die Form mit
`agent_timestamp` (§7.8.2.1) ist dieselbe benannte Einstellung wie bei
`CLIENT_Representation` (`representation_timestamps`, Vorgabe `absent` nach
Annex A).

### Annex A OBJK_DomainParticipant_Binary

**Spec:** Annex A, S. 109 — "@extensibility(APPENDABLE) struct
OBJK_DomainParticipant_Binary { @optional string<128> domain_reference;
@optional string<128> qos_profile_reference; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`participant`,
`ParticipantBinary`); `crates/xrce-agent/src/daemon/entity_repr.rs`
(`participant`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::participant_binary_in_both_forms`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_participant_is_read`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_domain_reference_is_resolved_against_the_domain_libraries`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_participant_qos_profile_reference_is_applied`

**Status:** `done` — Commits `c67ce23c724`, `b4cd825c268`. der Blob wird als XCDR2 gelesen:
Annex A (DHEADER, zwei `@optional`-Referenzen), dann die Form von §7.7.3.6.3
(`long domain_id` zuerst), dann die zwei Referenzen ohne DHEADER, die der
Client von eProsima sendet. `domain_reference` wird gegen die Domains der
`<domain_library>`-Dateien aufgelöst (`STATUS_ERR_UNKNOWN_REFERENCE` für einen
Namen in keiner Bibliothek; die `domain_id` der Domain muss mit der der
Darstellung übereinstimmen). `qos_profile_reference` wendet die
Teilnehmer-QoS des Profils an (USER_DATA, ENTITY_FACTORY; jede andere Richtlinie
ergibt `STATUS_ERR_INCOMPATIBLE`).

### Annex A OBJK_Topic_Binary als APPENDABLE

**Spec:** Annex A, S. 109 — "@extensibility(APPENDABLE) struct
OBJK_Topic_Binary { string<256> topic_name; @optional string<256>
type_reference; @optional DDS:XTypes::TypeIdentifier type_identifier; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`topic`,
`TopicBinary`, `Form::AnnexA`, `Form::Clauses`); `crates/xrce-agent/src/daemon/entity_repr.rs`
(`topic`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::annex_a_topic_has_a_dheader`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::the_36_octet_example_of_the_clause_is_a_topic`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::a_topic_with_a_type_identifier_reads_it_and_one_without_a_type_is_refused`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::string_bounds_are_checked`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_topic_names_its_topic_and_type`

**Status:** `done` — Commits `c67ce23c724`. gelesen wird die Form von Annex A (DHEADER
vor den Elementen) und die FINAL-Form von §7.7.3.7.3 (ohne DHEADER, die 36
Oktette des Beispiels in §7.7.3.1.3); §7.7.3.7.3 gibt keine Erweiterbarkeit an.
`string<256>` wird geprüft. Ein `type_identifier` hat Vorrang vor
`type_reference`; fehlen beide, antwortet der Agent mit
`STATUS_ERR_INVALID_DATA`.

### Annex A OBJK_Publisher_Binary als APPENDABLE

**Spec:** Annex A, S. 109 — "@extensibility(FINAL) struct
OBJK_Publisher_Binary_Qos { @optional sequence<string> partitions; @optional
sequence<octet> group_data; }; @extensibility(APPENDABLE) struct
OBJK_Publisher_Binary { @optional string publisher_name; @optional
OBJK_Publisher_Binary_Qos qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`container`,
`ContainerBinary`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`container`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::publisher_binary_of_annex_a`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_publisher_gives_its_partitions`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_publisher_has_default_qos`

**Status:** `done` — Commits `c67ce23c724`. die Form von Annex A (DHEADER, `publisher_name`,
`qos` mit `partitions` und `group_data`) wird gelesen; scheitert sie, liest
`container` die Form ohne DHEADER von §7.7.3.8.2, die der Client von eProsima
sendet. Der Name der QoS-Struktur lautet in §7.7.3.8.2
`OBJK_PUBLISHER_QosBinary`; auf der Leitung hat das keine Wirkung.

### Annex A OBJK_Subscriber_Binary als APPENDABLE

**Spec:** Annex A, S. 109 — "@extensibility(APPENDABLE) struct
OBJK_Subscriber_Binary { @optional string subscriber_name; @optional
OBJK_Subscriber_Binary_Qos qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`container`,
`ContainerBinary`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`container`
mit `element` = `subscriber`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::publisher_binary_of_annex_a`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_publisher_gives_its_partitions`

**Status:** `done` — Commits `c67ce23c724`. dieselbe Funktion und dasselbe Layout wie beim
Publisher; die Tests lesen den Blob des Publishers, ein Test mit einem eigenen
Blob des Subscribers fehlt.

### Annex A EndpointQosFlags mit is_history_keep_all

**Spec:** Annex A, S. 110 — "@bit_bound(16) bitmask EndpointQosFlags {
@position(0) is_reliable, @position(1) is_history_keep_all, [...] };"

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_endpoint_binary`,
`spec_flags`, `flags`); `crates/xrce-agent/src/daemon/binary_forms.rs`
(`EndpointBinary::bit1_is_keep_all`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::reader_binary_of_the_clause`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_writer_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_clause_binary_reader_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_writer_is_reliable_keep_last_transient_local`

**Status:** `done` — Commits `c67ce23c724`. Annex A (Bit 1 = `is_history_keep_all`) und
§7.7.3.10.2 (Bit 1 = `is_history_keep_last`) widersprechen sich. Das Bit wird
nach der Form gelesen, in der der Blob lesbar ist: Form von Annex A zuerst, dann
die Form der Klausel; die klassische CDR-Struktur von eProsima (`qos_flags`
`0x000b` im Mitschnitt) liest Bit 1 als KEEP_LAST. Bits außerhalb von
`EndpointQosFlags` und mehrere Durability-Bits ergeben
`STATUS_ERR_INVALID_DATA`.

### Annex A OBJK_DataWriter_Binary_Qos
**Spec:** Annex A, S. 110 — "@extensibility(FINAL) struct
OBJK_DataWriter_Binary_Qos : OBJK_Endpoint_Binary_Qos { @optional unsigned long
ownership_strength; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint_qos`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_ownership_strength`, `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`

**Status:** done — die XCDR2-Formen lesen `ownership_strength` als `unsigned long`; die klassische CDR-Struktur von eProsima liest 64 Bit. Der Test liest ein vorhandenes `ownership_strength` in einem XCDR2-Blob in beiden Byte-Reihenfolgen (2d6689d3ed6).
### Annex A OBJK_DataReader_Binary_Qos

**Spec:** Annex A, S. 110 — "@extensibility(FINAL) struct
OBJK_DataReader_Binary_Qos : OBJK_Endpoint_Binary_Qos { @optional unsigned long
timebasedfilter_msec; @optional string contentbased_filter; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint_qos`,
`EndpointQosBinary::timebasedfilter_msec`, `contentbased_filter`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_endpoint_binary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::reader_binary_of_the_clause`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_clause_binary_reader_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`

**Status:** `done` — Commits `c67ce23c724`, `33df0df0945`. `timebasedfilter_msec` wird als
`unsigned long` gelesen (Wert 50 im Test), `contentbased_filter` als `string`
(„x>100“ im Beispiel B.2.7). Die klassische CDR-Struktur von eProsima bleibt
als Rückfall.

### Annex A OBJK_DataWriter_Binary

**Spec:** Annex A, S. 110–111 — "@extensibility(APPENDABLE) struct
OBJK_DataWriter_Binary { string topic_name; @optonal OBJK_DataWriter_Binary_Qos
qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint`,
`EndpointBinary`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_writer_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_writer_is_reliable_keep_last_transient_local`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_blob_of_no_form_is_refused_naming_each_reading`

**Status:** `done` — Commits `c67ce23c724`, `33df0df0945`. gelesen werden drei Formen in
dieser Reihenfolge: Annex A (DHEADER, `topic_name`, `@optional qos`), die Form
von §7.7.3.10.2 (ohne DHEADER, `endpoint_qos` als Pflichtfeld) und die
klassische CDR-Struktur von eProsima (`ObjectId` des Topics statt `topic_name`).
Ein Blob in keiner Form wird abgewiesen; die Meldung nennt das Ergebnis jeder
Form.

### Annex A OBJK_DataReader_Binary

**Spec:** Annex A, S. 110 — "@extensibility(APPENDABLE) struct
OBJK_DataReader_Binary { string topic_name; @optional
OBJK_DataReader_Binary_Qos qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint` mit
`writer = false`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::reader_binary_of_the_clause`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_clause_binary_reader_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_reader_is_reliable_keep_last_transient_local`

**Status:** `done` — Commits `c67ce23c724`, `33df0df0945`. dieselben drei Formen wie beim
DataWriter; Annex A mit Filter ist durch B.2.7 gelesen, die Form der Klausel
durch den Test mit Tiefe 5 und `timebasedfilter_msec` 50.

### Annex A ObjectVariant mit OBJK_AGENT und OBJK_CLIENT

**Spec:** Annex A, S. 111 — "@extensibility(FINAL) union ObjectVariant
switch(ObjectKind) { // case OBJK_INVALID : indicates default or selected by
Agent. No data. case OBJK_AGENT : AGENT_Representation client; case
OBJK_CLIENT : CLIENT_Representation client; [...] };"

**Repo:** `crates/xrce/src/object_info.rs` (`ObjectConfig::Agent`,
`ObjectConfig::Client`, `ObjectInfo::config`); `crates/xrce-agent/src/daemon/session.rs`
(`agent_object_info`, `client_object_info`)

**Tests:** `crates/xrce/src/object_info.rs::tests::the_config_of_the_agent_is_its_agent_representation`,
`crates/xrce/src/submessages/get_info.rs::tests::client_info_follows_the_info_mask`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_checks_the_version_in_client_info`

**Status:** `done` — Commits `65cd1820a04`. die Fälle `OBJK_AGENT` und `OBJK_CLIENT`
stehen in `ObjectConfig`, dem `config` von `ObjectInfo` (INFO) und dem
`client_info` von GET_INFO; Kind-Oktett und Darstellung folgen aufeinander.
Die Aufzählung `ObjectVariant` für CREATE hat diese zwei Fälle nicht: eine
Darstellung mit Art Agent oder Client wird dort nicht gelesen. Beide Member
heißen in der IDL `client`; auf der Leitung hat das keine Wirkung.

### Annex A ActivityInfoVariant mit case OBJECTID_AGENT

**Spec:** Annex A, S. 113 — "@extensibility(FINAL) union ActivityInfoVariant
switch (ObjectKind) { case OBJECTID_AGENT : AGENT_ActivityInfo agent; case
OBJK_DATAWRITER : [...] case OBJK_DATAREADER : [...] };"

**Repo:** `crates/xrce/src/object_info.rs` (`ActivityInfoVariant`,
`ActivityInfoVariant::discriminator`: Agent 0x0D, DataWriter 0x05, DataReader
0x06)

**Tests:** `crates/xrce/src/object_info.rs::tests::agent_activity_info_has_a_dheader_and_a_locator_sequence`,
`crates/xrce/src/object_info.rs::tests::data_reader_activity_info_is_a_dheader_and_a_short`,
`crates/xrce/src/object_info.rs::tests::data_writer_activity_info_is_a_dheader_a_long_long_and_a_short`

**Status:** `done` — Commits `65cd1820a04`. der Diskriminator ist ein `ObjectKind`
(ein Oktett); `OBJECTID_AGENT` ist eine `ObjectId` aus zwei Oktetten und kein
Wert von `ObjectKind`, der Text von Annex A lässt sich nicht wörtlich nehmen.
Der Fall des Agent hat den Diskriminator `OBJK_AGENT` (0x0D). Die Member
tragen den DHEADER (`AGENT_ActivityInfo`: `availability`, `address_seq` als
`TransportLocatorSeq`).

### Annex A FORMAT_MASK = 0x0E

**Spec:** Annex A, S. 114 — "const DataFormat FORMAT_MASK = 0x0E; // 0b0000
1110"

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`DataFormat::FORMAT_MASK`, `DataFormat::from_flag_bits`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_rejects_reserved_format`,
`crates/xrce/src/submessages/write_data.rs::tests::data_format_raw_roundtrip`,
`crates/xrce/src/submessages/read_data.rs::tests::the_data_format_octet_has_the_values_of_annex_a`

**Status:** `done`

### Annex A SampleInfo mit SampleInfoDetail
**Spec:** Annex A, S. 114–115 — "typedef octet SampleInfoFormat; const
SampleInfoFormat FORMAT_EMPTY = 0x00; [...] FORMAT_SEQNUM = 0x01; [...]
FORMAT_TIMESTAMP = 0x02; [...] FORMAT_SEQN_TIMS = 0x03; [...] struct
SeqNumberAndTimestamp { unsigned long sequence_number; unsigned long
session_time_offset; }; union SampleInfoDetail switch(SampleInfoFormat) {
case FORMAT_EMPTY: case FORMAT_SEQNUM: unsigned long sequence_number; case
FORMAT_TIMESTAMP: unsigned long session_time_offset; case FORMAT_TIMESTAMP:
SeqNumberAndTimestamp seqnum_n_timestamp; }; struct SampleInfo {
SampleInfoFlags state; SampleInfoDetail detail; };"

**Repo:** `crates/xrce/src/sample_info.rs` (`SampleInfo`, `info_format`); `crates/xrce/src/encoding.rs` (`SampleInfoForm`)

**Tests:** `crates/xrce/src/sample_info.rs::tests::the_union_of_annex_a_is_read_by_its_discriminator`, `crates/xrce/src/sample_info.rs::tests::the_flat_form_ignores_the_padding_octet`

**Status:** done — `SampleInfo` ist der `state` mit der `SampleInfoDetail`-Union von Annex A: `FORMAT_EMPTY`, `FORMAT_SEQNUM`, `FORMAT_TIMESTAMP`, `FORMAT_SEQN_TIMS`. Die Union gibt den Fall `FORMAT_TIMESTAMP` zweimal an und lässt `FORMAT_EMPTY` in `FORMAT_SEQNUM` durchfallen; gelesen wird die gemeinte Form (nichts, Sequenznummer, Versatz, beide). `FORMAT_SEQN_TIMS` sind die 12 Oktette der flachen Struktur von §7.7.1 und der Bilder von Annex B.5.3 bis B.5.5, der Diskriminator im ersten Füll-Oktett. Vorgabe `SampleInfoForm::AnnexA`; die flache Form (`sample_info: flat`) ist für einen Client, der Nullen als Füllung sendet (die Union liest sie als `FORMAT_EMPTY`). 8be8c6101ca.
### Annex A ACKNACK_Payload mit unsigned short

**Spec:** Annex A, S. 119 — "struct ACKNACK_Payload { unsigned short
first_unacked_seq_num; octet nack_bitmap[2]; octet stream_id; };"

**Repo:** `crates/xrce/src/submessages/acknack.rs` (`AckNackPayload`,
`encode_body`, `decode_body`)

**Tests:** `crates/xrce/src/submessages/acknack.rs::tests::acknack_le_roundtrip`,
`crates/xrce/src/submessages/acknack.rs::tests::acknack_be_roundtrip`,
`crates/xrce/tests/unannotated_types.rs::the_default_is_appendable_with_a_dheader`,
`crates/xrce/tests/unannotated_types.rs::the_final_setting_writes_the_reference_implementations_octets`,
`crates/xrce/src/reliable.rs::tests::acknack_retransmit_of_lost_sample_across_wrap`

**Status:** `done` — Commits `f2ace917b5a`. §8.3.5.11.2 schreibt `short`; die Bytes des
Feldes sind gleich. Die Struktur trägt in Annex A kein `@extensibility`: sie
ist APPENDABLE und hat in der Vorgabe einen DHEADER (9 Oktette); die 5 Oktette
des Referenzagenten schreibt `unannotated_types: final`, beide Formen werden
gelesen. Der Code hält das Feld als `i16` und rechnet mit `SerialNumber16`.

### Annex A HEARTBEAT_Payload mit unsigned short

**Spec:** Annex A, S. 119 — "@extensibility(FINAL) struct HEARTBEAT_Payload {
unsigned short first_unacked_seq_num; unsigned short last_unacked_seq_num; octet
stream_id; };"

**Repo:** `crates/xrce/src/submessages/heartbeat.rs` (`HeartbeatPayload`,
`encode_body`, `decode_body`)

**Tests:** `crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_le_roundtrip`,
`crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_be_roundtrip`,
`crates/xrce/src/reliable.rs::tests::heartbeat_window_across_wrap_reports_rfc1982_order`

**Status:** `done` — die Struktur ist ausdrücklich `@extensibility(FINAL)`, der
Rumpf hat 5 Oktette ohne DHEADER. §8.3.5.12.2 schreibt `short first_unacked_seq_nr`
und `short last_unacked_seq_nr`; die Bytes sind gleich.

---

## Annex B Example Messages (Non-Normative)

### Annex B.1 CREATE_CLIENT

**Spec:** Annex B.1, S. 121–122 — CREATE_CLIENT von vendor_id {0x0F, 0x0F},
client_key {0x22, 0x33, 0x44, 0x55}, session_id 0xDD; Header {0x80, 0x00,
0x00, 0x00}, Bytes 8–21 `CLIENT_Representation`, Byte 21 "properties? =
FALSE".

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation::encode`);
`crates/xrce/src/submessages/create_client.rs` (`CreateClientPayload`)

**Tests:** `crates/xrce/src/submessages/create_client.rs::tests::annex_b_1_create_client_example_bytes`,
`crates/xrce/src/handshake.rs::tests::the_prose_form_has_a_time_t_after_the_vendor_id`

**Status:** `done` — Commits `d4d590690f6`, `711f93fd460`. der Test bildet Header und die
Bytes 8–21 nach. Der Text nennt `request_id` und `client_timestamp`, die das
Layout nicht enthält; mit der Einstellung `representation_timestamps: present`
schreibt der Code das `Time_t` des Textes, der zweite Test prüft diese Form mit
den Werten des Beispiels. Die Flags 0x07 („reuse, replace“) hat CREATE_CLIENT
nicht; `submessageLength` lautet „26 = 0x001B“, der Rumpf hat 14 Byte.

### Annex B.2.1 DomainParticipant mit REPRESENTATION_BY_REFERENCE

**Spec:** Annex B.2.1, S. 123–125 — CREATE mit `OBJK_PARTICIPANT`,
`REPRESENTATION_BY_REFERENCE`, Referenz „MyLibrary::MyParticipant“ (25 Byte mit
NUL), `domain_id` 0.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::encode_for_kind`,
`ObjectVariant::decode_for_kind_at`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::participant_representation_matches_the_captured_create_bytes`

**Status:** `n/a (informative)` — Beispiel. Der Test prüft die Form aus dem
Mitschnitt von eProsima: Länge auf Byte 16–19, Zeichen ab Byte 20. Das Beispiel
setzt die Länge auf Byte 16–19 und die Zeichen auf Byte 24–48; Byte 20–23 sind
nicht erklärt, die Bytes des Beispiels liest der Code nicht. `submessageLength`
lautet 26, die Payload hat 44 Byte. Der Text verweist auf Table 20 statt
Table 18.

### Annex B.2.2 DomainParticipant mit REPRESENTATION_IN_BINARY

**Spec:** Annex B.2.2, S. 125–127 — "Bytes 16-19 DHEADER of
OBJK_DomainParticipant_Binary (because extensibility is APPENDABLE) Encodes the
endianness and length [...] 0x80000002 = {0x02, 0x00, 0x00, 0x80}", danach zwei
Oktette für die `@optional` und `domain_id`.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`);
`crates/xrce-agent/src/daemon/binary_forms.rs` (`participant`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::annex_b_2_2_participant_variant`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_participant_is_read`

**Status:** `done` — Commits `33df0df0945`, `c67ce23c724`. der DHEADER `0x80000002` steht
an der Stelle, an der eProsima die Länge von `binary_representation` sendet;
der Code liest ihn dort als DHEADER (Bit 31 ist kein Teil der Länge), die zwei
`@optional`-Oktette und die `domain_id` danach.

### Annex B.2.3 DataWriter mit REPRESENTATION_IN_BINARY

**Spec:** Annex B.2.3, S. 127–129 — DHEADER 0x8000000C, `topic_name` „Square“
(7 Byte mit NUL), `qos?` = 0, `publisher_id` {0xBB, 0xB3}.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`);
`crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::annex_b_2_3_writer_variant_reads_as_the_blob_and_its_publisher`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`

**Status:** `done` — Commits `c67ce23c724`. der Blob mit `topic_name` und `qos?` = 0 und
die `publisher_id` danach werden gelesen. Der Text nennt den Publisher
`subscriber_id`.

### Annex B.2.4 DataWriter mit QoS

**Spec:** Annex B.2.4, S. 129–132 — "RELIABILITY policy set to BEST_EFFORT,
HISTORY policy set to KEEP_ALL and DEADLINE policy set to a period of 2
minutes"; "qos_flags = 0x0003 Indicates the flags for is_reliable and
is_history_keep_all are both set."; Byte 34 `history_depth?`, Byte 35
Füllung, Byte 36–39 `deadline` = 120000.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::annex_b_2_4_writer_with_qos_in_both_readings_of_the_padding`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`

**Status:** `done` — Commits `33df0df0945`. gelesen als XCDR2 der gedruckten IDL. Byte 35,
das die Tabelle Füllung nennt, ist das Oktett für `@optional deadline_msec`
(1); der Wert beginnt auf Byte 36. Das Füllbyte 43 vor `publisher_id` wird
sowohl im DHEADER gezählt (24) als auch außerhalb (23) gelesen. Der Text nennt
BEST_EFFORT, `qos_flags` 0x0003 und die Tabelle setzen `is_reliable`; gelesen
werden die Bytes.

### Annex B.2.5 DataWriter mit REPRESENTATION_AS_XML_STRING

**Spec:** Annex B.2.5, S. 132–134 — `<data_writer name="MyWriter"
topic_ref="Square"><data_writer_qos base_name="MyQosLib::MyProfile">
<deadline>[...]</deadline></data_writer_qos></data_writer>`, Länge 181,
`publisher_id` auf Byte 201–202.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint_from_xml`:
`topic_ref` als Name, `<data_writer_qos>` und `<datawriter_qos>` mit
`base_name`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_data_writer_states_its_qos_over_a_profile`

**Status:** `n/a (informative)` — Beispiel. Der Code löst `topic_ref="Square"`
als Name auf und liest `<data_writer_qos base_name="…">` über einem Profil des
Agenten (`3aea52dd70a`); der Test hat die Form des Beispiels, nicht seine Bytes.
Im Bild ist der Diskriminator 0x01, in Table 22 0x02; der Text nennt 180 und 181
Zeichen und „MyQosLib:MyProfile“ mit einem Doppelpunkt.

### Annex B.2.6 DataReader mit REPRESENTATION_IN_BINARY

**Spec:** Annex B.2.6, S. 134–136 — DataReader „Square“ mit DHEADER,
`topic_name`, `qos?` = 0, `subscriber_id` {0xCC, 0xC4}.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::annex_b_2_6_reader_with_default_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`

**Status:** `done` — Commits `33df0df0945`. gelesen wird der DHEADER 12 (`0x8000000C`
wie bei B.2.3). Der Text druckt den DHEADER als {0xB5, 0x00, 0x00, 0x00}; das
passt nicht zu den 26 Oktetten der Nachricht, der Code weist den gedruckten Wert
ab. Table 23 setzt den Diskriminator auf 0x05 (`OBJK_DATAWRITER`) für
`OBJK_DATAREADER` (0x06) und führt Byte 24–30 zweimal.

### Annex B.2.7 DataReader mit QoS und Filter

**Spec:** Annex B.2.7, S. 136–139 — "HISTORY policy set to KE EP_ALL and
DEADLINE policy set to a period of 5 minutes", `qos_flags` 0x0002, `deadline`
180000, `contentbased_filter` „x>100“.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::annex_b_2_7_reader_with_qos_and_filter`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`

**Status:** `done` — Commits `33df0df0945`. gelesen als XCDR2 der gedruckten IDL; Byte 35
ist das Oktett für `deadline_msec`, vor der `subscriber_id` steht kein Füllbyte.
180000 ms sind 3 Minuten (gelesen), der Text nennt 5; Bild und Table 24 nennen
am Ende `publisher_id` statt `subscriber_id`.

### Annex B.3.1 WRITE_DATA mit einem Sample

**Spec:** Annex B.3.1, S. 139–141 — flags 0x01, `submessageLength` 6, Byte 8–13
{0xAA, 0x01, 0x44, 0x05, 0x19, 0x00}.

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`WriteDataPayload`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_matches_the_annex_b_example`

**Status:** `done` — der Test bildet flags, Länge und Rumpf Byte für Byte nach.
Table 25 heißt „READ_DATA (single sample)“.

### Annex B.3.2 WRITE_DATA mit einer Folge ohne SampleInfo
**Spec:** Annex B.3.2, S. 141–143 — flags 0x09, `data_seq.length` = 4, vier
`short` {0x14, 0x00}, {0x11, 0x00}, {0x1A, 0x00}, {0x24, 0x00}.

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_3_2_write_data_with_a_sequence_without_sample_info`

**Status:** done — das Beispiel ist mit seinen Bytes getestet: Kennung 0x07, flags 0x09, Länge 16 (das Bild nennt die Länge 6 von B.3.1), vier Werte 20, 17, 26, 36 (8be8c6101ca).
### Annex B.3.3 WRITE_DATA mit Zeitstempel

**Spec:** Annex B.3.3, S. 143–146 — "Byte 12 info.state = 0x00 [...] Byte 13
info.detail.discriminator = FORMAT_TIMESTAMP Bytes 14-15 padding Bytes 16-19
info. Detail.session_time_offset Bytes 20-21 serialized_data".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel in der Form von Annex A mit
`SampleInfoDetail`. Der Code liest `SampleInfo` nach §7.7.1 mit 12 Byte und
liest dieses Beispiel nicht (offenes Item „Annex A SampleInfo mit
SampleInfoDetail“). `submessageLength` lautet „13 = 0x000D“ mit {0x06, 0x00},
der Rumpf hat 14 Byte.

### Annex B.3.4 WRITE_DATA mit einem disposed Sample

**Spec:** Annex B.3.4, S. 146–148 — "Byte 12 info.state = 0x02 [...] Byte 13
Info.detail.discriminator = FORMAT_NONE [...] Bytes 14-19 Serialized_data".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel in der Form von Annex A; der Code
liest es nicht (wie B.3.3). Bild und Table nennen FORMAT_EMPTY und
FORMAT_NONE; `submessageLength` lautet 6, der Rumpf hat 12 Byte.

### Annex B.4.1 READ_DATA für ein Sample
**Spec:** Annex B.4.1, S. 148–150 — "submessageId = READ_DATA = 0x07", flags
0x01, "submessageLength = 7", Byte 12 `preferred_stream_id` 0x00, Byte 13
`data_format` 0x00, Byte 14 und 15 die Oktette für `@optional` = 0.

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_4_1_read_data_for_a_sample`

**Status:** done — das Beispiel ist mit allen acht Oktetten der Payload getestet; Kennung 0x08 (das Bild nennt 0x07), Länge 8 (das Bild nennt 7) (8be8c6101ca).
### Annex B.4.2 READ_DATA mit Filter und DataDeliveryControl

**Spec:** Annex B.4.2, S. 150–153 — `data_format` 0x08, `content_filter`
„x>100“ mit Länge 6, DHEADER 0x80000008, `max_samples` 100,
`max_elapsed_time` 30000, `max_rate` 1024 „{0x00, 0x40}“, `min_pace_period`
1000.

**Repo:** `crates/xrce/src/submessages/read_data.rs` (`ReadDataPayload`,
`ReadSpecification`, `DataDeliveryControl`)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`,
`crates/xrce/src/submessages/read_data.rs::tests::the_content_filter_is_a_string_with_a_nul_and_an_aligned_length`

**Status:** `done` — Commits `1663f8f6240`. `data_format` 0x08 (FORMAT_DATA_SEQ),
`content_filter_expression` als XCDR2-String, `DataDeliveryControl` mit DHEADER;
der Test liest die Bytes des Bildes (Bit 31 des DHEADER zählt nicht zur Länge)
und schreibt den einfachen DHEADER 8. Table 30 legt DHEADER und `max_samples`
beide auf Byte 28–29; für 1024 druckt der Text {0x00, 0x40}, little-endian ist
es {0x00, 0x04}; Byte 14 und Byte 26 heißen „set to FALSE indicating there it
is present“. Die Form ohne
DHEADER schreibt `XrceClient::set_delivery_control_form(DeliveryControlForm::Bare)`.

### Annex B.5.1 DATA mit einem Sample
**Spec:** Annex B.5.1, S. 154–155 — flags 0x00 (big-endian),
`submessageLength` 8, `FooType.count` = 19 als {0x00, 0x00, 0x00, 0x13}.

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_5_1_data_with_one_sample`

**Status:** done — das Beispiel (Big-Endian, flags 0x00) ist mit seinen Bytes dekodiert und erneut kodiert (8be8c6101ca).
### Annex B.5.2 DATA mit einer Folge ohne SampleInfo
**Spec:** Annex B.5.2, S. 155–157 — `submessageLength` 16, `data_seq.length` = 2,
zwei `long`.

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_5_2_data_with_a_sequence_without_sample_info`

**Status:** done — das Beispiel ist getestet: Kennung 0x09 (das Bild nennt 0x08), flags 0x08 (E-Flag gelöscht, FORMAT_DATA_SEQ; das Bild nennt 0x00), `sequenceNr` {0x0A,0x00} = 10 (der Text nennt 1), `request_id` {AA,01} nach der Tabelle (der Text nennt {AA,02}) (8be8c6101ca).
### Annex B.5.3 DATA mit einem Sample und SampleInfo

**Spec:** Annex B.5.3, S. 157–159 — IDL mit `SampleInfoDetail`; Bild: "info.state
| info.sequence_number | info.session_time_offset | XCDR_Serialization(foo)".

**Repo:** `crates/xrce/src/submessages/data.rs` (`DataPayloadSample`);
`crates/xrce/src/sample_info.rs` (`SampleInfo`)

**Tests:** `crates/xrce/src/submessages/data.rs::tests::data_sample_matches_annex_b_5_3_layout`,
`crates/xrce/src/sample_info.rs::tests::sample_info_matches_annex_b_5_3_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_is_answered_with_a_sample_each`

**Status:** `done` — die Tests bilden das Bild des Beispiels nach. Das Bild hat
die Form von §7.7.1, die IDL darüber die von Annex A (offenes Item „Annex A
SampleInfo mit SampleInfoDetail“).

### Annex B.5.4 DATA mit einer Folge mit SampleInfo
**Spec:** Annex B.5.4, S. 159–161 — `submessageLength` 40, `sample_seq.length`
= 2, je Element `info` (12 Byte) und `data` (4 Byte).

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_5_4_data_with_a_sequence_with_sample_info`

**Status:** done — das Beispiel ist mit den 12 Oktetten der SampleInfo je Element getestet (Länge 40; flags 0x0A statt 0x00; `SampleInfoForm::Flat`) (8be8c6101ca).
### Annex B.5.5 DATA mit PackedSamples
**Spec:** Annex B.5.5, S. 161–163 — `submessageLength` 36, `info_base` (12
Byte), `sample_delta_seq.length` = 2, je Element `info_delta` (4 Byte) und
`data` (4 Byte).

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_5_5_data_with_packed_samples`

**Status:** done — das Beispiel ist nach dem Bild getestet: `request_id`/`object_id`, `info_base` (12), Anzahl, zwei `SampleInfoDelta` mit Daten, 36 Oktette; Table 34 listet `info_base` auf Byte 8 bis 19 ohne `request_id` und `object_id`, was zu keinem von beiden passt (8be8c6101ca).

---

## Annex C Additional Transport Mappings

### Annex C.1 Rahmen nach RFC 1662 für serielle Leitungen

**Spec:** Annex C.1, S. 165 — "This section normalizes the communication over
serial protocols such as SPI, I2C or RS-232. For this propose, the XRCE
Messages are framed using an HDLC-like framing similar to [IETF RFC-1662].
This framing is designed to meet all the functionality listen in clause 11.1,
except that it does not provide security."

**Repo:** `crates/xrce/src/transport_serial.rs` (`encode_frame_with`, `SerialFramer`);
`crates/xrce-agent/src/daemon/transport.rs` (`SerialLine::take_frame`, `serial::run_serial_listener`,
Feature `serial`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::the_framer_takes_frames_split_and_back_to_back_and_skips_noise`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_clients_of_a_serial_line_are_told_apart_by_their_address`

**Status:** `done` — Commits `cf109e295c0`. Rahmen nach C.1.1 bis C.1.3 in
`zerodds-xrce`, im Agent und in den Endpoint-SDKs (C, C++, Java, Python,
Ada-native). Micro-XRCE-DDS 3.0.1 rahmt ebenso, rechnet `frameCheck` aber als
CRC-16/ARC nur über die Payload (`StreamFramingProtocol.cpp`,
`stream_framing_protocol.c`, aus dem Quelltext gelesen); der Agent nimmt und
erzeugt diese Rahmen mit `frame_check: crc16-arc-payload`
(`FrameCheck::Crc16ArcPayload`). Nicht gegen einen Micro-XRCE-DDS-Client über
eine Leitung gemessen.

### Annex C.1.1 Aufbau des Rahmens

**Spec:** Annex C.1.1, S. 165 — "flag | sourceAddress | remoteAddress |
payloadLength | payload (up to 64KB) | frameCheck", je ein Oktett für flag und
die beiden Adressen, 16 Bit für `payloadLength` und `frameCheck`.

**Repo:** `crates/xrce/src/transport_serial.rs` (`encode_frame_with`, `FrameAssembler::push`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`

**Status:** done — `7E | sourceAddress | remoteAddress | payloadLength |
payload | frameCheck`; der Rahmen endet nach `payloadLength` Oktetten Payload
und zwei Oktetten `frameCheck`.

### Annex C.1.1.1 BEGIN_FLAG 0x7E

**Spec:** Annex C.1.1.1, S. 165 — "Each frame begins with a BEGIN_FLAG byte
which has a hexadecimal value of 0x7E."

**Repo:** `crates/xrce/src/transport_serial.rs` (`FLAG_BYTE`, `encode_frame_with`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`

**Status:** done

### Annex C.1.1.2 sourceAddress

**Spec:** Annex C.1.1.2, S. 165 — "The sourceAddress indicates the address of
the frame sender."

**Repo:** `crates/xrce/src/transport_serial.rs` (`SerialFrame::source`); `crates/xrce-agent/src/daemon/transport.rs` (`PeerTable::resolve_on_line`,
`bind_line`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::the_clients_of_a_serial_line_are_told_apart_by_their_address`

**Status:** done — der Agent sendet mit seiner Adresse (`address:`, Vorgabe 0,
die Adresse, an die die Micro-XRCE-DDS-Beispiele senden). Ein Rahmen gehört dem
Client, den sein Header oder CREATE_CLIENT nennt, sonst dem, der sich von seiner
`sourceAddress` registriert hat.

### Annex C.1.1.3 remoteAddress

**Spec:** Annex C.1.1.3, S. 165 — "The remoteAddress indicates the address of
the frame recipient."

**Repo:** `crates/xrce/src/transport_serial.rs` (`SerialFrame::remote`); `crates/xrce-agent/src/daemon/transport.rs` (`SerialLine::take_frame`,
`Route::Serial`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::the_clients_of_a_serial_line_are_told_apart_by_their_address`

**Status:** done — der Agent nimmt nur Rahmen an seine Adresse; seine
Nachrichten an einen Client tragen dessen Adresse.

### Annex C.1.1.4 payloadLength little-endian

**Spec:** Annex C.1.1.4, S. 166 — "The payloadLength indicates the length of the
payload field in bytes, that is, the XRCE Message. The payloadLength shall be
encoded using little endian format."

**Repo:** `crates/xrce/src/transport_serial.rs` (`encode_frame_with`, `FrameAssembler::push`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`

**Status:** done

### Annex C.1.1.5 payload ist eine XRCE-Nachricht

**Spec:** Annex C.1.1.5, S. 166 — "The payload shall contain an XRCE Message."

**Repo:** `crates/xrce/src/transport_serial.rs` (`encode_message`, `decode_frame`,
`SerialFramer::push_bytes`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::the_framer_takes_frames_split_and_back_to_back_and_skips_noise`,
`crates/xrce/src/transport_serial.rs::tests::decode_frame_returns_the_rest_after_the_frame`

**Status:** done — die Grenze ist 65 535 Oktette, was `payloadLength` fasst;
eine längere Payload weist `encode_frame` mit `FrameTooLong` ab.

### Annex C.1.1.6 frameCheck nach RFC 1662

**Spec:** Annex C.1.1.6, S. 166 — "The frameCheck shall contain the 16-bits
cyclic redundancy check (CRC) of the original frame without including the
BEGIN_FLAG or transparency stuffing. The frameCheck shall be computed as it is
described in [IETF RFC-1662] using the polynomials: x 16 + x 12 + x 5 + 1."

**Repo:** `crates/xrce/src/transport_serial.rs` (`fcs16`, `FrameCheck::compute`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::the_frame_check_is_the_fcs16_of_rfc_1662`,
`crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`

**Status:** done — FCS-16 nach RFC 1662 (gespiegelt `0x8408`, Init `0xFFFF`,
komplementiert) über Adressen, `payloadLength` und Payload; 0x906E für
„123456789“, der Prüfwert aus RFC 1662 Anhang C.

### Annex C.1.1.6 frameCheck little-endian

**Spec:** Annex C.1.1.6, S. 166 — "The frameCheck shall be encoded using little
endian format."

**Repo:** `crates/xrce/src/transport_serial.rs` (`encode_frame_with`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`

**Status:** done

### Annex C.1.2 Stopfen mit ESCAPE_FLAG und XOR_FLAG

**Spec:** Annex C.1.2, S. 166 — "The ESCAPE_FLAG has a hexadecimal value of
0x7D. Each byte inside the frame which value matches with the BEGIN_FLAG or the
ESCAPE_FLAG is replaced by two bytes: the ESCAPE_FLAG followed by the original
byte XOR with the XOR_FLAG which has a value of 0x20. [...] each ESCAPE_FLAG
found shall be discarded and an XOR with the XOR_FLAG operator shall be applied
on the next byte."

**Repo:** `crates/xrce/src/transport_serial.rs` (`ESCAPE_BYTE`, `STUFF_XOR`, `encode_frame_with`,
`FrameAssembler::push`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::flag_and_escape_octets_are_stuffed_everywhere_after_the_flag`,
`crates/xrce/src/transport_serial.rs::tests::escape_errors_and_short_frames_are_refused`

**Status:** done — gestopft wird alles nach dem Flag, auch Adressen, Länge und
`frameCheck`. Nach `0x7D` weist der Code jedes Byte ab, das nicht zu `0x7E` oder
`0x7D` wird; die Spec verlangt das nicht.

### Annex C.1.3 Verwerfen bei falscher CRC

**Spec:** Annex C.1.3, S. 166 — "A frame shall be dropped in the following
cases: When the frameCheck attached to the frame is not the same as the
computed CRC."

**Repo:** `crates/xrce/src/transport_serial.rs` (`FrameAssembler::push`); `crates/xrce-agent/src/daemon/transport.rs`
(`run_serial_listener`: Meldung „frame dropped“ mit dem Grund)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_frame_with_a_wrong_check_is_refused`,
`crates/xrce/src/transport_serial.rs::tests::the_frame_check_of_micro_xrce_dds_is_taken_when_chosen`

**Status:** done

### Annex C.1.3 Verwerfen bei BEGIN_FLAG vor dem Ende der Payload

**Spec:** Annex C.1.3, S. 166 — "When a (non escaped) BEGIN_FLAG is found
before the payload is completely read."

**Repo:** `crates/xrce/src/transport_serial.rs` (`SerialFramer::push_bytes`, `decode_frame_inner_with`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_begin_flag_inside_a_frame_drops_it_and_begins_the_next`

**Status:** done — der Rahmen wird mit `BeginFlagInFrame` verworfen, und das
Flag beginnt den nächsten Rahmen.

---

## Audit-Status

490 done / 0 partial / 0 open / 79 n/a (informative) / 0 n/a (rejected).

Test-Lauf: die Tests der Items liefen auf dem Linux-Testhost in den Runden 1 bis 6 (`zerodds-xrce`, `zerodds-xrce-agent`, `zerodds-xrce-client` mit und ohne Standard-Features, `zerodds-endpoint-e2e`, `tools/endpoints-gate` 24 von 24); die Items, die erst danach geändert wurden, sind von keinem Lauf bestätigt. Dieses Dokument hat keinen Test ausgeführt.

Formen anderer DDS-Produkte in DDS-XML: die Leser von `crates/xml` folgen den Schemas von DDS-XML 1.0 und weisen eine fremde Form ab (`<domainparticipant_qos>`, das Attribut `qos_profile_ref`, `baseType`, die IDL-Namen `long` und `octet` als `type`, und weitere), mit einem Fehler, der die Form und die der Spec nennt. Die Agent-Einstellung `vendor_forms: accept` liest sie (nötig für Profildateien von eProsima) und listet beim Start jede gelesene Form; Standard ist `refuse`. Geprüft ist das nur durch Lesen des Codes und der Tests; kein Lauf auf diesem Rechner.

Offene Punkte: keine.
