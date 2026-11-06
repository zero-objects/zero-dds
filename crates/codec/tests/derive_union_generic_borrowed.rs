// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Unions from data-carrying enums, generic types and borrowed fields at
//! run time: the IDL they export, the discriminator a value goes out under
//! and the one a reader is refused, the names of the instantiations, and
//! the octets a borrowed field lends.
//!
//! That each is the type of its IDL — identifier, object, octets in all
//! four encodings, key holder — is held by the generated codec in
//! `crates/zerodds-build/tests/dds_type.rs`
//! (`xtypes_7_3_4_7_4_issue_33_a_derived_type_and_its_idl_are_one_type`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::borrow::Cow;

use zerodds_codec::{CodecError, DdsType, Encapsulation, WireError};

#[derive(DdsType, Debug, Clone, Copy, PartialEq)]
#[dds(name = "u::Mode")]
enum Mode {
    #[dds(name = "OFF")]
    Off,
    #[dds(name = "ON", value = 7)]
    On,
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "u::Reading", extensibility = "final", discriminator = "i16")]
enum Reading {
    #[dds(case = 1, name = "celsius")]
    Celsius(f64),
    #[dds(case = [2, 3], name = "kelvin")]
    Kelvin(i16, f64),
    #[dds(default, name = "raw")]
    Raw(i16, Vec<u8>),
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "u::Toggle", discriminator = "Mode")]
enum Toggle {
    #[dds(case = Mode::On, name = "level")]
    Level(u8),
    #[dds(no_member)]
    Idle(Mode),
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "u::Counted")]
enum Counted {
    #[dds(name = "zero")]
    Zero(i32),
    #[dds(case = 5, name = "five")]
    Five(i32),
    #[dds(name = "six")]
    Six(String),
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "u::Point")]
struct Point {
    x: i32,
}

#[derive(DdsType, Debug, Clone, PartialEq)]
struct Pair<T> {
    first: T,
    second: T,
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "u::Window_{T}_{N}", extensibility = "final")]
struct Window<T, const N: usize> {
    values: [T; N],
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "u::Lent", extensibility = "final")]
struct Lent<'a> {
    text: &'a str,
    data: &'a [u8],
    note: Cow<'a, str>,
}

fn encoded<T: zerodds_codec::DdsSerialize>(value: &T, encapsulation: Encapsulation) -> Vec<u8> {
    let mut out = Vec::new();
    zerodds_codec::encode(value, encapsulation, &mut out).expect("encode");
    out
}

/// IDL 4.2 §7.4.1.4.4.4: the union a data-carrying enum exports — the
/// discriminator type, the labels written, counted and `default:`, an
/// enumeration's labels by their enumerators.
#[test]
fn idl_4_2_7_4_1_4_4_4_a_data_carrying_enum_exports_its_union() {
    assert_eq!(
        Reading::idl().expect("the IDL"),
        "module u { @final union Reading switch (short) { case 1: double celsius; case 2: case 3: \
         double kelvin; default: sequence<octet> raw; }; };\n"
    );
    assert_eq!(
        Toggle::idl().expect("the IDL"),
        "module u { enum Mode { OFF, @value(7) ON }; };\n\
         module u { @appendable union Toggle switch (::u::Mode) { case ::u::ON: octet level; }; };\n"
    );
    assert_eq!(
        Counted::idl().expect("the IDL"),
        "module u { @appendable union Counted switch (long) { case 0: long zero; case 5: long \
         five; case 6: string six; }; };\n"
    );
}

/// XTypes 1.3 §7.4.3.5.3 rule (10): the discriminator, then the selected
/// member; a variant that carries its discriminator goes out under it, and
/// a reader reads it back into the same variant.
#[test]
fn xtypes_7_4_3_5_3_rule_10_the_discriminator_a_variant_carries_goes_out() {
    for encapsulation in Encapsulation::ALL {
        for value in [
            Reading::Celsius(21.5),
            Reading::Kelvin(3, 290.0),
            Reading::Raw(-8, vec![1, 2]),
        ] {
            let bytes = encoded(&value, encapsulation);
            let back: Reading = zerodds_codec::decode(&bytes, encapsulation).expect("read back");
            assert_eq!(back, value, "{encapsulation:?}");
        }
        for value in [Toggle::Level(4), Toggle::Idle(Mode::Off)] {
            let bytes = encoded(&value, encapsulation);
            let back: Toggle = zerodds_codec::decode(&bytes, encapsulation).expect("read back");
            assert_eq!(back, value, "{encapsulation:?}");
        }
    }
    let bytes = encoded(&Reading::Kelvin(3, 1.0), Encapsulation::Xcdr2Le);
    assert_eq!(
        &bytes[..2],
        3i16.to_le_bytes(),
        "the carried label, not the first one"
    );
}

