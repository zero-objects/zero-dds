// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The hand-written `ShapeType` against the IDL it claims to be.
//!
//! `crates/dcps/src/interop.rs` carries a `ShapeType` written by hand:
//! its module docs quote the OMG `dds-rtps` suite's `srcCxx/shape.idl`
//! and a CDR layout worked out on paper. Every live interop test
//! publishes and subscribes that type — `fastdds_live_pub`,
//! `fastdds_live_sub`, `liveliness_qos`, `lifespan_qos`,
//! `time_based_filter_qos`, `shapes_api_e2e` — and `shapes_type_wire.rs`
//! checks it against byte sequences that were *also* derived by hand.
//! Hand against hand: a defect in this project's own IDL front end could
//! not fail any of them, and the hand-written type was a second source
//! of truth beside the generator.
//!
//! This is the independent check. The same values go out through two
//! paths that share no code:
//!
//! 1. `interop::ShapeType`'s `DdsType` impl — the hand-written codec on
//!    `zerodds_wire::Writer`.
//! 2. `interop/shapes/omg-dds-rtps.idl`, parsed and lowered by
//!    `zerodds-idl-compose`, planned by `zerodds-ast-layout` and walked
//!    by the reflective `zerodds-wire-codec` serialiser (through
//!    `zerodds-wire-probe`, which pins that walk to `serde_json::Value`).
//!
//! Path 2 compiles nothing: it reads the IDL at test time and follows
//! the layout plan, so what it asserts is what the front end says the
//! type's octets are — which is the thing under test. The generated
//! path is covered separately by `zerodds-wire-conformance`.
//!
//! All four modes the interop matrix carries are compared: `xcdr1-le`,
//! `xcdr1-be`, `xcdr2-le`, `xcdr2-be`.
//!
//! The second half of the file is about the *other* `ShapeType`.
//! `interop/shapes/shapesdemo.idl` is the four-member `@final` form the
//! classic ShapesDemo GUIs speak — a different type on the wire, not a
//! shorter one. `interop/shapes/README.md` says which peer speaks which.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_dcps::DdsType;
use zerodds_dcps::interop::{ShapeExtendedType, ShapeFillKind, ShapeType};
use zerodds_wire_probe::encode::{encode, hex, model_from_idl};
use zerodds_wire_probe::value::ProbeRequest;

// -- the four modes --------------------------------------------------

/// One column of the interop matrix: an XCDR version and a byte order.
#[derive(Clone, Copy)]
struct Mode {
    /// The name the matrix uses.
    name: &'static str,
    /// The XCDR2 framing of an `@appendable` aggregate is DELIMITED_CDR
    /// and of a `@final` one PLAIN_CDR2; under XCDR1 both are PLAIN_CDR
    /// (XTypes 1.3 Tab. 59). Recorded per mode rather than derived
    /// twice.
    appendable: Encoding,
    final_: Encoding,
    endianness: Endianness,
}

const MODES: [Mode; 4] = [
    Mode {
        name: "xcdr1-le",
        appendable: Encoding::PlainCdr1,
        final_: Encoding::PlainCdr1,
        endianness: Endianness::Little,
    },
    Mode {
        name: "xcdr1-be",
        appendable: Encoding::PlainCdr1,
        final_: Encoding::PlainCdr1,
        endianness: Endianness::Big,
    },
    Mode {
        name: "xcdr2-le",
        appendable: Encoding::DelimitedCdr,
        final_: Encoding::PlainCdr2,
        endianness: Endianness::Little,
    },
    Mode {
        name: "xcdr2-be",
        appendable: Encoding::DelimitedCdr,
        final_: Encoding::PlainCdr2,
        endianness: Endianness::Big,
    },
];

