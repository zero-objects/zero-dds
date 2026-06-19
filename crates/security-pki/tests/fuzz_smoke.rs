//! Stable-Rust fuzz smoke tests for the DDS-Security PKI decoder.
//!
//! Pseudo-random bytes into `parse_crl_serials`, the OCSP status decoder,
//! the handshake-token parser. DDS-Security 1.2 §9.3 (auth plugin).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_security_pki::{
    parse_crl_serials, parse_final_token, parse_ocsp_status, parse_reply_token, parse_request_token,
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
fn fuzz_parse_crl_serials_no_panic() {
    fuzz_decoder(0x4352_4C53, 2_000, |bytes| {
        let _ = parse_crl_serials(bytes);
    });
}

#[test]
fn fuzz_parse_ocsp_status_no_panic() {
    // parse_ocsp_status returns no Result, but must not panic.
    fuzz_decoder(0x4F43_5350, 2_000, |bytes| {
        let _ = parse_ocsp_status(bytes);
    });
}

#[test]
fn fuzz_parse_request_token_no_panic() {
    fuzz_decoder(0x5251_5354, 2_000, |bytes| {
        let _ = parse_request_token(bytes);
    });
}

#[test]
fn fuzz_parse_reply_token_no_panic() {
    fuzz_decoder(0x5250_4C59, 2_000, |bytes| {
        let _ = parse_reply_token(bytes);
    });
}

#[test]
fn fuzz_parse_final_token_no_panic() {
    fuzz_decoder(0x4649_4E4C, 2_000, |bytes| {
        let _ = parse_final_token(bytes);
    });
}

#[test]
fn empty_inputs_no_panic() {
    let _ = parse_crl_serials(&[]);
    let _ = parse_ocsp_status(&[]);
    let _ = parse_request_token(&[]);
    let _ = parse_reply_token(&[]);
    let _ = parse_final_token(&[]);
}

#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=255 {
        let buf = [b];
        let _ = parse_crl_serials(&buf);
        let _ = parse_ocsp_status(&buf);
        let _ = parse_request_token(&buf);
        let _ = parse_reply_token(&buf);
        let _ = parse_final_token(&buf);
    }
}
