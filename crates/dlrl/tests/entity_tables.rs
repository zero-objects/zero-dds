// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.6.3: the operations of the entity tables that are not
//! about Selections or collections: `Cache::lock` and `unlock` and the
//! transformation of an ObjectReference into an ObjectRoot (§8.1.6.3.4),
//! `ObjectHome::auto_deref` with `set_auto_deref`, `deref_all` and
//! `underef_all` (§8.1.6.3.7), `ObjectRoot::which_contained_modified`
//! (§8.1.6.3.14).

#![cfg(feature = "dcps")]
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, missing_docs)]

#[path = "support/dds.rs"]
mod dds;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use dds::{
    Bus, ObjectRecorder, Recorder, SelectionRecorder, coherent_pair, dbl, delete, enabled,
    load_until, mapping, mono, stays, vehicles, wait_until,
};
use zerodds_dlrl::cache::{
    Cache, CacheBase, CacheFactory, CacheListener, CacheUsage, Collection, FilterCriterion,
    ObjectListener, ObjectRoot, ObjectScope, ObjectState, QueryCriterion, SelectionCriterion,
    SelectionListener,
};
use zerodds_dlrl::mapping::Mapping;
use zerodds_dlrl::metamodel::DlrlException;
use zerodds_dlrl::model::{
    BasicType, ClassModel, CollectionBasis, Model, OBJECT_ROOT, ObjectRef, Relation, SimpleType,
    Value,
};
use zerodds_dlrl::types::{DlrlOid, RelationDescription, RelationKind};

const ALL: [&str; 4] = ["Radar", "Track", "Part", "Sensor"];

fn text(s: &str) -> Value {
    Value::String(s.into())
}

/// §8.1.6.3.4 `lock`: while the application holds the lock, the update
/// rounds of the Cache wait, and another thread's `lock` raises
/// `ExpiredTimeOut` when its time-out runs out or gets the lock once it
/// is free; `unlock` by a thread that does not hold it raises
/// `PreconditionNotMet`; the holder may lock again; a negative time-out
/// is `BadParameter`.
#[test]
fn dlrl_8_1_6_3_4_lock_holds_off_rounds_and_other_threads() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let cache = sub.cache.clone();

    assert!(matches!(
        cache.lock(-1),
        Err(DlrlException::BadParameter { .. })
    ));
    cache.lock(0).unwrap();

    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let radar = publ.home("Radar").create_object(&access).unwrap();
    radar.set("name", mono(text("r1"))).unwrap();
    access.write().unwrap();
    let oid = radar.oid();
    let base = CacheBase::Cache(cache.clone());
    // The sample arrives; the round that would apply it waits.
    stays(
        "no round while the Cache is locked",
        Duration::from_millis(600),
        || sub.home("Radar").find_object(oid, &base).is_err(),
    );

    // Another thread: its lock runs out, its unlock is refused.
    let other = cache.clone();
    let (timed_out, refused) = std::thread::spawn(move || {
        let start = Instant::now();
        let r = other.lock(150);
        let waited = start.elapsed();
        let u = other.unlock();
        (
            matches!(r, Err(DlrlException::ExpiredTimeOut { .. }))
                && waited >= Duration::from_millis(150),
            matches!(u, Err(DlrlException::PreconditionNotMet { .. })),
        )
    })
    .join()
    .unwrap();
    assert!(
        timed_out,
        "lock(150) of another thread raises ExpiredTimeOut"
    );
    assert!(refused, "unlock by a thread without the lock is refused");

    // The holder locks again and unlocks as often.
    cache.lock(0).unwrap();
    cache.unlock().unwrap();
    assert!(sub.home("Radar").find_object(oid, &base).is_err());

    // A thread waiting with a long time-out gets the lock once it is free.
    let waiter = cache.clone();
    let handle = std::thread::spawn(move || {
        let r = waiter.lock(10_000);
        let ok = r.is_ok();
        if ok {
            waiter.unlock().unwrap();
        }
        ok
    });
    std::thread::sleep(Duration::from_millis(200));
    cache.unlock().unwrap();
    assert!(handle.join().unwrap(), "the waiting thread got the lock");
    assert!(matches!(
        cache.unlock(),
        Err(DlrlException::PreconditionNotMet { .. })
    ));

    // Unlocked, the round applies the waiting sample.
    wait_until("the radar after unlock", || {
        sub.home("Radar").find_object(oid, &base).is_ok()
    });
    delete(sub);
    delete(publ);
}

