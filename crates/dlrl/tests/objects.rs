// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.3.1 DLRL objects: shared and local attributes, mono-
//! and multi-valued attributes, the type enforcement of values.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;

use common::{double, example_model, example_tags, long, oid, string};
use zerodds_dlrl::mapping::Mapping;
use zerodds_dlrl::model::{
    AttributeValue, BasicType, ClassModel, CollectionBasis, EnumerationType, Model, ModelError,
    OBJECT_ROOT, ObjectRef, Relation, SimpleStructType, SimpleType, TypeError, Value,
};
use zerodds_dlrl::rows::{ObjectData, RowError};
use zerodds_dlrl::types::DlrlOid;

fn mono(v: Value) -> AttributeValue {
    AttributeValue::Mono(v)
}

fn color() -> EnumerationType {
    EnumerationType::new("Color", &["RED", "GREEN"])
}

fn point() -> SimpleStructType {
    SimpleStructType::new("Point", &[("x", double()), ("y", double())])
}

fn typed_model() -> Model {
    Model::new(vec![
        ClassModel::new("Shape")
            .base(OBJECT_ROOT)
            .mono("count", long())
            .mono("small", SimpleType::Basic(BasicType::Short))
            .mono("color", SimpleType::Enumeration(color()))
            .mono("at", SimpleType::Struct(point()))
            .multi("tags", CollectionBasis::Set, string())
            .multi("labels", CollectionBasis::StrMap, string()),
    ])
    .unwrap()
}

#[test]
fn dlrl_8_1_3_1_simple_types_need_the_exact_type() {
    let m = typed_model();
    // Equal types pass.
    m.check_value("Shape", "count", &mono(Value::Long(3)))
        .unwrap();
    m.check_value("Shape", "small", &mono(Value::Short(3)))
        .unwrap();
    // long and short are different basic types.
    assert!(matches!(
        m.check_value("Shape", "count", &mono(Value::Short(3))),
        Err(TypeError::NotEqual { .. })
    ));
    assert!(matches!(
        m.check_value("Shape", "small", &mono(Value::Long(3))),
        Err(TypeError::NotEqual { .. })
    ));
    // An enumerator of the enumeration passes; another enumeration with
    // the same enumerator does not, nor an enumerator it does not have.
    let red = |t: &str| Value::Enum {
        type_name: t.into(),
        enumerator: "RED".into(),
    };
    m.check_value("Shape", "color", &mono(red("Color")))
        .unwrap();
    assert!(matches!(
        m.check_value("Shape", "color", &mono(red("Light"))),
        Err(TypeError::NotEqual { .. })
    ));
    assert!(matches!(
        m.check_value(
            "Shape",
            "color",
            &mono(Value::Enum {
                type_name: "Color".into(),
                enumerator: "BLUE".into()
            })
        ),
        Err(TypeError::UnknownEnumerator { .. })
    ));
    // A structure of the same name and members passes; another name, or
    // a member of another type, does not.
    let at = |name: &str, y: Value| Value::Struct {
        type_name: name.into(),
        fields: vec![("x".into(), Value::Double(1.0)), ("y".into(), y)],
    };
    m.check_value("Shape", "at", &mono(at("Point", Value::Double(2.0))))
        .unwrap();
    assert!(
        m.check_value("Shape", "at", &mono(at("Vector", Value::Double(2.0))))
            .is_err()
    );
    assert!(
        m.check_value("Shape", "at", &mono(at("Point", Value::Real(2.0))))
            .is_err()
    );
    // The double attribute x of the example takes no float.
    let ex = example_model();
    assert!(matches!(
        ex.check_value("Track", "x", &mono(Value::Real(1.0))),
        Err(TypeError::NotEqual { .. })
    ));
    ex.check_value("Track", "x", &mono(Value::Double(1.0)))
        .unwrap();
}

#[test]
fn dlrl_8_1_3_1_collections_are_homogeneous_and_of_their_basis() {
    let m = typed_model();
    let set = |v: Vec<Value>| AttributeValue::Set(v);
    m.check_value(
        "Shape",
        "tags",
        &set(vec![Value::String("a".into()), Value::String("b".into())]),
    )
    .unwrap();
    // Each item has the item type.
    assert!(matches!(
        m.check_value(
            "Shape",
            "tags",
            &set(vec![Value::String("a".into()), Value::Long(1)])
        ),
        Err(TypeError::NotEqual { .. })
    ));
    // A set holds an item once.
    assert!(matches!(
        m.check_value(
            "Shape",
            "tags",
            &set(vec![Value::String("a".into()), Value::String("a".into())])
        ),
        Err(TypeError::DuplicateSetItem { .. })
    ));
    // The basis of the value is the basis of the attribute.
    assert!(matches!(
        m.check_value("Shape", "labels", &AttributeValue::List(vec![])),
        Err(TypeError::WrongShape { .. })
    ));
    assert!(matches!(
        m.check_value("Shape", "labels", &mono(Value::String("a".into()))),
        Err(TypeError::WrongShape { .. })
    ));
    assert!(matches!(
        m.check_value("Shape", "count", &AttributeValue::Set(vec![])),
        Err(TypeError::WrongShape { .. })
    ));
    let mut labels = BTreeMap::new();
    labels.insert("k".to_string(), Value::String("v".into()));
    m.check_value("Shape", "labels", &AttributeValue::StrMap(labels))
        .unwrap();
}

