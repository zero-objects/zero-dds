# `zerodds-xcdr2-cpp` v1.0 — C++17 TypeSupport-Codegen

ZeroDDS Vendor-Spec. Implementiert in `crates/idl-cpp` (Codegen) und
`crates/cpp/include/dds/topic/` (Helper-Header). Konformanz gegen
[`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md).

## §1 Motivation

OMG **DDS-PSM-Cxx 1.0** (formal/2022-04-04) mandatiert
TypeSupport-Registrierung in `Participant::create_topic`, spezifiziert
aber **kein** konkretes `topic_type_support<T>`-Trait. RTI Connext und
eProsima Fast-DDS nutzen jeweils eigene Forms (`*TypeSupport`-Klassen
mit unterschiedlichen Method-Names).

Ohne diese Spec liefert `idl-cpp` zwar Datenklassen, aber **keine
Encoding-Methoden** — Apps muessten `serialize/deserialize` von Hand
schreiben (siehe Status vor v1.0: leerer Codegen, aspirational
Header-Comment in `TopicTraits.hpp`).

## §2 TypeSupport-Pattern

```cpp
namespace dds {
namespace topic {

template <typename T>
struct topic_type_support;          // forward (Spezialisierung pro T)

} // namespace topic
} // namespace dds
```

Spezialisierung pro IDL-`struct` durch idl-cpp emittiert.

## §3 Required Methods

```cpp
template <>
struct topic_type_support<MyType> {
    /// DDS-Type-Name (Module::Sub::Type, ASCII).
    static const char* type_name();

    /// Encoder. PLAIN_CDR2 LE default; `endian` parameter optional.
    static std::vector<uint8_t> encode(const MyType& v);
    static std::vector<uint8_t> encode_be(const MyType& v); // optional

    /// Decoder. Wirft `dds::core::Error` bei Buffer-Underrun.
    static MyType decode(const uint8_t* buf, size_t len);

    /// Key-Hash (16 Bytes MD5). All-Zero falls !is_keyed().
    static std::array<uint8_t, 16> key_hash(const MyType& v);

    /// Hat `MyType` mindestens ein @key-Member?
    static constexpr bool is_keyed();

    /// Final / Appendable / Mutable.
    static constexpr ::dds::core::policy::DataRepresentationKind extensibility();
};
```

`is_keyed` und `extensibility` als `constexpr` damit Topic-Constructor
sie zur Compile-Zeit prueft.

## §4 Codegen-Pflicht (idl-cpp)

Pro IDL-`struct` (Top-Level oder Modul-nested) MUSS `idl-cpp` eine
`topic_type_support<FQN>`-Spezialisierung emittieren. FQN ist
`::Module::Sub::Struct` (full-qualified, mit `::`-Prefix).

Pflicht-Members (aus AST):
- Alle Plain-Members (Primitive, String, Sequence, Nested-Struct, Enum,
  Array).
- `@optional` / `@shared` ueber EMHEADER M-Flag (Mutable) bzw.
  Present-Flag-Byte (Final/Appendable).
- `@key`-Members in Key-Hash-Generation.

Type-Name-Form: `"Module::Sub::Struct"` ohne fuehrendes `::`.

Beispiel-Output fuer `module Outer { struct S { long x; }; }`:

```cpp
namespace dds {
namespace topic {

template <>
struct topic_type_support<::Outer::S> {
    static const char* type_name() { return "Outer::S"; }
    static std::vector<uint8_t> encode(const ::Outer::S& v) {
        std::vector<uint8_t> out;
        ::dds::topic::xcdr2::write_le<int32_t>(out, v.x());
        return out;
    }
    static ::Outer::S decode(const uint8_t* buf, size_t len) {
        size_t pos = 0;
        ::Outer::S v;
        v.x(::dds::topic::xcdr2::read_le<int32_t>(buf, pos, len));
        return v;
    }
    static std::array<uint8_t, 16> key_hash(const ::Outer::S&) {
        return {{0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0}};
    }
    static constexpr bool is_keyed() { return false; }
    static constexpr ::dds::core::policy::DataRepresentationKind extensibility() {
        return ::dds::core::policy::DataRepresentationKind::FINAL;
    }
};

} // namespace topic
} // namespace dds
```

## §5 Wire-Type-Mapping

| IDL | C++17 | Wire (XCDR2 LE) |
|-----|-------|-----------------|
| `boolean` | `bool` | 1 Byte (0/1) |
| `octet` | `uint8_t` | 1 Byte |
| `char` | `char` | 1 Byte |
| `wchar` | `wchar_t` | 2 Byte LE |
| `short` / `int16` | `int16_t` | 2 Byte LE, Align(2) |
| `unsigned short` / `uint16` | `uint16_t` | 2 Byte LE, Align(2) |
| `long` / `int32` | `int32_t` | 4 Byte LE, Align(4) |
| `unsigned long` / `uint32` | `uint32_t` | 4 Byte LE, Align(4) |
| `long long` / `int64` | `int64_t` | 8 Byte LE, Align(8) |
| `unsigned long long` / `uint64` | `uint64_t` | 8 Byte LE, Align(8) |
| `float` | `float` | 4 Byte IEEE-754 LE, Align(4) |
| `double` | `double` | 8 Byte IEEE-754 LE, Align(8) |
| `string` | `std::string` | uint32 length+1 + UTF-8 + NUL, Align(4) |
| `wstring` | `std::wstring` | uint32 length + UTF-16-LE Code-Units, Align(4) |
| `sequence<T>` | `std::vector<T>` | uint32 count + T[] |
| `T[N]` | `std::array<T, N>` | T[] (N Elemente, kein Length) |
| nested `struct U` | `U` | rekursiv `topic_type_support<U>::encode` |
| `enum E` | `E` | int32 LE, Align(4) (per `@bit_bound` veraenderlich) |
| `@optional T` | `std::optional<T>` | M-Flag (Mutable) oder 1-Byte present (Final/Appendable) |
| `@external T` | `std::shared_ptr<T>` | wie Plain-Member (Heap-Indirektion ist Sprach-Detail) |

## §6 Extensibility

Codegen-Default: **`@appendable`** (DDS 1.4 §B.4.1).

```cpp
// @final
static constexpr extensibility() { return FINAL; }
// kein DHEADER, kein EMHEADER. Encoder schreibt Plain-CDR2.

// @appendable (default)
static constexpr extensibility() { return APPENDABLE; }
// DHEADER (4 Byte uint32 = body-size) prefixed.

// @mutable
static constexpr extensibility() { return MUTABLE; }
// PL_CDR2: DHEADER + EMHEADER pro Member.
```

Helper-Library liefert pro Mode dedicated Writer-Klassen:
`xcdr2::FinalWriter`, `xcdr2::AppendableWriter`, `xcdr2::MutableWriter`.

## §7 Key-Extraction

Per XTypes §7.6.8: `PlainCdr2BeKeyHolder` (Big-Endian Plain-CDR2 ueber
nur `@key`-Members), MD5 davon, 16 Bytes.

```cpp
static std::array<uint8_t, 16> key_hash(const Sensor& v) {
    std::vector<uint8_t> holder;
    ::dds::topic::xcdr2::write_be<int32_t>(holder, v.id()); // @key
    return ::dds::topic::xcdr2::md5(holder);
}
```

Wenn `is_keyed() == false`: 16 Null-Bytes zurueck.

## §8 Helper-Library

`crates/cpp/include/dds/topic/`:

| Header | Inhalt |
|--------|--------|
| `TopicTraits.hpp` | `topic_type_support<T>` forward + ByteSeq/string Defaults |
| `xcdr2.hpp` | Primitive-Helpers (`write_le`, `read_le`, Padding, DHEADER, EMHEADER) |
| `xcdr2_md5.hpp` | MD5 fuer Key-Hash (RFC 1321, public-domain) |

Pure C++17, header-only. Kein Linking gegen Rust-Layer; Cross-Compile-fest.

## §9 Conformance

L1-L4 gegen [`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md):

