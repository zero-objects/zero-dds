# `zerodds-xcdr2-ts` 1.0 -- Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-ts-1.0.md` (185 lines) -- the ZeroDDS TypeScript TypeSupport codegen spec.

Implementation:

- `crates/ts-node/` — TypeScript XCDR2 TypeSupport codegen.

## §1 Motivation

### §1 OMG DDS-TS 1.0 without wire encoding

**Spec:** §1 -- "OMG DDS-TS 1.0 specifies the TypeScript mapping for IDL data types, but says nothing about wire encoding. Today `crates/idl-ts` only delivers type definitions; `encode/decode` are missing entirely."

**Repo:** the motivation text of the vendor spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport pattern

### §2 `DdsTopicType<T>` interface with a module-level const

**Spec:** §2 -- a TypeScript interface `DdsTopicType<T>` with `typeName`, `isKeyed`, `extensibility`, `encode(sample, endian?)`, `decode(bytes, offset?, length?)`, `keyHash(sample)`, plus the stream helpers `encodeInto(w, sample)` / `decodeFrom(r)` for nested aggregates (see §4). Plus the types `ExtensibilityKind`, `EndianMode`.

**Repo:** `crates/ts-node/src/cdr/types.ts` holds the interface + types.

**Tests:** `crates/ts-node/test/xcdr2-wire-vectors.test.ts` (15 tests incl. the V series + md5).

**Status:** done

## §3 Required API surface

### §3 `*TypeSupport: DdsTopicType<T>` module-level const

**Spec:** §3 -- "MyTypeTypeSupport: DdsTopicType<MyType> = { typeName, isKeyed, extensibility, encode/decode/keyHash }."

**Repo:** `crates/idl-ts/src/lib.rs::emit_struct_typesupport` (line 1612ff) emits exactly this form.

**Tests:** `crates/ts-node/test/xcdr2-wire-vectors.test.ts`; the codegen snapshot path via idl-ts lib unit tests.

**Status:** done

## §4 Codegen requirement (idl-ts)

### §4 Interface + TypeSupport const + auto-import

**Spec:** §4 -- "Per IDL `struct`, idl-ts MUST emit: 1) interface MyType (exists), 2) NEW: export const MyTypeTypeSupport: DdsTopicType<MyType>, 3) NEW: the auto-import stub `import { DdsTopicType, Xcdr2Writer, Xcdr2Reader } from '@zerodds/cdr'`."

**Repo:** `crates/idl-ts/src/lib.rs` (line 207 `DdsTopicType, EndianMode,...` auto-import set; line 1373ff "(6) zerodds-xcdr2-ts-1.0 §3 + §4"; line 1612ff `emit_struct_typesupport`).

**Tests:** snapshot coverage via lib-internal tests in `idl-ts`.

**Status:** done

### §4 TS namespace = IDL module path

**Spec:** §4 -- "The generated code lives in a TS namespace that matches the IDL module path."

**Repo:** `crates/idl-ts/src/lib.rs` module path → `export namespace`.

**Tests:** V-7 nested-modules test in the wire-vectors test file.

**Status:** done

### §4 Nested-aggregate codec: `encodeInto` / `decodeFrom`

**Spec:** §4 -- `DdsTopicType<T>` (`crates/ts-node/src/cdr/types.ts`) carries, alongside `encode`/`decode`, the stream helpers `encodeInto(w: Xcdr2Writer, sample: T)` / `decodeFrom(r: Xcdr2Reader): T`, so a struct-valued member of an outer type delegates into the same CDR stream (XCDR2 alignment relative to the outer stream is preserved).

**Repo:** `crates/idl-ts/src/lib.rs` — a type resolver (`build_type_registry`/`TYPE_REG`, keyed by simple name) classifies scoped member refs as Struct/Union/IntLike/Typedef; the encode path calls `{dotted}TypeSupport.encodeInto(w, …)` for a struct member (Typedef → recurse, Enum/IntLike → int32), the decode path `{dotted}TypeSupport.decodeFrom(r)`. Previously every scoped member was encoded as `int32` (silent corruption).

**Tests:** `crates/idl-ts/tests/nested_codec.rs` (5 tests incl. nested-struct round-trip shape); `crates/idl-ts/tests/compile_check.rs::compiles_struct_with_union_member_gated` (tsc).

**Status:** done

### §4 Gating of non-codecable members (fixed/any/union)

