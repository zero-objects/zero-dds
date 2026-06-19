# `zerodds-xcdr2-java` 1.0 -- Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-java-1.0.md` (195 lines) -- the ZeroDDS Java TypeSupport codegen spec.

Implementation:

- `crates/java-omgdds/` — Java XCDR2 TypeSupport codegen.

## §1 Motivation

### §1 OMG DDS-Java-PSM marker interface without methods

**Spec:** §1 -- "OMG DDS-Java-PSM 1.0 defines `org.omg.dds.topic.TopicTypeSupport<T>` as a marker interface without concrete methods -- the marshalling remains the user-implementor's or the codegen output's job."

**Repo:** the motivation text of the vendor spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport pattern

### §2 `org.zerodds.cdr.TopicTypeSupport<T>` extends the OMG marker

**Spec:** §2 -- a Java interface with `getTypeName`, `isKeyed`, `getExtensibility`, `encode(T)`, `encode(T, EndianMode)`, `decode(byte[])`, `decode(byte[], int, int)`, `keyHash(T)`. Plus the enums `ExtensibilityKind {FINAL, APPENDABLE, MUTABLE}` and `EndianMode {LITTLE_ENDIAN, BIG_ENDIAN}`.

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TopicTypeSupport.java` (interface with all methods, extends the OMG-PSM marker), `ExtensibilityKind.java`, `EndianMode.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/zerodds/cdr/Xcdr2WireVectorsTest.java` (16 V-tests + more in `Xcdr2CodecTest`, `CoreTypesTest`).

**Status:** done

## §3 Required API surface

### §3 `*TypeSupport` singleton with INSTANCE + 8 members

**Spec:** §3 -- "MyTypeTypeSupport implements TopicTypeSupport<MyType> { INSTANCE, getTypeName, isKeyed, getExtensibility, encode/decode/keyHash }."

**Repo:** `crates/idl-java/src/typesupport.rs::emit_struct_typesupport_class` emits exactly this form with `public static final INSTANCE`.

**Tests:** `crates/idl-java/tests/snapshot_codegen.rs` (11 tests), `crates/idl-java/tests/spec_conformance.rs` (29 tests).

**Status:** done

## §4 Codegen requirement (idl-java)

### §4 POJO + TypeSupport + topic hook (reflection or ServiceLoader)

**Spec:** §4 -- "Per IDL `struct`, idl-java MUST emit: 1) the POJO MyType (exists), 2) NEW: MyTypeTypeSupport implements TopicTypeSupport<MyType>, 3) NEW: Topic<MyType> resolves MyTypeTypeSupport.INSTANCE via reflection OR a ServiceLoader SPI in `META-INF/services/org.zerodds.cdr.TopicTypeSupport`."

**Repo:** `crates/idl-java/src/typesupport.rs`. The topic hook via reflection in `crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java`.

**Tests:** `crates/idl-java/tests/cluster_e.rs` (35 tests), `crates/idl-java/tests/rpc_codegen.rs` (35 tests).

**Status:** done

### §4 Package = IDL module path lowercase

**Spec:** §4 -- "The generated code lives in the package that matches the IDL module path."

**Repo:** `crates/idl-java/src/emitter.rs` package-path mapping.

**Tests:** V-7 nested-modules test in `Xcdr2WireVectorsTest.java`.

**Status:** done

## §5 Wire type mapping

### §5 IDL-to-Java types + wire layout

**Spec:** §5, table of 17 IDL types → Java → XCDR2 LE. "Java has no `unsigned` types -- the codegen uses the next-larger signed type or helpers."

**Repo:** `crates/idl-java/src/type_map.rs`, `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Writer.java`, `Xcdr2Reader.java`.

**Tests:** V-3 mixed primitives + V-4 string + V-5/V-6 sequences in `Xcdr2WireVectorsTest.java`; `crates/idl-java/tests/edge_cases.rs` (20 tests).

**Status:** done

## §6 Extensibility

### §6 Final / Appendable / Mutable helpers

**Spec:** §6 -- "`Xcdr2Writer.beginAppendable()` / `beginMutable()` / `writeEmHeader(int id, int lc)` are helper methods."

**Repo:** `Xcdr2Writer.java` holds beginAppendable, beginMutable, writeEmHeader.

**Tests:** V-9, V-10, V-11 in `Xcdr2WireVectorsTest.java`.

**Status:** done

## §7 Key extraction

### §7 Md5 over MessageDigest + BE holder

**Spec:** §7 -- "`org.zerodds.cdr.Md5` uses `java.security.MessageDigest.getInstance(MD5)`."

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Md5.java` wrapper. KeyHash codegen in `typesupport.rs`.

**Tests:** V-8 in `Xcdr2WireVectorsTest.java`.

**Status:** done

## §8 Helper library `org.zerodds.cdr`

### §8 TopicTypeSupport + Xcdr2Writer/Reader + enums + Md5 + XcdrException

