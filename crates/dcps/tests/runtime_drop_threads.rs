// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What is left running of a `DcpsRuntime` once the last handle its owner
//! holds has dropped, without `shutdown` and without a participant: nothing
//! (`DcpsRuntime::start`: the worker threads hold the runtime weakly, and
//! its `Drop` stops and joins them). When each worker held its own
//! `Arc<DcpsRuntime>`, a dropped handle stopped nothing and the threads ran
//! until the process ended. One test in this binary, so that the threads of
//! the process are the threads of this test.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

/// The threads of this process by kernel task id, with their names.
#[cfg(target_os = "linux")]
fn threads() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir("/proc/self/task").expect("/proc/self/task") {
        let path = entry.expect("a task").path();
        let tid = path.file_name().unwrap().to_string_lossy().into_owned();
        let name = std::fs::read_to_string(path.join("comm")).unwrap_or_default();
        out.push((tid, name.trim().to_string()));
    }
    out
}

#[cfg(target_os = "linux")]
#[test]
fn the_last_handle_of_a_runtime_ends_and_joins_its_threads() {
    use std::time::{Duration, Instant};
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserWriterConfig};
    use zerodds_rtps::wire_types::GuidPrefix;

    let lease = common::lease_domain();
    // By kernel task id: a thread names itself once it runs, so the
    // heartbeat thread the lease above starts could still carry the
    // process's name here and its own one later, and was counted as the
    // runtime's (pipeline 9187, job 231306: `("12430", "zerodds-domain-")`
    // still running after the drop).
    let before: Vec<String> = threads().into_iter().map(|(tid, _)| tid).collect();
    let cfg = RuntimeConfig {
        spdp_multicast_send: false,
        ..common::isolated_cfg()
    };
    let rt =
        DcpsRuntime::start(lease.id(), GuidPrefix::from_bytes([0xD7; 12]), cfg).expect("runtime");
    rt.register_user_writer(UserWriterConfig {
        topic_name: "RuntimeDropThreads".into(),
        type_name: "RawBytes".into(),
        ..Default::default()
    })
    .expect("writer");
    rt.register_user_reader(UserReaderConfig {
        topic_name: "RuntimeDropThreads".into(),
        type_name: "RawBytes".into(),
        reliable: true,
        ..Default::default()
    })
    .expect("reader");
    let started: Vec<(String, String)> = threads()
        .into_iter()
        .filter(|(tid, _)| !before.contains(tid))
        .collect();
    assert!(
        started
            .iter()
            .any(|(_, name)| name.starts_with("zdds-recv-spdp")),
        "the runtime's threads are among {started:?}"
    );

    let dropping = Instant::now();
    drop(rt);
    let took = dropping.elapsed();
    // The drop joins: on Linux closing the receive sockets wakes the
    // threads at once, so it does not wait out a read timeout.
    assert!(took < Duration::from_millis(500), "the drop took {took:?}");
    // A joined thread leaves the task list once the kernel has released
    // it, a moment after the join.
    let deadline = Instant::now() + Duration::from_secs(1);
    let left = loop {
        let now = threads();
        let left: Vec<(String, String)> = started
            .iter()
            .filter(|(tid, _)| now.iter().any(|(running, _)| running == tid))
            .cloned()
            .collect();
        if left.is_empty() || Instant::now() > deadline {
            break left;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(left.is_empty(), "still running after the drop: {left:?}");
}
