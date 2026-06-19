//! Stable-Rust fuzz smoke tests for the DDS-RPC wire decoder.
//! Spec: DDS-RPC 1.0 §7.5 (wire mapping).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_rpc::wire_codec::{decode_reply_frame, decode_request_frame};

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
        let len = match i % 7 {
            0 => 0,
            1 => 1,
            2 => 16,
            3 => 64,
            4 => 256,
            5 => 1024,
            _ => 4096,
        };
        let bytes = random_bytes(&mut rng, len);
        f(&bytes);
    }
}

#[test]
fn fuzz_decode_request_frame_no_panic() {
    fuzz_decoder(0x5250_4352, 3_000, |bytes| {
        let _ = decode_request_frame(bytes);
    });
}

#[test]
fn fuzz_decode_reply_frame_no_panic() {
    fuzz_decoder(0x5250_4350, 3_000, |bytes| {
        let _ = decode_reply_frame(bytes);
    });
}

#[test]
fn empty_inputs_return_err_not_panic() {
    assert!(decode_request_frame(&[]).is_err());
    assert!(decode_reply_frame(&[]).is_err());
}

#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=255 {
        let _ = decode_request_frame(&[b]);
        let _ = decode_reply_frame(&[b]);
    }
}
