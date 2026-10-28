// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `enable`, `get_status_changes` and `get_instance_handle` of the C entities
//! (DDS 1.4 §2.2.2.1.1.6 to §2.2.2.1.1.8), and the ENTITY_FACTORY policy
//! (§2.2.3.20) they rest on.
//!
//! Every C entity was created enabled whatever the factory's
//! `autoenable_created_entities` said, and a DataWriter or DataReader was
//! registered with the runtime — announced and matched — at its creation.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::CString;
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::condition_ffi::zerodds_dr_has_data;
use zerodds::entities::ZeroDdsDataReader;
use zerodds::entity_ffi::{
    zerodds_dp_enable, zerodds_dp_get_instance_handle, zerodds_dp_get_status_changes,
    zerodds_dr_enable, zerodds_dr_get_instance_handle, zerodds_dr_get_status_changes,
    zerodds_dw_enable, zerodds_dw_get_instance_handle, zerodds_dw_get_status_changes,
    zerodds_pub_enable, zerodds_pub_get_instance_handle, zerodds_pub_get_status_changes,
    zerodds_sub_enable, zerodds_sub_get_instance_handle, zerodds_sub_get_status_changes,
    zerodds_topic_enable, zerodds_topic_get_instance_handle, zerodds_topic_get_status_changes,
};
use zerodds::extra_ffi::{
    zerodds_dp_get_default_publisher_qos, zerodds_dp_get_default_subscriber_qos,
    zerodds_dr_get_matched_publications, zerodds_dw_get_matched_subscriptions, zerodds_dw_get_qos,
    zerodds_dw_set_qos,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant,
    zerodds_dpf_get_default_participant_qos, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_contains_entity, zerodds_dp_create_publisher, zerodds_dp_create_subscriber,
    zerodds_dp_create_topic,
};
use zerodds::publisher_ffi::{
    ZeroDdsPublicationMatchedStatus, zerodds_dw_assert_liveliness,
    zerodds_dw_get_publication_matched_status, zerodds_dw_wait_for_matched, zerodds_dw_write,
    zerodds_pub_create_datawriter, zerodds_pub_suspend_publications,
};
use zerodds::qos_ffi::{ZeroDdsDataWriterQos, ZeroDdsDomainParticipantQos, ZeroDdsPublisherQos};
use zerodds::subscriber_ffi::{
    ZeroDdsSampleArray, ZeroDdsSubscriptionMatchedStatus,
    zerodds_dr_get_subscription_matched_status, zerodds_dr_return_loan, zerodds_dr_take,
    zerodds_sub_create_datareader,
};
use zerodds_dcps::psm_constants::status;

const OK: i32 = 0;
const PRECONDITION_NOT_MET: i32 = -5;
const NOT_ENABLED: i32 = -9;
const IMMUTABLE_POLICY: i32 = -10;

/// A participant whose ENTITY_FACTORY does not enable what it creates.
fn manual_participant_qos() -> ZeroDdsDomainParticipantQos {
    // SAFETY: plain data; `zerodds_dpf_get_default_participant_qos` fills it.
    unsafe {
        let mut qos: ZeroDdsDomainParticipantQos = std::mem::zeroed();
        assert_eq!(
            zerodds_dpf_get_default_participant_qos(zerodds_dpf_get_instance(), &mut qos),
            OK
        );
        qos.entity_factory.autoenable_created_entities = false;
        qos
    }
}

/// How many samples one take of `dr` returns.
fn take_count(dr: *mut ZeroDdsDataReader) -> usize {
    let mut arr = ZeroDdsSampleArray {
        buffers: ptr::null_mut(),
        lengths: ptr::null_mut(),
        infos: ptr::null_mut(),
        count: 0,
        loan_token: ptr::null_mut(),
    };
    // SAFETY: the reader is live; `arr` is a valid out-parameter.
    unsafe {
        let _ = zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0);
        let n = arr.count;
        if n > 0 {
            assert_eq!(zerodds_dr_return_loan(dr, &mut arr), OK);
        }
        n
    }
}

