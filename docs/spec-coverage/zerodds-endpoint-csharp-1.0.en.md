# `zerodds-endpoint-csharp` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-csharp-1.0.md` — the ZeroDDS C#
endpoint SDK spec. Complements the codegen coverage `zerodds-xcdr2-csharp`
(`docs/spec-coverage/zerodds-xcdr2-csharp-1.0.md`) — that doc covers
marshalling, this one covers transport.

Implementation:

- `endpoints/csharp/Zerodds.cs` (namespace `ZeroDDS`) — XCDR2 wire core
  (`Writer`/`Reader`), XRCE framing (`Xrce.WriteFrame`/`Xrce.ReadFrame`),
  `ITransport`/`MemTransport`, sync `Client`, async `AsyncReader`.
- `endpoints/csharp/Reliable.cs` (namespace `ZeroDDS.Endpoint`) — reliable
  sender/receiver state machine + HEARTBEAT/ACKNACK wire codec +
  `AsyncReliableWriter`/`ReliableWriterHandle`.
- `crates/endpoint-e2e/tests/csharp.rs` — ping-pong E2E;
  `crates/endpoint-e2e/tests/csharp_reliable.rs` — reliable-stream E2E +
  unit/golden + latency bench.

## §1 XRCE framing

**Spec:** §1 — 8-byte XRCE header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + body, byte-identical to `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/csharp/Zerodds.cs::Xrce` — `WriteFrame`/`ReadFrame`,
constants `SessionNoKey` (`0x80`) and `StreamBestEffort` (`0x01`).

**Tests:** `crates/endpoint-e2e/tests/csharp.rs::csharp_raw_udp` (raw XCDR2
over a plain UDP socket, no XRCE frame — its own minimal harness); the
framing itself is exercised live via `csharp_endpoint_sync` and
`csharp_endpoint_async` (§4).

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blocking `Client`: `Write` frames + delivers synchronously,
`Poll` is a single non-blocking receive (`null` when nothing is queued).

**Repo:** `endpoints/csharp/Zerodds.cs::ITransport` (`Deliver`/`Receive`, the
sole integration point), `MemTransport` (a lock-based in-memory reference),
`Client` (`Client(ITransport)`, `Write`/`Poll`, monotonic `ushort` seq
counter, default session `Xrce.SessionNoKey`/`Xrce.StreamBestEffort`).

**Tests:** `endpoints/csharp/ExampleSync.cs` (5 `Reading` samples over
`MemTransport`, full field decode, prints `ALL OK`); live E2E
`csharp_endpoint_sync` (§4).

**Status:** done.

## §3 Async `Reader`

**Spec:** §3 — `AsyncReader.Stream()` returns an `IAsyncEnumerable<byte[]>`;
the consumer iterates it with `await foreach`. No separate `AsyncWriter`
type for the best-effort path — on the send side, async shares the same
`Client.Write`.

**Repo:** `endpoints/csharp/Zerodds.cs::AsyncReader`
(`AsyncReader(ITransport)`, `Stream(CancellationToken)` — a non-blocking
poll per tick, else `Task.Delay(1)`).

**Tests:** `endpoints/csharp/ExampleAsync.cs` (5 `Reading` samples,
`await foreach`, full field decode, prints `ALL OK`); live E2E
`csharp_endpoint_async` (§4).

**Status:** done.

## §4 Ping-pong E2E (live)

**Spec:** §5.1/§5.2 — a C# app exchanges a typed sample with the shared Rust
XRCE peer over a real UDP socket: once as raw generated codec with no XRCE
frame, once as the full stack (generated types + endpoint SDK) sync and
async.

**Repo:** `crates/endpoint-e2e/tests/csharp.rs` — builds a real `dotnet` app
(net8.0, a `ProjectReference` onto the real `ZeroDDS.Cdr.csproj`) that
encodes a `Ping` with the GENERATED `PingTypeSupport.Encode`, ships it via
`UdpTransport : ZeroDDS.ITransport` (sync `Client.Write`/`Poll` or async
`AsyncReader.Stream`), and decodes the `Pong` with `PongTypeSupport.Decode`;
mode selected by CLI argument (`sync`/`async`/`raw`).

**Tests (codepit, gated on `dotnet`):**
- `csharp_raw_udp` — generated `Ping`/`Pong` codec directly over a raw UDP
  socket, no XRCE framing.
- `csharp_endpoint_sync` — full stack via `ZeroDDS.Client`.
- `csharp_endpoint_async` — full stack via `ZeroDDS.AsyncReader`.

3/3 passed (codepit, `dotnet` >= .NET 8 on PATH or `~/.dotnet/dotnet`; no
`dotnet` ⇒ a loud skip, no false-green).

**Status:** done.

## §5 Reliable stream — state machine, wire, async writer

