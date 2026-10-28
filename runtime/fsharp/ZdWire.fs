// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The F# reading of DDS-XTypes 1.3 §7.4: XCDR1 and XCDR2, both byte orders,
// PL_CDR1 parameter framing, DHEADER/EMHEADER for @mutable, and the
// try-construct discard/trim/use-default behaviour of §7.2.2.7. The
// counterpart of runtime/zig/zdwire.zig and runtime/c/zdwire.h, a further
// implementation of the clause and not a port of either: where this and the
// Rust runtime disagree on an octet, one of them has misread the
// specification.
//
// This module's bit-level mechanics — alignment, byte-order handling, the
// packed-BCD `fixed<P,S>` encoding, PL_CDR1 parameter headers — follow
// endpoints/fsharp/zerodds.fs, the hand-written endpoint SDK: the same
// octets, the same algorithms, extended here with what a generated codec
// needs and a hand-written endpoint does not — try-construct semantics,
// DHEADER/EMHEADER for a `@mutable` aggregate, and a framing policy the
// caller configures rather than one this module fixes.
//
// ## Errors are exceptions, and discard is not one of them
//
// F# has no error union the way Zig's `WireError!T` is one, and no
// out-parameter the way C's `zd_status_t` return is one: a fallible
// operation here simply raises `WireError`, unwinds through every call
// above it with no `try` at each site, and is caught once, at the harness
// that drives a probe. `crates/lang-rules/src/emit/fsharp.rs`'s module
// documentation is where that decision is argued.
//
// DDS-XTypes 1.3 §7.2.2.7's three try-construct behaviours are not folded
// into that exception, because "discarded" is not a failure the way a
// truncated stream is — the receiver drops the object and carries on, and a
// caller has to be able to tell the two apart *after* a decode finishes
// rather than catch two different exceptions during it. `Reader.Status` is
// therefore a plain mutable property, set by `ReaderRefuse` and read by
// whatever drives the probe once decoding returns, mirroring
// `zdwire.zig`'s `readerStatus()` exactly.
module ZdWire

open System
open System.Text

// -- errors and status -----------------------------------------------------

/// A hard failure: a stream that cannot be decoded at all, or a value with
/// no wire form. Raised, never returned — the module documentation says why.
exception WireError of string

/// The loop-break signal `crates/lang-render/src/fsharp.rs` renders
/// `Stmt::Leave` as `raise`. Carries nothing: it is caught by the nearest
/// enclosing `try` a generated `Stmt::Until` opened, and never escapes one —
/// a `LoopBreak` that reached the harness would be this column's defect and
/// not a wire question, so it is not caught there.
/// `of unit` and not a bare case: `crates/lang-render/src/fsharp.rs`
/// renders `Stmt::Leave` as `raise (ZdWire.LoopBreak())`, call syntax a
/// payload-less `exception LoopBreak` refuses ("this value is not a
/// function and cannot be applied", `fsc` 9.0.106) — only a payload
/// gives the case a constructor to call.
exception LoopBreak of unit

/// The early-return signal `crates/lang-render/src/fsharp.rs` renders
/// `Stmt::GiveBack` as `raise`. F# has no `return` keyword: a function's value
/// is its body's trailing expression, so a `return` the shared codec walk
/// writes inside a loop body — the `@mutable` read walk's answer to a second
/// header carrying an id it already read, DDS-XTypes 1.3 §7.4.3.4.2 — has no
/// spelling at all. This is that spelling. It is caught by the `try` a
/// generated `Stmt::Guarded` opens around the whole body, one level outside
/// every `Stmt::Until`, and the trailing expression after that `try` is what
/// the routine then answers.
///
/// A separate exception and not `LoopBreak`: an F# filter matches on the
/// exception's type, so `with :? ZdWire.LoopBreak -> ()` lets this one pass
/// through the loop it was raised in, which is exactly the difference between
/// leaving a loop and leaving the routine. `of unit` for the reason
/// `LoopBreak` carries one.
exception GiveBack of unit

/// The three behaviours DDS-XTypes 1.3 §7.2.2.7 attaches to a position, as
/// the reader's extra parameter takes it.
type TryConstruct =
    | Discard
    | UseDefault
    | Trim

/// What a decode left the reader in, mirroring `zdwire.zig`'s `Status`.
type Status =
    | Ok
    | Truncated
    | ImpossibleLength
    | OutOfRange
    | Discarded

/// Whether a PL_CDR1 parameter carries a member or an `@optional` member —
/// XTypes 1.3 §7.4.1.2's PID_PAD/ordinary-member distinction as the walk
/// hands it to `PidBegin`.
type ParameterRole =
    | Member
    | Optional

/// The byte order a stream is written or read in.
type Endian =
    | LE
    | BE

/// The XCDR version in force. XTypes 1.3 §7.4.1 puts MAXALIGN at eight
/// under XCDR1 and four under XCDR2.
type Encoding =
    | XCDR1
    | XCDR2

let private maxAlignOf (e: Encoding) =
    match e with
    | XCDR1 -> 8
    | XCDR2 -> 4

/// The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34.
let LC0 = 0
let LC1 = 1
let LC2 = 2
let LC3 = 3
let LC4 = 4
let LC5 = 5
let LC6 = 6
let LC7 = 7

/// PID_LIST_END. XTypes 1.3 §7.4.1.2 Table 31.
let PidListEndId = 0x3F02
/// The PID that introduces the twelve-octet parameter header.
let PidExtendedId = 0x3F01
/// The highest member id a short parameter header carries, inclusive.
let PidShortMaxId = 0x3F00
/// PID_IGNORE, the parameter §7.4.1.2.2 says a reader steps over
/// without looking for a member behind it.
let PidIgnoreId = 0x3F03

let private pidFlagMustUnderstand = 0x4000
/// FLAG_IMPL_EXTENSION, bit 15 of a short parameter id. XTypes 1.3
/// §7.4.1.2 splits the sixteen-bit field into a fourteen-bit id, bit 14
/// PID_MUST_UNDERSTAND and bit 15 PID_IMPL_EXTENSION.
let private pidFlagImplExtension = 0x8000
let private emFlagMustUnderstand = 0x40000000u

// -- what flags a parameter id carries, per position ------------------------

/// Where in a PL_CDR1 parameter list a flag question is asked. The F#
/// half of `zerodds_ast_layout::WirePosition`, cut down to the three
/// places that carry a parameter id at all.
///
/// An argument and not a field of the framing, because one stream
/// carries all three at once: a single setting made when the writer was
/// built could not answer for them together.
///
/// `MemberId` and not `Member`: `ParameterRole` already has a `Member`
/// case, and a second one would take the name away from it.
type PidPosition =
    | MemberId
    | Sentinel
    | ExtendedMarker

/// What flags a PL_CDR1 parameter id carries, asked per position.
///
/// §7.4.1.2 says nothing about the flags `PID_LIST_END` carries, and the
/// four measured implementations do not agree — `struct-mutable` and
/// `evolution-mutable-mustunderstand` under XCDR1:
///
/// ```text
///     Cyclone   member as declared    sentinel 7f02
///     RTI       member as declared    sentinel 7f02
///     Fast DDS  member never          sentinel 3f02
///     OpenDDS   member c0xx, always   sentinel 3f02
/// ```
///
/// The sentinel's answer cannot be read off the member's, which is what
/// makes `Sentinel` a position on this axis rather than a property of
/// the list.
type PidFlagPolicy =
    /// A bare member id, PID_MUST_UNDERSTAND on the sentinel and on the
    /// extended marker. Cyclone DDS and RTI Connext 7.7. The default.
    | SentinelMustUnderstand
    /// Bare everywhere. Fast DDS.
    | Bare
    /// Both flags on every member id whatever the member declared, a
    /// bare sentinel, and both flags on the extended marker. OpenDDS.
    | ImplExtensionOnMembers

/// The flag bits a parameter id carries at `position` under `policy`.
///
/// The member's own declaration is the second claimant on
/// PID_MUST_UNDERSTAND and is OR-ed on top of this by the writer; this
/// answers only the part that does not depend on the declaration.
let private pidFlagsAt (policy: PidFlagPolicy) (position: PidPosition) : int =
    match policy with
    | SentinelMustUnderstand ->
        match position with
        | Sentinel
        | ExtendedMarker -> pidFlagMustUnderstand
        | MemberId -> 0
    | Bare -> 0
    | ImplExtensionOnMembers ->
        match position with
        | MemberId
        | ExtendedMarker -> pidFlagImplExtension ||| pidFlagMustUnderstand
        | Sentinel -> 0

/// The name `PidFlagPolicy::as_str` gives, for diagnostics.
///
/// There is no parser back the other way, unlike the `wstring` axes
/// above: `wireprobe-zerodds-fsharp-gen` writes the union case itself,
/// so a misspelled policy is a compile error in the generated probe
/// rather than something a runtime lookup has to catch.
let pidFlagPolicyName (policy: PidFlagPolicy) : string =
    match policy with
    | SentinelMustUnderstand -> "sentinel-must-understand"
    | Bare -> "bare"
    | ImplExtensionOnMembers -> "impl-extension-members"

// -- the PL_CDR1 framing a generated probe configures -----------------------

/// The six independently-varying answers a PL_CDR1 dialect gives, exactly
/// the fields `wireprobe-zerodds-zig-gen`'s harness reads off `LayoutKey`
/// and writes into `zdwire.PlCdr1Framing`. A hand-written endpoint fixes
/// all six to the specification's own answer; a generated probe measures a
/// vendor dialect by varying them.
///
/// The EMHEADER length code is **not** here, although it once was: it is
/// an XCDR2 question and every field below is an XCDR1 one, and the other
/// nineteen bindings all carry it as a writer argument beside the framing
/// (`zdwire.Writer.init(heap, order, max_align, framing, short_codes)` in
/// Zig, `Writer(order, maxAlign, framing, shortCodes)` in C# and Kotlin).
/// See [`Writer`]'s `shortLengthCodes`.
type PlCdr1Framing =
    { /// What flags a parameter id carries, asked per position — a
      /// member's id, PID_LIST_END and the PID_EXTENDED marker. One
      /// setting for all three, because no position's answer can be read
      /// off another's.
      PidFlags: PidFlagPolicy
      /// Whether a short parameter id carries the must-understand bit the
      /// member itself declared.
      MemberMustUnderstand: bool
      /// Whether a short header's declared length pads to a four-octet
      /// boundary before the next parameter, for an ordinary member.
      PadDeclaredLength: bool
      /// The same, for an `@optional` member.
      PadDeclaredOptionalLength: bool
      /// Whether padding is written *after* the value even where the
      /// declared length already accounts for it.
      PadAfterValue: bool
      /// The highest member id this framing still gives the short header;
      /// above it every member takes the extended, twelve-octet form.
      HighestShortId: int }

/// The specification's own framing: every pad written, the short header
/// used through `PidShortMaxId`, and the must-understand flag set — what
/// Cyclone, RTI and Fast DDS all write for §7.4.1.2's mandatory members.
let specificationFraming: PlCdr1Framing =
    { PidFlags = SentinelMustUnderstand
      MemberMustUnderstand = true
      PadDeclaredLength = true
      PadDeclaredOptionalLength = true
      PadAfterValue = true
      HighestShortId = PidShortMaxId }

// -- the axes a dialect varies, as the shared walk names them ---------------
//
// The generated code hands each of these across as the string
// `crates/lang-rules/src/emit/fsharp.rs` renders for the walk's `Constant`
// — the same spelling `WstringLengthPolicy::as_str`,
// `WstringUnitOrder::as_str` and `FixedDigitCount::as_str` give. A name
// this runtime does not know is raised on rather than defaulted: a
// silently-defaulted policy would put a different length word on the wire
// and report success.

