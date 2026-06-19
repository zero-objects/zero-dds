# `zerodds-corba-codegen`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-codegen/badge.svg)](https://docs.rs/zerodds-corba-codegen)

OMG CORBA 3.3 Annex-A.1 IDL mapping codegen helpers — tables +
helpers consumed by `zerodds-idl-cpp`/`-csharp`/`-java` to generate
CORBA stub/skeleton code. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CORBA 3.3 Part 1 | Annex A (IDL type mappings), §10.7.3.1 (repository ID) |
| OMG IDL-to-C++ | `formal/2008-01-09` |
| OMG IDL-to-Java | `formal/2008-01-04` |

## What's inside

- **`SpecialType` / `TargetLanguage` / `language_mapping`** — 13
  Annex-A.1 special types (Object, ValueBase, AbstractBase,
  NativeRef, TypeCode, any, sequence-of-any, string, wstring, Time,
  fixed, ULongLong, LongDouble) with language mappings for C++ / C# /
  Java.
- **`StubOp` / `render_stub_op`** — client-stub template (sends a
  GIOP request, expects a reply with a ReplyStatus switch).
- **`SkeletonOp` / `render_skeleton_dispatch`** — server-side dispatch
  template (operation-name switch).
- **`build_repository_id`** — Spec §10.7.3.1 repository ID builder
  (`IDL:<scoped-name>:<major>.<minor>`).

## Layer position

Layer 8 — CORBA stack. Substrate for the three OMG PSM codegen crates
(`zerodds-idl-cpp` / `zerodds-idl-csharp` / `zerodds-idl-java`).

## Quickstart

```rust
use zerodds_corba_codegen::build_repository_id;

let id = build_repository_id(&["MyModule"], "MyInterface", 1, 0);
assert_eq!(id, "IDL:MyModule/MyInterface:1.0");
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Standard library. |
| `alloc` | ✅ (via std) | `String` / `Vec`. |

`no_std`-capable: `default-features = false, features = ["alloc"]`.

## Stability

`1.0.0-rc.1`. Public API + Annex-A.1 mapping tables are RC1-stable;
spec changes require a major bump.

## Tests

```bash
cargo test -p zerodds-corba-codegen
```

16 unit tests passing.

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/release/rc1-reviews/corba-codegen.md`](../../docs/release/rc1-reviews/corba-codegen.md) — RC1 review.
- [`zerodds-idl-cpp`](../idl-cpp), [`zerodds-idl-csharp`](../idl-csharp), [`zerodds-idl-java`](../idl-java) — consumers of the codegen helpers.
