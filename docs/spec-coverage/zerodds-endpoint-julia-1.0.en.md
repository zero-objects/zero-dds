# `zerodds-endpoint-julia` 1.0 — Spec Coverage

**Source:** docs/specs/zerodds-endpoint-julia-1.0.md — the ZeroDDS Julia
endpoint SDK spec. The marshalling comes from `crates/lang-rules`, column `Language::Julia`; this document covers the transport.

Implementation:

- `endpoints/julia/zerodds.jl` (`module ZeroDDS`) — XRCE framing
  (`write_frame`/`read_frame`), sync `Client`, `Task`+`Channel`-based
  `AsyncReader`.
- `endpoints/julia/reliable.jl` (`module Reliable`) — reliable sender/receiver
  state machine + HEARTBEAT/ACKNACK wire codec.
- `endpoints/julia/reliable_app.jl` — channel + drain `Task`
  (`ReliableAsyncWriter` construct) for the live E2E + the latency bench.
- `endpoints/julia/reliable_test.jl` — unit + byte-golden suite.
- `crates/endpoint-e2e/tests/julia.rs` — ping-pong E2E;
  `crates/endpoint-e2e/tests/julia_reliable.rs` — reliable-stream E2E.

## §1 XRCE framing

**Spec:** §1 — 8-byte XRCE header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + body, byte-identical to `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/julia/zerodds.jl::write_frame`/`read_frame`, constants
`SESSION_NOKEY` (`0x80`) and `STREAM_BEST_EFFORT` (`0x01`).

**Tests:** framing is exercised live via `xrce_8_3_5_8_julia_sync_app_writes_the_ping_and_reads_the_pong` and
`xrce_8_3_5_8_julia_async_app_writes_the_ping_and_reads_the_pong` (§4) — no separate raw-UDP-only test the way Go/C
have one (Julia has no isolated framing-only test path).

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — polling `Client`: `write!` frames + delivers synchronously,
`poll` is a single non-blocking receive (`nothing` when empty).

**Repo:** `endpoints/julia/zerodds.jl::Transport` (`deliver`/`receive`
closures, the sole integration point); `endpoints/julia/zerodds.jl::Client`
(`Client(t)`, `write!`, `poll`, monotonic `seq` counter, default session
`SESSION_NOKEY`/`STREAM_BEST_EFFORT`).

**Tests:** live E2E `xrce_8_3_5_8_julia_sync_app_writes_the_ping_and_reads_the_pong`
(`crates/endpoint-e2e/tests/julia.rs`) — full stack (generated `Ping`/`Pong`
types + `ZeroDDS.Client`) over a real UDP socket against the Rust XRCE peer.

**Status:** done.

## §3 Async `AsyncReader`

**Spec:** §3 — an `@async` `Task` polls the `Transport` and pushes unframed
sample bodies onto a `Channel` (push); the consumer blocks on `take!`. No
separate `AsyncWriter` type (sending stays `write!` on the sync `Client`).

**Repo:** `endpoints/julia/zerodds.jl::AsyncReader` (`start_reader` spawns
the receive `Task`, `Samples` channel `ch`, `running::Ref{Bool}` flag, `recv`
= `take!`, `stop!` sets `running[] = false`).

**Tests:** live E2E `xrce_8_3_5_8_julia_async_app_writes_the_ping_and_reads_the_pong`
(`crates/endpoint-e2e/tests/julia.rs`) — full stack via
`ZeroDDS.start_reader`/`ZeroDDS.Client.write!` against the Rust XRCE peer.

**Status:** done.

## §4 Ping-pong E2E (live)

**Spec:** §5.1 — a Julia app exchanges a typed sample with the shared Rust
XRCE peer over a real UDP socket: full stack (generated types from
`zerodds_lang_rules::generate::source`, the call behind `zerodds-idlc generate --lang julia`, + endpoint SDK) sync and async.

**Repo:** `crates/endpoint-e2e/tests/julia.rs` — `JULIA_APP` (wraps the
generated `Ping`/`Pong` types in `module Gen` alongside `zerodds.jl` to avoid
the `Endian`/`Writer`/`LE` name clash between generated code and the SDK;
mode `sync`/`async` via CLI argument; an `armed` flag serializes the first
`recvfrom` arm against a libuv datagram drop before the first `send` runs).

**Tests:** (Linux)
- `xrce_8_3_5_8_julia_sync_app_writes_the_ping_and_reads_the_pong` — full stack via `ZeroDDS.Client`.
- `xrce_8_3_5_8_julia_async_app_writes_the_ping_and_reads_the_pong` — full stack via `ZeroDDS.start_reader`.

**Status:** done.

## §5 Reliable stream — state machine, wire, async writer

