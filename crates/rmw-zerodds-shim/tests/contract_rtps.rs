// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `rmw-zerodds-shim` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps`:
//! - `src/services.rs`: `Guid::new(prefix, entity_id).to_bytes()` for the
//!   16-octet GUIDs of its endpoints, `GuidPrefix::from_bytes` of the first
//!   12 octets of a peer GUID, `VendorId::CYCLONE` to pick the Cyclone form
//!   of a service request, `EntityId` handles, and
//!   `inline_qos::SampleIdentityBytes::new(guid, sequence)` for the related
//!   sample identity, with `0xFFFF_FFFF_0000_0000` where the sequence number
//!   is unknown;
//! - `src/spatial_bridge.rs`: `publication_data::data_representation::XCDR`
//!   and `XCDR2`.
//!
//! Oracle: DDSI-RTPS 2.5 §9.3.1.5 (GUID_t), §9.3.2.1
//! (SEQUENCENUMBER_UNKNOWN: high = -1, low = 0), DDS-RPC 1.0 §7.5.1.1.1,
//! the OMG vendor id of Eclipse Cyclone DDS (0x0110), DDS-XTypes 1.3
//! §7.6.3.1.1.

use zerodds_rtps::inline_qos::SampleIdentityBytes;
use zerodds_rtps::publication_data::data_representation as dr;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, SequenceNumber, VendorId};

#[test]
fn rtps_9_3_1_5_an_endpoint_guid_is_prefix_then_entity_id() {
    let prefix = GuidPrefix::zerodds([1, 2, 3, 4], [5, 6, 7, 8, 9, 10]);
    let writer = EntityId::user_writer_with_key([0, 0, 0x31]);
    let bytes = Guid::new(prefix, writer).to_bytes();
    assert_eq!(bytes[..12], prefix.0);
    assert_eq!(bytes[12..], [0, 0, 0x31, 0x02]);
    let mut cut = [0_u8; 12];
    cut.copy_from_slice(&bytes[..12]);
    assert_eq!(GuidPrefix::from_bytes(cut), prefix);
}

#[test]
fn rtps_9_3_2_1_an_unknown_related_sequence_number_goes_out_as_sequencenumber_unknown() {
    let related = SampleIdentityBytes::new([3; 16], 0xFFFF_FFFF_0000_0000);
    let wire = related.to_bytes(true);
    assert_eq!(wire[16..], SequenceNumber::UNKNOWN.to_bytes_le());
    assert_eq!(SequenceNumber::UNKNOWN.split(), (-1, 0));
}

#[test]
fn the_cyclone_vendor_id_is_0x0110() {
    assert_eq!(VendorId::CYCLONE.to_bytes(), [0x01, 0x10]);
    assert_ne!(VendorId::CYCLONE, VendorId::ZERODDS);
}

#[test]
fn xtypes_7_6_3_1_1_xcdr_and_xcdr2() {
    assert_eq!((dr::XCDR, dr::XCDR2), (0, 2));
}
