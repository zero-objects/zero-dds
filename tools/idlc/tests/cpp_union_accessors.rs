// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `idlc generate --lang c++` for unions, compiled and run: the accessor functions of IDL4 to C++ 1.0.
//!
//! DDS-XTypes 1.3 §7.5.1.4: "The Plain Language Binding for structure and union types shall
//! correspond to the IDL language mappings for IDL structures and unions": a union has `_d()`
//! and, per case, a getter, a reference getter and setters, a setter selecting its case.
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
  @final struct Point { long x; long y; };\n\
  @final union Choice switch (long) {\n\
    case 1: long number;\n\
    case 2: case 3: string text;\n\
    case 4: Point where;\n\
    default: octet other;\n\
  };\n\
  @final struct Holder { Choice pick; long key; };\n\
};\n";

const MAIN: &str = r#"#include "types.hpp"
#include <cstdio>
#include <string>
#include <type_traits>
#include <vector>

#define CHECK(c) do { if (!(c)) { std::printf("line %d: %s\n", __LINE__, #c); ++bad; } } while (0)

// The accessors have the types IDL4 to C++ 1.0 gives them.
static_assert(std::is_same<decltype(std::declval<const m::Choice&>()._d()), std::int32_t>::value, "_d() const");
static_assert(std::is_same<decltype(std::declval<m::Choice&>().number()), std::int32_t&>::value, "number()");
static_assert(std::is_same<decltype(std::declval<const m::Choice&>().text()), const std::string&>::value, "text() const");

template <typename T>
static std::vector<std::uint8_t> encoded(const T &value) {
    return dds::topic::topic_type_support<T>::encode(value);
}

template <typename T>
static T decoded(const std::vector<std::uint8_t> &bytes) {
    return dds::topic::topic_type_support<T>::decode(
        bytes.data(), bytes.size(), zerodds::wire::Encoding::PlainCdr2, false);
}

int main() {
    int bad = 0;
    m::Choice choice;
    // A setter selects its case: the first label of it.
    choice.number(7);
    CHECK(choice._d() == 1 && choice.number() == 7);
    choice.text("hello");
    CHECK(choice._d() == 2 && choice.text() == "hello");
    std::string moved = "moved";
    choice.text(std::move(moved));
    CHECK(choice._d() == 2 && choice.text() == "moved");
    m::Point point;
    point.x = 3;
    point.y = 4;
    choice.where(point);
    CHECK(choice._d() == 4 && choice.where().x == 3 && choice.where().y == 4);
    // The default case has no label of its own: its setter leaves the discriminator.
    choice._d(9);
    choice.other(m::Choice().other());
    CHECK(choice._d() == 9);
    // The reference getter changes the value and not the discriminator.
    choice.number() = 11;
    CHECK(choice._d() == 9 && choice.number() == 11);
    // The discriminator has a modifier.
    choice._d(3);
    CHECK(choice._d() == 3);

    // The value goes through the codec as the discriminator and the case it selects.
    m::Choice sent;
    sent.text("on the wire");
    std::vector<std::uint8_t> bytes = encoded(sent);
    m::Choice back = decoded<m::Choice>(bytes);
    CHECK(back._d() == 2 && back.text() == "on the wire");
    sent.where(point);
    back = decoded<m::Choice>(encoded(sent));
    CHECK(back._d() == 4 && back.where().x == 3 && back.where().y == 4);
    sent.number(-5);
    back = decoded<m::Choice>(encoded(sent));
    CHECK(back._d() == 1 && back.number() == -5);

    // A union as a member.
    m::Holder holder;
    holder.key = 8;
    holder.pick.number(42);
    m::Holder holder_back = decoded<m::Holder>(encoded(holder));
    CHECK(holder_back.key == 8 && holder_back.pick._d() == 1 && holder_back.pick.number() == 42);
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
fn a_union_has_the_accessor_functions_and_goes_through_the_codec() {
    let found = compilers();
    if found.is_empty() {
        eprintln!("SKIPPED: neither clang++ nor g++ on the path.");
        return;
    }
    let dir = scratch("cpp-union-accessors");
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
        header.contains("_d() const") && header.contains("zd_d;"),
        "no accessor functions:\n{header}"
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
