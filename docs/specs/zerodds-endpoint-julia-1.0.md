<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-julia` v1.0 — Julia endpoint SDK

**Status:** normative · ZeroDDS vendor spec. Implemented in `endpoints/julia/`.

Analogous to [`zerodds-xcdr2-julia`](zerodds-xcdr2-julia-1.0.md) (the
marshalling is there) and the endpoint SDKs of other languages (`endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/d`, `endpoints/c`,
`endpoints/ada`, ...): the native pure Julia endpoint over XRCE framing, the
polling `Client`, the `Task`+`Channel`-based `AsyncReader` and the
reliable stream, so that a Julia app exchanges an XCDR2 sample byte-identically to
`crates/xrce` + `endpoints/c` with the shared Rust peer.

## §1 XRCE framing

An 8-byte XRCE header (`session`, `stream`, `seq` LE, submessage ID `0x07`
WRITE_DATA, `flags`, `len` LE) followed by the XCDR2 sample body — byte-identical
to `crates/xrce` and `endpoints/c`.

`endpoints/julia/zerodds.jl` (`module ZeroDDS`) MUST provide:

- `write_frame(session, stream, seq, sample::Vector{UInt8}) -> Vector{UInt8}`
  — frames a sample.
- `read_frame(frame::Vector{UInt8}) -> Union{Vector{UInt8},Nothing}` —
  unframes; `nothing` for a frame that is too short or a wrong submessage ID.
- Constants `SESSION_NOKEY` (`0x80`, best-effort, without ClientKey) and
  `STREAM_BEST_EFFORT` (`0x01`).

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

A polling client — the counterpart to the `Task`-based `AsyncReader`: `write!`
frames + delivers synchronously via the `Transport`; `poll` is a
non-blocking single receive (`nothing` when empty).

`endpoints/julia/zerodds.jl` MUST provide:

- `struct Transport(deliver::Function, receive::Function)` — the only
  integration point; the integrator supplies their own `deliver`/`receive`
  closures for their link (e.g. UDP, or the in-memory `mem_transport()`
  for tests/examples).
- `mutable struct Client(transport, session, stream, seq)` with
  `Client(t::Transport)`, `write!(c::Client, sample::Vector{UInt8})`,
  `poll(c::Client) -> Union{Vector{UInt8},Nothing}`.
- A monotonically increasing sequence number per `write!` (16-bit wraparound); by
  default `SESSION_NOKEY`/`STREAM_BEST_EFFORT`.

## §3 Async `Reader`/`Writer`

The idiomatic Julia async model: an `@async` `Task` polls the `Transport`
and pushes unframed sample bodies onto a buffered `Channel` (push);
the consumer blocks on `take!`. There is no separate `AsyncWriter` type
— sending stays `write!` on the sync `Client` (same framing, no
back-channel state of its own needed); the reliable `ReliableAsyncWriter` mechanism
(§4) is the place where send decoupling is normatively required.

`endpoints/julia/zerodds.jl` MUST provide:

- `mutable struct AsyncReader(ch::Channel{Vector{UInt8}}, running::Ref{Bool})`
  with `start_reader(t::Transport) -> AsyncReader` (spawns the receive `Task`
  via `@async`, drains the `Transport` in a loop with a
  backoff `sleep` on an empty poll) and `stop!(r::AsyncReader)` (sets
  `running[] = false`, the `Task` ends itself).
- `recv(r::AsyncReader) -> Vector{UInt8}` — blocking `take!(r.ch)`.
- The `AsyncReader` shares the XRCE framing from §1 and the same
  `Transport` contract as the sync `Client`.

## §4 Reliable stream

`endpoints/julia/reliable.jl` (`module Reliable`) implements the reliable
stream as an endpoint capability according to [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md)
— sender/receiver state machine, HEARTBEAT/ACKNACK wire codec and the
async-decoupled `ReliableAsyncWriter` mechanism: a producer-side
`Channel` (wait-free enqueue from the producer's point of view) plus a dedicated
drain `Task` (`@async`) that owns the `UDPSocket` and the reliable sender state,
sends, emits HEARTBEATs and retransmits on ACKNACK —
the producer never goes into the kernel.

The constants (`SENDER_WINDOW=16`, `RECEIVER_BUFFER=64`,
`HEARTBEAT_PERIOD_MS=500`, `MAX_PAYLOAD=65535`, reliable stream ID `0x80`),
the state machine contract (`submit!`/`pending_heartbeat!`/`recv_acknack!`/
`get_in_flight` on the `Sender`; `recv_data!`/`drain_in_order!`/
`pending_acknack`/`reset!` on the `Receiver`) and the wire format
(HEARTBEAT `0x0B`, ACKNACK `0x0A`, RFC-1982 16-bit sequence numbers via
`seq_lt`/`seq_gt`) are defined normatively there; `endpoints/julia/reliable.jl`
is the Julia binding of this contract, byte-identical to
`crates/xrce/src/reliable.rs` and every other endpoint SDK. The module is
encapsulated in `module Reliable` to avoid name collisions with `module ZeroDDS`
(`write_frame`, `Client`, ...).

## §5 Conformance

A Julia endpoint implementation is conformant if:

1. The full stack (generated types + `endpoints/julia`) exchanges a typed
   sample both via the sync `Client` and via the `Task`/`Channel`-based
   `AsyncReader` with the shared Rust XRCE peer over a real
   UDP socket.
2. HEARTBEAT and ACKNACK frames are byte-identical to the reference goldens
   and the reliable stream recovers datagram loss without gaps, in order
   (§4, `reliable-endpoint` v1.0 §5).
3. A latency measurement shows that the `ReliableAsyncWriter` enqueue (`put!` on
   the producer-side `Channel`) is measurably below an inline `send`
   (UDP `sendto`) — the evidence that async write takes the syscall latency out of
   the producer path.

## §6 Examples

- Sync: `endpoints/julia/example_sync.jl` — poll loop, full field decode.
- Async: `endpoints/julia/example_async.jl` — `Task`/`Channel` `AsyncReader`.
- Reliable: `endpoints/julia/example_reliable.jl` — in-process demo (no
  socket) of the loss recovery.
- Quickstart: `endpoints/julia/QUICKSTART.md`.

## §7 Errata + open questions

None. Sync, async and reliable are fully implemented and
byte-verified (see `docs/spec-coverage/zerodds-endpoint-julia-1.0.md`).
The `ReliableAsyncWriter` producer path uses Julia's `Channel` (lock+condvar,
no wait-free SPSC ring); the enqueue is nevertheless measurably cheaper than
the inline syscall (§5.3) — a real wait-free ring would be a separate
optimization, not a spec violation (see `endpoints/julia/reliable_app.jl`).
