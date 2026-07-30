//! Adversarial edge-hardening — REAL XCDR2 wire round-trips for the cases that
//! break TS adapters the moment they leave hello-world.
//!
//! Each test generates TypeScript for a small IDL fixture, then runs the
//! emitted `TypeSupport.encode` → `.decode` against the REAL `@zerodds/cdr`
//! runtime (`crates/ts-node/dist/cdr`) under Node and asserts the recovered
//! sample equals the input. They are skipped (never falsely pass) if `node`
//! or the built cdr runtime is missing.
//!
//! Coverage (the EDGE CHECKLIST):
//! 1. empty sequence / bounded sequence / string / wstring / map
//! 2. bound enforcement: exactly-N ok, over-N throws (seq / string / map)
//! 3. deep nesting: struct³, sequence<sequence<struct>>, map<string,struct-w/-seq>
//! 4. @optional aggregate present AND absent (absent stays undefined)
//! 5. union every branch incl. default; union as seq element and map value
//! 6. unicode: CJK + emoji in string and wstring
//! 7. arrays: array-of-struct, multi-dim, array-of-bounded-string
//! 8. extreme primitives: int*_MIN/MAX, uint*_MAX, float/double
//! 9. keyed: same @key different payload, key fields survive
//!
//! REGRESSIONS fixed by this sweep (verified by tests 2/3/sibling here):
//! - `sequence<sequence<T>>` / collection-in-map-value reused the loop var `_e`
//!   (`for (const _e of _e)`) and two sibling collection members reused
//!   `_seqtok` — both crashed/corrupted the encode. Now each collection loop
//!   gets a run-global unique temporary.
//! - bounded `sequence<T,N>`, `string<N>`, `map<K,V,N>` over N silently
//!   corrupted; they now throw `RangeError`.

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
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ts-node/dist/cdr")
}

