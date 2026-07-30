//! Snapshot tests for the TypeScript codegen output.

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
use zerodds_idl_ts::{generate_ts_source, generate_ts_source_with_amqp};

fn gen_default(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ts_source(&ast).expect("gen")
}

fn gen_with_amqp(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ts_source_with_amqp(&ast).expect("gen")
}

#[test]
fn snapshot_simple_struct() {
    insta::assert_snapshot!(gen_default("struct Point { long x; long y; };"));
}

#[test]
fn enum_member_encodes_as_int32() {
    // idl-ts is a full IDL4 codegen (unlike the thin backends): an enum member
    // is a 32-bit signed integer on the wire (XTypes 1.3 §7.4.5.1), emitted as
    // `writeInt32` — byte-identical to the verified int32 path.
    let ts = gen_default(
        "enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT }; \
         @final struct S { Mode kind; uint32 tail; };",
    );
    assert!(ts.contains("writeInt32"), "{ts}");
}

#[test]
fn snapshot_struct_with_string_and_sequence() {
    insta::assert_snapshot!(gen_default(
        "struct Bag { string name; sequence<long> ids; };"
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
fn snapshot_amqp_helpers_struct() {
    insta::assert_snapshot!(gen_with_amqp("struct Sensor { long id; double temp; };"));
}

#[test]
fn snapshot_amqp_helpers_union() {
    insta::assert_snapshot!(gen_with_amqp(
        "union U switch (long) { case 1: long a; case 2: double b; };"
    ));
}

// ============================================================================
// KeyHash correctness (XTypes 1.3 §7.6.8) — TypeScript TypeSupport
// ============================================================================

/// Isolates a struct's `keyHash(...)` method body so assertions about what
/// the KeyHash writes don't false-positive/false-negative against the
/// general `encodeInto`/`decodeFrom` bodies (which legitimately reference
/// the struct's full member set) or another struct's own `keyHash`.
fn key_hash_body<'a>(ts: &'a str, class: &str) -> &'a str {
    let marker = format!("keyHash(s: {class}): Uint8Array {{");
    let start = ts
        .find(&marker)
        .unwrap_or_else(|| panic!("{class}TypeSupport.keyHash present:\n{ts}"));
    let body_start = start + marker.len();
    let body_end = ts[body_start..]
        .find("\n  }\n")
        .map(|i| body_start + i)
        .unwrap_or(ts.len());
    &ts[body_start..body_end]
}

#[test]
fn keyhash_nested_struct_key_expands_inner_key_members_only() {
    // A `@key` member whose type is itself a struct with its OWN partial
    // `@key` annotations must expand into ONLY those `@key` members, not the
    // struct's full member list (previously: the Scoped-struct arm delegated
    // to `InnerTypeSupport.encodeInto(w, expr)`, encoding ALL of Inner's
    // members into the KeyHash).
    let ts = gen_default(
        "@final struct Inner { @key long x; long ignored; @key long y; };\n\
         @final struct Outer { @key Inner i; long z; };",
    );
    let kh = key_hash_body(&ts, "Outer");
    assert!(
        kh.contains("s.i.x") && kh.contains("s.i.y"),
        "nested-struct @key must expand into the inner struct's own @key members:\n{kh}"
    );
    assert!(
        !kh.contains("s.i.ignored"),
        "a non-@key inner member must NOT be included when the inner struct has explicit @key members:\n{kh}"
    );
    // The general (non-key) encoder is untouched: it still delegates to the
    // nested struct's own full encode.
    assert!(
        ts.contains("InnerTypeSupport.encodeInto(w, s.i)"),
        "general (non-key) encode must still delegate to the nested struct's full encode:\n{ts}"
    );
}

#[test]
fn keyhash_nested_struct_without_keys_uses_all_inner_members() {
    // XTypes 1.3 §7.6.8: an aggregate @key member with NO @key members of its
    // own is keyed in full (all its members).
    let ts = gen_default(
        "@final struct Pair { long a; long b; };\n\
         @final struct Holder { @key Pair p; };",
    );
    let kh = key_hash_body(&ts, "Holder");
    assert!(
        kh.contains("s.p.a") && kh.contains("s.p.b"),
        "a keyless nested struct must key on ALL its members:\n{kh}"
    );
}

#[test]
fn keyhash_typedef_in_nested_struct_key_still_dealiases() {
    let ts = gen_default(
        "typedef long MyId;\n\
         @final struct Inner { @key MyId x; };\n\
         @final struct Outer { @key Inner i; };",
    );
    let kh = key_hash_body(&ts, "Outer");
    assert!(
        kh.contains("s.i.x"),
        "a typedef-aliased @key member inside a nested @key struct must be written:\n{kh}"
    );
}

#[test]
fn keyhash_typedef_of_struct_dealiases_and_expands_own_key_subset() {
    // A `@key` member whose declared type is a TYPEDEF OF A STRUCT (not the
    // struct directly) previously fell straight through the `if let Some
    // (ScopedKind::Struct { .. })` check (which only matched a direct struct
    // reference, not `ScopedKind::Typedef`) to the generic
    // `emit_typespec_encode` fallback — writing the WHOLE nested struct via
    // `InnerTypeSupport.encodeInto(w, s.i)` instead of just its own `@key`
    // subset. A TS typedef is a pure structural alias (no runtime wrapper),
    // so the fix just recurses on the aliased type with the SAME expr and
    // lands on the Struct arm's key-subset expansion.
    let ts = gen_default(
        "@final struct Inner { @key long x; long ignored; };\n\
         typedef Inner InnerAlias;\n\
         @final struct Outer { @key InnerAlias i; long z; };",
    );
    let kh = key_hash_body(&ts, "Outer");
    assert!(
        kh.contains("s.i.x"),
        "typedef-of-struct @key member must dealias AND expand into the struct's own @key subset:\n{kh}"
    );
    assert!(
        !kh.contains("s.i.ignored"),
        "a non-@key inner member must NOT be included when the aliased struct has explicit @key members:\n{kh}"
    );
    assert!(
        !kh.contains("InnerTypeSupport.encodeInto"),
        "must NOT delegate to the nested struct's full (non-key) encode:\n{kh}"
    );
    // The general (non-key) encoder is untouched: it still delegates to the
    // nested struct's own full encode through the typedef alias.
    assert!(
        ts.contains("InnerTypeSupport.encodeInto(w, s.i)"),
        "general (non-key) encode must still delegate to the nested struct's full encode:\n{ts}"
    );
}

#[test]
fn keyhash_array_field_inside_nested_struct_key_is_a_loud_error() {
    // Medium fix: an array-typed field inside a nested-struct `@key` subset
    // is not a supported shape (matches every other backend's identical
    // rejection, e.g. `idl-rust`'s `emit_key_field_write`, `idl-cpp`'s
    // `emit_key_value_write`) — codegen must reject it with a loud error,
    // not silently DHEADER-frame a per-element encode of the array (which
    // `emit_key_declarator_encode` would otherwise do unchanged, mixing
    // DHEADER framing into a KeyHolder that must always be the FLAT
    // concatenation of key bytes — XTypes 1.3 §7.6.8).
    let idl = "@final struct Inner { @key long a[3]; };\n\
               @final struct Outer { @key Inner i; };";
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse must succeed");
    let err = zerodds_idl_ts::generate_ts_source(&ast)
        .expect_err("array field inside a nested-struct @key must be a codegen error");
    let msg = err.to_string();
    assert!(
        msg.contains("array @key field") && msg.contains("nested-struct key"),
        "{msg}"
    );
}

#[test]
fn keyhash_member_id_order_not_declaration_order() {
    // XTypes 1.3 §7.6.8.3.1.b: KeyHolder members in ascending member-id
    // order. Declaration order here is a, b; member-id order (via @id) is
    // b, a.
    let ts = gen_default("@final struct K { @id(1) @key octet a; @id(0) @key long b; };");
    let kh = key_hash_body(&ts, "K");
    let pos_a = kh.find("s.a").expect("a written");
    let pos_b = kh.find("s.b").expect("b written");
    assert!(
        pos_b < pos_a,
        "member-id 0 (b) must be written before member-id 1 (a):\n{kh}"
    );
}
