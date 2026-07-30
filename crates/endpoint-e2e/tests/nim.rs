// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Nim endpoint ping-pong: generated `idlc` types (`gen.nim`) + the pure-Nim
//! endpoint SDK (`endpoints/nim/zerodds.nim`) over a real UDP socket, with XRCE
//! framing. Sync path uses the `Client` poll loop; async path uses the
//! `AsyncReader` `Future`/`await` model. Gated on `nim`.
//!
//! Both the generated module and the SDK define the same wire core
//! (`Endian`/`eLE`/`Writer`/`initWriter`) — a full `import` of both would clash.
//! The app therefore `import ./gen` (full, for the types + `marshalXCDR`) and a
//! selective `from ./zerodds import …` (only the transport/client/reader
//! symbols), so the wire core comes from exactly one module.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_nim::{NimGenOptions, generate_nim_module};

fn nim_available() -> bool {
    Command::new("nim").arg("--version").output().is_ok()
}

/// The generated Ping/Pong Nim module (self-contained: its own imports + wire
/// core + `marshalXCDR`/`unmarshalXCDRPong`).
fn gen_module() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    generate_nim_module(&ast, &NimGenOptions::default()).expect("gen")
}

// The app: a real non-blocking UDP transport, then Ping → (sync poll | async
// recv) → print the Pong. `Client.write` frames+sends in both modes (the Nim SDK
// exposes a sync writer only); the async path proves the `AsyncReader` Future.
const NIM_ENDPOINT_MAIN: &str = r#"
proc udpTransport(peerPort: int): Transport =
  var sock = newSocket(AF_INET, SOCK_DGRAM, IPPROTO_UDP, buffered = false)
  sock.bindAddr(Port(0))
  sock.getFd().setBlocking(false)
  result = Transport(
    deliver: proc(frame: seq[byte]) =
      var s = newString(frame.len)
      for i in 0 ..< frame.len:
        s[i] = char(frame[i])
      sock.sendTo("127.0.0.1", Port(peerPort), s),
    receive: proc(): Option[seq[byte]] =
      var data = ""
      var address = ""
      var fp: Port
      try:
        let n = sock.recvFrom(data, 4096, address, fp)
        if n <= 0:
          return none(seq[byte])
        var b = newSeq[byte](n)
        for i in 0 ..< n:
          b[i] = byte(data[i])
        some(b)
      except CatchableError:
        none(seq[byte])
  )

when isMainModule:
  let mode = paramStr(1)
  let peerPort = parseInt(paramStr(2))
  let tr = udpTransport(peerPort)
  let ping = Ping(seq: 1'i32, msg: "hello from app")
  let sample = ping.marshalXCDR(eLE)
  let client = newClient(tr)
  client.write(sample)
  var body: seq[byte]
  if mode == "async":
    let reader = newAsyncReader(tr)
    let fut = reader.recv()
    if waitFor fut.withTimeout(10000):
      body = fut.read()
    else:
      quit("async: no pong", 1)
  else:
    var got = false
    let deadline = epochTime() + 10.0
    while epochTime() < deadline:
      let r = client.poll()
      if r.isSome:
        body = r.get
        got = true
        break
      sleep(1)
    if not got:
      quit("sync: no pong", 1)
  let pong = unmarshalXCDRPong(body, eLE)
  echo "PONG seq=", pong.seq, " reply=", pong.reply
"#;

/// Writes `gen.nim` + a copy of the SDK `zerodds.nim` + `main.nim`, compiles
/// (nim can be slow — do it *before* the ping-pong so the peer's recv timeout
/// only covers runtime), then spawns the binary. Returns the running app.
fn build_nim_endpoint(mode: &str, port: u16) -> Child {
    let dir = std::env::temp_dir().join(format!("pp_nim_{mode}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");

    std::fs::write(dir.join("gen.nim"), gen_module()).expect("write gen.nim");

    let sdk = format!(
        "{}/../../endpoints/nim/zerodds.nim",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::copy(&sdk, dir.join("zerodds.nim")).expect("copy zerodds.nim");

    let mut main = String::from(
        "import std/[os, net, nativesockets, options, asyncdispatch, times, strutils]\n\
         import ./gen\n\
         from ./zerodds import Transport, Client, newClient, write, poll, AsyncReader, newAsyncReader, recv\n",
    );
    main.push_str(NIM_ENDPOINT_MAIN);
    std::fs::write(dir.join("main.nim"), &main).expect("write main.nim");

    let bin = dir.join("pingapp");
    let out = Command::new("nim")
        .args(["c", "--hints:off", "--warnings:off"])
        .arg(format!("--nimcache:{}", dir.join("nimc").display()))
        .arg(format!("-o:{}", bin.display()))
        .arg(dir.join("main.nim"))
        .output()
        .expect("nim c");
    assert!(
        out.status.success(),
        "nim compile failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    Command::new(&bin)
        .arg(mode)
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nim app")
}

fn endpoint_case(mode: &str) {
    if !nim_available() {
        eprintln!("SKIP nim endpoint {mode}: `nim` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_nim_endpoint(mode, peer.port);
    ping_pong(&peer, child, &format!("nim/{mode}"));
}

#[test]
fn nim_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn nim_endpoint_async() {
    endpoint_case("async");
}
