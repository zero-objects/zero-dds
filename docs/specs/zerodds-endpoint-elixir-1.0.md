<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-elixir` v1.0 — Elixir endpoint SDK

**Status:** normative · ZeroDDS vendor spec. Implemented in `endpoints/elixir/`.

Analogous to [`zerodds-xcdr2-elixir`](zerodds-xcdr2-elixir-1.0.md) (there the
marshalling) and the endpoint SDKs of other languages (`endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/c`, ...): the native
Elixir endpoint over XRCE framing, sync `Client`, async
`AsyncReader`/`AsyncWriter` (BEAM process/mailbox) and the reliable stream, so
that an Elixir app exchanges an XCDR2 sample byte-identical to `crates/xrce` +
`endpoints/c` with the shared Rust peer. No native addon, no
NIF: Elixir binaries + bitstring syntax carry the wire.

## §1 XRCE framing

An 8-byte XRCE header (`session`, `stream`, `seq` LE, submessage ID `0x07`
WRITE_DATA, `flags`, `len` LE) followed by the XCDR2 sample body — byte-identical
to `crates/xrce` and `endpoints/c`.

`endpoints/elixir` (module `ZeroDDS.Endpoint`) MUST provide:

- `write_frame(session, stream, seq, sample)` — frames a sample.
- `read_frame(frame)` — deframes; `:error` on a frame that is too short or a wrong
  submessage ID.
- Constants `session_nokey/0` (`0x80`, best-effort, without ClientKey) and
  `stream_best_effort/0` (`0x01`).

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

A blocking client — counterpart to the process-based `AsyncReader`:
`write/2` frames + delivers synchronously via the `transport`; `poll/1` is a
non-blocking single receive (no built-in `receive/2` with timeout —
the caller polls itself in a deadline loop, see
`example_sync.exs`).

`endpoints/elixir` (module `ZeroDDS.Client`) MUST provide:

- A `transport` contract `%{deliver: fun/1, receive: fun/0}` — the only
  integration point; the integrator implements it for its link (e.g.
  `:gen_udp`, or the in-memory `ZeroDDS.MemTransport` for tests/examples).
- `new(transport)` — client with default `session`/`stream`
  (`ZeroDDS.Endpoint.session_nokey/0`/`stream_best_effort/0`) and `seq: 1`.
- `write(client, sample)` — frames, calls `transport.deliver.(frame)`,
  returns the client with an incremented (modulo `0x10000`) sequence number
  (Elixir structs are immutable — every call returns the new
  state).
- `poll(client)` — a non-blocking single receive via
  `transport.receive.()`; returns the deframed sample body or `nil`.

## §3 Async `Reader`/`Writer`

The idiomatic BEAM async model: a spawned process polls the
`transport` and sends deframed sample bodies as a
`{:zerodds_sample, body}` message to the `target` (push into the mailbox
of the consumer) instead of via a channel. There is no separate
`AsyncWriter` type: the sync `Client` (§2) already is the send path — producer
and `AsyncReader` share the same `transport`.

`endpoints/elixir` (module `ZeroDDS.AsyncReader`) MUST provide:

- `start(transport, target)` — spawns the receive process (`spawn/1`), which
  polls `transport.receive.()` in a loop and, on a deframed frame, sends
  `{:zerodds_sample, body}` to `target`; on an empty poll a short
  `Process.sleep(1)` before the next attempt.
- `stop(pid)` — sends `:zerodds_stop`; the process terminates after the
  running poll iteration.
- The same XRCE framing from §1 and the same `transport` contract as the sync
  `Client`.

## §4 Reliable stream

`endpoints/elixir` implements the reliable stream as an endpoint capability
according to [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — sender/
receiver state machine (module `ZeroDDS.Reliable`, `Sender`/`Receiver` as
immutable structs, since BEAM has no mutation — every call threads the
state through and returns the new one) as well as the HEARTBEAT/ACKNACK wire codec.

The constants (`sender_window/0`=16, `receiver_buffer/0`=64,
`heartbeat_period_ms/0`=500, `max_payload/0`=65535, reliable stream ID
`0x80`), the state machine contract (`Sender.submit/2`/`pending_heartbeat/2`/
`recv_acknack/2`/`get_in_flight/2` on the sender; `Receiver.recv_data/3`/
`drain_in_order/1`/`pending_acknack/2`/`reset/1` on the receiver) and the
wire format (HEARTBEAT `0x0B`, ACKNACK `0x0A`, RFC-1982 16-bit
sequence numbers) are defined normatively there; `endpoints/elixir/lib/reliable.ex`
is the Elixir binding of this contract, byte-identical to
`crates/xrce/src/reliable.rs` and every other endpoint SDK.

The async-decoupled sender is `ZeroDDS.Reliable.Drain`, a `GenServer`
instead of a wait-free ring: the producer calls `submit/2` (a `GenServer.cast`,
pays one mailbox send and returns immediately, without entering the
kernel), the drain process exclusively holds the `Sender` state and the
`:gen_udp` socket and handles submit-into-history, WRITE_DATA send, periodic
HEARTBEAT (tick every 50 ms) and ACKNACK-driven retransmit away from the
producer path. `finish/2` blocks (`GenServer.call`) until the send window
is completely drained (everything acked). Socket ownership order (BEAM
peculiarity): `start_link` (no socket I/O in `init/1`) → `:gen_udp.controlling_process`
(still called by the current owner) → `activate/1` (grants the
drain process permission to set `active: true` and to start the
ticks).

## §5 Conformance

An Elixir endpoint implementation is conformant if:

1. A raw UDP ping-pong with the generated `marshal_xcdr`/`unmarshal`
   (without XRCE frame) runs byte-correct with the Rust reference peer.
2. The full stack (generated types + `endpoints/elixir`) exchanges a typed
   sample both via the sync `Client` and via the `AsyncReader`
   (process/mailbox) with the shared Rust XRCE peer.
3. HEARTBEAT and ACKNACK frames are byte-identical to the reference goldens
   and the reliable stream recovers datagram loss in order without gaps (§4,
   `reliable-endpoint` v1.0 §5).
4. A latency measurement shows that `ZeroDDS.Reliable.Drain.submit/2`
   (`GenServer.cast`, mailbox send) is measurably below an inline
   `:gen_udp.send` — the evidence that the BEAM message passing decoupling
   takes the syscall latency out of the producer path.

## §6 Examples

- Sync: `endpoints/elixir/example_sync.exs` — poll loop with full
  field decode of a `Reading { id, value, label }`.
- Async: `endpoints/elixir/example_async.exs` — process/mailbox `AsyncReader`,
  same telemetry example.
- Reliable: `endpoints/elixir/example_reliable.exs` — in-process demo (no
  sockets) of the loss recovery over 12 samples.
- Quickstart: `endpoints/elixir/QUICKSTART.md`.

## §7 Errata + open questions

None. Sync, async and reliable are fully implemented and
byte-verified (see
`docs/spec-coverage/zerodds-endpoint-elixir-1.0.md`).
