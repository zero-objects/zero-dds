<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-ocaml` v1.0 — OCaml endpoint SDK

**Status:** normative · ZeroDDS vendor spec. Implemented in `endpoints/ocaml/`.

Analogous to [`zerodds-xcdr2-ocaml`](zerodds-xcdr2-ocaml-1.0.md) (there the
marshalling) and the endpoint SDKs of other languages (`endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/c`, `endpoints/ada`, ...): the
native, pure-OCaml endpoint (ADR 0013) over XRCE framing, sync `Client`, async
`AsyncReader` (a `Thread` that fills a mutex/condition mailbox) and the
reliable stream, so that an OCaml app exchanges an XCDR2 sample byte-identical to
`crates/xrce` + `endpoints/c` with the shared Rust peer — without
Lwt or Async, only the stdlib (`Thread`, `Mutex`, `Condition`, `Unix`) via
`threads.posix`.

## §1 XRCE framing

An 8-byte XRCE header (`session`, `stream`, `seq` LE, submessage ID `0x07`
WRITE_DATA, `flags`, `len` LE) followed by the XCDR2 sample body — byte-identical
to `crates/xrce` and `endpoints/c`.

`endpoints/ocaml/zerodds.ml` (module `Endpoint`) MUST provide:

- `write_frame : int -> int -> int -> bytes -> bytes` — frames a sample
  (`session`, `stream`, `seq`, `sample`).
- `read_frame : bytes -> bytes option` — deframes; `None` on a frame that is too short
  or a wrong submessage ID.
- Constants `session_nokey` (`0x80`, best-effort, without ClientKey) and
  `stream_best_effort` (`0x01`).

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

## §2 Sync `Client`

A blocking client — counterpart to the thread-based `AsyncReader`:
`write` frames + delivers synchronously via the `transport`; `poll` is a
non-blocking single receive.

`endpoints/ocaml/zerodds.ml` MUST provide:

- `type transport = { deliver : bytes -> unit; receive : unit -> bytes option }`
  — the only integration point; the integrator implements it for its
  link (e.g. UDP via `Unix.sendto`/`Unix.recvfrom`).
- `module Client` with `create : transport -> t`, `write : t -> bytes -> unit`,
  `poll : t -> bytes option`.
- A monotonically increasing sequence number (16-bit wraparound) per `write`;
  `session`/`stream` default to `session_nokey`/`stream_best_effort`.

## §3 Async `Reader`/`Writer`

The idiomatic OCaml stdlib async model (no Lwt, no Async): a
`Thread` polls the `transport` and puts deframed sample bodies into a
`Mailbox` (mutex + condition, FIFO list); the consumer blocks in
`recv` on `Condition.wait`.

`endpoints/ocaml/zerodds.ml` MUST provide:

- `module Mailbox` — a generic mutex/condition FIFO (`put`, `take`), the
  basis both of the ping-pong `AsyncReader` and (in `reliable.ml`)
  of the `ReliableAsyncWriter`.
- `module AsyncReader` with `start : transport -> t` (spawns the
  receive `Thread`), `recv : t -> bytes` (blocks until a
  deframed sample arrives) and `stop : t -> unit` (stops the poll loop).
- Sending over the async path shares the XRCE framing from §1 and the
  `transport` contract with the sync `Client`: an app frames with
  `Endpoint.write_frame` and delivers directly via `transport.deliver`
  (no separate `AsyncWriter` module for the unreliable path — see §4 for
  the reliable async writer with its own drain `Thread`).

## §4 Reliable stream

`endpoints/ocaml/reliable.ml` implements the reliable stream as an
endpoint capability according to [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md)
— sender/receiver state machine, HEARTBEAT/ACKNACK wire codec as well as the
async-decoupled `Reliable.Writer`, whose drain `Thread` owns the UDP `Unix`
socket and the reliable sender state, while the producer only
enqueues wait-free onto a mutex/condition `Mailbox` (`Mailbox.put` — lock the
mutex, cons, signal, unlock — never enters the kernel).

The constants (`window=16`, `recv_buf=64`, `heartbeat_period=0.5s`,
`max_payload=65535`, reliable stream ID `0x80`), the state machine contract
(`Sender.submit`/`pending_heartbeat`/`recv_acknack`/`get_in_flight`;
`Receiver.recv_data`/`drain_in_order`/`pending_acknack`/`reset`) and the
wire format (HEARTBEAT `0x0B`, ACKNACK `0x0A`, RFC-1982 16-bit
sequence numbers) are defined normatively there; `endpoints/ocaml/reliable.ml`
is the OCaml binding of this contract, byte-identical to
`crates/xrce/src/reliable.rs` and every other endpoint SDK. The module is called
`Reliable` (file-as-module) and deliberately does not depend on `zerodds.ml` —
both independently define a `module Wire`/their own wire primitives, so that
an app that links generated `idlc` code (which also exports a `module Wire`)
next to the SDK does not collide.

## §5 Conformance

An OCaml endpoint implementation is conformant if:

1. The full stack (generated types from `zerodds-idlc generate --lang ocaml` +
   `endpoints/ocaml`) exchanges a typed sample both via the sync `Client`
   and via `AsyncReader` with the shared Rust XRCE peer over a
   real UDP socket.
2. HEARTBEAT and ACKNACK frames are byte-identical to the reference goldens
   and the reliable stream recovers datagram loss in order without gaps
   (§4, `reliable-endpoint` v1.0 §5).
3. A latency measurement shows that `Reliable.Writer.enqueue`
   (`Mailbox.put` — mutex+cons+signal, no syscall) is measurably below an
   inline `Unix.sendto` — the evidence that async write takes the
   syscall latency out of the producer path.

## §6 Examples

- Sync: `endpoints/ocaml/example_sync.ml` — `Client.poll` in a loop,
  full field decode.
- Async: `endpoints/ocaml/example_async.ml` — `AsyncReader.start` +
  blocking `recv` on the mutex/condition mailbox.
- Reliable: `endpoints/ocaml/example_reliable.ml` — in-process demo (no
  socket) of the loss recovery.
- Quickstart: `endpoints/ocaml/QUICKSTART.md`.

## §7 Errata + open questions

None. Sync, async and reliable are fully implemented and
byte-verified (see `docs/spec-coverage/zerodds-endpoint-ocaml-1.0.md`).
