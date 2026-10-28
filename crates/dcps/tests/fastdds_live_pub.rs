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
        DurabilityKind, DurabilityQosPolicy, Duration as QosDuration, ReliabilityKind,
        ReliabilityQosPolicy,
    };

    use super::cross_vendor::{
        FastDurability, FastQos, FastReliability, require_or_skip, start_fastdds_pub,
    };

    /// Domain for FastDDS live tests. Dedicated number so it does not
    /// collide with the Cyclone tests (domain 42), and below `32768 / 250`
    /// so its RTPS ports stay out of the Linux ephemeral port range —
    /// a port the kernel also hands out makes the SPDP multicast bind
    /// fail with `EADDRINUSE`.
    const FASTDDS_DOMAIN: i32 = 96;

    fn rt_config() -> RuntimeConfig {
        RuntimeConfig {
            tick_period: Duration::from_millis(50),
            spdp_period: Duration::from_millis(300),
            ..RuntimeConfig::default()
        }
    }

    fn reader_qos(rel: ReliabilityKind, dur: DurabilityKind) -> DataReaderQos {
        DataReaderQos {
            reliability: ReliabilityQosPolicy {
                kind: rel,
                max_blocking_time: QosDuration::from_millis(100_i32),
            },
            durability: DurabilityQosPolicy { kind: dur },
            ..DataReaderQos::default()
        }
    }

    fn run_pub_test(topic: &'static str, fast_qos: FastQos, rdr_qos: DataReaderQos) {
        if !require_or_skip("fastdds_live_pub") {
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
        let sub = p.create_subscriber(SubscriberQos::default());
        let reader = sub
            .create_datareader::<ShapeType>(&topic_handle, rdr_qos)
            .expect("create reader");

        // Start the owned local Fast DDS publisher.
        let mut fast = start_fastdds_pub(topic, FASTDDS_DOMAIN as u32, &fast_qos)
            .expect("start fastdds shape publisher");

        // Discovery + data — 12 s budget.
        fast.ensure_running().expect("Fast DDS publisher startup");
        let deadline = std::time::Instant::now() + Duration::from_secs(12);
        let mut samples = Vec::new();
        while std::time::Instant::now() < deadline && samples.is_empty() {
            fast.ensure_running().expect("Fast DDS publisher exited");
            let _ = reader.wait_for_data(Duration::from_millis(200));
            samples.extend(reader.take().expect("decode Fast DDS samples"));
        }

        assert!(
            !samples.is_empty(),
            "no ShapeType samples received from FastDDS within 12 s on topic={topic}"
        );
        let s = &samples[0];
        assert_eq!(s, &ShapeType::new("RED", 50, 60, 30));
        assert!(
            !s.color.is_empty(),
            "empty color field — wire-decode broken?"
        );
        // The ShapesDemo canvas is typically <= 240x270.
        assert!(s.x >= 0 && s.x <= 1024, "x out of plausible range: {}", s.x);
        assert!(s.y >= 0 && s.y <= 1024, "y out of plausible range: {}", s.y);
        eprintln!(
            "FastDDS→ZeroDDS pub-test {topic} OK: {} samples, first={:?}",
            samples.len(),
            s
        );
    }

    /// Matrix entry (BestEffort × Volatile): default FastDDS setup,
    /// without reliability state. Fastest path, no heartbeat loop.
    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn fastdds_pub_besteffort_volatile_square() {
        run_pub_test(
            "Square",
            FastQos {
                reliability: FastReliability::BestEffort,
                durability: FastDurability::Volatile,
            },
            reader_qos(ReliabilityKind::BestEffort, DurabilityKind::Volatile),
        );
    }

    /// Matrix entry (Reliable × Volatile): full reliable loop with
    /// HEARTBEAT/ACKNACK on the wire — the stress test for
    /// cross-vendor reliable state.
    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn fastdds_pub_reliable_volatile_triangle() {
        run_pub_test(
            "Triangle",
            FastQos {
                reliability: FastReliability::Reliable,
                durability: FastDurability::Volatile,
            },
            reader_qos(ReliabilityKind::Reliable, DurabilityKind::Volatile),
        );
    }

    /// Matrix entry (Reliable × TransientLocal): matching live delivery.
    /// This fixture does not independently prove late-join cache replay.
    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn fastdds_pub_reliable_transient_local_circle() {
        run_pub_test(
            "Circle",
            FastQos {
                reliability: FastReliability::Reliable,
                durability: FastDurability::TransientLocal,
            },
            reader_qos(ReliabilityKind::Reliable, DurabilityKind::TransientLocal),
        );
    }

    /// Matrix entry (BestEffort × TransientLocal): unusual but
    /// allowed per spec; FastDDS should not resend the cache, but
    /// new samples still arrive.
    #[test]
    #[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
    fn fastdds_pub_besteffort_transient_local_square() {
        run_pub_test(
            "Square",
            FastQos {
                reliability: FastReliability::BestEffort,
                durability: FastDurability::TransientLocal,
            },
            reader_qos(ReliabilityKind::BestEffort, DurabilityKind::TransientLocal),
        );
    }
}

// macOS stub so the test binary also compiles without Linux.
#[cfg(not(target_os = "linux"))]
#[test]
#[ignore = "live FastDDS interop runs on Linux only (multicast loopback)"]
fn fastdds_pub_macos_stub() {
    eprintln!("FastDDS live tests run on Linux only — see the comment in shapes_api_e2e.rs.");
}
