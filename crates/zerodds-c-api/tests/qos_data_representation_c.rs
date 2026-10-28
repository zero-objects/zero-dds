// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DataRepresentation and TypeConsistencyEnforcement in the C QoS structs
//! (DDS-XTypes 1.3 Annex D: `TopicQos::representation`,
//! `DataWriterQos::representation`, `DataReaderQos::representation` and
//! `DataReaderQos::type_consistency`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::CString;
use std::ptr;

use zerodds::extra_ffi::{
    zerodds_dp_get_default_topic_qos, zerodds_dr_get_qos, zerodds_dw_get_qos,
    zerodds_sub_get_default_datareader_qos,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
    zerodds_dp_delete_contained_entities,
};
use zerodds::publisher_ffi::zerodds_pub_create_datawriter;
use zerodds::qos_ffi::{
    ZERODDS_DATA_REPRESENTATION_MAX, ZeroDdsDataReaderQos, ZeroDdsDataRepresentationQosPolicy,
    ZeroDdsDataWriterQos, ZeroDdsTopicQos, dr_qos_from_c, dr_qos_from_c_checked, dw_qos_from_c,
    topic_qos_from_c,
};
use zerodds::subscriber_ffi::zerodds_sub_create_datareader;
use zerodds::topic_ffi::zerodds_topic_get_qos;
use zerodds_types::qos::{TypeConsistencyEnforcement, TypeConsistencyKind};

const OK: i32 = 0;

fn list(ids: &[i16]) -> ZeroDdsDataRepresentationQosPolicy {
    let mut policy = ZeroDdsDataRepresentationQosPolicy {
        is_set: 1,
        count: ids.len() as u8,
        ids: [0; ZERODDS_DATA_REPRESENTATION_MAX],
    };
    policy.ids[..ids.len()].copy_from_slice(ids);
    policy
}

/// An all-zero struct, which the tests of this crate build with `zeroed`, is
/// the default: the policy is left unset and the runtime offers its own.
#[test]
fn a_zeroed_qos_leaves_the_representation_at_its_default() {
    // SAFETY: plain data.
    unsafe {
        let topic: ZeroDdsTopicQos = std::mem::zeroed();
        assert_eq!(topic_qos_from_c(&topic).data_representation, None);
        let writer: ZeroDdsDataWriterQos = std::mem::zeroed();
        assert_eq!(dw_qos_from_c(&writer).data_representation, None);
        let reader: ZeroDdsDataReaderQos = std::mem::zeroed();
        let qos = dr_qos_from_c(&reader);
        assert_eq!(qos.data_representation, None);
        assert_eq!(qos.type_consistency, TypeConsistencyEnforcement::default());
    }
}

/// §7.6.3.1.1: the list of a policy that is set is the list, and an empty
/// list is the single element XCDR.
#[test]
fn a_set_list_is_the_list_and_an_empty_one_is_kept_empty() {
    // SAFETY: plain data.
    unsafe {
        let mut topic: ZeroDdsTopicQos = std::mem::zeroed();
        topic.data_representation = list(&[2, 0]);
        assert_eq!(
            topic_qos_from_c(&topic).data_representation,
            Some(vec![2, 0])
        );
        let mut writer: ZeroDdsDataWriterQos = std::mem::zeroed();
        writer.data_representation = list(&[]);
        assert_eq!(dw_qos_from_c(&writer).data_representation, Some(vec![]));
        let mut reader: ZeroDdsDataReaderQos = std::mem::zeroed();
        reader.data_representation = list(&[0, 2, -3]);
        assert_eq!(
            dr_qos_from_c(&reader).data_representation,
            Some(vec![0, 2, -3])
        );
    }
}

