// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-websocket-bridge` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/daemon/server.rs`, feature `daemon`):
//! `qos::ExtensibilityForRepr`, the category it hands
//! `DcpsRuntime::set_user_writer_wire_extensibility` for a body whose
//! encapsulation header it passes on. It relies on the type having the three
//! categories of DDS-XTypes 1.3 Table 60 (FINAL, APPENDABLE, MUTABLE), each
//! with an encapsulation identifier under both encoding versions, and on
//! being a plain copyable value.
//!
//! Oracle: DDS-XTypes 1.3 §7.6.3.1.2, Table 60 (`CDR`, `PL_CDR`, `CDR2`,
//! `D_CDR2`, `PL_CDR2`, each in both byte orders).
//!
//! For the JSON text frames (`src/daemon/json_frames.rs`) it uses
//! `dynamic::type_::DynamicType`: `find` a type by scoped name in a model,
//! its `extensibility` (which picks the format of a client's sample) and
//! the `is_key` of its members (which makes the topic keyed). It relies on
//! DDS-XTypes 1.3 §7.2.2.4.4.4.8 (`@key`), §7.3.1.2.1.8 (a struct without
//! annotation is appendable) and §7.5.2 (`find` by scoped name).
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

#[test]
fn xtypes_7_5_2_a_found_type_names_its_extensibility_and_its_key_members() {
    use std::sync::Arc;
    use zerodds_types::dynamic::descriptor::ExtensibilityKind;
    use zerodds_types::dynamic::type_::DynamicType;
    let composed = zerodds_idl_compose::compose_source_with(
        "<contract>",
        "module m { @mutable struct K { @key long id; long v; }; struct D { long v; }; };",
        &zerodds_idl_compose::PathLoader::bare(),
        zerodds_idl_compose::ComposeOptions::default(),
    );
    assert!(!composed.failed());
    let ast = Arc::new(composed.ast);
    let keyed = |t: &DynamicType| {
        t.all_members_by_name()
            .values()
            .any(|m| m.descriptor().is_ok_and(|d| d.is_key))
    };
    let k = DynamicType::find(&ast, "m::K").unwrap();
    assert_eq!(k.extensibility(), Some(ExtensibilityKind::Mutable));
    assert!(keyed(&k));
    let d = DynamicType::find(&ast, "::m::D").unwrap();
    assert_eq!(d.extensibility(), Some(ExtensibilityKind::Appendable));
    assert!(!keyed(&d));
    assert!(DynamicType::find(&ast, "m::Missing").is_err());
}
