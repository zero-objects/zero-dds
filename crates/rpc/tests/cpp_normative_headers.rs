// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The normative C++ headers of DDS-RPC 1.0 §7.11.1 (`omg-dds-rpc-cxx.zip`:
//! `request_reply.h`, `function_call.h`, `sample.h`).
//!
//! * The three files are shipped unchanged in
//!   `runtime/cpp/include/dds/rpc/normative/`: their SHA-256 sums are those of
//!   `omg-dds-rpc-cxx.sha256` there, and, where the zip is at hand
//!   (`ZD_DDS_RPC_CXX_ZIP`, `internal/standards/cache/omg/`,
//!   `~/.cache/zerodds-spec/`), those of the files inside it.
//! * A translation unit that includes them together with
//!   `vendor_dependent.h` compiles with `-Wall -Wextra -Werror`. Against empty
//!   vendor files the same unit had 254 errors.
//! * A program written against them (`tests/cpp/normative_roundtrip.cpp`)
//!   runs a request/reply and a function call against the ZeroDDS runtime over
//!   `libzerodds` (Linux, or `ZD_RUN_GENERATED=1`).
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

const ZIP: &str = "omg-dds-rpc-cxx.zip";
const FILES: [&str; 3] = ["request_reply.h", "function_call.h", "sample.h"];

const INTERFACE: &str = "
    module robot {
      interface Calculator {
        long add(in long a, in long b);
      };
    };
";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn generated_header() -> String {
    let parsed = zerodds_idl_parse::parse(INTERFACE);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "calculator.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    // The whole generator path: the data types, the PSM classes of the
    // default mapping (DDS-PSM-Cxx 1.0, Table 7.1) and the RPC binding.
    let body = zerodds_lang_rules::generate::source_with_mapping(
        &ast,
        zerodds_lang_cst::Language::Cpp,
        zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
        zerodds_lang_rules::emit::encode::CodecOptions::default(),
        zerodds_lang_rules::generate::ServiceMapping::Enhanced,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    // What `zerodds-idlc generate` puts in front of the body: the runtime of
    // the RPC binding, the codec and the conversions of the PSM classes.
    let mut header = String::from("#pragma once\n\n#include <dds/rpc.hpp>\n");
    for include in zerodds_lang_rules::adapter::cpp::includes() {
        header.push_str(&format!("#include <{}>\n", include.path));
    }
    header.push_str("#include <map>\n#include <zerodds/psm/convert.hpp>\n\n");
    header.push_str(&body);
    header
}

fn run(command: &mut Command, what: &str) -> std::process::Output {
    let output = command.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        output.status.success(),
        "{what} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn normative_dir() -> PathBuf {
    root().join("runtime/cpp/include/dds/rpc/normative")
}

fn sha256(path: &Path) -> String {
    for tool in [&["sha256sum"][..], &["shasum", "-a", "256"][..]] {
        if let Ok(out) = Command::new(tool[0]).args(&tool[1..]).arg(path).output() {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout)
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string();
            }
        }
    }
    panic!("neither sha256sum nor shasum is available");
}

fn locate_zip() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("ZD_DDS_RPC_CXX_ZIP") {
        candidates.push(PathBuf::from(p));
    }
    candidates.push(root().join("internal/standards/cache/omg").join(ZIP));
    candidates.push(
        root()
            .join("../zerodds/internal/standards/cache/omg")
            .join(ZIP),
    );
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(Path::new(&home).join(".cache/zerodds-spec").join(ZIP));
    }
    candidates.into_iter().find(|c| c.is_file())
}

