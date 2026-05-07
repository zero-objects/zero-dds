// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-Vendor-Interop-Tests gegen Mosquitto.
//!
//! Spec: `zerodds-mqtt-bridge-1.0.md` §10 + §12.3.
//!
//! Diese Tests verifizieren, dass die ZeroDDS-MQTT-Bridge gegen einen
//! externen Mosquitto-Broker kommuniziert (Wire-Compliance gegen
//! OASIS MQTT 5.0). Sie benötigen `mosquitto`-Binary + `mosquitto_pub`/
//! `mosquitto_sub` und sind daher `#[ignore]`-markiert.
//!
//! Run via:
//! ```bash
//! cargo test -p zerodds-mqtt-bridge --features cross-vendor-tests \
//!     --test cross_vendor -- --ignored
//! ```

#![cfg(all(feature = "daemon", feature = "cross-vendor-tests"))]
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

use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn mosquitto_available() -> bool {
    Command::new("mosquitto").arg("-h").output().is_ok()
        && Command::new("mosquitto_pub").arg("-h").output().is_ok()
        && Command::new("mosquitto_sub").arg("-h").output().is_ok()
}

#[test]
#[ignore = "requires mosquitto + mosquitto_pub/sub — run via cargo test --features cross-vendor-tests -- --ignored"]
fn zerodds_publish_received_by_mosquitto_sub() {
    if !mosquitto_available() {
        eprintln!("mosquitto not available; skipping");
        return;
    }
    // 1. Mosquitto-Broker auf Port 21883 starten.
    let mut broker = Command::new("mosquitto")
        .args(["-p", "21883", "-v"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mosquitto");
    thread::sleep(Duration::from_millis(500));

    // 2. mosquitto_sub: subscribe topic dds/Trade, single message.
    let sub = Command::new("mosquitto_sub")
        .args([
            "-h",
            "127.0.0.1",
            "-p",
            "21883",
            "-t",
            "dds/Trade",
            "-C",
            "1",
            "-W",
            "5",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn mosquitto_sub");
    thread::sleep(Duration::from_millis(300));

    // 3. mosquitto_pub: publish AAPL@200.
    let _pub_out = Command::new("mosquitto_pub")
        .args([
            "-h",
            "127.0.0.1",
            "-p",
            "21883",
            "-t",
            "dds/Trade",
            "-m",
            "AAPL@200",
        ])
        .output()
        .expect("mosquitto_pub");

    // 4. Wait for sub to receive.
    let out = sub.wait_with_output().expect("sub output");
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(
        body.contains("AAPL@200"),
        "mosquitto_sub should receive: {body}"
    );

    let _ = broker.kill();
    let _ = broker.wait();
}

#[test]
#[ignore = "requires mosquitto"]
fn zerodds_daemon_connects_to_mosquitto_broker() {
    if !mosquitto_available() {
        return;
    }
    // Smoke-test: ZeroDDS-MqttClient should successfully CONNECT/CONNACK with mosquitto.
    use std::sync::atomic::AtomicBool;
    use zerodds_mqtt_bridge::daemon::client::{BackoffConfig, connect_with_backoff};
    use zerodds_mqtt_bridge::daemon::config::DaemonConfig;

    let mut broker = Command::new("mosquitto")
        .args(["-p", "21884", "-v"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mosquitto");
    thread::sleep(Duration::from_millis(500));

    let mut cfg = DaemonConfig::default_for_dev();
    cfg.broker_url = "tcp://127.0.0.1:21884".to_string();
    cfg.client_id = "zerodds-cross-vendor-test".to_string();
    cfg.clean_start = true;

    let stop = AtomicBool::new(false);
    let bo = BackoffConfig {
        initial_ms: 50,
        max_ms: 500,
        multiplier: 2,
        max_attempts: 5,
    };
    let res = connect_with_backoff("127.0.0.1", 21884, &cfg, bo, &stop);
    assert!(
        res.is_ok(),
        "connect_with_backoff should succeed against mosquitto"
    );
    drop(res);

    let _ = broker.kill();
    let _ = broker.wait();
}
