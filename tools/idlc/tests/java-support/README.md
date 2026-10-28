# Fresh generated Java DDS descriptor consumer

This fixture consumes `ZdTypes.java`, `ZdCodec.java`, and `ZdTypeSupports.java`
freshly generated from `support.idl`. No generated code is stored or repaired here.
Compile it with the matching `runtime/java` sources and the Java DDS binding JAR
from `crates/java-omgdds/java`. The build requires the source-matched CLI integration
that emits the companion as a third file.

Run separately for XCDR1 and XCDR2 output, in isolated output/class directories.
The consumer discovers the generated descriptor's DDS representation ID (0 or 2)
and checks independent LE/BE octets, both key-hash forms, mutation, strict decode
refusals, empty data, optional data and the productive DDS encapsulation bridge.
It uses explicit generated singleton descriptors, not reflection or an example shim.

The generated `typeInformation()` supplies standard XTypes metadata separately
from the historical vendor hash. The consumer checks fixed framing positions,
the complete hash and serialized object size, then carries those bytes through
both writer and reader SEDP parameter construction/parsing. It prints
`SOURCE_TYPE_INFORMATION` as hex evidence. Generator tests check dependent
types against the shared AST registry; unsupported or incomplete dependency
closures explicitly refuse metadata instead of reporting an empty closure.

The `Composite` fixture also checks the immutable `typeObjects()` registry:
minimal and complete objects for the root and both dependencies, no unrelated
objects in a leaf registry, unknown-ID absence, defensive copying and rejection
of bytes that no longer match the claimed equivalence hash. These are generated
service data only, not TypeLookup RPC replies or an advertised service.
The registry also carries complete-to-minimal pairs and each object's direct
dependency IDs, whose resolved entries retain the actual serialized sizes.

No sockets or DDS participants are opened by this bounded consumer. Its PASS marker
does not prove cross-process DDS delivery, remote TypeInformation discovery,
assignability or cross-vendor interoperability. Source-derived metadata is checked
against the shared AST object in the Rust generator unit test.
Announcing TypeInformation does not implement a TypeLookup service, and receiving
its raw bytes does not establish semantic type validation or assignability.

Suggested queued checks, not commands reported as already executed:

1. Run the `java_support::tests` CLI unit tests after module integration.
2. Generate the three Java files for each representation using the current CLI.
3. Compile those files, the wire runtime sources and `Consumer.java` against the
   source-matched DDS binding JAR, then run `Consumer` in that fresh classpath.
4. Keep generation, compiler output and both PASS markers as separate evidence.
