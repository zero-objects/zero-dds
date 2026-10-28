<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-lua` v1.0 — Lua endpoint SDK

**Status:** normative · ZeroDDS vendor spec. Implemented in `endpoints/lua/`.

Complements the codegen spec [`zerodds-xcdr2-lua`](zerodds-xcdr2-lua-1.0.md) (there
the marshalling via `string.pack`/`string.unpack`) and the endpoint SDKs of the
other languages (`endpoints/go`, `endpoints/zig`, `endpoints/nim`,
`endpoints/d`, `endpoints/c`, `endpoints/ada`, ...): the native
Lua endpoint module `zerodds` over XRCE framing, the sync `Client`, the
coroutine-based `asyncReader` and the reliable stream, so that a
`lua5.4` app exchanges an XCDR2 sample byte-identical to `crates/xrce` +
`endpoints/c` with the shared Rust peer.

Unlike the C, Go or Ada endpoints, stock `lua5.4` has **no native
operating system threads**. Wherever this spec writes "async", it means
**cooperative concurrency** throughout (coroutines, a
producer/consumer interleaving on the same OS thread) — never parallel
execution. §3 and §4 make this explicit at each point where it matters for
conformance.

## §1 XRCE framing

An 8-byte XRCE header (`session`, `stream`, `seq` LE, submessage ID `0x07`
WRITE_DATA, `flags`, `len` LE) followed by the XCDR2 sample body — byte-identical
to `crates/xrce` and `endpoints/c`:

```
[0]=session [1]=stream [2..4)=seq u16 LE [4]=submessage id [5]=flags [6..8)=len u16 LE [8..]=body
```

`endpoints/lua/zerodds.lua` (module `zerodds`, loaded via `require("zerodds")`)
MUST provide:

- `M.writeFrame(session, stream, seqNo, objectId, sample) -> string` — frames
  a sample; flags fixed at `0x01` (E flag = LE plus `DataFormat` `FORMAT_DATA`).
- `M.readFrame(frame) -> string|nil` — unframes; `nil` for a frame that is too short
  (`#frame < 12`), a wrong submessage ID (`frame:byte(5) ~= 0x07`) or a
  body that is shorter than the `BaseObjectRequest`.
- `M.frameBase(frame) -> requestId, objectId|nil` — reads the
  `BaseObjectRequest`.
- Constants `M.SESSION_NOKEY` (`0x80`, best-effort, without ClientKey) and
  `M.STREAM_BEST_EFFORT` (`0x01`).
- `M.LE` (`"<"`) / `M.BE` (`">"`) as `string.pack`/`string.unpack` endian prefixes,
  shared with the marshalling from `zerodds-xcdr2-lua`.

## §2 Sync `Client`

A polled, non-blocking client — Lua has no built-in
async I/O primitive that the SDK itself could use, so the
`Client` leaves deadline handling to the caller (cf. §5 item 1).

`endpoints/lua/zerodds.lua` MUST provide:

- A `Transport` contract as a plain Lua table
  `{ deliver = function(frame) ... end, receive = function() ... end }` —
  Lua has no interface construct, the contract is purely structural
  (duck typing); the integrator implements it for their link (e.g.
  UDP via `luasocket`). `receive()` returns `nil` if nothing is pending
  at the moment; whether and how long the underlying call blocks (e.g.
  a socket timeout) is decided by the integrator — the SDK itself never
  sleeps.
- `M.Client.new(transport) -> Client` with `session = M.SESSION_NOKEY`,
  `stream = M.STREAM_BEST_EFFORT`, `seq = 1` as defaults.
- `Client:write(sample)` — frames + delivers synchronously via the `Transport`;
  increments `seq` monotonically modulo `0x10000` per call.
- `Client:poll() -> body|nil` — exactly one non-blocking
  receive attempt: calls `transport.receive()` once, unframes on success.
  There is **no** built-in `Receive(timeout)` method as in the Go SDK; a
  caller that wants to wait up to a deadline polls in a
  loop itself (`repeat body = c:poll() until body ~= nil or <deadline reached>`,
  as in `crates/endpoint-e2e/tests/lua.rs`).

## §3 Async `Reader` (coroutine)

The idiomatic Lua async model for the read side: no goroutine/no
thread, but a coroutine that makes exactly one
`transport.receive()` attempt on each resume and returns the unframed sample body (or
`nil` if the transport is currently empty) to the caller.

`endpoints/lua/zerodds.lua` MUST provide:

- `M.asyncReader(transport) -> function` — returns a
  `coroutine.wrap` producer. Every call of the returned function value
  resumes the coroutine for exactly one `transport.receive()` attempt;
  returns either the unframed body or `nil`.

In `zerodds.lua` there is **no separate `AsyncWriter`** type for the
best-effort path: the write side is already a single, non-blocking
framing+deliver call (`Client:write`) — there is nothing that
a second, decoupled type on stock Lua could gain, as long as no
syscall-heavy producer loop is involved. Sync and async apps call
`Client:write` identically for the write path; the difference lies
solely in the read side (`Client:poll` vs. `asyncReader`). A
write-side async component with a real submit/drain split exists only
for the reliable stream (§4) — there it pays off, because a history
cache and a periodic HEARTBEAT are added there.

