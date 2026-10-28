// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The 'nil' listener with a mask (DDS 1.4 §2.2.2.1.1.3, §2.2.4.2.1,
//! §2.2.4.3.1): the statuses of the mask are handled by doing nothing: they
//! do not bubble up, and "the act of calling the 'nil' listener does not
//! reset the communication status".

#![cfg(all(feature = "std", target_os = "linux"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use common::{ParticipantGuard, isolated_cfg, unique_domain};
use zerodds_dcps::listener::DomainParticipantListener;
use zerodds_dcps::psm_constants::status as bits;
use zerodds_dcps::status::SubscriptionMatchedStatus;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, InstanceHandle,
    PublisherQos, RawBytes, SubscriberQos, TopicQos,
};

#[derive(Default)]
struct Told(AtomicU32);

impl DomainParticipantListener for Told {
    fn on_subscription_matched(&self, _reader: InstanceHandle, _status: SubscriptionMatchedStatus) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn a_nil_listener_does_not_pass_the_status_up_and_does_not_reset_it() {
    let p = ParticipantGuard::new(
        DomainParticipantFactory::instance()
            .create_participant_with_config(
                unique_domain(),
                DomainParticipantQos::default(),
                isolated_cfg(),
            )
            .expect("participant"),
    );
    let told = Arc::new(Told::default());
    p.set_listener(Some(told.clone()), bits::SUBSCRIPTION_MATCHED);

    let topic = p
        .create_topic::<RawBytes>("NilListenerMatched", TopicQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    reader.set_listener(None, bits::SUBSCRIPTION_MATCHED);
    let _writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    while reader.matched_publications().map_or(true, |m| m.is_empty()) {
        assert!(Instant::now() < deadline, "no match");
        std::thread::sleep(Duration::from_millis(10));
    }
    // Give the listener path time to act.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(told.0.load(Ordering::SeqCst), 0, "no bubble-up");
    let status = reader.get_subscription_matched_status().unwrap();
    assert_eq!(
        (status.total_count, status.total_count_change),
        (1, 1),
        "the nil listener did not take the change off"
    );
}

/// DDS 1.4 §2.2.2.2.2.1 `create_participant(domain_id, qos, a_listener,
/// mask)`.
#[test]
fn create_participant_takes_a_listener_and_a_mask() {
    let lease = common::lease_domain();
    let told = Arc::new(Told::default());
    let p = ParticipantGuard::new(
        DomainParticipantFactory::instance()
            .create_participant_with_listener(
                lease.id() as u32 as _,
                DomainParticipantQos::default(),
                Some(told),
                bits::SUBSCRIPTION_MATCHED,
            )
            .expect("participant"),
    );
    assert!(p.get_listener().is_some());
}
