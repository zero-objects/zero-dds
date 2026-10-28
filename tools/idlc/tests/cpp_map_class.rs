// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `idlc generate --lang c++` for maps, compiled and run: the Traditional
//! C++ binding of DDS-XTypes 1.3 §7.5.1.3.1 and §7.5.1.3.3.
//!
//! The classes `UInt32FooMap`, `UInt64FooMap` and `StringFooMap` stand
//! beside `Foo` in its namespace, a member of an IDL `map` has the type of
//! its class, the operations of Table 47 answer as the clause says, and a
//! sample with maps goes through `topic_type_support<T>::encode` and
//! `decode` with the entries in the order they were inserted.
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
  struct Foo { long a; };\n\
  enum Color { RED, GREEN };\n\
  @final struct Holder {\n\
    map<unsigned long, Foo> by_id;\n\
    map<string, long> counts;\n\
    map<long, string, 3> small;\n\
    map<unsigned long long, Color> colors;\n\
  };\n\
};\n";

const MAIN: &str = r#"#include "maps.hpp"
#include <cstdio>
#include <string>
#include <type_traits>
#include <vector>

#define CHECK(c) do { if (!(c)) { std::printf("line %d: %s\n", __LINE__, #c); ++bad; } } while (0)

// The class of a member is the class of the clause, in the namespace of the element.
static_assert(std::is_same<decltype(m::Holder::by_id), m::UInt32FooMap>::value,
              "map<unsigned long, Foo> is UInt32FooMap");
static_assert(std::is_same<decltype(m::Holder::colors), m::UInt64ColorMap>::value,
              "map<unsigned long long, Color> is UInt64ColorMap");
static_assert(std::is_same<decltype(m::Holder::counts), zerodds::Map<std::string, std::int32_t> >::value,
              "map<string, long> is a Map of string and int32_t");
static_assert(std::is_base_of<std::vector<std::pair<std::uint32_t, m::Foo> >, m::UInt32FooMap>::value,
              "a map is the vector of pairs the codec reads");
// The three classes of every type, used or not.
static_assert(sizeof(m::StringFooMap) > 0 && sizeof(m::UInt64FooMap) > 0, "");
static_assert(sizeof(m::StringColorMap) > 0 && sizeof(m::UInt32ColorMap) > 0, "");
static_assert(sizeof(m::UInt32HolderMap) > 0 && sizeof(m::StringHolderMap) > 0, "");

template <typename T>
static T again(const T &value, std::vector<std::uint8_t> &bytes) {
    bytes = dds::topic::topic_type_support<T>::encode(value);
    return dds::topic::topic_type_support<T>::decode(
        bytes.data(), bytes.size(), zerodds::wire::Encoding::PlainCdr2, false);
}

