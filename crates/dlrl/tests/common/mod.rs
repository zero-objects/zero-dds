// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The example of DDS 1.2 §8.2.3 as fixture: the model of §8.2.3.2 and
//! the full model tags of §8.2.3.3.
//!
//! The specification spells some names twice: the XML of §8.2.3.3 writes
//! `Y_TOPIC` and `RADAR_OID`, the table of §8.2.3.4 `Y-TOPIC` and
//! `RADAR-OID`; the fixture takes the XML. `comments` is a `StrMap` of
//! strings as in the IDL of §8.2.3.2 (the UML of §8.2.3.1 draws a list).

#![allow(dead_code, clippy::panic)]

use zerodds_dlrl::mapping::{
    ClassMappingSpec, KeyDescription, MemberMappingSpec, MultiTopicSpec, TopicSpec,
};
use zerodds_dlrl::model::{
    BasicType, ClassModel, CollectionBasis, Model, OBJECT_ROOT, Relation, SimpleType,
};
use zerodds_dlrl::types::DlrlOid;

pub fn double() -> SimpleType {
    SimpleType::Basic(BasicType::Double)
}

pub fn string() -> SimpleType {
    SimpleType::Basic(BasicType::String)
}

pub fn long() -> SimpleType {
    SimpleType::Basic(BasicType::Long)
}

pub fn oid(n: i32) -> DlrlOid {
    DlrlOid::new(0, 1, n)
}

/// The classes of §8.2.3.2: `Track`, `Track3D : Track`, `Radar`, with
/// `Track::w` local (§8.2.3.3 `<local name="w"/>`) and the association
/// `Track::a_radar` / `Radar::tracks`.
pub fn example_classes() -> Vec<ClassModel> {
    vec![
        ClassModel::new("Track")
            .base("DLRL::ObjectRoot")
            .mono("x", double())
            .mono("y", double())
            .multi("comments", CollectionBasis::StrMap, string())
            .local("w")
            .relation(Relation::to_one("a_radar", "Radar").inverse_of("tracks")),
        ClassModel::new("Track3D").base("Track").mono("z", double()),
        ClassModel::new("Radar").base(OBJECT_ROOT).relation(
            Relation::to_many("tracks", "Track", CollectionBasis::List).inverse_of("a_radar"),
        ),
    ]
}

pub fn example_model() -> Model {
    match Model::new(example_classes()) {
        Ok(m) => m,
        Err(e) => panic!("the example model of §8.2.3.2 is refused: {e}"),
    }
}

fn full_oid() -> KeyDescription {
    KeyDescription::full_oid("CLASS", "OID")
}

/// The `classMapping` elements of the full model tags of §8.2.3.3.
pub fn example_tags() -> Vec<ClassMappingSpec> {
    vec![
        ClassMappingSpec {
            class: "Track".into(),
            main_topic: Some(TopicSpec::new("TRACK-TOPIC", full_oid())),
            extension_topic: None,
            members: vec![
                MemberMappingSpec::MonoAttribute {
                    name: "x".into(),
                    place_topic: None,
                    value_fields: vec!["X".into()],
                },
                MemberMappingSpec::MonoAttribute {
                    name: "y".into(),
                    place_topic: Some(TopicSpec::new("Y_TOPIC", full_oid())),
                    value_fields: vec!["Y".into()],
                },
                MemberMappingSpec::MultiAttribute {
                    name: "comments".into(),
                    topic: MultiTopicSpec::new("COMMENTS-TOPIC", Some("INDEX"), full_oid()),
                    value_fields: vec!["COMMENT".into()],
                },
                MemberMappingSpec::MonoRelation {
                    name: "a_radar".into(),
                    place_topic: None,
                    target_key: KeyDescription::simple_oid("RADAR_OID"),
                },
            ],
        },
        ClassMappingSpec {
            class: "Track3D".into(),
            main_topic: Some(TopicSpec::new("TRACK-TOPIC", full_oid())),
            extension_topic: Some(TopicSpec::new("TRACK3D-TOPIC", full_oid())),
            members: vec![MemberMappingSpec::MonoAttribute {
                name: "z".into(),
                place_topic: None,
                value_fields: vec!["Z".into()],
            }],
        },
        ClassMappingSpec {
            class: "Radar".into(),
            main_topic: Some(TopicSpec::new(
                "RADAR-TOPIC",
                KeyDescription::simple_oid("OID"),
            )),
            extension_topic: None,
            members: vec![MemberMappingSpec::MultiRelation {
                name: "tracks".into(),
                topic: MultiTopicSpec::new(
                    "RADARTRACKS-TOPIC",
                    Some("INDEX"),
                    KeyDescription::simple_oid("RADAR-OID"),
                ),
                target_key: KeyDescription::full_oid("TRACK-CLASS", "TRACK-OID"),
            }],
        },
    ]
}
