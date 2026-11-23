// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.4 Structural mapping: the mapping rules on the rows they
//! produce (§8.1.4.2), the default mapping (§8.1.4.3) and the mapping
//! information of Figure 8.2 with its checks (§8.1.4.4).

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;

use common::{double, example_classes, example_model, example_tags, long, oid, string};
use zerodds_dlrl::mapping::{
    CellType, ClassMappingSpec, FieldDescription, KeyContent, KeyDescription, Mapping,
    MappingError, MemberMapping, MemberMappingSpec, MultiTopicSpec, TopicDescription, TopicSpec,
};
use zerodds_dlrl::model::{
    AttributeValue, BasicItem, BasicType, ClassModel, CollectionBasis, EnumerationType, Model,
    OBJECT_ROOT, ObjectRef, Relation, SequenceValue, SimpleStructType, SimpleType, TypeError,
    Value,
};
use zerodds_dlrl::rows::{Cell, ObjectData, Row, RowError, SkipReason};
use zerodds_dlrl::types::DlrlOid;

fn mono(v: Value) -> AttributeValue {
    AttributeValue::Mono(v)
}

fn field(name: &str, cell: CellType, key: bool) -> FieldDescription {
    FieldDescription {
        name: name.into(),
        cell,
        key,
    }
}

fn layout(t: &TopicDescription) -> Vec<(String, CellType, bool)> {
    t.fields
        .iter()
        .map(|f| (f.name.clone(), f.cell, f.key))
        .collect()
}

fn expect(fields: &[FieldDescription]) -> Vec<(String, CellType, bool)> {
    fields
        .iter()
        .map(|f| (f.name.clone(), f.cell, f.key))
        .collect()
}

fn example_mapping() -> Mapping {
    Mapping::new(example_model(), &example_tags()).unwrap()
}

fn track3d(n: i32) -> ObjectData {
    let mut comments = BTreeMap::new();
    comments.insert("first".to_string(), Value::String("seen".into()));
    comments.insert("second".to_string(), Value::String("lost".into()));
    ObjectData::new("Track3D", oid(n))
        .with("x", mono(Value::Double(1.5)))
        .with("y", mono(Value::Double(2.5)))
        .with("z", mono(Value::Double(3.5)))
        .with("comments", AttributeValue::StrMap(comments))
        .with(
            "a_radar",
            mono(Value::Ref(Some(ObjectRef::new("Radar", oid(100))))),
        )
}

fn radar(n: i32, tracks: Vec<ObjectRef>) -> ObjectData {
    ObjectData::new("Radar", oid(n)).with(
        "tracks",
        AttributeValue::List(tracks.into_iter().map(|t| Value::Ref(Some(t))).collect()),
    )
}

// ---------------------------------------------------------------------------
// §8.1.4.4
// ---------------------------------------------------------------------------

#[test]
fn dlrl_8_1_4_4_each_element_of_the_example_yields_its_topic_and_fields() {
    let m = example_mapping();
    let t = |n: &str| m.topic(n).unwrap_or_else(|| panic!("topic {n}"));
    let class = field("CLASS", CellType::String, true);
    let oid_f = field("OID", CellType::Oid, true);
    // The five topics of §8.2.3.4 …
    assert_eq!(
        layout(t("TRACK-TOPIC")),
        expect(&[
            class.clone(),
            oid_f.clone(),
            field("X", CellType::Double, false),
            field("RADAR_OID", CellType::Oid, false),
        ])
    );
    assert_eq!(
        layout(t("Y_TOPIC")),
        expect(&[
            class.clone(),
            oid_f.clone(),
            field("Y", CellType::Double, false)
        ])
    );
    assert_eq!(
        layout(t("COMMENTS-TOPIC")),
        expect(&[
            class.clone(),
            oid_f.clone(),
            field("INDEX", CellType::String, true),
            field("COMMENT", CellType::String, false),
        ])
    );
    assert_eq!(
        layout(t("TRACK3D-TOPIC")),
        expect(&[class, oid_f.clone(), field("Z", CellType::Double, false)])
    );
    assert_eq!(
        layout(t("RADARTRACKS-TOPIC")),
        expect(&[
            field("RADAR-OID", CellType::Oid, true),
            field("INDEX", CellType::Long, true),
            field("TRACK-CLASS", CellType::String, false),
            field("TRACK-OID", CellType::Oid, false),
        ])
    );
    // … and the main topic of Radar, which §8.2.3.4 does not list.
    assert_eq!(layout(t("RADAR-TOPIC")), expect(&[oid_f]));
    assert_eq!(m.topics().len(), 6);
    // Which classes put rows into which topic.
    assert_eq!(t("TRACK-TOPIC").classes, vec!["Track", "Track3D"]);
    assert_eq!(t("Y_TOPIC").classes, vec!["Track", "Track3D"]);
    assert_eq!(t("TRACK3D-TOPIC").classes, vec!["Track3D"]);
    assert_eq!(t("RADARTRACKS-TOPIC").classes, vec!["Radar"]);
    assert_eq!(t("TRACK-TOPIC").type_name, "TRACK-TOPIC");

    // Figure 8.2, Class.
    let track = m.class("Track").unwrap();
    assert_eq!(track.main_topic.name, "TRACK-TOPIC");
    assert!(track.full_oid_required());
    assert_eq!(track.class_field(), Some("CLASS"));
    assert_eq!(track.oid_field(), Some("OID"));
    let radar_cm = m.class("Radar").unwrap();
    assert!(!radar_cm.full_oid_required());
    assert_eq!(radar_cm.class_field(), None);
    assert_eq!(radar_cm.oid_field(), Some("OID"));
    let t3 = m.class("Track3D").unwrap();
    assert_eq!(t3.main_topic.name, "TRACK-TOPIC");
    assert_eq!(
        t3.extension_topic.as_ref().map(|e| e.name.as_str()),
        Some("TRACK3D-TOPIC")
    );
    // MonoAttribute: topic, target_field, key_fields; z defaults to the
    // extension topic, x to the main topic.
    let x = track.member("x").unwrap();
    assert_eq!(x.topic().name, "TRACK-TOPIC");
    assert_eq!(x.target_fields(), ["X"]);
    assert_eq!(x.key_fields(), ["CLASS", "OID"]);
    assert_eq!(t3.member("z").unwrap().topic().name, "TRACK3D-TOPIC");
    assert_eq!(track.member("y").unwrap().topic().name, "Y_TOPIC");
    // MultiAttribute: index_field.
    assert_eq!(
        track.member("comments").unwrap().index_field(),
        Some("INDEX")
    );
    // MonoRelation and MultiRelation: target_fields, full_oid_required,
    // is_composition.
    let a_radar = track.member("a_radar").unwrap();
    assert_eq!(a_radar.target_fields(), ["RADAR_OID"]);
    assert!(!a_radar.full_oid_required());
    let tracks = radar_cm.member("tracks").unwrap();
    assert_eq!(tracks.target_fields(), ["TRACK-CLASS", "TRACK-OID"]);
    assert!(tracks.full_oid_required());
    assert_eq!(tracks.key_fields(), ["RADAR-OID"]);
    assert_eq!(tracks.index_field(), Some("INDEX"));
    assert!(matches!(tracks, MemberMapping::MultiRelation(r) if !r.is_composition));
    // Inherited members through the derived class.
    let (owner, _) = m.member("Track3D", "y").unwrap();
    assert_eq!(owner.class, "Track");
}

