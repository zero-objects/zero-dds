// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon (`zerodds-amqp-bridge-daemon-1.0` §10) against Apache
//! Qpid Dispatch Router 1.19: the broker matrix of `support/broker_matrix.rs`
//! with qpid-proton as the client at the router. The router stores
//! nothing: a sender link gets credit only while a receiver of its address
//! is attached, and the outcome of a delivery is the receiver's, passed
//! back by the router. The addresses have the form `<prefix>/<name>` of
//! §5.1 (prefix `zd`, balanced).
//!
//! The tests need `qdrouterd` and qpid-proton's Python binding; they run in
//! the integration pillar's job `integration-bridges-vendor-amqp`, where
//! `tests/integration/provision.sh dispatch proton` installs them and a
//! missing program fails them:
//!
//! ```bash
//! cargo nextest run -p zerodds-amqp-endpoint --test bridge_dispatch_e2e --run-ignored all
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
#[path = "support/dispatch.rs"]
mod dispatch;

use dispatch::DispatchRouter;

struct MatrixDispatch(DispatchRouter);

impl broker_matrix::MatrixBroker for MatrixDispatch {
    fn amqp_yaml(&self) -> String {
        format!("  broker_url: \"amqp://{}\"\n", self.0.addr())
    }

    fn peer_url(&self) -> String {
        format!("amqp://{}", self.0.addr())
    }

    fn address(&self, name: &str) -> String {
        format!("zd/{name}")
    }

    fn stores(&self) -> bool {
        false
    }

    fn drop_connections(&mut self) {
        self.0.restart();
    }

    fn refused_address(&self) -> String {
        "zd_refused/x".into()
    }
}

fn router() -> MatrixDispatch {
    MatrixDispatch(DispatchRouter::start())
}

/// §10, §4.3 through Qpid Dispatch: DDS samples arrive at qpid-proton as
/// pass-through messages.
#[test]
#[ignore = "needs Qpid Dispatch Router and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_10_dispatch_dds_samples_arrive_as_pass_through_messages() {
    broker_matrix::samples_arrive_as_pass_through_messages(&mut router());
}

/// §10, §4.4 through Qpid Dispatch: a message of qpid-proton reaches a DDS
/// reader and is accepted; a bad body is rejected with
/// `amqp:decode-error`, an outcome the router passes back to the sender.
#[test]
#[ignore = "needs Qpid Dispatch Router and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_10_dispatch_a_message_reaches_a_reader_and_a_bad_body_is_rejected() {
    broker_matrix::a_message_reaches_a_reader_and_a_bad_body_is_rejected(&mut router());
}

/// §10, Part 2 §2.6.14 through Qpid Dispatch: a 20 000-byte sample at a
/// frame size of 4096 in both directions.
#[test]
#[ignore = "needs Qpid Dispatch Router and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_10_dispatch_a_multi_frame_sample_crosses_in_both_directions() {
    broker_matrix::a_multi_frame_sample_crosses_in_both_directions(&mut router());
}

/// §10, §9.3 through Qpid Dispatch: after a restart of the router the
/// bridge connects again and delivers the sample written meanwhile, once a
/// receiver gives its link credit.
#[test]
#[ignore = "needs Qpid Dispatch Router and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_10_dispatch_the_bridge_reconnects_after_the_broker_drops_the_connection() {
    broker_matrix::the_bridge_reconnects_after_the_broker_drops_the_connection(&mut router());
}

/// §10, §4.2 through Qpid Dispatch: a link to a link-routed prefix without
/// its connection is refused; the other link keeps running.
#[test]
#[ignore = "needs Qpid Dispatch Router and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_10_dispatch_a_refused_address_leaves_the_other_links_running() {
    broker_matrix::a_refused_address_leaves_the_other_links_running(&mut router());
}
