//! Build-Script: ruft cbindgen, generiert `include/zerodds.h`.
//!
//! Build-Scripts sind Tooling-Code (laufen nur zur Compile-Zeit, nicht im
//! Runtime-Pfad) — `unwrap`/`panic` sind hier akzeptabel, weil ein Fehler
//! in build.rs ohnehin den Build abbricht.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(&crate_dir).join("include");
    std::fs::create_dir_all(&out_dir).ok();

    let header_path = out_dir.join("zerodds.h");

    let cfg = cbindgen::Config::from_file(format!("{crate_dir}/cbindgen.toml"))
        .unwrap_or_else(|e| panic!("cbindgen.toml: {e:?}"));

    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(cfg)
        .generate()
    {
        Ok(b) => {
            b.write_to_file(&header_path);
            patch_option_fn_typedefs(&header_path);
            println!("cargo:rerun-if-changed=src/lib.rs");
            println!("cargo:rerun-if-changed=cbindgen.toml");
        }
        Err(e) => {
            // Bei Fehler nur warnen, nicht den Build brechen — cbindgen
            // ist nur fuer Header-Gen, nicht fuer den eigentlichen Bibliotheks-Build.
            println!("cargo:warning=cbindgen failed: {e:?}");
        }
    }
}

/// Cbindgen 0.29 rendert `Option<unsafe extern "C" fn(...)>` als opaque
/// struct-typedef ohne Body, weshalb C++-Compiler die Felder als
/// `incomplete type` ablehnen. Workaround: ersetze die Forward-Decls
/// durch echte function-pointer-typedefs (Rust hat eh die Null-Pointer-
/// Optimierung, ABI-mässig identisch).
fn patch_option_fn_typedefs(header_path: &std::path::Path) {
    let content = std::fs::read_to_string(header_path).unwrap_or_default();
    if content.is_empty() {
        return;
    }
    let replacements: &[(&str, &str)] = &[
        (
            "typedef struct zerodds_Option_ZeroDdsEncodeFn zerodds_Option_ZeroDdsEncodeFn;",
            "typedef int (*zerodds_Option_ZeroDdsEncodeFn)(const void *sample, uint8_t *out_buf, size_t out_cap, size_t *out_len);",
        ),
        (
            "typedef struct zerodds_Option_ZeroDdsDecodeFn zerodds_Option_ZeroDdsDecodeFn;",
            "typedef int (*zerodds_Option_ZeroDdsDecodeFn)(const uint8_t *buf, size_t len, void *out_sample);",
        ),
        (
            "typedef struct zerodds_Option_ZeroDdsKeyHashFn zerodds_Option_ZeroDdsKeyHashFn;",
            "typedef int (*zerodds_Option_ZeroDdsKeyHashFn)(const void *sample, uint8_t *out_hash);",
        ),
        (
            "typedef struct zerodds_Option_ZeroDdsSampleFreeFn zerodds_Option_ZeroDdsSampleFreeFn;",
            "typedef void (*zerodds_Option_ZeroDdsSampleFreeFn)(void *sample);",
        ),
        (
            "typedef struct zerodds_Option_ZeroDdsDataCallback zerodds_Option_ZeroDdsDataCallback;",
            "typedef void (*zerodds_Option_ZeroDdsDataCallback)(void *user_data, const uint8_t *payload, size_t payload_len);",
        ),
    ];
    let mut out = content;
    for (from, to) in replacements {
        out = out.replace(from, to);
    }
    // Strukt-Felder + Funktions-Args: "struct zerodds_Option_X" → "zerodds_Option_X".
    for fname in [
        "ZeroDdsEncodeFn",
        "ZeroDdsDecodeFn",
        "ZeroDdsKeyHashFn",
        "ZeroDdsSampleFreeFn",
        "ZeroDdsDataCallback",
    ] {
        let from = format!("struct zerodds_Option_{fname}");
        let to = format!("zerodds_Option_{fname}");
        out = out.replace(&from, &to);
    }
    let _ = std::fs::write(header_path, out);
}
