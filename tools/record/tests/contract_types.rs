// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-record` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-types` (`src/main.rs`, the `--decode`
//! path): `dynamic::type_::DynamicType`, resolved once per topic, cloned out
//! of a cache for each sample, whose `ast()` is the model the decoder walks;
//! `dynamic::DynamicData`, the value `zerodds_wire_codec::decode` produces
//! and the JSON and SQLite sinks consume. The library of this tool parses
//! arguments only, so the contract calls `zerodds-types` directly.
//!
//! This tool declares `zerodds-types` with `default-features = false,
//! features = ["std"]` and uses the `dynamic` module, which is behind the
//! `dynamic` feature; the feature reaches the build through
//! `zerodds-recorder-decode`, which keeps the defaults.
//!
//! Oracle: DDS-XTypes 1.3 §7.5.2.8 Table 54 (`name`, `kind`,
//! `member_count`), §7.3.4.3 (a COMPLETE TypeObject converts to a type with
//! no information loss) and the COMPLETE TypeObject CycloneDDS 11.0.1
//! writes for `module to { @final struct Plain { long a; short b; }; }`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use zerodds_types::dynamic::to_ast::dynamic_type;
use zerodds_types::dynamic::type_::DynamicType;
use zerodds_types::dynamic::{DynamicData, TypeKind};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_object::TypeObject;

const PLAIN: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";

fn plain() -> DynamicType {
    let bytes: Vec<u8> = (0..PLAIN.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&PLAIN[i..i + 2], 16).unwrap())
        .collect();
    let TypeObject::Complete(complete) = TypeObject::from_bytes_le(&bytes).unwrap() else {
        panic!("a COMPLETE object");
    };
    dynamic_type(&complete, &TypeRegistry::new()).unwrap()
}

#[test]
fn xtypes_7_5_2_8_a_cached_type_is_a_cheap_view_on_one_shared_model() {
    let ty = plain();
    assert_eq!(ty.name(), "Plain");
    assert_eq!(ty.qualified_name(), "to::Plain");
    assert_eq!(ty.kind(), TypeKind::Structure);
    assert_eq!(ty.member_count(), 2);
    // The cache hands out clones; each clone walks the same model.
    let cached = ty.clone();
    assert!(Arc::ptr_eq(ty.ast(), cached.ast()));
    assert_eq!(cached, ty);
    // The decoder looks the type up in that model by the scoped name.
    assert_eq!(
        DynamicType::find(cached.ast(), "to::Plain").unwrap(),
        ty,
        "the model answers for the type's own scoped name"
    );
}

#[test]
fn dynamic_data_is_a_value_the_decoder_can_build_and_the_sinks_can_read() {
    // `decode::<DynamicData>` needs both directions of the value contract.
    fn decoder_target<T: zerodds_wire_codec::Sink + zerodds_wire_codec::Source>() {}
    decoder_target::<DynamicData>();
}
