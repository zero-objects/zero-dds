// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The reach test and the renderers say the same thing.
//!
//! `walk` refuses a case whose operations a binding's `Surface` does not
//! admit, and each renderer refuses an operation it has no call for.
//! Two lists, one truth: a renderer that refused something the surface
//! admits would turn a measurable cell into a `generator-defect`, and a
//! surface that admitted something no renderer writes would do the same
//! one step later. So the admitted set is built here from the surface
//! and pushed through the renderer.
//!
//! The set is now nearly the whole of `Op`, which is the point of the
//! work this file was rewritten for: fourteen of the fifteen bindings
//! had no delimiter, no member header and no way off the four-octet
//! alignment cap, and the surface table that recorded it has one field
//! left.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_wireprobe_endpoint_gen::ops::{Form, Int, Lc, Op, Program, Real, Version};
use zerodds_wireprobe_endpoint_gen::render::render;
use zerodds_wireprobe_endpoint_gen::surface::Language;

/// A `float` argument in both spellings, from one `binary32`.
fn real(value: f32) -> Real {
    Real {
        value,
        text: value.to_string(),
    }
}

/// A body of exactly `octets` octets, for a length code that fixes one.
fn body(octets: usize) -> Vec<Op> {
    match octets {
        1 => vec![Op::U8(Int::new(1, 1))],
        2 => vec![Op::U16(Int::new(2, 2))],
        4 => vec![Op::U32(Int::new(4, 4))],
        _ => vec![Op::U64(Int::new(8, 8))],
    }
}

/// Every operation this binding's surface admits, once each.
fn admitted(lang: Language) -> Vec<Op> {
    let surface = lang.surface();
    let mut ops = vec![
        Op::U8(Int::new(0xab, -85)),
        Op::Bool(true),
        Op::Bool(false),
        Op::Char(Int::new(0x7a, 0x7a)),
        Op::Wchar(Int::new(0x1234, 0x1234)),
        Op::U16(Int::new(0xbeef, -16657)),
        Op::U32(Int::new(0xdead_beef, -559_038_737)),
        Op::U64(Int::new(0xdead_beef_dead_c0de, -2_401_053_092_612_311_330)),
        Op::Count(Int::new(3, 3)),
        Op::F32(real(-3.75)),
        Op::Str(vec![0xc3, 0xbc]),
        Op::Str(Vec::new()),
        Op::SeqU8(vec![1, 2, 3]),
        Op::SeqU8(Vec::new()),
        // The units of "äZ" and the empty one: a length that counts
        // octets has to be exercised at zero as well.
        Op::Wstr(vec![0x00e4, 0x005a]),
        Op::Wstr(Vec::new()),
        Op::Fixed {
            value: "-000000.001".to_string(),
            digits: 9,
            scale: 3,
        },
        Op::Fixed {
            value: "123456.789".to_string(),
            digits: 9,
            scale: 3,
        },
    ];
    for anchor in [1u8, 2, 4, 8] {
        ops.push(Op::Bitset {
            bits: 0xbc61_4e00_000f_ffff >> (8 * (8 - u32::from(anchor))),
            anchor,
        });
    }
    if surface.f64 {
        ops.push(Op::F64(core::f64::consts::PI));
    }
    ops.push(Op::DHeaderBegin);
    ops.push(Op::U32(Int::new(1, 1)));
    ops.push(Op::DHeaderEnd);
    // All eight length codes, each with a body its code admits. LC0..LC3
    // fix the size; LC4 takes any; LC5..LC7 need a leading word of their
    // own, which a string and a counted collection both have.
    for (code, body_ops) in [
        (0u8, body(1)),
        (1, body(2)),
        (2, body(4)),
        (3, body(8)),
        (4, vec![Op::U16(Int::new(1, 1)), Op::U16(Int::new(2, 2))]),
        (5, vec![Op::Str(vec![b'a'])]),
        (6, vec![Op::Count(Int::new(1, 1)), Op::U32(Int::new(7, 7))]),
        (7, vec![Op::Count(Int::new(1, 1)), Op::U64(Int::new(7, 7))]),
    ] {
        ops.push(Op::EmHeaderBegin {
            id: u32::from(code),
            must_understand: code % 2 == 0,
            lc: Lc(code),
        });
        ops.extend(body_ops);
        ops.push(Op::EmHeaderEnd { lc: Lc(code) });
    }
    ops.push(Op::PidBegin {
        pid: 3,
        must_understand: true,
    });
    ops.push(Op::U32(Int::new(1, 1)));
    ops.push(Op::PidEnd);
    ops.push(Op::PidExtendedBegin {
        member_id: 268_435_455,
        must_understand: false,
    });
    ops.push(Op::U32(Int::new(1, 1)));
    ops.push(Op::PidEnd);
    ops.push(Op::PidListEnd);
    ops
}

