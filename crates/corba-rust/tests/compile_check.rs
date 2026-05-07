// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Compile-Check: emittiert CORBA-Rust-Code aus IDL und ruft `cargo check`
//! gegen ein temp-Crate mit `zerodds-corba-rust`/`zerodds-cdr`/`zerodds-dcps`-Pfad-Deps
//! auf. Belegt §8.2: der Codegen-Output ist nicht nur snapshotbar sondern
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

use zerodds_corba_rust::{CorbaRustGenOptions, generate_corba_rust_module};
use zerodds_idl::config::ParserConfig;
use zerodds_idl::features::IdlFeatures;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

fn compile_generated(name: &str, idl: &str) {
    let cfg = ParserConfig {
        features: IdlFeatures::corba_full(),
        ..ParserConfig::default()
    };
    let ast = zerodds_idl::parse(idl, &cfg).expect("parse");
    // DataType-Module (struct/enum/exception) kommen aus idl-rust;
    // Service-Module (interface/valuetype/component) aus corba-rust.
    // Echter Workflow ruft beide Codegens — der compile_check muss
    // dieses Setup spiegeln.
    let data_src = generate_rust_module(&ast, &RustGenOptions::default()).expect("idl-rust gen");
    let corba_src =
        generate_corba_rust_module(&ast, &CorbaRustGenOptions::default()).expect("corba-rust gen");
    // Strip inner-Attribute (`#![...]`) aus dem zweiten Output, damit
    // beide concat-bar sind. Inner-Attrs sind nur am Crate-Anfang erlaubt.
    let corba_clean: String = corba_src
        .lines()
        .filter(|l| !l.trim_start().starts_with("#!["))
        .collect::<Vec<_>>()
        .join("\n");
    let rust_src = format!("{data_src}\n{corba_clean}");

    let tmp = std::env::temp_dir().join(format!("dds_corba_rust_compile_{name}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).expect("mkdir");

    let workspace_root = workspace_root();
    let cargo_toml = format!(
        r#"[package]
name = "compile_test_corba_{name}"
version = "0.0.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
zerodds-corba-rust = {{ path = "{ws}/crates/corba-rust" }}
zerodds-cdr = {{ path = "{ws}/crates/cdr" }}
zerodds-dcps = {{ path = "{ws}/crates/dcps" }}
zerodds-sql-filter = {{ path = "{ws}/crates/sql-filter" }}
"#,
        ws = workspace_root.display()
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
        Ok(s) if s.success() => {}
        Ok(s) => panic!(
            "generated CORBA-Rust did not compile (exit {:?}). source:\n{}",
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
fn compile_check_simple_interface() {
    compile_generated(
        "simple_interface",
        r#"interface Calculator {
            long add(in long a, in long b);
        };"#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_valuetype_state() {
    compile_generated(
        "valuetype_state",
        r#"valuetype Point {
            public long x;
            public long y;
        };"#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_interface_inheritance() {
    compile_generated(
        "iface_inheritance",
        r#"interface Base {
            void ping();
        };
        interface Derived : Base {
            long get_id();
        };"#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_interface_raises() {
    compile_generated(
        "iface_raises",
        r#"exception NotFound { string what; };
        interface Vault {
            string lookup(in string key) raises (NotFound);
        };"#,
    );
}
