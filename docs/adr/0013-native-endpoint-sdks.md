# 0013 — Native endpoint SDKs (C/C++/Python/Java) over a Rust hub, no FFI

- **Status:** accepted
- **Date:** 2026-07-21
- **Authors:** @sandra
- **Context:** `crates/xrce`, `crates/zerodds-c-api`, `crates/py`, `crates/java-omgdds`, `crates/idl-cpp`/`idl-java`/`idl-python`, `crates/cdr`, DDS-XRCE 1.0, DDSI-RTPS 2.5 §10.5

## Context

ZeroDDS connects native languages via FFI over the Rust core: `zerodds-c-api`
exposes a C ABI, `crates/py` (PyO3) and the Node/Java bindings sit on top of it as
wrappers. This approach requires a native library per OS × architecture and
a Rust toolchain for building.

Part of the target platforms for endpoints has no Rust toolchain and is
often big-endian, resource-constrained or old (68k class, i686 Linux, PA-RISC,
embedded terminals and control computers). These platforms are endpoints,
not central DDS instances; the full DDS runs on x86-64 Linux
(in future ARM64) with a POSIX environment.

## Decision

**Two-role topology:**

```
Hub (Rust, x86-64 Debian / ARM64): full modern DDS
  — Discovery, Routing, Security, RTPS fabric, endpoint agent (crates/xrce)
        ▲  Endpoint↔Hub protocol (XRCE-shaped), transport-opaque
        │
Endpoints (native, NO Rust): C · C++ · Python · Java
  — minimal: wire + Endpoint↔Hub framing + frame hook + composable modules
```

The **hub stays Rust** and carries the full stack. The **endpoints become
native, conservative SDKs** per language — self-contained, **no Rust underneath**,
**no hyper-modern features/libs**, so that they compile with the platform's own
toolchain.

**Crate/directory split** (endpoint SDKs, generated + conservative):
`endpoints/c`, `endpoints/cpp`, `endpoints/python`, `endpoints/java`; the
wire parts are generated from the existing IDL codegen (`idl-cpp`/`idl-java`/
`idl-python` + new C89 emitter); `crates/xrce` (Rust) is agent +
byte-identity oracle.

## Invariants (normative)

1. **No Rust on the endpoint.** Conservative language floors: **C89/C90**,
   **C++98/03**, **Python pure stdlib** (3.x conservative, 2.7-friendly style),
   **Java 8** (pure Java, no JNI native lib, no records/var/streams). No
   third-party libs, no forced malloc in C (static/arena buffers).

2. **Combinable units span the feature levels.** One unit graph, mirrored per language
   as link libs / jars / py imports: `wire-core` (framing +
   frame hook + XCDR primitives) · `wire-fixed` · `wire-variable` ·
   `feat-reliable` · `feat-security` · … An endpoint links exactly the subset
   that its footprint can carry.

3. **Two wire backends.** **`wire-fixed`** = static, **generated** per type
   (straight-line XCDR, no runtime type info) → tiny, deterministic,
   certification-friendly, known types/fixed topology. **`wire-variable`** =
   reflective/dynamic (DynamicData↔XCDR + TypeObject/DynamicType) → evolving/
   unknown types, monitor/spy, RE from capture. The Rust core already has both
   (typed `Xcdr2Writer` + reflective codec); the endpoints mirror that.

4. **Endianness first-class, not retrofitted.** Endpoint HW is big-endian
   (68k/PA-RISC/SPARC), hub little-endian (x86-64) → **BE↔LE is the normal case**.
   Serialization is **byte by byte with explicit shifts** (never struct/
   pointer casts onto bytes) → independent of host endianness *by construction*. The
   **wire byte order is an explicit parameter** (LE/BE encapsulation flag,
   RTPS 2.5 §10.5); both backends decode **both** orders and emit
   the chosen one. Tested against **BE and LE goldens**.

5. **The frame hook is the only mandatory integration point.** Contract at the
   boundary of the fully framed message (`deliver(frame)` / `receive() → frame`),
   transport-opaque — the user attaches the transport of their target behind it.
   Prefilled for POSIX sockets and serial transports; for other targets empty
   plus an example.

