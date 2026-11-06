// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-replay` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-rtps` (`src/main.rs`, `InjectState`):
//! `wire_types::EntityId` as the handle of the writer it registers per
//! recorded topic, kept in a map next to the identifier it last set.

use zerodds_rtps::wire_types::EntityId;

#[test]
fn entity_id_is_a_plain_handle() {
    fn handle<T: Copy + Eq + std::fmt::Debug + Send + Sync + 'static>() {}
    handle::<EntityId>();
}
