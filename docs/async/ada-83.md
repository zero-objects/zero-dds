<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS async — pre-Object Ada 83 (native endpoint)

The **oldest-legacy** Ada variant: strict **Ada 83** (`-gnat83`) — no modular
types, no `Interfaces`, no tagged types, no access-to-subprogram, no child
units. For defense toolchains that predate Object Ada. Additive: the modern
procedural (`ada-native`) and Object-Ada variants are untouched.

Packages: [`Zerodds_Ada83_Wire`](../../endpoints/ada-83/src/zerodds_ada83_wire.ads)
· [`Zerodds_Ada83_Endpoint`](../../endpoints/ada-83/src/zerodds_ada83_endpoint.ads)

## How it stays byte-identical without modern types

Ada 83 has no unsigned/modular types and no shift intrinsics, so byte
extraction uses `Long_Integer` `div`/`mod` (host-independent); `f32` goes
through `Unchecked_Conversion` to a packed 4-byte array with host-endian
normalization. The output is byte-identical to the Rust core and the other SDKs
(verified against the goldens, LE and BE).

## Async model: poll-based (no callbacks)

Ada 83 has no access-to-subprogram, so there is no callback — the endpoint is
**poll-driven**, the way legacy Ada 83 event loops work: the application frames
samples with `Xrce_Write_Frame` and, on the receive side, polls its transport,
calling `Xrce_Read_Frame` + a `Reader` to decode each body.

## Tests

`make -C endpoints/ada-83 test` (in CI via `endpoints-native`):

- `test_ada83_identity` — the `@final` sample encoded LE + BE, byte-identical to
  the Rust goldens.
- `test_ada83_async` — N samples framed into a FIFO, drained by a poll loop and
  decoded in order.
