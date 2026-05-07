// SPDX-License-Identifier: Apache-2.0
// DDS-TS 1.0 — WASM-Bindings Profile (Annex C).
//
// This module declares the normative TypeScript surface for the
// WASM-Bindings Profile. The actual runtime backing (a WASM module
// compiled from a DDS implementation, or a Node.js native module
// via N-API) is supplied at link time and lies outside this
// type-level surface. Stubs here throw at call time so that a
// declared-only deployment fails loudly rather than silently
// dropping samples.

import type { DdsAny } from "./dds_any.js";

const _ddsAnyMarker: DdsAny | undefined = undefined;
void _ddsAnyMarker;

// -----------------------------------------------------------
// §C.1.1 Branded Handle Types — string-literal-tag brands
// (cross-vendor stable).
// -----------------------------------------------------------

export type ParticipantHandle = number & { readonly __dds_brand: "participant" };
export type TopicHandle       = number & { readonly __dds_brand: "topic"       };
export type PublisherHandle   = number & { readonly __dds_brand: "publisher"   };
export type SubscriberHandle  = number & { readonly __dds_brand: "subscriber"  };
export type DataWriterHandle  = number & { readonly __dds_brand: "writer"      };
export type DataReaderHandle  = number & { readonly __dds_brand: "reader"      };

// -----------------------------------------------------------
// §C.1.2 Sample types
// -----------------------------------------------------------

/** OMG DDSI-RTPS GUID (16 bytes = 12-byte GuidPrefix + 4-byte EntityId). */
export interface DdsGuid {
  readonly prefix:   Uint8Array;
  readonly entityId: number;
}

export function makeDdsGuid(prefix: Uint8Array, entityId: number): DdsGuid {
  if (prefix.length !== 12) {
    throw new RangeError("DdsGuid.prefix requires 12 bytes");
  }
  if (!Number.isInteger(entityId) || entityId < 0 || entityId > 0xffffffff) {
    throw new RangeError("DdsGuid.entityId requires uint32");
  }
  return { prefix, entityId };
}

export interface SampleInfo {
  readonly validData:         boolean;
  readonly sampleState:       "read" | "not_read";
  readonly viewState:         "new"  | "not_new";
  readonly instanceState:
    | "alive"
    | "not_alive_disposed"
    | "not_alive_no_writers";
  readonly sourceTimestampNs: bigint;
  readonly sequenceNumber:    bigint;
  readonly publicationHandle: DdsGuid;
  readonly instanceHandle:    bigint;
}

export interface Sample {
  readonly bytes: Uint8Array;
  readonly info:  SampleInfo;
}

// -----------------------------------------------------------
// §C.1.3 Listener callback type
// -----------------------------------------------------------

export type DataAvailableCallback = (r: DataReaderHandle) => void;

// -----------------------------------------------------------
// §C.2 Required Operations
//
// Stub bodies throw `Error("WASM backend not bound")`. A
// concrete backend is bound by the consuming application via
// `bindWasmBackend(impl)` (below).
// -----------------------------------------------------------

export interface WasmBackend {
  createParticipant(domainId: number): ParticipantHandle;
  deleteParticipant(p: ParticipantHandle): void;

  createTopic(p: ParticipantHandle, name: string, typeName: string): TopicHandle;
  deleteTopic(t: TopicHandle): void;

  createPublisher(p: ParticipantHandle): PublisherHandle;
  createSubscriber(p: ParticipantHandle): SubscriberHandle;
  deletePublisher(pub: PublisherHandle): void;
  deleteSubscriber(sub: SubscriberHandle): void;

  createDataWriter(pub: PublisherHandle, topic: TopicHandle): DataWriterHandle;
  createDataReader(sub: SubscriberHandle, topic: TopicHandle): DataReaderHandle;

  writeSample(w: DataWriterHandle, xcdr2: Uint8Array): void;
  takeSamples(r: DataReaderHandle, max: number): ReadonlyArray<Sample>;

  deleteDataWriter(w: DataWriterHandle): void;
  deleteDataReader(r: DataReaderHandle): void;

  setDataAvailableListener(
    r: DataReaderHandle,
    cb: DataAvailableCallback | null,
  ): void;
}

let _backend: WasmBackend | undefined = undefined;

/** Binds a concrete WASM/N-API backend implementation. Once bound,
 *  the public C.2 operations dispatch to it. */
export function bindWasmBackend(impl: WasmBackend): void {
  _backend = impl;
}

function backend(): WasmBackend {
  if (_backend === undefined) {
    throw new Error(
      "WASM backend not bound — call bindWasmBackend(impl) before " +
      "invoking C.2 operations",
    );
  }
  return _backend;
}

export function createParticipant(domainId: number): ParticipantHandle {
  return backend().createParticipant(domainId);
}
export function deleteParticipant(p: ParticipantHandle): void {
  backend().deleteParticipant(p);
}
export function createTopic(
  p: ParticipantHandle,
  name: string,
  typeName: string,
): TopicHandle {
  return backend().createTopic(p, name, typeName);
}
export function deleteTopic(t: TopicHandle): void {
  backend().deleteTopic(t);
}
export function createPublisher(p: ParticipantHandle): PublisherHandle {
  return backend().createPublisher(p);
}
export function createSubscriber(p: ParticipantHandle): SubscriberHandle {
  return backend().createSubscriber(p);
}
export function deletePublisher(pub: PublisherHandle): void {
  backend().deletePublisher(pub);
}
export function deleteSubscriber(sub: SubscriberHandle): void {
  backend().deleteSubscriber(sub);
}
export function createDataWriter(
  pub: PublisherHandle,
  topic: TopicHandle,
): DataWriterHandle {
  return backend().createDataWriter(pub, topic);
}
export function createDataReader(
  sub: SubscriberHandle,
  topic: TopicHandle,
): DataReaderHandle {
  return backend().createDataReader(sub, topic);
}
export function writeSample(w: DataWriterHandle, xcdr2: Uint8Array): void {
  backend().writeSample(w, xcdr2);
}
export function takeSamples(
  r: DataReaderHandle,
  max: number,
): ReadonlyArray<Sample> {
  return backend().takeSamples(r, max);
}
export function deleteDataWriter(w: DataWriterHandle): void {
  backend().deleteDataWriter(w);
}
export function deleteDataReader(r: DataReaderHandle): void {
  backend().deleteDataReader(r);
}
export function setDataAvailableListener(
  r: DataReaderHandle,
  cb: DataAvailableCallback | null,
): void {
  backend().setDataAvailableListener(r, cb);
}

// -----------------------------------------------------------
// §C.3.1 / C.3.2 / C.3.3 — Wire-format and ownership rules
// are documented in DDS-TS 1.0 Annex C; they are runtime
// invariants of the bound backend. The TS surface enforces
// only the type-shape (Uint8Array, GUID-12-byte etc.).
// -----------------------------------------------------------