/// §8.1.6.3.4: the holder of the lock refreshes two CacheAccesses on one
/// state of the Cache: a round that arrives in between does not reach
/// either before `unlock`.
#[test]
fn dlrl_8_1_6_3_4_lock_gives_several_accesses_one_state() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let track = publ.home("Track").create_object(&access).unwrap();
    track.set("x", mono(Value::Double(1.0))).unwrap();
    track.set("y", mono(Value::Double(0.0))).unwrap();
    track.set("a_radar", mono(Value::Ref(None))).unwrap();
    access.write().unwrap();
    let oid = track.oid();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("the track", || {
        sub.home("Track").find_object(oid, &base).is_ok()
    });
    let primary = sub.home("Track").find_object(oid, &base).unwrap();
    let a = sub.cache.create_access(CacheUsage::ReadOnly).unwrap();
    let b = sub.cache.create_access(CacheUsage::ReadOnly).unwrap();
    a.create_contract(&primary, ObjectScope::SimpleObject, 0)
        .unwrap();
    b.create_contract(&primary, ObjectScope::SimpleObject, 0)
        .unwrap();

    sub.cache.lock(1000).unwrap();
    a.refresh().unwrap();
    track.set("x", mono(Value::Double(2.0))).unwrap();
    access.write().unwrap();
    std::thread::sleep(Duration::from_millis(400));
    b.refresh().unwrap();
    let x_of = |acc: &zerodds_dlrl::cache::CacheAccess| {
        let base = CacheBase::Access(acc.clone());
        dbl(&sub.home("Track").find_object(oid, &base).unwrap(), "x")
    };
    assert_eq!(x_of(&a), 1.0);
    assert_eq!(
        x_of(&b),
        1.0,
        "the update did not reach the Cache while it was locked"
    );
    sub.cache.unlock().unwrap();
    wait_until("the update after unlock", || dbl(&primary, "x") == 2.0);
    b.refresh().unwrap();
    assert_eq!(x_of(&b), 2.0);
    delete(sub);
    delete(publ);
}

/// Runs `f` on a thread of its own and gives its result; fails when it
/// does not return within `secs` seconds (a hang) or panics.
fn within<T: Send + 'static>(what: &str, secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(v) => v,
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("{what} did not return within {secs} s"),
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("{what} panicked"),
    }
}

