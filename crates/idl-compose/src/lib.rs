// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-idl-compose`. Safety classification: **STANDARD**.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only, no no_std use case
//! (same classification as the other `zerodds-idl-*` codegen crates it
//! sits alongside).
//!
//! Shared IDL composition pipeline for `zerodds-idlc` and every non-CLI
//! build-time consumer (`zerodds-build`, the CMake `zerodds_idlc_generate`
//! wrapper, future language build-graph integrations).
//!
//! `zerodds-idlc generate` does five things to a raw `.idl` file before a
//! `zerodds-idl-*` language emitter ever sees it:
//!
//! 1. **Preprocess** — `#include`/`#define`/`#ifdef`/`#pragma` expansion
//!    (`zerodds_idl::preprocessor`), with `-I`/`-D`-equivalent knobs.
//! 2. **Parse** — OMG IDL 4.2 -> AST (`zerodds_idl::parser`).
//! 3. **Vendor key-pragmas** — `#pragma keylist` / `DCPS_DATA_KEY` / `cats`
//!    (Cyclone/OpenDDS/OpenSplice) become synthetic `@key` annotations
//!    ([`key_pragmas`]).
//! 4. **Default extensibility / nested** — un-annotated aggregates get an
//!    explicit `@final`/`@appendable`/`@mutable`/`@nested` so every backend
//!    agrees ([`default_ext`]).
//! 5. **TypeObject lowering** — XTypes-1.3 minimal `TypeObject`s, XCDR2-LE
//!    serialised, one per named type ([`typeobject`]).
//!
//! Before this crate existed, only `zerodds-idlc` had all five steps; the
//! five in-repo `build.rs` scripts that called `zerodds_idl::parse` +
//! `zerodds_idl_rust::generate_rust_module` directly got none of them, so
//! their generated types were not feature-equivalent to the CLI's (no
//! `#include` tracking, no vendor key-pragma support, no
//! default-extensibility, no TypeObject block). [`compose`] is the single
//! function that runs all five steps; callers hand the resulting AST +
//! `TypeObjectBlob`s to whichever `zerodds-idl-*` emitter they need.

#![forbid(unsafe_code)]

pub mod compose;
pub mod default_ext;
pub mod key_pragmas;
pub mod project;
pub mod resolver;
pub mod typeobject;

pub use compose::{ComposeError, ComposeOptions, ComposeOutput, compose};
pub use default_ext::DefaultExt;
pub use project::{ProjectComposition, compose_project, strip_top_level_modules_rust};
pub use resolver::FsResolver;
pub use typeobject::TypeObjectBlob;
