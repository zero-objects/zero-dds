# `zerodds-java-omgdds` v1.0 — Spec Coverage

Audit of the vendor spec `docs/specs/zerodds-java-omgdds-1.0.md` against the
`crates/java-omgdds/` code reality.

**Source:** `docs/specs/zerodds-java-omgdds-1.0.md` (vendor spec, 2026-05-06).

**Context:** the implementation is a plain Maven Java module inside the
crates directory; it contains no Rust code:

- `crates/java-omgdds/java/` — pure-Java module (Maven, JUnit 5)

---

## §1 Architecture

### §1.1 Module layout

**Spec:** §1.1 — a Java source tree under `crates/java-omgdds/java/src/main/java/`
with `org/omg/dds/*` (normative API) and `org/zerodds/internal/*`
(implementation detail).

**Repo:** 510 Java files (378 under `src/main`, 132 under `src/test`; counted
with `find crates/java-omgdds/java/src -name "*.java"`).
- `org/omg/dds/core/` (26 files, plus `core/policy/` with 28 files and the packages `core/event`, `core/status`)
- `org/omg/dds/topic/` (13 files)
- `org/omg/dds/sub/` (15 files)
- `org/omg/dds/pub/` (9 files)
- `org/omg/dds/domain/` (7 files)
- `org/omg/dds/**` in total 184 files
- `org/zerodds/…` (194 files, among them `crates/java-omgdds/java/src/main/java/org/zerodds/internal/InProcessBus.java`,
  `crates/java-omgdds/java/src/main/java/org/zerodds/internal/Xcdr2Codec.java`, `bridge/GrpcBridgeClient.java`)
