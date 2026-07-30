//! Broad-audit P0-6 — array members in `@mutable` structs must reach the wire.
//!
//! Before the fix the C++ emitter's `@mutable` member path (`emit_mutable_*`,
//! `emit_pl_cdr1_*`) only serialized `Declarator::Simple` members; an array
//! member (`long values[2];`) was silently skipped — encode wrote nothing and
//! decode never read it, so the data was lost with no error (XTypes 1.3
//! §7.4.3.4.2). This test proves — through the REAL emit path AND a real C++
//! compile+run — that an array member now round-trips under BOTH XCDR2 (PL_CDR2
//! EMHEADER, LC=4 framing) and XCDR1 (PL_CDR1 PID parameter), for 1-D / multi-
//! dim / complex-element arrays, and that the array member's on-wire framing
//! matches the spec (LC=4 EMHEADER + NEXTINT = total array-body byte length).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

/// First available C++17 compiler (`clang++` or `g++`), or `None`.
fn cpp_compiler() -> Option<&'static str> {
    ["clang++", "g++"].into_iter().find(|cc| {
        Command::new(cc)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Path to the C++ runtime include dir (`crates/cpp/include`).
fn cpp_include_dir() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../cpp/include").to_string()
}

fn gen_cpp(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("cpp gen")
}

/// Compile `cpp` (a generated header) + `main_src` into a binary, run it, and
/// require exit 0 — a real encode/decode roundtrip, not just a syntax check.
fn compile_and_run(cc: &str, cpp: &str, main_src: &str) {
    let dir = tempfile::tempdir().expect("tempdir");
    let hdr = dir.path().join("gen.hpp");
    std::fs::write(&hdr, cpp).expect("write hdr");
    let main = dir.path().join("main.cpp");
    std::fs::write(&main, main_src).expect("write main");
    let bin = dir.path().join("a.out");
    let compile = Command::new(cc)
        .args(["-std=c++17", "-I"])
        .arg(dir.path())
        .arg("-I")
        .arg(cpp_include_dir())
        .arg(&main)
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("spawn compiler");
    assert!(
        compile.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = Command::new(&bin).output().expect("spawn bin");
    assert!(
        run.status.success(),
        "roundtrip binary failed (exit {:?}):\nstdout:\n{}\nstderr:\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
}

/// Source-level guard (always runs, no compiler needed): the `@mutable` array
/// member must NOT be emitted as a "not supported (skip)" comment, and must be
/// framed by the EMHEADER (XCDR2) and PL_CDR1 (XCDR1) member writers.
#[test]
fn mutable_array_member_is_wired_not_skipped() {
    let cpp = gen_cpp("@mutable struct M { long a; long values[2]; long b; };");
    assert!(
        !cpp.contains("array member 'values' not supported"),
        "the @mutable array member is still silently skipped"
    );
    // XCDR2 LC=4 frame + XCDR1 PID frame for the member.
    assert!(
        cpp.contains("emheader_nextint_begin"),
        "@mutable array member missing the XCDR2 LC=4 EMHEADER frame"
    );
    assert!(
        cpp.contains("pl_cdr1_member_begin"),
        "@mutable array member missing the XCDR1 PL_CDR1 frame"
    );
}

/// 1-D primitive array in an `@mutable` struct: full encode→decode roundtrip
/// under XCDR2 AND XCDR1, plus a concrete byte assertion on the array member's
/// LC=4 EMHEADER and its NEXTINT length word (= 8 bytes for `long[2]`).
#[test]
fn mutable_1d_primitive_array_roundtrips_and_byte_exact() {
    let Some(cc) = cpp_compiler() else {
        println!("no C++ compiler available, skipping compile+run");
        return;
    };
    let cpp = gen_cpp("@mutable struct M { long a; long values[2]; long b; };");
    let main = r#"
#include "gen.hpp"
#include <cassert>
#include <cstdio>
using namespace dds::topic;

static void check_roundtrip(xcdr2::XcdrVersion repr) {
    M m; m.a(111);
    m.values()[0] = 222; m.values()[1] = 333;
    m.b(444);
    auto bytes = topic_type_support<M>::encode(m, repr);
    auto back  = topic_type_support<M>::decode(bytes.data(), bytes.size(), repr);
    assert(back.a() == 111);
    assert(back.values()[0] == 222);   // array element 0 survived
    assert(back.values()[1] == 333);   // array element 1 survived
    assert(back.b() == 444);           // member AFTER the array not corrupted
}

int main() {
    check_roundtrip(xcdr2::XcdrVersion::Xcdr2);
    check_roundtrip(xcdr2::XcdrVersion::Xcdr1);

    // Concrete XCDR2-LE byte layout of `@mutable struct M { long a; long
    // values[2]; long b; }` (XTypes 1.3 §7.4.3.4.2, PL_CDR2):
    //   [0..4)   DHEADER  = 32 (body length)
    //   [4..8)   EMHEADER a      : LC=2, id=0  -> 0x20000000
    //   [8..12)  a value         = 111
    //   [12..16) EMHEADER values : LC=4, id=1  -> 0x40000001
    //   [16..20) NEXTINT         = 8  (2 x int32 = 8 bytes)
    //   [20..24) values[0]       = 222
    //   [24..28) values[1]       = 333
    //   [28..32) EMHEADER b      : LC=2, id=2  -> 0x20000002
    //   [32..36) b value         = 444
    M m; m.a(111); m.values()[0] = 222; m.values()[1] = 333; m.b(444);
    auto b = topic_type_support<M>::encode(m, xcdr2::XcdrVersion::Xcdr2);
    assert(b.size() == 36);
    // array member EMHEADER: LC=4, member_id=1 = 0x40000001 (little-endian
    // byte form 01 00 00 40).
    assert(b[12] == 0x01 && b[13] == 0x00 && b[14] == 0x00 && b[15] == 0x40);
    // NEXTINT length word = 8 (little-endian).
    assert(b[16] == 0x08 && b[17] == 0x00 && b[18] == 0x00 && b[19] == 0x00);
    // the two array elements follow immediately.
    assert(b[20] == 222 && b[24] == 77 /*333 & 0xFF*/);
    return 0;
}
"#;
    compile_and_run(cc, &cpp, main);
}

/// Multi-dimensional primitive array + 1-D string array + array of a nested
/// `@final` struct, all in an `@mutable` struct: each must round-trip fully
/// under XCDR2 AND XCDR1 (parity with the @final/@appendable array path).
#[test]
fn mutable_multidim_and_complex_arrays_roundtrip() {
    let Some(cc) = cpp_compiler() else {
        println!("no C++ compiler available, skipping compile+run");
        return;
    };
    let src = "\
module p0_6 {
  struct Point { long x; long y; };
  @mutable struct Bag {
    long          lead;
    long          grid[2][2];   // multi-dim primitive PARRAY
    string        names[2];     // 1-D string array
    Point         pts[2];       // array of a nested @final struct
    long          tail;
  };
};";
    let cpp = gen_cpp(src);
    let main = r#"
#include "gen.hpp"
#include <cassert>
using namespace dds::topic;

static void check(xcdr2::XcdrVersion repr) {
    p0_6::Bag g;
    g.lead(7);
    g.grid()[0][0] = 1; g.grid()[0][1] = 2;
    g.grid()[1][0] = 3; g.grid()[1][1] = 4;
    g.names()[0] = "alpha"; g.names()[1] = "beta";
    p0_6::Point p0; p0.x(10); p0.y(11);
    p0_6::Point p1; p1.x(20); p1.y(21);
    g.pts()[0] = p0; g.pts()[1] = p1;
    g.tail(99);

    auto bytes = topic_type_support<p0_6::Bag>::encode(g, repr);
    auto back  = topic_type_support<p0_6::Bag>::decode(bytes.data(), bytes.size(), repr);

    assert(back.lead() == 7);
    assert(back.grid()[0][0] == 1 && back.grid()[0][1] == 2);
    assert(back.grid()[1][0] == 3 && back.grid()[1][1] == 4);
    assert(back.names()[0] == "alpha" && back.names()[1] == "beta");
    assert(back.pts()[0].x() == 10 && back.pts()[0].y() == 11);
    assert(back.pts()[1].x() == 20 && back.pts()[1].y() == 21);
    assert(back.tail() == 99);  // member after all the arrays intact
}

int main() {
    check(xcdr2::XcdrVersion::Xcdr2);
    check(xcdr2::XcdrVersion::Xcdr1);
    return 0;
}
"#;
    compile_and_run(cc, &cpp, main);
}
