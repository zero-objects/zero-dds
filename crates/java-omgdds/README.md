# `zerodds-java-omgdds`

Native Java DDS PSM (`org.omg.dds.*`) — implements the
[OMG DDS Java PSM 1.0][java-psm], the spec-defined Java API form.
Part of [**ZeroDDS**](../../README.md). Safety class **STANDARD**.

OMG DDS Java PSM 1.0 spec audit (K15) completed 2026-04-28:
**156 done / 0 partial / 0 open / 15 n/a**. Fully spec-conform.

---

## Relationship to the JNI bridge

Two orthogonal Java paths:

* `zerodds-java-jni` — JNI bridge over the Rust runtime. Uses
  `zerodds-dcps` directly, an idiomatic Java wrapper without strict
  spec fidelity.
* `zerodds-java-omgdds` — this crate. Native Java classes that follow
  the `org.omg.dds.*` PSM exactly. Drop-in for applications that
  run today on the RTI Connext, OpenSplice or Cyclone DDS Java API.

Both paths use the same wire stack under the hood
(`crates/rtps` via JNI). The difference is only in the
Java API form.

## Build

```bash
# Maven build (standard Java):
mvn -f crates/java-omgdds/pom.xml package

# The JAR lands in:
ls crates/java-omgdds/target/zerodds-java-omgdds-0.0.0.jar
```

## Quickstart (OMG PSM form)

```java
import org.omg.dds.core.*;
import org.omg.dds.domain.*;
import org.omg.dds.pub.*;
import org.omg.dds.topic.*;

DomainParticipantFactory factory = DomainParticipantFactory.getInstance();
DomainParticipant dp = factory.createParticipant(0);

Topic<Pose> topic = dp.createTopic("Telemetry", Pose.class);
Publisher pub = dp.createPublisher();
DataWriter<Pose> writer = pub.createDataWriter(topic);

writer.write(new Pose("r1", 1.0, 2.0, 3.0));
```

`Pose` is generated via `zerodds-idlc Robot.idl --java`.

## Structure

```
crates/java-omgdds/
├── Cargo.toml          workspace member, build glue to the JNI layer
├── src/lib.rs          architecture doc + JNI hooks
├── src/main/java/      Java sources (Maven layout)
│   └── org/omg/dds/
│       ├── core/        Time, Duration, Status, Exception, Listener
│       ├── core/policy/ all 21 QoS policies, immutable builder
│       ├── domain/      DomainParticipantFactory, DomainParticipant
│       ├── topic/       Topic, ContentFilteredTopic, MultiTopic
│       ├── pub/         Publisher, DataWriter, PublisherListener
│       ├── sub/         Subscriber, DataReader, Sample, SampleSelector
│       ├── type/        TypeSupport, dynamic, builtin
│       └── rtps/        conversion helpers (delegate to the Rust stack)
└── pom.xml             Maven build
```

## Documentation Trail

For the user guide see
[Documentation Trail Station 05 → Java](../../documentation/05-integration/java.md).

## Spec references

* [OMG DDS Java PSM 1.0][java-psm]
* [OMG DDS 1.4][dds] §2.2 — DCPS
* `docs/spec-coverage/zerodds-java-psm-1.0.md` — per-section audit
* `docs/architecture/05_java_jni_bridge.md` — internal bridge architecture

[java-psm]: https://www.omg.org/spec/DDS-Java/1.0/
[dds]: https://www.omg.org/spec/DDS/1.4/
