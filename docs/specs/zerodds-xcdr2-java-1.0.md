# `zerodds-xcdr2-java` v1.0 — Java TypeSupport-Codegen

ZeroDDS Vendor-Spec. Implementiert in `crates/idl-java` (Codegen) und
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/` (Helper-Pkg
`org.zerodds.cdr`). Konformanz gegen
[`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md).

## §1 Motivation

OMG **DDS-Java-PSM 1.0** definiert `org.omg.dds.topic.TopicTypeSupport<T>`
als **Marker-Interface** ohne konkrete Methoden — das Marshalling
bleibt User-Implementor oder Codegen-Output. Heute liefert
`crates/idl-java` Datenklassen, aber **keine encode/decode**.

Diese Spec spezifiziert eine konkrete `TopicTypeSupport<T>`-Erweiterung
mit allen Methoden + Codegen-Pflicht in `idl-java`.

## §2 TypeSupport-Pattern

Anchor: `org.omg.dds.topic.TopicTypeSupport<T>` (DDS-Java-PSM Marker).
ZeroDDS extends:

```java
package org.zerodds.cdr;

public interface TopicTypeSupport<T>
        extends org.omg.dds.topic.TopicTypeSupport<T> {

    String getTypeName();
    boolean isKeyed();
    ExtensibilityKind getExtensibility();

    byte[] encode(T sample);
    byte[] encode(T sample, EndianMode endian);
    T decode(byte[] bytes);
    T decode(byte[] bytes, int offset, int length);
    byte[] keyHash(T sample);  // 16 Bytes (MD5)
}

public enum ExtensibilityKind { FINAL, APPENDABLE, MUTABLE }
public enum EndianMode { LITTLE_ENDIAN, BIG_ENDIAN }
```

Generierter Code: pro IDL-`struct` eine `*TypeSupport`-Klasse die
`org.zerodds.cdr.TopicTypeSupport<T>` implementiert (Singleton via
`INSTANCE`).

## §3 Required API-Surface

```java
package com.example.generated;

import org.zerodds.cdr.*;

public final class MyTypeTypeSupport implements TopicTypeSupport<MyType> {
    public static final MyTypeTypeSupport INSTANCE = new MyTypeTypeSupport();

    @Override public String getTypeName() { return "MyType"; }
    @Override public boolean isKeyed() { return false; }
    @Override public ExtensibilityKind getExtensibility() {
        return ExtensibilityKind.FINAL;
    }

    @Override public byte[] encode(MyType s) { return encode(s, EndianMode.LITTLE_ENDIAN); }
    @Override public byte[] encode(MyType s, EndianMode endian) {
        Xcdr2Writer w = new Xcdr2Writer(endian);
        w.writeInt32(s.getX());
        w.writeInt32(s.getY());
        return w.toByteArray();
    }
    @Override public MyType decode(byte[] bytes) { return decode(bytes, 0, bytes.length); }
    @Override public MyType decode(byte[] bytes, int offset, int length) {
        Xcdr2Reader r = new Xcdr2Reader(bytes, offset, length, EndianMode.LITTLE_ENDIAN);
        MyType v = new MyType();
        v.setX(r.readInt32());
        v.setY(r.readInt32());
        return v;
    }
    @Override public byte[] keyHash(MyType s) { return new byte[16]; }
}
```

## §4 Codegen-Pflicht (idl-java)

Pro IDL-`struct` MUSS `idl-java` emittieren:

1. POJO-Klasse `MyType` mit get/set (existiert).
2. **NEU:** `MyTypeTypeSupport` implements `org.zerodds.cdr.TopicTypeSupport<MyType>`.
3. **NEU:** Topic-Constructor-Hook: `Topic<MyType>` resolved
   `MyTypeTypeSupport.INSTANCE` ueber Reflection (Klassenname
   `{TypeName}TypeSupport` im selben Package) ODER per
   ServiceLoader-SPI in `META-INF/services/org.zerodds.cdr.TopicTypeSupport`.

Generierter Code lebt im Package das dem IDL-Modul-Pfad entspricht
(z.B. `module Outer.Inner { struct S }` → `package outer.inner;
public class S { ... }; public class STypeSupport implements
TopicTypeSupport<S>`).

## §5 Wire-Type-Mapping

