// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The whole interop corpus, as C#, through Roslyn.
//!
//! The counterpart of the `every_corpus_case_that_renders_also_compiles`
//! that `swift_end_to_end.rs`, `go_end_to_end.rs` and eight more carry.
//! The C# column had none: `docs/OPEN-ITEMS.md` records that the six
//! oldest columns lost theirs when `zerodds-idl-<lang>` was removed.
//!
//! A snapshot says the emitter produced the text it produced last time.
//! It does not say the text is C#, that every name in it resolves, or
//! that the conversions the mapping writes exist — C# has no implicit
//! conversion from an enum to an integer, `(bool)1` is not a cast it has,
//! and a `[Flags]` enum over a `ulong` is a different declaration from
//! one over an `int`. None of those is visible to a substring assertion.
//!
//! ## Roslyn directly, not `dotnet build`
//!
//! The reason `tools/wireprobe/wireprobe-zerodds-csharp` gives:
//! `dotnet build` spends two to four seconds in MSBuild before Roslyn
//! sees a file, which is a corpus sweep measured in hours; `csc.dll` out
//! of the SDK answers in well under two seconds. The SDK and its
//! reference pack are found the same way that script finds them, off
//! `dotnet --list-sdks`, because deriving them from the path of the
//! `dotnet` on `PATH` is right for a tarball install and wrong for a
//! Homebrew one.
//!
//! `print_stderr` is allowed here on purpose: a skipped compiler check
//! and a frontend refusal both have to be *said*, and standard error is
//! the channel a passing test does not otherwise use.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;

use std::path::PathBuf;
use std::process::Command;

use zerodds_lang_cst::Language;

#[path = "support/corpus.rs"]
mod corpus_harness;
#[path = "support/generate.rs"]
mod generate_harness;
// -- the corpus ----------------------------------------------------------
/// How many corpus cases have to reach Roslyn for this gate to mean
/// anything. 418 cases render today.
const MUST_RENDER: usize = 350;

/// The names the C# column itself refuses, each with the reason.
///
/// A list and not a filter: a case on it that starts rendering fails
/// too — an exemption nothing checks is an exemption that outlives its
/// cause.
const REFUSED: &[(&str, &str)] = &[];

/// The cases whose generated C# Roslyn rejects, each with the message it
/// answers. Empty, and a case that starts failing is a finding rather
/// than an entry to add without one.
const DOES_NOT_COMPILE: &[(&str, &str)] = &[];

// -- the compiler --------------------------------------------------------

/// Where Roslyn and the reference assemblies are.
struct Roslyn {
    /// The path of `csc.dll` inside the SDK.
    csc: PathBuf,
    /// A response file naming every reference assembly, one `-r:` per
    /// line. On the command line instead, `net9.0`'s hundred and sixty
    /// assemblies are longer than some `execve` limits.
    references: PathBuf,
}

/// The newest SDK the installation carries, and its reference pack.
///
/// `ZERODDS_CSC` and `ZERODDS_NETREF` override, as they do for the
/// probe. `None` where there is no compiler — a runtime-only install has
/// none, and neither does a host without .NET.
fn roslyn() -> Option<Roslyn> {
    let listed = Command::new("dotnet").arg("--list-sdks").output().ok()?;
    if !listed.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&listed.stdout).into_owned();
    // `<version> [<directory>]`, newest last once sorted by the version
    // the line starts with.
    let mut lines: Vec<&str> = text.lines().filter(|line| line.contains('[')).collect();
    lines.sort_unstable();
    let line = lines.last()?;
    let (version, rest) = line.split_once(' ')?;
    let directory = rest.trim().trim_start_matches('[').trim_end_matches(']');
    let sdk = PathBuf::from(directory);
    let csc = match std::env::var("ZERODDS_CSC") {
        Ok(chosen) => PathBuf::from(chosen),
        Err(_) => sdk.join(version).join("Roslyn/bincore/csc.dll"),
    };
    if !csc.is_file() {
        return None;
    }
    let pack = match std::env::var("ZERODDS_NETREF") {
        Ok(chosen) => PathBuf::from(chosen),
        Err(_) => {
            let root = sdk.parent()?.join("packs/Microsoft.NETCore.App.Ref");
            let mut versions: Vec<PathBuf> = std::fs::read_dir(&root)
                .ok()?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect();
            versions.sort();
            let newest = versions.last()?.join("ref");
            let mut frameworks: Vec<PathBuf> = std::fs::read_dir(&newest)
                .ok()?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect();
            frameworks.sort();
            frameworks.last()?.clone()
        }
    };
    let mut response = String::new();
    let mut assemblies: Vec<PathBuf> = std::fs::read_dir(&pack)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "dll"))
        .collect();
    assemblies.sort();
    for assembly in assemblies {
        response.push_str(&format!("-r:{}\n", assembly.display()));
    }
    let references =
        std::env::temp_dir().join(format!("zd-csharp-refs-{}.rsp", std::process::id()));
    std::fs::write(&references, response).ok()?;
    Some(Roslyn { csc, references })
}

