// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * Equality, twice: over a key value and over a descriptor.
 *
 * DDS-TS 1.0 §8.7. Two different questions that JavaScript answers the
 * same wrong way.
 *
 * `a === b` on two objects is reference equality, so two samples with
 * identical key members compare unequal and a DDS instance would split
 * in two. {@link equalKey} walks the key's {@link DdsTypeRef} instead
 * and compares the values the type describes.
 *
 * {@link descriptorsEqual} answers the other one: whether a second
 * `registerType` call carries the same type as the first. The registry
 * needs it to tell a harmless double-import from two different types
 * sharing a name, and the second has to throw.
 */

import { lookupType } from "./registry.js";
import type { DdsMemberDescriptor, DdsTypeDescriptor, DdsTypeRef } from "./types.js";

/**
 * How deep a value graph is walked before it is taken to be malformed.
 *
 * A descriptor graph has no cycles — `DdsTypeRef.kind === "ref"` carries
 * a name and not a reference, which is what §8.2 says it is for — so a
 * chain longer than this is a value that was built by hand and is
 * pointing at itself. Recursing into one would end the process rather
 * than report anything.
 */
const MAX_DEPTH = 64;

/** The synthetic member id a union's discriminator carries (§7.5.1). */
const DISCRIMINATOR_ID = 0xffffffff;

/**
 * Whether two key values described by `keyType` are the same key.
 *
 * DDS-TS 1.0 §8.7 (`equalKey`). The rules, in the order they apply:
 *
 * - Primitives, strings, `bigint`s and branded characters compare by
 *   **value**, with two departures from `===` that are both normative.
 *   `NaN` equals itself, because a key that never matches itself would
 *   make every sample its own instance; and `+0` equals `-0`, because
 *   `Object.is` separates them and nothing on the wire does.
 * - A `ref` resolves through `lookupType` and compares member by
 *   member. This is the case that overrides JavaScript's default: two
 *   distinct objects holding the same members are one key.
 * - An `any` compares its `typeId` and then its payload through the
 *   descriptor that `typeId` names.
 * - A map, a sequence and an array are **not** IDL key types
 *   (X-Types 1.3 §7.2.2.4.5); reaching one is a malformed descriptor
 *   and throws rather than answering.
 *
 * Floating-point keys are allowed here and are a poor idea in a topic
 * design: two writers that computed the same value by different routes
 * can differ in the last place, and then they are two instances.
 *
 * @throws TypeError for a key type IDL does not permit, for a `ref` the
 * registry does not answer for, and for a value graph deeper than
 * {@link MAX_DEPTH}.
 *
 * zerodds-lint: recursion-depth 64 (key nesting; bounded by `MAX_DEPTH`)
 */
export function equalKey(a: unknown, b: unknown, keyType: DdsTypeRef): boolean {
  return equalKeyAt(a, b, keyType, 0);
}

/** {@link equalKey}, carrying the depth it has reached. */
function equalKeyAt(a: unknown, b: unknown, keyType: DdsTypeRef, depth: number): boolean {
  if (depth > MAX_DEPTH) {
    throw new TypeError(`equalKey: the key nests past ${MAX_DEPTH}`);
  }
  switch (keyType.kind) {
    case "primitive":
    case "bitfield":
    case "string":
      return sameValue(a, b);

    case "ref":
      return equalRef(a, b, keyType.name, depth);

    case "any":
      return equalBoxed(a, b, depth);

    case "sequence":
    case "array":
    case "map":
      throw new TypeError(
        `equalKey: a \`${keyType.kind}\` is not an IDL key type`,
      );
  }
}

/**
 * Value equality with the two departures §8.7 requires.
 *
 * `===` for everything, then `NaN` against itself. `+0 === -0` is
 * already true, so the second departure needs no code — it needs
 * `Object.is` **not** to be used, which is what this comment records.
 */
function sameValue(a: unknown, b: unknown): boolean {
  if (a === b) {
    return true;
  }
  return (
    typeof a === "number" && typeof b === "number" && Number.isNaN(a) && Number.isNaN(b)
  );
}

/**
 * Two values of a named type, compared through its descriptor.
 *
 * The descriptor's `kind` decides how, because the TypeScript surface of
 * a named IDL type is not always an object: an enumeration is a string
 * literal, an alias is whatever it aliases, and only a struct, an
 * exception, a bitset or a union is a property bag.
 */
function equalRef(a: unknown, b: unknown, name: string, depth: number): boolean {
  const descriptor = lookupType(name);
  if (descriptor === undefined) {
    throw new TypeError(`equalKey: no descriptor is registered for \`${name}\``);
  }
  switch (descriptor.kind) {
    // A string-literal union or a number at the TypeScript surface.
    case "enum":
    case "bitmask":
      return sameValue(a, b);

    // One member named `value`, carrying the aliased type.
    case "alias": {
      const aliased = descriptor.fields[0];
      if (aliased === undefined) {
        throw new TypeError(`equalKey: the alias \`${name}\` describes no type`);
      }
      return equalKeyAt(a, b, aliased.type, depth + 1);
    }

    case "union":
      return equalUnion(a, b, descriptor, depth);

    case "struct":
    case "exception":
    case "bitset":
      return equalMembers(a, b, descriptor.fields, depth);

    // An interface has no data form, so no value of one can be a key.
    case "interface":
      throw new TypeError(`equalKey: \`${name}\` is an interface and has no value`);
  }
}

