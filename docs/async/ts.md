<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — TypeScript (binding async surface)

The TypeScript binding (`ts-node`) exposes a **Promise / async-iterator** async
surface in [`dds.ts`](../../crates/ts-node/src/dds.ts): `writeAsync`,
`waitForMatchedSubscription`/`waitForMatchedPublication`, `waitForData`,
`takeAsync`, and an `async *streamSamples()` async iterator.

## Surface

```typescript
await writer.writeAsync(sample);
await writer.waitForMatchedSubscription(1, 5000);

if (await reader.waitForData(3000)) {
  for (const s of await reader.takeAsync()) handle(s);
}

// or as an async iterator
for await (const s of reader.streamSamples()) handle(s);
```

The wait helpers cooperatively poll (never blocking the event loop) and
`streamSamples` drains via `waitForData`/`takeAsync` so no sample is lost.
Covered by the `ts-node` test suite (`test/`).
