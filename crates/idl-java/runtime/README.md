# zerodds idl-java runtime

Java sources shipped alongside generated code from `zerodds-idl-java`
(Cluster C5.4-b). Drop these files into the consumer Java project's
source tree (or package them into a small JAR) so generated classes
that reference `org.zerodds.types.*` and `org.omg.dds.topic.TopicType`
resolve at compile time.

## Annotation-Bridge

| IDL annotation                | Java annotation                      |
|-------------------------------|--------------------------------------|
| `@key`                        | `@org.zerodds.types.Key`             |
| `@id(N)`                      | `@org.zerodds.types.Id(N)`           |
| `@optional`                   | `@org.zerodds.types.Optional`        |
| `@must_understand`            | `@org.zerodds.types.MustUnderstand`  |
| `@external`                   | `@org.zerodds.types.External`        |
| `@nested`                     | `@org.zerodds.types.Nested`          |
| `@extensibility(FINAL|...)`   | `@org.zerodds.types.Extensibility`   |

## DDS-Java-PSM stub

`org.omg.dds.topic.TopicType<T>` — empty marker interface; every
top-level struct without `@nested` annotation implements
`TopicType<SelfType>`. The full DDS-Java-PSM Service-Environment SPI is
deferred to C5.5-b.

## Bitset / Bitmask runtime

Bitset and bitmask are emitted as fully-self-contained Java types — no
shared runtime classes. See generated source for the chosen mapping
(EnumSet&lt;Flag&gt; for bitmask; long-backed wrapper class for bitset
≤ 64 bits).
