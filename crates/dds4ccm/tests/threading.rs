// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The threading policy (DDS4CCM 1.1 §7.4.4): set at the port, the
//! component or the container; the thread pool of the port is used, "if
//! there is no port's policy, the component's threadpool is used", "if
//! there is no component's one, the container's threadpool is used", "if
//! there is no container's policy, then the default is applied".
//!
//! Each test records the thread a facet is called on.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use std::sync::Arc;

use common::{Deployment, Heard, RELIABLE_KEEP_ALL, Statuses, Track, eventually, profile, track};
use zerodds_dcps::HANDLE_NIL;
use zerodds_dds4ccm::{
    Component, ConnectorStatusListener, DataListenerControl, DdsEvent, DdsListen, DdsWrite,
    ListenerMode, NonChangeable, PortStatusListener, ThreadingPolicy, Writer,
};

/// A `DDS_Listen` port of `component` on `connector` with the threading
/// policy `policy`, its listener enabled one by one.
fn listening(
    connector: &DdsEvent<Track>,
    component: &Component,
    policy: Option<ThreadingPolicy>,
) -> (DdsListen<Track>, Arc<Heard>) {
    let port = DdsListen::<Track>::new();
    port.set_threading_policy(policy).unwrap();
    assert_eq!(port.threading_policy(), policy);
    let heard = Arc::new(Heard::default());
    port.connect_data_listener(heard.clone());
    port.data_control().set_mode(ListenerMode::OneByOne);
    connector.push_consumer(component, &port).unwrap();
    (port, heard)
}

/// The supplier of the publishing side, matched with `readers` ports.
fn supplier(d: &Deployment, connector: &DdsEvent<Track>, readers: usize) -> DdsWrite<Track> {
    let supplier = connector.supplier(&d.publisher()).unwrap();
    supplier
        .dds_entity()
        .wait_for_matched_subscription(readers, std::time::Duration::from_secs(15))
        .expect("the ports match the supplier");
    supplier
}

/// Sends `count` events and waits until each of `heard` got them.
fn send(supplier: &DdsWrite<Track>, count: u32, heard: &[&Arc<Heard>]) {
    for value in 0..count {
        supplier
            .data()
            .write_one(&track(1, value), HANDLE_NIL)
            .unwrap();
    }
    eventually("every listener got the events", || {
        let got = |heard: &&Arc<Heard>| heard.one.lock().unwrap().len() >= count as usize;
        heard.iter().all(got)
    });
}

fn names(names: &[&str]) -> Vec<Option<String>> {
    names.iter().map(|name| Some((*name).to_owned())).collect()
}

fn two_sides(d: &Deployment, topic: &str) -> (DdsEvent<Track>, DdsEvent<Track>) {
    let qos_profile = profile(RELIABLE_KEEP_ALL);
    (d.event(topic, &qos_profile), d.event(topic, &qos_profile))
}

/// The policy of the port: "port (for all its facets)". It wins over the
/// policies of the component and the container, and the port has a pool
/// of its own.
#[test]
fn dds4ccm_7_4_4_a_port_policy_wins() {
    let d = Deployment::new();
    let (supplying, consuming) = two_sides(&d, "Dds4CcmThreadsPort");
    d.subscribing
        .set_threading_policy(ThreadingPolicy::threads(2));
    let component = d.subscribing.component("sink");
    component.set_threading_policy(Some(ThreadingPolicy::SINGLE));

    let (port, heard) = listening(&consuming, &component, Some(ThreadingPolicy::SINGLE));
    let (other, other_heard) = listening(&consuming, &component, Some(ThreadingPolicy::SINGLE));
    let pool = port.thread_pool().expect("the port has a pool");
    assert_eq!(pool.name(), "dds4ccm-port");
    assert_eq!(pool.policy(), ThreadingPolicy::SINGLE);
    assert!(
        !Arc::ptr_eq(&pool, &other.thread_pool().unwrap()),
        "a pool per port"
    );

    let supplier = supplier(&d, &supplying, 2);
    send(&supplier, 5, &[&heard, &other_heard]);
    assert_eq!(heard.thread_names(), names(&["dds4ccm-port-0"]));
    assert_eq!(other_heard.thread_names(), names(&["dds4ccm-port-0"]));
    // The two ports are called on two threads: each on its pool.
    let thread_of = |heard: &Heard| heard.threads.lock().unwrap()[0].0;
    assert_ne!(thread_of(&heard), thread_of(&other_heard));

    // The pool is chosen when the facets are activated.
    assert_eq!(port.set_threading_policy(None), Err(NonChangeable));
    assert_eq!(
        port.threading_policy(),
        Some(ThreadingPolicy::SINGLE),
        "the policy is kept"
    );

    drop(supplier);
    supplying.remove().unwrap();
    consuming.remove().unwrap();
}

