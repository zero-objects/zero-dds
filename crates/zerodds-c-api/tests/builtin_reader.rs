// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DataReaders of the built-in subscriber (DDS 1.4 §2.2.5): `lookup_datareader` by topic
//! name, and `read`/`take` of the samples discovery put there, for the four built-in topics.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::CString;
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::builtin_reader_ffi::{
    ZERODDS_BUILTIN_PARTICIPANT, ZERODDS_BUILTIN_PUBLICATION, ZERODDS_BUILTIN_SUBSCRIPTION,
    ZERODDS_BUILTIN_TOPIC, ZeroDdsBuiltinReader, ZeroDdsBuiltinSample,
    zerodds_builtin_reader_destroy, zerodds_builtin_reader_kind, zerodds_builtin_reader_read,
    zerodds_builtin_samples_free, zerodds_builtin_subscriber_lookup_datareader,
};
use zerodds::extra_ffi::{zerodds_builtin_subscriber_destroy, zerodds_dp_get_builtin_subscriber};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
};
use zerodds::publisher_ffi::zerodds_pub_create_datawriter;
use zerodds::subscriber_ffi::zerodds_sub_create_datareader;

const OK: i32 = 0;

/// Reads `reader` until `want` accepts the samples or five seconds are over.
///
/// # Safety
/// `reader` is a live built-in reader.
unsafe fn read_until(
    reader: *mut ZeroDdsBuiltinReader,
    take: bool,
    want: impl Fn(&[ZeroDdsBuiltinSample]) -> bool,
) -> bool {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let mut samples: *mut ZeroDdsBuiltinSample = ptr::null_mut();
        let mut count = 0usize;
        // SAFETY: `reader` is live; the out pointers are locals.
        let rc = unsafe {
            zerodds_builtin_reader_read(reader, i32::from(take), &mut samples, &mut count)
        };
        assert_eq!(rc, OK);
        let found = if samples.is_null() {
            want(&[])
        } else {
            // SAFETY: the array the call returned, `count` long.
            want(unsafe { std::slice::from_raw_parts(samples, count) })
        };
        // SAFETY: the array and count that call returned.
        unsafe { zerodds_builtin_samples_free(samples, count) };
        if found {
            return true;
        }
        if Instant::now() > until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CBuiltinReader").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    // SAFETY: every pointer comes from the create call above its use and is checked; the
    // participants are deleted once at the end.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let reader_side = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        let writer_side = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());

        // The built-in readers hold what discovery found of the other participant: the writer's
        // side sees the reader as a subscription, the reader's side sees the writer.
        let bs_writer_side = zerodds_dp_get_builtin_subscriber(writer_side);
        let bs = zerodds_dp_get_builtin_subscriber(reader_side);
        assert!(!bs.is_null());
        let unknown = CString::new("DCPSNothing").unwrap();
        assert!(zerodds_builtin_subscriber_lookup_datareader(bs, unknown.as_ptr()).is_null());

        let name = |n: &str| CString::new(n).unwrap();
        let participants =
            zerodds_builtin_subscriber_lookup_datareader(bs, name("DCPSParticipant").as_ptr());
        let topics = zerodds_builtin_subscriber_lookup_datareader(bs, name("DCPSTopic").as_ptr());
        let publications =
            zerodds_builtin_subscriber_lookup_datareader(bs, name("DCPSPublication").as_ptr());
        let subscriptions = zerodds_builtin_subscriber_lookup_datareader(
            bs_writer_side,
            name("DCPSSubscription").as_ptr(),
        );
        assert_eq!(
            zerodds_builtin_reader_kind(participants),
            ZERODDS_BUILTIN_PARTICIPANT
        );
        assert_eq!(zerodds_builtin_reader_kind(topics), ZERODDS_BUILTIN_TOPIC);
        assert_eq!(
            zerodds_builtin_reader_kind(publications),
            ZERODDS_BUILTIN_PUBLICATION
        );
        assert_eq!(
            zerodds_builtin_reader_kind(subscriptions),
            ZERODDS_BUILTIN_SUBSCRIPTION
        );

        // The other participant's writer: a participant, a topic and a publication appear.
        let topic = zerodds_dp_create_topic(
            writer_side,
            topic_name.as_ptr(),
            type_name.as_ptr(),
            ptr::null(),
        );
        let publisher = zerodds_dp_create_publisher(writer_side, ptr::null());
        let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        assert!(!writer.is_null());
        // And this side's reader: a subscription.
        let local_topic = zerodds_dp_create_topic(
            reader_side,
            topic_name.as_ptr(),
            type_name.as_ptr(),
            ptr::null(),
        );
        let subscriber = zerodds_dp_create_subscriber(reader_side, ptr::null());
        let reader = zerodds_sub_create_datareader(subscriber, local_topic, ptr::null());
        assert!(!reader.is_null());

        assert!(
            read_until(participants, false, |s| s.iter().any(|x| x.info.valid_data
                && x.kind == ZERODDS_BUILTIN_PARTICIPANT
                && x.participant.guid != [0u8; 16])),
            "a participant sample with its key"
        );
        assert!(
            read_until(publications, false, |s| s.iter().any(|x| x.info.valid_data
                && !x.publication.topic_name.is_null()
                && std::ffi::CStr::from_ptr(x.publication.topic_name).to_str()
                    == Ok("CBuiltinReader"))),
            "the publication of the other participant's writer, by topic name"
        );
        assert!(
            read_until(subscriptions, false, |s| s.iter().any(|x| x
                .info
                .valid_data
                && !x.subscription.topic_name.is_null()
                && std::ffi::CStr::from_ptr(x.subscription.topic_name).to_str()
                    == Ok("CBuiltinReader"))),
            "the subscription of the other participant's reader, by topic name"
        );
        assert!(
            read_until(topics, false, |s| s.iter().any(|x| x.info.valid_data
                && !x.topic.name.is_null()
                && std::ffi::CStr::from_ptr(x.topic.name).to_str()
                    == Ok("CBuiltinReader"))),
            "the topic"
        );

        // `take` removes what it returned.
        assert!(
            read_until(participants, true, |s| !s.is_empty()),
            "take returns the samples"
        );
        let mut samples: *mut ZeroDdsBuiltinSample = ptr::null_mut();
        let mut count = 0usize;
        assert_eq!(
            zerodds_builtin_reader_read(participants, 0, &mut samples, &mut count),
            OK
        );
        zerodds_builtin_samples_free(samples, count);
        assert!(
            count <= 1,
            "after a take only samples that arrived since are left, got {count}"
        );

        for r in [participants, topics, publications, subscriptions] {
            zerodds_builtin_reader_destroy(r);
        }
        zerodds_builtin_subscriber_destroy(bs);
        zerodds_builtin_subscriber_destroy(bs_writer_side);
        let _ = zerodds_dpf_delete_participant(f, writer_side);
        let _ = zerodds_dpf_delete_participant(f, reader_side);
    }
}
