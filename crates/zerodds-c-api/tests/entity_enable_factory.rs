// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A DomainParticipantFactory whose ENTITY_FACTORY has
//! `autoenable_created_entities` FALSE creates its participants disabled, and
//! the entities of a disabled participant are created disabled whatever their
//! own factories say (DDS 1.4 §2.2.2.1.1.7, §2.2.3.20). Calling `enable` on
//! an entity whose factory is not enabled is PRECONDITION_NOT_MET.
//!
//! The factory is a process-wide singleton, so this test has a binary of its
//! own: set in the middle of another test file it would change the
//! participants of every test running beside it.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::CString;
use std::ptr;

use zerodds::entity_ffi::zerodds_pub_enable;
use zerodds::entity_ffi::{zerodds_dp_enable, zerodds_topic_enable};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
    zerodds_dpf_set_qos,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_topic, zerodds_dp_delete_publisher,
    zerodds_dp_delete_topic, zerodds_dp_get_current_time,
};
use zerodds::publisher_ffi::zerodds_pub_suspend_publications;
use zerodds::qos_ffi::ZeroDdsDomainParticipantFactoryQos;

const OK: i32 = 0;
const PRECONDITION_NOT_MET: i32 = -5;
const NOT_ENABLED: i32 = -9;

fn set_autoenable(on: bool) {
    // SAFETY: plain data, the factory is the process singleton.
    unsafe {
        let mut qos: ZeroDdsDomainParticipantFactoryQos = std::mem::zeroed();
        qos.entity_factory.autoenable_created_entities = on;
        assert_eq!(zerodds_dpf_set_qos(zerodds_dpf_get_instance(), &qos), OK);
    }
}

/// Puts the factory back, whatever the test does.
struct RestoreFactory;

impl Drop for RestoreFactory {
    fn drop(&mut self) {
        set_autoenable(true);
    }
}

#[test]
fn a_participant_of_a_factory_that_does_not_autoenable_is_created_disabled() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CEntityEnableFactory").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    let _restore = RestoreFactory;
    set_autoenable(false);
    // SAFETY: every pointer comes from the create call above its use and is
    // checked; the participant is deleted once at the end.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        assert!(!dp.is_null());
        let mut time = std::mem::zeroed();
        assert_eq!(
            zerodds_dp_get_current_time(dp, &mut time),
            NOT_ENABLED,
            "a disabled participant refuses get_current_time"
        );
        // Created from a disabled factory: disabled, and not enabled by a
        // factory that is not.
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        assert!(!topic.is_null() && !publisher.is_null());
        assert_eq!(zerodds_pub_suspend_publications(publisher), NOT_ENABLED);
        assert_eq!(zerodds_topic_enable(topic), PRECONDITION_NOT_MET);
        assert_eq!(zerodds_pub_enable(publisher), PRECONDITION_NOT_MET);

        // `enable` of the participant, and from then on its entities.
        assert_eq!(zerodds_dp_enable(dp), OK);
        assert_eq!(zerodds_dp_enable(dp), OK, "enable is idempotent");
        assert_eq!(zerodds_dp_get_current_time(dp, &mut time), OK);
        assert_eq!(zerodds_topic_enable(topic), OK);
        assert_eq!(zerodds_pub_enable(publisher), OK);
        assert_eq!(zerodds_pub_suspend_publications(publisher), OK);

        zerodds_dp_delete_publisher(dp, publisher);
        zerodds_dp_delete_topic(dp, topic);
        assert_eq!(zerodds_dpf_delete_participant(f, dp), OK);
    }
}
