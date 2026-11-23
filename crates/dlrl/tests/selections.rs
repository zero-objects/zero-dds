// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.6.3.8 to §8.1.6.3.13 and §8.1.6.4: ObjectListener,
//! Selection, SelectionCriterion, FilterCriterion, QueryCriterion,
//! SelectionListener, and the order in which the listeners of an update
//! round are called.

#![cfg(feature = "dcps")]
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, missing_docs)]

#[path = "support/dds.rs"]
mod dds;

use std::sync::{Arc, Mutex};

use dds::{
    Bus, ObjectRecorder, Recorder, SelectionRecorder, coherent_pair, dbl, delete, mapping, mono,
    vehicles, wait_until,
};
use zerodds_dlrl::cache::{
    CacheAccess, CacheBase, CacheDescription, CacheFactory, CacheUsage, FilterCriterion,
    ObjectHome, ObjectRoot, QueryCriterion, SelectionCriterion, SelectionListener,
};
use zerodds_dlrl::metamodel::DlrlException;
use zerodds_dlrl::model::Value;
use zerodds_dlrl::types::{CriterionKind, MembershipState};

const ALL: [&str; 4] = ["Radar", "Track", "Part", "Sensor"];

fn new_track(side: &dds::Side, access: &CacheAccess, x: f64) -> ObjectRoot {
    let t = side.home("Track").create_object(access).unwrap();
    t.set("x", mono(Value::Double(x))).unwrap();
    t.set("y", mono(Value::Double(0.0))).unwrap();
    t.set("a_radar", mono(Value::Ref(None))).unwrap();
    t
}

/// Waits for the end of the next update round and returns its events.
fn round(rec: &Recorder) -> Vec<String> {
    wait_until("the end of the round", || {
        rec.snapshot().contains(&"end".to_string())
    });
    rec.take()
}

/// The events of `round` between `begin` and `end`, sorted: the round
/// must start with `begin` and finish with `end`, so a listener call
/// outside the round fails here.
fn inside_round(events: &[String]) -> Vec<String> {
    assert!(events.len() >= 2, "{events:?}");
    assert_eq!(events[0], "begin", "{events:?}");
    assert_eq!(events[events.len() - 1], "end", "{events:?}");
    let mut inner = events[1..events.len() - 1].to_vec();
    inner.sort();
    inner
}

/// A filter that passes the tracks with `x` above `limit` and notes the
/// membership states it is asked with.
struct XAbove {
    limit: f64,
    asked: Mutex<Vec<(String, MembershipState)>>,
}

impl FilterCriterion for XAbove {
    fn check_object(&self, o: &ObjectRoot, membership_state: MembershipState) -> bool {
        self.asked
            .lock()
            .unwrap()
            .push((o.oid().to_string(), membership_state));
        dbl(o, "x") > self.limit
    }
}

/// The provided behaviour of a FilterCriterion: every object passes.
struct AllPass;

impl FilterCriterion for AllPass {}