**Spec:** §4 (references `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
stream (`stream_id 0x80`, §8.4.10/§8.4.11), mirroring the reference
`crates/xrce/src/reliable.rs`: `ReliableSender.Submit`/`PendingHeartbeat`/
`RecvAckNack`/`GetInFlight`/`InFlightSeqs`; `ReliableReceiver.RecvData`/
`DrainInOrder`/`PendingAckNack`/`Reset`. Window 16, receiver buffer 64,
heartbeat 500 ms, payload ≤ 65535, RFC-1982 16-bit sequence numbers.
Alongside it, the async-decoupled `AsyncReliableWriter`: the producer
enqueues via `ReliableWriterHandle.Enqueue` wait-free onto a bounded
`System.Threading.Channels` ring (`TryWrite`, `false` on a full ring), a
dedicated drain `Thread` owns the `ReliableSender` state and the UDP
`Socket` exclusively and does all the I/O (send, heartbeat, ACKNACK-driven
retransmit) — the producer never enters the kernel.

**Repo:** `endpoints/csharp/Reliable.cs` — `ReliableWire`
(`WriteFrame`/`TryUnframeWrite`, `HeartbeatFrame`/`TryParseHeartbeat`,
`AckNackFrame`/`TryParseAckNack`, constants); `ReliableSender`,
`ReliableReceiver`; `WriterCloseState`, `ReliableWriterHandle`
(`Enqueue`/`Close`), `AsyncReliableWriter` (`Start`/`Shutdown`/`Dispose`,
drain `Thread` named `zerodds-reliable-drain`);
`endpoints/csharp/ExampleReliable.cs` (all three modes: E2E sender, bench,
in-process demo).

**Tests (codepit, gated on `dotnet`):**
- `csharp_reliable_unit_and_golden`
  (`crates/endpoint-e2e/tests/csharp_reliable.rs`) builds + runs
  `endpoints/csharp/ReliableTests.cs` (`ZeroDDS.Tests.ReliableTests.Main`) —
  a single driver with 32 `Check(...)` assertions covering: sender
  monotonic seq (`"monotonic seq 0"`/`"monotonic seq 1"`,
  `"in-flight count"`), payload-too-large (`"payload too large"`), window
  fill + window-full (`"fill window"` ×16, `"window full"`), heartbeat
  first/body/silenced/after-500ms/none (`"heartbeat fires first"`/
  `"heartbeat body"`/`"heartbeat silenced <500ms"`/`"heartbeat after 500ms"`/
  `"no heartbeat when empty"`), ACKNACK partial/full clear
  (`"acknack clears acked"`/`"seq2 retransmittable"`/
  `"acknack full clear"`), receiver in-order/reorder/duplicate-drop/
  buffer-full (`"in-order drain"`/`"expected advanced"`/
  `"reorder: only seq0"`/`"reorder: seq1+2"`/`"duplicate dropped"`/
  `"fill recv buffer"` ×64/`"recv buffer full"`), pending-ACKNACK bitmap
  (`"slot 0 missing"`/`"slot 2 missing"`/`"slot 1 present"`/
  `"slot 3 present"`), reset (`"reset clears receiver"`), in-process
  end-to-end loss recovery (`"only seq0 before recovery"`/
  `"seq1 retransmittable"`/`"seq1+2 after recovery"`), byte-golden
  (`"heartbeat byte-golden (hardcoded)"` == `80 00 01 00 0b 01 05 00 01 00
  03 00 80`, `"acknack byte-golden (hardcoded)"` == `80 00 01 00 0a 01 05 00
  01 00 00 00 80`; additionally, when the Rust goldens could be generated
  via `zerodds-endpoint-golden`, `"heartbeat byte-identical to golden
  file"`/`"acknack byte-identical to golden file"` against the real
  `golden_heartbeat_le.bin`/`golden_acknack_le.bin`). Prints `ALL OK` at 0
  failures.
- `csharp_reliable_loss_recovery` — the peer drops every 3rd sample once;
  the `ExampleReliable` app (mode `<port>`) retransmits via
  `AsyncReliableWriter` on ACKNACK; all 12 samples delivered gap-free in
  order (asserted against `Reading.Id == Index`).
- `csharp_reliable_no_loss_baseline` — lossless baseline; 12/12.
- `csharp_reliable_standalone_example` — `ExampleReliable` with no
  arguments (in-process demo: lossy receiver thread + `AsyncReliableWriter`
  sender) reports `OK: 12 samples delivered gap-free in order despite
  injected loss`.

5/5 passed (codepit), 4 of which belong to this section (latency bench in
§6).

**Status:** done.

## §6 Latency — ring enqueue vs. inline `Socket.Send`

**Spec:** §5.4 — the producer path of `AsyncReliableWriter`
(`ReliableWriterHandle.Enqueue` → channel push) must be measurably below
the inline `Socket.Send` syscall — the evidence that async write removes
syscall latency from the producer path, not that it waits for ACKNACK.

**Repo:** `endpoints/csharp/ExampleReliable.cs::Bench` — 50000 iterations of
`ReliableWriterHandle.Enqueue` (drain thread plus a separate sink thread
drain the ring/socket so `Enqueue` never hits backpressure) vs. 50000
iterations of inline `Socket.Send` (no live peer needed, loopback sink),
measured via `Stopwatch` (1 tick = 100 ns).

**Tests (codepit):** `csharp_reliable_latency_bench`
(`crates/endpoint-e2e/tests/csharp_reliable.rs`) — mode `bench`; the Rust
test only asserts the `LATENCY` line is present (no hardcoded threshold),
the measured figure is logged. Observed order of magnitude (codepit): ring
enqueue **~6–10 µs** vs. inline `Socket.Send` **~442–504 µs** (~50–70×) —
the .NET JIT/GC overhead makes the absolute numbers higher than Go/Rust,
the ratio (enqueue with no kernel transition vs. an inline syscall) remains
the same evidence.

**Status:** done.

---

## Audit Status

6 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run (codepit, verified): `cargo test -p zerodds-endpoint-e2e --test csharp`
3/3 (ping-pong: `csharp_raw_udp`/`csharp_endpoint_sync`/`csharp_endpoint_async`);
`--test csharp_reliable` 5/5 (`csharp_reliable_loss_recovery`,
`csharp_reliable_no_loss_baseline`, `csharp_reliable_unit_and_golden` — 32
inline assertions incl. byte-golden, `csharp_reliable_standalone_example`,
`csharp_reliable_latency_bench`); latency bench ring enqueue ~6–10 µs /
inline `Socket.Send` ~442–504 µs (~50–70×).

Gated on `dotnet` (>= .NET 8, PATH or `~/.dotnet/dotnet`); without the
toolchain, a loud skip, no false-green.

Open items: none.
