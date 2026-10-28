//! Live local Fast DDS test using vendor-generated OMG appendable ShapeType.
//! Enable with INTEROP_REQUIRE=1 and FASTDDS_SHAPES pointing to the built client.
//! Each child has its own PID/log; missing tools and runtime errors fail.

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

#[path = "common/cross_vendor.rs"]
mod cross_vendor;

#[cfg(target_os = "linux")]
mod tests {
    use std::time::Duration;

    use zerodds_dcps::interop::ShapeType;
    use zerodds_dcps::runtime::RuntimeConfig;
    use zerodds_dcps::{
        DataReaderQos, DomainParticipantFactory, DomainParticipantQos, SubscriberQos, TopicQos,
    };
    use zerodds_qos::{
        DurabilityKind, DurabilityQosPolicy, Duration as QosDuration, HistoryKind,
        HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
    };

    use super::cross_vendor::{
        FastDurability, FastQos, FastReliability, require_or_skip, start_fastdds_pub,
    };

    const FASTDDS_DOMAIN: i32 = 98;
    const SAMPLES_TARGET: usize = 100;
    const RECEIVE_BUDGET: Duration = Duration::from_secs(20);
    const RECEIVE_MIN_BESTEFFORT: usize = 30;
    const RECEIVE_MIN_RELIABLE: usize = 80;

    fn run_matrix(
        topic: &'static str,
        fast: FastQos,
        zerodds_rel: ReliabilityKind,
        zerodds_dur: DurabilityKind,
        min_received: usize,
    ) {
        if !require_or_skip("qos_matrix") {
            return;
        }

        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(50),
            spdp_period: Duration::from_millis(300),
            ..RuntimeConfig::default()
        };
        let factory = DomainParticipantFactory::instance();
        let p = factory
            .create_participant_with_config(FASTDDS_DOMAIN, DomainParticipantQos::default(), cfg)
            .expect("create participant");
        let topic_handle = p
            .create_topic::<ShapeType>(topic, TopicQos::default())
            .expect("topic");
        let sub = p.create_subscriber(SubscriberQos::default());
        let reader = sub
            .create_datareader::<ShapeType>(
                &topic_handle,
                DataReaderQos {
                    reliability: ReliabilityQosPolicy {
                        kind: zerodds_rel,
                        max_blocking_time: QosDuration::from_millis(100_i32),
                    },
                    durability: DurabilityQosPolicy { kind: zerodds_dur },
                    // KEEP_ALL: every color here is a single instance, and
                    // the poll loop below expects a single `take()` to be
                    // able to harvest more than one sample accumulated
                    // since the last poll -- the default KEEP_LAST(1)
                    // would (correctly) cap each poll at 1 sample,
                    // turning the SAMPLES_TARGET/RECEIVE_BUDGET margin
                    // into a razor's edge.
                    history: HistoryQosPolicy {
                        kind: HistoryKind::KeepAll,
                        depth: 1,
                    },
                    ..DataReaderQos::default()
                },
            )
            .expect("reader");

        let mut fast = start_fastdds_pub(topic, FASTDDS_DOMAIN as u32, &fast)
            .expect("start fastdds shape pub");

        fast.ensure_running().expect("Fast DDS publisher startup");
        let deadline = std::time::Instant::now() + RECEIVE_BUDGET;
        let mut received = 0usize;
        while std::time::Instant::now() < deadline && received < SAMPLES_TARGET {
            fast.ensure_running().expect("Fast DDS publisher exited");
            let _ = reader.wait_for_data(Duration::from_millis(200));
            received += reader.take().expect("decode Fast DDS samples").len();
        }
        eprintln!(
            "QoS-Matrix [{topic}] fast={fast:?} zerodds=({zerodds_rel:?},{zerodds_dur:?}) \
             received={received}/{SAMPLES_TARGET}"
        );
        assert!(
            received >= min_received,
            "expected >= {min_received} samples, got {received}"
        );
    }

    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn qos_matrix_besteffort_volatile() {
        run_matrix(
            "Square",
            FastQos {
                reliability: FastReliability::BestEffort,
                durability: FastDurability::Volatile,
            },
            ReliabilityKind::BestEffort,
            DurabilityKind::Volatile,
            RECEIVE_MIN_BESTEFFORT,
        );
    }

    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn qos_matrix_besteffort_transient_local() {
        run_matrix(
            "Triangle",
            FastQos {
                reliability: FastReliability::BestEffort,
                durability: FastDurability::TransientLocal,
            },
            ReliabilityKind::BestEffort,
            DurabilityKind::TransientLocal,
            RECEIVE_MIN_BESTEFFORT,
        );
    }

    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn qos_matrix_reliable_volatile() {
        run_matrix(
            "Circle",
            FastQos {
                reliability: FastReliability::Reliable,
                durability: FastDurability::Volatile,
            },
            ReliabilityKind::Reliable,
            DurabilityKind::Volatile,
            RECEIVE_MIN_RELIABLE,
        );
    }

    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn qos_matrix_reliable_transient_local() {
        run_matrix(
            "Square",
            FastQos {
                reliability: FastReliability::Reliable,
                durability: FastDurability::TransientLocal,
            },
            ReliabilityKind::Reliable,
            DurabilityKind::TransientLocal,
            RECEIVE_MIN_RELIABLE,
        );
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
#[ignore = "live FastDDS interop runs on Linux only"]
fn fastdds_qos_matrix_macos_stub() {
    eprintln!("FastDDS QoS matrix tests run on Linux only.");
}
