// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wire-conformance` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules` (`build.rs`, a build
//! dependency): `apply(&ModelGraph::of(&ast), &rule::rust::catalogue())`,
//! `adapter::cst::rust_items` for the declarations and `encode::rust_codec`
//! for each type's `encode`/`decode` under the layout key `build.rs` gives
//! it; `src/lib.rs` includes the result as `generated`. The crate exists to
//! hold that code against the octets the layout calls for, so the contract
//! is the generated code's octets for the three framings.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.3.4.2 (FINAL: the members alone;
//! APPENDABLE: a DHEADER that counts what follows; §7.4.1.1.1, under XCDR2 a
//! value aligns to its size up to four) and §7.4.3.5.3 (a string is its
//! length with the NUL, the octets, the NUL).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_wire_conformance::generated::{Header, Newer, Point};
use zerodds_wire_conformance::{Endianness, Reader, Writer};

fn xcdr2(
    f: impl FnOnce(&mut Writer) -> Result<(), zerodds_wire_conformance::WireError>,
) -> Vec<u8> {
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    f(&mut w).unwrap();
    w.finish()
}

#[test]
fn xtypes_7_4_1_1_1_a_final_point_under_xcdr2_puts_its_double_at_four() {
    let bytes = xcdr2(|w| Point { x: 1, y: 2.0 }.encode(w));
    let mut expected = vec![1, 0, 0, 0];
    expected.extend_from_slice(&2.0f64.to_le_bytes());
    assert_eq!(bytes, expected);
}

#[test]
fn xtypes_7_4_3_4_2_an_appendable_header_writes_a_dheader_that_counts_its_members() {
    assert_eq!(
        xcdr2(|w| Header { seq: 9 }.encode(w)),
        [4, 0, 0, 0, 9, 0, 0, 0]
    );
}

#[test]
fn xtypes_7_4_3_5_3_a_string_member_is_its_length_its_octets_and_the_nul() {
    let newer = Newer {
        a: 1,
        b: 0.0,
        c: 2,
        d: "hi".into(),
    };
    let bytes = xcdr2(|w| newer.encode(w));
    // DHEADER, a, b at 8 (aligned to four, here already), c, padding, d.
    assert_eq!(&bytes[..4], &[(bytes.len() - 4) as u8, 0, 0, 0]);
    assert_eq!(&bytes[bytes.len() - 7..], &[3, 0, 0, 0, b'h', b'i', 0]);
    let mut reader = Reader::new(&bytes, Endianness::Little);
    assert_eq!(Newer::decode(&mut reader).unwrap().d, "hi");
}
