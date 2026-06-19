//! C5.5 — Live-Interop: FastDDS-Publisher → ZeroDDS-Subscriber.
//!
//! **Opt-in only** — `#[ignore]` marked. Invocation:
//!
//! ```bash
//! cargo test -p zerodds-dcps --features live-interop \
//!     --test fastdds_live_pub -- --ignored --nocapture
//! ```
//!
//! # Spec reference
//!
//! - DDSI-RTPS 2.5 §8.3 — RTPS wire encoding (DATA/HEARTBEAT/ACKNACK)
//! - XCDR2 encoding stack (`@key string color; int32 x,y,shapesize;`)
//! - DDS 1.4 §2.2.3 — RxO QoS match (Reliability/Durability)
//!
//! # Test flow (common to all matrix entries)
//!
//! 1. ZeroDDS participant + DataReader<ShapeType> on topic `Square`.
//! 2. `fastdds shape publisher --topic Square --color RED ...` on the
//!    Linux bench host via SSH. The publisher writes samples periodically.
//! 3. The reader collects at least 1 sample within 10 s. All fields are
//!    sanity-checked (color="RED", x/y within canvas range).
//! 4. The subprocess is killed via Drop.
//!
//! # Topic-naming convention
//!
//! `fastdds shape` uses exactly `Square`/`Triangle`/`Circle` (case-
//! sensitive, without namespace). ZeroDDS's `create_topic::<ShapeType>(name)`
//! accepts arbitrary strings, so it matches 1:1 — the only thing that
//! matters is that both sides use the same TYPE_NAME `ShapeType`.
//!
//! # Known edge cases
//!
//! - FastDDS does not send immediately on start; depending on the
//!   discovery race the first sample can take ~3 s → a generous 12 s
//!   timeout.
//! - `fastdds shape` sets a default color depending on the build; we
//!   match only "any sample with a non-empty color".

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
        FastDurability, FastQos, FastReliability, live_host_available, start_fastdds_pub,
    };

    /// Domain for FastDDS live tests. Dedicated block so it does not
    /// collide with the Cyclone tests (domain 42).
    const FASTDDS_DOMAIN: i32 = 142;

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
        if !live_host_available() {
            eprintln!("LLVM_HOST_AVAILABLE not set + sshpass missing — skipping");
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

        // Start the FastDDS publisher remotely.
        let _fast = start_fastdds_pub(topic, FASTDDS_DOMAIN as u32, &fast_qos)
            .expect("start fastdds shape publisher");

        // Discovery + data — 12 s budget.
        let deadline = std::time::Instant::now() + Duration::from_secs(12);
        let mut samples = Vec::new();
        while std::time::Instant::now() < deadline && samples.is_empty() {
            let _ = reader.wait_for_data(Duration::from_millis(200));
            samples.extend(reader.take().unwrap_or_default());
        }

        assert!(
            !samples.is_empty(),
            "no ShapeType samples received from FastDDS within 12 s on topic={topic}"
        );
        let s = &samples[0];
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

    /// Matrix entry (Reliable × TransientLocal): the writer-side
    /// sample cache is resent to the late-joining reader. We join
    /// *after* the publisher starts.
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
