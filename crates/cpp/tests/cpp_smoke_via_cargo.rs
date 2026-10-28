// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! Cargo-triggered C++ smoke test: compiles
//! `crates/cpp/tests/smoke_dds_psm.cpp` against libzerodds.dylib and
//! checks that the DDS-PSM-Cxx 1.0 header interface works end-to-end.
//!
//! Requires `clang++` (or `c++`) in PATH.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .expect("workspace root")
}

#[test]
fn dds_psm_cxx_smoke() {
    build_and_run(None);
}

/// The threaded smoke tests of DDS-PSM-Cxx 1.0 §7.3 (reentrancy), compiled with
/// `-fsanitize=thread`: a data race in the C++ headers is a failure. The Rust library is not
/// instrumented; its own locking is invisible to the sanitizer, so the tests are the ones in which
/// the threads touch the C++ objects.
#[test]
fn dds_psm_cxx_smoke_under_thread_sanitizer() {
    build_and_run_with(Some("threads"), Some("thread"));
}

/// The whole smoke program compiled with `-fsanitize=address`: a use of a freed object or a
/// write past a buffer in the C++ headers is a failure (the Rust library is not instrumented).
#[test]
fn dds_psm_cxx_smoke_under_address_sanitizer() {
    build_and_run_with(None, Some("address"));
}

/// The whole smoke program compiled as ISO C++ 2003 (`-std=c++03 -pedantic`, with g++ where it
/// exists) and run: the headers are C++03 (DDS-PSM-Cxx 1.0 §3 [C++], §7.1). The tests that need
/// C++11 (threads, move semantics, ...) are not in this program.
#[test]
fn dds_psm_cxx_smoke_as_cxx03() {
    build_and_run_standard(None, None, "c++03");
}

/// The threaded smoke tests without the sanitizer, one program per test, each run a few times.
#[test]
fn the_threaded_smoke_tests_pass_without_the_sanitizer() {
    for argument in ["threads-writers", "threads-conditions", "threads-entities"] {
        for _ in 0..3 {
            build_and_run(Some(argument));
        }
    }
}

/// How many leased domains the smoke program takes its `D(n)` from, in turn. The safe band of the
/// test host has 19 (`zerodds-test-domains`); a test of the program uses at most two at once.
const SMOKE_DOMAINS: usize = 4;

fn build_and_run(argument: Option<&str>) {
    build_and_run_with(argument, None);
}

/// Whether `setarch -R` can start a program here. A container's seccomp profile refuses the
/// personality it sets (`setarch: failed to set personality to x86_64: Operation not
/// permitted`); there the program starts as it is, with the randomization the kernel of that
/// host has.
fn setarch_disables_randomization() -> bool {
    Command::new("setarch")
        .arg(std::env::consts::ARCH)
        .arg("-R")
        .arg("true")
        .output()
        .is_ok_and(|out| out.status.success())
}

/// One smoke program builds and runs at a time (the build directory is shared). Its domains are
/// leased, `ZERODDS_TEST_DOMAINS`, so that no other process of the test host hears it.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn build_and_run_with(argument: Option<&str>, sanitizer: Option<&str>) {
    build_and_run_standard(argument, sanitizer, "c++17");
}

/// The compiler for the C++03 build: `CXX03`, else `g++` (the one that decides whether the
/// headers are C++03), else `CXX`/`c++`.
fn cxx03_compiler() -> String {
    if let Ok(cxx) = std::env::var("CXX03") {
        return cxx;
    }
    if Command::new("g++")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
    {
        return "g++".to_string();
    }
    std::env::var("CXX").unwrap_or_else(|_| "c++".to_string())
}

