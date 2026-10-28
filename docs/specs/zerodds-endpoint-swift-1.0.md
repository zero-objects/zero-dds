<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-swift` v1.0 — Swift endpoint SDK

**Status:** normative · ZeroDDS vendor spec. Implemented in `endpoints/swift/`.

Analogous to [`zerodds-xcdr2-swift`](zerodds-xcdr2-swift-1.0.md) (there the
marshalling) and the endpoint SDKs of other languages (`endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/c`, ...): the native
Swift endpoint over XRCE framing, sync `Client`, async `AsyncReader` (an
`AsyncStream`, the idiomatic Swift concurrency model) and the reliable
stream, so that a Swift app exchanges an XCDR2 sample byte-identical to
`crates/xrce` + `endpoints/c` with the shared Rust peer.

No C shim: `endpoints/swift/Sources/Zerodds/Zerodds.swift` is a
standalone, pure Swift wire core (only `Foundation`, for `NSLock`), which
coexists with the generated IDL module in an app by being compiled as its own
`Zerodds` module (see §6).

## §1 XRCE framing

An 8-byte XRCE header (`session`, `stream`, `seq` LE, submessage ID `0x07`
WRITE_DATA, `flags`, `len` LE) followed by the XCDR2 sample body — byte-identical
to `crates/xrce` and `endpoints/c`.

`endpoints/swift` MUST provide:

- `func xrceWriteFrame(session: UInt8, stream: UInt8, seq: UInt16, sample: [UInt8]) -> [UInt8]`
  — frames a sample.
- `func xrceReadFrame(_ frame: [UInt8]) -> [UInt8]?` — deframes; `nil` on a frame that is too
  short or a wrong submessage ID.
- Constants `xrceSessionNoKey` (`0x80`, best-effort, without ClientKey) and
  `xrceStreamBestEffort` (`0x01`).

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

A blocking client — counterpart to the `AsyncStream`-based
`AsyncReader`: `write` frames + delivers synchronously via the `Transport`;
`poll` is a non-blocking single receive.

`endpoints/swift` MUST provide:

- `protocol Transport: Sendable { func deliver(_ frame: [UInt8]); func receive() -> [UInt8]? }`
  — the only integration point; the integrator implements it for
  its link (e.g. UDP, see `UDPTransport` in `crates/endpoint-e2e/tests/swift.rs`).
- `final class Client` with `init(_ transport: Transport)`,
  `func write(_ sample: [UInt8])`, `func poll() -> [UInt8]?`.
- A monotonically increasing sequence number per `write`; `session`/`stream`
  default to `xrceSessionNoKey`/`xrceStreamBestEffort`.

## §3 Async `AsyncReader`

The idiomatic Swift concurrency model: `AsyncReader.stream()` returns
an `AsyncStream<[UInt8]>`, which the consumer iterates with `for await`; an
internal `Task` polls the `Transport` (backoff via `Task.sleep` on an empty
poll) and yields deframed sample bodies; `continuation.onTermination`
cancels the task.

`endpoints/swift` MUST provide:

- `final class AsyncReader: Sendable` with `init(_ transport: Transport)` and
  `func stream() -> AsyncStream<[UInt8]>`.
- On the send side, the same `Client.write` (§2) does the framing for the
  async-reading peer — no separate `AsyncWriter` type beyond the
  reliable `AsyncWriter` (§4); both share the XRCE framing from §1 and
  the same `Transport` contract.
- `MemTransport` — an `NSLock`-protected in-memory FIFO `Transport`
  implementation for tests + examples (`final class MemTransport: Transport, @unchecked Sendable`).

## §4 Reliable stream

`endpoints/swift` implements the reliable stream as an endpoint capability
according to [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — sender/
receiver state machine, HEARTBEAT/ACKNACK wire codec as well as the
async-decoupled `AsyncWriter`, whose dedicated drain `Thread` owns the
`Transport` and the reliable sender state, while the producer only
enqueues wait-free (`NSLock`-protected ring buffer `write`, back-pressure via
a `false` return) and never enters the kernel.

The constants (`reliableSenderWindow=16`, `reliableReceiverBuffer=64`,
`reliableHeartbeatPeriodMs=500`, `reliableMaxPayload=65535`, reliable
stream ID `reliableStreamId=0x80`), the state machine contract (`submit`/
`pendingHeartbeat`/`recvAckNack`/`getInFlight`/`inFlightSeqs` on
`ReliableSender`; `recvData`/`drainInOrder`/`pendingAckNack`/`reset` on
`ReliableReceiver`) and the wire format (HEARTBEAT `0x0B`, ACKNACK `0x0A`,
RFC-1982 16-bit sequence numbers, `seqLt`/`seqGt`) are defined normatively
there; `endpoints/swift/Reliable.swift` is the Swift binding of this
contract, byte-identical to `crates/xrce/src/reliable.rs` and every other
endpoint SDK.

`Reliable.swift` is self-contained (only `Foundation`, for
`Date`/`Thread`/`NSLock`/`DispatchSemaphore`) and is compiled as its own
`ZeroddsReliable` module, so that it never collides with the wire core types
(`Endianness`/`Writer`/`Reader`) of a generated IDL module —
the same procedure as for `Zerodds` in §6.

## §5 Conformance

A Swift endpoint implementation is conformant if:

1. The full stack (generated types + `endpoints/swift`) exchanges a typed
   sample both via the sync `Client` and via `AsyncReader` with the
   shared Rust XRCE peer.
2. HEARTBEAT and ACKNACK frames are byte-identical to the reference goldens
   and the reliable stream recovers datagram loss in order without gaps
   (§4, `reliable-endpoint` v1.0 §5).
3. A latency measurement shows that `AsyncWriter.write` (ring buffer push)
   is measurably below an inline `sendto` — the evidence that async write
   takes the syscall latency out of the producer path.
4. The wire core itself (`Writer`/`Reader`, §1 of the `zerodds-xcdr2-swift`
   spec) is byte-identical to the Rust reference, both big- and
   little-endian.

## §6 Module layout + examples

A generated IDL module and the SDKs (`Zerodds`, `ZeroddsReliable`)
each define their own wire core types (`Endianness`/`Writer`/`Reader`),
which would collide in a shared module. That is why each SDK is
compiled separately as an object + `.swiftmodule` (`-emit-module` +
`-emit-object`) and the app links against it (`swiftc -I <dir> app.swift
sdk.o`); the generated code stays in the app module. This procedure is used
by both the ping-pong and the reliable E2E harness
(`crates/endpoint-e2e/tests/swift.rs`, `crates/endpoint-e2e/tests/swift_reliable.rs`).

- Sync: `endpoints/swift/Sources/ZeroddsExampleSync` — `Client.poll()` in
  a loop.
- Async: `endpoints/swift/Sources/ZeroddsExampleAsync` — `for await` over
  `AsyncReader.stream()`.
- Reliable: `endpoints/swift/example_reliable.swift` — in-process demo (no
  socket) of the loss recovery.
- Quickstart: `endpoints/swift/QUICKSTART.md`.

## §7 Errata + open questions

The Swift toolchain (`swiftc`) is not available on the Linux test host (Linux bench host);
all Swift endpoint tests run exclusively locally on
macOS (`swiftc_available()` gate in the E2E harnesses — loud skip, no
false green). Otherwise, sync, async and reliable are fully
implemented and byte-verified (see
`docs/spec-coverage/zerodds-endpoint-swift-1.0.md`).