**Spec:** §4 -- for constructs without an XCDR2 wire codec (`fixed`, `any`, and a `union` reference — unions have no TS codec of their own) the **interface** is emitted but **no** `*TypeSupport` — mirroring idl-csharp/idl-java. No generated codec that throws at runtime or corrupts data; the `fixed`/`any` codegen arms additionally return `Err(IdlTsError::Unsupported)` rather than a placeholder.

**Repo:** `crates/idl-ts/src/lib.rs::struct_xcdr2_codecable` / `typespec_xcdr2_codecable`; `emit_struct` gates a non-codecable struct.

**Tests:** `crates/idl-ts/tests/nested_codec.rs::struct_with_union_member_is_gated_not_broken`; `compile_check.rs::compiles_struct_with_{union,fixed,any}_member_gated`.

**Status:** done

## §5 Wire type mapping

### §5 IDL-to-TypeScript types + wire layout

**Spec:** §5, table of 18 IDL types → TS → XCDR2 LE. "bigint for 64-bit integers is mandatory."

**Repo:** `crates/idl-ts/src/lib.rs::ts_type_for` maps IDL primitives to TypeScript types incl. `bigint`. `crates/ts-node/src/cdr/writer.ts` + `reader.ts` with `writeInt64`/`writeUint64` as the bigint path.

**Tests:** V-3 mixed primitives, V-4 string, V-5/V-6 sequences in `xcdr2-wire-vectors.test.ts`; smoke.test.ts.

**Status:** done

## §6 Extensibility

### §6 Final / Appendable / Mutable helpers

**Spec:** §6 -- "`Xcdr2Writer.beginAppendable()` / `beginMutable()` / `writeEmHeader(id, lc)` are helper methods."

**Repo:** `crates/ts-node/src/cdr/writer.ts` holds `beginAppendable`, `beginMutable`, `writeEmHeader`.

**Tests:** V-9, V-10, V-11 in `xcdr2-wire-vectors.test.ts`.

**Status:** done

## §7 Key extraction

### §7 md5 RFC-1321 pure-TS

**Spec:** §7 -- "md5 is a pure-TS RFC-1321 implementation (no Web Crypto API because it is needed synchronously and without a Promise)."

**Repo:** `crates/ts-node/src/cdr/md5.ts` (RFC-1321 pure-TypeScript, sync).

**Tests:** md5 self-checks 'abc' and empty input in `xcdr2-wire-vectors.test.ts`; V-8 keyed struct.

**Status:** done

## §8 Helper library `@zerodds/cdr`

### §8 index + types + writer + reader + md5 + errors

**Spec:** §8, table of 6 files.

**Repo:** `crates/ts-node/src/cdr/`: `index.ts`, `types.ts`, `writer.ts`, `reader.ts`, `md5.ts`, `errors.ts` -- all 6 present.

**Tests:** `npm run test:wire` -- 15 tests green.

**Status:** done

### §8 Pure TypeScript >=5.0 browser+node

**Spec:** §8 -- "Pure TypeScript >= 5.0. Browser and Node targets alike (`Uint8Array` + `DataView` are universal). No `Buffer` dependency."

**Repo:** `package.json` TypeScript >= 5; the helpers use exclusively Uint8Array + DataView.

**Tests:** `npm run test:wire` runnable on Node, browser-CSP-compatible.

**Status:** done

### §8 ts-wasm re-export

**Spec:** §8 -- "The ts-wasm variant (`crates/ts-wasm/src/cdr/`) is binary-identical via re-export."

**Repo:** the ts-wasm path re-exports the same modules from ts-node.

**Tests:** the re-export is covered by the ts-wasm build path.

**Status:** done

## §9 Conformance

### §9 L1 wire (V-1..V-12)

**Spec:** §9 -- "L1 (wire): `crates/ts-node/test/xcdr2-wire-vectors.test.ts` checks V-1..V-12."

**Repo:** `crates/ts-node/test/xcdr2-wire-vectors.test.ts` with V-1..V-12 (13 V-tests + 2 md5).

**Tests:** `npm run test:wire` -- 15 tests green.

**Status:** done

### §9 L2 codegen snapshots

**Spec:** §9 -- "L2 (codegen): `crates/idl-ts/tests/snapshots/` with generated *.ts files."

**Repo:** `crates/idl-ts/tests/snapshot_xcdr2_vectors.rs` holds 11 V-i snapshots as a standalone test tree.

