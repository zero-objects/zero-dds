// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Julia reliable endpoint, live: the pure-Julia reliable state machine + frame
//! codec (`endpoints/julia/reliable.jl`) driven by a decoupled Channel+drain-Task
//! sender (`drain` of `reliable_app.jl`) against the shared Rust reliable
//! peer: the peer drops, the app retransmits on ACKNACK, all samples arrive
//! gap-free. The samples are the `Ping` of the codec the Julia generator
//! writes from the IDL, so the run checks IDL, generated codec and runtime
//! together. And the producer-latency comparison. The unit and byte-golden suite runs in
//! the endpoints-gate column of Julia. Gated on `julia`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, scratch_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

fn julia_available() -> bool {
    Command::new("julia").arg("--version").output().is_ok()
}

fn endpoints_julia() -> PathBuf {
    PathBuf::from(format!(
        "{}/../../endpoints/julia",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// The reliable app: the codec `codegen::source(Language::Julia)` writes from
/// the IDL (`gen.jl`, `module Gen` over `runtime/julia/zerodds_wire.jl`)
/// encodes `Ping(seq = i, msg = "reliable")`; the producer puts each into the
/// `Channel` of the decoupled writer, whose drain task (`drain` of
/// `endpoints/julia/reliable_app.jl`, over the SDK's `Reliable.Sender`)
/// sends it.
const JULIA_RELIABLE_APP: &str = r#"
function encode_ping(i)
    zd_framing = Gen.ZeroddsWire.ZdPlCdr1Framing(
        pid_flags = "sentinel-must-understand",
        highest_short_id = 16128,
        pad_declared_length = true,
        pad_declared_optional_length = true,
        member_must_understand = true,
        pad_after_value = true,
    )
    w = Gen.ZeroddsWire.ZdWriter(Gen.ZeroddsWire.ZD_LITTLE, 4, zd_framing, false)
    Gen.zd_enc_Ping(w, Gen.Ping(Int32(i), "reliable"))
    Gen.ZeroddsWire.zd_writer_bytes(w)
end

port = parse(Int, ARGS[1])
n = parse(Int, ARGS[2])
chan = Channel{Vector{UInt8}}(1024)
drain_task = @async drain(port, chan)
for i in 0:(n - 1)
    put!(chan, encode_ping(i))
end
put!(chan, UInt8[])  # no more samples
wait(drain_task)
println("SENT $n")
"#;

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !julia_available() {
        eprintln!("SKIP julia_reliable {label}: `julia` not on PATH");
        return;
    }
    let n = RELIABLE_SAMPLES;
    let dir = scratch_dir(&format!("julia_reliable_{}", label.replace('/', "_")));
    std::fs::write(
        dir.join("gen.jl"),
        format!(
            "module Gen\ninclude(raw\"{}\")\nusing .ZeroddsWire\n\n{}\nend\n",
            runtime_dir("julia").join("zerodds_wire.jl").display(),
            source(Language::Julia)
        ),
    )
    .expect("write gen.jl");
    std::fs::write(
        dir.join("app.jl"),
        format!(
            "include(\"gen.jl\")\ninclude(raw\"{}\")\n{JULIA_RELIABLE_APP}",
            endpoints_julia()
                .join("reliable_app.jl")
                .canonicalize()
                .expect("endpoints/julia/reliable_app.jl")
                .display()
        ),
    )
    .expect("write app.jl");
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = Command::new("julia")
        .arg(dir.join("app.jl"))
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn julia reliable app");
    let delivered = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &delivered, n);
}

/// reliable-endpoint 1.0 §5 point 3: the Julia app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Julia toolchain; runs in integration-bindings-jvm-julia"]
fn reliable_5_3_julia_samples_arrive_in_order_despite_dropped_datagrams() {
    run_loss(Some(3), "julia/loss");
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Julia producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the Julia toolchain; runs in integration-bindings-jvm-julia"]
fn reliable_5_4_julia_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !julia_available() {
        eprintln!("SKIP julia_reliable bench: `julia` not on PATH");
        return;
    }
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let out = Command::new("julia")
        .arg(endpoints_julia().join("reliable_app.jl"))
        .arg(peer.port.to_string())
        .arg("0")
        .arg("bench")
        .output()
        .expect("run julia bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("julia reliable {}", stdout.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("julia", &stdout);
}
