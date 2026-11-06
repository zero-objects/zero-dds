//! Build script: calls cbindgen, generates `include/zerodds.h`.
//!
//! Build scripts are tooling code (they run only at compile time, not in the
//! runtime path) — `unwrap`/`panic` are acceptable here, because an error
//! in build.rs aborts the build anyway.
//!
//! The header is written into the source directory only when its content
//! changes. docs.rs (`DOCS_RS` set) builds in a read-only source directory
//! and needs no header, so the script does nothing there; the published
//! crate ships `include/zerodds.h`.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=DOCS_RS");
    if env::var_os("DOCS_RS").is_some() {
        return;
    }

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let header_path = PathBuf::from(&crate_dir).join("include").join("zerodds.h");

    let cfg = cbindgen::Config::from_file(format!("{crate_dir}/cbindgen.toml"))
        .unwrap_or_else(|e| panic!("cbindgen.toml: {e:?}"));

    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(cfg)
        .generate()
    {
        Ok(b) => {
            let mut raw = Vec::new();
            b.write(&mut raw);
            let header = patch_option_fn_typedefs(String::from_utf8(raw).unwrap());
            write_if_changed(&header_path, &header);
            // Exported declarations span many FFI modules. Watching only
            // lib.rs (or a hand-picked subset) leaves a stale public header
            // when another module changes an ABI contract.
            println!("cargo:rerun-if-changed=src");
            println!("cargo:rerun-if-changed=cbindgen.toml");
        }
        Err(e) => {
            // On error only warn, do not break the build — cbindgen
            // is only for header gen, not for the actual library build.
            println!("cargo:warning=cbindgen failed: {e:?}");
        }
    }
}

/// Writes the header only when it differs from the file on disk, so an
/// unchanged header keeps its timestamp and a read-only source directory
/// with a current header builds. A directory that cannot be written gets a
/// warning naming the file; the library itself does not need the header.
fn write_if_changed(path: &Path, content: &str) {
    if std::fs::read_to_string(path).is_ok_and(|old| old == content) {
        return;
    }
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(path, content));
    if let Err(e) = result {
        println!(
            "cargo:warning=could not write {}: {e}; the header there is not current",
            path.display()
        );
    }
}

/// Cbindgen 0.29 renders `Option<unsafe extern "C" fn(...)>` as an opaque
/// struct typedef without a body, which makes C++ compilers reject the fields
/// as an `incomplete type`. Workaround: replace the forward decls
/// with real function-pointer typedefs (Rust has the null-pointer
/// optimization anyway, ABI-identical).
fn patch_option_fn_typedefs(content: String) -> String {
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
            "typedef struct zerodds_Option_ZeroDdsDecodeReprFn zerodds_Option_ZeroDdsDecodeReprFn;",
            "typedef int (*zerodds_Option_ZeroDdsDecodeReprFn)(const uint8_t *buf, size_t len, uint8_t representation, void *out_sample);",
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
            "typedef void (*zerodds_Option_ZeroDdsDataCallback)(void *user_data, const uint8_t *payload, size_t payload_len, uint8_t representation, uint8_t big_endian);",
        ),
        // The callbacks of the four `zerodds_runtime_for_each_*`: without
        // these, C saw an incomplete struct passed by value and could not
        // call them.
        (
            "typedef struct zerodds_Option_ZeroDdsTopicCallback zerodds_Option_ZeroDdsTopicCallback;",
            "typedef void (*zerodds_Option_ZeroDdsTopicCallback)(void *user_data, const char *topic_name, const char *type_name);",
        ),
        (
            "typedef struct zerodds_Option_ZeroDdsEndpointCallback zerodds_Option_ZeroDdsEndpointCallback;",
            "struct zerodds_ZeroDdsEndpointInfo;\ntypedef void (*zerodds_Option_ZeroDdsEndpointCallback)(void *user_data, const struct zerodds_ZeroDdsEndpointInfo *info);",
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
        "ZeroDdsDecodeReprFn",
        "ZeroDdsKeyHashFn",
        "ZeroDdsSampleFreeFn",
        "ZeroDdsDataCallback",
        "ZeroDdsTopicCallback",
        "ZeroDdsEndpointCallback",
    ] {
        let from = format!("struct zerodds_Option_{fname}");
        let to = format!("zerodds_Option_{fname}");
        out = out.replace(&from, &to);
    }
    out
}
