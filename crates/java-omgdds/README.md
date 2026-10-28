# `zerodds-java-omgdds`

Native Java DDS PSM (`org.omg.dds.*`) — implements the
[OMG DDS Java PSM 1.0][java-psm], the spec-defined Java API form.
Part of [**ZeroDDS**](../../README.md). Safety class **STANDARD**.

OMG DDS Java PSM 1.0 spec audit: the item counts are in
`docs/spec-coverage/dds-java-psm-1.0.md` (447 done / 0 partial / 0 open /
79 n/a (informative)). The conformance level of Clause 2 is Minimum Profile,
Content-Subscription Profile, Persistence Profile, Ownership Profile and
Extensible and Dynamic Types, without the Object Model Profile (DLRL), which
`org.omg.dds.dlrl` and DLRL 1.2 cover separately.

---

## Java path

This crate is the Java binding of ZeroDDS: Java classes that follow the
`org.omg.dds.*` PSM, for applications written against the RTI Connext,
OpenSplice or Cyclone DDS Java API. It carries its own pure-Java RTPS
wire stack (`org.zerodds.rtps`, DDSI-RTPS 2.5): no JNI, no Rust
library, no native file in the JAR. There is no JNI binding.

## Build

```bash
# Maven build (standard Java):
mvn -f crates/java-omgdds/java/pom.xml package

# The JAR lands in:
ls crates/java-omgdds/java/target/omgdds-*.jar
```

## Quickstart (OMG PSM form)

```java
import org.omg.dds.core.*;
import org.omg.dds.domain.*;
import org.omg.dds.pub.*;
import org.omg.dds.topic.*;

// §7.3.1: the Service names its implementation class in a system property and
// loads it reflectively; §7.4.1: the participant factory is a per-environment
// singleton reached through getInstance(env).
System.setProperty(ServiceEnvironment.IMPLEMENTATION_CLASS_NAME_PROPERTY,
        "org.zerodds.ServiceEnvironmentImpl");
ServiceEnvironment env = ServiceEnvironment.createInstance(
        MyApp.class.getClassLoader());

DomainParticipantFactory factory = DomainParticipantFactory.getInstance(env);
DomainParticipant dp = factory.createParticipant(0);

Topic<Pose> topic = dp.createTopic("Telemetry", Pose.class);
Publisher pub = dp.createPublisher();
DataWriter<Pose> writer = pub.createDataWriter(topic);

writer.write(new Pose("r1", 1.0, 2.0, 3.0));
```

`Pose` is generated with `zerodds-idlc generate --lang java --out <dir> Robot.idl`
The default shape is a class with Java Bean accessors; `--java-mapping omg` gives
public fields (see "Two shapes" below).

### Two shapes of generated Java types

`idlc generate --lang java` has two mappings, selected with
`--java-mapping`:

* `psm` (default): Clause 9 of DDS-Java-PSM 1.0, "Improved Plain Language Binding for
  Java". An aggregation type is a final class with private state, a
  no-argument constructor and Java Bean accessors (`getX()`, `setX(v)`, and
  `isX()` for a `boolean`); an unbounded sequence is a
  `java.util.Collection<E>`; a bounded sequence and an array stay Java
  arrays (§9.1.1, §9.1.2). A union's discriminator is the property
  `discriminator`. The generated codec calls `org.zerodds.wire.Sequences`
  for what it cannot decide by type, so the JAR (or `runtime/java`) must be
  on the classpath.
* `omg`: the OMG IDL-to-Java mapping. A structure's members are public
  fields and every `sequence<T>` is a `T[]`. Code that reads the fields of
  the generated types directly passes `--java-mapping omg`.

The wire bytes are the same under both.

The TypeLookup types the binding itself ships
(`org.zerodds.typelookup.generated`) keep the `omg` mapping: the
`java-typelookup` gate (`tools/idlc/tests/java-typelookup`) reads their
public fields.

## Structure

```
crates/java-omgdds/
├── idl/typelookup/     IDL of the TypeLookup service
├── scripts/            generate-typelookup.sh
├── run_grpc_bridge_e2e.sh
├── java/src/main/java/ Java sources (Maven layout)
│   └── org/omg/dds/
│       ├── core/        Time, Duration, Status, Exception, Listener
│       ├── core/policy/ all 21 QoS policies, immutable builder
│       ├── domain/      DomainParticipantFactory, DomainParticipant
│       ├── topic/       Topic, ContentFilteredTopic, MultiTopic
│       ├── pub/         Publisher, DataWriter, PublisherListener
│       ├── sub/         Subscriber, DataReader, Sample, SampleSelector
│       └── type/        TypeSupport, dynamic, builtin
│   └── org/zerodds/
│       └── rtps/        pure-Java RTPS wire stack (SPDP/SEDP/DATA over UDP)
├── java/src/test/java/ tests
├── java/examples/
└── java/pom.xml         Maven build
```

## Documentation Trail

For the user guide see
[Documentation Trail Station 05 → Java](../../documentation/05-integration/java.md).

## Spec references

* [OMG DDS Java PSM 1.0][java-psm]
* [OMG DDS 1.4][dds] §2.2 — DCPS
* `docs/spec-coverage/zerodds-java-psm-1.0.md` — per-section audit

[java-psm]: https://www.omg.org/spec/DDS-Java/1.0/
[dds]: https://www.omg.org/spec/DDS/1.4/
