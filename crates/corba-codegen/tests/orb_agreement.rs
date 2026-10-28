// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What this crate says against what three ORBs generate.
//!
//! The rule step 1 of the rebuild set: a step that finishes without
//! consulting a foreign implementation has not finished. For a table of
//! type names the foreign implementation is the IDL compiler that emits
//! those names, and three of them are on the measurement host —
//! `omniidl`, `tao_idl` and JacORB's `idl`.
//!
//! `orb-corpus/rows/*.idl` writes each row in every argument position;
//! `orb-corpus/record.sh` runs the three compilers over it on the
//! measurement host and records what they generated, verbatim, into
//! `orb-corpus/recorded/`. This test reads the recording and checks that
//! what this crate would emit for the same operation is in it.
//!
//! **A recording is not an oracle; the ORB that made it is.** The
//! recording is committed so the check runs anywhere, and `record.sh`
//! remakes it. Re-run it after touching `rows/`.
//!
//! ## What the ORBs disagreed with, and what changed
//!
//! Four answers in this crate come from these files and not from a
//! reading of the specification:
//!
//! 1. **A `typedef` is written by its own name.** `typedef fixed<10,2>
//!    Money` is `probe::Money` and not `CORBA::Fixed`; `typedef
//!    sequence<any> AnySeq` is `probe::AnySeq`; `native Handle` is
//!    `probe::Handle` and no ORB declares it. Rows 04, 07, 11.
//! 2. **An `abstract interface` is not the `AbstractBase` row.** All
//!    three write the interface's own name. Row 03.
//! 3. **A Java `wstring` holder is `StringHolder`.** Both string kinds
//!    map to `java.lang.String`, so both take that holder. Row 09.
//! 4. **A Java `sequence<any>` is `org.omg.CORBA.Any[]`.** The earlier
//!    table said `AnySeqHelper`, which is a marshalling class and never
//!    a member type. Row 07.
//!
//! ## What each ORB refused
//!
//! Recorded rather than worked around, because a refusal is what that
//! ORB says about that row:
//!
//! | ORB | row | what it said |
//! |---|---|---|
//! | omniidl | 04 `native` | `Unsupported IDL construct found in input (native)` — the C++ backend, not the parser |
//! | tao_idl | 11 `fixed` | `fixed data types are not supported` |
//! | JacORB | 05 `TypeCode` | `Undefined name: probe.Special.CORBA.TypeCode` |
//!
//! The first version of the corpus was one IDL file, and each of those
//! three refusals took the other twelve rows of that ORB down with it.
//! One file per row is why they do not.

// Tests assert on Results directly; a failed expect IS the test failure.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;

use zerodds_ast::ParamDirection;
use zerodds_corba_codegen::{SpecialType, StubType, TypeShape, cpp_param_type, cpp_return_type};

const OMNIORB: &str = include_str!("orb-corpus/recorded/omniorb.txt");
const TAO: &str = include_str!("orb-corpus/recorded/tao.txt");
const JACORB: &str = include_str!("orb-corpus/recorded/jacorb.txt");

