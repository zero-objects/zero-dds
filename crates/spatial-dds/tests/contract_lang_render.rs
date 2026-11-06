// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-spatial-dds` with `zerodds-lang-render`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-render` (`build.rs`, a build
//! dependency): `rust::file`, which writes the SpatialDDS declarations and
//! codecs as the Rust module `src/lib.rs` includes. The crate relies on the
//! IDL modules arriving as nested Rust modules at their IDL paths, and on a
//! member named after a Rust keyword (`string type;`) arriving as a field
//! the crate's users can name.
//!
//! Oracle: the Rust reference, "Raw identifiers" (`r#type` names an
//! identifier that is a keyword) and "Modules" (a nested `pub mod` is
//! reached by its path), checked by the compiler building this target;
//! IDL 4.2 §7.5.2 (`spatial::core::PoseSE3` is the scoped name of the type).
#![allow(clippy::unwrap_used)]

use zerodds_spatial_dds::spatial::core::PoseSE3;
use zerodds_spatial_dds::spatial::disco::CoverageElement;

#[test]
fn a_member_named_after_a_rust_keyword_is_a_raw_identifier() {
    let element = CoverageElement {
        r#type: "volume".into(),
        ..CoverageElement::default()
    };
    assert_eq!(element.r#type, "volume");
}

#[test]
fn idl_7_5_2_a_type_is_reached_by_its_scoped_name_as_a_module_path() {
    let pose = zerodds_spatial_dds::spatial::core::PoseSE3 {
        t: [0.0; 3],
        q: [0.0, 0.0, 0.0, 1.0],
    };
    let same: PoseSE3 = pose.clone();
    assert_eq!(pose, same);
}