#[test]
fn dlrl_8_1_3_1_a_derived_object_fits_a_reference_and_a_collection_of_its_base() {
    let m = Model::new(
        common::example_classes()
            .into_iter()
            .chain([ClassModel::new("Sensor")
                .base(OBJECT_ROOT)
                .relation(Relation::to_one("best", "Track3D"))])
            .collect(),
    )
    .unwrap();
    let track = |class: &str, n| Value::Ref(Some(ObjectRef::new(class, oid(n))));
    // A Track3D in a reference to Track, and in a list of Track.
    m.check_value(
        "Radar",
        "tracks",
        &AttributeValue::List(vec![track("Track", 1), track("Track3D", 2)]),
    )
    .unwrap();
    // A Track is no Track3D, and a Radar is no Track.
    assert!(matches!(
        m.check_value("Sensor", "best", &mono(track("Track", 1))),
        Err(TypeError::NotIncluded { .. })
    ));
    m.check_value("Sensor", "best", &mono(track("Track3D", 1)))
        .unwrap();
    assert!(matches!(
        m.check_value(
            "Radar",
            "tracks",
            &AttributeValue::List(vec![track("Radar", 3)])
        ),
        Err(TypeError::NotIncluded { .. })
    ));
    // A to-1 relation may designate no object; a collection item may not.
    m.check_value("Track", "a_radar", &mono(Value::Ref(None)))
        .unwrap();
    assert!(matches!(
        m.check_value(
            "Radar",
            "tracks",
            &AttributeValue::List(vec![Value::Ref(None)])
        ),
        Err(TypeError::NilItem { .. })
    ));
    // A plain value is no reference.
    assert!(matches!(
        m.check_value("Track", "a_radar", &mono(Value::Long(1))),
        Err(TypeError::NotEqual { .. })
    ));
}

#[test]
fn dlrl_8_1_3_1_a_class_without_shared_attribute_is_refused() {
    let only_local = ClassModel::new("Note").base(OBJECT_ROOT).local("text");
    assert_eq!(
        Model::new(vec![only_local]),
        Err(ModelError::NoSharedAttribute("Note".into()))
    );
    // A relation is a shared attribute.
    Model::new(vec![
        ClassModel::new("A")
            .base(OBJECT_ROOT)
            .relation(Relation::to_one("b", "B")),
        ClassModel::new("B").base(OBJECT_ROOT).mono("n", long()),
    ])
    .unwrap();
    // A derived class without own attributes has the inherited ones.
    Model::new(vec![
        ClassModel::new("Base").base(OBJECT_ROOT).mono("n", long()),
        ClassModel::new("Derived").base("Base"),
    ])
    .unwrap();
    // A member name is used once in a class and its ancestors.
    assert!(matches!(
        Model::new(vec![
            ClassModel::new("Base").base(OBJECT_ROOT).mono("n", long()),
            ClassModel::new("Derived").base("Base").local("n"),
        ]),
        Err(ModelError::DuplicateMember { .. })
    ));
}

#[test]
fn dlrl_8_1_3_1_a_local_attribute_is_not_published() {
    let mapping = Mapping::new(example_model(), &example_tags()).unwrap();
    // w is local: no value check, no field in any topic, no value in a row.
    assert!(matches!(
        mapping
            .model()
            .check_value("Track", "w", &mono(Value::Long(1))),
        Err(TypeError::LocalMember { .. })
    ));
    for t in mapping.topics() {
        assert!(t.fields.iter().all(|f| f.name != "w" && f.name != "W"));
    }
    let track = ObjectData::new("Track", oid(1))
        .with("x", mono(Value::Double(1.0)))
        .with("y", mono(Value::Double(2.0)))
        .with("a_radar", mono(Value::Ref(None)));
    let rows = mapping.rows_of(&track).unwrap();
    assert!(
        rows.iter()
            .all(|r| r.cells.iter().all(|(n, _)| n != "w" && n != "W"))
    );
    // An object that carries a value for w is refused, not trimmed.
    let with_w = track.with("w", mono(Value::Long(7)));
    assert!(matches!(
        mapping.rows_of(&with_w),
        Err(RowError::UnknownMember { .. })
    ));
}

#[test]
fn dlrl_8_1_3_1_the_oid_is_part_of_every_object() {
    let mapping = Mapping::new(example_model(), &example_tags()).unwrap();
    let radar = ObjectData::new("Radar", oid(9)).with("tracks", AttributeValue::List(vec![]));
    let rows = mapping.rows_of(&radar).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].topic, "RADAR-TOPIC");
    assert_eq!(
        rows[0].get("OID"),
        Some(&zerodds_dlrl::rows::Cell::Oid(oid(9)))
    );
    let back = mapping.reconstruct(&rows).unwrap();
    assert_eq!(back.objects, vec![radar]);
}

#[test]
fn dlrl_8_1_3_1_a_reference_to_the_nil_oid_is_refused() {
    let m = example_model();
    // The nil oid designates no object: a reference cannot name one by it.
    assert_eq!(
        m.check_value(
            "Track",
            "a_radar",
            &mono(Value::Ref(Some(ObjectRef::new("Radar", DlrlOid::NIL))))
        ),
        Err(TypeError::NilOid {
            member: "a_radar".into()
        })
    );
    assert_eq!(
        m.check_value(
            "Radar",
            "tracks",
            &AttributeValue::List(vec![
                Value::Ref(Some(ObjectRef::new("Track", oid(1)))),
                Value::Ref(Some(ObjectRef::new("Track3D", DlrlOid::NIL))),
            ])
        ),
        Err(TypeError::NilOid {
            member: "tracks".into()
        })
    );
    // An unset to-1 relation and a reference by a real oid pass.
    m.check_value("Track", "a_radar", &mono(Value::Ref(None)))
        .unwrap();
    m.check_value(
        "Track",
        "a_radar",
        &mono(Value::Ref(Some(ObjectRef::new(
            "Radar",
            DlrlOid::new(0, 0, 1),
        )))),
    )
    .unwrap();
}
