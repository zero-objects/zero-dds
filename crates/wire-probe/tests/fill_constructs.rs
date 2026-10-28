// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C the emitter writes for the five constructs, and what it
//! refuses to write.
//!
//! # What these can and cannot show
//!
//! The emitter puts values into the *vendor's* generated structures; the
//! bytes still come from the vendor's serialiser. So a test here can
//! only show that the emitted statements say what the language binding
//! says they should — it cannot show that they compile, because that
//! needs `idlc` and `rtiddsgen` and those live on the Linux test host.
//! The assertions are on the text, and the compile is the harness's job.
//!
//! Where the binding leaves the C shape to the vendor, the emitter
//! refuses instead of guessing. Those refusals are the more useful half
//! of this file: they are what keeps a wrong sample out of a matrix cell
//! that would otherwise read as a measurement.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast_core::ScopedName;
use zerodds_wire_probe::encode::{ProbeError, model_from_idl};
use zerodds_wire_probe::fill::{Vendor, fill};

/// The statements the emitter writes for one type and one sample.
fn source(idl: &str, type_name: &str, values: &serde_json::Value, vendor: Vendor) -> String {
    emit(idl, type_name, values, vendor).expect("the emitter")
}

fn emit(
    idl: &str,
    type_name: &str,
    values: &serde_json::Value,
    vendor: Vendor,
) -> Result<String, ProbeError> {
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let segments: Vec<String> = type_name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    let type_ = ast
        .find_by_scoped_name(&ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the type");
    fill(&ast, type_, values, vendor)
}

// -- wstring -----------------------------------------------------------

/// XTypes 1.3 §7.5.1.1.1 leaves the C spelling of `Char16` to the
/// vendor — `wchar_t` "if the platform supports it and it is of
/// sufficient size", otherwise "an equivalent integer type". So the
/// emitter must not name the element type, and takes it from the
/// vendor's own field instead.
///
/// The code units are the same ones the ZeroDDS probe writes; the
/// trailing zero is the C terminator, which is how a `Char16 *` field
/// carries its length.
#[test]
fn a_wstring_is_emitted_as_a_typed_array_and_never_as_a_wide_literal() {
    let idl = "@final struct WStr { wstring w; };";
    for vendor in Vendor::ALL {
        let out = source(idl, "WStr", &json!({ "w": "ab" }), vendor);
        assert!(
            out.contains("static __typeof__(*sample.w) zd_wstr_1[] = { 0x0061, 0x0062, 0x0000 };"),
            "{vendor:?}: {out}"
        );
        assert!(out.contains("sample.w = zd_wstr_1;"), "{vendor:?}: {out}");
        assert!(!out.contains("L\""), "{vendor:?}: {out}");
    }
}

/// A code point outside the BMP is two code units in the array, the
/// same two the wire carries. §7.4.1.1.2.
#[test]
fn a_wstring_outside_the_bmp_emits_both_of_its_code_units() {
    let idl = "@final struct WStr { wstring w; };";
    let out = source(idl, "WStr", &json!({ "w": "\u{1d11e}" }), Vendor::Cyclone);
    assert!(out.contains("{ 0xd834, 0xdd1e, 0x0000 }"), "{out}");
}

/// A `sequence<wstring>` takes its element type from the sequence, which
/// is a field like any other.
///
/// AL-12. This was refused on the reasoning that a collection "has no
/// field to take the element type from until it has one", so neither
/// the Cyclone backing array nor the RTI `<T>Seq` prefix could be
/// spelled. The sequence *is* the field: Cyclone's `sequence<wstring>`
/// is a `dds_sequence_wstring` whose `_buffer` is a `wchar_t **`, and
/// RTI generates `DDS_WstringSeq` — both measured 2026-08-10 with
/// `idlc` and `rtiddsgen -language C`, both compiled, and both writing
/// bytes the ZeroDDS probe reproduces under `--dialect cyclone` and
/// `--dialect rti`.
#[test]
fn a_sequence_of_wstring_takes_its_element_type_from_the_sequence() {
    let idl = "@final struct WSeq { sequence<wstring> ws; };";

    let cyclone = source(idl, "WSeq", &json!({ "ws": ["ab"] }), Vendor::Cyclone);
    // The backing array's element type is the vendor's own, never named.
    assert!(
        cyclone.contains("static __typeof__(*sample.ws._buffer) zd_seq_1[1];"),
        "{cyclone}"
    );
    assert!(
        cyclone
            .contains("static __typeof__(*zd_seq_1[0]) zd_wstr_2[] = { 0x0061, 0x0062, 0x0000 };"),
        "{cyclone}"
    );
    assert!(!cyclone.contains("L\""), "{cyclone}");

    let rti = source(idl, "WSeq", &json!({ "ws": ["ab"] }), Vendor::Rti);
    assert!(
        rti.contains("DDS_WstringSeq_set_length(&sample.ws, 1);"),
        "{rti}"
    );
    assert!(
        rti.contains("(*DDS_WstringSeq_get_reference(&sample.ws, 0)) = zd_wstr_1;"),
        "{rti}"
    );
}

// -- bitmask -----------------------------------------------------------

/// XTypes 1.3 §7.5.1.2.2 defines the bitmask binding as an enumeration
/// of the flag names with `@value(1 << position)`, plus a typedef of the
/// type name to an unsigned integer. So the flags are enumerators, and
/// the emitter names them rather than computing the integer — which
/// leaves the position-to-bit mapping on the vendor's side, where the
/// thing under test is.
#[test]
fn a_bitmask_is_emitted_as_its_flag_names_ored_together() {
    let idl = "\
@bit_bound(8) bitmask Perm { READ, WRITE, EXEC };
@final struct B8 { Perm p; };";
    for vendor in Vendor::ALL {
        let out = source(idl, "B8", &json!({ "p": ["READ", "WRITE"] }), vendor);
        assert_eq!(out.trim(), "sample.p = READ | WRITE;", "{vendor:?}");
    }
}

/// Cyclone flattens the module path onto the enumerator; RTI does not.
/// `rtiddsgen -language C` emits the flag names bare — measured over
/// the matrix module-bitmask cells when the emitter split by vendor —
/// while Cyclone's idlc prefixes them with the flattened module path.
#[test]
fn a_bitmask_inside_a_module_carries_the_flattened_module_path() {
    let idl = "\
module app {
    bitmask Flags { A, B };
    @final struct Holder { Flags f; };
};";
    let out = source(idl, "app::Holder", &json!({ "f": ["B"] }), Vendor::Rti);
    assert!(out.contains("sample.f = B;"), "{out}");
    let out = source(idl, "app::Holder", &json!({ "f": ["B"] }), Vendor::Cyclone);
    assert!(out.contains("sample.f = app_B;"), "{out}");
}