#[test]
fn every_language_renders_everything_its_surface_admits() {
    for lang in Language::ALL {
        for (version, big_endian, form) in [
            (Version::Xcdr2, false, Form::Bits),
            (Version::Xcdr2, true, Form::Bits),
            (Version::Xcdr1, false, Form::Natural),
            (Version::Xcdr1, true, Form::Natural),
        ] {
            let program = Program {
                ops: admitted(lang),
                version,
            };
            let rendered = render(lang, &program, big_endian, form);
            assert!(
                rendered.is_ok(),
                "{}: the surface admits an operation the renderer refuses: {:?}",
                lang.as_str(),
                rendered.err()
            );
        }
    }
}

/// The encoding reaches the constructor in every one of the fifteen.
///
/// XTypes 1.3 §7.4.1 puts MAXALIGN at eight under XCDR1 and four under
/// XCDR2, and a program that did not say which one it wanted would be
/// measuring whichever the binding happened to default to. The two
/// renderings of the same operation list must therefore differ.
#[test]
fn the_two_encodings_render_differently_everywhere() {
    for lang in Language::ALL {
        let ops = vec![Op::U8(Int::new(1, 1)), Op::U64(Int::new(2, 2))];
        let one = render(
            lang,
            &Program {
                ops: ops.clone(),
                version: Version::Xcdr1,
            },
            false,
            Form::Bits,
        )
        .expect("xcdr1 renders");
        let two = render(
            lang,
            &Program {
                ops,
                version: Version::Xcdr2,
            },
            false,
            Form::Bits,
        )
        .expect("xcdr2 renders");
        assert_ne!(
            one,
            two,
            "{}: the same program renders identically under both encodings, so the \
             encoding never reached the binding",
            lang.as_str()
        );
    }
}

/// Nested framing needs one local per open header, and every one of the
/// fifteen can nest now.
#[test]
fn nested_framing_declares_one_local_per_open_header() {
    let program = Program {
        ops: vec![
            Op::DHeaderBegin,
            Op::EmHeaderBegin {
                id: 0,
                must_understand: false,
                lc: Lc(4),
            },
            Op::DHeaderBegin,
            Op::U32(Int::new(1, 1)),
            Op::DHeaderEnd,
            Op::EmHeaderEnd { lc: Lc(4) },
            Op::DHeaderEnd,
        ],
        version: Version::Xcdr2,
    };
    let text = render(Language::AdaNative, &program, false, Form::Bits)
        .expect("ada-native renders framing");
    for slot in ["B0", "B1", "B2"] {
        assert!(
            text.contains(&format!("{slot} : Natural := 0;")),
            "no local for {slot} in:\n{text}"
        );
    }
    // Three opens and three closes: two delimiters and one member
    // header, each closed against the local it opened.
    assert_eq!(text.matches("   DHeader_Begin (W, B").count(), 2);
    assert_eq!(text.matches("   DHeader_End (W, B").count(), 2);
    // The innermost header is closed first, so its local is the last
    // one opened and the first one released.
    let inner = text.find("DHeader_End (W, B2);").expect("B2 closed");
    let outer = text.find("DHeader_End (W, B0);").expect("B0 closed");
    assert!(inner < outer);

    // Every one of the fifteen renders the same nesting without a
    // refusal, which is what the fourteen that had no framing at all
    // could not do.
    for lang in Language::ALL {
        assert!(
            render(lang, &program, false, Form::Bits).is_ok(),
            "{} cannot nest a delimiter inside a member header",
            lang.as_str()
        );
    }
}

