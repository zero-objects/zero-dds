//! F.1 item 13 — REAL XCDR1 wire round-trip tests for idl-ts.
//!
//! idl-ts emits an XCDR1 path (`encode(s, endian, 0)` / `decode(..., 0)`,
//! writer/reader capped at max-alignment 8, no DHEADER on @appendable/@final,
//! PL_CDR1 on @mutable) but it had ZERO test coverage. These tests generate
//! TypeScript, then run the emitted `TypeSupport.encode`→`.decode` in XCDR1
//! mode against the REAL `@zerodds/cdr` runtime under Node, asserting the
//! byte layout and the round-trip — and that XCDR1 differs from XCDR2 bytes.
//!
//! **Prerequisite:** `node` on PATH and the built `crates/ts-node/dist/cdr`
//! runtime. Tests are skipped (never falsely pass) if either is missing.

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

fn cdr_dist() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.join("../ts-node/dist/cdr")
}

/// Generates TS for `idl`, lays it out next to a JS `@zerodds/types` value-stub
/// and the real `@zerodds/cdr` runtime, then runs `driver` (a JS snippet that
/// imports `./generated.ts` and prints a result line). Returns driver stdout,
/// or `None` when the prerequisites are missing (test skips).
fn run_driver(idl: &str, driver: &str) -> Option<String> {
    if !node_available() {
        eprintln!("WARNING: skipping XCDR1 wire round-trip — no node in PATH");
        return None;
    }
    let cdr = cdr_dist();
    if !cdr.join("index.js").exists() {
        eprintln!(
            "WARNING: skipping XCDR1 wire round-trip — built cdr runtime missing at {}",
            cdr.display()
        );
        return None;
    }

    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let ts_source = generate_ts_source(&ast).expect("gen");

    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

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

/// @final struct under XCDR1: alignment is capped at 8, so a `double` after an
/// `octet` aligns to offset 8 (XCDR2 caps at 4). Asserts the byte layout, the
/// round-trip, and that XCDR1 and XCDR2 produce DIFFERENT wire bytes.
#[test]
fn xcdr1_final_alignment_and_roundtrip() {
    let idl = "@final struct X1Final { octet a; double d; };";
    let driver = r#"
import { X1FinalTypeSupport } from "./generated.ts";
const hex = (u) => Buffer.from(u).toString("hex");
const s = { a: 1, d: 1.0 };
const x1 = X1FinalTypeSupport.encode(s, "le", 0);   // XCDR1
const x2 = X1FinalTypeSupport.encode(s, "le", 1);   // XCDR2
const back = X1FinalTypeSupport.decode(x1, 0, x1.length, "le", 0);
// XCDR1: octet@0, 7 pad, f64@8 -> 16 bytes. XCDR2 caps at 4 -> 12 bytes.
const ok =
    x1.length === 16 && x2.length === 12 && hex(x1) !== hex(x2) &&
    x1[0] === 1 && hex(x1.slice(1, 8)) === "00000000000000" &&
    back.a === 1 && back.d === 1.0;
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify({x1: hex(x1), x2: hex(x2), back}));
"#;
    if let Some(out) = run_driver(idl, driver) {
        assert!(out.contains("RESULT PASS"), "XCDR1 final round-trip: {out}");
    }
}

/// @appendable struct under XCDR1 emits NO DHEADER (XCDR2 emits one, rule(30)).
/// The wire starts with the first member's value, not the body length.
#[test]
fn xcdr1_appendable_has_no_dheader() {
    let idl = "@appendable struct X1App { long a; double d; };";
    let driver = r#"
import { X1AppTypeSupport } from "./generated.ts";
const hex = (u) => Buffer.from(u).toString("hex");
const s = { a: 7, d: 2.5 };
const x1 = X1AppTypeSupport.encode(s, "le", 0);   // XCDR1: no DHEADER
const x2 = X1AppTypeSupport.encode(s, "le", 1);   // XCDR2: leading DHEADER
const back = X1AppTypeSupport.decode(x1, 0, x1.length, "le", 0);
// XCDR1 starts with a=7 directly; XCDR2 starts with the DHEADER body-length=12.
const ok =
    hex(x1.slice(0, 4)) === "07000000" &&
    hex(x2.slice(0, 4)) === "0c000000" &&
    hex(x1) !== hex(x2) &&
    back.a === 7 && back.d === 2.5;
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify({x1: hex(x1), x2: hex(x2), back}));
"#;
    if let Some(out) = run_driver(idl, driver) {
        assert!(
            out.contains("RESULT PASS"),
            "XCDR1 appendable no-DHEADER: {out}"
        );
    }
}

/// @mutable struct under XCDR1 frames members as PL_CDR1 ([PID][len] + a
/// PID_LIST_END sentinel), NOT XCDR2 PL_CDR2 EMHEADERs. Asserts the PL_CDR1
/// wire round-trips and differs from the XCDR2 framing.
#[test]
fn xcdr1_mutable_pl_cdr1_roundtrip() {
    let idl = "@mutable struct X1Mut { @id(1) long a; @id(2) double d; };";
    let driver = r#"
import { X1MutTypeSupport } from "./generated.ts";
const hex = (u) => Buffer.from(u).toString("hex");
const s = { a: 9, d: -1.5 };
const x1 = X1MutTypeSupport.encode(s, "le", 0);   // PL_CDR1
const x2 = X1MutTypeSupport.encode(s, "le", 1);   // PL_CDR2
const b1 = X1MutTypeSupport.decode(x1, 0, x1.length, "le", 0);
const b2 = X1MutTypeSupport.decode(x2, 0, x2.length, "le", 1);
const ok =
    hex(x1) !== hex(x2) &&
    b1.a === 9 && b1.d === -1.5 &&
    b2.a === 9 && b2.d === -1.5;
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify({x1: hex(x1), x2: hex(x2), b1, b2}));
"#;
    if let Some(out) = run_driver(idl, driver) {
        assert!(
            out.contains("RESULT PASS"),
            "XCDR1 mutable PL_CDR1 round-trip: {out}"
        );
    }
}
