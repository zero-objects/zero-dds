<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Lua endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/lua
lua5.4 example_sync.lua     # subscriber owns the run-loop and polls
lua5.4 example_async.lua    # subscriber resumes a coroutine producer
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client:poll` in a loop (non-blocking; `nil` when empty).
  The idiom when you own the run-loop.
- **`example_async`** — `asyncReader` is a `coroutine.wrap` producer; each resume
  yields the next decoded sample (or `nil` when momentarily empty). The idiomatic
  Lua cooperative-concurrency model.

## Transport

`transport` is `{ deliver = function(frame), receive = function() }`. The examples
use the in-memory `memTransport` (a FIFO); a real UDP or shared-memory link is a
drop-in.

## Reliable stream: unannotated types

Option `unannotatedTypes` with the values `"appendable"` (default) and `"final"`
(`rel.UNANNOTATED_APPENDABLE` / `rel.UNANNOTATED_FINAL`). DDS-XRCE 1.0 Annex A
types without `@extensibility` are APPENDABLE (DDS-XTypes 1.3 section
7.3.1.2.1.8) and carry a DHEADER; `"final"` writes the FINAL form of the
reference implementation (eProsima). The SDK writes one such type,
`ACKNACK_Payload`: 17-octet frame (length field 9) with `"appendable"`, 13-octet
frame (length field 5) with `"final"`. `HEARTBEAT_Payload` is FINAL in both
settings. `parseAcknack` accepts both forms in either setting.

```lua
local rx = rel.Receiver.new({ unannotatedTypes = "final" })  -- per receiver
rx:acknackFrame()                                            -- 13 octets
rel.acknackFrame(0x80, 0x00, 1, 1, 0, 0, 0x80, "final")      -- frame builder, last argument
```

## XRCE client: representation timestamps

Option `representationTimestamps` with the values `"absent"` (default) and
`"present"`. DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`: the
prose of section 7.8.2.1 lists `Time_t client_timestamp` after
`xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not have it.
`"absent"` writes the Annex A form (what the SDK wrote before). `"present"`
writes the prose form: seconds `i32` and nanoseconds `u32`, little endian.
`Time_t` has no `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 section
7.3.1.2.1.8) with `unannotatedTypes = "appendable"` (default): DHEADER = 8, 12
octets. With `unannotatedTypes = "final"` there is no DHEADER, 8 octets.

The agent has to be set the same way (`representation_timestamps: absent |
present` in its configuration).

```lua
local client = xc.XrceClient.new(key, transport, { representationTimestamps = "present" })
client:connect()   -- CREATE_CLIENT with client:clientRepresentation(), Time_t = os.time()
xc.encodeClientRepresentation(ver, vendor, key, 1, 508,
  { representationTimestamps = "present", seconds = 1518905996, nanoseconds = 500000000 })
```

Bytes (client_key 01 02 03 04, session_id 01, version 01 00, vendor 00 00,
requested_mtu 508, seconds 1518905996, nanoseconds 500000000):

    absent (16):              58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
    present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
    present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01

The SDK does not read `AGENT_Representation`; `STATUS_AGENT` is a state change
only. The Lua client takes the wall clock with `os.time()` (whole seconds,
nanoseconds 0).

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4) — built on `string.pack` /
`string.unpack` (Lua 5.3+), `f32` via the `"f"` format. The same types can be
generated from IDL with `zerodds-idlc generate --lang lua` — see
[`docs/specs/zerodds-xcdr2-lua-1.0.md`](../../docs/specs/zerodds-xcdr2-lua-1.0.md).