/// Generates TS for `idl`, lays it out next to a JS `@zerodds/types` stub and
/// the real `@zerodds/cdr` runtime, runs `driver`, and returns its stdout.
/// Returns `None` (skip) when node or the cdr runtime is missing.
fn run_driver(idl: &str, driver: &str) -> Option<String> {
    if !node_available() {
        eprintln!("WARNING: skipping edge round-trip — no node in PATH");
        return None;
    }
    let cdr = cdr_dist();
    if !cdr.join("index.js").exists() {
        eprintln!(
            "WARNING: skipping edge round-trip — built cdr runtime missing at {}",
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

fn assert_pass(out: Option<String>, what: &str) {
    if let Some(out) = out {
        assert!(out.contains("RESULT PASS"), "{what} failed: {out}");
    }
}

/// Edge 1 — empty sequence / bounded sequence / string / wstring / map all
/// round-trip with count=0 (no crash).
#[test]
fn empty_collections_roundtrip() {
    let idl = "struct E { sequence<long> s; sequence<long,4> bs; \
                          string str; wstring ws; map<string,long> m; };";
    let driver = r#"
import { ETypeSupport } from "./generated.ts";
const s = { s: [], bs: [], str: "", ws: "", m: new Map() };
const r = ETypeSupport.decode(ETypeSupport.encode(s, "le"));
const ok = r.s.length === 0 && r.bs.length === 0 && r.str === "" &&
           r.ws === "" && r.m.size === 0;
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify({...r, m: [...r.m]}));
"#;
    assert_pass(run_driver(idl, driver), "empty-collections");
}

/// Edge 2 — bounded sequence / string / map: exactly N round-trips; over N
/// throws `RangeError` instead of silently corrupting.
#[test]
fn bound_enforcement() {
    let idl = "struct B { sequence<long,2> bs; string<3> name; map<string,long,2> m; };";
    let driver = r#"
import { BTypeSupport } from "./generated.ts";
// exactly N — must round-trip
const ok = BTypeSupport.decode(BTypeSupport.encode(
    { bs: [1, 2], name: "abc", m: new Map([["a",1],["b",2]]) }, "le"));
const exactOk = JSON.stringify(ok.bs) === "[1,2]" && ok.name === "abc" && ok.m.size === 2;
function over(s) { try { BTypeSupport.encode(s, "le"); return false; } catch (e) { return true; } }
const seqThrew  = over({ bs: [1,2,3], name: "ab", m: new Map() });
const strThrew  = over({ bs: [],      name: "abcd", m: new Map() });
const mapThrew  = over({ bs: [],      name: "ab", m: new Map([["a",1],["b",2],["c",3]]) });
const ok2 = exactOk && seqThrew && strThrew && mapThrew;
console.log("RESULT", ok2 ? "PASS" : "FAIL", JSON.stringify({exactOk, seqThrew, strThrew, mapThrew}));
"#;
    assert_pass(run_driver(idl, driver), "bound-enforcement");
}

/// Edge 3 — deep nesting: struct³, `sequence<sequence<struct>>`, and
/// `map<string, struct-with-a-sequence>`. (The nested-sequence case was the
/// `for (const _e of _e)` crash.)
#[test]
fn deep_nesting_roundtrip() {
    let idl = "struct A { long x; }; \
               struct B2 { A a; long y; }; \
               struct C { B2 b; long z; }; \
               struct SS { sequence<sequence<A>> grid; }; \
               struct WithSeq { sequence<long> nums; }; \
               struct MV { map<string,WithSeq> m; };";
    let driver = r#"
import { CTypeSupport, SSTypeSupport, MVTypeSupport } from "./generated.ts";
const c  = { b: { a: { x: 1 }, y: 2 }, z: 3 };
const ss = { grid: [[{x:1},{x:2}],[{x:3}]] };
const mv = { m: new Map([["a",{nums:[1,2,3]}],["b",{nums:[]}]]) };
const rc  = CTypeSupport.decode(CTypeSupport.encode(c, "le"));
const rss = SSTypeSupport.decode(SSTypeSupport.encode(ss, "le"));
const rmv = MVTypeSupport.decode(MVTypeSupport.encode(mv, "le"));
const ok =
    JSON.stringify(rc) === JSON.stringify(c) &&
    JSON.stringify(rss) === JSON.stringify(ss) &&
    JSON.stringify([...rmv.m]) === JSON.stringify([...mv.m]);
console.log("RESULT", ok ? "PASS" : "FAIL",
    JSON.stringify({rc, rss, mv: [...rmv.m]}));
"#;
    assert_pass(run_driver(idl, driver), "deep-nesting");
}

/// Edge 3b — sibling and nested *bounded* collections (the `_seqtok` redeclare
/// crash): two bounded collection members in one struct plus a bounded
/// sequence-of-bounded-sequence; exactly-N ok, over-N throws at both levels.
#[test]
fn sibling_and_bounded_nested_collections() {
    let idl = "struct A { long x; }; \
               struct BSS { sequence<A,2> items; sequence<sequence<long,3>,2> nn; };";
    let driver = r#"
import { BSSTypeSupport } from "./generated.ts";
const ok = BSSTypeSupport.decode(BSSTypeSupport.encode(
    { items: [{x:1},{x:2}], nn: [[1,2],[3]] }, "le"));
const exactOk =
    JSON.stringify(ok.items) === JSON.stringify([{x:1},{x:2}]) &&
    JSON.stringify(ok.nn) === JSON.stringify([[1,2],[3]]);
function over(s) { try { BSSTypeSupport.encode(s, "le"); return false; } catch (e) { return true; } }
const outerThrew = over({ items: [{x:1},{x:2},{x:3}], nn: [] });
const innerThrew = over({ items: [], nn: [[1,2,3,4]] });
const okAll = exactOk && outerThrew && innerThrew;
console.log("RESULT", okAll ? "PASS" : "FAIL", JSON.stringify({exactOk, outerThrew, innerThrew}));
"#;
    assert_pass(
        run_driver(idl, driver),
        "sibling/bounded-nested-collections",
    );
}

/// Edge 4 — @optional aggregates: optional sequence, nested struct, string;
/// present round-trips, absent stays `undefined` (not a zero-value).
#[test]
fn optional_aggregates_roundtrip() {
    let idl = "struct Inner { long x; }; \
               struct O { @optional sequence<long> s; \
                          @optional Inner nested; \
                          @optional string note; };";
    let driver = r#"
import { OTypeSupport } from "./generated.ts";
const present = { s: [1,2], nested: { x: 5 }, note: "hi" };
const absent  = { s: undefined, nested: undefined, note: undefined };
const rp = OTypeSupport.decode(OTypeSupport.encode(present, "le"));
const ra = OTypeSupport.decode(OTypeSupport.encode(absent, "le"));
const ok =
    JSON.stringify(rp.s) === "[1,2]" && rp.nested.x === 5 && rp.note === "hi" &&
    ra.s === undefined && ra.nested === undefined && ra.note === undefined;
console.log("RESULT", ok ? "PASS" : "FAIL",
    JSON.stringify({rp, raDefined: {s: ra.s !== undefined, nested: ra.nested !== undefined, note: ra.note !== undefined}}));
"#;
    assert_pass(run_driver(idl, driver), "optional-aggregates");
}

/// Edge 5 — union as a sequence element and as a map value, every branch
/// including the default discriminator.
#[test]
fn union_in_collections_roundtrip() {
    let idl = "union U switch(long){ case 1: long a; case 2: double b; default: boolean c; }; \
               struct SU { sequence<U> us; }; \
               struct MU { map<string,U> m; };";
    let driver = r#"
import { SUTypeSupport, MUTypeSupport } from "./generated.ts";
const su = { us: [ {discriminator:1,a:11}, {discriminator:2,b:2.5}, {discriminator:7,c:true} ] };
const mu = { m: new Map([["k1",{discriminator:1,a:99}],["k2",{discriminator:5,c:false}]]) };
const rsu = SUTypeSupport.decode(SUTypeSupport.encode(su, "le"));
const rmu = MUTypeSupport.decode(MUTypeSupport.encode(mu, "le"));
const ok =
    rsu.us[0].a === 11 && rsu.us[1].b === 2.5 && rsu.us[2].c === true &&
    rsu.us[2].discriminator === 7 &&
    rmu.m.get("k1").a === 99 && rmu.m.get("k2").c === false &&
    rmu.m.get("k2").discriminator === 5;
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify({us: rsu.us, m: [...rmu.m]}));
"#;
    assert_pass(run_driver(idl, driver), "union-in-collections");
}

/// Edge 6 — multi-byte UTF-8 (CJK + emoji) in string and UTF-16 in wstring;
/// exact code points survive.
#[test]
fn unicode_roundtrip() {
    let idl = "struct UC { string s; wstring w; };";
    let driver = r#"
import { UCTypeSupport } from "./generated.ts";
const s = { s: "héllo 世界 🚀 \u{1F600}", w: "日本語 🎉 Ωμέγα" };
const r = UCTypeSupport.decode(UCTypeSupport.encode(s, "le"));
const ok = r.s === s.s && r.w === s.w &&
           [...r.s].length === [...s.s].length && [...r.w].length === [...s.w].length;
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify(r));
"#;
    assert_pass(run_driver(idl, driver), "unicode");
}

