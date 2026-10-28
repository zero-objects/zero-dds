<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Nim endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/nim
nim c -r example_sync.nim     # subscriber polls with a bounded retry
nim c -r example_async.nim    # subscriber awaits the AsyncReader Future
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking, `Option`). The
  idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.recv()` returns a `Future[seq[byte]]`;
  `await` it on the `asyncdispatch` event loop. The idiom for `async`/`await`
  consumers.

## Transport

`Transport` is a pair of closures (`deliver` / `receive`). The examples use the
in-memory `memTransport()`; a real UDP or shared-memory link is a drop-in.

## Wire

Byte-identical to the Rust core and every other ZeroDDS endpoint (XCDR2, align
cap 4, `f32` via `cast[uint32]`). The same types can be generated from IDL with
`zerodds-idlc generate --lang nim` — see
[`docs/specs/zerodds-xcdr2-nim-1.0.md`](../../docs/specs/zerodds-xcdr2-nim-1.0.md).

## Reliable stream: unannotated types

Parameter `unannotatedTypes` of `newReceiver`, type `UnannotatedTypes`, values
`UnannotatedTypes.Appendable` (default) and `UnannotatedTypes.Final`. DDS-XRCE 1.0
Annex A types without `@extensibility` are APPENDABLE by DDS-XTypes 1.3 section
7.3.1.2.1.8 and carry a DHEADER. `Final` writes the FINAL form of the reference
implementation (eProsima).

```nim
var r = newReceiver(UnannotatedTypes.Final)
let frame = r.acknackFrame(0x80'u8, StreamNone, msgSeq, hintLastSeen)
# pure builder: the mode is the optional last parameter (default Appendable)
let f2 = acknackFrame(0x80'u8, StreamNone, 1, ack, UnannotatedTypes.Final)
```

Types covered: `ACKNACK_Payload` (`Appendable`: 17-octet frame, length field 9,
DHEADER = 5; `Final`: 13-octet frame, length field 5). This SDK writes no other
unannotated type (no `Time_t`, no `Property` entries: `CLIENT_Representation` carries
an absent `PropertySeq`). `HEARTBEAT_Payload` stays FINAL in both modes. The parser
accepts both forms in either setting.

## XRCE client: representation timestamps

DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`. The prose of 7.8.2.1 lists
`Time_t client_timestamp` after `xrce_vendor_id`; the IDL of Annex A and the bytes of
Annex B.1 do not have it. The bytes do not tell the two forms apart, so it is a setting.

- Name `RepresentationTimestamps`, values `Absent` (default, Annex A / B.1 form) and `Present` (prose form).
- Client: `newXrceClient(key, transport, RepresentationTimestamps.Present)` or `setRepresentationTimestamps`; `clientRepresentation(version, vendorId)` and `connect(version, vendorId)` build the representation with the wall clock as timestamp.
- Encoder: `encodeClientRepresentation(version, vendorId, key, session, mtu, RepresentationTimestamps.Present, seconds, nanoseconds, UnannotatedTypes.Final)`; the last parameter (default `Appendable`) is the same type as in `newReceiver`.
- The agent has to be set the same way (`representation_timestamps:` in its configuration).
- The client reads STATUS_AGENT only as a state change; it does not parse AGENT_Representation.

Layouts (client_key 01 02 03 04, session 0x01, mtu 508, 1518905996 s, 500000000 ns, little-endian):

```
absent (16):             58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

`Time_t` carries no `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8): DHEADER u32 = 8.
