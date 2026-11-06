// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Elixir reliable-stream endpoint, live: the pure-Elixir reliable state
//! machine (`endpoints/elixir/lib/reliable.ex`, module `ZeroDDS.Reliable`)
//! driven by its decoupled `ZeroDDS.Reliable.Drain` GenServer against the
//! shared Rust reliable peer: the peer drops, the app retransmits on ACKNACK,
//! all samples arrive gap-free. The samples are the `Ping` of the codec the
//! Elixir generator writes from the IDL, so the run checks IDL, generated
//! codec and runtime together. And the producer-latency comparison of
//! `reliable_app.exs`. The unit and byte-golden suite and the example
//! run in the endpoints-gate column of Elixir. Gated on `elixir` on PATH.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, scratch_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

fn elixir_available() -> bool {
    Command::new("elixir").arg("--version").output().is_ok()
}

fn endpoints_elixir() -> PathBuf {
    PathBuf::from(format!(
        "{}/../../endpoints/elixir",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// Absolute path to the reliable-stream SDK module (`ZeroDDS.Reliable` +
/// `ZeroDDS.Reliable.Drain`).
fn reliable_lib() -> PathBuf {
    endpoints_elixir()
        .join("lib/reliable.ex")
        .canonicalize()
        .expect("endpoints/elixir/lib/reliable.ex")
}

fn elixir_cmd(script: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new("elixir");
    cmd.current_dir(endpoints_elixir());
    cmd.arg("-r").arg(reliable_lib()).arg(script);
    for a in args {
        cmd.arg(a);
    }
    cmd
}

/// Spawns `elixir -r lib/reliable.ex <script> <args...>` (for the live E2E
/// cases, where the caller drives a peer against the running child).
fn spawn_elixir(script: &str, args: &[&str]) -> Child {
    elixir_cmd(script, args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn elixir")
}

/// Every `.ex` of `runtime/elixir`, `zerodds_wire.ex` first (the parent
/// defines the structs its sub-modules reference; `tests/elixir.rs`).
fn runtime_sources() -> Vec<PathBuf> {
    let lib = runtime_dir("elixir").join("lib");
    let mut out = vec![lib.join("zerodds_wire.ex")];
    let mut rest: Vec<_> = std::fs::read_dir(lib.join("zerodds_wire"))
        .expect("runtime/elixir/lib/zerodds_wire")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ex"))
        .collect();
    rest.sort();
    out.extend(rest);
    out
}

/// The reliable app: the codec `codegen::source(Language::Elixir)` writes from
/// the IDL (`gen.exs`, calling `runtime/elixir`) encodes
/// `%Ping{seq: i, msg: "reliable"}`, and the SDK's async-decoupled
/// `ZeroDDS.Reliable.Drain` sends it.
const ELIXIR_RELIABLE_APP: &str = r#"
alias ZeroDDS.Reliable.Drain

[port_s, n_s] = System.argv()
port = String.to_integer(port_s)
n = String.to_integer(n_s)
zd_framing = %ZeroddsWire.PlCdr1Framing{
  pid_flags: :"sentinel-must-understand",
  highest_short_id: 16128,
  pad_declared_length: true,
  pad_declared_optional_length: true,
  member_must_understand: true,
  pad_after_value: true
}
{:ok, socket} = :gen_udp.open(0, [:binary, active: false])
:ok = :gen_udp.connect(socket, {127, 0, 0, 1}, port)
{:ok, drain} = Drain.start_link(socket, 0x0015)
:ok = :gen_udp.controlling_process(socket, drain)
:ok = Drain.activate(drain)
for i <- 0..(n - 1) do
  zd_w = ZdCodec.zd_enc_Ping(ZeroddsWire.Writer.new(:le, 4, zd_framing, false), %Ping{seq: i, msg: "reliable"})
  Drain.submit(drain, ZeroddsWire.Writer.finish(zd_w))
end
:ok = Drain.finish(drain)
GenServer.stop(drain)
IO.puts("SENT #{n}")
"#;

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !elixir_available() {
        eprintln!("SKIP {label}: `elixir` not on PATH");
        return;
    }
    let n = RELIABLE_SAMPLES;
    let dir = scratch_dir(&format!("elixir_reliable_{}", label.replace('/', "_")));
    // The structs in a file of their own, required before the script: a
    // struct cannot be used in the compilation context that defines it.
    let gen_file = dir.join("gen.exs");
    std::fs::write(&gen_file, source(Language::Elixir)).expect("write gen.exs");
    let script = dir.join("app.exs");
    std::fs::write(&script, ELIXIR_RELIABLE_APP).expect("write app.exs");
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let mut cmd = Command::new("elixir");
    for rt in runtime_sources() {
        cmd.arg("-r").arg(rt);
    }
    let child = cmd
        .arg("-r")
        .arg(reliable_lib())
        .arg("-r")
        .arg(&gen_file)
        .arg(&script)
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn elixir reliable app");
    let delivered = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &delivered, n);
}

/// reliable-endpoint 1.0 §5 point 3: the Elixir app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Elixir toolchain; runs in integration-bindings-gnat-beam"]
fn reliable_5_3_elixir_samples_arrive_in_order_despite_dropped_datagrams() {
    run_loss(Some(3), "elixir/loss");
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Elixir producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the Elixir toolchain; runs in integration-bindings-gnat-beam"]
fn reliable_5_4_elixir_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !elixir_available() {
        eprintln!("SKIP elixir reliable producer latency: `elixir` not on PATH");
        return;
    }
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let child = spawn_elixir("reliable_app.exs", &[&peer.port.to_string(), "0", "bench"]);
    let out = child.wait_with_output().expect("wait bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("elixir reliable {}", stdout.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("elixir", &stdout);
}
