// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * IEEE 754 binary128 — the sixteen octets CDR gives `long double`.
 *
 * The TypeScript counterpart of `crates/float128`,
 * `runtime/c/src/float128.c`, `runtime/cpp`'s and
 * `runtime/java/.../Float128.java`: a sixth implementation of the same
 * clauses and not a binding to any of them. CORBA 3.3 part 2 §9.3.1.3
 * with Figure 9.2 gives the layout — one sign bit, a fifteen-bit
 * exponent biased by 16383, a 112-bit stored fraction — and DDS-XTypes
 * 1.3 Table 31 names it: quadruple precision, sixteen octets, aligned to
 * eight.
 *
 * ## Why the decimal is parsed here and not by the generator
 *
 * A `number` is a binary64 and drops sixty bits of a binary128's
 * significand, so a generated file that carried the value as a `number`
 * would have lost it before any octet was written. The decimal string
 * from the corpus reaches this file intact and is converted **exactly**:
 * `BigInt` is arbitrary precision, so `M * 10^D` is a rational this
 * computes with rather than approximates, and the single rounding is the
 * one IEEE 754 §4.3.1 requires — to nearest, ties to even.
 *
 * A converter in the generator would be the Rust column answering for
 * this one: the same argument `runtime/cpp/tests/corpus_bytes.cpp`
 * makes, and the reason five columns each parse the decimal themselves.
 */

/** Bits in the stored fraction. IEEE 754-2008 clause 3.6. */
const FRACTION_BITS = 112n;

/** What is added to the unbiased exponent to get the stored one. */
const EXPONENT_BIAS = 16383;

/** The stored exponent of an infinity or a NaN. */
const EXPONENT_MAX = 0x7fff;

/** The unbiased exponent of the smallest normal value. */
const MIN_NORMAL_EXPONENT = -16382;

/** `2^112`, the implied leading one in its stored position. */
const IMPLIED_ONE = 1n << FRACTION_BITS;

/**
 * The sixteen octets a decimal string is, least-significant first.
 *
 * Least-significant first because that is the order a little-endian
 * stream carries them and the order `Float128` is documented in;
 * `Writer.writeF128` reverses the whole sixteen for a big-endian stream,
 * which is CORBA Figure 9.2's rule and not two halves swapped in place.
 *
 * Accepts what the corpus writes: an optional sign, digits with an
 * optional point, an optional `e±nn`, and the three names a decimal
 * cannot spell — `inf`, `-inf`, `nan`. A negative zero keeps its sign,
 * which is the one thing a value's magnitude cannot carry and is a
 * different sixteen octets.
 */
