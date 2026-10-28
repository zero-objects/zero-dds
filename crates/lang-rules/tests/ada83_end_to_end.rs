// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Ada 83, through every layer of the rebuild.
//!
//! `ada_end_to_end.rs` is the counterpart for the Ada 2012 column;
//! this file follows its shape — every corpus case renders or names
//! its refusal, and the generated text compiles under a real GNAT
//! where one is on the path. What is not attempted here is full
//! parity with that file's dozens of unit tests: this column's own
//! distinguishing decisions are covered below, and the corpus gate is
//! the test that proves breadth. `docs/OPEN-ITEMS.md`-style honesty
//! applies to the corpus gate's own threshold: it names what does not
//! yet render and does not round up.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stderr
)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;
#[path = "support/resolved.rs"]
mod resolved;

use std::process::Command;
use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::ada83::{FILE_NAME, ada83_file};
use zerodds_lang_rules::rule::ada83;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Ada 83 out.
fn generate(idl: &str) -> String {
    try_generate(idl, "test.idl").unwrap_or_else(|why| panic!("{why}"))
}

/// The same, naming the layer that refused instead of panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    let ast = resolved::ast(idl, origin)?;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &ada83::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = ada83_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::ada83::file(&file))
}

// -- the mapping -----------------------------------------------------

#[test]
fn a_struct_of_primitives_becomes_an_ada_record() {
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(generated.contains("type Pose is record"), "{generated}");
    assert!(generated.contains("X : Long_Integer;"), "{generated}");
    assert!(generated.contains("Y : Long_Float;"), "{generated}");
    assert!(generated.contains("Active : Boolean;"), "{generated}");
    assert!(generated.contains("end record;"), "{generated}");
}

#[test]
fn every_integer_width_becomes_long_integer() {
    // Unlike the Ada 2012 column, one type answers for every
    // integer-holding IDL primitive: see the module documentation of
    // `zerodds_lang_rules::adapter::ada83`. `Interfaces` never reaches
    // the output at all.
    let generated = generate(
        "struct P {
           octet b; char c; wchar d; int8 e; uint8 f; short g; unsigned short h;
           long i; unsigned long j; long long k; unsigned long long l;
         };",
    );
    for field in ["B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L"] {
        assert!(
            generated.contains(&format!("{field} : Long_Integer;")),
            "missing {field} : Long_Integer;\n{generated}"
        );
    }
    // Not the bare word: the file's own header comment explains why
    // there is no `Interfaces` in prose, which would make a plain
    // substring check pass on its own explanation. What must not
    // appear is a qualified use of it.
    assert!(
        !generated.contains("Interfaces."),
        "Interfaces reached the output — Annex B is Ada 95\n{generated}"
    );
    assert!(!generated.contains("with Interfaces;"), "{generated}");
}

#[test]
fn float_and_double_take_the_two_ada_83_predefined_types() {
    let generated = generate("struct P { float a; double b; };");
    assert!(generated.contains("A : Float;"), "{generated}");
    assert!(generated.contains("B : Long_Float;"), "{generated}");
    // Never `Interfaces.IEEE_Float_32/64`: RM83 3.5.7 already
    // predefines both without Annex B.
    assert!(!generated.contains("IEEE_Float"), "{generated}");
}

#[test]
fn a_long_double_is_text_and_never_an_implementation_defined_float() {
    let generated = generate("struct S { long double big; };");
    assert!(generated.contains("Big : Zd_Wire83.Text;"), "{generated}");
    assert!(!generated.contains("Long_Long_Float"), "{generated}");
}

#[test]
fn a_string_is_zd_wire83_text_and_never_unbounded_string() {
    // No `Ada.Strings.Unbounded` here at all: it is a child unit of
    // `Ada`, and Ada 83's string handling is not organised that way.
    let generated = generate("struct Bag { string name; wstring wide; };");
    assert!(generated.contains("Name : Zd_Wire83.Text;"), "{generated}");
    assert!(generated.contains("Wide : Zd_Wire83.Text;"), "{generated}");
    // Same reasoning as the `Interfaces` check above: the file's own
    // header comment names `Ada.Strings.Unbounded` to say it is not
    // used, so the check is for the actual type name.
    assert!(
        !generated.contains("Unbounded_String"),
        "Ada.Strings.Unbounded reached the output\n{generated}"
    );
    assert!(!generated.contains(": String;"), "{generated}");
}

