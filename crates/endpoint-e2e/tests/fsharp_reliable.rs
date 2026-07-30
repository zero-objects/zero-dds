// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! F# reliable-stream endpoint E2E: the `endpoints/fsharp` app is the reliable
//! SENDER (submit -> WRITE_DATA -> on-ACKNACK retransmit) via the
//! async-decoupled writer (`AsyncReliableWriter`, a bounded
//! `System.Threading.Channels` ring + a dedicated drain `Thread`); the shared
//! Rust reliable peer injects loss and drives recovery. Also runs the F# unit +
//! byte-golden suite and the producer-latency bench. Gated on `dotnet` (>= .NET
//! 8, on PATH or `~/.dotnet/dotnet`).
//!
//! Mirror of `csharp_reliable.rs`, the .NET sibling: the reliable state machine,
//! the WRITE_DATA/HEARTBEAT/ACKNACK wire codec, and the drain-thread writer are
//! byte-identical to `endpoints/csharp` and `endpoints/c`; only the F# source
//! files + `.fsproj` (ordered `<Compile>` items) differ. The F# reliable module
//! is `ZeroDDSReliable` (single-identifier — zerodds.fs already owns the top-level
//! `module ZeroDDS`, and F# forbids a name being both module and namespace).

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

use zerodds_cdr::{BufferReader, Endianness};
use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

const N: usize = 12;

/// The `dotnet` binary: on PATH, else the codepit `~/.dotnet/dotnet` install.
fn dotnet_bin() -> Option<String> {
    let on_path = Command::new("dotnet")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if on_path {
        return Some("dotnet".to_string());
    }
    let home = std::env::var_os("HOME")?;
    let p = Path::new(&home).join(".dotnet/dotnet");
    if p.exists() {
        return Some(p.to_string_lossy().into_owned());
    }
    None
}

/// Sets the env vars `dotnet` needs to build/run reliably in a sandboxed CI
/// shell: telemetry off, no first-run experience, an explicit HOME /
/// DOTNET_CLI_HOME (some sandboxes start `dotnet` with an unset or unwritable
/// HOME, which breaks NuGet's fallback folder resolution), and
/// `DOTNET_ROLL_FORWARD=Major` so a `net8.0` target still runs on a host with
/// only the `net9.0` runtime (e.g. a Homebrew `dotnet`).
fn dotnet_envs(cmd: &mut Command) {
    cmd.env("DOTNET_NOLOGO", "1");
    cmd.env("DOTNET_CLI_TELEMETRY_OPTOUT", "1");
    cmd.env("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1");
    cmd.env("DOTNET_ROLL_FORWARD", "Major");
    if let Some(home) = std::env::var_os("HOME") {
        cmd.env("HOME", &home);
        cmd.env("DOTNET_CLI_HOME", &home);
    }
}

/// Absolute path to `endpoints/fsharp`.
fn endpoints_fsharp() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../endpoints/fsharp")
        .canonicalize()
        .expect("endpoints/fsharp must exist")
}

fn work_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("pp_fs_rel_{tag}_{}", std::process::id()))
}

/// A minimal F# `.fsproj` producing `<asm>.dll`. F# compiles source files in the
/// order the `<Compile>` items appear, so `files` MUST be in dependency order.
fn fsproj(files: &[&str], asm: &str) -> String {
    let compiles: String = files
        .iter()
        .map(|f| format!("    <Compile Include=\"{f}\" />\n"))
        .collect();
    format!(
        r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <AssemblyName>{asm}</AssemblyName>
    <Nullable>enable</Nullable>
    <WarningLevel>0</WarningLevel>
    <TreatWarningsAsErrors>false</TreatWarningsAsErrors>
  </PropertyGroup>
  <ItemGroup>
{compiles}  </ItemGroup>
</Project>
"#
    )
}

/// Copies `files` (relative to `endpoints/fsharp`) into `dir`, writes a minimal
/// `.fsproj` producing `<asm>.dll` (Compile items in the given order), builds
/// Release, and returns the dll path. Panics (fails the test, with the build log)
/// on a build failure.
fn build(dir: &Path, dotnet: &str, files: &[&str], asm: &str) -> PathBuf {
    let src = endpoints_fsharp();
    std::fs::create_dir_all(dir).expect("mkdir");
    for f in files {
        std::fs::copy(src.join(f), dir.join(f)).unwrap_or_else(|e| panic!("copy {f}: {e}"));
    }
    std::fs::write(dir.join("build.fsproj"), fsproj(files, asm)).expect("write fsproj");

    let out = dir.join("out");
    let mut c = Command::new(dotnet);
    c.args(["build", "-c", "Release", "--nologo", "-v", "quiet", "-o"])
        .arg(&out)
        .current_dir(dir);
    dotnet_envs(&mut c);
    let build = c.output().expect("dotnet build spawn");
    assert!(
        build.status.success(),
        "dotnet build FAILED ({files:?}):\n--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr),
    );
    out.join(format!("{asm}.dll"))
}

