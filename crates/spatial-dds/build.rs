// SPDX-License-Identifier: Apache-2.0
//! Generates the SpatialDDS Rust types from the vendored spec IDL using
//! ZeroDDS' own IDL→Rust codegen — the profile is not hand-written, it is the
//! real OpenArCloud SpatialDDS IDL run through ZeroDDS.
// A build script legitimately fails the build by panicking on a bad IDL / env.
#![allow(clippy::expect_used, clippy::unwrap_used)]
use std::path::PathBuf;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

fn main() {
    println!("cargo:rerun-if-changed=idl/spatial_core.idl");
    let idl = std::fs::read_to_string("idl/spatial_core.idl").expect("read spatial_core.idl");
    let ast =
        zerodds_idl::parse(&idl, &ParserConfig::default()).expect("parse SpatialDDS core IDL");
    let opts = RustGenOptions {
        cdr_only: true,
        ..RustGenOptions::default()
    };
    let rust = generate_rust_module(&ast, &opts).expect("generate Rust");
    // The generated module carries crate-level `#![allow(..)]` inner attributes
    // + crate-relative module paths (`spatial::`, `builtin::`); we include it at
    // this crate's root, so strip the inner attributes (they cannot sit after
    // other items) — the `#[allow]` on the include site covers them.
    let rust: String = rust
        .lines()
        .filter(|l| !l.trim_start().starts_with("#!["))
        .collect::<Vec<_>>()
        .join("\n");
    // The codegen emits crate-root-relative paths (`spatial::`, `builtin::`);
    // included into this crate they must be `crate::`-anchored so edition-2024
    // path resolution does not read them as extern crates.
    let rust = rust
        .replace("spatial::", "crate::spatial::")
        .replace("builtin::", "crate::builtin::");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("spatial_core.rs");
    std::fs::write(&out, rust).expect("write generated module");
}
