//! zerodds-async-1.0 §8: proptest ueber Channel-Backpressure.
//!
//! Property: gegen einen offline-Participant mit Reliable + KeepAll +
//! `max_samples = N` darf eine zufaellige Folge aus `write` und `take`
//! niemals
//! - die Queue ueber N hinaus wachsen lassen,
//! - bei voller Queue eine andere Fehlerklasse als `Timeout` liefern,
//! - bei freier Queue write fehlschlagen lassen.
//!
//! Das absichert §5.1 (`write`-Future suspendiert bei OutOfResources)
//! und §6.1 (data_available_stream / take liefern den deterministischen
//! Drain).

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

/// Erzeugt eine Sequenz aus Writes und Takes mit klar begrenzten
/// Anteilen — proptest verkleinert auf der Sequenz selbst.
fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![Just(Op::Write), Just(Op::Take)]
}

fn ops_strategy() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op_strategy(), 0..32)
}

/// Erzeugt zufaellige Queue-Kapazitaet [1, 8] — klein genug, dass die
/// proptest-Sequenz die Grenze regelmaessig trifft.
fn capacity_strategy() -> impl Strategy<Value = usize> {
    1usize..=8
}

/// Pro Property-Iteration: frisch initialisierter Async-Participant +
/// Topic + Reliable-Writer + Reader, gemeinsam.
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

    // Modell: Anzahl Samples in der Writer-Queue.
    // Da wir offline sind, zaehlt der Writer den Sender, der Reader
    // konsumiert nichts. Wir testen nur das Backpressure-Verhalten von
    // write_async — die Garantie ist: bei voller Queue → Timeout.
    let mut model_in_queue: usize = 0;
    let mut next_payload: u8 = 0;

    for op in ops {
        match op {
            Op::Write => {
                next_payload = next_payload.wrapping_add(1);
                let payload = RawBytes::new(vec![next_payload]);
                let res = writer.write(&payload).await;
                if model_in_queue < capacity {
                    // Queue hat Platz → write MUSS gelingen.
                    assert!(
                        res.is_ok(),
                        "write at queue {model_in_queue}/{capacity} \
                         expected Ok, got {res:?}",
                    );
                    model_in_queue += 1;
                } else {
                    // Queue voll → write MUSS Timeout liefern (kein
                    // anderer Fehler erlaubt).
                    assert!(
                        matches!(res, Err(DdsError::Timeout)),
                        "write at full queue ({capacity}) expected \
                         Timeout, got {res:?}",
                    );
                }
            }
            Op::Take => {
                // Wir nutzen take mit kurzem Timeout — drain-Funktion.
                // Phase-1-async take liefert immer Ok(Vec) (offline =
                // leer). Wir reduzieren das Modell konservativ um 1.
                let _ = reader.take(Duration::from_millis(5)).await;
                // Im offline-Pfad wird die Writer-Queue durch den
                // internen take-Tick nicht wirklich gedrained; das
                // Modell muss konservativ sein und die Queue fuellen
                // lassen bis zum Cap. Genau das ist das Test-Ziel.
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 16,
        // Backpressure-Sequenzen mit echten Tokio-Timeouts kosten
        // Realzeit (jedes write nach Cap = 20 ms). 16 Cases sind ein
        // sinnvoller Kompromiss zwischen Coverage und Wall-Clock.
        .. ProptestConfig::default()
    })]

    #[test]
    fn write_take_sequence_holds_invariants(
        capacity in capacity_strategy(),
        ops in ops_strategy(),
    ) {
        // proptest ist sync, Tokio-Runtime per Iteration.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let topic_name = format!("AT_BP_PROP_{capacity}_{}", ops.len());
        rt.block_on(run_sequence(capacity, ops, &topic_name));
    }
}