## §4 Reliable stream

`endpoints/lua` implements the reliable stream as an endpoint capability
according to [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — sender/
receiver state machine, HEARTBEAT/ACKNACK wire codec and an
`AsyncWriter` with a submit/drain split.

`endpoints/lua/reliable.lua` (standalone module, `require("reliable")`,
does **not** `require` `zerodds` — its `Writer`/`Reader` are
chunk-local and not reachable from outside; `reliable.lua` therefore carries
its own minimal frame codec, byte-identical to `crates/xrce` and the
other endpoint SDKs) MUST provide:

- The constants `SESSION_NOKEY=0x80`, `RELIABLE_STREAM_ID=0x80`,
  `HEARTBEAT_PERIOD_MS=500`, `SENDER_WINDOW=16`, `RECEIVER_BUFFER=64`,
  `MAX_PAYLOAD=65535`.
- The sender contract `Sender:submit(payload) -> seq|nil, err`
  (`"too_large"`/`"window_full"`), `Sender:pendingHeartbeat(nowMs)`,
  `Sender:recvAcknack(firstUnacked, nackLo, nackHi)`,
  `Sender:getInFlight(seq)`, `Sender:reset()`.
- The receiver contract `Receiver:recvData(seq, payload) -> bool`,
  `Receiver:drainInOrder() -> {seq, payload}[]`,
  `Receiver:pendingAcknack(hintLastSeen)`, `Receiver:reset()`.
- The wire codec `writeDataFrame`/`parseWriteData`,
  `heartbeatFrame`/`parseHeartbeat`, `acknackFrame`/`parseAcknack` — HEARTBEAT
  `0x0B`, ACKNACK `0x0A`, RFC 1982 16-bit sequence numbers, byte-identical to
  the reference goldens.
- An `AsyncWriter` with `push(payload) -> bool` (wait-free table insert,
  capped at `SENDER_WINDOW`) and `drain() -> n` (submits + sends everything
  buffered via the given `sendFn`).

**Honestly (binding part of this spec, not a footnote):** The
`AsyncWriter` is a **cooperative** submit/drain split, not a
thread-decoupled queue as in the Go or C SDK. On stock `lua5.4` there are
no native OS threads; `push` and `drain` run in the same call stack
of the same OS thread (the caller loop in `reliable_app.lua` calls
`writer:drain()` explicitly itself). `push` is cheaper than an inline
`sendto` because it is not a syscall (a pure table insert) — but there is
no second instance running in parallel that drains in the meantime; the
producer still drives the socket itself in the end, only via a
later, cheaper call on its own path. A latency measurement
between `push` and an inline `sendto` (§5 item 3, `reliable_app.lua`
mode `bench`) is accordingly **not evidence of concurrent decoupling**,
but of the pure call cost difference between a table insert and a
UDP syscall on the same thread.

## §5 Conformance

A Lua endpoint implementation is conformant if:

1. The full stack (generated types from `zerodds-xcdr2-lua` + `zerodds.lua`)
   exchanges a typed sample both via the sync `Client` and via
   `asyncReader` with the shared Rust XRCE peer over a real
   `luasocket` UDP datagram; the deadline loop lies with the
   caller (§2).
2. HEARTBEAT and ACKNACK frames are byte-identical to the reference goldens
   (`golden_heartbeat_le.bin`/`golden_acknack_le.bin`) and the
   reliable stream recovers datagram loss without gaps and in order (§4,
   `reliable-endpoint` v1.0 §5).
3. A latency measurement `AsyncWriter:push` vs. an inline `sendto` is taken
   — and is reported **honestly** as a call cost difference on the same OS thread,
   not as evidence of concurrency (§4).
4. If `luasocket` (the `socket` module) is missing on the `lua5.4`, the
   network-bound tests skip **loudly** (`SKIP ...` on stderr) instead of silently
   appearing green — no false green.

## §6 Examples

- Sync: `endpoints/lua/example_sync.lua` — poll loop via `Client:poll`,
  full field decode of a `Reading { id, value, label }`.
- Async: `endpoints/lua/example_async.lua` — the same telemetry, but via
  the `coroutine.wrap` `asyncReader`.
- Reliable: `endpoints/lua/example_reliable.lua` — in-process demo (no
  socket) of the loss recovery via the wire codec (`writeDataFrame`/
  `parseWriteData`), not only the pure state machine in memory.
- Quickstart: `endpoints/lua/QUICKSTART.md`.

## §7 Errata + open questions

- The live E2E tests (`crates/endpoint-e2e/tests/lua.rs`,
  `lua_reliable.rs`) call the binary strictly under the name `lua5.4`
  (`Command::new("lua5.4")`). A Lua-5.4-compatible binary with a different name
  (e.g. only `lua` or `lua5.5` on PATH) is not found and
  leads to the loud skip, not to a fallback to another binary — open
  whether a PATH alias/config override would be useful.
- Sync and async are fully implemented and byte-verified;
  reliable as well (see
  `docs/spec-coverage/zerodds-endpoint-lua-1.0.md`). No functional
  deficit open. The cooperative (not parallel) nature of `AsyncWriter`
  (§4) is a deliberate design point of this language, not an open item.
