// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The side tables and surfaces of DDS-TS 1.0 chapter 7.
//!
//! A TypeScript type states less than an IDL declaration does. It has no
//! length for an array, no bound for a string, a sequence or a map, no
//! width for a bitfield, no bit bound for a bitmask, and no value for a
//! `@default`. The clause answers that with exported constants beside
//! the declaration — §7.3.1, §7.7, §7.8, §7.9, §7.9.1 and §7.11 — and
//! this measures that each of them reaches the output and says the right
//! number.
//!
//! Three surfaces are measured here too, because they are the shape of
//! the generated code rather than a number beside it: an enumeration is
//! a string-literal union (§7.6), a union arm carries `discriminator`
//! (§7.5), and a map is a `ReadonlyMap` (§7.9.1).
//!
//! [`the_side_tables_and_their_codecs_type_check`] is the other half:
//! `tsc --strict` over the declarations **and both codec directions**
//! for each shape, beside the real runtime. A constant that is in the
//! text and does not compile is not an emission.
// `print_stdout`: a missing compiler has to be said on the harness's own
// channel, never passed over quietly.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use zerodds_ast::Ast;
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::default_typescript::typescript_defaulters_called_by;
use zerodds_lang_rules::emit::decode::decoder_from;
use zerodds_lang_rules::emit::encode::codec_from;
use zerodds_lang_rules::emit::typescript::TypeScript;
use zerodds_lang_rules::{ModelGraph, apply};

mod typescript_toolchain;

use typescript_toolchain::compiler_or_note;

/// One IDL source, lowered and resolved.
fn model(source: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(source);
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
    ast.resolve_all_by_name().expect("the model resolves");
    ast
}

/// The declarations one model materialises, rendered.
fn declarations(source: &str) -> String {
    let ast = model(source);
    let graph = ModelGraph::of(&ast);
    let applied =
        apply(&graph, &zerodds_lang_rules::rule::typescript::catalogue()).expect("the rules apply");
    zerodds_lang_rules::adapter::typescript::ts_items(&ast, &applied)
        .expect("the model materialises")
        .iter()
        .map(zerodds_lang_render::typescript::decl)
        .collect::<Vec<_>>()
        .join("\n")
}

/// That every one of `wanted` is in the materialised declarations.
fn holds(section: &str, source: &str, wanted: &[&str]) -> String {
    let text = declarations(source);
    for expected in wanted {
        assert!(
            text.contains(expected),
            "{section}: `{expected}` is not in the output:\n{text}"
        );
    }
    text
}

/// The named type of a model.
fn root(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    ast.types()
        .find(|(_, node)| node.name.as_str() == name)
        .map(|(id, _)| id)
        .expect("the root type")
}

/// The whole of one model as a TypeScript file: the runtime import, the
/// prelude, the declarations, the defaulters and **both** codec
/// directions.
fn generated_file(source: &str, type_name: &str) -> String {
    let ast = model(source);
    let graph = ModelGraph::of(&ast);
    let applied =
        apply(&graph, &zerodds_lang_rules::rule::typescript::catalogue()).expect("the rules apply");
    let id = root(&ast, type_name);
    let key = LayoutKey::in_dialect(
        Dialect::Specification,
        Encoding::PlCdr2,
        Endianness::Little,
        Purpose::Data,
    );
    let mut decls = zerodds_lang_rules::adapter::typescript::prelude();
    decls.extend(
        zerodds_lang_rules::adapter::typescript::ts_items(&ast, &applied)
            .expect("the model materialises"),
    );
    let mut codec =
        codec_from::<TypeScript>(&ast, id, key, Default::default()).expect("the write walk ends");
    codec.extend(
        decoder_from::<TypeScript>(&ast, id, key, Default::default()).expect("the read walk ends"),
    );
    decls.extend(
        typescript_defaulters_called_by(&ast, &codec).expect("Table 9 for every type reached"),
    );
    decls.extend(codec);
    let body = decls
        .iter()
        .map(zerodds_lang_render::typescript::decl)
        .collect::<Vec<_>>()
        .join("\n");
    format!("{RUNTIME_IMPORT}\n{body}\n")
}