/// §8.1.6.4.2 to §8.1.6.4.4, §8.1.6.3.9: with `auto_refresh` the
/// Selection follows each round. A creation calls the ObjectListeners,
/// then the Selections; a modification and a deletion the Selections,
/// then the ObjectListeners. An object enters, a member that stays and
/// changed is modified, a member that no longer passes or is deleted
/// leaves.
#[test]
fn dlrl_8_1_6_4_selections_and_object_listeners_in_the_order_of_creation_modification_deletion() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let rec = Arc::new(Recorder::default());
    sub.cache.attach_listener(rec.clone()).unwrap();
    let tracks = sub.home("Track");
    tracks.attach_listener(
        Arc::new(ObjectRecorder {
            tag: "track",
            answer: true,
            watched: vec!["x", "y"],
            sink: rec.clone(),
        }),
        false,
    );
    let q = QueryCriterion::new(tracks, "x > 10.0", &[]).unwrap();
    let sel = tracks
        .create_selection(SelectionCriterion::Query(q), true, false)
        .unwrap();
    assert!(sel.members().is_empty());
    sel.set_listener(Some(Arc::new(SelectionRecorder {
        tag: "sel",
        sink: rec.clone(),
    })))
    .unwrap();

    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let low = new_track(&publ, &access, 5.0);
    access.write().unwrap();
    assert_eq!(round(&rec), ["begin", "track created Track", "end"]);
    assert!(sel.members().is_empty());

    let t = new_track(&publ, &access, 20.0);
    access.write().unwrap();
    let oid = t.oid();
    assert_eq!(
        round(&rec),
        [
            "begin".to_string(),
            "track created Track".into(),
            format!("sel in {oid}"),
            "end".into()
        ]
    );
    let base = CacheBase::Cache(sub.cache.clone());
    let got = tracks.find_object(oid, &base).unwrap();
    assert_eq!(sel.members(), vec![got.clone()]);

    t.set("y", mono(Value::Double(3.0))).unwrap();
    access.write().unwrap();
    assert_eq!(
        round(&rec),
        [
            "begin".to_string(),
            format!("sel modified {oid}"),
            "track modified Track [y]".into(),
            "end".into()
        ]
    );

    t.set("x", mono(Value::Double(1.0))).unwrap();
    access.write().unwrap();
    assert_eq!(
        round(&rec),
        [
            "begin".to_string(),
            format!("sel out {oid}"),
            "track modified Track [x]".into(),
            "end".into()
        ]
    );
    assert!(sel.members().is_empty());

    low.set("x", mono(Value::Double(40.0))).unwrap();
    access.write().unwrap();
    let low_oid = low.oid();
    assert_eq!(
        round(&rec),
        [
            "begin".to_string(),
            format!("sel in {low_oid}"),
            "track modified Track [x]".into(),
            "end".into()
        ]
    );

    low.destroy().unwrap();
    access.write().unwrap();
    assert_eq!(
        round(&rec),
        [
            "begin".to_string(),
            format!("sel out {low_oid}"),
            "track deleted Track".into(),
            "end".into()
        ]
    );
    assert!(sel.members().is_empty());
    delete(publ);
    delete(sub);
}

