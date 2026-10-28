// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The read half of XCDR, implemented from DDS-XTypes 1.3 §7.4.
 *
 * The mirror of `writer.ts` clause for clause and not by inversion of
 * it: a reader written as "whatever undoes the writer" agrees with the
 * writer about a misreading of the clause, and agreement between two
 * halves of one column is the one thing that proves nothing.
 *
 * ## Two kinds of failure, and why they are not one
 *
 * A malformed *stream* throws. A length that runs past the end of the
 * buffer is not a state a caller can carry on from, and
 * {@link Truncated} against {@link ImpossibleLength} is the distinction
 * that matters: the first says the stream is short, the second says the
 * stream is lying. Allocating for a claimed length before checking it
 * is how a reader turns a bad message into an out-of-memory.
 *
 * A refusal about the *type* sets {@link Status} and does not throw.
 * The runtime owns the wire and the generated decoder owns the type,
 * and this reader cannot see an integer that names no enumerator, a
 * bitmask bit the declaration does not have, or a member that did not
 * arrive. §7.2.2.7's DISCARD is a sample the receiver drops and carries
 * on from, which is not a failure of the peer.
 *
 * ## What TypeScript decides differently from the other columns
 *
 * - `readChar` and `readWchar` answer with a one-character **string**,
 *   not with a code point. TypeScript has no character type, the
 *   materialiser declares a `char` member as `string`, and
 *   `Writer.writeChar` takes one; a reader answering with a `number`
 *   would be the only one of the three that did not.
 * - `readU32` answers with a `number`, because 4294967295 is far below
 *   `Number.MAX_SAFE_INTEGER`. `readI64` and `readU64` answer with a
 *   `bigint`, because 2^63 is not.
 * - `readEnum`, `readBitmask` and `readBitset` answer with a `bigint`
 *   whatever their bound. The generated decoder compares the answer
 *   against 64-bit patterns, and one comparison that mixed a `number`
 *   with a `bigint` is a `tsc` error rather than a wrong arm — which
 *   is the outcome this column wants.
 * - `readF128` answers with the sixteen octets. A binary64 would drop
 *   sixty bits of the significand at the assignment.
 */

import { Endianness, isLittle } from "./endianness.js";
import { ImpossibleLength, NotUtf8, OutOfRange, Truncated } from "./error.js";
import {
  FixedDigitCount,
  ParameterRole,
  Status,
  WcharFraming,
  WcharWidth,
  WstringLengthPolicy,
  WstringUnitOrder,
} from "./policy.js";
import {
  Float128,
  PID_EXTENDED,
  PID_IMPL_EXTENSION,
  PID_LIST_END,
  PID_MUST_UNDERSTAND,
  PID_SHORT_MAX_ID,
  PL_CDR1_DEFAULT,
  PlCdr1Framing,
  pidFlagsAt,
} from "./writer.js";

/**
 * The low fourteen bits of a short parameter id, which are the id.
 *
 * §7.4.1.2.2 puts `FLAG_IMPL_EXTENSION` at bit 15 and the
 * must-understand flag at bit 14; everything below them is the
 * identifier.
 */
export const PID_ID_MASK = 0x3fff;

/**
 * `PID_IGNORE`. §7.4.1.2.2: "All consumers of this Data Representation
 * shall ignore this parameter" — with no exception for the
 * must-understand bit above it.
 *
 * Declared here and not in `writer.ts` because no writer in this tree
 * emits one. It is a parameter a *peer* sends, so only the reader has
 * anything to do with it.
 */
export const PID_IGNORE = 0x3f03;

/** `FLAG_2` of an `eMemberHeader`: the member's must-understand. §7.4.1.2.1. */
const E_FLAG_MUST_UNDERSTAND = 0x4000_0000;

/**
 * `FLAG_1` of an `eMemberHeader`: `FLAG_IMPL_EXTENSION`, at bit 31
 * exactly as bit 15 carries it in the short header.
 */
const E_FLAG_IMPL_EXTENSION = 0x8000_0000;

/** The member id an `eMemberHeader` carries. §7.4.1.2.1. */
const E_MEMBER_ID_MASK = 0x0fff_ffff;

/** The UTF-16 byte-order mark. */
const BYTE_ORDER_MARK = 0xfeff;

/**
 * The byte-swapped mark.
 *
 * `U+FFFE` is not a character: a mark that reads as one says the units
 * behind it are the other way round, which is the whole of what a mark
 * is for.
 */
const SWAPPED_BYTE_ORDER_MARK = 0xfffe;

/**
 * The widest `fixed` IDL admits is `fixed<31, S>`, which is sixteen
 * octets — thirty-two nibbles. A value-precision field that presents no
 * sign nibble inside them is not a `fixed` at all.
 */
const MAX_FIXED_NIBBLES = 32;

/** The first nibble value [CDR] §9.3.2.8 reserves for a sign. */
const FIRST_SIGN_NIBBLE = 0x0a;

/** The sign nibble that means negative. [CDR] §9.3.2.8. */
const MINUS_NIBBLE = 0x0d;

/**
 * Everything a two-pass read has to put back.
 *
 * One caller: a `map` under `rule-15-omits`, where a delimited map
 * states no entry count and the entries are read once to count them and
 * again into storage sized from that count. TypeScript allocates its own
 * values, so there is no heap mark to carry — the cursor, the stream
 * origin and the status are the whole of it.
 *
 * The status is part of it deliberately. A refusal made during the
 * counting pass is a refusal about bytes that are about to be read
 * again, and carrying it forward would refuse the sample twice.
 */
