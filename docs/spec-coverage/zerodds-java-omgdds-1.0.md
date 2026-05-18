# `zerodds-java-omgdds` v1.0 — Spec-Coverage

Audit der Vendor-Spec `docs/specs/zerodds-java-omgdds-1.0.md` gegen
`crates/java-omgdds/` Code-Realität.

**Source:** `docs/specs/zerodds-java-omgdds-1.0.md` (Vendor-Spec, 2026-05-06).
**Repo:** `crates/java-omgdds/` (Rust-Codegen-Bridge) +
`crates/java-omgdds/java/` (Maven-Java-Modul).
**Stand:** 2026-05-07.

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

**Tests:** `CoreTypesTest` — 10 Tests fuer Time/Duration/InstanceHandle
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

**Status:** done — Default-QoS-Pfad live; XML-QoS-Loader ist Phase-2
in `zerodds-xml-1.0`-Audit-File getrackt (separate Spec).

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

**Spec:** §4 — Pure-Java hat im RC1 keinen RTPS-Wire-Stack.
Single-JVM-Pfad via `InProcessBus`; Multi-JVM und Cross-Vendor
kommen Phase-2 via gRPC-Bridge zu `libzerodds`-Server.

**Repo:** `crates/java-omgdds/java/` (Pure-Java mit `InProcessBus`).
Eine fruehere JNI-Bridge (`crates/zerodds-java-jni/`) wurde am
2026-05-07 (Commit `49b9b4c6`) entfernt.

**Status:** done — Decision-Matrix in Vendor-Spec §4 Tabelle.

### §4.2 gRPC-Bridge fuer Multi-Process

**Spec:** §5 Phase-2 — gRPC-Service in `crates/grpc-bridge/`
exponiert DCPS-Methods, Pure-Java-Client kann sich verbinden.

**Repo:** `crates/grpc-bridge/` existiert. `proto/dds-bridge.proto`
ist nicht implementiert.

**Status:** `n/a (Phase-2 Stretch)` — Vendor-Spec markiert das
explizit als Phase-2/v1.1-Stretch. RC1 lebt ohne. Decision-
Record im `.open.md`.

---

## §5 Stabilitaet

**Spec:** §6 — semver: v1.0 = aktuelle Surface (RC1).

**Repo:** Maven-Pom-Version, Cargo.toml-Version.

**Status:** done

---

## Zusammenfassung

| Sektion | Status |
|---|---|
| §1 Architektur | done (3/3) |
| §2 OMG-API-Coverage | done (6/6) |
| §3 Test-Pflicht | done |
| §4 Cross-Pfad | done (1/2) + n/a (1/2) |
| §5 Stabilitaet | done |

**Total Items:** 12
**done:** 11
**partial:** 0
**open:** 0
**n/a (Stretch):** 1

Siehe `zerodds-java-omgdds-1.0.open.md`.

## Abschluss-Bemerkung

Pure-Java-Pfad ist voll spec-konform fuer den dokumentierten Scope
(Single-JVM in-process Pub-Sub via InProcessBus + Cross-JVM via gRPC-
Bridge in v1.1). Der einzige `n/a`-Item (gRPC-Bridge) ist als
explizites Phase-2-Stretch im Vendor-Spec §5 markiert; RC1 ohne ist
korrekt.
