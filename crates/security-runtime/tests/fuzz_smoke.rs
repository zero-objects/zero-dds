//! Stable-Rust Fuzz-Smoke-Tests fuer DDS-Security-Runtime-Decoder.
//!
//! Pseudo-random Bytes in `decode_generic_message` (Builtin-Topic
//! ParticipantGenericMessage). Spec: DDS-Security 1.2 §7.4.4.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_security_runtime::builtin_topics::decode_generic_message;

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
        let len = match i % 8 {
            0 => 0,
            1 => 1,
            2 => 16,
            3 => 64,
            4 => 256,
            5 => 1024,
            6 => 4096,
            _ => 16384,
        };
        let bytes = random_bytes(&mut rng, len);
        f(&bytes);
    }
}

#[test]
fn fuzz_decode_generic_message_no_panic() {
    fuzz_decoder(0x5347_4D53, 3_000, |bytes| {
        let _ = decode_generic_message(bytes);
    });
}

#[test]
fn empty_input_returns_err_not_panic() {
    assert!(decode_generic_message(&[]).is_err());
}

#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=255 {
        let _ = decode_generic_message(&[b]);
    }
}
