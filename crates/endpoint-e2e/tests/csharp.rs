// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C# endpoint ping-pong: raw generated codegen over UDP, plus the full stack
//! (generated `TypeSupport` + endpoint SDK, sync `Client.Poll` and async
//! `AsyncReader.Stream`) with XRCE framing. Gated on `dotnet` (>= .NET 8).
//!
//! The app is a real .NET program: it builds a `Ping` record, encodes it with
//! the GENERATED `PingTypeSupport` (the same XCDR2 codec the roundtrip tests
//! exercise), ships it through the pure-C# endpoint SDK
//! (`endpoints/csharp/Zerodds.cs`) over a live UDP socket, and decodes the
//! `Pong` the Rust peer sends back with the GENERATED `PongTypeSupport`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_dotnet_build_lock::dotnet_build_guard;
use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_csharp::{CsGenOptions, generate_csharp};

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

/// Absolute path to the real `ZeroDDS.Cdr.csproj` (the runtime the generated
/// TypeSupport codec references).
fn zerodds_cdr_csproj() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cs/csharp/ZeroDDS.Cdr/ZeroDDS.Cdr.csproj")
        .canonicalize()
        .expect("ZeroDDS.Cdr.csproj must exist");
    p.to_string_lossy().into_owned()
}

/// The pure-C# endpoint SDK source (`endpoints/csharp/Zerodds.cs`).
fn endpoint_sdk_source() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/csharp/Zerodds.cs");
    std::fs::read_to_string(&p).expect("endpoints/csharp/Zerodds.cs must exist")
}

// The generated records reference `Omg.Types` (ITopicType, the [Key]/[Id]/…/
// [Extensibility] attributes, ExtensibilityKind, the sequence containers). Those
// types are compiled into `ZeroDDS.Cdr` (its .csproj `<Compile Include>`s
// `idl-csharp/runtime/Omg.Types.cs`), and the app references `ZeroDDS.Cdr` — so
// it gets the canonical Omg.Types from there. No local stub: a duplicate would
// collide (CS0436) with `ZeroDDS.Cdr`'s Omg.Types.

/// The `.csproj`: an exe on net8.0 referencing the real `ZeroDDS.Cdr` runtime.
fn csproj() -> String {
    format!(
        r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>disable</ImplicitUsings>
    <RollForward>LatestMajor</RollForward>
    <AssemblyName>app</AssemblyName>
    <NoWarn>CS0168;CS8019;CS8632;CS0219</NoWarn>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="{}" />
  </ItemGroup>
</Project>
"#,
        zerodds_cdr_csproj()
    )
}

/// The app. `argv`: `<mode> <port>` — mode is `sync` | `async` | `raw`.
/// `raw` sends the bare XCDR2 sample (no XRCE frame) and reads a bare reply,
/// exercising the generated codec independently of the SDK's framing.
const PROGRAM: &str = r#"using System;
using System.Net;
using System.Net.Sockets;
using System.Threading;
using System.Threading.Tasks;

// A live-UDP transport for the endpoint SDK: Deliver = send, Receive = a
// non-blocking recv (null when nothing is queued), which is what the SDK's
// sync poll loop and async stream both expect.
public sealed class UdpTransport : ZeroDDS.ITransport
{
    private readonly Socket _sock;
    public UdpTransport(Socket sock) => _sock = sock;
    public void Deliver(byte[] frame) { _sock.Send(frame); }
    public byte[]? Receive()
    {
        if (_sock.Available == 0) return null;
        var buf = new byte[65536];
        int n = _sock.Receive(buf);
        var body = new byte[n];
        Array.Copy(buf, body, n);
        return body;
    }
}

public static class Program
{
    public static async Task<int> Main(string[] args)
    {
        string mode = args[0];
        int port = int.Parse(args[1]);

        var sock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
        sock.Connect(new IPEndPoint(IPAddress.Loopback, port));

        // The typed Ping, encoded by the GENERATED TypeSupport (real XCDR2).
        var ping = new Ping { Seq = 1, Msg = "hello from app" };
        byte[] sample = PingTypeSupport.Instance.Encode(ping);

        byte[]? body = null;

        if (mode == "raw")
        {
            // Generated codec only: bare XCDR2 sample, bare reply — no SDK/XRCE.
            sock.Send(sample);
            var buf = new byte[65536];
            sock.ReceiveTimeout = 15000;
            int n = sock.Receive(buf);
            body = new byte[n];
            Array.Copy(buf, body, n);
        }
        else
        {
            var transport = new UdpTransport(sock);
            var client = new ZeroDDS.Client(transport);
            client.Write(sample); // SDK frames it (XRCE WRITE_DATA) and delivers.

            if (mode == "async")
            {
                using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(15));
                var reader = new ZeroDDS.AsyncReader(transport);
                try
                {
                    await foreach (var b in reader.Stream(cts.Token))
                    {
                        body = b;
                        break;
                    }
                }
                catch (OperationCanceledException) { }
            }
            else // sync
            {
                var deadline = DateTime.UtcNow.AddSeconds(15);
                while (DateTime.UtcNow < deadline)
                {
                    body = client.Poll();
                    if (body != null) break;
                    Thread.Sleep(5);
                }
            }
        }

