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
| `native_framing.*` | XRCE `WRITE_DATA` frame + HDLC serial frame (byte stuffing + CRC-16/CCITT-FALSE) |

## Tests (all byte-identical to the Rust goldens)

| Test | Checks |
|------|--------|
| `test_native_identity` | final / nested / mutable, **LE + BE** |
| `test_native_roundtrip` | reader round-trips the `@final` sample, LE + BE |
| `test_native_framing` | XRCE + serial frames vs the `zerodds-xrce` goldens |
| `test_native_reflect` | the reflective path reproduces all three goldens, LE + BE |

## Build & test

```sh
cargo run -p zerodds-endpoint-golden -- endpoints/ada-native/build
cd endpoints/ada-native
make test GOLDEN_DIR=build
```

Requires GNAT + gprbuild (`apt install gnat gprbuild`). The BE checks, produced
on a little-endian host, are the host-endian-independence (native + BE) proof.
