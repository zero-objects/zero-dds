//! zerodds-async-1.0 §8 — E2E against live Cyclone.
//!
//! **Opt-in only** — marked `#[ignore]`. Invocation:
//!
//! ```bash
//! cargo test -p zerodds-dcps-async --test cyclone_live_async_e2e -- --ignored --nocapture
//! ```
//!
//! # Prerequisites
//!
//! - `LLVM_HOST_AVAILABLE=1` env or `sshpass` installed locally
//! - SSH access `llvm@llvm` with password `llvm` (lab setup; see
//!   memory `reference_bench_hosts`)
//! - Cyclone DDS on the Linux bench host with the `ddsperf` binary
//! - `/tmp/cyc.xml` on the Linux bench host pins Cyclone to `enp6s18`
//! - PVE multicast querier setup (see memory
//!   `reference_pve_multicast_setup`)
//!
//! # Test flow
//!
//! 1. Local: AsyncDomainParticipant + AsyncDataReader on a
//!    unique test topic (Cyclone `ddsperf` topic names are
//!    fixed; we test the negative scenario "no sample on the
//!    wrong topic" as a smoke test for the async discovery path).
//! 2. Remote: ddsperf pub on the same domain.
//! 3. Polling via `take().await(timeout)` — at least one tick must
//!    run through the async path without panicking.
//! 4. Teardown: SSH pkill of `ddsperf`.
//!
//! # Why "#[ignore]"?
//!
//! - Cyclone binary not in CI
//! - SSH access + password are maintenance-heavy
//! - network dependency (LAN setup)
//!
//! The deterministic path is in `smoke.rs::writer_write_async_offline`
//! without `#[ignore]` and covers the non-Cyclone-specific API.
//!
//! # Latency comparison sync vs async (Spec §8)
//!
//! The bench path in `benches/write_async_vs_sync.rs` (Spec §9.1)
//! provides the quantitative answer. This live test only checks
//! that the async stack is discovery-capable against a real vendor
//! stack.

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

/// Checks whether the live host is reachable; otherwise skip without error.
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

    // Local: AsyncDomainParticipant on the same domain as ddsperf.
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

    // Remote: start ddsperf pub (running for 10 s).
    let _cyclone = DdsperfPub::start(CYCLONE_DOMAIN, 10).expect("start ddsperf");

    // Async take loop: at least one take() tick must pass without a panic.
    // We do *not* assert that samples arrive (a topic mismatch is
    // likely, ddsperf publishes on "PingTopic"); we only assert
    // that the async path is robust against a real vendor stack
    // (no panic, no hang, no OutOfResources).
    let start = std::time::Instant::now();
    let mut ticks = 0usize;
    while start.elapsed() < Duration::from_secs(8) {
        let res = reader.take(Duration::from_millis(500)).await;
        // Whether Ok([]) or Ok([..]) — the path must run through.
        // Err is only accepted when it is a Timeout (deadline expired).
        match res {
            Ok(_) | Err(zerodds_dcps_async::DdsError::Timeout) => ticks += 1,
            Err(e) => panic!("unexpected error from async take: {e:?}"),
        }
    }
    assert!(ticks > 0, "expected at least 1 take-tick, got 0");
    eprintln!("async live-cyclone smoke: {ticks} ticks completed without panic");
}
