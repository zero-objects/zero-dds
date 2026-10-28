// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Encoder and decoder bodies, built from the layout plan.
//!
//! Every framing decision in the output has to be one the plan made.
//! A body that framed a member differently from its own plan is a wire
//! format that disagrees with itself.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, Extensibility};
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::key::{
    Dialect, Encoding, Endianness, EnumeratorValue, LayoutKey, LengthCodePolicy,
    WstringLengthPolicy, WstringUnitOrder,
};
use zerodds_lang_rules::encode::{rust_codec, rust_codec_pair};

/// Lower `idl`, set `extensibility` on `type_name`, and generate.
fn codec(idl: &str, type_name: &str, key: LayoutKey) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let ast: Ast = lowered.ast;

    let id = ast
        .find_by_scoped_name(&ScopedName::absolute(vec![type_name.to_string()]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("{type_name} is not in the model"));

    let items = rust_codec(&ast, id, key).expect("codec");
    let file = zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::none(),
        uses: Vec::new(),
        items,
    };
    zerodds_lang_render::rust::file(&file)
}

/// As `codec`, through the second-pair entry point.
fn codec_pair(idl: &str, type_name: &str, key: LayoutKey) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let ast: Ast = lowered.ast;
    let id = ast
        .find_by_scoped_name(&ScopedName::absolute(vec![type_name.to_string()]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("{type_name} is not in the model"));
    let items = rust_codec_pair(&ast, id, key).expect("codec");
    let file = zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::none(),
        uses: Vec::new(),
        items,
    };
    zerodds_lang_render::rust::file(&file)
}

/// A second codec pair on one type carries its own names, all the way
/// into the calls it makes on a member.
///
/// A Rust type holds one inherent function per name, so the two streams
/// of a type that carries both cannot both be `encode`. What matters
/// beyond the two names is the third: the nested call. A body named
/// `encode_xcdr1` that called a member's `encode` would frame that
/// member under version 2 inside a version 1 stream, and the sample
/// would decode into wrong values with nothing reporting it.
#[test]
fn a_second_pair_renames_itself_and_every_call_it_makes() {
    let source = "@final struct Cell { long a; }; \
                  @final struct S { Cell nested; sequence<Cell> cells; };";
    let v1 = LayoutKey::data(Encoding::PlainCdr1, Endianness::Little);

    let pair = codec_pair(source, "S", v1);
    assert!(
        pair.contains("pub fn encode_xcdr1(&self, w: &mut Writer)"),
        "{pair}"
    );
    assert!(
        pair.contains("pub fn decode_xcdr1(r: &mut Reader)"),
        "{pair}"
    );
    assert!(
        pair.contains("self.nested.encode_xcdr1(w)?;"),
        "the nested call takes the same names: {pair}"
    );
    assert!(
        pair.contains("Cell::decode_xcdr1(r)?"),
        "the nested call takes the same names: {pair}"
    );
    assert!(
        pair.contains("|w, v| v.encode_xcdr1(w)"),
        "an element call too: {pair}"
    );

    // And nothing else: the `Default` impl and the key holder do not
    // belong to an encoding, and a second copy of either would be a
    // duplicate definition.
    assert!(!pair.contains("impl Default for S"), "{pair}");
    assert!(!pair.contains("key_hash"), "{pair}");
}

/// The first pair keeps the bare names whatever encoding it is given.
///
/// Every consumer that generates one codec per type — `crates/wire-
/// conformance` builds XCDR1 ones — calls `encode` and `decode`, and a
/// version-derived name would have renamed those out from under them.
#[test]
fn the_first_pair_is_named_the_same_under_both_versions() {
    let source = "@final struct S { long a; };";
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        let text = codec(source, "S", LayoutKey::data(encoding, Endianness::Little));
        assert!(
            text.contains("pub fn encode(&self, w: &mut Writer)"),
            "{text}"
        );
        assert!(text.contains("pub fn decode(r: &mut Reader)"), "{text}");
        assert!(!text.contains("encode_xcdr1"), "{text}");
    }
}

