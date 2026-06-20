# Codegen with `zerodds-idlc`

`zerodds-idlc` is the IDL compiler. It parses `.idl` files and emits
language-specific stubs.

## Invocation

```bash
zerodds-idlc <input.idl> --<backend> -o <out-dir>
```

One backend per invocation. The seven backends:

| Flag | Output |
|---|---|
| `--c`      | C99 header (`<base>.h`) |
| `--cpp`    | C++17 header (`<base>.hpp`) |
| `--rust`   | Rust module (`<base>.rs`) |
| `--csharp` | C# 10 source (`<base>.cs`) |
| `--java`   | Java 17 files (`<pkg>/…`) |
| `--python` | Python 3.10+ module (`<base>.py`) |
| `--ts`     | TypeScript per DDS-TS 1.0 (`<base>.ts`) |

### CORBA modifier

`--corba` is a *modifier* flag (like `--rti`): combined with
`--cpp`, `--csharp`, `--java` or `--rust` it additionally emits the
CORBA service traits (client stubs + server skeletons).

```bash
zerodds-idlc Robot.idl --rust --corba -o gen/rust
# → gen/rust/Robot.rs        — DDS types
# → gen/rust/Robot_corba.rs  — CORBA service code
```

### Parse-only

`--parse-only` validates the grammar without emitting code; combine
with `--rti` to tolerate `@RTI_*` vendor annotations.

## Outputs per backend

Each backend emits a single source file per input IDL (`Robot.idl` →
`Robot.rs` / `Robot.h` / `Robot.hpp` / …), containing every type the
file defines. The Java backend is the exception — one `.java` file per
class under the package directory. See
[`idlc-handbook.md` §8](idlc-handbook.md) for the full layout table.

### Rust

`gen/rust/Robot.rs` — one module with every type plus its
`zerodds_cdr::CdrEncode` / `CdrDecode` impls. With `--corba`, a second
file `Robot_corba.rs` carries the CORBA service code.

### C++17

`gen/cpp/Robot.hpp` — a header-only C++17 unit. Uses RAII and
`std::string` / `std::vector` / `std::array`.

### C#

`gen/cs/Robot.cs` — `class` (or `record class`) types with
`[DataMember]` attributes.

### Java 17

One `.java` file per class under the package directory derived from the
IDL `module` tree (e.g. `gen/java/com/example/robot/Pose.java`). POJO
with `record` for immutable types where possible.

### TypeScript (per DDS-TS 1.0 spec)

`gen/ts/Robot.ts` — interfaces plus CDR functions, per the DDS-TS 1.0
PSM.

### Python

`gen/py/Robot.py` — `@dataclass`-based, with explicit `encode_cdr()` /
`decode_cdr()` helpers.

## Build integration

### Cargo (build.rs)

```rust
// build.rs
fn main() {
    println!("cargo:rerun-if-changed=Robot.idl");
    let status = std::process::Command::new("zerodds-idlc")
        .args(["Robot.idl", "--rust", "-o", "src/gen"])
        .status()
        .expect("zerodds-idlc");
    assert!(status.success());
}
```

### CMake (C++)

```cmake
add_custom_command(
  OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/Robot.hpp
  COMMAND zerodds-idlc ${CMAKE_CURRENT_SOURCE_DIR}/Robot.idl --cpp -o ${CMAKE_CURRENT_BINARY_DIR}
  DEPENDS Robot.idl
)
```

The C++ backend emits a single header-only `Robot.hpp` — no separate
`.cpp` translation unit.

### Maven (Java)

Use `exec-maven-plugin` to run `zerodds-idlc` in the `generate-sources`
phase.

### npm (TypeScript)

```json
{
  "scripts": {
    "build:idl": "zerodds-idlc src/Robot.idl --ts -o src/gen"
  }
}
```

## Cross-language consistency

A single `.idl` file produces seven byte-compatible payloads. The
KeyHash for the same instance is identical across languages — a
Java publisher and a Python subscriber on the same topic see the
same instances.

## Idempotency

`zerodds-idlc` outputs are deterministic — run twice on the same input
yields byte-identical files. Useful for `git diff` review of
generated code (we recommend committing the generated stubs for
language ecosystems where bootstrap-from-source is awkward).

## Spec references

- OMG IDL 4.2 — the source language.
- OMG XTypes 1.3 — annotations and TypeObject.
- DDS-TS 1.0 — TypeScript PSM (`documentation/specs/dds-ts-1.0/`).
- `tools/idlc/README.md` — invocation flags and exit codes.
