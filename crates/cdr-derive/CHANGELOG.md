# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization.

### Spec references
- `docs/specs/zerodds-xcdr2-rust-1.0.md` §11.1 — `#[derive(DdsType)]`
  classified as a follow-up sprint; fully implemented in this crate.
- OMG XTypes 1.3 §7.4 — wire format of the emitted encode/decode
  methods.
- OMG XTypes 1.3 §7.6.8.4 — key-hash computation (≤ 16 bytes
  zero-pad, otherwise MD5).

### Public API
- [`DdsType`](src/lib.rs) — proc-macro derive attribute. Annotates
  a plain `struct` and emits an `impl DdsType` block.
- Inner attributes:
  - `#[dds(type_name = "...")]` — explicit TYPE_NAME override.
  - `#[dds(key)]` — per member, marks the field as `@key`.

### Implementation
- AST walk via `syn 2`. `DeriveInput` -> `Data::Struct` ->
  `Fields::Named` iteration.
- Per-field codegen via `quote::quote!` templates. Encode/decode
  delegates to the `zerodds_cdr::CdrEncode`/`CdrDecode` traits — no
  type-specific match table; every primitive + composite type in
  `zerodds_cdr` has an `impl CdrEncode/Decode`.
- The key-hash path emits an `encode_key_holder_be` override method
  when at least one `#[dds(key)]` member is present.

Currently focused on final extensibility (no DHEADER, no EMHEADER).
Appendable + mutable are left to the `idl-rust` codegen because their
logic is finer-grained at member granularity than the macro can
practically emit.

### Architecture
- Layer: 1 Primitives (helper crate for `zerodds-cdr` and
  `zerodds-dcps`).
- Dependencies (in): `syn 2`, `quote 1`, `proc-macro2 1`.
- Dependents (out): user code that uses `#[derive(DdsType)]`.
  Consumes the trait implementations from `zerodds-cdr` and
  `zerodds-dcps` at compile time (transitive build dependency).
- Feature flags: none.

### Stability
- All `pub` items are RC1-stable.
- The macro output form (exact token layout) is NOT stable and may
  change between minor versions; semantically the `impl DdsType` spec
  form stays the same.

[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