/// Ada rejects `4e0`: a real literal needs the point. Every literal this
/// generator emits carries one, and an uppercase exponent, so the same
/// text parses in all fifteen front ends.
#[test]
fn real_literals_carry_a_point_in_every_front_end() {
    let program = Program {
        ops: vec![
            Op::F32(real(4.0)),
            Op::F32(real(-3.75)),
            Op::F32(real(1.0e-30)),
        ],
        version: Version::Xcdr2,
    };
    let text = render(Language::Ada83, &program, false, Form::Bits).expect("ada-83 renders floats");
    for line in text.lines().filter(|line| line.contains("Put_F32")) {
        let literal = line
            .rsplit_once(", ")
            .map(|(_, rest)| rest.trim_end_matches(");"))
            .unwrap_or_default();
        assert!(literal.contains('.'), "`{literal}` has no point");
        assert!(literal.contains('E'), "`{literal}` has no exponent");
        assert!(
            !literal.contains('e'),
            "`{literal}` has a lowercase exponent"
        );
    }
}

/// The literal each renderer hands `put_f32` for one corpus decimal.
///
/// `0.30000001` is not the shortest decimal of any `binary32`: it reads
/// back as `0.3f32`, whose shortest decimal is `0.3`, which this
/// generator's `real32` spells `3.0E-1`. So a renderer that handed over
/// `Real::value` would carry `3.0E-1` and one that handed over
/// `Real::text` carries the corpus digits. The fifteen hand over the
/// corpus digits; `java`, which is not one of them, is pinned to the
/// narrowed value so the boundary is explicit.
fn f32_literal(lang: Language, form: Form) -> &'static str {
    match (lang, form) {
        (Language::Ada, _) => "Zdw.Put_F32 (W'Access, 0.30000001E0));",
        (Language::Ada83 | Language::AdaNative, _) => "Put_F32 (W, 0.30000001E0);",
        (Language::D, _) => "w.putF32(cast(float) 0.30000001);",
        (Language::Nim, _) => "w.putF32(float32(0.30000001))",
        (Language::Zig, _) => "try w.putF32(0.30000001);",
        (Language::Kotlin, _) => "w.putF32(0.30000001f)",
        (Language::Node, _) => "w.putF32(0.30000001);",
        (Language::FSharp, _) => "w.PutF32(0.30000001f)",
        (Language::Swift, _) => "w.putF32(0.30000001)",
        (Language::Go, _) => "w.PutF32(0.30000001)",
        (Language::Julia, Form::Bits) => "ZeroDDS.put_f32!(W, 0.30000001f0)",
        (Language::Julia, Form::Natural) => "ZeroDDS.put_f32!(W, 0.30000001)",
        (Language::Lua, _) => "w:putF32(0.30000001)",
        (Language::OCaml, _) => "Zerodds.Wire.put_f32 w (0.30000001);",
        (Language::Elixir, _) => "ZeroDDS.Wire.put_f32(0.30000001)",
        (Language::Python, _) => "w.put_f32(0.30000001)",
        (Language::Java, _) => "w.f32(3.0E-1f);",
    }
}

