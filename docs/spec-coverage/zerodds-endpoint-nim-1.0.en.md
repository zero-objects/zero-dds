# `zerodds-endpoint-nim` 1.0 — Spec-Coverage

**Source:** `docs/specs/zerodds-endpoint-nim-1.0.md` — the ZeroDDS Nim
endpoint SDK spec (XRCE framing, sync `Client`, async `AsyncReader`, reliable
stream).

**Implementation:**

- `endpoints/nim/zerodds.nim` — `Writer`/`Reader` (XCDR2, alignment cap 4),
  `writeFrame`/`readFrame` (XRCE best-effort stream 0x01), `Client` (sync),
  `AsyncReader` (async).
- `endpoints/nim/reliable.nim` — sender/receiver state machine + HEARTBEAT/
  ACKNACK frame codec (reliable stream 0x80).
- `endpoints/nim/reliable_app.nim` — Channel-based drain thread (producer
  enqueues, a drain thread owns the socket + sender state).
- `endpoints/nim/example_reliable.nim` — in-process demo of loss recovery.
- `crates/endpoint-e2e/tests/nim.rs`, `crates/endpoint-e2e/tests/nim_reliable.rs`
  — live E2E against the Rust XRCE peer.

## §1 XRCE framing

**Spec:** §1 — XRCE `WRITE_DATA` frame: an 8-byte header (session, stream, seq
LE, submessage id, flags, length LE) + payload, byte-identical to
`crates/xrce` and every other endpoint SDK.

**Repo:** `endpoints/nim/zerodds.nim::writeFrame`/`readFrame` (best-effort,
session `0x80`, stream `0x01`); `endpoints/nim/reliable.nim::reliableWriteFrame`
(reliable, stream `0x80`) — identical byte layout, different stream id.

**Tests:** `endpoints/nim/test.nim` — the marks `"sync loopback: 5 samples OK"`,
`"async loopback: 5 samples OK"` and `"negative frame vectors: OK"`
(byte identity of the wire core, CI job `endpoints-nim`); the frame roundtrip live in §2/§3/§4.

**Status:** done.

## §2 Sync Client

**Spec:** §2 — A non-blocking poll client: `write` frames+sends, `poll` reads
non-blocking and returns `Option[seq[byte]]`.

**Repo:** `endpoints/nim/zerodds.nim::Client`/`newClient`/`write`/`poll`
(monotonic `seqNo`, wraps at `0xffff`).

**Tests:** `xrce_8_3_5_8_nim_sync_app_writes_the_ping_and_reads_the_pong` (`crates/endpoint-e2e/tests/nim.rs`) — a live
poll loop against the Rust peer: an app built from generated `idlc` types
(gen.nim) plus the endpoint SDK exchanges a typed sample with the shared
Rust XRCE peer over a real UDP socket
(the helper build_nim_endpoint in `crates/endpoint-e2e/tests/nim.rs`
compiles gen.nim + a
copy of `zerodds.nim` + `main.nim`, spawns the app).

**Status:** done.

## §3 Async Reader

**Spec:** §3 — The idiomatic Nim `async`/`await` path: `recv` returns a
`Future[seq[byte]]` that resolves only once the deframed sample is ready.

**Repo:** `endpoints/nim/zerodds.nim::AsyncReader`/`newAsyncReader`/`recv`
(an `{.async.}` proc, polls via `sleepAsync(1)` until a frame is pending).

**Tests:** `xrce_8_3_5_8_nim_async_app_writes_the_ping_and_reads_the_pong` (`crates/endpoint-e2e/tests/nim.rs`) —
`waitFor fut.withTimeout(10000)` against the same generated app; `Client.write`
sends in both modes (the SDK only has a sync writer), the async path exercises
purely the `AsyncReader` `Future`.

**Tests:** (Linux) `xrce_8_3_5_8_nim_sync_app_writes_the_ping_and_reads_the_pong`, `xrce_8_3_5_8_nim_async_app_writes_the_ping_and_reads_the_pong` — both green.

**Status:** done.

## §4 Reliable stream

