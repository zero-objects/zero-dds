//! L3 — cross-PSM byte-identity over the FEATURE corpus (features.idl):
//! WStr, Mut, Bits, Tree, Arr, Prim.
//!
//! For each feature this test generates the TS PSM straight from
//! `generate_ts_source`, lays it next to the real `@zerodds/cdr` runtime, then
//! under Node:
//!   * ENCODE the CANONICAL.md sample and assert the bytes are byte-identical
//!     to the committed Rust reference golden
//!     (`_interop/goldens/<feature>.rust.bin`, produced by the cross-vendor-
//!     validated `zerodds-cdr` core), and
//!   * DECODE that same Rust golden and assert every field == canonical.
//!
//! This locks the TS backend to the XCDR2-LE wire of the Rust reference per
//! OMG XTypes 1.3 §7.4.3 (wstring §7.4.4.6, @mutable EMHEADER §7.4.3.4.2,
//! bitset/bitmask holder §7.4.5, recursion + collection DHEADER §7.4.3.5).
//!
//! Prerequisite: `node` (>= 22, native TS type-stripping) on PATH and the
//! built `crates/ts-node/dist/cdr` runtime. Skipped (never falsely passes) if
//! either is missing.

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

/// Removes the outer `module <name> { … }` wrapper so the generated TS is a
/// set of top-level declarations (Node-strippable), keeping the inner body
/// verbatim. The module wrapper does not affect the XCDR2 wire.
fn strip_module_wrapper(idl: &str, name: &str) -> String {
    let open = format!("module {name} {{");
    let Some(start) = idl.find(&open) else {
        return idl.to_string();
    };
    let body_start = start + open.len();
    // The matching close brace is the LAST `}` in the file (the module is the
    // sole top-level construct in features.idl).
    let body_end = idl.rfind('}').expect("module close brace");
    idl[body_start..body_end].to_string()
}

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn interop_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../zerodds-examples/idl-conformance/_interop")
        .canonicalize()
        .expect("interop dir")
}

fn cdr_dist() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ts-node/dist/cdr")
}

/// Generates TS for `features.idl`, then runs a Node driver (an mjs snippet
/// that imports `./generated.ts`) and returns its stdout. The driver receives
/// the absolute goldens dir as `process.argv[2]`.
fn run_feature_driver(driver: &str) -> Option<String> {
    if !node_available() {
        eprintln!("WARNING: skipping feature golden wire test — no node in PATH");
        return None;
    }
    let cdr = cdr_dist();
    if !cdr.join("index.js").exists() {
        eprintln!(
            "WARNING: skipping feature golden wire test — built cdr runtime missing at {}",
            cdr.display()
        );
        return None;
    }

    let idl_path = interop_dir().join("features.idl");
    let idl = std::fs::read_to_string(&idl_path).expect("read features.idl");
    // Node's `--experimental-strip-types` rejects TS `namespace` (non-erasable),
    // which the codegen emits for the IDL `module feat`. The module wrapper has
    // NO effect on the XCDR2 wire (it only nests TS names), so for this in-crate
    // wire gate we strip `module feat { … }` to get top-level `feat.*` symbols
    // as bare top-level declarations. The canonical interop harness
    // (`_interop/ts`, run with tsx) exercises the namespaced gen file verbatim.
    let idl = strip_module_wrapper(&idl, "feat");
    let ast = zerodds_idl::parse(&idl, &ParserConfig::default()).expect("parse features.idl");
    let ts_source = generate_ts_source(&ast).expect("gen features TS");

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

    let goldens = interop_dir().join("goldens");
    let output = Command::new("node")
        .current_dir(root)
        .arg("--experimental-strip-types")
        .arg("driver.mjs")
        .arg(goldens.to_str().unwrap())
        .output()
        .expect("spawn node");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        panic!(
            "node feature driver FAILED:\n--- source ---\n{ts_source}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        );
    }
    Some(stdout)
}