fn wait_for(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn entities_of_a_participant_that_does_not_autoenable_are_created_disabled() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CEntityEnableManual").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    let qos = manual_participant_qos();
    // SAFETY: every pointer comes from the create call above its use and is
    // checked; the participant is deleted once at the end.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, &qos);
        assert!(!dp.is_null());
        // The participant's own factory is the participant factory, which
        // enables it; `enable` of an enabled entity is OK and has no effect.
        assert_eq!(zerodds_dp_enable(dp), OK);
        assert_eq!(zerodds_dp_enable(dp), OK);

        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        assert!(!topic.is_null() && !publisher.is_null() && !subscriber.is_null());

        // A disabled Publisher refuses what is not on the list of §2.2.2.1.1.7.
        assert_eq!(zerodds_pub_suspend_publications(publisher), NOT_ENABLED);
        // Its writers are created disabled too, and enabling one is refused
        // while its Publisher is not enabled.
        let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        assert!(!writer.is_null());
        assert_eq!(zerodds_dw_write(writer, [1u8].as_ptr(), 1, 0), NOT_ENABLED);
        assert_eq!(zerodds_dw_enable(writer), PRECONDITION_NOT_MET);
        let reader = zerodds_sub_create_datareader(subscriber, topic, ptr::null());
        assert!(!reader.is_null());
        assert_eq!(zerodds_dr_enable(reader), PRECONDITION_NOT_MET);

        // The Publisher's default ENTITY_FACTORY enables what it created
        // when it is enabled itself.
        assert_eq!(zerodds_topic_enable(topic), OK);
        assert_eq!(zerodds_pub_enable(publisher), OK);
        assert_eq!(zerodds_pub_enable(publisher), OK, "enable is idempotent");
        assert_eq!(zerodds_pub_suspend_publications(publisher), OK);
        assert_eq!(zerodds_dw_assert_liveliness(writer), OK);
        assert_eq!(zerodds_sub_enable(subscriber), OK);
        assert_eq!(take_count(reader), 0);

        assert_eq!(zerodds_dpf_delete_participant(f, dp), PRECONDITION_NOT_MET);
    }
}

