//! Runs the test binary of `liveliness_qos.rs` again and again and counts the
//! failures (the rate of failure of a test that was red now and then in the
//! rounds). `#[ignore]`: it is a measurement, not a test of the crate. Run
//! `cargo test -p zerodds-dcps --test liveliness_qos --test repeat_liveliness_qos
//! -- --ignored --nocapture`; ten runs per test function keep one call within
//! the time budget.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    missing_docs
)]

use std::path::PathBuf;
use std::process::Command;

/// The binary of `liveliness_qos.rs` that cargo built beside this one.
fn target_binary() -> PathBuf {
    let exe = std::env::current_exe().expect("the path of this test binary");
    let dir = exe.parent().expect("the deps directory");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).expect("the deps directory") {
        let path = entry.expect("an entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("liveliness_qos-") || name.contains('.') {
            continue;
        }
        let modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .expect("mtime");
        if best.as_ref().is_none_or(|(at, _)| modified > *at) {
            best = Some((modified, path));
        }
    }
    best.expect("build `--test liveliness_qos` together with this test")
        .1
}

/// `runs` runs of the binary; the failed test names of each run are printed.
fn repeat(first: usize, runs: usize) {
    let binary = target_binary();
    let mut failed_runs = 0;
    for run in first..first + runs {
        let started = std::time::Instant::now();
        let output = Command::new(&binary)
            .output()
            .expect("the test binary starts");
        let text = String::from_utf8_lossy(&output.stdout);
        let failed: Vec<&str> = text
            .lines()
            .filter(|line| line.starts_with("test ") && line.ends_with("FAILED"))
            .collect();
        let summary = text
            .lines()
            .find(|line| line.starts_with("test result"))
            .unwrap_or("no result line");
        println!(
            "run {run:02}: {} in {:?}: {summary}",
            if output.status.success() {
                "ok"
            } else {
                "FAILED"
            },
            started.elapsed()
        );
        if !output.status.success() {
            failed_runs += 1;
            println!("{text}");
            println!("{}", String::from_utf8_lossy(&output.stderr));
            for line in failed {
                println!("  {line}");
            }
        }
    }
    println!(
        "runs {first}..{}: {failed_runs} failed of {runs}",
        first + runs - 1
    );
    assert_eq!(failed_runs, 0, "{failed_runs} of {runs} runs failed");
}

#[test]
#[ignore = "a measurement: ten runs of liveliness_qos"]
fn runs_01_to_10() {
    repeat(1, 10);
}

#[test]
#[ignore = "a measurement: ten runs of liveliness_qos"]
fn runs_11_to_20() {
    repeat(11, 10);
}

#[test]
#[ignore = "a measurement: ten runs of liveliness_qos"]
fn runs_21_to_30() {
    repeat(21, 10);
}
