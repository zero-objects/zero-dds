//! Stable-Rust fuzz smoke tests for the XCDR1 / PL_CDR1 decoder.
//!
//! Pseudo-random byte streams into `read_pl_cdr1_member` and
//! `read_all_pl_cdr1_members`. No decoder may panic on any input —
//! only `Ok(..)` or `Err(..)` are allowed.
//!
//! Spec anchor: XTypes 1.3 §7.4.1.2 (parameter-ID layout) and §7.4.2
//! (PL_CDR1 list format).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_cdr::Endianness;
use zerodds_cdr::buffer::BufferReader;
use zerodds_cdr::xcdr1::{read_all_pl_cdr1_members, read_pl_cdr1_member};

#[derive(Debug, Clone)]
struct XorShift32(u32);

impl XorShift32 {
    fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}

fn random_bytes(rng: &mut XorShift32, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let w = rng.next_u32().to_le_bytes();
        out.extend_from_slice(&w);
    }
    out.truncate(len);
    out
}

fn fuzz_decoder<F: FnMut(&[u8])>(seed: u32, iterations: usize, mut f: F) {
    let mut rng = XorShift32::new(seed);
    for i in 0..iterations {
        let len = match i % 10 {
            0 => 0,
            1 => 1,
            2 => 4,
            3 => 8,
            4 => 16,
            5 => 64,
            6 => 256,
            7 => 1024,
            8 => 4096,
            _ => 16384,
        };
        let bytes = random_bytes(&mut rng, len);
        f(&bytes);
    }
}

#[test]
fn fuzz_read_pl_cdr1_member_no_panic_le() {
    fuzz_decoder(0x504C_434C, 5_000, |bytes| {
        let mut r = BufferReader::new(bytes, Endianness::Little);
        let _ = read_pl_cdr1_member(&mut r);
    });
}

#[test]
fn fuzz_read_pl_cdr1_member_no_panic_be() {
    fuzz_decoder(0x504C_4342, 5_000, |bytes| {
        let mut r = BufferReader::new(bytes, Endianness::Big);
        let _ = read_pl_cdr1_member(&mut r);
    });
}

#[test]
fn fuzz_read_all_pl_cdr1_members_no_panic() {
    fuzz_decoder(0x504C_414C, 5_000, |bytes| {
        let mut r = BufferReader::new(bytes, Endianness::Little);
        let _ = read_all_pl_cdr1_members(&mut r);
    });
}

#[test]
fn empty_input_returns_err_not_panic() {
    let bytes: &[u8] = &[];
    let mut r = BufferReader::new(bytes, Endianness::Little);
    assert!(read_pl_cdr1_member(&mut r).is_err());
}

#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=255 {
        let buf = [b];
        let mut r = BufferReader::new(&buf, Endianness::Little);
        let _ = read_pl_cdr1_member(&mut r);
        let mut r = BufferReader::new(&buf, Endianness::Big);
        let _ = read_pl_cdr1_member(&mut r);
    }
}

/// PID_EXTENDED with an absurd body length (spec §7.4.1.2.2):
/// `[0x01 0x3F 0x08 0x00] [member_id=0xFFFFFFFF] [body_len=0xFFFFFFFF]`
/// The decoder MUST return `LengthExceeded`, not panic or allocate
/// memory.
#[test]
fn extended_header_oversize_length_no_panic_no_oom() {
    let buf = [
        0x01, 0x3F, 0x08, 0x00, // PID_EXTENDED + len=8
        0xFF, 0xFF, 0xFF, 0xFF, // member_id=u32::MAX
        0xFF, 0xFF, 0xFF, 0xFF, // body_len=u32::MAX
    ];
    let mut r = BufferReader::new(&buf, Endianness::Little);
    let res = read_pl_cdr1_member(&mut r);
    assert!(res.is_err(), "expected Err for oversize body_len");
}

/// 1000 PL_CDR1 member heads without a sentinel — the decoder must
/// terminate without running into unbounded allocation.
#[test]
fn long_unterminated_pl_cdr1_no_runaway() {
    let mut buf = Vec::new();
    for i in 0..1000u16 {
        buf.extend_from_slice(&i.to_le_bytes()); // pid
        buf.extend_from_slice(&0u16.to_le_bytes()); // len = 0
    }
    let mut r = BufferReader::new(&buf, Endianness::Little);
    let _ = read_all_pl_cdr1_members(&mut r);
}
