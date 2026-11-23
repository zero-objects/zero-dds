// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §2 Conformance, the first conformance point: "A CCM framework claiming
//! conformance with this 'DDS for Lightweight CCM' specification shall
//! support DDS-DCPS normative ports and connectors and their
//! configuration."
//!
//! One deployment with everything the point names at once: two containers,
//! the two normative connectors `DDS_State` and `DDS_Event`, each of the
//! six normative port types, and the configuration by the attributes of
//! the connector (`domain_id`, `topic_name`, `key_fields`, a `qos_profile`
//! given as a file URL with a `topic_filter`) and by a threading policy of
//! the container. The other test files measure each clause on its own.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use std::sync::Arc;

use common::{Deployment, Heard, StateHeard, Statuses, Track, eventually, get_all, pairs, track};
use zerodds_dcps::HANDLE_NIL;
use zerodds_dcps::qos::{HistoryKind, ReliabilityKind};
use zerodds_dds4ccm::{
    DataListenerControl, DdsEvent, DdsGet, DdsListen, DdsRead, DdsState, DdsStateListen,
    ListenerMode, NonChangeable, QueryFilter, Reader, ThreadingPolicy, Updater, Writer,
};

const STATE_TOPIC: &str = "Dds4CcmConformanceState";
const EVENT_TOPIC: &str = "Dds4CcmConformanceEvent";

/// One QoS profile for both connectors. `topic_filter` allocates its
/// values (§7.4.2.3.2): the state topic keeps the last four samples of an
/// instance, the event topic keeps all; both are reliable.
fn profile_document() -> String {
    let reliable = "<reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>";
    let last_four = "<history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>4</depth></history>";
    let all = "<history><kind>KEEP_ALL_HISTORY_QOS</kind></history>";
    format!(
        "<dds xmlns=\"http://www.omg.org/dds/\">\
           <qos_profile name=\"Other\"/>\
           <qos_profile name=\"Conformance\">\
             <datawriter_qos topic_filter=\"*State\">{reliable}{last_four}</datawriter_qos>\
             <datareader_qos topic_filter=\"*State\">{reliable}{last_four}</datareader_qos>\
             <datawriter_qos topic_filter=\"*Event\">{reliable}{all}</datawriter_qos>\
             <datareader_qos topic_filter=\"*Event\">{reliable}{all}</datareader_qos>\
           </qos_profile>\
         </dds>"
    )
}

