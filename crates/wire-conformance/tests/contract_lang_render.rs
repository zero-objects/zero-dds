// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wire-conformance` with `zerodds-lang-render`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-render` (`build.rs`, a build
//! dependency): `rust::file`, which writes the declarations and codecs of
//! `zerodds-lang-rules` as the Rust source `src/lib.rs` includes. The crate
//! relies on that text being Rust the compiler accepts with the items where
//! the tree put them: a struct with its fields public, an `impl` with
//! `encode` and `decode`, an enum with its variants.
//!
//! Oracle: the Rust reference (a `pub` field is readable and writable from
//! another crate; an `impl` block's methods are called with the method-call
//! syntax), checked by the compiler building this target.
#![allow(clippy::unwrap_used)]

use zerodds_wire_conformance::generated::{Point, Sensor};
use zerodds_wire_conformance::{Endianness, Reader, Writer};

#[test]
fn the_rendered_struct_has_public_fields_and_its_codec_in_an_impl() {
    let sensor = Sensor { id: 7, reading: -1 };
    let mut writer = Writer::new(Endianness::Little);
    sensor.encode(&mut writer).unwrap();
    let bytes = writer.finish();
    let mut reader = Reader::new(&bytes, Endianness::Little);
    let back = Sensor::decode(&mut reader).unwrap();
    assert_eq!((back.id, back.reading), (7, -1));
}

#[test]
fn a_rendered_struct_of_primitives_is_copyable_field_by_field() {
    let point = Point { x: 3, y: 0.5 };
    let Point { x, y } = point;
    assert_eq!(x, 3);
    assert!((y - 0.5).abs() < f64::EPSILON);
}
