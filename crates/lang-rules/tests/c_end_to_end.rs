// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to C, through every layer of the rebuild.
//!
//! The counterpart of `end_to_end.rs`, which does the same for Rust.
//! Parse, lower, adapt, apply, materialise, render — a test at any one
//! layer can pass while the chain is broken; this is the one that
//! cannot.
//!
//! What this does *not* check is the bytes. Those come out of
//! `runtime/c` when the generated code is compiled and run, and
//! `tools/wireprobe/wireprobe-zerodds-c` is what does that. A Rust test
//! that asserted bytes here would be asserting the layout plan a second
//! time and would pass whether or not any C compiler agrees.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::adapter::c::c_items;
use zerodds_lang_rules::decode_c::{c_decoder, decoder_name};
use zerodds_lang_rules::encode_c::{CCodecOptions, c_codec, c_key_holder, codec_name};
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

/// IDL in, C declarations out.
fn declarations(idl: &str) -> String {
    let ast = model(idl);
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::c::catalogue()).expect("apply");
    let decls = c_items(&ast, &applied).expect("materialise");
    zerodds_lang_render::c::file(&zerodds_lang_cst::c::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        guard: None,
        includes: Vec::new(),
        decls,
    })
}

/// IDL in, the encoder out, under one layout key.
fn codec(idl: &str, encoding: Encoding) -> String {
    let ast = model(idl);
    let key = LayoutKey::data(encoding, Endianness::Little);
    let decls = c_codec(&ast, key, CCodecOptions::default()).expect("codec");
    zerodds_lang_render::c::file(&zerodds_lang_cst::c::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        guard: None,
        includes: Vec::new(),
        decls,
    })
}

/// IDL in, the decoder out, under one layout key.
fn decoder(idl: &str, encoding: Encoding) -> String {
    let ast = model(idl);
    let key = LayoutKey::data(encoding, Endianness::Little);
    render(c_decoder(&ast, key, CCodecOptions::default()).expect("decoder"))
}

fn render(decls: Vec<zerodds_lang_cst::c::Decl>) -> String {
    zerodds_lang_render::c::file(&zerodds_lang_cst::c::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        guard: None,
        includes: Vec::new(),
        decls,
    })
}

// -- the declarations ----------------------------------------------------

#[test]
fn a_struct_of_primitives_becomes_a_c_struct() {
    let generated = declarations("struct Pose { long x; double y; boolean active; };");
    assert!(
        generated.contains("typedef struct Pose Pose;"),
        "{generated}"
    );
    assert!(generated.contains("int32_t x;"), "{generated}");
    assert!(generated.contains("double y;"), "{generated}");
    assert!(generated.contains("bool active;"), "{generated}");
}

