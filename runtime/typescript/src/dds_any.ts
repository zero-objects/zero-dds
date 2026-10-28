// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The boxed IDL `any`, and the two functions that open and close it.
 *
 * DDS-TS 1.0 §7.3.4 and §8.4. An IDL `any` carries a value together with
 * an explicit run-time type identity (OMG IDL 4.2 §7.4.14). TypeScript
 * `interface` declarations have none — they are erased — so an unwrapped
 * payload gives a consumer nothing to discover the type from. The
 * mapping therefore boxes: `{ typeId, value }`, where `typeId` is the
 * qualified IDL name and indexes the registry.
 *
 * Mapping `any` to TypeScript `any` or `unknown` is forbidden by the
 * spec, and the reason is the same one: `unknown` says the type is not
 * known, where the IDL construct says the type is carried.
 *
 * Both functions check the descriptor's type guard, and that check is
 * what makes the box worth having. A producer that boxed the wrong value
 * and a consumer that read a box it did not expect fail here, naming the
 * two type ids, rather than at whatever line first reads a property that
 * is not there.
 */

import type { DdsAny, DdsTypeDescriptor } from "./types.js";

/**
 * Box a value with the identity of its type.
 *
 * DDS-TS 1.0 §8.7 (`boxAny`). The guard is checked first: a box whose
 * `typeId` does not describe its `value` is worse than no box, because
 * every consumer downstream trusts it.
 *
 * @throws TypeError when `descriptor.typeGuard(value)` is false.
 */
export function boxAny<T>(descriptor: DdsTypeDescriptor<T>, value: T): DdsAny {
  if (!descriptor.typeGuard(value)) {
    throw new TypeError(`boxAny: the value is not a \`${descriptor.name}\``);
  }
  return { typeId: descriptor.name, value };
}

/**
 * Narrow a box to the type a descriptor describes.
 *
 * DDS-TS 1.0 §8.7 (`unboxAny`). Two conditions, both required: the
 * box's `typeId` is the descriptor's name, and the descriptor's guard
 * accepts the payload. The second is not redundant — a box built by hand
 * or arriving from another process carries whatever `typeId` its
 * producer wrote.
 *
 * The spec spells the first parameter `any`; it is named `boxed` here
 * because the identifier `any` beside a type annotation reads as the
 * TypeScript keyword. The shape is unchanged.
 *
 * @throws TypeError when either condition fails, naming which.
 */
export function unboxAny<T>(boxed: DdsAny, descriptor: DdsTypeDescriptor<T>): T {
  if (boxed.typeId !== descriptor.name) {
    throw new TypeError(
      `unboxAny: the box holds a \`${boxed.typeId}\`, not a \`${descriptor.name}\``,
    );
  }
  if (!descriptor.typeGuard(boxed.value)) {
    throw new TypeError(
      `unboxAny: the box says \`${descriptor.name}\` and the value is not one`,
    );
  }
  return boxed.value;
}
