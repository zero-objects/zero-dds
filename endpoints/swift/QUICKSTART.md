<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Swift endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/swift
swift run ZeroddsExampleSync     # subscriber owns the run-loop and polls
swift run ZeroddsExampleAsync    # subscriber iterates an AsyncStream
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`ZeroddsExampleSync`** — `Client.poll()` in a loop (non-blocking; `nil` when
  empty). The idiom when you own the run-loop.
- **`ZeroddsExampleAsync`** — `AsyncReader.stream()` is an `AsyncStream`; the
  consumer iterates it with `for await`. The idiomatic Swift structured-concurrency
  model.

## Transport

`Transport` is a protocol (`deliver(_:)`, `receive() -> [UInt8]?`). The examples use
the in-memory `MemTransport` (an `NSLock`-guarded FIFO); a real UDP or shared-memory
link is a drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via `.bitPattern` /
`Float(bitPattern:)`). The same types can be generated from IDL with
`zerodds-idlc generate --lang swift` — see
[`docs/specs/zerodds-xcdr2-swift-1.0.md`](../../docs/specs/zerodds-xcdr2-swift-1.0.md).

## Reliable stream: unannotated types

Parameter `unannotatedTypes` of `ReliableReceiver.init(unannotatedTypes:)` and of
`acknackFrame(_:unannotatedTypes:)`, type `UnannotatedTypes`, values `.appendable`
(default) and `.final`. DDS-XRCE 1.0 Annex A types without `@extensibility` are
APPENDABLE by DDS-XTypes 1.3 section 7.3.1.2.1.8 and carry a DHEADER. `.final`
writes the FINAL form of the reference implementation (eProsima).

```swift
let receiver = ReliableReceiver(unannotatedTypes: .final)
let frame = receiver.acknackFrame(hintLastSeen: last)
// pure builder
let f2 = acknackFrame(ack, unannotatedTypes: .final)
```

Types covered: `ACKNACK_Payload` (`.appendable`: 17-octet frame, length field 9,
DHEADER = 5; `.final`: 13-octet frame, length field 5). This SDK writes no other
unannotated type (no `Time_t`, no `Property` entries: `CLIENT_Representation` carries
an absent `PropertySeq`). `HEARTBEAT_Payload` stays FINAL in both modes. The parser
accepts both forms in either setting.

## XRCE client: representation timestamps

DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`. The prose of 7.8.2.1 lists
`Time_t client_timestamp` after `xrce_vendor_id`; the IDL of Annex A and the bytes of
Annex B.1 do not have it. The bytes do not tell the two forms apart, so it is a setting.

- Name `RepresentationTimestamps`, values `.absent` (default, Annex A / B.1 form) and `.present` (prose form).
- Client: `XrceClient(clientKey:transport:representationTimestamps: .present)` or the property `representationTimestamps`; `clientRepresentation(version:vendorId:)` and `connect(version:vendorId:)` build the representation with the wall clock as timestamp.
- Encoder: `encodeClientRepresentation(version:vendorId:clientKey:sessionId:requestedMtu:representationTimestamps: .present, timestamp: (seconds, nanoseconds), unannotatedTypes: .final)`; `XrceUnannotatedTypes` has the values `.appendable` (default) and `.final`, like `UnannotatedTypes` of the reliable stream (that file is outside the package). Without `timestamp:` the wall clock is used.
- The agent has to be set the same way (`representation_timestamps:` in its configuration).
- The client reads STATUS_AGENT only as a state change; it does not parse AGENT_Representation.

Layouts (client_key 01 02 03 04, session 0x01, mtu 508, 1518905996 s, 500000000 ns, little-endian):

```
absent (16):             58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

`Time_t` carries no `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8): DHEADER u32 = 8.
