//! L1 isolation test: separate OS processes exchange UDS datagrams.
//!
//! This test spawns itself as a child process with an
//! environment variable that steers the child into the sender path.
//! The parent binds a receiver and waits for the datagram.
//!
//! The test confirms that the UDS transport works not only same-process
//! but actually crosses process boundaries —
//! L1 from the isolation matrix.

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
use zerodds_transport_uds::{UdsConfig, UdsTransport};

const SENDER_ENV: &str = "ZERODDS_UDS_L1_SENDER";
const PAYLOAD: &[u8] = b"l1-cross-process-hello";

fn id_from(n: u8) -> [u8; 16] {
    let mut a = [0u8; 16];
    a[0] = 0xA1;
    a[15] = n;
    a
}

fn cfg(base: &std::path::Path, t: Duration) -> UdsConfig {
    UdsConfig {
        base_dir: base.to_path_buf(),
        max_datagram: 4096,
        recv_timeout: Some(t),
    }
}

#[test]
fn l1_datagram_crosses_process_boundary() {
    // If we were spawned as the sender child, run the sender path.
    if let Ok(base) = env::var(SENDER_ENV) {
        let base = std::path::PathBuf::from(base);
        let tx = UdsTransport::bind(id_from(0x02), cfg(&base, Duration::from_secs(2)))
            .expect("child bind sender");
        // Give the parent a brief moment to finish its bind.
        std::thread::sleep(Duration::from_millis(50));
        tx.send(&Locator::uds(id_from(0x01)), PAYLOAD)
            .expect("child send");
        return;
    }

    // Parent path: bind receiver, then spawn child as sender.
    let tmp = tempfile::tempdir().expect("tempdir");
    let rx = UdsTransport::bind(id_from(0x01), cfg(tmp.path(), Duration::from_secs(5)))
        .expect("parent bind receiver");

    let exe = env::current_exe().expect("current_exe");
    let child = Command::new(exe)
        .env(SENDER_ENV, tmp.path())
        .args(["--exact", "l1_datagram_crosses_process_boundary"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn child");

    let got = rx.recv().expect("parent recv from child");
    assert_eq!(&got.data[..], PAYLOAD);
    assert_eq!(got.source, Locator::uds(id_from(0x02)));

    // Wait for child to exit to avoid zombie.
    let _ = child.wait_with_output();
}
