//! End-to-end tests for the `isolation-smoke` binary (coverage
//! boost per phase2-0-coverage recommendation). Spawns the built
//! binary as a subprocess in sender/receiver role and checks the
//! Exit code + stderr output.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

fn smoke_bin() -> PathBuf {
    // Cargo sets CARGO_BIN_EXE_<name> for every bin target of the
    // same crate. This is the robust way, without guessing paths.
    PathBuf::from(env!("CARGO_BIN_EXE_isolation-smoke"))
}

fn unique_port(offset: u16) -> u16 {
    // Test pid + offset as port base. Tests run serially on
    // cargo test (default), collision-free.
    let pid = std::process::id() as u16;
    17000u16.wrapping_add(pid % 500).wrapping_add(offset)
}

#[test]
fn smoke_binary_udp_roundtrip_same_host() {
    let bin = smoke_bin();
    let rx_port = unique_port(0);
    let tx_port = unique_port(100);

    let receiver = Command::new(&bin)
        .args([
            "--transport=udp",
            "--role=receiver",
            &format!("--local=127.0.0.1:{rx_port}"),
            "--count=3",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn receiver");

    // Brief wait for receiver to bind.
    std::thread::sleep(Duration::from_millis(200));

    let sender = Command::new(&bin)
        .args([
            "--transport=udp",
            "--role=sender",
            &format!("--local=127.0.0.1:{tx_port}"),
            &format!("--peer=127.0.0.1:{rx_port}"),
            "--count=3",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("run sender");
    assert!(sender.status.success(), "sender failed: {sender:?}");

    let rx_out = receiver.wait_with_output().expect("receiver wait");
    assert!(rx_out.status.success(), "receiver failed: {rx_out:?}");
}

#[test]
fn smoke_binary_rejects_unknown_transport() {
    let out = Command::new(smoke_bin())
        .args(["--transport=quantum-entanglement", "--role=sender"])
        .output()
        .expect("run");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for config error"
    );
}

#[test]
fn smoke_binary_rejects_missing_local_for_udp() {
    // UDP sender without --local → config error (bind error at 1).
    let out = Command::new(smoke_bin())
        .args([
            "--transport=udp",
            "--role=sender",
            "--peer=127.0.0.1:9999",
            "--count=1",
        ])
        .output()
        .expect("run");
    // Exit code 1 (config) is correct — we forgot --local.
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn smoke_binary_rejects_bad_hex_id() {
    let out = Command::new(smoke_bin())
        .args([
            "--transport=uds",
            "--role=receiver",
            "--local-id=XYZ",
            "--count=1",
        ])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn smoke_binary_uds_roundtrip_same_host() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = smoke_bin();
    let base = tmp.path().to_str().unwrap().to_string();

    let receiver = Command::new(&bin)
        .args([
            "--transport=uds",
            "--role=receiver",
            "--local-id=a1",
            &format!("--base-dir={base}"),
            "--count=3",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn receiver");

    std::thread::sleep(Duration::from_millis(200));

    let sender = Command::new(&bin)
        .args([
            "--transport=uds",
            "--role=sender",
            "--local-id=a2",
            "--peer-id=a1",
            &format!("--base-dir={base}"),
            "--count=3",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("run sender");
    assert!(sender.status.success());
    let rx_out = receiver.wait_with_output().expect("receiver wait");
    assert!(rx_out.status.success());
}

/// zerodds-uds-transport-1.0 §5.2: `echo` sends every datagram back to the
/// locator it came from, `ping` sends `--count` datagrams one at a time and
/// checks each echo for its content and its source; the ping starts first
/// and waits for the echo's socket. Both log the exchange with the other's
/// locator. This is the pair the container test of §5.2 runs in two
/// containers (`tests/container_volume.rs`).
#[test]
fn uds_5_2_ping_and_echo_exchange_datagrams_in_both_directions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = smoke_bin();
    let base = tmp.path().to_str().unwrap().to_string();

    let ping = Command::new(&bin)
        .args([
            "--transport=uds",
            "--role=ping",
            "--local-id=b2",
            "--peer-id=b1",
            &format!("--base-dir={base}"),
            "--count=20",
            "--payload-size=300",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ping");
    std::thread::sleep(Duration::from_millis(300));
    let echo = Command::new(&bin)
        .args([
            "--transport=uds",
            "--role=echo",
            "--local-id=b1",
            &format!("--base-dir={base}"),
            "--count=20",
        ])
        .output()
        .expect("run echo");
    let ping = ping.wait_with_output().expect("ping wait");
    let echo_log = String::from_utf8_lossy(&echo.stderr);
    let ping_log = String::from_utf8_lossy(&ping.stderr);
    assert!(echo.status.success(), "echo: {echo_log}");
    assert!(ping.status.success(), "ping: {ping_log}");
    assert!(
        ping_log.contains(&format!("ping: 20 echoes received from Uds:{:0>32}", "b1")),
        "ping: {ping_log}"
    );
    assert!(
        echo_log.contains(&format!("echo: 20 datagrams echoed to Uds:{:0>32}", "b2")),
        "echo: {echo_log}"
    );
}

/// zerodds-uds-transport-1.0 §5.2: `--socket-mode` sets the mode of the
/// socket file and applies to UDS only; `echo` and `ping` are refused for
/// SHM, which has no answer path to a source locator.
#[test]
fn uds_5_2_socket_mode_and_the_echo_roles_are_refused_where_they_do_not_apply() {
    let bin = smoke_bin();
    for args in [
        &["--transport=udp", "--role=receiver", "--socket-mode=660"][..],
        &[
            "--transport=shm",
            "--role=echo",
            "--local-id=01",
            "--peer-id=02",
        ][..],
        &["--transport=uds", "--role=echo", "--socket-mode=9"][..],
    ] {
        let out = Command::new(&bin).args(args).output().expect("run");
        assert_eq!(out.status.code(), Some(1), "{args:?}: {out:?}");
    }
}
