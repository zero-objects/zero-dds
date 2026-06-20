# IDL language reference

ZeroDDS implements OMG IDL 4.2 — full grammar, no shortcuts. This
chapter covers the day-to-day subset.

## Modules

```idl
module Outer {
    module Inner {
        // … types here
    };
};
```

Modules nest freely; map to Rust modules, C++ namespaces, Java
packages, C# namespaces, TypeScript namespace blocks.

## Primitive types

| IDL | Wire size | Rust | C |
|---|---|---|---|
| `boolean` | 1 byte | `bool` | `int8_t` |
| `octet` | 1 byte | `u8` | `uint8_t` |
| `char` | 1 byte | `i8` | `int8_t` (UTF-8) |
| `wchar` | 2 byte (UTF-16) | `u16` | `uint16_t` |
| `short` | 2 byte | `i16` | `int16_t` |
| `unsigned short` | 2 byte | `u16` | `uint16_t` |
| `long` | 4 byte | `i32` | `int32_t` |
| `unsigned long` | 4 byte | `u32` | `uint32_t` |
| `long long` | 8 byte | `i64` | `int64_t` |
| `unsigned long long` | 8 byte | `u64` | `uint64_t` |
| `float` | 4 byte (IEEE 754) | `f32` | `float` |
| `double` | 8 byte | `f64` | `double` |
| `long double` | 16 byte (IEEE 754 binary128) | `f128` (when stable) | `long double` |
| `string<N>` | u32 length + bytes (UTF-8) | `String` (cap N) | `char*` |
| `wstring<N>` | u32 length + UTF-16 | `String` | `wchar_t*` |

Unbounded strings: `string` (no `<N>`) → no length cap, but the
encoder still validates against `ResourceLimits.max_serialized_size`.

## Structs

```idl
@final                    // extensibility kind, see §7.2.7 of XTypes
struct Pose {
    @key string<32> id;
    double x;
    double y;
    double z;
};
```

Extensibility kinds:

| Kind | Wire | Forward-compat |
|---|---|---|
| `@final` | Tightly packed, no extra header | No |
| `@appendable` | Length-prefixed (4 bytes) | New fields can be appended |
| `@mutable` | Each field is `(member-id, length, value)` triplet | Add / remove / reorder fields |

Default (no annotation): `@appendable` per XTypes 1.3 §7.2.7.

## Unions

```idl
enum Tag { LINEAR, ANGULAR, SPATIAL };

union Velocity switch (Tag) {
    case LINEAR:
        struct Linear { double mps; };
    case ANGULAR:
        struct Angular { double rps; };
    case SPATIAL:
        Pose pose;
};
```

The discriminator (`Tag` here) is encoded first; only the
selected branch follows.

## Enums

```idl
enum Mode { IDLE, ACTIVE, FAULTED };
```

Map to Rust enums (no payload), C++ scoped enums, Java enums.
Wire-encoded as `i32` per XTypes spec.

## Typedefs

```idl
typedef sequence<double, 6> Joints;
typedef long long Nanoseconds;
```

Pure aliases. The Rust backend treats them as type aliases,
preserving the full numeric / sequence type.

## Constants

```idl
const long MAX_JOINTS = 32;
const string<16> DEFAULT_ROBOT = "robot-01";
```

Constants are emitted as `pub const` in Rust, `constexpr` in
C++17, `public static final` in Java.

## Sequences

```idl
sequence<double>           positions_unbounded;
sequence<double, 32>       positions_bounded;
sequence<sequence<long, 8>, 4> matrix_4x8;
```

Wire format: `u32` element-count + `count × element`.

## Arrays

```idl
double matrix[4][4];
```

Fixed-size, no length prefix on the wire. Map to `[[f64; 4]; 4]`
in Rust, `std::array<std::array<double, 4>, 4>` in C++.

## Bitset / bitmask

```idl
@bit_bound(8)
bitset Flags {
    bitfield<1> active;
    bitfield<1> faulted;
    bitfield<6> error_code;
};

@bit_bound(16)
bitmask FilterMask {
    POSE,
    VELOCITY,
    ACCEL,
    @position(15) FLAG_RESERVED
};
```

`bitset` = packed bitfields with explicit widths. `bitmask` = enum
where each member is a single bit. Wire-encoded as `u8 / u16 / u32 /
u64` based on `@bit_bound`.

## Forward declarations

```idl
struct Inner;            // forward decl

struct Outer {
    sequence<Inner, 8> children;
};

struct Inner {
    long id;
};
```

For mutually recursive types via `sequence` or pointer-equivalent
constructs.

## Inheritance (XTypes 1.3 §7.2.2)

```idl
@mutable
struct Base {
    @key string<32> id;
};

@mutable
struct Derived : Base {
    double extra_field;
};
```

Derived types extend Base; compatible writers / readers can speak
either as long as the extensibility allows.

## Annotations primer

The full annotation reference is in [annotations.md](annotations.md).
Most-used annotations:

| Annotation | Purpose |
|---|---|
| `@key` | Marks fields used to compute the instance KeyHash |
| `@id(N)` | Stable Member-ID for `@mutable` structs |
| `@final`, `@appendable`, `@mutable` | Extensibility |
| `@hashid` | XTypes 1.3 §7.3.4.5 — auto-derive ID from member name |
| `@optional` | Member may be absent on the wire |
| `@verbatim("language", "code")` | Inject language-specific code in the codegen output |

## Reading further

- OMG IDL 4.2 — full normative grammar.
- OMG XTypes 1.3 — extensibility, type-objects, key hashing.
- `crates/idl/src/lib.rs` — the parser and AST.
- `crates/idl-cpp/`, `idl-csharp/`, `idl-java/`, `idl-ts/` — the
  per-language backends.