/// §8.1.6.3.9: with `auto_refresh` FALSE the members and the listener
/// change only at `refresh`, against the state of the last examination;
/// §8.1.6.3.11: `check_object` is asked with the membership state of
/// that examination; `set_listener` returns the previous listener.
#[test]
fn dlrl_8_1_6_3_9_a_selection_without_auto_refresh_changes_only_on_refresh() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let tracks = sub.home("Track");
    let filter = Arc::new(XAbove {
        limit: 10.0,
        asked: Mutex::new(Vec::new()),
    });
    let sel = tracks
        .create_selection(SelectionCriterion::Filter(filter.clone()), false, false)
        .unwrap();
    assert!(!sel.auto_refresh());
    assert!(!sel.concerns_contained());
    assert_eq!(sel.criterion().kind(), CriterionKind::Filter);
    let rec = Arc::new(Recorder::default());
    let first: Arc<dyn SelectionListener> = Arc::new(SelectionRecorder {
        tag: "first",
        sink: rec.clone(),
    });
    assert!(sel.set_listener(Some(first.clone())).unwrap().is_none());
    let second: Arc<dyn SelectionListener> = Arc::new(SelectionRecorder {
        tag: "sel",
        sink: rec.clone(),
    });
    let previous = sel.set_listener(Some(second.clone())).unwrap().unwrap();
    assert!(Arc::ptr_eq(&previous, &first));

    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let a = new_track(&publ, &access, 5.0);
    let b = new_track(&publ, &access, 20.0);
    access.write().unwrap();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("both tracks", || {
        tracks.get_objects(&base).unwrap().len() == 2
    });
    assert!(sel.members().is_empty());
    assert!(rec.take().is_empty());
    filter.asked.lock().unwrap().clear();

    sel.refresh().unwrap();
    assert_eq!(rec.take(), [format!("sel in {}", b.oid())]);
    let mut asked = std::mem::take(&mut *filter.asked.lock().unwrap());
    asked.sort();
    let mut expected = vec![
        (a.oid().to_string(), MembershipState::UndefinedMembership),
        (b.oid().to_string(), MembershipState::UndefinedMembership),
    ];
    expected.sort();
    assert_eq!(asked, expected);

    a.set("x", mono(Value::Double(30.0))).unwrap();
    b.set("y", mono(Value::Double(1.0))).unwrap();
    access.write().unwrap();
    wait_until("both modifications", || {
        let a2 = tracks.find_object(a.oid(), &base).unwrap();
        let b2 = tracks.find_object(b.oid(), &base).unwrap();
        dbl(&a2, "x") == 30.0 && dbl(&b2, "y") == 1.0
    });
    assert!(rec.take().is_empty());
    assert_eq!(sel.members().len(), 1);
    filter.asked.lock().unwrap().clear();
    sel.refresh().unwrap();
    let mut events = rec.take();
    events.sort();
    let mut expected = vec![
        format!("sel in {}", a.oid()),
        format!("sel modified {}", b.oid()),
    ];
    expected.sort();
    assert_eq!(events, expected);
    let mut asked = std::mem::take(&mut *filter.asked.lock().unwrap());
    asked.sort();
    let mut expected = vec![
        (a.oid().to_string(), MembershipState::NotMember),
        (b.oid().to_string(), MembershipState::AlreadyMember),
    ];
    expected.sort();
    assert_eq!(asked, expected);

    // Nothing changed since the last examination: no event.
    sel.refresh().unwrap();
    assert!(rec.take().is_empty());

    b.destroy().unwrap();
    access.write().unwrap();
    wait_until("the deletion", || {
        tracks.get_objects(&base).unwrap().len() == 1
    });
    assert_eq!(sel.members().len(), 2);
    sel.refresh().unwrap();
    assert_eq!(rec.take(), [format!("sel out {}", b.oid())]);
    assert_eq!(sel.members().len(), 1);

    let discarded = sel.set_listener(None).unwrap().unwrap();
    assert!(Arc::ptr_eq(&discarded, &second));
    assert!(sel.listener().is_none());
    delete(publ);
    delete(sub);
}

/// §8.1.6.3.9: with `concerns_contained` TRUE a member is modified when
/// an object it contains (composition) is modified; with FALSE only when
/// its own content is. §8.1.6.3.11: the provided `check_object` passes
/// every object.
#[test]
fn dlrl_8_1_6_3_9_concerns_contained_reports_a_modified_constituent() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let rec = Arc::new(Recorder::default());
    sub.cache.attach_listener(rec.clone()).unwrap();
    let sensors = sub.home("Sensor");
    let deep = sensors
        .create_selection(SelectionCriterion::Filter(Arc::new(AllPass)), true, true)
        .unwrap();
    let flat = sensors
        .create_selection(SelectionCriterion::Filter(Arc::new(AllPass)), true, false)
        .unwrap();
    assert!(deep.concerns_contained());
    deep.set_listener(Some(Arc::new(SelectionRecorder {
        tag: "deep",
        sink: rec.clone(),
    })))
    .unwrap();
    flat.set_listener(Some(Arc::new(SelectionRecorder {
        tag: "flat",
        sink: rec.clone(),
    })))
    .unwrap();

    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let part = publ.home("Part").create_object(&access).unwrap();
    part.set("serial", mono(Value::Long(1))).unwrap();
    let sensor = publ.home("Sensor").create_object(&access).unwrap();
    sensor
        .set("label", mono(Value::String("s".into())))
        .unwrap();
    sensor.get_list("parts").unwrap().add_object(&part).unwrap();
    access.write().unwrap();
    let oid = sensor.oid();
    assert_eq!(
        inside_round(&round(&rec)),
        [format!("deep in {oid}"), format!("flat in {oid}")]
    );

    part.set("serial", mono(Value::Long(2))).unwrap();
    access.write().unwrap();
    assert_eq!(
        round(&rec),
        [
            "begin".to_string(),
            format!("deep modified {oid}"),
            "end".into()
        ]
    );

    sensor
        .set("label", mono(Value::String("t".into())))
        .unwrap();
    access.write().unwrap();
    assert_eq!(
        inside_round(&round(&rec)),
        [
            format!("deep modified {oid}"),
            format!("flat modified {oid}")
        ]
    );
    delete(publ);
    delete(sub);
}

