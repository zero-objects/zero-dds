# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Changed

- The listener traits declare each callback group once: `PublisherListener`,
  `SubscriberListener` and `DomainParticipantListener` repeat the callbacks of
  `DataWriterListener`, `DataReaderListener`, `SubscriberListener` and `TopicListener` through one
  macro per group (`listener.rs`). The traits, their methods and defaults are the same.
- Tests, examples and the bridges that wrote every member of `UserWriterConfig` and
  `UserReaderConfig` name only the members that differ from `Default` and end in
  `..Default::default()`.

### Added

- The runtime setting `security.origin_authentication_fallback` (`spec`, `accept`;
  `ZERODDS_SEC_ORIGIN_AUTHENTICATION_FALLBACK`, the file before the environment) in
  `SecurityVendorSettings`, passed to the security crates like the other vendor forms.

### Changed

- The TypeLookup service writes its requests and replies in CDR2_LE with the encoding version 2 by
  default (DDS-XTypes 1.4 Beta 1 §7.6.3.3.3, OMG issue DDSXTY14-29), CDR_LE with version 1 under
  the setting `wire.type_lookup_encoding = xcdr1` (`ZERODDS_WIRE_TYPE_LOOKUP_ENCODING`,
  `RuntimeConfig::type_lookup_encoding`; DDSI-RTPS 2.5 §10.4 Table 10.2, XTypes 1.3 Table 60). A
  received sample is read in the encoding its identifier names; another identifier is reported as
  `typelookup.sample.rejected`. Cyclone DDS 11.0.1, OpenDDS 3.34.0 and RTI Connext 7.7.0 read only
  CDR2_LE (measured, `internal/release/evidence/rc7-typelookup-encapsulation`); with CDR_LE their
  TypeLookup failed, and after the grace a pair matched on its type name while the peer refused it,
  so ZeroDDS reported a match no sample crossed (GitHub #41, #42).

- The participant prefix: the process id carries a mark of the PID namespace (`namespaced_pid`, XOR with a mask from the inode of `/proc/self/ns/pid`, zero in the initial namespace), and the 16-bit seed comes from the randomness of the operating system, not the clock (DDSI-RTPS 2.5 §8.2.4.2). Containers with the host's network and their own PID namespace no longer share a prefix.

### Fixed

- `ignore_participant`, `ignore_topic`, `ignore_publication` and `ignore_subscription` (DDS 1.4
  §2.2.2.2.1.14–17): a matching pass that was under way when the call came no longer matches the
  ignored endpoint again. The pass looked at the ignore filter before it took the endpoint's slot
  and once more after the wiring; one that waited in between (for the participant cache) wired the
  endpoint after the call had disconnected it, and the ignored writer's samples were delivered
  until the second look took it out (`ignore_entities.rs`, 2 of 200 runs on the Linux test host).
  The filter is asked where the pair is wired, under the slot `disconnect_ignored` takes.
- `get_status_changes` and the StatusCondition of a `DataReader` and a `DataWriter` answer the
  communication statuses as the runtime's ledgers hold them at the call: changed since they were
  last read or handed to a listener (DDS 1.4 §2.2.4.1). The flag was raised only by a listener
  look, and a look the dispatcher thread ran made the entity's own look return at once, so a
  status that had changed read as unchanged until the dispatcher set the flag
  (`entity_scoping::get_status_changes_reports_the_entity_itself`: 7 of 200 runs red under four
  parallel loops on the Linux test host, 0 of 200 after). The C API's `get_status_changes`
  already read the ledgers.
- A participant learned in-process (another `DcpsRuntime` of the process on the same
  domain) is held under the lease it announces and renewed by its periodic SPDP
  announcement, as one learned from the wire (DDSI-RTPS 2.5 §8.5.3.2). The in-process
  path stored it without an observation time: a participant of the process that ended
  without notice stayed discovered, its writers matched and their liveliness was not
  lost (measured: `liveliness_qos::linux::a_writer_lost_with_its_participant_counts_as_not_alive`
  red in 7 of 100 runs on a loaded host).
- A reader whose topic protects the payload (`IS_PAYLOAD_PROTECTED` of its
  EndpointSecurityInfo) refuses a payload that begins with a CryptoHeader and does
  not open, also where the writer sets no NonStandardPayloadFlag (Cyclone DDS, Fast
  DDS). Such a payload reached the application as the CryptoContent (measured with
  Fast DDS, profile oa-enc). On a reader of an unprotected topic the payload stays
  a plaintext CDR_BE payload, and `security.inbound.payload_as_is` records the
  hand-out with the CryptoHeader and the errors.
- A participant refused by `check_remote_participant` (DDS-Security 1.2 §9.4.2.4
  `is_access_protected`, §9.8.7) stays ignored on a runtime started without a
  `DomainParticipant`; its next SPDP brought it and its writers back.
  An authenticating participant drops what was learned of its GUID unauthenticated.

### Added (DDS-Security 1.2 §9.8)

- The `IdentityStatusToken` of the participant data is the token obtained from
  `get_identity_status_token` once; it was obtained a second time and the
  second was not given back (§9.3.2.11.15).

- `RuntimeConfig::key_revision_ack_timeout` (5 s): a key revision is activated
  when the matched participants acknowledged its CryptoTokens or the time ran
  out, not at once (§9.8.9.5 steps 6 and 7, §9.8.10.4).

### Changed (DDS-Security 1.2 §9.8)

- The endpoints of a participant that did not authenticate are not asked of the
  AccessControl plugin (§9.8.8.1); whether a topic is read or write protected
  comes from the TopicSecurityConfig of a plugin of the application.
- With an AccessControl plugin of the application, the Protection Kinds of an
  endpoint follow its EndpointSecurityConfig, and the secure builtin endpoints
  are asked of the plugin (§9.8.4, §9.8.6).

- A revoked identity makes a new key revision only with `<enable_key_revision>`; an endpoint announces the `required_mask` of the plugin's `adjusted_algorithm_info` (PID 0x1013).
### Added (DDS-Security 1.2 §9.7, §9.8)

- `RuntimeConfig::security_data_tagging`; `PublicationBuiltinTopicData` and
  `SubscriptionBuiltinTopicData` carry `data_tags`.
- A Topic is created only where `check_create_topic` permits it; a remote
  endpoint of a topic `check_remote_topic` refuses is not matched; the
  endpoints of a participant that is not authenticated are not matched on
  read- or write-protected topics (§9.8.8.1).

### Fixed

- A match refused because the SEDP cache lost the remote endpoint records
  `match.refused` with the reason; it returned without an event.
- The secure TypeLookup decision read the authenticated peers through the
  lock of the security stack, which the crypto-token preparation holds.

### Changed

- Topic, instance, match and remote-topic checks, `set_qos` of participant,
  topic and writer, and the config of the endpoints go through the AccessControl
  plugin (DDS-Security 1.2 §9.4.2.9); a refusal is NOT_ALLOWED_BY_SECURITY.
  `participant_security_optional_attributes` is sent by default
  (`org.zerodds.sec.optional_attributes=omit` leaves it out); protected endpoints
  announce PID 0x1013; the regular TypeLookup endpoints do not hand out the
  types of protected topics. The endpoint crypto tokens no longer ask
  `builtin_match` under the security stack lock (hang of two tests).

### Added

- `builtin_types::TypeSupport`, `StringTypeSupport`, `KeyedStringTypeSupport`, `BytesTypeSupport` and
  `KeyedBytesTypeSupport` (DDS-XTypes 1.3 Annex E): `get_type_name` and `register_type(participant, name)`.

### Changed

- The runtime fetches the `IdentityStatusToken` (`get_identity_status_token`)
  into the secure SPDP, the algorithm info from
  `set_participant_security_config`, and acts on the events of the
  Authentication plugin: a revoked identity (`on_revoke_identity`) ends the
  communication with the participant, a changed identity status is announced,
  handshake failures of the tick are reported, a participant without an
  IdentityToken or with an incompatible configuration is unauthenticated like
  one that fails the handshake (DDS-Security 1.2 §9.3.2.10, §9.3.2.12).
  After a revocation the participant and every local endpoint get a new
  revision of their key material, and the participant token goes to the
  remaining authenticated participants (§9.3.2.12.2).
- With `dds.sec.crypto.rtps_psk_secret_passphrase` set, bootstrapping messages go out with the flags A and P and unprotected ones are refused (`security.inbound.refused`, check `inbound.psk`).

### Added (DDS-Security 1.2, §7 package)

- The TypeLookup service on the secure builtin endpoints (§7.5.5.2, §7.5.11,
  Table 21): own EntityIds, `PID_AVAILABLE_BUILTIN_ENDPOINTS_EXT` (0x1007) in
  the SPDP of a secured participant, every submessage protected, crypto tokens
  for the four endpoints to a peer that announces them. A peer that does not
  announce them is served on the plain endpoints.
- The builtin topics `DCPSParticipantsSecure`, `DCPSPublicationsSecure` and
  `DCPSSubscriptionsSecure` with their types (`identity_status_token`,
  `data_tags`) and RELIABLE QoS (§7.5.1.6 to §7.5.1.8).
- `DataWriterQos.data_tags` and `DataReaderQos.data_tags`, announced as
  `PID_DATA_TAGS`; the participant's propagated properties in the SPDP.
- An endpoint requires the algorithm its own property
  `dds.sec.crypto.symmetric_cipher_algorithm` names; PID 0x1013 is announced.
- A crypto token for an endpoint this participant does not have is not
  installed (`security.token.not_addressed`); a peer the join access control
  does not admit gets no access to the secure builtin topics.

### Changed

- The vendor-form settings are named `org.zerodds.sec.*` instead of
  `zerodds.sec.*` (`kx_key_fallback`, `compatibility_check`,
  `plaintext_loopback`, new `algo_name_nul`).

### Fixed

- The decision whether a peer is served on the secure TypeLookup endpoints
  took the lock of the security builtin stack a second time while the
  crypto-token preparation held it.

### Changed

- BREAKING (timing): the defaults are the spec's: `writer.nack_response_delay_ms` 200, `reader.heartbeat_response_delay_ms` 500, `discovery.spdp_period_ms` 30000 (DDSI-RTPS 2.5 §8.4.7.1, §8.4.10.1, §9.6.2.4); suppression durations stay 0, `writer.heartbeat_period_ms` 100. The former values (0, 0, 5000) are set through the same keys. A lease under 90 s still lowers the SPDP default to a third of it.

### Added

- `DataWriterQos.property` and `DataReaderQos.property`
  (`PropertyQosPolicy`, DDS-Security 1.2 §7.3.21); a change on an enabled
  entity is IMMUTABLE_POLICY (`PROPERTY`). `UserWriterConfig.property` and
  `UserReaderConfig.property` carry it to the runtime. The entries with
  `propagate = TRUE` are announced as `PID_PROPERTY_LIST`.
  `dds.sec.crypto.symmetric_cipher_algorithm` of the endpoint sets the key
  width of its key material (§10.5.1.1); a value outside Table 22 fails the
  creation with `SecurityConfiguration`. `qos::propagated_properties`.
- `security.inbound.refused` with `check = inbound.submessage` names the
  CryptoHeader (transformation_kind and key id) of a protected submessage that
  no key of its sender opens; it was dropped without a trace.

### Added

- A security configuration that does not validate fails the creation of the
  participant with the return code NOT_ALLOWED_BY_SECURITY (DDS-Security 1.2
  §9.8.1, §7.3.25); `DdsError::SecurityConfiguration` mapped to
  PRECONDITION_NOT_MET.
- `zerodds.sec.kx_key_fallback=accept`: a submessage or encrypted payload that
  no key of its sender opens is tried with the Kx key as well. `0b772e7e6ef`
  removed that unconditional fallback (DDS-Security 1.2 §10.5.2.1.2: the Kx key
  protects the volatile channel only); the setting is the vendor form.
- Compatibility of configurations (DDS-Security 1.2 §7.3.10 to §7.3.24): the
  SPDP carries the signature, key establishment and symmetric cipher algorithm
  infos (PIDs 0x1010 to 0x1012) of the authentication plugin and the crypto
  plugin; a remote participant whose `ParticipantSecurityProtectionInfo` masks
  or algorithm infos (a missing one is the default of the specification) are
  not compatible is not authenticated (`security.participant.incompatible`),
  and an endpoint whose `EndpointSecurityProtectionInfo` or cipher info is not
  compatible is not matched (`match.refused`). Before, none was compared.
  `zerodds.sec.compatibility_check=ignore` turns the checks off.
- Origin authentication (DDS-Security 1.2 §10.4.1.2.6.6, §10.5.3.1): the
  token of an endpoint to a remote endpoint, and the participant token to a
  remote participant, carry a receiver-specific key under a
  `*_WITH_ORIGIN_AUTHENTICATION` kind, and the announced masks name
  `is_submessage_origin_authenticated`, `is_rtps_origin_authenticated`,
  `is_discovery_origin_authenticated` and `is_liveliness_origin_authenticated`
  (they were never set).
- `transport.tcp_locator_kinds` (`vendor` default, `fast-dds`; env `ZERODDS_TCP_LOCATOR_KINDS`; `RuntimeConfig::tcp_locator_kinds_fast_dds`): the LocatorKind values of TCP locators.
- `transport.udp.max_message_size` (env `ZERODDS_UDP_MAX_MESSAGE_SIZE`, default 1472): the fragment size of a user writer follows the smallest `max_message_size` of its transports (DDSI-RTPS 2.5 §8.4.14.1), 1344 at the default. The proxies of a match carry `remoteGroupEntityId`.

### Changed

- Setting `discovery.type_object_v1` (`ZERODDS_TYPE_OBJECT_V1`, default off): endpoints and topics announce
  `type_id` and `type` (XTypes 1.1 Annex B) beside their `TypeInformation`; the `TypeObjectV1` of a remote
  endpoint is read into the registry as Complete TypeObjects before its type is matched.
- `delete_topic` waits for the DataWriters, DataReaders, ContentFilteredTopics and MultiTopics of its own topic
  and not for those of another topic of the same name (DDS-XTypes 1.3 §7.6.4.1): the users are counted per name
  and type name.
- Setting `xtypes.annotation_parameter_order` (`member-index`, `name`; `ZERODDS_ANNOTATION_PARAMETER_ORDER`).
- `read_w_condition`, `take_w_condition` and their `_selected` forms take a `ReadCondition` or a `QueryCondition`
  (`ReadConditionArg`) and return PRECONDITION_NOT_MET for a condition this reader did not create (DDS 1.4
  §2.2.2.5.3.10). New: `read_next_sample`, `take_next_sample`, `read_next_instance_w_condition`,
  `take_next_instance_w_condition`, `return_loan`, `DataReader::get_topicdescription`,
  `Subscriber::get_sample_lost_status`.
- `TypeSupport<T>` with `register_type` and `get_type_name` (§2.2.2.3.6.1);
  `DomainParticipant::create_topic_of_registered_type`. `zerodds_qos::QosPolicy` gives each of the 22
  policies its `name` (§2.2.2.1.3, §2.3.3); `psm_constants::qos_policy_name` has the name constants.
- `Publisher::wait_for_acknowledgments` (§2.2.2.4.1.12).
- A `DATA_REPRESENTATION` id other than XCDR1, XCDR2 and XML is `Unsupported` (it was `BadParameter`, §2.2.2.1.1.1).
- Every listener operation has an entity form (`on_data_available_entity(&AnyDataReader)` and the others,
  §2.3.3); the dispatcher calls it, its default calls the handle form. `TopicDescriptionHandle::instance_handle`.
- The `DCPSTopic` entry is one per participant, topic and type (§2.2.5):
  `TopicBuiltinTopicData::synthesize_key` takes the participant. `ignore_topic` with the handle of a
  remote participant's entry ignores the topic for that participant's endpoints, with the handle of the
  participant's own entry for all.
- An entity that is not enabled is inert (DDS 1.4 §2.2.2.1.1.6, §2.2.2.1.1.7): `get_status_changes` is empty, its
  listeners are not called, its ReadConditions and StatusCondition have `trigger_value` FALSE. `DomainParticipant`:
  `assert_liveliness` (§2.2.2.2.1.19); `try_get_discovered_participants`, `try_get_discovered_topics` and
  `try_contains_entity` return NOT_ENABLED before `enable`, as do `get_discovered_participant_data` and
  `get_discovered_topic_data`.
- `delete_contained_entities` of the participant also deletes ContentFilteredTopics and MultiTopics, and returns
  PRECONDITION_NOT_MET, with nothing deleted, while a GROUP publisher is inside a coherent set or a subscriber
  inside a group access (`Subscriber::delete_contained_entities` too).
- `RuntimeConfig::encapsulation_ids` selects the identifiers of the encapsulation header of a
  user sample: `Table60` (default, `{0x00, 0x06}` to `{0x00, 0x0b}` and `{0x00, 0x04}`, what Cyclone DDS,
  RTI Connext and Fast DDS write) or `Table39` (DDS-XTypes 1.3 §7.4.3.4: `{0x00, 0x10}` to
  `{0x00, 0x15}` and `{0x01, 0x00}`). A reader takes both; an XML sample under `{0x01, 0x00}` is read.
  Configuration key `wire.encapsulation_ids` (environment `ZERODDS_WIRE_ENCAPSULATION_IDS`); the PL_CDR1
  forms are `wire.pl_cdr1_headers` and `wire.pl_cdr1_reading` (`ZERODDS_WIRE_PL_CDR1_HEADERS`,
  `ZERODDS_WIRE_PL_CDR1_READING`).
- The specification dialect writes the exact length of a `@mutable` member in a PL_CDR1 parameter
  (XTypes 1.3 §7.4.1.2) and sets the must-understand bit on the discriminator of a `@mutable` union
  (§7.2.2.4.4.4.6). The rounded length of DDSI-RTPS 2.5 §9.4.2.11 is
  `zerodds_ast_layout::PlCdr1HeaderPolicy::ShortRoundedLength`; the vendor dialects keep what they were
  measured writing.
- The built-in Subscriber is a Subscriber (DDS 1.4 §2.2.5): `BuiltinSubscriber` derefs to its `Subscriber`, whose
  readers are the four built-in ones. `BuiltinSubscriber::lookup_datareader` and `BuiltinTopic::lookup` are
  gone; `lookup_datareader` returns `None` for an unknown name instead of `BadParameter`. The application
  cannot delete a built-in reader.
- `create_publisher_with_listener`, `create_subscriber_with_listener`, `create_topic_with_listener`,
  `create_datawriter_with_listener`, `create_datareader_with_listener` and
  `DomainParticipantFactory::create_participant_with_listener` take the listener and the mask (DDS 1.4
  §2.2.2.2.1.1, .3, .5, §2.2.2.2.2.1, §2.2.2.4.1.5, §2.2.2.5.2.5).
- `time::Duration::INFINITE` has `nanosec` `0x7fffffff` (DDS 1.4 §2.3.3: `DURATION_INFINITE_NSEC`); it
  was `0xffffffff`. This is the API value of `time::Duration`; the wire form (`zerodds_qos::Duration`,
  DDSI-RTPS §9.3.2) is unchanged and does not carry it. `From` converts between `zerodds_qos::Duration` and `time::Duration`.
- The store of a TRANSIENT or PERSISTENT topic follows the Topic's DURABILITY_SERVICE (DDS
  1.4 §2.2.3.5); a Topic at the default leaves it to the first writer, a writer that
  differs from a Topic that sets one is overridden and
  `writer.durability_service.overridden_by_topic` is recorded.
