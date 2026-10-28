# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Changed
- A CREATE by reference whose `object_reference` is no `elementNameReference` of DDS-XML (§7.7.3.1.1) is `STATUS_ERR_INVALID_DATA` (it was `STATUS_ERR_UNKNOWN_REFERENCE`); white space at its edges is collapsed.
- `GET_INFO(OBJECTID_AGENT)` reports the DTLS and TLS listeners and a serial line in `address_seq` as `ADDRESS_FORMAT_STRING` locators (`dtls://host:port`, `tls://host:port`, `serial://device?address=n`); it left them out. A plain locator of the same host would lead a client to connect without the protection of §8.3.2.4.
- A message to a ClientKey without a ProxyClient (other than CREATE_CLIENT, GET_INFO and `DELETE(OBJECTID_CLIENT)`) is dropped without an answer and reported as `SessionEvent::NoClient` (DDS-XRCE 1.0 §7.8.1; `StatusValue` has no `STATUS_ERR_NOCLIENT`); it was answered `STATUS_ERR_UNKNOWN_REFERENCE`. Configuration key `no_client_threshold:` (default 16, 0 never): after that many such messages from one peer the connection is closed (TCP) or the peer is no longer read (UDP).
- `STATUS_AGENT` carries the `agent_info` of the IDL of Annex A (§8.3.5.5.2) alone by default; a failure carries the `ResultStatus` the text gives it. Configuration key `status_agent:` (`annex_a`, default; `with_result_status`: always a `ResultStatus` in front, as the text and the eProsima client have it). The client reads both (`StatusAgentPayload::result_and_position`).
- A DomainParticipant is created disabled (DDS-XRCE 1.0 §7.8.3.1): `RealDdsPort::create_participant_with` no longer starts the DCPS runtime of the domain (or the one of a participant with USER_DATA); `DdsPort::enable` of the participant does, after the CREATE succeeded. A CREATE that fails leaves no runtime and no announcement. A domain id above 232 is refused at CREATE.
- `unknown_topic_types:` defaults to `refuse` (DDS-XRCE 1.0 §7.7.3.7.3: "The referenced type shall be known to the XRCE Agent", `STATUS_ERR_UNKNOWN_REFERENCE`); `forward` (the eProsima client names types the agent does not hold) is the named setting. `SessionEngineConfig::refuse_unknown_topic_types` is now `forward_unknown_topic_types`.

