// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! D reliable-stream endpoint, live: the D app is the reliable sender (submit +
//! WRITE_DATA + on-ACKNACK retransmit); the shared Rust reliable peer injects
//! loss and drives recovery. The samples are the `Ping` of the codec the D
//! generator writes from the IDL, so the run checks IDL, generated codec and
//! runtime together. Plus the producer-latency comparison. D's unit
//! and byte-golden suite and its in-process example run in the endpoints-gate column of D.
//! Gated on `gdc`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

fn gdc_available() -> bool {
    Command::new("gdc").arg("--version").output().is_ok()
}

fn endpoints_d() -> String {
    format!("{}/../../endpoints/d", env!("CARGO_MANIFEST_DIR"))
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pp_d_rel_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("mkdir");
    d
}

/// Compile the given `endpoints/d` sources into one binary with gdc.
fn gdc_build(bin: &Path, srcs: &[&str]) {
    let d = endpoints_d();
    let mut c = Command::new("gdc");
    for s in srcs {
        c.arg(format!("{d}/{s}"));
    }
    c.arg("-o").arg(bin);
    let o = c.output().expect("run gdc");
    assert!(
        o.status.success(),
        "gdc failed for {srcs:?}:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

/// The reliable app: the codec `codegen::source(Language::D)` writes from the
/// IDL (`gen.d`, calling `runtime/d/zdwire.d`) encodes
/// `Ping(seq = i, msg = "reliable")`, and the sender loop of
/// `endpoints/d/reliable_app.d` over the SDK's `Sender` (`reliable.d`) sends
/// it: WRITE_DATA for each, then HEARTBEAT and the retransmit of what an
/// ACKNACK marks missing until the window drains.
const D_RELIABLE_APP: &str = r#"
module app;

import gen;
import zdwire;
import reliable;
import std.socket;
import std.conv : to;
import std.stdio : stderr;
import core.time : MonoTime, dur;

ubyte[] encodePing(int i)
{
    Ping ping;
    ping.seq = i;
    ping.msg = "reliable";
    auto zdFraming = zdwire.PlCdr1Framing("sentinel-must-understand", 16128, true, true, true, true);
    auto zdW = new zdwire.Writer(zdwire.Endianness.little, 4, zdFraming, false);
    zd_enc_Ping(zdW, ping);
    return zdW.Finish();
}

void main(string[] args)
{
    immutable ushort port = to!ushort(args[1]);
    immutable int n = to!int(args[2]);
    auto sock = new UdpSocket();
    sock.bind(new InternetAddress("127.0.0.1", InternetAddress.PORT_ANY));
    auto peer = new InternetAddress("127.0.0.1", port);
    sock.setOption(SocketOptionLevel.SOCKET, SocketOption.RCVTIMEO, dur!"msecs"(50));

    Sender s;
    foreach (i; 0 .. n)
    {
        ushort seq;
        auto sample = encodePing(i);
        auto st = s.submit(sample, seq);
        if (st != SubmitStatus.ok)
        {
            stderr.writeln("submit failed: ", st);
            return;
        }
        sock.sendTo(reliableWriteFrame(seq, 0x0015, sample), peer);
    }

    immutable deadline = MonoTime.currTime + dur!"seconds"(20);
    ubyte[4096] buf;
    while (s.inFlightCount() > 0 && MonoTime.currTime < deadline)
    {
        Heartbeat hb;
        if (s.pendingHeartbeat(MonoTime.currTime, hb))
            sock.sendTo(heartbeatFrame(hb), peer);
        else
        {
            auto seqs = s.inFlightSeqs();
            sock.sendTo(heartbeatFrame(Heartbeat(cast(short) seqs[0],
                    cast(short) seqs[$ - 1], STREAM_RELIABLE)), peer);
        }
        Address from;
        auto got = sock.receiveFrom(buf[], from);
        if (got <= 0)
            continue;
        AckNack a;
        if (!parseAckNack(buf[0 .. got], a))
            continue;
        s.recvAcknack(a);
        ushort base = cast(ushort) a.firstUnacked;
        ushort bitmap = cast(ushort)(a.nack[0] | (a.nack[1] << 8));
        foreach (ushort i; 0 .. 16)
        {
            if (((bitmap >> i) & 1) == 0)
                continue;
            ushort seq = cast(ushort)(base + i);
            auto p = s.getInFlight(seq);
            if (p !is null)
                sock.sendTo(reliableWriteFrame(seq, 0x0015, p), peer);
        }
    }
    if (s.inFlightCount() != 0)
        stderr.writeln("RELIABLE INCOMPLETE: ", s.inFlightCount(), " still in-flight");
}
"#;

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !gdc_available() {
        eprintln!("SKIP {label}: gdc not on PATH");
        return;
    }
    let dir = tmp_dir(label.replace('/', "_").as_str());
    std::fs::write(
        dir.join("gen.d"),
        format!("module gen;\n\nimport zdwire;\n\n{}", source(Language::D)),
    )
    .expect("write gen.d");
    std::fs::write(dir.join("app.d"), D_RELIABLE_APP).expect("write app.d");
    std::fs::copy(runtime_dir("d").join("zdwire.d"), dir.join("zdwire.d")).expect("copy zdwire.d");
    std::fs::copy(
        format!("{}/reliable.d", endpoints_d()),
        dir.join("reliable.d"),
    )
    .expect("copy reliable.d");
    let app = dir.join("reliable_app");
    let o = Command::new("gdc")
        .args(["app.d", "gen.d", "zdwire.d", "reliable.d", "-o"])
        .arg(&app)
        .current_dir(&dir)
        .output()
        .expect("run gdc");
    assert!(
        o.status.success(),
        "gdc failed for the reliable app:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );

    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let n = RELIABLE_SAMPLES;
    let child = Command::new(&app)
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn d app");

    let samples = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &samples, n);
}

/// reliable-endpoint 1.0 §5 point 3: the D app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the D toolchain; runs in integration-bindings"]
fn reliable_5_3_d_samples_arrive_in_order_despite_dropped_datagrams() {
    run_loss(Some(3), "d/loss");
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the D producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the D toolchain; runs in integration-bindings"]
fn reliable_5_4_d_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !gdc_available() {
        eprintln!("SKIP d reliable latency bench: gdc not on PATH");
        return;
    }
    let dir = tmp_dir("bench");
    let bin = dir.join("reliable_bench");
    gdc_build(&bin, &["reliable.d", "reliable_bench.d"]);
    let o = Command::new(&bin).output().expect("run bench");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("producer latency"),
        "d bench produced no figure:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    eprintln!("d bench: {}", out.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("d", &out);
}
