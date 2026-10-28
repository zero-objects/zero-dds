<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-csharp` v1.0 — C# endpoint SDK

**Status:** normative · ZeroDDS vendor spec. Implemented in `endpoints/csharp/`.

Analogous to [`zerodds-xcdr2-csharp`](zerodds-xcdr2-csharp-1.0.md) (there the
marshalling) and the endpoint SDKs of other languages (`endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/c`, `endpoints/ada`, ...): the
native C# endpoint (ADR 0013, from scratch, no DDS vendor binding) over
XRCE framing, sync `Client`, async `AsyncReader`/`Stream()` (idiomatic
`IAsyncEnumerable`) and the reliable stream, so that a C# app exchanges an
XCDR2 sample byte-identical to `crates/xrce` + `endpoints/c` with the
shared Rust peer.

## §1 XRCE framing

An 8-byte XRCE header (`session`, `stream`, `seq` LE, submessage ID `0x07`
WRITE_DATA, `flags`, `len` LE) followed by the XCDR2 sample body — byte-identical
to `crates/xrce` and `endpoints/c`.

`endpoints/csharp` (namespace `ZeroDDS`) MUST provide:

- `Xrce.WriteFrame(ushort seq, byte[] sample) -> byte[]` — frames a sample.
- `Xrce.ReadFrame(byte[] frame) -> byte[]?` — unframes; `null` for a frame that is too short
  or a wrong submessage ID.
- Constants `Xrce.SessionNoKey` (`0x80`, best-effort, without ClientKey) and
  `Xrce.StreamBestEffort` (`0x01`).

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

A blocking client — counterpart to the `AsyncReader`: `Write` frames +
delivers synchronously via the `ITransport`; `Poll` is a non-blocking
single receive (`null` if nothing is pending).

`endpoints/csharp` MUST provide:

- `interface ITransport { void Deliver(byte[] frame); byte[]? Receive(); }`
  — the only integration point; the integrator implements it for
  their link (e.g. UDP, see `csharp_endpoint_sync`/`csharp_endpoint_async`
  in `crates/endpoint-e2e/tests/csharp.rs`).
- `sealed class Client` with `Client(ITransport transport)`,
  `Write(byte[] sample) -> void`, `Poll() -> byte[]?`.
- A monotonically increasing `ushort` sequence number per `Write`; `Session`/`Stream`
  by default `Xrce.SessionNoKey`/`Xrce.StreamBestEffort`.
- `MemTransport` as a lock-based in-memory reference implementation of
  `ITransport` (for examples/tests without a real socket).

## §3 Async `Reader`/`Writer`

The idiomatic C# async model: `AsyncReader.Stream()` returns an
`IAsyncEnumerable<byte[]>` that the consumer iterates with `await foreach` —
no separate receive task, no channel; the polling is in the
enumerator itself (non-blocking `Receive()` per tick, otherwise
`Task.Delay(1)` until cancellation).

`endpoints/csharp` MUST provide:

- `sealed class AsyncReader` with `AsyncReader(ITransport transport)` and
  `IAsyncEnumerable<byte[]> Stream(CancellationToken ct = default)`.
- On the sending side the async path shares the sync `Client.Write` (same
  framing, same `ITransport` contract) — there is no separate
  `AsyncWriter` type for the best-effort stream; the reliable stream has
  its own async-decoupled writer (§4).

## §4 Reliable stream

`endpoints/csharp` implements the reliable stream as an endpoint capability
according to [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — sender/
receiver state machine, HEARTBEAT/ACKNACK wire codec and the
async-decoupled `AsyncReliableWriter`, whose dedicated drain `Thread`
owns the UDP `Socket` and the reliable sender state, while the producer
enqueues wait-free via `ReliableWriterHandle.Enqueue` (non-blocking `Channel` write,
`System.Threading.Channels`) and never enters the kernel.

The constants (`ReliableWire.Window=16`, `ReliableWire.ReceiverBuffer=64`,
`ReliableWire.HeartbeatPeriod=500ms`, `ReliableWire.MaxPayload=65535`,
reliable stream ID `0x80`), the state machine contract (`Submit`/
`PendingHeartbeat`/`RecvAckNack`/`GetInFlight`/`InFlightSeqs` on
`ReliableSender`; `RecvData`/`DrainInOrder`/`PendingAckNack`/`Reset` on
`ReliableReceiver`) and the wire format (HEARTBEAT `0x0B`, ACKNACK `0x0A`,
RFC 1982 16-bit sequence numbers via `ReliableWire.SeqLt`/`SeqGt`) are defined
normatively there; `endpoints/csharp/Reliable.cs` (namespace
`ZeroDDS.Endpoint`) is the C# binding of this contract, byte-identical to
`crates/xrce/src/reliable.rs` and every other endpoint SDK.

`AsyncReliableWriter.Start(Socket sock, int capacity = 1024)` spawns the
drain `Thread` (name `zerodds-reliable-drain`); `Handle.Enqueue(byte[])`
returns `false` (backpressure) when the ring is full; `Shutdown()`/`Dispose()`
signals close and blocks until the drain thread has emptied the window
or a 5-second deadline expires (best-effort exit with a
vanished peer).

## §5 Conformance

A C# endpoint implementation is conformant if:

1. A raw UDP ping-pong with the generated `TypeSupport.Encode`/`Decode`
   (without XRCE frame) runs byte-correctly with the Rust reference peer.
2. The full stack (generated types + `endpoints/csharp`) exchanges a typed
   sample both via the sync `Client` and via `AsyncReader.Stream()`
   with the shared Rust XRCE peer.
3. HEARTBEAT and ACKNACK frames are byte-identical to the reference goldens
   and the reliable stream recovers datagram loss without gaps and in order
   (§4, `reliable-endpoint` v1.0 §5).
4. A latency measurement shows that `ReliableWriterHandle.Enqueue`
   (channel push) is measurably below an inline `Socket.Send` — the
   evidence that async write takes the syscall latency out of the producer path.

## §6 Examples

- Sync: `endpoints/csharp/ExampleSync.cs` — `Client.Poll()` loop, full
  field decode (5 `Reading` samples via `MemTransport`).
- Async: `endpoints/csharp/ExampleAsync.cs` — `AsyncReader.Stream()` with
  `await foreach`.
- Reliable: `endpoints/csharp/ExampleReliable.cs` — three modes: `<port>`
  (live UDP sender for the E2E), `bench` (enqueue vs. inline `Send` latency),
  without arguments (in-process demo with simulated loss).
- Quickstart: `endpoints/csharp/QUICKSTART.md`.

## §7 Errata + open questions

None. Sync, async and reliable are fully implemented and
byte-verified (see `docs/spec-coverage/zerodds-endpoint-csharp-1.0.md`).