/// IDL 4.2 §7.4.1.4.4.4: a discriminator that selects another case than
/// the variant's is refused before an octet is written, and so is one that
/// selects a case for the variant without a member.
#[test]
fn idl_4_2_7_4_1_4_4_4_a_discriminator_of_another_case_is_refused() {
    let mut out = Vec::new();
    for value in [Reading::Kelvin(1, 0.0), Reading::Raw(2, Vec::new())] {
        match zerodds_codec::encode(&value, Encapsulation::Xcdr2Le, &mut out) {
            Err(CodecError::Wire(WireError::OutOfRange { expected, .. })) => {
                assert!(expected.contains("selects the branch"), "{expected}");
            }
            other => panic!("a refusal, got {other:?}"),
        }
    }
    match zerodds_codec::encode(&Toggle::Idle(Mode::On), Encapsulation::Xcdr2Le, &mut out) {
        Err(CodecError::Wire(WireError::OutOfRange { expected, .. })) => {
            assert!(expected.contains("no case"), "{expected}");
        }
        other => panic!("a refusal, got {other:?}"),
    }
    assert!(out.is_empty(), "a refused sample writes nothing");
}

/// The name of each instantiation: the default template, a written one, a
/// `const` argument, a described type as an argument.
#[test]
fn issue_33_each_instantiation_of_a_generic_type_has_its_own_name() {
    assert_eq!(
        <Pair<u32> as zerodds_dcps::DdsType>::type_name(),
        "Pair_unsigned_long"
    );
    assert_eq!(
        <Pair<Point> as zerodds_dcps::DdsType>::type_name(),
        "Pair_u_Point"
    );
    assert_eq!(
        <Pair<String> as zerodds_dcps::DdsType>::type_name(),
        "Pair_string"
    );
    assert_eq!(
        <Window<i16, 4> as zerodds_dcps::DdsType>::type_name(),
        "u::Window_short_4"
    );
    assert_eq!(
        Pair::<Vec<Point>>::idl().expect("the IDL"),
        "module u { @appendable struct Point { long x; }; };\n\
         @appendable struct Pair_sequence_u_Point { sequence<::u::Point> first; \
         sequence<::u::Point> second; };\n"
    );
    assert_ne!(
        <Pair<u32> as zerodds_dcps::DdsType>::type_identifier(),
        <Pair<i32> as zerodds_dcps::DdsType>::type_identifier()
    );
}

/// The codec API reads a borrowed field lent out of the received bytes.
#[test]
fn issue_33_a_borrowed_field_is_lent_out_of_the_sample() {
    let data = [9u8, 8, 7];
    let value = Lent {
        text: "lent",
        data: &data,
        note: Cow::Borrowed("note"),
    };
    for encapsulation in Encapsulation::ALL {
        let bytes = encoded(&value, encapsulation);
        let back: Lent<'_> = zerodds_codec::decode(&bytes, encapsulation).expect("read");
        assert_eq!(back, value);
        let range = bytes.as_ptr_range();
        assert!(range.contains(&back.text.as_ptr()));
        assert!(range.contains(&back.data.as_ptr()));
        assert!(matches!(back.note, Cow::Borrowed(text) if range.contains(&text.as_ptr())));
    }
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "u::Sensor", extensibility = "final")]
struct Sensor {
    id: i32,
    reading: Reading,
    toggle: Toggle,
}

