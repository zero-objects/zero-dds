# @zerodds/wasm — ZeroDDS WASM bindings

WASM bindings over `zerodds-cdr` (XCDR1/XCDR2 codec) to JS/TS via
`wasm-bindgen`, plus a TypeScript DCPS-over-WebSocket runtime, for browser and
Node WASM environments. Published as the `@zerodds/wasm` npm package
(`dist/index.js` combines the wasm codec glue with the DCPS layer).

## Scope

**In scope:**
- `CdrEncoder` — writes primitives, strings, bytes
- `CdrDecoder` — reads primitives, strings
- LE/BE endianness via 0/1 constants
- alignment for XCDR1-conform padding
- KeyHash computation (XTypes 1.3 §7.6.8)
- DDS-TS 1.0 vendor-spec-conform TS PSM (see
  `documentation/specs/dds-ts-1.0/`)
- **DDS-TS 1.0 Annex C WASM-Bindings Profile** — a browser DCPS runtime
  (`src/dcps/`): `DomainParticipantFactory.instance()`,
  `createParticipantWebSocket(url, domain)`, and the participant / topic /
  publisher / subscriber / writer / reader entities, speaking the
  `crates/websocket-bridge` JSON protocol over a WebSocket transport. Both the
  fluent facade (`facade.ts`) and the normative flat C.2 operations
  (`operations.ts`, signature-for-signature with the spec) are exported.

**Out of scope:**
- Native UDP / multicast in the browser — WASM cannot do that; the WebSocket
  bridge (`crates/websocket-bridge/`) is the off-host transport per Annex C.4.1
  (browser transport SHALL be WebSocket / WebTransport / HTTP-3). The DCPS
  layer here is the browser-side client of that bridge.
- RTPS wire decode in the browser — theoretically possible, but without
  a UDP path rarely useful.

## Build

```bash
cd crates/ts-wasm
# 1. wasm codec (Node + Web targets):
npm run build:wasm
#    (= wasm-pack build --release --target web  --out-dir pkg-web
#       wasm-pack build --release --target nodejs --out-dir pkg-node)
# 2. TypeScript DCPS layer -> dist/:
npm install
npm run build        # tsc -p tsconfig.json
```

## Tests

```bash
npm test
# = node --test test/smoke.test.mjs           (wasm codec roundtrip)
#   node --test --import tsx test/dcps.test.ts (DCPS-over-WebSocket roundtrip,
#                                               flat C.2 ops, handle sentinel)
```

The codec smoke verifies the encode/decode roundtrip in LE/BE, bytes blob,
version. The DCPS test stands up an in-process bridge and drives a full browser
pub/sub roundtrip plus the Annex C.2 flat operations and C.5.3 handle cleanup.

## Use-case examples

### CDR validation in the browser frontend

```ts
import init, { CdrEncoder } from "@zerodds/wasm";
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
