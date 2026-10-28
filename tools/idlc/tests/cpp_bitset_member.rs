// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `idlc generate --lang c++` for a structure with a bitset member, under the default mapping,
//! compiled and run.
//!
//! Under `--cpp-mapping psm` a bitset is the native type, and the equality of a PSM class
//! compares every member with `==`. The native bitset had no `operator==`, so the header of a
//! structure with a bitset member did not compile.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/cpp_build.rs"]
mod cpp_build;

use std::path::{Path, PathBuf};
use std::process::Command;

fn idlc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const IDL: &str = "module m {\n\
  bitset Flags { bitfield<3> kind; bitfield<5> prio; };\n\
  bitset Padding { bitfield<8>; };\n\
  @final struct Bits { long n; m::Flags flags; m::Padding pad; };\n\
};\n";

const MAIN: &str = r#"#include "types.hpp"
#include <cstdio>
#include <vector>

#define CHECK(c) do { if (!(c)) { std::printf("line %d: %s\n", __LINE__, #c); ++bad; } } while (0)

int main() {
    int bad = 0;
    // The bitset compares field by field.
    m::Flags f;
    f.kind = 5;
    f.prio = 20;
    m::Flags g = f;
    CHECK(f == g && !(f != g));
    g.prio = 21;
    CHECK(f != g && !(f == g));
    // A bitset that is all padding is equal to every other one.
    CHECK(m::Padding() == m::Padding());

    // The structure compares its bitset member.
    m::Bits a;
    a.n(1);
    a.flags(f);
    m::Bits b = a;
    CHECK(a == b);
    b.flags(g);
    CHECK(a != b);

    // And goes through the codec with it.
    std::vector<std::uint8_t> bytes = dds::topic::topic_type_support<m::Bits>::encode(a);
    m::Bits back = dds::topic::topic_type_support<m::Bits>::decode(
        bytes.data(), bytes.size(), zerodds::wire::Encoding::PlainCdr2, false);
    CHECK(back == a && back.flags().kind == 5 && back.flags().prio == 20);
    return bad;
}
"#;

fn compilers() -> Vec<&'static str> {
    ["clang++", "g++"]
        .into_iter()
        .filter(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .collect()
}

#[test]
fn a_structure_with_a_bitset_member_compiles_under_the_default_mapping() {
    let found = compilers();
    if found.is_empty() {
        eprintln!("SKIPPED: neither clang++ nor g++ on the path.");
        return;
    }
    let dir = scratch("cpp-bitset-member");
    let file = dir.join("types.idl");
    std::fs::write(&file, IDL).unwrap();
    let output = idlc()
        .args(["generate", "--lang", "c++", "--out"])
        .arg(&dir)
        .arg(&file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "idlc generate types.idl: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let unit = dir.join("main.cpp");
    std::fs::write(&unit, MAIN).unwrap();
    for cxx in found {
        let program = dir.join(format!("main-{cxx}"));
        let output = cpp_build::compile(cxx, &dir, &unit, &program);
        assert!(
            output.status.success(),
            "{cxx}: the program does not compile:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let run = Command::new(&program).output().unwrap();
        assert!(
            run.status.success(),
            "{cxx}: {} check(s) failed:\n{}",
            run.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&run.stdout)
        );
    }
}