/// Whether the filter `expression` passes `sample`, as a `QueryCondition`
/// and a `ContentFilteredTopic` evaluate it (DDS 1.4 Annex B).
fn passes<T: zerodds_dcps::DdsType>(expression: &str, sample: &T) -> bool {
    let base = zerodds_dcps::ReadCondition::new(0xFFFF, 0xFFFF, 0xFFFF, |_, _, _| true);
    let condition =
        zerodds_dcps::QueryCondition::new(base, expression, Vec::new()).expect("expression");
    condition
        .evaluate(&zerodds_dcps::DdsTypeRow::new(sample))
        .unwrap_or(false)
}

/// DDS-XTypes 1.4 Beta 1 §7.6.7.1: "Union members shall be referenced
/// using the name of the member. The union discriminator may also be
/// referenced using the name “#discriminator”. […] if the discriminator
/// does not select the named member, the result of the filter evaluation
/// shall be false." An enumerated discriminator compares with the name of
/// its enumerator (DDS 1.4 Annex B.2 ENUMERATEDVALUE).
#[test]
fn xtypes_1_4_7_6_7_1_a_filter_reads_the_selected_branch_and_the_discriminator() {
    let sensor = Sensor {
        id: 4,
        reading: Reading::Kelvin(3, 290.5),
        toggle: Toggle::Level(9),
    };
    for (expression, expected) in [
        ("reading.#discriminator = 3", true),
        ("reading.#discriminator = 2", false),
        ("reading.kelvin > 290.0", true),
        ("reading.kelvin > 300.0", false),
        ("toggle.#discriminator = 'ON'", true),
        ("toggle.level = 9", true),
        // `celsius` is not the branch discriminator 3 selects: false,
        // whatever else the expression says.
        ("reading.celsius = 1.0", false),
        ("reading.celsius <> 1.0", false),
        ("NOT reading.celsius = 1.0", false),
        ("id = 4 OR reading.celsius = 1.0", false),
        ("reading.raw = 1 OR id = 4", false),
        ("id = 4", true),
    ] {
        assert_eq!(passes(expression, &sensor), expected, "{expression}");
    }
    // A single label fixes the discriminator a variant goes out under.
    let celsius = Sensor {
        id: 1,
        reading: Reading::Celsius(21.5),
        toggle: Toggle::Idle(Mode::Off),
    };
    assert!(passes(
        "reading.#discriminator = 1 AND reading.celsius = 21.5",
        &celsius
    ));
    assert!(passes("toggle.#discriminator = 'OFF'", &celsius));
    assert!(!passes("toggle.level = 0", &celsius));
    // A union as the topic type: its members at the root.
    assert!(passes(
        "#discriminator = 3 AND kelvin = 290.5",
        &Reading::Kelvin(3, 290.5)
    ));
    assert!(!passes("celsius = 290.5", &Reading::Kelvin(3, 290.5)));
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(
    name = "u::Command",
    discriminator = "Mode",
    extensibility = "final",
    key
)]
enum Command {
    #[dds(case = Mode::On, name = "level")]
    Level(u8),
    #[dds(case = Mode::Off, name = "reason")]
    Reason(String),
}

/// DDS-XTypes 1.3 §7.2.2.4.4.4.8 (`switch (@key D)`) and §7.6.8: the
/// union is a keyed type, its key holder is the discriminator — `ON` is 7,
/// a four-octet enumeration holder, big-endian — and its KeyHash those
/// octets zero-padded; read back from the holder the branch takes its
/// default.
#[test]
fn xtypes_7_6_8_the_key_holder_of_a_union_keyed_by_its_discriminator() {
    use zerodds_dcps::DdsType as _;
    let idl = zerodds_codec::idl::<Command>().expect("idl");
    assert!(idl.contains("switch (@key"), "{idl}");
    const { assert!(<Command as zerodds_dcps::DdsType>::HAS_KEY) };
    let command = Command::Level(3);
    assert_eq!(
        zerodds_codec::key_holder(&command).expect("holder"),
        [0, 0, 0, 7]
    );
    assert_eq!(
        command.key_hash(),
        Some([0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    );
    assert_eq!(
        zerodds_codec::from_key_holder::<Command>(&[0, 0, 0, 7]).expect("back"),
        Command::Level(0)
    );
    assert_eq!(
        zerodds_codec::from_key_holder::<Command>(&[0, 0, 0, 0]).expect("back"),
        Command::Reason(String::new())
    );
}