#[test]
fn no_use_type_clause_is_ever_written() {
    // The one thing `adapter::ada` needs `operator_types`/`context_clause`
    // for and this column does not: every scalar here is `Standard`'s
    // or `Zd_Wire83`'s, and neither needs a `use type` — see the module
    // documentation of `zerodds_lang_cst::ada83`.
    let generated = generate(
        "union U switch (short) { case -13: long a; };
         const long N = -5;
         struct S { long x; };",
    );
    assert!(!generated.contains("use type"), "{generated}");
    assert!(generated.contains("when -13 =>"), "{generated}");
    assert!(generated.contains("N : constant"), "{generated}");
}

#[test]
fn an_optional_member_becomes_a_discriminated_record() {
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(
        generated.contains("type Zd_Opt_Octet (Present : Boolean := False) is record"),
        "{generated}"
    );
    assert!(generated.contains("case Present is"), "{generated}");
    assert!(generated.contains("when True =>"), "{generated}");
    assert!(generated.contains("Value : Long_Integer;"), "{generated}");
    assert!(generated.contains("when False =>"), "{generated}");
    assert!(generated.contains("B : Zd_Opt_Octet;"), "{generated}");
    assert!(generated.contains("C : Long_Integer;"), "{generated}");
}

#[test]
fn a_member_named_after_a_reserved_word_escapes_with_a_letter() {
    let generated = generate("struct S { long type; long range; long access; };");
    assert!(
        generated.contains("Type_Idl : Long_Integer;"),
        "{generated}"
    );
    assert!(
        generated.contains("Range_Idl : Long_Integer;"),
        "{generated}"
    );
    assert!(
        generated.contains("Access_Idl : Long_Integer;"),
        "{generated}"
    );
}

#[test]
fn ada_95_reserved_words_are_not_escaped_here() {
    // `tagged`, `interface`, `protected` and their nine siblings are
    // not RM83 2.9 reserved words: a member named one needs no escape
    // under `-gnat83`, unlike under the Ada 2012 column.
    let generated = generate("struct S { long tagged_field; };");
    // The member is `tagged_field`, not one of the words itself, so
    // this checks the reserved list rather than the escape directly —
    // see `zerodds_lang_cst::ada83::RESERVED` for the full list and
    // `crates/lang-cst/src/ada83/mod.rs`'s own unit test.
    assert!(
        generated.contains("Tagged_Field : Long_Integer;"),
        "{generated}"
    );
}

#[test]
fn an_enumerator_is_a_named_constant_and_never_an_enumeration_literal() {
    // The single most important thing this column does differently
    // from `adapter::ada`: no enumeration type, no representation
    // clause, no `'Enum_Val`/`'Enum_Rep` — see the module
    // documentation of `zerodds_lang_rules::adapter::ada83`.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(
        generated.contains("IDLE : constant Long_Integer := 0;"),
        "{generated}"
    );
    assert!(
        generated.contains("ACTIVE : constant Long_Integer := 4;"),
        "{generated}"
    );
    assert!(
        generated.contains("FAULT : constant Long_Integer := 5;"),
        "{generated}"
    );
    assert!(
        !generated.contains("type Mode is ("),
        "an enumeration type reached the output\n{generated}"
    );
    assert!(
        !generated.contains("for Mode use"),
        "a representation clause reached the output\n{generated}"
    );
}

#[test]
fn a_sequence_becomes_an_access_to_an_unconstrained_array() {
    let generated = generate("struct Bag { sequence<long> ids; };");
    assert!(
        generated.contains("type Zd_Seq_Long_Array is array (Natural range <>) of Long_Integer;"),
        "{generated}"
    );
    assert!(
        generated.contains("type Zd_Seq_Long is access Zd_Seq_Long_Array;"),
        "{generated}"
    );
    assert!(generated.contains("Ids : Zd_Seq_Long;"), "{generated}");
}

#[test]
fn an_array_declares_one_type_per_dimension_outermost_first() {
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(
        generated.contains("type Zd_Arr_Long_3 is array (0 .. 2) of Long_Integer;"),
        "{generated}"
    );
    assert!(
        generated.contains("type Zd_Arr_Long_2_3 is array (0 .. 1) of Zd_Arr_Long_3;"),
        "{generated}"
    );
    let inner = generated
        .find("type Zd_Arr_Long_3 ")
        .expect("the inner type");
    let outer = generated
        .find("type Zd_Arr_Long_2_3 ")
        .expect("the outer type");
    assert!(inner < outer, "the outer type was declared first");
}

