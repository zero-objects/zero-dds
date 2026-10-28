// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The write half of XCDR, implemented from DDS-XTypes 1.3 §7.4.
 *
 * The fifth independent runtime beside `crates/wire`, `runtime/c`,
 * `runtime/cpp`, `runtime/java` and `runtime/python`. None of them
 * calls another, and that is the point: a difference between two of
 * them is a finding about the clause, not about a shared bug.
 *
 * What TypeScript decides differently from the others:
 *
 * - a `number` is an IEEE 754 binary64, so every integer narrower than
 *   2^53 is one and everything wider is a `bigint`. The boundary is the
 *   type's, not the wire's — `long long` is a `bigint` because a
 *   binary64 cannot hold every `i64`, and `long` is a `number` because
 *   it can hold every `i32`;
 * - a range check happens at the point of the write. In Rust an `i16`
 *   is an `i16` and the compiler has refused the value already; here
 *   `70000` is as much a `number` as `7` is, and `& 0xffff` would put
 *   `4464` on the wire and report success;
 * - a `long double` never becomes a `number`. Sixteen octets in and
 *   sixteen octets out; a binary64 would drop sixty bits of the
 *   significand at the assignment.
 */

import { Endianness, isLittle } from "./endianness.js";
import { OutOfRange } from "./error.js";
import {
  FixedDigitCount,
  ParameterRole,
  WcharFraming,
  WcharWidth,
  WstringLengthPolicy,
  WstringUnitOrder,
  wstringCountsCharacters,
  wstringIsTerminated,
  wstringOrderIsFixedLittle,
  wstringUnitWidth,
} from "./policy.js";

/** The parameter id that introduces the extended form. §7.4.1.2.2. */
export const PID_EXTENDED = 0x3f01;

/** The parameter id that ends a parameter list. §7.4.1.2.4. */
export const PID_LIST_END = 0x3f02;

/**
 * The highest member id a short parameter header can carry.
 *
 * §7.4.1.2.2 gives the range as 0 to 0x3F00 *inclusive*, and
 * PID_EXTENDED is one above it.
 */
export const PID_SHORT_MAX_ID = 0x3f00;

/** The implementation-extension flag of a parameter id. */
export const PID_IMPL_EXTENSION = 0x8000;

/** The must-understand flag of a parameter id. */
export const PID_MUST_UNDERSTAND = 0x4000;

/** `FLAG_2` of an `eMemberHeader`. §7.4.1.2.1. */
const E_FLAG_MUST_UNDERSTAND = 0x4000_0000;

/** The member id an `eMemberHeader` carries. §7.4.1.2.1. */
const E_MEMBER_ID_MASK = 0x0fff_ffff;

/** How many octets a KeyHash holder has. RTPS 2.5 §9.6.3.3. */
export const KEY_HASH_LEN = 16;

/**
 * The longest value a short parameter header's `ssize` can declare.
 *
 * §7.4.1.2.2's `ssize` is sixteen bits. Which values overrun it is a
 * property of the *sample* and not of the type — a `string` member is
 * short in almost every sample and over 64 KiB in some — so the layout
 * plan writes the short form and {@link Writer.finishPid} promotes it
 * where the length turns out not to fit.
 */
const PID_SHORT_MAX_LEN = 0xffff;

/** The code unit a byte order mark is. Unicode 15 §23.8. */
const BYTE_ORDER_MARK = 0xfeff;

/**
 * Where a framing may put flags.
 *
 * Wider than `ParameterRole` in `policy.ts`, which is what a
 * *generated codec* names: a sentinel and an extended marker are the
 * framing's own parameters and no member is ever one.
 */
export type FlagPosition = "member" | "sentinel" | "extended-marker";

/** Where a framing puts its flags on a parameter id. */
export interface PidFlags {
  readonly member: number;
  readonly sentinel: number;
  readonly extendedMarker: number;
}

/** No flags anywhere, which is Fast DDS's framing. */
export const PID_FLAGS_NONE: PidFlags = {
  member: 0,
  sentinel: 0,
  extendedMarker: 0,
};

/** The name `PidFlagPolicy::Bare` gives {@link PID_FLAGS_NONE}. */
export const PID_FLAGS_BARE: PidFlags = PID_FLAGS_NONE;

/**
 * A bare member id, the must-understand bit on the sentinel and on the
 * extended marker. Cyclone DDS and RTI Connext 7.7. The default.
 *
 * Measured, on `struct-mutable` and `annotation-hashid-implicit` under
 * XCDR1: both write `7f02` for the sentinel and `7f01` for the extended
 * marker — the must-understand bit alone. The implementation-extension
 * bit is **not** on either of them, and a runtime that also set it wrote
 * `ff02` and `ff01`, which is what OpenDDS writes for the marker and
 * nobody writes for the sentinel.
 */
export const PID_FLAGS_SENTINEL_MUST_UNDERSTAND: PidFlags = {
  member: 0,
  sentinel: PID_MUST_UNDERSTAND,
  extendedMarker: PID_MUST_UNDERSTAND,
};

/**
 * Both flags on every member id whatever the member declared, a bare
 * sentinel, and both on the extended marker. OpenDDS.
 *
 * `annotation-hashid-implicit` is `ff010008 0343172c 00000004`: both
 * flags on the marker, and a **bare** `eMemberHeader` behind it for a
 * member that declares nothing.
 */
export const PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS: PidFlags = {
  member: PID_MUST_UNDERSTAND | PID_IMPL_EXTENSION,
  sentinel: 0,
  extendedMarker: PID_MUST_UNDERSTAND | PID_IMPL_EXTENSION,
};

/** The flags a framing puts on a parameter in `role`. */
export function pidFlagsAt(flags: PidFlags, role: FlagPosition): number {
  switch (role) {
    case "member":
      return flags.member;
    case "sentinel":
      return flags.sentinel;
    case "extended-marker":
      return flags.extendedMarker;
  }
}

