# `zerodds-idlc` Handbook

End-user reference for the ZeroDDS IDL compiler. This document is the
primary operator manual: install, invoke, integrate, and troubleshoot.

This handbook covers the CLI, the full annotation set, and build
integration end to end; for the resulting on-the-wire byte form see
[`cdr-wire-format.md`](cdr-wire-format.md).

---

## 1 What is `zerodds-idlc`?

`zerodds-idlc` is the OMG IDL 4.2 compiler that turns `.idl` schema
files into language stubs across 17 codegen backends (Rust, C, C++,
C#, Java, Python, TypeScript, Go, Ada, Zig, Nim, D, Elixir, OCaml,
Julia, Lua, Swift) plus the matching XCDR1 / XCDR2 encoder / decoder
and `KeyHash` implementation. It is the single source of truth for
cross-language type compatibility: a Rust publisher and a Java
subscriber see the same wire bytes for the same schema because both
compile the same `.idl` through the same compiler.

Internally `zerodds-idlc` is a thin CLI on top of the `zerodds-idl`
parser crate (Earley engine + grammar deltas) and the per-language
emitter crates (`zerodds-idl-rust`, `zerodds-idl-cpp`,
`zerodds-idl-csharp`, `zerodds-idl-java`, `zerodds-idl-ts`,
`zerodds-idl-python`, `zerodds-idl-go`, `zerodds-idl-ada`,
`zerodds-idl-zig`, `zerodds-idl-nim`, `zerodds-idl-d`,
`zerodds-idl-elixir`, `zerodds-idl-ocaml`, `zerodds-idl-julia`,
`zerodds-idl-lua`, `zerodds-idl-swift`).

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
zerodds-idlc <COMMAND> <input.idl> [OPTIONS]
zerodds-idlc <input.idl> [BACKEND-FLAGS]          # flat form
```

`zerodds-idlc` accepts a sub-command form (`generate`, `check`,
`dump-ast`, `dump-typeobject`, `print-deps`) and a flat form where
backend flags are passed directly. The sub-command, if present, is the
first positional argument; global flags may precede it. The flat form
is equivalent to `generate`.

One `.idl` input file per invocation — `#include`d files are expanded
in-place by the built-in preprocessor.

### 3.2 Global flags

| Flag | Description |
|---|---|
| `-h`, `--help` | Print help and exit |
| `-V`, `--version` | Print version and exit |
| `-v`, `--verbose` | Increase log verbosity (repeat: `-vv`, `-vvv`) |
| `-q`, `--quiet` | Suppress non-error output |
| `--color <when>` | `auto` (default), `always`, `never` |
| `-I <dir>` | Add `<dir>` to the `#include` search path (repeatable) |
| `-D <name>[=<value>]` | Define a preprocessor macro (repeatable) |

### 3.3 Sub-command: `generate`

Compile an `.idl` file into language stubs.

```
zerodds-idlc generate <input.idl> <BACKEND...> [OPTIONS]
```

Backend selectors — repeatable, combine to emit several at once:

| Flag | Output |
|---|---|
| `--c` | C99 header (`<base>.h`) |
| `--cpp` | C++17 header (`<base>.hpp`) |
| `--rust` | Rust module (`<base>.rs`) |
| `--csharp` | C# source (`<base>.cs`) |
| `--java` | Java files (`<pkg>/<Type>.java`) |
| `--python` | Python module (`<base>.py`) |
| `--ts` | TypeScript module per DDS-TS 1.0 (`<base>.ts`) |
| `--go` | Go module (`<base>.go`) |
| `--ada` | Ada package spec/body (`<base>.ads`/`.adb`) |
| `--zig` | Zig module (`<base>.zig`) |
| `--nim` | Nim module (`<base>.nim`) |
| `--d` | D module (`<base>.d`) |
| `--elixir` | Elixir module (`<base>.ex`) |
| `--ocaml` | OCaml module (`<base>.ml`) |
| `--julia` | Julia module (`<base>.jl`) |
| `--lua` | Lua module (`<base>.lua`) |
| `--swift` | Swift module (`<base>.swift`) |
| `--all` | All 17 backends in one invocation |

Options:

| Flag | Description |
|---|---|
| `-o`, `--output <dir>` | Output directory |
| `--out-<lang> <dir>` | Per-backend output override (`<lang>` = `rust`/`c`/`cpp`/`csharp`/`java`/`python`/`ts`/`go`/`ada`/`zig`/`nim`/`d`/`elixir`/`ocaml`/`julia`/`lua`/`swift`); overrides `-o` for that backend |
| `--corba` | Additionally emit CORBA service code (with `--cpp`/`--csharp`/`--java`/`--rust`) |
| `--rti` | Accept the RTI Connext grammar delta while parsing |
| `--opendds`, `--cyclone` | Vendor-intent markers (vendor `#pragma`s are honoured regardless) |
| `--default-extensibility <kind>`, `-de` | Extensibility for types without `@final`/`@appendable`/`@mutable`: `final`, `appendable`, `mutable` |
| `--default-nested <true\|false>` | Mark types without `@nested`/`@topic` as `@nested` |
| `--no-typeobject` | Suppress XTypes TypeObject emission (default: on) |
| `--with-typeobject` | Force TypeObject emission on (this is the default) |
| `--scaffold` | Also emit the per-backend build file — see §6 |

The encoding version (XCDR1 vs XCDR2) is **not** a code-generation
choice — like RTI Connext, Fast DDS and Cyclone DDS, the generated code
supports the extensible-types representations and the wire version is
negotiated at runtime from each type's extensibility and the data-
representation QoS.

### 3.4 Sub-command: `check`

Parse and validate `.idl` without emitting code. Exit 0 on success,
non-zero on a parse error. Accepts `--rti`, `--opendds`, `--cyclone`.

```
zerodds-idlc check <input.idl> [--rti]
```

### 3.5 Sub-command: `dump-ast`

Print the parsed AST to stdout. Equivalent to the flat `--parse-only`
flag.

```
zerodds-idlc dump-ast <input.idl>
```

### 3.6 Sub-command: `dump-typeobject`

Lower every named type to its XTypes 1.3 Minimal `TypeObject` and print
them in topological order (dependencies first). Cross-type references
are resolved to their equivalence hash.

```
zerodds-idlc dump-typeobject <input.idl>
```

### 3.7 Sub-command: `print-deps`

Print a Make-style dependency line — the input plus every `#include`d
file. For build-system glue.

```
zerodds-idlc print-deps <input.idl>     # → "input.o: input.idl include1.idl ..."
```

### 3.8 Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Parse error (lex / syntax / build) |
| `2` | CLI usage error (bad flag, missing input, IO error) |
| `3` | Backend not supported on this build, or a codegen error |

---

## 4 Per-language backends

Each backend emits a single source file per input IDL containing all
the types it defines — `zerodds-idlc` is a single-file generator, not a
project scaffolder. The Java backend is the exception: it emits one
`.java` file per class under the package directory tree. Pass
`--scaffold` (§6) to additionally emit the matching build file.

When TypeObject emission is on (the default), each backend file ends
with the XCDR2-serialized XTypes Minimal `TypeObject` of every named
type — see §4.8.

### 4.1 Rust

```bash
zerodds-idlc generate Robot.idl --rust -o gen/rust
```

Emits `gen/rust/Robot.rs` — one module containing every type, its
`zerodds_cdr::CdrEncode` / `CdrDecode` impls, and (with `--corba`) a
second file `Robot_corba.rs` with the CORBA service code.

Consume it as a module (`mod robot;` over `Robot.rs`) or, with
`--scaffold`, as a crate via the emitted `Cargo.toml`.

### 4.2 C99

```bash
zerodds-idlc generate Robot.idl --c -o gen/c
```

Emits `gen/c/Robot.h` — a self-contained C99 header with the struct
definitions and inline encode/decode declarations. `--scaffold` adds a
`CMakeLists.txt`.

### 4.3 C++17

```bash
zerodds-idlc generate Robot.idl --cpp -o gen/cpp
```

Emits `gen/cpp/Robot.hpp` — a header-only C++17 unit (namespaces,
classes, serialisation). With `--corba` it additionally contains the
CORBA Annex-A.1 trait specialisations. `--scaffold` adds a
`CMakeLists.txt`.

### 4.4 C#

```bash
zerodds-idlc generate Robot.idl --csharp -o gen/cs
```

Emits `gen/cs/Robot.cs`. With `--corba` it includes the CORBA traits.
`--scaffold` adds a `Robot.csproj` (SDK-style).

### 4.5 Java 17

```bash
zerodds-idlc generate Robot.idl --java -o gen/java
```

Emits one `.java` file per class under the package directory derived
from the IDL `module` tree (e.g. `gen/java/com/example/robot/Pose.java`).
With TypeObject emission on, a `TypeObjects.java` is written alongside.
`--scaffold` adds a `pom.xml`.

