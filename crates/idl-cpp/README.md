# `zerodds-idl-cpp`

IDL4 → **C++17 and C header codegen** for ZeroDDS (OMG IDL4-CPP
formal/2018-07-01 + DDS-PSM-CXX 1.0 + DDS-RPC C++ PSM). Produces
standalone headers that dock onto the runtime via `zerodds-c-api`.

Part of the [**ZeroDDS**](../../README.md) project. Safety class
**SAFE (std-only)** — `forbid(unsafe_code)`, a build-time tool with no
no_std use case.

---

## Quick Start — C++

```rust
use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

let ast = zerodds_idl::parse(
    "module M { struct S { long x; }; };",
    &ParserConfig::default(),
)?;

let header = generate_cpp_header(&ast, &CppGenOptions::default())?;
assert!(header.contains("namespace M"));
assert!(header.contains("class S"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Quick Start — pure C

```rust
use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CGenOptions, generate_c_header};

let ast = zerodds_idl::parse(
    "struct Greeting { long id; string<128> text; };",
    &ParserConfig::default(),
)?;

let c_header = generate_c_header(&ast, &CGenOptions::default())?;
assert!(c_header.contains("Greeting"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

On the CLI, the `zerodds-idlc` tool exposes the two paths as the
`--cpp` and `--c` backends respectively. Output is written as
`<basename>.hpp` or `<basename>.h` respectively.

## Scope

| Block | What is emitted | Spec |
| --- | --- | --- |
| C5.1-a | Header-Layout, Primitive-Mapping, struct/enum/union/typedef/sequence/array/inheritance, Exception | IDL4-CPP §7 |
| C5.1-b | Status classes (13), QoS policies (22), DCPS entity header stubs | DCPS §7, §8 |
| C5.2 | DDS-PSM-CXX header skeleton layer | DDS-PSM-CXX 1.0 |
| C6.1.D-cpp | DDS-RPC C++ PSM: service interface, Requester, Replier, RemoteException hierarchy | DDS-RPC 1.0 §10 |
| C-Mode | Pure C headers with ZeroDDS conventions | `c_mode.rs` |

## Spec-Mapping

| Spec document | Section |
| --- | --- |
| OMG IDL 4.2 (ISO/IEC 19516) | §7 — construct mapping |
| OMG IDL4-CPP 1.0 | §7 — header layout |
| OMG DDS-PSM-CXX 1.0 | §3-§5 — entity API |
| OMG DDS-RPC 1.0 | §10 — C++ PSM |
| OMG DDS-XTypes 1.3 | §7.2.3 — annotations + extensibility |

## Deliberately NOT in the crate

- **Bitset/bitmask, map, fixed, any, interface, valuetype** — Phase-2 material.
- **Linker tests** — static header generation suffices; round-trip tests run in `crates/cpp/tests/`.

## Features

* `default = []` — std-only, no feature needed.

## Stability

`1.0.0-rc.2` — wire-byte-identical to Cyclone DDS / RTI Connext /
Fast-DDS. The API may still break in detail points before 1.0.0-final
(options field order, error variants); generated headers stay
ABI-stable.

## Tests

```bash
cargo test -p zerodds-idl-cpp
```

Fixture IDLs under `tests/fixtures/`, snapshot tests per construct.

## See also

- [`zerodds-idl`](../idl/README.md) — parser + AST (input side).
- [`zerodds-cpp`](../cpp/README.md) — C++17 RAII wrapper, runtime side of the binding.
- [`zerodds-c-api`](../zerodds-c-api/README.md) — C FFI that binds the headers against the Rust runtime.
- [`zerodds-idlc`](../../tools/idlc/README.md) — CLI with `--cpp` and `--c` flags.
- [`packaging/docker/cpp-runtime/`](../../packaging/docker/cpp-runtime/) — sandbox image with toolchain + headers.