- L1 (Wire): `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` prueft alle
  V-1..V-12 byte-genau.
- L2 (Codegen): `crates/idl-cpp/tests/snapshots/` enthaelt Snapshot
  pro Vektor.
- L3 (Cross-Lang): `crates/conformance/tests/cross_language_xcdr2.rs`
  ruft `cpp_runner` auf.
- L4 (Cross-Vendor): `crates/discovery/tests/cyclone_xcdr2_cpp.rs`
  Roundtrip vs. Cyclone.

## §10 Examples

`crates/cpp/examples/topic_typed_smoke.cpp` ist der Referenz-Smoke
(generierter `topic_type_support<Point>` + Pub/Sub-Loop).

## §11 Errata + Open-Questions

- **§11.1 wchar**: C++ `wchar_t` ist plattformabhaengig (4 Byte unter
  Linux, 2 Byte unter Windows). XTypes §7.4.4.6 verlangt 2 Byte UTF-16-LE
  auf der Wire. Codegen MUSS via `static_cast<uint16_t>` truncieren und
  bei Decode auf-extenden.
- **§11.2 long double**: XCDR2 spezifiziert kein `long double`-Wire.
  Codegen lehnt `long double`-Member mit Fehler ab.
- **§11.3 std::variant fuer union**: idl-cpp emittiert IDL-`union`
  als `std::variant<...>`. Encoder schreibt Discriminator + selected-
  case nach XTypes §7.4.4.5.
