// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DDS-DCPS connectors (DDS4CCM 1.1 §7.3): `DDS_Base` and
//! `DDS_TopicBase` with their attributes and `error_listener` (§7.3.1),
//! `DDS_State` (§7.3.2) and `DDS_Event` (§7.3.3) with their mirror ports.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use std::sync::Arc;

use common::{
    Deployment, Heard, RELIABLE_KEEP_ALL, StateHeard, Statuses, Track, eventually, get_all, pairs,
    profile, track,
};
use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::qos::DurabilityKind;
use zerodds_dcps::{DdsType, HANDLE_NIL, TopicQos};
use zerodds_dds4ccm::{
    ConnectorError, ConnectorStatusListener, DataListenerControl, DdsBase, DdsEvent, DdsGet,
    DdsListen, DdsRead, DdsState, DdsStateListen, DdsTopicBase, InstanceStatus, ListenerMode,
    NonChangeable, ProfileError, ReadInfo, Reader, Updater, UpdaterError, Writer,
};
use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};
use zerodds_types::{PrimitiveKind, TypeIdentifier};

/// A type without a key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Note {
    text: u32,
}

impl DdsType for Note {
    const TYPE_NAME: &'static str = "test::Dds4CcmNote";
    const XCDR1_MATCHES_XCDR2: bool = true;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.text.to_be_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let text: [u8; 4] = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "text" })?;
        Ok(Self {
            text: u32::from_be_bytes(text),
        })
    }
}

/// A type that carries its complete `TypeObject`:
/// `struct Plot { @key long id; long value; }`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Plot {
    id: u32,
    value: u32,
}

impl DdsType for Plot {
    const TYPE_NAME: &'static str = "test::Dds4CcmPlot";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn type_object() -> Option<TypeObject> {
        let long = || TypeIdentifier::Primitive(PrimitiveKind::Int32);
        let structure = TypeObjectBuilder::struct_type(Self::TYPE_NAME)
            .member("id", long(), |member| member.key())
            .member("value", long(), |member| member)
            .build_complete();
        Some(TypeObject::Complete(CompleteTypeObject::Struct(structure)))
    }

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_be_bytes());
        out.extend_from_slice(&self.value.to_be_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let word = |at: usize, what: &'static str| -> Result<u32, DecodeError> {
            bytes
                .get(at..at + 4)
                .and_then(|b| b.try_into().ok())
                .map(u32::from_be_bytes)
                .ok_or(DecodeError::Invalid { what })
        };
        Ok(Self {
            id: word(0, "id")?,
            value: word(4, "value")?,
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let id: [u8; 4] = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "key" })?;
        Ok(Self {
            id: u32::from_be_bytes(id),
            value: 0,
        })
    }
}

/// `configuration_complete` of a connector.
type Complete<'a> = &'a dyn Fn() -> Result<(), ConnectorError>;

/// `read_all` once it returns `count` samples.
fn read_all_of(reader: &dyn Reader<Track>, count: usize) -> Vec<(u32, u32)> {
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    eventually("the port holds the samples", || {
        reader.read_all(&mut data, &mut infos).unwrap();
        data.len() == count
    });
    pairs(&data)
}