/// "If there is no port's policy, the component's threadpool is used":
/// one pool for the ports of the component, also when the container has
/// a policy.
#[test]
fn dds4ccm_7_4_4_without_a_port_policy_the_pool_of_the_component_is_used() {
    let d = Deployment::new();
    let (supplying, consuming) = two_sides(&d, "Dds4CcmThreadsComponent");
    d.subscribing
        .set_threading_policy(ThreadingPolicy::threads(2));
    let component = d.subscribing.component("sink");
    component.set_threading_policy(Some(ThreadingPolicy::SINGLE));
    assert_eq!(component.threading_policy(), Some(ThreadingPolicy::SINGLE));

    let (first, first_heard) = listening(&consuming, &component, None);
    let (second, second_heard) = listening(&consuming, &component, None);
    let pool = first.thread_pool().expect("the component has a pool");
    assert_eq!(pool.name(), "subscribing.sink");
    assert!(Arc::ptr_eq(&pool, &second.thread_pool().unwrap()));

    let supplier = supplier(&d, &supplying, 2);
    send(&supplier, 5, &[&first_heard, &second_heard]);
    assert_eq!(first_heard.thread_names(), names(&["subscribing.sink-0"]));
    assert_eq!(second_heard.thread_names(), names(&["subscribing.sink-0"]));

    drop(supplier);
    supplying.remove().unwrap();
    consuming.remove().unwrap();
}

/// "If there is no component's one, the container's threadpool is used":
/// one pool for the ports of all components of the container.
#[test]
fn dds4ccm_7_4_4_without_a_component_policy_the_pool_of_the_container_is_used() {
    let d = Deployment::new();
    let (supplying, consuming) = two_sides(&d, "Dds4CcmThreadsContainer");
    let policy = ThreadingPolicy::threads(2).unwrap();
    d.subscribing.set_threading_policy(Some(policy));
    assert_eq!(d.subscribing.threading_policy(), Some(policy));
    let (first, second) = (
        d.subscribing.component("first"),
        d.subscribing.component("second"),
    );
    assert_eq!(first.threading_policy(), None);

    let (first_port, first_heard) = listening(&consuming, &first, None);
    let (second_port, second_heard) = listening(&consuming, &second, None);
    let pool = first_port.thread_pool().expect("the container has a pool");
    assert_eq!((pool.name(), pool.policy()), ("subscribing", policy));
    assert!(Arc::ptr_eq(&pool, &second_port.thread_pool().unwrap()));

    let supplier = supplier(&d, &supplying, 2);
    send(&supplier, 20, &[&first_heard, &second_heard]);
    let of_container = names(&["subscribing-0", "subscribing-1"]);
    for heard in [&first_heard, &second_heard] {
        let called_on = heard.thread_names();
        assert!(!called_on.is_empty());
        assert!(
            called_on.iter().all(|name| of_container.contains(name)),
            "{called_on:?}"
        );
        // Whatever thread of the pool calls: the events of the port keep
        // their order.
        let values: Vec<u32> = heard
            .one
            .lock()
            .unwrap()
            .iter()
            .map(|(t, _)| t.value)
            .collect();
        assert_eq!(values, (0..20).collect::<Vec<u32>>());
    }

    drop(supplier);
    supplying.remove().unwrap();
    consuming.remove().unwrap();
}

/// "If there is no container's policy, then the default is applied": no
/// pool. A facet is called on the thread that brings the event: a thread
/// of DDS for data that arrive, the thread of the caller for data the
/// port holds when its listener is enabled.
#[test]
fn dds4ccm_7_4_4_without_any_policy_the_default_applies() {
    let d = Deployment::new();
    let (supplying, consuming) = two_sides(&d, "Dds4CcmThreadsDefault");
    let component = d.subscribing.component("sink");
    assert_eq!(d.subscribing.threading_policy(), None);

    let (port, heard) = listening(&consuming, &component, None);
    assert!(port.thread_pool().is_none());
    let supplier = supplier(&d, &supplying, 1);
    send(&supplier, 3, &[&heard]);
    let here = std::thread::current().id();
    // The thread DDS reports the data on: no thread of a pool. (For a
    // writer of the same process that can be the thread that writes.)
    for (_, name) in heard.threads.lock().unwrap().iter() {
        let name = name.clone().unwrap_or_default();
        assert!(
            !name.starts_with("subscribing") && !name.starts_with("dds4ccm-port"),
            "no pool: {name}"
        );
    }

    // Data the port holds: delivered when the listener is enabled, before
    // `set_mode` returns, on this thread.
    port.data_control().set_mode(ListenerMode::NotEnabled);
    supplier
        .data()
        .write_one(&track(2, 100), HANDLE_NIL)
        .unwrap();
    // RELIABLE: acknowledged is received.
    supplier
        .dds_entity()
        .wait_for_acknowledgments(std::time::Duration::from_secs(10))
        .expect("the port holds the event");
    let before = heard.threads.lock().unwrap().len();
    port.data_control().set_mode(ListenerMode::OneByOne);
    let threads = heard.threads.lock().unwrap();
    assert_eq!(threads.len(), before + 1, "delivered before the return");
    assert_eq!(threads[before].0, here);
    drop(threads);

    drop(supplier);
    supplying.remove().unwrap();
    consuming.remove().unwrap();
}