/// One generated compilation unit through Roslyn.
///
/// `-nostdlib` with the reference pack named in the response file: the
/// generated declarations are a library and name nothing but the base
/// class library.
fn csharp_compiles(roslyn: &Roslyn, source: &str, tag: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("zd-csharp-corpus-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let file = dir.join("ZdTypes.cs");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new("dotnet")
        .arg(&roslyn.csc)
        .args(["-nologo", "-nostdlib", "-target:library"])
        .arg(format!("-out:{}", dir.join("ZdTypes.dll").display()))
        .arg(format!("@{}", roslyn.references.display()))
        .arg(&file)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn the_compiler_check_catches_a_deliberate_error() {
    // Without this, a green result below could come from a compiler that
    // accepts anything — or from a harness that never ran one.
    let Some(roslyn) = roslyn() else {
        eprintln!("SKIPPED: no .NET SDK with a `csc.dll` was found.");
        return;
    };
    let bad = "public sealed class S { public NoSuchTypeExists X; }\n";
    let why = csharp_compiles(&roslyn, bad, "deliberate").expect_err("Roslyn accepted a bad unit");
    assert!(why.contains("NoSuchTypeExists"), "{why}");
    // And a good one passes, so the check is not simply always red.
    csharp_compiles(
        &roslyn,
        "public sealed class S { public int X; }\n",
        "control",
    )
    .expect("Roslyn rejected a good unit");
}

/// The whole corpus through Roslyn, one invocation per case.
///
/// `#[ignore]`, because it is one compiler start per corpus case and
/// each costs about one and a half seconds — some ten minutes for the
/// corpus as it stands. It is what proves the column rather than samples
/// it, so it is run with `--include-ignored` and its result is what the
/// `idl-csharp-corpus` job records.
#[test]
#[ignore = "one Roslyn start per corpus case, ~10 minutes; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    let Some(roslyn) = roslyn() else {
        eprintln!("SKIPPED: no .NET SDK with a `csc.dll` was found.");
        return;
    };
    let mut failures = Vec::new();
    let mut unexpectedly_fine = Vec::new();
    let mut frontend = Vec::new();
    let mut checked = 0_usize;
    for (name, path) in corpus_harness::corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let refused = REFUSED.iter().find(|(case, _)| *case == name);
        let text = match (
            generate_harness::generate_declarations(
                &source,
                &path.display().to_string(),
                Language::CSharp,
            ),
            refused,
        ) {
            (Ok(text), None) => text,
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
                continue;
            }
            (Err(why), None) => {
                // The stage that refused is what decides whose failure it
                // is. `parse` and `lower` are the frontend's and are the
                // same for every column.
                if why.starts_with("parse:") || why.starts_with("lower:") {
                    frontend.push(format!("{name}: {why}"));
                } else {
                    failures.push(format!("{name}: {why}"));
                }
                continue;
            }
            (Err(_), Some(_)) => continue,
        };
        checked += 1;
        let known = DOES_NOT_COMPILE.iter().find(|(case, _)| *case == name);
        match (csharp_compiles(&roslyn, &text, &name), known) {
            (Ok(()), None) | (Err(_), Some(_)) => {}
            (Ok(()), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: compiles now, but is listed as {why}"));
            }
            (Err(why), None) => failures.push(format!("{name}: {why}")),
        }
    }
    if !frontend.is_empty() {
        eprintln!(
            "{} corpus case(s) the frontend refuses, so C# never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} did not compile:\n{failures:#?}",
        failures.len()
    );
    assert!(unexpectedly_fine.is_empty(), "{unexpectedly_fine:#?}");
    assert!(
        checked >= MUST_RENDER,
        "only {checked} corpus cases reached Roslyn; the gate says nothing \
         below {MUST_RENDER}"
    );
}
