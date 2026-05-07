//! WP QoS-Wiring T8c — End-to-End-Test: Writer.dispose ueber Wire,
//! Reader sieht NotAliveDisposed.
//!
//! Spec DDS 1.4 §2.2.2.4.2.10/.7 + §9.6.3.9: dispose/unregister
//! schicken eine DATA mit `key_flag=true` + PID_STATUS_INFO. Der
//! Remote-Reader klassifiziert das als Lifecycle-Marker und setzt
//! den Instance-State seines Trackers entsprechend.

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
    use std::thread;
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
        let factory = DomainParticipantFactory::instance();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
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
        // Default-WriterQos => autodispose=true; dispose schickt Wire-Marker.
        let (writer, reader) =
            pair_with_writer_qos(unique_domain(85), "DisposeWire", DataWriterQos::default());

        let s = KeyedRecord { id: 1, value: 100 };
        let h = writer.register_instance(&s).expect("register");
        writer.write(&s).expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(2));
        let _ = reader.take_with_info().expect("take initial");

        // Dispose -> sollte als Wire-Marker beim Reader ankommen.
        writer.dispose(&s, h).expect("dispose");
        thread::sleep(Duration::from_millis(500));

        let samples = reader.take_with_info().expect("take after dispose");
        assert!(
            samples.iter().any(|s| {
                !s.info.valid_data && s.info.instance_state == InstanceStateKind::NotAliveDisposed
            }),
            "Reader muss disposed-marker via Wire empfangen, got {samples:?}"
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
        let (writer, reader) = pair_with_writer_qos(unique_domain(85), "UnregWire", qos);

        let s = KeyedRecord { id: 2, value: 200 };
        let h = writer.register_instance(&s).expect("register");
        writer.write(&s).expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(2));
        let _ = reader.take_with_info().expect("take initial");

        writer.unregister_instance(&s, h).expect("unregister");
        thread::sleep(Duration::from_millis(500));

        let samples = reader.take_with_info().expect("take after unregister");
        // Bei autodispose=false sehen wir NoWriters (kein Disposed).
        assert!(
            samples
                .iter()
                .any(|s| s.info.instance_state == InstanceStateKind::NotAliveNoWriters),
            "Reader muss NoWriters-Marker bei autodispose=false sehen, got {samples:?}"
        );
    }
}
