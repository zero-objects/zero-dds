# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-flatdata-derive` crate.

### Spec references

- **`docs/specs/zerodds-flatdata-1.0.md`** §1.2 (derive macro): `#[derive(FlatStruct)]` generates `unsafe impl FlatStruct for T` with `TYPE_HASH = sha256(layout_signature(T))[..16]`.

### Public API

- `#[derive(FlatStruct)]` — proc-macro derive on `zerodds_flatdata::FlatStruct`. Generates an `unsafe impl` block with the `TYPE_HASH` constant.

There are no further public items: the crate is a pure `proc-macro = true` lib.

### Implementation

The `expand` function only accepts `struct` DeriveInputs (named, tuple, unit). `enum` and `union` are rejected with `compile_error!`, because their layout — even under `repr(C)` — is not predictable in the same form as struct layouts (discriminant position, untagged-union aliasing).

Before computing the hash, `has_repr_c_or_transparent(&input.attrs)` is checked. If `#[repr(C)]` or `#[repr(transparent)]` is missing, the macro rejects with `compile_error!` — turning the doc promise "FlatStruct requires repr(C)" into a compile-time guarantee instead of a caller-trust document.

The `TYPE_HASH` is derived from `sha256(<TypeName>{<field-name>:<ty>,...})[..16]`. The layout string explicitly contains:
- Type name (type rename → new hash).
- Field order (field reorder → new hash).
- Field names (for named structs) and field type strings (field add/remove + field type change → new hash).

The bounds `Copy + 'static + Send + Sync` are enforced by the trait itself — the macro generates no bound checks of its own. As a result, trait-bound errors appear as understandable `T: Copy`/etc. compiler errors at the use site, not as cryptic macro errors.

Generated code form:
```rust
#[automatically_derived]
unsafe impl ::zerodds_flatdata::FlatStruct for #name {
    const TYPE_HASH: [u8; 16] = [/* 16 byte */];
}
```

`WIRE_SIZE` and the `as_bytes`/`from_bytes_unchecked` methods come as a trait default from `zerodds-flatdata` and need no macro override.

### Architecture

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `syn 2`, `quote 1`, `proc-macro2 1`, `sha2` (workspace, default-features=false). No ZeroDDS crate deps.
- **Dependents (out):** `zerodds-flatdata` (`dev-dependencies`, for the `tests/derive.rs` smoke test); end-user crates that want to derive FlatStruct.
- **Feature flags:** none.

### Stability

All public macro paths are RC1-stable. The layout signature format is wire-stable: a change would invalidate all previously generated `TYPE_HASH` values and is therefore classified as a major breaking change.