- The persistence service of a TRANSIENT or PERSISTENT topic (DDS 1.4 §2.2.3.4): the
  endpoint of the last application DataWriter stays as the fictitious DataWriter
  while the store holds samples, so a reader that comes later gets them; an
  endpoint whose QoS is incompatible with the Topic QoS changes its
  OFFERED/REQUESTED_INCOMPATIBLE_QOS status, and an incompatible writer does not
  feed the store; the `service_cleanup_delay` of an instance starts when it is
  disposed and no live writer of the participant writes it (several writers, a
  deleted writer, a writer that lost its liveliness). A VOLATILE writer on a
  TRANSIENT topic now reports DURABILITY as incompatible.
- The user-traffic unicast socket (UDPv4) binds the port
  `PB + DG * domainId + d3 + PG * participantId` of DDSI-RTPS 2.5 §9.6.2.3, with
  the participantId of the SPDP unicast socket, and announces it. Before, it took
  an ephemeral port. If that port is taken, the socket takes an ephemeral port as
  before; this is counted (`user_port_fallbacks`) and logged
  (`transport.user_unicast.port_taken`). Not yet run against the vendors.
- Every receive path (user data, SPDP, SEDP, WLP, TypeLookup, security builtins)
  runs on `receiver_walk_in`: the receiver state carries the transport kind and
  source address of the datagram and the reception clock,
  and a reader proxy with no locator takes the unicast reply list (InfoReply) of
  its ACKNACK's message as its locators.

