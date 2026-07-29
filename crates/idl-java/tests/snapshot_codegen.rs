//! Snapshot tests for the Java codegen output.
//!
//! Compares the emitted `.java` files against snapshot files.
//! Changes via `cargo insta review`.

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
use zerodds_idl_java::{JavaGenOptions, generate_java_files, generate_java_files_with_amqp};

fn gen_default(src: &str) -> String {
    // Snapshots focus on POJO output. TypeSupport snapshots
    // live in separate tests (see `snapshot_typesupport_*`).
    let opts = JavaGenOptions {
        emit_typesupport: false,
        ..Default::default()
    };
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let files = generate_java_files(&ast, &opts).expect("gen");
    let mut combined = String::new();
    for f in &files {
        combined.push_str(&format!(
            "// === FILE: {}/{}.java ===\n",
            f.package_path, f.class_name
        ));
        combined.push_str(&f.source);
        combined.push('\n');
    }
    combined
}

fn gen_with_amqp(src: &str) -> String {
    let opts = JavaGenOptions {
        emit_typesupport: false,
        ..Default::default()
    };
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let files = generate_java_files_with_amqp(&ast, &opts).expect("gen");
    let mut combined = String::new();
    for f in &files {
        combined.push_str(&format!(
            "// === FILE: {}/{}.java ===\n",
            f.package_path, f.class_name
        ));
        combined.push_str(&f.source);
        combined.push('\n');
    }
    combined
}

#[test]
fn snapshot_simple_struct() {
    insta::assert_snapshot!(gen_default("struct Point { long x; long y; };"));
}

#[test]
fn snapshot_struct_with_string_and_sequence() {
    insta::assert_snapshot!(gen_default(
        "struct Bag { string name; sequence<long> ids; };"
    ));
}

#[test]
fn snapshot_typesupport_sequence_of_string_dheader() {
    // XCDR2 §7.4.3.5: seq<string> (non-primitive) → DHEADER (beginAppendable
    // / readDHeader); seq<long> has none. Cyclone-DDS-verified.
    insta::assert_snapshot!(gen_typesupport(
        "@final struct Tags { sequence<string> tags; };"
    ));
}

#[test]
fn snapshot_module_nesting() {
    insta::assert_snapshot!(gen_default(
        "module Outer { module Inner { struct S { long x; }; }; };"
    ));
}

#[test]
fn snapshot_enum() {
    insta::assert_snapshot!(gen_default("enum Color { RED, GREEN, BLUE };"));
}

#[test]
fn snapshot_union() {
    insta::assert_snapshot!(gen_default(
        "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };"
    ));
}

#[test]
fn snapshot_inheritance() {
    insta::assert_snapshot!(gen_default(
        "struct Base { long base_field; }; struct Child : Base { long child_field; };"
    ));
}

#[test]
fn snapshot_amqp_helpers_struct() {
    insta::assert_snapshot!(gen_with_amqp("struct Sensor { long id; double temp; };"));
}

#[test]
fn snapshot_amqp_helpers_union() {
    insta::assert_snapshot!(gen_with_amqp(
        "union U switch (long) { case 1: long a; case 2: double b; };"
    ));
}

fn gen_typesupport(src: &str) -> String {
    // Only the TypeSupport files (zerodds-xcdr2-java-1.0 §4).
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    let mut combined = String::new();
    for f in &files {
        if !f.class_name.ends_with("TypeSupport") {
            continue;
        }
        combined.push_str(&format!(
            "// === FILE: {}/{}.java ===\n",
            f.package_path, f.class_name
        ));
        combined.push_str(&f.source);
        combined.push('\n');
    }
    combined
}

#[test]
fn snapshot_typesupport_final_struct() {
    // V-2 from zerodds-xcdr2-bindings-conformance-1.0 §6.
    insta::assert_snapshot!(gen_typesupport("@final struct Point { long x; long y; };"));
}

#[test]
fn snapshot_typesupport_keyed_struct() {
    // V-8 from the conformance spec.
    insta::assert_snapshot!(gen_typesupport(
        "@final struct Sensor { @key long id; double value; };"
    ));
}