fn with_track(edit: impl FnOnce(&mut ClassMappingSpec)) -> Result<Mapping, MappingError> {
    let mut tags = example_tags();
    edit(&mut tags[0]);
    Mapping::new(example_model(), &tags)
}

fn with_radar(edit: impl FnOnce(&mut ClassMappingSpec)) -> Result<Mapping, MappingError> {
    let mut tags = example_tags();
    edit(&mut tags[2]);
    Mapping::new(example_model(), &tags)
}

#[test]
fn dlrl_8_1_4_4_a_collection_topic_differs_from_the_owner_topics() {
    let r = with_track(|t| {
        t.members[2] = MemberMappingSpec::MultiAttribute {
            name: "comments".into(),
            topic: MultiTopicSpec::new(
                "TRACK-TOPIC",
                Some("INDEX"),
                KeyDescription::full_oid("CLASS", "OID"),
            ),
            value_fields: vec!["COMMENT".into()],
        };
    });
    assert!(
        matches!(r, Err(MappingError::CollectionTopic { .. })),
        "{r:?}"
    );
    // Nor the topic of another member.
    let r = with_track(|t| {
        t.members[2] = MemberMappingSpec::MultiAttribute {
            name: "comments".into(),
            topic: MultiTopicSpec::new(
                "Y_TOPIC",
                Some("INDEX"),
                KeyDescription::full_oid("CLASS", "OID"),
            ),
            value_fields: vec!["COMMENT".into()],
        };
    });
    assert!(
        matches!(r, Err(MappingError::CollectionTopic { .. })),
        "{r:?}"
    );
}

#[test]
fn dlrl_8_1_4_4_key_fields_follow_the_key_content() {
    // FullOid with one field.
    let r = with_track(|t| {
        t.main_topic = Some(TopicSpec::new(
            "TRACK-TOPIC",
            KeyDescription {
                content: KeyContent::FullOid,
                fields: vec!["OID".into()],
            },
        ));
    });
    assert!(matches!(r, Err(MappingError::KeyFields { .. })), "{r:?}");
    // SimpleOid with two fields.
    let r = with_radar(|t| {
        t.main_topic = Some(TopicSpec::new(
            "RADAR-TOPIC",
            KeyDescription {
                content: KeyContent::SimpleOid,
                fields: vec!["A".into(), "B".into()],
            },
        ));
    });
    assert!(matches!(r, Err(MappingError::KeyFields { .. })), "{r:?}");
    // A place topic designates the owner as the main topic does (1 or
    // 2 key fields "depending on the Class definition").
    let r = with_track(|t| {
        t.members[1] = MemberMappingSpec::MonoAttribute {
            name: "y".into(),
            place_topic: Some(TopicSpec::new("Y_TOPIC", KeyDescription::simple_oid("OID"))),
            value_fields: vec!["Y".into()],
        };
    });
    assert!(
        matches!(r, Err(MappingError::OwnerKeyMismatch { .. })),
        "{r:?}"
    );
    // A set has no index field.
    let mut classes = example_classes();
    classes[0] = ClassModel::new("Track")
        .base(OBJECT_ROOT)
        .mono("x", double())
        .multi("tags", CollectionBasis::Set, string());
    let model = Model::new(classes[..1].to_vec()).unwrap();
    let r = Mapping::new(
        model,
        &[ClassMappingSpec {
            class: "Track".into(),
            members: vec![MemberMappingSpec::MultiAttribute {
                name: "tags".into(),
                topic: MultiTopicSpec::new("TAGS", Some("I"), KeyDescription::simple_oid("oid")),
                value_fields: vec!["TAG".into()],
            }],
            ..ClassMappingSpec::default()
        }],
    );
    assert!(matches!(r, Err(MappingError::IndexField { .. })), "{r:?}");
    // The index completes the owner key and cannot repeat one of its
    // fields.
    let r = with_track(|t| {
        t.members[2] = MemberMappingSpec::MultiAttribute {
            name: "comments".into(),
            topic: MultiTopicSpec::new(
                "COMMENTS-TOPIC",
                Some("OID"),
                KeyDescription::full_oid("CLASS", "OID"),
            ),
            value_fields: vec!["COMMENT".into()],
        };
    });
    assert_eq!(
        r.as_ref().err(),
        Some(&MappingError::FieldConflict {
            topic: "COMMENTS-TOPIC".into(),
            field: "OID".into(),
        }),
        "{r:?}"
    );
    // Nor can the item fields of a set, which complete the key as well.
    let model = Model::new(classes[..1].to_vec()).unwrap();
    let r = Mapping::new(
        model,
        &[ClassMappingSpec {
            class: "Track".into(),
            members: vec![MemberMappingSpec::MultiAttribute {
                name: "tags".into(),
                topic: MultiTopicSpec::new("TAGS", None, KeyDescription::simple_oid("oid")),
                value_fields: vec!["oid".into()],
            }],
            ..ClassMappingSpec::default()
        }],
    );
    assert_eq!(
        r.as_ref().err(),
        Some(&MappingError::FieldConflict {
            topic: "TAGS".into(),
            field: "oid".into(),
        }),
        "{r:?}"
    );
}

