# `zerodds-endpoint-elixir` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-elixir-1.0.md` — the ZeroDDS Elixir
endpoint SDK spec. The marshalling comes from `crates/lang-rules`, column `Language::Elixir`; this document covers the transport.

Implementation:

- `endpoints/elixir/lib/zerodds.ex` — XRCE framing (`ZeroDDS.Endpoint`), sync
  `ZeroDDS.Client`, async `ZeroDDS.AsyncReader`, `ZeroDDS.MemTransport`.
- `endpoints/elixir/lib/reliable.ex` — reliable sender/receiver state machine
  (`ZeroDDS.Reliable.Sender`/`Receiver`) + HEARTBEAT/ACKNACK wire codec
  (`ZeroDDS.Reliable`) + `ZeroDDS.Reliable.Drain`.
- `crates/endpoint-e2e/tests/elixir.rs` — ping-pong E2E;
  `crates/endpoint-e2e/tests/elixir_reliable.rs` — reliable-stream E2E.

## §1 XRCE framing

**Spec:** §1 — 8-byte XRCE header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + body, byte-identical to `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/elixir/lib/zerodds.ex::ZeroDDS.Endpoint` —
`write_frame/4`/`read_frame/1`, constants `session_nokey/0` (`0x80`) and
`stream_best_effort/0` (`0x01`).

**Tests:** `crates/endpoint-e2e/tests/elixir.rs::endpoint_elixir_5_1_the_generated_codec_pings_without_the_xrce_frame` (raw XCDR2
with no XRCE frame — its own minimal harness); the framing itself is
exercised live via `xrce_8_3_5_8_elixir_sync_app_writes_the_ping_and_reads_the_pong` and `xrce_8_3_5_8_elixir_async_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blocking `Client`: `write/2` frames + delivers synchronously,
`poll/1` is a single non-blocking receive; no built-in timeout-receive — the
caller polls itself in a deadline loop.

**Repo:** `endpoints/elixir/lib/zerodds.ex::ZeroDDS.Client` (`new/1`,
`write/2` with a modulo-`0x10000` sequence counter, `poll/1`); the
`transport` contract `%{deliver: fun/1, receive: fun/0}` as the sole
integration point; `ZeroDDS.MemTransport` as the in-memory reference
transport for tests/examples.

**Tests:** `endpoints/elixir/test.exs` — "sync loopback: 5 samples OK"
(`ZeroDDS.Client.write`/`poll` over `MemTransport`, full roundtrip of 5
samples); live E2E `xrce_8_3_5_8_elixir_sync_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §3 Async `Reader`/`Writer`

**Spec:** §3 — a spawned process polls the `transport` and sends unframed
sample bodies as `{:zerodds_sample, body}` to the `target`'s mailbox; there
is no separate `AsyncWriter` type — the sync `Client` (§2) is already the
send path.

**Repo:** `endpoints/elixir/lib/zerodds.ex::ZeroDDS.AsyncReader` (`start/2`
spawns the receive process, `stop/1` via a `:zerodds_stop` message).

**Tests:** `endpoints/elixir/test.exs` — "async loopback: 5 samples OK"
(`ZeroDDS.AsyncReader.start/2` over `MemTransport`, a `receive` block in the
consumer process for 5 samples); live E2E `xrce_8_3_5_8_elixir_async_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §4 Ping-pong E2E (live)

**Spec:** §5.1/§5.2 — an Elixir app exchanges a typed sample with the shared
Rust XRCE peer over a real UDP socket: once as raw generated codec with no
XRCE frame, once as the full stack (generated types + endpoint SDK) sync and
async.

**Repo:** `crates/endpoint-e2e/tests/elixir.rs` — `ELIXIR_RAW_MAIN` (raw UDP,
no XRCE frame, uses only the generated
`Zdgen.Ping.marshal_xcdr`/`Zdgen.Pong.unmarshal`), `ELIXIR_ENDPOINT_MAIN` (a
`%{deliver:, receive:}` transport over `:gen_udp` + the `ZeroDDS` module,
mode `sync`/`async` via CLI argument; the generated `Zdgen.*` types and the
`ZeroDDS.*` SDK live in disjoint namespaces and are loaded side by side).

**Tests:** (Linux)
- `endpoint_elixir_5_1_the_generated_codec_pings_without_the_xrce_frame` — generated `Ping`/`Pong` codec directly over a raw UDP
  socket, no XRCE framing.
- `xrce_8_3_5_8_elixir_sync_app_writes_the_ping_and_reads_the_pong` — full stack via `ZeroDDS.Client`.
- `xrce_8_3_5_8_elixir_async_app_writes_the_ping_and_reads_the_pong` — full stack via `ZeroDDS.AsyncReader` (poll for
  the write side, process/mailbox for the read side).

**Status:** done.

## §5 Reliable stream — state machine, wire, async writer

**Spec:** §4 (references `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
stream (`stream_id 0x80`, §8.4.10/§8.4.11), mirroring the reference
`crates/xrce/src/reliable.rs`: `ZeroDDS.Reliable.Sender.submit/2`/
`pending_heartbeat/2`/`recv_acknack/2`/`get_in_flight/2`;
`ZeroDDS.Reliable.Receiver.recv_data/3`/`drain_in_order/1`/
`pending_acknack/2`/`reset/1`. Window 16, receiver buffer 64, heartbeat
500 ms, payload ≤ 65535, RFC-1982 16-bit sequence numbers — as immutable
structs (every call threads the state through and returns it, BEAM has no
mutation). Alongside it, the async-decoupled `ZeroDDS.Reliable.Drain`: not a
wait-free ring but a `GenServer` — the producer `submit/2`s (a
`GenServer.cast`, one mailbox send, no kernel entry), a dedicated drain
process instance holds the `Sender` state and the `:gen_udp` socket and does
all the I/O (send, heartbeat tick every 50 ms, ACKNACK-driven retransmit).

