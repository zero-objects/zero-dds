# `zerodds-idl-ts`

IDL4 → **TypeScript codegen** for ZeroDDS, conforming to the vendor-
specific DDS-TS 1.0 mapping (`documentation/specs/dds-ts-1.0/`).
Target platforms: Node.js (via `@zerodds/node` and koffi FFI) and
the browser (via the `@zerodds/wasm` codec).

Part of the [**ZeroDDS**](../../README.md) project. Safety class
**STANDARD** — `forbid(unsafe_code)`, deterministic codegen.

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

In the CLI, `zerodds-idlc --ts -o <dir> <file.idl>` handles the codegen.
Output lands as `<basename>.ts` and imports the runtime from
`@zerodds/types`.

## Construct mapping (DDS-TS 1.0 §7)

| IDL | TypeScript |
| --- | --- |
| `boolean` | `boolean` |
| `char` / `wchar` | `Char` / `WChar` (branded, `runtime/branded.ts`) |
| `octet` | `number` (0..255) |
| `short`..`int32` | `number` |
| `long long`..`int64` | `bigint` |
| `float` / `double` | `number` |
| `long double` | `LongDouble` (16-byte opaque carrier) |
| `string` / `wstring` | `string` (+ `_BOUND` const when bounded) |
| `any` | `DdsAny` (boxed value with typeId) |
| `sequence<T>` | `Array<T>` (+ `_BOUND` when bounded) |
| `T[N]` | `Array<T>` (+ `_LENGTH` const) |
| `map<K, V>` | `ReadonlyMap<K', V'>` |
| `struct` | `interface` + `DdsTypeDescriptor` + type guard |
| `exception` | `interface extends DdsException` |
| `enum` | as-const object + string-literal union (no `enum`) |
| `union` | discriminated union + descriptor (literal narrowing) |
| `typedef T name` | `export type name = T` + descriptor |
| `bitset` | `interface` (number/bigint per width) + `_BITS` consts |
| `bitmask` | as-const shifts (number/bigint per `@bit_bound`) |
| `interface (Ops)` | `{Iface}Client` + `{Iface}Handler` + ServiceDescriptor |

What is generated is **structural typing without class promotion** —
JSON-friendly, no constructor ceremony, each type descriptor lives
as a side table with discriminant/bounds info.

## Spec mapping

| Spec document | Section |
| --- | --- |
| OMG IDL 4.2 (ISO/IEC 19516) | §7 — construct mapping |
| ZeroDDS DDS-TS 1.0 (vendor spec) | §7 — mapping tables |
| OMG DDS-XTypes 1.3 | §7.2.3 — annotations + extensibility |

## Runtime library

Generated code imports `@zerodds/types`, which provides per-type descriptors:

```typescript
import { DdsTypeDescriptor, encode, decode } from '@zerodds/types';
import { Greeting, GreetingDescriptor } from './gen/chat';

const bytes = encode(GreetingDescriptor, { id: 42, text: 'hi' });
const back = decode(GreetingDescriptor, bytes);
```

Browser code uses the same codegen and links only `@zerodds/wasm`
for the wire codec — live pub/sub in the browser is not possible
(WASM cannot do UDP multicast), but roundtrip validation is.

## Deliberately NOT in the crate

- **Runtime implementation** (`@zerodds/types`) — a separate npm bundle,
  not Rust code.
- **Linker tests against Node.js / browser** — these live in the respective
  binding crates (`ts-node`, `ts-wasm`).

## Features

* `default = []` — std-only.

## Stability

`1.0.0-rc.2` — wire-byte-identical to RTI Connext / Cyclone DDS /
Fast-DDS. The generated TS code is API-stable; the internal Rust API
(options fields, error variants) may still break before 1.0.0-final.

## Tests

```bash
cargo test -p zerodds-idl-ts
```

Cross-vendor wire-vector tests against JSON fixtures under `tests/`.

## See also

- [`zerodds-idl`](../idl/README.md) — parser + AST.
- [`zerodds-ts-node`](../ts-node/README.md) — Node.js runtime (koffi FFI).
- [`zerodds-ts-wasm`](../ts-wasm/README.md) — browser WASM codec.
- [`zerodds-web`](../web/README.md) — OMG DDS-WEB 1.0 REST PSM.
- [`zerodds-idlc`](../../tools/idlc/README.md) — CLI with the `--ts` flag.
- [`packaging/docker/ts-node-runtime/`](../../packaging/docker/ts-node-runtime/) — sandbox image.
