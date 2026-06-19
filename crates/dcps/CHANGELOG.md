# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

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