/// Edge 7 — array-of-bounded-string with every element distinct (mis-stride
/// would be caught). (array-of-struct and multi-dim are covered by the
/// `fixed_arrays_roundtrip` test in `wire_roundtrip.rs`.)
#[test]
fn array_of_bounded_string_roundtrip() {
    let idl = "struct AB { string<8> names[3]; };";
    let driver = r#"
import { ABTypeSupport } from "./generated.ts";
const s = { names: ["aa", "bbb", "cccc"] };
const r = ABTypeSupport.decode(ABTypeSupport.encode(s, "le"));
const ok = JSON.stringify(r.names) === JSON.stringify(s.names);
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify(r.names));
"#;
    assert_pass(run_driver(idl, driver), "array-of-bounded-string");
}

/// Edge 8 — extreme integer (min/max, uint max) and float/double values.
#[test]
fn extreme_primitives_roundtrip() {
    let idl = "struct P { int8 i8; uint8 u8; int16 i16; uint16 u16; \
                          int32 i32; uint32 u32; int64 i64; uint64 u64; \
                          float f; double d; };";
    let driver = r#"
import { PTypeSupport } from "./generated.ts";
const s = { i8:-128, u8:255, i16:-32768, u16:65535,
            i32:-2147483648, u32:4294967295,
            i64:-9223372036854775808n, u64:18446744073709551615n,
            f:3.5, d:-1.25 };
const r = PTypeSupport.decode(PTypeSupport.encode(s, "le"));
const ok = r.i8===-128 && r.u8===255 && r.i16===-32768 && r.u16===65535 &&
           r.i32===-2147483648 && r.u32===4294967295 &&
           r.i64===-9223372036854775808n && r.u64===18446744073709551615n &&
           r.f===3.5 && r.d===-1.25;
console.log("RESULT", ok ? "PASS" : "FAIL",
    JSON.stringify(r, (k, v) => typeof v === "bigint" ? v.toString() : v));
"#;
    assert_pass(run_driver(idl, driver), "extreme-primitives");
}

