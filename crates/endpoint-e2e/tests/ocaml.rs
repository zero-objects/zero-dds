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

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_ocaml::{OcamlGenOptions, generate_ocaml_module};

fn ocaml_available() -> bool {
    Command::new("ocamlfind").arg("printconf").output().is_ok()
}

/// The generated Ping/Pong module (its own `module Wire` + `Ping`/`Pong`).
fn gen_module() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    generate_ocaml_module(&ast, &OcamlGenOptions::default()).expect("gen")
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
  let ping : Gen.Ping.t = { Gen.Ping.seq = 1; msg = "hello from app" } in
  let sample = Gen.Ping.marshal ping Gen.Wire.LE in
  let body =
    if mode = "async" then begin
      let reader = Zerodds.AsyncReader.start transport in
      let frame =
        Zerodds.Endpoint.write_frame Zerodds.Endpoint.session_nokey
          Zerodds.Endpoint.stream_best_effort 1 sample
      in
      transport.Zerodds.deliver frame;
      let b = Zerodds.AsyncReader.recv reader in
      Zerodds.AsyncReader.stop reader;
      b
    end
    else begin
      let client = Zerodds.Client.create transport in
      Zerodds.Client.write client sample;
      let deadline = Unix.gettimeofday () +. 10.0 in
      let rec wait () =
        if Unix.gettimeofday () > deadline then failwith "sync: no pong"
        else match Zerodds.Client.poll client with Some b -> b | None -> wait ()
      in
      wait ()
    end
  in
  let pong = Gen.Pong.unmarshal body Gen.Wire.LE in
  Printf.printf "PONG seq=%d reply=%s\n" pong.Gen.Pong.seq pong.Gen.Pong.reply;
  flush stdout
"#;

fn build_ocaml(mode: &str, port: u16) -> Child {
    let dir = std::env::temp_dir().join(format!("pp_ocaml_{mode}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");

    // The SDK (its own `module Wire` + `Endpoint`/`Client`/`AsyncReader`).
    let sdk = format!(
        "{}/../../endpoints/ocaml/zerodds.ml",
        env!("CARGO_MANIFEST_DIR")
    );
    let sdk_src = std::fs::read_to_string(&sdk).expect("read endpoints/ocaml/zerodds.ml");
    std::fs::write(dir.join("zerodds.ml"), &sdk_src).expect("write zerodds.ml");
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