### Fixed

- The entity key of a reserved endpoint id that is not registered and of the group
  id of a Publisher or Subscriber is held until it is given back
  (`DcpsRuntime::release_entity_key`), so the 24-bit counter does not hand it out
  again after it wraps (DDSI-RTPS 2.5 §8.2.4.4).
- The OWNERSHIP_STRENGTH and LIFESPAN a DATA carries in-line apply to that sample
  instead of the writer's announced values (§8.3.8.2).
- `Time::INVALID` and `Time::INFINITE` cross the wire as the `Time_t` values of
  DDSI-RTPS 2.5 §9.3.2.1 ({0xffffffff, 0xffffffff} and {0xffffffff,
  0xfffffffe}) and are read back as such.
- The `get_*_status` operations of a disabled DataWriter, DataReader and Topic return the
  untriggered status instead of `NotEnabled` (DDS 1.4 §2.2.2.1.1.7), as the C ABI did.
- `read_w_condition_selected` and `take_w_condition_selected`: `max_samples` after the
  `ORDER BY`, `NoData` for an empty result. The built-in Subscriber has the nil
  listener with all statuses (DDS 1.4 §2.2.5) and is attached to its participant.
- `set_listener(None, mask)` with a mask installs the 'nil' listener
  (`listener::NilListener`): the statuses of the mask are handled by doing
  nothing, do not bubble up and are not reset (DDS 1.4 §2.2.2.1.1.3, §2.2.4.2.1,
  §2.2.4.3.1). The listener traits have a defaulted `is_nil`.