/**
 * The parts of PL_CDR1 framing a plan cannot answer.
 *
 * Every field is a measured divergence between implementations, not a
 * reading of the clause: §7.4.1.2 leaves the padding of a declared
 * length open, and the flags a vendor puts on every id are its own.
 */
export interface PlCdr1Framing {
  /** Which flags ride on every parameter id, by role. */
  readonly pidFlags: PidFlags;
  /**
   * The highest id written in the short form. `writePid` refuses an id
   * above it: §7.4.1.2.2 reserves the range above for the protocol's
   * own parameters.
   */
  readonly shortMaxId: number;
  /** Whether a declared length covers the value's trailing padding. */
  readonly padsDeclaredLength: boolean;
  /** The same, for an `@optional` member outside a `@mutable` type. */
  readonly padsOptionalLength: boolean;
  /** Whether a member's own must-understand reaches its id. */
  readonly memberMustUnderstand: boolean;
  /**
   * Whether the parameter that just ended writes its own trailing
   * padding to the next four-byte boundary. Every header writer aligns
   * itself, so the two readings only part at the end of the stream:
   * RTI pads after a final `@optional` parameter, Fast DDS does not.
   */
  readonly padAfterValue: boolean;
}

/** The framing every field of which is the specification's reading. */
export const PL_CDR1_DEFAULT: PlCdr1Framing = {
  pidFlags: PID_FLAGS_NONE,
  shortMaxId: PID_SHORT_MAX_ID,
  padsDeclaredLength: false,
  padsOptionalLength: false,
  memberMustUnderstand: true,
  padAfterValue: true,
};

/**
 * What `writeDheader` hands back, to be given to `finishDheader`.
 *
 * A `@final` aggregate has none, and the walk still declares one and
 * finishes it — so the absence is a value of this type rather than
 * `undefined`. `at` below zero is that absence; `finishDheader`
 * ignores it. The alternative is `DHeader | undefined` in every
 * generated signature and a null check at every finish, for a case the
 * layout plan already decided.
 */
export interface DHeader {
  readonly at: number;
}

/** The DHEADER a `@final` aggregate does not write. */
export function noDheader(): DHeader {
  return { at: -1 };
}

/** What `writeEmheader` hands back. */
export interface EmHeader {
  /** Where the NEXTINT is, or `undefined` where the code carries it. */
  readonly nextInt: number | undefined;
  /** Where the header word is, so a short code can rewrite it. */
  readonly word: number;
  /**
   * The code the header was written with, so that `finishEmheader` can
   * hold the member's value to the length the code declares. Codes 0 to
   * 3 are lengths; anything else declares nothing of its own.
   */
  readonly lengthCode: number;
}

/** What `writePid` and `writePidExtended` hand back. */
export interface Pid {
  readonly at: number;
  readonly extended: boolean;
  readonly savedOrigin: number;
  /**
   * What the parameter frames.
   *
   * A {@link ParameterRole} and not a {@link FlagPosition}: whether the
   * declared length covers the value's trailing padding is answered
   * differently for an `@optional` member outside a `@mutable` type than
   * for a member of one, and {@link Writer.finishPid} needs the answer
   * after the value has been written. A header that carried the flag
   * position instead had no way to reach `padsOptionalLength` at all,
   * so every `@optional` under XCDR1 took the member's padding rule.
   */
  readonly role: ParameterRole;
  readonly pid: number;
  readonly mustUnderstand: boolean;
}

/**
 * An IDL `long double`, as the sixteen octets it is.
 *
 * Declared here beside {@link Writer.writeF128} and `Reader.readF128`,
 * the two operations that traffic in it, rather than in `float128.ts`
 * with the decimal converter. The converter answers a `Uint8Array` and
 * needs no name for the wrapper; a reader that had to import the
 * wrapper from a third module would be one more edge for no gain.
 */
export interface Float128 {
  /** The sixteen octets, in the order the wire carries them. */
  readonly octets: Uint8Array;
}


export class Writer {
  private out: number[] = [];
  private origin = 0;
  private readonly maxAlignment: number;

  /**
   * A writer in `order` whose alignment is capped at `maxAlign`.
   *
   * Four for XCDR2, eight for XCDR1. §7.4.1: `ALIGN(N)` aligns to
   * `MIN(N, XCDR.maxalign)`. A writer that did not know this padded an
   * eight-octet primitive to eight under XCDR2 and put four octets on
   * the wire that no peer expects.
   */
  constructor(
    readonly order: Endianness,
    maxAlign = 8,
    readonly plCdr1: PlCdr1Framing = {
      pidFlags: PID_FLAGS_NONE,
      shortMaxId: PID_SHORT_MAX_ID,
      padsDeclaredLength: false,
      padsOptionalLength: false,
      memberMustUnderstand: true,
      padAfterValue: true,
    },
    public shortLengthCodes = false,
  ) {
    this.maxAlignment = Math.max(1, maxAlign);
  }

  /** The alignment cap in force. */
  get maxAlign(): number {
    return this.maxAlignment;
  }

  /** How many octets have been written. */
  position(): number {
    return this.out.length;
  }

  /** The octets written so far. */
  finish(): Uint8Array {
    return Uint8Array.from(this.out);
  }