/// The three independent questions a `wstring` length convention crosses:
/// whether the prefix counts characters or octets, whether a terminating
/// NUL code unit is written and counted, and how wide one code unit is.
type private WstringPolicy =
    { UnitWidth: int
      CountsCharacters: bool
      Terminated: bool }

/// The cell `name` picks out. The eight names are
/// `zerodds_ast_layout::WstringLengthPolicy`'s.
let private wstringPolicyOf (name: string) : WstringPolicy =
    match name with
    | "specification" ->
        { UnitWidth = 2; CountsCharacters = false; Terminated = false }
    | "character-count" ->
        { UnitWidth = 2; CountsCharacters = true; Terminated = false }
    | "character-count-terminated" ->
        { UnitWidth = 4; CountsCharacters = true; Terminated = true }
    | "specification-wide-units" ->
        { UnitWidth = 4; CountsCharacters = false; Terminated = false }
    | "ros-type-support" ->
        { UnitWidth = 4; CountsCharacters = true; Terminated = false }
    | "octets/terminated/2" ->
        { UnitWidth = 2; CountsCharacters = false; Terminated = true }
    | "octets/terminated/4" ->
        { UnitWidth = 4; CountsCharacters = false; Terminated = true }
    | "characters/terminated/2" ->
        { UnitWidth = 2; CountsCharacters = true; Terminated = true }
    | other -> raise (WireError(sprintf "a wstring length policy this runtime knows, not `%s`" other))

/// In what byte order a `wstring`'s code units go out, as the walk spells
/// it. `stream` is the stream's own order; `big-endian-always` is what
/// omniORB and JacORB decode without a mark; `byte-order-mark` writes
/// U+FEFF and then little-endian units; `host-unswapped` is Cyclone's
/// unswapped content over a big-endian stream.
let private wstringOrderKnown (order: string) : bool =
    order = "stream"
    || order = "big-endian-always"
    || order = "byte-order-mark"
    || order = "host-unswapped"

let private wstringOrderOf (order: string) : string =
    if wstringOrderKnown order then
        order
    else
        raise (WireError(sprintf "a wstring unit order this runtime knows, not `%s`" order))

/// Whether a code unit goes out little-endian whatever the stream's order.
let private wstringFixedLittle (order: string) : bool =
    order = "byte-order-mark" || order = "host-unswapped"

/// The code unit U+FEFF, as `byte-order-mark` writes it.
let byteOrderMarkUnit = 0xFEFF

/// The same mark read the other way round, which says every unit behind
/// it is byte-swapped too.
let private swappedByteOrderMarkUnit = 0xFFFE

/// Whether `fixed<P,S>` puts `P` digit nibbles on the wire (`declared`)
/// or a count the value decides, so that the reader finds the field's end
/// at the sign nibble. [CDR] §9.3.2.8 gives the packing and not the count;
/// omniORB writes the declared `P`, JacORB 3.9 the value's own precision
/// (`value-precision`), ACE/TAO 2.5.24 the digits of the
/// string as written, made odd by one leading zero (`value-string-digits`).
let private fixedCountIsValueDecided (count: string) : bool =
    match count with
    | "declared" -> false
    | "value-precision"
    | "value-string-digits" -> true
    | other ->
        raise (
            WireError(
                sprintf
                    "a fixed digit count of `declared`, `value-precision` or `value-string-digits`, not `%s`"
                    other
            )
        )

/// The lowest nibble value that is a sign and not a digit. [CDR]
/// §9.3.2.8 makes a digit `0`..`9`, which is what makes `value-precision`
/// self-delimiting on the read side.
let firstSignNibble = 0x0A

// -- packed BCD for fixed<P,S> ----------------------------------------------

/// The nibbles of `value` under `fixed<digits, scale>`: digit nibbles
/// most-significant first, then the sign nibble — `0xC` for positive or
/// zero and `0xD` for negative — with a leading zero prepended where the
/// nibble count would otherwise be odd.
///
/// Apart from `Writer` so it can be tested at the edges of its range. The
/// full `digits`-wide field is built whatever the count, because the
/// value's own precision is that field with its leading zeros removed and
/// both counts are then read off one construction.
let fixedNibbles (value: string) (digits: int) (scale: int) (count: string) : int[] =
    if digits < 1 || digits > 31 then
        raise (WireError(sprintf "fixed<%d,%d>: a digit count is 1..31" digits scale))
    if scale < 0 || scale > digits then
        raise (WireError(sprintf "fixed<%d,%d>: a scale is 0..digits" digits scale))
    let valueDecided = fixedCountIsValueDecided count
    let mutable text = value
    let mutable negative = false
    if text.Length > 0 && (text.[0] = '+' || text.[0] = '-') then
        negative <- text.[0] = '-'
        text <- text.Substring(1)
    let dot = text.IndexOf('.')
    let mutable integral = if dot >= 0 then text.Substring(0, dot) else text
    let mutable fraction = if dot >= 0 then text.Substring(dot + 1) else ""
    if integral = "" && fraction = "" then
        raise (WireError(sprintf "fixed<%d,%d>: `%s` has no digits" digits scale value))
    for part in [ integral; fraction ] do
        for ch in part do
            if ch < '0' || ch > '9' then
                raise (WireError(sprintf "fixed<%d,%d>: `%s` is not a decimal" digits scale value))
    let asWritten = integral.Length + scale
    integral <- integral.TrimStart('0')
    if integral.Length > digits - scale then
        raise (
            WireError(
                sprintf
                    "fixed<%d,%d>: `%s` needs %d integer digits and the type has %d"
                    digits
                    scale
                    value
                    integral.Length
                    (digits - scale)
            )
        )
    while fraction.Length > scale && fraction.EndsWith("0") do
        fraction <- fraction.Substring(0, fraction.Length - 1)
    if fraction.Length > scale then
        raise (
            WireError(
                sprintf
                    "fixed<%d,%d>: `%s` needs %d fraction digits and the type has %d"
                    digits
                    scale
                    value
                    fraction.Length
                    scale
            )
        )
    let every = Array.zeroCreate<int> digits
    let mutable at = digits - scale - integral.Length
    for ch in integral do
        every.[at] <- int ch - int '0'
        at <- at + 1
    for ch in fraction do
        every.[at] <- int ch - int '0'
        at <- at + 1
    let mutable leading = 0
    while leading < every.Length && every.[leading] = 0 do
        leading <- leading + 1
    let significant = every.Length - leading
    let wire =
        if count = "value-string-digits" then
            // ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one
            // leading zero (0.12345 under fixed<5,5> is 0012345c). Past 31
            // no reader can be told the field's width.
            let wire = asWritten + (if asWritten % 2 = 0 then 1 else 0)
            if wire > 31 then
                raise (WireError(sprintf "fixed<%d,%d>: `%s` is %d digits on the wire, more than 31" digits scale value wire))
            wire
        elif valueDecided then
            max significant 1
        else
            digits
    let odd = (wire + 1) % 2 = 1
    let nibbles = ResizeArray<int>()
    if odd then
        nibbles.Add(0)
    // A count wider than P is filled with leading zeros.
    let kept = min wire every.Length
    for _ in 1 .. (wire - kept) do
        nibbles.Add(0)
    for index in (every.Length - kept) .. (every.Length - 1) do
        nibbles.Add(every.[index])
    // A negative zero is written positive, because there is one zero.
    nibbles.Add(if negative && significant > 0 then 0x0D else 0x0C)
    nibbles.ToArray()

/// The octets `WriteFixed` writes.
let fixedOctets (value: string) (digits: int) (scale: int) (count: string) : byte[] =
    let nibbles = fixedNibbles value digits scale count
    [| for i in 0 .. (nibbles.Length / 2) - 1 -> byte ((nibbles.[2 * i] <<< 4) ||| nibbles.[2 * i + 1]) |]

/// The decimal `nibbles` and `sign` spell under `scale`, in exactly the
/// text `fixedNibbles` took: leading zeros gone, because the BCD field is
/// `digits` wide whatever the value and `"000.01"` is a decimal the write
/// side would never produce from the same number. A negative zero loses
/// its sign for the same reason.
let fixedText (nibbles: int[]) (sign: int) (scale: int) : string =
    let out = StringBuilder()
    for nibble in nibbles do
        if nibble > 9 then
            raise (WireError(sprintf "a BCD digit is 0..9, not %d" nibble))
        out.Append(char (int '0' + nibble)) |> ignore
    // The point sits `scale` digits from the right and not `digits - scale`
    // from the left: under the value's own count the field carries fewer
    // than `digits`, and one carrying fewer than the scale needs its
    // integer part written as a zero.
    while out.Length < scale + 1 do
        out.Insert(0, '0') |> ignore
    let text = out.ToString()
    let split = text.Length - scale
    let whole =
        let trimmed = text.Substring(0, split).TrimStart('0')
        if trimmed = "" then "0" else trimmed
    let fraction = text.Substring(split)
    let result = if scale = 0 then whole else whole + "." + fraction
    let zero = whole = "0" && not (Seq.exists (fun c -> c <> '0') fraction)
    if sign = 0x0D && not zero then "-" + result else result

/// The inverse of [`fixedOctets`] for the `declared` count, where the
/// field is `(digits + 2) / 2` octets wide.
let fixedDecimal (octets: byte[]) (digits: int) (scale: int) : string =
    let all = ResizeArray<int>()
    for b in octets do
        all.Add(int b >>> 4)
        all.Add(int b &&& 0x0F)
    let sign = all.[all.Count - 1]
    let start = if (digits + 1) % 2 = 1 then 1 else 0
    let nibbles = [| for i in start .. (all.Count - 2) -> all.[i] |]
    fixedText nibbles sign scale

// -- IEEE 754 binary128, for `long double` -----------------------------------
//
// .NET has no binary128 and no extended float at all — `float` holds 53 of
// the 113 significand bits — so a `long double` member holds the decimal
// text and the two conversions live here: a correctly rounded parse on the
// way in (to nearest, ties to even, in one rounding) and the exact decimal
// expansion on the way out, in exactly the spelling the sibling columns'
// renderers produce, because the columns are compared as text.
//
// This section's function-by-function shape follows
// runtime/go/float128.go line for line, using
// `System.Numerics.BigInteger` where Go uses `math/big` — the same role
// Python's own arbitrary-precision `int` plays in `zerodds_wire/float128.py`.
// CORBA 3.3 part 2 §9.3.1.3 with Figure 9.2 gives CDR long double one sign
// bit, a fifteen-bit exponent biased by 16383 and a 112-bit stored fraction;
// DDS-XTypes 1.3 Table 31 names the result quadruple precision, sixteen
// bytes, aligned to eight.

/// Fields of binary128. IEEE 754-2008 clause 3.6.
let private f128FractionBits = 112
let private f128ExponentBias = 16383
let private f128ExponentMax = 0x7FFF

let private f128SignMask = System.Numerics.BigInteger.One <<< 127
let private f128FractionMask = (System.Numerics.BigInteger.One <<< f128FractionBits) - System.Numerics.BigInteger.One
let private f128QuietBit = System.Numerics.BigInteger.One <<< (f128FractionBits - 1)
let private f128ExponentBits = System.Numerics.BigInteger(f128ExponentMax) <<< f128FractionBits

/// 10^n for n >= 0.
let private f128Pow10 (n: int) : System.Numerics.BigInteger =
    System.Numerics.BigInteger.Pow(System.Numerics.BigInteger(10), n)

