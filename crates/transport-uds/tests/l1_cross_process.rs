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
#[cfg(unix)]
use zerodds_transport_uds::UdsTransport;
use zerodds_transport_uds::{UdsConfig, UdsStreamTransport};

#[cfg(unix)]
const SENDER_ENV: &str = "ZERODDS_UDS_L1_SENDER";
const STREAM_SENDER_ENV: &str = "ZERODDS_UDS_L1_STREAM_SENDER";
const PAYLOAD: &[u8] = b"l1-cross-process-hello";

/// A temporary directory with a short path: under `/tmp` on Unix, where the
/// per-user TMPDIR of macOS takes 49 of the 104 bytes of `sun_path`.
fn short_tempdir() -> tempfile::TempDir {
    if cfg!(unix) {
        tempfile::Builder::new()
            .tempdir_in("/tmp")
            .expect("tempdir")
    } else {
        tempfile::tempdir().expect("tempdir")
    }
}

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
        ..UdsConfig::default()
    }
}

#[cfg(unix)]
#[test]
fn uds_3_1_l1_datagram_crosses_process_boundary() {
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
    let tmp = short_tempdir();
    let rx = UdsTransport::bind(id_from(0x01), cfg(tmp.path(), Duration::from_secs(5)))
        .expect("parent bind receiver");

    let exe = env::current_exe().expect("current_exe");
    let child = Command::new(exe)
        .env(SENDER_ENV, tmp.path())
        .args(["--exact", "uds_3_1_l1_datagram_crosses_process_boundary"])
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

/// §3.5 and §6.2: a child process connects to the listening socket of the
/// parent's stream transport and sends three messages; the parent receives
/// them whole, in order, with the child's id as the source. On Windows this
/// is the UDS transport.
#[test]
fn uds_3_5_l1_stream_frames_cross_the_process_boundary() {
    if let Ok(base) = env::var(STREAM_SENDER_ENV) {
        let base = std::path::PathBuf::from(base);
        let tx = UdsStreamTransport::bind(id_from(0x12), cfg(&base, Duration::from_secs(2)))
            .expect("child bind sender");
        for i in 0..3u8 {
            let mut msg = PAYLOAD.to_vec();
            msg.push(i);
            tx.send(&Locator::uds(id_from(0x11)), &msg)
                .expect("child send");
        }
        return;
    }
    let tmp = short_tempdir();
    let rx = UdsStreamTransport::bind(id_from(0x11), cfg(tmp.path(), Duration::from_secs(5)))
        .expect("parent bind receiver");
    let exe = env::current_exe().expect("current_exe");
    let child = Command::new(exe)
        .env(STREAM_SENDER_ENV, tmp.path())
        .args([
            "--exact",
            "uds_3_5_l1_stream_frames_cross_the_process_boundary",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn child");
    for i in 0..3u8 {
        let got = rx.recv().expect("parent recv from child");
        let mut msg = PAYLOAD.to_vec();
        msg.push(i);
        assert_eq!(&got.data[..], &msg[..]);
        assert_eq!(got.source, Locator::uds(id_from(0x12)));
    }
    let out = child.wait_with_output().expect("child");
    assert!(out.status.success(), "{out:?}");
}
