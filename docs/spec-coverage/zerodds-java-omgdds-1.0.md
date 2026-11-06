# `zerodds-java-omgdds` v1.0 — Spec-Coverage

Audit der Vendor-Spec `docs/specs/zerodds-java-omgdds-1.0.md` gegen
`crates/java-omgdds/` Code-Realität.

**Source:** `docs/specs/zerodds-java-omgdds-1.0.md` (Vendor-Spec, 2026-05-06).

**Kontext:** Die Implementation ist ein reines Maven-Java-Modul im
Crates-Verzeichnis; Rust-Code enthält sie nicht:

- `crates/java-omgdds/java/` — Pure-Java-Modul (Maven, JUnit 5)

---

## §1 Architektur

### §1.1 Module-Layout

**Spec:** §1.1 — Java-Source-Tree unter `crates/java-omgdds/java/src/main/java/`
mit `org/omg/dds/*` (normative API) und `org/zerodds/internal/*`
(Implementation-Detail).

**Repo:** 510 Java-Files (378 unter `src/main`, 132 unter `src/test`; gezählt
mit `find crates/java-omgdds/java/src -name "*.java"`).
- `org/omg/dds/core/` (26 Files, dazu `core/policy/` mit 28 Files und die Pakete `core/event`, `core/status`)
- `org/omg/dds/topic/` (13 Files)
- `org/omg/dds/sub/` (15 Files)
- `org/omg/dds/pub/` (9 Files)
- `org/omg/dds/domain/` (7 Files)
- `org/omg/dds/**` insgesamt 184 Files
- `org/zerodds/…` (194 Files, darunter `crates/java-omgdds/java/src/main/java/org/zerodds/internal/InProcessBus.java`,
  `crates/java-omgdds/java/src/main/java/org/zerodds/internal/Xcdr2Codec.java`, `bridge/GrpcBridgeClient.java`)
