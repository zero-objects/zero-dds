// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! zerodds-mqtt-bridge-1.0 §7.2 and §7.3 against Mosquitto: `password`
//! towards a broker with a password file (a wrong password and an anonymous
//! CONNECT are exit 5), `mtls` towards a broker that requires a client
//! certificate and takes its CN as the user name, and the ACL with the
//! certificate's DN as the subject.
//!
//! Needs `mosquitto`, `mosquitto_passwd`, `mosquitto_sub`; a missing binary
//! fails the test instead of passing it.
//!
//! ```bash
//! cargo test -p zerodds-mqtt-bridge --features cross-vendor-tests \
//!     --test mosquitto_auth -- --ignored --test-threads=1
//! ```

#![cfg(all(feature = "daemon", feature = "cross-vendor-tests"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_mqtt_bridge::daemon::acl::AclRule;
use zerodds_mqtt_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_mqtt_bridge::daemon::metrics::family;
use zerodds_mqtt_bridge::daemon::server;

const BIN: &str = env!("CARGO_BIN_EXE_zerodds-mqtt-bridged");

fn require(bins: &[&str]) {
    for bin in bins {
        assert!(
            Command::new(bin).arg("--help").output().is_ok(),
            "{bin} is not installed; this test measures against it"
        );
    }
}

fn workdir(leaf: &str) -> PathBuf {
    // Under the system temp directory and open to all: Mosquitto started as
    // root drops to its own user.
    let dir = std::env::temp_dir()
        .join(format!("zerodds-mqtt-auth-{}", std::process::id()))
        .join(leaf);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    use std::os::unix::fs::PermissionsExt;
    for d in [dir.parent().unwrap(), dir.as_path()] {
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o777)).unwrap();
    }
    dir
}

fn write_open(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).unwrap();
}

/// A port for the broker (Mosquitto takes it from its file), of
/// [`zerodds_test_domains::free_tcp_port`].
fn free_port() -> u16 {
    zerodds_test_domains::free_tcp_port()
}

/// A child process, killed and reaped when the test ends.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Mosquitto with `conf` (the listener lines and more) on `port`, logging
/// to `mosquitto.log` in `dir`.
fn start_broker(dir: &Path, port: u16, conf: &str) -> (Reaped, PathBuf) {
    let log = dir.join("mosquitto.log");
    let file = dir.join("mosquitto.conf");
    write_open(
        &file,
        &format!("{conf}log_type all\nlog_dest file {}\n", log.display()),
    );
    let child = Command::new("mosquitto")
        .arg("-c")
        .arg(&file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mosquitto");
    let broker = Reaped(child);
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "mosquitto did not open {port}");
        thread::sleep(Duration::from_millis(50));
    }
    (broker, log)
}

/// The binary with `args` until it exits (at most 15 s): its exit code and
/// stderr.
fn run_to_exit(args: &[String]) -> (Option<i32>, String) {
    let mut child = Reaped(
        Command::new(BIN)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let t0 = Instant::now();
    let status = loop {
        if let Some(st) = child.0.try_wait().unwrap() {
            break st;
        }
        assert!(t0.elapsed() < Duration::from_secs(15), "still running");
        thread::sleep(Duration::from_millis(20));
    };
    let mut err = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    (status.code(), err)
}

fn daemon_args(dir: &Path, domain: i32, port: u16, rest: &[&str]) -> Vec<String> {
    let base = dir.join("mqtt-bridged.yaml");
    write_open(&base, "log_level: info\n");
    let mut out = vec![
        "--config".to_string(),
        base.display().to_string(),
        "--domain".to_string(),
        domain.to_string(),
        "--broker".to_string(),
        format!("mqtt://127.0.0.1:{port}"),
        "--set".to_string(),
        format!("mqtt.session_dir={}", dir.join("sessions").display()),
    ];
    out.extend(rest.iter().map(|s| (*s).to_string()));
    out
}

/// Spec §7.2 `password`, §2 exit 5, against Mosquitto with a password file
/// and no anonymous access: the right password connects, a wrong one and a
/// CONNECT without credentials are refused in the CONNACK (Mosquitto: 0x86
/// or 0x87), exit 5.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_7_2_a_wrong_password_is_exit_5_against_mosquitto() {
    require(&["mosquitto", "mosquitto_passwd"]);
    let dir = workdir("password");
    let passwd = dir.join("passwd");
    write_open(&passwd, "");
    let made = Command::new("mosquitto_passwd")
        .arg("-b")
        .arg(&passwd)
        .args(["bridge", "s3cret"])
        .status()
        .unwrap();
    assert!(made.success());
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&passwd, std::fs::Permissions::from_mode(0o644)).unwrap();
    let port = free_port();
    let (_broker, log) = start_broker(
        &dir,
        port,
        &format!(
            "listener {port} 127.0.0.1\nallow_anonymous false\npassword_file {}\n",
            passwd.display()
        ),
    );
    let lease = zerodds_test_domains::lease_domain();
    for (password, id) in [("wrong", "auth-wrong"), ("", "auth-anon")] {
        let mut rest = vec!["--client-id", id];
        if !password.is_empty() {
            rest.extend(["--username", "bridge", "--password", password]);
        }
        let (code, err) = run_to_exit(&daemon_args(&dir, lease.id(), port, &rest));
        assert_eq!(code, Some(5), "{id}: {err}");
        assert!(err.contains("0x86") || err.contains("0x87"), "{id}: {err}");
    }
    let mut good = Reaped(
        Command::new(BIN)
            .args(daemon_args(
                &dir,
                lease.id(),
                port,
                &[
                    "--client-id",
                    "auth-good",
                    "--username",
                    "bridge",
                    "--password",
                    "s3cret",
                ],
            ))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !connected_as(&log, "auth-good", "bridge") {
        assert!(
            Instant::now() < deadline,
            "the daemon did not connect:\n{}",
            std::fs::read_to_string(&log).unwrap_or_default()
        );
        assert!(good.0.try_wait().unwrap().is_none(), "the daemon ended");
        thread::sleep(Duration::from_millis(50));
    }
}

