// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Julia endpoint ping-pong: the generated Ping/Pong types (marshal + decode)
//! driven through the native Julia endpoint SDK — sync `Client` poll and async
//! `AsyncReader` (Task + Channel) — over XRCE framing on a real UDP socket.
//! Gated on `julia`.
//!
//! Name-clash handling: `runtime/julia/zerodds_wire.jl` and
//! `endpoints/julia/zerodds.jl` each define their own wire core (`Writer`,
//! `LE`, ...). The SDK is already a `module ZeroDDS`; the runtime is
//! `module ZeroddsWire`, and the generated codec is wrapped in `module Gen`
//! (which pulls the runtime in), so the app refers to `Gen.Ping` /
//! `Gen.zd_enc_Ping` and `ZeroDDS.*` with nothing shadowed.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use zerodds_endpoint_e2e::codegen::{
    Language, endpoints_dir, runtime_dir, scratch_dir, source, tool_available,
};
use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong_with_timeout};

fn julia_available() -> bool {
    tool_available("julia", "--version")
}

/// The generated Ping/Pong types + codec, wrapped in `module Gen` over the
/// wire runtime it calls.
fn gen_module() -> String {
    format!(
        "module Gen\ninclude(raw\"{}\")\nusing .ZeroddsWire\n\n{}\nend\n",
        runtime_dir("julia").join("zerodds_wire.jl").display(),
        source(Language::Julia)
    )
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

    zd_framing = Gen.ZeroddsWire.ZdPlCdr1Framing(
        pid_flags = "sentinel-must-understand",
        highest_short_id = 16128,
        pad_declared_length = true,
        pad_declared_optional_length = true,
        member_must_understand = true,
        pad_after_value = true,
    )
    w = Gen.ZeroddsWire.ZdWriter(Gen.ZeroddsWire.ZD_LITTLE, 4, zd_framing, false)
    Gen.zd_enc_Ping(w, Gen.Ping(Int32(1), "hello from app"))
    sample = Gen.ZeroddsWire.zd_writer_bytes(w)

    body = nothing
    if mode == "async"
        reader = ZeroDDS.start_reader(tr)
        ZeroDDS.write!(ZeroDDS.Client(tr, ZeroDDS.xrce_object_id(0x001, ZeroDDS.OBJK_DATAWRITER)), sample)
        deadline = time() + 60.0
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
        client = ZeroDDS.Client(tr, ZeroDDS.xrce_object_id(0x001, ZeroDDS.OBJK_DATAWRITER))
        ZeroDDS.write!(client, sample)
        deadline = time() + 60.0
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

    r = Gen.ZeroddsWire.ZdReader(body, Gen.ZeroddsWire.ZD_LITTLE, 4, zd_framing)
    pong = Gen.zd_dec_Pong(r, Gen.ZeroddsWire.ZD_TRY_CONSTRUCT_DISCARD)
    println("PONG seq=$(pong.seq) reply=$(pong.reply)")
end

main()
"#;

fn build_julia(mode: &str, port: u16) -> Child {
    let sdk = endpoints_dir("julia").join("zerodds.jl");
    let dir = scratch_dir(&format!("julia_{mode}"));
    std::fs::write(dir.join("gen.jl"), gen_module()).expect("write gen");
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
    // Julia's first-run JIT (the generated module + `Sockets`) on a cold CI
    // depot can take well over the 30 s default before the app sends its ping.
    // That is Julia's compile latency, not a ZeroDDS round-trip — give it room.
    ping_pong_with_timeout(
        &peer,
        child,
        &format!("julia/{mode}"),
        Duration::from_secs(120),
    );
}

#[test]
fn julia_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn julia_endpoint_async() {
    endpoint_case("async");
}
