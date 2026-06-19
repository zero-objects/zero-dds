//! Stable-Rust fuzz smoke tests for the DDS-Security permissions/governance parser.
//!
//! Spec: DDS-Security 1.2 §9.4 (access control, governance + permissions XML).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_security_permissions::{parse_governance_xml, parse_permissions_xml};

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

fn fuzz_parser<F: FnMut(&str)>(seed: u32, iterations: usize, mut f: F) {
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
        let s = random_xml_like(&mut rng, len);
        f(&s);
    }
}

#[test]
fn fuzz_parse_governance_xml_no_panic() {
    fuzz_parser(0x474F_5658, 2_000, |s| {
        let _ = parse_governance_xml(s);
    });
}

#[test]
fn fuzz_parse_permissions_xml_no_panic() {
    fuzz_parser(0x5045_5258, 2_000, |s| {
        let _ = parse_permissions_xml(s);
    });
}

#[test]
fn empty_inputs_no_panic() {
    let _ = parse_governance_xml("");
    let _ = parse_permissions_xml("");
}

#[test]
fn deeply_nested_unclosed_tags_no_panic() {
    // Spec-relevant DoS resistance: with 50 unclosed tags it must
    // either error gracefully or be accepted, not panic.
    let mut s = String::new();
    for _ in 0..50 {
        s.push_str("<rule>");
    }
    let _ = parse_governance_xml(&s);
    let _ = parse_permissions_xml(&s);
}
