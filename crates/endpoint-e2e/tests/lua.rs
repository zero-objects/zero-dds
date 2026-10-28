// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lua endpoint ping-pong: the generated codec (`ZD.zd_enc_Ping` /
//! `ZD.zd_dec_Pong`, calling `runtime/lua`'s `zerodds_wire`) plus the
//! `endpoints/lua` SDK (sync `Client:poll`, async `asyncReader` coroutine) over
//! XRCE framing and a real luasocket UDP datagram. Gated on a Lua interpreter
//! plus luasocket.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{
    Language, endpoints_dir, runtime_dir, scratch_dir, source, tool_available,
};
use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong};

/// The Lua interpreter, by the names a distribution installs it under.
/// Homebrew's is plain `lua`; a Debian-style split ships `lua5.4`.
fn lua_binary() -> Option<&'static str> {
    ["lua5.4", "lua", "lua5.3"]
        .into_iter()
        .find(|l| tool_available(l, "-v"))
}

/// luasocket (the `socket` module) loadable by this interpreter — the app needs
/// real UDP; stock Lua has none. Loud skip if missing (no false green).
fn luasocket_available(lua: &str) -> bool {
    Command::new(lua)
        .arg("-e")
        .arg("require('socket')")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The generated Ping/Pong module, with the two tables its text writes into
/// and the runtime it calls.
///
/// The generator emits `M.Ping = …` and `function ZD.zd_enc_Ping(…)`; the
/// module table, its descriptor map and the codec table are the harness's to
/// declare, exactly as `wireprobe-zerodds-lua-gen` declares them. `ZD` is
/// returned so `require("gen")` hands the app the codec.
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

/// The XCDR2 little-endian writer options the peer decodes. The PL-CDR1 keys
/// never fire under plain XCDR2; they are spelled out because `writer_new`
/// reads them by name and an omitted key would be a different framing.
const LUA_WRITER_OPTS: &str = "{ endianness = \"le\", max_align = 4, \
     pid_flags = \"sentinel-must-understand\", highest_short_id = 16128, \
     pad_declared_length = true, pad_declared_optional_length = true, \
     member_must_understand = true, pad_after_value = true, \
     short_length_codes = false }";

// The app: generated types marshal the Ping and decode the Pong; the endpoint
// SDK frames/deframes XRCE and owns the run-loop; luasocket carries the UDP.
const LUA_MAIN: &str = r#"local ZD = require("gen")
local zdw = require("zerodds_wire")
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
local zd_writer = zdw.writer_new(__WRITER_OPTS__)
ZD.zd_enc_Ping(zd_writer, { seq = 1, msg = "hello from app" })
local sample = zdw.finish(zd_writer)
local body
local deadline = socket.gettime() + 60

if mode == "async" then
  z.Client.new(tr, z.objectId(0x001, z.OBJK_DATAWRITER)):write(sample)
  local reader = z.asyncReader(tr)
  repeat body = reader() until body ~= nil or socket.gettime() > deadline
  if body == nil then error("async: no pong") end
else
  local c = z.Client.new(tr, z.objectId(0x001, z.OBJK_DATAWRITER))
  c:write(sample)
  repeat body = c:poll() until body ~= nil or socket.gettime() > deadline
  if body == nil then error("sync: no pong") end
end

-- `reader_new` takes its settings positionally, unlike `writer_new`'s table.
local zd_reader = zdw.reader_new(body, "le", 4, "sentinel-must-understand", 16128)
local pong = ZD.zd_dec_Pong(zd_reader, "discard")
print(string.format("PONG seq=%d reply=%s", pong.seq, pong.reply))
"#;

fn build_lua_endpoint(lua: &str, mode: &str, port: u16) -> Child {
    let dir = scratch_dir(&format!("lua_ep_{mode}"));
    std::fs::write(dir.join("gen.lua"), gen_module()).expect("gen.lua");
    std::fs::write(
        dir.join("main.lua"),
        LUA_MAIN.replace("__WRITER_OPTS__", LUA_WRITER_OPTS),
    )
    .expect("main.lua");
    // `?.lua` for gen.lua (run from `dir`), the wire runtime for
    // `zerodds_wire`, and the SDK for `zerodds`. `zerodds_wire` is a directory
    // package, so `?/init.lua` is on the path beside `?.lua`.
    let rt = runtime_dir("lua");
    let lua_path = format!(
        "./?.lua;{rt}/?.lua;{rt}/?/init.lua;{ep}/?.lua;;",
        rt = rt.display(),
        ep = endpoints_dir("lua").display()
    );
    Command::new(lua)
        .arg("main.lua")
        .arg(mode)
        .arg(port.to_string())
        .current_dir(&dir)
        .env("LUA_PATH", lua_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lua")
}

fn endpoint_case(mode: &str) {
    let Some(lua) = lua_binary() else {
        eprintln!("SKIP lua endpoint {mode}: no Lua interpreter on PATH");
        return;
    };
    if !luasocket_available(lua) {
        eprintln!("SKIP lua endpoint {mode}: luasocket (`socket` module) not installed");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_lua_endpoint(lua, mode, peer.port);
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