#[test]
fn a_map_becomes_a_sequence_of_entries() {
    let generated = generate("struct S { map<long, string> table; };");
    assert!(
        generated.contains("type Zd_Map_Entry_Long_String is record"),
        "{generated}"
    );
    assert!(generated.contains("Key : Long_Integer;"), "{generated}");
    assert!(generated.contains("Value : Zd_Wire83.Text;"), "{generated}");
    assert!(
        generated.contains("Table : Zd_Map_Long_String;"),
        "{generated}"
    );
}

#[test]
fn a_typedef_becomes_a_subtype() {
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(
        generated.contains("subtype Distance is Long_Integer;"),
        "{generated}"
    );
    assert!(!generated.contains("type Distance is new"), "{generated}");
}

#[test]
fn a_union_discriminant_uses_the_enumerators_value_not_its_name() {
    // `adapter::ada`'s union writes `case D is when IDLE => …`; this
    // column writes the number, because there is no enumeration
    // literal `IDLE` to name.
    let generated = generate(
        "enum Mode { IDLE, ACTIVE, FAULT };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(
        generated.contains("type Payload (D : Long_Integer := 0) is record"),
        "{generated}"
    );
    assert!(generated.contains("case D is"), "{generated}");
    assert!(generated.contains("when 0 =>"), "{generated}");
    assert!(generated.contains("Idle : Long_Integer;"), "{generated}");
    assert!(generated.contains("when 1 =>"), "{generated}");
    assert!(generated.contains("Active : Long_Float;"), "{generated}");
    assert!(generated.contains("when others =>"), "{generated}");
}

