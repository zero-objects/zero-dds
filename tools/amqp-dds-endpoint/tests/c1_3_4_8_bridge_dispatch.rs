#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.3 / C.1.4 / C.1.8 — Bridge-Dispatch via DdsHost.
//!
//! Spec §C.1.3 Sender-Link Producer-to-DDS:
//! AMQP client publishes → DDS DataWriter sees the sample.
//! Test simulates this via the bridge helper (the AMQP body lands
//! in the DdsHost) without running the full daemon wire roundtrip.
//!
//! Spec §C.1.4 Receiver-Link DDS-to-Consumer:
//! analogous in reverse; DDS publishes → the AMQP receiver gets
//! a transfer.
//!
//! Spec §C.1.8 Catalog-Address:
//! AMQP client attaches a receiver to `$catalog`, sees a
//! one catalog entry per topic mapping.

mod common;

use amqp_dds_endpoint::bridge::{
    AttachOutcome, dispatch_attach, dispatch_transfer, encode_catalog_sample,
    produce_catalog_transfers, subscribe_outbound,
};
use amqp_dds_endpoint::dds_host::{DdsHost, InMemoryDdsHost};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use zerodds_amqp_endpoint::annex_a::{LinkDirection, TopicMapping};
use zerodds_amqp_endpoint::management::addresses;
use zerodds_amqp_endpoint::metrics::MetricsHub;

fn host_with_topics(specs: &[(&str, &str, LinkDirection)]) -> InMemoryDdsHost {
    let h = InMemoryDdsHost::new();
    for (addr, dds_topic, dir) in specs {
        h.register_topic(TopicMapping {
            amqp_address: (*addr).into(),
            dds_topic: (*dds_topic).into(),
            dds_type_name: "T".into(),
            direction: *dir,
            ..TopicMapping::default()
        })
        .unwrap();
    }
    h
}

#[test]
fn c1_3_amqp_producer_publishes_to_dds_via_bridge() {
    // Spec §C.1.3 — client sends an AMQP sample, the bridge forwards
    // to a DDS DataWriter.
    let host = host_with_topics(&[("Sensor", "SensorTopic", LinkDirection::DirProducerToDds)]);

    // 1. Client attaches a sender link to address "Sensor".
    let outcome = dispatch_attach(&host, "Sensor");
    let topic_id = match outcome {
        AttachOutcome::AttachedTopic { topic_id } => topic_id,
        other => panic!("expected AttachedTopic, got {other:?}"),
    };

    // DDS side subscribes (a real DataWriter would come from the DDS
    // crate; here a mock that holds onto the sample).
    let received = Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
    let received_cb = received.clone();
    host.subscribe_amqp_to_dds(
        topic_id,
        Arc::new(move |bytes: &[u8]| {
            received_cb.lock().unwrap().push(bytes.to_vec());
        }),
    )
    .unwrap();

    // 2. Client sends three transfer frames with XCDR2 body.
    let metrics = Arc::new(MetricsHub::new());
    for sample in [b"sample-1".as_slice(), b"sample-2", b"sample-3"] {
        dispatch_transfer(&host, topic_id, sample, &metrics).unwrap();
    }

    // 3. The DDS side has seen all three samples.
    let captured = received.lock().unwrap().clone();
    assert_eq!(captured.len(), 3);
    assert_eq!(captured[0], b"sample-1");
    assert_eq!(captured[1], b"sample-2");
    assert_eq!(captured[2], b"sample-3");

    // 4. Metric `transfers.received` shows three.
    assert_eq!(metrics.snapshot("transfers.received"), Some(3));
}

#[test]
fn c1_4_dds_publish_flows_to_amqp_consumer_callback() {
    // Spec §C.1.4 — a DDS DataReader gets a sample (=DDS-side
    // publish), the AMQP receiver link gets a transfer.
    let host = host_with_topics(&[("Pose", "PoseTopic", LinkDirection::DirDdsToConsumer)]);

    let outcome = dispatch_attach(&host, "Pose");
    let topic_id = match outcome {
        AttachOutcome::AttachedTopic { topic_id } => topic_id,
        other => panic!("expected AttachedTopic, got {other:?}"),
    };

    // AMQP receiver side subscribes; the callback simulates
    // the outbound AMQP transfer producer.
    let outbound_count = Arc::new(AtomicUsize::new(0));
    let outbound_cb = outbound_count.clone();
    let outbound_bytes = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let outbound_bytes_cb = outbound_bytes.clone();
    let _sid = subscribe_outbound(
        &host,
        topic_id,
        Arc::new(move |bytes: &[u8]| {
            outbound_cb.fetch_add(1, Ordering::Relaxed);
            *outbound_bytes_cb.lock().unwrap() = bytes.to_vec();
        }),
    )
    .unwrap();

    // DDS side publishes a sample (e.g. via discovery
    // or a real DataWriter; here directly host.publish).
    host.publish_to_dds(topic_id, b"pose-update").unwrap();

    // AMQP receiver side received exactly one outbound sample.
    assert_eq!(outbound_count.load(Ordering::Relaxed), 1);
    assert_eq!(*outbound_bytes.lock().unwrap(), b"pose-update");
}

