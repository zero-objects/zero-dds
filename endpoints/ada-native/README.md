<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS native Ada endpoint SDK — Stage 2 (pure Ada)

A **pure Ada** XCDR wire-core — no C, no FFI — part of the native endpoint SDK
(ADR 0013). Byte-for-byte identical to the Rust core (`zerodds-cdr`) and the C
SDK (`endpoints/c`): serialization is by explicit byte order, so the output is
independent of the host endianness and a big-endian target produces the same
wire as an x86-64 host. The wire-core is written in a restricted,
contract-carrying subset (`SPARK_Mode => On` on the spec) so it is amenable to
SPARK analysis.

## Coverage

| Unit | What it proves |
|------|----------------|
| `zerodds_native_wire.*` | XCDR2 primitives + alignment + DHEADER/EMHEADER, LE/BE |
| `native_samples.*` | `@final`, `@appendable` (nested + `sequence<struct>`), `@mutable` encoders |
| `native_reflect.*` | descriptor-driven reflective codec over a runtime value tree |
| `native_framing.*` | XRCE `WRITE_DATA` frame + DDS-XRCE Annex C serial frame (addresses, length, byte stuffing, RFC 1662 FCS-16) |

## Tests (all byte-identical to the Rust goldens)

| Test | Checks |
|------|--------|
| `test_native_identity` | final / nested / mutable, **LE + BE** |
| `test_native_roundtrip` | reader round-trips the `@final` sample, LE + BE |
| `test_native_framing` | XRCE + serial frames vs the `zerodds-xrce` goldens |
| `test_native_reflect` | the reflective path reproduces all three goldens, LE + BE |

## Unannotated types setting (reliable stream)

`Native_Reliable.Unannotated_Types` (`Appendable` | `Final`, default
`Appendable`) selects the form of the Annex A types of DDS-XRCE 1.0 that carry
no `@extensibility` annotation. DDS-XTypes 1.3 section 7.3.1.2.1.8 makes them
APPENDABLE (DHEADER in front of the members); `Final` writes the FINAL form of
the reference implementation (eProsima).

- Frame builder: `Acknack_Frame (First_Unacked, Bitmap, Stream, Msg_Seq, Mode)`;
  `Mode` is the last parameter and defaults to `Appendable`.
- Reliable receiver: `Receiver_State.Mode` (set per receiver); `Receiver_Acknack`
  builds the ACKNACK in that form.
- Types covered: `ACKNACK_Payload` (the only unannotated type this SDK writes).
  `Appendable`: 9-octet body (DHEADER = 5), length field 9, 17-octet frame.
  `Final`: 5-octet body, length field 5, 13-octet frame.
- `HEARTBEAT_Payload` and the other Annex A types annotated FINAL stay FINAL in
  both modes.
- `Parse_Acknack` accepts both forms in either setting.

## Build & test

```sh
cargo run -p zerodds-endpoint-golden -- endpoints/ada-native/build
cd endpoints/ada-native
make test GOLDEN_DIR=build
```

Requires GNAT + gprbuild (`apt install gnat gprbuild`). The BE checks, produced
on a little-endian host, are the host-endian-independence (native + BE) proof.
