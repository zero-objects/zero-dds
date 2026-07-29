<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-python` v1.0 — Python XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md):
the Python binding of the XCDR2 wire — what `zerodds-idlc --python` emits, how the
`zerodds` runtime library marshals it, and what the native `endpoints/python` SDK
provides. Unlike the thin backends, `idl-python` is a **full IDL4 DataType**
codegen (enum/union/typedef/map/array/nested/inheritance/bitmask/bitset/bounded).

## §1 Motivation

OMG has no IDL-to-Python mapping. ZeroDDS defines a Python XCDR2 binding in two
layers: the codegen backend (`crates/idl-python`) emits, per IDL type, an
`@idl_struct(...) @dataclass` class; the `zerodds` runtime library
(`crates/py/python/zerodds`, `cdr.py`/`idl.py`) marshals those reflectively,
byte-identical to the Rust `zerodds-cdr`. The native endpoint
(`endpoints/python`, ADR 0013) is a hand-written pure-Python wire-core.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }` the
`idl-python` backend emits an annotated dataclass; the runtime encodes it:

```python
@idl_struct(extensibility="final")
@dataclass
class Reading:
    id: UInt32
    value: Float32
    label: str

from zerodds import cdr
raw = cdr.encode(Reading(id=0x1000, value=20.0, label="bay-00"), endian=cdr.LE)
```

The hand-written `endpoints/python` wire-core exposes the same primitives directly
(§8) for a dependency-free endpoint:

```python
w = Writer(LE); w.put_u32(r.id); w.put_f32(r.value); w.put_string(r.label)
```

## §3 Required API-Surface

- **Codegen** (`idl-python`): per IDL type an `@idl_struct` `@dataclass` with the
  correct field brands (§5).
- **Runtime** (`zerodds.cdr`): `encode(obj, endian)` **and** `decode(cls, buf,
  endian)` — reflective marshal + unmarshal (generated decode is **done** here,
  via the runtime).
- **Endpoint** (`endpoints/python/zerodds_wire.py`): `Writer`
  (`put_u8/put_u16/put_u32/put_u64/put_bool/put_f32/put_f64/put_string/put_seq_u8`,
  DHEADER/EMHEADER helpers, `bytes`); `Reader`
  (`get_u8/get_u16/get_u32/get_u64/get_bool/get_f32/get_f64/get_string/get_seq_u8`,
  DHEADER/EMHEADER readers) — the byte-exact inverse.

## §4 Codegen-Pflicht (`idl-python`)

Per IDL construct, `zerodds-idlc --python` MUST emit the `@idl_struct`/`@dataclass`
(struct), `IntEnum` (enum), `@idl_union` factory (union), type alias (typedef),
`IntFlag` (bitmask), and the field brands for map/array/bounded types.
Extensibility drives framing (§6); `interface`/`valuetype`/`any` raise
`IdlPythonError::Unsupported` (§11 — true non-goals, not DDS DataTypes).

## §5 Wire-Type-Mapping

| IDL | Python (`idl-python` brand / `endpoints`) | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `bool` | 1 byte |
| `octet`/`uint8` | `UInt8` / `int` | 1 byte |
| `char` / `wchar` | `Char` / `WChar` | 1 byte / 2 bytes |
| `short`..`uint16` | `Int16`/`UInt16` | 2 bytes LE, align 2 |
| `long`..`uint32` | `Int32`/`UInt32` | 4 bytes LE, align 4 |
| `long long`..`uint64` | `Int64`/`UInt64` | 8 bytes LE, align 4 |
| `float` / `double` | `Float32` / `Float64` | 4 / 8 bytes IEEE-754 LE |
| `long double` | `LongDouble` (runtime brand) | 16 bytes IEEE-754 LE |
| `string` / `wstring` | `str` / `WString` | uint32 (len+1) + bytes + NUL |
| `string<N>`/`wstring<N>` | `BoundedString[N]`/`BoundedWString[N]` | bound enforced |
| `sequence<T>` | `typing.List[T]` | DHEADER (non-primitive elem) + elems |
| `sequence<octet>` | `bytes` | uint32 count + raw bytes |
| `T[N]` (array) | `Array[T, N]` brand | N × T, no length prefix |
| `map<K,V>` | `typing.Dict[K,V]` | XTypes map framing |
| `enum` | `IntEnum` subclass | Int32 |
| `union` | `@idl_union` factory | discriminator + active branch |

Byte order is an explicit parameter, so a big-endian target produces the same wire.

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER. `@mutable` — EMHEADER with per-member
`@id(N)` (XTypes 1.3 §7.4.3.4.2); `idl-python` computes the member-id list so the
EMHEADERs match cross-vendor. All three are **done** (smoke tests §9). The
hand-written `endpoints/python` wire-core supplies DHEADER/EMHEADER helpers.

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing (MD5 of key members' XCDR2-BE) is
provided by the `zerodds` runtime. Per-struct `keyHash` codegen — §11.

## §8 Wire-Core

`endpoints/python/zerodds_wire.py` is the reference pure-Python `Writer`/`Reader`
(incl. DHEADER/EMHEADER); the `zerodds.cdr` runtime marshals the codegen output.
Both byte-identical to `zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Codegen + runtime:** `crates/idl-python/tests/smoke.rs` (structural) +
  `gen_for_pytest.rs` → `python/tests` (`pytest`, encode/decode round-trip) — CI
  jobs `python-tests`.
- **Endpoint:** `endpoints/python/test_byte_identity.py` (+ `test_endpoint`,
  `test_mutable`, `test_nested`).

## §10 Examples

- Sync: [`endpoints/python/example_sync.py`](../../endpoints/python/example_sync.py)
  — poll loop, full field decode.
- Async: [`endpoints/python/example_async.py`](../../endpoints/python/example_async.py)
  — `asyncio` async generator (`async for body in reader.stream()`).
- Quickstart: [`endpoints/python/QUICKSTART.md`](../../endpoints/python/QUICKSTART.md).

## §11 Errata + Open-Questions

- **`interface` / `valuetype` / `any`** — raise `Unsupported`. True non-goals:
  these are RPC/OO/dynamic constructs, not DDS DataTypes; out of scope for a
  data-type wire binding.
- **per-struct `keyHash` codegen** — open (roadmap); the runtime computes key
  hashes today.

Unlike the thin backends, `idl-python` has **no** `long double` blocker (the
runtime `LongDouble` brand does not depend on Rust `f128`) and **no** open
enum/union/typedef/map/array/nested gap — those are done (§9). See the coverage doc.
