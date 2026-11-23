// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.6.1 DLRL requested functions, between Caches on two
//! participants: publishing (§8.1.6.1.1), subscribing with its implicit
//! versus explicit subscriptions, cache management and user interaction
//! (§8.1.6.1.2), and both in one application (§8.1.6.1.3).

#![cfg(feature = "dcps")]
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, missing_docs)]

#[path = "support/dds.rs"]
mod dds;

use std::sync::Arc;
use std::time::Duration;

use dds::{
    Bus, ObjectRecorder, Recorder, dbl, delete, enabled, mapping, mono, reference, stays,
    wait_until,
};
use zerodds_dlrl::cache::{CacheBase, CacheUsage, ObjectScope, ObjectState};
use zerodds_dlrl::mapping::Mapping;
use zerodds_dlrl::metamodel::DlrlException;
use zerodds_dlrl::model::{
    AttributeValue, BasicType, ClassModel, CollectionBasis, Model, OBJECT_ROOT, ObjectRef,
    Relation, SimpleType, Value,
};

const ALL: [&str; 4] = ["Radar", "Track", "Part", "Sensor"];

fn new_track(
    side: &dds::Side,
    access: &zerodds_dlrl::cache::CacheAccess,
    x: f64,
) -> zerodds_dlrl::cache::ObjectRoot {
    let t = side.home("Track").create_object(access).unwrap();
    t.set("x", mono(Value::Double(x))).unwrap();
    t.set("y", mono(Value::Double(0.0))).unwrap();
    t.set("a_radar", mono(Value::Ref(None))).unwrap();
    t
}

