# `zerodds-xcdr2-c` v1.0 — C-FFI XCDR2-Encoding

ZeroDDS Vendor-Spec. Implementiert in `crates/zerodds-c-api/src/xcdr2.rs`.
Konformanz gegen [`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md).

## §1 Motivation

Es gibt **keine OMG-DDS-C-PSM-Spec**. Die existierende
`zerodds-c-api-1.0` deckt Entity-Lifecycle und QoS ab, aber nicht
**typisiertes** Encoding — Apps reichen heute opaque Bytes weiter.

Diese Spec ergaenzt zerodds-c-api um eine **TypeSupport-FFI** mit
function-table-basiertem Dispatch. Damit koennen C-Apps pro IDL-Type
einen vorgenerierten `zerodds_typesupport_t` registrieren und
typed-encoded Samples in C schreiben/lesen.

## §2 TypeSupport-Pattern

```c
typedef struct zerodds_typesupport_s {
    /// 16 Byte Type-Identifier (XTypes §7.3.4 EquivalenceHash).
    uint8_t type_hash[16];

    /// NUL-terminierter DDS-Type-Name (Module::Sub::Type).
    const char* type_name;

    /// is_keyed: 1 falls @key-Members vorhanden.
    uint8_t is_keyed;

    /// 0=Final, 1=Appendable, 2=Mutable.
    uint8_t extensibility;

    /// Encoder. `sample` ist Pointer auf Sprach-spezifische Repr;
    /// `out_buf` darf NULL sein um nur die benoetigte Groesse zu
    /// erfragen (`*out_len` wird gesetzt). Returns 0 ok, !=0 error.
    int (*encode)(const void* sample,
                  uint8_t* out_buf, size_t out_cap,
                  size_t* out_len);

    /// Decoder. Schreibt nach `out_sample` (Caller-allocated).
    int (*decode)(const uint8_t* buf, size_t len,
                  void* out_sample);

    /// Key-Hash (16 Byte). Schreibt nach `out_hash`.
    int (*key_hash)(const void* sample, uint8_t out_hash[16]);

    /// Sample-Free fuer dynamisch allokierte Felder (strings, sequences).
    void (*sample_free)(void* sample);
} zerodds_typesupport_t;
```

ABI-stabil. Felder sind versioniert via `zerodds_c_api_version()`.

## §3 Required FFI-Functions

```c
// Topic-Erstellung mit TypeSupport.
int zerodds_topic_create_typed(
    zerodds_participant_t* participant,
    const char* topic_name,
    const zerodds_typesupport_t* type_support,
    zerodds_topic_t** out_topic);

// Writer/Reader mit TypeSupport schreibt typisierte Samples.
int zerodds_writer_write_typed(
    zerodds_writer_t* w,
    const zerodds_typesupport_t* ts,
    const void* sample);

int zerodds_reader_take_typed(
    zerodds_reader_t* r,
    const zerodds_typesupport_t* ts,
    void* out_sample,
    zerodds_sample_info_t* out_info);

// Standalone Encoding (ohne Writer).
int zerodds_xcdr2_encode(
    const zerodds_typesupport_t* ts,
    const void* sample,
    uint8_t* out_buf, size_t out_cap,
    size_t* out_len);

int zerodds_xcdr2_decode(
    const zerodds_typesupport_t* ts,
    const uint8_t* buf, size_t len,
    void* out_sample);
```

Return-Codes (Status-Codes wie zerodds-c-api §3): 0=OK, -7=BAD_PARAMETER,
-13=BUFFER_TOO_SMALL (then `*out_len` is required size), -3=UNSUPPORTED.

## §4 Codegen-Pflicht

Pro IDL-`struct` muss ein C-Codegen (idl-c, falls existent, oder als
Aufgabe von idl-cpp via `extern "C"`-Wrapper) bereitstellen:

```c
// Generierte Datenstruktur.
typedef struct MyType_s {
    int32_t x;
    int32_t y;
} MyType_t;

// Generierter TypeSupport (statische Tabelle).
extern const zerodds_typesupport_t MyType_typesupport;
```

`MyType_typesupport` ist `static const` mit fest verdrahteten Function-
Pointers auf die generierten `MyType_encode/decode/key_hash/free`-
Funktionen.

## §5 Wire-Type-Mapping

| IDL | C99 | Wire (XCDR2 LE) |
|-----|-----|-----------------|
| `boolean` | `uint8_t` (0/1) | 1 Byte |
| `octet` | `uint8_t` | 1 Byte |
| `char` | `char` | 1 Byte |
| `wchar` | `uint16_t` | 2 Byte LE |
| `short` / `int16` | `int16_t` | 2 Byte LE Align(2) |
| `unsigned short` / `uint16` | `uint16_t` | 2 Byte LE Align(2) |
| `long` / `int32` | `int32_t` | 4 Byte LE Align(4) |
| `unsigned long` / `uint32` | `uint32_t` | 4 Byte LE Align(4) |
| `long long` / `int64` | `int64_t` | 8 Byte LE Align(8) |
| `float` | `float` | 4 Byte IEEE-754 LE |
| `double` | `double` | 8 Byte IEEE-754 LE |
| `string` | `char*` (NUL-terminiert) | uint32 length+1 + UTF-8 + NUL |
| `sequence<T>` | `struct { uint32_t len; T* elems; }` | uint32 count + T[] |
| `T[N]` | `T[N]` | T[] N Elemente |
| nested `struct U` | `U` | rekursiv `U_typesupport.encode` |

Strings + Sequences sind heap-allocated; `sample_free()` MUSS sie
freigeben.

## §6 Memory-Ownership

| API | Caller | Callee |
|-----|--------|--------|
| `zerodds_xcdr2_encode` | bietet `out_buf` (kann NULL fuer Size-Probe) | schreibt `out_len` |
| `zerodds_xcdr2_decode` | bietet `out_sample` (zero-initialized) | allokiert Strings/Sequences im Sample |
| `zerodds_reader_take_typed` | bietet `out_sample` | wie decode |
| `ts.sample_free(sample)` | nach `decode`/`take` | gibt heap-Pointer frei |

## §7 Conformance

L1-L4 gegen [`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md):