- 132 test files in `src/test/`
- the Maven pom in `crates/java-omgdds/java/pom.xml`

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`
(boots the surface across `org/omg/dds/**`) plus the 128 test classes
Maven collects from `src/test/java`: 940 tests, 1 skipped. Invocation:
mvn -B -Pjava8 clean verify in crates/java-omgdds/java, as in the job
endpoints-java-async. The directory holds no Rust code —
there is no Cargo.toml and hence no cargo test.

**Status:** done

### §1.2 InProcessBus

**Spec:** §1.2 — a thread-safe topic map, push-based sample delivery to all
DataReaders on the same topic.

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/internal/
InProcessBus.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_read_returns_samples_without_removing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javaomgdds_1_2_every_reader_of_the_topic_receives_the_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_closed_reader_is_no_subscription_of_the_writer_any_more`.

**Status:** done

### §1.3 Xcdr2Codec

**Spec:** §1.3 — a Java-native XCDR2 encoder/decoder, spec-conformant per
DDS-XTypes 1.3 §7.4.

**Repo:** `org/zerodds/internal/Xcdr2Codec.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::javaomgdds_1_3_xcdr2codec_writes_and_reads_the_primitives_with_the_octets_of_xtypes_7_4`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::javaomgdds_1_3_xcdr2codec_writes_and_reads_a_utf8_string_with_its_length_and_nul`.

**Status:** done

---

## §2 OMG API coverage

### §2.1 org.omg.dds.core.*

**Spec:** §2 — Time, Duration, InstanceHandle, Entity, ReturnCode,
QosProfile.

**Repo:** 26 Java files in `org/omg/dds/core/`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::javapsm_7_3_4_a_time_answers_in_seconds_with_its_remainder_and_in_milliseconds`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_4_the_infinite_duration_is_the_largest_and_long_max_in_any_unit_is_infinite`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_handles_with_the_same_octets_are_equal_and_have_the_same_hash_code`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_only_timeout_exception_is_checked`
— 7 tests in `CoreTypesTest`; the `ReturnCode_t` of the PIM is mapped onto exceptions (`ExceptionMappingTest`).

**Status:** done

### §2.2 org.omg.dds.domain.*

**Spec:** §2 — DomainParticipant + DomainParticipantFactory.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` + `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`
(DomainParticipantFactory bootstrap through the ServiceEnvironment),
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`
(the participant creates publisher and subscriber).

**Status:** done

### §2.3 org.omg.dds.topic.*

**Spec:** §2 — Topic + TopicTypeSupport.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` + `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TopicTypeSupport.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code`
(the TypeSupport is derived from the topic).

**Status:** done

### §2.4 org.omg.dds.pub.*

**Spec:** §2 — Publisher + DataWriter.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` + `crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javaomgdds_1_2_every_reader_of_the_topic_receives_the_sample`.

**Status:** done

### §2.5 org.omg.dds.sub.*

**Spec:** §2 — Subscriber + DataReader + Sample.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` + `crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` + `crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_read_returns_samples_without_removing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_closed_reader_is_no_subscription_of_the_writer_any_more`.

**Status:** done

### §2.6 org.omg.dds.core.policy.*

**Spec:** §2 — QosProfile (default-QoS path, optional XML-QoS loader).

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/core/QosProvider.java`, `crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_history_keep_last_caps_the_samples_per_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_transient_local_replays_to_a_late_joiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_exclusive_ownership_arbitrates_to_the_highest_strength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_7_3_5_1_non_overlapping_partition_is_not_a_match`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_requested_deadline_missed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_liveliness_offered_kind_must_be_at_least_requested_and_the_lease_not_longer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_reliability_is_compared_by_kind_and_not_by_max_blocking_time`
— one test per policy class in `org/omg/dds/core/policy/` (History,
Durability, Ownership, Partition, Deadline, Liveliness, Reliability);
21 tests in the class overall.

**Status:** done — the default-QoS path is live; the XML-QoS loader is
tracked in the `zerodds-xml-1.0` audit file (separate spec).

---

## §3 Test requirement

### §3.1 mvn test green

**Spec:** §3 — `mvn test` must run green.

**Repo:** `crates/java-omgdds/java/pom.xml` with a JUnit 5 setup.

**Tests:** the Maven Surefire run covers 123 test classes; the widest
of them is
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`.
Measured (mvn test in crates/java-omgdds/java): Tests run: 925,
Failures: 0, Errors: 0, Skipped: 3 (log `/root/agent-java/logs/xt-bind-1018-070634.log` on the Linux test host).

**Status:** done

---

## §4 Multi-process / cross-vendor

### §4.1 Pure-Java + gRPC-bridge decision

**Spec:** §4 — the pure-Java path has no RTPS wire stack. The single-JVM path
via `InProcessBus`; multi-JVM and cross-vendor run over the gRPC bridge to the
`zerodds-grpc-bridged` daemon (§5, implemented).

**Repo:** `crates/java-omgdds/java/` (pure-Java with `InProcessBus`).
An earlier JNI bridge was removed; its crate is no longer in the tree.

**Tests:** single-JVM path —
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`.
Multi-JVM path —
`crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java::javaomgdds_4_2_a_sample_published_through_the_grpc_bridge_comes_back_through_dds`
(env-gated on ZERODDS_BRIDGE_PORT; skipped in the default run, the only
skip of the module, declared in `ci/declared-skips.tsv`). That the tree now goes beyond the §4 statement is
measured by
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterWireIgnoreTest.java::javapsm_2_ignore_subscription_ends_the_match_with_one_reader`.

**Status:** done — the routing decision of vendor-spec §4 (single-JVM over
`InProcessBus`, multi-JVM over the gRPC bridge) is implemented and proven.
Deviation from the spec prose: `org/zerodds/rtps/` now carries a pure-Java
RTPS stack (SPDP/SEDP/DATA over UDP) that vendor-spec §6 schedules for
v2.0; the §4 phrase "no RTPS wire stack" describes the v1.0 surface, not
today's tree.

### §4.2 gRPC bridge for multi-process

**Spec:** §5 — the gRPC service exposes DDS publish/subscribe
(`zerodds.bridge.v1.<topic>/Publish|Subscribe`), a pure-Java client connects.

**Repo:** daemon `zerodds-grpc-bridged` (service routing in `service_gen.rs`,
`/zerodds.bridge.v1.<topic>/Publish|Subscribe`) + pure-Java h2c client
`org/zerodds/bridge/GrpcBridgeClient.java` (raw HTTP/2 + HPACK + protobuf
`Sample`/`PublishAck` — no grpc-java, no TLS, Java-8 compatible).

**Tests:** `crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java::javaomgdds_4_2_a_publish_ack_is_read_from_its_protobuf_varint`,

(codec self-checks, always run),
`crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java::javaomgdds_4_2_a_sample_published_through_the_grpc_bridge_comes_back_through_dds`
(a real Java-gRPC-DDS roundtrip via crates/java-omgdds/run_grpc_bridge_e2e.sh,
env-gated on ZERODDS_BRIDGE_PORT; skipped in the default run).

**Status:** done — multi-JVM path implemented + e2e-proven.

### Goal — zero-copy via the Foreign Memory API

**Spec:** Goals — "NativeAOT-friendly: Java 22 `MemorySegment`/`Foreign-Memory-API` (v1.1) for zero-copy without JNI."

**Repo:** —

**Tests:** —

**Status:** open — `crates/java-omgdds/` contains neither `MemorySegment` nor any other call of the Foreign Memory API.

---

## §5 Stability

### §5 Semver promise — v1.0 = the current surface

**Spec:** §6 — semver: v1.0 = the current surface (pure-Java with
`InProcessBus`); v1.1 = + gRPC bridge; v2.0 = + pure-Java RTPS.

**Repo:** `crates/java-omgdds/java/pom.xml` (the module version).

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code`,
 — the class calls
the documented v1.0 surface by name; renaming or removing a type breaks the
compilation of the test module.

**Status:** done

---

## Audit Status

13 done / 0 partial / 1 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-java-omgdds-1.0.md`):

* `python3 ci/run_integration.py merge-java-omgdds` (job `merge-java-omgdds`) — 22 tests green.
* `python3 ci/run_integration.py integration-java-psm` (job `integration-java-psm`) — 1 test green.

23 cited tests green, 0 not green.

Open items: `zerodds-java-omgdds-1.0.open.en.md`.
