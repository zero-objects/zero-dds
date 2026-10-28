//! WP QoS-Wiring T8c — end-to-end test: Writer.dispose over the wire,
//! reader sees NotAliveDisposed.
//!
//! Spec DDS 1.4 §2.2.2.4.2.10/.7 + §9.6.3.9: dispose/unregister
//! send a DATA with `key_flag=true` + PID_STATUS_INFO. The
//! remote reader classifies it as a lifecycle marker and sets
//! its tracker's instance state accordingly.

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

#[path = "common/mod.rs"]
mod common;

#[cfg(target_os = "linux")]
mod linux {
    use std::time::Duration;

    use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
    use zerodds_dcps::runtime::RuntimeConfig;
    use zerodds_dcps::sample_info::InstanceStateKind;
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
        PublisherQos, SubscriberQos, TopicQos,
    };
    use zerodds_qos::WriterDataLifecycleQosPolicy;

    use super::common::unique_domain;

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    struct KeyedRecord {
        id: u32,
        value: u32,
    }

    impl DdsType for KeyedRecord {
        const TYPE_NAME: &'static str = "test::KeyedRecord";
        /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
        const XCDR1_MATCHES_XCDR2: bool = true;
        const HAS_KEY: bool = true;
        const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

        fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
            out.extend_from_slice(&self.id.to_be_bytes());
            out.extend_from_slice(&self.value.to_be_bytes());
            Ok(())
        }

        fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
            if bytes.len() < 4 {
                return Err(DecodeError::Invalid {
                    what: "truncated KeyedRecord",
                });
            }
            let id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let value = if bytes.len() >= 8 {
                u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]])
            } else {
                0
            };
            Ok(Self { id, value })
        }

        fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
            holder.write_u32(self.id);
        }

        /// The inverse of the holder above. DDS 1.4 §2.2.2.5.1.13 declares the
        /// key fields valid on a dispose/unregister sample; the holder is a
        /// PLAIN_CDR2-BE `uint32` and nothing else, so it is read as those four
        /// octets rather than through `Self::decode`, which reads the type's
        /// own stream.
        fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
            let head: [u8; 4] =
                bytes
                    .get(..4)
                    .and_then(|b| b.try_into().ok())
                    .ok_or(DecodeError::Invalid {
                        what: "truncated KeyedRecord key holder",
                    })?;
            Ok(Self {
                id: u32::from_be_bytes(head),
                ..Self::default()
            })
        }
    }

    fn fast_cfg() -> RuntimeConfig {
        RuntimeConfig {
            tick_period: Duration::from_millis(10),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        }
    }

    fn pair_with_writer_qos(
        domain: i32,
        topic: &str,
        wqos: DataWriterQos,
    ) -> (
        zerodds_dcps::DataWriter<KeyedRecord>,
        zerodds_dcps::DataReader<KeyedRecord>,
    ) {
        pair_with_reader_config(domain, topic, wqos, fast_cfg())
    }

    fn pair_with_reader_config(
        domain: i32,
        topic: &str,
        wqos: DataWriterQos,
        sub_cfg: RuntimeConfig,
    ) -> (
        zerodds_dcps::DataWriter<KeyedRecord>,
        zerodds_dcps::DataReader<KeyedRecord>,
    ) {
        let factory = DomainParticipantFactory::instance();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), sub_cfg)
            .expect("sub participant");
        let pub_topic = pub_p
            .create_topic::<KeyedRecord>(topic, TopicQos::default())
            .expect("pub topic");
        let sub_topic = sub_p
            .create_topic::<KeyedRecord>(topic, TopicQos::default())
            .expect("sub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        let writer = publisher
            .create_datawriter::<KeyedRecord>(&pub_topic, wqos)
            .expect("writer");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let reader = subscriber
            .create_datareader::<KeyedRecord>(&sub_topic, DataReaderQos::default())
            .expect("reader");

        writer
            .wait_for_matched_subscription(1, Duration::from_secs(15))
            .expect("writer match");
        reader
            .wait_for_matched_publication(1, Duration::from_secs(15))
            .expect("reader match");
        (writer, reader)
    }

    #[test]
    fn dispose_propagates_over_wire_to_reader() {
        // Default WriterQos => autodispose=true; dispose sends a wire marker.
        let (writer, reader) =
            pair_with_writer_qos(unique_domain(), "DisposeWire", DataWriterQos::default());

        let s = KeyedRecord { id: 1, value: 100 };
        let h = writer.register_instance(&s).expect("register");
        writer.write(&s).expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(2));
        let _ = reader.take_with_info().expect("take initial");

        // Dispose -> wire marker at the reader. A reliable writer => the marker IS
        // delivered; wait for the event (poll until deadline), do NOT assume a fixed
        // sleep duration — the latter breaks deterministically under slow
        // load/instrumentation (the marker arrives after >500 ms, and a single take misses
        // it). Event-driven instead of a timing assumption.
        writer.dispose(&s, h).expect("dispose");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut got = false;
        while !got && std::time::Instant::now() < deadline {
            let _ = reader.wait_for_data(Duration::from_millis(200));
            let samples = reader.take_with_info().expect("take after dispose");
            got = samples.iter().any(|s| {
                !s.info.valid_data && s.info.instance_state == InstanceStateKind::NotAliveDisposed
            });
        }
        assert!(
            got,
            "reader must receive the disposed marker via wire (reliable, within 10s)"
        );
    }

    #[test]
    fn unregister_with_autodispose_false_propagates_unregister_only() {
        let qos = DataWriterQos {
            writer_data_lifecycle: WriterDataLifecycleQosPolicy {
                autodispose_unregistered_instances: false,
            },
            ..Default::default()
        };
        let (writer, reader) = pair_with_writer_qos(unique_domain(), "UnregWire", qos);

        let s = KeyedRecord { id: 2, value: 200 };
        let h = writer.register_instance(&s).expect("register");
        writer.write(&s).expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(2));
        let _ = reader.take_with_info().expect("take initial");

        // Reliable => the NoWriters marker is delivered; wait for the event instead
        // of sleeping fixed (otherwise a timing break under instrumentation).
        writer.unregister_instance(&s, h).expect("unregister");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut got = false;
        while !got && std::time::Instant::now() < deadline {
            let _ = reader.wait_for_data(Duration::from_millis(200));
            let samples = reader.take_with_info().expect("take after unregister");
            // With autodispose=false we see NoWriters (no Disposed).
            got = samples
                .iter()
                .any(|s| s.info.instance_state == InstanceStateKind::NotAliveNoWriters);
        }
        assert!(
            got,
            "reader must see the NoWriters marker with autodispose=false (reliable, within 10s)"
        );
    }

    /// `reader.dispose_with_unregister`: an unregister under the default
    /// WRITER_DATA_LIFECYCLE sends one marker with the DisposedFlag and the
    /// UnregisteredFlag (DDSI-RTPS 2.5 §9.6.3.9). The reader takes it as a
    /// dispose by default and as an unregister with the setting off, so the
    /// instance its only writer unregistered is NOT_ALIVE_NO_WRITERS: the
    /// state OMG dds-rtps `Test_FinalInstanceState_2` expects of an OpenDDS
    /// publisher, which sends such a marker when its writer is deleted.
    #[test]
    fn a_dispose_unregister_marker_is_an_unregister_with_the_setting_off() {
        for (setting, expected) in [
            (true, InstanceStateKind::NotAliveDisposed),
            (false, InstanceStateKind::NotAliveNoWriters),
        ] {
            let sub_cfg = RuntimeConfig {
                reader_dispose_with_unregister: setting,
                ..fast_cfg()
            };
            let (writer, reader) = pair_with_reader_config(
                unique_domain(),
                "DisposeUnregisterWire",
                DataWriterQos::default(),
                sub_cfg,
            );

            let s = KeyedRecord { id: 3, value: 300 };
            let h = writer.register_instance(&s).expect("register");
            writer.write(&s).expect("write");
            let _ = reader.wait_for_data(Duration::from_secs(2));
            let _ = reader.take_with_info().expect("take initial");

            writer.unregister_instance(&s, h).expect("unregister");
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut state = None;
            while state.is_none() && std::time::Instant::now() < deadline {
                let _ = reader.wait_for_data(Duration::from_millis(200));
                state = reader
                    .take_with_info()
                    .expect("take after unregister")
                    .iter()
                    .find(|s| !s.info.valid_data)
                    .map(|s| s.info.instance_state);
            }
            assert_eq!(
                state,
                Some(expected),
                "reader.dispose_with_unregister={setting}"
            );
        }
    }

    /// DDS 1.4 §2.2.2.5.1.13: on a dispose/unregister marker
    /// `SampleInfo.valid_data` is `false` and "only the fields that form the
    /// key are meaningful" — so those fields must actually arrive. The marker
    /// carries no body, and the `InstanceTracker`'s KeyHolder is a
    /// PLAIN_CDR2-BE stream of the key members alone (no DHEADER, big-endian),
    /// which `ShapeType::decode` cannot read: feeding it there made every
    /// `take`/`read` of a marker fail with `WireError` instead of returning the
    /// instance. Uses `ShapeType` on purpose — its key is a string, so its
    /// KeyHolder and its XCDR2-LE body genuinely differ.
    #[test]
    fn dispose_marker_carries_the_key_fields_of_a_string_keyed_type() {
        use zerodds_dcps::interop::ShapeType;

        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("sub participant");
        let pub_topic = pub_p
            .create_topic::<ShapeType>("KeyFieldsSquare", TopicQos::default())
            .expect("pub topic");
        let sub_topic = sub_p
            .create_topic::<ShapeType>("KeyFieldsSquare", TopicQos::default())
            .expect("sub topic");
        let writer = pub_p
            .create_publisher(PublisherQos::default())
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");
        let reader = sub_p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<ShapeType>(&sub_topic, DataReaderQos::default())
            .expect("reader");
        writer
            .wait_for_matched_subscription(1, Duration::from_secs(15))
            .expect("writer match");
        reader
            .wait_for_matched_publication(1, Duration::from_secs(15))
            .expect("reader match");

        let s = ShapeType::new("MAGENTA", 10, 20, 30);
        let h = writer.register_instance(&s).expect("register");
        writer.write(&s).expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(2));
        let _ = reader.take_with_info().expect("take initial");

        writer.dispose(&s, h).expect("dispose");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut marker = None;
        while marker.is_none() && std::time::Instant::now() < deadline {
            let _ = reader.wait_for_data(Duration::from_millis(200));
            let samples = reader.take_with_info().expect("take after dispose");
            marker = samples.into_iter().find(|s| !s.info.valid_data);
        }
        let marker = marker.expect("dispose marker within 10s");
        assert_eq!(
            marker.info.instance_state,
            InstanceStateKind::NotAliveDisposed
        );
        assert_eq!(
            marker.data.color, "MAGENTA",
            "the key field must survive on an invalid-data sample"
        );
    }

    /// DDS 1.4 §2.2.2.5.1: `NOT_ALIVE_NO_WRITERS_INSTANCE_STATE` "indicates
    /// the instance has been declared as not-alive by the DataReader because
    /// it detected that there are no live DataWriter objects writing that
    /// instance". No lifecycle marker is sent here — the writer is simply gone
    /// — so the transition has to come from the discovery event alone. This is
    /// the OMG `dds-rtps` `Test_FinalInstanceState_2` scenario: a publisher
    /// that ends without unregistering or disposing anything. A deleted
    /// writer unregisters and disposes its instances (DDS 1.4 §2.2.2.4.1.6,
    /// §2.2.3.21); `writer.unregister_on_delete` off is the publisher that
    /// sends no marker, as Connext and Fast DDS do there.
    #[test]
    fn instance_goes_no_writers_when_its_last_writer_disappears() {
        use zerodds_dcps::interop::ShapeType;

        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain();
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("sub participant");
        let sub_topic = sub_p
            .create_topic::<ShapeType>("NoWritersSquare", TopicQos::default())
            .expect("sub topic");
        let reader = sub_p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<ShapeType>(&sub_topic, DataReaderQos::default())
            .expect("reader");

        {
            let no_markers = RuntimeConfig {
                writer_unregister_on_delete: false,
                ..fast_cfg()
            };
            let pub_p = factory
                .create_participant_with_config(domain, DomainParticipantQos::default(), no_markers)
                .expect("pub participant");
            let pub_topic = pub_p
                .create_topic::<ShapeType>("NoWritersSquare", TopicQos::default())
                .expect("pub topic");
            let writer = pub_p
                .create_publisher(PublisherQos::default())
                .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
                .expect("writer");
            writer
                .wait_for_matched_subscription(1, Duration::from_secs(15))
                .expect("writer match");
            reader
                .wait_for_matched_publication(1, Duration::from_secs(15))
                .expect("reader match");
            writer
                .write(&ShapeType::new("CYAN", 1, 2, 30))
                .expect("write");
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut seen = false;
            while !seen && std::time::Instant::now() < deadline {
                let _ = reader.wait_for_data(Duration::from_millis(200));
                seen = reader
                    .take_with_info()
                    .expect("take")
                    .iter()
                    .any(|s| s.info.valid_data);
            }
            assert!(
                seen,
                "the reader must see the sample before the writer leaves"
            );
            // Writer and its participant go away — no unregister, no dispose.
        }

        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut marker = None;
        while marker.is_none() && std::time::Instant::now() < deadline {
            let _ = reader.wait_for_data(Duration::from_millis(200));
            let samples = reader.take_with_info().expect("take after writer left");
            marker = samples.into_iter().find(|s| {
                !s.info.valid_data && s.info.instance_state == InstanceStateKind::NotAliveNoWriters
            });
        }
        let marker = marker.expect("NOT_ALIVE_NO_WRITERS within 15s after the writer left");
        assert_eq!(marker.data.color, "CYAN");
    }

    /// DDS 1.4 §2.2.2.4.1.6: "The deletion of the DataWriter will
    /// automatically unregister all instances", and with the default
    /// WRITER_DATA_LIFECYCLE it disposes them. `delete_datawriter` returns
    /// once the reliable reader has acknowledged those markers
    /// (`writer.linger_ms`), so the reader holds the dispose at that moment.
    /// Removed at once, the writer sent its SEDP dispose right behind the
    /// markers; Connext 7.7.0 then dropped the writer before it took them in
    /// some runs (OMG dds-rtps `Test_FinalInstanceState_1`).
    #[test]
    fn a_deleted_writer_goes_once_its_reader_has_the_dispose() {
        use zerodds_dcps::interop::ShapeType;
        use zerodds_dcps::qos::ReliabilityKind;

        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain();
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("sub participant");
        let sub_topic = sub_p
            .create_topic::<ShapeType>("LingerSquare", TopicQos::default())
            .expect("sub topic");
        let mut rq = DataReaderQos::default();
        rq.reliability.kind = ReliabilityKind::Reliable;
        let reader = sub_p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<ShapeType>(&sub_topic, rq)
            .expect("reader");

        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("pub participant");
        let pub_topic = pub_p
            .create_topic::<ShapeType>("LingerSquare", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");
        writer
            .wait_for_matched_subscription(1, Duration::from_secs(15))
            .expect("writer match");
        reader
            .wait_for_matched_publication(1, Duration::from_secs(15))
            .expect("reader match");
        writer
            .write(&ShapeType::new("ORANGE", 1, 2, 30))
            .expect("write");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut seen = false;
        while !seen && std::time::Instant::now() < deadline {
            let _ = reader.wait_for_data(Duration::from_millis(200));
            seen = reader
                .take_with_info()
                .expect("take")
                .iter()
                .any(|s| s.info.valid_data);
        }
        assert!(seen, "the reader must see the sample before the delete");

        publisher.delete_datawriter(&writer).expect("delete");
        let samples = reader.take_with_info().expect("take after delete");
        assert!(
            samples.iter().any(|s| !s.info.valid_data
                && s.info.instance_state == InstanceStateKind::NotAliveDisposed
                && s.data.color == "ORANGE"),
            "the dispose of the deleted writer is at the reader when \
             delete_datawriter returns, got {samples:?}"
        );
    }

    /// DDS 1.4 §2.2.4.2.2: DATA_AVAILABLE changes when an instance changes
    /// to NOT_ALIVE_NO_WRITERS, and §2.2.2.5.1.3 makes it so once no
    /// DataWriter writes the instance. The writer leaves without a marker
    /// while nothing reads: `on_data_available` is called — which lowers
    /// DATA_AVAILABLE again —, a ReadCondition for NOT_READ
    /// NOT_ALIVE_NO_WRITERS samples wakes a WaitSet, and the take after it
    /// hands out the marker. They stayed down until a read or take found the
    /// marker.
    #[test]
    fn a_writer_that_leaves_raises_data_available_before_any_read() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use zerodds_dcps::Entity;
        use zerodds_dcps::condition::{Condition, WaitSet};
        use zerodds_dcps::interop::ShapeType;
        use zerodds_dcps::psm_constants::status::DATA_AVAILABLE;
        use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};

        struct Count(Arc<AtomicUsize>);
        impl zerodds_dcps::listener::DataReaderListener for Count {
            fn on_data_available(&self, _reader: zerodds_dcps::InstanceHandle) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain();
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("sub participant");
        let sub_topic = sub_p
            .create_topic::<ShapeType>("LeavingSquare", TopicQos::default())
            .expect("sub topic");
        let reader = sub_p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<ShapeType>(&sub_topic, DataReaderQos::default())
            .expect("reader");
        let calls = Arc::new(AtomicUsize::new(0));
        let status = reader.get_status_condition();
        status.set_enabled_statuses(DATA_AVAILABLE);
        let no_writers = reader
            .create_readcondition(
                sample_state_mask::NOT_READ,
                view_state_mask::ANY,
                instance_state_mask::NOT_ALIVE_NO_WRITERS,
            )
            .expect("read condition");
        let waitset = WaitSet::new();
        waitset
            .attach_condition(Arc::clone(&no_writers) as Arc<dyn Condition>)
            .expect("attach");

        {
            let no_markers = RuntimeConfig {
                writer_unregister_on_delete: false,
                ..fast_cfg()
            };
            let pub_p = factory
                .create_participant_with_config(domain, DomainParticipantQos::default(), no_markers)
                .expect("pub participant");
            let pub_topic = pub_p
                .create_topic::<ShapeType>("LeavingSquare", TopicQos::default())
                .expect("pub topic");
            let writer = pub_p
                .create_publisher(PublisherQos::default())
                .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
                .expect("writer");
            writer
                .wait_for_matched_subscription(1, Duration::from_secs(15))
                .expect("writer match");
            reader
                .wait_for_matched_publication(1, Duration::from_secs(15))
                .expect("reader match");
            writer
                .write(&ShapeType::new("PURPLE", 1, 2, 30))
                .expect("write");
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut seen = false;
            while !seen && std::time::Instant::now() < deadline {
                let _ = reader.wait_for_data(Duration::from_millis(200));
                seen = reader
                    .take_with_info()
                    .expect("take")
                    .iter()
                    .any(|s| s.info.valid_data);
            }
            assert!(
                seen,
                "the reader must see the sample before the writer leaves"
            );
            assert!(!status.trigger_value(), "DATA_AVAILABLE up after the take");
            assert!(!no_writers.get_trigger_value());
            reader.set_listener(Some(Arc::new(Count(Arc::clone(&calls)))), DATA_AVAILABLE);
            // Writer and its participant go away — no unregister, no dispose.
        }

        // Nothing reads until the WaitSet wakes.
        let woken = waitset
            .wait(Duration::from_secs(15))
            .expect("the departed writer's marker woke the WaitSet");
        assert_eq!(woken.len(), 1);
        // DATA_AVAILABLE came up for the marker: the listener heard of it.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while calls.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
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
        assert_eq!(marker.data.color, "PURPLE");
        assert!(!status.trigger_value(), "DATA_AVAILABLE up after the take");
        assert!(!no_writers.get_trigger_value());
    }
}
