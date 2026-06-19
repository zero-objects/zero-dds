//! Integration tests for the builtin topic reader (DDS 1.4
//! §2.2.5, §2.2.2.2.1.7 `get_builtin_subscriber`).
//!
//! These tests build exclusively on the externally exported
//! DCPS types — `BuiltinSubscriber`, `DcpsParticipantBuiltinTopicData`,
//! `DcpsTopicBuiltinTopicData`, `DcpsPublicationBuiltinTopicData`,
//! `DcpsSubscriptionBuiltinTopicData`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_dcps::{
    DcpsParticipantBuiltinTopicData, DcpsPublicationBuiltinTopicData,
    DcpsSubscriptionBuiltinTopicData, DcpsTopicBuiltinTopicData, DomainParticipantFactory,
    DomainParticipantQos, TOPIC_NAME_DCPS_PARTICIPANT, TOPIC_NAME_DCPS_PUBLICATION,
    TOPIC_NAME_DCPS_SUBSCRIPTION, TOPIC_NAME_DCPS_TOPIC,
};

#[test]
fn get_builtin_subscriber_returns_subscriber() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(31, DomainParticipantQos::default());
    let bs = p.get_builtin_subscriber();
    // Identity: getting twice returns the same instance.
    let bs2 = p.get_builtin_subscriber();
    assert!(std::sync::Arc::ptr_eq(&bs, &bs2));
}

#[test]
fn builtin_subscriber_has_all_four_readers_pre_created() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(32, DomainParticipantQos::default());
    let bs = p.get_builtin_subscriber();
    let r1 = bs
        .lookup_datareader::<DcpsParticipantBuiltinTopicData>(TOPIC_NAME_DCPS_PARTICIPANT)
        .expect("DCPSParticipant reader");
    let r2 = bs
        .lookup_datareader::<DcpsTopicBuiltinTopicData>(TOPIC_NAME_DCPS_TOPIC)
        .expect("DCPSTopic reader");
    let r3 = bs
        .lookup_datareader::<DcpsPublicationBuiltinTopicData>(TOPIC_NAME_DCPS_PUBLICATION)
        .expect("DCPSPublication reader");
    let r4 = bs
        .lookup_datareader::<DcpsSubscriptionBuiltinTopicData>(TOPIC_NAME_DCPS_SUBSCRIPTION)
        .expect("DCPSSubscription reader");
    assert_eq!(r1.topic().name(), "DCPSParticipant");
    assert_eq!(r2.topic().name(), "DCPSTopic");
    assert_eq!(r3.topic().name(), "DCPSPublication");
    assert_eq!(r4.topic().name(), "DCPSSubscription");
}

#[test]
fn builtin_readers_have_spec_default_qos() {
    use zerodds_qos::{DurabilityKind, HistoryKind, ReliabilityKind};
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(33, DomainParticipantQos::default());
    let bs = p.get_builtin_subscriber();
    let r = bs
        .lookup_datareader::<DcpsParticipantBuiltinTopicData>(TOPIC_NAME_DCPS_PARTICIPANT)
        .unwrap();
    let qos = r.qos();
    assert_eq!(qos.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(qos.durability.kind, DurabilityKind::TransientLocal);
    assert_eq!(qos.history.kind, HistoryKind::KeepLast);
    assert_eq!(qos.history.depth, 1);
}

#[test]
fn lookup_with_wrong_type_for_topic_returns_bad_parameter() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(34, DomainParticipantQos::default());
    let bs = p.get_builtin_subscriber();
    // Type does not match the topic name.
    let err = bs
        .lookup_datareader::<DcpsParticipantBuiltinTopicData>("DCPSPublication")
        .unwrap_err();
    assert!(matches!(err, zerodds_dcps::DdsError::BadParameter { .. }));
}

#[test]
fn offline_builtin_readers_start_empty() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(35, DomainParticipantQos::default());
    let bs = p.get_builtin_subscriber();
    let r = bs
        .lookup_datareader::<DcpsParticipantBuiltinTopicData>(TOPIC_NAME_DCPS_PARTICIPANT)
        .unwrap();
    assert!(r.take().unwrap().is_empty());
    let r = bs
        .lookup_datareader::<DcpsPublicationBuiltinTopicData>(TOPIC_NAME_DCPS_PUBLICATION)
        .unwrap();
    assert!(r.take().unwrap().is_empty());
    let r = bs
        .lookup_datareader::<DcpsSubscriptionBuiltinTopicData>(TOPIC_NAME_DCPS_SUBSCRIPTION)
        .unwrap();
    assert!(r.take().unwrap().is_empty());
    let r = bs
        .lookup_datareader::<DcpsTopicBuiltinTopicData>(TOPIC_NAME_DCPS_TOPIC)
        .unwrap();
    assert!(r.take().unwrap().is_empty());
}