### Added
- Configuration key `vendor_forms:` (`refuse`, default; `accept`): the DDS-XML readers follow the schemas of DDS-XML 1.0 and refuse a form other DDS products write (`<domainparticipant_qos>`, the `qos_profile_ref` attribute, `baseType`, the IDL names `long` and `octet` as `type`, and others) with an error that names it and the form of the specification. `accept` reads them (profile files of eProsima) and prints each one at start.
- A `<types>` string of `CREATE(OBJK_TYPE)` may reference types the agent already knows (DDS-XRCE 1.0 §7.7.3.3.2): the types the client created, those of `type_files:` and of the profile files. A name that is neither defined in it nor known is `STATUS_ERR_UNKNOWN_REFERENCE`.
- Configuration key `delete_client_reply:` (`status`, default; `status_agent`): `DELETE(OBJECTID_CLIENT)` is answered with `STATUS` (§8.3.5.4, and the only message of Annex A that carries a `ResultStatus`); Table 14 names `STATUS_AGENT` for `delete_client`. The client ignores a `STATUS_AGENT` that arrives when no `CREATE_CLIENT` is pending.
- Configuration key `max_elapsed_time_unit:` (`seconds`, default; `milliseconds`): the unit of `max_elapsed_time` of a `DataDeliveryControl`. DDS-XRCE 1.0 Table 9 (the definition) gives seconds; the example of Annex B.4.2 writes 30000 for 30 seconds.
- QoS the agent applies (DDS-XRCE 1.0 §7.1, §7.7.3): LIVELINESS, LATENCY_BUDGET, DESTINATION_ORDER, TRANSPORT_PRIORITY, DURABILITY_SERVICE and TOPIC_DATA reach the DataWriter/DataReader (`EndpointQos`); the QoS of a Topic (`<topic_qos>`, `qos_profile`) is the default of the endpoints created for it (`entity_repr::TopicQos`, `endpoint_for_topic`); the QoS of an XML DomainParticipant (`<domain_participant_qos>`) is applied like the one of a `qos_profile_reference`. Configuration key `unsupported_qos:` (`refuse`, default; `ignore`): QoS the agent cannot apply (WRITER_DATA_LIFECYCLE with autodispose off, READER_DATA_LIFECYCLE with an autopurge delay, elements of a vendor dialect) is `STATUS_ERR_INCOMPATIBLE`; `ignore` creates the object without it and reports what was left out.
- `DdsPort::poll_samples` returns `PolledSample` (payload, `SampleInfoFlags`, source timestamp): a DATA in FORMAT_SAMPLE, _SAMPLE_SEQ or _PACKED_SAMPLES carries `VIEW_STATE_NEW` for the first sample of an instance, `INSTANCE_STATE_DISPOSED`/`_UNREGISTERED` for a dispose or unregister the DataReader reports (a sample without data), and the source timestamp of the writer as the `session_time_offset`. FORMAT_DATA and FORMAT_DATA_SEQ have no `SampleInfo` and send no marker. Configuration key `sample_info:` (`annex_a`, default; `flat`).
- Configuration key `representation_formats:` (`spec`, default; `any`). DDS-XRCE 1.0 §7.7.3.1 and Annex A give each kind its representation formats: a Type, QosProfile and Application by reference or XML, a Publisher, Subscriber, DataWriter and DataReader binary or XML, a DomainParticipant and Topic all three. A CREATE in another format is `STATUS_ERR_INVALID_DATA`; `any` admits it (the Micro XRCE-DDS client's `uxr_buffer_create_*_ref` creates Publishers, Subscribers, DataWriters and DataReaders by reference; not measured here: the captured session has none).
- Configuration key `representation_timestamps:` (`absent`, default; `present`): the form of `CLIENT_Representation` / `AGENT_Representation` (DDS-XRCE 1.0 §7.8.2.1 prose with `client_timestamp` / `agent_timestamp`, or the IDL of Annex A and the bytes of Annex B.1). The agent reads and writes the form named.
- `OBJK_DomainParticipant_Binary` (§7.7.3.6.3): `domain_reference` is resolved against the Domains of the `<domain_library>` files the agent read (`STATUS_ERR_UNKNOWN_REFERENCE` only for a name in no library; the Domain's `domain_id` must agree with the binary form's). `qos_profile_reference` is applied: USER_DATA (a participant with USER_DATA gets a DCPS runtime of its own, shared by participants with the same value) and ENTITY_FACTORY (held by the participant: its Publishers, Subscribers and Topics are disabled when `autoenable_created_entities` is false, and the endpoints under them are refused); a policy a DomainParticipant does not have in the profile's participant QoS is `STATUS_ERR_INCOMPATIBLE`.
- Configuration key `unannotated_types:` (`appendable`, default; `final`): the form of the Annex A types without `@extensibility`. `final` writes the octets of the reference implementation; clients that read only that form (eProsima Micro XRCE-DDS) need it. The agent reads both forms.
- The agent sends `TIMESTAMP` (DDS-XRCE 1.0 §8.3.5.15) on `STREAMID_NONE` with its own time: `SessionEngine::request_timestamp`, and to every client every `timestamp_interval_secs:` seconds at `tick` (default 0: none). A `TIMESTAMP_REPLY` of the client (§8.3.5.16) becomes `SessionEvent::ClockSynchronised` with the offset of the client's clock and the round trip, from the four timestamps as NTP does.
- `discovery.address:` sets the multicast group the agent joins for discovery (DDS-XRCE 1.0 §11.2.4; default 239.255.0.2). A `GET_INFO` of the Root that carries the `client_info` of a client of another major version is answered with an `INFO` of `STATUS_ERR_INCOMPATIBLE` (§8.4.4 Figure 10), on the discovery group and in or out of a session; `bind_discovery_socket` takes the group.
- A CREATE of a Publisher or Subscriber with XML makes the `<data_writer>` or `<data_reader>` elements it holds (DDS-XRCE 1.0 §7.7.3.8.1, §7.7.3.9.1); they were dropped. A topic named by `topic_ref` is looked up among the topics of the DomainParticipant of the Publisher. A `<types>` that defines several types needs exactly one without `nested="true"` (§7.7.3.3.2); a `<qos_library>` of a QosProfile holds a single `<qos_profile>` (§7.7.3.2.2), and a `base_name` written `Library:Profile` as in the clause is read as `Library::Profile`.
- Objects the agent's definitions hold exist for every client without a CREATE (DDS-XRCE 1.0 §7.3, §8.4.2, §8.4.3, §9.3): a WRITE_DATA, READ_DATA, GET_INFO or CREATE that names the `ObjectId` of a configured object (the MD5 of its name, Table 16) the client has not made gets it made, the DomainParticipant with everything inside it, subject to the AccessController (`STATUS_ERR_DENIED` without the permission to create). `client_applications:` (entries `client:` and `application:`, an Application or DomainParticipant reference) makes objects for a client when its `CREATE_CLIENT` arrives; a second `CREATE_CLIENT` of the session reuses them. The `ObjectId` of an object no definition has is still unknown (`STATUS_ERR_UNKNOWN_REFERENCE`).
- The XML forms of DDS-XML for the objects of a CREATE (DDS-XRCE 1.0 §7.7.3.2 to §7.7.3.11, Table 6): a DomainParticipant as `<domain_participant_library>` (also by `REPRESENTATION_BY_REFERENCE`, `Library::Participant`) with its `domain_id` or `domain_ref`, its `<register_type>`, `<topic>`, `<publisher>`, `<subscriber>`, `<data_writer>` and `<data_reader>`; a topic as `<topic name register_type_ref>` or by the bare name of a topic of the DomainParticipant or its Domain (§7.7.3.7.1); a DataWriter or DataReader with `topic_ref="Square"` (a name; `0x…` stays an ObjectId), `<datawriter_qos base_name="Lib::Profile">` / `<data_writer_qos>` / `<datareader_qos>` over a QosProfile, `qos_profile_ref` and `<content_filter><expression>`; a Publisher or Subscriber with `<publisher_qos>` / `<subscriber_qos>`. The creation of a DomainParticipant or Application makes the objects inside it, with the ObjectIds of their names (§9.3); a nested object that fails fails the whole.
- A QosProfile (`<qos_library>`), Type (`<types>`), Application (`<application_library>`) or DomainParticipant (`<domain_participant_library>`) a client creates with XML is known to the agent while the object exists, and any client refers to it with `REPRESENTATION_BY_REFERENCE` (§7.7.3.1.1, §9.2); CREATE of a QosProfile, Type or Application by reference needs a definition the agent holds (`STATUS_ERR_UNKNOWN_REFERENCE` otherwise).
- `xml_profile_files:` read the configuration file of DDS-XRCE 1.0 §9.3 (`dds-xml_system_example.xsd`) completely: `<domain_library>`, `<domain_participant_library>` and `<application_library>` as well as `<types>` and `<qos_library>`, a library also as the root of a file. The Domains, DomainParticipants and Applications are referenceable under `Library::Name` (`Library::Application::Participant` for a participant of an application), the topics, publishers, subscribers, DataWriters and DataReaders of a DomainParticipant under their names (Table 15). `daemon::definitions`, `AgentProfiles::predefined_objects`: every object has the `ObjectId` of the MD5 of its reference (Table 16). A file whose participants cannot be planned, or whose top-level objects derive one `ObjectId`, is refused at start. The elements were listed as unused.
- `DdsPort::enable` (DDS-XRCE 1.0 §7.8.3.1): a DataWriter or DataReader is made disabled and registered with the runtime, and so announced by SEDP, only when the XRCE object is complete. A CREATE that fails after the DDS side was made leaves no discovery traffic. `RealDdsPort` reserves the entity id at `create_writer`/`create_reader` and registers it at `enable`; the default implementation does nothing.
- `same_session_create_client: keep | restart` (`SessionEngineConfig::restart_on_same_session`): `keep` (default) follows DDS-XRCE 1.0 §7.8.2.1, a `CREATE_CLIENT` with the `session_id` of the client's session performs no action; `restart` starts the streams of the session again, for a client process that restarts with the same `ClientKey` and `session_id` and numbers its messages from 0, as the processes of the captured eProsima session do.
- DTLS and TLS listeners (DDS-XRCE 1.0 §8.3.2.4, §11.1 (5), §11.2, §11.3; features `dtls` and `tls`): `listeners:` entries `kind: dtls` and `kind: tls` with `bind`, `cert_file`, `key_file` and an optional `client_ca_file` (a client then has to present a certificate that chains to it). The agent authenticates itself with its certificate, the messages, the `ClientKey` among them, travel encrypted, and a plain-text `CREATE_CLIENT` on such a port gets no `STATUS_AGENT`. A build without the feature refuses the entry at start. Secured listeners are not announced as plain locators by `GET_INFO(OBJECTID_AGENT)` or the multicast discovery.
- AccessController of DDS-XRCE 1.0 §7.4 (`daemon::access`): the ClientKeys configured in the agent (`authorized_client_keys:`, four octets in hexadecimal in wire order; replaces the earlier `u32` form), properties a `CREATE_CLIENT` must carry (`required_client_properties:`, §7.8.2.1), and per-client rules (`access_rules:` with `effect`, `client`, `operations`, `kinds`, `domains`, `topics`) for CREATE, GET_INFO, DELETE, WRITE_DATA and READ_DATA of §7.8.3 to §7.8.5. A refused request is answered with `STATUS_ERR_DENIED` and reported as `SessionEvent::OperationDenied`. Replacing an object needs the permission to delete it. `SessionEngineConfig::access` and `DaemonConfig::access` carry the controller; without rules nothing is restricted, with rules an unmatched operation is denied.

### Fixed
- A `<types>` CREATE that references a known type is registered with the definitions of the agent; it was refused as `STATUS_ERR_INVALID_DATA` when the definitions were checked. The `type_files:` types are known there too.
- The CREATE of a participant as XML inside a `<dds>` root reads the `<domain_participant>` (or `<domain_participant_library>`) in it; its QoS (USER_DATA, ENTITY_FACTORY) was not read.
- The DDS runtime of an agent has a GUID prefix of its own: it was derived from the domain id and the host alone, so two agents on one host announced the same GUID in one domain and never discovered each other (DDS-XRCE 1.0 §10.2: clients at different agents exchange data through DDS). The prefix carries the process and a counter of the runtimes of the process.
- A WRITE_DATA, READ_DATA, DELETE or GET_INFO whose body cannot be read is answered with `STATUS_ERR_INVALID_DATA` (against the `request_id` and `object_id` in its first four octets when they are there) and reported as `SessionEvent::RequestUnreadable`; it was dropped without a reply.
- A CREATE with `reuse` and `replace` — the update of an object (§7.8.3.2, §8.4.13, Table 14) — replaces an object whose representation, container or domain does not match (`STATUS_OK`) and answers `STATUS_OK_MATCHES` for one that matches; it was `STATUS_ERR_MISMATCH` for every unequal object. The update needs the `update` permission of the AccessController on the object (a replace without `reuse` needs `delete`). `update_of_missing_object: refuse` answers the update of an object that does not exist with `STATUS_ERR_UNKNOWN_REFERENCE` (§7.8.3.2); the default creates it (Table 5).
- The DATA of a READ_DATA is in the `data_format` the READ_DATA asked for: FORMAT_SAMPLE one DATA per sample, FORMAT_DATA_SEQ, FORMAT_SAMPLE_SEQ and FORMAT_PACKED_SAMPLES as many samples in one DATA as a submessage holds (a packed one 256 at most) (DDS-XRCE 1.0 §7.8.5.1 Table 9, §8.3.5.10.2). Every format was FORMAT_DATA. The `SampleInfo` carries what the agent knows: `sequence_number` counts the samples delivered from the reader, `session_time_offset` is the time of delivery in milliseconds after the start of the session, `state` is zero; the DataReader port hands over no source timestamp or sample, view or instance state. A sample too large for a submessage is `STATUS_ERR_RESOURCES`, as before.
- A CREATE too short to hold a `BaseObjectRequest` is answered with `STATUS_ERR_INVALID_DATA` against `OBJECTID_INVALID` (the `request_id` when its two octets are there) and reported as `SessionEvent::CreateRefused` (DDS-XRCE 1.0 §8.3.5.2). It was dropped without a reply.
- The `ObjectId` of a CREATE is the prefix of the request's `object_id` with the `ObjectKind` of the representation's discriminator (§7.8.3.1); a request whose `object_id` carries another kind in its low bits was answered with `STATUS_ERR_INVALID_DATA`. The STATUS names the constructed `ObjectId`.
- A CREATE with `reuse` compares the container (`participant_id`, `publisher_id`, `subscriber_id`) and the `domain_id` of the representation as well: the same XML under another container is `STATUS_ERR_MISMATCH` (Table 5).
- A replace whose new DDS side cannot be made returns the object to its previous configuration, in the store and on the DDS side (§7.8.3.2), and answers with the error. The object was deleted.
- A `CREATE_CLIENT` whose payload cannot be read (a cookie that is not `XRCE_COOKIE`) is answered with a `STATUS_AGENT` with `STATUS_ERR_INVALID_DATA`, not with a `STATUS` against `OBJECTID_CLIENT` (DDS-XRCE 1.0 §7.8.2.1, §8.3.5.1).
- A `CREATE_CLIENT` for the session that exists no longer resets the reliable streams, the fragment runs, the backlog and the best-effort counters (§7.8.2.1: "shall not perform any action"); see `same_session_create_client`.
- `DELETE` of `OBJECTID_CLIENT` for a ClientKey without a ProxyClient is answered with `STATUS_ERR_UNKNOWN_REFERENCE` (§7.8.2.3); deleting a ProxyClient also drops the backlog and the best-effort counters of the client.
- A WRITE_DATA that was written is answered with `STATUS_OK` against the DataWriter and the request_id of the request, one STATUS for all samples of a sequence format (DDS-XRCE 1.0 §7.8.4.1, §8.3.5.8). An eProsima client that sends WRITE_DATA receives a STATUS it did not get from the eProsima agent in the captured session.
- A reassembled FRAGMENT run that holds no submessage is answered with `STATUS_ERR_INVALID_DATA` against `OBJECTID_CLIENT` (DDS-XRCE 1.0 §8.3.5.14).
- Binary representations (DDS-XRCE 1.0 §7.7.3.1.3, §7.7.3.6.3 to §7.7.3.11.2, Annex A, Annex B.2): `daemon::binary_forms` reads the structures in XCDR2 as Annex A gives them (`@extensibility(APPENDABLE)`, a DHEADER in front; bit 1 of `EndpointQosFlags` is `is_history_keep_all`) and as the clauses give them (`FINAL`; `OBJK_DomainParticipant_Binary` with `long domain_id`; the endpoint QoS not optional; bit 1 is `is_history_keep_last`): `OBJK_DomainParticipant_Binary` (the participant's binary form was ignored), `OBJK_Topic_Binary` with `type_reference` and `type_identifier` (the agent resolves a `TypeIdentifier` to the known type that has it, and the identifier takes precedence), `OBJK_Publisher_Binary`/`OBJK_Subscriber_Binary`, `OBJK_DataWriter_Binary`/`OBJK_DataReader_Binary` with `topic_name`. The classic-CDR structures of the eProsima client (the `topic_id` of an endpoint, the third string of a topic, 64-bit numbers) are read when a blob is none of these; the error of a blob that is none names what each reading found. The `CREATE` of Annex B.2.2 and B.2.3 (the DHEADER `0x8000000C`/`0x80000002` where `sequence<octet>` has its length) is read. A reference to an XRCE Domain the agent does not hold is `STATUS_ERR_UNKNOWN_REFERENCE`; a `qos_profile_reference` is checked and reported as not applied (the participants of a domain share one `DcpsRuntime`). `unknown_topic_types: refuse` makes a topic of a type the agent does not know `STATUS_ERR_UNKNOWN_REFERENCE` (§7.7.3.7.3, "shall be known"); `forward` stays the default.
- GET_INFO and INFO (DDS-XRCE 1.0 §7.8.2.2, §7.8.3.3, §8.3.5.3, §8.3.5.7): the payload's `InfoMask` is read and selects the activity and/or the configuration; the ObjectKind in the last four bits of the `object_id` selects the Root (`OBJK_AGENT`, any id of that kind) or the ProxyClient; the Root answers without a session on the session listeners too (and the multicast discovery answers on its own); the agent's INFO carries `AGENT_ActivityInfo` with its locators and the `AGENT_Representation` with version and vendor; the INFO of an object carries the `ObjectVariant` the object was created with (kind, representation, trailing field) and, for a DataWriter or DataReader, its activity; a failure (unknown object, no permission, a payload without mask) is an INFO whose `ResultStatus` is the error, not a STATUS; the INFO echoes `request_id` and `object_id`. `run_discovery_responder` takes the agent's `vendor_id`.
- WRITE_DATA (DDS-XRCE 1.0 §7.8.4.1, §8.3.5.8, Annex B.5.2 to B.5.5): FORMAT_DATA_SEQ, FORMAT_SAMPLE_SEQ and FORMAT_PACKED_SAMPLES are written, sample by sample; the topic's type tells where each serialized sample ends (`DdsPort::sample_length`), after the `SampleInfo` or `SampleInfoDelta` in front of it. A sequence that does not split (count the bytes cannot hold, bytes left over, a truncated sample, a topic of unknown type) writes nothing and is answered with `STATUS_ERR_INVALID_DATA`. The `session_time_offset` of a sample stamps it: the source time is the time the session was created (by the agent's clock; the `CLIENT_Representation` of Annex A has no `client_timestamp`) plus the offset (`DdsPort::write` takes `source_time`). A dispose or unregister of an instance the agent cannot name (the topic's type is unknown or has no key) is answered with `STATUS_ERR_INVALID_DATA` and writes nothing; it was written as a plain update with only an event. `WriteOutcome::DisposeRequestedButUnsupported` is removed.
- The locators the agent reports (`GET_INFO(OBJECTID_AGENT)`, multicast discovery) are `ADDRESS_FORMAT_MEDIUM` for UDP and TCP on an IPv4 address and `ADDRESS_FORMAT_LARGE` for IPv6 (DDS-XRCE 1.0 §11.2.1, §11.3.1), as a `TransportLocatorSeq`; they were `SMALL` for UDP, a variant `TransportLocatorFormat` does not have for TCP and serial lines, and IPv6 was left out. A serial listener has no locator and is not reported.

- Sessions and streams (DDS-XRCE 1.0 §8.2.2, §8.3.2.1, §8.3.2.3, §8.3.5.13): a message whose `session_id` is not the client's active session, or that carries `SESSION_ID_NONE_*` and is not a CREATE_CLIENT, is not processed and reported as `SessionEvent::MessageRefused`; a CREATE_CLIENT that asks for a session id of 0x00 or 0x80 is `STATUS_ERR_INVALID_DATA`; a message on a best-effort stream that is a duplicate of the last one or older is discarded (`SessionEvent::BestEffortMessageDiscarded`); RESET also ends the running reads, empties the backlog of messages waiting for a sender window, restarts the sequence numbers of the best-effort streams, and resets reliable stream 255, which it skipped.

- A Publisher and a Subscriber CREATE (DDS-XRCE 1.0 §7.7.3.8, §7.7.3.9, §7.8.3.1 Table 6) checks its `participant_id`: an id that is no `OBJK_PARTICIPANT` is `STATUS_ERR_INVALID_DATA`, a participant the client does not hold is `STATUS_ERR_UNKNOWN_REFERENCE`, and nothing is stored. Both now have a DDS entity (§7.5): `DdsPort::create_publisher` and `create_subscriber`; `RealDdsPort` gives each a group entity that its endpoints announce as `PID_GROUP_GUID` and applies PRESENTATION; ENTITY_FACTORY with `autoenable_created_entities` false refuses the endpoints under it, as XRCE cannot enable them. `DdsPort::create_writer` and `create_reader` take the Publisher or Subscriber; an endpoint, its container and its topic must belong to one participant. The XML form of a Publisher or Subscriber reads `<presentation>` and `<entity_factory>`.
- A CREATE in the representations the eProsima Micro-XRCE-DDS Client 3.0.1
  sends creates the DDS entity it describes. The daemon read only its own
  XML attribute form (`<topic name=".." type_name=".."/>`); the Fast DDS
  element form (`<topic><name>..</name><dataType>..</dataType></topic>`,
  `<data_writer><topic><kind>..`) and the `OBJK_*_Binary` blobs were
  created as topic `topic-<id>` with type `unknown`, which no DDS peer
  matches. `daemon::entity_repr` reads all three forms: topic and type
  names, an endpoint's topic by `ObjectId` or by DDS topic name, its
  Publisher/Subscriber, and the QoS they carry (RELIABILITY, DURABILITY,
  HISTORY, OWNERSHIP and its strength, DEADLINE, LIFESPAN,
  TIME_BASED_FILTER, a reader's content filter, USER_DATA, PARTITION,
  GROUP_DATA). An element the agent does not apply is named in the
  `RepresentationNotApplied` event; a representation that does not parse
  or names nothing the agent holds is refused (`CreateRefused`, STATUS
  `ERR_INVALID_DATA`/`ERR_UNKNOWN_REFERENCE`) before anything is stored.
- An endpoint without QoS in its representation gets the DDS 1.4 §2.2.3
  defaults of its role: a DataReader is BEST_EFFORT, not RELIABLE.
- A participant joins the domain its CREATE carries
  (`OBJK_PARTICIPANT_Representation.domain_id`); the field was ignored and
  every participant without an XML `<domain_id>` went to domain 0.
- A reused CREATE with an equal representation no longer registers a
  second DDS entity; a replaced one tears the old one down first. A CREATE
  whose DDS side fails is removed from the object store again.
- A reader's queue keeps its HISTORY (KEEP_LAST per instance) and
  TIME_BASED_FILTER; at its 1024-sample cap under KEEP_ALL a sample that
  cannot be kept is reported, where it was dropped without a word.
- An endpoint's writes and its teardown go to the runtime of its own
  domain, not to the first runtime the daemon holds.
- A client that registers again from a new address — every eProsima example
  process reuses its ClientKey — is answered at that address. The peer
  table kept every address a client had used and replied to any one of
  them; the second run of an example often saw no STATUS_AGENT ("Error at
  create session", measured).
- A topic is created with the type its type name names. The client's only
  binary OBJK_TYPE used to be applied to every topic of that client,
  whatever type it named. A client's OBJK_TYPE objects (binary COMPLETE
  TypeObject, or a DDS-XML type library) now define types by name;
  `type_files:` defines them for every client — the key was parsed and
  never used. Keyed-ness and extensibility come from the type itself.
- Best-effort output streams carry increasing sequence numbers (§8.3.2).
  Every message went out with 0, and a best-effort client takes only a
  message newer than the last: a subscriber received the first of three
  samples (measured).
- A reliable message the sender window has no room for waits for the
  client's ACKNACK. It was sent unsequenced, which a reliable client drops:
  the fragmented BigHelloWorld samples never arrived (measured).
- A `BY_REFERENCE` CREATE resolves against `xml_profile_files:`, Fast DDS
  profile files as the eProsima agent reads them with `-r` (a referenced
  `<topic profile_name="…">` is read like the same element inline;
  `<types>` in those files define topic types), and an attribute-form
  `qos_profile="Library::Profile"` against the DDS-XML `<qos_library>`
  documents of `qos_library_files:`, with its `base_name` chain. Both keys
  were parsed and never used; a reference used to create a topic named
  `topic-<id>` of type `unknown`.
- A `qos_profile` reference is read against a profile's filtered QoS
  containers (DDS-XML 1.0 §7.3.2.4.3). A profile may hold several
  DataWriter/DataReader containers, each with its own `topic_filter`;
  the agent has no topic name at that point, so the filterless
  container applies, else the first in document order.

### Removed

- The `domain:` key and the `--domain` flag. They were parsed and never
  used; a participant's domain is the one its CREATE carries.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the **`zerodds-xrce-agent`** crate as a Layer-7 profile.

### Spec-Referenzen
Siehe `README.md` + `docs/spec-coverage/<spec>.md`.

### Public-API
Siehe `README.md` + `src/lib.rs` Doc-Comments.

### Implementation
XRCE agent running in Full/Standard profile

### Architektur
- Layer: 7 (Profiles)

### Stabilitaet
All `pub` items are RC1-stable; breaking changes require a major bump.
