// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The build contract of `generate`: `-I` include directories, a depfile
//! naming every IDL file read, a manifest of what was written, and a stamp
//! written last and only for a run that wrote everything
//! (`cmake/zerodds_idlc_generate.cmake` drives it; `cmake/tests/run-idlc-generate.sh`
//! checks the rebuild behaviour with Ninja).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;
use std::process::Command;

fn idlc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("inc")).unwrap();
    dir
}

#[test]
fn an_include_from_a_search_directory_is_a_dependency_and_the_stamp_comes_last() {
    let dir = scratch("build-contract-ok");
    std::fs::write(
        dir.join("inc/common.idl"),
        "struct Point { long x; long y; };\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("robot.idl"),
        "#include \"common.idl\"\nstruct Robot { Point at; };\n",
    )
    .unwrap();
    let meta = dir.join("meta");
    let status = idlc()
        .args(["generate", "--lang", "cpp", "--out"])
        .arg(dir.join("out"))
        .arg("-I")
        .arg(dir.join("inc"))
        .arg("--stamp")
        .arg(meta.join("robot.stamp"))
        .arg("--depfile")
        .arg(meta.join("robot.d"))
        .arg("--output-manifest")
        .arg(meta.join("robot.json"))
        .arg(dir.join("robot.idl"))
        .status()
        .unwrap();
    assert!(status.success());
    assert!(meta.join("robot.stamp").exists());
    let depfile = std::fs::read_to_string(meta.join("robot.d")).unwrap();
    assert!(depfile.starts_with(&meta.join("robot.stamp").display().to_string()));
    assert!(
        depfile.contains("common.idl") && depfile.contains("robot.idl"),
        "{depfile}"
    );
    let manifest = std::fs::read_to_string(meta.join("robot.json")).unwrap();
    assert!(
        manifest.contains("\"outputs\"") && manifest.contains("\"c++\""),
        "{manifest}"
    );
    let outputs = std::fs::read_dir(dir.join("out")).unwrap().count();
    assert!(outputs >= 1, "nothing generated");
}

#[test]
fn a_failed_run_writes_no_stamp() {
    let dir = scratch("build-contract-fail");
    std::fs::write(
        dir.join("robot.idl"),
        "#include \"missing.idl\"\nstruct Robot { Point at; };\n",
    )
    .unwrap();
    let stamp = dir.join("robot.stamp");
    let status = idlc()
        .args(["generate", "--lang", "c", "--out"])
        .arg(dir.join("out"))
        .arg("--stamp")
        .arg(&stamp)
        .arg(dir.join("robot.idl"))
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(
        !stamp.exists(),
        "a failed run must not tell the build it is current"
    );
}

#[test]
fn lang_all_generates_every_language_in_one_run() {
    let dir = scratch("build-contract-all");
    std::fs::write(dir.join("p.idl"), "struct P { long x; };\n").unwrap();
    let manifest = dir.join("p.json");
    let status = idlc()
        .args(["generate", "--lang", "all", "--out"])
        .arg(dir.join("out"))
        .arg("--output-manifest")
        .arg(&manifest)
        .arg(dir.join("p.idl"))
        .status()
        .unwrap();
    assert!(status.success());
    let text = std::fs::read_to_string(&manifest).unwrap();
    for language in ["\"rust\"", "\"c++\"", "\"python\"", "\"java\""] {
        assert!(text.contains(language), "{language} missing: {text}");
    }
}

#[test]
fn two_inputs_writing_one_output_file_are_refused() {
    let dir = scratch("build-contract-collision");
    std::fs::write(dir.join("a.idl"), "struct A { long x; };\n").unwrap();
    std::fs::write(dir.join("b.idl"), "struct B { long y; };\n").unwrap();
    let java = idlc()
        .args(["generate", "--lang", "java", "--out"])
        .arg(dir.join("java"))
        .arg(dir.join("a.idl"))
        .arg(dir.join("b.idl"))
        .output()
        .unwrap();
    assert!(
        !java.status.success(),
        "the second input overwrote the first"
    );
    let stderr = String::from_utf8_lossy(&java.stderr);
    assert!(stderr.contains("already written"), "{stderr}");
    let python = idlc()
        .args(["generate", "--lang", "python", "--out"])
        .arg(dir.join("py"))
        .arg(dir.join("a.idl"))
        .arg(dir.join("b.idl"))
        .status()
        .unwrap();
    assert!(python.success(), "one file per input must still pass");
    assert!(dir.join("py/a.py").exists() && dir.join("py/b.py").exists());
}
