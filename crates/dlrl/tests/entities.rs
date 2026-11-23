// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.6.2 DLRL entities: the associations of Figure 8.4, the
//! isolation of Caches (one Publisher and one Subscriber per Cache,
//! relations only inside one Cache), and §8.1.3.1: two references to
//! one object designate one memory cell.

#![cfg(feature = "dcps")]
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, missing_docs)]

#[path = "support/dds.rs"]
mod dds;

use std::sync::Arc;

use dds::{Bus, coherent_pair, delete, enabled, mapping, mono, reference, wait_until};
use zerodds_dlrl::cache::{
    CacheBase, CacheDescription, CacheFactory, CacheKind, CacheUsage, Collection, FilterCriterion,
    ObjectScope, SelectionCriterion,
};
use zerodds_dlrl::metamodel::DlrlException;
use zerodds_dlrl::model::Value;
use zerodds_dlrl::types::CriterionKind;

const ALL: [&str; 4] = ["Radar", "Track", "Part", "Sensor"];

struct AllPass;

impl FilterCriterion for AllPass {}

/// Figure 8.4: from the CacheFactory to the Cache, its homes, their
/// Selections and criteria; from the Cache to its CacheAccesses, their
/// Contracts and the contracted objects; from an object to its home and
/// its owner (a Cache for a primary object, a CacheAccess for a clone).
#[test]
fn dlrl_8_1_6_2_figure_8_4_associations_navigate() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let factory = CacheFactory::get_instance();
    assert!(std::ptr::eq(factory, CacheFactory::get_instance()));
    let cache = factory.find_cache_by_name(sub.cache.name()).unwrap();
    assert_eq!(cache, sub.cache);
    assert_eq!(cache.homes(), sub.homes);
    let radars = cache.find_home_by_name("Radar").unwrap().unwrap();
    let sel = radars
        .create_selection(SelectionCriterion::Filter(Arc::new(AllPass)), true, false)
        .unwrap();
    assert_eq!(radars.selections(), vec![sel.clone()]);
    assert_eq!(sel.criterion().kind(), CriterionKind::Filter);
    assert_eq!(sel.home(), Some(radars.clone()));

    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let r = publ.home("Radar").create_object(&access).unwrap();
    r.set("name", mono(Value::String("r".into()))).unwrap();
    access.write().unwrap();
    let base = CacheBase::Cache(cache.clone());
    wait_until("the radar", || radars.find_object(r.oid(), &base).is_ok());
    let primary = radars.find_object(r.oid(), &base).unwrap();
    assert_eq!(primary.object_home(), Some(radars.clone()));
    assert_eq!(primary.owner(), Some(base.clone()));
    assert_eq!(base.kind(), CacheKind::Cache);
    assert_eq!(sel.members(), vec![primary.clone()]);

    let reading = cache.create_access(CacheUsage::ReadOnly).unwrap();
    assert_eq!(cache.sub_accesses(), vec![reading.clone()]);
    assert_eq!(reading.owner(), Some(cache.clone()));
    let contract = reading
        .create_contract(&primary, ObjectScope::SimpleObject, 0)
        .unwrap();
    assert_eq!(reading.contracts(), vec![contract.clone()]);
    assert_eq!(contract.contracted_object(), primary);
    reading.refresh().unwrap();
    let clone = radars
        .find_object(r.oid(), &CacheBase::Access(reading.clone()))
        .unwrap();
    assert_ne!(clone, primary);
    assert_eq!(clone.owner(), Some(CacheBase::Access(reading.clone())));
    assert_eq!(clone.object_home(), Some(radars.clone()));
    assert_eq!(
        CacheBase::Access(reading.clone()).kind(),
        CacheKind::CacheAccess
    );
    delete(publ);
    delete(sub);
}

/// §8.1.3.1, §8.1.6.3.14: an object is one memory cell in its Cache;
/// two relations to it and two look-ups by oid give the same cell.
#[test]
fn dlrl_8_1_3_1_two_references_to_one_oid_are_the_same_cell() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let r = publ.home("Radar").create_object(&access).unwrap();
    r.set("name", mono(Value::String("r".into()))).unwrap();
    let mut tracks = Vec::new();
    for x in [1.0, 2.0] {
        let t = publ.home("Track").create_object(&access).unwrap();
        t.set("x", mono(Value::Double(x))).unwrap();
        t.set("y", mono(Value::Double(0.0))).unwrap();
        t.set("a_radar", reference(&r)).unwrap();
        tracks.push(t);
    }
    access.write().unwrap();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("the tracks", || {
        sub.home("Track").get_objects(&base).unwrap().len() == 2
    });
    let t1 = sub
        .home("Track")
        .find_object(tracks[0].oid(), &base)
        .unwrap();
    let t2 = sub
        .home("Track")
        .find_object(tracks[1].oid(), &base)
        .unwrap();
    let via1 = t1.get_reference("a_radar").unwrap().unwrap();
    let via2 = t2.get_reference("a_radar").unwrap().unwrap();
    let direct = sub.home("Radar").find_object(r.oid(), &base).unwrap();
    assert_eq!(via1, via2);
    assert_eq!(via1, direct);
    assert_eq!(
        sub.home("Radar").find_object(r.oid(), &base).unwrap(),
        direct
    );
    // One cell: a later update is seen through every handle.
    r.set("name", mono(Value::String("s".into()))).unwrap();
    access.write().unwrap();
    wait_until("the new name", || {
        via1.get("name").unwrap() == mono(Value::String("s".into()))
    });
    assert_eq!(via2.get("name").unwrap(), mono(Value::String("s".into())));
    delete(publ);
    delete(sub);
}

