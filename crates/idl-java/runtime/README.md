# zerodds idl-java runtime

Java sources shipped alongside generated code from `zerodds-idl-java`.
Drop these files into the consumer Java project's source tree (or package
them into a small JAR) so generated classes that reference
`org.zerodds.types.*` resolve at compile time.

Nothing here lives in a package that begins with `org.omg`: DDS-Java-PSM
1.0 §7.2.6 forbids an implementation from putting its own types into
`org.omg.dds` or a package below it. The `org.omg.dds.*` API itself comes
from the PSM; the marker interface that used to sit in
`org.omg.dds.topic.TopicType` was one such extension and is gone.

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

## Bitset / Bitmask runtime

Bitset and bitmask are emitted as fully-self-contained Java types — no
shared runtime classes. See generated source for the chosen mapping
(EnumSet&lt;Flag&gt; for bitmask; long-backed wrapper class for bitset
≤ 64 bits).