/// The error `codec` would have panicked on.
fn codec_error(
    idl: &str,
    type_name: &str,
    key: LayoutKey,
) -> zerodds_lang_rules::encode::EncodeError {
    let parsed = zerodds_idl_parse::parse(idl);
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    let ast: Ast = lowered.ast;
    let id = ast
        .find_by_scoped_name(&ScopedName::absolute(vec![type_name.to_string()]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("{type_name} is not in the model"));
    rust_codec(&ast, id, key).expect_err("this type has no codec")
}

/// The XCDR2 encoding an extensibility calls for.
///
/// Plain for `@final`, delimited for `@appendable`, a parameter list for
/// `@mutable` — the encoding is what the extensibility means on the
/// wire, not a separate choice.
fn xcdr2_for(extensibility: Extensibility) -> LayoutKey {
    let encoding = match extensibility {
        Extensibility::Final => Encoding::PlainCdr2,
        Extensibility::Appendable => Encoding::DelimitedCdr,
        Extensibility::Mutable => Encoding::PlCdr2,
    };
    LayoutKey::data(encoding, Endianness::Little)
}

fn xcdr2() -> LayoutKey {
    xcdr2_for(Extensibility::Final)
}

#[test]
fn a_final_struct_of_primitives_encodes_as_a_run_of_writes() {
    // No DHEADER, no headers per member: the plan says the extent is
    // fixed, so the body is exactly the writes.
    let generated = codec(
        "@final struct Point { long x; double y; };",
        "Point",
        xcdr2(),
    );
    assert!(
        generated.contains("pub fn encode(&self, w: &mut Writer) -> Result<(), WireError> {"),
        "{generated}"
    );
    assert!(generated.contains("w.write_i32(self.x);"), "{generated}");
    assert!(generated.contains("w.write_f64(self.y);"), "{generated}");
    assert!(
        !generated.contains("write_dheader"),
        "a final type gained a DHEADER:\n{generated}"
    );
    assert!(
        !generated.contains("write_emheader"),
        "a final type gained member headers:\n{generated}"
    );
}

#[test]
fn the_decoder_mirrors_the_encoder_member_for_member() {
    // Built from the same plan, so the two agree by construction rather
    // than by review.
    let generated = codec(
        "@final struct Point { long x; double y; };",
        "Point",
        xcdr2(),
    );
    // The decoder is fallible: one that could not fail would have to
    // guess at bytes that are not there, and a guess is a value nobody
    // sent.
    assert!(
        generated.contains("pub fn decode(r: &mut Reader) -> Result<Self, WireError> {"),
        "{generated}"
    );
    assert!(generated.contains("let x = r.read_i32()?;"), "{generated}");
    assert!(generated.contains("let y = r.read_f64()?;"), "{generated}");
    assert!(generated.contains("Ok(Self { x, y })"), "{generated}");
    assert!(generated.contains("Self { x, y }"), "{generated}");
}

#[test]
fn an_appendable_struct_carries_the_dheader_the_plan_asks_for() {
    // XCDR2 frames an appendable type with a DHEADER; a reader without
    // one cannot skip members it does not know.
    let generated = codec(
        "@appendable struct S { long a; };",
        "S",
        xcdr2_for(Extensibility::Appendable),
    );
    // Opened as a placeholder and patched when the length is known: a
    // DHEADER never patched leaves a zero on the wire, and a reader that
    // trusts it stops before the first member.
    assert!(
        generated.contains("let dheader = w.write_dheader();"),
        "{generated}"
    );
    assert!(
        generated.contains("w.finish_dheader(dheader);"),
        "{generated}"
    );
    // On the read side the delimiter is bound rather than discarded:
    // §7.2.4.4.8 lets an `@appendable` writer stop before this reader's
    // last member, and the DHEADER is the only thing that says where.
    assert!(
        generated.contains("let end = r.read_dheader()? + r.position();"),
        "{generated}"
    );
}

/// XTypes 1.3 §7.4.3.5 rule (11): an `@appendable` union is a DHEADER,
/// then the discriminator and the member it selects. This column wrote
/// rule (10) — the `@final` form — for every union, so an `@appendable`
/// one went out four octets short of what the C, Java and C# columns
/// write (measured, `idl-conformance/_interop` `mapenum`).
#[test]
fn an_appendable_union_carries_the_dheader_the_plan_asks_for() {
    let generated = codec(
        "@appendable union U switch (long) { case 1: long a; default: double b; };",
        "U",
        xcdr2_for(Extensibility::Appendable),
    );
    assert!(
        generated.contains("let dheader = w.write_dheader();"),
        "{generated}"
    );
    assert!(
        generated.contains("w.finish_dheader(dheader);"),
        "{generated}"
    );
    // The reader binds the union's end and steps to it after the branch:
    // a newer writer's branch may carry more than this reader's type.
    assert!(
        generated.contains("let end = r.read_dheader()? + r.position();"),
        "{generated}"
    );
    assert!(generated.contains("r.skip_to(end)?;"), "{generated}");
}

/// Rules (27) and (28): a `@mutable` union frames its discriminator under
/// member id 0 and the selected branch under the branch's own id.
#[test]
fn a_mutable_union_frames_its_discriminator_and_its_branch() {
    let generated = codec(
        "@mutable union U switch (long) { case 1: @id(5) long a; default: @id(6) short b; };",
        "U",
        xcdr2_for(Extensibility::Mutable),
    );
    assert!(generated.contains("w.write_emheader(0,"), "{generated}");
    assert!(generated.contains("w.write_emheader(5,"), "{generated}");
    assert!(
        generated.contains("let dheader = w.write_dheader();"),
        "{generated}"
    );
    assert!(
        generated.matches("r.read_emheader()?").count() >= 2,
        "the reader does not read both member headers:\n{generated}"
    );
}

/// A `@final` union is rule (10) alone, as before.
#[test]
fn a_final_union_is_the_discriminator_and_its_branch_alone() {
    let generated = codec(
        "@final union U switch (long) { case 1: long a; default: short b; };",
        "U",
        xcdr2(),
    );
    assert!(
        !generated.contains("write_dheader") && !generated.contains("read_dheader"),
        "a final union gained a DHEADER:\n{generated}"
    );
    assert!(!generated.contains("emheader"), "{generated}");
}

#[test]
fn a_mutable_struct_writes_an_emheader_per_member() {
    let generated = codec(
        "@mutable struct S { @id(3) long a; @id(7) long b; };",
        "S",
        xcdr2_for(Extensibility::Mutable),
    );
    // The member id is the completion pass's, carried into the header.
    assert!(generated.contains("w.write_emheader(3,"), "{generated}");
    assert!(generated.contains("w.write_emheader(7,"), "{generated}");
}

#[test]
fn every_parameter_header_the_decoder_opens_it_also_closes() {
    // `Reader::read_pid` makes the parameter's value its own alignment
    // origin — XTypes 1.3 §7.4.1.2 calls each parameter "a
    // CDR-encapsulated block of data" — and `finish_pid` is the only
    // thing that puts the origin back, steps over the padding behind the
    // value and checks the declared length against what was read. The
    // encoder's `finish_pid` had no mirror, so the origin stayed at the
    // first block's start for the rest of the aggregate.
    let mutable = codec(
        "@mutable struct M { @id(1) double d; @id(2) short c; };",
        "M",
        LayoutKey::data(Encoding::PlCdr1, Endianness::Little),
    );
    assert_eq!(
        mutable.matches("r.finish_pid(pid)?;").count(),
        2,
        "one parameter block per member is opened and not closed:\n{mutable}"
    );
    // Rule (23): the list ends at its sentinel, not at the last member
    // the reader recognises. The decoder is header-driven, so the
    // sentinel is what ends the loop rather than a parameter read after
    // the last member the plan names.
    assert!(mutable.contains("while !ended {"), "{mutable}");
    assert!(mutable.contains("if pid.end {"), "{mutable}");
    assert!(mutable.contains("ended = true;"), "{mutable}");

    let optional = codec(
        "@final struct S { @optional octet b; double c; };",
        "S",
        LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
    );
    assert!(optional.contains("r.finish_pid(pid)?;"), "{optional}");
    assert!(
        !optional.contains("read_pid()"),
        "an OPT_FMEMBER header was read in the `Member` role:\n{optional}"
    );
    // An `@optional` member outside a `@mutable` type carries no
    // sentinel — I21, and all three vendors agree.
    assert!(!optional.contains("sentinel"), "{optional}");
}

#[test]
fn the_length_code_policy_is_visible_in_the_output() {
    // The two policies are both conformant and produce different bytes;
    // which one a build uses has to be readable in what it generated.
    let idl = "@mutable struct S { @id(1) string name; };";
    let compact = codec(
        idl,
        "S",
        LayoutKey::data(Encoding::PlCdr2, Endianness::Little)
            .with_length_codes(LengthCodePolicy::Compact),
    );
    let universal = codec(
        idl,
        "S",
        LayoutKey::data(Encoding::PlCdr2, Endianness::Little)
            .with_length_codes(LengthCodePolicy::UniversalNextInt),
    );
    assert_ne!(
        compact, universal,
        "the two length-code policies produced the same body"
    );
}

#[test]
fn a_member_with_no_writer_is_reported_rather_than_left_out() {
    // A member missing from an encoder is a wire format that silently
    // disagrees with its own plan. The property is the reporting and not
    // the construct, so this test moves to whatever is still unwritten
    // rather than being deleted with the gap it named: it stood on
    // `map` until `map` got a writer, then on `wstring` until this
    // runtime grew `write_wstring` and the eight cells of C1 × C2 × C3.
    // `fixed<P, S>` has a writer too now. A value box reaches the
    // emitter but still has no writer. Unlike `any` and `native`, it
    // does not fail earlier in layout. Keep supported members on both
    // sides of the gap and require the missing member to be named.
    let source = "valuetype V long; @final struct S { long before; V payload; long after; };";
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let error = codec_error(source, "S", xcdr2());
    assert!(
        matches!(
            &error,
            zerodds_lang_rules::encode::EncodeError::NoWriter { member, kind }
                if member == "payload" && *kind == "value box"
        ),
        "{error:?}"
    );
}

#[test]
fn a_map_writes_both_halves_of_each_entry_and_reads_them_back() {
    // The emitter arm, as text. `write_map` takes the container and one
    // element function per half — the key's first — and the runtime
    // writes the entry count from `BTreeMap::len`, so no count appears
    // here. The bytes those calls produce are held against a recorded
    // Fast DDS row in
    // `crates/wire-conformance/tests/map_bytes.rs`.
    //
    // Version 1, so the delimiter is not in the way of the shape being
    // asserted; rule (14) is the next test's question.
    let generated = codec(
        "@final struct S { map<string, long> entries; };",
        "S",
        LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
    );
    assert!(
        generated.contains(
            "w.write_map(&self.entries, |w, v| { w.write_string(v); Ok(()) }, |w, v| { w.write_i32(*v); Ok(()) })?;"
        ),
        "{generated}"
    );
    assert!(
        generated.contains("r.read_map_entries(Reader::read_string, Reader::read_i32)?"),
        "{generated}"
    );
}

#[test]
fn a_map_is_delimited_by_what_the_plan_says_and_not_by_its_own_arm() {
    // Rule (14) exempts a map from the DHEADER only when **both** halves
    // are primitive; rule (15) delimits the rest under XCDR2. The
    // emitter asks `zerodds_ast_layout::xcdr::plan` rather than deciding
    // it, so a plan and a body cannot disagree about the framing of the
    // same member.
    let exempt = codec("@final struct S { map<long, long> m; };", "S", xcdr2());
    assert!(exempt.contains("w.write_map(&self.m,"), "{exempt}");
    assert!(!exempt.contains("write_delimited_map"), "{exempt}");

    let delimited = codec("@final struct S { map<string, long> m; };", "S", xcdr2());
    assert!(
        delimited.contains("w.write_delimited_map(&self.m,"),
        "{delimited}"
    );
    assert!(
        delimited.contains("r.read_delimited_map_entries("),
        "the reader does not skip the delimiter the writer wrote:\n{delimited}"
    );

    // Version 1 delimits nothing at all, which is the other half of the
    // same claim: the arm did not choose, the plan did.
    let v1 = codec(
        "@final struct S { map<string, long> m; };",
        "S",
        LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
    );
    assert!(!v1.contains("write_delimited_map"), "{v1}");
}

#[test]
fn a_bounded_map_passes_its_bound_to_the_runtime() {
    // `map<K, V, N>`: Table 17's `O2.length <= T1.length` in entries.
    // The wire carries no bound, so the check is the writer's and the
    // bytes are a plain map's.
    let generated = codec("@final struct S { map<long, long, 4> m; };", "S", xcdr2());
    assert!(
        generated.contains("w.write_map_bounded(&self.m, 4,"),
        "{generated}"
    );
    // Table 11 names `map` among the forms a TRIM may be written on, so
    // the read side narrows through `fit_map` and not through
    // `fit_sequence`, whose unit is an element (`fit_map_entries` for a map held as its
    // entries, `fit_map` for a `BTreeMap`).
    assert!(generated.contains("r.fit_map_entries(m, 4,"), "{generated}");
}

#[test]
fn a_nested_aggregate_encodes_itself() {
    let generated = codec(
        "@final struct Header { long id; };
         @final struct Body { Header head; long value; };",
        "Body",
        xcdr2(),
    );
    // Literally itself: the nested type's own `encode` carries its own
    // plan, which is why there is no `Writer::write_nested` for it to
    // have gone through.
    assert!(generated.contains("self.head.encode(w)?;"), "{generated}");
    assert!(
        generated.contains("let head = Header::decode(r)?;"),
        "{generated}"
    );
    assert!(
        generated.contains("w.write_i32(self.value);"),
        "{generated}"
    );
}

#[test]
fn the_extensibility_the_source_wrote_decides_the_framing() {
    // Not a setting of the generator: three sources, three framings,
    // each the one its own annotation asks for.
    let final_ = codec("@final struct S { long a; };", "S", xcdr2());
    let appendable = codec(
        "@appendable struct S { long a; };",
        "S",
        xcdr2_for(Extensibility::Appendable),
    );
    let mutable = codec(
        "@mutable struct S { long a; };",
        "S",
        xcdr2_for(Extensibility::Mutable),
    );

    assert!(!final_.contains("write_dheader"));
    assert!(appendable.contains("write_dheader"));
    assert!(mutable.contains("write_emheader"));
}

// -- collections ---------------------------------------------------------

#[test]
fn a_sequence_writes_its_elements_by_their_own_writer() {
    // The element function is a closure and not a named path.
    // `Writer::write_i32` takes an `i32` by value where an element
    // function takes a `&i32`, so the path never compiled.
    let generated = codec("@final struct Bag { sequence<long> ids; };", "Bag", xcdr2());
    assert!(
        generated.contains("w.write_sequence(&self.ids, |w, v| { w.write_i32(*v); Ok(()) })?;"),
        "{generated}"
    );
    assert!(
        generated.contains("let ids = r.read_sequence(Reader::read_i32)?;"),
        "{generated}"
    );
}

#[test]
fn a_bounded_sequence_checks_its_bound_and_an_unbounded_one_has_none() {
    // The bound is a promise about the source, not a difference on the
    // wire: both write a length and then the elements. What the bounded
    // form adds is the refusal — Table 17 makes
    // `O2.length <= T1.length` the condition for constructing the
    // value at all, and a sender that wrote more elements than its own
    // declaration allows has written a value of a type nobody declared.
    let bounded = codec("@final struct B { sequence<long, 4> ids; };", "B", xcdr2());
    assert!(
        bounded.contains(
            "w.write_sequence_bounded(&self.ids, 4, |w, v| { w.write_i32(*v); Ok(()) })?;"
        ),
        "{bounded}"
    );
    let unbounded = codec("@final struct B { sequence<long> ids; };", "B", xcdr2());
    assert!(!unbounded.contains("write_sequence_bounded"), "{unbounded}");
}

#[test]
fn a_bounded_string_is_written_through_the_form_that_refuses_an_overrun() {
    // AL-14: `write_string_bounded` truncated at a character boundary
    // and reported nothing, which was tolerable only while nothing
    // called it. The generator calls it now, so it refuses instead —
    // the same values `wire-probe`'s `write_string` refuses (D6).
    let generated = codec("@final struct S { string<32> name; };", "S", xcdr2());
    assert!(
        generated.contains("w.write_string_bounded(&self.name, 32)?;"),
        "{generated}"
    );
}

#[test]
fn an_array_writes_no_length_because_its_count_is_in_the_type() {
    // One `write_array` per dimension: `long grid[2][3]` is
    // `[[i32; 3]; 2]`, so the outer call hands each row to a function
    // that writes a row. The count came out as trailing arguments the
    // runtime never took.
    let generated = codec("@final struct S { long grid[2][3]; };", "S", xcdr2());
    assert!(
        generated.contains(
            "w.write_array(&self.grid, |w, v| w.write_array(v, |w, v| { w.write_i32(*v); Ok(()) }))?;"
        ),
        "{generated}"
    );
    // And back into `[[i32; 3]; 2]`, whose lengths are inferred from the
    // field rather than passed as arguments.
    assert!(
        generated.contains("let grid = r.read_array_n(|r| r.read_array_n(Reader::read_i32))?;"),
        "{generated}"
    );
}

#[test]
fn a_collection_of_aggregates_is_delimited_and_one_of_primitives_is_not() {
    // XTypes 1.3 §7.4.3.5 rules (9) and (12). The generator asks
    // `zerodds_ast_layout::collection_is_delimited`, the same function
    // the planner asks, so the emitted call and the planned extent
    // cannot disagree. Cyclone and Fast DDS both write this delimiter;
    // `crates/ast-layout/tests/fixtures/vendor_bytes.md` has the bytes.
    let aggregates = codec(
        "@final struct Cell { long a; }; @final struct S { sequence<Cell> cells; Cell grid[2]; };",
        "S",
        xcdr2(),
    );
    // A nested aggregate encodes itself. There is no
    // `Writer::write_nested` and there was never going to be one — the
    // nested type's own plan is in its own `encode`.
    assert!(
        aggregates.contains("w.write_delimited_sequence(&self.cells, |w, v| v.encode(w))?;"),
        "{aggregates}"
    );
    assert!(
        aggregates.contains("w.write_delimited_array(&self.grid, |w, v| v.encode(w))?;"),
        "{aggregates}"
    );
    assert!(
        aggregates.contains("r.read_delimited_sequence(Cell::decode)?"),
        "{aggregates}"
    );

    // A primitive element takes no delimiter, under either version.
    let primitives = codec("@final struct S { sequence<long> ids; };", "S", xcdr2());
    assert!(
        primitives.contains("w.write_sequence(&self.ids, |w, v| { w.write_i32(*v); Ok(()) })?;"),
        "{primitives}"
    );

    // Neither does anything under XCDR1.
    let v1 = codec(
        "@final struct Cell { long a; }; @final struct S { sequence<Cell> cells; };",
        "S",
        LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
    );
    assert!(
        v1.contains("w.write_sequence(&self.cells, |w, v| v.encode(w))?;"),
        "{v1}"
    );
}

#[test]
fn a_collection_directly_inside_a_collection_is_emitted() {
    // AL-9, closed. The three shapes
    // `docs/refused-constructs.md` measured against Cyclone and
    // Fast DDS — `sequence<sequence<long>>`, `sequence<long> rows[2]`
    // and `sequence<Row>` over `typedef long Row[3]` — each nest one
    // element function inside another. The generator refused all three
    // because it emitted a *named path* as the element function, and
    // `Writer::write_sequence` takes its own element function as a
    // second argument, so no path has the right shape.
    let sequences = codec(
        "@final struct S { sequence<sequence<long> > rows; octet tail; };",
        "S",
        xcdr2(),
    );
    assert!(
        sequences.contains(
            "w.write_delimited_sequence(&self.rows, |w, v| w.write_sequence(v, |w, v| { w.write_i32(*v); Ok(()) }))?;"
        ),
        "{sequences}"
    );
    assert!(
        sequences.contains(
            "let rows = r.read_delimited_sequence(|r| r.read_sequence(Reader::read_i32))?;"
        ),
        "{sequences}"
    );

    let array_of_sequences = codec(
        "@final struct S { sequence<long> rows[2]; octet tail; };",
        "S",
        xcdr2(),
    );
    assert!(
        array_of_sequences.contains(
            "w.write_delimited_array(&self.rows, |w, v| w.write_sequence(v, |w, v| { w.write_i32(*v); Ok(()) }))?;"
        ),
        "{array_of_sequences}"
    );

    let sequence_of_arrays = codec(
        "typedef long Row[3]; @final struct S { sequence<Row> rows; octet tail; };",
        "S",
        xcdr2(),
    );
    assert!(
        sequence_of_arrays.contains(
            "w.write_delimited_sequence(&self.rows, |w, v| w.write_array(v, |w, v| { w.write_i32(*v); Ok(()) }))?;"
        ),
        "{sequence_of_arrays}"
    );
    assert!(
        sequence_of_arrays.contains(
            "let rows = r.read_delimited_sequence(|r| r.read_array_n(Reader::read_i32))?;"
        ),
        "{sequence_of_arrays}"
    );
}

#[test]
fn a_mutable_decoder_dispatches_on_the_member_id_and_a_positional_one_does_not() {
    // XTypes 1.3 §7.4.1.2.3 leaves a `@mutable` list unordered and
    // §7.4.3.4.2 puts the id on every header. A decoder that walked its
    // plan could take neither a reordered list nor an omitted member;
    // one that read a `@final` type by id would be inventing a framing
    // its own encoder does not write.
    let mutable = codec(
        "@mutable struct M { @id(1) long a; @id(2) short b; };",
        "M",
        xcdr2_for(Extensibility::Mutable),
    );
    assert!(mutable.contains("while r.position() < end {"), "{mutable}");
    assert!(mutable.contains("if header.member_id == 1 {"), "{mutable}");
    assert!(mutable.contains("if header.member_id == 2 {"), "{mutable}");
    // A member no header named takes its default — §7.4.1.2.3. Table 9's
    // zero for a `long` is also Rust's, so the spelling is the one clippy
    // asks for.
    assert!(
        mutable.contains("let a = a.unwrap_or_default();"),
        "{mutable}"
    );
    // The dispatch is one `else if` chain and not an `else { if .. }` per
    // member.
    assert!(
        mutable.contains("} else if header.member_id == 2 {"),
        "{mutable}"
    );
    // And one this type does not declare is skipped, or discards the
    // sample when the writer said it may not be.
    assert!(mutable.contains("if header.must_understand {"), "{mutable}");
    assert!(mutable.contains("r.skip_to(ends)?;"), "{mutable}");

    let final_ = codec("@final struct S { long a; short b; };", "S", xcdr2());
    assert!(
        !final_.contains("member_id =="),
        "a positional type read by id:\n{final_}"
    );
}

// -- `@optional` ---------------------------------------------------------

#[test]
fn an_optional_member_writes_its_marker_from_the_option_and_not_from_a_literal() {
    // Rule (20), §7.4.3.5.2. The `<is_present>` byte used to be a
    // literal `true`, because the field was the member's own type and
    // this column had no absence to write. It is the field's own answer
    // now, and the absence writes `false` and no value —
    // `opt_final_absent` XCDR2, which Cyclone, RTI and Fast DDS all
    // write as `00000000` where the present form is `01 ab 0000`.
    let generated = codec(
        "@final struct S { long a; @optional octet b; long c; };",
        "S",
        xcdr2(),
    );
    assert!(generated.contains("match &self.b {"), "{generated}");
    assert!(generated.contains("w.write_bool(true);"), "{generated}");
    assert!(generated.contains("w.write_u8(*b);"), "{generated}");
    assert!(generated.contains("w.write_bool(false);"), "{generated}");
    assert!(
        !generated.contains("has no Option to hold"),
        "the refusal survived the field that made it necessary:\n{generated}"
    );

    // And the decoder answers `Some`/`None` off the same byte.
    assert!(
        generated.contains("let b = if r.read_bool()? {"),
        "{generated}"
    );
    assert!(generated.contains("Some(r.read_u8()?)"), "{generated}");
    assert!(generated.contains("None"), "{generated}");
}

#[test]
fn an_optional_member_under_xcdr1_marks_the_absence_with_a_declared_length_of_zero() {
    // Rule (19) and §7.4.1.2. The parameter header is written whether or
    // not the value is — that is what makes the length the marker — so
    // it sits outside the match and `finish_pid` closes the block either
    // way. A decoder that skipped the close would leave the alignment
    // origin inside the parameter for the rest of the aggregate.
    //
    // The header's **form** is a dialect question and not this
    // property: `PlCdr1HeaderPolicy::Specification` takes the short
    // header wherever §7.4.1.2.2 allows it and `Dialect::Cyclone` takes
    // the extended one throughout. Both are asked, and the form is named
    // per dialect rather than read off whichever is `Dialect::default` —
    // this assertion was `write_pid_extended` against `LayoutKey::data`
    // alone, which was Cyclone's answer and was silently the default's.
    let idl = "@final struct S { @optional octet b; double c; };";
    let plan =
        |dialect| LayoutKey::data(Encoding::PlainCdr1, Endianness::Little).with_dialect(dialect);
    for (dialect, header) in [
        (
            Dialect::Specification,
            "let pid = w.write_pid(0, ParameterRole::Optional, false)?;",
        ),
        (
            Dialect::Cyclone,
            "let pid = w.write_pid_extended(0, ParameterRole::Optional, false)?;",
        ),
    ] {
        let generated = codec(idl, "S", plan(dialect));
        assert!(
            generated.contains(header),
            "`{}` did not write `{header}`:\n{generated}",
            dialect.as_str()
        );
        assert!(generated.contains("match &self.b {"), "{generated}");
        assert!(generated.contains("w.finish_pid(pid);"), "{generated}");
        assert!(
            generated.contains("let b = if pid.length == 0 {"),
            "{generated}"
        );
        assert!(generated.contains("r.finish_pid(pid)?;"), "{generated}");
    }
    // And the default is one of the two, named rather than assumed.
    assert_eq!(
        codec(
            idl,
            "S",
            LayoutKey::data(Encoding::PlainCdr1, Endianness::Little)
        ),
        codec(idl, "S", plan(Dialect::default())),
        "`LayoutKey::data` and `Dialect::default` disagree about the parameter header"
    );
}

#[test]
fn an_optional_member_of_a_mutable_type_omits_the_whole_parameter() {
    // Inside a parameter list there is no presence marker: §7.4.1.2.3
    // says a member the stream does not carry is one the reader has to
    // account for, and for an `@optional` member the account is the
    // absence itself. So the header, the value and the patch all sit
    // inside the `Some` arm and the `None` arm writes nothing at all.
    let generated = codec(
        "@mutable struct M { @id(1) @optional octet b; @id(2) long c; };",
        "M",
        xcdr2_for(Extensibility::Mutable),
    );
    let some = generated
        .split("match &self.b {")
        .nth(1)
        .expect("the optional member is matched over");
    let arm = some.split("None =>").next().expect("a None arm");
    assert!(
        arm.contains("w.write_emheader(1,"),
        "the header escaped the Some arm:\n{generated}"
    );
    assert!(
        arm.contains("w.finish_emheader(emheader)?;"),
        "the patch escaped the Some arm:\n{generated}"
    );
}

/// An enumeration member writes through the enumeration's own codec.
///
/// This test used to require `w.write_u16(self.state);`, and that line
/// never compiled: the field is a `Mode` and not a `u16`, so any tree
/// with an enumeration as a struct member produced a generated file
/// rustc rejected. Nothing caught it because the assertion is over the
/// generated **text** — `crates/spatial-dds` was the first tree that
/// compiled one.
///
/// The octets are unchanged. `enumeration_encode_fn` takes its width
/// from the same `enumeration_bit_bound(ast, type_, key.enum_holder)`
/// this arm did, so `Mode::encode` writes the same `write_u16` — one
/// call further in, and over `@value` rather than over Rust's
/// discriminant, which the generator sets to the same numbers.
#[test]
fn an_enum_member_is_written_through_the_enumerations_own_codec() {
    let generated = codec(
        "@bit_bound(16) enum Mode { IDLE };
         @final struct S { Mode state; };",
        "S",
        xcdr2(),
    );
    assert!(generated.contains("self.state.encode(w)?;"), "{generated}");
    assert!(
        !generated.contains("w.write_u16(self.state)"),
        "the field is a `Mode`, and that line does not compile:\n{generated}"
    );
}

/// And the enumeration's own codec is where the width lives.
#[test]
fn the_enumerations_codec_writes_the_width_its_bit_bound_calls_for() {
    let generated = codec(
        "@bit_bound(16) enum Mode { IDLE, RUNNING };",
        "Mode",
        xcdr2(),
    );
    assert!(generated.contains("w.write_u16("), "{generated}");
}

#[test]
fn a_typedef_is_written_as_what_it_aliases() {
    let generated = codec(
        "typedef sequence<long> Ids;
         @final struct S { Ids numbers; };",
        "S",
        xcdr2(),
    );
    assert!(
        generated.contains("w.write_sequence(&self.numbers, |w, v| { w.write_i32(*v); Ok(()) })?;"),
        "{generated}"
    );
}

// -- the KeyHash ---------------------------------------------------------

#[test]
fn a_keyed_type_gets_a_key_hash_over_its_key_members_only() {
    let generated = codec(
        "@final struct Sensor { @key long id; double value; };",
        "Sensor",
        xcdr2(),
    );
    assert!(
        generated.contains("pub fn key_hash(&self) -> Result<[u8; 16], WireError> {"),
        "{generated}"
    );
    assert!(
        generated.contains("holder.write_i32(self.id);"),
        "{generated}"
    );
    assert!(
        !generated.contains("holder.write_f64(self.value);"),
        "a non-key member reached the holder:\n{generated}"
    );
}

#[test]
fn a_char_write_carries_the_question_mark_the_runtime_needs() {
    // `Writer::write_char` and `write_wchar_member` refuse a code point
    // they cannot carry — DDS-XTypes 1.3 §7.2.2.2.1.2.2 gives `char`
    // eight bits and §7.2.2.2.1.2.5 gives `wchar` the BMP. Emitting the
    // call without `?` compiles, because an unused `Result` is a
    // warning, and puts the narrowed character on the wire.
    //
    // `write_wchar_member` and not `write_wchar`: the framing is per
    // position, so there is no positionless entry point to emit. A
    // member takes the count octet under TAO's dialect and a collection
    // element does not — `Writer::write_wchar_element` is the other one,
    // and `an_array_of_wchar_writes_the_element_form` below is where it
    // is asserted.
    let generated = codec(
        "@final struct Label { char tag; wchar wide; long n; };",
        "Label",
        xcdr2(),
    );
    assert!(
        generated.contains("w.write_char(self.tag)?;"),
        "{generated}"
    );
    assert!(
        generated.contains("w.write_wchar_member(self.wide)?;"),
        "{generated}"
    );
    // The writes that cannot refuse are left alone: a `?` on a unit is
    // not Rust.
    assert!(generated.contains("w.write_i32(self.n);"), "{generated}");
}

/// A `wchar` inside a collection takes the element entry point.
///
/// The generated codec is one function per type and cannot ask at run
/// time where it was called from, so the position has to be in the name
/// the emitter writes. If this goes back to `write_wchar_member`, then
/// `--dialect tao` writes GIOP 1.2's count octet before every element of
/// a `wchar` array, which is four octets per element more than `tao_idl`
/// writes and is the whole of what that preset used to leave over.
#[test]
fn an_array_of_wchar_writes_the_element_form() {
    let generated = codec("@final struct Label { wchar many[3]; };", "Label", xcdr2());
    assert!(
        generated.contains("Writer::write_wchar_element"),
        "{generated}"
    );
    assert!(
        !generated.contains("write_wchar_member"),
        "an element took the member's entry point: {generated}"
    );
}

#[test]
fn a_key_hash_over_a_char_carries_it_too() {
    let generated = codec("@final struct Label { @key char tag; };", "Label", xcdr2());
    assert!(
        generated.contains("holder.write_char(self.tag)?;"),
        "{generated}"
    );
    assert!(
        generated.contains("Ok(holder.zero_padded())"),
        "{generated}"
    );
}

#[test]
fn a_type_with_no_key_gets_no_key_hash() {
    // A KeyHash over nothing is not a KeyHash; emitting one would give
    // every instance of that type the same handle.
    let generated = codec("@final struct S { long a; };", "S", xcdr2());
    assert!(!generated.contains("key_hash"), "{generated}");
}

#[test]
fn a_holder_of_sixteen_bytes_or_fewer_is_the_hash_and_a_longer_one_is_md5ed() {
    // The plan decides, because the size is the plan's answer.
    let short = codec("@final struct A { @key long id; };", "A", xcdr2());
    assert!(short.contains("holder.zero_padded()"), "{short}");

    let long = codec(
        "@final struct B { @key long a; @key long second; @key long c; @key long d; @key long e; };",
        "B",
        xcdr2(),
    );
    assert!(long.contains("holder.md5()"), "{long}");
}

/// The holder is big-endian and capped at four, whatever the sample is.
///
/// Two peers that disagree about either compute different hashes for the
/// same sample, and neither can tell: DDSI-RTPS 2.5 §9.6.3.8 makes a
/// KeyHash sixteen opaque octets, so a wrong one shows up only as an
/// instance that never matches. XTypes 1.3 §7.6.8 step 4 serialises the
/// holder as PLAIN_CDR2 whatever version the sample goes out in, and
/// §7.4.1 gives PLAIN_CDR2 a `MAXALIGN` of four — the `Writer`'s own
/// default is eight, which put a `@key double` behind a `@key long` at
/// offset eight where the plan that sized the holder had it at four.
/// An aggregate `@key` member is handed the holder by reference.
///
/// Every other write is a method call on the sink and auto-references;
/// `MemberWrite::Nested` hands it over as an argument — `self.k.encode(sink)`
/// — and there Rust does not. The holder is a local `Writer`, so the emitter
/// wrote `self.k.encode(holder)` and the generated file did not compile:
/// `error[E0308]: expected &mut Writer, found Writer`, in both
/// `key_holder_be` and `key_hash`, for every keyed type whose key is a struct.
///
/// Nothing in this crate noticed, because rendering a string is not
/// compiling it. `rust_corpus_compiles` is the target that would have, and
/// the corpus had no case with an aggregate key.
#[test]
fn an_aggregate_key_member_takes_the_holder_by_reference() {
    let generated = codec(
        "@final struct Inner { long a; long b; };\n\
         @final struct S { @key Inner nested; long v; };",
        "S",
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    );
    assert!(
        generated.contains("self.nested.encode(&mut holder)?"),
        "the nested key write must take a reference:\n{generated}"
    );
    assert!(
        !generated.contains("self.nested.encode(holder)?"),
        "passing the local Writer by value does not compile:\n{generated}"
    );
    // The sink of the sample's own encoder is already a `&mut Writer`, so
    // it must NOT grow a second reference.
    assert!(
        generated.contains("self.nested.encode(w)?"),
        "the sample encoder's sink is already a reference:\n{generated}"
    );
}

#[test]
fn the_holder_is_big_endian_and_plain_cdr2_aligned_whatever_the_sample_is() {
    for encoding in [Encoding::PlainCdr2, Encoding::PlainCdr1] {
        let generated = codec(
            "@final struct S { @key long id; @key double at; };",
            "S",
            LayoutKey::data(encoding, Endianness::Little),
        );
        assert!(
            generated.contains(
                "let mut holder = Writer::with_max_align(zerodds_wire::Endianness::Big, 4);"
            ),
            "{generated}"
        );
        // Both functions, over one set of writes: the holder's octets
        // and the hash over them cannot be planned differently.
        assert!(
            generated.contains("pub fn key_holder_be(&self) -> Result<Vec<u8>, WireError>"),
            "{generated}"
        );
        assert_eq!(
            generated
                .matches("Writer::with_max_align(zerodds_wire::Endianness::Big, 4)")
                .count(),
            2,
            "{generated}"
        );
    }
}

#[test]
fn an_unbounded_key_is_md5ed_because_its_length_is_not_known_until_the_sample_is() {
    let generated = codec("@final struct S { @key string name; };", "S", xcdr2());
    assert!(generated.contains("holder.md5()"), "{generated}");
}

#[test]
fn an_appendable_tail_the_writer_did_not_have_takes_its_default() {
    // §7.2.4.4.8, Table 19: "Each non-optional member in a T1 object
    // that is not present in the T2 object takes its default value."
    // Under encoding version 2 the DHEADER says where the writer's
    // members stopped; under version 1 §7.2.4.2 leaves an appendable
    // type undelimited and the payload's end is what Cyclone, RTI and
    // Fast DDS all use instead — `evo-appendable-tail`, 2026-08-09.
    let delimited = codec(
        "@appendable struct S { long a; string b; short c; };",
        "S",
        LayoutKey::data(Encoding::DelimitedCdr, Endianness::Little),
    );
    assert!(
        delimited.contains("let end = r.read_dheader()? + r.position();"),
        "{delimited}"
    );
    assert!(
        delimited.contains("if r.position() >= end {"),
        "{delimited}"
    );
    // Table 9: `""` for a string and 0 for an integer, and not whatever
    // the language calls a zero.
    assert!(delimited.contains("String::new()"), "{delimited}");

    let undelimited = codec(
        "@appendable struct S { long a; short c; };",
        "S",
        LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
    );
    assert!(
        undelimited.contains("if r.remaining() == 0 {"),
        "{undelimited}"
    );

    // A `@final` type gets neither: Table 19 requires the same set of
    // member IDs on both sides, so a short stream is a type mismatch.
    let final_ = codec(
        "@final struct S { long a; short c; };",
        "S",
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    );
    assert!(!final_.contains("r.remaining() == 0"), "{final_}");
}

#[test]
fn an_appendable_tail_takes_an_explicit_default_over_table_nine() {
    // §7.2.2.4.4.4.9: the implicit default is the type's only where the
    // declaration gives no explicit one.
    let generated = codec(
        "@appendable struct S { long a; @default(42) long c; };",
        "S",
        LayoutKey::data(Encoding::DelimitedCdr, Endianness::Little),
    );
    assert!(generated.contains("42"), "{generated}");
}

#[test]
fn an_appendable_tail_of_an_enumeration_takes_its_first_literal() {
    // Table 9's `ENUM_TYPE` row is "the first literal in the
    // enumeration", which is not the language's zero: `@value` may have
    // moved it.
    let generated = codec(
        "enum Colour { @value(7) AMBER, @value(0) RED };
         @appendable struct S { long a; Colour c; };",
        "S",
        LayoutKey::data(Encoding::DelimitedCdr, Endianness::Little),
    );
    assert!(generated.contains("Colour::AMBER"), "{generated}");
}

// -- `try_construct` ------------------------------------------------------

#[test]
fn a_bounded_member_narrows_after_the_whole_value_is_off_the_wire() {
    // §7.2.2.7. The bound is not on the wire, so the read is the same
    // read either way and the narrowing is a step after it. A reader
    // that stopped at its own bound would leave the rest in the stream
    // and put every following member at the wrong offset — OpenDDS
    // consumes all twelve characters before it defaults or truncates,
    // and the members after it come back correct.
    let generated = codec(
        "@mutable struct M { @id(1) @try_construct(TRIM) string<5> text; @id(2) long b; };",
        "M",
        xcdr2_for(Extensibility::Mutable),
    );
    assert!(
        generated.contains("let text = r.read_string()?;"),
        "{generated}"
    );
    assert!(
        generated.contains("match r.fit_string(text, 5, TryConstruct::Trim)? {"),
        "{generated}"
    );
    assert!(
        generated.contains("Narrowed::Value(text) => Ok(text),"),
        "{generated}"
    );
}

#[test]
fn an_absent_annotation_is_discard_and_a_bare_one_is_use_default() {
    // §7.2.2.7 makes an absent annotation DISCARD; §7.3.1.2.1.12
    // declares the annotation's own parameter `default USE_DEFAULT`, so
    // a valueless one is USE_DEFAULT. Reading the second as the first
    // would drop samples the author asked to keep. OpenDDS is the
    // witness for both.
    let absent = codec("@final struct S { string<5> m; };", "S", xcdr2());
    assert!(
        absent.contains("r.fit_string(m, 5, TryConstruct::Discard)?"),
        "{absent}"
    );
    let bare = codec(
        "@final struct S { @try_construct string<5> m; };",
        "S",
        xcdr2(),
    );
    assert!(
        bare.contains("r.fit_string(m, 5, TryConstruct::UseDefault)?"),
        "{bare}"
    );
    // And USE_DEFAULT names the value, which is Table 9's for the type.
    assert!(
        bare.contains("Narrowed::Default => Ok(String::new())"),
        "{bare}"
    );
}

#[test]
fn an_explicit_default_beats_table_nine_at_the_member_position() {
    // §7.2.2.4.4.4.9: "An explicit definition associates a default
    // value with the member at the time where the aggregated type is
    // defined." Only the member position can carry one — a collection
    // element has no declarator to write it on.
    let generated = codec(
        "@final struct S { @try_construct(USE_DEFAULT) @default(\"hi\") string<5> m; };",
        "S",
        xcdr2(),
    );
    assert!(
        generated.contains("Narrowed::Default => Ok(\"hi\".to_string())"),
        "{generated}"
    );
}

#[test]
fn an_element_and_its_collection_narrow_separately() {
    // §7.3.1.2.1.12.4 gives the element position its own behaviour and
    // Table 17 keeps it apart from the collection's own bound: the
    // sequence's length overflow and an element's bound overflow are
    // two different failures. OpenDDS's generated element loop
    // implements exactly that split.
    let generated = codec(
        "typedef string<5> String5;
         @final struct S { sequence<@try_construct(TRIM) String5, 10> m; };",
        "S",
        xcdr2(),
    );
    assert!(
        generated.contains("r.fit_string(m, 5, TryConstruct::Trim)?"),
        "the element's own behaviour did not reach it:\n{generated}"
    );
    assert!(
        generated.contains("r.fit_sequence(m, 10, TryConstruct::Discard)?"),
        "the collection's own bound was dropped:\n{generated}"
    );
}

/// The enumeration codec takes each enumerator's integer from
/// `zerodds_ast_layout::enumerator_wire_value`, not from the declared
/// `@value`.
///
/// The Rust column's `encode` is a `match` naming each enumerator's
/// integer, and `decode` matches an incoming holder against the same
/// list — so those integers are what leaves and enters the process. They
/// were `Ast::effective_enumerator_value`'s, which answers `200` under
/// both settings, and the column therefore wrote `200` under a plan that
/// asked for the ordinal. No corpus row reaches this code, so nothing at
/// run time said so; `crates/wire-probe/tests/enumerator_value.rs::the_sites_go_through_the_crossing`
/// is what caught it.
#[test]
fn the_enumeration_codec_takes_the_integer_the_plan_gives_it() {
    const IDL: &str = "enum Colour { @value(200) RED, @value(9) BLUE };";

    let declared = codec(
        IDL,
        "Colour",
        xcdr2().with_enumerator_value(EnumeratorValue::Declared),
    );
    assert!(
        declared.contains("200"),
        "the declared value did not reach the codec:\n{declared}"
    );

    let ordinal = codec(
        IDL,
        "Colour",
        xcdr2().with_enumerator_value(EnumeratorValue::Ordinal),
    );
    assert!(
        !ordinal.contains("200"),
        "`EnumeratorValue::Ordinal` still wrote the declared `@value(200)`:\n{ordinal}"
    );
    assert_ne!(
        declared, ordinal,
        "the two settings produced the same codec, so the axis reaches nothing"
    );
}

/// A union case label that does not evaluate is refused, not skipped.
///
/// Skipping it was silent in both directions. The decoder got no arm for
/// that discriminator and fell through to the `default:` case or to the
/// refusal, answering a branch nobody sent; the encoder wrote the value
/// reserved for `default:` — `case UNDECLARED:` names a constant that is
/// not in the model, so `Ast::eval_integral` returns `Unresolved` and the
/// branch used to disappear from the generated codec with nothing said.
#[test]
fn a_union_case_label_that_does_not_evaluate_is_refused() {
    let error = codec_error(
        "union U switch (long) {
           case UNDECLARED: long a;
           default: short b;
         };",
        "U",
        xcdr2(),
    );
    assert!(
        matches!(
            error,
            zerodds_lang_rules::encode::EncodeError::NoWriter {
                kind: "a union case whose label does not evaluate",
                ..
            }
        ),
        "{error:?}"
    );
}

