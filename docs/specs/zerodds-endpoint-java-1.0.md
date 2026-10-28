<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-java` v1.0 — Java endpoint SDK

**Status:** normative · ZeroDDS vendor spec. Implemented in `endpoints/java/`.

Builds on [ADR 0013](../adr/0013-native-endpoint-sdks.md) (native endpoint SDKs,
frame hook contract) and DDS-XRCE 1.0 §8.3 (framing) as well as §8.4.10/§8.4.11
(reliable streams). The reliable delivery contract itself is standardized separately
in [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md). Analogous to
the endpoint SDKs of other languages (`endpoints/c`, `endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/d`, `endpoints/ada`, ...): a
pure Java 8 endpoint (no JNI) over XRCE framing, sync send/recv, async
reader/writer and the reliable stream, so that a Java app exchanges an
XCDR2 sample byte-identically to `crates/xrce` and `endpoints/c` with the
shared Rust peer.

The framing and wire core classes (`Zdw`, `ZdwEndpoint`, `ZdwReflect`)
are in the default package (no module overhead for a standalone sample);
the reliable stream — state machine, wire codec, async writer — is in the
package `org.zerodds.endpoint`.

## §1 XRCE framing

`endpoints/java/ZdwEndpoint.java` (default package) MUST produce and parse
DDS-XRCE-1.0-conformant frames both for packet-based transports (UDP) and for
serial byte-stream transports (RS-485, UART) —
byte-identical to `crates/xrce` and `endpoints/c`.

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

### §1.1 WRITE_DATA frame

An 8-byte header (`session`, `stream`, `seq` LE, submessage ID `0x07`
WRITE_DATA, `flags`, `len` LE — DDS-XRCE 1.0 §8.3.2.3/§8.3.4) followed by the
XCDR2 sample body. Best-effort operation without a ClientKey: `session ≥ 0x80`.

`endpoints/java` MUST provide:

- `ZdwEndpoint.xrceWriteFrame(int session, int stream, int seq, byte[] sample) -> byte[]`
  — frames a sample.
- Constants `ZdwEndpoint.SESSION_NOKEY` (`0x80`) and
  `ZdwEndpoint.STREAM_BEST_EFFORT` (`0x01`).

### §1.2 Receive path (DATA message)

The endpoint MUST be able to unframe DATA (id `0x09`) as well as WRITE_DATA submessages (id `0x07`)
via the same unwrap path:
`ZdwEndpoint.xrceReadBody(byte[] frame) -> byte[]` reads the submessage ID
from byte 4, checks it against `0x07`/`0x09` and returns the body from offset 8
according to the length in the `len` field.

### §1.3 Serial framing (Annex C)

For serial byte-stream transports the endpoint MUST implement the framing of
DDS-XRCE 1.0 Annex C.1: `7E | sourceAddress | remoteAddress |
payloadLength (2, LE) | payload | frameCheck (2, LE)`, byte stuffing of
`0x7E`/`0x7D` after the flag (C.1.2), `frameCheck` = FCS-16 according to RFC 1662
over the addresses, `payloadLength` and payload (C.1.1.6) —
`ZdwEndpoint.serialFrame(source, remote, payload)`,
`ZdwEndpoint.serialDeframe(frame)` (returns `SerialFrame` with `source`,
`remote`, `payload`; throws `IllegalArgumentException` on a wrong
`frameCheck`, a wrong escape, `0x7E` before the end of the frame and a truncated
frame) and `ZdwEndpoint.fcs16`, byte-identical to
`endpoints/c/src/zerodds_endpoint.c`.

## §2 sync send/recv

### §2.1 Frame hook contract

The endpoint is transport-opaque (ADR 0013, invariant 5): `ZdwEndpoint`
has no `Client` or `Transport` object of its own — framing/unframing
are stateless static methods. The integrator owns the socket or
in-memory queue itself and calls `xrceWriteFrame`/`xrceReadBody` directly
(frame hook symmetry: encode+send and receive+decode go through the same
static call path, no hidden object holds transport state).

### §2.2 Poll loop idiom

The baseline usage is synchronous: the integrator owns the run loop and
calls the receive function itself (poll, or blocking `receive()`
on a real `DatagramSocket`). Reference: `endpoints/java/ExampleSync.java`
(in-memory `ArrayDeque` poll loop) and the `sync` mode in
`crates/endpoint-e2e/tests/java.rs` (blocking
`DatagramSocket.receive()`).

## §3 async reader/writer

The idiomatic Java async model (`java.util.concurrent`): a
reader `Thread` drains the transport and pushes unframed sample bodies
onto a `BlockingQueue`; the consumer blocks on `take()`. Unlike
in `endpoints/go`, there is no dedicated `AsyncReader`/`AsyncWriter` class
for the plain (non-reliable) path — the integrator composes
reader thread + `BlockingQueue` itself from the stateless
`ZdwEndpoint` framing methods (§1), analogous to the `endpoints/c` reactor pattern,
but without its own callback type. For the reliable stream (§4), on the other hand,
a dedicated, purpose-built class exists in `AsyncReliableWriter`.

`endpoints/java` MUST provide:

- A runnable example of the reader thread/`BlockingQueue` pattern:
  `endpoints/java/ExampleAsync.java` (`ConcurrentLinkedQueue` transport,
  `LinkedBlockingQueue<byte[]>` as the inbox, the consumer blocks on `take()`).
- Live transport proof over a real `DatagramSocket` (not only an
  in-memory queue): the `async` mode in
  `crates/endpoint-e2e/tests/java.rs` — a reader `Thread` reads from the socket
  and puts the unframed body into a `LinkedBlockingQueue`, the consumer
  blocks on `take()`.

## §4 Reliable stream

`endpoints/java` implements the reliable stream as an endpoint capability
according to [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — sender/
receiver state machine, HEARTBEAT/ACKNACK wire codec and the
async-decoupled `AsyncReliableWriter`, whose drain `Thread` carries the
`ReliableSender` state and all the I/O, while the producer only
enqueues wait-free into a buffered `BlockingQueue` (never goes into the
kernel). Package `org.zerodds.endpoint`.

The constants (`ReliableWire.WINDOW=16`, `ReliableWire.RECV_BUF=64`,
`ReliableWire.HEARTBEAT_PERIOD_MS=500`, `ReliableWire.MAX_PAYLOAD=65535`,
reliable stream ID `ReliableWire.STREAM_RELIABLE=0x80`), the
state machine contract (`ReliableSender.submit`/`pendingHeartbeat`/
`recvAcknack`/`getInFlight` on the sender; `ReliableReceiver.recvData`/
`drainInOrder`/`pendingAcknack`/`reset` on the receiver) and the
wire format (`ReliableWire.heartbeatFrame`/`parseHeartbeat` — submessage ID
`0x0B`; `acknackFrame`/`parseAckNack` — submessage ID `0x0A`; RFC-1982
16-bit sequence numbers via `seqLt`/`seqGt`) are defined normatively in `reliable-endpoint-1.0`
§3/§4; `endpoints/java/org/zerodds/endpoint/*.java` is
the Java binding of this contract, byte-identical to
`crates/xrce/src/reliable.rs` and every other endpoint SDK.

`AsyncReliableWriter` (`org.zerodds.endpoint.AsyncReliableWriter`) MUST
provide:

- `submit(byte[] payload)` — blocking enqueue into an
  `ArrayBlockingQueue<byte[]>` (capacity 4096), no socket syscall on the
  producer path.
- `offer(byte[] payload) -> boolean` — non-blocking enqueue (for the
  latency bench, §6).
- A dedicated drain `Thread` that owns the `ReliableSender` state:
  takes queued samples into the send window (`submit` → `writeFrame` →
  `DatagramSocket.send`), serves the HEARTBEAT timer
  (`pendingHeartbeat`/`heartbeatFrame`), processes incoming ACKNACKs
  (`parseAckNack` → `recvAcknack` → retransmit of the sequence numbers still
  marked as missing from the in-flight buffer).
- `finish(long timeoutMs) -> boolean` — signals the end of production and
  blocks until queue and send window are empty (all samples acked) or
  the timeout expires.

## §5 Conformance

A Java endpoint implementation is conformant if:

1. The full stack (generated `idl-java` TypeSupport classes +
   `endpoints/java`) exchanges a typed sample both via the sync poll loop
   and via the reader thread/`BlockingQueue` async path with the
   shared Rust XRCE peer over a real `DatagramSocket`.
2. HEARTBEAT and ACKNACK frames are byte-identical to the reference goldens
   and the reliable stream recovers datagram loss without gaps, in order
   (§4, `reliable-endpoint` v1.0 §5).
3. A latency measurement shows that `AsyncReliableWriter.offer`
   (`BlockingQueue` enqueue) is measurably below an inline
   `DatagramSocket.send` — the evidence that async write takes the
   syscall latency out of the producer path.

## §6 Examples

- Sync: `endpoints/java/ExampleSync.java` — in-memory poll loop, full
  field decode (`Reading { id, value, label }`).
- Async: `endpoints/java/ExampleAsync.java` — reader `Thread` +
  `BlockingQueue`, full field decode.
- Reliable: `endpoints/java/ExampleReliable.java` — `AsyncReliableWriter`
  against a real UDP peer (no in-process stub).
- Latency bench: `endpoints/java/ReliableBench.java` — `BlockingQueue`
  enqueue vs. inline `DatagramSocket.send`.
- Unit/byte golden suite: `endpoints/java/ReliableSelfTest.java`.
- Quickstart: `endpoints/java/QUICKSTART.md`.

## §7 Errata + open questions

None. Sync, async and reliable are fully implemented and
byte-verified (see `docs/spec-coverage/zerodds-endpoint-java-1.0.md`).
