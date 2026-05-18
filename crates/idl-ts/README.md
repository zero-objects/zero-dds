# `zerodds-idl-ts`

IDL4 → **TypeScript-Codegen** fuer ZeroDDS, konform mit der vendor-
spezifischen DDS-TS 1.0 Mapping (`documentation/specs/dds-ts-1.0/`).
Ziel-Plattformen: Node.js (via `@zerodds/node` und koffi-FFI) und
Browser (via `@zerodds/wasm` Codec).

Teil des Projekts [**ZeroDDS**](../../README.md). Safety-Klasse
**STANDARD** — `forbid(unsafe_code)`, deterministischer Codegen.

---

## Quick Start

```rust
use zerodds_idl::config::ParserConfig;
use zerodds_idl_ts::generate_ts_source;

let ast = zerodds_idl::parse(
    "struct Greeting { @key long id; string<128> text; };",
    &ParserConfig::default(),
)?;

let ts_src = generate_ts_source(&ast)?;
assert!(ts_src.contains("Greeting"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Im CLI uebernimmt `zerodds-idlc --ts -o <dir> <file.idl>` den Codegen.
Output landet als `<basename>.ts` und importiert die Runtime aus
`@zerodds/types`.

## Konstrukt-Mapping (DDS-TS 1.0 §7)

| IDL | TypeScript |
| --- | --- |
| `boolean` | `boolean` |
| `char` / `wchar` | `Char` / `WChar` (branded, `runtime/branded.ts`) |
| `octet` | `number` (0..255) |
| `short`..`int32` | `number` |
| `long long`..`int64` | `bigint` |
| `float` / `double` | `number` |
| `long double` | `LongDouble` (16-byte opaque carrier) |
| `string` / `wstring` | `string` (+ `_BOUND` const wenn bounded) |
| `any` | `DdsAny` (boxed value mit typeId) |
| `sequence<T>` | `Array<T>` (+ `_BOUND` wenn bounded) |
| `T[N]` | `Array<T>` (+ `_LENGTH` const) |
| `map<K, V>` | `ReadonlyMap<K', V'>` |
| `struct` | `interface` + `DdsTypeDescriptor` + Type-Guard |
| `exception` | `interface extends DdsException` |
| `enum` | as-const object + string-literal-union (kein `enum`) |
| `union` | Discriminated-Union + Descriptor (literal narrowing) |
| `typedef T name` | `export type name = T` + Descriptor |
| `bitset` | `interface` (number/bigint per width) + `_BITS`-consts |
| `bitmask` | as-const shifts (number/bigint per `@bit_bound`) |
| `interface (Ops)` | `{Iface}Client` + `{Iface}Handler` + ServiceDescriptor |

Generiert wird **strukturelle Typisierung ohne Class-Promotion** —
JSON-friendly, keine Konstruktor-Ceremony, jeder Type-Descriptor lebt
als Side-Table mit Discriminant-/Bounds-Info.

## Spec-Mapping

| Spec-Dokument | Abschnitt |
| --- | --- |
| OMG IDL 4.2 (ISO/IEC 19516) | §7 — Konstrukt-Mapping |
| ZeroDDS DDS-TS 1.0 (vendor spec) | §7 — Mapping Tables |
| OMG DDS-XTypes 1.3 | §7.2.3 — Annotations + Extensibility |

## Runtime-Library

Generierter Code importiert `@zerodds/types`, das pro-Type Descriptors
zur Verfuegung stellt:

```typescript
import { DdsTypeDescriptor, encode, decode } from '@zerodds/types';
import { Greeting, GreetingDescriptor } from './gen/chat';

const bytes = encode(GreetingDescriptor, { id: 42, text: 'hi' });
const back = decode(GreetingDescriptor, bytes);
```

Browser-Code nutzt denselben Codegen und linkt nur `@zerodds/wasm`
fuer den Wire-Codec — Live-Pub/Sub im Browser ist nicht moeglich
(WASM kann kein UDP-Multicast), aber Roundtrip-Validierung schon.

## Bewusst NICHT im Crate

- **Runtime-Implementation** (`@zerodds/types`) — separate npm-Bundle,
  nicht Rust-Code.
- **Linker-Tests gegen Node.js / Browser** — sind in den jeweiligen
  Binding-Crates (`ts-node`, `ts-wasm`).

## Features

* `default = []` — std-only.

## Stabilitaet

`1.0.0-rc.2` — Wire-byte-identisch zu RTI Connext / Cyclone DDS /
Fast-DDS. Generierter TS-Code ist API-stabil; interne Rust-API
(Options-Felder, Error-Varianten) kann bis 1.0.0-final noch
brechen.

## Tests

```bash
cargo test -p zerodds-idl-ts
```

Cross-Vendor Wire-Vector-Tests gegen JSON-Fixtures unter `tests/`.

## See also

- [`zerodds-idl`](../idl/README.md) — Parser + AST.
- [`zerodds-ts-node`](../ts-node/README.md) — Node.js-Runtime (koffi-FFI).
- [`zerodds-ts-wasm`](../ts-wasm/README.md) — Browser-WASM-Codec.
- [`zerodds-web`](../web/README.md) — OMG DDS-WEB 1.0 REST-PSM.
- [`zerodds-idlc`](../../tools/idlc/README.md) — CLI mit `--ts` Flag.
- [`packaging/docker/ts-node-runtime/`](../../packaging/docker/ts-node-runtime/) — Sandbox-Image.
