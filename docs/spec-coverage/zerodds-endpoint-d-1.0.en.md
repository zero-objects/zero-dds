# `zerodds-endpoint-d` 1.0 — Spec Coverage

**Source:** [`docs/specs/zerodds-endpoint-d-1.0.md`](../specs/zerodds-endpoint-d-1.0.md)
— ZeroDDS D endpoint SDK spec (§1 XRCE framing, §2 sync Client, §3 async
Reader/Writer, §4 reliable stream via
[`reliable-endpoint-1.0`](../specs/reliable-endpoint-1.0.md)). Covers the
D endpoint SDK `endpoints/d/`: `zerodds.d` (XRCE framing, sync `Client`,
async `AsyncReader` via `std.concurrency`) and the self-contained
`reliable.d` (state machine + wire codec + lock-free `SpscRing`), plus the
live Ping-Pong E2E. Complements the codegen coverage `zerodds-xcdr2-d`.

**Implementation:** `endpoints/d/zerodds.d`, `endpoints/d/reliable.d`,
`endpoints/d/reliable_app.d`, `endpoints/d/example_reliable.d`,
`endpoints/d/reliable_test.d`, `endpoints/d/reliable_bench.d`;
E2E peer `crates/endpoint-e2e/tests/d.rs` + `d_reliable.rs`.

## §1 XRCE Framing

**Spec:** `zerodds-endpoint-d-1.0` §1 — 8-byte WRITE_DATA header (session,
stream, seq LE, submsg id `0x07`, flags `0x03`, len LE) + body; best-effort
stream `0x01`.

**Repo:** `zerodds.d` — `writeFrame`/`readFrame` (line 136–151), constants
`SessionNoKey = 0x80`, `StreamBestEffort = 0x01` (line 133–134).

**Tests:** exercised indirectly via the Ping-Pong E2E (§4) — `xrce_8_3_5_8_d_sync_app_writes_the_ping_and_reads_the_pong`
and `xrce_8_3_5_8_d_async_app_writes_the_ping_and_reads_the_pong` frame/deframe every sample through this code path.

**Status:** done.

## §2 Sync Client

**Spec:** `zerodds-endpoint-d-1.0` §2 — A blocking-poll `Client` over a
swappable `Transport` (deliver/receive delegates); the sequence number wraps
modulo 0x10000.

**Repo:** `zerodds.d` — `class Client` (line 162–182), `struct Transport`
(line 155–158), `memTransport()` as an in-memory FIFO for tests/examples.

**Tests:** (Linux) `xrce_8_3_5_8_d_sync_app_writes_the_ping_and_reads_the_pong` (Ping-Pong E2E, §4).

**Status:** done.

## §3 Async Reader

**Spec:** `zerodds-endpoint-d-1.0` §3 — A background actor via
`std.concurrency` (`spawn`/`send`/`receive`, Tid message passing) — the
idiomatic D concurrency primitive, analogous to Ada's protected object/task
and Rust's channel. At this layer only an async **Reader** exists; a
decoupled async **Writer** is part of the reliable stream (`SpscRing`, §5) —
the sync `Client` sends inline.

**Repo:** `zerodds.d` — `class AsyncReader` (line 214–222), `readerLoop`
(line 201–212).

**Tests:** (Linux) `xrce_8_3_5_8_d_async_app_writes_the_ping_and_reads_the_pong` (Ping-Pong E2E, §4).

**Status:** done.

## §4 Ping-Pong E2E (live)

**Spec:** no dedicated section in `zerodds-endpoint-d-1.0` — repo-internal
live proof for §1–§3 together: three live UDP tests against the shared Rust
XRCE peer: generated codegen over bare UDP (no XRCE frame), full stack via
the sync `Client` (§2), full stack via the async `AsyncReader` (§3).

**Repo:** `crates/endpoint-e2e/tests/d.rs`.

**Tests:** (Linux) `endpoint_c_3_4_d_generated_codec_pings_without_the_xrce_frame` + `xrce_8_3_5_8_d_sync_app_writes_the_ping_and_reads_the_pong` + `xrce_8_3_5_8_d_async_app_writes_the_ping_and_reads_the_pong`
(`crates/endpoint-e2e/tests/d.rs`, job `integration-bindings`).