**Spec:** §4 (references `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
stream (`stream_id 0x80`, §8.4.10/§8.4.11), mirroring the reference
`crates/xrce/src/reliable.rs`: `Sender.submit!`/`pending_heartbeat!`/
`recv_acknack!`/`get_in_flight`; `Receiver.recv_data!`/`drain_in_order!`/
`pending_acknack`/`reset!`. Window 16, receiver buffer 64, heartbeat 500 ms,
payload ≤ 65535, RFC-1982 16-bit sequence numbers (`seq_lt`/`seq_gt`).
Alongside it, the async-decoupled `ReliableAsyncWriter` construct: the
producer enqueues onto a `Channel`, a dedicated drain `Task` holds the
`Sender` state and does all the I/O (send, heartbeat, ACKNACK-driven
retransmit) — the producer never enters the kernel.

**Repo:** `endpoints/julia/reliable.jl` (`module Reliable`) —
`reliable_write_frame`/`heartbeat_frame`/`acknack_frame`/`parse_heartbeat`/
`parse_acknack`; `Sender`, `Receiver`; `endpoints/julia/reliable_app.jl` —
`run_reliable` (producer loop, `Channel` + `@async drain` task, empty payload
as the "no more samples" sentinel), `run_bench` (§6);
`endpoints/julia/example_reliable.jl` (runnable in-process demo, no socket).

**Tests:** (Linux)
- `endpoints/julia/reliable_test.jl` in the `julia` column of `tools/endpoints-gate`
  (job `integration-bindings-jvm-julia`) runs `julia
  reliable_test.jl <golden_dir>` — 26 `check(...)` assertions: monotonic seq
  (`endpointjulia_5_submit_assigns_monotonic_seq_0`/`_1`, `endpointjulia_5_submit_two_in_flight`),
  payload-too-large (`endpointjulia_5_submit_rejects_payload_too_large`), window-full
  (`endpointjulia_5_submit_rejects_when_window_full`), heartbeat first/body/silence/after-
  period/none (`endpointjulia_5_heartbeat_fires_first_time`,
  `endpointjulia_5_heartbeat_body_first_last_stream`, `endpointjulia_5_heartbeat_silenced_before_period`,
  `endpointjulia_5_heartbeat_fires_after_period`, `endpointjulia_5_heartbeat_none_when_window_empty`),
  ACKNACK partial/full clear (`endpointjulia_5_acknack_clears_acked_keeps_missing`,
  `endpointjulia_5_acknack_full_clear_when_no_bits_set`), receiver in-order/reorder/dedup/
  buffer-full (`endpointjulia_5_recv_data_buffers_in_order`, `endpointjulia_5_recv_data_reorder_blocks_on_gap`,
  `endpointjulia_5_recv_data_reorder_delivers`, `endpointjulia_5_recv_data_drops_duplicates`,
  `endpointjulia_5_recv_data_rejects_when_buffer_full`), pending-ACKNACK bitmap
  (`endpointjulia_5_pending_acknack_marks_missing_slots`), reset (`endpointjulia_5_reset_clears_state`),
  in-process end-to-end loss recovery (`endpointjulia_5_e2e_drain_blocks_on_lost_s1`,
  `endpointjulia_5_e2e_s1_retransmittable`, `endpointjulia_5_e2e_delivers_all_after_retransmit`), byte-golden
  against the reference goldens generated via `zerodds-xrce`
  (`endpointjulia_5_byte_golden_heartbeat`, `endpointjulia_5_byte_golden_acknack`) plus their parse
  round-trip (`endpointjulia_5_golden_heartbeat_parse`, `endpointjulia_5_golden_acknack_parse`). Expects
  `stdout` to contain `ALL OK`.
- `reliable_5_3_julia_samples_arrive_in_order_despite_dropped_datagrams` — peer drops every 3rd sample once; the app
  (`reliable_app.jl` `run_reliable`) retransmits on ACKNACK; all 12 samples
  delivered gap-free in order.

**Status:** done.

## §6 Latency — channel enqueue vs. inline `send`

**Spec:** §5.3 — the producer path of the `ReliableAsyncWriter` construct
(enqueue → `Channel` push) must be measurably below the inline `send`
(UDP `sendto`) syscall — the evidence that async write removes syscall
latency from the producer path, not that it waits for ACKNACK.

**Repo:** `endpoints/julia/reliable_app.jl::run_bench` — 20000 iterations of
inline `send(sock, ...)` (UDP `sendto`) vs. 20000 iterations of `put!(chan,
sample)` onto a sink `Task`, no live peer needed (an arbitrary loopback port,
only local dispatch cost under measurement). Julia's `Channel` is
lock+condvar-based, not a wait-free SPSC ring — documented as a known limit
in the spec text (`reliable_app.jl` header comment); the enqueue still comes
in well below the inline syscall.

**Tests:** (Linux) `reliable_5_4_julia_the_producer_enqueue_costs_less_than_the_inline_send`
(`crates/endpoint-e2e/tests/julia_reliable.rs`) — channel enqueue **80–106 ns**
vs. inline `send` **6.1–6.5 µs** (~60–75×).

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-julia-1.0.md`):

* `python3 ci/run_integration.py integration-bindings-jvm-julia` (job `integration-bindings-jvm-julia`) — 29 tests green.

29 cited tests green, 0 not green.

Open items: none. Known limit (not a spec violation): Julia's `Channel` is
lock+condvar-based, not a wait-free SPSC ring (§6).
