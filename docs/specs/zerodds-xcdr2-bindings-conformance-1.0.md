# `zerodds-xcdr2-bindings-conformance` v1.0 — Cross-Language Conformance

ZeroDDS Vendor-Spec. Single-Source-of-Truth fuer alle Sprach-Bindings
(`zerodds-xcdr2-{cpp,c,csharp,java,ts,rust}-1.0`).

## Motivation

OMG XTypes 1.3 §7.4 spezifiziert das XCDR1/XCDR2 Wire-Format byte-
genau. Aber **keine OMG-Spec** sagt:

- Wie ein Codegen pro Sprache `encode/decode` emittiert.
- Welches Trait/Interface die Sprach-Bindings tragen.
- Welche Method-Signaturen verbindlich sind.
- Wie Cross-Language-Roundtrip-Konformanz verifiziert wird.

RTI Connext, eProsima Fast-DDS und Eclipse Cyclone haben jeweils
eigene Patterns mit unterschiedlichen Method-Names, Trait-Layouts
und Type-Name-Konventionen. Cross-Vendor-Interop wird allein ueber
die Wire-Bytes erreicht; pro-Sprache-Code ist nicht portabel.

ZeroDDS schliesst die Luecke mit sechs Vendor-Specs (cpp, c, csharp,
java, ts, rust) — alle gegen **dieses Conformance-Dokument** geprueft.

## §1 Conformance-Levels

| Level | Anforderung |
|-------|-------------|
| **L1 — Wire** | Encoder/Decoder produziert/konsumiert XCDR2 §7.4 byte-genau. Pflicht-Pruefung gegen die §6-Wire-Test-Vektoren. |
| **L2 — Codegen** | Sprach-Codegen (idl-cpp/csharp/java/ts/rust) emittiert pro IDL-`struct` eine TypeSupport-Spezialisierung mit allen §3-Methoden. |
| **L3 — Cross-Language** | Bytes von Sprache A werden von Sprache B byte-identisch round-trippt fuer alle §6-Test-Types. |
| **L4 — Cross-Vendor** | Bytes von ZeroDDS werden von Cyclone DDS akzeptiert und vice versa. Pflicht fuer L3-zertifizierte Bindings. |

Eine Sprach-Spec ist **conform** wenn alle L1-L4 erfuellt sind.

## §2 Gemeinsames TypeSupport-Schema

Jede Sprach-Spec definiert eine `TypeSupport<T>`-Form (Trait, Interface,
Klasse, je nach Sprach-Idiom) mit den folgenden semantischen Methoden:

| Semantik | Pflicht | Beschreibung |
|----------|---------|--------------|
| `type_name()` | ja | Liefert den DDS-Type-Name als String (Convention: `Module::Sub::Struct`, ASCII, max 256 Bytes). |
| `encode(v) -> bytes` | ja | XCDR2 §7.4 Big-Endian (default) oder Little-Endian (Option). Ohne RTPS-Header, nur Payload. |
| `decode(bytes) -> v` | ja | Inverse zu `encode`. Gibt strukturiertes Sample-Object zurueck. |
| `key_hash(v) -> [u8; 16]` | bei `@key` | MD5 ueber `PlainCdr2BeKeyHolder` der `@key`-Felder; voll auf Null wenn kein Key (XTypes §7.6.8). |
| `is_keyed()` | ja | `true` falls mindestens ein Member `@key` traegt. |
| `extensibility_kind()` | ja | `Final` / `Appendable` / `Mutable` (XTypes §7.2.2.4.4). |

Sprach-spezifische Method-Names siehe pro Sprache. Semantik MUSS
identisch sein.

## §3 Wire-Format-Anker

Alle Sprach-Encoder produzieren XCDR2 gemaess **OMG XTypes 1.3 §7.4**
(formal/2025-04-04, ISBN 9780999998557). Insbesondere:

- §7.4.1 Encoding-Algorithmus.
- §7.4.1.5 Padding-Regeln (Alignment relativ zum Buffer-Start).
- §7.4.2 PL_CDR2 fuer Mutable-Types (EMHEADER).
- §7.4.4.4 DHEADER fuer Appendable-Types.
- §7.6.8 Key-Hash-Berechnung mit `PlainCdr2BeKeyHolder`.
  **§7.6.8.4 Pflicht**: Wenn der Holder ≤ 16 octets gross ist, ist
  `Key_Hash` der Holder-Inhalt mit zero-padding auf 16 octets.
  Sonst ist `Key_Hash` der MD5 des Holder-Inhalts. **MD5 ist NICHT
  unconditional** — konditional pro Holder-Groesse.
