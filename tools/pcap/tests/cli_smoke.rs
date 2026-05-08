// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_zerodds-pcap")
}

fn write_synthetic_pcap(path: &std::path::Path, with_rtps: bool) {
    let mut buf = Vec::new();
    // pcap file header (LE magic, snaplen 65535, linktype 1=ETH)
    buf.extend_from_slice(&0xA1B2_C3D4u32.to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&4u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&65535u32.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());

    // one record
    let mut rec = vec![0u8; 42]; // dummy ethernet+ip+udp header
    if with_rtps {
        rec.extend_from_slice(b"RTPS\x02\x05\x01\x10");
        rec.extend_from_slice(&[0u8; 12]); // guid prefix
        // No submessages — Header-only datagram is valid per spec.
    }
    buf.extend_from_slice(&0u32.to_le_bytes()); // ts_sec
    buf.extend_from_slice(&0u32.to_le_bytes()); // ts_usec
    buf.extend_from_slice(&(rec.len() as u32).to_le_bytes()); // incl
    buf.extend_from_slice(&(rec.len() as u32).to_le_bytes()); // orig
    buf.extend_from_slice(&rec);

    std::fs::write(path, buf).unwrap();
}

#[test]
fn help_exits_zero() {
    let out = Command::new(bin()).arg("--help").output().expect("spawn");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("zerodds-pcap"));
}

#[test]
fn version_exits_zero() {
    let out = Command::new(bin())
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success());
}

#[test]
fn no_args_exits_two() {
    let out = Command::new(bin()).output().expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn missing_file_exits_two() {
    let out = Command::new(bin()).arg("parse").output().expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unknown_subcommand_exits_two() {
    let out = Command::new(bin())
        .arg("frobnicate")
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn parse_nonexistent_file_exits_three() {
    let out = Command::new(bin())
        .args(["parse", "/nonexistent/file.pcap"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn stats_nonexistent_file_exits_three() {
    let out = Command::new(bin())
        .args(["stats", "/nonexistent/file.pcap"])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn parse_synthetic_pcap_with_rtps() {
    let path = std::env::temp_dir().join(format!("zds-pcap-test-{}-with.pcap", std::process::id()));
    write_synthetic_pcap(&path, true);
    let out = Command::new(bin())
        .args(["parse"])
        .arg(&path)
        .output()
        .expect("spawn");
    let _ = std::fs::remove_file(&path);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("rtps=1"), "stdout was: {stdout}");
    assert!(stdout.contains("guid_prefix="));
}

#[test]
fn parse_synthetic_pcap_without_rtps() {
    let path = std::env::temp_dir().join(format!("zds-pcap-test-{}-bare.pcap", std::process::id()));
    write_synthetic_pcap(&path, false);
    let out = Command::new(bin())
        .args(["parse"])
        .arg(&path)
        .output()
        .expect("spawn");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rtps=0"));
}

#[test]
fn stats_synthetic_pcap_runs() {
    let path =
        std::env::temp_dir().join(format!("zds-pcap-test-{}-stats.pcap", std::process::id()));
    write_synthetic_pcap(&path, true);
    let out = Command::new(bin())
        .args(["stats"])
        .arg(&path)
        .output()
        .expect("spawn");
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("frames total:"));
    assert!(stdout.contains("frames w/ RTPS:"));
}
