# `zerodds-java-omgdds` v1.0 — vendor spec

ZeroDDS vendor spec. Implemented in `crates/java-omgdds/java/`.

**Status:** Draft 2026-05-06.

## Motivation

OMG DDS-Java-PSM 1.0 (formal/2017-04-01) defines the normative
`org.omg.dds.*` API as a set of Java interfaces. The spec makes no
statement about the implementation architecture — vendors choose
between:

1. **JNI bridge:** delegate the Java interfaces to a C/C++ native library
   (RTI Connext style).
2. **Pure Java:** serve the Java interfaces with a native Java implementation
   (OpenSplice style).
3. **Hybrid:** pure Java with an optional native backend.

ZeroDDS chooses **path 2 (pure Java)** as its only implementation path:

- **`zerodds-java-omgdds`** — pure Java implementation of the
  `org.omg.dds.*` interfaces for single-process + (v1.1)
  gRPC bridge multi-process scenarios. **No JNI dependency, no
  `libzerodds` native lib in the Java path.**

An earlier JNI bridge is no longer used; its crate has been removed from the
tree. The IDL→Java codegen is today the Java column in
`crates/lang-rules/src/adapter/java.rs` and
`crates/lang-rules/src/emit/java.rs` with the runtime annotations in
`crates/idl-java/runtime/`; the runtime of this spec is
`crates/java-omgdds/`.

This vendor spec defines the pure Java implementation normatively.

## Goals

- **Spec compliance:** full `org.omg.dds.*` interface compliance per
  DDS-Java-PSM 1.0. User code written against `org.omg.dds.*`
  runs unchanged.
- **Zero native dependency:** no `libzerodds.dylib`/`.so`/`.dll`
  needed. A pure `.jar` artefact.
- **In-process bus:** local-only pub-sub via the `InProcessBus` class
  for tests, tools and single-process applications.
- **gRPC bridge (v1.1):** multi-process path via a gRPC service
  to a `libzerodds` server. With it, several pure Java
  JVMs communicate with C++/C#/native peers.
- **NativeAOT-friendly:** Java 22 `MemorySegment`/`Foreign-Memory-API`
  (v1.1) for zero-copy without JNI.

## Non-goals

- A direct RTPS wire stack in pure Java. Cross-vendor wire tests run
  over the gRPC bridge (v1.1).
- `org.omg.dds` modifications. This spec implements the PSM as
  given — no vendor annotations on the interface.

## §1 Architecture

### §1.1 Module layout

```
crates/java-omgdds/
  Cargo.toml                       # Rust codegen bridge to idl-java
  src/lib.rs                       # placeholder
  java/
    pom.xml                        # Maven project
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

Local pub-sub bus for single-JVM scenarios. Thread-safe topic map,
push-based sample delivery to all DataReaders on the same topic.

```java
public final class InProcessBus {
    public static InProcessBus instance();

    public <T> void publish(String topicName, T sample);
    public <T> void subscribe(String topicName, Consumer<T> handler);
}
```

### §1.3 Xcdr2Codec

Java-native XCDR2 encoder/decoder. Spec-conformant per DDS-XTypes 1.3 §7.4.

```java
public final class Xcdr2Codec {
    public static byte[] encode(Object sample);
    public static <T> T decode(byte[] bytes, Class<T> type);
}
```

## §2 OMG API coverage

Audit file: `docs/spec-coverage/dds-java-psm-1.0.md` (171 items, 156 done
+ 15 n/a). The pure Java path covers:

| Interface | Status | Note |
|---|---|---|
| `org.omg.dds.core.Entity` | ✅ Live | bound to InProcessBus |
| `org.omg.dds.core.Time` | ✅ Live | equivalent to Java `Instant` |
| `org.omg.dds.core.Duration` | ✅ Live | |
| `org.omg.dds.core.InstanceHandle` | ✅ Live | |
| `org.omg.dds.domain.DomainParticipant` | ✅ Live | InProcessBus |
| `org.omg.dds.domain.DomainParticipantFactory` | ✅ Live | Singleton |
| `org.omg.dds.topic.Topic<T>` | ✅ Live | TopicTypeSupport trait |
| `org.omg.dds.pub.Publisher` | ✅ Live | |
| `org.omg.dds.pub.DataWriter<T>` | ✅ Live | |
| `org.omg.dds.sub.Subscriber` | ✅ Live | |
| `org.omg.dds.sub.DataReader<T>` | ✅ Live | |
| `org.omg.dds.sub.Sample<T>` | ✅ Live | |
| `org.omg.dds.core.policy.*` | ✅ Live | default path |

## §3 Test obligation

Tests in `crates/java-omgdds/java/src/test/java/org/omg/dds/`:

- `CoreTypesTest`: Time/Duration/InstanceHandle roundtrips. (10 tests)
- `Xcdr2CodecTest`: encoder/decoder wire format roundtrip. (4 tests)
- `PubSubLoopbackTest`: full pub-sub path in-process. (4 tests)

`mvn test` delivers 18/18 green — see `docs/spec-coverage/LAYER-6-RC1-GAPS.md`.

## §4 Multi-process / cross-vendor

The pure Java path has no RTPS wire stack — `org.omg.dds.*` users
run single-JVM via `InProcessBus`. Cross-vendor wire (RTPS) and
multi-JVM run over the gRPC bridge (v1.1): the Java front end calls a
zerodds-grpc-bridged server, which links against `crates/dcps` and speaks
RTPS there. With it, Java users communicate with C++/Rust/C# peers
over the gRPC bridge without a native toolchain on the Java side.

| Scenario | v1.0 status | v1.1 plan |
|---|---|---|
| Single-JVM tools/tests/tutorials | ✅ done (InProcessBus) | unchanged |
| Multi-JVM local | ⏳ v1.1 | gRPC bridge to libzerodds server |
| Cross-vendor RTPS wire | ⏳ v1.1 | gRPC bridge to libzerodds server |
| ROS-2 rmw path | n/a | rclcpp adapter via `rmw_zerodds`, no Java |

## §5 gRPC bridge plan (v1.1)

- **gRPC service proto**: `crates/grpc-bridge/proto/dds-bridge.proto`
  defines the DCPS RPC schema (CreateParticipant, CreateTopic,
  Write, Take, etc.).
- **gRPC service server side**: `crates/grpc-bridge/src/dds_service.rs`
  wraps `zerodds-dcps` calls in the gRPC service.
- **Java client**: `crates/java-omgdds/java/src/main/java/org/zerodds/
  internal/grpc/GrpcDdsBridge.java` calls the server.
- **Selectable via system property**: `org.zerodds.bridge=inprocess`
  vs. `org.zerodds.bridge=grpc://host:port`.

## §6 Stability

Vendor spec, semver:

- v1.0 = current surface (pure Java with InProcessBus).
- v1.1 = + gRPC bridge to libzerodds server.
- v2.0 = + pure Java RTPS (optional).

## §7 Licence

Apache-2.0 (workspace default).

## §8 References

- OMG DDS-Java-PSM 1.0 (formal/2017-04-01)
- OMG DDS-XTypes 1.3 (formal/2019-02-01) §7.4 XCDR2
- OMG DDS 1.4 (formal/2015-04-10) §2.2.5 Built-in Topics
