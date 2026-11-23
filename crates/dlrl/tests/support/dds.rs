// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What the tests of the run-time entities share: a model of radars and
//! tracks, participants on one isolated bus, Caches with their homes,
//! recording listeners.

#![allow(
    dead_code,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zerodds_dcps::qos::PresentationAccessScope;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{DomainParticipant, DomainParticipantFactory, DomainParticipantQos, Entity};
use zerodds_dlrl::cache::{
    Cache, CacheDescription, CacheFactory, CacheListener, CacheUsage, ObjectHome, ObjectListener,
    ObjectRoot, SelectionListener,
};
use zerodds_dlrl::mapping::Mapping;
use zerodds_dlrl::model::{
    AttributeValue, BasicType, ClassModel, CollectionBasis, Model, OBJECT_ROOT, ObjectRef,
    Relation, SimpleType, Value,
};

/// An SPDP multicast group of its own per call, and fast discovery; the
/// participants of one test share one config (clone it).
pub fn isolated_cfg() -> RuntimeConfig {
    static SLOT: AtomicU32 = AtomicU32::new(0);
    let slot = SLOT.fetch_add(1, Ordering::Relaxed);
    let raw = std::process::id().wrapping_mul(2_654_435_761) ^ slot.wrapping_mul(0x9E37_79B9);
    let x = 1 + ((raw >> 8) & 0x7F) as u8;
    let y = 2 + (raw & 0x7F) as u8;
    RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        nack_response_delay: Duration::ZERO,
        reader_heartbeat_response_delay: Duration::ZERO,
        spdp_multicast_group: Ipv4Addr::new(239, 255, x, y),
        ..RuntimeConfig::default()
    }
}

pub struct Bus {
    pub domain: i32,
    pub cfg: RuntimeConfig,
}

impl Bus {
    pub fn new() -> Self {
        Self {
            domain: zerodds_test_domains::unique_domain(),
            cfg: isolated_cfg(),
        }
    }

    pub fn participant(&self) -> DomainParticipant {
        DomainParticipantFactory::instance()
            .create_participant_with_config(
                self.domain,
                DomainParticipantQos::default(),
                self.cfg.clone(),
            )
            .expect("participant")
    }
}

fn double() -> SimpleType {
    SimpleType::Basic(BasicType::Double)
}

fn string() -> SimpleType {
    SimpleType::Basic(BasicType::String)
}

/// `Radar { name }`, `Track { x, y, comments: StrMap<string>,
/// a_radar -> Radar }`, `Sensor { label, parts: List<Part> composition }`,
/// `Part { serial }`, with the default mapping of §8.1.4.3.
pub fn mapping() -> Arc<Mapping> {
    let model = Model::new(vec![
        ClassModel::new("Radar")
            .base(OBJECT_ROOT)
            .mono("name", string()),
        ClassModel::new("Track")
            .base(OBJECT_ROOT)
            .mono("x", double())
            .mono("y", double())
            .multi("comments", CollectionBasis::StrMap, string())
            .relation(Relation::to_one("a_radar", "Radar")),
        ClassModel::new("Part")
            .base(OBJECT_ROOT)
            .mono("serial", SimpleType::Basic(BasicType::Long)),
        ClassModel::new("Sensor")
            .base(OBJECT_ROOT)
            .mono("label", string())
            .relation(Relation::to_many("parts", "Part", CollectionBasis::List).composition()),
    ])
    .unwrap();
    Arc::new(Mapping::default_for(model).unwrap())
}

/// `Vehicle { name, tags: Set<string>, readings: IntMap<double> }` and
/// `Car : Vehicle { seats }`, with the default mapping.
pub fn vehicles() -> Arc<Mapping> {
    let model = Model::new(vec![
        ClassModel::new("Vehicle")
            .base(OBJECT_ROOT)
            .mono("name", string())
            .multi("tags", CollectionBasis::Set, string())
            .multi("readings", CollectionBasis::IntMap, double()),
        ClassModel::new("Car")
            .base("Vehicle")
            .mono("seats", SimpleType::Basic(BasicType::Long)),
    ])
    .unwrap();
    Arc::new(Mapping::default_for(model).unwrap())
}

pub struct Side {
    pub participant: DomainParticipant,
    pub cache: Cache,
    pub homes: Vec<ObjectHome>,
}

impl Side {
    pub fn home(&self, class: &str) -> &ObjectHome {
        self.homes
            .iter()
            .find(|h| h.class_name() == class)
            .expect("home")
    }
}

