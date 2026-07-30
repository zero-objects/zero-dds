# `zerodds-endpoint-ocaml` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-ocaml-1.0.md` — the ZeroDDS OCaml
endpoint SDK spec. Complements the codegen coverage `zerodds-xcdr2-ocaml`
(`docs/spec-coverage/zerodds-xcdr2-ocaml-1.0.md`) — that doc covers
marshalling, this one covers transport.

Implementation:

- `endpoints/ocaml/zerodds.ml` — module `Endpoint` (XRCE framing
  `write_frame`/`read_frame`), the `transport` type, `module Client`
  (sync), `module Mailbox` + `module AsyncReader` (async).
- `endpoints/ocaml/reliable.ml` — a self-contained module `Reliable`
  (file-as-module, no dependency on `zerodds.ml`): reliable
  sender/receiver state machine + HEARTBEAT/ACKNACK/WRITE_DATA wire codec +
  `module Writer` (the async-decoupled reliable writer with a drain
  `Thread`).
- `crates/endpoint-e2e/tests/ocaml.rs` — ping-pong E2E;
  `crates/endpoint-e2e/tests/ocaml_reliable.rs` — reliable-stream E2E +
  unit/golden + example + latency bench.

Both E2E test files are gated on `ocamlfind` (via `ocamlfind printconf`):
if the toolchain is missing they skip loudly (`eprintln!("SKIP ...")`), no
false-green.

## §1 XRCE framing

**Spec:** §1 — 8-byte XRCE header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + body, byte-identical to `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/ocaml/zerodds.ml::Endpoint` — `write_frame`/
`read_frame`, constants `session_nokey` (`0x80`) and `stream_best_effort`
(`0x01`).