#[test]
fn dlrl_8_1_4_4_the_given_elements_fit_the_members() {
    // A value field per cell.
    let r = with_track(|t| {
        t.members[0] = MemberMappingSpec::MonoAttribute {
            name: "x".into(),
            place_topic: None,
            value_fields: vec!["X1".into(), "X2".into()],
        };
    });
    assert!(
        matches!(
            r,
            Err(MappingError::ValueFields {
                expected: 1,
                given: 2,
                ..
            })
        ),
        "{r:?}"
    );
    // monoAttribute for a relation.
    let r = with_track(|t| {
        t.members[3] = MemberMappingSpec::MonoAttribute {
            name: "a_radar".into(),
            place_topic: None,
            value_fields: vec!["R".into()],
        };
    });
    assert!(matches!(r, Err(MappingError::MemberKind { .. })), "{r:?}");
    // A local member has no mapping.
    let r = with_track(|t| {
        t.members.push(MemberMappingSpec::MonoAttribute {
            name: "w".into(),
            place_topic: None,
            value_fields: vec!["W".into()],
        });
    });
    assert!(matches!(r, Err(MappingError::LocalMember { .. })), "{r:?}");
    // An inherited member is mapped by its class.
    let mut tags = example_tags();
    tags[1].members.push(MemberMappingSpec::MonoAttribute {
        name: "x".into(),
        place_topic: None,
        value_fields: vec!["X3".into()],
    });
    let r = Mapping::new(example_model(), &tags);
    assert!(
        matches!(r, Err(MappingError::InheritedMember { .. })),
        "{r:?}"
    );
    // An unknown member, an unknown class, a class mapped twice.
    let r = with_track(|t| {
        t.members.push(MemberMappingSpec::MonoAttribute {
            name: "q".into(),
            place_topic: None,
            value_fields: vec!["Q".into()],
        });
    });
    assert!(
        matches!(r, Err(MappingError::UnknownMember { .. })),
        "{r:?}"
    );
    let mut tags = example_tags();
    tags.push(ClassMappingSpec::new("Plane"));
    assert!(matches!(
        Mapping::new(example_model(), &tags),
        Err(MappingError::UnknownClass(_))
    ));
    let mut tags = example_tags();
    tags.push(ClassMappingSpec::new("Radar"));
    assert!(matches!(
        Mapping::new(example_model(), &tags),
        Err(MappingError::DuplicateClassMapping(_))
    ));
    // Two members writing one field.
    let r = with_track(|t| {
        t.members[3] = MemberMappingSpec::MonoRelation {
            name: "a_radar".into(),
            place_topic: None,
            target_key: KeyDescription::simple_oid("X"),
        };
    });
    assert!(
        matches!(
            r,
            Err(MappingError::FieldUsedTwice { .. } | MappingError::FieldConflict { .. })
        ),
        "{r:?}"
    );
    // One topic with two keys: y placed in the main topic of Radar.
    let r = with_track(|t| {
        t.members[1] = MemberMappingSpec::MonoAttribute {
            name: "y".into(),
            place_topic: Some(TopicSpec::new(
                "RADAR-TOPIC",
                KeyDescription::full_oid("CLASS", "OID"),
            )),
            value_fields: vec!["Y".into()],
        };
    });
    assert!(matches!(r, Err(MappingError::KeyConflict { .. })), "{r:?}");
    // One topic with two type names.
    let r = with_track(|t| {
        t.members[1] = MemberMappingSpec::MonoAttribute {
            name: "y".into(),
            place_topic: Some(TopicSpec {
                name: "TRACK-TOPIC".into(),
                type_name: Some("TrackType".into()),
                key: KeyDescription::full_oid("CLASS", "OID"),
            }),
            value_fields: vec!["Y".into()],
        };
    });
    assert!(
        matches!(r, Err(MappingError::TypeNameConflict { .. })),
        "{r:?}"
    );
    // A value field named twice: each leaf of a structure has a field of
    // its own, for a mono-valued and for a multi-valued attribute.
    let r = Mapping::new(
        kinds_model(),
        &[ClassMappingSpec {
            class: "Probe".into(),
            members: vec![MemberMappingSpec::MonoAttribute {
                name: "at".into(),
                place_topic: None,
                value_fields: vec!["P".into(), "P".into()],
            }],
            ..ClassMappingSpec::default()
        }],
    );
    assert_eq!(
        r.as_ref().err(),
        Some(&MappingError::RepeatedValueField {
            class: "Probe".into(),
            member: "at".into(),
            field: "P".into(),
        }),
        "{r:?}"
    );
    let at = SimpleType::Struct(SimpleStructType::new(
        "At",
        &[
            ("x", SimpleType::Basic(BasicType::Real)),
            ("y", SimpleType::Basic(BasicType::Real)),
        ],
    ));
    let path = || {
        Model::new(vec![ClassModel::new("Path").base(OBJECT_ROOT).multi(
            "points",
            CollectionBasis::List,
            at.clone(),
        )])
        .unwrap()
    };
    let points = |fields: [&str; 2]| ClassMappingSpec {
        class: "Path".into(),
        members: vec![MemberMappingSpec::MultiAttribute {
            name: "points".into(),
            topic: MultiTopicSpec::new("POINTS", Some("I"), KeyDescription::simple_oid("oid")),
            value_fields: fields.iter().map(|f| (*f).to_string()).collect(),
        }],
        ..ClassMappingSpec::default()
    };
    let r = Mapping::new(path(), &[points(["P", "P"])]);
    assert_eq!(
        r.as_ref().err(),
        Some(&MappingError::RepeatedValueField {
            class: "Path".into(),
            member: "points".into(),
            field: "P".into(),
        }),
        "{r:?}"
    );
    // With two distinct fields both leaves come back.
    let m = Mapping::new(path(), &[points(["PX", "PY"])]).unwrap();
    let o = ObjectData::new("Path", oid(4)).with(
        "points",
        AttributeValue::List(vec![Value::Struct {
            type_name: "At".into(),
            fields: vec![
                ("x".into(), Value::Real(1.0)),
                ("y".into(), Value::Real(2.0)),
            ],
        }]),
    );
    let rows = m.rows_of(&o).unwrap();
    assert_eq!(m.reconstruct(&rows).unwrap().objects, vec![o]);
}

// ---------------------------------------------------------------------------
// §8.1.4.3
// ---------------------------------------------------------------------------