**Spec:** §4 — XRCE reliable stream (`stream_id >= 128`, §8.4.10/§8.4.11),
mirrored from the reference `crates/xrce/src/reliable.rs`: sender `submit`/
`pendingHeartbeat`/`recvAcknack`/`getInFlight`; receiver `recvData`/
`drainInOrder`/`pendingAcknack`/`reset`. Window 16, receiver buffer 64,
heartbeat 500 ms, payload <= 65535, RFC-1982 16-bit sequence numbers. The
async writer is built as a Channel-drain thread: the producer enqueues
lock-based (Nim stdlib `Channel`, not a wait-free ring), a dedicated drain
thread owns the socket and the `Sender` state (sends WRITE_DATA, periodic
HEARTBEAT, receives ACKNACK, retransmits from `inFlight`).

**Repo:** `endpoints/nim/reliable.nim` (state machine + frame codec);
`endpoints/nim/reliable_app.nim::drain` (drain thread over `gChan.tryRecv`);
`endpoints/nim/reliable_test.nim` (unit + byte-golden);
`endpoints/nim/example_reliable.nim` (in-process demo, no socket).

**Tests:** (Linux)

- `reliable_5_3_nim_samples_arrive_in_order_despite_dropped_datagrams` (`crates/endpoint-e2e/tests/nim_reliable.rs`) —
  12 samples, the peer drops every 3rd datagram; the app retransmits on
  ACKNACK; all 12 delivered gap-free in order.
- `endpoints/nim/reliable_test.nim` in the `nim` column of `tools/endpoints-gate`
  (job `integration-bindings-zig-nim`) — compiles and runs `reliable_test.nim`;
  22 mirrored state-machine checks (sender: monotonic seq, payload-too-large,
  window-full, heartbeat first/silence/after-period/empty, ACKNACK clear
  partial/full; receiver: in-order, reorder block+delivery, duplicate-drop,
  buffer-full, ACKNACK bitmap, reset; plus 3 end-to-end loss-recovery checks)
  + 4 byte-golden checks (`endpointnim_4_byte_golden_heartbeat`, `endpointnim_4_byte_golden_acknack`
  against `golden_heartbeat_le.bin`/`golden_acknack_le.bin`, plus a parse
  round-trip of both goldens) — `ALL OK`.
- `reliable_5_4_nim_the_producer_enqueue_costs_less_than_the_inline_send` — the bench mode of `reliable_app.nim`:
  producer `enqueue`->return via Channel vs. inline `sendto` on the same
  thread (measurement in §5).

**Status:** done.

**Honestly noted:** Nim's stdlib `Channel` is lock-based (mutex + copy), not a
wait-free SPSC ring; the decoupling from the send syscall is real, a true ring
would push the enqueue figure lower still (comment in `reliable_app.nim`). The
reference spec (§4) does not mandate the concrete queue implementation.

## §5 Latency

**Spec:** §4 — A measurement that shows the producer decoupling from the
socket syscall (`reliable-endpoint` §5 item 4, latency bench).

**Repo:** `endpoints/nim/reliable_app.nim::runBench` — 20000 iterations per
path; `inline`: `sock.send` directly on the producer thread; `decoupled`:
`gChan.send` into a Channel drained by a separate thread.

**Tests:** (Linux) `reliable_5_4_nim_the_producer_enqueue_costs_less_than_the_inline_send` — enqueue **192 ns** vs.
inline `sendto` **3926 ns** (~20x).

**Status:** done — enqueue 192 ns against inline `sendto` at 3926 ns, which is
the decoupling §4 asks for. The spec does not prescribe the queue
implementation; a wait-free ring would be an optimisation and is not built.

---

## Audit-Status

5 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-nim-1.0.md`):

* `python3 ci/run_integration.py integration-bindings-zig-nim` (job `integration-bindings-zig-nim`) — 9 tests green.

9 cited tests green, 0 not green.

Open items: none. The tests of `zerodds-endpoint-e2e` (ping-pong + reliable)
run in the job `integration-bindings-zig-nim`. A true wait-free SPSC ring
instead of the lock-based `Channel` remains a possible further latency
reduction (not an open spec item, see §4).
