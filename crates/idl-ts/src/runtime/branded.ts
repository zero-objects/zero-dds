// SPDX-License-Identifier: Apache-2.0
// DDS-TS 1.0 — Branded primitive carriers.

/**
 * IDL `char` (ISO 8859-1 single character, code points U+0000..U+00FF).
 * Branded so it does not assignment-collide with plain `string`.
 */
export type Char = string & { readonly __dds_brand: "char" };

/**
 * IDL `wchar` (full Unicode scalar, U+0000..U+10FFFF excluding
 * the surrogate range U+D800..U+DFFF).
 */
export type WChar = string & { readonly __dds_brand: "wchar" };

/**
 * Validates and tags a single ISO 8859-1 character.
 *
 * Throws `RangeError` if the input is not exactly one UTF-16
 * code unit or its single code point exceeds U+00FF.
 */
export function makeChar(s: string): Char {
  if (s.length !== 1) {
    throw new RangeError("Char requires single ISO 8859-1 code point");
  }
  const cp = s.codePointAt(0);
  if (cp === undefined || cp > 0xff) {
    throw new RangeError("Char requires single ISO 8859-1 code point");
  }
  return s as Char;
}

/**
 * Validates and tags a single Unicode scalar.
 *
 * The check is performed by code-point count, not by code-unit
 * count, so astral-plane characters (emoji, supplementary CJK)
 * are accepted as exactly one scalar even though they occupy
 * two UTF-16 code units. Unpaired surrogates are rejected.
 */
export function makeWChar(s: string): WChar {
  // String iterator yields code points, not code units.
  const cps = Array.from(s);
  if (cps.length !== 1) {
    throw new RangeError("WChar requires exactly one Unicode scalar");
  }
  const cp = s.codePointAt(0);
  if (cp === undefined) {
    throw new RangeError("WChar requires exactly one Unicode scalar");
  }
  if (cp >= 0xd800 && cp <= 0xdfff) {
    throw new RangeError("WChar must not be an unpaired surrogate");
  }
  return s as WChar;
}

/**
 * Opaque carrier for IDL `long double`.
 *
 * The `bytes` array holds 16 octets of IEEE 754 binary128 data.
 * Byte order within `bytes` is implementation-defined and is not
 * part of the normative surface; cross-vendor interoperability is
 * achieved only through XCDR2 wire encoding.
 */
export interface LongDouble {
  readonly __dds_brand: "long_double";
  readonly bytes:       Uint8Array;
}

/**
 * Constructs a LongDouble carrier. Throws RangeError when
 * `bytes.length !== 16`.
 */
export function makeLongDouble(bytes: Uint8Array): LongDouble {
  if (bytes.length !== 16) {
    throw new RangeError("LongDouble requires 16 bytes");
  }
  return { __dds_brand: "long_double", bytes };
}
