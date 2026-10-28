// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The IDL types TypeScript has no structural form for.
 *
 * DDS-TS 1.0 §7.3.2, §7.3.3, §8.5 and §8.6. Three of them:
 * a `char` holding exactly one ISO 8859-1 character, a `wchar` holding
 * exactly one Unicode scalar, and a `long double` holding sixteen octets
 * of binary128.
 *
 * # Why the brands are string literals
 *
 * A brand made from a module-local `unique symbol` is nominal across
 * module boundaries: a `Char` produced by one vendor's runtime would not
 * satisfy another vendor's `Char` parameter even though both hold the
 * same character. The tags below — `"char"`, `"wchar"`, `"long_double"`
 * — and the property name `__dds_brand` are normative for exactly that
 * reason, and two conformant runtimes produce assignment-compatible
 * types without importing each other.
 *
 * The brand is a compile-time marker with no run-time effect: a `Char`
 * *is* a string at run time. What the factories below add is the check
 * the type cannot make.
 */

/**
 * An IDL `char`: one ISO 8859-1 character.
 *
 * DDS-TS 1.0 §7.3.2. Construct with {@link makeChar}.
 */
export type Char = string & { readonly __dds_brand: "char" };

/**
 * An IDL `wchar`: one Unicode scalar.
 *
 * DDS-TS 1.0 §7.3.2. Construct with {@link makeWChar}.
 */
export type WChar = string & { readonly __dds_brand: "wchar" };

/**
 * One ISO 8859-1 character, or a `RangeError`.
 *
 * OMG IDL 4.2 §7.2.6.2 makes `char` an 8-bit quantity holding a single
 * ISO 8859-1 character, so the value space is `U+0000`..`U+00FF` and
 * nothing above it. A string of length one whose code point is
 * `U+20AC` (the euro sign) is a perfectly good JavaScript string and is
 * not a `char`: it needs three octets in UTF-8 and there is no octet
 * that means it.
 *
 * The length check is on UTF-16 code units, which for this range is the
 * same as on code points — every scalar at or below `U+00FF` is one code
 * unit. {@link makeWChar} cannot use the same check, and that difference
 * is the whole reason these are two functions.
 */
export function makeChar(s: string): Char {
  const first = s.codePointAt(0);
  if (s.length !== 1 || first === undefined || first > 0xff) {
    throw new RangeError("Char requires single ISO 8859-1 code point");
  }
  return s as Char;
}

/**
 * One Unicode scalar, or a `RangeError`.
 *
 * OMG IDL 4.2 §7.2.6.2 makes `wchar` a full Unicode scalar:
 * `U+0000`..`U+10FFFF` less the surrogate range
 * `U+D800`..`U+DFFF`, which is not a scalar value but half of one.
 *
 * A JavaScript string is a sequence of UTF-16 **code units**, so every
 * scalar above the BMP is two of them: `"\u{1F600}".length` is 2. A
 * length check on `s.length === 1` would therefore reject every
 * astral-plane character, which is why this counts code points — the
 * string iterator yields those — and not units. DDS-TS 1.0 §7.3.2 states
 * that as a MUST NOT.
 */
export function makeWChar(s: string): WChar {
  // The iterator yields code points, so a surrogate pair counts once.
  const points = [...s];
  if (points.length !== 1) {
    throw new RangeError("WChar requires exactly one Unicode scalar");
  }
  const point = s.codePointAt(0);
  if (point === undefined || (point >= 0xd800 && point <= 0xdfff)) {
    throw new RangeError("WChar must not be an unpaired surrogate");
  }
  return s as WChar;
}

/**
 * An IDL `long double`, as the sixteen octets it is.
 *
 * DDS-TS 1.0 §7.3.3 and §8.6. A TypeScript `number` is an IEEE 754
 * binary64 (ECMA-262 §6.1.6.1) and drops sixty bits of a binary128's
 * significand, so a `number`-typed member would have lost the value
 * before any octet was written. The octets are the one form that carries
 * it without deciding what it means.
 *
 * The byte order *within* `bytes` is implementation-defined; what
 * crosses a DDS boundary is governed by OMG X-Types 1.3 §7.4.3 and is
 * decided by the codec, not here. `runtime/typescript/src/float128.ts`
 * holds the conversion from a decimal string to those sixteen octets,
 * least-significant first.
 */
export interface LongDouble {
  readonly __dds_brand: "long_double";
  /** Sixteen octets. Any other length is a `RangeError`. */
  readonly bytes: Uint8Array;
}

/**
 * Sixteen octets as a {@link LongDouble}, or a `RangeError`.
 *
 * The length is the whole check: a binary128 has no invalid bit pattern
 * — every one of the 2^128 is a number, an infinity or a NaN — so a
 * factory that inspected the exponent would refuse values IEEE 754
 * defines.
 */
export function makeLongDouble(bytes: Uint8Array): LongDouble {
  if (bytes.length !== 16) {
    throw new RangeError("LongDouble requires 16 bytes");
  }
  return { __dds_brand: "long_double", bytes };
}
