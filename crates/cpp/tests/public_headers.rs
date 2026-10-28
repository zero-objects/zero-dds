// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every public header of `dds/` compiles on its own: `#include <dds/pub/find.hpp>` must not
//! rely on an include of the application. The rule of the CMake target
//! `zerodds_psm_standalone_headers`: one translation unit per header outside `detail/`, nothing
//! links or runs.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(PathBuf::from)
        .expect("workspace root")
}

fn headers(dir: &Path, root: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read include dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            headers(&path, root, out);
        } else if path.extension().is_some_and(|e| e == "hpp") {
            let relative = path
                .strip_prefix(root)
                .expect("relative")
                .to_string_lossy()
                .into_owned();
            if !relative.contains("/detail/") {
                out.push(relative);
            }
        }
    }
}

#[test]
fn every_public_header_compiles_on_its_own() {
    compile_each_header("c++17", &["-Wall", "-Werror"]);
}

/// DDS-PSM-Cxx 1.0 §3 [C++] and §7.1: the headers need ISO/IEC C++ 2003 only. The same check as
/// above with `-std=c++03 -pedantic`; clang accepts C++11 constructs as an extension in C++03
/// mode, so for it they are errors.
#[test]
fn every_public_header_compiles_on_its_own_as_cxx03() {
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let version = Command::new(&cxx)
        .arg("--version")
        .output()
        .expect("c++ --version");
    let mut flags = vec!["-pedantic", "-Wno-long-long"];
    if String::from_utf8_lossy(&version.stdout).contains("clang") {
        flags.extend(["-Werror=c++11-extensions", "-Werror=c++11-narrowing"]);
    }
    compile_each_header("c++03", &flags);
}

