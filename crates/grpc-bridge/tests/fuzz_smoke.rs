//! Stable-Rust Fuzz-Smoke-Tests fuer den gRPC-Wire-Decoder.
//! Spec: gRPC HTTP/2 Protocol § Length-Prefixed Message + Path/Timeout.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_grpc_bridge::{decode_message, decode_timeout, parse_path};

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
fn fuzz_decode_message_no_panic() {
    let mut rng = XorShift32::new(0x4752_504D);
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
        let _ = decode_message(&bytes);
    }
}

#[test]
fn fuzz_parse_path_no_panic() {
    let mut rng = XorShift32::new(0x4752_5050);
    let alphabet: &[u8] = b"/abcdefghijklmnopqrstuvwxyz0123456789-_.:";
    for i in 0..3_000 {
        let len = match i % 7 {
            0 => 0,
            1 => 1,
            2 => 4,
            3 => 16,
            4 => 64,
            5 => 256,
            _ => 1024,
        };
        let mut s = String::new();
        while s.len() < len {
            let w = rng.next_u32();
            for shift in 0..4 {
                let b = ((w >> (shift * 8)) & 0xFF) as usize;
                s.push(alphabet[b % alphabet.len()] as char);
                if s.len() >= len {
                    break;
                }
            }
        }
        let _ = parse_path(&s);
    }
}

#[test]
fn fuzz_decode_timeout_no_panic() {
    let mut rng = XorShift32::new(0x4752_5054);
    let alphabet: &[u8] = b"0123456789HMSnumun";
    for i in 0..3_000 {
        let len = match i % 6 {
            0 => 0,
            1 => 1,
            2 => 4,
            3 => 16,
            _ => 64,
        };
        let mut s = String::new();
        while s.len() < len {
            let w = rng.next_u32();
            for shift in 0..4 {
                let b = ((w >> (shift * 8)) & 0xFF) as usize;
                s.push(alphabet[b % alphabet.len()] as char);
                if s.len() >= len {
                    break;
                }
            }
        }
        let _ = decode_timeout(&s);
    }
}

#[test]
fn empty_inputs_no_panic() {
    let _ = decode_message(&[]);
    let _ = parse_path("");
    let _ = decode_timeout("");
}

/// 32-bit Length-Prefix-Overflow: gRPC-Frame `[0x00 0xFF 0xFF 0xFF 0xFF]`
/// — Decoder muss `Err` liefern, nicht `Vec::with_capacity(u32::MAX)`.
#[test]
fn message_oversized_length_no_oom() {
    let buf = [0x00, 0xFF, 0xFF, 0xFF, 0xFF];
    let _ = decode_message(&buf);
}