6. **One wire truth.** Everything wire-critical is generated from *one* wire model
   (the same source as Rust). Every endpoint SDK is attached as "another
   vendor" in the cross-vendor byte-identity harness → byte-identical to the
   Rust reference, cannot drift. The full feature scope + hard protocol
   logic (discovery/reliability state machines/security handshake/routing) stays
   **hub-only**.

## Alternatives

1. **Keep FFI bindings over the Rust core** (status quo) — rejected: the
   friction above; does not build on no-Rust/restricted/old toolchains, native lib
   shipping, ABI drift, callback boundary.
2. **One portable C core with language bindings on top** — rejected:
   the languages are meant to be *native* (pure Java = JVM reach without a native lib;
   pure Python without a C extension), not another FFI jump.
3. **Full native DDS stack per endpoint** (discovery/RTPS fabric in C/…) —
   rejected: endpoints are not central instances; the hub does the DDS.
   A full stack ×4 multiplies the spec completeness surface unnecessarily.
4. **LE first, BE later** — rejected: the endpoint HW is BE; BE is the
   normal case, not the afterthought.

## Consequences

**Positive:** endpoints compile with the platform's own toolchain, nothing
native to ship, no ABI drift, native callbacks (solves O14 without FFI risk),
JVM/CPython reach on exotic platforms. The full stack stays Rust-hub-only,
endpoints generated + locked by the harness. Legacy integration becomes first-class.

**Negative/risks:** four native endpoint runtimes (albeit minimal +
generated). `wire-variable` in **C89** is the cost center (hand-written type descriptor
model + XCDR interpreter, arena allocation) — in Python/Java, by contrast, easy.
Conservative floors cost convenience (no modern language features).

## Follow-up tasks

- **P1 — `wire-core` (C89) ✅ (in progress, core in place).** `endpoints/c`: XCDR2
  primitives (u8..u64, f32/f64 via bit pattern, string, sequence<octet>), endian-
  safe (byte by byte, LE/BE parameter), XCDR2 alignment. `endpoints/golden-gen`
  (Rust/zerodds-cdr) supplies LE+LE goldens; a sample `wire-fixed` codec is
  byte-identical. **Verified:** native x86-64 **and PowerPC big-endian
  (qemu)** byte-identical (LE+BE), `-Werror`-clean gcc+clang, C89-pedantic — the
  endianness invariant proven against a real BE machine. **DHEADER ✅ (C):**
  @appendable + nested + sequence<non-primitive> (delimited CDR2, back-patch)
  byte-identical native x86-64 **and PowerPC-BE (qemu)** against
  `encode_appendable`/collection DHEADER of the Rust core. Open in P1:
  `wire-fixed` codegen (instead of a hand-written sample), `feat-reliable`, @mutable/EMHEADER,
  DHEADER mirror in Python/Java (C++ inherits via the facade).
- **`wire-fixed` codegen ✅ (C89 core).** `endpoints/codegen`
  (`zerodds-endpoint-codegen`) parses IDL via `zerodds_idl` and emits the
  C89 codec per `@final` struct over the zdw primitives. Generated from `sensor.idl`:
  wire calls identical to the hand-written sample, output byte-identical to Rust (golden
  LE+BE), native x86-64 **and PowerPC-BE**, `-Werror`. Open: @appendable/@mutable emit
  (DHEADER/EMHEADER — primitives proven), nested, C++/Python/Java emitters.
- **P2 — `wire-variable` ✅ (C).** `zerodds_reflect`: reflective XCDR codec over
  a runtime `zdw_dyn_field[]` descriptor instead of generated code (for
  evolving/unknown types, monitor/spy, characterization of foreign wire formats).
  Reflectively encoded = fixed = Rust golden, byte-identical LE+BE + reflective
  decode, native + PowerPC-BE. Both backends produce identical bytes.
- **P3 — C++/Python/Java, re-targeting the same goldens ✅ (wire-core, core part).**
  All four languages byte-identical to the same Rust goldens (LE+BE) +
  round trip, verified: **C89** (`endpoints/c`) + **C++98**
  (`endpoints/cpp`, facade over the C core) native x86-64 **and PowerPC-BE
  under qemu**, `-Werror` gcc+g++; **Python** (`endpoints/python`, pure stdlib
  2.7/3.x); **Java 8** (`endpoints/java`, pure, no JNI). **Extensibility triad
  ✅ in all four** — `@final` + `@appendable` (DHEADER/nested/sequence<non-primitive>)
  + `@mutable` (EMHEADER LC4), all byte-identical, native + PowerPC-BE
  (C/C++), Python/Java. Open per language: `wire-fixed` codegen (instead of
  hand-written samples), `feat-reliable`, `wire-variable`.