- §7.4.3 Encoding-Header (4-Byte) fuer Wire-Frames mit Encapsulation.

Default-Encoding: **PLAIN_CDR2 LE** (`0x00 0x01 0x00 0x00`).
Big-Endian-Variante via Caller-Option.

## §4 Annotations-Mapping

| IDL-Annotation | Wire-Effekt | Konformanz-Pflicht |
|----------------|-------------|--------------------|
| `@final` | PLAIN_CDR2, kein DHEADER | ja |
| `@appendable` (default) | DELIMITED_CDR2 mit DHEADER | ja |
| `@mutable` | PL_CDR2 mit EMHEADER pro Member | ja |
| `@key` | Member geht in Key-Hash | ja |
| `@id(N)` | EMHEADER member-id | ja |
| `@optional` | EMHEADER M-Flag (Mutable); fuer Final/Appendable: present-flag-byte | ja |
| `@bit_bound` | Bit-Limit fuer Bitset/Bitmask | ja |
| `@external` | Members als heap-stored (z.B. `Box<T>` in Rust) | ja |
| `@must_understand` | EMHEADER MU-Flag | ja |

Die volle Tabelle pro Sprache steht im jeweiligen `zerodds-xcdr2-*-1.0`-
Dokument in §4.

## §5 Type-Name-Konvention

DDS-Type-Names landen in PID_TYPE_NAME (Discovery) und in
TypeIdentifier-Lookup. Konvention Cross-Sprache:

```
"<Module1>::<Module2>::<Struct>"
```

Beispiele:
- `struct Point` (kein Modul) → `"Point"`.
- `module Outer { struct S }` → `"Outer::S"`.
- `module Outer { module Inner { struct S }}` → `"Outer::Inner::S"`.

Trennzeichen MUSS `::` sein (kein `/`, `.`, `_`). Encoding ASCII oder
UTF-8 (XTypes §7.3.1.1.1 erlaubt UTF-8).

## §6 Wire-Test-Vektoren

Pflicht-Konformanz gegen folgenden Korpus. Bytes sind **PLAIN_CDR2 LE**
ohne 4-Byte-Encoding-Header (Header wird vom RTPS-Layer prepended).

### V-1 Empty Final Struct

IDL:
```idl
@final
struct Empty {};
```

Wire (0 Bytes Payload):
```
(empty)
```

Type-Name: `"Empty"`.

### V-2 Plain Primitives Final

IDL:
```idl
@final
struct Point {
    long x;
    long y;
};
```

Sample: `Point{ x = 1, y = -2 }`.

Wire (8 Bytes):
```
01 00 00 00  FE FF FF FF
```

Type-Name: `"Point"`.

### V-3 Mixed Primitives Final

IDL:
```idl
@final
struct All {
    boolean b;
    octet   o;
    short   s;
    unsigned short us;
    long    l;
    unsigned long ul;
    long long ll;
    unsigned long long ull;
    float   f;
    double  d;
};
```

Sample: `b=true o=0xAB s=-12345 us=54321 l=-1234567 ul=2345678 ll=-987654321 ull=123456789 f=2.5 d=3.14159`.

Wire (48 Bytes, Padding gemaess §7.4.1.5 origin-relativ; b@0 o@1 s@2 us@4 pad@6 l@8 ul@12 ll@16 ull@24 f@32 pad@36 d@40):
```
01 AB                             # b@0, o@1
C7 CF                             # s@2 = -12345
31 D4                             # us@4 = 54321
00 00                             # pad@6 (2 Byte) zu Align(4) fuer l@8
79 29 ED FF                       # l@8 = -1234567 (LE)
CE CA 23 00                       # ul@12 = 2345678
4F 97 21 C5 FF FF FF FF           # ll@16 = -987654321
15 CD 5B 07 00 00 00 00           # ull@24 = 123456789
00 00 20 40                       # f@32 = 2.5
00 00 00 00                       # pad@36 (4 Byte) zu Align(8) fuer d@40
6E 86 1B F0 F9 21 09 40           # d@40 = 3.14159
```

Type-Name: `"All"`.

### V-4 String Final

IDL:
```idl
@final
struct Greeting {
    string text;
};
```

Sample: `text="hello"`.

Wire (`uint32 length + bytes + NUL` per XTypes §7.4.4.6, length inkl. NUL):
```
06 00 00 00  68 65 6C 6C 6F 00
```

Type-Name: `"Greeting"`.

