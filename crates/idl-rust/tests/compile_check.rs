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
zerodds-types = {{ path = "{}/crates/types" }}
"#,
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

/// Like [`compile_generated`], but proves the idiomatic multi-file
/// pattern: each `(module, idl)` pair is generated into its OWN file under
/// `src/generated/<module>.rs`, then all of them are pulled into a SINGLE
/// parent module via `include!` — exactly what a hand-written `build.rs`/
/// `lib.rs` does when several `.idl` files are compiled separately and
/// composed. Compiled with `-D warnings` so a reintroduced dead top-level
/// `use` (unused-import warning) also fails, not just hard errors.
///
/// Reproduces the two `include!`-composability failures reported against
/// idl-rust. First, inner attributes (`#![allow(..)]`) at file scope are
/// rejected inside a parent module ("an inner attribute is not permitted in
/// this context", #26). Second, a duplicated top-level `use zerodds_cdr::{..}`
/// raises E0252 ("defined multiple times", #25) when two generated files land
/// in the same module. Both are fixed by emitting no file-level inner
/// attributes and no top-level `use`: lint allows became per-item outer
/// attributes, and every external path is fully qualified.
fn compile_generated_multifile(test_name: &str, modules: &[(&str, &str)]) {
    let tmp = std::env::temp_dir().join(format!("dds_idl_rust_mf_{test_name}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src/generated")).expect("mkdir");

    let workspace_root = workspace_root();
    let cargo_toml = format!(
        r#"[package]
name = "compile_test_mf_{test_name}"
version = "0.0.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
zerodds-cdr = {{ path = "{r}/crates/cdr" }}
zerodds-dcps = {{ path = "{r}/crates/dcps" }}
zerodds-sql-filter = {{ path = "{r}/crates/sql-filter" }}
zerodds-types = {{ path = "{r}/crates/types" }}
"#,
        r = workspace_root.display()
    );
    std::fs::File::create(tmp.join("Cargo.toml"))
        .expect("create Cargo.toml")
        .write_all(cargo_toml.as_bytes())
        .expect("write Cargo.toml");

    // Generate each IDL into its own file (separate `generate_rust_module`
    // calls, as a real multi-file build would do).
    let mut includes = String::new();
    for (module, idl) in modules {
        let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
        let rust_src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");
        std::fs::File::create(tmp.join(format!("src/generated/{module}.rs")))
            .expect("create module file")
            .write_all(rust_src.as_bytes())
            .expect("write module file");
        includes.push_str(&format!("    include!(\"generated/{module}.rs\");\n"));
    }

    // The composition site: one parent module, many `include!`d files.
    let lib_rs = format!("mod generated {{\n{includes}}}\n");
    std::fs::File::create(tmp.join("src/lib.rs"))
        .expect("create lib.rs")
        .write_all(lib_rs.as_bytes())
        .expect("write lib.rs");

    let status = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(tmp.join("Cargo.toml"))
        .arg("--offline")
        .env("RUSTFLAGS", "-D warnings")
        .status();

    match status {
        Ok(s) if s.success() => { /* good */ }
        Ok(s) => panic!(
            "multi-file include! did not compile (exit {:?}). lib.rs:\n{lib_rs}",
            s.code(),
        ),
        Err(e) => panic!("cargo invocation failed: {e}"),
    }
}