static NAMES: AtomicU32 = AtomicU32::new(0);

/// A Cache with one home per class of `classes`, registered for pub/sub
/// but not enabled.
pub fn registered(bus: &Bus, usage: CacheUsage, mapping: &Arc<Mapping>, classes: &[&str]) -> Side {
    let participant = bus.participant();
    let name = format!(
        "cache-{}-{}-{}",
        std::process::id(),
        bus.domain,
        NAMES.fetch_add(1, Ordering::Relaxed)
    );
    let cache = CacheFactory::get_instance()
        .create_cache(
            usage,
            CacheDescription {
                name,
                domain: participant.clone(),
            },
        )
        .unwrap();
    let homes: Vec<ObjectHome> = classes
        .iter()
        .map(|c| {
            let h = ObjectHome::new(Arc::clone(mapping), c).unwrap();
            cache.register_home(&h).unwrap();
            h
        })
        .collect();
    cache.register_all_for_pubsub().unwrap();
    Side {
        participant,
        cache,
        homes,
    }
}

/// A Cache as [`registered`], enabled.
pub fn enabled(bus: &Bus, usage: CacheUsage, mapping: &Arc<Mapping>, classes: &[&str]) -> Side {
    let side = registered(bus, usage, mapping, classes);
    side.cache.enable_all_for_pubsub().unwrap();
    side
}

/// A READ_ONLY Cache and a WRITE_ONLY Cache of `classes` on two
/// participants, with PRESENTATION GROUP and `coherent_access` on the
/// Publisher and the Subscriber, so that one `CacheAccess::write` is one
/// update round of the reading Cache (§8.1.5.1); enabled, and every
/// DataWriter matched. Returns (reader, writer).
pub fn coherent_pair(bus: &Bus, mapping: &Arc<Mapping>, classes: &[&str]) -> (Side, Side) {
    let sub = registered(bus, CacheUsage::ReadOnly, mapping, classes);
    let publ = registered(bus, CacheUsage::WriteOnly, mapping, classes);
    let publisher = publ.cache.the_publisher().unwrap();
    let subscriber = sub.cache.the_subscriber().unwrap();
    let mut pq = publisher.get_qos();
    pq.presentation.access_scope = PresentationAccessScope::Group;
    pq.presentation.coherent_access = true;
    publisher.set_qos(pq).unwrap();
    let mut sq = subscriber.get_qos();
    sq.presentation.access_scope = PresentationAccessScope::Group;
    sq.presentation.coherent_access = true;
    subscriber.set_qos(sq).unwrap();
    sub.cache.enable_all_for_pubsub().unwrap();
    publ.cache.enable_all_for_pubsub().unwrap();
    for h in &publ.homes {
        for t in h.get_all_topic_names().unwrap() {
            let w: zerodds_dcps::serialized_type::DynamicDataWriter =
                publisher.lookup_datawriter(&t).expect("writer");
            wait_until("the subscription", || {
                !w.get_matched_subscriptions().unwrap().is_empty()
            });
        }
    }
    (sub, publ)
}

pub fn delete(side: Side) {
    CacheFactory::get_instance()
        .delete_cache(&side.cache)
        .unwrap();
}

/// The class `Station` on the topic `STATION` of an existing DCPS model
/// (DDS 1.2 §8.1.4.5): type `Legacy::StationType`, keyed by `SITE` and
/// `NUM`, no class and no oid field, `level` in `LEVEL`.
pub fn station_mapping() -> Arc<Mapping> {
    use zerodds_dlrl::mapping::{ClassMappingSpec, KeyDescription, MemberMappingSpec, TopicSpec};
    let model = Model::new(vec![
        ClassModel::new("Station")
            .base(OBJECT_ROOT)
            .mono("site", SimpleType::Basic(BasicType::String))
            .mono("num", SimpleType::Basic(BasicType::Long))
            .mono("level", SimpleType::Basic(BasicType::Double)),
    ])
    .unwrap();
    let attr = |name: &str, field: &str| MemberMappingSpec::MonoAttribute {
        name: name.into(),
        place_topic: None,
        value_fields: vec![field.into()],
    };
    let spec = ClassMappingSpec {
        class: "Station".into(),
        main_topic: Some(TopicSpec {
            type_name: Some("Legacy::StationType".into()),
            ..TopicSpec::new("STATION", KeyDescription::no_oid(&["SITE", "NUM"]))
        }),
        members: vec![
            attr("site", "SITE"),
            attr("num", "NUM"),
            attr("level", "LEVEL"),
        ],
        ..ClassMappingSpec::default()
    };
    Arc::new(Mapping::new(model, &[spec]).unwrap())
}