### V-5 Sequence<int32> Final

IDL:
```idl
@final
struct Bag {
    sequence<long> ids;
};
```

Sample: `ids = [1, 2, 3]`.

Wire (`uint32 count + element[]`, kein Padding zwischen Count und Elements weil count ist 4-Byte-aligned):
```
03 00 00 00  01 00 00 00  02 00 00 00  03 00 00 00
```

Type-Name: `"Bag"`.

### V-6 Sequence<string> Final

IDL:
```idl
@final
struct Tags {
    sequence<string> tags;
};
```

Sample: `tags = ["a", "bc"]`.

Wire:
```
02 00 00 00          # 2 Strings
02 00 00 00 61 00    # "a\0"
00 00                # 2-Byte-Pad zu Align(4) fuer naechsten string-length
03 00 00 00 62 63 00 # "bc\0"
```

Type-Name: `"Tags"`.

### V-7 Nested Modules Final

IDL:
```idl
module Outer {
    module Inner {
        @final
        struct S { long x; };
    };
};
```

Sample: `Outer::Inner::S{ x = 1234 }`.

Wire (4 Bytes):
```
D2 04 00 00
```

Type-Name: `"Outer::Inner::S"`.

### V-8 Keyed Struct (Final)

IDL:
```idl
@final
struct Sensor {
    @key long id;
    double value;
};
```

Sample: `Sensor{ id = 42, value = 3.14 }`.

Wire (16 Bytes, Padding fuer double-Alignment):
```
2A 00 00 00          # id = 42
00 00 00 00          # 4-Byte-Pad zu Align(8)
1F 85 EB 51 B8 1E 09 40  # value = 3.14
```

Key-Hash-Eingabe (`PlainCdr2BeKeyHolder`, BE):
```
00 00 00 2A
```

Key-Hash (zero-padded auf 16 Bytes, gemaess XTypes §7.6.8.4 weil
Holder-Groesse 4 ≤ 16):
```
00 00 00 2A 00 00 00 00 00 00 00 00 00 00 00 00
```

**Hinweis:** MD5 wird **nur** verwendet wenn der Holder > 16 octets ist
(XTypes §7.6.8.4). Bei kleinen Keys ist der Holder selbst der Hash mit
zero-padding. Bindings die unconditional MD5 emittieren sind
spec-non-conform.

Type-Name: `"Sensor"`.

### V-9 Appendable Struct

IDL:
```idl
@appendable
struct V {
    long a;
    long b;
};
```

Sample: `V{ a=1, b=2 }`.

Wire (DHEADER + Plain-Body):
```
08 00 00 00          # DHEADER: object size = 8
01 00 00 00          # a
02 00 00 00          # b
```

Type-Name: `"V"`.

### V-10 Mutable Struct

IDL:
```idl
@mutable
struct M {
    @id(1) long a;
    @id(2) string b;
};
```

Sample: `M{ a=42, b="hi" }`.

Wire (PL_CDR2; DHEADER zaehlt **alle** nachfolgenden Body-Bytes;
EMHEADER ist ambient-endian per XTypes §7.4.3.4.5; Reference-Encoder
zerodds_cdr nutzt **LC=4** universell mit NEXTINT-Prefix):
```
1B 00 00 00              # DHEADER object-size = 27
01 00 00 40              # EMHEADER LE: M=0 LC=4 id=1 (u32=0x40000001)
04 00 00 00              # NEXTINT = 4
2A 00 00 00              # a = 42
02 00 00 40              # EMHEADER LE: M=0 LC=4 id=2 (u32=0x40000002)
07 00 00 00              # NEXTINT = 7
03 00 00 00 68 69 00     # string "hi\0"
```

Body-Laenge: 12(member1: EMHEADER+NEXTINT+a) + 15(member2: EMHEADER+NEXTINT+string) = 27.

**Encoder-Wahl:** LC=2 (fuer 4-Byte primitive ohne NEXTINT) und LC=3
(NEXTINT-prefixed fuer variable-size) sind ebenfalls XTypes-conform.
Decoder MUSS alle LCs (0-7) akzeptieren. Cross-Vendor-Wire-Bytes
folgen dem Reference-Encoder zerodds_cdr (LC=4 universell).

Type-Name: `"M"`.

### V-11 Optional Member (Mutable)

IDL:
```idl
@mutable
struct O {
    @id(1) @optional long maybe;
};
```

