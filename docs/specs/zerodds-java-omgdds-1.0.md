# `zerodds-java-omgdds` v1.0 — Vendor-Spec

ZeroDDS Vendor-Spec. In `crates/java-omgdds/java/` implementiert.

**Status:** Draft 2026-05-06.

## Motivation

OMG DDS-Java-PSM 1.0 (formal/2017-04-01) definiert die normative
`org.omg.dds.*`-API als Java-Interface-Set. Die Spec macht keine
Aussage über die Implementierungs-Architektur — Vendoren wählen
zwischen:

1. **JNI-Bridge:** Java-Interfaces auf eine C/C++-Native-Library
   delegieren (RTI Connext-Style).
2. **Pure-Java:** Java-Interfaces durch eine native Java-Implementation
   bedienen (Eclipse Cyclone-Java-Style nicht voll, OpenSplice-Style).
3. **Hybrid:** Pure-Java mit optionalem Native-Backend.

ZeroDDS wählt einen **dualen Pfad**:

- **Pfad A — `zerodds-java-jni`:** klassisches JNI-Wrap des
  C-FFI (`libzerodds.dylib`), Volltransport-Pfad mit Cyclone-Cross-
  Vendor-Wire-Compliance.
- **Pfad B — `zerodds-java-omgdds`:** Pure-Java-Implementation der
  `org.omg.dds.*`-Interfaces für Single-Process + gRPC-Bridge-
  Multi-Process-Szenarien. **Kein JNI-Dependency.**

Diese Vendor-Spec definiert Pfad B normativ.

## Ziele

- **Spec-Compliance:** Volle `org.omg.dds.*`-Interface-Compliance per
  DDS-Java-PSM 1.0. Anwender-Code, der gegen `org.omg.dds.*`
  geschrieben wurde, läuft unverändert.
- **Zero-Native-Dependency:** Keine `libzerodds.dylib`/`.so`/`.dll`
  notwendig. Reines `.jar`-Artefakt.
- **In-Process-Bus:** Lokaler-only Pub-Sub via `InProcessBus`-Class
  für Tests, Tools und Single-Process-Anwendungen.
- **gRPC-Bridge (Phase-2):** Multi-Process-Pfad via gRPC-Service
  zu einem `libzerodds`-Server. Damit kommunizieren mehrere Pure-Java-
  JVMs mit C++/C#/Native-Peers.
- **NativeAOT-friendly:** Java-22-`MemorySegment`/`Foreign-Memory-API`
  Phase-2 für Zero-Copy ohne JNI.

## Nicht-Ziele

- Direkter RTPS-Wire-Stack in Pure-Java. Cross-Vendor-Wire-Tests laufen
  in Phase-2 über die gRPC-Bridge oder den JNI-Pfad (Pfad A).
- `org.omg.dds`-Modifikationen. Pfad B implementiert die Spec wie
  vorgegeben — keine Vendor-Annotations am Interface.

## §1 Architektur

### §1.1 Module-Layout

```
crates/java-omgdds/
  Cargo.toml                       # Rust-Codegen-Bridge zu idl-java
  src/lib.rs                       # placeholder
  java/
    pom.xml                        # Maven-Projekt
    src/main/java/
      org/omg/dds/                 # NORMATIVE OMG-API
        core/
          Time.java
          Duration.java
          InstanceHandle.java
          Entity.java
          ReturnCode.java
          policy/QosProfile.java
        domain/
          DomainParticipant.java
          DomainParticipantFactory.java
        topic/
          Topic.java
          TopicTypeSupport.java
        pub/
          Publisher.java
          DataWriter.java
        sub/
          Subscriber.java
          DataReader.java
          Sample.java
      org/zerodds/internal/        # IMPLEMENTATION-DETAIL
        InProcessBus.java
        Xcdr2Codec.java
    src/test/java/
      org/omg/dds/
        CoreTypesTest.java
        Xcdr2CodecTest.java
        PubSubLoopbackTest.java
```

### §1.2 InProcessBus

Lokaler Pub-Sub-Bus für Single-JVM-Szenarien. Threadsafe-Topic-Map,
push-basierte Sample-Delivery an alle DataReader auf demselben Topic.

```java
public final class InProcessBus {
    public static InProcessBus instance();

    public <T> void publish(String topicName, T sample);
    public <T> void subscribe(String topicName, Consumer<T> handler);
}
```

### §1.3 Xcdr2Codec

