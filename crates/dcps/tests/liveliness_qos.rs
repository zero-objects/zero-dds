//! Liveliness QoS tests (Automatic kind, reader side).
//!
//! Spec OMG DDS 1.4 §2.2.3.11 + §2.2.4.2.14:
//! * A writer with `LivelinessKind::Automatic` counts as "alive" as long
//!   as it regularly sends samples within its `lease_duration`.
//! * The reader side keeps counters for alive_count (revivals)
//!   and not_alive_count (lease expirations).
//!
//! WP-3.4a (this commit): Automatic kind + lease monitoring on the
//! reader. WP-3.4b: SEDP compatibility check + Manual-kind
//! explicit assert messages.

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

use std::thread;
use std::time::Duration;

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::{
    DataReaderQos, DomainParticipantFactory, DomainParticipantQos, SubscriberQos, TopicQos,
};

#[serial_test::file_serial(dcps)]
#[test]
fn liveliness_default_infinite_keeps_counters_zero() {
    // Default lease = INFINITE. The counter must never rise, even if
    // no sample ever arrives. Otherwise the API shape breaks.
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(
            common::unique_domain(),
            DomainParticipantQos::default(),
            common::isolated_cfg(),
        )
        .expect("participant");
    let topic = p
        .create_topic::<ShapeType>("Square", TopicQos::default())
        .expect("topic");
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<ShapeType>(&topic, DataReaderQos::default())
        .expect("reader");

    thread::sleep(Duration::from_millis(300));
    let (_alive, alive_c, not_alive_c) = reader.liveliness_changed_status();
    assert_eq!(alive_c, 0, "INFINITE lease must NEVER change alive_count");
    assert_eq!(
        not_alive_c, 0,
        "INFINITE lease must NEVER change not_alive_count"
    );
}

#[path = "common/mod.rs"]
mod common;

#[cfg(target_os = "linux")]
mod linux {
    use super::common::unique_domain;
    use super::*;
    use zerodds_dcps::{DataWriterQos, PublisherQos};
    use zerodds_qos::Duration as QosDuration;
    use zerodds_qos::LivelinessQosPolicy;

