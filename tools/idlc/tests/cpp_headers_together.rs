// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `idlc generate --lang c++` headers of different IDL files in one
//! translation unit, and the default values of a generated struct.
//!
//! Every header writes its codec as free functions named after what they
//! encode (`zd_enc_p_unsigned_long`, `zd_enc_string`, …) and a
//! `zd_endianness` constant. At file scope two headers whose types share
//! a primitive defined them twice, and a translation unit that included
//! both stopped at the second — 18 redefinitions for four of a user's
//! contract files. The codec now lives in
//! `namespace zd_<file stem>`, and a header included twice is `#pragma
//! once`.
//!
//! IDL4 to C++ 1.0 §7.2.4.3.1: a struct's default constructor
//! "initializes all built-in data types to their default value [...],
//! enumerators to their first value, and the rest of members using their
//! default constructor". The generated structs had no initializers, so
//! `S s;` left every built-in member indeterminate.
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

/// Two files whose types share `unsigned long`, `unsigned long long` and
/// `string` — the helpers that collided — and one with an enumeration
/// whose first enumerator is not 0, an array and a typedef.
const CTL: &str = "module ctl {\n\
  @appendable struct Msg {\n\
    @key unsigned long long sender;\n\
    unsigned long kind;\n\
    string name;\n\
  };\n\
};\n";
const STATE: &str = "module state {\n\
  enum Phase { @value(5) FIRST, @value(9) SECOND };\n\
  typedef long Count;\n\
  @appendable struct Sample {\n\
    @key unsigned long long entity;\n\
    unsigned long seq;\n\
    string label;\n\
    Phase stage;\n\
    Count total;\n\
    boolean valid;\n\
    double value;\n\
    long history[3];\n\
    sequence<long> path;\n\
  };\n\
};\n";

const MAIN: &str = "#include \"ctl.hpp\"\n\
#include \"state.hpp\"\n\
#include \"ctl.hpp\"\n\
int main() {\n\
  ctl::Msg m;\n\
  state::Sample s;\n\
  int bad = 0;\n\
  bad += m.sender != 0 || m.kind != 0 || !m.name.empty();\n\
  bad += s.entity != 0 || s.seq != 0 || s.total != 0 || s.valid || s.value != 0.0;\n\
  bad += s.stage != state::Phase::FIRST;\n\
  bad += s.history[0] != 0 || s.history[2] != 0 || !s.path.empty();\n\
  bad += zd_ctl::zd_endianness != zd_state::zd_endianness;\n\
  bad += dds::topic::topic_type_support<ctl::Msg>::encode(m).empty();\n\
  bad += dds::topic::topic_type_support<state::Sample>::encode(s).empty();\n\
  return bad;\n\
}\n";

#[test]
fn two_generated_headers_share_a_translation_unit_and_their_structs_start_at_their_defaults() {
    let cxx = ["c++", "g++", "clang++"].iter().find(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    });
    let Some(cxx) = cxx else {
        // The absence of the compiler is not a pass: `ZERODDS_REQUIRE_CXX=1` (the test host and the
        // CI job set it) makes it a failure, as `ZERODDS_REQUIRE_JDK=1` does for the Java tests.
        assert!(
            std::env::var("ZERODDS_REQUIRE_CXX").as_deref() != Ok("1"),
            "ZERODDS_REQUIRE_CXX=1 and no `c++`, `g++` or `clang++` on PATH"
        );
        eprintln!("SKIPPED: no C++ compiler on the path.");
        return;
    };
    let dir = scratch("cpp-headers-together");
    for (name, idl) in [("ctl", CTL), ("state", STATE)] {
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
    let unit = dir.join("main.cpp");
    std::fs::write(&unit, MAIN).unwrap();
    let program = dir.join("main");
    let output = cpp_build::compile(cxx, &dir, &unit, &program);
    assert!(
        output.status.success(),
        "the two headers do not compile in one translation unit:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status = Command::new(&program).status().unwrap();
    assert!(
        status.success(),
        "a default-constructed struct is not at the values IDL4 to C++ \
         7.2.4.3.1 names ({status}: the exit code counts the failed checks)"
    );
}
