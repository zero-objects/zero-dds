// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The cases of one corpus share of the C# gates in one .NET process.
//!
//! `csharp_corpus_compiles.rs` started `csc.dll` once per case and
//! `csharp_consumer_assembly.rs` twice, and a third .NET process for the
//! consuming assembly: on the CI runner host the 72 shares took 1002 s of test time
//! (pipeline 8967, job 229634), most of it .NET starting. `Together.cs`
//! beside this file compiles the cases of a share with the compiler of the
//! same SDK in one process, each as the gate's command line does, and runs
//! each consumer in a collectible load context of its own.
//!
//! A gate trusts the batch only when every case reports `OK`; on anything
//! else (a case that fails, a driver that does not build or start) it
//! checks every case alone through `csc.dll` as before, so that a failure
//! is named in the compiler's words.
//!
//! Included through `#[path = "support/csharp_together.rs"]` by both gates.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

/// The CPUs a driver here sizes its threads by: the codegen jobs run eight
/// tests side by side on a host that shows .NET 32 CPUs (as
/// `fsharp_end_to_end.rs`'s `DOTNET_PROCESSOR_COUNT`).
const DOTNET_PROCESSOR_COUNT: &str = "2";

/// What a gate compiles a case as: `Library` is `csharp_corpus_compiles.rs`'s
/// one assembly, `Consumer` the two assemblies and the run of
/// `csharp_consumer_assembly.rs` against the runtime assembly named.
pub enum Mode<'a> {
    /// One `ZdTypes.cs` per case into a library.
    Library,
    /// `ZdTypes.cs` and `Consumer.cs` per case, against `ZeroDds.Wire.dll`.
    Consumer(&'a Path),
}

/// `Together.dll`, compiled once for this test process beside the runtime
/// configuration of the `csc.dll` it uses; or `csc`'s message.
pub fn driver(csc: &Path, references: &Path) -> Result<PathBuf, String> {
    static DRIVER: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    DRIVER.get_or_init(|| build_driver(csc, references)).clone()
}

fn build_driver(csc: &Path, references: &Path) -> Result<PathBuf, String> {
    let roslyn = csc.parent().ok_or("csc.dll has no directory")?;
    let dir = std::env::temp_dir().join(format!("zd-csharp-together-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let out = dir.join("Together.dll");
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/csharp_together/Together.cs");
    let output = Command::new("dotnet")
        .arg(csc)
        .args([
            "-nologo",
            "-noconfig",
            "-nostdlib",
            "-target:exe",
            "-optimize+",
        ])
        .arg(format!("-out:{}", out.display()))
        .arg(format!("@{}", references.display()))
        .arg(format!(
            "-r:{}",
            roslyn.join("Microsoft.CodeAnalysis.dll").display()
        ))
        .arg(format!(
            "-r:{}",
            roslyn.join("Microsoft.CodeAnalysis.CSharp.dll").display()
        ))
        .arg(&source)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Together.cs did not compile:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    // The shared framework the compiler itself runs on.
    std::fs::copy(
        roslyn.join("csc.runtimeconfig.json"),
        dir.join("Together.runtimeconfig.json"),
    )
    .map_err(|error| error.to_string())?;
    Ok(out)
}

/// A counter, so two batches of one test process do not share a directory.
static SERIAL: AtomicU32 = AtomicU32::new(0);

/// The cases (`name`, the files of its directory) through one run of the
/// driver: `Ok` with the names of the cases that did not report `OK`, each
/// with what it said; `Err` when the driver did not run to its end.
pub fn run(
    csc: &Path,
    references: &Path,
    mode: &Mode<'_>,
    cases: &[(String, Vec<(&str, String)>)],
) -> Result<Vec<(String, String)>, String> {
    let driver = driver(csc, references)?;
    let roslyn = csc.parent().ok_or("csc.dll has no directory")?;
    let dir = std::env::temp_dir().join(format!(
        "zd-csharp-together-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let result = (|| {
        let mut manifest = String::new();
        let mut directories = Vec::new();
        for (index, (_, files)) in cases.iter().enumerate() {
            let case = dir.join(index.to_string());
            std::fs::create_dir_all(&case).map_err(|error| error.to_string())?;
            for (file, text) in files {
                std::fs::write(case.join(file), text).map_err(|error| error.to_string())?;
            }
            manifest.push_str(&format!("{}\n", case.display()));
            directories.push(case);
        }
        let list = dir.join("manifest.txt");
        std::fs::write(&list, manifest).map_err(|error| error.to_string())?;
        let mut command = Command::new("dotnet");
        command
            .env("DOTNET_PROCESSOR_COUNT", DOTNET_PROCESSOR_COUNT)
            .arg(&driver)
            .arg(roslyn);
        match mode {
            Mode::Library => {
                command.arg("library").arg(references);
            }
            Mode::Consumer(runtime) => {
                command.arg("consumer").arg(references).arg(runtime);
            }
        }
        let output = command
            .arg(&list)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "the driver answered {:?}:\n{}{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let mut failed = Vec::new();
        for ((name, _), case) in cases.iter().zip(&directories) {
            let said = std::fs::read_to_string(case.join("result.txt"))
                .map_err(|error| format!("{name}: no result: {error}"))?;
            if said != "OK" {
                failed.push((name.clone(), said));
            }
        }
        Ok(failed)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}
