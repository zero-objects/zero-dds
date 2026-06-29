// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Snapshot tests for the Rust codegen.

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

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

fn run(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    generate_rust_module(&ast, &RustGenOptions::default()).expect("gen")
}

#[test]
fn snapshot_simple_struct_primitives_only() {
    let idl = r#"
        struct Point {
            long x;
            long y;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_full_primitive_set() {
    let idl = r#"
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
            char c;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_enum() {
    let idl = r#"
        enum Color {
            RED,
            GREEN,
            BLUE
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

/// Bug O (#58): explicit `@value(N)` must drive both the `#[repr(i32)]`
/// discriminant and the `from_wire` reverse map (here 1/2/8, NOT 0/1/2).
#[test]
fn snapshot_enum_explicit_value() {
    let idl = r#"
        enum Sparse {
            @value(1) ONE,
            @value(2) TWO,
            @value(8) EIGHT
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

/// T2 (typesystem oracle F2): `@bit_bound(N)` selects the signed-integer
/// wire width of the enum holder (XTypes 1.3 §7.4.5.1 / §7.3.1.2.1.2) —
/// N≤8 → `i8` (1 octet), N≤16 → `i16` (2 octets), else `i32`. Cyclone honours
/// this; the prior fixed-`i32` path silently dropped `@bit_bound`. This is a
/// direct assertion (not a snapshot) so it stays decoupled from the embedded
/// TypeIdentifier-hash churn of the struct snapshots.
#[test]
fn bit_bound_enum_narrows_wire_width() {
    let gen8 = run(r#"
        @bit_bound(8)
        enum Tiny { T_A, T_B, T_C };
    "#);
    assert!(
        gen8.contains("<i8 as zerodds_cdr::CdrEncode>::encode(&(*self as i8), w)"),
        "@bit_bound(8) enum must encode as i8:\n{gen8}"
    );
    assert!(
        gen8.contains("i32::from(<i8 as zerodds_cdr::CdrDecode>::decode(r)?)"),
        "@bit_bound(8) enum must decode from i8:\n{gen8}"
    );

    let gen16 = run(r#"
        @bit_bound(16)
        enum Mid { M_A, M_B, M_C, M_D };
    "#);
    assert!(
        gen16.contains("<i16 as zerodds_cdr::CdrEncode>::encode(&(*self as i16), w)"),
        "@bit_bound(16) enum must encode as i16:\n{gen16}"
    );

    // Default (no @bit_bound) stays a full 32-bit i32 holder (§7.4.5.1).
    let gen_default = run(r#"
        enum Wide { W_A, W_B };
    "#);
    assert!(
        gen_default.contains("<i32 as zerodds_cdr::CdrEncode>::encode(&(*self as i32), w)"),
        "default enum must stay i32:\n{gen_default}"
    );
    assert!(
        !gen_default.contains("as i8"),
        "default enum must not narrow:\n{gen_default}"
    );
}

/// Bug R2 (#61): the decode of an array-of-struct member must name the
/// declarator-wrapped array type `[Point; 2]`, not the bare element type.
#[test]
fn snapshot_struct_array_of_struct() {
    let idl = r#"
        @nested struct Point { long x; long y; };
        struct Shapes { Point shape[2]; };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_typedef() {
    let idl = r#"
        typedef long Distance;
        typedef long Coord[3];
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_module_nested() {
    let idl = r#"
        module geom {
            struct Pose {
                double x;
                double y;
                double z;
            };
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_appendable_struct() {
    let idl = r#"
        @appendable
        struct Telemetry {
            unsigned long timestamp;
            double value;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_mutable_struct_with_ids() {
    let idl = r#"
        @mutable
        struct UserPrefs {
            @id(10) string name;
            @id(20) long age;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_with_string_and_sequence() {
    let idl = r#"
        struct Message {
            string topic;
            sequence<long> values;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_with_array_dimensions() {
    let idl = r#"
        struct Matrix3x3 {
            double cells[3][3];
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_with_single_key() {
    let idl = r#"
        struct Reading {
            @key long sensor_id;
            double value;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_with_multi_key_id_sorting() {
    let idl = r#"
        struct Composite {
            @key @id(20) long b;
            @key @id(10) long a;
            string payload;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_with_string_key_unbounded() {
    let idl = r#"
        struct UserRecord {
            @key string user_id;
            long age;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_bitset_with_named_bitfields() {
    let idl = r#"
        bitset Status {
            bitfield<1> ready;
            bitfield<1> error;
            bitfield<2> level;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_bitmask_with_const_values() {
    let idl = r#"
        bitmask Permissions {
            READ,
            WRITE,
            EXECUTE
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_with_fixed_field() {
    let idl = r#"
        struct Price {
            fixed<10, 2> amount;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_with_map_field() {
    let idl = r#"
        struct Lookup {
            map<long, string> entries;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_with_any_field() {
    let idl = r#"
        struct Envelope {
            string topic;
            any payload;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_optional_member_field_wraps_in_option() {
    let idl = r#"
        struct Profile {
            string name;
            @optional long age;
            @optional string email;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_nested_struct_emits_is_nested_const() {
    let idl = r#"
        @nested
        struct Inner {
            long x;
            long y;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_with_rust_reserved_word_identifiers() {
    // IDL identifiers like `type`, `match`, `mod`, `fn` are Rust-reserved.
    // The codegen must escape them as raw identifiers `r#…`.
    let idl = r#"
        struct match {
            long type;
            long mod;
            long fn;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_struct_with_field_value_filter_paths() {
    let idl = r#"
        struct SensorReading {
            long sensor_id;
            double value;
            string label;
            boolean valid;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_union() {
    let idl = r#"
        union Event switch (long) {
            case 1: long counter;
            case 2: double value;
            default: string message;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}
