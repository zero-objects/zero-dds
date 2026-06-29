//! TS-cluster + Bug N — REAL XCDR2 wire round-trip tests.
//!
//! These tests do not merely assert that generated code compiles: they
//! generate TypeScript for IDL exercising the four fixed defects (union member,
//! fixed-size arrays, enum-typed member, non-literal const), then run the
//! emitted `TypeSupport.encode` → `.decode` against the REAL `@zerodds/cdr`
//! runtime (`crates/ts-node/dist/cdr`) under Node, and assert the recovered
//! sample equals the input.
//!
//! **Prerequisite:** `node` (>= 22, native TS type-stripping) on PATH and the
//! built `crates/ts-node/dist/cdr` runtime. Tests are skipped if either is
//! missing — they never falsely pass.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ts::generate_ts_source;

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Absolute path to the built `@zerodds/cdr` runtime dist directory.
fn cdr_dist() -> PathBuf {
    // crates/idl-ts/tests/.. -> crates/idl-ts -> crates -> crates/ts-node/dist/cdr
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.join("../ts-node/dist/cdr")
}

/// Generates TS for `idl`, lays it out next to a JS `@zerodds/types` value-stub
/// and the real `@zerodds/cdr` runtime, then runs `driver` (a JS snippet that
/// imports `./generated.ts` and prints a result line). Returns driver stdout.
fn run_driver(idl: &str, driver: &str) -> Option<String> {
    if !node_available() {
        eprintln!("WARNING: skipping wire round-trip — no node in PATH");
        return None;
    }
    let cdr = cdr_dist();
    if !cdr.join("index.js").exists() {
        eprintln!(
            "WARNING: skipping wire round-trip — built cdr runtime missing at {}",
            cdr.display()
        );
        return None;
    }

    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let ts_source = generate_ts_source(&ast).expect("gen");

    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    // @zerodds/types — value stubs (registerType etc. are runtime no-ops here).
    let types_dir = root.join("node_modules/@zerodds/types");
    std::fs::create_dir_all(&types_dir).unwrap();
    std::fs::write(
        types_dir.join("package.json"),
        r#"{"name":"@zerodds/types","version":"0.0.0","type":"module","main":"index.js"}"#,
    )
    .unwrap();
    std::fs::write(
        types_dir.join("index.js"),
        "export function registerType() {}\n\
         export function makeChar(s) { return s; }\n\
         export function makeWChar(s) { return s; }\n\
         export function makeLongDouble(b) { return { bytes: b }; }\n",
    )
    .unwrap();

    // @zerodds/cdr — re-export the real built runtime by absolute path.
    let cdr_dir = root.join("node_modules/@zerodds/cdr");
    std::fs::create_dir_all(&cdr_dir).unwrap();
    std::fs::write(
        cdr_dir.join("package.json"),
        r#"{"name":"@zerodds/cdr","version":"0.0.0","type":"module","main":"index.js"}"#,
    )
    .unwrap();
    let cdr_abs = cdr.canonicalize().expect("canonicalize cdr dist");
    let cdr_url = format!("file://{}/index.js", cdr_abs.display());
    std::fs::write(
        cdr_dir.join("index.js"),
        format!("export * from {cdr_url:?};\n"),
    )
    .unwrap();

    std::fs::write(root.join("generated.ts"), &ts_source).unwrap();
    std::fs::write(root.join("driver.mjs"), driver).unwrap();

    let output = Command::new("node")
        .current_dir(root)
        .arg("--experimental-strip-types")
        .arg("driver.mjs")
        .output()
        .expect("spawn node");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        panic!(
            "node driver FAILED:\n--- source ---\n{ts_source}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        );
    }
    Some(stdout)
}

/// TS-cluster #1 — union member round-trips over the wire (each branch).
#[test]
fn union_member_roundtrips() {
    let idl = "union Reading switch (long) { \
                 case 1: long count; \
                 case 2: double temp; \
                 default: boolean flag; }; \
               struct Telemetry { long seq; Reading reading; };";
    let driver = r#"
import { TelemetryTypeSupport } from "./generated.ts";
function rt(s) {
    const bytes = TelemetryTypeSupport.encode(s, "le");
    return TelemetryTypeSupport.decode(bytes);
}
const a = { seq: 7, reading: { discriminator: 1, count: 1234 } };
const b = { seq: 8, reading: { discriminator: 2, temp: 3.5 } };
const c = { seq: 9, reading: { discriminator: 99, flag: true } };
const ra = rt(a), rb = rt(b), rc = rt(c);
const ok =
    ra.seq === 7 && ra.reading.discriminator === 1 && ra.reading.count === 1234 &&
    rb.seq === 8 && rb.reading.discriminator === 2 && rb.reading.temp === 3.5 &&
    rc.seq === 9 && rc.reading.discriminator === 99 && rc.reading.flag === true;
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify({ra, rb, rc}));
"#;
    if let Some(out) = run_driver(idl, driver) {
        assert!(
            out.contains("RESULT PASS"),
            "union round-trip failed: {out}"
        );
    }
}

