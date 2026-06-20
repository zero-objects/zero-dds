# Java

ZeroDDS provides a **Pure-Java** Java-PSM implementation — no JNI,
no native library on the Java side. The `org.omg.dds.*` API
(spec-compliant per OMG DDS-Java-PSM 1.0) is delivered as a plain
`.jar` artifact that runs on any JVM. The earlier JNI bridge was
retired in favour of pure Java.

## Maven

```xml
<dependency>
  <groupId>io.zerodds</groupId>
  <artifactId>zerodds-java-omgdds</artifactId>
  <version>${zerodds.version}</version>
</dependency>
```

Build the artifact locally — its version matches the workspace release:

```bash
cd crates/java-omgdds/java
mvn install
```

## What you get

- **`org.omg.dds.*`** — the OMG-defined Java-PSM API surface.
  Existing application code written against RTI Connext or
  OpenSplice's Java PSM compiles against this artifact unchanged.
- **`org.zerodds.internal.InProcessBus`** — single-JVM pub/sub
  loopback.
- **`org.zerodds.internal.Xcdr2Codec`** — Java-native XCDR2
  encoder/decoder (per OMG DDS-XTypes 1.3 §7.4).

No `libzerodds.so` / `.dylib` / `.dll` is required on the Java
classpath. No `System.loadLibrary` call. Pure JVM bytecode.

## Hello, world

```java
import org.omg.dds.core.*;
import org.omg.dds.domain.*;
import org.omg.dds.pub.*;
import org.omg.dds.topic.*;

public class Hello {
    public static void main(String[] args) throws Exception {
        DomainParticipantFactory factory =
            DomainParticipantFactory.getInstance();
        try (DomainParticipant dp = factory.createParticipant(0)) {
            Topic<Pose> topic = dp.createTopic("Telemetry", Pose.class);
            Publisher pub = dp.createPublisher();
            DataWriter<Pose> writer = pub.createDataWriter(topic);

            writer.write(new Pose("r1", 1.0, 2.0, 3.0));
        }
    }
}
```

`Pose` is generated from IDL by `zerodds-idlc Robot.idl --java`
(see `crates/idl-java/`).

## Generated types from IDL

```
gen/java/com/example/robot/
├── Pose.java                     # POJO with @TopicType
└── Telemetry.java
```

Each generated class implements `org.omg.dds.topic.TopicTypeSupport<T>`,
carries the `@TopicType` annotation, and supplies XCDR2
serialise / deserialise methods.

## Threading

`InProcessBus` is thread-safe; multiple Java threads can share
one `DomainParticipant` and publish / subscribe concurrently
without app-level locks.

## QoS

```java
DataWriterQos qos = DataWriterQos.copyFromTopicQos(topic.getQos());
qos = qos.withPolicy(qos.getReliability().withKind(ReliabilityKind.RELIABLE))
         .withPolicy(qos.getDurability().withKind(DurabilityKind.TRANSIENT_LOCAL));
DataWriter<Pose> w = pub.createDataWriter(topic, qos);
```

The fluent QoS builder mirrors the OMG DDS Java PSM 1.0
specification.

## Listeners

```java
DataReader<Pose> reader = sub.createDataReader(topic);
reader.setListener(new DataReaderListener<Pose>() {
    @Override public void onDataAvailable(DataReader<Pose> r) {
        SampleIterator<Pose> samples = r.take();
        while (samples.hasNext()) {
            Pose p = samples.next().getData();
            System.out.println(p);
        }
    }
});
```

## Spring / Quarkus

Wire `DomainParticipant` as a bean:

```java
@Configuration
public class DdsConfig {
    @Bean(destroyMethod = "close")
    public DomainParticipant participant() {
        return DomainParticipantFactory.getInstance().createParticipant(0);
    }
}
```

## Multi-process / Cross-vendor

The pure-Java side runs the in-process loopback (`InProcessBus`). To
communicate across JVM processes or with C++ / Rust / C# peers on the
RTPS wire, run a bridge daemon and have the Java app speak that
protocol — the daemon owns the RTPS stack:

- **gRPC bridge** — the Java client talks gRPC to a
  `zerodds-grpc-bridged` server.
- **MQTT / AMQP bridge** — run `zerodds-mqtt-bridged` or
  `zerodds-amqp-bridged` and publish/subscribe over that protocol.

## Reading further

- `crates/java-omgdds/README.md` — runtime details.
- OMG DDS Java PSM 1.0 — `formal/2017-04-01`.