/// §8.1.6.3.12: `set_parameters` and `set_query` change the query and
/// refresh the owning Selection when its `auto_refresh` is TRUE; an
/// invalid expression or parameter list raises SQLError and changes
/// nothing.
#[test]
fn dlrl_8_1_6_3_12_set_parameters_and_set_query_refresh_an_auto_refresh_selection() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let rec = Arc::new(Recorder::default());
    sub.cache.attach_listener(rec.clone()).unwrap();
    let tracks = sub.home("Track");
    let q = QueryCriterion::new(tracks, "x > %0", &["10.0".to_string()]).unwrap();
    assert_eq!(q.kind(), CriterionKind::Query);
    let sel = tracks
        .create_selection(SelectionCriterion::Query(q.clone()), true, false)
        .unwrap();
    assert_eq!(q.selection(), Some(sel.clone()));
    sel.set_listener(Some(Arc::new(SelectionRecorder {
        tag: "sel",
        sink: rec.clone(),
    })))
    .unwrap();
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let a = new_track(&publ, &access, 5.0);
    let b = new_track(&publ, &access, 20.0);
    access.write().unwrap();
    let mut events = round(&rec);
    events.sort();
    assert_eq!(
        events,
        [
            "begin".to_string(),
            "end".into(),
            format!("sel in {}", b.oid())
        ]
    );

    assert!(q.set_parameters(&["1.0".to_string()]).unwrap());
    assert_eq!(q.parameters(), ["1.0"]);
    assert_eq!(rec.take(), [format!("sel in {}", a.oid())]);
    assert_eq!(sel.members().len(), 2);

    assert!(q.set_query("x > %0", &["7.5".to_string()]).unwrap());
    assert_eq!(rec.take(), [format!("sel out {}", a.oid())]);
    assert_eq!(q.expression(), "x > %0");

    assert!(matches!(
        q.set_query("x >", &[]),
        Err(DlrlException::SqlError { .. })
    ));
    assert!(matches!(
        q.set_parameters(&[]),
        Err(DlrlException::SqlError { .. })
    ));
    assert!(matches!(
        q.set_query("depth > 1.0", &[]),
        Err(DlrlException::SqlError { .. })
    ));
    assert_eq!(q.expression(), "x > %0");
    assert_eq!(q.parameters(), ["7.5"]);
    assert!(rec.take().is_empty());
    assert_eq!(sel.members().len(), 1);
    delete(publ);
    delete(sub);
}

