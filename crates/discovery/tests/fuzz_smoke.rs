//! Stable-Rust fuzz smoke tests for the SPDP/SEDP discovery wire.
//!
//! Pseudo-random byte streams into `SpdpReceiver::parse_datagram`. No
//! decoder may panic — only `Ok` or `Err`. Spec anchors:
//! DDSI-RTPS 2.5 §8.5 (Discovery), built-in endpoints and
//! ParameterList decoding.
//!
//! For real cargo-fuzz see `crates/discovery/fuzz/` (phase TS-1
//! follow-up wave).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_discovery::spdp::SpdpReader;

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
        out.extend_from_slice(&rng.next_u32().to_le_bytes());
    }
    out.truncate(len);
    out
}

fn fuzz_decoder<F: FnMut(&[u8])>(seed: u32, iterations: usize, mut f: F) {
    let mut rng = XorShift32::new(seed);
    for i in 0..iterations {
        let len = match i % 9 {
            0 => 0,
            1 => 1,
            2 => 16,
            3 => 64,
            4 => 256,
            5 => 1024,
            6 => 4096,
            7 => 16384,
            _ => 65536,
        };
        let bytes = random_bytes(&mut rng, len);
        f(&bytes);
    }
}

#[test]
fn fuzz_spdp_parse_datagram_no_panic() {
    let recv = SpdpReader;
    fuzz_decoder(0x5350_4450, 3_000, |bytes| {
        let _ = recv.parse_datagram(bytes);
    });
}

#[test]
fn empty_input_returns_err_not_panic() {
    let recv = SpdpReader;
    assert!(recv.parse_datagram(&[]).is_err());
}

#[test]
fn single_byte_inputs_no_panic() {
    let recv = SpdpReader;
    for b in 0u8..=255 {
        let _ = recv.parse_datagram(&[b]);
    }
}

/// RTPS-Header-Magic (`RTPS`) plus Garbage-ParameterList.
#[test]
fn rtps_magic_with_garbage_payload_no_panic() {
    let mut rng = XorShift32::new(0xABCD_1234);
    for _ in 0..1000 {
        let mut bytes = b"RTPS\x02\x05\x00\x00".to_vec();
        bytes.extend_from_slice(&[0; 12]); // GUID-Prefix
        bytes.extend(random_bytes(&mut rng, 200));
        let recv = SpdpReader;
        let _ = recv.parse_datagram(&bytes);
    }
}