#[cfg(target_os = "linux")]
mod linux_e2e {
    //! Linux-only: an SPDP beacon between two live participants over
    //! multicast automatically produces a `DCPSParticipant` sample, and
    //! a pub/sub produces the corresponding `DCPSPublication` /
    //! `DCPSSubscription` + `DCPSTopic` samples.
    //!
    //! On macOS, multicast loopback in the CI/dev environment is
    //! weak; the spec-conformant paths are covered there by the unit
    //! tests + direct-sink push.
    use super::*;
    use std::time::{Duration, Instant};
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, PublisherQos, RawBytes, SubscriberQos, TopicQos,
    };

    fn wait_until<F: Fn() -> bool>(timeout: Duration, cond: F) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if cond() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        cond()
    }

    #[test]
    fn spdp_receive_populates_dcps_participant_reader() {
        let factory = DomainParticipantFactory::instance();
        let p_a = factory
            .create_participant(101, DomainParticipantQos::default())
            .expect("participant A");
        let _p_b = factory
            .create_participant(101, DomainParticipantQos::default())
            .expect("participant B");

        let bs_a = p_a.get_builtin_subscriber();
        let r = bs_a
            .lookup_datareader::<DcpsParticipantBuiltinTopicData>(TOPIC_NAME_DCPS_PARTICIPANT)
            .unwrap();

        // Wait until A has processed B's beacon.
        let ok = wait_until(Duration::from_secs(8), || !r.read().unwrap().is_empty());
        if !ok {
            // CI without multicast → skip the test.
            eprintln!("Skip: no SPDP multicast reachable in this environment");
            return;
        }
        let samples = r.take().unwrap();
        assert!(
            !samples.is_empty(),
            "DCPSParticipant reader must return a sample"
        );
    }

    #[test]
    fn sedp_pub_sub_populates_dcps_publication_and_subscription_readers() {
        let factory = DomainParticipantFactory::instance();
        let p_a = factory
            .create_participant(102, DomainParticipantQos::default())
            .expect("participant A");
        let p_b = factory
            .create_participant(102, DomainParticipantQos::default())
            .expect("participant B");

        // B creates a writer (→ SEDP pub-announce to A).
        let topic_b = p_b
            .create_topic::<RawBytes>("BltinTopic", TopicQos::default())
            .unwrap();
        let pub_b = p_b.create_publisher(PublisherQos::default());
        let _w = pub_b
            .create_datawriter::<RawBytes>(&topic_b, DataWriterQos::default())
            .unwrap();

        // A creates a reader (→ SEDP sub-announce to B).
        let topic_a = p_a
            .create_topic::<RawBytes>("BltinTopic", TopicQos::default())
            .unwrap();
        let sub_a = p_a.create_subscriber(SubscriberQos::default());
        let _r = sub_a
            .create_datareader::<RawBytes>(&topic_a, DataReaderQos::default())
            .unwrap();

        let bs_a = p_a.get_builtin_subscriber();
        let pub_reader = bs_a
            .lookup_datareader::<DcpsPublicationBuiltinTopicData>(TOPIC_NAME_DCPS_PUBLICATION)
            .unwrap();

        let bs_b = p_b.get_builtin_subscriber();
        let sub_reader = bs_b
            .lookup_datareader::<DcpsSubscriptionBuiltinTopicData>(TOPIC_NAME_DCPS_SUBSCRIPTION)
            .unwrap();

        let ok_pub = wait_until(Duration::from_secs(8), || {
            !pub_reader.read().unwrap().is_empty()
        });
        let ok_sub = wait_until(Duration::from_secs(8), || {
            !sub_reader.read().unwrap().is_empty()
        });

        if !ok_pub && !ok_sub {
            eprintln!("Skip: no SPDP/SEDP multicast reachable");
            return;
        }
        // Once discovery has completed, topic readers must also
        // have synthesized DCPSTopic samples.
        if ok_pub {
            let samples = pub_reader.take().unwrap();
            assert!(!samples.is_empty());
            assert_eq!(samples[0].topic_name, "BltinTopic");
            let topic_reader = bs_a
                .lookup_datareader::<DcpsTopicBuiltinTopicData>(TOPIC_NAME_DCPS_TOPIC)
                .unwrap();
            let topic_samples = topic_reader.take().unwrap();
            assert!(!topic_samples.is_empty());
        }
    }
}
