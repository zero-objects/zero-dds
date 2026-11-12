// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `generate --cpp-mapping idl4`, `--java-mapping idl4` and
//! `--csharp-mapping idl4` through the command line.
//!
//! A language mapping writes types and interfaces and no codec. IDL4 to
//! C++ 1.0 §7.3 maps `any` ("The IDL any type shall be mapped to a C++
//! omg::types::Any type"), §7.4 an interface and a parameter of interface
//! type; IDL4 to Java 1.0 and IDL4 to C# 1.0 have the same clauses. A
//! declaration that holds one of them has no type of DDS-XTypes 1.3 and
//! is left out for the languages that carry a codec; a language mapping
//! writes it.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

const IDL: &str = "struct Holder { any value; };\n\
interface Peer { Holder get(in Peer other); };\n\
struct Link { Peer far_end; long hops; };\n\
typedef sequence<Link> Links;\n\
struct Route { Links taken; };\n";

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every file under `dir`, as its path below `dir` and its text.
fn files(dir: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut open = vec![dir.to_path_buf()];
    while let Some(next) = open.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                open.push(path);
            } else {
                let name = path
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                found.push((name, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    found.sort();
    found
}

/// The files `generate --lang <lang> <flags>` writes for `source`, and
/// what it reported.
fn generate(
    tag: &str,
    source: &str,
    lang: &str,
    flags: &[&str],
) -> (Vec<(String, String)>, String) {
    let dir = scratch(tag);
    let idl = dir.join("route.idl");
    std::fs::write(&idl, source).unwrap();
    let out = dir.join("out");
    let run = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
        .args(["generate", "--lang", lang])
        .args(flags)
        .arg("--out")
        .arg(&out)
        .arg(&idl)
        .output()
        .unwrap();
    let stderr = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(run.status.success(), "{lang} {flags:?}: {stderr}");
    (files(&out), stderr)
}

fn text_of<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
    files
        .iter()
        .find(|(path, _)| path == name)
        .map(|(_, text)| text.as_str())
        .unwrap_or_else(|| {
            let names: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
            panic!("no {name} among {names:?}")
        })
}

/// IDL4 to C++ 1.0 §7.3 and §7.4 through `zerodds-idlc`: the structure
/// with an `any` member, the interface, the structure that holds a
/// reference to it and the typedef and structure behind those are all in
/// the header, and nothing is reported as left out.
#[test]
fn idl4cpp_7_3_a_structure_with_an_any_member_is_written_by_the_command_line() {
    let (files, stderr) = generate("idl4-cli-cpp", IDL, "c++", &["--cpp-mapping", "idl4"]);
    let header = text_of(&files, "route.hpp");
    for expected in [
        "struct Holder",
        "::omg::types::Any value;",
        "class Peer",
        "struct Link",
        "::omg::types::ref_type<::Peer> far_end;",
        "struct Route",
    ] {
        assert!(header.contains(expected), "{expected} missing:\n{header}");
    }
    assert!(!stderr.contains("no code is generated"), "{stderr}");
}

/// IDL4 to Java 1.0 §7.3 and §7.4 through `zerodds-idlc`: a file per
/// declaration, none left out.
#[test]
fn idl4java_7_3_a_structure_with_an_any_member_is_written_by_the_command_line() {
    let (files, stderr) = generate("idl4-cli-java", IDL, "java", &["--java-mapping", "idl4"]);
    for name in ["Holder.java", "Peer.java", "Link.java", "Route.java"] {
        text_of(&files, name);
    }
    assert!(!stderr.contains("no code is generated"), "{stderr}");
}

/// IDL4 to C# 1.0 §7.3 and §7.4 through `zerodds-idlc`: a file per
/// declaration, none left out.
#[test]
fn idl4csharp_7_3_a_structure_with_an_any_member_is_written_by_the_command_line() {
    let (files, stderr) = generate(
        "idl4-cli-csharp",
        IDL,
        "csharp",
        &["--csharp-mapping", "idl4"],
    );
    for name in ["Holder.cs", "IPeer.cs", "Link.cs", "Route.cs"] {
        text_of(&files, name);
    }
    assert!(!stderr.contains("no code is generated"), "{stderr}");
}

/// DDS-XTypes 1.3 §7.3.1.1.1 stays as it was for a language that carries
/// a codec: the declarations that reach `any` or an object reference are
/// reported and left out, the file's other declarations are generated.
#[test]
fn xtypes_7_3_1_1_1_a_codec_language_still_reports_what_it_leaves_out() {
    let (files, reported) = generate(
        "idl4-cli-python",
        "struct Holder { any value; };\nstruct Plain { long x; };\n",
        "python",
        &[],
    );
    assert!(
        reported.contains("::Holder reaches `any`") && reported.contains("no code is generated"),
        "{reported}"
    );
    let module = &files.first().expect("a module").1;
    assert!(
        module.contains("Plain") && !module.contains("Holder"),
        "{module}"
    );
}
