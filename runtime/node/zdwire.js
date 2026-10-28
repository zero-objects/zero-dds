// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The ZeroDDS Node wire runtime: DDS-XTypes 1.3 §7.4, implemented from
 * the clause.
 *
 * A further independent reading beside `crates/wire`, `runtime/c`,
 * `runtime/cpp`, `runtime/java`, `runtime/python`, `runtime/zig` and
 * `runtime/typescript`. None of them calls another — that is what makes
 * a difference between two columns a finding about the specification
 * and not a shared bug.
 *
 * This file is deliberately a close port of `runtime/typescript/src`,
 * stripped of its type annotations, and not a fresh reading from the
 * clause: the value model plain JavaScript needs is the one TypeScript
 * already answered — `number` is a binary64, so every integer up to
 * `i32` is one and everything wider is a `bigint`; a `char` and a
 * `wchar` are held as a one-character `string`; a `long double` never
 * becomes a `number`, because a binary64 would drop sixty bits of its
 * significand. Plain JavaScript has nothing TypeScript's runtime does
 * not already carry at the value level, so re-deriving it from Zig's
 * shape — `char` as a code-point integer, `@external` as a pointer —
 * would be answering a question this language does not ask.
 *
 * Two things plain JavaScript does differently from TypeScript, both
 * because there is no compiler holding a `number` and a `bigint` apart
 * here:
 *
 * - **A range check happens at the point of every write**, exactly as
 *   in TypeScript, and for the same reason: nothing upstream has
 *   already refused `70000` for an `i16`.
 * - **The three holder coercions — {@link zdNumber}, {@link zdBigInt},
 *   {@link zdChar} — exist for symmetry with the generated code and not
 *   because `as` would otherwise compile to the wrong thing.** Plain
 *   JavaScript has no `as` and no narrowing to refuse; `Number(x)` and
 *   `BigInt(x)` already do the conversion. They stay because a `char`
 *   held as a `string` needs the code-point reading regardless, and
 *   `crates/lang-rules/src/emit/node.rs`'s `cast` calls all three by
 *   name so the emitted call and this runtime's export agree on a
 *   spelling.
 *
 * ## What this module exports
 *
 * One `require("./zdwire.js")` worth of names: {@link Writer},
 * {@link Reader}, the policy tables of §"policies", the errors of
 * §"errors", the byte-order helpers, and {@link float128FromDecimalString}
 * with its inverse {@link float128ToDecimalString} — the one operation
 * `runtime/typescript` does not need, because nothing in that column's
 * own generator reads a `long double` back into JSON; this one does; see
 * `tools/wireprobe/wireprobe-zerodds-node-gen/src/dump.js`.
 */

"use strict";

// -- byte order -----------------------------------------------------------

/** Which end of a multi-byte value goes out first. */
const Endianness = Object.freeze({
  Little: "le",
  Big: "be",
});

/** Whether the low octet goes out first. */
function isLittle(order) {
  return order === Endianness.Little;
}

/**
 * The command line's spelling, or `undefined` for anything else.
 *
 * `undefined` and not a default: a typo in a wire-affecting flag has to
 * be loud, because the octets it produces look plausible.
 */
function parseEndianness(name) {
  if (name === "le" || name === "little") {
    return Endianness.Little;
  }
  if (name === "be" || name === "big") {
    return Endianness.Big;
  }
  return undefined;
}

// -- errors -----------------------------------------------------------------

/** Base of every wire failure. */
class WireError extends Error {
  constructor(message) {
    super(message);
    this.name = new.target.name;
  }
}

/** The stream ended before the value did. */
class Truncated extends WireError {
  constructor(at, needed, available) {
    super(`at ${at}: ${needed} bytes needed, ${available} available`);
    this.at = at;
    this.needed = needed;
    this.available = available;
  }
}

/**
 * A length field claims more than the stream holds.
 *
 * Separate from {@link Truncated}: a length that cannot be true is a
 * malformed stream, not a short one, and allocating for it is how a
 * reader turns a bad message into an out-of-memory.
 */
class ImpossibleLength extends WireError {
  constructor(at, claimed, available) {
    super(`at ${at}: a length of ${claimed} with ${available} bytes left`);
    this.at = at;
    this.claimed = claimed;
    this.available = available;
  }
}

/** A string held octets that are not UTF-8. */
class NotUtf8 extends WireError {
  constructor(at) {
    super(`at ${at}: the string is not UTF-8`);
    this.at = at;
  }
}

/**
 * A value outside what its type admits.
 *
 * Thrown by the writer as well as the reader: an integer here is a
 * `number` or a `bigint` holding anything its holder can hold, and the
 * check happens at the point of the write or the read rather than at a
 * compiler's refusal.
 */
class OutOfRange extends WireError {
  constructor(at, expected) {
    super(`at ${at}: expected ${expected}`);
    this.at = at;
    this.expected = expected;
  }
}

// -- policies ---------------------------------------------------------------
//
// The wire policies a layout plan cannot answer. Every one of these is a
// place where measured implementations differ and the clause leaves
// room, so the generated codec names the policy and this runtime carries
// it out. The names are `crates/ast-layout/src/key.rs`'s axes.

/** How many octets one `wchar` takes. */
const WcharWidth = Object.freeze({
  /** Two, which is Table 31 and seven of the nine measured columns. */
  Octets2: 2,
  /** Four, which is RTI's XCDR1, TAO, and rosidl's type support. */
  Octets4: 4,
});

/** What shape a `wchar` goes out in. */
const WcharFraming = Object.freeze({
  /** The code unit alone, at its width. */
  Fixed: "fixed",
  /**
   * CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 form: an octet count first,
   * then the code unit's octets, all aligned to one.
   */
  GiopLengthPrefixed: "giop-length-prefixed",
});

/** In what order a `wstring`'s code units land. */
const WstringUnitOrder = Object.freeze({
  /** The stream's, which the encapsulation identifier announced. */
  Stream: "stream",
  /** Big-endian whatever the stream says. */
  BigEndianAlways: "big-endian-always",
  /** Preceded by a byte order mark. */
  ByteOrderMark: "byte-order-mark",
  /** The host's, unswapped, which is what Fast CDR writes. */
  HostUnswapped: "host-unswapped",
});

/** What a `wstring`'s length field counts, and in what units. */
const WstringLengthPolicy = Object.freeze({
  /** §7.4.1.1.2: an octet count over UTF-16 units, no terminator. */
  Specification: "specification",
  /** A character count over two-octet units. */
  CharacterCount: "character-count",
  /** The same, with a terminator counted. */
  CharacterCountTerminated: "character-count-terminated",
  /** An octet count over four-octet units. */
  SpecificationWideUnits: "specification-wide-units",
  /** What `rosidl_typesupport_fastrtps` writes. */
  RosTypeSupport: "ros-type-support",
  /** An octet count with a two-octet terminator counted. */
  OctetsTerminated2: "octets-terminated-2",
  /** The same with a four-octet one. */
  OctetsTerminated4: "octets-terminated-4",
  /** A character count with a two-octet terminator counted. */
  CharactersTerminated2: "characters-terminated-2",
});

/** What a `fixed`'s digit count is taken from. */
const FixedDigitCount = Object.freeze({
  /** The IDL's `fixed<D, S>`. */
  Declared: "declared",
  /** The value's own precision (JacORB 3.9). */
  ValuePrecision: "value-precision",
  /**
   * The integer digits of the decimal as written, leading zeros
   * included, plus the S fraction digits, made odd by one leading zero: ACE/TAO 2.5.24's
   * `ACE_CDR::Fixed::from_string` through its own CDR operator (OpenDDS writes the declared count). `0.12345` under
   * `fixed<5,5>` is `0012345c`; past 31 digits it is refused. Read like
   * `ValuePrecision`.
   */
  ValueStringDigits: "value-string-digits",
});

/** What a PL_CDR1 parameter frames. */
const ParameterRole = Object.freeze({
  /** A member of the type. */
  Member: "member",
  /** An `@optional` member outside a `@mutable` type. */
  Optional: "optional",
});

/**
 * What §7.2.2.7 says to do with a member the stream cannot give.
 *
 * The reader's, and named here because the generated decoder passes it
 * down as a parameter.
 */
const TryConstruct = Object.freeze({
  /** Discard the whole sample. */
  Discard: "discard",
  /** Take Table 9's value. */
  UseDefault: "use-default",
  /** Keep what fits. §7.3.1.2.1.12.3. */
  Trim: "trim",
});

/** Why a reader stopped. */
const Status = Object.freeze({
  Ok: "ok",
  Truncated: "truncated",
  ImpossibleLength: "impossible-length",
  OutOfRange: "out-of-range",
  Discarded: "discarded",
});

/**
 * C1 of {@link WstringLengthPolicy} — whether the length prefix counts
 * characters and not octets.
 */
function wstringCountsCharacters(policy) {
  switch (policy) {
    case WstringLengthPolicy.CharacterCount:
    case WstringLengthPolicy.CharacterCountTerminated:
    case WstringLengthPolicy.RosTypeSupport:
    case WstringLengthPolicy.CharactersTerminated2:
      return true;
    default:
      return false;
  }
}

/** C2 — whether a terminating NUL code unit is written and counted. */
function wstringIsTerminated(policy) {
  switch (policy) {
    case WstringLengthPolicy.CharacterCountTerminated:
    case WstringLengthPolicy.OctetsTerminated2:
    case WstringLengthPolicy.OctetsTerminated4:
    case WstringLengthPolicy.CharactersTerminated2:
      return true;
    default:
      return false;
  }
}

/**
 * C3 — how many octets one of a `wstring`'s code units takes.
 *
 * **Not** the width of a scalar `wchar`, which is {@link WcharWidth}.
 */
