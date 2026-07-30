// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free invariants for the Ada backend: pure string checks over the
//! generated `.ads`/`.adb` that must hold WITHOUT a GNAT toolchain on PATH.
//!
//! These guard the construct-fix campaign (idl-construct-fix-campaign.md) for
//! `idl-ada`: struct inheritance (F10), `const` (F5), empty struct → `null
//! record` (F15), non-integer union discriminators (F11/F12/F13), `@mutable`
//! union framing (F14), interface-nested types (F39), and case-insensitive
//! component de-duplication (F41). They also assert the generic non-compile
//! patterns (no bare `TRUE`/`FALSE`, no `L"..."`, no component-less `record ...
//! end record`, no unescaped Ada reserved word as an identifier, no duplicated
//! top-level type symbol) never appear.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ada::{AdaGenOptions, generate_ada_module};

/// Generates a module, asserting the spec parses and emits successfully.
fn emit(src: &str) -> zerodds_idl_ada::AdaModule {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ada_module(&ast, &AdaGenOptions::default()).expect("gen")
}

/// Generates, returning `Err` unchanged (for loud-reject assertions).
fn try_gen(src: &str) -> zerodds_idl_ada::Result<zerodds_idl_ada::AdaModule> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ada_module(&ast, &AdaGenOptions::default())
}

fn all(m: &zerodds_idl_ada::AdaModule) -> String {
    format!("{}\n{}", m.spec, m.body)
}

// ---- F10: struct inheritance ------------------------------------------------

#[test]
fn inherited_members_appear_base_first() {
    let m = emit("@final struct Base { long a; }; @final struct Derived : Base { long b; };");
    // The derived record carries the base member first, then its own.
    let rec = m
        .spec
        .split("type Derived is record")
        .nth(1)
        .expect("Derived record");
    let a = rec.find("a : Integer_32;").expect("inherited `a`");
    let b = rec.find("b : Integer_32;").expect("own `b`");
    assert!(
        a < b,
        "base member must precede derived member:\n{}",
        m.spec
    );
    // And both are marshaled.
    assert!(all(&m).contains("Unsigned_32'Mod (V.a)"), "{}", m.body);
    assert!(all(&m).contains("Unsigned_32'Mod (V.b)"), "{}", m.body);
}

#[test]
fn multi_level_inheritance_flattens_whole_chain() {
    let m = emit(
        "@final struct A { long a; }; @final struct B : A { long b; }; \
         @final struct C : B { long c; };",
    );
    let rec = m.spec.split("type C is record").nth(1).expect("C record");
    for f in ["a : Integer_32;", "b : Integer_32;", "c : Integer_32;"] {
        assert!(rec.contains(f), "missing {f} in C:\n{}", m.spec);
    }
}

// ---- F5: const declarations -------------------------------------------------

#[test]
fn const_declarations_are_emitted_for_every_type() {
    let m = emit(
        "const long MAX = 10; const octet O = 3; const float PI = 3.14; \
         const double D = 2; const boolean FLAG = TRUE; const char C = 'A'; \
         const string NAME = \"hi\";",
    );
    for expect in [
        "MAX : constant Integer_32 := 10;",
        "O : constant Unsigned_8 := 3;",
        "PI : constant IEEE_Float_32 := 3.14;",
        "D : constant IEEE_Float_64 := 2.0;",
        "FLAG : constant Boolean := True;",
        "C : constant Character := Character'Val (65);",
        "NAME : constant String := \"hi\";",
    ] {
        assert!(m.spec.contains(expect), "missing `{expect}`:\n{}", m.spec);
    }
}

#[test]
fn const_expression_folds() {
    let m = emit("const long SHIFTED = 1 << 4; const long ORED = 8 | 1;");
    assert!(
        m.spec.contains("SHIFTED : constant Integer_32 := 16;"),
        "{}",
        m.spec
    );
    assert!(
        m.spec.contains("ORED : constant Integer_32 := 9;"),
        "{}",
        m.spec
    );
}

