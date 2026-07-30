// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lua endpoint ping-pong: the generated `idlc` types (`marshal_Ping` /
//! `unmarshal_Pong`) plus the `endpoints/lua` SDK (sync `Client:poll`, async
//! `asyncReader` coroutine) over XRCE framing and a real luasocket UDP datagram.
//! Gated on `lua5.4` + luasocket.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::Path;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_lua::{LuaGenOptions, generate_lua_module};

/// `lua5.4` on PATH.
fn lua_available() -> bool {
    Command::new("lua5.4").arg("-v").output().is_ok()
}

/// luasocket (the `socket` module) loadable by this `lua5.4` — the app needs
/// real UDP; stock Lua has none. Loud skip if missing (no false green).
fn luasocket_available() -> bool {
    Command::new("lua5.4")
        .arg("-e")
        .arg("require('socket')")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The generated Ping/Pong module. Its `marshal_Ping` / `unmarshal_Pong` are
/// globals (its `Writer`/`Reader`/`LE` are chunk-locals), so `require`-ing it
/// alongside the endpoint SDK's `zerodds` module never clashes.
fn gen_module() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    generate_lua_module(&ast, &LuaGenOptions::default()).expect("gen")
}

// The app: generated types marshal the Ping and decode the Pong; the endpoint
// SDK frames/deframes XRCE and owns the run-loop; luasocket carries the UDP.
const LUA_MAIN: &str = r#"require("gen") -- globals marshal_Ping / unmarshal_Pong
local z = require("zerodds")
local socket = require("socket")

local mode = arg[1]
local port = tonumber(arg[2])

-- Real UDP transport implementing the SDK's {deliver, receive} contract. A
-- 50ms socket timeout makes receive() block briefly then return nil when empty
-- (never a hot spin), exactly the nil the SDK poll/asyncReader expect.
local function udpTransport(peerPort)
  local u = assert(socket.udp())
  assert(u:setsockname("127.0.0.1", 0))
  assert(u:setpeername("127.0.0.1", peerPort))
  u:settimeout(0.05)
  return {
    deliver = function(frame) assert(u:send(frame)) end,
    receive = function()
      local data = u:receive()
      return data -- nil on timeout
    end,
  }
end

local tr = udpTransport(port)
local sample = marshal_Ping({ seq = 1, msg = "hello from app" }, z.LE)
local body
local deadline = socket.gettime() + 10

if mode == "async" then
  z.Client.new(tr):write(sample)
  local reader = z.asyncReader(tr)
  repeat body = reader() until body ~= nil or socket.gettime() > deadline
  if body == nil then error("async: no pong") end
else
  local c = z.Client.new(tr)
  c:write(sample)
  repeat body = c:poll() until body ~= nil or socket.gettime() > deadline
  if body == nil then error("sync: no pong") end
end

local pong = unmarshal_Pong(body, z.LE)
print(string.format("PONG seq=%d reply=%s", pong.seq, pong.reply))
"#;

fn build_lua_endpoint(mode: &str, port: u16) -> Child {
    let endpoints_lua = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../endpoints/lua")
        .canonicalize()
        .expect("endpoints/lua path");
    let dir = std::env::temp_dir().join(format!("pp_lua_ep_{mode}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("gen.lua"), gen_module()).expect("gen.lua");
    std::fs::write(dir.join("main.lua"), LUA_MAIN).expect("main.lua");
    // `?.lua` for gen.lua (run from `dir`), and the canonical SDK for `zerodds`.
    let lua_path = format!("./?.lua;{}/?.lua;;", endpoints_lua.display());
    Command::new("lua5.4")
        .arg("main.lua")
        .arg(mode)
        .arg(port.to_string())
        .current_dir(&dir)
        .env("LUA_PATH", lua_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lua5.4")
}

fn endpoint_case(mode: &str) {
    if !lua_available() {
        eprintln!("SKIP lua endpoint {mode}: `lua5.4` not on PATH");
        return;
    }
    if !luasocket_available() {
        eprintln!("SKIP lua endpoint {mode}: luasocket (`socket` module) not installed");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_lua_endpoint(mode, peer.port);
    ping_pong(&peer, child, &format!("lua/{mode}"));
}

#[test]
fn lua_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn lua_endpoint_async() {
    endpoint_case("async");
}
