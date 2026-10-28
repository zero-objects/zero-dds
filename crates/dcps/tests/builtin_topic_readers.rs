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
fn lookup_with_wrong_type_for_topic_is_nil() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(34, DomainParticipantQos::default());
    let bs = p.get_builtin_subscriber();
    // §2.2.5: "a nil value" for what is not a built-in reader of that type.
    assert!(
        bs.lookup_datareader::<DcpsParticipantBuiltinTopicData>("DCPSPublication")
            .is_none()
    );
    assert!(
        bs.lookup_datareader::<DcpsParticipantBuiltinTopicData>("NoSuchTopic")
            .is_none()
    );
}

/// DDS 1.4 §2.2.5: the built-in Subscriber is a Subscriber. It has a QoS, a
/// listener, a StatusCondition; its readers are the four built-in ones, so
/// `lookup_datareader` and `get_datareaders` find them; they are not deleted
/// by the application.
#[test]
fn the_builtin_subscriber_is_a_subscriber() {
    use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
    use zerodds_dcps::{DdsError, Entity};
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(37, DomainParticipantQos::default());
    let bs = p.get_builtin_subscriber();

    // QoS, listener and StatusCondition of a Subscriber.
    assert!(bs.get_qos().entity_factory.autoenable_created_entities);
    assert!(bs.get_listener().is_none());
    assert_eq!(
        bs.get_status_condition().enabled_statuses(),
        zerodds_dcps::psm_constants::status::ANY
    );

    // Its readers.
    let participant_reader = bs
        .lookup_datareader::<DcpsParticipantBuiltinTopicData>(TOPIC_NAME_DCPS_PARTICIPANT)
        .expect("the DCPSParticipant reader");
    assert!(
        bs.lookup_datareader::<DcpsTopicBuiltinTopicData>(TOPIC_NAME_DCPS_TOPIC)
            .is_some()
    );
    bs.sinks()
        .push_participant(&DcpsParticipantBuiltinTopicData {
            key: zerodds_rtps::wire_types::Guid::from_bytes([9u8; 16]),
            user_data: Vec::new(),
        })
        .unwrap();
    let with_samples = bs
        .get_datareaders(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    assert_eq!(with_samples.len(), 1, "the reader that holds a sample");

    // The application does not delete a built-in reader.
    assert!(matches!(
        bs.delete_datareader(&participant_reader),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        bs.delete_contained_entities(),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    assert!(
        bs.lookup_datareader::<DcpsParticipantBuiltinTopicData>(TOPIC_NAME_DCPS_PARTICIPANT)
            .is_some()
    );
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
#[allow(dead_code)]
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
        let domain = super::common::unique_domain();
        let cfg = super::common::isolated_cfg();
        let p_a = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("participant A");
        let _p_b = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("participant B");

        let bs_a = p_a.get_builtin_subscriber();
        let r = bs_a
            .lookup_datareader::<DcpsParticipantBuiltinTopicData>(TOPIC_NAME_DCPS_PARTICIPANT)
            .unwrap();

        // Wait until A has processed B's beacon.
        let ok = wait_until(Duration::from_secs(8), || !r.read().unwrap().is_empty());
        // No return without a check: a host without SPDP multicast fails here.
        assert!(ok, "no SPDP beacon of participant B reached participant A");
        let samples = r.take().unwrap();
        assert!(
            !samples.is_empty(),
            "DCPSParticipant reader must return a sample"
        );
    }

    #[test]
    fn sedp_pub_sub_populates_dcps_publication_and_subscription_readers() {
        let factory = DomainParticipantFactory::instance();
        let domain = super::common::unique_domain();
        let cfg = super::common::isolated_cfg();
        let p_a = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("participant A");
        let p_b = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
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

        // No return without a check: a host without SPDP/SEDP multicast fails here.
        assert!(
            ok_pub && ok_sub,
            "discovery did not reach the built-in readers (publication: {ok_pub}, subscription: {ok_sub})"
        );
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

    /// DDSI-RTPS 2.5 §8.5.4.2: a topic B creates reaches A's `DCPSTopic`
    /// reader over the SEDP topics endpoints, with the members only that
    /// announcement carries — HISTORY, RESOURCE_LIMITS and
    /// TRANSPORT_PRIORITY are in no endpoint announcement, and B creates
    /// no endpoint. B's `delete_contained_entities` disposes the instance.
    #[test]
    fn a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it() {
        use zerodds_dcps::{DdsType, InstanceStateKind};
        use zerodds_qos::{
            HistoryKind, HistoryQosPolicy, ResourceLimitsQosPolicy, TransportPriorityQosPolicy,
        };
        let factory = DomainParticipantFactory::instance();
        let domain = super::common::unique_domain();
        let p_a = factory
            .create_participant(domain, DomainParticipantQos::default())
            .expect("participant A");
        let p_b = factory
            .create_participant(domain, DomainParticipantQos::default())
            .expect("participant B");

        let mut qos = TopicQos::default();
        qos.history = HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 7,
        };
        qos.resource_limits = ResourceLimitsQosPolicy {
            max_samples: 100,
            max_instances: 10,
            max_samples_per_instance: 10,
        };
        qos.transport_priority = TransportPriorityQosPolicy { value: 3 };
        let _topic = p_b.create_topic::<RawBytes>("AnnouncedTopic", qos).unwrap();

        let reader = p_a
            .get_builtin_subscriber()
            .lookup_datareader::<DcpsTopicBuiltinTopicData>(TOPIC_NAME_DCPS_TOPIC)
            .unwrap();
        let key = DcpsTopicBuiltinTopicData::synthesize_key(
            p_b.runtime().expect("runtime").guid_prefix,
            "AnnouncedTopic",
            RawBytes::TYPE_NAME,
        );
        let announced = || {
            reader
                .read_with_info()
                .unwrap()
                .into_iter()
                .find(|s| s.data.key == key && s.info.valid_data)
        };
        assert!(
            wait_until(Duration::from_secs(8), || announced().is_some()),
            "B's topic announcement never reached A's DCPSTopic reader"
        );
        let sample = announced().unwrap().data;
        assert_eq!(sample.name, "AnnouncedTopic");
        assert_eq!(sample.history.depth, 7);
        assert_eq!(sample.resource_limits.max_samples, 100);
        assert_eq!(sample.resource_limits.max_instances, 10);
        assert_eq!(sample.transport_priority.value, 3);

        p_b.delete_contained_entities().unwrap();
        assert!(
            wait_until(Duration::from_secs(8), || {
                reader.read_with_info().unwrap().into_iter().any(|s| {
                    s.data.key == key
                        && s.info.instance_state == InstanceStateKind::NotAliveDisposed
                })
            }),
            "B's topic deletion never disposed A's DCPSTopic instance"
        );
    }

    /// DDS 1.4 §2.2.2.2.1.6: `delete_topic` on a topic no DataWriter,
    /// DataReader, ContentFilteredTopic or MultiTopic uses disposes the
    /// announcement on the SEDP topics writer (DDSI-RTPS 2.5 Table 8.84)
    /// exactly as `delete_contained_entities` does — while a live writer
    /// holds the topic, the delete is refused and the announcement stands.
    #[test]
    fn delete_topic_disposes_the_announced_topic_and_a_writer_holds_it() {
        use zerodds_dcps::{DdsType, InstanceStateKind};
        let factory = DomainParticipantFactory::instance();
        let domain = super::common::unique_domain();
        let p_a = factory
            .create_participant(domain, DomainParticipantQos::default())
            .expect("participant A");
        let p_b = factory
            .create_participant(domain, DomainParticipantQos::default())
            .expect("participant B");

        let topic = p_b
            .create_topic::<RawBytes>("DeleteTopicSEDP", TopicQos::default())
            .unwrap();
        let reader = p_a
            .get_builtin_subscriber()
            .lookup_datareader::<DcpsTopicBuiltinTopicData>(TOPIC_NAME_DCPS_TOPIC)
            .unwrap();
        let key = DcpsTopicBuiltinTopicData::synthesize_key(
            p_b.runtime().expect("runtime").guid_prefix,
            "DeleteTopicSEDP",
            RawBytes::TYPE_NAME,
        );
        let announced = || {
            reader
                .read_with_info()
                .unwrap()
                .into_iter()
                .find(|s| s.data.key == key && s.info.valid_data)
        };
        assert!(
            wait_until(Duration::from_secs(8), || announced().is_some()),
            "B's topic announcement never reached A's DCPSTopic reader"
        );

        // A DataWriter holds the topic: the delete is refused with
        // PRECONDITION_NOT_MET and has no effect (DDS 1.4 §2.2.2.2.1.6).
        let publisher = p_b.create_publisher(PublisherQos::default());
        let writer = publisher
            .create_datawriter(&topic, DataWriterQos::default())
            .unwrap();
        assert!(p_b.delete_topic(&topic).is_err());
        assert!(p_b.lookup_topicdescription("DeleteTopicSEDP").is_some());

        drop(writer);
        p_b.delete_topic(&topic).expect("no writer left");
        assert!(p_b.lookup_topicdescription("DeleteTopicSEDP").is_none());
        assert!(
            wait_until(Duration::from_secs(8), || {
                reader.read_with_info().unwrap().into_iter().any(|s| {
                    s.data.key == key
                        && s.info.instance_state == InstanceStateKind::NotAliveDisposed
                })
            }),
            "B's delete_topic never disposed A's DCPSTopic instance"
        );
    }

    /// DDS 1.4 §2.2.5: the `DCPSTopic` entry is "created when a Topic object
    /// is created", and its key distinguishes entries. B and C create the
    /// topic of the same name and type: A reads two entries, and C's
    /// deletion disposes C's entry, not B's.
    #[test]
    fn two_participants_creating_one_topic_are_two_dcps_topic_entries() {
        use zerodds_dcps::{DdsType, InstanceStateKind};
        let factory = DomainParticipantFactory::instance();
        let domain = super::common::unique_domain();
        let p_a = factory
            .create_participant(domain, DomainParticipantQos::default())
            .expect("participant A");
        let p_b = factory
            .create_participant(domain, DomainParticipantQos::default())
            .expect("participant B");
        let p_c = factory
            .create_participant(domain, DomainParticipantQos::default())
            .expect("participant C");
        let _topic_b = p_b
            .create_topic::<RawBytes>("SameTopicTwice", TopicQos::default())
            .unwrap();
        let _topic_c = p_c
            .create_topic::<RawBytes>("SameTopicTwice", TopicQos::default())
            .unwrap();
        let key_of = |p: &zerodds_dcps::DomainParticipant| {
            DcpsTopicBuiltinTopicData::synthesize_key(
                p.runtime().expect("runtime").guid_prefix,
                "SameTopicTwice",
                RawBytes::TYPE_NAME,
            )
        };
        let (key_b, key_c) = (key_of(&p_b), key_of(&p_c));
        assert_ne!(key_b, key_c, "two participants, two keys");

        let reader = p_a
            .get_builtin_subscriber()
            .lookup_datareader::<DcpsTopicBuiltinTopicData>(TOPIC_NAME_DCPS_TOPIC)
            .unwrap();
        let state_of = |key| {
            reader
                .read_with_info()
                .unwrap()
                .into_iter()
                .find(|s| s.data.key == key && s.info.valid_data)
                .map(|s| s.info.instance_state)
        };
        assert!(
            wait_until(Duration::from_secs(8), || {
                state_of(key_b).is_some() && state_of(key_c).is_some()
            }),
            "A did not read the entries of both B and C"
        );
        let entries = reader
            .read_with_info()
            .unwrap()
            .into_iter()
            .filter(|s| s.data.name == "SameTopicTwice" && s.info.valid_data)
            .count();
        assert_eq!(entries, 2, "one entry per participant");

        p_c.delete_contained_entities().unwrap();
        assert!(
            wait_until(Duration::from_secs(8), || {
                reader.read_with_info().unwrap().into_iter().any(|s| {
                    s.data.key == key_c
                        && s.info.instance_state == InstanceStateKind::NotAliveDisposed
                })
            }),
            "C's topic deletion never disposed C's entry"
        );
        assert_eq!(
            state_of(key_b),
            Some(InstanceStateKind::Alive),
            "B's entry stays"
        );
    }
}

/// Leases a DDS domain no other process on this host holds — the
/// domain number is the UDP port number, so it is a host-wide resource.
#[path = "common/mod.rs"]
mod common;

/// DDS 1.4 §2.2.5: the QoS of the built-in Subscriber and of every built-in
/// DataReader, policy by policy, as the table lists them.
#[test]
fn builtin_entities_have_the_qos_of_the_table() {
    use zerodds_qos::{
        DestinationOrderKind, DurabilityKind, HistoryKind, LivelinessKind, OwnershipKind,
        PresentationAccessScope, ReliabilityKind,
    };
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(36, DomainParticipantQos::default());
    let bs = p.get_builtin_subscriber();

    let sub = zerodds_dcps::Entity::get_qos(bs.subscriber());
    assert_eq!(
        sub.presentation.access_scope,
        PresentationAccessScope::Topic
    );
    assert!(!sub.presentation.coherent_access);
    assert!(!sub.presentation.ordered_access);
    assert!(sub.entity_factory.autoenable_created_entities);

    let readers = [
        bs.participant_reader().qos(),
        bs.topic_reader().qos(),
        bs.publication_reader().qos(),
        bs.subscription_reader().qos(),
    ];
    for qos in readers {
        assert_eq!(qos.durability.kind, DurabilityKind::TransientLocal);
        assert_eq!(qos.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(
            qos.reliability.max_blocking_time,
            zerodds_qos::Duration::from_millis(100)
        );
        assert_eq!(qos.history.kind, HistoryKind::KeepLast);
        assert_eq!(qos.history.depth, 1);
        assert!(qos.deadline.period.is_infinite());
        assert_eq!(qos.ownership.kind, OwnershipKind::Shared);
        assert_eq!(qos.liveliness.kind, LivelinessKind::Automatic);
        assert_eq!(
            qos.time_based_filter.minimum_separation,
            zerodds_qos::Duration::ZERO
        );
        assert_eq!(
            qos.destination_order.kind,
            DestinationOrderKind::ByReceptionTimestamp
        );
        assert_eq!(
            qos.resource_limits.max_samples,
            zerodds_dcps::psm_constants::LENGTH_UNLIMITED
        );
        assert_eq!(
            qos.resource_limits.max_instances,
            zerodds_dcps::psm_constants::LENGTH_UNLIMITED
        );
        assert_eq!(
            qos.resource_limits.max_samples_per_instance,
            zerodds_dcps::psm_constants::LENGTH_UNLIMITED
        );
        assert!(
            qos.reader_data_lifecycle
                .autopurge_nowriter_samples_delay
                .is_infinite()
        );
        assert!(
            qos.reader_data_lifecycle
                .autopurge_disposed_samples_delay
                .is_infinite()
        );
    }
}