function wstringUnitWidth(policy) {
  switch (policy) {
    case WstringLengthPolicy.CharacterCountTerminated:
    case WstringLengthPolicy.SpecificationWideUnits:
    case WstringLengthPolicy.RosTypeSupport:
    case WstringLengthPolicy.OctetsTerminated4:
      return 4;
    default:
      return 2;
  }
}

/**
 * Whether the units go out little-endian whatever the stream says.
 */
function wstringOrderIsFixedLittle(order) {
  return (
    order === WstringUnitOrder.ByteOrderMark || order === WstringUnitOrder.HostUnswapped
  );
}

/** The framing every field of which is the specification's reading. */
const PID_FLAGS_NONE = Object.freeze({ member: 0, sentinel: 0, extendedMarker: 0 });

/** The name `PidFlagPolicy::Bare` gives {@link PID_FLAGS_NONE}. */
const PID_FLAGS_BARE = PID_FLAGS_NONE;

/**
 * A bare member id, the must-understand bit on the sentinel and on the
 * extended marker. Cyclone DDS and RTI Connext 7.7. The default.
 */
const PID_FLAGS_SENTINEL_MUST_UNDERSTAND = Object.freeze({
  member: 0,
  sentinel: 0x4000,
  extendedMarker: 0x4000,
});

/**
 * Both flags on every member id whatever the member declared, a bare
 * sentinel, and both on the extended marker. OpenDDS.
 */
const PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS = Object.freeze({
  member: 0x4000 | 0x8000,
  sentinel: 0,
  extendedMarker: 0x4000 | 0x8000,
});

/** The flags a framing puts on a parameter in `role`. */
function pidFlagsAt(flags, role) {
  switch (role) {
    case "member":
      return flags.member;
    case "sentinel":
      return flags.sentinel;
    case "extended-marker":
      return flags.extendedMarker;
    default:
      return 0;
  }
}

/** The parameter id that introduces the extended form. §7.4.1.2.2. */
const PID_EXTENDED = 0x3f01;

/** The parameter id that ends a parameter list. §7.4.1.2.4. */
const PID_LIST_END = 0x3f02;

/**
 * `PID_IGNORE`. §7.4.1.2.2: "All consumers of this Data Representation
 * shall ignore this parameter" — with no exception for the
 * must-understand bit above it.
 */
const PID_IGNORE = 0x3f03;

/**
 * The highest member id a short parameter header can carry.
 *
 * §7.4.1.2.2 gives the range as 0 to 0x3F00 *inclusive*, and
 * PID_EXTENDED is one above it.
 */
const PID_SHORT_MAX_ID = 0x3f00;

/** The implementation-extension flag of a parameter id. */
const PID_IMPL_EXTENSION = 0x8000;

/** The must-understand flag of a parameter id. */
const PID_MUST_UNDERSTAND = 0x4000;

/** The low fourteen bits of a short parameter id, which are the id. */
const PID_ID_MASK = 0x3fff;

/** `FLAG_2` of an `eMemberHeader`. §7.4.1.2.1. */
const E_FLAG_MUST_UNDERSTAND = 0x4000_0000;

/** `FLAG_1` of an `eMemberHeader`: `FLAG_IMPL_EXTENSION`. */
const E_FLAG_IMPL_EXTENSION = 0x8000_0000;

/** The member id an `eMemberHeader` carries. §7.4.1.2.1. */
const E_MEMBER_ID_MASK = 0x0fff_ffff;

/** How many octets a KeyHash holder has. RTPS 2.5 §9.6.3.3. */
const KEY_HASH_LEN = 16;

/**
 * The longest value a short parameter header's `ssize` can declare.
 * §7.4.1.2.2's `ssize` is sixteen bits.
 */
const PID_SHORT_MAX_LEN = 0xffff;

/** The code unit a byte order mark is. Unicode 15 §23.8. */
const BYTE_ORDER_MARK = 0xfeff;

/** The byte-swapped mark. `U+FFFE` is not a character. */
const SWAPPED_BYTE_ORDER_MARK = 0xfffe;

/** The framing a `Writer` and a `Reader` use where the caller names none. */
const PL_CDR1_DEFAULT = Object.freeze({
  pidFlags: PID_FLAGS_NONE,
  shortMaxId: PID_SHORT_MAX_ID,
  padsDeclaredLength: false,
  padsOptionalLength: false,
  memberMustUnderstand: true,
  padAfterValue: true,
});

/** The DHEADER a `@final` aggregate does not write. */
function noDheader() {
  return { at: -1 };
}

// -- the three holder coercions ---------------------------------------------

/**
 * The same value in a `number`, whatever holder it arrived in.
 *
 * A one-character string answers with its code point, which is what the
 * narrowing means where the source is a `char`.
 */
function zdNumber(value) {
  if (typeof value === "string") {
    return value.codePointAt(0) ?? 0;
  }
  if (typeof value === "boolean") {
    return value ? 1 : 0;
  }
  return Number(value);
}

/** The same value in a `bigint`. See {@link zdNumber}. */
function zdBigInt(value) {
  if (typeof value === "string") {
    return BigInt(value.codePointAt(0) ?? 0);
  }
  if (typeof value === "boolean") {
    return value ? 1n : 0n;
  }
  return BigInt(value);
}

/**
 * The same value as a one-character `string`, which is what an IDL
 * `char` and `wchar` are held as here.
 */
function zdChar(value) {
  if (typeof value === "string") {
    return value;
  }
  return String.fromCodePoint(Number(value));
}

// -- long double: IEEE 754 binary128 -----------------------------------------
//
// CORBA 3.3 part 2 §9.3.1.3 with Figure 9.2 gives the layout — one sign
// bit, a fifteen-bit exponent biased by 16383, a 112-bit stored fraction
// — and DDS-XTypes 1.3 Table 31 names it: quadruple precision, sixteen
// octets, aligned to eight. `Writer.writeF128` and `Reader.readF128`
// traffic in the sixteen octets alone; a `number` never appears, because
// a binary64 would drop sixty bits of the significand at the assignment.

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
 * The sixteen octets a decimal string is, least-significant first — the
 * order a little-endian stream carries them and the order
 * {@link Writer.writeF128} takes; it reverses the whole sixteen for a
 * big-endian stream, which is CORBA Figure 9.2's rule and not two halves
 * swapped in place.
 *
 * Accepts what the corpus writes: an optional sign, digits with an
 * optional point, an optional `e±nn`, and the three names a decimal
 * cannot spell — `inf`, `-inf`, `nan`.
 */
function float128FromDecimalString(text) {
  const trimmed = text.trim();
  const lowered = trimmed.toLowerCase();
  if (lowered === "inf" || lowered === "+inf" || lowered === "infinity" || lowered === "+infinity") {
    return octetsOf(false, EXPONENT_MAX, 0n);
  }
  if (lowered === "-inf" || lowered === "-infinity") {
    return octetsOf(true, EXPONENT_MAX, 0n);
  }
  if (lowered === "nan" || lowered === "+nan") {
    return octetsOf(false, EXPONENT_MAX, 1n << (FRACTION_BITS - 1n));
  }
  if (lowered === "-nan") {
    return octetsOf(true, EXPONENT_MAX, 1n << (FRACTION_BITS - 1n));
  }

  const parsed = decimalParts(trimmed);
  if (parsed === undefined) {
    throw new RangeError(`a decimal, not \`${text}\``);
  }
  const { negative, digits, exponent } = parsed;
  if (digits === 0n) {
    return octetsOf(negative, 0, 0n);
  }

  let numerator = digits;
  let denominator = 1n;
  if (exponent >= 0) {
    numerator *= 10n ** BigInt(exponent);
  } else {
    denominator = 10n ** BigInt(-exponent);
  }

  let unbiased = bitLength(numerator) - bitLength(denominator);
  while (!atLeastPowerOfTwo(numerator, denominator, unbiased)) {
    unbiased -= 1;
  }
  while (atLeastPowerOfTwo(numerator, denominator, unbiased + 1)) {
    unbiased += 1;
  }

  let scale = Math.max(unbiased, MIN_NORMAL_EXPONENT);
  let significand = roundedShiftedQuotient(numerator, denominator, 112 - scale);
  if (significand >= IMPLIED_ONE << 1n) {
    scale += 1;
    significand = roundedShiftedQuotient(numerator, denominator, 112 - scale);
  }

  if (significand < IMPLIED_ONE) {
    return octetsOf(negative, 0, significand);
  }
  const biased = scale + EXPONENT_BIAS;
  if (biased >= EXPONENT_MAX) {
    return octetsOf(negative, EXPONENT_MAX, 0n);
  }
  return octetsOf(negative, biased, significand - IMPLIED_ONE);
}

/**
 * The inverse of {@link float128FromDecimalString}: the exact decimal a
 * little-endian-ordered sixteen octets is.
 *
 * `runtime/typescript` has no counterpart — nothing in that column's own
 * generator reads a `long double` back into JSON — so this is written
 * from the clause directly rather than ported.
 *
 * Exact and not shortest: a binary fraction terminates in base ten
 * because `2^-k == 5^k / 10^k`, so the significand times `5^k` is the
 * decimal digits with the point moved `k` places, with nothing rounded
 * on the way. `float128FromDecimalString` parses the result back to the
 * same sixteen octets by construction, because it too computes on the
 * exact rational rather than on a `number`.
 */