/// ENCODE each feature → byte-identical with the Rust reference golden, and
/// DECODE each Rust golden → every field == canonical.
#[test]
fn feature_goldens_byte_identical_and_decode_rust() {
    // The driver imports the generated feat namespace, encodes the canonical
    // samples, byte-compares against goldens/<f>.rust.bin, and decodes those
    // goldens back. It prints one RESULT line per feature.
    let driver = r#"
import { readFileSync } from "node:fs";
import {
    Perm,
    WStrTypeSupport, MutTypeSupport, BitsTypeSupport,
    TreeTypeSupport, ArrTypeSupport, PrimTypeSupport,
} from "./generated.ts";
const DIR = process.argv[2];

function hex(b) { return Array.from(b, x => x.toString(16).padStart(2, "0")).join(""); }
function readGolden(name) {
    const buf = readFileSync(`${DIR}/${name}.rust.bin`);
    return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
}
function bytesEq(a, b) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
    return true;
}

const WSTR = { label: "café", text: "日本語\u{1F389}" };
const MUT = { a: 1000000, b: 2.5, c: "ok" };
const BITS = { perm: (Perm.READ | Perm.EXEC) >>> 0, flags: { kind: 5, prio: 20 } };
const TREE = { value: 1, kids: [ { value: 2, kids: [ { value: 4, kids: [] } ] }, { value: 3, kids: [] } ] };
const ARR = { grid: [[10,11,12],[13,14,15]], shape: [ {x:1,y:2}, {x:3,y:4} ] };
const PRIM = { i8:-128,u8:255,i16:-32768,u16:65535,i32:-2147483648,u32:4294967295,
    i64:-9223372036854775808n,u64:18446744073709551615n,f32:3.5,f64:-1234.5,b:true,o:0xab,ch:"Z" };

const cases = [
    ["wstr", WStrTypeSupport, WSTR, (g) => g.label === "café" && g.text === "日本語\u{1F389}"],
    ["mut",  MutTypeSupport,  MUT,  (g) => g.a === 1000000 && g.b === 2.5 && g.c === "ok"],
    ["bits", BitsTypeSupport, BITS, (g) => g.perm === 5 && g.flags.kind === 5 && g.flags.prio === 20],
    ["tree", TreeTypeSupport, TREE, (g) =>
        g.value === 1 && g.kids.length === 2 &&
        g.kids[0].value === 2 && g.kids[0].kids.length === 1 && g.kids[0].kids[0].value === 4 &&
        g.kids[0].kids[0].kids.length === 0 &&
        g.kids[1].value === 3 && g.kids[1].kids.length === 0],
    ["arr",  ArrTypeSupport,  ARR,  (g) =>
        g.grid[0][0]===10 && g.grid[0][2]===12 && g.grid[1][0]===13 && g.grid[1][2]===15 &&
        g.shape[0].x===1 && g.shape[0].y===2 && g.shape[1].x===3 && g.shape[1].y===4],
    ["prim", PrimTypeSupport, PRIM, (g) =>
        g.i8===-128 && g.u8===255 && g.i16===-32768 && g.u16===65535 &&
        g.i32===-2147483648 && g.u32===4294967295 &&
        g.i64===-9223372036854775808n && g.u64===18446744073709551615n &&
        g.f32===3.5 && g.f64===-1234.5 && g.b===true && g.o===0xab && g.ch==="Z"],
];

for (const [name, TS, sample, check] of cases) {
    const enc = TS.encode(sample, "le");
    const gold = readGolden(name);
    const same = bytesEq(enc, gold);
    const dec = TS.decode(gold);
    const decOk = check(dec);
    console.log(`RESULT ${name} ENC=${same ? "MATCH" : "DIFF"} DEC=${decOk ? "OK" : "BAD"} enc=${hex(enc)} gold=${hex(gold)}`);
}
"#;

    if let Some(out) = run_feature_driver(driver) {
        for feature in ["wstr", "mut", "bits", "tree", "arr", "prim"] {
            let needle = format!("RESULT {feature} ENC=MATCH DEC=OK");
            assert!(
                out.contains(&needle),
                "feature {feature} did not converge with the Rust golden:\n{out}"
            );
        }
    }
}
