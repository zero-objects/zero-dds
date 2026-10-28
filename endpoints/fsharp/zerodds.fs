// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-F# endpoint SDK (ADR 0013): a from-scratch XCDR wire-core on
// .NET/Mono, byte-identical to the Rust core and the other SDKs. Sync (poll)
// and async (an `async { }` MailboxProcessor agent — the idiomatic F# actor).

module ZeroDDS

open System

type Endian =
    | LE
    | BE

// The XCDR version the stream is written in.
//
// It is a constructor argument and has no default. XTypes 1.3 §7.4.1 puts
// MAXALIGN at eight under XCDR1 and four under XCDR2, so a writer that did not
// know which one it was in would be writing XCDR2 and calling it XCDR — which
// is what this SDK did until the alignment cap became a field: `Align` read
// `min a 4`, and every `long long` in an XCDR1 sample landed four octets early
// with nothing in the interface to say so.
type Encoding =
    /// PLAIN_CDR/PL_CDR — MAXALIGN 8.
    | XCDR1
    /// PLAIN_CDR2/DELIMITED_CDR/PL_CDR2 — MAXALIGN 4.
    | XCDR2

/// The alignment cap this encoding imposes. XTypes 1.3 §7.4.1.
let maxAlignOf (e: Encoding) = match e with XCDR1 -> 8 | XCDR2 -> 4

// The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The code
// occupies bits 28..30 of the EMHEADER and decides how the member's byte length
// is recovered: LC0..LC3 give it from the code alone, LC4..LC7 read it from a
// NEXTINT that follows the header.
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
/// The PID that introduces the twelve-octet parameter header. §7.4.1.2.
let PidExtendedId = 0x3F01
/// The highest member id a short parameter header carries, inclusive.
let PidShortMaxId = 0x3F00
// Bit 14 of a short parameter id. XTypes 1.3 §7.4.1.2.
let private pidFlagMustUnderstand = 0x4000
// Bit 30 of the eMemberHeader inside an extended parameter header — not bit 31,
// which is the implementation-extension flag.
let private emFlagMustUnderstand = 0x40000000u

// The octets an argument of `bits` width may carry.
//
// F#'s `int` is signed and wider than every wire width below 32, so a value
// that fits neither the signed nor the unsigned range of the width cannot be a
// pattern of that width at all. Truncating it is the defect this refuses:
// `PutU16 70000` wrote `7011`, the low sixteen bits of 0x11170, and the frame
// was the right length either way so nothing downstream could tell.
let private pattern (v: int) (bits: int) (call: string) =
    let unsignedMax = (1L <<< bits) - 1L
    let signedMin = -(1L <<< (bits - 1))
    if int64 v > unsignedMax || int64 v < signedMin then
        invalidArg "v" (sprintf "%s: %d is not a %d-bit value (%d..%d)" call v bits signedMin unsignedMax)
    uint32 (int64 v &&& unsignedMax)