  /** Pad to `align`, counted from the stream's origin. */
  alignTo(align: number): void {
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
   * Returns the origin displaced, for {@link finishPackedElement}.
   */
  beginPackedElement(): number {
    const saved = this.origin;
    this.origin = this.out.length;
    return saved;
  }

  /** Restore the origin {@link beginPackedElement} displaced. */
  finishPackedElement(saved: number): void {
    this.origin = saved;
  }

  private refuse(expected: string): never {
    throw new OutOfRange(this.out.length, expected);
  }

  private check(value: number, low: number, high: number, what: string): void {
    if (!Number.isInteger(value) || value < low || value > high) {
      this.refuse(`${what} in [${low}, ${high}], not ${value}`);
    }
  }

  private checkBig(value: bigint, low: bigint, high: bigint, what: string): void {
    if (value < low || value > high) {
      this.refuse(`${what} in [${low}, ${high}], not ${value}`);
    }
  }

  private raw(octets: Uint8Array | number[]): void {
    for (const octet of octets) {
      this.out.push(octet & 0xff);
    }
  }

  /** An integer of `width` octets, in the stream's order. */
  private writeInt(value: bigint, width: number, align = width): void {
    this.alignTo(align);
    this.rawInt(value, width, isLittle(this.order));
  }

  /**
   * An integer of `width` octets in a named order, with no alignment.
   *
   * The order is an argument because two forms fix it against the
   * stream: CORBA's length-prefixed `wchar` and the `wstring` unit
   * orders. Everything else goes through {@link writeInt}.
   */
  private rawInt(value: bigint, width: number, little: boolean): void {
    const octets = new Array<number>(width);
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

  // -- primitives ------------------------------------------------------

  /** `boolean`, one octet, 0 or 1. */
  writeBool(value: boolean): void {
    this.out.push(value ? 1 : 0);
  }

  /** `octet` and `uint8`. */
  writeU8(value: number): void {
    this.check(value, 0, 0xff, "an octet");
    this.out.push(value);
  }

  /** `int8`. */
  writeI8(value: number): void {
    this.check(value, -0x80, 0x7f, "an int8");
    this.out.push(value & 0xff);
  }

  /**
   * `char`, one octet.
   *
   * A code point above 255 is refused rather than truncated: a `char`
   * field holding `ä` from a UTF-8 source is a mistake worth stopping
   * on, and `0xe4` is a different character.
   */
  writeChar(value: string): void {
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
   *
   * Two octets under Table 31 and for seven of the nine measured
   * implementations; four under RTI's XCDR1, under TAO and under
   * `rosidl_typesupport_fastrtps_c`. The width is an argument of its
   * own and not a reading of the `wstring` policy, because
   * `rmw_cyclonedds_cpp` writes two here and four inside a `wstring`.
   *
   * The BMP restriction of §7.2.2.2.1.2.5 is a property of the type
   * and not of the width: the four-octet form zero-extends a code
   * unit, it does not carry a code point.
   *
   * `order` reaches only the length-prefixed form: a scalar cannot
   * carry a byte order mark, so anything but the stream's order means
   * CORBA's default, which is big-endian.
   */
  writeWchar(
    value: string,
    width: WcharWidth = WcharWidth.Octets2,
    framing: WcharFraming = WcharFraming.Fixed,
    order: WstringUnitOrder = WstringUnitOrder.Stream,
  ): void {
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
      // CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 form: the octet count
      // first, then the code unit's octets, all aligned to one. The
      // count is the transmission code set's width and not the value's,
      // so it is the same octet for every character. Measured
      // 2026-08-10: omniORB writes `02 00e4` for U+00E4 in either
      // stream order and TAO `04 e4000000`.
      this.out.push(width);
      const little = order === WstringUnitOrder.Stream && isLittle(this.order);
      this.rawInt(BigInt(point), width, little);
      return;
    }
    this.writeInt(BigInt(point), width);
  }

  /** `short`. */
  writeI16(value: number): void {
    this.check(value, -0x8000, 0x7fff, "a short");
    this.writeInt(BigInt(value), 2);
  }

  /** `unsigned short`. */
  writeU16(value: number): void {
    this.check(value, 0, 0xffff, "an unsigned short");
    this.writeInt(BigInt(value), 2);
  }

  /** `long`. */
  writeI32(value: number): void {
    this.check(value, -0x8000_0000, 0x7fff_ffff, "a long");
    this.writeInt(BigInt(value), 4);
  }

  /** `unsigned long`. */
  writeU32(value: number): void {
    this.check(value, 0, 0xffff_ffff, "an unsigned long");
    this.writeInt(BigInt(value), 4);
  }

  /** `long long`, which is a `bigint` because a binary64 is not one. */
  writeI64(value: bigint): void {
    this.checkBig(value, -(2n ** 63n), 2n ** 63n - 1n, "a long long");
    this.writeInt(value, 8);
  }

  /** `unsigned long long`. */
  writeU64(value: bigint): void {
    this.checkBig(value, 0n, 2n ** 64n - 1n, "an unsigned long long");
    this.writeInt(value, 8);
  }

  /**
   * The Cyclone union overlay: the low `octets` of `pattern`, written
   * as one unsigned integer of that width in the stream's byte order.
   *
   * A discriminator carrying a label the `union-labels` dialect axis
   * says selects nothing makes the peer fall through to `default:` and
   * serialise its one C union storage *as that arm* — the selected
   * member's octets, reinterpreted. The pattern is the `bigint` whose
   * little-endian bytes are the selected member's little-endian
   * storage octets, zero-extended; the `bitPattern*` functions
   * produce it.
   */
  writeUnionOverlay(pattern: bigint, octets: number): void {
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

  /**
   * `float`, a binary32.
   *
   * The value is rounded once, here, and not by the caller: a `number`
   * is a binary64 and the narrowing is part of writing the field.
   */
  writeF32(value: number): void {
    this.alignTo(4);
    const buffer = new DataView(new ArrayBuffer(4));
    buffer.setFloat32(0, value, isLittle(this.order));
    this.raw(new Uint8Array(buffer.buffer));
  }

  /** `double`, a binary64. */
  writeF64(value: number): void {
    this.alignTo(8);
    const buffer = new DataView(new ArrayBuffer(8));
    buffer.setFloat64(0, value, isLittle(this.order));
    this.raw(new Uint8Array(buffer.buffer));
  }

  /**
   * `long double`, as the sixteen octets it is.
   *
   * The octets arrive in the order the wire carries them for a
   * little-endian stream, which is how `ZdFloat128` holds them, and are
   * reversed for a big-endian one. A `number` never appears: a binary64
   * would drop sixty bits of the significand.
   */
  writeF128(value: Float128): void {
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
   * `string`: a four-octet length that **includes** the terminator,
   * then the octets, then a NUL. §7.4.1.1.1.
   */
  writeString(value: string): void {
    const octets = new TextEncoder().encode(value);
    this.writeU32(octets.length + 1);
    this.raw(octets);
    this.out.push(0);
  }

  /**
   * A bounded `string`: the same bytes {@link writeString} writes. The
   * bound is a promise about the source and not a difference on the
   * wire, so this is the only place it can be enforced — refused rather
   * than cut at a character boundary, which would put a sample on the
   * wire nobody wrote. The bound counts octets, which is what a
   * `string<N>` bound counts for every measured implementation.
   */
  writeStringBounded(value: string, bound: number): void {
    const octets = new TextEncoder().encode(value).length;
    if (octets > bound) {
      this.refuse(`at most ${bound} octets in a bounded string, and this one has ${octets}`);
    }
    this.writeString(value);
  }

  /**
   * A bounded `wstring`, as {@link writeStringBounded} is to
   * {@link writeString}. The bound counts UTF-16 code units, not
   * characters and not octets, whatever `policy` puts on the wire.
   */
  writeWstringBounded(
    value: string,
    bound: number,
    policy: WstringLengthPolicy,
    order: WstringUnitOrder,
  ): void {
    this.checkWstringBound(value, bound);
    this.writeWstringUtf8(value, policy, order);
  }

  /**
   * The count check every bounded `sequence` makes before a byte of it
   * is written. Table 17 makes `O2.length <= T1.length` the condition
   * for constructing a `sequence<T, N>`, and the wire carries no bound
   * to compare against: a sender whose count already exceeds its own
   * declaration has written a value of a type nobody declared.
   */
  checkSequenceBound(actual: number, bound: number): void {
    if (actual > bound) {
      this.refuse(`at most ${bound} elements in a bounded sequence, and this one has ${actual}`);
    }
  }

  /**
   * The count check every bounded `map` makes before a byte of it is
   * written. Its own message and not {@link checkSequenceBound}'s: the
   * unit is an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
   * map-as-a-sequence-of-pairs equivalence.
   */
  checkMapBound(actual: number, bound: number): void {
    if (actual > bound) {
      this.refuse(`at most ${bound} entries in a bounded map, and this one has ${actual}`);
    }
  }

  /**
   * `wstring`: a four-octet **octet** count, then UTF-16 code units.
   *
   * No BOM and no terminator — §7.4.1.1.2 forbids both outright. The
   * order the units go out in is the stream's, which the encapsulation
   * identifier already announced.
   *
   * The specification cell of {@link writeWstringUtf8}, kept as a
   * method of its own because it is the one every reader of this file
   * looks for.
   */
  writeWstring(value: string): void {
    this.writeWstringUtf8(
      value,
      WstringLengthPolicy.Specification,
      WstringUnitOrder.Stream,
    );
  }

  /**
   * A `wstring` in the form `policy` and `order` name.
   *
   * {@link WstringLengthPolicy.Specification} is a four-octet octet
   * count and then UTF-16 code units. No BOM and no terminator —
   * §7.4.1.1.2 forbids both outright: "The UTF-16 representation … shall
   * not include a Byte Order Mark (BOM). The representation shall also
   * not include any terminating NUL character(s)."
   *
   * The count is of **octets** under that policy and not of characters:
   * §7.4.1.1.2 says "This is twice the number of characters in the
   * string", with the clause's own rationale being that a code point
   * outside the Basic Multilingual Plane takes two units and an octet
   * count still reaches the end. So `"𝄞"` is a length of four.
   *
   * A TypeScript `string` already holds UTF-16 code units, which is the
   * one thing this column gets for free — `charCodeAt` is the unit and
   * `codePointAt` the character, and the two differ exactly where the
   * policies do.
   */
  writeWstringUtf8(
    value: string,
    policy: WstringLengthPolicy,
    order: WstringUnitOrder,
  ): void {
    const width = wstringUnitWidth(policy);
    const counts = wstringCountsCharacters(policy);
    if (counts && width === 2) {
      // Fast CDR counts characters and writes each four-octet `wchar_t`
      // as two octets, so a code point outside the BMP goes out as its
      // low sixteen bits and arrives as a different character. A
      // vendor's length convention is a policy; its data loss is not, so
      // the value is refused. The refusal is on the pair (characters,
      // two octets) and not on a named form: a character count over
      // four-octet units carries the surrogate pair intact.
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
    // CORBA §9.3.1.6's GIOP 1.1 rule, and RTI's XCDR1 form: a NUL code
    // unit at the end, inside whatever the length counts.
    //
    // An empty `wstring` carries no mark: omniORB writes `00000000` for
    // one, measured 2026-08-10 and byte-identical to every other
    // implementation. A mark on an empty string would be a two-octet
    // string that decodes to nothing, and no peer writes it.
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
   * is the UTF-8 octet count and only C2 — the terminator — still reaches
   * the wire. `wstring-member` is `04 00 00 00 61 62 63 00` for `"abc"`:
   * three octets, a NUL, and the four declares both.
   */
  writeWstringUtf8Octets(value: string, policy: WstringLengthPolicy): void {
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
  writeWstringUtf8OctetsBounded(
    value: string,
    bound: number,
    policy: WstringLengthPolicy,
  ): void {
    this.checkWstringBound(value, bound);
    this.writeWstringUtf8Octets(value, policy);
  }

  /**
   * Refuses a `wstring<N>` value longer than its bound. The bound counts
   * UTF-16 code units, whatever form the wire carries.
   */
  private checkWstringBound(value: string, bound: number): void {
    const units = value.length;
    if (units > bound) {
      this.refuse(
        `at most ${bound} UTF-16 code units in a bounded wstring, and this one has ${units}`,
      );
    }
  }

  /** One `wstring` code unit, at its width and in its order. */
  private writeWstringUnit(unit: number, width: 2 | 4, order: WstringUnitOrder): void {
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

  /**
   * `fixed<digits, scale>` as packed BCD. [CDR] §9.3.2.8.
   *
   * DDS-XTypes 1.3 does not carry the type at all — §7.2.2 has no
   * fixed-point kind and §7.4.3.5.3 has no rule whose left-hand side
   * matches one — so the form is CORBA's, which is where an IDL `fixed`
   * came from and the only wire definition it has.
   *
   * The value arrives as a `string` because a `number` is a binary64 and
   * `fixed<31, 2>` is exactly the case a binary64 cannot carry. It is
   * not parsed through anything either: a parser that accepted an
   * exponent or normalised a scale would change the digit count, which
   * is the measured axis {@link FixedDigitCount} names.
   *
   * Aligned to one. [CDR] §9.3.2.8 gives a fixed-point value an
   * alignment of one octet, so a `fixed` never pads.
   */
  writeFixed(value: string, digits: number, scale: number, count: FixedDigitCount): void {
    const nibbles = this.fixedNibbles(value, digits, scale, count);
    for (let index = 0; index + 1 < nibbles.length; index += 2) {
      this.out.push((((nibbles[index] ?? 0) << 4) | (nibbles[index + 1] ?? 0)) & 0xff);
    }
  }

  /**
   * The BCD nibbles of `text` under `fixed<digits, scale>`.
   *
   * [CDR] §9.3.2.8: digit nibbles most-significant first, then a sign
   * nibble — `0xc` for positive or zero, `0xd` for negative — with a
   * leading zero nibble prepended when the nibble count is odd so the
   * nibbles pair up.
   *
   * Neither half is padded in the direction that would lose a digit: a
   * value too long for its integer part or its fraction is one this type
   * cannot represent, and is refused.
   *
   * How many digits go out is {@link FixedDigitCount}: the clause gives
   * the packing and says nothing about the count, and the four
   * implementations that write a `fixed` split two and two.
   */
  private fixedNibbles(
    text: string,
    digits: number,
    scale: number,
    count: FixedDigitCount,
  ): number[] {
    if (digits === 0) {
      this.refuse("a fixed with at least one digit");
    }
    // OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
    // fixed<31, S> is the widest declaration IDL admits. A wider one has
    // no packing defined for it, so it is refused rather than packed into
    // octets no reader can be told the width of.
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
    if (!/^\d*$/.test(whole) || !/^\d*$/.test(fraction)) {
      this.refuse(bad);
    }
    const asWritten = whole.length + scale;
    const integralWidth = digits - scale;
    whole = whole.replace(/^0+/, "");
    if (whole.length > integralWidth) {
      this.refuse(
        `a decimal whose integer part fits ${integralWidth} digits, not \`${text}\``,
      );
    }
    if (fraction.length > scale) {
      this.refuse(`a decimal whose fraction fits ${scale} digits, not \`${text}\``);
    }

    // The full `digits`-wide field first, whatever the count: the
    // value's precision is that field with its leading zeros removed, so
    // both counts are read off one construction.
    const every = new Array<number>(digits).fill(0);
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
    const nibbles: number[] = [];
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
    // A negative zero is written positive, because there is one zero.
    nibbles.push(negative && significant > 0 ? 0x0d : 0x0c);
    return nibbles;
  }

  // -- aggregates ------------------------------------------------------

  /** A DHEADER: a four-octet length of what follows. §7.4.3.5.3. */
  writeDheader(): DHeader {
    this.alignTo(4);
    const at = this.out.length;
    this.raw([0, 0, 0, 0]);
    return { at };
  }

  /**
   * Patch a DHEADER with the length of what followed it.
   *
   * A header from {@link noDheader} is not there and is ignored, which
   * is what lets a generated codec write the same two lines for a
   * `@final` aggregate and an `@appendable` one.
   */
  finishDheader(header: DHeader): void {
    if (header.at < 0) {
      return;
    }
    this.patch(header.at, this.out.length - header.at - 4, 4);
  }

  /**
   * An XCDR2 EMHEADER: the member id, its length code and the
   * must-understand flag in one 32-bit word. §7.4.3.4.2.
   *
   * The length code is the layout plan's decision and arrives here as
   * a number; this runtime does not choose one.
   *
   * LC0 through LC3 carry the length in the code itself. LC4 is
   * followed by a NEXTINT. LC5 through LC7 also have one, and
   * §7.4.3.4.2 says it "shall be reused also as part of the serialized
   * member", so no placeholder is reserved for them: the member's own
   * leading word lands in that position and *is* the NEXTINT.
   */
  writeEmheader(memberId: number, lengthCode: number, mustUnderstand: boolean): EmHeader {
    // §7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and §7.4.3.4.2
    // leaves them the low 28 bits of the word. Masking a larger one
    // writes a well-formed header for `id & 0x0FFFFFFF`: the peer reads
    // a member the caller did not name, and nothing at either end says
    // so.
    if (!Number.isInteger(memberId) || memberId < 0 || memberId > E_MEMBER_ID_MASK) {
      this.refuse(`a member id inside the 28 bits of an EMHEADER, not ${memberId}`);
    }
    this.alignTo(4);
    const word =
      ((mustUnderstand ? 1 : 0) << 31) |
      ((lengthCode & 0x7) << 28) |
      (memberId & 0x0fff_ffff);
    this.writeInt(BigInt(word >>> 0), 4);
    const wordAt = this.out.length - 4;
    if (lengthCode === 4) {
      const at = this.out.length;
      this.raw([0, 0, 0, 0]);
      return { nextInt: at, word: wordAt, lengthCode };
    }
    return { nextInt: undefined, word: wordAt, lengthCode };
  }

  /**
   * Patch an EMHEADER's NEXTINT with the length of what followed.
   *
   * Under {@link shortLengthCodes} an LC4 header whose member turned
   * out to be 1, 2, 4 or 8 octets instead loses its NEXTINT and states
   * the length in the code, which is what Fast DDS does.
   */
  finishEmheader(header: EmHeader): void {
    if (header.nextInt === undefined) {
      // Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
      // octets, §7.4.3.4.2. A value of another size misdeclares it and
      // the reader's skip lands inside data, so the mismatch is refused
      // here rather than written. Codes 5 to 7 declare nothing of their
      // own: the member's leading length word is the NEXTINT.
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

  /**
   * Remove an LC4 header's NEXTINT and raise its length code.
   *
   * Every octet of the member's value is unchanged and only its
   * position moves, which is safe for one reason and only under XCDR2:
   * MAXALIGN is four there, so the value began at a four-aligned
   * offset with the NEXTINT in front of it and begins at a four-aligned
   * offset without it.
   */
  private shortenEmheader(header: EmHeader, length: number): void {
    const at = header.nextInt;
    if (at === undefined) {
      return;
    }
    const code = { 1: 0, 2: 1, 4: 2, 8: 3 }[length as 1 | 2 | 4 | 8];
    this.out.splice(at, 4);
    const previous = this.read(header.word, 4);
    this.patch(header.word, ((previous & ~(0x7 << 28)) | (code << 28)) >>> 0, 4);
  }

  // -- parameter lists -------------------------------------------------

  /**
   * A PL_CDR1 parameter header, short form: `[id:u16][length:u16]`.
   *
   * `mustUnderstand` is bit 14 of the id. §7.4.1.2.2: the flag "shall
   * be set if and only if the must_understand property of the member is
   * set to true".
   */
  writePid(
    pid: number,
    role: ParameterRole = ParameterRole.Member,
    mustUnderstand = false,
  ): Pid {
    // §7.4.1.2.2 gives the short form the ids 0 to `shortMaxId`
    // inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
    // itself. A short header with a reserved id collides with the
    // protocol's own parameters, so such an id takes the extended form
    // or none.
    if (!Number.isInteger(pid) || pid < 0 || pid > this.plCdr1.shortMaxId) {
      this.refuse(
        `a parameter id the short header carries, not ${pid} — the extended header takes it`,
      );
    }
    this.alignTo(4);
    // Two claimants on the same word, deliberately: the member's own
    // declaration, and whatever the framing puts on every id regardless.
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

  /**
   * A PL_CDR1 parameter header, extended form.
   *
   * `[PID_EXTENDED][8][id:u32][length:u32]`, required when the id
   * reaches the reserved range or the value is longer than `0xFFFF`.
   * §7.4.1.2.
   */
  writePidExtended(
    pid: number,
    role: ParameterRole = ParameterRole.Member,
    mustUnderstand = false,
  ): Pid {
    // The same 28 bits as an EMHEADER's, here in the `eMemberHeader` of
    // §7.4.1.2.1. Masking aliases the parameter onto another member
    // exactly as it does there.
    if (!Number.isInteger(pid) || pid < 0 || pid > E_MEMBER_ID_MASK) {
      this.refuse(`a member id inside the 28 bits of an eMemberHeader, not ${pid}`);
    }
    this.alignTo(4);
    const marker = PID_EXTENDED | pidFlagsAt(this.plCdr1.pidFlags, "extended-marker");
    this.writeInt(BigInt(marker), 2);
    this.writeInt(8n, 2);
    // The member's must-understand is `FLAG_2` of the `eMemberHeader`
    // (§7.4.1.2.1) and not the marker's bit above it.
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

  /**
   * Start a parameter's value, returning the origin it replaces.
   *
   * "Each element, or parameter, within a parameter list data
   * structure is simply a CDR-encapsulated block of data" — §7.4.1.2.
   * A block counts alignment from its own start, so an eight-octet
   * member at the head of one is already aligned and takes no padding
   * inside its own length.
   */
  private beginParameterValue(): number {
    const saved = this.origin;
    this.origin = this.out.length;
    return saved;
  }

  /**
   * Patch a parameter header with its value's length.
   *
   * The declared length covers the value's trailing padding or stops at
   * the value, depending on the framing — and which of the two framing
   * flags answers is the header's own {@link Pid.role}, not an argument
   * the caller has to remember. A generated codec calls
   * `w.finishPid(h)`, exactly as the C and Java columns do; a runtime
   * that took the answer as a defaulted parameter never reached
   * `padsOptionalLength` at all.
   *
   * The padding itself starts the next parameter at a four-octet offset
   * measured from the enclosing stream, so the origin goes back before
   * it is written.
   */
  finishPid(header: Pid): void {
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
    // Under Fast DDS's framing the parameter writes no padding of its
    // own — the next header aligns itself, and a final parameter ends
    // the stream where its value does.
    if (this.plCdr1.padAfterValue) {
      this.alignTo(4);
    }
  }

  /**
   * Rewrite a short parameter header as the extended form.
   *
   * §7.4.1.2.2's `ssize` is sixteen bits, and which values overrun it is
   * a property of the sample rather than of the type. So the plan writes
   * the short form for an unbounded member and the fallback lives here,
   * where the length is known. Without it a value over 64 KiB had its
   * length silently truncated to sixteen bits and the payload still
   * looked well-formed — a reader would take the next parameter from
   * inside this one.
   *
   * Eight octets are inserted between the header and the value. The
   * value's own octets do not change: XCDR1's MAXALIGN is eight and the
   * insertion is eight, so every offset inside it keeps its remainder.
   */
  private promotePid(header: Pid, length: number): void {
    const start = header.at - 2;
    this.out.splice(header.at + 2, 0, ...new Array<number>(8).fill(0));
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
  writeSentinel(): void {
    this.alignTo(4);
    this.writeInt(BigInt(PID_LIST_END | pidFlagsAt(this.plCdr1.pidFlags, "sentinel")), 2);
    this.writeInt(0n, 2);
  }

  // -- enumerations ----------------------------------------------------

  /**
   * An enumerator, as the integer its bit bound calls for.
   *
   * §7.4.3.5.3 rule (5) is
   * `XCDR << {O : ENUM_TYPE} = XCDR << {O.value : O.holder_type}`, and
   * the ENUM_TYPE row of the same clause's table says "O.holder_type is
   * either Int8, Int16 or Int32 depending on the value of the
   * `@bit_bound` annotation". A **signed** holder: `@value(-1)` is the
   * two's complement of one, so at the default bound it goes out as
   * `ffffffff`.
   *
   * What fits is everything expressible in the bound's bits, signed or
   * unsigned — `-2^(n-1) ..= 2^n - 1`. RTI Connext 7.7 accepts
   * `@value(-1)` at the default bound and Cyclone accepts `@value(200)`
   * under `@bit_bound(8)`; a signed-only range would reject the second
   * and a wider one would accept values neither can express.
   *
   * §7.3.1.2.1.5 caps the bound itself at 1 to 32, so a bound outside
   * that has no holder and is refused rather than widened.
   *
   * The bound comes **first**, which is the order the shared walk in
   * `crates/lang-rules/src/emit/encode.rs` calls it in and the order
   * `zdw_write_enum` and Java's `writeEnum` take. When both arguments
   * were a `number` a swapped pair was a legal call that wrote a
   * plausible octet — the Java column found exactly that, at run time.
   *
   * The value is a `bigint` and the bound a `number`, which is the same
   * widened holder `zdw_write_enum`'s `int64_t` and Java's `long` are:
   * the walk casts the enumerator to `Scalar::Int64` before the call, so
   * the two arguments now have different types and a swap does not
   * compile.
   */
  writeEnum(bitBound: number, value: bigint): void {
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

  /**
   * A bitmask, as the unsigned integer its bit bound calls for.
   *
   * §7.4.3.5.3 rule (6) writes `O.value` as `O.holder_type`, and Table
   * 33 of §7.4.1.1.3.2 gives the holder: `UInt8` for a bound of 1 to 8,
   * `UInt16` to 16, `UInt32` to 32 and `UInt64` to 64. The same clause
   * fixes the direction — "Bit indexes are counted from zero starting at
   * the least-significant bit of the full byte size of the bitmask" — so
   * bit `n` is `1 << n` whatever the holder's width.
   *
   * A bit at or past the bound is refused: the clause says those bits
   * "are not considered to be part of the bitmask", so setting one would
   * put a value on the wire no peer must read back.
   *
   * The value is a `bigint` at every bound and not only at 64, because
   * the caller is a generated codec that casts once. The bound comes
   * first, as it does for {@link writeEnum}.
   */
  writeBitmask(bitBound: number, bits: bigint): void {
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

  /**
   * One bitfield, shifted into the accumulator a bitset packs into.
   *
   * The counterpart of `zdw_bitfield` and of Java's `Writer.bitfield`.
   * A method of the writer and not a free function, because the shared
   * walk emits it as a call on the writer — `call_expr` in
   * `crates/lang-rules/src/emit/encode.rs` is `T::runtime("w", …)` — and
   * a free function of that name would be a second `bitfield` the
   * generated code never reaches.
   *
   * A value wider than the field is refused rather than masked: masking
   * would put a different number on the wire than the caller asked for,
   * and the caller would never find out.
   */
  bitfield(value: bigint, width: number, position: number): bigint {
    if (width <= 0 || width > 64 || position < 0 || position + width > 64) {
      this.refuse(`a bitfield of ${width} bits at ${position}, inside 64`);
    }
    const limit = (1n << BigInt(width)) - 1n;
    if (value < 0n || value > limit) {
      this.refuse(`a value inside ${width} bits, not ${value}`);
    }
    return value << BigInt(position);
  }

  /**
   * A bitset, as the anchor integer its bitfields pack into.
   *
   * DDS-XTypes 1.3 has no serialisation rule for a bitset — §7.4.3.5.3
   * runs from rule (1) to rule (30) and no left-hand side matches one.
   * How wide the anchor is comes from the layout planner and arrives
   * here as a count of **octets**, not of bits: `zerodds_ast_layout`'s
   * `Extent::Sized` is an octet count and the shared walk passes it
   * through. A runtime that read it as a bit count answered "8" for an
   * eight-octet anchor and wrote one octet where the plan said eight.
   */
  writeBitset(anchorOctets: number, bits: bigint): void {
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

  // -- patching --------------------------------------------------------

  private patch(at: number, value: number, width: number): void {
    let rest = BigInt(value) & ((1n << BigInt(width * 8)) - 1n);
    const octets = new Array<number>(width);
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

  private read(at: number, width: number): number {
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

  // -- the KeyHash -------------------------------------------------------

  /**
   * The holder as a KeyHash, zero-padded to sixteen octets.
   *
   * For a holder of sixteen octets or fewer. DDSI-RTPS 2.5 §9.6.3.8.
   */
  zeroPadded(): Uint8Array {
    const hash = new Uint8Array(KEY_HASH_LEN);
    hash.set(this.out.slice(0, KEY_HASH_LEN));
    return hash;
  }

  /**
   * The MD5 of the holder, for one longer than sixteen octets.
   *
   * MD5 because DDSI-RTPS 2.5 §9.6.3.8 names it, not as a security claim:
   * the KeyHash identifies an instance and is not a signature.
   */
  md5(): Uint8Array {
    return md5(this.finish());
  }
}

/**
 * The MD5 a KeyHash is made of. RFC 1321.
 *
 * Hand-rolled rather than a call into a platform digest, for the same
 * reason `runtime/csharp`'s `Md5.cs` gives and not `runtime/java`'s: this
 * runtime is isomorphic (Node, a browser, `runtime/typescript`'s own WASM
 * bridge in `wasm.ts`), and the Web Crypto API a browser exposes has no
 * MD5 at all — `SubtleCrypto.digest` supports only the SHA family — so
 * there is no single platform primitive to call from every environment
 * this package runs in, unlike Java's `MessageDigest` or even C#'s
 * FIPS-mode-sensitive one. `WireSelfTest`'s Java and C# counterparts check
 * RFC 1321's own test vectors; `against-recorded-values.ts` does the same
 * here, so the arithmetic is asserted and not assumed.
 *
 * Not a security primitive here and not used as one: the specification
 * names MD5 for this, and a different digest would produce hashes that no
 * other vendor agrees with. DDSI-RTPS 2.5 §9.6.3.8.
 */
export function md5(input: Uint8Array): Uint8Array {
  const s: readonly number[] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
  ];
  const k: readonly number[] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
    0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
    0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
    0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
    0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
    0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
    0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
    0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
    0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
  ];

  const bitLength = BigInt(input.length) * 8n;
  let paddedLength = input.length + 1;
  paddedLength += (56 - (paddedLength % 64) + 64) % 64;
  const message = new Uint8Array(paddedLength + 8);
  message.set(input);
  message[input.length] = 0x80;
  for (let index = 0; index < 8; index += 1) {
    message[paddedLength + index] = Number((bitLength >> BigInt(8 * index)) & 0xffn);
  }

  let a0 = 0x67452301;
  let b0 = 0xefcdab89;
  let c0 = 0x98badcfe;
  let d0 = 0x10325476;

  const rotl = (value: number, count: number): number =>
    ((value << count) | (value >>> (32 - count))) >>> 0;

  for (let chunkStart = 0; chunkStart < message.length; chunkStart += 64) {
    const words = new Array<number>(16);
    for (let word = 0; word < 16; word += 1) {
      const at = chunkStart + word * 4;
      words[word] =
        ((message[at] ?? 0) |
          ((message[at + 1] ?? 0) << 8) |
          ((message[at + 2] ?? 0) << 16) |
          ((message[at + 3] ?? 0) << 24)) >>>
        0;
    }

    let [a, b, c, d] = [a0, b0, c0, d0];
    for (let step = 0; step < 64; step += 1) {
      let mixed: number;
      let which: number;
      if (step < 16) {
        mixed = ((b & c) | (~b & d)) >>> 0;
        which = step;
      } else if (step < 32) {
        mixed = ((d & b) | (~d & c)) >>> 0;
        which = (5 * step + 1) % 16;
      } else if (step < 48) {
        mixed = (b ^ c ^ d) >>> 0;
        which = (3 * step + 5) % 16;
      } else {
        mixed = (c ^ (b | ~d)) >>> 0;
        which = (7 * step) % 16;
      }
      const sum = (a + mixed + (k[step] ?? 0) + (words[which] ?? 0)) >>> 0;
      const rotated = rotl(sum, s[step] ?? 0);
      [a, b, c, d] = [d, (b + rotated) >>> 0, b, c];
    }

    a0 = (a0 + a) >>> 0;
    b0 = (b0 + b) >>> 0;
    c0 = (c0 + c) >>> 0;
    d0 = (d0 + d) >>> 0;
  }

  const digest = new Uint8Array(16);
  const store = (at: number, value: number): void => {
    digest[at] = value & 0xff;
    digest[at + 1] = (value >>> 8) & 0xff;
    digest[at + 2] = (value >>> 16) & 0xff;
    digest[at + 3] = (value >>> 24) & 0xff;
  };
  store(0, a0);
  store(4, b0);
  store(8, c0);
  store(12, d0);
  return digest;
}

/**
 * The low `octets` of `value`'s two's-complement bits, zero-extended —
 * {@link Writer.writeUnionOverlay}'s pattern for an integer, character,
 * enumeration or bitmask arm. The mask is the function: `-2` in four
 * octets has to come out `0x00000000fffffffen`.
 */
export function bitPatternInt(value: bigint, octets: number): bigint {
  const pattern = BigInt.asUintN(64, value);
  return octets >= 8 ? pattern : pattern & ((1n << BigInt(octets * 8)) - 1n);
}

/** One octet, 0 or 1 — the boolean overlay pattern. */
export function bitPatternBool(value: boolean): bigint {
  return value ? 1n : 0n;
}

/** The IEEE 754 binary32 bits, zero-extended. */
export function bitPatternF32(value: number): bigint {
  const buffer = new DataView(new ArrayBuffer(4));
  buffer.setFloat32(0, value, true);
  return BigInt(buffer.getUint32(0, true));
}

/** The IEEE 754 binary64 bits. */
export function bitPatternF64(value: number): bigint {
  const buffer = new DataView(new ArrayBuffer(8));
  buffer.setFloat64(0, value, true);
  return buffer.getBigUint64(0, true);
}
