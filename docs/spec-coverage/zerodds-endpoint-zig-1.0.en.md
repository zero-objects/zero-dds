# `zerodds-endpoint-zig` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-zig-1.0.md` — ZeroDDS Zig endpoint SDK spec
(XRCE framing, sync `Client`, async `Reader`/`Writer`, reliable stream via
[`reliable-endpoint-1.0`](../specs/reliable-endpoint-1.0.md)). Complements the
codegen coverage `zerodds-xcdr2-zig` (`docs/spec-coverage/zerodds-xcdr2-zig-1.0.md`) —
that doc covers marshalling, this one covers transport.

Implementation:

- `endpoints/zig/` — the pure-Zig endpoint SDK (ADR 0013: from-scratch XCDR wire core,
  no C): `src/zerodds.zig` (XRCE framing, `Client`, `AsyncReader`), `src/reliable.zig`
  (reliable state machine + async-decoupled `AsyncWriter`), `example_sync.zig` /
  `example_async.zig` / `example_reliable.zig`.
- `crates/endpoint-e2e/tests/zig.rs` — the live ping-pong E2E (raw/sync/async) against
  the shared Rust XRCE peer.
- `crates/endpoint-e2e/tests/zig_reliable.rs` — the reliable E2E (loss recovery,
  lossless baseline, example, latency bench).

## §1 XRCE framing

### §1 WRITE_DATA/DATA frame (8-byte header + body)

**Spec:** 8-byte XRCE header (session, stream, seq LE, submsg id `0x07` WRITE_DATA,
flags, len LE) + body, byte-identical to `crates/xrce` + `endpoints/c`.

**Repo:** `endpoints/zig/src/zerodds.zig` — `xrceWriteFrame`/`xrceReadFrame`,
constants `XRCE_SESSION_NOKEY` (`0x80`) and `XRCE_STREAM_BEST_EFFORT` (`0x01`);
`Transport` as a function-pointer vtable (`deliver`/`receive`, no heap).

**Tests:** in-file test `byte identity vs Rust goldens` in `zerodds.zig`
(`Writer` LE+BE against `build/golden_le.bin`/`golden_be.bin`).

**Status:** done.

### §1 Raw codec without an XRCE frame (ping-pong E2E)

**Spec:** evidence that the generated sample codec (`zerodds-xcdr2-zig`) also
round-trips over a real channel without XRCE framing — the ground the framing sits on.

**Repo:** `crates/endpoint-e2e/tests/zig.rs::ZIG_RAW_MAIN` — raw UDP, no XRCE frame,
uses only `gen.zig` (the generated `Ping`/`Pong` codec).

**Tests (codepit):** `zig_raw_udp` — generated `Ping`/`Pong` codec directly over a raw
UDP socket, no XRCE framing. Passed.

**Status:** done.

## §2 sync Client

### §2 `Client.write`/`poll`

**Spec:** A blocking pull path: `write(sample)` frames + delivers through the
`Transport`, `poll()` receives a frame and returns the unframed body.

**Repo:** `endpoints/zig/src/zerodds.zig::Client` (`write`/`poll`, `seq` counter,
fixed `txbuf`/`rxbuf`, no heap).

**Tests:** in-file test `sync loopback (pull)` (in-memory FIFO transport, 5 samples
roundtrip); live E2E `crates/endpoint-e2e/tests/zig.rs::ZIG_ENDPOINT_MAIN` (mode
`sync`) via `UdpTransport` + `zerodds.zig`.

**Tests (codepit):** `zig_endpoint_sync` — full stack via `Client.write`/`poll`
against the shared Rust XRCE peer. Passed.

**Status:** done.

## §3 async Reader/Writer

### §3 `AsyncReader` (callback reactor)

**Spec:** A callback reactor (push) as the counterpart to sync `poll` — Zig has no
async/await, so the reactor dispatches each ready frame to a consumer callback.

**Repo:** `endpoints/zig/src/zerodds.zig::AsyncReader` (`on_sample` callback, `poll()`
dispatches one frame, `run(max)` drains up to `max` frames or until nothing more is
ready).

**Tests:** in-file test `async loopback (push / callback reactor)` (5 samples,
`Collector.on` callback collects IDs in order); live E2E
`crates/endpoint-e2e/tests/zig.rs::ZIG_ENDPOINT_MAIN` (mode `async`) via
`UdpTransport` + `zerodds.zig`.

**Tests (codepit):** `zig_endpoint_async` — full stack via `AsyncReader.run` against
the shared Rust XRCE peer. Passed.

**Status:** done.

**Note:** a standalone best-effort `AsyncWriter` (without reliability) does not exist
for Zig — the async-decoupled write side is covered exclusively by the reliable
`AsyncWriter` (§4) (spec §3, `docs/specs/zerodds-endpoint-zig-1.0.md`). Not an open
item, a deliberate spec decision.

