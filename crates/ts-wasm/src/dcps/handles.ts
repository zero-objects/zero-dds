// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// handles.ts — DDS-TS 1.0 Annex C.1 handle + sample types.
//
// The brand tags ("participant", "topic", …) and the property name
// `__dds_brand` are normative (Annex C.1.1); string-literal brands compare
// structurally across vendor/module boundaries.

/// C.1.1 — branded handle types. Each handle is a non-negative 32-bit integer
/// index into the host resource table; `0` is the invalid-handle sentinel.
export type ParticipantHandle = number & { readonly __dds_brand: "participant" };
export type TopicHandle = number & { readonly __dds_brand: "topic" };
export type PublisherHandle = number & { readonly __dds_brand: "publisher" };
export type SubscriberHandle = number & { readonly __dds_brand: "subscriber" };
export type DataWriterHandle = number & { readonly __dds_brand: "writer" };
export type DataReaderHandle = number & { readonly __dds_brand: "reader" };

/// C.1.2 — OMG DDSI-RTPS GUID (12-byte GuidPrefix + 4-byte EntityId).
export interface DdsGuid {
  readonly prefix: Uint8Array; // length === 12
  readonly entityId: number; // 32-bit unsigned
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

/// C.1.2 — strict subset of the OMG DDS 1.4 SampleInfo (§2.2.2.5.4).
export interface SampleInfo {
  readonly validData: boolean;
  readonly sampleState: "read" | "not_read";
  readonly viewState: "new" | "not_new";
  readonly instanceState: "alive" | "not_alive_disposed" | "not_alive_no_writers";
  readonly sourceTimestampNs: bigint;
  readonly sequenceNumber: bigint;
  readonly publicationHandle: DdsGuid;
  readonly instanceHandle: bigint;
  /// Wire byte order of `bytes` (the bridge reports it out-of-band because the
  /// forwarded payload has no encapsulation header): `false` = little-endian,
  /// `true` = big-endian. The typed decoder dispatches its byte order on it.
  readonly bigEndian: boolean;
}

/// C.1.2 — a taken sample. `bytes` is the XCDR2 octet sequence exactly as it
/// crossed the wire.
export interface Sample {
  readonly bytes: Uint8Array;
  readonly info: SampleInfo;
}

/// C.1.3 — data-available listener callback.
export type DataAvailableCallback = (r: DataReaderHandle) => void;

/// The null GUID (all-zero prefix + entityId 0). Used as the publication handle
/// when the bridge does not surface the originating writer GUID.
export function nullGuid(): DdsGuid {
  return { prefix: new Uint8Array(12), entityId: 0 };
}