/// §8.1.6.2 (Cache): a Publisher and a Subscriber are attached to one
/// Cache only; two Caches of one participant have their own.
#[test]
fn dlrl_8_1_6_2_a_publisher_and_a_subscriber_belong_to_one_cache() {
    let bus = Bus::new();
    let participant = bus.participant();
    let make = |n: u32| {
        CacheFactory::get_instance()
            .create_cache(
                CacheUsage::ReadWrite,
                CacheDescription {
                    name: format!("isolation-{}-{}-{n}", std::process::id(), bus.domain),
                    domain: participant.clone(),
                },
            )
            .unwrap()
    };
    let (a, b) = (make(1), make(2));
    let (pa, pb) = (a.the_publisher().unwrap(), b.the_publisher().unwrap());
    let (sa, sb) = (a.the_subscriber().unwrap(), b.the_subscriber().unwrap());
    assert!(!Arc::ptr_eq(&pa, &pb));
    assert!(!Arc::ptr_eq(&sa, &sb));
    CacheFactory::get_instance().delete_cache(&a).unwrap();
    CacheFactory::get_instance().delete_cache(&b).unwrap();
}

/// §8.1.6.2 (Cache): "Only DLRL objects belonging to one Cache can be put
/// in relation": a to-1 relation and a relation collection refuse an
/// object of another Cache, and an object without oid.
#[test]
fn dlrl_8_1_6_2_a_relation_across_two_caches_is_refused() {
    let bus = Bus::new();
    let m = mapping();
    let one = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let two = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let a1 = one.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let a2 = two.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let foreign_radar = two.home("Radar").create_object(&a2).unwrap();
    let foreign_part = two.home("Part").create_object(&a2).unwrap();
    let radar = one.home("Radar").create_object(&a1).unwrap();
    let track = one.home("Track").create_object(&a1).unwrap();
    let sensor = one.home("Sensor").create_object(&a1).unwrap();

    assert!(matches!(
        track.set_reference("a_radar", Some(&foreign_radar)),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    let parts = sensor.get_list("parts").unwrap();
    assert!(matches!(
        parts.add_object(&foreign_part),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    assert_eq!(parts.length(), 0);

    // An object of the same Cache without oid (not yet registered) is
    // refused by both kinds of relation.
    let unregistered_radar = one.home("Radar").create_unregistered_object(&a1).unwrap();
    let unregistered_part = one.home("Part").create_unregistered_object(&a1).unwrap();
    match track.set_reference("a_radar", Some(&unregistered_radar)) {
        Err(DlrlException::PreconditionNotMet { message }) => {
            assert!(message.contains("no oid"), "{message}");
        }
        other => panic!("a reference to an object without oid: {other:?}"),
    }
    match parts.add_object(&unregistered_part) {
        Err(DlrlException::PreconditionNotMet { message }) => {
            assert!(message.contains("no oid"), "{message}");
        }
        other => panic!("a list item without oid: {other:?}"),
    }
    assert_eq!(parts.length(), 0);
    one.home("Radar")
        .register_object(&unregistered_radar)
        .unwrap();
    track
        .set_reference("a_radar", Some(&unregistered_radar))
        .unwrap();
    track.set_reference("a_radar", None).unwrap();

    // Another access of the same Cache is the same Cache.
    let a1b = one.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let part = one.home("Part").create_object(&a1b).unwrap();
    parts.add_object(&part).unwrap();
    track.set_reference("a_radar", Some(&radar)).unwrap();
    assert_eq!(track.get_reference("a_radar").unwrap(), Some(radar.clone()));
    track.set_reference("a_radar", None).unwrap();
    assert_eq!(track.get_reference("a_radar").unwrap(), None);
    assert!(matches!(
        track.set_reference("x", Some(&radar)),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    delete(one);
    delete(two);
}

/// §8.1.3.1 "Object identity is given by an oid": two Caches of one
/// process, on two participants, never allocate one oid. Their oids
/// carry the host id of the GUID prefixes and the process id (marked
/// with its PID namespace) in the first two values, and the third comes
/// from one counter of the process, so that every oid of either Cache
/// differs from every other one in the third value.
#[test]
fn dlrl_8_1_3_1_two_caches_never_allocate_one_oid() {
    use std::collections::BTreeSet;
    use zerodds_dcps::participant::{host_id_bytes, namespaced_pid, pid_namespace_inode};

    let bus = Bus::new();
    let m = mapping();
    let one = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let two = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    assert_ne!(
        one.participant.instance_handle(),
        two.participant.instance_handle()
    );
    let a1 = one.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let a2 = two.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let mut oids = Vec::new();
    for _ in 0..200 {
        oids.push(one.home("Radar").create_object(&a1).unwrap().oid());
        oids.push(two.home("Radar").create_object(&a2).unwrap().oid());
    }
    let host = i32::from_be_bytes(host_id_bytes());
    let pid =
        i32::from_ne_bytes(namespaced_pid(std::process::id(), pid_namespace_inode()).to_ne_bytes());
    for oid in &oids {
        assert_eq!((oid.value[0], oid.value[1]), (host, pid), "{oid}");
        assert!(!oid.is_nil());
    }
    let thirds: BTreeSet<i32> = oids.iter().map(|o| o.value[2]).collect();
    assert_eq!(thirds.len(), oids.len(), "an oid allocated twice");
    delete(one);
    delete(two);
}
