// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The reflection half of the descriptor runtime.
 *
 * DDS-TS 1.0 §8.7. A registry keyed by qualified IDL name, and the five
 * functions that read a descriptor: the key members out of a sample, the
 * topic name off the type, the defaults into a partial value, and — in
 * `./dds_any.js` and `./equal.js` — the box and the key comparison.
 *
 * # Why there is a registry at all
 *
 * Because `DdsTypeRef.kind === "ref"` carries a **name** and not a
 * descriptor (§8.2). That is what makes a descriptor graph acyclic and
 * therefore JSON-roundtrippable, and it is what lets two structs refer to
 * each other when neither can be initialised first. The cost is that a
 * consumer walking the graph has to resolve those names somewhere, and
 * this is the somewhere: every generated module ends with
 * `registerType(<Type>Type)`, and {@link lookupType} answers.
 */

import { descriptorsEqual } from "./equal.js";
import type { DdsTypeDescriptor, GuardedTypeOf } from "./types.js";

/**
 * Every descriptor registered so far, by qualified IDL name.
 *
 * One map for the module graph. Two bundles that each carry their own
 * copy of this runtime have two registries and each sees its own
 * registrations, which §8.7 names as out of scope for the equivalence
 * check.
 */
const registry = new Map<string, DdsTypeDescriptor>();

/**
 * Add a descriptor to the registry.
 *
 * DDS-TS 1.0 §8.7. Registering the same type twice is silent — a module
 * imported from two places runs its registration twice and that is not
 * an error. Registering a **different** type under a name already taken
 * throws, and it has to: the second registration would otherwise decide
 * what every `{ kind: "ref" }` to that name resolves to, and every
 * consumer downstream would read one type's samples through another
 * type's members with nothing reporting it.
 *
 * @throws TypeError when the name is taken by a structurally different
 * descriptor.
 */
export function registerType(d: DdsTypeDescriptor): void {
  const seen = registry.get(d.name);
  if (seen === undefined) {
    registry.set(d.name, d);
    return;
  }
  if (descriptorsEqual(seen, d)) {
    return;
  }
  throw new TypeError(
    `registerType: \`${d.name}\` is already registered as a different type`,
  );
}

/**
 * The descriptor registered under a qualified IDL name, or `undefined`.
 *
 * DDS-TS 1.0 §8.7. `undefined` and not a throw: a consumer that reached
 * a `ref` to a type whose module was never imported is asking a question
 * this can answer, and the caller decides whether that is a failure.
 */
export function lookupType(qualifiedName: string): DdsTypeDescriptor | undefined {
  return registry.get(qualifiedName);
}

/**
 * The values of a sample's key members, in declaration order.
 *
 * DDS-TS 1.0 §8.7. Declaration order is the order the descriptor lists
 * them, which is the order the IDL declared them and the order X-Types
 * 1.3 §7.6.8 hashes them in — so the array can be fed to a key hash
 * without being sorted first.
 *
 * The sample's shape is **not** checked. A caller that does not trust
 * where the sample came from runs `descriptor.typeGuard` first; one that
 * just decoded it through the generated codec already knows.
 *
 * A type with no `@key` member returns `[]`, which is the whole of the
 * DDS single-instance case and not an error.
 */
export function getKey<T>(
  sample: T,
  descriptor: DdsTypeDescriptor<T>,
): ReadonlyArray<unknown> {
  const bag = sample as unknown as Record<string, unknown>;
  const out: unknown[] = [];
  for (const member of descriptor.fields) {
    if (member.base === true && member.type.kind === "ref") {
      // The keys of the base come first, as they are written first (§7.4.1.1.5.1).
      const base = lookupType(member.type.name);
      const held = bag[member.name];
      if (base !== undefined && held !== undefined) {
        out.push(...getKey(held, base));
      }
      continue;
    }
    if (member.key) {
      out.push(bag[member.name]);
    }
  }
  return out;
}

/**
 * The topic name the type is bound to, or `undefined`.
 *
 * DDS-TS 1.0 §8.7. `descriptor.topic` unchanged — a function rather than
 * a property access so that a consumer holding only the reflection API
 * has one, and so that the `@topic` binding has a name to be looked up
 * by.
 */
export function getTopic<T>(descriptor: DdsTypeDescriptor<T>): string | undefined {
  return descriptor.topic;
}

/**
 * A partial value, completed from the descriptor's `@default` values.
 *
 * DDS-TS 1.0 §8.7. Every member the descriptor records a
 * non-`undefined` `default` for and `partial` does not carry is filled
 * from that default. A member with neither stays **absent** — not
 * `undefined`, absent — because a `@optional` member that is present and
 * undefined is a different sample from one that is not there at all, and
 * under XCDR2 the two are different octets.
 */
export function withDefaults<T>(descriptor: DdsTypeDescriptor<T>, partial: Partial<T>): T {
  const out: Record<string, unknown> = { ...(partial as Record<string, unknown>) };
  for (const member of descriptor.fields) {
    if (member.base === true && member.type.kind === "ref") {
      const base = lookupType(member.type.name);
      if (base !== undefined) {
        const held = (out[member.name] ?? {}) as Record<string, unknown>;
        out[member.name] = withDefaults(base, held);
      }
      continue;
    }
    if (member.default === undefined || member.name in out) {
      continue;
    }
    out[member.name] = member.default;
  }
  return out as T;
}

/**
 * Whether a rejection carries one of a set of IDL exceptions.
 *
 * DDS-TS 1.0 §8.7. A generated Operations-Profile client rejects with an
 * `Error` whose `cause` is the decoded IDL exception payload, so a
 * catch-handler asks this and gets the union of the listed types back as
 * a narrowing:
 *
 * ```ts
 * catch (err) {
 *   if (isOneOf(err, [NotFoundType, DeniedType])) {
 *     err.cause; // NotFound | Denied
 *   }
 * }
 * ```
 *
 * Never throws. A `catch` block is the one place where a helper that
 * threw would replace the error being handled with its own, so anything
 * that is not an `Error` — a thrown string, a rejected `undefined` — is
 * simply `false`.
 */
export function isOneOf<DS extends ReadonlyArray<DdsTypeDescriptor>>(
  err: unknown,
  descriptors: DS,
): err is Error & { cause: GuardedTypeOf<DS[number]> } {
  if (!(err instanceof Error)) {
    return false;
  }
  const cause: unknown = err.cause;
  return descriptors.some((descriptor) => descriptor.typeGuard(cause));
}
