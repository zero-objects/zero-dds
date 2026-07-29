# `zerodds-idlc`

ZeroDDS **IDL4-Compiler-CLI** — uebersetzt OMG IDL 4.2-Specs in
Sprach-Bindings fuer 17 Codegen-Backends: C, C++, Rust, TypeScript,
C#, Java, Python, Go, Ada, Zig, Nim, D, Elixir, OCaml, Julia, Lua,
Swift. Nutzt intern den `zerodds-idl`-Parser plus die
`zerodds-idl-<lang>`-Codegen-Crates und orchestriert sie ueber
einheitliche Flags.

Teil des Projekts [**ZeroDDS**](../../README.md). Safety-Klasse
**SAFE (std-only)** — Build-Zeit-Tool, deterministisch.

---

## Quick Start

```bash
# IDL parsen, AST drucken (kein Codegen)
zerodds-idlc --parse-only chat.idl

# IDL → Sprach-Code (eines der 17 Backends)
zerodds-idlc --rust   -o gen/rust   chat.idl     # → gen/rust/chat.rs
zerodds-idlc --c      -o gen/c      chat.idl     # → gen/c/chat.h
zerodds-idlc --cpp    -o gen/cpp    chat.idl     # → gen/cpp/chat.hpp
zerodds-idlc --ts     -o gen/ts     chat.idl     # → gen/ts/chat.ts
zerodds-idlc --csharp -o gen/cs     chat.idl     # → gen/cs/chat.cs
zerodds-idlc --java   -o gen/java   chat.idl     # → gen/java/<pkg>/<Class>.java
zerodds-idlc --python -o gen/py     chat.idl     # → gen/py/chat.py
# plus --go, --ada, --zig, --nim, --d, --elixir, --ocaml, --julia, --lua, --swift
```

`--rti` aktiviert das RTI-Connext-Grammar-Delta (akzeptiert
`keylist`-Direktive und andere Vendor-Konstrukte) — additiv auf die
OMG-IDL-4.2-Base.

## Flags

| Flag | Bedeutung |
| --- | --- |
| `--parse-only` | Parse + AST drucken, kein Codegen. |
| `--rti` | RTI-Connext-Delta zusaetzlich zur OMG-IDL-4.2-Base laden. |
| `--c` | C-Header-Backend (`zerodds-idl-cpp::c_mode`). |
| `--cpp` | C++17-Header-Backend (`zerodds-idl-cpp`). |
| `--rust` | Rust-Modul-Backend (`zerodds-idl-rust`). |
| `--ts` | TypeScript-Backend (`zerodds-idl-ts`). |
| `--csharp` | C#-Backend (`zerodds-idl-csharp`). |
| `--java` | Java-Backend (`zerodds-idl-java`) — Multi-File-Output im Package-Layout. |
| `--python` | Python-Backend (`zerodds-idl-python`). |
| `--go` | Go-Backend (`zerodds-idl-go`). |
| `--ada` | Ada-Backend (`zerodds-idl-ada`). |
| `--zig` | Zig-Backend (`zerodds-idl-zig`). |
| `--nim` | Nim-Backend (`zerodds-idl-nim`). |
| `--d` | D-Backend (`zerodds-idl-d`). |
| `--elixir` | Elixir-Backend (`zerodds-idl-elixir`). |
| `--ocaml` | OCaml-Backend (`zerodds-idl-ocaml`). |
| `--julia` | Julia-Backend (`zerodds-idl-julia`). |
| `--lua` | Lua-Backend (`zerodds-idl-lua`). |
| `--swift` | Swift-Backend (`zerodds-idl-swift`). |
| `--all` | Alle 17 Backends in einem Aufruf. |
| `-o, --output DIR` | Ausgabe-Verzeichnis. Pflicht fuer alle Backend-Modi. |
| `-h, --help` | Hilfetext. |
| `-V, --version` | Versions-Info. |

`--parse-only` und Backend-Flags sind **mutually exclusive**. Mehrere
Backend-Flags sind pro Aufruf erlaubt (kombinierbar, oder `--all`).

## Backends

