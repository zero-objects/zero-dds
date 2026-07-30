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

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_elixir::{ElixirGenOptions, generate_elixir_module};

fn elixir_available() -> bool {
    Command::new("elixir").arg("--version").output().is_ok()
}

/// Absolute path to the canonical Elixir endpoint SDK (`endpoints/elixir`).
fn sdk_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../endpoints/elixir/lib/zerodds.ex")
        .canonicalize()
        .expect("endpoints/elixir/lib/zerodds.ex")
}

/// Generated `Zdgen.Ping`/`Zdgen.Pong` module source.
fn gen_module() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    generate_elixir_module(&ast, &ElixirGenOptions::default()).expect("gen")
}

/// Writes `<generated> + <main>` to a temp `.exs` and spawns
/// `elixir -r <sdk> <script> <args...>`.
fn spawn_app(tag: &str, main: &str, args: &[&str]) -> Child {
    let mut src = gen_module();
    src.push('\n');
    src.push_str(main);
    let dir = std::env::temp_dir().join(format!("pp_elixir_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let script = dir.join("app.exs");
    std::fs::write(&script, &src).expect("write");
    let mut cmd = Command::new("elixir");
    cmd.arg("-r").arg(sdk_path()).arg(&script);
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
ping = struct(Zdgen.Ping, seq: 1, msg: "hello from app")
sample = Zdgen.Ping.marshal_xcdr(ping, :little)
:ok = :gen_udp.send(sock, {127, 0, 0, 1}, String.to_integer(port), sample)
{:ok, {_ip, _p, data}} = :gen_udp.recv(sock, 0, 30_000)
pong = Zdgen.Pong.unmarshal(data, :little)
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
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = spawn_app("raw", ELIXIR_RAW_MAIN, &[&port.to_string()]);
    sock.set_read_timeout(Some(Duration::from_secs(30)))
        .expect("timeout");
    let mut buf = [0u8; 4096];
    let (n, app) = sock.recv_from(&mut buf).expect("recv");
    let mut r = BufferReader::new(&buf[..n], Endianness::Little).xcdr2();
    let seq = r.read_u32().expect("seq");
    let msg = r.read_string().expect("msg");
    assert_eq!((seq, msg.as_str()), (1, "hello from app"));
    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    w.write_u32(seq).expect("s");
    w.write_string(&format!("pong:{msg}")).expect("r");
    sock.send_to(&w.into_bytes(), app).expect("send");
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

ping = struct(Zdgen.Ping, seq: 1, msg: "hello from app")
sample = Zdgen.Ping.marshal_xcdr(ping, :little)

body =
  case mode do
    "async" ->
      # Write while the main process still owns the socket, then hand the
      # socket to the reader process and wait for the sample in our mailbox.
      c = ZeroDDS.Client.new(transport)
      ZeroDDS.Client.write(c, sample)
      reader = ZeroDDS.AsyncReader.start(transport, self())
      :ok = :gen_udp.controlling_process(sock, reader)

      b =
        receive do
          {:zerodds_sample, body} -> body
        after
          20_000 -> raise("async: no pong")
        end

      ZeroDDS.AsyncReader.stop(reader)
      b

    _ ->
      c = ZeroDDS.Client.new(transport)
      c = ZeroDDS.Client.write(c, sample)
      deadline = System.monotonic_time(:millisecond) + 20_000

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

pong = Zdgen.Pong.unmarshal(body, :little)
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
