# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The Nim wire runtime: DDS-XTypes 1.3 section 7.4 from the clause.
#
# The counterpart of crates/wire, runtime/c, runtime/go, runtime/d and
# the rest — a further implementation of the same clauses, not a binding
# to any of them. Where two columns disagree on an octet, one of them
# has misread the specification, and that is the point of having both.
# Nothing here decides a type question: which members are written, in
# what order, under which header and with which length code arrives as
# arguments from the generated codec.
#
# The operations take the shared walk's argument order — writeEnum
# (bitBound, value), writeBitset(anchor, value) — under the walk's
# spelling in lower camel, because this runtime was written against the
# walk rather than adapted to it afterwards.
#
# A malformed stream or a value outside its type raises a WireException
# and unwinds to the harness, which turns it into the verdict's exit
# code; a refusal about the *type* sets the reader's status instead and
# the generated decoder walks on to the end of the sample.
#
# An IDL `long double` is held as decimal text — Nim has `float32`,
# `float64` and no binary128, and a binary128 has 113 significand bits —
# so the two conversions live here: a correctly rounded parse (to
# nearest, ties to even, in one rounding) over a small limb integer, and
# the exact decimal expansion in the spelling the sibling columns'
# renderers produce.
#
# The PL_CDR1 reader follows runtime/csharp/src/Reader.cs: finishPidRead
# measures the value against its own start and checks the declared
# length against the configured convention, and the trailing alignment
# is written only under a framing that pads after the value — under
# Fast DDS's the next header aligns itself and a final parameter ends
# where its value does.

import std/math
import std/sequtils
import std/strutils
import std/unicode

# -- errors ----------------------------------------------------------------

type
  ## Base of every wire failure.
  WireException* = object of CatchableError
  ## The stream ended before the value did.
  Truncated* = object of WireException
  ## A length field claims more than the stream holds. Separate from
  ## Truncated: a length that cannot be true is a malformed stream, not
  ## a short one.
  ImpossibleLength* = object of WireException
  ## A string held bytes that are not UTF-8.
  NotUtf8* = object of WireException
  ## A value outside what its type admits.
  OutOfRange* = object of WireException

proc raiseTruncated(at, needed, available: int) {.noreturn.} =
  raise newException(Truncated,
    "at " & $at & ": " & $needed & " bytes needed, " & $available & " available")

proc raiseImpossible(at, claimed, available: int) {.noreturn.} =
  raise newException(ImpossibleLength,
    "at " & $at & ": a length of " & $claimed & " with " & $available & " bytes left")

proc raiseNotUtf8(at: int) {.noreturn.} =
  raise newException(NotUtf8, "at " & $at & ": the string is not UTF-8")

proc raiseOutOfRange(at: int, expected: string) {.noreturn.} =
  raise newException(OutOfRange, "at " & $at & ": expected " & expected)

# -- policies --------------------------------------------------------------

type
  ## The byte order a stream is written or read in.
  Endianness* = enum
    EndianLittle
    EndianBig

  ## What a receiving position does with a value it cannot hold.
  ## DDS-XTypes 1.3 7.2.2.7, Table 11. The field names are the walk's
  ## spellings, so the generated code names them directly.
  TryConstruct* = enum
    TryConstructDiscard
    TryConstructUseDefault
    TryConstructTrim

  ## What a reader answers about the type, without raising.
  Status* = enum
    StatusOk
    StatusTruncated
    StatusImpossibleLength
    StatusOutOfRange
    StatusDiscarded

  ## Which of 7.4.1.2's two roles a parameter header stands in.
  ParameterRole* = enum
    ParameterMember
    ParameterOptional

## A status, spelled the way the sibling runtimes log it.
proc statusText*(status: Status): string =
  case status
  of StatusOk: "ok"
  of StatusTruncated: "truncated"
  of StatusImpossibleLength: "impossible-length"
  of StatusOutOfRange: "out-of-range"
  of StatusDiscarded: "discarded"

# The parameter ids and flags of XTypes 1.3 7.4.1.2.2 and DDSI-RTPS 2.5
# 9.4.2.11.2.
const
  PidExtended* = 0x3F01
  PidListEnd* = 0x3F02
  PidIgnore* = 0x3F03
  PidShortMaxLen* = 0xFFFF
  PidMustUnderstand* = 0x4000
  PidImplExtension* = 0x8000
  eFlagMustUnderstand = 0x4000_0000'u32
  eFlagImplExtension = 0x8000_0000'u32
  eMemberIdMask = 0x0FFF_FFFF'u32
  pidIdMask = 0x3FFF

## What flags a PL_CDR1 parameter id carries at `position` — "member",
## "sentinel" or "extended-marker". An unknown policy raises: a typo in
## a wire-affecting setting must be loud.
proc pidFlagsAt(policy: string, position: string): int =
  if policy == "sentinel-must-understand":
    if position == "sentinel" or position == "extended-marker":
      return PidMustUnderstand
    return 0
  if policy == "bare":
    return 0
  if policy == "impl-extension-members":
    if position == "member" or position == "extended-marker":
      return PidImplExtension or PidMustUnderstand
    return 0
  raiseOutOfRange(0, "a PID-flag policy, not `" & policy & "`")

type
  ## How a PL_CDR1 parameter list is framed — the ways the measured
  ## implementations differ inside an XCDR1 sample.
  PlCdr1Framing* = object
    ## What flags a parameter id carries, asked per position.
    pidFlags*: string
    ## The highest member id a short parameter header carries. Both
    ## sides: `writePid` refuses an id above it rather than putting a
    ## reserved parameter on the wire.
    highestShortId*: int
    ## Whether a @mutable member's declared length covers its padding.
    padDeclaredLength*: bool
    ## The same question for an @optional member outside @mutable.
    padDeclaredOptionalLength*: bool
    ## Whether a member's own must-understand bit goes out at all.
    memberMustUnderstand*: bool
    ## Whether the parameter's own trailing padding to four is written
    ## (and read) after the value, or left to the next header.
    padAfterValue*: bool

proc padsDeclaredLength(framing: PlCdr1Framing, role: ParameterRole): bool =
  if role == ParameterOptional:
    framing.padDeclaredOptionalLength
  else:
    framing.padDeclaredLength

# -- the wstring C1 x C2 x C3 product --------------------------------------

type WstringCell = object
  countsCharacters: bool
  terminated: bool
  width: int

## The five named forms and the three coordinate spellings; the
## spellings are WstringLengthPolicy::as_str's. `false` says the name is
## not a policy at all.
proc wstringCellOf(policy: string, cell: var WstringCell): bool =
  case policy
  of "specification", "octets/unterminated/2":
    cell = WstringCell(countsCharacters: false, terminated: false, width: 2)
    true
  of "character-count", "characters/unterminated/2":
    cell = WstringCell(countsCharacters: true, terminated: false, width: 2)
    true
  of "character-count-terminated", "characters/terminated/4":
    cell = WstringCell(countsCharacters: true, terminated: true, width: 4)
    true
  of "specification-wide-units", "octets/unterminated/4":
    cell = WstringCell(countsCharacters: false, terminated: false, width: 4)
    true
  of "ros-type-support", "characters/unterminated/4":
    cell = WstringCell(countsCharacters: true, terminated: false, width: 4)
    true
  of "octets/terminated/2":
    cell = WstringCell(countsCharacters: false, terminated: true, width: 2)
    true
  of "octets/terminated/4":
    cell = WstringCell(countsCharacters: false, terminated: true, width: 4)
    true
  of "characters/terminated/2":
    cell = WstringCell(countsCharacters: true, terminated: true, width: 2)
    true
  else:
    false

proc wstringOrderKnown(order: string): bool =
  order == "stream" or order == "big-endian-always" or
    order == "byte-order-mark" or order == "host-unswapped"

const byteOrderMark = 0xFEFF'u16
const swappedByteOrderMark = 0xFFFE'u16

# -- handles ---------------------------------------------------------------

type
  ## A DHEADER waiting for its length; `at` below zero is the delimiter
  ## a @final aggregate does not write.
  DHeader* = object
    at*: int

  ## An EMHEADER waiting for its NEXTINT; `nextInt` is -1 where the
  ## length code carries the length itself.
  EmHeader* = object
    nextInt*: int
    word*: int
    ## The code the header was written with, so that `finishEmheader`
    ## can hold the member's value to the length the code declares.
    ## Codes 0 to 3 are lengths; anything else declares nothing of its
    ## own.
    lengthCode*: int

  ## A parameter header waiting for its length.
  Pid* = object
    at*: int
    extended*: bool
    savedOrigin*: int
    role*: ParameterRole
    pid*: int
    mustUnderstand*: bool

  ## Everything a two-pass read has to put back.
  ReaderMark* = object
    at*: int
    origin*: int
    status*: Status

  ## An EMHEADER as read. `extent` is how many octets the member
  ## occupies after the header word, the NEXTINT included.
  ## XTypes 1.3 7.4.3.4.2.
  EmHeaderRead* = object
    memberId*: int
    lengthCode*: int
    mustUnderstand*: bool
    nextInt*: int
    extent*: int

  ## A parameter header as read. `endOfList` and not `end`, which is a
  ## Nim keyword the generated field access could only reach stropped.
  PidRead* = object
    pid*: int
    length*: int
    endOfList*: bool
    mustUnderstand*: bool
    implExtension*: bool
    reserved*: bool
    ignore*: bool
    role*: ParameterRole
    savedOrigin*: int
    valueAt*: int

## Whether this parameter can be a member of the reading type: false
## for a vendor-private parameter and for a reserved id. 7.4.1.2.2,
## 7.2.2.4.4.4.6.
proc isUserMember*(header: PidRead): bool =
  not header.implExtension and not header.reserved and not header.ignore

## The DHEADER a value that takes no delimiter does not write; the
## writer's finishDheader ignores it.
proc noDheader*(): DHeader =
  DHeader(at: -1)

# -- helpers the generated code names --------------------------------------

## A traced cell holding a copy of `v`: what an `@external` member is
## assigned with — Nim has no address-of on a call's result, and a
## temporary per site would need a name no two sites in one scope could
## share.
proc zdRef*[T](v: T): ref T =
  new(result)
  result[] = v

## The walk's Int64 widening. A conversion for everything that fits one
## — an enumeration, a signed or narrow unsigned integer — and a `cast`
## for `uint64`, whose values above `int64.high` the conversion would
## refuse where the union overlay needs the bits.
proc zdI64*[T](v: T): int64 =
  when T is uint64:
    cast[int64](v)
  else:
    int64(v)

## C11 7.24.4's unsigned-octet order, as the sign the walk compares.
## Nim's own string order over UTF-8 is exactly that order.
proc strcmp*(a, b: string): int =
  if a < b: -1
  elif a > b: 1
  else: 0

