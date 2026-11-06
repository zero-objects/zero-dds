// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-cli-common` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/lib.rs`):
//! `wire_types::GuidPrefix::zerodds(host_id, unique)` in `stable_prefix`, the
//! public octets `GuidPrefix.0` in `participant_guid`, and the same-host test
//! `GuidPrefix::is_same_host` its documentation relies on. Oracle:
//! DDSI-RTPS 2.5 §9.3.1.5 (vendor id in the first two prefix octets) and
//! Table 9.2 (`ENTITYID_PARTICIPANT = {{00,00,01},c1}`).

use zerodds_rtps::wire_types::{EntityId, GuidPrefix, VendorId};

#[test]
fn rtps_9_3_1_5_a_tool_prefix_carries_the_vendor_id_and_the_host_id() {
    let prefix = zerodds_cli_common::stable_prefix(0xFE);
    assert_eq!(prefix.0[..2], VendorId::ZERODDS.to_bytes());
    assert_eq!(prefix.0[10], 0xFE, "the tool marker");
    // Two tools on one host are same-host peers of each other.
    assert!(prefix.is_same_host(zerodds_cli_common::stable_prefix(0xFD)));
    let host = prefix.host_id();
    let elsewhere = GuidPrefix::zerodds([!host[0], host[1], host[2], host[3]], [0; 6]);
    assert!(!prefix.is_same_host(elsewhere));
}

#[test]
fn rtps_9_3_1_3_the_participant_guid_ends_in_entityid_participant() {
    let prefix = GuidPrefix::zerodds([1, 2, 3, 4], [5, 6, 7, 8, 9, 10]);
    let guid = zerodds_cli_common::participant_guid(prefix);
    assert_eq!(guid[..12], prefix.0);
    assert_eq!(guid[12..], EntityId::PARTICIPANT.to_bytes());
    assert_eq!(EntityId::PARTICIPANT.to_bytes(), [0x00, 0x00, 0x01, 0xc1]);
}