**Status:** done.

## §5 Reliable Stream — State Machine + async-decoupled Writer

**Spec:** `zerodds-endpoint-d-1.0` §4, referencing `reliable-endpoint-1.0`
§3.1/§3.2/§3.3 — XRCE reliable stream (`stream_id >= 128`, §8.4.10/§8.4.11).
Sender `submit`/`pendingHeartbeat`/`recvAcknack`/`getInFlight`; Receiver
`recvData`/`drainInOrder`/`pendingAcknack`/`reset`. Window 16, receiver
buffer 64, heartbeat 500 ms, payload ≤ 65535, RFC-1982 16-bit sequence
numbers. Self-contained wire codec (no `Endian`/`Writer` name clash with
`zerodds.d`). Plus `SpscRing` — a wait-free single-producer/single-consumer
ring (`CAP = 1024`) as the async-decoupled writer: the producer's enqueue is
just a slot store plus one release-store of `head`, no lock, no syscall; a
separate drain thread owns the socket and the sender state.

**Repo:** `endpoints/d/reliable.d` (Sender/Receiver/`SpscRing`/wire codec),
`endpoints/d/reliable_app.d` (sender app: initial burst + heartbeat/
ACKNACK-driven retransmit loop), `endpoints/d/example_reliable.d` (in-process
demo), `endpoints/d/reliable_test.d` (unit + byte-golden suite).

**Tests:** (Linux)
- `endpoints/d/reliable_test.d` in the `d` column of `tools/endpoints-gate`
  (job `integration-endpoints`) — `reliable_test.d` asserts monotone seq,
  payload-too-large, window-full, heartbeat first/silenced-&lt;500ms/
  after-500ms/empty, ACKNACK clear/full-clear, in-order drain, reorder,
  duplicate-drop, buffer-full, pending-ACKNACK bitmap, reset, plus 2
  hardcoded byte-goldens (HEARTBEAT/ACKNACK) — and, when the Rust goldens
  are generated, an additional byte-identical check against
  `golden_heartbeat_le.bin`/`golden_acknack_le.bin`. Prints "ALL OK".
- `reliable_5_3_d_samples_arrive_in_order_despite_dropped_datagrams` — peer drops every 3rd datagram (12 samples);
  the app the test builds from the `Ping` the language's generator writes from the IDL of `crates/endpoint-e2e/src/codegen.rs`, and the loop of `reliable_app.d`
  retransmits on ACKNACK; all 12 delivered gap-free in order, the test reads every sample by the IDL's XCDR2 layout (`assert_reliable_pings`).
- `endpoints/d/example_reliable.d` in the `d` column of `tools/endpoints-gate`
  (job `integration-endpoints`) — `example_reliable.d`, in-process Sender/Receiver
  loss-recovery demo, N=12, prints "RELIABLE OK".

**Status:** done.

## §6 Latency — producer enqueue vs. inline sendto

**Spec:** `reliable-endpoint-1.0` §5 item 4 (latency bench), referenced from
`zerodds-endpoint-d-1.0` §4 — micro-bench comparing an inline UDP `sendto`
per sample against the wait-free `SpscRing` enqueue — the syscall the
decoupled writer removes from the producer's hot path.

**Repo:** `endpoints/d/reliable_bench.d` — `ITERS = 20000`, `MonoTime`
timestamps around `sendto` and `ring.enqueue`, sorted, median taken.

**Tests:** (Linux) `reliable_5_4_d_the_producer_enqueue_costs_less_than_the_inline_send` — inline-sendto median
3600 ns; ring-enqueue median 0 ns.

**HONEST NOTE:** the ring enqueue falls below this machine's `MonoTime`
resolution — the median reads 0 ns as a bench-granularity limit, not a claim
of "infinitely fast". What is measurable and real: the 3600 ns syscall is
removed from the producer's path once decoupled via the ring.

**Status:** done — measured to the resolution limit of the bench setup. The
limit belongs to the instrument, not to the implementation.

## Audit Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-d-1.0.md`):

* `python3 ci/run_integration.py integration-bindings` (job `integration-bindings`) — 5 tests green.

5 cited tests green, 0 not green.

Open items: none.
