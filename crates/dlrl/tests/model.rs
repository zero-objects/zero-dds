// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.3.3 Metamodel: every construct of Figure 8.1 can be
//! described, and ill-formed types are refused.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{double, long, string};
use zerodds_dlrl::mapping::{CellType, Mapping};
use zerodds_dlrl::model::{
    AttributeType, BasicItem, BasicType, ClassModel, CollectionBasis, EnumerationType,
    MAX_STRUCT_NESTING, Member, Model, ModelError, Multiplicity, OBJECT_ROOT, Relation,
    SimpleStructType, SimpleType,
};

const BASES: [CollectionBasis; 4] = [
    CollectionBasis::List,
    CollectionBasis::StrMap,
    CollectionBasis::IntMap,
    CollectionBasis::Set,
];

fn basic_types() -> Vec<(&'static str, BasicType, CellType)> {
    let items = [
        BasicItem::Long,
        BasicItem::Short,
        BasicItem::Char,
        BasicItem::Octet,
        BasicItem::Real,
        BasicItem::Double,
        BasicItem::String,
    ];
    let mut v = vec![
        ("b_long", BasicType::Long, CellType::Long),
        ("b_short", BasicType::Short, CellType::Short),
        ("b_char", BasicType::Char, CellType::Char),
        ("b_octet", BasicType::Octet, CellType::Octet),
        ("b_real", BasicType::Real, CellType::Real),
        ("b_double", BasicType::Double, CellType::Double),
        ("b_string", BasicType::String, CellType::String),
    ];
    let seq_names = [
        "s_long", "s_short", "s_char", "s_octet", "s_real", "s_double", "s_string",
    ];
    for (i, n) in items.iter().zip(seq_names) {
        v.push((n, BasicType::Sequence(*i), CellType::Sequence(*i)));
    }
    v
}

fn every_construct() -> Vec<ClassModel> {
    let day = EnumerationType::new("Day", &["MON", "TUE"]);
    let inner = SimpleStructType::new("Inner", &[("a", long()), ("b", string())]);
    let outer = SimpleStructType::new(
        "Outer",
        &[("inner", SimpleType::Struct(inner)), ("d", double())],
    );
    let mut all = ClassModel::new("AllTypes")
        .base(OBJECT_ROOT)
        .final_class()
        .mono("e", SimpleType::Enumeration(day.clone()))
        .mono("st", SimpleType::Struct(outer));
    for (name, b, _) in basic_types() {
        all = all.mono(name, SimpleType::Basic(b));
    }
    for basis in BASES {
        all = all
            .multi(
                &format!("m_{}", basis.pattern()),
                basis,
                SimpleType::Enumeration(day.clone()),
            )
            .relation(Relation::to_many(
                &format!("r_{}", basis.pattern()),
                "Track",
                basis,
            ));
    }
    all = all
        .relation(Relation::to_one("one", "Track"))
        .relation(Relation::to_one("owned", "Radar").composition());
    let mut classes = common::example_classes();
    classes.push(all);
    classes
}

