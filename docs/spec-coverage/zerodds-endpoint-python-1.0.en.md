# `zerodds-endpoint-python` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-python-1.0.md` — the ZeroDDS Python
endpoint SDK spec. Complements the codegen coverage `zerodds-xcdr2-python`
(`docs/spec-coverage/zerodds-xcdr2-python-1.0.md`) — that doc covers
marshalling, this one covers transport.

Implementation:

- `endpoints/python/zerodds_endpoint.py` — XRCE framing
  (`xrce_write_frame`/`xrce_read_frame`), serial HDLC framing
  (`serial_frame`/`serial_deframe`), sync `Client`, `MemTransport`.
- `endpoints/python/zerodds_reliable.py` — reliable sender/receiver state
  machine + HEARTBEAT/ACKNACK wire codec + `ReliableWriter`.
- `endpoints/python/example_async.py` — the asyncio `AsyncReader` pattern
  (§3).
- `crates/endpoint-e2e/tests/python.rs` — ping-pong E2E;
  `crates/endpoint-e2e/tests/python_reliable.rs` — reliable-stream E2E.

## §1 XRCE framing

**Spec:** §1 — 8-byte XRCE header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA/`0x09` DATA, flags, len LE) + body, byte-identical to
`crates/xrce` + `endpoints/c`; plus Annex-C serial HDLC (byte-stuffing +
CRC-16-CCITT-FALSE).

**Repo:** `endpoints/python/zerodds_endpoint.py` —
`xrce_write_frame`/`xrce_read_frame`, `crc16_ccitt_false`,
`serial_frame`/`serial_deframe`, constants `XRCE_SESSION_NOKEY` (`0x80`),
`XRCE_STREAM_BEST_EFFORT` (`0x01`), `XRCE_STREAM_NONE` (`0x00`).

**Tests:** `endpoints/python/test_endpoint.py` (`python3 test_endpoint.py
<golden_dir>`) — WRITE_DATA framing, serial framing, DATA receive, serial
deframe round-trip, HEARTBEAT parse, ACKNACK framing, each checked against
`golden_xrce_le.bin`/`golden_serial_le.bin`/`golden_data_le.bin`/
`golden_heartbeat_le.bin`/`golden_acknack_le.bin`, reports `ALL OK`. The
framing itself is additionally exercised live via
`python_endpoint_sync`/`python_endpoint_async` (§4).

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — non-blocking `Client`: `write` frames + delivers via the
transport, `poll` is a single non-blocking receive.

**Repo:** `endpoints/python/zerodds_endpoint.py::Client`
(`__init__(transport, session, stream)`/`write`/`poll`, monotonic `seq`
counter mod 2¹⁶, default session `XRCE_SESSION_NOKEY`/
`XRCE_STREAM_BEST_EFFORT`); `MemTransport` (in-memory FIFO,
`deliver`/`receive`) as the reference transport; a transport is pure
duck-typing (`deliver(frame)`/`receive() -> bytes|None`), not a formal
interface like Go's `Transport`.

**Tests:** `endpoints/python/example_sync.py` — 5 typed
`Reading(id, value, label)` samples via `Client.write`/`.poll()`, full field
decode, reports `ALL OK`; live E2E `python_endpoint_sync` (§4).

**Status:** done.

## §3 Async (asyncio receive pattern)

**Spec:** §3 — no dedicated SDK type (unlike Go's `AsyncReader`/
`AsyncWriter`): an `async def stream(self)` generator polls
`transport.receive()` non-blockingly and yields the unframed body; the
consumer iterates with `async for`. No separate `AsyncWriter` — `Client.write`
serves sync and async alike since it is already non-blocking.

**Repo:** `endpoints/python/example_async.py::AsyncReader.stream()` — the
reference pattern over `MemTransport`; the same class (inline duplicated,
same semantics) in the E2E app
`crates/endpoint-e2e/tests/python.rs::PY_APP::run_async` over a real
`UdpTransport`.

**Tests:** `endpoints/python/example_async.py` — 5 `Reading` samples via
`async for`, full field decode, reports `ALL OK`; live E2E
`python_endpoint_async` (§4).

**Status:** done.

## §4 Ping-pong E2E (live)

**Spec:** §5.1/§5.2 — a Python app exchanges a typed sample with the shared
Rust XRCE peer over a real UDP socket: once as raw generated codec
(`.encode()`/`.decode()`) with no XRCE frame, once as the full stack
(generated `@idl_struct` dataclasses + endpoint SDK) sync and async.

**Repo:** `crates/endpoint-e2e/tests/python.rs` — `PY_APP` (`run_raw` raw over
UDP with no XRCE frame, uses only `Ping.encode()`/`Pong.decode()`; `run_sync`
via `ze.Client`; `run_async` via the asyncio pattern from §3), mode selected
via `sys.argv[1]`.

**Tests (codepit):**
- `python_raw_udp` — generated `Ping`/`Pong` codec directly over a raw UDP
  socket, no XRCE framing.
- `python_endpoint_sync` — full stack via `ze.Client`.
- `python_endpoint_async` — full stack via the asyncio receive pattern.

3/3 passed (codepit).

**Status:** done.

## §5 Reliable stream — state machine, wire, async writer

