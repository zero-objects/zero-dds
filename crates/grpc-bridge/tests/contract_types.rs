// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-grpc-bridge` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/bin/zerodds-grpc-bridged.rs`, feature `std`):
//! `qos::ExtensibilityForRepr`, the category it hands
//! `DcpsRuntime::set_user_writer_wire_extensibility` for a body whose
//! encapsulation header it passes on. It relies on the type having the three
//! categories of DDS-XTypes 1.3 Table 60 (FINAL, APPENDABLE, MUTABLE), each
//! with an encapsulation identifier under both encoding versions, and on
//! being a plain copyable value.
//!
//! Oracle: DDS-XTypes 1.3 §7.6.3.1.2, Table 60 (`CDR`, `PL_CDR`, `CDR2`,
//! `D_CDR2`, `PL_CDR2`, each in both byte orders).
#![allow(clippy::unwrap_used)]

use zerodds_types::qos::{
    DataRepresentationId, ExtensibilityForRepr, check_data_repr_extensibility,
};

#[test]
fn xtypes_7_6_3_1_2_each_category_has_an_identifier_under_both_versions() {
    for category in [
        ExtensibilityForRepr::Final,
        ExtensibilityForRepr::Appendable,
        ExtensibilityForRepr::Mutable,
    ] {
        let copied = category;
        assert_eq!(copied, category);
        for representation in [DataRepresentationId::Xcdr1, DataRepresentationId::Xcdr2] {
            assert_eq!(
                check_data_repr_extensibility(representation, category),
                Ok(()),
                "{representation:?} {category:?}"
            );
        }
    }
    assert_ne!(
        ExtensibilityForRepr::Final,
        ExtensibilityForRepr::Appendable
    );
    assert_ne!(
        ExtensibilityForRepr::Appendable,
        ExtensibilityForRepr::Mutable
    );
}
