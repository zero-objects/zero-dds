// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rs` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/lib.rs`): the whole crate,
//! re-exported as `zerodds_rs::types` (`pub use zerodds_types as types`).
//! Oracle: the identity of the re-exported items (`TypeId`), and that the
//! default features of `zerodds-types` this crate's dependency keeps
//! (`std`, `dynamic`) reach the user through the re-export.

use core::any::TypeId;

#[test]
fn the_types_module_is_zerodds_types_itself() {
    assert_eq!(
        TypeId::of::<zerodds_rs::types::TypeIdentifier>(),
        TypeId::of::<zerodds_types::TypeIdentifier>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::types::TypeObject>(),
        TypeId::of::<zerodds_types::TypeObject>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::types::TypeInformation>(),
        TypeId::of::<zerodds_types::TypeInformation>()
    );
}

#[test]
fn the_dynamic_type_api_reaches_the_user_through_the_re_export() {
    // `dynamic` is a default feature of `zerodds-types`; this crate keeps
    // the defaults, so the module is part of what it re-exports.
    assert_eq!(
        TypeId::of::<zerodds_rs::types::dynamic::DynamicType>(),
        TypeId::of::<zerodds_types::dynamic::DynamicType>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::types::dynamic::DynamicData>(),
        TypeId::of::<zerodds_types::dynamic::DynamicData>()
    );
}
