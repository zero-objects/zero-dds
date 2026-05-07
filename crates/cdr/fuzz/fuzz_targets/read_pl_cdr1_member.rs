#![no_main]
//! Fuzz-Target: `read_pl_cdr1_member` (XTypes 1.3 §7.4.1.2).

use zerodds_cdr::Endianness;
use zerodds_cdr::buffer::BufferReader;
use zerodds_cdr::xcdr1::read_pl_cdr1_member;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let endianness = if data[0] & 1 != 0 {
        Endianness::Little
    } else {
        Endianness::Big
    };
    let mut r = BufferReader::new(&data[1..], endianness);
    let _ = read_pl_cdr1_member(&mut r);
});
