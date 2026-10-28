<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-d` v1.0 — D endpoint SDK (XRCE framing, sync/async, reliable stream)

ZeroDDS vendor spec. Implemented in `endpoints/d/`.

Complements the codegen spec [`zerodds-xcdr2-d`](zerodds-xcdr2-d-1.0.md): there the
wire mapping IDL→D, here the endpoint building block that carries a `marshalXCDR` sample as an
XRCE frame over a UDP transport — polled synchronously, asynchronously via
`std.concurrency`, optionally reliable (§4).

## §1 XRCE framing

A WRITE_DATA sample is framed as an 8-byte header + body:

```
[session][stream][seq_lo][seq_hi][0x07][0x01][len_lo][len_hi]
[req_hi][req_lo][obj_hi][obj_lo][sample...]
```

- `session` — `SessionNoKey = 0x80`.
- `stream` — `StreamBestEffort = 0x01` for the best-effort path of this building block;
  reliable streams use `stream_id >= 128` (§4, `reliable-endpoint-1.0`).
- `seq` — 16-bit little-endian, wraps modulo `0x10000`.
- Submessage ID `0x07` (WRITE_DATA), flags `0x01` (E flag = LE plus
  `DataFormat` `FORMAT_DATA` = `0b000` in bits 1..3).
- `len` — 16-bit little-endian body length.

`writeFrame(session, stream, seqNo, objectId, sample) -> ubyte[]` MUST emit the header
as above, write the `BaseObjectRequest` and append the sample unchanged.
`readFrame(frame) -> ubyte[]` MUST return the sample behind the
`BaseObjectRequest` if `frame.length >= 12 && frame[4] == 0x07`
and the declared body length is at least four, otherwise `null`.

## §2 Sync client

A blocking, polled `Client` over an exchangeable `Transport`:

```d
struct Transport {
    void delegate(ubyte[]) deliver;
    ubyte[] delegate() receive; // null if nothing is pending
}

class Client {
    this(Transport t);
    void write(ubyte[] sample);   // frames + deliver(); seqNo++ (mod 0x10000)
    ubyte[] poll();                // one non-blocking receive(); body or null
}
```

`write` MUST call `writeFrame` with the current `seqNo`, pass the result to `transport.deliver`
and then increment `seqNo` modulo `0x10000`. `poll` MUST make exactly
one `transport.receive()` call and, if a frame is present, return its
`readFrame` result; if nothing is pending, `null`.

The `Transport` is a pure delegate contract — the SDK does not prescribe a socket.
`memTransport()` returns an in-memory FIFO instance for tests/examples;
live operation binds `deliver`/`receive` to a UDP socket.

## §3 Async reader/writer

On this level an async **reader** exists as a background actor via
`std.concurrency` (`spawn`/`send`/`receive`, Tid message passing):

```d
void readerLoop(Tid owner);   // runs in the reader thread

class AsyncReader {
    this();                              // spawns readerLoop
    void feed(ubyte[] frame);            // send(tid, frame) — raw frame in
    immutable(ubyte)[] recv();           // receiveOnly — decoded body out
    void stop();                         // send(tid, true) — terminate
}
```

`readerLoop` MUST deframe every incoming `immutable(ubyte)[]` message as a frame
(`readFrame`) and, if the result is not empty, send the body to
`owner`; a `bool` message terminates the loop. Payloads are
`immutable` so that they cross the thread boundary safely — D enforces this
structurally (no `shared`/lock needed).

A decoupled async **writer** is not a separate building block of this level,
but part of the reliable stream (§4): the sync `Client` (§2) sends inline;
the reliable stream sender decouples via the `SpscRing`.

## §4 Reliable stream

The reliable stream is an optional extension of §1–§3: state machine,
wire codec and async-decoupled writer according to
[`reliable-endpoint` v1.0](reliable-endpoint-1.0.md). D implements the
canonical contract (§3 there) on its own in `endpoints/d/reliable.d` —
no import of `zerodds.d`'s `Endian`/`Writer`, to avoid name
collisions:

- `Sender` — `submit`/`pendingHeartbeat`/`recvAcknack`/`getInFlight`.
- `Receiver` — `recvData`/`drainInOrder`/`pendingAcknack`/`reset`.
- Wire codec — `Heartbeat`/`AckNack`, `heartbeatFrame`/`acknackFrame`/
  `reliableWriteFrame` + the associated `parse*` functions; byte-identical
  to `golden_heartbeat_le.bin`/`golden_acknack_le.bin`.
- `SpscRing` — a wait-free single-producer/single-consumer ring
  (`CAP = 1024`) as the async-decoupled writer: the producer only does one
  slot store + one release store on `head`, no lock, no syscall; a
  separate drain thread owns socket + sender state.

Constants, error cases, wire format and test/evidence obligations are defined
normatively in `reliable-endpoint-1.0` §3–§5 and apply unchanged to D.
