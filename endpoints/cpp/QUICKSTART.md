<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS C++ endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/cpp
make examples
```

`make examples` builds and runs `build/example_sync` and `build/example_async`.
Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** (C++98 facade) — a poll loop over the transport
  (`zdw_xrce_read_frame` + `zerodds::Reader`); the idiom when you own the
  run-loop.
- **`example_async`** (C++17 facade) — the event-driven reactor:
  `zerodds::AsyncReader` binds a `std::function` callback and `run()` drains the
  transport, dispatching each decoded sample. `zerodds::AsyncWriter` publishes.

## Transport

A `zdw_transport` is `ctx` + `deliver` / `receive` function pointers. The examples
use an in-memory FIFO; a real UDP or serial link is a drop-in.

## Wire & codegen

The C++ wire facade (`include/zerodds_wire.hpp`) is a thin `zerodds::Writer` /
`Reader` over the C89 core (`../c`), byte-identical to the Rust core (XCDR2, align
cap 4). IDL types are generated with `zerodds-idlc generate --lang c++` (a full IDL4 codegen:
struct/enum/union → `std::variant`/typedef/array/nested/map/@mutable) — see
[`docs/specs/zerodds-xcdr2-cpp-1.0.md`](../../docs/specs/zerodds-xcdr2-cpp-1.0.md).

## Unannotated types (DDS-XRCE 1.0 Annex A)

`ACKNACK_Payload` carries no `@extensibility` in Annex A and is APPENDABLE by
DDS-XTypes 1.3 section 7.3.1.2.1.8: the ACKNACK body is 9 octets (DHEADER `5`,
then the 5 octets of the members), the frame 17 octets. This is the only
unannotated type `zerodds_reliable.hpp` and the session client write
(CREATE_CLIENT writes no `Property`). The setting is
`UnannotatedTypes::Appendable` (default) or `UnannotatedTypes::Final` (body 5
octets, 13-octet frame, the form of the reference implementation). It is set per
`Receiver` or per call:

```cpp
Receiver rx(UnannotatedTypes::Final);
Bytes ack = rx.pending_acknack_frame(0x80, STREAM_NONE, seq);   // 13 octets
Bytes b = acknack_frame(0x80, STREAM_NONE, 1, 1, 0, 0, 0x80,
                        UnannotatedTypes::Final);               // pure builder
```

`parse_acknack` accepts both forms in either setting. `HEARTBEAT_Payload` stays
FINAL in both settings.

## Representation timestamps (DDS-XRCE 1.0 7.8.2.1)

The prose of 7.8.2.1 lists `Time_t client_timestamp` in `CLIENT_Representation`
after `xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not
have it. The two forms cannot be told apart by their bytes, so it is a setting:
`xrce::RepresentationTimestamps::Absent` (default, Annex A form, 16 octets) or
`xrce::RepresentationTimestamps::Present` (prose form). The agent has to be set
the same way (`representation_timestamps: absent | present` in its
configuration).

`Time_t` carries no `@extensibility` and is APPENDABLE (DDS-XTypes 1.3
7.3.1.2.1.8): DHEADER `8`, seconds `i32`, nanoseconds `u32`, little endian, 12
octets. `xrce::UnannotatedTypes::Final` writes no DHEADER (8 octets);
`Appendable` is the default. (This is the C++98 enum of
`zerodds_xrce_client.hpp`, separate from `zerodds::UnannotatedTypes` of
`zerodds_reliable.hpp`.)

```cpp
XrceClient c(key, xrce::RepresentationTimestamps::Present);   // 2nd/3rd ctor argument
auto f = c.create_client_frame(0x01);                          // wall clock at the call
// pure encoder with explicit values:
auto g = xrce::frame_create_client(0x01, key, xrce::RepresentationTimestamps::Present,
                                   xrce::UnannotatedTypes::Final, 1518905996L, 500000000UL);
```

`frame_create_client(session_id, key)` and `XrceClient(key)` mean absent. Layouts
(client_key `01 02 03 04`, session_id `01`, requested_mtu 508, seconds
1518905996, nanoseconds 500000000):

```
absent (16):               58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28):  58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):       58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

The submessage length field and the message grow accordingly. The client reads
`STATUS_AGENT` only as a state change; it does not parse `AGENT_Representation`.
