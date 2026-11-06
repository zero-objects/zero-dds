// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-websocket-bridge` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/daemon/server.rs`, feature
//! `daemon`): `wire_types::EntityId` as the handle `zerodds-dcps` returns for
//! a user writer or reader, kept by value in a `BTreeMap<String, EntityId>` behind an
//! `Arc` the connection threads share, and handed back to the runtime to write.
//! The contract is a plain, thread-safe value whose writer and reader
//! handles stay apart. Oracle: DDSI-RTPS 2.5 §9.3.1.2, Table 9.1.

use zerodds_rtps::wire_types::{EntityId, EntityKind};

#[test]
fn entity_id_is_a_plain_value_threads_can_share() {
    fn shared_handle<T: Copy + Eq + Ord + std::fmt::Debug + Send + Sync + 'static>() {}
    shared_handle::<EntityId>();
    let writer = EntityId::user_writer_with_key([0, 0, 7]);
    let kept = std::sync::Arc::new(std::collections::BTreeMap::from([(
        "t".to_string(),
        writer,
    )]));
    let seen = std::thread::spawn({
        let kept = std::sync::Arc::clone(&kept);
        move || kept["t"]
    })
    .join()
    .unwrap_or(EntityId::UNKNOWN);
    assert_eq!(seen, writer);
}

#[test]
fn rtps_9_3_1_2_writer_and_reader_handles_of_one_key_differ() {
    let key = [0, 0, 7];
    assert_eq!(
        EntityId::user_writer_with_key(key).entity_kind.to_byte(),
        0x02
    );
    assert_eq!(
        EntityId::user_reader_with_key(key).entity_kind.to_byte(),
        0x07
    );
    assert_ne!(
        EntityId::user_writer_with_key(key),
        EntityId::user_reader_with_key(key)
    );
    assert_eq!(
        EntityId::user_writer_no_key(key).entity_kind,
        EntityKind::UserWriterNoKey
    );
}
