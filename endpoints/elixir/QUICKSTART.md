<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Elixir endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/elixir
elixir -r lib/zerodds.ex example_sync.exs     # subscriber polls
elixir -r lib/zerodds.ex example_async.exs    # subscriber receives in its mailbox
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll/1` in a loop (non-blocking; `nil` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.start/2` spawns a BEAM process that sends
  `{:zerodds_sample, body}` to your mailbox; you `receive` them. The idiomatic
  OTP concurrency model (back-pressure, supervision, distribution all follow).

## Transport

`Transport` is `%{deliver: fun/1, receive: fun/0}`. The examples use the in-memory
`MemTransport` (an Agent-backed FIFO); a real UDP or shared-memory link is a
drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`<<v::float-little-32>>`). The same types can be generated from IDL with
`zerodds-idlc generate --lang elixir` — see
[`docs/specs/zerodds-xcdr2-elixir-1.0.md`](../../docs/specs/zerodds-xcdr2-elixir-1.0.md).

## Reliable stream: unannotated types

Option `unannotated_types:` of `ZeroDDS.Reliable.Receiver.new/1`, values
`:appendable` (default) and `:final`. DDS-XRCE 1.0 Annex A types without
`@extensibility` are APPENDABLE by DDS-XTypes 1.3 section 7.3.1.2.1.8 and carry a
DHEADER. `:final` writes the FINAL form of the reference implementation (eProsima).

```elixir
r = ZeroDDS.Reliable.Receiver.new(unannotated_types: :final)
frame = ZeroDDS.Reliable.Receiver.acknack_frame(r, 0x80, 0x00, msg_seq, hint_last_seen)
# the pure builder takes the mode as an optional 7th argument (default :appendable)
ZeroDDS.Reliable.acknack_frame(0x80, 0x00, 1, 1, 0, 0x80, :final)
```

Types covered: `ACKNACK_Payload` (`:appendable`: 17-octet frame, length field 9,
DHEADER = 5; `:final`: 13-octet frame, length field 5). This SDK writes no other
unannotated type (no `Time_t`, no `Property` entries: `CLIENT_Representation` carries
an absent `PropertySeq`). `HEARTBEAT_Payload` stays FINAL in both modes. The parsers
accept both forms in either setting.

## XRCE client: representation timestamps

DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`. The prose of 7.8.2.1 lists
`Time_t client_timestamp` after `xrce_vendor_id`; the IDL of Annex A and the bytes of
Annex B.1 do not have it. The bytes do not tell the two forms apart, so it is a setting.

- Option `representation_timestamps:`, values `:absent` (default, Annex A / B.1 form) and `:present` (prose form).
- Set on the client: `ZeroDDS.XrceClient.new(key, transport, representation_timestamps: :present)` or `ZeroDDS.XrceClient.set_representation_timestamps/2`; `ZeroDDS.XrceClient.client_representation/4` builds the representation for `connect/2` (wall clock for the timestamp).
- Set on the encoder: `ZeroDDS.XrceWire.encode_client_representation(version, vendor, key, session, mtu, representation_timestamps: :present, timestamp: {sec, nsec})`; without `timestamp:` the wall clock is used. `unannotated_types: :final` leaves out the DHEADER.
- The agent has to be set the same way (`representation_timestamps:` in its configuration).
- The client reads STATUS_AGENT only as a state change; it does not parse AGENT_Representation.

Layouts (client_key 01 02 03 04, session 0x01, mtu 508, 1518905996 s, 500000000 ns, little-endian):

```
absent (16):             58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

`Time_t` carries no `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8): DHEADER u32 = 8.