    #[serial_test::file_serial(dcps)]
    #[test]
    fn reader_with_short_lease_marks_writer_not_alive_when_silent() {
        // Reader with a 150ms lease, no writer sends samples →
        // not_alive_count should rise to >=1 within 1s.
        // We need a matched writer (otherwise the
        // last_sample_received clock does not start); the writer sends **one**
        // sample, then goes silent.
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain();
        let cfg = super::common::isolated_cfg();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        // LIVELINESS is RxO: the offered lease must not exceed the requested
        // one (DDS 1.4 §2.2.3.11), and only MANUAL_BY_TOPIC makes a silent
        // writer not alive — AUTOMATIC is kept alive by its participant.
        let wqos = DataWriterQos {
            liveliness: LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByTopic,
                lease_duration: QosDuration::from_millis(100),
            },
            ..Default::default()
        };
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, wqos)
            .expect("writer");

        let sub_topic = sub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("sub topic");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let rqos = DataReaderQos {
            liveliness: LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::Automatic,
                lease_duration: QosDuration::from_millis(150),
            },
            ..Default::default()
        };
        let reader = subscriber
            .create_datareader::<ShapeType>(&sub_topic, rqos)
            .expect("reader");

        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("match");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("match");

        // One sample, then 1s of silence.
        writer
            .write(&ShapeType::new("RED", 1, 2, 30))
            .expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(1));
        let _ = reader.take().expect("take");

        thread::sleep(Duration::from_millis(1_000));

        let (alive, _alive_c, not_alive_c) = reader.liveliness_changed_status();
        assert!(
            not_alive_c >= 1,
            "writer stays silent 1s with a 150ms lease → not_alive_count must be >=1, got {not_alive_c}"
        );
        assert!(
            !alive,
            "current state should be not-alive, got alive={alive}"
        );
    }

    #[serial_test::file_serial(dcps)]
    #[test]
    fn reader_sees_writer_alive_again_after_resumed_publishing() {
        // Writer → silence → writer active again. alive_count should
        // count the not-alive → alive transition once.
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain();
        let cfg = super::common::isolated_cfg();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        // LIVELINESS is RxO: the offered lease must not exceed the requested
        // one (DDS 1.4 §2.2.3.11), and only MANUAL_BY_TOPIC makes a silent
        // writer not alive — AUTOMATIC is kept alive by its participant.
        let wqos = DataWriterQos {
            liveliness: LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByTopic,
                lease_duration: QosDuration::from_millis(100),
            },
            ..Default::default()
        };
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, wqos)
            .expect("writer");

        let sub_topic = sub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("sub topic");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let rqos = DataReaderQos {
            liveliness: LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::Automatic,
                lease_duration: QosDuration::from_millis(150),
            },
            ..Default::default()
        };
        let reader = subscriber
            .create_datareader::<ShapeType>(&sub_topic, rqos)
            .expect("reader");

        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("match");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("match");

        // Initial sample + wait (→ not_alive).
        writer
            .write(&ShapeType::new("RED", 1, 2, 30))
            .expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(1));
        let _ = reader.take().expect("take");
        thread::sleep(Duration::from_millis(500));

        let (alive_mid, _, not_alive_mid) = reader.liveliness_changed_status();
        assert!(
            !alive_mid && not_alive_mid >= 1,
            "Writer should be marked not-alive after silence"
        );

        // Writer becomes active again.
        writer
            .write(&ShapeType::new("RED", 3, 4, 30))
            .expect("write 2");
        let _ = reader.wait_for_data(Duration::from_secs(1));
        let _ = reader.take().expect("take");

        let (alive_end, alive_c_end, _not_alive_c_end) = reader.liveliness_changed_status();
        assert!(alive_end, "writer should be alive again, got {alive_end}");
        assert!(
            alive_c_end >= 1,
            "transition not_alive -> alive should give alive_count >=1, got {alive_c_end}"
        );
    }

    /// Two participants and a reader/writer pair on `topic`, matched: the
    /// writer offers `writer` LIVELINESS, the reader requests AUTOMATIC with
    /// `reader_lease_ms`.
    fn live_pair(
        topic: &str,
        writer: LivelinessQosPolicy,
        reader_lease_ms: i32,
    ) -> (
        zerodds_dcps::DomainParticipant,
        zerodds_dcps::DomainParticipant,
        zerodds_dcps::DataWriter<ShapeType>,
        zerodds_dcps::DataReader<ShapeType>,
    ) {
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain();
        let cfg = super::common::isolated_cfg();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");
        let pub_topic = pub_p
            .create_topic::<ShapeType>(topic, TopicQos::default())
            .expect("pub topic");
        let writer = pub_p
            .create_publisher(PublisherQos::default())
            .create_datawriter::<ShapeType>(
                &pub_topic,
                DataWriterQos {
                    liveliness: writer,
                    ..Default::default()
                },
            )
            .expect("writer");
        let sub_topic = sub_p
            .create_topic::<ShapeType>(topic, TopicQos::default())
            .expect("sub topic");
        let reader = sub_p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<ShapeType>(
                &sub_topic,
                DataReaderQos {
                    liveliness: LivelinessQosPolicy {
                        kind: zerodds_qos::LivelinessKind::Automatic,
                        lease_duration: QosDuration::from_millis(reader_lease_ms),
                    },
                    ..Default::default()
                },
            )
            .expect("reader");
        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("match");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("match");
        (pub_p, sub_p, writer, reader)
    }

    /// An AUTOMATIC writer is asserted by its participant (DDS 1.4
    /// §2.2.3.11), not by its writes: one that never writes stays alive at
    /// a remote reader past many of its 200 ms leases. The WLP beat used to
    /// follow the participant's lease (100 s / 3), and the reader measured
    /// the writer by the samples it received.
    #[serial_test::file_serial(dcps)]
    #[test]
    fn an_automatic_writer_that_never_writes_stays_alive_at_a_remote_reader() {
        let (_pub_p, _sub_p, writer, reader) = live_pair(
            "LiveAuto",
            LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::Automatic,
                lease_duration: QosDuration::from_millis(200),
            },
            300,
        );
        thread::sleep(Duration::from_millis(1_200));
        assert_eq!(reader.liveliness_changed_status(), (true, 1, 0));
        // Deleted normally, the writer leaves the count (§2.2.4.1).
        drop(writer);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while reader.liveliness_changed_status() != (false, 0, 0)
            && std::time::Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(reader.liveliness_changed_status(), (false, 0, 0));
    }

    /// `get_liveliness_changed_status` hands out the changes since the last
    /// read and resets them (DDS 1.4 §2.2.4.1).
    #[serial_test::file_serial(dcps)]
    #[test]
    fn get_liveliness_changed_status_reports_and_resets_the_changes() {
        let (_pub_p, _sub_p, _writer, reader) = live_pair(
            "LiveGetStatus",
            LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::Automatic,
                lease_duration: QosDuration::from_millis(200),
            },
            300,
        );
        let st = reader.get_liveliness_changed_status().unwrap();
        assert_eq!((st.alive_count, st.not_alive_count), (1, 0));
        assert_eq!((st.alive_count_change, st.not_alive_count_change), (1, 0));
        assert!(!st.last_publication_handle.is_nil());
        let st = reader.get_liveliness_changed_status().unwrap();
        assert_eq!((st.alive_count, st.not_alive_count), (1, 0));
        assert_eq!((st.alive_count_change, st.not_alive_count_change), (0, 0));
    }

    /// `DataWriter::assert_liveliness` reaches a remote reader as a
    /// HEARTBEAT with the liveliness flag (DDSI-RTPS 2.5 §8.3.7.5): a
    /// MANUAL_BY_TOPIC writer that asserts and never writes is alive there.
    /// The call used to set a local timestamp and send nothing.
    #[serial_test::file_serial(dcps)]
    #[test]
    fn assert_liveliness_keeps_a_manual_writer_alive_at_a_remote_reader() {
        let (_pub_p, _sub_p, writer, reader) = live_pair(
            "LiveAssertWire",
            LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByTopic,
                lease_duration: QosDuration::from_millis(150),
            },
            200,
        );
        for _ in 0..20 {
            writer.assert_liveliness();
            thread::sleep(Duration::from_millis(40));
        }
        assert_eq!(reader.liveliness_changed_status(), (true, 1, 0));
        assert_eq!(writer.liveliness_lost_count(), 0);
        // It stops asserting: its lease runs out at the reader, once.
        thread::sleep(Duration::from_millis(600));
        assert_eq!(reader.liveliness_changed_status(), (false, 0, 1));
        assert_eq!(writer.liveliness_lost_count(), 1);
    }

    /// DDS 1.4 §2.2.2.5.1.3: an instance is NOT_ALIVE_NO_WRITERS once "there
    /// are no live DataWriter entities writing that instance"; §2.2.4.2.2
    /// names "the loss of liveliness of the DataWriter of an instance for
    /// which there is no other DataWriter". The writer's lease runs out, the
    /// reader reports NOT_ALIVE_NO_WRITERS for its instance; the writer
    /// writes again, the instance is ALIVE with no_writers_generation_count 1.
    #[serial_test::file_serial(dcps)]
    #[test]
    fn an_instance_whose_writer_lost_its_liveliness_has_no_writers() {
        use zerodds_dcps::sample_info::InstanceStateKind;
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain();
        let cfg = super::common::isolated_cfg();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");
        let pub_topic = pub_p
            .create_topic::<ShapeType>("LostSquare", TopicQos::default())
            .expect("pub topic");
        let wqos = DataWriterQos {
            liveliness: LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByTopic,
                lease_duration: QosDuration::from_millis(200),
            },
            ..Default::default()
        };
        let writer = pub_p
            .create_publisher(PublisherQos::default())
            .create_datawriter::<ShapeType>(&pub_topic, wqos)
            .expect("writer");
        let sub_topic = sub_p
            .create_topic::<ShapeType>("LostSquare", TopicQos::default())
            .expect("sub topic");
        let reader = sub_p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<ShapeType>(&sub_topic, DataReaderQos::default())
            .expect("reader");
        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("match");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("match");

        writer
            .write(&ShapeType::new("TEAL", 1, 2, 30))
            .expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(2));
        assert!(
            reader
                .take_with_info()
                .expect("take")
                .iter()
                .any(|s| s.info.valid_data)
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut marker = None;
        while marker.is_none() && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
            marker = reader
                .take_with_info()
                .expect("take")
                .into_iter()
                .find(|s| {
                    !s.info.valid_data
                        && s.info.instance_state == InstanceStateKind::NotAliveNoWriters
                });
        }
        let marker = marker.expect("NOT_ALIVE_NO_WRITERS once the lease ran out");
        assert_eq!(marker.data.color, "TEAL");

        writer
            .write(&ShapeType::new("TEAL", 3, 4, 30))
            .expect("write again");
        let _ = reader.wait_for_data(Duration::from_secs(2));
        let again = reader.take_with_info().expect("take");
        let sample = again
            .iter()
            .find(|s| s.info.valid_data)
            .expect("the sample written again");
        assert_eq!(sample.info.instance_state, InstanceStateKind::Alive);
        assert_eq!(sample.info.no_writers_generation_count, 1);
    }

    /// DDS 1.4 §2.2.4.1 LivelinessChangedStatus: not_alive_count "increases
    /// when a DataWriter considered alive fails to assert its liveliness
    /// and becomes not alive for some reason other than the normal deletion
    /// of that DataWriter". The writer's participant ends without notice
    /// and its lease runs out at the reader: the writer is unmatched and
    /// moves from alive_count to not_alive_count. It used to leave both, as
    /// a deleted writer does.
    #[serial_test::file_serial(dcps)]
    #[test]
    fn a_writer_lost_with_its_participant_counts_as_not_alive() {
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain();
        let sub_cfg = super::common::isolated_cfg();
        let pub_cfg = zerodds_dcps::runtime::RuntimeConfig {
            participant_lease_duration: Duration::from_millis(800),
            ..sub_cfg.clone()
        };
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), pub_cfg)
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), sub_cfg)
            .expect("sub participant");
        let pub_topic = pub_p
            .create_topic::<ShapeType>("LostWithParticipant", TopicQos::default())
            .expect("pub topic");
        let writer = pub_p
            .create_publisher(PublisherQos::default())
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");
        let sub_topic = sub_p
            .create_topic::<ShapeType>("LostWithParticipant", TopicQos::default())
            .expect("sub topic");
        let reader = sub_p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<ShapeType>(&sub_topic, DataReaderQos::default())
            .expect("reader");
        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("match");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("match");
        let st = reader.get_liveliness_changed_status().unwrap();
        assert_eq!((st.alive_count, st.not_alive_count), (1, 0));
        let publication = reader.matched_publications().unwrap();
        assert_eq!(publication.len(), 1);

        pub_p.runtime().expect("runtime").__vanish();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while reader.matched_publication_count() > 0 && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(
            reader.matched_publication_count(),
            0,
            "the participant's lease did not run out"
        );
        let st = reader.get_liveliness_changed_status().unwrap();
        assert_eq!((st.alive_count, st.not_alive_count), (0, 1));
        assert_eq!((st.alive_count_change, st.not_alive_count_change), (-1, 1));
        assert_eq!(
            st.last_publication_handle, publication[0],
            "the lost writer"
        );
    }

    /// DDS 1.4 §2.2.4.2.2: DATA_AVAILABLE changes with "the loss of
    /// liveliness of the DataWriter of an instance for which there is no
    /// other DataWriter". The writer's lease runs out while nothing reads:
    /// `on_data_available` is called — which lowers DATA_AVAILABLE again —,
    /// a ReadCondition for NOT_READ NOT_ALIVE_NO_WRITERS samples triggers,
    /// and the take that follows hands out the marker.
    /// They stayed down until a read or take found the marker.
    #[serial_test::file_serial(dcps)]
    #[test]
    fn a_lost_writer_raises_data_available_before_any_read() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use zerodds_dcps::Entity;
        use zerodds_dcps::condition::{Condition, WaitSet};
        use zerodds_dcps::psm_constants::status::DATA_AVAILABLE;
        use zerodds_dcps::sample_info::{
            InstanceStateKind, instance_state_mask, sample_state_mask, view_state_mask,
        };

        struct Count(Arc<AtomicUsize>);
        impl zerodds_dcps::listener::DataReaderListener for Count {
            fn on_data_available(&self, _reader: zerodds_dcps::InstanceHandle) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain();
        let cfg = super::common::isolated_cfg();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");
        let pub_topic = pub_p
            .create_topic::<ShapeType>("LostAvailable", TopicQos::default())
            .expect("pub topic");
        let wqos = DataWriterQos {
            liveliness: LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByTopic,
                lease_duration: QosDuration::from_millis(200),
            },
            ..Default::default()
        };
        let writer = pub_p
            .create_publisher(PublisherQos::default())
            .create_datawriter::<ShapeType>(&pub_topic, wqos)
            .expect("writer");
        let sub_topic = sub_p
            .create_topic::<ShapeType>("LostAvailable", TopicQos::default())
            .expect("sub topic");
        let reader = sub_p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<ShapeType>(&sub_topic, DataReaderQos::default())
            .expect("reader");
        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("match");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("match");

        writer
            .write(&ShapeType::new("OLIVE", 1, 2, 30))
            .expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(2));
        assert!(
            reader
                .take_with_info()
                .expect("take")
                .iter()
                .any(|s| s.info.valid_data)
        );

        let calls = Arc::new(AtomicUsize::new(0));
        reader.set_listener(Some(Arc::new(Count(Arc::clone(&calls)))), DATA_AVAILABLE);
        let status = reader.get_status_condition();
        status.set_enabled_statuses(DATA_AVAILABLE);
        assert!(!status.trigger_value(), "DATA_AVAILABLE up after the take");
        let no_writers = reader
            .create_readcondition(
                sample_state_mask::NOT_READ,
                view_state_mask::ANY,
                instance_state_mask::NOT_ALIVE_NO_WRITERS,
            )
            .expect("read condition");
        assert!(!no_writers.get_trigger_value());
        let waitset = WaitSet::new();
        waitset
            .attach_condition(Arc::clone(&no_writers) as Arc<dyn Condition>)
            .expect("attach");

        // Nothing reads until the WaitSet wakes.
        let woken = waitset
            .wait(Duration::from_secs(3))
            .expect("the lost writer's marker woke the WaitSet");
        assert_eq!(woken.len(), 1);
        // DATA_AVAILABLE came up for the marker: the listener heard of it.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while calls.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "on_data_available calls");
        // DDS 1.4 §2.2.4.2.2: DATA_AVAILABLE "becomes FALSE when ... the
        // corresponding listener operation (on_data_available) is called".
        // The listener runs on the thread that delivered the marker, before
        // the WaitSet wakes; the test looked for the flag up after it.
        assert!(
            !status.trigger_value(),
            "on_data_available lowered DATA_AVAILABLE"
        );

        reader.set_listener(None, 0);
        let taken = reader.take_with_info().expect("take");
        let marker = taken
            .iter()
            .find(|s| !s.info.valid_data)
            .expect("the NOT_ALIVE_NO_WRITERS marker");
        assert_eq!(
            marker.info.instance_state,
            InstanceStateKind::NotAliveNoWriters
        );
        assert_eq!(marker.data.color, "OLIVE");
        assert!(!status.trigger_value(), "DATA_AVAILABLE up after the take");
        assert!(!no_writers.get_trigger_value());
    }
}
