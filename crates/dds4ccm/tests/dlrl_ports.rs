// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS4CCM 1.1 §8: the DDS-DLRL ports and connectors. The interface
//! `CacheOperation` (§8.2.1.1), the home receptacles (§8.2.1.2), the
//! composition rule (§8.2.2), the DLRL connector (§8.3) and the DCPS
//! entities of a DLRL port (§8.4.1), over the DLRL entities of
//! `zerodds-dlrl`, with two containers that meet through DDS.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use common::{Deployment, Statuses, Track, eventually, isolated_cfg, profile};
use zerodds_dcps::psm_constants::status as status_kind;
use zerodds_dcps::qos::{
    DeadlineQosPolicy, DurabilityKind, HistoryKind, HistoryQosPolicy, ReliabilityKind,
    ReliabilityQosPolicy,
};
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, Entity, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_dds4ccm::dlrl::{
    Cache, CacheListener, CacheUsage, DlrlException, Mapping, ObjectHome, ObjectRoot,
};
use zerodds_dds4ccm::{
    CacheOperation, ConnectorError, ConnectorStatusListener, Container, DeclarationError,
    DlrlConnector, DlrlPort, DlrlPortType, HomeReceptacle, NonChangeable,
};
use zerodds_dlrl::cache::{CacheBase, DcpsState, ObjectListener};
use zerodds_dlrl::dcps_binding;
use zerodds_dlrl::generation::{MemberDecl, ModelDescription, TypeRef, ValuetypeDecl, generate};
use zerodds_dlrl::model::{AttributeValue, BasicType, ObjectRef, Value};
use zerodds_dlrl::model_tags::parse_model_tags;
use zerodds_types::dynamic::DynamicSample;

/// The object model of the tests, made by the DLRL generator of
/// `zerodds-dlrl` (DDS 1.2 §8.2.2.1) from a model description and model
/// tags without class mapping (the default mapping): `Radar { name }`,
/// `Track { x, y, a_radar -> Radar }`, on the topics `Radar` and `Track`.
fn mapping() -> Arc<Mapping> {
    let member = |name: &str, ty: TypeRef| MemberDecl {
        name: name.into(),
        ty,
    };
    let description = ModelDescription {
        valuetypes: vec![
            ValuetypeDecl {
                name: "Radar".into(),
                bases: vec!["DDS::ObjectRoot".into()],
                members: vec![member("name", TypeRef::Basic(BasicType::String))],
            },
            ValuetypeDecl {
                name: "Track".into(),
                bases: vec!["DDS::ObjectRoot".into()],
                members: vec![
                    member("x", TypeRef::Basic(BasicType::Double)),
                    member("y", TypeRef::Basic(BasicType::Double)),
                    member("a_radar", TypeRef::Valuetype("Radar".into())),
                ],
            },
        ],
        types: vec![],
    };
    let tags = parse_model_tags(br#"<?xml version="1.0"?><Dlrl name="Tracking"/>"#).unwrap();
    Arc::new(generate(&description, &tags).unwrap())
}

/// `porttype RadarTrackPort { uses CCM_DDS::CacheOperation cache; uses
/// RadarHome radar_home; uses TrackHome track_home; }`.
fn port_type(mapping: &Arc<Mapping>) -> DlrlPortType {
    DlrlPortType::new(
        "RadarTrackPort",
        Arc::clone(mapping),
        "cache",
        &[("radar_home", "Radar"), ("track_home", "Track")],
    )
    .unwrap()
}

/// A configured connector of `domain` with the profile `qos_profile`.
fn connector(
    domain: i32,
    qos_profile: &str,
    mirror_ports: Vec<(&str, DlrlPortType)>,
) -> DlrlConnector {
    let connector = DlrlConnector::new(mirror_ports).unwrap();
    connector.set_domain_id(domain).unwrap();
    connector.set_qos_profile(qos_profile).unwrap();
    connector.configuration_complete().unwrap();
    connector
}

/// TRANSIENT_LOCAL topics, DataWriters and DataReaders, KEEP_ALL
/// DataWriters, the partition `dlrl` on Publisher and Subscriber: an
/// object written before the reading port exists arrives.
fn durable() -> String {
    const DURABILITY: &str = "<durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>";
    const PARTITION: &str = "<partition><name><element>dlrl</element></name></partition>";
    profile(&format!(
        "<topic_qos>{DURABILITY}</topic_qos>\
         <datawriter_qos>{DURABILITY}<history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos>\
         <datareader_qos>{DURABILITY}</datareader_qos>\
         <publisher_qos>{PARTITION}</publisher_qos>\
         <subscriber_qos>{PARTITION}</subscriber_qos>"
    ))
}

fn mono(value: Value) -> AttributeValue {
    AttributeValue::Mono(value)
}

fn reference(object: &ObjectRoot) -> AttributeValue {
    mono(Value::Ref(Some(ObjectRef::new(
        &object.class_name(),
        object.oid(),
    ))))
}

/// Writes a radar named `name` through `port`.
fn write_radar(port: &DlrlPort, name: &str) -> ObjectRoot {
    let access = port.cache().create_access(CacheUsage::WriteOnly).unwrap();
    let radar = port
        .home("radar_home")
        .unwrap()
        .create_object(&access)
        .unwrap();
    radar.set("name", mono(Value::String(name.into()))).unwrap();
    access.write().unwrap();
    radar
}

fn participant(domain: i32, cfg: &RuntimeConfig) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
        .unwrap()
}

fn delete(participant: &DomainParticipant) {
    let _ = participant.delete_contained_entities();
    let _ = DomainParticipantFactory::instance().delete_participant(participant);
}

/// The calls of a `CacheListener`.
#[derive(Default)]
struct Rounds {
    events: Mutex<Vec<&'static str>>,
}

impl Rounds {
    fn events(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }

    fn note(&self, event: &'static str) {
        self.events.lock().unwrap().push(event);
    }
}

impl CacheListener for Rounds {
    fn on_begin_updates(&self) {
        self.note("begin");
    }

    fn on_end_updates(&self) {
        self.note("end");
    }

    fn on_updates_enabled(&self) {
        self.note("enabled");
    }

    fn on_updates_disabled(&self) {
        self.note("disabled");
    }
}

/// The objects an `ObjectListener` heard of as created.
#[derive(Default)]
struct Created {
    objects: Mutex<Vec<ObjectRoot>>,
}

impl ObjectListener for Created {
    fn on_object_created(&self, the_object: &ObjectRoot) -> bool {
        self.objects.lock().unwrap().push(the_object.clone());
        true
    }

