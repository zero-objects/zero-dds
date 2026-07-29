# `zerodds-endpoint-lua` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-lua-1.0.md` — the ZeroDDS Lua
endpoint SDK spec. Complements the codegen coverage `zerodds-xcdr2-lua`
(`docs/spec-coverage/zerodds-xcdr2-lua-1.0.md`) — that doc covers
marshalling, this one covers transport.

Implementation:

- `endpoints/lua/zerodds.lua` — XRCE framing (`writeFrame`/`readFrame`),
  sync `Client`, coroutine-based `asyncReader`.
- `endpoints/lua/reliable.lua` — reliable sender/receiver state machine +
  HEARTBEAT/ACKNACK wire codec + cooperative `AsyncWriter`.
- `endpoints/lua/reliable_app.lua` — live UDP sender app for the E2E (loss
  recovery + latency bench).
- `crates/endpoint-e2e/tests/lua.rs` — ping-pong E2E;
  `crates/endpoint-e2e/tests/lua_reliable.rs` — reliable-stream E2E.
- Gate: `lua5.4` + `luasocket` (the `socket` module) on PATH; either missing
  → loud skip, no false green.

## §1 XRCE framing

**Spec:** §1 — 8-byte XRCE header (session, stream, seq LE, submsg id
`0x07` WRITE_DATA, flags `0x03`, len LE) + body, byte-identical to
`crates/xrce` + `endpoints/c`.

**Repo:** `endpoints/lua/zerodds.lua` — `M.writeFrame`/`M.readFrame`,
constants `M.SESSION_NOKEY` (`0x80`) and `M.STREAM_BEST_EFFORT` (`0x01`).

**Tests:** no framing test isolated from the full stack; framing is
exercised live via `lua_endpoint_sync`/`lua_endpoint_async` (§4) — every
sample round-trips through `writeFrame`/`readFrame` on the wire.

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — polled, non-blocking `Client`; `Write` frames + delivers
synchronously, `Poll` is a single non-blocking receive attempt; there is no
built-in `Receive(timeout)` — deadline looping is the caller's
responsibility.

**Repo:** `endpoints/lua/zerodds.lua::Client` (`Client.new`/`Client:write`/
`Client:poll`, `session`/`stream` defaults `SESSION_NOKEY`/
`STREAM_BEST_EFFORT`, monotonically increasing `seq` starting at `1`);
`Transport` contract as a structural table `{deliver, receive}` (Lua has no
interface construct).

**Tests:** `endpoints/lua/test.lua` (sync loopback over `memTransport`,
part of the `zerodds-xcdr2-lua` coverage); live E2E `lua_endpoint_sync`
(§4).

**Status:** done.

## §3 Async `Reader` (coroutine)

**Spec:** §3 — `asyncReader` is a `coroutine.wrap` producer; each resume
makes exactly one `transport.receive()` attempt and returns the unframed
body or `nil`. There is no separate `AsyncWriter` for the best-effort path
— writing stays `Client:write`, identical for sync and async apps; a real
submit/drain split only exists for the reliable stream (§5), where it
carries a history cache and a periodic HEARTBEAT worth decoupling.

**Repo:** `endpoints/lua/zerodds.lua::M.asyncReader`.

**Tests:** `endpoints/lua/test.lua` (async loopback over `memTransport`);
live E2E `lua_endpoint_async` (§4).

**Status:** done.

## §4 Ping-pong E2E (live)

**Spec:** §5.1 — a `lua5.4` app exchanges a typed sample with the shared
Rust XRCE peer over a real `luasocket` UDP datagram: full stack (generated
`zerodds-idl-lua` types + `endpoints/lua`), both sync and async.

**Repo:** `crates/endpoint-e2e/tests/lua.rs` — `LUA_MAIN` (generated
`gen.lua` with `marshal_Ping`/`unmarshal_Pong` as globals +
`require("zerodds")`, `udpTransport` implementing `{deliver, receive}`, mode
`sync`/`async` via CLI argument, 50ms socket timeout for a non-blocking
`receive()`).

**Tests (reproduced locally this session with `lua` 5.5 instead of
`lua5.4` — `string.pack`/`coroutine` semantics are identical since Lua 5.3;
`example_sync.lua` and `example_async.lua` both ran clean with `ALL OK`, see
§6 — the live UDP ping-pong itself needs the Rust peer from
`zerodds-endpoint-e2e` and was not run against the peer in this session):

- `lua_endpoint_sync` — full stack via `zerodds.Client`.
- `lua_endpoint_async` — full stack via `zerodds.asyncReader`.

2/2 (prior CI/codepit run; see Audit Status).

**Status:** done.

## §5 Reliable stream — state machine, wire, `AsyncWriter`

