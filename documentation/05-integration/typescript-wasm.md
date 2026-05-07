# TypeScript (Browser / WASM)

A browser cannot open UDP sockets, so the WASM binding is
**CDR-codec-only** — it encodes / decodes sample bytes that you
ship to the network via WebSocket / WebTransport / fetch.

For a full WebSocket bridge see
`crates/websocket-bridge/` — it gateways DDS to a JSON-or-CDR
WebSocket protocol that the browser can speak.

## npm

```bash
npm install @zerodds/wasm
```

(Pre-release: build from `crates/ts-wasm/` via `wasm-pack`.)

## Quick start

```ts
import init, {
  XcdrEncoder, XcdrDecoder, KeyHash,
} from "@zerodds/wasm";

await init();

const enc = new XcdrEncoder();
enc.write_u32(42);
enc.write_string("hello");
const bytes: Uint8Array = enc.finish();

// ship `bytes` over WebSocket to a DDS bridge — server-side, the
// bridge speaks proper RTPS to the rest of the domain.
```

## API surface

| Function | Purpose |
|---|---|
| `XcdrEncoder` | XCDR2-LE encoder for primitives + strings + bytes |
| `XcdrDecoder` | XCDR2-LE decoder, symmetric to encoder |
| `KeyHash.compute(bytes)` | XTypes 1.3 §7.6.8 KeyHash from CDR-encoded key fields |
| `version()` | WASM-module version string |
| `Endianness.LE / BE` | Constants for cross-compat with big-endian peers |

## Bundle size

The WASM blob is ~80 KiB gzip — it ships with the codec only, no
runtime / no transport.

## Use case: WebSocket bridge

Architecture:

```
[Browser tab]
  TS app
   │ uses @zerodds/wasm to encode CDR bytes
   ▼
   WebSocket  ←→  [Server: zerodds-websocket-bridge]
                      │ decodes WS frame
                      │ injects as DDS sample
                      ▼
                   the rest of the DDS domain
                   (UDP / multicast / however it talks)
```

Subscribe-direction is symmetric.

## Generated stubs

`zerodds-idlc Robot.idl --ts -o src/gen` produces TypeScript types
that work for **both** the Node and WASM environments — same
source, different runtime. The generated `*.cdr.encode()` /
`*.cdr.decode()` use `@zerodds/wasm` under the hood when
available, falling back to a pure-TS encoder otherwise (slower but
no WASM init step).

## Without npm — `<script>` tag

```html
<script type="module">
  import init, { XcdrEncoder } from "https://unpkg.com/@zerodds/wasm@latest/zerodds_wasm.js";
  await init();
  const enc = new XcdrEncoder();
  enc.write_u32(123);
  console.log(enc.finish());
</script>
```

## Browser-side runtime in the future

Full DDS-in-the-browser via WebTransport (UDP-equivalent over
HTTP/3) is roadmap; current state is "use the codec + WebSocket
bridge".

## Reading further

- `crates/ts-wasm/README.md` — WASM build instructions, API ref.
- DDS-TS 1.0 specification — TypeScript PSM (formal vendor spec
  at `documentation/specs/dds-ts-1.0/`).
- `crates/websocket-bridge/README.md` — server-side wire-format.