#[test]
fn dlrl_8_1_4_3_the_minimal_tags_of_8_2_3_3_yield_the_default_topics_and_fields() {
    // The minimal tags carry no classMapping beyond <local name="w"/>,
    // which is part of the model here.
    let m = Mapping::default_for(example_model()).unwrap();
    let t = |n: &str| m.topic(n).unwrap_or_else(|| panic!("topic {n}"));
    let class = field("class", CellType::String, true);
    let oid_f = field("oid", CellType::Oid, true);
    assert_eq!(
        layout(t("Track")),
        expect(&[
            class.clone(),
            oid_f.clone(),
            field("x", CellType::Double, false),
            field("y", CellType::Double, false),
            field("a_radar", CellType::Oid, false),
        ])
    );
    assert_eq!(
        layout(t("Track.comments")),
        expect(&[
            class.clone(),
            oid_f.clone(),
            field("index", CellType::String, true),
            field("comments", CellType::String, false),
        ])
    );
    assert_eq!(
        layout(t("Track3D")),
        expect(&[class, oid_f.clone(), field("z", CellType::Double, false)])
    );
    assert_eq!(layout(t("Radar")), expect(std::slice::from_ref(&oid_f)));
    assert_eq!(
        layout(t("Radar.tracks")),
        expect(&[
            oid_f,
            field("index", CellType::Long, true),
            field("tracks.class", CellType::String, false),
            field("tracks.oid", CellType::Oid, false),
        ])
    );
    assert_eq!(m.topics().len(), 5);
    // Track3D lives in the main topic of Track with its extension table.
    let t3 = m.class("Track3D").unwrap();
    assert_eq!(t3.main_topic.name, "Track");
    assert_eq!(t3.extension_topic.as_ref().unwrap().name, "Track3D");
    // Radar has no derived class and its own main topic: no class part.
    assert!(!m.class("Radar").unwrap().full_oid_required());
    assert!(m.class("Track").unwrap().full_oid_required());
}

#[test]
fn dlrl_8_1_4_3_a_given_mapping_overrides_the_default_per_attribute() {
    let pos = SimpleStructType::new("Pos", &[("lat", double()), ("lon", double())]);
    let one = SimpleStructType::new("One", &[("only", long())]);
    let model = Model::new(vec![
        ClassModel::new("Ship")
            .base(OBJECT_ROOT)
            .mono("name", string())
            .mono("pos", SimpleType::Struct(pos.clone()))
            .mono("tag", SimpleType::Struct(one))
            .mono("speed", double()),
        ClassModel::new("Port")
            .base(OBJECT_ROOT)
            .relation(Relation::to_one("ship", "Ship")),
    ])
    .unwrap();
    // Defaults: '.' joins attribute and sub-field; one field keeps the
    // attribute name.
    let d = Mapping::default_for(model.clone()).unwrap();
    let ship = d.topic("Ship").unwrap();
    for f in ["name", "pos.lat", "pos.lon", "tag", "speed"] {
        assert!(ship.field(f).is_some(), "{f}");
    }
    assert_eq!(
        d.topic("Port").unwrap().field("ship").map(|f| f.cell),
        Some(CellType::Oid)
    );
    // Only pos and the main topic given: the rest keeps the default.
    let spec = ClassMappingSpec {
        class: "Ship".into(),
        main_topic: Some(TopicSpec::new("SHIPS", KeyDescription::simple_oid("ID"))),
        extension_topic: None,
        members: vec![MemberMappingSpec::MonoAttribute {
            name: "pos".into(),
            place_topic: Some(TopicSpec::new(
                "POSITIONS",
                KeyDescription::simple_oid("SHIP"),
            )),
            value_fields: vec!["LAT".into(), "LON".into()],
        }],
    };
    let m = Mapping::new(model, &[spec]).unwrap();
    let ships = m.topic("SHIPS").unwrap();
    assert_eq!(ships.key_fields(), vec!["ID"]);
    for f in ["name", "tag", "speed"] {
        assert!(ships.field(f).is_some(), "{f}");
    }
    assert!(ships.field("pos.lat").is_none());
    let positions = m.topic("POSITIONS").unwrap();
    assert_eq!(
        layout(positions),
        expect(&[
            field("SHIP", CellType::Oid, true),
            field("LAT", CellType::Double, false),
            field("LON", CellType::Double, false),
        ])
    );
    assert!(m.topic("Ship").is_none());
    // The given names reach the rows.
    let o = ObjectData::new("Ship", oid(4))
        .with("name", mono(Value::String("Eider".into())))
        .with(
            "pos",
            mono(Value::Struct {
                type_name: "Pos".into(),
                fields: vec![
                    ("lat".into(), Value::Double(54.3)),
                    ("lon".into(), Value::Double(10.1)),
                ],
            }),
        )
        .with(
            "tag",
            mono(Value::Struct {
                type_name: "One".into(),
                fields: vec![("only".into(), Value::Long(7))],
            }),
        )
        .with("speed", mono(Value::Double(12.0)));
    let rows = m.rows_of(&o).unwrap();
    assert_eq!(rows[0].topic, "SHIPS");
    assert_eq!(rows[0].get("ID"), Some(&Cell::Oid(oid(4))));
    assert_eq!(rows[0].get("tag"), Some(&Cell::Long(7)));
    assert_eq!(rows[1].topic, "POSITIONS");
    assert_eq!(rows[1].get("LON"), Some(&Cell::Double(10.1)));
    assert_eq!(m.reconstruct(&rows).unwrap().objects, vec![o]);
}

// ---------------------------------------------------------------------------
// §8.1.4.2
// ---------------------------------------------------------------------------

