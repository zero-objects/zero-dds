// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The native C++ binding of optional and external members, compiled and
//! run.
//!
//! DDS-XTypes 1.3 §7.5.1.2.4.2 (an optional member is a plain pointer,
//! NULL for an omitted value), §7.5.1.2.4.4 (an optional array is a
//! pointer to the array), §7.5.1.2.3.2 (the constructor, destructor and
//! copy of an external member behave as in C: NULL, deleted, a deep copy)
//! and §7.5.1.2.3.2.1 (an external member that is not optional may not be
//! unset at a write). The expected octets are the ones §7.4.3.5.2 rule
//! (20) gives an `OPT_FMEMBER`.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compiler() -> String {
    if let Ok(chosen) = std::env::var("CXX") {
        return chosen;
    }
    ["c++", "g++", "clang++"]
        .iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(|found| (*found).to_string())
        .expect("a C++ compiler (c++, g++ or clang++) on the path; this test does not skip")
}

fn generated(idl: &str) -> String {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    let body = zerodds_lang_rules::generate::source(
        &ast,
        Language::Cpp,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    )
    .unwrap_or_else(|why| panic!("generate: {why:?}"));
    // The includes the C++ generator writes at the head of a file.
    format!(
        "#pragma once\n#include <array>\n#include <cstdint>\n#include <memory>\n\
         #include <optional>\n#include <string>\n#include <utility>\n#include <vector>\n\
         #include \"zerodds/wire.hpp\"\n{body}"
    )
}