| Flag | Output | Library-Crate | Spec |
| --- | --- | --- | --- |
| `--c` | `<dir>/<base>.h` | `zerodds-idl-cpp::c_mode` | ZeroDDS-C-Convention |
| `--cpp` | `<dir>/<base>.hpp` | `zerodds-idl-cpp` | OMG IDL4-CPP, DDS-PSM-CXX |
| `--rust` | `<dir>/<base>.rs` | `zerodds-idl-rust` | ZeroDDS-IDL-Rust 1.0 |
| `--ts` | `<dir>/<base>.ts` | `zerodds-idl-ts` | ZeroDDS DDS-TS 1.0 |
| `--csharp` | `<dir>/<base>.cs` | `zerodds-idl-csharp` | OMG IDL4-CSharp |
| `--java` | `<dir>/<pkg/path>/<Class>.java` | `zerodds-idl-java` | OMG IDL4-Java, DDS-Java-PSM |
| `--python` | `<dir>/<base>.py` | `zerodds-idl-python` | Python PSM, Annex B (see `crates/idl-python/README.md`) |
| `--go` | `<dir>/<base>.go` | `zerodds-idl-go` | no OMG mapping; ZeroDDS-defined, see `docs/idl-go.md` |
| `--ada` | `<dir>/<base>.ads`/`.adb` | `zerodds-idl-ada` | no OMG mapping; ZeroDDS-defined, see `docs/idl-ada.md` |
| `--zig` | `<dir>/<base>.zig` | `zerodds-idl-zig` | no OMG mapping; ZeroDDS-defined, see `docs/idl-zig.md` |
| `--nim` | `<dir>/<base>.nim` | `zerodds-idl-nim` | no OMG mapping; ZeroDDS-defined, see `docs/idl-nim.md` |
| `--d` | `<dir>/<base>.d` | `zerodds-idl-d` | no OMG mapping; ZeroDDS-defined, see `docs/idl-d.md` |
| `--elixir` | `<dir>/<base>.ex` | `zerodds-idl-elixir` | no OMG mapping; ZeroDDS-defined, see `docs/idl-elixir.md` |
| `--ocaml` | `<dir>/<base>.ml` | `zerodds-idl-ocaml` | no OMG mapping; ZeroDDS-defined, see `docs/idl-ocaml.md` |
| `--julia` | `<dir>/<base>.jl` | `zerodds-idl-julia` | no OMG mapping; ZeroDDS-defined, see `docs/idl-julia.md` |
| `--lua` | `<dir>/<base>.lua` | `zerodds-idl-lua` | no OMG mapping; ZeroDDS-defined, see `docs/idl-lua.md` |
| `--swift` | `<dir>/<base>.swift` | `zerodds-idl-swift` | no OMG mapping; ZeroDDS-defined, see `docs/idl-swift.md` |

Note: the Python **binding** crate (`crates/py`, `zerodds-py-1.0`) also
offers a separate codegen-free `@idl_struct` decorator path; this is
independent of the `--python` idlc backend above.

## Exit-Codes

| Code | Bedeutung |
| --- | --- |
| 0 | Erfolg |
| 1 | Parse-Fehler (Lex / Recognize / Build) |
| 2 | CLI-/IO-Fehler (Args, Datei nicht lesbar, fehlendes `-o`) |
| 3 | Backend-Fehler oder Feature noch nicht implementiert |

## Spec-Mapping

| Spec-Dokument | Abschnitt |
| --- | --- |
| OMG IDL 4.2 (ISO/IEC 19516) | §7 — Syntax + Semantik (via `zerodds-idl`) |
| OMG DDS-XTypes 1.3 | §7.4 — Sprach-Mapping-Regeln (per Backend) |

Pro-Sprache-PSM-Specs siehe README der jeweiligen `zerodds-idl-<lang>`-
Crate.

## Stabilitaet

`1.0.0-rc.2`. CLI-Flags und Exit-Codes sind ab 1.0.0-final stabil.
Generierter Code pro Sprache ist wire-byte-identisch zu Cyclone DDS /
RTI Connext / Fast-DDS.

## Tests

```bash
cargo test -p zerodds-idlc
```

17 Unit-Tests pro Backend (Codegen-Erfolg + Symbol-Verifikation) plus
CLI-Edge-Cases (mutually-exclusive, missing-output, unknown flags).

## See also

- [`zerodds-idl`](../../crates/idl/README.md) — OMG IDL 4.2-Parser (Input-Seite).
- [`zerodds-idl-cpp`](../../crates/idl-cpp/README.md) — C/C++-Codegen-Library.
- [`zerodds-idl-rust`](../../crates/idl-rust/README.md) — Rust-Codegen-Library.
- [`zerodds-idl-ts`](../../crates/idl-ts/README.md) — TypeScript-Codegen-Library.
- [`zerodds-idl-csharp`](../../crates/idl-csharp/README.md) — C#-Codegen-Library.
- [`zerodds-idl-java`](../../crates/idl-java/README.md) — Java-Codegen-Library.
- [`zerodds-idl-python`](../../crates/idl-python/README.md) — Python-Codegen-Library.
- [`zerodds-idl-go`](../../docs/idl-go.md) — Go-Codegen-Library.
- [`zerodds-idl-ada`](../../docs/idl-ada.md) — Ada-Codegen-Library.
- [`zerodds-idl-zig`](../../docs/idl-zig.md) — Zig-Codegen-Library.
- [`zerodds-idl-nim`](../../docs/idl-nim.md) — Nim-Codegen-Library.
- [`zerodds-idl-d`](../../docs/idl-d.md) — D-Codegen-Library.
- [`zerodds-idl-elixir`](../../docs/idl-elixir.md) — Elixir-Codegen-Library.
- [`zerodds-idl-ocaml`](../../docs/idl-ocaml.md) — OCaml-Codegen-Library.
- [`zerodds-idl-julia`](../../docs/idl-julia.md) — Julia-Codegen-Library.
- [`zerodds-idl-lua`](../../docs/idl-lua.md) — Lua-Codegen-Library.
- [`zerodds-idl-swift`](../../docs/idl-swift.md) — Swift-Codegen-Library.