## How many octets `text` is, which is what a string<N> bound counts.
proc utf8Octets*(text: string): int =
  text.len

## How many UTF-16 code units `text` is, which is what a wstring<N>
## bound counts. A scalar above the basic plane is two.
proc utf16Units*(text: string): int =
  for r in text.runes:
    result += (if int(r) > 0xFFFF: 2 else: 1)

## `text` cut to at most `octets` bytes, never inside a character.
## Table 11: the order is preserved, so this is a prefix.
proc trimUtf8*(text: string, octets: int): string =
  var used = 0
  for r in text.runes:
    let size = r.size
    if used + size > octets:
      break
    used += size
  text[0 ..< used]

## `text` cut to at most `units` UTF-16 code units.
proc trimUtf16*(text: string, units: int): string =
  var used = 0
  var octets = 0
  for r in text.runes:
    let cost = if int(r) > 0xFFFF: 2 else: 1
    if used + cost > units:
      break
    used += cost
    octets += r.size
  text[0 ..< octets]

## `point` as the one octet an IDL char is, refused above U+00FF rather
## than truncated: the refusal is the sample's — the same one the
## sibling runtimes raise.
proc checkedChar*(point: uint32): uint8 =
  if point > 0xFF'u32:
    raiseOutOfRange(0, "a character below U+0100")
  uint8(point)

## `point` as the one UTF-16 code unit an IDL wchar is, refused outside
## the BMP. 7.2.2.2.1.2.5.
proc checkedWchar*(point: uint32): uint16 =
  if point > 0xFFFF'u32:
    raiseOutOfRange(0, "a BMP character")
  uint16(point)

## A 64-bit signed integer in decimal: a JSON number is a double and
## loses the low bits past 2^53, so both 64-bit widths travel as
## strings.
proc formatI64*(v: int64): string =
  $v

## A 64-bit unsigned integer in decimal.
proc formatU64*(v: uint64): string =
  $v

## Whether a float has a JSON form at all: NaN and the infinities are
## not JSON.
proc jsonNumberOk*(v: float64): bool =
  v.classify notin {fcNan, fcInf, fcNegInf}