#[test]
fn dlrl_8_1_4_2_1_an_object_exists_with_its_row_in_the_main_topic() {
    let m = example_mapping();
    let obj = track3d(1);
    let rows = m.rows_of(&obj).unwrap();
    // Main row first, then the extension and place rows, then one row
    // per comment.
    let topics: Vec<&str> = rows.iter().map(|r| r.topic.as_str()).collect();
    assert_eq!(
        topics,
        vec![
            "TRACK-TOPIC",
            "Y_TOPIC",
            "TRACK3D-TOPIC",
            "COMMENTS-TOPIC",
            "COMMENTS-TOPIC"
        ]
    );
    let back = m.reconstruct(&rows).unwrap();
    assert_eq!(back.objects, vec![obj.clone()]);
    assert!(back.skipped.is_empty());

    // Without the main row: no object, and every other row is reported.
    let without_main: Vec<Row> = rows
        .iter()
        .filter(|r| r.topic != "TRACK-TOPIC")
        .cloned()
        .collect();
    let back = m.reconstruct(&without_main).unwrap();
    assert!(back.objects.is_empty());
    assert_eq!(back.skipped.len(), 4);
    assert!(back.skipped.iter().all(|s| s.reason
        == SkipReason::NoMainRow {
            class: "Track3D".into(),
            oid: oid(1)
        }));

    // The main row alone: the object exists; members placed elsewhere
    // have no value yet, the collection is empty.
    let back = m.reconstruct(&rows[..1]).unwrap();
    let o = back.object("Track3D", oid(1)).unwrap();
    assert_eq!(o.values.get("x"), Some(&mono(Value::Double(1.5))));
    assert!(!o.values.contains_key("y"));
    assert!(!o.values.contains_key("z"));
    assert_eq!(
        o.values.get("comments"),
        Some(&AttributeValue::StrMap(BTreeMap::new()))
    );
    // Two main rows for one object are refused.
    let twice = [rows[0].clone(), rows[0].clone()];
    assert!(matches!(
        m.reconstruct(&twice),
        Err(RowError::DuplicateRow { .. })
    ));
}

#[test]
fn dlrl_8_1_4_2_2_the_class_part_is_required_for_a_shared_main_topic_and_for_a_target_with_derived_classes()
 {
    let m = example_mapping();
    // A Track and a Track3D with the same oid value are two objects: the
    // class part of the key tells them apart in the shared main topic.
    let track = ObjectData::new("Track", oid(1))
        .with("x", mono(Value::Double(9.0)))
        .with("y", mono(Value::Double(8.0)))
        .with("comments", AttributeValue::StrMap(BTreeMap::new()))
        .with("a_radar", mono(Value::Ref(None)));
    let t3 = track3d(1);
    let mut rows = m.rows_of(&track).unwrap();
    assert_eq!(rows[0].get("CLASS"), Some(&Cell::String("Track".into())));
    rows.extend(m.rows_of(&t3).unwrap());
    let r = radar(100, vec![t3.reference(), track.reference()]);
    let radar_rows = m.rows_of(&r).unwrap();
    // Radar needs no class part: no class field in its rows' keys.
    assert_eq!(radar_rows[0].cells.len(), 1);
    // The references to tracks carry the class part.
    assert_eq!(
        radar_rows[1].get("TRACK-CLASS"),
        Some(&Cell::String("Track3D".into()))
    );
    assert_eq!(
        radar_rows[2].get("TRACK-CLASS"),
        Some(&Cell::String("Track".into()))
    );
    rows.extend(radar_rows);
    let back = m.reconstruct(&rows).unwrap();
    assert_eq!(back.objects.len(), 3);
    assert_eq!(back.object("Track", oid(1)), Some(&track));
    assert_eq!(back.object("Track3D", oid(1)), Some(&t3));
    assert_eq!(back.object("Radar", oid(100)), Some(&r));

    // The mapping refuses what would lose the class.
    let r = with_radar(|t| {
        t.members[0] = MemberMappingSpec::MultiRelation {
            name: "tracks".into(),
            topic: MultiTopicSpec::new(
                "RADARTRACKS-TOPIC",
                Some("INDEX"),
                KeyDescription::simple_oid("RADAR-OID"),
            ),
            target_key: KeyDescription::simple_oid("TRACK-OID"),
        };
    });
    assert!(matches!(r, Err(MappingError::TargetKey { .. })), "{r:?}");
    // Track with a key without class part, Track3D by default in the
    // same main topic: two classes in one table without a class part.
    let mut tags = example_tags();
    tags.remove(1);
    tags[0].main_topic = Some(TopicSpec::new(
        "TRACK-TOPIC",
        KeyDescription::simple_oid("OID"),
    ));
    for m in &mut tags[0].members {
        match m {
            MemberMappingSpec::MonoAttribute {
                place_topic: Some(p),
                ..
            } => p.key = KeyDescription::simple_oid("OID"),
            MemberMappingSpec::MultiAttribute { topic, .. } => {
                topic.key = KeyDescription::simple_oid("OID");
            }
            _ => {}
        }
    }
    let r = Mapping::new(example_model(), &tags);
    assert_eq!(
        r.as_ref().err(),
        Some(&MappingError::ClassPartRequired {
            topic: "TRACK-TOPIC".into(),
            classes: vec!["Track".into(), "Track3D".into()],
        }),
        "{r:?}"
    );
    // A class part naming a class of no local model is reported.
    let mut foreign = rows[0].clone();
    foreign.cells[0].1 = Cell::String("Plane".into());
    let back = m.reconstruct(&[foreign]).unwrap();
    assert!(back.objects.is_empty());
    assert_eq!(
        back.skipped[0].reason,
        SkipReason::UnknownClass("Plane".into())
    );
    // A class part naming a class that has no rows in the topic.
    let mut misplaced = m.rows_of(&t3).unwrap()[2].clone();
    assert_eq!(misplaced.topic, "TRACK3D-TOPIC");
    misplaced.cells[0].1 = Cell::String("Track".into());
    let back = m.reconstruct(&[misplaced]).unwrap();
    assert_eq!(
        back.skipped[0].reason,
        SkipReason::NotMappedHere("Track".into())
    );
}