**Spec:** §4 (references `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
stream (`stream_id 0x80`, §8.4.10/§8.4.11), mirroring the reference
`crates/xrce/src/reliable.rs`: `Sender:submit`/`pendingHeartbeat`/
`recvAcknack`/`getInFlight`; `Receiver:recvData`/`drainInOrder`/
`pendingAcknack`/`reset`. Window 16, receiver buffer 64, heartbeat 500 ms,
payload ≤ 65535, RFC-1982 16-bit sequence numbers. Alongside it, the
`AsyncWriter` (`push`/`drain`) — **cooperative**, not thread-decoupled (§4
of the spec, honesty paragraph; expanded in §6 of this coverage).

**Repo:** `endpoints/lua/reliable.lua` — `writeDataFrame`/`parseWriteData`,
`heartbeatFrame`/`parseHeartbeat`, `acknackFrame`/`parseAcknack`; `Sender`,
`Receiver`; `AsyncWriter` (`push`/`drain`/`pending`/`isEmpty`);
`endpoints/lua/example_reliable.lua` (runnable in-process demo, no socket);
`endpoints/lua/reliable_app.lua` (live UDP sender app for the E2E, modes
`run`/`bench`).

**Tests:**

- `lua_reliable_unit_and_golden` (`crates/endpoint-e2e/tests/lua_reliable.rs`)
  runs `lua5.4 reliable_test.lua <golden_dir>` against the golden
  HEARTBEAT/ACKNACK bytes, generated by the test itself via `zerodds-xrce`
  (the same library that would surface a wire regression). Reproduced
  locally with `lua` 5.5 instead of `lua5.4`: **48 checks**, all `ok` (`ALL
  OK`) — 44 state-machine/frame-roundtrip checks (monotonic `seq`, payload
  too large, window full, heartbeat first/silence/after period, heartbeat
  with an empty window, ACKNACK partial/full clear, receiver
  reorder/dedup/buffer-full, pending-ACKNACK bitmap, reset, in-memory
  end-to-end loss recovery, `AsyncWriter` push/drain including the window
  cap) plus 4 byte-golden checks (`byte_golden_heartbeat`,
  `byte_golden_acknack`, `golden_heartbeat_parse`, `golden_acknack_parse`)
  — HEARTBEAT `80 00 01 00 0B 01 05 00 01 00 03 00 80` and ACKNACK
  `80 00 01 00 0A 01 05 00 01 00 00 00 80`, identical to the reference
  goldens shared with the other SDKs.
- `lua_reliable_loss_recovery` — the peer drops every 3rd sample once
  (`bind_reliable_peer(Some(3))`); the app (`reliable_app.lua` mode `run`)
  retransmits on ACKNACK; all 12 samples delivered gap-free, in order
  (asserted by value and order in the test).
- `lua_reliable_no_loss` — same app with no drop, lossless baseline; 12/12.
- `lua_reliable_example` — `example_reliable.lua` runs and reports
  `RELIABLE OK: 12/12 delivered gap-free in 1 round(s), sequence 0..11
  verified in order`. Reproduced locally (`lua` 5.5): exactly this output.

4/4 in this group (latency bench in §6).

**Status:** done.

## §6 Producer latency — honestly: cooperative, not concurrent

**Spec:** §4 of the spec, honesty paragraph — `AsyncWriter:push` (table
insert) vs. an inline `sendto`, explicitly **not** evidence of thread
decoupling: stock `lua5.4` has no native OS threads; `push` and `drain` run
on the same call stack of the same OS thread (the caller loop in
`reliable_app.lua` calls `drain()` itself — see that file's comment). The
measurement only shows the call-cost difference between a table insert and
a UDP syscall — not concurrent processing.

**Repo:** `endpoints/lua/reliable_app.lua` function `runBench` — 20000
iterations of inline `udp:send` vs. 20000 iterations of `AsyncWriter`
enqueue (a plain table insert, no drain inside the timed loop); prints
`BENCH enqueue_ns=... inline_send_ns=... note=cooperative_single_os_thread_no_concurrent_drain`.

**Tests:** `lua_reliable_producer_latency` — runs `reliable_app.lua` mode
`bench`, asserts only that `BENCH` appears in the output (deliberately
**no** `enqueue < inline` hard assertion — the test itself documents this as
an "honest note", see the `lua_reliable.rs` comment).

Reproduced locally this session (`lua` 5.5, not `lua5.4`, not codepit —
a plausibility check of the mechanism, not the reference figure): 4 runs,
`enqueue_ns` 14–18, `inline_send_ns` 2829–3998 — same order of magnitude
(roughly 180–260× in this sample), the table insert clearly under the UDP
syscall, as expected for a plain call-cost difference on the same thread.
The earlier codepit/CI reference run reported enqueue ~29–32 ns / inline
~3780–4050 ns — this session did not re-run the codepit measurement, only
confirmed the local order of magnitude.

**Status:** done (as an honest call-cost figure; explicitly not declared as
evidence of concurrency — that distinction is part of the conformance here,
not an open item).

---

## Audit Status

6 done / 0 partial / 0 open / 0 n/a (informational) / 0 n/a (rejected).

Reference test run (`cargo test -p zerodds-endpoint-e2e`, gated on `lua5.4`
+ `luasocket`): `--test lua` 2/2 (`lua_endpoint_sync`,
`lua_endpoint_async`); `--test lua_reliable` 5/5
(`lua_reliable_unit_and_golden` — 48 Lua checks incl. byte-golden,
`lua_reliable_loss_recovery`, `lua_reliable_no_loss`, `lua_reliable_example`,
`lua_reliable_producer_latency`).

This session directly ran `reliable_test.lua`, `example_sync.lua`,
`example_async.lua`, `example_reliable.lua`, and the `bench` mode of
`reliable_app.lua` locally with `lua` 5.5 (not `lua5.4`, no Rust peer), and
observed the outputs cited above (48/48 checks, `ALL OK` ×2, `RELIABLE OK:
12/12 ... 1 round(s)`, `BENCH ...`). The live UDP ping-pong and the
reliable loss-recovery against the Rust peer were **not** re-run this
session (no `lua5.4` binary, no Rust peer setup available); their status
rests on the prior CI/codepit reference run.

Open items: none functional. `endpoints/lua`'s test harness binds strictly
to the binary name `lua5.4` (no fallback to other Lua-5.4-compatible binary
names) — see `docs/specs/zerodds-endpoint-lua-1.0.md` §7.
