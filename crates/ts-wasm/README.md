# zerodds-ts-wasm — ZeroDDS WASM bindings

WASM bindings over `zerodds-cdr` (XCDR1/XCDR2 codec) to JS/TS via
`wasm-bindgen`, for browser and Node WASM environments.

## Scope

**In scope:**
- `CdrEncoder` — writes primitives, strings, bytes
- `CdrDecoder` — reads primitives, strings
- LE/BE endianness via 0/1 constants
- alignment for XCDR1-conform padding
- KeyHash computation (XTypes 1.3 §7.6.8)
- DDS-TS 1.0 vendor-spec-conform TS PSM (see
  `documentation/specs/dds-ts-1.0/`)

**Out of scope:**
- Live DDS pub/sub in the browser over UDP/multicast — WASM cannot do
  that. Instead: the WebSocket bridge (`crates/websocket-bridge/`)
  as a server-side gateway, plus this codec on the browser side for
  CDR encode/decode. Architecture sketch in
  [Documentation Trail Station 05 → typescript-wasm](../../documentation/05-integration/typescript-wasm.md).
- RTPS wire decode in the browser — theoretically possible, but without
  a UDP path rarely useful.

## Build

```bash
cd crates/ts-wasm
# Node target (for tests + server-side):
wasm-pack build --release --target nodejs --out-dir pkg-node
# Web target (for the browser via ES modules):
wasm-pack build --release --target web --out-dir pkg-web
```

## Smoke test

```bash
node --test crates/ts-wasm/test/smoke.test.mjs
```

Verifies the encode/decode roundtrip in LE/BE, bytes blob, version.

## Use-case examples

### CDR validation in the browser frontend

```ts
import init, { CdrEncoder } from "@zerodds/ts-wasm";
await init();
const enc = new CdrEncoder(0); // little-endian
enc.writeU32(temperature);
enc.writeString(sensorId);
const cdr = enc.finish();
ws.send(cdr); // to the WebSocket gateway
```

### Schema-conform type check without a server roundtrip

Validate form inputs against the IDL type before they go over the wire:
the encoder enforces range checks (e.g. write_u8 rejects 256),
alignment bugs become visible locally.

## Bundle size

- Node build (`pkg-node/`): WASM ~ 45 kB optimized via `wasm-opt -Oz`
- Web build (`pkg-web/`): identical WASM, only different JS glue.