- `ReadSelection` with `DataReader::read_selected`, `take_selected`,
  `read_instance_selected`, `take_instance_selected`, `read_next_instance_selected`
  and `take_next_instance_selected`: `max_samples`, the three state masks and
  `NoData` when nothing meets them (DDS 1.4 §2.2.2.5.3.8 to §2.2.2.5.3.17).
- `DataWriter::write_with_handle` and `write_w_timestamp_with_handle` take the
  `handle` of DDS 1.4 §2.2.2.4.2.11 and §2.2.2.4.2.12: HANDLE_NIL, or the handle of
  the key; another existing instance is `PreconditionNotMet`, none `BadParameter`.
- `lookup_topicdescription` finds ContentFilteredTopics and MultiTopics; a
  `TopicDescriptionHandle` has `kind()`, `topic::<T>()`,
  `content_filtered_topic::<T>()` and `multitopic::<T>()`. `find_topic` also
  waits for a Topic created locally meanwhile, takes enabled Topics only and
  counts as obtaining the Topic for `delete_topic` (DDS 1.4 §2.2.2.2.1.11,
  §2.2.2.2.1.12). `create_contentfilteredtopic` and `create_multitopic` want
  `T: 'static`.
- `get_key_value` of an instance known by its KeyHash reads the key holder from
  the 16 octets and requires zeros after it (DDSI-RTPS 2.5 §9.6.4.8 Step 5.1);
  a key holder shorter than 16 octets was refused for its padding.
- An entity has one `StatusCondition`; `get_entity` returns the entity
  (`EntityRef`), `ReadCondition::get_datareader` returns the reader, and
  `WaitSet::detach_condition` returns `PreconditionNotMet` for a condition that
  is not attached (DDS 1.4 §2.2.2.1.9, §2.2.2.5.8.1, §2.2.2.1.6.2).
- The Rust `DataReader` fills `sample_rank`, `generation_rank` and
  `absolute_generation_rank` (DDS 1.4 §2.2.2.5.1.6).
- The readers of the topics of a MultiTopic reader are not visible to the
  application; the members of a base structure are fields of a MultiTopic's
  topics; the view_state of a MultiTopic instance is NEW while a constituent
  is (DDS 1.4 §2.2.2.3.4).
- A write after an unregister makes the writer a writer of the instance again;
  the instance was left NOT_ALIVE_NO_WRITERS.
- `create_multitopic` reads the topics from the `FROM` clause of the
  subscription expression, which is a `TopicExpression` (DDS 1.4 Annex B.2),
  and checks it against the topics and the result type; its parameter
  `related_topic_names` is gone. `JoinedRow`, `hash_join_two` and
  `MultiTopic::evaluate_joined` are removed.
- A `QueryCondition` takes an `ORDER BY` (Annex B.2 `QueryExpression`).
- `get_key_value` returns `BadParameter` instead of `Unsupported` for an
  instance known by its hash alone (DDS 1.4 §2.2.2.4.2.9, §2.2.2.5.3.29).
- A topic without a key has one instance (DDS 1.4 §2.2.1.2.2): the writer
  registers, looks up, disposes and unregisters it, the reader gives it a
  handle, NEW at the first sample, NOT_ALIVE_DISPOSED and
  NOT_ALIVE_NO_WRITERS. HISTORY, RESOURCE_LIMITS, TIME_BASED_FILTER and
  DESTINATION_ORDER apply to it as to any instance.
- The TRANSIENT and PERSISTENT store belongs to the participant's topic and
  outlives its writer; it takes its sequence numbers from itself, applies
  `service_cleanup_delay` and drops expired samples (DDS 1.4 §2.2.3.4,
  §2.2.3.16). A store that refuses a sample is reported
  (`writer.durability.store_failed`).
- `get_datareaders` under GROUP with `ordered_access` orders the samples of
  one group by group sequence number.
- `DurabilityBackend::replay_for_topic` is provided by
  `replay_for_topic_at`; the trait has `next_sequence`;
  `DurabilitySample::expires_at` and `SampleWire::group` are new fields.
- `delete_contentfilteredtopic` and `delete_multitopic` delete the object for
  every handle (DDS 1.4 §2.2.2.2.1.8, §2.2.2.2.1.10): `PreconditionNotMet`
  while a DataReader created on it exists, `AlreadyDeleted` afterwards, and
  the use of the related topics ends, so `delete_topic` works. They returned
  `Ok(())` and changed nothing.
- `Time::new` and `Duration::new` carry whole seconds of `nanosec` into `sec`
  (DDS 1.4 §2.3.2); `Time::from_millis` and `Duration::from_millis` floor a
  negative value instead of producing a `nanosec` above a second.
- `create_publisher` and `create_subscriber` on a deleted participant return
  a deleted entity (DDS 1.4 §2.2.1.1).

### Added

- Observability event `rtps.decode.rejected` (kind, submessage, offset, vendor, reason, left_out)
  for a message or submessage the RTPS decoders refuse, cut short or skip; the first 32 events,
  then one a second. The sink of the first runtime in the process takes it.
- `DdsType::MAX_SERIALIZED_SIZE`, `UserWriterConfig::data_max_size_serialized`,
  `entity_name` of the writer and reader configs, and
  `RuntimeConfig::participant_name` (`discovery.participant_name`, env
  `ZERODDS_PARTICIPANT_NAME`): `dataMaxSizeSerialized` and the entity names go
  out in the announcements. BREAKING: `UserWriterConfig` and `UserReaderConfig`
  have new fields.
