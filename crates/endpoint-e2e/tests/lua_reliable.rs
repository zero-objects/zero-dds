// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lua reliable-stream endpoint, live: the pure-Lua reliable state machine +
//! frame codec (`endpoints/lua/reliable.lua`) driven by a cooperative
//! submit/drain sender (the loop of `endpoints/lua/reliable_app.lua`) against
//! the shared Rust reliable peer: the peer drops, the app retransmits on
//! ACKNACK, all samples arrive gap-free. The samples are the `Ping` of the
//! codec the Lua generator writes from the IDL, so the run checks IDL,
//! generated codec and runtime together. And the producer-latency comparison (one OS
//! thread: the push into the writer's table against the inline send). The unit
//! and byte-golden suite and the example run in the endpoints-gate column of Lua.
//! Gated on `lua5.4` + luasocket.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, scratch_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

/// The Lua interpreter, by the names a distribution installs it under.
///
/// This mirrors `tests/lua.rs::lua_binary`. Binding only to `lua5.4` made
/// every case here skip on a host whose Lua is Homebrew's plain `lua`,
/// while the ping-pong cases next door ran — the same runtime, reported as
/// covered in one file and unmeasured in the other. `string.pack` and the
/// coroutine semantics these scripts use are unchanged since 5.3.
fn lua_binary() -> Option<&'static str> {
    ["lua5.4", "lua", "lua5.3"]
        .into_iter()
        .find(|l| Command::new(l).arg("-v").output().is_ok())
}

/// luasocket (the `socket` module) loadable by this interpreter -- the app
/// needs real UDP; stock Lua has none. Loud skip if missing (no false green).
fn luasocket_available(lua: &str) -> bool {
    Command::new(lua)
        .arg("-e")
        .arg("require('socket')")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The gate every case takes. Prints the reason and returns `None` when the
/// interpreter or luasocket is missing.
fn lua_ready(case: &str) -> Option<&'static str> {
    let Some(lua) = lua_binary() else {
        eprintln!("SKIP {case}: no Lua interpreter on PATH (tried lua5.4, lua, lua5.3)");
        return None;
    };
    if !luasocket_available(lua) {
        eprintln!("SKIP {case}: luasocket (`socket` module) not installed for `{lua}`");
        return None;
    }
    Some(lua)
}

fn endpoints_lua() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../endpoints/lua")
        .canonicalize()
        .expect("endpoints/lua path")
}

/// Runs a script in place under `endpoints/lua` (so its own
/// `package.path = "./?.lua;" .. package.path` resolves `require("reliable")`).
fn run_lua(lua: &str, script: &str, args: &[String]) -> Child {
    let mut cmd = Command::new(lua);
    cmd.arg(script);
    for a in args {
        cmd.arg(a);
    }
    cmd.current_dir(endpoints_lua())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {lua} {script}: {e}"))
}

/// The generated `Ping`/`Pong` codec as the module `gen` (`tests/lua.rs`
/// declares the module tables the same way).
fn gen_module() -> String {
    format!(
        "local zdw = require(\"zerodds_wire\")\n\
         local M = {{}}\n\
         M._descriptors = {{}}\n\
         local ZD = {{}}\n\n\
         {}\n\n\
         return ZD\n",
        source(Language::Lua)
    )
}

/// The reliable app: the codec `codegen::source(Language::Lua)` writes from
/// the IDL encodes `{ seq = i, msg = "reliable" }`; the producer pushes each
/// into the SDK's `AsyncWriter` (`endpoints/lua/reliable.lua`) and the drain
/// loop of `reliable_app.lua` sends, heartbeats and retransmits.
const LUA_RELIABLE_APP: &str = r#"local ZD = require("gen")
local zdw = require("zerodds_wire")
local rel = require("reliable")
local socket = require("socket")

local function nowMs()
  return math.floor(socket.gettime() * 1000)
end

