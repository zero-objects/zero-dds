# `zerodds-endpoint-julia` 1.0 — Spec Coverage

**Source:** docs/specs/zerodds-endpoint-julia-1.0.md — the ZeroDDS Julia
endpoint SDK spec. Complements the codegen coverage `zerodds-xcdr2-julia`
(`docs/spec-coverage/zerodds-xcdr2-julia-1.0.md`) — that doc covers
marshalling, this one covers transport.

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

**Tests:** framing is exercised live via `julia_endpoint_sync` and
`julia_endpoint_async` (§4) — no separate raw-UDP-only test the way Go/C
have one (Julia has no isolated framing-only test path).

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — polling `Client`: `write!` frames + delivers synchronously,
`poll` is a single non-blocking receive (`nothing` when empty).

**Repo:** `endpoints/julia/zerodds.jl::Transport` (`deliver`/`receive`
closures, the sole integration point); `endpoints/julia/zerodds.jl::Client`
(`Client(t)`, `write!`, `poll`, monotonic `seq` counter, default session
`SESSION_NOKEY`/`STREAM_BEST_EFFORT`).

**Tests:** live E2E `julia_endpoint_sync`
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

**Tests:** live E2E `julia_endpoint_async`
(`crates/endpoint-e2e/tests/julia.rs`) — full stack via
`ZeroDDS.start_reader`/`ZeroDDS.Client.write!` against the Rust XRCE peer.

**Status:** done.

## §4 Ping-pong E2E (live)

**Spec:** §5.1 — a Julia app exchanges a typed sample with the shared Rust
XRCE peer over a real UDP socket: full stack (generated types from
`crates/idl-julia` + endpoint SDK) sync and async.

**Repo:** `crates/endpoint-e2e/tests/julia.rs` — `JULIA_APP` (wraps the
generated `Ping`/`Pong` types in `module Gen` alongside `zerodds.jl` to avoid
the `Endian`/`Writer`/`LE` name clash between generated code and the SDK;
mode `sync`/`async` via CLI argument; an `armed` flag serializes the first
`recvfrom` arm against a libuv datagram drop before the first `send` runs).

**Tests (codepit):**
- `julia_endpoint_sync` — full stack via `ZeroDDS.Client`.
- `julia_endpoint_async` — full stack via `ZeroDDS.start_reader`.

2/2 passed (codepit).

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

**Tests (codepit):**
- `julia_reliable_unit_and_golden`
  (`crates/endpoint-e2e/tests/julia_reliable.rs`) runs `julia
  reliable_test.jl <golden_dir>` — 26 `check(...)` assertions: monotonic seq
  (`submit_assigns_monotonic_seq_0`/`_1`, `submit_two_in_flight`),
  payload-too-large (`submit_rejects_payload_too_large`), window-full
  (`submit_rejects_when_window_full`), heartbeat first/body/silence/after-
  period/none (`heartbeat_fires_first_time`,
  `heartbeat_body_first_last_stream`, `heartbeat_silenced_before_period`,
  `heartbeat_fires_after_period`, `heartbeat_none_when_window_empty`),
  ACKNACK partial/full clear (`acknack_clears_acked_keeps_missing`,
  `acknack_full_clear_when_no_bits_set`), receiver in-order/reorder/dedup/
  buffer-full (`recv_data_buffers_in_order`, `recv_data_reorder_blocks_on_gap`,
  `recv_data_reorder_delivers`, `recv_data_drops_duplicates`,
  `recv_data_rejects_when_buffer_full`), pending-ACKNACK bitmap
  (`pending_acknack_marks_missing_slots`), reset (`reset_clears_state`),
  in-process end-to-end loss recovery (`e2e_drain_blocks_on_lost_s1`,
  `e2e_s1_retransmittable`, `e2e_delivers_all_after_retransmit`), byte-golden
  against the reference goldens generated via `zerodds-xrce`
  (`byte_golden_heartbeat`, `byte_golden_acknack`) plus their parse
  round-trip (`golden_heartbeat_parse`, `golden_acknack_parse`). Expects
  `stdout` to contain `ALL OK`.
- `julia_reliable_loss_recovery` — peer drops every 3rd sample once; the app
  (`reliable_app.jl` `run_reliable`) retransmits on ACKNACK; all 12 samples
  delivered gap-free in order.
- `julia_reliable_no_loss` — lossless baseline; 12/12.

3 of the 4 tests in this section (latency bench in §6); combined with §6,
4/4 passed (codepit).

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

**Tests (codepit):** `julia_reliable_producer_latency`
(`crates/endpoint-e2e/tests/julia_reliable.rs`) — channel enqueue **80–106 ns**
vs. inline `send` **6.1–6.5 µs** (~60–75×).

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run (codepit, verified): `cargo test -p zerodds-endpoint-e2e --test
julia` 2/2 (ping-pong: `julia_endpoint_sync`/`julia_endpoint_async`);
`--test julia_reliable` 4/4 (`julia_reliable_unit_and_golden` — 26
`check(...)` assertions incl. byte-golden, `julia_reliable_loss_recovery`,
`julia_reliable_no_loss`, `julia_reliable_producer_latency`); latency bench
channel enqueue 80–106 ns / inline `send` 6.1–6.5 µs (~60–75×).

Open items: none. Known limit (not a spec violation): Julia's `Channel` is
lock+condvar-based, not a wait-free SPSC ring (§6).
