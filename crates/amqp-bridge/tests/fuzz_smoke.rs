//! Stable-Rust fuzz smoke tests for the AMQP wire decoders.
//!
//! Pseudo-random byte streams into all top-level decoders. **Not real
//! coverage-guided fuzzing**, but a panic smoke test: no decoder may
//! panic on any input — only `Ok(..)` or `Err(..)` are allowed.
//!
//! For real cargo-fuzz targets see `crates/amqp-bridge/fuzz/`
//! (requires nightly + `cargo install cargo-fuzz`).
//!
//! Spec anchors:
//! * `decode_frame_header` — OASIS amqp-1.0-transport §2.3
//! * `decode_value` — OASIS amqp-1.0-types §1.3-§1.6
//! * `decode_performative` — OASIS amqp-1.0-transport §2.7
//! * `MessageSection::decode` — OASIS amqp-1.0-messaging §3.2

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_amqp_bridge::frame::decode_frame_header;
use zerodds_amqp_bridge::performatives::decode_performative;
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_amqp_bridge::types::decode_value;

/// Simple xorshift32 RNG — deterministic, reproducible. Copied from
/// `crates/rtps/tests/common/mod.rs::XorShift32` (no test-helper crate
/// available, hence inlined).
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
        let w = rng.next_u32().to_le_bytes();
        out.extend_from_slice(&w);
    }
    out.truncate(len);
    out
}

/// Drives a decoder closure with `iterations` pseudo-random inputs of
/// varying sizes. The test fails **only** on a panic — all `Ok`/`Err`
/// results are acceptable.
fn fuzz_decoder<F: FnMut(&[u8])>(seed: u32, iterations: usize, mut f: F) {
    let mut rng = XorShift32::new(seed);
    for i in 0..iterations {
        // Lengths: 0, 1, 2, 4, 8, 16, 64, 256, 1024, 4096 — covers
        // typical buffer boundaries + AMQP frame-size fields.
        let len = match i % 10 {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 4,
            4 => 8,
            5 => 16,
            6 => 64,
            7 => 256,
            8 => 1024,
            _ => 4096,
        };
        let bytes = random_bytes(&mut rng, len);
        f(&bytes);
    }
}

#[test]
fn fuzz_decode_frame_header_no_panic() {
    fuzz_decoder(0x4D51_5048, 5_000, |bytes| {
        let _ = decode_frame_header(bytes);
    });
}

#[test]
fn fuzz_decode_value_no_panic() {
    fuzz_decoder(0x5641_4C55, 5_000, |bytes| {
        let _ = decode_value(bytes);
    });
}

#[test]
fn fuzz_decode_performative_no_panic() {
    fuzz_decoder(0x5045_5246, 5_000, |bytes| {
        let _ = decode_performative(bytes);
    });
}

#[test]
fn fuzz_decode_section_no_panic() {
    fuzz_decoder(0x5345_4354, 5_000, |bytes| {
        let _ = MessageSection::decode(bytes);
    });
}

/// Boundary tests: empty input must not panic.
#[test]
fn empty_input_returns_err_not_panic() {
    assert!(decode_frame_header(&[]).is_err());
    assert!(decode_value(&[]).is_err());
    assert!(decode_performative(&[]).is_err());
    assert!(MessageSection::decode(&[]).is_err());
}

/// Boundary tests: 1-byte input must not panic.
#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=255 {
        let buf = [b];
        let _ = decode_frame_header(&buf);
        let _ = decode_value(&buf);
        let _ = decode_performative(&buf);
        let _ = MessageSection::decode(&buf);
    }
}

/// Length-prefix bypass attempt: declared length greater than the
/// actual buffer length. Per spec the decoder must return `Err`, not
/// panic or read out of bounds.
#[test]
fn length_prefix_overflow_no_panic() {
    // str8 with declared length 255 but only a 3-byte buffer:
    // `0xA1 0xFF "ab"` — the decoder MUST return Err.
    let buf = [0xA1u8, 0xFF, b'a', b'b'];
    let _ = decode_value(&buf);

    // list8 with an absurd length prefix.
    let buf = [0xC0u8, 0xFF, 0x00];
    let _ = decode_value(&buf);

    // Frame with a declared size far over the buffer:
    // SIZE=0xFFFF_FFFF DOFF=0x02 TYPE=0x00 CHANNEL=0x0000
    let buf = [0xFFu8, 0xFF, 0xFF, 0xFF, 0x02, 0x00, 0x00, 0x00];
    let _ = decode_frame_header(&buf);
}

/// Deep nesting: list-in-list-in-... — the DoS guard must kick in
/// (recursion cap or iterative decoder).
#[test]
fn deeply_nested_list_no_panic_no_oom() {
    // 1000 nested list8 with count=1, each with declared size > 0.
    let mut buf = Vec::new();
    for _ in 0..1000 {
        buf.push(0xC0); // list8
        buf.push(0x02); // size = 2 (count + first-element-tag)
        buf.push(0x01); // count = 1
    }
    buf.push(0x40); // null as the innermost element
    let _ = decode_value(&buf);
}