**Repo:** `endpoints/elixir/lib/reliable.ex` —
`ZeroDDS.Reliable.write_frame/2`, `heartbeat_frame/6`/`parse_heartbeat/1`,
`acknack_frame/6`/`parse_acknack/1`; `ZeroDDS.Reliable.Sender`,
`ZeroDDS.Reliable.Receiver`; `ZeroDDS.Reliable.Drain`
(`start_link/1`/`activate/1`/`submit/2`/`finish/2`, FIFO `drain_pending`
cascade, socket ownership handoff `controlling_process` → `activate`);
`endpoints/elixir/example_reliable.exs` (runnable in-process demo, no
socket); `endpoints/elixir/reliable_app.exs` (UDP sender app, including a
`bench` mode for the latency test).

**Tests:** (Linux)
- `endpoints/elixir/reliable_test.exs` in the `elixir` column of `tools/endpoints-gate`
  (job `integration-endpoints`) runs
  `elixir -r lib/reliable.ex reliable_test.exs [golden_dir]` — 22 unit
  checks: monotonic seq (`endpointelixir_5_submit_assigns_monotonic_seqnrs`,
  `endpointelixir_5_submit_two_in_flight`), payload-too-large
  (`endpointelixir_5_submit_rejects_payload_too_large`), window-full
  (`endpointelixir_5_submit_rejects_when_window_full`), heartbeat
  first/body/t0/silence/after/none (`endpointelixir_5_pending_heartbeat_fires_first_time`/
  `_body_first_last_stream`/`_fires_at_t0`/`_silenced_before_period`/
  `_fires_after_period`/`_none_when_window_empty`), ACKNACK partial/full
  clear (`endpointelixir_5_recv_acknack_clears_acked_keeps_missing`/
  `_full_clear_when_no_bits_set`), receiver in-order/reorder/dedup/buffer-full
  (`endpointelixir_5_recv_data_buffers_in_order`/`_reorder_blocks_on_gap`/`_reorder_delivers`/
  `_drops_duplicates`/`_rejects_when_buffer_full`), pending-ACKNACK bitmap
  (`endpointelixir_5_pending_acknack_marks_missing_slots`), reset
  (`endpointelixir_5_reset_clears_state_completely`), end-to-end loss recovery in three steps
  (`endpointelixir_5_e2e_drain_blocks_on_lost_middle_sample`/
  `endpointelixir_5_e2e_missing_sample_retransmittable`/`endpointelixir_5_e2e_delivers_all_after_retransmit`).
  Plus — when the Rust golden generator (`zerodds-endpoint-golden`) ran — 4
  more checks: byte-golden (`endpointelixir_5_byte_golden_heartbeat`/`endpointelixir_5_byte_golden_acknack`:
  `heartbeat_frame(0x80,0x00,1,1,3,0x80)` ==
  `80 00 01 00 0B 01 05 00 01 00 03 00 80`,
  `acknack_frame(0x80,0x00,1,1,0,0x80)` ==
  `80 00 01 00 0A 01 05 00 01 00 00 00 80` — identical to the reference
  goldens) and parse roundtrip (`endpointelixir_5_golden_heartbeat_parse`/
  `endpointelixir_5_golden_acknack_parse`). No false-green: if the golden generator did not
  run, this is logged best-effort rather than silently skipped.
- `reliable_5_3_elixir_samples_arrive_in_order_despite_dropped_datagrams` — peer drops every 3rd sample once; the app
  the test builds from the `Ping` the language's generator writes from the IDL of `crates/endpoint-e2e/src/codegen.rs`, and `ZeroDDS.Reliable.Drain` retransmits on
  ACKNACK; all 12 samples delivered gap-free in order, the test reads every sample by the IDL's XCDR2 layout (`assert_reliable_pings`).
- `endpoints/elixir/example_reliable.exs` in the `elixir` column of `tools/endpoints-gate`
  (job `integration-endpoints`) — `example_reliable.exs` runs and reports
  `RELIABLE OK` (12 samples, drop simulation + ACKNACK recovery in the
  in-process model).

**Status:** done.

## §6 Latency — GenServer `submit` cast vs. inline `:gen_udp.send`

**Spec:** §5.4 — the producer path of `ZeroDDS.Reliable.Drain.submit/2`
(`GenServer.cast` → mailbox send) must be measurably below the inline
`:gen_udp.send` syscall — the evidence that BEAM message-passing removes
syscall latency from the producer path, not that it waits for ACKNACK.
Unlike a wait-free ring (Go/Zig/Nim), the decoupling here is a GenServer
cast — a mailbox send with scheduler involvement, hence a smaller but still
clear margin over the inline syscall.

**Repo:** `endpoints/elixir/reliable_app.exs::ReliableApp.bench/1` — 20000
iterations of inline `:gen_udp.send` (connected socket) vs. 20000 iterations
of `ZeroDDS.Reliable.Drain.submit/2`, no live peer needed (an arbitrary bound
UDP port, only local dispatch cost under measurement).

**Tests:** (Linux) `reliable_5_4_elixir_the_producer_enqueue_costs_less_than_the_inline_send`
(`crates/endpoint-e2e/tests/elixir_reliable.rs`) — `submit` cast **452 ns**
vs. inline `:gen_udp.send` **5618 ns** (~12×).

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-elixir-1.0.md`):

* `python3 ci/run_integration.py integration-bindings-gnat-beam` (job `integration-bindings-gnat-beam`) — 5 tests green.
* `python3 ci/run_integration.py integration-endpoints` (job `integration-endpoints`) — 16 tests green.

21 cited tests green, 0 not green.

Open items: none.