/// Regression for the two reported idl-rust `include!` bugs (#25/#26): two
/// module-wrapped IDLs, generated separately, composed into one parent
/// module via `include!`. Must compile without the inner-attribute error
/// and without the E0252 "defined multiple times" collision.
#[test]
#[ignore = "requires cargo offline + path-deps; run with --include-ignored"]
fn compile_check_multifile_include_composition() {
    compile_generated_multifile(
        "two_modules",
        &[
            (
                "common",
                "module common { struct KeyLabel { @key uint32 id; string label; }; };",
            ),
            (
                "metrics",
                "module metrics { struct SystemCpuInfo { @key string id; string brand; \
                 string name; float usage; }; };",
            ),
        ],
    );
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

/// Regression for #20: a `@key` member whose type is a typedef alias to a
/// primitive must dealias, not hard-error. `emit_key_field_write` /
/// `key_holder_atom_size` (struct_emit.rs) previously called only
/// `type_map::struct_def_by_scoped` on `TypeSpec::Scoped` — a typedef chasing
/// to a non-struct terminal (a plain `long`) fell straight into the
/// "complex @key field (enum or unresolved nested type)" error, even though
/// `type_map::resolve_typedef_to_spec` (already used for non-key member
/// types) can chase the alias to its primitive. Fast (no cargo): asserts the
/// codegen succeeds and the KeyHolder writer uses the primitive's `write_i32`,
/// not an error.
#[test]
fn typedef_key_resolves_to_primitive_leaf() {
    let idl = r#"typedef long MyId; struct S { @key MyId id; };"#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let src = generate_rust_module(&ast, &RustGenOptions::default())
        .expect("typedef-to-primitive @key member must generate, not hard-error (regression #20)");
    assert!(
        src.contains("holder.write_i32(self.id);"),
        "typedef @key must dealias to the primitive KeyHolder write:\n{src}"
    );
    // KEY_HOLDER_MAX_SIZE must be computed (Some), not fall back to the
    // dynamic-size MD5 path (None) — the dealiased primitive has a known
    // fixed atom size.
    assert!(
        src.contains("KEY_HOLDER_MAX_SIZE"),
        "typedef @key must still get a static KeyHolder size:\n{src}"
    );
}

/// Regression for #20, chain variant: `resolve_typedef_to_spec` chases up to
/// 16 alias levels; a 2-level typedef chain (`Distance -> Meters -> long`)
/// used as `@key` must resolve all the way to the primitive.
#[test]
fn typedef_key_two_level_chain_resolves_to_primitive() {
    let idl = r#"typedef long Meters; typedef Meters Distance; struct S { @key Distance d; };"#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let src = generate_rust_module(&ast, &RustGenOptions::default())
        .expect("2-level typedef chain @key must resolve to the primitive (regression #20)");
    assert!(
        src.contains("holder.write_i32(self.d);"),
        "2-level typedef chain @key must dealias to the primitive KeyHolder write:\n{src}"
    );
}

/// Regression for #20, non-key control: a NON-`@key` member of the same
/// typedef-aliased type must be unaffected by the key-path fix (it never used
/// the broken path — `emit_member_encode`/`rust_type_for` already dealias
/// correctly). Guards against a fix that only works for `@key`-tagged fields.
#[test]
fn typedef_non_key_member_still_generates() {
    let idl = r#"typedef long MyId; struct S { MyId id; @key long other; };"#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let src = generate_rust_module(&ast, &RustGenOptions::default())
        .expect("non-key typedef member must keep generating (control, regression #20)");
    assert!(
        src.contains("pub id: MyId,") && src.contains("pub type MyId = i32;"),
        "typedef-to-primitive non-key field keeps the alias type (which resolves to i32):\n{src}"
    );
    assert!(
        src.contains("holder.write_i32(self.other);"),
        "the plain-primitive @key member must be unaffected:\n{src}"
    );
}

/// Regression for #20, extended check: a `@key` member whose type is a
/// typedef to an ENUM must still produce the correctly-labeled
/// "complex @key field" error — not a crash and not a silently wrong
/// primitive write. `resolve_typedef_to_spec` chases the typedef to the
/// enum's own scoped name, which then falls through to the existing
/// `struct_def_by_scoped`-fails guard.
#[test]
fn typedef_key_of_enum_gives_labeled_error_not_crash() {
    let idl = r#"enum Kind { K_A, K_B }; typedef Kind KindAlias; struct S { @key KindAlias k; };"#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let err = generate_rust_module(&ast, &RustGenOptions::default())
        .expect_err("typedef-to-enum @key must be a labeled error, not succeed silently-wrong");
    match err {
        zerodds_idl_rust::RustGenError::Unsupported { what, .. } => {
            assert_eq!(
                what, "complex @key field (enum or unresolved nested type)",
                "typedef-to-enum @key must keep the enum-labeled error"
            );
        }
        other => panic!("expected Unsupported{{enum..}}, got {other:?}"),
    }
}

/// Regression for #20, extended check: a `@key` member whose type is a
/// typedef to a SEQUENCE must land in the sequence-labeled error, not the
/// generic "enum or unresolved nested type" one — `resolve_typedef_to_spec`
/// resolves straight to `TypeSpec::Sequence`, which recurses into the
/// existing, correctly-named `Unsupported{"complex @key field (sequence)"}`
/// arm instead of stopping at the Scoped arm's fallback.
#[test]
fn typedef_key_of_sequence_gives_labeled_error() {
    let idl = r#"typedef sequence<long> Ids; struct S { @key Ids ids; long dummy; };"#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let err = generate_rust_module(&ast, &RustGenOptions::default())
        .expect_err("typedef-to-sequence @key must be a labeled error");
    match err {
        zerodds_idl_rust::RustGenError::Unsupported { what, .. } => {
            assert_eq!(
                what, "complex @key field (sequence)",
                "typedef-to-sequence @key must get the sequence-labeled error, not the generic one"
            );
        }
        other => panic!("expected Unsupported{{sequence}}, got {other:?}"),
    }
}

/// Bug #20 end-to-end: the typedef-aliased `@key` member must actually
/// compile (not just generate source that looks right).
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_typedef_key_primitive() {
    compile_generated(
        "typedef_key_primitive",
        r#"typedef long MyId; struct S { @key MyId id; double value; };"#,
    );
}

/// Regression for #22 (encode-bound half): `emit_member_bound_checks` did
/// not thread the `@optional` flag into `emit_bound_checks_decl`, so a
/// bounded `string`/`wstring`/`sequence`/`map` member that is ALSO
/// `@optional` hard-errored the generated encoder (`self.{field}.len()` on
/// an `Option<String>` — `Option` has no `.len()`). Fast (no cargo): asserts
/// the generated encode body guards the bound check behind an
/// `if let Some(..)`, not a bare `.len()` on the `Option` itself.
#[test]
fn optional_bounded_string_bound_check_guards_through_option() {
    let idl = r#"struct S { @optional string<8> label; long id; };"#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let src = generate_rust_module(&ast, &RustGenOptions::default())
        .expect("optional bounded string must generate, not hard-error (regression #22)");
    assert!(
        src.contains("if let ::core::option::Option::Some(ref __zd_opt_b) = self.label"),
        "the bound check on an @optional bounded field must be guarded by an Option check:\n{src}"
    );
    assert!(
        !src.contains("self.label.len()"),
        "must never call .len() directly on the Option-wrapped field:\n{src}"
    );
}

/// Regression for #22 end-to-end: `@optional string<N>` (the reported repro
/// shape) must actually compile.
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_optional_bounded_string() {
    compile_generated(
        "optional_bounded_string",
        r#"struct S { @optional string<8> label; long id; };"#,
    );
}

/// Regression for #22 (decode-bound half, the SEVERE finding): before this
/// fix, a generated `decode` only checked the remaining wire buffer
/// (`zerodds_cdr` composite decoders), never the IDL-declared bound N — a
/// `string<8>` field would decode an arbitrarily large well-formed payload
/// without complaint (XTypes 1.3 §7.4.3 requires the bound enforced on BOTH
/// encode and decode). Fast (no cargo): asserts the generated decode body
/// now checks the decoded value's length against the bound and raises
/// `DecodeError::BoundExceeded`, for both a bounded string and a bounded
/// sequence.
#[test]
fn decode_body_rejects_over_bound_values_not_just_remaining_buffer() {
    let idl = r#"struct S { string<8> label; sequence<long,4> vals; };"#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");
    assert!(
        src.contains(
            "zerodds_cdr::DecodeError::BoundExceeded { actual: __zd_dv.len(), bound: 8, message: \"decoded string length exceeds its IDL bound (8)\" }"
        ),
        "decode must reject a bounded string exceeding its IDL bound, not just check remaining buffer:\n{src}"
    );
    assert!(
        src.contains(
            "zerodds_cdr::DecodeError::BoundExceeded { actual: __zd_dv.len(), bound: 4, message: \"decoded sequence length exceeds its IDL bound (4)\" }"
        ),
        "decode must reject a bounded sequence exceeding its IDL bound:\n{src}"
    );
}

/// Regression for #22, `@optional` decode-bound variant: the decode-side
/// bound check must also fire through the `Option` wrapper (guarded by
/// `if let Some`), symmetric to the encode-side fix above.
#[test]
fn decode_body_rejects_over_bound_optional_value() {
    let idl = r#"struct S { @optional string<8> label; long id; };"#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");
    assert!(
        src.contains("if let ::core::option::Option::Some(ref __zd_dvi) = __zd_dv"),
        "decode-side bound check on an @optional bounded field must be guarded by an Option check:\n{src}"
    );
    assert!(
        src.contains("zerodds_cdr::DecodeError::BoundExceeded { actual: __zd_dvi.len(), bound: 8"),
        "decode-side bound check must run on the unwrapped Option value:\n{src}"
    );
}

/// Regression for #22, control: a bounded member with NO violation must
/// still round-trip (the new checks must not be over-eager / off-by-one).
/// Also exercises the wire-level DecodeError::BoundExceeded plumbing end to
/// end via the raw CdrEncode/CdrDecode path (not just DdsType, whose
/// decode-error detail collapses through a pre-existing, unrelated
/// zerodds-dcps placeholder shim — not this fix's concern).
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_decode_bound_enforcement() {
    compile_generated(
        "decode_bound_enforcement",
        r#"struct S { string<8> label; sequence<long,4> vals; @optional wstring<3> w; };"#,
    );
}

/// Coverage gap (NOT a correctness bug — deep review of #22
/// decode-bounds-cross-backend, self-check of the reference `idl-rust`
/// backend): `string<N> names[3]` (an ARRAY of a bounded string) has no test
/// proving its per-element decode bound check, even though the
/// implementation already handles it — `array_elem_needs_manual_decode`
/// (`type_map.rs`) routes `TypeSpec::String` through
/// `emit_manual_array_decode` (`struct_emit.rs`), whose innermost per-element
/// branch already calls `emit_decode_bound_checks` when
/// `type_has_bounds(elem_spec)` holds (see its own doc comment: "B1
/// follow-up array-of-bounded-element gap ... previously decoded each
/// element with no IDL-bound check at all"). This test closes the coverage
/// gap the existing `array_of_bounded_sequence_checks_element_not_array_length`
/// test (`bounded_sequence.rs`) left open for `string<N>[...]` specifically
/// (that test only covers `sequence<octet,4>[...]`).
#[test]
fn decode_body_rejects_over_bound_array_of_bounded_string() {
    let idl = r#"struct A { string<4> names[3]; };"#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");
    assert!(
        src.contains(
            "zerodds_cdr::DecodeError::BoundExceeded { actual: __zd_dv.len(), bound: 4, message: \"decoded string length exceeds its IDL bound (4)\" }"
        ),
        "array-of-bounded-string element decode must reject an over-bound \
         element, not just check the wire's remaining buffer:\n{src}"
    );
    // Must be the per-element MANUAL array decode path (String has no Copy
    // impl, so it cannot ride the `[T; N]: CdrDecode` blanket impl) — proves
    // the check is reached via `emit_manual_array_decode`, not skipped
    // because the array took some other, unchecked path.
    assert!(
        src.contains("let mut __arr: ::std::vec::Vec<") && src.contains("__arr.push("),
        "array-of-bounded-string must decode via the manual per-element array \
         decoder (String is not Copy, so the blanket [T; N] impl cannot apply):\n{src}"
    );
}

/// End-to-end compile proof for the coverage-gap test above: an array of a
/// bounded string must actually compile, not just look right in the
/// generated text.
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn compile_check_decode_bound_array_of_bounded_string() {
    compile_generated(
        "decode_bound_array_of_bounded_string",
        r#"struct A { string<4> names[3]; };"#,
    );
}
