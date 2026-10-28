// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust expression the HDDS dump writes, and what it refuses.
//!
//! The read half of the HDDS column. The text is asserted here because
//! `hddsgen` cannot run in this checkout: what is checked is that the
//! emitter names the generated field, the generated enum variant and
//! the canonical value form the shape calls for, not that the bytes
//! came back. Building the generated type against `hdds-shim` and
//! reading a payload back is what the Linux host measures.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast_core::ScopedName;
use zerodds_wire_probe::dump_rust::dump_hdds;
use zerodds_wire_probe::encode::{ProbeError, model_from_idl};

fn emit(idl: &str, type_name: &str) -> Result<String, ProbeError> {
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
    dump_hdds(&ast, type_)
}

/// A structure is one JSON object, its members read from the generated
/// fields in declaration order.
#[test]
fn a_structure_reads_each_field_into_its_key() {
    let out = emit("@final struct S { long a; double b; };", "S").expect("the emitter");
    assert!(
        out.contains(r#"zd_object.insert("a".to_string(), serde_json::Value::from(sample.a))"#),
        "{out}"
    );
    assert!(
        out.contains(r#"zd_object.insert("b".to_string(), zd_json_f64(sample.b))"#),
        "{out}"
    );
}

/// A base's members are flattened under their own names, read through
/// the generated `base` field — XTypes 1.3 §7.4.1.1.5.1.
#[test]
fn a_base_member_is_flattened_and_read_through_the_base_field() {
    let out = emit(
        "@final struct Base { long base_a; };
         @final struct S : Base { long derived_a; };",
        "S",
    )
    .expect("the emitter");
    assert!(
        out.contains(
            r#"zd_object.insert("base_a".to_string(), serde_json::Value::from(sample.base.base_a))"#
        ),
        "{out}"
    );
    assert!(out.contains(r#"zd_object.insert("derived_a".to_string(), serde_json::Value::from(sample.derived_a))"#), "{out}");
}

/// A sequence's length is the bytes'; it becomes a JSON array built from
/// the generated `Vec`'s iterator.
#[test]
fn a_sequence_becomes_an_array_over_the_vec() {
    let out = emit("@final struct S { sequence<long> ids; };", "S").expect("the emitter");
    assert!(out.contains("(sample.ids).iter()"), "{out}");
    assert!(out.contains("serde_json::Value::Array("), "{out}");
}

/// An absent `@optional` member is a key that is left out, never a
/// `null` — XTypes 1.3 §7.4.1.2.3 and the canonical form's rule.
#[test]
fn an_optional_member_is_a_key_only_when_it_is_present() {
    let out = emit("@final struct S { long a; @optional long b; };", "S").expect("the emitter");
    assert!(out.contains("if let Some("), "{out}");
    assert!(
        out.contains(r#"zd_object.insert("b".to_string(),"#),
        "{out}"
    );
    assert!(
        !out.contains(r#"zd_object.insert("b".to_string(), serde_json::Value::Null)"#),
        "{out}"
    );
}

/// An enumeration is matched to its generated unit variants and printed
/// as the enumerator's name.
#[test]
fn an_enumeration_is_printed_as_its_variant_name() {
    let out = emit("enum Color { RED, GREEN };", "Color").expect("the emitter");
    assert!(out.contains("match &sample"), "{out}");
    assert!(out.contains(r#"Color::RED => "RED""#), "{out}");
    assert!(out.contains(r#"Color::GREEN => "GREEN""#), "{out}");
}

/// A union is `{"_d": …}` plus the variant's payload, and the
/// discriminator is the first label of the selected case.
#[test]
fn a_labelled_union_prints_the_discriminator_and_the_member() {
    let out = emit(
        "union Choice switch (long) {
             case 1: long whole;
             case 2: short part;
         };",
        "Choice",
    )
    .expect("the emitter");
    assert!(out.contains("Choice::Whole("), "{out}");
    assert!(out.contains("Choice::Part("), "{out}");
    assert!(
        out.contains(r#"zd_object.insert("_d".to_string(), serde_json::Value::from(1i64))"#),
        "{out}"
    );
    assert!(
        out.contains(r#"zd_object.insert("whole".to_string(),"#),
        "{out}"
    );
}

/// A union whose case is reached by `default:` has no stored
/// discriminator in the generated enum, so `_d` cannot be recovered and
/// the emitter says so rather than inventing one.
#[test]
fn a_defaulted_union_is_refused_because_its_discriminator_is_not_stored() {
    let error = emit(
        "union Choice switch (long) { case 1: long whole; default: short part; };",
        "Choice",
    )
    .expect_err("a default case has no recoverable discriminator");
    assert!(matches!(error, ProbeError::Unfilled { .. }), "{error:?}");
}

/// A map is the value file's array of two-element arrays, because a JSON
/// object keys on a string and a map key need not be one.
#[test]
fn a_map_becomes_an_array_of_key_value_pairs() {
    let out = emit("@final struct S { map<long, long> table; };", "S").expect("the emitter");
    assert!(out.contains("(sample.table).iter()"), "{out}");
    assert!(out.contains("serde_json::Value::Array(vec!["), "{out}");
}

/// A bitset's generated holder has no measured getter, so the read shape
/// is refused by name — a gap of this probe, not of HDDS.
#[test]
fn a_bitset_is_refused_because_its_read_shape_is_unmeasured() {
    let error = emit(
        "bitset Flags { bitfield<3> low; bitfield<5> high; };
         @final struct S { Flags bits; };",
        "S",
    )
    .expect_err("no measured bitfield getter");
    assert!(matches!(error, ProbeError::Unfilled { .. }), "{error:?}");
}

/// A recursive type is what `hddsgen` emits with infinite size and no
/// decoder, so the read direction refuses it rather than walking a cycle
/// with no end.
#[test]
fn a_recursive_type_is_refused() {
    let error = emit(
        "module m { struct Node;
             @mutable struct Node { long value; @optional Node next; }; };",
        "m::Node",
    )
    .expect_err("hddsgen generates no usable type for a cycle");
    assert!(matches!(error, ProbeError::Unsupported { .. }), "{error:?}");
}

/// A `long double` has no Rust primitive in `hddsgen`'s output, so it is
/// refused the same way the fill side refuses it.
#[test]
fn a_long_double_is_refused() {
    let error = emit("@final struct S { long double x; };", "S")
        .expect_err("no Rust primitive for a long double");
    assert!(matches!(error, ProbeError::Unsupported { .. }), "{error:?}");
}