/// The hand-written codec's octets for one mode.
fn hand_written(sample: &ShapeType, mode: Mode) -> Vec<u8> {
    let mut out = Vec::new();
    let result = match (mode.appendable, mode.endianness) {
        (Encoding::PlainCdr1, Endianness::Little) => sample.encode_xcdr1(&mut out),
        (Encoding::PlainCdr1, Endianness::Big) => sample.encode_xcdr1_be(&mut out),
        (_, Endianness::Little) => sample.encode(&mut out),
        (_, Endianness::Big) => sample.encode_be(&mut out),
    };
    result.unwrap_or_else(|e| panic!("hand-written ShapeType refuses {}: {e}", mode.name));
    out
}

/// What the hand-written codec makes of octets the IDL path produced.
fn hand_written_decode(bytes: &[u8], mode: Mode) -> ShapeType {
    let decoded = match (mode.appendable, mode.endianness) {
        (Encoding::PlainCdr1, Endianness::Little) => ShapeType::decode_xcdr1(bytes),
        (Encoding::PlainCdr1, Endianness::Big) => ShapeType::decode_xcdr1_be(bytes),
        (_, Endianness::Little) => ShapeType::decode(bytes),
        (_, Endianness::Big) => ShapeType::decode_be(bytes),
    };
    decoded.unwrap_or_else(|e| {
        panic!(
            "hand-written ShapeType cannot read its own {} octets: {e}",
            mode.name
        )
    })
}

// -- the IDL path ----------------------------------------------------

fn shapes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../interop/shapes")
        .canonicalize()
        .expect("interop/shapes exists")
}

/// The OMG five-member `@appendable` form, as a lowered model.
fn omg_model() -> Ast {
    let path = shapes_dir().join("omg-dds-rtps.idl");
    let source = std::fs::read_to_string(&path).expect("the OMG shape IDL");
    model_from_idl(&source, &path.display().to_string()).expect("the OMG shape IDL lowers cleanly")
}

/// The four-member `@final` form the ShapesDemo GUIs speak.
fn shapesdemo_model() -> Ast {
    let path = shapes_dir().join("shapesdemo.idl");
    let source = std::fs::read_to_string(&path).expect("the ShapesDemo shape IDL");
    model_from_idl(&source, &path.display().to_string())
        .expect("the ShapesDemo shape IDL lowers cleanly")
}

/// The reflective walk's octets for one model, value and mode.
fn through_idl(model: &Ast, values: &Value, encoding: Encoding, endianness: Endianness) -> Vec<u8> {
    through_idl_named(model, "ShapeType", values, encoding, endianness)
}

/// The same, for a model whose topic type is not called `ShapeType`.
fn through_idl_named(
    model: &Ast,
    type_name: &str,
    values: &Value,
    encoding: Encoding,
    endianness: Endianness,
) -> Vec<u8> {
    encode(
        model,
        &ProbeRequest::new(type_name, encoding, endianness),
        values,
    )
    .expect("the layout plan serialises the sample")
}

/// The sample as the probe's value document. Written out member by
/// member rather than derived, so a member the hand-written struct
/// gained and the IDL did not would fail here instead of being skipped.
fn as_value(sample: &ShapeType) -> Value {
    json!({
        "color": sample.color,
        "x": sample.x,
        "y": sample.y,
        "shapesize": sample.shapesize,
        "additional_payload_size": sample.additional_payload_size,
    })
}

/// The samples both paths are asked for.
///
/// The colours are chosen for their lengths: `"RED"` and `"BLUE"` put
/// the string's null terminator at different offsets, `"MAGENTA"` lands
/// exactly on a four-octet boundary and `"A"` leaves the widest pad. A
/// padding rule that differed between the two paths shows up as a
/// length difference in at least one of them.
fn samples() -> Vec<ShapeType> {
    vec![
        ShapeType::new("RED", 42, 77, 30),
        ShapeType::new("BLUE", -1, 270, 30),
        ShapeType::new("A", 0, 0, 0),
        ShapeType::with_payload("MAGENTA", 7, 8, 9, vec![1, 2, 3, 4, 5]),
        ShapeType::with_payload("CYAN", i32::MIN, i32::MAX, -1, vec![0xff; 17]),
    ]
}