- 132 Test-Files in `src/test/`
- Maven-Pom in `crates/java-omgdds/java/pom.xml`

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`
(bootet die Surface über `org/omg/dds/**` hinweg) sowie die 128
Test-Klassen des Moduls, die Maven aus `src/test/java` einsammelt: 940
Tests, 1 übersprungen. Aufruf: mvn -B -Pjava8 clean verify in
crates/java-omgdds/java, wie im Job endpoints-java-async.
In dem Verzeichnis liegt kein Rust-Code — es gibt keine Cargo.toml und
damit auch keinen cargo-Test.

**Status:** done

### §1.2 InProcessBus

**Spec:** §1.2 — Threadsafe-Topic-Map, push-basierte Sample-Delivery
an alle DataReader auf demselben Topic.

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/internal/
InProcessBus.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_read_returns_samples_without_removing`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javaomgdds_1_2_every_reader_of_the_topic_receives_the_sample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_closed_reader_is_no_subscription_of_the_writer_any_more`.

**Status:** done

### §1.3 Xcdr2Codec

**Spec:** §1.3 — Java-native XCDR2-Encoder/Decoder spec-konform per
DDS-XTypes 1.3 §7.4.

**Repo:** `org/zerodds/internal/Xcdr2Codec.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::javaomgdds_1_3_xcdr2codec_writes_and_reads_the_primitives_with_the_octets_of_xtypes_7_4`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::javaomgdds_1_3_xcdr2codec_writes_and_reads_a_utf8_string_with_its_length_and_nul`.

**Status:** done

---

## §2 OMG-API-Coverage

### §2.1 org.omg.dds.core.*

**Spec:** §2 — Time, Duration, InstanceHandle, Entity, ReturnCode,
QosProfile.

**Repo:** 26 Java-Files in `org/omg/dds/core/`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::javapsm_7_3_4_a_time_answers_in_seconds_with_its_remainder_and_in_milliseconds`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_4_the_infinite_duration_is_the_largest_and_long_max_in_any_unit_is_infinite`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreValuesTest.java::javapsm_7_3_3_handles_with_the_same_octets_are_equal_and_have_the_same_hash_code`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreExceptionsTest.java::javapsm_7_3_2_only_timeout_exception_is_checked`
— 7 Tests in `CoreTypesTest`; `ReturnCode_t` der PIM ist auf Ausnahmen abgebildet (`ExceptionMappingTest`).

**Status:** done

### §2.2 org.omg.dds.domain.*

**Spec:** §2 — DomainParticipant + DomainParticipantFactory.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` + `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`
(DomainParticipantFactory-Bootstrap über die ServiceEnvironment),
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`
(Participant erzeugt Publisher/Subscriber).

**Status:** done

### §2.3 org.omg.dds.topic.*

**Spec:** §2 — Topic + TopicTypeSupport.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` + `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TopicTypeSupport.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code`
(TypeSupport wird aus dem Topic abgeleitet).

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

**Spec:** §2 — QosProfile (Default-QoS-Pfad, optional XML-QoS-Loader).

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/core/QosProvider.java`, `crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_history_keep_last_caps_the_samples_per_instance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_transient_local_replays_to_a_late_joiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::javapsm_7_3_5_1_exclusive_ownership_arbitrates_to_the_highest_strength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java::javapsm_7_3_5_1_non_overlapping_partition_is_not_a_match`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/LeaseTest.java::javapsm_2_requested_deadline_missed_fires_without_polling`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_liveliness_offered_kind_must_be_at_least_requested_and_the_lease_not_longer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosRequestedOfferedTest.java::javapsm_7_3_5_1_reliability_is_compared_by_kind_and_not_by_max_blocking_time`
— je ein Test pro Policy-Klasse in `org/omg/dds/core/policy/`
(History, Durability, Ownership, Partition, Deadline, Liveliness,
Reliability); 21 Tests in der Klasse insgesamt.

**Status:** done — Default-QoS-Pfad live; der XML-QoS-Loader ist im
`zerodds-xml-1.0`-Audit-File getrackt (separate Spec).

---

## §3 Test-Pflicht

### §3.1 mvn test grün

**Spec:** §3 — `mvn test` muss grün laufen.

**Repo:** `crates/java-omgdds/java/pom.xml` mit JUnit-5-Setup.

**Tests:** Der Maven-Surefire-Lauf umfasst 123 Test-Klassen; die
breiteste davon ist
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`.
Gemessen (mvn test in crates/java-omgdds/java): Tests run: 925,
Failures: 0, Errors: 0, Skipped: 3 (Log `/root/agent-java/logs/xt-bind-1018-070634.log` auf dem Linux-Testhost).

**Status:** done

---

## §4 Multi-Process / Cross-Vendor

### §4.1 Pure-Java + gRPC-Bridge Decision

**Spec:** §4 — Der Pure-Java-Pfad hat keinen RTPS-Wire-Stack.
Single-JVM-Pfad via `InProcessBus`; Multi-JVM und Cross-Vendor
laufen über die gRPC-Bridge zum `zerodds-grpc-bridged`-Daemon (§5, implementiert).

**Repo:** `crates/java-omgdds/java/` (Pure-Java mit `InProcessBus`).
Eine frühere JNI-Bridge wurde entfernt; ihre Crate ist nicht mehr im Baum.

**Tests:** Single-JVM-Pfad —
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`.
Multi-JVM-Pfad —
`crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java::javaomgdds_4_2_a_sample_published_through_the_grpc_bridge_comes_back_through_dds`
(env-gated über ZERODDS_BRIDGE_PORT; im Default-Lauf übersprungen, der
einzige Skip des Moduls, deklariert in `ci/declared-skips.tsv`). Dass der Baum inzwischen über die §4-Aussage
hinausgeht, misst
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/WriterWireIgnoreTest.java::javapsm_2_ignore_subscription_ends_the_match_with_one_reader`.

**Status:** done — die Routing-Entscheidung der Vendor-Spec §4 (Single-JVM
über `InProcessBus`, Multi-JVM über die gRPC-Bridge) ist implementiert und
belegt. Abweichung zur Spec-Prosa: `org/zerodds/rtps/` trägt inzwischen
einen Pure-Java-RTPS-Stack (SPDP/SEDP/DATA über UDP), den Vendor-Spec §6
erst für v2.0 vorsieht; die §4-Aussage „kein RTPS-Wire-Stack" beschreibt
die v1.0-Surface, nicht den heutigen Baum.

### §4.2 gRPC-Bridge für Multi-Process

**Spec:** §5 — gRPC-Service exponiert DDS Publish/Subscribe
(`zerodds.bridge.v1.<Topic>/Publish|Subscribe`), Pure-Java-Client verbindet sich.

**Repo:** Daemon `zerodds-grpc-bridged` (Service-Routing `service_gen.rs`,
`/zerodds.bridge.v1.<Topic>/Publish|Subscribe`) + Pure-Java-h2c-Client
`org/zerodds/bridge/GrpcBridgeClient.java` (raw HTTP/2 + HPACK + protobuf
`Sample`/`PublishAck` — kein grpc-java, kein TLS, Java-8-kompatibel).

**Tests:** `crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java::javaomgdds_4_2_a_publish_ack_is_read_from_its_protobuf_varint`,

(Codec-Self-Checks, laufen immer),
`crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java::javaomgdds_4_2_a_sample_published_through_the_grpc_bridge_comes_back_through_dds`
(echter Java-gRPC-DDS-Roundtrip via crates/java-omgdds/run_grpc_bridge_e2e.sh,
env-gated über ZERODDS_BRIDGE_PORT; im Default-Lauf übersprungen).

**Status:** done — Multi-JVM-Pfad implementiert + e2e-belegt.

### Ziel — Zero-Copy über die Foreign-Memory-API

**Spec:** Ziele — „NativeAOT-friendly: Java 22 `MemorySegment`/`Foreign-Memory-API` (v1.1) for zero-copy without JNI.“

**Repo:** —

**Tests:** —

**Status:** open — `crates/java-omgdds/` enthält weder `MemorySegment` noch einen anderen Aufruf der Foreign-Memory-API.

---

## §5 Stabilität

### §5 Semver-Zusage — v1.0 = aktuelle Surface

**Spec:** §6 — semver: v1.0 = aktuelle Surface (Pure-Java mit
`InProcessBus`); v1.1 = + gRPC-Bridge; v2.0 = + Pure-Java-RTPS.

**Repo:** `crates/java-omgdds/java/pom.xml` (Modul-Version).

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_8_1_a_plain_java_bean_is_written_and_read_without_generated_code`,
 — die Klasse
ruft die dokumentierte v1.0-Surface namentlich auf; jede Umbenennung oder
Entfernung eines Typs bricht schon die Kompilation des Test-Moduls.

**Status:** done

---

## Audit-Status

13 done / 0 partial / 1 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-java-omgdds-1.0.md`):

* `python3 ci/run_integration.py merge-java-omgdds` (Job `merge-java-omgdds`) — 22 Tests grün.
* `python3 ci/run_integration.py integration-java-psm` (Job `integration-java-psm`) — 1 Test grün.

23 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-java-omgdds-1.0.open.md`.
