// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-poa` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/poa.rs`, `src/servant.rs`):
//! only the type `Endianness`, in the public signatures of `Poa::dispatch`
//! and `Servant::dispatch_object_operation`. The value is passed by copy
//! down to the servant and to `zerodds_corba_giop::object_ops`, which
//! encodes the reply in it; the POA itself reads nothing from it. Cargo
//! refuses the crate if the type goes; what is left for a test is that it
//! is the same `Copy` value with the two byte orders of CDR.
//!
//! Oracle: CORBA 3.3 part 2 §9.3.1 (CDR has exactly two byte orders, big-
//! and little-endian, chosen by the sender), §9.4.1 (bit 0 of the GIOP
//! flags: 0 big-endian, 1 little-endian).

use zerodds_wire::Endianness;

#[test]
fn corba_9_3_1_endianness_is_a_copyable_choice_of_two_byte_orders() {
    fn passed_by_copy<T: Copy + Eq + core::fmt::Debug + Send + Sync + 'static>() {}
    passed_by_copy::<Endianness>();
    assert_eq!(Endianness::ALL, [Endianness::Big, Endianness::Little]);
    assert!(Endianness::Little.is_little());
    assert!(!Endianness::Big.is_little());
}