/// The packing PutFixed writes, apart so it can be tested at the edges of its
/// range without a Writer.
///
/// `value` is the decimal as the sample wrote it — an optional sign, an integer
/// part and an optional fraction. Everything after that is this function's:
///
///   - the integer part is left-padded and the fraction right-padded to exactly
///     `digits` decimal digits, with the point implied `scale` from the right;
///     the scale itself is not on the wire,
///   - a leading zero nibble is prepended when `digits + 1` is odd, so digits
///     and sign pair up,
///   - two nibbles to the octet, most significant digit first, high nibble
///     first,
///   - the last nibble is the sign: 0xD negative, 0xC otherwise. A negative
///     zero goes out positive, because there is one zero to write,
///   - alignment is one, and no multi-octet integer is written, so the byte
///     order never enters.
///
/// The octet count is therefore ceil((digits + 1) / 2), whatever the value.
let fixedOctets (value: string) (digits: int) (scale: int) : byte[] =
    if digits < 1 || digits > 31 then
        invalidArg "digits" (sprintf "fixed<%d,%d>: a digit count is 1..31" digits scale)
    if scale < 0 || scale > digits then
        invalidArg "scale" (sprintf "fixed<%d,%d>: a scale is 0..digits" digits scale)
    let mutable text = value
    let mutable negative = false
    if text.Length > 0 && (text.[0] = '+' || text.[0] = '-') then
        negative <- text.[0] = '-'
        text <- text.Substring(1)
    let dot = text.IndexOf('.')
    let mutable integral = if dot >= 0 then text.Substring(0, dot) else text
    let mutable fraction = if dot >= 0 then text.Substring(dot + 1) else ""
    if integral = "" && fraction = "" then
        invalidArg "value" (sprintf "fixed<%d,%d>: `%s` has no digits" digits scale value)
    for part in [ integral; fraction ] do
        for ch in part do
            if ch < '0' || ch > '9' then
                invalidArg "value" (sprintf "fixed<%d,%d>: `%s` is not a decimal" digits scale value)
    // Leading zeros carry no value, so they are dropped before the width is
    // checked: `000000.001` is one significant integer digit, not six.
    integral <- integral.TrimStart('0')
    if integral.Length > digits - scale then
        invalidArg "value"
            (sprintf "fixed<%d,%d>: `%s` needs %d integer digits and the type has %d"
                digits scale value integral.Length (digits - scale))
    // A trailing zero in the fraction is padding either way, so it is dropped
    // before the width is checked and re-added below.
    while fraction.Length > scale && fraction.EndsWith("0") do
        fraction <- fraction.Substring(0, fraction.Length - 1)
    if fraction.Length > scale then
        invalidArg "value"
            (sprintf "fixed<%d,%d>: `%s` needs %d fraction digits and the type has %d"
                digits scale value fraction.Length scale)
    let nibbles = ResizeArray<int>()
    if (digits + 1) % 2 = 1 then nibbles.Add(0)
    for _ in 1 .. (digits - scale - integral.Length) do nibbles.Add(0)
    for ch in integral do nibbles.Add(int ch - int '0')
    for ch in fraction do nibbles.Add(int ch - int '0')
    for _ in 1 .. (scale - fraction.Length) do nibbles.Add(0)
    nibbles.Add(if negative && Seq.exists (fun n -> n <> 0) nibbles then 0x0D else 0x0C)
    [| for i in 0 .. (nibbles.Count / 2) - 1 -> byte ((nibbles.[2 * i] <<< 4) ||| nibbles.[2 * i + 1]) |]

/// An open PL_CDR1 parameter header, to be handed back to PidEnd.
type Pid =
    { At: int
      Extended: bool
      SavedOrigin: int }

/// A member header as EmHeaderRead returns it.
type Member =
    { Id: uint32
      MustUnderstand: bool
      LengthCode: int
      Extent: uint32 }

/// A PL_CDR1 parameter header as PidRead returns it.
type Parameter =
    { Id: uint32
      MustUnderstand: bool
      Length: uint32
      ListEnd: bool }

// --- Writer: a byte accumulator; the encoding decides the alignment cap ---