#[test]
fn enum_typed_const_uses_enumerator() {
    let m = emit("enum Color { Red, Green, Blue }; const Color FAV = Green;");
    assert!(
        m.spec.contains("FAV : constant Color := Green;"),
        "{}",
        m.spec
    );
}

// ---- F15: empty struct → null record ---------------------------------------

#[test]
fn empty_struct_emits_null_record() {
    let m = emit("@final struct Empty {};");
    assert!(m.spec.contains("type Empty is null record;"), "{}", m.spec);
    // A component-less `record ... end record;` is a GNAT syntax error.
    assert!(
        !m.spec.contains("is record\n   end record;"),
        "component-less record leaked:\n{}",
        m.spec
    );
}

// ---- F11/F12/F13: non-integer union discriminators -------------------------

#[test]
fn enum_discriminated_union_uses_enumerator_labels() {
    let m = emit(
        "enum Color { Red, Green, Blue }; \
         union U switch (Color) { case Red: long x; case Green: double y; default: octet z; };",
    );
    assert!(m.spec.contains("disc : Color;"), "{}", m.spec);
    // The union's `case V.disc` dispatch uses enumerator labels, not integers.
    let dispatch = m
        .body
        .split("case V.disc is")
        .nth(1)
        .expect("union dispatch");
    assert!(dispatch.contains("when Red =>"), "{}", m.body);
    assert!(dispatch.contains("when Green =>"), "{}", m.body);
    let first_arm = dispatch.split("end case;").next().unwrap_or(dispatch);
    assert!(
        !first_arm.contains("when 0 =>"),
        "integer label leaked into enum dispatch:\n{}",
        m.body
    );
}

#[test]
fn char_discriminated_union_uses_val_labels() {
    let m = emit("union CU switch (char) { case 'a': long x; case 'b': long y; };");
    assert!(m.spec.contains("disc : Character;"), "{}", m.spec);
    assert!(m.body.contains("when Character'Val (97) =>"), "{}", m.body);
    assert!(m.body.contains("when Character'Val (98) =>"), "{}", m.body);
}

#[test]
fn bool_discriminated_union_uses_true_false_labels() {
    let m = emit("union BU switch (boolean) { case TRUE: long x; default: long y; };");
    assert!(m.spec.contains("disc : Boolean;"), "{}", m.spec);
    assert!(m.body.contains("when True =>"), "{}", m.body);
    // No bare IDL `TRUE`/`FALSE` keyword.
    assert!(!all(&m).contains("TRUE"), "bare TRUE leaked:\n{}", all(&m));
    assert!(
        !all(&m).contains("FALSE"),
        "bare FALSE leaked:\n{}",
        all(&m)
    );
}

// ---- F14: @mutable union framing -------------------------------------------

#[test]
fn mutable_union_emits_emheader_framing() {
    let m = emit("@mutable union MU switch (long) { case 1: long x; case 2: double y; };");
    // Discriminator member id 0, branches id 1/2, all LC4-framed (like the
    // @mutable struct path).
    assert!(
        m.body.contains("16#40000000#"),
        "disc EMHEADER:\n{}",
        m.body
    );
    assert!(
        m.body.contains("16#40000001#"),
        "branch1 EMHEADER:\n{}",
        m.body
    );
    assert!(
        m.body.contains("16#40000002#"),
        "branch2 EMHEADER:\n{}",
        m.body
    );
    // Decode skips DHEADER + the disc EMHEADER/NEXTINT then per-branch framing.
    assert!(
        m.body.matches("Skip_U32 (Data, Pos, Endian);").count() >= 3,
        "mutable union decode must skip DHEADER + EMHEADER words:\n{}",
        m.body
    );
}

// ---- F39: interface-nested types -------------------------------------------

#[test]
fn interface_nested_type_and_const_are_emitted() {
    let m = emit("interface Svc { struct Req { long id; }; const long V = 5; };");
    assert!(m.spec.contains("type Svc_sReq is record"), "{}", m.spec);
    assert!(m.spec.contains("id : Integer_32;"), "{}", m.spec);
    assert!(
        m.spec.contains("Svc_sV : constant Integer_32 := 5;"),
        "{}",
        m.spec
    );
}