/// §8.1.6.3.4 with §8.1.6.3.1 `delete_cache`: the thread that holds the
/// lock deletes the Cache. The update thread, whose next round waits for
/// the lock, ends, and `delete_cache` returns; the `unlock` afterwards
/// raises `AlreadyDeleted`.
#[test]
fn dlrl_8_1_6_3_4_delete_cache_by_the_holder_of_the_lock_returns() {
    let bus = Bus::new();
    let m = mapping();
    let side = enabled(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let cache = side.cache.clone();
    let (deleted, unlocked) = within("lock, then delete_cache on one thread", 10, move || {
        cache.lock(0).unwrap();
        // The update thread looks for samples every 200 ms; its round
        // now waits for the lock.
        std::thread::sleep(Duration::from_millis(500));
        let d = CacheFactory::get_instance().delete_cache(&cache);
        (d, cache.unlock())
    });
    deleted.unwrap();
    assert!(
        matches!(unlocked, Err(DlrlException::AlreadyDeleted { .. })),
        "{unlocked:?}"
    );
    assert!(side.cache.is_deleted());
}

/// §8.1.6.3.4 with §8.1.6.3.1 `delete_cache`: thread A holds the lock,
/// thread C waits for it, thread B deletes the Cache. `delete_cache`
/// returns, C's `lock` raises `AlreadyDeleted` at once instead of waiting
/// for its time-out, and A's `unlock` afterwards does not block and
/// raises `AlreadyDeleted`.
#[test]
fn dlrl_8_1_6_3_4_delete_cache_while_another_thread_holds_the_lock_returns() {
    let bus = Bus::new();
    let m = mapping();
    let side = enabled(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let cache = side.cache.clone();
    let (locked_tx, locked_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let a = cache.clone();
    let holder = std::thread::spawn(move || {
        a.lock(0).unwrap();
        locked_tx.send(()).unwrap();
        go_rx.recv().unwrap();
        let start = Instant::now();
        let r = a.unlock();
        (r, start.elapsed())
    });
    locked_rx.recv().unwrap();
    let c = cache.clone();
    let waiter = std::thread::spawn(move || {
        let start = Instant::now();
        let r = c.lock(20_000);
        (r, start.elapsed())
    });
    std::thread::sleep(Duration::from_millis(500));
    let b = cache.clone();
    within(
        "delete_cache while thread A holds the lock",
        10,
        move || CacheFactory::get_instance().delete_cache(&b),
    )
    .unwrap();
    let (waited, waited_for) = waiter.join().unwrap();
    assert!(
        matches!(waited, Err(DlrlException::AlreadyDeleted { .. })),
        "{waited:?}"
    );
    assert!(
        waited_for < Duration::from_secs(10),
        "the waiting lock ended with the Cache, after {waited_for:?}"
    );
    go_tx.send(()).unwrap();
    let (unlocked, took) = within("thread A's unlock", 10, move || holder.join().unwrap());
    assert!(
        matches!(unlocked, Err(DlrlException::AlreadyDeleted { .. })),
        "{unlocked:?}"
    );
    assert!(took < Duration::from_secs(1), "unlock took {took:?}");
    assert!(matches!(
        cache.lock(0),
        Err(DlrlException::AlreadyDeleted { .. })
    ));
}

/// Notes the panics of the update thread of a Cache, whose end
/// `delete_cache` does not wait for when a listener on that thread calls
/// it, until [`PanicWatch::finish`].
struct PanicWatch {
    panics: Arc<Mutex<Vec<String>>>,
    previous: Arc<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>,
}

impl PanicWatch {
    fn new(cache: &Cache) -> Self {
        let thread = format!("dlrl-{}", cache.name());
        let panics = Arc::new(Mutex::new(Vec::<String>::new()));
        let previous: Arc<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync> =
            Arc::from(std::panic::take_hook());
        {
            let panics = Arc::clone(&panics);
            let previous = Arc::clone(&previous);
            std::panic::set_hook(Box::new(move |info| {
                if std::thread::current().name() == Some(thread.as_str()) {
                    panics.lock().unwrap().push(info.to_string());
                }
                previous(info);
            }));
        }
        Self { panics, previous }
    }

    fn any(&self) -> bool {
        !self.panics.lock().unwrap().is_empty()
    }

    /// Restores the previous panic hook and returns the panics noted.
    fn finish(self) -> Vec<String> {
        let _ours = std::panic::take_hook();
        let previous = self.previous;
        std::panic::set_hook(Box::new(move |info| previous(info)));
        std::mem::take(&mut *self.panics.lock().unwrap())
    }
}

/// A CacheListener that deletes its Cache in its first
/// `on_begin_updates` and notes the result, and counts `on_end_updates`.
struct DeletingAtBegin {
    cache: Cache,
    deleted: Mutex<Option<Result<(), DlrlException>>>,
    ends: AtomicU32,
}

impl CacheListener for DeletingAtBegin {
    fn on_begin_updates(&self) {
        let mut deleted = self.deleted.lock().unwrap();
        if deleted.is_none() {
            *deleted = Some(CacheFactory::get_instance().delete_cache(&self.cache));
        }
    }
    fn on_end_updates(&self) {
        self.ends.fetch_add(1, Ordering::SeqCst);
    }
    fn on_updates_enabled(&self) {}
    fn on_updates_disabled(&self) {}
}

/// §8.1.6.3.4 with §8.1.6.3.1 `delete_cache` called by a CacheListener
/// in `on_begin_updates`, on the Cache's own update thread: it returns,
/// the round applies nothing and ends with `on_end_updates`, and the
/// update thread does not panic (its round used the topics that
/// `delete_cache` had taken away). A panic hook watches the thread of
/// the Cache, whose end `delete_cache` does not wait for here.
#[test]
fn dlrl_8_1_6_3_4_delete_cache_in_on_begin_updates_ends_the_round() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let watch = PanicWatch::new(&sub.cache);
    let listener = Arc::new(DeletingAtBegin {
        cache: sub.cache.clone(),
        deleted: Mutex::new(None),
        ends: AtomicU32::new(0),
    });
    sub.cache
        .attach_listener(Arc::clone(&listener) as Arc<dyn CacheListener>)
        .unwrap();

    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let radar = publ.home("Radar").create_object(&access).unwrap();
    radar.set("name", mono(text("r1"))).unwrap();
    access.write().unwrap();
    wait_until("the end of the round or a panic", || {
        listener.ends.load(Ordering::SeqCst) > 0 || watch.any()
    });
    // A panic after on_end_updates would come within this time.
    std::thread::sleep(Duration::from_millis(500));

    assert_eq!(watch.finish(), Vec::<String>::new());
    assert!(matches!(*listener.deleted.lock().unwrap(), Some(Ok(()))));
    assert_eq!(listener.ends.load(Ordering::SeqCst), 1);
    assert!(sub.cache.is_deleted());
    assert!(matches!(
        sub.cache.load(),
        Err(DlrlException::AlreadyDeleted { .. })
    ));
    delete(publ);
}

/// An ObjectListener that notes `<tag> created <class>`, deletes its
/// Cache when it is not deleted yet and answers FALSE: without the
/// deletion the next listener of the home and then the listeners of the
/// parent's home would hear the creation.
struct DeletingOnCreate {
    tag: &'static str,
    cache: Cache,
    sink: Arc<Recorder>,
}

impl ObjectListener for DeletingOnCreate {
    fn on_object_created(&self, o: &ObjectRoot) -> bool {
        self.sink
            .events
            .lock()
            .unwrap()
            .push(format!("{} created {}", self.tag, o.class_name()));
        if !self.cache.is_deleted() {
            CacheFactory::get_instance()
                .delete_cache(&self.cache)
                .unwrap();
        }
        false
    }
    fn on_object_modified(&self, _o: &ObjectRoot) -> bool {
        false
    }
    fn on_object_deleted(&self, _o: &ObjectRoot) -> bool {
        false
    }
}

/// A SelectionListener that notes `<tag> in <class>` and deletes its
/// Cache when it is not deleted yet.
struct DeletingOnIn {
    tag: &'static str,
    cache: Cache,
    sink: Arc<Recorder>,
}

impl SelectionListener for DeletingOnIn {
    fn on_object_in(&self, o: &ObjectRoot) {
        self.sink
            .events
            .lock()
            .unwrap()
            .push(format!("{} in {}", self.tag, o.class_name()));
        if !self.cache.is_deleted() {
            CacheFactory::get_instance()
                .delete_cache(&self.cache)
                .unwrap();
        }
    }
    fn on_object_out(&self, _o: &ObjectRoot) {}
    fn on_object_modified(&self, _o: &ObjectRoot) {}
}

/// Every object passes.
struct AllPass;

impl FilterCriterion for AllPass {}

/// Writes a `Car` from `publ` and returns the events of the round of
/// `sub` that `rec` records, after the round ended without a panic of
/// the update thread of `sub`.
fn round_after_a_car(publ: &dds::Side, watch: PanicWatch, rec: &Recorder) -> Vec<String> {
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let car = publ.home("Car").create_object(&access).unwrap();
    car.set("name", mono(text("a"))).unwrap();
    car.set("seats", mono(Value::Long(4))).unwrap();
    access.write().unwrap();
    wait_until("the end of the round or a panic", || {
        rec.snapshot().iter().any(|e| e == "end") || watch.any()
    });
    // A panic after on_end_updates would come within this time.
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(watch.finish(), Vec::<String>::new());
    rec.take()
}

/// §8.1.6.3.4 with §8.1.6.3.1 `delete_cache` called by an ObjectListener
/// on the Cache's thread: no other listener of the round is called but
/// `on_end_updates`. Not the second listener of the same home and not
/// the listener of the parent's home (the deleting listeners answer
/// FALSE, so without the deletion all of them hear the creation), and the
/// Selection of the parent's home is not examined. Whichever of the two
/// listeners of `Car` comes first deletes the Cache.
#[test]
fn dlrl_8_1_6_3_4_delete_cache_in_an_object_listener_calls_no_further_listener() {
    let bus = Bus::new();
    let m = vehicles();
    let (sub, publ) = coherent_pair(&bus, &m, &["Vehicle", "Car"]);
    let watch = PanicWatch::new(&sub.cache);
    let rec = Arc::new(Recorder::default());
    sub.cache.attach_listener(rec.clone()).unwrap();
    for tag in ["car-a", "car-b"] {
        sub.home("Car").attach_listener(
            Arc::new(DeletingOnCreate {
                tag,
                cache: sub.cache.clone(),
                sink: rec.clone(),
            }),
            false,
        );
    }
    sub.home("Vehicle").attach_listener(
        Arc::new(ObjectRecorder {
            tag: "vehicle",
            answer: true,
            watched: vec!["name"],
            sink: rec.clone(),
        }),
        false,
    );
    let sel = sub
        .home("Vehicle")
        .create_selection(SelectionCriterion::Filter(Arc::new(AllPass)), true, false)
        .unwrap();
    sel.set_listener(Some(Arc::new(SelectionRecorder {
        tag: "vehicles",
        sink: rec.clone(),
    })))
    .unwrap();

    let events = round_after_a_car(&publ, watch, &rec);
    assert_eq!(events.len(), 3, "{events:?}");
    assert_eq!(events[0], "begin");
    assert!(
        ["car-a created Car", "car-b created Car"].contains(&events[1].as_str()),
        "{events:?}"
    );
    assert_eq!(events[2], "end");
    assert!(sub.cache.is_deleted());
    delete(publ);
}

/// §8.1.6.3.4 with §8.1.6.3.1 `delete_cache` called by a
/// SelectionListener on the Cache's thread: no further Selection of the
/// round is examined, neither the second one of the same home nor the
/// one of the parent's home, and no other listener is called but
/// `on_end_updates`. Whichever of the three Selections comes first
/// deletes the Cache.
#[test]
fn dlrl_8_1_6_3_4_delete_cache_in_a_selection_listener_calls_no_further_listener() {
    let bus = Bus::new();
    let m = vehicles();
    let (sub, publ) = coherent_pair(&bus, &m, &["Vehicle", "Car"]);
    let watch = PanicWatch::new(&sub.cache);
    let rec = Arc::new(Recorder::default());
    sub.cache.attach_listener(rec.clone()).unwrap();
    for (class, tag) in [("Car", "car-a"), ("Car", "car-b"), ("Vehicle", "vehicles")] {
        let sel = sub
            .home(class)
            .create_selection(SelectionCriterion::Filter(Arc::new(AllPass)), true, false)
            .unwrap();
        sel.set_listener(Some(Arc::new(DeletingOnIn {
            tag,
            cache: sub.cache.clone(),
            sink: rec.clone(),
        })))
        .unwrap();
    }

    let events = round_after_a_car(&publ, watch, &rec);
    assert_eq!(events.len(), 3, "{events:?}");
    assert_eq!(events[0], "begin");
    assert!(
        ["car-a in Car", "car-b in Car", "vehicles in Car"].contains(&events[1].as_str()),
        "{events:?}"
    );
    assert_eq!(events[2], "end");
    assert!(sub.cache.is_deleted());
    delete(publ);
}

/// The events of a round: the CacheListener's calls, with the
/// interruption indication read in `on_end_updates`, and the object
/// listener's calls, in one list.
#[derive(Default)]
struct Events(Mutex<Vec<String>>);

impl Events {
    fn push(&self, e: String) {
        self.0.lock().unwrap().push(e);
    }

    fn snapshot(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

/// A CacheListener that calls `disable_updates` in its first
/// `on_begin_updates` when `disable_at_begin` is set, and notes
/// `end interrupted=<Cache::last_round_interrupted>` in `on_end_updates`.
struct Interrupting {
    cache: Cache,
    disable_at_begin: AtomicBool,
    events: Arc<Events>,
}

impl CacheListener for Interrupting {
    fn on_begin_updates(&self) {
        self.events.push("begin".into());
        if self.disable_at_begin.swap(false, Ordering::SeqCst) {
            self.cache.disable_updates().unwrap();
        }
    }
    fn on_end_updates(&self) {
        self.events.push(format!(
            "end interrupted={}",
            self.cache.last_round_interrupted()
        ));
    }
    fn on_updates_enabled(&self) {
        self.events.push("enabled".into());
    }
    fn on_updates_disabled(&self) {
        self.events.push("disabled".into());
    }
}

/// An ObjectListener that notes `created <name>` and calls
/// `disable_updates` in its first `on_object_created` when `disable` is
/// set.
struct DisablingOnCreate {
    cache: Cache,
    disable: AtomicBool,
    events: Arc<Events>,
}

impl ObjectListener for DisablingOnCreate {
    fn on_object_created(&self, o: &ObjectRoot) -> bool {
        let name = match o.get("name").unwrap() {
            zerodds_dlrl::model::AttributeValue::Mono(Value::String(s)) => s,
            other => panic!("name: {other:?}"),
        };
        self.events.push(format!("created {name}"));
        if self.disable.swap(false, Ordering::SeqCst) {
            self.cache.disable_updates().unwrap();
        }
        true
    }
    fn on_object_modified(&self, _o: &ObjectRoot) -> bool {
        self.events.push("modified".into());
        true
    }
    fn on_object_deleted(&self, _o: &ObjectRoot) -> bool {
        self.events.push("deleted".into());
        true
    }
}

/// §8.1.6.3.4 `disable_updates` called in the middle of a set of updates:
/// "the Listener will receive end_updates with a parameter that indicates
/// that the updates have been interrupted". `on_end_updates` has no
/// parameter in the table of §8.1.6.3.5 and in the IDL; the indication is
/// `Cache::last_round_interrupted`, read in `on_end_updates`. Called in
/// `on_begin_updates`, the updates of the round stay registered: no
/// object listener hears them, the object is not in the Cache, and
/// `enable_updates` applies them as a round with listeners that is not
/// interrupted.
#[test]
fn dlrl_8_1_6_3_4_disable_updates_in_on_begin_updates_interrupts_the_round() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let events = Arc::new(Events::default());
    let listener: Arc<dyn CacheListener> = Arc::new(Interrupting {
        cache: sub.cache.clone(),
        disable_at_begin: AtomicBool::new(true),
        events: Arc::clone(&events),
    });
    sub.cache.attach_listener(Arc::clone(&listener)).unwrap();
    let objects: Arc<dyn ObjectListener> = Arc::new(DisablingOnCreate {
        cache: sub.cache.clone(),
        disable: AtomicBool::new(false),
        events: Arc::clone(&events),
    });
    sub.home("Radar")
        .attach_listener(Arc::clone(&objects), false);
    assert!(!sub.cache.last_round_interrupted());

    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let radar = publ.home("Radar").create_object(&access).unwrap();
    radar.set("name", mono(text("r1"))).unwrap();
    access.write().unwrap();
    wait_until("the end of the interrupted round", || {
        events.snapshot().iter().any(|e| e.starts_with("end"))
    });
    assert_eq!(
        events.take(),
        ["begin", "disabled", "end interrupted=true"],
        "no object listener hears the updates of the interrupted round"
    );
    assert!(!sub.cache.updates_enabled());
    assert!(sub.cache.last_round_interrupted());
    let base = CacheBase::Cache(sub.cache.clone());
    assert!(
        sub.home("Radar").find_object(radar.oid(), &base).is_err(),
        "the updates stay registered"
    );

    sub.cache.enable_updates().unwrap();
    assert_eq!(
        events.take(),
        ["enabled", "begin", "created r1", "end interrupted=false"]
    );
    assert!(!sub.cache.last_round_interrupted());
    assert!(sub.home("Radar").find_object(radar.oid(), &base).is_ok());

    sub.home("Radar").detach_listener(&objects);
    sub.cache.detach_listener(&listener).unwrap();
    delete(sub);
    delete(publ);
}

/// §8.1.6.3.4 `disable_updates` called by an object listener in the
/// middle of a round: the updates of the round were applied before the
/// first object listener, so the listeners hear the rest of the round;
/// `on_end_updates` reads the interruption at
/// `Cache::last_round_interrupted`. The updates that arrive afterwards
/// are registered: they change the Cache only at `load`, without
/// listener.
#[test]
fn dlrl_8_1_6_3_4_disable_updates_from_an_object_listener_interrupts_the_round() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    let events = Arc::new(Events::default());
    let listener: Arc<dyn CacheListener> = Arc::new(Interrupting {
        cache: sub.cache.clone(),
        disable_at_begin: AtomicBool::new(false),
        events: Arc::clone(&events),
    });
    sub.cache.attach_listener(Arc::clone(&listener)).unwrap();
    let objects: Arc<dyn ObjectListener> = Arc::new(DisablingOnCreate {
        cache: sub.cache.clone(),
        disable: AtomicBool::new(true),
        events: Arc::clone(&events),
    });
    sub.home("Radar")
        .attach_listener(Arc::clone(&objects), false);

    // One write, one coherent set: one round with two creations.
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let r1 = publ.home("Radar").create_object(&access).unwrap();
    r1.set("name", mono(text("r1"))).unwrap();
    let r2 = publ.home("Radar").create_object(&access).unwrap();
    r2.set("name", mono(text("r2"))).unwrap();
    access.write().unwrap();
    wait_until("the end of the round", || {
        events.snapshot().iter().any(|e| e.starts_with("end"))
    });
    let mut got = events.take();
    assert_eq!(got.first().map(String::as_str), Some("begin"));
    assert_eq!(got.last().map(String::as_str), Some("end interrupted=true"));
    got.retain(|e| e.starts_with("created") || e == "disabled");
    assert_eq!(got.len(), 3, "{got:?}");
    assert_eq!(got[1], "disabled", "{got:?}");
    let mut created: Vec<&str> = [got[0].as_str(), got[2].as_str()].to_vec();
    created.sort_unstable();
    assert_eq!(created, ["created r1", "created r2"]);
    assert!(!sub.cache.updates_enabled());
    assert!(sub.cache.last_round_interrupted());

    // Manual mode: the next update waits for load.
    r1.set("name", mono(text("r1b"))).unwrap();
    access.write().unwrap();
    let base = CacheBase::Cache(sub.cache.clone());
    let primary = sub.home("Radar").find_object(r1.oid(), &base).unwrap();
    let name = |o: &ObjectRoot| o.get("name").unwrap();
    stays(
        "the Cache keeps the name until load",
        Duration::from_millis(600),
        || name(&primary) == mono(text("r1")),
    );
    wait_until("the update after load", || {
        sub.cache.load().unwrap();
        name(&primary) == mono(text("r1b"))
    });
    assert!(events.take().is_empty(), "load calls no listener");

    sub.home("Radar").detach_listener(&objects);
    sub.cache.detach_listener(&listener).unwrap();
    delete(sub);
    delete(publ);
}

/// §8.1.6.3.4: the transformation of an ObjectReference gives the
/// ObjectRoot of the Cache, with its state loaded (here of a home with
/// `auto_deref` FALSE); a reference to no object of the Cache raises
/// `NotFound`; the ObjectRoot is not modifiable.
#[test]
fn dlrl_8_1_6_3_4_an_object_reference_becomes_its_object_root() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    sub.home("Radar").set_auto_deref(false);
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let radar = publ.home("Radar").create_object(&access).unwrap();
    radar.set("name", mono(text("r1"))).unwrap();
    access.write().unwrap();
    let oid = radar.oid();
    let base = CacheBase::Cache(sub.cache.clone());
    wait_until("the radar", || {
        sub.home("Radar").find_object(oid, &base).is_ok()
    });
    let primary = sub.home("Radar").find_object(oid, &base).unwrap();
    assert!(!primary.is_dereferenced());

    let got = sub
        .cache
        .deref_reference(&ObjectRef::new("Radar", oid))
        .unwrap();
    assert_eq!(got, primary, "the already instantiated ObjectRoot");
    assert!(got.is_dereferenced());
    assert_eq!(got.get("name").unwrap(), mono(text("r1")));
    assert!(matches!(
        got.set("name", mono(text("other"))),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        sub.cache
            .deref_reference(&ObjectRef::new("Radar", DlrlOid::new(7, 7, 7))),
        Err(DlrlException::NotFound { .. })
    ));
    assert!(matches!(
        sub.cache.deref_reference(&ObjectRef::new("Track", oid)),
        Err(DlrlException::NotFound { .. })
    ));
    delete(sub);
    delete(publ);
}

/// §8.1.6.3.7 `auto_deref` FALSE: an object arrives without its state
/// loaded; read state, modified attributes, a QueryCriterion and the
/// clone of a CacheAccess work without loading it; the first access
/// loads it. `underef_all` unloads, `deref_all` loads, `set_auto_deref`
/// TRUE loads all; the collection changes of a round are those of the
/// object loaded later; a deleted object keeps its last state. The
/// default is TRUE.
#[test]
fn dlrl_8_1_6_3_7_auto_deref_false_loads_the_state_on_access() {
    let bus = Bus::new();
    let m = mapping();
    let (sub, publ) = coherent_pair(&bus, &m, &ALL);
    sub.cache.disable_updates().unwrap();
    let tracks = sub.home("Track");
    assert!(tracks.auto_deref());
    tracks.set_auto_deref(false);
    assert!(!tracks.auto_deref());
    let q = QueryCriterion::new(tracks, "x > 1.0", &[]).unwrap();
    let sel = tracks
        .create_selection(SelectionCriterion::Query(q), true, false)
        .unwrap();

    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let radar = publ.home("Radar").create_object(&access).unwrap();
    radar.set("name", mono(text("r"))).unwrap();
    let t = publ.home("Track").create_object(&access).unwrap();
    t.set("x", mono(Value::Double(5.0))).unwrap();
    t.set("y", mono(Value::Double(6.0))).unwrap();
    t.set_reference("a_radar", Some(&radar)).unwrap();
    t.get_str_map("comments")
        .unwrap()
        .put("a", text("first"))
        .unwrap();
    access.write().unwrap();
    let oid = t.oid();

    let track = load_until(&sub, "Track", oid, "the track", |_| true);
    assert_eq!(track.read_state(), ObjectState::New);
    assert!(!track.is_dereferenced(), "not loaded on arrival");
    assert_eq!(sel.members(), vec![track.clone()]);
    assert!(
        !track.is_dereferenced(),
        "the QueryCriterion does not load it"
    );
    let radar_primary = sub
        .home("Radar")
        .find_object(radar.oid(), &CacheBase::Cache(sub.cache.clone()))
        .unwrap();
    assert!(
        radar_primary.is_dereferenced(),
        "Radar's home has auto_deref TRUE"
    );

    // The clone of a CacheAccess holds the state; the primary stays.
    let view = sub.cache.create_access(CacheUsage::ReadOnly).unwrap();
    view.create_contract(&track, ObjectScope::SimpleObject, 0)
        .unwrap();
    view.refresh().unwrap();
    let clone = tracks
        .find_object(oid, &CacheBase::Access(view.clone()))
        .unwrap();
    assert!(clone.is_dereferenced());
    assert_eq!(dbl(&clone, "x"), 5.0);
    assert!(!track.is_dereferenced());

    // A modification: the attribute flags without loading.
    t.set("x", mono(Value::Double(7.0))).unwrap();
    access.write().unwrap();
    let track = load_until(&sub, "Track", oid, "x modified", |o| {
        o.is_attribute_modified("x").unwrap()
    });
    assert_eq!(track.read_state(), ObjectState::Modified);
    assert!(!track.is_attribute_modified("y").unwrap());
    assert!(!track.is_attribute_modified("comments").unwrap());
    assert!(track.is_modified(ObjectScope::SimpleObject).unwrap());
    assert!(!track.is_dereferenced());
    assert_eq!(dbl(&track, "x"), 7.0, "the most recent state");
    assert!(track.is_dereferenced(), "loaded by the access");

    // Loaded once, it follows the rounds; underef_all unloads it.
    tracks.underef_all();
    assert!(!track.is_dereferenced());
    t.get_str_map("comments")
        .unwrap()
        .put("b", text("second"))
        .unwrap();
    access.write().unwrap();
    let track = load_until(&sub, "Track", oid, "comments modified", |o| {
        o.is_attribute_modified("comments").unwrap()
    });
    assert!(!track.is_dereferenced());
    let comments = track.get_str_map("comments").unwrap();
    assert_eq!(comments.added_elements().unwrap(), ["b".to_string()]);
    assert!(comments.removed_elements().unwrap().is_empty());
    assert_eq!(comments.length(), 2);
    assert!(track.is_dereferenced());

    tracks.underef_all();
    tracks.deref_all();
    assert!(track.is_dereferenced());
    tracks.underef_all();
    tracks.set_auto_deref(true);
    assert!(track.is_dereferenced(), "set_auto_deref(TRUE) loads all");
    tracks.underef_all();
    t.set("y", mono(Value::Double(9.0))).unwrap();
    access.write().unwrap();
    let track = load_until(&sub, "Track", oid, "y modified", |o| {
        o.is_attribute_modified("y").unwrap()
    });
    assert!(
        track.is_dereferenced(),
        "with auto_deref TRUE a round loads it"
    );

    // A deleted object keeps its last state.
    tracks.set_auto_deref(false);
    tracks.underef_all();
    assert!(!track.is_dereferenced());
    t.destroy().unwrap();
    access.write().unwrap();
    wait_until("the deletion", || {
        sub.cache.load().unwrap();
        track.read_state() == ObjectState::Deleted
    });
    assert_eq!(dbl(&track, "y"), 9.0);
    assert_eq!(dbl(&track, "x"), 7.0);
    delete(sub);
    delete(publ);
}

/// `Box { label, near -> Part, lid -> Part, parts: List<Part>,
/// spares: Set<Part>, by_name: StrMap<Part>, by_slot: IntMap<Part> }`,
/// all relations but `near` compositions; `Part { serial }`.
fn boxes() -> Arc<Mapping> {
    let model = Model::new(vec![
        ClassModel::new("Part")
            .base(OBJECT_ROOT)
            .mono("serial", SimpleType::Basic(BasicType::Long)),
        ClassModel::new("Box")
            .base(OBJECT_ROOT)
            .mono("label", SimpleType::Basic(BasicType::String))
            .relation(Relation::to_one("near", "Part"))
            .relation(Relation::to_one("lid", "Part").composition())
            .relation(Relation::to_many("parts", "Part", CollectionBasis::List).composition())
            .relation(Relation::to_many("spares", "Part", CollectionBasis::Set).composition())
            .relation(Relation::to_many("by_name", "Part", CollectionBasis::StrMap).composition())
            .relation(Relation::to_many("by_slot", "Part", CollectionBasis::IntMap).composition()),
    ])
    .unwrap();
    Arc::new(Mapping::default_for(model).unwrap())
}

/// §8.1.6.3.14 `which_contained_modified`: the composition relations
/// that designate a contained object modified by the round, with the
/// index of a List and the key of a StrMap or IntMap; a plain relation
/// to a modified object is not named; NEW contained objects are not
/// modified; for a clone, the modifications of the last refresh.
#[test]
fn dlrl_8_1_6_3_14_which_contained_modified_names_relation_index_and_key() {
    let bus = Bus::new();
    let m = boxes();
    let (sub, publ) = coherent_pair(&bus, &m, &["Part", "Box"]);
    sub.cache.disable_updates().unwrap();
    let access = publ.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let part = |serial: i32| {
        let p = publ.home("Part").create_object(&access).unwrap();
        p.set("serial", mono(Value::Long(serial))).unwrap();
        p
    };
    let (lid, p0, p1, spare, named, slotted) =
        (part(1), part(2), part(3), part(4), part(5), part(6));
    let b = publ.home("Box").create_object(&access).unwrap();
    b.set("label", mono(text("b"))).unwrap();
    b.set_reference("near", Some(&p1)).unwrap();
    b.set_reference("lid", Some(&lid)).unwrap();
    let parts = b.get_list("parts").unwrap();
    parts.add_object(&p0).unwrap();
    parts.add_object(&p1).unwrap();
    b.get_set("spares").unwrap().add_object(&spare).unwrap();
    b.get_str_map("by_name")
        .unwrap()
        .put_object("n", &named)
        .unwrap();
    b.get_int_map("by_slot")
        .unwrap()
        .put_object(7, &slotted)
        .unwrap();
    access.write().unwrap();
    let oid = b.oid();

    let got = load_until(&sub, "Box", oid, "the box", |o| {
        o.read_state() == ObjectState::New
    });
    assert!(
        got.which_contained_modified().unwrap().is_empty(),
        "NEW contained objects are not modified"
    );
    let view = sub.cache.create_access(CacheUsage::ReadOnly).unwrap();
    view.create_contract(&got, ObjectScope::ContainedObjects, 0)
        .unwrap();
    view.refresh().unwrap();
    let clone = sub
        .home("Box")
        .find_object(oid, &CacheBase::Access(view.clone()))
        .unwrap();
    assert!(clone.which_contained_modified().unwrap().is_empty());

    // The lid, the second list item (also designated by `near`) and the
    // map item "n".
    lid.set("serial", mono(Value::Long(11))).unwrap();
    p1.set("serial", mono(Value::Long(13))).unwrap();
    named.set("serial", mono(Value::Long(15))).unwrap();
    access.write().unwrap();
    let got = load_until(&sub, "Box", oid, "contained modified", |o| {
        !o.which_contained_modified().unwrap().is_empty()
    });
    let first = got.which_contained_modified().unwrap();
    assert_eq!(
        first,
        vec![
            RelationDescription::StrMap {
                name: "by_name".into(),
                key: "n".into()
            },
            RelationDescription::Ref { name: "lid".into() },
            RelationDescription::List {
                name: "parts".into(),
                index: 1
            },
        ]
    );
    assert!(!got.is_modified(ObjectScope::SimpleObject).unwrap());
    assert!(got.is_modified(ObjectScope::ContainedObjects).unwrap());

    // The set item and the IntMap item 7.
    spare.set("serial", mono(Value::Long(14))).unwrap();
    slotted.set("serial", mono(Value::Long(16))).unwrap();
    access.write().unwrap();
    let got = load_until(&sub, "Box", oid, "second round", |o| {
        o.which_contained_modified()
            .unwrap()
            .iter()
            .any(|d| d.kind() == RelationKind::SetRelation)
    });
    assert_eq!(
        got.which_contained_modified().unwrap(),
        vec![
            RelationDescription::IntMap {
                name: "by_slot".into(),
                key: 7
            },
            RelationDescription::Set {
                name: "spares".into()
            },
        ]
    );

    // The clone: the modifications of the refresh since the first one.
    view.refresh().unwrap();
    assert_eq!(
        clone.which_contained_modified().unwrap(),
        vec![
            RelationDescription::StrMap {
                name: "by_name".into(),
                key: "n".into()
            },
            RelationDescription::IntMap {
                name: "by_slot".into(),
                key: 7
            },
            RelationDescription::Ref { name: "lid".into() },
            RelationDescription::List {
                name: "parts".into(),
                index: 1
            },
            RelationDescription::Set {
                name: "spares".into()
            },
        ]
    );
    delete(sub);
    delete(publ);
}