// -- the agreement ---------------------------------------------------

#[test]
fn hand_written_shape_type_encodes_as_the_omg_idl_in_all_four_modes() {
    let model = omg_model();
    for sample in samples() {
        let values = as_value(&sample);
        for mode in MODES {
            let by_hand = hand_written(&sample, mode);
            let by_idl = through_idl(&model, &values, mode.appendable, mode.endianness);
            assert_eq!(
                by_hand,
                by_idl,
                "\n{mode} disagrees for {sample:?}\n  crates/dcps/src/interop.rs : {hand}\n  interop/shapes/omg-dds-rtps.idl: {idl}\n",
                mode = mode.name,
                hand = hex(&by_hand),
                idl = hex(&by_idl),
            );
        }
    }
}

#[test]
fn hand_written_shape_type_decodes_what_the_omg_idl_produced() {
    let model = omg_model();
    for sample in samples() {
        let values = as_value(&sample);
        for mode in MODES {
            let by_idl = through_idl(&model, &values, mode.appendable, mode.endianness);
            assert_eq!(
                hand_written_decode(&by_idl, mode),
                sample,
                "{} sample does not survive the IDL path",
                mode.name
            );
        }
    }
}

/// The XCDR2-LE case of the agreement, spelled out against the same
/// literal `shapes_type_wire.rs` carries — so the two files cannot
/// drift apart silently while both stay green.
#[test]
fn the_hand_derived_reference_octets_are_what_the_omg_idl_says() {
    const EXPECTED_RED_42_77_30: &[u8] = &[
        0x18, 0x00, 0x00, 0x00, // DHEADER = 24
        0x04, 0x00, 0x00, 0x00, // color.length
        0x52, 0x45, 0x44, 0x00, // "RED\0"
        0x2a, 0x00, 0x00, 0x00, // x = 42
        0x4d, 0x00, 0x00, 0x00, // y = 77
        0x1e, 0x00, 0x00, 0x00, // shapesize = 30
        0x00, 0x00, 0x00, 0x00, // additional_payload_size.length = 0
    ];
    let by_idl = through_idl(
        &omg_model(),
        &as_value(&ShapeType::new("RED", 42, 77, 30)),
        Encoding::DelimitedCdr,
        Endianness::Little,
    );
    assert_eq!(
        by_idl,
        EXPECTED_RED_42_77_30,
        "the reference in shapes_type_wire.rs is not what the IDL plans:\n  idl: {}\n  ref: {}",
        hex(&by_idl),
        hex(EXPECTED_RED_42_77_30)
    );
}

// -- the other ShapeType ---------------------------------------------

/// The two forms are two types on the wire, in every mode. Nothing may
/// treat one as a shorter spelling of the other.
#[test]
fn the_shapesdemo_form_and_the_omg_form_differ_on_the_wire() {
    let omg = omg_model();
    let demo = shapesdemo_model();
    let sample = ShapeType::new("RED", 42, 77, 30);
    let omg_value = as_value(&sample);
    let demo_value = json!({
        "color": sample.color,
        "x": sample.x,
        "y": sample.y,
        "shapesize": sample.shapesize,
    });
    for mode in MODES {
        let five = through_idl(&omg, &omg_value, mode.appendable, mode.endianness);
        let four = through_idl(&demo, &demo_value, mode.final_, mode.endianness);
        assert_ne!(
            five, four,
            "{} — the @appendable and the @final ShapeType must not coincide",
            mode.name
        );
    }
}