function float128ToDecimalString(octets) {
  if (octets.length !== 16) {
    throw new RangeError(`sixteen octets, not ${octets.length}`);
  }
  let bits = 0n;
  for (let index = 15; index >= 0; index -= 1) {
    bits = (bits << 8n) | BigInt(octets[index] ?? 0);
  }
  const negative = (bits >> 127n) !== 0n;
  const biasedExponent = Number((bits >> FRACTION_BITS) & 0x7fffn);
  const fraction = bits & (IMPLIED_ONE - 1n);
  const sign = negative ? "-" : "";

  if (biasedExponent === EXPONENT_MAX) {
    return fraction === 0n ? `${sign}inf` : `${sign}nan`;
  }
  if (biasedExponent === 0 && fraction === 0n) {
    return negative ? "-0" : "0";
  }

  let mantissa;
  let exponent;
  if (biasedExponent === 0) {
    // A subnormal: no implied leading one, the smallest normal's
    // exponent held fixed.
    mantissa = fraction;
    exponent = MIN_NORMAL_EXPONENT - Number(FRACTION_BITS);
  } else {
    mantissa = fraction | IMPLIED_ONE;
    exponent = biasedExponent - EXPONENT_BIAS - Number(FRACTION_BITS);
  }

  if (exponent >= 0) {
    const whole = mantissa << BigInt(exponent);
    return `${sign}${whole.toString()}`;
  }
  const k = -exponent;
  const scaled = mantissa * 5n ** BigInt(k);
  let digits = scaled.toString();
  if (digits.length <= k) {
    digits = "0".repeat(k - digits.length + 1) + digits;
  }
  const split = digits.length - k;
  const whole = digits.slice(0, split);
  let fractionText = digits.slice(split).replace(/0+$/u, "");
  if (fractionText === "") {
    return `${sign}${whole}`;
  }
  return `${sign}${whole}.${fractionText}`;
}