Maven integration via `exec-maven-plugin` in the `generate-sources`
phase:

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

Emits `gen/py/Robot.py` — a `@dataclass`-based module with
`encode_cdr()` / `decode_cdr()` helpers. `--scaffold` adds a
`pyproject.toml`.

### 4.7 TypeScript (DDS-TS 1.0)

```bash
zerodds-idlc generate Robot.idl --ts -o gen/ts
```

Emits `gen/ts/Robot.ts` per the DDS-TS 1.0 PSM. `--scaffold` adds a
`package.json` + `tsconfig.json`.

npm integration:

```json
{
  "scripts": {
    "build:idl": "zerodds-idlc generate src/idl/Robot.ts --ts -o src/gen"
  }
}
```

### 4.8 TypeObject block

With TypeObject emission on (the default; `--no-typeobject` to suppress),
every backend file ends with the XTypes 1.3 Minimal `TypeObject` of
each named type, XCDR2-LE serialized as a byte constant — a Rust
`pub mod type_objects`, a C/C++ `static`/`constexpr` array, a C#/Java
`TypeObjects` class, a Python `bytes([...])`, a TS `Uint8Array`. This
mirrors the default-on TypeObject generation of RTI Connext, Fast DDS
and Cyclone DDS and feeds XTypes discovery / TypeLookup.

---

## 5 Annotations and pragmas

The compiler honours the following annotations from XTypes 1.3 and
DDS 1.4:

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

        let out_file = std::path::Path::new(&out_dir)
            .join(path.file_stem().unwrap())
            .with_extension("rs");

        let status = Command::new("zerodds-idlc")
            .arg("generate")
            .arg(&path)
            .arg("--rust")
            .arg("-o")
            .arg(&out_dir)
            .status()
            .expect("failed to run zerodds-idlc");
        assert!(status.success(),
            "zerodds-idlc failed for {}", path.display());

        // The generated file carries file-level `#![allow(...)]` inner
        // attributes (it is meant to stand alone as its own module file).
        // `include!`, below, splices those tokens into the *including*
        // file rather than a fresh one, and an inner attribute is only
        // legal at the very start of the file it textually ends up in —
        // so strip them here and re-apply the same allows as outer
        // attributes on the `include!` call site instead (see `lib.rs`).
        let generated = std::fs::read_to_string(&out_file).unwrap();
        let stripped: String = generated
            .lines()
            .filter(|l| !l.trim_start().starts_with("#!["))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&out_file, stripped).unwrap();
    }
}
```

In `lib.rs`, include the generated module (`Robot.idl` → `Robot.rs`) with
the equivalent outer attributes re-applied (see the `build.rs` comment
above):

```rust
#[allow(
    clippy::too_many_lines,
    clippy::useless_conversion,
    unused_imports,
    non_snake_case
)]
mod robot {
    include!(concat!(env!("OUT_DIR"), "/Robot.rs"));
}
pub use robot::Robot;
```

> ▶ Runnable example: [`idlc-buildrs`](https://github.com/zero-objects/zero-dds-snippets/tree/master/idlc-buildrs)
> (both fences above, verbatim — `build.rs` runs the real `zerodds-idlc`
> CLI, `lib.rs`/`main.rs` `include!` and round-trip the generated
> `Robot::Pose`).

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
zerodds-idlc generate PoseKey.idl --rust -o gen/rust
```

`KeyHash` is computed per DDSI-RTPS 2.5 §9.6.3.8: serialise the key
fields in big-endian XCDR1, then MD5 the result if it exceeds 16
bytes, otherwise use the serialised key directly. This algorithm is
spec-mandated and not configurable — it matches RTI Connext, Fast DDS
and Cyclone DDS for cross-vendor interop.

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