/// A `wstring` member is written in the cell the **key** names, and the
/// cell is in the emitted call.
///
/// D3 for as long as it stood: a `wstring` is a UInt32 length and UTF-16
/// code units, this column had `write_string` and nothing else, and the
/// generator's `TypeKind::String { bound, .. }` dropped the width on the
/// floor — so a `wstring` member would have gone out as UTF-8 with a
/// terminator under a length counting octets, a different value of a
/// different type with no error anywhere. It was refused by name
/// instead.
///
/// The runtime has the writer now, and what replaced the refusal is not
/// one form but eight: C1 (what the length counts) × C2 (whether a
/// terminating NUL joins the run and is counted) × C3 (how wide one code
/// unit is), plus C4, `WstringUnitOrder`, on its own axis. All four come
/// out of the [`LayoutKey`] and go into the call, the way `MapDelimiter`
/// comes out of the key and picks `write_delimited_sequence`. A runtime
/// default would be a second place the wire format lives.
#[test]
fn a_wstring_member_is_written_in_the_cell_the_key_names() {
    let specification = codec("@final struct S { wstring name; };", "S", xcdr2());
    assert!(
        specification.contains(
            "w.write_wstring(&self.name, \
             zerodds_wire::WstringLengthPolicy::new(\
             zerodds_wire::WstringLengthCounts::Octets, \
             zerodds_wire::WstringTerminator::Absent, \
             zerodds_wire::WstringUnitWidth::Two), \
             zerodds_wire::WstringUnitOrder::Stream, \
             zerodds_wire::WirePosition::Member)?"
        ),
        "the default cell is not §7.4.1.1.2's:\n{specification}"
    );
    assert!(
        specification.contains("r.read_wstring("),
        "the read side did not take one:\n{specification}"
    );

    // RTI's XCDR1 cell and Cyclone's byte order, on the same IDL. A
    // column that had baked the specification's cell into the runtime
    // would emit the same text for both.
    let rti = codec(
        "@final struct S { wstring name; };",
        "S",
        xcdr2()
            .with_wstring_length(WstringLengthPolicy::CHARACTER_COUNT_TERMINATED)
            .with_wstring_unit_order(WstringUnitOrder::HostUnswapped),
    );
    assert!(
        rti.contains(
            "zerodds_wire::WstringLengthCounts::Characters, \
             zerodds_wire::WstringTerminator::Present, \
             zerodds_wire::WstringUnitWidth::Four"
        ),
        "the key's cell did not reach the call:\n{rti}"
    );
    assert!(
        rti.contains("zerodds_wire::WstringUnitOrder::HostUnswapped"),
        "the key's unit order did not reach the call:\n{rti}"
    );
    assert_ne!(
        specification, rti,
        "two cells that produce different octets produced the same body"
    );
}

