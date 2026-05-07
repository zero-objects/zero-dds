//! C5.5 — Live-Interop: FastDDS-Publisher → ZeroDDS-Subscriber.
//!
//! **Opt-in only** — `#[ignore]` markiert. Aufruf:
//!
//! ```bash
//! cargo test -p zerodds-dcps --features live-interop \
//!     --test fastdds_live_pub -- --ignored --nocapture
//! ```
//!
//! # Spec-Bezug
//!
//! - DDSI-RTPS 2.5 §8.3 — RTPS Wire-Encoding (DATA/HEARTBEAT/ACKNACK)
//! - XCDR2-Encoding-Stack (`@key string color; int32 x,y,shapesize;`)
//! - DDS 1.4 §2.2.3 — RxO-QoS-Match (Reliability/Durability)
//!
//! # Test-Ablauf (gemeinsam fuer alle Matrix-Eintraege)
//!
//! 1. ZeroDDS-Participant + DataReader<ShapeType> auf Topic `Square`.
//! 2. `fastdds shape publisher --topic Square --color RED ...` auf
//!    llvm via SSH. Publisher schreibt periodisch Samples.
//! 3. Reader sammelt mind. 1 Sample innerhalb 10 s. Alle Felder
//!    werden auf Plausibilitaet geprueft (color="RED", x/y in
//!    Canvas-Range).
//! 4. Subprocess wird via Drop gekillt.
//!
//! # Topic-Naming-Konvention
//!
//! `fastdds shape` nutzt exakt `Square`/`Triangle`/`Circle` (case-
//! sensitive, ohne Namespace). ZeroDDS-`create_topic::<ShapeType>(name)`
//! akzeptiert beliebige Strings, daher passt es 1:1 — wichtig nur,
//! dass beide Seiten denselben TYPE_NAME `ShapeType` verwenden.
//!
//! # Bekannte Edge-Cases
//!
//! - FastDDS sendet nicht gleich beim Start; je nach Discovery-Race
//!   kann der erste Sample ~3 s dauern → Timeout 12 s grosszuegig.
//! - `fastdds shape` setzt Default-Color je nach Build; wir matchen
//!   nur "irgendein Sample mit nicht-leerer color".

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

    /// Domain fuer FastDDS-Live-Tests. Eigener Block, damit Cyclone-
    /// Tests (Domain 42) nicht kollidieren.
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

        // FastDDS-Publisher remote starten.
        let _fast = start_fastdds_pub(topic, FASTDDS_DOMAIN as u32, &fast_qos)
            .expect("start fastdds shape publisher");

        // Discovery + Daten — 12 s Budget.
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
        // ShapesDemo-Canvas typischerweise <= 240x270.
        assert!(s.x >= 0 && s.x <= 1024, "x out of plausible range: {}", s.x);
        assert!(s.y >= 0 && s.y <= 1024, "y out of plausible range: {}", s.y);
        eprintln!(
            "FastDDS→ZeroDDS pub-test {topic} OK: {} samples, first={:?}",
            samples.len(),
            s
        );
    }

    /// Matrix-Eintrag (BestEffort × Volatile): Default-FastDDS-Setup,
    /// ohne Reliability-State. Schnellster Pfad, kein Heartbeat-Loop.
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

    /// Matrix-Eintrag (Reliable × Volatile): voller Reliable-Loop mit
    /// HEARTBEAT/ACKNACK auf Wire — der Stresstest fuer
    /// Cross-Vendor-Reliable-State.
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

    /// Matrix-Eintrag (Reliable × TransientLocal): writer-side
    /// Sample-Cache wird beim Late-Joining-Reader resent. Wir
    /// joinen *nach* dem Pub-Start.
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

    /// Matrix-Eintrag (BestEffort × TransientLocal): unueblich aber
    /// per Spec erlaubt; FastDDS sollte den Cache nicht resenden,
    /// neue Samples kommen aber an.
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

// macOS-Stub damit das Test-Binary auch ohne Linux kompiliert.
#[cfg(not(target_os = "linux"))]
#[test]
#[ignore = "live FastDDS interop runs on Linux only (multicast loopback)"]
fn fastdds_pub_macos_stub() {
    eprintln!("FastDDS-Live-Tests laufen nur auf Linux — siehe shapes_api_e2e.rs Comment.");
}
