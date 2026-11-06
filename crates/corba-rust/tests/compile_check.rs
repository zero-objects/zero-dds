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
//! ## One workspace, one `cargo check`
//!
//! Every case is a member crate of one workspace under the test target
//! directory, checked with one `cargo check --workspace` into a target
//! directory of its own that the next run reuses: the dependencies are
//! checked once, not once per case. A case that does not compile names
//! its package in the compiler's message.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::io::Write;
use std::path::{Path, PathBuf};
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

/// The cases: the name of the member crate and its IDL.
const CASES: &[(&str, &str)] = &[
    (
        "simple_interface",
        r"interface Calculator {
            long add(in long a, in long b);
        };",
    ),
    (
        "valuetype_state",
        r"valuetype Point {
            public long x;
            public long y;
        };",
    ),
    // CORBA Messaging §22: `@ami` → an AmiHandler trait, `sendc_`,
    // `sendp_` and a poller. Covers return+in, void+no-out, and
    // return+in+out.
    (
        "ami",
        r"interface Bank {
            @ami long deposit(in long amount);
            @ami void close();
            @ami long transfer(in long amount, out long balance);
        };",
    ),
    // §15.3.4 state flattening: `ExtendedValue` carries `version` from
    // the base and `name` of its own, and marshals them in that order.
    (
        "valuetype_inheritance",
        r"valuetype Base {
            public long version;
        };
        valuetype Extended : Base {
            public string name;
        };",
    ),
    (
        "iface_inheritance",
        r"interface Base {
            void ping();
        };
        interface Derived : Base {
            long get_id();
        };",
    ),
    (
        "iface_raises",
        r#"exception NotFound { string what; };
        interface Vault {
            string lookup(in string key) raises (NotFound);
        };"#,
    ),
    // The property this file exists for: a `struct` the data column
    // declares, used as a parameter and a return by the service column.
    // The two have to name it the same way and agree that it has an
    // inherent `encode` and `decode`.
    (
        "aggregates",
        r"struct Reading { long id; double value; };
        interface Sensor {
            Reading latest();
            void push(in long n, out Reading r);
            long count(in sequence<Reading> batch);
        };",
    ),
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
    (
        "enum_boundary",
        r"enum Health { OK, DEGRADED };
        interface Monitor {
            Health status();
            void set(in Health h);
        };",
    ),
    // An enumeration as a *member*: the data column writes the member
    // through its holder, and the field it declares for it is the
    // enumeration, not an integer.
    (
        "enum_member",
        r"enum Health { OK, DEGRADED };
        struct Status { Health h; long n; };
        interface Monitor {
            Status current();
        };",
    ),
];

/// The source of one member crate: the data column, then the service
/// column.
fn source(idl: &str) -> String {
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
    format!(
        "#![allow(dead_code, unused_imports, unused_variables, clippy::all)]\n\
         use zerodds_wire::{{Reader, Writer, WireError}};\n\
         {data}\n{service}"
    )
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    std::fs::File::create(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .expect("write");
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|path| path.join("Cargo.lock").exists())
        .map_or(manifest.clone(), std::path::Path::to_path_buf)
}

