<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS F# endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/fsharp
dotnet fsi example_sync.fsx     # subscriber polls
dotnet fsi example_async.fsx    # subscriber awaits the MailboxProcessor agent
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.Poll()` in a loop (non-blocking; `None` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.RecvAsync()` returns `Async<byte[]>` served
  by a `MailboxProcessor` agent; `let! body = ...` inside an `async { }` workflow.
  The idiomatic F# async model.

## Transport

`Transport` is a record of two functions (`Deliver` / `Receive`). The examples
use the in-memory `memTransport ()`; a real UDP or shared-memory link is a drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via `BitConverter`).
F# is .NET-native, so IDL types are generated with `zerodds-idlc generate --lang c#` and
consumed directly — see
[`docs/specs/zerodds-xcdr2-fsharp-1.0.md`](../../docs/specs/zerodds-xcdr2-fsharp-1.0.md).

## Reliable stream: unannotated types

`UnannotatedTypes` (module `ZeroDDSReliable`, `reliable.fs`) selects the
extensibility used for the DDS-XRCE 1.0 Annex A types that carry no
`@extensibility`. Per DDS-XTypes 1.3 section 7.3.1.2.1.8 these are APPENDABLE and
carry a DHEADER.

| Value | Form | ACKNACK |
| --- | --- | --- |
| `Appendable` (default) | DHEADER in front of the struct | body 9 octets (DHEADER = 5), length field 9, frame 17 octets |
| `Final` | no DHEADER, form of eProsima | body 5 octets, length field 5, frame 13 octets |

Set it per receiver: `ReliableReceiver(Final)` (optional constructor argument);
`receiver.PendingAckNackFrame(msgSeq)` then builds the frame in that form. The
frame builders: `Wire.ackNackFrameWith Final ack msgSeq` takes the mode;
`Wire.ackNackFrame ack msgSeq` keeps its signature and means `Appendable`.

Types covered: ACKNACK_Payload (the only unannotated Annex A type the F# SDK
writes among the reliable-stream types). HEARTBEAT_Payload stays FINAL in both
settings. `CLIENT_Representation` is not affected by this setting; see
"XRCE client: representation timestamps" below.
`Wire.tryParseAckNack` accepts both forms in either setting.

## XRCE client: representation timestamps

DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`: the prose of section
7.8.2.1 lists `Time_t client_timestamp` after `xrce_vendor_id`; the IDL of Annex A
and the bytes of Annex B.1 do not have it. The forms cannot be told apart by their
bytes, so it is a setting.

| Value | Form |
| --- | --- |
| `RepresentationTimestamps.Absent` (default) | Annex A / B.1 form, 16 octets |
| `RepresentationTimestamps.Present` | prose form of 7.8.2.1, `Time_t` after `xrce_vendor_id` |

Set it on the client: `client.RepresentationTimestamps <- RepresentationTimestamps.Present`;
`XrceFrame.createClientFor client` then builds CREATE_CLIENT with the wall clock at the time
of the call. The pure functions take explicit values:
`XrceFrame.encodeClientRepresentationWith version vendorId key sessionId mtu timestamps
extensibility seconds nanoseconds`, likewise `XrceFrame.createClientWith sessionId key
timestamps extensibility seconds nanoseconds`. `encodeClientRepresentation` and
`createClient` keep meaning Absent, bytes unchanged. `Time_t` carries no `@extensibility`, so
it is appendable (DDS-XTypes 1.3 7.3.1.2.1.8): `XrceUnannotatedTypes.Appendable` (default,
also `client.UnannotatedTypes`) writes a DHEADER = 8, `Final` none.

The agent has to be set the same way (`representation_timestamps: absent | present` in its
configuration). Layouts (client_key 01 02 03 04, session_id 1, vendor 00 00, version 01 00,
requested_mtu 508, seconds 1518905996, nanoseconds 500000000, little endian):

```
absent (16):              58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

The submessage length field of CREATE_CLIENT and the message grow accordingly. The
F# client does not parse `AGENT_Representation`; STATUS_AGENT is only a state change
(`MarkConnected()`).