#[test]
fn a_bitmask_becomes_long_integer_constants() {
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(
        generated.contains("READY : constant Long_Integer := 1;"),
        "{generated}"
    );
    assert!(
        generated.contains("MOVING : constant Long_Integer := 16;"),
        "{generated}"
    );
    assert!(
        !generated.contains("is new Long_Integer"),
        "a derived type reached the output; Ada 83 has no modular type to derive from\n{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_record_of_long_integer_fields() {
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("type B is record"), "{generated}");
    assert!(generated.contains("Low : Long_Integer;"), "{generated}");
    assert!(generated.contains("High : Long_Integer;"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_record_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("type V is record"), "{generated}");
    assert!(generated.contains("Counter : Long_Integer;"), "{generated}");
    assert!(
        generated.contains("Secret : Zd_Wire83.Text;"),
        "{generated}"
    );
}

#[test]
fn a_constant_becomes_a_constant_object() {
    let generated = generate("const octet LIMIT = 8;");
    assert!(
        generated.contains("LIMIT : constant Long_Integer := 8;"),
        "{generated}"
    );
}

#[test]
fn an_exception_becomes_a_record_and_not_an_ada_exception() {
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(generated.contains("type Failure is record"), "{generated}");
    assert!(generated.contains("Code : Long_Integer;"), "{generated}");
    assert!(
        generated.contains("Reason : Zd_Wire83.Text;"),
        "{generated}"
    );
}

#[test]
fn an_interface_is_refused_because_ada_83_has_no_form_for_one() {
    // The clean no-form this column reports at the top of
    // `ada83_items`: RM83 has no tagged type, no interface type, no
    // `abstract`. Refused before any per-construct rule is consulted.
    let error = try_generate("interface I { void go(); };", "interface.idl")
        .expect_err("an IDL interface was not refused");
    assert!(error.contains("interface"), "{error}");
    assert!(error.contains("Ada 83 has no interface type"), "{error}");
}

#[test]
fn a_module_flattens_onto_the_name() {
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(
        generated.contains("type Common_Util_Location is record"),
        "{generated}"
    );
}

#[test]
fn a_native_declaration_becomes_a_derived_address_type() {
    let generated = generate("native Handle;");
    assert!(
        generated.contains("type Handle is new System.Address;"),
        "{generated}"
    );
    assert!(generated.contains("with System;"), "{generated}");
}

#[test]
fn a_self_referential_type_through_a_sequence_renders_with_the_rm83_3_10_1_cut() {
    // RM83 3.10.1's incomplete-type cut, ported from `adapter::ada`:
    // `Node` is declared incomplete, its sequence's array is deferred
    // behind it, and the completion follows. See the module
    // documentation's point 4.
    let generated = generate("struct Node { long value; sequence<Node> children; };");
    assert!(generated.contains("type Node;"), "{generated}");
    assert!(generated.contains("type Zd_Seq_Node_Array;"), "{generated}");
    assert!(
        generated.contains("type Zd_Seq_Node is access Zd_Seq_Node_Array;"),
        "{generated}"
    );
    assert!(generated.contains("type Node is record"), "{generated}");
    assert!(
        generated.contains("type Zd_Seq_Node_Array is array (Natural range <>) of Node;"),
        "{generated}"
    );
    let incomplete = generated.find("type Node;\n").expect("the incomplete type");
    let complete = generated
        .find("type Node is record")
        .expect("the completion");
    assert!(incomplete < complete, "{generated}");
}

#[test]
fn a_cycle_an_array_closes_is_refused_citing_both_clauses() {
    // The one shape the cut still refuses: RM83 3.10.1 admits an
    // incomplete type only as an access type's designated subtype,
    // never as an array element, in every Ada revision — and this is
    // also IDL 4.2 §7.4.1.4.4.4.4's own refusal, which
    // `wireprobe-zerodds-ada83-gen` answers before this materialiser
    // even runs (see the module documentation of
    // `zerodds_lang_rules::adapter::ada83`). This test reaches the
    // materialiser directly, so it sees the fallback answer.
    let error = try_generate(
        "struct Node { long value; Node pair[2]; };",
        "recursive-array.idl",
    )
    .expect_err("an array closing a cycle was not refused");
    assert!(error.contains("7.4.1.4.4.4.4"), "{error}");
    assert!(error.contains("3.10.1"), "{error}");
}

#[test]
fn a_self_referential_type_through_an_optional_external_member_becomes_a_reference() {
    let generated =
        generate("struct Node; struct Node { long value; @optional @external Node next; };");
    assert!(generated.contains("type Node;"), "{generated}");
    assert!(
        generated.contains("type Zd_Ref_Node is access Node;"),
        "{generated}"
    );
    assert!(generated.contains("Next : Zd_Ref_Node;"), "{generated}");
    assert!(!generated.contains("Zd_Opt_Node"), "{generated}");
}

#[test]
fn mutual_recursion_renders_with_one_incomplete_declaration() {
    let generated = generate(
        "struct B; struct A { long value; sequence<B> bs; };
         struct B { long weight; sequence<A> as; };",
    );
    let cut_a = generated.contains("type A;\n");
    let cut_b = generated.contains("type B;\n");
    assert!(cut_a != cut_b, "expected exactly one cut\n{generated}");
    assert!(generated.contains("type A is record"), "{generated}");
    assert!(generated.contains("type B is record"), "{generated}");
}

#[test]
fn recursion_through_a_union_branch_renders() {
    let generated = generate(
        "struct Node;
         union Branch switch (long) { case 0: long leaf; default: sequence<Node> nodes; };
         struct Node { long value; Branch link; };",
    );
    assert!(generated.contains("type Node;"), "{generated}");
    assert!(
        generated.contains("type Branch (D : Long_Integer := 0) is record"),
        "{generated}"
    );
    assert!(generated.contains("type Node is record"), "{generated}");
}

/// The codec generator, for the shapes whose defect is only visible in
/// the routines and not in the declarations.
fn codec(idl: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    zerodds_lang_rules::generate::source(
        &ast,
        zerodds_lang_cst::Language::Ada83,
        zerodds_ast_layout::LayoutKey::data(
            zerodds_ast_layout::Encoding::PlainCdr2,
            zerodds_ast_layout::Endianness::Little,
        ),
    )
    .expect("a codec")
}

/// A `wstring` is not a `string` with a different name.
///
/// Both are held in `Zd_Wire83.Text` — see the module documentation of
/// `adapter::ada83` — and `type_tag` used to answer `Str` for both, so
/// the two members collapsed onto one hoisted routine and whichever
/// came first wrote them both. `Write_String` is UTF-8 with the NUL in
/// the length; `Write_Wstring_Utf8` is UTF-16 code units without one
/// (DDS-XTypes 1.3 7.4.1.1.2). The wire form was the string's, for a
/// member the IDL declared `wstring`.
#[test]
fn a_wstring_is_not_written_by_the_string_routine() {
    let generated = codec("struct S { string a; wstring b; };");
    assert!(
        generated.contains("Zd_Wire83.Write_String (w, v);"),
        "{generated}"
    );
    assert!(
        generated.contains("Zd_Wire83.Write_Wstring_Utf8 (w, v,"),
        "the wstring member is written by the string routine\n{generated}"
    );
    assert!(
        generated.contains("procedure zd_enc_Wstring "),
        "{generated}"
    );
}

/// A bound belongs to the routine the collection is hoisted into.
///
/// The unbounded member is declared first on purpose: the hoisted
/// routine is keyed by the tag, so with the bound left out of it the
/// bounded member was decoded by the unbounded member's routine and
/// its length check disappeared entirely.
#[test]
fn a_sequence_bound_survives_an_unbounded_sequence_of_the_same_element() {
    let generated = codec("struct S { sequence<long> open; sequence<long, 4> capped; };");
    assert!(
        generated.contains("zd_n > 4"),
        "the bound of `sequence<long, 4>` is nowhere in the codec\n{generated}"
    );
    assert!(
        generated.contains("procedure zd_enc_Seq_Of_Long ")
            && generated.contains("procedure zd_enc_Seq_Of_4_Long "),
        "{generated}"
    );
}

/// The same for a `string`, whose bound is carried the same way.
#[test]
fn a_string_bound_survives_an_unbounded_string_beside_it() {
    let generated = codec("struct S { string open; string<4> capped; };");
    assert!(
        generated.contains("procedure zd_enc_String ")
            && generated.contains("procedure zd_enc_String_4 "),
        "the bounded string shares the unbounded one's routine\n{generated}"
    );
}

/// A boxed value whose target does not resolve is refused by name.
///
/// It used to answer `Ok(())`, which declares nothing and says so
/// nowhere: `valuetype VB Missing;` produced an empty package. Every
/// other unresolved reference in `declare_type` — a sequence's
/// element, a map's key, a typedef's target, a member's type — is
/// `Unevaluated`, and this one is now too.
#[test]
fn a_boxed_value_with_an_unresolved_target_is_refused() {
    let parsed = zerodds_idl_parse::parse("valuetype VB Missing;");
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    let mut ast: Ast = lowered.ast;
    let _ = ast.resolve_all_by_name();
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &ada83::catalogue()).expect("apply");
    let error = ada83_file(&ast, &applied).expect_err("a refusal");
    let text = error.to_string();
    assert!(
        text.contains("boxed value target"),
        "unexpected refusal: {text}"
    );
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_generated_character_is_printable_ascii() {
    // GNAT's default source representation is Latin-1, not UTF-8;
    // exactly the property `adapter::ada`'s own corpus gate checks.
    let generated = generate(
        "module m { enum Mode { IDLE, @value(4) ACTIVE };
           union U switch (Mode) { case IDLE: long a; };
           struct S { @optional string text; sequence<long> q; long g[2]; long type; };
           @bit_bound(8) bitmask F { BIT };
           exception E { long code; };
           native H; const string TAG = \"abc\"; };",
    );
    for (index, ch) in generated.char_indices() {
        assert!(
            ch == '\n' || (' '..='~').contains(&ch),
            "byte {index} is {ch:?}, which GNAT reads as Latin-1\n{generated}"
        );
    }
}

// -- the ascending-key map order ---------------------------------------

/// The IDL through the front end, for the codec walk.
fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");
    ast
}