/// Waits up to 15 s for `cond`.
pub fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for {what}");
}

/// Applies the waiting updates as rounds until `cond` holds of the
/// object `oid` of `class`; the last round is the one that made it hold.
pub fn load_until(
    side: &Side,
    class: &str,
    oid: zerodds_dlrl::types::DlrlOid,
    what: &str,
    cond: impl Fn(&ObjectRoot) -> bool,
) -> ObjectRoot {
    let base = zerodds_dlrl::cache::CacheBase::Cache(side.cache.clone());
    let mut found = None;
    wait_until(what, || {
        side.cache.load().unwrap();
        match side.home(class).find_object(oid, &base) {
            Ok(o) if cond(&o) => {
                found = Some(o);
                true
            }
            _ => false,
        }
    });
    found.unwrap()
}

/// Holds for `for_` that `cond` stays true.
pub fn stays(what: &str, for_: Duration, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + for_;
    while Instant::now() < deadline {
        assert!(cond(), "{what} no longer holds");
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn mono(v: Value) -> AttributeValue {
    AttributeValue::Mono(v)
}

pub fn dbl(o: &ObjectRoot, attr: &str) -> f64 {
    match o.get(attr).unwrap() {
        AttributeValue::Mono(Value::Double(d)) => d,
        other => panic!("{attr}: {other:?}"),
    }
}

pub fn reference(o: &ObjectRoot) -> AttributeValue {
    mono(Value::Ref(Some(ObjectRef::new(&o.class_name(), o.oid()))))
}

/// The events a listener heard, in order.
#[derive(Default)]
pub struct Recorder {
    pub events: Mutex<Vec<String>>,
}

impl Recorder {
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }

    fn push(&self, e: String) {
        self.events.lock().unwrap().push(e);
    }
}

impl CacheListener for Recorder {
    fn on_begin_updates(&self) {
        self.push("begin".into());
    }
    fn on_end_updates(&self) {
        self.push("end".into());
    }
    fn on_updates_enabled(&self) {
        self.push("enabled".into());
    }
    fn on_updates_disabled(&self) {
        self.push("disabled".into());
    }
}

/// A SelectionListener that notes `<tag> in|out|modified <oid>`.
pub struct SelectionRecorder {
    pub tag: &'static str,
    pub sink: Arc<Recorder>,
}

impl SelectionListener for SelectionRecorder {
    fn on_object_in(&self, o: &ObjectRoot) {
        self.sink.push(format!("{} in {}", self.tag, o.oid()));
    }
    fn on_object_out(&self, o: &ObjectRoot) {
        self.sink.push(format!("{} out {}", self.tag, o.oid()));
    }
    fn on_object_modified(&self, o: &ObjectRoot) {
        self.sink.push(format!("{} modified {}", self.tag, o.oid()));
    }
}

/// An ObjectListener that notes `created <class>`, `modified <class>
/// [<modified attributes>]` and `deleted <class>`, and answers `answer`.
pub struct ObjectRecorder {
    pub tag: &'static str,
    pub answer: bool,
    pub watched: Vec<&'static str>,
    pub sink: Arc<Recorder>,
}

impl ObjectRecorder {
    fn modified_attrs(&self, o: &ObjectRoot) -> String {
        let m: Vec<&str> = self
            .watched
            .iter()
            .copied()
            .filter(|a| o.is_attribute_modified(a).unwrap())
            .collect();
        m.join(",")
    }
}

impl ObjectListener for ObjectRecorder {
    fn on_object_created(&self, o: &ObjectRoot) -> bool {
        self.sink
            .push(format!("{} created {}", self.tag, o.class_name()));
        self.answer
    }
    fn on_object_modified(&self, o: &ObjectRoot) -> bool {
        self.sink.push(format!(
            "{} modified {} [{}]",
            self.tag,
            o.class_name(),
            self.modified_attrs(o)
        ));
        self.answer
    }
    fn on_object_deleted(&self, o: &ObjectRoot) -> bool {
        self.sink
            .push(format!("{} deleted {}", self.tag, o.class_name()));
        self.answer
    }
}
