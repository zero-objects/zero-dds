<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-zig` v1.0 — Zig endpoint SDK: XRCE framing, sync/async, reliable stream

**Status:** normative · ADR 0013 (native endpoint SDKs, no full stack).

ZeroDDS vendor spec. Implemented in `endpoints/zig/`. Builds on DDS-XRCE 1.0 §8.3
(message/submessage framing, WRITE_DATA/DATA) and [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md)
(reliable stream, §8.4.10/§8.4.11). Extends the codegen spec
[`zerodds-xcdr2-zig`](zerodds-xcdr2-zig-1.0.md) (marshalling of the sample bodies) with the
transport: framing, sync/async delivery, reliable delivery.

## §1 XRCE framing

The endpoint is transport-opaque (ADR 0013, invariant 5): it frames and deframes, but delivers
the complete frame via a `Transport` filled in by the integrator — no
socket of its own in the SDK.

The pure Zig wire core (`endpoints/zig/src/zerodds.zig`, module `zerodds`) MUST
provide:

- `XRCE_SESSION_NOKEY: u8 = 0x80` and `XRCE_STREAM_BEST_EFFORT: u8 = 0x01`.
- `xrceWriteFrame(out, session, stream, seq, sample) FrameError!usize` — builds the
  8-byte XRCE header (`session_id`, `stream_id`, `sequence_nr` LE,
  WRITE_DATA submessage id `0x07`, flags, length LE) + the body byte-identical to
  `crates/xrce` and `endpoints/c`. A sample that the frame cannot describe
  MUST come back as an error (`SampleTooLarge`, `BufferTooSmall`) and not as
  length zero.
- `xrceReadFrame(frame) ?[]const u8` — the byte-exact inverse: deframes WRITE_DATA and
  DATA messages (§8.3.4) alike and returns the body.
- `Transport` as a function pointer vtable (`deliver`/`receive`), without heap allocation —
  the integrator binds the real channel (in-memory, UDP, serial, …).

The submessage body MUST be the `BaseObjectRequest` (DDS-XRCE 1.0 §7.7.8:
`request_id` then `object_id`, two octets each, most significant first),
followed by the sample; the flag byte MUST be `0x01` — E flag
(little-endian body) plus `DataFormat` `FORMAT_DATA` (`0b000`) in bits
1..3 (§8.3.5.8).

Both Zig framings satisfy this: `xrceWriteFrame` writes
`XRCE_WRITE_FLAGS` (`0x01`), `endpoints/zig/src/reliable.zig`'s
`writeDataFrame` writes `WRITE_FLAGS` (`0x01`), and both put the
`BaseObjectRequest` before the sample. Measured by
`crates/endpoint-e2e/tests/xrce_write_framing.rs` — the columns
`zig_endpoint_frames_conformant_write_data` and
`zig_reliable_frames_conformant_write_data` pass the built frame to
`zerodds_xrce::WriteDataPayload::try_from_submessage`.

## §2 sync client

A blocking pull path for callers that own the run loop themselves (no
async/await in Zig).

`Client` MUST provide:

- `write(sample) FrameError!bool` — frames the XCDR2 body with a monotonic `seq` and
  delivers it via the `Transport`; a sample that cannot be framed is an error, and the
  `seq` is then not advanced.
- `poll() ?[]const u8` — receives a frame via the `Transport` and returns the
  deframed body, or `null`.

Fixed `txbuf`/`rxbuf` (no runtime allocation for framing).

## §3 async reader/writer

Zig has no async/await; the async receive path is a callback reactor as the
counterpart to the sync `poll`.

`AsyncReader` MUST provide:

- `OnSample = *const fn (ctx: *anyopaque, body: []const u8) void` — the
  consumer callback.
- `poll() bool` — dispatches at most one ready frame to `on_sample`.
- `run(max) usize` — drains up to `max` frames or until the `Transport` delivers nothing
  more; return value = number of dispatched frames.

A standalone best-effort `AsyncWriter` (counterpart without reliability) is not part
of this SDK for Zig — the async-decoupled write side is covered exclusively by the
reliable `AsyncWriter` (§4), whose wait-free producer path delivers the same
latency advantage, plus reliable delivery.

## §4 reliable stream

The reliable stream (sender/receiver state machine, wire format, async writer contract,
test and evidence obligation) is standardised across languages in [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md).
The Zig implementation (`endpoints/zig/src/reliable.zig`) MUST mirror this contract
1:1:

- `Sender.submit`/`pendingHeartbeat`/`recvAckNack`/`getInFlight` — §3.2 of the reference.
- `Receiver.recvData`/`drainInto`/`pendingAckNack`/`reset` — §3.3 of the reference.
- `writeDataFrame`/`heartbeatFrame`/`acknackFrame` (+ parser) — byte-golden against the
  reference goldens, §4 of the reference.
- `AsyncWriter` — wait-free SPSC ring enqueue (`RING_CAP`/`SLOT_CAP`) on the
  producer path, a drain thread holds the `Sender` state and does all of the I/O
  (send, heartbeat, ACKNACK-driven retransmit); the producer never enters the
  kernel — §2 of the reference.
- Constants `SENDER_WINDOW=16`, `RECEIVER_BUFFER=64`, `MAX_PAYLOAD=65535`,
  `HEARTBEAT_PERIOD_MS=500`, `RELIABLE_STREAM_ID=0x80` — §3.1 of the reference.

Test and evidence obligation (unit, byte-golden, E2E loss recovery, latency bench, example) —
§5 of the reference.
