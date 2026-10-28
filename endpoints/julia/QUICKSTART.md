<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Julia endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/julia
julia example_sync.jl     # subscriber owns the run-loop and polls
julia example_async.jl    # subscriber blocks on a Channel filled by a Task
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `ZeroDDS.poll` in a loop (non-blocking; `nothing` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `ZeroDDS.start_reader` spawns a `Task` that fills a
  `Channel`; the consumer blocks on `ZeroDDS.recv` (`take!`). The idiomatic Julia
  concurrency model.

## Transport

`Transport` is `(deliver::Function, receive::Function)`. The examples use the
in-memory `mem_transport()` (a FIFO); a real UDP or shared-memory link is a
drop-in.

## Reliable stream: unannotated types

Keyword `unannotated_types` of type `Reliable.UnannotatedTypes` with the values
`UNANNOTATED_APPENDABLE` (default) and `UNANNOTATED_FINAL`. DDS-XRCE 1.0 Annex A
types without `@extensibility` are APPENDABLE (DDS-XTypes 1.3 section
7.3.1.2.1.8) and carry a DHEADER; `UNANNOTATED_FINAL` writes the FINAL form of
the reference implementation (eProsima). The SDK writes one such type,
`ACKNACK_Payload`: 17-octet frame (length field 9) with `UNANNOTATED_APPENDABLE`,
13-octet frame (length field 5) with `UNANNOTATED_FINAL`. `HEARTBEAT_Payload` is
FINAL in both settings. `parse_acknack` accepts both forms in either setting.

```julia
rx = Reliable.Receiver(unannotated_types = Reliable.UNANNOTATED_FINAL)  # per receiver
Reliable.acknack_frame(rx)                                              # 13 octets
Reliable.acknack_frame(0x80, Reliable.STREAM_NONE, 1, UInt16(1), UInt8(0), UInt8(0), 0x80;
                       unannotated_types = Reliable.UNANNOTATED_FINAL)  # frame builder
```

## XRCE client: representation timestamps

Keyword `representation_timestamps` with the values `:absent` (default) and
`:present`. DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`: the
prose of section 7.8.2.1 lists `Time_t client_timestamp` after
`xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not have it.
`:absent` writes the Annex A form (what the SDK wrote before). `:present`
writes the prose form: seconds `Int32` and nanoseconds `UInt32`, little endian.
`Time_t` has no `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 section
7.3.1.2.1.8) with `unannotated_types = :appendable` (default): DHEADER = 8, 12
octets. With `unannotated_types = :final` there is no DHEADER, 8 octets.

The agent has to be set the same way (`representation_timestamps: absent |
present` in its configuration).

```julia
c = XrceClient.Client(key, transport; representation_timestamps = :present)
XrceClient.connect!(c)    # CREATE_CLIENT with client_representation(c), Time_t = wall clock
XrceClient.encode_client_representation(ver, vendor, key, 0x01;
    representation_timestamps = :present, seconds = 1518905996, nanoseconds = 500000000)
```

Bytes (client_key 01 02 03 04, session_id 01, version 01 00, vendor 00 00,
requested_mtu 508, seconds 1518905996, nanoseconds 500000000):

    absent (16):              58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
    present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
    present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01

The SDK does not read `AGENT_Representation`; `STATUS_AGENT` is a state change
only.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`reinterpret(UInt32, Float32(v))`). The same types can be generated from IDL with
`zerodds-idlc generate --lang julia` — see
[`docs/specs/zerodds-xcdr2-julia-1.0.md`](../../docs/specs/zerodds-xcdr2-julia-1.0.md).