#[test]
fn snapshot_typesupport_mutable_struct() {
    // V-10 from the conformance spec.
    insta::assert_snapshot!(gen_typesupport(
        "@mutable struct M { @id(1) long a; @id(2) string b; };"
    ));
}

// REGRESSION GATE: @mutable members WITHOUT explicit @id must take SEQUENTIAL
// 0-based ids (XTypes 1.3 §7.3.4.3 @autoid default; vendor-confirmed vs Cyclone).
// The Java backend previously started its auto-id counter at 1 (off-by-one),
// diverging from rust/cpp/c/python/ts/csharp + Cyclone on the @mutable wire.
#[test]
fn mutable_autoid_is_zero_based_sequential() {
    let ts = gen_typesupport("@mutable struct AutoId { long a; long b; };");
    assert!(
        ts.contains("writeEmHeader(0,"),
        "first auto-id @mutable member must be id 0 (sequential), got:\n{ts}"
    );
    assert!(
        ts.contains("writeEmHeader(1,"),
        "second auto-id @mutable member must be id 1 (sequential), got:\n{ts}"
    );
    // Guard against the old 1-based regression (would emit id 2 for the 2nd member).
    assert!(
        !ts.contains("writeEmHeader(2,"),
        "1-based auto-id regression: no member should be id 2 here"
    );
}

// ============================================================================
// KeyHash correctness (XTypes 1.3 §7.6.8) — java TypeSupport
// ============================================================================

/// Isolates the `keyHash(...)` method body so assertions about what the
/// KeyHash writes don't false-positive/false-negative against the general
/// `encode`/`decode` bodies (which legitimately reference the struct's full
/// member set) or another struct's own `keyHash`.
fn key_hash_body<'a>(ts: &'a str, class: &str) -> &'a str {
    let marker = format!("public byte[] keyHash({class} sample) {{");
    let start = ts
        .find(&marker)
        .unwrap_or_else(|| panic!("{class}TypeSupport.keyHash present:\n{ts}"));
    let body_start = start + marker.len() - 1;
    let body_end = ts[body_start..]
        .find("\n    }\n")
        .map(|i| body_start + i)
        .unwrap_or(ts.len());
    &ts[body_start..body_end]
}

#[test]
fn keyhash_nested_struct_key_expands_inner_key_members_only() {
    // A `@key` member whose type is itself a struct with its OWN partial
    // `@key` annotations must expand into ONLY those `@key` members, not the
    // struct's full member list (previously: the catch-all Scoped arm
    // delegated to `InnerTypeSupport.INSTANCE.encode(...)`, encoding ALL of
    // Inner's members into the KeyHash).
    let ts = gen_typesupport(
        "@final struct Inner { @key long x; long ignored; @key long y; };\n\
         @final struct Outer { @key Inner i; long z; };",
    );
    let kh = key_hash_body(&ts, "Outer");
    assert!(
        kh.contains("(sample.getI()).getX()") && kh.contains("(sample.getI()).getY()"),
        "nested-struct @key must expand into the inner struct's own @key members:\n{kh}"
    );
    assert!(
        !kh.contains("getIgnored"),
        "a non-@key inner member must NOT be included when the inner struct has explicit @key members:\n{kh}"
    );
    // The general (non-key) encoder is untouched: it still delegates to the
    // nested struct's own full encode.
    assert!(
        ts.contains("InnerTypeSupport.INSTANCE.encode(sample.getI()"),
        "general (non-key) encode must still delegate to the nested struct's full encode:\n{ts}"
    );
}

#[test]
fn keyhash_nested_struct_without_keys_uses_all_inner_members() {
    // XTypes 1.3 §7.6.8: an aggregate @key member with NO @key members of its
    // own is keyed in full (all its members).
    let ts = gen_typesupport(
        "@final struct Pair { long a; long b; };\n\
         @final struct Holder { @key Pair p; };",
    );
    let kh = key_hash_body(&ts, "Holder");
    assert!(
        kh.contains("(sample.getP()).getA()") && kh.contains("(sample.getP()).getB()"),
        "a keyless nested struct must key on ALL its members:\n{kh}"
    );
}