/// Every one of the eight cells reaches the emitted call, and no two of
/// them emit the same body.
///
/// The sweep is the whole product and not the measured five. A cell that
/// no emitter can name is a cell no run can measure, which is exactly
/// what the enumeration of named forms cost: two ROS type supports wrote
/// a form this project had no way to express.
#[test]
fn every_cell_of_the_wstring_product_reaches_the_emitted_call() {
    let mut bodies = Vec::new();
    for cell in WstringLengthPolicy::ALL_CELLS {
        let generated = codec(
            "@final struct S { wstring name; };",
            "S",
            xcdr2().with_wstring_length(cell),
        );
        assert!(
            generated.contains("w.write_wstring(&self.name, "),
            "cell {} emitted no write:\n{generated}",
            cell.as_str()
        );
        bodies.push(generated);
    }
    bodies.sort_unstable();
    bodies.dedup();
    assert_eq!(
        bodies.len(),
        8,
        "eight cells did not produce eight distinct bodies"
    );
}

/// A `wstring` element is written at the **element** position, and a
/// `wstring<N>` bound goes to `fit_wstring`.
///
/// Two properties in one case because both are substitutions the shape
/// alone does not carry. The element position is rewritten where the
/// `wchar` element's framing is rewritten — a caller that could leave it
/// alone would take the member's answer for an element by not asking,
/// which is how a `wchar` element got a member's framing through five
/// emitters. And the bound counts UTF-16 code units and not octets, so
/// it goes to a different narrowing: `wstring<5>` is a five-unit buffer
/// in every measured mapping, and `fit_string` would have counted the
/// UTF-8 octets of the value instead.
#[test]
fn a_wstring_element_takes_the_element_position_and_a_bound_takes_fit_wstring() {
    let generated = codec(
        "@final struct S { wstring<5> one; sequence<wstring> many; };",
        "S",
        xcdr2(),
    );
    assert!(
        generated.contains("w.write_wstring_bounded(&self.one, 5, "),
        "the bound did not reach the call:\n{generated}"
    );
    assert!(
        generated.contains("r.fit_wstring(one, 5, TryConstruct::Discard)?"),
        "the bound went to the octet-counting narrowing:\n{generated}"
    );
    assert!(
        generated.contains("|w, v| w.write_wstring(v, ")
            && generated.contains("zerodds_wire::WirePosition::Element)"),
        "the element did not take the element position:\n{generated}"
    );
    // And the member's own call still says `Member`, so the rewrite is a
    // substitution and not a change of default.
    assert!(
        generated.contains("zerodds_wire::WirePosition::Member)"),
        "the member position went missing:\n{generated}"
    );
}