export interface ReaderMark {
  readonly at: number;
  readonly origin: number;
  readonly status: Status;
}

/** An EMHEADER as read. §7.4.3.4.2. */
export interface EmHeaderRead {
  /** The 28-bit member id. */
  readonly memberId: number;
  /** The length code, 0 to 7. */
  readonly lengthCode: number;
  /** Whether the writer set the member's must-understand flag. */
  readonly mustUnderstand: boolean;
  /** The NEXTINT, for the codes that carry one. */
  readonly nextInt: number;
  /** Whether there was a NEXTINT at all. */
  readonly hasNextInt: boolean;
  /**
   * How many octets the member occupies after the header word, NEXTINT
   * included where there is one.
   *
   * What a reader skipping an unknown member advances by, and what a
   * reader of a known one checks itself against. §7.4.3.4.2: codes 0 to
   * 3 state a size of one, two, four or eight octets outright; code 4 is
   * followed by a NEXTINT holding the member's length; and codes 5, 6
   * and 7 have a NEXTINT that is *also* the member's own leading word,
   * so the member spans that word plus the length it states, times one,
   * four or eight.
   */
  readonly extent: number;
}

/** A PL_CDR1 parameter header as read. §7.4.1.2. */
export interface PidRead {
  /** The member id, with the flag bits taken off. */
  readonly pid: number;
  /** The declared length. */
  readonly length: number;
  /** Whether this is the `PID_LIST_END` sentinel. */
  readonly end: boolean;
  /** Whether the writer set the member's must-understand flag. */
  readonly mustUnderstand: boolean;
  /**
   * Whether the writer set `FLAG_IMPL_EXTENSION`.
   *
   * §7.4.1.2.2: the parameter is the writing implementation's own, and a
   * user-defined member "will never set the FLAG_IMPL_EXTENSION bit".
   * Masking it off with the rest would make a vendor-private parameter
   * indistinguishable from the member with the same low fourteen bits.
   */
  readonly implExtension: boolean;
  /**
   * Whether the short-form id is inside §7.4.1.2.2's reserved range,
   * which no member id occupies.
   */
  readonly reserved: boolean;
  /** `PID_IGNORE`, which every consumer shall ignore. */
  readonly ignore: boolean;
  /** Which of §7.4.1.2's two roles the caller read it in. */
  readonly role: ParameterRole;
  /** The stream origin the parameter's value block replaced. */
  readonly savedOrigin: number;
  /**
   * Where the parameter's value starts — the first octet after the
   * header.
   *
   * Carried on the header rather than worked out again at the finish,
   * because working it out again needs two things at once that are not
   * both still there: the enclosing origin has been put back by then,
   * and the header's own width is four octets or twelve depending on a
   * form the finish can only guess at from the values it reads.
   *
   * `runtime/java` reconstructs it as `savedOrigin + headerOctets` and
   * that is only the value start for the *first* parameter of a list:
   * the enclosing origin does not move between parameters, so the
   * second parameter's used length comes out as its offset from the
   * start of the whole list. Measured on
   * `tests/interop-results/struct-mutable/zerodds-rust/pl-cdr1.le.bin`,
   * where the second member's 8 octets are computed as 16 and the
   * sample is refused OUT_OF_RANGE.
   */
  readonly valueAt: number;
}

/**
 * Whether a parameter can be one of the reading type's members.
 *
 * False for a vendor-private parameter, for a reserved id and for
 * `PID_IGNORE`. All three are legal for a peer to send and none of them
 * is a member, so all three take the unknown-parameter path — skipped,
 * or the sample discarded when the writer set must-understand.
 * §7.4.1.2.2, §7.2.2.4.4.4.6.
 *
 * A free function and not a method, because the generated decoder calls
 * it without a receiver.
 */
export function pidIsUserMember(header: PidRead): boolean {
  return !header.implExtension && !header.reserved && !header.ignore;
}

/**
 * How many octets `text` is as UTF-8, which is what a `string<N>` bound
 * counts: `string<5>` is `char[6]` in the C mapping of every measured
 * DDS implementation, so a five-scalar value of ten octets is one no
 * peer can hold.
 */
export function utf8Octets(text: string): number {
  return new TextEncoder().encode(text).length;
}

/**
 * How many UTF-16 code units `text` is, which is what a `wstring<N>`
 * bound counts. A surrogate pair is two, which is what `String.length`
 * already answers.
 */
export function utf16Units(text: string): number {
  return text.length;
}

/**
 * The same value in a `number`, whatever holder it arrived in.
 *
 * The generated decoder's narrowings are `as` in every other column and
 * `as` is *erased* here: `zd_bits as number` on a `bigint` is a `tsc`
 * error, and where it is not — `x as unknown as number` — it would put
 * a `bigint` in a `number` field and every later comparison would be
 * false. So the three coercions below are real conversions, and the
 * TypeScript backend emits a call to one where the other columns emit a
 * cast.
 *
 * A one-character string answers with its code point, which is what the
 * narrowing means where the source is a `char`.
 */
export function zdNumber(value: number | bigint | string | boolean): number {
  if (typeof value === "string") {
    return value.codePointAt(0) ?? 0;
  }
  if (typeof value === "boolean") {
    return value ? 1 : 0;
  }
  return Number(value);
}

