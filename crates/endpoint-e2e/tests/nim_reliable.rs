// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Nim reliable endpoint, live: the pure-Nim reliable state machine + frame
//! codec (`endpoints/nim/reliable.nim`) driven by a decoupled ring+drain-thread
//! sender (the drain of `reliable_app.nim`) against the shared Rust reliable
//! peer: the peer drops, the app retransmits on ACKNACK, all samples arrive
//! gap-free. The samples are the `Ping` of the codec the Nim generator writes
//! from the IDL, so the run checks IDL, generated codec and runtime together.
//! And the producer-latency comparison. The unit and byte-golden suite runs in
//! the endpoints-gate column of Nim. Gated on `nim`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, scratch_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

fn nim_available() -> bool {
    Command::new("nim").arg("--version").output().is_ok()
}

fn endpoints_nim() -> PathBuf {
    PathBuf::from(format!(
        "{}/../../endpoints/nim",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// Compiles a Nim source under `endpoints/nim/` (its `import ./reliable` resolves
/// in place) to a temp binary. `tag` keeps parallel tests from sharing a dir.
fn compile_nim(src: &str, tag: &str, threads: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rel_nim_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let bin = dir.join("app");
    let mut cmd = Command::new("nim");
    cmd.args(["c", "--hints:off", "--warnings:off", "-d:release"]);
    if threads {
        cmd.arg("--threads:on");
    }
    let out = cmd
        .arg(format!("--nimcache:{}", dir.join("nimc").display()))
        .arg(format!("-o:{}", bin.display()))
        .arg(endpoints_nim().join(src))
        .output()
        .expect("nim c");
    assert!(
        out.status.success(),
        "nim compile {src} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    bin
}

/// The reliable app after the generated codec (inlined: the generator marks
/// nothing with Nim's `*`, `tests/nim.rs`): `encodePing` writes
/// `Ping(seq: i, msg: "reliable")` with it, the producer pushes each into the
/// SPSC ring of the decoupled writer, and the drain thread of
/// `endpoints/nim/reliable_app.nim` over the SDK's `Sender` sends,
/// heartbeats and retransmits.
const NIM_RELIABLE_APP: &str = r#"
proc toStr(b: seq[byte]): string =
  result = newString(b.len)
  for i, x in b:
    result[i] = char(x)

proc toBytes(s: string, n: int): seq[byte] =
  result = newSeq[byte](n)
  for i in 0 ..< n:
    result[i] = byte(s[i])

proc nowMs(): int = int(epochTime() * 1000.0)

proc encodePing(i: int): seq[byte] =
  var zdFraming = zdwire.PlCdr1Framing(pidFlags: "sentinel-must-understand",
    highestShortId: 16128, padDeclaredLength: true,
    padDeclaredOptionalLength: true, memberMustUnderstand: true,
    padAfterValue: true)
  var w = zdwire.newWriter(zdwire.EndianLittle, 4, zdFraming, false)
  zd_enc_Ping(w, Ping(seq: int32(i), msg: "reliable"))
  w.finish()

const QueueSlots = 4096
type
  Queue = SpscRing[QueueSlots]
  DrainArgs = tuple[port: int, queue: ptr Queue]

proc drain(args: DrainArgs) {.thread.} =
  let queue = args.queue
  var sock = newSocket(AF_INET, SOCK_DGRAM, IPPROTO_UDP)
  sock.connect("127.0.0.1", Port(args.port))
  var sender = newSender()
  var msgSeq = 1
  var closed = false
  let startMs = nowMs()
  while true:
    var msg: seq[byte]
    while queue[].tryPop(msg):
      if msg.len == 0:
        closed = true
      else:
        let sub = sender.submit(msg)
        if sub.status == srOk:
          sock.send(toStr(reliableWriteFrame(sub.seq, 0x0015'u16, msg)))
    if closed and sender.inFlightCount == 0:
      break
    if nowMs() - startMs > 20000:
      break
    let hb = sender.pendingHeartbeat(nowMs())
    if hb.isSome:
      sock.send(toStr(heartbeatFrame(SessionNoKey, StreamNone, msgSeq, hb.get)))
      inc msgSeq
    for _ in 0 ..< 64:
      var fds = @[sock.getFd()]
      if selectRead(fds, 20) <= 0:
        break
      var buf = newString(2048)
      var fromAddress = ""
      var fromPort: Port
      var n = -1
      try:
        n = sock.recvFrom(buf, 2048, fromAddress, fromPort)
      except OSError:
        discard
      if n <= 0:
        break
      let ack = parseAcknack(toBytes(buf, n))
      if ack.isSome:
        sender.recvAcknack(ack.get)
    if sender.inFlightCount > 0:
      var pending: seq[tuple[seq: uint16, payload: seq[byte]]] = @[]
      for pr in sender.inFlightPairs:
        pending.add(pr)
      for pr in pending:
        sock.send(toStr(reliableWriteFrame(pr.seq, 0x0015'u16, pr.payload)))
  sock.close()

when isMainModule:
  let port = parseInt(paramStr(1))
  let n = parseInt(paramStr(2))
  let queue = createShared(Queue)
  var t: Thread[DrainArgs]
  createThread(t, drain, (port, queue))
  for i in 0 ..< n:
    discard queue[].push(encodePing(i))
  discard queue[].push(@[]) # no more samples
  joinThread(t)
  freeShared(queue)
  echo "SENT ", n
"#;

fn run_loss(drop_every: Option<usize>, tag: &str, label: &str) {
    if !nim_available() {
        eprintln!("SKIP nim_reliable {label}: `nim` not on PATH");
        return;
    }
    let n = RELIABLE_SAMPLES;
    let dir = scratch_dir(&format!("nim_reliable_{tag}"));
    std::fs::copy(
        runtime_dir("nim").join("zdwire.nim"),
        dir.join("zdwire.nim"),
    )
    .expect("copy zdwire.nim");
    for module in ["reliable.nim", "spsc_ring.nim"] {
        std::fs::copy(endpoints_nim().join(module), dir.join(module))
            .unwrap_or_else(|e| panic!("copy {module}: {e}"));
    }
    std::fs::write(
        dir.join("app.nim"),
        format!(
            "import std/[net, nativesockets, options, os, strutils, times]\n\
             import zdwire\nimport ./reliable\nimport ./spsc_ring\n\n{}\n{NIM_RELIABLE_APP}",
            source(Language::Nim)
        ),
    )
    .expect("write app.nim");
    let bin = dir.join("app");
    let out = Command::new("nim")
        .args([
            "c",
            "--hints:off",
            "--warnings:off",
            "-d:release",
            "--threads:on",
        ])
        .arg(format!("--nimcache:{}", dir.join("nimc").display()))
        .arg(format!("-o:{}", bin.display()))
        .arg(dir.join("app.nim"))
        .output()
        .expect("nim c");
    assert!(
        out.status.success(),
        "nim compile of the reliable app failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = Command::new(&bin)
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nim reliable app");
    let delivered = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &delivered, n);
}

/// reliable-endpoint 1.0 §5 point 3: the Nim app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Nim toolchain; runs in integration-bindings-zig-nim"]
fn reliable_5_3_nim_samples_arrive_in_order_despite_dropped_datagrams() {
    run_loss(Some(3), "loss", "nim/loss");
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Nim producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the Nim toolchain; runs in integration-bindings-zig-nim"]
fn reliable_5_4_nim_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !nim_available() {
        eprintln!("SKIP nim_reliable bench: `nim` not on PATH");
        return;
    }
    let bin = compile_nim("reliable_app.nim", "bench", true);
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let out = Command::new(&bin)
        .arg(peer.port.to_string())
        .arg("0")
        .arg("bench")
        .output()
        .expect("run nim bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("nim reliable {}", stdout.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("nim", &stdout);
}
