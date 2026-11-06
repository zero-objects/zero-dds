// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Kotlin reliable-stream endpoint: `endpoints/kotlin`'s `AsyncReliableWriter`
//! (package `zerodds`) is the reliable sender -- a producer path enqueues into
//! an `ArrayBlockingQueue`, a drain `Thread` owns the `ReliableSender` state,
//! frames + sends `WRITE_DATA` over a real `DatagramSocket`, fires `HEARTBEAT`
//! on a timer, and retransmits on `ACKNACK` until the send window drains. The
//! shared Rust reliable peer (`bind_reliable_peer`/`reliable_receive`) injects
//! loss and drives recovery. The samples are the `Ping` of the codec the
//! Kotlin generator writes from the IDL, so the run checks IDL, generated
//! codec and runtime together. The unit and byte-golden suite runs in
//! the endpoints-gate column of Kotlin. Gated on `kotlinc`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

fn kotlinc_available() -> bool {
    Command::new("kotlinc")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn endpoints_kotlin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/kotlin")
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pp_kt_rel_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("mkdir tmp");
    d
}

/// Compiles the reliable core (`src/Reliable.kt`) plus the given driver file
/// (resolved under `endpoints/kotlin/`) into a self-contained `app.jar`
/// (`-include-runtime` bundles the Kotlin stdlib so plain `java -cp` runs it).
/// Returns the jar path.
fn kotlinc_build(out_dir: &Path, driver: &str) -> PathBuf {
    let kt = endpoints_kotlin();
    let jar = out_dir.join("app.jar");
    let o = Command::new("kotlinc")
        .arg(kt.join("src/Reliable.kt"))
        .arg(kt.join(driver))
        .arg("-include-runtime")
        .arg("-d")
        .arg(&jar)
        .output()
        .expect("run kotlinc");
    assert!(
        o.status.success(),
        "kotlinc failed for {driver}:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    jar
}

/// The reliable app: the codec `codegen::source(Language::Kotlin)` writes
/// from the IDL (`Gen.kt`, calling `runtime/kotlin/ZdWire.kt`) encodes
/// `Ping(seq = i, msg = "reliable")`, and the SDK's async-decoupled
/// `AsyncReliableWriter` sends it.
const KT_RELIABLE_APP: &str = r#"import zerodds.AsyncReliableWriter
import zerodds.wire.Endianness
import zerodds.wire.PlCdr1Framing
import zerodds.wire.Writer
import java.net.InetAddress
import kotlin.system.exitProcess

fun encodePing(i: Int): ByteArray {
    val w = Writer(Endianness.LITTLE, 4, PlCdr1Framing.specification(), false)
    zd_enc_Ping(w, Ping(seq = i, msg = "reliable"))
    return w.finish()
}

fun main(args: Array<String>) {
    val port = args[0].toInt()
    val n = args[1].toInt()
    val writer = AsyncReliableWriter(InetAddress.getByName("127.0.0.1"), port, 0x0015)
    writer.start()
    for (i in 0 until n) {
        writer.submit(encodePing(i))
    }
    val ok = writer.finish(20_000)
    writer.close()
    if (!ok) {
        System.err.println("RELIABLE INCOMPLETE")
        exitProcess(1)
    }
    println("SENT $n")
}
"#;

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !kotlinc_available() {
        eprintln!("SKIP {label}: kotlinc not on PATH");
        return;
    }
    let n = RELIABLE_SAMPLES;
    let dir = tmp_dir(label.replace('/', "_").as_str());
    std::fs::write(
        dir.join("Gen.kt"),
        format!("import zerodds.wire.*\n\n{}", source(Language::Kotlin)),
    )
    .expect("write Gen.kt");
    std::fs::write(dir.join("App.kt"), KT_RELIABLE_APP).expect("write App.kt");
    let jar = dir.join("app.jar");
    let o = Command::new("kotlinc")
        .arg(runtime_dir("kotlin").join("ZdWire.kt"))
        .arg(endpoints_kotlin().join("src/Reliable.kt"))
        .arg(dir.join("Gen.kt"))
        .arg(dir.join("App.kt"))
        .arg("-include-runtime")
        .arg("-d")
        .arg(&jar)
        .output()
        .expect("run kotlinc");
    assert!(
        o.status.success(),
        "kotlinc failed for the reliable app:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );

    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child: Child = Command::new("java")
        .arg("-cp")
        .arg(&jar)
        .arg("AppKt")
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the kotlin reliable app");

    let delivered = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &delivered, n);
}

/// reliable-endpoint 1.0 §5 point 3: the Kotlin app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Kotlin toolchain; runs in integration-bindings-jvm-julia"]
fn reliable_5_3_kotlin_samples_arrive_in_order_despite_dropped_datagrams() {
    run_loss(Some(3), "kotlin/loss");
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Kotlin producer's enqueue
/// into the async-decoupled writer costs less than the inline send it
/// replaces, which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the Kotlin toolchain; runs in integration-bindings-jvm-julia"]
fn reliable_5_4_kotlin_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !kotlinc_available() {
        eprintln!("SKIP kotlin reliable bench: kotlinc not on PATH");
        return;
    }
    let dir = tmp_dir("bench");
    let jar = kotlinc_build(&dir, "ReliableExample.kt");
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let out = Command::new("java")
        .arg("-cp")
        .arg(&jar)
        .arg("ReliableExampleKt")
        .arg(peer.port.to_string())
        .arg("0")
        .arg("bench")
        .output()
        .expect("run kotlin ReliableExample bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("kotlin reliable {}", stdout.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("kotlin", &stdout);
}
