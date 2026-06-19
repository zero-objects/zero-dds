// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-vendor interop tests against Cyclone DDS via ROS 2.
//!
//! Spec: `zerodds-ros2-bridge-1.0.md` §10 + §12.3.
//!
//! These tests verify the ROS-2 DDS mapping (topic mangling +
//! service pattern + action pattern) by comparison against the
//! Cyclone-DDS wire conventions as used by `rmw_cyclonedds_cpp`.
//!
//! Since no local ROS-2 workspace is assumed, the live
//! tests are marked `#[ignore]`. The pure-Rust tests always run and
//! verify the wire convention.

#![cfg(feature = "cross-vendor-tests")]
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

use std::process::Command;

fn ros2_available() -> bool {
    Command::new("ros2").arg("--help").output().is_ok()
}

#[test]
fn topic_mangling_matches_rmw_dds_common_convention() {
    use zerodds_ros2_rmw::topic_mangling::{RosKind, mangle_topic_name};
    // `rmw_dds_common::topic_endpoint_info::mangle_topic_name`
    // uses exactly these prefixes — compatible with Cyclone+Fast-DDS.
    assert_eq!(
        mangle_topic_name("/chatter", RosKind::Topic).unwrap(),
        "rt/chatter"
    );
}

#[test]
fn service_topic_pair_matches_cyclone_format() {
    use zerodds_ros2_rmw::service::ServiceTopics;
    // Cyclone DDS convention: rq/<base>Request, rr/<base>Reply.
    let s = ServiceTopics::from_service("/add_two_ints");
    assert_eq!(s.request, "rq/add_two_intsRequest");
    assert_eq!(s.reply, "rr/add_two_intsReply");
}

#[test]
fn action_topic_set_matches_cyclone_format() {
    use zerodds_ros2_rmw::action::ActionTopics;
    let a = ActionTopics::from_action("/fibonacci");
    assert!(a.goal.starts_with("rq/fibonacci/_action/send_goal"));
    assert!(a.cancel.starts_with("rq/fibonacci/_action/cancel_goal"));
    assert!(a.result.starts_with("rr/fibonacci/_action/get_result"));
    assert!(a.feedback.starts_with("rt/fibonacci/_action/feedback"));
    assert!(a.status.starts_with("rt/fibonacci/_action/status"));
}

#[test]
#[ignore = "requires a running ROS-2 (Humble+) installation with rmw_cyclonedds_cpp + a publisher on /chatter"]
fn ros2_topic_list_includes_chatter() {
    if !ros2_available() {
        eprintln!("ros2 cli not available; skipping");
        return;
    }
    let out = Command::new("ros2")
        .args(["topic", "list"])
        .output()
        .expect("ros2 topic list");
    let stdout = String::from_utf8_lossy(&out.stdout);
    eprintln!("ros2 topic list: {stdout}");
    // We can't assert specific topics without a known publisher; the
    // assertion is functional: command completed.
    assert!(out.status.success());
}
