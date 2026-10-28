// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `idlc generate --lang c++` for runs of octets, compiled and run.
//!
//! A `sequence<octet>` and a one-dimensional `octet[N]` are written with
//! one `Writer::write_octets` and read with one `Reader::read_octets`
//! (`crates/lang-rules/tests/cpp_octet_runs.rs` holds the text). Here the
//! header goes through the compiler the way a consumer builds it, and a
//! sample goes through `topic_type_support<T>::encode` and `decode`.
//!
//! clang is the compiler that matters for the first half: it reports a
//! `static` function nothing calls under `-Wunused-function`, in an
//! included header too, and g++ 14 does not (measured on one header with
//! one uncalled `static void`: clang++ 19 exits 1, g++ 14.2 exits 0). A
//! header for `OnlyRuns` that still carried `zd_enc_p_octet` fails here
//! under clang.
//!
//! The bytes below are this file's own reading of XCDR2 for a `@final`
//! struct, a regression pin and not an oracle. What the bytes are measured
//! against is the matrix: the C++ column's cells for the corpus cases
//! with octets, compared with the recorded ones and with the Rust column.
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

/// Octets in runs only — a sequence, an array, a typedef'd element and
/// `uint8` — and, in a second file, octets both in runs and not.
const RUNS: &str = "module m {\n\
  typedef octet Byte;\n\
  @final struct OnlyRuns {\n\
    sequence<octet, 64> a;\n\
    octet b[4];\n\
    sequence<Byte> c;\n\
    uint8 d[3];\n\
  };\n\
};\n";
const MIXED: &str = "module n {\n\
  @final struct Mixed {\n\
    octet grid[2][2];\n\
    octet single;\n\
    sequence<octet> tail;\n\
  };\n\
};\n";

const MAIN: &str = r#"#include "runs.hpp"
#include "mixed.hpp"
#include <cstdio>
#include <vector>

template <typename T>
static T again(const T &value, std::vector<std::uint8_t> &bytes) {
    bytes = dds::topic::topic_type_support<T>::encode(value);
    return dds::topic::topic_type_support<T>::decode(
        bytes.data(), bytes.size(), zerodds::wire::Encoding::PlainCdr2, false);
}

int main() {
    int bad = 0;
    m::OnlyRuns runs;
    runs.a = {0x00, 0x7F, 0x80, 0xFF};
    runs.b = {1, 2, 3, 4};
    runs.c = {9, 8};
    runs.d = {5, 6, 7};
    std::vector<std::uint8_t> bytes;
    m::OnlyRuns back = again(runs, bytes);
    const std::vector<std::uint8_t> want = {4, 0, 0, 0, 0x00, 0x7F, 0x80, 0xFF, 1, 2, 3, 4,
                                            2, 0, 0, 0, 9, 8, 5, 6, 7};
    if (bytes != want) { std::puts("OnlyRuns: bytes"); bad++; }
    if (back.a != runs.a || back.b != runs.b || back.c != runs.c || back.d != runs.d) {
        std::puts("OnlyRuns: value"); bad++;
    }

    n::Mixed mixed;
    mixed.grid = {{{1, 2}, {3, 4}}};
    mixed.single = 0xAB;
    mixed.tail = {0xCD, 0xEF};
    n::Mixed other = again(mixed, bytes);
    const std::vector<std::uint8_t> mixed_want = {1, 2, 3, 4, 0xAB, 0, 0, 0,
                                                  2, 0, 0, 0, 0xCD, 0xEF};
    if (bytes != mixed_want) { std::puts("Mixed: bytes"); bad++; }
    if (other.grid != mixed.grid || other.single != mixed.single || other.tail != mixed.tail) {
        std::puts("Mixed: value"); bad++;
    }

    // An empty run writes its count and nothing else, and reads back empty.
    m::OnlyRuns empty;
    m::OnlyRuns none = again(empty, bytes);
    if (bytes.size() != 4 + 4 + 4 + 3 || !none.a.empty() || !none.c.empty()) {
        std::puts("OnlyRuns: empty"); bad++;
    }
    return bad;
}
"#;

/// The compilers on the path, clang first.
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
fn runs_of_octets_compile_under_werror_and_come_back_as_written() {
    let found = compilers();
    if found.is_empty() {
        eprintln!("SKIPPED: neither clang++ nor g++ on the path.");
        return;
    }
    if !found.contains(&"clang++") {
        eprintln!("NOTE: no clang++ on the path, so -Wunused-function was not checked.");
    }
    let dir = scratch("cpp-octet-runs");
    for (name, idl) in [("runs", RUNS), ("mixed", MIXED)] {
        let file = dir.join(format!("{name}.idl"));
        std::fs::write(&file, idl).unwrap();
        let output = idlc()
            .args([
                "generate",
                "--lang",
                "c++",
                "--cpp-mapping",
                "native",
                "--out",
            ])
            .arg(&dir)
            .arg(&file)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "idlc generate {name}.idl: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let header = std::fs::read_to_string(dir.join("runs.hpp")).unwrap();
    assert!(!header.contains("zd_enc_p_octet"), "{header}");
    let unit = dir.join("main.cpp");
    std::fs::write(&unit, MAIN).unwrap();
    for cxx in found {
        let program = dir.join(format!("main-{cxx}"));
        let output = cpp_build::compile(cxx, &dir, &unit, &program);
        assert!(
            output.status.success(),
            "{cxx}: the headers do not compile:\n{}",
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