/// The write walk under `--map-entries ascending-key`, rendered.
fn codec_with_ascending_keys(idl: &str, type_name: &str) -> String {
    let ast = model(idl);
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == type_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let mut key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    key.map_entries = zerodds_ast_layout::MapEntryOrder::AscendingKey;
    let decls = zerodds_lang_rules::emit::encode::codec_from::<
        zerodds_lang_rules::emit::ada83::Ada83,
    >(&ast, root, key, Default::default())
    .expect("the write walk reaches the end");
    decls
        .iter()
        .map(zerodds_lang_render::ada83::decl)
        .collect::<Vec<_>>()
        .join("\n")
}

/// An `unsigned long long` map key is compared as an unsigned value.
///
/// This column holds every integer in `Long_Integer`, which is signed:
/// the key `16045690984833335998` is the negative `Long_Integer`
/// holding that bit pattern, and Ada's own `<` puts it first, ahead of
/// `7`. Fast DDS and OpenDDS write the unsigned order, `7` first —
/// which is what `MapKeyOrdering::UnsignedInteger` names and what
/// `Zd_Wire83.U64_Less` answers. Ada 83 has no modular type to borrow
/// the order from; Ada 95's `Interfaces.Unsigned_64` is what
/// `emit::ada` uses and what RM83 does not have.
///
/// The mutation this is here for: dropping the `unsigned_less`
/// override so the walk's default `<` is emitted. The assertions below
/// are red the moment the comparison is Ada's own operator.
#[test]
fn an_unsigned_long_long_map_key_is_compared_unsigned() {
    let text = codec_with_ascending_keys(
        "@final struct S { map<unsigned long long, short> m; };",
        "S",
    );
    // Twice, and not once: `map_key_before` asks the order in both
    // directions — `less(a, b)` and `not less(b, a)` — and a signed `<`
    // left in either of them decides the order for the pairs the other
    // one does not separate.
    assert_eq!(
        text.matches("Zd_Wire83.U64_Less (").count(),
        2,
        "the unsigned comparison is not both directions of the rank:\n{text}"
    );

    // An alias of the key type is the same key type: the crossing
    // resolves through it, and a column that stopped at the alias would
    // write the signed order for `typedef unsigned long long Id;`
    // alone.
    let text = codec_with_ascending_keys(
        "typedef unsigned long long Id; @final struct S { map<Id, short> m; };",
        "S",
    );
    assert_eq!(
        text.matches("Zd_Wire83.U64_Less (").count(),
        2,
        "an aliased key type lost the unsigned comparison:\n{text}"
    );

    // A signed eight-octet key keeps Ada's own `<`: `Long_Integer` is
    // signed and `MapKeyOrdering::SignedInteger` is exactly its order.
    let text = codec_with_ascending_keys("@final struct S { map<long long, short> m; };", "S");
    assert!(
        !text.contains("U64_Less"),
        "a signed key took the unsigned comparison:\n{text}"
    );

    // The narrower unsigned widths keep Ada's own `<`. They live
    // zero-extended in `Long_Integer` — `unsigned long`'s largest value
    // is 4294967295, thirty-two bits below `Long_Integer'Last` — so the
    // signed order and the unsigned order are the same order, and the
    // runtime call would be noise.
    for narrower in [
        "@final struct S { map<unsigned long, short> m; };",
        "@final struct S { map<unsigned short, short> m; };",
        "@final struct S { map<octet, short> m; };",
    ] {
        let text = codec_with_ascending_keys(narrower, "S");
        assert!(
            !text.contains("U64_Less"),
            "{narrower}: the eight-octet comparison reached a narrower key:\n{text}"
        );
    }
}

