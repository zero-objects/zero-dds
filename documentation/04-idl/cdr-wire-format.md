# CDR Wire Format

Reference for the on-the-wire byte form of every IDL type encoded by
`zerodds-idlc` and decoded by every ZeroDDS binding. This document is
written for two audiences:

* Implementers of new bindings or interop bridges who need to know
  exactly how each IDL construct maps to bytes.
* Operators debugging a `KeyHash` mismatch, a wire-incompatibility
  between vendors, or a corrupted sample.

For the syntax of the IDL itself see [`language.md`](language.md);
for annotation semantics see [`annotations.md`](annotations.md); for
the compiler that produces these bytes see
[`idlc-handbook.md`](idlc-handbook.md).

---

## 1 What is CDR?

CDR is the OMG Common Data Representation — a binary serialisation
format originally specified for CORBA GIOP and inherited by DDSI-RTPS
2.5 §10 as the default DDS wire format. CDR is fixed-layout: there
is no field name on the wire, only positions. Endianness is signalled
once per buffer.

ZeroDDS implements two CDR generations:

* **XCDR1** — the original CORBA / DDS 1.4 form. Plain layout,
  no headers around aggregates. Used by every legacy DDS vendor.
* **XCDR2** — the DDS-XTypes 1.3 §7.4 form. Adds aggregate
  headers (`DHEADER`) and per-member headers (`EMHEADER`) for
  extensibility. Required for `MUTABLE` and `APPENDABLE` types.

Both generations share the same primitive encoding; they differ in
how aggregates (`struct`, `union`) are framed. A single buffer is
either XCDR1 or XCDR2 — never mixed. Selection is made at
serialisation time by the publisher and signalled to the subscriber
in the 4-byte `representation_identifier` prefix (see §3 below).

---

## 2 XCDR1 vs XCDR2 at a glance

| Aspect | XCDR1 | XCDR2 |
|---|---|---|
| Aggregate header (`DHEADER`) | none | 4 bytes for `APPENDABLE` and `MUTABLE` |
| Per-member header (`EMHEADER`) | none | 4 bytes for `MUTABLE` |
| Optional fields | not supported | 1-byte presence + value |
| Map types | not supported | length + key/value pairs |
| Default alignment | natural up to 8 | natural up to 4 (max alignment is `4` even for `int64`) |
| `enum` width | always 32-bit | follows `@bit_bound` |
| String length prefix | 32-bit | 32-bit |
| Sequence length prefix | 32-bit | 32-bit |

XCDR2 is the default for newly compiled types unless the IDL is
explicitly `@final` and the operator passes `--xcdr1`. Existing
vendor deployments that have not yet adopted XCDR2 will receive XCDR1
on a per-Topic basis if discovery negotiates it (XTypes 1.3 §7.6.5).

---

## 3 Encapsulation header

Every serialised buffer is prefixed with 4 bytes:

```
+---+---+---+---+
| EI_HI | EI_LO |  options (2 bytes)
+---+---+---+---+
```

Where `EI = representation_identifier`:

| `EI_HI` | `EI_LO` | Meaning |
|---|---|---|
| `0x00` | `0x00` | XCDR1 big-endian |
| `0x00` | `0x01` | XCDR1 little-endian |
| `0x00` | `0x02` | Parameter list big-endian (RTPS discovery) |
| `0x00` | `0x03` | Parameter list little-endian (RTPS discovery) |
| `0x00` | `0x06` | XCDR2 big-endian, `PLAIN_FINAL` |
| `0x00` | `0x07` | XCDR2 little-endian, `PLAIN_FINAL` |
| `0x00` | `0x08` | XCDR2 big-endian, `DELIMIT_APPENDABLE` |
| `0x00` | `0x09` | XCDR2 little-endian, `DELIMIT_APPENDABLE` |
| `0x00` | `0x0A` | XCDR2 big-endian, `PL_MUTABLE` |
| `0x00` | `0x0B` | XCDR2 little-endian, `PL_MUTABLE` |

`options` is reserved for XCDR2 (XTypes 1.3 §7.6.3.1.2). Bit 1 holds
the number of trailing padding bytes in the buffer (0–3); other bits
are zero.

Decoder algorithm:

```
read 4 bytes prefix
endianness = bit 0 of EI_LO (0=BE, 1=LE)
encoding   = (EI_LO >> 1) & 0x07
            // 0=XCDR1, 1=PL_DISCOVERY, 3=XCDR2_FINAL, 4=XCDR2_APPENDABLE, 5=XCDR2_MUTABLE
all subsequent reads use `endianness`
```

---

## 4 Alignment

