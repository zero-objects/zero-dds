# TypeScript (Node.js)

The Node binding loads the C-FFI through [`koffi`][koffi] —
no native module compilation, just `npm install`.

## npm

```bash
npm install zerodds-node                  # when published
# or, until then, link to a local checkout:
npm install file:../zerodds/crates/ts-node
```

## Native runtime requirement

The Node binding uses the same `libzerodds` shared library as the
C-FFI. Install via your platform's package manager — see
[01 Getting Started → installation](../01-getting-started/installation.md).

## Hello, world

```ts
import { Runtime } from "zerodds-node";

async function main() {
  const rt = await Runtime.create({ domainId: 0 });
  try {
    const w = await rt.createWriter({
      topic: "Hello",
      type: "RawBytes",
      reliable: true,
    });

    if (!await w.waitForMatched(1, 5000)) {
      throw new Error("no subscriber");
    }

    await w.write(new TextEncoder().encode("hello from Node"));
    await w.destroy();
  } finally {
    await rt.destroy();
  }
}
main();
```

Subscriber:

```ts
const r = await rt.createReader({ topic: "Hello", type: "RawBytes", reliable: true });
while (true) {
  const buf = await r.take();
  if (buf) {
    console.log("got:", new TextDecoder().decode(buf));
  } else {
    await new Promise(s => setTimeout(s, 10));
  }
}
```

## Generated types from IDL

`zerodds-idlc Robot.idl --ts -o src/gen` produces:

```ts
// src/gen/Robot/Pose.ts
export interface Pose {
  id: string;
  x: number;
  y: number;
  z: number;
}

export const PoseCdr = {
  encode(p: Pose): Uint8Array { /* generated */ },
  decode(buf: Uint8Array): Pose { /* generated */ },
};
```

Use with the typed API:

```ts
import { PoseCdr, type Pose } from "./gen/Robot/Pose";

const w = await rt.createTypedWriter<Pose>({
  topic: "Telemetry",
  type: "Robot::Pose",
  cdr: PoseCdr,
});

await w.write({ id: "r1", x: 1, y: 2, z: 3 });
```

## QoS

```ts
import { Reliability, Durability } from "zerodds-node";

const w = await rt.createWriter({
  topic: "Telemetry",
  type: "Robot::Pose",
  qos: {
    reliability: Reliability.Reliable,
    durability: Durability.TransientLocal,
    deadlinePeriodMs: 50,
    historyKeepLast: 10,
  },
});
```

## Promise-based API

Every native call returns a Promise. Internally calls go through
`koffi`'s async dispatch, which queues onto a libuv worker — they
don't block the Node event loop.

## TypeScript types

The package ships `.d.ts` declarations. Strict-mode TypeScript
compiles cleanly.

## Reading further

- `crates/ts-node/README.md` — koffi-binding details.
- DDS-TS 1.0 specification at
  `documentation/specs/dds-ts-1.0/` — wire-mapping for
  TypeScript.
- [typescript-wasm.md](typescript-wasm.md) — for the browser
  flavour.

[koffi]: https://koffi.dev/