/// §8.1.6.1.1: objects created, modified and destroyed in a CacheAccess
/// and written reach the Cache of another participant.
#[test]
fn dlrl_8_1_6_1_1_created_modified_and_destroyed_objects_reach_a_second_cache() {
    let bus = Bus::new();
    let m = mapping();
    let sub = enabled(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let publ = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let base = CacheBase::Cache(sub.cache.clone());
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let t = new_track(&publ, &access, 1.0);
    assert_eq!(t.write_state(), ObjectState::New);
    access.write().unwrap();
    assert_eq!(t.write_state(), ObjectState::NotModified);
    let tracks = sub.home("Track");
    wait_until("the created track", || {
        tracks.find_object(t.oid(), &base).is_ok()
    });
    let got = tracks.find_object(t.oid(), &base).unwrap();
    assert_eq!(dbl(&got, "x"), 1.0);
    // The objects of a Cache are not modifiable.
    assert!(matches!(
        got.set("x", mono(Value::Double(9.0))),
        Err(DlrlException::PreconditionNotMet { .. })
    ));

    t.set("x", mono(Value::Double(2.5))).unwrap();
    assert_eq!(t.write_state(), ObjectState::Modified);
    access.write().unwrap();
    wait_until("the modification", || dbl(&got, "x") == 2.5);

    t.destroy().unwrap();
    assert!(matches!(
        t.destroy(),
        Err(DlrlException::AlreadyDeleted { .. })
    ));
    access.write().unwrap();
    assert!(access.objects().is_empty());
    wait_until("the destruction", || {
        tracks.find_object(t.oid(), &base).is_err()
    });
    assert_eq!(got.read_state(), ObjectState::Deleted);
    delete(publ);
    delete(sub);
}

/// §8.1.6.1.1: concurrent threads of modifications, each with its own
/// CacheAccess, publish both their sets.
#[test]
fn dlrl_8_1_6_1_1_two_write_accesses_in_two_threads_publish_both_sets() {
    let bus = Bus::new();
    let m = mapping();
    let sub = enabled(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let publ = Arc::new(enabled(&bus, CacheUsage::WriteOnly, &m, &ALL));
    let threads: Vec<_> = (0..2)
        .map(|n| {
            let publ = Arc::clone(&publ);
            std::thread::spawn(move || {
                let access = publ.cache.create_access(CacheUsage::ReadWrite).unwrap();
                for i in 0..3 {
                    let r = publ.home("Radar").create_object(&access).unwrap();
                    r.set("name", mono(Value::String(format!("t{n}-{i}"))))
                        .unwrap();
                }
                access.write().unwrap();
                access.objects().len()
            })
        })
        .collect();
    for t in threads {
        assert_eq!(t.join().unwrap(), 3);
    }
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("six radars", || {
        sub.home("Radar").get_objects(&base).unwrap().len() == 6
    });
    let mut names: Vec<String> = sub
        .home("Radar")
        .get_objects(&base)
        .unwrap()
        .iter()
        .map(|r| match r.get("name").unwrap() {
            AttributeValue::Mono(Value::String(s)) => s,
            other => panic!("{other:?}"),
        })
        .collect();
    names.sort();
    assert_eq!(names, ["t0-0", "t0-1", "t0-2", "t1-0", "t1-1", "t1-2"]);
    delete(Arc::try_unwrap(publ).ok().unwrap());
    delete(sub);
}

/// §8.1.6.1.2, §8.1.6.4: incoming data become objects; the Cache
/// listener frames each round, the home listeners hear of creation,
/// modification and deletion.
#[test]
fn dlrl_8_1_6_1_2_incoming_data_become_objects_and_fire_the_listeners() {
    let bus = Bus::new();
    let m = mapping();
    let sub = enabled(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let rec = Arc::new(Recorder::default());
    sub.cache.attach_listener(rec.clone()).unwrap();
    sub.home("Track").attach_listener(
        Arc::new(ObjectRecorder {
            tag: "track",
            answer: true,
            watched: vec!["x", "y"],
            sink: rec.clone(),
        }),
        false,
    );
    let publ = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let t = new_track(&publ, &access, 1.0);
    access.write().unwrap();
    wait_until("creation heard", || {
        rec.snapshot().contains(&"end".to_string())
    });
    assert_eq!(rec.take(), ["begin", "track created Track", "end"]);
    let base = CacheBase::Cache(sub.cache.clone());
    let got = sub.home("Track").find_object(t.oid(), &base).unwrap();
    // After the round the states are clean.
    assert_eq!(got.read_state(), ObjectState::NotModified);
    assert_eq!(
        sub.home("Track").get_created_objects(&base).unwrap(),
        vec![got.clone()]
    );

    t.set("y", mono(Value::Double(4.0))).unwrap();
    access.write().unwrap();
    wait_until("modification heard", || {
        rec.snapshot().contains(&"end".to_string())
    });
    assert_eq!(rec.take(), ["begin", "track modified Track [y]", "end"]);
    assert_eq!(
        sub.home("Track").get_modified_objects(&base).unwrap(),
        vec![got.clone()]
    );

    t.destroy().unwrap();
    access.write().unwrap();
    wait_until("deletion heard", || {
        rec.snapshot().contains(&"end".to_string())
    });
    assert_eq!(rec.take(), ["begin", "track deleted Track", "end"]);
    assert_eq!(got.read_state(), ObjectState::Deleted);
    assert!(sub.home("Track").get_objects(&base).unwrap().is_empty());
    delete(publ);
    delete(sub);
}

/// §8.1.6.1.2.1: a relation to an object that is no longer in the Cache
/// (destroyed by its publisher) raises NotFound.
#[test]
fn dlrl_8_1_6_1_2_1_a_relation_to_an_object_that_is_gone_is_not_found() {
    let bus = Bus::new();
    let m = mapping();
    let sub = enabled(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let publ = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let r = publ.home("Radar").create_object(&access).unwrap();
    r.set("name", mono(Value::String("r".into()))).unwrap();
    let t = new_track(&publ, &access, 1.0);
    t.set("a_radar", reference(&r)).unwrap();
    access.write().unwrap();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("track and radar", || {
        sub.home("Radar").find_object(r.oid(), &base).is_ok()
            && sub.home("Track").find_object(t.oid(), &base).is_ok()
    });
    let got = sub.home("Track").find_object(t.oid(), &base).unwrap();
    assert_eq!(
        got.get_reference("a_radar").unwrap().unwrap().oid(),
        r.oid()
    );
    // The radar is destroyed; the track still designates it.
    r.destroy().unwrap();
    access.write().unwrap();
    wait_until("radar gone", || {
        sub.home("Radar").find_object(r.oid(), &base).is_err()
    });
    assert!(matches!(
        got.get_reference("a_radar"),
        Err(DlrlException::NotFound { .. })
    ));
    assert!(matches!(
        got.get_reference("x"),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    delete(publ);
    delete(sub);
}

/// The model of [`mapping`] with `SpecialRadar { range }`, derived from
/// `Radar`, which shares Radar's main topic.
fn mapping_with_special_radar() -> Arc<Mapping> {
    let double = || SimpleType::Basic(BasicType::Double);
    let model = Model::new(vec![
        ClassModel::new("Radar")
            .base(OBJECT_ROOT)
            .mono("name", SimpleType::Basic(BasicType::String)),
        ClassModel::new("SpecialRadar")
            .base("Radar")
            .mono("range", double()),
        ClassModel::new("Track")
            .base(OBJECT_ROOT)
            .mono("x", double())
            .mono("y", double())
            .multi(
                "comments",
                CollectionBasis::StrMap,
                SimpleType::Basic(BasicType::String),
            )
            .relation(Relation::to_one("a_radar", "Radar")),
    ])
    .unwrap();
    Arc::new(Mapping::default_for(model).unwrap())
}

/// §8.1.6.1.2.1: "navigating to an object for which no subscription
/// exists raises NotFound". The subscribing Cache has homes for Radar and
/// Track, none for SpecialRadar; a track that designates a SpecialRadar
/// arrives, and navigating its relation raises NotFound because the
/// class has no subscription, while a track that designates a Radar
/// navigates.
#[test]
fn dlrl_8_1_6_1_2_1_a_relation_to_a_class_without_subscription_is_not_found() {
    let bus = Bus::new();
    let m = mapping_with_special_radar();
    let sub = enabled(&bus, CacheUsage::ReadOnly, &m, &["Radar", "Track"]);
    let publ = enabled(
        &bus,
        CacheUsage::WriteOnly,
        &m,
        &["Radar", "SpecialRadar", "Track"],
    );
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let plain = publ.home("Radar").create_object(&access).unwrap();
    plain
        .set("name", mono(Value::String("plain".into())))
        .unwrap();
    let special = publ.home("SpecialRadar").create_object(&access).unwrap();
    special
        .set("name", mono(Value::String("special".into())))
        .unwrap();
    special.set("range", mono(Value::Double(9.0))).unwrap();
    let to_plain = new_track(&publ, &access, 1.0);
    to_plain.set("a_radar", reference(&plain)).unwrap();
    let to_special = new_track(&publ, &access, 2.0);
    to_special.set("a_radar", reference(&special)).unwrap();
    access.write().unwrap();

    let base = CacheBase::Cache(sub.cache.clone());
    let tracks = sub.home("Track");
    wait_until("both tracks and the plain radar", || {
        tracks.find_object(to_plain.oid(), &base).is_ok()
            && tracks.find_object(to_special.oid(), &base).is_ok()
            && sub.home("Radar").find_object(plain.oid(), &base).is_ok()
    });
    let got_plain = tracks.find_object(to_plain.oid(), &base).unwrap();
    assert_eq!(
        got_plain.get_reference("a_radar").unwrap().unwrap().oid(),
        plain.oid()
    );
    let got_special = tracks.find_object(to_special.oid(), &base).unwrap();
    assert_eq!(
        got_special.get("a_radar").unwrap(),
        mono(Value::Ref(Some(ObjectRef::new(
            "SpecialRadar",
            special.oid()
        ))))
    );
    match got_special.get_reference("a_radar") {
        Err(DlrlException::NotFound { message }) => {
            assert!(message.contains("no subscription"), "{message}");
        }
        other => panic!("{other:?}"),
    }
    // The special radar is not an object of the subscribing Cache.
    assert!(
        sub.home("Radar")
            .get_objects(&base)
            .unwrap()
            .iter()
            .all(|r| r.oid() != special.oid())
    );
    delete(publ);
    delete(sub);
}

/// §8.1.6.1.2.2: incoming updates are applied transparently; switched
/// off, they wait until `load` (without listener) or `enable_updates`
/// (with listeners).
#[test]
fn dlrl_8_1_6_1_2_2_updates_apply_transparently_until_disabled() {
    let bus = Bus::new();
    let m = mapping();
    let sub = enabled(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let rec = Arc::new(Recorder::default());
    sub.cache.attach_listener(rec.clone()).unwrap();
    let publ = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let t = new_track(&publ, &access, 1.0);
    access.write().unwrap();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("track", || {
        sub.home("Track").find_object(t.oid(), &base).is_ok()
    });
    let got = sub.home("Track").find_object(t.oid(), &base).unwrap();
    t.set("x", mono(Value::Double(2.0))).unwrap();
    access.write().unwrap();
    wait_until("transparent update", || dbl(&got, "x") == 2.0);

    sub.cache.disable_updates().unwrap();
    assert!(!sub.cache.updates_enabled());
    rec.take();
    t.set("x", mono(Value::Double(3.0))).unwrap();
    access.write().unwrap();
    stays("held while disabled", Duration::from_millis(800), || {
        dbl(&got, "x") == 2.0
    });
    sub.cache.load().unwrap();
    assert_eq!(dbl(&got, "x"), 3.0);
    // load triggers no listener; the state of the manual round holds.
    assert!(rec.snapshot().is_empty());
    assert_eq!(got.read_state(), ObjectState::Modified);
    assert!(got.is_attribute_modified("x").unwrap());
    assert!(!got.is_attribute_modified("y").unwrap());

    t.set("x", mono(Value::Double(4.0))).unwrap();
    access.write().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(dbl(&got, "x"), 3.0);
    sub.cache.enable_updates().unwrap();
    wait_until("applied on enable", || dbl(&got, "x") == 4.0);
    wait_until("round end", || rec.snapshot().contains(&"end".to_string()));
    assert_eq!(rec.take(), ["enabled", "begin", "end"]);
    delete(publ);
    delete(sub);
}

/// §8.1.6.1.2.3: several incoming updates are reported as a whole (one
/// round), and a modification names the modified attributes.
#[test]
fn dlrl_8_1_6_1_2_3_one_round_reports_several_updates_and_the_modified_attributes() {
    let bus = Bus::new();
    let m = mapping();
    let sub = enabled(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let rec = Arc::new(Recorder::default());
    let publ = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let first = new_track(&publ, &access, 1.0);
    access.write().unwrap();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("first", || {
        sub.home("Track").find_object(first.oid(), &base).is_ok()
    });

    sub.cache.attach_listener(rec.clone()).unwrap();
    sub.home("Track").attach_listener(
        Arc::new(ObjectRecorder {
            tag: "track",
            answer: true,
            watched: vec!["x", "y", "comments"],
            sink: rec.clone(),
        }),
        false,
    );
    sub.cache.disable_updates().unwrap();
    first.set("x", mono(Value::Double(5.0))).unwrap();
    let second = new_track(&publ, &access, 2.0);
    let third = new_track(&publ, &access, 3.0);
    access.write().unwrap();
    // All three are written before the Cache takes them as one round.
    std::thread::sleep(Duration::from_millis(800));
    sub.cache.enable_updates().unwrap();
    wait_until("round", || rec.snapshot().contains(&"end".to_string()));
    let mut events = rec.take();
    assert_eq!(events.first().map(String::as_str), Some("disabled"));
    assert_eq!(events.get(1).map(String::as_str), Some("enabled"));
    assert_eq!(events.get(2).map(String::as_str), Some("begin"));
    assert_eq!(events.last().map(String::as_str), Some("end"));
    let middle: Vec<String> = events.drain(3..events.len() - 1).collect();
    assert_eq!(
        middle,
        [
            "track created Track",
            "track created Track",
            "track modified Track [x]"
        ]
    );
    assert_eq!(
        sub.home("Track").get_created_objects(&base).unwrap().len(),
        2
    );
    for o in [&second, &third] {
        assert!(sub.home("Track").find_object(o.oid(), &base).is_ok());
    }
    delete(publ);
    delete(sub);
}

/// §8.1.6.1.3: an application that publishes and subscribes holds the
/// incoming updates (disable_updates) while it writes, and takes them
/// when it chooses.
#[test]
fn dlrl_8_1_6_1_3_disable_updates_holds_incoming_updates_while_the_application_writes() {
    let bus = Bus::new();
    let m = mapping();
    let both = enabled(&bus, CacheUsage::ReadWrite, &m, &ALL);
    let other = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let base = CacheBase::Cache(both.cache.clone());
    let foreign_access = other.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let foreign = new_track(&other, &foreign_access, 1.0);
    foreign_access.write().unwrap();
    wait_until("foreign track", || {
        both.home("Track").find_object(foreign.oid(), &base).is_ok()
    });
    let seen = both
        .home("Track")
        .find_object(foreign.oid(), &base)
        .unwrap();

    both.cache.disable_updates().unwrap();
    let own_access = both.cache.create_access(CacheUsage::ReadWrite).unwrap();
    let own = new_track(&both, &own_access, 10.0);
    foreign.set("x", mono(Value::Double(2.0))).unwrap();
    foreign_access.write().unwrap();
    own_access.write().unwrap();
    stays("both held", Duration::from_millis(800), || {
        dbl(&seen, "x") == 1.0 && both.home("Track").find_object(own.oid(), &base).is_err()
    });
    both.cache.load().unwrap();
    assert_eq!(dbl(&seen, "x"), 2.0);
    let mine = both.home("Track").find_object(own.oid(), &base).unwrap();
    assert_eq!(dbl(&mine, "x"), 10.0);
    assert_eq!(mine.read_state(), ObjectState::New);
    delete(other);
    delete(both);
}

/// §8.1.6.1.3, §8.1.6.5.2: a write access holds its clones apart from
/// incoming updates until it is refreshed; refresh rolls back what was
/// not written.
#[test]
fn dlrl_8_1_6_1_3_a_write_access_is_not_changed_by_incoming_updates_until_refresh() {
    let bus = Bus::new();
    let m = mapping();
    let both = enabled(&bus, CacheUsage::ReadWrite, &m, &ALL);
    let other = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let base = CacheBase::Cache(both.cache.clone());
    let foreign_access = other.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let foreign = new_track(&other, &foreign_access, 1.0);
    foreign_access.write().unwrap();
    wait_until("foreign track", || {
        both.home("Track").find_object(foreign.oid(), &base).is_ok()
    });
    let primary = both
        .home("Track")
        .find_object(foreign.oid(), &base)
        .unwrap();

    let access = both.cache.create_access(CacheUsage::ReadWrite).unwrap();
    access
        .create_contract(&primary, ObjectScope::SimpleObject, 0)
        .unwrap();
    access.refresh().unwrap();
    let clone = access.objects().pop().unwrap();
    assert_eq!(clone.read_state(), ObjectState::New);
    assert_eq!(clone.write_state(), ObjectState::NotModified);
    assert_ne!(clone, primary);
    clone.set("y", mono(Value::Double(9.0))).unwrap();
    assert_eq!(clone.write_state(), ObjectState::Modified);

    foreign.set("x", mono(Value::Double(5.0))).unwrap();
    foreign_access.write().unwrap();
    wait_until("primary updated", || dbl(&primary, "x") == 5.0);
    assert_eq!(dbl(&clone, "x"), 1.0);
    assert_eq!(dbl(&clone, "y"), 9.0);

    access.refresh().unwrap();
    assert_eq!(dbl(&clone, "x"), 5.0);
    assert_eq!(dbl(&clone, "y"), 0.0);
    assert_eq!(clone.read_state(), ObjectState::Modified);
    assert!(clone.is_attribute_modified("x").unwrap());
    assert!(clone.is_modified(ObjectScope::SimpleObject).unwrap());
    assert_eq!(clone.write_state(), ObjectState::NotModified);
    // A contract on an object of no Cache is refused; purge empties.
    assert!(matches!(
        access.create_contract(&clone, ObjectScope::SimpleObject, 0),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    access.purge().unwrap();
    assert!(access.objects().is_empty() && access.contracts().is_empty());
    both.cache.delete_access(&access).unwrap();
    assert!(matches!(
        both.cache.delete_access(&access),
        Err(DlrlException::AlreadyDeleted { .. })
    ));
    delete(other);
    delete(both);
}

/// The keys of the StrMap `comments` of an object.
fn comment_keys(o: &zerodds_dlrl::cache::ObjectRoot) -> Vec<String> {
    match o.get("comments").unwrap() {
        AttributeValue::StrMap(m) => m.keys().cloned().collect(),
        other => panic!("comments: {other:?}"),
    }
}

/// §8.1.6.1.1, §8.1.6.1.3: an application that publishes and subscribes
/// destroys objects another participant created: their clones in a
/// CacheAccess, destroyed and written, dispose the rows that participant
/// wrote, and a third Cache sees the objects deleted. A Sensor's Parts,
/// contained through a composition and cloned only by the destruction
/// of the Sensor, go with it.
#[test]
fn dlrl_8_1_6_1_3_destroying_a_received_object_reaches_a_third_cache() {
    let bus = Bus::new();
    let m = mapping();
    let both = enabled(&bus, CacheUsage::ReadWrite, &m, &ALL);
    let other = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let observer = enabled(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let in_both = CacheBase::Cache(both.cache.clone());
    let in_observer = CacheBase::Cache(observer.cache.clone());

    let foreign_access = other.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let track = new_track(&other, &foreign_access, 1.0);
    let sensor = other.home("Sensor").create_object(&foreign_access).unwrap();
    sensor
        .set("label", mono(Value::String("s".into())))
        .unwrap();
    let mut objects = vec![("Track", track.oid()), ("Sensor", sensor.oid())];
    for serial in 0..2 {
        let part = other.home("Part").create_object(&foreign_access).unwrap();
        part.set("serial", mono(Value::Long(serial))).unwrap();
        sensor.get_list("parts").unwrap().add_object(&part).unwrap();
        objects.push(("Part", part.oid()));
    }
    foreign_access.write().unwrap();
    for (class, oid) in &objects {
        wait_until("the objects in both Caches that read", || {
            both.home(class).find_object(*oid, &in_both).is_ok()
                && observer.home(class).find_object(*oid, &in_observer).is_ok()
        });
    }

    let access = both.cache.create_access(CacheUsage::ReadWrite).unwrap();
    for (class, oid) in [("Track", track.oid()), ("Sensor", sensor.oid())] {
        let primary = both.home(class).find_object(oid, &in_both).unwrap();
        access
            .create_contract(&primary, ObjectScope::SimpleObject, 0)
            .unwrap();
    }
    access.refresh().unwrap();
    for clone in access.objects() {
        clone.destroy().unwrap();
    }
    assert_eq!(
        access.objects().len(),
        4,
        "the destruction of the Sensor cloned its Parts"
    );
    access.write().unwrap();
    assert!(access.objects().is_empty());
    for (class, oid) in &objects {
        wait_until("the destruction in the third Cache", || {
            observer
                .home(class)
                .find_object(*oid, &in_observer)
                .is_err()
        });
        wait_until("the destruction in the writing Cache", || {
            both.home(class).find_object(*oid, &in_both).is_err()
        });
    }
    delete(observer);
    delete(other);
    delete(both);
}

/// §8.1.6.1.1, §8.1.6.1.3: an application that publishes and subscribes
/// removes an item of a collection of an object another participant
/// created and changes an attribute; written, the row of the item is
/// disposed beside the changed row, and a third Cache has the object
/// without the item.
#[test]
fn dlrl_8_1_6_1_3_removing_an_item_of_a_received_object_reaches_a_third_cache() {
    let bus = Bus::new();
    let m = mapping();
    let both = enabled(&bus, CacheUsage::ReadWrite, &m, &ALL);
    let other = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let observer = enabled(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let in_both = CacheBase::Cache(both.cache.clone());
    let in_observer = CacheBase::Cache(observer.cache.clone());

    let foreign_access = other.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let track = new_track(&other, &foreign_access, 1.0);
    let comments = track.get_str_map("comments").unwrap();
    comments.put("a", Value::String("first".into())).unwrap();
    comments.put("b", Value::String("second".into())).unwrap();
    foreign_access.write().unwrap();
    let tracks = observer.home("Track");
    wait_until("the track with two comments", || {
        tracks
            .find_object(track.oid(), &in_observer)
            .is_ok_and(|t| comment_keys(&t) == ["a", "b"])
            && both
                .home("Track")
                .find_object(track.oid(), &in_both)
                .is_ok_and(|t| comment_keys(&t) == ["a", "b"])
    });

    let primary = both
        .home("Track")
        .find_object(track.oid(), &in_both)
        .unwrap();
    let access = both.cache.create_access(CacheUsage::ReadWrite).unwrap();
    access
        .create_contract(&primary, ObjectScope::SimpleObject, 0)
        .unwrap();
    access.refresh().unwrap();
    let clone = access.objects().pop().unwrap();
    assert!(clone.get_str_map("comments").unwrap().remove("a").unwrap());
    clone.set("x", mono(Value::Double(2.0))).unwrap();
    access.write().unwrap();

    let seen = tracks.find_object(track.oid(), &in_observer).unwrap();
    wait_until("x = 2 and the comment 'a' gone in the third Cache", || {
        dbl(&seen, "x") == 2.0 && comment_keys(&seen) == ["b"]
    });
    wait_until("the comment 'a' gone in the writing Cache", || {
        comment_keys(&primary) == ["b"]
    });
    delete(observer);
    delete(other);
    delete(both);
}
