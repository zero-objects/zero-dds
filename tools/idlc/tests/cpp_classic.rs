// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `idlc generate --lang c++ --cpp-mapping classic`, compiled and run: the classic OMG IDL to C++
//! 1.3 mapping of the data types, over the native codec.
//!
//! The declarations between the markers `ZD-CLASSIC-BEGIN` and `ZD-CLASSIC-END` are C++03 and are
//! compiled alone under `-std=c++03`. The whole header, with the native types, the codec and the
//! conversions, is compiled under C++17 and a sample goes through `topic_type_support<T>::encode`
//! and `decode`: the bytes are the bytes of the native sample.
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
  enum Color { RED, GREEN, @value(7) BLUE };\n\
  bitmask Flags { @position(0) A, @position(3) B };\n\
  typedef sequence<long> Longs;\n\
  typedef double Grid[2][3];\n\
  @final struct Base { long id; Color shade; };\n\
  @final struct Point { long x; long y; };\n\
  @final struct Holder : Base {\n\
    string name;\n\
    wstring label;\n\
    Longs values;\n\
    sequence<Point> points;\n\
    Grid cells;\n\
    octet data[4];\n\
    @optional long extra;\n\
    map<string, long> table;\n\
    Flags mask;\n\
  };\n\
  @final union Choice switch (long) {\n\
    case 1: long number;\n\
    case 2: case 3: string text;\n\
    case 4: Point where;\n\
    default: octet other;\n\
  };\n\
  @final struct Frame { Choice pick; Color tint; };\n\
};\n";