    fn on_object_modified(&self, _the_object: &ObjectRoot) -> bool {
        true
    }

    fn on_object_deleted(&self, _the_object: &ObjectRoot) -> bool {
        true
    }
}

/// §8.2.2: the fragment creates the Cache by `cache_usage`, registers the
/// home of each receptacle, applies the QoS profile and enables the
/// infrastructure, "so that DLRL objects can be created and used the DLRL
/// way". An object graph written through the port of one container
/// arrives as objects through the port of the other, over DDS.
#[test]
fn dds4ccm_8_2_2_an_object_written_through_one_port_arrives_as_an_object_through_the_other() {
    let d = Deployment::new();
    let m = mapping();
    let writing_side = connector(d.domain, &durable(), vec![("p1", port_type(&m))]);
    let reading_side = connector(d.domain, &durable(), vec![("p1", port_type(&m))]);
    let writing = writing_side
        .connect("p1", &d.publisher(), CacheUsage::WriteOnly)
        .unwrap();

    // The Cache of the usage, the homes registered in the order of the
    // receptacles, the DCPS entities enabled.
    let cache = writing_side.cache_of(&writing).unwrap();
    assert_eq!(cache.cache_usage(), CacheUsage::WriteOnly);
    assert_eq!(cache.pubsub_state(), DcpsState::Enabled);
    let classes: Vec<String> = cache
        .homes()
        .iter()
        .map(|home| home.class_name().to_owned())
        .collect();
    assert_eq!(classes, ["Radar", "Track"]);
    let radars = writing.home("radar_home").unwrap();
    let tracks = writing.home("track_home").unwrap();
    assert_eq!(radars.registration_index(), Some(0));
    assert_eq!(tracks.registration_index(), Some(1));

    // The QoS profile: its values, and the Cache's where it is silent.
    let publisher = cache.the_publisher().unwrap();
    assert!(cache.the_subscriber().is_none());
    assert_eq!(publisher.get_qos().partition.names, vec!["dlrl".to_owned()]);
    for topic in ["Radar", "Track"] {
        let writer = publisher.lookup_datawriter::<DynamicSample>(topic).unwrap();
        assert_eq!(
            writer.get_qos().durability.kind,
            DurabilityKind::TransientLocal
        );
        assert_eq!(writer.get_qos().history.kind, HistoryKind::KeepAll);
        assert_eq!(
            writer.topic().qos().durability.kind,
            DurabilityKind::TransientLocal
        );
    }

    // Written before the reading port exists: the TRANSIENT_LOCAL of the
    // profile brings it to the late reader.
    let access = writing
        .cache()
        .create_access(CacheUsage::WriteOnly)
        .unwrap();
    let radar = radars.create_object(&access).unwrap();
    radar
        .set("name", mono(Value::String("north".into())))
        .unwrap();
    let track = tracks.create_object(&access).unwrap();
    track.set("x", mono(Value::Double(1.5))).unwrap();
    track.set("y", mono(Value::Double(-2.0))).unwrap();
    track.set("a_radar", reference(&radar)).unwrap();
    access.write().unwrap();

    let reading = reading_side
        .connect("p1", &d.subscriber(), CacheUsage::ReadOnly)
        .unwrap();
    let reading_cache = reading_side.cache_of(&reading).unwrap();
    assert!(reading_cache.the_publisher().is_none());
    let subscriber = reading_cache.the_subscriber().unwrap();
    assert_eq!(
        subscriber.get_qos().partition.names,
        vec!["dlrl".to_owned()]
    );
    let reader = subscriber
        .lookup_datareader::<DynamicSample>("Track")
        .unwrap();
    assert_eq!(
        reader.get_qos().durability.kind,
        DurabilityKind::TransientLocal
    );
    assert_eq!(reader.get_qos().reliability.kind, ReliabilityKind::Reliable);

    let objects = reading.cache();
    eventually("the track and its radar arrive", || {
        objects.objects().len() == 2
    });
    let arrived = objects.objects();
    let of_class = |class: &str| {
        arrived
            .iter()
            .find(|o| o.class_name() == class)
            .cloned()
            .unwrap()
    };
    let (got_radar, got_track) = (of_class("Radar"), of_class("Track"));
    assert_eq!(got_radar.oid(), radar.oid());
    assert_eq!(got_track.oid(), track.oid());
    assert_eq!(
        got_radar.get("name").unwrap(),
        mono(Value::String("north".into()))
    );
    assert_eq!(got_track.get("x").unwrap(), mono(Value::Double(1.5)));
    assert_eq!(got_track.get("y").unwrap(), mono(Value::Double(-2.0)));
    assert_eq!(got_track.get("a_radar").unwrap(), reference(&radar));
    assert_eq!(got_track.object_home().as_ref(), reading.home("track_home"));

    writing_side.remove().unwrap();
    reading_side.remove().unwrap();
}

