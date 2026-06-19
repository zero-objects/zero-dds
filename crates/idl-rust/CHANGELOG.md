# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- **`@mutable` encoder lacked DHEADER wrapping**
  (`src/struct_emit.rs::emit_encode_body`). The encode side emitted
  only the EMHEADER list, while the decode side already used
  `decode_appendable` (DHEADER-aware). Generated code could encode
  but not decode its own output. Both sides now wrap the body
  symmetrically. Spec anchor: OMG XTypes 1.3 §7.4.3.4.4 + master
  conformance §6 V-10.

### Changed

- **`TYPE_NAME` now uses fully-qualified IDL scoped name**
  with `::` module separator (`Module::Sub::Struct`). Threading via
  `src/emitter.rs::emit_module` (raw IDL module names) into
  `src/struct_emit.rs::emit_dds_type_impl`. Spec: master conformance
  §3 / §5 / V-7. Snapshot: `snapshot_codegen__snapshot_module_nested.snap`.
- **`EXTENSIBILITY` const emitted per generated `impl DdsType`**.
  One of `Extensibility::Final` / `::Appendable` / `::Mutable` per
  XTypes 1.3 §7.4.5. Spec: zerodds-xcdr2-rust §2.3 + §6.

### Notes

- See `docs/spec-coverage/zerodds-xcdr2-rust-1.0.md` for the per-
  section coverage map and §11 errata (binding-impl vs spec naming
  aliases for `IS_KEYED` / `HAS_KEY`, `key_hash` /
  `compute_key_hash`, `ExtensibilityKind` / `Extensibility`).

## [1.0.0-rc.1]

Initial release materialization of the `zerodds-idl-rust` crate. Closes the architecture gap "pure-Rust DDS stack without a Rust codegen". Complements the other language codegens (`idl-cpp` / `idl-csharp` / `idl-java` / `idl-ts`) with the Rust path.

### Spec references

- **OMG IDL4** (formal/2018-07-01) §7.4 — type system, annotations, modules.
- **OMG XTypes 1.3** (formal/2024-04-04) §7.4 — XCDR2 wire format, extensibility (final / appendable / mutable), §7.4.5.1 enum encoding (i32), §7.6.8 KeyHash + KeyHolder member-id sorting.
- **OMG DDS 1.4** (formal/2015-04-10) §2.2.3.4 — DataType topic registration via `DdsType::TYPE_NAME`.

### Public API

- `generate_rust_module(spec, opts)` — AST → Rust string (central entry function).
- `RustGenOptions { header_comment }` — codegen options.
- `error::RustGenError` — error family (`Unsupported`, `InvalidAnnotation`, `UnresolvedType`).
- `error::Result<T>` — result alias.

Module structure:
- `emitter` — top-level AST walker (modules + definitions).
- `struct_emit` — struct + DdsType impl (encode/decode/encode_key_holder_be).
- `enum_emit` — enum + CdrEncode/CdrDecode impl + `from_wire`.
- `union_emit` — union → Rust enum with discriminator.
- `typedef_emit` — typedef → `pub type X = Y;`.
- `type_map` — IDL type → Rust type mapping incl. wire-size bound for KeyHolder computation.
- `annotations` — `@key`/`@id`/`@final`/`@appendable`/`@mutable`/`@extensibility`/`@must_understand`/`@nested`/`@optional`.

### Implementation

The codegen strategy is **string-based** analogous to the other idl-* crates (no quote/syn) — a Rust codegen for Rust code needs no macro hygiene; the output strings are parsed by rustc and type errors caught against the emitted code.

Encode/decode bodies uniformly use the `zerodds_cdr::CdrEncode`/`CdrDecode` trait path instead of method calls — all primitives and composite types have `impl CdrEncode` in `zerodds_cdr::encode` / `zerodds_cdr::composite`. The codegen thus emits the same pattern for `i32` (primitive), `String` (composite), `Vec<T>` (composite) and nested structs (trait auto-resolve).

Extensibility modes are mapped to the `zerodds_cdr::struct_enc` helpers:
- `@final` (default): direct encode in declaration order.
- `@appendable`: `zerodds_cdr::struct_enc::encode_appendable(writer, |w| { ... })` with DHEADER wrap.
- `@mutable`: `zerodds_cdr::struct_enc::MutableStructEncoder` with per-member-ID + LengthCode.

@key fields are written member-id-sorted in `encode_key_holder_be` (spec §7.6.8.3.1.b). `KEY_HOLDER_MAX_SIZE` is computed at compile time from the wire sizes of the @key members — fixed-size members yield a `Some(n)` bound (zero-pad path), variable-size members (String, Vec) yield `None` (MD5 path in `compute_key_hash`).

`From<zerodds_cdr::EncodeError>` and `From<zerodds_cdr::DecodeError>` are implemented in `zerodds_dcps::dds_type`, so the `?` operator in the generated code transparently converts between the two error hierarchies.

### Architecture

- **Layer:** 3 (schema).
- **Dependencies (in):** `zerodds-idl` (AST + parser).
- **Dependents (out):** end-user build scripts (typically in a `build.rs` or as a CLI tool).
- **Feature flags:** none — std-only build-time tool.

### Tests

- **Snapshot tests** in `tests/snapshot_codegen.rs` — 13 tests, each snapshot is committed under `tests/snapshots/`. Covers: simple struct, full primitive set, enum, typedef, module-nested, appendable, mutable with @id, struct with String+sequence, struct with multi-dim array, union, single-key, multi-key with @id sorting, string-key (unbounded).
- **Compile-check tests** in `tests/compile_check.rs` (`#[ignore]`-gated) — 8 tests, each writes the emitted code into a temp crate with path deps on `zerodds-cdr`+`zerodds-dcps` and runs `cargo check`. Proves end-to-end: the codegen output is really compilable against the ZeroDDS stack.

```bash
cargo test -p zerodds-idl-rust --tests
cargo test -p zerodds-idl-rust --test compile_check -- --include-ignored
```

### Stability

All `pub` items are RC1-stable; breaking changes require a major bump. The **emitted Rust code** has its own stability guarantee: for a given IDL source the codegen output stays API-compatible across minor versions (whitespace + comment refactors are allowed, breaking API changes are not).

### Out of scope (phase 2+)

- IDL `bitset` / `bitmask` (spec §7.4.7) — out of scope, since DDS topics do not typically use them.
- IDL `fixed` (spec §7.4.4.5) — out of scope (financial-domain need only via the CORBA path).
- IDL `map<K, V>` (spec §7.4.4.6) — out of scope (no standard Rust mapping without a BTreeMap/HashMap choice).
- IDL `any` (spec §7.4.4.7) — out of scope (type erasure does not fit the Rust generic strategy).
- IDL `valuetype` / `interface` / `component` / `home` — CORBA constructs, outside the DDS DataType path.
- Mutable decode with arbitrary member order (Phase 2 read_mutable_member loop) — currently decodes mutable as appendable in declaration order.
