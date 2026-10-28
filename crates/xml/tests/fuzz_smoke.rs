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

// ---------------------------------------------------------------------
// Depth: every entry point refuses deep nesting before anything recurses
// ---------------------------------------------------------------------

type Entry = (&'static str, fn(&str) -> bool);

/// Every function that takes XML text; each returns whether it refused.
const ENTRY_POINTS: &[Entry] = &[
    ("parse_xml_tree", |x| parse_xml_tree(x).is_err()),
    ("parse_dds_xml", |x| parse_dds_xml(x).is_err()),
    ("parse_dds_xml_with", |x| {
        zerodds_xml::parse_dds_xml_with(x, &zerodds_xml::ReadOptions::report_and_skip()).is_err()
    }),
    ("parse_qos_libraries", |x| parse_qos_libraries(x).is_err()),
    ("parse_qos_libraries_with", |x| {
        zerodds_xml::parse_qos_libraries_with(x, &zerodds_xml::ReadOptions::report_and_skip())
            .is_err()
    }),
    ("parse_domain_libraries", |x| {
        parse_domain_libraries(x).is_err()
    }),
    ("parse_domain_participant_libraries", |x| {
        parse_domain_participant_libraries(x).is_err()
    }),
    ("parse_application_libraries", |x| {
        parse_application_libraries(x).is_err()
    }),
    ("parse_type_libraries", |x| {
        zerodds_xml::parse_type_libraries(x).is_err()
    }),
    ("parse_type_libraries_with", |x| {
        zerodds_xml::parse_type_libraries_with(x, &zerodds_xml::ReadOptions::default()).is_err()
    }),
    ("QosProfileRegistry::from_xml", |x| {
        zerodds_xml::QosProfileRegistry::from_xml(x).is_err()
    }),
    ("QosProfileRegistry::from_xml_with", |x| {
        zerodds_xml::QosProfileRegistry::from_xml_with(x, &zerodds_xml::ReadOptions::default())
            .is_err()
    }),
    ("load_type_libraries_from_string", |x| {
        zerodds_xml::load_type_libraries_from_string(x, zerodds_xml::ValidationMode::Lax).is_err()
    }),
];

/// Unclosed and closed runs of `<a>` of several lengths, bare and below the
/// roots the loaders know, are refused — not run into a stack overflow.
#[test]
fn deep_nesting_is_refused_by_every_entry_point() {
    for (name, entry) in ENTRY_POINTS {
        for depth in [100usize, 1_000, 100_000] {
            let open = "<a>".repeat(depth);
            let closed = format!("{open}{}", "</a>".repeat(depth));
            for root in ["", "<dds>", "<types>", "<qos_library name=\"L\">"] {
                for body in [&open, &closed] {
                    let src = format!("{root}{body}");
                    assert!(entry(&src), "{name}: depth {depth} under `{root}` was read");
                }
            }
        }
    }
}

/// Markup that looks like nesting but is none must not be counted: the
/// same entry points still read a shallow document that carries `<` in a
/// comment, a CDATA section and an attribute value.
#[test]
fn nesting_scan_ignores_comments_cdata_and_quoted_text() {
    let noise = "<!-- <a><a><a> -->".repeat(200)
        + &"<![CDATA[<a><a><a>]]>".repeat(200)
        + &"<?pi <a><a> ?>".repeat(200);
    let src = format!("<dds a=\"x>y\">{noise}</dds>");
    assert!(parse_xml_tree(&src).is_ok());
    // 1000 siblings are no nesting.
    let siblings = format!("<dds>{}</dds>", "<a/>".repeat(900));
    assert!(parse_xml_tree(&siblings).is_ok());
}

/// Nesting inside an internal DTD subset's entity expansion: a short entity
/// value of nested elements, referenced below a document that is itself
/// nearly at the cap, goes over it and is refused; a value over the size
/// limit is refused where it is declared.
#[test]
fn deep_nesting_through_entity_expansion_is_refused() {
    let value = format!("{}{}", "<a>".repeat(35), "</a>".repeat(35));
    let below = format!("{}&e;{}", "<b>".repeat(60), "</b>".repeat(60));
    let src = format!("<!DOCTYPE dds [ <!ENTITY e \"{value}\"> ]><dds>{below}</dds>");
    for (name, entry) in ENTRY_POINTS {
        assert!(
            entry(&src),
            "{name}: entity expansion over the cap was read"
        );
    }
    let big = format!("{}{}", "<a>".repeat(100), "</a>".repeat(100));
    let src = format!("<!DOCTYPE dds [ <!ENTITY e \"{big}\"> ]><dds>&e;</dds>");
    assert!(parse_xml_tree(&src).is_err());
    // A `>` and a `<a>` inside the subset's quotes and comments do not
    // end the declaration or count as tags.
    let tricky = "<!DOCTYPE dds [ <!-- <a><a> --> <!ENTITY e \"x>y\"> ]><dds>&e;</dds>";
    assert!(parse_xml_tree(tricky).is_ok());
}
