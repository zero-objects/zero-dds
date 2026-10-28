<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Go endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples as XRCE WRITE_DATA and delivers
them over a transport; a subscriber decodes **every field**.

```sh
cd endpoints/go
go run ./example_sync     # subscriber polls with a 2s deadline
go run ./example_async    # subscriber ranges the AsyncReader.Samples channel
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.Poll()` in a loop with a deadline (non-blocking
  receive; sleep-retry when empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader` runs a goroutine that pushes decoded frames
  onto a `Samples` channel; you `range` it and `Close()` when done. The idiom for
  event-driven consumers.

## Transport

The examples use a small in-memory `loopback` transport. It implements the
one-method-each `Transport` interface (`Deliver` / `Receive`), so swapping in a
real UDP or shared-memory link is a drop-in — see `async_test.go`'s
`udpTransport` for a live-UDP `net.UDPConn` implementation.

## Reliable stream: unannotated types

Setting: `UnannotatedTypes` with the values `UnannotatedAppendable` (default)
and `UnannotatedFinal`. DDS-XRCE 1.0 Annex A types without `@extensibility`
are APPENDABLE (DDS-XTypes 1.3 section 7.3.1.2.1.8) and carry a DHEADER;
`UnannotatedFinal` writes the FINAL form of the reference implementation
(eProsima). The SDK writes one such type, `ACKNACK_Payload`: 17-octet frame
(body length 9, DHEADER = 5) with `UnannotatedAppendable`, 13-octet frame
(body length 5) with `UnannotatedFinal`. `HEARTBEAT_Payload` is FINAL in both
settings. Readers (`ParseAckNack`) accept both forms in either setting.

- Receiver: `NewReliableReceiverWith(zerodds.UnannotatedFinal)`;
  `receiver.AckNackFrame(...)` writes the form of the receiver.
- Frame builder: `AckNackFrameWith(session, streamHdr, msgSeq, body, mode)`;
  `AckNackFrame(...)` without `mode` is `UnannotatedAppendable`.

## XRCE client: representation timestamps

Setting: `RepresentationTimestamps` with the values
`RepresentationTimestampsAbsent` (default) and `RepresentationTimestampsPresent`.
DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`: the prose of
section 7.8.2.1 lists `Time_t client_timestamp` after `xrce_vendor_id`; the IDL
of Annex A and the bytes of Annex B.1 do not have it. Absent writes the
Annex A form (what the SDK wrote before). Present writes the prose form:
seconds `i32` and nanoseconds `u32`, little endian. `Time_t` has no
`@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 section 7.3.1.2.1.8) with
`UnannotatedAppendable` (default): DHEADER = 8, 12 octets. With
`UnannotatedFinal` there is no DHEADER, 8 octets.

The agent has to be set the same way (`representation_timestamps: absent |
present` in its configuration).

- Client: `client.SetRepresentationTimestamps(zerodds.RepresentationTimestampsPresent, zerodds.UnannotatedAppendable)`;
  `client.ClientRepresentation(version, vendorID, sessionID, mtu)` uses the
  wall clock at the time of the call.
- Encoder: `EncodeClientRepresentationWith(version, vendorID, key, sessionID, mtu, ts, seconds, nanoseconds, unann)`;
  `EncodeClientRepresentation(...)` is absent.

Bytes (client_key 01 02 03 04, session_id 01, version 01 00, vendor 00 00,
requested_mtu 508, seconds 1518905996, nanoseconds 500000000):

    absent (16):             58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
    present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
    present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01

The SDK does not read `AGENT_Representation`; `STATUS_AGENT` is used as a state
change only.

## Wire

The wire is byte-identical to the Rust core and every other ZeroDDS endpoint
(XCDR2, alignment cap 4, `f32` via `math.Float32bits`). The same types can be
generated from IDL with `zerodds-idlc generate --lang go` — see
[`docs/specs/zerodds-xcdr2-go-1.0.md`](../../docs/specs/zerodds-xcdr2-go-1.0.md).
