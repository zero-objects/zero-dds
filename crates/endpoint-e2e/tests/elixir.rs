// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Elixir endpoint ping-pong: raw generated codegen over UDP, plus the full
//! stack (generated `Zdgen.*` types + `ZeroDDS.*` endpoint SDK, sync `Client`
//! and async `AsyncReader` process/mailbox) with XRCE framing. Gated on
//! `elixir` on PATH. The generated module (`Zdgen.Wire`/`Zdgen.Ping`/…) and the
//! SDK (`ZeroDDS.Wire`/`ZeroDDS.Client`/…) live in disjoint namespaces, so both
//! are loaded side by side with no clash: the SDK via `elixir -r <lib>`, the
//! generated module inlined into the app script.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{
    Language, endpoints_dir, runtime_dir, scratch_dir, source, tool_available,
};
use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong, xcdr2_reader, xcdr2_writer};

fn elixir_available() -> bool {
    tool_available("elixir", "--version")
}

/// Absolute path to the canonical Elixir endpoint SDK (`endpoints/elixir`).
fn sdk_path() -> std::path::PathBuf {
    endpoints_dir("elixir").join("lib/zerodds.ex")
}

/// Every `.ex` of `runtime/elixir`, which `elixir -r` loads before the script.
/// `zerodds_wire.ex` first: the sub-modules under `zerodds_wire/` are compiled
/// in whatever order the directory yields, and the parent defines the structs
/// they reference.
fn runtime_sources() -> Vec<std::path::PathBuf> {
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

/// The generated `Ping`/`Pong` structs and the `ZdCodec` module.
fn gen_module() -> String {
    source(Language::Elixir)
}

/// The XCDR2 little-endian framing both directions use.
const ELIXIR_FRAMING: &str = "zd_framing = %ZeroddsWire.PlCdr1Framing{\n\
     \x20 pid_flags: :\"sentinel-must-understand\",\n\
     \x20 highest_short_id: 16128,\n\
     \x20 pad_declared_length: true,\n\
     \x20 pad_declared_optional_length: true,\n\
     \x20 member_must_understand: true,\n\
     \x20 pad_after_value: true\n\
     }\n";

/// Writes `<generated> + <main>` to a temp `.exs` and spawns
/// `elixir -r <sdk> <script> <args...>`.
fn spawn_app(tag: &str, main: &str, args: &[&str]) -> Child {
    let dir = scratch_dir(&format!("elixir_{tag}"));
    // The generated structs go in their own file, required before the script:
    // a struct cannot be used in the same compilation context that defines it,
    // so `%Ping{}` in the script needs `Ping` to have been compiled already.
    let gen_file = dir.join("gen.exs");
    std::fs::write(&gen_file, gen_module()).expect("write gen.exs");
    let script = dir.join("app.exs");
    std::fs::write(&script, main.replace("__FRAMING__", ELIXIR_FRAMING)).expect("write app.exs");
    let mut cmd = Command::new("elixir");
    for rt in runtime_sources() {
        cmd.arg("-r").arg(rt);
    }
    cmd.arg("-r")
        .arg(sdk_path())
        .arg("-r")
        .arg(&gen_file)
        .arg(&script);
    for a in args {
        cmd.arg(a);
    }
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn elixir")
}

// ---- 1) raw codegen over a plain UDP socket ----

const ELIXIR_RAW_MAIN: &str = r#"
[port] = System.argv()
{:ok, sock} = :gen_udp.open(0, [:binary, active: false])
__FRAMING__
zd_w = ZdCodec.zd_enc_Ping(ZeroddsWire.Writer.new(:le, 4, zd_framing, false), %Ping{seq: 1, msg: "hello from app"})
sample = ZeroddsWire.Writer.finish(zd_w)
:ok = :gen_udp.send(sock, {127, 0, 0, 1}, String.to_integer(port), sample)
{:ok, {_ip, _p, data}} = :gen_udp.recv(sock, 0, 90_000)
{pong, _} = ZdCodec.zd_dec_Pong(ZeroddsWire.Reader.new(data, :le, 4, zd_framing), :discard)
IO.puts("PONG seq=#{pong.seq} reply=#{pong.reply}")
"#;

#[test]
fn elixir_raw_udp() {
    if !elixir_available() {
        eprintln!("SKIP elixir_raw_udp: `elixir` not on PATH");
        return;
    }
    // The raw app sends a bare XCDR2 Ping (no XRCE frame); run a minimal peer.
    use std::net::UdpSocket;
    use std::time::Duration;
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = spawn_app("raw", ELIXIR_RAW_MAIN, &[&port.to_string()]);
    sock.set_read_timeout(Some(Duration::from_secs(30)))
        .expect("timeout");
    let mut buf = [0u8; 4096];
    let (n, app) = sock.recv_from(&mut buf).expect("recv");
    let mut r = xcdr2_reader(&buf[..n]);
    let seq = r.read_u32().expect("seq");
    let msg = r.read_string().expect("msg");
    assert_eq!((seq, msg.as_str()), (1, "hello from app"));
    let mut w = xcdr2_writer();
    w.write_u32(seq);
    w.write_string(&format!("pong:{msg}"));
    sock.send_to(&w.finish(), app).expect("send");
    let out = child.wait_with_output().expect("wait");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("PONG seq=1 reply=pong:hello from app"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---- 2/3) full stack: generated types + endpoint SDK (sync/async) over XRCE ----
//
// A real `:gen_udp` transport (`%{deliver, receive}`) feeds the SDK. `receive`
// is non-blocking (10 ms recv timeout; any error — including a transient
// `:not_owner` during the async control handoff — maps to `nil` and retries).
// sync: `ZeroDDS.Client` polls in-process. async: `ZeroDDS.AsyncReader` owns the
// socket and pushes `{:zerodds_sample, body}` to our mailbox (BEAM idiom).

const ELIXIR_ENDPOINT_MAIN: &str = r#"
[mode, port] = System.argv()
peer_port = String.to_integer(port)
{:ok, sock} = :gen_udp.open(0, [:binary, active: false])

recv_fun = fn ->
  case :gen_udp.recv(sock, 0, 10) do
    {:ok, {_ip, _p, data}} -> data
    {:error, _} -> nil
  end
end

transport = %{
  deliver: fn frame ->
    :ok = :gen_udp.send(sock, {127, 0, 0, 1}, peer_port, frame)
    true
  end,
  receive: recv_fun
}

__FRAMING__
zd_w = ZdCodec.zd_enc_Ping(ZeroddsWire.Writer.new(:le, 4, zd_framing, false), %Ping{seq: 1, msg: "hello from app"})
sample = ZeroddsWire.Writer.finish(zd_w)

body =
  case mode do
    "async" ->
      # Write while the main process still owns the socket, then hand the
      # socket to the reader process and wait for the sample in our mailbox.
      c = ZeroDDS.Client.new(
          transport,
          ZeroDDS.Endpoint.object_id(0x001, ZeroDDS.Endpoint.objk_datawriter())
        )
      ZeroDDS.Client.write(c, sample)
      reader = ZeroDDS.AsyncReader.start(transport, self())
      :ok = :gen_udp.controlling_process(sock, reader)

      b =
        receive do
          {:zerodds_sample, body} -> body
        after
          90_000 -> raise("async: no pong")
        end

      ZeroDDS.AsyncReader.stop(reader)
      b

    _ ->
      c = ZeroDDS.Client.new(
          transport,
          ZeroDDS.Endpoint.object_id(0x001, ZeroDDS.Endpoint.objk_datawriter())
        )
      c = ZeroDDS.Client.write(c, sample)
      deadline = System.monotonic_time(:millisecond) + 90_000

      poll = fn poll ->
        case ZeroDDS.Client.poll(c) do
          nil ->
            if System.monotonic_time(:millisecond) > deadline do
              raise("sync: no pong")
            else
              poll.(poll)
            end

          b ->
            b
        end
      end

      poll.(poll)
  end

{pong, _} = ZdCodec.zd_dec_Pong(ZeroddsWire.Reader.new(body, :le, 4, zd_framing), :discard)
IO.puts("PONG seq=#{pong.seq} reply=#{pong.reply}")
"#;

fn endpoint_case(mode: &str) {
    if !elixir_available() {
        eprintln!("SKIP elixir endpoint {mode}: `elixir` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = spawn_app(mode, ELIXIR_ENDPOINT_MAIN, &[mode, &peer.port.to_string()]);
    ping_pong(&peer, child, &format!("elixir/{mode}"));
}

#[test]
fn elixir_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn elixir_endpoint_async() {
    endpoint_case("async");
}
