//! Stable-Rust fuzz smoke tests for the WebSocket wire decoder.
//! Spec: RFC 6455 §5.2 (frame format).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_websocket_bridge::decode;

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
fn fuzz_websocket_decode_no_panic() {
    let mut rng = XorShift32::new(0x5753_4B54);
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
fn empty_input_no_panic() {
    let _ = decode(&[]);
}

#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=255 {
        let _ = decode(&[b]);
    }
}

/// 64-bit length-prefix overflow: WS frame with `len_kind=0x7F`
/// and a max u64 length field — the decoder must return `Err`, not
/// call `Vec::with_capacity(u64::MAX)`.
#[test]
fn extended_length_overflow_no_oom() {
    let buf = [
        0x82, // FIN=1, opcode=Binary
        0xFF, // MASK=1, len_kind=0x7F (8-byte ext length)
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // length=u64::MAX
        0x00, 0x00, 0x00, 0x00, // mask key
    ];
    let _ = decode(&buf);
}
