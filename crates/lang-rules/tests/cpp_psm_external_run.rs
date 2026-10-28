// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@external` under `CppMapping::Psm`: `dds::core::external<T>`, compiled
//! and run through the PSM class, `zerodds::psm::Conv` and the native codec.
//!
//! DDS-XTypes 1.3 §7.5.1.2.3.3 and §7.5.1.2.3.3.16: "A member that is both
//! external and optional shall be mapped as if it was just external. The
//! difference is that it is valid for ptr_ to be empty when writing a sample
//! containing this member. If the member is only external but not optional,
//! then it is not allowed for ptr_ to be empty at the time of a write."
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_rules::adapter::cpp::{CppMapping, CppMappingScope};
use zerodds_lang_rules::adapter::interface::ServiceMapping;
use zerodds_lang_rules::emit::encode::CodecOptions;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn header(idl: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "psm.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    let _scope = CppMappingScope::enter(CppMapping::Psm);
    zerodds_lang_rules::generate::cpp_source_with_mapping(
        &ast,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
        CodecOptions::default(),
        "zd_codec",
        ServiceMapping::Basic,
    )
    .expect("generate")
    .text
}

/// The head the C++ generator writes: the includes of the native column and
/// those the PSM classes need.
const HEAD: &str = "#pragma once\n#include <array>\n#include <cstdint>\n#include <map>\n\
                    #include <memory>\n#include <optional>\n#include <string>\n\
                    #include <utility>\n#include <vector>\n#include \"zerodds/wire.hpp\"\n\
                    #include \"zerodds/psm/convert.hpp\"\n";

#[test]
fn a_psm_external_member_is_dds_core_external_and_an_unset_one_is_refused_at_a_write() {
    let dir = std::env::temp_dir().join(format!("zd-psm-external-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a work directory");
    std::fs::write(
        dir.join("gen.hpp"),
        format!(
            "{HEAD}{}",
            header("@final struct E { @external long a; @optional @external long b; };")
        ),
    )
    .expect("the header");
    std::fs::write(
        dir.join("main.cpp"),
        r#"
#include <cstdio>
#include "gen.hpp"
#include "zerodds/psm/convert.hpp"
using zerodds::wire::Writer;
using zerodds::wire::Endianness;

static_assert(std::is_same<decltype(std::declval<E &>().a()), ::dds::core::external<int32_t> &>::value,
              "an external member is dds::core::external<T>");

int main() {
    // Only external, empty at the write: not allowed.
    ::dds::core::external<int32_t> none;
    E unset(none, none);
    bool refused = false;
    try {
        Writer w(Endianness::Little, 4);
        auto native = zerodds::psm::Conv<E>::to_native(unset);
        zd_codec::zd_enc_E(w, native);
    } catch (const zerodds::wire::UnsetExternal &) {
        refused = true;
    }
    std::printf("unset: %s\n", refused ? "refused" : "written");
    // Set, with the optional one empty: written, with the marker 0.
    ::dds::core::external<int32_t> five(new int32_t(5));
    E set(five, none);
    Writer w(Endianness::Little, 4);
    auto native = zerodds::psm::Conv<E>::to_native(set);
    zd_codec::zd_enc_E(w, native);
    for (auto b : w.bytes()) { std::printf("%02x", static_cast<unsigned>(b)); }
    std::printf("\n");
    return 0;
}
"#,
    )
    .expect("the program");
    let runtime = root().join("runtime/cpp");
    let binary = dir.join("program");
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let mut command = Command::new(cxx);
    command
        .args([
            "-std=c++17",
            "-Wall",
            "-Wextra",
            "-Wno-unused-function",
            "-Werror",
        ])
        .arg(format!("-I{}", dir.display()))
        .arg(format!("-I{}", root().join("crates/cpp/include").display()))
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
        "does not compile:\n{}\n{}",
        String::from_utf8_lossy(&compiled.stderr),
        std::fs::read_to_string(dir.join("gen.hpp")).unwrap_or_default()
    );
    let ran = Command::new(&binary).output().expect("the program starts");
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let out = String::from_utf8_lossy(&ran.stdout).into_owned();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "unset: refused", "{out}");
    assert_eq!(lines[1], "0500000000", "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