/// How close the two are, stated as octets: under XCDR1 neither form
/// carries a DHEADER (XTypes 1.3 Tab. 59), so the four-member `@final`
/// body is exactly the five-member body without its trailing
/// `sequence<octet>` length. That is why a reader of one silently
/// mis-frames the other's sample rather than running off the end.
#[test]
fn the_shapesdemo_form_is_the_omg_form_without_its_trailing_sequence() {
    let sample = ShapeType::new("BLUE", -1, 270, 30);
    let demo_value = json!({
        "color": sample.color,
        "x": sample.x,
        "y": sample.y,
        "shapesize": sample.shapesize,
    });
    for endianness in [Endianness::Little, Endianness::Big] {
        let five = through_idl(
            &omg_model(),
            &as_value(&sample),
            Encoding::PlainCdr1,
            endianness,
        );
        let four = through_idl(
            &shapesdemo_model(),
            &demo_value,
            Encoding::PlainCdr1,
            endianness,
        );
        let mut four_plus_empty_sequence = four.clone();
        four_plus_empty_sequence.extend_from_slice(&[0u8; 4]);
        assert_eq!(
            five,
            four_plus_empty_sequence,
            "XCDR1 {endianness:?}: {} vs {}",
            hex(&five),
            hex(&four)
        );
    }
}

// -- ShapeExtendedType -----------------------------------------------

// The same measurement for the other hand-written type in
// `crates/dcps/src/interop.rs`.
//
// `ShapeExtendedType` is what RTI Connext 7.x ShapesDemo publishes
// without `-dataType Shape`, and until `interop/shapes/shapesdemo-extended.idl`
// was written it had no IDL anywhere in the tree and nothing held its
// octets against one — its codec, its member order and its enum width
// were asserted only against byte sequences derived by the same hand
// that wrote the codec. This closes it the way
// `hand_written_shape_type_encodes_as_the_omg_idl_in_all_four_modes`
// closes it for `ShapeType`.

/// The `@final` six-member RTI 7.x form, as a lowered model.
fn extended_model() -> Ast {
    let path = shapes_dir().join("shapesdemo-extended.idl");
    let source = std::fs::read_to_string(&path).expect("the ShapeExtended IDL");
    model_from_idl(&source, &path.display().to_string())
        .expect("the ShapeExtended IDL lowers cleanly")
}

/// The hand-written codec's octets for one mode.
fn extended_hand_written(sample: &ShapeExtendedType, mode: Mode) -> Vec<u8> {
    let mut out = Vec::new();
    let result = match (mode.final_, mode.endianness) {
        (Encoding::PlainCdr1, Endianness::Little) => sample.encode_xcdr1(&mut out),
        (Encoding::PlainCdr1, Endianness::Big) => sample.encode_xcdr1_be(&mut out),
        (_, Endianness::Little) => sample.encode(&mut out),
        (_, Endianness::Big) => sample.encode_be(&mut out),
    };
    result.unwrap_or_else(|e| panic!("hand-written ShapeExtendedType refuses {}: {e}", mode.name));
    out
}

/// What the hand-written codec makes of octets the IDL path produced.
fn extended_hand_written_decode(bytes: &[u8], mode: Mode) -> ShapeExtendedType {
    let decoded = match (mode.final_, mode.endianness) {
        (Encoding::PlainCdr1, Endianness::Little) => ShapeExtendedType::decode_xcdr1(bytes),
        (Encoding::PlainCdr1, Endianness::Big) => ShapeExtendedType::decode_xcdr1_be(bytes),
        (_, Endianness::Little) => ShapeExtendedType::decode(bytes),
        (_, Endianness::Big) => ShapeExtendedType::decode_be(bytes),
    };
    decoded.unwrap_or_else(|e| {
        panic!(
            "hand-written ShapeExtendedType cannot read its own {} octets: {e}",
            mode.name
        )
    })
}

