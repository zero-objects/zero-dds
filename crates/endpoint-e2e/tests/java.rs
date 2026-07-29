// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Java endpoint ping-pong: the generated `idl-java` TypeSupport marshals the
//! `Ping` and decodes the `Pong`, the endpoint SDK (`ZdwEndpoint`) does the XRCE
//! `WRITE_DATA` framing, and a real `java.net.DatagramSocket` carries the bytes
//! to the Rust peer. `sync` = the caller owns the receive loop; `async` = a
//! reader thread drains the socket into a `LinkedBlockingQueue` the consumer
//! blocks on (the idiomatic `java.util.concurrent` model). Gated on `javac`.
//!
//! Generated code is compiled against the **real** ZeroDDS Java runtime
//! (`crates/java-omgdds` + `crates/idl-java/runtime`) — the actual classes the
//! TypeSupport ships against (`org.zerodds.cdr.*`), never stubs.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_java::{JavaGenOptions, generate_java_files};

const CP_SEP: &str = if cfg!(windows) { ";" } else { ":" };

fn javac_available() -> bool {
    Command::new("javac")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Recursively collects every `.java` file under `dir`.
fn collect_java(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_java(&path, out);
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(path);
        }
    }
}

/// Compiles the real ZeroDDS Java runtime (`java-omgdds` main sources +
/// `idl-java/runtime`) once into a classes dir; returns its path. `None` when
/// `javac` is unavailable or the runtime fails to compile.
fn runtime_classes() -> Option<&'static Path> {
    static CELL: OnceLock<Option<PathBuf>> = OnceLock::new();
    CELL.get_or_init(|| {
        if !javac_available() {
            return None;
        }
        let out = std::env::temp_dir().join(format!("pp_java_rt_{}", std::process::id()));
        std::fs::create_dir_all(&out).ok()?;
        let mut srcs = Vec::new();
        collect_java(
            &manifest().join("../java-omgdds/java/src/main/java"),
            &mut srcs,
        );
        collect_java(&manifest().join("../idl-java/runtime"), &mut srcs);
        if srcs.is_empty() {
            eprintln!("SKIP java: no runtime sources found — check crate layout");
            return None;
        }
        let output = Command::new("javac")
            .arg("-nowarn")
            .arg("-d")
            .arg(&out)
            .args(&srcs)
            .output()
            .ok()?;
        if !output.status.success() {
            eprintln!(
                "SKIP java: real runtime failed to compile:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return None;
        }
        Some(out)
    })
    .as_deref()
}

// The app: default-package `Main`. Uses the generated `Ping`/`Pong`
// TypeSupport (marshal/decode) + the endpoint SDK `ZdwEndpoint` (XRCE framing)
// over a real UDP datagram socket.
const MAIN_JAVA: &str = r#"
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
import java.util.Arrays;
import java.util.concurrent.LinkedBlockingQueue;