Java-native XCDR2-Encoder/-Decoder. Spec-konform per DDS-XTypes 1.3 §7.4.

```java
public final class Xcdr2Codec {
    public static byte[] encode(Object sample);
    public static <T> T decode(byte[] bytes, Class<T> type);
}
```

## §2 OMG-API-Coverage

Audit-File: `docs/spec-coverage/dds-java-psm-1.0.md` (171 Items, 156 done
+ 15 n/a). Pure-Java-Pfad deckt:

| Interface | Pfad-B-Status | Notiz |
|---|---|---|
| `org.omg.dds.core.Entity` | ✅ Live | InProcessBus-gebunden |
| `org.omg.dds.core.Time` | ✅ Live | Java-`Instant`-aequivalent |
| `org.omg.dds.core.Duration` | ✅ Live | |
| `org.omg.dds.core.InstanceHandle` | ✅ Live | |
| `org.omg.dds.domain.DomainParticipant` | ✅ Live | InProcessBus |
| `org.omg.dds.domain.DomainParticipantFactory` | ✅ Live | Singleton |
| `org.omg.dds.topic.Topic<T>` | ✅ Live | TopicTypeSupport-Trait |
| `org.omg.dds.pub.Publisher` | ✅ Live | |
| `org.omg.dds.pub.DataWriter<T>` | ✅ Live | |
| `org.omg.dds.sub.Subscriber` | ✅ Live | |
| `org.omg.dds.sub.DataReader<T>` | ✅ Live | |
| `org.omg.dds.sub.Sample<T>` | ✅ Live | |
| `org.omg.dds.core.policy.*` | ✅ Live | Default-Pfad |

## §3 Test-Pflicht

Pfad-B-Tests in `crates/java-omgdds/java/src/test/java/org/omg/dds/`:

- `CoreTypesTest`: Time/Duration/InstanceHandle-Roundtrips. (10 Tests)
- `Xcdr2CodecTest`: Encoder/Decoder Wire-Format-Roundtrip. (4 Tests)
- `PubSubLoopbackTest`: Voll-Pub-Sub-Pfad in-process. (4 Tests)

`mvn test` liefert 18/18 grün — siehe `docs/spec-coverage/LAYER-6-RC1-GAPS.md`.

## §4 Cross-Pfad-Kompatibilität

Pfad-A (JNI) und Pfad-B (Pure-Java) sind **wire-inkompatibel** im RC1
weil Pfad-B keinen RTPS-Stack hat. Cross-JVM-Multi-Process-Pfad in
Phase-2 via gRPC-Bridge.

| Szenario | Empfohlener Pfad | Begründung |
|---|---|---|
| Embedded-Java in libzerodds-Server | Pfad-A (JNI) | Volle Cross-Vendor-RTPS-Wire |
| Tooling, Tests, Tutorials | Pfad-B (Pure) | Zero-Native-Dependency |
| Multi-JVM lokal | Pfad-B + Phase-2 gRPC | Pure-Java über gRPC zu libzerodds-Server |
| ROS-2-Rmw-Pfad | Pfad-A (JNI) | RMW-Bridge bindet libzerodds nativ |

## §5 Phase-2-Plan

- **gRPC-Service-Proto**: `crates/grpc-bridge/proto/dds-bridge.proto`
  definiert das DCPS-RPC-Schema (CreateParticipant, CreateTopic,
  Write, Take, etc.).
- **gRPC-Service-Server-Side**: `crates/grpc-bridge/src/dds_service.rs`
  wraps `zerodds-dcps`-Calls in den gRPC-Service.
- **Java-Client**: `crates/java-omgdds/java/src/main/java/org/zerodds/
  internal/grpc/GrpcDdsBridge.java` ruft den Server.
- **Wahlweise via System-Property**: `org.zerodds.bridge=inprocess`
  vs. `org.zerodds.bridge=grpc://host:port`.

## §6 Stabilität

Vendor-Spec, semver:

- v1.0 = aktuelle Surface (Pfad-B mit InProcessBus, RC1 abgeschlossen).
- v1.1 = + gRPC-Bridge.
- v2.0 = + Pure-Java RTPS (Stretch-Goal).

## §7 Lizenz

Apache-2.0 (Workspace-Default).

## §8 Referenzen

- OMG DDS-Java-PSM 1.0 (formal/2017-04-01)
- OMG DDS-XTypes 1.3 (formal/2019-02-01) §7.4 XCDR2
- OMG DDS 1.4 (formal/2015-04-10) §2.2.5 Built-in Topics