/// §8.2.1.1: `CacheOperation` is "only the operative subset of the
/// DDS::Cache functions and attributes", which "mimic exactly the
/// DDS::Cache ones": the attributes are those of the Cache of the port,
/// `refresh` applies the updates of the manual mode, the listener, update
/// and access management act on the Cache with its exceptions; after the
/// connector is removed every operation raises `AlreadyDeleted`.
#[test]
fn dds4ccm_8_2_1_1_cache_operation_is_the_operative_part_of_the_cache_of_the_port() {
    let d = Deployment::new();
    let m = mapping();
    let writing_side = connector(d.domain, &durable(), vec![("p1", port_type(&m))]);
    let reading_side = connector(d.domain, &durable(), vec![("p1", port_type(&m))]);
    let writing = writing_side
        .connect("p1", &d.publisher(), CacheUsage::ReadWrite)
        .unwrap();
    let reading = reading_side
        .connect("p1", &d.subscriber(), CacheUsage::ReadOnly)
        .unwrap();
    let ops: Arc<dyn CacheOperation> = reading.cache();
    let cache: Cache = reading_side.cache_of(&reading).unwrap();

    // The attributes.
    assert_eq!(ops.cache_usage(), CacheUsage::ReadOnly);
    assert_eq!(writing.cache().cache_usage(), CacheUsage::ReadWrite);
    assert_eq!(ops.homes(), cache.homes());
    let homes: Vec<ObjectHome> = vec![
        reading.home("radar_home").unwrap().clone(),
        reading.home("track_home").unwrap().clone(),
    ];
    assert_eq!(ops.homes(), homes);
    assert!(ops.updates_enabled());
    assert!(ops.objects().is_empty());
    assert!(ops.sub_accesses().is_empty());
    assert!(ops.listeners().is_empty());

    // Listener management.
    let rounds = Arc::new(Rounds::default());
    let listener: Arc<dyn CacheListener> = rounds.clone();
    ops.attach_listener(Arc::clone(&listener)).unwrap();
    assert_eq!(ops.listeners().len(), 1);
    assert!(Arc::ptr_eq(&ops.listeners()[0], &listener));
    assert!(Arc::ptr_eq(&cache.listeners()[0], &listener));

    // Updates management: in manual mode `refresh` applies the updates,
    // without listener.
    ops.disable_updates().unwrap();
    assert!(!ops.updates_enabled());
    assert!(!cache.updates_enabled());
    let first = write_radar(&writing, "first");
    std::thread::sleep(Duration::from_millis(300));
    assert!(ops.objects().is_empty(), "manual mode: nothing applied");
    eventually("refresh applies the radar", || {
        ops.refresh().unwrap();
        ops.objects().len() == 1
    });
    assert_eq!(ops.objects()[0].oid(), first.oid());
    assert_eq!(rounds.events(), ["disabled"]);
    ops.enable_updates().unwrap();
    assert!(ops.updates_enabled());
    assert_eq!(rounds.events(), ["disabled", "enabled"]);
    // Automatic mode: the next radar comes with an update round.
    let second = write_radar(&writing, "second");
    eventually("the second radar arrives in a round", || {
        ops.objects().iter().any(|o| o.oid() == second.oid())
            && rounds.events().ends_with(&["begin", "end"])
    });
    ops.detach_listener(&listener).unwrap();
    assert!(ops.listeners().is_empty());
    assert!(cache.listeners().is_empty());

    // CacheAccess management, with PreconditionNotMet.
    let access = ops.create_access(CacheUsage::ReadOnly).unwrap();
    assert_eq!(ops.sub_accesses(), vec![access.clone()]);
    assert_eq!(cache.sub_accesses(), vec![access.clone()]);
    assert!(matches!(
        ops.create_access(CacheUsage::ReadWrite),
        Err(DlrlException::PreconditionNotMet { .. })
    ));
    ops.delete_access(&access).unwrap();
    assert!(ops.sub_accesses().is_empty());
    let foreign = writing.cache().create_access(CacheUsage::ReadOnly).unwrap();
    assert!(matches!(
        ops.delete_access(&foreign),
        Err(DlrlException::PreconditionNotMet { .. })
    ));

    // The connector removed: AlreadyDeleted from every operation; the
    // attributes keep their value.
    reading_side.remove().unwrap();
    assert!(reading_side.cache_of(&reading).is_none());
    let already_deleted = |result: Result<(), DlrlException>| {
        matches!(result, Err(DlrlException::AlreadyDeleted { .. }))
    };
    assert!(already_deleted(ops.refresh()));
    assert!(already_deleted(ops.attach_listener(Arc::clone(&listener))));
    assert!(already_deleted(ops.detach_listener(&listener)));
    assert!(already_deleted(ops.enable_updates()));
    assert!(already_deleted(ops.disable_updates()));
    assert!(already_deleted(
        ops.create_access(CacheUsage::ReadOnly).map(|_| ())
    ));
    assert!(already_deleted(ops.delete_access(&foreign)));
    assert_eq!(ops.cache_usage(), CacheUsage::ReadOnly);
    assert_eq!(ops.homes(), homes);
    writing_side.remove().unwrap();
}

/// §8.2.1.2: "For each DLRL object type to be part of the application,
/// the DLRL extended port should comprise a receptacle of type the related
/// home inheriting from DDS::ObjectHome. That class should have been
/// generated by the DDS-DLRL product tooling. All accesses to the DLRL
/// objects of this type will be manageable through this entry point."
/// Each home receptacle of a port is the home of its class of the object
/// model the DLRL generator made, registered with the Cache of the port;
/// the objects of the class are created, found and heard of through it.
#[test]
fn dds4ccm_8_2_1_2_each_home_receptacle_is_the_home_of_its_class_of_the_generated_model() {
    let d = Deployment::new();
    let m = mapping();
    let side = connector(d.domain, "", vec![("p1", port_type(&m))]);
    let port = side
        .connect("p1", &d.publisher(), CacheUsage::ReadWrite)
        .unwrap();
    let cache = side.cache_of(&port).unwrap();
    let radars = port.home("radar_home").unwrap();
    let tracks = port.home("track_home").unwrap();
    assert_eq!(radars.class_name(), "Radar");
    assert_eq!(tracks.class_name(), "Track");
    assert!(Arc::ptr_eq(radars.mapping(), &m));
    assert_eq!(
        cache.find_home_by_name("Radar").unwrap().as_ref(),
        Some(radars)
    );
    assert_eq!(
        cache.find_home_by_name("Track").unwrap().as_ref(),
        Some(tracks)
    );
    assert!(port.home("cache").is_none());
    assert!(port.home("ship_home").is_none());
    assert_eq!(port.port_type().homes()[1].class, "Track");

    // The entry point to the objects of its class.
    assert_eq!(tracks.get_topic_name("x").unwrap(), "Track");
    assert_eq!(tracks.get_all_topic_names().unwrap(), vec!["Track"]);
    let created = Arc::new(Created::default());
    tracks.attach_listener(created.clone(), false);
    let access = port.cache().create_access(CacheUsage::ReadWrite).unwrap();
    let radar = radars.create_object(&access).unwrap();
    radar
        .set("name", mono(Value::String("west".into())))
        .unwrap();
    let track = tracks.create_object(&access).unwrap();
    track.set("x", mono(Value::Double(0.5))).unwrap();
    track.set("y", mono(Value::Double(0.25))).unwrap();
    track.set("a_radar", reference(&radar)).unwrap();
    let in_access = CacheBase::Access(access.clone());
    assert_eq!(tracks.get_objects(&in_access).unwrap(), vec![track.clone()]);
    assert_eq!(tracks.find_object(track.oid(), &in_access).unwrap(), track);
    access.write().unwrap();
    // READ_WRITE: the port subscribes to what it publishes; the track
    // reaches the listener of its home.
    eventually("the track is created in the Cache of the port", || {
        let heard = created.objects.lock().unwrap();
        heard.len() == 1 && heard[0].oid() == track.oid()
    });
    let heard = created.objects.lock().unwrap()[0].clone();
    assert_eq!(heard.object_home().as_ref(), Some(tracks));
    side.remove().unwrap();
}