fn build_and_run_standard(argument: Option<&str>, sanitizer: Option<&str>, standard: &str) {
    let thread_sanitizer = sanitizer == Some("thread");
    let _turn = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = workspace_root();
    let cpp_inc = root.join("crates/cpp/include");
    let c_inc = root.join("crates/zerodds-c-api/include");
    let wire_inc = root.join("runtime/cpp/include");
    let src = root.join("crates/cpp/tests/smoke_dds_psm.cpp");

    // Build the cdylib first, into a target directory of its own.
    //
    // Two reasons, and the second is why this is not the workspace's.
    // A `cargo build` inside a `cargo test` writes where the outer run
    // is reading and writing, and on CI that directory is a 64 GB cache
    // shared across pipelines. A cancelled run leaves half-written
    // `.rmeta` behind, and the next build over them answers
    //
    //     error[E0786]: found invalid metadata files for crate
    //     `zerodds_rtps`
    //
    // which is what this test hit after a day of cancelled pipelines.
    // Its own directory cannot be poisoned by the outer run and cannot
    // poison it.
    //
    // And the failure says what happened: `.status()` threw the
    // compiler's account away and left `cargo build zerodds-c-api
    // failed` with nothing behind it.
    let build_dir = root.join("target/cpp-smoke");
    let out = Command::new(env!("CARGO"))
        .args(["build", "-p", "zerodds-c-api", "--release"])
        .arg("--target-dir")
        .arg(&build_dir)
        .current_dir(&root)
        .output()
        .expect("cargo build");
    assert!(
        out.status.success(),
        "cargo build zerodds-c-api failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Locate libzerodds.dylib (macOS) / libzerodds.so (Linux).
    // CI sets --target=<triple>, so the output lib is under
    // target/<triple>/release/; locally without a target under
    // target/release/. Search both variants + fall back to deps/.
    let lib_names = ["libzerodds.dylib", "libzerodds.so"];
    let candidate_dirs = {
        // The build above put it in the first two; the rest stay as a
        // fallback for a tree where it was built some other way.
        let mut v: Vec<PathBuf> = vec![
            build_dir.join("release"),
            build_dir.join("release/deps"),
            root.join("target/release"),
            root.join("target/release/deps"),
        ];
        if let Ok(entries) = std::fs::read_dir(root.join("target")) {
            for e in entries.flatten() {
                let rel = e.path().join("release");
                if rel.is_dir() {
                    v.push(rel.clone());
                    v.push(rel.join("deps"));
                }
            }
        }
        v
    };
    let lib_dir = candidate_dirs
        .into_iter()
        .find(|d| lib_names.iter().any(|n| d.join(n).exists()))
        .expect("libzerodds.{dylib,so} not found in any target/*/release[/deps]");

    // Compile.
    let bin = std::env::temp_dir().join(format!(
        "zerodds_cpp_smoke_{}_{}",
        std::process::id(),
        sanitizer.unwrap_or("none")
    ));
    let cxx03 = standard == "c++03";
    let cxx = if cxx03 {
        cxx03_compiler()
    } else {
        std::env::var("CXX").unwrap_or_else(|_| "c++".to_string())
    };
    let mut compiler = Command::new(&cxx);
    compiler.arg(format!("-std={standard}")).arg("-Wall");
    if cxx03 {
        // ISO C++ 2003 as the specification asks (§3 [C++], §7.1); `long long` is a C99/C++11
        // type the C API uses.
        compiler.args([
            "-pedantic",
            "-Wno-long-long",
            "-Wno-c++11-long-long",
            "-pthread",
        ]);
        if cxx.contains("clang") {
            compiler.arg("-Wno-unknown-warning-option");
        }
    }
    if let Some(kind) = sanitizer {
        compiler
            .arg(format!("-fsanitize={kind}"))
            .args(["-g", "-O1", "-fno-omit-frame-pointer"]);
    }
    let out = compiler
        .arg("-I")
        .arg(&cpp_inc)
        .arg("-I")
        .arg(&c_inc)
        .arg("-I")
        .arg(&wire_inc)
        .arg(&src)
        .arg("-L")
        .arg(&lib_dir)
        .args(["-lzerodds"])
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("c++ compile");
    if !out.status.success() {
        panic!(
            "c++ compile failed:\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // Run.
    // clang's ThreadSanitizer stops with "unexpected memory mapping" on a kernel whose address
    // space randomization is wider than the sanitizer's shadow memory assumes (the Linux test
    // host's kernel); `setarch -R` starts the program without randomization, which is the documented
    // way around it.
    let mut run = if sanitizer.is_some() && setarch_disables_randomization() {
        let mut command = Command::new("setarch");
        command.arg(std::env::consts::ARCH).arg("-R").arg(&bin);
        command
    } else {
        Command::new(&bin)
    };
    if let Some(argument) = argument {
        run.arg(argument);
    }
    if thread_sanitizer {
        run.env(
            "TSAN_OPTIONS",
            "halt_on_error=0 exitcode=0 second_deadlock_stack=1",
        );
    }
    // The domains of the program, which its positions `D(n)` take in turn.
    let leases: Vec<_> = (0..SMOKE_DOMAINS)
        .map(|_| zerodds_test_domains::lease_domain())
        .collect();
    let domains = leases
        .iter()
        .map(|lease| lease.id().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let out = run
        .env("ZERODDS_TEST_DOMAINS", &domains)
        .env(
            if cfg!(target_os = "macos") {
                "DYLD_LIBRARY_PATH"
            } else {
                "LD_LIBRARY_PATH"
            },
            &lib_dir,
        )
        .output()
        .expect("run smoke");
    drop(leases);
    let _ = std::fs::remove_file(&bin);
    // The sanitizer's reports go to stderr; the program's exit code is its own verdict.
    if !out.status.success() {
        panic!(
            "smoke test failed:\nbin={}\nlib_dir={}\nstatus={:?}\nstdout={:?}\nstderr={:?}",
            bin.display(),
            lib_dir.display(),
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("OK"),
        "expected OK marker, got: {}\nstderr (the sanitizer replaces the exit code of the program when it reports, so a failed check shows only here):\n{}",
        s,
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .filter(|line| !line.starts_with("[smoke]"))
            .take(60)
            .collect::<Vec<_>>()
            .join("\n")
    );
    if thread_sanitizer {
        // libzerodds is Rust code the sanitizer does not instrument: its locks, channels and
        // atomics are invisible to it, and memory handed from one thread to another through
        // them is reported as a race. A report located in the library (the `SUMMARY` line names
        // the module of the access) is no verdict on the C++ headers and is counted, not failed;
        // a report located anywhere else, in the headers or in the test, fails.
        let stderr = String::from_utf8_lossy(&out.stderr);
        // One report per block between the `==================` lines. Its access stacks are the
        // text before `Location is` / `Thread T` / `Mutex`: a report is the library's when
        // one of the two accesses has a frame in libzerodds (the first frame of an access is
        // often the sanitizer's own memcpy/free interceptor, so the summary line is no guide).
        let blocks: Vec<&str> = stderr
            .split("==================")
            .filter(|block| block.contains("WARNING: ThreadSanitizer: data race"))
            .collect();
        let accesses = |block: &str| -> String {
            let end = ["Location is", "Thread T", "Mutex M", "SUMMARY:"]
                .iter()
                .filter_map(|marker| block.find(marker))
                .min()
                .unwrap_or(block.len());
            block[..end].to_string()
        };
        let reports: Vec<&str> = blocks.clone();
        let own: Vec<String> = blocks
            .iter()
            .filter(|block| !accesses(block).contains("libzerodds"))
            .map(|block| block.lines().take(24).collect::<Vec<_>>().join("\n"))
            .collect();
        eprintln!(
            "ThreadSanitizer: {} reports, {} in libzerodds, {} elsewhere",
            reports.len(),
            reports.len() - own.len(),
            own.len()
        );
        assert!(own.is_empty(), "data races in the C++ code:\n{own:#?}");
    }
}
