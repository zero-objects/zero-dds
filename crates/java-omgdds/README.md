# `zerodds-java-omgdds`

Native Java DDS-PSM (`org.omg.dds.*`) — implementiert das
[OMG DDS Java PSM 1.0][java-psm], die spec-definierte Java-API-Form.
Teil von [**ZeroDDS**](../../README.md). Safety-Klasse **STANDARD**.

OMG-DDS-Java-PSM 1.0 Spec-Audit (K15) abgeschlossen 2026-04-28:
**156 done / 0 partial / 0 open / 15 n/a**. Voll spec-konform.

---

## Beziehung zur JNI-Bridge

Zwei orthogonale Java-Pfade:

* `zerodds-java-jni` — JNI-Bridge ueber die Rust-Runtime. Nutzt
  `zerodds-dcps` direkt, idiomatischer Java-Wrapper ohne strikte
  Spec-Treue.
* `zerodds-java-omgdds` — diese Crate. Native Java-Klassen die exakt
  dem `org.omg.dds.*`-PSM folgen. Drop-in fuer Anwendungen, die
  heute auf RTI Connext, OpenSplice oder Cyclone DDS Java-API
  laufen.

Beide Pfade nutzen unter der Haube den gleichen Wire-Stack
(`crates/rtps` via JNI). Der Unterschied liegt nur in der
Java-API-Form.

## Build

```bash
# Maven-Build (Standard-Java):
mvn -f crates/java-omgdds/pom.xml package

# JAR landet in:
ls crates/java-omgdds/target/zerodds-java-omgdds-0.0.0.jar
```

## Quickstart (OMG-PSM-Form)

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

`Pose` ist generiert via `zerodds-idlc Robot.idl --java`.

## Struktur

```
crates/java-omgdds/
├── Cargo.toml          Workspace-Member, Build-Glue zur JNI-Schicht
├── src/lib.rs          Architektur-Doc + JNI-Hooks
├── src/main/java/      Java-Sources (Maven-Layout)
│   └── org/omg/dds/
│       ├── core/        Time, Duration, Status, Exception, Listener
│       ├── core/policy/ alle 21 QoS-Policies, immutable Builder
│       ├── domain/      DomainParticipantFactory, DomainParticipant
│       ├── topic/       Topic, ContentFilteredTopic, MultiTopic
│       ├── pub/         Publisher, DataWriter, PublisherListener
│       ├── sub/         Subscriber, DataReader, Sample, SampleSelector
│       ├── type/        TypeSupport, dynamic, builtin
│       └── rtps/        Konvertierungs-Helper (delegiert an Rust-Stack)
└── pom.xml             Maven Build
```

## Documentation Trail

Fuer den User-Guide siehe
[Documentation Trail Station 05 → Java](../../documentation/05-integration/java.md).

## Spec-Referenzen

* [OMG DDS Java PSM 1.0][java-psm]
* [OMG DDS 1.4][dds] §2.2 — DCPS
* `docs/spec-coverage/zerodds-java-psm-1.0.md` — Per-Section-Audit
* `docs/architecture/05_java_jni_bridge.md` — interne Bridge-Architektur

[java-psm]: https://www.omg.org/spec/DDS-Java/1.0/
[dds]: https://www.omg.org/spec/DDS/1.4/