// ---- F41: case-insensitive component de-duplication ------------------------

#[test]
fn case_colliding_struct_members_are_disambiguated() {
    let m = emit("@final struct Ci { long value; long Value; };");
    let rec = m.spec.split("type Ci is record").nth(1).expect("Ci record");
    // Ada is case-insensitive; the two must not collide as identical components.
    assert!(rec.contains("value : Integer_32;"), "{}", m.spec);
    assert!(rec.contains("Value_U : Integer_32;"), "{}", m.spec);
}

#[test]
fn union_branch_named_like_disc_is_disambiguated() {
    // A branch field spelled `Disc` collides (case-insensitively) with the
    // synthetic `disc` discriminator component.
    let m = emit("union DU switch (long) { case 1: long Disc; };");
    assert!(m.spec.contains("disc : Integer_32;"), "{}", m.spec);
    assert!(m.spec.contains("Disc_U : Integer_32;"), "{}", m.spec);
}

// ---- Reserved-word escaping (no unescaped Ada keyword as an identifier) -----

#[test]
fn ada_reserved_words_as_idl_identifiers_are_escaped() {
    // A struct and members named after Ada reserved words must escape to the
    // `_Id` form (Ada has no stropping; see keywords.rs).
    let m = emit("@final struct Rec { long record; long type; long begin; };");
    assert!(m.spec.contains("record_Id : Integer_32;"), "{}", m.spec);
    assert!(m.spec.contains("type_Id : Integer_32;"), "{}", m.spec);
    assert!(m.spec.contains("begin_Id : Integer_32;"), "{}", m.spec);
    // The escaped form is not itself a bare reserved-word component.
    for kw in ["record", "type", "begin"] {
        assert!(
            !m.spec.contains(&format!("      {kw} : Integer_32;")),
            "unescaped `{kw}` component leaked:\n{}",
            m.spec
        );
    }
}

// ---- Generic non-compile patterns ------------------------------------------

#[test]
fn no_forbidden_source_patterns() {
    let m = emit(
        "enum Color { Red, Green }; \
         const boolean B = FALSE; \
         @final struct Base { long a; }; \
         @final struct Derived : Base { long b; wstring w; }; \
         union U switch (Color) { case Red: long x; default: long y; };",
    );
    let s = all(&m);
    // C/C++ macro-ish or wide-literal artifacts must never appear in Ada.
    assert!(!s.contains("L\""), "C wide-string prefix leaked:\n{s}");
    assert!(!s.contains("TRUE"), "bare TRUE leaked");
    assert!(!s.contains("FALSE"), "bare FALSE leaked");
    // No component-less record.
    assert!(
        !s.contains("is record\n   end record;"),
        "empty record leaked"
    );
}

#[test]
fn no_duplicate_top_level_type_symbol() {
    let m = emit(
        "module a { @final struct R { long v; }; }; \
         module b { @final struct R { long w; }; }; \
         interface S { struct R { long z; }; };",
    );
    // Collect every `type <Name> is` symbol and assert uniqueness.
    let mut names: Vec<&str> = Vec::new();
    for line in m.spec.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("type ") {
            if let Some(name) = rest.split_whitespace().next() {
                if rest.contains(" is ") {
                    names.push(name);
                }
            }
        }
    }
    let mut sorted = names.clone();
    sorted.sort_unstable();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(
        before,
        sorted.len(),
        "duplicate top-level type symbol among {names:?}"
    );
}

// ---- Loud-reject invariants (no silent drop / no wrong wire) ---------------

#[test]
fn nested_sequence_of_sequence_is_loudly_rejected() {
    // A `sequence<sequence<T>>` element is not yet supported; it must be a loud
    // `Err`, never a silently wrong wire form.
    let r = try_gen("@final struct N { sequence<sequence<long>> s; };");
    assert!(r.is_err(), "nested sequence must be rejected loudly");
}