        if (body == null)
        {
            Console.Error.WriteLine($"{mode}: no pong");
            return 1;
        }

        var pong = PongTypeSupport.Instance.Decode(body);
        Console.WriteLine($"PONG seq={pong.Seq} reply={pong.Reply}");
        return 0;
    }
}
"#;

/// Lays out the project + generated + SDK + app sources in `dir` and builds it,
/// returning the path to the produced `app.dll`. Panics (fails the test, with
/// the build log) on a build failure.
fn build_app(dotnet: &str, dir: &Path) -> PathBuf {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse IDL");
    let generated = generate_csharp(&ast, &CsGenOptions::default()).expect("generate C#");

    std::fs::create_dir_all(dir).expect("mkdir");
    std::fs::write(dir.join("app.csproj"), csproj()).expect("csproj");
    std::fs::write(dir.join("Generated.cs"), &generated).expect("generated");
    std::fs::write(dir.join("Zerodds.cs"), endpoint_sdk_source()).expect("sdk");
    std::fs::write(dir.join("Program.cs"), PROGRAM).expect("program");

    let out = dir.join("out");
    // Build up front (off the peer's clock) so the app starts — and sends its
    // Ping — promptly once spawned; the first restore/build can exceed the
    // peer's recv timeout otherwise.
    //
    // Serialize the build: every csharp/conformance test references the SAME
    // ZeroDDS.Cdr project, so concurrent `dotnet build`s race on its obj/ output
    // (CS2012) — across test binaries and processes, not just this binary's
    // threads. A cross-process advisory lock (held only around the build)
    // removes the race; the built apps still run concurrently.
    let guard = dotnet_build_guard();
    let build = Command::new(dotnet)
        .args(["build", "-c", "Release", "--nologo", "-v", "quiet", "-o"])
        .arg(&out)
        .current_dir(dir)
        .env("DOTNET_NOLOGO", "1")
        .env("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1")
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
        .output()
        .expect("dotnet build spawn");
    drop(guard);
    assert!(
        build.status.success(),
        "dotnet build FAILED:\n--- stdout ---\n{}\n--- stderr ---\n{}\n--- generated ---\n{generated}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr),
    );
    out.join("app.dll")
}

/// Spawns the built app in `mode`, aimed at `port`.
fn spawn_app(dotnet: &str, dll: &Path, mode: &str, port: u16) -> Child {
    Command::new(dotnet)
        .arg(dll)
        .arg(mode)
        .arg(port.to_string())
        .env("DOTNET_NOLOGO", "1")
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn app")
}

fn work_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("pp_cs_{tag}_{}", std::process::id()))
}

// ---- full stack: generated types + endpoint SDK (sync/async) over XRCE ----

fn endpoint_case(mode: &str) {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP csharp endpoint {mode}: `dotnet` not found");
        return;
    };
    let dir = work_dir(mode);
    let dll = build_app(&dotnet, &dir);
    let peer: Peer = bind_peer().expect("bind peer");
    let child = spawn_app(&dotnet, &dll, mode, peer.port);
    ping_pong(&peer, child, &format!("csharp/{mode}"));
}

#[test]
fn csharp_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn csharp_endpoint_async() {
    endpoint_case("async");
}

// ---- raw generated codec over a plain UDP socket (no SDK, no XRCE) ----

#[test]
fn csharp_raw_udp() {
    let Some(dotnet) = dotnet_bin() else {
        eprintln!("SKIP csharp_raw_udp: `dotnet` not found");
        return;
    };
    use std::net::UdpSocket;
    use std::time::Duration;
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

    let dir = work_dir("raw");
    let dll = build_app(&dotnet, &dir);

    // A minimal inline peer: the raw app sends a bare XCDR2 Ping (no XRCE frame).
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = spawn_app(&dotnet, &dll, "raw", port);
    sock.set_read_timeout(Some(Duration::from_secs(30)))
        .expect("timeout");

    let mut buf = [0u8; 4096];
    let (n, app) = sock.recv_from(&mut buf).expect("recv ping");
    let mut r = BufferReader::new(&buf[..n], Endianness::Little).xcdr2();
    let seq = r.read_u32().expect("seq");
    let msg = r.read_string().expect("msg");
    assert_eq!((seq, msg.as_str()), (1, "hello from app"));

    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    w.write_u32(seq).expect("s");
    w.write_string(&format!("pong:{msg}")).expect("r");
    sock.send_to(&w.into_bytes(), app).expect("send pong");

    let out = child.wait_with_output().expect("wait");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("PONG seq=1 reply=pong:hello from app"),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
