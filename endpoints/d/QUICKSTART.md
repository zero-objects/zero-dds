<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS D endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/d
make example_sync     # subscriber polls
make example_async    # subscriber consumes via the std.concurrency actor
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking; `null` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader` spawns a `std.concurrency` thread; you
  `feed` frames and `recv()` decoded bodies (message passing, `immutable(ubyte)[]`
  payloads). The idiom for actor-style consumers.

## Transport

`Transport` is a pair of delegates (`deliver` / `receive`). The examples use the
in-memory `memTransport()`; a real UDP or shared-memory link is a drop-in.

## Wire

Byte-identical to the Rust core and every other ZeroDDS endpoint (XCDR2, align
cap 4, `f32` via `*cast(uint*)&v`). The same types can be generated from IDL with
`zerodds-idlc generate --lang d` — see
[`docs/specs/zerodds-xcdr2-d-1.0.md`](../../docs/specs/zerodds-xcdr2-d-1.0.md).

## Unannotated types (DDS-XRCE 1.0 Annex A)

`ACKNACK_Payload` carries no `@extensibility` in Annex A and is APPENDABLE by
DDS-XTypes 1.3 section 7.3.1.2.1.8: the ACKNACK body is 9 octets (DHEADER `5`,
then the 5 octets of the members), the frame 17 octets. This is the only
unannotated type `reliable.d` writes (`xrce_client.d` sends no `Property`).
The setting is `UnannotatedTypes.appendable` (default) or
`UnannotatedTypes.final_` (`final` is a D keyword; body 5 octets, 13-octet
frame, the form of the reference implementation). It is set per `Receiver`
or per call:

```d
Receiver rx;
rx.unannotatedTypes = UnannotatedTypes.final_;
auto ack = rx.pendingAcknackFrame(false, 0);   // 13 octets
auto b = acknackFrame(AckNack(1, [0, 0], 0x80), UnannotatedTypes.final_);
```

`parseAckNack` accepts both forms in either setting. `HEARTBEAT_Payload` stays
FINAL in both settings.

## Representation timestamps (DDS-XRCE 1.0 7.8.2.1)

The prose of 7.8.2.1 lists `Time_t client_timestamp` in `CLIENT_Representation`
after `xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not
have it. The two forms cannot be told apart by their bytes, so it is a setting
of `xrce_client.d`: `RepresentationTimestamps.absent` (default, Annex A form,
16 octets) or `RepresentationTimestamps.present` (prose form). The agent has to
be set the same way (`representation_timestamps: absent | present` in its
configuration).

`Time_t` carries no `@extensibility` and is APPENDABLE (DDS-XTypes 1.3
7.3.1.2.1.8): DHEADER `8`, seconds `i32`, nanoseconds `u32`, little endian, 12
octets. `XrceUnannotatedTypes.final_` writes no DHEADER (8 octets);
`appendable` is the default.

```d
// client option (3rd/4th constructor argument); connect uses the wall clock
auto c = new XrceClient(key, transport, RepresentationTimestamps.present);
c.connect([cast(ubyte) 1, 0], [cast(ubyte) 0, 0]);
// pure encoder with explicit values (seconds, nanoseconds last)
auto r = encodeClientRepresentation(ver, vendor, key, 0x01, DefaultMtu,
             RepresentationTimestamps.present, XrceUnannotatedTypes.final_,
             1518905996, 500000000);
```

`encodeClientRepresentation(ver, vendor, key, session)` and `new XrceClient(key,
transport)` mean absent. Layouts (client_key `01 02 03 04`, session_id `01`,
requested_mtu 508, seconds 1518905996, nanoseconds 500000000):

```
absent (16):               58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28):  58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):       58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

The submessage length field and the message grow accordingly. The D client
reads `STATUS_AGENT` only as a state change (`markConnected`); it does not parse
`AGENT_Representation`.