/** The same value in a `bigint`. See {@link zdNumber}. */
export function zdBigInt(value: number | bigint | string | boolean): bigint {
  if (typeof value === "string") {
    return BigInt(value.codePointAt(0) ?? 0);
  }
  return BigInt(value);
}

/**
 * The same value as a one-character `string`, which is what an IDL
 * `char` and `wchar` are here.
 *
 * A value that is already a string is itself; a number or a `bigint` is
 * the character its code point names. `String(65)` would be `"65"`,
 * which is two characters and a different value.
 */
export function zdChar(value: number | bigint | string): string {
  if (typeof value === "string") {
    return value;
  }
  return String.fromCodePoint(Number(value));
}

/**
 * What a `wstring` length policy answers, as its three coordinates.
 *
 * Not a list of named forms: C1 says whether the count is of characters
 * or of the octets they occupy, C2 whether a terminator is counted, and
 * C3 how wide one unit is. A cell reachable only by its name cannot be
 * asked for by someone who has measured it and not yet named it, which
 * is how the ROS form stayed unreachable in an earlier column.
 */
interface WstringCell {
  readonly countsCharacters: boolean;
  readonly terminated: boolean;
  readonly unitWidth: number;
}

const WSTRING_CELLS: ReadonlyMap<WstringLengthPolicy, WstringCell> = new Map([
  [
    WstringLengthPolicy.Specification,
    { countsCharacters: false, terminated: false, unitWidth: 2 },
  ],
  [
    WstringLengthPolicy.CharacterCount,
    { countsCharacters: true, terminated: false, unitWidth: 2 },
  ],
  [
    WstringLengthPolicy.CharacterCountTerminated,
    { countsCharacters: true, terminated: true, unitWidth: 4 },
  ],
  [
    WstringLengthPolicy.SpecificationWideUnits,
    { countsCharacters: false, terminated: false, unitWidth: 4 },
  ],
  [
    WstringLengthPolicy.RosTypeSupport,
    { countsCharacters: true, terminated: false, unitWidth: 4 },
  ],
  [
    WstringLengthPolicy.OctetsTerminated2,
    { countsCharacters: false, terminated: true, unitWidth: 2 },
  ],
  [
    WstringLengthPolicy.OctetsTerminated4,
    { countsCharacters: false, terminated: true, unitWidth: 4 },
  ],
  [
    WstringLengthPolicy.CharactersTerminated2,
    { countsCharacters: true, terminated: true, unitWidth: 2 },
  ],
]);

/**
 * Whether a unit order reads its code units little-endian regardless of
 * the stream.
 *
 * `host-unswapped` is what Fast CDR writes — the host's order, never
 * byte-swapped — and this runtime's host is little-endian wherever
 * JavaScript's `DataView` default is not asked for. `byte-order-mark`
 * reads little too and then obeys the mark it found, which is correct in
 * all four combinations of writer host and stream order: the mark is
 * written in the same order as the content it introduces.
 */
function readsLittleAlways(order: WstringUnitOrder): boolean {
  return (
    order === WstringUnitOrder.HostUnswapped ||
    order === WstringUnitOrder.ByteOrderMark
  );
}

export class Reader {
  private readonly bytes: Uint8Array;
  private at = 0;
  private origin = 0;
  private readonly maxAlignment: number;
  private state: Status = Status.Ok;

  /**
   * A reader over `data` in `order`, capped at `maxAlign`.
   *
   * Four for XCDR2, eight for XCDR1 — {@link Writer}'s counterpart.
   * §7.4.1: `ALIGN(N)` aligns to `MIN(N, XCDR.maxalign)`. A reader that
   * skipped padding the writer never wrote reads the wrong octets from
   * the second member on.
   *
   * `plCdr1` is the writer's framing, and the reader needs two things
   * out of it. Under a framing that puts `FLAG_IMPL_EXTENSION` on every
   * member id — OpenDDS writes `c0010004` for `@id(1) long a` — the flag
   * no longer tells a vendor-private parameter from a user member. And
   * §7.4.1.2.2 stops the short form's ids at `0x3F00` inclusive while
   * one measured implementation carries the whole low fourteen bits, so
   * a reader holding the clause's threshold as a constant reports
   * `@id(16383)` as a reserved parameter and drops the member.
   */
  constructor(
    data: Uint8Array,
    readonly order: Endianness,
    maxAlign = 8,
    readonly plCdr1: PlCdr1Framing = PL_CDR1_DEFAULT,
  ) {
    this.bytes = data;
    this.maxAlignment = Math.max(1, maxAlign);
  }

  /** The alignment cap in force. */
  get maxAlign(): number {
    return this.maxAlignment;
  }

  // -- the status a generated decoder reads ----------------------------

  /**
   * Whether a refusal has been made, so every later read does nothing.
   *
   * The generated decoder carries on to the end of the type rather than
   * returning early — that is what lets one `if` at the top of the
   * caller decide what to do with the sample — and a read that consumed
   * octets after a refusal would leave the cursor somewhere no writer
   * put it, which the trailing-octet check at the top would then report
   * as a second, invented, fault.
   */
  private stopped(): boolean {
    return this.state !== Status.Ok;
  }

  /** How the read is going. */
  readerStatus(): Status {
    return this.state;
  }

  /** Refuse the stream from outside the runtime. The first refusal sticks. */
  readerRefuse(refusal: Status): void {
    if (refusal !== Status.Ok && this.state === Status.Ok) {
      this.state = refusal;
    }
  }

