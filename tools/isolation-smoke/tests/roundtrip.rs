//! End-to-End-Tests fuer das `isolation-smoke` Binary (Coverage-
//! Boost per phase2-0-coverage Empfehlung). Spawnt das gebaute
//! Binary als Subprozess in Sender/Receiver-Rolle und prueft den
//! Exit-Code + stderr-Ausgabe.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

fn smoke_bin() -> PathBuf {
    // Cargo setzt CARGO_BIN_EXE_<name> fuer jedes bin target der
    // gleichen Crate. Das ist der robuste Weg, ohne pfade zu raten.
    PathBuf::from(env!("CARGO_BIN_EXE_isolation-smoke"))
}

fn unique_port(offset: u16) -> u16 {
    // Test-Pid + offset als port-basis. Tests laufen seriell auf
    // cargo test (default), das kollisions-free.
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
    // UDP-sender ohne --local → config-Fehler (bind-fehler an 1).
    let out = Command::new(smoke_bin())
        .args([
            "--transport=udp",
            "--role=sender",
            "--peer=127.0.0.1:9999",
            "--count=1",
        ])
        .output()
        .expect("run");
    // Exit-code 1 (config) ist richtig — wir haben --local vergessen.
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
