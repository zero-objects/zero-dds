# zerodds-ts-wasm — ZeroDDS WASM-Bindings

WASM-Bindings ueber `zerodds-cdr` (XCDR1/XCDR2-Codec) nach JS/TS via
`wasm-bindgen`, fuer Browser- und Node-WASM-Umgebungen.

## Scope

**In-Scope:**
- `CdrEncoder` — schreibt Primitives, Strings, Bytes
- `CdrDecoder` — liest Primitives, Strings
- LE/BE-Endianness ueber 0/1-Konstanten
- Alignment fuer XCDR1-konforme Padding
- KeyHash-Berechnung (XTypes 1.3 §7.6.8)
- DDS-TS 1.0 Vendor-Spec-konformer TS-PSM (siehe
  `documentation/specs/dds-ts-1.0/`)

**Out-of-Scope:**
- Live-DDS-Pub-Sub im Browser ueber UDP/Multicast — WASM kann das
  nicht. Stattdessen: WebSocket-Bridge (`crates/websocket-bridge/`)
  als Server-Side-Gateway, plus dieser Codec auf Browser-Seite zur
  CDR-Encode/Decode. Architektur-Skizze in
  [Documentation Trail Station 05 → typescript-wasm](../../documentation/05-integration/typescript-wasm.md).
- RTPS-Wire-Decode im Browser — theoretisch moeglich, aber ohne
  UDP-Pfad selten sinnvoll.

## Build

```bash
cd crates/ts-wasm
# Node-Target (fuer Tests + Server-Side):
wasm-pack build --release --target nodejs --out-dir pkg-node
# Web-Target (fuer Browser via ES-Modules):
wasm-pack build --release --target web --out-dir pkg-web
```

## Smoke-Test

```bash
node --test crates/ts-wasm/test/smoke.test.mjs
```

Verifiziert Encode/Decode-Roundtrip in LE/BE, Bytes-Blob, Version.

## Use-Case-Beispiele

### CDR-Validation im Browser-Frontend

```ts
import init, { CdrEncoder } from "@zerodds/ts-wasm";
await init();
const enc = new CdrEncoder(0); // little-endian
enc.writeU32(temperature);
enc.writeString(sensorId);
const cdr = enc.finish();
ws.send(cdr); // an WebSocket-Gateway
```

### Schema-konformer Type-Check ohne Server-Roundtrip

Form-Eingaben gegen IDL-Typ validieren bevor sie ueber's Netz gehen:
Encoder erzwingt Range-Checks (z.B. write_u8 lehnt 256 ab),
Alignment-Bugs werden lokal sichtbar.

## Bundle-Groesse

- Node-Build (`pkg-node/`): WASM ~ 45 kB optimized via `wasm-opt -Oz`
- Web-Build (`pkg-web/`): identisches WASM, nur anderer JS-Glue.
