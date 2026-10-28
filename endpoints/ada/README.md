<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS native Ada endpoint SDK — Stage 1

Thin Ada bindings over the C89 wire-core (`endpoints/c`), part of the native
endpoint SDK (ADR 0013). Stage 1 reuses the audited C XCDR primitives through
`Interfaces.C` / `pragma Import`, so an Ada endpoint produces byte output
identical to the Rust core and the C SDK. The pure-Ada wire (final / appendable
/ mutable, reflective, XRCE, serial) is **Stage 2**, `endpoints/ada-native`.

## Layout

| File | Purpose |
|------|---------|
| `src/zdw.ads` | `Interfaces.C` binding of `zerodds_wire.h` (writer/reader, primitives, DHEADER/EMHEADER helpers) |
| `src/sample_sensor.ads/.adb` | a representative `@final` type + its fixed codec, mirroring `endpoints/c/test/sample_sensor.*` |
| `src/sample_fixtures.ads/.adb` | the shared fixed test vector (matches `endpoints/golden-gen`) |
| `test/test_byte_identity.adb` | encodes LE+BE, compares to the Rust goldens, round-trips a decode |
| `test/test_udp_loopback.adb` | live UDP loopback E2E over `GNAT.Sockets` |
| `zerodds_ada.gpr` | GNAT project (compiles the Ada + the C wire-core) |

## Unannotated types setting (reliable stream)

`Reliable.Unannotated_Types` (`Appendable` | `Final`, default `Appendable`)
selects the form of the Annex A types of DDS-XRCE 1.0 that carry no
`@extensibility` annotation. DDS-XTypes 1.3 section 7.3.1.2.1.8 makes them
APPENDABLE (DHEADER in front of the members); `Final` writes the FINAL form of
the reference implementation (eProsima).

- Frame builder: `Reliable.Acknack_Frame (First, Bitmap, Stream, Msg_Seq, Mode)`;
  `Mode` is the last parameter and defaults to `Appendable`.
- Reliable receiver: `Receiver_State.Mode` (set per receiver); `Receiver_Acknack`
  builds the ACKNACK in that form.
- Types covered: `ACKNACK_Payload` (the only unannotated type this SDK writes).
  `Appendable`: 9-octet body (DHEADER = 5), length field 9, 17-octet frame.
  `Final`: 5-octet body, length field 5, 13-octet frame.
- `HEARTBEAT_Payload` and the other Annex A types annotated FINAL stay FINAL in
  both modes.
- `Parse_Acknack` accepts both forms in either setting.

## Representation timestamps (XRCE client)

`Xrce_Client.Representation_Timestamps` (`Absent` | `Present`, default
`Absent`). DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`: the
prose of section 7.8.2.1 lists `Time_t client_timestamp` after
`xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not have it.
The forms cannot be told apart by their bytes, so the SDK has a setting.

- Encoder: `Encode_Client_Representation (..., Timestamps, Seconds, Nanoseconds, Mode)`;
  the four parameters are last and default to `Absent`, 0, 0, `Reliable.Appendable`.
  Existing calls keep meaning `Absent`, bytes unchanged.
- Client: `Set_Representation_Timestamps (C, Value, Mode)`; `Frame_Connect (C,
  Version, Vendor_Id, Requested_Mtu)` builds the CREATE_CLIENT frame and reads
  the wall clock at the time of the call when `Present`.
- `Mode` (`Reliable.Unannotated_Types`): `Time_t` has no `@extensibility`, so it
  is APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8, DHEADER u32 = 8, 12 octets);
  `Final` writes it without DHEADER (8 octets).
- The agent has to be set the same way: `representation_timestamps: absent | present`
  in its configuration.
- Layouts (client_key 01 02 03 04, session_id 01, vendor 00 00, version 01 00,
  requested_mtu 508, seconds 1518905996, nanoseconds 500000000, little endian):
  - absent (16): `58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01`
  - present, appendable (28): `58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01`
  - present, final (24): `58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01`
- The submessage length field and the message grow accordingly.
- `Mark_Connected` takes STATUS_AGENT as a state change only; AGENT_Representation
  is not parsed.

## Build & test

Generate the goldens once (from the workspace root), then build and run:

```sh
cargo run -p zerodds-endpoint-golden -- endpoints/ada/build
cd endpoints/ada
make test GOLDEN_DIR=build
```

Expected: `LE: … byte-identical to Rust golden`, `BE: …`, round-trip `ok`, and
`UDP loopback round-trip ok`.

Requires GNAT + gprbuild (`apt install gnat gprbuild`).