/// Shifts `value` right by `shift`, rounding to nearest with ties to
/// even. The second answer says whether anything was discarded.
let private f128ShiftRound (value: System.Numerics.BigInteger) (shift: int) : System.Numerics.BigInteger * bool =
    if shift <= 0 then
        (value, false)
    else
        let kept = value >>> shift
        let mask = (System.Numerics.BigInteger.One <<< shift) - System.Numerics.BigInteger.One
        let dropped = value &&& mask
        if dropped.IsZero then
            (kept, false)
        else
            let half = System.Numerics.BigInteger.One <<< (shift - 1)
            let keptOdd = (kept &&& System.Numerics.BigInteger.One) = System.Numerics.BigInteger.One
            if dropped > half || (dropped = half && keptOdd) then
                (kept + System.Numerics.BigInteger.One, true)
            else
                (kept, true)

/// Builds a pattern from a sign, an integral significand and a binary
/// exponent, rounding to nearest with ties to even. The second answer
/// says whether the rounding lost anything.
let private f128Compose
    (negative: bool)
    (significandIn: System.Numerics.BigInteger)
    (exponentIn: int)
    : System.Numerics.BigInteger * bool =
    let sign = if negative then f128SignMask else System.Numerics.BigInteger.Zero
    if significandIn.IsZero then
        (sign, false)
    // Clamped before anything is added: a decimal parser can hand this
    // an exponent of a hundred thousand.
    elif exponentIn > f128ExponentMax then
        (sign ||| f128ExponentBits, true)
    elif exponentIn < -(f128FractionBits + 1 + f128ExponentBias + 200) then
        (sign, true)
    else
        let mutable lost = false
        let mutable value = significandIn
        let mutable exponent = exponentIn
        let leading = int (value.GetBitLength()) - 1
        if leading > f128FractionBits then
            let drop = leading - f128FractionBits
            let shifted, dropped = f128ShiftRound value drop
            value <- shifted
            lost <- lost || dropped
            exponent <- exponent + drop
            if int (value.GetBitLength()) - 1 > f128FractionBits then
                value <- value >>> 1
                exponent <- exponent + 1
        else
            let shift = f128FractionBits - leading
            value <- value <<< shift
            exponent <- exponent - shift

        let biased = exponent + f128ExponentBias + f128FractionBits
        if biased >= f128ExponentMax then
            (sign ||| f128ExponentBits, true)
        elif biased <= 0 then
            // Subnormal. A value that rounds up to the smallest normal
            // carries out of the fraction into the exponent field on
            // its own, so the fraction is deliberately not masked here.
            let rounded, dropped = f128ShiftRound value (1 - biased)
            (sign ||| rounded, lost || dropped)
        else
            let out = (System.Numerics.BigInteger(biased) <<< f128FractionBits) ||| (value &&& f128FractionMask)
            (out ||| sign, lost)

/// Rounds `numerator`/`denominator` to binary128, to nearest with ties
/// to even. Scaled so the quotient carries more bits than binary128
/// needs, then everything discarded folds into one sticky bit — that is
/// what makes the single rounding at the end the correct one.
let private f128FromFraction
    (negative: bool)
    (numerator: System.Numerics.BigInteger)
    (denominator: System.Numerics.BigInteger)
    : System.Numerics.BigInteger =
    let wanted = 130
    let shift = max (wanted - (int (numerator.GetBitLength()) - int (denominator.GetBitLength()))) 0
    let scaled = numerator <<< shift
    let quotient0, remainder = System.Numerics.BigInteger.DivRem(scaled, denominator)
    let quotient =
        if remainder.IsZero then
            quotient0
        else
            quotient0 ||| System.Numerics.BigInteger.One
    let bits, _ = f128Compose negative quotient (-shift)
    bits

/// The pattern's sixteen bytes, least-significant first.
let private f128Bytes (bits: System.Numerics.BigInteger) : byte[] =
    // `ToByteArray(isUnsigned = true, isBigEndian = true)` answers the
    // fewest bytes that hold the value; `bits` here is always in
    // [0, 2^128) so it never needs a sign byte, but it can still answer
    // fewer than sixteen and has to be left-padded to them.
    let raw = bits.ToByteArray(true, true)
    let out = Array.zeroCreate<byte> 16
    Array.blit raw 0 out (16 - raw.Length) raw.Length
    Array.rev out

