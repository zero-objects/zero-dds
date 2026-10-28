<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Python endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading(id, value, label)` samples and delivers them; a subscriber decodes
**every field**.

```sh
cd endpoints/python
python3 example_sync.py     # subscriber owns the run-loop and polls
python3 example_async.py    # subscriber iterates an asyncio async generator
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking; `None` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.stream()` is an `asyncio` async generator;
  the consumer iterates it with `async for`. The idiomatic asyncio model
  (Python 3 only).

## Transport

A transport is any object with `deliver(frame)` and `receive() -> frame|None`.
The examples use the in-memory `MemTransport` (a FIFO); a real UDP or serial link
is a drop-in — `zerodds_endpoint.py` also carries the XRCE and HDLC-serial framing.

## Reliable stream: unannotated types

Option `unannotated_types` with the values `"appendable"` (default) and
`"final"` (`UNANNOTATED_APPENDABLE` / `UNANNOTATED_FINAL` in
`zerodds_reliable.py`). DDS-XRCE 1.0 Annex A types without `@extensibility`
are APPENDABLE (DDS-XTypes 1.3 section 7.3.1.2.1.8) and carry a DHEADER;
`"final"` writes the FINAL form of the reference implementation (eProsima).
The SDK writes one such type, `ACKNACK_Payload`: 17-octet frame (length field 9)
with `"appendable"`, 13-octet frame (length field 5) with `"final"`.
`HEARTBEAT_Payload` is FINAL in both settings. `parse_acknack` accepts both forms
in either setting.

```python
rx = ReliableReceiver(unannotated_types="final")   # per receiver
rx.acknack_frame()                                  # 13 octets
acknack_frame(1, 1, 0, 0x80, unannotated_types="final")   # frame builder
```

`zerodds_endpoint.acknack_frame(..., unannotated_types="appendable")` takes the
same argument.

## XRCE client: representation timestamps

Option `representation_timestamps` with the values `"absent"` (default) and
`"present"`. DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`: the
prose of section 7.8.2.1 lists `Time_t client_timestamp` after
`xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not have it.
`"absent"` writes the Annex A form (what the SDK wrote before). `"present"`
writes the prose form: seconds `i32` and nanoseconds `u32`, little endian.
`Time_t` has no `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 section
7.3.1.2.1.8) with `unannotated_types="appendable"` (default): DHEADER = 8, 12
octets. With `unannotated_types="final"` there is no DHEADER, 8 octets.

The agent has to be set the same way (`representation_timestamps: absent |
present` in its configuration).

```python
client = XrceClient(key, representation_timestamps="present")
rep = client.client_representation()     # Time_t = wall clock at the call
encode_client_representation(ver, vendor, key, 1, 508,
                             representation_timestamps="present",
                             seconds=1518905996, nanoseconds=500000000)   # explicit
```

Bytes (client_key 01 02 03 04, session_id 01, version 01 00, vendor 00 00,
requested_mtu 508, seconds 1518905996, nanoseconds 500000000):

    absent (16):              58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
    present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
    present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01

The SDK does not read `AGENT_Representation`; `STATUS_AGENT` is a state change
only.

## Wire & codegen

The endpoint wire-core (`zerodds_wire.py`) is pure stdlib, byte-identical to the
Rust core (XCDR2, align cap 4, `f32` via `struct.pack("<f")`). IDL types can also
be generated with `zerodds-idlc generate --lang python` — a full IDL4 DataType
codegen, emitting plain `@dataclasses.dataclass` classes plus
`ZD_TOPIC_DESCRIPTORS` (run against `runtime/python`'s `zerodds_wire`, the
one Python CDR codec — not a hand-written decorator) — see
[`docs/specs/zerodds-xcdr2-python-1.0.md`](../../docs/specs/zerodds-xcdr2-python-1.0.md).