/// §8.2.2: "A DLRL extended port should be made of: A CacheOperation
/// receptacle; As many DDS:ObjectHome-derived receptacles as DLRL object
/// types that will be used by the component using that DLRL port." A port
/// type has one `CacheOperation` receptacle and at least one home
/// receptacle, one per class of its object model, under distinct
/// identifiers.
#[test]
fn dds4ccm_8_2_2_a_port_type_has_one_cache_operation_receptacle_and_a_home_receptacle_per_class() {
    let m = mapping();
    let declare = |name: &str, cache: &str, homes: &[(&str, &str)]| {
        DlrlPortType::new(name, Arc::clone(&m), cache, homes)
    };
    let port = declare(
        "MyDlrlPort_1",
        "cache",
        &[("radar_home", "Radar"), ("track_home", "Track")],
    )
    .unwrap();
    assert_eq!(port.name(), "MyDlrlPort_1");
    assert_eq!(port.cache_receptacle(), "cache");
    assert_eq!(
        port.homes(),
        [
            HomeReceptacle {
                name: "radar_home".into(),
                class: "Radar".into(),
            },
            HomeReceptacle {
                name: "track_home".into(),
                class: "Track".into(),
            },
        ]
    );
    assert_eq!(port.topic_names(), ["Radar", "Track"]);
    assert!(Arc::ptr_eq(port.mapping(), &m));

    assert_eq!(declare("P", "cache", &[]), Err(DeclarationError::NoHome));
    assert_eq!(
        declare("P", "cache", &[("a", "Radar"), ("b", "Radar")]),
        Err(DeclarationError::DuplicateClass("Radar".into()))
    );
    assert_eq!(
        declare("P", "cache", &[("a", "Ship")]),
        Err(DeclarationError::UnknownClass("Ship".into()))
    );
    assert_eq!(
        declare("P", "cache", &[("cache", "Radar")]),
        Err(DeclarationError::DuplicateName("cache".into()))
    );
    assert_eq!(
        declare("P", "objects", &[("Objects", "Radar")]),
        Err(DeclarationError::DuplicateName("Objects".into()))
    );
    assert_eq!(
        declare("P", "cache", &[("1home", "Radar")]),
        Err(DeclarationError::Name("1home".into()))
    );
    assert_eq!(
        declare("My Port", "cache", &[("a", "Radar")]),
        Err(DeclarationError::Name("My Port".into()))
    );
    // IDL 4.2 §7.2.4: an identifier that collides with a keyword is
    // illegal, also when it differs from it only in case.
    assert_eq!(
        declare("P", "home", &[("radars", "Radar")]),
        Err(DeclarationError::Name("home".into()))
    );
    assert_eq!(
        declare("P", "cache", &[("Uses", "Radar")]),
        Err(DeclarationError::Name("Uses".into()))
    );
    assert_eq!(
        declare("PortType", "cache", &[("radars", "Radar")]),
        Err(DeclarationError::Name("PortType".into()))
    );
    assert_eq!(
        declare("P", "LOCAL", &[("radars", "Radar")]),
        Err(DeclarationError::Name("LOCAL".into()))
    );
    // A name that only contains a keyword is one.
    assert!(declare("HomePort", "uses_cache", &[("radar_home", "Radar")]).is_ok());
}

/// §8.2.2: the DLRL entities judge what the fragment registers: a class
/// whose relation target has no home receptacle is `BadHomeDefinition`. A
/// topic the DomainParticipant has keeps its QoS and serves a port whose
/// profile sets no other value (§7.4.3); one that sets another value is
/// refused with `TopicQos`. The refused fragment leaves no entity in the
/// DomainParticipant.
#[test]
fn dds4ccm_8_2_2_a_refused_fragment_leaves_no_entity() {
    let cfg = isolated_cfg();
    let domain = zerodds_test_domains::unique_domain();
    let p = participant(domain, &cfg);
    let container = Container::new("dlrl-refused");
    container
        .set_default_entities(p.clone(), None, None)
        .unwrap();
    let m = mapping();
    let entities = || (p.publishers_len(), p.subscribers_len(), p.topics_len());

    let tracks_only = DlrlPortType::new(
        "TrackPort",
        Arc::clone(&m),
        "cache",
        &[("track_home", "Track")],
    )
    .unwrap();
    let side = connector(domain, "", vec![("p1", tracks_only)]);
    let refused = side.connect("p1", &container.component("c"), CacheUsage::ReadWrite);
    assert!(
        matches!(
            refused,
            Err(ConnectorError::Dlrl(
                DlrlException::BadHomeDefinition { .. }
            ))
        ),
        "{refused:?}"
    );
    assert_eq!(entities(), (0, 0, 0));

    let durable_side = connector(domain, &durable(), vec![("p1", port_type(&m))]);
    let port = durable_side
        .connect("p1", &container.component("durable"), CacheUsage::ReadWrite)
        .unwrap();
    assert_eq!(entities(), (1, 1, 2));
    // A profile that sets no topic policy: the TRANSIENT_LOCAL topics
    // serve it.
    let silent_side = connector(domain, "", vec![("p1", port_type(&m))]);
    let silent = silent_side
        .connect("p1", &container.component("silent"), CacheUsage::ReadOnly)
        .unwrap();
    let silent_cache = silent_side.cache_of(&silent).unwrap();
    let reader = silent_cache
        .the_subscriber()
        .unwrap()
        .lookup_datareader::<DynamicSample>("Radar")
        .unwrap();
    assert_eq!(
        reader.topic().qos().durability.kind,
        DurabilityKind::TransientLocal
    );
    assert_eq!(entities(), (1, 2, 2));
    silent_side.remove().unwrap();
    // A profile that sets VOLATILE for the topics.
    let volatile_side = connector(
        domain,
        &profile(
            "<topic_qos><durability><kind>VOLATILE_DURABILITY_QOS</kind></durability></topic_qos>",
        ),
        vec![("p1", port_type(&m))],
    );
    let refused =
        volatile_side.connect("p1", &container.component("volatile"), CacheUsage::ReadOnly);
    assert_eq!(
        refused.err(),
        Some(ConnectorError::TopicQos {
            topic_name: "Radar".into(),
            policies: vec!["durability"],
        })
    );
    assert_eq!(entities(), (1, 1, 2));
    drop(port);
    durable_side.remove().unwrap();
    assert_eq!(entities(), (0, 0, 0));
    delete(&p);
}

