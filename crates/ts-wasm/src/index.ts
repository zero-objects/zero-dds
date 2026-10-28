// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// index.ts — public surface of the @zerodds/wasm package.
//
// Combines two layers:
//   1. the wasm-bindgen CDR codec glue (default `init`, `version`, CdrEncoder,
//      CdrDecoder) — generated under pkg-web/ by `wasm-pack build --target web`.
//      `CdrEncoder`/`CdrDecoder` build on `zerodds-wire`'s `Writer`/`Reader`
//      (see lib.rs), the same runtime the generated Rust encoders call — the
//      one CDR codec this package carries;
//   2. the DDS-TS Annex C browser DCPS-over-WebSocket runtime (facade + flat
//      C.2 operations).
//
// A package that generates TypeScript for the browser and wants a pure-TS
// (non-WASM) codec depends on `@zerodds/wire` (`runtime/typescript`)
// directly — the package `zerodds-idlc generate --lang typescript` already
// imports from, and the one this package used to re-export its own,
// dead-relative-to-that-codegen copy of (`./cdr/`, XTypes 1.3 §7.4.3.4.2's
// EMHEADER LC=3 written with a NEXTINT for a variable-length @mutable
// member — 8 inline octets and no NEXTINT is what a conformant peer reads
// there, so that copy desynced from the member on; removed with it).
//
// The snippet `import init, { DomainParticipantFactory } from '@zerodds/wasm'`
// resolves `init` (default, WASM bootstrap) and `DomainParticipantFactory`
// (DCPS facade) from here.

// 1. WASM codec glue. `init` is the default __wbg_init bootstrap.
import init, { version, CdrEncoder, CdrDecoder } from "../pkg-web/zerodds_ts_wasm.js";
export default init;
export { version, CdrEncoder, CdrDecoder };

// 2. Browser DCPS facade (the quickstart API).
export {
  DomainParticipantFactory,
  DomainParticipant,
  Topic,
  Publisher,
  Subscriber,
  DataWriter,
  DataReader,
} from "./dcps/facade.js";

// 2b. Flat DDS-TS Annex C.2 operations + sample factory (spec-conformant
//     signature-for-signature surface).
export {
  registerParticipant,
  deleteParticipant,
  createTopic,
  deleteTopic,
  createPublisher,
  createSubscriber,
  deletePublisher,
  deleteSubscriber,
  createDataWriter,
  createDataReader,
  writeSample,
  takeSamples,
  deleteDataWriter,
  deleteDataReader,
  setDataAvailableListener,
  sampleFromBytes,
} from "./dcps/operations.js";

// 2c. Annex C.1 handle + sample types.
export {
  makeDdsGuid,
  nullGuid,
  type ParticipantHandle,
  type TopicHandle,
  type PublisherHandle,
  type SubscriberHandle,
  type DataWriterHandle,
  type DataReaderHandle,
  type DdsGuid,
  type Sample,
  type SampleInfo,
  type DataAvailableCallback,
} from "./dcps/handles.js";

export {
  type WebSocketLike,
  type WebSocketFactory,
  type BridgeNotification,
  BridgeTransport,
  bytesToBase64,
  base64ToBytes,
} from "./dcps/transport.js";
