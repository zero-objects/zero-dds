// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-rtps` (`src/main.rs`): the type
//! `wire_types::GuidPrefix` of the prefix it takes from
//! `zerodds_cli_common::stable_prefix` and hands to `DcpsRuntime::start`.
//! The contract is that the prefix is a 12-octet value that names the vendor
//! in its first two octets. Oracle: DDSI-RTPS 2.5 §9.3.1.1, §9.3.1.5.

use zerodds_rtps::wire_types::{GuidPrefix, VendorId};

#[test]
fn rtps_9_3_1_1_a_prefix_is_twelve_octets_and_names_the_vendor() {
    assert_eq!(GuidPrefix::WIRE_SIZE, 12);
    let prefix = GuidPrefix::zerodds([9, 9, 9, 9], [1, 2, 3, 4, 5, 6]);
    assert_eq!(prefix.to_bytes().len(), 12);
    assert_eq!(prefix.0[..2], VendorId::ZERODDS.to_bytes());
    assert_eq!(GuidPrefix::from_bytes(prefix.to_bytes()), prefix);
    assert_eq!(GuidPrefix::UNKNOWN.0, [0; 12], "GUIDPREFIX_UNKNOWN");
}
