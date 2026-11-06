// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C# reliable-stream endpoint E2E: the `endpoints/csharp` app is the reliable
//! SENDER (submit -> WRITE_DATA -> on-ACKNACK retransmit) via the
//! async-decoupled writer (`AsyncReliableWriter`, a bounded
//! `System.Threading.Channels` ring + a dedicated drain `Thread`); the shared
//! Rust reliable peer injects loss and drives recovery. Its samples are the
//! `Ping` of the codec the C# generator writes from the IDL, so the run
//! checks IDL, generated codec and runtime together. `ExampleReliable` runs
//! its in-process example and the producer-latency comparison. The unit and
//! byte-golden suite runs in the endpoints-gate column of C#. Gated on `dotnet`
//! (>= .NET 8, on PATH or `~/.dotnet/dotnet`).

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

use zerodds_dotnet_build_lock::dotnet_build_guard;
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
/// shell: telemetry off, no first-run experience, and an explicit HOME /
/// DOTNET_CLI_HOME (some sandboxes start `dotnet` with an unset or
/// unwritable HOME, which breaks NuGet's fallback folder resolution).
fn dotnet_envs(cmd: &mut Command) {
    cmd.env("DOTNET_NOLOGO", "1");
    cmd.env("DOTNET_CLI_TELEMETRY_OPTOUT", "1");
    cmd.env("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1");
    if let Some(home) = std::env::var_os("HOME") {
        cmd.env("HOME", &home);
        cmd.env("DOTNET_CLI_HOME", &home);
    }
}

/// Absolute path to `endpoints/csharp`.
fn endpoints_csharp() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../endpoints/csharp")
        .canonicalize()
        .expect("endpoints/csharp must exist")
}

fn work_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("pp_cs_rel_{tag}_{}", std::process::id()))
}

