// SPDX-License-Identifier: Apache-2.0
// DDS-TS 1.0 — @zerodds/types runtime barrel.
//
// Conformance: this module exports every symbol required by the
// Descriptor-Runtime Profile (Annex B §B.2.1 of the DDS-TS 1.0
// vendor specification).

export type {
  ExtensibilityKind,
  PrimitiveName,
  DdsTypeRef,
  DescriptorKind,
  DdsMemberDescriptor,
  DdsTypeDescriptor,
  GuardedTypeOf,
} from "./types.js";

export {
  type Char,
  type WChar,
  type LongDouble,
  makeChar,
  makeWChar,
  makeLongDouble,
} from "./branded.js";

export {
  type DdsAny,
  type DdsException,
} from "./dds_any.js";

export {
  registerType,
  lookupType,
  getKey,
  getTopic,
  withDefaults,
  boxAny,
  unboxAny,
} from "./registry.js";

export {
  equalKey,
  isOneOf,
} from "./equal.js";

export type {
  ParameterMode,
  OperationParameterDescriptor,
  OperationDescriptor,
  AttributeDescriptor,
  ServiceDescriptor,
} from "./operations.js";

// Annex C — WASM-Bindings Profile (opt-in).
export type {
  ParticipantHandle, TopicHandle,
  PublisherHandle, SubscriberHandle,
  DataWriterHandle, DataReaderHandle,
  DdsGuid, SampleInfo, Sample,
  DataAvailableCallback,
  WasmBackend,
} from "./wasm.js";

export {
  makeDdsGuid,
  bindWasmBackend,
  createParticipant, deleteParticipant,
  createTopic, deleteTopic,
  createPublisher, createSubscriber,
  deletePublisher, deleteSubscriber,
  createDataWriter, createDataReader,
  writeSample, takeSamples,
  deleteDataWriter, deleteDataReader,
  setDataAvailableListener,
} from "./wasm.js";

// In-memory test backend (§C.5 round-trip reference).
export { createInMemoryBackend } from "./test_backend.js";
