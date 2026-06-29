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
fn struct_with_union_member_routes_through_union_codec() {
    // TS-cluster #1: a union now has an XCDR2 codec, so a struct containing a
    // union member must emit a TypeSupport that routes the member through the
    // union's `TypeSupport.encodeInto`/`decodeFrom` (shared CDR stream).
    let ts = gen_ts(
        "union Choice switch (long) { case 1: long a; case 2: double b; }; \
         struct Holder { Choice c; long n; };",
    );
    assert!(
        ts.contains("export interface Holder"),
        "data type must emit"
    );
    assert!(
        ts.contains("export const ChoiceTypeSupport"),
        "union must get its own XCDR2 TypeSupport:\n{ts}"
    );
    assert!(
        ts.contains("export const HolderTypeSupport"),
        "struct with a union member must NOT be gated anymore:\n{ts}"
    );
    assert!(
        ts.contains("ChoiceTypeSupport.encodeInto(w, s.c)"),
        "union member must encode via the union codec:\n{ts}"
    );
    assert!(
        ts.contains("ChoiceTypeSupport.decodeFrom(r)"),
        "union member must decode via the union codec:\n{ts}"
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
fn nested_and_sibling_collections_use_unique_loop_vars() {
    // Edge-hardening regression: a `sequence<sequence<T>>` reused the loop var
    // `_e` (`for (const _e of _e)`) and two SIBLING collection members reused
    // the `_seqtok` token (a `const` re-declare that won't parse). Each
    // collection loop must now get a unique run-global temporary.
    let ts = gen_ts(
        "struct A { long x; }; \
         struct S { sequence<sequence<A>> grid; sequence<long> nums; map<string,A> m; };",
    );
    // No loop ever iterates its own iterator.
    assert!(
        !ts.contains("for (const _e of _e)"),
        "nested sequence must not reuse the loop var:\n{ts}"
    );
    // `_seqtok` is suffixed (e.g. `_seqtok0`), never the bare form that a
    // sibling member would re-declare in the same scope.
    assert!(
        !ts.contains("const _seqtok ="),
        "sequence tokens must be uniquely suffixed:\n{ts}"
    );
    // At least two distinct suffixed loop vars exist (grid outer + inner / map).
    assert!(
        ts.contains("for (const _e0 of") && ts.contains("for (const _e1 of"),
        "distinct collection loops must get distinct loop vars:\n{ts}"
    );
}

#[test]
fn bounded_collections_emit_capacity_guard() {
    // Edge-hardening regression: a bounded sequence/string/map over its bound
    // silently corrupted; encode must now emit a capacity guard that throws.
    let ts = gen_ts("struct B { sequence<long,2> bs; string<3> name; map<string,long,4> m; };");
    assert!(
        ts.contains("bs.length > 2") && ts.contains("throw new RangeError"),
        "bounded sequence must guard its capacity:\n{ts}"
    );
    assert!(
        ts.contains("].length > 3"),
        "bounded string must guard its capacity (by code points):\n{ts}"
    );
    assert!(
        ts.contains(".size > 4"),
        "bounded map must guard its capacity:\n{ts}"
    );
}

#[test]
fn enum_field_routes_through_ordinal_map() {
    // TS-cluster #3: an enum's TS surface type is a string-literal union, so the
    // codec must map through the emitted `<Enum>Ordinal` / `<Enum>FromOrdinal`
    // maps — a bare `as unknown as number` cast would write NaN over the wire.
    let ts = gen_ts("enum Color { RED, GREEN, BLUE }; struct S { Color c; };");
    assert!(
        ts.contains("w.writeInt32(ColorOrdinal[s.c])"),
        "enum field must encode via the ordinal map:\n{ts}"
    );
    assert!(
        ts.contains("ColorFromOrdinal.get(r.readInt32())"),
        "enum field must decode via the from-ordinal map:\n{ts}"
    );
    assert!(
        !ts.contains("s.c as unknown as number"),
        "enum field must NOT use the broken numeric cast:\n{ts}"
    );
}