fn csproj(asm: &str) -> String {
    format!(
        r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>disable</ImplicitUsings>
    <!-- Run on a newer major runtime when 8.0 is not the one installed. A host
         carrying only .NET 9 refuses a net8.0 app outright otherwise ("You must
         install or update .NET to run this application"), which is a missing
         runtime version and not a failing endpoint. `csharp.rs` carries the
         same line. -->
    <RollForward>LatestMajor</RollForward>
    <AssemblyName>{asm}</AssemblyName>
    <NoWarn>CS0168;CS8632;CS0219</NoWarn>
  </PropertyGroup>
</Project>
"#
    )
}

/// Copies `files` (relative to `endpoints/csharp`) into `dir`, writes a minimal
/// `.csproj` producing `<asm>.dll`, builds Release, and returns the dll path.
/// Panics (fails the test, with the build log) on a build failure.
fn build(dir: &Path, dotnet: &str, files: &[&str], asm: &str) -> PathBuf {
    zerodds_endpoint_e2e::build_dotnet_assembly(
        dir,
        dotnet,
        &endpoints_csharp(),
        files,
        asm,
        ("build.csproj", csproj(asm)),
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
                &["Zerodds.cs", "Reliable.cs", "ExampleReliable.cs"],
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

/// The generated C# split as `tests/csharp.rs` splits it: the declarations,
/// and the members of `ZdCodec` (private static) for the app class.
fn split_generated() -> (String, String) {
    let text = source(Language::CSharp);
    let at = text
        .find("public static class ZdCodec")
        .expect("generated C# declares `public static class ZdCodec`");
    let types = text[..at].trim_end().to_string();
    let body = &text[at..];
    let open = body.find('{').expect("class opens a brace");
    let members = body[open + 1..]
        .trim_end()
        .strip_suffix('}')
        .expect("class closes with `}`")
        .to_string();
    (types, members)
}

/// The reliable app: the codec `codegen::source(Language::CSharp)` writes from
/// the IDL (calling `runtime/csharp`, `ZeroDds.Wire`) encodes
/// `Ping { Seq = i, Msg = "reliable" }`, and the SDK's async-decoupled
/// `AsyncReliableWriter` (`endpoints/csharp/Reliable.cs`) sends it.
const RELIABLE_PROGRAM: &str = r#"using System;
using System.Net;
using System.Net.Sockets;
using System.Threading;

public static class Program
{
__CODEC__

    private static byte[] EncodePing(int i)
    {
        var ping = new Ping { Seq = i, Msg = "reliable" };
        var zdFraming = new global::ZeroDds.Wire.PlCdr1Framing(
            global::ZeroDds.Wire.PidFlagPolicy.SentinelMustUnderstand, 16128,
            true, true, true, true);
        var zdW = new global::ZeroDds.Wire.Writer(
            global::ZeroDds.Wire.Endianness.Little, 4, zdFraming, false);
        zd_enc_Ping(zdW, ping);
        return zdW.Finish();
    }

    public static int Main(string[] args)
    {
        int port = int.Parse(args[0]);
        int n = int.Parse(args[1]);
        var sock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
        sock.Connect(new IPEndPoint(IPAddress.Loopback, port));
        var writer = global::ZeroDDS.Endpoint.AsyncReliableWriter.Start(sock, 0x0015, 64);
        for (int i = 0; i < n; i++)
        {
            var item = EncodePing(i);
            while (!writer.Handle.Enqueue(item)) Thread.Yield();
        }
        writer.Shutdown();
        Console.WriteLine($"SENT {n}");
        return 0;
    }
}
"#;

/// The project of the reliable app: an exe referencing the wire runtime the
/// generated codec calls.
fn reliable_csproj() -> String {
    format!(
        r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>disable</ImplicitUsings>
    <RollForward>LatestMajor</RollForward>
    <AssemblyName>reliable_app</AssemblyName>
    <NoWarn>CS0168;CS8019;CS8632;CS0219</NoWarn>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="{}" />
  </ItemGroup>
</Project>
"#,
        runtime_dir("csharp").join("ZeroDds.Wire.csproj").display()
    )
}

/// Lays out the generated declarations, the SDK sources and the app and
/// builds them, once per test binary.
fn reliable_app_dll(dotnet: &str) -> PathBuf {
    static DLL: OnceLock<PathBuf> = OnceLock::new();
    DLL.get_or_init(|| {
        let dir = work_dir("reliable_app");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let (types, codec) = split_generated();
        std::fs::write(dir.join("app.csproj"), reliable_csproj()).expect("csproj");
        std::fs::write(dir.join("Generated.cs"), &types).expect("generated");
        for f in ["Zerodds.cs", "Reliable.cs"] {
            std::fs::copy(endpoints_csharp().join(f), dir.join(f))
                .unwrap_or_else(|e| panic!("copy {f}: {e}"));
        }
        std::fs::write(
            dir.join("Program.cs"),
            RELIABLE_PROGRAM.replace("__CODEC__", &codec),
        )
        .expect("program");
        let out = dir.join("out");
        let guard = dotnet_build_guard();
        let mut c = Command::new(dotnet);
        c.args(["build", "-c", "Release", "--nologo", "-v", "quiet", "-o"])
            .arg(&out)
            .current_dir(&dir);
        dotnet_envs(&mut c);
        let build = c.output().expect("dotnet build spawn");
        drop(guard);
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

/// reliable-endpoint 1.0 §5 point 3: the C# app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the C# toolchain; runs in integration-bindings-dotnet"]
fn reliable_5_3_csharp_samples_arrive_in_order_despite_dropped_datagrams() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP csharp reliable loss recovery: `dotnet` not found");
        return;
    };
    run_case(&dotnet, Some(3), "csharp/loss");
}

/// reliable-endpoint 1.0 §5 point 5: the C# reliable example runs and
/// reports its sequence without gaps; the endpoints-gate column of C#
/// builds it without running it.
#[test]
#[ignore = "needs the C# toolchain; runs in integration-bindings-dotnet"]
fn reliable_5_5_csharp_example_recovers_its_sequence_without_gaps() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP csharp reliable standalone example: `dotnet` not found");
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
        "csharp standalone example FAILED:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr),
    );
}

// ---- producer latency: decoupled Enqueue vs inline Socket.Send ----

/// reliable-endpoint 1.0 §5 point 4 and §2: the C# producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the C# toolchain; runs in integration-bindings-dotnet"]
fn reliable_5_4_csharp_the_producer_enqueue_costs_less_than_the_inline_send() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP csharp reliable latency bench: `dotnet` not found");
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
        "csharp bench produced no figure:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr),
    );
    eprintln!("csharp bench: {}", out.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("csharp", &out);
}