/// Annex D's `TypeConsistencyEnforcementQosPolicy` reaches the reader's QoS,
/// and a kind it does not define is refused.
#[test]
fn the_type_consistency_of_a_reader_is_read_and_a_wrong_kind_is_refused() {
    // SAFETY: plain data.
    unsafe {
        let mut reader: ZeroDdsDataReaderQos = std::mem::zeroed();
        reader.type_consistency.is_set = 1;
        reader.type_consistency.kind = 0;
        reader.type_consistency.ignore_member_names = 1;
        reader.type_consistency.force_type_validation = 1;
        let qos = dr_qos_from_c_checked(&reader).expect("DISALLOW_TYPE_COERCION");
        assert_eq!(
            qos.type_consistency.kind,
            TypeConsistencyKind::DisallowTypeCoercion
        );
        assert!(qos.type_consistency.ignore_member_names);
        assert!(qos.type_consistency.force_type_validation);
        assert!(!qos.type_consistency.ignore_sequence_bounds);

        reader.type_consistency.kind = 1;
        let qos = dr_qos_from_c_checked(&reader).expect("ALLOW_TYPE_COERCION");
        assert_eq!(
            qos.type_consistency.kind,
            TypeConsistencyKind::AllowTypeCoercion
        );

        reader.type_consistency.kind = 2;
        assert!(dr_qos_from_c_checked(&reader).is_err(), "kind 2 is no kind");
    }
}

/// The values go through the entities: a topic keeps the list it was created
/// with, a writer and a reader too, and a reader created with a kind Annex D
/// does not define is not created.
#[test]
fn the_entities_hand_the_representation_back() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CRepresentation").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    // SAFETY: every pointer comes from the call above its use and is
    // checked; the participant is deleted once at the end.
    unsafe {
        let factory = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(factory, lease.id() as u32, ptr::null());
        assert!(!dp.is_null());

        let mut topic_qos: ZeroDdsTopicQos = std::mem::zeroed();
        assert_eq!(zerodds_dp_get_default_topic_qos(dp, &mut topic_qos), OK);
        assert_eq!(
            topic_qos.data_representation.is_set, 0,
            "the default of the policy is left unset"
        );
        topic_qos.data_representation = list(&[2, 0]);
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), &topic_qos);
        assert!(!topic.is_null());
        let mut got: ZeroDdsTopicQos = std::mem::zeroed();
        assert_eq!(zerodds_topic_get_qos(topic, &mut got), OK);
        assert_eq!(got.data_representation.is_set, 1);
        assert_eq!(got.data_representation.count, 2);
        assert_eq!(&got.data_representation.ids[..2], &[2, 0]);

        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        let mut writer_qos: ZeroDdsDataWriterQos = std::mem::zeroed();
        writer_qos.data_representation = list(&[0]);
        let writer = zerodds_pub_create_datawriter(publisher, topic, &writer_qos);
        assert!(!writer.is_null());
        let mut writer_got: ZeroDdsDataWriterQos = std::mem::zeroed();
        assert_eq!(zerodds_dw_get_qos(writer, &mut writer_got), OK);
        assert_eq!(writer_got.data_representation.count, 1);
        assert_eq!(writer_got.data_representation.ids[0], 0);

        let mut reader_qos: ZeroDdsDataReaderQos = std::mem::zeroed();
        assert_eq!(
            zerodds_sub_get_default_datareader_qos(subscriber, &mut reader_qos),
            OK
        );
        reader_qos.data_representation = list(&[0, 2]);
        reader_qos.type_consistency.is_set = 1;
        reader_qos.type_consistency.kind = 0;
        let reader = zerodds_sub_create_datareader(subscriber, topic, &reader_qos);
        assert!(!reader.is_null());
        let mut reader_got: ZeroDdsDataReaderQos = std::mem::zeroed();
        assert_eq!(zerodds_dr_get_qos(reader, &mut reader_got), OK);
        assert_eq!(reader_got.data_representation.count, 2);
        assert_eq!(&reader_got.data_representation.ids[..2], &[0, 2]);
        assert_eq!(reader_got.type_consistency.is_set, 1);
        assert_eq!(reader_got.type_consistency.kind, 0);

        reader_qos.type_consistency.kind = 7;
        let refused = zerodds_sub_create_datareader(subscriber, topic, &reader_qos);
        assert!(refused.is_null(), "kind 7 is no TypeConsistencyKind");

        assert_eq!(zerodds_dp_delete_contained_entities(dp), OK);
        assert_eq!(zerodds_dpf_delete_participant(factory, dp), OK);
    }
}
