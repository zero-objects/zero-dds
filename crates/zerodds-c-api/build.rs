//! Build script: calls cbindgen, generates `include/zerodds.h`.
//!
//! Build scripts are tooling code (they run only at compile time, not in the
//! runtime path) — `unwrap`/`panic` are acceptable here, because an error
//! in build.rs aborts the build anyway.

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
            // On error only warn, do not break the build — cbindgen
            // is only for header gen, not for the actual library build.
            println!("cargo:warning=cbindgen failed: {e:?}");
        }
    }
}

/// Cbindgen 0.29 renders `Option<unsafe extern "C" fn(...)>` as an opaque
/// struct typedef without a body, which makes C++ compilers reject the fields
/// as an `incomplete type`. Workaround: replace the forward decls
/// with real function-pointer typedefs (Rust has the null-pointer
/// optimization anyway, ABI-identical).
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
            "typedef void (*zerodds_Option_ZeroDdsDataCallback)(void *user_data, const uint8_t *payload, size_t payload_len, uint8_t representation);",
        ),
    ];
    let mut out = content;
    for (from, to) in replacements {
        out = out.replace(from, to);
    }
    // Struct fields + function args: "struct zerodds_Option_X" → "zerodds_Option_X".
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
