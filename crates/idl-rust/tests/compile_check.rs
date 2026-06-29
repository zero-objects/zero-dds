// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Compile check: emits Rust code from IDL and runs `rustc --emit=metadata`
//! against the generated code with zerodds_cdr + zerodds_dcps as deps.
//!
//! Proves phase H: the codegen output is not only snapshottable but
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

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

/// Emits Rust code, writes it to a temp file and runs
/// `cargo check` against a test crate that includes the code as a
/// module. Fails with pretty diagnostics if the
/// generated code does not compile.
fn compile_generated(name: &str, idl: &str) {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let rust_src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");

    let tmp = std::env::temp_dir().join(format!("dds_idl_rust_compile_{name}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).expect("mkdir");

    // Test-crate Cargo.toml with path deps on zerodds-cdr + zerodds-dcps.
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

/// Regression for Bug A: `field_value` must resolve scoped field types — a
/// nested struct forwards `.field_value()`, but a typedef-to-primitive or an
/// enum is a terminal value (forwarding those does not compile). Models the
/// NGVA shape (AEP-4754 Vol V §8.1.3: typedef-in-units + enum + nested struct).
/// Fast (no cargo) — asserts on the generated source.
#[test]
fn ngva_field_value_resolves_leaf_vs_struct() {
    let idl = r#"module nga {
      typedef double CurrentInAmpsType;
      enum CoordinateSystemType { COORDINATE_SYSTEM_TYPE__BNG, COORDINATE_SYSTEM_TYPE__MGRS };
      @final struct LinearVelocity2DType { double xComponent; double yComponent; };
      @appendable struct Nav {
        @key string<32>      vehicleId;
        LinearVelocity2DType velocity;
        CoordinateSystemType coordinateSystem;
        CurrentInAmpsType    batteryCurrent;
      };
    };"#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");

    // nested struct member → forwards field_value()
    assert!(
        src.contains("self.velocity.field_value"),
        "struct member must forward field_value()"
    );
    // typedef-to-primitive → terminal leaf, NOT forwarded
    assert!(
        !src.contains("self.batteryCurrent.field_value"),
        "typedef-to-primitive must not forward field_value()"
    );
    assert!(
        src.contains("\"batteryCurrent\" =>"),
        "typedef-to-primitive must emit a terminal value arm"
    );
    // enum → terminal int, NOT forwarded
    assert!(
        !src.contains("self.coordinateSystem.field_value"),
        "enum must not forward field_value()"
    );
}

/// Bug A end-to-end: the NGVA-shaped struct must actually compile
/// (typedef-primitive + enum + nested-struct members). Opt-in like the others.
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_ngva_field_value() {
    compile_generated(
        "ngva_field_value",
        r#"module nga {
          typedef double CurrentInAmpsType;
          typedef float  HeadingType;
          enum CoordinateSystemType { COORDINATE_SYSTEM_TYPE__BNG, COORDINATE_SYSTEM_TYPE__MGRS };
          @final struct LinearVelocity2DType { double xComponent; double yComponent; };
          @appendable struct Navigation_Resource_Specification {
            @key string<32>      vehicleId;
            HeadingType          heading;
            LinearVelocity2DType velocity;
            CoordinateSystemType coordinateSystem;
            CurrentInAmpsType    batteryCurrent;
          };
        };"#,
    );
}

/// Bug L: an array bound written as a named/scoped constant or a const
/// expression (§7.4.1.4.4.5 `positive_int_const ::= const_expr`) must parse and
/// — because the Rust backend does not emit IDL `const` declarations — be
/// resolved to its integer value at codegen time.
#[test]
fn array_bound_by_named_const_resolves() {
    let idl = "module conf { \
                 const long N = 4; \
                 const long M = N * 2; \
                 struct S { long v[N]; long w[M]; long z[1 << 3]; }; \
               };";
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let rust = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");
    assert!(rust.contains("[i32; 4]"), "named const bound N=4:\n{rust}");
    assert!(
        rust.contains("[i32; 8]"),
        "const expr M=N*2=8 / 1<<3=8:\n{rust}"
    );
}

/// Bug H: a union with an enum (scoped) discriminator must generate, with the
/// discriminant repr i32 (DDS enums are 32-bit, §7.4.5.1) and `case ENUM_LIT:`
/// labels resolved to the same discriminants the generated enum uses.
#[test]
fn union_with_enum_discriminator_resolves_labels() {
    let idl = "module conf { \
                 enum Kind { K_A, K_B, K_C }; \
                 union EnumUnion switch (Kind) { \
                   case K_A: long a; \
                   case K_B: short b; \
                   default: octet other; \
                 }; \
               };";
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let rust = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");
    // K_A → discriminant 0, K_B → 1 (matching enum_emit), encoded as i32.
    assert!(
        rust.contains("encode(&((0) as i32)") && rust.contains("encode(&((1) as i32)"),
        "enum case labels K_A/K_B must resolve to 0/1 as i32:\n{rust}"
    );
}

/// Compile gate for Bug H (cargo check, ignored — needs offline path deps).
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_union_enum_discriminator() {
    compile_generated(
        "union_enum_disc",
        "module conf { \
           enum Kind { K_A, K_B, K_C }; \
           union EnumUnion switch (Kind) { \
             case K_A: long a; \
             case K_B: short b; \
             default: octet other; \
           }; \
         };",
    );
}

/// Edge-hardening: an `@optional` NESTED-STRUCT member must compile. The
/// `field_value` dotted-path arm used to emit `self.opt.field_value(..)`
/// where `opt` is `Option<T>` (no `field_value` method) → E0599. The arm
/// now forwards through the Option (`as_ref().and_then(..)`).
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_optional_nested_struct_field_value() {
    compile_generated(
        "optional_nested_struct",
        "@nested @final struct Inner { long v; }; \
         @final struct Outer { @optional Inner inner; long id; };",
    );
}

/// Edge-hardening: a `wstring` member's `field_value` must compile. The
/// generated field type is `zerodds_cdr::WString` (a `String` newtype), so
/// the `Value::String` arm must go through `.as_str().to_string()` rather
/// than `.clone()` (which would yield a `WString`, not a `String`) → E0308.
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_wstring_field_value() {
    compile_generated(
        "wstring_field_value",
        "@final struct WText { wstring label; @optional wstring note; };",
    );
}

/// Edge-hardening: a fixed-size ARRAY-OF-STRING member must compile. The
/// blanket `impl CdrDecode for [T; N]` requires `T: Copy`; `String` is not
/// `Copy`, so `<[String; 3] as CdrDecode>::decode` would not compile. The
/// codegen now emits an element-wise manual decoder for non-`Copy` array
/// elements (struct/union/string/sequence/map).
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_array_of_string_member() {
    compile_generated(
        "array_of_string",
        "@final struct Names { string names[3]; sequence<long> rows[2]; };",
    );
}
