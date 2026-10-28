// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The runtime defaults the public pages state as numbers.
//!
//! `documentation/01-getting-started/` and the "Why ZeroDDS for ROS" pages
//! (`internal/why-zerodds-for-ros/`, rendered under
//! `website/why-zerodds-for-ros/`) name the reassembly cap and the
//! representation offer of `RuntimeConfig::ros_defaults()` as facts. Their
//! reviews in `internal/documentation/doc-review-decisions.json` cite this
//! file, the settings page `man/man5/zerodds.yaml.5` and
//! `runtime_config_layering.rs` as evidence, so a change to one of these
//! values fails here and sends the pages back to review.

#![allow(clippy::expect_used)]

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_rtps::publication_data::data_representation as dr;

/// "ZeroDDS's reassembly cap is 16 MiB by default (configurable via
/// `reader.max_reassembly_sample_bytes`)" — `large-data.md`; the settings
/// page states the same default.
#[test]
fn the_reassembly_cap_is_16_mib_by_default_and_the_settings_page_says_so() {
    assert_eq!(
        RuntimeConfig::default().max_reassembly_sample_bytes,
        16 * 1024 * 1024
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let page = std::fs::read_to_string(root.join("man/man5/zerodds.yaml.5")).expect("the page");
    let entry = page
        .split_once(".B reader.max_reassembly_sample_bytes\n")
        .map(|(_, rest)| rest.lines().next().unwrap_or_default())
        .expect("the page documents reader.max_reassembly_sample_bytes");
    assert!(
        entry.contains("Default 16777216."),
        "the settings page states another default: {entry}"
    );
}

/// "A single `RuntimeConfig::ros_defaults()` sets the representation offers
/// (XCDR1 + XCDR2) and the 16 MiB reassembly cap" — `config-complexity.md`.
#[test]
fn ros_defaults_offer_xcdr1_and_xcdr2_and_keep_the_16_mib_cap() {
    let ros = RuntimeConfig::ros_defaults();
    assert_eq!(ros.data_representation_offer, vec![dr::XCDR, dr::XCDR2]);
    assert_eq!(ros.max_reassembly_sample_bytes, 16 * 1024 * 1024);
}
