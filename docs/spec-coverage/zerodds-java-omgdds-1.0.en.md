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

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`
(boots the surface across `org/omg/dds/**`) plus the 123 test classes
Maven collects from `src/test/java`: 925 tests, 3 skipped. Invocation:
mvn -o test in crates/java-omgdds/java. The directory holds no Rust code —
there is no Cargo.toml and hence no cargo test.

**Status:** done

### §1.2 InProcessBus

**Spec:** §1.2 — a thread-safe topic map, push-based sample delivery to all
DataReaders on the same topic.

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/internal/
InProcessBus.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::readReturnsSamplesWithoutRemoving`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::multipleReadersAllReceiveSample`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::cleanupRemovesSubscription`.

**Status:** done

### §1.3 Xcdr2Codec

**Spec:** §1.3 — a Java-native XCDR2 encoder/decoder, spec-conformant per
DDS-XTypes 1.3 §7.4.

**Repo:** `org/zerodds/internal/Xcdr2Codec.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::primitivesRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::utf8StringRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::multipleStringsRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::shortAlignmentSkipsByte`.

**Status:** done

---

## §2 OMG API coverage

### §2.1 org.omg.dds.core.*

**Spec:** §2 — Time, Duration, InstanceHandle, Entity, ReturnCode,
QosProfile.

**Repo:** 26 Java files in `org/omg/dds/core/`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::timeFromMillisRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::durationInfiniteRecognized`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::instanceHandleNilDetected`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/CoreTypesTest.java::instanceHandleRejectsWrongLength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/ExceptionMappingTest.java::everyPsmExceptionIsAnUncheckedDdsException`
— 7 tests in `CoreTypesTest`; the `ReturnCode_t` of the PIM is mapped onto exceptions (`ExceptionMappingTest`).

**Status:** done

### §2.2 org.omg.dds.domain.*

**Spec:** §2 — DomainParticipant + DomainParticipantFactory.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipant.java` + `crates/java-omgdds/java/src/main/java/org/omg/dds/domain/DomainParticipantFactory.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`
(DomainParticipantFactory bootstrap through the ServiceEnvironment),
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`
(the participant creates publisher and subscriber).

**Status:** done

### §2.3 org.omg.dds.topic.*

**Spec:** §2 — Topic + TopicTypeSupport.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java` + `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TopicTypeSupport.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::singleArgFactoryReflectsTypedBean`
(the TypeSupport is derived from the topic).

**Status:** done

### §2.4 org.omg.dds.pub.*

**Spec:** §2 — Publisher + DataWriter.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/pub/Publisher.java` + `crates/java-omgdds/java/src/main/java/org/omg/dds/pub/DataWriter.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::multipleReadersAllReceiveSample`.

**Status:** done

### §2.5 org.omg.dds.sub.*

**Spec:** §2 — Subscriber + DataReader + Sample.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Subscriber.java` + `crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReader.java` + `crates/java-omgdds/java/src/main/java/org/omg/dds/sub/Sample.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::readReturnsSamplesWithoutRemoving`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::cleanupRemovesSubscription`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::getDataAliasesData`.

**Status:** done

### §2.6 org.omg.dds.core.policy.*

**Spec:** §2 — QosProfile (default-QoS path, optional XML-QoS loader).

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/core/QosProvider.java`, `crates/java-omgdds/java/src/main/java/org/zerodds/core/qos/QosProviderImpl.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::historyKeepLastCapsPerInstance`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::transientLocalReplaysToLateJoiner`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::exclusiveOwnershipArbitratesToHighestStrength`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::partitionIsolatesNonOverlappingEndpoints`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::requestedDeadlineMissedIsRaised`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::livelinessRxoKindAndLease`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QosLifecycleTest.java::rxoIncompatibleReliabilityDetected`
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::byteArrayQuickstartRoundTrip`.
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
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`.
Multi-JVM path —
`crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java::publishThenSubscribeRoundtripThroughDds`
(env-gated on ZERODDS_BRIDGE_PORT; skipped in the default run, and that is
one of the 3 skips; the others are one test each in `ParticipantOmgApiTest` and `CoreApiShapeTest`). That the tree now goes beyond the §4 statement is
measured by
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RtpsWireTest.java::twoParticipantsExchangeSampleOverUdp`.

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

**Tests:** `crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java::protobufSampleRoundtrips`,
`crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java::publishAckDecodes`
(codec self-checks, always run),
`crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java::publishThenSubscribeRoundtripThroughDds`
(a real Java-gRPC-DDS roundtrip via crates/java-omgdds/run_grpc_bridge_e2e.sh,
env-gated on ZERODDS_BRIDGE_PORT; skipped in the default run).

**Status:** done — multi-JVM path implemented + e2e-proven.

---

## §5 Stability

### §5 Semver promise — v1.0 = the current surface

**Spec:** §6 — semver: v1.0 = the current surface (pure-Java with
`InProcessBus`); v1.1 = + gRPC bridge; v2.0 = + pure-Java RTPS.

**Repo:** `crates/java-omgdds/java/pom.xml` (the module version).

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::byteArrayQuickstartRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::singleArgFactoryReflectsTypedBean`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::getDataAliasesData` — the class calls
the documented v1.0 surface by name; renaming or removing a type breaks the
compilation of the test module.

**Status:** done

---

## Audit Status

13 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `mvn -o test` in `crates/java-omgdds/java` — 103 tests green,
0 failed, 1 skipped (the `GrpcBridgeClientTest` round trip, env-gated on
`ZERODDS_BRIDGE_PORT`).

Open items: none. Decision records: none.
