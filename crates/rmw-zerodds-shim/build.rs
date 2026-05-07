//! build.rs — Generiert include/rmw_zerodds.h via cbindgen.
//!
//! Build-Scripts sind Tooling-Code (laufen nur zur Compile-Zeit, nicht im
//! Runtime-Pfad) — `expect`/`panic` sind hier akzeptabel, weil ein Fehler
//! in build.rs ohnehin den Build abbricht.

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out = PathBuf::from(&crate_dir)
        .join("include")
        .join("rmw_zerodds.h");
    let cfg_path = PathBuf::from(&crate_dir).join("cbindgen.toml");
    let cfg = cbindgen::Config::from_file(&cfg_path)
        .unwrap_or_else(|e| panic!("cbindgen.toml parse: {e}"));
    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(cfg)
        .generate()
        .map(|b| b.write_to_file(&out))
        .unwrap_or_else(|e| panic!("cbindgen generate: {e:?}"));
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
}
