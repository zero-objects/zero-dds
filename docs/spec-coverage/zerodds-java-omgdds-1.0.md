# `zerodds-java-omgdds` v1.0 — Spec-Coverage

Audit der Vendor-Spec `docs/specs/zerodds-java-omgdds-1.0.md` gegen
`crates/java-omgdds/` Code-Realität.

**Source:** `docs/specs/zerodds-java-omgdds-1.0.md` (Vendor-Spec, 2026-05-06).

**Kontext:** Die Implementation lebt in einer Crate (mit eingebettetem Maven-Java-Modul):

- `crates/java-omgdds/` — Rust-Codegen-Bridge + Pure-Java-Modul unter `java/` (Maven)

---

## §1 Architektur

### §1.1 Module-Layout

**Spec:** §1.1 — Java-Source-Tree unter `crates/java-omgdds/java/src/main/java/`
mit `org/omg/dds/*` (normative API) und `org/zerodds/internal/*`
(Implementation-Detail).

**Repo:** 23 Java-Files (siehe `find crates/java-omgdds/java -name "*.java"`).
- `org/omg/dds/core/` (5 Files)
- `org/omg/dds/topic/` (2 Files)
- `org/omg/dds/sub/` (3 Files)
- `org/omg/dds/pub/` (2 Files)
- `org/omg/dds/domain/` (2 Files)
- `org/omg/dds/core/policy/` (1 File)
- `org/zerodds/internal/InProcessBus.java` (1 File)
- `org/zerodds/internal/Xcdr2Codec.java` (1 File)
- 3 Test-Files in `src/test/`
- Maven-Pom in `crates/java-omgdds/java/pom.xml`

**Tests:** `mvn test` 18 passed (CoreTypesTest 10, Xcdr2CodecTest 4,
PubSubLoopbackTest 4) + 1 cargo-test in `crates/java-omgdds`.

**Status:** done

### §1.2 InProcessBus