- `ContentFilter<T>`, `DomainParticipant::register_contentfilter`,
  `lookup_contentfilter`, `unregister_contentfilter` and
  `create_contentfilteredtopic_with_class`: filter classes of the application
  (DDSI-RTPS 2.5 §8.7.3.4). The class name is the `filterClassName` of the
  reader's `PID_CONTENT_FILTER_PROPERTY`.
- `RuntimeConfig::user_multicast` (`transport.user_multicast`, env
  `ZERODDS_USER_MULTICAST`, default false): joins the SPDP group on the
  user-traffic multicast port `PB + DG * domainId + d2` (DDSI-RTPS 2.5 §9.6.2.3)
  and announces it as the default multicast locator.
- `RuntimeConfig::clock_skew_threshold_s` (`wire.clock_skew_threshold_s`, env
  `ZERODDS_WIRE_CLOCK_SKEW_THRESHOLD_S`, default 0 = off): the threshold of
  `CLOCK_SKEW_DETECTED` (§8.3.7.4), which the spec does not give.
- `DcpsRuntime::user_port_fallbacks` and `reply_locator_fallbacks`.
- `RuntimeConfig::port_mapping` (`zerodds_rtps::wire_types::PortMapping`): the port
  parameters PB, DG, PG and d0 to d3 of DDSI-RTPS 2.5 §9.6.2.3 as settings
  `ports.port_base`, `ports.domain_gain`, `ports.participant_gain`, `ports.d0` to
  `ports.d3` (env `ZERODDS_PORT_BASE`, `ZERODDS_PORT_DOMAIN_GAIN`,
  `ZERODDS_PORT_PARTICIPANT_GAIN`, `ZERODDS_PORT_D0` to `ZERODDS_PORT_D3`).
- `DcpsRuntime::write_user_sample_to_readers` — a sample for the readers it
  names (by GUID) and no other: remote readers get a GAP, readers of the
  runtime are not handed it. DDS-RPC 1.0 §7.11.1.4.3: a request through a
  requester bound to a service instance goes to that instance only.
- `DcpsRuntime::unreadable_content_filters` and the event
  `sedp.content_filter.unreadable`: a subscription whose ContentFilterProperty
  did not read is taken without a filter and reported.
- `RuntimeConfig::writer_heartbeat_period`, `reader_heartbeat_response_delay`
  and `reader_heartbeat_suppression_duration` (`writer.heartbeat_period_ms`,
  `reader.heartbeat_response_delay_ms`, `reader.heartbeat_suppression_duration_ms`,
  env `ZERODDS_WRITER_HEARTBEAT_PERIOD_MS`,
  `ZERODDS_READER_HEARTBEAT_RESPONSE_DELAY_MS`,
  `ZERODDS_READER_HEARTBEAT_SUPPRESSION_DURATION_MS`). The defaults are the
  values of before.
- `RuntimeConfig::spdp_new_sequence_per_period`
  (`discovery.spdp_new_sequence_per_period`, env
  `ZERODDS_SPDP_NEW_SEQUENCE_PER_PERIOD`, default `false`).
- `RuntimeConfig::accept_empty_acknack_base_zero` (`wire.accept_empty_acknack_base_zero`,
  env `ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO`, default `false`) and
  `DcpsRuntime::refused_messages`, `invalid_message_rests`,
  `skipped_must_understand_submessages`.
- `Subscriber::create_datareader_with_multitopic`,
  `DomainParticipant::create_dynamic_multitopic`, `QueryCondition::order_by`
  and `compare_rows`: a DataReader on a MultiTopic joins the topics of its
  `FROM` clause (DDS 1.4 §2.2.2.3.4).
- `dcps.presentation_instance_coherence` (`per_instance` | `none`, env
  `ZERODDS_PRESENTATION_INSTANCE_COHERENCE`), `PresentationInstanceCoherence`:
  `none` presents the samples of a coherent set as they arrive under
  PRESENTATION `access_scope` INSTANCE (DDS 1.4 §2.2.3.6, p. 115); the default
  `per_instance` holds the set (the table on p. 106).
- `Time::is_normalized`, `Duration::is_normalized`,
  `DomainParticipant::try_create_publisher` and `try_create_subscriber`
  (`AlreadyDeleted` after `delete_participant`).
- `DcpsRuntime::user_writer_history_and_limits` and
  `user_reader_resource_limits` — HISTORY depth, RESOURCE_LIMITS and
  `max_blocking_time` an endpoint was registered with, to refuse a change of
  an immutable policy (DDS 1.4 §2.2.3).
- `DataWriter::next_sequence_number` and
  `DcpsRuntime::user_writer_next_sequence_number` — the RTPS sequence number
  the next sample of a user writer will carry. Used by `zerodds-rpc` to name
  the identity of a request sample before writing it.
- `DomainParticipant::create_topic_with_type_name` — a topic announced in
  SEDP under a type name the caller chooses. DDS-RPC 1.0 §7.5.1.1.6/7 name
  the topic types of an interface `${interfaceName}_Request` and `_Reply`.
- `DataWriter::write_related` — writes a sample with
  `PID_RELATED_SAMPLE_IDENTITY` in its inline QoS and returns the RTPS
  sequence number (DDS-RPC 1.0 §7.8.2).
- `DataWriter::set_rpc_discovery`, `DataReader::set_rpc_discovery`,
  `matched_rpc_readers`, `matched_rpc_writers`, `DataReader::rtps_guid`,
  `runtime::RpcDiscoveryExt` and `RpcPeer` — announce and read the DDS-RPC
  1.0 §7.6.2.1 discovery extension (`PID_SERVICE_INSTANCE_NAME` 0x0080,
  `PID_RELATED_ENTITY_GUID` 0x0081, `PID_TOPIC_ALIASES` 0x0082) of the
  endpoints of this participant and of the matched peers.
- `DdsType::encode_continuing` / `decode_continuing` — encode or decode a
  value that continues a sample at an offset, alignment counted from the
  sample start (DDS-RPC 1.0 §7.5.1.1: the `Call` union behind a header).
- Topic aliases (DDS-RPC 1.0 §7.4.3): a reader and a writer on different
  topics match when the topic of one is among the announced `topic_aliases`
  of the other, over the wire and inside one participant. Endpoints
  without aliases match as before.
- `DataReader::take_with_wire` and `sample_bytes::SampleWire` — `take` with
  the writer GUID, the RTPS sequence number and the related sample identity
  of each sample's DATA. `SampleBytes` carries them (`with_wire`, `wire`).
- `SerializedTypeSupport::read_body_prefix`: reads the first sample of a buffer that holds more behind it and returns how many octets it took (for transports that pack serialized samples with no length each, e.g. DDS-XRCE 1.0 Annex A `sequence<SampleData>`).

- `DynamicDataWriter` and `DynamicDataReader` (DDS-XTypes 1.3 §7.6.6.2):
  `FooDataWriter` and `FooDataReader` for "Foo" = `DynamicData`, that is
  `DataWriter<DynamicSample>` and `DataReader<DynamicSample>`, over a topic
  from `DomainParticipant::create_dynamic_topic`. The
  `SerializedSample` path (`create_serialized_topic`) stays as it is.

