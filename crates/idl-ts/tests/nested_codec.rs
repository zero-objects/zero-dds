//! Regression tests for the XCDR2 nested-type codec dispatch.
//!
//! Before the type-kind resolver, every scoped reference was encoded as a
//! single `int32` (`writeInt32(x as unknown as number)`) — silently corrupting
//! nested structs/unions. These tests lock in that a nested struct/union routes
//! through its `TypeSupport.encodeInto`/`decodeFrom` (shared CDR stream,
//! alignment-correct), a typedef resolves to its aliased type, and an enum
//! still uses the int32 representation.

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;

fn gen_ts(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    zerodds_idl_ts::generate_ts_source(&ast).expect("gen")
}

#[test]
fn nested_struct_uses_nested_codec_not_int32() {
    let ts = gen_ts("struct Inner { long x; long y; }; struct Outer { Inner inner; long z; };");
    assert!(
        ts.contains("InnerTypeSupport.encodeInto(w, s.inner)"),
        "nested struct must encode via the nested codec:\n{ts}"
    );
    assert!(
        ts.contains("InnerTypeSupport.decodeFrom(r)"),
        "nested struct must decode via the nested codec:\n{ts}"
    );
    // The silent-corruption form must be gone for the nested field.
    assert!(
        !ts.contains("s.inner as unknown as number"),
        "nested struct must not be encoded as int32:\n{ts}"
    );
}

#[test]
fn struct_with_union_member_is_gated_not_broken() {
    // Unions have no XCDR2 codec in idl-ts, so a struct containing a union
    // member must be gated (data type emitted, TypeSupport skipped) — NOT a
    // reference to a non-existent UnionTypeSupport (which would not compile).
    let ts = gen_ts(
        "union Choice switch (long) { case 1: long a; case 2: double b; }; \
         struct Holder { Choice c; long n; };",
    );
    assert!(
        ts.contains("export interface Holder"),
        "data type must emit"
    );
    assert!(
        !ts.contains("ChoiceTypeSupport"),
        "must not reference a non-existent union TypeSupport:\n{ts}"
    );
    assert!(
        !ts.contains("export const HolderTypeSupport"),
        "struct with a union member must be gated (no TypeSupport):\n{ts}"
    );
}

#[test]
fn typedef_of_struct_resolves_to_nested_codec() {
    let ts = gen_ts(
        "struct Inner { long x; }; typedef Inner Alias; \
         struct Outer { Alias a; long n; };",
    );
    assert!(
        ts.contains("InnerTypeSupport.encodeInto(w, s.a)"),
        "typedef-of-struct must resolve to the nested codec:\n{ts}"
    );
}

#[test]
fn typedef_of_primitive_stays_primitive() {
    let ts = gen_ts("typedef long Counter; struct S { Counter c; };");
    assert!(
        ts.contains("w.writeInt32(s.c)"),
        "typedef-of-primitive must encode as the underlying primitive:\n{ts}"
    );
    assert!(
        !ts.contains("CounterTypeSupport"),
        "typedef alias must not get a TypeSupport call:\n{ts}"
    );
}

#[test]
fn enum_field_still_uses_int32() {
    let ts = gen_ts("enum Color { RED, GREEN, BLUE }; struct S { Color c; };");
    assert!(
        ts.contains("s.c as unknown as number"),
        "enum field keeps the int32 representation:\n{ts}"
    );
}
