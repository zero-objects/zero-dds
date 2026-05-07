// SPDX-License-Identifier: Apache-2.0
// DDS-TS 1.0 — Boxed any + exception marker.

/**
 * Boxed value carrying an explicit type identity.
 *
 * Maps IDL `any`. The `typeId` is a qualified IDL name that
 * indexes into the descriptor registry; the `value` is the
 * boxed payload. Producers construct via `boxAny`; consumers
 * narrow via `unboxAny`.
 */
export interface DdsAny {
  readonly typeId: string;
  readonly value:  unknown;
}

/**
 * Marker interface for IDL exception types.
 *
 * Every IDL `exception` is mapped to a TypeScript interface that
 * extends DdsException, allowing callers to distinguish exception
 * payloads from ordinary structs at compile time.
 */
export interface DdsException {
  readonly __dds_exception: true;
}