Sample-A `O{ maybe = Some(7) }` (Reference LC=4 mit NEXTINT):
```
0C 00 00 00          # DHEADER = 12 (4 EMHEADER + 4 NEXTINT + 4 value)
01 00 00 40          # EMHEADER LE: M=0 LC=4 id=1 (u32=0x40000001)
04 00 00 00          # NEXTINT = 4
07 00 00 00          # value = 7
```

Alternativ (encoder LC=2, kompakter):
```
08 00 00 00          # DHEADER = 8 (4 EMHEADER + 4 value)
01 00 00 20          # EMHEADER LE: M=0 LC=2 id=1 (u32=0x20000001)
07 00 00 00          # value = 7
```

Sample-B `O{ maybe = None }`:
```
00 00 00 00          # DHEADER = 0
```

Type-Name: `"O"`.

### V-12 Mutable Sentinel End-Marker

Mutable-Streams enden mit dem PID_LIST_END-Sentinel (XCDR1) bzw.
implizit beim DHEADER-Bound (XCDR2). XCDR2-Bindings DUERFEN keinen
expliziten Sentinel emittieren — die DHEADER-Groesse begrenzt das Lesen.

## §7 Cross-Language-Roundtrip-Tests

`crates/conformance` haelt eine deklarative Test-Matrix:

```
tests/xcdr2_cross_language/
├── vectors.json         # alle V-1 .. V-12 als (idl, sample, wire-hex)
├── runner_cpp.sh        # encode-cpp.cpp / decode-cpp.cpp
├── runner_csharp.sh
├── runner_java.sh
├── runner_ts.sh
├── runner_rust.rs
└── runner_c.sh          # zerodds-c-api FFI
```

Pro Sprache:
1. Sample-Konstruktion via Codegen-Output.
2. Encode → Bytes vergleichen mit `wire-hex`.
3. Decode `wire-hex` → Re-Encode → Bytes vergleichen (Roundtrip).
4. Cross-Language-Pruefung: Encode in Sprache A → Decode in Sprache B.

Conformance pro Binding: alle V-1..V-12 muessen pruefen.

## §8 Cross-Vendor (Cyclone DDS)

Pflicht ab L4. Test-Skript `tests/interop/xcdr2_cross_vendor.sh`
erwartet:

- Cyclone-Encoder produziert die gleichen Bytes fuer V-1..V-12 wie
  ZeroDDS (modulo Type-Name-Mapping).
- ZeroDDS-Subscriber decoded Cyclone-Bytes ohne Verlust.
- ZeroDDS-Publisher publiziert; Cyclone-Subscriber verifiziert.

Die Cyclone-Test-Vektoren liegen in
`crates/discovery/tests/fixtures/cyclone-xcdr2/*.bin`.

## §9 Errata + Edge-Cases

- **§9.1 wstring**: XTypes §7.4.4.6 spezifiziert `wstring` als
  UTF-16-LE auf der Wire mit 2-Byte-Code-Units. Sprach-Bindings
  MUESSEN UTF-16 emittieren, **nicht** UTF-8.
- **§9.2 Empty Mutable**: DHEADER = 0 ist legal; kein EMHEADER folgt.
- **§9.3 Sequence-Bound-Overflow**: Decoder MUSS `bound`-Annotation
  pruefen und bei Verletzung Fehler werfen (XTypes §7.2.2.4.4.4.10).
- **§9.4 Cycle-Sample**: Self-referenzierende Types (`@external`)
  brauchen Heap-Indirektion; Wire-Format identisch zu Plain-Member.

## §10 Versioning

Diese Spec ist `1.0`. Inkompatible Aenderungen → `2.0`.
Backward-Compatible (z.B. neue Test-Vektoren V-13ff) → `1.x`.
Verlangtes Wire-Format-Behavior bleibt **immer** XTypes 1.3 §7.4.

## §11 Cross-Reference

| Sprach-Spec | Datei |
|-------------|-------|
| C++ | `docs/specs/zerodds-xcdr2-cpp-1.0.md` |
| C-FFI | `docs/specs/zerodds-xcdr2-c-1.0.md` |
| C# | `docs/specs/zerodds-xcdr2-csharp-1.0.md` |
| Java | `docs/specs/zerodds-xcdr2-java-1.0.md` |
| TypeScript | `docs/specs/zerodds-xcdr2-ts-1.0.md` |
| Rust | `docs/specs/zerodds-xcdr2-rust-1.0.md` |

Die Wire-Test-Vektoren leben in genau **dieser** Datei. Sprach-Specs
referenzieren §6 und ergaenzen nur Sprach-spezifische Sektionen.
