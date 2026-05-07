//! Stable-Rust Fuzz-Smoke-Tests fuer DDS-XRCE-Wire-Decoder.
//!
//! Pseudo-random Byte-Streams in `Message::decode` (Top-Level
//! XRCE-Frame) plus `SubmessageHeader::from_bytes` und SLIP-Frame-
//! Decoder. Kein Decoder darf panicen.
//!
//! Spec: DDS-XRCE 1.0 §7 (Wire-Protocol).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_xrce::transport_serial::{decode_frame, decode_frame_inner};
use zerodds_xrce::{Message, SubmessageHeader};

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
            2 => 8,
            3 => 16,
            4 => 64,
            5 => 256,
            6 => 1024,
            _ => 4096,
        };
        let bytes = random_bytes(&mut rng, len);
        f(&bytes);
    }
}

#[test]
fn fuzz_xrce_message_decode_no_panic() {
    fuzz_decoder(0x5852_434D, 5_000, |bytes| {
        let _ = Message::decode(bytes);
    });
}

#[test]
fn fuzz_xrce_submessage_header_no_panic() {
    fuzz_decoder(0x5852_4348, 5_000, |bytes| {
        let _ = SubmessageHeader::from_bytes(bytes);
    });
}

#[test]
fn fuzz_xrce_serial_decode_frame_no_panic() {
    fuzz_decoder(0x5852_4346, 5_000, |bytes| {
        let _ = decode_frame(bytes);
    });
}

#[test]
fn fuzz_xrce_serial_decode_frame_inner_no_panic() {
    fuzz_decoder(0x5852_4349, 5_000, |bytes| {
        let _ = decode_frame_inner(bytes);
    });
}

#[test]
fn empty_inputs_return_err_not_panic() {
    assert!(Message::decode(&[]).is_err());
    assert!(SubmessageHeader::from_bytes(&[]).is_err());
}

#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=255 {
        let buf = [b];
        let _ = Message::decode(&buf);
        let _ = SubmessageHeader::from_bytes(&buf);
        let _ = decode_frame(&buf);
        let _ = decode_frame_inner(&buf);
    }
}
