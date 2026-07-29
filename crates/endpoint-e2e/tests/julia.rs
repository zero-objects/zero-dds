// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Julia endpoint ping-pong: the generated Ping/Pong types (marshal + decode)
//! driven through the native Julia endpoint SDK — sync `Client` poll and async
//! `AsyncReader` (Task + Channel) — over XRCE framing on a real UDP socket.
//! Gated on `julia`.
//!
//! Name-clash handling: the generated code and `endpoints/julia/zerodds.jl`
//! each define their own wire core (`Endian`, `Writer`, `LE`, ...). The SDK is
//! already a `module ZeroDDS`; we wrap the generated code in `module Gen`, so
//! the app refers to `Gen.Ping`/`Gen.marshal_xcdr`/`Gen.LE` and `ZeroDDS.*`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::Path;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_julia::{JuliaGenOptions, generate_julia_module};

fn julia_available() -> bool {
    Command::new("julia").arg("--version").output().is_ok()
}

/// The generated Ping/Pong wire module (struct + marshal + decode).
fn gen_module() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    generate_julia_module(&ast, &JuliaGenOptions::default()).expect("gen")
}

// The app: generated types marshalled/decoded, transferred via the endpoint SDK
// over a real UDP socket. A background Task does the blocking `recvfrom` and
// feeds a Channel; the SDK's `Transport.receive` pops it non-blocking (nothing
// when empty) — the shape the sync poll loop and async reader both expect.
//
// libuv drops datagrams that arrive before `recvfrom` has armed the socket, so
// the receive Task must be running its `recvfrom` before the first `send` — we
// gate the ping behind an `armed` flag (a bare `@async` + `send` race loses).
const JULIA_APP: &str = r#"
using Sockets

function main()
    mode = ARGS[1]
    peerport = parse(Int, ARGS[2])
    sock = UDPSocket()
    bind(sock, ip"127.0.0.1", 0)
    peeraddr = ip"127.0.0.1"

    rxq = Channel{Vector{UInt8}}(32)
    armed = Ref(false)
    errormonitor(@async begin
        while true
            armed[] = true
            _, data = recvfrom(sock)
            put!(rxq, Vector{UInt8}(data))
        end
    end)
    while !armed[]
        sleep(0.005)
    end
    sleep(0.1)

    deliver = frame -> (send(sock, peeraddr, peerport, frame); nothing)
    receive = () -> isready(rxq) ? take!(rxq) : nothing
    tr = ZeroDDS.Transport(deliver, receive)

    sample = Gen.marshal_xcdr(Gen.Ping(Int32(1), "hello from app"), Gen.LE)

    body = nothing
    if mode == "async"
        reader = ZeroDDS.start_reader(tr)
        ZeroDDS.write!(ZeroDDS.Client(tr), sample)
        deadline = time() + 10.0
        while time() < deadline
            if isready(reader.ch)
                body = take!(reader.ch)
                break
            end
            sleep(0.005)
        end
        ZeroDDS.stop!(reader)
        body === nothing && error("async: no pong")
    else
        client = ZeroDDS.Client(tr)
        ZeroDDS.write!(client, sample)
        deadline = time() + 10.0
        while time() < deadline
            b = ZeroDDS.poll(client)
            if b !== nothing
                body = b
                break
            end
            sleep(0.005)
        end
        body === nothing && error("sync: no pong")
    end

    pong = Gen.unmarshal_xcdr_Pong(body, Gen.LE)
    println("PONG seq=$(pong.seq) reply=$(pong.reply)")
end

main()
"#;

fn build_julia(mode: &str, port: u16) -> Child {
    let gen_src = gen_module();
    let sdk = Path::new(&format!(
        "{}/../../endpoints/julia/zerodds.jl",
        env!("CARGO_MANIFEST_DIR")
    ))
    .canonicalize()
    .expect("endpoints/julia path");
    let dir = std::env::temp_dir().join(format!("pp_julia_{mode}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    // Wrap the generated wire core in its own module to dodge the Endian/Writer
    // clash with the SDK's ZeroDDS module.
    std::fs::write(dir.join("gen.jl"), format!("module Gen\n{gen_src}\nend\n")).expect("write gen");
    let main = format!(
        "include(\"gen.jl\")\ninclude(raw\"{}\")\n{JULIA_APP}",
        sdk.display()
    );
    std::fs::write(dir.join("main.jl"), main).expect("write main");
    Command::new("julia")
        .arg(dir.join("main.jl"))
        .arg(mode)
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn julia")
}

fn endpoint_case(mode: &str) {
    if !julia_available() {
        eprintln!("SKIP julia endpoint {mode}: `julia` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_julia(mode, peer.port);
    ping_pong(&peer, child, &format!("julia/{mode}"));
}

#[test]
fn julia_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn julia_endpoint_async() {
    endpoint_case("async");
}