- `SerializedTypeSupport::register_type` (Table 65, DDS 1.4 §2.2.2.3.6):
  registers the type with a participant under its name or another
  (`with_type_name`); registering it again under a name is not an error,
  another type under a taken name is `PreconditionNotMet`.
  `DomainParticipant::create_registered_topic` creates a topic of a
  registered type by its name.

- The XML Data Representation on the wire (DDS-XTypes 1.3 §7.4.4,
  Table 60; DDSI-RTPS 2.5 Table 10.3): `xml_data::{to_xml, to_xml_key,
  from_xml}`, and a writer whose first `DataRepresentation` is XML (`XML`
  in `wire.data_representation_offer`, or `dr::XML` in the offer) sends the
  document of its sample under `{0x00, 0x04}`, as its restored samples and
  the `serializedKey` of a dispose or an unregister do. A reader turns a
  sample under that header into the XCDR2 body of the same value, against
  its registered type; a document that is none of the type's is refused
  with `reader.sample.encapsulation_refused`. The body handed to an XML
  writer is XCDR1, and the writer needs its type registered, otherwise the
  write fails. Before, XML was refused as the first element of an offer.
  `SerializedTypeSupport::write_key_only_data`.

- `SerializedTypeSupport::get_type`, `create_sample`, `create_dynamic_sample`,
  `create_type_support`, `delete_type_support` and `get_type_name`
  (DDS-XTypes 1.3 §7.6.6.1, Tables 63 to 65): the conversion between a
  sample and a `DynamicData` of its type, and a support for a `DynamicType`
  that holds a copy of the type and refuses a nested type and a type that
  is not a structure or a union.

- A topic type in a strongly connected component (DDS-XTypes 1.3 §7.3.4.9)
  announces `PID_TYPE_INFORMATION` (§7.6.3.2.1) and is served by the
  TypeLookup server as a whole: `DdsType::type_component`,
  `DcpsRuntime::register_type_component`, and a registry that files the
  components a `getTypes` reply carries. A recursive type announced no
  TypeInformation before, and matching fell back to the type name.
- A filter on a `SerializedSample` reaches the value of a map by its key
  (`m['key']`, and `m[3]` for an integer key, §7.6.7.1) and sees an absent
  optional member as `null`, not as a field that does not exist (§7.6.7.2).
- `builtin_types` (DDS-XTypes 1.3 §7.6.5, Annex E): `DdsString`,
  `DdsKeyedString`, `DdsBytes` and `DdsKeyedBytes` as topic types in their
  appendable XCDR2 and XCDR1 forms; `StringDataWriter`, `StringDataReader`
  and the other aliases of the type-specific interfaces; and the operations
  of `KeyedStringDataWriter`, `BytesDataWriter` and `KeyedBytesDataWriter`
  (`register_instance_w_key`, `write_string_w_key`, `write_w_bytes`,
  `write_bytes_w_key`, `dispose_w_key`, `get_key_value_w_key`,
  `lookup_instance_w_key` and their `_w_timestamp` forms) as
  `KeyedStringDataWriterOperations`, `BytesDataWriterOperations` and
  `KeyedBytesDataWriterOperations`.
- `DomainParticipant::lookup_topicdescription_at(name, index)`
  (DDS-XTypes 1.3 §7.6.4.2): iterates over the topics of one name in creation
  order and returns `None` outside the range; `lookup_topicdescription(name)`
  is index 0. `DcpsRuntime::dispose_local_topic_of_type`.
- `TopicQos::data_representation` (DDS-XTypes 1.3 §7.6.3.1.1): the first element
  applies to writers of the topic, the whole list to readers, copied by
  `Publisher::copy_from_topic_qos` and `Subscriber::copy_from_topic_qos`; it is
  immutable once the topic is enabled.
- `RuntimeConfig::absent_type_consistency`, the setting
  `xtypes.type_consistency_absent` (`ZERODDS_TYPE_CONSISTENCY_ABSENT`):
  `disallow`, the default, holds a remote reader that announces no
  TypeConsistencyEnforcement to DISALLOW_TYPE_COERCION (DDS-XTypes 1.3
  §7.6.3.4.1); `allow` takes the policy's default for it. A reader that
  announces DISALLOW itself keeps it.
- `psm_constants::qos_policy_name` (`DataRepresentation`,
  `TypeConsistencyEnforcement`) and `psm_constants::data_representation_id`
  (`XCDR` 0, `XML` 1, `XCDR2` 2), the constants of Annex D.

### Fixed

- Key material and crypto tokens exist for an endpoint whose submessages or
  payload are protected (DDS-Security 1.2 §9.4.2.7 Table 37); a user endpoint
  of a topic with neither gets none and no token goes out for it. A deleted
  endpoint and a participant that is gone release their keys
  (`unregister_local_endpoint`, `unregister_matched_remote_participant`) and
  the record of the tokens sent.

- A volatile secure message (key material, tokens) that the KEEP_ALL writer
  cannot take is no longer discarded: it is held back and counted
  (`volatile_secure_deferred`, event `security.volatile.write_deferred`), or
  counted and recorded when refused (`volatile_secure_failed`, event
  `security.volatile.write_failed`).

- Secure discovery and liveliness are chosen per topic (DDS-Security 1.2
  §7.5.1.7, §7.5.1.8, §7.5.2): an endpoint is announced on the secure SEDP
  topics when its topic rule has `enable_discovery_protection`, and the
  liveliness of a writer goes over the secure ParticipantMessage writer when
  its topic rule has `enable_liveliness_protection`; a participant with
  writers of both kinds beats on both writers, each with its own sequence
  numbers. `IS_LIVELINESS_PROTECTED` in the endpoint mask follows the same
  topic flag. The domain-wide `discovery_protection_kind` and
  `liveliness_protection_kind` only protect the secure endpoints. Before,
  both kinds switched every endpoint of the participant.

- Secure SPDP (DDS-Security 1.2 §7.5.1.6): a change of the participant data
  takes the next sequence number on the secure SPDP writer (every change was
  number 1); the departure of a secured participant is sent on that writer;
  it goes to the authenticated participants, not to the multicast group, and
  at once when a participant authenticates. After the authentication the
  plain SPDP of that participant only renews its lease and liveliness.

- `Publisher::set_qos` refuses a change of PARTITION on an enabled publisher
  that holds a DataWriter with a durability other than VOLATILE on a topic
  with `enable_read_access_control` (`ImmutablePolicy`; DDS-Security 1.2
  §7.4.5).

- Under `data_protection_kind` ENCRYPT the KeyHash of a sample or of a dispose or
  unregister is the MD5 of the key, whatever its size; a key of at most 16
  octets was its own KeyHash beside the ciphertext. A write whose key is
  unknown is refused with `NotAllowedBySecurity` (DDS-Security 1.2 §7.4.4).

- A user submessage or payload is decoded with a key of its sender only; the
  key derived from the SharedSecret (Kx) is tried for
  ParticipantVolatileMessageSecure alone (DDS-Security 1.2 §9.5.1.10.6).