  /**
   * Take back a {@link Status.Discarded}, and only that one.
   *
   * A member whose own `@try_construct` is `USE_DEFAULT` but which also
   * carries an explicit `@default` cannot be answered inside the type's
   * decoder: the decoder knows Table 9's value for the type and not the
   * literal the member's declaration gave it, and §7.2.2.4.4.4.9 puts
   * the literal first. So the call site asks for DISCARD, and where it
   * gets one it clears it and assigns the literal.
   *
   * Every other status stays: a truncated stream is not a narrowing.
   */
  readerClearDiscard(): boolean {
    if (this.state === Status.Discarded) {
      this.state = Status.Ok;
      return true;
    }
    return false;
  }

  /** Where it is. The generated decoder's name for {@link position}. */
  readerPosition(): number {
    return this.at;
  }

  /** How many octets are left. */
  readerRemaining(): number {
    return this.remaining();
  }

  /**
   * Move the alignment origin to the current position.
   *
   * The read-side pair of the writer's `beginPackedElement`, for
   * `ElementPadding::Packed`. Returns the origin displaced.
   */
  readerBeginPackedElement(): number {
    const saved = this.origin;
    this.origin = this.at;
    return saved;
  }

  /** Restore the origin {@link readerBeginPackedElement} displaced. */
  readerFinishPackedElement(saved: number): void {
    this.origin = saved;
  }

