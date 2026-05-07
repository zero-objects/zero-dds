//! zerodds-async-1.0 §8 — E2E gegen Cyclone-Live.
//!
//! **Opt-in only** — `#[ignore]` markiert. Aufruf:
//!
//! ```bash
//! cargo test -p zerodds-dcps-async --test cyclone_live_async_e2e -- --ignored --nocapture
//! ```
//!
//! # Voraussetzungen
//!
//! - `LLVM_HOST_AVAILABLE=1` env oder `sshpass` lokal installiert
//! - SSH-Zugriff `llvm@llvm` mit Passwort `llvm` (Lab-Setup; siehe
//!   Memory `reference_bench_hosts`)
//! - Cyclone DDS auf `llvm` mit `ddsperf` Binary
//! - `/tmp/cyc.xml` auf `llvm` pinnt Cyclone auf `enp6s18`
//! - PVE-Multicast-Querier-Setup (siehe Memory
//!   `reference_pve_multicast_setup`)
//!
//! # Test-Ablauf
//!
//! 1. Lokal: AsyncDomainParticipant + AsyncDataReader auf einem
//!    eindeutigen Test-Topic (Cyclone `ddsperf`-Topic-Namen sind
//!    fixed; wir testen das Negativ-Szenario "kein Sample bei
//!    falschem Topic" als Smoke fuer den Async-Discovery-Pfad).
//! 2. Remote: ddsperf-Pub auf demselben Domain.
//! 3. Polling via `take().await(timeout)` — mindestens ein Tick muss
//!    durch den async-Pfad laufen, ohne zu panicen.
//! 4. Teardown: SSH-pkill auf `ddsperf`.
//!
//! # Warum "#[ignore]"?
//!
//! - Cyclone-Binary nicht im CI
//! - SSH-Zugang + Passwort pflegeintensiv
//! - Netzwerk-Abhaengigkeit (LAN-Setup)
//!
//! Der deterministische Pfad ist in `smoke.rs::writer_write_async_offline`
//! ohne `#[ignore]` und deckt die nicht-Cyclone-spezifische API ab.
//!
//! # Latency-Vergleich Sync vs Async (Spec §8)
//!
//! Der Bench-Pfad in `benches/write_async_vs_sync.rs` (Spec §9.1)
//! liefert die quantitative Antwort. Dieser Live-Test prueft nur,
//! dass der async-Stack gegen einen echten Vendor-Stack
//! discovery-faehig ist.

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

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use zerodds_dcps::RawBytes;
use zerodds_dcps_async::{AsyncDomainParticipantFactory, DataReaderQos, SubscriberQos, TopicQos};

const SSH_USER: &str = "llvm";
const SSH_PASS: &str = "llvm";
const SSH_HOST: &str = "llvm";
const CYCLONE_DOMAIN: u32 = 42;

/// Pruefung ob Live-Host erreichbar ist; sonst skip ohne Fehler.
fn live_host_available() -> bool {
    if std::env::var("LLVM_HOST_AVAILABLE").is_ok() {
        return true;
    }
    Command::new("sshpass")
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

struct DdsperfPub {
    child: Child,
}

impl DdsperfPub {
    fn start(domain: u32, duration_secs: u32) -> std::io::Result<Self> {
        let cmd = format!(
            "CYCLONEDDS_URI=file:///tmp/cyc.xml timeout {} ddsperf -i {} -D {} pub 1Hz \
             > /tmp/cyc_pub_async_e2e.log 2>&1",
            duration_secs + 5,
            domain,
            duration_secs,
        );
        let child = Command::new("sshpass")
            .args(["-p", SSH_PASS, "ssh", "-o", "StrictHostKeyChecking=no"])
            .arg(format!("{SSH_USER}@{SSH_HOST}"))
            .arg(&cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self { child })
    }
}

impl Drop for DdsperfPub {
    fn drop(&mut self) {
        let _ = Command::new("sshpass")
            .args(["-p", SSH_PASS, "ssh", "-o", "StrictHostKeyChecking=no"])
            .arg(format!("{SSH_USER}@{SSH_HOST}"))
            .arg(format!("pkill -f 'ddsperf -i {CYCLONE_DOMAIN}' || true"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[ignore = "requires Cyclone-DDS Lab access (llvm@llvm, sshpass, multicast)"]
#[tokio::test(flavor = "multi_thread")]
async fn async_reader_does_not_panic_against_live_cyclone_pub() {
    if !live_host_available() {
        eprintln!("skip: LLVM_HOST_AVAILABLE not set and no sshpass — see module docs");
        return;
    }

    // Lokal: AsyncDomainParticipant auf demselben Domain wie ddsperf.
    let f = AsyncDomainParticipantFactory::instance();
    let p = f
        .create_participant(CYCLONE_DOMAIN as i32)
        .expect("create_participant");
    let topic = p
        .create_topic::<RawBytes>("DDSPerfTopic", TopicQos::default())
        .expect("topic");
    let subr = p.create_subscriber(SubscriberQos::default());
    let reader = subr
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader");

    // Remote: ddsperf-Pub starten (10 s laufend).
    let _cyclone = DdsperfPub::start(CYCLONE_DOMAIN, 10).expect("start ddsperf");

    // Async-take-Loop: mindestens ein take()-Tick muss ohne Panic durch.
    // Wir asserten *nicht*, dass Samples ankommen (Topic-Mismatch ist
    // wahrscheinlich, ddsperf publiziert auf "PingTopic"); wir asserten
    // nur, dass der async-Pfad robust gegen einen echten Vendor-Stack
    // ist (kein Panic, kein Hang, kein OutOfResources).
    let start = std::time::Instant::now();
    let mut ticks = 0usize;
    while start.elapsed() < Duration::from_secs(8) {
        let res = reader.take(Duration::from_millis(500)).await;
        // Egal ob Ok([]) oder Ok([..]) — der Pfad muss runlaufen.
        // Err nur akzeptiert wenn Timeout (deadline expired).
        match res {
            Ok(_) | Err(zerodds_dcps_async::DdsError::Timeout) => ticks += 1,
            Err(e) => panic!("unexpected error from async take: {e:?}"),
        }
    }
    assert!(ticks > 0, "expected at least 1 take-tick, got 0");
    eprintln!("async live-cyclone smoke: {ticks} ticks completed without panic");
}