**Ping-pong E2E sum (§1/§2/§3):** `zig_raw_udp`/`zig_endpoint_sync`/`zig_endpoint_async`
— 3/3 passed (codepit).

## §4 reliable stream

### §4 State machine (sender + receiver) + byte-golden HEARTBEAT/ACKNACK

**Spec:** XRCE reliable stream (`stream_id 0x80`, §8.4.10/§8.4.11), mirroring the
reference `crates/xrce/src/reliable.rs`: `Sender.submit`/`pendingHeartbeat`/
`recvAckNack`/`getInFlight` (history + retransmit); `Receiver.recvData`/`drainInto`/
`pendingAckNack`/`reset` (reorder + dedup). Window 16, receiver buffer 64, heartbeat
500 ms, payload ≤ 65535, RFC-1982 16-bit sequence numbers. HEARTBEAT (`0x0B`) and
ACKNACK (`0x0A`) byte-identical to the C SDK's reference goldens. Full contract
details in [`reliable-endpoint-1.0`](../specs/reliable-endpoint-1.0.md).

**Repo:** `endpoints/zig/src/reliable.zig` — `writeDataFrame`/`parseWriteData`,
`heartbeatFrame`/`parseHeartbeat`, `acknackFrame`/`parseAckNack`; `Sender`, `Receiver`.

**Tests (codepit):** `zig_reliable_unit` — `zig test` on `reliable.zig`: the
mirror-of-reference unit suite (monotonic seq, payload-too-large, window-full,
heartbeat first/silence/none, ACKNACK partial/full clear, receiver reorder/dedup/
buffer-full, pending-ACKNACK bitmap, reset, in-process end-to-end loss recovery) plus
the byte-golden assertion in the same `zig test` run: `heartbeatFrame(1,3)` ==
`80 00 01 00 0b 01 05 00 01 00 03 00 80`, `acknackFrame(1,0)` ==
`80 00 01 00 0a 01 05 00 01 00 00 00 80` — identical to the reference goldens.
Passed.

**Status:** done.

### §4 Async-decoupled `AsyncWriter` (SPSC ring) + loss-recovery E2E

**Spec:** The producer enqueues wait-free into an SPSC ring, a dedicated drain thread
holds the `Sender` state and does all the I/O (send, heartbeat, ACKNACK-driven
retransmit) — the producer never enters the kernel. Reliable delivery survives
datagram loss — verified live against the shared Rust peer with injected loss.

**Repo:** `endpoints/zig/src/reliable.zig::AsyncWriter` (wait-free SPSC ring
`RING_CAP=1024`/`SLOT_CAP=512` via `std.atomic.Value`, `write`/`pop`/`drainLoop`/
`drainAckNacks`/`close`); `endpoints/zig/example_reliable.zig` (runnable in-process
demo, no socket).

**Tests (codepit):**
- `zig_reliable_loss_recovery` — peer drops every 3rd sample once; the app
  retransmits on ACKNACK; all 12 samples delivered gap-free in order.
- `zig_reliable_no_loss` — lossless baseline; 12/12.
- `zig_reliable_example` — `example_reliable.zig` runs and reports
  `sequence 0..11 verified in order`.

3/3 passed (codepit) — together with `zig_reliable_unit` (§4 above) 4/5 of the
reliable test set; the latency bench is §5.

**Note (honest):** while building `AsyncWriter.close()`, a shutdown deadlock was found
and fixed in `drainLoop` — a lingering window with no incoming ACKNACKs must not block
`close()`; `drainLoop` now checks `running` on every iteration instead of only while
waiting for an ACKNACK.

**Status:** done.

## §5 Latency

### §5 SPSC-ring push vs. inline `sendto`

**Spec:** The producer path of `AsyncWriter` (`write` → ring-slot memcpy + release
store) must be measurably below the inline `sendto` syscall — the evidence that async
write removes syscall latency from the producer path, not that it waits for ACKNACK.

**Repo:** `crates/endpoint-e2e/tests/zig_reliable.rs::runBench` (Zig app, mode
`bench`): 4000 iterations of inline `sendto` vs. 4000 iterations of
`AsyncWriter.write` after 100 warmup pushes.

**Tests (codepit):** `zig_reliable_latency_bench` — producer push (wait-free ring)
**14 ns** vs. inline `sendto` **7950 ns** (~568×), asserts `decoupled_ns < inline_ns`.
Passed.

**Status:** done.

---

## Audit status

7 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run (codepit, verified): `cargo test -p zerodds-endpoint-e2e --test zig`
3/3 (ping-pong: `zig_raw_udp`/`zig_endpoint_sync`/`zig_endpoint_async`);
`--test zig_reliable` 5/5 (`zig_reliable_unit` incl. byte-golden,
`zig_reliable_loss_recovery`, `zig_reliable_no_loss`, `zig_reliable_example`,
`zig_reliable_latency_bench`); latency bench decoupled 14 ns / inline
`sendto` 7950 ns (~568×).

Open items: none.
