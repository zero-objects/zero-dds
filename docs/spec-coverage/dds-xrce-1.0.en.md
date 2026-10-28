# DDS-XRCE 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xrce-1.0.pdf` (166 pages, OMG formal/2020-02-01) —
[OMG DDS-XRCE 1.0](https://www.omg.org/spec/DDS-XRCE/1.0/)

Item-by-item audit against the PDF according to
`internal/spec-coverage/PROCESS.md`. Page numbers are PDF pages (PDF page =
printed page + 8). Every field of the types of §7.7 and Annex A, every
submessage with its flags and payload, every check and every return value of
the operations of §7.8, every rule of the reliable streams, every row of
Table 5, Table 6 and Table 15 and every transport mapping is an item of its
own. `done` stands only where code and test cover the requirement; a test
citation names the test function as `file::module::name`. A repository
citation names file, symbol and, where it helps, the line (l.); the lines refer
to the state with which this file was checked in. Every statement about
behaviour is read from the code; the cited tests ran on the Linux test host,
this audit runs none. Where code carries a requirement but no test checks it,
the item is `partial` and the note names the missing test.

**Context:** DDS-XRCE is spread over these parts of ZeroDDS:

- `crates/xrce/` — the message codec (`MessageHeader`, `Submessage`, the 16
  payloads), the types of §7.7 (`ObjectId`, `ObjectVariant`, `ResultStatus`,
  `ObjectInfo`, `SampleInfo`, `TransportLocator`), `CLIENT_Representation` and
  `AGENT_Representation` (`handshake`), reliable streams (`reliable`),
  fragmentation (`fragment`), continuous reading (`continuous_read`), the
  object store per client (`object_store`), transports for UDP, TCP and a
  serial line (`transport_*`), DTLS and TLS behind the features `dtls` and
  `tls`, a generator of CREATE messages from an XML file (`xml_config`).
- `crates/xrce-agent/` — `XrceAgent` (client registry, object stores, sample
  queue per reader); behind the feature `daemon` the session logic
  (`daemon::session::SessionEngine`), the reading of representations
  (`daemon::entity_repr`), profiles (`daemon::profiles`), the connection to
  `zerodds-dcps` (`daemon::dcps_bridge::RealDdsPort`), listeners for UDP, TCP
  and serial (`daemon::transport`), discovery (`daemon::discovery`) and the
  binary `zerodds-xrce-agentd`.
- `crates/xrce-client/` — `XrceClient`, a synchronous client over the trait
  `ClientTransport`.
- `crates/endpoint-e2e/tests/xrce_write_framing.rs` — checks the WRITE_DATA
  frames of the endpoint SDKs of further languages against the codec of
  `crates/xrce`.

The specification defines no programming interface of the client (§8.1,
p. 57: "Do not have a standard API"). The endpoint SDKs of the languages are
therefore not listed as items; the ZeroDDS C API has no functions for XRCE.

Several structures have different forms in §7/§8 and in Annex A
(`CLIENT_Representation`, `SampleInfo`, the binary forms of the objects and
others). The item names the form the code follows; the form of the other place
is an item of its own. Where the code follows the form eProsima Micro-XRCE-DDS
3.0.1 sends, this rests on the capture under
`tools/wire-capture/xrce-sessions/`; a run against an eProsima agent or client
is not part of this audit.

The specification names `dds_xrce_types.idl` and `dds_xrce_model.xmi` as
normative machine-readable files (p. 1). They are not in the cache; a
comparison with them is missing.

**Open items:** none.

---

## §1 Scope

### §1 Scope

**Spec:** §1, p. 11 — "This specification defines an XRCE Protocol between a
resource constrained, low-powered device (client) and an Agent (the server).
[...] The specification's purpose and scope are to ensure that applications
based on different vendors' implementations of the XRCE Protocol and XRCE Agent
are compatible and interoperable."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; protocol and agent are listed
item by item from §7 on.

---

## §2 Conformance

### §2 Ten profiles as conformance points

**Spec:** §2, p. 12 — "This specification defines ten profiles. Each
constitutes a separate conformance point".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping; whether a profile is met follows
from the status of the mapped items. The tests in
`crates/xrce/tests/profile_conformance.rs` only check that the values of
`SubmessageId` decode; they show no behaviour.

### §2 Read Access profile

**Spec:** §2, p. 12 — "Provides the clients the ability to read data on
pre-configured Topics with pre-configured QoS policies. Requires implementation
of all submessage types except for CREATE, INFO, WRITE_DATA, and DELETE,
including the associated behaviors."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: items on §8.3.5 (all submessages
except CREATE, INFO, WRITE_DATA, DELETE), §7.8.5, §8.4.3, §8.4.14. GET_INFO
without a session and InfoMask (65cd1820a04), the DATA formats (d5fbdbe672e),
`MAX_SAMPLES_ZERO` (2fa0800f55a) and pre-configured DataReaders (b6e55cfa859)
are carried in those items.

### §2 Write Access profile

**Spec:** §2, p. 12 — "Provides the clients the ability to write data on
pre-configured Topics with pre-configured QoS policies. Requires implementation
of all submessage types except for CREATE, INFO, READ_DATA, DATA, and DELETE,
including the associated behaviors."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: items on §8.3.5, §7.8.4, §8.4.2,
§8.4.14. STATUS on WRITE_DATA (dc0209b38fe), the sequence formats of WRITE_DATA
(4f1fb9f5f4f) and pre-configured DataWriters (b6e55cfa859) are carried in those
items.

### §2 Configure Entities profile

**Spec:** §2, p. 12 — "Provides the clients the ability define
DomainParticipant, Topic, Publisher, Subscriber, DataWriter, and DataReader
entities using pre-configured QoS policies and data-types. Requires
implementation of the CREATE_CLIENT, DELETE_CLIENT, CREATE, and DELETE
submessage and the associated behaviors."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: items on §7.8.2, §7.8.3, §7.7.3 and
Table 6. Table 10 has no DELETE_CLIENT submessage; `delete_client` runs over
DELETE (§8.3.5.4). The participant check for Publisher and Subscriber
(dd55e9c9df1) and the DDS-XML forms of the representations (3aea52dd70a,
9239562286c) are carried in those items.

### §2 Configure QoS profile

**Spec:** §2, p. 12 — "Provides client the ability to define QoS profiles to
be used by DDS entities. Requires implementation of the CREATE submessage and
the associated behaviors for object kind OBJK_QOSPROFILE."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: items on §7.7.3.2, Table 6
OBJK_QOSPROFILE and §8.4.7. The DDS-XML form of the QosProfile is implemented
(3aea52dd70a, 9239562286c); which policies the agent applies is in the items on
§7.1.

### §2 Configure types profile

**Spec:** §2, p. 12 — "Provides client the ability to explicitly define data
types to be used for DDS Topics. Requires implementation of the CREATE
submessage and the associated behaviors for object kind OBJK_TYPE."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: items on §7.7.3.3, Table 6 OBJK_TYPE
and §8.4.8. A `<types>` that refers to types the agent already knows
(§7.7.3.3.2) is `done` there.

### §2 Discovery access profile

**Spec:** §2, p. 12 — "Provides the clients the ability to discover the Topics
and Types available on the DDS Global Data Space. Requires implementation of
the GET_INFO and INFO submessage and the associated behaviors."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: items on §7.8.2.2, §7.8.3.3,
§8.3.5.3, §8.3.5.7 and §7.7.13. GET_INFO returns the agent (also without a
session), the objects of the client and the objects configured for it
(65cd1820a04, b6e55cfa859). The agent has no query for the Topics and Types of
the Global Data Space; `session.rs`, `dds_port.rs` and `dcps_bridge.rs` hold no
code for it.

### §2 File based configuration profile

**Spec:** §2, p. 12 — "Provides a standard way to configure the Agent using
XML files. Requires implementation of the file-based configuration mechanism
described in clause 9.3".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: items on §9.3 and Table 15.

### §2 UDP Transport profile

**Spec:** §2, p. 12 — "Implements the mapping of the protocol to the UDP
transport. Requires implementing the mechanisms described in clause 11.2 (UDP
Transport)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: items on §11.2.

### §2 TCP Transport profile

**Spec:** §2, p. 12 — "Implements the mapping of the protocol to the TCP
transport. Requires implementing the mechanisms described in clause 11.3 (TCP
Transport)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: items on §11.3.

### §2 Complete profile

**Spec:** §2, p. 12 — "Complete profile. Requires implementation of the
complete specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: all items of this document.

---

## §3 References

### §3.1 [IETF RFC-1982] Serial Number Arithmetic

**Spec:** §3.1, p. 13 — "[IETF RFC-1982] Serial Number Arithmetic."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the requirement is in the item on
§8.3.2.3.

### §3.1 [IDL] IDL 4.2

**Spec:** §3.1, p. 13 — "[IDL] Interface Definition Language (IDL), version
4.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the types of Annex A are written
in IDL and listed as items on §7.7 and Annex A.

### §3.1 [DDS] DDS 1.4

**Spec:** §3.1, p. 13 — "[DDS] Data Distribution Service for Real-Time
Systems Specification, version 1.4".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; DDS entities in the items on
Table 6, the filter syntax in the item on Table 9.

### §3.1 [DDS-XML] DDS Consolidated XML Syntax 1.0

**Spec:** §3.1, p. 13 — "[DDS-XML] DDS Consolidated XML Syntax, version 1.0".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the XML forms in the items on
§7.7.3 and §9.3.

### §3.1 [DDS-XTYPES] XTypes 1.2

**Spec:** §3.1, p. 13 — "[DDS-XTYPES] Extensible And Dynamic Topic Types for
DDS, version 1.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; XCDR2 in the item on §7.7.

### §3.1 [UML] 2.5

**Spec:** §3.1, p. 13 — "[UML] Unified Modeling Language, version 2.5".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference for the diagrams.

### §3.1 [UDP] RFC 768

**Spec:** §3.1, p. 13 — "[UDP] User Datagram Protocol, IETF RFC 768".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the mapping in the items on
§11.2.

### §3.1 [TCP] RFC 793

**Spec:** §3.1, p. 13 — "[TCP] Transmission Control Protocol, STD 7, IETF RFC
793".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the mapping in the items on
§11.3.

### §3.1 [DTLS] RFC 6347

**Spec:** §3.1, p. 13 — "[DTLS] Datagram Transport Layer Security, version
1.2, IETF RFC 6347".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the specification names DTLS in
§11.2 (p. 99) as an option. `crates/xrce/src/transport_dtls.rs` (feature
`dtls`) is named in the item on §11.2.

### §3.1 [TLS] RFC 5246

**Spec:** §3.1, p. 13 — "[TLS] The Transport Layer Security (TLS) Protocol,
version 1.2, IETF RFC 5246".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the specification names TLS in
§11.3 (p. 100–101) as an option. `crates/xrce/src/transport_tls.rs` (feature
`tls`) is named in the item on §11.3.

### §3.1 [IETF RFC-1662] PPP in HDLC-like Framing

**Spec:** §3.1, p. 13 — "[IETF RFC-1662] PPP in HDLC-like Framing."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the framing in the items on
Annex C.

### §3.2 [SMART] Smart Transducers 1.0

**Spec:** §3.2, p. 13 — "[SMART] Smart Transducers Specification, version
1.0".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — non-normative reference.

---

## §4 Terms and Definitions

### §4 Terms

**Spec:** §4, p. 14 — definitions of "Data Distribution Service (DDS)",
"DDS Domain", "DDS DomainParticipant", "DDS Global Data Space", "GUID".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — terms.

---

## §5 Symbols

### §5 Acronyms

**Spec:** §5, p. 15 — DDS, IDL, RTPS, XRCE.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acronyms.

---

## §6 Additional Information

### §6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, p. 16 — "This specification does not change any adopted OMG
specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — note.

### §6.2 Acknowledgements

**Spec:** §6.2, p. 16 — submitting companies (Real-Time Innovations,
eProsima, TwinOaks Computing).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acknowledgement.

---

## §7.1 General

### §7.1 The agent is a DDS participant

**Spec:** §7.1, p. 17 — "The XRCE Agent is a DDS Participant in the DDS Global
Data Space. The DDS-XRCE protocol allows the client to use the XRCE Agent as a
proxy in order to produce and consume data in the DDS Global Data Space."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort`,
`runtime_for`: one `DcpsRuntime` per domain)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** `done` — the test writes and reads through a `DomainParticipant` of
`zerodds-dcps` with no XRCE on the other side.

### §7.1 UML model of the agent

**Spec:** §7.1, p. 17 — "this specification defines a UML model for the XRCE
Agent. This model, called the DDS-XRCE Object Model, defines the objects,
interfaces, and operations to be implemented by the agent."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping; the objects are in the items on
§7.4 to §7.6, the operations in §7.8.

### §7.1 Every topic with every QoS
**Spec:** §7.1, p. 17 — "Despite being simpler, the DDS-XRCE Object Model
provides XRCE clients complete access to the DDS Global Data space. Any DDS
Topic may be published or subscribed to on any DDS with any QoS."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_qos_profile`, `qos_policy`, `TopicQos`, `participant_qos_of`); `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`create_writer`, `create_reader`, `create_participant_with`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_qos_of_a_topic_is_the_default_of_its_endpoints`, `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_qos_of_an_xml_participant_is_applied`, `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_participant_qos_profile_reference_is_applied`, `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_participant_qos_is_applied_to_the_participant`

**Status:** done — every policy the DDS entity has is applied: on an endpoint RELIABILITY, DURABILITY, DURABILITY_SERVICE, HISTORY, DEADLINE, LATENCY_BUDGET, LIFESPAN, LIVELINESS, OWNERSHIP, OWNERSHIP_STRENGTH, DESTINATION_ORDER, TRANSPORT_PRIORITY, TIME_BASED_FILTER, PARTITION, USER_DATA, TOPIC_DATA, RESOURCE_LIMITS; on a Publisher and Subscriber PARTITION, GROUP_DATA, PRESENTATION, ENTITY_FACTORY; on a Topic as the default of its endpoints (`TopicQos`); on a DomainParticipant USER_DATA (a runtime of its own) and ENTITY_FACTORY, from the `qos_profile_reference` of the binary form and from `<domain_participant_qos>` of the XML form (b4cd825c268, 2c715803843). What the runtime cannot do is not accepted and ignored: `STATUS_ERR_INCOMPATIBLE`, unless `unsupported_qos: ignore` (a setting for the profile files of other vendors).
### §7.1 QoS the agent cannot apply
**Spec:** §7.1, p. 17 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_not_applied`, `create_core`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_qos_profile`); `crates/xrce-agent/src/daemon/config.rs` (`unsupported_qos`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::qos_the_agent_cannot_apply_is_refused_unless_ignoring_is_named`, `crates/xrce-agent/src/daemon/config.rs::tests::unsupported_qos_is_refused_unless_ignoring_is_named`

**Status:** done — a QoS the agent cannot apply (WRITER_DATA_LIFECYCLE with autodispose off, READER_DATA_LIFECYCLE with an autopurge delay, an element of a vendor dialect) makes the CREATE `STATUS_ERR_INCOMPATIBLE`; nothing is stored. `unsupported_qos: ignore` creates the object without it and reports what is missing (`SessionEvent::RepresentationNotApplied`). A policy without meaning for the kind of endpoint (a TIME_BASED_FILTER of a writer) is not part of its QoS and is not reported. 2c715803843.

---

## §7.2 XRCE Client

### §7.2 Reasons for an object model of its own

**Spec:** §7.2, p. 18 — three reasons: (1) "These clients expect a simplified
interface with no callbacks, and use parameters encoded in text." (2) "The
DDS-XRCE DDS Object Model must overcome intermittent connectivity by
introducing a 'session,' which can exist across repeated sleep-wakeup cycles by
a device." (3) "it is desirable to have an access control model that
authenticates each client application/principal".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale. The session is in the items on
§8.2.2 and §7.8.2.1, access control in the item on §7.4 AccessController.

### §7.2 Kinds of clients

**Spec:** §7.2, p. 19 — simple devices without discovery, devices that
activate and deactivate "data push", clients that create objects themselves,
and clients that use sequence numbers and timestamps.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description; switching delivery on and off is
in the item on Table 9 `max_samples`.

---

## §7.3 XRCE Agent

### §7.3 Presence of the client in the DDS data space

**Spec:** §7.3, p. 19 — "The purpose of the DDS-XRCE Agent (XRCE Agent) is to
establish and maintain the presence of the XRCE Client in the DDS data-space."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine`,
`apply_dds_create`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::create_writer`, `create_reader`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** `done`

### §7.3 Pre-configured and client-created resources

**Spec:** §7.3, p. 19 — "Resources may be preconfigured with well-known names,
or dynamically created by an XRCE Client."

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::predefined_objects`);
`crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions`);
`crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::ensure_predefined`,
`make_client_applications`); `crates/xrce-agent/src/daemon/config.rs` (keys
`xml_profile_files`, `client_applications`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_reference_is_read_like_the_profile_it_names`

**Status:** `done` — resources created by the client: CREATE. Configured
resources: the files of §9.3 (`<types>`, `<qos_library>`, `<domain_library>`,
`<domain_participant_library>`, `<application_library>`) from
`xml_profile_files:` are provided under the ObjectId from the MD5 of their name
(03371b4965c, dc63d65691f, 3aea52dd70a, b6e55cfa859).

### §7.3 Writing through a pre-configured resource without CREATE

**Spec:** §7.3, p. 19 — "a Client that has access to this resource can write
data using this resource simply by referring to the existing 'MySquareWriter'.
The Client does not have to create a resource."

**Repo:** `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::ensure_predefined`, called from `handle_write_data`,
`handle_read_data`, `handle_get_info` and `handle_create`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_reads_through_a_preconfigured_data_reader_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_preconfigured_object_is_made_only_for_a_client_allowed_to_create_it`

**Status:** `done` — WRITE_DATA on the ObjectId of a configured DataWriter makes
the DomainParticipant with everything inside it for the client (b6e55cfa859);
the access rules apply. An ObjectId with no configuration gives
`STATUS_ERR_UNKNOWN_REFERENCE`.

### §7.3 Queries by the client

**Spec:** §7.3, p. 19 — "This specification enables Clients to be in charge of
when data is received, and to request the XRCE Agent to return data that
matches a set of constraints. [...] an XRCE Client queries the XRCE Agent when
it wakes up."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`pump_read`); `crates/xrce-agent/src/lib.rs` (`XrceAgent::pull_sample`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_elapsed_time_is_measured_from_the_real_request_time`,
`crates/xrce-agent/src/lib.rs::tests::after_submit_pull_returns_sample_in_fifo_order`

**Status:** `done`

### §7.3 DDS-RTPS towards the other participants

**Spec:** §7.3, p. 20 — "the standard interactions occurring with the XRCE
Client use the DDS-XRCE protocol, while interactions with other DDS domain
participants use the DDS-RTPS protocol."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** `done`

### §7.3 Shared or dedicated DDS entities

**Spec:** §7.3, p. 19 — "a resource 'MySquareWriter' may be associated with a
DDS DataWriter shared by many DDS-XRCE clients, or an XRCE Client may have its
own dedicated 'MySquareWriter'".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — implementation freedom; the duty to behave
"as if" is in the item on §10.3.

---

## §7.4 Model Overview

### §7.4 Root as singleton and factory

**Spec:** §7.4, p. 21 — "The Root singleton is the entry point for the service.
It functions as a factory for all the Objects managed by the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine`);
`crates/xrce-agent/src/lib.rs` (`XrceAgent`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`

**Status:** `done`

### §7.4 ProxyClient

**Spec:** §7.4, p. 21 — "The ProxyClient class models the XRCE Client
application that interacts with the XRCE Agent using the XRCE protocol."

**Repo:** `crates/xrce-agent/src/lib.rs` (`XrceAgent::register_client`, one
`ObjectStore` per `ClientKey`)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::multiple_clients_isolated`

**Status:** `done`

### §7.4 An application belongs to one ProxyClient

**Spec:** §7.4, p. 21 — "Each Application object is associated with a single
XRCE ProxyClient and gets its access rights from those assigned to the XRCE
Client."

**Repo:** `crates/xrce-agent/src/lib.rs` (`XrceAgent::create_object`);
`crates/xrce-agent/src/daemon/access.rs` (`AccessController::permits`, rules per
ClientKey, ObjectKind `application`); `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::permit`)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::create_application_object_via_objk_application`,
`crates/xrce-agent/src/lib.rs::tests::multiple_clients_isolated`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_without_permission_is_denied_and_stores_nothing`

**Status:** `done` — an Application is held in the object store of its client;
its rights are the client's rules in the AccessController (d66ada44616,
515bcc677f3). Read from the code: the check takes the ClientKey and the
ObjectKind of the object; no test has an Application as the refused object.

### §7.4 Application with DomainParticipants

**Spec:** §7.4, p. 21 — "The Application class models a software application
that connects with the XRCE Client and manages the DDS objects needed to
publish and subscribe data on one or more DDS Domains. An XRCE Application can
be associated with zero of more DomainParticipant objects."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create` returns
`DdsCreate::Application`, `create_nested` makes the DomainParticipants, each
under the ObjectId of its name); `crates/xrce-agent/src/daemon/definitions.rs`
(`ApplicationDef`, `ParticipantDef`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_creates_the_objects_inside_it`

**Status:** `done` — an Application from an `<application_library>` makes its
DomainParticipants with everything inside them, also for a client it is assigned
to in `client_applications:` (3aea52dd70a, b6e55cfa859).

### §7.4 AccessController

**Spec:** §7.4, p. 21 — "The AccessController is responsible for making
decisions regarding the resources and operations a particular XRCE ProxyClient
is allowed to perform. It contains rules that associate a Client with
privileges, which determine which DDS domain an application executing on
behalf of a client may join, which DDS Topics the application can read and
write, and so on."

**Repo:** `crates/xrce-agent/src/daemon/access.rs` (`AccessController`, `Rule`,
`Subject`, `Operation`); `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::permit`, `permits`, `deny_create_client`);
`crates/xrce-agent/src/daemon/config.rs` (keys `authorized_client_keys`,
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

**Status:** `done` — rules per client for operation, ObjectKind, DDS domain and
topic; an operation no rule matches is answered with `STATUS_ERR_DENIED` as soon
as one rule exists, a controller without rules restricts nothing (d66ada44616,
515bcc677f3).

### §7.4 DomainParticipant as a proxy

**Spec:** §7.4, p. 21 — "The DDS-XRCE DomainParticipant is a proxy for a DDS
DomainParticipant and models the association with a DDS domain and the
capability of the Application to publish and subscribe to Topics on that
domain."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::create_participant_with`, `runtime_for`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::full_chain_against_a_real_leased_domain`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::two_large_samples_reach_a_default_reader_on_the_same_agent`

**Status:** `done` — all XRCE DomainParticipants of a domain share one
`DcpsRuntime`, except a participant with USER_DATA (a runtime of its own per
domain and value); §10.3 allows that as long as the behaviour looks as if each
client had entities of its own.

---

## §7.5 XRCE DDS Proxy Objects

### §7.5 Proxy objects delegate to DDS objects

**Spec:** §7.5, p. 22 — "These proxy objects allow the client application to
participate as first-class citizens on the DDS network by delegating the actual
DDS behavior and DDS-RTPS protocol implementation to the proxy DDS objects."
Figure 5: DomainParticipant, Publisher, Subscriber, DataWriter, DataReader,
Topic, QosProfile and Qos as values.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`apply_dds_create`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::create_participant_with`, `create_publisher`,
`create_subscriber`, `create_writer`, `create_reader`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_publisher_is_a_dds_entity_of_its_participants_domain`

**Status:** `done` — DataWriter and DataReader are DDS endpoints in the
`DcpsRuntime`, Publisher and Subscriber each have a DDS entity of their own in
the participant (dd55e9c9df1), the Topic is held as name and type in the state
of the `RealDdsPort`, QosProfile and Qos enter as values.

### §7.5 Publisher and Subscriber as a DDS entity

**Spec:** §7.5, p. 22 — as above.

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

**Status:** `done` — Publisher and Subscriber are a group entity in the
participant of their domain; PRESENTATION and ENTITY_FACTORY from the XML forms
take effect (ENTITY_FACTORY `autoenable_created_entities: false` refuses the
endpoints under it), PARTITION and GROUP_DATA reach the endpoints through their
`EndpointQos`. A participant that does not exist gives
`STATUS_ERR_UNKNOWN_REFERENCE` (dd55e9c9df1). GROUP_DATA is announced with the
endpoint, not with an announcement of the Publisher of its own.

---

## §7.6 XRCE Object Identification

### §7.6 ObjectId unique per agent and ClientKey

**Spec:** §7.6, p. 22 — "This implies that the ObjectId shall be unique within
the scope of an Agent and a ClientKey."

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore`);
`crates/xrce-agent/src/lib.rs` (`XrceAgent::register_client`, one object store
per `ClientKey`)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_strict_on_existing_returns_conflict`,
`crates/xrce-agent/src/lib.rs::tests::multiple_clients_isolated`

**Status:** `done`

### §7.6 ObjectId configured in the agent or chosen by the client

**Spec:** §7.6, p. 22 — "The value of the ObjectId for a particular object
shall be configured on the XRCE Agent or specified by the XRCE Client at the
time the object is created."

**Repo:** `crates/xrce/src/object_id.rs` (`ObjectId::from_reference`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create`, `ensure_predefined`)

**Tests:** `crates/xrce/src/object_id.rs::tests::from_reference_matches_the_examples_of_the_spec`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_object_id_is_the_prefix_and_the_kind_of_the_representation`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done` — the `ObjectId` the client chose is taken over; the
`ObjectId` configured in the agent is the one from the MD5 of the name per §9.3
and Table 16 (03371b4965c, b6e55cfa859).

### §7.6 OBJECTID_INVALID {0x00, 0x00}

**Spec:** §7.6, p. 23 — "The value {0x00, 0x00} is referred as
OBJECTID_INVALID and represents an invalid object."

**Repo:** `crates/xrce/src/object_id.rs` (`OBJECTID_INVALID` = 0x0000,
`OBJECTID_SESSION` = 0xFFFF); `crates/xrce/src/object_store.rs`
(`ObjectStore::create` refuses `OBJECTID_INVALID`)

**Tests:** `crates/xrce/src/object_id.rs::tests::invalid_object_id_is_all_zeros`,
`crates/xrce/src/object_id.rs::tests::reserved_object_ids_per_spec_7_6_and_annex_a`,
`crates/xrce/src/object_store.rs::tests::create_invalid_object_id_rejected`

**Status:** `done` — `OBJECTID_INVALID` is `{0x00, 0x00}`, `0xFFFF` is
`OBJECTID_SESSION` of Annex A (f63fa2d1218); the constants of the SDKs in
`endpoints/` follow (5530a71bb29).

### §7.6 OBJECTID_CLIENT {0xFF, 0xFE}

**Spec:** §7.6, p. 23 — "The value {0xFF, 0xFE} is referred as OBJECTID_CLIENT
and represents the XRCE ProxyClient object."

**Repo:** `crates/xrce/src/object_id.rs` (`OBJECTID_CLIENT`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_delete`)

**Tests:** `crates/xrce/src/object_id.rs::tests::client_singleton_has_kind_client`,
`crates/xrce-client/src/lib.rs::tests::disconnect_sends_delete_client_and_clears_state`

**Status:** `done`

### §7.6 Two reserved values

**Spec:** §7.6, p. 23 — "There are two reserved values for ObjectId."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Annex A (p. 103) names four values
(`OBJECTID_INVALID`, `OBJECTID_AGENT`, `OBJECTID_CLIENT`, `OBJECTID_SESSION`);
the two further ones are listed as items on Annex A.

### §7.6 Resource name instead of ObjectId

**Spec:** §7.6, p. 23 — "Alternatively, objects may also be identified by a
string resourceName. The format of this name depends on the resource and
provides a way to refer to a resource configured on the agent using a
configuration file or similar means."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`referenced`);
`crates/xrce-agent/src/daemon/profiles.rs` (`AgentProfiles`);
`crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_is_created_by_name_or_by_register_type_ref`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_reference_is_read_like_the_profile_it_names`

**Status:** `done` — a name resolves against the agent's files: the
`profile_name` of a profile file and the DDS-XML names (`Library::Element`,
Table 15) of the libraries of §9.3 (dc63d65691f, 3aea52dd70a).

---

## §7.7 Data types used to model operations on XRCE Objects

### §7.7 IDL as in Annex A, XCDR2 encoding

**Spec:** §7.7, p. 23 — "The IDL definitions for the data types shall be as
specified in Annex A IDL Types. When serializing these types into a binary
representation the encoding shall follow the rules defined in in
[DDS-XTYPES] for XCDR version 2 encoding."

**Repo:** `crates/xrce/src/` (codecs per type, see the following items;
`encoding.rs`: `UnannotatedTypes`, `RepresentationTimestamps`);
`crates/xrce-agent/src/daemon/binary_forms.rs` (XCDR2 reader of the binary
forms)

**Tests:** `crates/xrce/tests/foreign_frames.rs::every_client_datagram_decodes_and_writes_back`,
`crates/xrce/tests/foreign_frames.rs::every_agent_datagram_decodes_and_writes_back`,
`crates/xrce/tests/unannotated_types.rs::the_default_is_appendable_with_a_dheader`

**Status:** `done` — the payloads of the submessages and the types of Annex A
are read and written as XCDR2. Types without `@extensibility` are APPENDABLE
with a DHEADER; the FINAL form of the reference implementation is the setting
`unannotated_types: final` (f2ace917b5a). Extensions beyond Annex A are in the
items of the individual types, among them `requested_mtu` behind `properties` of
the CLIENT_Representation. The classic CDR form of the binary objects of the
eProsima client stays as a fallback (`entity_repr.rs`, `Cdr`).

### §7.7 Alignment, DHEADER and byte order of the Annex A types

**Spec:** §7.7, p. 23 — as above.

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

**Status:** `done` — the deviations are fixed: `PropertySeq` with lengths
aligned to 4 (65cd1820a04); `TransportLocator` with alignment, DHEADER and
numeric ports (c304e509660); `content_filter_expression` as an XCDR2 string and
`DataDeliveryControl` with a DHEADER (1663f8f6240); `AGENT_ActivityInfo`,
`DATAREADER_ActivityInfo`, `DATAWRITER_ActivityInfo`, `ObjectInfo` and
`ActivityInfoVariant` per Annex A (65cd1820a04); the binary forms of Annex A as
XCDR2 with a DHEADER (c67ce23c724); `CLIENT_Representation`,
`AGENT_Representation`, `ObjectInfo` and the locators in the byte order of the E
flag (b43d5e6b848).

---

## §7.7.1 Data and Samples

### §7.7.1 Five formats and the values of DataFormat

**Spec:** §7.7.1, p. 23 — "The five possible representation are: SampleData,
Sample, SampleDataSeq, SampleSeq, and SamplePackedData. They respectively
correspond to the DataFormat values FORMAT_DATA, FORMAT_DATA_SEQ,
FORMAT_SAMPLE, FORMAT_SAMPLE_SEQ, and FORMAT_PACKED."

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`DataFormat`,
`AnyWriteData`); `crates/xrce/src/submessages/data.rs` (`AnyData`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::any_write_data_dispatches_every_format`,
`crates/xrce/src/submessages/data.rs::tests::any_data_dispatches_every_format`,
`crates/xrce/src/submessages/read_data.rs::tests::the_data_format_octet_has_the_values_of_annex_a`

**Status:** `done` — the code maps as Table 1 and Annex A do (`SampleDataSeq` to
FORMAT_DATA_SEQ, `Sample` to FORMAT_SAMPLE; values 0x00, 0x02, 0x08, 0x0A,
0x0E). The “respectively” list of this sentence maps `Sample` to FORMAT_DATA_SEQ
and names `SamplePackedData` and `FORMAT_PACKED`, which Annex A does not have.

### §7.7.1 SampleData

**Spec:** §7.7.1, p. 23 — "@extensibility(FINAL) struct SampleData {
XCDRSerializedBuffer serialized_data; }; In this structure the
XCDRSerializedBuffer represents the bytes resulting from serializing the
application-specific data type that is being sent using the XCDR version 2
rules defined in clause 7.4 of [DDS-XTYPES]."

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayload::encode_body`, `try_from_submessage`);
`crates/xrce/src/submessages/data.rs` (`DataPayload`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_matches_the_annex_b_example`,
`crates/xrce/src/submessages/data.rs::tests::data_base_object_request_precedes_the_sample`

**Status:** done — `serialized_data` runs without a length of its own to the
end of the submessage, as in Annex B.3.1 (`submessageLength = 6`). Annex A
does not define `XCDRSerializedBuffer`.

### §7.7.1 SampleInfoFlags

**Spec:** §7.7.1, p. 23 — "@bit_bound(8) bitmask SampleInfoFlags {
@position(0) INSTANCE_STATE_UNREGISTERED, @position(1)
INSTANCE_STATE_DISPOSED, @position(2) VIEW_STATE_NEW, @position(3)
SAMPLE_STATE_READ, };"

**Repo:** `crates/xrce/src/sample_info.rs` (`SampleInfoFlags`)

**Tests:** `crates/xrce/src/sample_info.rs::tests::sample_info_flags_bit_positions`,
`crates/xrce/src/sample_info.rs::tests::sample_info_flags_all_four_bits_independent`

**Status:** done

### §7.7.1 SampleInfo with sequence_number and session_time_offset

**Spec:** §7.7.1, p. 24 — "@extensibility(FINAL) struct SampleInfo {
SampleInfoFlags state; unsigned long sequence_number; unsigned long
session_time_offset; // milliseconds up to 53 days };"

**Repo:** `crates/xrce/src/sample_info.rs` (`SampleInfo`,
`WIRE_SIZE = 12`)

**Tests:** `crates/xrce/src/sample_info.rs::tests::sample_info_matches_annex_b_5_3_layout`,
`crates/xrce/src/sample_info.rs::tests::sample_info_roundtrip_be`

**Status:** done — `state`, 3 bytes of padding, two `unsigned long`. Annex A
(p. 115) defines `SampleInfo` differently; that is an item of its own on
Annex A.

### §7.7.1 Sample

**Spec:** §7.7.1, p. 24 — "@extensibility(FINAL) struct Sample { SampleInfo
info; SampleData data; };"

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayloadSample`); `crates/xrce/src/submessages/data.rs`
(`DataPayloadSample`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_sample_matches_annex_b_5_3_layout`,
`crates/xrce/src/submessages/data.rs::tests::data_sample_matches_annex_b_5_3_layout`

**Status:** done

### §7.7.1 DeciSecond and SampleInfoDelta

**Spec:** §7.7.1, p. 24 — "typedef unsigned short DeciSecond; // 10e-1
seconds @extensibility(FINAL) struct SampleInfoDelta { SampleInfoFlags state;
octet seq_number_delta; DeciSecond timestamp_delta; // In 1/10 of seconds };"

**Repo:** `crates/xrce/src/sample_info.rs` (`SampleInfoDelta`,
`WIRE_SIZE = 4`)

**Tests:** `crates/xrce/src/sample_info.rs::tests::sample_info_delta_roundtrip`,
`crates/xrce/src/sample_info.rs::tests::sample_info_delta_decode_short_is_eof`

**Status:** done

### §7.7.1 SampleDelta

**Spec:** §7.7.1, p. 24 — "@extensibility(FINAL) struct SampleDelta {
SampleInfoDelta info_delta; SampleData data; };"

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayloadPackedSamples::elements`); `crates/xrce/src/sample_info.rs`
(`SampleInfoDelta::decode`); `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::split_write_items`, `SessionEngine::data_submessages`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_packed_samples_matches_annex_b_5_5_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::packed_samples_resolve_their_deltas_against_the_base`

**Status:** done — the layout per element is mapped; in the codec the elements stay
one block (`elements`) because `SampleData` carries no length. The agent
separates them on WRITE_DATA by the type of the topic (`split_write_items`,
4f1fb9f5f4f) and writes them on READ_DATA (`data_submessages`, d5fbdbe672e).

### §7.7.1 PackedSamples

**Spec:** §7.7.1, p. 24 — "@extensibility(FINAL) struct PackedSamples {
SampleInfo info_base; sequence<SampleDelta> sample_delta_seq; };"

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayloadPackedSamples`); `crates/xrce/src/submessages/data.rs`
(`DataPayloadPackedSamples`)

**Tests:** `crates/xrce/src/submessages/data.rs::tests::data_packed_samples_matches_annex_b_5_5_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_packed_samples_is_answered_with_one_packed_samples`,
`crates/xrce-agent/src/daemon/session.rs::tests::packed_samples_resolve_their_deltas_against_the_base`

**Status:** done — the agent reads the form on WRITE_DATA (4f1fb9f5f4f) and writes it on
READ_DATA (d5fbdbe672e).

---

## §7.7.2 DataRepresentation

### §7.7.2 DataRepresentation as a union over DataFormat

**Spec:** §7.7.2, p. 24 and p. 26 — "The DataRepresentation is defined as a
union discriminated by a DataFormat. Depending on the discriminator it selects
one of the formats defined in clause 7.7.1." "union DataRepresentation
switch(DataFormat) { case FORMAT_DATA: SampleData data; case FORMAT_SAMPLE:
Sample sample; case FORMAT_DATA_SEQ: SampleDataSeq data_seq; case
FORMAT_SAMPLE_SEQ: SampleSeq sample_seq; case FORMAT_PACKED_SAMPLES:
PackedSamples packed_samples; };"

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`AnyWriteData`,
discriminator from the flags through `DataFormat::from_flag_bits`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::any_write_data_dispatches_every_format`

**Status:** done — the discriminator is in the submessage flags (§8.3.5.8.1),
not in the payload.

### §7.7.2 Table 1 FORMAT_DATA

**Spec:** §7.7.2, Table 1, p. 25 — "FORMAT_DATA struct SampleData [...]
Contains the data for a single sample without additional sample information."

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`WriteDataPayload`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_roundtrip_format_data`

**Status:** done

### §7.7.2 Table 1 FORMAT_DATA_SEQ

**Spec:** §7.7.2, Table 1, p. 25 — "FORMAT_DATA_SEQ struct SampleDataSeq [...]
Contains a sequence of data samples."

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayloadDataSeq`); `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::split_write_items`, `SessionEngine::data_submessages`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_data_seq_matches_annex_b_5_2_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_data_seq_is_split_by_the_type_and_written_sample_by_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_data_seq_is_answered_with_one_data_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_sequence_formats_split_at_the_size_of_a_submessage`

**Status:** done — number of elements and the elements as one block. The agent separates
the elements on WRITE_DATA by the type of the topic and writes each one
(4f1fb9f5f4f); on READ_DATA it writes one DATA_SEQ per submessage
(d5fbdbe672e). Assumption, not measured: the elements sit back to back with no
padding, each read from alignment origin 0 (Annex B.5.2 shows only elements of
4 bytes).

### §7.7.2 Table 1 FORMAT_SAMPLE with a timestamp relative to the session
**Spec:** §7.7.2, Table 1, p. 25 — "The SampleInfo holds the DDS
InstanceState, SampleState, and ViewState of the corresponding DDS Sample. It
also contains the sample sequence number and timestamp. The timestamp is
represented as an offset relative to the session timestamp established when
the session was created. The session timestamp corresponds to the
client_timestamp attribute in CLIENT_Representation".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`data_submessages`, `source_offset_ms`, `handle_write_data`); `crates/xrce/src/sample_info.rs` (`SampleInfo`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_sample_info_is_what_the_datareader_gave`, `crates/xrce-agent/src/daemon/session.rs::tests::the_client_timestamp_is_the_session_timestamp`

**Status:** done — state, sequence number and offset are carried. In WRITE_DATA the offset is the time after the start of the session and becomes the source timestamp (4f1fb9f5f4f). In READ_DATA the agent sends FORMAT_SAMPLE with the writer's source timestamp as the offset and the state of the DataReader (8be8c6101ca). The start of the session is the `client_timestamp` of the CREATE_CLIENT (0f32e029a31).
### §7.7.2 Content of the SampleInfo and reference point of the timestamp
**Spec:** §7.7.2, Table 1, p. 25 — as above.

**Repo:** `crates/xrce-agent/src/daemon/dds_port.rs` (`PolledSample`, `DdsPort::poll_samples`); `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`spawn_reader_pump`, `ReaderQueue::push`); `crates/xrce-agent/src/daemon/session.rs` (`data_submessages`, `session_start`); `crates/xrce-client/src/lib.rs` (`session_time_offset_ms`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_sample_info_is_what_the_datareader_gave`, `crates/xrce-agent/src/daemon/session.rs::tests::the_client_timestamp_is_the_session_timestamp`

**Status:** done — `DdsPort::poll_samples` returns `PolledSample` (payload, `SampleInfoFlags`, source timestamp): `VIEW_STATE_NEW` for the first sample of an instance, `INSTANCE_STATE_DISPOSED`/`_UNREGISTERED` for a dispose or unregister the DataReader reports (a sample without data), `SAMPLE_STATE_READ` never (a sample is handed over once). FORMAT_DATA and FORMAT_DATA_SEQ have no SampleInfo and send no markers. The reference point is the `client_timestamp` (`representation_timestamps: present`) or the agent's clock; the client writes the milliseconds since `connect` as the offset. 8be8c6101ca, 0f32e029a31.
### §7.7.2 Table 1 FORMAT_SAMPLE_SEQ

**Spec:** §7.7.2, Table 1, p. 25 — "FORMAT_SAMPLE_SEQ struct SampleSeq [...]
Contains a sequence of samples, each containing both the data and the
additional sample information."

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`WriteDataPayloadSampleSeq`); `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::split_write_items`, `SessionEngine::data_submessages`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_sample_seq_matches_annex_b_5_4_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_seq_carries_state_and_time_per_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_seq_is_answered_with_one_sample_seq`

**Status:** done — the agent reads the form on WRITE_DATA (state and time per sample,
4f1fb9f5f4f) and writes it on READ_DATA (d5fbdbe672e). What the SampleInfo
holds on reading is in the `open` item on §7.7.2.

### §7.7.2 Table 1 FORMAT_PACKED_SAMPLES and resulting_info

**Spec:** §7.7.2, Table 1, p. 25 — "This representation is limited to samples
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

**Status:** done — `resolve` converts `timestamp_delta` (tenths of a second) with 100 to
milliseconds; the formula of the spec adds quantities in milliseconds and
tenths of a second without a conversion. The agent reads the form on
WRITE_DATA and writes it on READ_DATA with at most 256 elements per submessage
(the range of `seq_number_delta`, an `octet`); it writes `timestamp_delta` and
`state` as 0 (4f1fb9f5f4f, d5fbdbe672e).

---

## §7.7.3 ObjectVariant

### §7.7.3 ObjectVariant as a union over ObjectKind

**Spec:** §7.7.3, p. 26 — "The ObjectVariant type is defined as a union
discriminated by ObjectKind. Each value of the discriminator selects an
appropriate object representation for that kind."

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::encode_for_kind`, `decode_for_kind`, `ObjectVariantExtra`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`,
`crates/xrce/src/object_repr.rs::tests::participant_representation_matches_the_captured_create_bytes`,
`crates/xrce/src/object_repr.rs::tests::subscriber_representation_matches_the_captured_create_bytes`

**Status:** done — `ObjectKind` as an octet, then the representation of the
kind and its trailing field (`domain_id` or the container's `ObjectId`).

### §7.7.3 Several formats per kind

**Spec:** §7.7.3, p. 26 — "For a given ObjectKind the, ObjectVariant type
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

### §7.7.3.1 Three values of RepresentationFormat

**Spec:** §7.7.3.1, p. 26 — "There are three RepresentationFormat values:
REPRESENTATION_BY_REFERENCE, REPRESENTATION_AS_XML_STRING, and
REPRESENTATION_IN_BINARY." Annex A, p. 106: 0x01, 0x02, 0x03.

**Repo:** `crates/xrce/src/object_repr.rs` (`repr_disc`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::discriminator_returns_correct_byte`,
`crates/xrce/src/object_repr.rs::tests::unknown_discriminator_rejected`

**Status:** done

### §7.7.3.1 Three or two formats per kind

**Spec:** §7.7.3.1, p. 26 — "Some object kinds support all three formats; in
this case the corresponding representation extends the type struct
OBJK_Representation3_Base. Other object kinds support only two formats, and
therefore, extend the type struct OBJK_RepresentationRefAndXML_Base or the type
OBJK_RepresentationBinAndXML_Base."

**Repo:** `crates/xrce/src/object_repr.rs` (`decode_for_kind`); `crates/xrce-agent/src/daemon/session.rs`
(`format_is_allowed`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`

**Status:** done — `decode_for_kind` reads all three discriminators; the agent admits per
kind only the formats of the spec: Participant and Topic all three, Type,
QosProfile and Application reference and XML, Publisher, Subscriber,
DataWriter and DataReader binary and XML. Any other format is
STATUS_ERR_INVALID_DATA (bdf3f6ee966). The setting is in the item "Allowed
formats per kind".

### §7.7.3.1 Allowed formats per kind

**Spec:** §7.7.3.1, p. 26 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`format_is_allowed`, check in `SessionEngine::handle_create`);
`crates/xrce-agent/src/daemon/config.rs` (key `representation_formats`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`,
`crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_representation_formats_can_be_widened`

**Status:** done — the normative reading (§7.7.3.1, Annex A) is the default: a CREATE in a
format that the union of the kind does not have is STATUS_ERR_INVALID_DATA
with a `CreateRefused` event. The other reading is the setting
`representation_formats: spec | any` (default `spec`); `any` admits all
formats, for example for the Micro XRCE-DDS client, which creates Publishers,
Subscribers, DataWriters and DataReaders by `*_ref` (not measured, the capture
has XML and binary only). Both readings are tested (bdf3f6ee966).

### §7.7.3.1 OBJK_Representation3Formats

**Spec:** §7.7.3.1, p. 26–27 — "union OBJK_Representation3Formats
switch(RepresentationFormat) { case REPRESENTATION_BY_REFERENCE :
string<REFERENCE_MAX_LEN> object_reference case
REPRESENTATION_AS_XML_STRING : string xml_string_representation; case
REPRESENTATION_IN_BINARY : sequence<octet> binary_representation; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant`,
`encode_with_pad`: length, characters and NUL for the strings, length
and octets for the sequence)

**Tests:** `crates/xrce/src/object_repr.rs::tests::topic_representation_matches_the_captured_create_bytes`,
`crates/xrce/src/object_repr.rs::tests::in_binary_roundtrip`

**Status:** done — the bound `REFERENCE_MAX_LEN` is an item of its own.

### §7.7.3.1 OBJK_RepresentationRefAndXMLFormats

**Spec:** §7.7.3.1, p. 27 — "union OBJK_RepresentationRefAndXMLFormats
switch(RepresentationFormat) { case REPRESENTATION_BY_REFERENCE :
string<REFERENCE_MAX_LEN> object_reference; case REPRESENTATION_AS_XML_STRING
: string string_representation; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::ByReference`,
`ByXmlString`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::object_variant_xml_form_supports_topic_qosprofile_application`

**Status:** done — the encoding of the two cases; that `IN_BINARY` must not
occur here is in the `open` item "Allowed formats per kind not checked".

### §7.7.3.1 OBJK_RepresentationBinAndXMLFormats

**Spec:** §7.7.3.1, p. 27 — "union OBJK_RepresentationBinAndXMLFormats
switch(RepresentationFormat) { case REPRESENTATION_IN_BINARY :
sequence<octet> binary_representation; case REPRESENTATION_AS_XML_STRING :
string string_representation; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::InBinary`,
`ByXmlString`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::subscriber_representation_matches_the_captured_create_bytes`

**Status:** done — as above for `BY_REFERENCE`.

### §7.7.3.1 Base types with the member representation

**Spec:** §7.7.3.1, p. 27 — "struct OBJK_RepresentationRefAndXML_Base {
OBJK_RepresentationRefAndXMLFormats representation; }; struct
OBJK_RepresentationBinAndXML_Base { OBJK_RepresentationBinAndXMLFormats
representation; }; struct OBJK_Representation3_Base {
OBJK_Representation3Formats representation; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`encode_for_kind`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`

**Status:** done

### §7.7.3.1 REFERENCE_MAX_LEN = 128

**Spec:** §7.7.3.1, p. 26 — "const long REFERENCE_MAX_LEN = 128;" with
`string<REFERENCE_MAX_LEN> object_reference`.

**Repo:** `crates/xrce/src/object_repr.rs` (`REFERENCE_MAX_LEN`, check in
`ObjectVariant::encode_with_pad` and `decode_with_pad`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::a_reference_longer_than_reference_max_len_is_refused`

**Status:** done — reading and writing refuse a reference of more than 128 characters; 128
characters are allowed (d96b535312c).

### §7.7.3.1 Further formats such as JSON

**Spec:** §7.7.3.1, p. 27–28 — "It is expected that additional
representations may be added after they are defined in other OMG
specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — outlook.

### §7.7.3.1.1 Reference to a description known to the agent

**Spec:** §7.7.3.1.1, p. 28 — "The REPRESENTATION_BY_REFERENCE represents the
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

**Status:** done — Participant, Topic, Publisher, Subscriber, DataWriter and DataReader
resolve against `profile_name` of a profile file in the Fast DDS format;
QosProfile (`Lib::Profile`), Application (`Lib::App`) and DomainParticipant
(`Lib::Participant`) against the libraries of the files and against the
definitions of other clients (dc63d65691f, 3aea52dd70a, b6e55cfa859). An
unknown reference is STATUS_ERR_UNKNOWN_REFERENCE. Publisher, Subscriber,
DataWriter and DataReader by reference exist only with
`representation_formats: any`. Type and Domain are in the items on §7.7.3.3.1
and §7.7.3.4.1.

### §7.7.3.1.1 object_reference as DDS-XML elementNameReference
**Spec:** §7.7.3.1.1, p. 28 — "The object_reference shall be a string
formatted as defined by the XSD simpleType elementNameReference defined in the
[DDS-XML] specification file dds-xml_domain_definitions_nonamespace.xsd."

**Repo:** `crates/xrce/src/object_repr.rs` (`element_name_reference`); `crates/xrce-agent/src/daemon/session.rs` (`handle_create`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::element_name_references_follow_the_xsd_pattern`, `crates/xrce-agent/src/daemon/session.rs::tests::an_object_reference_must_be_an_element_name_reference`

**Status:** done — the agent checks the `object_reference` of a CREATE as the XSD type `elementNameReference` (`dds-xml_qos_definitions_nonamespace.xsd`): `whiteSpace collapse`, then the pattern `((::)?[a-zA-Z0-9_.])+`. A string of another shape is STATUS_ERR_INVALID_DATA, a well-formed one the agent does not hold STATUS_ERR_UNKNOWN_REFERENCE; references of the form `Library::Name` are resolved against the libraries of DDS-XML (3aea52dd70a, dc63d65691f). a515c614d98.
### §7.7.3.1.1 Pre-configuration off-line or through the protocol

**Spec:** §7.7.3.1.1, p. 28 — "The pre-configuration may be done off-line
prior to starting the XRCE Agent or may be done on-line using the DDS-XRCE
protocol in combination with the REPRESENTATION_AS_XML_STRING."

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs` (`AgentProfiles::load`, `AgentProfiles::add_profiles`);
`crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::definition_to_register`, `rebuild_profiles`)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::the_eprosima_refs_file_indexes_every_entity_profile`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::types_and_qos_libraries_in_a_dds_document_are_read`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`

**Status:** done — off-line through `xml_profile_files:` (`<types>`, `<qos_library>`,
`<domain_library>`, `<domain_participant_library>`,
`<application_library>`; dc63d65691f). Through the protocol: a CREATE with the
XML of a QosProfile, Type, Application or DomainParticipant makes a definition
that other clients use by reference for as long as the object of the creating
client exists (3aea52dd70a).

### §7.7.3.1.1 Example of a reference

**Spec:** §7.7.3.1.1, p. 28 — "The following string is an example of an
object_reference used to represent an XRCE QosProfile:
'MyQosLibrary::MyQosProfile'."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example.

### §7.7.3.1.1 Kinds with a reference

**Spec:** §7.7.3.1.1, p. 28 — "This format is available for the XRCE Object
kinds that can be configured as libraries in the XRCE Agent. These are XRCE
Type, QosProfile, Domain, DomainParticipan, and Application."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`format_is_allowed`); `crates/xrce-agent/src/daemon/config.rs` (key
`representation_formats`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`,
`crates/xrce-agent/src/daemon/session.rs::tests::references_resolve_against_the_profile_files`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_representation_formats_can_be_widened`

**Status:** done — the default is the reading of Annex A: reference for Participant and Topic
(`OBJK_Representation3_Base`) and for Type, QosProfile and Application
(`OBJK_RepresentationRefAndXML_Base`); Publisher, Subscriber, DataWriter and
DataReader have binary and XML only. The spec contradicts itself:
OBJK_TOPIC_Representation has a reference (§7.7.3.7.1, p. 33), §8.4.10 and
§8.4.11 (p. 82–83) name it for DataWriter and DataReader as well. The other
reading is the setting `representation_formats: any` (bdf3f6ee966); both are
tested.

### §7.7.3.1.2 XML as in DDS-XML
**Spec:** §7.7.3.1.2, p. 28–29 — "The string shall contain an XML element
formatted according to the [DDS-XML] specification. The format of the string
is defined for each Object kind in clauses 7.7.3.2 to 7.7.3.11."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs`; `crates/xrce-agent/src/daemon/definitions.rs`

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_type_by_reference_names_a_known_type_by_its_fully_qualified_name`, `crates/xrce-agent/tests/spec_items_session.rs::a_subscriber_in_xml_has_its_qos_and_its_nested_data_reader`

**Status:** done — Type, QosProfile, Application, DomainParticipant (with the objects it holds), Topic, Publisher, Subscriber, DataWriter and DataReader are read as DDS-XML (3aea52dd70a, 9239562286c). A `<types>` may reference types already known (item on §7.7.3.3.2). Creating a Domain by CREATE is not defined by the specification (§7.7.3.4).
### §7.7.3.1.2 DDS-XML forms of entities
**Spec:** §7.7.3.1.2, p. 28–29 — as above.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint_from_xml`, `container_from_xml`, `topic_from_xml`, `apply_qos_profile`); `crates/xrce-agent/src/daemon/definitions.rs`

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_subscriber_in_xml_has_its_qos_and_its_nested_data_reader`, `crates/xrce-agent/tests/spec_items_session.rs::a_data_reader_in_xml_has_its_qos_and_its_content_filter`, `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_qos_of_a_topic_is_the_default_of_its_endpoints`

**Status:** done — read and tested: `register_type_ref`, `topic_ref` as the name of the topic, `<datawriter_qos>`/`<datareader_qos>` with `base_name`, `qos_profile_ref`, `<publisher_qos>`, `<subscriber_qos>`, `<topic_qos>`, `<domain_participant_qos>`, `<content_filter>` and the libraries `<qos_library>`, `<domain_library>`, `<domain_participant_library>`, `<application_library>` (3aea52dd70a, 9239562286c, 2c715803843, 0f85937a436).
### §7.7.3.1.2 XML for remote configuration

**Spec:** §7.7.3.1.2, p. 29 — "This format is intended for remotely
configuring the agent. Typically it will not be used by the XRCE Clients except
in deployments where the client-to-agent connection has sufficient
bandwidth."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — note. The example below it (p. 29) has
wrongly closed elements (`<reliability>`, `<data_reader_qos>`,
`<qos_library>`).

### §7.7.3.1.3 Binary as XCDR2 of the kind's structure

**Spec:** §7.7.3.1.3, p. 28 — "The REPRESENTATION_IN_BINARY represents objects
using a binary_representation octet sequence. The octet sequence is the result
of serializing an IDL-defined data-structure that depends on the kind of object
using the XCDR version 2 format defined in [DDS-XTYPES]."

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`participant`, `topic`, `container`, `endpoint`:
XCDR2 reader); `crates/xrce-agent/src/daemon/entity_repr.rs` (`Cdr`: the structures of the
eProsima Micro-XRCE-DDS client 3.0.1 as a further reading)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::annex_a_topic_has_a_dheader`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::the_36_octet_example_of_the_clause_is_a_topic`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_topic_names_its_topic_and_type`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_big_endian_binary_topic_reads_back`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_blob_of_no_form_is_refused_naming_each_reading`

**Status:** done — the `sequence<octet>` is read as XCDR2 of the structure of the kind, in
both forms of the spec (Annex A with a DHEADER, the clauses as FINAL); a blob
that is none of the forms is refused, naming each reading. The eProsima
structures (classic CDR) stay as a further reading (c67ce23c724).

### §7.7.3.1.3 Binary forms of §7.7.3 and Annex A

**Spec:** §7.7.3.1.3, p. 28 — as above.

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

**Status:** done — Participant, Topic, Publisher, Subscriber, DataWriter and DataReader are
read in the form of Annex A (DHEADER) and the form of the clauses:
`string topic_name` for DataWriter and DataReader, `TypeIdentifier` and
`type_reference` for the Topic, the binary form of the Participant
(c67ce23c724).

### §7.7.3.1.3 Example OBJK_Topic_Binary with 36 bytes

**Spec:** §7.7.3.1.3, p. 29 — topic "Square", `type_reference`
"MyTypes::ShapeType", 36 bytes little-endian.

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`topic`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`topic`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::the_36_octet_example_of_the_clause_is_a_topic`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_topic_names_its_topic_and_type`

**Status:** done — the test decodes the 36 bytes of the example as a topic with
`type_reference` "MyTypes::ShapeType". The characters of the example are
'S','q','u','e','r','e'; `binary_forms` reads them that way (92633850a97),
the text of the spec calls the topic "Square". In addition `entity_repr` reads
the same structure with "Square" as a 36-byte blob (c67ce23c724).

---

## §7.7.3.2 XRCE QosProfile

### §7.7.3.2 OBJK_QOSPROFILE_Representation

**Spec:** §7.7.3.2, p. 29 — "struct OBJK_QOSPROFILE_Representation :
OBJK_RepresentationRefAndXML_Base { };"

**Repo:** `crates/xrce/src/object_repr.rs` (`decode_for_kind`, no trailing
field for `ObjectKind::QosProfile`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`

**Status:** done — encoding; the effect is in the following items.

### §7.7.3.2.1 Reference to a QosProfile "Lib::Profile"

**Spec:** §7.7.3.2.1, p. 29 — "the object_reference field shall contain the
fully qualified name of a QosProfile known to the XRCE Agent. The fully
qualified name is composed of the name of the Qos library and the name of the
QosProfile within the library."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, branch OBJK_QOSPROFILE); `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::qos_profile`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`

**Status:** done — CREATE(`OBJK_QOSPROFILE`) with REPRESENTATION_BY_REFERENCE resolves
"Lib::Profile" against the QoS libraries of the files and of the definitions of
other clients; a name in no library is STATUS_ERR_UNKNOWN_REFERENCE
(3aea52dd70a).

### §7.7.3.2.2 XML with one qos_library and one qos_profile

**Spec:** §7.7.3.2.2, p. 29 — "the string_representation field shall contain a
single <qos_library> top-level XML element with the syntax defined by the XSD
complexType qosLibrary [...]. The <qos_library> element shall contain a single
<qos_profile> child element."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::definition_to_register`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`

**Status:** done — the `<qos_library>` with exactly one `<qos_profile>` is read and kept as
a definition; a library of two profiles is STATUS_ERR_INVALID_DATA
(9239562286c).

### §7.7.3.2.2 Reference to other QosProfiles

**Spec:** §7.7.3.2.2, p. 29–30 — "The REPRESENTATION_AS_XML_STRING
representation may reference other QoS profiles already known to the Agent.
[...] <qos_profile name="ModifiedProfile" base_name="MyQosLib:MyQosProfile">"

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::definition_to_register`); `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::qos_profile`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`

**Status:** done — `base_name` with one colon ("Q:One") is read like "Q::One"; the
profile inherits from the profile the agent knows (9239562286c). The example
writes `base_name` with one colon, §7.7.3.2.1 requires "::".

---

## §7.7.3.3 XRCE Type

### §7.7.3.3 OBJK_TYPE_Representation

**Spec:** §7.7.3.3, p. 30 — "struct OBJK_TYPE_Representation :
OBJK_RepresentationRefAndXML_Base { };"

**Repo:** `crates/xrce/src/object_repr.rs` (`decode_for_kind`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_binary_complete_type_object_is_named_by_its_own_type_name`

**Status:** done — encoding. `REPRESENTATION_IN_BINARY` for `OBJK_TYPE` is
STATUS_ERR_INVALID_DATA with the default `representation_formats: spec` (item
"Allowed formats per kind"); with `any`, `entity_repr::types` reads a
`TypeObject`.

### §7.7.3.3.1 Reference to a type with modules
**Spec:** §7.7.3.3.1, p. 30 — "the object_reference field shall contain the
fully qualified name of an XRCE Type known to the XRCE Agent. The fully
qualified name is composed of the name of the type prepended by the names of
the enclosing modules."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, `find_type_definition`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_type_by_reference_names_a_known_type_by_its_fully_qualified_name`

**Status:** done — CREATE(`OBJK_TYPE`) with REPRESENTATION_BY_REFERENCE resolves the name (`MyModule::ShapeType`) against the types of the client, of the files (`<types>`), of the definitions of other clients and of `type_files:`; an unknown name, also one without the module, is STATUS_ERR_UNKNOWN_REFERENCE (3aea52dd70a, 0f85937a436).
### §7.7.3.3.2 XML with types, one type with nested=false

**Spec:** §7.7.3.3.2, p. 30 — "the string_representation field shall contain a
single <types> top-level XML element [...]. Within the <types> element there
may be multiple types defined. In this case only one type shall have the
nested annotation (see [DDS-XTYPES]) set to false. This corresponds to the
XRCE Type being created."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`types`, `nested_flags`);
`crates/xrce-agent/src/daemon/session.rs` (`DdsCreate::Types`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_gets_the_type_its_type_name_names`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_xml_type_library_compiles_to_named_complete_types`

**Status:** done — `<types>` is read as DDS-XML and compiled; all named types are
registered. With several types exactly one must not carry `nested`, otherwise
STATUS_ERR_INVALID_DATA (9239562286c). A topic finds its type by name
(`SessionEngine::resolve_type`).

### §7.7.3.3.2 Annotation nested

**Spec:** §7.7.3.3.2, p. 30 — as above.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`types`, `nested_flags`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`

**Status:** done — `nested_flags` reads the annotation per type (also inside modules); two
types without `nested="true"` are STATUS_ERR_INVALID_DATA, one without and one
with `nested="true"` is STATUS_OK (9239562286c).

### §7.7.3.3.2 Reference to known types
**Spec:** §7.7.3.3.2, p. 30 — "The string_representation may reference other
Types already known to the Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`types`); `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, `ObjectKind::Type`); `crates/xml/src/typeobject_bridge.rs` (`bridge_library_complete_with_known`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_types_string_may_reference_types_the_agent_knows`, `crates/xrce-agent/src/daemon/session.rs::tests::a_types_string_references_a_type_created_before`

**Status:** done — `types` passes the types known to the agent to `zerodds_xml::bridge_library_complete_with_known(lib, known)`: those the same client created, those of `type_files:` and those of the profile files. A reference to one of them becomes its `EquivalenceHashComplete`; the known type is not defined again. A name that is neither in the string nor known is STATUS_ERR_UNKNOWN_REFERENCE. Not checked: whether a reference succeeds to a type another client created in binary (not as XML `<types>`); the types of XML `<types>` belong to the definitions of the agent (§9.2).

---

## §7.7.3.4 XRCE Domain

### §7.7.3.4 OBJK_DOMAIN_Representation
**Spec:** §7.7.3.4, p. 30–31 — "struct OBJK_DOMAIN_Representation :
OBJK_RepresentationRefAndXML_Base { };"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the specification gives Domain no value of `ObjectKind` and no case in `ObjectVariant` (§7.7.5, p. 39; Annex A, p. 103 and p. 111): a CREATE cannot name a Domain, there is nothing to fulfil. Kind 0x0F is `OBJK_OTHER` (f63fa2d1218). Domains come from the `<domain_library>` of the files and can be referenced as `Library::Domain` (`domain_ref`, `domain_reference`).
### §7.7.3.4.1 Reference to a domain "Lib::Domain"
**Spec:** §7.7.3.4.1, p. 31 — "the object_reference field shall contain the
fully qualified name of an XRCE Domain definition known to the Agent."

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`domain`, `domain_scope`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_domain_reference_is_resolved_against_the_domain_libraries`

**Status:** done — a Domain can be referenced as "Lib::Domain": `domain_ref` of the DomainParticipant in XML and `domain_reference` of the binary form are resolved against the `<domain_library>` of the files (dc63d65691f, b4cd825c268). It cannot be the `object_reference` of a CREATE because the specification gives the Domain no ObjectKind (§7.7.3.4).
### §7.7.3.4.2 XML with domain_library and one domain
**Spec:** §7.7.3.4.2, p. 31 — "The XML shall contain a single
<domain_library> top-level XML element [...]. The <domain_library> element
shall contain a single <domain> child element. [...] The string_representation
may reference Types already known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`add_domain_library`)

**Tests:** `crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`

**Status:** done — the `<domain_library>` with its `<domain>` elements is read from the files (also as a root; `base_name`, `register_type`, `topic`; dc63d65691f). A Domain as the content of a CREATE is not defined by the specification (§7.7.3.4).

---

## §7.7.3.5 XRCE Application

### §7.7.3.5 OBJK_APPLICATION_Representation

**Spec:** §7.7.3.5, p. 31 — "The OBJK_TYPE_Representation supports the
REPRESENTATION_BY_REFERENCE and REPRESENTATION_AS_XML_STRING formats. [...]
struct OBJK_APPLICATION_Representation : OBJK_RepresentationRefAndXML_Base {
};"

**Repo:** `crates/xrce/src/object_repr.rs` (`decode_for_kind`);
`crates/xrce-agent/src/lib.rs` (`XrceAgent::create_object`)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::create_application_object_via_objk_application`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`

**Status:** done — encoding and storage. The sentence names `OBJK_TYPE_Representation`
instead of `OBJK_APPLICATION_Representation`. A binary CREATE is
STATUS_ERR_INVALID_DATA (item "Allowed formats per kind").

### §7.7.3.5.1 Reference to an application "Lib::App"

**Spec:** §7.7.3.5.1, p. 31 — "the object_reference field shall contain the
fully qualified name of an XRCE Application definition known to the Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`application_definition`); `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::application`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`

**Status:** done — "Lib::App" is resolved against the `<application_library>` of the files
and of the definitions of other clients; the CREATE makes the DomainParticipant
of the Application with everything inside it. An unknown Application is
STATUS_ERR_UNKNOWN_REFERENCE (3aea52dd70a, b6e55cfa859).

### §7.7.3.5.2 XML with application_library and one application

**Spec:** §7.7.3.5.2, p. 31–32 — "The XML shall contain a single
<application_library> top-level XML element [...]. This element shall contain
a single <application> child element. [...] The string_representation may
reference XRCE Types, Qos Profiles, Domains, or DomainParticipants already
known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`application_definition`); `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::add_application_library`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::what_cannot_be_planned_is_refused`

**Status:** done — the `<application_library>` with one `<application>` is read; its
Participants refer to types, QoS profiles, Domains and DomainParticipants of
the definitions (3aea52dd70a). The example (p. 32) creates in the Subscriber a
`<data_writer name="MyReader" topic_ref="Circle">` although only "Square" is
defined; the code refuses a `topic_ref` to a topic the Participant does not
have.

---

## §7.7.3.6 XRCE DomainParticipant

### §7.7.3.6 OBJK_PARTICIPANT_Representation with domain_id

**Spec:** §7.7.3.6, p. 32 — "struct OBJK_PARTICIPANT_Representation :
OBJK_Representation3_Base { short domain_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`encode_for_kind`:
alignment to 2 before `domain_id`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::participant_representation_matches_the_captured_create_bytes`

**Status:** done

### §7.7.3.6.1 Reference to a DomainParticipant

**Spec:** §7.7.3.6.1, p. 32 — "the object_reference field shall contain the
fully qualified name of an XRCE DomainParticipant definition known to the
Agent. The fully qualified name is composed of the name of the
DomainParticipant library and the name of the DomainParticipant within the
library."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`, `participant_definition`); `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::participant`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_reference_is_read_like_the_profile_it_names`

**Status:** done — the reference resolves against `<participant profile_name>` of a profile
file and against the `<domain_participant_library>` ("Lib::Participant", for an
Application "Lib::App::Participant"); the domain comes from the definition
(`domain_id` or `domain_ref`) or from the profile (3aea52dd70a, b6e55cfa859).

### §7.7.3.6.2 XML with domain_participant_library

**Spec:** §7.7.3.6.2, p. 32 — "the string_representation field shall contain a
single <domain_participant_library> top-level XML element [...]. This element
shall contain a single <domain_participant> child element. [...] The
string_representation may reference XRCE Types, Qos Profiles, Domains, or
DomainParticipants already known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant_definition`, `participant_source`);
`crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions::add_participant_library`, `plan_participant`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_creates_the_objects_inside_it`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_with_a_broken_part_creates_nothing`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`

**Status:** done — the `<domain_participant_library>` with one `<domain_participant>` is
read; the CREATE also makes the contained topics, publishers, subscribers and
endpoints (ObjectId from the name) and enables them with the Participant. If
a part fails nothing is left (3aea52dd70a).

### §7.7.3.6.3 OBJK_DomainParticipant_Binary

**Spec:** §7.7.3.6.3, p. 33 — "struct OBJK_DomainParticipant_Binary { long
domain_id; @optional string<128> domain_reference; @optional string<128>
qos_profile_reference; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`participant`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::participant_binary_in_both_forms`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_participant_is_read`

**Status:** done — three readings of the blob: Annex A (DHEADER, without `domain_id`), the
form of the clauses (`long domain_id` first, FINAL) and the two octets `00 00`
of the eProsima client. `domain_id` of the binary form and that of the
representation (`short`) must agree, otherwise STATUS_ERR_INVALID_DATA
(c67ce23c724).

### §7.7.3.6.3 domain_reference
**Spec:** §7.7.3.6.3, p. 33 — "Any XRCE Topic and Type definitions contained in
the referenced domain are considered defined within the scope of the XRCE
DomainParticipant and become available as references to construct XRCE
objects contained by the DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`); `crates/xrce-agent/src/daemon/definitions.rs` (`domain_scope`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_domain_reference_is_resolved_against_the_domain_libraries`

**Status:** done — `domain_reference` is resolved against the Domains of the `<domain_library>`; the `domain_id` of the Domain is the participant's and must agree with the `long domain_id` of the binary form; a name in no library is STATUS_ERR_UNKNOWN_REFERENCE (b4cd825c268). The Domain's topics and types are in the participant's scope (4a57a1fa4ce, the next item).
### §7.7.3.6.3 Topics and types of the Domain with the binary form
**Spec:** §7.7.3.6.3, p. 33 — as above.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`ParticipantDescription::scope`); `crates/xrce-agent/src/daemon/session.rs` (`participant_scopes`); `crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions::domain_scope`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_domain_reference_is_resolved_against_the_domain_libraries`

**Status:** done — with the binary form and a `domain_reference` the Domain's `register_type` and topics are in the participant's scope as with the XML participant with `domain_ref`; a topic of the participant finds them by name.
### §7.7.3.6.3 qos_profile_reference

**Spec:** §7.7.3.6.3, p. 33 — "If specified, the corresponding DDS
DomainPaticipant shall be created using that Qos. Otherwise, the
DomainPaticipant shall be created using the DDS default Qos."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`participant`, `participant_qos_of`); `crates/xrce-agent/src/daemon/dds_port.rs`
(`DdsPort::create_participant_with`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_participant_qos_profile_reference_is_applied`

**Status:** done — the participant QoS of the profile (`<domain_participant_qos>`) applies per
Participant: ENTITY_FACTORY by the Participant (with `autoenable_created_entities`
false its Publishers, Subscribers and Topics stay disabled), USER_DATA through a
DCPS runtime of its own per domain and value. Another policy in the
participant QoS is not in the XSD of DDS-XML 1.0: `crates/xml` refuses the file
when it is read (test `a_participant_policy_outside_the_schema_is_refused`); if a
resolved profile carries one anyway it is STATUS_ERR_INCOMPATIBLE. There is no
policy the schema allows in the participant QoS that the agent cannot apply
(USER_DATA and ENTITY_FACTORY are both applied). A profile without participant QoS
applies nothing; without a reference the DDS default QoS applies (b4cd825c268).

---

## §7.7.3.7 XRCE Topic

### §7.7.3.7 OBJK_TOPIC_Representation with participant_id

**Spec:** §7.7.3.7, p. 33 — "struct OBJK_TOPIC_Representation :
OBJK_Representation3_Base { ObjectId participant_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`,
`ObjectVariantExtra::parent`: `ObjectId` without alignment behind the variant)

**Tests:** `crates/xrce/src/object_repr.rs::tests::topic_representation_matches_the_captured_create_bytes`,
`crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`

**Status:** done

### §7.7.3.7 The topic belongs to the DomainParticipant

**Spec:** §7.7.3.7, p. 33 — "Independent of the representation format, the
field participant_id shall contain the ObjectId of an XRCE DomainParticipant
object. The referenced or created Topic will belong to the specified
DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::plan_dds_create`,
branch `ObjectKind::Topic`; `participant_of_create`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_is_created_by_name_or_by_register_type_ref`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::null_port_rejects_topic_before_participant`

**Status:** done — kind and existence of the participant are checked (wrong
kind: STATUS_ERR_INVALID_DATA, missing participant:
STATUS_ERR_UNKNOWN_REFERENCE), the topic is created with it. Tested are the
creation under the participant and the order at port level; there is no test of
the failure path of a topic at session level (dd55e9c9df1).

### §7.7.3.7.1 Reference to the name of a topic in the participant

**Spec:** §7.7.3.7.1, p. 33 — "the object_reference field shall contain the
bare name of an XRCE Topic defined in XRCE DomainParticipant identified by the
participant_id. The Topic could be defined directly on the XRCE
DomainParticipant, or else in the XRCE Domain associated with the
DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`topic`, branch BY_REFERENCE:
`ParticipantScope::topic`, then `<topic profile_name>` of the profile files);
`crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions::scope_of`:
topics of the participant and of its Domain)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_topic_is_created_by_name_or_by_register_type_ref`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_reference_is_read_like_the_profile_it_names`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`

**Status:** done — the name is resolved against the topics of the participant
and of its Domain; a name that neither has and the profile files do not have
gives STATUS_ERR_UNKNOWN_REFERENCE. The CREATE test names a topic of the
participant; the topics of the Domain are checked by the test of
`definitions.rs` on the planning of a participant (3aea52dd70a).

### §7.7.3.7.2 XML with one topic
**Spec:** §7.7.3.7.2, p. 34 — "the string_representation field shall contain a
single <topic> top-level XML element with the syntax defined by the XSD
complexType topic [...]. <topic name="Square" register_type_ref="ShapeType"
/> The string_representation may reference XRCE Types or QosProfiles already
known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`topic_from_xml`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_qos_of_a_topic_is_the_default_of_its_endpoints`

**Status:** done — `<topic name=… register_type_ref=…/>` (the example of the spec) is read: the registration is looked up in the participant and its Domain, its type among the types the agent knows; a registration the participant does not have gives STATUS_ERR_UNKNOWN_REFERENCE. The attribute form (`type_name`) and the `<dataType>` of Fast DDS stay readable. The QoS and the QosProfile references of the topic are applied (next item). 3aea52dd70a.
### §7.7.3.7.2 QoS of a topic and QosProfile references
**Spec:** §7.7.3.7.2, p. 34 — "The string_representation may reference XRCE Types or
QosProfiles already known to the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`TopicQos`, `topic_from_xml`, `endpoint_for_topic`); `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`)

**Tests:** `crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_qos_of_a_topic_is_the_default_of_its_endpoints`

**Status:** done — `<topic_qos base_name=…>` and `qos_profile`/`qos_profile_ref` of the `<topic>` are read; what the topic states is the default of the endpoints created for it (DDS 1.4 §2.2.2.3.1), the endpoint's own representation wins; TOPIC_DATA is part of it. 2c715803843.
### §7.7.3.7.3 OBJK_Topic_Binary

**Spec:** §7.7.3.7.3, p. 34 — "struct OBJK_Topic_Binary { string<256>
topic_name; @optional string<256> type_reference; @optional
DDS:XTypes::TypeIdentifier type_identifier; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`topic`, `TopicBinary`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`topic`, branch `InBinary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::the_36_octet_example_of_the_clause_is_a_topic`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::annex_a_topic_has_a_dheader`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::string_bounds_are_checked`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_topic_names_its_topic_and_type`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_topic_names_its_topic_and_type`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_big_endian_binary_topic_reads_back`

**Status:** done — XCDR2 per §7.7.3.1.3: `topic_name`, optional
`type_reference`, optional `type_identifier` (`TypeIdentifier`), `string<256>`
checked. The spec gives the structure twice: Annex A as APPENDABLE with a
DHEADER, the clause as FINAL without. Both forms are read (Annex A first, the
form of the blob decides, no setting), then the structure of the eProsima
client. The 36 octets of the clause's example spell "Squere" (the text:
"Square"); the test expects the octets. c67ce23c724, 92633850a97.

### §7.7.3.7.3 type_identifier

**Spec:** §7.7.3.7.3, p. 34 — "The type_identifier, if present, shall contain
the DDS-XTYPES TypeIdentifier for the data-type. See clause 7.3.2 of
[DDS-XTYPES]."

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`topic`:
`TypeIdentifier::decode_from`); `crates/xrce-agent/src/daemon/session.rs`
(`type_name_of`, `plan_dds_create`, branch Topic)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::a_topic_with_a_type_identifier_reads_it_and_one_without_a_type_is_refused`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::string_bounds_are_checked`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_topic_may_name_its_type_by_identifier`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_can_name_its_type_by_type_identifier`

**Status:** done — a present `type_identifier` is read as the XTypes
`TypeIdentifier` and compared with the COMPLETE identifiers of the types known
to the agent (`OBJK_TYPE` of the client, `type_files:`); an identifier without a
match gives STATUS_ERR_UNKNOWN_REFERENCE. The bounds `string<256>` are checked.
c67ce23c724.

### §7.7.3.7.3 Not both missing, type_identifier takes precedence
**Spec:** §7.7.3.7.3, p. 34 — "Either type_reference or type_identifier may be
used to identify the XRCE Type associated with the Topic. Either member may be
omitted, but not both. If both are present the type_identifier shall take
precedence."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`); `crates/xrce-agent/src/daemon/binary_forms.rs` (`topic`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_binary_topic_takes_its_type_from_the_type_identifier_and_needs_one_of_the_two`

**Status:** done — if both members are missing this is STATUS_ERR_INVALID_DATA (f87b3c3d0b2, c67ce23c724); with `type_identifier` alone the type is found; if both are there the identifier takes precedence (0f85937a436).
### §7.7.3.7.3 type_reference to a known type

**Spec:** §7.7.3.7.3, p. 34 — "The type_reference, if present, shall contain
the fully qualified name of the type [...]. The referenced type shall be known
to the XRCE Agent either via pre-configuration, or as a result of a prior
create operation executed on the XRCE ProxyClient; see 7.8.3.1."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::resolve_type`,
`find_type_definition`, `apply_dds_create`, branch Topic)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_topic_gets_the_type_its_type_name_names`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_configured_type_is_known_to_every_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_of_an_unknown_type_is_refused_when_the_agent_is_told_to`

**Status:** done — the type is looked up first among the `OBJK_TYPE` of the
client (a prior CREATE), then in `type_files:` (pre-configuration); a topic gets
the definition of the type its name names (test with two types of the client).
An unknown type gives STATUS_ERR_UNKNOWN_REFERENCE (item below). e4d37a6aff6.

### §7.7.3.7.3 A topic with an unknown type

**Spec:** §7.7.3.7.3, p. 34 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`apply_dds_create`, branch Topic;
`SessionEngineConfig::forward_unknown_topic_types`);
`crates/xrce-agent/src/daemon/config.rs` (key `unknown_topic_types`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_topic_of_an_unknown_type_is_refused_when_the_agent_is_told_to`,
`crates/xrce-agent/src/daemon/config.rs::tests::unknown_topic_types_are_refused_unless_forwarding_is_named`

**Status:** done — the default is the spec: a topic whose type the agent does
not know gives STATUS_ERR_UNKNOWN_REFERENCE, nothing is stored. Named setting:
agent key `unknown_topic_types: forward` creates the topic and reports
`SessionEvent::UnknownTypeForwarding`; samples then go out without a key and
without `TypeInformation` (the eProsima client names types the agent does not
hold). e4d37a6aff6, 0da6aef56b5.

---

## §7.7.3.8 XRCE Publisher

### §7.7.3.8 OBJK_PUBLISHER_Representation with participant_id

**Spec:** §7.7.3.8, p. 34 — "struct OBJK_PUBLISHER_Representation :
OBJK_RepresentationBinAndXML_Base { ObjectId participant_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`,
`ObjectVariantExtra::parent`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`

**Status:** done — encoding; the check is in the next item.

### §7.7.3.8 The publisher belongs to the DomainParticipant

**Spec:** §7.7.3.8, p. 34 — "the member participant_id shall contain the
ObjectId of an XRCE DomainParticipant object. The referenced or created
Publisher shall belong to the specified DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, branch
Publisher/Subscriber; `participant_of_create`; `apply_dds_create`:
`DdsPort::create_publisher`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_or_subscriber_needs_an_existing_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_and_a_subscriber_get_a_dds_entity_with_their_policies`

**Status:** done — kind and existence of the participant are checked (missing
participant: STATUS_ERR_UNKNOWN_REFERENCE, other kind: STATUS_ERR_INVALID_DATA,
nothing is stored); the publisher is created as a DDS Publisher under the
participant. dd55e9c9df1.

### §7.7.3.8.1 XML with one publisher

**Spec:** §7.7.3.8.1, p. 34–35 — "the string_representation field shall
contain a single <publisher> top-level XML element [...]. Note that the XML
representation of a Publisher allows specifying Qos policies and including
nested DataWriter objects. [...] Any referenced object must have been
previously created or configured on the XRCE Agent."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`container`,
`container_from_xml`: `<publisher_qos>`, `<qos>`, nested `<data_writer>`);
`crates/xrce-agent/src/daemon/session.rs` (`create_embedded`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_empty_container_xml_is_a_default_container`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_container_reads_presentation_and_entity_factory`

**Status:** done — `<publisher>` with `<publisher_qos base_name=…>` (PARTITION,
GROUP_DATA, PRESENTATION, ENTITY_FACTORY are applied) and the Fast DDS `<qos>`;
policies a Publisher does not have are named in `not_applied`. The example (p.
35) closes `<publisher name="MyPublisher"/>` before its child elements.
3aea52dd70a, 9239562286c.

### §7.7.3.8.1 publisher_qos and nested DataWriters

**Spec:** §7.7.3.8.1, p. 35 — as above.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`container_from_xml`:
`ContainerDescription::embedded`); `crates/xrce-agent/src/daemon/session.rs`
(`create_embedded`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`

**Status:** done — nested `<data_writer>` elements are created under the
publisher with the ObjectId of their name (§9.3); if one fails (in the test: a
topic the participant does not have, STATUS_ERR_UNKNOWN_REFERENCE), the CREATE
of the publisher fails and nothing is left. 9239562286c.

### §7.7.3.8.2 OBJK_Publisher_Binary

**Spec:** §7.7.3.8.2, p. 35 — "struct OBJK_PUBLISHER_QosBinary { @optional
sequence<string> partitions; @optional sequence<octet> group_data; }; struct
OBJK_Publisher_Binary { @optional string publisher_name; @optional
OBJK_PUBLISHER_QosBinary qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`container`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`container`, branch `InBinary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::publisher_binary_of_annex_a`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_publisher_gives_its_partitions`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_publisher_has_default_qos`

**Status:** done — `partitions` and `group_data` are applied, `publisher_name`
is read and not used. Annex A (APPENDABLE, DHEADER) is read first, then the form
of the clause without a DHEADER, which the eProsima client sends as well.
c67ce23c724.

---

## §7.7.3.9 XRCE Subscriber

### §7.7.3.9 OBJK_SUBSCRIBER_Representation with participant_id

**Spec:** §7.7.3.9, p. 35 — "struct OBJK_SUBSCRIBER_Representation :
OBJK_RepresentationBinAndXML_Base { ObjectId participant_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::subscriber_representation_matches_the_captured_create_bytes`

**Status:** done — encoding.

### §7.7.3.9 The subscriber belongs to the DomainParticipant

**Spec:** §7.7.3.9, p. 35 — "the member participant_id shall contain the
ObjectId of an XRCE DomainParticipant object. The referenced or created
Subscriber shall belong to the specified DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, branch
Publisher/Subscriber; `participant_of_create`; `apply_dds_create`:
`DdsPort::create_subscriber`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_or_subscriber_needs_an_existing_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_and_a_subscriber_get_a_dds_entity_with_their_policies`

**Status:** done — as for the Publisher (the same branch, both kinds in the
tests). dd55e9c9df1.

### §7.7.3.9.1 XML with one subscriber
**Spec:** §7.7.3.9.1, p. 35–36 — "the string_representation field shall
contain a single <subscriber> top-level XML element [...]. Note that the XML
representation of a Subscriber allows specifying Qos policies and including
nested DataReader objects."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`container_from_xml`); `crates/xrce-agent/src/daemon/session.rs` (`create_embedded`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_subscriber_in_xml_has_its_qos_and_its_nested_data_reader`

**Status:** done — the code reads `<subscriber>` in the same branches as `<publisher>` (3aea52dd70a); the test creates a `<subscriber>` with `<subscriber_qos>` and content (0f85937a436).
### §7.7.3.9.1 subscriber_qos and nested DataReaders
**Spec:** §7.7.3.9.1, p. 36 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_embedded`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`container_from_xml`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_subscriber_in_xml_has_its_qos_and_its_nested_data_reader`

**Status:** done — the code creates the `<data_reader>` elements of a `<subscriber>` as the `<data_writer>` elements of a `<publisher>` (9239562286c); the test creates a subscriber with a nested `<data_reader>` by CREATE and checks the partition and the reader (0f85937a436).
### §7.7.3.9.2 OBJK_Subscriber_Binary

**Spec:** §7.7.3.9.2, p. 36 — "struct OBJK_SUBSCRIBER_QosBinary { @optional
sequence<string> partitions; @optional sequence<octet> group_data; }; struct
OBJK_Subscriber_Binary { @optional string subscriber_name; @optional
OBJK_SUBSCRIBER_QosBinary qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`container`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`container`, the same reading
routine as for the Publisher)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::publisher_binary_of_annex_a`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_publisher_gives_its_partitions`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_publisher_has_default_qos`

**Status:** done — the tests read the form of the Publisher, the reading routine
is the same. c67ce23c724.

---

## §7.7.3.10 XRCE DataWriter

### §7.7.3.10 DATAWRITER_Representation with publisher_id

**Spec:** §7.7.3.10, p. 36 — "struct DATAWRITER_Representation :
OBJK_RepresentationBinAndXML_Base { ObjectId publisher_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`,
`crates/xrce/src/object_repr.rs::tests::annex_b_2_3_writer_variant_reads_as_the_blob_and_its_publisher`

**Status:** done — encoding.

### §7.7.3.10 The DataWriter belongs to the publisher

**Spec:** §7.7.3.10, p. 36 — "the member publisher_id shall contain the
ObjectId of an XRCE Publisher object. The referenced or created DataWriter
shall belong to the specified Publisher."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, branch
DataWriter/DataReader)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_endpoint_and_its_topic_belong_to_one_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`

**Status:** done — kind and existence of the publisher are checked (missing
publisher: STATUS_ERR_UNKNOWN_REFERENCE), DataWriter and topic must belong to
one participant; the DataWriter inherits PARTITION and GROUP_DATA of the
publisher.

### §7.7.3.10.1 XML with one data_writer

**Spec:** §7.7.3.10.1, p. 36–37 — "the string_representation field shall
contain a single <data_writer> top-level XML element [...]. <data_writer
name="MySquareWriter" topic_ref="Square"/> The referenced Topic must have been
previously created or configured on the XRCE DomainParticipant [...]. The XML
representation of a DataWriter allows specifying Qos policies."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`,
`endpoint_from_xml`: attribute form, Fast DDS form, `topic_ref` as a name,
`<datawriter_qos>` / `<data_writer_qos>`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_data_writer_states_its_qos_over_a_profile`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_attribute_form_still_reads`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_eprosima_element_form_names_topic_type_and_kind`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_qos_profile_reference_applies_the_profile`

**Status:** done — read are the DDS-XML form (`<data_writer name=…
topic_ref=…>`, `<datawriter_qos base_name=…>`), the attribute form (`topic_ref`
as an ObjectId in hex, `qos_profile`, `reliability`) and the Fast DDS form. A
DataWriter has no BY_REFERENCE representation per §7.7.3.1; the agent key
`representation_formats: any` admits it (the eProsima client uses it).
3aea52dd70a, bdf3f6ee966.

### §7.7.3.10.1 topic_ref as a name and data_writer_qos

**Spec:** §7.7.3.10.1, p. 36–37 — as above.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint_from_xml`: `topic_ref`,
`datawriter_qos` / `data_writer_qos`); `crates/xrce-agent/src/daemon/session.rs`
(`resolve_topic`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_data_writer_states_its_qos_over_a_profile`

**Status:** done — `topic_ref="Square"` is resolved as the name of a topic of
the participant (`0x…` stays the ObjectId); `<datawriter_qos base_name=…>`
applies the profile, policies of the element go over it. The code reads the
spelling `<data_writer_qos>` of the clause as well; the tests use
`<datawriter_qos>`. The example (p. 36) names the container "XRCE Susbcriber".
3aea52dd70a.

### §7.7.3.10.2 OBJK_DataWriter_Binary

**Spec:** §7.7.3.10.2, p. 37 — "struct OBJK_DataWriter_Binary { string
topic_name; OBJK_Endpoint_QosBinary endpoint_qos; @optional unsigned long
ownership_strength; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint`, `endpoint_qos`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`, branch `InBinary`,
`apply_endpoint_binary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_writer_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_writer_is_reliable_keep_last_transient_local`,
`crates/xrce-agent/src/daemon/session.rs::tests::annex_b_2_3_create_of_a_binary_data_writer`,
`crates/xrce-agent/src/daemon/session.rs::tests::annex_b_2_4_create_of_a_binary_data_writer_with_qos`

**Status:** done — `topic_name`, `endpoint_qos` and `ownership_strength`
(`unsigned long`). Annex A (APPENDABLE, `qos` optional) and the clause (FINAL,
`endpoint_qos` mandatory) are both read, Annex A first, the form of the blob
decides (no setting); then the structure of the eProsima client (`ObjectId
topic_id`, `ownership_strength` with 64 bits). The Annex B.2.3 and B.2.4
examples are tested byte by byte. The form of the clause is not tested for the
DataWriter on its own, it is for the DataReader (item below). c67ce23c724,
33df0df0945.

### §7.7.3.10.2 EndpointQosFlags

**Spec:** §7.7.3.10.2, p. 37 — "@bit_bound(16) bitmask EndpointQosFlags {
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

**Status:** done — bit 1 is `is_history_keep_last` (clause, eProsima client) or
`is_history_keep_all` (Annex A, p. 110) depending on the form; the form of the
blob decides (`EndpointBinary::bit1_is_keep_all`). The tests check both
readings. c67ce23c724.

### §7.7.3.10.2 OBJK_Endpoint_QosBinary

**Spec:** §7.7.3.10.2, p. 37 — "struct OBJK_Endpoint_QosBinary {
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
`lifespan_msec` and `user_data` (all but the flags optional). c67ce23c724.

---

## §7.7.3.11 XRCE DataReader

### §7.7.3.11 DATAREADER_Representation with subscriber_id

**Spec:** §7.7.3.11, p. 38 — "struct DATAREADER_Representation :
OBJK_RepresentationBinAndXML_Base { ObjectId subscriber_id; };"

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field`,
`crates/xrce/src/object_repr.rs::tests::annex_b_2_6_reader_with_default_qos`

**Status:** done — encoding.

### §7.7.3.11 The DataReader belongs to the subscriber

**Spec:** §7.7.3.11, p. 38 — "the member subscriber_id shall contain the
ObjectId of an XRCE Subscriber object. The referenced or created DataReader
will belong to the specified Subscriber."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, branch
DataWriter/DataReader)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_reads_through_a_preconfigured_data_reader_without_a_create`

**Status:** done — kind and existence of the subscriber are checked; the
DataReader inherits PARTITION and GROUP_DATA of the subscriber.

### §7.7.3.11.1 XML with one data_reader and filters
**Spec:** §7.7.3.11.1, p. 38 — "the string_representation field shall contain
a single <data_reader> top-level XML element [...]. The XML representation of
a DataReader allows specifying Qos policies. [...] The XML representation of a
DataReader may also contain time-based and content-based filters."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint_from_xml`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_data_reader_in_xml_has_its_qos_and_its_content_filter`

**Status:** done — tested are the attribute form, the Fast DDS form with `timeBasedFilter`, a `<data_reader topic_ref=…>` from the configuration file and `<data_reader_qos>` with `<content_filter>` (0f85937a436). A DataReader has no BY_REFERENCE representation by §7.7.3.1; `representation_formats: any` admits it.
### §7.7.3.11.1 data_reader_qos and content_filter
**Spec:** §7.7.3.11.1, p. 38 — "<content_filter name="MyFilter">
<expression> x &gt; 5 </expression> </content_filter>"

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint_from_xml`, `apply_qos_profile`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_data_reader_in_xml_has_its_qos_and_its_content_filter`

**Status:** done — `<content_filter><expression>` sets the reader's filter (the attribute `name` is not used), `<datareader_qos>`/`<data_reader_qos>` apply profile and own policies; the test reads both from XML and checks QoS and filter at the DDS port (0f85937a436, 3aea52dd70a).
### §7.7.3.11.2 OBJK_DataReader_Binary

**Spec:** §7.7.3.11.2, p. 38–39 — "struct OBJK_DataReader_Binary { string
topic_name; OBJK_Endpoint_QosBinary endpoint_qos; @optional unsigned long
timebasedfilter_msec; @optional string contentbased_filter; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint`, `endpoint_qos`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`, branch `InBinary`,
`apply_endpoint_binary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::reader_binary_of_the_clause`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_clause_binary_reader_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_reader_is_reliable_keep_last_transient_local`

**Status:** done — `topic_name`, `endpoint_qos`, `timebasedfilter_msec` and
`contentbased_filter` (applied as the filter of the reader). The form of the
clause and Annex A are read as for the DataWriter; Annex B.2.6 and B.2.7 are
tested byte by byte. In example B.2.7 the text says five minutes, the table
180000 ms; the octets are read. c67ce23c724, 33df0df0945.

---

## §7.7.4 ObjectId

### §7.7.4 ObjectId as octet[2] per client and agent

**Spec:** §7.7.4, p. 39 — "Each ObjectId is scoped to an XRCE Client and Agent
pair. [...] typedef octet ObjectId [2];"

**Repo:** `crates/xrce/src/object_id.rs` (`ObjectId::to_bytes`, `ObjectId::from_bytes`)

**Tests:** `crates/xrce/src/object_id.rs::tests::bytes_are_big_endian`,
`crates/xrce/src/object_info.rs::tests::base_object_request_roundtrip`

**Status:** done — the two octets in order, independent of the E flag.

---

## §7.7.5 ObjectKind

### §7.7.5 Values of ObjectKind

**Spec:** §7.7.5, p. 39 — "XRCE objects are classified into 14 kinds. [...]
typedef octet ObjectKind; const ObjectKind OBJK_INVALID = 0x00; [...]
OBJK_PARTICIPANT = 0x01; OBJK_TOPIC = 0x02; OBJK_PUBLISHER = 0x03;
OBJK_SUBSCRIBER = 0x04; OBJK_DATAWRITER = 0x05; OBJK_DATAREADER = 0x06;
OBJK_TYPE = 0x0A; OBJK_QOSPROFILE = 0x0B; OBJK_APPLICATION = 0x0C; OBJK_AGENT =
0x0D; OBJK_CLIENT = 0x0E;"

**Repo:** `crates/xrce/src/object_kind.rs` (constants `OBJK_*`, `ObjectKind::from_u8`)

**Tests:** `crates/xrce/src/object_kind.rs::tests::all_spec_kinds_roundtrip`,
`crates/xrce/src/object_kind.rs::tests::raw_const_values_match_spec`,
`crates/xrce/src/object_kind.rs::tests::unknown_byte_rejected`

**Status:** done — 13 values: the twelve of the text and `OBJK_OTHER = 0x0F`
(Annex A). The text names 14 kinds and lists 12. f63fa2d1218.

---

## §7.7.6 ObjectIdPrefix

### §7.7.6 ObjectId from prefix and ObjectKind

**Spec:** §7.7.6, p. 39 — "The ObjectId of an object is composed combining 12
bits from the ObjectIdPrefix and four bits from the ObjectKind. [...] typedef
octet ObjectIdPrefix [2]; [...] object_id[0] = objectid_prefix[0]
object_id[1] = (objectid_prefix[1]&0xF0) + object_kind"

**Repo:** `crates/xrce/src/object_id.rs` (`ObjectId::new`, `ObjectId::kind`,
`ObjectId::from_reference`)

**Tests:** `crates/xrce/src/object_id.rs::tests::new_packs_kind_into_lower_4_bits`,
`crates/xrce/src/object_id.rs::tests::bytes_are_big_endian`,
`crates/xrce/src/object_id.rs::tests::from_reference_matches_the_examples_of_the_spec`

**Status:** done — in the code the prefix is the 12-bit number `raw_id`; a type
`ObjectIdPrefix` is missing. `ObjectId::from_reference` forms the prefix from
the first 12 bits of the MD5 (§9.3, 03371b4965c). `ObjectId::new_with_mask` and
`kind_mask` read bit 15 as "assigned by the client"; the spec does not say that
and the session does not use it.

---

## §7.7.7 ResultStatus

### §7.7.7 StatusValue

**Spec:** §7.7.7, p. 40 — "@bit_bound(8) enum StatusValue { @value(0x00)
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

**Spec:** §7.7.7, p. 40 — "struct ResultStatus { StatusValue status; octet
implementation_status; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ResultStatus`, `encode_into`, `decode_at`);
`crates/xrce/src/encoding.rs` (`UnannotatedTypes`)

**Tests:** `crates/xrce/src/object_info.rs::tests::result_status_roundtrip`,
`crates/xrce/src/object_info.rs::tests::result_status_decode_short_returns_eof`,
`crates/xrce/tests/unannotated_types.rs::the_default_is_appendable_with_a_dheader`,
`crates/xrce/tests/unannotated_types.rs::the_final_setting_writes_the_reference_implementations_octets`,
`crates/xrce/tests/unannotated_types.rs::both_forms_are_read_in_either_setting`

**Status:** done — Annex A gives `ResultStatus` no `@extensibility`: the default
is APPENDABLE with a DHEADER per DDS-XTypes 1.3 §7.3.1.2.1.8 (6 octets, 10 in
the `BaseObjectReply`). The octets of the reference implementation (2 and 6,
without a DHEADER) are written by the setting
`encoding::UnannotatedTypes::Final` (agent key `unannotated_types: final`); both
forms are read in either setting. f2ace917b5a, 3874ad8d6a6.

### §7.7.7 Table 2 STATUS_OK

**Spec:** §7.7.7, Table 2, p. 41 — "Indicates a successful execution of the
operation".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`status_reply`, `agent_status_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_successful_write_is_answered_with_status_ok`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done — a successful WRITE_DATA is answered with STATUS_OK as well
(dc0209b38fe); the eProsima agent does not reply to it.

### §7.7.7 Table 2 STATUS_OK_MATCHED

**Spec:** §7.7.7, Table 2, p. 41 — "Indicates a successful execution of a
create or update operation on a resource when the resource already existed on
the Agent and the resource state already matched the one requested by the
operation."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Reused`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create`)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_reuse_returns_equal_marker`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`

**Status:** done — a representation equal to that of the existing object gives
STATUS_OK_MATCHED with `reuse` and changes nothing. Table 5 spells
STATUS_OK_MATCHES, `StatusValue` in Annex A and in the code is STATUS_OK_MATCHED
(the name only). 8b949a13ef5.

### §7.7.7 Table 2 STATUS_ERR_DDS_ERROR

**Spec:** §7.7.7, Table 2, p. 41 — "Indicates a failure in the execution of
the operation caused by an error when creating or operating on the DDS
resource related to the operation."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`dds_port_error_status`,
`apply_dds_create`); `crates/xrce-agent/src/daemon/dds_port.rs`
(`DdsPortError::DdsError`, `DdsPort::enable`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_dds_entity_is_enabled_when_the_creation_has_succeeded`

**Status:** done — if the DDS side of a CREATE cannot be enabled (`enable`) or
created, the reply is STATUS_ERR_DDS_ERROR, nothing is stored and the DDS side
is deleted. e4d37a6aff6.

### §7.7.7 Table 2 STATUS_ERR_MISMATCH

**Spec:** §7.7.7, Table 2, p. 41 — "Indicates a failure in the execution of a
create or update operation on a resource when the resource already existed on
the Agent, the state did not match the one requested by the operation, and it
was not possible to change the state of the resource."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Reused { equal: false }`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create`)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_reuse_returns_equal_marker`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`

**Status:** done — `reuse` without `replace` and an unequal representation give
STATUS_ERR_MISMATCH; with `replace` the object is replaced (Table 5).
8b949a13ef5.

### §7.7.7 Table 2 STATUS_ERR_ALREADY_EXISTS

**Spec:** §7.7.7, Table 2, p. 41 — "Indicates a failure in the execution of a
create operation due to the fact that the resource already existed."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Conflict`)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_strict_on_existing_returns_conflict`

**Status:** done

### §7.7.7 Table 2 STATUS_ERR_DENIED

**Spec:** §7.7.7, Table 2, p. 41 — "Indicates a failure in the execution of an
operation due to lack of permissions."

**Repo:** `crates/xrce-agent/src/daemon/access.rs` (`AccessController`);
`crates/xrce-agent/src/daemon/session.rs` (check before CREATE, DELETE,
GET_INFO, WRITE_DATA, READ_DATA)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_key_not_configured_in_the_agent_is_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_without_permission_is_denied_and_stores_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::write_and_read_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::client_properties_are_checked_against_what_the_agent_requires`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_update_needs_the_permission_to_update`

**Status:** done — a ClientKey that is not in `authorized_client_keys`, a
missing client property and an operation without permission give
STATUS_ERR_DENIED; nothing is stored. d66ada44616, 515bcc677f3.

### §7.7.7 Table 2 STATUS_ERR_UNKNOWN_REFERENCE

**Spec:** §7.7.7, Table 2, p. 41 — "Indicates a failure in the execution of an
operation due to the fact that the referenced resource is not known to the
Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`dds_port_error_status`,
`handle_delete`, `handle_get_info`, `participant_of_create`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::full_chain_against_a_real_leased_domain`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::null_port_rejects_topic_before_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_or_subscriber_needs_an_existing_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** done — the tests check `DdsPortError::UnknownReference`, a missing
participant, a missing object in GET_INFO and the deletion of the client without
a ProxyClient; `dds_port_error_status` maps the port error to the value.

### §7.7.7 Table 2 STATUS_ERR_INVALID_DATA

**Spec:** §7.7.7, Table 2, p. 41 — "Indicates a failure in the execution due
to wrong on invalid input parameter data."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`,
`handle_read_data`, `refuse_unreadable`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_request_that_cannot_be_read_is_answered_invalid_data`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_without_a_base_object_request_is_answered`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`

**Status:** done — an unreadable body of CREATE, READ_DATA, WRITE_DATA, DELETE
and GET_INFO, a representation in a format the kind does not have and a filter
that cannot be honored give STATUS_ERR_INVALID_DATA. 133a0db3c4f, bdf3f6ee966.

### §7.7.7 Table 2 STATUS_ERR_INCOMPATIBLE

**Spec:** §7.7.7, Table 2, p. 41 — "Indicates a failure in the execution of an
operation due to an incompatibility between the Client and the Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`,
`handle_get_info`: version in `client_info`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_rejects_wrong_major_version`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_checks_the_version_in_client_info`

**Status:** done

### §7.7.7 Table 2 STATUS_ERR_RESOURCES
**Spec:** §7.7.7, Table 2, p. 41 — "Indicates a failure in the execution of an
operation due to a resource error on the Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`, `create_core`, `handle_message`, `emit_possibly_fragmented`, `data_submessages`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::create_client_over_max_clients_is_a_resource_error`, `crates/xrce-agent/tests/spec_items_session.rs::create_over_max_objects_per_client_is_a_resource_error`, `crates/xrce-agent/tests/spec_items_session.rs::a_full_reorder_buffer_of_a_reliable_stream_is_a_resource_error`, `crates/xrce-agent/tests/spec_items_session.rs::a_fragment_run_over_its_limit_is_a_resource_error`, `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_too_large_for_data_is_reported`

**Status:** done — the agent sends the value at `max_clients`, `max_objects_per_client`, a full receiver buffer, a FRAGMENT run over the limit and a sample that does not fit a DATA; each of these paths has a test (2fa0800f55a, 0f85937a436). An INFO that cannot be encoded has no test.

---

## §7.7.8 BaseObjectRequest

### §7.7.8 BaseObjectRequest

**Spec:** §7.7.8, p. 41 — "@extensibility(FINAL) struct BaseObjectRequest {
RequestId request_id; ObjectId object_id; };"

**Repo:** `crates/xrce/src/object_info.rs` (`BaseObjectRequest`)

**Tests:** `crates/xrce/src/object_info.rs::tests::base_object_request_roundtrip`,
`crates/xrce/src/object_info.rs::tests::base_object_request_decode_short_returns_eof`

**Status:** done — FINAL, 4 octets without a DHEADER.

### §7.7.8 request_id correlates reply and request

**Spec:** §7.7.8, p. 41–42 — "request_id (RequestId) identifies each request.
It is used to correlate a reply with the related request. It is scoped to each
XRCE Client and Agent pair. Note that it is possible to reuse a value of the
request_id for future requests as long as the previous request with that value
is known by Client and Agent to no longer be active."

**Repo:** `crates/xrce-client/src/lib.rs` (`allocate_request_id`, `request_id_octets`);
`crates/xrce-agent/src/daemon/session.rs` (`status_reply`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::full_lifecycle_creates_unique_request_ids_and_real_submessages`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §7.7.8 object_id as target and prefix

**Spec:** §7.7.8, p. 42 — "object_id (ObjectId) the ObjectId that is the target
of the request. For requests that create objects, the object_id conveys the
ObjectIdPrefix for the created object."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`: `ObjectId` from the
prefix of the request and the kind of the representation)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_object_id_is_the_prefix_and_the_kind_of_the_representation`

**Status:** done — for the formation of the `ObjectId` at CREATE see the item on
§7.8.3.1 "input_objectid". e4d37a6aff6.

---

## §7.7.9 BaseObjectReply

### §7.7.9 BaseObjectReply

**Spec:** §7.7.9, p. 42 — "@extensibility(FINAL) struct BaseObjectReply {
BaseObjectRequest related_request; ResultStatus result; };"

**Repo:** `crates/xrce/src/object_info.rs` (`BaseObjectReply`)

**Tests:** `crates/xrce/src/object_info.rs::tests::base_object_reply_roundtrip`,
`crates/xrce/tests/unannotated_types.rs::both_forms_are_read_in_either_setting`

**Status:** done — FINAL: `related_request` (4 octets) and `result`; the
`ResultStatus` inside carries its DHEADER (item `ResultStatus`).

### §7.7.9 related_request

**Spec:** §7.7.9, p. 42 — "related_request contains the request_id and
object_id of the request that caused the reply to be sent [...]. For requests
that create objects, the object_id conveys the desired ObjectId for the
created object."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`status_reply`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §7.7.9 status on success and failure

**Spec:** §7.7.9, p. 42 — "If the operation succeeded the StatusValue shall be
set to STATUS_OK or STATUS_OK_MATCHED. If it failed it shall be set to the
value that corresponds to the type of error encountered."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`: mapping of
`CreateOutcome` to the status)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`

**Status:** done — the text names the members `status` and
`implementation_status`; the structure has `result`.

### §7.7.9 implementation_status

**Spec:** §7.7.9, p. 42 — "implementation_status (octet) provides an
implementation-specific (vendor-specific) return status. The value is scoped by
the XrceVendorId of the Agent. It shall only be interpreted by clients that
understand the implementation status values of the XrceVendorId of the Agent
that returned it."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`status_reply`: value 0);
`crates/xrce-client/src/lib.rs` (`handle_status` does not evaluate it)

**Tests:** `crates/xrce/src/object_info.rs::tests::result_status_roundtrip`

**Status:** done

---

## §7.7.10 RelatedObjectRequest

### §7.7.10 RelatedObjectRequest as a typedef

**Spec:** §7.7.10, p. 42 — "typedef RelatedObjectRequest BaseObjectRequest; The
interpretation is the same as for the related_request that appears in the
BaseObjectReply".

**Repo:** `crates/xrce/src/object_info.rs` (`RelatedObjectRequest`)

**Tests:** `crates/xrce/src/object_info.rs::tests::related_object_request_is_base_object_request`

**Status:** done — the spec writes the `typedef` in reverse order; `typedef
BaseObjectRequest RelatedObjectRequest` is meant.

---

## §7.7.11 CreationMode

### §7.7.11 CreationMode

**Spec:** §7.7.11, p. 43 — "struct CreationMode { boolean reuse; boolean
replace; };"

**Repo:** `crates/xrce/src/object_store.rs` (`CreationMode`);
`crates/xrce/src/submessages/create.rs` (`CREATE_FLAG_REUSE`,
`CREATE_FLAG_REPLACE`)

**Tests:** `crates/xrce/src/submessages/create.rs::tests::create_roundtrip_carries_reuse_replace_flags`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`

**Status:** done — on the wire as flags of CREATE (§8.3.5.2.1); `reuse` and
`replace` act per Table 5 (8b949a13ef5).

---

## §7.7.12 ActivityInfoVariant

### §7.7.12 InfoMask

**Spec:** §7.7.12, p. 43 — "bitmask InfoMask { @position(0)
INFO_CONFIGURATION, @position(1) INFO_ACTIVITY };"

**Repo:** `crates/xrce/src/object_info.rs` (`InfoMask`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`, `object_info`:
configuration and activity by the mask)

**Tests:** `crates/xrce/src/object_info.rs::tests::info_mask_bits_and_wire_form`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_writer_reports_its_activity`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_an_info_mask_is_invalid_data`

**Status:** done — `InfoMask` is a 32-bit bitmask (`INFO_CONFIGURATION` bit 0,
`INFO_ACTIVITY` bit 1, 32 bits per XTypes without `@bit_bound`) and is read from
GET_INFO; a GET_INFO without a mask gives STATUS_ERR_INVALID_DATA. 65cd1820a04.

### §7.7.12 AGENT_ActivityInfo

**Spec:** §7.7.12, p. 43 — "@extensibility(APPENDABLE) struct
AGENT_ActivityInfo { short availability; TransportLocatorSeq address_seq; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ActivityInfoVariant::Agent`);
`crates/xrce-agent/src/daemon/session.rs` (`agent_object_info`)

**Tests:** `crates/xrce/src/object_info.rs::tests::agent_activity_info_has_a_dheader_and_a_locator_sequence`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_a_session_is_answered_for_the_agent_only`

**Status:** done — `@extensibility(APPENDABLE)`: discriminator `OBJK_AGENT`
(0x0D; Annex A writes `OBJECTID_AGENT`, an ObjectId of two octets and no value
of `ObjectKind`), DHEADER, `short availability` and a `TransportLocatorSeq` from
the listeners of the agent. 65cd1820a04, c304e509660.

### §7.7.12 DATAREADER_ActivityInfo

**Spec:** §7.7.12, p. 43 — "@extensibility(APPENDABLE) struct
DATAREADER_ActivityInfo { short highest_acked_num; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ActivityInfoVariant::DataReader`
l. 524, `encode_into` l. 574, `decode_at` l. 633);
`crates/xrce-agent/src/daemon/session.rs` (`object_info` l. 3002)

**Tests:** `crates/xrce/src/object_info.rs::tests::data_reader_activity_info_is_a_dheader_and_a_short`,
`crates/xrce/src/object_info.rs::tests::appended_members_of_an_activity_are_skipped`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_reader_reports_its_activity`

**Status:** `done` — DHEADER and `short highest_acked_num`; members that a newer
version appends are skipped by the reader through the DHEADER. The agent sends
the activity in the INFO to a GET_INFO of a DataReader: `highest_acked_num` is
the highest sequence number the client has acknowledged on the reliable stream
the READ_DATA delivers on, otherwise 0. The spec does not define the value; not
measured against another agent. Commit 65cd1820a04.

### §7.7.12 DATAWRITER_ActivityInfo

**Spec:** §7.7.12, p. 43 — "@extensibility(APPENDABLE) struct
DATAWRITER_ActivityInfo { unsigned long long sample_seq_num; short
stream_seq_num; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ActivityInfoVariant::DataWriter`
l. 529, `encode_into` l. 574, `decode_at` l. 633);
`crates/xrce-agent/src/daemon/session.rs` (`object_info` l. 3002,
`dds_write_and_report` l. 3282)

**Tests:** `crates/xrce/src/object_info.rs::tests::data_writer_activity_info_is_a_dheader_a_long_long_and_a_short`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_writer_reports_its_activity`

**Status:** `done` — DHEADER, `unsigned long long sample_seq_num` (aligned to 4)
and `short stream_seq_num`. The agent counts the samples written per DataWriter
(`sample_seq_num`) and keeps the sequence number in the header of the last
WRITE_DATA message (`stream_seq_num`). The spec does not define either value;
not measured against another agent. Commit 65cd1820a04.

### §7.7.12 ActivityInfoVariant as a union

**Spec:** §7.7.12, p. 43–44 — "union ActivityInfoVariant (ObjectKind) { case
OBJK_DATAWRITER : DATAWRITER_ActivityInfo data_writer; case OBJK_DATAREADER :
DATAREADER_ActivityInfo data_reader; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ActivityInfoVariant` l. 512,
`discriminator` l. 540, `encode_into` l. 574, `decode_at` l. 633)

**Tests:** `crates/xrce/src/object_info.rs::tests::agent_activity_info_has_a_dheader_and_a_locator_sequence`,
`crates/xrce/src/object_info.rs::tests::activity_info_decode_unknown_discriminator_rejected`,
`crates/xrce/src/object_info.rs::tests::activity_info_decode_truncated_rejected`,
`crates/xrce/src/object_info.rs::tests::appended_members_of_an_activity_are_skipped`

**Status:** `done` — the discriminator is the octet of the `ObjectKind`
(OBJK_DATAWRITER 0x05, OBJK_DATAREADER 0x06); the members carry the DHEADER; an
unknown discriminator is an error. Annex A (p. 113) has the agent's case in
addition and writes `OBJECTID_AGENT`, an ObjectId of two octets and no value of
`ObjectKind`; the code takes `OBJK_AGENT` (0x0D) as the discriminator of
`AGENT_ActivityInfo`. The IDL of the clause lacks the word `switch`. Commit
65cd1820a04.

---

## §7.7.13 ObjectInfo

### §7.7.13 ObjectInfo with activity and config

**Spec:** §7.7.13, p. 44 — "@extensibility(FINAL) struct ObjectInfo {
@optional ActivityInfoVariant activity; @optional ObjectVariant config; };"

**Repo:** `crates/xrce/src/object_info.rs` (`ObjectInfo` l. 760, `encode_into`
l. 794, `decode_at` l. 834, `ObjectConfig` l. 690);
`crates/xrce-agent/src/daemon/session.rs` (`object_info` l. 3002, `answer_info`
l. 2962)

**Tests:** `crates/xrce/src/object_info.rs::tests::object_info_puts_the_activity_before_the_config`,
`crates/xrce/src/object_info.rs::tests::the_config_of_a_topic_is_the_object_variant_it_was_created_with`,
`crates/xrce/src/object_info.rs::tests::the_config_of_the_agent_is_its_agent_representation`,
`crates/xrce/src/object_info.rs::tests::an_empty_object_info_is_two_presence_octets`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`

**Status:** `done` — `activity` before `config`, one presence octet for each of
the two `@optional`; `config` is an `ObjectVariant` with its `ObjectKind` (for
the agent the `AGENT_Representation`, for an object the representation it was
created with). Alignment is counted from the start of the payload, the byte
order follows the E flag. Commits 65cd1820a04, b43d5e6b848.

---

## §7.7.14 ReadSpecification

### §7.7.14 DataDeliveryControl

**Spec:** §7.7.14, p. 44 — "@extensibility(APPENDABLE) struct
DataDeliveryControl { unsigned short max_samples; unsigned short
max_elapsed_time; unsigned short max_bytes_per_second; unsigned short
min_pace_period; // milliseconds };"

**Repo:** `crates/xrce/src/submessages/read_data.rs` (`DataDeliveryControl`
l. 60, `encode_into` l. 79, `decode` l. 93, `DeliveryControlForm` l. 127);
`crates/xrce-client/src/lib.rs` (`XrceClient::set_delivery_control_form` l. 372)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::read_data_matches_the_captured_read_data_example`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`

**Status:** `done` — four `unsigned short` in this order, with a DHEADER
(APPENDABLE). The form without a DHEADER that the eProsima capture shows is the
named setting `DeliveryControlForm::Bare`
(`XrceClient::set_delivery_control_form`); the reader takes both forms. Commit
1663f8f6240.

### §7.7.14 DHEADER of DataDeliveryControl

**Spec:** §7.7.14, p. 44 — as above; Annex B.4.2, p. 151–152, shows the
DHEADER.

**Repo:** `crates/xrce/src/submessages/read_data.rs`
(`ReadSpecification::encode_into` l. 151, `decode` l. 204)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`,
`crates/xrce/src/submessages/read_data.rs::tests::read_data_matches_the_captured_read_data_example`

**Status:** `done` — a DHEADER with the length 8, aligned to 4, is written.
Annex B.4.2 also sets bit 31 in the DHEADER (0x80000008), which XCDR2 does not
have in a DHEADER; the reader does not take it as part of the length. Reading
works with and without a DHEADER. Commit 1663f8f6240.

### §7.7.14 ReadSpecification

**Spec:** §7.7.14, p. 44 — "@extensibility(FINAL) struct ReadSpecification {
StreamId preferred_stream_id; DataFormat data_format; @optional string
content_filter_expression; @optional DataDeliveryControl delivery_control;
};"

**Repo:** `crates/xrce/src/submessages/read_data.rs` (`ReadSpecification`
l. 114, `encode_into` l. 151, `decode` l. 204, `ReadDataPayload` l. 312)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::read_data_matches_the_captured_read_data_example`,
`crates/xrce/src/submessages/read_data.rs::tests::read_data_full_spec_roundtrip`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`

**Status:** `done` — order of the fields, presence octets of the two
`@optional`, values of `data_format` after Annex A and an XCDR2 string (see the
items on data_format, content_filter_expression and DataDeliveryControl). Commit
1663f8f6240.

### §7.7.14 Values of data_format

**Spec:** §7.7.14, p. 44; Annex A, p. 113–114 — "const DataFormat FORMAT_DATA
= 0x00; const DataFormat FORMAT_SAMPLE = 0x02; const DataFormat
FORMAT_DATA_SEQ = 0x08; const DataFormat FORMAT_SAMPLE_SEQ = 0x0A; const
DataFormat FORMAT_PACKED_SAMPLES = 0x0E;"

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`DataFormat` l. 52, `raw`
l. 69, `from_raw` l. 78); `crates/xrce/src/submessages/read_data.rs`
(`ReadSpecification::encode_into` l. 151);
`crates/xrce-agent/src/daemon/session.rs` (`data_submessages` l. 3581)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::the_data_format_octet_has_the_values_of_annex_a`,
`crates/xrce/src/submessages/read_data.rs::tests::read_spec_rejects_reserved_data_format`,
`crates/xrce/src/submessages/write_data.rs::tests::data_format_raw_roundtrip`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_data_seq_is_answered_with_one_data_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_is_answered_with_a_sample_each`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_seq_is_answered_with_one_sample_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_packed_samples_is_answered_with_one_packed_samples`

**Status:** `done` — `DataFormat::raw` gives 0x00, 0x02, 0x08, 0x0A and 0x0E;
`from_raw` reads the same values, bits outside FORMAT_MASK (0x0E) and reserved
combinations are `ValueOutOfRange`. The agent delivers DATA in the requested
format. Commits 1663f8f6240, d5fbdbe672e.

### §7.7.14 Encoding of content_filter_expression

**Spec:** §7.7.14, p. 44 — "@optional string content_filter_expression;"
(XCDR2 by §7.7); Annex B.4.2, p. 151–152: padding byte 15, length 6 for
"x>100".

**Repo:** `crates/xrce/src/submessages/read_data.rs`
(`ReadSpecification::encode_into` l. 151, `decode` l. 204)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::the_content_filter_is_a_string_with_a_nul_and_an_aligned_length`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`

**Status:** `done` — XCDR2 string: the length is an `unsigned long`, aligned to
4 (from the start of the payload), and counts the NUL; the NUL follows the
characters. Annex B.4.2 (padding octet 15, length 6 for "x>100") is pinned by a
test with the bytes. Commit 1663f8f6240.

---

## §7.8.1 Use of the ClientKey

### §7.8.1 The ClientKey identifies the client and carries permissions

**Spec:** §7.8.1, p. 44 — "The ClientKey is assigned to each client. The
ClientKey uniquely identifies the client to a particular agent. The ClientKey
is associated with a set of permissions for the client within the agent."

**Repo:** `crates/xrce-agent/src/lib.rs` (`XrceAgent::clients` l. 102);
`crates/xrce-agent/src/daemon/transport.rs` (`PeerTable::resolve` l. 111);
`crates/xrce-agent/src/daemon/access.rs` (`AccessController::permits` l. 212,
`Rule` l. 125)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::multiple_clients_isolated`,
`crates/xrce-agent/src/daemon/transport.rs::tests::peer_table_resolves_header_client_key_directly`,
`crates/xrce-agent/src/daemon/access.rs::tests::a_controller_without_rules_restricts_nothing`,
`crates/xrce-agent/src/daemon/access.rs::tests::the_first_matching_rule_decides_and_no_match_denies`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_without_permission_is_denied_and_stores_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::write_and_read_without_permission_are_denied`

**Status:** `done` — the ClientKey identifies the client. The permissions are in
the AccessController (§7.4): rules per ClientKey for operation, ObjectKind, DDS
domain and topic (configuration key `access_rules`); the first matching rule
decides, without a match the operation is refused, without rules everything is
allowed. An operation without permission gets STATUS_ERR_DENIED. The rules are
read at start-up; a reload with SIGHUP does not change them. Commits
d66ada44616, 515bcc677f3.

### §7.8.1 The ClientKey is secret and configured in client and agent

**Spec:** §7.8.1, p. 45 — "The ClientKey shall be considered secret. It must be
configured both on the Client and in the Agent. The creation and configuration
are outside the scope of this specification."

**Repo:** `crates/xrce-agent/src/daemon/access.rs`
(`AccessController::client_authorized` l. 193, `parse_client_key` l. 227);
`crates/xrce-agent/src/daemon/config.rs` (`authorized_client_keys`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create_client` l. 1227)

**Tests:** `crates/xrce-agent/src/daemon/access.rs::tests::only_configured_client_keys_are_authorized`,
`crates/xrce-agent/src/daemon/access.rs::tests::a_client_key_is_read_in_wire_order`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_key_not_configured_in_the_agent_is_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::without_configured_client_keys_every_key_is_accepted`

**Status:** `done` — configuration key `authorized_client_keys` (four octets in
the order of transmission); a ClientKey outside the list gets STATUS_ERR_DENIED
and no session, without a list the agent accepts every key. The client carries
its ClientKey (`XrceClient::new`). The protection of the ClientKey on the
transport after §8.3.2.4 is the listeners `kind: dtls` and `kind: tls`. Commits
d66ada44616, 515bcc677f3, 01470e273bc.

### §7.8.1 The ClientKey is not interpreted

**Spec:** §7.8.1, p. 45 — "The ClientKey shall not be interpreted."

**Repo:** `crates/xrce/src/header.rs` (`ClientKey` l. 110)

**Tests:** `crates/xrce/src/handshake.rs::tests::client_representation_decodes_the_captured_create_client_prefix`

**Status:** `done`

### §7.8.1 Operations only with an existing ProxyClient

**Spec:** §7.8.1, p. 45 — "With the exception of the operations create_client
and get_info on the XRCE Root, all other operations expect that the ClientKey
references an already exiting XRCE ProxyClient. If this is not the case, the
operation shall fail."

**Repo:** `crates/xrce-agent/src/lib.rs` (`XrceAgent::create_object` l. 240,
`pull_sample` l. 313: `AgentError::UnknownClient`);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` l. 1583,
`handle_delete` l. 2733)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::create_object_for_unknown_client_rejected`,
`crates/xrce-agent/src/lib.rs::tests::pull_sample_unknown_client_rejected`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** `done` — an operation for a ClientKey without a ProxyClient is not
carried out; the session answers with `STATUS_ERR_UNKNOWN_REFERENCE`.

### §7.8.1 STATUS_ERR_NOCLIENT or a silent drop
**Spec:** §7.8.1, p. 45 — "To avoid information leakage that could compromise
security, the failure to locate a ClientKey may in some cases result in a
returnValue having STATUS_ERR_NOCLIENT while in others it may silently drop the
connection to the client."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_message`, `drops_for_unknown_client`, `SessionEvent::NoClient`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_message_to_an_unknown_client_key_is_dropped_and_counted`

**Status:** done — §7.8.1 gives two ways: `STATUS_ERR_NOCLIENT` or a silent drop. `StatusValue` (Annex A) has no such value, the agent drops the message without an answer and reports `SessionEvent::NoClient`; `create_client` and `get_info` are exempt, `delete_client` gets its answer by §7.8.2.3. 7356cbaaa8c.
### §7.8.1 Counter for STATUS_ERR_NOCLIENT and closing the connection
**Spec:** §7.8.1, p. 45 — "The Agent shall maintain a counter on the number of
times the STATUS_ERR_NOCLIENT was sent on an established connection, and once
a certain threshold is crossed it shall close the connection. The Agent may
subsequently refuse or throttle new connections originating from the same
client transport endpoint that was previously closed."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`PeerTable::strike`, `PeerTable::is_blocked`, `apply_events`); `crates/xrce-agent/src/daemon/config.rs` (`no_client_threshold`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::messages_to_an_unknown_client_key_close_the_connection_at_the_threshold`, `crates/xrce-agent/src/daemon/config.rs::tests::the_no_client_threshold_can_be_set`

**Status:** done — the agent counts the messages per peer to an unknown ClientKey; at `no_client_threshold` (default 16, 0 never) the TCP connection is closed and a UDP peer is not read any more. 7356cbaaa8c.

---

## §7.8.2 XRCE Root

### §7.8.2 The Root authenticates and creates ProxyClients

**Spec:** §7.8.2, p. 45 — "The XRCE Root object represents the Agent. An XRCE
Agent is a singleton object that all agents shall instantiate. The XRCE Root
is responsible for authenticating client applications and creating the XRCE
ProxyClient object associated with each client."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine`,
`handle_create_client` l. 1168; checks l. 1227 and l. 1240);
`crates/xrce-agent/src/daemon/access.rs` (`AccessController`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_key_not_configured_in_the_agent_is_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::client_properties_are_checked_against_what_the_agent_requires`

**Status:** `done` — the ProxyClient is created. Authentication is done by the
AccessController: configured ClientKeys (`authorized_client_keys`) and required
properties with name and value (`required_client_properties`), otherwise
STATUS_ERR_DENIED and no session. Commits d66ada44616, 515bcc677f3.

### §7.8.2 Table 3 Root operations

**Spec:** §7.8.2, Table 3, p. 45 — `create_client`, `get_info`,
`delete_client`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the operations are listed one by
one in §7.8.2.1 to §7.8.2.3.

### §7.8.2.1 create_client: inputs and outputs

**Spec:** §7.8.2.1, p. 45–46 — "client_representation (CLIENT_Representation):
a representation of the Client. [...] returnValue (ResultStatus) [...]
agent_info (AGENT_Representation): a representation of the Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
l. 1168, `agent_status_reply` l. 4331)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`

**Status:** `done` — STATUS_AGENT carries `ResultStatus` and
`AGENT_Representation` (item on §8.3.5.5).

### §7.8.2.1 CLIENT_Representation with client_timestamp

**Spec:** §7.8.2.1, p. 46 — "struct CLIENT_Representation { XrceCookie
xrce_cookie; XrceVersion xrce_version; XrceVendorId xrce_vendor_id; Time_t
client_timestamp; ClientKey client_key; SessionId session_id; @optional
PropertySeq properties; };"

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation` l. 328,
`encode_form` l. 407, `decode_form` l. 463); `crates/xrce/src/encoding.rs`
(`RepresentationTimestamps` l. 214); `crates/xrce-agent/src/daemon/config.rs`
(`representation_timestamps`); `crates/xrce-client/src/lib.rs`
(`XrceClient::set_representation_timestamps` l. 364)

**Tests:** `crates/xrce/src/handshake.rs::tests::by_default_the_representations_have_no_timestamp`,
`crates/xrce/src/handshake.rs::tests::the_prose_form_has_a_time_t_after_the_vendor_id`,
`crates/xrce/src/handshake.rs::tests::client_representation_decodes_the_captured_create_client_prefix`,
`crates/xrce-client/src/lib.rs::tests::the_representation_timestamp_is_an_option_of_the_client`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_form_of_the_representations_can_be_set`

**Status:** `done` — the spec contradicts itself: the text (§7.8.2.1,
§8.3.5.1.2) names `Time_t client_timestamp` after `xrce_vendor_id`; the IDL of
Annex A (p. 105–106), the bytes of Annex B.1 and the eProsima capture (16
octets) do not have it. The bytes do not tell the forms apart. The default is
the form of the IDL; the form with a timestamp is the named setting
`representation_timestamps: absent | present` (agent key),
`encoding::RepresentationTimestamps` (Rust API),
`XrceClient::set_representation_timestamps` and the option `representation
timestamps` in the XRCE client of every SDK. The `Time_t` follows
`UnannotatedTypes` (by default a DHEADER with the length 8). The tests check the
bytes of both forms. Commits 711f93fd460, a8e8f6953a4.

### §7.8.2.1 AGENT_Representation with agent_timestamp

**Spec:** §7.8.2.1, p. 46 — "struct AGENT_Representation { XrceCookie
xrce_cookie; XrceVersion xrce_version; XrceVendorId xrce_vendor_id; Time_t
agent_timestamp; @optional PropertySeq properties; };"

**Repo:** `crates/xrce/src/handshake.rs` (`AgentRepresentation` l. 356,
`encode_form` l. 554, `decode_form` l. 599); `crates/xrce/src/encoding.rs`
(`RepresentationTimestamps` l. 214)

**Tests:** `crates/xrce/src/handshake.rs::tests::the_agent_representation_follows_the_setting_the_same_way`,
`crates/xrce/src/handshake.rs::tests::agent_representation_decodes_the_captured_status_agent_body`,
`crates/xrce-client/src/lib.rs::tests::the_representation_timestamp_is_an_option_of_the_client`

**Status:** `done` — as for the item on `client_timestamp`: the default is the
form of Annex A (p. 106) without `agent_timestamp`; the form with the timestamp
is the setting `representation_timestamps: present`. The agent reads the setting
at start-up (`server.rs`, `set_representation_timestamps`). Commits 711f93fd460,
a8e8f6953a4.

### §7.8.2.1 Check of xrce_cookie

**Spec:** §7.8.2.1, p. 46 — "Check the xrce_cookie to ensure it matches the
predefined XRCE_COOKIE constant. If it does not match the creation shall fail
and set the returnValue StatusValue to STATUS_ERR_INVALID_DATA."

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation::decode_at`
l. 455); `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
l. 1175)

**Tests:** `crates/xrce/src/submessages/create_client.rs::tests::refuses_a_payload_whose_cookie_is_not_xrce`,
`crates/xrce/src/handshake.rs::tests::client_representation_rejects_wrong_cookie`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`

**Status:** `done` — the cookie is checked; a wrong one gets a STATUS_AGENT with
STATUS_ERR_INVALID_DATA, no client is registered. Commit ed2ca7b49a9.

### §7.8.2.1 Answer to a wrong cookie as STATUS

**Spec:** §7.8.2.1, p. 46 — as above; §8.3.5.1, p. 63: "The XRCE Agent shall
send a STATUS_AGENT message in response".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
l. 1175–1192, `agent_status_reply` l. 4331)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_unreadable_property_seq_is_refused_with_invalid_data`

**Status:** `done` — the answer is a STATUS_AGENT (§8.3.5.1) with the ClientKey
of the header when the header has one, otherwise zeros. Commit ed2ca7b49a9.

### §7.8.2.1 Check of the major version

**Spec:** §7.8.2.1, p. 46 — "Check that the major version (xrce_version[0])
matches the XRCE_VERSION_MAJOR. If it does not match, the creation shall fail
and set the returnValue StatusValue to STATUS_ERR_INCOMPATIBLE."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
l. 1212)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_rejects_wrong_major_version`

**Status:** `done`

### §7.8.2.1 client_key authorized

**Spec:** §7.8.2.1, p. 46 — "Check that the Client identified by the
client_key is authorized to connect to the XRCE Agent. If this check fails the
operation shall fail and set the returnValue StatusValue to
STATUS_ERR_DENIED."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
l. 1227, `deny_create_client` l. 1337); `crates/xrce-agent/src/daemon/access.rs`
(`AccessController::client_authorized` l. 193)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_key_not_configured_in_the_agent_is_denied`,
`crates/xrce-agent/src/daemon/access.rs::tests::only_configured_client_keys_are_authorized`

**Status:** `done` — a ClientKey outside `authorized_client_keys` gets
STATUS_ERR_DENIED; no session is created. Commits d66ada44616, 515bcc677f3.

### §7.8.2.1 Check of the properties

**Spec:** §7.8.2.1, p. 46 — "Check the Client properties, if present. These
may contain vendor-specific information that may prevent the Agent from
accepting the connection from the Client. [...] If this check fails the
operation shall fail and set the returnValue StatusValue to the appropriate
value."

**Repo:** `crates/xrce-agent/src/daemon/access.rs`
(`AccessController::properties_accepted` l. 201); `crates/xrce/src/handshake.rs`
(`decode_properties` l. 266); `crates/xrce-agent/src/daemon/session.rs`
(`handle_create_client` l. 1240)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::client_properties_are_checked_against_what_the_agent_requires`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_unreadable_property_seq_is_refused_with_invalid_data`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_big_endian_create_client_with_properties_is_read_right`,
`crates/xrce/src/handshake.rs::tests::a_malformed_property_seq_is_an_error_not_absent_properties`

**Status:** `done` — if the agent requires properties
(`required_client_properties`, name and value), it answers a client without them
or with another value with STATUS_ERR_DENIED. A PropertySeq that cannot be read
is an error and is answered with STATUS_ERR_INVALID_DATA; it does not count as
"no properties". Commits 515bcc677f3, 65cd1820a04.

### §7.8.2.1 Existing ProxyClient with the same session_id

**Spec:** §7.8.2.1, p. 47 — "If a ProxyClient exists and has the same
session_id, then the operation shall not perform any action and shall set the
returnValue StatusValue to STATUS_OK."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
l. 1274 `same_session`, l. 1302–1332)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_same_session_id_reconnect_keeps_object_store`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_the_same_session_leaves_the_streams_alone`

**Status:** `done` — object store and streams stay, the answer is STATUS_OK. The
variant that resets the streams is the setting of the following item. Commit
ed2ca7b49a9.

### §7.8.2.1 Streams on the same session_id

**Spec:** §7.8.2.1, p. 47 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
l. 1290, `SessionEngineConfig::restart_on_same_session`);
`crates/xrce-agent/src/daemon/config.rs` (`same_session_create_client`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_the_same_session_leaves_the_streams_alone`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_the_same_session_can_restart_the_streams`,
`crates/xrce-agent/src/daemon/config.rs::tests::a_create_client_for_the_same_session_can_restart_the_streams`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done` — default `keep` (spec): a CREATE_CLIENT with the session_id
of the existing ProxyClient "shall not perform any action", the streams go on.
Named setting `same_session_create_client: restart`: the agent discards the
states of the reliable streams, of the fragmentation, the backlog and the
counters of the best-effort streams. The eProsima processes need this; they
reuse ClientKey and `session_id` and start at sequence number 0 (capture; the
replay test sets `restart`). Commit ed2ca7b49a9.

### §7.8.2.1 Existing ProxyClient with another session_id

**Spec:** §7.8.2.1, p. 47 — "If a ProxyClient exists and has a different
session_id then the operation shall delete the existing XRCE ProxyClient
object and subsequently take the same actions as if there had not been a
ProxyClient associated with the client_key."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
l. 1275)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_different_session_id_resets_object_store`

**Status:** `done`

### §7.8.2.1 Sufficient resources
**Spec:** §7.8.2.1, p. 47 — "Check that there are sufficient internal
resources to complete the create operation. If there are not, then the
operation shall fail and set the returnValue StatusValue to
STATUS_ERR_RESOURCES."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::create_client_over_max_clients_is_a_resource_error`

**Status:** done — `handle_create_client` checks `max_clients` and answers STATUS_ERR_RESOURCES; a repeated CREATE_CLIENT of a client that exists stays STATUS_OK (0f85937a436).
### §7.8.2.1 Deletion resets the communication state

**Spec:** §7.8.2.1, p. 47 — "deletion of an existing ProxyClient resets any
prior communication state between the client and the agent. Any messages that
were cached pending acknowledgments shall be discarded."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
l. 1275–1301, `handle_delete` l. 2733)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_different_session_id_resets_object_store`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done`

### §7.8.2.1 New ProxyClient: session_id, streams from 0, STATUS_OK

**Spec:** §7.8.2.1, p. 47 — "If the Agent creates a ProxyClient object it
shall: Initialize its state to have the specified session_id. Initialize the
built-in streams with sequence number 0. Set the returnValue StatusValue to
STATUS_OK. Return a representation of the XRCE Agent in the agent_info."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`
l. 1302–1332); `crates/xrce/src/reliable.rs` (`ReliableStreamState::new`:
`next_seq = 0`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::reliable_outbound_messages_get_real_incrementing_sequence_numbers`

**Status:** `done`

### §7.8.2.1 Timestamps for synchronisation

**Spec:** §7.8.2.1, p. 47 — "The Agent and Client may use the client_timestamp
and agent_timestamp to detect time-synchronization differences [...]. The use
of this information is left outside the scope of this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — outside the specification.

### §7.8.2.1 XrceVersion and XrceVendorId for configuration

**Spec:** §7.8.2.1, p. 47 — "The Agent and Client may use the XrceVersion and
XrceVendorId to further configure their protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — permission.

### §7.8.2.2 Root get_info: inputs and outputs

**Spec:** §7.8.2.2, p. 47 — "info_mask (InfoMask): selects the kind of
information to retrieve. client_info (ObjectInfo): a representation of the
Client. [...] returnValue (ResultStatus) [...] agent_info (ObjectInfo): a
representation of the Agent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info` l. 2838,
`agent_object_info` l. 4242, `client_info_status` l. 242);
`crates/xrce/src/object_info.rs` (`InfoMask` l. 449);
`crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply` l. 107);
`crates/xrce/src/submessages/get_info.rs` (`GetInfoPayload::with_client_info`
l. 45)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_checks_the_version_in_client_info`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_info_mask_selects_activity_and_configuration`,
`crates/xrce/src/object_info.rs::tests::info_mask_bits_and_wire_form`

**Status:** `done` — INFO with `ResultStatus` and the `ObjectInfo` of the agent.
`info_mask` is a 32-bit mask (`InfoMask`) that the agent reads from the
GET_INFO; it selects `activity` and `config`. `client_info`: `GET_INFO_Payload`
(§8.3.5.3.2, Annex A) has no member for it, §7.8.2.2 and Figure 10 (§8.4.4) name
it. The default is the payload of Annex A; `GetInfoPayload::with_client_info`
appends `client_info` behind the InfoMask, the agent reads it when it is there
(item on `client_info`). Commits 65cd1820a04, 5798b040952.

### §7.8.2.2 get_info before create_client and over another connection

**Spec:** §7.8.2.2, p. 47 — "The operation get_info returns information on the
XRCE Agent and may be used prior to establishing a Session with the XRCE Agent
[...]. The operation get_info may be used over a different transport or
connection".

**Repo:** `crates/xrce-agent/src/daemon/discovery.rs` (`run_discovery_responder`
l. 43); `crates/xrce-agent/src/daemon/session.rs` (`refusal` l. 936,
`handle_submessage` l. 1141, `handle_get_info` l. 2838)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_tcp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_a_session_is_answered_for_the_agent_only`

**Status:** `done` — the responder on the discovery port and the session engine
on the UDP and TCP listeners answer GET_INFO(OBJK_AGENT) without a session; a
transport other than that of the session that follows is possible too. There is
no test of its own for the serial line listener. Commits 65cd1820a04,
67ead4d9708.

### §7.8.2.2 GET_INFO without a session on a listener

**Spec:** §7.8.2.2, p. 47 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`refusal` l. 936,
`handle_submessage` l. 1141, `handle_get_info` l. 2838)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_tcp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_a_session_is_answered_for_the_agent_only`

**Status:** `done` — `handle_submessage` passes a GET_INFO without a resolved
client or with a header without a session to `handle_get_info(None, ...)`;
`GET_INFO(OBJK_AGENT)` before CREATE_CLIENT is answered on the UDP and TCP
listeners, any other kind with `STATUS_ERR_UNKNOWN_REFERENCE`. Commits
65cd1820a04, 67ead4d9708.

### §7.8.2.2 client_info with CLIENT_Representation and CLIENTKEY_INVALID

**Spec:** §7.8.2.2, p. 47–48 — "The ObjectVariant member within client_info
shall contain a CLIENT_Representation, which provides information on the XRCE
Client that makes the request. [...] The client_key field of
CLIENT_Representation shall be set to the value CLIENTKEY_INVALID".

**Repo:** `crates/xrce/src/submessages/get_info.rs`
(`GetInfoPayload::with_client_info` l. 45, `client_info` l. 62);
`crates/xrce-agent/src/daemon/session.rs` (`client_info_status` l. 242);
`crates/xrce-client/src/discovery.rs` (`AgentDiscovery::with_client_info`
l. 113); `crates/xrce-client/src/lib.rs` (`XrceClient::request_agent_info`
l. 622)

**Tests:** `crates/xrce/src/submessages/get_info.rs::tests::client_info_follows_the_info_mask`,
`crates/xrce-client/src/discovery.rs::tests::client_info_names_the_version_and_hides_the_client_key`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_checks_the_version_in_client_info`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::a_client_of_another_major_version_is_answered_incompatible`,
`crates/xrce-client/src/lib.rs::tests::request_agent_info_works_before_the_session_and_sends_the_mask`

**Status:** `done` — `client_info` is an `ObjectInfo` behind the InfoMask whose
`config` is a `CLIENT_Representation` with `client_key` CLIENTKEY_INVALID
(zeros). Annex A has no member for it in `GET_INFO_Payload`; a receiver that
does not read it answers the GET_INFO as without. The agent reads it: another
major version gets an INFO with STATUS_ERR_INCOMPATIBLE (Figure 10), an
unreadable one or one that is no CLIENT_Representation STATUS_ERR_INVALID_DATA.
The client sends it only on `AgentDiscovery::with_client_info`;
`XrceClient::request_agent_info` sends the GET_INFO of Annex A. Commits
65cd1820a04, 5798b040952.

### §7.8.2.2 agent_info with AGENT_Representation and AGENT_ActivityInfo

**Spec:** §7.8.2.2, p. 48 — "The ObjectVariant member within agent_info shall
contain an AGENT_ActivityInfo [...]. The ActivityInfoVariant member within
agent_info shall contain an AGENT_Representation".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`agent_object_info`
l. 4242); `crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply`
l. 107)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_info_mask_selects_activity_and_configuration`

**Status:** `done` — `activity` carries `ActivityInfoVariant::Agent`
(`AGENT_ActivityInfo`, layout see §7.7.12), `config` the `AGENT_Representation`.
The text swaps the two members: `config` is `ObjectVariant`, `activity` is
`ActivityInfoVariant`; the code follows Annex A. Commit 65cd1820a04.

### §7.8.2.2 AGENT_Representation in agent_info

**Spec:** §7.8.2.2, p. 48 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`agent_object_info`
l. 4242, `handle_get_info` l. 2838); `crates/xrce-agent/src/daemon/discovery.rs`
(`build_info_reply` l. 107)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_info_mask_selects_activity_and_configuration`

**Status:** `done` — `config` is `ObjectConfig::Agent(AgentRepresentation)` with
version and vendor of the agent, as far as the InfoMask asks for `config`; a
client picks an agent with it after §8.4.4. `properties` is empty. Commit
65cd1820a04.

### §7.8.2.2 address_seq reports the addresses
**Spec:** §7.8.2.2, p. 48 — "ActivityInfoVariant member address_seq shall be
used to inform the XRCE Client of the transport addresses over which it can be
reached and can receive calls to create_client."

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`, `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_names_the_secured_and_the_serial_listeners`

**Status:** done — `address_seq` carries the addresses of the listeners: UDP and TCP with IPv4 as `ADDRESS_FORMAT_MEDIUM`, with IPv6 as `ADDRESS_FORMAT_LARGE` (§11.2.1, §11.3.1), as a `TransportLocatorSeq` in the `AGENT_ActivityInfo` with a DHEADER; DTLS, TLS and a serial line as `ADDRESS_FORMAT_STRING` (next item). c304e509660, 65cd1820a04, 8d2b85bfeb6.
### §7.8.2.2 address_seq names the DTLS and TLS listeners
**Spec:** §7.8.2.2, p. 48 — as above.

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_names_the_secured_and_the_serial_listeners`

**Status:** done — the listeners `kind: dtls` and `kind: tls` are reported as `ADDRESS_FORMAT_STRING` locators (`dtls://host:port`, `tls://host:port`), a serial line as `serial://device?address=n`: `TransportLocatorFormat` has no value of its own for them, and a plain locator of the same host would lead a client to connect without the protection of §8.3.2.4. 8d2b85bfeb6.
### §7.8.2.2 properties for an authentication

**Spec:** §7.8.2.2, p. 48 — "The properties field of type PropertySeq [...] may
be used to implement an authentication protocol for the XRCE Agent. The
specific mechanism is outside the scope of this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — outside the specification.

### §7.8.2.3 delete_client deletes the ProxyClient

**Spec:** §7.8.2.3, p. 48 — "If the object is found it shall be delete and
returnValue StatusValue shall be set to STATUS_OK."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete` l. 2733)

**Tests:** `crates/xrce-client/src/lib.rs::tests::disconnect_sends_delete_client_and_clears_state`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done`

### §7.8.2.3 delete_client without a ProxyClient

**Spec:** §7.8.2.3, p. 48 — "The XRCE Agent shall check the ClientKey to
locate an existing XRCE: ProxyClient. If the object is not found the operation
shall fail and returnValue StatusValue shall be set to
STATUS_ERR_UNKNOWN_REFERENCE."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete` l. 2733:
`deletes_client`, `deleted_clients`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** `done` — `DELETE(OBJECTID_CLIENT)` for a ClientKey without a
ProxyClient gets `STATUS_ERR_UNKNOWN_REFERENCE`. The same request again that
ended the session is a retransmission and gets `STATUS_OK`. Commits ed2ca7b49a9,
ffa31fc59af.

---

## §7.8.3 XRCE ProxyClient

### §7.8.3 ProxyClient by the ClientKey

**Spec:** §7.8.3, p. 48 — "The XRCE ProxyClient object represents a specific
XRCE Client inside a concrete XRCE Agent. The ProxyClient object is identified
by the ClientKey."

**Repo:** `crates/xrce-agent/src/lib.rs` (`XrceAgent::clients` l. 102)

**Tests:** `crates/xrce-agent/src/lib.rs::tests::register_client_idempotent`

**Status:** `done`

### §7.8.3 Table 4 ProxyClient operations

**Spec:** §7.8.3, Table 4, p. 48–49 — `create`, `update`, `get_info`,
`delete`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the operations are listed one by
one in §7.8.3.1 to §7.8.3.4.

### §7.8.3.1 create: inputs and outputs

**Spec:** §7.8.3.1, p. 49 — "creation_mode (CreationMode) [...] objectid_prefix
(ObjectIdPrefix) [...] object_representation (ObjectVariant) [...] The
object_id in the returnValue shall be derived from the object_prefix input
parameter."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create` l. 1452,
`create_core` l. 1583, `status_reply` l. 4293)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done`

### §7.8.3.1 input_objectkind and input_objectid

**Spec:** §7.8.3.1, p. 49 — "We will refer to this ObjectKind as the
'input_objectkind'. [...] The combination of the objectid_prefix and the
ObjectKind contained in the object_representation discriminator shall be used
to construct the 'input' ObjectId."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create` l. 1531:
`ObjectId::new(request.object_id.raw_id_12(), kind)`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_object_id_is_the_prefix_and_the_kind_of_the_representation`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** `done` — the `ObjectId` is the prefix of the request (12 bits) and
the kind of the representation's discriminator; the STATUS names it. Commit
e4d37a6aff6.

### §7.8.3.1 object_id with another nibble

**Spec:** §7.8.3.1, p. 49 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`
l. 1531–1544)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_object_id_is_the_prefix_and_the_kind_of_the_representation`

**Status:** `done` — the lower nibble of the request's `object_id` is not read:
even with another kind in it `handle_create` creates the object of the
discriminator's kind under the same prefix (in the test a participant under the
`object_id` of a publisher kind). Commit e4d37a6aff6.

### §7.8.3.1 Table 5: object does not exist

**Spec:** §7.8.3.1, Table 5, p. 50 — "Don't care / Don't care / NO — Create
object according to Table 6."

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore::create` l. 152,
`CreateOutcome::Created` l. 175)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_inserts_new_object`

**Status:** `done`

### §7.8.3.1 Table 5: reuse=FALSE, replace=FALSE, exists

**Spec:** §7.8.3.1, Table 5, p. 50 — "FALSE / FALSE / YES — No action taken.
Set the StatusValue within returnValue to: STATUS_ERR_ALREADY_EXISTS."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Conflict` l. 208);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` l. 1692)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_strict_on_existing_returns_conflict`

**Status:** `done`

### §7.8.3.1 Table 5: reuse=FALSE, replace=TRUE, exists

**Spec:** §7.8.3.1, Table 5, p. 50 — "FALSE / TRUE / YES — Delete existing
object as specified by the delete operation. Create object according to Table
6. Set the StatusValue within returnValue to: STATUS_OK."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Replaced` l. 206);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` l. 1700)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_replace_increments_version`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_replace_that_fails_returns_the_object_to_its_previous_configuration`

**Status:** `done` — this includes that a replacement that fails returns the
object to its previous configuration (§7.8.3.2). Commit e4d37a6aff6.

### §7.8.3.1 Table 5: reuse=TRUE, replace=FALSE, exists

**Spec:** §7.8.3.1, Table 5, p. 50 — "TRUE / FALSE / YES — Check if
object_representation matches the existing Object: If it matches no action is
taken [...] STATUS_OK_MATCHES. If it does not match no action is taken [...]
STATUS_ERR_MISMATCH."

**Repo:** `crates/xrce/src/object_store.rs` (`CreateOutcome::Reused` l. 180);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` l. 1688–1693)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_reuse_returns_equal_marker`,
`crates/xrce-agent/src/daemon/session.rs::tests::reuse_compares_the_container_of_the_representation`

**Status:** `done` — equal: `STATUS_OK_MATCHED`, unequal: `STATUS_ERR_MISMATCH`,
no change in either case. Table 5 spells `STATUS_OK_MATCHES`; `StatusValue` in
Annex A and `ResultStatusCode::OkMatched` (0x01) are named `STATUS_OK_MATCHED`.
Representation, container and domain are compared (item below). Commit
e4d37a6aff6.

### §7.8.3.1 Comparison with participant_id, publisher_id, domain_id

**Spec:** §7.8.3.1, Table 5, p. 50 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core` l. 1671:
`same_extra`, `ObjectVariantExtra`); `crates/xrce/src/object_store.rs`
(`ObjectStore::create` l. 152)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::reuse_compares_the_container_of_the_representation`

**Status:** `done` — the comparison on `reuse` includes the container
(participant_id, publisher_id, subscriber_id) and the domain_id, which the
object store does not hold (`ObjectVariantExtra` per object in the session). The
same XML string under another container gets `STATUS_ERR_MISMATCH`. Commit
e4d37a6aff6.

### §7.8.3.1 Table 5: reuse=TRUE, replace=TRUE, exists

**Spec:** §7.8.3.1, Table 5, p. 50 — "TRUE / TRUE / YES — Check if
object_representation matches the existing Object: If it matches, no action is
taken [...] STATUS_OK_MATCHES. If it does not match, delete existing object as
specified by the delete operation and then create a new object according to
Table 6. Set the StatusValue within returnValue to: STATUS_OK."

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore::create` l. 177–192);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` l. 1672–1681)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_reuse_and_replace_replaces_an_unequal_object`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`

**Status:** `done` — on equality `STATUS_OK_MATCHED` without a change, on
inequality the object is deleted and created anew (`STATUS_OK`), the DDS side
included. Commit 8b949a13ef5.

### §7.8.3.1 reuse and replace replace an unequal object

**Spec:** §7.8.3.1, Table 5, p. 50 — as above.

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore::create` l. 177–192);
`crates/xrce-agent/src/daemon/session.rs` (`create_core` l. 1672–1681)

**Tests:** `crates/xrce/src/object_store.rs::tests::create_reuse_and_replace_replaces_an_unequal_object`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_update_of_a_missing_object_creates_it_or_is_refused`,
`crates/xrce-agent/src/daemon/session.rs::tests::an_update_needs_the_permission_to_update`

**Status:** `done` — `ObjectStore::create` treats `reuse` with and without
`replace` differently: without `replace` an unequal object gets
`STATUS_ERR_MISMATCH`, with `replace` it replaces the object. That is also the
operation `update` (§7.8.3.2, Table 14, §8.4.13): `XrceClient::update_object`,
permission `update` in the AccessController, without an existing object it is
created (Table 5); `update_of_missing_object: refuse` gives
`STATUS_ERR_UNKNOWN_REFERENCE` (§7.8.3.2). The two readings contradict each
other in the spec; the default is Table 5, tested in
`the_update_of_a_missing_object_creates_it_or_is_refused`. Commit 8b949a13ef5.

### §7.8.3.1 All representations with reference and XML

**Spec:** §7.8.3.1, p. 50 — "the representations for the different kinds of
objects all derive from either OBJK_Representation2_Base or
OBJK_Representation3_Base. Therefore, they all have at least the
REPRESENTATION_BY_REFERENCE and the REPRESENTATION_AS_XML_STRING."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`format_is_allowed` l. 279,
`handle_create` l. 1508); `crates/xrce-agent/src/daemon/config.rs`
(`representation_formats`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_formats_of_each_kind_are_enforced`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_representation_formats_can_be_widened`

**Status:** `done` — the spec contradicts itself: the text says all
representations have `REPRESENTATION_BY_REFERENCE` and
`REPRESENTATION_AS_XML_STRING`; §7.7.3.1 and Annex A give Type, QosProfile,
Application and Domain reference or XML (`OBJK_RepresentationRefAndXML_Base`),
Publisher, Subscriber, DataWriter and DataReader binary or XML
(`OBJK_RepresentationBinAndXML_Base`), DomainParticipant and Topic all three.
There is no `OBJK_Representation2_Base`. The default is Annex A
(`representation_formats: spec`): any other format gets STATUS_ERR_INVALID_DATA.
The named setting `representation_formats: any` takes every format, for clients
that create Publishers, Subscribers, DataWriters and DataReaders by reference;
not measured against such a client. Commits bdf3f6ee966, f0995555d4a.

### §7.8.3.1 Nested objects

**Spec:** §7.8.3.1, p. 50 — "Certain representations support the
representation of nested objects. [...] In this case, the creation of the XRCE
object shall also create the nested objects and the failure to create any
nested entity shall be considered a failure to create the contained entity as
well."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_nested` l. 1833,
`create_embedded` l. 1785, `create_core` l. 1743–1759)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_creates_the_objects_inside_it`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_with_a_broken_part_creates_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`

**Status:** `done` — a DomainParticipant or an Application creates the objects
its definition holds (DDS-XML), and so does a Publisher or Subscriber with its
embedded DataWriters and DataReaders. If a child fails, the whole fails: what
was created is removed (`remove_with_nested`) and the previous object comes
back. Commits 3aea52dd70a, 9239562286c.
### §7.8.3.1 A proxy creates the DDS entity

**Spec:** §7.8.3.1, p. 50 — "Some of the XRCE objects may be defined by this
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
DataWriter and DataReader have a DDS entity. Publisher and Subscriber are made
by `DdsPort::create_publisher` and `create_subscriber` (PRESENTATION,
ENTITY_FACTORY, PARTITION, GROUP_DATA are applied). QosProfile, Type and
Application have no DDS entity in Table 6.

### §7.8.3.1 Failure of the DDS entity is failure of the object

**Spec:** §7.8.3.1, p. 50 — "Failure to create a DDS Entity shall be
considered a failure to create the proxy XRCE object as well."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core`,
`apply_dds_create`, `remove_with_nested`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_dds_entity_is_enabled_when_the_creation_has_succeeded`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_of_an_unknown_type_is_refused_when_the_agent_is_told_to`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_participant_definition_with_a_broken_part_creates_nothing`

**Status:** `done` — Commits `e4d37a6aff6`, `3aea52dd70a`. If `apply_dds_create` or
`DdsPort::enable` fails, `create_core` deletes the DDS side and the store entry
and answers with the code of the error. If a nested object fails, the whole
object is removed (`remove_with_nested`).

### §7.8.3.1 No discovery traffic on failure, entities disabled
**Spec:** §7.8.3.1, p. 50 — "If the creation of the XRCE object fails then
there should be no associated DDS-RTPS discovery traffic generated by the
Agent. This means that all DDS entities shall be created disabled [...] and
enabled (if so configured by their QoS) only after it has been determined that
the creation has succeeded."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`enable`, `create_participant_with`); `crates/xrce-agent/src/daemon/session.rs` (`create_core`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_participant_qos_is_applied_to_the_participant`, `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::an_endpoint_is_announced_only_when_it_is_enabled`

**Status:** done — DataWriters, DataReaders and the DomainParticipant are registered with the `DcpsRuntime`, and so discovered, only by `DdsPort::enable`; `create_core` calls `enable` when the object is stored and nothing can fail any more. An object that fails in creation or in `enable` is never announced. e4d37a6aff6, 232eb724129.
### §7.8.3.1 DomainParticipant is created disabled
**Spec:** §7.8.3.1, p. 50 — "This means that all DDS entities shall be created
disabled [...] and enabled (if so configured by their QoS) only after it has
been determined that the creation has succeeded."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`create_participant_with`, `enable`, `runtime_for`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_participant_qos_is_applied_to_the_participant`

**Status:** done — `create_participant_with` no longer starts a runtime; it records the participant (for USER_DATA the slot of a runtime of its own), `enable` starts it when the CREATE succeeds. A domain id above 232 is refused at CREATE. 232eb724129.
### §7.8.3.1 Success yields STATUS_OK

**Spec:** §7.8.3.1, p. 50 — "If the creation succeeds the Agent shall set the
StatusValue within returnStatus to STATUS_OK.."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`,
`create_core`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done — A successful creation is STATUS_OK; with `reuse` and an equal
representation STATUS_OK_MATCHED (Table 5 spells STATUS_OK_MATCHES; only the
name in the code differs).

### §7.8.3.1 Table 6 OBJK_QOSPROFILE

**Spec:** §7.8.3.1, Table 6, p. 51 — "The agent shall use that definition to
create an XRCE QosProfile in accordance to the representation defined in
7.7.3.2."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`definition_to_register`, `rebuild_profiles`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`

**Status:** `done` — Commits `3aea52dd70a`, `9239562286c`. The object has no DDS entity.
A `<qos_library>` with one `<qos_profile>` is known to the agent as a definition
for as long as the object exists; other clients create it by
REPRESENTATION_BY_REFERENCE. A reference resolves against the profile files and
these definitions.

### §7.8.3.1 Table 6 OBJK_TYPE
**Spec:** §7.8.3.1, Table 6, p. 51 — "The agent shall use that definition to
create an XRCE Type in accordance to the representation defined in 7.7.3.3."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`types`); `crates/xrce-agent/src/daemon/session.rs` (`find_type_definition`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_type_by_reference_names_a_known_type_by_its_fully_qualified_name`

**Status:** done — XML `<types>` (exactly one non-nested type) and a binary `TypeObject` create types; a reference resolves against the types known to the agent, and a type may refer to a type already known (item on §7.7.3.3.2).
### §7.8.3.1 Table 6 OBJK_APPLICATION

**Spec:** §7.8.3.1, Table 6, p. 51 — "The agent shall use that definition to
create an XRCE Application with all the contained entities found within the
definition in accordance to the representation defined in 7.7.3.5."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`create_nested`, `ensure_predefined`);
`crates/xrce-agent/src/daemon/definitions.rs` (`plan_participant`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`

**Status:** `done` — Commits `9239562286c`, `b6e55cfa859`. `create_nested` makes the
DomainParticipants of the Application with the objects inside them under the
ObjectIds of their names (Table 16); a second creation reuses the objects that
exist.

### §7.8.3.1 Table 6 OBJK_PARTICIPANT

**Spec:** §7.8.3.1, Table 6, p. 51 — "The agent shall use that definition to
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

**Status:** `done` — Commits `3aea52dd70a`, `b4cd825c268`. The participant binds the
`DcpsRuntime` of its domain. The entities in the definition are made with it; if
one fails, the whole participant is removed. `domain_reference` and
`qos_profile_reference` of the binary form are resolved.

### §7.8.3.1 Table 6 OBJK_TOPIC

**Spec:** §7.8.3.1, Table 6, p. 51 — "The agent shall locate the XRCE
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

**Status:** `done` — Commits `0da6aef56b5`. In the `DcpsRuntime` the topic is the name
and type of its endpoints. A type the agent does not know follows the agent key
`unknown_topic_types`: `refuse` (default) answers STATUS_ERR_UNKNOWN_REFERENCE
and creates nothing, `forward` creates the topic and forwards the samples
undecoded.

### §7.8.3.1 Table 6 OBJK_PUBLISHER

**Spec:** §7.8.3.1, Table 6, p. 51–52 — "The agent shall locate the XRCE
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

**Status:** `done` — Commits `dd55e9c9df1`. A participant the client does not hold gives
STATUS_ERR_UNKNOWN_REFERENCE; a `participant_id` of another kind
STATUS_ERR_INVALID_DATA; nothing is stored. The DDS Publisher is made under the
participant with PRESENTATION, ENTITY_FACTORY, PARTITION and GROUP_DATA.

### §7.8.3.1 Table 6 OBJK_SUBSCRIBER

**Spec:** §7.8.3.1, Table 6, p. 51–52 — as for the publisher, for the
subscriber.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`,
`participant_of_create`, `DdsCreate::Container`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::create_subscriber`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_or_subscriber_needs_an_existing_participant`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_and_a_subscriber_get_a_dds_entity_with_their_policies`,
`crates/xrce-agent/src/daemon/dds_port.rs::tests::publisher_and_subscriber_need_their_participant_and_endpoints_need_them`

**Status:** `done` — Commits `dd55e9c9df1`. As for the Publisher, for the Subscriber.

### §7.8.3.1 Table 6 OBJK_DATAWRITER

**Spec:** §7.8.3.1, Table 6, p. 52 — "The agent shall locate the XRCE
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

**Status:** done — The writer inherits PARTITION and GROUP_DATA of the
Publisher. A Publisher the client does not hold gives
STATUS_ERR_UNKNOWN_REFERENCE; Publisher and topic must belong to the same
participant.

### §7.8.3.1 Table 6 OBJK_DATAREADER

**Spec:** §7.8.3.1, Table 6, p. 52 — "OBJK_DATEREADER The ObjectVariant is a
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

**Status:** done — The row of the spec writes `OBJK_DATEREADER` and
`DATAWRITER_Representation`; `OBJK_DATAREADER` and `DATAREADER_Representation`
are meant. The reader inherits PARTITION and GROUP_DATA of the Subscriber; a
missing Subscriber gives STATUS_ERR_UNKNOWN_REFERENCE.

### §7.8.3.2 update of an existing object

**Spec:** §7.8.3.2, p. 52 — "This operation shall attempt to update an
existing object in the XRCE Agent. If the object exists and the update is
successful STATUS_OK shall be returned".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core`);
`crates/xrce/src/object_store.rs` (`ObjectStore::create`);
`crates/xrce-client/src/lib.rs` (`XrceClient::update_object`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`,
`crates/xrce-client/src/lib.rs::tests::update_object_sends_a_create_with_reuse_and_replace`

**Status:** `done` — Commits `8b949a13ef5`. Table 14 and §8.4.13 map update to CREATE
with `reuse` and `replace`. An equal representation (container and `domain_id`
included) gives STATUS_OK_MATCHED and changes nothing; a different one replaces
the object with its DDS side (STATUS_OK).

### §7.8.3.2 update without an object

**Spec:** §7.8.3.2, p. 52 — "If the object does not already exist
STATUS_ERR_UNKNOWN_REFERENCE shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core`);
`crates/xrce-agent/src/daemon/config.rs` (`update_of_missing_object`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_update_of_a_missing_object_creates_it_or_is_refused`

**Status:** `done` — Commits `8b949a13ef5`. The spec contradicts itself: §7.8.3.2
requires STATUS_ERR_UNKNOWN_REFERENCE, Table 5 and §8.4.13 let the same CREATE
make the object. The default is Table 5 (create); the agent key
`update_of_missing_object: refuse` gives STATUS_ERR_UNKNOWN_REFERENCE.

### §7.8.3.2 update with invalid parameters, return to the old configuration

**Spec:** §7.8.3.2, p. 52 — "If the update was unsuccessful due to invalid
parameters, STATUS_ERR_INVALID_DATA shall be returned. If an update is
unsuccessful the referenced object shall return to its previous
configuration."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core`,
`refuse_create`, `restore_previous`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_replace_that_fails_returns_the_object_to_its_previous_configuration`

**Status:** `done` — Commits `e4d37a6aff6`. A representation no object can be made from
is refused with its STATUS before anything changes (STATUS_ERR_INVALID_DATA for
a malformed representation, STATUS_ERR_UNKNOWN_REFERENCE for a reference that
does not resolve). If the new DDS side fails, `restore_previous` restores the
store entry and the DDS side from the stored plan.

### §7.8.3.2 update without permission

**Spec:** §7.8.3.2, p. 52 — "If the object cannot be updated due to permission
restrictions, STATUS_ERR_DENIED shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`create_core`, `permits`);
`crates/xrce-agent/src/daemon/access.rs` (`Operation::Update`,
`AccessController::permits`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::an_update_needs_the_permission_to_update`,
`crates/xrce-agent/src/daemon/session.rs::tests::replacing_an_object_needs_the_permission_to_delete_it`

**Status:** `done` — Commits `515bcc677f3`, `8b949a13ef5`. An update (CREATE with `reuse`
and `replace`) needs the permission `Operation::Update`, a replace without
`reuse` the permission `Operation::Delete`; otherwise STATUS_ERR_DENIED.

### §7.8.3.3 get_info of an object

**Spec:** §7.8.3.3, p. 52 — "object_info (ObjectInfo): contains the current
activity and configuration of the specified object. This operation returns the
configuration and activity data for an existing object."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`,
`object_info`, `answer_info`); `crates/xrce/src/object_info.rs` (`ObjectInfo`);
`crates/xrce/src/submessages/info.rs` (`InfoPayload`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_writer_reports_its_activity`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_reader_reports_its_activity`,
`crates/xrce-agent/src/daemon/session.rs::tests::any_object_id_of_kind_agent_is_the_root_in_a_session`

**Status:** `done` — Commits `65cd1820a04`. The answer is an INFO with `activity`
(DataWriter, DataReader) and `config` (the ObjectVariant the object was created
with); the InfoMask selects. `sample_seq_num`, `stream_seq_num` and
`highest_acked_num` are counted by the agent itself; the spec does not define
the values, and a comparison with another agent was not measured.

### §7.8.3.3 get_info without an object

**Spec:** §7.8.3.3, p. 53 — "If the object does not already exist
STATUS_ERR_UNKNOWN_REFERENCE shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`,
`answer_info`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`

**Status:** `done` — Commits `65cd1820a04`. The agent answers with an INFO whose
ResultStatus is STATUS_ERR_UNKNOWN_REFERENCE and that carries the `request_id`
and `object_id` of the request (§8.3.5.7.2: the answer to GET_INFO is INFO).

### §7.8.3.3 get_info without permission

**Spec:** §7.8.3.3, p. 53 — "If the object cannot be accessed due to
permission restrictions STATUS_ERR_DENIED shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`);
`crates/xrce-agent/src/daemon/access.rs` (`Operation::GetInfo`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`

**Status:** `done` — Commits `515bcc677f3`. Without the permission `Operation::GetInfo`
the agent answers with an INFO with STATUS_ERR_DENIED; the object stays.

### §7.8.3.4 delete of an object

**Spec:** §7.8.3.4, p. 53 — "This operation deletes an existing object. If the
object is successfully deleted STATUS_OK shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::delete_fixture_decodes_and_removes_the_object_create_added`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** done — The spec gives the input `object_id` the type
`ObjectIdPrefix`. DELETE(OBJECTID_CLIENT) ends the session and deletes the
ProxyClient (`ed2ca7b49a9`).

### §7.8.3.4 delete without an object
**Spec:** §7.8.3.4, p. 53 — "If the object does not exist
STATUS_ERR_UNKNOWN_REFERENCE shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::delete_of_an_object_that_does_not_exist_is_an_unknown_reference`, `crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** done — `handle_delete` answers STATUS_ERR_UNKNOWN_REFERENCE when no object was removed; for DELETE(OBJECTID_CLIENT) without a ProxyClient (`ed2ca7b49a9`) and for an ordinary object (0f85937a436).
### §7.8.3.4 delete without permission

**Spec:** §7.8.3.4, p. 53 — "If the object cannot be deleted due to permission
restrictions, STATUS_ERR_DENIED shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`);
`crates/xrce-agent/src/daemon/access.rs` (`Operation::Delete`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::replacing_an_object_needs_the_permission_to_delete_it`

**Status:** `done` — Commits `515bcc677f3`. Without the permission `Operation::Delete`
the agent answers STATUS_ERR_DENIED; the object stays.

---

## §7.8.4 XRCE DataWriter

### §7.8.4 Table 7 write operation

**Spec:** §7.8.4, Table 7, p. 53 — `write` with `object_id` and `data`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview.

### §7.8.4.1 write writes one or more samples

**Spec:** §7.8.4.1, p. 53 — "This operation writes one or more samples using
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
FORMAT_SAMPLE_SEQ and FORMAT_PACKED_SAMPLES are written. The elements of a
sequence lie back to back without a length (Annex A `sequence<SampleData>`); the
type of the topic (`DdsPort::sample_length`) tells them apart. Assumption, not
measured against another agent: there is no padding between the elements, each
is read from alignment origin 0 (Annex B.5.2 shows 4-byte elements only).

### §7.8.4.1 Sequence formats when writing

**Spec:** §7.8.4.1, p. 53 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`split_write_items`,
`handle_write_data`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::sample_length`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_data_seq_is_split_by_the_type_and_written_sample_by_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sequence_that_does_not_split_writes_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sequence_of_a_topic_of_unknown_type_is_refused`

**Status:** `done` — Commits `4f1fb9f5f4f`. The three sequence formats are written sample
by sample. A sequence that does not split (count larger than the bytes, bytes
behind the last sample, truncated sample) writes nothing and gives
STATUS_ERR_INVALID_DATA. The same holds for a topic of unknown type (possible
only with `unknown_topic_types: forward`) whose samples cannot be told apart.

### §7.8.4.1 Success yields STATUS_OK

**Spec:** §7.8.4.1, p. 53 — "If the data is successfully written STATUS_OK
shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_successful_write_is_answered_with_status_ok`

**Status:** `done` — Commits `dc0209b38fe`. A successful WRITE_DATA is answered with a
STATUS that names the `request_id` and DataWriter of the request; several
samples in one request give one STATUS. The capture of the eProsima agent holds
no answer to WRITE_DATA; the text of the spec (§8.3.5.8, Table 14) is followed.

### §7.8.4.1 Unknown DataWriter

**Spec:** §7.8.4.1, p. 53 — "If the XRCE DataWriter object identified by the
object_id does not exist, the ResultStatus STATUS_ERR_UNKNOWN_REFERENCE shall
be returned."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`);
`crates/xrce-agent/src/daemon/session.rs` (`dds_write_and_report`,
`dds_port_error_status`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::full_chain_against_a_real_leased_domain`

**Status:** done — `RealDdsPort::write` returns `DdsPortError::UnknownReference`
for a writer that does not exist; the session answers
STATUS_ERR_UNKNOWN_REFERENCE.

### §7.8.4.1 write without permission

**Spec:** §7.8.4.1, p. 53 — "If the client is not allowed to write data using
the referenced object_id due to permission restrictions, the ResultStatus
STATUS_ERR_DENIED shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`,
`permit`); `crates/xrce-agent/src/daemon/access.rs` (`Operation::WriteData`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::write_and_read_without_permission_are_denied`

**Status:** `done` — Commits `515bcc677f3`. Without the permission `Operation::WriteData`
the agent answers STATUS_ERR_DENIED; nothing reaches the DDS side.

### §7.8.4.1 Invalid data

**Spec:** §7.8.4.1, p. 53 — "If the data could not be written successfully
due, for example invalid data format, the ResultStatus
STATUS_ERR_INVALID_DATA shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`,
`refuse_unreadable`, `dds_port_error_status`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_request_that_cannot_be_read_is_answered_invalid_data`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sequence_that_does_not_split_writes_nothing`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_dispose_the_agent_cannot_identify_is_answered_with_a_status`

**Status:** `done` — Commits `133a0db3c4f`, `4f1fb9f5f4f`. A WRITE_DATA whose body cannot
be read, a sequence that does not split, and a dispose with no identifiable
instance give STATUS_ERR_INVALID_DATA. If the known type of the topic does not
read a payload, `RealDdsPort::write` returns `DdsPortError::InvalidType` (also
STATUS_ERR_INVALID_DATA); this branch has no test of its own.

### §7.8.4.1 Timestamp in SampleInfo

**Spec:** §7.8.4.1, p. 53 — "The DataRepresentation type also supports sending
sample information in addition to the data. This is encoded in the SampleInfo
type (see 7.7.1) allowing sending timestamps and also notifications of dispose
and unregister."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`split_write_items`,
`source_time_of`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::write`, `he_timestamp`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_is_stamped_with_the_session_time_plus_its_offset`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_seq_carries_state_and_time_per_sample`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::known_keyed_type_gets_a_real_keyhash_and_honors_dispose`

**Status:** `done` — Commits `4f1fb9f5f4f`. The flags for dispose and unregister and
`session_time_offset` are read from SampleInfo (FORMAT_SAMPLE,
FORMAT_SAMPLE_SEQ) and SampleInfoDelta (FORMAT_PACKED_SAMPLES); the timestamp
reaches the DDS writer as the source time.

### §7.8.4.1 session_time_offset as the source time

**Spec:** §7.8.4.1, p. 53 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`source_time_of`,
`handle_create_client`); `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::write`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_is_stamped_with_the_session_time_plus_its_offset`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_seq_carries_state_and_time_per_sample`

**Status:** `done` — Commits `4f1fb9f5f4f`. The source time is the session time plus
`session_time_offset`. The session time is the agent's clock when the session
was created: Annex A `CLIENT_Representation` has no `client_timestamp`. With
`representation_timestamps: present` the agent reads a `client_timestamp` but
does not use it as the session time. FORMAT_DATA carries no SampleInfo and is
not stamped.

### §7.8.4.1 dispose
**Spec:** §7.8.4.1, p. 54 — "If the DataRepresentation contains a Sample where
the SampleInfo has the 'dispose' flag set, the XRCE Agent shall call the
dispose operation on the corresponding DDS DataWriter for the instance
identified in the associated data."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`); `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::dispose_on_a_topic_of_unknown_type_is_refused`, `crates/xrce-agent/src/daemon/session.rs::tests::a_dispose_the_agent_cannot_identify_is_answered_with_a_status`, `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done — for a known keyed type the agent calls `write_user_lifecycle` with the flag DISPOSED and the real KeyHash (4f1fb9f5f4f). The specification asks for the dispose "for the instance identified in the associated data"; a type without a key or an unknown type identifies no instance, the data cannot be interpreted by the agent: STATUS_ERR_INVALID_DATA (next item).
### §7.8.4.1 dispose without a known type
**Spec:** §7.8.4.1, p. 54 — as above.

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`); `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::dispose_on_a_topic_of_unknown_type_is_refused`, `crates/xrce-agent/src/daemon/session.rs::tests::a_dispose_the_agent_cannot_identify_is_answered_with_a_status`, `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done — for a topic of unknown type or of a type without `@key` the agent cannot determine the instance from the data (`SerializedTypeSupport::key_material` is `None`); an instance the specification wants identified does not exist. `RealDdsPort::write` writes nothing and returns `DdsPortError::InvalidType`; the session answers STATUS_ERR_INVALID_DATA, the specification's status for data the agent cannot interpret, and reports `SessionEvent::DdsPortFailed` (4f1fb9f5f4f).
### §7.8.4.1 unregister
**Spec:** §7.8.4.1, p. 54 — "Similarly there is a Sample where the SampleInfo
has the 'unregister' flag set, the XRCE Agent shall call the unregister
operation on the corresponding DDS DataWriter for the instance identified in
the associated data."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::write`); `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::dispose_on_a_topic_of_unknown_type_is_refused`, `crates/xrce-agent/src/daemon/session.rs::tests::a_dispose_the_agent_cannot_identify_is_answered_with_a_status`, `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done — for a known keyed type the agent calls `write_user_lifecycle` with the flag UNREGISTERED; the native reader of the e2e test sees the unregistration (4f1fb9f5f4f). For a type without a key or without a known type STATUS_ERR_INVALID_DATA applies as for dispose.

---

## §7.8.5 XRCE DataReader

### §7.8.5 Table 8 read operation

**Spec:** §7.8.5, Table 8, p. 54 — `read` with `object_id`,
`read_specification` and the output `read_data`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview.

### §7.8.5.1 read reads samples

**Spec:** §7.8.5.1, p. 54 — "This operation reads one or more samples from the
XRCE DataReader identified by the object_id. If the data is successfully read
STATUS_OK shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`pump_read`, `data_submessages`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::read_data_elapsed_time_is_measured_from_the_real_request_time`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_data_seq_is_answered_with_one_data_seq`

**Status:** `done` — Commits `2fa0800f55a`, `d5fbdbe672e`. Success is DATA (§8.3.5.10) in
the requested `data_format`.

### §7.8.5.1 read without an object

**Spec:** §7.8.5.1, p. 54 — "If the object does not exist
STATUS_ERR_UNKNOWN_REFERENCE shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`);
`crates/xrce-agent/src/daemon/dds_port.rs` (`NullDdsPort::set_read_filter`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort::set_read_filter`)

**Tests:** `crates/xrce-agent/src/daemon/dds_port.rs::tests::null_port_rejects_content_filter_but_accepts_clearing_it`

**Status:** done — A reader that does not exist makes `DdsPort::set_read_filter`
return `DdsPortError::UnknownReference`; `handle_read_data` answers
STATUS_ERR_UNKNOWN_REFERENCE and starts no read.

### §7.8.5.1 read without permission

**Spec:** §7.8.5.1, p. 54 — "If the client is not allowed to read data using
the referenced object_id due to permission restrictions, STATUS_ERR_DENIED
shall be returned."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`permit`); `crates/xrce-agent/src/daemon/access.rs` (`Operation::ReadData`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::write_and_read_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/session.rs::tests::read_with_permission_is_not_denied`

**Status:** `done` — Commits `515bcc677f3`. Without the permission `Operation::ReadData`
the agent answers STATUS_ERR_DENIED and starts no read.

### §7.8.5.1 Table 9 preferred_stream_id

**Spec:** §7.8.5.1, Table 9, p. 54 — "Specifies the StreamId the Agent should
use to send the Samples to the Client. The special value STREAMID_NONE
indicates that Agent can choose the StreamId unconstrained. The Agent is
allowed to use an StreamId different from the preferred_stream_id if it
determines that using the preferred_stream_id could cause problems with its
resource-management."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`pump_read`, `ActiveRead`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_above_the_clients_mtu_on_a_reliable_stream_splits_into_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::streamid_none_lets_the_agent_choose_the_stream`

**Status:** `done` — Commits `2fa0800f55a`. DATA goes on the requested stream; for
STREAMID_NONE the agent chooses STREAMID_BUILTIN_BEST_EFFORTS.

### §7.8.5.1 STREAMID_NONE as the requested stream

**Spec:** §7.8.5.1, Table 9, p. 54 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::streamid_none_lets_the_agent_choose_the_stream`

**Status:** `done` — Commits `2fa0800f55a`. The agent chooses the built-in best-effort
stream 0x01; DATA does not go on STREAMID_NONE.

### §7.8.5.1 Table 9 data_format

**Spec:** §7.8.5.1, Table 9, p. 55 — "data_format DataFormat Selects one the
data formats. See 7.7.1"; p. 55: "The setting of the data_format controls
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

**Status:** `done` — Commits `d5fbdbe672e`. DATA comes in FORMAT_DATA, FORMAT_SAMPLE,
FORMAT_DATA_SEQ, FORMAT_SAMPLE_SEQ or FORMAT_PACKED_SAMPLES as requested; the
sequence formats split at the size of a submessage (PACKED_SAMPLES at most 256
samples). The SampleInfo values are only what the agent knows: `sequence_number`
counts the delivered samples, `session_time_offset` is the time of delivery,
`state` is 0 because `DdsPort::poll_samples` hands over payload bytes only (no
source timestamp, no instance, view or sample state).

### §7.8.5.1 Table 9 content_filter_expression

**Spec:** §7.8.5.1, Table 9, p. 55 — "A content filter expression selecting
which data to read. The syntax shall be as specified in Annex B (Syntax for
Queries and Filters) of the DDS specification [DDS]." p. 55: "Only samples for
which the filter evaluates to TRUE shall be returned to the XRCE Client."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs`
(`RealDdsPort::set_read_filter`, `CompiledReadFilter`, `drain_buffer`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`);
`crates/xrce/src/submessages/read_data.rs` (`ReadSpecification`)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::a_content_filter_admits_an_xcdr2_sample`,
`crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`,
`crates/xrce/src/submessages/read_data.rs::tests::the_content_filter_is_a_string_with_a_nul_and_an_aligned_length`

**Status:** `done` — Commits `1663f8f6240`. The encoding on the wire is an XCDR2 string
with a NUL. A filter the agent cannot apply gives a STATUS and delivers no
unfiltered data.

### §7.8.5.1 Table 9 max_samples

**Spec:** §7.8.5.1, Table 9, p. 55 — "Maximum number of samples to return as a
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

**Status:** done — The limit is counted; the agent takes from the DataReader
only the samples the read still delivers. The text on `MAX_SAMPLES_UNLIMITED`
says “there is limit”; §8.3.5.9.2 (p. 71) says “no limit”, which the code
follows.

### §7.8.5.1 MAX_SAMPLES_ZERO and MAX_SAMPLES_UNLIMITED

**Spec:** §7.8.5.1, Table 9, p. 55; §8.3.5.9.2, p. 71 — as above.

**Repo:** `crates/xrce/src/continuous_read.rs` (`MAX_SAMPLES_ZERO`,
`MAX_SAMPLES_UNLIMITED`); `crates/xrce-agent/src/daemon/session.rs`
(`handle_read_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::max_samples_zero_cancels_the_active_read`,
`crates/xrce/src/continuous_read.rs::tests::max_samples_zero_ends_the_read_and_unlimited_has_no_limit`

**Status:** `done` — Commits `2fa0800f55a`. 0 ends the running read of the DataReader
without further DATA; 0xFFFF reads without a limit.

### §7.8.5.1 Table 9 max_elapsed_time
**Spec:** §7.8.5.1, Table 9, p. 55 — "Maximum amount of time in seconds that
may be spent delivering the samples from the read operation. [...] The special
value MAX_ELAPSED_TIME_UNLIMITED = 0 indicates there is no maximum".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`elapsed_time_limit`, `handle_read_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::max_elapsed_time_is_in_seconds_or_in_the_milliseconds_of_the_example`

**Status:** done — the agent reads `max_elapsed_time` as seconds (Table 9, the definition; 0 is MAX_ELAPSED_TIME_UNLIMITED) and measures from the time of the request. Annex B.4.2 (an example) writes 30000 for 30 seconds; that reading is the setting `max_elapsed_time_unit: milliseconds`. e94545ab3bd.
### §7.8.5.1 max_elapsed_time in milliseconds after Annex B.4.2
**Spec:** Annex B.4.2, p. 152 — "over a time window not to exceed 30 seconds"
with `max_elapsed_time = 30000 = 0x7530`; against it Table 9, p. 55: "Maximum
amount of time in seconds".

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`max_elapsed_time_unit`); `crates/xrce-agent/src/daemon/session.rs` (`SessionEngineConfig::max_elapsed_time_in_milliseconds`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::max_elapsed_time_is_in_seconds_or_in_the_milliseconds_of_the_example`, `crates/xrce-agent/src/daemon/config.rs::tests::the_unit_of_max_elapsed_time_can_be_set`

**Status:** done — the reading of the Annex B example (milliseconds) is the setting `max_elapsed_time_unit: milliseconds`. e94545ab3bd.
### §7.8.5.1 Table 9 max_bytes_per_sec

**Spec:** §7.8.5.1, Table 9, p. 55 — "Maximum rate in bytes per second at which
the data may be returned to the read operation."

**Repo:** `crates/xrce/src/continuous_read.rs` (`ReadStream`,
`ReadStream::pull_pending_samples`)

**Tests:** `crates/xrce/src/continuous_read.rs::tests::rate_limit_partitions_samples_over_time`

**Status:** done — The field is named `max_bytes_per_second` in the IDL.

### §7.8.5.1 Table 9 min_pace_period

**Spec:** §7.8.5.1, Table 9, p. 55 — "Minimum separation between data messages
returned from the read operation in milliseconds." p. 55: "This period makes
it possible for the client to go into a sleep cycle between messages."

**Repo:** `crates/xrce/src/continuous_read.rs`
(`ReadStream::pull_pending_samples`)

**Tests:** `crates/xrce/src/continuous_read.rs::tests::pacing_throttles_per_period`

**Status:** done

---

## §8.1 General

### §8.1 Properties of the protocol

**Spec:** §8.1, p. 57 — bandwidth 40–100 Kbit/s, sleep cycles, language
independence, minimal discovery, full access to DDS, updates of several
topics, reliable and best-effort delivery, security at the transport level,
access to all data, "Provide a full implementation requiring less than 100KB
of code."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — design goals; the code size is not
measured.

### §8.1 XRCE clients without a standard API and without peer-to-peer

**Spec:** §8.1, p. 57 — "In contrast to applications that use the DDS API
directly, XRCE Clients: Do not have a standard API [...]. Cannot operate
without infrastructure support. [...] Do not communicate directly
peer-to-peer."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description.

---

## §8.2 Definitions

### §8.2 Terms client, agent, session, message

**Spec:** §8.2, p. 57 — "At a high level, a client communicates with an agent
using the DDS-XRCE protocol, exchanging messages on a stream belonging to a
session."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — terms.

### §8.2.1 One transport message carries one XRCE message

**Spec:** §8.2.1, p. 57 — "The underlying XRCE Transport shall transfer each
message as a unit. A single XRCE Transport 'message' shall transport a single
XRCE message."

**Repo:** `crates/xrce/src/transport_udp.rs` (`send_message`, `recv_message`);
`crates/xrce/src/transport_tcp.rs` (`XrceTcpClient::send_message`,
`recv_message`)

**Tests:** `crates/xrce/src/transport_udp.rs::tests::rfc768_udp_delivers_one_datagram_per_message_with_boundaries_intact`,
`crates/xrce/src/transport_tcp.rs::tests::rfc793_byte_stream_is_delimited_by_the_length_prefix_not_by_writes`

**Status:** done

### §8.2.1 Alignment from the first byte of the message

**Spec:** §8.2.1, p. 57 — "XRCE messages shall be encoded assuming the first
byte has a 16-byte alignment. Therefore, the encoding is independent of any
transport heading or prefix that may precede it."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`Message::encode`,
`Message::decode`); `crates/xrce-agent/src/daemon/binary_forms.rs`
(`participant`, `topic`, `container`, `endpoint`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`Cdr`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::message_two_submessages_padded_correctly`

**Status:** done — The codecs of the payloads align relative to the start of
their structure; these starts lie on multiples of 4. `binary_forms.rs` reads the
binary forms of §7.7.3 as XCDR2 (alignment up to 4) relative to the start of the
`sequence<octet>`; `entity_repr.rs::Cdr` reads the structures of eProsima as
classic CDR (alignment up to 8) from the same start.

### §8.2.2 Session with a handshake

**Spec:** §8.2.2, p. 58 — "A session defines a bi-directional connection
between a client and an agent that has been established with a handshake."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`);
`crates/xrce-client/src/lib.rs` (`XrceClient::connect`, `handle_status_agent`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::udp_listener_answers_create_client_over_loopback`,
`crates/xrce-client/src/lib.rs::tests::connect_sends_a_real_create_client_and_moves_to_connecting`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`

**Status:** done — The agent answers CREATE_CLIENT with STATUS_AGENT; a wrong
cookie gives STATUS_AGENT with STATUS_ERR_INVALID_DATA (`ed2ca7b49a9`).

### §8.2.2 Up to 256 streams per session

**Spec:** §8.2.2, p. 58 — "A session can contain independent, reliable, and
best-effort message streams. Each session may have up to 256 streams."

**Repo:** `crates/xrce/src/header.rs` (`StreamId`);
`crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::reliable`)

**Tests:** `crates/xrce/src/header.rs::tests::stream_id_classification`,
`crates/xrce-agent/src/daemon/session.rs::tests::reset_clears_reads_backlog_best_effort_counters_and_every_reliable_stream`

**Status:** `done` — Commits `619184de8a1`. `StreamId` has 256 values; the agent keeps
the state per `(client, stream)`, stream 255 included (RESET clears all).

### §8.2.2 At most one active session

**Spec:** §8.2.2, p. 58 — "There can be at most one active session between an
XRCE client and an XRCE agent. Creation of a new session closes any previous
sessions."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`,
`refusal`, `session_ids`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_different_session_id_resets_object_store`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_message_of_a_replaced_session_is_not_processed`

**Status:** `done` — Commits `619184de8a1`. A new `session_id` replaces the old session;
messages with the old `session_id` are not processed (see the next item).

### §8.2.2 Messages of an ended session

**Spec:** §8.2.2, p. 58 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`SessionEngine::refusal`,
`handle_message`, `SessionEvent::MessageRefused`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_message_of_a_replaced_session_is_not_processed`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_message_without_a_session_is_not_processed_unless_it_is_a_create_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_a_session_id_of_no_session_is_refused`

**Status:** `done` — Commits `619184de8a1`. A message whose `session_id` is not the
active session of the client is not processed and not answered; the agent
reports `SessionEvent::MessageRefused`. The values 0x00 and 0x80 (no session)
are accepted only for CREATE_CLIENT and for GET_INFO (§7.8.2.2).

### §8.2.3 Streams with sequence numbers of their own

**Spec:** §8.2.3, p. 58 — "A stream represents an independent ordered flow of
messages within a session. Messages are ordered within a stream by means of a
sequence number. The sequence numbers used by different streams are
independent of each other."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`reliable`,
`best_effort_seq` per `(client, stream)`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::best_effort_messages_get_increasing_sequence_numbers`

**Status:** done

### §8.2.3 Constant byte order per stream

**Spec:** §8.2.3, p. 58 — "Streams can be reliable or best efforts. Each stream
uses a constant endianness to encode the data in the message/submessage
headers and payload."

**Repo:** `crates/xrce/src/submessages/` (`into_submessage` of all payloads with
`FLAG_E_LITTLE_ENDIAN`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_flags_are_e_le_and_format_data`

**Status:** done — ZeroDDS sends little-endian; on reception every payload follows the
E flag of its submessage (`b43d5e6b848`, `c304e509660`). The code does not
check that the byte order is equal within a stream.

### §8.2.4 The client starts the session

**Spec:** §8.2.4, p. 58 — "An XRCE client is the entity that initiates the
establishment of a session with an XRCE agent."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::connect`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::connect_sends_a_real_create_client_and_moves_to_connecting`

**Status:** done

### §8.2.5 The agent accepts sessions

**Spec:** §8.2.5, p. 58 — "An XRCE agent is the entity that listens to and
accepts requests to establish sessions from XRCE clients."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`run_udp_listener`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::udp_listener_answers_create_client_over_loopback`

**Status:** done

---

## §8.3 Message Structure

### §8.3.1 Header and one or more submessages

**Spec:** §8.3.1, p. 58 — "An XRCE message is composed of a message Header
followed by one or more Submessages and shall be transferred as a unit by the
underlying XRCE Transport."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`Message`, `Message::new`, `encode`,
`decode`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::message_encode_decode_roundtrip_with_key`,
`crates/xrce/tests/foreign_frames.rs::every_client_datagram_decodes_and_writes_back`,
`crates/xrce/tests/wire_roundtrips.rs::a_message_without_a_submessage_is_refused`

**Status:** done — header and submessages. `Message::new`, `encode` and `decode` refuse
a message without a submessage (`619184de8a1`, see the item below).

### §8.3.1 Message without a submessage

**Spec:** §8.3.1, p. 58 — as above.

**Repo:** `crates/xrce/src/submessages/mod.rs` (`Message::new`, `Message::encode`,
`Message::decode`; error `ValueOutOfRange`)

**Tests:** `crates/xrce/tests/wire_roundtrips.rs::a_message_without_a_submessage_is_refused`

**Status:** done — `Message::new` and `encode` with an empty list, and `decode` of a bare
header, return an error (`619184de8a1`). The two tests that fixed the earlier
behaviour now assert the refusal.

### §8.3.2 Layout of the header

**Spec:** §8.3.2, p. 59 — `sessionId` (8 bits), `streamId` (8 bits),
`sequenceNr` (16 bits), `clientKey` (32 bits, "if sessionId <= 127").

**Repo:** `crates/xrce/src/header.rs` (`MessageHeader`, `write_to`, `read_from`)

**Tests:** `crates/xrce/src/header.rs::tests::header_layout_bytes_no_key`,
`crates/xrce/src/header.rs::tests::header_layout_bytes_with_key`,
`crates/xrce/src/header.rs::tests::header_decode_truncated_with_key`

**Status:** done

### §8.3.2.1 sessionId with 8 bits, unique per client

**Spec:** §8.3.2.1, p. 59 — "A session is identified by an 8-bit sessionId. The
sessionId is unique to an XRCE Agent for a given XRCE Client. The sessionId
also determines whether the Header includes a clientKey or not."

**Repo:** `crates/xrce/src/header.rs` (`SessionId`);
`crates/xrce-agent/src/daemon/session.rs` (`session_ids`)

**Tests:** `crates/xrce/src/header.rs::tests::session_id_127_carries_client_key`

**Status:** done

### §8.3.2.1 sessionId 0 to 127 with clientKey

**Spec:** §8.3.2.1, p. 59 — "If the sessionId is between 0 and 127 (0x00 to
0x7f), both included, then the Header shall include the clientKey and the
sessionId is scoped by the clientKey."

**Repo:** `crates/xrce/src/header.rs` (`SessionId::carries_client_key`,
`MessageHeader::with_client_key`)

**Tests:** `crates/xrce/src/header.rs::tests::header_with_client_key_wire_size_8`,
`crates/xrce/src/header.rs::tests::header_constructor_rejects_inconsistent_with_key`

**Status:** done

### §8.3.2.1 sessionId 128 to 255 without clientKey

**Spec:** §8.3.2.1, p. 59 — "If the sessionId is between 128 and 255 (0x80 to
0xff), both included, then the Header shall not include the clientKey and the
sessionId is scoped by the source address of the message."

**Repo:** `crates/xrce/src/header.rs` (`MessageHeader::without_client_key`);
`crates/xrce-agent/src/daemon/transport.rs` (`PeerTable`)

**Tests:** `crates/xrce/src/header.rs::tests::header_no_client_key_wire_size_4`,
`crates/xrce-agent/src/daemon/transport.rs::tests::peer_table_resolves_bound_address`

**Status:** done

### §8.3.2.1 clientKey from the source address

**Spec:** §8.3.2.1, p. 59 — "If the clientKey does not appear explicitly in the
message header, the XRCE Agent must be able to locate it from the source
address of the message (see clause 8.3.2.4)."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`PeerTable::resolve`, `bind`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::peer_table_bootstraps_from_create_client_payload`,
`crates/xrce-agent/src/daemon/transport.rs::tests::a_client_registering_again_is_answered_at_its_new_address`

**Status:** done

### §8.3.2.1 SESSION_ID_NONE_WITH_CLIENT_KEY = 0x00

**Spec:** §8.3.2.1, p. 59 — "The value 0 (0x00) shall be used to indicate the
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

**Status:** done — the client sends CREATE_CLIENT in a header with 0x00 (session 0 to
127) or 0x80; in such a header the agent processes only CREATE_CLIENT (and
GET_INFO, §7.8.2.2) and refuses a CREATE_CLIENT that asks for 0x00 or 0x80 as
its `session_id` (`619184de8a1`).

### §8.3.2.1 SESSION_ID_NONE_WITHOUT_CLIENT_KEY = 0x80

**Spec:** §8.3.2.1, p. 59 — "The value 128 (0x80) shall be used to indicate the
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

**Status:** done — the CREATE_CLIENT of a client with a `session_id` of 128 to 255 goes
in a header with 0x80 and no `clientKey`; the INFO of the discovery uses 0x80
(`619184de8a1`).

### §8.3.2.1 Values for "no session" in the header

**Spec:** §8.3.2.1, p. 59 — as above.

**Repo:** `crates/xrce/src/header.rs` (`SessionId::is_none`, `SessionId::none_like`);
`crates/xrce-agent/src/daemon/session.rs` (`refusal`, `handle_create_client`,
`SessionEvent::MessageRefused`);
`crates/xrce-client/src/lib.rs` (`XrceClient::connect`, `send_control`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_message_without_a_session_is_not_processed_unless_it_is_a_create_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_message_of_a_replaced_session_is_not_processed`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_for_a_session_id_of_no_session_is_refused`,
`crates/xrce-client/src/lib.rs::tests::create_client_travels_in_a_header_without_a_session`

**Status:** done — `XrceClient::connect` sends CREATE_CLIENT with `SESSION_ID_NONE_*` in
the header (0x00 with `clientKey`, 0x80 without); in a header without a
session the agent processes only CREATE_CLIENT and GET_INFO, refuses 0x00 and
0x80 as a requested `session_id`, and drops messages of a replaced session
(`619184de8a1`).

### §8.3.2.2 Streams within the session

**Spec:** §8.3.2.2, p. 59–60 — "Each XRCE message belongs to a single stream.
[...] Streams are scoped by the session they belong to."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`reliable`,
`best_effort_seq` keyed by `(client, stream)`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::best_effort_messages_get_increasing_sequence_numbers`

**Status:** done

### §8.3.2.2 STREAMID_NONE

**Spec:** §8.3.2.2, p. 60 — "The streamId with value 0 (0x00) is referred as
STREAMID_NONE. This stream is used for messages exclusively containing
submessages that do not belong to any stream."

**Repo:** `crates/xrce/src/header.rs` (`StreamId::NONE`)

**Tests:** `crates/xrce/src/header.rs::tests::stream_id_classification`

**Status:** done — the rules per submessage are at §8.3.5.11, §8.3.5.12,
§8.3.5.15 and §8.3.5.16.

### §8.3.2.2 streamId 1 to 127 best-effort

**Spec:** §8.3.2.2, p. 60 — "The streams with streamId between 1 (0x01) and 127
(0x7F), both included, shall be best-effort streams."

**Repo:** `crates/xrce/src/header.rs` (`StreamId::is_best_effort`)

**Tests:** `crates/xrce/src/header.rs::tests::stream_id_classification`

**Status:** done

### §8.3.2.2 streamId 128 to 255 reliable, the leading bit as a flag

**Spec:** §8.3.2.2, p. 60 — "The streams with streamId between 128 (0x80) and
255 (0xFF), both included, shall be reliable streams. Based on the rules above
if the streamId is not STREAMID_NONE, then the leading bit of the streamId can
be interpreted as a flag that indicates the reliability of the stream."

**Repo:** `crates/xrce/src/header.rs` (`StreamId::is_reliable`)

**Tests:** `crates/xrce/src/header.rs::tests::stream_id_classification`

**Status:** done

### §8.3.2.2 Built-in streams 0x01 and 0x80

**Spec:** §8.3.2.2, p. 60 — "There are two built-in streams that are created
whenever a session is created: A built-in best-effort stream identified by a
streamId with value 1 (0x01). [...] A built-in reliable stream identified by a
streamId with value 128 (0x80)."

**Repo:** `crates/xrce/src/header.rs` (`StreamId::BUILTIN_BEST_EFFORT`,
`BUILTIN_RELIABLE`); `crates/xrce-client/src/lib.rs`
(`send_maybe_fragmented`: stream 0x80 or 0x01)

**Tests:** `crates/xrce/src/header.rs::tests::stream_id_classification`,
`crates/xrce-agent/src/daemon/session.rs::tests::reliable_outbound_messages_get_real_incrementing_sequence_numbers`

**Status:** done — a stream's state comes into being on first use.

### §8.3.2.3 STREAMID_NONE without order

**Spec:** §8.3.2.3, p. 60 — "For the stream with streamId STREAMID_NONE, the
sequenceNr does not impose any order; however, it still may be used to discard
duplicate messages."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_message`: the order check
only for best-effort and reliable streams)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::best_effort_streams_drop_duplicates_and_older_messages`

**Status:** done — on STREAMID_NONE the agent checks neither order nor duplicates; the
spec permits discarding duplicates there and does not require it
(`619184de8a1`).

### §8.3.2.3 Order and duplicates on the other streams

**Spec:** §8.3.2.3, p. 60 — "For the stream with streamId different from
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

**Status:** done — reliable streams reorder and discard duplicates through `recv_data`
and `drain_in_order`; best-effort streams discard duplicates and older
messages in the agent and in the client (`619184de8a1`, see the item below).

### §8.3.2.3 Order and duplicates on best-effort streams

**Spec:** §8.3.2.3, p. 60 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`take_best_effort`,
`best_effort_in`, `SessionEvent::BestEffortMessageDiscarded`);
`crates/xrce-client/src/lib.rs` (`ingest_message`, `best_effort_in`,
`ClientEvent::BestEffortMessageDiscarded`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::best_effort_streams_drop_duplicates_and_older_messages`,
`crates/xrce-client/src/lib.rs::tests::a_best_effort_stream_discards_duplicate_and_older_messages`

**Status:** done — a message that is not newer than the last one of its stream (RFC 1982,
16 bit, also across the wrap of the counter) is discarded and reported as an
event; each stream counts for itself (`619184de8a1`).

### §8.3.2.3 Serial number arithmetic with 16 bits

**Spec:** §8.3.2.3, p. 60 — "Addition and comparison of sequence numbers shall
use Serial Number Arithmetic as defined by [IETF RFC-1982] with SERIAL_BITS
set to 16. This implies that the maximum number of outstanding
(unacknowledged) messages for a specific client session stream is limited to
2 15 —that is, 32768."

**Repo:** `crates/xrce/src/serial_number.rs` (`SerialNumber16`,
`wrapping_lt`, `wrapping_cmp`)

**Tests:** `crates/xrce/src/serial_number.rs::tests::lt_across_wrap_boundary`,
`crates/xrce/src/serial_number.rs::tests::undefined_pair_at_exactly_half_window`,
`crates/xrce/src/serial_number.rs::tests::wrap_around_does_not_break_lt_for_consecutive_numbers`

**Status:** done — the sender keeps at most 16 messages open
(`SENDER_WINDOW_CAP`).

### §8.3.2.3 sequenceNr little-endian

**Spec:** §8.3.2.3, p. 60 — "The sequenceNr shall be encoded using little
endian format."

**Repo:** `crates/xrce/src/header.rs` (`write_to`)

**Tests:** `crates/xrce/src/header.rs::tests::header_layout_bytes_no_key`

**Status:** done

### §8.3.2.4 clientKey for sessionId 0 to 127

**Spec:** §8.3.2.4, p. 60 — "The clientKey shall be present on the Header if
the sessionId is between 0 and 127. [...] If the clientKey is present, it shall
contain the ClientKey associated with the XRCE Client."

**Repo:** `crates/xrce-client/src/lib.rs` (`header_client_key`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::every_fixture_shares_the_same_client_key_and_session_id`

**Status:** done

### §8.3.2.4 Deriving the ClientKey from the source address

**Spec:** §8.3.2.4, p. 60 — "If the clientKey is not present, the XRCE Agent
shall be able to derive the ClientKey associated with the XRCE Client from the
source address of the message. This means that the ClientKey has either been
pre-configured on the XRCE Agent for that particular source address, or it has
been exchanged as part of the session establishment."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`PeerTable::bind`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::peer_table_bootstraps_from_create_client_payload`

**Status:** done — through the exchange in CREATE_CLIENT; a pre-configuration
per source address does not exist.

### §8.3.2.4 Protection of the ClientKey by the transport

**Spec:** §8.3.2.4, p. 60 — "Any exchange of the clientKey is protected by the
security mechanisms provided by the XRCE transport."

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`ListenerConfig::Dtls`,
`ListenerConfig::Tls`); `crates/xrce-agent/src/daemon/transport/secure.rs`;
`crates/xrce/src/transport_dtls.rs`, `crates/xrce/src/transport_tls.rs`

**Tests:** `crates/xrce-agent/tests/secure_listeners.rs::tls::a_client_that_trusts_the_agents_certificate_opens_a_session`,
`crates/xrce-agent/tests/secure_listeners.rs::tls::a_plain_text_create_client_on_the_tls_port_is_not_answered`,
`crates/xrce-agent/tests/secure_listeners.rs::dtls::a_client_that_trusts_the_agents_certificate_opens_a_session`,
`crates/xrce-agent/tests/secure_listeners.rs::dtls::a_plain_text_create_client_on_the_dtls_port_is_not_answered`

**Status:** done — listeners `kind: dtls` and `kind: tls` with certificate and key from
PEM files (`01470e273bc`); plain text on that port is not answered. The
features `dtls` and `tls` are not part of `daemon`; a binary needs
`--features dtls,tls`. Secured listeners are not announced in GET_INFO(AGENT)
and the multicast discovery. The DTLS handshake of one peer runs in the
listener's accept task, one at a time.

### §8.3.3 Submessage with header and payload up to 64 KB

**Spec:** §8.3.3, p. 60 — "Following the message header there shall be one or
more submessages. A Submessage shall be composed of a SubmessageHeader and a
payload." Payload "up to to 64 KB".

**Repo:** `crates/xrce/src/submessages/mod.rs` (`Submessage`,
`Submessage::new`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::message_decode_rejects_truncated_body`,
`crates/xrce/src/submessages/mod.rs::tests::message_encode_decode_roundtrip_no_key_single_sm`

**Status:** done

### §8.3.3 Submessages on multiples of 4

**Spec:** §8.3.3, p. 60 — "Submessages shall start at an offset that is a
multiple of 4 relative to the beginning of the Message. This means that
additional padding may be added between the end of a submessage and the
beginning of the next submessage."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`Message::encode`,
`Message::decode`: `UnalignedSubmessage`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::message_two_submessages_padded_correctly`,
`crates/xrce/tests/foreign_frames.rs::every_client_datagram_decodes_and_writes_back`

**Status:** done — after the last submessage the code writes no padding,
eProsima writes it; the decoder accepts both.

### §8.3.4 Layout of the SubmessageHeader

**Spec:** §8.3.4, p. 61 — `submessageId` (8 bits), `flags` (8 bits),
`submessageLength` (16 bits).

**Repo:** `crates/xrce/src/submessages/mod.rs` (`SubmessageHeader`,
`to_bytes`, `from_bytes`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::submessage_header_roundtrip`

**Status:** done

### §8.3.4.1 submessageId

**Spec:** §8.3.4.1, p. 61 — "The submessageId identifies the kind of
submessage. The kinds of submessages are defined in 8.3.5."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`SubmessageId::from_u8`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::submessage_id_rejects_unknown_byte`

**Status:** done — an unknown `submessageId` discards the whole message.

### §8.3.4.2 Endianness bit in the flags

**Spec:** §8.3.4.2, p. 61 — "Bit 0, the 'Endianness' bit, shall indicate the
endianness used to encode the submessage header and payload. If the Endianness
bit is set to 0, the encoding shall be big endian and otherwise little endian.
The flags field for all submessage kinds shall have the Endianness bit.
Specific submessage kinds may define additional flag bits."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`FLAG_E_LITTLE_ENDIAN`,
`SubmessageHeader::body_endianness`); `crates/xrce/src/handshake.rs`,
`crates/xrce/src/object_info.rs`, `crates/xrce/src/transport_locator.rs`
(`encode_with`/`decode_with`); `crates/xrce-agent/src/daemon/session.rs`,
`crates/xrce-client/src/lib.rs` (reading with `body_endianness`)

**Tests:** `crates/xrce/src/submessages/acknack.rs::tests::acknack_be_roundtrip`,
`crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_be_roundtrip`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_big_endian_binary_topic_reads_back`,
`crates/xrce/src/submessages/create_client.rs::tests::create_client_follows_the_e_flag`

**Status:** done — all payloads follow the bit (`b43d5e6b848`, `c304e509660`, see the
item below). The agent writes its own replies little-endian with E=1; each
submessage states its own byte order.

### §8.3.4.2 Payloads and the endianness bit

**Spec:** §8.3.4.2, p. 61 — as above.

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation`,
`AgentRepresentation`, `PropertySeq`: `encode_with`/`decode_with`);
`crates/xrce/src/object_info.rs` (`ObjectInfo`, `ActivityInfoVariant`);
`crates/xrce/src/transport_locator.rs` (ports in the byte order of the flag);
`crates/xrce/src/submessages/create_client.rs`
(`CreateClientPayload::into_submessage_with`)

**Tests:** `crates/xrce/src/handshake.rs::tests::a_big_endian_client_representation_with_properties_reads_back`,
`crates/xrce/src/handshake.rs::tests::a_big_endian_agent_representation_with_properties_reads_back`,
`crates/xrce/src/submessages/create_client.rs::tests::create_client_follows_the_e_flag`,
`crates/xrce/src/transport_locator.rs::tests::medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_big_endian_create_client_with_properties_is_read_right`,
`crates/xrce-client/src/lib.rs::tests::a_big_endian_status_agent_is_read_with_its_properties`

**Status:** done — `ClientRepresentation`, `AgentRepresentation`, `PropertySeq`,
`ObjectInfo`, `ActivityInfoVariant` and the ports of the `TransportLocator`
are read and written in the byte order of the E flag (`b43d5e6b848`,
`c304e509660`). The agent still writes the INFO it builds little-endian with
E=1.

### §8.3.4.3 submessageLength without the header, little-endian

**Spec:** §8.3.4.3, p. 61 — "The submessageLength indicates the length of the
Submessage (excluding the Submessage header). The submessageLength shall be
encoded using little endian format, independent of the value of the flags."

**Repo:** `crates/xrce/src/submessages/mod.rs` (`SubmessageHeader::to_bytes`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::submessage_header_length_is_always_le_even_with_be_body`

**Status:** done

### §8.3.4.4 payload with the types of §7.7

**Spec:** §8.3.4.4, p. 61 — "The definition of the payload shall use the data
types defined in clause 7.7."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure of the specification; the types
are in the items on §7.7.

---

## §8.3.5 Submessage Types

### §8.3.5 Table 10 values of the SubmessageId

**Spec:** §8.3.5, Table 10, p. 62–63 — CREATE_CLIENT 0, CREATE 1, GET_INFO 2,
DELETE 3, STATUS_AGENT 4, STATUS 5, INFO 6, WRITE_DATA 7, READ_DATA 8, DATA 9,
ACKNACK 10, HEARTBEAT 11, RESET 12, FRAGMENT 13, TIMESTAMP 14, TIMESTAMP_REPLY
15.

**Repo:** `crates/xrce/src/submessages/mod.rs` (`SubmessageId`)

**Tests:** `crates/xrce/tests/profile_conformance.rs::spec_1_2_submessage_ids_match_spec_assignment`,
`crates/xrce/src/submessages/mod.rs::tests::submessage_id_roundtrip_all_16_values`

**Status:** done — the text before it says 13 kinds; Table 10 and Figure 7
show 16.

### §8.3.5.1 CREATE_CLIENT from the client

**Spec:** §8.3.5.1, p. 63 — "The CREATE_CLIENT submessage shall be sent by the
XRCE Client to create an XRCE ProxyClient."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::connect`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::connect_sends_a_real_create_client_and_moves_to_connecting`

**Status:** done

### §8.3.5.1 Reception calls create_client

**Spec:** §8.3.5.1, p. 63 — "Reception of this submessage shall result in the
XRCE Agent calling the create_client operation on the XRCE Root object, see
7.8.2.1. The parameters to this operation are obtained from the payload."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`

**Status:** done

### §8.3.5.1 STATUS_AGENT in response

**Spec:** §8.3.5.1, p. 63 — "The XRCE Agent shall send a STATUS_AGENT message
in response, see 8.3.5.5."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`agent_status_reply`,
`handle_create_client`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_client_rejects_wrong_major_version`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`

**Status:** done — every outcome is answered with STATUS_AGENT; a wrong cookie or an
unreadable payload gives STATUS_AGENT with `STATUS_ERR_INVALID_DATA`
(`ed2ca7b49a9`).

### §8.3.5.1 Flags of CREATE_CLIENT

**Spec:** §8.3.5.1.1, p. 63 — "The CREATE_CLIENT submessage does not define
any additional flag bits beyond the common ones specified in 8.3.4.2."

**Repo:** `crates/xrce/src/submessages/create_client.rs`
(`CreateClientPayload::into_submessage`)

**Tests:** `crates/xrce/src/submessages/create_client.rs::tests::create_client_roundtrip_via_submessage`,
`crates/xrce/src/submessages/create_client.rs::tests::annex_b_1_create_client_example_bytes`

**Status:** done — the code sets the E flag only. Annex B.1 names 0x07 ("reuse,
replace"), which CREATE_CLIENT does not have; the test for Annex B.1
(`d4d590690f6`) checks the bytes of the `CLIENT_Representation`, not that flag.

### §8.3.5.1 Payload CREATE_CLIENT_Payload

**Spec:** §8.3.5.1.2, p. 64 — "struct CLIENT_Representation { XrceCookie
xrce_cookie; XrceVersion xrce_version; XrceVendorId xrce_vendor_id; ClientKey
client_key; SessionId session_id; @optional PropertySeq properties; }; struct
CREATE_CLIENT_Payload { CLIENT_Representation client_representation; };"

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation`, `encode_form`,
`decode_form`); `crates/xrce/src/encoding.rs`
(`RepresentationTimestamps`); `crates/xrce/src/submessages/create_client.rs`;
agent key `representation_timestamps` in
`crates/xrce-agent/src/daemon/config.rs`; `XrceClient::set_representation_timestamps`

**Tests:** `crates/xrce/src/submessages/create_client.rs::tests::decodes_the_committed_fixture_member_by_member`,
`crates/xrce/src/submessages/create_client.rs::tests::annex_b_1_create_client_example_bytes`,
`crates/xrce/src/handshake.rs::tests::client_representation_decodes_the_captured_create_client_prefix`,
`crates/xrce/src/handshake.rs::tests::by_default_the_representations_have_no_timestamp`,
`crates/xrce/src/handshake.rs::tests::the_prose_form_has_a_time_t_after_the_vendor_id`,
`crates/xrce-client/src/lib.rs::tests::the_representation_timestamp_is_an_option_of_the_client`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_form_of_the_representations_can_be_set`

**Status:** done — Annex A and the bytes of Annex B.1 have no `client_timestamp`; that is
the default. The prose of §7.8.2.1 and §8.3.5.1.2 lists a `Time_t` after
`xrce_vendor_id`; that form is the setting `representation_timestamps:
present` (Rust: `encoding::RepresentationTimestamps::Present`; SDKs: option
`representation timestamps`) (`711f93fd460`, `a8e8f6953a4`). Both forms are
tested with bytes in `zerodds-xrce` and in the client. The code reads and
writes two octets after `properties` as `requested_mtu`, as eProsima sends
them; the spec does not know the field. `PropertySeq` is aligned after Annex A
(`65cd1820a04`).

### §8.3.5.2 CREATE from the client

**Spec:** §8.3.5.2, p. 64 — "The CREATE submessage shall be sent by the XRCE
Client to create an XRCE Object."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::create_object`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::create_object_builds_the_real_two_level_object_variant`

**Status:** done

### §8.3.5.2 Reception calls create with flags and payload

**Spec:** §8.3.5.2, p. 64 — "Reception of this submessage shall result in the
XRCE Agent calling the create operation on the XRCE ProxyClient object [...].
The parameters to this operation shall be obtained from the SubmessageHeader
flags and payload."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §8.3.5.2 STATUS in response to CREATE

**Spec:** §8.3.5.2, p. 64 — "The XRCE Agent shall send a STATUS submessage in
response, see 8.3.5.6."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`, `status_reply`,
`refuse_unreadable`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_without_a_base_object_request_is_answered`

**Status:** done — every request gets a STATUS: a payload without a `BaseObjectRequest`
and an unreadable representation give `STATUS_ERR_INVALID_DATA`
(`e4d37a6aff6`, `133a0db3c4f`).

### §8.3.5.2 CREATE without BaseObjectRequest

**Spec:** §8.3.5.2, p. 64 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`,
`SessionEvent::CreateRefused`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_without_a_base_object_request_is_answered`

**Status:** done — a CREATE of fewer than 4 octets gives a STATUS with
`STATUS_ERR_INVALID_DATA` for `OBJECTID_INVALID` (with the `request_id` when
its two octets are there) and a `CreateRefused` event (`e4d37a6aff6`).

### §8.3.5.2 Flags reuse and replace

**Spec:** §8.3.5.2.1, p. 64 — "Bit 1, the 'Reuse' bit, encodes the value of the
CreationMode reuse field. Bit 2, the 'Replace' bit, encodes the value of the
CreationMode replace field."

**Repo:** `crates/xrce/src/submessages/create.rs` (`CREATE_FLAG_REUSE`,
`CREATE_FLAG_REPLACE`)

**Tests:** `crates/xrce/src/submessages/create.rs::tests::create_roundtrip_carries_reuse_replace_flags`,
`crates/xrce/src/submessages/create.rs::tests::create_default_flags_have_no_reuse_no_replace`

**Status:** done

### §8.3.5.2 Payload CREATE_Payload

**Spec:** §8.3.5.2.2, p. 64–65 — "struct CREATE_Payload : BaseObjectRequest {
ObjectVariant object_representation; };"

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`:
`BaseObjectRequest::decode`, `ObjectVariant::decode_for_kind`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::create_fixture_decodes_into_an_object_the_agent_accepts`,
`crates/xrce/src/object_repr.rs::tests::participant_representation_matches_the_captured_create_bytes`

**Status:** done

### §8.3.5.3 GET_INFO from the client

**Spec:** §8.3.5.3, p. 65 — "The GET_INFO submessage shall be sent by the XRCE
Client to get information about a resource identified by its object_id."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::request_get_info`,
`XrceClient::request_agent_info`, `ClientEvent::Info`,
`ClientEvent::InfoUnreadable`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::request_agent_info_works_before_the_session_and_sends_the_mask`,
`crates/xrce-client/src/lib.rs::tests::request_get_info_needs_a_session_and_names_the_object`,
`crates/xrce-client/src/lib.rs::tests::an_info_is_read_into_an_event`

**Status:** done — `request_get_info` asks about an object of the session,
`request_agent_info` about the agent, also without a session; an INFO is
evaluated as `ClientEvent::Info` (`65cd1820a04`).

### §8.3.5.3 Target by the ObjectKind of the object_id

**Spec:** §8.3.5.3, p. 65 — "The targeted XRCE Object shall depend on the
ObjectKind encoded in the last 4 bits of the object_id. If the ObjectKind is
set to OBJK_AGENT, then it shall result in the XRCE Agent calling the get_info
operation on the XRCE Root object [...]. If the ObjectKind is set to one of
OBJK_PARTICIPANT, [...] That is to a value between 0x01 and 0x0c (both
included), then it shall result in the XRCE Agent calling the get_info
operation on the XRCE ProxyClient object".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`: kind
`OBJK_AGENT` is the Root, 0x01 to 0x0C the ProxyClient, `OBJK_CLIENT` the
session); `crates/xrce-agent/src/daemon/discovery.rs`
(`run_discovery_responder`)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_a_session_is_answered_for_the_agent_only`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`

**Status:** done — the Root answers every `object_id` of kind `OBJK_AGENT`, also without
a session; the kinds 0x01 to 0x0C go to the ProxyClient (`65cd1820a04`). For
the Root the spec refers to §7.8.3.3 instead of §7.8.2.2, and the list says
"OBJK" instead of `OBJK_TOPIC`.

### §8.3.5.3 Distinction by the nibble of the kind

**Spec:** §8.3.5.3, p. 65 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`);
`crates/xrce-agent/src/daemon/discovery.rs` (`run_discovery_responder`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::any_object_id_of_kind_agent_is_the_root_in_a_session`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_tcp_listener_answers_get_info_before_a_session`

**Status:** done — `handle_get_info` and the responder check the kind of the `object_id`,
not the value 0xFFFD; an `ObjectId` `0x010` of kind `OBJK_AGENT` is answered
by the Root (`65cd1820a04`, `67ead4d9708`).

### §8.3.5.3 INFO in response to GET_INFO

**Spec:** §8.3.5.3, p. 65 — "The XRCE Agent shall send an INFO submessage in
response to this message, see 8.3.5.6."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`, `answer_info`,
`InfoPayload`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`

**Status:** done — every answer to GET_INFO is an INFO; for an unknown object it carries
`STATUS_ERR_UNKNOWN_REFERENCE`, without permission `STATUS_ERR_DENIED`
(`65cd1820a04`). The reference says §8.3.5.6 instead of §8.3.5.7.

### §8.3.5.3 Flags of GET_INFO

**Spec:** §8.3.5.3.1, p. 65 — "The GET_INFO submessage does not define any
additional flag bits beyond the common ones specified in 8.3.4.2."

**Repo:** `crates/xrce/src/submessages/get_info.rs`
(`GetInfoPayload::into_submessage`)

**Tests:** `crates/xrce/src/submessages/get_info.rs::tests::get_info_roundtrip`

**Status:** done

### §8.3.5.3 Payload GET_INFO_Payload

**Spec:** §8.3.5.3.2, p. 65 — "struct GET_INFO_Payload : BaseObjectRequest {
InfoMask info_mask; };"

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`:
`BaseObjectRequest::decode`, `GetInfoPayload::parse`);
`crates/xrce/src/object_info.rs` (`InfoMask`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_an_info_mask_is_invalid_data`,
`crates/xrce/src/object_info.rs::tests::info_mask_bits_and_wire_form`,
`crates/xrce/src/submessages/get_info.rs::tests::get_info_roundtrip`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`

**Status:** done — the agent reads the `BaseObjectRequest` and the `info_mask`; without
the mask it answers with an INFO and `STATUS_ERR_INVALID_DATA`
(`65cd1820a04`). The `client_info` of Figure 10 (§8.4.4) follows the
`InfoMask` (`GetInfoPayload::with_client_info`), because Annex A has no member
for it.

### §8.3.5.4 DELETE from the client

**Spec:** §8.3.5.4, p. 65 — "The DELETE submessage shall be sent by the XRCE
Client to delete the XRCE:ProxyClient or any other XRCE Object".

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::delete_object`,
`disconnect`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::disconnect_sends_delete_client_and_clears_state`

**Status:** done

### §8.3.5.4 delete_client for OBJK_CLIENT, delete otherwise

**Spec:** §8.3.5.4, p. 65–66 — "The related XRCE Object is identified by the
object_id field in the payload. If the ObjectVariant contained within the
payload has ObjectKind set to OBJK_CLIENT, then the XRCE Agent shall call the
delete_client operation. Otherwise it shall call the delete operation."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`

**Status:** done — the code takes the kind from the `object_id`; DELETE_Payload has no
`ObjectVariant`. `delete_client` without a ProxyClient gives
`STATUS_ERR_UNKNOWN_REFERENCE` (`ed2ca7b49a9`).

### §8.3.5.4 STATUS in response to DELETE

**Spec:** §8.3.5.4, p. 66 — "The XRCE Agent shall send a STATUS submessage in
response, see 8.3.5.6."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`, `status_reply`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done — the code answers every DELETE with a STATUS, `delete_client`
included, as the text of the clause says. Table 14 (p. 86) names STATUS_AGENT
for `delete_client`; there is no setting for it.

### §8.3.5.4 Flags and payload DELETE_Payload

**Spec:** §8.3.5.4.1–§8.3.5.4.2, p. 66 — no additional flags; "struct
DELETE_Payload : BaseObjectRequest { };"

**Repo:** `crates/xrce/src/submessages/delete.rs` (`DeletePayload`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::delete_fixture_decodes_and_removes_the_object_create_added`

**Status:** done

### §8.3.5.5 STATUS_AGENT on CREATE_CLIENT

**Spec:** §8.3.5.5, p. 66 — "The STATUS_AGENT submessage shall be sent by the
XRCE Agent in response to a CREATE_CLIENT submessage. The submessage shall
contain the returnStatus to the create_client operation invocation".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`agent_status_reply`; `SessionEngine::handle_create_client`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_client_rejects_wrong_major_version`

**Status:** `done` — Commits `ed2ca7b49a9`. every failure of CREATE_CLIENT (wrong cookie,
a `session_id` that is no session, wrong major version, ClientKey not
authorized, no resources) is answered with a STATUS_AGENT, not a STATUS.

### §8.3.5.5 Flags of STATUS_AGENT

**Spec:** §8.3.5.5.1, p. 66 — no additional flags.

**Repo:** `crates/xrce/src/submessages/status_agent.rs`
(`StatusAgentPayload::into_submessage`)

**Tests:** `crates/xrce/src/submessages/status_agent.rs::tests::status_agent_roundtrip`,
`crates/xrce-client/src/lib.rs::tests::a_big_endian_status_agent_is_read_with_its_properties`

**Status:** done — the agent writes the submessage with the E flag set and no
other flag; the client also reads E=0 (§8.3.4.2, `b43d5e6b848`).

### §8.3.5.5 Payload STATUS_AGENT_Payload
**Spec:** §8.3.5.5.2, p. 66–67 — "struct STATUS_AGENT_Payload {
AGENT_Representation agent_info; }; If the operation fails, the
STATUS_AGENT_Payload shall have the ResultStatus within the BaseObjectReply set
to with the StatusValue that corresponds to the type of error encountered."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`agent_status_reply`); `crates/xrce/src/submessages/status_agent.rs` (`StatusAgentPayload::result_and_position`); `crates/xrce-client/src/lib.rs` (`handle_status_agent`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_status_agent_payload_is_annex_a_unless_the_result_status_is_named`, `crates/xrce-agent/src/daemon/config.rs::tests::the_payload_of_status_agent_can_be_set`

**Status:** done — the IDL of Annex A is `STATUS_AGENT_Payload { AGENT_Representation agent_info; }`: a successful CREATE_CLIENT is answered with the `AGENT_Representation` alone (`agent_timestamp` only with `representation_timestamps: present`). The form of the text and of the eProsima capture with a `ResultStatus` in front is the setting `status_agent: with_result_status`; a failure carries the `ResultStatus` the text gives it in both forms because the IDL has no room for it. The client reads both (told apart by the cookie); the replay sets the setting. f514e3eae4b.
### §8.3.5.5 request_id and object_id in STATUS_AGENT
**Spec:** §8.3.5.5.2, p. 67 — "The request_id and object_id within the
BaseObjectReply shall match the namesake fields in the BaseObjectRequest of the
corresponding CREATE_CLIENT message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — The text asks that `request_id` and `object_id` in the `BaseObjectReply` of the STATUS_AGENT equal those of the `BaseObjectRequest` of the CREATE_CLIENT. `CREATE_CLIENT_Payload` has no `BaseObjectRequest` in Annex A (only the `CLIENT_Representation`), so there is nothing to equal, and `STATUS_AGENT_Payload` has no `BaseObjectReply` (see the payload item). There is no requirement that can be fulfilled.
### §8.3.5.5 xrce_cookie XRCE

**Spec:** §8.3.5.5.2, p. 67 — "The xrce_cookie shall be set to the four bytes
{'X', 'R', 'C', 'E'}."

**Repo:** `crates/xrce/src/handshake.rs` (`XRCE_COOKIE`,
`AgentRepresentation::encode_into`)

**Tests:** `crates/xrce/src/handshake.rs::tests::agent_representation_roundtrip_no_properties`,
`crates/xrce/src/handshake.rs::tests::agent_representation_decodes_the_captured_status_agent_body`

**Status:** done

### §8.3.5.5 xrce_version of the agent

**Spec:** §8.3.5.5.2, p. 67 — "The xrce_version shall be set to the version of
the XRCE protocol that the Agent will implement in its connection to the
Client."

**Repo:** `crates/xrce/src/handshake.rs` (`XRCE_VERSION`);
`crates/xrce-agent/src/daemon/session.rs` (`agent_status_reply`)

**Tests:** `crates/xrce/src/handshake.rs::tests::agent_representation_decodes_the_captured_status_agent_body`

**Status:** done

### §8.3.5.6 STATUS on CREATE and DELETE

**Spec:** §8.3.5.6, p. 67 — "The STATUS submessage shall be sent by the XRCE
Agent in response to a CREATE or DELETE."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`status_reply`; `handle_create`, `handle_delete`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`,
`crates/xrce-agent/src/daemon/session.rs::tests::delete_client_needs_a_proxy_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_without_a_base_object_request_is_answered`

**Status:** done

### §8.3.5.6 STATUS on READ_DATA on failure

**Spec:** §8.3.5.6, p. 67 — "The STATUS submessage shall also be sent by the
XRCE Agent in response to a READ_DATA submessage when the returnStatus to the
read_data operation is anything other than STATUS_OK."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`; `refuse_unreadable`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_request_that_cannot_be_read_is_answered_invalid_data`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_too_large_for_data_is_reported`

**Status:** done — a failure to set the filter, an unreadable request
(`133a0db3c4f`) and a sample that fits no submessage are answered with a STATUS.

### §8.3.5.6 STATUS carries the operation's return value

**Spec:** §8.3.5.6, p. 67 — "The STATUS submessage shall contain the
returnStatus to the operation that was triggered by the corresponding request
message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`status_reply`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §8.3.5.6 Flags and payload STATUS_Payload

**Spec:** §8.3.5.6.1–§8.3.5.6.2, p. 67 — no additional flags; "struct
STATUS_Payload : BaseObjectReply { }; [...] The request_id and object_id
within the BaseObjectReply shall match the namesake fields in the
corresponding request message."

**Repo:** `crates/xrce/src/object_info.rs` (`BaseObjectReply`, `ResultStatus`);
`crates/xrce/src/submessages/status.rs` (`StatusPayload`)

**Tests:** `crates/xrce/src/object_info.rs::tests::base_object_reply_roundtrip`,
`crates/xrce/tests/unannotated_types.rs::the_final_setting_writes_the_reference_implementations_octets`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done — `ResultStatus` has no `@extensibility` in Annex A and is
APPENDABLE: by default it carries a DHEADER (`BaseObjectReply` 10 octets), with
`unannotated_types: final` it is 6 octets as in the capture; both forms are read
(`f2ace917b5a`, `3874ad8d6a6`).

### §8.3.5.7 INFO on GET_INFO

**Spec:** §8.3.5.7, p. 67 — "The INFO submessage shall be sent by the XRCE
Agent to the XRCE Client in response to a GET_INFO message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`, `answer_info`);
`crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply`)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_tcp_listener_answers_get_info_before_a_session`

**Status:** `done` — Commits `65cd1820a04`, `67ead4d9708`. inside a session, before a
session at the UDP and TCP listeners, and over the discovery group.

### §8.3.5.7 Flags of INFO

**Spec:** §8.3.5.7.1, p. 67 — no additional flags.

**Repo:** `crates/xrce/src/submessages/info.rs` (`InfoPayload::into_submessage_with`)

**Tests:** `crates/xrce/src/submessages/info.rs::tests::info_roundtrip`,
`crates/xrce/src/submessages/info.rs::tests::info_payload_is_the_reply_and_the_object_info`

**Status:** done — the E flag states the byte order of the payload; the agent
writes E=1 (`65cd1820a04`).

### §8.3.5.7 Payload INFO_Payload

**Spec:** §8.3.5.7.2, p. 67–68 — "struct INFO_Payload : BaseObjectReply {
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

**Status:** `done` — Commits `65cd1820a04`. `BaseObjectReply` and `ObjectInfo` (activity
before config, `@optional` as a presence octet) per Annex A; `activity` and
`config` follow the `InfoMask`. The agent counts the values `highest_acked_num`
(DataReader) and `stream_seq_num` and `sample_seq_num` (DataWriter) itself; the
specification does not define them, not measured against another agent.

### §8.3.5.7 Failure in INFO

**Spec:** §8.3.5.7.2, p. 68 — "If the operation fails the ResultStatus within
the BaseObjectReply shall be set to the StatusValue that corresponds to the
type of error encountered."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`, `answer_info`);
`crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_an_info_mask_is_invalid_data`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_and_delete_without_permission_are_denied`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::a_get_info_without_a_mask_is_answered_with_an_error_info`

**Status:** `done` — Commits `65cd1820a04`. a failure of GET_INFO (UNKNOWN_REFERENCE,
INVALID_DATA, DENIED) is sent as an INFO carrying the status, not as a STATUS.

### §8.3.5.7 request_id and object_id of the GET_INFO

**Spec:** §8.3.5.7.2, p. 68 — "The request_id and object_id within the
BaseObjectReply shall match the identically named fields in the
BaseObjectRequest of the corresponding GET_INFO message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`: `InfoRequest::base`, `answer_info`);
`crates/xrce-agent/src/daemon/discovery.rs` (`build_info_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/session.rs::tests::any_object_id_of_kind_agent_is_the_root_in_a_session`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::any_object_id_of_kind_agent_is_the_root`

**Status:** `done` — Commits `65cd1820a04`. the tests check `request_id` and `object_id`
in the INFO, also for an `object_id` of kind OBJK_AGENT other than 0xFFFD.

### §8.3.5.8 WRITE_DATA from the client

**Spec:** §8.3.5.8, p. 68 — "The WRITE_DATA submessage is used by the XRCE
Client to write data using an XRCE DataWriter object within the XRCE Agent."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::request_write`,
`XrceClient::request_write_sample`); `crates/xrce/src/submessages/write_data.rs`

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_matches_the_annex_b_example`,
`crates/endpoint-e2e/tests/xrce_write_framing.rs::rust_endpoint_frames_conformant_write_data`

**Status:** done — `xrce_write_framing.rs` checks the frames of the endpoint
SDKs of further languages against the same decoder.

### §8.3.5.8 Reception calls write, STATUS in response

**Spec:** §8.3.5.8, p. 68 — "Reception of this submessage shall result in the
XRCE Agent calling the write operation on an XRCE DataWriter object (see
7.8.4.1). The XRCE Agent shall respond with a STATUS submessage."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_successful_write_is_answered_with_status_ok`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** `done` — Commits `dc0209b38fe`. `write` is called; a write without error is
answered with STATUS_OK, a failure with its status. The reply follows the text
of §8.3.5.8; the capture of the eProsima agent shows no reply to WRITE_DATA,
there is no setting for it.

### §8.3.5.8 DataWriter from object_id, data from the payload

**Spec:** §8.3.5.8, p. 68 — "The data parameter to the write operation shall be
obtained from the payload. The related XRCE DataWriter is identified by the
object_id field in the payload. Upon reception of this message the XRCE Agent
shall located the XRCE DataWriter identified by the object_id and use it to
write the data to the DDS domain."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`, `dds_write_and_report`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`

**Status:** done

### §8.3.5.8 Flags: endianness and DataFormat in bits 1 to 3

**Spec:** §8.3.5.8.1, p. 68 — "The WRITE_DATA sub-message uses the lowest
order 4 bits of the flags: Bit 0 indicates the 'Endianness' [...]. Bits 1, 2,
and 3 shall be set to indicate the DataFormat used for the payload."

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`DataFormat::to_flag_bits`, `DataFormat::from_flag_bits`, `DataFormat::FORMAT_MASK`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_sample_flags_carry_format_sample`,
`crates/xrce/src/submessages/write_data.rs::tests::write_data_rejects_reserved_format`

**Status:** done

### §8.3.5.8 Table 11 flag values

**Spec:** §8.3.5.8.1, Table 11, p. 68–69 — FORMAT_DATA 0x0/0x1, FORMAT_SAMPLE
0x2/0x3, FORMAT_DATA_SEQ 0x8/0x9, FORMAT_SAMPLE_SEQ 0xA/0xB,
FORMAT_PACKED_SAMPLES 0xE/0xF; example 0x09 for FORMAT_DATA_SEQ
little-endian.

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`DataFormat`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_flags_are_e_le_and_format_data`,
`crates/xrce/src/submessages/write_data.rs::tests::data_format_raw_roundtrip`,
`crates/xrce/src/submessages/write_data.rs::tests::any_write_data_dispatches_every_format`

**Status:** done

### §8.3.5.8 Table 12 payload per DataFormat

**Spec:** §8.3.5.8.2, Table 12, p. 69–70 — `WRITE_DATA_Payload_Data`,
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

**Status:** `done` — Commits `4f1fb9f5f4f`. codec and processing in the agent: the
samples of the three sequence formats are told apart by the topic's type and
written one by one, with a `SampleInfo` each. For a topic of unknown type a
sequence cannot be split and is answered with STATUS_ERR_INVALID_DATA, nothing
is written. Assumption, not measured against another agent: the samples sit back
to back with no padding, each from alignment origin 0 (Annex B.5.2 shows 4-octet
elements only).

### §8.3.5.9 READ_DATA from the client

**Spec:** §8.3.5.9, p. 70 — "The READ_DATA submessage is used by the XRCE
Client to initiate a reception (read) of data from an XRCE DataReader object
within the XRCE Agent."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::request_read`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::read_data_fixture_decodes_into_a_real_read_specification`

**Status:** done

### §8.3.5.9 Reception calls read, DATA or STATUS

**Spec:** §8.3.5.9, p. 70 — "Reception of this submessage shall result in the
XRCE Agent calling the read operation on an XRCE DataReader object (see
7.8.5.1) one or more times. Depending on the returnStatus, the XRCE Agent may
respond with a DATA submessages or a STATUS submessage. The read_specification
parameters to the read operation shall be obtained from the payload."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`, `pump_read`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_elapsed_time_is_measured_from_the_real_request_time`,
`crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`

**Status:** done

### §8.3.5.9 One or more DATA, DataReader from object_id

**Spec:** §8.3.5.9, p. 70 — "The XRCE Agent shall send one or more DATA
submessages in response to this message, see 8.3.5.10. The related XRCE
DataReader is identified by the object_id field in the payload."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`pump_read`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done

### §8.3.5.9 DATA until the end criterion

**Spec:** §8.3.5.9, p. 70 — "After reception of this message, the XRCE Agent
shall continue to send DATA submessages to the client until either the 'end
criteria' specified in the payload read_specification and
continuous_read_options attained".

**Repo:** `crates/xrce/src/continuous_read.rs` (`ReadStream`);
`crates/xrce-agent/src/daemon/session.rs` (`tick`, `pump_read`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_elapsed_time_is_measured_from_the_real_request_time`,
`crates/xrce/src/continuous_read.rs::tests::max_samples_cap_enforced`

**Status:** `done` — Commits `2fa0800f55a`. `continuous_read_options` does not exist in
the IDL; its counterpart is `delivery_control` (item "Reading without and with
continuous_read_options" below).

### §8.3.5.9 A new READ_DATA for the same object_id

**Spec:** §8.3.5.9, p. 70 — "[...] or else a new READ_DATA message for the same
object_id is received from the XRCE Client."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`: `self.reads.insert`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_new_read_data_replaces_the_active_read`

**Status:** done — the new read replaces the active one; DATA then carries only the
`request_id` of the new one.

### §8.3.5.9 Control of the delivery time

**Spec:** §8.3.5.9, p. 71 — "The read operation also allows an XRCE Client to
control when data may be sent by the XRCE Agent so that the Agent does not
unnecessarily wake up the Client during its sleep cycle."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — purpose; the fields are in the items on
Table 9.

### §8.3.5.9 Flags of READ_DATA

**Spec:** §8.3.5.9.1, p. 71 — no additional flags.

**Repo:** `crates/xrce/src/submessages/read_data.rs`
(`ReadDataPayload::into_submessage`, `ReadDataPayload::into_submessage_with`)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::read_data_minimal_roundtrip`

**Status:** done

### §8.3.5.9 Payload READ_DATA_Payload

**Spec:** §8.3.5.9.2, p. 71 — "struct READ_DATA_Payload : BaseObjectRequest {
ReadSpecification read_specification; };"

**Repo:** `crates/xrce/src/submessages/read_data.rs` (`ReadDataPayload`,
`ReadSpecification`, `DataDeliveryControl`, `DeliveryControlForm`);
`crates/xrce-client/src/lib.rs` (`XrceClient::set_delivery_control_form`)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::read_data_base_object_request_precedes_the_spec`,
`crates/xrce/src/submessages/read_data.rs::tests::read_data_matches_the_captured_read_data_example`,
`crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`,
`crates/xrce/src/submessages/read_data.rs::tests::the_content_filter_is_a_string_with_a_nul_and_an_aligned_length`,
`crates/xrce/src/submessages/read_data.rs::tests::the_data_format_octet_has_the_values_of_annex_a`

**Status:** `done` — Commits `1663f8f6240`. `BaseObjectRequest` and `ReadSpecification`
per Annex A (`data_format` 0x00/0x02/0x08/0x0A/0x0E, `content_filter_expression`
as an XCDR2 string, `DataDeliveryControl` with a DHEADER). The contradiction
between Annex B.4.2 (DHEADER) and the eProsima capture (no DHEADER) is a named
setting: `XrceClient::set_delivery_control_form(DeliveryControlForm::Bare)`
writes the form without a DHEADER; the agent reads both.

### §8.3.5.9 Reading without and with continuous_read_options

**Spec:** §8.3.5.9.2, p. 71–72 — "If the continuous_read_options member is not
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

**Status:** `done` — Commits `2fa0800f55a`. the IDL does not know
`continuous_read_options`, `pace_period`, `max_total_samples` and
`max_total_elapsed_time`; the code takes the presence of `delivery_control` as
the marker (`max_samples`, `max_elapsed_time`, `min_pace_period`). With
`delivery_control` it reads continuously; without, it reads once and returns
what is in the cache. The difference to Micro-XRCE-DDS 3.0.1
(`DataReader::read`, read from the source: the next sample without a time limit)
is the named setting `read_without_delivery_control: once | next-sample`
(default `once`).

### §8.3.5.9 Reading without delivery_control reads the cache once

**Spec:** §8.3.5.9.2, p. 71–72 — as above.

**Repo:** `crates/xrce/src/continuous_read.rs` (`ReadStream::pull_pending_samples`);
`crates/xrce-agent/src/daemon/session.rs` (`pump_read`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_read_without_delivery_control_returns_the_cache_once`,
`crates/xrce-agent/src/daemon/session.rs::tests::next_sample_waits_for_one_sample`

**Status:** `done` — Commits `2fa0800f55a`. without `delivery_control` the agent delivers
all samples in the cache and ends the read; it does not deliver a sample that
arrives later (default `once`). With `read_without_delivery_control:
next-sample` it delivers the next sample that arrives.

### §8.3.5.9 MIN_PACE_PERIOD_NONE

**Spec:** §8.3.5.9.2, p. 72 — "The member min_pace_period may take the special
value MIN_PACE_PERIOD_NONE. This value shall indicate that there is no minimum
time interval between samples."

**Repo:** `crates/xrce/src/continuous_read.rs` (`ReadStream::capacity`,
`ReadStream::pull_pending_samples`: 0 without a minimum interval)

**Tests:** `crates/xrce/src/continuous_read.rs::tests::max_samples_cap_enforced`,
`crates/xrce/src/continuous_read.rs::tests::rate_limit_partitions_samples_over_time`,
`crates/xrce/src/continuous_read.rs::tests::pacing_throttles_per_period`

**Status:** done — the specification gives `MIN_PACE_PERIOD_NONE` no value;
the code takes 0.

### §8.3.5.10 DATA on STATUS_OK, STATUS otherwise

**Spec:** §8.3.5.10, p. 72 — "The DATA submessage shall be sent by the XRCE
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

### §8.3.5.10 Sequence of DATA until the end or cancellation

**Spec:** §8.3.5.10, p. 72 — "A single READ_DATA message may result on
multiple, possible an open-ended sequence, of DATA submessages sent as a
response by the XRCE Agent. The DATA messages will continue to be sent until
the one of the terminating conditions on the READ_DATA operation is reached,
or until it is explicitly cancelled."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`tick`, `pump_read`, `handle_read_data`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::read_data_elapsed_time_is_measured_from_the_real_request_time`,
`crates/xrce-agent/src/daemon/session.rs::tests::max_samples_zero_cancels_the_active_read`

**Status:** done — until the end condition or the cancellation with `MAX_SAMPLES_ZERO`.

### §8.3.5.10 request_id and object_id of the READ_DATA

**Spec:** §8.3.5.10, p. 72 — "The request_id and object_id within the DATA
payload shall match the namesake fields in the corresponding READ_DATA
message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`pump_read`: `ActiveRead::request_id`;
`data_submessages`: `BaseObjectRequest`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce/src/submessages/data.rs::tests::data_base_object_request_precedes_the_sample`

**Status:** done

### §8.3.5.10 Flags of DATA

**Spec:** §8.3.5.10.1, p. 72 — "The flags shall be interpreted the same way as
the flags of the WRITE_DATA submessage."

**Repo:** `crates/xrce/src/submessages/data.rs` (`DataPayload::flags`,
`DataPayloadSample::flags`, `DataPayloadDataSeq::flags`, `DataPayloadSampleSeq::flags`,
`DataPayloadPackedSamples::flags`)

**Tests:** `crates/xrce/src/submessages/data.rs::tests::data_uses_id_9`,
`crates/xrce/src/submessages/data.rs::tests::data_sample_rejects_non_sample_format`

**Status:** done

### §8.3.5.10 DATA format as requested in READ_DATA

**Spec:** §8.3.5.10.2, p. 72 — "The format the payload shall match the one
requested in the READ_DATA message having the matching request_id. It shall
also match the DataFormat encoded in the flags".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`: `ActiveRead::format`;
`data_submessages`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_data_seq_is_answered_with_one_data_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_is_answered_with_a_sample_each`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_seq_is_answered_with_one_sample_seq`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_packed_samples_is_answered_with_one_packed_samples`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_sequence_formats_split_at_the_size_of_a_submessage`

**Status:** `done` — Commits `d5fbdbe672e`. the agent sends DATA in the `data_format` the
READ_DATA asks for (FORMAT_DATA, _SAMPLE, _DATA_SEQ, _SAMPLE_SEQ,
_PACKED_SAMPLES); the sequence formats split at the size of a submessage.

### §8.3.5.10 Table 13 payload per DataFormat

**Spec:** §8.3.5.10.2, Table 13, p. 72–73 — `DATA_Payload_Data`, `_Sample`,
`_DataSeq`, `_SampleSeq`, `_PackedSamples`; "All the DATA payload
representations extend RelatedObjectRequest."

**Repo:** `crates/xrce/src/submessages/data.rs` (`DataPayload`,
`DataPayloadSample`, `DataPayloadDataSeq`, `DataPayloadSampleSeq`,
`DataPayloadPackedSamples`, `AnyData`);
`crates/xrce-agent/src/daemon/session.rs` (`data_submessages`)

**Tests:** `crates/xrce/src/submessages/data.rs::tests::any_data_dispatches_every_format`,
`crates/xrce/src/submessages/data.rs::tests::data_sample_seq_matches_annex_b_5_4_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_seq_is_answered_with_one_sample_seq`

**Status:** `done` — Commits `d5fbdbe672e`. codec and agent. In the list the closing
brace after `DATA_Payload_Sample` is missing (p. 73). The agent fills the
`SampleInfo` with what it knows: `sequence_number` counts the samples delivered,
`session_time_offset` is the time of delivery, `state` is 0 because
`DdsPort::poll_samples` hands over the payload bytes only (source time and
instance, view and sample state are not carried).

### §8.3.5.11 ACKNACK for a protocol independent of the transport

**Spec:** §8.3.5.11, p. 74 — "The ACKNACK submessage is used to enable a
transport independent reliability protocol to be implemented. If the
transport used for a session is able to reliably send messages [...] then
these messages may not be required."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — purpose; the code uses ACKNACK over TCP as
well.

### §8.3.5.11 The agent sends ACKNACK only on HEARTBEAT

**Spec:** §8.3.5.11, p. 74 — "in general it is expected that it is the XRCE
Client that initiates any synchronization, and therefore, the XRCE Agent will
only send ACKNACK messages in response to HEARTBEAT messages."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_heartbeat`;
no other place in the agent produces ACKNACK)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::out_of_order_reliable_messages_wait_for_the_gap_and_report_a_truthful_acknack`

**Status:** done

### §8.3.5.11 ACKNACK on STREAMID_NONE
**Spec:** §8.3.5.11, p. 74 — "The ACKNACK submessage does not belong to any
stream, for this reason the MessageHeader shall have the streamId set to
STREAM_ID_NONE".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_heartbeat`); `crates/xrce-client/src/lib.rs` (`handle_heartbeat`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::the_acknack_of_the_agent_is_on_streamid_none`

**Status:** done — `handle_heartbeat` of the agent and `XrceClient::handle_heartbeat` set `StreamId::NONE` in the ACKNACK header, the payload names the stream (0f85937a436).
### §8.3.5.11 Flags of ACKNACK

**Spec:** §8.3.5.11.1, p. 74 — no additional flags.

**Repo:** `crates/xrce/src/submessages/acknack.rs`
(`AckNackPayload::into_submessage`)

**Tests:** `crates/xrce/src/submessages/acknack.rs::tests::acknack_roundtrip_via_submessage`

**Status:** done

### §8.3.5.11 Payload ACKNACK_Payload

**Spec:** §8.3.5.11.2, p. 74 — "struct ACKNACK_Payload { short
first_unacked_seq_num; octet[2] nack_bitmap; octet stream_id; };"

**Repo:** `crates/xrce/src/submessages/acknack.rs` (`AckNackPayload`,
`ACKNACK_BODY_SIZE` = 5 octets of the members; `encode_body`, `decode_body`)

**Tests:** `crates/xrce/src/submessages/acknack.rs::tests::acknack_le_roundtrip`,
`crates/xrce/src/submessages/acknack.rs::tests::acknack_be_roundtrip`,
`crates/xrce/tests/unannotated_types.rs::the_default_is_appendable_with_a_dheader`,
`crates/xrce/tests/unannotated_types.rs::the_final_setting_writes_the_reference_implementations_octets`,
`crates/xrce/tests/unannotated_types.rs::both_forms_are_read_in_either_setting`

**Status:** `done` — Commits `f2ace917b5a`. `ACKNACK_Payload` has no `@extensibility` in
Annex A and is APPENDABLE (DDS-XTypes 1.3 §7.3.1.2.1.8): by default a DHEADER (4
octets) and 5 octets of members; with `unannotated_types: final` it is 5 octets
as in the eProsima capture; both forms are read, in every SDK (`a293d8fc5e9`,
`b590f6539d0`). The text says `short`, Annex A `unsigned short` for
`first_unacked_seq_num`; both have 2 octets.

### §8.3.5.11 first_unacked_seq_num

**Spec:** §8.3.5.11.2, p. 74 — "The first_unacked_seq_num shall indicate that
all sequence numbers up to but not including it have been received."

**Repo:** `crates/xrce/src/reliable.rs` (`ReliableStreamState::pending_acknack`:
`expected_seq`; `ReliableStreamState::recv_acknack`)

**Tests:** `crates/xrce/src/reliable.rs::tests::pending_acknack_marks_missing_slots`,
`crates/xrce/src/reliable.rs::tests::recv_acknack_clears_acked_seqnrs`

**Status:** done — the example below it says "up to and including 99" for the
value 100.

### §8.3.5.11 nack_bitmap

**Spec:** §8.3.5.11.2, p. 74 — "The nack_bitmap shall indicate missing sequence
numbers, starting from first_unacked_seq_num. [...] first_unacked_seq_num set
to 100 and nack_bitmap set to 0x4009 [...] would indicate that sequence
numbers 100, 103, and 114 are missing."

**Repo:** `crates/xrce/src/reliable.rs` (`ReliableStreamState::pending_acknack`,
`ReliableStreamState::recv_acknack`: `u16::from_le_bytes(nack_bitmap)`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_acknack`);
`crates/xrce-client/src/lib.rs` (`XrceClient::handle_acknack`)

**Tests:** `crates/xrce/src/reliable.rs::tests::end_to_end_sender_receiver_with_loss_recovery`,
`crates/xrce-agent/src/daemon/session.rs::tests::acknack_nack_triggers_retransmission_of_the_exact_buffered_bytes`

**Status:** done — bit i stands for `first_unacked_seq_num + i`,
`nack_bitmap[0]` for bits 0 to 7. The specification does not fix which octet
of `octet[2]` carries the low bits; every ACKNACK in the eProsima capture has
`nack_bitmap = 0x0000` (counted with a script over the file), so the order
cannot be checked against it.

### §8.3.5.11 stream_id of the ACKNACK

**Spec:** §8.3.5.11.2, p. 74 — "The stream_id indicates the stream to which the
ACKNACK is directed."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_acknack`:
`self.reliable.get_mut(&(client, ack.stream_id))`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::acknack_nack_triggers_retransmission_of_the_exact_buffered_bytes`

**Status:** done

### §8.3.5.12 HEARTBEAT for a protocol independent of the transport

**Spec:** §8.3.5.12, p. 74 — "The HEARTBEAT submessage is used to enable a
transport independent reliability protocol to be implemented. [...] If a
session transport is able to reliably send messages [...] then these messages
may not be required."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — purpose.

### §8.3.5.12 HEARTBEAT only for an active client

**Spec:** §8.3.5.12, p. 74 — "This specification does not dictate the timing
of HEARTBEAT messages. However, in general it is expected that it is the XRCE
Agent will only send HEARTBEAT messages when it has some indication that the
XRCE Client is active and not in a sleep cycle."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — expectation. The code sends HEARTBEAT in
`tick` every `DEFAULT_HEARTBEAT_PERIOD` (500 ms) while unacknowledged messages
exist and the session has not expired, regardless of a sleep cycle.

### §8.3.5.12 HEARTBEAT on STREAMID_NONE

**Spec:** §8.3.5.12, p. 74 — "The HEARTBEAT submessage does not belong to any
stream, for this reason the MessageHeader shall have the stream_id set to
STREAM_ID_NONE".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`tick`);
`crates/xrce-client/src/lib.rs` (`XrceClient::tick`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_heartbeat_of_the_agent_is_on_streamid_none`,
`crates/xrce-client/src/lib.rs::tests::a_heartbeat_of_the_client_is_on_streamid_none`

**Status:** done — the header of HEARTBEAT carries STREAMID_NONE, the payload the
stream it reports.

### §8.3.5.12 Flags of HEARTBEAT

**Spec:** §8.3.5.12.1, p. 74 — no additional flags.

**Repo:** `crates/xrce/src/submessages/heartbeat.rs`
(`HeartbeatPayload::into_submessage`)

**Tests:** `crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_roundtrip_via_submessage`

**Status:** done

### §8.3.5.12 Payload HEARTBEAT_Payload

**Spec:** §8.3.5.12.2, p. 75 — "struct HEARTBEAT_Payload { short
first_unacked_seq_nr; short last_unacked_seq_nr; octet stream_id; };"

**Repo:** `crates/xrce/src/submessages/heartbeat.rs` (`HeartbeatPayload`,
`HEARTBEAT_BODY_SIZE` = 5)

**Tests:** `crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_le_roundtrip`,
`crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_be_roundtrip`

**Status:** done — Annex A has `@extensibility(FINAL)`: 5 octets without a
DHEADER. Annex A names the members `first_unacked_seq_num` and
`last_unacked_seq_num` (`unsigned short`), the text `first_unacked_seq_nr` and
`last_unacked_seq_nr` (`short`); the code uses `_nr` and `i16`, both have 2
octets.

### §8.3.5.12 first_unacked_seq_nr and last_unacked_seq_nr

**Spec:** §8.3.5.12.2, p. 75 — "The first_unacked_seq_nr indicates the first
available message sequence number on the sending side. The
last_unacked_seq_nr indicates the first available message sequence number on
the sending side."

**Repo:** `crates/xrce/src/reliable.rs` (`ReliableStreamState::pending_heartbeat`:
`window_base` and the highest open `seq`)

**Tests:** `crates/xrce/src/reliable.rs::tests::pending_heartbeat_fires_first_time`,
`crates/xrce/src/reliable.rs::tests::heartbeat_window_end_with_hole_is_serial_max_not_count`

**Status:** done — the sentence on `last_unacked_seq_nr` repeats the one on
`first_unacked_seq_nr`; the code sets the last open number.

### §8.3.5.12 stream_id of the HEARTBEAT

**Spec:** §8.3.5.12.2, p. 75 — "The stream_id indicates the stream to which the
HEARTBEAT is directed."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_heartbeat`:
state per `hb.stream_id`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::out_of_order_reliable_messages_wait_for_the_gap_and_report_a_truthful_acknack`

**Status:** done

### §8.3.5.13 RESET resets the session state

**Spec:** §8.3.5.13, p. 75 — "The RESET submessage shall be used to reset and
re-establish a session. It contains no payload. It shall cause the XRCE Agent
to reset all state associated with the session_id indicated in the submessage
header."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_reset`);
`crates/xrce/src/reliable.rs` (`ReliableStreamState::reset`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::reset_clears_reads_backlog_best_effort_counters_and_every_reliable_stream`,
`crates/xrce/src/reliable.rs::tests::reset_clears_state_completely`

**Status:** `done` — Commits `619184de8a1`. RESET resets all reliable streams (stream 255
included), the reassembly of FRAGMENT, the running reads, the backlog waiting
for room in a sender window and the sequence counters of the best-effort
streams, in and out. The objects of the ProxyClient and its `session_id` stay;
the specification does not say whether "all state" includes them.

### §8.3.5.13 Running reads and counters after RESET

**Spec:** §8.3.5.13, p. 75 — as above.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_reset`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::reset_clears_reads_backlog_best_effort_counters_and_every_reliable_stream`

**Status:** `done` — Commits `619184de8a1`. `handle_reset` ends running READ_DATA,
empties the backlog and resets the counters of the best-effort streams; the test
sends RESET through the session.

### §8.3.5.13 Flags and empty payload of RESET

**Spec:** §8.3.5.13.1–§8.3.5.13.2, p. 75 — no additional flags; "The RESET
submessage shall have an empty payload."

**Repo:** `crates/xrce/src/submessages/reset.rs` (`ResetPayload`)

**Tests:** `crates/xrce/src/submessages/reset.rs::tests::reset_roundtrip_empty_body`,
`crates/xrce/src/submessages/reset.rs::tests::reset_rejects_nonempty_body`

**Status:** done

### §8.3.5.14 FRAGMENT for submessages above the MTU

**Spec:** §8.3.5.14, p. 75 — "The FRAGMENT submessage is used to enable sending
of other submessages whose length exceeds the transport MTU."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`emit_possibly_fragmented`);
`crates/xrce-client/src/lib.rs` (`XrceClient::send_maybe_fragmented`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_above_the_clients_mtu_on_a_reliable_stream_splits_into_fragments`,
`crates/xrce-client/src/lib.rs::tests::write_above_mtu_splits_into_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_fragment_run_without_a_submessage_is_answered_invalid_data`

**Status:** done — the MTU comes from the field `requested_mtu` that eProsima
appends; if it is missing, 508 octets apply. FRAGMENT only on reliable streams
(§8.3.5.14): on a best-effort stream the agent sends a too large submessage
whole and reports `SessionEvent::ReliableSendOverflowed`. A run without a
submessage is refused (`1933142acaf`).

### §8.3.5.14 FRAGMENT only on reliable streams

**Spec:** §8.3.5.14, p. 75 — "The FRAGMENT message shall only be sent within
reliable streams."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`emit_possibly_fragmented`:
sent whole on best-effort streams); `crates/xrce-client/src/lib.rs`
(`send_maybe_fragmented`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_sample_above_the_clients_mtu_on_a_reliable_stream_splits_into_fragments`,
`crates/xrce-client/src/lib.rs::tests::write_above_mtu_splits_into_fragments`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done — agent and client split only on reliable streams; a submessage
above the MTU goes out whole on a best-effort stream (event
`ReliableSendOverflowed`). On reception the code reassembles FRAGMENT on any
stream. The best-effort branch has no test of its own.

### §8.3.5.14 Last Fragment bit

**Spec:** §8.3.5.14, p. 75 — "all FRAGMENT submessage except for the last
shall have the 'Last Fragment' bit in the flags set to 0. The last FRAGMENT
submessage shall have the 'Last Fragment' flag set 1."

**Repo:** `crates/xrce/src/submessages/fragment.rs` (`FRAGMENT_FLAG_LAST`)

**Tests:** `crates/xrce/src/submessages/fragment.rs::tests::fragment_roundtrip_carries_last_flag`,
`crates/xrce/src/submessages/fragment.rs::tests::fragment_intermediate_has_no_last_flag`

**Status:** done

### §8.3.5.14 Reassembly and interpretation as submessages

**Spec:** §8.3.5.14, p. 75 — "Upon reception of the last fragment submessage
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

**Status:** done — the bytes are concatenated in the order of the stream and
processed under the header of the message with the last fragment, every
submessage of the run (9b18d126ef3).

### §8.3.5.14 Every submessage of the reassembled run

**Spec:** §8.3.5.14, p. 75 — as above.

**Repo:** `crates/xrce/src/submessages/mod.rs` (`decode_submessages`);
`crates/xrce-agent/src/daemon/session.rs` (`process_message_submessages`);
`crates/xrce-client/src/lib.rs` (`XrceClient`, `ClientEvent::FragmentDiscarded`)

**Tests:** `crates/xrce/src/submessages/mod.rs::tests::a_submessage_run_decodes_every_submessage_past_the_padding`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_fragmented_run_of_two_submessages_runs_both`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_fragment_run_without_a_submessage_is_answered_invalid_data`,
`crates/xrce-client/src/lib.rs::tests::a_fragment_run_without_a_submessage_is_reported`

**Status:** done — agent and client take every submessage of the reassembled
run, at multiples of 4 (§8.3.3) (9b18d126ef3). A run without a submessage, or
one that is no sequence of submessages, gives STATUS `STATUS_ERR_INVALID_DATA`
at the agent and `ClientEvent::FragmentDiscarded` at the client (1933142acaf).

### §8.3.5.14 Flags of FRAGMENT

**Spec:** §8.3.5.14.1, p. 75 — "Bit 0 indicates the 'Endianness' [...]. Bit 1,
the 'Last Fragment' bit, indicates the last fragment in the sequence."

**Repo:** `crates/xrce/src/submessages/fragment.rs` (`FragmentPayload::flags`)

**Tests:** `crates/xrce/src/submessages/fragment.rs::tests::fragment_roundtrip_carries_last_flag`

**Status:** done

### §8.3.5.14 FRAGMENT payload in the buffer

**Spec:** §8.3.5.14.2, p. 75 — "The payload of the FRAGMENT submessage is
opaque. The Agent shall cache the payload bytes of all FRAGMENT submessages for
a Stream in the order of the stream sequence number until the last FRAGMENT
submessage is received."

**Repo:** `crates/xrce/src/fragment.rs` (`FragmentAssembler`);
`crates/xrce-agent/src/daemon/session.rs` (`FragmentRun`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_tick_between_fragments_keeps_the_reassembly`,
`crates/xrce/src/fragment.rs::tests::out_of_order_reassembly`

**Status:** done — an incomplete run is discarded after 10 s
(`DEFAULT_GC_TTL`) and reported as an event.

### §8.3.5.15 Sending TIMESTAMP

**Spec:** §8.3.5.15, p. 76 — "The TIMESTAMP submessage is used to send
timestamp information. [...] The TIMESTAMP submessage does not belong to any
stream, for this reason the MessageHeader shall have the streamId set to
STREAM_ID_NONE". §8.3.5.15.2: "The transmit_timestamp shall contain the
timestamp of the sender of the message at the time the message is sent."

**Repo:** `crates/xrce-agent/src/daemon/session.rs`
(`SessionEngine::request_timestamp`, `tick` with `timestamp_interval`,
`handle_timestamp_reply`); `crates/xrce-client/src/lib.rs`
(`XrceClient::send_timestamp`, `send_timestamp_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::the_agent_sends_a_timestamp_and_reads_the_reply`,
`crates/xrce-agent/src/daemon/session.rs::tests::tick_sends_a_timestamp_every_interval`,
`crates/xrce-client/src/lib.rs::tests::the_client_sends_and_reads_timestamps`

**Status:** done — agent and client send TIMESTAMP on `STREAMID_NONE` with the
sender's clock (2b718de0166). The agent sends unprompted at the interval of the
agent key `timestamp_interval_secs`; default 0 (none), since the eProsima
clients do not expect a TIMESTAMP. The client's TIMESTAMP_REPLY gives the event
`ClockSynchronised`.

### §8.3.5.15 Flags and payload TIMESTAMP_Payload

**Spec:** §8.3.5.15.1–§8.3.5.15.2, p. 76 — no additional flags; "struct
TIMESTAMP_Payload { Time_t transmit_timestamp; };"

**Repo:** `crates/xrce/src/submessages/timestamp.rs` (`TimestampPayload`,
`TimePoint`, `TimePoint::encode_into`, `TimePoint::decode_at`)

**Tests:** `crates/xrce/src/submessages/timestamp.rs::tests::timestamp_roundtrip_via_submessage`,
`crates/xrce/src/submessages/timestamp.rs::tests::time_t_roundtrip_be`,
`crates/xrce/src/submessages/timestamp.rs::tests::a_timestamp_body_has_the_dheader_of_an_appendable_time_t`

**Status:** done — `TIMESTAMP_Payload` is `@extensibility(FINAL)`; `Time_t` has
no annotation in Annex A and is APPENDABLE (body: DHEADER 8 and 8 octets). The 8
octets of the reference implementation are written by the setting
`unannotated_types: final` (Rust `UnannotatedTypes::Final`); both forms are read
(f2ace917b5a, 3874ad8d6a6).

### §8.3.5.16 TIMESTAMP_REPLY as an answer

**Spec:** §8.3.5.16, p. 76 — "The TIMESTAMP_REPLY submessage is used to reply
to a timestamp information message."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_timestamp`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::timestamp_reply_echoes_originate_and_uses_real_wall_clock`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_agent_sends_a_timestamp_and_reads_the_reply`,
`crates/xrce-client/src/lib.rs::tests::the_client_sends_and_reads_timestamps`

**Status:** done — the agent answers TIMESTAMP, the client answers the agent's
TIMESTAMP (`send_timestamp_reply`).

### §8.3.5.16 TIMESTAMP_REPLY on STREAMID_NONE
**Spec:** §8.3.5.16, p. 76 — "The TIMESTAMP_REPLY submessage does not belong
to any stream, for this reason the MessageHeader shall have the streamId set to
STREAM_ID_NONE".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_timestamp`); `crates/xrce-client/src/lib.rs` (`send_control`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::the_timestamp_reply_of_the_agent_is_on_streamid_none`

**Status:** done — the code sets `StreamId::NONE` in the reply of the agent and of the client; the test checks the header of the agent's TIMESTAMP_REPLY (0f85937a436).
### §8.3.5.16 Flags and payload TIMESTAMP_REPLY_Payload

**Spec:** §8.3.5.16.1–§8.3.5.16.2, p. 76 — no additional flags; "struct
TIMESTAMP_REPLY_Payload { Time_t transmit_timestamp; Time_t receive_timestamp;
Time_t originate_timestamp; };"

**Repo:** `crates/xrce/src/submessages/timestamp_reply.rs`
(`TimestampReplyPayload`)

**Tests:** `crates/xrce/src/submessages/timestamp_reply.rs::tests::timestamp_reply_roundtrip_le`,
`crates/xrce/src/submessages/timestamp_reply.rs::tests::timestamp_reply_roundtrip_via_submessage`,
`crates/xrce/src/transport_udp.rs::tests::loopback_roundtrip_timestamp_reply`

**Status:** done — `TIMESTAMP_REPLY_Payload` is `@extensibility(FINAL)` with
three `Time_t`, each APPENDABLE (DHEADER 8, 36 octets together); with
`unannotated_types: final` three times 8 octets; both forms are read
(f2ace917b5a).

### §8.3.5.16 Meaning of the three timestamps

**Spec:** §8.3.5.16.2, p. 76 — "The transmit_timestamp shall contain the
timestamp of the sender of the TIMESTAMP_REPLY message at the time the message
is sent. The originate_timestamp shall contain the transmit_timestamp of the
related TIMESTAMP [...]. The receive_timestamp the shall contain the timestamp
at which the related TIMESTAMP message was received."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_timestamp`,
`wall_clock_now`, `handle_timestamp_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::timestamp_reply_echoes_originate_and_uses_real_wall_clock`,
`crates/xrce-agent/src/daemon/session.rs::tests::the_agent_sends_a_timestamp_and_reads_the_reply`

**Status:** done — `originate_timestamp` is the `transmit_timestamp` of the
TIMESTAMP; `transmit_timestamp` and `receive_timestamp` of the agent's reply
come from one reading of the clock. From the four times of a client's
TIMESTAMP_REPLY the agent computes offset and round trip.

---

## §8.4 Interaction Model

### §8.4.1 Typical flows, partly replaceable by configuration

**Spec:** §8.4.1, p. 77 — "This section describes typical message flows. [...]
some of the message flows are optional and may be replaced by out-of-band
configuration of the XRCE Client and Agent."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description; the flows are in the following
items.

### §8.4.2 Sending through a pre-configured DataWriter

**Spec:** §8.4.2, p. 77 — "The XRCE Agent has been pre-configured to create an
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

**Status:** done — WRITE_DATA on the `ObjectId` of a configured resource (MD5 of
the name, §9.3) makes the DomainParticipant with everything inside it for the
client; the AccessController (§7.4) applies. The text names an
`application_object_id` in CREATE_CLIENT and a flag `reuse`;
`CLIENT_Representation` (Annex A) and §8.3.5.1.1 have neither. The trigger is
the agent key `client_applications:` (ClientKey, Application or
DomainParticipant reference); CREATE_CLIENT makes the objects, a second
CREATE_CLIENT of the same session keeps using them (03371b4965c, b6e55cfa859).

### §8.4.3 Receiving through a pre-configured DataReader

**Spec:** §8.4.3, p. 77–78 — "The XRCE Agent has been pre-configured to create
an XRCE Application containing a DomainParticipant, Subscriber and DataReader.
The DataReader pre-configured object_id is known to the XRCE Client. [...] A
subsequent READ messag e references the ObjectId of those DataReaders".
Figure 9: CREATE_CLIENT(reuse=1), STATUS_AGENT, READ_DATA, DATA, DELETE.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`ensure_predefined`, `make_client_applications`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_client_reads_through_a_preconfigured_data_reader_without_a_create`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`

**Status:** done — READ_DATA on the `ObjectId` of the configured DataReader
makes it, DATA comes with the `request_id` of the READ_DATA. The same deviation
from the text (`application_object_id`, `reuse`) and the same agent key
`client_applications:` as in §8.4.2 (b6e55cfa859).

### §8.4.4 Discovery of an agent through GET_INFO

**Spec:** §8.4.4, p. 78–79 — "This flow is only required when the Client is not
pre-configured with the TransportLocator of the XRCE Agent. It allows an XRCE
Client to be configured to content one or more TransportLocators (which may
include multicast addresses) in order to dynamically discover the presence and
actual Address of the Agents." Figure 10: GET_INFO(OBJECTID_AGENT,
CLIENT_Representation), INFO(AGENT_Representation): STATUS_OK, INFO():
STATUS_ERR_INCOMPATIBLE, then CREATE_CLIENT and STATUS_AGENT.

**Repo:** `crates/xrce-agent/src/daemon/discovery.rs`
(`run_discovery_responder`, `build_info_reply`, `bind_discovery_socket`);
`crates/xrce-agent/src/daemon/session.rs` (`client_info_status`,
`handle_get_info`); `crates/xrce/src/discovery.rs` (`MulticastDiscovery`)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_info_mask_selects_activity_and_configuration`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::a_client_of_another_major_version_is_answered_incompatible`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_discovery_group_is_the_one_given`,
`crates/xrce/src/discovery.rs::tests::loopback_send_recv_roundtrip`

**Status:** done — the agent answers `GET_INFO(OBJECTID_AGENT)` on the discovery
port with INFO (AGENT_Representation and AGENT_ActivityInfo by the InfoMask),
with and without a session; the multicast address is configurable
(`discovery.address:`). Annex A's `GET_INFO_Payload` has no field for the
`CLIENT_Representation` of Figure 10; Annex A is the default, Figure 10 is an
extension behind the InfoMask (`GetInfoPayload::with_client_info`, client:
`AgentDiscovery::with_client_info`) that an agent without it ignores
(5798b040952, 65cd1820a04).

### §8.4.4 Discovery on the client side

**Spec:** §8.4.4, p. 78–79 — as above.

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

**Status:** done — the client sends GET_INFO to a list of addresses at a fixed
period, evaluates the INFO and connects to the first agent that fits (UDP, TCP).
The tests reach the agents by unicast on loopback; reception on a multicast
group is not tested (5798b040952).

### §8.4.4 INFO with STATUS_ERR_INCOMPATIBLE

**Spec:** §8.4.4, Figure 10, p. 79 — "INFO(): STATUS_ERR_INCOMPATIBLE".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`client_info_status`,
`handle_get_info`); `crates/xrce-agent/src/daemon/discovery.rs`
(`build_info_reply`)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::a_client_of_another_major_version_is_answered_incompatible`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_checks_the_version_in_client_info`,
`crates/xrce-client/src/discovery.rs::tests::client_info_names_the_version_and_hides_the_client_key`

**Status:** done — if the GET_INFO carries a `CLIENT_Representation` of another
major version than the agent's, the answer is an INFO with
`STATUS_ERR_INCOMPATIBLE`; an unreadable one gives `STATUS_ERR_INVALID_DATA`.
The `client_info` is the extension behind the InfoMask of the item above
(5798b040952).

### §8.4.4 Selection among several agents

**Spec:** §8.4.4, p. 78 — "the XRCE Client may discover more than one XRCE
Agent. In that case it may use the information received about the XRCE Agent
configuration [...] and the XRCE Agent activity [...] to select the most
appropriate XRCE Agent and even connect to more than one XRCE Agents."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — permission for the client.

### §8.4.5 Connecting with CREATE_CLIENT and STATUS_AGENT

**Spec:** §8.4.5, p. 79–80 — "An XRCE Client connects to an Agent using the
CREATE_CLIENT message. The Agent responds with a STATUS_AGENT indicating whether
the connection succeeded and the ClientProxy was created on behalf of the XRCE
Client."

**Repo:** `crates/xrce-client/src/lib.rs` (`XrceClient::connect`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`,
`agent_status_reply`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::full_lifecycle_creates_unique_request_ids_and_real_submessages`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`

**Status:** done

### §8.4.6 Creating a complete application

**Spec:** §8.4.6, p. 80 — "An XRCE Client uses the CREATE message to create an
XRCE Application. The CREATE message carries a CREATE_Payload containing an
ObjectVariant with ObjectKind set to OBJK_APPLICATION. The corresponding
OBJK_APPLICATION_Representation may use the REPRESENTATION_BY_REFERENCE to
refer to an Application pre-configured in the Agent or it may use the
REPRESENTATION_AS_XML_STRING to fully describe the Application including any
necessary Types, Qos, and DDS Entities."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, branch
`ObjectKind::Application`); `crates/xrce-agent/src/daemon/entity_repr.rs`
(`application_definition`); `crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::add_application_library`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_create_client_makes_the_objects_configured_for_the_client`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`

**Status:** done — CREATE(OBJK_APPLICATION) with an `<application_library>`
makes the DomainParticipants of the application with topics, publishers,
subscribers and endpoints; BY_REFERENCE names an application of the agent's
definitions (3aea52dd70a, b6e55cfa859).

### §8.4.7 Defining a QosProfile

**Spec:** §8.4.7, p. 80 — "An XRCE Client uses the CREATE message to define Qos
Profile. The CREATE message carries a CREATE_Payload containing an
ObjectVariant with ObjectKind set to OBJK_QOSPROFILE. The corresponding
OBJK_QOSPROFILE_Representation may use the REPRESENTATION_AS_XML_STRING to
fully describe the Qos Profile." Other objects are later created with this
profile.

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, branch
`ObjectKind::QosProfile`, `definition_to_register`, `rebuild_profiles`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_qos_profile`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::type_and_qos_profile_xml_follow_their_clauses`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_qos_profile_reference_applies_the_profile`

**Status:** done — the profile belongs to the agent's definitions as long as the
object exists; a later CREATE names it as `Library::Profile` (`qos_profile_ref`,
`base_name`). In Figure 13 client and agent are swapped (3aea52dd70a).

### §8.4.8 Defining types
**Spec:** §8.4.8, p. 81 — "An XRCE Client uses the CREATE message to create an
XRCE Type. The CREATE message carries a CREATE_Payload containing an
ObjectVariant with ObjectKind set to OBJK_TYPE. The corresponding
OBJK_TYPE_Representation may use the REPRESENTATION_AS_XML_STRING to fully
describe the DDS-XTYPES Type including any referenced types."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`types`)

**Tests:** `crates/xrce-agent/tests/spec_items_session.rs::a_type_by_reference_names_a_known_type_by_its_fully_qualified_name`

**Status:** done — a type as `<types>` is compiled and known under its name; it may refer to types the agent already knows (item on §7.7.3.3.2 "Reference to known types").
### §8.4.9 Creating a topic

**Spec:** §8.4.9, p. 81 — "An XRCE Client uses the CREATE message to create an
XRCE Topic. The CREATE message carries a CREATE_Payload containing an
ObjectVariant with ObjectKind set to OBJK_TOPIC. The corresponding
OBJK_TOPIC_Representation may use the REPRESENTATION_IN_BINARY or the
REPRESENTATION_AS_XML_STRING to fully define the Topic."

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`topic`,
`topic_from_xml`); `crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_topic_gets_the_type_its_type_name_names`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_creates_the_dds_entities_the_real_clients_describe`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_topic_of_an_unknown_type_is_refused_when_the_agent_is_told_to`

**Status:** done — the XML form in the first test, the binary form from the
eProsima capture in the second. A topic of a type the agent does not know gives
`STATUS_ERR_UNKNOWN_REFERENCE` (agent key `unknown_topic_types: refuse`,
default; `forward` creates it), third test (0da6aef56b5).

### §8.4.10 Creating a DataWriter and writing

**Spec:** §8.4.10, p. 82 — "An XRCE Client uses the CREATE message to create an
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

**Status:** done — XML in the first test, the binary form in the second. Annex A
and §7.7.3.1 give the DataWriter only binary and XML; §8.4.10 also names
`REPRESENTATION_BY_REFERENCE`. Annex A is the default (the reference gives
`STATUS_ERR_INVALID_DATA`, fourth test); the agent key `representation_formats:
any` admits the reference (third test) (bdf3f6ee966, f0995555d4a). The text
writes `DATAREADER_Representation` for the DataWriter; Figure 16 names
`DELETE_RESOURCE`, no such submessage exists (DELETE is meant).

### §8.4.11 Creating a DataReader and reading

**Spec:** §8.4.11, p. 82–83 — "An XRCE Client uses the CREATE message to create
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

**Status:** done — binary and XML as for the DataWriter; the reference to a
DataReader is not provided by Annex A and §7.7.3.1 and is admitted only with the
agent key `representation_formats: any` (third and fourth test) (bdf3f6ee966).
The introduction says “to publish data” for the DataReader.

### §8.4.12 Information on a resource

**Spec:** §8.4.12, p. 83 — "An XRCE Client may use this mechanism to determine
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

**Status:** done — GET_INFO on an object of the client gives an INFO with the
`ObjectVariant` it was created with (with its `ObjectKind`) and the activity of
a DataWriter or DataReader by the InfoMask (65cd1820a04). GET_INFO on the
`ObjectId` of a QoS profile or type from the agent's definitions makes the
object for the client (`ensure_predefined`, b6e55cfa859); this branch has no
test of its own.

### §8.4.13 Updating a resource

**Spec:** §8.4.13, p. 84 — "An XRCE Client uses the CREATE message with the
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

**Status:** done — CREATE with `reuse` and `replace` replaces an unequal object
(Table 5); if the DDS side fails, the old configuration applies again. The
update of a missing object contradicts itself in the spec: §7.8.3.2 requires
`STATUS_ERR_UNKNOWN_REFERENCE`, Table 5 and §8.4.13 create it. The default is
Table 5; the agent key `update_of_missing_object: refuse` gives
`STATUS_ERR_UNKNOWN_REFERENCE` (8b949a13ef5, e4d37a6aff6).

### §8.4.14 Reliability per stream, only on reliable streams

**Spec:** §8.4.14, p. 84 — "Reliability is implemented separately for each
Stream, and only for the reliable streams which are identified by the stream_id
value being between 0x80 and 0xFF."

**Repo:** `crates/xrce/src/reliable.rs` (`ReliableStreamState::new`);
`crates/xrce-agent/src/daemon/session.rs` (`reliable` per `(client, stream)`,
`handle_message`)

**Tests:** `crates/xrce/src/reliable.rs::tests::constructor_panics_on_best_effort_stream`,
`crates/xrce-agent/src/daemon/session.rs::tests::best_effort_messages_get_increasing_sequence_numbers`

**Status:** done

### §8.4.14 Two endpoints, each with its own state machine

**Spec:** §8.4.14, p. 84 — "A Stream has exactly two endpoints, the sending
endpoint and the receiving endpoint. [...] The sender and receiver endpoint on
a Stream each execute its own protocol state machine."

**Repo:** `crates/xrce/src/reliable.rs` (`ReliableStreamState`: sender
`submit`, `pending_heartbeat`, `recv_acknack`; receiver `recv_data`,
`drain_in_order`, `pending_acknack`)

**Tests:** `crates/xrce/src/reliable.rs::tests::end_to_end_sender_receiver_with_loss_recovery`

**Status:** done — both halves live in one state per stream; agent and client
use one of them per direction.

### §8.4.14 Serial number arithmetic

**Spec:** §8.4.14, p. 84 — "Sequence number arithmetic and comparisons shall
use Serial Number Arithmetic as specified in clause 8.3.2.3 sequenceNr."

**Repo:** `crates/xrce/src/reliable.rs` (`serial_min_in_flight`,
`serial_max_in_flight`); `crates/xrce/src/serial_number.rs`

**Tests:** `crates/xrce/src/reliable.rs::tests::heartbeat_window_across_wrap_reports_rfc1982_order`,
`crates/xrce/src/reliable.rs::tests::acknack_retransmit_of_lost_sample_across_wrap`

**Status:** done

### §8.4.14.1 State of the sender

**Spec:** §8.4.14.1, p. 85 — "The sender maintains two state variables
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

**Status:** done — the sender keeps the open messages instead of two numbers;
`window_base` corresponds to the highest acknowledged number plus one.

### §8.4.14.1 HEARTBEAT until full acknowledgement

**Spec:** §8.4.14.1, p. 85 — "While the HighestAcknowledgedSequenceNumber is
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

**Status:** done — every 500 ms. Figure 20 writes the transition to "Initial"
with `HiguestReceivedSeqNumber`, the text with the highest acknowledged
number.

### §8.4.14.2 State of the receiver

**Spec:** §8.4.14.2, p. 85–86 — "The receiver maintains two state variables
associated with the stream. The HighestReceivedSequenceNumber and the
HighestAnnouncedSequenceNumber. Each time a Message is received the
HighestReceivedSequenceNumber may be updated (assuming all previous messages
have been received). [...] Each time a HEARTBEAT is received the
HighestAnnouncedSequenceNumber may be adjusted."

**Repo:** `crates/xrce/src/reliable.rs` (`recv_data`, `drain_in_order`,
`pending_acknack`)

**Tests:** `crates/xrce/src/reliable.rs::tests::recv_data_buffers_in_order`,
`crates/xrce/src/reliable.rs::tests::pending_acknack_marks_missing_slots`

**Status:** done — the highest announced number is not stored; it enters the
ACKNACK on the HEARTBEAT as `hint_last_seen`.

### §8.4.14.2 The client sends ACKNACK while messages are missing

**Spec:** §8.4.14.2, p. 86 — "If the receiver is a n XRCE Client, then while
the HiguestReceivedSequenceNumber is less than the
HighestAnnouncedSequenceNumber, the received sends ACKNACK messages to request
the messages corresponding to the missing sequence numbers. These ACKNACK
messages may be periodic or optimized using on vendor specific mechanism."

**Repo:** `crates/xrce-client/src/lib.rs` (`handle_heartbeat`)

**Tests:** `crates/xrce-client/src/lib.rs::tests::heartbeat_is_answered_with_acknack`

**Status:** done — the client sends ACKNACK on every HEARTBEAT, not on a timer
of its own (Figure 21: `NackPeriod`); the agent sends HEARTBEAT every 500 ms
as long as messages are open.

### §8.4.14.2 The agent sends ACKNACK only on HEARTBEAT

**Spec:** §8.4.14.2, p. 86 — "If the receiver is the XRCE Agent, then it only
sends ACKNACK messages in response to receiving a HEARTBEAT."

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_heartbeat`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::out_of_order_reliable_messages_wait_for_the_gap_and_report_a_truthful_acknack`

**Status:** done — as in the item on §8.3.5.11.

---

## §8.5 XRCE Object Operation Traceability

### §8.5 Table 14 create_client

**Spec:** §8.5, Table 14, p. 86 — "XRCE Root / create_client / CREATE_CLIENT /
STATUS_AGENT".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create_client`,
`agent_status_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::create_client_happy_path_registers_and_replies_ok`,
`crates/xrce-agent/src/daemon/session.rs::tests::create_client_rejects_wrong_major_version`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_wrong_cookie_is_answered_with_a_status_agent`

**Status:** done — a wrong cookie is answered with STATUS_AGENT and
`STATUS_ERR_INVALID_DATA` (ed2ca7b49a9). The caption of Table 14 reads
“Predefined XRCE Objects from parsing the Example XML configuration XML file”,
like that of Table 16.

### §8.5 Table 14 get_info of the Root

**Spec:** §8.5, Table 14, p. 86 — "XRCE Root / get_info / GET_INFO / INFO".

**Repo:** `crates/xrce-agent/src/daemon/discovery.rs`
(`run_discovery_responder`); `crates/xrce-agent/src/daemon/session.rs`
(`handle_get_info`)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_in_a_session_returns_activity_and_representation`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_without_a_session_is_answered_for_the_agent_only`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_udp_listener_answers_get_info_before_a_session`

**Status:** done

### §8.5 Table 14 delete_client with STATUS_AGENT
**Spec:** §8.5, Table 14, p. 86 — "XRCE Root / delete_client / DELETE /
STATUS_AGENT".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`delete_client_reply`); `crates/xrce-agent/src/daemon/config.rs` (`delete_client_reply`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::delete_client_is_answered_with_status_unless_status_agent_is_named`, `crates/xrce-agent/src/daemon/config.rs::tests::the_reply_to_delete_client_can_be_set`

**Status:** done — `DELETE(OBJECTID_CLIENT)` is answered with STATUS (§8.3.5.4; in the IDL of Annex A only STATUS carries a `ResultStatus`); the STATUS_AGENT of Table 14 is the setting `delete_client_reply: status_agent`. The client takes a STATUS_AGENT that arrives with no CREATE_CLIENT pending for the answer to its DELETE. a44974c49d7.
### §8.5 Table 14 create

**Spec:** §8.5, Table 14, p. 86 — "XRCE ProxyClient / create / CREATE (flags for
creation) / STATUS".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_create`)

**Tests:** `crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §8.5 Table 14 update

**Spec:** §8.5, Table 14, p. 86 — "XRCE ProxyClient / update / CREATE (flags for
reuse) / STATUS".

**Repo:** `crates/xrce/src/object_store.rs` (`ObjectStore::create`);
`crates/xrce-agent/src/daemon/session.rs` (`handle_create`);
`crates/xrce-client/src/lib.rs` (`XrceClient::update_object`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_create_with_reuse_and_replace_updates_the_object`,
`crates/xrce-client/src/lib.rs::tests::update_object_sends_a_create_with_reuse_and_replace`,
`crates/xrce/src/object_store.rs::tests::create_reuse_and_replace_replaces_an_unequal_object`

**Status:** done — as in the item on §8.4.13, including the setting
`update_of_missing_object`. The table names only “flags for reuse”, §8.4.13 sets
`reuse` and `replace`; the update is a CREATE with both (8b949a13ef5).

### §8.5 Table 14 get_info of the ProxyClient

**Spec:** §8.5, Table 14, p. 86 — "XRCE ProxyClient / get_info / GET_INFO /
INFO".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_get_info`,
`object_info`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_an_object_returns_the_object_variant_it_was_created_with`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_missing_object_is_an_info_with_unknown_reference`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_a_data_writer_reports_its_activity`

**Status:** done — GET_INFO on an object gives INFO, on an unknown `ObjectId` an
INFO with `STATUS_ERR_UNKNOWN_REFERENCE`, without permission `STATUS_ERR_DENIED`
(65cd1820a04, d7338ca58f2).

### §8.5 Table 14 delete

**Spec:** §8.5, Table 14, p. 86 — "XRCE ProxyClient / delete / DELETE /
STATUS".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_delete`)

**Tests:** `crates/xrce-agent/tests/endpoint_client_fixtures.rs::delete_fixture_decodes_and_removes_the_object_create_added`,
`crates/xrce-agent/tests/replay_foreign_session.rs::replay_matches_the_real_agents_create_client_create_and_delete_replies`

**Status:** done

### §8.5 Table 14 write

**Spec:** §8.5, Table 14, p. 86 — "XRCE DataWriter / write / WRITE_DATA,
FRAGMENT / STATUS".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_write_data`,
`process_message_submessages`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_successful_write_is_answered_with_status_ok`

**Status:** done — WRITE_DATA direct and in FRAGMENT; a successful WRITE_DATA
gives STATUS with `STATUS_OK`, a failure its error code (dc0209b38fe). The
eProsima agent does not answer a successful WRITE_DATA; this agent follows the
text of the spec.

### §8.5 Table 14 read

**Spec:** §8.5, Table 14, p. 86 — "XRCE DataReader / read / READ_DATA / DATA,
FRAGMENT, STATUS".

**Repo:** `crates/xrce-agent/src/daemon/session.rs` (`handle_read_data`,
`pump_read`, `emit_possibly_fragmented`)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_sample_above_the_clients_mtu_on_a_reliable_stream_splits_into_fragments`,
`crates/xrce-agent/src/daemon/session.rs::tests::read_data_with_an_unhonorable_content_filter_is_rejected_not_silently_unfiltered`

**Status:** done

---

## §9 XRCE Agent Configuration

### §9.1 Prior knowledge of the agent for REPRESENTATION_BY_REFERENCE

**Spec:** §9.1, p. 87 — "The XRCE Agent may be configured such that it has a
priori knowledge XRCE Objects. This allows XRCE Clients to reference and create
XRCE Objects in a very compact manner using the representation format
REPRESENTATION_BY_REFERENCE, see clause 7.7.3.3.1 REPRESENTATION_BY_REFERENCE
format."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — permission. The clause titled
"REPRESENTATION_BY_REFERENCE format" is §7.7.3.1.1; §7.7.3.3.1 belongs to XRCE
Type.

### §9.1 Two standard mechanisms, others allowed

**Spec:** §9.1, p. 87 — "This specification provides two standard mechanism s
to configure the XRCE Agent. Implementations may also provide additional
mechanisms: Remote configuration using the XRCE Protocol; Local file-based
configuration".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the two mechanisms are in the
items on §9.2 and §9.3. The profile files in the Fast DDS format
(`xml_profile_files:`) are an additional mechanism.

### §9.2 ConfigurationClient with the ordinary protocol

**Spec:** §9.2, p. 87 — "An application may use a n XRCE Client with the only
purpose of defining and creating XRCE Objects that are intended for other
applications. [...] The protocol used by the XRCE ConfigurationClient is the
same used by any other XRCE Client. The only difference is that an XRCE
ConfigurationClient never uses the READ_DATA or WRITE messages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description of a role of the client; it
needs no message of its own.

### §9.2 Objects of the ConfigurationClient for other clients

**Spec:** §9.2, p. 87–88 — "Any other XRCE Client can reference XRCE Objects
created by an XRCE ConfigurationClient." Figure 22: the ConfigurationClient
creates with CREATE, another client with CREATE(REPRESENTATION_BY_REFERENCE)
and reads with READ_DATA. "These XRCE Objects are later referenced by a
different XRCE Client using the representation formats
REPRESENTATION_BY_REFERENCE."

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`SystemDefinitions`);
`crates/xrce-agent/src/daemon/session.rs` (`definition_to_register`,
`rebuild_profiles`, `ensure_predefined`);
`crates/xrce-agent/src/daemon/profiles.rs` (`AgentProfiles::predefined_objects`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_client_writes_through_a_preconfigured_data_writer_without_a_create`

**Status:** done — a ConfigurationClient creates QosProfile, Application and
DomainParticipant with CREATE (XML); another client creates them with
`REPRESENTATION_BY_REFERENCE` as long as the ConfigurationClient's object exists
(the test covers QosProfile and Application). Type and DomainParticipant take
the same path (`definition_to_register`), without a test of their own. A Domain
has no `ObjectKind` (§7.7.5, Annex A) and comes from the files. Figure 22 calls
the answer to CREATE_CLIENT `STATUS_CLIENT`; the submessage is called
STATUS_AGENT (3aea52dd70a, b6e55cfa859).

### §9.2 Another network or another transport

**Spec:** §9.2, p. 87 — "Note that the XRCE ConfigurationClient may be
communicating with the Agent using a different network or transport, which may
not have the same constraints as a typical XRCE Client."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — note.

### §9.3 Loading an XML file according to dds-xml_system_example.xsd

**Spec:** §9.3, p. 88 — "The XRCE Agent shall provide a configuration or
run-time option to load a n XML file formatted according to the schema defined
in the [DDS-XML] machine-readable file dds-xml_system_example.xsd."

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`xml_profile_files`,
`qos_library_files`); `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::load`, `AgentProfiles::add_profiles`);
`crates/xrce-agent/src/daemon/definitions.rs`
(`SystemDefinitions::add_document`)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::types_and_qos_libraries_in_a_dds_document_are_read`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`

**Status:** done — `xml_profile_files:` takes a document with the root `<dds>`
(or a library as the root) and reads `<types>`, `<qos_library>`,
`<domain_library>`, `<domain_participant_library>` and `<application_library>`
(dc63d65691f, 3aea52dd70a). The schema file is not in the cache; checked against
§9.3.1.

### §9.3 Domain, Application and DomainParticipant libraries of the configuration file

**Spec:** §9.3, p. 88 — as above.

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::add_profiles`); `crates/xrce-agent/src/daemon/definitions.rs`
(`add_domain_library`, `add_participant_library`, `add_application_library`)

**Tests:** `crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::a_library_can_be_the_root_and_a_clash_of_ids_is_refused`

**Status:** done — the three libraries are read, also as the root of the
document, with `base_name` inheritance. Other elements of `<dds>` appear only in
the start-up log (`AgentProfiles::unused`) (dc63d65691f, 3aea52dd70a).

### §9.3 One XRCE object per element, for all clients by reference

**Spec:** §9.3, p. 88 — "The XRCE Agent shall parse the XML file and for each of
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

**Status:** done — the agent knows every element of Table 15 as an object with
the `ObjectId` from the MD5 of the reference (§9.3, Table 16); every client gets
it with CREATE and `REPRESENTATION_BY_REFERENCE` or on first access to the
`ObjectId`. Two top-level objects with the same `ObjectId` are refused when the
file is loaded (03371b4965c, b6e55cfa859).

### §9.3 Table 15 <types>

**Spec:** §9.3, Table 15, p. 88 — "<types> / XRCE Type. / The created XRCE Types
shall be referenceable using their fully qualified name, which includes the
names of enclosing modules. For example: 'MyModule::MyNestedModule::MyStructType'"

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs` (`add_types`,
`AgentProfiles::resolve_type_ref`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`types`);
`crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, branch
`ObjectKind::Type`)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::types_and_qos_libraries_in_a_dds_document_are_read`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_xml_type_library_compiles_to_named_complete_types`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_configured_type_is_known_to_every_client`

**Status:** done — the types are known to all clients under their fully
qualified name with modules (`ShapesDemoTypes::ShapeType`); a topic finds them
through `type_name`. CREATE with `OBJK_TYPE` and `REPRESENTATION_BY_REFERENCE`
accepts a known name (`plan_dds_create`, without a test of its own). References
of a `<types>` to other known types: item on §7.7.3.3.2 (dc63d65691f,
3aea52dd70a).

### §9.3 Table 15 <qos_profile>

**Spec:** §9.3, Table 15, p. 88 — "<qos_profile> (Child of <qos_library>) /
XRCE QosProfile. / The created XRCE Types shall be referenceable using their
fully qualified name, which includes the names of enclosing Qos Profile
Library. For example: 'MyProfileLibrary::MyQosProfile'"

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::qos_profile`, `AgentProfiles::predefined_objects`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_qos_profile`);
`crates/xrce-agent/src/daemon/session.rs` (`plan_dds_create`, branch
`ObjectKind::QosProfile`)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::types_and_qos_libraries_in_a_dds_document_are_read`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_qos_profile_reference_applies_the_profile`,
`crates/xrce-agent/src/daemon/session.rs::tests::what_a_configuration_client_defines_is_referenced_by_another_client`

**Status:** done — an endpoint names the profile as `Library::Profile`; CREATE
with `OBJK_QOSPROFILE` makes a profile that later CREATEs use (3aea52dd70a). The
row writes “XRCE Types” for the QosProfile.

### §9.3 Table 15 <domain>

**Spec:** §9.3, Table 15, p. 89 — "<domain> (Child of <domain_library>) / XRCE
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

**Status:** done — `<domain_library>` is read; a Domain is named
`Library::Domain` and resolved by `domain_ref` (XML) and `domain_reference`
(binary) of a DomainParticipant. The Domain has no `ObjectKind` (§7.7.5, Annex
A): a CREATE cannot name it, it has no `ObjectId` (dc63d65691f, b4cd825c268).

### §9.3 Table 15 <topic> in <domain>

**Spec:** §9.3, Table 15, p. 89 — "<topic> (Child of <domain>) / XRCE Topic /
The created XRCE Topic shall be referenceable using its name from any
DomainParticipant that references the Domain where the Topic is defined. For
example: 'ExampleTopic'"

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`DomainDef::topics`,
`SystemDefinitions::scope_of`, `ParticipantScope::topic`)

**Tests:** `crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`

**Status:** done — the topics of a Domain are in the scope of every
DomainParticipant that names the Domain (§7.7.3.7.1); a DataWriter or DataReader
finds them under their name, also through the `base_name` of a Domain
(3aea52dd70a).

### §9.3 Table 15 <application>

**Spec:** §9.3, Table 15, p. 89 — "<application> (Child of
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

**Status:** `done` — Commits `dc63d65691f`, `b6e55cfa859`. `<application_library>` is
read, also as the root of a file. The Application is known under
"Library::Application" (example "Apps::App"); its ObjectId comes from the MD5 of
that string. Another client creates it with REPRESENTATION_BY_REFERENCE for as
long as the object that defined it exists.

### §9.3 Table 15 <domain_participant>

**Spec:** §9.3, Table 15, p. 89 — "<domain_participant> (Child of
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
`<domain_participant_library>` is read, also as the root of a file. The
participant is known under "Library::Participant", inside an Application under
"Library::Application::Participant" (Table 16:
"MyApplications::ShapesDemoApp::MyParticipant"); its ObjectId comes from the MD5
of the string. The form `<participant profile_name="…">` of a Fast DDS profiles
file stays readable beside it.

### §9.3 Table 15 <topic> in <domain_participant>

**Spec:** §9.3, Table 15, p. 89 — "<topic> (Child of <domain_participant>) /
XRCE Topic / The created XRCE Topic shall be referenceable using its name from
any objects in the same DomainParticipant. For example: 'ExampleTopic'"

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`plan_participant`: the
topics of the participant and of its domain);
`crates/xrce-agent/src/daemon/session.rs` (`create_nested`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_topic_is_created_by_name_or_by_register_type_ref`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::a_participant_library_and_inheritance`

**Status:** `done` — Commits `3aea52dd70a`, `9239562286c`. A topic in the participant
is an object with the ObjectId of its name (example "Square" {0xce, 0xb2}). The
objects of the same participant name it with that name (`topic_ref="Square"` of
a DataWriter becomes the ObjectId of the topic); a name the participant does not
hold is refused. The form `<topic profile_name="…">` in the Fast DDS format
stays readable beside it.

### §9.3 Table 15 <publisher> and <subscriber>

**Spec:** §9.3, Table 15, p. 89 — "<publisher> <subscriber> (Child of
<domain_participant>) / XRCE Publisher, XRCE Subscriber / The created XRCE
Publisher or Subscriber shall be referenceable using their name. No
qualification is necessary since these entities are always referenced within
the scope of a DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/definitions.rs` (`plan_participant`);
`crates/xrce-agent/src/daemon/session.rs` (`create_nested`);
`crates/xrce-agent/src/daemon/dcps_bridge.rs` (`container_for` checks that topic
and Publisher/Subscriber belong to the same participant)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_publisher_with_nested_data_writers_creates_them`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::what_cannot_be_planned_is_refused`

**Status:** `done` — Commits `3aea52dd70a`, `9239562286c`. Publisher and Subscriber are
objects with the ObjectId of their name (example "MyPublisher" {0x13, 0xe3}),
without a library in front, and apply within the scope of their participant. Two
participants of one client that define an entity of the same name derive the
same ObjectId (the spec derives from the name only); the second is
`STATUS_ERR_ALREADY_EXISTS` (read from the code, not covered by a test).

### §9.3 Table 15 <data_writer> and <data_reader>

**Spec:** §9.3, Table 15, p. 89 — "<data_writer> < data_reader> (Child of
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
and DataReader are objects with the ObjectId of their name (example
"MySquareWriter" {0x1c, 0xc5}) and hang under their Publisher or Subscriber
(§9.3.1; Table 15 names `<domain_participant>` as the parent element). A client
writes and reads through them without a CREATE.

### §9.3 ObjectId from the MD5 of the reference

**Spec:** §9.3, p. 89 — "The XRCE Objects created from the file-based
configuration shall have their ObjectId automatically derived from the
REPRESENTATION_BY_REFERENCE string. Specifically, the ObjectIdPrefix (see
7.7.6) shall be set to the first 2 bytes of the MD5 hash computed on the
REPRESENTATION_BY_REFERENCE string. The MD5 treats each string character as a
byte and does not include the NUL terminating character of the string."
Example: "MyWriter", MD5 0x03e26181adfef529038bf0dce7cab871, ObjectIdPrefix
{0x03, 0xe2}.

**Repo:** `crates/xrce/src/object_id.rs` (`ObjectId::from_reference`: MD5 of the
string without the NUL, the first two bytes, the low 4 bits replaced by the
ObjectKind)

**Tests:** `crates/xrce/src/object_id.rs::tests::from_reference_matches_the_examples_of_the_spec`

**Status:** `done` — Commits `03371b4965c`. "MyWriter" gives the MD5
0x03e26181adfef529038bf0dce7cab871 (test with the crate `md-5`; also recomputed
with Python `hashlib`, measured), the ObjectIdPrefix {0x03, 0xe2} and, with the
kind DataWriter, the ObjectId {0x03, 0xe5}. The last sentence of the spec writes
"The ObjectIdPrefix shall be computed by combining the ObjectIdPrefix with the
ObjectKind"; the ObjectId is meant. The comment in
`crates/xrce/src/xml_config.rs` refers to `ObjectId::from_reference`.

### §9.3.1 Example file

**Spec:** §9.3.1, p. 90–91 — "The following XML file could be used to configure
an XRCE Agent." `<types>` with module `ShapesDemoTypes`, `<qos_library
name="MyQosLibrary">`, `<application_library name="MyApplications">` with
`<application name="ShapesDemoApp">`, participant `MyParticipant`, three
topics, `MyPublisher` with `MySquareWriter` and `MyCircleWriter`,
`MySubscriber` with `MyTriangleRdr`.

**Repo:** `crates/xrce-agent/src/daemon/profiles.rs`
(`AgentProfiles::add_profiles` reads the file)

**Tests:** `crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`

**Status:** `n/a (informative)` — example. The test loads the file with the
domain and the type added. `domain_ref` names
"ShapesDomainLibrary::ShapesDomain", which the file does not define;
`register_type type_ref="ShapeType"` names the type without its module
`ShapesDemoTypes`.

### §9.3.1 Table 16 predefined objects

**Spec:** §9.3.1, Table 16, p. 92 — "An XRCE Agent loading the above
configuration file would have the pre-defined XRCE Objects shown in Table 16."
Twelve rows with reference, ObjectPrefix and ObjectId, e.g.
"ShapesDemoTypes::ShapeType" {0x59, 0x51} {0x59, 0x5a}.

**Repo:** `crates/xrce/src/object_id.rs` (`ObjectId::from_reference`);
`crates/xrce-agent/src/daemon/profiles.rs` (`AgentProfiles::predefined_objects`)

**Tests:** `crates/xrce/src/object_id.rs::tests::from_reference_matches_the_examples_of_the_spec`,
`crates/xrce-agent/src/daemon/profiles.rs::tests::the_example_file_of_the_spec_defines_the_objects_of_table_16`,
`crates/xrce-agent/src/daemon/definitions.rs::tests::the_example_file_defines_the_objects_of_table_16`

**Status:** `done` — Commits `03371b4965c`, `dc63d65691f`. The test in `object_id.rs`
recomputes all twelve rows of Table 16 from their own string (prefix, ObjectId,
kind in the low nibble). The agent tests check the ObjectIds of the objects it
derives from the file. Two strings of Table 16 differ from the file in §9.3.1:
"MyApplications::SimpleShapesDemoApplication" instead of
"MyApplications::ShapesDemoApp" (MD5 prefix of "ShapesDemoApp": {0xeb, 0x1d})
and "MyTriangleReader" instead of "MyTriangleRdr" (MD5 prefix of
"MyTriangleRdr": {0x50, 0x38}). The agent derives from the names of the file by
the rule of §9.3, so the ObjectIds of these two rows differ from the table.
There is no setting for the other reading. The participant is named
"MyApplications::ShapesDemoApp::MyParticipant", not after the pattern of Table
15.

---

## §10 XRCE Deployments

### §10 Operations with one agent, exchange through DDS

**Spec:** §10, p. 93 — "All the operations described in the DDS-XRCE PIM
pertain to the interaction of a client application with a single DDS-XRCE
Agent. [...] client applications may interact with each other despite
connecting to different DDS-XRCE Agents. These interactions would happen as a
consequence of the DDS-XRCE Agents creating and performing operations on DDS
DomainParticipant entities".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description.

### §10.1 Client through the agent to any DDS participant

**Spec:** §10.1, p. 93 — "The DDS DomainParticipant will discover the proxy DDS
Entities that the XRCE Agent creates on behalf of the Client and with use the
standard DDS-RTPS Interoperability protocol to communicate wit the Agent. The
XRCE Client will communicate with the XRCE Agent using the XRCE Protocol. Using
this protocol it can direct the XRCE Agent to create new DDS entities and use
these entities to read and write data on the DDS Global Data Space."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`RealDdsPort` l. 310,
`runtime_for` l. 396)

**Tests:** `crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`,
`crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::full_chain_against_a_real_leased_domain`

**Status:** done — the first test writes and reads against a DCPS participant
of its own in the same process.

### §10.2 Clients at different agents through DDS
**Spec:** §10.2, p. 93 — "XRCE Agents appear as DDS DomainParticipants in the
DDS Dom ain. For this reason XRCE Client applications that are connected to
different XRCE Agents will communicate with each other without the need for
further configuration."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`runtime_instance`, `runtime_for`)

**Tests:** `crates/xrce-agent/tests/two_agents.rs::two_clients_on_two_agents_exchange_a_sample_through_dds`

**Status:** done — two daemons in one domain, a DataWriter through the first, a DataReader through the second: the sample arrives with equal bytes. The GUID prefixes of the runtimes were derived from the domain and the host alone, so two agents on one host did not discover each other; they carry the process and a counter (3068f2df50c). The test is written, not run.
### §10.3 Several clients at one agent as with entities of their own

**Spec:** §10.3, p. 94 — "Multiple XRCE Client applications may be connected to
the same XRCE Agent. In it up to the implementation of the XRCE Agent whether
the DDS Entities it creates are exclusive to each XRCE Client or alternatively
are shared across XRCE Clients. However, the behavior observable by the XRCE
Client shall be as if the DDS XRCE Agent creates separate DDS Objects exclusive
to each XRCE Client. [...] If the XRCE Agent shares DDS entities among
different XRCE Clients, then the requiremen t to behave 'as if' each had its
own separate entities requires that the local DDS DataWriter entities discover
and match the local DDS DataReader entities in the same DomainParticipant."

**Repo:** `crates/xrce-agent/src/daemon/dcps_bridge.rs` (`runtime_for` l. 396:
one `DcpsRuntime` per domain for all clients, a further one per domain and
USER_DATA through `own_runtime_slot`; `create_writer` l. 654, `create_reader` l.
716)

**Tests:** `crates/xrce-agent/src/daemon/dcps_bridge.rs::tests::two_large_samples_reach_a_default_reader_on_the_same_agent`

**Status:** `done` — the writer of one client reaches the reader of another
client in the same `DcpsRuntime`. A separate GUID and the QoS of the participant
per client are not checked; by the code, a participant with its own USER_DATA
gets a `DcpsRuntime` of its own (read, not covered by a test).

### §10.3 Shortcut between local writer and reader

**Spec:** §10.3, p. 94 — "An implementation of an XRCE Agent may choose to
create faster communication path between the local XRCE DataWriter and
DataReader objects [...]. This 'shortcut' can be implemented as an
optimization".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — permission; the agent goes through the
`DcpsRuntime`.

### §10.4 Federation of agents

**Spec:** §10.4, p. 95 — "The specification supports federated deployments
where XRCE Agents appear as Clients to other XRCE Agents. In other to support
these deployments the XRCE Agent implementation must implement the client -side
of the XRCE Protocol in addition to the server part. Supporting this kind of
deployment is an implementation decision".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — decision of the implementation; the agent
has no client side.

### §10.5 Direct communication between applications

**Spec:** §10.5, p. 96 — "The specification supports applications having
direct communications using only the XRCE Protoco l. In order to do this each
application must implement both the XRCE Client and the XRCE Agent part of the
protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deployment option.

### §10.6 Combined deployment

**Spec:** §10.6, p. 97–98 — "Figure 28 below illustrates a scenario where the
different deployments are combined into a single system."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — figure.

---

## §11 Transport Mappings

### §11.1 Not bound to one transport

**Spec:** §11.1, p. 99 — "The XRCE protocol is not limited to any specific
transports. It can be mapped to most existing network transports such as UDP,
TCP and low bandwidth transports such as Bluetooth, ZigBee and 6LoWPAN."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description; ZeroDDS has UDP, TCP and a
serial line (items on §11.2, §11.3 and Annex C).

### §11.1 Expected properties (1) to (4)

**Spec:** §11.1, p. 99 — "To run without additional overhead, it is expected
that the transport supports the following functionality: (1) Deliver messages
of at least 64 bytes. (2) Handle the integrity of messages, dropping any
messages that are corru pted. [...] (3) Provide the size of the received
message as well as the source address. [...] (4) Support bi-directional
communication."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — expectation of the transport. The serial
line adds length, addresses and CRC (items on Annex C).

### §11.1 Expected property (5) security

**Spec:** §11.1, p. 99 — "(5) Provide transport-level security, specifically the
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

**Status:** `n/a (informative)` — expectation of the transport. Under the
features `tls` and `dtls` the agent has the listeners `kind: tls` and `kind:
dtls` with the agent's certificate and key (`01470e273bc`): the client
authenticates the agent, the messages travel encrypted (item on §8.3.2.4
"Protection of the ClientKey by the transport"). The listeners `udp`, `tcp` and
`serial` are not secured. The tests run only with the respective feature.

### §11.1 Not required: reliability, ordering, notification of losses

**Spec:** §11.1, p. 99 — "The following functionality is explicitly not
required from the transport: (1) It does not need to provide reliability. [...]
(2) It does not need to provide ordering. [...] (3) It does not need to provide
notification of dropped messages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — permission; reliability and ordering are in
the items on §8.3.2.3 and §8.4.14.

### §11.1 Envelope for missing properties

**Spec:** §11.1, p. 99 — "Transports that do not meet some of the above
pre-requisites may still be used by adding the missing information as an
envelope around the XRCE message. [...] If the transport does not support
integrity a CRC suffix could be added to the XRCE message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — possibility; the envelopes for TCP and the
serial line are in the items on §11.3.3 and Annex C.

### §11.2 UDP without security, DTLS or VPN

**Spec:** §11.2, p. 99 — "The UDP transport meets all the functionality listed
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

**Status:** `n/a (informative)` — note. Under the feature `dtls` the agent has
the listener `kind: dtls` (DTLS 1.2 with the agent's certificate,
`01470e273bc`); a plain-text CREATE_CLIENT on that port is not answered. The
tests run only with that feature.

### §11.2.1 UDP v4 with ADDRESS_FORMAT_MEDIUM

**Spec:** §11.2.1, p. 99 — "When XRCE is mapped to the UDP v4, the
TransportLocator union shall use the TransportLocatorFormat discriminator
ADDRESS_FORMAT_MEDIUM. This selects the member medium_locator of type
TransportLocatorMedium [...] struct TransportLocatorMedium { octet address[4];
unsigned short port; };"

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators` l. 438);
`crates/xrce/src/transport_locator.rs` (`TransportLocatorMedium`,
`TransportLocator::encode_member`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`,
`crates/xrce/src/transport_locator.rs::tests::medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag`,
`crates/xrce/src/transport_locator.rs::tests::a_udp_locator_carries_the_default_agent_port`

**Status:** `done` — Commits `c304e509660`. a UDP listener with an IPv4 address is
announced as `TransportLocator::Medium` (discriminator ADDRESS_FORMAT_MEDIUM):
four address octets, one padding octet, the port as an `unsigned short` in the
byte order of the E flag.

### §11.2.1 UDP v6 with ADDRESS_FORMAT_LARGE

**Spec:** §11.2.1, p. 99–100 — "When XRCE is mapped to the UDP v6, the
TransportLocator union shall use the TransportLocatorFormat discriminator
ADDRESS_FORMAT_LARGE. [...] struct TransportLocatorLarge { octet address[16];
unsigned long port; }; The address field shall contain the IP v6 address and the
port field shall contain the UDP/IP v6 port number."

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators` l. 438);
`crates/xrce/src/transport_locator.rs` (`TransportLocatorLarge`,
`TransportLocator::encode_member`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`,
`crates/xrce/src/transport_locator.rs::tests::large_has_three_padding_octets_and_a_32_bit_port`

**Status:** `done` — Commits `c304e509660`. a UDP listener with an IPv6 address is
announced as `TransportLocator::Large`: 16 address octets, three padding octets,
the port as an `unsigned long`.

### §11.2.2 Agent as UDP server, client as UDP client

**Spec:** §11.2.2, p. 100 — "The UDP Server sends replies back the Client using
the address and port received in the message. [...] When communicating over UDP
the XRCE Agent shall behave as an UDP Server and the XRCE Client as the UDP
Client."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`run_udp_listener` l.
428, `PeerTable`, `Route::Udp`); `crates/xrce/src/transport_udp.rs`
(`XrceUdpSender::bind` l. 54)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::udp_listener_answers_create_client_over_loopback`,
`crates/xrce-agent/src/daemon/transport.rs::tests::a_client_registering_again_is_answered_at_its_new_address`

**Status:** done — the answer goes to the address under which the client last
registered with CREATE_CLIENT; without an entry, to the source address.

### §11.2.2 Port of the agent pre-configured

**Spec:** §11.2.2, p. 100 — "The XRCE Agent shall be pre-configured with the
port number it shall listen to."

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`ListenerConfig::Udp`,
default `0.0.0.0:2018`)

**Tests:** `crates/xrce-agent/src/daemon/config.rs::tests::default_config_has_one_udp_listener_on_2018`,
`crates/xrce-agent/src/daemon/config.rs::tests::a_listener_can_be_written_as_one_string`

**Status:** done — the specification names no default port.
`agent_default_port` in `crates/xrce/src/transport_udp.rs` (l. 27) computes
7400 + 4 · domain; the agent does not use the function.

### §11.2.2 Address and port of the agent pre-configured in the client

**Spec:** §11.2.2, p. 100 — "The XRCE Client shall be pre-configured with the
UDP/IP address and port of the XRCE Agent."

**Repo:** `crates/xrce/src/transport_udp.rs` (`XrceUdpSender::bind` l. 54);
`crates/xrce-client/src/lib.rs` (`ClientTransport` l. 89)

**Tests:** `crates/xrce/src/transport_udp.rs::tests::loopback_roundtrip_create_client`,
`crates/xrce-agent/tests/e2e_udp_loopback.rs::e2e_udp_loopback_keyed_dispose_reliable_besteffort_and_fragments`

**Status:** done — `XrceClient` gets the address through its
`ClientTransport`; `crates/xrce-client` itself has no UDP transport, the test
builds it in `e2e_udp_loopback.rs`.

### §11.2.3 No envelope, exactly one message per datagram

**Spec:** §11.2.3, p. 100 — "The mapping of the XRCE Protocol to UDP/IP does not
add any additional envelopes around the XRCE message. The UDP/IP payload shall
contain exactly one XRCE message."

**Repo:** `crates/xrce/src/transport_udp.rs` (`send_message` l. 67,
`recv_message` l. 89)

**Tests:** `crates/xrce/src/transport_udp.rs::tests::rfc768_udp_delivers_one_datagram_per_message_with_boundaries_intact`,
`crates/xrce/src/transport_udp.rs::tests::rfc768_datagram_larger_than_udp_payload_limit_is_rejected`

**Status:** done

### §11.2.4 Multicast address and port of the agent

**Spec:** §11.2.4, p. 100 — "XRCE Agent discovery may be done using UDP/IP
multicast. The XRCE Agents shall be pre-configured with the multicast address
and port number they shall listen to. By default, they shall be the address
239.255.0.2 and the port 7400."

**Repo:** `crates/xrce/src/discovery.rs` (`XRCE_DISCOVERY_GROUP`,
`XRCE_DISCOVERY_PORT`); `crates/xrce-agent/src/daemon/discovery.rs`
(`bind_discovery_socket` l. 146); `crates/xrce-agent/src/daemon/config.rs`
(`discovery_address`, `discovery_port`)

**Tests:** `crates/xrce/src/discovery.rs::tests::discovery_constants_match_spec`,
`crates/xrce/src/discovery.rs::tests::the_discovery_port_is_bound_shared`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_discovery_group_is_the_one_given`,
`crates/xrce-agent/src/daemon/config.rs::tests::the_discovery_group_can_be_set`

**Status:** `done` — Commits `5798b040952`. default group 239.255.0.2 and port 7400;
`discovery.address:` and `discovery.port:` set both (item below).

### §11.2.4 Setting the multicast address

**Spec:** §11.2.4, p. 100 — as above.

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`discovery_address`, key
`discovery.address`); `crates/xrce-agent/src/daemon/discovery.rs`
(`bind_discovery_socket` l. 146); `crates/xrce-agent/src/daemon/server.rs` (call
l. 400)

**Tests:** `crates/xrce-agent/src/daemon/config.rs::tests::the_discovery_group_can_be_set`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::the_discovery_group_is_the_one_given`

**Status:** `done` — Commits `5798b040952`. the agent joins the configured group; the
configuration and `bind_discovery_socket` refuse an address that is no IPv4
multicast address. In the client the multicast address is an entry of the
candidate list of `discover_over_udp`.

### §11.2.4 Periodic GET_INFO of the client to the multicast address

**Spec:** §11.2.4, p. 100 — "To discover Agents via multicast the XRCE Client
shall send the GET_INFO message (see 8.3.5.3) periodically to the configured
multicast address and port. [...] The XRCE Client shall stop sending the
periodic message once it receives a suitable response from an Agent."

**Repo:** `crates/xrce-client/src/discovery.rs` (`AgentDiscovery::poll`,
`on_datagram`, `discover_over_udp`); `crates/xrce/src/discovery.rs`
(`MulticastDiscovery::send_multicast`)

**Tests:** `crates/xrce-client/src/discovery.rs::tests::get_info_goes_to_the_candidates_every_period`,
`crates/xrce-client/src/discovery.rs::tests::a_suitable_answer_ends_the_search`,
`crates/xrce-client/src/discovery.rs::tests::the_search_over_udp_finds_the_agent_that_answers`

**Status:** `done` — Commits `5798b040952`. `GET_INFO(OBJECTID_AGENT)` without session
and without ClientKey goes out at the first call and then every period to all
addresses of the list (multicast address and port or unicast candidates); a
suitable answer (INFO with `STATUS_OK` and the same major version) ends the
sending, a `STATUS_ERR_INCOMPATIBLE` or another major version does not. The test
with sockets sends to unicast candidates on loopback; reception on a real
multicast group is not checked.

### §11.2.4 Answer of the agent with its TransportLocators

**Spec:** §11.2.4, p. 100 — "This message shall invoke the get_info operation
(see 7.8.2.2) on the XRCE Agent, which shall respond and include its
TransportLocators."

**Repo:** `crates/xrce-agent/src/daemon/discovery.rs` (`run_discovery_responder`
l. 43, `build_info_reply` l. 107); `crates/xrce-agent/src/daemon/server.rs`
(`listener_locators` l. 438)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`

**Status:** `done` — Commits `65cd1820a04`, `c304e509660`. INFO with
`AGENT_ActivityInfo.address_seq` as the `TransportLocatorSeq` of the UDP and TCP
listeners (Medium for IPv4, Large for IPv6) and with the `AGENT_Representation`.
The agent does not announce listeners with DTLS or TLS and the serial line;
Annex A has no `TransportLocatorFormat` for them.

### §11.2.4 Unicast discovery over a list of addresses

**Spec:** §11.2.4, p. 100 — "XRCE Agent discovery may be done using UDP/IP
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

**Status:** `done` — Commits `5798b040952`. the client takes a list of candidate
addresses, sends `GET_INFO` periodically to all of them and stops at a suitable
answer.

### §11.3 TCP without security and length, TLS or VPN

**Spec:** §11.3, p. 100–101 — "The TCP transport meets all the functionality
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

**Status:** `n/a (informative)` — note; the length is in the item on §11.3.3.
Under the feature `tls` the agent has the listener `kind: tls` (TLS with the
agent's certificate, `01470e273bc`); the 2-byte length prefix is inside the
encrypted stream. The tests run only with that feature.

### §11.3.1 TCP v4 with ADDRESS_FORMAT_MEDIUM

**Spec:** §11.3.1, p. 101 — "When XRCE is mapped to the TCP/IP version 4, the
TransportLocator union shall use the TransportLocatorFormat discriminator
ADDRESS_FORMAT_MEDIUM."

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators` l. 438);
`crates/xrce/src/transport_locator.rs` (`TransportLocatorMedium`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`,
`crates/xrce/src/transport_locator.rs::tests::medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag`

**Status:** `done` — Commits `c304e509660`. a TCP listener with an IPv4 address is
announced as `TransportLocator::Medium` (ADDRESS_FORMAT_MEDIUM); the
discriminator 0x04 no longer exists.

### §11.3.1 TCP v6 with ADDRESS_FORMAT_LARGE

**Spec:** §11.3.1, p. 101 — "When XRCE is mapped to the TCP/IP version 6, the
TransportLocator union shall use the TransportLocatorFormat discriminator
ADDRESS_FORMAT_LARGE. [...] The address field shall contain the IP v6 address
and the port field shall contain the TCP/IP v6 port number."

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (`listener_locators` l. 438);
`crates/xrce/src/transport_locator.rs` (`TransportLocatorLarge`)

**Tests:** `crates/xrce-agent/src/daemon/server.rs::tests::listener_locators_maps_udp_tcp_and_ipv6`,
`crates/xrce/src/transport_locator.rs::tests::large_has_three_padding_octets_and_a_32_bit_port`

**Status:** `done` — Commits `c304e509660`. a TCP listener with an IPv6 address is
announced as `TransportLocator::Large`, with the port number in `locator_port`.

### §11.3.2 Agent as TCP server, client as TCP client

**Spec:** §11.3.2, p. 101 — "The TCP Server accepts the connection from the
Client. This establishes a bi-directional communication channel. [...] The TCP
Server can send and receive messages to and from the Client. When
communicating over TCP the XRCE Agent shall behave as a TCP Server and the XRCE
Client as the TCP Client."

**Repo:** `crates/xrce/src/transport_tcp.rs` (`XrceTcpServer::accept` l. 160,
`XrceTcpClient::connect` l. 52); `crates/xrce-agent/src/daemon/transport.rs`
(`run_tcp_listener` l. 550, `run_tcp_connection` l. 485)

**Tests:** `crates/xrce/src/transport_tcp.rs::tests::tcp_loopback_create_client_roundtrip`,
`crates/xrce/src/transport_tcp.rs::tests::tcp_loopback_three_message_chain`,
`crates/xrce-agent/src/daemon/transport.rs::tests::a_tcp_client_gets_the_messages_of_the_agent_over_its_connection`

**Status:** done — the roles are checked in the transport and at the agent's
TCP listener; the agent sends over the connection without a request too (items
below).

### §11.3.2 TCP listener of the agent

**Spec:** §11.3.2, p. 101 — as above.

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`run_tcp_listener` l.
550, `run_tcp_connection` l. 485)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::a_tcp_client_gets_the_messages_of_the_agent_over_its_connection`

**Status:** done — the test starts the listener, registers a client over TCP
and receives STATUS_AGENT. The agent does not use `XrceTcpServer`.

### §11.3.2 Messages of the agent without a request over TCP

**Spec:** §11.3.2, p. 101 — "The TCP Server can send and receive messages to
and from the Client."

**Repo:** `crates/xrce-agent/src/daemon/transport.rs` (`run_ticker` l. 410,
`deliver` l. 367, `Route`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::a_tcp_client_gets_the_messages_of_the_agent_over_its_connection`

**Status:** `done` — Commits `2d3b8106888`. one thread (`run_ticker`, transport.rs
l. 410) calls `tick` for all listeners; `deliver` (l. 367) sends each message
the way its client registered on: the socket of a UDP listener or the queue of
the writer of a TCP connection or serial line. The routes of a connection go
with it. The test receives over the connection a message the agent sends
without a request.

### §11.3.2 Port of the agent pre-configured

**Spec:** §11.3.2, p. 101 — "The XRCE Agent shall be pre-configured with the
port number it shall listen to."

**Repo:** `crates/xrce-agent/src/daemon/config.rs` (`ListenerConfig::Tcp`)

**Tests:** `crates/xrce-agent/src/daemon/config.rs::tests::a_listener_can_be_written_as_one_string`

**Status:** done

### §11.3.2 Address and port of the agent pre-configured in the client

**Spec:** §11.3.2, p. 101 — "The XRCE Client shall be pre-configured with the
TCP/IP address and port of the XRCE Agent."

**Repo:** `crates/xrce/src/transport_tcp.rs` (`XrceTcpClient::connect` l. 52);
`crates/xrce-client/src/discovery.rs` (`TcpTransport`, `connect_over_tcp`)

**Tests:** `crates/xrce/src/transport_tcp.rs::tests::tcp_loopback_create_client_roundtrip`,
`crates/xrce-client/src/discovery.rs::tests::the_search_over_tcp_connects_to_the_agent_that_accepts`

**Status:** `done` — Commits `5798b040952`. `crates/xrce-client` has a
`ClientTransport` over TCP in `TcpTransport` (feature `std`); `connect_over_tcp`
connects to the pre-configured addresses and sends CREATE_CLIENT.

### §11.3.3 Length in two bytes, little-endian

**Spec:** §11.3.3, p. 101 — "The mapping of the XRCE Protocol to TCP/IP adds a
2-byte prefix as an envelope the XRCE message. The 2-byte prefix shall contain
the length of the XRCE message that follows encoded as little endian."

**Repo:** `crates/xrce/src/transport_tcp.rs` (`TCP_LENGTH_PREFIX_SIZE` l. 37,
`XrceTcpClient::send_message` l. 70, `recv_message` l. 103)

**Tests:** `crates/xrce/src/transport_tcp.rs::tests::tcp_length_prefix_size_constant`,
`crates/xrce/src/transport_tcp.rs::tests::rfc793_byte_stream_is_delimited_by_the_length_prefix_not_by_writes`,
`crates/xrce/src/transport_tcp.rs::tests::rfc793_frame_split_across_two_writes_is_reassembled`

**Status:** done

### §11.3.3 Exactly one message, alignment unchanged

**Spec:** §11.3.3, p. 101 — "After the 2-byte envelope the TCP/IP payload shall
contain exactly one XRCE message. The alignment of the XRCE message shall not be
changed by the added 2-byte prefix. Stated differently the XRCE message shall
consider its first byte to be aligned to an 8-byte (XCDR maximum alignment)
boundary."

**Repo:** `crates/xrce/src/transport_tcp.rs` (`send_message`: `msg.encode()`
before the length)

**Tests:** `crates/xrce/src/transport_tcp.rs::tests::tcp_loopback_three_message_chain`,
`crates/xrce/src/submessages/mod.rs::tests::message_two_submessages_padded_correctly`

**Status:** done — the message is encoded without knowledge of the length.
§8.2.1 names an alignment of 16 bytes for the start of the message, §11.3.3 one
of 8 bytes.

### §11.3.4 Multicast discovery also for TCP

**Spec:** §11.3.4, p. 102 — "XRCE Agent discovery may be done using UDP/IP
multicast even if the communication will be over TCP. The XRCE Agents may be
pre-configured with the multicast address and port number they shall listen
to. By default they shall be the address 239.255.0.2 and the port 7400."

**Repo:** `crates/xrce-agent/src/daemon/server.rs` (responder independent of the
kind of listener); `crates/xrce-agent/src/daemon/discovery.rs`
(`bind_discovery_socket` l. 146)

**Tests:** `crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`,
`crates/xrce/src/discovery.rs::tests::discovery_constants_match_spec`

**Status:** done — the locator of a TCP listener in the answer is dealt with
in the item on §11.3.1.

### §11.3.4 Periodic GET_INFO of the client for TCP

**Spec:** §11.3.4, p. 102 — "To discover Agents via multicast the XRCE Client
shall send the GET_INFO message (see 8.3.5.3) periodically to the configured
multicast address and port. [...] The XRCE Client shall stop sending the
periodic message once it receives a suitable response from an Agent."

**Repo:** `crates/xrce-client/src/discovery.rs` (`AgentDiscovery`,
`discover_over_udp`)

**Tests:** `crates/xrce-client/src/discovery.rs::tests::get_info_goes_to_the_candidates_every_period`,
`crates/xrce-client/src/discovery.rs::tests::a_suitable_answer_ends_the_search`,
`crates/xrce-client/src/discovery.rs::tests::the_search_over_udp_finds_the_agent_that_answers`

**Status:** `done` — Commits `5798b040952`. as in the item on §11.2.4: the client
searches for the agent with `GET_INFO` to the multicast address and then
connects over TCP (item below).

### §11.3.4 Discovery through TCP connections to a list of addresses

**Spec:** §11.3.4, p. 102 — "The XRCE Clients shall be pre-configured with a
list of candidate TCP addresses and ports where XRCE Agents may be located. To
discover Agents via unicast the XRCE Client shall periodically attempt to
establish TCP connections to the configured addresses and ports. One a
connection is established it shall send the CREATE_CLIENT message [...]. The
XRCE Client shall stop making periodic connection attempts once it receives a
suitable response from an Agent."

**Repo:** `crates/xrce-client/src/discovery.rs` (`connect_over_tcp`)

**Tests:** `crates/xrce-client/src/discovery.rs::tests::the_search_over_tcp_connects_to_the_agent_that_accepts`

**Status:** `done` — Commits `5798b040952`. each period the client tries to connect to
every address of the list; on a connection it sends CREATE_CLIENT and stops at a
`STATUS_AGENT` with `STATUS_OK`.

### §11.4 Other transports

**Spec:** §11.4, p. 102 — "The XRCE Protocol is well suited to be mapped to
other transports [...]. it is expected that future revisions of this
specification will provide additional transport mappings. An example Serial
Transport mapping can be found in Annex C.1"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — outlook; the serial line is in the items on
Annex C.

---

## Annex A IDL Types

### Annex A Types equal to §7.7 and §8.3.5

**Spec:** Annex A, p. 103–120 — "module dds { module xrce { [...] }; };" with
ClientKey, ObjectKind, ObjectId, XrceCookie, XrceVersion, XrceVendorId, Time_t,
SessionId, StreamId, RepresentationFormat and the representations of the
objects, CreationMode, RequestId, StatusValue, ResultStatus, InfoMask, the
ActivityInfo types, ObjectInfo, BaseObjectRequest, BaseObjectReply,
DataFormat, DataDeliveryControl, ReadSpecification, SampleInfoFlags,
SampleInfoDelta, SampleData, Sample, SampleDelta, PackedSamples,
DataRepresentation, the payloads of the submessages and SubmessageId.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — mapping: where Annex A has the same form as
§7.7 or §8.3.5, the requirement is in the item there. Deviating and additional
forms are in the following items. Annex A writes
`SESSIONID_NONE_WITH_CLIENT_KEY`, §8.3.2.1 `SESSION_ID_NONE_WITH_CLIENT_KEY`;
Annex A writes `STREAMID_BUILTIN_BEST_EFFORTS`.

### Annex A Types without @extensibility

**Spec:** Annex A, p. 104–112, p. 119 — "struct Time_t { long seconds; unsigned
long nanoseconds; };", "struct TransportLocatorMedium { [...] };", "struct
Property { string name; string value; };", "struct ResultStatus { StatusValue
status; octet implementation_status; };", "struct ACKNACK_Payload { [...] };"
without annotation; DDS-XTypes 1.3 §7.3.1.2.1.8, p. 96: "In the event that the
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

**Status:** `done` — Commits `f2ace917b5a`, `3874ad8d6a6`, `cd542d21c14`. Without
`@extensibility` the type is APPENDABLE (DDS-XTypes 1.3 §7.3.1.2.1.8): `Time_t`,
`ResultStatus`, `Property`, `TransportLocator` and `ACKNACK_Payload` carry a
DHEADER in XCDR2; this is the default. The octets of the eProsima capture
(STATUS 6 bytes payload, ACKNACK 5 bytes, read from
`tools/wire-capture/xrce-sessions/agent-to-client.hex`) are the named setting
FINAL: agent key `unannotated_types: final`, Rust API
`zerodds_xrce::encoding::UnannotatedTypes::Final`; every SDK has the option
"unannotated types" (`b590f6539d0`). The readers accept both forms.

### Annex A OBJK_OTHER = 0x0F

**Spec:** Annex A, p. 103 — "const ObjectKind OBJK_OTHER = 0x0F;"

**Repo:** `crates/xrce/src/object_kind.rs` (`OBJK_OTHER` l. 43,
`ObjectKind::Other`, `ObjectKind::from_u8`)

**Tests:** `crates/xrce/src/object_kind.rs::tests::all_spec_kinds_roundtrip`,
`crates/xrce/src/object_kind.rs::tests::raw_const_values_match_spec`

**Status:** `done` — Commits `f63fa2d1218`. the value 0x0F is named `OBJK_OTHER` as in
Annex A; `OBJK_DOMAIN` no longer exists. The spec gives the Domain no ObjectKind
(item on §7.7.3.4).

### Annex A OBJECTID_AGENT {0xFF, 0xFD}

**Spec:** Annex A, p. 103 — "#define OBJECTID_AGENT {0xFF,0xFD}"

**Repo:** `crates/xrce/src/object_id.rs` (`OBJECTID_AGENT` l. 44)

**Tests:** `crates/xrce/src/object_id.rs::tests::agent_singleton_has_kind_agent`,
`crates/xrce-agent/src/daemon/discovery.rs::tests::discovery_responder_answers_get_info_on_agent`

**Status:** done

### Annex A OBJECTID_SESSION {0xFF, 0xFF}

**Spec:** Annex A, p. 103 — "#define OBJECTID_SESSION {0xFF,0xFF}"

**Repo:** `crates/xrce/src/object_id.rs` (`OBJECTID_SESSION` l. 42,
`OBJECTID_INVALID` l. 40)

**Tests:** `crates/xrce/src/object_id.rs::tests::session_object_id_is_all_ones`,
`crates/xrce/src/object_id.rs::tests::reserved_object_ids_per_spec_7_6_and_annex_a`

**Status:** `done` — Commits `f63fa2d1218`. `OBJECTID_SESSION` is {0xFF, 0xFF};
`OBJECTID_INVALID` is {0x00, 0x00} (§7.6). The spec does not say what
`OBJECTID_SESSION` stands for; the code does not use the constant.

### Annex A XRCE_VENDOR_INVALID

**Spec:** Annex A, p. 104 — "typedef octet XrceVendorId[2]; #define
XRCE_VENDOR_INVALID1 0x00 #define XRCE_VENDOR_INVALID1 0x00"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — constants without a rule. The line appears
twice with the same name; presumably the two octets of an invalid vendor are
meant.

### Annex A TransportLocatorFormat

**Spec:** Annex A, p. 104 — "@bit_bound(8) enum TransportLocatorFormat {
ADDRESS_FORMAT_SMALL, ADDRESS_FORMAT_MEDIUM, ADDRESS_FORMAT_LARGE,
ADDRESS_FORMAT_STRING };" and p. 105 "union TransportLocator switch
(TransportLocatorFormat) { [...] case ADDRESS_FORMAT_LARGE:
TransportLocatorLarge medium_locator; [...] };"

**Repo:** `crates/xrce/src/transport_locator.rs` (`fmt_disc` l. 50,
`TransportLocator::encode_into`, `decode_at`)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag`,
`crates/xrce/src/transport_locator.rs::tests::large_has_three_padding_octets_and_a_32_bit_port`,
`crates/xrce/src/transport_locator.rs::tests::small_is_two_address_octets_and_a_one_octet_port`,
`crates/xrce/src/transport_locator.rs::tests::string_length_is_aligned_to_four`,
`crates/xrce/src/transport_locator.rs::tests::unknown_discriminator_rejected`

**Status:** `done` — Commits `c304e509660`. the four values 0 to 3; the discriminator
is one octet before the member, by default behind the DHEADER of the union (item
on Annex A "Types without @extensibility"). Annex A names the member for LARGE
`medium_locator`; on the wire the name has no effect.

### Annex A Discriminators 0x04 and 0x05

**Spec:** Annex A, p. 104 — as above.

**Repo:** `crates/xrce/src/transport_locator.rs` (`fmt_disc` has only 0x00 to
0x03)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::unknown_discriminator_rejected`

**Status:** `done` — Commits `c304e509660`. `fmt_disc::TCP` and `fmt_disc::SERIAL` and
`TransportLocator::Tcp` and `Serial` are removed; the test refuses the
discriminators 4 and 5. TCP listeners are announced as Medium or Large, the
serial line not at all.

### Annex A TransportLocatorSmall

**Spec:** Annex A, p. 104–105 — "struct TransportLocatorSmall { octet
address[2]; octet locator_port; };"

**Repo:** `crates/xrce/src/transport_locator.rs` (`TransportLocatorSmall`:
`address: [u8; 2]`, `port: u8`)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::small_is_two_address_octets_and_a_one_octet_port`

**Status:** `done` — Commits `c304e509660`. two address octets and one port octet,
without padding.

### Annex A TransportLocatorMedium

**Spec:** Annex A, p. 105 — "struct TransportLocatorMedium { octet address[4];
unsigned short locator_port; };"

**Repo:** `crates/xrce/src/transport_locator.rs` (`TransportLocatorMedium`,
`encode_member`)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag`,
`crates/xrce/src/transport_locator.rs::tests::a_udp_locator_carries_the_default_agent_port`

**Status:** `done` — Commits `c304e509660`. as in the item on §11.2.1: one padding
octet after the address, the port as a number in the byte order of the E flag.

### Annex A TransportLocatorLarge

**Spec:** Annex A, p. 105 — "struct TransportLocatorLarge { octet address[16];
unsigned long locator_port; };"

**Repo:** `crates/xrce/src/transport_locator.rs` (`TransportLocatorLarge`,
`TransportLocator::encode_into`, `TransportLocator::decode_at`)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::large_has_three_padding_octets_and_a_32_bit_port`,
`crates/xrce/src/transport_locator.rs::tests::a_locator_sequence_is_a_count_and_aligned_locators`

**Status:** `done` — Commits `c304e509660`, `f2ace917b5a`. 16 address octets, three
padding bytes, the port as an `unsigned long` in the byte order of the E flag.
The union and the struct of its case carry a DHEADER each when they have no
`@extensibility` (APPENDABLE per DDS-XTypes 1.3 §7.3.1.2.1.8); the agent key
`unannotated_types: final` writes them without. The test of the padding bytes
runs in the `Final` setting, the test of the sequence in the default.

### Annex A TransportLocatorString

**Spec:** Annex A, p. 105 — "struct TransportLocatorString { string value; };"

**Repo:** `crates/xrce/src/transport_locator.rs` (`TransportLocator::String`,
`TRANSPORT_LOCATOR_STRING_MAX`, `TransportLocator::encode_into`)

**Tests:** `crates/xrce/src/transport_locator.rs::tests::string_length_is_aligned_to_four`,
`crates/xrce/src/transport_locator.rs::tests::a_long_string_locator_is_not_cut_at_256`,
`crates/xrce/src/transport_locator.rs::tests::a_string_without_its_nul_is_refused`

**Status:** `done` — Commits `c304e509660`. the length is an `unsigned long`, aligned
to 4 and in the byte order of the E flag; the characters and a NUL follow. The
upper limit is 65 535 bytes (the size of a submessage), no longer 256.

### Annex A Property and PropertySeq

**Spec:** Annex A, p. 105 — "struct Property { string name; string value; };
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

**Status:** `done` — Commits `65cd1820a04`, `f2ace917b5a`, `b43d5e6b848`. the number of
elements and the lengths of the strings are `unsigned long`s, aligned to 4 from
the start of the payload; in `CLIENT_Representation` the number lies at offset
16. A `Property` without `@extensibility` carries a DHEADER (default);
`unannotated_types: final` writes it without; both forms are read. The byte
order follows the E flag. A sequence that cannot be read is an error and does
not count as absent; the agent answers it with `STATUS_ERR_INVALID_DATA`. Upper
limit on reading: 64 elements.

### Annex A CLIENT_Representation

**Spec:** Annex A, p. 105–106 — "@extensibility(FINAL) struct
CLIENT_Representation { XrceCookie xrce_cookie; XrceVersion xrce_version;
XrceVendorId xrce_vendor_id; ClientKey client_key; SessionId session_id;
@optional PropertySeq properties; };"

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation`, `encode_with`,
`decode_with`, `encode_into`, `decode_at`)

**Tests:** `crates/xrce/src/handshake.rs::tests::client_representation_decodes_the_captured_create_client_prefix`,
`crates/xrce/src/handshake.rs::tests::client_representation_roundtrip_with_properties`,
`crates/xrce/src/handshake.rs::tests::by_default_the_representations_have_no_timestamp`,
`crates/xrce/src/handshake.rs::tests::the_prose_form_has_a_time_t_after_the_vendor_id`

**Status:** `done` — Commits `65cd1820a04`, `b43d5e6b848`, `711f93fd460`. the code
follows the form of Annex A; `@optional` is an octet 0 or 1, `properties` lies
as in the item above. The text of §7.8.2.1 also names a `Time_t
client_timestamp` after `xrce_vendor_id`; the IDL and the bytes of Annex B.1
do not have it. The form of Annex A is the default; the form of the text is the
named setting `RepresentationTimestamps::Present` (agent key
`representation_timestamps: present`, `XrceClient::set_representation_timestamps`,
option `representation timestamps` in every SDK). The two octets after
`properties` (`requested_mtu`) are an extension of eProsima, aligned to 2.

### Annex A AGENT_Representation

**Spec:** Annex A, p. 106 — "@extensibility(FINAL) struct AGENT_Representation
{ XrceCookie xrce_cookie; XrceVersion xrce_version; XrceVendorId
xrce_vendor_id; @optional PropertySeq properties; };"

**Repo:** `crates/xrce/src/handshake.rs` (`AgentRepresentation`, `encode_with`,
`decode_with`, `encode_into`, `decode_at`)

**Tests:** `crates/xrce/src/handshake.rs::tests::agent_representation_decodes_the_captured_status_agent_body`,
`crates/xrce/src/handshake.rs::tests::agent_representation_roundtrip_no_properties`,
`crates/xrce/src/handshake.rs::tests::the_agent_representation_follows_the_setting_the_same_way`

**Status:** `done` — Commits `65cd1820a04`, `b43d5e6b848`, `711f93fd460`. the form with
`agent_timestamp` (§7.8.2.1) is the same named setting as for
`CLIENT_Representation` (`representation_timestamps`, default `absent` per
Annex A).

### Annex A OBJK_DomainParticipant_Binary

**Spec:** Annex A, p. 109 — "@extensibility(APPENDABLE) struct
OBJK_DomainParticipant_Binary { @optional string<128> domain_reference;
@optional string<128> qos_profile_reference; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`participant`,
`ParticipantBinary`); `crates/xrce-agent/src/daemon/entity_repr.rs`
(`participant`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::participant_binary_in_both_forms`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_participant_is_read`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_domain_reference_is_resolved_against_the_domain_libraries`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_participant_qos_profile_reference_is_applied`

**Status:** `done` — Commits `c67ce23c724`, `b4cd825c268`. the blob is read as XCDR2:
Annex A (DHEADER, two `@optional` references), then the form of §7.7.3.6.3
(`long domain_id` first), then the two references without a DHEADER that the
eProsima client sends. `domain_reference` is resolved against the Domains of the
`<domain_library>` files (`STATUS_ERR_UNKNOWN_REFERENCE` for a name in no
library; the `domain_id` of the Domain must agree with that of the
representation). `qos_profile_reference` applies the participant QoS of the
profile (USER_DATA, ENTITY_FACTORY; any other policy gives
`STATUS_ERR_INCOMPATIBLE`).

### Annex A OBJK_Topic_Binary as APPENDABLE

**Spec:** Annex A, p. 109 — "@extensibility(APPENDABLE) struct
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

**Status:** `done` — Commits `c67ce23c724`. both the form of Annex A (a DHEADER in front
of the members) and the FINAL form of §7.7.3.7.3 (no DHEADER, the 36 octets of
the example in §7.7.3.1.3) are read; §7.7.3.7.3 gives no extensibility.
`string<256>` is checked. A `type_identifier` takes precedence over
`type_reference`; if both are missing the agent answers
`STATUS_ERR_INVALID_DATA`.

### Annex A OBJK_Publisher_Binary as APPENDABLE

**Spec:** Annex A, p. 109 — "@extensibility(FINAL) struct
OBJK_Publisher_Binary_Qos { @optional sequence<string> partitions; @optional
sequence<octet> group_data; }; @extensibility(APPENDABLE) struct
OBJK_Publisher_Binary { @optional string publisher_name; @optional
OBJK_Publisher_Binary_Qos qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`container`,
`ContainerBinary`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`container`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::publisher_binary_of_annex_a`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_publisher_gives_its_partitions`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_publisher_has_default_qos`

**Status:** `done` — Commits `c67ce23c724`. the form of Annex A (DHEADER,
`publisher_name`, `qos` with `partitions` and `group_data`) is read; if that
fails, `container` reads the form of §7.7.3.8.2 without a DHEADER, which the
eProsima client sends. §7.7.3.8.2 names the QoS structure
`OBJK_PUBLISHER_QosBinary`; this has no effect on the wire.

### Annex A OBJK_Subscriber_Binary as APPENDABLE

**Spec:** Annex A, p. 109 — "@extensibility(APPENDABLE) struct
OBJK_Subscriber_Binary { @optional string subscriber_name; @optional
OBJK_Subscriber_Binary_Qos qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`container`,
`ContainerBinary`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`container`
with `element` = `subscriber`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::publisher_binary_of_annex_a`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_publisher_gives_its_partitions`

**Status:** `done` — Commits `c67ce23c724`. the same function and the same layout as
for the publisher; the tests read the blob of the publisher, a test with a
blob of its own for the subscriber is missing.

### Annex A EndpointQosFlags with is_history_keep_all

**Spec:** Annex A, p. 110 — "@bit_bound(16) bitmask EndpointQosFlags {
@position(0) is_reliable, @position(1) is_history_keep_all, [...] };"

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_endpoint_binary`,
`spec_flags`, `flags`); `crates/xrce-agent/src/daemon/binary_forms.rs`
(`EndpointBinary::bit1_is_keep_all`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::reader_binary_of_the_clause`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_writer_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_clause_binary_reader_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_writer_is_reliable_keep_last_transient_local`

**Status:** `done` — Commits `c67ce23c724`. Annex A (bit 1 = `is_history_keep_all`)
and §7.7.3.10.2 (bit 1 = `is_history_keep_last`) contradict each other. The bit
is read according to the form in which the blob is readable: the form of
Annex A first, then the form of the clause; the classic CDR structure of
eProsima (`qos_flags` `0x000b` in the capture) reads bit 1 as KEEP_LAST. Bits
outside `EndpointQosFlags` and more than one durability bit give
`STATUS_ERR_INVALID_DATA`.

### Annex A OBJK_DataWriter_Binary_Qos
**Spec:** Annex A, p. 110 — "@extensibility(FINAL) struct
OBJK_DataWriter_Binary_Qos : OBJK_Endpoint_Binary_Qos { @optional unsigned long
ownership_strength; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint_qos`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_ownership_strength`, `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`

**Status:** done — the XCDR2 forms read `ownership_strength` as an `unsigned long`; the classic CDR structure of eProsima reads 64 bits. The test reads a present `ownership_strength` in an XCDR2 blob in both byte orders (2d6689d3ed6).
### Annex A OBJK_DataReader_Binary_Qos

**Spec:** Annex A, p. 110 — "@extensibility(FINAL) struct
OBJK_DataReader_Binary_Qos : OBJK_Endpoint_Binary_Qos { @optional unsigned long
timebasedfilter_msec; @optional string contentbased_filter; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint_qos`,
`EndpointQosBinary::timebasedfilter_msec`, `contentbased_filter`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`apply_endpoint_binary`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::reader_binary_of_the_clause`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_clause_binary_reader_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`

**Status:** `done` — Commits `c67ce23c724`, `33df0df0945`. `timebasedfilter_msec` is read
as an `unsigned long` (value 50 in the test), `contentbased_filter` as a
`string` ("x>100" in the example B.2.7). The classic CDR structure of eProsima
remains as the fallback.

### Annex A OBJK_DataWriter_Binary

**Spec:** Annex A, p. 110–111 — "@extensibility(APPENDABLE) struct
OBJK_DataWriter_Binary { string topic_name; @optonal OBJK_DataWriter_Binary_Qos
qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint`,
`EndpointBinary`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::an_annex_a_binary_writer_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_writer_is_reliable_keep_last_transient_local`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_blob_of_no_form_is_refused_naming_each_reading`

**Status:** `done` — Commits `c67ce23c724`, `33df0df0945`. three forms are read in this
order: Annex A (DHEADER, `topic_name`, `@optional qos`), the form of §7.7.3.10.2
(no DHEADER, `endpoint_qos` as a mandatory member) and the classic CDR structure
of eProsima (`ObjectId` of the topic instead of `topic_name`). A blob in no form
is refused; the message names the result of each form.

### Annex A OBJK_DataReader_Binary

**Spec:** Annex A, p. 110 — "@extensibility(APPENDABLE) struct
OBJK_DataReader_Binary { string topic_name; @optional
OBJK_DataReader_Binary_Qos qos; };"

**Repo:** `crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint` with
`writer = false`); `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`)

**Tests:** `crates/xrce-agent/src/daemon/binary_forms.rs::tests::reader_binary_of_the_clause`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_clause_binary_reader_names_its_topic_and_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_captured_binary_reader_is_reliable_keep_last_transient_local`

**Status:** `done` — Commits `c67ce23c724`, `33df0df0945`. the same three forms as for
the DataWriter; Annex A with a filter is read through B.2.7, the form of the
clause through the test with depth 5 and `timebasedfilter_msec` 50.

### Annex A ObjectVariant with OBJK_AGENT and OBJK_CLIENT

**Spec:** Annex A, p. 111 — "@extensibility(FINAL) union ObjectVariant
switch(ObjectKind) { // case OBJK_INVALID : indicates default or selected by
Agent. No data. case OBJK_AGENT : AGENT_Representation client; case
OBJK_CLIENT : CLIENT_Representation client; [...] };"

**Repo:** `crates/xrce/src/object_info.rs` (`ObjectConfig::Agent`,
`ObjectConfig::Client`, `ObjectInfo::config`); `crates/xrce-agent/src/daemon/session.rs`
(`agent_object_info`, `client_object_info`)

**Tests:** `crates/xrce/src/object_info.rs::tests::the_config_of_the_agent_is_its_agent_representation`,
`crates/xrce/src/submessages/get_info.rs::tests::client_info_follows_the_info_mask`,
`crates/xrce-agent/src/daemon/session.rs::tests::get_info_of_the_agent_checks_the_version_in_client_info`

**Status:** `done` — Commits `65cd1820a04`. the cases `OBJK_AGENT` and `OBJK_CLIENT`
are in `ObjectConfig`, the `config` of `ObjectInfo` (INFO) and the `client_info`
of GET_INFO; the kind octet and the representation follow each other. The
`ObjectVariant` enum for CREATE does not have these two cases: a representation
of kind Agent or Client is not read there. Both members are called `client` in
the IDL; this has no effect on the wire.

### Annex A ActivityInfoVariant with case OBJECTID_AGENT

**Spec:** Annex A, p. 113 — "@extensibility(FINAL) union ActivityInfoVariant
switch (ObjectKind) { case OBJECTID_AGENT : AGENT_ActivityInfo agent; case
OBJK_DATAWRITER : [...] case OBJK_DATAREADER : [...] };"

**Repo:** `crates/xrce/src/object_info.rs` (`ActivityInfoVariant`,
`ActivityInfoVariant::discriminator`: Agent 0x0D, DataWriter 0x05, DataReader
0x06)

**Tests:** `crates/xrce/src/object_info.rs::tests::agent_activity_info_has_a_dheader_and_a_locator_sequence`,
`crates/xrce/src/object_info.rs::tests::data_reader_activity_info_is_a_dheader_and_a_short`,
`crates/xrce/src/object_info.rs::tests::data_writer_activity_info_is_a_dheader_a_long_long_and_a_short`

**Status:** `done` — Commits `65cd1820a04`. the discriminator is an `ObjectKind` (one
octet); `OBJECTID_AGENT` is an `ObjectId` of two octets and no value of
`ObjectKind`, so the text of Annex A cannot be taken literally. The case of the
agent has the discriminator `OBJK_AGENT` (0x0D). The members carry the DHEADER
(`AGENT_ActivityInfo`: `availability`, `address_seq` as a
`TransportLocatorSeq`).

### Annex A FORMAT_MASK = 0x0E

**Spec:** Annex A, p. 114 — "const DataFormat FORMAT_MASK = 0x0E; // 0b0000
1110"

**Repo:** `crates/xrce/src/submessages/write_data.rs`
(`DataFormat::FORMAT_MASK`, `DataFormat::from_flag_bits`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_rejects_reserved_format`,
`crates/xrce/src/submessages/write_data.rs::tests::data_format_raw_roundtrip`,
`crates/xrce/src/submessages/read_data.rs::tests::the_data_format_octet_has_the_values_of_annex_a`

**Status:** `done`

### Annex A SampleInfo with SampleInfoDetail
**Spec:** Annex A, p. 114–115 — "typedef octet SampleInfoFormat; const
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

**Status:** done — `SampleInfo` is the `state` with the `SampleInfoDetail` union of Annex A: `FORMAT_EMPTY`, `FORMAT_SEQNUM`, `FORMAT_TIMESTAMP`, `FORMAT_SEQN_TIMS`. The union gives the case `FORMAT_TIMESTAMP` twice and lets `FORMAT_EMPTY` fall through to `FORMAT_SEQNUM`; the intended reading is used (nothing, sequence number, offset, both). `FORMAT_SEQN_TIMS` is the 12 octets of the flat struct of §7.7.1 and of the figures of Annex B.5.3 to B.5.5, the discriminator in the first padding octet. The default is `SampleInfoForm::AnnexA`; the flat form (`sample_info: flat`) is for a client that sends zeros as padding (the union reads them as `FORMAT_EMPTY`). 8be8c6101ca.
### Annex A ACKNACK_Payload with unsigned short

**Spec:** Annex A, p. 119 — "struct ACKNACK_Payload { unsigned short
first_unacked_seq_num; octet nack_bitmap[2]; octet stream_id; };"

**Repo:** `crates/xrce/src/submessages/acknack.rs` (`AckNackPayload`,
`encode_body`, `decode_body`)

**Tests:** `crates/xrce/src/submessages/acknack.rs::tests::acknack_le_roundtrip`,
`crates/xrce/src/submessages/acknack.rs::tests::acknack_be_roundtrip`,
`crates/xrce/tests/unannotated_types.rs::the_default_is_appendable_with_a_dheader`,
`crates/xrce/tests/unannotated_types.rs::the_final_setting_writes_the_reference_implementations_octets`,
`crates/xrce/src/reliable.rs::tests::acknack_retransmit_of_lost_sample_across_wrap`

**Status:** `done` — Commits `f2ace917b5a`. §8.3.5.11.2 writes `short`; the bytes of the
field are the same. In Annex A the structure carries no `@extensibility`: it is
APPENDABLE and has a DHEADER in the default (9 octets); the 5 octets of the
reference agent are written by `unannotated_types: final`, both forms are read.
The code holds the field as `i16` and calculates with `SerialNumber16`.

### Annex A HEARTBEAT_Payload with unsigned short

**Spec:** Annex A, p. 119 — "@extensibility(FINAL) struct HEARTBEAT_Payload {
unsigned short first_unacked_seq_num; unsigned short last_unacked_seq_num; octet
stream_id; };"

**Repo:** `crates/xrce/src/submessages/heartbeat.rs` (`HeartbeatPayload`,
`encode_body`, `decode_body`)

**Tests:** `crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_le_roundtrip`,
`crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_be_roundtrip`,
`crates/xrce/src/reliable.rs::tests::heartbeat_window_across_wrap_reports_rfc1982_order`

**Status:** `done` — the structure is explicitly `@extensibility(FINAL)`, the
body has 5 octets without a DHEADER. §8.3.5.12.2 writes `short first_unacked_seq_nr`
and `short last_unacked_seq_nr`; the bytes are the same.

---

## Annex B Example Messages (Non-Normative)

### Annex B.1 CREATE_CLIENT

**Spec:** Annex B.1, p. 121–122 — CREATE_CLIENT of vendor_id {0x0F, 0x0F},
client_key {0x22, 0x33, 0x44, 0x55}, session_id 0xDD; header {0x80, 0x00,
0x00, 0x00}, bytes 8–21 `CLIENT_Representation`, byte 21 "properties? =
FALSE".

**Repo:** `crates/xrce/src/handshake.rs` (`ClientRepresentation::encode`);
`crates/xrce/src/submessages/create_client.rs` (`CreateClientPayload`)

**Tests:** `crates/xrce/src/submessages/create_client.rs::tests::annex_b_1_create_client_example_bytes`,
`crates/xrce/src/handshake.rs::tests::the_prose_form_has_a_time_t_after_the_vendor_id`

**Status:** `done` — Commits `d4d590690f6`, `711f93fd460`. the test reproduces the header
and bytes 8–21. The text names `request_id` and `client_timestamp`, which the
layout does not contain; with the setting `representation_timestamps: present`
the code writes the `Time_t` of the text, and the second test checks that form
with the values of the example. CREATE_CLIENT has no flags 0x07 ("reuse,
replace"); `submessageLength` reads "26 = 0x001B", the body has 14 bytes.

### Annex B.2.1 DomainParticipant with REPRESENTATION_BY_REFERENCE

**Spec:** Annex B.2.1, p. 123–125 — CREATE with `OBJK_PARTICIPANT`,
`REPRESENTATION_BY_REFERENCE`, reference "MyLibrary::MyParticipant" (25 bytes
with NUL), `domain_id` 0.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::encode_for_kind`,
`ObjectVariant::decode_for_kind_at`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::participant_representation_matches_the_captured_create_bytes`

**Status:** `n/a (informative)` — example. The test checks the form from the
capture of eProsima: length at bytes 16–19, characters from byte 20. The example
puts the length at bytes 16–19 and the characters at bytes 24–48; bytes 20–23
are not explained, the code does not read the bytes of the example.
`submessageLength` reads 26, the payload has 44 bytes. The text refers to
Table 20 instead of Table 18.

### Annex B.2.2 DomainParticipant with REPRESENTATION_IN_BINARY

**Spec:** Annex B.2.2, p. 125–127 — "Bytes 16-19 DHEADER of
OBJK_DomainParticipant_Binary (because extensibility is APPENDABLE) Encodes the
endianness and length [...] 0x80000002 = {0x02, 0x00, 0x00, 0x80}", then two
octets for the `@optional` members and `domain_id`.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`);
`crates/xrce-agent/src/daemon/binary_forms.rs` (`participant`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::annex_b_2_2_participant_variant`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::a_spec_binary_participant_is_read`

**Status:** `done` — Commits `33df0df0945`, `c67ce23c724`. the DHEADER `0x80000002` stands
where eProsima sends the length of `binary_representation`; the code reads it
there as a DHEADER (bit 31 is no part of the length), followed by the two
`@optional` octets and the `domain_id`.

### Annex B.2.3 DataWriter with REPRESENTATION_IN_BINARY

**Spec:** Annex B.2.3, p. 127–129 — DHEADER 0x8000000C, `topic_name` "Square"
(7 bytes with NUL), `qos?` = 0, `publisher_id` {0xBB, 0xB3}.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`);
`crates/xrce-agent/src/daemon/binary_forms.rs` (`endpoint`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::annex_b_2_3_writer_variant_reads_as_the_blob_and_its_publisher`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`

**Status:** `done` — Commits `c67ce23c724`. the blob with `topic_name` and `qos?` = 0 and
the `publisher_id` after it are read. The text calls the publisher
`subscriber_id`.

### Annex B.2.4 DataWriter with QoS

**Spec:** Annex B.2.4, p. 129–132 — "RELIABILITY policy set to BEST_EFFORT,
HISTORY policy set to KEEP_ALL and DEADLINE policy set to a period of 2
minutes"; "qos_flags = 0x0003 Indicates the flags for is_reliable and
is_history_keep_all are both set."; byte 34 `history_depth?`, byte 35
padding, bytes 36–39 `deadline` = 120000.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::annex_b_2_4_writer_with_qos_in_both_readings_of_the_padding`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`,
`crates/xrce-agent/src/daemon/binary_forms.rs::tests::writer_binary_of_annex_a_with_qos`

**Status:** `done` — Commits `33df0df0945`. read as XCDR2 of the printed IDL. Byte 35,
which the table calls padding, is the octet for `@optional deadline_msec` (1);
the value starts at byte 36. The padding byte 43 before `publisher_id` is read
both counted in the DHEADER (24) and outside it (23). The text names
BEST_EFFORT, `qos_flags` 0x0003 and the table set `is_reliable`; the bytes are
read.

### Annex B.2.5 DataWriter with REPRESENTATION_AS_XML_STRING

**Spec:** Annex B.2.5, p. 132–134 — `<data_writer name="MyWriter"
topic_ref="Square"><data_writer_qos base_name="MyQosLib::MyProfile">
<deadline>[...]</deadline></data_writer_qos></data_writer>`, length 181,
`publisher_id` at bytes 201–202.

**Repo:** `crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint_from_xml`:
`topic_ref` as a name, `<data_writer_qos>` and `<datawriter_qos>` with
`base_name`)

**Tests:** `crates/xrce-agent/src/daemon/session.rs::tests::a_data_writer_states_its_qos_over_a_profile`

**Status:** `n/a (informative)` — example. The code resolves `topic_ref="Square"`
as a name and reads `<data_writer_qos base_name="…">` over a profile of the
agent (`3aea52dd70a`); the test has the form of the example, not its bytes. In
the figure the discriminator is 0x01, in Table 22 0x02; the text names 180 and
181 characters and "MyQosLib:MyProfile" with one colon.

### Annex B.2.6 DataReader with REPRESENTATION_IN_BINARY

**Spec:** Annex B.2.6, p. 134–136 — DataReader "Square" with DHEADER,
`topic_name`, `qos?` = 0, `subscriber_id` {0xCC, 0xC4}.

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::annex_b_2_6_reader_with_default_qos`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`

**Status:** `done` — Commits `33df0df0945`. the DHEADER 12 (`0x8000000C`, as in B.2.3) is
read. The text prints the DHEADER as {0xB5, 0x00, 0x00, 0x00}; that does not fit
the 26 octets of the message, the code refuses the printed value. Table 23 sets
the discriminator to 0x05 (`OBJK_DATAWRITER`) for `OBJK_DATAREADER` (0x06) and
lists bytes 24–30 twice.

### Annex B.2.7 DataReader with QoS and filter

**Spec:** Annex B.2.7, p. 136–139 — "HISTORY policy set to KE EP_ALL and
DEADLINE policy set to a period of 5 minutes", `qos_flags` 0x0002, `deadline`
180000, `contentbased_filter` "x>100".

**Repo:** `crates/xrce/src/object_repr.rs` (`ObjectVariant::decode_for_kind_at`);
`crates/xrce-agent/src/daemon/entity_repr.rs` (`endpoint`)

**Tests:** `crates/xrce/src/object_repr.rs::tests::annex_b_2_7_reader_with_qos_and_filter`,
`crates/xrce-agent/src/daemon/entity_repr.rs::tests::the_annex_b_binary_examples_read_as_their_qos`

**Status:** `done` — Commits `33df0df0945`. read as XCDR2 of the printed IDL; byte 35 is
the octet for `deadline_msec`, no padding byte stands before the
`subscriber_id`. 180000 ms are 3 minutes (read), the text names 5; the figure and
Table 24 name `publisher_id` instead of `subscriber_id` at the end.

### Annex B.3.1 WRITE_DATA with one sample

**Spec:** Annex B.3.1, p. 139–141 — flags 0x01, `submessageLength` 6, bytes 8–13
{0xAA, 0x01, 0x44, 0x05, 0x19, 0x00}.

**Repo:** `crates/xrce/src/submessages/write_data.rs` (`WriteDataPayload`)

**Tests:** `crates/xrce/src/submessages/write_data.rs::tests::write_data_matches_the_annex_b_example`

**Status:** `done` — the test reproduces flags, length and body byte for byte.
Table 25 is called "READ_DATA (single sample)".

### Annex B.3.2 WRITE_DATA with a sequence without SampleInfo
**Spec:** Annex B.3.2, p. 141–143 — flags 0x09, `data_seq.length` = 4, four
`short` {0x14, 0x00}, {0x11, 0x00}, {0x1A, 0x00}, {0x24, 0x00}.

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_3_2_write_data_with_a_sequence_without_sample_info`

**Status:** done — the example is tested with its bytes: id 0x07, flags 0x09, length 16 (the figure prints the length 6 of B.3.1), four values 20, 17, 26, 36 (8be8c6101ca).
### Annex B.3.3 WRITE_DATA with timestamp

**Spec:** Annex B.3.3, p. 143–146 — "Byte 12 info.state = 0x00 [...] Byte 13
info.detail.discriminator = FORMAT_TIMESTAMP Bytes 14-15 padding Bytes 16-19
info. Detail.session_time_offset Bytes 20-21 serialized_data".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example in the form of Annex A with
`SampleInfoDetail`. The code reads `SampleInfo` per §7.7.1 with 12 bytes and
does not read this example (open item "Annex A SampleInfo with
SampleInfoDetail"). `submessageLength` reads "13 = 0x000D" with {0x06, 0x00},
the body has 14 bytes.

### Annex B.3.4 WRITE_DATA with a disposed sample

**Spec:** Annex B.3.4, p. 146–148 — "Byte 12 info.state = 0x02 [...] Byte 13
Info.detail.discriminator = FORMAT_NONE [...] Bytes 14-19 Serialized_data".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example in the form of Annex A; the code does
not read it (as for B.3.3). The figure and the table name FORMAT_EMPTY and
FORMAT_NONE; `submessageLength` reads 6, the body has 12 bytes.

### Annex B.4.1 READ_DATA for one sample
**Spec:** Annex B.4.1, p. 148–150 — "submessageId = READ_DATA = 0x07", flags
0x01, "submessageLength = 7", byte 12 `preferred_stream_id` 0x00, byte 13
`data_format` 0x00, bytes 14 and 15 the octets for `@optional` = 0.

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_4_1_read_data_for_a_sample`

**Status:** done — the example is tested with all eight octets of the payload; id 0x08 (the figure prints 0x07), length 8 (the figure prints 7) (8be8c6101ca).
### Annex B.4.2 READ_DATA with filter and DataDeliveryControl

**Spec:** Annex B.4.2, p. 150–153 — `data_format` 0x08, `content_filter`
"x>100" with length 6, DHEADER 0x80000008, `max_samples` 100,
`max_elapsed_time` 30000, `max_rate` 1024 "{0x00, 0x40}", `min_pace_period`
1000.

**Repo:** `crates/xrce/src/submessages/read_data.rs` (`ReadDataPayload`,
`ReadSpecification`, `DataDeliveryControl`)

**Tests:** `crates/xrce/src/submessages/read_data.rs::tests::annex_b_4_2_read_data_with_filter_and_delivery_control`,
`crates/xrce/src/submessages/read_data.rs::tests::the_content_filter_is_a_string_with_a_nul_and_an_aligned_length`

**Status:** `done` — Commits `1663f8f6240`. `data_format` 0x08 (FORMAT_DATA_SEQ),
`content_filter_expression` as an XCDR2 string, `DataDeliveryControl` with a
DHEADER; the test reads the bytes of the figure (bit 31 of the DHEADER is no
part of the length) and writes the plain DHEADER 8. Table 30 puts the DHEADER
and `max_samples` both at bytes 28–29; for 1024 the text prints {0x00, 0x40},
little-endian it is {0x00, 0x04}; byte 14 and byte 26 read "set to FALSE
indicating there it is present". The form without a DHEADER is written by
`XrceClient::set_delivery_control_form(DeliveryControlForm::Bare)`.

### Annex B.5.1 DATA with one sample
**Spec:** Annex B.5.1, p. 154–155 — flags 0x00 (big-endian),
`submessageLength` 8, `FooType.count` = 19 as {0x00, 0x00, 0x00, 0x13}.

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_5_1_data_with_one_sample`

**Status:** done — the example (big-endian, flags 0x00) is decoded with its bytes and re-encoded (8be8c6101ca).
### Annex B.5.2 DATA with a sequence without SampleInfo
**Spec:** Annex B.5.2, p. 155–157 — `submessageLength` 16, `data_seq.length` = 2,
two `long`.

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_5_2_data_with_a_sequence_without_sample_info`

**Status:** done — the example is tested: id 0x09 (the figure prints 0x08), flags 0x08 (E flag clear, FORMAT_DATA_SEQ; the figure prints 0x00), `sequenceNr` {0x0A,0x00} = 10 (the text says 1), `request_id` {AA,01} by the table (the text says {AA,02}) (8be8c6101ca).
### Annex B.5.3 DATA with one sample and SampleInfo

**Spec:** Annex B.5.3, p. 157–159 — IDL with `SampleInfoDetail`; figure: "info.state
| info.sequence_number | info.session_time_offset | XCDR_Serialization(foo)".

**Repo:** `crates/xrce/src/submessages/data.rs` (`DataPayloadSample`);
`crates/xrce/src/sample_info.rs` (`SampleInfo`)

**Tests:** `crates/xrce/src/submessages/data.rs::tests::data_sample_matches_annex_b_5_3_layout`,
`crates/xrce/src/sample_info.rs::tests::sample_info_matches_annex_b_5_3_layout`,
`crates/xrce-agent/src/daemon/session.rs::tests::a_read_for_sample_is_answered_with_a_sample_each`

**Status:** `done` — the tests reproduce the figure of the example. The figure
has the form of §7.7.1, the IDL above it that of Annex A (open item "Annex A
SampleInfo with SampleInfoDetail").

### Annex B.5.4 DATA with a sequence with SampleInfo
**Spec:** Annex B.5.4, p. 159–161 — `submessageLength` 40, `sample_seq.length`
= 2, per element `info` (12 bytes) and `data` (4 bytes).

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_5_4_data_with_a_sequence_with_sample_info`

**Status:** done — the example is tested with the 12 octets of the SampleInfo per element (length 40; flags 0x0A instead of 0x00; `SampleInfoForm::Flat`) (8be8c6101ca).
### Annex B.5.5 DATA with PackedSamples
**Spec:** Annex B.5.5, p. 161–163 — `submessageLength` 36, `info_base` (12
bytes), `sample_delta_seq.length` = 2, per element `info_delta` (4 bytes) and
`data` (4 bytes).

**Repo:** `crates/xrce/src/submessages/write_data.rs`; `crates/xrce/src/submessages/data.rs`; `crates/xrce/src/submessages/read_data.rs`; `crates/xrce/src/sample_info.rs`

**Tests:** `crates/xrce/tests/annex_b_examples.rs::b_5_5_data_with_packed_samples`

**Status:** done — the example is tested after the figure: `request_id`/`object_id`, `info_base` (12), count, two `SampleInfoDelta` with data, 36 octets; Table 34 lists `info_base` at bytes 8 to 19 without `request_id` and `object_id`, which fits neither (8be8c6101ca).

---

## Annex C Additional Transport Mappings

### Annex C.1 Framing according to RFC 1662 for serial lines

**Spec:** Annex C.1, p. 165 — "This section normalizes the communication over
serial protocols such as SPI, I2C or RS-232. For this propose, the XRCE
Messages are framed using an HDLC-like framing similar to [IETF RFC-1662].
This framing is designed to meet all the functionality listen in clause 11.1,
except that it does not provide security."

**Repo:** `crates/xrce/src/transport_serial.rs` (`encode_frame_with`, `SerialFramer`);
`crates/xrce-agent/src/daemon/transport.rs` (`SerialLine::take_frame`, `serial::run_serial_listener`,
feature `serial`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::the_framer_takes_frames_split_and_back_to_back_and_skips_noise`,
`crates/xrce-agent/src/daemon/transport.rs::tests::the_clients_of_a_serial_line_are_told_apart_by_their_address`

**Status:** `done` — Commits `cf109e295c0`. frames after C.1.1 to C.1.3 in
`zerodds-xrce`, the agent and the endpoint SDKs (C, C++, Java, Python,
Ada-native). Micro-XRCE-DDS 3.0.1 frames the same way but computes
`frameCheck` as CRC-16/ARC over the payload alone (`StreamFramingProtocol.cpp`,
`stream_framing_protocol.c`, read from the source); the agent takes and makes
those frames with `frame_check: crc16-arc-payload`
(`FrameCheck::Crc16ArcPayload`). Not measured against a Micro-XRCE-DDS client
over a line.

### Annex C.1.1 Structure of the frame

**Spec:** Annex C.1.1, p. 165 — "flag | sourceAddress | remoteAddress |
payloadLength | payload (up to 64KB) | frameCheck", one octet each for flag
and the two addresses, 16 bits for `payloadLength` and `frameCheck`.

**Repo:** `crates/xrce/src/transport_serial.rs` (`encode_frame_with`, `FrameAssembler::push`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`

**Status:** done — `7E | sourceAddress | remoteAddress | payloadLength |
payload | frameCheck`; the frame ends after `payloadLength` octets of payload
and two octets of `frameCheck`.

### Annex C.1.1.1 BEGIN_FLAG 0x7E

**Spec:** Annex C.1.1.1, p. 165 — "Each frame begins with a BEGIN_FLAG byte
which has a hexadecimal value of 0x7E."

**Repo:** `crates/xrce/src/transport_serial.rs` (`FLAG_BYTE`, `encode_frame_with`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`

**Status:** done

### Annex C.1.1.2 sourceAddress

**Spec:** Annex C.1.1.2, p. 165 — "The sourceAddress indicates the address of
the frame sender."

**Repo:** `crates/xrce/src/transport_serial.rs` (`SerialFrame::source`); `crates/xrce-agent/src/daemon/transport.rs` (`PeerTable::resolve_on_line`,
`bind_line`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::the_clients_of_a_serial_line_are_told_apart_by_their_address`

**Status:** done — the agent sends with its address (`address:`, default 0,
the address the Micro-XRCE-DDS examples send to). A frame belongs to the
client its header or CREATE_CLIENT names, else to the one registered from its
`sourceAddress`.

### Annex C.1.1.3 remoteAddress

**Spec:** Annex C.1.1.3, p. 165 — "The remoteAddress indicates the address of
the frame recipient."

**Repo:** `crates/xrce/src/transport_serial.rs` (`SerialFrame::remote`); `crates/xrce-agent/src/daemon/transport.rs` (`SerialLine::take_frame`,
`Route::Serial`)

**Tests:** `crates/xrce-agent/src/daemon/transport.rs::tests::the_clients_of_a_serial_line_are_told_apart_by_their_address`

**Status:** done — the agent takes only frames to its address; its messages to
a client carry that client's address.

### Annex C.1.1.4 payloadLength little-endian

**Spec:** Annex C.1.1.4, p. 166 — "The payloadLength indicates the length of the
payload field in bytes, that is, the XRCE Message. The payloadLength shall be
encoded using little endian format."

**Repo:** `crates/xrce/src/transport_serial.rs` (`encode_frame_with`, `FrameAssembler::push`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`

**Status:** done

### Annex C.1.1.5 payload is an XRCE message

**Spec:** Annex C.1.1.5, p. 166 — "The payload shall contain an XRCE Message."

**Repo:** `crates/xrce/src/transport_serial.rs` (`encode_message`, `decode_frame`,
`SerialFramer::push_bytes`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::the_framer_takes_frames_split_and_back_to_back_and_skips_noise`,
`crates/xrce/src/transport_serial.rs::tests::decode_frame_returns_the_rest_after_the_frame`

**Status:** done — the limit is 65 535 octets, what `payloadLength` holds;
`encode_frame` refuses a longer payload with `FrameTooLong`.

### Annex C.1.1.6 frameCheck according to RFC 1662

**Spec:** Annex C.1.1.6, p. 166 — "The frameCheck shall contain the 16-bits
cyclic redundancy check (CRC) of the original frame without including the
BEGIN_FLAG or transparency stuffing. The frameCheck shall be computed as it is
described in [IETF RFC-1662] using the polynomials: x 16 + x 12 + x 5 + 1."

**Repo:** `crates/xrce/src/transport_serial.rs` (`fcs16`, `FrameCheck::compute`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::the_frame_check_is_the_fcs16_of_rfc_1662`,
`crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`

**Status:** done — FCS-16 of RFC 1662 (reflected `0x8408`, init `0xFFFF`,
complemented) over addresses, `payloadLength` and payload; 0x906E for
"123456789", the check value of RFC 1662 Appendix C.

### Annex C.1.1.6 frameCheck little-endian

**Spec:** Annex C.1.1.6, p. 166 — "The frameCheck shall be encoded using little
endian format."

**Repo:** `crates/xrce/src/transport_serial.rs` (`encode_frame_with`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`

**Status:** done

### Annex C.1.2 Stuffing with ESCAPE_FLAG and XOR_FLAG

**Spec:** Annex C.1.2, p. 166 — "The ESCAPE_FLAG has a hexadecimal value of
0x7D. Each byte inside the frame which value matches with the BEGIN_FLAG or the
ESCAPE_FLAG is replaced by two bytes: the ESCAPE_FLAG followed by the original
byte XOR with the XOR_FLAG which has a value of 0x20. [...] each ESCAPE_FLAG
found shall be discarded and an XOR with the XOR_FLAG operator shall be applied
on the next byte."

**Repo:** `crates/xrce/src/transport_serial.rs` (`ESCAPE_BYTE`, `STUFF_XOR`, `encode_frame_with`,
`FrameAssembler::push`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::flag_and_escape_octets_are_stuffed_everywhere_after_the_flag`,
`crates/xrce/src/transport_serial.rs::tests::escape_errors_and_short_frames_are_refused`

**Status:** done — everything after the flag is stuffed, addresses, length and
`frameCheck` too. After `0x7D` the code refuses every byte that does not become
`0x7E` or `0x7D`; the spec does not require that.

### Annex C.1.3 Dropping on a wrong CRC

**Spec:** Annex C.1.3, p. 166 — "A frame shall be dropped in the following
cases: When the frameCheck attached to the frame is not the same as the
computed CRC."

**Repo:** `crates/xrce/src/transport_serial.rs` (`FrameAssembler::push`); `crates/xrce-agent/src/daemon/transport.rs`
(`run_serial_listener`: message "frame dropped" with the reason)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_frame_with_a_wrong_check_is_refused`,
`crates/xrce/src/transport_serial.rs::tests::the_frame_check_of_micro_xrce_dds_is_taken_when_chosen`

**Status:** done

### Annex C.1.3 Dropping on a BEGIN_FLAG before the end of the payload

**Spec:** Annex C.1.3, p. 166 — "When a (non escaped) BEGIN_FLAG is found
before the payload is completely read."

**Repo:** `crates/xrce/src/transport_serial.rs` (`SerialFramer::push_bytes`, `decode_frame_inner_with`)

**Tests:** `crates/xrce/src/transport_serial.rs::tests::a_begin_flag_inside_a_frame_drops_it_and_begins_the_next`

**Status:** done — the frame is dropped with `BeginFlagInFrame`, and the flag
begins the next frame.

---

## Audit-Status

490 done / 0 partial / 0 open / 79 n/a (informative) / 0 n/a (rejected).

Test run: the tests of the items ran on the Linux test host in rounds 1 to 6 (`zerodds-xrce`, `zerodds-xrce-agent`, `zerodds-xrce-client` with and without default features, `zerodds-endpoint-e2e`, `tools/endpoints-gate` 24 of 24); items changed after that are not confirmed by a run. This document ran no test.

Forms of other DDS products in DDS-XML: the readers of `crates/xml` follow the schemas of DDS-XML 1.0 and refuse a foreign form (`<domainparticipant_qos>`, the `qos_profile_ref` attribute, `baseType`, the IDL names `long` and `octet` as `type`, and others) with an error that names the form and the specification's. The agent setting `vendor_forms: accept` reads them (needed for profile files of eProsima) and lists each form read at start; the default is `refuse`. This is checked only by reading the code and the tests; nothing was run on this machine.

Open items: none.
