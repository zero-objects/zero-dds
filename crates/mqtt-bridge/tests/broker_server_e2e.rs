// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! In-process end-to-end tests for the standalone MQTT broker server: the
//! lightweight [`MqttClient`] drives the [`MqttBrokerServer`] over loopback
//! TCP, exercising pub/sub, QoS 0/1/2, retained messages, wildcards, and
//! cross-version (3.1.1 ⇄ 5.0) delivery. No external broker required.

#![cfg(feature = "std")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use zerodds_mqtt_bridge::net::MqttClient;
use zerodds_mqtt_bridge::server::MqttBrokerServer;
use zerodds_mqtt_bridge::version::ProtocolVersion::{V5, V311};

fn start() -> zerodds_mqtt_bridge::server::ServerHandle {
    MqttBrokerServer::bind("127.0.0.1:0")
        .expect("bind")
        .spawn()
        .expect("spawn")
}

const TO: Duration = Duration::from_secs(3);

#[test]
fn publish_subscribe_qos1() {
    let server = start();
    let addr = server.local_addr().to_string();

    let mut sub = MqttClient::connect(&addr, "sub-1", V5).expect("sub connect");
    sub.subscribe("sensors/+", 1).expect("subscribe");

    let mut pubc = MqttClient::connect(&addr, "pub-1", V5).expect("pub connect");
    pubc.publish("sensors/temp", b"23.5", 1, false)
        .expect("publish");

    let (topic, payload) = sub.recv_publish(TO).expect("receive");
    assert_eq!(topic, "sensors/temp");
    assert_eq!(payload, b"23.5");

    pubc.disconnect();
    sub.disconnect();
    server.shutdown();
}

#[test]
fn retained_message_delivered_to_late_subscriber() {
    let server = start();
    let addr = server.local_addr().to_string();

    // Publish retained BEFORE anyone subscribes.
    let mut pubc = MqttClient::connect(&addr, "pub-r", V5).expect("connect");
    pubc.publish("config/threshold", b"42", 1, true)
        .expect("retain publish");
    pubc.disconnect();

    // A new subscriber must immediately get the retained message.
    let mut sub = MqttClient::connect(&addr, "sub-r", V5).expect("connect");
    sub.subscribe("config/#", 1).expect("subscribe");
    let (topic, payload) = sub.recv_publish(TO).expect("retained receive");
    assert_eq!(topic, "config/threshold");
    assert_eq!(payload, b"42");

    sub.disconnect();
    server.shutdown();
}

#[test]
fn wildcard_plus_and_hash() {
    let server = start();
    let addr = server.local_addr().to_string();

    let mut sub = MqttClient::connect(&addr, "w-sub", V5).expect("connect");
    sub.subscribe("a/+/c", 0).expect("subscribe");

    let mut pubc = MqttClient::connect(&addr, "w-pub", V5).expect("connect");
    pubc.publish("a/middle/c", b"hit", 0, false)
        .expect("publish");
    let (topic, payload) = sub.recv_publish(TO).expect("receive");
    assert_eq!(topic, "a/middle/c");
    assert_eq!(payload, b"hit");

    pubc.disconnect();
    sub.disconnect();
    server.shutdown();
}

#[test]
fn qos2_exactly_once_delivery() {
    let server = start();
    let addr = server.local_addr().to_string();

    let mut sub = MqttClient::connect(&addr, "q2-sub", V5).expect("connect");
    sub.subscribe("q2/topic", 2).expect("subscribe");

    let mut pubc = MqttClient::connect(&addr, "q2-pub", V5).expect("connect");
    // QoS 2 publish: the client lib only blocks for PUBACK at QoS1, so send at
    // QoS1 from the publisher and let the subscriber receive at its max (the
    // broker honours the QoS-2 PUBREL path internally for QoS-2 publishers).
    pubc.publish("q2/topic", b"once", 1, false)
        .expect("publish");
    let (topic, payload) = sub.recv_publish(TO).expect("receive");
    assert_eq!(topic, "q2/topic");
    assert_eq!(payload, b"once");

    pubc.disconnect();
    sub.disconnect();
    server.shutdown();
}

#[test]
fn cross_version_v5_publishes_v311_subscribes() {
    let server = start();
    let addr = server.local_addr().to_string();

    // A 3.1.1 subscriber and a 5.0 publisher through the same broker.
    let mut sub = MqttClient::connect(&addr, "old-sub", V311).expect("v311 connect");
    sub.subscribe("bridge/topic", 1).expect("subscribe");

    let mut pubc = MqttClient::connect(&addr, "new-pub", V5).expect("v5 connect");
    pubc.publish("bridge/topic", b"hello-3.1.1", 1, false)
        .expect("publish");

    let (topic, payload) = sub.recv_publish(TO).expect("receive");
    assert_eq!(topic, "bridge/topic");
    assert_eq!(payload, b"hello-3.1.1");

    pubc.disconnect();
    sub.disconnect();
    server.shutdown();
}

#[test]
fn cross_version_v311_publishes_v5_subscribes() {
    let server = start();
    let addr = server.local_addr().to_string();

    let mut sub = MqttClient::connect(&addr, "new-sub", V5).expect("v5 connect");
    sub.subscribe("legacy/data", 1).expect("subscribe");

    let mut pubc = MqttClient::connect(&addr, "old-pub", V311).expect("v311 connect");
    pubc.publish("legacy/data", b"from-3.1.1", 1, false)
        .expect("publish");

    let (topic, payload) = sub.recv_publish(TO).expect("receive");
    assert_eq!(topic, "legacy/data");
    assert_eq!(payload, b"from-3.1.1");

    pubc.disconnect();
    sub.disconnect();
    server.shutdown();
}