/// No flags set is the integer zero. An empty expression would not
/// compile, and leaving the statement out would test whatever
/// `memset` left behind rather than the sample.
#[test]
fn an_empty_bitmask_is_emitted_as_zero_and_not_as_nothing() {
    let idl = "\
bitmask Flags { A, B };
@final struct Holder { Flags f; };";
    let out = source(idl, "Holder", &json!({ "f": [] }), Vendor::Cyclone);
    assert_eq!(out.trim(), "sample.f = 0;");
}

/// A misspelled flag would otherwise emit a name that does not compile,
/// which is a build failure with no path in it. Caught here instead,
/// with the declared names listed.
#[test]
fn a_misspelled_flag_is_refused_with_the_declared_names() {
    let idl = "\
bitmask Flags { A, B };
@final struct Holder { Flags f; };";
    let error = emit(idl, "Holder", &json!({ "f": ["C"] }), Vendor::Cyclone).expect_err("a typo");
    assert!(
        matches!(&error, ProbeError::Unknown { declared, .. }
            if declared == &["A".to_string(), "B".to_string()]),
        "{error}"
    );
}

// -- the three with no C binding ---------------------------------------

/// XTypes 1.3 §7.5.1 builds the plain language binding on the OMG
/// IDL-to-C mapping and extends it for primitive types, annotations,
/// bitmask types and external members. It extends it for no bitset, no
/// map and no fixed-point type, and the mapping underneath has none of
/// the three. `vendor_bytes.md` records no measurement of any of them
/// either.
///
/// Each refusal names its construct, so a matrix cell that stays empty
/// says why it is empty.
#[test]
fn the_constructs_with_no_c_binding_are_refused_by_name() {
    let cases = [
        (
            "bitset B { bitfield<3> a; };\n@final struct H { B packed; };",
            json!({ "packed": { "a": 1 } }),
            "bitset",
        ),
        (
            "@final struct H { map<long, long> m; };",
            json!({ "m": [[1, 2]] }),
            "map",
        ),
        (
            "@final struct H { fixed<5,2> f; };",
            json!({ "f": "1.00" }),
            "fixed",
        ),
    ];
    for (idl, values, construct) in cases {
        for vendor in Vendor::ALL {
            let error = emit(idl, "H", &values, vendor).expect_err("a refusal");
            assert!(
                matches!(&error, ProbeError::Unsupported { kind, .. } if kind.contains(construct)),
                "{construct}, {vendor:?}: {error}"
            );
        }
    }
}

