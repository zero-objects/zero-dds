# 05 – Integration per Language

ZeroDDS ships first-class bindings for seven runtimes. Each
language gets a one-page guide here; deeper API reference is in
the per-crate rustdoc + the language's idiomatic docs.

| Runtime | Crate / Plugin | Page |
|---|---|---|
| Rust | `zerodds-rs` (re-export of `zerodds-dcps`) | [rust.md](rust.md) |
| C | `zerodds-c-api` (`zerodds.h`) | [c.md](c.md) |
| C++ | `zerodds-cpp` (RAII over `zerodds.h`) | [cpp.md](cpp.md) |
| C# | `zerodds-cs` (P/Invoke) | [csharp.md](csharp.md) |
| Java | `zerodds-java-omgdds` (Pure-Java DDS-Java-PSM, `org.omg.dds.*`) | [java.md](java.md) |
| Python | `zerodds-py` (`pyo3`) | [python.md](python.md) |
| TypeScript (Node) | `koffi` + `zerodds.h` | [typescript-node.md](typescript-node.md) |
| TypeScript (Browser) | `zerodds-ts-wasm` (CDR codec) | [typescript-wasm.md](typescript-wasm.md) |
| ROS-2 | `rmw-zerodds-shim` (RMW plugin) | [ros2.md](ros2.md) |

## Pick by use case

| You want to … | Pick |
|---|---|
| Write Rust application | `zerodds-rs` |
| Drop into existing C codebase | `zerodds-c-api` |
| Modern C++17 with RAII | `zerodds-cpp` |
| Unity / Mono game | `zerodds-cs` |
| Spring / JVM service | `zerodds-java-omgdds` |
| Quick prototype / data-science notebook | `zerodds-py` |
| Node.js backend | TypeScript-Node |
| Browser frontend | TypeScript-WASM (CDR codec) + WebSocket bridge |
| Replace cyclonedds in ROS-2 | `rmw-zerodds-shim` |

## Cross-cutting topics

These apply to every binding:

- IDL types are language-neutral — generate stubs once
  ([04 IDL](../04-idl/README.md)).
- QoS policies match across languages — same constants, same
  match rules.
- Discovery is automatic — a Java publisher and a Python
  subscriber on the same domain find each other without any
  language-specific configuration.

## Wire interop

Every binding speaks DDSI-RTPS 2.5 and CDR / XCDR2. A C++
publisher produces wire bytes that a Java subscriber can parse,
and vice-versa.

## Versioning

The C-FFI surface (`zerodds.h`) is the long-term-stable contract.
Language-specific bindings track it. When the C-FFI bumps to
`0.1.0`, all bindings get the same stamp.