local function encodePing(i)
  local w = zdw.writer_new({ endianness = "le", max_align = 4,
    pid_flags = "sentinel-must-understand", highest_short_id = 16128,
    pad_declared_length = true, pad_declared_optional_length = true,
    member_must_understand = true, pad_after_value = true,
    short_length_codes = false })
  ZD.zd_enc_Ping(w, { seq = i, msg = "reliable" })
  return zdw.finish(w)
end

local port = tonumber(arg[1])
local n = tonumber(arg[2])
local udp = assert(socket.udp())
assert(udp:setpeername("127.0.0.1", port))
udp:settimeout(0.02)
local sender = rel.Sender.new()
local writer = rel.AsyncWriter.new(sender, function(frame) udp:send(frame) end, 0x0015)
for i = 0, n - 1 do
  local payload = encodePing(i)
  while not writer:push(payload) do
    writer:drain()
  end
end
local msgSeq = 1
local startMs = nowMs()
while true do
  writer:drain()
  if writer:isEmpty() and sender:inFlightCount() == 0 then break end
  if nowMs() - startMs > 20000 then break end
  local hb = sender:pendingHeartbeat(nowMs())
  if hb ~= nil then
    udp:send(rel.heartbeatFrame(rel.SESSION_NOKEY, rel.STREAM_NONE, msgSeq, hb.first, hb.last, hb.streamId))
    msgSeq = msgSeq + 1
  end
  for _ = 1, 64 do
    local data = udp:receive()
    if not data then break end
    local ack = rel.parseAcknack(data)
    if ack ~= nil then
      sender:recvAcknack(ack.firstUnacked, ack.nackLo, ack.nackHi)
    end
  end
  if sender:inFlightCount() > 0 then
    for seq, payload in sender:inFlightPairs() do
      udp:send(rel.writeDataFrame(seq, 0x0015, payload))
    end
  end
end
udp:close()
print("SENT " .. n)
"#;

fn run_loss(drop_every: Option<usize>, label: &str) {
    let Some(lua) = lua_ready(label) else {
        return;
    };
    let n = RELIABLE_SAMPLES;
    let dir = scratch_dir(&format!("lua_reliable_{}", label.replace('/', "_")));
    std::fs::write(dir.join("gen.lua"), gen_module()).expect("gen.lua");
    std::fs::write(dir.join("app.lua"), LUA_RELIABLE_APP).expect("app.lua");
    // `?.lua` for gen.lua (run from `dir`), the wire runtime (a directory
    // package, so `?/init.lua` too) and the SDK's `reliable` module.
    let rt = runtime_dir("lua");
    let lua_path = format!(
        "./?.lua;{rt}/?.lua;{rt}/?/init.lua;{ep}/?.lua;;",
        rt = rt.display(),
        ep = endpoints_lua().display()
    );
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = Command::new(lua)
        .arg("app.lua")
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .current_dir(&dir)
        .env("LUA_PATH", lua_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {lua} app.lua: {e}"));
    let delivered = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &delivered, n);
}

/// reliable-endpoint 1.0 §5 point 3: the Lua app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Lua toolchain; runs in integration-bindings-gnat-beam"]
fn reliable_5_3_lua_samples_arrive_in_order_despite_dropped_datagrams() {
    // Peer drops every 3rd distinct sample once -> the app must retransmit.
    run_loss(Some(3), "lua/loss");
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Lua producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the Lua toolchain; runs in integration-bindings-gnat-beam"]
fn reliable_5_4_lua_the_producer_enqueue_costs_less_than_the_inline_send() {
    let Some(lua) = lua_ready("lua reliable latency") else {
        return;
    };
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let child = run_lua(
        lua,
        "reliable_app.lua",
        &[peer.port.to_string(), "0".to_string(), "bench".to_string()],
    );
    let out = child.wait_with_output().expect("bench wait");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Stock lua5.4 has no OS threads: the drain runs later on the same
    // thread, so this compares the producer's push with the inline send on
    // one thread, not a concurrent drain.
    eprintln!(
        "lua reliable (cooperative, single OS thread): {}",
        stdout.trim()
    );
    zerodds_endpoint_e2e::assert_enqueue_below_inline("lua", &stdout);
}
