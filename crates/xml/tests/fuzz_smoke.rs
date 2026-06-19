//! Stable-Rust fuzz smoke tests for the XML parser.
//!
//! Pseudo-random string inputs into all top-level parsers
//! (`parse_xml_tree`, `parse_dds_xml`, `parse_qos_libraries`,
//! `parse_domain_libraries`, `parse_application_libraries`,
//! `parse_domain_participant_libraries`). No parser may
//! panic — only `Ok(..)` or `Err(..)`.
//!
//! Spec anchor: DDS-XML 1.0 (XSD-driven loader).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_xml::application::parse_application_libraries;
use zerodds_xml::domain::parse_domain_libraries;
use zerodds_xml::parser::parse_xml_tree;
use zerodds_xml::participant::parse_domain_participant_libraries;
use zerodds_xml::qos_parser::parse_qos_libraries;
use zerodds_xml::zerodds_xml::parse_dds_xml;

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

fn random_ascii_xml_like(rng: &mut XorShift32, len: usize) -> String {
    // ASCII character set with XML-typical characters overrepresented.
    const ALPHABET: &[u8] = b"<>/\"'= \tabcdefghijklmnopqrstuvwxyzABCDEFGHIJ0123456789-_:";
    let mut out = String::with_capacity(len);
    while out.len() < len {
        let w = rng.next_u32();
        for shift in 0..4 {
            let b = ((w >> (shift * 8)) & 0xFF) as usize;
            out.push(ALPHABET[b % ALPHABET.len()] as char);
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
        let src = random_ascii_xml_like(&mut rng, len);
        f(&src);
    }
}

#[test]
fn fuzz_parse_xml_tree_no_panic() {
    fuzz_parser(0x584D_4C54, 3_000, |src| {
        let _ = parse_xml_tree(src);
    });
}

#[test]
fn fuzz_parse_dds_xml_no_panic() {
    fuzz_parser(0x4444_5358, 3_000, |src| {
        let _ = parse_dds_xml(src);
    });
}

#[test]
fn fuzz_parse_qos_libraries_no_panic() {
    fuzz_parser(0x514F_534C, 3_000, |src| {
        let _ = parse_qos_libraries(src);
    });
}

#[test]
fn fuzz_parse_domain_libraries_no_panic() {
    fuzz_parser(0x444F_4D4C, 3_000, |src| {
        let _ = parse_domain_libraries(src);
    });
}

#[test]
fn fuzz_parse_application_libraries_no_panic() {
    fuzz_parser(0x4150_504C, 3_000, |src| {
        let _ = parse_application_libraries(src);
    });
}

#[test]
fn fuzz_parse_domain_participant_libraries_no_panic() {
    fuzz_parser(0x4450_4C4C, 3_000, |src| {
        let _ = parse_domain_participant_libraries(src);
    });
}

#[test]
fn empty_input_no_panic() {
    let _ = parse_xml_tree("");
    let _ = parse_dds_xml("");
    let _ = parse_qos_libraries("");
    let _ = parse_domain_libraries("");
    let _ = parse_application_libraries("");
    let _ = parse_domain_participant_libraries("");
}

/// Nested unclosed tags: `<a><a><a>...` 50 times — empirically
/// below the stack limit.
#[test]
fn nested_unclosed_tags_within_safe_depth() {
    let mut src = String::new();
    for _ in 0..50 {
        src.push_str("<a>");
    }
    let _ = parse_xml_tree(&src);
}

/// Spec control test: 1000 unclosed tags trigger the pre-validation
/// cap (`parser::MAX_TREE_DEPTH = 64`) — the parser MUST return a
/// `LimitExceeded` error, not run into a stack overflow.
///
/// TS-1 finding 3 (fixed 2026-05-01).
#[test]
fn deeply_nested_unclosed_tags_rejected_by_depth_cap() {
    let mut src = String::new();
    for _ in 0..1000 {
        src.push_str("<a>");
    }
    let res = parse_xml_tree(&src);
    assert!(res.is_err(), "expected error from depth-cap, got {res:?}");
}

/// Single-byte inputs.
#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=127 {
        let src = (b as char).to_string();
        let _ = parse_xml_tree(&src);
    }
}
