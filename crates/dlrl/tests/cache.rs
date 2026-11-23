// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.1.5 Operational mapping: the DCPS entities of a Cache, their
//! creation and activation, and QoS set on them between the two; the
//! DCPS types of the topics (§8.1.4.2, §8.1.4.6).

#![cfg(feature = "dcps")]
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, missing_docs)]

#[path = "support/dds.rs"]
mod dds;

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use dds::{
    Bus, ObjectRecorder, Recorder, delete, enabled, mapping, mono, reference, registered, stays,
    wait_until,
};
use zerodds_dcps::qos::{
    DurabilityKind, DurabilityQosPolicy, PresentationAccessScope, ReliabilityKind,
};
use zerodds_dcps::serialized_type::{DynamicDataReader, DynamicDataWriter, SerializedTypeSupport};
use zerodds_dcps::{DdsError, Entity, Publisher, Subscriber, TopicQos};
use zerodds_dlrl::cache::{
    CacheBase, CacheDescription, CacheFactory, CacheUsage, DcpsState, ObjectHome, ObjectListener,
    ObjectRoot,
};
use zerodds_dlrl::dcps_binding;
use zerodds_dlrl::mapping::Mapping;
use zerodds_dlrl::metamodel::DlrlException;
use zerodds_dlrl::model::{
    AttributeValue, BasicType, ClassModel, Model, OBJECT_ROOT, Relation, SimpleType, Value,
};
use zerodds_dlrl::rows::{Cell, Row};
use zerodds_dlrl::types::DlrlOid;
use zerodds_types::dynamic::DynamicSample;

const ALL: [&str; 4] = ["Radar", "Track", "Part", "Sensor"];

fn writer(p: &Publisher, topic: &str) -> DynamicDataWriter {
    p.lookup_datawriter(topic).expect("writer")
}

fn reader(s: &Subscriber, topic: &str) -> DynamicDataReader {
    s.lookup_datareader(topic).expect("reader")
}

fn topics(side: &dds::Side) -> Vec<String> {
    let mut all = Vec::new();
    for h in &side.homes {
        for t in h.get_all_topic_names().unwrap() {
            if !all.contains(&t) {
                all.push(t);
            }
        }
    }
    all
}

/// §8.1.5.1: every DataWriter and DataReader of a Cache is attached to its
/// one Publisher and Subscriber, one per topic of each registered class.
#[test]
fn dlrl_8_1_5_1_all_writers_and_readers_of_a_cache_belong_to_its_one_publisher_and_subscriber() {
    let bus = Bus::new();
    let m = mapping();
    let side = registered(&bus, CacheUsage::ReadWrite, &m, &ALL);
    let publisher = side.cache.the_publisher().unwrap();
    let subscriber = side.cache.the_subscriber().unwrap();
    let names = topics(&side);
    assert_eq!(
        names,
        vec![
            "Radar",
            "Track",
            "Track.comments",
            "Part",
            "Sensor",
            "Sensor.parts"
        ]
    );
    for t in &names {
        let w = writer(&publisher, t);
        let r = reader(&subscriber, t);
        assert_eq!(w.topic().name(), t.as_str());
        assert_eq!(r.topic().name(), t.as_str());
    }
    // Each attribute's topic, as the home names it.
    let track = side.home("Track");
    assert_eq!(track.get_topic_name("x").unwrap(), "Track");
    assert_eq!(track.get_topic_name("comments").unwrap(), "Track.comments");
    assert_eq!(track.get_topic_name("a_radar").unwrap(), "Track");
    // The Publisher and Subscriber are the Cache's own: a second Cache on
    // the same participant has others.
    let other = CacheFactory::get_instance()
        .create_cache(
            CacheUsage::ReadWrite,
            CacheDescription {
                name: format!("other-{}", bus.domain),
                domain: side.participant.clone(),
            },
        )
        .unwrap();
    assert_ne!(
        other.the_publisher().unwrap().get_instance_handle(),
        publisher.get_instance_handle()
    );
    assert_ne!(
        other.the_subscriber().unwrap().get_instance_handle(),
        subscriber.get_instance_handle()
    );
    CacheFactory::get_instance().delete_cache(&other).unwrap();
    delete(side);
}

