// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `CodecTarget::enumerator_ordinal` converts an enumeration value and
//! nothing else.
//!
//! Swift and Zig have no `<` on an enumeration value. A map keyed by one,
//! ordered by enumerator value under the Fast DDS dialect, compares
//! `.rawValue` in Swift and `@intFromEnum(…)` in Zig ("operator < not
//! allowed for type 'main.Colour'" was the matrix's record for Zig on
//! `map-key-enum`). The union overlay of the Cyclone dialect asks the same
//! seam for an integer arm, and there the value stays as it is: `.rawValue`
//! on an `Int32` and `@intFromEnum` on an `i32` do not compile, and Swift
//! generated `v.whole!.rawValue` for `combined-everything` after the map
//! key fix.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_cst::common::Language;
use zerodds_lang_rules::generate;

fn model(source: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

fn source(idl: &str, language: Language, dialect: Dialect) -> String {
    let key = LayoutKey::in_dialect(dialect, Encoding::PlainCdr1, Endianness::Big, Purpose::Data);
    generate::source(&model(idl), language, key)
        .unwrap_or_else(|error| panic!("{language:?} refused the source: {error}"))
}

const MAP_KEY_ENUM: &str = "\
enum Colour { RED, GREEN, BLUE };
@final struct Sample { map<Colour, long> m; };
";

// A `short` discriminator: under the Cyclone dialect its negative label
// selects nothing and the overlay writes the arm as `default:`'s storage
// (`zerodds_ast_layout::union_negative_matches_nothing`); a `long` one
// keeps the label.
const OVERLAY: &str = "\
union Choice switch (short) {
    case -1: long whole;
    default: double fraction;
};
@final struct Sample { Choice picked; };
";

#[test]
fn swift_orders_an_enumeration_key_by_its_raw_value() {
    let text = source(MAP_KEY_ENUM, Language::Swift, Dialect::FastDds);
    assert!(
        text.contains(".key.rawValue < "),
        "no rawValue comparison:\n{text}"
    );
}

#[test]
fn zig_orders_an_enumeration_key_by_its_tag() {
    let text = source(MAP_KEY_ENUM, Language::Zig, Dialect::FastDds);
    assert!(
        text.contains("@intFromEnum(v[zd_k].key) < @intFromEnum(v[i].key)"),
        "no @intFromEnum comparison:\n{text}"
    );
}

#[test]
fn swift_leaves_an_integer_overlay_arm_as_it_is() {
    let text = source(OVERLAY, Language::Swift, Dialect::Cyclone);
    assert!(
        text.contains("writeUnionOverlay(zdBitPatternInt(zdTo(Int64.self, v.whole!), 4), 8)"),
        "overlay write changed:\n{text}"
    );
    assert!(!text.contains("whole!.rawValue"), "{text}");
}

#[test]
fn zig_leaves_an_integer_overlay_arm_as_it_is() {
    let text = source(OVERLAY, Language::Zig, Dialect::Cyclone);
    assert!(
        text.contains("writeUnionOverlay(zdwire.bitPatternInt(zdwire.asI64(v.whole), 4), 8)"),
        "overlay write changed:\n{text}"
    );
    assert!(!text.contains("@intFromEnum(v.whole)"), "{text}");
}