- L1 (Wire): `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` prueft
  V-1..V-12 byte-genau via FFI.
- L2 (Codegen): C-Codegen ist Teil von idl-cpp (`--c-mode`-Flag) ODER
  separate idl-c-Crate.
- L3 (Cross-Lang): C-Encoder vs Rust-Decoder, C-Decoder vs Rust-Encoder.
- L4 (Cross-Vendor): C-FFI ueber RTPS gegen Cyclone DDS.

## §8 Examples

```c
#include "zerodds.h"
#include "MyType.h" // generated: MyType_t + MyType_typesupport

int main(void) {
    zerodds_participant_t* p;
    zerodds_participant_create(0, &p);

    zerodds_topic_t* topic;
    zerodds_topic_create_typed(p, "MyTopic", &MyType_typesupport, &topic);

    // ... writer/reader create ...

    MyType_t sample = { .x = 42, .y = -7 };
    zerodds_writer_write_typed(writer, &MyType_typesupport, &sample);
    return 0;
}
```

## §9 Errata + Edge-Cases

- **§9.1 const-Strings**: `MyType_t.text` ist `char*` (mutable);
  `decode` allokiert per `malloc` und `sample_free` ruft `free`.
- **§9.2 Sequence-Bound**: Generierter `decode` prueft Bound aus IDL
  `sequence<T, N>`-Annotation und faellt bei Verletzung mit -7 zurueck.
- **§9.3 C99 vs C++ ABI**: Strukturen sind `extern "C"`-kompatibel;
  C++ Konsumenten linkten direkt gegen C-FFI.
- **§9.4 enum-Width**: C-Codegen emittiert `enum`-Typen mit explizitem
  `int32_t`-Storage (nicht `int`) fuer ABI-Stabilitaet.