int main() {
    int bad = 0;
    m::Foo foo1; foo1.a = 1;
    m::Foo foo2; foo2.a = 2;

    // initialize: the initial size and capacity of the map are 0.
    m::UInt32FooMap map;
    CHECK(map.get_size() == 0u && map.get_max_size() == 0u);
    // insert past the capacity.
    CHECK(map.insert(7u, foo1) == DDS::RETCODE_PRECONDITION_NOT_MET);

    // set_max_size, insert.
    CHECK(map.set_max_size(2u) == DDS::RETCODE_OK);
    CHECK(map.get_max_size() == 2u);
    CHECK(map.insert(7u, foo1, false) == DDS::RETCODE_OK);
    CHECK(map.insert(7u, foo2, false) == DDS::RETCODE_BAD_PARAMETER);   // insert: the key exists
    CHECK(map.insert(8u, foo2, false) == DDS::RETCODE_OK);
    CHECK(map.insert(9u, foo2) == DDS::RETCODE_PRECONDITION_NOT_MET);   // full
    CHECK(map.get_size() == 2u);
    CHECK(map.set_max_size(1u) == DDS::RETCODE_BAD_PARAMETER);          // below the size

    // insert with replace is insert_or_assign.
    CHECK(map.insert(7u, foo2, true) == DDS::RETCODE_OK);
    CHECK(map.find_element(7u) != 0 && map.find_element(7u)->a == 2);
    CHECK(map.get_size() == 2u);
    CHECK(map.find_element(99u) == 0);

    // get_first, get_next, get_pair, find_entry, in insertion order.
    m::UInt32FooMap::MapEntry entry = map.get_first();
    std::uint32_t key = 0;
    m::Foo *element = 0;
    CHECK(entry.valid());
    CHECK(map.get_pair(entry, &key, &element) && key == 7u && element->a == 2);
    CHECK(map.get_next(entry));
    CHECK(map.get_pair(entry, &key, &element) && key == 8u);
    CHECK(!map.get_next(entry));
    CHECK(!entry.valid());
    CHECK(!map.get_pair(entry, &key, &element));
    CHECK(map.find_entry(8u).valid() && !map.find_entry(1u).valid());
    CHECK(map.get_first().valid());

    // erase.
    CHECK(map.erase(7u) == DDS::RETCODE_OK);
    CHECK(map.get_size() == 1u);
    CHECK(map.erase(7u) == DDS::RETCODE_BAD_PARAMETER);

    // copy: too small without autogrow, then with it; the source is not changed.
    m::UInt32FooMap other;
    CHECK(other.copy(map, false) == DDS::RETCODE_PRECONDITION_NOT_MET);
    CHECK(other.get_size() == 0u);
    CHECK(other.copy(map, true) == DDS::RETCODE_OK);
    CHECK(other.get_size() == 1u && other.find_element(8u) != 0 && other.find_element(8u) != map.find_element(8u));
    CHECK(map.get_size() == 1u);

    // clear: the size is 0, the maximum is not changed.
    map.clear();
    CHECK(map.get_size() == 0u && map.get_max_size() == 2u);

    // A string key is compared by its characters; a bound is the largest capacity.
    zerodds::Map<std::string, std::int32_t> counts;
    CHECK(counts.set_max_size(2u) == DDS::RETCODE_OK);
    CHECK(counts.insert("one", 1, false) == DDS::RETCODE_OK);
    CHECK(counts.find_element(std::string("on") + "e") != 0);
    CHECK(counts.insert(std::string("on") + "e", 2, false) == DDS::RETCODE_BAD_PARAMETER);
    zerodds::Map<std::int32_t, std::string, 3> small;
    CHECK(small.set_max_size(4u) == DDS::RETCODE_BAD_PARAMETER);
    CHECK(small.set_max_size(3u) == DDS::RETCODE_OK);

    // A sample with maps: the entries come back in the order they were inserted.
    m::Holder holder;
    CHECK(holder.by_id.set_max_size(4u) == DDS::RETCODE_OK);
    CHECK(holder.by_id.insert(9u, foo1) == DDS::RETCODE_OK);
    CHECK(holder.by_id.insert(3u, foo2) == DDS::RETCODE_OK);
    CHECK(holder.counts.set_max_size(1u) == DDS::RETCODE_OK);
    CHECK(holder.counts.insert("x", 5) == DDS::RETCODE_OK);
    CHECK(holder.small.set_max_size(3u) == DDS::RETCODE_OK);
    CHECK(holder.small.insert(-1, "a") == DDS::RETCODE_OK);
    CHECK(holder.colors.set_max_size(1u) == DDS::RETCODE_OK);
    CHECK(holder.colors.insert(5u, m::Color::GREEN) == DDS::RETCODE_OK);
    std::vector<std::uint8_t> bytes;
    m::Holder back = again(holder, bytes);
    CHECK(back.by_id.size() == 2u && back.by_id[0].first == 9u && back.by_id[1].first == 3u);
    CHECK(back.by_id.find_element(3u) != 0 && back.by_id.find_element(3u)->a == 2);
    CHECK(back.counts.find_element("x") != 0 && *back.counts.find_element("x") == 5);
    CHECK(back.small.get_size() == 1u && back.small.find_element(-1) != 0);
    CHECK(back.colors.find_element(5u) != 0 && *back.colors.find_element(5u) == m::Color::GREEN);
    // What was read fills the map: no room is left, and set_max_size makes some.
    CHECK(back.by_id.get_max_size() == 2u);
    CHECK(back.by_id.insert(1u, foo1) == DDS::RETCODE_PRECONDITION_NOT_MET);
    CHECK(back.by_id.set_max_size(3u) == DDS::RETCODE_OK);
    CHECK(back.by_id.insert(1u, foo1) == DDS::RETCODE_OK);
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
fn a_map_is_a_class_with_the_operations_of_table_47() {
    let found = compilers();
    if found.is_empty() {
        eprintln!("SKIPPED: neither clang++ nor g++ on the path.");
        return;
    }
    let dir = scratch("cpp-map-class");
    let file = dir.join("maps.idl");
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
        "idlc generate maps.idl: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let header = std::fs::read_to_string(dir.join("maps.hpp")).unwrap();
    for alias in [
        "using UInt32Foo",
        "using StringColorMap",
        "using UInt64HolderMap",
    ] {
        assert!(header.contains(alias), "{alias}:\n{header}");
    }
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