- **P4** Endpoint↔Hub protocol (XRCE-shaped) + agent-side terminator at the hub;
  frame hook contract + POSIX/serial example fillings. **Frame hook ✅ (C):**
  `zerodds_endpoint.h` `zdw_transport` (deliver/receive, transport-opaque) +
  loopback demo, native + PowerPC-BE. **XRCE WRITE_DATA framing ✅ (C):**
  `zdw_xrce_write_frame`/`read_frame` — MessageHeader + WRITE_DATA submessage,
  byte-identical to a real `crates/xrce` message (48 B), full path
  encode→frame→hook→unwrap→decode. **Serial framing ✅ (C):**
  `zdw_serial_frame`/`deframe` — Annex C.1 (7E + addresses + length LE + stuff +
  FCS-16 per RFC 1662 LE), byte-identical to the `crates/xrce` serial framer,
  deframe+CRC+unwrap+decode.
  Everything native + PowerPC-BE, `-Werror`. **Reliable ✅** (HEARTBEAT parse +
  ACKNACK build byte-identical), **DATA receive path ✅** (bidirectional),
  **all four languages hub-capable ✅** (Python/Java pure, C++ via C reuse).
  **Live UDP E2E ✅:** C endpoint → real UDP → running `zerodds-xrce` agent
  (`endpoints/xrce-agent-demo`), which parses the message + reads the SensorReading
  back (live UDP acceptance).
  **Multi-language codegen ✅:** `wire-fixed` emitter for **C89 + Python + Java**
  (C++ uses the C codec via `extern "C"`), all byte-identical.
  **Reflective codec ✅** in **C + Python + Java** (C++ via C reuse).

## Status — complete

All four languages hub-capable; both wire backends in several languages **including the
extensibility triad + nested + sequence<struct>**:
- `wire-fixed` codegen: C89 / Python / Java (all 3 modes; C89 additionally nested);
  C++ uses the C codec.
- `wire-variable` reflective: C / Python / Java (final/appendable/mutable + nested +
  sequence<struct>); C++ via C reuse.
- XRCE (write/data/heartbeat/acknack) + serial framing, bidirectional + reliable.

Everything byte-identical to the Rust truth, verified native x86-64 + PowerPC-BE
(qemu, full suite `-Werror`) + live UDP E2E against a real `zerodds-xrce`
agent. No open items.
- **P5** Extend the byte-identity harness with the four SDKs as "vendors".

## State in the code

The decision above holds; paths and scope have changed since then:

- `endpoints/` contains 22 endpoint languages: `ada`, `ada-83`, `ada-native`,
  `c`, `cpp`, `csharp`, `d`, `elixir`, `fsharp`, `go`, `java`, `julia`,
  `kotlin`, `lua`, `nim`, `node`, `ocaml`, `python`, `rust`, `swift`,
  `typescript`, `zig`; besides these `codegen`, `golden-gen`, `testdata`,
  `examples`, `xrce-agent-demo`.
- `crates/idl-cpp`, `crates/idl-python` and the crate `zerodds_idl` no longer
  exist; `crates/cdr` and `crates/idl-java` are not Cargo packages.
  The IDL parser is `crates/idl-parse` with `crates/idl-lower`, the
  language codegen is `crates/lang-rules` (invocation: `zerodds-idlc generate --lang`).
  `zerodds-endpoint-codegen` (`endpoints/codegen`) reads IDL via
  `zerodds-idl-parse` and `zerodds-idl-lower`.
- The codec that generated code calls is `crates/wire`; DynamicType and
  DynamicData live in `crates/types`.

## References

- DDS-XRCE 1.0 (client/agent split); DDSI-RTPS 2.5 §10.5 (encapsulation +
  byte order flag)
- `crates/xrce` (Rust reference + agent), `crates/wire` (XCDR codec), `crates/types`
  (DynamicType/DynamicData)
- ADR 0009 (hub-side service architecture, adapter-driven)