#[test]
fn a_writer_of_a_publisher_that_does_not_autoenable_is_not_announced_until_enabled() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CEntityEnableWriter").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    // SAFETY: as above.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        assert!(!dp.is_null());
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let mut pub_qos: ZeroDdsPublisherQos = std::mem::zeroed();
        assert_eq!(zerodds_dp_get_default_publisher_qos(dp, &mut pub_qos), OK);
        pub_qos.entity_factory.autoenable_created_entities = false;
        let publisher = zerodds_dp_create_publisher(dp, &pub_qos);
        let mut sub_qos: ZeroDdsPublisherQos = std::mem::zeroed();
        assert_eq!(zerodds_dp_get_default_subscriber_qos(dp, &mut sub_qos), OK);
        sub_qos.entity_factory.autoenable_created_entities = false;
        let subscriber = zerodds_dp_create_subscriber(dp, &sub_qos);
        assert!(!topic.is_null() && !publisher.is_null() && !subscriber.is_null());

        let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        let reader = zerodds_sub_create_datareader(subscriber, topic, ptr::null());
        assert!(!writer.is_null() && !reader.is_null());
        let (rt, w_eid, r_eid) = ((*writer).rt.clone(), (*writer).eid, (*reader).eid);

        // Created disabled: not announced over SEDP, not matched.
        assert!(rt.latest_announced_publication(w_eid).is_none());
        assert!(rt.latest_announced_subscription(r_eid).is_none());
        assert_eq!(zerodds_dw_write(writer, [1u8].as_ptr(), 1, 0), NOT_ENABLED);
        assert_eq!(zerodds_dw_wait_for_matched(writer, 1, 10), NOT_ENABLED);
        assert_eq!(zerodds_dw_assert_liveliness(writer), NOT_ENABLED);
        assert_eq!(take_count(reader), 0);
        let mut handles = [0u64; 4];
        let mut count = 0usize;
        assert_eq!(
            zerodds_dw_get_matched_subscriptions(writer, handles.as_mut_ptr(), &mut count, 4),
            NOT_ENABLED
        );
        // The get status operations are callable and untriggered.
        let mut matched: ZeroDdsPublicationMatchedStatus = std::mem::zeroed();
        matched.total_count = 99;
        assert_eq!(
            zerodds_dw_get_publication_matched_status(writer, &mut matched),
            OK
        );
        assert_eq!((matched.total_count, matched.current_count), (0, 0));
        let mut changes = 99u32;
        assert_eq!(zerodds_dw_get_status_changes(writer, &mut changes), OK);
        assert_eq!(changes, 0);

        // The QoS can be set, immutable policies included, before the enable
        // (§2.2.2.1.1.2), and the writer is announced with it.
        let mut dw_qos: ZeroDdsDataWriterQos = std::mem::zeroed();
        assert_eq!(zerodds_dw_get_qos(writer, &mut dw_qos), OK);
        dw_qos.durability.kind = 1;
        assert_eq!(zerodds_dw_set_qos(writer, &dw_qos), OK);

        // The publisher and the subscriber are enabled — the participant
        // autoenables what it creates; only their own children are not — so
        // enabling them again is idempotent and enables no writer or reader.
        assert_eq!(zerodds_pub_enable(publisher), OK);
        assert_eq!(zerodds_sub_enable(subscriber), OK);
        assert!(
            rt.latest_announced_publication(w_eid).is_none(),
            "the publisher's enable does not enable its writer: it does not autoenable"
        );

        assert_eq!(zerodds_dw_enable(writer), OK);
        assert_eq!(zerodds_dw_enable(writer), OK, "enable is idempotent");
        let announced = rt
            .latest_announced_publication(w_eid)
            .expect("the enabled writer is announced");
        assert_eq!(
            announced.durability,
            zerodds_qos::DurabilityKind::TransientLocal,
            "announced with the QoS it had at the enable"
        );
        assert_eq!(
            zerodds_dw_set_qos(writer, &{
                let mut again = dw_qos;
                again.durability.kind = 0;
                again
            }),
            IMMUTABLE_POLICY,
            "an enabled writer's immutable policy is immutable"
        );

        assert_eq!(zerodds_dr_enable(reader), OK);
        assert!(rt.latest_announced_subscription(r_eid).is_some());
        assert_eq!(zerodds_dw_wait_for_matched(writer, 1, 5_000), OK);
        assert_eq!(zerodds_dw_write(writer, [7u8].as_ptr(), 1, 0), OK);
        wait_for("the sample reaches the reader", || take_count(reader) == 1);

        assert_eq!(zerodds_dpf_delete_participant(f, dp), PRECONDITION_NOT_MET);
    }
}

#[test]
fn instance_handles_are_the_ones_contains_entity_and_the_matched_lists_use() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CEntityEnableHandles").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    // SAFETY: as above.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        let reader = zerodds_sub_create_datareader(subscriber, topic, ptr::null());
        assert!(!writer.is_null() && !reader.is_null());

        let (mut h_dp, mut h_topic, mut h_pub, mut h_sub, mut h_dw, mut h_dr) =
            (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
        assert_eq!(zerodds_dp_get_instance_handle(dp, &mut h_dp), OK);
        assert_eq!(zerodds_topic_get_instance_handle(topic, &mut h_topic), OK);
        assert_eq!(zerodds_pub_get_instance_handle(publisher, &mut h_pub), OK);
        assert_eq!(zerodds_sub_get_instance_handle(subscriber, &mut h_sub), OK);
        assert_eq!(zerodds_dw_get_instance_handle(writer, &mut h_dw), OK);
        assert_eq!(zerodds_dr_get_instance_handle(reader, &mut h_dr), OK);
        let all = [h_dp, h_topic, h_pub, h_sub, h_dw, h_dr];
        for (i, a) in all.iter().enumerate() {
            assert_ne!(*a, 0, "entity {i} has HANDLE_NIL");
            assert_eq!(zerodds_dp_contains_entity(dp, *a), 1, "entity {i}");
            for b in &all[i + 1..] {
                assert_ne!(a, b, "two entities share a handle");
            }
        }
        assert_eq!(zerodds_dp_contains_entity(dp, 0), 0);
        assert_eq!(zerodds_dp_contains_entity(dp, 0x1234_5678), 0);

        // The handle a writer is listed under by its reader, and a reader
        // by its writer (§2.2.2.5.3.11, §2.2.2.4.2.12).
        assert_eq!(zerodds_dw_wait_for_matched(writer, 1, 5_000), OK);
        let mut handles = [0u64; 4];
        let mut count = 0usize;
        assert_eq!(
            zerodds_dr_get_matched_publications(reader, handles.as_mut_ptr(), &mut count, 4),
            OK
        );
        assert_eq!(&handles[..count], &[h_dw]);
        assert_eq!(
            zerodds_dw_get_matched_subscriptions(writer, handles.as_mut_ptr(), &mut count, 4),
            OK
        );
        assert_eq!(&handles[..count], &[h_dr]);

        assert_eq!(
            zerodds_dp_get_instance_handle(ptr::null_mut(), &mut h_dp),
            -2
        );
    }
}

