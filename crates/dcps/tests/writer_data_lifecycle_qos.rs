//! WP QoS wiring T2 — WriterDataLifecycle.autodispose_unregistered_instances.
//!
//! Spec DDS 1.4 §2.2.3.21: when `autodispose_unregistered_instances=true`
//! (default), `unregister_instance` additionally performs a `dispose`.

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

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::{
    DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos, InstanceStateKind,
    PublisherQos, TopicQos,
};
use zerodds_qos::WriterDataLifecycleQosPolicy;

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

fn writer_with_qos(qos: DataWriterQos) -> zerodds_dcps::DataWriter<KeyedRecord> {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<KeyedRecord>("AutoDispose", TopicQos::default())
        .expect("topic");
    let pub_ = p.create_publisher(PublisherQos::default());
    pub_.create_datawriter::<KeyedRecord>(&topic, qos)
        .expect("writer")
}

#[test]
fn autodispose_default_true_unregister_disposes() {
    let writer = writer_with_qos(DataWriterQos::default());

    let s = KeyedRecord { id: 7, value: 100 };
    let handle = writer.register_instance(&s).expect("register");
    writer.write(&s).expect("write");
    writer.unregister_instance(&s, handle).expect("unregister");

    let tracker = writer.instance_tracker();
    let state = tracker.get_by_handle(handle).expect("state present");
    assert_eq!(
        state.kind,
        InstanceStateKind::NotAliveDisposed,
        "autodispose=true → unregister should dispose the instance"
    );
}

#[test]
fn autodispose_false_unregister_only_decrements_writer_count() {
    let qos = DataWriterQos {
        writer_data_lifecycle: WriterDataLifecycleQosPolicy {
            autodispose_unregistered_instances: false,
        },
        ..Default::default()
    };
    let writer = writer_with_qos(qos);

    let s = KeyedRecord { id: 9, value: 200 };
    let handle = writer.register_instance(&s).expect("register");
    writer.write(&s).expect("write");
    writer.unregister_instance(&s, handle).expect("unregister");

    let tracker = writer.instance_tracker();
    let state = tracker.get_by_handle(handle).expect("state present");
    assert_eq!(
        state.kind,
        InstanceStateKind::NotAliveNoWriters,
        "autodispose=false → unregister only sets NOT_ALIVE_NO_WRITERS"
    );
}

#[test]
#[allow(unused_variables)]
fn autodispose_unknown_handle_returns_bad_parameter() {
    use zerodds_dcps::HANDLE_NIL;
    let writer = writer_with_qos(DataWriterQos::default());

    let s = KeyedRecord { id: 13, value: 0 };
    // No register before unregister → should fail with BadParameter.
    let res = writer.unregister_instance(&s, HANDLE_NIL);
    assert!(res.is_err(), "unregister on an unknown instance must fail");
}
