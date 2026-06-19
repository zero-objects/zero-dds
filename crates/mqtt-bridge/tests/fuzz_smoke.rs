//! Stable-Rust fuzz smoke tests for the MQTT-5.0 wire decoder.
//!
//! Spec: MQTT 5.0 §1.5 (data representation) + §3 (control packets).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_mqtt_bridge::decode_publish;
use zerodds_mqtt_bridge::{
    decode_binary_data, decode_two_byte_int, decode_utf8_string, decode_vbi,
};

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
            2 => 4,
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
fn fuzz_decode_publish_no_panic() {
    fuzz_decoder(0x4D51_5054, 3_000, |bytes| {
        let _ = decode_publish(bytes);
    });
}

#[test]
fn fuzz_decode_vbi_no_panic() {
    fuzz_decoder(0x4D51_5642, 3_000, |bytes| {
        let _ = decode_vbi(bytes);
    });
}

#[test]
fn fuzz_decode_two_byte_int_no_panic() {
    fuzz_decoder(0x4D51_5432, 3_000, |bytes| {
        let _ = decode_two_byte_int(bytes);
    });
}

#[test]
fn fuzz_decode_utf8_string_no_panic() {
    fuzz_decoder(0x4D51_5538, 3_000, |bytes| {
        let _ = decode_utf8_string(bytes);
    });
}

#[test]
fn fuzz_decode_binary_data_no_panic() {
    fuzz_decoder(0x4D51_4244, 3_000, |bytes| {
        let _ = decode_binary_data(bytes);
    });
}

#[test]
fn empty_inputs_no_panic() {
    let _ = decode_publish(&[]);
    let _ = decode_vbi(&[]);
    let _ = decode_two_byte_int(&[]);
    let _ = decode_utf8_string(&[]);
    let _ = decode_binary_data(&[]);
}

#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=255 {
        let buf = [b];
        let _ = decode_publish(&buf);
        let _ = decode_vbi(&buf);
        let _ = decode_two_byte_int(&buf);
        let _ = decode_utf8_string(&buf);
        let _ = decode_binary_data(&buf);
    }
}
