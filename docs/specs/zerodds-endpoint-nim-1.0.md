<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-nim` v1.0 — Nim endpoint SDK: XRCE framing, sync/async, reliable stream

**Status:** normative.

ZeroDDS vendor spec. Implemented in `endpoints/nim/`. Extends
[`zerodds-xcdr2-nim`](zerodds-xcdr2-nim-1.0.md) (IDL codegen/marshalling) with the
transport/delivery layer: XRCE framing, the sync `Client`, the async
`AsyncReader` and — for reliable delivery — the contract from
[`reliable-endpoint`](reliable-endpoint-1.0.md).

## §1 XRCE framing

`endpoints/nim/zerodds.nim` MUST provide `writeFrame`/`readFrame` as a DDS-XRCE 1.0
WRITE_DATA submessage (§8.3.2 MessageHeader, §8.3.4
SubmessageHeader, §8.3.5 WRITE_DATA, submessage id `0x07`): an 8-byte header —
session (`0x80` no-key), stream (`0x01` best-effort; `≥ 0x80` reliable, §8.3.2
bit 7), sequence number LE, submessage id, flags, length LE — followed by the
XCDR2 sample body. Byte-identical to `crates/xrce` and every other
endpoint SDK. `readFrame` is the exact inverse: parse the header, return the body
limited to the size given in the length field.

The submessage body is the `BaseObjectRequest` (DDS-XRCE 1.0 §7.7.8:
`request_id` then `object_id`, two octets each, most significant first),
followed by the sample. The flag byte is `0x01` — E flag (little-endian body)
plus `DataFormat` `FORMAT_DATA` (`0b000`) in bits 1..3 (§8.3.5.8);
`FORMAT_SAMPLE` (`0b001`, flag byte `0x03`) additionally requires a
`SampleInfo`, which this profile does not carry. The `request_id` is the
`sequence_nr` of the same message (§7.7.8 only requires uniqueness per
stream), the `object_id` names the target DataWriter — without it no
XRCE agent can assign the frame. The read path returns the sample after these
four octets and rejects a body that is too short for them.
Measured by `crates/endpoint-e2e/tests/xrce_write_framing.rs`.

## §2 Sync client

`Client`/`newClient` MUST provide a non-blocking poll client over a
UDP socket: `write(sample: seq[byte])` frames the body via §1
and sends it; `poll(): Option[seq[byte]]` reads non-blocking and
returns `some(body)` for a complete frame, otherwise `none`. The
sequence number is a monotonic `uint16` counter that wraps around at `0xffff`
(no error, no reset).

## §3 Async reader

`AsyncReader`/`newAsyncReader` MUST provide the idiomatic Nim `async`/`await` path:
`recv(): Future[seq[byte]]` is an `{.async.}` procedure that
only resolves once a complete frame has been received and deframed.
Internally, polling MUST happen via `sleepAsync` (cooperative yielding via
`asyncdispatch`), not via a busy spin visible to the caller.

## §4 Reliable stream

Reliable delivery MUST follow the contract from [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md):

- **State machine (§3):** sender `submit`/`pendingHeartbeat`/`recvAcknack`/
  `getInFlight`; receiver `recvData`/`drainInOrder`/`pendingAcknack`/`reset`
  — idiomatic Nim names, semantics identical to the reference
  `crates/xrce/src/reliable.rs`.
- **Constants (§3.1):** `SENDER_WINDOW = 16`, `RECEIVER_BUFFER = 64`,
  `HEARTBEAT_PERIOD = 500ms`, `MAX_PAYLOAD = 65535`, RFC-1982 16-bit
  sequence numbers, reliable stream id `≥ 128` (bit 7 set).
- **Wire (§4):** HEARTBEAT (`0x0B`) and ACKNACK (`0x0A`) byte-identical to the
  reference goldens (`golden_heartbeat_le.bin`/`golden_acknack_le.bin`).
- **Async decoupling (§2/§5):** the producer MUST never enter the kernel — it
  enqueues wait-free or lock-based into a queue; a dedicated
  drain thread owns the socket and the sender state (send WRITE_DATA,
  periodic HEARTBEAT, ACKNACK reception, retransmit from the history). The
  concrete queue implementation (wait-free ring vs. lock-based
  stdlib `Channel`) is not prescribed; a latency measurement
  (`enqueue` vs. inline `sendto`) that shows the decoupling is mandatory
  (§5 item 4 of the reference spec). `endpoints/nim` uses a
  single-producer-single-consumer ring without a lock (`spsc_ring.nim`,
  checked by `spsc_ring_test.nim`); when it is full, the producer waits for
  the drain thread.
- **Test/evidence obligation (§5):** unit (state machine mirror), byte-golden
  (HEARTBEAT/ACKNACK), E2E loss recovery live against the shared Rust peer
  with injected drop, latency bench, a runnable
  `example_reliable_*` — all five apply unchanged to `endpoints/nim`.
