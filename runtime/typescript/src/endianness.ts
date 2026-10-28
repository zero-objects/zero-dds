// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * Byte order.
 *
 * The counterpart of `crates/wire/src/endianness.rs` and of
 * `runtime/python/zerodds_wire/endianness.py`. A module of its own
 * because the writer and the reader both need it and neither should
 * own it.
 */

/** Which end of a multi-byte value goes out first. */
export enum Endianness {
  Little = "le",
  Big = "be",
}

/** Whether the low octet goes out first. */
export function isLittle(order: Endianness): boolean {
  return order === Endianness.Little;
}

/**
 * The command line's spelling, or `undefined` for anything else.
 *
 * `undefined` and not a default: a typo in a wire-affecting flag has to
 * be loud, because the octets it produces look plausible.
 */
export function parseEndianness(name: string): Endianness | undefined {
  if (name === "le" || name === "little") {
    return Endianness.Little;
  }
  if (name === "be" || name === "big") {
    return Endianness.Big;
  }
  return undefined;
}