/// Lazily builds (and caches for the process's lifetime) the `ExampleReliable`
/// app -- used by every loss/baseline/bench/standalone test, so a single
/// `dotnet build` serves all of them (tests run `--test-threads=1`).
fn app_dll(dotnet: &str) -> PathBuf {
    static APP_DLL: OnceLock<PathBuf> = OnceLock::new();
    APP_DLL
        .get_or_init(|| {
            let dir = work_dir("app");
            build(
                &dir,
                dotnet,
                &["zerodds.fs", "reliable.fs", "ExampleReliable.fs"],
                "app",
            )
        })
        .clone()
}

/// Lazily builds (and caches) the `ReliableTests` unit/byte-golden runner.
fn test_dll(dotnet: &str) -> PathBuf {
    static TEST_DLL: OnceLock<PathBuf> = OnceLock::new();
    TEST_DLL
        .get_or_init(|| {
            let dir = work_dir("unit");
            build(
                &dir,
                dotnet,
                &["reliable.fs", "ReliableTests.fs"],
                "reltest",
            )
        })
        .clone()
}

fn spawn(dotnet: &str, dll: &Path, args: &[String]) -> Child {
    let mut c = Command::new(dotnet);
    c.arg(dll);
    for a in args {
        c.arg(a);
    }
    dotnet_envs(&mut c);
    c.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dotnet app")
}

// ---- loss recovery + lossless baseline (the E2E sender) ----

fn run_case(dotnet: &str, drop_every: Option<usize>, label: &str) {
    let dll = app_dll(dotnet);
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = spawn(dotnet, &dll, &[peer.port.to_string()]);

    let delivered = reliable_receive(&peer, child, label, N);
    assert_eq!(delivered.len(), N, "{label}: delivered count");
    // Each sample is a Reading { id, value, label }; the id must equal its index
    // (gap-free, in-order delivery despite injected loss).
    for (i, payload) in delivered.iter().enumerate() {
        let mut r = BufferReader::new(payload, Endianness::Little).xcdr2();
        let id = r.read_u32().expect("reading id");
        assert_eq!(id as usize, i, "{label}: sample {i} id (gap-free order)");
    }
}

#[test]
fn fsharp_reliable_loss_recovery() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP fsharp_reliable_loss_recovery: `dotnet` not found");
        return;
    };
    run_case(&dotnet, Some(3), "fsharp/loss");
}

#[test]
fn fsharp_reliable_no_loss_baseline() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP fsharp_reliable_no_loss_baseline: `dotnet` not found");
        return;
    };
    run_case(&dotnet, None, "fsharp/baseline");
}

// ---- unit suite + byte-golden ----

#[test]
fn fsharp_reliable_unit_and_golden() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP fsharp_reliable_unit_and_golden: `dotnet` not found");
        return;
    };
    let dll = test_dll(&dotnet);

    // Best-effort: generate the Rust goldens and pass them for a byte-identical
    // check; the F# suite also asserts the hardcoded golden bytes unconditionally.
    let gold = work_dir("gold");
    std::fs::create_dir_all(&gold).expect("mkdir gold");
    let gen_res = Command::new("cargo")
        .args(["run", "-q", "-p", "zerodds-endpoint-golden", "--"])
        .arg(&gold)
        .output();
    let hb = gold.join("golden_heartbeat_le.bin");
    let ak = gold.join("golden_acknack_le.bin");

    let mut c = Command::new(&dotnet);
    c.arg(&dll);
    if gen_res.map(|o| o.status.success()).unwrap_or(false) && hb.exists() && ak.exists() {
        c.arg(&hb).arg(&ak);
    }
    dotnet_envs(&mut c);
    let o = c.output().expect("run ReliableTests");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        o.status.success() && out.contains("ALL OK"),
        "fsharp unit/golden FAILED:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr),
    );
}

// ---- standalone in-process demo (all modes runnable) ----

#[test]
fn fsharp_reliable_standalone_example() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP fsharp_reliable_standalone_example: `dotnet` not found");
        return;
    };
    let dll = app_dll(&dotnet);
    let mut c = Command::new(&dotnet);
    c.arg(&dll);
    dotnet_envs(&mut c);
    let o = c.output().expect("run standalone example");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("OK:"),
        "fsharp standalone example FAILED:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr),
    );
}

// ---- producer latency: decoupled Enqueue vs inline Socket.Send ----

#[test]
fn fsharp_reliable_latency_bench() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP fsharp_reliable_latency_bench: `dotnet` not found");
        return;
    };
    let dll = app_dll(&dotnet);
    let mut c = Command::new(&dotnet);
    c.arg(&dll).arg("bench");
    dotnet_envs(&mut c);
    let o = c.output().expect("run bench");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("LATENCY"),
        "fsharp bench produced no figure:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr),
    );
    eprintln!("fsharp bench: {}", out.trim());
}
