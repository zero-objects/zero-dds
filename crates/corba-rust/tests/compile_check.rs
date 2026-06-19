// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Compile check: emits CORBA Rust code from IDL and runs `cargo check`
//! against a temp crate with `zerodds-corba-rust`/`zerodds-cdr`/`zerodds-dcps` path deps.
//! Demonstrates §8.2: the codegen output is not only snapshottable but
//! also actually compilable.

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
    // DataType modules (struct/enum/exception) come from idl-rust;
    // service modules (interface/valuetype/component) from corba-rust.
    // The real workflow calls both codegens — the compile_check must
    // mirror this setup.
    let data_src = generate_rust_module(&ast, &RustGenOptions::default()).expect("idl-rust gen");
    let corba_src =
        generate_corba_rust_module(&ast, &CorbaRustGenOptions::default()).expect("corba-rust gen");
    // Strip inner attributes (`#![...]`) from the second output so that
    // both are concatenable. Inner attrs are only allowed at the crate start.
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
fn compile_check_ami() {
    // CORBA Messaging §22: `@ami` interface → AmiHandler trait + sendc_/sendp_ +
    // poller. Covers ret+in, void+no-out, and ret+in+out.
    compile_generated(
        "ami",
        r#"interface Bank {
            @ami long deposit(in long amount);
            @ami void close();
            @ami long transfer(in long amount, out long balance);
        };"#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_valuetype_inheritance() {
    // §15.3.4 state flattening: ExtendedValue must carry `version` (base) + `name`
    // (derived) and marshal them in this order.
    compile_generated(
        "valuetype_inheritance",
        r#"valuetype Base {
            public long version;
        };
        valuetype Extended : Base {
            public string name;
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