/// §8.2.2, §7.4.3: whether a topic the DomainParticipant has serves a
/// port does not depend on the moment the fragment of another connector
/// creates it. A port of a profile with TRANSIENT_LOCAL topics and a port
/// of a profile that sets no topic policy connect at the same time in one
/// DomainParticipant, round after round. The second port is accepted,
/// whichever fragment creates the topics. The first is accepted with
/// TRANSIENT_LOCAL topics, or refused with `TopicQos` for a topic the
/// second created with the default VOLATILE; never with an error of the
/// DLRL entities.
#[test]
fn dds4ccm_8_2_2_ports_connected_at_the_same_time_are_judged_by_the_topics_they_meet() {
    const ROUNDS: usize = 40;
    let cfg = isolated_cfg();
    let domain = zerodds_test_domains::unique_domain();
    let p = participant(domain, &cfg);
    let container = Container::new("dlrl-same-time");
    container
        .set_default_entities(p.clone(), None, None)
        .unwrap();
    let m = mapping();
    let durable_side = connector(domain, &durable(), vec![("p1", port_type(&m))]);
    let silent_side = connector(domain, "", vec![("p1", port_type(&m))]);
    // The DURABILITY of a topic of the port of the silent side, through
    // its DataReader.
    let durability = |port: &DlrlPort, topic: &str| {
        silent_side
            .cache_of(port)
            .unwrap()
            .the_subscriber()
            .unwrap()
            .lookup_datareader::<DynamicSample>(topic)
            .unwrap()
            .topic()
            .qos()
            .durability
            .kind
    };
    for round in 0..ROUNDS {
        let barrier = Barrier::new(2);
        let (durable_port, silent_port) = std::thread::scope(|scope| {
            let durable_port = scope.spawn(|| {
                let component = container.component(&format!("durable-{round}"));
                barrier.wait();
                durable_side.connect("p1", &component, CacheUsage::ReadWrite)
            });
            let silent_port = scope.spawn(|| {
                let component = container.component(&format!("silent-{round}"));
                barrier.wait();
                silent_side.connect("p1", &component, CacheUsage::ReadOnly)
            });
            (durable_port.join().unwrap(), silent_port.join().unwrap())
        });
        let silent_port = silent_port.unwrap_or_else(|error| panic!("round {round}: {error:?}"));
        match &durable_port {
            Ok(_) => {
                for topic in ["Radar", "Track"] {
                    assert_eq!(
                        durability(&silent_port, topic),
                        DurabilityKind::TransientLocal,
                        "round {round}, {topic}"
                    );
                }
            }
            Err(ConnectorError::TopicQos {
                topic_name,
                policies,
            }) => {
                assert_eq!(policies, &vec!["durability"], "round {round}");
                assert_eq!(
                    durability(&silent_port, topic_name),
                    DurabilityKind::Volatile,
                    "round {round}, {topic_name}"
                );
            }
            Err(error) => panic!("round {round}: {error:?}"),
        }
        drop((durable_port, silent_port));
        durable_side.remove().unwrap();
        silent_side.remove().unwrap();
        assert_eq!(
            (p.publishers_len(), p.subscribers_len(), p.topics_len()),
            (0, 0, 0),
            "round {round}"
        );
    }
    delete(&p);
}

/// §8.3: "A DLRL connector should inherit from the connector DDS_Base, to
/// be given a ConnectorStatusListener port, a domain id and a QoS profile
/// attribute, and add as many mirror ports as there exist DLRL extended
/// ports to share the related set of topics." The attributes are fixed by
/// `configuration_complete`; a component's port is connected to a mirror
/// port by its name; `remove` deletes what the fragments created, and the
/// container keeps its defaults while a port uses them.
#[test]
fn dds4ccm_8_3_a_dlrl_connector_is_a_dds_base_with_a_mirror_port_per_dlrl_port() {
    let d = Deployment::new();
    let m = mapping();
    let radar_only =
        DlrlPortType::new("RadarPort", Arc::clone(&m), "ops", &[("radars", "Radar")]).unwrap();
    assert_eq!(
        DlrlConnector::new(vec![]).err(),
        Some(DeclarationError::NoMirrorPort)
    );
    assert_eq!(
        DlrlConnector::new(vec![("p1", port_type(&m)), ("P1", radar_only.clone())]).err(),
        Some(DeclarationError::DuplicateName("P1".into()))
    );
    assert_eq!(
        DlrlConnector::new(vec![("p-1", port_type(&m))]).err(),
        Some(DeclarationError::Name("p-1".into()))
    );
    assert_eq!(
        DlrlConnector::new(vec![("MirrorPort", port_type(&m))]).err(),
        Some(DeclarationError::Name("MirrorPort".into()))
    );

    let connector =
        DlrlConnector::new(vec![("p1", port_type(&m)), ("p2", radar_only.clone())]).unwrap();
    let declared: Vec<(&str, &str)> = connector
        .mirror_ports()
        .into_iter()
        .map(|(name, port_type)| (name, port_type.name()))
        .collect();
    assert_eq!(declared, [("p1", "RadarTrackPort"), ("p2", "RadarPort")]);

    // DDS_Base.
    assert_eq!(connector.domain_id(), 0);
    assert_eq!(connector.qos_profile(), "");
    let errors = Arc::new(Statuses::default());
    assert!(
        connector
            .connect_error_listener(errors.clone() as Arc<dyn ConnectorStatusListener>)
            .is_none()
    );
    connector.set_domain_id(d.domain).unwrap();
    connector.set_qos_profile(durable()).unwrap();
    assert_eq!(
        connector
            .connect("p1", &d.publisher(), CacheUsage::WriteOnly)
            .err(),
        Some(ConnectorError::NotConfigured)
    );
    connector.configuration_complete().unwrap();
    assert_eq!(connector.set_domain_id(1), Err(NonChangeable));
    assert_eq!(connector.set_qos_profile(""), Err(NonChangeable));
    assert_eq!(connector.domain_id(), d.domain);
    assert_eq!(
        connector
            .connect("p3", &d.publisher(), CacheUsage::WriteOnly)
            .err(),
        Some(ConnectorError::UnknownMirrorPort("p3".into()))
    );

    // Each mirror port serves its port type; the two share the topic
    // Radar.
    let p1 = connector
        .connect("p1", &d.publisher(), CacheUsage::WriteOnly)
        .unwrap();
    let p2 = connector
        .connect("p2", &d.subscriber(), CacheUsage::ReadOnly)
        .unwrap();
    assert_eq!(p1.mirror_port(), "p1");
    assert_eq!(p2.mirror_port(), "p2");
    assert_eq!(p2.port_type(), &radar_only);
    assert_eq!(p2.cache().homes().len(), 1);
    let radar = write_radar(&p1, "south");
    let ops = p2.cache();
    eventually("the radar of p1 arrives at p2", || {
        ops.objects().iter().any(|o| o.oid() == radar.oid())
    });

    // The container keeps its defaults while the port lives.
    assert_eq!(
        d.publishing.remove().err().map(|e| e.error_code),
        Some(zerodds_dcps::error::return_code::PRECONDITION_NOT_MET)
    );
    connector.remove().unwrap();
    assert!(connector.cache_of(&p1).is_none());
    assert!(matches!(
        p1.cache().refresh(),
        Err(DlrlException::AlreadyDeleted { .. })
    ));
    assert!(matches!(
        p2.cache().enable_updates(),
        Err(DlrlException::AlreadyDeleted { .. })
    ));
    d.publishing.remove().unwrap();
    d.subscribing.remove().unwrap();
}

