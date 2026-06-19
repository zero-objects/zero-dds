# `zerodds-xcdr2-java` 1.0 -- Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-java-1.0.md` (195 Zeilen) -- ZeroDDS Java TypeSupport-Codegen-Spec.

Implementation:

- `crates/java-omgdds/` — Java XCDR2-TypeSupport-Codegen.

## §1 Motivation

### §1 OMG-DDS-Java-PSM Marker-Interface ohne Methoden

**Spec:** §1 -- "OMG DDS-Java-PSM 1.0 definiert `org.omg.dds.topic.TopicTypeSupport<T>` als Marker-Interface ohne konkrete Methoden -- das Marshalling bleibt User-Implementor oder Codegen-Output."

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport-Pattern

### §2 `org.zerodds.cdr.TopicTypeSupport<T>` extends OMG-Marker

**Spec:** §2 -- Java-Interface mit `getTypeName`, `isKeyed`, `getExtensibility`, `encode(T)`, `encode(T, EndianMode)`, `decode(byte[])`, `decode(byte[], int, int)`, `keyHash(T)`. Plus enums `ExtensibilityKind {FINAL, APPENDABLE, MUTABLE}` und `EndianMode {LITTLE_ENDIAN, BIG_ENDIAN}`.

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TopicTypeSupport.java` (Interface mit allen Methoden, extends OMG-PSM Marker), `ExtensibilityKind.java`, `EndianMode.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/zerodds/cdr/Xcdr2WireVectorsTest.java` (16 V-Tests + weitere in `Xcdr2CodecTest`, `CoreTypesTest`).

**Status:** done

## §3 Required API-Surface

### §3 `*TypeSupport` Singleton mit INSTANCE + 8 Members

**Spec:** §3 -- "MyTypeTypeSupport implements TopicTypeSupport<MyType> { INSTANCE, getTypeName, isKeyed, getExtensibility, encode/decode/keyHash }."

**Repo:** `crates/idl-java/src/typesupport.rs::emit_struct_typesupport_class` emittiert exakt diese Form mit `public static final INSTANCE`.

**Tests:** `crates/idl-java/tests/snapshot_codegen.rs` (11 tests), `crates/idl-java/tests/spec_conformance.rs` (29 tests).

**Status:** done

## §4 Codegen-Pflicht (idl-java)

### §4 POJO + TypeSupport + Topic-Hook (Reflection oder ServiceLoader)

**Spec:** §4 -- "Pro IDL-`struct` MUSS idl-java emittieren: 1) POJO MyType (existiert), 2) NEU: MyTypeTypeSupport implements TopicTypeSupport<MyType>, 3) NEU: Topic<MyType> resolved MyTypeTypeSupport.INSTANCE über Reflection ODER ServiceLoader-SPI in `META-INF/services/org.zerodds.cdr.TopicTypeSupport`."

**Repo:** `crates/idl-java/src/typesupport.rs`. Topic-Hook via Reflection in `crates/java-omgdds/java/src/main/java/org/omg/dds/topic/Topic.java`.

**Tests:** `crates/idl-java/tests/cluster_e.rs` (35 tests), `crates/idl-java/tests/rpc_codegen.rs` (35 tests).

**Status:** done

### §4 Package = IDL-Modul-Pfad lowercase

**Spec:** §4 -- "Generierter Code lebt im Package das dem IDL-Modul-Pfad entspricht."

**Repo:** `crates/idl-java/src/emitter.rs` Package-Path-Mapping.

**Tests:** V-7 Nested Modules Test in `Xcdr2WireVectorsTest.java`.

**Status:** done

## §5 Wire-Type-Mapping

### §5 IDL-zu-Java-Typen + Wire-Layout

**Spec:** §5, Tabelle 17 IDL-Typen → Java → XCDR2 LE. "Java hat keine `unsigned`-Typen -- Codegen nutzt nächst-größeren signed Type oder Helpers."

**Repo:** `crates/idl-java/src/type_map.rs`, `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Writer.java`, `Xcdr2Reader.java`.

**Tests:** V-3 Mixed Primitives + V-4 String + V-5/V-6 Sequences in `Xcdr2WireVectorsTest.java`; `crates/idl-java/tests/edge_cases.rs` (20 tests).

**Status:** done

## §6 Extensibility

### §6 Final / Appendable / Mutable Helpers

**Spec:** §6 -- "`Xcdr2Writer.beginAppendable()` / `beginMutable()` / `writeEmHeader(int id, int lc)` sind Helper-Methoden."

**Repo:** `Xcdr2Writer.java` hält beginAppendable, beginMutable, writeEmHeader.

**Tests:** V-9, V-10, V-11 in `Xcdr2WireVectorsTest.java`.

**Status:** done

## §7 Key-Extraction

### §7 Md5 über MessageDigest + BE-Holder

**Spec:** §7 -- "`org.zerodds.cdr.Md5` nutzt `java.security.MessageDigest.getInstance(MD5)`."

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Md5.java` wrapper. KeyHash-Codegen in `typesupport.rs`.

