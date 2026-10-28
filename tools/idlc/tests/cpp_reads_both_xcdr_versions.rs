// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A C++ header generated for one XCDR version reads a sample in the
//! other.
//!
//! The writer chooses the representation (DDS-XTypes 1.3 §7.6.3.1.2) and
//! the encapsulation names it; a reader takes what arrives. The header
//! held one codec, planned for `--encoding`, and
//! `topic_type_support<T>::decode` threw for the other version — so a C++
//! reader generated for XCDR2 refused every sample a peer wrote in
//! XCDR1, and the reverse. The header now carries the other version's
//! decoders in a nested namespace (`xcdr1` or `xcdr2`) and the trait
//! reads each version through its own.
//!
//! The octets below are the two encodings of one value, written out from
//! the clauses rather than taken from this tree's encoder: XCDR1 aligns
//! an eight-octet member to eight and gives an `@appendable` struct no
//! DHEADER (§7.4.1), XCDR2 caps alignment at four and opens it with one
//! (§7.4.2, §7.4.3.4.2 for the length).
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/cpp_build.rs"]
mod cpp_build;

use std::path::Path;
use std::process::Command;

const IDL: &str = "module t {\n  @appendable struct S { long a; double d; string text; };\n};\n";

/// `S { a = 7, d = 2.5, text = "hi" }`, little-endian, in both versions.
const MAIN: &str = r#"#include "s.hpp"
#include <cstdio>
#include <cstring>
int main() {
  // XCDR1: a, four octets of padding to align d to eight, d, text. No DHEADER.
  const std::uint8_t v1[] = {
    0x07, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x40,
    0x03, 0x00, 0x00, 0x00,  'h', 'i', 0x00 };
  // XCDR2: the DHEADER (19 octets follow), a, d at four, text.
  const std::uint8_t v2[] = {
    0x13, 0x00, 0x00, 0x00,
    0x07, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x40,
    0x03, 0x00, 0x00, 0x00,  'h', 'i', 0x00 };
  using support = dds::topic::topic_type_support<t::S>;
  int bad = 0;
  const t::S one = support::decode(v1, sizeof v1, zerodds::wire::Encoding::PlainCdr1, false);
  bad += one.a != 7 || one.d != 2.5 || one.text != "hi";
  const t::S two = support::decode(v2, sizeof v2, zerodds::wire::Encoding::PlainCdr2, false);
  bad += two.a != 7 || two.d != 2.5 || two.text != "hi";
  if (bad) std::fprintf(stderr, "v1 %d %g %s / v2 %d %g %s\n", one.a, one.d, one.text.c_str(), two.a, two.d, two.text.c_str());
  return bad;
}
"#;

#[test]
fn a_header_generated_for_xcdr2_reads_xcdr1_and_xcdr2() {
    let cxx = ["c++", "g++", "clang++"].iter().find(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    });
    let Some(cxx) = cxx else {
        eprintln!("SKIPPED: no C++ compiler on the path.");
        return;
    };
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cpp-reads-both-xcdr-versions");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let idl = dir.join("s.idl");
    std::fs::write(&idl, IDL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
        .args([
            "generate",
            "--lang",
            "c++",
            "--cpp-mapping",
            "native",
            "--encoding",
            "xcdr2",
            "--out",
        ])
        .arg(&dir)
        .arg(&idl)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "idlc generate: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let header = std::fs::read_to_string(dir.join("s.hpp")).unwrap();
    assert!(
        header.contains("namespace xcdr1 {"),
        "the header carries one version's codec:\n{header}"
    );
    let unit = dir.join("main.cpp");
    std::fs::write(&unit, MAIN).unwrap();
    let program = dir.join("main");
    let output = cpp_build::compile(cxx, &dir, &unit, &program);
    assert!(
        output.status.success(),
        "the header does not compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(&program).output().unwrap();
    assert!(
        run.status.success(),
        "a version was read wrong or refused ({}): {}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
}
