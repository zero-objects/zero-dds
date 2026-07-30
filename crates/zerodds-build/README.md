# `zerodds-build`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-build/badge.svg)](https://docs.rs/zerodds-build)

A [`prost-build`](https://docs.rs/prost-build)-style `build.rs` helper for
[ZeroDDS](https://zerodds.org) IDL: `Config::new().include_dir(...).compile(&["x.idl"])`.

## Layer position

Layer 3 (schema), build-time-only dependency (put it under
`[build-dependencies]`, not `[dependencies]`). Depends on
`zerodds-idl-compose` + `zerodds-idl-rust`.

## Why this exists

`zerodds-idlc generate --rust` is not just `zerodds_idl::parse` +
`zerodds_idl_rust::generate_rust_module` — it also preprocesses
`#include`/`#define`/`#ifdef`/`#pragma`, applies vendor key-pragmas
(`#pragma keylist` / `DCPS_DATA_KEY` / `cats`), patches default
extensibility/nestedness, and lowers XTypes TypeObjects. Before this crate
existed, five in-repo `build.rs` scripts (`crates/spatial-dds`,
`crates/spatial-ros2`, `crates/corba-interop`, two `zerodds-examples/*`)
called the two low-level functions directly and silently skipped all four
of those steps — no `#include` dependency tracking (editing an included
file did not trigger a rebuild), no vendor key-pragma support, no
default-extensibility, no TypeObject block. `zerodds-build` runs the same
pipeline the CLI runs (`zerodds-idl-compose::compose`), so a `build.rs`
gets CLI-feature-equivalent generated types.

## Usage

```toml
[build-dependencies]
zerodds-build = "1.0.0-rc.6"
```

```rust,no_run
// build.rs
fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .include_dir("idl")
        .compile(&["idl/Robot.idl"])
}
```

```rust,ignore
// src/lib.rs
#[allow(non_snake_case, unused_imports)]
mod robot {
    include!(concat!(env!("OUT_DIR"), "/Robot.rs"));
}
```

The generated `<stem>.rs` carries its own file-level `#![allow(...)]` inner
attributes (it is meant to stand alone). `include!` splices those tokens
into the *including* file, where an inner attribute is only legal at the
very start — so either wrap the `include!` in a module with the same
`#[allow(...)]` applied as an outer attribute (as above), or strip the
inner-attribute lines from the generated file yourself in `build.rs` before
including it verbatim (the pattern the pre-existing in-repo `build.rs`
scripts and `zerodds-examples/idlc-buildrs` use).

Every `.idl` path passed to `compile()`, plus every file it transitively
`#include`s, is reported via `cargo:rerun-if-changed` — editing an included
`.idl` triggers a rebuild, not just editing the top-level file.

## `Config`

Mirrors `zerodds-idlc`'s own flags:

| `Config` method | CLI equivalent |
|---|---|
| `include_dir(dir)` | `-I <dir>` |
| `define(name, value)` | `-D NAME[=VALUE]` |
| `out_dir(dir)` | `-o <dir>` (default: `$OUT_DIR`) |
| `default_extensibility(ext)` | `--default-extensibility <final\|appendable\|mutable>` |
| `default_nested(bool)` | `--default-nested <true\|false>` |
| `typeobject(bool)` | absence/presence of `--no-typeobject` (default: on) |
| `cdr_only(bool)` | `zerodds-idl-rust`'s CORBA/GIOP-only mode |
| `header_comment(text)` | header comment in the generated file |

## Not yet covered

Only the Rust backend is wired (this crate wraps `zerodds-idl-rust`). The
composition pipeline itself (`zerodds-idl-compose`) is backend-agnostic —
a `zerodds-build`-style crate for another target language would depend on
the same `compose()` and the matching `zerodds-idl-<lang>` emitter.