/// The comparison compiles against the real `Zd_Wire83` under
/// `-gnat83`.
///
/// `gnat_compiles` below compiles the type declarations alone, which
/// name no runtime operation at all; the codec is where
/// `Zd_Wire83.U64_Less` appears, so this assembles the two the way
/// `wireprobe-zerodds-ada83-gen` assembles `zd83_probe.adb` and hands
/// the whole unit to GNAT with `runtime/ada-83/src` on the search
/// path. A comparison spelled against a subprogram the runtime does
/// not declare is red here and nowhere else in this file.
#[test]
fn the_unsigned_key_comparison_compiles_against_the_runtime() {
    if !gnat_available() {
        eprintln!("SKIPPED: `gnatmake` is not on the path.");
        return;
    }
    const IDL: &str = "@final struct S { map<unsigned long long, short> m; };";
    let mut body = items_of(IDL);
    body.push('\n');
    body.push_str(&codec_with_ascending_keys(IDL, "S"));

    let mut unit = String::from("with Zd_Wire83;\nprocedure Zd83_Unit is\n");
    for line in body.lines() {
        if line.is_empty() {
            unit.push('\n');
        } else {
            unit.push_str("   ");
            unit.push_str(line);
            unit.push('\n');
        }
    }
    unit.push_str("begin\n   null;\nend Zd83_Unit;\n");
    if let Err(why) = gnat_compiles_unit(&unit) {
        panic!("{unit}\n\ngnatmake said:\n{why}");
    }
}

