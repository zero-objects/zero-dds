# `@zerodds/node` — ZeroDDS TypeScript-Node-Binding

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

TypeScript bindings for ZeroDDS in Node.js via `koffi` FFI against
`libzerodds.dylib` / `.so` / `.dll` (from `crates/zerodds-c-api`).

## Spec

- OMG DDS 1.4 (formal/2015-04-10) §2.2.2 — DCPS API
- OMG DDS-PSM-Cxx 1.0 (formal/2013-11-01) §7.5 — adapted to
  TypeScript idioms
- ZeroDDS vendor spec `zerodds-c-api-1.0` as the FFI foundation

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
# First build libzerodds
cargo build --release -p zerodds-c-api

# Then the TS module
cd crates/ts-node
npm install
npm run build
npm test
```

## API

- **`DomainParticipantFactory`** — singleton, creates participants
- **`DomainParticipant`** — Domain-Lifecycle, RAII
- **`Topic<T>`** — Topic + Type-Traits
- **`Publisher`** / **`DataWriter<T>`** — Pub-Side
- **`Subscriber`** / **`DataReader<T>`** — Sub-Side
- **`GuardCondition`** / **`WaitSet`** — Conditions
- **Legacy:** `Runtime` / `Writer` / `Reader` (type-erased path)

## Stability

Pre-1.0 — the API may change. The RC1-conform spec API is the
`DomainParticipantFactory`-based surface; the legacy `Runtime` API
stays stable for Apex.AI / ROS-2 RMW use cases.

## Links

- Spec-Coverage: `docs/spec-coverage/zerodds-c-api-1.0.md`
- Vendor-Spec: `docs/specs/zerodds-c-api-1.0.md`
- CHANGELOG: [`CHANGELOG.md`](./CHANGELOG.md)