**Spec:** §4 (references `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
stream (`stream_id 0x80`, §8.4.10/§8.4.11), mirroring the reference
`crates/xrce/src/reliable.rs`: `ReliableSender.submit`/`pending_heartbeat`/
`recv_acknack`/`get_in_flight`; `ReliableReceiver.recv_data`/
`drain_in_order`/`pending_acknack`/`reset`. Window 16, receiver buffer 64,
heartbeat 500 ms, payload ≤ 65535, RFC-1982 16-bit sequence numbers
(`seq_lt`/`seq_gt`). Alongside it, the async-decoupled `ReliableWriter`: the
producer `enqueue()`s onto a `queue.Queue`, a dedicated drain
`threading.Thread` holds the `ReliableSender` state and does all the I/O
(send, heartbeat, ACKNACK-driven retransmit).

**Honest note:** `ReliableWriter.enqueue()` is **not** a wait-free ring-buffer
push like in Rust/C — it is a lock-protected `queue.Queue.put()`. What is
real is the I/O decoupling: the GIL is released around the drain thread's
blocking `socket.send`/`recv` calls, so `enqueue()` never blocks on a
syscall — thread + GIL-release-around-syscalls decoupling, not a lock-free
data plane (spec §4 honest note).

**Repo:** `endpoints/python/zerodds_reliable.py` —
`reliable_write_frame`/`reliable_unframe`, `heartbeat_frame`/
`parse_heartbeat`, `acknack_frame`/`parse_acknack`; `ReliableSender`,
`ReliableReceiver`; `ReliableWriter` (`queue.Queue`-based,
`enqueue`/`start`/`close`, drain `threading.Thread`);
`endpoints/python/example_reliable.py` (in-process demo with a lossy
receiver thread, UDP sender mode `run`, latency bench mode `bench`).

**Tests (codepit):**
- `python_reliable_unit_and_golden`
  (`crates/endpoint-e2e/tests/python_reliable.rs`) runs
  `endpoints/python/reliable_test.py` against goldens generated via
  `zerodds-xrce` — 21 checks (`check()` calls) across 13 test functions:
  monotonic seq + in-flight count (`test_submit_assigns_monotonic_seqnrs`: 3
  checks), payload-too-large (`test_submit_rejects_payload_too_large`),
  window-full (`test_submit_rejects_when_window_full`), heartbeat
  body/silenced/fires-after/none-when-empty (`test_pending_heartbeat`: 4
  checks), ACKNACK partial/full clear
  (`test_recv_acknack_clears_acked_keeps_missing`/
  `test_recv_acknack_full_clear_when_no_bits_set`), receiver
  in-order/reorder/dedup/buffer-full (`test_recv_data_buffers_in_order`/
  `test_recv_data_reorders_out_of_order`: 2 checks/
  `test_recv_data_drops_duplicates`/`test_recv_data_rejects_when_buffer_full`),
  pending-ACKNACK bitmap (`test_pending_acknack_marks_missing_slots`), reset
  (`test_reset_clears_state`), in-process end-to-end loss recovery
  (`test_end_to_end_sender_receiver_with_loss_recovery`: 3 checks), plus
  conditionally (when a golden dir is passed) byte-golden `test_byte_golden`:
  `heartbeat_frame(1,1,3,0x80)` == the `golden_heartbeat_le.bin` generated by
  `zerodds-xrce`, `acknack_frame(1,1,0,0x80)` == the generated
  `golden_acknack_le.bin`, plus round-trip parse — reports `ALL OK`.
- `python_reliable_loss_recovery` — peer drops every 3rd sample once; the app
  (`example_reliable.py run`) retransmits on ACKNACK via the `ReliableWriter`;
  all 12 samples delivered gap-free in order.
- `python_reliable_no_loss` — lossless baseline; 12/12.
- `python_reliable_producer_latency` — latency bench (§6).

4/4 passed (codepit), 3 of which belong to this section (latency bench in
§6).

**Status:** done.

## §6 Latency — `enqueue()` vs. inline `socket.send`

**Spec:** §5.4 — the producer path of `ReliableWriter` (`enqueue()` →
`queue.Queue.put()`) must be measurably below the inline `socket.send`
syscall — the evidence that syscall latency is decoupled from the producer
path (spec §4 honest note: thread + GIL release, not a wait-free ring).

**Repo:** `endpoints/python/example_reliable.py::run_bench` — 20000
iterations of inline `sock.send(reliable_write_frame(...))` vs. 20000
iterations of `ReliableWriter.enqueue(sample)` against an idle-draining sink
socket (no live peer needed, only local dispatch cost under measurement).

**Tests (codepit):** `python_reliable_producer_latency`
(`crates/endpoint-e2e/tests/python_reliable.rs`) — enqueue **704 ns** vs.
inline `send` **4108 ns** (~5.8×).

**Honest note:** the factor is well below Go's ~175–220× and Rust's/C's
wait-free-ring numbers — expected, since `enqueue()` itself is already a
`queue.Queue.put()` (lock + internal deque append, not a bare memory store)
and CPython interpreter overhead dominates both sides of the measurement.
The number evidences decoupling from the syscall, not wait-freedom.

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run (codepit, verified): `cargo test -p zerodds-endpoint-e2e --test
python` 3/3 (ping-pong: `python_raw_udp`/`python_endpoint_sync`/
`python_endpoint_async`); `--test python_reliable` 4/4
(`python_reliable_loss_recovery`/`python_reliable_no_loss`/
`python_reliable_unit_and_golden` — 21 Python unit checks incl. byte-golden/
`python_reliable_producer_latency`); latency bench enqueue 704 ns / inline
`send` 4108 ns (~5.8×) — thread + GIL-release decoupling, not a wait-free
ring (see honest note §5/§6).

Open items: none.