/// A kind with no codec of its own is refused by name. Asked for one,
/// `rust_codec` emitted `impl Count { … }` for `typedef long Count;` —
/// E0390 on an alias of a primitive, and an `encode` that wrote nothing.
#[test]
fn an_alias_has_no_codec_of_its_own_and_says_so() {
    let error = codec_error(
        "typedef long Count;",
        "Count",
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    );
    assert!(
        matches!(
            &error,
            zerodds_lang_rules::encode::EncodeError::NoWriter { member, .. } if member == "Count"
        ),
        "{error:?}"
    );
}

/// A nested `@mutable` type in two samples: `Small`'s own type stays
/// within RTI's short-header step and `Large`'s passes it.
///
/// The same IDL as `tools/idlc/tests/dialect_byte_equality.rs`'s
/// `RTI_STEP_IDL` and `crates/lang-rules/src/emit/encode.rs`'s `Inner`
/// row: `@mutable struct Inner { long a; sequence<long, 100> s; }` is
/// small on its own and is nevertheless framed with `PID_EXTENDED` as a
/// member of a sample past the step. Measured on rtiddsgen's
/// `PRESTypePlugin_interpretedSerialize`, XCDR1 little endian, and
/// modelled by
/// `zerodds_ast_layout::key::PlCdr1HeaderPolicy::ExtendedForConstructedInLargeSamples`.
const RTI_STEP: &str = "\
@mutable struct Inner { long a; sequence<long, 100> s; };
@mutable struct Small { long id; Inner part; sequence<octet, 100> big; };
@mutable struct Large { long id; Inner part; sequence<octet, 65532> big; };";