/// `Fix { v, site -> Site }`, `Site { name }`: the class that refers sorts
/// before its target, so a CacheAccess writes the rows of a Fix before
/// those of its Site.
fn fix_mapping() -> Arc<Mapping> {
    let model = Model::new(vec![
        ClassModel::new("Fix")
            .base(OBJECT_ROOT)
            .mono("v", SimpleType::Basic(BasicType::Double))
            .relation(Relation::to_one("site", "Site")),
        ClassModel::new("Site")
            .base(OBJECT_ROOT)
            .mono("name", SimpleType::Basic(BasicType::String)),
    ])
    .unwrap();
    Arc::new(Mapping::default_for(model).unwrap())
}

fn group_coherent_publisher(p: &Publisher) {
    let mut qos = p.get_qos();
    qos.presentation.access_scope = PresentationAccessScope::Group;
    qos.presentation.coherent_access = true;
    p.set_qos(qos).unwrap();
}

fn group_coherent_subscriber(s: &Subscriber) {
    let mut qos = s.get_qos();
    qos.presentation.access_scope = PresentationAccessScope::Group;
    qos.presentation.coherent_access = true;
    s.set_qos(qos).unwrap();
}

/// Notes, at the moment a Fix is reported created, whether its relation
/// to its Site can be navigated.
#[derive(Default)]
struct Navigates {
    results: Mutex<Vec<bool>>,
}

impl ObjectListener for Navigates {
    fn on_object_created(&self, o: &ObjectRoot) -> bool {
        let ok = matches!(o.get_reference("site"), Ok(Some(_)));
        self.results.lock().unwrap().push(ok);
        true
    }
    fn on_object_modified(&self, _o: &ObjectRoot) -> bool {
        true
    }
    fn on_object_deleted(&self, _o: &ObjectRoot) -> bool {
        true
    }
}

/// §8.1.5.1, §8.1.5.3, §8.1.6.1.1: a graph of related objects, created in
/// one CacheAccess and written through the one Publisher, arrives in one
/// update round: "when a relation is set to a newly created object, set
/// up of the relation and the object creation are simultaneously
/// performed". PRESENTATION GROUP with `coherent_access` is set on the
/// Publisher and the Subscriber between register_all_for_pubsub and
/// enable_all_for_pubsub; each Fix is written before its Site, and when
/// a Fix is reported created, its Site is there.
#[test]
fn dlrl_8_1_5_1_a_related_graph_arrives_in_one_update_round() {
    const GRAPHS: usize = 20;
    let bus = Bus::new();
    let m = fix_mapping();
    let reader_side = registered(&bus, CacheUsage::ReadOnly, &m, &["Fix", "Site"]);
    let writer_side = registered(&bus, CacheUsage::WriteOnly, &m, &["Fix", "Site"]);
    let publisher = writer_side.cache.the_publisher().unwrap();
    let subscriber = reader_side.cache.the_subscriber().unwrap();
    group_coherent_publisher(&publisher);
    group_coherent_subscriber(&subscriber);
    reader_side.cache.enable_all_for_pubsub().unwrap();
    writer_side.cache.enable_all_for_pubsub().unwrap();

    let rounds = Arc::new(Recorder::default());
    reader_side.cache.attach_listener(rounds.clone()).unwrap();
    for class in ["Fix", "Site"] {
        reader_side.home(class).attach_listener(
            Arc::new(ObjectRecorder {
                tag: "o",
                answer: true,
                watched: vec![],
                sink: rounds.clone(),
            }),
            false,
        );
    }
    let navigates = Arc::new(Navigates::default());
    reader_side
        .home("Fix")
        .attach_listener(navigates.clone(), false);
    for t in ["Fix", "Site"] {
        let w = writer(&publisher, t);
        wait_until("the subscription", || {
            !w.get_matched_subscriptions().unwrap().is_empty()
        });
    }

    let access = writer_side
        .cache
        .create_access(CacheUsage::WriteOnly)
        .unwrap();
    let mut last = None;
    for i in 0..GRAPHS {
        let site = writer_side.home("Site").create_object(&access).unwrap();
        site.set("name", mono(Value::String(format!("s{i}"))))
            .unwrap();
        let fix = writer_side.home("Fix").create_object(&access).unwrap();
        fix.set("v", mono(Value::Double(i as f64))).unwrap();
        fix.set("site", reference(&site)).unwrap();
        access.write().unwrap();
        last = Some((fix, site));
    }

    wait_until("every Fix", || {
        navigates.results.lock().unwrap().len() == GRAPHS
    });
    let results = navigates.results.lock().unwrap().clone();
    assert!(
        results.iter().all(|ok| *ok),
        "a Fix without its Site: {results:?}"
    );
    // Each round has as many Sites as Fixes created.
    let events = rounds.snapshot();
    let mut fixes = 0usize;
    let mut sites = 0usize;
    for e in &events {
        match e.as_str() {
            "begin" => (fixes, sites) = (0, 0),
            "o created Fix" => fixes += 1,
            "o created Site" => sites += 1,
            "end" => assert_eq!(fixes, sites, "one round: {events:?}"),
            _ => {}
        }
    }
    let (fix, site) = last.unwrap();
    let cache = CacheBase::Cache(reader_side.cache.clone());
    let got = reader_side
        .home("Fix")
        .find_object(fix.oid(), &cache)
        .unwrap();
    let r = got.get_reference("site").unwrap().unwrap();
    assert_eq!(r.oid(), site.oid());
    assert_eq!(
        r.get("name").unwrap(),
        mono(Value::String(format!("s{}", GRAPHS - 1)))
    );
    delete(writer_side);
    delete(reader_side);
}

