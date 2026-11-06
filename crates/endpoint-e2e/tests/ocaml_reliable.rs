// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OCaml reliable-stream endpoint: the OCaml app is the reliable sender --
//! an async-decoupled `Writer` (Mutex/Condition `Mailbox` + a drain `Thread`
//! that owns the `Sender` state and a UDP socket: submit -> history ->
//! WRITE_DATA, HEARTBEAT tick, on-ACKNACK retransmit). The shared Rust
//! reliable peer injects loss and drives recovery. The samples are the `Ping`
//! of the codec the OCaml generator writes from the IDL, so the run checks
//! IDL, generated codec and runtime together. Plus the producer-latency
//! comparison. OCaml's unit and byte-golden suite and the in-process example
//! run in the endpoints-gate column of OCaml. Gated on `ocamlfind`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

fn ocamlfind_available() -> bool {
    Command::new("ocamlfind").arg("printconf").output().is_ok()
}

fn endpoints_ocaml() -> String {
    format!("{}/../../endpoints/ocaml", env!("CARGO_MANIFEST_DIR"))
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pp_ocaml_rel_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("mkdir");
    d
}

/// Copies the given `endpoints/ocaml` sources into `dir` (deps-first order)
/// and compiles them into one binary with `ocamlfind ocamlopt`.
fn ocaml_build(dir: &Path, bin_name: &str, srcs: &[&str]) -> PathBuf {
    let src_dir = endpoints_ocaml();
    for s in srcs {
        let content = std::fs::read_to_string(format!("{src_dir}/{s}"))
            .unwrap_or_else(|e| panic!("read endpoints/ocaml/{s}: {e}"));
        std::fs::write(dir.join(s), content).unwrap_or_else(|e| panic!("write {s}: {e}"));
    }
    let mut c = Command::new("ocamlfind");
    c.args([
        "ocamlopt",
        "-thread",
        "-package",
        "unix,threads.posix",
        "-linkpkg",
    ]);
    for s in srcs {
        c.arg(s);
    }
    c.arg("-o").arg(bin_name);
    let o = c.current_dir(dir).output().expect("run ocamlfind ocamlopt");
    assert!(
        o.status.success(),
        "ocamlfind ocamlopt failed for {srcs:?}:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    dir.join(bin_name)
}

/// The reliable app: the codec `codegen::source(Language::OCaml)` writes from
/// the IDL (`gen.ml`, calling `runtime/ocaml/zdwire.ml`) encodes
/// `{ seq = i; msg = "reliable" }`, and the SDK's async-decoupled
/// `Reliable.Writer` (`endpoints/ocaml/reliable.ml`) sends it.
const OCAML_RELIABLE_APP: &str = r#"
let encode_ping (i : int) : bytes =
  let zd_framing = { Zdwire.pid_flags = "sentinel-must-understand";
                     highest_short_id = 16128; pad_declared_length = true;
                     pad_declared_optional_length = true;
                     member_must_understand = true; pad_after_value = true } in
  let w = Zdwire.new_writer Zdwire.Little 4 zd_framing false in
  let ping : Gen.ping = { Gen.seq = Int32.of_int i; msg = "reliable" } in
  Gen.zd_enc_ping w ping;
  Zdwire.finish w

let () =
  let port = int_of_string Sys.argv.(1) in
  let n = int_of_string Sys.argv.(2) in
  let sock = Unix.socket Unix.PF_INET Unix.SOCK_DGRAM 0 in
  Unix.bind sock (Unix.ADDR_INET (Unix.inet_addr_loopback, 0));
  let peer_addr = Unix.ADDR_INET (Unix.inet_addr_loopback, port) in
  let writer = Reliable.Writer.create sock peer_addr 0x0015 in
  for i = 0 to n - 1 do
    Reliable.Writer.enqueue writer (encode_ping i)
  done;
  let drained = Reliable.Writer.wait_drained writer 20.0 in
  Reliable.Writer.stop writer;
  if not drained then begin
    Printf.eprintf "RELIABLE INCOMPLETE: %d still in-flight\n%!"
      (Reliable.Writer.in_flight_count writer);
    exit 1
  end;
  Printf.printf "SENT %d\n%!" n
"#;

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !ocamlfind_available() {
        eprintln!("SKIP {label}: ocamlfind not on PATH");
        return;
    }
    let dir = tmp_dir(label.replace('/', "_").as_str());
    std::fs::copy(
        runtime_dir("ocaml").join("zdwire.ml"),
        dir.join("zdwire.ml"),
    )
    .expect("copy zdwire.ml");
    std::fs::write(dir.join("gen.ml"), source(Language::OCaml)).expect("write gen.ml");
    std::fs::copy(
        format!("{}/reliable.ml", endpoints_ocaml()),
        dir.join("reliable.ml"),
    )
    .expect("copy reliable.ml");
    std::fs::write(dir.join("app.ml"), OCAML_RELIABLE_APP).expect("write app.ml");
    // Dependency order: the wire runtime, the generated module, the SDK's
    // reliable module, the app.
    let o = Command::new("ocamlfind")
        .args([
            "ocamlopt",
            "-thread",
            "-package",
            "unix,threads.posix",
            "-linkpkg",
            "zdwire.ml",
            "gen.ml",
            "reliable.ml",
            "app.ml",
            "-o",
            "reliable_app",
        ])
        .current_dir(&dir)
        .output()
        .expect("run ocamlfind ocamlopt");
    assert!(
        o.status.success(),
        "ocamlfind ocamlopt failed for the reliable app:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );

    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let n = RELIABLE_SAMPLES;
    let child = Command::new(dir.join("reliable_app"))
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ocaml app");

    let samples = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &samples, n);
}

/// reliable-endpoint 1.0 §5 point 3: the OCaml app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the OCaml toolchain; runs in integration-bindings-gnat-beam"]
fn reliable_5_3_ocaml_samples_arrive_in_order_despite_dropped_datagrams() {
    run_loss(Some(3), "ocaml/loss");
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the OCaml producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the OCaml toolchain; runs in integration-bindings-gnat-beam"]
fn reliable_5_4_ocaml_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !ocamlfind_available() {
        eprintln!("SKIP ocaml reliable latency bench: ocamlfind not on PATH");
        return;
    }
    let dir = tmp_dir("bench");
    let bin = ocaml_build(
        &dir,
        "reliable_bench",
        &["reliable.ml", "reliable_bench.ml"],
    );
    let o = Command::new(&bin).output().expect("run bench");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("producer latency"),
        "ocaml bench produced no figure:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    eprintln!("ocaml bench: {}", out.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("ocaml", &out);
}
