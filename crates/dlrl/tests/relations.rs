// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.3.2 relations among DLRL objects: single inheritance
//! under `ObjectRoot` (§8.1.3.2.1) and associations of to-1 and to-many
//! relations, plain or composition, with inverses (§8.1.3.2.2).

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{example_classes, example_model, long};
use zerodds_dlrl::mapping::{Mapping, MemberMapping};
use zerodds_dlrl::model::{
    ClassModel, CollectionBasis, Model, ModelError, Multiplicity, OBJECT_ROOT, OBJECT_ROOT_NAMES,
    Relation,
};

fn names(v: &[&ClassModel]) -> Vec<String> {
    v.iter().map(|c| c.name.clone()).collect()
}

#[test]
fn dlrl_8_1_3_2_1_track3d_is_a_track_and_an_object_root() {
    let m = example_model();
    assert_eq!(m.parent("Track3D").map(|c| c.name.as_str()), Some("Track"));
    assert!(
        m.parent("Track").is_none(),
        "Track is directly under ObjectRoot"
    );
    assert_eq!(names(&m.children("Track")), vec!["Track3D"]);
    assert!(m.children("Track3D").is_empty());
    assert_eq!(names(&m.lineage("Track3D")), vec!["Track3D", "Track"]);
    // The lineage ends at a class whose DLRL base is ObjectRoot.
    let top = m.lineage("Track3D").last().copied().unwrap();
    assert!(
        top.bases
            .iter()
            .any(|b| OBJECT_ROOT_NAMES.contains(&b.as_str()))
    );
    assert!(m.is_kind_of("Track3D", "Track"));
    assert!(m.is_kind_of("Track3D", "Track3D"));
    assert!(!m.is_kind_of("Track", "Track3D"));
    assert!(!m.is_kind_of("Radar", "Track"));
    // Track3D has the members of Track.
    assert!(m.member("Track3D", "x").is_some());
    assert!(m.member("Track3D", "a_radar").is_some());
    assert!(m.member("Track", "z").is_none());
    // Native bases beside the DLRL base (§8.1.3.2.1 "any number of
    // native language objects").
    let m = Model::new(vec![
        ClassModel::new("Plot")
            .base("Serializable")
            .base("DDS::ObjectRoot")
            .base("Comparable")
            .mono("n", long()),
    ])
    .unwrap();
    assert!(m.parent("Plot").is_none());
    assert_eq!(m.native_bases("Plot"), vec!["Serializable", "Comparable"]);
}

#[test]
fn dlrl_8_1_3_2_1_two_dlrl_bases_are_refused() {
    let mut classes = example_classes();
    classes.push(
        ClassModel::new("Hybrid")
            .base("Track")
            .base("Radar")
            .mono("h", long()),
    );
    assert_eq!(
        Model::new(classes),
        Err(ModelError::SeveralDlrlBases {
            class: "Hybrid".into(),
            bases: vec!["Track".into(), "Radar".into()],
        })
    );
    // ObjectRoot and a DLRL class are two DLRL bases as well.
    let mut classes = example_classes();
    classes.push(
        ClassModel::new("Hybrid")
            .base(OBJECT_ROOT)
            .base("Track")
            .mono("h", long()),
    );
    assert!(matches!(
        Model::new(classes),
        Err(ModelError::SeveralDlrlBases { .. })
    ));
}

#[test]
fn dlrl_8_1_3_2_1_every_class_descends_from_object_root_and_no_final_class_is_extended() {
    assert_eq!(
        Model::new(vec![
            ClassModel::new("Loose")
                .base("Serializable")
                .mono("n", long())
        ]),
        Err(ModelError::NoDlrlBase("Loose".into()))
    );
    assert_eq!(
        Model::new(vec![
            ClassModel::new("Base")
                .base(OBJECT_ROOT)
                .final_class()
                .mono("n", long()),
            ClassModel::new("Derived").base("Base").mono("m", long()),
        ]),
        Err(ModelError::FinalParent {
            class: "Derived".into(),
            parent: "Base".into(),
        })
    );
    assert!(matches!(
        Model::new(vec![
            ClassModel::new("A").base("B").mono("a", long()),
            ClassModel::new("B").base("A").mono("b", long()),
        ]),
        Err(ModelError::InheritanceCycle(_))
    ));
}

fn pair(left: Relation, right: Relation) -> Result<Model, ModelError> {
    Model::new(vec![
        ClassModel::new("L").base(OBJECT_ROOT).relation(left),
        ClassModel::new("R").base(OBJECT_ROOT).relation(right),
    ])
}