/// Parses `text` and answers the sixteen octets, least-significant
/// first. A malformed decimal raises [`WireError`].
let f128FromDecimal (text: string) : byte[] =
    let refuse () : 'a =
        raise (WireError(sprintf "a decimal for a long double, not `%s`" text))
    let stripped = text.Trim()
    let lowered = stripped.ToLowerInvariant()
    match lowered with
    | "inf"
    | "+inf"
    | "infinity"
    | "+infinity" -> f128Bytes f128ExponentBits
    | "-inf"
    | "-infinity" -> f128Bytes (f128ExponentBits ||| f128SignMask)
    | "nan"
    | "+nan" -> f128Bytes (f128ExponentBits ||| f128QuietBit)
    | "-nan" -> f128Bytes (f128ExponentBits ||| f128QuietBit ||| f128SignMask)
    | _ ->
        let mutable body = stripped
        let mutable negative = false
        if body.Length > 0 && (body.[0] = '+' || body.[0] = '-') then
            negative <- body.[0] = '-'
            body <- body.Substring(1)
        let mutable mantissa = body
        let mutable exponent10 = 0
        let split = body.IndexOfAny([| 'e'; 'E' |])
        if split >= 0 then
            mantissa <- body.Substring(0, split)
            match Int32.TryParse(body.Substring(split + 1)) with
            | true, parsed -> exponent10 <- parsed
            | false, _ -> refuse ()
        let point = mantissa.IndexOf('.')
        let whole = if point >= 0 then mantissa.Substring(0, point) else mantissa
        let fraction = if point >= 0 then mantissa.Substring(point + 1) else ""
        let digits = whole + fraction
        if digits = "" then
            refuse ()
        // An underscore is refused rather than ignored: in a value
        // file it is a typo, and reading `1_5` as fifteen is a quiet
        // reinterpretation.
        for ch in digits do
            if ch < '0' || ch > '9' then
                refuse ()
        let significand = System.Numerics.BigInteger.Parse(digits)
        let exponent10' = exponent10 - fraction.Length
        if significand.IsZero then
            if negative then f128Bytes f128SignMask else f128Bytes System.Numerics.BigInteger.Zero
        else
            // The exact rational, then one correct rounding on it. A
            // two-step parse through `float` rounds twice, and double
            // rounding gets the last bit wrong for the values that
            // matter here.
            let numerator, denominator =
                if exponent10' >= 0 then
                    (significand * f128Pow10 exponent10', System.Numerics.BigInteger.One)
                else
                    (significand, f128Pow10 (-exponent10'))
            f128Bytes (f128FromFraction negative numerator denominator)

/// Spells `0.<digits> * 10^point` in the three forms all the columns
/// use.
let private f128Render (negative: bool) (digits: string) (point: int) : string =
    let sign = if negative then "-" else ""
    if point > 0 && point <= 34 then
        if digits.Length <= point then
            sign + digits + String('0', point - digits.Length) + ".0"
        else
            sign + digits.Substring(0, point) + "." + digits.Substring(point)
    elif point > -6 && point <= 0 then
        sign + "0." + String('0', -point) + digits
    else
        let mantissa =
            if digits.Length > 1 then
                digits.Substring(0, 1) + "." + digits.Substring(1)
            else
                digits.Substring(0, 1) + ".0"
        let exponent = point - 1
        if exponent >= 0 then
            sprintf "%s%se+%d" sign mantissa exponent
        else
            sprintf "%s%se-%d" sign mantissa (-exponent)

/// Every digit the value has, in the corpus's canonical form — the
/// exact expansion and not the shortest one, in exactly the spelling
/// `crates/float128` and the sibling runtimes produce, because the
/// columns are compared as text.
let f128ToExactDecimal (bytes: byte[]) : string =
    if bytes.Length <> 16 then
        raise (WireError(sprintf "a binary128 pattern is sixteen octets, not %d" bytes.Length))
    let bigEndian = Array.rev bytes
    let bits = System.Numerics.BigInteger(bigEndian, isUnsigned = true, isBigEndian = true)
    let negative = not ((bits &&& f128SignMask).IsZero)
    let biased = int ((bits >>> f128FractionBits) &&& System.Numerics.BigInteger(f128ExponentMax))
    let fraction = bits &&& f128FractionMask
    if biased = f128ExponentMax then
        if not fraction.IsZero then "nan"
        elif negative then "-inf"
        else "inf"
    else
        let significand, exponent =
            if biased = 0 then
                (fraction, 1 - f128ExponentBias - f128FractionBits)
            else
                (fraction ||| (System.Numerics.BigInteger.One <<< f128FractionBits), biased - f128ExponentBias - f128FractionBits)
        if significand.IsZero then
            if negative then "-0.0" else "0.0"
        else
            let digits, point =
                if exponent >= 0 then
                    let shifted = significand <<< exponent
                    let d = shifted.ToString()
                    (d, d.Length)
                else
                    // m * 2^-k is exactly m * 5^k * 10^-k.
                    let scale = -exponent
                    let scaled = significand * System.Numerics.BigInteger.Pow(System.Numerics.BigInteger(5), scale)
                    let d = scaled.ToString()
                    (d, d.Length - scale)
            let trimmed = digits.TrimEnd('0')
            let trimmed = if trimmed = "" then "0" else trimmed
            f128Render negative trimmed point

// -- handles the runtime hands back ------------------------------------------

/// An open DHEADER, to be handed back to `FinishDheader`.
type DHeader = { BodyStart: int }

/// A value that takes no DHEADER — the plan says so and the walk still
/// needs a handle to hand `FinishDheader`, which then does nothing. A
/// free function and not a `Writer` method: it touches no writer state
/// at all, and `crates/lang-rules/src/emit/mod.rs`'s own documentation
/// of `Handle`/`CodecTarget::runtime_static` calls this out as "an
/// operation on the runtime that has no receiver" — C's free function
/// and this one are the same answer, where Java's is a static method on
/// the writer instead.
let noDheader () : DHeader = { BodyStart = -1 }

/// An open EMHEADER, to be handed back to `FinishEmheader`.
///
/// `Word` is where the header word itself begins and `BodyStart` where
/// the member's first octet does — four apart under `LC0`..`LC3` and
/// `LC5`..`LC7`, eight apart under `LC4`, whose NEXTINT sits between
/// them. The word's own position is kept because `FinishEmheader` may
/// have to raise the length code in it after the member is written; the
/// C# runtime's `EmHeader.Word` carries it for the same reason.
type EmHeader =
    { Word: int
      BodyStart: int
      LengthCode: int }

/// An open PL_CDR1 parameter header, to be handed back to `FinishPid`.
///
/// `Role` travels in the handle because the shared walk closes a
/// parameter with the handle alone — `crates/lang-rules/src/emit/
/// encode.rs` emits `finish_pid(handle)` — while whether the declared
/// length covers the trailing padding is a question about the role
/// (XTypes 1.3 §7.4.1.2). The C# runtime's `Pid` carries it for the same
/// reason.
type Pid =
    { At: int
      Extended: bool
      SavedOrigin: int
      Role: ParameterRole }

/// A saved reader position, for a two-pass read. What `ReaderSave` returns
/// and `ReaderRestore` takes.
type ReaderMark = { Pos: int; Origin: int }

/// A member header as `ReadEmheader` returns it.
type EmHeaderRead =
    { Id: uint32
      MustUnderstand: bool
      LengthCode: int
      Extent: uint32 }

/// A PL_CDR1 parameter header as `ReadPid` returns it.
type PidRead =
    { Id: uint32
      MustUnderstand: bool
      Length: uint32
      ListEnd: bool
      /// PID_IGNORE, which §7.4.1.2.2 says a reader steps over without
      /// looking for a member behind it.
      Ignore: bool
      /// Where the parameter's value begins, for `FinishPidRead`.
      ValueStart: int
      /// The alignment origin the parameter's own block replaced, put
      /// back by `FinishPidRead`. A nested parameter list would
      /// otherwise resume counting alignment from the inner block.
      SavedOrigin: int
      /// The role the caller read this header in. It travels with the
      /// handle for the same reason the writer's `Pid.Role` does — the
      /// shared walk closes a parameter with the handle alone.
      Role: ParameterRole }

    /// Whether this is a user member and not the sentinel — the one field
    /// `crates/lang-rules/src/emit/mod.rs`'s `Handle::PidRead` documents as
    /// a method on the header rather than a free function, because the
    /// walk always has a header in hand when it asks.
    member this.IsUserMember = not this.ListEnd

/// A member header slot before anything has been read into it. The walk
/// declares `zd_h` with no initialiser — C leaves the struct
/// uninitialised and F# has no such state — and the loop behind the
/// declaration overwrites it before anything reads it. `LengthCode = -1`
/// so a value that escaped that loop names no code in Table 34.
let noEmheaderRead () : EmHeaderRead =
    { Id = 0u
      MustUnderstand = false
      LengthCode = -1
      Extent = 0u }

/// A parameter header slot before anything has been read into it, for
/// the same reason [`noEmheaderRead`] is one. `ListEnd` is `true`, so a
/// value that escaped the loop that fills it reads as the sentinel and
/// not as a user member.
let noPidRead () : PidRead =
    { Id = 0u
      MustUnderstand = false
      Length = 0u
      ListEnd = true
      Ignore = false
      ValueStart = -1
      SavedOrigin = -1
      Role = Member }

// -- Writer: a byte accumulator ---------------------------------------------

/// Accumulates octets. `maxAlign` is [`Encoding`]'s cap on every alignment
/// this writer performs; `framing` is the PL_CDR1 policy `PidBegin` and
/// `PidEnd` write under.
///
/// `shortLengthCodes` is the XCDR2 half of the length-code question: an
/// `LC4` header whose member turns out to be 1, 2, 4 or 8 octets long
/// loses its NEXTINT and states the length in the code instead. Set it
/// from `LengthCodePolicy::short_code_where_it_fits` — the layout plan
/// already answers for every member whose *type* fixes the size, and
/// this is the other half, which only the writer can answer because it
/// depends on the sample. Fast DDS and OpenDDS write it; Cyclone, RTI
/// and the specification dialect do not.
type Writer(endian: Endian, encoding: Encoding, framing: PlCdr1Framing, shortLengthCodes: bool) =
    let buf = ResizeArray<byte>()
    let maxAlign = maxAlignOf encoding
    // The position alignment is measured from — zero for the stream, moved
    // to a PL_CDR1 parameter's own value for the parameter's length, exactly
    // as `endpoints/fsharp/zerodds.fs`'s `Writer` does.
    let mutable origin = 0

    /// A writer under the specification's own framing and no short
    /// length codes — what every hand-written endpoint writes, and what
    /// Cyclone and RTI write. There is deliberately no three-argument
    /// form: a caller that names a vendor's PL_CDR1 framing has a
    /// dialect in hand and must answer this axis too, and leaving it
    /// implicit is how the F# column came to write `LC4` under OpenDDS
    /// and Fast DDS where the other nineteen wrote `LC3`.
    new(endian: Endian, encoding: Encoding) =
        Writer(endian, encoding, specificationFraming, false)

    member _.MaxAlign = maxAlign

    /// Whether an `LC4` header is shortened when its member fits a code.
    member _.ShortLengthCodes = shortLengthCodes

    /// The stream's own length so far — a method and not a bare
    /// property, named the way the shared codec walk's `position` seam
    /// calls it: [`CodecTarget::runtime`] always renders a call, parens
    /// included.
    member _.Position() : int = buf.Count

    member _.Align(a: int) =
        let cap = min a maxAlign
        let pad = (cap - ((buf.Count - origin) % cap)) % cap
        for _ in 1..pad do
            buf.Add(0uy)

    /// `ElementPadding::Packed`: each collection element and each
    /// aggregate value counts its alignment from its own first octet
    /// instead of from the encapsulation. Returns the origin it displaced;
    /// `FinishPackedElement` restores it. `array-1d-struct-final` is
    /// `01 00 00 00 02 00 03 00 00 00 04 00 77`, the second Cell's `long`
    /// at offset six where the clause puts it at eight.
    member _.BeginPackedElement() : int =
        let saved = origin
        origin <- buf.Count
        saved

    member _.FinishPackedElement(saved: int) : unit = origin <- saved

    member this.Put(a: int, le: byte[]) =
        this.Align(a)
        if endian = BE then buf.AddRange(Array.rev le) else buf.AddRange(le)

    member _.WriteBool(v: bool) = buf.Add(if v then 1uy else 0uy)
    member _.WriteU8(v: byte) = buf.Add(v)
    member _.WriteI8(v: sbyte) = buf.Add(byte v)
    member _.WriteChar(v: byte) = buf.Add(v)

    member this.WriteU16(v: uint16) =
        this.Put(2, [| byte v; byte (v >>> 8) |])

    member this.WriteI16(v: int16) = this.WriteU16(uint16 v)

    member this.WriteU32(v: uint32) =
        this.Put(4, [| byte v; byte (v >>> 8); byte (v >>> 16); byte (v >>> 24) |])

    member this.WriteI32(v: int32) = this.WriteU32(uint32 v)

    /// A `long long` at its natural eight-octet alignment, which `MaxAlign`
    /// then caps: eight under XCDR1 and four under XCDR2.
    member this.WriteU64(v: uint64) =
        this.Put(8, [| for i in 0..7 -> byte (v >>> (8 * i)) |])

    member this.WriteI64(v: int64) = this.WriteU64(uint64 v)

    member this.WriteF32(v: float32) =
        let b = BitConverter.GetBytes(v)
        let le = if BitConverter.IsLittleEndian then b else Array.rev b
        this.Put(4, le)

    member this.WriteF64(v: float) =
        let b = BitConverter.GetBytes(v)
        let le = if BitConverter.IsLittleEndian then b else Array.rev b
        this.Put(8, le)

    /// An IDL `long double`: sixteen octets of IEEE 754 binary128,
    /// aligned to eight. `v` is the decimal text this column holds a
    /// `long double` member as; `f128FromDecimal` is the one correctly
    /// rounded parse.
    member this.WriteF128(v: string) =
        this.Put(8, f128FromDecimal v)

    /// An IDL `enum`: DDS-XTypes 1.3 §7.4.3.5.3 makes the wire width
    /// the `@bit_bound`'s and not the tag's own four octets, so
    /// `bitBound` (in bits) picks the write. `value` arrives widened to
    /// `int64` — the walk's own `Self::cast`, through `ZdWire.asI64` —
    /// because the generated field could be any of the four widths and
    /// this method takes one signature for all of them.
    member this.WriteEnum(bitBound: int, value: int64) : unit =
        if bitBound <= 8 then
            this.WriteI8(sbyte value)
        elif bitBound <= 16 then
            this.WriteI16(int16 value)
        elif bitBound <= 32 then
            this.WriteI32(int32 value)
        else
            this.WriteI64(value)

    /// An IDL `bitmask`: the same width rule as `WriteEnum`, unsigned.
    member this.WriteBitmask(bitBound: int, value: uint64) : unit =
        if bitBound <= 8 then
            this.WriteU8(byte value)
        elif bitBound <= 16 then
            this.WriteU16(uint16 value)
        elif bitBound <= 32 then
            this.WriteU32(uint32 value)
        else
            this.WriteU64(value)

    /// An IDL `wchar`. `width` is 2 or 4 octets and `giopLengthPrefixed`
    /// says whether a GIOP length octet precedes it — the two axes CORBA
    /// and DDS disagree on for the same construct.
    ///
    /// `order` is the `wstring` unit order the shared walk hands every
    /// wide-character write, and it decides something only in the GIOP
    /// form: CORBA 3.3 part 2 §9.3.1.6's octet count is followed by the
    /// code unit's own octets at alignment one, and only
    /// `WstringUnitOrder::Stream` takes the stream's byte order there —
    /// measured for the C# runtime, omniORB writes `02 00e4` for U+00E4
    /// in either stream order and TAO `04 e4000000`. Outside the GIOP
    /// form the unit is an ordinary integer in the stream's own order and
    /// `order` has nothing to say.
    member this.WriteWchar(codePoint: int, width: int, giopLengthPrefixed: bool, order: string) =
        if giopLengthPrefixed then
            this.WriteU8(byte width)
            let octets =
                match width with
                | 2 -> [| byte codePoint; byte (codePoint >>> 8) |]
                | 4 ->
                    [| byte codePoint
                       byte (codePoint >>> 8)
                       byte (codePoint >>> 16)
                       byte (codePoint >>> 24) |]
                | other -> raise (WireError(sprintf "a wchar width is 2 or 4 octets, not %d" other))
            let little = order = "stream" && endian = LE
            buf.AddRange(if little then octets else Array.rev octets)
        else
            match width with
            | 2 -> this.WriteU16(uint16 codePoint)
            | 4 -> this.WriteU32(uint32 codePoint)
            | other -> raise (WireError(sprintf "a wchar width is 2 or 4 octets, not %d" other))

    /// §7.4.1.1.1 Table 30: a `uint32` byte count (the UTF-8 encoding plus
    /// the terminating NUL) then the octets then the NUL.
    member this.WriteString(s: string) =
        let raw = Encoding.UTF8.GetBytes(s)
        this.WriteU32(uint32 (raw.Length + 1))
        buf.AddRange(raw)
        this.WriteU8(0uy)

    /// A `string<N>`. The same octets as an unbounded one — a bound is a
    /// promise about the source and not a difference on the wire, so this
    /// is the only place it can be enforced — refused rather than cut at
    /// a character boundary, which would put a sample on the wire nobody
    /// wrote.
    ///
    /// The bound is a `uint32`, as `CheckSequenceBound`'s is: the generated
    /// codec writes every bound as an unsigned literal (`32u`), and an
    /// `int` parameter made each bounded string a compile error.
    member this.WriteStringBounded(s: string, bound: uint32) =
        let raw = Encoding.UTF8.GetBytes(if isNull s then "" else s)
        if uint32 raw.Length > bound then
            raise (
                WireError(
                    sprintf
                        "at most %d octets in a bounded string, and this one has %d"
                        bound
                        raw.Length
                )
            )
        this.WriteString(s)

    /// A bounded `wstring`, as `WriteStringBounded` is to `WriteString`.
    /// The bound counts UTF-16 code units, not characters and not
    /// octets, whatever `policy` puts on the wire.
    member this.WriteWstringBounded(value: string, bound: uint32, policy: string, order: string) =
        let text = if isNull value then "" else value
        let units = text.Length
        if uint32 units > bound then
            raise (
                WireError(
                    sprintf
                        "at most %d UTF-16 code units in a bounded wstring, and this one has %d"
                        bound
                        units
                )
            )
        this.WriteWstringUtf8(text, policy, order)

    /// `WstringCodeUnits::Utf8`: the characters' own UTF-8 octets, with
    /// the length an octet count rather than §7.4.1.1.2's "twice the
    /// number of characters". `wstring-member` is
    /// `04 00 00 00 61 62 63 00` for `"abc"`; only C2 — the terminator —
    /// still reaches the wire. A lone surrogate has no UTF-8 octets, so
    /// it is refused rather than replaced.
    member this.WriteWstringUtf8Octets(value: string, policy: string) =
        let text = if isNull value then "" else value
        let form = wstringPolicyOf policy
        let mutable index = 0
        while index < text.Length do
            let unit = text.[index]
            if Char.IsHighSurrogate(unit) then
                if index + 1 < text.Length && Char.IsLowSurrogate(text.[index + 1]) then
                    index <- index + 2
                else
                    raise (WireError "a Unicode code point, not a lone surrogate")
            elif Char.IsLowSurrogate(unit) then
                raise (WireError "a Unicode code point, not a lone surrogate")
            else
                index <- index + 1
        let raw = Encoding.UTF8.GetBytes(text)
        let length = raw.Length + (if form.Terminated then 1 else 0)
        this.WriteU32(uint32 length)
        buf.AddRange(raw)
        if form.Terminated then
            this.WriteU8(0uy)

    /// A bounded UTF-8-octet `wstring`, as `WriteWstringBounded` is to
    /// `WriteWstringUtf8`. The bound still counts UTF-16 code units, which
    /// is what every `wstring<N>` bound means, while the wire carries the
    /// UTF-8 octets.
    member this.WriteWstringUtf8OctetsBounded(value: string, bound: uint32, policy: string) =
        let text = if isNull value then "" else value
        let units = text.Length
        if uint32 units > bound then
            raise (
                WireError(
                    sprintf
                        "at most %d UTF-16 code units in a bounded wstring, and this one has %d"
                        bound
                        units
                )
            )
        this.WriteWstringUtf8Octets(text, policy)

    /// The count check every bounded `sequence` makes before a byte of it
    /// is written. Table 17 makes `O2.length <= T1.length` the condition
    /// for constructing a `sequence<T, N>`, and the wire carries no bound
    /// to compare against: a sender whose count already exceeds its own
    /// declaration has written a value of a type nobody declared.
    member this.CheckSequenceBound(actual: uint32, bound: uint32) =
        if actual > bound then
            raise (
                WireError(
                    sprintf
                        "at most %d elements in a bounded sequence, and this one has %d"
                        bound
                        actual
                )
            )

    /// The count check every bounded `map` makes before a byte of it is
    /// written. Its own message and not `CheckSequenceBound`'s: the unit
    /// is an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
    /// map-as-a-sequence-of-pairs equivalence.
    member this.CheckMapBound(actual: uint32, bound: uint32) =
        if actual > bound then
            raise (
                WireError(
                    sprintf
                        "at most %d entries in a bounded map, and this one has %d"
                        bound
                        actual
                )
            )

    /// One `wstring` code unit, at its width and in its order.
    member private this.WriteWstringUnit(unit: int, width: int, order: string) =
        let little = wstringFixedLittle order
        let bigAlways = order = "big-endian-always"
        let octets =
            if width = 4 then
                [| byte unit; byte (unit >>> 8); byte (unit >>> 16); byte (unit >>> 24) |]
            else
                [| byte unit; byte (unit >>> 8) |]
        if bigAlways then
            this.Align(width)
            buf.AddRange(Array.rev octets)
        elif little then
            this.Align(width)
            buf.AddRange(octets)
        else
            this.Put(width, octets)

    /// A `wstring`, in the form `policy` and `order` name.
    ///
    /// Under `specification` the count is of **octets** and not of
    /// characters — XTypes 1.3 §7.4.1.1.2, "This is twice the number of
    /// characters in the string" — with no BOM and no terminator, both
    /// of which the same clause forbids outright. A .NET `string`
    /// already holds UTF-16 code units, which is the one thing this
    /// column gets free; what it does not get free is the surrogate
    /// rule, so a lone surrogate is refused rather than written.
    member this.WriteWstringUtf8(value: string, policy: string, order: string) =
        let text = if isNull value then "" else value
        let form = wstringPolicyOf policy
        let order = wstringOrderOf order
        if form.CountsCharacters && form.UnitWidth = 2 then
            // Fast CDR counts characters and hands each `wchar_t` over as
            // two octets, so a code point outside the BMP would go out as
            // its low sixteen bits and arrive as a different character. A
            // vendor's length convention is a policy; its data loss is
            // not, so the value is refused. The refusal is on the pair
            // (characters, two octets): a character count over four-octet
            // units carries the surrogate pair intact.
            for index in 0 .. text.Length - 1 do
                if Char.IsHighSurrogate(text.[index]) then
                    raise (
                        WireError(
                            "a code point inside the BMP: a character count over two-octet "
                            + "units has no way to carry one outside it"
                        )
                    )
        let units = ResizeArray<int>()
        let mutable index = 0
        while index < text.Length do
            let unit = text.[index]
            if Char.IsHighSurrogate(unit)
               && index + 1 < text.Length
               && Char.IsLowSurrogate(text.[index + 1]) then
                // A well-formed pair: both units go out, and a byte count
                // still reaches the end of the string. §7.4.1.1.2.
                units.Add(int unit)
                units.Add(int text.[index + 1])
                index <- index + 2
            elif Char.IsSurrogate(unit) then
                raise (WireError "a Unicode code point, not a lone surrogate")
            else
                units.Add(int unit)
                index <- index + 1
        let marked = order = "byte-order-mark" && units.Count > 0
        let total = units.Count + (if form.Terminated then 1 else 0) + (if marked then 1 else 0)
        this.WriteU32(uint32 (if form.CountsCharacters then total else total * form.UnitWidth))
        if marked then
            this.WriteWstringUnit(byteOrderMarkUnit, form.UnitWidth, order)
        for unit in units do
            this.WriteWstringUnit(unit, form.UnitWidth, order)
        if form.Terminated then
            this.WriteWstringUnit(0, form.UnitWidth, order)

    member _.WriteBytes(b: byte[]) = buf.AddRange(b)

    member this.WriteSeqU8(b: byte[]) =
        this.WriteU32(uint32 b.Length)
        buf.AddRange(b)

    /// `fixed<digits, scale>` as packed BCD, [CDR] §9.3.2.8. `count` says
    /// whether the declared `digits` nibbles go out or as many as the
    /// value has — the axis the four implementations that write one split
    /// two and two on.
    member this.WriteFixed(value: string, digits: int, scale: int, count: string) =
        this.WriteBytes(fixedOctets value digits scale count)

    /// The Cyclone union overlay: the low `octets` of `pattern`, written
    /// as one unsigned integer of that width in the stream's byte order.
    ///
    /// A discriminator carrying a label the `union-labels` dialect axis
    /// says selects nothing makes the peer fall through to `default:` and
    /// serialise its one C union storage *as that arm* — the selected
    /// member's octets, reinterpreted. `pattern` is the `uint64` whose
    /// little-endian bytes are the selected member's storage octets,
    /// zero-extended; `bitPatternInt` and its siblings produce it.
    member this.WriteUnionOverlay(pattern: uint64, octets: int) =
        match octets with
        | 1 -> this.WriteU8(byte (pattern &&& 0xFFUL))
        | 2 -> this.WriteU16(uint16 (pattern &&& 0xFFFFUL))
        | 4 -> this.WriteU32(uint32 (pattern &&& 0xFFFFFFFFUL))
        | 8 -> this.WriteU64(pattern)
        | other -> raise (WireError(sprintf "a union arm of 1, 2, 4 or 8 octets, not %d" other))

    /// One bitfield, shifted into the accumulator a `bitset` packs into.
    /// A value wider than the field is refused rather than masked:
    /// masking would put a different number on the wire than the caller
    /// asked for and the caller would never find out.
    member _.Bitfield(value: uint64, width: int, position: int) : uint64 =
        if width <= 0 || width > 64 || position < 0 || position + width > 64 then
            raise (WireError(sprintf "a bitfield of %d bits at %d, inside 64" width position))
        let limit = if width >= 64 then UInt64.MaxValue else (1UL <<< width) - 1UL
        if value > limit then
            raise (WireError(sprintf "a value inside %d bits, not %d" width value))
        value <<< position

    /// A `bitset` anchor: the holder the declared bitfields pack into, at
    /// the anchor's own width and therefore its own alignment. A value the
    /// anchor cannot hold is refused rather than masked — masking would
    /// drop the bitfields above the cut and report success.
    member this.WriteBitset(anchor: int, bits: uint64) =
        let check (limit: uint64) (what: string) =
            if bits > limit then
                raise (WireError(sprintf "%s holds at most %d, not %d" what limit bits))
        match anchor with
        | 1 ->
            check 0xFFUL "a one-octet bitset anchor"
            this.WriteU8(byte bits)
        | 2 ->
            check 0xFFFFUL "a two-octet bitset anchor"
            this.WriteU16(uint16 bits)
        | 4 ->
            check 0xFFFFFFFFUL "a four-octet bitset anchor"
            this.WriteU32(uint32 bits)
        | 8 -> this.WriteU64(bits)
        | other -> raise (WireError(sprintf "a bitset anchor is one, two, four or eight octets, not %d" other))

    // -- back-patched framing -------------------------------------------

    member private _.PatchU32(at: int, v: uint32) =
        let le = [| byte v; byte (v >>> 8); byte (v >>> 16); byte (v >>> 24) |]
        for i in 0..3 do
            buf.[at + i] <- (if endian = BE then le.[3 - i] else le.[i])

    member private _.PatchU16(at: int, v: int) =
        let le = [| byte v; byte (v >>> 8) |]
        for i in 0..1 do
            buf.[at + i] <- (if endian = BE then le.[1 - i] else le.[i])

    /// The `uint32` already written at `at`, read back in this stream's
    /// byte order. The counterpart of `PatchU32`, and the only reader on
    /// the write side: `ShortenEmheader` has to keep the member id and
    /// the must-understand bit of a header word whose length code it
    /// raises.
    member private _.ReadBackU32(at: int) : uint32 =
        let mutable v = 0u
        for i in 0..3 do
            let shift = if endian = BE then 8 * (3 - i) else 8 * i
            v <- v ||| (uint32 buf.[at + i] <<< shift)
        v

    /// Reserves a 4-aligned DHEADER slot. XTypes 1.3 §7.4.3.5.
    member this.WriteDheader() : DHeader =
        this.WriteU32(0u)
        { BodyStart = buf.Count }

    /// Back-patches the DHEADER with the byte length written since it
    /// opened. A [`noDheader`] handle patches nothing.
    member this.FinishDheader(h: DHeader) =
        if h.BodyStart >= 0 then
            this.PatchU32(h.BodyStart - 4, uint32 (buf.Count - h.BodyStart))

    /// The EMHEADER of one `@mutable` member. XTypes 1.3 §7.4.3.4.2:
    /// `(must_understand &lt;&lt; 31) | (length_code &lt;&lt; 28) | member_id`.
    /// For `LC4` a NEXTINT slot follows and `FinishEmheader` fills it; for
    /// `LC0`..`LC3` the code alone gives the length; for `LC5`..`LC7` the
    /// body's own leading word is the length and nothing is patched.
    ///
    /// `memberId` is an `int` and not a `uint32` for the reason
    /// `crates/lang-rules/src/emit/fsharp.rs` states at its `declare`
    /// seam: the shared walk hands every member id through its own
    /// `int` seam, and F# converts no integer width implicitly, so a
    /// runtime that declared `uint32` here would reject `w.WriteEmheader
    /// (54728492, 2, false)` outright. `runtime/go/writer.go` takes the
    /// same `int` for the same argument. The 28-bit bound is checked
    /// here, so nothing wider reaches the header.
    member this.WriteEmheader(memberId: int, lengthCode: int, mustUnderstand: bool) : EmHeader =
        if memberId < 0 || memberId > 0x0FFFFFFF then
            raise (WireError(sprintf "member id %d does not fit the 28 bits of an EMHEADER" memberId))
        if lengthCode < LC0 || lengthCode > LC7 then
            raise (WireError(sprintf "length code %d is not one of LC0..LC7" lengthCode))
        let mutable header = (uint32 lengthCode <<< 28) ||| uint32 memberId
        if mustUnderstand then
            header <- header ||| 0x80000000u
        this.WriteU32(header)
        // After the write, because `WriteU32` aligns to four first and the
        // padding it may have emitted comes before the word.
        let wordAt = buf.Count - 4
        if lengthCode = LC4 then
            this.WriteU32(0u)
        { Word = wordAt
          BodyStart = buf.Count
          LengthCode = lengthCode }

    /// Back-patches an EMHEADER's NEXTINT with the length of what
    /// followed it.
    ///
    /// Under [`Writer.ShortLengthCodes`] an `LC4` header whose member
    /// turned out to be 1, 2, 4 or 8 octets long instead loses its
    /// NEXTINT and states the length in the code, which is what Fast DDS
    /// and OpenDDS write. XTypes 1.3 §7.4.3.4.2 makes `LC0` through
    /// `LC3` the lengths 1, 2, 4 and 8 with no NEXTINT at all, so both
    /// forms are legal for such a member and the peer's choice is the
    /// one to reproduce.
    member this.FinishEmheader(h: EmHeader) =
        let written = buf.Count - h.BodyStart
        if h.LengthCode = LC4 then
            if shortLengthCodes && (written = 1 || written = 2 || written = 4 || written = 8) then
                this.ShortenEmheader(h, written)
            else
                this.PatchU32(h.BodyStart - 4, uint32 written)
        elif h.LengthCode <= LC3 then
            let want = 1 <<< h.LengthCode
            if written <> want then
                raise (WireError(sprintf "LC%d frames %d octets and the member wrote %d" h.LengthCode want written))
        elif written < 4 then
            raise (
                WireError(
                    sprintf
                        "LC%d takes its length from the body's first four octets and the member wrote %d"
                        h.LengthCode
                        written
                )
            )

    /// Removes an `LC4` header's NEXTINT and raises its length code to
    /// the one that spells `written`.
    ///
    /// Every octet of the member's value is unchanged and only its
    /// position moves, which is safe for one reason and only under
    /// XCDR2: `MAXALIGN` is four there, so the value began at a
    /// four-aligned offset with the NEXTINT in front of it and begins at
    /// a four-aligned offset without it, and no padding inside it was
    /// computed against an offset that has moved. Nothing outside the
    /// member has been written yet — `FinishEmheader` runs immediately
    /// after the value — so an enclosing DHEADER or parameter header
    /// sits at a lower offset and is patched afterwards from
    /// `buf.Count`, which the removal has already corrected.
    member private this.ShortenEmheader(h: EmHeader, written: int) =
        let code =
            match written with
            | 1 -> 0u
            | 2 -> 1u
            | 4 -> 2u
            | _ -> 3u
        buf.RemoveRange(h.BodyStart - 4, 4)
        let previous = this.ReadBackU32(h.Word)
        this.PatchU32(h.Word, (previous &&& ~~~(0x7u <<< 28)) ||| (code <<< 28))

    /// A PL_CDR1 short parameter header: at a four-octet boundary an
    /// `unsigned short` id, an `unsigned short` byte length. Both go out
    /// in the stream's byte order. `mustUnderstand` is the member's own
    /// declaration and §7.4.1.2.2's flag is written "if and only if" it
    /// is set — `framing.MemberMustUnderstand` is the second claimant on
    /// the same bit and `framing.PidFlags` asked at `MemberId` the third,
    /// the one a dialect puts on every id regardless. An
    /// `@optional` member's own presence is the codec's, not this
    /// header's, concern; `role` only decides what `FinishPid` counts
    /// into the declared length.
    member this.WritePid(pid: int, role: ParameterRole, mustUnderstand: bool) : Pid =
        if pid < 0 || pid > framing.HighestShortId then
            raise (WireError(sprintf "member id %d needs the extended parameter header" pid))
        this.Align(4)
        let flagged = mustUnderstand && framing.MemberMustUnderstand
        let declared = if flagged then pid ||| pidFlagMustUnderstand else pid
        let id = declared ||| pidFlagsAt framing.PidFlags MemberId
        this.Put(2, [| byte id; byte (id >>> 8) |])
        let at = buf.Count
        buf.Add(0uy)
        buf.Add(0uy)
        let saved = origin
        origin <- buf.Count
        { At = at
          Extended = false
          SavedOrigin = saved
          Role = role }

    /// The twelve-octet parameter header XTypes 1.3 §7.4.1.2 requires
    /// above `framing.HighestShortId` or for a value past 65535 octets.
    /// The marker's own flags come from `framing.PidFlags` asked at the
    /// `ExtendedMarker` position; the member's must-understand is bit 30
    /// of the eMemberHeader and never the marker's bit above it.
    ///
    /// `memberId` is an `int` for the same reason `WriteEmheader`'s is.
    member this.WritePidExtended(memberId: int, role: ParameterRole, mustUnderstand: bool) : Pid =
        if memberId < 0 || memberId > 0x0FFFFFFF then
            raise (WireError(sprintf "member id %d does not fit the 28 bits of an eMemberHeader" memberId))
        this.Align(4)
        let marker = PidExtendedId ||| pidFlagsAt framing.PidFlags ExtendedMarker
        this.Put(2, [| byte marker; byte (marker >>> 8) |])
        this.Put(2, [| 8uy; 0uy |])
        let flagged = mustUnderstand && framing.MemberMustUnderstand
        let header =
            if flagged then uint32 memberId ||| emFlagMustUnderstand else uint32 memberId
        this.Put(4, [| byte header; byte (header >>> 8); byte (header >>> 16); byte (header >>> 24) |])
        let at = buf.Count
        for _ in 1..4 do
            buf.Add(0uy)
        let saved = origin
        origin <- buf.Count
        { At = at
          Extended = true
          SavedOrigin = saved
          Role = role }

    /// Closes the parameter `WritePid` or `WritePidExtended` opened. The
    /// declared length counts the value's octets and, where `framing`
    /// says so, the padding that brings the next header to a four-octet
    /// boundary — a reader skips `length` from the end of the header and
    /// has to land on the next parameter.
    member this.FinishPid(h: Pid) =
        let width = if h.Extended then 4 else 2
        let mutable length = buf.Count - h.At - width
        let padsLength =
            match h.Role with
            | Member -> framing.PadDeclaredLength
            | Optional -> framing.PadDeclaredOptionalLength
        if padsLength then
            let past = (buf.Count - h.SavedOrigin) % 4
            if past <> 0 then
                length <- length + (4 - past)
        if h.Extended then
            this.PatchU32(h.At, uint32 length)
        else
            if length > 0xFFFF then
                raise (WireError(sprintf "a short parameter header describes at most 65535 octets, not %d" length))
            this.PatchU16(h.At, length)
        origin <- h.SavedOrigin
        if framing.PadAfterValue then
            this.Align(4)

    /// The sentinel that closes a PL_CDR1 parameter list: PID_LIST_END with
    /// a zero length, four-aligned. Its flags come from the framing asked
    /// at the `Sentinel` position, which is the only thing that decides
    /// them — PID_LIST_END is not a member and has no declaration of its
    /// own. 0x7F02 under Cyclone's and RTI's framing, the bare 0x3F02
    /// under Fast DDS's and OpenDDS's.
    member this.WriteSentinel() =
        this.Align(4)
        let id = PidListEndId ||| pidFlagsAt framing.PidFlags Sentinel
        this.Put(2, [| byte id; byte (id >>> 8) |])
        this.Put(2, [| 0uy; 0uy |])

    member _.Finish() : byte[] = buf.ToArray()

// -- Reader: a byte cursor ---------------------------------------------------

type Reader(data: byte[], endian: Endian, encoding: Encoding) =
    let mutable pos = 0
    let maxAlign = maxAlignOf encoding
    let mutable origin = 0
    let mutable status = Ok

    member _.MaxAlign = maxAlign

    /// The cursor's own position, named the way the shared codec walk's
    /// `reader_position` seam calls it — not a bare `Position` property,
    /// because [`CodecTarget::runtime`] always renders a call, parens
    /// included, and a property does not take any.
    member _.ReaderPosition() : int = pos

    /// The octets not yet read.
    member _.ReaderRemaining() : int = data.Length - pos

    /// What the last decode left this reader in — §7.2.2.7's soft
    /// "discarded" state included. A generated decoder never inspects
    /// this itself; the harness that drove it reads it once the call
    /// returns, exactly as `zdwire.zig`'s `readerStatus()` is read.
    member _.ReaderStatus() : Status = status

    /// Marks the reader with `reason` — DDS-XTypes 1.3 §7.2.2.7's
    /// `TryConstruct.Discard` is one of the walk's own reasons to call
    /// this, and a value out of range or a length no stream that long
    /// can carry are two more; the walk chooses which and passes it
    /// rather than this method deciding. Not an exception, because a
    /// refusal here is not this reader's failure to go on — the walk
    /// may still return a value from the position, and the harness
    /// reads the status once the whole decode finishes.
    member _.ReaderRefuse(reason: Status) : unit = status <- reason

    /// Clears a discard a caller has already accounted for, so a later
    /// member's own try-construct behaviour is judged on its own
    /// position and not on one a sibling member left standing.
    member _.ReaderClearDiscard() : unit =
        if status = Discarded then
            status <- Ok

    member this.ReaderSave() : ReaderMark = { Pos = pos; Origin = origin }

    member this.ReaderRestore(mark: ReaderMark) : unit =
        pos <- mark.Pos
        origin <- mark.Origin

    /// The read-side pair of `Writer.BeginPackedElement`. See that method
    /// for the axis and the recorded rows.
    member _.ReaderBeginPackedElement() : int =
        let saved = origin
        origin <- pos
        saved

    member _.ReaderFinishPackedElement(saved: int) : unit = origin <- saved

    /// Steps the cursor over the padding that brings it to an `a`-octet
    /// boundary of the enclosing block, capped by `MaxAlign` exactly as
    /// `Take` caps it. Separate from `Take` because a parameter header
    /// aligns to four before reading two octets, which no single read
    /// expresses.
    member _.AlignRead(a: int) : unit =
        let cap = min a maxAlign
        let pad = (cap - ((pos - origin) % cap)) % cap
        if pos + pad > data.Length then
            raise (WireError(sprintf "%d octets of padding requested at %d, %d remain" pad pos (data.Length - pos)))
        pos <- pos + pad

    member this.ReaderSkipTo(target: int) : unit =
        if target < pos || target > data.Length then
            raise (WireError(sprintf "cannot skip to %d from %d in a stream of %d" target pos data.Length))
        pos <- target

    member this.Take(a: int, n: int) : byte[] =
        let cap = min a maxAlign
        let pad = (cap - ((pos - origin) % cap)) % cap
        if pos + pad + n > data.Length then
            raise (WireError(sprintf "%d octets requested at %d, %d remain" n (pos + pad) (data.Length - pos - pad)))
        pos <- pos + pad
        let chunk = Array.sub data pos n
        pos <- pos + n
        if endian = BE then Array.rev chunk else chunk

    member _.ReadU8() : byte =
        if pos >= data.Length then
            raise (WireError(sprintf "one octet requested at %d, none remain" pos))
        let b = data.[pos]
        pos <- pos + 1
        b

    member this.ReadI8() : sbyte = sbyte (this.ReadU8())
    member this.ReadBool() : bool = this.ReadU8() <> 0uy
    member this.ReadChar() : byte = this.ReadU8()

    member this.ReadU16() : uint16 =
        let b = this.Take(2, 2)
        uint16 b.[0] ||| (uint16 b.[1] <<< 8)

    member this.ReadI16() : int16 = int16 (this.ReadU16())

    member this.ReadU32() : uint32 =
        let b = this.Take(4, 4)
        uint32 b.[0] ||| (uint32 b.[1] <<< 8) ||| (uint32 b.[2] <<< 16) ||| (uint32 b.[3] <<< 24)

    member this.ReadI32() : int32 = int32 (this.ReadU32())

    /// A `long long` at its natural eight-octet alignment, which `MaxAlign`
    /// then caps: eight under XCDR1 and four under XCDR2.
    member this.ReadU64() : uint64 =
        let b = this.Take(8, 8)
        let mutable v = 0UL
        for i in 0..7 do
            v <- v ||| (uint64 b.[i] <<< (8 * i))
        v

    member this.ReadI64() : int64 = int64 (this.ReadU64())

    member this.ReadF32() : float32 =
        let le = this.Take(4, 4)
        let hb = if BitConverter.IsLittleEndian then le else Array.rev le
        BitConverter.ToSingle(hb, 0)

    member this.ReadF64() : float =
        let le = this.Take(8, 8)
        let hb = if BitConverter.IsLittleEndian then le else Array.rev le
        BitConverter.ToDouble(hb, 0)

    /// The inverse of `Writer.WriteF128`: sixteen octets, aligned to
    /// eight, back into the decimal text `f128ToExactDecimal` gives it.
    member this.ReadF128() : string =
        let le = this.Take(8, 16)
        f128ToExactDecimal le

    /// The inverse of `Writer.WriteEnum`: read at the `@bit_bound`'s
    /// width and widen to `int64`, which `ZdWire.asI64` (through the
    /// walk's own `Self::cast`, `enum&lt;T&gt;`) narrows back to the
    /// generated tag's own width.
    member this.ReadEnum(bitBound: int) : int64 =
        if bitBound <= 8 then
            int64 (this.ReadI8())
        elif bitBound <= 16 then
            int64 (this.ReadI16())
        elif bitBound <= 32 then
            int64 (this.ReadI32())
        else
            this.ReadI64()

    /// The inverse of `Writer.WriteBitmask`.
    member this.ReadBitmask(bitBound: int) : uint64 =
        if bitBound <= 8 then
            uint64 (this.ReadU8())
        elif bitBound <= 16 then
            uint64 (this.ReadU16())
        elif bitBound <= 32 then
            uint64 (this.ReadU32())
        else
            this.ReadU64()

    /// The inverse of `Writer.WriteWchar`, `order` included: the GIOP
    /// form's octets are at alignment one and only
    /// `WstringUnitOrder::Stream` reads them in the stream's byte order.
    member this.ReadWchar(width: int, giopLengthPrefixed: bool, order: string) : int =
        if giopLengthPrefixed then
            this.ReadU8() |> ignore
            let mutable value = 0
            let raw = Array.init width (fun _ -> this.ReadU8())
            let ordered = if order = "stream" && endian = LE then raw else Array.rev raw
            for index in (width - 1) .. -1 .. 0 do
                value <- (value <<< 8) ||| int ordered.[index]
            value
        else
            match width with
            | 2 -> int (this.ReadU16())
            | 4 -> int (this.ReadU32())
            | other -> raise (WireError(sprintf "a wchar width is 2 or 4 octets, not %d" other))

    /// The inverse of `Writer.WriteString`: read the announced byte count
    /// (including the NUL) and decode all but the last octet as UTF-8.
    member this.ReadString() : string =
        let n = int (this.ReadU32())
        if n < 1 then
            raise (WireError "a string's length is at least one octet, for the terminator")
        if pos + n > data.Length then
            raise (WireError(sprintf "a string of %d octets requested at %d, fewer remain" n pos))
        let s =
            try
                Encoding.UTF8.GetString(data, pos, n - 1)
            with _ ->
                raise (WireError "a string was not valid UTF-8")
        pos <- pos + n
        s

    /// The same read, under the name the shared codec walk's
    /// `read_string_alloc` seam asks for — C's `zdw_read_string_alloc`
    /// names the allocation its own reader has to perform for the read
    /// direction that `ReadString` does not; a .NET `string` is heap
    /// storage either way, so the two names answer alike here.
    member this.ReadStringAlloc() : string = this.ReadString()

    /// The octets an order-fixed code unit occupies, taken without the
    /// stream's own byte order applied — `Take` reverses for a
    /// big-endian stream and a `wstring` unit under `big-endian-always`
    /// or `host-unswapped` is not in the stream's order at all.
    member private _.TakeRaw(a: int, n: int) : byte[] =
        let cap = min a maxAlign
        let pad = (cap - ((pos - origin) % cap)) % cap
        if pos + pad + n > data.Length then
            raise (WireError(sprintf "%d octets requested at %d, %d remain" n (pos + pad) (data.Length - pos - pad)))
        pos <- pos + pad
        let chunk = Array.sub data pos n
        pos <- pos + n
        chunk

    /// One `wstring` code unit at `width` octets, in `order`.
    member private this.ReadWstringUnit(width: int, order: string) : int =
        if wstringFixedLittle order then
            let b = this.TakeRaw(width, width)
            let mutable v = 0
            for i in 0 .. width - 1 do
                v <- v ||| (int b.[i] <<< (8 * i))
            v
        elif order = "big-endian-always" then
            let b = this.TakeRaw(width, width)
            let mutable v = 0
            for i in 0 .. width - 1 do
                v <- (v <<< 8) ||| int b.[i]
            v
        elif width = 4 then
            int (this.ReadU32())
        else
            int (this.ReadU16())

    /// Take the byte-order mark off the front and apply what it said.
    ///
    /// The units arrived little-endian, because that is what omniORB
    /// writes and what the `byte-order-mark` order therefore reads.
    /// `FEFF` — the mark is the right way up and the units are as read.
    /// `FFFE` — the mark is byte-swapped, so every unit behind it is
    /// too. No mark at all — CORBA's rule for a GIOP 1.2 `wstring`
    /// without one is UTF-16 big-endian, so every unit read
    /// little-endian is swapped. The mark is never a character of the
    /// returned string in any of the three.
    member private _.StripByteOrderMark(units: int[]) : int[] =
        if units.Length = 0 then
            units
        elif units.[0] = byteOrderMarkUnit then
            Array.sub units 1 (units.Length - 1)
        else
            let from = if units.[0] = swappedByteOrderMarkUnit then 1 else 0
            [| for i in from .. units.Length - 1 -> ((units.[i] &&& 0xFF) <<< 8) ||| (units.[i] >>> 8) |]

    /// A `wstring`, in the form `policy` and `order` name — the inverse
    /// of `Writer.WriteWstringUtf8`.
    ///
    /// The form is never inferred from the stream. The three axes are
    /// not separable: a Fast DDS `"ab"` followed by a `long` is
    /// byte-identical to a specification `"a"` followed by the same
    /// `long`, differing only in padding no clause fixes. Inferring
    /// would let the length field — the one field a hostile peer
    /// controls — decide how much is read.
    member this.ReadWstringUtf8(policy: string, order: string) : string =
        let form = wstringPolicyOf policy
        let order = wstringOrderOf order
        let declared = int (this.ReadU32())
        if declared < 0 then
            raise (WireError(sprintf "a wstring length of %d octets" declared))
        let count =
            if form.CountsCharacters then
                declared
            else
                // §7.4.1.1.2 counts octets, so a count the unit width does
                // not divide is a stream no encoder produced: rounding it
                // down would leave the remainder in the stream and put
                // every later member at the wrong offset.
                if declared % form.UnitWidth <> 0 then
                    raise (
                        WireError(
                            sprintf "a wstring's octet length %d is not a multiple of %d" declared form.UnitWidth
                        )
                    )
                declared / form.UnitWidth
        if count * form.UnitWidth > data.Length - pos then
            raise (
                WireError(
                    sprintf "a wstring of %d octets requested at %d, %d remain" (count * form.UnitWidth) pos (data.Length - pos)
                )
            )
        let mutable units = [| for _ in 1..count -> this.ReadWstringUnit(form.UnitWidth, order) |]
        if form.UnitWidth = 4 then
            for unit in units do
                if unit > 0xFFFF then
                    raise (WireError "a four-octet wstring character holding one UTF-16 unit")
        if form.Terminated then
            // The count includes the NUL, so a count of zero is a frame
            // that does not hold its own terminator.
            if units.Length = 0 then
                raise (WireError "a wstring whose count includes its terminator carries at least one unit")
            if units.[units.Length - 1] <> 0 then
                raise (WireError "a NUL character ending the wstring, as its count includes one")
            units <- Array.sub units 0 (units.Length - 1)
            // A NUL inside the text is the narrow string's problem in wide
            // form: every C peer stops at it. Only where a terminator is
            // expected — a cell without one may carry U+0000 as a
            // character.
            for unit in units do
                if unit = 0 then
                    raise (WireError "no NUL inside a wstring a C peer would stop at")
        if order = "byte-order-mark" then
            units <- this.StripByteOrderMark(units)
        let text = StringBuilder(units.Length)
        let mutable index = 0
        while index < units.Length do
            let unit = units.[index]
            if unit >= 0xD800 && unit <= 0xDBFF && index + 1 < units.Length
               && units.[index + 1] >= 0xDC00 && units.[index + 1] <= 0xDFFF then
                text.Append(char unit).Append(char units.[index + 1]) |> ignore
                index <- index + 2
            elif unit >= 0xD800 && unit <= 0xDFFF then
                raise (WireError "a UTF-16 sequence with no lone surrogate")
            else
                text.Append(char unit) |> ignore
                index <- index + 1
        text.ToString()

    /// The inverse of `Writer.WriteWstringUtf8Octets`: the declared length
    /// is the UTF-8 octet count, the terminator `policy` asks for is the
    /// last of those octets, and a count the stream cannot hold, a missing
    /// or an interior NUL, or an ill-formed UTF-8 sequence is refused.
    member this.ReadWstringUtf8Octets(policy: string) : string =
        let form = wstringPolicyOf policy
        let declared = int (this.ReadU32())
        if declared < 0 then
            raise (WireError(sprintf "a wstring length of %d octets" declared))
        if declared > this.ReaderRemaining() then
            raise (
                WireError(
                    sprintf "a wstring of %d octets requested at %d, %d remain" declared pos (data.Length - pos)
                )
            )
        let raw = this.Take(1, declared)
        let kept =
            if form.Terminated then
                if declared = 0 || raw.[declared - 1] <> 0uy then
                    raise (WireError "a NUL octet ending the wstring, as its count includes one")
                for i in 0 .. declared - 2 do
                    if raw.[i] = 0uy then
                        raise (WireError "no NUL inside a wstring a C peer would stop at")
                Array.sub raw 0 (declared - 1)
            else
                raw
        // §7.4.1.1.2's "twice the number of characters" is not this axis:
        // the declared length is the UTF-8 octet count, and a value that
        // is not UTF-8 is refused rather than carried as a string no peer
        // could tell was never a wstring.
        try
            (UTF8Encoding(false, true)).GetString(kept)
        with _ ->
            raise (WireError "a wstring was not valid UTF-8")

    /// `text` cut to at most `octets` UTF-8 bytes, never inside a
    /// character. Table 11: "The order of the characters in the string
    /// … is preserved", so this is a prefix. A character that would
    /// straddle the bound is left out whole — a peer holding
    /// `char[octets + 1]` cannot store half of one either, and half a
    /// UTF-8 sequence is not text.
    member _.TrimUtf8(text: string, octets: int) : string =
        if isNull text || octets < 0 then
            ""
        else
            let kept = StringBuilder()
            let mutable used = 0
            let mutable index = 0
            let mutable stop = false
            while not stop && index < text.Length do
                let step =
                    if Char.IsHighSurrogate(text.[index])
                       && index + 1 < text.Length
                       && Char.IsLowSurrogate(text.[index + 1]) then
                        2
                    else
                        1
                let width = Encoding.UTF8.GetByteCount(text.Substring(index, step))
                if used + width > octets then
                    stop <- true
                else
                    kept.Append(text.Substring(index, step)) |> ignore
                    used <- used + width
                    index <- index + step
            kept.ToString()

    /// `text` cut to at most `units` UTF-16 code units, never between
    /// the halves of a surrogate pair.
    member _.TrimUtf16(text: string, units: int) : string =
        if isNull text || units < 0 then
            ""
        elif text.Length <= units then
            text
        else
            // A high surrogate at the cut has lost its pair, and half a
            // pair is not a character.
            let end_ = if units > 0 && Char.IsHighSurrogate(text.[units - 1]) then units - 1 else units
            text.Substring(0, end_)

    member this.ReadSeqU8() : byte[] =
        let n = int (this.ReadU32())
        if pos + n > data.Length then
            raise (WireError(sprintf "a sequence of %d octets requested at %d, fewer remain" n pos))
        let b = Array.sub data pos n
        pos <- pos + n
        b

    /// `fixed<digits, scale>` as the decimal string the writer took.
    ///
    /// Under `declared` the field is `(digits + 2) / 2` octets wide.
    /// Under `value-precision` and `value-string-digits` its end is read out of the octets: [CDR]
    /// §9.3.2.8 makes a digit `0`..`9` and a sign `0xA` upwards, and the
    /// nibble count is even by construction, so the field ends at the
    /// first octet whose **low** nibble is a sign. A field that never
    /// presents one within the sixteen octets of the widest `fixed` IDL
    /// admits is refused.
    member this.ReadFixed(digits: int, scale: int, count: string) : string =
        if digits < 1 || digits > 31 || scale > digits then
            raise (WireError(sprintf "a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<%d, %d>" digits scale))
        if fixedCountIsValueDecided count then
            let buffer = ResizeArray<int>()
            let mutable sign = -1
            while sign < 0 do
                if buffer.Count >= 32 then
                    raise (WireError "a sign nibble within sixteen octets")
                let octet = int (this.ReadU8())
                buffer.Add(octet >>> 4)
                buffer.Add(octet &&& 0x0F)
                if (octet &&& 0x0F) >= firstSignNibble then
                    sign <- octet &&& 0x0F
            let nibbles = [| for i in 0 .. buffer.Count - 2 -> buffer.[i] |]
            fixedText nibbles sign scale
        else
            let octets = (digits + 2) / 2
            fixedDecimal (this.TakeRaw(1, octets)) digits scale

    member this.ReadBitset(anchor: int) : uint64 =
        match anchor with
        | 1 -> uint64 (this.ReadU8())
        | 2 -> uint64 (this.ReadU16())
        | 4 -> uint64 (this.ReadU32())
        | 8 -> this.ReadU64()
        | other -> raise (WireError(sprintf "a bitset anchor is one, two, four or eight octets, not %d" other))

    member this.ReadDheader() : uint32 = this.ReadU32()

    /// A member header: the id, the must-understand flag, the length
    /// code, and the member body's extent in octets measured from the
    /// position this call leaves the cursor at. `LC5`..`LC7` peek the
    /// body's own leading word without consuming it.
    member this.ReadEmheader() : EmHeaderRead =
        let header = this.ReadU32()
        let id = header &&& 0x0FFFFFFFu
        let mustUnderstand = (header &&& 0x80000000u) <> 0u
        let lengthCode = int ((header >>> 28) &&& 0x7u)
        let extent =
            if lengthCode <= LC3 then
                1u <<< lengthCode
            elif lengthCode = LC4 then
                this.ReadU32()
            else
                let at = pos
                let word = this.ReadU32()
                pos <- at
                if lengthCode = LC5 then 4u + word
                elif lengthCode = LC6 then 4u + 4u * word
                else 4u + 8u * word
        { Id = id
          MustUnderstand = mustUnderstand
          LengthCode = lengthCode
          Extent = extent }

    /// A PL_CDR1 parameter header, either form: the id with its flags
    /// stripped, the must-understand flag, the declared byte length, and
    /// whether the list ends here. The extended form is recognised by the
    /// marker PID_EXTENDED; its must-understand answer comes from bit 30
    /// of the eMemberHeader and never from the marker.
    member this.ReadPid() : PidRead = this.ReadPidAs(Member)

    /// The same header, read in the role the caller is looking for.
    ///
    /// The role is the caller's and not the stream's: §7.4.1.2 gives a
    /// member and an `@optional` member the same four header octets, and
    /// only what the declared length is taken to cover differs. The
    /// shared walk asks for it by name (`read_pid_as`), so this is the
    /// entry point a generated decoder uses and `ReadPid` the one a
    /// hand-written caller does.
    member this.ReadPidAs(role: ParameterRole) : PidRead =
        // §7.4.1.2: a parameter header begins at a four-octet boundary of
        // the enclosing stream. `WritePid` aligns before writing one, and
        // a reader that only aligned to the two octets of the id would
        // find the next header wherever the previous value happened to
        // end.
        this.AlignRead(4)
        let raw = int (this.ReadU16())
        let short = raw &&& 0x3FFF
        if short = PidListEndId then
            this.ReadU16() |> ignore
            { Id = 0u
              MustUnderstand = false
              Length = 0u
              ListEnd = true
              Ignore = false
              ValueStart = pos
              SavedOrigin = origin
              Role = role }
        elif short = PidExtendedId then
            this.ReadU16() |> ignore
            let header = this.ReadU32()
            let length = this.ReadU32()
            let saved = origin
            origin <- pos
            { Id = header &&& 0x0FFFFFFFu
              MustUnderstand = (header &&& emFlagMustUnderstand) <> 0u
              Length = length
              ListEnd = false
              Ignore = false
              ValueStart = pos
              SavedOrigin = saved
              Role = role }
        else
            let length = uint32 (this.ReadU16())
            let saved = origin
            origin <- pos
            { Id = uint32 short
              MustUnderstand = (raw &&& pidFlagMustUnderstand) <> 0
              Length = length
              ListEnd = false
              Ignore = short = PidIgnoreId
              ValueStart = pos
              SavedOrigin = saved
              Role = role }

    /// Moves the cursor past a parameter value read via `header`, and
    /// restores the stream's alignment origin — `zdwire.zig`'s
    /// `finishPidRead`.
    ///
    /// The declared length locates the value's end and the next header
    /// finds itself by aligning, which is what lets one reader take a
    /// framing that rounds the length up to four and one that does not.
    member _.FinishPidRead(header: PidRead) =
        if not header.ListEnd then
            pos <- header.ValueStart + int header.Length
        origin <- header.SavedOrigin

// -- helpers the codec walk names directly -----------------------------------

/// C11 §7.24.4's unsigned-octet order over two UTF-8 strings — the order a
/// `map<string, V>`'s ascending-key rule sorts by. .NET's ordinal
/// `String.CompareOrdinal` is UTF-16 code-unit order, which is not the same
/// order for a code point above the Basic Multilingual Plane; comparing the
/// UTF-8 octets directly is what makes the two agree.
let strcmp (a: string) (b: string) : int =
    let ab = Encoding.UTF8.GetBytes(a)
    let bb = Encoding.UTF8.GetBytes(b)
    let n = min ab.Length bb.Length
    let mutable i = 0
    let mutable result = 0
    while result = 0 && i < n do
        result <- compare ab.[i] bb.[i]
        i <- i + 1
    if result <> 0 then result else compare ab.Length bb.Length

/// `false` before `true` — the ascending-key order's reading of a
/// `boolean` map key's one-octet wire value. F#'s own `<` on `bool`
/// already answers this; the free function exists so the generated codec
/// spells every ordering the same way, through one name.
let boolLess (a: bool) (b: bool) : bool = a < b

/// The value's storage bits, zero-extended into the runtime's 64-bit
/// pattern argument for the Cyclone union overlay
/// (`crates/lang-rules/src/emit/mod.rs`'s `BitPatternSource`).
let bitPatternInt (value: int64) (octets: int) : uint64 =
    let mask =
        match octets with
        | 1 -> 0xFFUL
        | 2 -> 0xFFFFUL
        | 4 -> 0xFFFFFFFFUL
        | 8 -> 0xFFFFFFFFFFFFFFFFUL
        | other -> raise (WireError(sprintf "a bit pattern is one, two, four or eight octets, not %d" other))

    (uint64 value) &&& mask

let bitPatternBool (value: bool) : uint64 = if value then 1UL else 0UL
let bitPatternF32 (value: float32) : uint64 = uint64 (BitConverter.SingleToUInt32Bits(value))
let bitPatternF64 (value: float) : uint64 = BitConverter.DoubleToUInt64Bits(value)

/// Widens a value the walk cannot see the concrete type of — an
/// enumeration sample, a union overlay arm — to the runtime's
/// sixty-four-bit integer holder. `crates/lang-rules/src/emit/zig.rs`'s
/// `asI64`/`asU64` read the same way; this column has one function for
/// both because F#'s `int64`/`uint64` conversions from any narrower
/// integer type are unconditional widenings and never need to branch on
/// signedness the way a Zig `@intCast` does.
let inline asI64 (value: ^T) : int64 = int64 value
let inline asU64 (value: ^T) : uint64 = uint64 value

/// An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.
/// A code point above U+00FF is refused and not truncated — the same
/// refusal `endpoints/fsharp/zerodds.fs`'s `PutChar` makes, so a
/// generated codec and the hand-written SDK answer alike on
/// `primitive-char-above-latin1`.
let checkedChar (codePoint: int) : byte =
    if codePoint < 0 || codePoint > 0xFF then
        raise (WireError(sprintf "expected a character below U+0100, found U+%04X" codePoint))
    byte codePoint

/// An IDL `wchar`: a code point above U+FFFF is refused, width-independent
/// — a four-octet form would zero-extend a code *unit*, not a code point.
/// The answer is a `uint16` and not an `int`, exactly as `checkedChar`'s
/// is a `byte`: the check above is what makes the narrowing total, and
/// the F# materialiser holds a `wchar` member in a `uint16`, so a
/// checker that answered `int` would need a conversion at every place a
/// sample or a default names a wide character.
let checkedWchar (codePoint: int) : uint16 =
    if codePoint < 0 || codePoint > 0xFFFF then
        raise (WireError(sprintf "expected a BMP character, found U+%04X" codePoint))
    uint16 codePoint

/// The UTF-8 octet count of a .NET string, for a `wstring` policy that
/// counts octets in the specification's own width rather than in the
/// wide units some vendors write.
let utf8Octets (s: string) : int = Encoding.UTF8.GetByteCount(s)

/// The number of two-octet units a .NET string's UTF-16 form already is
/// — used unchanged for a `wstring` policy that writes UTF-16 code units
/// natively, since .NET's own string storage already is that.
let utf16Units (s: string) : int = s.Length