const MAIN: &str = r#"#include "types.hpp"
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#define CHECK(c) do { if (!(c)) { std::printf("line %d: %s\n", __LINE__, #c); ++bad; } } while (0)

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
    // The types are the classic ones.
    CHECK(sizeof(CORBA::Long) == 4 && sizeof(CORBA::LongLong) == 8);
    m::Holder holder;
    CHECK(holder.shade == m::RED);            // an enumeration starts at its first enumerator
    holder.id = 7;
    holder.shade = m::BLUE;
    holder.name = "text";                      // a String_var copies
    holder.label = L"wide";
    holder.values.length(3);
    holder.values[0] = 1; holder.values[1] = 2; holder.values[2] = 3;
    holder.points.length(1);
    holder.points[0].x = 5; holder.points[0].y = 6;
    for (int i = 0; i < 2; ++i) for (int j = 0; j < 3; ++j) holder.cells[i][j] = i * 3 + j;
    for (int i = 0; i < 4; ++i) holder.data[i] = static_cast<CORBA::Octet>(250 + i);
    holder.extra = new CORBA::Long(42);      // an @optional member is a plain pointer (Table D.3)
    CHECK(holder.table.insert("one", 1));
    holder.mask = m::Flags::A | m::Flags::B;

    // The classic sample and the native one are the same octets.
    zd_native::m::Holder native;
    native.id = 7;
    native.shade = zd_native::m::Color::BLUE;
    native.name = "text";
    native.label = "wide";
    native.values = std::vector<std::int32_t>{1, 2, 3};
    native.points.resize(1);
    native.points[0].x = 5; native.points[0].y = 6;
    for (int i = 0; i < 2; ++i) for (int j = 0; j < 3; ++j) native.cells[i][j] = i * 3 + j;
    for (int i = 0; i < 4; ++i) native.data[i] = static_cast<std::uint8_t>(250 + i);
    native.extra = new std::int32_t(42);
    native.table.set_max_size(2);
    native.table.insert("one", 1);
    native.mask = (1u << 0) | (1u << 3);
    std::vector<std::uint8_t> bytes = encoded(holder);
    CHECK(bytes == dds::topic::topic_type_support<zd_native::m::Holder>::encode(native));

    m::Holder back = decoded<m::Holder>(bytes);
    CHECK(back.id == 7);
    CHECK(back.shade == m::BLUE);
    CHECK(std::string(back.name.in()) == "text");
    CHECK(std::wstring(back.label.in()) == L"wide");
    CHECK(back.values.length() == 3 && back.values[2] == 3);
    CHECK(back.points.length() == 1 && back.points[0].y == 6);
    CHECK(back.cells[1][2] == 5.0 && back.data[3] == 253);
    CHECK(back.extra != 0 && *back.extra == 42);
    CHECK(back.table.length() == 1 && back.table["one"] == 1);
    CHECK(back.mask() == (m::Flags::A | m::Flags::B));

    // A union keeps its case and its discriminator.
    m::Frame frame;
    frame.tint = m::GREEN;
    frame.pick.text("hello");
    CHECK(frame.pick._d() == 2);
    m::Frame frame_back = decoded<m::Frame>(encoded(frame));
    CHECK(frame_back.pick._d() == 2 && std::string(frame_back.pick.text().in()) == "hello");
    CHECK(frame_back.tint == m::GREEN);
    frame.pick.where(holder.points[0]);
    frame_back = decoded<m::Frame>(encoded(frame));
    CHECK(frame_back.pick._d() == 4 && frame_back.pick.where().x == 5);

    // The three map classes of the clause stand beside every type.
    m::UInt32PointMap by_id;
    by_id.set_max_size(1);
    CHECK(by_id.insert(3u, holder.points[0]) == DDS::RETCODE_OK);
    m::StringColorMap by_name;
    CHECK(by_name.get_max_size() == 0u);
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
fn xtypes_7_5_1_the_classic_types_are_cpp03_and_the_codec_writes_the_native_bytes() {
    let found = compilers();
    if found.is_empty() {
        eprintln!("SKIPPED: neither clang++ nor g++ on the path.");
        return;
    }
    let dir = scratch("cpp-classic");
    let file = dir.join("types.idl");
    std::fs::write(&file, IDL).unwrap();
    let output = idlc()
        .args([
            "generate",
            "--lang",
            "c++",
            "--cpp-mapping",
            "classic",
            "--out",
        ])
        .arg(&dir)
        .arg(&file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "idlc generate types.idl: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let header = std::fs::read_to_string(dir.join("types.hpp")).unwrap();

    // The C++03 part, alone.
    let begin = header.find("ZD-CLASSIC-BEGIN").expect("the opening marker");
    let end = header.find("ZD-CLASSIC-END").expect("the closing marker");
    assert!(begin < end);
    let region = &header[begin..end];
    let region = &region[region.find('\n').unwrap() + 1..];
    let three = dir.join("classic03.cpp");
    std::fs::write(&three, format!("{region}\nint main() {{ return 0; }}\n")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for cxx in &found {
        let compiled = Command::new(cxx)
            .args(["-std=c++03", "-Wall", "-Wextra", "-Werror"])
            .arg(format!("-I{}", root.join("runtime/cpp/include").display()))
            .arg(&three)
            .arg("-o")
            .arg(dir.join(format!("classic03-{cxx}")))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{cxx}: the classic declarations are not C++03:\n{}\n{region}",
            String::from_utf8_lossy(&compiled.stderr)
        );
    }

    // The whole header, with the codec, under C++17.
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

const D2_IDL: &str = "module d {\n\
  @verbatim(language = \"cc\", placement = BEFORE_DECLARATION, text = \"// d2-verbatim\")\n\
  @final struct Defaults { @default(7) long seven; @default(TRUE) boolean yes; long plain; };\n\
  @final bitset Bits { bitfield<1> flag; bitfield<3>; bitfield<4, unsigned short> nibble; };\n\
  @final bitset MoreBits : Bits { bitfield<2> two; };\n\
  @bit_bound(16) bitmask Mask { @position(0) LOW, @position(15) HIGH };\n\
  typedef map<long, string, 2> Bounded;\n\
  @final struct Holder { Bounded names; map<long, long> counts; @optional string note; MoreBits bits; ::d::Mask mask; };\n\
  @final union ByWChar switch (wchar) { case L'a': long a; default: short other; };\n\
  @final union ByOctet switch (octet) { case 1: long one; case 2: string two; };\n\
  @final union ByInt8 switch (int8) { case -1: long minus; case 1: long plus; };\n\
};\n";

const D2_MAIN: &str = r#"#include "d2.hpp"
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#define CHECK(c) do { if (!(c)) { std::printf("line %d: %s\n", __LINE__, #c); ++bad; } } while (0)

template <typename T>
static T round_trip(const T &value) {
    std::vector<std::uint8_t> bytes = dds::topic::topic_type_support<T>::encode(value);
    return dds::topic::topic_type_support<T>::decode(
        bytes.data(), bytes.size(), zerodds::wire::Encoding::PlainCdr2, false);
}

int main() {
    int bad = 0;
    // Table D.4: @default initializes the member.
    d::Defaults defaults;
    CHECK(defaults.seven == 7 && defaults.yes && defaults.plain == 0);
    // D.2.1.3.1: the map class.
    d::Bounded names;
    CHECK(names.maximum() == 2 && names.length() == 0);
    CHECK(names.insert(1, "one") && names.insert(2, "two"));
    CHECK(!names.insert(3, "three"));                         // past the maximum
    CHECK(!names.insert(1, "uno", false));                    // held, replace FALSE
    CHECK(names.insert_or_assign(1, "uno") && std::string(names[1].in()) == "uno");
    CHECK(names.erase(2) && !names.erase(2) && names.length() == 1);
    names.clear();
    CHECK(names.length() == 0 && names.maximum() == 2);
    zerodds::classic::Map<CORBA::Long, CORBA::Long> counts(5);
    CHECK(counts.maximum() == 5);
    counts.maximum(1);
    CHECK(counts.insert(1, 1) && !counts.insert(2, 2));
    zerodds::classic::Map<CORBA::Long, CORBA::Long> copy(counts);
    CHECK(copy.length() == 1 && copy.maximum() == 1);
    // D.2.1.3.2 and D.2.1.3.3: bit fields and the C.2.4.1.1 struct.
    d::MoreBits bits = d::MoreBits();
    bits.flag = true;
    bits.nibble = 9;
    bits.two = 3;
    d::Mask mask;
    mask |= d::Mask::HIGH;
    CHECK(mask() == 0x8000u);
    // Table D.3: an optional string is a char*, which the structure owns and copies deeply.
    d::Holder holder;
    CHECK(holder.note == 0);
    holder.note = CORBA::string_dup("note");
    holder.names.insert(5, "five");
    holder.counts.insert(1, 2);
    holder.bits = bits;
    holder.mask = mask;
    d::Holder copied(holder);
    CHECK(copied.note != holder.note && std::strcmp(copied.note, "note") == 0);
    d::Holder back = round_trip(holder);
    CHECK(back.note != 0 && std::strcmp(back.note, "note") == 0);
    CHECK(back.names.length() == 1 && std::string(back.names[5].in()) == "five");
    CHECK(back.counts[1] == 2);
    CHECK(back.bits.flag && back.bits.nibble == 9 && back.bits.two == 3);
    CHECK(back.mask() == 0x8000u);
    // D.2.1.2: wchar, octet and int8 discriminators.
    d::ByWChar w;
    w.a(3);
    d::ByWChar w_back = round_trip(w);
    CHECK(w_back._d() == L'a' && w_back.a() == 3);
    d::ByOctet o;
    o.two("deux");
    d::ByOctet o_back = round_trip(o);
    CHECK(o_back._d() == 2 && std::string(o_back.two().in()) == "deux");
    d::ByInt8 i;
    i.minus(4);
    d::ByInt8 i_back = round_trip(i);
    CHECK(i_back._d() == -1 && i_back.minus() == 4);
    return bad;
}
"#;

/// IDL4 to C++ 1.0 Annex D.2, for the classic mapping: the map class of D.2.1.3.1, bit sets as
/// bit fields (D.2.1.3.2), the bit mask struct of C.2.4.1.1 (D.2.1.3.3), wchar, octet and int8
/// discriminators (D.2.1.2), a plain pointer for an `@optional` member (Table D.3), `@default`
/// (Table D.4) and `@verbatim` (D.2.3.5); the C++03 part alone and a round trip through the codec.
#[test]
fn idl4cpp_d_2_the_classic_mapping_takes_the_idl4_building_blocks_as_annex_d_says() {
    let found = compilers();
    if found.is_empty() {
        eprintln!("SKIPPED: neither clang++ nor g++ on the path.");
        return;
    }
    let dir = scratch("cpp-classic-d2");
    let file = dir.join("d2.idl");
    std::fs::write(&file, D2_IDL).unwrap();
    let output = idlc()
        .args([
            "generate",
            "--lang",
            "c++",
            "--cpp-mapping",
            "classic",
            "--out",
        ])
        .arg(&dir)
        .arg(&file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "idlc generate d2.idl: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let header = std::fs::read_to_string(dir.join("d2.hpp")).unwrap();
    let begin = header.find("ZD-CLASSIC-BEGIN").expect("the opening marker");
    let end = header.find("ZD-CLASSIC-END").expect("the closing marker");
    let region = &header[begin..end];
    assert!(
        region.contains("// d2-verbatim"),
        "the verbatim text:\n{region}"
    );
    let region = &region[region.find('\n').unwrap() + 1..];
    let three = dir.join("classic03.cpp");
    std::fs::write(&three, format!("{region}\nint main() {{ return 0; }}\n")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for cxx in &found {
        let compiled = Command::new(cxx)
            .args(["-std=c++03", "-Wall", "-Wextra", "-Werror"])
            .arg(format!("-I{}", root.join("runtime/cpp/include").display()))
            .arg(&three)
            .arg("-o")
            .arg(dir.join(format!("classic03-{cxx}")))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{cxx}: the classic declarations are not C++03:\n{}\n{region}",
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
    let unit = dir.join("main.cpp");
    std::fs::write(&unit, D2_MAIN).unwrap();
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
