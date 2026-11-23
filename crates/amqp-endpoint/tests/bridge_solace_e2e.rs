// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon (`zerodds-amqp-bridge-daemon-1.0` §10) against Solace
//! PubSub+ Standard 10.26: the broker matrix of `support/broker_matrix.rs`
//! with qpid-proton as the client at the broker, and the address form
//! `topic://<name>` of §5.1.
//!
//! What differs from Artemis, as measured: Solace attaches a sender to any
//! queue name and rejects each message for a queue that does not exist, so
//! the test creates its queues through SEMP before the links attach (the
//! names carry the process id; the broker outlives the test). It refuses
//! the attach of a receiver from a queue its user may not consume
//! (`amqp:unauthorized-access`), which is the refused link of the matrix.
//! A dropped connection is SEMP's `disconnect` of every AMQP client. The
//! bridge connects over TLS with SASL PLAIN (`support/solace.rs`).
//!
//! The broker is a container (`support/solace.rs` names the variables); the
//! tests run in the integration pillar's job
//! `integration-bridges-vendor-amqp-solace`, where it is a service
//! container, and a missing broker fails them. They run one at a time (the
//! profile `integration`), since they share the broker:
//!
//! ```bash
//! ZERODDS_SOLACE=127.0.0.1 cargo nextest run --profile integration \
//!   -p zerodds-amqp-endpoint --test bridge_solace_e2e --run-ignored all
//! ```

#![cfg(all(feature = "std", unix))]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

#[path = "support/broker_matrix.rs"]
mod broker_matrix;
#[path = "support/solace.rs"]
mod solace;

use solace::SolaceBroker;

struct MatrixSolace(SolaceBroker);

impl MatrixSolace {
    /// The queue name of the node `name` for this process.
    fn queue_name(name: &str) -> String {
        format!("{name}_{}", std::process::id())
    }
}

impl broker_matrix::MatrixBroker for MatrixSolace {
    fn amqp_yaml(&self) -> String {
        format!(
            "  broker_url: \"amqps://{}:{}\"\n  sasl:\n    mechanism: PLAIN\n    username: {}\n    password: {}\n  tls:\n    enabled: true\n    ca_file: \"{}\"\n",
            self.0.host,
            self.0.amqps_port,
            solace::USER,
            solace::PASSWORD,
            self.0.ca_file().display()
        )
    }

    fn peer_url(&self) -> String {
        format!(
            "amqp://{}:{}@{}:{}",
            solace::USER,
            solace::PASSWORD,
            self.0.host,
            self.0.amqp_port
        )
    }

    /// The queue, created now; `zd_out` in the plain form, the other
    /// queues as `queue://<name>`.
    fn address(&self, name: &str) -> String {
        let queue = Self::queue_name(name);
        self.0.queue(&queue);
        if name == "zd_out" {
            queue
        } else {
            format!("queue://{queue}")
        }
    }

    fn stores(&self) -> bool {
        true
    }

    fn drop_connections(&mut self) {
        self.0.disconnect_amqp_clients();
    }

    fn refused_address(&self) -> String {
        let queue = Self::queue_name("zd_refused");
        self.0.queue_without_access(&queue);
        format!("queue://{queue}")
    }

    fn refused_direction(&self) -> &'static str {
        "in"
    }
}

fn broker() -> MatrixSolace {
    MatrixSolace(SolaceBroker::connect())
}

/// §10, §4.3 against Solace: DDS samples arrive at qpid-proton as
/// pass-through messages.
#[test]
#[ignore = "needs a Solace PubSub+ container and qpid-proton; runs in integration-bridges-vendor-amqp-solace"]
fn amqpbridge_10_solace_dds_samples_arrive_as_pass_through_messages() {
    broker_matrix::samples_arrive_as_pass_through_messages(&mut broker());
}

/// §10, §4.4 against Solace: a message of qpid-proton reaches a DDS reader;
/// a bad body is rejected by the bridge and reaches none.
#[test]
#[ignore = "needs a Solace PubSub+ container and qpid-proton; runs in integration-bridges-vendor-amqp-solace"]
fn amqpbridge_10_solace_a_message_reaches_a_reader_and_a_bad_body_is_rejected() {
    broker_matrix::a_message_reaches_a_reader_and_a_bad_body_is_rejected(&mut broker());
}

/// §10, Part 2 §2.6.14 against Solace: a 20 000-byte sample at a frame size
/// of 4096 in both directions.
#[test]
#[ignore = "needs a Solace PubSub+ container and qpid-proton; runs in integration-bridges-vendor-amqp-solace"]
fn amqpbridge_10_solace_a_multi_frame_sample_crosses_in_both_directions() {
    broker_matrix::a_multi_frame_sample_crosses_in_both_directions(&mut broker());
}

/// §10, §9.3 against Solace: after the broker ends the connection (SEMP
/// `disconnect`) the bridge connects again and delivers the sample written
/// meanwhile.
#[test]
#[ignore = "needs a Solace PubSub+ container and qpid-proton; runs in integration-bridges-vendor-amqp-solace"]
fn amqpbridge_10_solace_the_bridge_reconnects_after_the_broker_drops_the_connection() {
    broker_matrix::the_bridge_reconnects_after_the_broker_drops_the_connection(&mut broker());
}

/// §10, §4.2 against Solace: the attach of a receiver from a queue the
/// bridge's user may not consume is refused; the sender link beside it
/// keeps running.
#[test]
#[ignore = "needs a Solace PubSub+ container and qpid-proton; runs in integration-bridges-vendor-amqp-solace"]
fn amqpbridge_10_solace_a_refused_address_leaves_the_other_links_running() {
    broker_matrix::a_refused_address_leaves_the_other_links_running(&mut broker());
}

/// §5.1 against Solace: `topic://<name>`, a topic without a queue. Two
/// receivers of qpid-proton subscribe first; each gets every sample the
/// bridge sends there, and each round trip ends `accepted` at the bridge.
#[test]
#[ignore = "needs a Solace PubSub+ container and qpid-proton; runs in integration-bridges-vendor-amqp-solace"]
fn amqpbridge_5_1_solace_a_topic_address_reaches_every_subscriber() {
    use broker_matrix::{
        Bridge, MatrixBroker, ProtonPeer, STEP, bridge_yaml, config, matched_writer, participant,
        topic,
    };
    let solace = broker();
    let address = format!("topic://{}", MatrixSolace::queue_name("zd_multi"));
    let mut first = ProtonPeer::connect(&solace.peer_url());
    first.receiver(&address, 10);
    let mut second = ProtonPeer::connect(&solace.peer_url());
    second.receiver(&address, 10);
    let bridge = Bridge::start(config(&bridge_yaml(
        &solace,
        "multi",
        "",
        &topic("MatrixMulti", &address, "out"),
    )));
    let dds = participant();
    let w = matched_writer(&dds, "MatrixMulti");
    for n in 1u8..=2 {
        dds.write_user_sample(w, vec![n, n, n, n]).unwrap();
    }
    let got_first = first.take(&address, 2, STEP);
    let got_second = second.take(&address, 2, STEP);
    bridge.wait_metric("zerodds_amqp_bridge_dispositions_accepted_total", 2);
    bridge.stop();
    dds.shutdown();
    for got in [&got_first, &got_second] {
        assert_eq!(got.len(), 2, "{got:?}");
        for (i, m) in got.iter().enumerate() {
            let n = (i + 1) as u8;
            assert_eq!(&m.body[4..], &[n, n, n, n], "{m:?}");
            assert!(m.properties.contains("dds:bridge-id="), "{m:?}");
        }
    }
}