Each field is aligned to its natural alignment relative to the start
of the encapsulation body (i.e. byte 4 of the buffer). Padding bytes
are zero.

| Type | XCDR1 alignment | XCDR2 alignment |
|---|---|---|
| `int8`, `uint8`, `octet`, `bool`, `char` | 1 | 1 |
| `int16`, `uint16` | 2 | 2 |
| `int32`, `uint32`, `float32`, `enum` | 4 | 4 |
| `int64`, `uint64`, `float64` | 8 | 4 |
| `float128` | 8 | 4 |

Note: XCDR2 caps alignment at 4 even for 8-byte primitives. This is
a deliberate change from XCDR1 — a `struct { uint8 a; int64 b; }`
takes 11 bytes in XCDR2 but 16 bytes in XCDR1.

The alignment counter resets on each `DHEADER` boundary in XCDR2
(XTypes 1.3 §7.4.4.4): the body inside a delimited aggregate is
realigned to its own start.

---

## 5 Primitive types

| IDL type | Bytes | Encoding |
|---|---|---|
| `bool` / `boolean` | 1 | `0x00` = false, `0x01` = true; other values are an error on receive |
| `octet` | 1 | raw byte |
| `int8` | 1 | two's complement |
| `uint8` | 1 | unsigned |
| `char` | 1 | ISO-8859-1 (Latin-1) |
| `wchar` | 2 | UCS-2 / UTF-16 code unit (no surrogate pair) |
| `int16` | 2 | two's complement, endianness from prefix |
| `uint16` | 2 | unsigned |
| `int32` | 4 | two's complement |
| `uint32` | 4 | unsigned |
| `int64` | 8 | two's complement |
| `uint64` | 8 | unsigned |
| `float32` | 4 | IEEE 754 binary32 |
| `float64` | 8 | IEEE 754 binary64 |
| `float128` | 16 | IEEE 754 binary128 (rarely supported) |

Endianness is governed by the encapsulation header. Within a
single buffer there is no mixing.

---

## 6 String and wstring

### 6.1 `string`

```
+--------+--------+--------+--------+
|       length (uint32)             |  4 bytes
+--------+--------+--------+--------+
| octets ...                        |  length bytes (UTF-8 by default)
+--------+--------+--------+--------+
| 0x00 (NUL terminator)             |  1 byte (included in length)
+--------+--------+--------+--------+
```

The length is the number of bytes including the trailing NUL. An
empty string is encoded as `length=1, "\0"`.

### 6.2 `wstring`

XCDR1: each character is 2 bytes (UTF-16 code unit), the length
field is the number of code units including the trailing NUL.

XCDR2: each character is encoded as UTF-8 bytes; length is byte
count including NUL — same form as `string`.

### 6.3 Bounded strings

`string<N>` and `wstring<N>` use the same wire form as their
unbounded variants. The bound is enforced on encode (length > N
fails) and on decode (length > N raises a `try_construct` error).

---

## 7 Sequence

```
+--------+--------+--------+--------+
|       length (uint32)             |  4 bytes
+--------+--------+--------+--------+
| element 0                         |  natural size + alignment
| element 1                         |
| ...                               |
+--------+--------+--------+--------+
```

The length is the number of elements (not bytes). Each element is
encoded according to its IDL type, with alignment relative to the
sequence start.

`sequence<T, N>`: bound `N` is enforced on encode; on decode an
`length > N` triggers `@try_construct(DISCARD)` by default.

---

## 8 Array

Fixed-size, no length prefix:

```
+--------+--------+--------+--------+
| element 0                         |
| element 1                         |
| ...                               |
| element N-1                       |
+--------+--------+--------+--------+
```

`T[N]` always emits exactly `N` elements. Multi-dimensional arrays
are encoded row-major (C order): `T[A][B]` is `A * B` elements with
the right-most index varying fastest.

---

## 9 Map (XCDR2 only)

```
+--------+--------+--------+--------+
|       length (uint32)             |  number of entries
+--------+--------+--------+--------+
| key 0                             |
| value 0                           |
| key 1                             |
| value 1                           |
| ...                               |
+--------+--------+--------+--------+
```

XCDR1 has no map type — IDL `map<K, V>` compiles to a sequence of
key-value structs in XCDR1 mode.

---

## 10 Optional (XCDR2 only)

```
+--------+
| flag   |  1 byte: 0x00 = absent, 0x01 = present
+--------+
| value  |  encoded only if flag == 0x01
+--------+
```

