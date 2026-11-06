// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! F# reliable-stream endpoint E2E: the `endpoints/fsharp` app is the reliable
//! SENDER (submit -> WRITE_DATA -> on-ACKNACK retransmit) via the
//! async-decoupled writer (`AsyncReliableWriter`, a bounded
//! `System.Threading.Channels` ring + a dedicated drain `Thread`); the shared
//! Rust reliable peer injects loss and drives recovery. Its samples are the
//! `Ping` of the codec the F# generator writes from the IDL, so the run checks
//! IDL, generated codec and runtime together. `ExampleReliable` runs its
//! in-process example and the producer-latency comparison. The unit and
//! byte-golden suite runs in the endpoints-gate column of F#. Gated on `dotnet`
//! (>= .NET 8, on PATH or `~/.dotnet/dotnet`).
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

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

/// The `dotnet` binary: on PATH, else the test host's `~/.dotnet/dotnet` install.
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
    zerodds_endpoint_e2e::build_dotnet_assembly(
        dir,
        dotnet,
        &endpoints_fsharp(),
        files,
        asm,
        ("build.fsproj", fsproj(files, asm)),
        dotnet_envs,
    )
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

// ---- loss recovery (the E2E sender) ----

/// The reliable app: the codec `codegen::source(Language::FSharp)` writes from
/// the IDL (`Gen.fs`, module `ZdTypes`, calling `runtime/fsharp/ZdWire.fs`)
/// encodes `Ping(seq = i, msg = "reliable")`, and the SDK's async-decoupled
/// `AsyncReliableWriter` (`endpoints/fsharp/reliable.fs`) sends it.
const RELIABLE_APP_FS: &str = r#"module App

open System.Net
open System.Net.Sockets
open System.Threading
open ZeroDDSReliable

let encodePing (i: int) : byte[] =
    let p = ZdTypes.Ping()
    p.seq <- i
    p.msg <- "reliable"
    let w = ZdWire.Writer(ZdWire.LE, ZdWire.XCDR2, ZdWire.specificationFraming, false)
    ZdTypes.zd_enc_Ping w p
    w.Finish()

[<EntryPoint>]
let main argv =
    let port = int argv.[0]
    let n = int argv.[1]
    let sock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp)
    sock.Connect(IPEndPoint(IPAddress.Loopback, port))
    let writer = AsyncReliableWriter.Start(sock, 0x0015us, 64)
    for i in 0 .. n - 1 do
        let item = encodePing i
        while not (writer.Handle.Enqueue(item)) do
            Thread.Yield() |> ignore
    writer.Shutdown()
    printfn "SENT %d" n
    0
"#;

/// Lays out the wire runtime, the generated module, the SDK's reliable
/// module and the app (in F#'s compile order) and builds them, once per test
/// binary.
fn reliable_app_dll(dotnet: &str) -> PathBuf {
    static DLL: OnceLock<PathBuf> = OnceLock::new();
    DLL.get_or_init(|| {
        let dir = work_dir("reliable_app");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::copy(
            runtime_dir("fsharp").join("ZdWire.fs"),
            dir.join("ZdWire.fs"),
        )
        .expect("copy ZdWire.fs");
        // A file of a multi-file project opens with its module (FS0222).
        std::fs::write(
            dir.join("Gen.fs"),
            format!("module ZdTypes\n\n{}", source(Language::FSharp)),
        )
        .expect("write Gen.fs");
        std::fs::copy(
            endpoints_fsharp().join("reliable.fs"),
            dir.join("reliable.fs"),
        )
        .expect("copy reliable.fs");
        std::fs::write(dir.join("App.fs"), RELIABLE_APP_FS).expect("write App.fs");
        let files = ["ZdWire.fs", "Gen.fs", "reliable.fs", "App.fs"];
        std::fs::write(dir.join("build.fsproj"), fsproj(&files, "reliable_app"))
            .expect("write fsproj");
        let out = dir.join("out");
        let mut c = Command::new(dotnet);
        c.args(["build", "-c", "Release", "--nologo", "-v", "quiet", "-o"])
            .arg(&out)
            .current_dir(&dir);
        dotnet_envs(&mut c);
        let build = c.output().expect("dotnet build spawn");
        assert!(
            build.status.success(),
            "dotnet build of the reliable app FAILED:\n--- stdout ---\n{}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr),
        );
        out.join("reliable_app.dll")
    })
    .clone()
}

fn run_case(dotnet: &str, drop_every: Option<usize>, label: &str) {
    let dll = reliable_app_dll(dotnet);
    let n = RELIABLE_SAMPLES;
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = spawn(dotnet, &dll, &[peer.port.to_string(), n.to_string()]);
    let delivered = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &delivered, n);
}

/// reliable-endpoint 1.0 §5 point 3: the F# app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the F# toolchain; runs in integration-bindings-dotnet"]
fn reliable_5_3_fsharp_samples_arrive_in_order_despite_dropped_datagrams() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP fsharp reliable loss recovery: `dotnet` not found");
        return;
    };
    run_case(&dotnet, Some(3), "fsharp/loss");
}

/// reliable-endpoint 1.0 §5 point 5: the F# reliable example runs and
/// reports its sequence without gaps; the endpoints-gate column of F#
/// builds it without running it.
#[test]
#[ignore = "needs the F# toolchain; runs in integration-bindings-dotnet"]
fn reliable_5_5_fsharp_example_recovers_its_sequence_without_gaps() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP fsharp reliable standalone example: `dotnet` not found");
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

/// reliable-endpoint 1.0 §5 point 4 and §2: the F# producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the F# toolchain; runs in integration-bindings-dotnet"]
fn reliable_5_4_fsharp_the_producer_enqueue_costs_less_than_the_inline_send() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP fsharp reliable latency bench: `dotnet` not found");
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
    zerodds_endpoint_e2e::assert_enqueue_below_inline("fsharp", &out);
}