#[test]
fn dlrl_8_1_3_3_every_construct_of_figure_8_1_is_describable() {
    let m = Model::new(every_construct()).unwrap();
    let all = m.class("AllTypes").unwrap();
    // Class: final, parent, children.
    assert!(all.is_final);
    assert!(m.parent("AllTypes").is_none());
    assert_eq!(m.children("Track").len(), 1);
    // MonoAttribute of each BasicType, an EnumerationType and a
    // SimpleStructType.
    for (name, b, _) in basic_types() {
        match m.member("AllTypes", name) {
            Some((_, Member::Attribute(a))) => {
                assert_eq!(a.ty, AttributeType::Mono(SimpleType::Basic(b)));
            }
            other => panic!("{name}: {other:?}"),
        }
    }
    assert!(matches!(
        m.member("AllTypes", "e"),
        Some((_, Member::Attribute(a))) if matches!(a.ty, AttributeType::Mono(SimpleType::Enumeration(_)))
    ));
    assert!(matches!(
        m.member("AllTypes", "st"),
        Some((_, Member::Attribute(a))) if matches!(a.ty, AttributeType::Mono(SimpleType::Struct(_)))
    ));
    // MultiAttribute (MultiSimpleType) and MultiRelation (MultiRefType)
    // on each collection basis; MonoRelation; Relation::is_composition;
    // Relation::inverse (in the example classes).
    for basis in BASES {
        assert!(matches!(
            m.member("AllTypes", &format!("m_{}", basis.pattern())),
            Some((_, Member::Attribute(a))) if matches!(a.ty, AttributeType::Multi { basis: b, .. } if b == basis)
        ));
        assert!(matches!(
            m.member("AllTypes", &format!("r_{}", basis.pattern())),
            Some((_, Member::Relation(r))) if r.multiplicity == Multiplicity::ToMany(basis)
        ));
    }
    assert!(matches!(
        m.member("AllTypes", "owned"),
        Some((_, Member::Relation(r))) if r.is_composition && r.multiplicity == Multiplicity::ToOne
    ));
    assert!(matches!(
        m.member("Track", "a_radar"),
        Some((_, Member::Relation(r))) if r.inverse.as_deref() == Some("tracks")
    ));
    // Each construct reaches a DCPS description under the default
    // mapping, with the cell type of its type.
    let mapping = Mapping::default_for(m).unwrap();
    let main = mapping.topic("AllTypes").unwrap();
    for (name, _, cell) in basic_types() {
        assert_eq!(main.field(name).map(|f| f.cell), Some(cell), "{name}");
    }
    assert_eq!(main.field("e").map(|f| f.cell), Some(CellType::Long));
    for f in ["st.inner.a", "st.inner.b", "st.d"] {
        assert!(main.field(f).is_some(), "{f}");
    }
    for basis in BASES {
        let t = mapping
            .topic(&format!("AllTypes.m_{}", basis.pattern()))
            .unwrap();
        assert_eq!(t.field("index").is_some(), basis != CollectionBasis::Set);
        let r = mapping
            .topic(&format!("AllTypes.r_{}", basis.pattern()))
            .unwrap();
        // Track has a derived class: each item reference is a full oid.
        let rel = format!("r_{}", basis.pattern());
        assert_eq!(
            r.field(&format!("{rel}.class")).map(|f| f.cell),
            Some(CellType::String)
        );
        assert_eq!(
            r.field(&format!("{rel}.oid")).map(|f| f.cell),
            Some(CellType::Oid)
        );
    }
}

#[test]
fn dlrl_8_1_3_3_ill_formed_types_are_refused() {
    let with =
        |t: SimpleType| Model::new(vec![ClassModel::new("C").base(OBJECT_ROOT).mono("v", t)]);
    assert!(matches!(
        with(SimpleType::Enumeration(EnumerationType::new("E", &[]))),
        Err(ModelError::BadEnumeration { .. })
    ));
    assert!(matches!(
        with(SimpleType::Enumeration(EnumerationType::new(
            "E",
            &["A", "A"]
        ))),
        Err(ModelError::BadEnumeration { .. })
    ));
    assert!(matches!(
        with(SimpleType::Enumeration(
            EnumerationType::new("E", &["A", "B"]).coded_as(&["a"])
        )),
        Err(ModelError::BadEnumeration { .. })
    ));
    assert!(matches!(
        with(SimpleType::Struct(SimpleStructType::new("S", &[]))),
        Err(ModelError::BadStruct { .. })
    ));
    assert!(matches!(
        with(SimpleType::Struct(SimpleStructType::new(
            "S",
            &[("a", long()), ("a", double())]
        ))),
        Err(ModelError::BadStruct { .. })
    ));
    assert!(matches!(
        Model::new(vec![
            ClassModel::new("C").base(OBJECT_ROOT).mono("n", long()),
            ClassModel::new("C").base(OBJECT_ROOT).mono("n", long()),
        ]),
        Err(ModelError::DuplicateClass(_))
    ));
}

/// A simple type holds at most `MAX_STRUCT_NESTING` structures inside one
/// another; `Model::new` refuses a deeper one, so the functions that walk
/// the types of a model go down a bounded number of calls.
#[test]
fn dlrl_8_1_3_3_structures_nested_deeper_than_max_struct_nesting_are_refused() {
    let nested = |n: usize| {
        let mut t = long();
        for i in 0..n {
            t = SimpleType::Struct(SimpleStructType::new(&format!("S{i}"), &[("m", t)]));
        }
        t
    };
    let with =
        |t: SimpleType| Model::new(vec![ClassModel::new("C").base(OBJECT_ROOT).mono("v", t)]);
    assert!(with(nested(MAX_STRUCT_NESTING)).is_ok());
    assert!(matches!(
        with(nested(MAX_STRUCT_NESTING + 1)),
        Err(ModelError::BadStruct { .. })
    ));
}
