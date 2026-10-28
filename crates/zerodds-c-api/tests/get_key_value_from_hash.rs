// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds_dw_get_key_value` for an instance registered only by its sixteen
//! octets (DDS 1.4 §2.2.2.4.2.9): the key is read out of the KeyHash where
//! DDSI-RTPS 2.5 §9.6.4.8 Step 5.1 writes it out; where it does not, the one
//! error the clause names, BAD_PARAMETER.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod typed_writer;

use zerodds::dyntype_ffi::zerodds_dyntype_destroy;
use zerodds::extra_ffi::{zerodds_dw_get_key_value, zerodds_dw_register_instance};
use zerodds::factory_ffi::zerodds_dpf_delete_participant;

const OK: i32 = 0;
const BAD_PARAMETER: i32 = -6;

#[test]
fn the_key_comes_out_of_a_hash_that_spells_it_and_nothing_else_does() {
    let lease = zerodds_test_domains::lease_domain();
    // SAFETY: handles come from the create calls above their use and are
    // released at the end.
    unsafe {
        let typed_writer::TypedWriter {
            t, f, dp, writer, ..
        } = typed_writer::typed_writer(
            "@final struct KRow { @key long id; double v; };",
            "KRow",
            "CKeyFromHash",
            lease.id() as u32,
        );

        // The KeyHash of id 7: the four PLAIN_CDR2-BE octets, zero-padded.
        let mut spelled = [0u8; 16];
        spelled[..4].copy_from_slice(&7i32.to_be_bytes());
        let mut handle = 0u64;
        assert_eq!(
            zerodds_dw_register_instance(writer, spelled.as_ptr(), 16, &mut handle),
            OK
        );
        let mut buf = [0u8; 16];
        let mut len = buf.len();
        assert_eq!(
            zerodds_dw_get_key_value(writer, handle, buf.as_mut_ptr(), &mut len),
            OK
        );
        assert_eq!(&buf[..len], &7i32.to_be_bytes(), "the key holder of id 7");

        // A hash that is no key of this type.
        let nothing = [0xAAu8; 16];
        let mut other = 0u64;
        assert_eq!(
            zerodds_dw_register_instance(writer, nothing.as_ptr(), 16, &mut other),
            OK
        );
        let mut len = buf.len();
        assert_eq!(
            zerodds_dw_get_key_value(writer, other, buf.as_mut_ptr(), &mut len),
            BAD_PARAMETER
        );

        zerodds_dyntype_destroy(t);
        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}

/// §9.6.4.8 Step 5.1: the KeyHash is the key holder in big-endian CDR padded
/// with zeros to 16 octets. The holder is read from the 16 octets, and every
/// octet after it must be zero. A key whose own last octet is 0x00 is not cut
/// short at that octet, and a non-zero octet in the padding refuses the hash.
#[test]
fn a_key_ending_in_a_zero_octet_is_read_whole_and_the_padding_must_be_zero() {
    let lease = zerodds_test_domains::lease_domain();
    // SAFETY: handles come from the create calls above their use and are
    // released at the end.
    unsafe {
        let typed_writer::TypedWriter {
            t, f, dp, writer, ..
        } = typed_writer::typed_writer(
            "@final struct KRow2 { @key long id; @key octet flag; double v; };",
            "KRow2",
            "CKeyFromHashZeroEnd",
            lease.id() as u32,
        );

        // id 7, flag 0: five octets, the last of them 0x00, then eleven of padding.
        let mut spelled = [0u8; 16];
        spelled[..4].copy_from_slice(&7i32.to_be_bytes());
        let mut handle = 0u64;
        assert_eq!(
            zerodds_dw_register_instance(writer, spelled.as_ptr(), 16, &mut handle),
            OK
        );
        let mut buf = [0u8; 16];
        let mut len = buf.len();
        assert_eq!(
            zerodds_dw_get_key_value(writer, handle, buf.as_mut_ptr(), &mut len),
            OK
        );
        assert_eq!(&buf[..len], &[0, 0, 0, 7, 0], "id 7 and flag 0, whole");

        // The same holder with a non-zero octet in the padding.
        let mut dirty = spelled;
        dirty[9] = 1;
        let mut other = 0u64;
        assert_eq!(
            zerodds_dw_register_instance(writer, dirty.as_ptr(), 16, &mut other),
            OK
        );
        let mut len = buf.len();
        assert_eq!(
            zerodds_dw_get_key_value(writer, other, buf.as_mut_ptr(), &mut len),
            BAD_PARAMETER
        );

        zerodds_dyntype_destroy(t);
        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}
