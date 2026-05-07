//! Stable-Rust Fuzz-Smoke-Tests fuer den CoAP-Wire-Decoder.
//! Spec: RFC 7252 §3 (Message Format).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_coap_bridge::decode;

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

#[test]
fn fuzz_coap_decode_no_panic() {
    let mut rng = XorShift32::new(0x434F_4150);
    for i in 0..3_000 {
        let len = match i % 8 {
            0 => 0,
            1 => 1,
            2 => 4,
            3 => 16,
            4 => 64,
            5 => 256,
            6 => 1024,
            _ => 4096,
        };
        let bytes = random_bytes(&mut rng, len);
        let _ = decode(&bytes);
    }
}

#[test]
fn empty_input_returns_err_not_panic() {
    assert!(decode(&[]).is_err());
}

#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=255 {
        let _ = decode(&[b]);
    }
}