/// The declarations alone, as the probe generator takes them — not the
/// package `ada83_file` wraps them in.
fn items_of(idl: &str) -> String {
    let ast = model(idl);
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &ada83::catalogue()).expect("the rules apply");
    zerodds_lang_rules::adapter::ada83::ada83_items(&ast, &applied)
        .expect("the declarations materialise")
        .iter()
        .map(zerodds_lang_render::ada83::decl)
        .collect::<Vec<_>>()
        .join("\n")
}

/// One generated compilation unit, compiled against `runtime/ada-83`.
fn gnat_compiles_unit(source: &str) -> Result<(), String> {
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/ada-83/src")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let dir = std::env::temp_dir().join(format!(
        "zd-ada83-unit-{}-{}",
        std::process::id(),
        source.len()
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    std::fs::write(dir.join("zd83_unit.adb"), source).map_err(|error| error.to_string())?;
    let output = Command::new("gnatmake")
        .arg("-c")
        .arg("-gnat83")
        .arg(format!("-I{}", runtime.display()))
        .arg("zd83_unit.adb")
        .current_dir(&dir)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

// -- the corpus --------------------------------------------------------

/// Every corpus case, by name.
fn corpus() -> Vec<(String, std::path::PathBuf)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root).expect("the corpus directory") {
        let entry = entry.expect("a corpus entry");
        let idl = entry.path().join("type.idl");
        if idl.is_file() {
            out.push((entry.file_name().to_string_lossy().into_owned(), idl));
        }
    }
    out.sort();
    out
}

/// How many corpus cases have to reach Ada 83 for this gate to mean
/// anything.
///
/// Measured, not assumed: of 418 corpus cases with a `type.idl`, this
/// column renders 417 and refuses exactly one — `recursive-array` — by
/// name, per the module documentation's point 4: RM83 3.10.1 admits an
/// incomplete type only as an access type's designated subtype, never
/// as an array element, so an array closing a cycle has no bound in
/// any Ada revision. The other six recursive cases —
/// `recursive-mutual`, `recursive-optional-external`,
/// `recursive-self-sequence`, `recursive-through-map`,
/// `recursive-typedef-sequence`, `recursive-union-branch` — render,
/// through the RM83 3.10.1 cut ported from `adapter::ada`. No IDL
/// `interface` case is in the corpus today, so that refusal path is
/// exercised only by
/// `an_interface_is_refused_because_ada_83_has_no_form_for_one` above
/// and not by this gate. The bound below leaves a little slack under
/// the measured 417 rather than pinning it exactly.
const MUST_RENDER: usize = 412;

#[test]
fn every_corpus_case_renders_or_names_its_failure() {
    let cases = corpus();
    assert!(
        cases.len() > 300,
        "the corpus is not where it was: {} cases",
        cases.len()
    );
    let mut unexpected: Vec<String> = Vec::new();
    let mut frontend = Vec::new();
    let mut refused_by_this_column: Vec<String> = Vec::new();
    let mut rendered = 0_usize;
    for (name, path) in cases {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        match try_generate(&source, &path.display().to_string()) {
            Ok(_) => rendered += 1,
            Err(why) => {
                if why.starts_with("parse:") || why.starts_with("lower:") {
                    frontend.push(format!("{name}: {why}"));
                } else if why.contains("interface")
                    || why.contains("7.4.1.4.4.4.4")
                    || why.contains("no aggregate")
                {
                    refused_by_this_column.push(format!("{name}: {why}"));
                } else {
                    unexpected.push(format!("{name}: {why}"));
                }
            }
        }
    }
    if !frontend.is_empty() {
        eprintln!(
            "{} corpus case(s) the frontend refuses, so Ada 83 never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    eprintln!(
        "{} corpus case(s) this column refuses by name (interface, an array closing a \
         cycle, or a zero-component aggregate): {refused_by_this_column:#?}",
        refused_by_this_column.len()
    );
    assert!(
        unexpected.is_empty(),
        "refused for a reason that is neither an interface, an array closing a cycle, \
         nor an empty aggregate — investigate:\n{unexpected:#?}"
    );
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached Ada 83; the gate says nothing below \
         {MUST_RENDER}"
    );
}

// -- the compiler --------------------------------------------------------