#[test]
fn dlrl_8_1_3_2_2_plain_inverses_make_1_1_1_n_and_n_m() {
    // 1-1
    pair(
        Relation::to_one("r", "R").inverse_of("l"),
        Relation::to_one("l", "L").inverse_of("r"),
    )
    .unwrap();
    // 1-n, with each collection basis on the to-many end
    for basis in [
        CollectionBasis::List,
        CollectionBasis::Set,
        CollectionBasis::StrMap,
        CollectionBasis::IntMap,
    ] {
        pair(
            Relation::to_one("r", "R").inverse_of("ls"),
            Relation::to_many("ls", "L", basis).inverse_of("r"),
        )
        .unwrap();
    }
    // n-m
    let m = pair(
        Relation::to_many("rs", "R", CollectionBasis::Set).inverse_of("ls"),
        Relation::to_many("ls", "L", CollectionBasis::List).inverse_of("rs"),
    )
    .unwrap();
    let (_, member) = m.member("L", "rs").unwrap();
    assert!(matches!(
        member,
        zerodds_dlrl::model::Member::Relation(Relation {
            multiplicity: Multiplicity::ToMany(CollectionBasis::Set),
            ..
        })
    ));
}

#[test]
fn dlrl_8_1_3_2_2_inverse_relations_name_each_other_and_their_types_match() {
    // The named inverse does not exist.
    assert!(matches!(
        pair(
            Relation::to_one("r", "R").inverse_of("nothing"),
            Relation::to_one("l", "L"),
        ),
        Err(ModelError::InverseNotMutual { .. })
    ));
    // It exists but does not name the relation back.
    assert!(matches!(
        pair(
            Relation::to_one("r", "R").inverse_of("l"),
            Relation::to_one("l", "L"),
        ),
        Err(ModelError::InverseNotMutual { .. })
    ));
    // It names it back but targets another class than the owner.
    let m = Model::new(vec![
        ClassModel::new("L")
            .base(OBJECT_ROOT)
            .relation(Relation::to_one("r", "R").inverse_of("l")),
        ClassModel::new("R")
            .base(OBJECT_ROOT)
            .relation(Relation::to_one("l", "Other").inverse_of("r")),
        ClassModel::new("Other").base(OBJECT_ROOT).mono("n", long()),
    ]);
    assert!(matches!(m, Err(ModelError::InverseTypeMismatch { .. })));
    // A relation targets a class of the model.
    assert!(matches!(
        Model::new(vec![
            ClassModel::new("L")
                .base(OBJECT_ROOT)
                .relation(Relation::to_one("r", "Nowhere"))
        ]),
        Err(ModelError::UnknownTarget { .. })
    ));
}

#[test]
fn dlrl_8_1_3_2_2_the_inverse_of_a_composition_must_be_to_1() {
    // 1-n: the compound holds a list of constituents, each constituent
    // a to-1 reference back.
    let m = pair(
        Relation::to_many("parts", "R", CollectionBasis::List)
            .composition()
            .inverse_of("whole"),
        Relation::to_one("whole", "L").inverse_of("parts"),
    )
    .unwrap();
    // 1-1
    pair(
        Relation::to_one("part", "R")
            .composition()
            .inverse_of("whole"),
        Relation::to_one("whole", "L").inverse_of("part"),
    )
    .unwrap();
    // The inverse of a composition cannot be to-many …
    assert!(matches!(
        pair(
            Relation::to_many("parts", "R", CollectionBasis::Set)
                .composition()
                .inverse_of("wholes"),
            Relation::to_many("wholes", "L", CollectionBasis::Set).inverse_of("parts"),
        ),
        Err(ModelError::CompositionInverse { .. })
    ));
    // … nor a composition itself.
    assert!(matches!(
        pair(
            Relation::to_one("part", "R")
                .composition()
                .inverse_of("whole"),
            Relation::to_one("whole", "L")
                .composition()
                .inverse_of("part"),
        ),
        Err(ModelError::CompositionInverse { .. })
    ));
    // The mapping carries is_composition (Figure 8.2).
    let mapping = Mapping::default_for(m).unwrap();
    let (_, parts) = mapping.member("L", "parts").unwrap();
    let (_, whole) = mapping.member("R", "whole").unwrap();
    assert!(matches!(parts, MemberMapping::MultiRelation(r) if r.is_composition));
    assert!(matches!(whole, MemberMapping::MonoRelation(r) if !r.is_composition));
}