**Spec:** §1.2 — Threadsafe-Topic-Map, push-basierte Sample-Delivery
an alle DataReader auf demselben Topic.

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/internal/
InProcessBus.java`.

**Tests:** `PubSubLoopbackTest` — 4 Tests verifizieren Pub→Sub
delivery in-process.

**Status:** done

### §1.3 Xcdr2Codec

**Spec:** §1.3 — Java-native XCDR2-Encoder/Decoder spec-konform per
DDS-XTypes 1.3 §7.4.

**Repo:** `org/zerodds/internal/Xcdr2Codec.java`.

**Tests:** `Xcdr2CodecTest` — 4 Tests verifizieren wire-format
Roundtrip.

**Status:** done

---

## §2 OMG-API-Coverage

### §2.1 org.omg.dds.core.*

**Spec:** §2 — Time, Duration, InstanceHandle, Entity, ReturnCode,
QosProfile.

**Repo:** 5+ Java-Files in `org/omg/dds/core/`.

**Tests:** `CoreTypesTest` — 10 Tests für Time/Duration/InstanceHandle
Roundtrips.

**Status:** done

### §2.2 org.omg.dds.domain.*

**Spec:** §2 — DomainParticipant + DomainParticipantFactory.

**Repo:** `DomainParticipant.java` + `DomainParticipantFactory.java`.

**Tests:** indirekt via `PubSubLoopbackTest`.

**Status:** done

### §2.3 org.omg.dds.topic.*

**Spec:** §2 — Topic + TopicTypeSupport.

**Repo:** `Topic.java` + `TopicTypeSupport.java`.

**Tests:** `PubSubLoopbackTest`.

**Status:** done

### §2.4 org.omg.dds.pub.*

**Spec:** §2 — Publisher + DataWriter.

**Repo:** `Publisher.java` + `DataWriter.java`.

**Tests:** `PubSubLoopbackTest`.

**Status:** done

### §2.5 org.omg.dds.sub.*

**Spec:** §2 — Subscriber + DataReader + Sample.

**Repo:** `Subscriber.java` + `DataReader.java` + `Sample.java`.

**Tests:** `PubSubLoopbackTest`.

**Status:** done

### §2.6 org.omg.dds.core.policy.*

**Spec:** §2 — QosProfile (Default-QoS-Pfad, optional XML-QoS-Loader).

**Repo:** `QosProfile.java`.

**Tests:** indirekt.

**Status:** done — Default-QoS-Pfad live; der XML-QoS-Loader ist im
`zerodds-xml-1.0`-Audit-File getrackt (separate Spec).

---

## §3 Test-Pflicht

### §3.1 mvn test grün

**Spec:** §3 — `mvn test` muss 18/18 grün liefern.

**Repo:** `crates/java-omgdds/java/pom.xml` mit JUnit-5-Setup.

**Tests:** Verifikation: `mvn test` in `crates/java-omgdds/java/`
liefert "Tests run: 18, Failures: 0, Errors: 0, Skipped: 0".

**Status:** done

---

## §4 Multi-Process / Cross-Vendor

### §4.1 Pure-Java + gRPC-Bridge Decision

**Spec:** §4 — Der Pure-Java-Pfad hat keinen RTPS-Wire-Stack.
Single-JVM-Pfad via `InProcessBus`; Multi-JVM und Cross-Vendor
laufen über die gRPC-Bridge zum `zerodds-grpc-bridged`-Daemon (§5, implementiert).

**Repo:** `crates/java-omgdds/java/` (Pure-Java mit `InProcessBus`).
Eine frühere JNI-Bridge (`crates/zerodds-java-jni/`) wurde am entfernt.

**Status:** done — Decision-Matrix in Vendor-Spec §4 Tabelle.

### §4.2 gRPC-Bridge für Multi-Process

**Spec:** §5 — gRPC-Service exponiert DDS Publish/Subscribe
(`zerodds.bridge.v1.<Topic>/Publish|Subscribe`), Pure-Java-Client verbindet sich.

**Repo:** Daemon `zerodds-grpc-bridged` (Service-Routing `service_gen.rs`,
`/zerodds.bridge.v1.<Topic>/Publish|Subscribe`) + Pure-Java-h2c-Client
`org/zerodds/bridge/GrpcBridgeClient.java` (raw HTTP/2 + HPACK + protobuf
`Sample`/`PublishAck` — kein grpc-java, kein TLS, Java-8-kompatibel).

**Tests:** `GrpcBridgeClientTest` — Protobuf-Codec-Self-Checks (immer grün) +
`publishThenSubscribeRoundtripThroughDds` (echtes Java→gRPC→DDS-DataWriter→
DataReader→gRPC→Java-Roundtrip, via `run_grpc_bridge_e2e.sh`, env-gated über
`ZERODDS_BRIDGE_PORT`).

**Status:** done — Multi-JVM-Pfad implementiert + e2e-belegt.

---

## §5 Stabilität

**Spec:** §6 — semver: v1.0 = aktuelle Surface.

**Repo:** Maven-Pom-Version, Cargo.toml-Version.

**Status:** done

---

## Zusammenfassung

| Sektion | Status |
|---|---|
| §1 Architektur | done (3/3) |
| §2 OMG-API-Coverage | done (6/6) |
| §3 Test-Pflicht | done |
| §4 Cross-Pfad | done (2/2) |
| §5 Stabilität | done |

**Total Items:** 12
**done:** 12
**partial:** 0
**open:** 0
**n/a (Stretch):** 0

## Zusammenfassung

Der Pure-Java-Pfad ist voll spec-konform: Single-JVM in-process Pub-Sub via
`InProcessBus` **und** Multi-JVM/Cross-Process über die gRPC-Bridge
(`zerodds-grpc-bridged` ↔ Pure-Java-h2c-`GrpcBridgeClient`, e2e-belegt). Kein
JNI im Pure-Java-Pfad. Keine offenen oder zurückgestellten Items mehr.
