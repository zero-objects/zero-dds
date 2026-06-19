//! Stable-Rust fuzz smoke tests for the IDL parser.
//!
//! Pseudo-random UTF-8 strings into `zerodds_idl::parse`. The parser must not
//! panic on any input — only `Ok(..)` or `Err(..)` are
//! allowed. Complements the IDL-specific compliance tests already
//! present in the `crates/idl/tests/` directory with adversarial
//! input robustness.
//!
//! Spec anchor: OMG IDL 4.2 — lexer + parser state machine.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;

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

/// Generates a pseudo-random ASCII string of length `len`.
/// Limited to the printable ASCII range plus `\n\t` — the
/// parser must accept UTF-8-valid input, but here we want to
/// hammer the lexer/parser path, not the UTF-8 validation.
fn random_ascii(rng: &mut XorShift32, len: usize) -> String {
    let mut out = String::with_capacity(len);
    while out.len() < len {
        let w = rng.next_u32();
        for shift in 0..4 {
            let b = ((w >> (shift * 8)) & 0x7F) as u8;
            // Map invalid control characters (except \n \t) to space.
            let c = match b {
                b'\n' | b'\t' => b as char,
                0..=0x1F | 0x7F => ' ',
                _ => b as char,
            };
            out.push(c);
            if out.len() >= len {
                break;
            }
        }
    }
    out
}

/// Generates pseudo-random bytes and tries to interpret them as UTF-8.
/// On bad UTF-8 we take the lossy path — from that too
/// the parser must cleanly return `Err`.
fn random_utf8_lossy(rng: &mut XorShift32, len: usize) -> String {
    let mut bytes = Vec::with_capacity(len);
    while bytes.len() < len {
        let w = rng.next_u32().to_le_bytes();
        bytes.extend_from_slice(&w);
    }
    bytes.truncate(len);
    String::from_utf8_lossy(&bytes).into_owned()
}

fn fuzz_parser<F: FnMut(&str)>(seed: u32, iterations: usize, mut f: F) {
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
        let src = if i % 2 == 0 {
            random_ascii(&mut rng, len)
        } else {
            random_utf8_lossy(&mut rng, len)
        };
        f(&src);
    }
}

#[test]
fn fuzz_idl_parse_no_panic() {
    let cfg = ParserConfig::default();
    fuzz_parser(0x4944_4C50, 2_000, |src| {
        let _ = zerodds_idl::parse(src, &cfg);
    });
}

#[test]
fn empty_input_parses_to_empty_spec() {
    let res = zerodds_idl::parse("", &ParserConfig::default());
    // An empty spec is a valid IDL corpus ("nothing declared").
    assert!(res.is_ok());
}

#[test]
fn single_char_inputs_no_panic() {
    let cfg = ParserConfig::default();
    for c in 0u8..=127 {
        let src = (c as char).to_string();
        let _ = zerodds_idl::parse(&src, &cfg);
    }
}

/// Deeply nested modules: depth `module M { ... }` inside each other.
/// Spec-compliant, but a stack-overflow DoS vector; the recursion cap
/// must kick in or the parser must work iteratively.
///
/// Currently at depth=64 — empirically below the stack limit of the
/// recursive-descent parser. **Higher depths trigger
/// stack overflow** (TS-1 finding 2026-05-01); see
/// `deeply_nested_modules_known_overflow` for the ignored
/// reproduction test, and the open issue on the parser recursion cap.
#[test]
fn deeply_nested_modules_within_safe_depth() {
    let mut src = String::new();
    let depth = 64;
    for i in 0..depth {
        src.push_str(&format!("module M{i} {{ "));
    }
    for _ in 0..depth {
        src.push_str("};");
    }
    let _ = zerodds_idl::parse(&src, &ParserConfig::default());
}

/// Spec control test: 256 nested modules trigger the pre-tokenization
/// cap (`parser::MAX_NESTING_DEPTH = 64`) — the parser MUST return a
/// `DepthLimit` error, not run into stack overflow.
///
/// TS-1 finding 1 (fixed 2026-05-01).
#[test]
fn deeply_nested_modules_rejected_by_depth_cap() {
    let mut src = String::new();
    let depth = 256;
    for i in 0..depth {
        src.push_str(&format!("module M{i} {{ "));
    }
    for _ in 0..depth {
        src.push_str("};");
    }
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        matches!(res, Err(zerodds_idl::Error::DepthLimit { .. })),
        "expected DepthLimit error, got {res:?}"
    );
}

/// Very long identifiers (10k characters) — the lexer must bound them
/// or pass through without an allocation explosion.
#[test]
fn very_long_identifier_no_panic() {
    let ident: String = "a".repeat(10_000);
    let src = format!("struct {ident} {{ long x; }};");
    let _ = zerodds_idl::parse(&src, &ParserConfig::default());
}

/// Realistic number of annotations: `@final @final ... struct S{};`
/// With 50 annotations the parser runs through in <1s.
#[test]
fn many_annotations_no_panic_realistic() {
    let mut src = String::new();
    for _ in 0..50 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let _ = zerodds_idl::parse(&src, &ParserConfig::default());
}