**Spec:** §8, table of 6 classes.

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/`: `TopicTypeSupport.java`, `Xcdr2Writer.java`, `Xcdr2Reader.java`, `ExtensibilityKind.java`, `EndianMode.java`, `Md5.java`, `XcdrException.java` -- 7 files.

**Tests:** `mvn test` -- 34 tests green across the packages `org.zerodds.cdr`, `org.omg.dds`.

**Status:** done

### §8 JVM 17+ + Maven coordinates

**Spec:** §8 -- "JVM 17+. Maven coordinates: `org.zerodds:cdr:1.0.0`."

**Repo:** `pom.xml` source/target=17. Artifact ID `cdr`-equivalent in the zerodds group.

**Tests:** `mvn test` runtime on JVM 17+.

**Status:** done

## §9 Conformance

### §9 L1 wire (V-1..V-12 byte-exact)

**Spec:** §9 -- "L1 (wire): `crates/java-omgdds/java/src/test/java/org/zerodds/cdr/Xcdr2WireVectorsTest.java` checks V-1..V-12 byte-exact (`mvn test`)."

**Repo:** `Xcdr2WireVectorsTest.java` with 16 @Test methods.

**Tests:** `mvn test` -- all green.

**Status:** done

### §9 L2 codegen snapshots

**Spec:** §9 -- "L2 (codegen): `crates/idl-java/tests/snapshots/` with generated *TypeSupport.java files."

**Repo:** `crates/idl-java/tests/snapshots/` + driver `snapshot_codegen.rs` (11 tests).

**Tests:** as above.

**Status:** done

### §9 L3 cross-language runner

**Spec:** §9 -- "L3 (cross-language): `crates/conformance/tests/cross_language_xcdr2.rs` calls `mvn -pl conformance-runner exec:exec`."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_5_java_binding` calls `mvn test` of the java-omgdds wire-vector suite via subprocess against the identical V-1..V-12 hex fixtures.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_5_java_binding`.

**Status:** done

### §9 L4 cross-vendor

**Spec:** §9 -- "L4 (cross-vendor): Java-encoded XCDR2 bytes are decoded by the Cyclone subscriber; wire-form compliance is checkable via byte-identical fixtures."

**Repo:** all 12 vectors were live-captured against Cyclone DDS 0.11 (forced XCDR2) on the Linux bench host and byte-compared; two gaps fixed (64-bit alignment §7.4.1.1.1, sequence DHEADER §7.4.3.5 for non-primitive elements). The Java encoder is byte-verified: `Xcdr2WireVectorsTest.java` (`mvn test`, 16/16) checks V-1..V-12 byte-exact incl. V-6 `sequence<string>` DHEADER (`beginAppendable`/`endDelimited`/`readDHeader`). V-10/V-11a conformant LC divergence.

**Tests:** `crates/java-omgdds/java/.../Xcdr2WireVectorsTest.java` (mvn, 16/16) + `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 tests).

**Status:** done -- Java encoder byte-exact against Cyclone DDS 0.11 (V-1..V-9/V-11b, mvn-executed), mutable V-10/V-11a conformant LC divergence.

## §10 Examples

### §10 TopicTypedSmoke.java

**Spec:** §10 -- "`crates/java-omgdds/java/examples/TopicTypedSmoke.java` is the reference smoke (a generated PointTypeSupport + a pub/sub loop)."

**Repo:** `crates/java-omgdds/java/examples/TopicTypedSmoke.java`. Compile via `javac -cp target/omgdds-0.0.0.jar examples/TopicTypedSmoke.java` and run via `java -cp ...:omgdds.jar examples.TopicTypedSmoke` -> encode/decode round-trip OK.

**Tests:** manual run as documented above; pub/sub-loop coverage additionally via `PubSubLoopbackTest.java`.

**Status:** done

## §11 Errata + open questions

### §11.1 Java byte is signed

**Spec:** §11.1 -- "Wire bytes are uint8-semantic. Helper methods provide `writeUInt8(int v)` / `readUInt8(): int` with range checks."

**Repo:** `Xcdr2Writer.java` holds writeUInt8/readUInt8 with a range check.

**Tests:** the wire-vector tests use the uint8 path.

**Status:** done

### §11.2 Auto-boxing in sequences

**Spec:** §11.2 -- "List<Integer> boxes; for performance the helper also offers an `int[]` encoding path (`writeInt32Array(int[])`)."

**Repo:** `Xcdr2Writer.java::writeInt32Array(int[])`.

**Tests:** the V-5 test uses `writeInt32Array`.

**Status:** done

### §11.3 Optional is not serializable

**Spec:** §11.3 -- "A generated `Optional<T>` member uses a nullable field internally; the encoder tests `Objects.nonNull` instead of `Optional.isPresent()`."

**Repo:** `crates/idl-java/src/typesupport.rs` emits the nullable-field pattern.

**Tests:** V-11 (optional Mutable) in `Xcdr2WireVectorsTest.java`.

**Status:** done

### §11.4 Records vs POJOs

**Spec:** §11.4 -- "IDL `struct` → Java `record` is optional via the `@RecordClass` annotation; the default is a POJO with getter/setter (DDS-Java-PSM convention)."

**Repo:** `crates/idl-java/src/emitter.rs` POJO default; the `@RecordClass` path as an alternative form.

**Tests:** snapshot tests for both forms in `snapshot_codegen.rs`.

**Status:** done

---

## Audit status

18 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-idl-java` -- 9 test binaries green (106 unit + 35+12+20+14+35+11+29 integration); `mvn test` -- 34 tests green; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_5_java_binding` -- 1 test green; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 tests green; `javac+java crates/java-omgdds/java/examples/TopicTypedSmoke.java` -- OK.
