//! Stable-Rust Fuzz-Smoke-Tests fuer AMQP-Wire-Decoder.
//!
//! Pseudo-random Byte-Streams in alle Top-Level-Decoder. **Kein
//! echtes coverage-guided Fuzzing**, aber ein Panic-Smoke: kein
//! Decoder darf auf irgendeinem Input panicen — nur `Ok(..)` oder
//! `Err(..)` sind erlaubt.
//!
//! Fuer echtes cargo-fuzz-Targets siehe `crates/amqp-bridge/fuzz/`
//! (benoetigt nightly + `cargo install cargo-fuzz`).
//!
//! Spec-Anker:
//! * `decode_frame_header` — OASIS amqp-1.0-transport §2.3
//! * `decode_value` — OASIS amqp-1.0-types §1.3-§1.6
//! * `decode_performative` — OASIS amqp-1.0-transport §2.7
//! * `MessageSection::decode` — OASIS amqp-1.0-messaging §3.2

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_amqp_bridge::frame::decode_frame_header;
use zerodds_amqp_bridge::performatives::decode_performative;
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_amqp_bridge::types::decode_value;

/// Einfacher xorshift32-RNG — deterministisch, reproducible. Kopie
/// aus `crates/rtps/tests/common/mod.rs::XorShift32` (kein
/// Test-Helper-Crate vorhanden, daher inline).
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

/// Treibt eine Decoder-Closure mit `iterations` pseudo-random
/// Inputs in variablen Groessen. Test schlaegt **nur** bei Panic
/// fehl — alle `Ok`/`Err`-Resultate sind akzeptabel.
fn fuzz_decoder<F: FnMut(&[u8])>(seed: u32, iterations: usize, mut f: F) {
    let mut rng = XorShift32::new(seed);
    for i in 0..iterations {
        // Laengen: 0, 1, 2, 4, 8, 16, 64, 256, 1024, 4096 — deckt
        // typische Buffer-Grenzen + AMQP-Frame-Size-Felder ab.
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

/// Boundary-Tests: leerer Input darf nicht panicen.
#[test]
fn empty_input_returns_err_not_panic() {
    assert!(decode_frame_header(&[]).is_err());
    assert!(decode_value(&[]).is_err());
    assert!(decode_performative(&[]).is_err());
    assert!(MessageSection::decode(&[]).is_err());
}

/// Boundary-Tests: 1-Byte-Input darf nicht panicen.
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

/// Length-Prefix-Bypass-Versuch: deklarierte Length grösser als
/// tatsächliche Buffer-Length. Spec-konform muss der Decoder
/// `Err` liefern, nicht panicen oder out-of-bounds lesen.
#[test]
fn length_prefix_overflow_no_panic() {
    // str8 mit deklarierter Length 255, aber nur 3 Bytes Buffer:
    // `0xA1 0xFF "ab"` — Decoder MUSS Err liefern.
    let buf = [0xA1u8, 0xFF, b'a', b'b'];
    let _ = decode_value(&buf);

    // list8 mit absurder Length-Prefix.
    let buf = [0xC0u8, 0xFF, 0x00];
    let _ = decode_value(&buf);

    // Frame mit deklarierter Size weit ueber Buffer:
    // SIZE=0xFFFF_FFFF DOFF=0x02 TYPE=0x00 CHANNEL=0x0000
    let buf = [0xFFu8, 0xFF, 0xFF, 0xFF, 0x02, 0x00, 0x00, 0x00];
    let _ = decode_frame_header(&buf);
}

/// Tiefe Verschachtelung: list-in-list-in-... — DoS-Schutz muss
/// greifen (Recursion-Cap oder iterativer Decoder).
#[test]
fn deeply_nested_list_no_panic_no_oom() {
    // 1000 mal list8 mit count=1, jeweils mit deklarierter Size>0.
    let mut buf = Vec::new();
    for _ in 0..1000 {
        buf.push(0xC0); // list8
        buf.push(0x02); // size = 2 (count + first-element-tag)
        buf.push(0x01); // count = 1
    }
    buf.push(0x40); // null als innerstes Element
    let _ = decode_value(&buf);
}
