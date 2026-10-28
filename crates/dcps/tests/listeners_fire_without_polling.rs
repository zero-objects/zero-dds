// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS 1.4 §2.2.4.1: a listener is called when its status changes. These
//! tests set listeners and then never ask for a status — no getter, no
//! `drive_listeners()`, no `wait_for_matched_*`, all of which run the
//! same delta polls on the caller's thread and would hide the question.
//! What the listener hears, it heard from the runtime's dispatcher.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use common::{isolated_cfg, match_timeout, unique_domain};
use zerodds_dcps::listener::{DataReaderListener, DataWriterListener};
use zerodds_dcps::psm_constants::status as bits;
use zerodds_dcps::status::{PublicationMatchedStatus, SubscriptionMatchedStatus};
use zerodds_dcps::*;

#[derive(Default)]
struct Matched {
    calls: AtomicU32,
    current: AtomicI32,
}

impl DataReaderListener for Matched {
    fn on_subscription_matched(&self, _r: InstanceHandle, s: SubscriptionMatchedStatus) {
        self.current.store(s.current_count, Ordering::SeqCst);
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

impl DataWriterListener for Matched {
    fn on_publication_matched(&self, _w: InstanceHandle, s: PublicationMatchedStatus) {
        self.current.store(s.current_count, Ordering::SeqCst);
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

/// Sleeps until `what` holds or `timeout` passes, asking nothing of DDS.
fn until(timeout: Duration, what: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    what()
}

#[test]
fn matched_listeners_fire_on_match_and_unmatch_without_a_poll() {
    let cfg = isolated_cfg();
    let factory = DomainParticipantFactory::instance();
    let domain = unique_domain();
    let pub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
        .expect("pub participant");
    let sub_p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("sub participant");
    let pub_topic = pub_p
        .create_topic::<RawBytes>("ListenersWithoutPolling", TopicQos::default())
        .expect("pub topic");
    let sub_topic = sub_p
        .create_topic::<RawBytes>("ListenersWithoutPolling", TopicQos::default())
        .expect("sub topic");
    let publisher = pub_p.create_publisher(PublisherQos::default());
    let subscriber = sub_p.create_subscriber(SubscriberQos::default());

    let writer_heard = Arc::new(Matched::default());
    let reader_heard = Arc::new(Matched::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&pub_topic, DataWriterQos::default())
        .expect("writer");
    writer.set_listener(Some(writer_heard.clone()), bits::ANY);
    let reader = subscriber
        .create_datareader::<RawBytes>(&sub_topic, DataReaderQos::default())
        .expect("reader");
    reader.set_listener(Some(reader_heard.clone()), bits::ANY);

    assert!(
        until(match_timeout(), || {
            writer_heard.current.load(Ordering::SeqCst) == 1
                && reader_heard.current.load(Ordering::SeqCst) == 1
        }),
        "no on_*_matched with a count of 1 without polling: writer {} calls, reader {} calls",
        writer_heard.calls.load(Ordering::SeqCst),
        reader_heard.calls.load(Ordering::SeqCst)
    );

    // The unmatch is a change too: drop the reader, and the writer's
    // listener hears its count go back to 0.
    drop(reader);
    assert!(
        until(match_timeout(), || writer_heard
            .current
            .load(Ordering::SeqCst)
            == 0),
        "no on_publication_matched for the unmatch"
    );
    for p in [&pub_p, &sub_p] {
        let _ = p.delete_contained_entities();
        let _ = factory.delete_participant(p);
    }
}
