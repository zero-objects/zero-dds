# `zerodds/ts-node-runtime`

Sandbox-Runtime-Image fuer **TypeScript-Bindings unter Node.js**
(`@zerodds/node` aus `crates/ts-node`, via koffi-FFI gegen
`libzerodds.so`). Zielgruppe: Coding-Challenges in Zero-Learn-
Sandboxes und Quickstart-Demos fuer TypeScript-DDS-Entwicklung.

Teil von [**ZeroDDS**](../../../README.md). Anders als die Daemon-
Images liefert dieses Image **keinen ENTRYPOINT-Service**, sondern
eine bereite Node-Umgebung mit global installiertem `@zerodds/node`
und `tsx` fuer Direct-TS-Run ohne separaten Build-Step.

> **Hinweis:** Dieses Image ist Node.js-only. Browser-WASM-Pfad
> (`zerodds-ts-wasm`) braucht keinen Sandbox-Container — der laeuft
> im Browser, nicht im Server-Image. Live-Pub/Sub im Browser ist
> ohnehin nicht moeglich (WASM kann kein UDP-Multicast).

---

## Build

Vom Repo-Root:

```bash
docker build \
  -f packaging/docker/ts-node-runtime/Dockerfile \
  -t zerodds/ts-node-runtime:rc3 \
  .
```

4-Stage Build (chef → rust-builder → node-builder → runtime). Erst-
Build ~5-15 Min; Folge-Builds nutzen cargo-chef-Caching aggressiv.

## Run

Interaktive Shell (default CMD):

```bash
docker run --rm -it zerodds/ts-node-runtime:rc3
```

Direkter TS-Run mit gemountetem Lerner-Code:

```bash
docker run --rm -it \
  -v "$PWD/workspace:/workspace" \
  zerodds/ts-node-runtime:rc3 \
  tsx /workspace/main.ts
```

Sandbox-Style (read-only-root, kein Netz nach aussen):

```bash
docker run --rm \
  --read-only \
  --tmpfs /tmp:rw,size=64m \
  --network none \
  -v "$PWD/workspace:/workspace:ro" \
  zerodds/ts-node-runtime:rc3 \
  tsx /workspace/loopback_pubsub.ts
```

## Was drin ist

| Komponente | Version | Pfad |
| --- | --- | --- |
| Node.js | 22 (bookworm-slim) | `/usr/local/bin/node` |
| `@zerodds/node` (global) | RC-Build aus `crates/ts-node` | `/usr/local/lib/node_modules/@zerodds/node/` |
| `tsx` (global) | 4.x | `/usr/local/bin/tsx` |
| `libzerodds.so` | RC-Build aus `crates/zerodds-c-api` | `/usr/local/lib/libzerodds.so` |
| `zerodds-idlc` | mit `--ts` Backend | `/usr/local/bin/zerodds-idlc` |
| Init | `tini` (PID 1) | `/usr/bin/tini` |

`NODE_PATH=/usr/local/lib/node_modules` und `LD_LIBRARY_PATH=/usr/local/lib`
sind gesetzt — `import { ... } from '@zerodds/node'` und der koffi-FFI-
Loader funktionieren ohne weitere Massnahmen.

## Lerner-Workflow

```bash
# IDL -> TypeScript-Modul
zerodds-idlc --ts -o gen chat.idl
# erzeugt gen/chat.ts mit Type-Definitions + Codec-Roundtrip.

# Lerner schreibt main.ts:
cat > main.ts <<'EOF'
import { DomainParticipant } from '@zerodds/node';
import { Greeting } from './gen/chat.js';

const p = new DomainParticipant({ domainId: 0 });
const writer = p.createWriter(Greeting, { topic: 'greetings' });
writer.write({ id: 42, text: 'hallo welt' });
EOF

# Inline ausfuehren ohne tsc-Build:
tsx main.ts
```

Loopback-Pub/Sub im selben Container (Discovery via Multicast-Loopback):

```typescript
import { fork } from 'node:child_process';
import { DomainParticipant } from '@zerodds/node';
import { Greeting } from './gen/chat.js';

// Subscriber in Child-Process:
if (process.argv[2] === '--sub') {
  const p = new DomainParticipant({ domainId: 0 });
  const reader = p.createReader(Greeting, { topic: 'greetings' });
  for (let i = 0; i < 5; i++) {
    const sample = await reader.takeNext({ timeoutMs: 1000 });
    console.log(sample);
  }
  process.exit(0);
}

// Publisher im Main-Process:
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

- **Discovery nur via Loopback-Multicast** im selben Container — wie
  in py-runtime und cpp-runtime, vermerkt in
  `documentation/06-operations/deployment.md` (`unicast static
  peer-list` ist RC3-`planned`).
- **`tsx` ist Dev-Tool**, kein Production-Bundler. Fuer Production-
  Builds in einer separaten Pipeline `tsc` + `node dist/main.js`
  nutzen — das ts-node-Image ist auf Lerner-/Demo-Workflows
  zugeschnitten.
- **koffi-FFI braucht ausfuehrbaren Memory** fuer FFI-Trampolines —
  bei stark restriktivem seccomp-Profil (`--security-opt
  seccomp=...`) testen.

## See also

- [`crates/ts-node/README.md`](../../../crates/ts-node/README.md) — Node.js-Binding-Crate.
- [`crates/idl-ts/README.md`](../../../crates/idl-ts/README.md) — IDL→TypeScript-Codegen.
- [`crates/ts-wasm/README.md`](../../../crates/ts-wasm/README.md) — Browser-WASM-Schwester (Codec-only, nicht in diesem Image).
- [`packaging/docker/py-runtime/`](../py-runtime/) — Python-Schwester-Image.
- [`packaging/docker/cpp-runtime/`](../cpp-runtime/) — C/C++-Schwester-Image.