#[test]
fn status_changes_show_what_happened_and_clear_when_the_status_is_read() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CEntityEnableStatus").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    // SAFETY: as above.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        let reader = zerodds_sub_create_datareader(subscriber, topic, ptr::null());
        assert!(!writer.is_null() && !reader.is_null());

        let changes = |get: &dyn Fn(*mut u32) -> i32| -> u32 {
            let mut out = u32::MAX;
            assert_eq!(get(&mut out), OK);
            out
        };
        let dw_changes = || changes(&|o| zerodds_dw_get_status_changes(writer, o));
        let dr_changes = || changes(&|o| zerodds_dr_get_status_changes(reader, o));
        let sub_changes = || changes(&|o| zerodds_sub_get_status_changes(subscriber, o));

        // Nothing has happened to a participant, a publisher, a topic.
        assert_eq!(changes(&|o| zerodds_dp_get_status_changes(dp, o)), 0);
        assert_eq!(
            changes(&|o| zerodds_pub_get_status_changes(publisher, o)),
            0
        );
        assert_eq!(changes(&|o| zerodds_topic_get_status_changes(topic, o)), 0);

        // The match triggers PUBLICATION_MATCHED and SUBSCRIPTION_MATCHED
        // (§2.2.4.1) until the statuses are read (§2.2.4.2).
        assert_eq!(zerodds_dw_wait_for_matched(writer, 1, 5_000), OK);
        wait_for("the writer's match is a status change", || {
            dw_changes() & status::PUBLICATION_MATCHED != 0
        });
        wait_for("the reader's match is a status change", || {
            dr_changes() & status::SUBSCRIPTION_MATCHED != 0
        });
        let mut pm: ZeroDdsPublicationMatchedStatus = std::mem::zeroed();
        assert_eq!(
            zerodds_dw_get_publication_matched_status(writer, &mut pm),
            OK
        );
        assert_eq!(pm.current_count, 1);
        assert_eq!(dw_changes() & status::PUBLICATION_MATCHED, 0);
        let mut sm: ZeroDdsSubscriptionMatchedStatus = std::mem::zeroed();
        assert_eq!(
            zerodds_dr_get_subscription_matched_status(reader, &mut sm),
            OK
        );
        assert_eq!(sm.current_count, 1);
        assert_eq!(dr_changes() & status::SUBSCRIPTION_MATCHED, 0);

        // A sample not read yet is DATA_AVAILABLE on the reader and
        // DATA_ON_READERS on its subscriber, until it is taken.
        assert_eq!(dr_changes() & status::DATA_AVAILABLE, 0);
        assert_eq!(sub_changes() & status::DATA_ON_READERS, 0);
        assert_eq!(zerodds_dw_write(writer, [3u8].as_ptr(), 1, 0), OK);
        wait_for("the sample arrives", || zerodds_dr_has_data(reader));
        assert_ne!(dr_changes() & status::DATA_AVAILABLE, 0);
        assert_ne!(sub_changes() & status::DATA_ON_READERS, 0);
        assert_eq!(take_count(reader), 1);
        assert_eq!(dr_changes() & status::DATA_AVAILABLE, 0);
        assert_eq!(sub_changes() & status::DATA_ON_READERS, 0);
    }
}