Inside a `MUTABLE` struct, optional members are typically combined
with `EMHEADER` `must_understand=0` instead — the EMHEADER's
presence already signals the field exists. The 1-byte flag is used
inside `FINAL` and `APPENDABLE` aggregates that contain optional
fields.

XCDR1 has no optional encoding — IDL `@optional` members compile to
mandatory fields with sentinel values in XCDR1 mode (and a warning
is emitted).

---

## 11 Final struct

`@final` (the default in pre-XTypes IDL):

```
+--------+--------+--------+--------+
| field 0 (aligned)                 |
| field 1 (aligned)                 |
| ...                               |
+--------+--------+--------+--------+
```

No header, no per-field framing. The decoder must know the schema
exactly — there is no way to recover from a missing or extra field.

XCDR1 always uses this layout for all structs (extensibility was not
yet specified).

---

## 12 Appendable struct

`@appendable` (the default in XTypes 1.3 unless overridden):

```
+--------+--------+--------+--------+
|     DHEADER (uint32)              |  body length in bytes
+--------+--------+--------+--------+
| field 0                           |
| field 1                           |
| ...                               |
| (extra bytes the receiver skips)  |
+--------+--------+--------+--------+
```

The `DHEADER` permits the publisher to add fields at the end of the
struct — the subscriber reads as many fields as it knows about and
skips the rest using `DHEADER - bytes_consumed`.

`DHEADER` byte count excludes the `DHEADER` itself. Maximum body
size is `2^32 - 1` bytes; in practice limited by RTPS fragment size.

---

## 13 Mutable struct

`@mutable`:

```
+--------+--------+--------+--------+
|     DHEADER (uint32)              |
+--------+--------+--------+--------+
|     EMHEADER 0 (uint32)           |
| field 0 (length-prefixed if LC>=4)|
+--------+--------+--------+--------+
|     EMHEADER 1                    |
| field 1                           |
+--------+--------+--------+--------+
| ...                               |
+--------+--------+--------+--------+
| optional sentinel EMHEADER (PID_LIST_END = 0x3F02) for XCDR1-style PL only
+--------+--------+--------+--------+
```

`EMHEADER` (XTypes 1.3 §7.4.3.4.2):

```
bits  31    must_understand flag (1 = receiver MUST recognise)
bits 30-28  Length-Code (LC) — 0..7
bits 27-0   Member-ID (28-bit, range 0..0x0FFFFFFF)
```

Length-Code encoding:

| LC | Field length |
|---|---|
| 0 | 1 byte |
| 1 | 2 bytes |
| 2 | 4 bytes |
| 3 | 8 bytes |
| 4 | length is `uint32` immediately after EMHEADER |
| 5 | length is `uint32` immediately after EMHEADER, also count of elements (sequence) |
| 6 | length is `uint32 * 4` (array of 4-byte primitives) |
| 7 | length is `uint32 * 8` (array of 8-byte primitives) |

A `MUTABLE` decoder reads `EMHEADER`s in order, looks up the member
ID in its schema, and either decodes the field or skips
`field_length` bytes if the member is unknown.

Unknown members with `must_understand=1` cause the sample to be
discarded (XTypes 1.3 §7.6.5.2 `must_understand` semantics).

---

## 14 Union

```
+--------+--------+--------+--------+
| discriminator (aligned to its type)|
+--------+--------+--------+--------+
| selected member (aligned)         |
+--------+--------+--------+--------+
```

The discriminator is a `bool`, integer, `char`, or `enum` — any
discrete primitive. The selected member is the one matching the
discriminator value, or the `default` case if no `case` matches.

Unions are always `@final` (XTypes 1.3 §7.2.2.4.4.4 — unions cannot
be `@mutable`). Therefore there is no `DHEADER`.

---

## 15 Enum

XCDR1: always 32-bit, encoded as `int32` in declaration order
starting at 0 (or at the explicit `@value(N)`).

XCDR2: width follows `@bit_bound`:

| `@bit_bound` | Width on wire |
|---|---|
| 1–8 | 1 byte |
| 9–16 | 2 bytes |
| 17–32 (default) | 4 bytes |

`@bit_bound(8)` is the most common override — a single byte for
status enums, log levels, etc.

---

## 16 Bitmask

XCDR1: encoded as `uint32` — bits set per `@position`.

XCDR2: width follows `@bit_bound`, default 32:

```idl
@bit_bound(16)
bitmask FaultFlags {
    @position(0) BATTERY_LOW,
    @position(1) MOTOR_OVERHEAT,
};
```

→ encoded as `uint16`, bit 0 = battery, bit 1 = motor.

---

## 17 Bitset

A `bitset` is a packed structure of bitfields:

```idl
bitset SensorStatus {
    bitfield<3> ready;
    bitfield<5> error_code;
    bitfield<8> reserved;
};
```

Fields are packed least-significant-bit first into the smallest
integer that fits the total bit count. `SensorStatus` above is
16 bits → encoded as a single `uint16`. Padding to alignment is
applied per the integer width.

XCDR1 and XCDR2 produce identical layouts for bitset.

---

## 18 KeyHash

`KeyHash` is computed per DDSI-RTPS 2.5 §9.6.3.8:

1. Serialise the key fields (annotated `@key`) in IDL declaration
   order using XCDR1 big-endian — regardless of the sample's actual
   encoding.
2. If the result is `<= 16` bytes, pad with `0x00` to exactly 16
   bytes.
3. If the result is `> 16` bytes, take MD5 of the serialised bytes
   (16 bytes).

The result is always exactly 16 bytes. It is published in the RTPS
`KeyHash` parameter (`PID_KEY_HASH = 0x0070`).

`zerodds-idlc generate ... --with-keyhash-md5` forces step 3 even
for short keys; this is needed for interop with vendors that always
MD5 (some legacy RTI Connext deployments).

---

## 19 Cross-vendor wire compatibility

A single `.idl` compiled by ZeroDDS produces bytes that are
byte-identical to the same IDL compiled by any conformant vendor.
The `crates/cdr` test suite verifies this against captured Cyclone
DDS, Fast-DDS, and RTI Connext byte streams.

Common interop gotchas:

* **Endianness assumption.** Some vendors hard-code little-endian
  on x86. ZeroDDS picks the local endianness for outbound and
  honours the prefix for inbound.
* **Alignment cap.** XCDR2's 4-byte cap differs from XCDR1's 8-byte.
  A type that round-trips on XCDR1 may have a different byte layout
  on XCDR2; both are correct.
* **Default extensibility.** Pre-XTypes vendors emit `@final` by
  default; XTypes 1.3 specifies `@appendable`. This produces a
  4-byte `DHEADER` difference. Either both ends use XTypes
  defaults, or both override to `@final`.
* **NUL terminator on string.** Always present, always counted in
  the length. A vendor that omits the NUL is non-conformant.
* **wstring encoding.** XCDR1 = UTF-16 code units; XCDR2 = UTF-8.
  These are byte-incompatible — the `representation_identifier` is
  the discriminator.

---

## 20 Debugging tips

### 20.1 Reading a hex dump

```
0000  00 01 00 00  10 00 00 00  68 65 6c 6c  6f 2c 20 77   ........hello, w
0010  6f 72 6c 64  00 00 00 00  ...                        orld....
```

* Bytes 0–3: `00 01 00 00` — XCDR1 little-endian, no options.
* Bytes 4–7: `10 00 00 00` — `length = 0x10` = 16 (LE). String is
  16 bytes including the trailing NUL.
* Bytes 8–23: `"hello, world\0"` followed by 3 bytes of alignment
  padding for the next field.

### 20.2 Common mismatches

* **Length-prefix off by one.** Forgot the NUL terminator. Fix:
  always include it in the count.
* **Aggregate too short.** Missing `DHEADER` on an `@appendable`
  type — receiver tries to read into the next sample. Fix: ensure
  publisher and subscriber agree on extensibility (check
  `dump-typeobject`).
* **Member ID collision.** Two `@mutable` members with the same
  `@id` cause one to be lost. Fix: use `@autoid(HASH)` or assign
  distinct `@id(N)` values.
* **`must_understand=1` for unknown member.** Receiver discards the
  sample silently (no error to application). Fix: drop
  `@must_understand` from optional members.

### 20.3 Useful tools

* `zerodds-idlc dump-typeobject <file.idl>` — print the
  `TypeObject` so you can compare schemas across vendors.
* `zerodds-recorder` + `zerodds-replay` — capture live wire
  bytes to a `.zddsrec` file, then dump frames offline.
* `wireshark` with the OMG RTPS dissector — decode RTPS packets
  including the embedded CDR payload.

---

## 21 Cross-reference

* [`language.md`](language.md) — IDL syntax.
* [`annotations.md`](annotations.md) — annotation reference.
* [`idlc-handbook.md`](idlc-handbook.md) — operator manual for the compiler.
* OMG DDS-XTypes 1.3 §7.4 — XCDR1/XCDR2 specification.
* OMG DDSI-RTPS 2.5 §10 — RTPS use of CDR.
* OMG DDSI-RTPS 2.5 §9.6.3.8 — `KeyHash` computation.
* `crates/cdr/src/lib.rs` — the canonical Rust implementation.