/// Edge 9 — keyed type: two samples with the same `@key` and different
/// payloads; key fields round-trip identically.
#[test]
fn keyed_same_key_different_payload() {
    let idl = "struct K { @key long id; @key string name; long payload; };";
    let driver = r#"
import { KTypeSupport } from "./generated.ts";
const a = { id: 5, name: "node", payload: 100 };
const b = { id: 5, name: "node", payload: 200 };
const ra = KTypeSupport.decode(KTypeSupport.encode(a, "le"));
const rb = KTypeSupport.decode(KTypeSupport.encode(b, "le"));
// keys identical, payloads differ; key hashes match (same key)
const kha = Buffer.from(KTypeSupport.keyHash(a)).toString("hex");
const khb = Buffer.from(KTypeSupport.keyHash(b)).toString("hex");
const ok = ra.id === 5 && ra.name === "node" && ra.payload === 100 &&
           rb.id === 5 && rb.name === "node" && rb.payload === 200 &&
           kha === khb;
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify({ra, rb, kha, khb}));
"#;
    assert_pass(run_driver(idl, driver), "keyed");
}

/// KeyHash correctness regression (byte-exact): a `@key` member whose type
/// is a TYPEDEF alias of a struct — previously fell through to the general
/// (non-key) encoder, writing the WHOLE nested struct (including `ignored`)
/// into the KeyHash instead of just the aliased struct's own `@key` subset
/// (here: `x` alone). XTypes 1.3 §7.6.8.4: BE holder <=16 octets -> zero-pad
/// to 16.
#[test]
fn keyhash_byte_exact_typedef_of_struct_dealiases_to_own_key_subset() {
    let idl = "struct Inner { @key long x; long ignored; };\n\
               typedef Inner InnerAlias;\n\
               struct Outer { @key InnerAlias i; long tail; };";
    let driver = r#"
import { OuterTypeSupport } from "./generated.ts";
const o = { i: { x: 7, ignored: 99 }, tail: 5 };
const h = OuterTypeSupport.keyHash(o);
const expected = new Uint8Array([0,0,0,7, 0,0,0,0,0,0,0,0,0,0,0,0]);
const bytesOk = h.length === 16 && expected.every((b, idx) => h[idx] === b);
// A `tail`/`ignored`-only change must NOT move the KeyHash.
const o2 = { i: { x: 7, ignored: 42 }, tail: 999 };
const h2 = OuterTypeSupport.keyHash(o2);
const stableOk = h.length === h2.length && [...h].every((b, idx) => h2[idx] === b);
const ok = bytesOk && stableOk;
console.log("RESULT", ok ? "PASS" : "FAIL", JSON.stringify({h: [...h], h2: [...h2]}));
"#;
    assert_pass(run_driver(idl, driver), "keyhash-typedef-of-struct");
}