  /**
   * Move the cursor forward to `target`, which is never backwards.
   *
   * A framing word pointing behind the cursor means the reader has
   * already read past what it framed, and seeking back would paper over
   * it: the member would be read twice and the sample would come back
   * plausible.
   */
  readerSkipTo(target: number): void {
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
  readerSave(): ReaderMark {
    return { at: this.at, origin: this.origin, status: this.state };
  }

  /** Put back what {@link readerSave} took. */
  readerRestore(mark: ReaderMark): void {
    this.at = mark.at;
    this.origin = mark.origin;
    this.state = mark.status;
  }

  // -- state -----------------------------------------------------------

  /** Where it is. */
  position(): number {
    return this.at;
  }

  /** How many octets are left. */
  remaining(): number {
    return Math.max(0, this.bytes.length - this.at);
  }

  /** Skip padding to `align`, counted from the stream's origin. */
  alignTo(align: number): void {
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
  skip(count: number): void {
    if (this.stopped()) {
      return;
    }
    this.take(count);
  }

  /** Move to an absolute offset inside the buffer. */
  seek(target: number): void {
    if (this.stopped()) {
      return;
    }
    if (target < 0 || target > this.bytes.length) {
      throw new Truncated(this.at, target, this.bytes.length);
    }
    this.at = target;
  }

  /**
   * Advance over `count` octets and answer where they started.
   *
   * A negative count is refused rather than treated as zero: it can only
   * come from a length field this reader computed wrongly, and a zero
   * there would carry on reading at an offset nothing wrote.
   */
  private take(count: number): number {
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

  /**
   * An integer of `width` octets in the stream's order, as a `bigint`.
   *
   * A `bigint` throughout and narrowed by the caller, because the eight-
   * octet case does not fit a `number` and a second code path for it
   * would be the one place the two widths could disagree about a sign.
   */
  private readInt(width: number, signed: boolean): bigint {
    this.alignTo(width);
    return this.rawInt(width, signed, isLittle(this.order));
  }

  /** {@link readInt} in a stated order rather than the stream's. */
  private rawInt(width: number, signed: boolean, little: boolean): bigint {
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
   * `boolean`.
   *
   * An octet that is neither 0 nor 1 is a stream that disagrees with its
   * own type, and is refused rather than coerced: `!!octet` would turn
   * every one of the 254 impossible values into `true` and say nothing.
   */
  readBool(): boolean {
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
  readU8(): number {
    if (this.stopped()) {
      return 0;
    }
    return this.bytes[this.take(1)] ?? 0;
  }

  /** `int8`. */
  readI8(): number {
    if (this.stopped()) {
      return 0;
    }
    const value = this.bytes[this.take(1)] ?? 0;
    return value >= 0x80 ? value - 0x100 : value;
  }

  /**
   * `char`, one octet, as the one-character string it is here.
   *
   * The octet is a code point in `[0, 255]` and not a UTF-8 lead: an IDL
   * `char` is one octet and §7.4.1.1.1 gives it no encoding of its own,
   * so `0xE4` is U+00E4 and not the first half of anything.
   */
  readChar(): string {
    if (this.stopped()) {
      return "\0";
    }
    return String.fromCharCode(this.bytes[this.take(1)] ?? 0);
  }

  /**
   * `wchar`, at `width` octets and in `framing`'s shape.
   *
   * The four-octet form is RTI Connext's under XCDR1, where the high
   * sixteen bits are padding around one UTF-16 code unit. A value that
   * uses them is refused rather than narrowed: narrowing would return a
   * character the peer never sent.
   *
   * {@link WcharFraming.GiopLengthPrefixed} is CORBA 3.3 part 2
   * §9.3.1.6's GIOP 1.2 form, which all three ORBs write. The octet
   * count is checked against the configured code-set width rather than
   * believed: a stream whose first octet says `0xE4` — which is what a
   * bare two-octet `wchar` looks like read this way — would otherwise
   * consume 228 octets of whatever followed and return a character
   * assembled out of the next four members.
   */
  readWchar(
    width: WcharWidth = WcharWidth.Octets2,
    framing: WcharFraming = WcharFraming.Fixed,
    order: WstringUnitOrder = WstringUnitOrder.Stream,
  ): string {
    if (this.stopped()) {
      return "\0";
    }
    const where = this.at;
    const octets: number = width;
    let value: number;
    if (framing === WcharFraming.GiopLengthPrefixed) {
      const declared = this.readU8();
      if (declared !== octets) {
        throw new OutOfRange(
          where,
          `a GIOP 1.2 wchar of ${octets} octets, the configured transmission ` +
            `code set, and not ${declared}`,
        );
      }
      // A scalar cannot carry a byte-order mark, so `byte-order-mark`
      // means CORBA's default here — big-endian — exactly as
      // `big-endian-always` does. The units are aligned to one inside
      // the GIOP form, so `rawInt` and not `readInt`.
      const little =
        order === WstringUnitOrder.Stream && isLittle(this.order);
      value = Number(this.rawInt(octets, false, little));
    } else if (octets === 4) {
      value = this.readU32();
      if (value > 0xffff) {
        throw new OutOfRange(where, "a four-octet wchar holding one UTF-16 code unit");
      }
    } else {
      value = this.readU16();
    }
    // §7.2.2.2.1.2.5 restricts a `wchar` to the BMP, and a lone
    // surrogate is not a character of it: returning one would produce a
    // string no `TextEncoder` can encode back.
    if (value >= 0xd800 && value <= 0xdfff) {
      throw new OutOfRange(where, "a Unicode code point");
    }
    return String.fromCharCode(value);
  }

  /** `short`. */
  readI16(): number {
    if (this.stopped()) {
      return 0;
    }
    return Number(this.readInt(2, true));
  }

  /** `unsigned short`. */
  readU16(): number {
    if (this.stopped()) {
      return 0;
    }
    return Number(this.readInt(2, false));
  }

  /** `long`. */
  readI32(): number {
    if (this.stopped()) {
      return 0;
    }
    return Number(this.readInt(4, true));
  }

  /**
   * `unsigned long`, as a `number`.
   *
   * 4294967295 is far below `Number.MAX_SAFE_INTEGER`, so unlike the
   * Java column nothing has to widen and unlike the C column nothing
   * comes back negative.
   */
  readU32(): number {
    if (this.stopped()) {
      return 0;
    }
    return Number(this.readInt(4, false));
  }

  /** `long long`, which is a `bigint` because a binary64 is not one. */
  readI64(): bigint {
    if (this.stopped()) {
      return 0n;
    }
    return this.readInt(8, true);
  }

  /**
   * `unsigned long long`, as the value and not as a pattern.
   *
   * Where the Java column has to hand back `-1L` for
   * `0xFFFFFFFFFFFFFFFF` because it has nothing wider than a `long`, a
   * `bigint` holds 18446744073709551615 as itself.
   */
  readU64(): bigint {
    if (this.stopped()) {
      return 0n;
    }
    return this.readInt(8, false);
  }

  /** `float`, a binary32. */
  readF32(): number {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(4);
    const from = this.take(4);
    return new DataView(
      this.bytes.buffer,
      this.bytes.byteOffset + from,
      4,
    ).getFloat32(0, isLittle(this.order));
  }

  /** `double`, a binary64. */
  readF64(): number {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(8);
    const from = this.take(8);
    return new DataView(
      this.bytes.buffer,
      this.bytes.byteOffset + from,
      8,
    ).getFloat64(0, isLittle(this.order));
  }

  /**
   * `long double`: all sixteen octets, as the writer's counterpart holds
   * them.
   *
   * The octets come back in the order a little-endian stream carries
   * them, which is what {@link Writer.writeF128} takes; a big-endian
   * stream's are reversed on the way in. Reading the first eight as a
   * `double` and skipping the rest would decode every value a binary64
   * cannot hold as a different number and say nothing.
   */
  readF128(): Float128 {
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
  readU16Little(): number {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(2);
    return Number(this.rawInt(2, false, true));
  }

  /** A 16-bit value big-endian whatever the stream's order is. */
  readU16Big(): number {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(2);
    return Number(this.rawInt(2, false, false));
  }

  /** A 32-bit value little-endian whatever the stream's order is. */
  readU32Little(): number {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(4);
    return Number(this.rawInt(4, false, true));
  }

  /** A 32-bit value big-endian whatever the stream's order is. */
  readU32Big(): number {
    if (this.stopped()) {
      return 0;
    }
    this.alignTo(4);
    return Number(this.rawInt(4, false, false));
  }

  // -- strings ---------------------------------------------------------

  /**
   * `string`: a four-octet length that includes the terminator, then the
   * octets, then a NUL. §7.4.1.1.1.
   *
   * A declared length of zero is impossible rather than empty: the count
   * includes the terminator, so the shortest honest frame states one.
   * A length past the end is {@link ImpossibleLength} and not
   * {@link Truncated} — the stream is not short, the field is lying, and
   * a reader that allocated for it first would be the vector.
   */
  readString(): string {
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

  /**
   * A `string` the reader materialises itself.
   *
   * `runtime/c` takes it out of a heap the caller set, because an
   * embedded target decides where memory comes from. TypeScript has one
   * answer to that question and this is it; the name is kept so that one
   * walk names one operation in both columns.
   */
  readStringAlloc(): string {
    return this.readString();
  }

  /**
   * A `wstring`, in whichever length convention the policy names.
   *
   * `runtime/c` converts to UTF-8 because that is what its
   * `const char *` holds, and its name carries it. A TypeScript `string`
   * is UTF-16 already and {@link readWstring} is the whole of the
   * operation.
   */
  readWstringUtf8(
    policy: WstringLengthPolicy,
    order: WstringUnitOrder,
  ): string {
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
  readWstringUtf8Octets(policy: WstringLengthPolicy): string {
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
          throw new OutOfRange(
            where,
            "no NUL inside a wstring a C peer would stop at",
          );
        }
      }
      count -= 1;
    }
    return this.decodeUtf8(from, count, where);
  }

  /**
   * A `wstring`, in the form `policy` and `order` name.
   *
   * Under {@link WstringLengthPolicy.Specification} the count is octets
   * and not characters — §7.4.1.1.2 — so a count the code-unit width
   * does not divide is a stream no encoder produced and is refused
   * rather than rounded down: rounding would leave the remainder in the
   * stream and put every later member at the wrong offset.
   *
   * The form is never inferred from the stream. The cells are not
   * separable: a Fast DDS `"ab"` followed by a `long` is byte-identical
   * to a specification `"a"` followed by the same `long`, differing only
   * in padding no clause fixes. Choosing between them would mean letting
   * the length field — the one field a hostile peer controls — decide
   * how much is read.
   */
  readWstring(policy: WstringLengthPolicy, order: WstringUnitOrder): string {
    if (this.stopped()) {
      return "";
    }
    const where = this.at;
    const cell = WSTRING_CELLS.get(policy);
    if (cell === undefined) {
      throw new OutOfRange(where, `a wstring length policy, not \`${policy}\``);
    }
    const declared = this.readU32();
    let count: number;
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
      // The count includes the NUL, so a count of zero is a frame that
      // does not hold its own terminator.
      if (declared < 1 || units.length === 0) {
        throw new ImpossibleLength(where, declared, this.remaining());
      }
      if (units[units.length - 1] !== 0) {
        throw new OutOfRange(
          where,
          "a NUL character ending the wstring, as its count includes one",
        );
      }
      units = units.slice(0, units.length - 1);
      // A NUL inside the text is the narrow string's problem in wide
      // form: every C peer stops at it. Only where a terminator is
      // expected — a cell without one is entitled to carry U+0000 as a
      // character.
      if (units.includes(0)) {
        throw new OutOfRange(
          where,
          "no NUL inside a wstring a C peer would stop at",
        );
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
          out += String.fromCodePoint(
            0x10000 + ((unit - 0xd800) << 10) + (low - 0xdc00),
          );
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

  /**
   * `count` `wstring` characters of `width` octets, as UTF-16 units.
   *
   * The whole run is checked against what is left **before** the first
   * unit is read, because a count is what a hostile peer controls: the
   * check is the difference between refusing a frame and reading 4·2^32
   * octets of whatever follows.
   *
   * The four-octet width is RTI's XCDR1 form, where a character is a
   * code unit zero-extended. A value above `0xFFFF` there is not a form
   * this reader knows how to narrow, so it is refused rather than
   * truncated — truncating is the defect this whole policy exists
   * because of.
   */
  private readWstringUnits(
    where: number,
    count: number,
    width: number,
    order: WstringUnitOrder,
  ): number[] {
    if (count < 0 || count * width > this.remaining()) {
      throw new ImpossibleLength(where, count * width, this.remaining());
    }
    const little = readsLittleAlways(order);
    const units: number[] = [];
    for (let index = 0; index < count; index += 1) {
      let unit: number;
      if (width === 4) {
        if (order === WstringUnitOrder.BigEndianAlways) {
          unit = this.readU32Big();
        } else if (little) {
          unit = this.readU32Little();
        } else {
          unit = this.readU32();
        }
        if (unit > 0xffff) {
          throw new OutOfRange(
            where,
            "a four-octet wstring character holding one UTF-16 unit",
          );
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

  /**
   * `text` cut to at most `octets` UTF-8 octets, never inside a
   * character.
   *
   * Table 11: "The order of the characters in the string ... is
   * preserved", so this is a prefix. A character that would straddle the
   * bound is left out whole: a peer holding `char[octets + 1]` cannot
   * store half of one either, and half a UTF-8 sequence is not text.
   */
  trimUtf8(text: string, octets: number): string {
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

  /**
   * `text` cut to at most `units` UTF-16 code units, never between the
   * halves of a surrogate pair.
   */
  trimUtf16(text: string, units: number): string {
    if (units < 0) {
      return "";
    }
    if (text.length <= units) {
      return text;
    }
    let end = units;
    // A high surrogate at the cut has lost its pair, and half a pair is
    // not a character.
    const last = text.charCodeAt(end - 1);
    if (end > 0 && last >= 0xd800 && last <= 0xdbff) {
      end -= 1;
    }
    return text.slice(0, end);
  }

  /**
   * `fixed<digits, scale>`, as the decimal string the writer took.
   *
   * The mirror of `Writer.writeFixed`: `ceil((digits + 1) / 2)` octets
   * of BCD, digits most-significant first, sign nibble last.
   *
   * The leading zeros go, because the BCD field is `digits` wide
   * whatever the value: the nibbles for `0.01` in a `fixed<5, 2>` spell
   * `00001`, and returning `"000.01"` would put a decimal on the read
   * side that the write side would never produce from the same value.
   * A negative zero loses its sign for the same reason: `-0.00` and
   * `0.00` are one value, and the sign nibble is the only place they
   * differ.
   *
   * Under {@link FixedDigitCount.ValuePrecision} the field's end is read
   * out of the octets: [CDR] §9.3.2.8 makes a digit `0` to `9` and a
   * sign `0xa` upwards, and the nibble count is even by construction, so
   * the field ends at the first octet whose **low** nibble is a sign. A
   * field that never presents one within the sixteen octets of the
   * widest `fixed` IDL admits is refused.
   */
  readFixed(digits: number, scale: number, count: FixedDigitCount): string {
    if (this.stopped()) {
      return "";
    }
    // The same bound as the writer's, on this side too: OMG IDL 4.2 rule
    // (42) caps the BCD rule at 31 digits, and a reader asked for a wider
    // field would take octets no declaration can name.
    if (digits <= 0 || digits > 31 || scale > digits) {
      throw new OutOfRange(
        this.at,
        `a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<${digits}, ${scale}>`,
      );
    }
    let nibbles: number[] = [];
    let sign: number;
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
      // An even declared digit count leaves the field one nibble wider
      // than the value, and the pad is at the front.
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
    // The point sits `scale` digits from the right and not
    // `digits - scale` from the left: under the value's count the field
    // carries fewer than `digits`, and one that carries fewer than the
    // scale — `1c` under `fixed<5,2>` — needs its integer part written
    // as a zero.
    text = text.padStart(scale + 1, "0");
    const split = text.length - scale;
    const whole = text.slice(0, split).replace(/^0+(?=.)/, "");
    const fraction = text.slice(split);
    const out = scale === 0 ? whole : `${whole}.${fraction}`;
    const zero = whole === "0" && !/[1-9]/.test(fraction);
    return sign === MINUS_NIBBLE && !zero ? `-${out}` : out;
  }

  // -- the bit-bounded kinds -------------------------------------------

  /**
   * An enumerator's holder, **sign-extended from its holder's width**.
   *
   * §7.4.3.5.3 rule (5) makes the holder signed: `@value(200)` under a
   * bound of eight goes out as `c8`, and that octet read as the type
   * says is `-56`. `@value(-1)` and `@value(255)` are the same octet and
   * nothing in the stream separates them — which one was meant is a
   * question about the *type*, and the generated decoder answers it by
   * comparing this against each declared enumerator's own pattern, taken
   * the same way.
   *
   * The Java column had this unsigned while its walk compared against
   * the signed pattern, and every `@bit_bound(8)` enumerator above 127
   * matched no arm at all — the sample came back discarded rather than
   * wrong, which is why it took a corpus run to find. The sign extension
   * is from the *holder's* width and not from the bound: `@bit_bound(5)`
   * still occupies an octet, and rule (5) makes the octet signed.
   */
  readEnum(bitBound: number): bigint {
    if (this.stopped()) {
      return 0n;
    }
    if (bitBound <= 0 || bitBound > 32) {
      throw new OutOfRange(
        this.at,
        `an @bit_bound between 1 and 32, not ${bitBound}`,
      );
    }
    const width = bitBound <= 8 ? 8 : bitBound <= 16 ? 16 : 32;
    const pattern =
      width === 8
        ? BigInt(this.readU8())
        : width === 16
          ? BigInt(this.readU16())
          : BigInt(this.readU32());
    const sign = 1n << BigInt(width - 1);
    return (pattern & sign) === 0n ? pattern : pattern - (1n << BigInt(width));
  }

  /** A bitmask's holder. Table 33 of §7.4.1.1.3.2. */
  readBitmask(bitBound: number): bigint {
    if (this.stopped()) {
      return 0n;
    }
    if (bitBound <= 0 || bitBound > 64) {
      throw new OutOfRange(
        this.at,
        `an @bit_bound between 1 and 64, not ${bitBound}`,
      );
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
  readBitset(anchorOctets: number): bigint {
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
        throw new OutOfRange(
          this.at,
          `a bitset anchor of 1, 2, 4 or 8 octets, not ${anchorOctets}`,
        );
    }
  }

  // -- framing ---------------------------------------------------------

  /**
   * A sequence's element count, refusing one the stream cannot hold.
   *
   * An element is one octet or more, so a stream with fewer octets left
   * than elements claimed cannot be honest about it. Allocating for it
   * is how a reader turns a bad message into an out-of-memory — and in
   * this column `new Array(n)` for a claimed 4294967295 is exactly that.
   */
  readSequenceLength(): number {
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
  readDheader(): number {
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

  /**
   * An EMHEADER, unpacked into its parts and the NEXTINT it carries.
   *
   * Codes 5 to 7 report the NEXTINT they read, but the reader is put
   * back before it: §7.4.3.4.2 says that word "shall be reused also as
   * part of the serialized member", so it is the member's own leading
   * length and the member's decoder has to see it.
   */
  readEmheader(): EmHeaderRead {
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
  readPid(): PidRead {
    return this.readPidAs(ParameterRole.Member);
  }

  /**
   * A PL_CDR1 parameter header, in whichever form it was written.
   *
   * `role` is the caller's, not the stream's: the octets are the same
   * octets and only what the declared length means differs. §7.4.1.2.
   */
  readPidAs(role: ParameterRole): PidRead {
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
      // The four flag bits are not part of the id: §7.4.1.2.1 splits the
      // `eMemberHeader` into `FLAG_1` to `FLAG_4` and a 28-bit member
      // id, and `FLAG_2` is the member's must-understand. Taking the
      // whole word gave a Cyclone member declared `@must_understand` an
      // id of `0x40000001`, which matches nothing, and the member was
      // dropped.
      const memberHeader = this.readU32();
      const wideLength = this.readU32();
      // The parameter's value is its own stream block: §7.4.1.2 counts a
      // member's internal alignment from the first octet after the
      // header, not from the start of the message.
      const saved = this.origin;
      this.origin = this.at;
      return {
        pid: memberHeader & E_MEMBER_ID_MASK,
        length: wideLength,
        end: false,
        mustUnderstand: (memberHeader & E_FLAG_MUST_UNDERSTAND) !== 0,
        implExtension: (memberHeader & E_FLAG_IMPL_EXTENSION) !== 0,
        // The extended id is 28 bits and has no reserved range.
        reserved: false,
        ignore: false,
        role,
        savedOrigin: saved,
        valueAt: this.at,
      };
    }
    const identifier = pid & PID_ID_MASK;
    const end = identifier === (PID_LIST_END & PID_ID_MASK);
    // Under a framing that puts `FLAG_IMPL_EXTENSION` on every member id
    // the bit no longer tells a vendor-private parameter from a user
    // member — OpenDDS writes `c0010004` for `@id(1) long a`. It is
    // reported absent there because under that framing it carries no
    // information, and reporting it would send every member down the
    // unknown-parameter path.
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

  /**
   * End a parameter's value and step to the next header.
   *
   * The padding to four follows the value and is outside the declared
   * length, so the next parameter is found by alignment and not by
   * adding the length — which is what lets one reader take RTI's
   * rounded-up length and Fast DDS's exact one.
   *
   * The declared length is checked against the configured framing rather
   * than believed: a length belonging to the other convention is a
   * stream this reader was pointed at with the wrong framing, and
   * reading on would put every later member at an offset no writer
   * produced. Both readings are accepted where they coincide, which is
   * every value already a multiple of four.
   */
  finishPidRead(header: PidRead): void {
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
    if (
      header.length !== (pads ? padded : used) &&
      header.length !== used &&
      header.length !== padded
    ) {
      this.readerRefuse(Status.OutOfRange);
    }
    this.alignTo(4);
  }

  /**
   * Step over a member this reader does not know.
   *
   * The whole point of the length code, and the reason §7.4.3.4.2 gives
   * one to every mutable member: a reader of an older type has to be
   * able to walk past what it does not have a field for.
   */
  skipMember(header: EmHeaderRead): void {
    if (this.stopped()) {
      return;
    }
    if (header.lengthCode > 3 && !header.hasNextInt) {
      throw new OutOfRange(this.at, "a NEXTINT for a length code of 4 or above");
    }
    this.skip(header.extent);
  }

  /**
   * Start reading a parameter's value, answering with the origin it
   * replaces. The reader's half of the writer's parameter block.
   */
  beginParameterValue(): number {
    const saved = this.origin;
    this.origin = this.at;
    return saved;
  }

  /** End a parameter's value and step over its trailing padding. */
  finishParameterValue(savedOrigin: number): void {
    this.origin = savedOrigin;
    this.alignTo(4);
  }

  // -- text -------------------------------------------------------------

  /**
   * `length` octets from `from` as UTF-8, refusing anything that is not.
   *
   * `fatal: true` is the whole of it: without it `TextDecoder` replaces
   * every malformed sequence with U+FFFD and answers a string that is
   * the right length and the wrong text, which is precisely the silent
   * substitution {@link NotUtf8} exists to prevent.
   */
  private decodeUtf8(from: number, length: number, where: number): string {
    try {
      return new TextDecoder("utf-8", { fatal: true }).decode(
        this.bytes.subarray(from, from + length),
      );
    } catch {
      throw new NotUtf8(where);
    }
  }
}

/** An {@link EmHeaderRead} with its {@link EmHeaderRead.extent} worked out. */
function emHeader(
  memberId: number,
  lengthCode: number,
  mustUnderstand: boolean,
  nextInt: number,
  hasNextInt: boolean,
): EmHeaderRead {
  let extent: number;
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

/**
 * Take the byte-order mark off the front and apply what it said.
 *
 * The units arrived little-endian, because that is what omniORB writes
 * on a little-endian host and what the `byte-order-mark` order therefore
 * reads. Three cases, and the third is the one that matters:
 *
 * - `FEFF` — the mark is the right way up, the units are as read.
 * - `FFFE` — the mark is byte-swapped, so every unit behind it is too,
 *   and each one is swapped back. This is what a conforming big-endian
 *   omniORB peer produces and what a reader that ignored the mark would
 *   return as different characters, well formed and the right length.
 * - no mark — CORBA's rule for a GIOP 1.2 `wstring` without one is
 *   UTF-16 big-endian, so every unit read little-endian is swapped.
 *
 * The mark is never a character of the returned string in any of the
 * three, which is the half §7.4.1.1.2 cares about.
 */
function stripByteOrderMark(units: number[]): number[] {
  if (units.length === 0) {
    return units;
  }
  if (units[0] === BYTE_ORDER_MARK) {
    return units.slice(1);
  }
  const rest = units[0] === SWAPPED_BYTE_ORDER_MARK ? units.slice(1) : units;
  return rest.map((unit) => ((unit & 0xff) << 8) | (unit >> 8));
}
