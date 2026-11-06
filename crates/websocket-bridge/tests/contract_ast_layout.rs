// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-websocket-bridge` with `zerodds-ast-layout`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses (`src/daemon/json_frames.rs`, feature `daemon`):
//! the `Encoding` and `Endianness` it names for a sample: from a
//! representation identifier (DDS-XTypes 1.3 Table 60) and for a client's
//! sample (XCDR2 little endian). It relies on the five CDR encodings of
//! Table 60 and the two byte orders being distinct values.
//!
//! Oracle: DDS-XTypes 1.3 §7.6.3.1.2, Table 60.
#![allow(missing_docs)]

use zerodds_ast_layout::{Encoding, Endianness};

#[test]
fn xtypes_7_6_3_1_2_the_encodings_and_byte_orders_of_table_60() {
    let encodings = [
        Encoding::PlainCdr1,
        Encoding::PlCdr1,
        Encoding::PlainCdr2,
        Encoding::DelimitedCdr,
        Encoding::PlCdr2,
    ];
    for (i, a) in encodings.iter().enumerate() {
        for b in &encodings[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert_ne!(Endianness::Big, Endianness::Little);
}