export function float128FromDecimalString(text: string): Uint8Array {
  const trimmed = text.trim();
  const lowered = trimmed.toLowerCase();
  if (lowered === "inf" || lowered === "+inf" || lowered === "infinity") {
    return octetsOf(false, EXPONENT_MAX, 0n);
  }
  if (lowered === "-inf" || lowered === "-infinity") {
    return octetsOf(true, EXPONENT_MAX, 0n);
  }
  if (lowered === "nan" || lowered === "-nan") {
    // A quiet NaN with no payload: the leading fraction bit set, which
    // is IEEE 754 §6.2.1's quiet bit.
    return octetsOf(lowered.startsWith("-"), EXPONENT_MAX, 1n << (FRACTION_BITS - 1n));
  }

  const parsed = decimalParts(trimmed);
  if (parsed === undefined) {
    throw new RangeError(`a decimal, not \`${text}\``);
  }
  const { negative, digits, exponent } = parsed;
  if (digits === 0n) {
    return octetsOf(negative, 0, 0n);
  }

  // The value as an exact fraction. Nothing is rounded until the
  // significand is formed, which is what makes the result correctly
  // rounded rather than rounded twice.
  let numerator = digits;
  let denominator = 1n;
  if (exponent >= 0) {
    numerator *= 10n ** BigInt(exponent);
  } else {
    denominator = 10n ** BigInt(-exponent);
  }

  // The unbiased exponent: the largest `e` with `2^e <= value`.
  let unbiased = bitLength(numerator) - bitLength(denominator);
  while (!atLeastPowerOfTwo(numerator, denominator, unbiased)) {
    unbiased -= 1;
  }
  while (atLeastPowerOfTwo(numerator, denominator, unbiased + 1)) {
    unbiased += 1;
  }

  // Below the smallest normal the exponent is fixed and the significand
  // is what loses bits, which is what a subnormal is. Clamping here
  // rather than branching keeps one rounding for both cases: at the
  // boundary the rounded significand comes out as exactly `2^112`,
  // which is the smallest normal and is read back as one below.
  let scale = Math.max(unbiased, MIN_NORMAL_EXPONENT);
  let significand = roundedShiftedQuotient(numerator, denominator, 112 - scale);
  if (significand >= IMPLIED_ONE << 1n) {
    // Rounding carried past the top bit — `0x1.ffff…f` became `2`. One
    // more exponent and one more rounding, which cannot carry again.
    scale += 1;
    significand = roundedShiftedQuotient(numerator, denominator, 112 - scale);
  }

  if (significand < IMPLIED_ONE) {
    return octetsOf(negative, 0, significand);
  }
  const biased = scale + EXPONENT_BIAS;
  if (biased >= EXPONENT_MAX) {
    // Past the largest finite value. An infinity and not a saturated
    // significand: IEEE 754 §7.4 says an overflow rounds to infinity
    // under round-to-nearest, and the largest finite value would be a
    // number the caller did not write.
    return octetsOf(negative, EXPONENT_MAX, 0n);
  }
  return octetsOf(negative, biased, significand - IMPLIED_ONE);
}

/** The three parts of a decimal, or `undefined` where there are not three. */
function decimalParts(
  text: string,
): { negative: boolean; digits: bigint; exponent: number } | undefined {
  const match = /^([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/.exec(text);
  if (match === null) {
    return undefined;
  }
  const whole = match[2] ?? "";
  const fraction = match[3] ?? "";
  if (whole === "" && fraction === "") {
    return undefined;
  }
  const exponent = Number(match[4] ?? "0") - fraction.length;
  return {
    negative: match[1] === "-",
    digits: BigInt(`${whole}${fraction}` === "" ? "0" : `${whole}${fraction}`),
    exponent,
  };
}

/** Whether `numerator / denominator >= 2^power`. */
function atLeastPowerOfTwo(numerator: bigint, denominator: bigint, power: number): boolean {
  if (power >= 0) {
    return numerator >= denominator << BigInt(power);
  }
  return numerator << BigInt(-power) >= denominator;
}

/**
 * `round(numerator * 2^shift / denominator)`, to nearest with ties to
 * even.
 *
 * The one rounding in the whole conversion. Doing it on the exact
 * rational is what makes it correct: rounding the decimal to a `number`
 * first and then to a binary128 rounds twice, and two roundings differ
 * from one in the last place often enough that the corpus's
 * `needs_all_of_them` is exactly the case that catches it.
 */
function roundedShiftedQuotient(numerator: bigint, denominator: bigint, shift: number): bigint {
  const top = shift >= 0 ? numerator << BigInt(shift) : numerator;
  const bottom = shift >= 0 ? denominator : denominator << BigInt(-shift);
  const quotient = top / bottom;
  const remainder = top % bottom;
  const twice = remainder * 2n;
  if (twice > bottom || (twice === bottom && (quotient & 1n) === 1n)) {
    return quotient + 1n;
  }
  return quotient;
}

/** How many bits a positive `BigInt` occupies. */
function bitLength(value: bigint): number {
  return value === 0n ? 0 : value.toString(2).length;
}

/** The sixteen octets of one sign, biased exponent and stored fraction. */
function octetsOf(negative: boolean, biasedExponent: number, fraction: bigint): Uint8Array {
  let bits = fraction | (BigInt(biasedExponent) << FRACTION_BITS);
  if (negative) {
    bits |= 1n << 127n;
  }
  const octets = new Uint8Array(16);
  for (let index = 0; index < 16; index += 1) {
    octets[index] = Number((bits >> BigInt(index * 8)) & 0xffn);
  }
  return octets;
}
