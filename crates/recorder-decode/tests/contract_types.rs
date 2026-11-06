// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-recorder-decode` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`, all from `dynamic`:
//! - `src/type_source.rs`: `DynamicType::find` on the composed model, first
//!   with the name as given, then with the one scoped name whose suffix it
//!   is.
//! - `src/wire.rs`: `DynamicType::extensibility` and
//!   `descriptor::ExtensibilityKind::{Mutable, Appendable}`, which choose the
//!   XCDR encoding of a sample.
//! - `src/sqlite_sink.rs`: the schema walk over `DynamicType::members`,
//!   `DynamicTypeMember::{name, dynamic_type}`, `DynamicType::{kind,
//!   resolve_alias, element_type, key_element_type}` and
//!   `descriptor::TypeKind` (the float, text, enumeration, structure,
//!   bitset, sequence, array, map, bitmask and union kinds).
//! - `src/json_sink.rs` and `src/sqlite_sink.rs`: `DynamicData`, matched on
//!   its twelve variants; any other variant becomes JSON `null` or an SQL
//!   `NULL`.
//!
//! The model is composed with `zerodds-idl-compose`, as `TypeBook::from_idl`
//! does. The value of the last test comes from
//! `zerodds_wire_codec::default_of_type`, which builds it through
//! `DynamicData`'s own `Sink` constructors, the ones a decoded sample is
//! built with.
//!
//! Oracle: DDS-XTypes 1.3 §7.5.2.8 Table 54 (`kind`, members, `element_type`,
//! `key_element_type`), §7.5.2.2 and Annex C (the TypeKind of each IDL
//! construct), §7.3.1.2.1.8 (extensibility of an aggregated type, none for
//! other kinds), §7.2.2.4.4.4.7 Table 9 (default values).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;
use std::sync::Arc;

use zerodds_types::dynamic::DynamicData;
use zerodds_types::dynamic::descriptor::{ExtensibilityKind, TypeKind};
use zerodds_types::dynamic::type_::DynamicType;

const IDL: &str = "\
module rec {
  enum Color { RED, GREEN };
  bitmask Flags { A, B };
  bitset Bits { bitfield<3> low; bitfield<5> high; };
  typedef long Meters;
  @final struct Inner { long v; };
  @appendable struct Grown { long v; };
  @mutable struct Moving { long v; };
  union Choice switch (long) { case 1: long i; default: string s; };
  @final struct All {
    boolean b;
    octet o;
    char c;
    wchar w;
    short s;
    unsigned long long u;
    float f;
    double d;
    long double ld;
    string str;
    wstring wstr;
    fixed<5,2> fx;
    Color tint;
    Flags options;
    Bits packed;
    Meters m;
    sequence<long> seq;
    long arr[2];
    map<string, long> mp;
    Inner nested;
    Choice pick;
  };
};";