#[test]
fn dlrl_8_1_4_2_2_the_nil_oid_designates_no_object() {
    let m = example_mapping();
    let track = |n: DlrlOid, radar: Option<ObjectRef>| {
        ObjectData::new("Track", n)
            .with("x", mono(Value::Double(1.0)))
            .with("y", mono(Value::Double(2.0)))
            .with("comments", AttributeValue::StrMap(BTreeMap::new()))
            .with("a_radar", mono(Value::Ref(radar)))
    };
    // An object with the nil oid has no rows.
    assert_eq!(
        m.rows_of(&track(DlrlOid::NIL, None)),
        Err(RowError::NilOid {
            class: "Track".into(),
            topic: "TRACK-TOPIC".into(),
        })
    );
    assert_eq!(
        m.rows_of(
            &ObjectData::new("Radar", DlrlOid::NIL).with("tracks", AttributeValue::List(vec![]))
        )
        .err(),
        Some(RowError::NilOid {
            class: "Radar".into(),
            topic: "RADAR-TOPIC".into(),
        })
    );
    // A reference cannot name an object by the nil oid, neither to-1
    // nor as a collection item.
    assert_eq!(
        m.rows_of(&track(oid(1), Some(ObjectRef::new("Radar", DlrlOid::NIL)))),
        Err(RowError::Type(TypeError::NilOid {
            member: "a_radar".into()
        }))
    );
    let r = ObjectData::new("Radar", oid(100)).with(
        "tracks",
        AttributeValue::List(vec![Value::Ref(Some(ObjectRef::new(
            "Track",
            DlrlOid::NIL,
        )))]),
    );
    assert_eq!(
        m.rows_of(&r),
        Err(RowError::Type(TypeError::NilOid {
            member: "tracks".into()
        }))
    );

    // A row whose owner key holds the nil oid is refused when read: in
    // the main topic with a class part, in a place topic, and in a main
    // topic without class part.
    let rows = m.rows_of(&track(oid(1), None)).unwrap();
    assert_eq!(rows[0].topic, "TRACK-TOPIC");
    assert_eq!(rows[1].topic, "Y_TOPIC");
    let nil_owner = |row: &Row| {
        let mut row = row.clone();
        for (name, cell) in &mut row.cells {
            if name == "OID" {
                *cell = Cell::Oid(DlrlOid::NIL);
            }
        }
        row
    };
    for (i, topic) in [(0, "TRACK-TOPIC"), (1, "Y_TOPIC")] {
        let mut changed = rows.clone();
        changed[i] = nil_owner(&rows[i]);
        assert_eq!(
            m.reconstruct(&changed),
            Err(RowError::NilOid {
                class: "Track".into(),
                topic: topic.into(),
            })
        );
    }
    let radar_rows = m.rows_of(&radar(100, vec![])).unwrap();
    assert_eq!(
        m.reconstruct(&[nil_owner(&radar_rows[0])]),
        Err(RowError::NilOid {
            class: "Radar".into(),
            topic: "RADAR-TOPIC".into(),
        })
    );

    // A reference to no object is an empty class part with the nil oid
    // and reads back as no object; a class part beside the nil oid is
    // refused.
    let full = with_track(|t| {
        t.members[3] = MemberMappingSpec::MonoRelation {
            name: "a_radar".into(),
            place_topic: None,
            target_key: KeyDescription::full_oid("RADAR_CLASS", "RADAR_OID"),
        };
    })
    .unwrap();
    let unset = track(oid(2), None);
    let rows = full.rows_of(&unset).unwrap();
    assert_eq!(
        rows[0].get("RADAR_CLASS"),
        Some(&Cell::String(String::new()))
    );
    assert_eq!(rows[0].get("RADAR_OID"), Some(&Cell::Oid(DlrlOid::NIL)));
    assert_eq!(full.reconstruct(&rows).unwrap().objects, vec![unset]);
    let mut named = rows.clone();
    for (name, cell) in &mut named[0].cells {
        if name == "RADAR_CLASS" {
            *cell = Cell::String("Radar".into());
        }
    }
    assert!(
        matches!(
            full.reconstruct(&named),
            Err(RowError::BadCell { ref field, .. }) if field == "RADAR_CLASS"
        ),
        "{:?}",
        full.reconstruct(&named)
    );
}

fn kinds_model() -> Model {
    Model::new(vec![
        ClassModel::new("Probe")
            .base(OBJECT_ROOT)
            .mono("count", long())
            .mono(
                "level",
                SimpleType::Enumeration(EnumerationType::new("Level", &["LOW", "HIGH"])),
            )
            .mono(
                "mode",
                SimpleType::Enumeration(
                    EnumerationType::new("Mode", &["IDLE", "RUN"]).coded_as(&["idle", "running"]),
                ),
            )
            .mono(
                "at",
                SimpleType::Struct(SimpleStructType::new(
                    "At",
                    &[
                        ("x", SimpleType::Basic(BasicType::Real)),
                        ("y", SimpleType::Basic(BasicType::Real)),
                    ],
                )),
            )
            .mono(
                "samples",
                SimpleType::Basic(BasicType::Sequence(BasicItem::Octet)),
            )
            .relation(Relation::to_one("peer", "Probe")),
    ])
    .unwrap()
}

#[test]
fn dlrl_8_1_4_2_3_cells_per_attribute_kind() {
    let m = Mapping::default_for(kinds_model()).unwrap();
    let o = ObjectData::new("Probe", oid(2))
        .with("count", mono(Value::Long(-3)))
        .with(
            "level",
            mono(Value::Enum {
                type_name: "Level".into(),
                enumerator: "HIGH".into(),
            }),
        )
        .with(
            "mode",
            mono(Value::Enum {
                type_name: "Mode".into(),
                enumerator: "RUN".into(),
            }),
        )
        .with(
            "at",
            mono(Value::Struct {
                type_name: "At".into(),
                fields: vec![
                    ("x".into(), Value::Real(1.0)),
                    ("y".into(), Value::Real(-1.0)),
                ],
            }),
        )
        .with(
            "samples",
            mono(Value::Sequence(SequenceValue::Octet(vec![1, 2, 3]))),
        )
        .with(
            "peer",
            mono(Value::Ref(Some(ObjectRef::new("Probe", oid(7))))),
        );
    let rows = m.rows_of(&o).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].cells,
        vec![
            ("oid".to_string(), Cell::Oid(oid(2))),
            ("count".to_string(), Cell::Long(-3)),
            // enumeration as integer by default …
            ("level".to_string(), Cell::Long(1)),
            // … or as string with an enumDef
            ("mode".to_string(), Cell::String("running".into())),
            // a structure as one cell per member
            ("at.x".to_string(), Cell::Real(1.0)),
            ("at.y".to_string(), Cell::Real(-1.0)),
            (
                "samples".to_string(),
                Cell::Sequence(SequenceValue::Octet(vec![1, 2, 3]))
            ),
            // a reference as the oid of the target (no class part needed)
            ("peer".to_string(), Cell::Oid(oid(7))),
        ]
    );
    assert_eq!(m.reconstruct(&rows).unwrap().objects, vec![o.clone()]);
    // A cell of the wrong type and an enumerator out of range are
    // refused when read.
    let mut bad = rows.clone();
    bad[0].cells[1].1 = Cell::Short(1);
    assert!(matches!(m.reconstruct(&bad), Err(RowError::BadCell { .. })));
    let mut bad = rows.clone();
    bad[0].cells[2].1 = Cell::Long(2);
    assert!(matches!(m.reconstruct(&bad), Err(RowError::BadCell { .. })));
    let mut bad = rows;
    bad[0].cells.retain(|(n, _)| n != "at.y");
    assert!(matches!(
        m.reconstruct(&bad),
        Err(RowError::MissingField { .. })
    ));
    // An object without a value for a mono-valued member is refused.
    let mut partial = o;
    partial.values.remove("count");
    assert!(matches!(
        m.rows_of(&partial),
        Err(RowError::MissingValue { .. })
    ));
}

