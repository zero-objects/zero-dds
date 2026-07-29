# `zerodds-xcdr2-kotlin` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-kotlin-1.0.md` — the ZeroDDS Kotlin XCDR2 TypeSupport codegen spec.

Implementation:

- `crates/idl-java/` — IDL → Kotlin codegen (`marshalXCDR`, self-contained writer).
- `endpoints/kotlin/` — pure-Kotlin/JVM XCDR2 wire core (writer/reader) + sync/async SDK.

## §1 Motivation

### §1 No OMG-IDL-to-D wire mapping

**Spec:** §1 — ZeroDDS defines the D XCDR2 wire mapping.

**Repo:** the motivation text of the vendor spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal pattern

### §2 struct + `marshalXCDR(endian, allocator) ![]u8`

**Spec:** §2 — per IDL `@final struct` a D `struct` + `marshalXCDR` method.

**Repo:** `crates/idl-java/src/emitter.rs::emit_struct`.

**Tests:** `crates/idl-java/tests/golden.rs::final_struct_emits_type_and_marshal`

**Status:** done

## §3 Required API surface

### §3 Writer/reader primitives + generated marshalXCDR

**Spec:** §3 — writer (putU8..putSeqU8, bytes), reader (getU8/getU16/getU32/getU64/getF32/getString/getSeqU8), generated `marshalXCDR`.

**Repo:** `endpoints/kotlin/src/Zerodds.kt` (writer, reader incl. getU8/getU16/getU64/getF32/getString/getSeqU8 (a byte-exact inverse)); `crates/idl-java/src/emitter.rs` (marshalXCDR).

**Tests:** `endpoints/kotlin/src/Zerodds.kt` Test.kt (byte identity + sync + async), `endpoints/kotlin/example_sync|async/Example.kt` (full field decode), `crates/idl-java/tests/golden.rs`.

**Status:** done

### §3.a Generated `unmarshalXCDR` (decode codegen)

**Spec:** §3/§11 — decode runs via `Reader`; a generated `unmarshalXCDR` is missing.

**Repo:** — (decode via `Reader`; see examples.)

**Tests:** `endpoints/kotlin/example_sync|async/Example.kt`.

**Status:** n/a (rejected)

**Decision record:** encode codegen (`marshalXCDR`) + a full `Reader` for decode; generated decode is deliberately not part of this binding spec across all 17 idlc backends (the `Reader` covers every field type byte-identically), it belongs to the optional full TypeSupport codegen (roadmap).

## §4 Codegen requirement (`idl-java`)

### §4 struct + marshalXCDR + embedded writer

**Spec:** §4 — per struct: D struct, `marshalXCDR`, self-contained writer.

**Repo:** `crates/idl-java/src/emitter.rs`; `tools/idlc` `Backend::D`, `--java` (JVM interop).

**Tests:** `crates/idl-java/tests/golden.rs`.

**Status:** done

## §5 Wire type mapping

### §5 IDL → Kotlin → XCDR2 (alignment cap 4)

**Spec:** §5 — bool/octet/char/short..long long/float/double/string/sequence<octet> with exact wire layout; signed via Float.floatToRawIntBits.

**Repo:** `crates/idl-java/src/emitter.rs::map_type/map_primitive/map_integer/map_sequence`; `endpoints/kotlin/src/Zerodds.kt` (Put*/getLE align cap 4).

**Tests:** `crates/idl-java/tests/golden.rs::byte_identity_vs_rust_goldens` (@final LE+BE).

**Status:** done

### §5.a wchar / wstring / long double / map / array / nested / union

**Spec:** §5/§11 — further IDL constructs.

**Repo:** — (`idl-java codegen::Unsupported`).

**Tests:** —

**Status:** n/a (rejected)

**Decision record:** deliberately out of v1.0 scope, uniform across all 17 backends (the core = the primitive/string/sequence<octet> of the @final golden). Roadmap-tracked; backends honestly return `Unsupported` instead of a wrong wire.

## §6 Extensibility

### §6 @final (compact) + @appendable (DHEADER)

**Spec:** §6 — @final without DHEADER, @appendable with a uint32 body length + body.

**Repo:** `crates/idl-java/src/emitter.rs::emit_struct` (final/appendable).

**Tests:** `crates/idl-java/tests/golden.rs::final_struct_...` + `appendable_struct_frames_a_dheader`.

**Status:** done

### §6.a @mutable (EMHEADER)

**Spec:** §6 — @mutable EMHEADER framing.

**Repo:** — (`Unsupported`).

**Tests:** —

**Status:** n/a (rejected)

**Decision record:** @mutable EMHEADER is not emitted uniformly across all 17 backends; @final+@appendable cover the common case. A cross-cutting follow-up (roadmap), deliberately deferred. `idl-java` returns `Unsupported`.

## §7 Key extraction

### §7 Non-keyed 16-zero-byte key

**Spec:** §7 — non-keyed → 16 zero bytes; keyed → MD5 (XCDR2-BE), at runtime.

**Repo:** `endpoints/kotlin/src/Zerodds.kt` (the writer in big mode produces the BE serialization).

**Tests:** —

**Status:** done

### §7.a Per-struct generated `keyHash` from `@key`

**Spec:** §7 — codegen of a `keyHash` method from `@key`.

**Repo:** —

**Tests:** —

**Status:** n/a (rejected)

**Decision record:** key-hash codegen belongs to the full DCPS TypeSupport, not the XCDR2 wire binding v1.0; the DCPS runtime computes key hashes. Deferred uniformly across all 17 backends.

## §8 Wire core

### §8 `endpoints/kotlin` as the reference writer/reader

**Spec:** §8 — reference wire core, byte-identical to `zerodds-cdr`.

**Repo:** `endpoints/kotlin/src/Zerodds.kt`.

**Tests:** Test.kt, CI job `endpoints-kotlin`.

**Status:** done

## §9 Conformance

### §9 Golden byte identity @final LE+BE

**Spec:** §9 — encoding == golden_le.bin / golden_be.bin byte for byte.

**Repo:** `crates/idl-java`, `endpoints/kotlin`.

**Tests:** `crates/idl-java/tests/golden.rs::byte_identity_vs_rust_goldens` (CI `idl-java`); `endpoints/kotlin` Test.kt (CI `endpoints-kotlin`).

**Status:** done

## §10 Examples

### §10 sync + async deep examples + quickstart

**Spec:** §10 — runnable sync/async examples.

**Repo:** `endpoints/kotlin/example_sync/Example.kt`, `endpoints/kotlin/example_async/Example.kt`, `endpoints/kotlin/QUICKSTART.md`.

**Tests:** CI job `endpoints-kotlin` runs both (kotlinc + java -jar sync/async).

**Status:** done

## §11 Errata + open questions

### §11 Honest non-goals

**Spec:** §11 — decode codegen, keyHash codegen, @mutable, wchar/wstring/map/array/nested/union/long double.

**Repo:** see §3.a/§5.a/§6.a/§7.a (each `n/a (rejected)` with a decision record).

**Tests:** —

**Status:** n/a (informative)