/// §8.1.5.2: register_all_for_pubsub creates the DCPS entities, not
/// enabled: no subscription is made until enable_all_for_pubsub.
#[test]
fn dlrl_8_1_5_2_register_creates_and_enable_activates_the_dcps_entities() {
    let bus = Bus::new();
    let m = mapping();
    // A publishing Cache that is already enabled.
    let publishing = enabled(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let track_writer = writer(&publishing.cache.the_publisher().unwrap(), "Track");

    // The preconditions on the way.
    let participant = bus.participant();
    let cache = CacheFactory::get_instance()
        .create_cache(
            CacheUsage::ReadOnly,
            CacheDescription {
                name: format!("sub-{}", bus.domain),
                domain: participant.clone(),
            },
        )
        .unwrap();
    assert_eq!(cache.pubsub_state(), DcpsState::Initial);
    assert!(matches!(
        cache.register_all_for_pubsub(),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        cache.enable_all_for_pubsub(),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    let homes: Vec<ObjectHome> = ALL
        .iter()
        .map(|c| ObjectHome::new(Arc::clone(&m), c).unwrap())
        .collect();
    // Track refers to Radar: without Radar's home the definition is bad.
    cache.register_home(&homes[1]).unwrap();
    assert!(matches!(
        cache.register_all_for_pubsub(),
        Err(DlrlException::BadHomeDefinition { .. })
    ));
    assert!(matches!(
        cache.register_home(&homes[1]),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    for h in [&homes[0], &homes[2], &homes[3]] {
        cache.register_home(h).unwrap();
    }
    assert_eq!(cache.find_home_by_index(0).unwrap().unwrap(), homes[1]);
    assert_eq!(cache.find_home_by_name("Radar").unwrap().unwrap(), homes[0]);
    assert!(matches!(
        homes[1].get_topic_name("x"),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    cache.register_all_for_pubsub().unwrap();
    cache.register_all_for_pubsub().unwrap(); // a no-op
    assert_eq!(cache.pubsub_state(), DcpsState::Registered);
    let late = ObjectHome::new(Arc::clone(&m), "Radar").unwrap();
    assert!(matches!(
        cache.register_home(&late),
        Err(DlrlException::PreconditionNotMet { .. })
    ));

    // Created, not enabled: the publishing side sees no subscription.
    let track_reader = reader(&cache.the_subscriber().unwrap(), "Track");
    assert!(!track_reader.is_enabled());
    stays(
        "no subscription before enable",
        Duration::from_millis(800),
        || track_writer.get_matched_subscriptions().unwrap().is_empty(),
    );
    cache.enable_all_for_pubsub().unwrap();
    cache.enable_all_for_pubsub().unwrap(); // a no-op
    assert_eq!(cache.pubsub_state(), DcpsState::Enabled);
    assert!(track_reader.is_enabled());
    wait_until("the subscription", || {
        !track_writer.get_matched_subscriptions().unwrap().is_empty()
    });
    assert!(matches!(
        cache.register_all_for_pubsub(),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    CacheFactory::get_instance().delete_cache(&cache).unwrap();
    delete(publishing);
}

/// §8.1.5.2: a `register_all_for_pubsub` that fails leaves no DCPS
/// entity of its own and the state INITIAL; called again, it fails the
/// same way. Once with a topic whose fields are no IDL identifiers (the
/// field names of the table of §8.2.3.4, `RADAR-OID`), refused before
/// the first entity; once with a topic that DCPS refuses after the
/// topics before it were created (a topic of that name, of another type
/// and with other QoS, exists in the participant), whose entities are
/// deleted again. With
/// that topic gone, the call succeeds; `delete_cache` leaves no topic.
#[test]
fn dlrl_8_1_5_2_a_refused_topic_leaves_no_dcps_entity() {
    let bus = Bus::new();
    let participant = bus.participant();
    let make = |n: u32| {
        CacheFactory::get_instance()
            .create_cache(
                CacheUsage::ReadWrite,
                CacheDescription {
                    name: format!("refused-{}-{}-{n}", std::process::id(), bus.domain),
                    domain: participant.clone(),
                },
            )
            .unwrap()
    };
    let endpoints_gone = |cache: &zerodds_dlrl::cache::Cache, m: &Mapping| {
        let publisher = cache.the_publisher().unwrap();
        let subscriber = cache.the_subscriber().unwrap();
        for t in m.topics() {
            assert!(
                publisher
                    .lookup_datawriter::<DynamicSample>(&t.name)
                    .is_none(),
                "the DataWriter of {} is left",
                t.name
            );
            assert!(
                subscriber
                    .lookup_datareader::<DynamicSample>(&t.name)
                    .is_none(),
                "the DataReader of {} is left",
                t.name
            );
        }
    };
    let no_entity_of = |cache: &zerodds_dlrl::cache::Cache, m: &Mapping| {
        endpoints_gone(cache, m);
        for t in m.topics() {
            assert!(
                participant.lookup_topicdescription(&t.name).is_none(),
                "topic {} is left",
                t.name
            );
        }
    };

    // Refused by the DCPS binding.
    let example = Arc::new(Mapping::new(common::example_model(), &common::example_tags()).unwrap());
    let cache = make(1);
    for c in ["Track", "Track3D", "Radar"] {
        cache
            .register_home(&ObjectHome::new(Arc::clone(&example), c).unwrap())
            .unwrap();
    }
    for _ in 0..2 {
        match cache.register_all_for_pubsub() {
            Err(DlrlException::DcpsError { message }) => {
                assert!(message.contains("RADAR-OID"), "{message}");
            }
            other => panic!("expected DCPSError, got {other:?}"),
        }
        assert_eq!(cache.pubsub_state(), DcpsState::Initial);
        no_entity_of(&cache, &example);
    }
    CacheFactory::get_instance().delete_cache(&cache).unwrap();

    // Refused by DCPS: a topic Sensor.parts of another type (that of the
    // topic Radar) with other QoS exists (DDS-XTypes 1.3 §7.6.4.1: the
    // objects of one topic name have consistent QoS). The topics of the
    // homes Radar, Track and Part come before it.
    let m = mapping();
    let radar = m.topic("Radar").unwrap();
    let support =
        SerializedTypeSupport::create_type_support(&dcps_binding::topic_type(radar).unwrap())
            .unwrap();
    let mut qos = TopicQos::default();
    qos.durability.kind = DurabilityKind::TransientLocal;
    let squatter = participant
        .create_dynamic_topic("Sensor.parts", qos, support)
        .unwrap();
    let cache = make(2);
    for c in ALL {
        cache
            .register_home(&ObjectHome::new(Arc::clone(&m), c).unwrap())
            .unwrap();
    }
    for _ in 0..2 {
        assert!(
            matches!(
                cache.register_all_for_pubsub(),
                Err(DlrlException::DcpsError { .. })
            ),
            "the topic Sensor.parts is refused"
        );
        assert_eq!(cache.pubsub_state(), DcpsState::Initial);
        endpoints_gone(&cache, &m);
        let mut left: Vec<String> = m
            .topics()
            .iter()
            .filter(|t| participant.lookup_topicdescription(&t.name).is_some())
            .map(|t| t.name.clone())
            .collect();
        left.sort();
        assert_eq!(
            left,
            ["Sensor.parts"],
            "only the topic created before is left"
        );
    }
    participant.delete_topic(&squatter).unwrap();
    cache.register_all_for_pubsub().unwrap();
    assert_eq!(cache.pubsub_state(), DcpsState::Registered);
    CacheFactory::get_instance().delete_cache(&cache).unwrap();
    for t in m.topics() {
        assert!(
            participant.lookup_topicdescription(&t.name).is_none(),
            "topic {} is left after delete_cache",
            t.name
        );
    }
}

/// §8.1.5.2, §8.1.6.3.1: the cache usage decides whether a Publisher, a
/// Subscriber or both are created, and which CacheAccess may write.
#[test]
fn dlrl_8_1_5_2_cache_usage_decides_publisher_subscriber_or_both() {
    let bus = Bus::new();
    let m = mapping();
    let read_only = registered(&bus, CacheUsage::ReadOnly, &m, &ALL);
    assert!(read_only.cache.the_publisher().is_none());
    let sub = read_only.cache.the_subscriber().unwrap();
    assert!(
        sub.lookup_datareader::<zerodds_types::dynamic::DynamicSample>("Track")
            .is_some()
    );
    assert!(matches!(
        read_only.cache.create_access(CacheUsage::ReadWrite),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    assert!(read_only.cache.create_access(CacheUsage::ReadOnly).is_ok());

    let write_only = registered(&bus, CacheUsage::WriteOnly, &m, &ALL);
    assert!(write_only.cache.the_subscriber().is_none());
    let publ = write_only.cache.the_publisher().unwrap();
    assert!(
        publ.lookup_datawriter::<zerodds_types::dynamic::DynamicSample>("Track")
            .is_some()
    );
    assert!(
        write_only
            .cache
            .create_access(CacheUsage::WriteOnly)
            .is_ok()
    );

    let both = registered(&bus, CacheUsage::ReadWrite, &m, &ALL);
    assert!(both.cache.the_publisher().is_some());
    assert!(both.cache.the_subscriber().is_some());

    // A second Cache of one name is refused; find and delete.
    let factory = CacheFactory::get_instance();
    assert!(matches!(
        factory.create_cache(
            CacheUsage::ReadOnly,
            CacheDescription {
                name: both.cache.name().into(),
                domain: both.participant.clone(),
            },
        ),
        Err(DlrlException::AlreadyExisting { .. })
    ));
    assert_eq!(
        factory.find_cache_by_name(both.cache.name()).unwrap(),
        both.cache
    );
    let name = both.cache.name().to_string();
    delete(both);
    assert!(factory.find_cache_by_name(&name).is_none());
    delete(read_only);
    delete(write_only);
}

/// §8.1.6.2, §8.2.1.2.1: `write` on a CacheAccess in read mode raises
/// `ReadOnlyMode` with its message, also inside a Cache that may write.
#[test]
fn dlrl_8_1_6_2_write_on_a_read_only_access_raises_read_only_mode() {
    let bus = Bus::new();
    let m = mapping();
    let both = registered(&bus, CacheUsage::ReadWrite, &m, &ALL);
    let reading = both.cache.create_access(CacheUsage::ReadOnly).unwrap();
    match reading.write() {
        Err(e @ DlrlException::ReadOnlyMode { .. }) => {
            assert_eq!(e.repository_id(), "IDL:DDS/ReadOnlyMode:1.0");
            assert!(e.message().contains("READ_ONLY"), "{e}");
        }
        other => panic!("expected ReadOnlyMode, got {other:?}"),
    }
    let writing = both.cache.create_access(CacheUsage::ReadWrite).unwrap();
    assert!(!matches!(
        writing.write(),
        Err(DlrlException::ReadOnlyMode { .. })
    ));
    delete(both);
}

/// §8.1.5.3: QoS set on a DataWriter between register_all_for_pubsub and
/// enable_all_for_pubsub is the QoS it is enabled with; after enable an
/// immutable policy no longer changes.
#[test]
fn dlrl_8_1_5_3_qos_set_between_register_and_enable_reaches_the_writer() {
    let bus = Bus::new();
    let m = mapping();
    let side = registered(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let w = writer(&side.cache.the_publisher().unwrap(), "Track");
    let mut qos = w.get_qos();
    qos.durability = DurabilityQosPolicy {
        kind: DurabilityKind::TransientLocal,
    };
    w.set_qos(qos.clone()).unwrap();
    side.cache.enable_all_for_pubsub().unwrap();
    assert_eq!(w.get_qos().durability.kind, DurabilityKind::TransientLocal);
    qos.durability = DurabilityQosPolicy {
        kind: DurabilityKind::Volatile,
    };
    assert!(w.set_qos(qos).is_err());
    delete(side);
}

/// A read-write Cache named `name` (with the bus's domain) on
/// `participant`, and a home of `Radar` of the mapping `m`.
fn radar_cache(
    bus: &Bus,
    participant: &zerodds_dcps::DomainParticipant,
    m: &Arc<Mapping>,
    name: &str,
) -> (zerodds_dlrl::cache::Cache, ObjectHome) {
    let cache = CacheFactory::get_instance()
        .create_cache(
            CacheUsage::ReadWrite,
            CacheDescription {
                name: format!("{name}-{}", bus.domain),
                domain: participant.clone(),
            },
        )
        .unwrap();
    let radar = ObjectHome::new(Arc::clone(m), "Radar").unwrap();
    (cache, radar)
}

/// Whether `r` is PRECONDITION_NOT_MET.
fn precondition(r: Result<(), DlrlException>) -> bool {
    matches!(r, Err(DlrlException::PreconditionNotMet { .. }))
}

/// A topic QoS with TRANSIENT_LOCAL durability.
fn transient_local() -> TopicQos {
    TopicQos {
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::TransientLocal,
        },
        ..TopicQos::default()
    }
}

/// §8.1.5.3: a Topic is enabled when it is created, so its QoS is given
/// before register_all_for_pubsub (`set_topic_qos`): the Topic is created
/// with it. Only in the DCPS state INITIAL, only for a topic of the
/// classes of the registered homes, only a consistent QoS. A second Cache
/// on the participant uses the Topic when it asks for the same QoS, is
/// refused when it asks for another one, and uses it as it is when it asks
/// for none.
#[test]
fn dlrl_8_1_5_3_the_qos_of_a_topic_is_given_before_register_all_for_pubsub() {
    let bus = Bus::new();
    let m = mapping();
    let participant = bus.participant();
    let cache_with_radar = |name: &str| radar_cache(&bus, &participant, &m, name);
    let durable = transient_local();
    let (cache, radar) = cache_with_radar("topic-qos");
    // No home is registered: no topic is known yet.
    assert!(precondition(cache.set_topic_qos("Radar", durable.clone())));
    cache.register_home(&radar).unwrap();
    // A topic of a class whose home is not registered.
    assert!(precondition(cache.set_topic_qos("Track", durable.clone())));
    let mut inconsistent = TopicQos::default();
    inconsistent.history.depth = 0;
    assert!(matches!(
        cache.set_topic_qos("Radar", inconsistent),
        Err(DlrlException::BadParameter { .. })
    ));
    cache.set_topic_qos("Radar", durable.clone()).unwrap();
    cache.register_all_for_pubsub().unwrap();
    let w = writer(&cache.the_publisher().unwrap(), "Radar");
    assert_eq!(w.topic().qos(), durable);
    assert!(precondition(
        cache.set_topic_qos("Radar", TopicQos::default())
    ));

    let (same, radar) = cache_with_radar("topic-qos-same");
    same.register_home(&radar).unwrap();
    same.set_topic_qos("Radar", durable.clone()).unwrap();
    same.register_all_for_pubsub().unwrap();
    let w = writer(&same.the_publisher().unwrap(), "Radar");
    assert_eq!(w.topic().qos(), durable);

    let (other, radar) = cache_with_radar("topic-qos-other");
    other.register_home(&radar).unwrap();
    other.set_topic_qos("Radar", TopicQos::default()).unwrap();
    assert!(matches!(
        other.register_all_for_pubsub(),
        Err(DlrlException::DcpsError { .. })
    ));
    assert_eq!(other.pubsub_state(), DcpsState::Initial);
    // Refused, the Cache gave its share of the Topic back: the two others
    // still hold it.
    assert_eq!(participant.topic_users("Radar").unwrap().data_writers, 2);

    let (plain, radar) = cache_with_radar("topic-qos-plain");
    plain.register_home(&radar).unwrap();
    plain.register_all_for_pubsub().unwrap();
    let w = writer(&plain.the_publisher().unwrap(), "Radar");
    assert_eq!(w.topic().qos(), durable);
    for c in [&cache, &same, &other, &plain] {
        CacheFactory::get_instance().delete_cache(c).unwrap();
    }
    assert_eq!(participant.topics_len(), 0);
}

/// §8.1.5.3: `set_new_topic_qos` gives the QoS of a Topic that
/// register_all_for_pubsub creates. A Topic of that name and type the
/// DomainParticipant has already is used with the QoS it has, where
/// `set_topic_qos` with the same QoS is refused. The preconditions are
/// those of `set_topic_qos`; a later call for the topic, of either
/// operation, replaces an earlier one.
#[test]
fn dlrl_8_1_5_3_a_qos_for_a_new_topic_leaves_a_topic_that_exists_as_it_is() {
    let bus = Bus::new();
    let m = mapping();
    let participant = bus.participant();
    let cache_with_radar = |name: &str| radar_cache(&bus, &participant, &m, name);
    let durable = transient_local();
    let (first, radar) = cache_with_radar("new-topic-qos");
    // No home is registered: no topic is known yet.
    assert!(precondition(
        first.set_new_topic_qos("Radar", durable.clone())
    ));
    first.register_home(&radar).unwrap();
    // A topic of a class whose home is not registered.
    assert!(precondition(
        first.set_new_topic_qos("Track", durable.clone())
    ));
    let mut inconsistent = TopicQos::default();
    inconsistent.history.depth = 0;
    assert!(matches!(
        first.set_new_topic_qos("Radar", inconsistent),
        Err(DlrlException::BadParameter { .. })
    ));
    // The later call gives the QoS of the Topic that is created.
    first.set_topic_qos("Radar", TopicQos::default()).unwrap();
    first.set_new_topic_qos("Radar", durable.clone()).unwrap();
    first.register_all_for_pubsub().unwrap();
    let w = writer(&first.the_publisher().unwrap(), "Radar");
    assert_eq!(w.topic().qos(), durable);
    assert!(precondition(
        first.set_new_topic_qos("Radar", TopicQos::default())
    ));

    // The Topic exists TRANSIENT_LOCAL: the default QoS for a new topic
    // leaves it as it is.
    let (second, radar) = cache_with_radar("new-topic-qos-exists");
    second.register_home(&radar).unwrap();
    second
        .set_new_topic_qos("Radar", TopicQos::default())
        .unwrap();
    second.register_all_for_pubsub().unwrap();
    let w = writer(&second.the_publisher().unwrap(), "Radar");
    assert_eq!(w.topic().qos(), durable);
    assert_eq!(participant.topic_users("Radar").unwrap().data_writers, 2);

    // set_topic_qos after set_new_topic_qos: the default QoS is required,
    // and the Topic that exists is refused.
    let (third, radar) = cache_with_radar("new-topic-qos-required");
    third.register_home(&radar).unwrap();
    third
        .set_new_topic_qos("Radar", TopicQos::default())
        .unwrap();
    third.set_topic_qos("Radar", TopicQos::default()).unwrap();
    assert!(matches!(
        third.register_all_for_pubsub(),
        Err(DlrlException::DcpsError { .. })
    ));
    assert_eq!(third.pubsub_state(), DcpsState::Initial);
    assert_eq!(participant.topic_users("Radar").unwrap().data_writers, 2);
    for c in [&first, &second, &third] {
        CacheFactory::get_instance().delete_cache(c).unwrap();
    }
    assert_eq!(participant.topics_len(), 0);
}

/// §8.1.5.3: the Publisher and the Subscriber of a Cache are not enabled
/// until enable_all_for_pubsub, so a policy that is immutable once enabled
/// (PRESENTATION) is set between register_all_for_pubsub and
/// enable_all_for_pubsub; afterwards it no longer changes.
#[test]
fn dlrl_8_1_5_3_presentation_set_between_register_and_enable_reaches_publisher_and_subscriber() {
    let bus = Bus::new();
    let m = mapping();
    let side = registered(&bus, CacheUsage::ReadWrite, &m, &ALL);
    let publisher = side.cache.the_publisher().unwrap();
    let subscriber = side.cache.the_subscriber().unwrap();
    assert!(!publisher.is_enabled());
    assert!(!subscriber.is_enabled());
    group_coherent_publisher(&publisher);
    group_coherent_subscriber(&subscriber);
    side.cache.enable_all_for_pubsub().unwrap();
    assert!(publisher.is_enabled());
    assert!(subscriber.is_enabled());
    assert!(writer(&publisher, "Track").is_enabled());
    assert!(reader(&subscriber, "Track").is_enabled());
    let pq = publisher.get_qos();
    assert_eq!(pq.presentation.access_scope, PresentationAccessScope::Group);
    assert!(pq.presentation.coherent_access);
    let sq = subscriber.get_qos();
    assert_eq!(sq.presentation.access_scope, PresentationAccessScope::Group);
    assert!(sq.presentation.coherent_access);
    let mut back = pq;
    back.presentation.coherent_access = false;
    assert!(matches!(
        publisher.set_qos(back),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    let mut back = sq;
    back.presentation.coherent_access = false;
    assert!(matches!(
        subscriber.set_qos(back),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    delete(side);
}

/// §8.1.5.3: two attributes of one object with different QoS: `x` in
/// topic Track, TRANSIENT_LOCAL; `comments` in topic Track.comments,
/// VOLATILE. A Cache that joins later gets the object with `x`, without
/// the comments.
#[test]
fn dlrl_8_1_5_3_two_attributes_of_one_object_take_different_qos() {
    let bus = Bus::new();
    let m = mapping();
    let side = registered(&bus, CacheUsage::WriteOnly, &m, &ALL);
    let publisher = side.cache.the_publisher().unwrap();
    for t in ["Track", "Radar"] {
        let w = writer(&publisher, t);
        let mut qos = w.get_qos();
        qos.durability.kind = DurabilityKind::TransientLocal;
        w.set_qos(qos).unwrap();
    }
    side.cache.enable_all_for_pubsub().unwrap();
    let access = side.cache.create_access(CacheUsage::WriteOnly).unwrap();
    let t = side.home("Track").create_object(&access).unwrap();
    t.set("x", mono(Value::Double(7.0))).unwrap();
    t.set("y", mono(Value::Double(8.0))).unwrap();
    t.set("a_radar", mono(Value::Ref(None))).unwrap();
    let mut comments = std::collections::BTreeMap::new();
    comments.insert("k".to_string(), Value::String("v".into()));
    t.set("comments", AttributeValue::StrMap(comments)).unwrap();
    access.write().unwrap();

    // Joins after the write.
    let late = registered(&bus, CacheUsage::ReadOnly, &m, &ALL);
    let sub = late.cache.the_subscriber().unwrap();
    for name in ["Track", "Radar"] {
        let r = reader(&sub, name);
        let mut qos = r.get_qos();
        qos.durability.kind = DurabilityKind::TransientLocal;
        assert_eq!(qos.reliability.kind, ReliabilityKind::Reliable);
        r.set_qos(qos).unwrap();
    }
    late.cache.enable_all_for_pubsub().unwrap();
    let base = CacheBase::Cache(late.cache.clone());
    wait_until("the durable part of the track", || {
        late.home("Track").find_object(t.oid(), &base).is_ok()
    });
    let got = late.home("Track").find_object(t.oid(), &base).unwrap();
    assert_eq!(dds::dbl(&got, "x"), 7.0);
    // The volatile part was not kept for the late joiner.
    let comments_reader = reader(&sub, "Track.comments");
    stays(
        "no comments",
        Duration::from_millis(500),
        || matches!(got.get("comments"), Ok(AttributeValue::StrMap(m)) if m.is_empty()),
    );
    assert_eq!(
        comments_reader.get_qos().durability.kind,
        DurabilityKind::Volatile
    );
    delete(late);
    delete(side);
}

/// §8.1.4.2, §8.1.4.6: the DCPS type of a topic: one member per field,
/// key fields `@key`, a reference as `DDS::DLRLOid`; a row is one sample
/// and reads back unchanged.
#[test]
fn dlrl_8_1_4_6_the_dcps_type_of_each_topic_carries_its_rows() {
    let m = mapping();
    let track = m.topic("Track").unwrap();
    let idl = dcps_binding::dcps_description(m.topics()).unwrap();
    assert!(idl.contains(
        "module DDS { @final struct DLRLOid { long value0; long value1; long value2; }; };"
    ));
    assert!(
        idl.contains("@final struct Track { @key DDS::DLRLOid oid; double x; double y; DDS::DLRLOid a_radar; };"),
        "{idl}"
    );
    assert!(
        idl.contains("@final struct Track_comments { @key DDS::DLRLOid oid; @key string index; string comments; };"),
        "{idl}"
    );
    let ty = dcps_binding::topic_type(track).unwrap();
    let row = Row {
        topic: "Track".into(),
        cells: vec![
            ("oid".into(), Cell::Oid(DlrlOid::new(1, 2, 3))),
            ("x".into(), Cell::Double(0.5)),
            ("y".into(), Cell::Double(-1.0)),
            ("a_radar".into(), Cell::Oid(DlrlOid::new(4, 5, 6))),
        ],
    };
    let sample = dcps_binding::row_to_sample(track, &ty, &row).unwrap();
    assert_eq!(
        dcps_binding::sample_to_row(track, &sample, false).unwrap(),
        row
    );
    let key_only = dcps_binding::sample_to_row(track, &sample, true).unwrap();
    assert_eq!(
        key_only.cells,
        vec![("oid".to_string(), Cell::Oid(DlrlOid::new(1, 2, 3)))]
    );
    // A cell of the wrong type, a field the topic does not have.
    let mut bad = row.clone();
    bad.cells[1].1 = Cell::Long(1);
    assert!(dcps_binding::row_to_sample(track, &ty, &bad).is_err());
    let mut extra = row;
    extra.cells.push(("z".into(), Cell::Double(0.0)));
    assert!(dcps_binding::row_to_sample(track, &ty, &extra).is_err());
}