**Tests:** no isolated framing unit test (unlike Go's `go_raw_udp`); the
framing itself is exercised live via `ocaml_endpoint_sync`/
`ocaml_endpoint_async` (§4) — the app there frames explicitly via
`Zerodds.Endpoint.write_frame` before the `AsyncReader`/`Client` takes over.

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blocking `Client`: `write` frames + delivers synchronously,
`poll` is a single non-blocking receive.

**Repo:** `endpoints/ocaml/zerodds.ml::transport` record
(`deliver`/`receive`, the sole integration point);
`endpoints/ocaml/zerodds.ml::Client` (`create`/`write`/`poll`, monotonic
`seq` counter with 16-bit wraparound, default session
`session_nokey`/`stream_best_effort`).

**Tests:** live E2E `ocaml_endpoint_sync`
(`crates/endpoint-e2e/tests/ocaml.rs`) — the embedded OCaml app
(`OCAML_MAIN`) builds a `Gen.Ping` sample via the generated
`zerodds-idlc --ocaml` codec, sends it via `Zerodds.Client.write` to the
Rust peer, and polls `Zerodds.Client.poll` until the `Gen.Pong` reply
(10s deadline).

**Status:** done.

## §3 Async `AsyncReader`

**Spec:** §3 — a `Thread` polls the `transport` and pushes unframed sample
bodies onto a Mutex/Condition `Mailbox` (FIFO); the consumer blocks in
`recv` on `Condition.wait`. No Lwt, no Async — just `Thread`/`Mutex`/
`Condition` from `threads.posix`.

**Repo:** `endpoints/ocaml/zerodds.ml::Mailbox` (`put`/`take`, a generic
Mutex/Condition FIFO); `endpoints/ocaml/zerodds.ml::AsyncReader`
(`start` spawns the receive `Thread` via `Thread.create loop ()`,
`recv`/`stop`).

**Tests:** live E2E `ocaml_endpoint_async`
(`crates/endpoint-e2e/tests/ocaml.rs`) — the app starts
`Zerodds.AsyncReader.start`, delivers the framed `Gen.Ping` sample directly
via `transport.deliver`, blocks in `AsyncReader.recv` until the `Gen.Pong`
reply, and stops the reader.

**Status:** done.

## §4 Ping-pong E2E (live)

**Spec:** §5.1 — an OCaml app exchanges a typed sample with the shared Rust
XRCE peer over a real UDP socket, once via the sync `Client`, once via
`AsyncReader`, each with the full stack (generated `Gen.Ping`/`Gen.Pong`
types from `zerodds-idlc --ocaml` + `endpoints/ocaml`).

**Repo:** `crates/endpoint-e2e/tests/ocaml.rs` — `OCAML_MAIN` (an embedded
OCaml source, mode `sync`/`async` via CLI argument), compiled together with
the generated `gen.ml` (`Gen.Ping`/`Gen.Pong`, its own `module Gen.Wire`)
and the SDK `zerodds.ml` (its own `module Zerodds.Wire`). Both `Wire`
modules stay separate compilation units so no name collision occurs (the
test documents this explicitly in a comment).

**Tests (codepit):**
- `ocaml_endpoint_sync` — full stack via `Zerodds.Client`.
- `ocaml_endpoint_async` — full stack via `Zerodds.AsyncReader`.

2/2 passed (codepit).

**Status:** done.

## §5 Reliable stream — state machine, wire, async writer

**Spec:** §4 (references `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
stream (`stream_id 0x80`, §8.4.10/§8.4.11), mirroring the reference
`crates/xrce/src/reliable.rs`: `Sender.submit`/`pending_heartbeat`/
`recv_acknack`/`get_in_flight`; `Receiver.recv_data`/`drain_in_order`/
`pending_acknack`/`reset`. Window 16, receiver buffer 64, heartbeat 500 ms,
payload ≤ 65535, RFC-1982 16-bit sequence numbers. Alongside it, the
async-decoupled `Reliable.Writer`: the producer enqueues wait-free via
`Mailbox.put` (lock the mutex, cons, `Condition.signal`, unlock — no
syscall), a dedicated drain `Thread` holds the `Sender` state and the UDP
`Unix` socket and does all the I/O (`Writer.tick`: drain the mailbox →
`Sender.submit` → send WRITE_DATA, tick the heartbeat, poll for ACKNACK via
`Unix.select` with a 20ms timeout and retransmit on it) — the producer
never enters the kernel.

**Repo:** `endpoints/ocaml/reliable.ml` — `reliable_write_frame`/
`parse_write_frame`, `heartbeat_frame`/`parse_heartbeat`,
`acknack_frame`/`parse_acknack`; `module Sender`, `module Receiver`;
`module Mailbox` (its own copy, independent of `zerodds.ml`'s `Mailbox`);
`module Writer` (`create` spawns the drain `Thread`,
`enqueue`/`wait_drained`/`stop`/`in_flight_count`);
`endpoints/ocaml/example_reliable.ml` (a runnable in-process demo, no
socket); `endpoints/ocaml/reliable_app.ml` (the live UDP sender app for the
E2E, argv = `<peer-port> <N>`).

**Tests (codepit):**
- `ocaml_reliable_unit_and_golden` (`crates/endpoint-e2e/tests/ocaml_reliable.rs`)
  compiles and runs `endpoints/ocaml/reliable_test.ml` — a single script (no
  named `Test*` functions like Go's suite, instead ~34 sequential `check`
  assertions inside one `let () = ...` block) covering: monotonic seq
  (`monotonic seq 0`/`1`, `in-flight count`), payload-too-large (`payload
  too large`), window-full (`fill window`/`window full`), heartbeat
  first/silence/after-period/none (`heartbeat body`, `heartbeat silenced
  <500ms`, `heartbeat after 500ms`, `no heartbeat when empty`), ACKNACK
  partial/full clear (`acknack clears acked`/`seq2 retransmittable`,
  `acknack full clear`), receiver in-order/reorder/dedup/buffer-full
  (`in-order drain[0]`/`[1]`/`shape`, `expected advanced`, `reorder: only
  seq0`/`reorder: 1+2`, `duplicate dropped`, `fill recv buffer`/`recv
  buffer full`), pending-ACKNACK bitmap (`slot 0 missing`/`slot 2 missing`/
  `slot 1 present`/`slot 3 present`), reset (`reset clears receiver`),
  in-process end-to-end loss recovery (`submit e2e`, `only seq0 before
  recovery`, `seq1 retransmittable`, `seq1+2 after recovery`), and
  byte-golden (`heartbeat byte-golden (hardcoded)` ==
  `80 00 01 00 0b 01 05 00 01 00 03 00 80`, `acknack byte-golden
  (hardcoded)` == `80 00 01 00 0a 01 05 00 01 00 00 00 80` — identical to
  the reference goldens; optionally also byte-identity against the `.bin`
  files freshly generated by `zerodds-endpoint-golden`, when the Rust
  golden tool ran successfully). Prints `ALL OK` and exit code 0 on
  success.
- `ocaml_reliable_loss_recovery` — peer drops every 3rd sample once; the
  app (`reliable_app`) retransmits on ACKNACK; all 12 samples delivered
  gap-free in order.
- `ocaml_reliable_no_loss` — lossless baseline; 12/12.
- `ocaml_reliable_example` — `example_reliable` runs and reports
  `delivered: 0 1 2 ... 11` + `RELIABLE OK`.

5/5 passed (codepit), 4 of which belong to this section (latency bench in
§6).

**Status:** done.

## §6 Latency — mailbox enqueue vs. inline `sendto`

**Spec:** §5.4 — the producer path of `Reliable.Writer` (`enqueue` →
`Mailbox.put`) must be measurably below the inline `sendto` syscall — the
evidence that async write removes syscall latency from the producer path,
not that it waits for ACKNACK.

**Repo:** `endpoints/ocaml/reliable_bench.ml` — median over 500 batches of
200 iterations each (`Unix.gettimeofday` has only microsecond-ish
resolution, hence batch timing) of inline `Unix.sendto` (a real kernel
transition) vs. `Mailbox.put` (lock the mutex, cons, signal, unlock, no
syscall), no live peer needed (a loopback socket, only local dispatch cost
under measurement).

**Tests (codepit):** `ocaml_reliable_latency_bench`
(`crates/endpoint-e2e/tests/ocaml_reliable.rs`) — mailbox-enqueue median
**~30 ns** vs. inline `sendto` median **~4.1 µs** (~130–140×). The exact
figure varies run to run (batch median via `Unix.gettimeofday`); the test
only asserts the output contains the line `producer latency: ...`, not the
concrete factor.

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run (codepit, verified): `cargo test -p zerodds-endpoint-e2e --test ocaml`
2/2 (ping-pong: `ocaml_endpoint_sync`/`ocaml_endpoint_async`);
`--test ocaml_reliable` 5/5 (`ocaml_reliable_unit_and_golden` — ~34
sequential checks incl. byte-golden, `ocaml_reliable_loss_recovery`,
`ocaml_reliable_no_loss`, `ocaml_reliable_example`,
`ocaml_reliable_latency_bench`); latency bench mailbox enqueue ~30 ns /
inline `sendto` ~4.1 µs (~130–140×).

Open items: none.