#[test]
fn keyhash_typedef_in_nested_struct_key_still_dealiases() {
    // Java's typedef arm already dealiases correctly outside a nested-key
    // struct (Bug J #65(4)); this asserts it still does so once the member
    // is reached through the new key-subset expansion path.
    let ts = gen_typesupport(
        "typedef long MyId;\n\
         @final struct Inner { @key MyId x; };\n\
         @final struct Outer { @key Inner i; };",
    );
    let kh = key_hash_body(&ts, "Outer");
    assert!(
        kh.contains("((sample.getI()).getX()).value()"),
        "a typedef-aliased @key member inside a nested @key struct must dealias:\n{kh}"
    );
}

#[test]
fn keyhash_typedef_of_struct_dealiases_and_expands_own_key_subset() {
    // A `@key` member whose declared type is a TYPEDEF OF A STRUCT (not the
    // struct directly) previously fell straight through the `if let
    // Some(ResolvedKind::Struct { .. })` check (which only matched a direct
    // struct reference, not `ResolvedKind::Typedef`) to the generic
    // `emit_typespec_encode` fallback — writing the WHOLE nested struct via
    // `InnerTypeSupport.INSTANCE.encode(...)` instead of just its own `@key`
    // subset. Unwrap the Java typedef wrapper's `.value()` and expand into
    // `Inner`'s own `@key` subset, same as a direct (non-aliased) nested-key
    // struct member.
    let ts = gen_typesupport(
        "@final struct Inner { @key long x; long ignored; };\n\
         typedef Inner InnerAlias;\n\
         @final struct Outer { @key InnerAlias i; long z; };",
    );
    let kh = key_hash_body(&ts, "Outer");
    assert!(
        kh.contains("((sample.getI()).value()).getX()"),
        "typedef-of-struct @key member must dealias AND expand into the struct's own @key subset:\n{kh}"
    );
    assert!(
        !kh.contains("getIgnored"),
        "a non-@key inner member must NOT be included when the aliased struct has explicit @key members:\n{kh}"
    );
    assert!(
        !kh.contains("InnerTypeSupport.INSTANCE.encode"),
        "must NOT delegate to the nested struct's full (non-key) encode:\n{kh}"
    );
    // The general (non-key) encoder is untouched: it still delegates to the
    // nested struct's own full encode via the typedef wrapper.
    assert!(
        ts.contains("InnerTypeSupport.INSTANCE.encode((sample.getI()).value()"),
        "general (non-key) encode must still delegate to the nested struct's full encode:\n{ts}"
    );
}

#[test]
fn keyhash_array_field_inside_nested_struct_key_is_a_loud_error() {
    // Medium fix: an array-typed field inside a nested-struct `@key` subset
    // is not a supported shape (matches every other backend's identical
    // rejection, e.g. `idl-rust`'s `emit_key_field_write`, `idl-cpp`'s
    // `emit_key_value_write`) — codegen must reject it with a loud
    // `UnsupportedConstruct`, not silently DHEADER-frame a per-element
    // encode of the array (which `emit_key_declarator_encode` would
    // otherwise do unchanged, mixing DHEADER framing into a KeyHolder that
    // must always be the FLAT concatenation of key bytes — XTypes 1.3
    // §7.6.8).
    let idl = "@final struct Inner { @key long a[3]; };\n\
               @final struct Outer { @key Inner i; };";
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse must succeed");
    let err = generate_java_files(&ast, &JavaGenOptions::default())
        .expect_err("array field inside a nested-struct @key must be a codegen error");
    let msg = err.to_string();
    assert!(
        msg.contains("array @key field inside a nested-struct key"),
        "{msg}"
    );
}

#[test]
fn keyhash_member_id_order_not_declaration_order() {
    // XTypes 1.3 §7.6.8.3.1.b: KeyHolder members in ascending member-id
    // order. Declaration order here is a, b; member-id order (via @id) is
    // b, a.
    let ts = gen_typesupport("@final struct K { @id(1) @key octet a; @id(0) @key long b; };");
    let kh = key_hash_body(&ts, "K");
    let pos_a = kh.find("sample.getA()").expect("a written");
    let pos_b = kh.find("sample.getB()").expect("b written");
    assert!(
        pos_b < pos_a,
        "member-id 0 (b) must be written before member-id 1 (a):\n{kh}"
    );
}
