<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS C# endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading(Id, Value, Label)` samples and delivers them; a subscriber decodes
**every field**.

```sh
cd endpoints/csharp
dotnet run --project ExampleSync.csproj     # subscriber owns the run-loop and polls
dotnet run --project ExampleAsync.csproj    # subscriber iterates an IAsyncEnumerable
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`ExampleSync`** — `Client.Poll()` in a loop (non-blocking; `null` when empty).
  The idiom when you own the run-loop.
- **`ExampleAsync`** — `AsyncReader.Stream()` is an `IAsyncEnumerable<byte[]>`; the
  consumer iterates it with `await foreach`. The idiomatic C# async model.

## Byte-identity

```sh
cargo run -p zerodds-endpoint-golden -- build          # (from the workspace root)
dotnet run --project ByteIdentity.csproj -- build/golden_le.bin build/golden_be.bin
```

`ByteIdentity` proves the `@final` encoding is byte-for-byte identical to the Rust
golden (LE + BE).

## Transport

`ITransport` is `Deliver(frame)` + `Receive() -> frame?`. The examples use the
in-memory `MemTransport` (a locked FIFO); a real UDP or serial link is a drop-in.

## Wire & codegen

The pure-C# wire-core (`Zerodds.cs`) is byte-identical to the Rust core (XCDR2,
align cap 4, `f32` via `BitConverter`). IDL types are generated with
`zerodds-idlc generate --lang c#` (a full IDL4 codegen: struct/enum/union → discriminated
record/typedef/array/map/@mutable) — see
[`docs/specs/zerodds-xcdr2-csharp-1.0.md`](../../docs/specs/zerodds-xcdr2-csharp-1.0.md).

## Reliable stream: unannotated types

`UnannotatedTypes` (namespace `ZeroDDS.Endpoint`, `Reliable.cs`) selects the
extensibility used for the DDS-XRCE 1.0 Annex A types that carry no
`@extensibility`. Per DDS-XTypes 1.3 section 7.3.1.2.1.8 these are APPENDABLE and
carry a DHEADER.

| Value | Form | ACKNACK |
| --- | --- | --- |
| `UnannotatedTypes.Appendable` (default) | DHEADER in front of the struct | body 9 octets (DHEADER = 5), length field 9, frame 17 octets |
| `UnannotatedTypes.Final` | no DHEADER, form of eProsima | body 5 octets, length field 5, frame 13 octets |

Set it per receiver: `new ReliableReceiver(UnannotatedTypes.Final)`;
`receiver.PendingAckNackFrame(hint)` then builds the frame in that form. The frame
builder takes it as an optional argument:
`ReliableWire.AckNackFrame(ack, msgSeq, UnannotatedTypes.Final)`; without the
argument it means Appendable.

Types covered: ACKNACK_Payload (the only unannotated Annex A type the C# SDK
writes among the reliable-stream types). HEARTBEAT_Payload stays FINAL in both
settings. `CLIENT_Representation` is not affected by this setting; see
"XRCE client: representation timestamps" below. `ReliableWire.TryParseAckNack`
accepts both forms in either setting.

## XRCE client: representation timestamps

DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`: the prose of section
7.8.2.1 lists `Time_t client_timestamp` after `xrce_vendor_id`; the IDL of Annex A
and the bytes of Annex B.1 do not have it. The forms cannot be told apart by their
bytes, so it is a setting.

| Value | Form |
| --- | --- |
| `RepresentationTimestamps.Absent` (default) | Annex A / B.1 form, 16 octets |
| `RepresentationTimestamps.Present` | prose form of 7.8.2.1, `Time_t` after `xrce_vendor_id` |

Set it on the client: `new XrceClient(key, RepresentationTimestamps.Present)` or
`client.RepresentationTimestamps = ...`; `client.FrameCreateClient()` then uses the wall
clock at the time of the call. The pure encoder takes explicit values as optional
parameters: `XrceFrame.EncodeClientRepresentation(version, vendorId, key, sessionId, mtu,
timestamps, extensibility, seconds, nanoseconds)`, likewise `XrceFrame.CreateClient(sessionId,
key, timestamps, extensibility, seconds, nanoseconds)`. Without them the meaning is Absent,
bytes unchanged. `Time_t` carries no `@extensibility`, so it is appendable (DDS-XTypes 1.3
7.3.1.2.1.8): `XrceUnannotatedTypes.Appendable` (default, also `client.UnannotatedTypes`)
writes a DHEADER = 8, `Final` none.

The agent has to be set the same way (`representation_timestamps: absent | present` in its
configuration). Layouts (client_key 01 02 03 04, session_id 1, vendor 00 00, version 01 00,
requested_mtu 508, seconds 1518905996, nanoseconds 500000000, little endian):

```
absent (16):              58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

The submessage length field of CREATE_CLIENT and the message grow accordingly. The
C# client does not parse `AGENT_Representation`; STATUS_AGENT is only a state change
(`MarkConnected()`).