#[test]
fn the_shipped_headers_are_the_omg_files() {
    let list = std::fs::read_to_string(normative_dir().join("omg-dds-rpc-cxx.sha256"))
        .expect("the sha256 list");
    for file in FILES {
        let expected = list
            .lines()
            .filter_map(|l| l.split_once("  "))
            .find(|(_, name)| name.trim() == file)
            .map(|(sum, _)| sum.to_string())
            .unwrap_or_else(|| panic!("{file} is not in the sha256 list"));
        assert_eq!(
            sha256(&normative_dir().join(file)),
            expected,
            "{file} differs from the OMG file recorded in omg-dds-rpc-cxx.sha256"
        );
    }
    let Some(zip) = locate_zip() else {
        println!("{ZIP} not at hand: only the recorded sums were checked");
        return;
    };
    let tmp = std::env::temp_dir().join(format!("zd-normative-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("tmp");
    let st = Command::new("unzip")
        .args(["-q", "-o"])
        .arg(&zip)
        .arg("-d")
        .arg(&tmp)
        .status()
        .expect("unzip");
    assert!(st.success(), "unzip {zip:?}");
    for file in FILES {
        let inside = std::fs::read_dir(&tmp)
            .expect("dir")
            .filter_map(Result::ok)
            .map(|e| e.path().join(file))
            .find(|p| p.is_file())
            .unwrap_or_else(|| panic!("{file} not in {zip:?}"));
        assert_eq!(
            sha256(&inside),
            sha256(&normative_dir().join(file)),
            "{file} differs from the file in the zip"
        );
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn the_normative_headers_compile_against_vendor_dependent_h() {
    let tmp = std::env::temp_dir().join(format!("zd-normative-tu-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("tmp");
    std::fs::write(
        tmp.join("main.cpp"),
        "#include <dds/rpc/normative/request_reply.h>\n#include <dds/rpc/normative/function_call.h>\n\
         int main() { return 0; }\n",
    )
    .expect("main");
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let out = Command::new(cxx)
        .args(["-std=c++17", "-fsyntax-only", "-Wall", "-Wextra", "-Werror"])
        .arg(format!(
            "-I{}",
            root().join("runtime/cpp/include").display()
        ))
        .arg(tmp.join("main.cpp"))
        .output()
        .expect("c++");
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_program_against_the_normative_headers_runs_against_libzerodds() {
    if !(cfg!(target_os = "linux") || std::env::var("ZD_RUN_GENERATED").is_ok_and(|v| v == "1")) {
        println!("not Linux and ZD_RUN_GENERATED is not 1: the program is compiled by no one here");
        return;
    }
    let root = root();
    let build_dir = root.join("target/cpp-smoke");
    run(
        Command::new(env!("CARGO"))
            .args(["build", "-p", "zerodds-c-api", "--release", "--target-dir"])
            .arg(&build_dir)
            .current_dir(&root),
        "cargo build zerodds-c-api",
    );
    let lib_dir = build_dir.join("release");
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("dds-rpc-cpp")
        .join("normative");
    std::fs::create_dir_all(&at).expect("the build directory");
    std::fs::write(at.join("calculator.hpp"), generated_header()).expect("the generated header");
    let binary = at.join("normative_roundtrip");
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    run(
        Command::new(cxx)
            .args(["-std=c++17", "-Wall", "-Wextra", "-pthread"])
            .arg(format!("-I{}", at.display()))
            .arg(format!("-I{}", root.join("runtime/cpp/include").display()))
            .arg(format!("-I{}", root.join("crates/cpp/include").display()))
            .arg(format!(
                "-I{}",
                root.join("crates/zerodds-c-api/include").display()
            ))
            .arg(root.join("crates/rpc/tests/cpp/normative_roundtrip.cpp"))
            .args(
                ["writer", "reader", "float128", "md5"]
                    .map(|unit| root.join(format!("runtime/cpp/src/{unit}.cpp"))),
            )
            .arg(format!("-L{}", lib_dir.display()))
            .arg("-lzerodds")
            .arg("-o")
            .arg(&binary),
        "compiling the normative program",
    );
    let output = run(
        Command::new(&binary)
            .env("LD_LIBRARY_PATH", &lib_dir)
            .env("DYLD_LIBRARY_PATH", &lib_dir),
        "the normative program",
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program said: {text}"
    );
}
