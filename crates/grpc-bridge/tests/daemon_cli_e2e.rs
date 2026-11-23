// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-grpc-bridged` §2: the options and the exit codes of the binary.
//!
//! 0 for `--version`, `--help` and a valid `--check-config`; 1 for a
//! configuration error; 2 when the listener does not bind; 3 when the DDS
//! participant does not start; 4 when the TLS material does not load. The
//! short forms are `--set` of their file key and win over the file.

#![cfg(all(unix, feature = "std"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::path::PathBuf;
use std::process::{Command, Output};

fn temp_dir(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-grpcd-cli-{name}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The binary with `args`, its environment cleared of a system file: each
/// test names its own `--config`.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
        .args(args)
        .output()
        .unwrap()
}

fn text(out: &Output) -> String {
    format!(
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// A configuration file of the test.
fn file(name: &str, body: &str) -> PathBuf {
    let path = temp_dir(name).join("grpc-bridged.yaml");
    std::fs::write(&path, body).unwrap();
    path
}

/// §2: `--version` and `--help` print and exit 0.
#[test]
fn grpcbridge_2_version_and_help_exit_0() {
    let version = run(&["--version"]);
    assert_eq!(version.status.code(), Some(0), "{}", text(&version));
    assert!(
        String::from_utf8_lossy(&version.stdout)
            .starts_with(concat!("zerodds-grpc-bridged ", env!("CARGO_PKG_VERSION"))),
        "{}",
        text(&version)
    );
    let help = run(&["--help"]);
    assert_eq!(help.status.code(), Some(0), "{}", text(&help));
    let usage = String::from_utf8_lossy(&help.stderr);
    for option in [
        "--config",
        "--set",
        "--check-config",
        "--bind",
        "--domain",
        "--tls-cert",
        "--tls-key",
        "--tls-client-ca",
        "--reflection",
        "--topic",
        "--log-level",
        "--metrics",
        "--version",
        "--help",
    ] {
        assert!(usage.contains(option), "{option} not in the usage: {usage}");
    }
}

/// §2 exit 1: an unknown argument, a file that does not parse, a key the
/// daemon refuses, a value outside its range, `--tls-cert` without
/// `--tls-key`.
#[test]
fn grpcbridge_2_exit_1_on_an_unknown_argument_and_an_invalid_file() {
    let out = run(&["--no-such-option"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("unknown argument: --no-such-option"));
    for (name, body, expect) in [
        ("syntax", "grpc: [unclosed\n", "config"),
        ("key", "grpc:\n  bnd: \"127.0.0.1:0\"\n", "grpc.bnd"),
        (
            "range",
            "grpc:\n  max_frame_size: 1\n",
            "grpc.max_frame_size",
        ),
        ("type", "domain: seven\n", "domain"),
    ] {
        let path = file(name, body);
        let out = run(&["--config", path.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(1), "{name}: {}", text(&out));
        assert!(text(&out).contains(expect), "{name}: {}", text(&out));
    }
    let out = run(&["--tls-cert", "/nowhere.pem"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("--tls-cert needs --tls-key"));
}

/// §2, §7.2, §1 L6: settings that the start refuses as a configuration
/// error (exit 1) are refused by `--check-config` too, with exit 1 —
/// `auth.mode: bearer` without a token, `mtls` without TLS or without a
/// client CA, tenants without TLS (with the file and line of `tenants`),
/// cleartext h2c on a bind that is not loopback (§4.1), a `metrics.listen`
/// that is no IP address and port (§8.2), from the file and from
/// `--metrics`.
#[test]
fn grpcbridge_2_check_config_refuses_what_the_start_refuses() {
    let tenants = "tenants:\n  - name: t\n    subjects: [\"CN=t\"]\n";
    for (name, body, expect) in [
        (
            "bearer",
            "auth:\n  mode: bearer\n",
            "auth.mode: bearer needs auth.bearer_token",
        ),
        ("mtls", "auth:\n  mode: mtls\n", "grpc.tls.client_ca_file"),
        (
            "mtls-no-ca",
            "grpc:\n  tls:\n    enabled: true\n    cert_file: c.pem\n    key_file: k.pem\nauth:\n  mode: mtls\n",
            "grpc.tls.client_ca_file",
        ),
        ("tenants", tenants, "grpc-bridged.yaml:"),
        // §4.1: cleartext h2c only on a loopback bind.
        ("h2c", "grpc:\n  bind: \"0.0.0.0:0\"\n", "loopback"),
        // §8.2: the admin listener is an IP address and port.
        (
            "metrics-host",
            "metrics:\n  enabled: true\n  listen: \"localhost:9095\"\n",
            "metrics.listen",
        ),
    ] {
        let path = file(name, body);
        let checked = run(&["--config", path.to_str().unwrap(), "--check-config"]);
        assert_eq!(checked.status.code(), Some(1), "{name}: {}", text(&checked));
        assert!(
            text(&checked).contains(expect),
            "{name}: {}",
            text(&checked)
        );
        assert!(
            text(&checked).contains("configuration invalid"),
            "{name}: {}",
            text(&checked)
        );
        let bind = if name == "h2c" {
            "0.0.0.0:0"
        } else {
            "127.0.0.1:0"
        };
        let started = run(&["--config", path.to_str().unwrap(), "--bind", bind]);
        assert_eq!(started.status.code(), Some(1), "{name}: {}", text(&started));
        assert!(
            text(&started).contains(expect),
            "{name}: {}",
            text(&started)
        );
    }
    // `--metrics` is `metrics.listen` from the command line.
    let checked = run(&["--metrics", "localhost:9095", "--check-config"]);
    assert_eq!(checked.status.code(), Some(1), "{}", text(&checked));
    assert!(
        text(&checked).contains("`localhost:9095` is no IP address and port"),
        "{}",
        text(&checked)
    );
    // A bearer token makes the same file valid.
    let path = file(
        "bearer-ok",
        "grpc:\n  bind: \"127.0.0.1:0\"\nauth:\n  mode: bearer\n  bearer_token: tk\n",
    );
    let checked = run(&["--config", path.to_str().unwrap(), "--check-config"]);
    assert_eq!(checked.status.code(), Some(0), "{}", text(&checked));
}

/// §2 exit 2: the listener's port is taken.
#[test]
fn grpcbridge_2_exit_2_when_the_port_is_in_use() {
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = taken.local_addr().unwrap().to_string();
    let domain = zerodds_test_domains::lease_domain();
    let path = file("bind", "topics: []\n");
    let out = run(&[
        "--config",
        path.to_str().unwrap(),
        "--bind",
        &addr,
        "--domain",
        &domain.id().to_string(),
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains(&format!("bind {addr}")),
        "{}",
        text(&out)
    );
}

/// §2 exit 3: a DDS participant that cannot start (domain 233 lies beyond
/// the UDP ports of DDSI-RTPS §9.6.1.1); also a tenant's (§1 L6).
#[test]
fn grpcbridge_2_exit_3_when_the_participant_does_not_start() {
    let path = file("dds", "topics: []\n");
    let out = run(&[
        "--config",
        path.to_str().unwrap(),
        "--bind",
        "127.0.0.1:0",
        "--domain",
        "233",
    ]);
    assert_eq!(out.status.code(), Some(3), "{}", text(&out));
    let domain = zerodds_test_domains::lease_domain();
    // Tenants need TLS with a client CA (§1 L6); the material loads.
    let dir = temp_dir("tenant-pki");
    let mut ca = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let ca = ca.self_signed(&ca_key).unwrap();
    let key = rcgen::KeyPair::generate().unwrap();
    let leaf = rcgen::CertificateParams::new(vec!["localhost".to_string()])
        .unwrap()
        .signed_by(&key, &ca, &ca_key)
        .unwrap();
    for (name, pem) in [
        ("ca.pem", ca.pem()),
        ("server.pem", leaf.pem()),
        ("server.key", key.serialize_pem()),
    ] {
        std::fs::write(dir.join(name), pem).unwrap();
    }
    let path = file(
        "tenant-dds",
        &format!(
            "grpc:\n  tls:\n    enabled: true\n    cert_file: \"{0}/server.pem\"\n    key_file: \"{0}/server.key\"\n    client_ca_file: \"{0}/ca.pem\"\ntenants:\n  - name: far\n    domain: 233\n    subjects: [\"CN=far\"]\n",
            dir.display()
        ),
    );
    let out = run(&[
        "--config",
        path.to_str().unwrap(),
        "--bind",
        "127.0.0.1:0",
        "--domain",
        &domain.id().to_string(),
    ]);
    assert_eq!(out.status.code(), Some(3), "{}", text(&out));
}

/// §2 exit 4: TLS material that does not load — a missing file, a file
/// that holds no certificate.
#[test]
fn grpcbridge_2_exit_4_when_the_tls_material_does_not_load() {
    let dir = temp_dir("tls");
    let garbage = dir.join("garbage.pem");
    std::fs::write(&garbage, "not a certificate\n").unwrap();
    let missing = dir.join("missing.pem");
    for (cert, key) in [(&missing, &missing), (&garbage, &garbage)] {
        let out = run(&[
            "--bind",
            "127.0.0.1:0",
            "--tls-cert",
            cert.to_str().unwrap(),
            "--tls-key",
            key.to_str().unwrap(),
            "--config",
            file("tls-empty", "topics: []\n").to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(4), "{}", text(&out));
        assert!(text(&out).contains("tls:"), "{}", text(&out));
    }
}

/// §2: each short form is `--set` of its file key, wins over the file and
/// names itself as the origin under `--check-config` (exit 0).
#[test]
fn grpcbridge_2_each_short_form_wins_over_the_file() {
    let path = file(
        "precedence",
        "domain: 7\nlog_level: warn\ngrpc:\n  bind: \"127.0.0.1:1\"\n  reflection:\n    enabled: false\n  tls:\n    client_ca_file: /file/ca.pem\nmetrics:\n  enabled: false\n  listen: \"127.0.0.1:3\"\n",
    );
    let out = run(&[
        "--config",
        path.to_str().unwrap(),
        "--domain",
        "3",
        "--bind",
        "127.0.0.1:2",
        "--log-level",
        "debug",
        "--reflection",
        "--metrics",
        "127.0.0.1:4",
        "--tls-cert",
        "/flag/cert.pem",
        "--tls-key",
        "/flag/key.pem",
        "--tls-client-ca",
        "/flag/ca.pem",
        "--topic",
        "A::B=BStream",
        "--check-config",
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let report = String::from_utf8_lossy(&out.stdout);
    for (key, value) in [
        ("domain", "3"),
        ("grpc.bind", "127.0.0.1:2"),
        ("log_level", "debug"),
        ("grpc.reflection.enabled", "true"),
        ("metrics.listen", "127.0.0.1:4"),
        ("metrics.enabled", "true"),
        ("grpc.tls.cert_file", "/flag/cert.pem"),
        ("grpc.tls.key_file", "/flag/key.pem"),
        ("grpc.tls.enabled", "true"),
        ("grpc.tls.client_ca_file", "/flag/ca.pem"),
    ] {
        let line = format!("  {key} = {value}  (command line `--set {key}={value}`)");
        assert!(report.contains(&line), "{line}\n{report}");
    }
    assert!(
        report.contains("topics[0] = A::B=BStream  (command line `--set +topics=A::B=BStream`)"),
        "{report}"
    );
    assert!(report.contains("configuration valid"), "{report}");
}