/**
 * Two property bags, compared over the members a descriptor lists.
 *
 * A member absent from both is equal — that is what `@optional` means —
 * and a member absent from one only is not. Comparing with `in` rather
 * than against `undefined` keeps those two apart: a member explicitly
 * set to `undefined` is present, and a `@optional` member that is not
 * set is not.
 */
function equalMembers(
  a: unknown,
  b: unknown,
  members: ReadonlyArray<DdsMemberDescriptor>,
  depth: number,
): boolean {
  if (!isRecord(a) || !isRecord(b)) {
    return false;
  }
  for (const member of members) {
    const inA = member.name in a;
    const inB = member.name in b;
    if (inA !== inB) {
      return false;
    }
    if (!inA) {
      continue;
    }
    if (!equalKeyAt(a[member.name], b[member.name], member.type, depth + 1)) {
      return false;
    }
  }
  return true;
}

/**
 * Two union values: the discriminator first, then the selected case.
 *
 * The discriminator's **property name** comes out of the descriptor
 * rather than being written in here. It is the member carrying the
 * synthetic id `0xFFFFFFFF` (§7.5.1), and reading it from the side-table
 * is what keeps this function agreeing with whatever the generator
 * spelled it.
 */
function equalUnion(
  a: unknown,
  b: unknown,
  descriptor: DdsTypeDescriptor,
  depth: number,
): boolean {
  const discriminator = descriptor.fields.find((field) => field.id === DISCRIMINATOR_ID);
  if (discriminator === undefined) {
    throw new TypeError(
      `equalKey: the union \`${descriptor.name}\` has no discriminator member`,
    );
  }
  if (!isRecord(a) || !isRecord(b)) {
    return false;
  }
  if (!sameValue(a[discriminator.name], b[discriminator.name])) {
    return false;
  }
  // Whichever case the discriminator selected is the one present; the
  // others are absent in both and `equalMembers` passes over them.
  const cases = descriptor.fields.filter((field) => field.id !== DISCRIMINATOR_ID);
  return equalMembers(a, b, cases, depth);
}

/**
 * Two boxed `any` values: the identity first, then the payload.
 *
 * The payload is compared through the descriptor `typeId` names, which
 * is the whole reason the box carries one.
 */
function equalBoxed(a: unknown, b: unknown, depth: number): boolean {
  if (!isRecord(a) || !isRecord(b)) {
    return false;
  }
  const typeId = a["typeId"];
  if (typeof typeId !== "string" || typeId !== b["typeId"]) {
    return false;
  }
  return equalRef(a["value"], b["value"], typeId, depth + 1);
}

/** Whether a value can be indexed by a member name. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/**
 * Whether two descriptors describe the same type.
 *
 * DDS-TS 1.0 §8.7 (`registerType`). Deep value equality over every
 * field, with `typeGuard` compared by reference identity — a function is
 * not deep-equals comparable, and two closures that behave identically
 * are still two closures.
 *
 * What this is for: a module imported twice in one bundle runs its
 * `registerType` call twice with the same descriptor object, and that
 * has to be silent. Two different IDL types compiled to one qualified
 * name is the case that must not be, and it is the one this separates
 * out.
 */
export function descriptorsEqual(a: DdsTypeDescriptor, b: DdsTypeDescriptor): boolean {
  if (a === b) {
    return true;
  }
  if (a.typeGuard !== b.typeGuard) {
    return false;
  }
  return deepEqual(withoutGuard(a), withoutGuard(b), 0);
}

/** A descriptor's data, without the one member that is not data. */
function withoutGuard(descriptor: DdsTypeDescriptor): Record<string, unknown> {
  const { typeGuard: _guard, ...data } = descriptor;
  return data as Record<string, unknown>;
}

/**
 * Deep value equality over the JSON-shaped half of a descriptor.
 *
 * Arrays, plain objects, and the primitives — plus `Uint8Array`, which a
 * `@default` on a `long double` member carries and which is an object
 * whose `===` is reference equality.
 *
 * A property present with the value `undefined` and a property that is
 * absent compare equal here: `exactOptionalPropertyTypes` keeps the two
 * apart in the type system, and a generator that wrote `topic: undefined`
 * where another omitted `topic` did not emit a different type.
 *
 * zerodds-lint: recursion-depth 64 (descriptor nesting; bounded by `MAX_DEPTH`)
 */
function deepEqual(a: unknown, b: unknown, depth: number): boolean {
  if (depth > MAX_DEPTH) {
    throw new TypeError(`registerType: the descriptor nests past ${MAX_DEPTH}`);
  }
  if (sameValue(a, b)) {
    return true;
  }
  if (a instanceof Uint8Array && b instanceof Uint8Array) {
    return a.length === b.length && a.every((octet, at) => octet === b[at]);
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    return (
      a.length === b.length &&
      a.every((element, at) => deepEqual(element, b[at], depth + 1))
    );
  }
  if (isRecord(a) && isRecord(b) && !Array.isArray(a) && !Array.isArray(b)) {
    const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
    for (const key of keys) {
      if (!deepEqual(a[key], b[key], depth + 1)) {
        return false;
      }
    }
    return true;
  }
  return false;
}
