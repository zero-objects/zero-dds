// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-security-main` with `zerodds-build`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-build`: its build script compiles
//! `interop/shapes/omg-dds-security.idl` with `Config::new()`, the include
//! directory, `default_extensibility(DefaultExt::Appendable)` and
//! `dds_type_impls(true)`; `src/shape_type.rs` includes the generated module
//! and writes, reads and keys `ShapeType` through its `DdsType` impl. The
//! contract includes the same module from `OUT_DIR`.
//!
//! Oracle: the suite's IDL (no extensibility annotation, so the
//! configured default APPENDABLE, DDS-XTypes 1.3 §7.3.1.2.1.8; the bound
//! `COLOR_MAX_LENGTH` = 64; the key written `//@key`, which Table 24
//! makes the `@key` annotation), §7.4.3.4.2 (DELIMITED_CDR: a DHEADER with
//! the length of what follows), §7.4.3.5.3 (a string), and §7.6.8 (the
//! KeyHash depends on the key members only).
#![allow(clippy::unwrap_used, clippy::panic, dead_code)]

mod generated {
    include!(concat!(env!("OUT_DIR"), "/omg-dds-security.rs"));
}

use generated::ShapeType;
use zerodds_dcps::DdsType;

fn shape(color: &str, x: i32) -> ShapeType {
    ShapeType {
        color: color.into(),
        x,
        y: 77,
        shapesize: 30,
    }
}

#[test]
fn xtypes_7_3_1_2_1_8_the_unannotated_type_takes_the_configured_appendable_and_its_dheader() {
    assert_eq!(
        <ShapeType as DdsType>::EXTENSIBILITY,
        zerodds_dcps::dds_type::Extensibility::Appendable
    );
    let mut out = Vec::new();
    DdsType::encode(&shape("BLACK", 42), &mut out).unwrap();
    // DHEADER 24, the string (6 with the NUL, padded to 16), three longs.
    assert_eq!(
        out,
        [
            24, 0, 0, 0, 6, 0, 0, 0, b'B', b'L', b'A', b'C', b'K', 0, 0, 0, 42, 0, 0, 0, 77, 0, 0,
            0, 30, 0, 0, 0
        ]
    );
    assert_eq!(<ShapeType as DdsType>::decode(&out).unwrap().x, 42);
}

#[test]
fn xtypes_7_6_8_the_suffix_key_makes_the_colour_the_one_key_member() {
    assert!(
        <ShapeType as DdsType>::has_key(),
        "`//@key` is the `@key` annotation (DDS-XTypes 1.3 Table 24)"
    );
    let black = DdsType::key_hash(&shape("BLACK", 42));
    assert!(black.is_some());
    assert_eq!(
        black,
        DdsType::key_hash(&shape("BLACK", 1)),
        "x is not a key member"
    );
    assert_ne!(black, DdsType::key_hash(&shape("RED", 42)));
}

#[test]
fn xtypes_7_2_2_4_3_a_colour_past_its_bound_is_refused() {
    let long = "C".repeat(65);
    let mut out = Vec::new();
    assert!(
        DdsType::encode(&shape(&long, 1), &mut out).is_err(),
        "string<COLOR_MAX_LENGTH> holds at most 64 characters"
    );
}

#[test]
fn the_type_is_announced_by_the_suites_name_with_its_type_object() {
    assert_eq!(<ShapeType as DdsType>::TYPE_NAME, "ShapeType");
    assert!(<ShapeType as DdsType>::type_object().is_some());
}