#[test]
fn dlrl_8_1_4_2_3_a_collection_is_one_row_per_item_keyed_by_owner_and_index() {
    let model = Model::new(vec![
        ClassModel::new("Log")
            .base(OBJECT_ROOT)
            .multi("lines", CollectionBasis::List, string())
            .multi("by_name", CollectionBasis::StrMap, long())
            .multi("by_code", CollectionBasis::IntMap, double())
            .multi("tags", CollectionBasis::Set, string()),
    ])
    .unwrap();
    let m = Mapping::default_for(model).unwrap();
    let keys = |t: &str| m.topic(t).unwrap().key_fields();
    assert_eq!(keys("Log.lines"), vec!["oid", "index"]);
    assert_eq!(
        m.topic("Log.lines").unwrap().field("index").unwrap().cell,
        CellType::Long
    );
    assert_eq!(
        m.topic("Log.by_name").unwrap().field("index").unwrap().cell,
        CellType::String
    );
    assert_eq!(
        m.topic("Log.by_code").unwrap().field("index").unwrap().cell,
        CellType::Long
    );
    // A set is designated by its value.
    assert_eq!(keys("Log.tags"), vec!["oid", "tags"]);

    let mut by_name = BTreeMap::new();
    by_name.insert("a".to_string(), Value::Long(1));
    let mut by_code = BTreeMap::new();
    by_code.insert(404, Value::Double(0.5));
    by_code.insert(-1, Value::Double(1.5));
    let o = ObjectData::new("Log", oid(3))
        .with(
            "lines",
            AttributeValue::List(vec![
                Value::String("one".into()),
                Value::String("two".into()),
                Value::String("three".into()),
            ]),
        )
        .with("by_name", AttributeValue::StrMap(by_name))
        .with("by_code", AttributeValue::IntMap(by_code))
        .with(
            "tags",
            AttributeValue::Set(vec![Value::String("x".into()), Value::String("y".into())]),
        );
    let rows = m.rows_of(&o).unwrap();
    let of = |t: &str| rows.iter().filter(|r| r.topic == t).collect::<Vec<_>>();
    assert_eq!(of("Log").len(), 1);
    let lines = of("Log.lines");
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[2].get("index"), Some(&Cell::Long(2)));
    assert_eq!(lines[2].get("lines"), Some(&Cell::String("three".into())));
    assert_eq!(lines[2].get("oid"), Some(&Cell::Oid(oid(3))));
    assert_eq!(
        of("Log.by_name")[0].get("index"),
        Some(&Cell::String("a".into()))
    );
    assert_eq!(of("Log.by_code").len(), 2);
    let tags = of("Log.tags");
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().all(|r| r.get("index").is_none()));
    // Back in any order.
    let mut shuffled: Vec<Row> = rows.iter().rev().cloned().collect();
    shuffled.rotate_left(2);
    assert_eq!(m.reconstruct(&shuffled).unwrap().objects, vec![o]);
    // A gap in the ranks of a list, and two rows for one index.
    let gap: Vec<Row> = rows
        .iter()
        .filter(|r| !(r.topic == "Log.lines" && r.get("index") == Some(&Cell::Long(1))))
        .cloned()
        .collect();
    assert!(matches!(m.reconstruct(&gap), Err(RowError::ListGap { .. })));
    let mut twice = rows.clone();
    twice.push(lines[0].clone());
    assert!(matches!(
        m.reconstruct(&twice),
        Err(RowError::DuplicateRow { .. })
    ));
}

/// §8.1.4.5: a fixed DCPS model — a topic keyed by `SITE`, `NUM`, no
/// class and no oid field — and a relation that designates its rows by
/// those two fields.
fn legacy_mapping() -> Mapping {
    let model = Model::new(vec![
        ClassModel::new("Station")
            .base(OBJECT_ROOT)
            .mono("site", string())
            .mono("num", long())
            .mono("level", double()),
        ClassModel::new("Probe")
            .base(OBJECT_ROOT)
            .mono("label", string())
            .relation(Relation::to_one("station", "Station"))
            .relation(Relation::to_many("seen", "Station", CollectionBasis::List)),
    ])
    .unwrap();
    let station = ClassMappingSpec {
        class: "Station".into(),
        main_topic: Some(TopicSpec {
            type_name: Some("Legacy::StationType".into()),
            ..TopicSpec::new("STATION", KeyDescription::no_oid(&["SITE", "NUM"]))
        }),
        members: vec![
            MemberMappingSpec::MonoAttribute {
                name: "site".into(),
                place_topic: None,
                value_fields: vec!["SITE".into()],
            },
            MemberMappingSpec::MonoAttribute {
                name: "num".into(),
                place_topic: None,
                value_fields: vec!["NUM".into()],
            },
            MemberMappingSpec::MonoAttribute {
                name: "level".into(),
                place_topic: None,
                value_fields: vec!["LEVEL".into()],
            },
        ],
        ..ClassMappingSpec::default()
    };
    let probe = ClassMappingSpec {
        class: "Probe".into(),
        members: vec![
            MemberMappingSpec::MonoRelation {
                name: "station".into(),
                place_topic: None,
                target_key: KeyDescription::no_oid(&["AT_SITE", "AT_NUM"]),
            },
            MemberMappingSpec::MultiRelation {
                name: "seen".into(),
                topic: MultiTopicSpec::new(
                    "PROBE_SEEN",
                    Some("RANK"),
                    KeyDescription::simple_oid("PROBE"),
                ),
                target_key: KeyDescription::no_oid(&["S_SITE", "S_NUM"]),
            },
        ],
        ..ClassMappingSpec::default()
    };
    Mapping::new(model, &[station, probe]).unwrap()
}

fn station(site: &str, num: i32, level: f64) -> ObjectData {
    ObjectData::new("Station", oid(900 + num))
        .with("site", mono(Value::String(site.into())))
        .with("num", mono(Value::Long(num)))
        .with("level", mono(Value::Double(level)))
}

