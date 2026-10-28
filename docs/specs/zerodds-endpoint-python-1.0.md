<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-python` v1.0 — Python endpoint SDK

**Status:** normative · ZeroDDS vendor spec. Implemented in `endpoints/python/`.

Analogous to [`zerodds-xcdr2-python`](zerodds-xcdr2-python-1.0.md) (there the
marshalling) and the endpoint SDKs of other languages (`endpoints/go`,
`endpoints/nim`, `endpoints/c`, `endpoints/ada`, ...): the native
Python endpoint over XRCE framing, sync `Client`, the idiomatic
asyncio receive pattern and the reliable stream, so that a Python app exchanges an
XCDR2 sample byte-identical to `crates/xrce` + `endpoints/c` with the shared
Rust peer. Pure stdlib, compatible with Python 2.7 and 3.x
(`zerodds_endpoint.py`, `zerodds_wire.py`); the asyncio layer is Python-3-only.

## §1 XRCE framing

An 8-byte XRCE header (`session`, `stream`, `seq` LE, submessage ID `0x07`
WRITE_DATA/`0x09` DATA, `flags`, `len` LE) followed by the XCDR2 sample body —
byte-identical to `crates/xrce` and `endpoints/c`. In addition, the serial
framing of DDS-XRCE 1.0 Annex C.1 (addresses, `payloadLength` LE,
byte stuffing `0x7E`/`0x7D`/XOR `0x20`, FCS-16 per RFC 1662 LE), a mirror of
`endpoints/c/src/zerodds_endpoint.c`.

`endpoints/python/zerodds_endpoint.py` MUST provide:

- `xrce_write_frame(session, stream, seq, sample) -> bytes` — frames a
  sample as WRITE_DATA.
- `xrce_read_frame(frame) -> bytes` — unframes WRITE_DATA (`0x07`) or DATA
  (`0x09`); raises `ValueError` for a frame that is too short or a wrong
  submessage ID.
- `serial_frame(source, remote, payload) -> bytes` /
  `serial_deframe(frame) -> (source, remote, payload)` — Annex C frames;
  `serial_deframe` raises `ValueError` on a wrong `frameCheck`, a wrong
  escape, `0x7E` before the end of the frame (C.1.3) and a truncated frame;
  `fcs16` as a standalone function.
- Constants `XRCE_SESSION_NOKEY` (`0x80`, best-effort, without ClientKey),
  `XRCE_STREAM_BEST_EFFORT` (`0x01`), `XRCE_STREAM_NONE` (`0x00`).

The submessage body is the `BaseObjectRequest` (DDS-XRCE 1.0 §7.7.8:
`request_id` then `object_id`, two octets each, most significant first),
followed by the sample. The flag byte is `0x01` — E flag (little-endian body)
plus `DataFormat` `FORMAT_DATA` (`0b000`) in bits 1..3 (§8.3.5.8);
`FORMAT_SAMPLE` (`0b001`, flag byte `0x03`) additionally requires a
`SampleInfo`, which this profile does not carry. The `request_id` is the
`sequence_nr` of the same message (§7.7.8 only requires uniqueness per
stream), the `object_id` names the target DataWriter — without it no
XRCE agent can assign the frame. The read path returns the sample behind these
four octets and rejects a body that is too short for them.
Measured by `crates/endpoint-e2e/tests/xrce_write_framing.rs`.

## §2 Sync `Client`

A non-blocking client for poll loops: `write` frames + delivers via
the transport; `poll` is a non-blocking single receive.

`endpoints/python/zerodds_endpoint.py` MUST provide:

- Duck typing instead of a formal interface: a transport is any object with
  `deliver(frame)` and `receive() -> bytes|None`. `MemTransport` (in-memory
  FIFO) is the bundled reference implementation for tests/examples;
  UDP or serial transports are drop-ins with the same signature.
- `class Client` with `Client(transport, session=XRCE_SESSION_NOKEY,
  stream=XRCE_STREAM_BEST_EFFORT)`, `.write(sample)` (frames + delivers, seq
  monotonically increasing mod 2¹⁶), `.poll()` (one non-blocking receive; returns
  the unframed body or `None`).

## §3 Async (asyncio receive pattern)

Unlike in Go — where `AsyncReader`/`AsyncWriter` are separate types
shipped in the SDK — `zerodds_endpoint.py` deliberately stays
2.7/3.x-compatible and import-clean without an `asyncio` dependency. Async is
therefore not a separate SDK type, but a documented, idiomatic
asyncio pattern **on the same transport duck type as `Client`**: an
`async def stream(self)` generator polls `transport.receive()` without blocking
and yields `ze.xrce_read_frame(frame)` for every frame that has arrived,
otherwise `await asyncio.sleep(...)`. The consumer iterates with `async for`. There
is no separate `AsyncWriter`: `Client.write()` is already
non-blocking (no waiting on I/O), the same write side serves sync
and async.

Reference implementations of this pattern (not a standalone SDK export,
but byte-identical in framing to §1/§2):

- `endpoints/python/example_async.py::AsyncReader` — async generator over
  `MemTransport`.
- The inline `AsyncReader` class of the E2E app in
  `crates/endpoint-e2e/tests/python.rs` (`run_async`) — the same pattern over
  a real `UdpTransport`.

## §4 Reliable stream

`endpoints/python/zerodds_reliable.py` implements the reliable stream as an
endpoint capability according to [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md)
— sender/receiver state machine, HEARTBEAT/ACKNACK wire codec and the
async-decoupled `ReliableWriter`.

The constants (`SENDER_WINDOW=16`, `RECEIVER_BUFFER=64`,
`HEARTBEAT_PERIOD_S=0.5`, `MAX_PAYLOAD=65535`, reliable stream ID `0x80`),
the state machine contract (`ReliableSender.submit`/`pending_heartbeat`/
`recv_acknack`/`get_in_flight`; `ReliableReceiver.recv_data`/
`drain_in_order`/`pending_acknack`/`reset`) and the wire format (HEARTBEAT
`0x0B`, ACKNACK `0x0A`, RFC 1982 16-bit sequence numbers via
`seq_lt`/`seq_gt`) are defined normatively there; `zerodds_reliable.py` is
the Python binding of this contract, byte-identical to
`crates/xrce/src/reliable.rs` and every other endpoint SDK.

**Honest note (producer decoupling, not a wait-free ring):** unlike the
Rust/C `ReliableAsyncWriter`, `ReliableWriter.enqueue()` is **not** a
wait-free ring buffer push, but a `queue.Queue.put()` — a
lock-protected deque append. What is real: a dedicated drain
`threading.Thread` holds the `ReliableSender` state and the entire socket I/O
(sending, heartbeat, ACKNACK-driven retransmit); the GIL is released around the
blocking `socket.send`/`recv` calls of the drain thread, so
that `enqueue()` never waits on a syscall and the drain thread meanwhile
works truly concurrently. This is "thread + GIL release around syscalls"
decoupling, not a lock-free data plane — the CPython GIL makes real
parallelism on the producer and drain side fundamentally impossible; what is decoupled
is the **syscall latency**, not the CPU time.

## §5 Conformance

A Python endpoint implementation is conformant if:

1. A raw UDP ping-pong with the generated `.encode()`/`.decode()`
   (without XRCE frame) runs byte-correctly with the Rust reference peer.
2. The full stack (generated types + `endpoints/python`) exchanges a typed
   sample both via the sync `Client` and via the asyncio pattern from
   §3 with the shared Rust XRCE peer.
3. HEARTBEAT and ACKNACK frames are byte-identical to the reference goldens
   and the reliable stream recovers datagram loss without gaps and in order
   (§4, `reliable-endpoint` v1.0 §5).
4. A latency measurement shows that `ReliableWriter.enqueue` is measurably below
   an inline `socket.send` — the evidence that the syscall latency is decoupled from
   the producer path (§4, honest note: thread + GIL release,
   not wait-free).

## §6 Examples

- Sync: `endpoints/python/example_sync.py` — poll loop over `MemTransport`,
  full field decode (`Reading(id, value, label)`).
- Async: `endpoints/python/example_async.py` — asyncio async generator
  `AsyncReader.stream()`.
- Reliable: `endpoints/python/example_reliable.py` — in-process demo (lossy
  receiver thread + `ReliableWriter`, no external peer needed), plus
  UDP sender mode (`run`) and latency bench mode (`bench`) for the E2E.
- Quickstart: `endpoints/python/QUICKSTART.md`.

## §7 Errata + open questions

None. Sync, the asyncio receive pattern and reliable are fully
implemented and byte-verified (see
`docs/spec-coverage/zerodds-endpoint-python-1.0.md`).