fn compile_each_header(standard: &str, flags: &[&str]) {
    let root = workspace_root();
    let include = root.join("crates/cpp/include");
    let mut list = Vec::new();
    headers(&include.join("dds"), &include, &mut list);
    list.sort();
    assert!(list.len() > 100, "found only {} headers", list.len());

    let work = std::env::temp_dir().join(format!(
        "zerodds_cpp_headers_{standard}_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&work).expect("work directory");
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let queue = std::sync::Mutex::new(list.clone());
    let failures = std::sync::Mutex::new(Vec::<String>::new());
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                loop {
                    let Some(header) = queue.lock().unwrap().pop() else {
                        return;
                    };
                    let unit = work.join(format!("{}.cpp", header.replace(['/', '.'], "_")));
                    std::fs::write(&unit, format!("#include <{header}>\n")).expect("unit");
                    let out = Command::new(&cxx)
                        .arg(format!("-std={standard}"))
                        .args(flags)
                        .args(["-fsyntax-only", "-I"])
                        .arg(&include)
                        .arg("-I")
                        .arg(root.join("crates/zerodds-c-api/include"))
                        .arg("-I")
                        .arg(root.join("runtime/cpp/include"))
                        .arg(&unit)
                        .output()
                        .expect("c++");
                    if !out.status.success() {
                        failures.lock().unwrap().push(format!(
                            "{header}:\n{}",
                            String::from_utf8_lossy(&out.stderr)
                                .lines()
                                .take(6)
                                .collect::<Vec<_>>()
                                .join("\n")
                        ));
                    }
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&work);
    let failures = failures.into_inner().unwrap();
    assert!(
        failures.is_empty(),
        "{} of {} headers do not compile on their own:\n{}",
        failures.len(),
        list.len(),
        failures.join("\n")
    );
    eprintln!(
        "{} public headers compile on their own as {standard}",
        list.len()
    );
}

/// The CMake target itself (`zerodds_psm_standalone_headers`), configured and built from the
/// top-level project. It builds the Rust libraries as well, so it is not part of the default run:
/// `cargo test -p zerodds-cpp --test public_headers -- --ignored`.
#[test]
#[ignore = "builds the whole project with CMake; run with --ignored"]
fn the_cmake_target_for_standalone_headers_builds() {
    let root = workspace_root();
    let build = std::env::temp_dir().join(format!("zerodds_cmake_headers_{}", std::process::id()));
    let configure = Command::new("cmake")
        .arg("-S")
        .arg(&root)
        .arg("-B")
        .arg(&build)
        .args([
            "-DZERODDS_BUILD_TESTS=ON",
            "-DZERODDS_BUILD_EXAMPLES=OFF",
            "-DCMAKE_BUILD_TYPE=Release",
        ])
        .output()
        .expect("cmake");
    assert!(
        configure.status.success(),
        "cmake configure failed:\n{}\n{}",
        String::from_utf8_lossy(&configure.stdout),
        String::from_utf8_lossy(&configure.stderr)
    );
    let built = Command::new("cmake")
        .arg("--build")
        .arg(&build)
        .args(["--target", "zerodds_psm_standalone_headers", "-j", "8"])
        .output()
        .expect("cmake --build");
    let tail: String = String::from_utf8_lossy(&built.stdout)
        .lines()
        .rev()
        .take(30)
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::remove_dir_all(&build);
    assert!(
        built.status.success(),
        "zerodds_psm_standalone_headers failed:\n{tail}\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
}

/// Every name the normative headers declare is reachable through the header that declares it:
/// `#include <dds/sub/TSample.hpp>` is enough to write `dds::sub::Sample`. The list is made from
/// the headers of the OMG by `tools/psm-omg-names.py` (`omg_names.txt`: header, namespace, name);
/// a translation unit per line includes the header and names the identifier in a
/// using-declaration.
#[test]
fn every_name_of_the_omg_headers_is_reachable_through_its_header() {
    let root = workspace_root();
    let include = root.join("crates/cpp/include");
    let list = std::fs::read_to_string(root.join("crates/cpp/tests/omg_names.txt"))
        .expect("omg_names.txt");
    let lines: Vec<(String, String, String)> = list
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut parts = l.split_whitespace();
            (
                parts.next().unwrap().to_string(),
                parts.next().unwrap().to_string(),
                parts.next().unwrap().to_string(),
            )
        })
        .collect();
    assert!(lines.len() > 150, "found only {} names", lines.len());
    let work = std::env::temp_dir().join(format!("zerodds_cpp_names_{}", std::process::id()));
    std::fs::create_dir_all(&work).expect("work directory");
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let queue = std::sync::Mutex::new(lines.clone());
    let failures = std::sync::Mutex::new(Vec::<String>::new());
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                loop {
                    let Some((header, namespace, name)) = queue.lock().unwrap().pop() else {
                        return;
                    };
                    let unit = work.join(format!(
                        "{}_{}.cpp",
                        header.replace(['/', '.'], "_"),
                        namespace.replace("::", "_") + "_" + &name
                    ));
                    std::fs::write(
                        &unit,
                        if namespace.ends_with("_def") {
                            // The enumeration inside the struct that `safe_enum` takes; the
                            // normative header declares a namespace there.
                            format!("#include <{header}>\nusing probe = ::{namespace}::{name};\n")
                        } else {
                            format!("#include <{header}>\nnamespace probe {{ using ::{namespace}::{name}; }}\n")
                        },
                    )
                    .expect("unit");
                    let out = Command::new(&cxx)
                        .args(["-std=c++17", "-fsyntax-only", "-I"])
                        .arg(&include)
                        .arg("-I")
                        .arg(root.join("crates/zerodds-c-api/include"))
                        .arg("-I")
                        .arg(root.join("runtime/cpp/include"))
                        .arg(&unit)
                        .output()
                        .expect("c++");
                    if !out.status.success() {
                        failures.lock().unwrap().push(format!(
                            "{header}: {namespace}::{name}: {}",
                            String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")
                        ));
                    }
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&work);
    let mut failures = failures.into_inner().unwrap();
    failures.sort();
    assert!(
        failures.is_empty(),
        "{} of {} names are not reachable through their header:\n{}",
        failures.len(),
        lines.len(),
        failures.join("\n")
    );
}

/// DDS-PSM-Cxx 1.0 §2.2: the namespaces of the specification hold the names of the specification.
/// `tools/psm-omg-names.py` lists the names our public headers (outside `detail/`) declare at
/// namespace level; each one is an identifier of the normative headers (`omg_words.txt`) or is
/// listed with its reason in `vendor_names.txt`. An entry of that file that the headers no
/// longer need fails too.
#[test]
fn no_vendor_name_in_the_normative_namespaces() {
    let root = workspace_root();
    let out = Command::new("python3")
        .arg(root.join("tools/psm-omg-names.py"))
        .arg(root.join("crates/cpp/include"))
        .output()
        .expect("python3 runs tools/psm-omg-names.py");
    assert!(
        out.status.success(),
        "psm-omg-names.py failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let words: std::collections::BTreeSet<String> =
        std::fs::read_to_string(root.join("crates/cpp/tests/omg_words.txt"))
            .expect("omg_words.txt")
            .split_whitespace()
            .map(str::to_string)
            .collect();
    assert!(words.len() > 500, "omg_words.txt has {} words", words.len());
    let vendor: std::collections::BTreeSet<(String, String)> =
        std::fs::read_to_string(root.join("crates/cpp/tests/vendor_names.txt"))
            .expect("vendor_names.txt")
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(|l| {
                let mut p = l.split_whitespace();
                (p.next().unwrap().to_string(), p.next().unwrap().to_string())
            })
            .collect();
    let mut declared = std::collections::BTreeSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut p = line.split_whitespace();
        let (_header, namespace, name) = (p.next(), p.next(), p.next());
        let (Some(namespace), Some(name)) = (namespace, name) else {
            continue;
        };
        if namespace.split("::").any(|part| part == "detail") {
            continue;
        }
        declared.insert((namespace.to_string(), name.to_string()));
    }
    assert!(declared.len() > 150, "found only {} names", declared.len());
    let unknown: Vec<String> = declared
        .iter()
        .filter(|(ns, name)| !words.contains(name) && !vendor.contains(&(ns.clone(), name.clone())))
        .map(|(ns, name)| format!("{ns}::{name}"))
        .collect();
    assert!(
        unknown.is_empty(),
        "names outside the normative headers, not in vendor_names.txt: {}",
        unknown.join(", ")
    );
    let stale: Vec<String> = vendor
        .iter()
        .filter(|(ns, name)| {
            !declared.contains(&(ns.clone(), name.clone())) || words.contains(name)
        })
        .map(|(ns, name)| format!("{ns}::{name}"))
        .collect();
    assert!(
        stale.is_empty(),
        "entries of vendor_names.txt the headers do not need: {}",
        stale.join(", ")
    );
}
