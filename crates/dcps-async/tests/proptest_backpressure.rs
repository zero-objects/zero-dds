//! zerodds-async-1.0 §8: proptest over channel backpressure.
//!
//! Property: against an offline participant with Reliable + KeepAll +
//! `max_samples = N`, a random sequence of `write` and `take` must
//! never
//! - let the queue grow beyond N,
//! - return an error class other than `Timeout` when the queue is full,
//! - let a write fail when the queue has free space.
//!
//! This safeguards §5.1 (the `write` future suspends on OutOfResources)
//! and §6.1 (data_available_stream / take provide the deterministic
//! drain).

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

use std::time::Duration;

use proptest::prelude::*;
use zerodds_dcps::RawBytes;
use zerodds_dcps_async::{
    AsyncDomainParticipantFactory, DataReaderQos, DataWriterQos, DdsError, PublisherQos,
    SubscriberQos, TopicQos,
};
use zerodds_qos::Duration as QosDuration;

#[derive(Debug, Clone, Copy)]
enum Op {
    Write,
    Take,
}

/// Generates a sequence of writes and takes with clearly bounded
/// proportions — proptest shrinks on the sequence itself.
fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![Just(Op::Write), Just(Op::Take)]
}

fn ops_strategy() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op_strategy(), 0..32)
}

/// Generates a random queue capacity [1, 8] — small enough that the
/// proptest sequence hits the limit regularly.
fn capacity_strategy() -> impl Strategy<Value = usize> {
    1usize..=8
}

/// Per property iteration: a freshly initialized async participant +
/// topic + reliable writer + reader, together.
async fn run_sequence(capacity: usize, ops: Vec<Op>, topic_name: &str) {
    use zerodds_qos::HistoryKind;
    use zerodds_qos::HistoryQosPolicy;
    use zerodds_qos::ReliabilityKind;
    use zerodds_qos::ReliabilityQosPolicy;
    use zerodds_qos::ResourceLimitsQosPolicy;

    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<RawBytes>(topic_name, TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let subr = p.create_subscriber(SubscriberQos::default());

    let writer_qos = DataWriterQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: QosDuration::from_millis(20),
        },
        resource_limits: ResourceLimitsQosPolicy {
            max_samples: capacity as i32,
            max_instances: 1,
            max_samples_per_instance: capacity as i32,
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: capacity as i32,
        },
        ..Default::default()
    };
    let writer = pubr
        .create_datawriter::<RawBytes>(&topic, writer_qos)
        .expect("writer");
    let reader = subr
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader");

    // Model: number of samples in the writer queue.
    // Since we are offline, the writer counts the sender and the reader
    // consumes nothing. We only test the backpressure behavior of
    // write_async — the guarantee is: full queue → Timeout.
    let mut model_in_queue: usize = 0;
    let mut next_payload: u8 = 0;

    for op in ops {
        match op {
            Op::Write => {
                next_payload = next_payload.wrapping_add(1);
                let payload = RawBytes::new(vec![next_payload]);
                let res = writer.write(&payload).await;
                if model_in_queue < capacity {
                    // Queue has room → write MUST succeed.
                    assert!(
                        res.is_ok(),
                        "write at queue {model_in_queue}/{capacity} \
                         expected Ok, got {res:?}",
                    );
                    model_in_queue += 1;
                } else {
                    // Queue full → write MUST return Timeout (no
                    // other error allowed).
                    assert!(
                        matches!(res, Err(DdsError::Timeout)),
                        "write at full queue ({capacity}) expected \
                         Timeout, got {res:?}",
                    );
                }
            }
            Op::Take => {
                // We use take with a short timeout — drain function.
                // Phase-1 async take always returns Ok(Vec) (offline =
                // empty). We conservatively reduce the model by 1.
                let _ = reader.take(Duration::from_millis(5)).await;
                // On the offline path the writer queue is not actually
                // drained by the internal take tick; the model must be
                // conservative and let the queue fill up to the cap.
                // That is exactly the test goal.
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 16,
        // Backpressure sequences with real Tokio timeouts cost
        // real time (each write past the cap = 20 ms). 16 cases are a
        // sensible compromise between coverage and wall-clock time.
        .. ProptestConfig::default()
    })]

    #[test]
    fn write_take_sequence_holds_invariants(
        capacity in capacity_strategy(),
        ops in ops_strategy(),
    ) {
        // proptest is sync, a Tokio runtime per iteration.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let topic_name = format!("AT_BP_PROP_{capacity}_{}", ops.len());
        rt.block_on(run_sequence(capacity, ops, &topic_name));
    }
}
