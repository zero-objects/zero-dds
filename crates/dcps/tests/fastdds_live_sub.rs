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
        DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, TopicQos,
    };
    use zerodds_qos::{
        DurabilityKind, DurabilityQosPolicy, Duration as QosDuration, ReliabilityKind,
        ReliabilityQosPolicy,
    };

    use super::cross_vendor::{
        FastDurability, FastQos, FastReliability, require_or_skip, start_fastdds_sub,
    };

    const FASTDDS_DOMAIN: i32 = 97;

    fn rt_config() -> RuntimeConfig {
        RuntimeConfig {
            tick_period: Duration::from_millis(50),
            spdp_period: Duration::from_millis(300),
            ..RuntimeConfig::default()
        }
    }

    fn writer_qos(rel: ReliabilityKind, dur: DurabilityKind) -> DataWriterQos {
        DataWriterQos {
            reliability: ReliabilityQosPolicy {
                kind: rel,
                max_blocking_time: QosDuration::from_millis(100_i32),
            },
            durability: DurabilityQosPolicy { kind: dur },
            ..DataWriterQos::default()
        }
    }

    fn run_sub_test(topic: &'static str, fast_qos: FastQos, wr_qos: DataWriterQos) {
        if !require_or_skip("fastdds_live_sub") {
            return;
        }

        let factory = DomainParticipantFactory::instance();
        let p = factory
            .create_participant_with_config(
                FASTDDS_DOMAIN,
                DomainParticipantQos::default(),
                rt_config(),
            )
            .expect("create participant");
        let topic_handle = p
            .create_topic::<ShapeType>(topic, TopicQos::default())
            .expect("create topic");
        let publisher = p.create_publisher(PublisherQos::default());
        let writer = publisher
            .create_datawriter::<ShapeType>(&topic_handle, wr_qos)
            .expect("create writer");

        // Start the owned local Fast DDS subscriber (before the
        // first write, so discovery + any TransientLocal resend work).
        let mut fast = start_fastdds_sub(topic, FASTDDS_DOMAIN as u32, &fast_qos)
            .expect("start fastdds shape subscriber");

        // Discovery 3 s, then write periodically.
        std::thread::sleep(Duration::from_secs(3));
        for i in 0..40 {
            let s = ShapeType::new("RED", 50 + i % 100, 60 + i % 100, 30);
            writer.write(&s).expect("write");
            std::thread::sleep(Duration::from_millis(200));
        }

        // Give the FastDDS subscriber some time to flush the log.
        std::thread::sleep(Duration::from_secs(2));

        fast.ensure_running().expect("Fast DDS reader exited");
        assert!(
            fast.output()
                .expect("read owned vendor log")
                .lines()
                .any(|line| line.starts_with("SAMPLE color=RED ")),
            "owned Fast DDS subscriber log shows no samples for topic={topic}"
        );
        eprintln!("ZeroDDS→FastDDS sub-test {topic} OK");
    }

    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn fastdds_sub_besteffort_volatile_square() {
        run_sub_test(
            "Square",
            FastQos {
                reliability: FastReliability::BestEffort,
                durability: FastDurability::Volatile,
            },
            writer_qos(ReliabilityKind::BestEffort, DurabilityKind::Volatile),
        );
    }

    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn fastdds_sub_reliable_volatile_triangle() {
        run_sub_test(
            "Triangle",
            FastQos {
                reliability: FastReliability::Reliable,
                durability: FastDurability::Volatile,
            },
            writer_qos(ReliabilityKind::Reliable, DurabilityKind::Volatile),
        );
    }

    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn fastdds_sub_reliable_transient_local_circle() {
        run_sub_test(
            "Circle",
            FastQos {
                reliability: FastReliability::Reliable,
                durability: FastDurability::TransientLocal,
            },
            writer_qos(ReliabilityKind::Reliable, DurabilityKind::TransientLocal),
        );
    }

    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn fastdds_sub_besteffort_transient_local_square() {
        run_sub_test(
            "Square",
            FastQos {
                reliability: FastReliability::BestEffort,
                durability: FastDurability::TransientLocal,
            },
            writer_qos(ReliabilityKind::BestEffort, DurabilityKind::TransientLocal),
        );
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
#[ignore = "live FastDDS interop runs on Linux only"]
fn fastdds_sub_macos_stub() {
    eprintln!("FastDDS live tests run on Linux only.");
}
