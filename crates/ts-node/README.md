# `@zerodds/node` — ZeroDDS TypeScript-Node-Binding

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

TypeScript-Bindings für ZeroDDS in Node.js via `koffi`-FFI gegen
`libzerodds.dylib` / `.so` / `.dll` (aus `crates/zerodds-c-api`).

## Spec

- OMG DDS 1.4 (formal/2015-04-10) §2.2.2 — DCPS-API
- OMG DDS-PSM-Cxx 1.0 (formal/2013-11-01) §7.5 — adaptiert auf
  TypeScript-Idiome
- ZeroDDS Vendor-Spec `zerodds-c-api-1.0` als FFI-Foundation

## Layer

Layer 6 — PSMs / Bindings.

## Quickstart

```typescript
import {
  DomainParticipantFactory, Topic, Publisher, DataWriter,
  Subscriber, DataReader, ByteSeqTraits,
} from "@zerodds/node";

const dp = DomainParticipantFactory.createParticipant(0);
const t = Topic.create(dp, "Chatter", ByteSeqTraits);
const pub = Publisher.create(dp);
const dw = DataWriter.create(pub, t);
const sub = Subscriber.create(dp);
const dr = DataReader.create(sub, t);

dw.write(new Uint8Array([1, 2, 3, 4]));

dr.destroy();
sub.destroy();
dw.destroy();
pub.destroy();
t.destroy();
dp.destroy();
```

## Build

```bash
# Erst libzerodds bauen
cargo build --release -p zerodds-c-api

# Dann TS-Module
cd crates/ts-node
npm install
npm run build
npm test
```

## API

- **`DomainParticipantFactory`** — Singleton, erzeugt Participants
- **`DomainParticipant`** — Domain-Lifecycle, RAII
- **`Topic<T>`** — Topic + Type-Traits
- **`Publisher`** / **`DataWriter<T>`** — Pub-Side
- **`Subscriber`** / **`DataReader<T>`** — Sub-Side
- **`GuardCondition`** / **`WaitSet`** — Conditions
- **Legacy:** `Runtime` / `Writer` / `Reader` (typgelöschter Pfad)

## Stabilitaet

Pre-1.0 — API kann sich aendern. RC1-konformer Spec-API ist
`DomainParticipantFactory`-basierte Surface; Legacy `Runtime`-API
bleibt fuer Apex.AI-/ROS-2-RMW-Use-Cases stabil.

## Links

- Spec-Coverage: `docs/spec-coverage/zerodds-c-api-1.0.md`
- Vendor-Spec: `docs/specs/zerodds-c-api-1.0.md`
- CHANGELOG: [`CHANGELOG.md`](./CHANGELOG.md)