/// §7.3.1: `domain_id` and `qos_profile` of `DDS_Base`, `topic_name` and
/// `key_fields` of `DDS_TopicBase`: "Any attempt to change those
/// attributes once the configuration is complete will raise a
/// NonChangeable exception."
#[test]
fn dds4ccm_7_3_1_the_attributes_raise_non_changeable_once_the_configuration_is_complete() {
    let empty = profile("");

    let base = DdsBase::new();
    assert_eq!((base.domain_id(), base.qos_profile().as_str()), (0, ""));
    // Changeable, more than once, while the connector is configured.
    base.set_domain_id(7).unwrap();
    base.set_domain_id(8).unwrap();
    base.set_qos_profile("<dds/>").unwrap();
    base.set_qos_profile(empty.clone()).unwrap();
    assert!(!base.is_configured());
    base.configuration_complete().unwrap();
    assert!(base.is_configured());
    assert_eq!(base.set_domain_id(9), Err(NonChangeable));
    assert_eq!(base.set_qos_profile(""), Err(NonChangeable));
    assert_eq!((base.domain_id(), base.qos_profile()), (8, empty.clone()));
    assert_eq!(
        base.configuration_complete(),
        Err(ConnectorError::AlreadyConfigured)
    );

    // `DDS_TopicBase : DDS_Base`, and the two pattern connectors, which
    // inherit from it: all four attributes.
    let topic_base = DdsTopicBase::new();
    let state = DdsState::<Track>::new();
    let event = DdsEvent::<Track>::new();
    let connectors: [(&DdsTopicBase, Complete<'_>); 3] = [
        (&topic_base, &|| {
            topic_base.configuration_complete::<Track>()
        }),
        (&state, &|| state.configuration_complete()),
        (&event, &|| event.configuration_complete()),
    ];
    for (connector, complete) in connectors {
        assert_eq!(connector.topic_name(), "");
        assert!(connector.key_fields().is_empty());
        connector.set_domain_id(3).unwrap();
        connector.set_qos_profile(empty.clone()).unwrap();
        connector.set_topic_name("Before").unwrap();
        connector.set_topic_name("Dds4CcmAttributes").unwrap();
        connector.set_key_fields(vec!["id".into()]).unwrap();
        complete().unwrap();
        assert!(connector.is_configured());
        assert_eq!(connector.set_domain_id(4), Err(NonChangeable));
        assert_eq!(connector.set_qos_profile(""), Err(NonChangeable));
        assert_eq!(connector.set_topic_name("After"), Err(NonChangeable));
        assert_eq!(connector.set_key_fields(Vec::new()), Err(NonChangeable));
        assert_eq!(connector.domain_id(), 3);
        assert_eq!(connector.qos_profile(), empty);
        assert_eq!(connector.topic_name(), "Dds4CcmAttributes");
        assert_eq!(connector.key_fields(), ["id"]);
        assert_eq!(complete(), Err(ConnectorError::AlreadyConfigured));
    }
}

/// §7.3.1: "The QoS profile could be given either as a file URL or as the
/// XML string itself." Both reach the DataReader of a port.
#[test]
fn dds4ccm_7_3_1_qos_profile_is_a_file_url_or_the_xml_string() {
    let d = Deployment::new();
    let depth = |depth: u32| {
        format!(
            "<datareader_qos><history><kind>KEEP_LAST_HISTORY_QOS</kind>\
             <depth>{depth}</depth></history></datareader_qos>"
        )
    };
    let document = format!(
        "<dds><qos_profile name=\"Shallow\">{}</qos_profile>\
         <qos_profile name=\"Deep\">{}</qos_profile></dds>",
        depth(3),
        depth(9)
    );
    // The history depth of the DataReader a port of a connector with
    // this `qos_profile` gets.
    let depth_with = |qos_profile: &str, topic: &str| {
        let connector = d.state(topic, qos_profile);
        let port = DdsRead::<Track>::new();
        connector.passive_observer(&d.subscriber(), &port).unwrap();
        let depth = port.dds_entity().unwrap().qos().history.depth;
        connector.remove().unwrap();
        depth
    };

    // The XML string: its last profile.
    assert_eq!(depth_with(&document, "Dds4CcmProfileString"), 9);

    // The file URL, with an escaped blank in its path; the fragment names
    // another profile of the file.
    let dir = std::env::temp_dir().join(format!("dds4ccm profiles {}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("profiles.xml");
    std::fs::write(&file, &document).unwrap();
    // `file:///tmp/…`; on Windows `file:///C:/…` (RFC 8089 Appendix E.2).
    let path = file.display().to_string().replace('\\', "/");
    let slash = if path.starts_with('/') { "" } else { "/" };
    let url = format!("file://{slash}{path}").replace(' ', "%20");
    assert_eq!(depth_with(&url, "Dds4CcmProfileFile"), 9);
    assert_eq!(
        depth_with(&format!("{url}#Shallow"), "Dds4CcmProfileNamed"),
        3
    );

    // A profile the file does not have, and a file that is not there.
    let missing = DdsState::<Track>::new();
    missing.set_topic_name("Dds4CcmProfileMissing").unwrap();
    missing.set_key_fields(vec!["id".into()]).unwrap();
    missing.set_qos_profile(format!("{url}#Missing")).unwrap();
    assert_eq!(
        missing.configuration_complete(),
        Err(ConnectorError::QosProfile(ProfileError::UnknownProfile(
            "Missing".into()
        )))
    );
    std::fs::remove_dir_all(&dir).unwrap();
    missing.set_qos_profile(url).unwrap();
    assert!(matches!(
        missing.configuration_complete(),
        Err(ConnectorError::QosProfile(ProfileError::File { .. }))
    ));
}

/// A `qos_profile` that cannot be read fails the configuration with the
/// reason; the configuration is not complete then, and the attributes can
/// still be set.
#[test]
fn dds4ccm_7_3_1_a_profile_that_cannot_be_read_leaves_the_connector_unconfigured() {
    let d = Deployment::new();
    let connector = DdsEvent::<Track>::new();
    connector.set_domain_id(d.domain).unwrap();
    connector.set_topic_name("Dds4CcmBadProfile").unwrap();
    connector.set_key_fields(vec!["id".into()]).unwrap();

    // An enumeration value DDS does not have.
    connector
        .set_qos_profile(profile(
            "<datareader_qos><history><kind>KEEP_SOME_HISTORY_QOS</kind></history></datareader_qos>",
        ))
        .unwrap();
    let error = connector.configuration_complete().unwrap_err();
    assert!(
        matches!(error, ConnectorError::QosProfile(ProfileError::Xml(_))),
        "{error:?}"
    );
    // Neither an XML string nor a file URL.
    connector.set_qos_profile("/etc/qos.xml").unwrap();
    let error = connector.configuration_complete().unwrap_err();
    assert!(
        matches!(error, ConnectorError::QosProfile(ProfileError::Url(_))),
        "{error:?}"
    );
    assert!(!connector.is_configured());

    // No fragment of a connector that is not configured.
    let port = DdsGet::<Track>::new();
    assert_eq!(
        connector.pull_consumer(&d.subscriber(), &port),
        Err(ConnectorError::NotConfigured)
    );
    assert!(matches!(
        connector.supplier(&d.publisher()),
        Err(ConnectorError::NotConfigured)
    ));
    assert!(port.dds_entity().is_none());

    connector.set_qos_profile("").unwrap();
    connector.configuration_complete().unwrap();
    connector.pull_consumer(&d.subscriber(), &port).unwrap();
    assert!(port.dds_entity().is_some());
    connector.remove().unwrap();
}

/// §7.3: the configuration names "the name of the DDS Topic that is
/// associated to the data type" and "the list of fields making up the key
/// for that Topic". The topic of the fragments has that name; a
/// `key_fields` that contradicts the key of the data type fails the
/// configuration.
#[test]
fn dds4ccm_7_3_1_topic_name_and_key_fields() {
    let d = Deployment::new();
    let connector = DdsState::<Track>::new();
    connector.set_domain_id(d.domain).unwrap();
    assert_eq!(
        connector.configuration_complete(),
        Err(ConnectorError::TopicName)
    );
    connector.set_topic_name("Dds4CcmTopicOfConnector").unwrap();
    // `Track` has a key, the attribute names none.
    assert_eq!(
        connector.configuration_complete(),
        Err(ConnectorError::KeyFields {
            key_fields: Vec::new(),
            key_of_type: None,
        })
    );
    assert!(!connector.is_configured());
    connector.set_key_fields(vec!["id".into()]).unwrap();
    connector.configuration_complete().unwrap();

    // Both sides are on the topic of that name, with the data type.
    let observable = connector.observable(&d.publisher(), false).unwrap();
    let port = DdsRead::<Track>::new();
    connector.passive_observer(&d.subscriber(), &port).unwrap();
    for topic in [
        observable.dds_entity().topic(),
        port.dds_entity().unwrap().topic(),
    ] {
        assert_eq!(topic.name(), "Dds4CcmTopicOfConnector");
        assert_eq!(topic.type_name(), Track::TYPE_NAME);
    }
    connector.remove().unwrap();

    // A type without a key, and an attribute that names one.
    let keyless = DdsEvent::<Note>::new();
    keyless.set_topic_name("Dds4CcmKeyless").unwrap();
    keyless.set_key_fields(vec!["text".into()]).unwrap();
    assert_eq!(
        keyless.configuration_complete(),
        Err(ConnectorError::KeyFields {
            key_fields: vec!["text".into()],
            key_of_type: None,
        })
    );
    keyless.set_key_fields(Vec::new()).unwrap();
    keyless.configuration_complete().unwrap();
}

/// `key_fields` of a connector whose data type carries its complete
/// `TypeObject` are compared with the names of the key members; the
/// connector of such a type carries the data through DDS.
#[test]
fn dds4ccm_7_3_1_key_fields_of_a_type_with_its_type_object_are_its_key_members() {
    let d = Deployment::new();
    let connector = DdsState::<Plot>::new();
    connector.set_domain_id(d.domain).unwrap();
    connector.set_topic_name("Dds4CcmPlot").unwrap();
    connector
        .set_qos_profile(profile(RELIABLE_KEEP_ALL))
        .unwrap();
    // `value` is a member and not a key member; the type has a key, so a
    // check for presence alone would let it pass.
    for wrong in [vec!["value".to_owned()], vec!["id".into(), "value".into()]] {
        connector.set_key_fields(wrong.clone()).unwrap();
        assert_eq!(
            connector.configuration_complete(),
            Err(ConnectorError::KeyFields {
                key_fields: wrong,
                key_of_type: Some(vec!["id".into()]),
            })
        );
        assert!(!connector.is_configured());
    }
    connector.set_key_fields(vec!["id".into()]).unwrap();
    connector.configuration_complete().unwrap();

    let port = DdsRead::<Plot>::new();
    connector.passive_observer(&d.subscriber(), &port).unwrap();
    let observable = connector.observable(&d.publisher(), false).unwrap();
    observable
        .dds_entity()
        .wait_for_matched_subscription(1, std::time::Duration::from_secs(15))
        .expect("the observer matches the observable");
    let plot = |id, value| Plot { id, value };
    let updater = observable.data();
    updater.create_one(&plot(1, 10), HANDLE_NIL).unwrap();
    updater.create_one(&plot(2, 20), HANDLE_NIL).unwrap();
    // The key of the type tells the instances apart: the first exists.
    assert!(matches!(
        updater.create_one(&plot(1, 11), HANDLE_NIL),
        Err(UpdaterError::AlreadyCreated(_))
    ));
    updater.update_one(&plot(1, 11), HANDLE_NIL).unwrap();
    let reader = port.data().unwrap();
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    eventually("the port holds the samples", || {
        reader.read_all(&mut data, &mut infos).unwrap();
        data.len() == 3
    });
    let mut read: Vec<(u32, u32)> = data.iter().map(|p| (p.id, p.value)).collect();
    read.sort_unstable();
    assert_eq!(read, [(1, 10), (1, 11), (2, 20)]);

    drop((updater, observable));
    connector.remove().unwrap();
}

/// The DataReader the connector creates for a `DDS_Update` port with
/// `is_global_scope` reports to `error_listener` as the entities of the
/// other ports do: REQUESTED_INCOMPATIBLE_QOS reaches it through DDS.
#[test]
fn dds4ccm_7_3_1_error_listener_gets_the_statuses_of_the_observer_of_a_global_update_port() {
    const TOPIC: &str = "Dds4CcmObserverStatuses";
    let d = Deployment::new();
    // The observing side requests RELIABLE of what it reads, the other
    // side offers BEST_EFFORT.
    let reliable = profile(
        "<datareader_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></datareader_qos>",
    );
    let best_effort = profile(
        "<datawriter_qos><reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability></datawriter_qos>",
    );
    let observing = d.state(TOPIC, &reliable);
    let offering = d.state(TOPIC, &best_effort);
    let errors = Arc::new(Statuses::default());
    observing.connect_error_listener(errors.clone() as Arc<dyn ConnectorStatusListener>);

    // The only DataReader of the observing connector is the one its
    // `DDS_Update` port looks the instances up on.
    let global = observing.observable(&d.subscriber(), true).unwrap();
    let updater = global.data();
    let observer = updater.observer().expect("the observer of the port");
    assert_eq!(
        observer.qos().reliability.kind,
        zerodds_dcps::qos::ReliabilityKind::Reliable
    );
    assert!(!errors.has("requested_incompatible_qos"));
    let other = offering.observable(&d.publisher(), false).unwrap();
    eventually(
        "REQUESTED_INCOMPATIBLE_QOS of the observer reaches error_listener",
        || errors.has("requested_incompatible_qos"),
    );
    // A DataWriter that offers RELIABLE comes up in the other container:
    // the match of the observer is a status no operation names.
    let matched = format!(
        "unexpected:reader:{:#x}",
        zerodds_dcps::psm_constants::status::SUBSCRIPTION_MATCHED
    );
    let before = errors.events().iter().filter(|e| **e == matched).count();
    let reliable_writer = observing.observable(&d.publisher(), false).unwrap();
    eventually("the match of the observer is reported", || {
        errors.events().iter().filter(|e| **e == matched).count() > before
    });

    drop((updater, global, other, reliable_writer));
    offering.remove().unwrap();
    observing.remove().unwrap();
}

/// §7.3.1: "DDS_Base connector uses a ConnectorStatusListener port for
/// reporting configuration errors." The fragments report through it:
/// INCONSISTENT_TOPIC from the TopicListener the connector sets on its
/// topic, the incompatible QoS of both ends.
#[test]
fn dds4ccm_7_3_1_error_listener_gets_the_inconsistent_topic_and_the_incompatible_qos() {
    let d = Deployment::new();
    let reported = |connector: &DdsEvent<Track>| {
        let statuses = Arc::new(Statuses::default());
        let before =
            connector.connect_error_listener(statuses.clone() as Arc<dyn ConnectorStatusListener>);
        assert!(before.is_none());
        statuses
    };
    // The consuming side requests RELIABLE, the supplying side offers
    // BEST_EFFORT: two connector fragments of one topic whose profiles do
    // not agree.
    let reliable = profile(
        "<datareader_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></datareader_qos>",
    );
    let best_effort = profile(
        "<datawriter_qos><reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability></datawriter_qos>",
    );
    let consuming = d.event("Dds4CcmConfigurationErrors", &reliable);
    let supplying = d.event("Dds4CcmConfigurationErrors", &best_effort);
    let (consumer_errors, supplier_errors) = (reported(&consuming), reported(&supplying));

    let port = DdsGet::<Track>::new();
    consuming.pull_consumer(&d.subscriber(), &port).unwrap();
    let supplier = supplying.supplier(&d.publisher()).unwrap();
    eventually("the incompatible QoS reaches both connectors", || {
        consumer_errors.has("requested_incompatible_qos")
            && supplier_errors.has("offered_incompatible_qos")
    });
    assert!(!consumer_errors.has("inconsistent_topic"));

    // A second topic of the name with another type and a QoS that is not
    // consistent with the connector's, in the DomainParticipant of the
    // consuming fragment: DDS reports INCONSISTENT_TOPIC on the
    // connector's topic.
    let participant = port
        .dds_entity()
        .unwrap()
        .get_subscriber()
        .get_participant()
        .unwrap();
    let mut other = TopicQos::default();
    other.topic_data.value = vec![1];
    assert!(
        participant
            .create_topic::<Note>("Dds4CcmConfigurationErrors", other)
            .is_err()
    );
    eventually("INCONSISTENT_TOPIC reaches error_listener", || {
        consumer_errors.has("inconsistent_topic")
    });
    assert!(
        !supplier_errors.has("inconsistent_topic"),
        "the topic of the other DomainParticipant is consistent"
    );

    // A disconnected listener is not called any more.
    let disconnected = consuming.disconnect_error_listener();
    assert!(disconnected.is_some());
    let count = consumer_errors.events().len();
    let mut again = TopicQos::default();
    again.topic_data.value = vec![2];
    assert!(
        participant
            .create_topic::<Note>("Dds4CcmConfigurationErrors", again)
            .is_err()
    );
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(consumer_errors.events().len(), count);

    drop(supplier);
    supplying.remove().unwrap();
    consuming.remove().unwrap();
}

/// Two connectors that use one topic in one DomainParticipant share the
/// topic: INCONSISTENT_TOPIC reaches the `error_listener` of both, and the
/// topic lives until the last of them is removed.
#[test]
fn dds4ccm_7_3_1_two_connectors_of_one_topic_both_get_the_inconsistent_topic() {
    const TOPIC: &str = "Dds4CcmSharedTopic";
    let d = Deployment::new();
    let component = d.subscriber();
    let (first, second) = (d.event(TOPIC, ""), d.event(TOPIC, ""));
    let reported = |connector: &DdsEvent<Track>| {
        let statuses = Arc::new(Statuses::default());
        connector.connect_error_listener(statuses.clone() as Arc<dyn ConnectorStatusListener>);
        statuses
    };
    let (first_errors, second_errors) = (reported(&first), reported(&second));
    let (first_port, second_port) = (DdsGet::<Track>::new(), DdsGet::<Track>::new());
    first.pull_consumer(&component, &first_port).unwrap();
    second.pull_consumer(&component, &second_port).unwrap();
    let reader = first_port.dds_entity().unwrap();
    let participant = reader.get_subscriber().get_participant().unwrap();
    // A topic of the name with another type and a QoS that is not
    // consistent: refused, and reported on the connectors' topic.
    let clash = |octet: u8| {
        let mut other = TopicQos::default();
        other.topic_data.value = vec![octet];
        assert!(participant.create_topic::<Note>(TOPIC, other).is_err());
    };
    let count = |statuses: &Statuses| {
        let events = statuses.events();
        events.iter().filter(|e| *e == "inconsistent_topic").count()
    };

    clash(1);
    eventually("INCONSISTENT_TOPIC reaches both connectors", || {
        count(&first_errors) == 1 && count(&second_errors) == 1
    });

    // The first connector goes: the topic stays for the second, which
    // still hears and still gets ports.
    first.remove().unwrap();
    let readers = |participant: &zerodds_dcps::DomainParticipant| {
        participant.topic_users(TOPIC).unwrap().data_readers
    };
    assert_eq!(readers(&participant), 1, "the port of the second connector");
    clash(2);
    eventually(
        "INCONSISTENT_TOPIC reaches the connector that is left",
        || count(&second_errors) == 2,
    );
    assert_eq!(count(&first_errors), 1, "a removed connector hears nothing");
    let third_port = DdsGet::<Track>::new();
    second.pull_consumer(&component, &third_port).unwrap();
    assert_eq!(third_port.dds_entity().unwrap().topic().name(), TOPIC);

    assert_eq!(readers(&participant), 2);

    // The last connector deletes the topic: deleting it again is
    // ALREADY_DELETED.
    let topic = third_port.dds_entity().unwrap().topic().clone();
    second.remove().unwrap();
    let again = participant.delete_topic(&topic).unwrap_err();
    assert_eq!(
        again.as_return_code(),
        zerodds_dcps::error::return_code::ALREADY_DELETED
    );
}

/// A topic that exists in the DomainParticipant of a port keeps its QoS.
/// It serves a connector when it has every policy the `topic_qos` of the
/// connector's profile sets; a connector whose profile sets another value
/// is refused with the policies named, and nothing of its port is created.
/// This holds for the topic of another connector and for a topic the
/// application created.
#[test]
fn dds4ccm_7_3_1_a_topic_that_exists_must_have_the_topic_qos_of_the_profile() {
    const TOPIC: &str = "Dds4CcmTopicQos";
    let d = Deployment::new();
    let component = d.subscriber();
    let transient_local = profile(
        "<topic_qos><durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability></topic_qos>",
    );
    let durability_of = |port: &DdsRead<Track>| {
        let reader = port.dds_entity().unwrap();
        reader.topic().qos().durability.kind
    };

    let first = d.state(TOPIC, &transient_local);
    let port = DdsRead::<Track>::new();
    first.passive_observer(&component, &port).unwrap();
    assert_eq!(durability_of(&port), DurabilityKind::TransientLocal);
    let reader = port.dds_entity().unwrap();
    let participant = reader.get_subscriber().get_participant().unwrap();
    let users = || {
        let users = participant.topic_users(TOPIC).unwrap();
        (users.data_readers, users.data_writers)
    };
    assert_eq!(users(), (1, 0));

    // A second connector of the topic in this DomainParticipant whose
    // profile sets another durability and topic data: refused.
    let other = d.state(
        TOPIC,
        &profile(
            "<topic_qos><durability><kind>VOLATILE_DURABILITY_QOS</kind></durability>\
             <topic_data><value>1</value></topic_data></topic_qos>",
        ),
    );
    let refused = DdsRead::<Track>::new();
    let error = other.passive_observer(&component, &refused).unwrap_err();
    assert_eq!(
        error,
        ConnectorError::TopicQos {
            topic_name: TOPIC.into(),
            policies: vec!["durability", "topic_data"],
        }
    );
    assert!(refused.dds_entity().is_none());
    assert!(matches!(
        other.observable(&component, false),
        Err(ConnectorError::TopicQos { .. })
    ));
    assert_eq!(users(), (1, 0), "nothing of the refused ports was created");
    assert_eq!(durability_of(&port), DurabilityKind::TransientLocal);

    // A profile that sets the value the topic has, and one that sets
    // nothing for the topic, are served by it.
    let same = d.state(TOPIC, &transient_local);
    let silent = d.state(TOPIC, "");
    for connector in [&same, &silent] {
        let port = DdsRead::<Track>::new();
        connector.passive_observer(&component, &port).unwrap();
        assert_eq!(durability_of(&port), DurabilityKind::TransientLocal);
    }
    assert_eq!(users(), (3, 0));

    // The refused connector gets its topic where the name is free: in the
    // DomainParticipant of the other container, with its own values.
    other.passive_observer(&d.publisher(), &refused).unwrap();
    let elsewhere = refused.dds_entity().unwrap().topic().qos();
    assert_eq!(
        (elsewhere.durability.kind, elsewhere.topic_data.value),
        (DurabilityKind::Volatile, vec![1])
    );

    // A topic the application created.
    const OF_APPLICATION: &str = "Dds4CcmTopicQosOfApplication";
    let mut qos = TopicQos::default();
    qos.topic_data.value = vec![7];
    let of_application = participant
        .create_topic::<Track>(OF_APPLICATION, qos)
        .unwrap();
    let asking = d.state(OF_APPLICATION, &transient_local);
    let late = DdsRead::<Track>::new();
    assert_eq!(
        asking.passive_observer(&component, &late),
        Err(ConnectorError::TopicQos {
            topic_name: OF_APPLICATION.into(),
            policies: vec!["durability"],
        })
    );
    let agreeing = d.state(
        OF_APPLICATION,
        &profile("<topic_qos><topic_data><value>7</value></topic_data></topic_qos>"),
    );
    agreeing.passive_observer(&component, &late).unwrap();
    let topic = late.dds_entity().unwrap().topic().qos();
    assert_eq!(topic.topic_data.value, [7]);
    assert_eq!(topic.durability.kind, DurabilityKind::Volatile);

    for connector in [&agreeing, &asking, &silent, &same, &other, &first] {
        connector.remove().unwrap();
    }
    // The topic of the application is still the application's: both
    // connectors gave back what they obtained, so one `delete_topic`
    // deletes it.
    participant.delete_topic(&of_application).unwrap();
    let again = participant.delete_topic(&of_application).unwrap_err();
    assert_eq!(
        again.as_return_code(),
        zerodds_dcps::error::return_code::ALREADY_DELETED
    );
}

/// §7.3.2 `DDS_State`: the role observable (`DDS_Update`) and the four
/// observers `passive_observer` (`DDS_Read`), `pull_observer` (`DDS_Get`),
/// `push_observer` (`DDS_Listen`) and `push_state_observer`
/// (`DDS_StateListen`). "All those roles relate to the connector's topic":
/// what the observable publishes reaches each observer in the way of its
/// port.
#[test]
fn dds4ccm_7_3_2_state_connector_the_observable_reaches_the_four_observers() {
    let d = Deployment::new();
    let qos_profile = profile(RELIABLE_KEEP_ALL);
    // One connector, deployed as a fragment on each side.
    let publishing = d.state("Dds4CcmState", &qos_profile);
    let subscribing = d.state("Dds4CcmState", &qos_profile);
    let sink = d.subscriber();

    let passive = DdsRead::<Track>::new();
    let pull = DdsGet::<Track>::new();
    let push = DdsListen::<Track>::new();
    let push_state = DdsStateListen::<Track>::new();
    let heard = Arc::new(Heard::default());
    let state_heard = Arc::new(StateHeard::default());
    push.connect_data_listener(heard.clone());
    push.data_control().set_mode(ListenerMode::OneByOne);
    push_state.connect_data_listener(state_heard.clone());
    push_state.data_control().set_mode(ListenerMode::OneByOne);
    subscribing.passive_observer(&sink, &passive).unwrap();
    subscribing.pull_observer(&sink, &pull).unwrap();
    subscribing.push_observer(&sink, &push).unwrap();
    subscribing.push_state_observer(&sink, &push_state).unwrap();

    let observable = publishing.observable(&d.publisher(), false).unwrap();
    observable
        .dds_entity()
        .wait_for_matched_subscription(4, std::time::Duration::from_secs(15))
        .expect("the four observers match the observable");
    let updater = observable.data();
    assert!(!updater.is_global_scope());
    updater.create_one(&track(1, 10), HANDLE_NIL).unwrap();
    updater.update_one(&track(1, 11), HANDLE_NIL).unwrap();
    updater.create_one(&track(2, 20), HANDLE_NIL).unwrap();
    updater.delete_one(&track(2, 0), HANDLE_NIL).unwrap();

    // passive_observer "are just reading the state when they want": the
    // samples of the instance that is alive.
    assert_eq!(
        read_all_of(&*passive.data().unwrap(), 2),
        [(1, 10), (1, 11)]
    );
    // pull_observer "are getting the state changes": the samples, and
    // the deletion of the second instance.
    assert_eq!(
        get_all(&*pull.fresh_data().unwrap(), 4),
        (vec![(1, 10), (1, 11), (2, 20)], vec![2])
    );
    // push_observer "are being notified with the state changes".
    eventually("the push observer is notified", || {
        heard.one.lock().unwrap().len() >= 4
    });
    let changes: Vec<(u32, u32, InstanceStatus)> = heard
        .one
        .lock()
        .unwrap()
        .iter()
        .map(|(t, info)| (t.id, t.value, info.instance_status))
        .collect();
    let of = |id: u32| -> Vec<(u32, InstanceStatus)> {
        let of_instance = changes.iter().filter(|c| c.0 == id);
        of_instance.map(|c| (c.1, c.2)).collect()
    };
    assert_eq!(
        of(1),
        [
            (10, InstanceStatus::InstanceCreated),
            (11, InstanceStatus::InstanceUpdated)
        ]
    );
    assert_eq!(
        of(2),
        [
            (20, InstanceStatus::InstanceCreated),
            (0, InstanceStatus::InstanceDeleted)
        ]
    );
    // push_state_observer "with different operations depending on the
    // instance status".
    eventually("the state observer is told the deletion", || {
        state_heard
            .calls()
            .iter()
            .any(|(call, _)| *call == "deletion")
    });
    let calls = state_heard.calls();
    let of_instance = |id: u32| -> Vec<&str> {
        calls
            .iter()
            .filter(|(_, data)| data.iter().any(|t| t.id == id))
            .map(|(call, _)| *call)
            .collect()
    };
    assert_eq!(of_instance(1), ["creation", "one_update"]);
    assert_eq!(of_instance(2), ["creation", "deletion"]);

    // A second observable whose Updater checks the lifecycle in the data
    // space (`is_global_scope`): it finds the instance of the first.
    let global = publishing.observable(&d.publisher(), true).unwrap();
    assert!(global.data().is_global_scope());
    eventually(
        "the global observable sees the instance of the first",
        || {
            matches!(
                global.data().create_one(&track(1, 99), HANDLE_NIL),
                Err(UpdaterError::AlreadyCreated(_))
            )
        },
    );

    drop((observable, global));
    publishing.remove().unwrap();
    subscribing.remove().unwrap();
}

/// §7.3.2: "Typically, with this pattern, HISTORY QoS should be set to
/// KEEP_LAST"; §7.3.3: "... should be set to KEEP_ALL". The DataWriters
/// and DataReaders of a connector have the kind of its pattern unless the
/// QoS profile sets HISTORY.
#[test]
fn dds4ccm_7_3_2_and_7_3_3_history_is_that_of_the_pattern_unless_the_profile_sets_it() {
    let d = Deployment::new();
    let history = |kind: &str| {
        let policy = format!("<history><kind>{kind}</kind></history>");
        profile(&format!(
            "<datawriter_qos>{policy}</datawriter_qos><datareader_qos>{policy}</datareader_qos>"
        ))
    };
    // The HISTORY kind of the DataWriter and the DataReader a connector
    // with `qos_profile` creates.
    let of_state = |qos_profile: &str, topic: &str| {
        let connector = d.state(topic, qos_profile);
        let writer = connector.observable(&d.publisher(), true).unwrap();
        let port = DdsRead::<Track>::new();
        connector.passive_observer(&d.subscriber(), &port).unwrap();
        let kinds = [
            writer.dds_entity().qos().history.kind,
            writer.data().observer().unwrap().qos().history.kind,
            port.dds_entity().unwrap().qos().history.kind,
        ];
        drop(writer);
        connector.remove().unwrap();
        kinds
    };
    let of_event = |qos_profile: &str, topic: &str| {
        let connector = d.event(topic, qos_profile);
        let writer = connector.supplier(&d.publisher()).unwrap();
        let port = DdsGet::<Track>::new();
        connector.pull_consumer(&d.subscriber(), &port).unwrap();
        let kinds = [
            writer.dds_entity().qos().history.kind,
            port.dds_entity().unwrap().qos().history.kind,
        ];
        drop(writer);
        connector.remove().unwrap();
        kinds
    };
    use zerodds_dcps::qos::HistoryKind::{KeepAll, KeepLast};
    // Without a profile, and with one that is silent about HISTORY.
    let silent = profile(
        "<datawriter_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></datawriter_qos>",
    );
    assert_eq!(of_state("", "Dds4CcmHistoryState"), [KeepLast; 3]);
    assert_eq!(
        of_state(&silent, "Dds4CcmHistoryStateSilent"),
        [KeepLast; 3]
    );
    assert_eq!(of_event("", "Dds4CcmHistoryEvent"), [KeepAll; 2]);
    assert_eq!(of_event(&silent, "Dds4CcmHistoryEventSilent"), [KeepAll; 2]);
    // The profile decides where it sets HISTORY.
    assert_eq!(
        of_state(&history("KEEP_ALL_HISTORY_QOS"), "Dds4CcmHistoryStateSet"),
        [KeepAll; 3]
    );
    assert_eq!(
        of_event(&history("KEEP_LAST_HISTORY_QOS"), "Dds4CcmHistoryEventSet"),
        [KeepLast; 2]
    );
}

/// §7.3.3 `DDS_Event`: the role supplier (`DDS_Write`) and the consumers
/// `pull_consumer` (`DDS_Get`) and `push_consumer` (`DDS_Listen`): every
/// event the supplier sends reaches both.
#[test]
fn dds4ccm_7_3_3_event_connector_the_supplier_reaches_both_consumers() {
    let d = Deployment::new();
    let qos_profile = profile(RELIABLE_KEEP_ALL);
    let supplying = d.event("Dds4CcmEvent", &qos_profile);
    let consuming = d.event("Dds4CcmEvent", &qos_profile);
    let sink = d.subscriber();

    let pull = DdsGet::<Track>::new();
    let push = DdsListen::<Track>::new();
    let heard = Arc::new(Heard::default());
    push.connect_data_listener(heard.clone());
    push.data_control().set_mode(ListenerMode::OneByOne);
    consuming.pull_consumer(&sink, &pull).unwrap();
    consuming.push_consumer(&sink, &push).unwrap();

    let supplier = supplying.supplier(&d.publisher()).unwrap();
    supplier
        .dds_entity()
        .wait_for_matched_subscription(2, std::time::Duration::from_secs(15))
        .expect("both consumers match the supplier");
    let events = [(1, 1), (1, 2), (2, 1), (1, 3)];
    for (id, value) in events {
        supplier
            .data()
            .write_one(&track(id, value), HANDLE_NIL)
            .unwrap();
    }

    let mut sent = events.to_vec();
    sent.sort_unstable();
    assert_eq!(
        get_all(&*pull.fresh_data().unwrap(), 4),
        (sent.clone(), Vec::new())
    );
    eventually("the push consumer is notified of every event", || {
        heard.one.lock().unwrap().len() == 4
    });
    let notified: Vec<Track> = heard
        .one
        .lock()
        .unwrap()
        .iter()
        .map(|(t, _)| t.clone())
        .collect();
    assert_eq!(pairs(&notified), sent);
    // The events of one instance arrive in the order they were sent.
    let of_first: Vec<u32> = notified
        .iter()
        .filter(|t| t.id == 1)
        .map(|t| t.value)
        .collect();
    assert_eq!(of_first, [1, 2, 3]);

    drop(supplier);
    supplying.remove().unwrap();
    consuming.remove().unwrap();
}

/// A listener the connection of its port calls with the data the
/// DataReader holds may use the connector of the port: here it connects a
/// second port to it. The connection calls the listener after the
/// connector has completed the fragment and holds none of its locks.
#[test]
fn dds4ccm_7_4_1_a_listener_called_at_the_connection_may_use_the_connector() {
    use std::sync::Mutex;
    use std::sync::mpsc;
    use std::time::Duration;
    use zerodds_dds4ccm::Component;

    const TOPIC: &str = "Dds4CcmListenerAtConnection";
    const DURABLE: &str = "<durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>\
         <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>\
         <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>";

    /// The port the listener connected and the result of the connection.
    type Connected = Option<(DdsRead<Track>, Result<(), ConnectorError>)>;

    /// Connects a `DDS_Read` port to `connector` at its first call.
    struct Connecting {
        connector: Arc<DdsState<Track>>,
        component: Component,
        heard: Mutex<Vec<Track>>,
        connected: Mutex<Connected>,
    }

    impl zerodds_dds4ccm::Listener<Track> for Connecting {
        fn on_one_data(&self, datum: &Track, _: &ReadInfo) {
            self.heard.lock().unwrap().push(datum.clone());
            let mut connected = self.connected.lock().unwrap();
            if connected.is_none() {
                let port = DdsRead::<Track>::new();
                let result = self.connector.passive_observer(&self.component, &port);
                *connected = Some((port, result));
            }
        }

        fn on_many_data(&self, data: &[Track], infos: &[ReadInfo]) {
            for (datum, info) in data.iter().zip(infos) {
                self.on_one_data(datum, info);
            }
        }
    }

    let d = Deployment::new();
    let qos_profile = profile(&format!(
        "<datawriter_qos>{DURABLE}</datawriter_qos><datareader_qos>{DURABLE}</datareader_qos>"
    ));
    let source = d.state(TOPIC, &qos_profile);
    // In the DomainParticipant of the port: the sample is in its
    // DataReader when the connector creates it.
    let observable = source.observable(&d.subscriber(), false).unwrap();
    observable
        .data()
        .create_one(&track(1, 10), HANDLE_NIL)
        .unwrap();

    let connector = Arc::new(d.state(TOPIC, &qos_profile));
    let component = d.subscriber();
    let listener = Arc::new(Connecting {
        connector: Arc::clone(&connector),
        component: component.clone(),
        heard: Mutex::new(Vec::new()),
        connected: Mutex::new(None),
    });
    let port = Arc::new(DdsListen::<Track>::new());
    port.connect_data_listener(listener.clone());
    port.data_control().set_mode(ListenerMode::OneByOne);
    let (done, connection) = mpsc::channel();
    {
        let (connector, port, listener) = (
            Arc::clone(&connector),
            Arc::clone(&port),
            Arc::clone(&listener),
        );
        std::thread::spawn(move || {
            let result = connector.push_observer(&component, &port);
            // What the listener heard while the connection ran.
            let heard = listener.heard.lock().unwrap().clone();
            let _ = done.send((result, heard));
        });
    }
    let (result, heard) = connection
        .recv_timeout(Duration::from_secs(20))
        .expect("the connection of the port waits for its own listener");
    result.unwrap();
    // The sample was in the DataReader when the port was connected: the
    // listener was called by the connection, on its thread.
    assert_eq!(heard, [track(1, 10)]);
    let connected = listener.connected.lock().unwrap().take();
    let (second, result) = connected.expect("the listener connected a port");
    result.unwrap();
    assert_eq!(second.dds_entity().unwrap().topic().name(), TOPIC);
    connector.remove().unwrap();
    source.remove().unwrap();
}
