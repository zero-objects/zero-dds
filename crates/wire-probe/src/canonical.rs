// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The one form two implementations' values can be compared in — at
//! `serde_json::Value`.
//!
//! `zerodds_wire_codec::canonical` walks the type beside the value over
//! any representation; this pins it to the value file's own, so that the
//! gates, `tools/interop-results` and the C-column harness keep calling
//! `canonicalise(&ast, type_, &value, path)` and
//! `text(&value)` and get back a `serde_json::Value` and a `String`
//! rather than an inference failure.
//!
//! The recursion markers below say the same thing `contract.rs`'s
//! module note says: a wrapper that calls
//! `zerodds_wire_codec::canonical::canonicalise` reads to
//! `dds_bounded_recursion` as a call to `canonicalise`, because the
//! lint takes a path call by its last segment. Depth 1 each.

use serde_json::Value;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_wire_codec::encode::ProbeError;

pub use zerodds_wire_codec::canonical::Difference;

/// Put one value into canonical form against its type. See
/// [`zerodds_wire_codec::canonical::canonicalise`].
///
/// # Errors
/// [`ProbeError`], naming the path in the value.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::canonical::canonicalise`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn canonicalise(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    path: &str,
) -> Result<Value, ProbeError> {
    zerodds_wire_codec::canonical::canonicalise(ast, type_, value, path)
}

/// The canonical text of a canonical value. See
/// [`zerodds_wire_codec::canonical::text`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::canonical::text`, and the lint reads a
/// path call by its last segment. See the module note.
#[must_use]
#[inline]
pub fn text(value: &Value) -> String {
    zerodds_wire_codec::canonical::text(value)
}

/// Where two canonical values first differ. See
/// [`zerodds_wire_codec::canonical::difference`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::canonical::difference`, and the lint
/// reads a path call by its last segment. See the module note.
#[must_use]
#[inline]
pub fn difference(expected: &Value, found: &Value, path: &str) -> Option<Difference> {
    zerodds_wire_codec::canonical::difference(expected, found, path)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::encode::model_from_idl;
    use serde_json::json;

    fn model(idl: &str) -> Ast {
        model_from_idl(idl, "canonical.idl").expect("the fixture parses")
    }

    fn type_of(ast: &Ast, name: &str) -> Id<TypeNode> {
        ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            name.to_string(),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the type is in the model")
    }

    #[test]
    fn the_text_does_not_depend_on_the_order_the_keys_went_in() {
        let one: Value = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
        let other: Value = serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap();
        assert_eq!(text(&one), text(&other));
        assert_eq!(text(&one), r#"{"a":1,"b":2}"#);
    }

    #[test]
    fn a_sixty_four_bit_integer_stays_a_string_and_a_long_stays_a_number() {
        let ast = model("@final struct S { long small; long long big; };");
        let type_ = type_of(&ast, "S");
        let out = canonicalise(
            &ast,
            type_,
            &json!({ "small": -7, "big": "9007199254740993" }),
            "S",
        )
        .expect("the sample is well formed");
        assert_eq!(text(&out), r#"{"big":"9007199254740993","small":-7}"#);
    }

    #[test]
    fn a_float_member_is_rounded_to_the_width_it_is_written_at() {
        let ast = model("@final struct S { float f; double d; };");
        let type_ = type_of(&ast, "S");
        let out = canonicalise(&ast, type_, &json!({ "f": 0.1, "d": 0.1 }), "S")
            .expect("the sample is well formed");
        // The `float` member cannot hold 0.1 and the `double` one can;
        // this is the difference a type-blind normaliser would report as
        // a decode defect.
        assert_eq!(text(&out), r#"{"d":0.1,"f":0.10000000149011612}"#);
    }

    #[test]
    fn an_absent_optional_is_a_missing_key_and_an_explicit_null_is_an_error() {
        let ast = model("@mutable struct S { long a; @optional long b; };");
        let type_ = type_of(&ast, "S");
        let out = canonicalise(&ast, type_, &json!({ "a": 1 }), "S").expect("b may be absent");
        assert_eq!(text(&out), r#"{"a":1}"#);
        let refused = canonicalise(&ast, type_, &json!({ "a": 1, "b": null }), "S");
        assert!(refused.is_err(), "an explicit null is not an absent member");
    }

    #[test]
    fn a_bitmask_is_ordered_by_declaration_and_deduplicated() {
        let ast = model("@bit_bound(8) bitmask M { A, B, C };\n@final struct S { M flags; };");
        let type_ = type_of(&ast, "S");
        let out = canonicalise(&ast, type_, &json!({ "flags": ["C", "A", "A"] }), "S")
            .expect("the names are declared");
        assert_eq!(text(&out), r#"{"flags":["A","C"]}"#);
    }

    #[test]
    fn a_fixed_is_written_with_the_digits_the_type_has() {
        let ast = model("@final struct S { fixed<7, 3> amount; };");
        let type_ = type_of(&ast, "S");
        for (written, canonical) in [
            ("1.5", "1.500"),
            ("0001.500", "1.500"),
            ("-0.000", "0.000"),
            ("-12.25", "-12.250"),
        ] {
            let out = canonicalise(&ast, type_, &json!({ "amount": written }), "S")
                .expect("the decimal fits");
            assert_eq!(text(&out), format!(r#"{{"amount":"{canonical}"}}"#));
        }
    }

    #[test]
    fn the_difference_names_the_first_place_two_samples_part() {
        let left = json!({ "a": 1, "b": [1, 2, 3] });
        let right = json!({ "a": 1, "b": [1, 9, 3] });
        let found = difference(&left, &right, "S").expect("they differ");
        assert_eq!(found.path, "S.b[1]");
        assert_eq!(found.expected.as_deref(), Some("2"));
        assert_eq!(found.found.as_deref(), Some("9"));
        assert_eq!(difference(&left, &left, "S"), None);
    }

    #[test]
    fn a_key_that_names_no_member_is_refused_rather_than_dropped() {
        let ast = model("@final struct S { long a; };");
        let type_ = type_of(&ast, "S");
        let refused = canonicalise(&ast, type_, &json!({ "a": 1, "typo": 2 }), "S");
        assert!(matches!(refused, Err(ProbeError::Unknown { .. })));
    }
}