/// §8.3, §7.2.2.1.5: the `ConnectorStatusListener` of a DLRL connector
/// gets the statuses of the DCPS entities of its ports: INCONSISTENT_TOPIC
/// of a topic, OFFERED_INCOMPATIBLE_QOS of a DataWriter,
/// REQUESTED_INCOMPATIBLE_QOS of a DataReader, and PUBLICATION_MATCHED
/// and SUBSCRIPTION_MATCHED with `on_unexpected_status`. DATA_AVAILABLE
/// stays with the Cache, which still applies the updates.
#[test]
fn dds4ccm_8_3_error_listener_gets_the_statuses_of_the_dcps_entities_of_the_ports() {
    let cfg = isolated_cfg();
    let domain = zerodds_test_domains::unique_domain();
    let (p, q, other) = (
        participant(domain, &cfg),
        participant(domain, &cfg),
        participant(domain, &cfg),
    );
    let (writing_container, reading_container) = (
        Container::new("dlrl-writing"),
        Container::new("dlrl-reading"),
    );
    writing_container
        .set_default_entities(p.clone(), None, None)
        .unwrap();
    reading_container
        .set_default_entities(q.clone(), None, None)
        .unwrap();
    let m = mapping();
    let side = connector(domain, "", vec![("p1", port_type(&m))]);
    let errors = Arc::new(Statuses::default());
    side.connect_error_listener(errors.clone() as Arc<dyn ConnectorStatusListener>);
    let writing = side
        .connect(
            "p1",
            &writing_container.component("w"),
            CacheUsage::WriteOnly,
        )
        .unwrap();
    let reading = side
        .connect(
            "p1",
            &reading_container.component("r"),
            CacheUsage::ReadOnly,
        )
        .unwrap();
    let matched = |kind: u32, entity: &str| format!("unexpected:{entity}:{kind:#x}");
    eventually("the matches of the DataWriters and DataReaders", || {
        errors.has(&matched(status_kind::PUBLICATION_MATCHED, "writer"))
            && errors.has(&matched(status_kind::SUBSCRIPTION_MATCHED, "reader"))
    });
    let radar = write_radar(&writing, "east");
    let ops = reading.cache();
    eventually("the Cache still applies the updates", || {
        ops.objects().iter().any(|o| o.oid() == radar.oid())
    });

    // A DataReader and a DataWriter of the topic Radar in another
    // participant, with QoS the ports' entities do not meet.
    let description = m.topic("Radar").unwrap();
    let support =
        SerializedTypeSupport::create_type_support(&dcps_binding::topic_type(description).unwrap())
            .unwrap();
    let topic = other
        .create_dynamic_topic("Radar", TopicQos::default(), support)
        .unwrap();
    let deadline = DataReaderQos {
        deadline: DeadlineQosPolicy {
            period: zerodds_qos::Duration::from_secs(1),
        },
        ..DataReaderQos::default()
    };
    let _strict_reader = other
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<DynamicSample>(&topic, deadline)
        .unwrap();
    let best_effort = DataWriterQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            ..ReliabilityQosPolicy::default()
        },
        ..DataWriterQos::default()
    };
    let _loose_writer = other
        .create_publisher(PublisherQos::default())
        .create_datawriter::<DynamicSample>(&topic, best_effort)
        .unwrap();
    eventually("the incompatible QoS of both ports", || {
        errors.has("offered_incompatible_qos") && errors.has("requested_incompatible_qos")
    });

    // A topic Radar of another type and QoS in the DomainParticipant of
    // the writing port.
    assert!(!errors.has("inconsistent_topic"));
    let mut inconsistent = TopicQos::default();
    inconsistent.topic_data.value = vec![1];
    assert!(p.create_topic::<Track>("Radar", inconsistent).is_err());
    eventually("INCONSISTENT_TOPIC reaches error_listener", || {
        errors.has("inconsistent_topic")
    });
    assert_eq!(
        errors
            .events()
            .iter()
            .filter(|e| *e == "inconsistent_topic")
            .count(),
        1
    );

    side.remove().unwrap();
    for participant in [&p, &q, &other] {
        delete(participant);
    }
}

