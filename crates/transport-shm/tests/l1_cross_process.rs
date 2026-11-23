//! L1 isolation test: POSIX-SHM transport across process boundaries.
//!
//! As with transport-uds, the parent spawns itself via
//! `Command::new(current_exe())` as a child. The child joins the
//! segment created by the parent, writes a datagram, the parent
//! reads it.

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

use std::env;
use std::process::{Command, Stdio};
use std::time::Duration;

use zerodds_rtps::wire_types::Locator;
use zerodds_transport::Transport;
use zerodds_transport_shm::{PosixShmTransport, ShmConfig};

const CHILD_ENV_FLINK: &str = "ZERODDS_SHM_L1_FLINK";
const PAYLOAD: &[u8] = b"l1-posix-shm-cross-process";

fn id(n: u8) -> [u8; 16] {
    let mut a = [0u8; 16];
    a[0] = 0xB0;
    a[15] = n;
    a
}

fn cfg(flink_dir: &std::path::Path) -> ShmConfig {
    ShmConfig {
        capacity: 1 << 16,
        flink_dir: flink_dir.to_path_buf(),
        max_datagram: 4096,
        recv_timeout: Some(Duration::from_secs(3)),
    }
}

#[test]
fn shm_2_3_l1_shm_datagram_crosses_process_boundary() {
    // Child role: join as owner? No — the owner created the segment
    // in the parent. Child joins as **sender** only if we invert
    // the model; but this transport is SpSc with sender=Owner.
    //
    // So: parent opens as owner (sender), child opens as consumer
    // (receiver), OR the inverse. We pick: child is the owner that
    // allocates the segment, parent is the consumer that reads.
    // Child must allocate first so parent.open_consumer succeeds.
    //
    // That means the test flow is:
    //   parent spawns child → child opens owner, sends, exits.
    //   parent opens consumer (after a short delay to let child
    //   initialize), reads the frame.
    //
    // The segment lives for the lifetime of the child's Shmem;
    // `shared_memory` keeps it alive until the owning process
    // unmaps, so parent must read before child exits.

    if let Ok(flink_dir) = env::var(CHILD_ENV_FLINK) {
        let flink_dir = std::path::PathBuf::from(flink_dir);
        let owner = PosixShmTransport::open_owner(id(0x02), id(0x01), cfg(&flink_dir))
            .expect("child open_owner");
        // Small delay so the parent has time to open_consumer.
        std::thread::sleep(Duration::from_millis(100));
        owner
            .send(&Locator::shm(id(0x01)), PAYLOAD)
            .expect("child send");
        // Keep the owner alive a bit so the consumer can read the
        // frame out before we unmap.
        std::thread::sleep(Duration::from_millis(500));
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let exe = env::current_exe().expect("current_exe");
    // `ZERODDS_TEST_RUNNER` starts the child through a runner (qemu-user
    // without binfmt), as cargo's target runner starts the parent.
    let runner = env::var("ZERODDS_TEST_RUNNER").unwrap_or_default();
    let mut runner = runner.split_whitespace();
    let mut command = match runner.next() {
        Some(program) => {
            let mut c = Command::new(program);
            c.args(runner).arg(&exe);
            c
        }
        None => Command::new(&exe),
    };
    let child = command
        .env(CHILD_ENV_FLINK, tmp.path())
        .args([
            "--exact",
            "shm_2_3_l1_shm_datagram_crosses_process_boundary",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn child");

    // Retry open_consumer until the child has created the segment
    // (up to 2 seconds). Prevents a race on slow CI hosts.
    let start = std::time::Instant::now();
    let consumer = loop {
        match PosixShmTransport::open_consumer(id(0x01), id(0x02), cfg(tmp.path())) {
            Ok(c) => break c,
            Err(_) if start.elapsed() < Duration::from_secs(2) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("parent open_consumer failed: {e}"),
        }
    };

    let got = consumer.recv().expect("parent recv");
    assert_eq!(&got.data[..], PAYLOAD);
    assert_eq!(got.source, Locator::shm(id(0x02)));

    let _ = child.wait_with_output();
}

const CHILD_ENV_WAKE: &str = "ZERODDS_SHM_L1_WAKE";

/// §4.4 across processes: a consumer parked on the ring in this process
/// is woken by the push of an owner in another process. The child opens
/// the owner side once the consumer is there, waits 300 ms (the consumer
/// is parked by then) and sends; the consumer's recv has a 20 s timeout,
/// which a wake that does not cross the process boundary would run out.
#[test]
fn shm_4_4_l1_a_push_wakes_a_consumer_in_another_process() {
    if let Ok(flink_dir) = env::var(CHILD_ENV_WAKE) {
        let flink_dir = std::path::PathBuf::from(flink_dir);
        let owner = PosixShmTransport::open_owner(id(0x04), id(0x03), cfg(&flink_dir))
            .expect("child open_owner");
        assert!(owner.peer_attached(), "the consumer opened first");
        std::thread::sleep(Duration::from_millis(300));
        owner
            .send(&Locator::shm(id(0x03)), PAYLOAD)
            .expect("child send");
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while owner.occupied_bytes() > 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = cfg(tmp.path());
    config.recv_timeout = Some(Duration::from_secs(20));
    let consumer =
        PosixShmTransport::open_consumer(id(0x03), id(0x04), config).expect("open_consumer");
    let exe = env::current_exe().expect("current_exe");
    let runner = env::var("ZERODDS_TEST_RUNNER").unwrap_or_default();
    let mut runner = runner.split_whitespace();
    let mut command = match runner.next() {
        Some(program) => {
            let mut c = Command::new(program);
            c.args(runner).arg(&exe);
            c
        }
        None => Command::new(&exe),
    };
    let child = command
        .env(CHILD_ENV_WAKE, tmp.path())
        .args([
            "--exact",
            "shm_4_4_l1_a_push_wakes_a_consumer_in_another_process",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn child");
    let start = std::time::Instant::now();
    let got = consumer.recv().expect("parent recv");
    let waited = start.elapsed();
    assert_eq!(&got.data[..], PAYLOAD);
    assert!(
        waited < Duration::from_secs(5),
        "the push of the other process woke the consumer after {waited:?}"
    );
    let status = child.wait_with_output().expect("child").status;
    assert!(status.success(), "child: {status}");
}
