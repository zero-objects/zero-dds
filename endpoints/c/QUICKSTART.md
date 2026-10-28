<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS C endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/c
make examples
```

`make examples` builds and runs `build/example_sync` and `build/example_async`.
Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** (C89) — a poll loop over the transport (`zdw_endpoint_recv`
  + `zdw_xrce_read_frame`); the idiom when you own the run-loop.
- **`example_async`** (C11) — the event-driven reactor: `zdw_async_reader` binds
  an `on_sample` callback and `zdw_async_run` drains the transport, dispatching
  each decoded sample. No threads assumed, no malloc.

## Transport

A `zdw_transport` is `ctx` + `deliver` / `receive` function pointers. The examples
use an in-memory FIFO; a real UDP or serial link is a drop-in
(`examples/udp_endpoint.c` drives the same codec over a socket).

## Wire & codegen

The wire-core (`src/zerodds_wire.c`, `-std=c89 -pedantic`) is byte-identical to
the Rust core (XCDR2, align cap 4, `zdw_put_f32`). IDL types are generated with
`zerodds-idlc generate --lang c` (the C column of `crates/lang-rules`) — see
[`docs/specs/zerodds-xcdr2-c-1.0.md`](../../docs/specs/zerodds-xcdr2-c-1.0.md).

## Unannotated types (DDS-XRCE 1.0 Annex A)

`ACKNACK_Payload` carries no `@extensibility` in Annex A and is APPENDABLE by
DDS-XTypes 1.3 section 7.3.1.2.1.8: the ACKNACK body is 9 octets (DHEADER `5`,
then the 5 octets of the members), the frame 17 octets. This is the only
unannotated type the C SDK writes (CREATE_CLIENT writes no `Property`).
The setting is `ZDW_UNANNOTATED_APPENDABLE` (0, default) or
`ZDW_UNANNOTATED_FINAL` (1: body 5 octets, 13-octet frame, the form of the
reference implementation). It is set per reliable stream:

```c
zdw_reliable_init_with(&rx, 0x80, ZDW_UNANNOTATED_FINAL);
n = zdw_reliable_acknack_frame(&rx, buf, sizeof buf, 0x80, 0x00, seq);
/* pure builder: zdw_xrce_acknack_frame_with(..., ZDW_UNANNOTATED_FINAL);
 * zdw_xrce_acknack_frame(...) and zdw_reliable_init(...) mean APPENDABLE. */
```

`zdw_xrce_acknack_read` accepts both forms in either setting.
`HEARTBEAT_Payload` stays FINAL in both settings.

## Representation timestamps (DDS-XRCE 1.0 7.8.2.1)

The prose of 7.8.2.1 lists `Time_t client_timestamp` in `CLIENT_Representation`
after `xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not
have it. The two forms cannot be told apart by their bytes, so it is a setting:
`ZDWC_REPRESENTATION_TIMESTAMPS_ABSENT` (0, default, Annex A form, 16 octets)
or `ZDWC_REPRESENTATION_TIMESTAMPS_PRESENT` (1, prose form). The agent has to be
set the same way (`representation_timestamps: absent | present` in its
configuration).

`Time_t` carries no `@extensibility` and is APPENDABLE (DDS-XTypes 1.3
7.3.1.2.1.8): DHEADER `8`, seconds `i32`, nanoseconds `u32`, little endian, 12
octets. With `ZDWC_UNANNOTATED_FINAL` there is no DHEADER (8 octets).

```c
zdwc_client c;
zdwc_client_init(&c, key);
zdwc_set_representation_timestamps(&c, ZDWC_REPRESENTATION_TIMESTAMPS_PRESENT,
                                   ZDWC_UNANNOTATED_APPENDABLE);
n = zdwc_client_frame_create_client(&c, 0x01, buf, sizeof buf); /* wall clock */
/* pure builder with explicit values:
 * zdwc_frame_create_client_with(0x01, key, ZDWC_REPRESENTATION_TIMESTAMPS_PRESENT,
 *                               ZDWC_UNANNOTATED_APPENDABLE, sec, nsec, buf, cap);
 * zdwc_frame_create_client(...) means absent. */
```

Layouts (client_key `01 02 03 04`, session_id `01`, requested_mtu 508, seconds
1518905996, nanoseconds 500000000):

```
absent (16):               58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28):  58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):       58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

The submessage length field and the message grow accordingly. The C client
reads `STATUS_AGENT` only as a state change (`zdwc_mark_connected`); it does not
parse `AGENT_Representation`.
