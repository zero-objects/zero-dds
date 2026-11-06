// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-types` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-codec`, only with its `dynamic`
//! feature (`src/dynamic/data.rs`, `type_.rs`, `sample.rs`): `DynamicData`
//! implements the value traits `Source` and `Sink` (and builds through
//! `Aggregate`), and a fresh value of a type is
//! `default::of_type::<DynamicData>` (`DynamicSample::new`, the `Clone` of a
//! sample) or, for one member, `default::member`
//! (`DynamicTypeMember::default_value`); a `ProbeError` is turned into
//! `DynamicError::Inconsistent` through its `Display`.
//!
//! Oracle: DDS-XTypes 1.3 §7.2.2.4.4.4.9 (a member's explicit default
//! first, otherwise the default of its type), §7.2.2.4.4.4.7 Table 9
//! (`BOOLEAN` False, integers and floats 0, `STRING` "", `SEQUENCE` empty,
//! `ENUM` the first literal, `STRUCTURE` its non-optional members at their
//! defaults) and §7.3.1.2.1.10 (`@default_literal` names the enumeration's
//! default).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_ast::PrimitiveKind;
use zerodds_types::dynamic::{
    DynamicData, DynamicTypeBuilder, ExtensibilityKind, NewMember, TypeKind,
};
use zerodds_wire_codec::{ProbeError, Sink, Source};

fn member<'a>(members: &'a [(String, DynamicData)], name: &str) -> &'a DynamicData {
    members
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("`{name}` is a member"))
}

#[test]
fn dynamic_data_is_a_value_of_both_walks() {
    fn value<V: Source + Sink>() {}
    value::<DynamicData>();
    fn shown<E: core::fmt::Display>() {}
    shown::<ProbeError>();
}

#[test]
fn xtypes_7_2_2_4_4_4_9_an_explicit_default_wins_over_table_9() {
    let mut builder = DynamicTypeBuilder::new();
    let int32 = builder.primitive(TypeKind::Int32).unwrap();
    let sample = builder
        .structure("Sample", ExtensibilityKind::Final)
        .unwrap();
    builder
        .add_member(sample, &NewMember::new("count", int32).with_default("42"))
        .unwrap();
    builder
        .add_member(sample, &NewMember::new("plain", int32))
        .unwrap();
    let value: DynamicData =
        zerodds_wire_codec::default::of_type(builder.model(), sample, "").unwrap();
    let DynamicData::Aggregate(members) = value else {
        panic!("a structure's default is an aggregate");
    };
    assert_eq!(members.len(), 2);
    assert_eq!(
        member(&members, "count"),
        &DynamicData::integer(PrimitiveKind::Int32, 42)
    );
    assert_eq!(
        member(&members, "plain"),
        &DynamicData::integer(PrimitiveKind::Int32, 0)
    );
}

#[test]
fn xtypes_7_2_2_4_4_4_7_table_9_defaults_of_each_kind() {
    let mut builder = DynamicTypeBuilder::new();
    let boolean = builder.primitive(TypeKind::Boolean).unwrap();
    let float64 = builder.primitive(TypeKind::Float64).unwrap();
    let int32 = builder.primitive(TypeKind::Int32).unwrap();
    let string = builder.string(false, None).unwrap();
    let sequence = builder.sequence(int32, None).unwrap();
    let colour = builder.enumeration("Colour", None).unwrap();
    builder.add_enumerator(colour, "RED", None, false).unwrap();
    builder
        .add_enumerator(colour, "GREEN", None, false)
        .unwrap();
    let all = builder.structure("All", ExtensibilityKind::Final).unwrap();
    for (name, type_) in [
        ("b", boolean),
        ("f", float64),
        ("s", string),
        ("q", sequence),
        ("e", colour),
    ] {
        builder
            .add_member(all, &NewMember::new(name, type_))
            .unwrap();
    }
    let value: DynamicData = zerodds_wire_codec::default_of_type(builder.model(), all, "").unwrap();
    let DynamicData::Aggregate(members) = value else {
        panic!("a structure's default is an aggregate");
    };
    assert_eq!(member(&members, "b"), &DynamicData::Boolean(false));
    assert_eq!(
        member(&members, "f"),
        &DynamicData::float(PrimitiveKind::Float64, 0.0)
    );
    assert_eq!(member(&members, "s"), &DynamicData::Text(String::new()));
    assert_eq!(member(&members, "q"), &DynamicData::Collection(Vec::new()));
    assert_eq!(
        member(&members, "e"),
        &DynamicData::Enumerator(String::from("RED")),
        "the first literal"
    );
}

#[test]
fn xtypes_7_3_1_2_1_10_default_literal_names_the_enumerations_default() {
    let mut builder = DynamicTypeBuilder::new();
    let colour = builder.enumeration("Colour", None).unwrap();
    builder.add_enumerator(colour, "RED", None, false).unwrap();
    builder.add_enumerator(colour, "GREEN", None, true).unwrap();
    let value: DynamicData =
        zerodds_wire_codec::default::of_type(builder.model(), colour, "").unwrap();
    assert_eq!(value, DynamicData::Enumerator(String::from("GREEN")));
}
