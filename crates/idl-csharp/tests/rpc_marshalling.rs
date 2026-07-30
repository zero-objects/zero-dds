// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! REAL client↔server RPC marshalling round-trip for the C# PSM (F.1 item 11).
//!
//! Generates C# for a `@service`, compiles it together with the real
//! `Zerodds.Rpc` runtime and an in-process loopback transport, then runs it: a
//! generated `Requester` marshals each call into the `object[]` tuple, the
//! generated `Replier` dispatches it to a handler, and the reply tuple flows
//! back — covering a value return, an `inout` holder write-back, an `out`
//! holder, and a `oneway`. This is the C# counterpart of the Java PSM's
//! compile-against-real-runtime test, but it additionally EXECUTES the round
//! trip, proving the marshalling (not just that it compiles).
//!
//! **Prerequisite:** the `dotnet` CLI on `PATH`. Skipped (never falsely passes)
//! if it is missing.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::path::Path;
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_csharp::{CsGenOptions, generate_csharp};

fn dotnet_available() -> bool {
    Command::new("dotnet")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

const SERVICE_IDL: &str = "@service interface Calc { \
     long long add(in long long a, in long long b); \
     void inc(inout long v); \
     void result(out long v); \
     oneway void log(in string m); \
 };";

// In-process loopback transport + handler + entry point. References the
// generated CalcRequester/CalcReplier and the Zerodds.Rpc runtime.
const PROGRAM_CS: &str = r#"
using System.Threading.Tasks;

sealed class Loopback : Zerodds.Rpc.IRequester
{
    private readonly CalcReplier replier;
    public Loopback(CalcReplier replier) { this.replier = replier; }
    public Task<object> SendRequest(int methodId, object payload)
        => Task.FromResult<object>(replier.Dispatch(methodId, payload)!);
    public void SendOneway(int methodId, object payload)
        => replier.Dispatch(methodId, payload);
    public void Dispose() {}
}

sealed class NoopReplier : Zerodds.Rpc.IReplier
{
    public void SendReply(object reply) {}
    public void Dispose() {}
}

sealed class Handler : CalcService
{
    public string? LastLog;
    public long Add(long a, long b) => a + b;
    public void Inc(Zerodds.Rpc.Holder<int> v) { v.Value += 1; }
    public void Result(Zerodds.Rpc.Holder<int> v) { v.Value = 42; }
    public void Log(string m) { LastLog = m; }
}

static class Program
{
    static void Main()
    {
        var handler = new Handler();
        var replier = new CalcReplier(new NoopReplier(), handler);
        var client = new CalcRequester(new Loopback(replier));

        long sum = client.Add(2, 3);

        var inout = new Zerodds.Rpc.Holder<int>(10);
        client.Inc(inout);

        var outp = new Zerodds.Rpc.Holder<int>();
        client.Result(outp);

        client.Log("hi");

        if (sum == 5 && inout.Value == 11 && outp.Value == 42 && handler.LastLog == "hi")
            System.Console.WriteLine("MARSHAL_OK");
        else
            System.Console.WriteLine($"MARSHAL_FAIL sum={sum} inout={inout.Value} out={outp.Value} log={handler.LastLog}");
    }
}
"#;

#[test]
fn rpc_client_server_marshalling_round_trip() {
    if !dotnet_available() {
        eprintln!("WARNING: skipping C# RPC marshalling round-trip — no dotnet in PATH");
        return;
    }

    let ast = zerodds_idl::parse(SERVICE_IDL, &ParserConfig::full_4_2()).expect("parse");
    let generated = generate_csharp(&ast, &CsGenOptions::default()).expect("gen");

    // The real runtime shipped alongside the codegen.
    let runtime =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/rpc/Rpc.cs"))
            .expect("read Zerodds.Rpc runtime");

    let tmp = tempfile::tempdir().expect("tmpdir");
    let root = tmp.path();

    let csproj = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <RollForward>LatestMajor</RollForward>
    <Nullable>enable</Nullable>
    <TreatWarningsAsErrors>true</TreatWarningsAsErrors>
    <NoWarn>CS8019</NoWarn>
  </PropertyGroup>
</Project>
"#;
    std::fs::write(root.join("Rpc.csproj"), csproj).unwrap();
    std::fs::write(root.join("Generated.cs"), &generated).unwrap();
    std::fs::write(root.join("Rpc.cs"), &runtime).unwrap();
    std::fs::write(root.join("Program.cs"), PROGRAM_CS).unwrap();

    let output = Command::new("dotnet")
        .args(["run", "--nologo", "--verbosity", "quiet"])
        .current_dir(root)
        .output()
        .expect("spawn dotnet run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("MARSHAL_OK"),
        "C# RPC round-trip failed.\n--- generated ---\n{generated}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
}
