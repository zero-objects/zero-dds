# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-dcps`-Crate.

### Spec-Referenzen

- **OMG DDS 1.4 §2.2** — komplettes DCPS-Modul: Entity-Hierarchie, alle 22 QoS-Policies, Built-in-Topics, ContentFilteredTopic, MultiTopic, Conditions/WaitSet, Listener-Bubble-Up.
- **DDSI-RTPS 2.5 §8.5** — SPDP-/SEDP-Discovery, WLP (§8.4.13), Inline-QoS (§9.6.4.8 PID_KEY_HASH + §9.6.3.9 PID_STATUS_INFO), HEADER_EXTENSION-Checksum-Wiring.
- **OMG XTypes 1.3 §7.6.3** — TypeLookup-Service-Endpoints (§7.6.3.3.4), TypeIdentifier-aware Subscriber-Match (§7.6.3.7), TypeConsistencyEnforcement-Filter (DDS 1.4 §2.2.3 Default).
- **OMG DDS-Security 1.2** — opt-in `security`-Feature spannt das `SharedSecurityGate` ueber den UDP-Hot-Path (Plugin-Runtime in `zerodds-security-runtime`).

### Public-API

**Factory + Participant:**
- `DomainParticipantFactory` — Singleton (`instance()`), `create_participant`/`create_participant_offline`/`create_participant_with_config`, `lookup_participant`, `delete_participant`, `set_default_participant_qos`/`get_default_participant_qos`, `DomainParticipantFactoryQos`.
- `DomainParticipant` — `create_publisher`/`create_subscriber`/`create_topic`, `register_type_object`, `enqueue_type_lookup`, `on_remote_publication_discovered`/`on_remote_subscription_discovered`, `ignore_participant`/`ignore_topic`/`ignore_publication`/`ignore_subscription`, `get_discovered_*`, `delete_contained_entities`, `IgnoreFilter`.
- `DomainId` — Spec-konforme Domain-Id (i32).

**Entity-Hierarchie:**
- `Publisher` / `DataWriter<T>` — `create_datawriter`, `lookup_datawriter`, `write`/`write_w_timestamp`, `register_instance`/`unregister_instance`/`dispose`, `get_key_value`, `wait_for_acknowledgments`, `assert_liveliness`, Begin-/End-Coherent-Changes.
- `Subscriber` / `DataReader<T>` — `create_datareader`, `lookup_datareader`, `read`/`take`/`read_w_condition`/`take_w_condition`, `read_next_instance`/`take_next_instance`, `get_matched_publications`, `set_filter_expression`, `set_listener`.
- `Topic<T>` / `TopicDescription` / `TopicDescriptionHandle` / `ContentFilteredTopic` / `MultiTopic` / `JoinedRow` / `hash_join_two`.

**Built-in-Topics (DDS 1.4 §2.2.5):**
- `BuiltinSubscriber` / `BuiltinSinks` / `BuiltinTopic` / `builtin_reader_qos`.
- `DcpsParticipantBuiltinTopicData` (alias `ParticipantBuiltinTopicData`).
- `DcpsTopicBuiltinTopicData` (alias `TopicBuiltinTopicData`) inkl. `synthesize_key`.
- `DcpsPublicationBuiltinTopicData` / `DcpsSubscriptionBuiltinTopicData`.
- Topic-Namen-Konstanten `TOPIC_NAME_DCPS_PARTICIPANT`/`_TOPIC`/`_PUBLICATION`/`_SUBSCRIPTION`.

**Conditions/WaitSet:**
- `Condition`, `ReadCondition`, `QueryCondition`, `GuardCondition`, `StatusCondition`, `WaitSet`.
- Status-Mask-Helpers (`StatusMask`, `immutable_if_enabled`).

**QoS:**
- `DomainParticipantQos`, `PublisherQos`, `SubscriberQos`, `TopicQos`, `DataWriterQos`, `DataReaderQos`.
- Re-Exports der 22 Policies aus `zerodds-qos` (Durability, Reliability, History, ResourceLimits, Liveliness, Deadline, Latency, Lifespan, Ownership/OwnershipStrength, Partition, Presentation, DurabilityService, TimeBasedFilter, DestinationOrder, EntityFactory, ReaderDataLifecycle, WriterDataLifecycle, UserData, TopicData, GroupData, TransportPriority, TypeConsistencyEnforcement).

