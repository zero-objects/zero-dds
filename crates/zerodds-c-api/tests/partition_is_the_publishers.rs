// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PARTITION is a Publisher and Subscriber policy (DDS 1.4 §2.2.3.13): a
//! writer and a reader created with no QoS of their own match exactly when
//! their Publisher's and Subscriber's partitions overlap.
//!
//! The endpoint took its partition from the C ABI's DataWriterQos /
//! DataReaderQos only, so a partition set where DDS puts it gated nothing:
//! `A` on the publisher matched `B` on the subscriber (measured in
//! zerodds-examples idl-conformance/_qos/cpp).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::{CString, c_char};
use std::ptr;

use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
};
use zerodds::publisher_ffi::{zerodds_dw_wait_for_matched, zerodds_pub_create_datawriter};
use zerodds::qos_ffi::ZeroDdsPublisherQos;
use zerodds::subscriber_ffi::zerodds_sub_create_datareader;

const OK: i32 = 0;

/// A Publisher/Subscriber QoS naming one partition, entities auto-enabled.
fn group_qos(names: &[*const c_char]) -> ZeroDdsPublisherQos {
    // SAFETY: ZeroDdsPublisherQos is repr(C) plain data; all-zero is a null
    // name array, no group data and default presentation, each valid bits.
    let mut qos: ZeroDdsPublisherQos = unsafe { core::mem::zeroed() };
    qos.partition.names = names.as_ptr();
    qos.partition.names_len = names.len();
    qos.entity_factory.autoenable_created_entities = true;
    qos
}

/// Whether a default-QoS writer and reader match across two participants
/// whose Publisher and Subscriber name `pub_name` and `sub_name`.
fn matches(topic: &str, pub_name: &str, sub_name: &str) -> bool {
    let lease = zerodds_test_domains::lease_domain();
    let topic_c = CString::new(topic).unwrap();
    let type_c = CString::new("PartitionProbe").unwrap();
    let pub_c = CString::new(pub_name).unwrap();
    let sub_c = CString::new(sub_name).unwrap();
    let pub_names = [pub_c.as_ptr()];
    let sub_names = [sub_c.as_ptr()];
    let pub_qos = group_qos(&pub_names);
    let sub_qos = group_qos(&sub_names);
    // SAFETY: every pointer comes from the create call right above its use and
    // is checked; the name arrays and CStrings outlive the creates that read
    // them; both participants are deleted exactly once.
    unsafe {
        let factory = zerodds_dpf_get_instance();
        let dp_pub = zerodds_dpf_create_participant(factory, lease.id() as u32, ptr::null());
        let dp_sub = zerodds_dpf_create_participant(factory, lease.id() as u32, ptr::null());
        assert!(!dp_pub.is_null() && !dp_sub.is_null());
        let t_pub = zerodds_dp_create_topic(dp_pub, topic_c.as_ptr(), type_c.as_ptr(), ptr::null());
        let t_sub = zerodds_dp_create_topic(dp_sub, topic_c.as_ptr(), type_c.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp_pub, &pub_qos);
        let subscriber = zerodds_dp_create_subscriber(dp_sub, &sub_qos);
        assert!(!publisher.is_null() && !subscriber.is_null());
        let writer = zerodds_pub_create_datawriter(publisher, t_pub, ptr::null());
        let reader = zerodds_sub_create_datareader(subscriber, t_sub, ptr::null());
        assert!(!writer.is_null() && !reader.is_null());
        let matched = zerodds_dw_wait_for_matched(writer, 1, 2_000) == OK;
        let _ = zerodds_dpf_delete_participant(factory, dp_pub);
        let _ = zerodds_dpf_delete_participant(factory, dp_sub);
        matched
    }
}

#[test]
fn the_publishers_and_subscribers_partitions_decide_the_match() {
    assert!(
        matches("CFfiPartitionSame", "A", "A"),
        "partition A on both sides did not match"
    );
    assert!(
        !matches("CFfiPartitionOther", "A", "B"),
        "partition A on the publisher matched B on the subscriber"
    );
}