## The low `octets` of `value`'s two's-complement bits, zero-extended —
## writeUnionOverlay's pattern for an integer, character, enumeration
## or bitmask arm. The mask is the function: a plain widening of a
## negative value sign-extends, and `-2` in four octets has to be
## 0x00000000FFFFFFFE.
proc bitPatternInt*(value: int64, octets: int): uint64 =
  let pattern = cast[uint64](value)
  if octets >= 8:
    pattern
  else:
    pattern and ((1'u64 shl (octets * 8)) - 1)

## One octet, 0 or 1 — the boolean overlay pattern.
proc bitPatternBool*(value: bool): uint64 =
  if value: 1 else: 0

## The IEEE 754 binary32 bits, zero-extended.
proc bitPatternF32*(value: float32): uint64 =
  uint64(cast[uint32](value))

## The IEEE 754 binary64 bits.
proc bitPatternF64*(value: float64): uint64 =
  cast[uint64](value)

## The UTF-16 code units of `v`, without a BOM. Invalid UTF-8 is
## refused: writing its replacement character would put a string on the
## wire the caller never held.
proc utf16UnitsOf(v: string, at: int): seq[uint16] =
  if validateUtf8(v) != -1:
    raiseOutOfRange(at, "a UTF-8 wstring")
  for r in v.runes:
    let point = int(r)
    if point < 0x10000:
      result.add(uint16(point))
    else:
      let offset = point - 0x10000
      result.add(uint16(0xD800 + (offset shr 10)))
      result.add(uint16(0xDC00 + (offset and 0x3FF)))

# -- binary128 as limbs ----------------------------------------------------
#
# Nim's standard library has no arbitrary-precision integer, and a
# binary128 parse that is correctly rounded needs one: the exact
# rational, then one rounding on it — a two-step parse through a double
# rounds twice. The limb type below carries exactly the operations the
# two conversions use and nothing else.

type ZdBig = seq[uint32] # little-endian limbs, no trailing zero limbs

proc bigNormalise(a: var ZdBig) =
  while a.len > 0 and a[^1] == 0'u32:
    a.setLen(a.len - 1)

proc bigIsZero(a: ZdBig): bool =
  a.len == 0

proc bigFromInt(v: uint64): ZdBig =
  if v == 0: return @[]
  result = @[uint32(v and 0xFFFF_FFFF'u64)]
  if v > 0xFFFF_FFFF'u64:
    result.add(uint32(v shr 32))

proc bigMulSmall(a: ZdBig, m: uint32): ZdBig =
  var carry = 0'u64
  for limb in a:
    let wide = uint64(limb) * uint64(m) + carry
    result.add(uint32(wide and 0xFFFF_FFFF'u64))
    carry = wide shr 32
  if carry > 0'u64:
    result.add(uint32(carry))
  bigNormalise(result)

proc bigAddSmall(a: ZdBig, v: uint32): ZdBig =
  result = a
  var carry = uint64(v)
  var i = 0
  while carry > 0'u64:
    if i >= result.len:
      result.add(0'u32)
    let wide = uint64(result[i]) + carry
    result[i] = uint32(wide and 0xFFFF_FFFF'u64)
    carry = wide shr 32
    inc i
  bigNormalise(result)

proc bigCmp(a, b: ZdBig): int =
  if a.len != b.len:
    return (if a.len < b.len: -1 else: 1)
  for i in countdown(a.len - 1, 0):
    if a[i] != b[i]:
      return (if a[i] < b[i]: -1 else: 1)
  0

## a - b; the caller guarantees a >= b.
proc bigSub(a, b: ZdBig): ZdBig =
  result = a
  var borrow = 0'i64
  for i in 0 ..< result.len:
    var diff = int64(result[i]) - borrow - (if i < b.len: int64(b[i]) else: 0'i64)
    if diff < 0:
      diff += 0x1_0000_0000'i64
      borrow = 1
    else:
      borrow = 0
    result[i] = uint32(diff)
  bigNormalise(result)

proc bigShl(a: ZdBig, bits: int): ZdBig =
  if bigIsZero(a) or bits == 0:
    result = a
    bigNormalise(result)
    return
  let limbs = bits div 32
  let rest = bits mod 32
  result = newSeq[uint32](limbs)
  var carry = 0'u32
  for limb in a:
    if rest == 0:
      result.add(limb)
    else:
      result.add((limb shl rest) or carry)
      carry = limb shr (32 - rest)
  if rest != 0 and carry != 0'u32:
    result.add(carry)
  bigNormalise(result)

proc bigShr(a: ZdBig, bits: int): ZdBig =
  if bits == 0:
    result = a
    return
  let limbs = bits div 32
  let rest = bits mod 32
  if limbs >= a.len:
    return @[]
  for i in limbs ..< a.len:
    var limb = a[i] shr rest
    if rest != 0 and i + 1 < a.len:
      limb = limb or (a[i + 1] shl (32 - rest))
    result.add(limb)
  bigNormalise(result)

proc bigBit(a: ZdBig, index: int): bool =
  let limb = index div 32
  if limb >= a.len:
    return false
  ((a[limb] shr (index mod 32)) and 1'u32) == 1'u32

proc bigSetBit(a: var ZdBig, index: int) =
  let limb = index div 32
  while a.len <= limb:
    a.add(0'u32)
  a[limb] = a[limb] or (1'u32 shl (index mod 32))

proc bigBitLength(a: ZdBig): int =
  if a.len == 0:
    return 0
  var top = a[^1]
  result = (a.len - 1) * 32
  while top > 0'u32:
    top = top shr 1
    inc result

## Shift-subtract division: quotient and remainder of a / b, b != 0.
proc bigDivMod(a, b: ZdBig): (ZdBig, ZdBig) =
  var quotient: ZdBig = @[]
  var remainder: ZdBig = @[]
  for i in countdown(bigBitLength(a) - 1, 0):
    remainder = bigShl(remainder, 1)
    if bigBit(a, i):
      remainder = bigAddSmall(remainder, 1)
    if bigCmp(remainder, b) >= 0:
      remainder = bigSub(remainder, b)
      bigSetBit(quotient, i)
  bigNormalise(quotient)
  (quotient, remainder)

proc bigDivModSmall(a: ZdBig, m: uint32): (ZdBig, uint32) =
  var quotient = newSeq[uint32](a.len)
  var rest = 0'u64
  for i in countdown(a.len - 1, 0):
    let wide = (rest shl 32) or uint64(a[i])
    quotient[i] = uint32(wide div uint64(m))
    rest = wide mod uint64(m)
  bigNormalise(quotient)
  (quotient, uint32(rest))

proc bigPow(base: uint32, exponent: int): ZdBig =
  result = @[1'u32]
  for _ in 0 ..< exponent:
    result = bigMulSmall(result, base)

proc bigToDecimal(a: ZdBig): string =
  if bigIsZero(a):
    return "0"
  var value = a
  var chunks: seq[uint32] = @[]
  while not bigIsZero(value):
    let (quotient, rest) = bigDivModSmall(value, 1_000_000_000'u32)
    chunks.add(rest)
    value = quotient
  result = $chunks[^1]
  for i in countdown(chunks.len - 2, 0):
    result.add(strutils.align($chunks[i], 9, '0'))

const f128FractionBits = 112
const f128ExponentBias = 16383
const f128ExponentMax = 0x7FFF

proc f128SignMask(): ZdBig =
  bigShl(@[1'u32], 127)

proc f128FractionMask(): ZdBig =
  bigSub(bigShl(@[1'u32], f128FractionBits), @[1'u32])

proc f128ExponentBits(): ZdBig =
  bigShl(bigFromInt(uint64(f128ExponentMax)), f128FractionBits)

proc f128QuietBit(): ZdBig =
  bigShl(@[1'u32], f128FractionBits - 1)

proc bigOr(a, b: ZdBig): ZdBig =
  result = a
  while result.len < b.len:
    result.add(0'u32)
  for i in 0 ..< b.len:
    result[i] = result[i] or b[i]
  bigNormalise(result)

proc bigAnd(a, b: ZdBig): ZdBig =
  for i in 0 ..< min(a.len, b.len):
    result.add(a[i] and b[i])
  bigNormalise(result)

proc f128Bytes(bits: ZdBig): array[16, uint8] =
  var value = bits
  for i in 0 ..< 16:
    if value.len > 0:
      result[i] = uint8(value[0] and 0xFF'u32)
    value = bigShr(value, 8)

## Shift right, rounding to nearest with ties to even.
proc f128ShiftRound(value: ZdBig, shift: int, dropped: var bool): ZdBig =
  dropped = false
  if shift <= 0:
    return value
  let kept = bigShr(value, shift)
  let mask = bigSub(bigShl(@[1'u32], shift), @[1'u32])
  let remainder = bigAnd(value, mask)
  if bigIsZero(remainder):
    return kept
  dropped = true
  let half = bigShl(@[1'u32], shift - 1)
  let comparison = bigCmp(remainder, half)
  if comparison > 0 or (comparison == 0 and bigBit(kept, 0)):
    return bigAddSmall(kept, 1)
  kept

## Build a pattern from a sign, an integral significand and a binary
## exponent, rounding to nearest with ties to even.
proc f128Compose(negative: bool, significand: ZdBig, exponent: int): ZdBig =
  let sign = if negative: f128SignMask() else: @[]
  if bigIsZero(significand):
    return sign
  if exponent > f128ExponentMax:
    return bigOr(sign, f128ExponentBits())
  if exponent < -(f128FractionBits + 1 + f128ExponentBias + 200):
    return sign
  var value = significand
  var exponent = exponent
  var dropped = false
  let leading = bigBitLength(value) - 1
  if leading > f128FractionBits:
    let drop = leading - f128FractionBits
    value = f128ShiftRound(value, drop, dropped)
    exponent += drop
    if bigBitLength(value) - 1 > f128FractionBits:
      value = bigShr(value, 1)
      exponent += 1
  else:
    let shift = f128FractionBits - leading
    value = bigShl(value, shift)
    exponent -= shift
  let biased = exponent + f128ExponentBias + f128FractionBits
  if biased >= f128ExponentMax:
    return bigOr(sign, f128ExponentBits())
  if biased <= 0:
    # Subnormal; a value that rounds up to the smallest normal carries
    # into the exponent field on its own.
    let rounded = f128ShiftRound(value, 1 - biased, dropped)
    return bigOr(sign, rounded)
  bigOr(sign,
    bigOr(bigShl(bigFromInt(uint64(biased)), f128FractionBits),
      bigAnd(value, f128FractionMask())))

## Round numerator/denominator to binary128, to nearest with ties to
## even: scale so the quotient carries more bits than binary128 needs,
## fold everything discarded into one sticky bit, round once.
proc f128FromFraction(negative: bool, numerator, denominator: ZdBig): ZdBig =
  const wanted = 130
  var shift = wanted - (bigBitLength(numerator) - bigBitLength(denominator))
  if shift < 0:
    shift = 0
  let scaled = bigShl(numerator, shift)
  var (quotient, remainder) = bigDivMod(scaled, denominator)
  if not bigIsZero(remainder):
    quotient = bigOr(quotient, @[1'u32])
  f128Compose(negative, quotient, -shift)

## Parse `text` and answer the sixteen bytes, least-significant first.
## A malformed decimal raises OutOfRange at `at`.
proc f128FromDecimal(text: string, at: int): array[16, uint8] =
  # Qualified, because `std/strutils` and `std/unicode` both export a
  # `strip` that matches `(string)`. Nim 2.0.8 — the version the CI job
  # pins — calls that ambiguous and refuses the module; 2.2.10 (this
  # workstation and the measurement host) resolves it and compiles. The
  # module name says which one, on every version.
  let stripped = strutils.strip(text)
  case toLowerAscii(stripped)
  of "inf", "+inf", "infinity", "+infinity":
    return f128Bytes(f128ExponentBits())
  of "-inf", "-infinity":
    return f128Bytes(bigOr(f128ExponentBits(), f128SignMask()))
  of "nan", "+nan":
    return f128Bytes(bigOr(f128ExponentBits(), f128QuietBit()))
  of "-nan":
    return f128Bytes(bigOr(bigOr(f128ExponentBits(), f128QuietBit()), f128SignMask()))
  else:
    discard

  var negative = false
  var digitsPart = stripped
  if digitsPart.len > 0 and (digitsPart[0] == '+' or digitsPart[0] == '-'):
    negative = digitsPart[0] == '-'
    digitsPart = digitsPart[1 .. ^1]
  var mantissa = digitsPart
  var exponent10 = 0
  var split = digitsPart.find('e')
  if split < 0:
    split = digitsPart.find('E')
  if split >= 0:
    mantissa = digitsPart[0 ..< split]
    try:
      exponent10 = parseInt(digitsPart[split + 1 .. ^1])
    except ValueError:
      raiseOutOfRange(at, "a decimal for a long double, not `" & text & "`")
  var whole = mantissa
  var fraction = ""
  let point = mantissa.find('.')
  if point >= 0:
    whole = mantissa[0 ..< point]
    fraction = mantissa[point + 1 .. ^1]
  let digits = whole & fraction
  if digits.len == 0:
    raiseOutOfRange(at, "a decimal for a long double, not `" & text & "`")
  var significand: ZdBig = @[]
  for c in digits:
    # An underscore is refused rather than ignored: in a value file it
    # is a typo, not a digit separator.
    if c < '0' or c > '9':
      raiseOutOfRange(at, "a decimal for a long double, not `" & text & "`")
    significand = bigAddSmall(bigMulSmall(significand, 10), uint32(ord(c) - ord('0')))
  exponent10 -= fraction.len

  if bigIsZero(significand):
    if negative:
      return f128Bytes(f128SignMask())
    return f128Bytes(@[])
  # The exact rational, then one correct rounding on it: a two-step
  # parse through a double rounds twice.
  var numerator = significand
  var denominator: ZdBig = @[1'u32]
  if exponent10 >= 0:
    for _ in 0 ..< exponent10:
      numerator = bigMulSmall(numerator, 10)
  else:
    denominator = bigPow(10, -exponent10)
  f128Bytes(f128FromFraction(negative, numerator, denominator))

## 0.<digits> * 10^point, in the three forms all the columns use.
proc f128Render(negative: bool, digits: string, point: int): string =
  let sign = if negative: "-" else: ""
  if 0 < point and point <= 34:
    if digits.len <= point:
      return sign & digits & repeat('0', point - digits.len) & ".0"
    return sign & digits[0 ..< point] & "." & digits[point .. ^1]
  if -6 < point and point <= 0:
    return sign & "0." & repeat('0', -point) & digits
  var mantissa = digits[0 ..< 1] & "."
  if digits.len > 1:
    mantissa.add(digits[1 .. ^1])
  else:
    mantissa.add("0")
  let exponent = point - 1
  if exponent >= 0:
    return sign & mantissa & "e+" & $exponent
  sign & mantissa & "e-" & $(-exponent)

## Every digit the value has, in the corpus's canonical form — the
## exact expansion, in exactly the spelling the sibling renderers
## produce, because the columns are compared as text.
proc f128ToExactDecimal(bytes: array[16, uint8]): string =
  var bits: ZdBig = @[]
  for i in 0 ..< 16:
    if bytes[i] != 0'u8:
      bits = bigOr(bits, bigShl(bigFromInt(uint64(bytes[i])), i * 8))

  let negative = not bigIsZero(bigAnd(bits, f128SignMask()))
  let biasedBig = bigShr(bits, f128FractionBits)
  var biased = 0
  if biasedBig.len > 0:
    biased = int(biasedBig[0] and 0x7FFF'u32)
  let fraction = bigAnd(bits, f128FractionMask())

  if biased == f128ExponentMax:
    if not bigIsZero(fraction):
      return "nan"
    return (if negative: "-inf" else: "inf")
  var significand: ZdBig
  var exponent: int
  if biased == 0:
    significand = fraction
    exponent = 1 - f128ExponentBias - f128FractionBits
  else:
    significand = bigOr(fraction, bigShl(@[1'u32], f128FractionBits))
    exponent = biased - f128ExponentBias - f128FractionBits
  if bigIsZero(significand):
    return (if negative: "-0.0" else: "0.0")
  var digits: string
  var point: int
  if exponent >= 0:
    digits = bigToDecimal(bigShl(significand, exponent))
    point = digits.len
  else:
    # m * 2^-k is exactly m * 5^k * 10^-k.
    let scale = -exponent
    var scaled = significand
    for _ in 0 ..< scale:
      scaled = bigMulSmall(scaled, 5)
    digits = bigToDecimal(scaled)
    point = digits.len - scale
  while digits.len > 0 and digits[^1] == '0':
    digits.setLen(digits.len - 1)
  if digits.len == 0:
    digits = "0"
  f128Render(negative, digits, point)

# -- fixed BCD -------------------------------------------------------------

proc fixedNibbles(text: string, digits, scale: int, count: string, at: int): seq[int] =
  if digits == 0:
    raiseOutOfRange(at, "a fixed with at least one digit")
  # OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
  # fixed<31, S> is the widest declaration IDL admits. A wider one has no
  # packing defined for it, so it is refused rather than packed into
  # octets no reader can be told the width of.
  if digits > 31:
    raiseOutOfRange(at, "a fixed of at most 31 digits, not " & $digits)
  if scale > digits:
    raiseOutOfRange(at, "a scale no larger than the digit count")
  if count != "declared" and count != "value-precision" and
      count != "value-string-digits":
    raiseOutOfRange(at, "a fixed digit count, not `" & count & "`")
  var negative = false
  var magnitude = text
  if magnitude.len > 0 and (magnitude[0] == '-' or magnitude[0] == '+'):
    negative = magnitude[0] == '-'
    magnitude = magnitude[1 .. ^1]
  var whole = magnitude
  var fraction = ""
  let point = magnitude.find('.')
  if point >= 0:
    whole = magnitude[0 ..< point]
    fraction = magnitude[point + 1 .. ^1]
  proc refuseFixed(at: int, text: string, digits, scale: int) {.noreturn.} =
    raiseOutOfRange(at, "a decimal that fits fixed<" & $digits & ", " & $scale &
      ">, not `" & text & "`")
  if whole.len == 0 and fraction.len == 0:
    refuseFixed(at, text, digits, scale)
  for c in whole:
    if c < '0' or c > '9':
      refuseFixed(at, text, digits, scale)
  for c in fraction:
    if c < '0' or c > '9':
      refuseFixed(at, text, digits, scale)
  let asWritten = whole.len + scale
  let integralWidth = digits - scale
  while whole.len > 0 and whole[0] == '0':
    whole = whole[1 .. ^1]
  if whole.len > integralWidth:
    raiseOutOfRange(at, "a decimal whose integer part fits " & $integralWidth &
      " digits, not `" & text & "`")
  if fraction.len > scale:
    raiseOutOfRange(at, "a decimal whose fraction fits " & $scale &
      " digits, not `" & text & "`")

  var every: seq[int] = @[]
  for _ in 0 ..< integralWidth - whole.len:
    every.add(0)
  for c in whole:
    every.add(ord(c) - ord('0'))
  for c in fraction:
    every.add(ord(c) - ord('0'))
  for _ in 0 ..< scale - fraction.len:
    every.add(0)
  var significant = 0
  for i in 0 ..< every.len:
    if every[i] != 0:
      significant = every.len - i
      break
  var wire = every.len
  if count == "value-precision":
    wire = significant
    if wire < 1:
      wire = 1
  elif count == "value-string-digits":
    # ACE/TAO 2.5.24's ACE_CDR::Fixed::from_string (OpenDDS writes the declared count): the
    # digits as written, made odd by one leading zero (0.12345 under
    # fixed<5,5> is 0012345c). Past 31 no reader can be told the width.
    wire = asWritten + (if asWritten mod 2 == 0: 1 else: 0)
    if wire > 31:
      raiseOutOfRange(at, "a fixed of at most 31 digits on the wire, not " & $wire &
        " for `" & text & "`")
  var nibbles: seq[int] = @[]
  if (wire + 1) mod 2 == 1:
    nibbles.add(0)
  # A count wider than P is filled with leading zeros.
  let kept = min(wire, every.len)
  for _ in kept ..< wire:
    nibbles.add(0)
  nibbles.add(every[every.len - kept .. ^1])
  var sign = 0x0C
  if negative and significant != 0:
    sign = 0x0D
  nibbles.add(sign)
  nibbles

# -- the writer ------------------------------------------------------------

type
  ## Accumulates the bytes of one sample. Alignment is relative to the
  ## start of the stream: XCDR aligns each primitive to its own size
  ## counted from the encapsulation. XTypes 1.3 7.4.1.1.
  Writer* = ref object
    outBytes: seq[uint8]
    endianness: Endianness
    origin: int
    maxAlign: int
    plCdr1: PlCdr1Framing
    ## Whether an LC4 header whose member turns out to be 1, 2, 4 or 8
    ## bytes is rewritten with a short length code (Fast DDS's form).
    shortLengthCodes: bool

proc newWriter*(endianness: Endianness, maxAlign: int, plCdr1: PlCdr1Framing,
                shortLengthCodes: bool): Writer =
  Writer(endianness: endianness, maxAlign: max(maxAlign, 1), plCdr1: plCdr1,
         shortLengthCodes: shortLengthCodes)

## How many bytes have been written.
proc position*(w: Writer): int =
  w.outBytes.len

## The bytes written, as a copy.
proc finish*(w: Writer): seq[uint8] =
  w.outBytes

proc alignTo(w: Writer, alignment: int) =
  var alignment = alignment
  if alignment > w.maxAlign:
    alignment = w.maxAlign
  if alignment <= 1:
    return
  let offset = w.outBytes.len - w.origin
  let padding = (alignment - offset mod alignment) mod alignment
  for _ in 0 ..< padding:
    w.outBytes.add(0'u8)

proc writeRaw(w: Writer, unsigned: uint64, width: int) =
  if w.endianness == EndianLittle:
    for i in 0 ..< width:
      w.outBytes.add(uint8((unsigned shr (8 * i)) and 0xFF'u64))
  else:
    for i in countdown(width - 1, 0):
      w.outBytes.add(uint8((unsigned shr (8 * i)) and 0xFF'u64))

proc writeInt(w: Writer, unsigned: uint64, width: int) =
  w.alignTo(width)
  w.writeRaw(unsigned, width)

# -- primitives ------------------------------------------------------------

## `boolean`: one byte, 0 or 1.
proc writeBool*(w: Writer, v: bool) =
  w.outBytes.add(if v: 1'u8 else: 0'u8)

## `octet` and `uint8`.
proc writeU8*(w: Writer, v: uint8) =
  w.outBytes.add(v)

## `int8`.
proc writeI8*(w: Writer, v: int8) =
  w.outBytes.add(cast[uint8](v))

## `char`: one octet — which is the whole member.
proc writeChar*(w: Writer, v: uint8) =
  w.outBytes.add(v)

## `short`.
proc writeI16*(w: Writer, v: int16) =
  w.writeInt(uint64(cast[uint16](v)), 2)

## `unsigned short`.
proc writeU16*(w: Writer, v: uint16) =
  w.writeInt(uint64(v), 2)

## `long` — 32 bits.
proc writeI32*(w: Writer, v: int32) =
  w.writeInt(uint64(cast[uint32](v)), 4)

## `unsigned long`.
proc writeU32*(w: Writer, v: uint32) =
  w.writeInt(uint64(v), 4)

## `long long`.
proc writeI64*(w: Writer, v: int64) =
  w.writeInt(cast[uint64](v), 8)

## `unsigned long long`.
proc writeU64*(w: Writer, v: uint64) =
  w.writeInt(v, 8)

## `float`.
proc writeF32*(w: Writer, v: float32) =
  w.writeInt(uint64(cast[uint32](v)), 4)

## `double`.
proc writeF64*(w: Writer, v: float64) =
  w.writeInt(cast[uint64](v), 8)

## `long double`: sixteen bytes of IEEE 754 binary128, aligned to eight
## (capped by maxAlign). The value arrives as decimal text and is
## correctly rounded here, once. The two byte orders are the whole
## sixteen bytes reversed — CORBA 3.3 part 2 9.3.1.3.
proc writeF128*(w: Writer, v: string) =
  let bytes = f128FromDecimal(v, w.outBytes.len)
  w.alignTo(8)
  if w.endianness == EndianLittle:
    for i in 0 ..< 16:
      w.outBytes.add(bytes[i])
  else:
    for i in countdown(15, 0):
      w.outBytes.add(bytes[i])

# -- characters and strings ------------------------------------------------

## A 16-bit value little-endian whatever the stream's order is.
proc writeU16Little*(w: Writer, v: uint16) =
  w.alignTo(2)
  w.outBytes.add(uint8(v and 0xFF'u16))
  w.outBytes.add(uint8(v shr 8))

## A 16-bit value big-endian whatever the stream's order is.
proc writeU16Big*(w: Writer, v: uint16) =
  w.alignTo(2)
  w.outBytes.add(uint8(v shr 8))
  w.outBytes.add(uint8(v and 0xFF'u16))

## A 32-bit value little-endian whatever the stream's order is.
proc writeU32Little*(w: Writer, v: uint32) =
  w.alignTo(4)
  for i in 0 ..< 4:
    w.outBytes.add(uint8((v shr (8 * i)) and 0xFF'u32))

## A 32-bit value big-endian whatever the stream's order is.
proc writeU32Big*(w: Writer, v: uint32) =
  w.alignTo(4)
  for i in countdown(3, 0):
    w.outBytes.add(uint8((v shr (8 * i)) and 0xFF'u32))

## `wchar`, at `width` octets and in `framing`'s shape.
## "giop-length-prefixed" is CORBA 3.3 part 2 9.3.1.6's GIOP 1.2 form:
## the octet count first, then the code unit's octets, all aligned to
## one; every order but "stream" means big-endian there.
proc writeWchar*(w: Writer, v: uint16, width: int, framing: string, order: string) =
  if width != 2 and width != 4:
    raiseOutOfRange(w.outBytes.len, "a wchar width of 2 or 4")
  if framing == "giop-length-prefixed":
    w.outBytes.add(uint8(width))
    let big = order != "stream" or w.endianness != EndianLittle
    if big:
      for i in countdown(width - 1, 0):
        w.outBytes.add(uint8((uint32(v) shr (8 * i)) and 0xFF'u32))
    else:
      for i in 0 ..< width:
        w.outBytes.add(uint8((uint32(v) shr (8 * i)) and 0xFF'u32))
    return
  if framing != "fixed":
    raiseOutOfRange(w.outBytes.len, "a wchar framing, not `" & framing & "`")
  if width == 4:
    w.writeU32(uint32(v))
    return
  w.writeU16(v)

## A string: a four-byte length including the terminator, the UTF-8
## bytes, then a NUL. The length counts bytes. XTypes 1.3 7.4.1.1.
proc writeString*(w: Writer, v: string) =
  w.writeU32(uint32(v.len + 1))
  for c in v:
    w.outBytes.add(uint8(ord(c)))
  w.outBytes.add(0'u8)

## A bounded string: the same bytes writeString writes. The bound is a
## promise about the source and not a difference on the wire, so this
## is the only place it can be enforced — refused rather than cut at a
## character boundary, which would put a sample on the wire nobody
## wrote.
proc writeStringBounded*(w: Writer, v: string, bound: int) =
  if v.len > bound:
    raiseOutOfRange(w.outBytes.len,
      "at most " & $bound & " octets in a bounded string, and this one has " & $v.len)
  w.writeString(v)

## The count check every bounded sequence makes before a byte of it is
## written. Table 17 makes O2.length <= T1.length the condition for
## constructing a sequence<T, N>, and the wire carries no bound to
## compare against: a sender whose count already exceeds its own
## declaration has written a value of a type nobody declared.
proc checkSequenceBound*(w: Writer, actual: uint32, bound: uint32) =
  if actual > bound:
    raiseOutOfRange(w.outBytes.len,
      "at most " & $bound & " elements in a bounded sequence, and this one has " & $actual)

## The count check every bounded map makes before a byte of it is
## written. Its own message and not checkSequenceBound's: the unit is
## an entry and not an element, per XTypes 1.3 7.4.1.1.4's
## map-as-a-sequence-of-pairs equivalence.
proc checkMapBound*(w: Writer, actual: uint32, bound: uint32) =
  if actual > bound:
    raiseOutOfRange(w.outBytes.len,
      "at most " & $bound & " entries in a bounded map, and this one has " & $actual)

proc writeWstringUnit(w: Writer, unit: uint16, width: int, order: string) =
  let little = order == "byte-order-mark" or order == "host-unswapped"
  if width == 4:
    if order == "big-endian-always":
      w.writeU32Big(uint32(unit))
    elif little:
      w.writeU32Little(uint32(unit))
    else:
      w.writeU32(uint32(unit))
    return
  if order == "big-endian-always":
    w.writeU16Big(unit)
  elif little:
    w.writeU16Little(unit)
  else:
    w.writeU16(unit)

## A wstring, in the form `policy` and `order` name. The value arrives
## as UTF-8 and the conversion to UTF-16 units happens here, which is
## what makes 7.4.1.1.2's surrogate rule this runtime's own reading.
proc writeWstringUtf8*(w: Writer, v: string, policy: string, order: string) =
  var cell: WstringCell
  if not wstringCellOf(policy, cell):
    raiseOutOfRange(w.outBytes.len, "a wstring length policy, not `" & policy & "`")
  if not wstringOrderKnown(order):
    raiseOutOfRange(w.outBytes.len, "a wstring unit order, not `" & order & "`")
  var units = utf16UnitsOf(v, w.outBytes.len)
  if cell.countsCharacters and cell.width == 2:
    # A character count over two-octet units has no way to carry a code
    # point outside the BMP.
    for index, unit in units:
      if unit >= 0xD800'u16 and unit <= 0xDFFF'u16:
        raiseOutOfRange(index, "a code point inside the BMP: a character count over " &
          "two-octet units has no way to carry one outside it")
  if cell.terminated:
    units.add(0'u16)
  # An empty wstring carries no mark.
  if order == "byte-order-mark" and units.len > 0:
    units.insert(byteOrderMark, 0)
  if cell.countsCharacters:
    w.writeU32(uint32(units.len))
  else:
    w.writeU32(uint32(units.len * cell.width))
  for unit in units:
    w.writeWstringUnit(unit, cell.width, order)

## A bounded wstring, as writeStringBounded is to writeString. The
## bound counts UTF-16 code units, not characters and not octets,
## whatever `policy` puts on the wire.
proc writeWstringBounded*(w: Writer, v: string, bound: int, policy: string, order: string) =
  let units = utf16UnitsOf(v, w.outBytes.len)
  if units.len > bound:
    raiseOutOfRange(w.outBytes.len,
      "at most " & $bound & " UTF-16 code units in a bounded wstring, and this one has " &
        $units.len)
  w.writeWstringUtf8(v, policy, order)

## A wstring whose characters go out as their own UTF-8 octets.
##
## `WstringCodeUnits::Utf8` is the measured departure of one product's
## generated serialiser: XTypes 1.3 Table 31 gives a wstring's code unit
## as Char16 and 7.4.1.1.2 makes the length "twice the number of
## characters", so the clause has no UTF-8 form at all. The declared
## length is the UTF-8 octet count plus the terminator the policy asks
## for. wstring-member is `04 00 00 00 61 62 63 00` for "abc": three
## octets and a NUL, and the four declares both.
proc writeWstringUtf8Octets*(w: Writer, v: string, policy: string) =
  var cell: WstringCell
  if not wstringCellOf(policy, cell):
    raiseOutOfRange(w.outBytes.len, "a wstring length policy, not `" & policy & "`")
  # The same malformed-UTF-8 refusal writeWstringUtf8 makes.
  if validateUtf8(v) != -1:
    raiseNotUtf8(w.outBytes.len)
  let length = v.len + (if cell.terminated: 1 else: 0)
  if length > 0xFFFFFFFF:
    raiseOutOfRange(w.outBytes.len, "a wstring length the UInt32 field can hold")
  w.writeU32(uint32(length))
  for c in v:
    w.outBytes.add(uint8(c))
  if cell.terminated:
    w.outBytes.add(0'u8)

## A bounded UTF-8-octet wstring, as writeWstringBounded is to
## writeWstringUtf8: the bound still counts UTF-16 code units, which is
## what a wstring<N> bound means to every peer, while the wire carries
## the UTF-8 octets.
proc writeWstringUtf8OctetsBounded*(w: Writer, v: string, bound: int, policy: string) =
  let units = utf16UnitsOf(v, w.outBytes.len)
  if units.len > bound:
    raiseOutOfRange(w.outBytes.len,
      "at most " & $bound & " UTF-16 code units in a bounded wstring, and this one has " &
        $units.len)
  w.writeWstringUtf8Octets(v, policy)

## Move the alignment origin to the current position and answer the
## origin displaced, for finishPackedElement.
##
## `ElementPadding::Packed` is HDDS's measured departure from 7.4.1,
## which counts every element's alignment from the encapsulation: each
## collection element and each aggregate value counts its own alignment
## from its own first octet instead. array-1d-struct-final is
## `01000000 0200 03000000 0400 77`, the second Cell's long at offset
## six where the clause puts it at eight.
proc beginPackedElement*(w: Writer): int =
  result = w.origin
  w.origin = w.outBytes.len

## Restore the origin beginPackedElement displaced.
proc finishPackedElement*(w: Writer, saved: int) =
  w.origin = saved

# -- fixed -----------------------------------------------------------------

## `fixed<digits, scale>` as packed BCD, [CDR] 9.3.2.8 — the only wire
## definition an IDL fixed has. `count` is "declared", "value-precision"
## or "value-string-digits".
proc writeFixed*(w: Writer, v: string, digits: int, scale: int, count: string) =
  let nibbles = fixedNibbles(v, digits, scale, count, w.outBytes.len)
  var i = 0
  while i < nibbles.len:
    w.outBytes.add(uint8((nibbles[i] shl 4) or nibbles[i + 1]))
    i += 2

# -- the bit-bounded kinds -------------------------------------------------

## An enumerator, as the integer its bit bound calls for. The holder is
## signed — 7.4.3.5.3 rule (5) — and what fits is everything
## expressible in the bound's bits: -2^(n-1) ..= 2^n - 1.
proc writeEnum*(w: Writer, bitBound: int, value: int64) =
  if bitBound == 0 or bitBound > 32:
    raiseOutOfRange(w.outBytes.len, "an @bit_bound between 1 and 32, not " & $bitBound)
  if value < -(1'i64 shl (bitBound - 1)) or value > (1'i64 shl bitBound) - 1:
    raiseOutOfRange(w.outBytes.len, "an enumerator a " & $bitBound & "-bit holder can hold")
  let pattern = cast[uint64](value) and ((1'u64 shl bitBound) - 1)
  if bitBound <= 8:
    w.writeU8(uint8(pattern))
  elif bitBound <= 16:
    w.writeU16(uint16(pattern))
  else:
    w.writeU32(uint32(pattern))

## A bitmask, as the unsigned integer Table 33 gives its bound.
proc writeBitmask*(w: Writer, bitBound: int, value: uint64) =
  if bitBound == 0 or bitBound > 64:
    raiseOutOfRange(w.outBytes.len, "an @bit_bound between 1 and 64, not " & $bitBound)
  if bitBound < 64 and value >= (1'u64 shl bitBound):
    raiseOutOfRange(w.outBytes.len, "a bitmask below 2^" & $bitBound)
  if bitBound <= 8:
    w.writeU8(uint8(value))
  elif bitBound <= 16:
    w.writeU16(uint16(value))
  elif bitBound <= 32:
    w.writeU32(uint32(value))
  else:
    w.writeU64(value)

## One bitfield, shifted into the accumulator a bitset packs into. A
## value wider than the field is refused rather than masked.
proc bitfield*(w: Writer, value: uint64, width: int, position: int): uint64 =
  if width <= 0 or width > 64 or position < 0 or position + width > 64:
    raiseOutOfRange(w.outBytes.len, "a bitfield of " & $width & " bits at " &
      $position & ", inside 64")
  if width < 64 and value > (1'u64 shl width) - 1:
    raiseOutOfRange(w.outBytes.len, "a value inside " & $width & " bits")
  value shl position

## The Cyclone union overlay: the low `octets` of `pattern`, written as
## one unsigned integer of that width in the stream's byte order. A
## discriminator carrying a label the `union-labels` dialect axis says
## selects nothing makes the peer fall through to `default:` and
## serialise its one C union storage as that arm — the selected
## member's octets, reinterpreted. The `bitPattern*` procedures produce
## the pattern.
proc writeUnionOverlay*(w: Writer, pattern: uint64, octets: int) =
  case octets
  of 1: w.writeU8(uint8(pattern and 0xFF'u64))
  of 2: w.writeU16(uint16(pattern and 0xFFFF'u64))
  of 4: w.writeU32(uint32(pattern and 0xFFFF_FFFF'u64))
  of 8: w.writeU64(pattern)
  else:
    raiseOutOfRange(w.outBytes.len, "a union arm of 1, 2, 4 or 8 octets, not " & $octets)

## A bitset, as the anchor integer its bitfields pack into; the
## anchor's width comes from the layout planner.
proc writeBitset*(w: Writer, anchor: int, value: uint64) =
  case anchor
  of 1: w.writeU8(uint8(value and 0xFF'u64))
  of 2: w.writeU16(uint16(value and 0xFFFF'u64))
  of 4: w.writeU32(uint32(value and 0xFFFF_FFFF'u64))
  of 8: w.writeU64(value)
  else:
    raiseOutOfRange(w.outBytes.len, "a bitset anchor of 1, 2, 4 or 8 bytes, not " & $anchor)

# -- framing ---------------------------------------------------------------

proc patch(w: Writer, at: int, value: uint64, width: int) =
  if width < 8 and value >= (1'u64 shl (width * 8)):
    raiseOutOfRange(at, "a " & $width & "-byte length")
  if w.endianness == EndianLittle:
    for i in 0 ..< width:
      w.outBytes[at + i] = uint8((value shr (8 * i)) and 0xFF'u64)
  else:
    for i in 0 ..< width:
      w.outBytes[at + width - 1 - i] = uint8((value shr (8 * i)) and 0xFF'u64)

## A DHEADER: a four-byte length placeholder, patched when the members
## are done.
proc writeDheader*(w: Writer): DHeader =
  w.alignTo(4)
  let at = w.outBytes.len
  for _ in 0 ..< 4:
    w.outBytes.add(0'u8)
  DHeader(at: at)

## Patch a DHEADER with the length of what followed it; a header from
## noDheader is not there and is ignored.
proc finishDheader*(w: Writer, header: DHeader) =
  if header.at < 0:
    return
  w.patch(header.at, uint64(w.outBytes.len - header.at - 4), 4)

## An XCDR2 EMHEADER: id, length code and must-understand in one 32-bit
## word. 7.4.3.4.2. LC4 reserves a NEXTINT; LC5 to LC7 reuse the
## member's own leading word.
proc writeEmheader*(w: Writer, memberId: int, lengthCode: int, mustUnderstand: bool): EmHeader =
  ## 7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and 7.4.3.4.2
  ## leaves them the low 28 bits of the word. Masking a larger one writes
  ## a well-formed header for `id and 0x0FFFFFFF`: the peer reads a
  ## member the caller did not name, and nothing at either end says so.
  if memberId < 0 or uint32(memberId) > eMemberIdMask:
    raiseOutOfRange(w.outBytes.len,
      "a member id inside the 28 bits of an EMHEADER, not " & $memberId)
  w.alignTo(4)
  var word = (uint32(lengthCode and 0x7) shl 28) or (uint32(memberId) and eMemberIdMask)
  if mustUnderstand:
    word = word or 0x8000_0000'u32
  w.writeU32(word)
  let wordAt = w.outBytes.len - 4
  if lengthCode == 4:
    let at = w.outBytes.len
    for _ in 0 ..< 4:
      w.outBytes.add(0'u8)
    return EmHeader(nextInt: at, word: wordAt, lengthCode: lengthCode)
  EmHeader(nextInt: -1, word: wordAt, lengthCode: lengthCode)

proc readPatched(w: Writer, at: int): uint32 =
  if w.endianness == EndianLittle:
    for i in countdown(3, 0):
      result = (result shl 8) or uint32(w.outBytes[at + i])
  else:
    for i in 0 ..< 4:
      result = (result shl 8) or uint32(w.outBytes[at + i])

proc shortenEmheader(w: Writer, header: EmHeader, length: int) =
  let code: uint32 =
    case length
    of 1: 0'u32
    of 2: 1'u32
    of 4: 2'u32
    else: 3'u32
  w.outBytes.delete(header.nextInt .. header.nextInt + 3)
  let previous = w.readPatched(header.word)
  w.patch(header.word, uint64((previous and not (0x7'u32 shl 28)) or (code shl 28)), 4)

## Patch an EMHEADER's NEXTINT; under `shortLengthCodes` an LC4 header
## whose member is 1, 2, 4 or 8 bytes loses its NEXTINT and states the
## length in the code.
proc finishEmheader*(w: Writer, header: EmHeader) =
  if header.nextInt < 0:
    ## Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
    ## octets, 7.4.3.4.2. A value of another size misdeclares it and the
    ## reader's skip lands inside data, so the mismatch is refused here
    ## rather than written. Codes 5 to 7 declare nothing of their own:
    ## the member's leading length word is the NEXTINT.
    if header.lengthCode >= 0 and header.lengthCode <= 3 and
        header.word + 4 <= w.outBytes.len:
      let declared = 1 shl header.lengthCode
      let body = w.outBytes.len - header.word - 4
      if body != declared:
        raiseOutOfRange(w.outBytes.len, "a member of " & $declared &
          " octets under length code " & $header.lengthCode & ", not " & $body)
    return
  let length = w.outBytes.len - header.nextInt - 4
  if w.shortLengthCodes and (length == 1 or length == 2 or length == 4 or length == 8):
    w.shortenEmheader(header, length)
    return
  w.patch(header.nextInt, uint64(length), 4)

proc beginParameterValue(w: Writer): int =
  result = w.origin
  w.origin = w.outBytes.len

## A PL_CDR1 parameter header, short form: [id:u16][length:u16].
proc writePid*(w: Writer, pid: int, role: ParameterRole, mustUnderstand: bool): Pid =
  ## 7.4.1.2.2 gives the short form the ids 0 to `highestShortId`
  ## inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
  ## itself. A short header with a reserved id collides with the
  ## protocol's own parameters, so such an id takes the extended form or
  ## none. This is the write side of the constant the reader has always
  ## consulted.
  if pid < 0 or pid > w.plCdr1.highestShortId:
    raiseOutOfRange(w.outBytes.len,
      "a parameter id the short header carries, not " & $pid &
      " — the extended header takes it")
  w.alignTo(4)
  var pid = pid
  if mustUnderstand and w.plCdr1.memberMustUnderstand:
    pid = pid or PidMustUnderstand
  pid = pid or pidFlagsAt(w.plCdr1.pidFlags, "member")
  let bare = pid and not (PidMustUnderstand or PidImplExtension)
  w.writeU16(uint16(pid and 0xFFFF))
  let at = w.outBytes.len
  w.outBytes.add(0'u8)
  w.outBytes.add(0'u8)
  Pid(at: at, extended: false, savedOrigin: w.beginParameterValue(), role: role,
      pid: bare, mustUnderstand: mustUnderstand)

## A PL_CDR1 parameter header, extended form:
## [PID_EXTENDED][8][id:u32][length:u32]. 7.4.1.2. The member's
## must-understand is FLAG_2 of the eMemberHeader.
proc writePidExtended*(w: Writer, pid: int, role: ParameterRole, mustUnderstand: bool): Pid =
  ## The same 28 bits as an EMHEADER's, here in the eMemberHeader of
  ## 7.4.1.2.1. Masking aliases the parameter onto another member exactly
  ## as it does there.
  if pid < 0 or uint32(pid) > eMemberIdMask:
    raiseOutOfRange(w.outBytes.len,
      "a member id inside the 28 bits of an eMemberHeader, not " & $pid)
  w.alignTo(4)
  let marker = PidExtended or pidFlagsAt(w.plCdr1.pidFlags, "extended-marker")
  w.writeU16(uint16(marker and 0xFFFF))
  w.writeU16(8'u16)
  var memberHeader = uint32(pid) and eMemberIdMask
  if mustUnderstand and w.plCdr1.memberMustUnderstand:
    memberHeader = memberHeader or eFlagMustUnderstand
  w.writeU32(memberHeader)
  let at = w.outBytes.len
  for _ in 0 ..< 4:
    w.outBytes.add(0'u8)
  Pid(at: at, extended: true, savedOrigin: w.beginParameterValue(), role: role,
      pid: 0, mustUnderstand: false)

proc promotePid(w: Writer, header: Pid, length: int) =
  let start = header.at - 2
  var padding = newSeq[uint8](8)
  w.outBytes.insert(padding, header.at + 2)
  let marker = PidExtended or pidFlagsAt(w.plCdr1.pidFlags, "extended-marker")
  w.patch(start, uint64(marker), 2)
  w.patch(start + 2, 8'u64, 2)
  var memberHeader = uint32(header.pid) and eMemberIdMask
  if header.mustUnderstand and w.plCdr1.memberMustUnderstand:
    memberHeader = memberHeader or eFlagMustUnderstand
  w.patch(start + 4, uint64(memberHeader), 4)
  w.patch(start + 8, uint64(length), 4)

## Patch a parameter header with its value's length; the padding to
## four goes outside the declared length or inside it, by framing.
proc finishPid*(w: Writer, header: Pid) =
  let width = if header.extended: 4 else: 2
  var length = w.outBytes.len - header.at - width
  if w.plCdr1.padsDeclaredLength(header.role):
    let past = (w.outBytes.len - header.savedOrigin) mod 4
    if past != 0:
      length += 4 - past
  if not header.extended and length > PidShortMaxLen:
    w.promotePid(header, length)
  else:
    w.patch(header.at, uint64(length), width)
  w.origin = header.savedOrigin
  # Under Fast DDS's framing the parameter writes no padding of its own
  # — the next header aligns itself, and a final parameter ends the
  # stream where its value does.
  if w.plCdr1.padAfterValue:
    w.alignTo(4)

## The PID_LIST_END sentinel a PL_CDR1 parameter list ends with; its
## flags come from the framing asked at the "sentinel" position.
proc writeSentinel*(w: Writer) =
  w.alignTo(4)
  let sentinel = PidListEnd or pidFlagsAt(w.plCdr1.pidFlags, "sentinel")
  w.writeU16(uint16(sentinel and 0xFFFF))
  w.writeU16(0'u16)

# -- the reader ------------------------------------------------------------

type
  ## Reads one sample's bytes: the mirror of the writer clause for
  ## clause. A malformed stream raises; a refusal about the type sets
  ## the status and the generated decoder walks on.
  Reader* = ref object
    data: seq[uint8]
    at: int
    endianness: Endianness
    maxAlign: int
    origin: int
    plCdr1: PlCdr1Framing
    status: Status

proc newReader*(data: seq[uint8], endianness: Endianness, maxAlign: int,
                plCdr1: PlCdr1Framing): Reader =
  Reader(data: data, endianness: endianness, maxAlign: max(maxAlign, 1), plCdr1: plCdr1)

# -- the status a generated decoder reads ----------------------------------

## How the read is going.
proc readerStatus*(r: Reader): Status =
  r.status

## Refuse the stream from outside the runtime; the first refusal
## sticks, and StatusOk refuses nothing.
proc readerRefuse*(r: Reader, refusal: Status) =
  if refusal != StatusOk and r.status == StatusOk:
    r.status = refusal

## Take back a StatusDiscarded, and only that one: a truncated stream
## is not a narrowing.
proc readerClearDiscard*(r: Reader): bool =
  if r.status == StatusDiscarded:
    r.status = StatusOk
    return true
  false

## Where the reader is.
proc readerPosition*(r: Reader): int =
  r.at

## How many bytes are left.
proc readerRemaining*(r: Reader): int =
  if r.at > r.data.len:
    return 0
  r.data.len - r.at

## Move the cursor forward to `target`, never backwards: a framing word
## pointing behind the cursor is a refusal about the stream's shape,
## and past the end raises.
proc readerSkipTo*(r: Reader, target: int) =
  if r.status != StatusOk:
    return
  if target < r.at:
    r.readerRefuse(StatusOutOfRange)
    return
  if target > r.data.len:
    raiseTruncated(r.at, target - r.at, r.readerRemaining())
  r.at = target

## The cursor, the origin and the status, to be put back later.
proc readerSave*(r: Reader): ReaderMark =
  ReaderMark(at: r.at, origin: r.origin, status: r.status)

## Put back what readerSave took.
proc readerRestore*(r: Reader, mark: ReaderMark) =
  r.at = mark.at
  r.origin = mark.origin
  r.status = mark.status

## Move the alignment origin to the current position and answer the
## origin displaced, for readerFinishPackedElement. The read-side half of
## beginPackedElement.
proc readerBeginPackedElement*(r: Reader): int =
  result = r.origin
  r.origin = r.at

## Restore the origin readerBeginPackedElement displaced.
proc readerFinishPackedElement*(r: Reader, saved: int) =
  r.origin = saved

# -- the cursor ------------------------------------------------------------

proc alignTo(r: Reader, alignment: int) =
  var alignment = alignment
  if alignment > r.maxAlign:
    alignment = r.maxAlign
  if alignment <= 1:
    return
  let offset = r.at - r.origin
  r.at += (alignment - offset mod alignment) mod alignment

proc take(r: Reader, count: int): int =
  ## Answers the start of the taken run; the caller indexes r.data.
  let endAt = r.at + count
  if endAt > r.data.len:
    raiseTruncated(r.at, count, r.readerRemaining())
  result = r.at
  r.at = endAt

proc readUnsigned(r: Reader, width: int): uint64 =
  r.alignTo(width)
  let start = r.take(width)
  if r.endianness == EndianLittle:
    for i in countdown(width - 1, 0):
      result = (result shl 8) or uint64(r.data[start + i])
  else:
    for i in 0 ..< width:
      result = (result shl 8) or uint64(r.data[start + i])

# -- primitives ------------------------------------------------------------

## `boolean`; a byte that is neither 0 nor 1 is refused rather than
## coerced.
proc readBool*(r: Reader): bool =
  let position = r.at
  let value = r.data[r.take(1)]
  if value == 0'u8:
    return false
  if value == 1'u8:
    return true
  raiseOutOfRange(position, "a boolean of 0 or 1")

## `octet` and `uint8`.
proc readU8*(r: Reader): uint8 =
  r.data[r.take(1)]

## `int8`.
proc readI8*(r: Reader): int8 =
  cast[int8](r.data[r.take(1)])

## `char`: one octet.
proc readChar*(r: Reader): uint8 =
  r.data[r.take(1)]

## `short`.
proc readI16*(r: Reader): int16 =
  cast[int16](uint16(r.readUnsigned(2)))

## `unsigned short`.
proc readU16*(r: Reader): uint16 =
  uint16(r.readUnsigned(2))

## `long`.
proc readI32*(r: Reader): int32 =
  cast[int32](uint32(r.readUnsigned(4)))

## `unsigned long`.
proc readU32*(r: Reader): uint32 =
  uint32(r.readUnsigned(4))

## `long long`.
proc readI64*(r: Reader): int64 =
  cast[int64](r.readUnsigned(8))

## `unsigned long long`.
proc readU64*(r: Reader): uint64 =
  r.readUnsigned(8)

## `float`.
proc readF32*(r: Reader): float32 =
  cast[float32](uint32(r.readUnsigned(4)))

## `double`.
proc readF64*(r: Reader): float64 =
  cast[float64](r.readUnsigned(8))

## `long double`: all sixteen bytes, as the exact decimal the generated
## member holds.
proc readF128*(r: Reader): string =
  r.alignTo(8)
  let start = r.take(16)
  var bytes: array[16, uint8]
  if r.endianness == EndianLittle:
    for i in 0 ..< 16:
      bytes[i] = r.data[start + i]
  else:
    for i in 0 ..< 16:
      bytes[i] = r.data[start + 15 - i]
  f128ToExactDecimal(bytes)

## `wchar`, at `width` octets and in `framing`'s shape. The four-byte
## form zero-extends one UTF-16 code unit; a value that uses the high
## bits, and a lone surrogate, are refused.
proc readWchar*(r: Reader, width: int, framing: string, order: string): uint16 =
  let position = r.at
  if width != 2 and width != 4:
    raiseOutOfRange(position, "a wchar width of 2 or 4")
  var value: uint32
  if framing == "giop-length-prefixed":
    let declared = r.readU8()
    if int(declared) != width:
      raiseOutOfRange(position, "a GIOP 1.2 wchar of " & $width &
        " octets, the configured transmission code set, and not " & $declared)
    let big = order != "stream" or r.endianness != EndianLittle
    let start = r.take(width)
    value = 0
    if big:
      for i in 0 ..< width:
        value = (value shl 8) or uint32(r.data[start + i])
    else:
      for i in countdown(width - 1, 0):
        value = (value shl 8) or uint32(r.data[start + i])
  elif framing != "fixed":
    raiseOutOfRange(position, "a wchar framing, not `" & framing & "`")
  elif width == 4:
    value = r.readU32()
    if value > 0xFFFF'u32:
      raiseOutOfRange(position, "a four-byte wchar holding one UTF-16 code unit")
  else:
    value = uint32(r.readU16())
  if value >= 0xD800'u32 and value <= 0xDFFF'u32:
    raiseOutOfRange(position, "a Unicode code point")
  uint16(value)

# -- strings ---------------------------------------------------------------

## A string: a length including the terminator, the bytes, the NUL.
proc readString*(r: Reader): string =
  let position = r.at
  let length = int(r.readU32())
  if length == 0 or length > r.readerRemaining() + 1:
    raiseImpossible(position, length, r.readerRemaining())
  let start = r.take(length - 1)
  result = newString(length - 1)
  for i in 0 ..< length - 1:
    result[i] = char(r.data[start + i])
  if validateUtf8(result) != -1:
    raiseNotUtf8(position)
  discard r.readU8()

## readString under the name the shared walk calls it by: a Nim string
## owns its storage, so the two names are one procedure.
proc readStringAlloc*(r: Reader): string =
  r.readString()

## A 16-bit value little-endian whatever the stream's order is.
proc readU16Little*(r: Reader): uint16 =
  r.alignTo(2)
  let start = r.take(2)
  uint16(r.data[start]) or (uint16(r.data[start + 1]) shl 8)

## A 16-bit value big-endian whatever the stream's order is.
proc readU16Big*(r: Reader): uint16 =
  r.alignTo(2)
  let start = r.take(2)
  (uint16(r.data[start]) shl 8) or uint16(r.data[start + 1])

## A 32-bit value little-endian whatever the stream's order is.
proc readU32Little*(r: Reader): uint32 =
  r.alignTo(4)
  let start = r.take(4)
  uint32(r.data[start]) or (uint32(r.data[start + 1]) shl 8) or
    (uint32(r.data[start + 2]) shl 16) or (uint32(r.data[start + 3]) shl 24)

## A 32-bit value big-endian whatever the stream's order is.
proc readU32Big*(r: Reader): uint32 =
  r.alignTo(4)
  let start = r.take(4)
  (uint32(r.data[start]) shl 24) or (uint32(r.data[start + 1]) shl 16) or
    (uint32(r.data[start + 2]) shl 8) or uint32(r.data[start + 3])

proc stripByteOrderMark(units: seq[uint16]): seq[uint16] =
  if units.len == 0:
    return units
  if units[0] == byteOrderMark:
    return units[1 .. ^1]
  var rest = units
  if units[0] == swappedByteOrderMark:
    rest = units[1 .. ^1]
  for unit in rest:
    result.add((unit shl 8) or (unit shr 8))

proc wstringUnits(r: Reader, position, count, width: int, order: string): seq[uint16] =
  if count * width > r.readerRemaining():
    raiseImpossible(position, count * width, r.readerRemaining())
  let little = order == "byte-order-mark" or order == "host-unswapped"
  for _ in 0 ..< count:
    if width == 4:
      var wide: uint32
      if order == "big-endian-always":
        wide = r.readU32Big()
      elif little:
        wide = r.readU32Little()
      else:
        wide = r.readU32()
      if wide > 0xFFFF'u32:
        raiseOutOfRange(position, "a four-byte wstring character holding one UTF-16 unit")
      result.add(uint16(wide))
      continue
    if order == "big-endian-always":
      result.add(r.readU16Big())
    elif little:
      result.add(r.readU16Little())
    else:
      result.add(r.readU16())

## A wstring, in the form `policy` and `order` name, answered as UTF-8
## — which is what the generated member holds. The form is never
## inferred from the stream.
proc readWstringUtf8*(r: Reader, policy: string, order: string): string =
  let position = r.at
  let declared = int(r.readU32())
  if not wstringOrderKnown(order):
    raiseOutOfRange(position, "a wstring unit order, not `" & order & "`")
  var cell: WstringCell
  if not wstringCellOf(policy, cell):
    raiseOutOfRange(position, "a wstring length policy, not `" & policy & "`")
  var count = declared
  if not cell.countsCharacters:
    if declared mod cell.width != 0:
      raiseImpossible(position, declared, r.readerRemaining())
    count = declared div cell.width
  var units = r.wstringUnits(position, count, cell.width, order)
  if cell.terminated:
    if declared < 1:
      raiseImpossible(position, declared, r.readerRemaining())
    let last = units[^1]
    units = units[0 ..< units.len - 1]
    if last != 0'u16:
      raiseOutOfRange(position, "a NUL character ending the wstring, as its count includes one")
    for unit in units:
      if unit == 0'u16:
        raiseOutOfRange(position, "no NUL inside a wstring a C peer would stop at")
  if order == "byte-order-mark":
    units = stripByteOrderMark(units)
  var index = 0
  while index < units.len:
    let unit = units[index]
    if unit >= 0xD800'u16 and unit <= 0xDBFF'u16 and index + 1 < units.len:
      let low = units[index + 1]
      if low >= 0xDC00'u16 and low <= 0xDFFF'u16:
        let point = 0x10000 + ((int(unit) - 0xD800) shl 10) + (int(low) - 0xDC00)
        result.add($Rune(point))
        index += 2
        continue
    if unit >= 0xD800'u16 and unit <= 0xDFFF'u16:
      raiseOutOfRange(position, "a UTF-16 sequence with no lone surrogate")
    result.add($Rune(int(unit)))
    inc index

## The inverse of writeWstringUtf8Octets: the declared length is the
## UTF-8 octet count, the terminator the policy asks for is the last of
## those octets, and a count the stream cannot hold, a missing or an
## interior NUL, or an ill-formed UTF-8 sequence is refused. See
## `WstringCodeUnits::Utf8`.
proc readWstringUtf8Octets*(r: Reader, policy: string): string =
  let position = r.at
  let declared = int(r.readU32())
  var cell: WstringCell
  if not wstringCellOf(policy, cell):
    raiseOutOfRange(position, "a wstring length policy, not `" & policy & "`")
  if declared > r.readerRemaining():
    raiseImpossible(position, declared, r.readerRemaining())
  let start = r.take(declared)
  var length = declared
  if cell.terminated:
    if declared < 1 or r.data[start + declared - 1] != 0'u8:
      raiseOutOfRange(position, "a NUL octet ending the wstring, as its count includes one")
    length = declared - 1
    # A NUL inside the text is the narrow string's problem in wide form:
    # every C peer stops at it.
    for i in 0 ..< length:
      if r.data[start + i] == 0'u8:
        raiseOutOfRange(position, "no NUL inside a wstring a C peer would stop at")
  result = newString(length)
  for i in 0 ..< length:
    result[i] = char(r.data[start + i])
  if validateUtf8(result) != -1:
    raiseNotUtf8(position)

## `fixed<digits, scale>`, as the decimal string the writer took: the
## leading zeros go, and a negative zero loses its sign. Under
## "value-precision" and "value-string-digits" the field ends at the first octet whose low nibble
## is a sign, and one that never presents a sign within the sixteen
## octets of the widest fixed IDL admits is refused.
proc readFixed*(r: Reader, digits: int, scale: int, count: string): string =
  if count != "declared" and count != "value-precision" and
      count != "value-string-digits":
    raiseOutOfRange(r.at, "a fixed digit count, not `" & count & "`")
  # The same bound as the writer's, on this side too: OMG IDL 4.2 rule
  # (42) caps the BCD rule at 31 digits, and a reader asked for a wider
  # field would take octets no declaration can name.
  if digits <= 0 or digits > 31 or scale > digits:
    raiseOutOfRange(r.at, "a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<" &
      $digits & ", " & $scale & ">")
  var nibbles: seq[int] = @[]
  var sign: int
  if count == "value-precision" or count == "value-string-digits":
    while true:
      if nibbles.len >= 32:
        raiseOutOfRange(r.at, "a sign nibble within sixteen octets")
      let b = int(r.data[r.take(1)])
      nibbles.add(b shr 4)
      nibbles.add(b and 0x0F)
      if (b and 0x0F) >= 0x0A:
        break
    sign = nibbles[^1]
    nibbles = nibbles[0 ..< nibbles.len - 1]
  else:
    let width = (digits + 2) div 2
    let start = r.take(width)
    for i in 0 ..< width:
      let b = int(r.data[start + i])
      nibbles.add(b shr 4)
      nibbles.add(b and 0x0F)
    sign = nibbles[^1]
    nibbles = nibbles[0 ..< nibbles.len - 1]
    if (digits + 1) mod 2 == 1:
      nibbles = nibbles[1 .. ^1]
  for nibble in nibbles:
    if nibble > 9:
      raiseOutOfRange(r.at, "BCD digits")
  var text = ""
  for nibble in nibbles:
    text.add(char(ord('0') + nibble))
  while text.len < scale + 1:
    text = "0" & text
  let split = text.len - scale
  var whole = text[0 ..< split]
  while whole.len > 1 and whole[0] == '0':
    whole = whole[1 .. ^1]
  let fraction = text[split .. ^1]
  result = whole
  if scale > 0:
    result = result & "." & fraction
  var zero = whole == "0"
  for digit in fraction:
    if digit != '0':
      zero = false
  if sign == 0x0D and not zero:
    return "-" & result

# -- the trims a bound reaches for -----------------------------------------

## The trim, reached as a method because the shared walk emits it as a
## call on the reader.
proc trimUtf8*(r: Reader, text: string, octets: int): string =
  trimUtf8(text, octets)

## The wide half.
proc trimUtf16*(r: Reader, text: string, units: int): string =
  trimUtf16(text, units)

# -- the bit-bounded kinds -------------------------------------------------

## An enumerator's holder, as the signed integer 7.4.3.5.3 makes it —
## the holder's width and not the bound's.
proc readEnum*(r: Reader, bitBound: int): int64 =
  if bitBound == 0 or bitBound > 32:
    raiseOutOfRange(r.at, "an @bit_bound between 1 and 32, not " & $bitBound)
  if bitBound <= 8:
    return int64(cast[int8](r.readU8()))
  if bitBound <= 16:
    return int64(cast[int16](r.readU16()))
  int64(cast[int32](r.readU32()))

## A bitmask's holder. Table 33 of 7.4.1.1.3.2.
proc readBitmask*(r: Reader, bitBound: int): uint64 =
  if bitBound == 0 or bitBound > 64:
    raiseOutOfRange(r.at, "an @bit_bound between 1 and 64, not " & $bitBound)
  if bitBound <= 8:
    return uint64(r.readU8())
  if bitBound <= 16:
    return uint64(r.readU16())
  if bitBound <= 32:
    return uint64(r.readU32())
  r.readU64()

## A bitset's anchor integer, of the width the planner sized.
proc readBitset*(r: Reader, anchor: int): uint64 =
  case anchor
  of 1: uint64(r.readU8())
  of 2: uint64(r.readU16())
  of 4: uint64(r.readU32())
  of 8: r.readU64()
  else:
    raiseOutOfRange(r.at, "a bitset anchor of 1, 2, 4 or 8 bytes, not " & $anchor)

# -- framing ---------------------------------------------------------------

## A DHEADER: the length the members that follow occupy, refusing one
## the stream cannot hold.
proc readDheader*(r: Reader): uint32 =
  let position = r.at
  let length = r.readU32()
  if int(length) > r.readerRemaining():
    raiseImpossible(position, int(length), r.readerRemaining())
  length

proc emExtent(header: EmHeaderRead): int =
  if header.lengthCode <= 3:
    return 1 shl header.lengthCode
  if header.nextInt < 0:
    return 0
  if header.lengthCode == 4:
    return header.nextInt
  var scale = 1
  if header.lengthCode == 6:
    scale = 4
  elif header.lengthCode == 7:
    scale = 8
  4 + header.nextInt * scale

## An EMHEADER, unpacked. Codes 5 to 7 report the NEXTINT they read,
## but the reader is put back before it: that word is the member's own
## leading length. 7.4.3.4.2.
proc readEmheader*(r: Reader): EmHeaderRead =
  let word = r.readU32()
  result.memberId = int(word and eMemberIdMask)
  result.lengthCode = int((word shr 28) and 0x7'u32)
  result.mustUnderstand = (word shr 31) != 0'u32
  result.nextInt = -1
  if result.lengthCode == 4:
    result.nextInt = int(r.readU32())
  elif result.lengthCode >= 5:
    let position = r.at
    result.nextInt = int(r.readU32())
    r.at = position
  result.extent = emExtent(result)

proc readPidHeader(r: Reader, role: ParameterRole): PidRead =
  r.alignTo(4)
  let pid = int(r.readU16())
  let length = int(r.readU16())
  if (pid and pidIdMask) == (PidExtended and pidIdMask):
    # The four flag bits are not part of the id: 7.4.1.2.1 splits the
    # eMemberHeader into FLAG_1 to FLAG_4 and a 28-bit member id, and
    # FLAG_2 is the member's must-understand.
    let memberHeader = r.readU32()
    let wideLength = int(r.readU32())
    result.pid = int(memberHeader and eMemberIdMask)
    result.length = wideLength
    result.endOfList = false
    result.mustUnderstand = (memberHeader and eFlagMustUnderstand) != 0'u32
    result.implExtension = (memberHeader and eFlagImplExtension) != 0'u32
    result.role = role
    return
  let identifier = pid and pidIdMask
  let endOfList = identifier == (PidListEnd and pidIdMask)
  result.pid = identifier
  result.length = length
  result.endOfList = endOfList
  result.mustUnderstand = (pid and PidMustUnderstand) != 0
  # Under a framing that puts FLAG_IMPL_EXTENSION on every member id
  # the bit carries no information and is reported absent.
  result.implExtension = (pid and PidImplExtension) != 0 and
    (pidFlagsAt(r.plCdr1.pidFlags, "member") and PidImplExtension) == 0
  result.reserved = not endOfList and identifier > r.plCdr1.highestShortId
  result.ignore = identifier == PidIgnore
  result.role = role

## A parameter header, and the value block it opens: 7.4.1.2 makes a
## parameter's value its own stream block, so a member's internal
## alignment is counted from the first octet after the header. The
## sentinel opens no block — nothing follows it, and the origin it
## would have to give back stays where it is.
proc readPidAs*(r: Reader, role: ParameterRole): PidRead =
  result = r.readPidHeader(role)
  result.savedOrigin = r.origin
  result.valueAt = r.at
  if result.endOfList:
    return
  if result.length > r.readerRemaining():
    r.readerRefuse(StatusImpossibleLength)
    return
  r.origin = r.at

## The member role's form, for the walk's bare `read_pid`.
proc readPid*(r: Reader): PidRead =
  r.readPidAs(ParameterMember)

## End a parameter's value and step to the next header.
##
## The declared length is measured against the value's own start and
## checked against the configured framing rather than believed: a
## length belonging to the other convention is a stream this reader was
## pointed at with the wrong framing, and reading on would put every
## later member at an offset no writer produced. Both readings of the
## same value are accepted where they coincide, which is every value
## already a multiple of four. runtime/csharp/src/Reader.cs carries the
## same logic.
##
## The trailing alignment is the framing's: under a padding that
## follows the value the reader steps it, and under Fast DDS's the next
## header aligns itself — a final parameter ends where its value does.
proc finishPidRead*(r: Reader, header: PidRead) =
  r.origin = header.savedOrigin
  if r.status != StatusOk or header.endOfList:
    return
  let used = r.at - header.valueAt
  let declared = header.length
  let padded = (used + 3) and not 3
  let pads = r.plCdr1.padsDeclaredLength(header.role)
  if declared != (if pads: padded else: used) and declared != used and declared != padded:
    r.readerRefuse(StatusOutOfRange)
  if r.plCdr1.padAfterValue:
    r.alignTo(4)

when isMainModule:
  # The three axes Dialect::Hdds selects. Expected octets from the
  # recorded HDDS columns:
  # tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin
  # (`04 00 00 00 61 62 63 00` for "abc"),
  # tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin
  # (`01 00 00 00 02 00 03 00 00 00 04 00 77`) and
  # tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin
  # (the first Cell of `one`, `06 00 00 00 79 29 ed ff fe ff`).
  let framing = PlCdr1Framing(
    pidFlags: "sentinel-must-understand",
    highestShortId: 0x3EFF,
    padDeclaredLength: false,
    padDeclaredOptionalLength: false,
    memberMustUnderstand: true,
    padAfterValue: true)

  block aUtf8WstringCountsItsOctets:
    var w = newWriter(EndianLittle, 4, framing, false)
    w.writeWstringUtf8Octets("abc", "octets/terminated/2")
    doAssert w.finish() == @[4'u8, 0, 0, 0, 0x61, 0x62, 0x63, 0]
    var r = newReader(w.finish(), EndianLittle, 4, framing)
    doAssert r.readWstringUtf8Octets("octets/terminated/2") == "abc"

  block aBoundedUtf8WstringBoundsItsUnits:
    var w = newWriter(EndianLittle, 4, framing, false)
    w.writeWstringUtf8OctetsBounded("ሴäZ", 3, "octets/terminated/2")
    doAssert w.finish() == @[7'u8, 0, 0, 0, 0xe1, 0x88, 0xb4, 0xc3, 0xa4, 0x5a, 0]

  block aPackedElementCountsFromItsOwnStart:
    var w = newWriter(EndianLittle, 4, framing, false)
    var saved = w.beginPackedElement()
    w.writeI32(1)
    w.writeI16(2)
    w.finishPackedElement(saved)
    saved = w.beginPackedElement()
    w.writeI32(3)
    w.writeI16(4)
    w.finishPackedElement(saved)
    w.writeU8(119)
    doAssert w.finish() == @[1'u8, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119]
    var r = newReader(w.finish(), EndianLittle, 4, framing)
    saved = r.readerBeginPackedElement()
    doAssert r.readI32() == 1
    doAssert r.readI16() == 2
    r.readerFinishPackedElement(saved)
    saved = r.readerBeginPackedElement()
    doAssert r.readI32() == 3
    doAssert r.readI16() == 4
    r.readerFinishPackedElement(saved)
    doAssert r.readU8() == 119

  block aSequenceElementLengthFramesTheElement:
    var w = newWriter(EndianLittle, 4, framing, false)
    let d = w.writeDheader()
    w.writeI32(-1234567)
    w.writeI16(-2)
    w.finishDheader(d)
    let got = w.finish()
    doAssert got == @[6'u8, 0, 0, 0, 0x79, 0x29, 0xed, 0xff, 0xfe, 0xff]
    var r = newReader(got, EndianLittle, 4, framing)
    let declared = int(r.readU32())
    let endAt = r.readerPosition() + declared
    doAssert r.readI32() == -1234567
    doAssert r.readI16() == -2
    doAssert r.readerPosition() == endAt
