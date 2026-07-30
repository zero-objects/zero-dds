<!-- SPDX-License-Identifier: Apache-2.0 -->
# IDLC codegen vs native-endpoint coverage (step H review)

Two distinct code paths carry a type into a language:

- **IDL codegen backend** (`tools/idlc`, `Backend` enum → `crates/idl-*`): parses
  IDL and emits *typed* marshalling code. Each backend is a full emitter
  (2 000–20 000 LOC) with golden/wire-vector tests.
- **Native endpoint SDK** (ADR 0013, `endpoints/*`): a hand-written, byte-identical
  XCDR wire-core per language (the XRCE micro-endpoint reach). Proven against the
  same Rust goldens.

## Codegen backends

`zerodds-idlc` emits all 17 codegen backends: **C, C++, Rust, TypeScript, C#,
Java, Python** (the original "Phase 1: alle sieben Backends"), plus **Go,
Ada, Zig, Nim, D, Elixir, OCaml, Julia, Lua, Swift** added by the
full-program build. C is a mode of `idl-cpp`. Byte-identity is covered by
`idl-cpp/tests/xcdr2_wire_vectors`, `idl-rust/tests/wire_roundtrip`,
`idl-ts/tests/feature_golden_wire`, and the per-backend smoke tests. These map to
the languages with an OMG-standard or established DDS IDL mapping.

## Per-language map

| Language | IDL codegen backend | Native endpoint | Verdict |
|---|---|---|---|
| C | ✅ `--c` | ✅ `endpoints/c` | complete |
| C++ | ✅ `--cpp` | ✅ `endpoints/cpp` | complete |
| Rust | ✅ `--rust` | ✅ core | complete |
| Python | ✅ `--python` | ✅ `endpoints/python` | complete |
| Java | ✅ `--java` | ✅ `endpoints/java` | complete |
| C# | ✅ `--csharp` | — (binding) | complete |
| TypeScript | ✅ `--ts` | ✅ `endpoints/node` | complete |
| **Kotlin** | ↳ via `idl-java` | ✅ `endpoints/kotlin` | **covered by inheritance** — Kotlin is JVM and consumes the `idl-java` output directly (Java interop); a separate backend would duplicate it. |
| **F#** | ↳ via `idl-csharp` | ✅ `endpoints/fsharp` | **covered by inheritance** — F# is .NET and consumes the `idl-csharp` output directly; a separate backend would duplicate it. |
| Ada (Obj + 83) | ✅ `--ada` (`idl-ada`) | ✅ `endpoints/ada-native`, `ada-83` | **complete** — byte-identical, [idl-ada.md](../idl/idl-ada.md) |
| Go | ✅ `--go` (`idl-go`) | ✅ `endpoints/go` | **complete** — byte-identical, [idl-go.md](../idl/idl-go.md) |
| Zig | ✅ `--zig` (`idl-zig`) | ✅ `endpoints/zig` | **complete** — byte-identical, [idl-zig.md](../idl/idl-zig.md) |
| Elixir | ✅ `--elixir` (`idl-elixir`) | ✅ `endpoints/elixir` | **complete** — byte-identical, [idl-elixir.md](../idl/idl-elixir.md) |
| OCaml | ✅ `--ocaml` (`idl-ocaml`) | ✅ `endpoints/ocaml` | **complete** — byte-identical, [idl-ocaml.md](../idl/idl-ocaml.md) |
| Julia | ✅ `--julia` (`idl-julia`) | ✅ `endpoints/julia` | **complete** — byte-identical, [idl-julia.md](../idl/idl-julia.md) |
| Nim | ✅ `--nim` (`idl-nim`) | ✅ `endpoints/nim` | **complete** — byte-identical, [idl-nim.md](../idl/idl-nim.md) |
| D | ✅ `--d` (`idl-d`) | ✅ `endpoints/d` | **complete** — byte-identical, [idl-d.md](../idl/idl-d.md) |
| Lua | ✅ `--lua` (`idl-lua`) | ✅ `endpoints/lua` | **complete** — byte-identical, [idl-lua.md](../idl/idl-lua.md) |
| Swift | ✅ `--swift` (`idl-swift`) | ✅ `endpoints/swift` | **complete** — byte-identical (verified on macOS), [idl-swift.md](../idl/idl-swift.md) |

## Former candidate backends — closed

Go/Ada/Zig/Nim/D/Elixir/OCaml/Julia/Lua/Swift previously had no IDL *type*
codegen: a user writing IDL could not emit typed structs for them, only the
byte-identical hand-written wire core (the native endpoint SDKs). Each was a
fresh emitter (no OMG IDL language mapping exists for any of them) rather than
conformance work — see
[`internal/roadmap/async-native-plan.md`](../../internal/roadmap/async-native-plan.md)
for the per-backend build log. All ten have since shipped (`idl-go`, `idl-ada`,
`idl-zig`, `idl-nim`, `idl-d`, `idl-elixir`, `idl-ocaml`, `idl-julia`,
`idl-lua`, `idl-swift`) and are reflected as complete in the table above —
none remain open. Kotlin and F# are the only endpoint languages deliberately
left without their own backend, because they consume the JVM/.NET codegen
unchanged.