| Backend | Generated source | `--scaffold` build file |
|---|---|---|
| `--rust` | `<base>.rs` (+ `<base>_corba.rs` with `--corba`) | `Cargo.toml` |
| `--c` | `<base>.h` | `CMakeLists.txt` |
| `--cpp` | `<base>.hpp` | `CMakeLists.txt` |
| `--csharp` | `<base>.cs` | `<base>.csproj` |
| `--java` | `<package>/<Type>.java` (+ `TypeObjects.java`) | `pom.xml` |
| `--python` | `<base>.py` | `pyproject.toml` |
| `--ts` | `<base>.ts` | `package.json`, `tsconfig.json` |
| `--go` | `<base>.go` | see [`idl-go.md`](../../docs/idl-go.md) |
| `--ada` | `<base>.ads`/`.adb` | see [`idl-ada.md`](../../docs/idl-ada.md) |
| `--zig` | `<base>.zig` | see [`idl-zig.md`](../../docs/idl-zig.md) |
| `--nim` | `<base>.nim` | see [`idl-nim.md`](../../docs/idl-nim.md) |
| `--d` | `<base>.d` | see [`idl-d.md`](../../docs/idl-d.md) |
| `--elixir` | `<base>.ex` | see [`idl-elixir.md`](../../docs/idl-elixir.md) |
| `--ocaml` | `<base>.ml` | see [`idl-ocaml.md`](../../docs/idl-ocaml.md) |
| `--julia` | `<base>.jl` | see [`idl-julia.md`](../../docs/idl-julia.md) |
| `--lua` | `<base>.lua` | see [`idl-lua.md`](../../docs/idl-lua.md) |
| `--swift` | `<base>.swift` | see [`idl-swift.md`](../../docs/idl-swift.md) |

`<base>` is the input file stem (`Robot.idl` → `Robot`). All paths are
relative to the backend's output directory (`--out-<lang>` if given,
otherwise `-o`). With TypeObject emission on (the default), the
serialized Minimal `TypeObject`s are appended to the generated source
(for `--java`: written as a separate `TypeObjects.java`).

---

## 9 Troubleshooting

### 9.1 `parse failed: unexpected token '@' at line N`

The grammar mode does not recognise the annotation. Remove it, or
upgrade the compiler — XTypes 1.3 annotations are supported since
`1.0.0-rc.1`. For RTI `@RTI_*` vendor annotations pass `--rti`.

### 9.2 `parse failed: unknown identifier 'string<32>' at line N`

You probably wrote `string <32>` (with a space). The bound form is
`string<N>` with no space, or `string` for unbounded.

### 9.3 `validation: @key on @optional field is not allowed`

XTypes 1.3 §7.2.2.4.4: key members cannot be `@optional`. Either
drop `@optional` or move the key to a separate non-optional field.

### 9.4 `output IO error: cannot create <dir>/...`

The output directory's parent is missing or read-only. `zerodds-idlc`
creates the output directory itself but not its grandparents — run
`mkdir -p` on the parent first.

### 9.5 `KeyHash mismatch with peer vendor`

The most common cause is endianness. ZeroDDS computes the KeyHash in
big-endian XCDR1 per DDSI-RTPS 2.5 §9.6.3.8 — the spec-mandated
algorithm. Verify with `zerodds-idlc dump-typeobject` that both sides
agree on the type structure.

### 9.6 `Java: stale generated code after an IDL change`

Re-run `mvn generate-sources`, or delete
`target/generated-sources/java` and rebuild — `zerodds-idlc` overwrites
generated source but the Maven build may cache the old classes.

### 9.7 `Python: ModuleNotFoundError`

The generated `<base>.py` is not on `PYTHONPATH`. Add its directory to
`sys.path` or `PYTHONPATH`.

### 9.8 `TypeObject emission skipped — RecursiveType(...)`

A type references itself transitively (a dependency cycle). The
TypeObject mapper does not yet emit XTypes Strongly-Connected-Component
identifiers, so the TypeObject block is skipped — the type definitions
and CDR codecs are still generated. `--no-typeobject` suppresses the
warning.

### 9.9 Wire bytes differ from a reference vendor

Use `dump-typeobject` to confirm the type structure matches and
`dump-ast` to confirm the IDL parsed identically. If both agree but the
wire bytes still differ, the cause is usually the encoding version
(XCDR1 vs XCDR2) — that is negotiated at runtime from the type's
extensibility and the data-representation QoS, not at code-generation
time (see [`cdr-wire-format.md`](cdr-wire-format.md)).

---

## 10 Cross-reference

* [`cdr-wire-format.md`](cdr-wire-format.md) — on-the-wire byte form.
* [`../05-integration/java.md`](../05-integration/java.md),
  [`typescript-wasm.md`](../05-integration/typescript-wasm.md) — wiring
  generated stubs into a participant.
* Per-crate `README.md` under `crates/idl*/` — backend internals.
* OMG IDL 4.2 — `https://www.omg.org/spec/IDL/4.2/`.
* OMG XTypes 1.3 — `https://www.omg.org/spec/DDS-XTypes/1.3/`.
* OMG DDS-TS 1.0 — the TypeScript PSM.