/// "port (for all its facets)": `status` (`PortStatusListener`) is called
/// on the pool as `data_listener` is. `error_listener` of the connector is
/// no facet of the port and is called on the thread of DDS.
#[test]
fn dds4ccm_7_4_4_the_status_facet_is_called_on_the_pool_of_the_port() {
    let d = Deployment::new();
    let deadline = "<deadline><period><sec>0</sec><nanosec>200000000</nanosec></period></deadline>";
    let qos_profile = profile(&format!(
        "<datawriter_qos>{deadline}</datawriter_qos><datareader_qos>{deadline}</datareader_qos>"
    ));
    let supplying = d.event("Dds4CcmThreadsStatus", &qos_profile);
    let consuming = d.event("Dds4CcmThreadsStatus", &qos_profile);
    let errors = Arc::new(Statuses::default());
    consuming.connect_error_listener(errors.clone() as Arc<dyn ConnectorStatusListener>);
    let component = d.subscribing.component("sink");
    component.set_threading_policy(Some(ThreadingPolicy::SINGLE));

    let port = DdsListen::<Track>::new();
    let statuses = Arc::new(Statuses::default());
    port.connect_status(statuses.clone() as Arc<dyn PortStatusListener>);
    consuming.push_consumer(&component, &port).unwrap();

    // One event, then silence: the deadline of the reader is missed.
    let supplier = supplier(&d, &supplying, 1);
    supplier.data().write_one(&track(1, 1), HANDLE_NIL).unwrap();
    eventually("on_requested_deadline_missed of the port", || {
        statuses.has("requested_deadline_missed")
    });
    let called_on = statuses.threads.lock().unwrap().clone();
    assert!(
        called_on
            .iter()
            .all(|name| name.as_deref() == Some("subscribing.sink-0")),
        "{called_on:?}"
    );
    // The match went to error_listener (`on_unexpected_status`), not on
    // the pool.
    eventually("the match reaches error_listener", || {
        !errors.events().is_empty()
    });
    let reported_on = errors.threads.lock().unwrap().clone();
    assert!(
        reported_on
            .iter()
            .all(|name| name.as_deref() != Some("subscribing.sink-0")),
        "{reported_on:?}"
    );

    drop(supplier);
    supplying.remove().unwrap();
    consuming.remove().unwrap();
}

/// "When a facet is activated": a policy holds for the ports activated
/// after it is set. A port keeps the pool it was activated with.
#[test]
fn dds4ccm_7_4_4_a_policy_holds_for_the_facets_activated_after_it() {
    let d = Deployment::new();
    let (supplying, consuming) = two_sides(&d, "Dds4CcmThreadsLater");
    let component = d.subscribing.component("sink");

    let (before, before_heard) = listening(&consuming, &component, None);
    component.set_threading_policy(Some(ThreadingPolicy::SINGLE));
    let (after, after_heard) = listening(&consuming, &component, None);
    component.set_threading_policy(ThreadingPolicy::threads(2));
    let (resized, resized_heard) = listening(&consuming, &component, None);
    component.set_threading_policy(None);
    let (without, without_heard) = listening(&consuming, &component, None);

    assert!(before.thread_pool().is_none());
    assert!(without.thread_pool().is_none());
    let (single, double) = (after.thread_pool().unwrap(), resized.thread_pool().unwrap());
    assert_eq!(single.policy(), ThreadingPolicy::SINGLE);
    assert_eq!(double.policy().threads.get(), 2);
    assert!(!Arc::ptr_eq(&single, &double));

    let supplier = supplier(&d, &supplying, 4);
    send(
        &supplier,
        5,
        &[&before_heard, &after_heard, &resized_heard, &without_heard],
    );
    let on_pool = |heard: &Heard| {
        let called_on = heard.thread_names();
        called_on.iter().all(|name| {
            name.as_deref()
                .is_some_and(|n| n.starts_with("subscribing.sink-"))
        })
    };
    assert!(on_pool(&after_heard) && on_pool(&resized_heard));
    assert!(!on_pool(&before_heard) && !on_pool(&without_heard));

    drop(supplier);
    supplying.remove().unwrap();
    consuming.remove().unwrap();
}