| IDL | Java | Wire (XCDR2 LE) |
|-----|------|-----------------|
| `boolean` | `boolean` | 1 Byte |
| `octet` | `byte` (sign-flip) | 1 Byte |
| `char` | `byte` (ASCII) | 1 Byte |
| `wchar` | `char` (UTF-16) | 2 Byte LE |
| `short` | `short` | 2 Byte LE Align(2) |
| `unsigned short` | `int` (zero-extended) | 2 Byte LE Align(2) |
| `long` | `int` | 4 Byte LE Align(4) |
| `unsigned long` | `long` (zero-extended) | 4 Byte LE Align(4) |
| `long long` | `long` | 8 Byte LE Align(8) |
| `float` | `float` | 4 Byte IEEE-754 LE |
| `double` | `double` | 8 Byte IEEE-754 LE |
| `string` | `String` (UTF-8) | uint32 length+1 + UTF-8 + NUL |
| `wstring` | `String` (UTF-16) | uint32 length + UTF-16-LE |
| `sequence<T>` | `List<T>` | uint32 count + T[] |
| `T[N]` | `T[]` (fixed-length) | T[] N Elemente |
| nested `struct U` | `U` | rekursiv `UTypeSupport.INSTANCE.encode(...)` (inline) |
| `enum E` | `enum E { A, B; static fromInt; toInt }` | int32 LE |
| `@optional T` | `Optional<T>` (oder `T` falls primitiv: `OptionalInt`) | M-Flag / present-byte |

Java hat keine `unsigned`-Typen — Codegen nutzt naechst-groesseren
signed Type oder Helpers `Integer.toUnsignedLong(...)`.

## §6 Extensibility

```java
@Override public ExtensibilityKind getExtensibility() {
    return ExtensibilityKind.FINAL; // / APPENDABLE / MUTABLE
}
```

`Xcdr2Writer.beginAppendable()` / `beginMutable()` /
`writeEmHeader(int id, int lc)` sind Helper-Methoden.

## §7 Key-Extraction

```java
@Override public byte[] keyHash(Sensor s) {
    Xcdr2Writer w = new Xcdr2Writer(EndianMode.BIG_ENDIAN);
    w.writeInt32(s.getId()); // @key
    return Md5.hash(w.toByteArray());
}
```

`org.zerodds.cdr.Md5` nutzt `java.security.MessageDigest.getInstance("MD5")`.

## §8 Helper-Library `org.zerodds.cdr`

`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/`:

| Klasse | Zweck |
|--------|-------|
| `TopicTypeSupport<T>` | Interface (extends OMG-PSM) |
| `Xcdr2Writer` | Padding + DHEADER + EMHEADER + Primitive |
| `Xcdr2Reader` | Decoder |
| `ExtensibilityKind`, `EndianMode` | enums |
| `Md5` | java.security wrapper |
| `XcdrException extends RuntimeException` | Runtime-Errors |

JVM 17+ (Java-PSM-Spec mandatiert ≥ 8, aber `record`-Klassen brauchen
17). Maven-Coordinates: `org.zerodds:cdr:1.0.0`.

## §9 Conformance

L1-L4 gegen [`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md):

- L1 (Wire): `crates/java-omgdds/java/src/test/java/org/zerodds/cdr/Xcdr2WireVectorsTest.java`
  prueft V-1..V-12 byte-genau (`mvn test`).
- L2 (Codegen): `crates/idl-java/tests/snapshots/` mit generierten
  `*TypeSupport.java`-Files.
- L3 (Cross-Lang): `crates/conformance/tests/cross_language_xcdr2.rs`
  ruft `mvn -pl conformance-runner exec:exec`.
- L4 (Cross-Vendor): Java encoded → Cyclone-Subscriber decoded via
  Pure-Java-Implementation.

## §10 Examples

`crates/java-omgdds/java/examples/TopicTypedSmoke.java` ist Referenz-
Smoke (generierter `PointTypeSupport` + Pub/Sub-Loop).

## §11 Errata + Open-Questions

- **§11.1 Java `byte` ist signed**: Wire-Bytes sind `uint8`-semantisch.
  Helper-Methoden bieten `writeUInt8(int v)` / `readUInt8(): int` mit
  Range-Checks.
- **§11.2 Auto-boxing in Sequences**: `List<Integer>` boxes; fuer
  Performance bietet Helper auch `int[]`-Encoding-Path
  (`writeInt32Array(int[])`).
- **§11.3 `Optional` ist nicht serializable**: Generierter
  `Optional<T>`-Member nutzt nullable-Field intern; Encoder testet
  `Objects.nonNull` statt `Optional.isPresent()`.
- **§11.4 Records vs POJOs**: IDL-`struct` → Java `record` ist
  optional via `@RecordClass`-Annotation; default ist POJO mit
  Getter/Setter (DDS-Java-PSM-Konvention).
