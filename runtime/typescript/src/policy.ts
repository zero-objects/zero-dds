// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The wire policies a layout plan cannot answer.
 *
 * Every one of these is a place where measured implementations differ
 * and the clause leaves room, so the generated codec names the policy
 * and the runtime carries it out. The names are
 * `crates/ast-layout/src/key.rs`'s axes, in TypeScript's spelling; the
 * measurements and the arguments are there.
 *
 * A member per value and no catch-all anywhere that reads one: a
 * policy this column has not implemented has to be a compile error and
 * not a value that looks plausible.
 */

/** How many octets one `wchar` takes. */
export enum WcharWidth {
  /** Two, which is Table 31 and seven of the nine measured columns. */
  Octets2 = 2,
  /** Four, which is RTI's XCDR1, TAO, and rosidl's type support. */
  Octets4 = 4,
}

/** What shape a `wchar` goes out in. */
export enum WcharFraming {
  /** The code unit alone, at its width. */
  Fixed = "fixed",
  /**
   * CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 form: an octet count first,
   * then the code unit's octets, all aligned to one.
   */
  GiopLengthPrefixed = "giop-length-prefixed",
}

/** In what order a `wstring`'s code units land. */
export enum WstringUnitOrder {
  /** The stream's, which the encapsulation identifier announced. */
  Stream = "stream",
  /** Big-endian whatever the stream says. */
  BigEndianAlways = "big-endian-always",
  /** Preceded by a byte order mark. */
  ByteOrderMark = "byte-order-mark",
  /** The host's, unswapped, which is what Fast CDR writes. */
  HostUnswapped = "host-unswapped",
}

/** What a `wstring`'s length field counts, and in what units. */
export enum WstringLengthPolicy {
  /** §7.4.1.1.2: an octet count over UTF-16 units, no terminator. */
  Specification = "specification",
  /** A character count over two-octet units. */
  CharacterCount = "character-count",
  /** The same, with a terminator counted. */
  CharacterCountTerminated = "character-count-terminated",
  /** An octet count over four-octet units. */
  SpecificationWideUnits = "specification-wide-units",
  /** What `rosidl_typesupport_fastrtps` writes. */
  RosTypeSupport = "ros-type-support",
  /** An octet count with a two-octet terminator counted. */
  OctetsTerminated2 = "octets-terminated-2",
  /** The same with a four-octet one. */
  OctetsTerminated4 = "octets-terminated-4",
  /** A character count with a two-octet terminator counted. */
  CharactersTerminated2 = "characters-terminated-2",
}

/** What a `fixed`'s digit count is taken from. */
export enum FixedDigitCount {
  /** The IDL's `fixed<D, S>`. */
  Declared = "declared",
  /** The value's own precision (JacORB 3.9). */
  ValuePrecision = "value-precision",
  /**
   * The integer digits of the decimal as written, leading zeros
   * included, plus the S fraction digits, made odd by one leading zero: ACE/TAO 2.5.24's
   * `ACE_CDR::Fixed::from_string` through its own CDR operator (OpenDDS writes the declared count). `0.12345` under
   * `fixed<5,5>` is `0012345c`; past 31 digits it is refused. Read like
   * `ValuePrecision`.
   */
  ValueStringDigits = "value-string-digits",
}

/** What a PL_CDR1 parameter frames. */
export enum ParameterRole {
  /** A member of the type. */
  Member = "member",
  /** An `@optional` member outside a `@mutable` type. */
  Optional = "optional",
}

/**
 * What §7.2.2.7 says to do with a member the stream cannot give.
 *
 * The reader's, and named here because the generated decoder passes it
 * down as a parameter.
 */
export enum TryConstruct {
  /** Discard the whole sample. */
  Discard = "discard",
  /** Take Table 9's value. */
  UseDefault = "use-default",
  /** Keep what fits. §7.3.1.2.1.12.3. */
  Trim = "trim",
}

/**
 * C1 of {@link WstringLengthPolicy} — whether the length prefix counts
 * characters and not octets.
 *
 * A function per axis rather than three fields on an object, because a
 * TypeScript string enum is a name and a value and carries nothing
 * else; Java hangs the three on the enum constant and C on a table.
 * Which axis a cell sits on is the same in all three, and
 * `crates/ast-layout/src/key.rs` is where the measurements are.
 */
export function wstringCountsCharacters(policy: WstringLengthPolicy): boolean {
  switch (policy) {
    case WstringLengthPolicy.CharacterCount:
    case WstringLengthPolicy.CharacterCountTerminated:
    case WstringLengthPolicy.RosTypeSupport:
    case WstringLengthPolicy.CharactersTerminated2:
      return true;
    case WstringLengthPolicy.Specification:
    case WstringLengthPolicy.SpecificationWideUnits:
    case WstringLengthPolicy.OctetsTerminated2:
    case WstringLengthPolicy.OctetsTerminated4:
      return false;
  }
}

/** C2 — whether a terminating NUL code unit is written and counted. */
export function wstringIsTerminated(policy: WstringLengthPolicy): boolean {
  switch (policy) {
    case WstringLengthPolicy.CharacterCountTerminated:
    case WstringLengthPolicy.OctetsTerminated2:
    case WstringLengthPolicy.OctetsTerminated4:
    case WstringLengthPolicy.CharactersTerminated2:
      return true;
    case WstringLengthPolicy.Specification:
    case WstringLengthPolicy.CharacterCount:
    case WstringLengthPolicy.SpecificationWideUnits:
    case WstringLengthPolicy.RosTypeSupport:
      return false;
  }
}

/**
 * C3 — how many octets one of a `wstring`'s code units takes.
 *
 * **Not** the width of a scalar `wchar`, which is {@link WcharWidth}:
 * `rmw_cyclonedds_cpp` 1.3.4 writes two octets for a `wchar` member and
 * four for a `wstring`'s units, which is why the two are separate axes.
 */
export function wstringUnitWidth(policy: WstringLengthPolicy): 2 | 4 {
  switch (policy) {
    case WstringLengthPolicy.Specification:
    case WstringLengthPolicy.CharacterCount:
    case WstringLengthPolicy.OctetsTerminated2:
    case WstringLengthPolicy.CharactersTerminated2:
      return 2;
    case WstringLengthPolicy.CharacterCountTerminated:
    case WstringLengthPolicy.SpecificationWideUnits:
    case WstringLengthPolicy.RosTypeSupport:
    case WstringLengthPolicy.OctetsTerminated4:
      return 4;
  }
}

/**
 * Whether the units go out little-endian whatever the stream says.
 *
 * True for the byte-order-mark form, whose mark omniORB writes
 * little-endian, and for the host-unswapped one. The one thing the unit
 * writers ask of this axis besides `BigEndianAlways`.
 */
export function wstringOrderIsFixedLittle(order: WstringUnitOrder): boolean {
  return order === WstringUnitOrder.ByteOrderMark || order === WstringUnitOrder.HostUnswapped;
}

/** Why a reader stopped. */
export enum Status {
  Ok = "ok",
  Truncated = "truncated",
  ImpossibleLength = "impossible-length",
  OutOfRange = "out-of-range",
  Discarded = "discarded",
}