/// One generated spec through GNAT, `-gnat83` — a probe that only
/// builds under a later mode is not this column's answer.
fn gnat_compiles(source: &str, tag: &str) -> Result<(), String> {
    // `-I` at the runtime's sources, the same switch
    // [`gnat_compiles_unit`] passes. The generated package names
    // `Zd_Wire83` in a `with` clause — Ada resolves a `with` at compile
    // time and not at bind time — so without the path GNAT stops at
    // `file "zd_wire83.ads" not found` before it has read a line of the
    // declarations this test is about.
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/ada-83/src")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let dir = std::env::temp_dir().join(format!(
        "zd-ada83-compile-{}-{tag}-{}",
        std::process::id(),
        source.len()
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    std::fs::write(dir.join(FILE_NAME), source).map_err(|error| error.to_string())?;
    let output = Command::new("gnatmake")
        .arg("-c")
        .arg("-gnat83")
        .arg(format!("-I{}", runtime.display()))
        .arg(FILE_NAME)
        .current_dir(&dir)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn gnat_available() -> bool {
    Command::new("gnatmake")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

const SAMPLES: &[(&str, &str)] = &[
    ("pose", "struct Pose { long x; double y; boolean active; };"),
    (
        "primitives",
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
    ),
    (
        "collections",
        "struct S { @optional octet b; long c; string name; wstring wide; \
         sequence<long> ids; long grid[2][3]; map<long, string> table; };",
    ),
    (
        "reserved",
        "struct T { long type; long range; long access; long RECORD; };",
    ),
    (
        "enum_union",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
    ),
    (
        "bitmask",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
    ),
    (
        "bitset",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
    ),
    (
        "valuetype",
        "valuetype V { public long counter; private string secret; };",
    ),
    (
        "constants",
        "const octet LIMIT = 8; const string TAG = \"abc\"; const double SCALE = 1e5; \
         const boolean ON = TRUE;",
    ),
    (
        "exception",
        "exception Failure { long code; string reason; };",
    ),
    ("native", "native Handle;"),
    (
        "module",
        "module common { module util { struct Location { long x; }; }; };",
    ),
    (
        "typedef",
        "typedef long Distance; struct Dist { Distance d; };",
    ),
    (
        "nested",
        "struct Inner { long x; }; struct Middle { Inner i; sequence<Inner> many; }; \
         struct Outer { Middle m; Middle grid[2]; };",
    ),
];

/// Every sample above, compiled under `-gnat83`.
#[test]
fn the_generated_ada_compiles_under_strict_ada_83() {
    if !gnat_available() {
        eprintln!(
            "SKIPPED: `gnatmake` is not on the path, so the generated Ada 83 was not \
             compiled. Looked for `gnatmake --version`; install GNAT (Debian: `gnat`, \
             Alire: `gnat_native`) or run this on a host that has it."
        );
        return;
    }
    for (label, idl) in SAMPLES {
        let generated = generate(idl);
        if let Err(why) = gnat_compiles(&generated, label) {
            panic!("{label}: {idl}\n\n{generated}\n\ngnatmake said:\n{why}");
        }
    }
}

/// The whole corpus through GNAT under `-gnat83`, one package per
/// case.
#[test]
#[ignore = "one `gnatmake` invocation per corpus case; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    assert!(
        gnat_available(),
        "the explicitly selected ada83 gate requires its compiler"
    );
    let mut failures = Vec::new();
    let mut checked = 0_usize;
    for (name, path) in corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let text = match try_generate(&source, &path.display().to_string()) {
            Ok(text) => text,
            // Preserve the explicitly documented Ada83 surface refusals from
            // every_corpus_case_renders_or_names_its_failure, never a silent
            // catch-all generator failure. The same minimum remains required.
            Err(why)
                if why.contains("interface")
                    || why.contains("7.4.1.4.4.4.4")
                    || why.contains("no aggregate") =>
            {
                eprintln!("EXPECTED_BACKEND_REFUSAL {name}: {why}");
                continue;
            }
            Err(why) => panic!("accepted corpus case {name} failed generation: {why}"),
        };
        checked += 1;
        if let Err(why) = gnat_compiles(&text, &name) {
            failures.push(format!("{name}: {why}"));
        }
    }
    eprintln!("{checked} corpus case(s) reached ada83_end_to_end");
    assert!(
        failures.is_empty(),
        "{} of the corpus did not compile:\n{failures:#?}",
        failures.len()
    );
    assert!(
        checked >= MUST_RENDER,
        "too few corpus cases reached the selected gate: {checked}"
    );
}