/// RTI's data key for XCDR1.
fn rti_xcdr1() -> LayoutKey {
    LayoutKey::in_dialect(
        Dialect::Rti,
        Encoding::PlCdr1,
        Endianness::Little,
        zerodds_ast_layout::key::Purpose::Data,
    )
}

/// The same key under another dialect, so a test can hold everything else
/// fixed.
fn in_dialect_xcdr1(dialect: Dialect) -> LayoutKey {
    LayoutKey::in_dialect(
        dialect,
        Encoding::PlCdr1,
        Endianness::Little,
        zerodds_ast_layout::key::Purpose::Data,
    )
}

/// Under `--dialect rti` and XCDR1 the header form is the **sample's**
/// answer, so a generated type cannot bake it in: the encoder is split
/// into a body that takes `zd_extended` and a public entry that resolves
/// the sample's own step once and hands it down.
///
/// The three things a body that did not split would miss, all visible in
/// the text: the flag parameter, the run-time branch over the two
/// parameter-header writers, and the nested call that carries the flag
/// on. The fourth — the entry's constant — is the whole of the decision,
/// and it is the one that differs between `Small` and `Large`.
#[test]
fn under_rti_the_header_form_is_chosen_at_run_time_from_the_samples_own_step() {
    let small = codec(RTI_STEP, "Small", rti_xcdr1());
    assert!(
        small.contains("pub fn encode_zd_extended(&self, w: &mut Writer, zd_extended: bool)"),
        "the encoder body does not take the sample's decision:\n{small}"
    );
    assert!(
        small.contains("let pid = if zd_extended {")
            && small.contains("w.write_pid_extended(1, ParameterRole::Member, false)?")
            && small.contains("w.write_pid(1, ParameterRole::Member, false)?"),
        "a constructed member is not framed at run time:\n{small}"
    );
    assert!(
        small.contains("self.part.encode_zd_extended(w, zd_extended)?;"),
        "the nested call does not carry the flag on:\n{small}"
    );
    // `id` is a `long`: `keeps_short_header` counts a primitive among the
    // types that keep the short form however large the sample is, so its
    // header is not branched on.
    assert!(
        small.contains("let pid = w.write_pid(0, ParameterRole::Member, false)?;"),
        "a primitive member was given a run-time branch:\n{small}"
    );
    // `Small`'s own type stays within the step, so its entry says so.
    assert!(
        small.contains("self.encode_zd_extended(w, false)"),
        "`Small`'s entry did not resolve its own step:\n{small}"
    );

    let large = codec(RTI_STEP, "Large", rti_xcdr1());
    assert!(
        large.contains("self.encode_zd_extended(w, true)"),
        "`Large`'s entry did not resolve the step it passes:\n{large}"
    );
    // And the nested type keeps the short form as a root of its own, which
    // is the case the same step would frame differently one level up.
    let inner = codec(RTI_STEP, "Inner", rti_xcdr1());
    assert!(
        inner.contains("self.encode_zd_extended(w, false)"),
        "`Inner`'s own entry framed it as if the sample were large:\n{inner}"
    );
}

