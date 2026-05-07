//! C5.5 — Live-Interop: ZeroDDS-Publisher → FastDDS-Subscriber.
//!
//! **Opt-in only** — `#[ignore]` markiert. Aufruf:
//!
//! ```bash
//! cargo test -p zerodds-dcps --features live-interop \
//!     --test fastdds_live_sub -- --ignored --nocapture
//! ```
//!
//! # Spec-Bezug
//!
//! - DDSI-RTPS 2.5 §8.3 — DATA-Submessage outbound
//! - XCDR2-Encoding (`@key string color; int32 x,y,shapesize;`)
//! - PID_TYPE_NAME / PID_TOPIC_NAME — FastDDS muss unsere SEDP-
//!   Publication als matchend zum lokalen Reader erkennen.
//!
//! # Test-Ablauf
//!
//! 1. ZeroDDS-Participant + DataWriter<ShapeType> auf Topic `Square`.
//! 2. Writer schickt periodisch Samples ueber 10 s.
//! 3. `fastdds shape subscriber --topic Square ...` auf llvm via SSH.
//! 4. Subscriber-Log auf llvm wird remote gegrep't auf `Sample
//!    received`-Substring (FastDDS-Standard-Log-Format).
//! 5. Erfolg = mind. 1 "Sample received"-Line im Log.
//!
//! # Bekannte Edge-Cases
//!
//! - FastDDS-`shape` matched nur `ShapeType` als IDL-Typ; unser
//!   `ShapeType` muss exakt diesen Type-Name fuehren (ist es —
//!   `interop.rs` setzt das hart).
//! - `fastdds shape subscriber` printet auf stdout, daher pipen wir
//!   auf eine Logdatei und tail-en.

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
    use std::process::Command;
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
        FastDurability, FastQos, FastReliability, SSH_HOST, SSH_PASS, SSH_USER,
        live_host_available, start_fastdds_sub,
    };

    const FASTDDS_DOMAIN: i32 = 143;

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

    /// Pruefen ob das FastDDS-Sub-Log auf llvm einen Sample-Empfang
    /// dokumentiert. `fastdds shape subscriber` schreibt eine Zeile
    /// pro empfangenem Sample.
    fn fastdds_log_has_sample(topic: &str) -> bool {
        let path = format!("/tmp/fastdds_sub_{topic}.log");
        let cmd = format!("grep -c 'received' {path} 2>/dev/null || echo 0");
        let out = Command::new("sshpass")
            .arg("-p")
            .arg(SSH_PASS)
            .arg("ssh")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg(format!("{SSH_USER}@{SSH_HOST}"))
            .arg(&cmd)
            .output();
        if let Ok(o) = out {
            if let Ok(s) = core::str::from_utf8(&o.stdout) {
                if let Ok(n) = s.trim().parse::<u32>() {
                    return n > 0;
                }
            }
        }
        false
    }

    fn run_sub_test(topic: &'static str, fast_qos: FastQos, wr_qos: DataWriterQos) {
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
        let publisher = p.create_publisher(PublisherQos::default());
        let writer = publisher
            .create_datawriter::<ShapeType>(&topic_handle, wr_qos)
            .expect("create writer");

        // FastDDS-Subscriber auf llvm starten (vor dem ersten Write,
        // damit Discovery + ggf. TransientLocal-Resend funktionieren).
        let _fast = start_fastdds_sub(topic, FASTDDS_DOMAIN as u32, &fast_qos)
            .expect("start fastdds shape subscriber");

        // Discovery 3 s, dann periodisch schreiben.
        std::thread::sleep(Duration::from_secs(3));
        for i in 0..40 {
            let s = ShapeType::new("RED", 50 + i % 100, 60 + i % 100, 30);
            writer.write(&s).expect("write");
            std::thread::sleep(Duration::from_millis(200));
        }

        // FastDDS-Subscriber bekommt etwas Zeit, das Log zu flushen.
        std::thread::sleep(Duration::from_secs(2));

        assert!(
            fastdds_log_has_sample(topic),
            "fastdds shape subscriber log on llvm shows no samples for topic={topic}"
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
    eprintln!("FastDDS-Live-Tests laufen nur auf Linux.");
}
