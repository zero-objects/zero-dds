// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CosTime of ZeroDDS against omniORB 4.3 (OMG Time Service 1.1 §1.2,
//! §2.1, §1.3.4, §1.3.5). The omniORB client in `tests/omniorb/` is built
//! from the IDL of Appendix C.1 with omniidl and g++ and calls the
//! [`TimeServer`] of this process through `corbaloc`.
//!
//! Needs omniidl, g++ and libomniORB4; it runs in the integration job
//! `integration-bridges-vendor-corba` (ci/integration_scenarios.py). A
//! missing piece fails the test.
//!
//! ```bash
//! cargo test -p zerodds-time-service --features serve --test omniorb_e2e -- --ignored
//! ```

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_time_service::TimeService;
use zerodds_time_service::servant::RemoteTimePolicy;
use zerodds_time_service::server::TimeServer;

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/omniorb")
}

fn step(cmd: &mut Command) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
    assert!(
        out.status.success(),
        "{cmd:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// omniidl + g++: `time_client` in a fresh directory.
fn build() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("time-omniorb");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    step(
        Command::new("omniidl")
            .args(["-bcxx", "-C"])
            .arg(&dir)
            .arg(src().join("CosTime.idl")),
    );
    step(
        Command::new("g++")
            .arg("-std=c++17")
            .arg("-I")
            .arg(&dir)
            .arg(src().join("time_client.cc"))
            .arg(dir.join("CosTimeSK.cc"))
            .args(["-lomniORB4", "-lomnithread", "-o"])
            .arg(dir.join("time_client")),
    );
    dir
}

/// The output of the omniORB client against a server of `service` with
/// `policy`.
fn run_client(dir: &Path, service: TimeService, policy: RemoteTimePolicy) -> String {
    let srv = TimeServer::start_with_policy(
        service,
        "127.0.0.1:0".parse().unwrap(),
        String::from("127.0.0.1"),
        policy,
    )
    .unwrap();
    step(
        Command::new(dir.join("time_client"))
            .arg(srv.corbaloc("127.0.0.1"))
            .args(["-ORBclientCallTimeOutPeriod", "15000"]),
    )
}

/// The lines after `universal_time` and its attributes.
fn value_operation_lines() -> Vec<String> {
    vec![
        "secure_universal_time TimeUnavailable".to_string(),
        "new_universal_time BAD_PARAM".to_string(),
        "new_interval BAD_PARAM".to_string(),
        // MidC 1000 vs 2000: TCLessThan (1); IntervalC of a with itself
        // (inaccuracy 50): TCIndeterminate (3).
        "compare_time 1 3".to_string(),
        "time_to_interval 1000 2000".to_string(),
        // [1000, 2000] spans the UTO 1000 ± 50 partly: OTOverlap (2).
        "spans 2 1000 1050".to_string(),
        "time 1500 500".to_string(),
        "uto_from_utc 777 1 2 -60".to_string(),
    ]
}

#[test]
#[ignore = "needs omniidl, g++ and libomniORB4 (integration-bridges-vendor-corba)"]
fn time_1_2_an_omniorb_client_uses_the_time_service() {
    let dir = build();
    let tick = 138_000_000_000_000_000u64;
    let service = TimeService::with_source(move || tick)
        .with_inaccuracy(0x0002_0000_0005)
        .unwrap()
        .with_tdf(60);
    let out = run_client(&dir, service, RemoteTimePolicy::AnswerCorrectingProxies);
    let lines: Vec<&str> = out.lines().collect();
    // absolute_time of a UTO of time 0 carries the inaccuracy of that UTO.
    let mut expected = vec![
        format!("universal_time {tick} 5 2 60"),
        format!("attributes {tick} {} 60", 0x0002_0000_0005u64),
        format!("absolute_time {tick} 0 60"),
    ];
    expected.extend(value_operation_lines());
    assert_eq!(lines, expected, "client output:\n{out}");

    // App. B.2: by default the server gives an omniORB proxy no current
    // time, neither through universal_time nor through absolute_time
    // (CORBA::NO_PERMISSION); the operations without current time are
    // served.
    let out = run_client(
        &dir,
        TimeService::with_source(move || tick),
        RemoteTimePolicy::Refuse,
    );
    let lines: Vec<&str> = out.lines().collect();
    let mut expected = vec![
        "universal_time NO_PERMISSION".to_string(),
        "absolute_time NO_PERMISSION".to_string(),
    ];
    expected.extend(value_operation_lines());
    assert_eq!(lines, expected, "client output:\n{out}");
}