/// The narrow-string path is untouched by the wide one: a `string` is
/// still a literal for Cyclone and a `DDS_String_dup` for RTI.
#[test]
fn a_narrow_string_still_takes_each_vendors_own_idiom() {
    let idl = "@final struct Str { string s; };";
    assert_eq!(
        source(idl, "Str", &json!({ "s": "abc" }), Vendor::Cyclone).trim(),
        "sample.s = (char *)\"abc\";"
    );
    assert_eq!(
        source(idl, "Str", &json!({ "s": "abc" }), Vendor::Rti).trim(),
        "sample.s = DDS_String_dup(\"abc\");"
    );
}

// -- what a walk over a partial accessor left out ------------------------

/// The C emitter fills the members a struct inherits.
///
/// The walk went over `Ast::members_of`, which answers with a type's own
/// members. The base's were never assigned, and a C sample is
/// zero-initialised, so the vendor probe serialised a structure of the
/// right length with the base fields at zero and reported success:
///
/// ```text
/// 0000000000000000182d4454fb210940ab   what it produced
/// 7929edfffeff0000182d4454fb210940ab   what the value said
/// ```
///
/// The right length is what made it hard to see. XTypes 1.3
/// §7.4.1.1.5.1 puts the base's members first, so the plan wrote three
/// and the fill assigned one.
#[test]
fn the_c_emitter_fills_an_inherited_member() {
    let idl = "
        struct Base { long base_a; short base_b; };
        struct Sample : Base { double own; };
    ";
    let values = json!({"base_a": -1234, "base_b": -2, "own": 2.5});
    let source = source(idl, "::Sample", &values, Vendor::Cyclone);
    // Cyclone's idlc renders the base as an embedded member named
    // `parent`, so the inherited fields are reached through it —
    // measured over the matrix struct-inheritance cells when the walk
    // was taught the base members at all.
    assert!(
        source.contains("sample.parent.base_a"),
        "base_a unfilled:\n{source}"
    );
    assert!(
        source.contains("sample.parent.base_b"),
        "base_b unfilled:\n{source}"
    );
    assert!(source.contains("sample.own"), "own unfilled:\n{source}");
}

/// A key that names no member is refused, not skipped.
///
/// This is what would have caught the defect above: `base_a` matched
/// nothing and was dropped without a word. `encode.rs` and
/// `fill_cpp.rs` both refuse here, and all three read the same value
/// file — a key one acts on and another ignores means the probes
/// disagree about what the sample is.
///
/// The path is rooted at the type's own scoped name, taken from the
/// model rather than from the caller's spelling: `::Sample` and
/// `Sample` name the same place, and a refusal the three probes report
/// under different paths cannot be compared between them.
#[test]
fn the_c_emitter_refuses_a_key_that_names_no_member() {
    let idl = "struct Sample { long a; };";
    let values = json!({"a": 1, "nope": 2});
    match emit(idl, "::Sample", &values, Vendor::Cyclone) {
        Err(ProbeError::Unknown { path, .. }) => assert_eq!(path, "Sample.nope"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}
