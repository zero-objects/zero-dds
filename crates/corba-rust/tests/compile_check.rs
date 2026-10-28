// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The generated code is compiled, not only snapshotted.
//!
//! A snapshot says the emitter produced the text it produced last
//! time. It does not say the text is Rust, that every name in it
//! resolves, or that the two columns a user concatenates agree about
//! what a type is called. This runs `cargo check` over exactly that
//! concatenation.
//!
//! ## Both columns
//!
//! `zerodds-lang-rules` emits the **data** types — `struct`, `union`,
//! `enum`, `exception` — with the inherent `encode` and `decode` the
//! stubs call. `zerodds-corba-rust` emits the **service** constructs.
//! The previous version of this test used `zerodds-idl-rust` for the
//! first, which is not in this tree.
//!
//! ## Why it is `#[ignore]`
//!
//! It shells out to `cargo check` against path dependencies, which
//! needs a populated registry and a writable target directory. Run it
//! with `--include-ignored`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use zerodds_ast::Ast;
use zerodds_ast_layout::key::{Dialect, Encoding, LayoutKey, Purpose};
use zerodds_corba_rust::{CorbaRustGenOptions, generate_corba_rust_module};
use zerodds_lang_rules::adapter::cst::rust_items;
use zerodds_lang_rules::encode::rust_codec;
use zerodds_lang_rules::rule::rust;
use zerodds_lang_rules::{ModelGraph, apply};

/// Parse, lower, resolve.
fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "compile_check.idl");
    assert!(
        lowered.diagnostics.iter().all(|d| !d.is_error()),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.broken.is_empty(),
        "every name must resolve: {:?}",
        report.broken
    );
    ast
}

/// The data column: the types and their codecs.
///
/// **Two rules are taken out**, and that is the boundary between the
/// two columns rather than a convenience here.
/// `zerodds_lang_rules::rule::rust::catalogue()` carries
/// `interface-to-trait` and `valuetype-to-struct`, which produce a
/// `pub trait Calculator { fn add(&self, a: i32, b: i32) -> i32; }`
/// and a struct for a valuetype. Both are the answers the **DDS** Rust
/// PSM gives. The CORBA PSM's answers are different — an operation
/// that crosses a wire returns a `Result`, and a valuetype maps to a
/// trait plus a state carrier — and this crate emits them. Applying
/// both catalogues unfiltered produces two declarations of one name,
/// which is what this test found the first time it ran.
///
/// `rust::corba_data_catalogue()` is that subset, the one call every
/// consumer of both columns makes.
fn data_column(ast: &Ast) -> String {
    let graph = ModelGraph::of(ast);
    let applied = apply(&graph, &rust::corba_data_catalogue()).expect("apply the Rust rules");
    let mut items = rust_items(ast, &applied).expect("materialise");
    // The declarations come out of the rules; the `encode` and `decode`
    // beside them come out of the layout plan, one call per named type.
    // The stubs this crate emits call those two by name, so a data
    // column without them compiles to a set of types with no wire form
    // — which is what the first run of this test found.
    //
    // The key is omniORB's classic CDR, big-endian: that is the
    // encoding a GIOP request body carries (CORBA 3.3 part 2 §15.3)
    // and the one the generated stubs write.
    let key = LayoutKey::in_dialect(
        Dialect::OmniOrb,
        Encoding::PlainCdr1,
        zerodds_ast_layout::key::Endianness::Big,
        Purpose::Data,
    );
    for (id, node) in ast.types() {
        if node.name.is_empty() {
            continue;
        }
        if !matches!(
            node.kind,
            zerodds_ast::TypeKind::Structure { .. }
                | zerodds_ast::TypeKind::Union { .. }
                | zerodds_ast::TypeKind::Enumeration { .. }
                | zerodds_ast::TypeKind::Bitmask { .. }
                | zerodds_ast::TypeKind::Bitset { .. }
        ) {
            continue;
        }
        items.extend(rust_codec(ast, id, key).expect("a codec for every declared data type"));
    }
    for (id, node) in ast.exceptions() {
        let _ = (id, node);
    }
    let file = zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::none(),
        uses: Vec::new(),
        items,
    };
    zerodds_lang_render::rust::file(&file)
}