#[test]
fn dlrl_8_1_4_5_objects_over_a_keyed_topic_without_oid_fields() {
    let m = legacy_mapping();
    // The topic is the fixed one: its key fields are the identity
    // attributes, typed by them; no class and no oid field.
    assert_eq!(m.class("Station").unwrap().identity, vec!["site", "num"]);
    let t = m.topic("STATION").unwrap();
    assert_eq!(t.type_name, "Legacy::StationType");
    assert_eq!(
        layout(t),
        expect(&[
            field("SITE", CellType::String, true),
            field("NUM", CellType::Long, true),
            field("LEVEL", CellType::Double, false),
        ])
    );
    // The relation's target fields take the target's key types.
    let probe = m.topic("Probe").unwrap();
    assert_eq!(probe.field("AT_SITE").unwrap().cell, CellType::String);
    assert_eq!(probe.field("AT_NUM").unwrap().cell, CellType::Long);

    // The row of a Station carries its key values and no oid.
    let a = station("north", 7, 1.5);
    let rows = m.rows_of(&a).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].cells,
        vec![
            ("SITE".to_string(), Cell::String("north".into())),
            ("NUM".to_string(), Cell::Long(7)),
            ("LEVEL".to_string(), Cell::Double(1.5)),
        ]
    );
    assert_eq!(
        m.identity_key(&a).unwrap(),
        vec![Cell::String("north".into()), Cell::Long(7)]
    );

    // Reading: one key value, one oid, the same in every round and for
    // the references that hold the key.
    let mut keys = zerodds_dlrl::rows::KeyAssociation::with_creator(5, 6);
    let first = m.reconstruct_with(&rows, &mut keys).unwrap();
    let got = &first.objects[0];
    assert_eq!(got.class, "Station");
    assert_eq!(got.values["num"], mono(Value::Long(7)));
    assert_eq!(got.oid.value[..2], [5, 6]);
    let probe_row = Row {
        topic: "Probe".into(),
        cells: vec![
            ("oid".into(), Cell::Oid(oid(1))),
            ("label".into(), Cell::String("p".into())),
            ("AT_SITE".into(), Cell::String("north".into())),
            ("AT_NUM".into(), Cell::Long(7)),
        ],
    };
    let again = m
        .reconstruct_with([&rows[0], &probe_row], &mut keys)
        .unwrap();
    assert_eq!(again.object("Station", got.oid).unwrap(), got);
    assert_eq!(
        again.object("Probe", oid(1)).unwrap().values["station"],
        mono(Value::Ref(Some(ObjectRef::new("Station", got.oid))))
    );
    // Another key value is another object.
    let b = m.rows_of(&station("south", 7, 0.0)).unwrap();
    let other = m.reconstruct_with(&b, &mut keys).unwrap();
    assert_ne!(other.objects[0].oid, got.oid);

    // Writing a reference needs the association: the oid alone is not
    // on the wire.
    let p = ObjectData::new("Probe", oid(2))
        .with("label", mono(Value::String("q".into())))
        .with(
            "station",
            mono(Value::Ref(Some(ObjectRef::new("Station", got.oid)))),
        )
        .with(
            "seen",
            AttributeValue::List(vec![Value::Ref(Some(ObjectRef::new("Station", got.oid)))]),
        );
    assert_eq!(
        m.rows_of(&p),
        Err(RowError::UnknownKey {
            class: "Station".into(),
            oid: got.oid
        })
    );
    let rows = m.rows_of_with(&p, &keys).unwrap();
    let main = rows.iter().find(|r| r.topic == "Probe").unwrap();
    assert_eq!(main.get("AT_SITE"), Some(&Cell::String("north".into())));
    assert_eq!(main.get("AT_NUM"), Some(&Cell::Long(7)));
    let seen = rows.iter().find(|r| r.topic == "PROBE_SEEN").unwrap();
    assert_eq!(seen.get("S_NUM"), Some(&Cell::Long(7)));
    // "No object" has no value in fixed key fields.
    let none = ObjectData::new("Probe", oid(3))
        .with("label", mono(Value::String("r".into())))
        .with("station", mono(Value::Ref(None)))
        .with("seen", AttributeValue::List(vec![]));
    assert!(matches!(
        m.rows_of_with(&none, &keys),
        Err(RowError::NoObjectInNoOidReference { .. })
    ));
}

#[test]
fn dlrl_8_1_4_5_a_key_field_must_be_an_identity_attribute() {
    let model = || {
        Model::new(vec![
            ClassModel::new("Legacy")
                .base(OBJECT_ROOT)
                .mono("v", long())
                .mono(
                    "history",
                    SimpleType::Basic(BasicType::Sequence(BasicItem::Long)),
                ),
        ])
        .unwrap()
    };
    let spec = |fields: &[&str], members: Vec<MemberMappingSpec>| ClassMappingSpec {
        class: "Legacy".into(),
        main_topic: Some(TopicSpec::new("LEGACY", KeyDescription::no_oid(fields))),
        members,
        ..ClassMappingSpec::default()
    };
    // A key field no attribute fills.
    assert_eq!(
        Mapping::new(model(), &[spec(&["SITE"], vec![])]),
        Err(MappingError::NoOidKeyField {
            class: "Legacy".into(),
            field: "SITE".into()
        })
    );
    // A key field filled by a sequence: no key type.
    let by_sequence = vec![MemberMappingSpec::MonoAttribute {
        name: "history".into(),
        place_topic: None,
        value_fields: vec!["H".into()],
    }];
    assert!(matches!(
        Mapping::new(model(), &[spec(&["H"], by_sequence)]),
        Err(MappingError::NoOidKeyField { .. })
    ));
    // The identity attribute: accepted.
    let by_long = vec![MemberMappingSpec::MonoAttribute {
        name: "v".into(),
        place_topic: None,
        value_fields: vec!["V".into()],
    }];
    let m = Mapping::new(model(), &[spec(&["V"], by_long)]).unwrap();
    assert_eq!(m.class("Legacy").unwrap().identity, vec!["v"]);
    // Key cells are checked when read.
    let bad = Row {
        topic: "LEGACY".into(),
        cells: vec![
            ("V".into(), Cell::String("x".into())),
            (
                "history".into(),
                Cell::Sequence(SequenceValue::Long(vec![])),
            ),
        ],
    };
    assert!(matches!(
        m.reconstruct(&[bad]),
        Err(RowError::BadCell { .. })
    ));
}
