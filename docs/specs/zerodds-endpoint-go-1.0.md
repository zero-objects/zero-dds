<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-go` v1.0 — Go endpoint SDK

**Status:** normative · ZeroDDS vendor spec. Implemented in `endpoints/go/`.

Analogous to [`zerodds-xcdr2-go`](zerodds-xcdr2-go-1.0.md) (there the marshalling)
and the endpoint SDKs of other languages (`endpoints/zig`, `endpoints/nim`,
`endpoints/c`, `endpoints/ada`, ...): the native Go endpoint over XRCE framing,
sync `Client`, async `AsyncReader`/`AsyncWriter` and the reliable stream, so
that a Go app exchanges an XCDR2 sample byte-identical to `crates/xrce` +
`endpoints/c` with the shared Rust peer.

## §1 XRCE framing

An 8-byte XRCE header (`session`, `stream`, `seq` LE, submessage ID `0x07`
WRITE_DATA, `flags`, `len` LE) followed by the XCDR2 sample body — byte-identical
to `crates/xrce` and `endpoints/c`.

`endpoints/go` (package `zerodds`) MUST provide:

- `XrceWriteFrame(session, stream byte, seq uint16, sample []byte) []byte` —
  frames a sample.
- `XrceReadFrame(frame []byte) (body []byte, ok bool)` — unframes; `ok=false`
  for a frame that is too short or a wrong submessage ID.
- Constants `XrceSessionNoKey` (`0x80`, best-effort, without ClientKey) and
  `XrceStreamBestEffort` (`0x01`).

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

A blocking client — counterpart to the goroutine-based
`AsyncReader`/`AsyncWriter`: `Write` frames + delivers synchronously via the
`Transport`; `Poll` is a non-blocking single receive; `Receive`
blocks up to a timeout (polling).

`endpoints/go` MUST provide:

- `type Transport interface { Deliver(frame []byte) error; Receive(buf []byte) (n int, again bool, err error) }`
  — the only integration point; the integrator implements it for their
  link (e.g. UDP).
- `type Client struct { ... }` with `NewClient(t Transport) *Client`,
  `(*Client).Write(sample []byte) error`, `(*Client).Poll() (body []byte, ok bool, err error)`,
  `(*Client).Receive(timeout time.Duration) (body []byte, ok bool, err error)`.
- A monotonically increasing sequence number per `Write`; `Session`/`Stream` by
  default `XrceSessionNoKey`/`XrceStreamBestEffort`.

## §3 Async `Reader`/`Writer`

The idiomatic Go async model: a goroutine polls the `Transport` and
pushes unframed sample bodies onto a channel (push); the consumer ranges
over the channel. The `AsyncWriter` is the counterpart on the sending side
(same framing as `Client.Write`, but without a back channel).

`endpoints/go` MUST provide:

- `type AsyncReader struct { Samples chan []byte; ... }` with
  `NewAsyncReader(t Transport) *AsyncReader` (spawns the receive goroutine)
  and `(*AsyncReader).Close()` (stops the goroutine; `Samples` is
  drained and closed).
- `type AsyncWriter struct { ... }` with `NewAsyncWriter(t Transport) *AsyncWriter`
  and `(*AsyncWriter).Write(sample []byte) error`.
- Both share the XRCE framing from §1 and the same `Transport` contract
  as the sync `Client`.

## §4 Reliable stream

`endpoints/go` implements the reliable stream as an endpoint capability according to
[`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — sender/
receiver state machine, HEARTBEAT/ACKNACK wire codec and the
async-decoupled `ReliableAsyncWriter`, whose drain goroutine owns the
`Transport` and the reliable sender state, while the producer only
enqueues wait-free onto a buffered channel (never enters the kernel).

The constants (`SenderWindow=16`, `ReceiverBufferCap=64`,
`HeartbeatPeriod=500ms`, `ReliableMaxPayload=65535`, reliable stream ID
`0x80`), the state machine contract (`Submit`/`PendingHeartbeat`/
`RecvAcknack`/`GetInFlight` on the sender; `RecvData`/`DrainInOrder`/
`PendingAcknack`/`Reset` on the receiver) and the wire format
(HEARTBEAT `0x0B`, ACKNACK `0x0A`, RFC 1982 16-bit sequence numbers) are defined
normatively there; `endpoints/go/reliable.go` is the Go binding of this
contract, byte-identical to `crates/xrce/src/reliable.rs` and every other
endpoint SDK.

## §5 Conformance

A Go endpoint implementation is conformant if:

1. A raw UDP ping-pong with the generated `MarshalXCDR`/`UnmarshalXCDR*`
   (without XRCE frame) runs byte-correctly with the Rust reference peer.
2. The full stack (generated types + `endpoints/go`) exchanges a typed sample
   both via the sync `Client` and via `AsyncReader`/`AsyncWriter`
   with the shared Rust XRCE peer.
3. HEARTBEAT and ACKNACK frames are byte-identical to the reference goldens
   and the reliable stream recovers datagram loss without gaps and in order
   (§4, `reliable-endpoint` v1.0 §5).
4. A latency measurement shows that `ReliableAsyncWriter.Enqueue` (ring buffer
   push) is measurably below an inline `sendto` — the evidence that
   async write takes the syscall latency out of the producer path.

## §6 Examples

- Sync: `endpoints/go/example_sync` — poll loop with deadline, full
  field decode.
- Async: `endpoints/go/example_async` — goroutine/channel `AsyncReader`.
- Reliable: `endpoints/go/example_reliable` — in-process demo (no socket)
  of the loss recovery.
- Quickstart: `endpoints/go/QUICKSTART.md`.

## §7 Errata + open questions

None. Sync, async and reliable are fully implemented and
byte-verified (see `docs/spec-coverage/zerodds-endpoint-go-1.0.md`).
