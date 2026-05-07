# Codegen with `zerodds-idlc`

`zerodds-idlc` is the IDL compiler. It parses `.idl` files and emits
language-specific stubs.

## Invocation

```bash
zerodds-idlc <input.idl> [<input2.idl> …] \
  --rust   -o gen/rust    # Rust stubs
  --cpp    -o gen/cpp     # C++17
  --csharp -o gen/cs      # C# 10
  --java   -o gen/java    # Java 17
  --python -o gen/py      # Python 3.10+
  --ts     -o gen/ts      # TypeScript per DDS-TS 1.0
```

Multiple backends can be combined in one invocation.

## Outputs per backend

### Rust

```
gen/rust/
├── lib.rs                     # mod-tree mirror of IDL modules
└── robot/                     # module Robot
    ├── mod.rs
    ├── pose.rs                # struct Pose
    ├── pose_keyhash.rs        # KeyHash impl
    ├── pose_xcdr.rs           # CDR encoder + decoder
    └── telemetry.rs
```

The Rust crate exposes:

- `Robot::Pose` with derive `Clone`, `Debug`, `PartialEq`, `Serialize`,
  `Deserialize`.
- `impl zerodds_types::TopicType for Pose { fn type_name() -> &'static str; ... }`.
- `impl zerodds_cdr::Encode for Pose { fn encode(&self, w: &mut Writer); }`
  and the symmetric `Decode`.

### C++17

```
gen/cpp/
├── Robot.hpp
└── Robot.cpp
```

Uses RAII and `std::string` / `std::vector` / `std::array`.
Compatible with the `zerodds-cpp` binding crate.

### C#

```
gen/cs/
├── Robot/
│   ├── Pose.cs
│   └── Telemetry.cs
```

Plain `class` types with `[DataMember]` attributes. Compatible
with the `zerodds-cs` P/Invoke binding.

### Java 17

```
gen/java/com/example/robot/
├── Pose.java
└── Telemetry.java
```

POJO with `record` for immutable types where possible.

### TypeScript (per DDS-TS 1.0 spec)

```
gen/ts/
├── index.ts
├── Robot/
│   ├── Pose.ts                 # interface + cdr functions
│   └── Telemetry.ts
└── package.json
```

Compatible with both the `zerodds-ts-wasm` (browser) and koffi-based
node binding (`Welle 4a`).

### Python

```
gen/py/robot/
├── __init__.py
├── pose.py
└── telemetry.py
```

`@dataclass`-based, with explicit `encode_cdr()` / `decode_cdr()`
helpers.

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
  OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/Robot.hpp ${CMAKE_CURRENT_BINARY_DIR}/Robot.cpp
  COMMAND zerodds-idlc ${CMAKE_CURRENT_SOURCE_DIR}/Robot.idl --cpp -o ${CMAKE_CURRENT_BINARY_DIR}
  DEPENDS Robot.idl
)
```

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