/// §8.3, §7.2.2.1.5: two ports of one DLRL connector in one
/// DomainParticipant use its topics together. An INCONSISTENT_TOPIC of
/// such a topic reaches the `error_listener` of the connector once, and
/// that of another connector of the topic there once as well.
#[test]
fn dds4ccm_8_3_two_ports_of_a_connector_in_one_participant_hear_inconsistent_topic_once() {
    let cfg = isolated_cfg();
    let domain = zerodds_test_domains::unique_domain();
    let p = participant(domain, &cfg);
    let container = Container::new("dlrl-two-ports");
    container
        .set_default_entities(p.clone(), None, None)
        .unwrap();
    let m = mapping();
    let reported = |connector: &DlrlConnector| {
        let statuses = Arc::new(Statuses::default());
        connector.connect_error_listener(statuses.clone() as Arc<dyn ConnectorStatusListener>);
        statuses
    };
    let side = connector(domain, "", vec![("p1", port_type(&m))]);
    let other_side = connector(domain, "", vec![("p1", port_type(&m))]);
    let (errors, other_errors) = (reported(&side), reported(&other_side));
    // The two ports of `side` first, then the port of `other_side`: the
    // listener of a topic calls the connectors in this order.
    let reading = side
        .connect("p1", &container.component("r"), CacheUsage::ReadOnly)
        .unwrap();
    let writing = side
        .connect("p1", &container.component("w"), CacheUsage::WriteOnly)
        .unwrap();
    let other = other_side
        .connect("p1", &container.component("o"), CacheUsage::ReadOnly)
        .unwrap();
    for port in [&reading, &writing] {
        assert_eq!(
            side.cache_of(port).unwrap().participant().instance_handle(),
            p.instance_handle()
        );
    }
    assert_eq!(
        other_side
            .cache_of(&other)
            .unwrap()
            .participant()
            .instance_handle(),
        p.instance_handle()
    );
    assert_eq!(p.topics_len(), 2);

    let count = |statuses: &Statuses| {
        let events = statuses.events();
        events.iter().filter(|e| *e == "inconsistent_topic").count()
    };
    let mut inconsistent = TopicQos::default();
    inconsistent.topic_data.value = vec![1];
    assert!(p.create_topic::<Track>("Radar", inconsistent).is_err());
    // The listener of the topic calls `side` before `other_side` in one
    // pass: when `other_side` has heard, every call to `side` is done.
    eventually("INCONSISTENT_TOPIC reaches the second connector", || {
        count(&other_errors) == 1
    });
    assert_eq!(count(&errors), 1, "{:?}", errors.events());
    assert_eq!(count(&other_errors), 1, "{:?}", other_errors.events());

    side.remove().unwrap();
    other_side.remove().unwrap();
    assert_eq!(p.topics_len(), 0);
    delete(&p);
}

/// §8.4.1: "As a DLRL port corresponds to one Cache, it must be given its
/// own Publisher and/or Subscriber (depending on the cache usage). In
/// addition, it will get as many DataReaders and/or DataWriters as there
/// are topics used by the DLRL objects." The DomainParticipant is the
/// container's default; one dedicated to the port when the connector's
/// profile asks of it what the default does not have.
#[test]
fn dds4ccm_8_4_1_a_dlrl_port_has_its_own_publisher_and_subscriber_and_an_entity_per_topic() {
    let cfg = isolated_cfg();
    let domain = zerodds_test_domains::unique_domain();
    let p = participant(domain, &cfg);
    let default_publisher = Arc::new(p.create_publisher(PublisherQos::default()));
    let default_subscriber = Arc::new(p.create_subscriber(SubscriberQos::default()));
    let container = Container::new("dlrl-entities");
    container
        .set_default_entities(
            p.clone(),
            Some(Arc::clone(&default_publisher)),
            Some(Arc::clone(&default_subscriber)),
        )
        .unwrap();
    let m = mapping();
    let side = connector(domain, "", vec![("p1", port_type(&m))]);
    let groups = || (p.publishers_len(), p.subscribers_len());
    let users = |topic: &str| {
        let users = p.topic_users(topic).unwrap();
        (users.data_writers, users.data_readers)
    };
    assert_eq!(groups(), (1, 1));

    let both = side
        .connect("p1", &container.component("both"), CacheUsage::ReadWrite)
        .unwrap();
    assert_eq!(groups(), (2, 2));
    assert_eq!(p.topics_len(), 2);
    for topic in ["Radar", "Track"] {
        assert_eq!(users(topic), (1, 1), "{topic}");
    }
    let readers = side
        .connect("p1", &container.component("reader"), CacheUsage::ReadOnly)
        .unwrap();
    assert_eq!(groups(), (2, 3));
    let writers = side
        .connect("p1", &container.component("writer"), CacheUsage::WriteOnly)
        .unwrap();
    assert_eq!(groups(), (3, 3));
    for topic in ["Radar", "Track"] {
        assert_eq!(users(topic), (2, 2), "{topic}");
    }
    assert_eq!(p.topics_len(), 2);
    assert!(!default_publisher.has_datawriters());
    assert!(!default_subscriber.has_datareaders());

    // Each port has its own Publisher or Subscriber, by its usage.
    let cache_of = |port: &DlrlPort| side.cache_of(port).unwrap();
    let handle = |group: Option<zerodds_dcps::InstanceHandle>| group.unwrap();
    let publishers = [
        handle(
            cache_of(&both)
                .the_publisher()
                .map(|g| g.get_instance_handle()),
        ),
        handle(
            cache_of(&writers)
                .the_publisher()
                .map(|g| g.get_instance_handle()),
        ),
        default_publisher.get_instance_handle(),
    ];
    let subscribers = [
        handle(
            cache_of(&both)
                .the_subscriber()
                .map(|g| g.get_instance_handle()),
        ),
        handle(
            cache_of(&readers)
                .the_subscriber()
                .map(|g| g.get_instance_handle()),
        ),
        default_subscriber.get_instance_handle(),
    ];
    for set in [publishers, subscribers] {
        assert_ne!(set[0], set[1]);
        assert_ne!(set[0], set[2]);
        assert_ne!(set[1], set[2]);
    }
    assert!(cache_of(&readers).the_publisher().is_none());
    assert!(cache_of(&writers).the_subscriber().is_none());
    for port in [&both, &readers, &writers] {
        assert_eq!(
            cache_of(port).participant().instance_handle(),
            p.instance_handle()
        );
    }

    // A connector that asks of the DomainParticipant what the default
    // does not have: a DomainParticipant dedicated to the port.
    let apart = connector(
        domain,
        &profile(
            "<domainparticipant_qos><user_data><value>7</value></user_data></domainparticipant_qos>",
        ),
        vec![("p1", port_type(&m))],
    );
    let dedicated = apart
        .connect("p1", &container.component("apart"), CacheUsage::ReadOnly)
        .unwrap();
    let dedicated_participant = apart.cache_of(&dedicated).unwrap().participant().clone();
    assert_ne!(dedicated_participant.instance_handle(), p.instance_handle());
    assert_eq!(dedicated_participant.qos().user_data.value, vec![7]);
    assert_eq!(dedicated_participant.subscribers_len(), 1);
    assert_eq!(groups(), (3, 3));

    apart.remove().unwrap();
    side.remove().unwrap();
    assert_eq!(groups(), (1, 1));
    assert_eq!(p.topics_len(), 0);
    delete(&p);
}

