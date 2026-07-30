<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-kotlin` v1.0 — Kotlin XCDR2 TypeSupport

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md):
the Kotlin/JVM binding of the XCDR2 wire — the native `endpoints/kotlin` SDK, and
how IDL types reach Kotlin.

## §1 Motivation

OMG has no IDL-to-Kotlin mapping. ZeroDDS provides a pure-Kotlin/JVM XCDR2
wire-core (`endpoints/kotlin`), byte-identical to the Rust core.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```kotlin
data class Reading(val id: Long, val value: Float, val label: String) {
    fun marshalXCDR(endian: Endian): ByteArray {
        val w = Writer(endian)
        w.putU32(id)
        w.putF32(value)
        w.putString(label)
        return w.bytes()
    }
}
```

`uint32`/`uint64` map to `Long` (the JVM has no unsigned primitives); the wire
still writes 4/8 bytes.

## §3 Required API-Surface

`endpoints/kotlin/src/Zerodds.kt` (package `zerodds`) MUST provide: `enum class
Endian { LITTLE, BIG }`; `Writer` (`putU8/putU16/putU32/putU64/putF32/putBytes/
putString/putSeqU8`, `bytes`); `Reader` (`getU8/getU16/getU32/getU64/getF32/
getString/getSeqU8` — the byte-exact inverse, `f32` via
`Float.floatToRawIntBits`/`intBitsToFloat`). Decode is a `Reader` walk (§10);
generated decode / key hash — §11.

## §4 Codegen (inherits `idl-java`)

Kotlin has **no dedicated idl backend**: it is JVM-native and consumes the
`zerodds-idlc --java` output directly (Java interop) — the generated Java
TypeSupport is callable from Kotlin unchanged, so a separate `idl-kotlin` would
only duplicate it (see `docs/async/idlc-coverage.md`). The native
`endpoints/kotlin` wire-core is byte-identical, so hand-written Kotlin types
(§2) and idl-java-generated types share the same wire.

## §5 Wire-Type-Mapping

| IDL | Kotlin | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `Boolean` | 1 byte |
| `octet`/`uint8` | `Int` (0-255) | 1 byte |
| `char` | `Int` | 1 byte |
| `short`/`int16` | `Int` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `Int` | 2 bytes LE, align 2 |
| `long`/`int32` | `Int` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `Long` | 4 bytes LE, align 4 |
| `long long`/`int64` | `Long` | 8 bytes LE, align 4 |
| `unsigned long long`/`uint64` | `Long` | 8 bytes LE, align 4 |
| `float` | `Float` | 4 bytes IEEE-754 LE (`floatToRawIntBits`) |
| `double` | `Double` | 8 bytes IEEE-754 LE |
| `string` | `String` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `ByteArray` | uint32 count + raw bytes |

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER. `@mutable` — EMHEADER; via idl-java
codegen. The hand-written `endpoints/kotlin` types are `@final`.

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing is runtime/idl-java-provided.

## §8 Wire-Core

`endpoints/kotlin/src/Zerodds.kt` is the reference `Writer`/`Reader`,
byte-identical to `zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Endpoint:** `endpoints/kotlin/src/Test.kt` — CI job `endpoints-kotlin`.
- **Codegen:** inherited from `crates/idl-java/tests` (CI `endpoints-java-async`
  / idl-java golden vectors).

## §10 Examples

- Sync: [`endpoints/kotlin/example_sync/Example.kt`](../../endpoints/kotlin/example_sync/Example.kt)
  — poll loop, full field decode.
- Async: [`endpoints/kotlin/example_async/Example.kt`](../../endpoints/kotlin/example_async/Example.kt)
  — background thread + `LinkedBlockingQueue` channel (`samples.take()`).
- Quickstart: [`endpoints/kotlin/QUICKSTART.md`](../../endpoints/kotlin/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope for the hand-written wire-core, uniform across all
endpoints: generated decode, per-struct `keyHash`, and `@mutable`/`wchar`/
`wstring`/`map`/array/nested/union — provided via the idl-java codegen path where
needed. See the coverage doc's decision records.