fn model() -> Arc<zerodds_ast::Ast> {
    let composed = zerodds_idl_compose::compose_source(
        "<type-file>",
        IDL,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(
        !composed.failed(),
        "the type file composes without an error"
    );
    Arc::new(composed.ast)
}

#[test]
fn xtypes_7_5_2_8_find_takes_the_scoped_name_with_or_without_leading_colons() {
    let ast = model();
    assert_eq!(
        DynamicType::find(&ast, "rec::All")
            .unwrap()
            .qualified_name(),
        "rec::All"
    );
    assert_eq!(
        DynamicType::find(&ast, "::rec::All")
            .unwrap()
            .qualified_name(),
        "rec::All"
    );
    // A bare name is not a scoped name of this model: the crate's suffix
    // search takes over.
    assert!(DynamicType::find(&ast, "All").is_err());
    assert!(DynamicType::find(&ast, "rec::Absent").is_err());
}

#[test]
fn xtypes_7_3_1_2_1_8_the_extensibility_that_chooses_the_encoding() {
    let ast = model();
    let extensibility = |name: &str| DynamicType::find(&ast, name).unwrap().extensibility();
    assert_eq!(extensibility("rec::Inner"), Some(ExtensibilityKind::Final));
    assert_eq!(
        extensibility("rec::Grown"),
        Some(ExtensibilityKind::Appendable)
    );
    assert_eq!(
        extensibility("rec::Moving"),
        Some(ExtensibilityKind::Mutable)
    );
    assert!(
        extensibility("rec::Choice").is_some(),
        "a union has one too"
    );
    assert_eq!(extensibility("rec::Color"), None, "an enumeration has none");
}

#[test]
fn xtypes_7_5_2_8_the_kinds_the_sqlite_schema_walk_tells_apart() {
    let all = DynamicType::find(&model(), "rec::All").unwrap();
    let kinds: Vec<(String, TypeKind)> = all
        .members()
        .iter()
        .map(|m| (m.name().to_string(), m.dynamic_type().unwrap().kind()))
        .collect();
    let expected = [
        ("b", TypeKind::Boolean),
        ("o", TypeKind::Byte),
        ("c", TypeKind::Char8),
        ("w", TypeKind::Char16),
        ("s", TypeKind::Int16),
        ("u", TypeKind::UInt64),
        ("f", TypeKind::Float32),
        ("d", TypeKind::Float64),
        ("ld", TypeKind::Float128),
        ("str", TypeKind::String8),
        ("wstr", TypeKind::String16),
        ("tint", TypeKind::Enumeration),
        ("options", TypeKind::Bitmask),
        ("packed", TypeKind::Bitset),
        ("m", TypeKind::Alias),
        ("seq", TypeKind::Sequence),
        ("arr", TypeKind::Array),
        ("mp", TypeKind::Map),
        ("nested", TypeKind::Structure),
        ("pick", TypeKind::Union),
    ];
    for (name, kind) in expected {
        assert!(
            kinds.iter().any(|(n, k)| n == name && *k == kind),
            "`{name}` is not {kind:?}: {kinds:?}"
        );
    }
    let member = |name: &str| all.member_by_name(name).unwrap().dynamic_type().unwrap();
    // An alias is stored as what it names.
    assert_eq!(member("m").resolve_alias().unwrap().kind(), TypeKind::Int32);
    assert_eq!(
        member("seq").element_type().unwrap().kind(),
        TypeKind::Int32
    );
    assert_eq!(
        member("arr").element_type().unwrap().kind(),
        TypeKind::Int32
    );
    let map = member("mp");
    assert_eq!(map.key_element_type().unwrap().kind(), TypeKind::String8);
    assert_eq!(map.element_type().unwrap().kind(), TypeKind::Int32);
}

/// The variants `data_to_json` and `scalar_param` convert; the walk refuses
/// any other, which those two would turn into `null`.
fn walk(data: &DynamicData, seen: &mut BTreeSet<&'static str>) {
    let name = match data {
        DynamicData::Boolean(_) => "Boolean",
        DynamicData::Integer { .. } => "Integer",
        DynamicData::Float { .. } => "Float",
        DynamicData::LongDouble(_) => "LongDouble",
        DynamicData::Character { .. } => "Character",
        DynamicData::Text(_) => "Text",
        DynamicData::Fixed(_) => "Fixed",
        DynamicData::Enumerator(_) => "Enumerator",
        DynamicData::Bits(items) | DynamicData::Collection(items) => {
            for item in items {
                walk(item, seen);
            }
            if matches!(data, DynamicData::Bits(_)) {
                "Bits"
            } else {
                "Collection"
            }
        }
        DynamicData::Bitfields(fields) | DynamicData::Aggregate(fields) => {
            for (_, value) in fields {
                walk(value, seen);
            }
            if matches!(data, DynamicData::Bitfields(_)) {
                "Bitfields"
            } else {
                "Aggregate"
            }
        }
        other => panic!("a variant the JSON and SQLite sinks do not convert: {other:?}"),
    };
    seen.insert(name);
}

#[test]
fn xtypes_7_2_2_4_4_4_7_every_value_of_every_kind_is_a_variant_the_sinks_convert() {
    let ast = model();
    let all = DynamicType::find(&ast, "rec::All").unwrap();
    let value: DynamicData =
        zerodds_wire_codec::default_of_type(&ast, all.id(), "rec::All").unwrap();
    let mut seen = BTreeSet::new();
    walk(&value, &mut seen);
    assert_eq!(
        seen,
        BTreeSet::from([
            "Aggregate",
            "Bitfields",
            "Bits",
            "Boolean",
            "Character",
            "Collection",
            "Enumerator",
            "Fixed",
            "Float",
            "Integer",
            "LongDouble",
            "Text",
        ])
    );
}