/// §8.1.6.3.7: `create_selection` raises PreconditionNotMet while the
/// DCPS state is INITIAL and BadParameter (§8.2.1.2.2) for a query of
/// another class or one that belongs to another Selection;
/// `delete_selection` raises PreconditionNotMet for a Selection the
/// home did not create, and deletes the Selection with its criterion:
/// the deleted Selection and QueryCriterion raise AlreadyDeleted
/// (§8.1.6.2, §8.2.1.1), and `create_selection` refuses the criterion.
#[test]
fn dlrl_8_1_6_3_7_create_and_delete_selection_check_their_preconditions() {
    let bus = Bus::new();
    let m = mapping();
    let participant = bus.participant();
    let cache = CacheFactory::get_instance()
        .create_cache(
            CacheUsage::ReadOnly,
            CacheDescription {
                name: format!("selections-{}-{}", std::process::id(), bus.domain),
                domain: participant.clone(),
            },
        )
        .unwrap();
    let homes: Vec<ObjectHome> = ALL
        .iter()
        .map(|c| {
            let h = ObjectHome::new(Arc::clone(&m), c).unwrap();
            cache.register_home(&h).unwrap();
            h
        })
        .collect();
    let tracks = &homes[1];
    let radars = &homes[0];
    assert!(matches!(
        tracks.create_selection(SelectionCriterion::Filter(Arc::new(AllPass)), true, false),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    cache.register_all_for_pubsub().unwrap();

    let radar_query = QueryCriterion::new(radars, "name = 'r'", &[]).unwrap();
    assert!(matches!(
        tracks.create_selection(SelectionCriterion::Query(radar_query), true, false),
        Err(DlrlException::BadParameter { .. })
    ));
    let q = QueryCriterion::new(tracks, "x > 1.0", &[]).unwrap();
    let sel = tracks
        .create_selection(SelectionCriterion::Query(q.clone()), false, false)
        .unwrap();
    assert!(matches!(
        tracks.create_selection(SelectionCriterion::Query(q.clone()), false, false),
        Err(DlrlException::BadParameter { .. })
    ));
    assert_eq!(tracks.selections(), vec![sel.clone()]);
    assert_eq!(sel.home().as_ref(), Some(tracks));

    assert!(matches!(
        radars.delete_selection(&sel),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    tracks.delete_selection(&sel).unwrap();
    assert!(tracks.selections().is_empty());
    assert!(sel.home().is_none());
    assert!(q.selection().is_none());
    // An operation on the deleted Selection raises AlreadyDeleted
    // (§8.1.6.2, §8.2.1.1).
    assert!(sel.is_deleted());
    assert!(matches!(
        sel.refresh(),
        Err(DlrlException::AlreadyDeleted { .. })
    ));
    assert!(matches!(
        tracks.delete_selection(&sel),
        Err(DlrlException::AlreadyDeleted { .. })
    ));
    // delete_selection deleted the criterion with the Selection: it is
    // not changed any more and cannot be attached again.
    assert!(q.is_deleted());
    assert!(matches!(
        q.set_parameters(&[]),
        Err(DlrlException::AlreadyDeleted { .. })
    ));
    assert!(matches!(
        q.set_query("x > 2.0", &[]),
        Err(DlrlException::AlreadyDeleted { .. })
    ));
    assert_eq!(q.expression(), "x > 1.0");
    match tracks.create_selection(SelectionCriterion::Query(q.clone()), false, false) {
        Err(DlrlException::BadParameter { message }) => {
            assert!(message.contains("deleted"), "{message}");
        }
        other => panic!("a deleted QueryCriterion: {other:?}"),
    }
    assert!(tracks.selections().is_empty());
    assert!(q.selection().is_none());
    // A criterion that never belonged to a Selection is not deleted.
    let fresh = QueryCriterion::new(tracks, "x > 1.0", &[]).unwrap();
    assert!(!fresh.is_deleted());
    let again = tracks
        .create_selection(SelectionCriterion::Query(fresh.clone()), false, false)
        .unwrap();
    assert_eq!(fresh.selection(), Some(again));
    CacheFactory::get_instance().delete_cache(&cache).unwrap();
}

/// §8.1.6.3.8, §8.1.6.4.2, §8.1.6.4.3: the ObjectListeners of the home of
/// the object's class are called first; TRUE from all of them stops the
/// propagation to the home of the parent class, one FALSE lets it go on.
/// A Selection of the parent's home is concerned with the derived
/// objects and is examined after the ObjectListeners of a creation.
#[test]
fn dlrl_8_1_6_3_8_object_listener_true_stops_parent_listeners() {
    let bus = Bus::new();
    let m = vehicles();
    let classes = ["Vehicle", "Car"];
    let (sub, publ) = coherent_pair(&bus, &m, &classes);
    let rec = Arc::new(Recorder::default());
    sub.cache.attach_listener(rec.clone()).unwrap();
    let listener = |tag: &'static str, answer: bool| {
        Arc::new(ObjectRecorder {
            tag,
            answer,
            watched: vec!["name"],
            sink: rec.clone(),
        })
    };
    let car1 = listener("car1", true);
    let car2 = listener("car2", true);
    sub.home("Car").attach_listener(car1.clone(), false);
    sub.home("Car").attach_listener(car2.clone(), false);
    sub.home("Vehicle")
        .attach_listener(listener("vehicle", true), false);
    let sel = sub
        .home("Vehicle")
        .create_selection(SelectionCriterion::Filter(Arc::new(AllPass)), true, false)
        .unwrap();
    sel.set_listener(Some(Arc::new(SelectionRecorder {
        tag: "vehicles",
        sink: rec.clone(),
    })))
    .unwrap();

    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let car = publ.home("Car").create_object(&access).unwrap();
    car.set("name", mono(Value::String("a".into()))).unwrap();
    car.set("seats", mono(Value::Long(4))).unwrap();
    access.write().unwrap();
    let events = round(&rec);
    assert_eq!(events.len(), 5, "{events:?}");
    let mut level = events[1..3].to_vec();
    level.sort();
    assert_eq!(level, ["car1 created Car", "car2 created Car"]);
    assert_eq!(events[3], format!("vehicles in {}", car.oid()));

    let car2: Arc<dyn zerodds_dlrl::cache::ObjectListener> = car2;
    sub.home("Car").detach_listener(&car2);
    sub.home("Car")
        .attach_listener(listener("car2", false), false);
    car.set("name", mono(Value::String("b".into()))).unwrap();
    access.write().unwrap();
    let events = round(&rec);
    assert_eq!(events.len(), 6, "{events:?}");
    assert_eq!(events[1], format!("vehicles modified {}", car.oid()));
    let mut level = events[2..4].to_vec();
    level.sort();
    assert_eq!(
        level,
        ["car1 modified Car [name]", "car2 modified Car [name]"]
    );
    assert_eq!(events[4], "vehicle modified Car [name]");
    delete(publ);
    delete(sub);
}

/// §8.1.6.3.4, §8.1.6.3.9: in manual update mode `load` applies the
/// updates without listener; a Selection with `auto_refresh` follows
/// the round, and its listener is not called.
#[test]
fn dlrl_8_1_6_3_9_load_updates_an_auto_refresh_selection_without_listener() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let tracks = sub.home("Track");
    let q = QueryCriterion::new(tracks, "x > 10.0", &[]).unwrap();
    let sel = tracks
        .create_selection(SelectionCriterion::Query(q), true, false)
        .unwrap();
    let rec = Arc::new(Recorder::default());
    sel.set_listener(Some(Arc::new(SelectionRecorder {
        tag: "sel",
        sink: rec.clone(),
    })))
    .unwrap();
    sub.cache.disable_updates().unwrap();
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let t = new_track(&publ, &access, 20.0);
    access.write().unwrap();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("the loaded track", || {
        sub.cache.load().unwrap();
        tracks.find_object(t.oid(), &base).is_ok()
    });
    assert_eq!(
        sel.members(),
        vec![tracks.find_object(t.oid(), &base).unwrap()]
    );
    t.set("x", mono(Value::Double(2.0))).unwrap();
    access.write().unwrap();
    wait_until("the member gone", || {
        sub.cache.load().unwrap();
        sel.members().is_empty()
    });
    assert!(rec.take().is_empty());
    delete(publ);
    delete(sub);
}
