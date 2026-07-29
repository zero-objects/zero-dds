<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — IDL → Go codegen backend

`zerodds-idlc --go` emits **native Go** from OMG IDL: a self-contained source
file with a shared XCDR2 `Writer` (byte-identical to the hand-written
`endpoints/go` core) and, per IDL `struct`, a Go struct plus a
`MarshalXCDR(endian)` method whose wire output matches the Rust goldens exactly.

Crate: [`crates/idl-go`](../crates/idl-go) · registered in `tools/idlc`
(`Backend::Go`, flag `--go`, part of `--all`).

## Usage

```sh
zerodds-idlc types.idl --go        # emits types.go
zerodds-idlc types.idl --all       # all eight backends, incl. Go
```

## Mapping

| IDL | Go |
|-----|-----|
| `struct` `@final` | struct + compact `MarshalXCDR` (no DHEADER) |
| `struct` `@appendable` | struct + DHEADER-framed `MarshalXCDR` |
| `octet` / `uint8` | `byte` / `uint8` |
| `short`..`long long` (signed/unsigned) | `int16`..`uint64` |
| `float` / `double` | `float32` / `float64` |
| `boolean` / `char` | `bool` / `byte` |
| `string` | `string` |
| `sequence<octet>` | `[]byte` |
| `sequence<primitive>` | `[]T` (u32 count + per-element) |

`@mutable`, unions, nested-struct members, maps, `long double`, `wchar`, and
`wstring` currently raise `IdlGoError::Unsupported` (tracked for follow-up).

## Byte-identity (CI job `idl-go`)

The `crates/idl-go` test suite: string smoke tests always, plus
`byte_identity_vs_rust_goldens` — generates Go for the `@final` golden type,
compiles and runs it (`go run`), and asserts its LE + BE wire equals
`golden_le.bin` / `golden_be.bin`. Toolchain: `golang-go` from apt.
