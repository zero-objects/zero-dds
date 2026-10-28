<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-ada` v1.0 — Ada endpoint SDK (XRCE framing, sync/async, reliable stream)

ZeroDDS vendor spec. Implemented in `endpoints/ada/`.

Complements the codegen spec [`zerodds-xcdr2-ada`](zerodds-xcdr2-ada-1.0.md): there the
wire mapping IDL→Ada (ADR 0013 stage 1: `Interfaces.C` bindings over the
C89 wire core `zdw`), here the endpoint building block that carries an XCDR2-marshalled sample
as an XRCE frame over a UDP transport — sync via a protected
`Mailbox` object, async via an Ada task, optionally reliable (§4).

## §1 XRCE framing

A WRITE_DATA sample is framed as an 8-byte header + body (DDS-XRCE 1.0
§8.3.2/§8.3.4), byte-identical to `crates/xrce` + `endpoints/c`:

```
[session][stream][seq_lo][seq_hi][0x07][flags][len_lo][len_hi][body...]
```

- `session` — no-key session (`0x80`).
- `stream` — `0x01` (best-effort) on the sync/async path; `stream_id ≥ 128`
  (bit 7 set) on the reliable path (§4, see DDS-XRCE 1.0 §8.3.2).
- `seq` — 16-bit LE sequence number.
- Submessage id `0x07` (WRITE_DATA); `len` — 16-bit LE body length.
- `body` — the XCDR2-encoded sample, bound via `endpoints/ada/src/zdw.ads`
  (package `Zdw`, `Interfaces.C` FFI over the C89 wire core), byte-identical to the
  `zerodds-cdr` reference (see `zerodds-xcdr2-ada-1.0` §3/§5).

`Deep_Reading.Frame` / `Deep_Reading.Deframe` (`endpoints/ada/src/deep_reading.ads`)
MUST build and parse this frame for the best-effort stream (`0x01`);
`Reliable.Write_Frame` (`endpoints/ada/src/reliable.adb`) for the reliable stream
(`stream_id = 0x80`, §4).

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

## §2 sync client

A blocking receive path via the idiomatic Ada concurrency primitive: the
protected object `Deep_Reading.Mailbox`.

MUST:

- `Mailbox.Deliver(Frame)` — puts a received, decoded frame into the
  protected FIFO.
- `Mailbox.Receive(Frame)` / `Mailbox.Try_Receive(Frame, Success)` — the
  blocking and the non-blocking entry respectively; the integrator owns the
  poll/wait loop, no background thread on this path.

Reference path: [`example_sync.adb`](../../endpoints/ada/test/example_sync.adb) — a
poll loop over `Mailbox.Try_Receive`, complete field decode. Ping-pong proof
against the shared Rust XRCE peer: `crates/endpoint-e2e/tests/ada.rs` (sync mode,
real UDP socket).

## §3 async reader/writer

A background reader as its own Ada task (`Reader_Task`) — no async runtime
beyond `task`/`protected object`, the idiomatic Ada concurrency model.

MUST:

- `Reader_Task` — drains the transport in a loop, decodes every
  WRITE_DATA body and stores it via `Mailbox.Deliver`.
- The calling task blocks on `Mailbox.Receive` (the same entry as in §2) —
  no separate callback mechanism.

Reference path: [`example_async.adb`](../../endpoints/ada/test/example_async.adb) —
a `Reader_Task` + a protected `Mailbox`; the main task blocks on
`Inbox.Receive`. Ping-pong proof: `crates/endpoint-e2e/tests/ada.rs`
(async mode).

A decoupled async **writer** is not a building block of its own at this level, but
part of the reliable stream (§4).

## §4 reliable stream

The reliable stream is an optional extension of §1–§3: state machine,
wire codec and async-decoupled writer according to
[`reliable-endpoint` v1.0](reliable-endpoint-1.0.md). Ada implements the
canonical contract (§3 there) in `endpoints/ada/src/reliable.{ads,adb}`:

- Sender — `Submit`/`Pending_Heartbeat`/`Recv_Acknack`/`Get_In_Flight`.
- Receiver — `Recv_Data`/`Drain_Next`/`Pending_Acknack`/`Reset`.
- Wire codec — `Heartbeat_Frame`/`Acknack_Frame` + the corresponding `Parse_*`
  functions; byte-identical to `golden_heartbeat_le.bin`/`golden_acknack_le.bin`.
- `Reliable.Send_Ring` — a protected ring (`Enqueue`/`Dequeue`/`Close`) as an
  async-decoupled writer: the producer only does a wait-free `Enqueue`,
  no syscall; a dedicated drain task
  ([`example_reliable.adb`](../../endpoints/ada/test/example_reliable.adb),
  `GNAT.Sockets`) owns the socket and the sender state and does all the I/O (send,
  heartbeat, ACKNACK-driven retransmit).

Constants, error cases, wire format and the test/evidence requirement are defined normatively in
`reliable-endpoint-1.0` §3–§5 and apply unchanged to Ada.