- The WLP writer sends the history it holds to a new reader proxy at once, and
  serves the repairs asked for before it writes the next sample. With a runtime
  tick as long as the beat period, every request met a newer sample of the
  same kind (KEEP_LAST 1) and was answered with a GAP: the reader never heard
  from the writer, and a writer that wrote before it knew the reader never got
  in step with it.
- A participant that same-process discovery entered into the discovery cache
  gets its WLP reader proxy from the tick (`sync_wlp_readers`); only a beacon
  created it before, so WLP reached one direction between two runtimes of a
  process.
- `create_topic`, `create_topic_with_type_name`, `create_contentfilteredtopic`
  and `DcpsRuntime::start` refuse a topic, type, content-filtered topic or domain
  tag name longer than 256 octets (`string<256>`, DDSI-RTPS 2.5 Table 9.18).
- The participant's `manualLivelinessCount` goes up with every
  MANUAL_BY_PARTICIPANT sample and a new SPDP announcement follows (DDSI-RTPS
  2.5 Table 8.78); a receiver takes a changed count as that participant's
  manual assertion. The WLP writer treats a reader with
  `BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER` in its `builtinEndpointQos` as
  best effort (§8.4.13.3).
- WLP submessages whose INFO_DESTINATION names another participant are ignored
  and counted (`WlpEndpoint::misaddressed`, `DcpsRuntime::wlp_misaddressed`;
  DDSI-RTPS 2.5 §8.3.8.8). They were taken by whichever participant held the
  port.
- The WLP writer is a reliable writer (DDSI-RTPS 2.5 §8.4.13): one sample per
  instance (KEEP_LAST 1), a reader proxy for each participant that announces a
  BuiltinParticipantMessageReader, HEARTBEATs until acknowledged and repair of
  what a reader asks for. `WlpEndpoint::tick` returns the datagrams with
  their targets; `wlp_acknack_resends` is replaced by `handle_reader_messages`.
  `protect_wlp_outbound` protects the HEARTBEAT and GAP of the secure WLP writer
  as it protects its DATA.
- A write, `assert_liveliness`, dispose and unregister of a
  MANUAL_BY_PARTICIPANT writer send the participant's ParticipantMessageData
  (DDSI-RTPS 2.5 §8.7.2.2.3).
- The entityKey of a new endpoint, topic or group is no longer taken from the
  24-bit counter alone: once the counter has wrapped, a key that a registered
  writer, reader or announced topic still holds is skipped (DDSI-RTPS 2.5
  §9.3.1.2).
- The tail of a participant's GUID prefix is a process-wide counter added to
  a seed that the clock gives once per process (`next_process_unique`), so the
  participants of one process, the C API and the bridges included, never
  share a prefix (DDSI-RTPS 2.5 §8.2.4.2). It was the clock of each call
  added to the counter. `process_unique` takes the seed instead of the clock.
  `DcpsRuntime::prefix_collisions` counts an SPDP announcement that carries
  the participant's own prefix without being its own beacon.
- A user writer's fragment size no longer changes with the matched readers
  (DDSI-RTPS 2.5 §8.4.14.1); a same-host reader gets the sample as one DATA
  through the message limit of its reader proxy.
- A writer whose first representation was XML (`0x0004`) was given a
  `CDR_LE` header in front of a CDR body: it announced XML and sent CDR.
  `register_user_writer*` and `set_user_writer_data_rep_override` now refuse
  an offer that does not start with XCDR1 or XCDR2, and the framing of a
  sample returns `BadParameter` for any other representation instead of
  falling back to `CDR_LE` (DDS-XTypes 1.3 Table 39, §7.6.3.1.2).

- `create_topic` with a name that is in use and another type creates a second
  topic of that name when the QoS equals the first one's (DDS-XTypes 1.3
  §7.6.4.1: "multiple objects of the same name, provided that all of them
  represent type-based subsets of 'the same' network topic"); it returned
  `InconsistentPolicy`. The same type name with other type metadata, and a
  QoS that differs, are still `InconsistentPolicy`. The users of a topic are
  counted per name, so `delete_topic` of one of several same-named topics
  waits for the readers and writers of all of them.
- Filters on a `SerializedTypeSupport` sample reach a sequence or array
  element by `[n]` and a bitmask flag by `['NAME']` (DDS-XTypes 1.3
  §7.6.7.1). Map values are not reachable: the dynamic form does not mark
  which collections are maps.
- The four built-in types are `@extensibility(APPENDABLE)` and not `@nested`
  (DDS-XTypes 1.3 §7.6.5, Annex E); they were declared `@nested`.
  `BuiltinType::extensibility` reads the extensibility.
- The TypeLookup endpoints use HISTORY KEEP_ALL (DDS-XTypes 1.3 §7.6.3.3);
  they used KEEP_LAST 32, which replaced older requests and replies.
- The announced `TypeInformation` carries the dependent TypeIdentifiers of
  the topic type and their count (DDS-XTypes 1.3 §7.6.3.2.1); the count was 0.
- DataRepresentation matching follows DDS-XTypes 1.3 §7.6.3.1.1: the default
  match mode is `Strict` (a writer's offered representation must be in the
  reader's sequence; `ZERODDS_DATA_REPR_MATCH=tolerant` keeps the overlap rule
  for a peer that needs it), and an explicitly empty list of preferences is
  the single element XCDR instead of an error at `create_datawriter`.
- A payload whose encapsulation identifier is `{0x00, 0x10}` to `{0x00, 0x15}`
  (PLAIN_CDR2, PL_CDR2, DELIMIT_CDR2; DDS-XTypes 1.3 Table 39, DDSI-RTPS 2.5
  Table 10.3) is read; it was refused as an unknown scheme. Writing still
  uses the identifiers of Table 60.
