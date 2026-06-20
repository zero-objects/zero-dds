// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// index.ts — public surface of the @zerodds/wasm package.
//
// Combines three layers:
//   1. the wasm-bindgen CDR codec glue (default `init`, `version`, CdrEncoder,
//      CdrDecoder) — generated under pkg-web/ by `wasm-pack build --target web`;
//   2. the DDS-TS Annex C browser DCPS-over-WebSocket runtime (facade + flat
//      C.2 operations);
//   3. the @zerodds/cdr XCDR2 codec re-export.
//
// The snippet `import init, { DomainParticipantFactory } from '@zerodds/wasm'`
// resolves `init` (default, WASM bootstrap) and `DomainParticipantFactory`
// (DCPS facade) from here.

// 1. WASM codec glue. `init` is the default __wbg_init bootstrap.
import init, { version, CdrEncoder, CdrDecoder } from "../pkg-web/dds_ts_wasm.js";
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

// 3. XCDR2 codec re-export (so codegen output can `import { Xcdr2Writer } from
//    '@zerodds/wasm'` if desired).
export {
  Xcdr2Writer,
  Xcdr2Reader,
  md5,
  XcdrError,
  type DdsTopicType,
  type ExtensibilityKind,
  type EndianMode,
} from "./cdr/index.js";