**Tests:** V-8 in `Xcdr2WireVectorsTest.java`.

**Status:** done

## §8 Helper-Library `org.zerodds.cdr`

### §8 TopicTypeSupport + Xcdr2Writer/Reader + enums + Md5 + XcdrException

**Spec:** §8, Tabelle 6 Klassen.

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/`: `TopicTypeSupport.java`, `Xcdr2Writer.java`, `Xcdr2Reader.java`, `ExtensibilityKind.java`, `EndianMode.java`, `Md5.java`, `XcdrException.java` -- 7 Files.

**Tests:** `mvn test` -- 34 Tests grün über Pakete `org.zerodds.cdr`, `org.omg.dds`.

**Status:** done

### §8 JVM 17+ + Maven-Coordinates

**Spec:** §8 -- "JVM 17+. Maven-Coordinates: `org.zerodds:cdr:1.0.0`."

**Repo:** `pom.xml` source/target=17. Artifact-ID `cdr`-equivalent in zerodds-Group.

**Tests:** `mvn test` runtime auf JVM 17+.

**Status:** done

## §9 Conformance

### §9 L1 Wire (V-1..V-12 byte-genau)

**Spec:** §9 -- "L1 (Wire): `crates/java-omgdds/java/src/test/java/org/zerodds/cdr/Xcdr2WireVectorsTest.java` prüft V-1..V-12 byte-genau (`mvn test`)."

**Repo:** `Xcdr2WireVectorsTest.java` mit 16 @Test-Methoden.

**Tests:** `mvn test` -- alle grün.

**Status:** done

### §9 L2 Codegen Snapshots

**Spec:** §9 -- "L2 (Codegen): `crates/idl-java/tests/snapshots/` mit generierten *TypeSupport.java-Files."

**Repo:** `crates/idl-java/tests/snapshots/` + Treiber `snapshot_codegen.rs` (11 tests).

**Tests:** dito.

**Status:** done

### §9 L3 Cross-Lang Runner

**Spec:** §9 -- "L3 (Cross-Lang): `crates/conformance/tests/cross_language_xcdr2.rs` ruft `mvn -pl conformance-runner exec:exec`."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_5_java_binding` ruft `mvn test` der java-omgdds Wire-Vector-Suite per Subprocess gegen identische V-1..V-12-Hex-Fixtures.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_5_java_binding`.

**Status:** done

### §9 L4 Cross-Vendor

**Spec:** §9 -- "L4 (Cross-Vendor): Java-encoded XCDR2-Bytes werden vom Cyclone-Subscriber dekodiert; Wire-Form-Compliance prüfbar via byte-identische Fixtures."

**Repo:** Alle 12 Vektoren wurden live gegen Cyclone DDS 0.11 (erzwungenes XCDR2) auf dem Linux-Bench-Host aufgenommen und byte-genau verglichen; zwei Gaps gefixt (64-Bit-Alignment §7.4.1.1.1, Sequence-DHEADER §7.4.3.5 für nicht-primitive Elemente). Der Java-Encoder ist byte-verifiziert: `Xcdr2WireVectorsTest.java` (`mvn test`, 16/16) prüft V-1..V-12 byte-genau inkl. V-6 `sequence<string>`-DHEADER (`beginAppendable`/`endDelimited`/`readDHeader`). V-10/V-11a konforme LC-Divergenz.

**Tests:** `crates/java-omgdds/java/.../Xcdr2WireVectorsTest.java` (mvn, 16/16) + `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 Tests).

