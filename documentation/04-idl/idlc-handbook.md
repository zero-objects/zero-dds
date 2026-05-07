# `zerodds-idlc` Handbook

End-user reference for the ZeroDDS IDL compiler. This document is the
primary operator manual: install, invoke, integrate, and troubleshoot.

For language syntax see [`language.md`](language.md); for annotation
semantics see [`annotations.md`](annotations.md); for the conceptual
codegen flow see [`codegen.md`](codegen.md); for the resulting
on-the-wire byte form see [`cdr-wire-format.md`](cdr-wire-format.md).

---

## 1 What is `zerodds-idlc`?

`zerodds-idlc` is the OMG IDL 4.2 compiler that turns `.idl` schema
files into language stubs (Rust, C, C++, C#, Java, Python, TypeScript)
plus the matching XCDR1 / XCDR2 encoder / decoder and `KeyHash`
implementation. It is the single source of truth for cross-language
type compatibility: a Rust publisher and a Java subscriber see the
same wire bytes for the same schema because both compile the same
`.idl` through the same compiler.

Internally `zerodds-idlc` is a thin CLI on top of the `zerodds-idl`
parser crate (Earley engine + grammar deltas) and the per-language
emitter crates (`zerodds-idl-rust`, `zerodds-idl-cpp`,
`zerodds-idl-csharp`, `zerodds-idl-java`, `zerodds-idl-ts`).

---

## 2 Installation

### 2.1 From source

```bash
git clone https://github.com/zero-objects/zero-dds.git
cd zero-dds
cargo install --path tools/idlc
zerodds-idlc --version
```

### 2.2 From crates.io

```bash
cargo install zerodds-idlc
```

### 2.3 Homebrew (macOS / Linux)

```bash
brew tap zero-objects/zero-dds
brew install zerodds-idlc
```

### 2.4 APT (Debian / Ubuntu)

```bash
curl -fsSL https://zerodds.org/apt/zerodds.gpg \
  | sudo tee /etc/apt/keyrings/zerodds.gpg > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/zerodds.gpg] \
  https://zerodds.org/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/zerodds.list
sudo apt update
sudo apt install zerodds-idlc
```

### 2.5 Scoop (Windows)

```powershell
scoop bucket add zerodds https://github.com/zero-objects/scoop-zerodds
scoop install zerodds-idlc
```

### 2.6 Verifying the install

```bash
zerodds-idlc --version
zerodds-idlc --help
```

---

## 3 CLI reference

### 3.1 Top-level synopsis

```
zerodds-idlc [GLOBAL-FLAGS] <COMMAND> [ARGS...]
zerodds-idlc <input.idl> [<input2.idl>...] [BACKEND-FLAGS]
```

The compiler accepts both a sub-command form (`generate`, `check`,
`dump-ast`, `dump-typeobject`) and a flat form where backend flags
are passed directly. The flat form is preserved for legacy build
scripts; new build scripts should use `generate`.

### 3.2 Global flags

| Flag | Description |
|---|---|
| `-h`, `--help` | Print help and exit |
| `-V`, `--version` | Print version and exit |
| `-v`, `--verbose` | Increase log verbosity (repeat for more, max `-vvv`) |
| `-q`, `--quiet` | Suppress non-error output |
| `--color <when>` | `auto` (default), `always`, `never` |
| `-I <path>` | Add `<path>` to include-search list (repeatable) |
| `-D <name>[=<value>]` | Define preprocessor macro |
| `--no-default-includes` | Suppress built-in include paths |

### 3.3 Sub-command: `generate`

Compile one or more `.idl` files into language stubs.

```
zerodds-idlc generate <input.idl>... [OPTIONS]
```

| Flag | Description |
|---|---|
| `--rust` | Emit Rust stubs |
| `--c` | Emit C99 stubs |
| `--cpp` | Emit C++17 stubs |
| `--csharp` | Emit C# 10 stubs |
| `--java` | Emit Java 17 stubs |
| `--python` | Emit Python 3.10+ stubs |
| `--ts` | Emit TypeScript per DDS-TS 1.0 |
| `--all` | Emit all backends in one invocation |
| `-o <path>` | Output directory (per-backend sub-tree under it) |
| `--out-rust <path>` | Override Rust output dir |
| `--out-cpp <path>` | Override C++ output dir |
| `--out-csharp <path>` | Override C# output dir |
| `--out-java <path>` | Override Java output dir |
| `--out-python <path>` | Override Python output dir |
| `--out-ts <path>` | Override TS output dir |
| `--package <name>` | Override top-level package / module name |
| `--namespace <name>` | C++ / C# alias for `--package` |
| `--xcdr1` | Emit XCDR1-only encoders (default: XCDR1+XCDR2) |
| `--xcdr2-only` | Emit XCDR2-only encoders |
| `--with-typeobject` | Emit `TypeObject` constants (default: on for `--rust` / `--cpp`) |
| `--no-typeobject` | Suppress `TypeObject` emission |
| `--with-keyhash-md5` | Force MD5 key-hash even for `<= 16 byte` keys |
| `--no-cdr` | Skip CDR encoder/decoder emission (type-defs only) |
| `--no-keyhash` | Skip `KeyHash` emission |
| `--rust-edition <2021\|2024>` | Rust edition for `Cargo.toml` (default 2024) |
| `--rust-no-cargo-toml` | Do not emit a `Cargo.toml` (caller manages it) |
| `--cpp-include-prefix <p>` | Prepend `<p>/` to generated `#include` paths |
| `--java-package-prefix <p>` | Prepend `<p>` to all Java packages |
| `--ts-module <esm\|cjs>` | TypeScript module form (default `esm`) |
| `--python-package-style <flat\|nested>` | Python package layout |

### 3.4 Sub-command: `check`

Parse `.idl` and validate without emitting code. Exit code 0 on
success, non-zero on parse / validation error.

```
zerodds-idlc check <input.idl>...
```

| Flag | Description |
|---|---|
| `--strict` | Reject XTypes-deviations (vendor-specific extensions, RTI/OpenSplice deltas) |
| `--rti` | Enable RTI Connext grammar delta |
| `--opendds` | Enable OpenDDS grammar delta |
| `--cyclone` | Enable Eclipse Cyclone grammar delta |
| `--show-warnings` | Print non-fatal warnings (default: stderr) |

### 3.5 Sub-command: `dump-ast`

Dump the parsed AST as JSON to stdout. Useful for tooling that wants
to introspect the IDL without writing a parser.

```
zerodds-idlc dump-ast <input.idl> [--format json|s-expr|pretty]
```

### 3.6 Sub-command: `dump-typeobject`

Print the `TypeObject` (XTypes 1.3 §7.3.4) for each top-level type
in the input.

```
zerodds-idlc dump-typeobject <input.idl> [--format hex|json|cdr]
```

### 3.7 Sub-command: `print-deps`

Print Make-style dependency information. Used by build-system glue.

```
zerodds-idlc print-deps <input.idl>     # → "input.o: input.idl include1.idl ..."
```

### 3.8 Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Parse error (lex / syntax / build) |
| `2` | CLI usage error (bad flag, missing input, IO error) |
| `3` | Backend not supported on this build |
| `4` | Validation error (semantic check failed) |
| `5` | Output IO error (cannot write generated file) |

---

## 4 Per-language backends

### 4.1 Rust

```bash
zerodds-idlc generate Robot.idl --rust -o gen/rust
```

Output layout:

```
gen/rust/
├── Cargo.toml                # name = robot, derives zerodds-types
├── src/
│   ├── lib.rs                # mod-tree + pub re-exports
│   └── robot/
│       ├── mod.rs            # module Robot
│       ├── pose.rs           # struct Pose + #[derive(DdsType)]
│       ├── pose_keyhash.rs   # impl KeyHash for Pose
│       ├── pose_xcdr.rs      # impl Encode<Xcdr2> + Decode<Xcdr2>
│       └── telemetry.rs
└── README.md                 # auto-generated usage
```

`Cargo.toml` integration:

```toml
[dependencies]
robot = { path = "gen/rust" }
zerodds-rs = "1"
```

Use:

```rust
use robot::Pose;
use zerodds_rs::{DomainParticipantFactory, TopicQos};

let pose = Pose { robot_id: "r1".into(), x: 0.0, y: 0.0, z: 0.0, yaw: 0.0 };
let writer = participant.create_datawriter::<Pose>(&topic, qos)?;
writer.write(&pose)?;
```

### 4.2 C99

```bash
zerodds-idlc generate Robot.idl --c -o gen/c
```

Output layout:

```
gen/c/
├── Robot.h                   # struct + function decls
├── Robot.c                   # encode / decode / keyhash
└── Robot_typeobject.h        # XTypes TypeObject
```

CMake integration:

```cmake
add_custom_command(
  OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/Robot.h
         ${CMAKE_CURRENT_BINARY_DIR}/Robot.c
  COMMAND zerodds-idlc generate
          ${CMAKE_CURRENT_SOURCE_DIR}/Robot.idl
          --c -o ${CMAKE_CURRENT_BINARY_DIR}
  DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/Robot.idl)
add_library(robot ${CMAKE_CURRENT_BINARY_DIR}/Robot.c)
target_link_libraries(robot PUBLIC zerodds-c)
```

### 4.3 C++17

```bash
zerodds-idlc generate Robot.idl --cpp -o gen/cpp
```

Output layout:

```
gen/cpp/
├── Robot.hpp                 # namespaces + class definitions
├── Robot.cpp                 # serialise / deserialise
├── RobotPubSubTypes.hpp      # DDS-PSM-Cxx Topic-Trait
└── RobotPubSubTypes.cpp
```

CMake integration:

```cmake
add_custom_command(
  OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/Robot.hpp
         ${CMAKE_CURRENT_BINARY_DIR}/Robot.cpp
  COMMAND zerodds-idlc generate
          ${CMAKE_CURRENT_SOURCE_DIR}/Robot.idl
          --cpp -o ${CMAKE_CURRENT_BINARY_DIR}
  DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/Robot.idl)
add_library(robot ${CMAKE_CURRENT_BINARY_DIR}/Robot.cpp)
target_link_libraries(robot PUBLIC zerodds-cpp)
```

Use:

```cpp
#include "Robot.hpp"
#include <dds/dds.hpp>

dds::topic::Topic<Robot::Pose> topic(participant, "Pose");
dds::pub::DataWriter<Robot::Pose> writer(publisher, topic);
writer.write({"r1", 0, 0, 0, 0});
```

### 4.4 C# 10

```bash
zerodds-idlc generate Robot.idl --csharp -o gen/cs
```

Output layout:

```
gen/cs/
├── Robot/
│   ├── Pose.cs               # public class Pose
│   ├── Telemetry.cs
│   └── PoseTypeSupport.cs    # ITypeSupport impl
└── Robot.csproj              # auto-generated, references ZeroDDS.Core
```

`csproj` integration:

```xml
<ItemGroup>
  <ProjectReference Include="gen/cs/Robot.csproj" />
</ItemGroup>
```

### 4.5 Java 17

```bash
zerodds-idlc generate Robot.idl --java -o gen/java \
  --java-package-prefix com.example
```

Output layout:

```
gen/java/com/example/robot/
├── Pose.java                 # POJO (record where possible)
├── PoseTypeSupport.java      # TypeSupport (org.omg.dds API)
├── Telemetry.java
└── module-info.java          # JPMS module descriptor
```

Maven integration:

```xml
<plugin>
  <groupId>org.codehaus.mojo</groupId>
  <artifactId>exec-maven-plugin</artifactId>
  <executions>
    <execution>
      <phase>generate-sources</phase>
      <goals><goal>exec</goal></goals>
      <configuration>
        <executable>zerodds-idlc</executable>
        <arguments>
          <argument>generate</argument>
          <argument>${project.basedir}/src/main/idl/Robot.idl</argument>
          <argument>--java</argument>
          <argument>-o</argument>
          <argument>${project.build.directory}/generated-sources/java</argument>
        </arguments>
      </configuration>
    </execution>
  </executions>
</plugin>
```

### 4.6 Python 3.10+

```bash
zerodds-idlc generate Robot.idl --python -o gen/py
```

Output layout:

```
gen/py/robot/
├── __init__.py
├── pose.py                   # @dataclass Pose
├── telemetry.py
└── _xcdr.py                  # encode_cdr() / decode_cdr() helpers
```

`pyproject.toml` integration:

```toml
[build-system]
requires = ["setuptools>=64", "zerodds-idlc-build>=1.0"]
build-backend = "zerodds_idlc_build"

[tool.zerodds-idlc]
sources = ["src/idl/Robot.idl"]
output = "src/robot"
```

### 4.7 TypeScript (DDS-TS 1.0)

```bash
zerodds-idlc generate Robot.idl --ts -o gen/ts
```

Output layout:

```
gen/ts/
├── package.json              # name, types, exports
├── tsconfig.json
├── src/
│   ├── index.ts              # re-exports
│   └── robot/
│       ├── Pose.ts           # interface + cdr fns
│       └── Telemetry.ts
```

npm integration:

```json
{
  "scripts": {
    "build:idl": "zerodds-idlc generate src/idl/Robot.idl --ts -o src/gen"
  }
}
```

---

## 5 Annotations and pragmas

The compiler honours the following annotations from XTypes 1.3 and
DDS 1.4. See [`annotations.md`](annotations.md) for full semantics.

| Annotation | Effect on codegen |
|---|---|
| `@key` | Field is part of the instance key; included in `KeyHash`. |
| `@id(N)` | Member ID for `MUTABLE` extensibility — preserved across versions. |
| `@hashid("name")` | Use 32-bit MurmurHash3 of the literal as member ID. |
| `@final` | Struct cannot evolve; no `DHEADER` on the wire. |
| `@appendable` | Struct can grow at the end; `DHEADER` prefix. |
| `@mutable` | Struct supports per-field versioning; `DHEADER` + `EMHEADER`. |
| `@optional` | Field encoded as 1-byte presence + value; XCDR2 only. |
| `@nested` | Type is only used as a member; no top-level Topic. |
| `@try_construct(USE_DEFAULT\|TRIM\|DISCARD)` | Receive-side fallback for missing / oversized members. |
| `@external` | Member stored as pointer (Rust `Box`, C++ `unique_ptr`) to break cycles. |
| `@verbatim("rust", "...")` | Inject literal target-language code (also `cpp`, `csharp`, `java`, `python`, `ts`). |
| `@autoid(SEQUENTIAL\|HASH)` | Default member-ID strategy for the struct. |
| `@bit_bound(N)` | Override default bit-width of integer / enum members. |
| `@must_understand` | Receiver MUST recognise this member, else discard the sample. |
| `@unit("m/s")`, `@min`, `@max`, `@range` | Documentation-only; preserved in `TypeObject`. |

Pragmas (legacy IDL 3.x form, accepted for compatibility):

```idl
#pragma keylist Robot::Pose robot_id
#pragma DCPSTopic com::example::Pose Robot::Pose
```

These are translated to `@key` annotations internally.

---

## 6 build.rs integration (Rust)

A typical Rust crate that consumes IDL types ships its `.idl` files
in the source tree and compiles them through a `build.rs`:

```rust
// build.rs
use std::process::Command;

fn main() {
    let idl_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("idl");
    let out_dir = std::env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed={}", idl_dir.display());
    for entry in std::fs::read_dir(&idl_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("idl") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());

        let status = Command::new("zerodds-idlc")
            .arg("generate")
            .arg(&path)
            .arg("--rust")
            .arg("--rust-no-cargo-toml")
            .arg("-o")
            .arg(&out_dir)
            .status()
            .expect("failed to run zerodds-idlc");
        assert!(status.success(),
            "zerodds-idlc failed for {}", path.display());
    }
}
```

In `lib.rs`:

```rust
include!(concat!(env!("OUT_DIR"), "/robot/mod.rs"));
```

This pattern is the same as `tonic-build` / `prost-build`. For
projects that prefer a build-script-free workflow, commit the
generated stubs to the repository — the output is deterministic.

---

## 7 Cookbook

### 7.1 Struct with optional fields

```idl
@appendable
struct Telemetry {
    @key string<32> robot_id;
    @optional double battery_v;
    @optional double cpu_load;
};
```

Compile:

```bash
zerodds-idlc generate Telemetry.idl --rust -o gen/rust
```

The Rust backend emits `Option<f64>` for `@optional` members; on the
wire the field is preceded by a 1-byte presence flag (XCDR2).

### 7.2 Valuetype with inheritance

```idl
valuetype Vehicle {
    public string id;
    public double mass_kg;
};
valuetype Truck : Vehicle {
    public double payload_kg;
};
```

The C++ backend emits virtual class hierarchies; the Java backend
uses `extends`; the Rust backend emits a flattened struct with a
type-tag enum (Rust has no inheritance — the discriminator is
preserved on the wire).

### 7.3 Sequence of typed data

```idl
@final
struct PointCloud {
    sequence<float, 65536> x;
    sequence<float, 65536> y;
    sequence<float, 65536> z;
};
```

`sequence<T, N>` is a bounded sequence — the compiler emits a
`Vec<T>` (Rust) / `std::vector<T>` (C++) / `T[]` (Java) and
validates `len <= N` on the encode path. Without the bound the
sequence is unbounded; encoders accept any length up to the
participant `max_message_size`.

### 7.4 Recursive types

```idl
struct TreeNode {
    @key long id;
    string label;
    sequence<@external TreeNode> children;
};
```

`@external` is required on members that would otherwise create a
cycle. The Rust backend emits `Box<TreeNode>` inside the sequence;
the C++ backend emits `std::unique_ptr<TreeNode>`.

### 7.5 Modules and namespaces

```idl
module com {
    module example {
        module robot {
            @final
            struct Pose { /* ... */ };
        };
    };
};
```

The compiler maps modules to:

* Rust: nested `mod` tree, `com::example::robot::Pose`.
* C++: nested `namespace`, `com::example::robot::Pose`.
* C#: nested `namespace`, `com.example.robot.Pose`.
* Java: package `com.example.robot.Pose`.
* Python: nested package, `com.example.robot.Pose`.
* TypeScript: nested namespace under `package.json::name`.

### 7.6 Includes

```idl
// Common.idl
struct Vec3 { double x; double y; double z; };

// Robot.idl
#include "Common.idl"
struct Pose { Vec3 position; double yaw; };
```

Compile:

```bash
zerodds-idlc generate Robot.idl --rust -o gen/rust -I .
```

The compiler resolves `#include` against `-I` paths in the order
given. Each included file is compiled exactly once; transitive
includes are detected and short-circuited.

### 7.7 Custom annotations

```idl
@annotation owner {
    string name;
    string email;
};

@owner(name = "Alice", email = "alice@example.com")
struct Sensor { /* ... */ };
```

User-defined annotations are preserved in the `TypeObject` but do
not affect codegen by default. Plugin emitters (`--rust-plugin
my_plugin.so`) can read them.

### 7.8 Key extraction

```idl
@final
struct PoseKey {
    @key string<32> robot_id;
    @key long sub_id;
    double x;
    double y;
};
```

Compile:

```bash
zerodds-idlc generate PoseKey.idl --rust --with-keyhash-md5 -o gen/rust
```

`KeyHash` is computed per DDSI-RTPS 2.5 §9.6.3.8: serialise key
fields in big-endian XCDR1, then MD5 if the result exceeds 16
bytes. With `--with-keyhash-md5` the MD5 is forced even for
short keys (compatible with RTI Connext default behaviour).

### 7.9 Type evolution

`@appendable` and `@mutable` types can evolve:

```idl
// v1
@mutable
struct Config {
    @id(1) string host;
    @id(2) long port;
};

// v2 — add field, keep IDs stable
@mutable
struct Config {
    @id(1) string host;
    @id(2) long port;
    @id(3) @optional string username;
};
```

Subscribers built against v1 can read v2 samples (the new field is
ignored); subscribers built against v2 can read v1 samples (the
optional field decodes as `None`). Compatibility is checked by the
`TypeObject` exchange in the discovery phase — see XTypes 1.3
§7.6.5.

### 7.10 Bitset / bitmask

```idl
bitmask FaultFlags {
    @position(0) BATTERY_LOW,
    @position(1) MOTOR_OVERHEAT,
    @position(2) ENCODER_FAILURE,
};

bitset SensorStatus {
    bitfield<3> ready;
    bitfield<5> error_code;
    bitfield<8> reserved;
};
```

Both compile to per-language bitfield types (Rust `bitflags!`,
C++ `enum class : uint64_t`, Java `EnumSet`, etc.).

### 7.11 String bounds

```idl
@final
struct Message {
    string<256> subject;       // bounded
    string body;               // unbounded
    wstring<128> author;       // 16-bit chars, bounded
};
```

Bounded strings have their bound validated on the encode path and
declared in the `TypeObject`. Unbounded strings are accepted up to
the participant `max_message_size`.

### 7.12 Enums with explicit bit-width

```idl
@bit_bound(8)
enum Severity {
    INFO,
    WARNING,
    ERROR,
    CRITICAL,
};
```

`@bit_bound(8)` makes the enum encode as a single byte instead of
the default 32-bit. Useful for protocol structs where every byte
counts (sensor packets, telemetry).

---

## 8 Output layout summary

| Backend | Top-level files | Per-type files |
|---|---|---|
| `--rust` | `Cargo.toml`, `src/lib.rs` | `<module>/<type>.rs`, `<type>_keyhash.rs`, `<type>_xcdr.rs` |
| `--c` | `<module>.h`, `<module>.c` | inline in `<module>.h` |
| `--cpp` | `<module>.hpp`, `<module>.cpp`, `<module>PubSubTypes.hpp/.cpp` | inline in `<module>.hpp` |
| `--csharp` | `<module>.csproj` | `<module>/<type>.cs`, `<type>TypeSupport.cs` |
| `--java` | `module-info.java` | `<package>/<type>.java`, `<type>TypeSupport.java` |
| `--python` | `<package>/__init__.py`, `<package>/_xcdr.py` | `<package>/<type>.py` |
| `--ts` | `package.json`, `tsconfig.json`, `src/index.ts` | `src/<module>/<type>.ts` |

All paths are relative to `-o <path>`.

---

## 9 Troubleshooting

### 9.1 `parse failed: unexpected token '@' at line N`

The grammar mode does not recognise XTypes annotations. Pass
`--strict` or remove the annotation, or upgrade the compiler — XTypes
1.3 annotations are supported since `1.0.0-rc.1`.

### 9.2 `parse failed: unknown identifier 'string<32>' at line N`

You probably wrote `string <32>` (with a space). The bound form is
`string<N>` with no space, or `string` for unbounded.

### 9.3 `validation: @key on @optional field is not allowed`

XTypes 1.3 §7.2.2.4.4: key members cannot be `@optional`. Either
drop `@optional` or move the key to a separate non-optional field.

### 9.4 `output IO error: cannot create gen/rust/Cargo.toml`

Either the output directory is read-only, or the parent is missing.
The compiler does not create grandparent directories automatically;
run `mkdir -p gen` first or use `--rust-no-cargo-toml` to skip it.

### 9.5 `KeyHash mismatch with peer vendor`

The most common cause is endianness. ZeroDDS computes the KeyHash in
big-endian XCDR1 per DDSI-RTPS 2.5 §9.6.3.8. Some legacy vendor
builds default to little-endian — pass `--with-keyhash-md5` and
verify with `zerodds-idlc dump-typeobject` that the `equivalence_hash`
matches.

### 9.6 `linker error: undefined reference to '<TypeName>::encode'`

The C / C++ backend split type-defs and CDR functions across `.hpp`
and `.cpp`. Make sure your build links the `.cpp` file. CMake users
should `target_sources(<target> PRIVATE Robot.cpp)`.

### 9.7 `Java: NoSuchMethodError: <Type>.encodeCdr`

The `*TypeSupport.java` file was not regenerated after an IDL
change. Re-run `mvn generate-sources` or delete
`target/generated-sources/java` and rebuild.

### 9.8 `Python: ModuleNotFoundError: No module named 'robot'`

Either `gen/py` is not on `PYTHONPATH` or the package layout is
flat where a nested layout was expected. Use
`--python-package-style nested` to match the IDL `module` tree, or
add `gen/py` to `sys.path`.

### 9.9 `TypeScript: Cannot find type 'CdrWriter'`

`@zerodds/ts-core` is not installed. The generated `package.json`
lists it as a dependency:

```bash
cd gen/ts
npm install
```

### 9.10 `cargo build` fails with `error: cannot find macro 'cdr_encode!'`

You are missing the `zerodds-cdr-derive` dependency. Either pass
`--rust-no-cargo-toml` and manage `Cargo.toml` yourself, or let the
compiler emit it (the default).

### 9.11 `--all` produces an empty output directory

`--all` requires `-o` to point to an existing directory. Run
`mkdir -p gen` first. The compiler does not auto-create the root.

### 9.12 Wire bytes differ from a reference vendor

Use `dump-typeobject` to confirm the type matches and `dump-ast` to
confirm the IDL parsed identically. If both agree but the wire
bytes still differ, the most likely culprit is XCDR1 versus XCDR2 —
the encoding version is selected at runtime by the
`representation_identifier` byte (see
[`cdr-wire-format.md`](cdr-wire-format.md)). Pass `--xcdr1` to force
XCDR1 emission for legacy interop.

---

## 10 Cross-reference

* [`language.md`](language.md) — IDL syntax and semantics.
* [`annotations.md`](annotations.md) — annotation reference.
* [`codegen.md`](codegen.md) — conceptual codegen flow.
* [`cdr-wire-format.md`](cdr-wire-format.md) — on-the-wire byte form.
* [`../05-integration/README.md`](../05-integration/README.md) — wiring the generated stubs into a participant.
* OMG IDL 4.2 — `https://www.omg.org/spec/IDL/4.2/`.
* OMG XTypes 1.3 — `https://www.omg.org/spec/DDS-XTypes/1.3/`.
* OMG DDS-TS 1.0 — TypeScript PSM, see `documentation/specs/dds-ts-1.0/`.
