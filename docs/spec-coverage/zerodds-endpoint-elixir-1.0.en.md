# `zerodds-endpoint-elixir` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-elixir-1.0.md` — the ZeroDDS Elixir
endpoint SDK spec. Complements the codegen coverage `zerodds-xcdr2-elixir`
(`docs/spec-coverage/zerodds-xcdr2-elixir-1.0.md`) — that doc covers
marshalling, this one covers transport.

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

**Tests:** `crates/endpoint-e2e/tests/elixir.rs::elixir_raw_udp` (raw XCDR2
with no XRCE frame — its own minimal harness); the framing itself is
exercised live via `elixir_endpoint_sync` and `elixir_endpoint_async` (§4).

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
samples); live E2E `elixir_endpoint_sync` (§4).

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
consumer process for 5 samples); live E2E `elixir_endpoint_async` (§4).

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

**Tests (codepit):**
- `elixir_raw_udp` — generated `Ping`/`Pong` codec directly over a raw UDP
  socket, no XRCE framing.
- `elixir_endpoint_sync` — full stack via `ZeroDDS.Client`.
- `elixir_endpoint_async` — full stack via `ZeroDDS.AsyncReader` (poll for
  the write side, process/mailbox for the read side).

3/3 passed (codepit).

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
socket); `endpoints/elixir/reliable_app.exs` (live UDP sender app for the
E2E, including a `bench` mode).

**Tests (codepit):**
- `elixir_reliable_unit_and_golden`
  (`crates/endpoint-e2e/tests/elixir_reliable.rs`) runs
  `elixir -r lib/reliable.ex reliable_test.exs [golden_dir]` — 22 unit
  checks: monotonic seq (`submit_assigns_monotonic_seqnrs`,
  `submit_two_in_flight`), payload-too-large
  (`submit_rejects_payload_too_large`), window-full
  (`submit_rejects_when_window_full`), heartbeat
  first/body/t0/silence/after/none (`pending_heartbeat_fires_first_time`/
  `_body_first_last_stream`/`_fires_at_t0`/`_silenced_before_period`/
  `_fires_after_period`/`_none_when_window_empty`), ACKNACK partial/full
  clear (`recv_acknack_clears_acked_keeps_missing`/
  `_full_clear_when_no_bits_set`), receiver in-order/reorder/dedup/buffer-full
  (`recv_data_buffers_in_order`/`_reorder_blocks_on_gap`/`_reorder_delivers`/
  `_drops_duplicates`/`_rejects_when_buffer_full`), pending-ACKNACK bitmap
  (`pending_acknack_marks_missing_slots`), reset
  (`reset_clears_state_completely`), end-to-end loss recovery in three steps
  (`e2e_drain_blocks_on_lost_middle_sample`/
  `e2e_missing_sample_retransmittable`/`e2e_delivers_all_after_retransmit`).
  Plus — when the Rust golden generator (`zerodds-endpoint-golden`) ran — 4
  more checks: byte-golden (`byte_golden_heartbeat`/`byte_golden_acknack`:
  `heartbeat_frame(0x80,0x00,1,1,3,0x80)` ==
  `80 00 01 00 0B 01 05 00 01 00 03 00 80`,
  `acknack_frame(0x80,0x00,1,1,0,0x80)` ==
  `80 00 01 00 0A 01 05 00 01 00 00 00 80` — identical to the reference
  goldens) and parse roundtrip (`golden_heartbeat_parse`/
  `golden_acknack_parse`). No false-green: if the golden generator did not
  run, this is logged best-effort rather than silently skipped.
- `elixir_reliable_loss_recovery` — peer drops every 3rd sample once; the app
  (`reliable_app.exs`) retransmits via `ZeroDDS.Reliable.Drain` on ACKNACK;
  all 12 samples delivered gap-free in order.
- `elixir_reliable_no_loss` — lossless baseline; 12/12.
- `elixir_reliable_example` — `example_reliable.exs` runs and reports
  `RELIABLE OK` (12 samples, drop simulation + ACKNACK recovery in the
  in-process model).

5/5 passed (codepit), 4 of which belong to this section (latency bench in
§6).

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

**Tests (codepit):** `elixir_reliable_producer_latency`
(`crates/endpoint-e2e/tests/elixir_reliable.rs`) — `submit` cast **452 ns**
vs. inline `:gen_udp.send` **5618 ns** (~12×).

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run (codepit, verified): `cargo test -p zerodds-endpoint-e2e --test elixir`
3/3 (ping-pong: `elixir_raw_udp`/`elixir_endpoint_sync`/`elixir_endpoint_async`);
`--test elixir_reliable` 5/5 (`elixir_reliable_unit_and_golden` — 22 Elixir
unit checks incl. byte-golden, `elixir_reliable_loss_recovery`,
`elixir_reliable_no_loss`, `elixir_reliable_example`,
`elixir_reliable_producer_latency`); latency bench `submit` cast 452 ns /
inline `:gen_udp.send` 5618 ns (~12×).

Open items: none.
