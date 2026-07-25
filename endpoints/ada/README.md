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