type Writer(endian: Endian, encoding: Encoding) =
    let buf = ResizeArray<byte>()
    let maxAlign = maxAlignOf encoding
    // The position alignment is measured from. Zero for the stream, and moved
    // to the start of a PL_CDR1 parameter's value for the length of that
    // parameter — §7.4.1.2 aligns a parameter's value against its own block,
    // not against the stream. Without it a `double` in a parameter takes four
    // octets of padding and the header declares twelve where Cyclone, RTI and
    // Fast DDS all declare eight.
    let mutable origin = 0

    member _.MaxAlign = maxAlign

    member _.Align(a: int) =
        let cap = min a maxAlign
        let pad = (cap - ((buf.Count - origin) % cap)) % cap
        for _ in 1 .. pad do
            buf.Add(0uy)

    // le: the value already encoded little-endian; reversed for BE.
    member this.Put(a: int, le: byte[]) =
        this.Align(a)
        if endian = BE then buf.AddRange(Array.rev le) else buf.AddRange(le)

    member _.PutU8(v: int) = buf.Add(byte (pattern v 8 "PutU8"))

    /// A `boolean` is the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it: 0
    /// or 1, never any other pattern.
    member _.PutBool(v: bool) = buf.Add(if v then 1uy else 0uy)

    member this.PutU16(v: int) =
        let p = pattern v 16 "PutU16"
        this.Put(2, [| byte p; byte (p >>> 8) |])

    member this.PutU32(v: uint32) =
        this.Put(4, [| byte v; byte (v >>> 8); byte (v >>> 16); byte (v >>> 24) |])

    /// A `long long` at its natural eight-octet alignment, which MaxAlign then
    /// caps: eight under XCDR1 and four under XCDR2. XTypes 1.3 §7.4.1.
    member this.PutU64(v: uint64) =
        this.Put(8, [| for i in 0 .. 7 -> byte (v >>> (8 * i)) |])

    member this.PutF32(v: float32) =
        // GetBytes yields platform-endian bytes; on LE hosts that is our LE form.
        let b = BitConverter.GetBytes(v)
        let le = if BitConverter.IsLittleEndian then b else Array.rev b
        this.Put(4, le)

    /// `double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2 and the
    /// floating type most DDS topics use; it goes out as its IEEE-754 bit
    /// pattern at the same alignment as PutU64.
    member this.PutF64(v: float) =
        let b = BitConverter.GetBytes(v)
        let le = if BitConverter.IsLittleEndian then b else Array.rev b
        this.Put(8, le)

    member _.PutBytes(b: byte[]) = buf.AddRange(b)

    // Wire rule (docs/specs/zerodds-xcdr2-fsharp-1.0.md §"string"): uint32 (UTF-8
    // byte count incl. NUL) + UTF-8 bytes + NUL. The length prefix is the BYTE
    // count of the UTF-8 encoding, not the .NET string length (UTF-16 code units)
    // — multibyte characters occupy more than one byte. Byte-identical to the
    // C/Rust core.
    member this.PutString(s: string) =
        let raw = Text.Encoding.UTF8.GetBytes(s)
        this.PutU32(uint32 (raw.Length + 1))
        buf.AddRange(raw)
        this.PutU8(0)

    member this.PutSeqU8(b: byte[]) =
        this.PutU32(uint32 b.Length)
        buf.AddRange(b)

    /// An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.
    ///
    /// A code point above U+00FF is refused and not truncated. `š` is U+0161 and
    /// its low octet is 0x61, which is `a`: a truncating writer produces a frame
    /// of the right length holding a different, plausible character, and nothing
    /// downstream can tell. The Rust core refuses `primitive-char-above-latin1`
    /// in the same words.
    member _.PutChar(codePoint: int) =
        if codePoint < 0 || codePoint > 0xFF then
            invalidArg "codePoint" (sprintf "expected a character below U+0100, found U+%04X" codePoint)
        buf.Add(byte codePoint)

    /// An IDL `wchar`: one two-octet code unit, 2-aligned, in the stream's byte
    /// order.
    ///
    /// The four policy axes a wide character carries are answered here and not
    /// by the caller: the unit is two octets wide, it goes out in the stream's
    /// byte order, there is no GIOP length octet before it, and no byte-order
    /// mark.
    ///
    /// A code point above U+FFFF is refused. The refusal is width-independent —
    /// a four-octet form would zero-extend a code *unit*, not a code point —
    /// which is how the Rust core refuses `primitive-wchar-above-bmp`.
    member this.PutWchar(codePoint: int) =
        if codePoint < 0 || codePoint > 0xFFFF then
            invalidArg "codePoint" (sprintf "expected a BMP character, found U+%04X" codePoint)
        this.PutU16(codePoint)

    /// An IDL `wstring` from its UTF-16 code units.
    ///
    /// The length word counts **octets** and not code units, there is no
    /// terminator, there is no byte-order mark, and each unit goes out two
    /// octets wide in the stream's own byte order. Those are the four axes, and
    /// they are the binding's answer — the caller hands over units and nothing
    /// else.
    member this.PutWstring(units: int[]) =
        this.PutU32(uint32 (units.Length * 2))
        for unit in units do
            if unit < 0 || unit > 0xFFFF then
                invalidArg "units" (sprintf "a wstring code unit is two octets, found %d" unit)
            this.PutU16(unit)

    /// A `bitset` as its anchor word: the holder the declared bits pack into,
    /// written at the anchor's own width and therefore at the anchor's own
    /// alignment — eight under XCDR1 and four under XCDR2 for an eight-octet
    /// anchor. XTypes 1.3 §7.3.1.4 and §7.4.1.
    ///
    /// `bits` is the packed holder and `anchor` its width in octets, both
    /// properties of the declared type. What this call decides is the pair the
    /// type does not: which write the holder goes out through, and what that
    /// write aligns to.
    member this.PutBitset(bits: uint64, anchor: int) =
        match anchor with
        | 1 -> this.PutU8(int (bits &&& 0xFFUL))
        | 2 -> this.PutU16(int (bits &&& 0xFFFFUL))
        | 4 -> this.PutU32(uint32 (bits &&& 0xFFFFFFFFUL))
        | 8 -> this.PutU64(bits)
        | _ ->
            invalidArg "anchor"
                (sprintf "a bitset anchor is one, two, four or eight octets, not %d" anchor)

    /// An IDL `fixed<digits,scale>` as packed BCD.
    /// CDR (OMG CORBA 3.3) §9.3.2.8 / IDL 4.2 §7.4.1.4.5.
    member this.PutFixed(value: string, digits: int, scale: int) =
        this.PutBytes(fixedOctets value digits scale)

    // --- back-patched framing --------------------------------------------
    //
    // A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same
    // shape: a slot reserved before the body and filled in once the body's
    // length is known. These are the only writes into an already-written
    // position, and they honour the wire byte order like every other write.

    member private _.PatchU32(at: int, v: uint32) =
        let le = [| byte v; byte (v >>> 8); byte (v >>> 16); byte (v >>> 24) |]
        for i in 0 .. 3 do
            buf.[at + i] <- if endian = BE then le.[3 - i] else le.[i]

    member private _.PatchU16(at: int, v: int) =
        let le = [| byte v; byte (v >>> 8) |]
        for i in 0 .. 1 do
            buf.[at + i] <- if endian = BE then le.[1 - i] else le.[i]

    /// Reserves a 4-aligned DHEADER slot and returns the position the body
    /// starts at, to hand back to DHeaderEnd. XTypes 1.3 §7.4.3.5.
    member this.DHeaderBegin() =
        this.PutU32(0u)
        buf.Count

    /// Back-patches the DHEADER with the byte length written since bodyStart.
    /// The DHEADER itself is not counted — §7.4.3.5 measures the content that
    /// follows it.
    member this.DHeaderEnd(bodyStart: int) =
        this.PatchU32(bodyStart - 4, uint32 (buf.Count - bodyStart))

    /// Writes the EMHEADER for one member of a @mutable aggregate and returns
    /// the body-start position. XTypes 1.3 §7.4.3.4.2:
    ///
    ///     EMHEADER = (must_understand &lt;&lt; 31) ||| (length_code &lt;&lt; 28) ||| member_id
    ///
    /// with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
    /// EmHeaderEnd fills it in; for LC0..LC3 the code alone gives the length and
    /// nothing follows; for LC5..LC7 the body's own leading four octets are the
    /// length, so no separate NEXTINT is written and EmHeaderEnd has nothing to
    /// patch.
    member this.EmHeaderBegin(id: uint32, mustUnderstand: bool, lengthCode: int) =
        if id > 0x0FFFFFFFu then
            invalidArg "id" (sprintf "member id %d does not fit the 28 bits of an EMHEADER" id)
        if lengthCode < LC0 || lengthCode > LC7 then
            invalidArg "lengthCode" (sprintf "length code %d is not one of LC0..LC7" lengthCode)
        let mutable header = (uint32 lengthCode <<< 28) ||| id
        if mustUnderstand then header <- header ||| 0x80000000u
        this.PutU32(header)
        if lengthCode = LC4 then this.PutU32(0u)
        buf.Count

    /// Closes the member EmHeaderBegin opened.
    ///
    /// It takes the length code back rather than remembering it, so a caller
    /// that paired an LC4 begin with a differently-coded end gets a refusal
    /// instead of a NEXTINT patched over the first four octets of a member body.
    member this.EmHeaderEnd(bodyStart: int, lengthCode: int) =
        let written = buf.Count - bodyStart
        if lengthCode = LC4 then
            this.PatchU32(bodyStart - 4, uint32 written)
        elif lengthCode <= LC3 then
            // 1, 2, 4, 8 — the code *is* the length, so a body of any other size
            // means the plan and the member disagree. Silence here would put a
            // length on the wire that no octet count backs.
            let want = 1 <<< lengthCode
            if written <> want then
                invalidArg "lengthCode"
                    (sprintf "LC%d frames %d octets and the member wrote %d" lengthCode want written)
        elif written < 4 then
            invalidArg "lengthCode"
                (sprintf "LC%d takes its length from the body's first four octets and the member wrote %d"
                    lengthCode written)

    /// Writes a PL_CDR1 short parameter header. XTypes 1.3 §7.4.1.2: at a
    /// four-octet boundary an `unsigned short` parameter id, then an `unsigned
    /// short` byte length of the value that follows. Both go out in the stream's
    /// byte order.
    ///
    /// The must-understand flag is bit 14 of the id field. Bit 15 is the
    /// implementation-extension flag and is not set here.
    ///
    /// The parameter's value aligns against the block and not against the
    /// stream, so the alignment origin moves for the length of the parameter and
    /// PidEnd restores it.
    member this.PidBegin(pid: int, mustUnderstand: bool) : Pid =
        if pid < 0 || pid > PidShortMaxId then
            invalidArg "pid" (sprintf "member id %d needs the extended parameter header" pid)
        this.Align(4)
        let id = if mustUnderstand then pid ||| pidFlagMustUnderstand else pid
        this.Put(2, [| byte id; byte (id >>> 8) |])
        let at = buf.Count
        buf.Add(0uy)
        buf.Add(0uy)
        let saved = origin
        origin <- buf.Count
        { At = at; Extended = false; SavedOrigin = saved }

    /// Writes the twelve-octet parameter header XTypes 1.3 §7.4.1.2 requires for
    /// a member id above 0x3F00 or a value longer than 65535 octets:
    ///
    ///     [PID_EXTENDED ||| mu : u16][8 : u16][eMemberHeader : u32][length : u32]
    ///
    /// The must-understand flag on the marker is not the member's — it is on the
    /// marker under this framing whatever the member says, and the member's own
    /// flag is bit 30 of the eMemberHeader. Reading it off the marker instead
    /// makes every parameter must-understand, which is what Cyclone's wire looks
    /// like if you do.
    member this.PidExtendedBegin(memberId: uint32, mustUnderstand: bool) : Pid =
        if memberId > 0x0FFFFFFFu then
            invalidArg "memberId"
                (sprintf "member id %d does not fit the 28 bits of an eMemberHeader" memberId)
        this.Align(4)
        let marker = PidExtendedId ||| pidFlagMustUnderstand
        this.Put(2, [| byte marker; byte (marker >>> 8) |])
        this.Put(2, [| 8uy; 0uy |])
        let header =
            if mustUnderstand then memberId ||| emFlagMustUnderstand else memberId
        this.Put(4, [| byte header; byte (header >>> 8); byte (header >>> 16); byte (header >>> 24) |])
        let at = buf.Count
        for _ in 1 .. 4 do buf.Add(0uy)
        let saved = origin
        origin <- buf.Count
        { At = at; Extended = true; SavedOrigin = saved }

    /// Closes the parameter PidBegin or PidExtendedBegin opened.
    ///
    /// The declared length counts the value's octets **and** the padding that
    /// brings the next header back to a four-octet boundary: a reader skips
    /// `length` from the end of the header and has to land on the next
    /// parameter. The padding itself is written after the length is patched, so
    /// the two cannot disagree.
    member this.PidEnd(header: Pid) =
        let width = if header.Extended then 4 else 2
        let mutable length = buf.Count - header.At - width
        // The parameter's own origin is still in force here, so the round-up is
        // measured from the block that the length describes.
        let past = (buf.Count - header.SavedOrigin) % 4
        if past <> 0 then length <- length + (4 - past)
        if header.Extended then
            this.PatchU32(header.At, uint32 length)
        else
            if length > 0xFFFF then
                invalidArg "header"
                    (sprintf "a short parameter header describes at most 65535 octets, not %d" length)
            this.PatchU16(header.At, length)
        origin <- header.SavedOrigin
        this.Align(4)

    /// The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END
    /// with a zero length, four-aligned. XTypes 1.3 §7.4.1.2.
    ///
    /// The must-understand flag is set on the sentinel — 0x7F02 on the wire,
    /// which is what Cyclone and RTI write. Fast DDS writes the bare 0x3F02; the
    /// flag is not the sentinel's meaning either way, and this is the majority
    /// form the Rust column records.
    member this.PidListEnd() =
        this.Align(4)
        let id = PidListEndId ||| pidFlagMustUnderstand
        this.Put(2, [| byte id; byte (id >>> 8) |])
        this.Put(2, [| 0uy; 0uy |])

    member _.Bytes() = buf.ToArray()

