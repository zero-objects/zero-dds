// SPDX-License-Identifier: Apache-2.0
// DDS-TS 1.0 — Key equality and exception narrowing.

import type {
  DdsTypeDescriptor,
  DdsTypeRef,
  GuardedTypeOf,
} from "./types.js";
import type { DdsAny } from "./dds_any.js";
import { lookupType } from "./registry.js";

/**
 * Structural value-equality between two key values described by
 * `keyType`.
 *
 * Primitive, string, bigint and branded-character keys compare
 * via JavaScript value-equality (===) with one normative
 * exception: NaN floating-point values compare equal to
 * themselves (Object.is-style), distinct NaN bit-patterns
 * compare equal, and +0 / -0 compare equal.
 *
 * `ref`-kind keys recurse through the referenced descriptor
 * (overriding JavaScript's default reference-equality on
 * objects). `any`-kind keys compare both `typeId` and the
 * unboxed value.
 *
 * Map, sequence, and array values are not permitted as IDL keys
 * and are not handled here.
 */
export function equalKey(
  a: unknown,
  b: unknown,
  keyType: DdsTypeRef,
): boolean {
  switch (keyType.kind) {
    case "primitive":
      return scalarEqual(a, b);
    case "bitfield":
      return scalarEqual(a, b);
    case "string":
      return scalarEqual(a, b);
    case "ref":
      return refEqual(a, b, keyType.name);
    case "any":
      return anyEqual(a, b);
    default:
      // sequence/array/map are not permitted as IDL keys.
      return false;
  }
}

function scalarEqual(a: unknown, b: unknown): boolean {
  // Object.is handles NaN === NaN as true and +0 / -0 as equal.
  return Object.is(a, b);
}

function refEqual(a: unknown, b: unknown, refName: string): boolean {
  if (a === b) return true;
  if (typeof a !== "object" || typeof b !== "object") return false;
  if (a === null || b === null) return false;
  const descriptor = lookupType(refName);
  if (descriptor === undefined) {
    // Unknown referenced descriptor: fall back to deep-equal.
    return deepEqual(a, b);
  }
  const aObj = a as Record<string, unknown>;
  const bObj = b as Record<string, unknown>;
  for (const field of descriptor.fields) {
    const av = aObj[field.name];
    const bv = bObj[field.name];
    if (!equalKey(av, bv, field.type)) return false;
  }
  return true;
}

function anyEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== "object" || typeof b !== "object") return false;
  if (a === null || b === null) return false;
  const aAny = a as DdsAny;
  const bAny = b as DdsAny;
  if (aAny.typeId !== bAny.typeId) return false;
  const descriptor = lookupType(aAny.typeId);
  if (descriptor === undefined) {
    return deepEqual(aAny.value, bAny.value);
  }
  return refEqual(aAny.value, bAny.value, descriptor.name);
}

function deepEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  if (typeof a !== "object" || typeof b !== "object") return false;
  if (a === null || b === null) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (!deepEqual(a[i], b[i])) return false;
    }
    return true;
  }
  const aKeys = Object.keys(a as object);
  const bKeys = Object.keys(b as object);
  if (aKeys.length !== bKeys.length) return false;
  for (const k of aKeys) {
    if (!deepEqual(
      (a as Record<string, unknown>)[k],
      (b as Record<string, unknown>)[k],
    )) {
      return false;
    }
  }
  return true;
}

/**
 * Type-narrowing helper for catch-handlers of generated
 * Operations-Profile clients.
 *
 * Returns true if and only if `err` is an Error whose attached
 * `cause` property satisfies the type-guard of any of the
 * supplied exception descriptors. Non-Error inputs return false
 * (the function never throws).
 */
export function isOneOf<DS extends ReadonlyArray<DdsTypeDescriptor>>(
  err: unknown,
  descriptors: DS,
): err is Error & { cause: GuardedTypeOf<DS[number]> } {
  if (!(err instanceof Error)) return false;
  const cause = (err as { cause?: unknown }).cause;
  if (cause === undefined) return false;
  for (const d of descriptors) {
    if (d.typeGuard(cause)) return true;
  }
  return false;
}