#[test]
fn every_idl_primitive_reaches_a_c_type() {
    let generated = declarations(
        "struct P {
           boolean a; octet b; char c; wchar d;
           int8 e; uint8 f; short g; unsigned short h;
           long i; unsigned long j; long long k; unsigned long long l;
           float m; double n;
         };",
    );
    for expected in [
        "bool a;",
        "uint8_t b;",
        "char c;",
        "uint16_t d;",
        "int8_t e;",
        "uint8_t f;",
        "int16_t g;",
        "uint16_t h;",
        "int32_t i;",
        "uint32_t j;",
        "int64_t k;",
        "uint64_t l;",
        "float m;",
        "double n;",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn a_module_becomes_a_prefix_because_c_has_no_namespaces() {
    let generated = declarations(
        "module common { module util { struct Location { double x; }; }; };
         struct Robot { ::common::util::Location at; };",
    );
    assert!(
        generated.contains("typedef struct common_util_Location common_util_Location;"),
        "{generated}"
    );
    assert!(
        generated.contains("common_util_Location at;"),
        "{generated}"
    );
}

#[test]
fn a_sequence_becomes_a_length_and_a_buffer() {
    // Not a NUL-terminated run: a `sequence<octet>` may contain a zero,
    // and a terminator would cut the sample short without saying so.
    let generated = declarations("struct Bag { sequence<long> ids; string name; };");
    assert!(generated.contains("uint32_t _length;"), "{generated}");
    // [C-MAP]: a sequence is `_maximum`, `_length`, `_buffer` and `_release`.
    assert!(generated.contains("uint32_t _maximum;"), "{generated}");
    assert!(generated.contains("bool _release;"), "{generated}");
    assert!(generated.contains("int32_t *_buffer;"), "{generated}");
    assert!(generated.contains("zd_string name;"), "{generated}");
}

#[test]
fn two_collections_of_the_same_shape_are_one_declaration() {
    // The names are structural, so `sequence<long>` twice is one C type
    // and not two that differ only in a counter.
    let generated = declarations("struct Two { sequence<long> a; sequence<long> b; };");
    let first = generated
        .find("struct zd_seq_long {")
        .expect("the sequence");
    assert!(
        !generated[first + 1..].contains("struct zd_seq_long {"),
        "the declaration is emitted twice:\n{generated}"
    );
}

#[test]
fn a_double_and_a_long_double_collection_do_not_share_a_declaration() {
    // Both are a C `double` in memory and eight against sixteen bytes on
    // the wire. A structural name taken from the C type would give them
    // one encoder and send the second at the first's width.
    let generated = declarations("struct S { sequence<double> a; sequence<long double> b; };");
    assert!(generated.contains("zd_seq_double"), "{generated}");
    assert!(generated.contains("zd_seq_long_double"), "{generated}");
}

#[test]
fn an_array_keeps_its_dimensions_outermost_first() {
    let generated = declarations("struct S { long grid[2][3]; };");
    assert!(
        generated.contains("typedef int32_t zd_arr_long_2_3[2][3];"),
        "{generated}"
    );
}

#[test]
fn an_enumeration_becomes_an_integer_and_constants_not_a_c_enum() {
    // A C enumeration constant is an `int` (C11 §6.7.2.2) and
    // `@bit_bound(32)` admits values an `int` cannot hold, so the holder
    // is written out rather than left to the implementation.
    let generated = declarations("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(generated.contains("typedef int32_t Mode;"), "{generated}");
    assert!(generated.contains("#define IDLE ((Mode)0)"), "{generated}");
    assert!(
        generated.contains("#define ACTIVE ((Mode)4)"),
        "{generated}"
    );
    assert!(generated.contains("#define FAULT ((Mode)5)"), "{generated}");
}

#[test]
fn a_union_becomes_a_discriminator_beside_a_union() {
    let generated = declarations(
        "enum Mode { IDLE, ACTIVE };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(generated.contains("union Payload_u {"), "{generated}");
    assert!(generated.contains("Mode _d;"), "{generated}");
    assert!(generated.contains("union Payload_u _u;"), "{generated}");
}

#[test]
fn an_optional_member_is_a_pointer_and_null_is_the_absent_one() {
    // XTypes 1.3 §7.5.1.2.4.1: "Optional members shall be represented
    // using pointers. [...] a member of type X shall instead be mapped
    // as type pointer-to-X. [...] A NULL pointer shall indicate an
    // omitted value." There is no flag beside the member.
    let generated = declarations("@mutable struct S { @optional long maybe; };");
    assert!(generated.contains("int32_t *maybe;"), "{generated}");
    assert!(!generated.contains("_has_"), "{generated}");
    // "String and wide string members are already represented using
    // pointers, so the mappings for these members shall not change."
    let strings = declarations("@mutable struct T { @optional string s; @optional wstring w; };");
    assert!(strings.contains("zd_string s;"), "{strings}");
    assert!(strings.contains("zd_wstring w;"), "{strings}");
    assert!(!strings.contains("_has_"), "{strings}");
}

#[test]
fn a_derived_struct_holds_the_base_as_its_first_member_parent() {
    // XTypes 1.3 §7.5.1.4.1.3: "The language binding shall be generated as
    // if an instance of the base type were the first member of the subtype
    // with the name 'parent'". The plan still puts the base's members on
    // the wire first, and the codec reaches them as `parent.m`.
    let generated = declarations(
        "@final struct Base { long base_a; };
         @final struct Derived : Base { long own; };
         @final struct Further : Derived { long last; };",
    );
    let derived = generated.find("struct Derived {").expect("Derived");
    let body = &generated[derived
        ..generated[derived..]
            .find("};")
            .map_or(generated.len(), |at| derived + at)];
    let parent = body.find("Base parent;").expect("the member parent");
    let own = body.find("own;").expect("the own member");
    assert!(parent < own, "the base is not first:\n{body}");
    assert!(
        !body.contains("base_a;"),
        "the base's member is flattened in:\n{body}"
    );
    // And a base of a base is a parent of a parent.
    let further = generated.find("struct Further {").expect("Further");
    assert!(
        generated[further..].contains("Derived parent;"),
        "{generated}"
    );
    // The table of IDL names reaches the inherited members by their path.
    assert!(
        generated.contains("parent.base_a") && generated.contains("parent.parent.base_a"),
        "{generated}"
    );
}

#[test]
fn a_bitmask_becomes_an_integer_and_one_constant_per_bit() {
    let generated = declarations("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("typedef uint16_t Flags;"), "{generated}");
    // XTypes 1.3 §7.5.1.2.2: the flags are the literals of an enumeration
    // `<TypeName>Bits`, valued `1 << position`.
    assert!(
        generated.contains("typedef uint16_t FlagsBits;"),
        "{generated}"
    );
    assert!(
        generated.contains("#define READY ((FlagsBits)1 << 0)"),
        "{generated}"
    );
    assert!(
        generated.contains("#define MOVING ((FlagsBits)1 << 4)"),
        "{generated}"
    );
}

#[test]
fn a_bitset_becomes_separate_integers_not_c_bitfields() {
    // C leaves the allocation order of bitfields to the implementation,
    // and the order on the wire is the declaration's. The packing is the
    // codec's, where it can be seen.
    let generated = declarations("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("uint8_t low;"), "{generated}");
    assert!(generated.contains("uint16_t high;"), "{generated}");
    assert!(!generated.contains(" : 3;"), "{generated}");
}

#[test]
fn a_constant_becomes_a_define_and_an_exception_a_struct() {
    let generated = declarations("const octet LIMIT = 8; exception Failure { long code; };");
    assert!(
        generated.contains("#define LIMIT ((uint8_t)8)"),
        "{generated}"
    );
    assert!(
        generated.contains("typedef struct Failure Failure;"),
        "{generated}"
    );
}

#[test]
fn a_native_declaration_becomes_an_opaque_typedef() {
    let generated = declarations("native Handle;");
    assert!(generated.contains("typedef void *Handle;"), "{generated}");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { sequence<A> ys; };";
    assert_eq!(declarations(idl), declarations(idl));
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // The prelude is the two string shapes and nothing else; a
    // user-declared annotation adds no declaration of its own.
    let generated = declarations("@annotation Sensitive { string reason; };");
    assert!(!generated.contains("Sensitive"), "{generated}");
}

#[test]
fn a_type_is_declared_before_it_is_used() {
    // The Rust materialiser can walk the model in declaration order; C
    // cannot, because a translation unit that names a struct before
    // declaring it does not compile.
    let generated = declarations(
        "struct Inner { long x; };
         struct Outer { Inner one; };",
    );
    let inner = generated.find("struct Inner {").expect("Inner");
    let outer = generated.find("struct Outer {").expect("Outer");
    assert!(
        inner < outer,
        "the declaration order is wrong:\n{generated}"
    );
}

// -- the codec -----------------------------------------------------------

#[test]
fn an_appendable_struct_takes_the_dheader_the_plan_gives_it() {
    let generated = codec("@appendable struct S { long a; };", Encoding::PlainCdr2);
    assert!(
        generated.contains("zdw_dheader d = zdw_write_dheader(w);"),
        "{generated}"
    );
    assert!(
        generated.contains("zdw_finish_dheader(w, d);"),
        "{generated}"
    );
}

#[test]
fn a_final_struct_takes_no_dheader_and_still_says_so() {
    // The body's shape does not depend on the plan's answer:
    // `zdw_no_dheader` writes nothing and finishes nothing, so the two
    // cases differ in one call and not in their structure.
    let generated = codec("@final struct S { long a; };", Encoding::PlainCdr2);
    assert!(
        generated.contains("zdw_dheader d = zdw_no_dheader();"),
        "{generated}"
    );
}

#[test]
fn a_mutable_member_carries_the_length_code_the_plan_chose() {
    // A string's own length word doubles as the NEXTINT, which is length
    // code 5. A generator that wrote 4 there would put every following
    // member four bytes late.
    let generated = codec(
        "@mutable struct S { long a; string text; };",
        Encoding::PlCdr2,
    );
    assert!(
        generated.contains("zdw_write_emheader(w, 0, 2, false)"),
        "the four-byte member is not LC2:\n{generated}"
    );
    assert!(
        generated.contains("zdw_write_emheader(w, 1, 5, false)"),
        "the string is not LC5:\n{generated}"
    );
}

#[test]
fn a_mutable_struct_under_xcdr1_ends_with_a_sentinel() {
    let generated = codec("@mutable struct S { long a; };", Encoding::PlCdr1);
    assert!(generated.contains("zdw_write_pid"), "{generated}");
    assert!(generated.contains("zdw_write_sentinel(w);"), "{generated}");
}

#[test]
fn an_optional_member_is_written_only_when_it_is_there() {
    let generated = codec(
        "@mutable struct S { @optional long maybe; };",
        Encoding::PlCdr2,
    );
    assert!(generated.contains("if (v->maybe != NULL) {"), "{generated}");
}

#[test]
fn a_sequence_of_a_primitive_carries_no_delimiter_and_one_of_a_struct_does() {
    // §7.4.3.5 rules (8) to (13), read off the plan and not decided
    // again here.
    let primitive = codec("struct S { sequence<long> ids; };", Encoding::PlainCdr2);
    let position = primitive
        .find("static void zd_enc_zd_seq_long(")
        .and_then(|at| primitive[at..].find("zdw_no_dheader()"));
    assert!(position.is_some(), "{primitive}");

    let aggregate = codec(
        "@final struct Cell { long a; }; struct S { sequence<Cell> cells; };",
        Encoding::PlainCdr2,
    );
    let at = aggregate
        .find("static void zd_enc_zd_seq_Cell(zdw_writer *w, const zd_seq_Cell *v) {")
        .expect("the sequence encoder");
    assert!(
        aggregate[at..].starts_with(
            "static void zd_enc_zd_seq_Cell(zdw_writer *w, const zd_seq_Cell *v) {\n    zdw_dheader d = zdw_write_dheader(w);"
        ),
        "{}",
        &aggregate[at..at + 200]
    );
}

#[test]
fn a_union_selects_its_branch_by_the_discriminator() {
    let generated = codec(
        "union Choice switch (long) { case 1: case 2: long whole; default: double rest; };",
        Encoding::PlainCdr2,
    );
    assert!(
        generated.contains("if (v->_d == (int32_t)1 || v->_d == (int32_t)2) {"),
        "{generated}"
    );
    assert!(generated.contains("v->_u.whole"), "{generated}");
    assert!(generated.contains("v->_u.rest"), "{generated}");
}

#[test]
fn a_union_member_the_stream_does_not_carry_takes_its_default() {
    // M11. §7.2.4.4.8, Table 19, the UNION_TYPE row: "If the
    // discriminator value does not select any member in O2, then: If the
    // discriminator value selects a member m1 in O1, then m1 is set to
    // its default value according to its type." Two versions of a union
    // whose newer one gave an existing discriminator value a case is
    // exactly that; RTI is the witness, `evo-union-unselected`, which
    // hands back `{"_d":2,"b":0}` in both encoding versions.
    //
    // This column refused the payload instead: `read_mmember` treated a
    // `PID_LIST_END` where the member's parameter would be as a stream
    // that disagrees with the type. Which of the two it is is read off
    // the framing — the sentinel under rule (28), the delimiter's extent
    // under rule (27) — and never guessed.
    let v1 = decoder(
        "@mutable union Choice switch (long) { case 1: long a; case 2: short b; };",
        Encoding::PlCdr1,
    );
    assert!(v1.contains("if (zd_hu1.end) {"), "{v1}");
    assert!(v1.contains("zd_def_p_long(&v->_u.a);"), "{v1}");
    assert!(v1.contains("zd_sent = true;"), "{v1}");
    // The sentinel is read once, and the member's own read is what may
    // have consumed it.
    assert!(v1.contains("if (!zd_sent) {"), "{v1}");

    let v2 = decoder(
        "@mutable union Choice switch (long) { case 1: long a; case 2: short b; };",
        Encoding::PlCdr2,
    );
    assert!(
        v2.contains("if (zdw_reader_position(r) >= zd_end) {"),
        "{v2}"
    );
    assert!(v2.contains("zd_def_p_short(&v->_u.b);"), "{v2}");

    // A `@final` union gets neither, and that is the clause and not an
    // omission: Table 19 requires two final unions to have an identical
    // set of labels, so a stream missing the member is a type mismatch.
    let final_ = decoder(
        "@final union Choice switch (long) { case 1: long a; case 2: short b; };",
        Encoding::PlainCdr2,
    );
    assert!(!final_.contains("zd_def_p_long"), "{final_}");
}

#[test]
fn a_wide_union_label_keeps_a_literal_its_type_can_hold() {
    // `case 16045690984833335998:` over an `unsigned long long` does not
    // fit a signed integer, and a bare decimal there is a constant whose
    // type the compiler picks.
    let generated = codec(
        "union Choice switch (unsigned long long) {
           case 16045690984833335998: long whole; case 7: double rest; };",
        Encoding::PlainCdr2,
    );
    assert!(
        generated.contains("UINT64_C(16045690984833335998)"),
        "{generated}"
    );
}

#[test]
fn an_array_encoder_takes_the_array_and_everything_else_an_address() {
    // C adjusts an array parameter to a pointer to its first element,
    // which is what lets the body index it directly.
    let generated = codec("struct S { long grid[2][3]; };", Encoding::PlainCdr2);
    assert!(
        generated
            .contains("static void zd_enc_zd_arr_long_2_3(zdw_writer *w, const zd_arr_long_2_3 v)"),
        "{generated}"
    );
    assert!(
        generated.contains("zd_enc_zd_arr_long_2_3(w, v->grid);"),
        "{generated}"
    );
}

#[test]
fn a_keyed_type_gets_a_holder_and_an_unkeyed_one_does_not() {
    let keyed = model("@final struct S { @key long id; long payload; };");
    let root = keyed
        .types()
        .find(|(_, node)| node.name == "S")
        .map(|(id, _)| id)
        .expect("S");
    let holder_key = LayoutKey::key(Encoding::PlainCdr2);
    let holder = c_key_holder(&keyed, root, holder_key).expect("holder");
    assert!(holder.is_some(), "a keyed type has no holder");

    let plain = model("@final struct S { long payload; };");
    let root = plain
        .types()
        .find(|(_, node)| node.name == "S")
        .map(|(id, _)| id)
        .expect("S");
    assert!(
        c_key_holder(&plain, root, holder_key)
            .expect("holder")
            .is_none(),
        "a KeyHash over nothing is not a KeyHash"
    );
}

/// XTypes 1.3 §7.6.8 Step 1 over a union keyed by its discriminator: the
/// holder is the discriminator alone. The member walk finds no `@key` member
/// in a union, and the C column refused the KeyHash of
/// `key-on-union-discriminator` ("m::U declares no @key member") where the
/// Rust column wrote `00000001` and twelve zero octets.
#[test]
fn a_union_keyed_by_its_discriminator_gets_a_holder_of_the_discriminator() {
    let ast = model(
        "module m { @final union U switch (@key long) { case 1: long a; case 2: short b; }; };",
    );
    let root = ast
        .types()
        .find(|(_, node)| node.name == "U")
        .map(|(id, _)| id)
        .expect("U");
    let holder = c_key_holder(&ast, root, LayoutKey::key(Encoding::PlainCdr2))
        .expect("holder")
        .expect("a union keyed by its discriminator has a holder");
    let generated = render(vec![holder]);
    assert!(generated.contains("zd_keyholder_m_U"), "{generated}");
    assert!(
        generated.contains("v->_d"),
        "the holder writes the discriminator: {generated}"
    );
    assert!(
        !generated.contains("v->_u"),
        "the branches are not keys: {generated}"
    );
}

#[test]
fn a_key_holder_under_rti_states_the_short_header_decision_it_never_receives() {
    // Under `--dialect rti` with XCDR1 the PL_CDR1 header form is decided
    // per data sample, so every encoder body takes a `zd_extended` flag
    // the sample's root hands down. A key holder is `Purpose::Key`: the
    // reflective planner keeps it short whatever the sample
    // (`extended_as_constructed` gates on `Purpose::Data`), so the holder
    // has no decision to receive and must pass the constant `false` to the
    // bodies it calls. Naming the flag instead is a C compile error, which
    // is how this was found.
    let ast = model(
        "@mutable struct Inner { long a; };
         @mutable struct S { @key Inner part; long tail; };",
    );
    let root = ast
        .types()
        .find(|(_, node)| node.name == "S")
        .map(|(id, _)| id)
        .expect("S");
    let key = LayoutKey::in_dialect(
        Dialect::Rti,
        Encoding::PlCdr1,
        Endianness::Big,
        Purpose::Key,
    );
    let holder = c_key_holder(&ast, root, key)
        .expect("holder")
        .expect("a keyed type has a holder");
    let generated = render(vec![holder]);
    assert!(
        generated.contains("zd_enc_Inner_zd_extended(w, &v->part, false)"),
        "{generated}"
    );
    assert!(
        !generated.contains(", zd_extended)"),
        "the holder references a flag it does not take: {generated}"
    );
}

#[test]
fn the_data_and_key_encoders_do_not_share_a_name() {
    // A key holder is a different plan over the same types — only the
    // `@key` members — so it needs its own set of functions.
    let ast = model("@final struct S { @key long id; long payload; };");
    let root = ast
        .types()
        .find(|(_, node)| node.name == "S")
        .map(|(id, _)| id)
        .expect("S");
    assert_ne!(
        codec_name(&ast, root, Purpose::Data),
        codec_name(&ast, root, Purpose::Key)
    );
}

#[test]
fn a_bitset_that_inherits_packs_the_base_at_the_low_positions() {
    // Fast DDS is the only implementation that parses `bitset Derived :
    // Base`, and it flattens the chain into one `std::bitset<N>` with
    // the base's bitfields at the least significant end in declaration
    // order — `docs/refused-constructs.md`. `low` therefore sits at
    // position 0 and `high` at position 3, and the anchor is sized over
    // both.
    let ast = model(
        "bitset Base { bitfield<3> low; };
         bitset Derived : Base { bitfield<5> high; };
         @final struct S { Derived value; };",
    );
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let decls = c_codec(&ast, key, CCodecOptions::default()).expect("an encoder");
    let text = render(decls);
    assert!(
        text.contains("zdw_bitfield(w, (uint64_t)v->low, 3, 0)"),
        "the base's field takes the low positions: {text}"
    );
    assert!(
        text.contains("zdw_bitfield(w, (uint64_t)v->high, 5, 3)"),
        "the derived field starts above the base's three bits: {text}"
    );
    assert!(
        text.contains("zdw_write_bitset(w, 1, bits)"),
        "one anchor over the whole chain: {text}"
    );
}

// -- the decoder ---------------------------------------------------------

#[test]
fn the_encoder_and_the_decoder_do_not_share_a_name() {
    // Both directions are emitted into one translation unit by the
    // vendor-bytes gate, and a name they shared would be a redefinition
    // rather than a diagnosis.
    let ast = model("@final struct S { long a; };");
    let root = ast
        .types()
        .find(|(_, node)| node.name == "S")
        .map(|(id, _)| id)
        .expect("S");
    assert_ne!(
        codec_name(&ast, root, Purpose::Data),
        decoder_name(&ast, root)
    );
}

#[test]
fn a_decoder_reads_a_primitive_through_the_runtime_and_not_by_hand() {
    // Every byte-level rule stays in `runtime/c`. A decoder that
    // assembled an integer out of `zdw_read_u8` would be a second
    // implementation of the byte order in the generated code, where no
    // second opinion can be diffed against it.
    let generated = decoder(
        "@final struct S { long a; double b; };",
        Encoding::PlainCdr2,
    );
    assert!(generated.contains("*v = zdw_read_i32(r);"), "{generated}");
    assert!(generated.contains("*v = zdw_read_f64(r);"), "{generated}");
}

#[test]
fn a_sequence_decoder_checks_the_count_before_it_allocates() {
    // A count larger than the stream is how a reader turns a malformed
    // message into an out-of-memory. The check is in front of the
    // allocation and not behind it.
    let generated = decoder(
        "@final struct S { sequence<long> ids; };",
        Encoding::PlainCdr2,
    );
    let check = generated
        .find("ZDW_IMPOSSIBLE_LENGTH")
        .expect("the count is checked");
    let alloc = generated
        .find("zdw_reader_alloc")
        .expect("the elements are allocated");
    assert!(check < alloc, "{generated}");
}

#[test]
fn a_bounded_sequence_decoder_refuses_a_count_past_the_bound() {
    let generated = decoder(
        "@final struct S { sequence<long, 4> ids; };",
        Encoding::PlainCdr2,
    );
    assert!(generated.contains("zd_n > 4u"), "{generated}");
}

#[test]
fn an_enumeration_decoder_assigns_the_declared_value_and_not_the_pattern() {
    // §7.4.3.5.3 makes the holder signed, so `@value(200)` under
    // `@bit_bound(8)` reads back as `-56`. The C binding stores the
    // declared value, which is what the encoder writes from, so an arm
    // assigning the sign-extended pattern produces an integer no
    // enumerator constant equals.
    let generated = decoder(
        "@bit_bound(8) enum Colour { @value(200) BLUE };
         @final struct S { Colour c; };",
        Encoding::PlainCdr2,
    );
    assert!(generated.contains("zd_raw == INT64_C(-56)"), "{generated}");
    assert!(generated.contains("*v = BLUE;"), "{generated}");
}

#[test]
fn an_unknown_enumerator_is_refused_rather_than_handed_back_as_a_number() {
    let generated = decoder(
        "enum Colour { RED, GREEN };
         @final struct S { Colour c; };",
        Encoding::PlainCdr2,
    );
    // `ZDW_DISCARDED` and not `ZDW_OUT_OF_RANGE`: Table 18 makes an
    // enumerator this type does not declare a construction failure, and
    // DDS-XTypes 1.3 §7.2.2.7 says the sample is discarded rather than
    // the stream being wrong. The two statuses are kept apart so a
    // subscriber can tell a narrowed peer from a broken one.
    assert!(
        generated.contains("zdw_reader_refuse(r, ZDW_DISCARDED)"),
        "{generated}"
    );
}

/// The four behaviours DDS-XTypes 1.3 §7.2.2.7 defines, in the C
/// column.
///
/// The behaviour is a *parameter* of every decoder and not a property of
/// the type: the clause attaches it to the position — this member, this
/// element, this map key, this discriminator — and one
/// `typedef string<5>` may be used at two positions that want different
/// answers.
#[test]
fn a_decoder_takes_the_behaviour_of_the_position_that_calls_it() {
    let generated = decoder(
        "@mutable struct S { long a; @try_construct(TRIM) string<5> m; \
         @try_construct(USE_DEFAULT) string<5> d; string<5> p; };",
        Encoding::PlainCdr2,
    );
    assert!(
        generated.contains("zdw_try_construct zd_tc"),
        "the decoders take no behaviour: {generated}"
    );
    for constant in ["ZDW_TC_TRIM", "ZDW_TC_USE_DEFAULT", "ZDW_TC_DISCARD"] {
        assert!(
            generated.contains(constant),
            "no call passes {constant}: {generated}"
        );
    }
}

/// The bound is applied after the value has been read, never instead of
/// reading it.
///
/// A decoder that stopped at its own bound would leave the rest of the
/// string in the stream and put every following member at the wrong
/// offset — a decode that succeeds and is wrong, which is worse than the
/// overflow it was avoiding.
#[test]
fn a_bounded_string_is_read_whole_and_narrowed_afterwards() {
    let generated = decoder(
        "@mutable struct S { @try_construct(TRIM) string<5> m; };",
        Encoding::PlainCdr2,
    );
    let read = generated
        .find("zdw_read_string_alloc")
        .expect("the string is read");
    let measured = generated
        .find("zdw_utf8_octets")
        .expect("the bound is measured");
    assert!(read < measured, "the bound is checked first: {generated}");
    assert!(generated.contains("zdw_trim_utf8"), "{generated}");
}

/// Table 11 does not define TRIM off a string, wstring, sequence or map,
/// and the IDL frontend refuses such a declaration. A model built
/// another way still must not silently trim: the generated code has no
/// trim arm for an enumeration at all, and falls through to the discard.
#[test]
fn an_enumeration_has_no_trim_arm() {
    let generated = decoder(
        "enum Colour { RED, GREEN };
         @mutable struct S { @try_construct(USE_DEFAULT) Colour c; };",
        Encoding::PlainCdr2,
    );
    assert!(generated.contains("ZDW_DISCARDED"), "{generated}");
    // Table 9's `ENUM_TYPE` row is the first literal, or the
    // `@default_literal` where one is declared.
    assert!(generated.contains("RED"), "{generated}");
}

#[test]
fn a_bitfield_is_narrowed_into_the_width_the_materialiser_gave_it() {
    // The anchor is sixty-four bits and the member is as wide as
    // `c_holder` made it; an implicit narrowing is a conversion the
    // compiler makes quietly and `-Wconversion` refuses.
    let generated = decoder(
        "bitset Flags { bitfield<3> low; bitfield<5> high; };
         @final struct S { Flags f; };",
        Encoding::PlainCdr2,
    );
    assert!(generated.contains("(uint8_t)"), "{generated}");
    assert!(!generated.contains("v->low = (uint64_t)"), "{generated}");
}

#[test]
fn a_mutable_aggregate_is_driven_by_its_headers_and_not_by_the_plan() {
    // A writer may order the members as it likes and leave any
    // `@optional` one out, so a decoder walking its own plan would
    // misread both.
    let generated = decoder(
        "@mutable struct S { long a; @optional long b; };",
        Encoding::PlainCdr2,
    );
    assert!(generated.contains("while ("), "{generated}");
    assert!(generated.contains("zd_h.member_id =="), "{generated}");
    assert!(generated.contains("zd_h.must_understand"), "{generated}");
}

#[test]
fn a_mutable_aggregate_under_xcdr1_stops_at_the_sentinel() {
    let generated = decoder("@mutable struct S { long a; };", Encoding::PlCdr1);
    assert!(generated.contains("zd_p.end"), "{generated}");
    assert!(generated.contains("break;"), "{generated}");
    assert!(generated.contains("zdw_finish_pid_read"), "{generated}");
}

#[test]
fn an_optional_member_outside_a_mutable_type_reads_its_marker() {
    // §7.4.3.5.2 rule (20) puts a BOOLEAN `<is_present>` in front of an
    // `OPT_FMEMBER` under XCDR2, so the presence flag comes off the wire
    // and the branches say what the structure holds. Nothing is guessed,
    // nothing rewinds, and the `zd_guessed` flag that made the guess
    // falsifiable has nothing left to falsify.
    //
    // The flag lands in a local first, and that is the backend seam
    // rather than a style: a target where absence *is* the value has
    // nowhere to put a `boolean` — `v.x != null` is not an lvalue — so
    // `CodecTarget::set_present` says what each column does with it. C
    // assigns its parallel field in both branches; Java assigns `null`
    // in one and nothing in the other.
    let generated = decoder(
        "@appendable struct S { long a; @optional long b; long c; };",
        Encoding::DelimitedCdr,
    );
    assert!(
        generated.contains("bool zd_has_b = zdw_read_bool(r);"),
        "{generated}"
    );
    assert!(generated.contains("if (zd_has_b) {"), "{generated}");
    // Both branches, so the structure never carries what the caller's
    // storage happened to hold: present gets storage, absent gets NULL.
    assert!(
        generated.contains("v->b = (int32_t *)zdw_reader_alloc(r, sizeof(int32_t));"),
        "{generated}"
    );
    assert!(generated.contains("v->b = NULL;"), "{generated}");
    assert!(!generated.contains("zdw_reader_save(r)"), "{generated}");
    assert!(!generated.contains("zd_guessed"), "{generated}");
}

#[test]
fn an_optional_member_under_xcdr1_reads_a_parameter_header_either_way() {
    // §7.4.1.2: "If the member takes no value within that particular
    // object, the data length indicated by the header shall be zero."
    // The header is always there, so a header for another id is a
    // refusal and not a rewind.
    let generated = decoder(
        "@appendable struct S { long a; @optional long b; long c; };",
        Encoding::PlainCdr1,
    );
    assert!(
        generated.contains("zdw_read_pid_as(r, ZDW_PARAM_OPTIONAL)"),
        "{generated}"
    );
    assert!(
        generated.contains("zd_has_b = zd_p1.length != 0u"),
        "{generated}"
    );
    assert!(
        generated.contains("zdw_reader_refuse(r, ZDW_OUT_OF_RANGE)"),
        "{generated}"
    );
}

#[test]
fn an_optional_member_outside_a_mutable_type_writes_its_marker() {
    // The write side of the two rules above: the marker goes out whether
    // or not the member does, and the value only behind it.
    let flagged = codec(
        "@final struct S { long a; @optional octet b; long c; };",
        Encoding::PlainCdr2,
    );
    assert!(
        flagged.contains("zdw_write_bool(w, v->b != NULL);"),
        "{flagged}"
    );
    assert!(flagged.contains("if (v->b != NULL) {"), "{flagged}");

    let parameter = codec(
        "@final struct S { long a; @optional octet b; long c; };",
        Encoding::PlainCdr1,
    );
    assert!(parameter.contains("ZDW_PARAM_OPTIONAL"), "{parameter}");
    // The header is outside the presence test and the value inside it,
    // which is what makes an absent member declare a length of zero.
    let header = parameter.find("ZDW_PARAM_OPTIONAL").expect("the header");
    let guard = parameter[header..]
        .find("if (v->b != NULL)")
        .expect("the guard");
    let finish = parameter[header..]
        .find("zdw_finish_pid")
        .expect("the patch");
    assert!(guard < finish, "{parameter}");
}

#[test]
fn a_mutable_member_that_did_not_arrive_takes_its_default() {
    // §7.4.1.2.3: "if the receiver's member is not optional, it
    // logically takes its default value". This used to refuse with
    // ZDW_TRUNCATED, which is the one exchange `@mutable` exists for —
    // a new subscriber reading an old publisher. Cyclone, RTI and Fast
    // DDS all default; `evo-mutable-tail`, 2026-08-09.
    let generated = decoder("@mutable struct S { long a; };", Encoding::PlainCdr2);
    assert!(!generated.contains("ZDW_TRUNCATED"), "{generated}");
    assert!(generated.contains("if (zd_seen0 == false)"), "{generated}");
    assert!(generated.contains("zd_def_p_long(&v->a)"), "{generated}");
    assert!(generated.contains("*v = 0;"), "{generated}");
}

#[test]
fn every_table_nine_row_has_a_c_default() {
    // One member per row of Table 9, behind a `@mutable` type so each
    // can go missing. The values are the ones Cyclone, RTI and Fast DDS
    // all hand back for the same IDL — `evo-mutable-defaults`.
    let generated = decoder(
        "enum Colour { RED, GREEN, BLUE };
         @appendable struct Inner { long n; string t; };
         @mutable struct S {
             @id(1) octet by; @id(2) boolean bo; @id(3) short i16;
             @id(4) long long i64; @id(5) float f32; @id(6) double f64;
             @id(7) char c8; @id(8) string text; @id(9) Colour e;
             @id(10) sequence<long> sq; @id(11) long arr[3];
             @id(12) Inner nested;
         };",
        Encoding::PlainCdr2,
    );
    for expected in [
        "static void zd_def_p_octet(uint8_t *v)",
        "static void zd_def_p_boolean(bool *v)",
        "*v = false;",
        "INT64_C(0)",
        "*v = 0.0f;",
        "*v = 0.0;",
        "*v = \"\";",
        "v->_maximum = 0u;",
        "v->_length = 0u;",
        "v->_buffer = NULL;",
        "v->_release = false;",
        "*v = RED;",
    ] {
        assert!(generated.contains(expected), "{expected}\n{generated}");
    }
}

#[test]
fn an_appendable_tail_the_writer_did_not_have_takes_its_default() {
    // The other half. Under XCDR2 the DHEADER says where the writer
    // stopped; under XCDR1 there is none and the payload's end is what
    // all three vendors use — `evo-appendable-tail`.
    let delimited = decoder(
        "@appendable struct S { long a; double b; short c; };",
        Encoding::PlainCdr2,
    );
    assert!(delimited.contains("bool zd_tail = false;"), "{delimited}");
    assert!(
        delimited.contains("zdw_reader_position(r) >= zd_end"),
        "{delimited}"
    );
    assert!(delimited.contains("zd_def_p_short(&v->c)"), "{delimited}");

    let undelimited = decoder(
        "@appendable struct S { long a; double b; short c; };",
        Encoding::PlainCdr1,
    );
    // `0` and not `0u`: `zdw_reader_remaining` answers in the target's
    // own size type, and the walk compares a size against a plain zero
    // — `crates/lang-rules/src/emit/decode.rs`'s `reached` says why.
    assert!(
        undelimited.contains("zdw_reader_remaining(r) == 0"),
        "{undelimited}"
    );

    // A `@final` type gets neither: Table 19 requires the same set of
    // member IDs on both sides, so a short stream is a type mismatch.
    let final_ = decoder(
        "@final struct S { long a; double b; short c; };",
        Encoding::PlainCdr2,
    );
    assert!(!final_.contains("zd_tail"), "{final_}");
}

#[test]
fn an_explicit_default_beats_table_nine() {
    // §7.2.2.4.4.4.9: the implicit default is the type's only where the
    // declaration gives no explicit one. Substituting zero for a
    // `@default(42)` would be a silently wrong value.
    let generated = decoder(
        "@mutable struct S { @id(1) long a; @id(2) @default(42) long b; };",
        Encoding::PlainCdr2,
    );
    assert!(generated.contains("v->b = 42;"), "{generated}");
    assert!(generated.contains("zd_def_p_long(&v->a)"), "{generated}");
}

#[test]
fn a_bitset_that_inherits_reads_the_whole_chain_too() {
    // The two directions cover the same constructs, so a case is either
    // in the matrix on both sides or in neither.
    let ast = model(
        "bitset Base { bitfield<3> low; };
         bitset Derived : Base { bitfield<5> high; };
         @final struct S { Derived value; };",
    );
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let decls = c_decoder(&ast, key, CCodecOptions::default()).expect("a decoder");
    let text = render(decls);
    assert!(text.contains("v->low"), "{text}");
    assert!(text.contains("v->high"), "{text}");
}

#[test]
fn a_long_double_is_one_c_type_in_the_declaration_and_in_both_codecs() {
    // The declaration, the encoder's parameter, the decoder's
    // destination, the defaulter's destination and a sequence's element
    // storage are five places that name the C type of one member. They
    // have to name the same one: `zdw_f128` is sixteen bytes and the
    // `double` `c_primitive` answers is eight, so a disagreement is
    // either a translation unit that does not compile — which is what
    // the three-column gate found — or, where a pointer conversion is
    // silent, an allocation half the size the elements need.
    //
    // Written as five assertions over one IDL rather than as a unit test
    // of `c_value_type`, because the defect was never in that function:
    // it was in the three emitters that each kept a copy of it.
    let idl = "@final struct S { long double one; sequence<long double> many; };";

    let declared = declarations(idl);
    assert!(declared.contains("zdw_f128 one;"), "{declared}");
    assert!(declared.contains("zdw_f128 *_buffer;"), "{declared}");
    assert!(
        !declared.contains("double one;"),
        "the member is declared as a C `double`:\n{declared}"
    );

    let written = codec(idl, Encoding::PlainCdr2);
    assert!(
        written.contains("zd_enc_p_long_double(zdw_writer *w, const zdw_f128 *v)"),
        "{written}"
    );

    let read = decoder(idl, Encoding::PlainCdr2);
    assert!(
        read.contains("zd_dec_p_long_double(zdw_reader *r, zdw_f128 *v"),
        "{read}"
    );
    // The element storage a bounded-or-not sequence allocates: the
    // `sizeof` and the cast both have to be the sixteen-byte type.
    assert!(read.contains("sizeof(zdw_f128)"), "{read}");
    assert!(read.contains("(zdw_f128 *)zdw_reader_alloc"), "{read}");
}

#[test]
fn every_defaulter_the_decoder_calls_is_also_defined() {
    // The invariant, checked structurally rather than case by case: a
    // `zd_def_*` the decoder calls and does not define is a translation
    // unit that does not compile, and the interop corpus is where that
    // used to be found — a night later, in a C compiler's words.
    //
    // The list below is one IDL per reason a defaulter exists. The two
    // that were missing are the last four: a bounded collection and a
    // bounded string default *themselves* when the stream overruns the
    // bound under `@try_construct(USE_DEFAULT)`, and `@final` permits
    // it, so the seeding that only walked a non-final aggregate's
    // members never reached them.
    let sources = [
        // A member that can go missing: `@mutable` and `@appendable`.
        "@mutable struct S { long a; string b; };",
        "@appendable struct S { long a; sequence<long> b; };",
        // A union case the discriminator selects and the stream omits.
        "@mutable union U switch (long) { case 1: long one; default: string other; };
         @final struct S { U picked; };",
        // An explicit `@try_construct(USE_DEFAULT)` member.
        "@final struct S { @try_construct(USE_DEFAULT) string<4> text; };",
        // A bound the sender can overrun, with no annotation at all.
        "@final struct S { string<5> text; };",
        "@final struct S { wstring<5> text; };",
        "@final struct S { sequence<long,2> v; };",
        "typedef string<5> Name; @final struct S { Name n; };",
        "typedef sequence<long,2> Row; @final struct S { Row r; };",
        // An array of a bounded element, where the default is reached
        // through the element and not through the member.
        "@final struct S { sequence<string<3>,2> grid[2]; };",
    ];
    for idl in sources {
        for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
            let text = decoder(idl, encoding);
            for name in called_defaulters(&text) {
                assert!(
                    text.contains(&format!("static void {name}(")),
                    "`{name}` is called and not defined, under {encoding:?}, \
                     for `{idl}`:\n{text}"
                );
            }
        }
    }
}

/// Every `zd_def_*` the rendered C calls, by name.
///
/// Reads the rendered text rather than the CST: what has to hold is a
/// property of the file a C compiler sees, and a walk of the tree that
/// the emitter and the test both did would agree with the emitter about
/// a mistake.
fn called_defaulters(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (index, _) in text.match_indices("zd_def_") {
        // A definition or a prototype, not a call: `static void ` or a
        // `;` after the parameter list. Only the call sites matter, and
        // a call is preceded by whitespace and followed by `(`.
        if text[..index].ends_with("static void ") {
            continue;
        }
        let rest = &text[index..];
        let Some(end) = rest.find('(') else { continue };
        let name = &rest[..end];
        if name.contains(char::is_whitespace) {
            continue;
        }
        if !out.iter().any(|seen| seen == name) {
            out.push(name.to_string());
        }
    }
    out
}

// -- what the model holds and C does not ---------------------------------

#[test]
fn a_member_of_type_any_is_refused_by_name_rather_than_emitted() {
    // `any` is in the model because Building Block Any is one of
    // IDL 4.2's fifteen. What it holds is decided at run time, so there
    // is no C type for it and no wire form to generate against.
    //
    // Both halves used to say nothing: the header wrote `any a;`, which
    // no compiler reads, and reported success. The generator now names
    // the construct and the language.
    let ast = model("struct S { any a; long b; };");
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::c::catalogue()).expect("apply");
    let error = c_items(&ast, &applied).expect_err("`any` has no C form");
    let text = error.to_string();
    assert!(text.contains("any"), "{text}");
    assert!(text.contains('C'), "{text}");

    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let refused = c_codec(&ast, key, CCodecOptions::default()).expect_err("no encoder");
    assert!(refused.to_string().contains("any"), "{refused}");
    assert!(refused.to_string().contains("CORBA CDR"), "{refused}");
}

#[test]
fn a_member_of_type_object_is_refused_by_name_rather_than_emitted() {
    // The same rule as `any`, one building block on. An object reference
    // is a handle into the RPC arc — `coverage.rs` says so and
    // `zerodds_ast_layout` refuses to plan one — and writing `Object o;`
    // into a C header produced a file no compiler reads.
    let ast = model("struct S { Object o; long b; };");
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::c::catalogue()).expect("apply");
    let error = c_items(&ast, &applied).expect_err("`Object` has no C form");
    assert!(error.to_string().contains("Object"), "{error}");
}

/// The `Char16` codec is marked as possibly uncalled under TAO's
/// framing, and under no other.
///
/// TAO writes GIOP 1.2's octet count for a `wchar` member and a bare
/// unit for a `wchar` collection element, so the element form is written
/// inline at the call site. A type whose only `Char16` is an array
/// element then leaves `zd_enc_p_wchar` without a caller — and the
/// generated C is compiled under `-Werror=unused-function`, which turned
/// 1120 slots of the full-tier three-column gate red.
///
/// The attribute is the narrow answer and it has to stay narrow: an
/// uncalled generated codec is otherwise a generator that emitted the
/// wrong thing, and that warning is worth keeping. So this asserts both
/// halves — present under TAO, absent under every other dialect. The
/// list below is the nine and not "the shipped default": which of them
/// is the default has moved once and the property is the same for all
/// of them.
#[test]
fn only_taos_wchar_codec_may_be_uncalled() {
    let idl = "@final struct Sample { wchar many[3]; };";
    let ast = model(idl);
    let render = |dialect| {
        let key = LayoutKey::in_dialect(
            dialect,
            Encoding::PlainCdr1,
            Endianness::Little,
            Purpose::Data,
        );
        let decls = c_codec(&ast, key, CCodecOptions::default()).expect("codec");
        zerodds_lang_render::c::file(&zerodds_lang_cst::c::File {
            doc: zerodds_lang_cst::common::Doc::none(),
            guard: None,
            includes: Vec::new(),
            decls,
        })
    };

    let tao = render(zerodds_ast_layout::Dialect::Tao);
    assert!(
        tao.contains("__attribute__((unused))"),
        "TAO's wchar codec is not marked and the C will not compile:\n{tao}"
    );
    assert!(
        tao.contains("ZDW_WCHAR_FIXED"),
        "the element write did not take the element framing:\n{tao}"
    );

    for dialect in zerodds_ast_layout::Dialect::ALL
        .into_iter()
        .filter(|dialect| *dialect != zerodds_ast_layout::Dialect::Tao)
    {
        // A preset with an axis the **C runtime** does not carry has no
        // C to inspect: the codec is refused. `runtime/c` now carries all
        // three of `Dialect::Hdds`'s axes, so this is the empty set and
        // every dialect below is rendered; a target that has not been
        // given them is refused in `hdds_axes_are_refused.rs`.
        let key = LayoutKey::in_dialect(
            dialect,
            Encoding::PlainCdr1,
            Endianness::Little,
            Purpose::Data,
        );
        if zerodds_lang_rules::encode::axes_the_codecs_carry_for::<zerodds_lang_rules::emit::c::C>(
            key,
        )
        .is_err()
        {
            assert!(
                c_codec(&ast, key, CCodecOptions::default()).is_err(),
                "{dialect:?} selects an axis the codecs do not carry and still got a C codec"
            );
            continue;
        }
        let text = render(dialect);
        assert!(
            !text.contains("__attribute__((unused))"),
            "{dialect:?} marks a codec as possibly uncalled; only TAO's framing splits the \
             positions, so this would hide a real generator defect:\n{text}"
        );
    }
}
