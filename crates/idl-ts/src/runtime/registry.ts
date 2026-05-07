// SPDX-License-Identifier: Apache-2.0
// DDS-TS 1.0 — Descriptor registry and reflection helpers.

import type { DdsTypeDescriptor } from "./types.js";
import type { DdsAny } from "./dds_any.js";

const registry = new Map<string, DdsTypeDescriptor>();

/**
 * Adds a descriptor to the process-wide registry keyed by name.
 *
 * Re-registration with a structurally-equivalent descriptor is
 * silently ignored. Re-registration with a structurally-different
 * descriptor throws.
 *
 * Structural equivalence is defined as deep value-equality of all
 * descriptor fields except `typeGuard`, which is compared by
 * reference identity (functions are not deep-equals comparable).
 */
export function registerType(d: DdsTypeDescriptor): void {
  const existing = registry.get(d.name);
  if (existing === undefined) {
    registry.set(d.name, d);
    return;
  }
  if (existing === d) {
    return;
  }
  if (!structurallyEquivalent(existing, d)) {
    throw new Error(
      `registerType: collision for "${d.name}" with ` +
      `structurally different descriptor`,
    );
  }
  // Equivalent re-registration: keep the first.
}

/**
 * Returns the previously-registered descriptor for a qualified
 * name, or undefined.
 */
export function lookupType(qualifiedName: string): DdsTypeDescriptor | undefined {
  return registry.get(qualifiedName);
}

/**
 * Returns the values of all key-marked members in declaration
 * order. The function does not validate the sample's shape; the
 * caller is expected to invoke `descriptor.typeGuard` first if
 * the sample's origin is untrusted.
 */
export function getKey<T>(
  sample: T,
  descriptor: DdsTypeDescriptor<T>,
): ReadonlyArray<unknown> {
  const out: unknown[] = [];
  const obj = sample as unknown as Record<string, unknown>;
  for (const field of descriptor.fields) {
    if (field.key) {
      out.push(obj[field.name]);
    }
  }
  return out;
}

/** Returns descriptor.topic unchanged. */
export function getTopic<T>(descriptor: DdsTypeDescriptor<T>): string | undefined {
  return descriptor.topic;
}

/**
 * Returns a new value of T in which any property absent from
 * `partial` but defined in the descriptor's member list with a
 * non-undefined `default` is filled from that default. Properties
 * absent from both remain absent.
 */
export function withDefaults<T>(
  descriptor: DdsTypeDescriptor<T>,
  partial: Partial<T>,
): T {
  const out: Record<string, unknown> = { ...(partial as Record<string, unknown>) };
  for (const field of descriptor.fields) {
    if (!(field.name in out) && field.default !== undefined) {
      out[field.name] = field.default;
    }
  }
  return out as T;
}

/**
 * Constructs a DdsAny carrying the typed value plus the
 * descriptor's qualified name as `typeId`. Throws if the value
 * does not satisfy the descriptor's type-guard.
 */
export function boxAny<T>(
  descriptor: DdsTypeDescriptor<T>,
  value: T,
): DdsAny {
  if (!descriptor.typeGuard(value)) {
    throw new TypeError(
      `boxAny: value does not satisfy descriptor "${descriptor.name}"`,
    );
  }
  return { typeId: descriptor.name, value };
}

/**
 * Returns the typed value if `any.typeId` matches the descriptor
 * name and the descriptor's type-guard accepts the unboxed value.
 * Throws TypeError otherwise.
 */
export function unboxAny<T>(
  any: DdsAny,
  descriptor: DdsTypeDescriptor<T>,
): T {
  if (any.typeId !== descriptor.name) {
    throw new TypeError(
      `unboxAny: typeId "${any.typeId}" does not match ` +
      `descriptor "${descriptor.name}"`,
    );
  }
  if (!descriptor.typeGuard(any.value)) {
    throw new TypeError(
      `unboxAny: value does not satisfy descriptor "${any.typeId}"`,
    );
  }
  return any.value as T;
}

// --------------------------------------------------------------
// Internal: structural equivalence.
//
// Compares all serialisable descriptor fields. The typeGuard
// function is intentionally excluded — functions are not deep-
// equals comparable; the registry distinguishes only by structure.
// --------------------------------------------------------------

function structurallyEquivalent(
  a: DdsTypeDescriptor,
  b: DdsTypeDescriptor,
): boolean {
  return jsonEquivalent(stripGuard(a), stripGuard(b));
}

function stripGuard(d: DdsTypeDescriptor): unknown {
  const { typeGuard: _ignored, ...rest } = d;
  return rest;
}

function jsonEquivalent(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return false;
  if (typeof a !== "object") return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (!jsonEquivalent(a[i], b[i])) return false;
    }
    return true;
  }
  const aKeys = Object.keys(a as object).sort();
  const bKeys = Object.keys(b as object).sort();
  if (aKeys.length !== bKeys.length) return false;
  for (let i = 0; i < aKeys.length; i++) {
    if (aKeys[i] !== bKeys[i]) return false;
  }
  for (const k of aKeys) {
    const av = (a as Record<string, unknown>)[k];
    const bv = (b as Record<string, unknown>)[k];
    if (!jsonEquivalent(av, bv)) return false;
  }
  return true;
}

// Test-only: clear the registry between conformance tests.
// Not part of the conformance surface.
export function __clearRegistryForTests(): void {
  registry.clear();
}
