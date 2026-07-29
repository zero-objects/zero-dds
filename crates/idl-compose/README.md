# `zerodds-idl-compose`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-idl-compose/badge.svg)](https://docs.rs/zerodds-idl-compose)

Shared IDL composition pipeline for [ZeroDDS](https://zerodds.org), extracted
from `zerodds-idlc` so build-time consumers other than the CLI —
[`zerodds-build`](../../crates/zerodds-build), the CMake
`zerodds_idlc_generate()` wrapper, future language build-graph integrations —
get CLI-feature-equivalent output instead of reimplementing a thinner
version of the pipeline ad hoc.

## Layer position

Layer 3 (schema), build-time tool, std-only. Depends only on `zerodds-idl`.

## What it does

`zerodds-idlc generate` runs five steps on a raw `.idl` file before handing
the AST to a `zerodds-idl-*` language emitter:

1. **Preprocess** — `#include`/`#define`/`#ifdef`/`#pragma` expansion
   (`zerodds_idl::preprocessor`), with `-I`/`-D`-equivalent knobs
   (`ComposeOptions::include_dirs` / `defines`).
2. **Parse** — OMG IDL 4.2 -> AST.
3. **Vendor key-pragmas** — `#pragma keylist` / `DCPS_DATA_KEY` / `cats`
   (Cyclone / OpenDDS / OpenSplice) become synthetic `@key` annotations
   (`key_pragmas`).
4. **Default extensibility / nested** — un-annotated `struct`/`union`/`enum`
   get an explicit `@final`/`@appendable`/`@mutable`/`@nested` so every
   backend sees the same default (`default_ext`).
5. **TypeObject lowering** — XTypes-1.3 minimal `TypeObject`s, XCDR2-LE
   serialised, one per named type, topologically ordered (`typeobject`).

Before this crate existed, only the CLI ran all five steps. The five
in-repo `build.rs` scripts that called `zerodds_idl::parse` +
`zerodds_idl_rust::generate_rust_module` directly (`crates/spatial-dds`,
`crates/spatial-ros2`, `crates/corba-interop`, and two
`zerodds-examples/*`) got none of them — no `#include` tracking, no vendor
key-pragma support, no default-extensibility, no TypeObject block — so their
generated types were not feature-equivalent to what the CLI would have
produced for the same IDL.

## Usage

```rust
use std::path::Path;
use zerodds_idl_compose::{compose, ComposeOptions};

let opts = ComposeOptions {
    include_dirs: vec!["idl/".into()],
    ..ComposeOptions::default()
};
let out = compose(Path::new("idl/Robot.idl"), &opts).expect("compose");
// out.ast            -> hand to a zerodds-idl-* emitter (e.g. zerodds_idl_rust)
// out.type_objects    -> zerodds_idl_compose::typeobject::render_rust(&out.type_objects)
// out.dependency_files -> cargo:rerun-if-changed / Makefile deps
```

`zerodds-idlc` itself is refactored on top of this crate — the CLI is now a
thin arg-parser + per-backend emit loop around `compose()`, so CLI and
non-CLI consumers cannot drift.
