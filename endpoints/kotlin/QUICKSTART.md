<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Kotlin endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/kotlin
kotlinc src/Zerodds.kt example_sync/Example.kt  -include-runtime -d sync.jar  && java -jar sync.jar
kotlinc src/Zerodds.kt example_async/Example.kt -include-runtime -d async.jar && java -jar async.jar
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking; `null` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader` runs a daemon thread that pushes decoded
  frames onto a `LinkedBlockingQueue`; you `samples.take()` (blocking) and
  `close()` when done. The idiom for channel-style consumers.

## Transport

`Transport` is a two-method interface (`deliver` / `receive`). The examples use
the thread-safe in-memory `MemTransport`; a real UDP or shared-memory link is a
drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`Float.floatToRawIntBits`). Kotlin is JVM-native, so IDL types are generated with
`zerodds-idlc generate --lang java` and consumed directly — see
[`docs/specs/zerodds-xcdr2-kotlin-1.0.md`](../../docs/specs/zerodds-xcdr2-kotlin-1.0.md).

## Reliable stream: unannotated types

`UnannotatedTypes` (package `zerodds`, `src/Reliable.kt`) selects the
extensibility used for the DDS-XRCE 1.0 Annex A types that carry no
`@extensibility`. Per DDS-XTypes 1.3 section 7.3.1.2.1.8 these are APPENDABLE and
carry a DHEADER.

| Value | Form | ACKNACK |
| --- | --- | --- |
| `UnannotatedTypes.APPENDABLE` (default) | DHEADER in front of the struct | body 9 octets (DHEADER = 5), length field 9, frame 17 octets |
| `UnannotatedTypes.FINAL` | no DHEADER, form of eProsima | body 5 octets, length field 5, frame 13 octets |

Set it per receiver: `ReliableReceiver(unannotatedTypes = UnannotatedTypes.FINAL)`;
`receiver.pendingAcknackFrame(hint)` then builds the frame in that form. The frame
builder takes it as an optional argument: `acknackFrame(ack, UnannotatedTypes.FINAL)`;
without the argument it means APPENDABLE.

Types covered: ACKNACK_Payload (the only unannotated Annex A type the Kotlin SDK
writes among the reliable-stream types). HEARTBEAT_Payload stays FINAL in both
settings. `CLIENT_Representation` is not affected by this setting; see
"XRCE client: representation timestamps" below.
`parseAckNack` accepts both forms in either setting.

## XRCE client: representation timestamps

DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`: the prose of section
7.8.2.1 lists `Time_t client_timestamp` after `xrce_vendor_id`; the IDL of Annex A
and the bytes of Annex B.1 do not have it. The forms cannot be told apart by their
bytes, so it is a setting.

| Value | Form |
| --- | --- |
| `RepresentationTimestamps.ABSENT` (default) | Annex A / B.1 form, 16 octets |
| `RepresentationTimestamps.PRESENT` | prose form of 7.8.2.1, `Time_t` after `xrce_vendor_id` |

Set it on the client: `XrceClient(key, RepresentationTimestamps.PRESENT)` or
`client.representationTimestamps = ...`; `client.frameCreateClient()` then uses the wall
clock at the time of the call. The pure encoder takes explicit values:
`XrceFrame.encodeClientRepresentation(version, vendorId, key, sessionId, mtu, timestamps,
extensibility, seconds, nanoseconds)`, likewise `XrceFrame.createClient(sessionId, key,
timestamps, extensibility, seconds, nanoseconds)`. Without these arguments the meaning is
ABSENT, bytes unchanged. `Time_t` carries no `@extensibility`, so it is APPENDABLE
(DDS-XTypes 1.3 7.3.1.2.1.8): `XrceUnannotatedTypes.APPENDABLE` (default, also
`client.unannotatedTypes`) writes a DHEADER = 8, `FINAL` none.

The agent has to be set the same way (`representation_timestamps: absent | present` in its
configuration). Layouts (client_key 01 02 03 04, session_id 1, vendor 00 00, version 01 00,
requested_mtu 508, seconds 1518905996, nanoseconds 500000000, little endian):

```
absent (16):              58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

The submessage length field of CREATE_CLIENT and the message grow accordingly. The
Kotlin client does not parse `AGENT_Representation`; STATUS_AGENT is only a state change
(`markConnected()`).
