//! Stable-Rust fuzz smoke tests for the AMQP endpoint decoder.
//!
//! Pseudo-random Inputs in `parse_amqp_body`, XML-Config-Parser
//! (`parse_config`, `parse_governance`). Spec: DDS-AMQP 1.0 §8 + §9.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_amqp_endpoint::config_xml::{parse_config, parse_governance};
use zerodds_amqp_endpoint::parse_amqp_body;

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
fn random_xml_like(rng: &mut XorShift32, len: usize) -> String {
    let alphabet: &[u8] = b"<>/\"'= \tabcdefghijklmnopqrstuvwxyz0123456789-_:";
    let mut out = String::new();
    while out.len() < len {
        let w = rng.next_u32();
        for shift in 0..4 {
            let b = ((w >> (shift * 8)) & 0xFF) as usize;
            out.push(alphabet[b % alphabet.len()] as char);
            if out.len() >= len {
                break;
            }
        }
    }
    out
}

#[test]
fn fuzz_parse_amqp_body_no_panic() {
    let mut rng = XorShift32::new(0x414D_4250);
    let content_types = [
        None,
        Some("application/octet-stream"),
        Some("application/json"),
        Some("application/vnd.dds.xcdr2"),
        Some("application/vnd.dds.amqp-native"),
    ];
    for i in 0..2_000 {
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
        let ct = content_types[i % content_types.len()];
        let _ = parse_amqp_body(&bytes, ct);
    }
}

#[test]
fn fuzz_parse_config_no_panic() {
    let mut rng = XorShift32::new(0x584D_4C43);
    for i in 0..1_500 {
        let len = match i % 6 {
            0 => 0,
            1 => 16,
            2 => 64,
            3 => 256,
            4 => 1024,
            _ => 4096,
        };
        let s = random_xml_like(&mut rng, len);
        let _ = parse_config(&s);
    }
}

#[test]
fn fuzz_parse_governance_no_panic() {
    let mut rng = XorShift32::new(0x584D_4C47);
    for i in 0..1_500 {
        let len = match i % 6 {
            0 => 0,
            1 => 16,
            2 => 64,
            3 => 256,
            4 => 1024,
            _ => 4096,
        };
        let s = random_xml_like(&mut rng, len);
        let _ = parse_governance(&s);
    }
}

#[test]
fn empty_inputs_no_panic() {
    let _ = parse_amqp_body(&[], None);
    let _ = parse_config("");
    let _ = parse_governance("");
}