/// A dialect whose PL_CDR1 header form the plan fixes generates exactly
/// the code it generated before: one `encode` per codec, the form the
/// plan named, and no flag anywhere.
///
/// The property is not only "no `zd_extended`": an emitter that added
/// the split everywhere would pass that and still change every dialect's
/// output, so the names and the calls are pinned too.
#[test]
fn a_dialect_with_a_fixed_header_form_generates_no_flag_at_all() {
    for dialect in [Dialect::Cyclone, Dialect::FastDds, Dialect::Specification] {
        let text = codec(RTI_STEP, "Small", in_dialect_xcdr1(dialect));
        assert!(
            !text.contains("zd_extended"),
            "{dialect:?} grew the per-sample flag:\n{text}"
        );
        assert!(
            text.contains("pub fn encode(&self, w: &mut Writer) -> Result<(), WireError>"),
            "{dialect:?} lost the entry:\n{text}"
        );
        // Cyclone takes `PID_EXTENDED` for every member and the other two
        // take the short form where they can; either way the plan named
        // it and nothing branches.
        assert!(
            !text.contains("let pid = if zd_extended"),
            "{dialect:?} grew a run-time branch on the header form:\n{text}"
        );
        assert!(
            text.contains("self.part.encode(w)?;"),
            "{dialect:?} lost the plain nested call:\n{text}"
        );
    }
}