/// `cargo check` over a workspace of one member crate per entry of
/// `members` (name, source) under `CARGO_TARGET_TMPDIR/<workspace>`, into
/// the target directory every workspace of this file shares.
fn check(workspace: &str, members: &[(&str, String)]) -> std::process::Output {
    let root = workspace_root();
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("corba-rust-compile-check");
    let dir = base.join(workspace);
    let names: Vec<String> = members
        .iter()
        .map(|(name, _)| format!("\"{name}\""))
        .collect();
    write(
        &dir.join("Cargo.toml"),
        &format!(
            "[workspace]\nresolver = \"3\"\nmembers = [{}]\n",
            names.join(", ")
        ),
    );
    // The versions the repository locked, so that `--offline` finds them.
    std::fs::copy(root.join("Cargo.lock"), dir.join("Cargo.lock")).expect("copy Cargo.lock");
    for (name, source) in members {
        write(
            &dir.join(name).join("Cargo.toml"),
            &format!(
                r#"[package]
name = "compile_check_{name}"
version = "0.0.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
zerodds-corba-rust = {{ path = "{root}/crates/corba-rust" }}
zerodds-corba-rt = {{ path = "{root}/crates/corba-rt" }}
zerodds-wire = {{ path = "{root}/crates/wire" }}
zerodds-float128 = {{ path = "{root}/crates/float128" }}
"#,
                root = root.display()
            ),
        );
        write(&dir.join(name).join("src/lib.rs"), source);
    }
    Command::new(env!("CARGO"))
        .arg("check")
        .arg("--workspace")
        .arg("--offline")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(dir.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(base.join("target"))
        .output()
        .expect("cargo invocation")
}

/// zerodds-corba-rust 1.1 §3, §3.1, §4.1, §5 and §9: the code generated
/// for each case compiles together with the data column, and every name
/// in it resolves.
#[test]
fn corbarust_3_the_generated_code_compiles_with_the_data_column() {
    let members: Vec<(&str, String)> = CASES
        .iter()
        .map(|(name, idl)| (*name, source(idl)))
        .collect();
    let output = check("generated", &members);
    assert!(
        output.status.success(),
        "the generated code does not compile (exit {:?})\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The check above can fail: a member with a type error is reported by
/// the compiler, with its package.
#[test]
fn the_compile_check_reports_a_type_error() {
    let output = check(
        "deliberate-error",
        &[(
            "deliberate_error",
            "pub fn answer() -> i32 { \"forty-two\" }\n".to_string(),
        )],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "a type error must fail the check");
    assert!(stderr.contains("E0308"), "{stderr}");
    assert!(
        stderr.contains("compile_check_deliberate_error"),
        "{stderr}"
    );
}

/// OMG IDL 4.2 §7.4.7.4.4 and CORBA 3.3 part 1 §9.5.1: the state of a
/// `custom` valuetype is marshaled by code the application provides,
/// and that code writes the state of the bases too. The compiler is the
/// oracle: with the application's `PointCustomMarshal` on `PointValue`
/// the generated code compiles; without it, the generated
/// `ValueMarshal` and the registered factory name the trait that is not
/// implemented.
#[test]
fn idl_7_4_7_4_4_a_custom_valuetype_is_marshaled_by_the_application() {
    let generated = source(
        r"valuetype Base { public long version; };
        custom valuetype Point : Base { public long x; public long y; };",
    );
    assert!(
        generated.contains("pub trait PointCustomMarshal"),
        "{generated}"
    );
    assert!(
        !generated.contains("pub trait BaseCustomMarshal"),
        "only the custom valuetype has one: {generated}"
    );
    // The application's marshaling: its own order, the base's state
    // included.
    let application = r"
impl PointCustomMarshal for PointValue {
    fn marshal(&self, os: &mut zerodds_wire::Writer) -> ::core::result::Result<(), zerodds_wire::WireError> {
        os.write_i32(self.y);
        os.write_i32(self.x);
        os.write_i32(self.version);
        Ok(())
    }
    fn unmarshal(is: &mut zerodds_wire::Reader<'_>) -> ::core::result::Result<Self, zerodds_wire::WireError> {
        let y = is.read_i32()?;
        let x = is.read_i32()?;
        let version = is.read_i32()?;
        Ok(Self { version, x, y })
    }
}
";
    let with = check(
        "custom-marshal",
        &[("custom_marshal", format!("{generated}{application}"))],
    );
    assert!(
        with.status.success(),
        "with the application's marshaling the code compiles (exit {:?})\n{}",
        with.status.code(),
        String::from_utf8_lossy(&with.stderr)
    );
    let without = check(
        "custom-marshal-missing",
        &[("custom_marshal_missing", generated)],
    );
    let stderr = String::from_utf8_lossy(&without.stderr);
    assert!(
        !without.status.success(),
        "without the application's marshaling the code must not compile"
    );
    assert!(
        stderr.contains("E0277") && stderr.contains("PointCustomMarshal"),
        "{stderr}"
    );
}

/// OMG IDL 4.2 §7.4.5.3, rule (105): a value type body holds attributes
/// beside its state and operations, and they map as an interface's do —
/// a getter, and a setter unless the attribute is `readonly`. The
/// compiler is the oracle: code that reads both attributes and writes the
/// writable one compiles against the generated trait; code that writes the
/// `readonly` one does not, for want of the method.
#[test]
fn idl_7_4_5_3_a_value_types_attributes_are_methods_of_its_trait() {
    let generated = source(
        r"valuetype Account {
            public long balance;
            attribute string owner;
            readonly attribute long id;
        };",
    );
    let application = r"
pub fn touch<T: Account>(account: &T) {
    let _ = account.balance();
    let _ = account.owner();
    let _ = account.set_owner(::core::default::Default::default());
    let _ = account.id();
}
";
    let with = check(
        "value-attribute",
        &[("value_attribute", format!("{generated}{application}"))],
    );
    assert!(
        with.status.success(),
        "the attributes are methods of the trait (exit {:?})\n{}",
        with.status.code(),
        String::from_utf8_lossy(&with.stderr)
    );
    let readonly = r"
pub fn write_id<T: Account>(account: &T) {
    let _ = account.set_id(7);
}
";
    let without = check(
        "value-attribute-readonly",
        &[("value_attribute_readonly", format!("{generated}{readonly}"))],
    );
    let stderr = String::from_utf8_lossy(&without.stderr);
    assert!(
        !without.status.success(),
        "a readonly attribute has no setter"
    );
    assert!(
        stderr.contains("E0599") && stderr.contains("set_id"),
        "{stderr}"
    );
}
