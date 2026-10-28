# `zerodds/ts-node-runtime`

Sandbox runtime image for **TypeScript bindings under Node.js**
(`@zerodds/node` from `crates/ts-node`, via koffi FFI against
`libzerodds.so`). Target audience: coding challenges in Zero-Learn
sandboxes and quickstart demos for TypeScript DDS development.

Part of [**ZeroDDS**](../../../README.md). Unlike the daemon
images, this image delivers **no ENTRYPOINT service**, but
a ready Node environment with globally installed `@zerodds/node`
and `tsx` for running TS directly without a separate build step.

> **Note:** This image is Node.js-only. The browser WASM path
> (`zerodds-ts-wasm`) needs no sandbox container — it runs
> in the browser, not in the server image. Live pub/sub in the browser is
> not possible anyway (WASM cannot do UDP multicast).

---

## Build

From the repo root:

```bash
docker build \
  -f packaging/docker/ts-node-runtime/Dockerfile \
  -t zerodds/ts-node-runtime:rc7 \
  .
```

4-stage build (chef → rust-builder → node-builder → runtime). First
build ~5-15 min; subsequent builds use cargo-chef caching aggressively.

## Run

Interactive shell (default CMD):

```bash
docker run --rm -it zerodds/ts-node-runtime:rc7
```

Direct TS run with mounted learner code:

```bash
docker run --rm -it \
  -v "$PWD/workspace:/workspace" \
  zerodds/ts-node-runtime:rc7 \
  tsx /workspace/main.ts
```

Sandbox style (read-only root, no network to the outside):

```bash
docker run --rm \
  --read-only \
  --tmpfs /tmp:rw,size=64m \
  --network none \
  -v "$PWD/workspace:/workspace:ro" \
  zerodds/ts-node-runtime:rc7 \
  tsx /workspace/loopback_pubsub.ts
```

## What is inside

| Component | Version | Path |
| --- | --- | --- |
| Node.js | 22 (bookworm-slim) | `/usr/local/bin/node` |
| `@zerodds/node` (global) | RC build from `crates/ts-node` | `/usr/local/lib/node_modules/@zerodds/node/` |
| `tsx` (global) | 4.x | `/usr/local/bin/tsx` |
| `libzerodds.so` | RC build from `crates/zerodds-c-api` | `/usr/local/lib/libzerodds.so` |
| `zerodds-idlc` | with `generate --lang typescript` | `/usr/local/bin/zerodds-idlc` |
| Init | `tini` (PID 1) | `/usr/bin/tini` |

`NODE_PATH=/usr/local/lib/node_modules` and `LD_LIBRARY_PATH=/usr/local/lib`
are set — `import { ... } from '@zerodds/node'` and the koffi FFI
loader work without further measures.

## Learner workflow

```bash
# IDL -> TypeScript module
zerodds-idlc generate --lang typescript --out gen chat.idl
# creates gen/chat.ts with type definitions + codec roundtrip.

# The learner writes main.ts:
cat > main.ts <<'EOF'
import { DomainParticipant } from '@zerodds/node';
import { Greeting } from './gen/chat.js';

const p = new DomainParticipant({ domainId: 0 });
const writer = p.createWriter(Greeting, { topic: 'greetings' });
writer.write({ id: 42, text: 'hallo welt' });
EOF

# Run inline without a tsc build:
tsx main.ts
```

Loopback pub/sub in the same container (discovery via multicast loopback):

```typescript
import { fork } from 'node:child_process';
import { DomainParticipant } from '@zerodds/node';
import { Greeting } from './gen/chat.js';

// Subscriber in a child process:
if (process.argv[2] === '--sub') {
  const p = new DomainParticipant({ domainId: 0 });
  const reader = p.createReader(Greeting, { topic: 'greetings' });
  for (let i = 0; i < 5; i++) {
    const sample = await reader.takeNext({ timeoutMs: 1000 });
    console.log(sample);
  }
  process.exit(0);
}

// Publisher in the main process:
fork(import.meta.filename, ['--sub']);
await new Promise(r => setTimeout(r, 500));
const p = new DomainParticipant({ domainId: 0 });
const writer = p.createWriter(Greeting, { topic: 'greetings' });
for (let i = 0; i < 5; i++) {
  writer.write({ id: i, text: `hello ${i}` });
  await new Promise(r => setTimeout(r, 100));
}
```

## Limits

- **Discovery only via loopback multicast** in the same container — as
  in py-runtime and cpp-runtime. Multi-host discovery uses unicast peers: `discovery.peers` / `ZERODDS_PEERS` (see `docs/doc-map/config.en.md`).
- **`tsx` is a dev tool**, not a production bundler. For production
  builds use `tsc` + `node dist/main.js` in a separate pipeline
  — the ts-node image is tailored to learner/demo workflows.
- **koffi FFI needs executable memory** for FFI trampolines —
  test with a strongly restrictive seccomp profile (`--security-opt
  seccomp=...`).

## See also

- [`crates/ts-node/README.md`](../../../crates/ts-node/README.md) — Node.js binding crate.
- [`crates/idl-ts/README.md`](../../../crates/idl-ts/README.md) — IDL→TypeScript codegen.
- [`crates/ts-wasm/README.md`](../../../crates/ts-wasm/README.md) — browser WASM sister (codec-only, not in this image).
- [`packaging/docker/py-runtime/`](../py-runtime/) — Python sister image.
- [`packaging/docker/cpp-runtime/`](../cpp-runtime/) — C/C++ sister image.