/// Every renderer spells the corpus decimal for a `float`, in the form
/// its front end parses: the seven that take a lowercase `e` unchanged,
/// the three Ada bindings through an Ada real literal, and Julia through
/// its own `f` exponent. This is the pin the ten brought onto the corpus
/// decimal were missing.
#[test]
fn every_renderer_hands_put_f32_the_literal_its_front_end_needs() {
    let program = Program {
        ops: vec![Op::F32(Real {
            value: 0.3,
            text: "0.30000001".to_string(),
        })],
        version: Version::Xcdr2,
    };
    for lang in Language::ALL {
        for form in [Form::Bits, Form::Natural] {
            let text = render(lang, &program, false, form)
                .unwrap_or_else(|error| panic!("{}: {error}", lang.as_str()));
            let expected = f32_literal(lang, form);
            assert!(
                text.contains(expected),
                "{} ({form:?}): expected `{expected}` in:\n{text}",
                lang.as_str()
            );
        }
    }
}

/// A negative corpus decimal keeps its sign, and the two Ada bindings
/// that need the unary minus made visible still emit the `use type`
/// clause — now driven by the literal's sign and not by the narrowed
/// `binary32`.
#[test]
fn a_negative_corpus_decimal_keeps_its_sign() {
    let program = Program {
        ops: vec![Op::F32(Real {
            value: -0.0,
            text: "-0.0".to_string(),
        })],
        version: Version::Xcdr2,
    };
    let ada = render(Language::Ada, &program, false, Form::Bits).expect("ada renders");
    assert!(
        ada.contains("use type Interfaces.C.C_float;"),
        "ada has no `use type` for the negative literal:\n{ada}"
    );
    assert!(
        ada.contains("Zdw.Put_F32 (W'Access, -0.0E0));"),
        "ada did not keep the negative zero:\n{ada}"
    );
    let native =
        render(Language::AdaNative, &program, false, Form::Bits).expect("ada-native renders");
    assert!(
        native.contains("use type Interfaces.IEEE_Float_32;"),
        "ada-native has no `use type` for the negative literal:\n{native}"
    );
    assert!(
        native.contains("Put_F32 (W, -0.0E0);"),
        "ada-native did not keep the negative zero:\n{native}"
    );
}