/// The runtime names a generated file may reach, imported wholesale.
///
/// Which of them a file needs is the layout key's answer and not the
/// type's, so the probe
/// (`tools/wireprobe/wireprobe-zerodds-typescript-gen`) imports the same
/// set for the same reason: a file that imported only what one key
/// needed would not compile under another. The tree sets no
/// `noUnusedLocals`, and an unused `type` import is erased entirely.
const RUNTIME_IMPORT: &str = "import {\n  \
     Endianness, FixedDigitCount, ParameterRole, PID_FLAGS_BARE,\n  \
     PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS, PID_FLAGS_SENTINEL_MUST_UNDERSTAND,\n  \
     PID_SHORT_MAX_ID, TryConstruct, WcharFraming, WcharWidth, Writer,\n  \
     WstringLengthPolicy, WstringUnitOrder, float128FromDecimalString, noDheader,\n  \
     bitPatternBool, bitPatternF32, bitPatternF64, bitPatternInt,\n  \
     zdBigInt, zdChar, zdNumber, Reader, Status as ZdReaderStatus,\n  \
     pidIsUserMember, utf8Octets, utf16Units,\n  \
     type DHeader, type EmHeader, type Pid, type PlCdr1Framing,\n  \
     type EmHeaderRead, type PidRead, type ReaderMark,\n\
     } from \"../src/index.js\";\n";

// -- §7.3.1 Bounded Strings ----------------------------------------------

#[test]
fn s_7_3_1_a_bounded_string_states_its_bound() {
    let text = holds(
        "§7.3.1",
        "typedef string<128> TopicName; \
         @final struct S { string<8> narrow; wstring<16> wide; string open; };",
        &[
            "export const S_narrow_BOUND = 8;",
            "export const S_wide_BOUND = 16;",
            // A standalone bounded-string typedef takes the alias's own
            // name.
            "export const TopicName_BOUND = 128;",
        ],
    );
    // An unbounded string has no bound to state.
    assert!(!text.contains("S_open_BOUND"), "{text}");
}

/// A bound that will not fold is named, not read as "unbounded".
///
/// `string<1/0>` parses, lowers without a diagnostic and reaches the
/// materialiser with `eval_integral` failing — the door defect 5 of
/// `docs/silent-drop-sweep.md` came through for an array length. There
/// are two places the answer would go missing: the `_BOUND` constant
/// §7.3.1 asks for is simply not written, and the descriptor's
/// `DdsTypeRef` carries no `bound` property (§8.2). A consumer of either
/// reads exactly what `string open` produces, so a declaration the IDL
/// bounded arrives as one that is not, from a generator that reported
/// success.
#[test]
fn s_7_3_1_a_bound_that_does_not_evaluate_is_refused_and_not_read_as_unbounded() {
    for source in [
        "@final struct S { string<1/0> narrow; };",
        "@final struct S { sequence<long, 1/0> many; };",
        "typedef string<1/0> TopicName; @final struct S { TopicName t; };",
    ] {
        let ast = model(source);
        let graph = ModelGraph::of(&ast);
        let applied = apply(&graph, &zerodds_lang_rules::rule::typescript::catalogue())
            .expect("the rules apply");
        // Both halves: the types alone, and the types with their
        // descriptor side-tables. The bound reaches the output through
        // two separate walks and either one dropping it is the defect.
        for materialised in [
            zerodds_lang_rules::adapter::typescript::ts_items(&ast, &applied),
            zerodds_lang_rules::adapter::typescript::ts_items_with_descriptors(&ast, &applied),
        ] {
            let error = materialised
                .err()
                .unwrap_or_else(|| panic!("the bound is refused, not dropped: {source}"));
            let said = format!("{error}");
            assert!(
                said.contains("collection bound"),
                "the refusal names the construct: {said}"
            );
        }
    }
}

// -- §7.5 Union Mapping --------------------------------------------------

#[test]
fn s_7_5_a_union_arm_names_its_discriminator() {
    let text = holds(
        "§7.5",
        "@final union Choice switch (long) { case 1: long one; case 2: long two; \
         default: short other; };
         @final struct S { Choice pick; };",
        &[
            "{ discriminator: 1; one: number }",
            "{ discriminator: 2; two: number }",
            // The `default:` arm is the discriminator's type minus the
            // labels the other cases claim.
            "{ discriminator: Exclude<number, 1 | 2>; other: number }",
        ],
    );
    // `_d` was the property before §7.5 was read; nothing writes it now.
    assert!(!text.contains("_d:"), "{text}");
}

#[test]
fn s_7_5_an_enumeration_discriminator_narrows_on_the_enumerator() {
    holds(
        "§7.5",
        "enum Tag { TAG_A, TAG_B, TAG_C }; \
         @final union Variant switch (Tag) { case TAG_A: long i; case TAG_B: string s; \
         default: octet raw; };
         @final struct S { Variant v; };",
        &[
            "{ discriminator: typeof Tag.TAG_A; i: number }",
            "{ discriminator: typeof Tag.TAG_B; s: string }",
            "{ discriminator: Exclude<Tag, typeof Tag.TAG_A | typeof Tag.TAG_B>; raw: number }",
        ],
    );
}