// --- Reader: a byte cursor ---

type Reader(data: byte[], endian: Endian, encoding: Encoding) =
    let mutable pos = 0
    let maxAlign = maxAlignOf encoding
    // Mirrors the writer's: a PL_CDR1 parameter's value aligns against its own
    // block.
    let mutable origin = 0

    member _.MaxAlign = maxAlign

    member _.Pos
        with get () = pos
        and set (v) = pos <- v

    member _.Take(a: int, n: int) =
        let cap = min a maxAlign
        let pad = (cap - ((pos - origin) % cap)) % cap
        pos <- pos + pad
        let chunk = Array.sub data pos n
        pos <- pos + n
        if endian = BE then Array.rev chunk else chunk

    member this.GetU32() =
        let b = this.Take(4, 4)
        uint32 b.[0]
        ||| (uint32 b.[1] <<< 8)
        ||| (uint32 b.[2] <<< 16)
        ||| (uint32 b.[3] <<< 24)

    // --- primitives beyond GetU32 — the byte-exact inverse of the Writer ---
    member _.GetU8() =
        let b = data.[pos]
        pos <- pos + 1
        b

    member this.GetU16() =
        let b = this.Take(2, 2)
        uint16 b.[0] ||| (uint16 b.[1] <<< 8)

    /// A `long long` at its natural eight-octet alignment, which MaxAlign then
    /// caps: eight under XCDR1 and four under XCDR2.
    ///
    /// It passed 4 here while the writer passed 8, which under XCDR1 read the
    /// four octets of padding as the value's low half and returned a different
    /// number with no error anywhere.
    member this.GetU64() =
        let b = this.Take(8, 8)
        let mutable v = 0UL
        for i in 0 .. 7 do
            v <- v ||| (uint64 b.[i] <<< (8 * i))
        v

    member this.GetF32() =
        let le = this.Take(4, 4)
        let hb = if BitConverter.IsLittleEndian then le else Array.rev le
        BitConverter.ToSingle(hb, 0)

    // Inverse of PutString: read n bytes (n incl. the NUL), decode the first
    // n-1 as UTF-8.
    member this.GetString() =
        let n = int (this.GetU32())
        let s = Text.Encoding.UTF8.GetString(data, pos, n - 1)
        pos <- pos + n
        s

    member this.GetSeqU8() =
        let n = int (this.GetU32())
        let b = Array.sub data pos n
        pos <- pos + n
        b

    member this.GetF64() =
        let le = this.Take(8, 8)
        let hb = if BitConverter.IsLittleEndian then le else Array.rev le
        BitConverter.ToDouble(hb, 0)

    member this.GetBool() = this.GetU8() <> 0uy

    /// The one octet an IDL `char` occupies.
    member this.GetChar() = int (this.GetU8())

    /// One two-octet `wchar` code unit.
    member this.GetWchar() = int (this.GetU16())

    /// A `wstring` back into its UTF-16 code units. The length word counts
    /// octets, so the unit count is half of it.
    member this.GetWstring() =
        let octets = int (this.GetU32())
        [| for _ in 1 .. (octets / 2) -> int (this.GetU16()) |]

    /// A `bitset` anchor of the given width back into its holder word.
    member this.GetBitset(anchor: int) =
        match anchor with
        | 1 -> uint64 (this.GetU8())
        | 2 -> uint64 (this.GetU16())
        | 4 -> uint64 (this.GetU32())
        | 8 -> this.GetU64()
        | _ ->
            invalidArg "anchor"
                (sprintf "a bitset anchor is one, two, four or eight octets, not %d" anchor)

    /// A `fixed<digits,scale>` back into the decimal it was written from: a
    /// leading `-` when the sign nibble is 0xD, `digits - scale` integer digits,
    /// and a `.` with `scale` fraction digits where the scale is not zero.
    ///
    /// The scale is not on the wire, so it is an argument here for the same
    /// reason it is one in PutFixed.
    member this.GetFixed(digits: int, scale: int) =
        let octets = (digits + 2) / 2
        let nibbles = ResizeArray<int>()
        for _ in 1 .. octets do
            let b = int (this.GetU8())
            nibbles.Add(b >>> 4)
            nibbles.Add(b &&& 0x0F)
        let sign = nibbles.[nibbles.Count - 1]
        let out = Text.StringBuilder()
        if sign = 0x0D then out.Append('-') |> ignore
        for i in 0 .. digits - 1 do
            if scale <> 0 && i = digits - scale then out.Append('.') |> ignore
            out.Append(char (int '0' + nibbles.[nibbles.Count - 1 - digits + i])) |> ignore
        out.ToString()

    /// Reads a DHEADER and returns the announced body byte length.
    member this.DHeaderRead() = this.GetU32()

    /// A member header: the member id, the must-understand flag, the length
    /// code, and the member body's extent in octets measured from the position
    /// this call leaves the cursor at.
    ///
    /// LC0..LC3 take the extent from the code. LC4 consumes the NEXTINT that
    /// follows the header, and the extent is that word. LC5..LC7 do not consume
    /// anything: the next four octets are the member's own leading word and
    /// belong to the body, so they are peeked and the extent is 4 + the word for
    /// LC5, 4 + 4×word for LC6 and 4 + 8×word for LC7 — the last two count
    /// elements, not octets.
    member this.EmHeaderRead() : Member =
        let header = this.GetU32()
        let id = header &&& 0x0FFFFFFFu
        let mustUnderstand = (header &&& 0x80000000u) <> 0u
        let lengthCode = int ((header >>> 28) &&& 0x7u)
        let extent =
            if lengthCode <= LC3 then
                1u <<< lengthCode
            elif lengthCode = LC4 then
                this.GetU32()
            else
                let at = pos
                let word = this.GetU32()
                pos <- at
                if lengthCode = LC5 then 4u + word
                elif lengthCode = LC6 then 4u + 4u * word
                else 4u + 8u * word
        { Id = id
          MustUnderstand = mustUnderstand
          LengthCode = lengthCode
          Extent = extent }

    /// A PL_CDR1 parameter header in either form: the member id with its flags
    /// stripped, the member's must-understand flag, the declared byte length,
    /// and whether the list ends here.
    ///
    /// The extended form is recognised by the marker PID_EXTENDED, and the
    /// must-understand answer then comes from bit 30 of the eMemberHeader and
    /// never from the marker — the marker carries the flag under this framing
    /// whatever the member says.
    ///
    /// The value's alignment origin is moved to the position this call leaves,
    /// and PidSkip restores it.
    member this.PidRead() : Parameter =
        let raw = int (this.GetU16())
        let short = raw &&& 0x3FFF
        if short = PidListEndId then
            this.GetU16() |> ignore
            { Id = 0u; MustUnderstand = false; Length = 0u; ListEnd = true }
        elif short = PidExtendedId then
            this.GetU16() |> ignore // the slength, always 8
            let header = this.GetU32()
            let length = this.GetU32()
            origin <- pos
            { Id = header &&& 0x0FFFFFFFu
              MustUnderstand = (header &&& emFlagMustUnderstand) <> 0u
              Length = length
              ListEnd = false }
        else
            let length = uint32 (this.GetU16())
            origin <- pos
            { Id = uint32 short
              MustUnderstand = (raw &&& pidFlagMustUnderstand) <> 0
              Length = length
              ListEnd = false }

    /// Moves the cursor past a parameter value of `length` octets from
    /// `bodyStart` and restores the stream's alignment origin.
    member _.PidSkip(bodyStart: int, length: uint32) =
        pos <- bodyStart + int length
        origin <- 0
        while pos % 4 <> 0 do
            pos <- pos + 1

