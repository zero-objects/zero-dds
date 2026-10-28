// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A QoS value the service does not support is UNSUPPORTED (DDS 1.4
//! §2.2.2.1.1.1: "If the application supplies a non-default value for a QoS
//! policy that is not supported by the implementation of the service … the
//! operation will fail and return UNSUPPORTED").

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::{
    DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, TopicQos,
};

/// The writers of ZeroDDS emit XCDR1, XCDR2 or XML. Any other
/// DATA_REPRESENTATION id is a value the service does not support: the
/// writer is not created, and the return code is UNSUPPORTED (2).
#[test]
fn a_data_representation_the_service_does_not_emit_is_unsupported() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("Unsupported", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());

    let refused = publisher
        .create_datawriter::<RawBytes>(
            &topic,
            DataWriterQos {
                data_representation: Some(vec![7]),
                ..DataWriterQos::default()
            },
        )
        .expect_err("id 7 is no representation the service emits");
    assert!(
        matches!(refused, DdsError::Unsupported { .. }),
        "{refused:?}"
    );
    assert_eq!(refused.as_return_code(), 2, "RETCODE_UNSUPPORTED");
    assert!(!publisher.has_datawriters(), "nothing was created");

    for supported in [vec![0_i16], vec![2], vec![1], vec![], vec![2, 0]] {
        publisher
            .create_datawriter::<RawBytes>(
                &topic,
                DataWriterQos {
                    data_representation: Some(supported.clone()),
                    ..DataWriterQos::default()
                },
            )
            .unwrap_or_else(|e| panic!("{supported:?} is supported: {e:?}"));
    }
}