/// Compile `idl`'s header, a `main` made of `body` and the runtime, run it
/// and answer its standard output.
fn run(tag: &str, idl: &str, body: &str) -> String {
    let dir = std::env::temp_dir().join(format!("zd-cpp-run-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a work directory");
    std::fs::write(dir.join("gen.hpp"), generated(idl)).expect("the header");
    std::fs::write(
        dir.join("main.cpp"),
        format!(
            "#include <cstdio>\n#include <iostream>\n#include \"gen.hpp\"\n\
             using zerodds::wire::Writer;\nusing zerodds::wire::Reader;\n\
             using zerodds::wire::Endianness;\n\
             void hex(const Writer &w) {{\n\
             \x20   for (auto b : w.bytes()) {{ std::printf(\"%02x\", static_cast<unsigned>(b)); }}\n\
             \x20   std::printf(\"\\n\");\n}}\n\
             int main() {{\n{body}\n    return 0;\n}}\n"
        ),
    )
    .expect("the program");
    let runtime = root().join("runtime/cpp");
    let binary = dir.join("program");
    let mut command = Command::new(compiler());
    command
        .args([
            "-std=c++17",
            "-Wall",
            "-Wextra",
            "-Wno-unused-function",
            "-Werror",
        ])
        .arg(format!("-I{}", dir.display()))
        .arg(format!("-I{}", runtime.join("include").display()))
        .arg("-o")
        .arg(&binary)
        .arg(dir.join("main.cpp"));
    for source in ["writer.cpp", "reader.cpp", "float128.cpp", "md5.cpp"] {
        command.arg(runtime.join("src").join(source));
    }
    let compiled = command.output().expect("the compiler starts");
    assert!(
        compiled.status.success(),
        "the generated C++ does not compile:\n{}\n{}",
        String::from_utf8_lossy(&compiled.stderr),
        std::fs::read_to_string(dir.join("gen.hpp")).unwrap_or_default()
    );
    let ran = Command::new(&binary).output().expect("the program starts");
    let stdout = String::from_utf8_lossy(&ran.stdout).into_owned();
    assert!(
        ran.status.success(),
        "the program failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    stdout
}

const OPTIONALS: &str =
    "@final struct S { long a; @optional long b; @optional string c; long d; };";

#[test]
fn an_optional_member_is_written_with_its_marker_and_read_back_through_a_pointer() {
    let out = run(
        "optional",
        OPTIONALS,
        r#"
    S sample;
    sample.a = 1;
    sample.b = new std::int32_t(2);
    sample.d = 4;
    Writer w(Endianness::Little, 4);
    zd_enc_S(w, sample);
    hex(w);
    {
        S back;
        Reader r(w.bytes(), Endianness::Little, 4);
        zd_dec_S(r, back, zerodds::wire::TryConstruct::Discard);
        std::printf("read a=%d b=%d c=%d d=%d\n", static_cast<int>(back.a),
                    back.b != nullptr ? static_cast<int>(*back.b) : -1, back.c != nullptr,
                    static_cast<int>(back.d));
    }
    zerodds::wire::destroy(sample.b);
    Writer w2(Endianness::Little, 4);
    zd_enc_S(w2, sample);
    hex(w2);
    S back2;
    back2.b = new std::int32_t(77);
    Reader r2(w2.bytes(), Endianness::Little, 4);
    zd_dec_S(r2, back2, zerodds::wire::TryConstruct::Discard);
    std::printf("read b=%d c=%d\n", back2.b != nullptr, back2.c != nullptr);
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines[0],
        "01000000".to_string() + "01000000" + "02000000" + "00000000" + "04000000",
        "{out}"
    );
    assert_eq!(lines[1], "read a=1 b=2 c=0 d=4", "{out}");
    assert_eq!(
        lines[2],
        "01000000".to_string() + "00000000" + "04000000",
        "{out}"
    );
    assert_eq!(lines[3], "read b=0 c=0", "{out}");
}

#[test]
fn an_optional_string_is_a_pointer_to_a_string() {
    let out = run(
        "optional-string",
        OPTIONALS,
        r#"
    S sample;
    sample.a = 1;
    sample.c = new std::string("hi");
    sample.d = 4;
    Writer w(Endianness::Little, 4);
    zd_enc_S(w, sample);
    hex(w);
    S back;
    Reader r(w.bytes(), Endianness::Little, 4);
    zd_dec_S(r, back, zerodds::wire::TryConstruct::Discard);
    std::printf("read b=%d c=%s\n", back.b != nullptr, back.c != nullptr ? back.c->c_str() : "(null)");
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines[0],
        "01000000".to_string() + "0001" + "0000" + "03000000" + "686900" + "00" + "04000000",
        "{out}"
    );
    assert_eq!(lines[1], "read b=0 c=hi", "{out}");
}

#[test]
fn an_optional_array_is_a_pointer_to_the_array_and_not_an_array_of_pointers() {
    // §7.5.1.2.4.4: `@optional long array_member[10]` is
    // `int32_t (*array_member)[10]`. The array of this binding is
    // `std::array<std::int32_t, N>`, which has the layout of the C array,
    // so the pointer to it is the pointer the clause spells.
    let out = run(
        "optional-array",
        "@final struct M { @optional long array_member[3]; };",
        r#"
    static_assert(sizeof(std::array<std::int32_t, 3>) == sizeof(std::int32_t[3]));
    M sample;
    sample.array_member = new std::array<std::int32_t, 3>{{1, 2, 3}};
    std::int32_t (*spelled_in_the_clause)[3] =
        reinterpret_cast<std::int32_t (*)[3]>(sample.array_member);
    std::printf("first %d\n", static_cast<int>((*spelled_in_the_clause)[0]));
    Writer w(Endianness::Little, 4);
    zd_enc_M(w, sample);
    hex(w);
    M back;
    Reader r(w.bytes(), Endianness::Little, 4);
    zd_dec_M(r, back, zerodds::wire::TryConstruct::Discard);
    std::printf("read %d %d %d\n", static_cast<int>((*back.array_member)[0]),
                static_cast<int>((*back.array_member)[1]), static_cast<int>((*back.array_member)[2]));
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "first 1", "{out}");
    assert_eq!(
        lines[1],
        "01000000".to_string() + "01000000" + "02000000" + "03000000",
        "{out}"
    );
    assert_eq!(lines[2], "read 1 2 3", "{out}");
}

#[test]
fn a_copy_of_a_class_with_a_pointer_member_is_deep_and_the_destructor_deletes() {
    // §7.5.1.2.3.2: constructor, destructor and copy "the same as
    // specified for C": NULL, deleted, deep.
    let out = run(
        "deep-copy",
        "@final struct E { long a; @optional long b; @external long c; };",
        r#"
    E x;
    std::printf("fresh %d %d\n", x.b == nullptr, x.c == nullptr);
    x.a = 7;
    x.b = new std::int32_t(9);
    x.c = std::make_shared<std::int32_t>(5);
    E y = x;
    std::printf("ctor %d %d %d %d\n", y.a, *y.b, y.b != x.b, y.c != x.c);
    E z;
    z.b = new std::int32_t(1);
    z = x;
    std::printf("assign %d %d %d\n", *z.b, z.b != x.b, *z.c);
    *x.b = 100;
    std::printf("independent %d %d\n", *y.b, *z.b);
    E moved = std::move(y);
    std::printf("move %d %d\n", moved.b != nullptr, y.b == nullptr);
    E &again = z;
    z = again;
    std::printf("self %d\n", *z.b);
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "fresh 1 1", "{out}");
    assert_eq!(lines[1], "ctor 7 9 1 1", "{out}");
    assert_eq!(lines[2], "assign 9 1 5", "{out}");
    assert_eq!(lines[3], "independent 9 9", "{out}");
    assert_eq!(lines[4], "move 1 1", "{out}");
    assert_eq!(lines[5], "self 9", "{out}");
}

#[test]
fn an_external_member_that_is_not_optional_may_not_be_unset_at_a_write() {
    // §7.5.1.2.3.2.1.
    let out = run(
        "external-unset",
        "@final struct E { @external long a; @optional @external long b; };",
        r#"
    E sample;
    try {
        Writer w(Endianness::Little, 4);
        zd_enc_E(w, sample);
        std::printf("no refusal\n");
    } catch (const zerodds::wire::UnsetExternal &) {
        std::printf("unset: refused\n");
    }
    sample.a = std::make_shared<std::int32_t>(5);
    Writer w(Endianness::Little, 4);
    zd_enc_E(w, sample);
    hex(w);
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "unset: refused", "{out}");
    // The optional external member may be unset: marker 0 after the value.
    assert_eq!(lines[1], "05000000".to_string() + "00", "{out}");
}

#[test]
fn a_member_on_a_cycle_is_an_owned_pointer_and_the_chain_is_freed() {
    let out = run(
        "cycle",
        "struct Node; @mutable struct Node { long value; @optional Node next; };",
        r#"
    Node head;
    head.value = 1;
    head.next = new Node();
    head.next->value = 2;
    Node copy = head;
    std::printf("chain %d %d %d\n", copy.value, copy.next->value, copy.next != head.next);
    Writer w(Endianness::Little, 4);
    zd_enc_Node(w, head);
    Node back;
    Reader r(w.bytes(), Endianness::Little, 4);
    zd_dec_Node(r, back, zerodds::wire::TryConstruct::Discard);
    std::printf("read %d %d %d\n", back.value, back.next->value, back.next->next == nullptr);
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "chain 1 2 1", "{out}");
    assert_eq!(lines[1], "read 1 2 1", "{out}");
}