/// The key holder is not a data sample and has no root to resolve the
/// step from, so it passes the constant `false` — the short form, as
/// `zerodds_ast_layout::xcdr::extended_as_constructed` keeps it for the
/// key purpose and as `crate::emit::encode`'s `nested_call_with` passes
/// it for the same reason.
#[test]
fn the_key_holder_under_rti_states_the_short_form() {
    let text = codec(
        "\
@final struct Head { long p; };
@mutable struct Keyed {
    @key Head first;
    @key long n;
    sequence<octet, 65532> blob;
};",
        "Keyed",
        rti_xcdr1(),
    );
    assert!(
        text.contains("self.first.encode_zd_extended(&mut holder, false)?;"),
        "the key holder did not state the short form:\n{text}"
    );
}

/// A member id past sixteen bits takes the extended PL_CDR1 header with
/// the whole id (XTypes 1.3 §7.4.1.2.2). The plan's 16-bit `pid` saturates
/// at `u16::MAX` there, and the codec wrote that: every such member went
/// out as member 65535.
#[test]
fn a_member_id_past_sixteen_bits_is_written_whole_in_the_extended_header() {
    let text = codec(
        "@mutable struct H { @id(0x12345) long a; };",
        "H",
        LayoutKey::data(Encoding::PlCdr1, Endianness::Little),
    );
    assert!(
        text.contains("write_pid_extended(74565"),
        "the extended header names member 0x12345:\n{text}"
    );
    assert!(!text.contains("65535"), "no saturated id:\n{text}");
}