// --- XRCE WRITE_DATA framing ---

let SessionNoKey = 0x80uy
let StreamBestEffort = 0x01uy

// E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
// DDS-XRCE 1.0 Sec 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would additionally
// require a SampleInfo in the body, which this profile does not carry.
let XrceWriteFlags = 0x01uy

/// Wire size of the BaseObjectRequest that precedes the sample (Sec 7.7.8).
let XrceBaseSize = 4

/// OBJK_DATAWRITER -- the ObjectKind of a DataWriter (Sec 7.2.2).
let ObjkDataWriter = 0x05

/// An ObjectId (Sec 7.2.1) from a 12-bit raw id and a 4-bit ObjectKind.
let xrceObjectId (rawId: int) (kind: int) = ((rawId &&& 0x0FFF) <<< 4) ||| (kind &&& 0x0F)

// The 16-bit submessage_length field cannot describe a sample larger than
// 0xFFFF, so a sample above it has no WRITE_DATA form: refuse rather than
// truncate the length while appending the full payload (the C SDK's
// `zdw_xrce_write_frame` rejects the same way).
//
// `byte[] option` and not an empty array. This returned `Array.empty` and the
// comment claimed the caller saw the refusal; `Client.Write` never looked, so
// `writeFrame 0x80uy 0x01uy 1 (Array.zeroCreate 70000)` had `.Length = 0` and
// went out as an empty datagram with no error anywhere. `None` is a refusal a
// caller cannot spend without unwrapping it, which is what the comment always
// said was happening.
let writeFrame
    (session: byte)
    (stream: byte)
    (seq: int)
    (objectId: int)
    (sample: byte[])
    : byte[] option =
    let n = XrceBaseSize + sample.Length

    if n > 0xFFFF then
        None
    else

    // 8-byte header, then the BaseObjectRequest (Sec 7.7.8: request_id then
    // object_id, each two octets most significant first), then the sample.
    // `objectId` names the target DataWriter -- without it no XRCE agent can
    // route the sample. The request_id is the message's own `seq`, which
    // Sec 7.7.8 only requires to be unique per stream.
    let hdr =
        [| session
           stream
           byte (seq &&& 0xff)
           byte ((seq >>> 8) &&& 0xff)
           0x07uy
           XrceWriteFlags
           byte (n &&& 0xff)
           byte ((n >>> 8) &&& 0xff)
           byte ((seq >>> 8) &&& 0xff)
           byte (seq &&& 0xff)
           byte ((objectId >>> 8) &&& 0xff)
           byte (objectId &&& 0xff) |]

    Some(Array.append hdr sample)

// Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo, len_hi]
// then the sample body. The 16-bit little-endian submessage_length (bytes 6..7)
// bounds the body exactly: Array.sub frame 8 smLen, never (frame.Length - 8) to
// the end, so trailing padding or an appended submessage is not folded into the
// sample. Returns None for a short header, a wrong submessage id, or a declared
// length that runs past the datagram (truncation / wrong length).
//
// Direction (DDS-XRCE 8.3.5): WRITE_DATA (0x07) is client->agent — what this
// endpoint SENDS — and DATA (0x09) is agent->client, which is the only one a
// client receives from a real hub. Both are accepted, as the C, Kotlin, Node
// and Swift SDKs accept both: 0x09 for hub traffic and 0x07 for loopback.
//
// This used to test `frame.[4] = 0x07uy` alone, so every DATA frame a hub sent
// returned None and `Client.Poll` reported it as "nothing arrived". Measured
// against the other three on 2026-08-18 with one 0x09 frame carrying one octet:
// Kotlin, Node and Swift returned the octet, F# returned None.
let readFrame (frame: byte[]) =
    if frame.Length >= 8 + XrceBaseSize && (frame.[4] = 0x07uy || frame.[4] = 0x09uy) then
        let smLen = int frame.[6] ||| (int frame.[7] <<< 8)

        if smLen < XrceBaseSize || 8 + smLen > frame.Length then
            None
        else
            Some(Array.sub frame (8 + XrceBaseSize) (smLen - XrceBaseSize))
    else
        None