#[test]
fn c1_8_catalog_receiver_gets_one_sample_per_topic() {
    // Spec §C.1.8 — receiver on `$catalog` gets one per
    // configured topic mapping one sample.
    let host = host_with_topics(&[
        ("Sensor", "SensorTopic", LinkDirection::DirProducerToDds),
        ("Pose", "PoseTopic", LinkDirection::DirDdsToConsumer),
        ("Cmd", "CmdTopic", LinkDirection::DirBoth),
    ]);

    // Client attaches a receiver link on `$catalog`.
    let outcome = dispatch_attach(&host, addresses::CATALOG);
    assert_eq!(outcome, AttachOutcome::AttachedCatalog);

    // The bridge produces the catalog sample stream.
    let samples = produce_catalog_transfers(&host);
    assert_eq!(samples.len(), 3);

    // Each sample can be encoded as an AMQP wire body.
    for s in samples {
        let body = encode_catalog_sample(s).unwrap();
        assert!(
            body.len() > 8,
            "encoded body too short: {} bytes",
            body.len()
        );
        // 0x00 = described-prefix.
        assert_eq!(body[0], 0x00);
    }
}

#[test]
fn c1_3_unknown_address_attaches_with_unknown_address_outcome() {
    // Spec §C.1.3 + §7.5.1 — attach to an unconfigured
    // address returns UnknownAddress (the caller maps it to
    // amqp:not-found).
    let host = host_with_topics(&[]);
    let outcome = dispatch_attach(&host, "NotInCatalog");
    assert_eq!(outcome, AttachOutcome::UnknownAddress);
}

#[test]
fn c2_2_bridge_outbound_publish_flows_to_dds() {
    // Spec §C.2.2 — bridge sends a sender link after outbound
    // Broker side (AMQP). The broker forwards, a second
    // The bridge endpoint connects as the receiver side and calls
    // dispatch_transfer with the bytes — they land in DDS.
    //
    // We simulate this end-to-end: two bridges that share the
    // topic; samples flow from Bridge-A's DDS via AMQP
    // to Bridge-B's DDS.
    let bridge_a = host_with_topics(&[("Cmd", "CmdTopic", LinkDirection::DirBoth)]);
    let bridge_b = host_with_topics(&[("Cmd", "CmdTopic", LinkDirection::DirBoth)]);

    // Bridge A subscribes to the DDS side (outbound path).
    let topic_a = bridge_a.lookup("Cmd").unwrap();
    let topic_b = bridge_b.lookup("Cmd").unwrap();

    // Mock wire roundtrip: when bridge A's DDS side publishes,
    // the AMQP receiver outbound callback is invoked, which
    // (via simulated AMQP wire) arrives as an inbound transfer at
    // Bridge-B.
    let bridge_b_metrics = Arc::new(MetricsHub::new());
    let bridge_b_metrics_cb = bridge_b_metrics.clone();
    let bridge_b_for_cb = std::sync::Arc::new(bridge_b);
    let bridge_b_for_cb_clone = bridge_b_for_cb.clone();
    bridge_a
        .subscribe_amqp_to_dds(
            topic_a,
            Arc::new(move |bytes: &[u8]| {
                // Simulated AMQP wire roundtrip: Bridge-A -> Broker -> Bridge-B
                // -> dispatch_transfer on Bridge-B's DdsHost.
                dispatch_transfer(
                    &*bridge_b_for_cb_clone,
                    topic_b,
                    bytes,
                    &bridge_b_metrics_cb,
                )
                .unwrap();
            }),
        )
        .unwrap();

    // Bridge-A's DDS side publishes; Bridge-B's DDS side
    // receives it.
    let received_at_b = Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
    let received_at_b_cb = received_at_b.clone();
    bridge_b_for_cb
        .subscribe_amqp_to_dds(
            topic_b,
            Arc::new(move |bytes| {
                received_at_b_cb.lock().unwrap().push(bytes.to_vec());
            }),
        )
        .unwrap();

    bridge_a.publish_to_dds(topic_a, b"motor-on").unwrap();

    let received = received_at_b.lock().unwrap().clone();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0], b"motor-on");
    assert_eq!(bridge_b_metrics.snapshot("transfers.received"), Some(1));
}

#[test]
fn c2_3_bridge_inbound_from_broker_flows_to_dds() {
    // Spec §C.2.3 — Receiver-Link Broker-to-Bridge: Broker
    // sends an AMQP transfer, bridge calls dispatch_transfer →
    // DDS-DataWriter.
    let bridge = host_with_topics(&[(
        "Telemetry",
        "TelemetryTopic",
        LinkDirection::DirProducerToDds,
    )]);
    let topic = bridge.lookup("Telemetry").unwrap();
    let captured = Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
    let captured_cb = captured.clone();
    bridge
        .subscribe_amqp_to_dds(
            topic,
            Arc::new(move |bytes| {
                captured_cb.lock().unwrap().push(bytes.to_vec());
            }),
        )
        .unwrap();

    // Broker-zu-Bridge-Transfer simulieren.
    let metrics = Arc::new(MetricsHub::new());
    dispatch_transfer(&bridge, topic, b"telemetry-1", &metrics).unwrap();
    dispatch_transfer(&bridge, topic, b"telemetry-2", &metrics).unwrap();

    assert_eq!(captured.lock().unwrap().len(), 2);
    assert_eq!(metrics.snapshot("transfers.received"), Some(2));
}