/// Spec control test: 100 consecutive annotations trigger
/// the pre-tokenization cap (`parser::MAX_CONSECUTIVE_ANNOTATIONS = 64`)
/// — the parser MUST return an `AnnotationLimit` error, instead of running into
/// O(n²) CST-build costs.
///
/// TS-1 finding 2 (fixed 2026-05-01).
///
/// The count is deliberately low (100, not 1000): on mutation-test runs
/// that disable the cap, recognize runs over all annotations.
/// 1000 would blow the 90s mutation timeout there, 100 does not.
#[test]
fn many_annotations_rejected_by_annotation_cap() {
    let mut src = String::new();
    for _ in 0..100 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "expected AnnotationLimit, got {res:?}"
    );
}

/// Mutation test: depth=MAX_NESTING_DEPTH+1 must trigger exactly for `> MAX`
/// (catches `>` -> `==` and `>` -> `>=` mutations).
/// MAX_NESTING_DEPTH=64; 65 nested {} must return DepthLimit.
#[test]
fn nesting_depth_just_over_cap_rejected() {
    let depth = 65; // = MAX_NESTING_DEPTH + 1
    let mut src = String::new();
    for i in 0..depth {
        src.push_str(&format!("module M{i} {{ "));
    }
    for _ in 0..depth {
        src.push_str("};");
    }
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        matches!(res, Err(zerodds_idl::Error::DepthLimit { .. })),
        "depth=65 must trigger DepthLimit, got {res:?}"
    );
}

/// Mutation test: depth=MAX_NESTING_DEPTH (64) must NOT trigger
/// (catches the `>` -> `>=` mutation that would already error at 64).
#[test]
fn nesting_depth_at_cap_accepted() {
    let depth = 64; // = MAX_NESTING_DEPTH
    let mut src = String::new();
    for i in 0..depth {
        src.push_str(&format!("module M{i} {{ "));
    }
    for _ in 0..depth {
        src.push_str("};");
    }
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    // Expected: NO DepthLimit error. Other errors (lex/parse) are
    // acceptable here, the cap itself must not kick in.
    assert!(
        !matches!(res, Err(zerodds_idl::Error::DepthLimit { .. })),
        "depth=64 must NOT trigger DepthLimit, got {res:?}"
    );
}

/// Mutation test: 65 consecutive annotations trigger the cap.
/// Catches `>` -> `==` and `>` -> `>=` mutations at the annotation limit.
#[test]
fn consecutive_annotations_just_over_cap_rejected() {
    let mut src = String::new();
    for _ in 0..65 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "65 annotations must trigger AnnotationLimit, got {res:?}"
    );
}

/// Mutation test: 64 consecutive annotations must NOT trigger.
/// Catches the `>` -> `>=` mutation at the annotation limit.
#[test]
fn consecutive_annotations_at_cap_accepted() {
    let mut src = String::new();
    for _ in 0..64 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        !matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "64 annotations must NOT trigger AnnotationLimit, got {res:?}"
    );
}

/// Mutation test: a semicolon resets the annotation counter.
/// Proof: 200 `@final;` in a row must NOT run into AnnotationLimit,
/// because each `;` resets the counter (even if the parser
/// then fails at recognize due to syntax).
/// Catches the `==` -> `!=` mutation on the `;`-reset branch (line 86).
#[test]
fn semicolon_resets_annotation_counter() {
    let mut src = String::new();
    for _ in 0..200 {
        src.push_str("@final; ");
    }
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    // Original behavior: pre-check passes (`;` resets), recognize
    // fails syntactically. Mutation `!=`: the pre-check collects 200
    // `@`s without resetting and fires AnnotationLimit from the 65th.
    assert!(
        !matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "semicolons must reset annotation counter — got AnnotationLimit unexpectedly: {res:?}"
    );
}

/// Mutation test: the `}` branch decrements depth correctly; the `@` branch
/// must be reached separately, not "swallowed" by the `}`-`!=` mutation.
///
/// Proof: 65 `@final` without block structures MUST run into AnnotationLimit.
/// With the mutation `} else if p == "}"` -> `} else if p != "}"`,
/// every `@` would fall into the `}` branch (because `@` != `}`) and
/// `consecutive_at` would always be 0, no cap trigger.
///
/// Input without `{` `}` so that the `}` branch is the only one that could
/// "steal" `@` — the `;` branch only kicks in after the `@` branch.
#[test]
fn close_brace_branch_does_not_swallow_at() {
    let mut src = String::new();
    for _ in 0..65 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "65 @final must trigger AnnotationLimit (catches `}}`-branch swallowing `@`), got {res:?}"
    );
}

/// Mutation test: the `@` branch actually does `consecutive_at += 1`,
/// not `*=` (which would go 0 -> 0 -> 0 -> ... never overflowing).
/// Catches the `+=` -> `*=` mutation.
#[test]
fn at_branch_increments_not_multiplies() {
    let mut src = String::new();
    for _ in 0..70 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    // With `*=`: `consecutive_at` starts at 0, every `@` does 0*1+0=0 or
    // 0*1=0; never > MAX. Original: every `@` is +=1, fires after 65.
    assert!(
        matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "increment-not-multiply: 70 @final must trigger AnnotationLimit, got {res:?}"
    );
}