/// A CA and two certificates it signed: one for the broker (`localhost`)
/// and one for the bridge (`CN=bridge-01`), each written as PEM into `dir`.
fn certificates(dir: &Path) {
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut ca = rcgen::CertificateParams::new(Vec::new()).unwrap();
    ca.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca.distinguished_name
        .push(rcgen::DnType::CommonName, "zerodds test CA");
    let ca_cert = ca.self_signed(&ca_key).unwrap();
    write_open(&dir.join("ca.pem"), &ca_cert.pem());
    for (name, cn, sans) in [
        ("server", "localhost", vec!["localhost".to_string()]),
        ("client", "bridge-01", Vec::new()),
        ("watcher", "watcher", Vec::new()),
    ] {
        let key = rcgen::KeyPair::generate().unwrap();
        let mut p = rcgen::CertificateParams::new(sans).unwrap();
        p.distinguished_name = rcgen::DistinguishedName::new();
        p.distinguished_name.push(rcgen::DnType::CommonName, cn);
        let cert = p.signed_by(&key, &ca_cert, &ca_key).unwrap();
        write_open(&dir.join(format!("{name}.pem")), &cert.pem());
        write_open(&dir.join(format!("{name}.key")), &key.serialize_pem());
    }
}

/// Spec §7.2 `mtls` and §7.3 against Mosquitto with `require_certificate`
/// and `use_identity_as_username`: the daemon connects with its client
/// certificate alone (Mosquitto logs the CN `bridge-01` as the user name);
/// its subject is the DN `CN=bridge-01`, whose rule lets it publish
/// `plant/status` and nothing else: a `mosquitto_sub` with its own
/// certificate gets `plant/status` and not `plant/secret`.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_7_2_mtls_and_the_dn_subject_against_mosquitto() {
    require(&["mosquitto", "mosquitto_sub"]);
    let dir = workdir("mtls");
    certificates(&dir);
    let port = free_port();
    let (_broker, log) = start_broker(
        &dir,
        port,
        &format!(
            "listener {port} 127.0.0.1\ncafile {d}/ca.pem\ncertfile {d}/server.pem\nkeyfile {d}/server.key\nrequire_certificate true\nuse_identity_as_username true\nallow_anonymous false\n",
            d = dir.display()
        ),
    );
    let lease = zerodds_test_domains::lease_domain();
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.domain = lease.id();
    cfg.broker_url = format!("mqtts://127.0.0.1:{port}");
    cfg.client_id = "mtls-bridge".into();
    cfg.session_dir = dir.join("sessions").display().to_string();
    cfg.broker_tls_enabled = true;
    cfg.broker_tls_server_name = "localhost".into();
    cfg.broker_tls_ca_file = dir.join("ca.pem").display().to_string();
    cfg.broker_tls_client_cert_file = dir.join("client.pem").display().to_string();
    cfg.broker_tls_client_key_file = dir.join("client.key").display().to_string();
    cfg.auth_mode = zerodds_mqtt_bridge::daemon::auth::AuthKind::parse("mtls");
    cfg.acl_default_deny = Some(true);
    cfg.acl_rules = vec![AclRule {
        subject: "CN=bridge-01".into(),
        allow_publish: vec!["plant/status".into()],
        allow_subscribe: Vec::new(),
    }];
    for (dds, mqtt) in [("Status", "plant/status"), ("Secret", "plant/secret")] {
        cfg.topics.push(TopicConfig {
            dds_name: dds.into(),
            dds_type: dds.into(),
            mqtt_topic: mqtt.into(),
            direction: "out".into(),
            mqtt_qos: Some(1),
            reliability: "reliable".into(),
            durability: "volatile".into(),
            ..Default::default()
        });
    }
    let mut h = server::start(cfg).expect("the daemon connects with its certificate");
    assert!(
        connected_as(&log, "mtls-bridge", "bridge-01"),
        "{}",
        std::fs::read_to_string(&log).unwrap_or_default()
    );
    let watcher = Command::new("mosquitto_sub")
        .args(["-h", "localhost", "-p", &port.to_string(), "-v", "-W", "4"])
        .args(["-t", "plant/#", "--cafile"])
        .arg(dir.join("ca.pem"))
        .arg("--cert")
        .arg(dir.join("watcher.pem"))
        .arg("--key")
        .arg(dir.join("watcher.key"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(800));
    for name in ["Status", "Secret"] {
        let eid = h.user_readers[name];
        assert!(h.runtime.test_inject_user_alive(eid, b"reading".to_vec()));
    }
    let out = String::from_utf8_lossy(&watcher.wait_with_output().unwrap().stdout).into_owned();
    assert!(out.contains("plant/status"), "{out}");
    assert!(!out.contains("plant/secret"), "{out}");
    let m = h.metrics.clone().unwrap();
    assert_eq!(m.value(family::ACL_DENIALS, &[("reason", "publish")]), 1);
    h.shutdown();
}

/// Whether Mosquitto's log has a line of a new client `id` with the user
/// name `user` (`New client connected from … as <id> (…, u'<user>').`).
fn connected_as(log: &Path, id: &str, user: &str) -> bool {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .any(|l| {
            l.contains("New client connected")
                && l.contains(&format!(" as {id} ("))
                && l.contains(&format!("u'{user}'"))
        })
}
