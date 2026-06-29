//! Cross-backend IDL-feature conformance matrix.
//!
//! Drives every (feature fixture × backend) **generation** cell through the
//! real `zerodds-idlc` CLI and compares the pass/fail result against the
//! expected matrix below. The point is regression *and* progress detection:
//! - a cell that is expected `Ok` but now fails  → a regression (test fails);
//! - a cell that is expected `Err` but now passes → a backend gained the
//!   feature; update the table (test fails, telling you to).
//!
//! Fixtures live in `tests/conformance/fixtures/*.idl`, one per IDL feature
//! (incl. complex cases like recursion). This is the generation gate only —
//! semantic/compile correctness for individual features is proven by each
//! codegen crate's own clang/cargo/javac roundtrip tests (e.g. the recursive
//! tree roundtrip in `zerodds-idl-cpp`). Compile-level known-opens are noted in
//! `docs`/`internal` and in the per-cell comments.
//!
//! The `C` backend is a deliberately narrow "Foundation scope" profile
//! (structs of directly-encodable members); its many `Err` cells are by design
//! (tracked as Bug C), not regressions.

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_zerodds-idlc");

/// All seven backend flags, in matrix-column order.
const BACKENDS: [&str; 7] = ["c", "cpp", "rust", "ts", "csharp", "java", "python"];

/// Expected generation outcome per (feature, [c, cpp, rust, ts, csharp, java,
/// python]). `true` = generation must succeed, `false` = known-unsupported
/// (see the trailing comment for the tracking bug).
const EXPECTED: &[(&str, [bool; 7])] = &[
    //                       c      cpp    rust   ts     c#     java   py
    ("01_primitives", [true, true, true, true, true, true, true]),
    ("02_strings", [true, true, true, true, true, true, true]), // C: Bug C
    ("03_enums", [true, true, true, true, true, true, true]),   // C scope widened (Bug C)
    (
        "04_extensibility",
        [true, true, true, true, true, true, true],
    ),
    (
        "05_nested_structs",
        [true, true, true, true, true, true, true],
    ), // C scope widened (Bug C)
    ("06_typedefs", [true, true, true, true, true, true, true]), // C: typedef-to-aggregate still open
    ("07_sequences", [true, true, true, true, true, true, true]), // C: seq still open (java Bug J fixed)
    ("08_arrays", [true, true, true, true, true, true, true]),    // C scope widened (Bug C)
    ("09_unions", [true, true, true, true, true, true, true]), // C: Bug C (rust Bug H, py Bug I fixed)
    ("10_keys", [true, true, true, true, true, true, true]),
    ("11_optional", [true, true, true, true, true, true, true]),
    (
        "12_bitset_bitmask",
        [true, true, true, true, true, true, true],
    ), // C: Bug C
    ("13_maps", [true, true, true, true, true, true, true]), // C: Bug C (py Bug K fixed)
    ("14_recursion", [true, true, true, true, true, true, true]), // C: Bug C (self-recursion fixed: Bug G)
    (
        "14b_mutual_recursion",
        [true, true, true, true, true, true, true],
    ), // C: Bug C; cpp compile-open: Bug G2
    ("15_constants", [true, true, true, true, true, true, true]), // ts Bug N + py Bug M fixed; C: const array bound still open
    ("16_modules", [true, true, true, true, true, true, true]),   // C scope widened (Bug C)
    (
        "17_forward_decl",
        [true, true, true, true, true, true, true],
    ), // C: Bug C
    ("18_annotations", [true, true, true, true, true, true, true]),
    // Combined-feature topic type. Generation passes on all but C — but several
    // of these ✓ cells FAIL a real-DCPS roundtrip (union dropped, array decode,
    // module flattening, …); see internal/idl-codegen/real-dcps-findings.md.
    ("20_mixed_combo", [true, true, true, true, true, true, true]), // C: Bug C
];

fn fixture_path(name: &str) -> String {
    format!(
        "{}/tests/conformance/fixtures/{name}.idl",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Runs `zerodds-idlc generate <fixture> --<backend> -o <tmp>` with stdin
/// closed (the CLI reads stdin; an open inherited stdin stalls a loop) and
/// returns whether code generation succeeded.
fn generates(fixture: &str, backend: &str) -> bool {
    let out = tempfile::tempdir().expect("tempdir");
    Command::new(BIN)
        .arg("generate")
        .arg(fixture_path(fixture))
        .arg(format!("--{backend}"))
        .arg("-o")
        .arg(out.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn conformance_matrix_matches_expected() {
    let mut mismatches: Vec<String> = Vec::new();
    for (feature, expected) in EXPECTED {
        for (i, backend) in BACKENDS.iter().enumerate() {
            let got = generates(feature, backend);
            if got != expected[i] {
                let dir = if got {
                    "now PASSES but expected Err — a backend gained this feature; update EXPECTED"
                } else {
                    "REGRESSION: expected Ok but generation failed"
                };
                mismatches.push(format!("  {feature} / {backend}: {dir}"));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "conformance matrix drift:\n{}",
        mismatches.join("\n")
    );
}