**Sample + Lifecycle:**
- `Sample`, `SampleInfo`, `SampleStateKind`/`ViewStateKind`/`InstanceStateKind`, `*_state_mask`-Helpers.
- `InstanceTracker`, `InstanceState`, `InstanceHandle`, `InstanceHandleAllocator`, `HANDLE_NIL`, `KeyHash`.
- `CoherentScope`, `CoherentSetMarker`, `GroupAccessScope`.

**Type-System:**
- `DdsType`-Trait + `DdsTypeRow` (RowAccess-Adapter fuer SQL-Filter), `RawBytes` (key-less Topic-Type), `DecodeError`/`EncodeError`.

**Time:**
- `Time`, `Duration`, `get_current_time`.

**Error:**
- `DdsError` (alle Spec-Returnkodes), `Result<T>`.

### Implementierung

Live-Mode-Runtime (`DcpsRuntime`) bindet drei UDP-Sockets pro Participant (SPDP-Multicast-Receiver, SPDP-Unicast-Fallback, User-Unicast) und spawnt einen Event-Loop-Thread. Der Loop sendet periodische SPDP-Beacons, pollt alle Sockets non-blocking, dispatched SEDP-Pub/Sub-Announces in den `DiscoveredEndpointsCache`, schiebt User-Daten in die passenden DataReader-Slots, fuehrt den WLP-Tick (Heartbeat alle `lease_duration / 3`), bedient die TypeLookup-Service-Endpoints (Reliable-Writer/Reader auf `TL_SVC_REQ`/`TL_SVC_REPLY`-GUIDs), und exekutiert die per-Slot-Mutex-Architektur fuer die Hot-Path-Lieferung. Mit aktivem `security`-Feature laeuft jeder Outbound-/Inbound-Byte durch das `SharedSecurityGate` (DDS-Security 1.2 §8.5.1.9). Multi-Interface-Bindings (`InterfaceBindingSpec`) ermoeglichen Per-Subnet-Routing.

Das Subscriber-Match wendet TypeIdentifier-aware Compatibility-Checks (XTypes 1.3 §7.6.3.7) an, sobald beide Seiten einen `TYPE_IDENTIFIER` annoncen. Mismatch bumpt `requested_incompatible_qos.last_policy_id = TYPE_CONSISTENCY_ENFORCEMENT`. Default-Path bleibt der reine `type_name`-Vergleich (DDS 1.4 §2.2.3). Cross-Vendor-Interop-Pfade gegen Cyclone-DDS und Fast-DDS sind in den `cyclone_live_*`-Test-Modulen verifiziert (mit `live-interop`-Feature).

Exclusive-Ownership-Filter (DDS 1.4 §2.2.3.23) ist Cross-Layer voll verdrahtet: per-Sample `writer_guid` + `ownership_strength` werden vom RTPS-Reader an den DCPS-Slot durchgereicht, der Subscriber konsultiert pro Sample die `instance_tracker::should_accept_sample_under_exclusive_ownership`-Logik. Builtin-Topics nutzen Shared-Ownership (Filter inactive). Der Durability-Backend (Transient In-Memory + Persistent On-Disk via `OnDiskDurabilityBackend`) speichert pro Sample Topic + Instance-Key + monotone Writer-Sequenz, sodass Late-Joiner ordered Replay erhalten.

`forbid(unsafe_code)` (Ausnahme: opt-in `flatdata-integration` mit per-Block-`SAFETY`-Kommentaren).

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-foundation`, `-cdr`, `-qos`, `-types`, `-rtps`, `-transport`, `-transport-udp`, `-discovery`, `-sql-filter`. Optional: `-security-runtime` (Feature `security`), `-flatdata` (Feature `flatdata-integration`), `-inspect-endpoint` (Embargo-Feature `inspect`).
- **Dependents (out):** `zerodds-dcps-async`, alle Bridges (`amqp-bridge`, `coap-bridge`, `grpc-bridge`, `mqtt-bridge`, `websocket-bridge`, `zenoh-bridge`), die PSM-Crates (`cpp`, `cs`, `java`, `py`, `rs`, `ts-node`, `ts-wasm`, `zerodds-c-api`, `zerodds-java-jni`), Profile-Crates (`conformance`, `dlrl`, `xrce`, `web`, `ros2-rmw`, `rmw-zerodds-shim`, `opcua-gateway`, `zerodds-soap`).
- **Feature-Flags:** siehe Tabelle in `README.md`.

### Stabilitaet

Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump auf `2.0.0`. Doc-hidden Test-Hooks (`__push_raw`, `__drain_pending`, `__push_raw_with_writer`) sind interne Test-API und nicht stabil. Embargo-Feature `inspect` ist nicht im Public-Mirror enthalten und wird mit dem PDE-Release umfunktioniert.