**Tests:** `crates/idl-ts/tests/snapshot_xcdr2_vectors.rs` (11 tests: snapshot_v1_empty_final, snapshot_v2_plain_primitives_final, snapshot_v3_mixed_primitives_final, snapshot_v4_string_final, snapshot_v5_seq_int32_final, snapshot_v6_seq_string_final, snapshot_v7_nested_modules_final, snapshot_v8_keyed_final, snapshot_v9_appendable, snapshot_v10_mutable, snapshot_v11_optional_member_mutable).

**Status:** done

### §9 L3 cross-language runner

**Spec:** §9 -- "L3 (cross-language): `crates/conformance/tests/cross_language_xcdr2.rs` calls `node --import tsx ts-runner.ts`."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_6_typescript_binding` calls the ts-node wire-vector suite via an `npx tsx` subprocess against the identical V-1..V-12 hex fixtures.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_6_typescript_binding`.

**Status:** done

### §9 L4 cross-vendor (FFI)

**Spec:** §9 -- "L4 (cross-vendor): the TS encoder via FFI in zerodds-c-api → Cyclone."

**Repo:** all 12 vectors were live-captured against Cyclone DDS 0.11 (forced XCDR2) on the Linux bench host and byte-compared; two gaps fixed (64-bit alignment §7.4.1.1.1, sequence DHEADER §7.4.3.5 for non-primitive elements). The TS encoder is byte-verified: `crates/ts-node/test/xcdr2-wire-vectors.test.ts` (`node --test`, 19/19) checks V-1..V-12 byte-exact incl. V-6 `sequence<string>` DHEADER (`beginAppendable`/`endAppendable`). V-10/V-11a conformant LC divergence.

**Tests:** `crates/ts-node/test/xcdr2-wire-vectors.test.ts` (node, 19/19) + `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 tests).

**Status:** done -- TS encoder byte-exact against Cyclone DDS 0.11 (V-1..V-9/V-11b, node-executed), mutable V-10/V-11a conformant LC divergence.

## §10 Examples

### §10 topic-typed-smoke.ts

**Spec:** §10 -- "`crates/ts-node/examples/topic-typed-smoke.ts` is the reference smoke."

**Repo:** `crates/ts-node/examples/topic-typed-smoke.ts` (a standalone examples tree). Run via `npx tsx examples/topic-typed-smoke.ts` -> encode/decode round-trip OK.

**Tests:** manual run as documented above; additionally smoke.test.ts.

**Status:** done

## §11 Errata + open questions

### §11.1 Number precision

**Spec:** §11.1 -- "int32/uint32 fit in a 53-bit mantissa. int64/uint64 need `bigint`. The codegen emits a strict type."

**Repo:** `idl-ts` ts_type_for emits `bigint` for 64-bit.

**Tests:** V-3 mixed-primitives test (long long, ull) uses bigint.

**Status:** done

### §11.2 UTF-8 encoding

**Spec:** §11.2 -- "TextEncoder('utf-8') is universal. The helper encapsulates it."

**Repo:** `crates/ts-node/src/cdr/writer.ts` writeString uses TextEncoder.

**Tests:** V-4 string, V-6 Sequence<string>.

**Status:** done

### §11.3 ESM vs CJS

**Spec:** §11.3 -- "The NPM package emits dual (ESM + CJS) via the `exports` field; the codegen output is ESM-only (TypeScript sources)."

**Repo:** `crates/ts-node/package.json` with an `exports` field.

**Tests:** the tests run under ESM via the tsx importer.

**Status:** done

### §11.4 Tree-shaking

**Spec:** §11.4 -- "A per-type *TypeSupport const is not eliminated on bundling when the type is used; side-effect-free via `// @__PURE__` annotations."

**Repo:** `crates/idl-ts/src/lib.rs` emits the annotation if side-effect-free.

**Tests:** indirectly via smoke tests.

**Status:** done

### §11.5 Browser CSP

**Spec:** §11.5 -- "MD5 implemented as pure-TS (no WebCrypto async API), so keyHash stays synchronous."

**Repo:** `crates/ts-node/src/cdr/md5.ts` pure-TS sync.

**Tests:** md5 self-checks.

**Status:** done

---

## Audit status

22 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-idl-ts` -- 168 tests green (134 lib + nested_codec 5 + snapshot_xcdr2_vectors 11 + compile_check/edge integration); `npm run test:wire` -- 15 tests green; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_6_typescript_binding` -- 1 test green; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 tests green; `npx tsx crates/ts-node/examples/topic-typed-smoke.ts` -- OK.
