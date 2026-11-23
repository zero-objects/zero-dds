// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.6.3.15 to §8.1.6.3.19: the collections `List`, `Set`,
//! `StrMap` and `IntMap` of an object, written in a CacheAccess and read,
//! with the elements added, removed and modified by an update round, in
//! the Cache of another participant.
//!
//! The subscribing Cache is in manual update mode: `load` applies the
//! waiting updates as one round whose states hold until the next one
//! (§8.1.6.3.14), so the changes of a round can be read after it.

#![cfg(feature = "dcps")]
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, missing_docs)]

#[path = "support/dds.rs"]
mod dds;

use dds::{Bus, coherent_pair, delete, load_until, mapping, mono, vehicles, wait_until};
use zerodds_dlrl::cache::{CacheBase, CacheUsage, Collection, ObjectScope};
use zerodds_dlrl::metamodel::DlrlException;
use zerodds_dlrl::model::Value;
use zerodds_dlrl::types::DlrlOid;

const ALL: [&str; 4] = ["Radar", "Track", "Part", "Sensor"];

fn long(v: i32) -> Value {
    Value::Long(v)
}

fn text(s: &str) -> Value {
    Value::String(s.into())
}

/// §8.1.6.3.16: `add`, `put`, `get`, `remove` of a relation list in a
/// CacheAccess; `NoSuchElement` for an index that is not there; the
/// indexes added, removed and modified by a round in the other Cache.
/// §8.1.6.3.15: `length` and `values`. A collection of an object of the
/// Cache is not writable, and a member of another basis is no List.
#[test]
fn dlrl_8_1_6_3_16_list_operations_and_the_changes_of_a_round() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    sub.cache.disable_updates().unwrap();
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let part = |serial: i32| {
        let p = publ.home("Part").create_object(&access).unwrap();
        p.set("serial", mono(long(serial))).unwrap();
        p
    };
    let (p1, p2, p3) = (part(1), part(2), part(3));
    let sensor = publ.home("Sensor").create_object(&access).unwrap();
    sensor.set("label", mono(text("s"))).unwrap();
    let list = sensor.get_list("parts").unwrap();
    assert_eq!(list.length(), 0);
    list.add_object(&p1).unwrap();
    list.add_object(&p2).unwrap();
    assert_eq!(list.get_object(1).unwrap(), p2);
    list.put_object(1, &p3).unwrap();
    assert_eq!(list.get_object(1).unwrap(), p3);
    list.put_object(2, &p2).unwrap();
    assert_eq!(list.length(), 3);
    assert!(matches!(
        list.put_object(5, &p1),
        Err(DlrlException::NoSuchElement { .. })
    ));
    assert!(matches!(
        list.get(-1),
        Err(DlrlException::NoSuchElement { .. })
    ));
    // A value that does not fit the item type of the relation.
    assert!(matches!(
        list.add(long(7)),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    assert!(list.remove().unwrap());
    assert_eq!(list.length(), 2);
    assert_eq!(
        list.values(),
        vec![
            Value::Ref(Some(p1.reference())),
            Value::Ref(Some(p3.reference()))
        ]
    );
    access.write().unwrap();

    let got = load_until(&sub, "Sensor", sensor.oid(), "the sensor", |_| true);
    let sub_list = got.get_list("parts").unwrap();
    assert_eq!(sub_list.length(), 2);
    assert_eq!(sub_list.added_elements().unwrap(), [0, 1]);
    assert!(sub_list.removed_elements().unwrap().is_empty());
    let base = CacheBase::Cache(sub.cache.clone());
    assert_eq!(
        sub_list.get_object(0).unwrap(),
        sub.home("Part").find_object(p1.oid(), &base).unwrap()
    );
    assert!(matches!(
        sub_list.add_object(&sub_list.get_object(0).unwrap()),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        got.get_set("parts"),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        got.get_list("label"),
        Err(DlrlException::PreconditionNotMet { .. })
    ));

    list.remove().unwrap();
    list.put_object(0, &p2).unwrap();
    access.write().unwrap();
    let got = load_until(&sub, "Sensor", sensor.oid(), "the shorter list", |o| {
        o.get_list("parts").unwrap().length() == 1
    });
    let sub_list = got.get_list("parts").unwrap();
    assert!(sub_list.added_elements().unwrap().is_empty());
    assert_eq!(sub_list.removed_elements().unwrap(), [1]);
    assert_eq!(sub_list.modified_elements().unwrap(), [0]);
    delete(publ);
    delete(sub);
}

/// §8.1.6.3.17: `add` of an element already there and `remove` of an
/// absent one are ignored; `contains`; the elements added and removed by
/// a round.
#[test]
fn dlrl_8_1_6_3_17_set_operations_and_the_changes_of_a_round() {
    let bus = Bus::new();
    let m = vehicles();
    let classes = ["Vehicle", "Car"];
    let (sub, publ) = coherent_pair(&bus, &m, &classes);
    sub.cache.disable_updates().unwrap();
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let v = publ.home("Vehicle").create_object(&access).unwrap();
    v.set("name", mono(text("v"))).unwrap();
    let tags = v.get_set("tags").unwrap();
    tags.add(text("a")).unwrap();
    tags.add(text("b")).unwrap();
    tags.add(text("a")).unwrap();
    assert!(!tags.remove(&text("z")).unwrap());
    assert_eq!(tags.length(), 2);
    assert!(tags.contains(&text("a")).unwrap());
    assert!(!tags.contains(&text("z")).unwrap());
    assert!(matches!(
        tags.add(long(1)),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    access.write().unwrap();

    let got = load_until(&sub, "Vehicle", v.oid(), "the vehicle", |_| true);
    let sub_tags = got.get_set("tags").unwrap();
    let mut added = sub_tags.added_elements().unwrap();
    added.sort_by_key(Value::describe);
    assert_eq!(added, [text("a"), text("b")]);

    assert!(tags.remove(&text("a")).unwrap());
    tags.add(text("c")).unwrap();
    access.write().unwrap();
    let got = load_until(&sub, "Vehicle", v.oid(), "the new tags", |o| {
        o.get_set("tags").unwrap().contains(&text("c")).unwrap()
    });
    let sub_tags = got.get_set("tags").unwrap();
    assert_eq!(sub_tags.added_elements().unwrap(), [text("c")]);
    assert_eq!(sub_tags.removed_elements().unwrap(), [text("a")]);
    assert_eq!(sub_tags.length(), 2);
    delete(publ);
    delete(sub);
}

/// §8.1.6.3.19: `put`, `get`, `remove`, `keys` of an IntMap;
/// `NoSuchElement` from `get` of a key that is not there, FALSE from its
/// `remove` (§8.2.1.1: a warning is a return value); the keys added, removed
/// and modified by a round.
#[test]
fn dlrl_8_1_6_3_19_int_map_operations_and_the_changes_of_a_round() {
    let bus = Bus::new();
    let m = vehicles();
    let classes = ["Vehicle", "Car"];
    let (sub, publ) = coherent_pair(&bus, &m, &classes);
    sub.cache.disable_updates().unwrap();
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let v = publ.home("Car").create_object(&access).unwrap();
    v.set("name", mono(text("c"))).unwrap();
    v.set("seats", mono(long(2))).unwrap();
    let readings = v.get_int_map("readings").unwrap();
    readings.put(3, Value::Double(1.5)).unwrap();
    readings.put(-1, Value::Double(2.0)).unwrap();
    assert_eq!(readings.keys(), [-1, 3]);
    assert_eq!(readings.get(3).unwrap(), Value::Double(1.5));
    assert_eq!(readings.values(), [Value::Double(2.0), Value::Double(1.5)]);
    assert!(matches!(
        readings.get(7),
        Err(DlrlException::NoSuchElement { .. })
    ));
    assert!(!readings.remove(7).unwrap(), "§8.2.1.1: a warning");
    access.write().unwrap();

    let got = load_until(&sub, "Car", v.oid(), "the car", |_| true);
    assert_eq!(
        got.get_int_map("readings")
            .unwrap()
            .added_elements()
            .unwrap(),
        [-1, 3]
    );

    readings.put(3, Value::Double(9.0)).unwrap();
    assert!(readings.remove(-1).unwrap());
    readings.put(8, Value::Double(0.5)).unwrap();
    access.write().unwrap();
    let got = load_until(&sub, "Car", v.oid(), "the new readings", |o| {
        o.get_int_map("readings").unwrap().keys() == [3, 8]
    });
    let sub_readings = got.get_int_map("readings").unwrap();
    assert_eq!(sub_readings.added_elements().unwrap(), [8]);
    assert_eq!(sub_readings.removed_elements().unwrap(), [-1]);
    assert_eq!(sub_readings.modified_elements().unwrap(), [3]);
    delete(publ);
    delete(sub);
}

/// §8.1.6.3.18: `put`, `get`, `remove`, `keys` of a StrMap;
/// `NoSuchElement` from `get` of a key that is not there, FALSE from its
/// `remove` (§8.2.1.1: a warning is a return value); the keys added, removed
/// and modified by a round.
#[test]
fn dlrl_8_1_6_3_18_str_map_operations_and_the_changes_of_a_round() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    sub.cache.disable_updates().unwrap();
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let t = publ.home("Track").create_object(&access).unwrap();
    t.set("x", mono(Value::Double(0.0))).unwrap();
    t.set("y", mono(Value::Double(0.0))).unwrap();
    t.set("a_radar", mono(Value::Ref(None))).unwrap();
    let comments = t.get_str_map("comments").unwrap();
    comments.put("k1", text("x")).unwrap();
    comments.put("k2", text("y")).unwrap();
    assert_eq!(comments.keys(), ["k1", "k2"]);
    assert_eq!(comments.get("k2").unwrap(), text("y"));
    assert!(matches!(
        comments.get("zz"),
        Err(DlrlException::NoSuchElement { .. })
    ));
    assert!(!comments.remove("zz").unwrap(), "§8.2.1.1: a warning");
    assert!(matches!(
        comments.put_object("k3", &t),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    access.write().unwrap();

    let got = load_until(&sub, "Track", t.oid(), "the track", |_| true);
    assert_eq!(
        got.get_str_map("comments")
            .unwrap()
            .added_elements()
            .unwrap(),
        ["k1", "k2"]
    );

    comments.put("k1", text("z")).unwrap();
    comments.remove("k2").unwrap();
    comments.put("k3", text("w")).unwrap();
    access.write().unwrap();
    let got = load_until(&sub, "Track", t.oid(), "the new comments", |o| {
        o.get_str_map("comments").unwrap().keys() == ["k1", "k3"]
    });
    let sub_comments = got.get_str_map("comments").unwrap();
    assert_eq!(sub_comments.added_elements().unwrap(), ["k3"]);
    assert_eq!(sub_comments.removed_elements().unwrap(), ["k2"]);
    assert_eq!(sub_comments.modified_elements().unwrap(), ["k1"]);
    assert_eq!(sub_comments.get("k1").unwrap(), text("z"));
    delete(publ);
    delete(sub);
}

/// §8.1.6.3.15 to §8.1.6.3.18, §8.1.6.3.3: for a clone in a CacheAccess
/// the elements added, removed and modified are those of the last
/// `refresh`, not of an update round of the Cache: a list and a map of
/// contracted objects after the first refresh, after a refresh that
/// follows an update, after a refresh without update, and after a
/// shrinking update.
#[test]
fn dlrl_8_1_6_3_15_the_changes_of_a_clone_are_those_of_the_last_refresh() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let part = |serial: i32| {
        let p = publ.home("Part").create_object(&access).unwrap();
        p.set("serial", mono(long(serial))).unwrap();
        p
    };
    let (p1, p2, p3) = (part(1), part(2), part(3));
    let sensor = publ.home("Sensor").create_object(&access).unwrap();
    sensor.set("label", mono(text("s"))).unwrap();
    let list = sensor.get_list("parts").unwrap();
    list.add_object(&p1).unwrap();
    list.add_object(&p2).unwrap();
    let t = publ.home("Track").create_object(&access).unwrap();
    t.set("x", mono(Value::Double(0.0))).unwrap();
    t.set("y", mono(Value::Double(0.0))).unwrap();
    t.set("a_radar", mono(Value::Ref(None))).unwrap();
    let comments = t.get_str_map("comments").unwrap();
    comments.put("k1", text("x")).unwrap();
    comments.put("k2", text("y")).unwrap();
    access.write().unwrap();

    let base = CacheBase::Cache(sub.cache.clone());
    let primary = |class: &str, oid: DlrlOid| sub.home(class).find_object(oid, &base);
    wait_until("the sensor and the track", || {
        primary("Sensor", sensor.oid()).is_ok() && primary("Track", t.oid()).is_ok()
    });
    let reading = sub.cache.create_access(CacheUsage::ReadOnly).unwrap();
    for (class, oid) in [("Sensor", sensor.oid()), ("Track", t.oid())] {
        reading
            .create_contract(&primary(class, oid).unwrap(), ObjectScope::SimpleObject, 0)
            .unwrap();
    }
    let in_access = CacheBase::Access(reading.clone());
    let clone_list = || {
        sub.home("Sensor")
            .find_object(sensor.oid(), &in_access)
            .unwrap()
            .get_list("parts")
            .unwrap()
    };
    let clone_map = || {
        sub.home("Track")
            .find_object(t.oid(), &in_access)
            .unwrap()
            .get_str_map("comments")
            .unwrap()
    };

    // First refresh: the clones are NEW, every element is added.
    reading.refresh().unwrap();
    assert_eq!(clone_list().added_elements().unwrap(), [0, 1]);
    assert!(clone_list().removed_elements().unwrap().is_empty());
    assert!(clone_list().modified_elements().unwrap().is_empty());
    assert_eq!(clone_map().added_elements().unwrap(), ["k1", "k2"]);
    assert!(clone_map().removed_elements().unwrap().is_empty());
    assert!(clone_map().modified_elements().unwrap().is_empty());

    // An update reaches the Cache; the clones change only at refresh.
    list.put_object(0, &p2).unwrap();
    list.add_object(&p3).unwrap();
    comments.put("k1", text("z")).unwrap();
    comments.remove("k2").unwrap();
    comments.put("k3", text("w")).unwrap();
    access.write().unwrap();
    wait_until("the update in the Cache", || {
        primary("Sensor", sensor.oid()).is_ok_and(|o| o.get_list("parts").unwrap().length() == 3)
            && primary("Track", t.oid())
                .is_ok_and(|o| o.get_str_map("comments").unwrap().keys() == ["k1", "k3"])
    });
    assert_eq!(clone_list().length(), 2);
    assert_eq!(clone_map().keys(), ["k1", "k2"]);
    reading.refresh().unwrap();
    assert_eq!(clone_list().length(), 3);
    assert_eq!(clone_list().added_elements().unwrap(), [2]);
    assert!(clone_list().removed_elements().unwrap().is_empty());
    assert_eq!(clone_list().modified_elements().unwrap(), [0]);
    assert_eq!(clone_map().added_elements().unwrap(), ["k3"]);
    assert_eq!(clone_map().removed_elements().unwrap(), ["k2"]);
    assert_eq!(clone_map().modified_elements().unwrap(), ["k1"]);
    assert_eq!(clone_map().get("k1").unwrap(), text("z"));

    // A refresh without an update in between changes nothing.
    reading.refresh().unwrap();
    assert!(clone_list().added_elements().unwrap().is_empty());
    assert!(clone_list().removed_elements().unwrap().is_empty());
    assert!(clone_list().modified_elements().unwrap().is_empty());
    assert!(clone_map().added_elements().unwrap().is_empty());
    assert!(clone_map().removed_elements().unwrap().is_empty());
    assert!(clone_map().modified_elements().unwrap().is_empty());

    // A shrinking update: the index beyond the new length is removed.
    list.remove().unwrap();
    access.write().unwrap();
    wait_until("the shorter list in the Cache", || {
        primary("Sensor", sensor.oid()).is_ok_and(|o| o.get_list("parts").unwrap().length() == 2)
    });
    reading.refresh().unwrap();
    assert_eq!(clone_list().removed_elements().unwrap(), [2]);
    assert!(clone_list().added_elements().unwrap().is_empty());
    assert!(clone_list().modified_elements().unwrap().is_empty());
    assert!(clone_map().added_elements().unwrap().is_empty());
    assert!(clone_map().modified_elements().unwrap().is_empty());
    delete(publ);
    delete(sub);
}
