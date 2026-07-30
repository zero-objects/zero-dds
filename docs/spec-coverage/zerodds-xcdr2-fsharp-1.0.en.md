# `zerodds-xcdr2-fsharp` 1.0 — Spec-Coverage

**Source:** `docs/specs/zerodds-xcdr2-fsharp-1.0.md` — ZeroDDS F# XCDR2 TypeSupport codegen spec.

Implementation:

- `crates/idl-csharp/` — IDL → F# codegen (`marshalXCDR`, self-contained writer).
- `endpoints/fsharp/` — pure-Kotlin/JVM XCDR2 wire core (writer/reader) + sync/async SDK.

## §1 Motivation

### §1 No OMG IDL-to-D wire mapping

**Spec:** §1 — ZeroDDS defines the D XCDR2 wire mapping.

**Repo:** Motivation text of the vendor spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal pattern

### §2 struct + `marshalXCDR(endian, allocator) ![]u8`

**Spec:** §2 — one D `struct` + `marshalXCDR` method per IDL `@final struct`.

**Repo:** `crates/idl-csharp/src/emitter.rs::emit_struct`.

**Tests:** `crates/idl-csharp/tests/golden.rs::final_struct_emits_type_and_marshal`

**Status:** done

## §3 Required API surface

### §3 Writer/reader primitives + generated marshalXCDR

**Spec:** §3 — writer (putU8..putSeqU8, bytes), reader (getU8/getU16/getU32/getU64/getF32/getString/getSeqU8), generated `marshalXCDR`.

**Repo:** `endpoints/fsharp/zerodds.fs` (writer, reader incl. getU8/getU16/getU64/getF32/getString/getSeqU8 (byte-exact inverse)); `crates/idl-csharp/src/emitter.rs` (marshalXCDR).

**Tests:** `endpoints/fsharp/zerodds.fs` test.fsx (byte identity + sync + async), `endpoints/fsharp/example_sync|async.fsx` (full field decode), `crates/idl-csharp/tests/golden.rs`.

**Status:** done

### §3.a Generated `unmarshalXCDR` (decode codegen)

**Spec:** §3/§11 — decode goes through `Reader`; generated `unmarshalXCDR` is missing.

**Repo:** — (decode via `Reader`; see examples.)

**Tests:** `endpoints/fsharp/example_sync|async.fsx`.

**Status:** n/a (rejected)

**Decision record:** Encode codegen (`marshalXCDR`) + a full `Reader` for decode; generated decode is deliberately not part of this binding spec across all 17 idlc backends (the `Reader` covers every field type byte-identically); it belongs to the optional full TypeSupport codegen (roadmap).

## §4 Codegen requirement (`idl-csharp`)

### §4 struct + marshalXCDR + embedded writer

**Spec:** §4 — per struct: D struct, `marshalXCDR`, self-contained writer.

**Repo:** `crates/idl-csharp/src/emitter.rs`; `tools/idlc` `Backend::D`, `--csharp` (.NET interop).

**Tests:** `crates/idl-csharp/tests/golden.rs`.

**Status:** done

## §5 Wire type mapping

### §5 IDL → F# → XCDR2 (alignment cap 4)

**Spec:** §5 — bool/octet/char/short..long long/float/double/string/sequence<octet> with exact wire layout; signed via BitConverter.

**Repo:** `crates/idl-csharp/src/emitter.rs::map_type/map_primitive/map_integer/map_sequence`; `endpoints/fsharp/zerodds.fs` (Put*/getLE align cap 4).

**Tests:** `crates/idl-csharp/tests/golden.rs::byte_identity_vs_rust_goldens` (@final LE+BE).

**Status:** done

### §5.a wchar / wstring / long double / map / array / nested / union

**Spec:** §5/§11 — further IDL constructs.

**Repo:** `crates/idl-csharp/src/` (union, `map<>`, array, nested-struct, `wchar`/`wstring`/`long double` emit arms).

**Tests:** —

**Status:** done

**Decision record:** The `idl-csharp` codegen emits these constructs (unions, `map<>`, arrays, nested-struct members, `wchar`, `wstring`, `long double`). Only `@mutable` unions remain deferred (see §6.a).

## §6 Extensibility

### §6 @final (compact) + @appendable (DHEADER)

**Spec:** §6 — @final without DHEADER, @appendable with uint32 body length + body.

**Repo:** `crates/idl-csharp/src/emitter.rs::emit_struct` (Final/Appendable).

**Tests:** `crates/idl-csharp/tests/golden.rs::final_struct_...` + `appendable_struct_frames_a_dheader`.

**Status:** done

### §6.a @mutable (EMHEADER)

**Spec:** §6 — @mutable EMHEADER framing.

**Repo:** `crates/idl-csharp/src/` (`@mutable` PL_CDR2 EMHEADER / PL_CDR1 paths).

**Tests:** —

**Status:** done (structs); `@mutable` unions deferred

**Decision record:** `@mutable` structs are emitted — PL_CDR2 EMHEADER framing under XCDR2, PL_CDR1 under XCDR1. Only `@mutable` unions are not yet emitted → `Unsupported`.

## §7 Key extraction

### §7 Non-keyed 16-zero-byte key

**Spec:** §7 — non-keyed → 16 zero bytes; keyed → MD5 (XCDR2-BE), at runtime.

**Repo:** `endpoints/fsharp/zerodds.fs` (the writer in big mode produces the BE serialization).

**Tests:** —

**Status:** done

### §7.a Per-struct generated `keyHash` from `@key`

**Spec:** §7 — codegen of a `keyHash` method from `@key`.

**Repo:** —

**Tests:** —

**Status:** n/a (rejected)

**Decision record:** Key-hash codegen belongs to the full DCPS TypeSupport, not to the XCDR2 wire binding v1.0; the DCPS runtime computes key hashes. Deferred uniformly across all 17 backends.

## §8 Wire core

### §8 `endpoints/fsharp` as reference writer/reader

**Spec:** §8 — reference wire core, byte-identical to `zerodds-cdr`.

**Repo:** `endpoints/fsharp/zerodds.fs`.

**Tests:** test.fsx, CI job `endpoints-fsharp`.

**Status:** done

## §9 Conformance

### §9 Golden byte identity @final LE+BE

**Spec:** §9 — encoding == golden_le.bin / golden_be.bin byte-for-byte.

**Repo:** `crates/idl-csharp`, `endpoints/fsharp`.

**Tests:** `crates/idl-csharp/tests/golden.rs::byte_identity_vs_rust_goldens` (CI `idl-csharp`); `endpoints/fsharp` test.fsx (CI `endpoints-fsharp`).

**Status:** done

## §10 Examples

### §10 sync + async deep examples + quickstart

**Spec:** §10 — runnable sync/async examples.

**Repo:** `endpoints/fsharp/example_sync.fsx`, `endpoints/fsharp/example_async.fsx`, `endpoints/fsharp/QUICKSTART.md`.

**Tests:** CI job `endpoints-fsharp` runs both (dotnet fsi example_sync.fsx + example_async.fsx).

**Status:** done

## §11 Errata + open questions

### §11 Honest non-goals

**Spec:** §11 — decode codegen, keyHash codegen, `@mutable` unions.

**Repo:** see §3.a and §7.a (each `n/a (rejected)` with a decision record); §5.a and §6.a are now `done`.

**Tests:** —

**Status:** n/a (informative)
