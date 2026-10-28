# Java integration

The current Java workflow connects fresh IDL-generated codecs to the Java
DDS API through an explicit generated descriptor. Start with the executable
[Java conformance example](../../zerodds-examples/idl-conformance/java/README.md),
its [App.java](../../zerodds-examples/idl-conformance/java/App.java), and its
[build-and-run.sh](../../zerodds-examples/idl-conformance/java/build-and-run.sh).
Those files own the complete runnable sample rather than an independent
Hello World fragment in this guide.

## Matching build inputs

Use the CLI, generated source, wire runtime and Java binding from the same
candidate checkout. The [Maven POM](../../crates/java-omgdds/java/pom.xml)
currently declares `org.zerodds:omgdds:1.0.0-rc.7`, with Java source/target 17.
These are local build coordinates, not a claim that this candidate is
available from Maven Central or another public repository.

Local packaging, from the ZeroDDS root:

```sh
mvn -f crates/java-omgdds/java/pom.xml package
```

This is a source-backed packaging instruction, not a statement that this
documentation pass ran the complete Maven suite. Packaging requires the
POM's dependencies/plugins and source directories. The POM also includes
the existing crates/idl-java/runtime sources. Local installation into the
Maven cache uses the same POM with the `install` goal, not deployment.

The example requires JDK 17 or newer with java, javac and jar on PATH.
Pass the exact already built binding JAR or classes directory and CLI
executable to its script. It does not build a missing binding or select an
arbitrary JAR from a wildcard. Its README supplies the full invocation.

The Java binding is implemented without a required JNI library. The current
wire codec sources used by this example are separately compiled from
[runtime/java/src/main/java/org/zerodds/wire](../../runtime/java/src/main/java/org/zerodds/wire).
Do not substitute the older Xcdr2Codec utility or reflective POJO support
for the new generated codec.

## Current generator output and descriptor

The script checks the shared
[Telemetry IDL](../../zerodds-examples/idl-conformance/rust/idl/20_mixed_combo.idl)
and invokes the current CLI form `generate --lang java --encoding xcdr2 --out DIR FILE`.
There is no flat `--java` generation interface.

For this aggregate schema, the output is three Java compilation units:

| File | Role |
|---|---|
| ZdTypes.java | Declarations nested under ZdTypes, including public fields and map-entry classes |
| ZdCodec.java | Generated data encoders/decoders using the matching wire runtime |
| ZdTypeSupports.java | Explicit DDS descriptors using the generated codecs, shared key plans and source-derived metadata |

Wire-only consumers need the first two plus the wire runtime. The third
additionally requires the Java DDS binding. Enum-only or explicitly nested
schemas do not necessarily receive topic-support companions.

The example explicitly selects
`ZdTypeSupports.Support_combo_Telemetry.INSTANCE` and passes it to both
`subscriber.createDataReader(topic, support)` and
`publisher.createDataWriter(topic, support)`. See the complete App.java above
for value construction, lifecycle scope and assertions. No annotation scanning,
reflection fallback or hand-written serializer is added to connect the example.

A generated descriptor implements the selected representation, not both
versions automatically. DDS representation IDs are 0 for XCDR1 and 2 for
XCDR2. Its explicit codec methods support both byte orders and refuse a
different representation. Typed decoding checks reader status and complete
body consumption. The runtime bridge preserves incoming encapsulation and
checks its framing against extensibility before decode.

## What the executable example establishes

The migrated Telemetry example was measured with fresh check/generate,
javac -Werror and the local DDS API loopback. It checks keys, enum,
typedef-to-double, nested struct sequence, union, map entries, optional
presence/absence and fixed array. It also checks explicit LE/BE decoding,
key stability/mutation and refusal of trailing bytes.

The run is **in-process**, using the Java DDS API and its local bus. It is
not a UDP or cross-process measurement merely because it creates a
DomainParticipant. The script disables RTPS, rejects an inherited enabling
ZERODDS_RTPS setting, and retains fresh generated/classes directories.
Historical generated/, runtime-support/ and out/ are neither removed nor
compiled.

Separate primitive and string/wstring example coverage has not yet been
migrated from the older fixture arrangement. That is an outstanding
example task, not a newly inferred product restriction.

## Transport and remaining boundaries

The binding contains its own
[pure-Java RTPS implementation](../../crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java)
and [DDS bridge](../../crates/java-omgdds/java/src/main/java/org/zerodds/rtps/DdsWireBridge.java).
A separate bridge daemon is therefore not a universal prerequisite.
However, this example intentionally does not certify that transport.

Standard PID_TYPE_INFORMATION, TypeLookup/assignability, remote lifecycle
and cross-vendor endpoint behavior still need their own implementation and
measured acceptance. Returning local TypeObject bytes or a vendor hash is
not a standard discovery exchange. The descriptor's local key checks do not
prove remote instance lifecycle.

This guide makes no blanket Java-PSM certification, unchanged RTI/OpenSplice
application compatibility, thread-safety guarantee for every API, or universal
QoS/listener support claim. Verify the exact required operations against
source and measured tests.

## Related documentation

- [IDL compiler handbook](../04-idl/idlc-handbook.md), especially generation,
  runtime matching and executable companions.
- [Java conformance example](../../zerodds-examples/idl-conformance/java/README.md).
- [Java binding source overview](../../crates/java-omgdds/README.md).

Links to zerodds-examples assume its checkout is nested in the ZeroDDS
workspace at that directory, as used for internal integration. No publication
or distribution availability is implied by these workspace links.