/** The three parts of a decimal, or `undefined` where there are not three. */
function decimalParts(text) {
  const match = /^([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/u.exec(text);
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
function atLeastPowerOfTwo(numerator, denominator, power) {
  if (power >= 0) {
    return numerator >= denominator << BigInt(power);
  }
  return numerator << BigInt(-power) >= denominator;
}

/**
 * `round(numerator * 2^shift / denominator)`, to nearest with ties to
 * even.
 */
function roundedShiftedQuotient(numerator, denominator, shift) {
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
function bitLength(value) {
  return value === 0n ? 0 : value.toString(2).length;
}

/** The sixteen octets of one sign, biased exponent and stored fraction. */
function octetsOf(negative, biasedExponent, fraction) {
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

// -- the union-overlay bit patterns -------------------------------------

/**
 * The low `octets` of `value`'s two's-complement bits, zero-extended —
 * {@link Writer#writeUnionOverlay}'s pattern for an integer, character,
 * enumeration or bitmask arm.
 */
function bitPatternInt(value, octets) {
  const pattern = BigInt.asUintN(64, value);
  return octets >= 8 ? pattern : pattern & ((1n << BigInt(octets * 8)) - 1n);
}

/** One octet, 0 or 1 — the boolean overlay pattern. */
function bitPatternBool(value) {
  return value ? 1n : 0n;
}

/** The IEEE 754 binary32 bits, zero-extended. */
function bitPatternF32(value) {
  const buffer = new DataView(new ArrayBuffer(4));
  buffer.setFloat32(0, value, true);
  return BigInt(buffer.getUint32(0, true));
}

/** The IEEE 754 binary64 bits. */
function bitPatternF64(value) {
  const buffer = new DataView(new ArrayBuffer(8));
  buffer.setFloat64(0, value, true);
  return buffer.getBigUint64(0, true);
}

// -- the writer -----------------------------------------------------------

class Writer {
  /**
   * A writer in `order` whose alignment is capped at `maxAlign`.
   *
   * Four for XCDR2, eight for XCDR1. §7.4.1: `ALIGN(N)` aligns to
   * `MIN(N, XCDR.maxalign)`.
   */
  constructor(order, maxAlign = 8, plCdr1 = PL_CDR1_DEFAULT, shortLengthCodes = false) {
    this.order = order;
    this.out = [];
    this.origin = 0;
    this.maxAlignment = Math.max(1, maxAlign);
    this.plCdr1 = plCdr1;
    this.shortLengthCodes = shortLengthCodes;
  }

  /** The alignment cap in force. */
  get maxAlign() {
    return this.maxAlignment;
  }

  /** How many octets have been written. */
  position() {
    return this.out.length;
  }

  /** The octets written so far. */
  finish() {
    return Uint8Array.from(this.out);
  }

  /** Pad to `align`, counted from the stream's origin. */
  alignTo(align) {
    const to = Math.min(align, this.maxAlignment);
    if (to <= 1) {
      return;
    }
    const offset = this.out.length - this.origin;
    const padding = (to - (offset % to)) % to;
    for (let index = 0; index < padding; index += 1) {
      this.out.push(0);
    }
  }

  /**
   * Move the alignment origin to the current position.
   *
   * `ElementPadding::Packed` is HDDS's measured departure from XTypes 1.3
   * §7.4.1, which counts every element's alignment from the encapsulation:
   * each collection element and each aggregate value counts its own
   * alignment from its own first octet instead. `array-1d-struct-final` is
   * `01000000 0200 03000000 0400 77`, the second `Cell`'s `long` at offset
   * six where the clause puts it at eight, and `typedef-of-struct` is the
   * same inside two `@final` members.
   *
   * Returns the origin displaced, for `finishPackedElement`.
   */
  beginPackedElement() {
    const saved = this.origin;
    this.origin = this.out.length;
    return saved;
  }

  /** Restore the origin `beginPackedElement` displaced. */
  finishPackedElement(saved) {
    this.origin = saved;
  }

  refuse(expected) {
    throw new OutOfRange(this.out.length, expected);
  }

  check(value, low, high, what) {
    if (!Number.isInteger(value) || value < low || value > high) {
      this.refuse(`${what} in [${low}, ${high}], not ${value}`);
    }
  }

  checkBig(value, low, high, what) {
    if (value < low || value > high) {
      this.refuse(`${what} in [${low}, ${high}], not ${value}`);
    }
  }

  raw(octets) {
    for (const octet of octets) {
      this.out.push(octet & 0xff);
    }
  }

  /** An integer of `width` octets, in the stream's order. */
  writeInt(value, width, align = width) {
    this.alignTo(align);
    this.rawInt(value, width, isLittle(this.order));
  }

  /** An integer of `width` octets in a named order, with no alignment. */
  rawInt(value, width, little) {
    const octets = new Array(width);
    let rest = value & ((1n << BigInt(width * 8)) - 1n);
    for (let index = 0; index < width; index += 1) {
      octets[index] = Number(rest & 0xffn);
      rest >>= 8n;
    }
    if (!little) {
      octets.reverse();
    }
    this.raw(octets);
  }

  // -- primitives ----------------------------------------------------

  /** `boolean`, one octet, 0 or 1. */
  writeBool(value) {
    this.out.push(value ? 1 : 0);
  }

  /** `octet` and `uint8`. */
  writeU8(value) {
    this.check(value, 0, 0xff, "an octet");
    this.out.push(value);
  }

  /** `int8`. */
  writeI8(value) {
    this.check(value, -0x80, 0x7f, "an int8");
    this.out.push(value & 0xff);
  }

  /**
   * `char`, one octet, as the one-character `string` it is held as.
   *
   * A code point above 255 is refused rather than truncated.
   */
  writeChar(value) {
    if ([...value].length !== 1) {
      this.refuse(`exactly one character, not \`${value}\``);
    }
    const point = value.codePointAt(0) ?? 0;
    if (point > 0xff) {
      this.refuse(`a character below U+0100, not U+${point.toString(16).toUpperCase()}`);
    }
    this.out.push(point);
  }

  /**
   * `wchar`, at `width` octets and in the shape `framing` gives it.
   */
  writeWchar(
    value,
    width = WcharWidth.Octets2,
    framing = WcharFraming.Fixed,
    order = WstringUnitOrder.Stream,
  ) {
    if ([...value].length !== 1) {
      this.refuse(`exactly one character, not \`${value}\``);
    }
    const point = value.codePointAt(0) ?? 0;
    if (point > 0xffff) {
      this.refuse(`a BMP character, not U+${point.toString(16).toUpperCase()}`);
    }
    if (width !== WcharWidth.Octets2 && width !== WcharWidth.Octets4) {
      this.refuse(`a wchar width of 2 or 4, not ${String(width)}`);
    }
    if (framing === WcharFraming.GiopLengthPrefixed) {
      this.out.push(width);
      const little = order === WstringUnitOrder.Stream && isLittle(this.order);
      this.rawInt(BigInt(point), width, little);
      return;
    }
    this.writeInt(BigInt(point), width);
  }

  /** `short`. */
  writeI16(value) {
    this.check(value, -0x8000, 0x7fff, "a short");
    this.writeInt(BigInt(value), 2);
  }

  /** `unsigned short`. */
  writeU16(value) {
    this.check(value, 0, 0xffff, "an unsigned short");
    this.writeInt(BigInt(value), 2);
  }

  /** `long`. */
  writeI32(value) {
    this.check(value, -0x8000_0000, 0x7fff_ffff, "a long");
    this.writeInt(BigInt(value), 4);
  }

  /** `unsigned long`. */
  writeU32(value) {
    this.check(value, 0, 0xffff_ffff, "an unsigned long");
    this.writeInt(BigInt(value), 4);
  }

  /** `long long`, a `bigint` because a binary64 is not one. */
  writeI64(value) {
    this.checkBig(value, -(2n ** 63n), 2n ** 63n - 1n, "a long long");
    this.writeInt(value, 8);
  }

  /** `unsigned long long`. */
  writeU64(value) {
    this.checkBig(value, 0n, 2n ** 64n - 1n, "an unsigned long long");
    this.writeInt(value, 8);
  }

  /**
   * The Cyclone union overlay: the low `octets` of `pattern`, written as
   * one unsigned integer of that width in the stream's byte order.
   */
  writeUnionOverlay(pattern, octets) {
    switch (octets) {
      case 1:
        this.writeU8(Number(pattern & 0xffn));
        return;
      case 2:
        this.writeU16(Number(pattern & 0xffffn));
        return;
      case 4:
        this.writeU32(Number(pattern & 0xffff_ffffn));
        return;
      case 8:
        this.writeU64(pattern);
        return;
      default:
        throw new OutOfRange(this.out.length, `a union arm of 1, 2, 4 or 8 octets, not ${octets}`);
    }
  }

  /** `float`, a binary32. */
  writeF32(value) {
    this.alignTo(4);
    const buffer = new DataView(new ArrayBuffer(4));
    buffer.setFloat32(0, value, isLittle(this.order));
    this.raw(new Uint8Array(buffer.buffer));
  }

  /** `double`, a binary64. */
  writeF64(value) {
    this.alignTo(8);
    const buffer = new DataView(new ArrayBuffer(8));
    buffer.setFloat64(0, value, isLittle(this.order));
    this.raw(new Uint8Array(buffer.buffer));
  }

  /**
   * `long double`, as the sixteen octets it is. `value` is
   * `{ octets: Uint8Array(16) }`, little-endian-ordered; reversed for a
   * big-endian stream. A `number` never appears.
   */
  writeF128(value) {
    if (value.octets.length !== 16) {
      this.refuse(`sixteen octets, not ${value.octets.length}`);
    }
    this.alignTo(8);
    const octets = Array.from(value.octets);
    if (!isLittle(this.order)) {
      octets.reverse();
    }
    this.raw(octets);
  }

  // -- strings ---------------------------------------------------------

  /**
   * `string`: a four-octet length that **includes** the terminator, then
   * the octets, then a NUL. §7.4.1.1.1.
   */
  writeString(value) {
    const octets = new TextEncoder().encode(value);
    this.writeU32(octets.length + 1);
    this.raw(octets);
    this.out.push(0);
  }

  /**
   * A bounded `string`: the same bytes `writeString` writes. The bound is
   * a promise about the source and not a difference on the wire, so this
   * is the only place it can be enforced — refused rather than cut at a
   * character boundary, which would put a sample on the wire nobody
   * wrote. The bound counts octets, which is what a `string<N>` bound
   * counts for every measured implementation.
   */
  writeStringBounded(value, bound) {
    const octets = new TextEncoder().encode(value).length;
    if (octets > bound) {
      this.refuse(`at most ${bound} octets in a bounded string, and this one has ${octets}`);
    }
    this.writeString(value);
  }

  /**
   * A bounded `wstring`, as `writeStringBounded` is to `writeString`. The
   * bound counts UTF-16 code units, not characters and not octets,
   * whatever `policy` puts on the wire.
   */
  writeWstringBounded(value, bound, policy, order) {
    const units = value.length;
    if (units > bound) {
      this.refuse(
        `at most ${bound} UTF-16 code units in a bounded wstring, and this one has ${units}`,
      );
    }
    this.writeWstringUtf8(value, policy, order);
  }

  /**
   * The count check every bounded `sequence` makes before a byte of it is
   * written. Table 17 makes `O2.length <= T1.length` the condition for
   * constructing a `sequence<T, N>`, and the wire carries no bound to
   * compare against: a sender whose count already exceeds its own
   * declaration has written a value of a type nobody declared.
   */
  checkSequenceBound(actual, bound) {
    if (actual > bound) {
      this.refuse(`at most ${bound} elements in a bounded sequence, and this one has ${actual}`);
    }
  }

  /**
   * The count check every bounded `map` makes before a byte of it is
   * written. Its own message and not `checkSequenceBound`'s: the unit is
   * an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
   * map-as-a-sequence-of-pairs equivalence.
   */
  checkMapBound(actual, bound) {
    if (actual > bound) {
      this.refuse(`at most ${bound} entries in a bounded map, and this one has ${actual}`);
    }
  }

  /** `wstring`: a four-octet **octet** count, then UTF-16 code units. */
  writeWstring(value) {
    this.writeWstringUtf8(value, WstringLengthPolicy.Specification, WstringUnitOrder.Stream);
  }

  /** A `wstring` in the form `policy` and `order` name. */
  writeWstringUtf8(value, policy, order) {
    const width = wstringUnitWidth(policy);
    const counts = wstringCountsCharacters(policy);
    if (counts && width === 2) {
      for (const character of value) {
        const point = character.codePointAt(0) ?? 0;
        if (point > 0xffff) {
          this.refuse(
            "a code point inside the BMP: a character count over two-octet units " +
              `has no way to carry U+${point.toString(16).toUpperCase()}`,
          );
        }
      }
    }
    const count = value.length;
    const terminated = wstringIsTerminated(policy);
    const marked = order === WstringUnitOrder.ByteOrderMark && count > 0;
    const total = count + (terminated ? 1 : 0) + (marked ? 1 : 0);
    this.writeU32(counts ? total : total * width);
    if (marked) {
      this.writeWstringUnit(BYTE_ORDER_MARK, width, order);
    }
    for (let index = 0; index < count; index += 1) {
      this.writeWstringUnit(value.charCodeAt(index), width, order);
    }
    if (terminated) {
      this.writeWstringUnit(0, width, order);
    }
  }

  /**
   * A `wstring` whose characters go out as their own UTF-8 octets.
   *
   * `WstringCodeUnits::Utf8` is the measured departure of one product's
   * generated serialiser: XTypes 1.3 Table 31 gives a `wstring`'s code unit
   * as `Char16` and §7.4.1.1.2 makes the length "twice the number of
   * characters", so the clause has no UTF-8 form at all. The declared length
   * is the UTF-8 octet count and only C2 — the terminator — still reaches the
   * wire. `wstring-member` is `04 00 00 00 61 62 63 00` for `"abc"`: three
   * octets, a NUL, and the four declares both.
   */
  writeWstringUtf8Octets(value, policy) {
    // A lone surrogate is not a code point. `TextEncoder` would replace one
    // with U+FFFD and report success — a different character on the wire,
    // silently. The iterator yields code points, so a lone surrogate is the
    // only value in the surrogate range here.
    for (const character of value) {
      const point = character.codePointAt(0) ?? 0;
      if (point >= 0xd800 && point <= 0xdfff) {
        this.refuse("a Unicode code point, not a lone surrogate");
      }
    }
    const octets = new TextEncoder().encode(value);
    const terminated = wstringIsTerminated(policy);
    const total = octets.length + (terminated ? 1 : 0);
    if (total > 0xffffffff) {
      this.refuse("a wstring length the UInt32 field can hold");
    }
    this.writeU32(total);
    this.raw(octets);
    if (terminated) {
      this.out.push(0);
    }
  }

  /**
   * A bounded UTF-8-octet `wstring`. The bound still counts UTF-16 code
   * units, which is what a `wstring<N>` bound means, while the wire carries
   * the UTF-8 octets.
   */
  writeWstringUtf8OctetsBounded(value, bound, policy) {
    const units = value.length;
    if (units > bound) {
      this.refuse(
        `at most ${bound} UTF-16 code units in a bounded wstring, and this one has ${units}`,
      );
    }
    this.writeWstringUtf8Octets(value, policy);
  }

  /** One `wstring` code unit, at its width and in its order. */
  writeWstringUnit(unit, width, order) {
    if (order === WstringUnitOrder.BigEndianAlways) {
      this.alignTo(width);
      this.rawInt(BigInt(unit), width, false);
      return;
    }
    if (wstringOrderIsFixedLittle(order)) {
      this.alignTo(width);
      this.rawInt(BigInt(unit), width, true);
      return;
    }
    this.writeInt(BigInt(unit), width);
  }

  /** `fixed<digits, scale>` as packed BCD. [CDR] §9.3.2.8. */
  writeFixed(value, digits, scale, count) {
    const nibbles = this.fixedNibbles(value, digits, scale, count);
    for (let index = 0; index + 1 < nibbles.length; index += 2) {
      this.out.push((((nibbles[index] ?? 0) << 4) | (nibbles[index + 1] ?? 0)) & 0xff);
    }
  }

  /** The BCD nibbles of `text` under `fixed<digits, scale>`. */
  fixedNibbles(text, digits, scale, count) {
    if (digits === 0) {
      this.refuse("a fixed with at least one digit");
    }
    if (digits > 31) {
      this.refuse(`a fixed of at most 31 digits, not ${digits}`);
    }
    if (scale > digits) {
      this.refuse("a scale no larger than the digit count");
    }
    let magnitude = text;
    let negative = false;
    if (magnitude.startsWith("-")) {
      negative = true;
      magnitude = magnitude.slice(1);
    } else if (magnitude.startsWith("+")) {
      magnitude = magnitude.slice(1);
    }
    const dot = magnitude.indexOf(".");
    let whole = dot < 0 ? magnitude : magnitude.slice(0, dot);
    const fraction = dot < 0 ? "" : magnitude.slice(dot + 1);
    const bad = `a decimal that fits fixed<${digits}, ${scale}>, not \`${text}\``;
    if (whole === "" && fraction === "") {
      this.refuse(bad);
    }
    if (!/^\d*$/u.test(whole) || !/^\d*$/u.test(fraction)) {
      this.refuse(bad);
    }
    const asWritten = whole.length + scale;
    const integralWidth = digits - scale;
    whole = whole.replace(/^0+/u, "");
    if (whole.length > integralWidth) {
      this.refuse(`a decimal whose integer part fits ${integralWidth} digits, not \`${text}\``);
    }
    if (fraction.length > scale) {
      this.refuse(`a decimal whose fraction fits ${scale} digits, not \`${text}\``);
    }

    const every = new Array(digits).fill(0);
    let at = integralWidth - whole.length;
    for (const character of whole) {
      every[at] = character.charCodeAt(0) - 0x30;
      at += 1;
    }
    for (const character of fraction) {
      every[at] = character.charCodeAt(0) - 0x30;
      at += 1;
    }
    let leadingZeros = 0;
    while (leadingZeros < every.length && every[leadingZeros] === 0) {
      leadingZeros += 1;
    }
    const significant = every.length - leadingZeros;
    let wire = digits;
    if (count === FixedDigitCount.ValuePrecision) {
      wire = Math.max(significant, 1);
    } else if (count === FixedDigitCount.ValueStringDigits) {
      // ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
      // zero. Past 31 no reader can be told the field's width.
      wire = asWritten + (asWritten % 2 === 0 ? 1 : 0);
      if (wire > 31) {
        this.refuse(`a fixed of at most 31 digits on the wire, not ${wire} for \`${text}\``);
      }
    }

    const odd = (wire + 1) % 2 === 1;
    const nibbles = [];
    if (odd) {
      nibbles.push(0);
    }
    // A count wider than P is filled with leading zeros.
    const kept = Math.min(wire, every.length);
    for (let index = kept; index < wire; index += 1) {
      nibbles.push(0);
    }
    for (let index = every.length - kept; index < every.length; index += 1) {
      nibbles.push(every[index] ?? 0);
    }
    nibbles.push(negative && significant > 0 ? 0x0d : 0x0c);
    return nibbles;
  }

  // -- aggregates --------------------------------------------------------

  /** A DHEADER: a four-octet length of what follows. §7.4.3.5.3. */
  writeDheader() {
    this.alignTo(4);
    const at = this.out.length;
    this.raw([0, 0, 0, 0]);
    return { at };
  }

  /** Patch a DHEADER with the length of what followed it. */
  finishDheader(header) {
    if (header.at < 0) {
      return;
    }
    this.patch(header.at, this.out.length - header.at - 4, 4);
  }

  /**
   * An XCDR2 EMHEADER: the member id, its length code and the
   * must-understand flag in one 32-bit word. §7.4.3.4.2.
   */
  writeEmheader(memberId, lengthCode, mustUnderstand) {
    if (!Number.isInteger(memberId) || memberId < 0 || memberId > E_MEMBER_ID_MASK) {
      this.refuse(`a member id inside the 28 bits of an EMHEADER, not ${memberId}`);
    }
    this.alignTo(4);
    const word =
      ((mustUnderstand ? 1 : 0) << 31) | ((lengthCode & 0x7) << 28) | (memberId & 0x0fff_ffff);
    this.writeInt(BigInt(word >>> 0), 4);
    const wordAt = this.out.length - 4;
    if (lengthCode === 4) {
      const at = this.out.length;
      this.raw([0, 0, 0, 0]);
      return { nextInt: at, word: wordAt, lengthCode };
    }
    return { nextInt: undefined, word: wordAt, lengthCode };
  }

  /** Patch an EMHEADER's NEXTINT with the length of what followed. */
  finishEmheader(header) {
    if (header.nextInt === undefined) {
      if (header.lengthCode >= 0 && header.lengthCode <= 3 && header.word + 4 <= this.out.length) {
        const declared = 1 << header.lengthCode;
        const body = this.out.length - header.word - 4;
        if (body !== declared) {
          this.refuse(
            `a member of ${declared} octets under length code ${header.lengthCode}, not ${body}`,
          );
        }
      }
      return;
    }
    const length = this.out.length - header.nextInt - 4;
    if (this.shortLengthCodes && [1, 2, 4, 8].includes(length)) {
      this.shortenEmheader(header, length);
      return;
    }
    this.patch(header.nextInt, length, 4);
  }

  /** Remove an LC4 header's NEXTINT and raise its length code. */
  shortenEmheader(header, length) {
    const at = header.nextInt;
    if (at === undefined) {
      return;
    }
    const code = { 1: 0, 2: 1, 4: 2, 8: 3 }[length];
    this.out.splice(at, 4);
    const previous = this.read(header.word, 4);
    this.patch(header.word, ((previous & ~(0x7 << 28)) | (code << 28)) >>> 0, 4);
  }

  // -- parameter lists -----------------------------------------------

  /** A PL_CDR1 parameter header, short form: `[id:u16][length:u16]`. */
  writePid(pid, role = ParameterRole.Member, mustUnderstand = false) {
    if (!Number.isInteger(pid) || pid < 0 || pid > this.plCdr1.shortMaxId) {
      this.refuse(
        `a parameter id the short header carries, not ${pid} — the extended header takes it`,
      );
    }
    this.alignTo(4);
    let id = pid;
    if (mustUnderstand && this.plCdr1.memberMustUnderstand) {
      id |= PID_MUST_UNDERSTAND;
    }
    id |= pidFlagsAt(this.plCdr1.pidFlags, "member");
    const bare = pid & ~(PID_MUST_UNDERSTAND | PID_IMPL_EXTENSION);
    this.writeInt(BigInt(id & 0xffff), 2);
    const at = this.out.length;
    this.raw([0, 0]);
    return {
      at,
      extended: false,
      savedOrigin: this.beginParameterValue(),
      role,
      pid: bare,
      mustUnderstand,
    };
  }

  /** A PL_CDR1 parameter header, extended form. */
  writePidExtended(pid, role = ParameterRole.Member, mustUnderstand = false) {
    if (!Number.isInteger(pid) || pid < 0 || pid > E_MEMBER_ID_MASK) {
      this.refuse(`a member id inside the 28 bits of an eMemberHeader, not ${pid}`);
    }
    this.alignTo(4);
    const marker = PID_EXTENDED | pidFlagsAt(this.plCdr1.pidFlags, "extended-marker");
    this.writeInt(BigInt(marker), 2);
    this.writeInt(8n, 2);
    let header = pid & E_MEMBER_ID_MASK;
    if (mustUnderstand && this.plCdr1.memberMustUnderstand) {
      header |= E_FLAG_MUST_UNDERSTAND;
    }
    this.writeInt(BigInt(header >>> 0), 4);
    const at = this.out.length;
    this.raw([0, 0, 0, 0]);
    return {
      at,
      extended: true,
      savedOrigin: this.beginParameterValue(),
      role,
      pid,
      mustUnderstand,
    };
  }

  /** Start a parameter's value, returning the origin it replaces. */
  beginParameterValue() {
    const saved = this.origin;
    this.origin = this.out.length;
    return saved;
  }

  /** Patch a parameter header with its value's length. */
  finishPid(header) {
    const width = header.extended ? 4 : 2;
    let length = this.out.length - header.at - width;
    const pads =
      header.role === ParameterRole.Optional
        ? this.plCdr1.padsOptionalLength
        : this.plCdr1.padsDeclaredLength;
    if (pads) {
      const past = (this.out.length - header.savedOrigin) % 4;
      if (past !== 0) {
        length += 4 - past;
      }
    }
    if (!header.extended && length > PID_SHORT_MAX_LEN) {
      this.promotePid(header, length);
    } else {
      this.patch(header.at, length, width);
    }
    this.origin = header.savedOrigin;
    if (this.plCdr1.padAfterValue) {
      this.alignTo(4);
    }
  }

  /** Rewrite a short parameter header as the extended form. */
  promotePid(header, length) {
    const start = header.at - 2;
    this.out.splice(header.at + 2, 0, ...new Array(8).fill(0));
    const marker = PID_EXTENDED | pidFlagsAt(this.plCdr1.pidFlags, "extended-marker");
    this.patch(start, marker & 0xffff, 2);
    this.patch(start + 2, 8, 2);
    let memberHeader = header.pid & E_MEMBER_ID_MASK;
    if (header.mustUnderstand && this.plCdr1.memberMustUnderstand) {
      memberHeader |= E_FLAG_MUST_UNDERSTAND;
    }
    this.patch(start + 4, memberHeader >>> 0, 4);
    this.patch(start + 8, length, 4);
  }

  /** The `[PID_LIST_END][0]` that ends a parameter list. §7.4.1.2.4. */
  writeSentinel() {
    this.alignTo(4);
    this.writeInt(BigInt(PID_LIST_END | pidFlagsAt(this.plCdr1.pidFlags, "sentinel")), 2);
    this.writeInt(0n, 2);
  }

  // -- enumerations ------------------------------------------------------

  /**
   * An enumerator, as the integer its bit bound calls for. §7.4.3.5.3
   * rule (5): the holder is signed.
   */
  writeEnum(bitBound, value) {
    if (bitBound <= 0 || bitBound > 32) {
      this.refuse(`an @bit_bound between 1 and 32, not ${bitBound}`);
    }
    const low = -(1n << BigInt(bitBound - 1));
    const high = (1n << BigInt(bitBound)) - 1n;
    if (value < low || value > high) {
      this.refuse(`an enumerator a ${bitBound}-bit holder can hold, not ${value}`);
    }
    const pattern = value & ((1n << BigInt(bitBound)) - 1n);
    if (bitBound <= 8) {
      this.writeU8(Number(pattern));
    } else if (bitBound <= 16) {
      this.writeU16(Number(pattern));
    } else {
      this.writeU32(Number(pattern));
    }
  }

  /** A bitmask, as the unsigned integer its bit bound calls for. */
  writeBitmask(bitBound, bits) {
    if (bitBound <= 0 || bitBound > 64) {
      this.refuse(`an @bit_bound between 1 and 64, not ${bitBound}`);
    }
    if (bits < 0n || bits >> BigInt(bitBound) !== 0n) {
      this.refuse(`a bitmask below 2^${bitBound}, not ${bits}`);
    }
    if (bitBound <= 8) {
      this.writeU8(Number(bits));
    } else if (bitBound <= 16) {
      this.writeU16(Number(bits));
    } else if (bitBound <= 32) {
      this.writeU32(Number(bits));
    } else {
      this.writeU64(bits);
    }
  }

  /** One bitfield, shifted into the accumulator a bitset packs into. */
  bitfield(value, width, position) {
    if (width <= 0 || width > 64 || position < 0 || position + width > 64) {
      this.refuse(`a bitfield of ${width} bits at ${position}, inside 64`);
    }
    const limit = (1n << BigInt(width)) - 1n;
    if (value < 0n || value > limit) {
      this.refuse(`a value inside ${width} bits, not ${value}`);
    }
    return value << BigInt(position);
  }

  /** A bitset, as the anchor integer its bitfields pack into. */
  writeBitset(anchorOctets, bits) {
    switch (anchorOctets) {
      case 1:
        this.writeBitmask(8, bits);
        return;
      case 2:
        this.writeBitmask(16, bits);
        return;
      case 4:
        this.writeBitmask(32, bits);
        return;
      case 8:
        this.writeBitmask(64, bits);
        return;
      default:
        this.refuse(`a bitset anchor of 1, 2, 4 or 8 octets, not ${anchorOctets}`);
    }
  }

  // -- patching ------------------------------------------------------

  patch(at, value, width) {
    let rest = BigInt(value) & ((1n << BigInt(width * 8)) - 1n);
    const octets = new Array(width);
    for (let index = 0; index < width; index += 1) {
      octets[index] = Number(rest & 0xffn);
      rest >>= 8n;
    }
    if (!isLittle(this.order)) {
      octets.reverse();
    }
    for (let index = 0; index < width; index += 1) {
      this.out[at + index] = octets[index] ?? 0;
    }
  }

  read(at, width) {
    const octets = this.out.slice(at, at + width);
    if (!isLittle(this.order)) {
      octets.reverse();
    }
    let value = 0;
    for (let index = width - 1; index >= 0; index -= 1) {
      value = value * 256 + (octets[index] ?? 0);
    }
    return value >>> 0;
  }
}

// -- the reader -------------------------------------------------------------

class Reader {
  /**
   * A reader over `data` in `order`, capped at `maxAlign`.
   *
   * Four for XCDR2, eight for XCDR1 — {@link Writer}'s counterpart.
   */
  constructor(data, order, maxAlign = 8, plCdr1 = PL_CDR1_DEFAULT) {
    this.bytes = data;
    this.order = order;
    this.at = 0;
    this.origin = 0;
    this.maxAlignment = Math.max(1, maxAlign);
    this.plCdr1 = plCdr1;
    this.state = Status.Ok;
  }

  /** The alignment cap in force. */
  get maxAlign() {
    return this.maxAlignment;
  }

  // -- the status a generated decoder reads -------------------------

  stopped() {
    return this.state !== Status.Ok;
  }

  /** How the read is going. */
  readerStatus() {
    return this.state;
  }

  /** Refuse the stream from outside the runtime. The first refusal sticks. */
  readerRefuse(refusal) {
    if (refusal !== Status.Ok && this.state === Status.Ok) {
      this.state = refusal;
    }
  }

  /** Take back a {@link Status.Discarded}, and only that one. */
  readerClearDiscard() {
    if (this.state === Status.Discarded) {
      this.state = Status.Ok;
      return true;
    }
    return false;
  }

  /** Where it is. The generated decoder's name for {@link position}. */
  readerPosition() {
    return this.at;
  }

  /** How many octets are left. */
  readerRemaining() {
    return this.remaining();
  }

  /**
   * Move the alignment origin to the current position.
   *
   * The read-side pair of the writer's `beginPackedElement`, for
   * `ElementPadding::Packed`. Returns the origin displaced.
   */
  readerBeginPackedElement() {
    const saved = this.origin;
    this.origin = this.at;
    return saved;
  }

  /** Restore the origin `readerBeginPackedElement` displaced. */
  readerFinishPackedElement(saved) {
    this.origin = saved;
  }

  /** Move the cursor forward to `target`, which is never backwards. */
  readerSkipTo(target) {
    if (this.stopped()) {
      return;
    }
    if (target < this.at) {
      this.readerRefuse(Status.OutOfRange);
      return;
    }
    if (target > this.bytes.length) {
      throw new Truncated(this.at, target - this.at, this.remaining());
    }
    this.at = target;
  }

  /** The cursor, the origin and the status, to be put back later. */
  readerSave() {
    return { at: this.at, origin: this.origin, status: this.state };
  }

  /** Put back what {@link readerSave} took. */
  readerRestore(mark) {
    this.at = mark.at;
    this.origin = mark.origin;
    this.state = mark.status;
  }

  // -- state -----------------------------------------------------------

  /** Where it is. */
  position() {
    return this.at;
  }

  /** How many octets are left. */
  remaining() {
    return Math.max(0, this.bytes.length - this.at);
  }

  /** Skip padding to `align`, counted from the stream's origin. */
  alignTo(align) {
    if (this.stopped()) {
      return;
    }
    const to = Math.min(align, this.maxAlignment);
    if (to <= 1) {
      return;
    }
    const offset = this.at - this.origin;
    this.at += (to - (((offset % to) + to) % to)) % to;
  }

  /** Step over `count` octets, refusing to step past the end. */
  skip(count) {
    if (this.stopped()) {
      return;
    }
    this.take(count);
  }

  /** Move to an absolute offset inside the buffer. */
  seek(target) {
    if (this.stopped()) {
      return;
    }
    if (target < 0 || target > this.bytes.length) {
      throw new Truncated(this.at, target, this.bytes.length);
    }
    this.at = target;
  }

  take(count) {
    if (!Number.isInteger(count) || count < 0) {
      throw new Truncated(this.at, count, this.remaining());
    }
    const end = this.at + count;
    if (end > this.bytes.length) {
      throw new Truncated(this.at, count, this.remaining());
    }
    const from = this.at;
    this.at = end;
    return from;
  }

  readInt(width, signed) {
    this.alignTo(width);
    return this.rawInt(width, signed, isLittle(this.order));
  }

  rawInt(width, signed, little) {
    const from = this.take(width);
    let value = 0n;
    for (let index = 0; index < width; index += 1) {
      const shift = little ? index : width - 1 - index;
      value |= BigInt(this.bytes[from + index] ?? 0) << BigInt(8 * shift);
    }
    if (signed) {
      const sign = 1n << BigInt(width * 8 - 1);
      if ((value & sign) !== 0n) {
        value -= 1n << BigInt(width * 8);
      }
    }
    return value;
  }

  // -- primitives ------------------------------------------------------

  /**
   * `boolean`. An octet that is neither 0 nor 1 is a stream that
   * disagrees with its own type, and is refused rather than coerced.
   */
  readBool() {
    if (this.stopped()) {
      return false;
    }
    const where = this.at;
    const value = this.readU8();
    if (value === 0) {
      return false;
    }
    if (value === 1) {
      return true;
    }
    throw new OutOfRange(where, "a boolean of 0 or 1");
  }

  /** `octet` and `uint8`. */
  readU8() {
    if (this.stopped()) {
      return 0;
    }
    return this.bytes[this.take(1)] ?? 0;
  }

  /** `int8`. */
  readI8() {
    if (this.stopped()) {
      return 0;
    }
    const value = this.bytes[this.take(1)] ?? 0;
    return value >= 0x80 ? value - 0x100 : value;
  }

  /** `char`, one octet, as the one-character string it is here. */
  readChar() {
    if (this.stopped()) {
      return "\0";
    }
    return String.fromCharCode(this.bytes[this.take(1)] ?? 0);
  }

  /** `wchar`, at `width` octets and in `framing`'s shape. */
  readWchar(width = WcharWidth.Octets2, framing = WcharFraming.Fixed, order = WstringUnitOrder.Stream) {
    if (this.stopped()) {
      return "\0";
    }
    const where = this.at;
    const octets = width;
    let value;
    if (framing === WcharFraming.GiopLengthPrefixed) {
      const declared = this.readU8();
      if (declared !== octets) {
        throw new OutOfRange(
          where,
          `a GIOP 1.2 wchar of ${octets} octets, the configured transmission ` +
            `code set, and not ${declared}`,
        );
      }
      const little = order === WstringUnitOrder.Stream && isLittle(this.order);
      value = Number(this.rawInt(octets, false, little));
    } else if (octets === 4) {
      value = this.readU32();
      if (value > 0xffff) {
        throw new OutOfRange(where, "a four-octet wchar holding one UTF-16 code unit");
      }
    } else {
      value = this.readU16();
    }
    if (value >= 0xd800 && value <= 0xdfff) {
      throw new OutOfRange(where, "a Unicode code point");
    }
    return String.fromCharCode(value);
  }

  /** `short`. */
  readI16() {
    if (this.stopped()) {
      return 0;
    }
    return Number(this.readInt(2, true));
  }

  /** `unsigned short`. */
  readU16() {
    if (this.stopped()) {
      return 0;
    }
    return Number(this.readInt(2, false));
  }

  /** `long`. */
  readI32() {
    if (this.stopped()) {
      return 0;
    }
    return Number(this.readInt(4, true));
  }

  /** `unsigned long`, as a `number`. */
  readU32() {
    if (this.stopped()) {
      return 0;
    }
    return Number(this.readInt(4, false));
  }

  /** `long long`, a `bigint` because a binary64 is not one. */
  readI64() {
    if (this.stopped()) {
      return 0n;
    }
    return this.readInt(8, true);
  }

  /** `unsigned long long`, as the value and not as a pattern. */
  readU64() {
    if (this.stopped()) {
      return 0n;
    }
    return this.readInt(8, false);
  }

  /** `float`, a binary32. */
  readF32() {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(4);
    const from = this.take(4);
    return new DataView(this.bytes.buffer, this.bytes.byteOffset + from, 4).getFloat32(
      0,
      isLittle(this.order),
    );
  }

  /** `double`, a binary64. */
  readF64() {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(8);
    const from = this.take(8);
    return new DataView(this.bytes.buffer, this.bytes.byteOffset + from, 8).getFloat64(
      0,
      isLittle(this.order),
    );
  }

  /**
   * `long double`: all sixteen octets, as the writer's counterpart holds
   * them, `{ octets: Uint8Array(16) }`.
   */
  readF128() {
    if (this.stopped()) {
      return { octets: new Uint8Array(16) };
    }
    this.alignTo(8);
    const from = this.take(16);
    const octets = this.bytes.slice(from, from + 16);
    if (!isLittle(this.order)) {
      octets.reverse();
    }
    return { octets };
  }

  /** A 16-bit value little-endian whatever the stream's order is. */
  readU16Little() {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(2);
    return Number(this.rawInt(2, false, true));
  }

  /** A 16-bit value big-endian whatever the stream's order is. */
  readU16Big() {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(2);
    return Number(this.rawInt(2, false, false));
  }

  /** A 32-bit value little-endian whatever the stream's order is. */
  readU32Little() {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(4);
    return Number(this.rawInt(4, false, true));
  }

  /** A 32-bit value big-endian whatever the stream's order is. */
  readU32Big() {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(4);
    return Number(this.rawInt(4, false, false));
  }

  // -- strings ---------------------------------------------------------

  /** `string`: a four-octet length that includes the terminator. */
  readString() {
    if (this.stopped()) {
      return "";
    }
    const where = this.at;
    const length = this.readU32();
    if (length === 0 || length > this.remaining() + 1) {
      throw new ImpossibleLength(where, length, this.remaining());
    }
    const from = this.take(length - 1);
    const text = this.decodeUtf8(from, length - 1, where);
    this.readU8();
    return text;
  }

  /** A `string` the reader materialises itself. */
  readStringAlloc() {
    return this.readString();
  }

  /** A `wstring`, in whichever length convention the policy names. */
  readWstringUtf8(policy, order) {
    return this.readWstring(policy, order);
  }

  /**
   * The inverse of `Writer.writeWstringUtf8Octets`.
   *
   * `WstringCodeUnits::Utf8`: the declared length is the UTF-8 octet count,
   * and the terminator `policy` asks for is the last of those octets. A count
   * the stream cannot hold, a missing or an interior NUL, or an ill-formed
   * UTF-8 sequence is refused, never a replacement character or a truncated
   * value.
   */
  readWstringUtf8Octets(policy) {
    if (this.stopped()) {
      return "";
    }
    const where = this.at;
    const cell = WSTRING_CELLS.get(policy);
    if (cell === undefined) {
      throw new OutOfRange(where, `a wstring length policy, not \`${policy}\``);
    }
    const declared = this.readU32();
    if (declared > this.remaining()) {
      throw new ImpossibleLength(where, declared, this.remaining());
    }
    const from = this.take(declared);
    let count = declared;
    if (cell.terminated) {
      if (count === 0 || this.bytes[from + count - 1] !== 0) {
        throw new OutOfRange(
          where,
          "a NUL octet ending the wstring, as its count includes one",
        );
      }
      // A NUL inside the text is the narrow string's problem in wide form:
      // every C peer stops at it.
      for (let index = 0; index < count - 1; index += 1) {
        if (this.bytes[from + index] === 0) {
          throw new OutOfRange(where, "no NUL inside a wstring a C peer would stop at");
        }
      }
      count -= 1;
    }
    return this.decodeUtf8(from, count, where);
  }

  /** A `wstring`, in the form `policy` and `order` name. */
  readWstring(policy, order) {
    if (this.stopped()) {
      return "";
    }
    const where = this.at;
    const cell = WSTRING_CELLS.get(policy);
    if (cell === undefined) {
      throw new OutOfRange(where, `a wstring length policy, not \`${policy}\``);
    }
    const declared = this.readU32();
    let count;
    if (cell.countsCharacters) {
      count = declared;
    } else {
      if (declared % cell.unitWidth !== 0) {
        throw new ImpossibleLength(where, declared, this.remaining());
      }
      count = declared / cell.unitWidth;
    }
    let units = this.readWstringUnits(where, count, cell.unitWidth, order);
    if (cell.terminated) {
      if (declared < 1 || units.length === 0) {
        throw new ImpossibleLength(where, declared, this.remaining());
      }
      if (units[units.length - 1] !== 0) {
        throw new OutOfRange(where, "a NUL character ending the wstring, as its count includes one");
      }
      units = units.slice(0, units.length - 1);
      if (units.includes(0)) {
        throw new OutOfRange(where, "no NUL inside a wstring a C peer would stop at");
      }
    }
    if (order === WstringUnitOrder.ByteOrderMark) {
      units = stripByteOrderMark(units);
    }
    let out = "";
    let index = 0;
    while (index < units.length) {
      const unit = units[index] ?? 0;
      if (unit >= 0xd800 && unit <= 0xdbff && index + 1 < units.length) {
        const low = units[index + 1] ?? 0;
        if (low >= 0xdc00 && low <= 0xdfff) {
          out += String.fromCodePoint(0x10000 + ((unit - 0xd800) << 10) + (low - 0xdc00));
          index += 2;
          continue;
        }
      }
      if (unit >= 0xd800 && unit <= 0xdfff) {
        throw new OutOfRange(where, "a UTF-16 sequence with no lone surrogate");
      }
      out += String.fromCharCode(unit);
      index += 1;
    }
    return out;
  }

  readWstringUnits(where, count, width, order) {
    if (count < 0 || count * width > this.remaining()) {
      throw new ImpossibleLength(where, count * width, this.remaining());
    }
    const little = readsLittleAlways(order);
    const units = [];
    for (let index = 0; index < count; index += 1) {
      let unit;
      if (width === 4) {
        if (order === WstringUnitOrder.BigEndianAlways) {
          unit = this.readU32Big();
        } else if (little) {
          unit = this.readU32Little();
        } else {
          unit = this.readU32();
        }
        if (unit > 0xffff) {
          throw new OutOfRange(where, "a four-octet wstring character holding one UTF-16 unit");
        }
      } else if (order === WstringUnitOrder.BigEndianAlways) {
        unit = this.readU16Big();
      } else if (little) {
        unit = this.readU16Little();
      } else {
        unit = this.readU16();
      }
      units.push(unit);
    }
    return units;
  }

  /** `text` cut to at most `octets` UTF-8 octets, never inside a character. */
  trimUtf8(text, octets) {
    if (octets < 0) {
      return "";
    }
    const encoder = new TextEncoder();
    let out = "";
    let used = 0;
    for (const character of text) {
      const width = encoder.encode(character).length;
      if (used + width > octets) {
        break;
      }
      out += character;
      used += width;
    }
    return out;
  }

  /** `text` cut to at most `units` UTF-16 code units. */
  trimUtf16(text, units) {
    if (units < 0) {
      return "";
    }
    if (text.length <= units) {
      return text;
    }
    let end = units;
    const last = text.charCodeAt(end - 1);
    if (end > 0 && last >= 0xd800 && last <= 0xdbff) {
      end -= 1;
    }
    return text.slice(0, end);
  }

  /** `fixed<digits, scale>`, as the decimal string the writer took. */
  readFixed(digits, scale, count) {
    if (this.stopped()) {
      return "";
    }
    if (digits <= 0 || digits > 31 || scale > digits) {
      throw new OutOfRange(
        this.at,
        `a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<${digits}, ${scale}>`,
      );
    }
    let nibbles = [];
    let sign;
    if (
      count === FixedDigitCount.ValuePrecision ||
      count === FixedDigitCount.ValueStringDigits
    ) {
      for (;;) {
        if (nibbles.length >= MAX_FIXED_NIBBLES) {
          throw new OutOfRange(this.at, "a sign nibble within sixteen octets");
        }
        const octet = this.readU8();
        nibbles.push(octet >> 4);
        nibbles.push(octet & 0x0f);
        if ((octet & 0x0f) >= FIRST_SIGN_NIBBLE) {
          break;
        }
      }
      sign = nibbles.pop() ?? 0;
    } else if (count === FixedDigitCount.Declared) {
      const octets = Math.floor((digits + 2) / 2);
      const from = this.take(octets);
      for (let index = 0; index < octets; index += 1) {
        const octet = this.bytes[from + index] ?? 0;
        nibbles.push(octet >> 4);
        nibbles.push(octet & 0x0f);
      }
      sign = nibbles.pop() ?? 0;
      if ((digits + 1) % 2 === 1) {
        nibbles = nibbles.slice(1);
      }
    } else {
      throw new OutOfRange(this.at, `a fixed digit count, not \`${count}\``);
    }
    let text = "";
    for (const nibble of nibbles) {
      if (nibble > 9) {
        throw new OutOfRange(this.at, "BCD digits");
      }
      text += String.fromCharCode(0x30 + nibble);
    }
    text = text.padStart(scale + 1, "0");
    const split = text.length - scale;
    const whole = text.slice(0, split).replace(/^0+(?=.)/u, "");
    const fraction = text.slice(split);
    const out = scale === 0 ? whole : `${whole}.${fraction}`;
    const zero = whole === "0" && !/[1-9]/u.test(fraction);
    return sign === MINUS_NIBBLE && !zero ? `-${out}` : out;
  }

  // -- the bit-bounded kinds ---------------------------------------------

  /**
   * An enumerator's holder, sign-extended from its holder's width.
   * §7.4.3.5.3 rule (5) makes the holder signed.
   */
  readEnum(bitBound) {
    if (this.stopped()) {
      return 0n;
    }
    if (bitBound <= 0 || bitBound > 32) {
      throw new OutOfRange(this.at, `an @bit_bound between 1 and 32, not ${bitBound}`);
    }
    const width = bitBound <= 8 ? 8 : bitBound <= 16 ? 16 : 32;
    const pattern =
      width === 8 ? BigInt(this.readU8()) : width === 16 ? BigInt(this.readU16()) : BigInt(this.readU32());
    const sign = 1n << BigInt(width - 1);
    return (pattern & sign) === 0n ? pattern : pattern - (1n << BigInt(width));
  }

  /** A bitmask's holder. Table 33 of §7.4.1.1.3.2. */
  readBitmask(bitBound) {
    if (this.stopped()) {
      return 0n;
    }
    if (bitBound <= 0 || bitBound > 64) {
      throw new OutOfRange(this.at, `an @bit_bound between 1 and 64, not ${bitBound}`);
    }
    if (bitBound <= 8) {
      return BigInt(this.readU8());
    }
    if (bitBound <= 16) {
      return BigInt(this.readU16());
    }
    if (bitBound <= 32) {
      return BigInt(this.readU32());
    }
    return this.readU64();
  }

  /** A bitset's anchor integer, of the width the planner sized. */
  readBitset(anchorOctets) {
    if (this.stopped()) {
      return 0n;
    }
    switch (anchorOctets) {
      case 1:
        return BigInt(this.readU8());
      case 2:
        return BigInt(this.readU16());
      case 4:
        return BigInt(this.readU32());
      case 8:
        return this.readU64();
      default:
        throw new OutOfRange(this.at, `a bitset anchor of 1, 2, 4 or 8 octets, not ${anchorOctets}`);
    }
  }

  // -- framing -------------------------------------------------------

  /** A sequence's element count, refusing one the stream cannot hold. */
  readSequenceLength() {
    if (this.stopped()) {
      return 0;
    }
    const where = this.at;
    const count = this.readU32();
    if (count > this.remaining()) {
      throw new ImpossibleLength(where, count, this.remaining());
    }
    return count;
  }

  /** A DHEADER: the length the members that follow occupy. §7.4.3.5.3. */
  readDheader() {
    if (this.stopped()) {
      return 0;
    }
    const where = this.at;
    const length = this.readU32();
    if (length > this.remaining()) {
      throw new ImpossibleLength(where, length, this.remaining());
    }
    return length;
  }

  /** An EMHEADER, unpacked into its parts and the NEXTINT it carries. */
  readEmheader() {
    if (this.stopped()) {
      return emHeader(0, 0, false, 0, false);
    }
    const word = this.readU32();
    const lengthCode = (word >>> 28) & 0x7;
    const memberId = word & 0x0fff_ffff;
    const mustUnderstand = word >>> 31 !== 0;
    if (lengthCode === 4) {
      return emHeader(memberId, lengthCode, mustUnderstand, this.readU32(), true);
    }
    if (lengthCode >= 5) {
      const where = this.at;
      const peeked = this.readU32();
      this.at = where;
      return emHeader(memberId, lengthCode, mustUnderstand, peeked, true);
    }
    return emHeader(memberId, lengthCode, mustUnderstand, 0, false);
  }

  /** A PL_CDR1 parameter header in the {@link ParameterRole.Member} role. */
  readPid() {
    return this.readPidAs(ParameterRole.Member);
  }

  /** A PL_CDR1 parameter header, in whichever form it was written. */
  readPidAs(role) {
    if (this.stopped()) {
      return {
        pid: 0,
        length: 0,
        end: true,
        mustUnderstand: false,
        implExtension: false,
        reserved: false,
        ignore: false,
        role,
        savedOrigin: this.origin,
        valueAt: this.at,
      };
    }
    this.alignTo(4);
    const pid = this.readU16();
    const length = this.readU16();
    if ((pid & PID_ID_MASK) === (PID_EXTENDED & PID_ID_MASK)) {
      const memberHeader = this.readU32();
      const wideLength = this.readU32();
      const saved = this.origin;
      this.origin = this.at;
      return {
        pid: memberHeader & E_MEMBER_ID_MASK,
        length: wideLength,
        end: false,
        mustUnderstand: (memberHeader & E_FLAG_MUST_UNDERSTAND) !== 0,
        implExtension: (memberHeader & E_FLAG_IMPL_EXTENSION) !== 0,
        reserved: false,
        ignore: false,
        role,
        savedOrigin: saved,
        valueAt: this.at,
      };
    }
    const identifier = pid & PID_ID_MASK;
    const end = identifier === (PID_LIST_END & PID_ID_MASK);
    const flagged =
      (pid & PID_IMPL_EXTENSION) !== 0 &&
      (pidFlagsAt(this.plCdr1.pidFlags, "member") & PID_IMPL_EXTENSION) === 0;
    const saved = this.origin;
    this.origin = this.at;
    return {
      pid: identifier,
      length,
      end,
      mustUnderstand: (pid & PID_MUST_UNDERSTAND) !== 0,
      implExtension: flagged,
      reserved: !end && identifier > this.plCdr1.shortMaxId,
      ignore: identifier === PID_IGNORE,
      role,
      savedOrigin: saved,
      valueAt: this.at,
    };
  }

  /** End a parameter's value and step to the next header. */
  finishPidRead(header) {
    this.origin = header.savedOrigin;
    if (this.stopped() || header.end) {
      return;
    }
    const used = this.at - header.valueAt;
    const padded = (used + 3) & ~3;
    const pads =
      header.role === ParameterRole.Optional
        ? this.plCdr1.padsOptionalLength
        : this.plCdr1.padsDeclaredLength;
    if (header.length !== (pads ? padded : used) && header.length !== used && header.length !== padded) {
      this.readerRefuse(Status.OutOfRange);
    }
    this.alignTo(4);
  }

  /** Step over a member this reader does not know. */
  skipMember(header) {
    if (this.stopped()) {
      return;
    }
    if (header.lengthCode > 3 && !header.hasNextInt) {
      throw new OutOfRange(this.at, "a NEXTINT for a length code of 4 or above");
    }
    this.skip(header.extent);
  }

  /** Start reading a parameter's value, answering with the origin it replaces. */
  beginParameterValue() {
    const saved = this.origin;
    this.origin = this.at;
    return saved;
  }

  /** End a parameter's value and step over its trailing padding. */
  finishParameterValue(savedOrigin) {
    this.origin = savedOrigin;
    this.alignTo(4);
  }

  // -- text -----------------------------------------------------------

  decodeUtf8(from, length, where) {
    try {
      return new TextDecoder("utf-8", { fatal: true }).decode(this.bytes.subarray(from, from + length));
    } catch {
      throw new NotUtf8(where);
    }
  }
}

/** Whether a parameter can be one of the reading type's members. */
function pidIsUserMember(header) {
  return !header.implExtension && !header.reserved && !header.ignore;
}

/** How many octets `text` is as UTF-8, which is what a `string<N>` bound counts. */
function utf8Octets(text) {
  return new TextEncoder().encode(text).length;
}

/** How many UTF-16 code units `text` is, which is what a `wstring<N>` bound counts. */
function utf16Units(text) {
  return text.length;
}

/**
 * What a `wstring` length policy answers, as its three coordinates.
 */
const WSTRING_CELLS = new Map([
  [WstringLengthPolicy.Specification, { countsCharacters: false, terminated: false, unitWidth: 2 }],
  [WstringLengthPolicy.CharacterCount, { countsCharacters: true, terminated: false, unitWidth: 2 }],
  [
    WstringLengthPolicy.CharacterCountTerminated,
    { countsCharacters: true, terminated: true, unitWidth: 4 },
  ],
  [
    WstringLengthPolicy.SpecificationWideUnits,
    { countsCharacters: false, terminated: false, unitWidth: 4 },
  ],
  [WstringLengthPolicy.RosTypeSupport, { countsCharacters: true, terminated: false, unitWidth: 4 }],
  [WstringLengthPolicy.OctetsTerminated2, { countsCharacters: false, terminated: true, unitWidth: 2 }],
  [WstringLengthPolicy.OctetsTerminated4, { countsCharacters: false, terminated: true, unitWidth: 4 }],
  [
    WstringLengthPolicy.CharactersTerminated2,
    { countsCharacters: true, terminated: true, unitWidth: 2 },
  ],
]);

/** Whether a unit order reads its code units little-endian regardless of the stream. */
function readsLittleAlways(order) {
  return order === WstringUnitOrder.HostUnswapped || order === WstringUnitOrder.ByteOrderMark;
}

/** An `EmHeaderRead` with its `extent` worked out. */
function emHeader(memberId, lengthCode, mustUnderstand, nextInt, hasNextInt) {
  let extent;
  if (lengthCode <= 3) {
    extent = 1 << lengthCode;
  } else if (lengthCode === 4) {
    extent = nextInt;
  } else {
    const scale = lengthCode === 5 ? 1 : lengthCode === 6 ? 4 : 8;
    extent = 4 + nextInt * scale;
  }
  return { memberId, lengthCode, mustUnderstand, nextInt, hasNextInt, extent };
}

/** Take the byte-order mark off the front and apply what it said. */
function stripByteOrderMark(units) {
  if (units.length === 0) {
    return units;
  }
  if (units[0] === BYTE_ORDER_MARK) {
    return units.slice(1);
  }
  const rest = units[0] === SWAPPED_BYTE_ORDER_MARK ? units.slice(1) : units;
  return rest.map((unit) => ((unit & 0xff) << 8) | (unit >> 8));
}

/** The widest `fixed` IDL admits is `fixed<31, S>` — sixteen octets. */
const MAX_FIXED_NIBBLES = 32;

/** The first nibble value [CDR] §9.3.2.8 reserves for a sign. */
const FIRST_SIGN_NIBBLE = 0x0a;

/** The sign nibble that means negative. [CDR] §9.3.2.8. */
const MINUS_NIBBLE = 0x0d;

module.exports = {
  Endianness,
  isLittle,
  parseEndianness,
  WireError,
  Truncated,
  ImpossibleLength,
  NotUtf8,
  OutOfRange,
  WcharWidth,
  WcharFraming,
  WstringUnitOrder,
  WstringLengthPolicy,
  FixedDigitCount,
  ParameterRole,
  TryConstruct,
  Status,
  PID_FLAGS_NONE,
  PID_FLAGS_BARE,
  PID_FLAGS_SENTINEL_MUST_UNDERSTAND,
  PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS,
  PID_SHORT_MAX_ID,
  PID_EXTENDED,
  PID_LIST_END,
  PID_IGNORE,
  KEY_HASH_LEN,
  PL_CDR1_DEFAULT,
  noDheader,
  zdNumber,
  zdBigInt,
  zdChar,
  float128FromDecimalString,
  float128ToDecimalString,
  bitPatternInt,
  bitPatternBool,
  bitPatternF32,
  bitPatternF64,
  Writer,
  Reader,
  pidIsUserMember,
  utf8Octets,
  utf16Units,
};