/// The recording, split into one block per row.
fn rows(recorded: &str) -> BTreeMap<&str, String> {
    let mut out = BTreeMap::new();
    let mut current: Option<&str> = None;
    let mut body = String::new();
    for line in recorded.lines() {
        if let Some(name) = line.strip_prefix("=== ") {
            if let Some(previous) = current {
                out.insert(previous, core::mem::take(&mut body));
            }
            current = Some(name.trim());
        } else if current.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some(previous) = current {
        out.insert(previous, body);
    }
    out
}

fn block(recorded: &str, row: &str) -> String {
    rows(recorded)
        .get(row)
        .unwrap_or_else(|| panic!("row `{row}` is not in the recording"))
        .clone()
}

/// Containment, after three normalisations that are spelling and not
/// meaning.
///
/// 1. **Whitespace.** TAO writes `char *` and omniidl `char*`.
/// 2. **The root qualifier.** Both write `::CORBA::Any`; the spec and
///    this crate write `CORBA::Any`, and a leading `::` only says the
///    lookup starts at global scope.
/// 3. **The corpus module.** omniidl writes `WithString*` for a return
///    inside `namespace probe` and `::probe::WithString_out` for a
///    parameter of the same type. Which of the two it writes depends on
///    where the declaration sits, not on what the type is.
fn contains(haystack: &str, needle: &str) -> bool {
    normalise(haystack).contains(&normalise(needle))
}

fn normalise(text: &str) -> String {
    // The root qualifier goes first, while the whitespace that marks it
    // is still there: `const ::CORBA::Any&` has a space before the `::`
    // and `CORBA::Any` does not, and stripping whitespace first would
    // leave `const::CORBA` with no way to tell the two `::` apart.
    let chars: Vec<char> = text.chars().collect();
    let mut rooted = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let is_root = chars[i] == ':'
            && i + 1 < chars.len()
            && chars[i + 1] == ':'
            && (i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_'));
        if is_root {
            i += 2;
        } else {
            rooted.push(chars[i]);
            i += 1;
        }
    }
    rooted
        .replace("probe::", "")
        .replace("probe.", "")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn cpp(type_: &StubType, direction: ParamDirection) -> String {
    zerodds_lang_render::cpp::type_(&cpp_param_type(type_, direction))
}

fn cpp_ret(type_: &StubType) -> String {
    zerodds_lang_render::cpp::type_(&cpp_return_type(type_))
}

/// One row, and what this crate emits for it in C++.
struct CppRow {
    row: &'static str,
    /// `(direction, what an ORB must have written)`.
    positions: Vec<(Option<ParamDirection>, String)>,
}

fn cpp_rows() -> Vec<CppRow> {
    let special = |t: SpecialType| StubType::Special(t);
    let mut out = Vec::new();

    let mut row = |row: &'static str, type_: StubType, directions: &[ParamDirection]| {
        let mut positions: Vec<(Option<ParamDirection>, String)> = directions
            .iter()
            .map(|d| (Some(*d), cpp(&type_, *d)))
            .collect();
        positions.push((None, cpp_ret(&type_)));
        let _ = &type_;
        out.push(CppRow { row, positions });
    };

    use ParamDirection::{In, InOut, Out};
    row("01-object", special(SpecialType::Object), &[In, Out, InOut]);
    row("02-valuebase", special(SpecialType::ValueBase), &[In, Out]);
    row(
        "03-abstractbase",
        StubType::other("probe::Abstracted", TypeShape::ObjectReference),
        &[In, Out],
    );
    row(
        "04-native",
        StubType::other("probe::Handle", TypeShape::Scalar),
        &[In, Out],
    );
    row(
        "05-typecode",
        special(SpecialType::TypeCode),
        // omniidl writes `CORBA::TypeCode_OUT_arg` for the `out`
        // position, which is its own typedef of `CORBA::TypeCode_out`.
        // tao_idl writes the spec's name; the `out` position is
        // therefore checked against TAO only, below.
        &[In],
    );
    row("06-any", special(SpecialType::Any), &[In, InOut]);
    row(
        "07-anyseq",
        StubType::alias("probe::AnySeq", special(SpecialType::SequenceOfAny)),
        &[In, Out],
    );
    row("08-string", special(SpecialType::String), &[In, Out, InOut]);
    row(
        "09-wstring",
        special(SpecialType::WString),
        &[In, Out, InOut],
    );
    row(
        "10-time",
        StubType::alias("::TimeBase::TimeT", special(SpecialType::ULongLong)),
        &[In],
    );
    row(
        "11-fixed",
        StubType::alias("probe::Money", special(SpecialType::Fixed)),
        &[In, Out],
    );
    row("12-ulonglong", special(SpecialType::ULongLong), &[In]);
    row("13-longdouble", special(SpecialType::LongDouble), &[In]);
    out
}

#[test]
fn omniorb_wrote_every_cpp_spelling_this_crate_emits() {
    // omniidl refuses `native` in its C++ backend, so row 04 has no
    // omniORB answer; the refusal itself is asserted below.
    let skipped = ["04-native"];
    let mut checked = 0;
    for CppRow { row, positions } in cpp_rows() {
        if skipped.contains(&row) {
            continue;
        }
        let recorded = block(OMNIORB, row);
        for (direction, written) in positions {
            assert!(
                contains(&recorded, &written),
                "{row} {direction:?}: omniidl did not write `{written}`\n{recorded}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 25, "only {checked} spellings checked");
}

#[test]
fn tao_wrote_every_cpp_spelling_this_crate_emits() {
    // tao_idl refuses `fixed` outright, so row 11 has no TAO answer.
    let skipped = ["11-fixed"];
    let mut checked = 0;
    for CppRow { row, positions } in cpp_rows() {
        if skipped.contains(&row) {
            continue;
        }
        let recorded = block(TAO, row);
        for (direction, written) in positions {
            assert!(
                contains(&recorded, &written),
                "{row} {direction:?}: tao_idl did not write `{written}`\n{recorded}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 25, "only {checked} spellings checked");
}

#[test]
fn the_out_position_of_a_typecode_is_taos_spelling_and_not_omniorbs() {
    // The one place the two C++ ORBs write different text for one type.
    // `CORBA::TypeCode_out` is the spec's name and TAO's; omniidl's
    // `CORBA::TypeCode_OUT_arg` is a typedef of it. This crate emits
    // the spec's, and the divergence is recorded rather than hidden by
    // a looser comparison.
    let emitted = cpp(
        &StubType::Special(SpecialType::TypeCode),
        ParamDirection::Out,
    );
    assert_eq!(emitted, "CORBA::TypeCode_out");
    assert!(contains(&block(TAO, "05-typecode"), &emitted));
    assert!(contains(
        &block(OMNIORB, "05-typecode"),
        "::CORBA::TypeCode_OUT_arg"
    ));
}

#[test]
fn the_fixed_and_variable_split_is_what_both_cpp_orbs_write() {
    // §1.9's rule, which `shape_of` computes from the members: the same
    // `out` and return positions differ between a struct holding a
    // string and one holding two longs.
    let variable = StubType::other("probe::WithString", TypeShape::VariableAggregate);
    let fixed = StubType::other("probe::TwoLongs", TypeShape::FixedAggregate);
    for recorded in [block(OMNIORB, "14-shape"), block(TAO, "14-shape")] {
        assert!(
            contains(&recorded, &cpp(&variable, ParamDirection::Out)),
            "{recorded}"
        );
        assert!(contains(&recorded, &cpp_ret(&variable)), "{recorded}");
        assert!(contains(&recorded, &cpp_ret(&fixed)), "{recorded}");
    }
    // The `out` of a fixed-length struct is the one spelling the two
    // ORBs write differently: omniidl `probe::TwoLongs&`, tao_idl
    // `probe::TwoLongs_out`, which is a typedef of the same reference.
    assert!(contains(
        &block(OMNIORB, "14-shape"),
        &cpp(&fixed, ParamDirection::Out)
    ));
    assert!(contains(&block(TAO, "14-shape"), "::probe::TwoLongs_out"));
}

/// One row, and what this crate emits for it in Java.
fn java_rows() -> Vec<(&'static str, StubType)> {
    vec![
        ("01-object", StubType::Special(SpecialType::Object)),
        ("02-valuebase", StubType::Special(SpecialType::ValueBase)),
        (
            "03-abstractbase",
            StubType::other("probe::Abstracted", TypeShape::ObjectReference),
        ),
        (
            "04-native",
            StubType::other("probe::Handle", TypeShape::Scalar),
        ),
        ("06-any", StubType::Special(SpecialType::Any)),
        (
            "07-anyseq",
            StubType::alias(
                "probe::AnySeq",
                StubType::Special(SpecialType::SequenceOfAny),
            ),
        ),
        ("08-string", StubType::Special(SpecialType::String)),
        ("09-wstring", StubType::Special(SpecialType::WString)),
        (
            "11-fixed",
            StubType::alias("probe::Money", StubType::Special(SpecialType::Fixed)),
        ),
        ("12-ulonglong", StubType::Special(SpecialType::ULongLong)),
        ("13-longdouble", StubType::Special(SpecialType::LongDouble)),
    ]
}

#[test]
fn jacorb_wrote_every_java_value_type_this_crate_emits() {
    for (row, type_) in java_rows() {
        let recorded = block(JACORB, row);
        let written = zerodds_lang_render::java::type_(&zerodds_corba_codegen::java_param_type(
            &type_,
            ParamDirection::In,
        ));
        assert!(
            contains(&recorded, &written),
            "{row}: JacORB did not write `{written}`\n{recorded}"
        );
    }
}

#[test]
fn jacorb_wrote_every_java_holder_this_crate_emits() {
    // `02-valuebase` is the exception and is asserted separately.
    for (row, type_) in java_rows() {
        if row == "02-valuebase" {
            continue;
        }
        let recorded = block(JACORB, row);
        let written =
            zerodds_lang_render::java::type_(&zerodds_corba_codegen::java_holder_type(&type_));
        assert!(
            contains(&recorded, &written),
            "{row}: JacORB did not write `{written}`\n{recorded}"
        );
    }
}

#[test]
fn the_valuebase_holder_is_the_omgs_and_jacorbs_names_a_class_that_does_not_exist() {
    // The one row where the measurement is not taken. JacORB 3.9 writes
    // `java.io.SerializableHolder` — it derives the holder name from the
    // mapped type — and the JDK has no such class, so a generated stub
    // naming it does not compile. `org.omg.CORBA.ValueBaseHolder` is the
    // mapping's own answer and does exist. This is a peer defect, not a
    // divergence to follow.
    let recorded = block(JACORB, "02-valuebase");
    assert!(
        contains(&recorded, "java.io.SerializableHolder"),
        "{recorded}"
    );
    let written = zerodds_lang_render::java::type_(&zerodds_corba_codegen::java_holder_type(
        &StubType::Special(SpecialType::ValueBase),
    ));
    assert_eq!(written, "org.omg.CORBA.ValueBaseHolder");
}

#[test]
fn a_typedef_takes_its_own_java_holder_only_where_the_aliased_type_has_none() {
    // Measured in row 15: a struct alias resolves through to
    // `probe.WithStringHolder` and a sequence alias does not — it takes
    // `probe.LongsAliasHolder`, because a sequence has no Java
    // declaration of its own.
    let recorded = block(JACORB, "15-typedefs");
    let struct_alias = StubType::alias(
        "probe::StructAlias",
        StubType::other("probe::WithString", TypeShape::VariableAggregate),
    );
    let sequence_alias = StubType::alias(
        "probe::LongsAlias",
        StubType::other("probe::Longs", TypeShape::Collection),
    );
    for type_ in [&struct_alias, &sequence_alias] {
        let written =
            zerodds_lang_render::java::type_(&zerodds_corba_codegen::java_holder_type(type_));
        assert!(contains(&recorded, &written), "`{written}`\n{recorded}");
    }
    assert_eq!(
        zerodds_lang_render::java::type_(&zerodds_corba_codegen::java_holder_type(&struct_alias)),
        "probe.WithStringHolder"
    );
    assert_eq!(
        zerodds_lang_render::java::type_(&zerodds_corba_codegen::java_holder_type(&sequence_alias)),
        "probe.LongsAliasHolder"
    );
}

#[test]
fn each_orbs_refusal_is_recorded_against_the_row_it_refused() {
    // The three refusals, asserted so a re-recording that silently
    // starts succeeding — a toolchain upgrade — is noticed rather than
    // leaving a row unchecked for good.
    assert!(
        block(OMNIORB, "04-native").contains("Unsupported IDL construct found in input (native)"),
        "omniidl no longer refuses `native`; row 04 can be checked against it"
    );
    assert!(
        block(TAO, "11-fixed").contains("fixed data types are not supported"),
        "tao_idl no longer refuses `fixed`; row 11 can be checked against it"
    );
    assert!(
        block(JACORB, "05-typecode").contains("Undefined name"),
        "JacORB no longer refuses `CORBA::TypeCode`; row 05 can be checked against it"
    );
}

#[test]
fn the_recording_covers_every_row_of_the_corpus() {
    let expected: Vec<&str> = vec![
        "01-object",
        "02-valuebase",
        "03-abstractbase",
        "04-native",
        "05-typecode",
        "06-any",
        "07-anyseq",
        "08-string",
        "09-wstring",
        "10-time",
        "11-fixed",
        "12-ulonglong",
        "13-longdouble",
        "14-shape",
        "15-typedefs",
    ];
    for (orb, recorded) in [("omniorb", OMNIORB), ("tao", TAO), ("jacorb", JACORB)] {
        let seen = rows(recorded);
        let mut names: Vec<&str> = seen.keys().copied().collect();
        names.sort_unstable();
        assert_eq!(names, expected, "{orb}");
    }
}
