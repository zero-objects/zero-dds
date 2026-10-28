// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OCaml endpoint ping-pong: the generated `idlc` types (`Gen.Ping`/`Gen.Pong`,
//! marshal + unmarshal) plus the native OCaml endpoint SDK (`endpoints/ocaml`,
//! sync `Client` and async `AsyncReader`) drive a real UDP socket with XRCE
//! framing against the shared Rust peer. Gated on `ocamlfind`.
//!
//! Name-clash note: the generated module and the SDK each define their own
//! `module Wire` (byte-identical wire core). Kept apart as separate compilation
//! units — `gen.ml` → `Gen.Wire`, `zerodds.ml` → `Zerodds.Wire` — so the app
//! qualifies each explicitly and nothing collides.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{
    Language, endpoints_dir, runtime_dir, scratch_dir, source, tool_available,
};
use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong};

fn ocaml_available() -> bool {
    tool_available("ocamlfind", "printconf")
}

/// The generated Ping/Pong types + codec. It calls `Zdwire`, which is
/// `runtime/ocaml/zdwire.ml` compiled into the same program.
fn gen_module() -> String {
    source(Language::OCaml)
}

// The app: build a Ping via the generated marshal, ship it through the endpoint
// SDK (sync `Client` or async `AsyncReader`) over UDP + XRCE, decode the Pong
// with the generated unmarshal, print the exact line the peer asserts.
const OCAML_MAIN: &str = r#"
let () =
  let mode = Sys.argv.(1) in
  let port = int_of_string Sys.argv.(2) in
  let sock = Unix.socket Unix.PF_INET Unix.SOCK_DGRAM 0 in
  Unix.bind sock (Unix.ADDR_INET (Unix.inet_addr_loopback, 0));
  let peer = Unix.ADDR_INET (Unix.inet_addr_loopback, port) in
  let deliver (frame : bytes) =
    ignore (Unix.sendto sock frame 0 (Bytes.length frame) [] peer)
  in
  let buf = Bytes.create 4096 in
  let receive () =
    match Unix.select [ sock ] [] [] 0.05 with
    | [], _, _ -> None
    | _ ->
        let n, _ = Unix.recvfrom sock buf 0 4096 [] in
        Some (Bytes.sub buf 0 n)
  in
  let transport = { Zerodds.deliver; receive } in
  let zd_framing = { Zdwire.pid_flags = "sentinel-must-understand";
                     highest_short_id = 16128; pad_declared_length = true;
                     pad_declared_optional_length = true;
                     member_must_understand = true; pad_after_value = true } in
  let w = Zdwire.new_writer Zdwire.Little 4 zd_framing false in
  let ping : Gen.ping = { Gen.seq = 1l; msg = "hello from app" } in
  Gen.zd_enc_ping w ping;
  let sample = Zdwire.finish w in
  let body =
    if mode = "async" then begin
      let reader = Zerodds.AsyncReader.start transport in
      let frame =
        Zerodds.Endpoint.write_frame Zerodds.Endpoint.session_nokey
          Zerodds.Endpoint.stream_best_effort 1
          (Zerodds.Endpoint.object_id 0x001 Zerodds.Endpoint.objk_datawriter)
          sample
      in
      transport.Zerodds.deliver frame;
      let b = Zerodds.AsyncReader.recv reader in
      Zerodds.AsyncReader.stop reader;
      b
    end
    else begin
      let client = Zerodds.Client.create transport
          (Zerodds.Endpoint.object_id 0x001 Zerodds.Endpoint.objk_datawriter) in
      Zerodds.Client.write client sample;
      let deadline = Unix.gettimeofday () +. 10.0 in
      let rec wait () =
        if Unix.gettimeofday () > deadline then failwith "sync: no pong"
        else match Zerodds.Client.poll client with Some b -> b | None -> wait ()
      in
      wait ()
    end
  in
  let r = Zdwire.new_reader (Bytes.to_string body) Zdwire.Little 4 zd_framing in
  let pong = Gen.zd_dec_pong r Zdwire.Discard in
  Printf.printf "PONG seq=%ld reply=%s\n" pong.Gen.seq pong.Gen.reply;
  flush stdout
"#;

fn build_ocaml(mode: &str, port: u16) -> Child {
    let dir = scratch_dir(&format!("ocaml_{mode}"));

    // The SDK (its own `module Wire` + `Endpoint`/`Client`/`AsyncReader`).
    let sdk_src = std::fs::read_to_string(endpoints_dir("ocaml").join("zerodds.ml"))
        .expect("read endpoints/ocaml/zerodds.ml");
    std::fs::write(dir.join("zerodds.ml"), &sdk_src).expect("write zerodds.ml");
    // The wire runtime the generated codec calls, as its own compilation unit
    // (`Zdwire`) beside the SDK's unrelated `Zerodds.Wire`.
    std::fs::copy(
        runtime_dir("ocaml").join("zdwire.ml"),
        dir.join("zdwire.ml"),
    )
    .expect("copy zdwire.ml");
    std::fs::write(dir.join("gen.ml"), gen_module()).expect("write gen.ml");
    std::fs::write(dir.join("main.ml"), OCAML_MAIN).expect("write main.ml");

    // Dependency order: SDK, generated types, then the app. Threads for the
    // async reader's Mutex/Condition mailbox; Unix for the UDP socket.
    let build = Command::new("ocamlfind")
        .args([
            "ocamlopt",
            "-thread",
            "-package",
            "unix,threads.posix",
            "-linkpkg",
            "zdwire.ml",
            "zerodds.ml",
            "gen.ml",
            "main.ml",
            "-o",
            "main_bin",
        ])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind ocamlopt");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    Command::new("./main_bin")
        .arg(mode)
        .arg(port.to_string())
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn main_bin")
}

fn endpoint_case(mode: &str) {
    if !ocaml_available() {
        eprintln!("SKIP ocaml endpoint {mode}: `ocamlfind` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_ocaml(mode, peer.port);
    ping_pong(&peer, child, &format!("ocaml/{mode}"));
}

#[test]
fn ocaml_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn ocaml_endpoint_async() {
    endpoint_case("async");
}