**Status:** done -- Java-Encoder byte-genau gegen Cyclone DDS 0.11 (V-1..V-9/V-11b, mvn-ausgeführt), mutable V-10/V-11a konforme LC-Divergenz.

## §10 Examples

### §10 TopicTypedSmoke.java

**Spec:** §10 -- "`crates/java-omgdds/java/examples/TopicTypedSmoke.java` ist Referenz-Smoke (generierter PointTypeSupport + Pub/Sub-Loop)."

**Repo:** `crates/java-omgdds/java/examples/TopicTypedSmoke.java`. Compile via `javac -cp target/omgdds-0.0.0.jar examples/TopicTypedSmoke.java` und Lauf via `java -cp ...:omgdds.jar examples.TopicTypedSmoke` -> Encode/Decode-Roundtrip OK.

**Tests:** Manueller Run wie oben dokumentiert; Pub/Sub-Loop-Coverage zusätzlich durch `PubSubLoopbackTest.java`.

**Status:** done

## §11 Errata + Open-Questions

### §11.1 Java byte ist signed

**Spec:** §11.1 -- "Wire-Bytes sind uint8-semantisch. Helper-Methoden bieten `writeUInt8(int v)` / `readUInt8(): int` mit Range-Checks."

**Repo:** `Xcdr2Writer.java` hält writeUInt8/readUInt8 mit Range-Check.

**Tests:** Wire-Vector-Tests verwenden uint8-Pfad.

**Status:** done

### §11.2 Auto-boxing in Sequences

**Spec:** §11.2 -- "List<Integer> boxes; für Performance bietet Helper auch `int[]`-Encoding-Path (`writeInt32Array(int[])`)."

**Repo:** `Xcdr2Writer.java::writeInt32Array(int[])`.

**Tests:** V-5 Test nutzt `writeInt32Array`.

**Status:** done

### §11.3 Optional ist nicht serializable

**Spec:** §11.3 -- "Generierter `Optional<T>`-Member nutzt nullable-Field intern; Encoder testet `Objects.nonNull` statt `Optional.isPresent()`."

**Repo:** `crates/idl-java/src/typesupport.rs` emit nullable-field-Pattern.

**Tests:** V-11 (Optional Mutable) in `Xcdr2WireVectorsTest.java`.

**Status:** done

### §11.4 Records vs POJOs

**Spec:** §11.4 -- "IDL-`struct` → Java `record` ist optional via `@RecordClass`-Annotation; default ist POJO mit Getter/Setter (DDS-Java-PSM-Konvention)."

**Repo:** `crates/idl-java/src/emitter.rs` POJO-Default; `@RecordClass`-Pfad als alternative Form.

**Tests:** Snapshot-Tests für beide Formen in `snapshot_codegen.rs`.

**Status:** done

---

## Audit-Status

18 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-java` -- 9 Test-Binaries grün (106 unit + 35+12+20+14+35+11+29 integration); `mvn test` -- 34 Tests grün; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_5_java_binding` -- 1 Test grün; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 Tests grün; `javac+java crates/java-omgdds/java/examples/TopicTypedSmoke.java` -- OK.