/// §2, conformance point 1, over DDS: a component of one container
/// publishes a state through `DDS_Update` and events through `DDS_Write`;
/// components of another container observe the state through `DDS_Read`,
/// `DDS_Get`, `DDS_Listen` and `DDS_StateListen` and consume the events
/// through `DDS_Get` and `DDS_Listen`. The connectors are configured by
/// their attributes, the entities have the QoS of the profile, the
/// listeners run on the thread pool of their container.
#[test]
fn dds4ccm_2_two_containers_exchange_state_and_events_through_the_normative_ports_and_connectors() {
    let d = Deployment::new();

    // Configuration: the profile as a file, named by a URL with the
    // profile as fragment.
    let dir = std::env::temp_dir().join(format!("dds4ccm-conformance-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("profiles.xml");
    std::fs::write(&file, profile_document()).unwrap();
    let path = file.display().to_string().replace('\\', "/");
    let slash = if path.starts_with('/') { "" } else { "/" };
    let qos_profile = format!("file://{slash}{path}#Conformance");

    // The threading policy of the subscribing container (§7.4.4).
    let policy = ThreadingPolicy::threads(2).unwrap();
    d.subscribing.set_threading_policy(Some(policy));

    // Each connector is deployed as a fragment on each side, configured by
    // domain_id, topic_name, key_fields and qos_profile.
    let state = |d: &Deployment| -> DdsState<Track> {
        let connector = DdsState::<Track>::new();
        connector.set_domain_id(d.domain).unwrap();
        connector.set_topic_name(STATE_TOPIC).unwrap();
        connector.set_key_fields(vec!["id".into()]).unwrap();
        connector.set_qos_profile(qos_profile.clone()).unwrap();
        connector.configuration_complete().unwrap();
        connector
    };
    let event = |d: &Deployment| -> DdsEvent<Track> {
        let connector = DdsEvent::<Track>::new();
        connector.set_domain_id(d.domain).unwrap();
        connector.set_topic_name(EVENT_TOPIC).unwrap();
        connector.set_key_fields(vec!["id".into()]).unwrap();
        connector.set_qos_profile(qos_profile.clone()).unwrap();
        connector.configuration_complete().unwrap();
        connector
    };
    let (state_out, state_in) = (state(&d), state(&d));
    let (event_out, event_in) = (event(&d), event(&d));
    // The configuration is fixed from here on.
    assert_eq!(state_out.set_domain_id(d.domain + 1), Err(NonChangeable));
    assert_eq!(event_in.set_topic_name("Another"), Err(NonChangeable));
    let errors = Arc::new(Statuses::default());
    state_in.connect_error_listener(errors.clone());
    event_in.connect_error_listener(errors.clone());

    // The subscribing components and their ports.
    let (monitor, logger) = (
        d.subscribing.component("monitor"),
        d.subscribing.component("logger"),
    );
    let passive = DdsRead::<Track>::new();
    let pull = DdsGet::<Track>::new();
    let push = DdsListen::<Track>::new();
    let push_state = DdsStateListen::<Track>::new();
    // The attribute `filter` of a port (§7.2.2.2): the passive observer
    // sees the first instance only.
    passive
        .set_filter(QueryFilter {
            expression: "id = %0".into(),
            parameters: vec!["1".into()],
        })
        .unwrap();
    let statuses = Arc::new(Statuses::default());
    passive.connect_status(statuses.clone());
    let heard = Arc::new(Heard::default());
    push.connect_data_listener(heard.clone());
    push.data_control().set_mode(ListenerMode::OneByOne);
    let state_heard = Arc::new(StateHeard::default());
    push_state.connect_data_listener(state_heard.clone());
    push_state.data_control().set_mode(ListenerMode::OneByOne);
    state_in.passive_observer(&monitor, &passive).unwrap();
    state_in.pull_observer(&monitor, &pull).unwrap();
    state_in.push_observer(&logger, &push).unwrap();
    state_in.push_state_observer(&logger, &push_state).unwrap();

    let pull_events = DdsGet::<Track>::new();
    let push_events = DdsListen::<Track>::new();
    let events_heard = Arc::new(Heard::default());
    push_events.connect_data_listener(events_heard.clone());
    push_events.data_control().set_mode(ListenerMode::OneByOne);
    event_in.pull_consumer(&monitor, &pull_events).unwrap();
    event_in.push_consumer(&logger, &push_events).unwrap();

    // The publishing component and its two ports.
    let source = d.publisher();
    let observable = state_out.observable(&source, false).unwrap();
    let supplier = event_out.supplier(&source).unwrap();

    // The entities are configured as the connector says: the domain, the
    // topic, and the QoS the profile allocates to the topic by its filter.
    let state_writer = observable.dds_entity();
    let event_writer = supplier.dds_entity();
    assert_eq!(state_writer.topic().name(), STATE_TOPIC);
    assert_eq!(event_writer.topic().name(), EVENT_TOPIC);
    let qos = state_writer.qos();
    assert_eq!(qos.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(
        (qos.history.kind, qos.history.depth),
        (HistoryKind::KeepLast, 4)
    );
    let qos = event_writer.qos();
    assert_eq!(qos.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(qos.history.kind, HistoryKind::KeepAll);
    let state_reader = passive.dds_entity().unwrap();
    let event_reader = pull_events.dds_entity().unwrap();
    assert_eq!(
        (
            state_reader.qos().history.kind,
            state_reader.qos().history.depth
        ),
        (HistoryKind::KeepLast, 4)
    );
    assert_eq!(event_reader.qos().history.kind, HistoryKind::KeepAll);
    for participant in [
        state_writer.get_publisher().get_participant().unwrap(),
        state_reader.get_subscriber().get_participant().unwrap(),
        event_reader.get_subscriber().get_participant().unwrap(),
    ] {
        assert_eq!(participant.domain_id(), d.domain);
    }

    state_writer
        .wait_for_matched_subscription(4, std::time::Duration::from_secs(15))
        .expect("the four observers match the observable");
    event_writer
        .wait_for_matched_subscription(2, std::time::Duration::from_secs(15))
        .expect("the two consumers match the supplier");

    // The state: two instances, the second deleted again.
    let updater = observable.data();
    updater.create_one(&track(1, 10), HANDLE_NIL).unwrap();
    updater.update_one(&track(1, 11), HANDLE_NIL).unwrap();
    updater.create_one(&track(2, 20), HANDLE_NIL).unwrap();
    updater.delete_one(&track(2, 0), HANDLE_NIL).unwrap();
    // The events.
    let sent = [(7, 1), (7, 2), (8, 1)];
    for (id, value) in sent {
        supplier
            .data()
            .write_one(&track(id, value), HANDLE_NIL)
            .unwrap();
    }

    // DDS_Read: the samples of the instance its filter lets through.
    let reader = passive.data().unwrap();
    let (mut data, mut infos) = (Vec::new(), Vec::new());
    eventually("the passive observer holds the state", || {
        reader.read_all(&mut data, &mut infos).unwrap();
        data.len() == 2
    });
    assert_eq!(pairs(&data), [(1, 10), (1, 11)]);
    // DDS_Get on the state: every change, and the deletion.
    assert_eq!(
        get_all(&*pull.fresh_data().unwrap(), 4),
        (vec![(1, 10), (1, 11), (2, 20)], vec![2])
    );
    // DDS_Listen on the state.
    eventually("the push observer is notified of every change", || {
        heard.one.lock().unwrap().len() >= 4
    });
    // DDS_StateListen: an operation per instance status.
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

    // DDS_Get and DDS_Listen on the events: every event, those of one
    // instance in the order they were sent.
    assert_eq!(
        get_all(&*pull_events.fresh_data().unwrap(), 3),
        (sent.to_vec(), Vec::new())
    );
    eventually("the push consumer is notified of every event", || {
        events_heard.one.lock().unwrap().len() == 3
    });
    let notified: Vec<Track> = events_heard
        .one
        .lock()
        .unwrap()
        .iter()
        .map(|(t, _)| t.clone())
        .collect();
    assert_eq!(pairs(&notified), sent);
    let of_seven: Vec<u32> = notified
        .iter()
        .filter(|t| t.id == 7)
        .map(|t| t.value)
        .collect();
    assert_eq!(of_seven, [1, 2]);

    // The listeners were called on the thread pool of their container.
    let of_container = [
        Some("subscribing-0".to_owned()),
        Some("subscribing-1".to_owned()),
    ];
    for called_on in [heard.thread_names(), events_heard.thread_names()] {
        assert!(!called_on.is_empty());
        assert!(
            called_on.iter().all(|name| of_container.contains(name)),
            "{called_on:?}"
        );
    }
    // Nothing was wrong: the port heard of no missed deadline and no lost
    // sample, the connectors of no inconsistent topic, no incompatible QoS,
    // no rejected sample and no missed deadline.
    assert_eq!(statuses.events(), Vec::<String>::new());
    for error in [
        "inconsistent_topic",
        "requested_incompatible_qos",
        "sample_rejected",
        "offered_deadline_missed",
        "offered_incompatible_qos",
    ] {
        assert!(!errors.has(error), "{error}: {:?}", errors.events());
    }

    drop((observable, supplier));
    for connector in [&state_out, &state_in] {
        connector.remove().unwrap();
    }
    for connector in [&event_out, &event_in] {
        connector.remove().unwrap();
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
