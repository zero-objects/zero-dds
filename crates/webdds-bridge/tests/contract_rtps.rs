// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-webdds-bridge` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/types_registry.rs`): it cuts
//! the first 12 octets of a discovered endpoint GUID and wraps them as
//! `wire_types::GuidPrefix(..)` to address a TypeLookup request to that
//! participant. The contract is that the prefix is exactly the first 12 GUID
//! octets. Oracle: DDSI-RTPS 2.5 §9.3.1.5 (`GUID_t = {guidPrefix, entityId}`).

use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

#[test]
fn rtps_9_3_1_5_the_prefix_is_the_first_twelve_guid_octets() {
    let endpoint = Guid::new(
        GuidPrefix([0x01, 0xF0, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]),
        EntityId::user_writer_with_key([0, 0, 1]),
    );
    let bytes = endpoint.to_bytes();
    let mut cut = [0_u8; 12];
    cut.copy_from_slice(&bytes[..12]);
    assert_eq!(GuidPrefix(cut), endpoint.prefix);
    assert_eq!(bytes[12..], [0, 0, 1, 0x02], "entityKey then entityKind");
}