// -- §7.6 Enum Mapping ---------------------------------------------------

#[test]
fn s_7_6_an_enumeration_is_a_string_literal_union_with_its_ordinals() {
    let text = holds(
        "§7.6",
        "enum Op { @value(0) ADD, @value(1) SUB, @value(7) NEG };",
        &[
            "export const Op = { ADD: \"ADD\", SUB: \"SUB\", NEG: \"NEG\" } as const;",
            "export type Op = (typeof Op)[keyof typeof Op];",
            "export const OpOrdinal: Readonly<Record<Op, number>> = \
             { ADD: 0, SUB: 1, NEG: 7 } as const;",
            "export const OpFromOrdinal: ReadonlyMap<number, Op> = \
             new Map([[0, Op.ADD], [1, Op.SUB], [7, Op.NEG]] as const);",
        ],
    );
    // No TypeScript `enum` keyword, and no numeric alias.
    assert!(!text.contains("export enum "), "{text}");
    assert!(!text.contains("export type Op = number;"), "{text}");
}

#[test]
fn s_7_6_implicit_ordinals_are_emitted_too() {
    holds(
        "§7.6",
        "enum Colour { RED, GREEN, BLUE };",
        &[
            "export const ColourOrdinal: Readonly<Record<Colour, number>> = \
             { RED: 0, GREEN: 1, BLUE: 2 } as const;",
        ],
    );
}

// -- §7.7 Bitset Mapping -------------------------------------------------

#[test]
fn s_7_7_a_bitset_states_the_width_of_every_bitfield() {
    holds(
        "§7.7",
        "bitset Flags { bitfield<3> low; bitfield<5> high; bitfield<40> wide; };",
        &[
            "export const Flags_low_BITS = 3;",
            "export const Flags_high_BITS = 5;",
            "export const Flags_wide_BITS = 40;",
            // The clause's holder rule, which the constants sit beside.
            "low: number;",
            "wide: bigint;",
        ],
    );
}

// -- §7.8 Bitmask Mapping ------------------------------------------------

#[test]
fn s_7_8_a_bitmask_states_its_bit_bound() {
    holds(
        "§7.8",
        "bitmask Permissions { READ, WRITE, EXEC }; \
         @bit_bound(48) bitmask BigFlags { @position(0) flag00, @position(47) flag47 };",
        &[
            "export const Permissions_BIT_BOUND = 32;",
            "export const BigFlags_BIT_BOUND = 48;",
        ],
    );
}

// -- §7.9 Sequence and Array Mapping -------------------------------------

#[test]
fn s_7_9_a_bounded_sequence_and_an_array_state_their_sizes() {
    let text = holds(
        "§7.9",
        "typedef sequence<octet, 1024> ByteBuffer; \
         typedef long Row[4]; \
         @final struct Sample { sequence<long, 10> pixels; sequence<string> tags; \
         double v[3]; long grid[2][3]; };",
        &[
            "export const Sample_pixels_BOUND = 10;",
            "export const Sample_v_LENGTH = 3;",
            "export const Sample_grid_LENGTH_DIM1 = 2;",
            "export const Sample_grid_LENGTH_DIM2 = 3;",
            // The alias forms of §7.10.
            "export const ByteBuffer_BOUND = 1024;",
            "export const Row_LENGTH = 4;",
        ],
    );
    // An unbounded sequence has no bound to state.
    assert!(!text.contains("Sample_tags_BOUND"), "{text}");
    // A one-dimensional array takes `_LENGTH` and never `_LENGTH_DIM1`.
    assert!(!text.contains("Sample_v_LENGTH_DIM"), "{text}");
}

// -- §7.9.1 Maps ---------------------------------------------------------

#[test]
fn s_7_9_1_a_map_is_a_readonly_map_and_states_its_bound() {
    let text = holds(
        "§7.9.1",
        "@final struct Index { map<string, long, 1024> by_name; map<long, string> free; };",
        &[
            "by_name: ReadonlyMap<string, number>;",
            "free: ReadonlyMap<number, string>;",
            "export const Index_by_name_BOUND = 1024;",
        ],
    );
    // Never a `Record`: it loses key uniqueness and commits to an
    // enumeration order.
    assert!(!text.contains("Record<string, number>"), "{text}");
    // An unbounded map has no bound to state.
    assert!(!text.contains("Index_free_BOUND"), "{text}");
}

