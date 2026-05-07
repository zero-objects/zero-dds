// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Compile-Check: emittiert Rust-Code aus IDL und ruft `rustc --emit=metadata`
//! gegen den generierten Code mit zerodds_cdr + zerodds_dcps als deps auf.
//!
//! Belegt Phase-H: der Codegen-Output ist nicht nur snapshotbar sondern
//! auch tatsaechlich kompilierbar.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

/// Emittiert Rust-Code, schreibt ihn in eine temp-Datei und ruft
/// `cargo check` gegen ein Test-Crate auf, das den Code als Modul
/// inkludiert. Schlaegt mit Pretty-Diagnostics fehl, wenn der
/// generierte Code nicht kompiliert.
fn compile_generated(name: &str, idl: &str) {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let rust_src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");

    let tmp = std::env::temp_dir().join(format!("dds_idl_rust_compile_{name}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).expect("mkdir");

    // Test-Crate-Cargo.toml mit Pfad-Deps auf zerodds-cdr + zerodds-dcps.
    let workspace_root = workspace_root();
    let cargo_toml = format!(
        r#"[package]
name = "compile_test_{name}"
version = "0.0.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
zerodds-cdr = {{ path = "{}/crates/cdr" }}
zerodds-dcps = {{ path = "{}/crates/dcps" }}
zerodds-sql-filter = {{ path = "{}/crates/sql-filter" }}
zerodds-types = {{ path = "{}/crates/types" }}
"#,
        workspace_root.display(),
        workspace_root.display(),
        workspace_root.display(),
        workspace_root.display()
    );
    std::fs::File::create(tmp.join("Cargo.toml"))
        .expect("create Cargo.toml")
        .write_all(cargo_toml.as_bytes())
        .expect("write Cargo.toml");

    std::fs::File::create(tmp.join("src/lib.rs"))
        .expect("create lib.rs")
        .write_all(rust_src.as_bytes())
        .expect("write lib.rs");

    let status = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(tmp.join("Cargo.toml"))
        .arg("--offline")
        .status();

    match status {
        Ok(s) if s.success() => { /* good */ }
        Ok(s) => panic!(
            "generated rust did not compile (exit {:?}). source:\n{}",
            s.code(),
            rust_src
        ),
        Err(e) => panic!("cargo invocation failed: {e}"),
    }
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|p| p.join("Cargo.lock").exists())
        .map(|p| p.to_path_buf())
        .unwrap_or(manifest)
}

#[test]
#[ignore = "requires cargo offline + path-deps; run with --include-ignored"]
fn compile_check_simple_struct_primitives() {
    compile_generated("simple_primitives", r#"struct Point { long x; long y; };"#);
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_full_primitive_set() {
    compile_generated(
        "full_primitives",
        r#"
        struct AllPrimitives {
            int8 i8;
            uint8 u8;
            short s;
            unsigned short us;
            long l;
            unsigned long ul;
            long long ll;
            unsigned long long ull;
            float f;
            double d;
            boolean b;
            octet o;
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_appendable() {
    compile_generated(
        "appendable",
        r#"
        @appendable
        struct Telemetry {
            unsigned long timestamp;
            double value;
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_with_string_and_sequence() {
    compile_generated(
        "string_seq",
        r#"
        struct Message {
            string topic;
            sequence<long> values;
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_enum() {
    compile_generated(
        "enum_color",
        r#"
        enum Color {
            RED,
            GREEN,
            BLUE
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_typedef() {
    compile_generated("typedef_distance", r#"typedef long Distance;"#);
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_reserved_word_identifiers() {
    compile_generated(
        "reserved",
        r#"
        struct match {
            long type;
            long mod;
            long fn;
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_nested_struct_emits_is_nested_const() {
    compile_generated(
        "nested",
        r#"
        @nested
        struct Inner {
            long x;
            long y;
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_optional_member_field() {
    compile_generated(
        "optional_field",
        r#"
        struct Profile {
            string name;
            @optional long age;
            @optional string email;
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_mutable_with_arbitrary_member_order() {
    compile_generated(
        "mutable_arb",
        r#"
        @mutable
        struct UserPrefs {
            @id(10) string name;
            @id(20) long age;
            @id(30) boolean active;
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_bitset() {
    compile_generated(
        "bitset",
        r#"
        bitset Status {
            bitfield<1> ready;
            bitfield<1> error;
            bitfield<2> level;
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_bitmask() {
    compile_generated(
        "bitmask",
        r#"
        bitmask Permissions {
            READ,
            WRITE,
            EXECUTE
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_keyed_struct() {
    compile_generated(
        "keyed",
        r#"
        struct Reading {
            @key long sensor_id;
            double value;
        };
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_multi_key_id_sorting() {
    compile_generated(
        "multikey",
        r#"
        struct Composite {
            @key @id(20) long b;
            @key @id(10) long a;
            string payload;
        };
        "#,
    );
}