public final class Main {
    public static void main(String[] args) throws Exception {
        String mode = args[0];
        int port = Integer.parseInt(args[1]);
        InetAddress host = InetAddress.getByName("127.0.0.1");
        DatagramSocket sock = new DatagramSocket();
        sock.setSoTimeout(20000);

        // Marshal the Ping with the generated TypeSupport (XCDR2 little-endian),
        // frame it with the endpoint SDK, and send it over UDP.
        Ping ping = new Ping();
        ping.setSeq(1);
        ping.setMsg("hello from app");
        byte[] sample = PingTypeSupport.INSTANCE.encode(
            ping, org.zerodds.cdr.EndianMode.LITTLE_ENDIAN);
        byte[] frame = ZdwEndpoint.xrceWriteFrame(
            ZdwEndpoint.SESSION_NOKEY, ZdwEndpoint.STREAM_BEST_EFFORT, 1, sample);
        sock.send(new DatagramPacket(frame, frame.length, host, port));

        byte[] body;
        if (mode.equals("async")) {
            // Async: a reader thread drains the socket; the consumer blocks on take().
            final LinkedBlockingQueue<byte[]> inbox = new LinkedBlockingQueue<byte[]>();
            final DatagramSocket rsock = sock;
            Thread reader = new Thread(new Runnable() {
                @Override
                public void run() {
                    try {
                        byte[] buf = new byte[4096];
                        DatagramPacket pkt = new DatagramPacket(buf, buf.length);
                        rsock.receive(pkt);
                        byte[] f = Arrays.copyOf(pkt.getData(), pkt.getLength());
                        inbox.add(ZdwEndpoint.xrceReadBody(f));
                    } catch (Exception e) {
                        // Leave the inbox empty; the consumer's take() would then block
                        // and the test's process timeout catches the failure.
                    }
                }
            });
            reader.setDaemon(true);
            reader.start();
            body = inbox.take();
        } else {
            // Sync: the caller owns the run-loop and blocks on receive().
            byte[] buf = new byte[4096];
            DatagramPacket pkt = new DatagramPacket(buf, buf.length);
            sock.receive(pkt);
            byte[] f = Arrays.copyOf(pkt.getData(), pkt.getLength());
            body = ZdwEndpoint.xrceReadBody(f);
        }

        Pong pong = PongTypeSupport.INSTANCE.decode(body);
        System.out.println("PONG seq=" + pong.getSeq() + " reply=" + pong.getReply());
        sock.close();
    }
}
"#;

/// Generates + compiles the Java app for `mode`, then spawns it pointed at
/// `port`. `None` if the runtime is unavailable (skip).
fn build_java(mode: &str, port: u16) -> Option<Child> {
    let classes = runtime_classes()?;
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");

    let src_dir = std::env::temp_dir().join(format!("pp_java_{mode}_{}", std::process::id()));
    let out_dir = src_dir.join("out");
    let _ = std::fs::remove_dir_all(&src_dir);
    std::fs::create_dir_all(&out_dir).expect("mkdir out");

    // Generated classes into their package dirs.
    let mut paths = Vec::new();
    for f in &files {
        let dir = if f.package_path.is_empty() {
            src_dir.clone()
        } else {
            src_dir.join(f.package_path.replace('.', "/"))
        };
        std::fs::create_dir_all(&dir).expect("mkdir pkg");
        let path = dir.join(format!("{}.java", f.class_name));
        std::fs::write(&path, &f.source).expect("write generated");
        paths.push(path);
    }

    // The endpoint SDK (XRCE framing) + the app driver, both default-package.
    let ep_src = manifest().join("../../endpoints/java/ZdwEndpoint.java");
    let ep_dst = src_dir.join("ZdwEndpoint.java");
    std::fs::copy(&ep_src, &ep_dst).expect("copy ZdwEndpoint.java");
    paths.push(ep_dst);
    let main_path = src_dir.join("Main.java");
    std::fs::write(&main_path, MAIN_JAVA).expect("write Main.java");
    paths.push(main_path);

    // Compile everything against the real runtime classpath.
    let javac = Command::new("javac")
        .arg("-nowarn")
        .arg("-classpath")
        .arg(classes)
        .arg("-d")
        .arg(&out_dir)
        .args(&paths)
        .output()
        .expect("javac");
    assert!(
        javac.status.success(),
        "javac failed:\n{}",
        String::from_utf8_lossy(&javac.stderr)
    );

    let cp = format!("{}{CP_SEP}{}", classes.display(), out_dir.display());
    let child = Command::new("java")
        .arg("-cp")
        .arg(cp)
        .arg("Main")
        .arg(mode)
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn java");
    Some(child)
}

fn endpoint_case(mode: &str) {
    if !javac_available() {
        eprintln!("SKIP java endpoint {mode}: `javac` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let Some(child) = build_java(mode, peer.port) else {
        eprintln!("SKIP java endpoint {mode}: runtime unavailable");
        return;
    };
    ping_pong(&peer, child, &format!("java/{mode}"));
}

#[test]
fn java_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn java_endpoint_async() {
    endpoint_case("async");
}