// -- §7.11 Optional and Default Values -----------------------------------

#[test]
fn s_7_11_a_default_reaches_a_side_table_constant() {
    let text = holds(
        "§7.11",
        "@final struct Config { @default(5000) long timeout_ms; @default(\"on\") string mode; \
         @default(TRUE) boolean live; long plain; };",
        &[
            "export const Config_timeout_ms_DEFAULT: number = 5000;",
            "export const Config_mode_DEFAULT: string = \"on\";",
            "export const Config_live_DEFAULT: boolean = true;",
            // The annotation changes no interface shape.
            "timeout_ms: number;",
            "mode: string;",
        ],
    );
    // A member with no `@default` gets no constant.
    assert!(!text.contains("Config_plain_DEFAULT"), "{text}");
}

#[test]
fn s_7_11_an_optional_member_is_a_question_mark_and_undefined() {
    holds(
        "§7.11",
        "@final struct Frame { long seq; @optional long retry; };",
        &["retry?: number | undefined;"],
    );
}

// -- the compiler --------------------------------------------------------

/// Every shape above — declarations, defaulters and both codec
/// directions — through `tsc` under the settings the probe compiles a
/// generated file with.
///
/// One test and not one per shape, because `tsc` costs seconds and the
/// failure names the file it came from.
#[test]
fn the_side_tables_and_their_codecs_type_check() {
    let shapes: &[(&str, &str, &str)] = &[
        (
            "side-strings",
            "typedef string<128> TopicName; \
             @final struct S { string<8> narrow; wstring<16> wide; TopicName topic; };",
            "S",
        ),
        (
            "side-collections",
            "typedef sequence<octet, 1024> ByteBuffer; \
             @final struct S { sequence<long, 10> pixels; sequence<string> tags; \
             double v[3]; long grid[2][3]; ByteBuffer raw; };",
            "S",
        ),
        (
            "side-map",
            "@final struct S { map<string, long, 1024> by_name; map<long, string> free; };",
            "S",
        ),
        (
            "side-map-struct-key",
            "@final struct K { long a; }; @final struct S { map<K, long, 8> by_key; };",
            "S",
        ),
        (
            "side-map-in-sequence",
            "@final struct S { sequence<map<long, long>> rows; };",
            "S",
        ),
        (
            "side-enum",
            "enum Op { @value(0) ADD, @value(1) SUB, @value(7) NEG }; \
             enum Colour { RED, GREEN, BLUE }; \
             @final struct S { Op which; Colour shade; };",
            "S",
        ),
        (
            "side-enum-union",
            "enum Tag { TAG_A, TAG_B, TAG_C }; \
             @final union Variant switch (Tag) { case TAG_A: long i; case TAG_B: string s; \
             default: octet raw; }; \
             @final struct S { Variant v; };",
            "S",
        ),
        (
            "side-union-integer",
            "@final union Choice switch (long) { case 1: long one; case 2: long two; \
             default: short other; }; \
             @final union Closed switch (long) { case 1: long one; }; \
             @final struct S { Choice pick; Closed only; };",
            "S",
        ),
        (
            "side-bits",
            "bitset Flags { bitfield<3> low; bitfield<5> high; bitfield<40> wide; }; \
             bitmask Permissions { READ, WRITE, EXEC }; \
             @bit_bound(48) bitmask BigFlags { @position(0) flag00, @position(47) flag47 }; \
             @final struct S { Flags f; Permissions p; BigFlags b; };",
            "S",
        ),
        (
            "side-defaults",
            "@final struct S { @default(5000) long timeout_ms; @default(\"on\") string mode; \
             @default(TRUE) boolean live; @optional long retry; };",
            "S",
        ),
    ];
    let Some(tsc) = compiler_or_note("the TypeScript column") else {
        return;
    };
    let mut reported: Vec<String> = Vec::new();
    for (name, source, type_name) in shapes {
        let report = typescript_toolchain::typecheck(
            &tsc,
            name,
            &generated_file(source, type_name),
            // A side table is indexed and its optional members are the
            // thing under test, so this gate asks for two flags beyond
            // the shared set.
            &["--noUncheckedIndexedAccess", "--exactOptionalPropertyTypes"],
        );
        if !report.trim().is_empty() {
            reported.push(format!("-- {name}\n{report}"));
        }
    }
    assert!(
        reported.is_empty(),
        "the generated side tables do not compile:\n{}",
        reported.join("\n")
    );
}
