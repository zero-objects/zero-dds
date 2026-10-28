<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Zig endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples as XRCE WRITE_DATA and delivers them
over a transport; a subscriber decodes **every field**.

```sh
cd endpoints/zig
zig build run-example_sync    # subscriber polls (pull)
zig build run-example_async   # subscriber uses the callback-reactor AsyncReader
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking pull). Zig has no
  async/await; this is the idiom when you own the run-loop.
- **`example_async`** — `AsyncReader` is a callback reactor: `run(n)` drains the
  transport and dispatches each decoded frame to your `on_sample` callback. The
  push idiom for event-driven consumers.

## Transport

`Transport` is a function-pointer vtable (`deliver` / `receive`, no heap). The
examples use a tiny in-memory `Fifo`; a real UDP or shared-memory link is a
drop-in.

## Wire

Byte-identical to the Rust core and every other ZeroDDS endpoint (XCDR2, align
cap 4, `f32` via `@bitCast`). The same types can be generated from IDL with
`zerodds-idlc generate --lang zig` — see
[`docs/specs/zerodds-xcdr2-zig-1.0.md`](../../docs/specs/zerodds-xcdr2-zig-1.0.md).

## Reliable stream: unannotated types

Field `unannotated_types` of `reliable.Receiver` (set with
`Receiver.initWith(alloc, mode)`), type `reliable.UnannotatedTypes`, values
`.appendable` (default) and `.final`. DDS-XRCE 1.0 Annex A types without
`@extensibility` are APPENDABLE by DDS-XTypes 1.3 section 7.3.1.2.1.8 and carry a
DHEADER. `.final` writes the FINAL form of the reference implementation (eProsima).

```zig
var rcv = reliable.Receiver.initWith(alloc, .final);
const n = rcv.acknackFrame(&buf, last_seen);              // 13 octets with .final
const m = reliable.acknackFrameWith(&buf, ack, .final);   // pure builder with the mode
const k = reliable.acknackFrame(&buf, ack);               // unchanged: appendable
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

- Name `xrce_client.RepresentationTimestamps`, values `.absent` (default, Annex A / B.1 form) and `.present` (prose form).
- Client: `XrceClient.initWith(key, &transport, .present)` or the field `representation_timestamps`; `clientRepresentation(out, version, vendor_id, mtu)` and `connectDefault(version, vendor_id, mtu)` build the representation with the wall clock as timestamp (`out` needs 28 octets).
- Encoder: `encodeClientRepresentationWith(out, version, vendor, key, session, mtu, .{ .timestamps = .present, .seconds = s, .nanoseconds = ns, .unannotated_types = .final })`; `unannotated_types` is `reliable.UnannotatedTypes` (default `.appendable`). `encodeClientRepresentation` is unchanged (`.absent`).
- The agent has to be set the same way (`representation_timestamps:` in its configuration).
- The client reads STATUS_AGENT only as a state change; it does not parse AGENT_Representation.

Layouts (client_key 01 02 03 04, session 0x01, mtu 508, 1518905996 s, 500000000 ns, little-endian):

```
absent (16):             58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

`Time_t` carries no `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8): DHEADER u32 = 8.