/// `(request_id, object_id)` of a data frame (Sec 7.7.8), or None under the
/// same rules as `readFrame`.
let frameBase (frame: byte[]) =
    match readFrame frame with
    | None -> None
    | Some _ ->
        Some(
            (int frame.[8] <<< 8) ||| int frame.[9],
            (int frame.[10] <<< 8) ||| int frame.[11]
        )

// --- transport ---

// NoComparison/NoEquality: the record holds functions, which satisfy
// neither constraint; saying so is what lets the SDK compile at
// warning level 5 with warnings as errors (FS1178).
[<NoComparison; NoEquality>]
type Transport =
    { Deliver: byte[] -> unit
      Receive: unit -> byte[] option }

// --- sync client ---

/// `objectId` names the target DataWriter in every frame's BaseObjectRequest
/// (Sec 7.7.8).
type Client(transport: Transport, objectId: int) =
    let mutable seq = 1

    // A sample the submessage_length cannot describe raises here rather than
    // going out as a short datagram. The sequence is not advanced: nothing was
    // written, so the next call carries the number this one did not spend.
    member _.Write(sample: byte[]) =
        match writeFrame SessionNoKey StreamBestEffort seq objectId sample with
        | None ->
            invalidArg
                "sample"
                (sprintf
                    "%d octets: the 16-bit XRCE submessage_length describes at most %d"
                    sample.Length
                    0xFFFF)
        | Some frame ->
            transport.Deliver(frame)
            seq <- (seq + 1) &&& 0xffff

    // One non-blocking receive: returns the sample body, or None.
    member _.Poll() =
        match transport.Receive() with
        | None -> None
        | Some frame -> readFrame frame

