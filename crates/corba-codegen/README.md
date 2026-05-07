# `zerodds-corba-codegen`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-codegen/badge.svg)](https://docs.rs/zerodds-corba-codegen)

OMG CORBA 3.3 Annex-A.1 IDL-Mapping-Codegen-Helpers — Tabellen +
Helper, die `zerodds-idl-cpp`/`-csharp`/`-java` zur Erzeugung CORBA-
Stub-/Skeleton-Codes konsumieren. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CORBA 3.3 Part 1 | Annex A (IDL-Type-Mappings), §10.7.3.1 (Repository-ID) |
| OMG IDL-to-C++ | `formal/2008-01-09` |
| OMG IDL-to-Java | `formal/2008-01-04` |

## Was ist drin

- **`SpecialType` / `TargetLanguage` / `language_mapping`** — 13
  Annex-A.1 Special-Types (Object, ValueBase, AbstractBase,
  NativeRef, TypeCode, any, sequence-of-any, string, wstring, Time,
  fixed, ULongLong, LongDouble) mit Sprach-Mappings fuer C++ / C# /
  Java.
- **`StubOp` / `render_stub_op`** — Client-Stub-Template (sendet
  GIOP-Request, erwartet Reply mit ReplyStatus-Switch).
- **`SkeletonOp` / `render_skeleton_dispatch`** — Server-Side-Dispatch-
  Template (Operation-Name-Switch).
- **`build_repository_id`** — Spec §10.7.3.1 Repository-ID-Builder
  (`IDL:<scoped-name>:<major>.<minor>`).

## Schichten-Position

Layer 8 — CORBA-Stack. Substrat fuer die drei OMG-PSM-Codegen-Crates
(`zerodds-idl-cpp` / `zerodds-idl-csharp` / `zerodds-idl-java`).

## Quickstart

```rust
use zerodds_corba_codegen::build_repository_id;

let id = build_repository_id(&["MyModule"], "MyInterface", 1, 0);
assert_eq!(id, "IDL:MyModule/MyInterface:1.0");
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Standard-Library. |
| `alloc` | ✅ (via std) | `String` / `Vec`. |

`no_std`-fahig: `default-features = false, features = ["alloc"]`.

## Stabilitaet

`1.0.0-rc.1`. Public-API + Annex-A.1-Mapping-Tabellen sind RC1-
stabil; Spec-Aenderungen erfordern Major-Bump.

## Tests

```bash
cargo test -p zerodds-corba-codegen
```

16 Unit-Tests grün.

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/corba-codegen.md`](../../docs/release/rc1-reviews/corba-codegen.md) — RC1-Review.
- [`zerodds-idl-cpp`](../idl-cpp), [`zerodds-idl-csharp`](../idl-csharp), [`zerodds-idl-java`](../idl-java) — Konsumenten der Codegen-Helpers.