/// TS-cluster #1 (enum-discriminated) — a union whose discriminator is an enum
/// round-trips, including the implicit/default branch (combo `Reading`-shape).
/// (Top-level, not module-nested, so Node's type-stripping accepts it; the
/// module-nested form is covered by `compiles_conformance_fixtures` tsc-check.)
#[test]
fn enum_disc_union_member_roundtrips() {
    let idl = "enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT }; \
               union Reading switch (Mode) { \
                 case MODE_IDLE:   long      idleTicks; \
                 case MODE_ACTIVE: double    activeRate; \
                 default:          string    faultCode; }; \
               struct Wrap { long seq; Reading reading; };";
    let driver = r#"
import { Mode, WrapTypeSupport } from "./generated.ts";
function rt(s) { return WrapTypeSupport.decode(WrapTypeSupport.encode(s, "le")); }
const a = { seq: 1, reading: { discriminator: Mode.MODE_IDLE, idleTicks: 77 } };
const b = { seq: 2, reading: { discriminator: Mode.MODE_ACTIVE, activeRate: 9.25 } };
const c = { seq: 3, reading: { discriminator: Mode.MODE_FAULT, faultCode: "E13" } };
const ra = rt(a), rb = rt(b), rc = rt(c);
const ok =
    ra.reading.discriminator === Mode.MODE_IDLE && ra.reading.idleTicks === 77 &&
    rb.reading.discriminator === Mode.MODE_ACTIVE && rb.reading.activeRate === 9.25 &&
    rc.reading.discriminator === Mode.MODE_FAULT && rc.reading.faultCode === "E13";
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify({ra, rb, rc}));
"#;
    if let Some(out) = run_driver(idl, driver) {
        assert!(
            out.contains("RESULT PASS"),
            "enum-disc union round-trip failed: {out}"
        );
    }
}

/// TS-cluster #2 — fixed-size arrays (1-D, multi-dim, array-of-struct) round-trip.
#[test]
fn fixed_arrays_roundtrip() {
    let idl = "struct Point { long x; long y; }; \
               struct Arrays { \
                 long vec[3]; \
                 long grid[2][2]; \
                 Point shape[2]; \
                 double cube[2][2][2]; };";
    let driver = r#"
import { ArraysTypeSupport } from "./generated.ts";
const s = {
    vec: [10, 20, 30],
    grid: [[1, 2], [3, 4]],
    shape: [{ x: 1, y: 2 }, { x: 3, y: 4 }],
    cube: [[[1.5, 2.5], [3.5, 4.5]], [[5.5, 6.5], [7.5, 8.5]]],
};
const bytes = ArraysTypeSupport.encode(s, "le");
const r = ArraysTypeSupport.decode(bytes);
const ok =
    JSON.stringify(r.vec) === JSON.stringify(s.vec) &&
    JSON.stringify(r.grid) === JSON.stringify(s.grid) &&
    JSON.stringify(r.shape) === JSON.stringify(s.shape) &&
    JSON.stringify(r.cube) === JSON.stringify(s.cube);
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify(r));
"#;
    if let Some(out) = run_driver(idl, driver) {
        assert!(
            out.contains("RESULT PASS"),
            "array round-trip failed: {out}"
        );
    }
}

/// TS-cluster #3 — enum-typed member round-trips through the ordinal map.
#[test]
fn enum_member_roundtrips() {
    let idl = "enum Color { RED, GREEN, BLUE }; \
               enum Mode { @value(5) FAST, SLOW }; \
               struct UsesEnum { Color primary; Mode mode; long n; };";
    let driver = r#"
import { UsesEnumTypeSupport } from "./generated.ts";
function rt(s) { return UsesEnumTypeSupport.decode(UsesEnumTypeSupport.encode(s, "le")); }
const s = { primary: "BLUE", mode: "SLOW", n: 42 };
const r = rt(s);
// BLUE is ordinal 2; SLOW is ordinal 6 (FAST=@value(5)). The wire carries the
// ordinal, and the recovered value must be the same string member.
const ok = r.primary === "BLUE" && r.mode === "SLOW" && r.n === 42;
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify(r));
"#;
    if let Some(out) = run_driver(idl, driver) {
        assert!(out.contains("RESULT PASS"), "enum round-trip failed: {out}");
    }
}

/// Bug N — a non-literal const (`const M = N * 2`) folds to its computed value,
/// and an array bound expressed via that const has the right wire length.
#[test]
fn non_literal_const_folds_and_drives_array_bound() {
    // BASE = 3; DERIVED = BASE * 2 + 1 = 7; SUM = BASE + DERIVED = 10.
    let idl = "const long BASE = 3; \
               const long DERIVED = BASE * 2 + 1; \
               const long SUM = BASE + DERIVED; \
               struct Holder { long vec[DERIVED]; long n; };";
    let driver = r#"
import { DERIVED, SUM, BASE, HolderTypeSupport } from "./generated.ts";
const constsOk = BASE === 3 && DERIVED === 7 && SUM === 10;
const s = { vec: [1, 2, 3, 4, 5, 6, 7], n: 99 };
const r = HolderTypeSupport.decode(HolderTypeSupport.encode(s, "le"));
const arrOk = JSON.stringify(r.vec) === JSON.stringify(s.vec) && r.n === 99;
console.log("RESULT", (constsOk && arrOk) ? "PASS" : "FAIL", JSON.stringify({BASE, DERIVED, SUM, r}));
"#;
    if let Some(out) = run_driver(idl, driver) {
        assert!(
            out.contains("RESULT PASS"),
            "const-fold / array-bound round-trip failed: {out}"
        );
    }
}