fn compile_generated(name: &str, idl: &str) {
    let ast = model(idl);
    let data = data_column(&ast);
    let service =
        generate_corba_rust_module(&ast, &CorbaRustGenOptions::default()).expect("corba-rust gen");
    // Inner attributes are only legal at the start of a crate, and the
    // service column writes some; the data column goes first.
    let service: String = service
        .lines()
        .filter(|line| !line.trim_start().starts_with("#!["))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!(
        "#![allow(dead_code, unused_imports, unused_variables, clippy::all)]\n\
         use zerodds_wire::{{Reader, Writer, WireError}};\n\
         {data}\n{service}"
    );

    let tmp = std::env::temp_dir().join(format!("zerodds_corba_rust_compile_{name}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).expect("mkdir");

    let root = workspace_root();
    let manifest = format!(
        r#"[package]
name = "compile_test_corba_{name}"
version = "0.0.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
zerodds-corba-rust = {{ path = "{root}/crates/corba-rust" }}
zerodds-corba-rt = {{ path = "{root}/crates/corba-rt" }}
zerodds-wire = {{ path = "{root}/crates/wire" }}
zerodds-float128 = {{ path = "{root}/crates/float128" }}

[workspace]
"#,
        root = root.display()
    );
    std::fs::File::create(tmp.join("Cargo.toml"))
        .expect("create Cargo.toml")
        .write_all(manifest.as_bytes())
        .expect("write Cargo.toml");
    std::fs::File::create(tmp.join("src/lib.rs"))
        .expect("create lib.rs")
        .write_all(source.as_bytes())
        .expect("write lib.rs");

    let output = Command::new(env!("CARGO"))
        .arg("check")
        .arg("--manifest-path")
        .arg(tmp.join("Cargo.toml"))
        .arg("--offline")
        .output()
        .expect("cargo invocation");
    assert!(
        output.status.success(),
        "the generated code does not compile (exit {:?})\n--- cargo ---\n{}\n--- source ---\n{source}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|path| path.join("Cargo.lock").exists())
        .map_or(manifest.clone(), std::path::Path::to_path_buf)
}

#[test]
#[ignore = "shells out to cargo check; run with --include-ignored"]
fn compile_check_simple_interface() {
    compile_generated(
        "simple_interface",
        r"interface Calculator {
            long add(in long a, in long b);
        };",
    );
}

#[test]
#[ignore = "shells out to cargo check; run with --include-ignored"]
fn compile_check_valuetype_state() {
    compile_generated(
        "valuetype_state",
        r"valuetype Point {
            public long x;
            public long y;
        };",
    );
}

#[test]
#[ignore = "shells out to cargo check; run with --include-ignored"]
fn compile_check_ami() {
    // CORBA Messaging §22: `@ami` → an AmiHandler trait, `sendc_`,
    // `sendp_` and a poller. Covers return+in, void+no-out, and
    // return+in+out.
    compile_generated(
        "ami",
        r"interface Bank {
            @ami long deposit(in long amount);
            @ami void close();
            @ami long transfer(in long amount, out long balance);
        };",
    );
}

#[test]
#[ignore = "shells out to cargo check; run with --include-ignored"]
fn compile_check_valuetype_inheritance() {
    // §15.3.4 state flattening: `ExtendedValue` carries `version` from
    // the base and `name` of its own, and marshals them in that order.
    compile_generated(
        "valuetype_inheritance",
        r"valuetype Base {
            public long version;
        };
        valuetype Extended : Base {
            public string name;
        };",
    );
}

#[test]
#[ignore = "shells out to cargo check; run with --include-ignored"]
fn compile_check_interface_inheritance() {
    compile_generated(
        "iface_inheritance",
        r"interface Base {
            void ping();
        };
        interface Derived : Base {
            long get_id();
        };",
    );
}

#[test]
#[ignore = "shells out to cargo check; run with --include-ignored"]
fn compile_check_interface_raises() {
    compile_generated(
        "iface_raises",
        r#"exception NotFound { string what; };
        interface Vault {
            string lookup(in string key) raises (NotFound);
        };"#,
    );
}

#[test]
#[ignore = "shells out to cargo check; run with --include-ignored"]
fn compile_check_aggregates_across_the_boundary() {
    // The property this file exists for: a `struct` the data column
    // declares, used as a parameter and a return by the service column.
    // The two have to name it the same way and agree that it has an
    // inherent `encode` and `decode`.
    compile_generated(
        "aggregates",
        r"struct Reading { long id; double value; };
        interface Sensor {
            Reading latest();
            void push(in long n, out Reading r);
            long count(in sequence<Reading> batch);
        };",
    );
}

#[test]
#[ignore = "shells out to cargo check; run with --include-ignored"]
fn compile_check_enum_across_the_boundary() {
    // The case that found the enumeration hole in the data column.
    // `zerodds_lang_rules::encode::rust_codec` walks a type's layout
    // *members*; an enumeration has none, so it produced
    //
    //     impl Health {
    //         pub fn encode(&self, w: &mut Writer) -> Result<(), WireError> { Ok(()) }
    //         pub fn decode(r: &mut Reader) -> Result<Self, WireError> { Ok(Self { }) }
    //     }
    //
    // — a struct literal on an enum, which is not Rust, and an encoder
    // that put zero octets on the wire for a value that has a wire
    // form. Invisible until an operation took an enumeration as a
    // parameter, because an argument has no enclosing aggregate to be a
    // member of. Fixed in `encode.rs`; this is the case that says so.
    compile_generated(
        "enum_boundary",
        r"enum Health { OK, DEGRADED };
        interface Monitor {
            Health status();
            void set(in Health h);
        };",
    );
}

#[test]
#[ignore = "shells out to cargo check; run with --include-ignored"]
fn compile_check_enum_member_of_a_struct() {
    // An enumeration as a *member*: the data column writes the member
    // through its holder, and the field it declares for it is the
    // enumeration, not an integer.
    compile_generated(
        "enum_member",
        r"enum Health { OK, DEGRADED };
        struct Status { Health h; long n; };
        interface Monitor {
            Status current();
        };",
    );
}
