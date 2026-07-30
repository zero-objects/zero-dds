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

/// All 17 backend flags, in matrix-column order.
const BACKENDS: [&str; 17] = [
    "c", "cpp", "rust", "ts", "csharp", "java", "python", "go", "ada", "zig", "nim", "d", "elixir",
    "ocaml", "julia", "lua", "swift",
];

/// Expected generation outcome per (feature, [c, cpp, rust, ts, csharp, java,
/// python, go, ada, zig, nim, d, elixir, ocaml, julia, lua, swift]). `true` =
/// generation must succeed, `false` = known-unsupported (see the trailing
/// comment for the tracking bug).
//
// Thin-backend scope (go, ada, zig, nim, d, elixir, ocaml, julia, lua, swift):
// several `false` cells below are genuine feature limitations of these
// flattening backends, surfaced as a HARD generation error since #21/#24
// (`53132d54`, `622a50d5`). Before those merges the same cells exited 0 while
// SILENTLY dropping the unsupported type/member from the emitted file — a
// false green the exit-code gate could not see (verified: at the matrix's
// original all-`true` commit `9397614d`, e.g. `06_typedefs --go` emitted the
// wire prelude with NO `UsesTypedefs` record). Honest error > silent drop, so
// these cells now correctly read `false`. Per-cell reason in the trailing
// comment.
const EXPECTED: &[(&str, [bool; 17])] = &[
    //                       c      cpp    rust   ts     c#     java   py     go     ada    zig    nim    d      elixir ocaml  julia  lua    swift
    (
        "01_primitives",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ),
    (
        "02_strings",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ), // C: Bug C
    (
        "03_enums",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ), // C scope widened (Bug C)
    (
        "04_extensibility",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ),
    (
        "05_nested_structs",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ), // C scope widened (Bug C)
    (
        "06_typedefs",
        [
            true, true, true, true, true, true, true, false, false, false, false, false, false,
            false, false, false, false,
        ],
    ), // C: typedef-to-aggregate still open. go..swift: typedef-to-array (`long M[3][3]`) / typedef-to-`sequence<primitive>` not supported by the flattening backends (was a silent drop pre-#21).
    (
        "07_sequences",
        [
            true, true, true, true, true, true, true, true, false, true, true, true, true, true,
            true, true, true,
        ],
    ), // ada: sequence<primitive>/<string> noch offen (limit-blockiert). go/zig/nim/d/elixir/ocaml/julia/lua/swift: unterstützt seit thin-F Welle 1.
    (
        "08_arrays",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ), // ocaml: array-of-struct-Default seit Welle 2 (F27) unterstützt.
    (
        "09_unions",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ), // Alle Backends: non-integer union-Discriminator (enum/char/bool) seit Welle 2 (F11) unterstützt.
    (
        "10_keys",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ),
    (
        "11_optional",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ),
    (
        "12_bitset_bitmask",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ), // Alle Backends: bitset/bitmask unterstützt (thin-F Welle 1 + go-Fix).
    (
        "13_maps",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ), // C: Bug C (py Bug K fixed)
    (
        "14_recursion",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ), // C: Bug C (self-recursion fixed: Bug G); go/ada/zig/nim/d/elixir/ocaml/julia/lua/swift: generation succeeds, TypeObject emission skipped (RecursiveType, warning only)
    (
        "14b_mutual_recursion",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ), // C: Bug C; cpp compile-open: Bug G2; go/ada/zig/nim/d/elixir/ocaml/julia/lua/swift: generation succeeds, TypeObject emission skipped (RecursiveType, warning only)
    (
        "15_constants",
        [
            true, true, true, true, true, true, true, true, false, false, false, false, false,
            false, false, false, false,
        ],
    ), // ts Bug N + py Bug M fixed; C: const array bound still open. go: central const-eval resolves `long items[MAX_ITEMS]` (audit P1 "Const-Eval driftet"). ada..swift: const-expression array size still unsupported (literal-only array_size in those backends).
    (
        "16_modules",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ), // C scope widened (Bug C)
    (
        "17_forward_decl",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, false,
            true, true, true,
        ],
    ), // C: Bug C. ocaml: recursive `Node`/`Variant` — no default for the array-of-struct element type.
    (
        "18_annotations",
        [
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, true,
        ],
    ),
    // Combined-feature topic type. It exercises the union/typedef/sequence/const
    // features above, so the flattening backends (go..swift) hit the same
    // limitations here and fail generation; the seven mature backends pass.
    (
        "20_mixed_combo",
        [
            true, true, true, true, true, true, true, false, false, false, false, false, false,
            false, false, false, false,
        ],
    ), // C: Bug C. go..swift: inherits the union/typedef/const limitations of the individual feature rows.
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