/// The SDK source a binding's generated program is compiled against. Only
/// the four whose framing spelling this file now pins.
fn sdk_source(lang: Language) -> String {
    let relative = match lang {
        Language::Kotlin => "endpoints/kotlin/src/Zerodds.kt",
        Language::Nim => "endpoints/nim/zerodds.nim",
        Language::Swift => "endpoints/swift/Sources/Zerodds/Zerodds.swift",
        Language::Zig => "endpoints/zig/src/zerodds.zig",
        other => panic!("no SDK source named for {}", other.as_str()),
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..");
    let path = root.join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The binding exports the operation under this declaration.
fn assert_sdk_has(lang: Language, declaration: &str) {
    let source = sdk_source(lang);
    assert!(
        source.contains(declaration),
        "{} does not declare `{declaration}`",
        lang.as_str()
    );
}

/// The framing calls, in the spelling the SDK the program is compiled
/// against actually exports.
///
/// The reach test above only asks that a renderer returns `Ok`. A
/// `LengthCode` passed as an integer, a nested `Pid` named unqualified or
/// a `void` function wrapped in `try` all render without an error and
/// fail only when the binding's compiler sees them, which is how four
/// bindings diverged unnoticed. These checks read the SDK source and the
/// rendered text together, which is the agreement the compile on the
/// measurement host enforces.
#[test]
fn framing_calls_name_what_the_sdk_exports() {
    let program = Program {
        ops: vec![
            Op::DHeaderBegin,
            Op::EmHeaderBegin {
                id: 0,
                must_understand: false,
                lc: Lc(4),
            },
            Op::U32(Int::new(1, 1)),
            Op::EmHeaderEnd { lc: Lc(4) },
            Op::Char(Int::new(0x7a, 0x7a)),
            Op::Wchar(Int::new(0x1234, 0x1234)),
            Op::DHeaderEnd,
            Op::PidBegin {
                pid: 3,
                must_understand: false,
            },
            Op::U32(Int::new(1, 1)),
            Op::PidEnd,
        ],
        version: Version::Xcdr2,
    };
    let rendered = |lang: Language| {
        render(lang, &program, true, Form::Bits)
            .unwrap_or_else(|error| panic!("{}: {error}", lang.as_str()))
    };

    // Zig exports `emHeaderBegin`/`emHeaderEnd` (capital H), takes the
    // code as its own `LengthCode`, and its `dheaderEnd` answers `void`.
    let zig = rendered(Language::Zig);
    assert_sdk_has(Language::Zig, "pub fn emHeaderBegin(");
    assert!(
        zig.contains("w.emHeaderBegin(0, false, .lc4)"),
        "zig EMHEADER open:\n{zig}"
    );
    assert!(
        zig.contains("try w.emHeaderEnd(b1, .lc4);"),
        "zig EMHEADER close:\n{zig}"
    );
    assert!(
        zig.contains("w.dheaderEnd(b0);"),
        "zig DHEADER close:\n{zig}"
    );
    assert!(
        !zig.contains("try w.dheaderEnd"),
        "zig wraps the void `dheaderEnd` in `try`:\n{zig}"
    );
    assert!(
        !zig.contains("emheaderBegin"),
        "zig uses the lowercase name:\n{zig}"
    );
    assert!(
        zig.contains("zd.Writer.Pid"),
        "zig names the nested Pid unqualified:\n{zig}"
    );

    // Nim's `LengthCode` is an enum too; its `Pid` is a top-level object,
    // so only the code spelling was wrong.
    let nim = rendered(Language::Nim);
    assert_sdk_has(Language::Nim, "proc emHeaderBegin*(");
    assert!(
        nim.contains("w.emHeaderBegin(0'u32, false, lc4)"),
        "nim EMHEADER open:\n{nim}"
    );
    assert!(
        nim.contains("w.emHeaderEnd(b1, lc4)"),
        "nim EMHEADER close:\n{nim}"
    );
    assert!(
        !nim.contains("false, 4)"),
        "nim passes the length code as an integer:\n{nim}"
    );

    // Kotlin's `Pid` is the nested class `Writer.Pid`, not `zerodds.Pid`.
    let kotlin = rendered(Language::Kotlin);
    assert_sdk_has(Language::Kotlin, "class Pid(");
    assert!(
        kotlin.contains("import zerodds.Writer.Pid"),
        "kotlin Pid import:\n{kotlin}"
    );
    assert!(
        kotlin.contains("var p0: Pid"),
        "kotlin Pid local:\n{kotlin}"
    );
    assert!(
        !kotlin.contains("import zerodds.Pid\n"),
        "kotlin imports a top-level Pid:\n{kotlin}"
    );

    // Swift's member header takes a `LengthCode` and throws, its `Pid` is
    // nested, and `putChar`/`putWchar` refuse by throwing.
    let swift = rendered(Language::Swift);
    assert_sdk_has(Language::Swift, "func emheaderBegin(");
    assert_sdk_has(Language::Swift, "func putChar(");
    assert!(
        swift.contains("b1 = try w.emheaderBegin(id: 0, mustUnderstand: false, lengthCode: .lc4)"),
        "swift EMHEADER open:\n{swift}"
    );
    assert!(
        swift.contains("try w.emheaderEnd(b1, lengthCode: .lc4)"),
        "swift EMHEADER close:\n{swift}"
    );
    assert!(
        swift.contains("try w.putChar(122)"),
        "swift char write:\n{swift}"
    );
    assert!(
        swift.contains("try w.putWchar(4660)"),
        "swift wchar write:\n{swift}"
    );
    assert!(
        swift.contains("var p0: Writer.Pid? = nil"),
        "swift names the nested Pid unqualified:\n{swift}"
    );
    assert!(
        !swift.contains("lengthCode: 4"),
        "swift passes the length code as an integer:\n{swift}"
    );
}