- A user sample's serialized payload is padded with zero octets to a 4-byte
  offset and the count is written into the two least significant bits of the
  second options byte of the encapsulation header (DDS-XTypes 1.3
  §7.6.3.1.2). The options were always 0.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-dcps` crate.

### Spec references

- **OMG DDS 1.4 §2.2** — complete DCPS module: entity hierarchy, all 22 QoS policies, built-in topics, ContentFilteredTopic, MultiTopic, Conditions/WaitSet, listener bubble-up.
- **DDSI-RTPS 2.5 §8.5** — SPDP/SEDP discovery, WLP (§8.4.13), inline QoS (§9.6.4.8 PID_KEY_HASH + §9.6.3.9 PID_STATUS_INFO), HEADER_EXTENSION checksum wiring.
- **OMG XTypes 1.3 §7.6.3** — TypeLookup service endpoints (§7.6.3.3.4), TypeIdentifier-aware subscriber match (§7.6.3.7), TypeConsistencyEnforcement filter (DDS 1.4 §2.2.3 default).
- **OMG DDS-Security 1.2** — the opt-in `security` feature spans the `SharedSecurityGate` over the UDP hot path (plugin runtime in `zerodds-security-runtime`).

### Public-API

**Factory + Participant:**
- `DomainParticipantFactory` — singleton (`instance()`), `create_participant`/`create_participant_offline`/`create_participant_with_config`, `lookup_participant`, `delete_participant`, `set_default_participant_qos`/`get_default_participant_qos`, `DomainParticipantFactoryQos`.
- `DomainParticipant` — `create_publisher`/`create_subscriber`/`create_topic`, `register_type_object`, `enqueue_type_lookup`, `on_remote_publication_discovered`/`on_remote_subscription_discovered`, `ignore_participant`/`ignore_topic`/`ignore_publication`/`ignore_subscription`, `get_discovered_*`, `delete_contained_entities`, `IgnoreFilter`.
- `DomainId` — spec-conformant domain id (i32).

**Entity hierarchy:**
- `Publisher` / `DataWriter<T>` — `create_datawriter`, `lookup_datawriter`, `write`/`write_w_timestamp`, `register_instance`/`unregister_instance`/`dispose`, `get_key_value`, `wait_for_acknowledgments`, `assert_liveliness`, Begin-/End-Coherent-Changes.
- `Subscriber` / `DataReader<T>` — `create_datareader`, `lookup_datareader`, `read`/`take`/`read_w_condition`/`take_w_condition`, `read_next_instance`/`take_next_instance`, `get_matched_publications`, `set_filter_expression`, `set_listener`.
- `Topic<T>` / `TopicDescription` / `TopicDescriptionHandle` / `ContentFilteredTopic` / `MultiTopic` / `JoinedRow` / `hash_join_two`.

**Built-in topics (DDS 1.4 §2.2.5):**
- `BuiltinSubscriber` / `BuiltinSinks` / `BuiltinTopic` / `builtin_reader_qos`.
- `DcpsParticipantBuiltinTopicData` (alias `ParticipantBuiltinTopicData`).
- `DcpsTopicBuiltinTopicData` (alias `TopicBuiltinTopicData`) incl. `synthesize_key`.
- `DcpsPublicationBuiltinTopicData` / `DcpsSubscriptionBuiltinTopicData`.
- Topic-name constants `TOPIC_NAME_DCPS_PARTICIPANT`/`_TOPIC`/`_PUBLICATION`/`_SUBSCRIPTION`.

**Conditions/WaitSet:**
- `Condition`, `ReadCondition`, `QueryCondition`, `GuardCondition`, `StatusCondition`, `WaitSet`.
- Status-mask helpers (`StatusMask`, `immutable_if_enabled`).

**QoS:**
- `DomainParticipantQos`, `PublisherQos`, `SubscriberQos`, `TopicQos`, `DataWriterQos`, `DataReaderQos`.
- Re-exports of the 22 policies from `zerodds-qos` (Durability, Reliability, History, ResourceLimits, Liveliness, Deadline, Latency, Lifespan, Ownership/OwnershipStrength, Partition, Presentation, DurabilityService, TimeBasedFilter, DestinationOrder, EntityFactory, ReaderDataLifecycle, WriterDataLifecycle, UserData, TopicData, GroupData, TransportPriority, TypeConsistencyEnforcement).

**Sample + Lifecycle:**
- `Sample`, `SampleInfo`, `SampleStateKind`/`ViewStateKind`/`InstanceStateKind`, `*_state_mask`-Helpers.
- `InstanceTracker`, `InstanceState`, `InstanceHandle`, `InstanceHandleAllocator`, `HANDLE_NIL`, `KeyHash`.
- `CoherentScope`, `CoherentSetMarker`, `GroupAccessScope`.

**Type system:**
- `DdsType` trait + `DdsTypeRow` (RowAccess adapter for SQL filters), `RawBytes` (key-less topic type), `DecodeError`/`EncodeError`.

**Time:**
- `Time`, `Duration`, `get_current_time`.

**Error:**
- `DdsError` (all spec return codes), `Result<T>`.

### Implementation

The live-mode runtime (`DcpsRuntime`) binds three UDP sockets per participant (SPDP multicast receiver, SPDP unicast fallback, user unicast) and spawns an event-loop thread. The loop sends periodic SPDP beacons, polls all sockets non-blocking, dispatches SEDP pub/sub announces into the `DiscoveredEndpointsCache`, pushes user data into the matching DataReader slots, runs the WLP tick (heartbeat every `lease_duration / 3`), serves the TypeLookup service endpoints (reliable writer/reader on the `TL_SVC_REQ`/`TL_SVC_REPLY` GUIDs), and executes the per-slot mutex architecture for hot-path delivery. With the `security` feature active, every outbound/inbound byte runs through the `SharedSecurityGate` (DDS-Security 1.2 §8.5.1.9). Multi-interface bindings (`InterfaceBindingSpec`) enable per-subnet routing.

Subscriber matching applies TypeIdentifier-aware compatibility checks (XTypes 1.3 §7.6.3.7) as soon as both sides announce a `TYPE_IDENTIFIER`. A mismatch bumps `requested_incompatible_qos.last_policy_id = TYPE_CONSISTENCY_ENFORCEMENT`. The default path remains the plain `type_name` comparison (DDS 1.4 §2.2.3). Cross-vendor interop paths against Cyclone DDS and Fast DDS are verified in the `cyclone_live_*` test modules (with the `live-interop` feature).

The exclusive-ownership filter (DDS 1.4 §2.2.3.23) is fully wired cross-layer: per-sample `writer_guid` + `ownership_strength` are passed from the RTPS reader to the DCPS slot, and the subscriber consults the `instance_tracker::should_accept_sample_under_exclusive_ownership` logic per sample. Built-in topics use shared ownership (filter inactive). The durability backend (transient in-memory + persistent on-disk via `OnDiskDurabilityBackend`) stores topic + instance key + monotonic writer sequence per sample, so late joiners receive an ordered replay.

`forbid(unsafe_code)` (exception: the opt-in `flatdata-integration` with per-block `SAFETY` comments).

### Architecture

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-foundation`, `-cdr`, `-qos`, `-types`, `-rtps`, `-transport`, `-transport-udp`, `-discovery`, `-sql-filter`. Optional: `-security-runtime` (feature `security`), `-flatdata` (feature `flatdata-integration`), `-inspect-endpoint` (embargo feature `inspect`).
- **Dependents (out):** `zerodds-dcps-async`, all bridges (`amqp-bridge`, `coap-bridge`, `grpc-bridge`, `mqtt-bridge`, `websocket-bridge`, `zenoh-bridge`), the PSM crates (`cpp`, `cs`, `java`, `py`, `rs`, `ts-node`, `ts-wasm`, `zerodds-c-api`, `zerodds-java-jni`), profile crates (`conformance`, `dlrl`, `xrce`, `web`, `ros2-rmw`, `rmw-zerodds-shim`, `opcua-gateway`, `zerodds-soap`).
- **Feature flags:** see the table in `README.md`.

### Stability

All `pub` items are RC1-stable; breaking changes require a major bump to `2.0.0`. Doc-hidden test hooks (`__push_raw`, `__drain_pending`, `__push_raw_with_writer`) are internal test API and not stable. The embargo feature `inspect` is not included in the public mirror and is repurposed with the PDE release.
