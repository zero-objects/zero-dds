// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-dds-bridge` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/bridge_daemon.rs`, feature
//! `std`): `wire_types::EntityId` as the writer handle of `zerodds-dcps`,
//! kept by value next to a key in a `BTreeMap`, and
//! `publication_data::data_representation::XCDR`, the one representation it
//! offers for its writers (`set_user_writer_data_rep_override`): the CORBA
//! side speaks classic CDR. Oracle: DDS-XTypes 1.3 §7.6.3.1.1
//! (`XCDR_DATA_REPRESENTATION = 0`), DDSI-RTPS 2.5 §9.3.1.2.

use zerodds_rtps::publication_data::data_representation as dr;
use zerodds_rtps::wire_types::EntityId;

#[test]
fn xtypes_7_6_3_1_1_xcdr_is_representation_zero() {
    assert_eq!(dr::XCDR, 0);
    assert_ne!(dr::XCDR, dr::XCDR2);
}

#[test]
fn entity_id_is_a_plain_value_kept_beside_a_key() {
    fn handle<T: Copy + Eq + std::fmt::Debug + Send + Sync + 'static>() {}
    handle::<EntityId>();
    let writer = EntityId::user_writer_with_key([0, 1, 2]);
    let by_key = std::collections::BTreeMap::from([(vec![1_u8], (0_usize, writer))]);
    assert_eq!(by_key[&vec![1_u8]].1, writer);
    assert_eq!(
        writer.entity_kind.to_byte(),
        0x02,
        "DDSI-RTPS 2.5 Table 9.1"
    );
}