/// The sample as the probe's value document, member by member. The
/// enumeration is named by its IDL label, not by its discriminant: the
/// walk resolves the label against the model, so a file that renamed or
/// reordered the four constants fails here rather than agreeing on a
/// number.
fn extended_as_value(sample: &ShapeExtendedType) -> Value {
    let fill = match sample.fill_kind {
        ShapeFillKind::SolidFill => "SOLID_FILL",
        ShapeFillKind::TransparentFill => "TRANSPARENT_FILL",
        ShapeFillKind::HorizontalHatch => "HORIZONTAL_HATCH",
        ShapeFillKind::VerticalHatch => "VERTICAL_HATCH",
    };
    json!({
        "color": sample.color,
        "x": sample.x,
        "y": sample.y,
        "shapesize": sample.shapesize,
        "fillKind": fill,
        "angle": sample.angle,
    })
}

/// The samples both paths are asked for. The colours vary the string's
/// length so the pad in front of `x` differs; the four fill kinds are
/// all used, so a discriminant width or an ordering difference cannot
/// hide behind the zero one; `angle` carries a value whose four octets
/// are all distinct, which is what makes the byte order visible.
fn extended_samples() -> Vec<ShapeExtendedType> {
    vec![
        ShapeExtendedType::new("RED", 42, 77, 30, ShapeFillKind::SolidFill, 45.5),
        ShapeExtendedType::new("BLUE", -1, 270, 30, ShapeFillKind::TransparentFill, -0.125),
        ShapeExtendedType::new("A", 0, 0, 0, ShapeFillKind::HorizontalHatch, 0.0),
        ShapeExtendedType::new(
            "MAGENTA",
            i32::MIN,
            i32::MAX,
            -1,
            ShapeFillKind::VerticalHatch,
            1.234_567_9,
        ),
    ]
}

#[test]
fn hand_written_shape_extended_type_encodes_as_its_idl_in_all_four_modes() {
    let model = extended_model();
    for sample in extended_samples() {
        let values = extended_as_value(&sample);
        for mode in MODES {
            let by_hand = extended_hand_written(&sample, mode);
            let by_idl = through_idl_named(
                &model,
                "ShapeExtendedType",
                &values,
                mode.final_,
                mode.endianness,
            );
            assert_eq!(
                by_hand,
                by_idl,
                "\n{mode} disagrees for {sample:?}\n  crates/dcps/src/interop.rs            : {hand}\n  interop/shapes/shapesdemo-extended.idl: {idl}\n",
                mode = mode.name,
                hand = hex(&by_hand),
                idl = hex(&by_idl),
            );
        }
    }
}

#[test]
fn hand_written_shape_extended_type_decodes_what_its_idl_produced() {
    let model = extended_model();
    for sample in extended_samples() {
        let values = extended_as_value(&sample);
        for mode in MODES {
            let by_idl = through_idl_named(
                &model,
                "ShapeExtendedType",
                &values,
                mode.final_,
                mode.endianness,
            );
            assert_eq!(
                extended_hand_written_decode(&by_idl, mode),
                sample,
                "{} — the hand-written reader disagrees with the IDL writer",
                mode.name
            );
        }
    }
}

/// `ShapeExtendedType` is not a longer `ShapeType`. The two are
/// separate topic types (different `TYPE_NAME`), and their octets
/// differ from the first member on, because the ShapesDemo `@final`
/// form bounds nothing and this one adds two members.
#[test]
fn the_extended_form_and_the_shapesdemo_form_are_two_types() {
    let demo = shapesdemo_model();
    let extended = extended_model();
    let plain = ShapeType::new("RED", 42, 77, 30);
    let demo_value = json!({
        "color": plain.color,
        "x": plain.x,
        "y": plain.y,
        "shapesize": plain.shapesize,
    });
    let ext = ShapeExtendedType::new("RED", 42, 77, 30, ShapeFillKind::SolidFill, 0.0);
    for mode in MODES {
        let four = through_idl(&demo, &demo_value, mode.final_, mode.endianness);
        let six = through_idl_named(
            &extended,
            "ShapeExtendedType",
            &extended_as_value(&ext),
            mode.final_,
            mode.endianness,
        );
        assert_ne!(
            four, six,
            "{} — ShapeType and ShapeExtendedType must not coincide",
            mode.name
        );
    }
}