/// §8.4.2: the QoS profile of a DLRL connector holds the values of all
/// its topics, and `topic_filter` assigns them, as in §7.4.3: the
/// `topic_qos`, `datawriter_qos` and `datareader_qos` of each topic by
/// its name in the mapping. The DomainParticipant, the Publisher and the
/// Subscriber of a port serve all its topics and take the container of
/// their kind that is selected for every topic of the port; a container
/// of a topic the port does not have does not apply to it; topics that
/// select different containers for one of these entities refuse the
/// port with `TopicFilter`, and the refused fragment leaves no entity.
#[test]
fn dds4ccm_8_4_2_topic_filter_assigns_the_values_of_the_profile_to_the_entities_of_each_topic() {
    let cfg = isolated_cfg();
    let domain = zerodds_test_domains::unique_domain();
    let p = participant(domain, &cfg);
    let container = Container::new("dlrl-topic-filter");
    container
        .set_default_entities(p.clone(), None, None)
        .unwrap();
    let m = mapping();
    let radar_only =
        DlrlPortType::new("RadarPort", Arc::clone(&m), "cache", &[("radars", "Radar")]).unwrap();
    let entities = || (p.publishers_len(), p.subscribers_len(), p.topics_len());
    let partition =
        |name: &str| format!("<partition><name><element>{name}</element></name></partition>");
    let keep_last = |depth: i32| {
        format!("<history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>{depth}</depth></history>")
    };

    // One profile with the values of the topics Radar and Track.
    let per_topic = connector(
        domain,
        &profile(&format!(
            "<topic_qos topic_filter=\"Track\"><durability>\
             <kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability></topic_qos>\
             <datawriter_qos topic_filter=\"Track\"><history>\
             <kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos>\
             <datawriter_qos>{}</datawriter_qos>\
             <datareader_qos topic_filter=\"Rad*\">{}</datareader_qos>\
             <publisher_qos>{}</publisher_qos>\
             <subscriber_qos topic_filter=\"*\">{}</subscriber_qos>",
            keep_last(3),
            keep_last(5),
            partition("objects"),
            partition("everything"),
        )),
        vec![("p1", port_type(&m))],
    );
    let port = per_topic
        .connect("p1", &container.component("both"), CacheUsage::ReadWrite)
        .unwrap();
    let cache = per_topic.cache_of(&port).unwrap();
    let publisher = cache.the_publisher().unwrap();
    let subscriber = cache.the_subscriber().unwrap();
    let writer = |topic: &str| publisher.lookup_datawriter::<DynamicSample>(topic).unwrap();
    let reader = |topic: &str| {
        subscriber
            .lookup_datareader::<DynamicSample>(topic)
            .unwrap()
    };
    let history = |qos: HistoryQosPolicy| (qos.kind, qos.depth);
    // The topics.
    assert_eq!(
        writer("Track").topic().qos().durability.kind,
        DurabilityKind::TransientLocal
    );
    assert_eq!(
        writer("Radar").topic().qos().durability.kind,
        DurabilityKind::Volatile
    );
    // The DataWriters: the container of Track, the one without filter.
    assert_eq!(writer("Track").get_qos().history.kind, HistoryKind::KeepAll);
    assert_eq!(
        history(writer("Radar").get_qos().history),
        (HistoryKind::KeepLast, 3)
    );
    // The DataReaders: the container of `Rad*`; Track keeps the QoS the
    // Cache gave it, as does every policy the profile does not set.
    assert_eq!(
        history(reader("Radar").get_qos().history),
        (HistoryKind::KeepLast, 5)
    );
    assert_eq!(
        history(reader("Track").get_qos().history),
        (HistoryKind::KeepLast, 1)
    );
    for topic in ["Radar", "Track"] {
        assert_eq!(
            reader(topic).get_qos().reliability.kind,
            ReliabilityKind::Reliable
        );
    }
    // Publisher and Subscriber: a container selected for both topics,
    // without filter or with one that matches both.
    assert_eq!(
        publisher.get_qos().partition.names,
        vec!["objects".to_owned()]
    );
    assert_eq!(
        subscriber.get_qos().partition.names,
        vec!["everything".to_owned()]
    );
    per_topic.remove().unwrap();
    assert_eq!(entities(), (0, 0, 0));

    // A `publisher_qos` for Track only.
    let track_publisher = connector(
        domain,
        &profile(&format!(
            "<publisher_qos topic_filter=\"Track\">{}</publisher_qos>",
            partition("tracks")
        )),
        vec![("p1", port_type(&m)), ("p2", radar_only.clone())],
    );
    // It does not apply to a port of Radar only.
    let radars = track_publisher
        .connect("p2", &container.component("radars"), CacheUsage::WriteOnly)
        .unwrap();
    let radar_publisher = track_publisher
        .cache_of(&radars)
        .unwrap()
        .the_publisher()
        .unwrap();
    assert!(radar_publisher.get_qos().partition.names.is_empty());
    assert_eq!(entities(), (1, 0, 1));
    // The one Publisher of a port of Radar and Track: the profile selects
    // a container for Track and none for Radar.
    assert_eq!(
        track_publisher
            .connect("p1", &container.component("both"), CacheUsage::WriteOnly)
            .err(),
        Some(ConnectorError::TopicFilter {
            container: "publisher_qos",
            topic_names: ["Radar".into(), "Track".into()],
        })
    );
    assert_eq!(entities(), (1, 0, 1));
    // A port without a Publisher.
    let _readers = track_publisher
        .connect("p1", &container.component("readers"), CacheUsage::ReadOnly)
        .unwrap();
    assert_eq!(entities(), (1, 1, 2));
    track_publisher.remove().unwrap();
    assert_eq!(entities(), (0, 0, 0));

    // A `domainparticipant_qos` for Track only.
    let track_participant = connector(
        domain,
        &profile(
            "<domainparticipant_qos topic_filter=\"Track\">\
             <user_data><value>7</value></user_data></domainparticipant_qos>",
        ),
        vec![("p1", port_type(&m)), ("p2", radar_only)],
    );
    let radars = track_participant
        .connect("p2", &container.component("radars"), CacheUsage::ReadOnly)
        .unwrap();
    assert_eq!(
        track_participant
            .cache_of(&radars)
            .unwrap()
            .participant()
            .instance_handle(),
        p.instance_handle()
    );
    assert_eq!(
        track_participant
            .connect("p1", &container.component("both"), CacheUsage::ReadOnly)
            .err(),
        Some(ConnectorError::TopicFilter {
            container: "domainparticipant_qos",
            topic_names: ["Radar".into(), "Track".into()],
        })
    );
    assert_eq!(entities(), (0, 1, 1));
    track_participant.remove().unwrap();
    assert_eq!(entities(), (0, 0, 0));
    delete(&p);
}
