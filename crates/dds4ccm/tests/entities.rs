// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DCPS entities behind the ports (DDS4CCM 1.1 §7.4.1) and the use of
//! QoS profiles (§7.4.3): a DataReader or DataWriter per port; Subscriber,
//! Publisher and DomainParticipant of the port, the component or the
//! container; the profile of the connector, of a component and of a
//! container.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use std::sync::Arc;

use common::{Deployment, RELIABLE_KEEP_ALL, Track, eventually, isolated_cfg, profile, track};
use zerodds_dcps::error::return_code;
use zerodds_dcps::qos::{DurabilityKind, HistoryKind, ReliabilityKind};
use zerodds_dcps::{
    DataReader, DataWriter, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, Entity,
    HANDLE_NIL, InstanceHandle, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_dds4ccm::{
    Component, ConnectorError, Container, DdsGet, DdsListen, DdsRead, DdsState, DdsStateListen,
    DdsUpdate, LevelError, Reader, Updater,
};

/// The DataReader of a port, its Subscriber and its DomainParticipant.
fn reading(reader: &DataReader<Track>) -> (InstanceHandle, InstanceHandle, InstanceHandle) {
    let subscriber = reader.get_subscriber();
    let participant = subscriber.get_participant().unwrap();
    (
        reader.instance_handle(),
        subscriber.get_instance_handle(),
        participant.instance_handle(),
    )
}

/// The DataWriter of a port, its Publisher and its DomainParticipant.
fn writing(writer: &DataWriter<Track>) -> (InstanceHandle, InstanceHandle, InstanceHandle) {
    let publisher = writer.get_publisher();
    let participant = publisher.get_participant().unwrap();
    (
        writer.instance_handle(),
        publisher.get_instance_handle(),
        participant.instance_handle(),
    )
}

/// A `DDS_Read` port of `component` on `connector`, as the handles of
/// [`reading`].
fn read_port(
    connector: &DdsState<Track>,
    component: &Component,
) -> (InstanceHandle, InstanceHandle, InstanceHandle) {
    let port = DdsRead::<Track>::new();
    connector.passive_observer(component, &port).unwrap();
    reading(&port.dds_entity().unwrap())
}

/// A `DDS_Update` port of `component` on `connector`, as the handles of
/// [`writing`].
fn update_port(
    connector: &DdsState<Track>,
    component: &Component,
) -> (InstanceHandle, InstanceHandle, InstanceHandle) {
    let port: DdsUpdate<Track> = connector.observable(component, false).unwrap();
    writing(port.dds_entity())
}

/// The DomainParticipant a new `DDS_Read` port of `component` on
/// `connector` is placed in.
fn plain_reader_participant(
    connector: &DdsState<Track>,
    component: &Component,
) -> zerodds_dcps::DomainParticipant {
    let port = DdsRead::<Track>::new();
    connector.passive_observer(component, &port).unwrap();
    let subscriber = port.dds_entity().unwrap().get_subscriber();
    subscriber.get_participant().unwrap()
}

fn partition(name: &str) -> String {
    format!("<partition><name><element>{name}</element></name></partition>")
}

fn user_data(octet: u8) -> String {
    format!("<user_data><value>{octet}</value></user_data>")
}

/// §7.4.1: "the DDS ports are related to one data type and should
/// therefore be attached to one DataReader and/or DataWriter, which are
/// entirely dedicated to their port". The Subscriber, the Publisher and
/// the DomainParticipant are "shared by the components running in that
/// container" when nothing asks for more: without a profile the ports of
/// all components of a container have the container's.
#[test]
fn dds4ccm_7_4_1_each_port_has_its_reader_or_writer_and_shares_the_entities_of_the_container() {
    let d = Deployment::new();
    let connector = d.state("Dds4CcmAllocation", "");
    let first = d.subscribing.component("first");
    let second = d.subscribing.component("second");

    let read = DdsRead::<Track>::new();
    let get = DdsGet::<Track>::new();
    let listen = DdsListen::<Track>::new();
    let state_listen = DdsStateListen::<Track>::new();
    connector.passive_observer(&first, &read).unwrap();
    connector.pull_observer(&first, &get).unwrap();
    connector.push_observer(&second, &listen).unwrap();
    connector
        .push_state_observer(&second, &state_listen)
        .unwrap();
    let readers = [
        reading(&read.dds_entity().unwrap()),
        reading(&get.dds_entity().unwrap()),
        reading(&listen.dds_entity().unwrap()),
        reading(&state_listen.dds_entity().unwrap()),
    ];
    let writers = [
        update_port(&connector, &first),
        update_port(&connector, &second),
    ];

    // One DataReader, one DataWriter per port.
    for (at, (reader, ..)) in readers.iter().enumerate() {
        for (other, ..) in &readers[at + 1..] {
            assert_ne!(reader, other, "each port has a DataReader of its own");
        }
    }
    assert_ne!(writers[0].0, writers[1].0, "each port has its DataWriter");

    // One Subscriber, one Publisher, one DomainParticipant: the
    // container's.
    let (_, subscriber, participant) = readers[0];
    let (_, publisher, _) = writers[0];
    for reader in readers {
        assert_eq!((reader.1, reader.2), (subscriber, participant));
    }
    for writer in writers {
        assert_eq!((writer.1, writer.2), (publisher, participant));
    }

    // The components of another container have that container's.
    let elsewhere = read_port(&connector, &d.publishing.component("third"));
    assert_ne!(elsewhere.2, participant);

    // Another connector of the same domain uses the same default
    // entities, and a connector of another domain a DomainParticipant of
    // that domain.
    let other_topic = d.state("Dds4CcmAllocationOther", "");
    assert_eq!(read_port(&other_topic, &first).1, subscriber);
    assert_eq!(read_port(&other_topic, &first).2, participant);
    let other_domain = DdsState::<Track>::new();
    other_domain
        .set_domain_id(zerodds_test_domains::unique_domain())
        .unwrap();
    other_domain.set_topic_name("Dds4CcmAllocation").unwrap();
    other_domain.set_key_fields(vec!["id".into()]).unwrap();
    other_domain.configuration_complete().unwrap();
    let port = DdsRead::<Track>::new();
    other_domain.passive_observer(&first, &port).unwrap();
    let reader = port.dds_entity().unwrap();
    let of_domain = reader.get_subscriber().get_participant().unwrap();
    assert_eq!(of_domain.domain_id(), other_domain.domain_id());
    assert_ne!(of_domain.instance_handle(), participant);

    other_domain.remove().unwrap();
    other_topic.remove().unwrap();
    connector.remove().unwrap();
}

/// §7.4.3: the profile of the connector "should contain all values for
/// initializing DDS Entities that are required by the connector": topic,
/// DataWriter, DataReader, Publisher, Subscriber, DomainParticipant. The
/// values are read back from the entities.
#[test]
fn dds4ccm_7_4_3_the_connector_profile_initialises_every_entity() {
    let d = Deployment::new();
    let qos_profile = profile(&format!(
        "<topic_qos><durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability></topic_qos>\
         <datawriter_qos>\
           <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>\
           <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>\
         </datawriter_qos>\
         <datareader_qos>\
           <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>4</depth></history>\
         </datareader_qos>\
         <publisher_qos>{}</publisher_qos>\
         <subscriber_qos>{}</subscriber_qos>\
         <domainparticipant_qos>{}</domainparticipant_qos>",
        partition("left"),
        partition("right"),
        user_data(7),
    ));
    let connector = d.state("Dds4CcmInitialised", &qos_profile);
    let component = d.subscribing.component("component");

    let update = connector.observable(&component, false).unwrap();
    let writer = update.dds_entity();
    let read = DdsRead::<Track>::new();
    connector.passive_observer(&component, &read).unwrap();
    let reader = read.dds_entity().unwrap();

    let writer_qos = writer.qos();
    assert_eq!(writer_qos.reliability.kind, ReliabilityKind::BestEffort);
    assert_eq!(writer_qos.history.kind, HistoryKind::KeepAll);
    let reader_qos = reader.qos();
    assert_eq!(
        (reader_qos.history.kind, reader_qos.history.depth),
        (HistoryKind::KeepLast, 4)
    );
    for topic in [writer.topic(), reader.topic()] {
        assert_eq!(topic.qos().durability.kind, DurabilityKind::TransientLocal);
    }
    let publisher = writer.get_publisher();
    assert_eq!(publisher.get_qos().partition.names, ["left"]);
    let subscriber = reader.get_subscriber();
    assert_eq!(subscriber.get_qos().partition.names, ["right"]);
    for participant in [
        publisher.get_participant().unwrap(),
        subscriber.get_participant().unwrap(),
    ] {
        assert_eq!(participant.qos().user_data.value, [7]);
        assert_eq!(participant.domain_id(), d.domain);
    }
    // What the profile does not set is the default of DDS.
    assert_eq!(
        writer_qos.durability.kind,
        DataWriterQos::default().durability.kind
    );
    assert_eq!(
        publisher.get_qos().presentation,
        PublisherQos::default().presentation
    );

    drop(update);
    connector.remove().unwrap();
}

/// §7.4.3: default entities "should be used preferably if their setting
/// is compatible with the QoS requested in the connector's profile. If
/// they are not compatible, specific entities dedicated to the
/// 'non-compatible' port will be created."
#[test]
fn dds4ccm_7_4_3_a_port_the_defaults_do_not_suit_gets_dedicated_entities() {
    let d = Deployment::new();
    let component = d.subscribing.component("component");
    let plain = d.state("Dds4CcmDedicated", "");
    let (_, default_subscriber, default_participant) = read_port(&plain, &component);
    let (_, default_publisher, _) = update_port(&plain, &component);

    // A Subscriber and a Publisher in a partition: the defaults have
    // none. Each port gets its own, in the DomainParticipant that suits.
    let partitioned = d.state(
        "Dds4CcmDedicated",
        &profile(&format!(
            "<subscriber_qos>{0}</subscriber_qos><publisher_qos>{0}</publisher_qos>",
            partition("apart")
        )),
    );
    let first = read_port(&partitioned, &component);
    let second = read_port(&partitioned, &component);
    assert_eq!(
        (first.2, second.2),
        (default_participant, default_participant)
    );
    assert_ne!(first.1, default_subscriber);
    assert_ne!(second.1, default_subscriber);
    assert_ne!(first.1, second.1, "dedicated to the port, not shared");
    let writer = update_port(&partitioned, &component);
    assert_eq!(writer.2, default_participant);
    assert_ne!(writer.1, default_publisher);
    // The profile asks nothing of the Publisher in this connector: the
    // default serves.
    let subscriber_only = d.state(
        "Dds4CcmDedicated",
        &profile(&format!(
            "<subscriber_qos>{}</subscriber_qos>",
            partition("apart")
        )),
    );
    assert_eq!(
        update_port(&subscriber_only, &component).1,
        default_publisher
    );

    // A DomainParticipant with user data: the default has none. The port
    // gets its own, and in it a Subscriber of its own.
    let own_participant = d.state(
        "Dds4CcmDedicated",
        &profile(&format!(
            "<domainparticipant_qos>{}</domainparticipant_qos>",
            user_data(9)
        )),
    );
    let third = read_port(&own_participant, &component);
    let fourth = read_port(&own_participant, &component);
    assert_ne!(third.2, default_participant);
    assert_ne!(fourth.2, default_participant);
    assert_ne!(third.2, fourth.2, "dedicated to the port, not shared");
    // Removing a connector deletes what was dedicated to its ports: a
    // DomainParticipant that is deleted creates nothing any more.
    let port = DdsRead::<Track>::new();
    own_participant.passive_observer(&component, &port).unwrap();
    let subscriber = port.dds_entity().unwrap().get_subscriber();
    let of_port = subscriber.get_participant().unwrap();
    assert_ne!(of_port.instance_handle(), default_participant);
    assert!(
        of_port
            .try_create_publisher(PublisherQos::default())
            .is_ok()
    );
    own_participant.remove().unwrap();
    let deleted = of_port
        .try_create_publisher(PublisherQos::default())
        .unwrap_err();
    assert_eq!(
        deleted.as_return_code(),
        zerodds_dcps::error::return_code::ALREADY_DELETED
    );
    // And the Subscribers and Publishers dedicated to a port: the default
    // DomainParticipant keeps the two defaults of the container.
    let of_container = plain_reader_participant(&plain, &component);
    assert!(of_container.subscribers_len() > 1 && of_container.publishers_len() > 1);
    subscriber_only.remove().unwrap();
    partitioned.remove().unwrap();
    plain.remove().unwrap();
    assert_eq!(
        (
            of_container.subscribers_len(),
            of_container.publishers_len()
        ),
        (1, 1)
    );
}

/// §7.4.3: "specific entities dedicated to the 'non-compatible' port will
/// be created." A `DDS_Update` port with `is_global_scope` has a DataWriter
/// and a DataReader: one DomainParticipant dedicated to the port holds
/// both.
#[test]
fn dds4ccm_7_4_3_a_global_update_port_has_both_entities_in_its_dedicated_participant() {
    const TOPIC: &str = "Dds4CcmDedicatedUpdate";
    let d = Deployment::new();
    let component = d.subscribing.component("component");
    let plain = d.state(TOPIC, "");
    let (_, default_subscriber, default_participant) = read_port(&plain, &component);

    // A DomainParticipant with user data, which no default has, and a
    // Subscriber in a partition.
    let own_participant = d.state(
        TOPIC,
        &profile(&format!(
            "<domainparticipant_qos>{}</domainparticipant_qos>\
             <subscriber_qos>{}</subscriber_qos>",
            user_data(9),
            partition("observing")
        )),
    );
    let of_port = {
        let port = own_participant.observable(&component, true).unwrap();
        let updater = port.data();
        let observer = updater.observer().expect("the observer of the port");
        let (writer, reader) = (writing(port.dds_entity()), reading(observer));
        assert_ne!(writer.2, default_participant);
        assert_eq!(
            reader.2, writer.2,
            "one DomainParticipant for both entities"
        );
        let subscriber = observer.get_subscriber();
        assert_eq!(subscriber.get_qos().partition.names, ["observing"]);
        let participant = subscriber.get_participant().unwrap();
        assert_eq!(participant.qos().user_data.value, [9]);
        assert_eq!(
            (participant.publishers_len(), participant.subscribers_len()),
            (1, 1),
            "the Publisher and the Subscriber dedicated to the port"
        );
        // A second port of the kind has a DomainParticipant of its own.
        let second = own_participant.observable(&component, true).unwrap();
        let of_second = writing(second.dds_entity());
        assert_ne!(of_second.2, writer.2);
        let second_updater = second.data();
        assert_eq!(reading(second_updater.observer().unwrap()).2, of_second.2);
        participant
    };

    // A port the defaults suit has both entities in the default
    // DomainParticipant, the DataReader in the default Subscriber.
    {
        let shared = plain.observable(&component, true).unwrap();
        let updater = shared.data();
        let reader = reading(updater.observer().unwrap());
        assert_eq!(writing(shared.dds_entity()).2, default_participant);
        assert_eq!(
            (reader.1, reader.2),
            (default_subscriber, default_participant)
        );
    }

    // Removing the connector deletes the DomainParticipant of the port.
    own_participant.remove().unwrap();
    let deleted = of_port
        .try_create_publisher(PublisherQos::default())
        .unwrap_err();
    assert_eq!(deleted.as_return_code(), return_code::ALREADY_DELETED);
    plain.remove().unwrap();
}

/// §7.4.3: a profile of a component defines the "component's default
/// DomainParticipant, Subscriber, and/or Publisher"; "any topic_qos,
/// datareader_qos, or datawriter_qos is simply ignored". A profile of a
/// container defines the "container's defaults that should be used in
/// priority, if suitable".
#[test]
fn dds4ccm_7_4_3_the_defaults_of_the_container_come_first_then_those_of_the_component() {
    let d = Deployment::new();
    let container = &d.subscribing;
    container
        .set_qos_profile(&profile(&format!(
            "<subscriber_qos>{0}</subscriber_qos><publisher_qos>{0}</publisher_qos>",
            partition("container")
        )))
        .unwrap();
    let component = container.component("with profile");
    component
        .set_qos_profile(&profile(&format!(
            "<subscriber_qos>{0}</subscriber_qos><publisher_qos>{0}</publisher_qos>\
             <topic_qos><durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability></topic_qos>\
             <datareader_qos><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datareader_qos>\
             <datawriter_qos><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos>",
            partition("component")
        )))
        .unwrap();
    let plain = container.component("without profile");
    let requesting = |name: &str| {
        d.state(
            "Dds4CcmLevels",
            &profile(&format!(
                "<subscriber_qos>{0}</subscriber_qos><publisher_qos>{0}</publisher_qos>",
                partition(name)
            )),
        )
    };
    let partition_of_subscriber = |port: &DdsRead<Track>| {
        let subscriber = port.dds_entity().unwrap().get_subscriber();
        subscriber.get_qos().partition.names
    };

    // A connector that requests nothing: the container's defaults, for
    // the ports of both components.
    let silent = d.state("Dds4CcmLevels", "");
    let of_container = read_port(&silent, &component);
    assert_eq!(read_port(&silent, &plain).1, of_container.1);
    let writer_of_container = update_port(&silent, &component);
    assert_eq!(update_port(&silent, &plain).1, writer_of_container.1);

    // What the container's defaults have: the container's.
    let wants_container = requesting("container");
    assert_eq!(read_port(&wants_container, &component).1, of_container.1);
    assert_eq!(
        update_port(&wants_container, &component).1,
        writer_of_container.1
    );

    // What only the component's defaults have: the component's, shared by
    // the ports of the component, in the DomainParticipant of the
    // container.
    let wants_component = requesting("component");
    let port = DdsRead::<Track>::new();
    wants_component.passive_observer(&component, &port).unwrap();
    assert_eq!(partition_of_subscriber(&port), ["component"]);
    let of_component = reading(&port.dds_entity().unwrap());
    assert_ne!(of_component.1, of_container.1);
    assert_eq!(of_component.2, of_container.2);
    assert_eq!(read_port(&wants_component, &component).1, of_component.1);
    let writer_of_component = update_port(&wants_component, &component);
    assert_eq!(
        update_port(&wants_component, &component).1,
        writer_of_component.1
    );
    assert_ne!(writer_of_component.1, writer_of_container.1);
    // A component without that profile has no such default: dedicated.
    let dedicated = read_port(&wants_component, &plain);
    assert_ne!(dedicated.1, of_component.1);
    assert_ne!(dedicated.1, of_container.1);

    // What neither has: dedicated to the port.
    let wants_other = requesting("other");
    let first = read_port(&wants_other, &component);
    assert_ne!(first.1, of_container.1);
    assert_ne!(first.1, of_component.1);
    assert_ne!(first.1, read_port(&wants_other, &component).1);

    // topic_qos, datareader_qos and datawriter_qos of the component's
    // profile reach no entity: the connector's profile is silent, so they
    // have the defaults of DDS.
    let reader = port.dds_entity().unwrap();
    assert_eq!(reader.qos().history.kind, HistoryKind::KeepLast);
    assert_eq!(
        reader.topic().qos().durability.kind,
        DurabilityKind::Volatile
    );
    let update = wants_component.observable(&component, false).unwrap();
    assert_eq!(
        update.dds_entity().qos().history.kind,
        HistoryKind::KeepLast
    );
    drop(update);

    for connector in [silent, wants_container, wants_component, wants_other] {
        connector.remove().unwrap();
    }
}

/// §7.4.3: a component profile with a `domain_participant_qos` defines the
/// component's default DomainParticipant; it serves the ports the
/// container's does not suit, with a Publisher and a Subscriber in it.
#[test]
fn dds4ccm_7_4_3_a_component_profile_defines_the_default_participant_of_the_component() {
    let d = Deployment::new();
    let component = d.subscribing.component("own participant");
    component
        .set_qos_profile(&profile(&format!(
            "<domainparticipant_qos>{}</domainparticipant_qos>",
            user_data(5)
        )))
        .unwrap();
    let silent = d.state("Dds4CcmOwnParticipant", "");
    let wants_five = d.state(
        "Dds4CcmOwnParticipant",
        &profile(&format!(
            "<domainparticipant_qos>{}</domainparticipant_qos>",
            user_data(5)
        )),
    );

    // The container's default suits a connector that requests nothing.
    let of_container = read_port(&silent, &component);
    // The component's suits the one that requests its user data: shared
    // by the ports of the component, with one Subscriber and one
    // Publisher in it.
    let first = read_port(&wants_five, &component);
    let second = read_port(&wants_five, &component);
    assert_ne!(first.2, of_container.2);
    assert_eq!((first.1, first.2), (second.1, second.2));
    let writer = update_port(&wants_five, &component);
    assert_eq!(writer.2, first.2);
    assert_eq!(update_port(&wants_five, &component).1, writer.1);
    // A component without that profile: dedicated DomainParticipants.
    let plain = d.subscribing.component("plain");
    let dedicated = read_port(&wants_five, &plain);
    assert_ne!(dedicated.2, first.2);
    assert_ne!(dedicated.2, of_container.2);

    wants_five.remove().unwrap();
    silent.remove().unwrap();
}

/// §7.4.1: "components and containers can be given DDS entities that can
/// be used by the infrastructure for servicing embedded ports if they
/// meet the port requirements."
#[test]
fn dds4ccm_7_4_1_a_container_and_a_component_can_be_given_their_entities() {
    let domain = zerodds_test_domains::unique_domain();
    let cfg = isolated_cfg();
    let factory = DomainParticipantFactory::instance();
    let create = || {
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .unwrap()
    };
    let (of_container, of_component, foreign) = (create(), create(), create());
    let container = Container::with_runtime_config("given", cfg.clone());
    let component = container.component("given");
    let plain = container.component("plain");

    // The entities must belong together.
    let stray = foreign.create_publisher(PublisherQos::default());
    assert_eq!(
        container.set_default_entities(of_container.clone(), Some(Arc::new(stray)), None),
        Err(LevelError::ForeignEntity)
    );
    // The container: a DomainParticipant and a Subscriber; its Publisher
    // it creates itself. The component: all three, in a partition.
    let given_subscriber = Arc::new(of_container.create_subscriber(SubscriberQos::default()));
    let given_subscriber_handle = given_subscriber.get_instance_handle();
    container
        .set_default_entities(of_container.clone(), None, Some(given_subscriber.clone()))
        .unwrap();
    let in_partition = |name: &str| zerodds_dcps::qos::PartitionQosPolicy {
        names: vec![name.to_owned()],
    };
    let publisher = Arc::new(of_component.create_publisher(PublisherQos {
        partition: in_partition("given"),
        ..PublisherQos::default()
    }));
    let subscriber = Arc::new(of_component.create_subscriber(SubscriberQos {
        partition: in_partition("given"),
        ..SubscriberQos::default()
    }));
    let given = (
        publisher.get_instance_handle(),
        subscriber.get_instance_handle(),
    );
    component
        .set_default_entities(
            of_component.clone(),
            Some(publisher.clone()),
            Some(subscriber.clone()),
        )
        .unwrap();

    let state = |qos_profile: &str| {
        let connector = DdsState::<Track>::new();
        connector.set_domain_id(domain).unwrap();
        connector.set_topic_name("Dds4CcmGiven").unwrap();
        connector.set_key_fields(vec!["id".into()]).unwrap();
        connector.set_qos_profile(qos_profile).unwrap();
        connector.configuration_complete().unwrap();
        connector
    };
    // A connector that requests nothing is served by what the container
    // was given, and by the Publisher the container creates in it.
    let silent = state("");
    let reader = read_port(&silent, &component);
    assert_eq!(
        (reader.1, reader.2),
        (given_subscriber_handle, of_container.instance_handle())
    );
    assert_eq!(read_port(&silent, &plain).1, given_subscriber_handle);
    let writer = update_port(&silent, &plain);
    assert_eq!(writer.2, of_container.instance_handle());
    assert_eq!(update_port(&silent, &component).1, writer.1);
    // The entities are in use: they cannot be exchanged.
    assert_eq!(
        container.set_default_entities(foreign.clone(), None, None),
        Err(LevelError::NonChangeable)
    );
    assert_eq!(
        container.set_qos_profile(""),
        Err(LevelError::NonChangeable)
    );

    // A connector that requests the partition is served by what the
    // component was given, since the container's do not meet it; in a
    // component that was given nothing the port gets its own, in the
    // DomainParticipant of the container.
    let partitioned = state(&profile(&format!(
        "<publisher_qos>{0}</publisher_qos><subscriber_qos>{0}</subscriber_qos>",
        partition("given")
    )));
    let reader = read_port(&partitioned, &component);
    assert_eq!(
        (reader.1, reader.2),
        (given.1, of_component.instance_handle())
    );
    let writer = update_port(&partitioned, &component);
    assert_eq!(
        (writer.1, writer.2),
        (given.0, of_component.instance_handle())
    );
    let dedicated = read_port(&partitioned, &plain);
    assert_eq!(dedicated.2, of_container.instance_handle());
    assert_ne!(dedicated.1, given_subscriber_handle);

    // Removing the connectors and the container deletes what they
    // created and leaves what was given.
    partitioned.remove().unwrap();
    silent.remove().unwrap();
    container.remove().unwrap();
    assert_eq!(
        (
            of_container.subscribers_len(),
            of_container.publishers_len()
        ),
        (1, 0),
        "the given Subscriber stays, the created Publisher went"
    );
    assert_eq!(
        (
            of_component.subscribers_len(),
            of_component.publishers_len()
        ),
        (1, 1),
        "what the component was given stays"
    );
    // What was given still works for who gave it.
    let topic = of_container
        .create_topic::<Track>("Dds4CcmGivenAfter", zerodds_dcps::TopicQos::default())
        .unwrap();
    let reader = given_subscriber.create_datareader(&topic, zerodds_dcps::DataReaderQos::default());
    assert!(reader.is_ok());
    drop((reader, publisher, subscriber, given_subscriber));
    for participant in [of_container, of_component, foreign] {
        participant.delete_contained_entities().unwrap();
        factory.delete_participant(&participant).unwrap();
    }
}

/// `Container::new`: the DomainParticipants the container creates run with
/// the standard runtime configuration, and carry the data of its ports.
#[test]
fn dds4ccm_7_4_1_a_container_creates_its_participants_with_the_standard_runtime_configuration() {
    use zerodds_dcps::runtime::RuntimeConfig;

    let domain = zerodds_test_domains::unique_domain();
    let container = Container::new("standard");
    container
        .set_qos_profile(&profile(&format!(
            "<domainparticipant_qos>{}</domainparticipant_qos>",
            user_data(4)
        )))
        .unwrap();
    let component = container.component("component");
    let connector = DdsState::<Track>::new();
    connector.set_domain_id(domain).unwrap();
    connector.set_topic_name("Dds4CcmStandardRuntime").unwrap();
    connector.set_key_fields(vec!["id".into()]).unwrap();
    connector
        .set_qos_profile(profile(RELIABLE_KEEP_ALL))
        .unwrap();
    connector.configuration_complete().unwrap();

    let port = DdsRead::<Track>::new();
    connector.passive_observer(&component, &port).unwrap();
    let observable = connector.observable(&component, false).unwrap();
    let subscriber = port.dds_entity().unwrap().get_subscriber();
    let participant = subscriber.get_participant().unwrap();
    assert_eq!(participant.domain_id(), domain);
    assert_eq!(participant.qos().user_data.value, [4]);
    assert_eq!(
        writing(observable.dds_entity()).2,
        participant.instance_handle()
    );
    let standard = RuntimeConfig::from_standard_paths().unwrap();
    let runtime = participant
        .runtime()
        .expect("the runtime of the participant");
    assert_eq!(
        (
            runtime.config.tick_period,
            runtime.config.spdp_period,
            runtime.config.spdp_multicast_group
        ),
        (
            standard.tick_period,
            standard.spdp_period,
            standard.spdp_multicast_group
        )
    );

    observable
        .dds_entity()
        .wait_for_matched_subscription(1, std::time::Duration::from_secs(15))
        .expect("the port matches the observable");
    let created = observable.data().create_one(&track(3, 30), HANDLE_NIL);
    created.unwrap();
    let reader = port.data().unwrap();
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    eventually("the port reads what was published", || {
        reader.read_all(&mut data, &mut infos).unwrap();
        data.len() == 1
    });
    assert_eq!(data, [track(3, 30)]);

    drop(observable);
    connector.remove().unwrap();
    container.remove().unwrap();
    let deleted = participant
        .try_create_publisher(PublisherQos::default())
        .unwrap_err();
    assert_eq!(deleted.as_return_code(), return_code::ALREADY_DELETED);
}

/// A DomainParticipant given to a component serves the ports whose
/// connector requests what only it has.
#[test]
fn dds4ccm_7_4_1_a_given_participant_serves_the_ports_it_suits() {
    let domain = zerodds_test_domains::unique_domain();
    let cfg = isolated_cfg();
    let factory = DomainParticipantFactory::instance();
    let with_user_data = DomainParticipantQos {
        user_data: zerodds_dcps::qos::UserDataQosPolicy { value: vec![3] },
        ..DomainParticipantQos::default()
    };
    let given = factory
        .create_participant_with_config(domain, with_user_data, cfg.clone())
        .unwrap();
    let container = Container::with_runtime_config("container", cfg);
    let component = container.component("component");
    component
        .set_default_entities(given.clone(), None, None)
        .unwrap();

    let connector = DdsState::<Track>::new();
    connector.set_domain_id(domain).unwrap();
    connector.set_topic_name("Dds4CcmGivenParticipant").unwrap();
    connector.set_key_fields(vec!["id".into()]).unwrap();
    connector
        .set_qos_profile(profile(&format!(
            "<domainparticipant_qos>{}</domainparticipant_qos>",
            user_data(3)
        )))
        .unwrap();
    connector.configuration_complete().unwrap();

    // The container's default has no user data; the component's given
    // DomainParticipant has what the connector requests. The component
    // creates its Subscriber and Publisher in it, once.
    let first = read_port(&connector, &component);
    let second = read_port(&connector, &component);
    assert_eq!(first.2, given.instance_handle());
    assert_eq!(first.1, second.1);
    let writer = update_port(&connector, &component);
    assert_eq!(writer.2, given.instance_handle());

    connector.remove().unwrap();
    container.remove().unwrap();
    // The given DomainParticipant is still there, without the entities
    // the component created in it.
    assert_eq!((given.publishers_len(), given.subscribers_len()), (0, 0));
    factory.delete_participant(&given).unwrap();
}

/// A profile a component or a container cannot read is refused with the
/// reason; a port before `configuration_complete` of its connector and a
/// port connected twice are refused.
#[test]
fn dds4ccm_7_4_3_what_cannot_be_configured_is_refused() {
    let d = Deployment::new();
    let component = d.subscribing.component("component");
    assert!(matches!(
        component.set_qos_profile("qos.xml"),
        Err(LevelError::QosProfile(_))
    ));
    assert!(matches!(
        d.subscribing.set_qos_profile("<dds><qos_profile/></dds>"),
        Err(LevelError::QosProfile(_))
    ));
    assert_eq!(component.name(), "component");
    assert_eq!(d.subscribing.name(), "subscribing");

    let connector = d.state("Dds4CcmRefused", "");
    let port = DdsRead::<Track>::new();
    connector.passive_observer(&component, &port).unwrap();
    // The port is configured: a second connection is PRECONDITION_NOT_MET.
    let again = connector.passive_observer(&component, &port).unwrap_err();
    assert!(
        matches!(again, ConnectorError::Dds(error)
            if error.error_code == zerodds_dcps::error::return_code::PRECONDITION_NOT_MET),
        "{again:?}"
    );
    // The profile of a level whose defaults serve a port is fixed.
    assert_eq!(
        d.subscribing.set_qos_profile(""),
        Err(LevelError::NonChangeable)
    );
    connector.remove().unwrap();
    // After its connectors the container is removed, and then its
    // defaults can be configured again.
    d.subscribing.remove().unwrap();
    d.subscribing.set_qos_profile("").unwrap();
}

/// A port connection that is refused keeps nothing of what it created:
/// the Publisher or Subscriber dedicated to the port and the topic the
/// connector created for it are deleted again, and the connector serves
/// the next port as if the refused one had not been tried.
#[test]
fn dds4ccm_7_4_3_a_refused_port_keeps_no_entity_of_the_connector() {
    let d = Deployment::new();
    let component = d.subscribing.component("component");
    let plain = d.state("Dds4CcmRefusedKeepsNothing", "");
    let port = DdsRead::<Track>::new();
    plain.passive_observer(&component, &port).unwrap();
    // The default Publisher of the container is created with the first
    // publishing port; it is the container's and stays.
    let update = plain.observable(&component, false).unwrap();
    let participant = port
        .dds_entity()
        .unwrap()
        .get_subscriber()
        .get_participant()
        .unwrap();
    assert_eq!(
        (participant.publishers_len(), participant.subscribers_len()),
        (1, 1),
        "the default Publisher and Subscriber of the container"
    );
    let counts = || {
        (
            participant.publishers_len(),
            participant.subscribers_len(),
            participant.topics_len(),
        )
    };
    let before = counts();

    // The port is configured: the connection to a second connector is
    // refused after that connector placed it in a dedicated Subscriber
    // and created its topic in the DomainParticipant.
    const OTHER: &str = "Dds4CcmRefusedKeepsNothingOther";
    let partitioned = d.state(
        OTHER,
        &profile(&format!(
            "<subscriber_qos>{0}</subscriber_qos><publisher_qos>{0}</publisher_qos>",
            partition("apart")
        )),
    );
    let again = partitioned.passive_observer(&component, &port).unwrap_err();
    assert!(
        matches!(&again, ConnectorError::Dds(error)
            if error.error_code == return_code::PRECONDITION_NOT_MET),
        "{again:?}"
    );
    assert_eq!(counts(), before, "the refused port kept an entity");
    // The topic of the refused port is gone from the connector as well:
    // the next port gets a topic that exists.
    let next = DdsRead::<Track>::new();
    partitioned.passive_observer(&component, &next).unwrap();
    assert_eq!(next.dds_entity().unwrap().topic().name(), OTHER);
    assert_eq!(
        counts(),
        (before.0, before.1 + 1, before.2 + 1),
        "one Subscriber and one topic for the port that was served"
    );
    partitioned.remove().unwrap();
    assert_eq!(counts(), before);

    // A topic that exists with another QoS refuses the connector after
    // the Publisher or Subscriber dedicated to the port was created.
    const OF_APPLICATION: &str = "Dds4CcmRefusedKeepsNothingOfApplication";
    let mut qos = TopicQos::default();
    qos.topic_data.value = vec![7];
    let of_application = participant
        .create_topic::<Track>(OF_APPLICATION, qos)
        .unwrap();
    let with_application = counts();
    let asking = d.state(
        OF_APPLICATION,
        &profile(&format!(
            "<topic_qos><durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability></topic_qos>\
             <subscriber_qos>{0}</subscriber_qos><publisher_qos>{0}</publisher_qos>",
            partition("apart")
        )),
    );
    let refused = DdsRead::<Track>::new();
    assert!(matches!(
        asking.passive_observer(&component, &refused),
        Err(ConnectorError::TopicQos { .. })
    ));
    assert!(matches!(
        asking.observable(&component, true),
        Err(ConnectorError::TopicQos { .. })
    ));
    assert!(refused.dds_entity().is_none());
    assert_eq!(
        counts(),
        with_application,
        "the refused ports kept an entity"
    );
    asking.remove().unwrap();
    participant.delete_topic(&of_application).unwrap();
    drop(update);
    plain.remove().unwrap();
}

/// A port removed by itself is removed: the removal of its connector after
/// it, and a second removal of the port, have nothing left to delete and
/// succeed.
#[test]
fn dds4ccm_7_4_1_a_port_removed_before_its_connector_leaves_the_connector_removable() {
    let d = Deployment::new();
    let component = d.subscribing.component("component");
    let connector = d.state("Dds4CcmPortRemovedFirst", "");
    let port = DdsListen::<Track>::new();
    connector.push_observer(&component, &port).unwrap();
    let reader = port.dds_entity().unwrap();
    port.remove().unwrap();
    assert!(matches!(
        reader.take(),
        Err(zerodds_dcps::DdsError::AlreadyDeleted)
    ));
    port.remove().unwrap();
    connector.remove().unwrap();
    // The container keeps no reader of the port: it can be removed.
    d.subscribing.remove().unwrap();
}

/// A removal that fails deletes nothing it could not delete, and the next
/// removal tries it again: the port, its connector and its container are
/// removable once the application gives up what kept the DataReader. Here
/// it is a ReadCondition the application created on `dds_entity`, which
/// DDS 1.4 §2.2.2.5.2.6 lets no DataReader be deleted with.
#[test]
fn dds4ccm_7_4_1_a_removal_that_failed_is_tried_again() {
    use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
    let held = |reader: &DataReader<Track>| {
        reader
            .create_readcondition(
                sample_state_mask::ANY,
                view_state_mask::ANY,
                instance_state_mask::ANY,
            )
            .unwrap()
    };
    let refused = |result: Result<(), zerodds_dds4ccm::ccm_dds::InternalError>| {
        assert_eq!(
            result.unwrap_err().error_code,
            return_code::PRECONDITION_NOT_MET
        );
    };
    let deleted = |reader: &DataReader<Track>| {
        matches!(reader.take(), Err(zerodds_dcps::DdsError::AlreadyDeleted))
    };
    let d = Deployment::new();
    let component = d.subscribing.component("component");

    // The port removes itself; its DataReader is in the default Subscriber
    // of the container.
    let connector = d.state("Dds4CcmRemovalTriedAgain", "");
    let port = DdsListen::<Track>::new();
    connector.push_observer(&component, &port).unwrap();
    let reader = port.dds_entity().unwrap();
    let condition = held(&reader);
    refused(port.remove());
    // Not removed: the second call fails as the first did, and so does
    // the connector, whose first step is the removal of the port.
    refused(port.remove());
    refused(connector.remove());
    assert!(!deleted(&reader));
    refused(d.subscribing.remove());
    reader.delete_readcondition(&condition).unwrap();
    port.remove().unwrap();
    assert!(deleted(&reader));
    port.remove().unwrap();
    connector.remove().unwrap();

    // The connector removes the port; its DataReader is in a Subscriber
    // dedicated to it.
    let partitioned = d.state(
        "Dds4CcmRemovalTriedAgainSubscriber",
        &profile(&format!(
            "<subscriber_qos>{}</subscriber_qos>",
            partition("apart")
        )),
    );
    let port = DdsRead::<Track>::new();
    partitioned.passive_observer(&component, &port).unwrap();
    let reader = port.dds_entity().unwrap();
    let participant = reader.get_subscriber().get_participant().unwrap();
    let subscribers = participant.subscribers_len();
    let condition = held(&reader);
    refused(partitioned.remove());
    refused(partitioned.remove());
    assert!(!deleted(&reader));
    assert_eq!(
        participant.subscribers_len(),
        subscribers,
        "the Subscriber of the port stays with its DataReader"
    );
    reader.delete_readcondition(&condition).unwrap();
    partitioned.remove().unwrap();
    assert!(deleted(&reader));
    assert_eq!(participant.subscribers_len(), subscribers - 1);
    port.remove().unwrap();

    // The connector removes the port; its DataReader is in a
    // DomainParticipant dedicated to it. The DomainParticipant goes with
    // what it contains at the first removal, which reports the error of
    // the port; the second has nothing left to delete.
    let own_participant = d.state(
        "Dds4CcmRemovalTriedAgainParticipant",
        &profile(&format!(
            "<domainparticipant_qos>{}</domainparticipant_qos>",
            user_data(9)
        )),
    );
    let port = DdsRead::<Track>::new();
    own_participant.passive_observer(&component, &port).unwrap();
    let reader = port.dds_entity().unwrap();
    let _condition = held(&reader);
    refused(own_participant.remove());
    assert!(deleted(&reader));
    own_participant.remove().unwrap();

    d.subscribing.remove().unwrap();
}

/// `Container::remove` while a port still has its DataReader or DataWriter
/// in a default entity of the container is `PRECONDITION_NOT_MET`: nothing
/// is deleted, the port works on and the defaults stay the defaults.
#[test]
fn dds4ccm_7_4_1_a_container_whose_defaults_serve_a_port_is_not_removed() {
    const TOPIC: &str = "Dds4CcmContainerInUse";
    let d = Deployment::new();
    let qos_profile = profile(RELIABLE_KEEP_ALL);
    let container = &d.subscribing;
    let component = container.component("component");
    let refused = || {
        let error = container.remove().unwrap_err();
        assert_eq!(error.error_code, return_code::PRECONDITION_NOT_MET);
    };

    let connector = d.state(TOPIC, &qos_profile);
    let port = DdsRead::<Track>::new();
    connector.passive_observer(&component, &port).unwrap();
    let before = reading(&port.dds_entity().unwrap());
    let subscriber = port.dds_entity().unwrap().get_subscriber();
    let participant = subscriber.get_participant().unwrap();
    // The default Publisher of the container, which no port uses any
    // more: the connector of its only DataWriter is removed.
    let gone = d.state("Dds4CcmContainerInUseGone", "");
    update_port(&gone, &component);
    gone.remove().unwrap();
    let defaults = || (participant.subscribers_len(), participant.publishers_len());
    assert_eq!(defaults(), (1, 1));

    // A port has its DataReader in the default Subscriber.
    refused();
    assert_eq!(defaults(), (1, 1), "neither default was deleted");
    // The defaults are still the defaults, and still fixed.
    assert_eq!(read_port(&connector, &component).1, before.1);
    assert_eq!(update_port(&connector, &component).2, before.2);
    assert_eq!(
        container.set_qos_profile(""),
        Err(LevelError::NonChangeable)
    );
    // The DataReader of the port still gets what is published.
    let source = d.state(TOPIC, &qos_profile);
    let observable = source.observable(&d.publisher(), false).unwrap();
    observable
        .dds_entity()
        .wait_for_matched_subscription(2, std::time::Duration::from_secs(15))
        .expect("the two ports match the observable");
    let created = observable.data().create_one(&track(1, 10), HANDLE_NIL);
    created.unwrap();
    let reader = port.data().unwrap();
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    eventually("the port reads what was published", || {
        reader.read_all(&mut data, &mut infos).unwrap();
        data.len() == 1
    });
    assert_eq!(data, [track(1, 10)]);
    connector.remove().unwrap();
    assert_eq!(defaults(), (1, 1));

    // A port has its DataReader in a Subscriber dedicated to it, in the
    // default DomainParticipant.
    let partitioned = d.state(
        TOPIC,
        &profile(&format!(
            "<subscriber_qos>{}</subscriber_qos>",
            partition("apart")
        )),
    );
    let dedicated = read_port(&partitioned, &component);
    assert_eq!(dedicated.2, before.2);
    assert_ne!(dedicated.1, before.1);
    refused();
    assert_eq!(defaults(), (2, 1), "the port keeps its Subscriber");
    partitioned.remove().unwrap();

    // No port any more, and a topic of the application in the default
    // DomainParticipant: its default Publisher and Subscriber go, the
    // DomainParticipant stays the default until the topic is deleted.
    let topic = participant
        .create_topic::<Track>("Dds4CcmContainerInUseTopic", TopicQos::default())
        .unwrap();
    refused();
    assert_eq!(defaults(), (0, 0));
    assert_eq!(
        container.set_qos_profile(""),
        Err(LevelError::NonChangeable)
    );
    let kept = plain_reader_participant(&gone, &component);
    assert_eq!(kept.instance_handle(), before.2);
    gone.remove().unwrap();
    participant.delete_topic(&topic).unwrap();
    container.remove().unwrap();
    let deleted = participant
        .try_create_publisher(PublisherQos::default())
        .unwrap_err();
    assert_eq!(deleted.as_return_code(), return_code::ALREADY_DELETED);
    container.set_qos_profile("").unwrap();

    drop(observable);
    source.remove().unwrap();
}
