// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Java reliable-stream endpoint: `endpoints/java`'s `AsyncReliableWriter`
//! (package `org.zerodds.endpoint`) is the reliable sender -- a producer
//! path enqueues into a `BlockingQueue`, a drain `Thread` owns the
//! `ReliableSender` state, frames + sends `WRITE_DATA` over a real
//! `DatagramSocket`, fires `HEARTBEAT` on a timer, and retransmits on
//! `ACKNACK` until the send window drains. The shared Rust reliable peer
//! (`bind_reliable_peer`/`reliable_receive`) injects loss and drives
//! recovery. The samples are the `Ping` of the codec the Java generator
//! writes from the IDL, so the run checks IDL, generated codec and runtime
//! together. Plus the producer-latency comparison. The unit and byte-golden
//! suite runs in the endpoints-gate column of Java. Gated on `javac`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

fn javac_available() -> bool {
    Command::new("javac")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn endpoints_java() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/java")
}

/// The `org.zerodds.endpoint` state-machine + wire + async-writer classes,
/// laid out at their package path so `javac -d` derives the package
/// directory automatically.
fn core_sources() -> Vec<PathBuf> {
    let base = endpoints_java().join("org/zerodds/endpoint");
    [
        "UnannotatedTypes.java",
        "ReliableWire.java",
        "ReliableSender.java",
        "ReliableReceiver.java",
        "AsyncReliableWriter.java",
    ]
    .iter()
    .map(|f| base.join(f))
    .collect()
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pp_java_rel_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("mkdir tmp");
    d
}

/// Compiles [`core_sources`] plus the given default-package driver file(s)
/// (resolved under `endpoints/java/`) into `out_dir`.
fn javac_build(out_dir: &Path, extra: &[&str]) {
    let java_dir = endpoints_java();
    let mut srcs = core_sources();
    for f in extra {
        srcs.push(java_dir.join(f));
    }
    let o = Command::new("javac")
        .arg("-encoding")
        .arg("UTF-8")
        .arg("-nowarn")
        .arg("-d")
        .arg(out_dir)
        .args(&srcs)
        .output()
        .expect("run javac");
    assert!(
        o.status.success(),
        "javac failed for {extra:?}:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

/// Every `.java` file under `dir`.
fn collect_java(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            collect_java(&path, out);
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(path);
        }
    }
}

/// The generated Java split as `tests/java.rs` splits it: `ZdTypes` as a
/// file of its own, the members of `ZdCodec` (private static) inlined into
/// the app class.
fn split_generated() -> (String, String) {
    let text = source(Language::Java);
    let at = text
        .find("public class ZdCodec {")
        .expect("generated Java declares `public class ZdCodec`");
    let types = text[..at].trim_end().to_string();
    let body = &text[at..];
    let open = body.find('\n').expect("class opens a line");
    let members = body[open + 1..]
        .trim_end()
        .strip_suffix('}')
        .expect("class closes with `}`")
        .to_string();
    (types, members)
}

/// The reliable app around the inlined codec: the generated encoder writes
/// `Ping { seq = i, msg = "reliable" }` and the SDK's async-decoupled
/// `AsyncReliableWriter` sends it.
const APP_HEAD: &str = r#"
import java.net.InetAddress;

import org.zerodds.endpoint.AsyncReliableWriter;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.PlCdr1Framing;
import org.zerodds.wire.Reader;
import org.zerodds.wire.TryConstruct;
import org.zerodds.wire.Writer;

public final class App {
"#;

const APP_TAIL: &str = r#"
    static byte[] encodePing(int i) {
        ZdTypes.Ping ping = new ZdTypes.Ping();
        ping.seq = i;
        ping.msg = "reliable";
        Writer w = new Writer(Endianness.LITTLE, 4, PlCdr1Framing.defaults(), false);
        zd_enc_Ping(w, ping);
        return w.finish();
    }

    public static void main(String[] args) throws Exception {
        int port = Integer.parseInt(args[0]);
        int n = Integer.parseInt(args[1]);
        AsyncReliableWriter writer =
            new AsyncReliableWriter(InetAddress.getByName("127.0.0.1"), port, 0x0015);
        writer.start();
        for (int i = 0; i < n; i++) {
            writer.submit(encodePing(i));
        }
        boolean ok = writer.finish(20_000);
        writer.close();
        if (!ok) {
            System.err.println("RELIABLE INCOMPLETE");
            System.exit(1);
        }
        System.out.println("SENT " + n);
    }
}
"#;

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !javac_available() {
        eprintln!("SKIP {label}: javac not on PATH");
        return;
    }
    let n = RELIABLE_SAMPLES;
    let dir = tmp_dir(label.replace('/', "_").as_str());
    let (types, members) = split_generated();
    std::fs::write(dir.join("ZdTypes.java"), format!("{types}\n")).expect("write ZdTypes");
    std::fs::write(
        dir.join("App.java"),
        format!("{APP_HEAD}{members}\n{APP_TAIL}"),
    )
    .expect("write App");
    let mut srcs = core_sources();
    collect_java(
        &runtime_dir("java").join("src/main/java/org/zerodds/wire"),
        &mut srcs,
    );
    srcs.push(dir.join("ZdTypes.java"));
    srcs.push(dir.join("App.java"));
    let out = dir.join("out");
    let o = Command::new("javac")
        .arg("-encoding")
        .arg("UTF-8")
        .arg("-nowarn")
        .arg("-d")
        .arg(&out)
        .args(&srcs)
        .output()
        .expect("run javac");
    assert!(
        o.status.success(),
        "javac failed for the reliable app:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );

    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child: Child = Command::new("java")
        .arg("-cp")
        .arg(&out)
        .arg("App")
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the java reliable app");

    let delivered = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &delivered, n);
}

/// reliable-endpoint 1.0 §5 point 3: the Java app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Java toolchain; runs in integration-bindings"]
fn reliable_5_3_java_samples_arrive_in_order_despite_dropped_datagrams() {
    run_loss(Some(3), "java/loss");
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Java producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the Java toolchain; runs in integration-bindings"]
fn reliable_5_4_java_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !javac_available() {
        eprintln!("SKIP java reliable latency bench: javac not on PATH");
        return;
    }
    let dir = tmp_dir("bench");
    javac_build(&dir, &["ReliableBench.java"]);
    let o = Command::new("java")
        .arg("-cp")
        .arg(&dir)
        .arg("ReliableBench")
        .output()
        .expect("run ReliableBench");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("producer latency"),
        "java bench produced no figure:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    eprintln!("java bench: {}", out.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("java", &out);
}
