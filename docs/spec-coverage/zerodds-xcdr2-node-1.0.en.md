# `zerodds-xcdr2-node` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-node-1.0.md` — the ZeroDDS Node XCDR2 TypeSupport codegen spec.

Implementation:

- `crates/idl-ts/` — IDL → Node codegen (`marshalXCDR`, self-contained writer).
- `endpoints/node/` — pure-Kotlin/JVM XCDR2 wire core (writer/reader) + sync/async SDK.

## §1 Motivation

### §1 No OMG-IDL-to-D wire mapping

**Spec:** §1 — ZeroDDS defines the D XCDR2 wire mapping.

**Repo:** the motivation text of the vendor spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal pattern

### §2 struct + `marshalXCDR(endian, allocator) ![]u8`

**Spec:** §2 — per IDL `@final struct` a D `struct` + `marshalXCDR` method.

**Repo:** `crates/idl-ts/src/emitter.rs::emit_struct`.

**Tests:** `crates/idl-ts/tests/golden.rs::final_struct_emits_type_and_marshal`

**Status:** done

## §3 Required API surface

### §3 Writer/reader primitives + generated marshalXCDR

**Spec:** §3 — writer (putU8..putSeqU8, bytes), reader (getU8/getU16/getU32/getU64/getF32/getString/getSeqU8), generated `marshalXCDR`.

**Repo:** `endpoints/node/zerodds.js` (writer, reader incl. getU8/getU16/getU64/getF32/getString/getSeqU8 (a byte-exact inverse)); `crates/idl-ts/src/emitter.rs` (marshalXCDR).

**Tests:** `endpoints/node/zerodds.js` test.js (byte identity + sync + async), `endpoints/node/example_sync|async.js` (full field decode), `crates/idl-ts/tests/golden.rs`.

**Status:** done

### §3.a Generated `unmarshalXCDR` (decode codegen)

**Spec:** §3/§11 — decode runs via `Reader`; a generated `unmarshalXCDR` is missing.

**Repo:** — (decode via `Reader`; see examples.)

**Tests:** `endpoints/node/example_sync|async.js`.

**Status:** n/a (rejected)

**Decision record:** encode codegen (`marshalXCDR`) + a full `Reader` for decode; generated decode is deliberately not part of this binding spec across all 17 idlc backends (the `Reader` covers every field type byte-identically), it belongs to the optional full TypeSupport codegen (roadmap).

## §4 Codegen requirement (`idl-ts`)

### §4 struct + marshalXCDR + embedded writer

**Spec:** §4 — per struct: D struct, `marshalXCDR`, self-contained writer.

**Repo:** `crates/idl-ts/src/emitter.rs`; `tools/idlc` `Backend::D`, `--ts` (TypeScript).

**Tests:** `crates/idl-ts/tests/golden.rs`.

**Status:** done

## §5 Wire type mapping

### §5 IDL → Node → XCDR2 (alignment cap 4)

**Spec:** §5 — bool/octet/char/short..long long/float/double/string/sequence<octet> with exact wire layout; signed via Buffer.writeFloatLE.

**Repo:** `crates/idl-ts/src/emitter.rs::map_type/map_primitive/map_integer/map_sequence`; `endpoints/node/zerodds.js` (Put*/getLE align cap 4).

**Tests:** `crates/idl-ts/tests/golden.rs::byte_identity_vs_rust_goldens` (@final LE+BE).

**Status:** done

### §5.a wchar / wstring / long double / map / array / nested / union

**Spec:** §5/§11 — further IDL constructs.

**Repo:** `crates/idl-ts/src/` (union, `map<>`, array, nested-struct, `wchar`/`wstring`/`long double` emit arms).

**Tests:** —

**Status:** done

**Decision record:** The `idl-ts` codegen emits these constructs (unions, `map<>`, arrays, nested-struct members, `wchar`, `wstring`, `long double`). Only `@mutable` unions remain deferred (see §6.a).

## §6 Extensibility

### §6 @final (compact) + @appendable (DHEADER)

**Spec:** §6 — @final without DHEADER, @appendable with a uint32 body length + body.

**Repo:** `crates/idl-ts/src/emitter.rs::emit_struct` (final/appendable).

**Tests:** `crates/idl-ts/tests/golden.rs::final_struct_...` + `appendable_struct_frames_a_dheader`.

**Status:** done

### §6.a @mutable (EMHEADER)

**Spec:** §6 — @mutable EMHEADER framing.

**Repo:** `crates/idl-ts/src/` (`@mutable` PL_CDR2 EMHEADER / PL_CDR1 paths).

**Tests:** —

**Status:** done (structs); `@mutable` unions deferred

**Decision record:** `@mutable` structs are emitted — PL_CDR2 EMHEADER framing under XCDR2, PL_CDR1 under XCDR1. Only `@mutable` unions are not yet emitted → `Unsupported`.

## §7 Key extraction

### §7 Non-keyed 16-zero-byte key

**Spec:** §7 — non-keyed → 16 zero bytes; keyed → MD5 (XCDR2-BE), at runtime.

**Repo:** `endpoints/node/zerodds.js` (the writer in big mode produces the BE serialization).

**Tests:** —

**Status:** done

### §7.a Per-struct generated `keyHash` from `@key`

**Spec:** §7 — codegen of a `keyHash` method from `@key`.

**Repo:** —

**Tests:** —

**Status:** n/a (rejected)

**Decision record:** key-hash codegen belongs to the full DCPS TypeSupport, not the XCDR2 wire binding v1.0; the DCPS runtime computes key hashes. Deferred uniformly across all 17 backends.

## §8 Wire core

### §8 `endpoints/node` as the reference writer/reader

**Spec:** §8 — reference wire core, byte-identical to `zerodds-cdr`.

**Repo:** `endpoints/node/zerodds.js`.

**Tests:** test.js, CI job `endpoints-node`.

**Status:** done

## §9 Conformance

### §9 Golden byte identity @final LE+BE

**Spec:** §9 — encoding == golden_le.bin / golden_be.bin byte for byte.

**Repo:** `crates/idl-ts`, `endpoints/node`.

**Tests:** `crates/idl-ts/tests/golden.rs::byte_identity_vs_rust_goldens` (CI `idl-ts`); `endpoints/node` test.js (CI `endpoints-node`).

**Status:** done

## §10 Examples

### §10 sync + async deep examples + quickstart

**Spec:** §10 — runnable sync/async examples.

**Repo:** `endpoints/node/example_sync.js`, `endpoints/node/example_async.js`, `endpoints/node/QUICKSTART.md`.

**Tests:** CI job `endpoints-node` runs both (node example_sync.js + example_async.js).

**Status:** done

## §11 Errata + open questions

### §11 Honest non-goals

**Spec:** §11 — decode codegen, keyHash codegen, `@mutable` unions.

**Repo:** see §3.a and §7.a (each `n/a (rejected)` with a decision record); §5.a and §6.a are now `done`.

**Tests:** —

**Status:** n/a (informative)
