<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Node (native endpoint)

A native **pure-JavaScript** endpoint SDK (ADR 0013) for Node — a from-scratch
XCDR wire-core, no native addon, byte-identical to the Rust core and the other
SDKs. **Sync** (poll) and **async** (an async iterator, the idiomatic Node
model). No dependencies — just the Node stdlib.

Sources: [`endpoints/node`](../../endpoints/node) (`zerodds.js`) · example:
[`endpoints/node/example.js`](../../endpoints/node/example.js) (`node example.js`).

## Sync

```js
const z = require('./zerodds');
const c = new z.Client(transport);      // transport: { deliver, receive }
c.write(sampleXCDR);                     // frame as XRCE WRITE_DATA + deliver
const body = c.poll();                   // one non-blocking receive, or null
```

## Async (async iterator)

```js
const r = new z.AsyncReader(transport);
for await (const body of r.stream()) {   // decoded bodies as they arrive
  const id = new z.Reader(body, z.LITTLE).getU32();
}
r.close();
```

## Wire-core

`Writer` / `Reader` cover the XCDR primitives with alignment and LE/BE.
`Buffer.writeFloatLE` and BigInt (`putU64`) keep `f32`/`u64` byte-identical to
the Rust core.

## Tests (CI job `endpoints-node`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`node --test`)
- the runnable example (`node example.js`)

Runs on Node 18+ (built-in `node:test`, async iterators).
