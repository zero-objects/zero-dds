//! build.rs — generates include/rmw_zerodds.h via cbindgen.
//!
//! Build scripts are tooling code (run only at compile time, not on the
//! runtime path) — `expect`/`panic` are acceptable here, because an error
//! in build.rs aborts the build anyway.
//!
//! The header is written into the source directory only when its content
//! changes. docs.rs (`DOCS_RS` set) builds in a read-only source directory
//! and needs no header, so the script does nothing there; the published
//! crate ships `include/rmw_zerodds.h`.

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=DOCS_RS");
    if env::var_os("DOCS_RS").is_some() {
        return;
    }
    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out = PathBuf::from(&crate_dir)
        .join("include")
        .join("rmw_zerodds.h");
    let cfg_path = PathBuf::from(&crate_dir).join("cbindgen.toml");
    let cfg = cbindgen::Config::from_file(&cfg_path)
        .unwrap_or_else(|e| panic!("cbindgen.toml parse: {e}"));
    let bindings = cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(cfg)
        .generate()
        .unwrap_or_else(|e| panic!("cbindgen generate: {e:?}"));
    let mut header = Vec::new();
    bindings.write(&mut header);
    // Unchanged header: no write, so the file keeps its timestamp and a
    // read-only source directory with a current header builds.
    let current = std::fs::read(&out).is_ok_and(|old| old == header);
    if !current
        && let Err(e) = out
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&out, &header))
    {
        println!(
            "cargo:warning=could not write {}: {e}; the header there is not current",
            out.display()
        );
    }
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
}