// --- async reader: an async{} MailboxProcessor agent (idiomatic F#) ---

[<NoComparison>]
type private ReaderMsg = Next of AsyncReplyChannel<byte[]>

type AsyncReader(transport: Transport) =
    let agent =
        MailboxProcessor<ReaderMsg>.Start(fun inbox ->
            let rec loop () =
                async {
                    let! (Next reply) = inbox.Receive()
                    // cooperatively poll until a decoded sample is available
                    let rec pull () =
                        async {
                            match transport.Receive() with
                            | Some frame ->
                                match readFrame frame with
                                | Some body -> reply.Reply(body)
                                | None -> return! pull ()
                            | None ->
                                do! Async.Sleep(1)
                                return! pull ()
                        }

                    do! pull ()
                    return! loop ()
                }

            loop ())

    member _.RecvAsync() : Async<byte[]> = agent.PostAndAsyncReply(fun rc -> Next rc)
    member this.Recv() = this.RecvAsync() |> Async.RunSynchronously

// In-memory FIFO transport for tests + example.
let memTransport () =
    let q = System.Collections.Generic.Queue<byte[]>()
    let gate = obj ()

    { Deliver = fun frame -> lock gate (fun () -> q.Enqueue(Array.copy frame))
      Receive = fun () -> lock gate (fun () -> if q.Count > 0 then Some(q.Dequeue()) else None) }
