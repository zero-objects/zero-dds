// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// operations.ts — DDS-TS 1.0 Annex C.2 flat operations + the C.1.2 Sample
// factory. The fluent facade (`facade.ts`) is the ergonomic form; this module
// provides the normative signature-for-signature surface so a TS application
// can retarget between browser and Node bindings without source changes
// (Annex C.4.2).
//
// Handles are 32-bit-integer indices into a per-process resource table; `0` is
// the invalid-handle sentinel and is rejected at runtime (Annex C.1.1).

import {
  type DataReaderHandle,
  type DataWriterHandle,
  type DataAvailableCallback,
  type ParticipantHandle,
  type PublisherHandle,
  type Sample,
  type SampleInfo,
  type SubscriberHandle,
  type TopicHandle,
  nullGuid,
} from "./handles.js";
import {
  DataReader,
  DataWriter,
  DomainParticipant,
  Publisher,
  Subscriber,
  Topic,
} from "./facade.js";

/// Builds a C.1.2 Sample from raw XCDR2 `bytes`. Source info that the bridge
/// JSON protocol does not carry (timestamps, sequence number, writer GUID) is
/// filled with neutral defaults; `validData` is true for a real payload.
export function sampleFromBytes(bytes: Uint8Array): Sample {
  const info: SampleInfo = {
    validData: true,
    sampleState: "not_read",
    viewState: "new",
    instanceState: "alive",
    sourceTimestampNs: 0n,
    sequenceNumber: 0n,
    publicationHandle: nullGuid(),
    instanceHandle: 0n,
  };
  return { bytes, info };
}

// ---- Resource table (Annex C.1.1) ----

type Entity =
  | { kind: "participant"; obj: DomainParticipant }
  | { kind: "topic"; obj: Topic }
  | { kind: "publisher"; obj: Publisher }
  | { kind: "subscriber"; obj: Subscriber }
  | { kind: "writer"; obj: DataWriter }
  | { kind: "reader"; obj: DataReader };

// Index 0 is the reserved invalid sentinel; real handles start at 1.
const table: (Entity | null)[] = [null];

// Derived-handle ownership: participant handle -> the topic/pub/sub/writer/
// reader handles created under it, so `deleteParticipant` can release them all
// (Annex C.5.3). `ownerOf` maps each child handle back to its participant so a
// writer/reader created from a pub/sub can resolve its owning participant.
const derived = new Map<number, Set<number>>();
const ownerOf = new Map<number, number>();

function insert(e: Entity): number {
  table.push(e);
  return table.length - 1;
}

// Records that handle `child` belongs to participant `owner`.
function own(owner: number, child: number): void {
  let s = derived.get(owner);
  if (!s) {
    s = new Set<number>();
    derived.set(owner, s);
  }
  s.add(child);
  ownerOf.set(child, owner);
}

// Validates that handle `h` resolves to a live entity of `kind` and returns the
// matching table slot (already narrowed via the runtime `kind` check). Index 0
// is the reserved invalid sentinel (Annex C.1.1).
function slot(h: number, kind: Entity["kind"]): Entity {
  if (h === 0) throw new RangeError(`invalid ${kind} handle 0`);
  const e = table[h];
  if (!e || e.kind !== kind) {
    throw new RangeError(`invalid or deleted ${kind} handle ${h}`);
  }
  return e;
}

function getParticipant(h: number): DomainParticipant {
  const e = slot(h, "participant");
  return (e as { obj: DomainParticipant }).obj;
}
function getTopic(h: number): Topic {
  const e = slot(h, "topic");
  return (e as { obj: Topic }).obj;
}
function getPublisher(h: number): Publisher {
  const e = slot(h, "publisher");
  return (e as { obj: Publisher }).obj;
}
function getSubscriber(h: number): Subscriber {
  const e = slot(h, "subscriber");
  return (e as { obj: Subscriber }).obj;
}
function getWriter(h: number): DataWriter {
  const e = slot(h, "writer");
  return (e as { obj: DataWriter }).obj;
}
function getReader(h: number): DataReader {
  const e = slot(h, "reader");
  return (e as { obj: DataReader }).obj;
}

function free(h: number, kind: Entity["kind"]): void {
  slot(h, kind);
  table[h] = null;
}

/// Registers an already-connected participant (from
/// `DomainParticipantFactory.createParticipantWebSocket`) in the flat table and
/// returns its handle. The flat C.2 surface needs a live transport, which only
/// the async connect path can provide.
export function registerParticipant(p: DomainParticipant): ParticipantHandle {
  return insert({ kind: "participant", obj: p }) as ParticipantHandle;
}

// ---- C.2.1 Participant and Topic ----

export function deleteParticipant(p: ParticipantHandle): void {
  getParticipant(p).destroy();
  // Cascade-release every derived handle (Annex C.5.3): subsequent operations
  // on them SHALL throw, never silently succeed.
  const children = derived.get(p);
  if (children) {
    for (const child of children) {
      table[child] = null;
      ownerOf.delete(child);
    }
    derived.delete(p);
  }
  free(p, "participant");
}

export function createTopic(
  p: ParticipantHandle,
  name: string,
  typeName: string,
): TopicHandle {
  const topic = getParticipant(p).createTopic(name, typeName);
  const h = insert({ kind: "topic", obj: topic });
  own(p, h);
  return h as TopicHandle;
}

export function deleteTopic(t: TopicHandle): void {
  free(t, "topic");
}

// ---- C.2.2 Publisher and Subscriber ----

export function createPublisher(p: ParticipantHandle): PublisherHandle {
  const pub = getParticipant(p).createPublisher();
  const h = insert({ kind: "publisher", obj: pub });
  own(p, h);
  return h as PublisherHandle;
}

export function createSubscriber(p: ParticipantHandle): SubscriberHandle {
  const sub = getParticipant(p).createSubscriber();
  const h = insert({ kind: "subscriber", obj: sub });
  own(p, h);
  return h as SubscriberHandle;
}

export function deletePublisher(pub: PublisherHandle): void {
  free(pub, "publisher");
}

export function deleteSubscriber(sub: SubscriberHandle): void {
  free(sub, "subscriber");
}

// ---- C.2.3 DataWriter and DataReader ----

export function createDataWriter(
  pub: PublisherHandle,
  topic: TopicHandle,
): DataWriterHandle {
  const w = getPublisher(pub).createBytesWriter(getTopic(topic));
  const h = insert({ kind: "writer", obj: w });
  const owner = ownerOf.get(pub);
  if (owner !== undefined) own(owner, h);
  return h as DataWriterHandle;
}

export function createDataReader(
  sub: SubscriberHandle,
  topic: TopicHandle,
): DataReaderHandle {
  const r = getSubscriber(sub).createBytesReader(getTopic(topic));
  const h = insert({ kind: "reader", obj: r });
  const owner = ownerOf.get(sub);
  if (owner !== undefined) own(owner, h);
  return h as DataReaderHandle;
}

export function writeSample(w: DataWriterHandle, xcdr2: Uint8Array): void {
  getWriter(w).write(xcdr2);
}

export function takeSamples(
  r: DataReaderHandle,
  max: number,
): ReadonlyArray<Sample> {
  return getReader(r).takeSamples(max);
}

export function deleteDataWriter(w: DataWriterHandle): void {
  free(w, "writer");
}

export function deleteDataReader(r: DataReaderHandle): void {
  free(r, "reader");
}

// ---- C.2.4 Listener Registration ----

export function setDataAvailableListener(
  r: DataReaderHandle,
  cb: DataAvailableCallback | null,
): void {
  getReader(r).setDataAvailableListener(cb);
}
