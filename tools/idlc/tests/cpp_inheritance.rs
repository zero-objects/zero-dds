// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `idlc generate --lang c++` for inheritance and for the octet, compiled and run.
//!
//! DDS-XTypes 1.3 §7.5.1.4.1.1: "The C++ struct corresponding to the subtype shall publicly
//! inherit from the C++ struct corresponding to the supertype." §7.5.1.1.2: only Boolean,
//! Char8 and Byte may map to one C++ type; every other type of Table 43 is told from every
//! other by an overload, `Byte` from `UInt8` included.
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
  @final struct Base { long a; octet o; };\n\
  @final struct Derived : Base { string s; };\n\
  @final struct Further : Derived { uint8 u; };\n\
  @final struct Flat { long a; octet o; string s; uint8 u; };\n\
};\n";

const MAIN: &str = r#"#include "types.hpp"
#include <cstdio>
#include <string>
#include <type_traits>
#include <vector>

#define CHECK(c) do { if (!(c)) { std::printf("line %d: %s\n", __LINE__, #c); ++bad; } } while (0)

// Public inheritance, one level at a time.
static_assert(std::is_base_of<m::Base, m::Derived>::value, "Derived is a Base");
static_assert(std::is_base_of<m::Derived, m::Further>::value, "Further is a Derived");
static_assert(std::is_convertible<m::Derived*, m::Base*>::value, "the base is public");
static_assert(std::is_convertible<m::Further*, m::Base*>::value, "and so is the base of the base");
// Only its own members are declared: the struct is its base plus a string.
static_assert(sizeof(m::Derived) >= sizeof(m::Base) + sizeof(std::string), "");

// The octet and the uint8 are two types.
static int which(std::uint8_t) { return 1; }
static int which(zerodds::wire::Octet) { return 2; }
static_assert(!std::is_same<decltype(m::Base::o), std::uint8_t>::value, "octet is not uint8_t");
static_assert(std::is_same<decltype(m::Further::u), std::uint8_t>::value, "uint8 is uint8_t");

template <typename T>
static std::vector<std::uint8_t> encoded(const T &value) {
    return dds::topic::topic_type_support<T>::encode(value);
}

int main() {
    int bad = 0;
    m::Further further;
    further.a = 5;
    further.o = 0xAB;
    further.s = "text";
    further.u = 200;
    CHECK(which(further.o) == 2 && which(further.u) == 1);
    CHECK(which(zerodds::wire::Octet(1)) == 2 && which(std::uint8_t(1)) == 1);

    // The base members are reached through the base.
    m::Base &base = further;
    CHECK(base.a == 5 && base.o == 0xAB);

    // 7.4.1.1.5.1: "exactly as if all of the members had been defined, in the same order, in
    // the most-derived type".
    m::Flat flat;
    flat.a = 5;
    flat.o = 0xAB;
    flat.s = "text";
    flat.u = 200;
    CHECK(encoded(further) == encoded(flat));

    m::Further back = dds::topic::topic_type_support<m::Further>::decode(
        encoded(further).data(), encoded(further).size(), zerodds::wire::Encoding::PlainCdr2, false);
    CHECK(back.a == 5 && back.o == 0xAB && back.s == "text" && back.u == 200);
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
fn a_derived_struct_inherits_and_the_octet_is_its_own_type() {
    let found = compilers();
    if found.is_empty() {
        eprintln!("SKIPPED: neither clang++ nor g++ on the path.");
        return;
    }
    let dir = scratch("cpp-inheritance");
    let file = dir.join("types.idl");
    std::fs::write(&file, IDL).unwrap();
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
        "idlc generate types.idl: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let header = std::fs::read_to_string(dir.join("types.hpp")).unwrap();
    assert!(
        header.contains("Derived : public") && header.contains("m::Base"),
        "no base class:\n{header}"
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
