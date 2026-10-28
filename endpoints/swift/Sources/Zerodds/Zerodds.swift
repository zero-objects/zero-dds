// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-Swift endpoint SDK (ADR 0013): a from-scratch XCDR wire-core, no
// C shim, byte-identical to the Rust core and the other SDKs. Sync (poll) and
// async (an AsyncStream — the idiomatic Swift concurrency model).

import Foundation

public enum Endianness: Sendable { case little, big }

/// The XCDR version the stream is written in.
///
/// It is a constructor argument and has no default. XTypes 1.3 §7.4.1 puts
/// MAXALIGN at eight under XCDR1 and four under XCDR2, so a writer that did not
/// know which one it was in would be writing XCDR2 and calling it XCDR — which
/// is what this SDK did until the alignment cap became a field: `align` read
/// `a > 4 ? 4 : a`, and every `long long` in an XCDR1 sample landed four octets
/// early with nothing in the interface to say so.
public enum Encoding: Sendable {
    /// PLAIN_CDR/PL_CDR — MAXALIGN 8.
    case xcdr1
    /// PLAIN_CDR2/DELIMITED_CDR/PL_CDR2 — MAXALIGN 4.
    case xcdr2

    /// The alignment cap this encoding imposes. XTypes 1.3 §7.4.1.
    public var maxAlign: Int { self == .xcdr1 ? 8 : 4 }
}

/// The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The code
/// occupies bits 28..30 of the EMHEADER and decides how the member's byte length
/// is recovered: LC0..LC3 give it from the code alone, LC4..LC7 read it from a
/// NEXTINT that follows the header.
public enum LengthCode: Int, Sendable, CaseIterable {
    /// The member body is one octet.
    case lc0 = 0
    /// Two octets.
    case lc1 = 1
    /// Four octets.
    case lc2 = 2
    /// Eight octets.
    case lc3 = 3
    /// A NEXTINT holding the body's byte length follows the EMHEADER.
    case lc4 = 4
    /// The body's first four octets are themselves the length, and they are
    /// part of the body. NEXTINT is not written a second time.
    case lc5 = 5
    /// As LC5, and the length counts four-octet elements.
    case lc6 = 6
    /// As LC5, and the length counts eight-octet elements.
    case lc7 = 7
}

/// PID_LIST_END. XTypes 1.3 §7.4.1.2 Table 31.
public let pidListEndID: UInt16 = 0x3F02
/// The PID that introduces the twelve-octet parameter header. §7.4.1.2.
public let pidExtendedID: UInt16 = 0x3F01
/// The highest member id a short parameter header carries, inclusive.
public let pidShortMaxID: UInt16 = 0x3F00
// Bit 14 of a short parameter id. XTypes 1.3 §7.4.1.2.
let pidFlagMustUnderstand: UInt16 = 0x4000
// Bit 30 of the eMemberHeader inside an extended parameter header — not bit 31,
// which is the implementation-extension flag.
let emFlagMustUnderstand: UInt32 = 0x4000_0000

/// What a write refuses rather than truncates.
///
/// Swift's integer types already carry the wire widths — `putU16` takes a
/// `UInt16`, so `putU16(70000)` is a compile error and not a wrong frame — which
/// is why this SDK needs no range check on its scalar writes and the three
/// others do. What is left are the arguments no type constrains: a code point, a
/// member id, an anchor width and a decimal's digits.
public enum WireError: Error, CustomStringConvertible {
    case outOfRange(String)

    public var description: String {
        switch self {
        case .outOfRange(let text): return text
        }
    }
}

// --- Writer: a byte buffer + endian; the encoding decides the alignment cap ---

public struct Writer {
    public private(set) var buf: [UInt8] = []
    let endian: Endianness
    let encoding: Encoding
    /// The alignment cap this encoding imposes. XTypes 1.3 §7.4.1.
    public var maxAlign: Int { encoding.maxAlign }
    // The position alignment is measured from. Zero for the stream, and moved to
    // the start of a PL_CDR1 parameter's value for the length of that parameter
    // — §7.4.1.2 aligns a parameter's value against its own block, not against
    // the stream. Without it a `double` in a parameter takes four octets of
    // padding and the header declares twelve where Cyclone, RTI and Fast DDS all
    // declare eight.
    var origin: Int = 0

    public init(_ endian: Endianness, _ encoding: Encoding) {
        self.endian = endian
        self.encoding = encoding
    }

    mutating func align(_ a: Int) {
        let cap = a > maxAlign ? maxAlign : a
        while (buf.count - origin) % cap != 0 { buf.append(0) }
    }
    // le: the value already encoded little-endian; reversed for big.
    mutating func putLE(_ a: Int, _ le: [UInt8]) {
        align(a)
        buf.append(contentsOf: endian == .big ? le.reversed() : le)
    }
    public mutating func putU8(_ v: UInt8) { buf.append(v) }
    public mutating func putBool(_ v: Bool) { buf.append(v ? 1 : 0) }
    public mutating func putU16(_ v: UInt16) {
        putLE(2, [UInt8(v & 0xff), UInt8((v >> 8) & 0xff)])
    }
    public mutating func putU32(_ v: UInt32) {
        putLE(4, [UInt8(v & 0xff), UInt8((v >> 8) & 0xff), UInt8((v >> 16) & 0xff), UInt8((v >> 24) & 0xff)])
    }
    /// A `long long` at its natural eight-octet alignment, which maxAlign then
    /// caps: eight under XCDR1 and four under XCDR2. XTypes 1.3 §7.4.1.
    public mutating func putU64(_ v: UInt64) {
        var le = [UInt8]()
        for i in 0..<8 { le.append(UInt8((v >> (8 * UInt64(i))) & 0xff)) }
        putLE(8, le)
    }
    public mutating func putF32(_ v: Float) { putU32(v.bitPattern) }
    /// `double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2 and the
    /// floating type most DDS topics use; it goes out as its IEEE-754 bit
    /// pattern at the same alignment as putU64.
    public mutating func putF64(_ v: Double) { putU64(v.bitPattern) }
    public mutating func putBytes(_ b: [UInt8]) { buf.append(contentsOf: b) }
    public mutating func putString(_ s: String) {
        let bytes = Array(s.utf8)
        putU32(UInt32(bytes.count + 1))
        putBytes(bytes)
        putU8(0)
    }
    public mutating func putSeqU8(_ b: [UInt8]) {
        putU32(UInt32(b.count))
        putBytes(b)
    }

    /// An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.
    ///
    /// A code point above U+00FF is refused and not truncated. `š` is U+0161 and
    /// its low octet is 0x61, which is `a`: a truncating writer produces a frame
    /// of the right length holding a different, plausible character, and nothing
    /// downstream can tell. The Rust core refuses `primitive-char-above-latin1`
    /// in the same words.
    public mutating func putChar(_ codePoint: UInt32) throws {
        guard codePoint <= 0xFF else {
            throw WireError.outOfRange(
                String(format: "expected a character below U+0100, found U+%04X", codePoint))
        }
        buf.append(UInt8(codePoint))
    }

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
    public mutating func putWchar(_ codePoint: UInt32) throws {
        guard codePoint <= 0xFFFF else {
            throw WireError.outOfRange(
                String(format: "expected a BMP character, found U+%04X", codePoint))
        }
        putU16(UInt16(codePoint))
    }

    /// An IDL `wstring` from its UTF-16 code units.
    ///
    /// The length word counts **octets** and not code units, there is no
    /// terminator, there is no byte-order mark, and each unit goes out two
    /// octets wide in the stream's own byte order. Those are the four axes, and
    /// they are the binding's answer — the caller hands over units and nothing
    /// else.
    public mutating func putWstring(_ units: [UInt16]) {
        putU32(UInt32(units.count * 2))
        for unit in units { putU16(unit) }
    }

    /// A `bitset` as its anchor word: the holder the declared bits pack into,
    /// written at the anchor's own width and therefore at the anchor's own
    /// alignment — eight under XCDR1 and four under XCDR2 for an eight-octet
    /// anchor. XTypes 1.3 §7.3.1.4 and §7.4.1.
    ///
    /// `bits` is the packed holder and `anchor` its width in octets, both
    /// properties of the declared type. What this call decides is the pair the
    /// type does not: which write the holder goes out through, and what that
    /// write aligns to.
    public mutating func putBitset(_ bits: UInt64, anchor: Int) throws {
        switch anchor {
        case 1: putU8(UInt8(bits & 0xff))
        case 2: putU16(UInt16(bits & 0xffff))
        case 4: putU32(UInt32(bits & 0xffff_ffff))
        case 8: putU64(bits)
        default:
            throw WireError.outOfRange(
                "a bitset anchor is one, two, four or eight octets, not \(anchor)")
        }
    }

    /// An IDL `fixed<digits,scale>` as packed BCD.
    /// CDR (OMG CORBA 3.3) §9.3.2.8 / IDL 4.2 §7.4.1.4.5.
    public mutating func putFixed(_ value: String, digits: Int, scale: Int) throws {
        putBytes(try fixedOctets(value, digits: digits, scale: scale))
    }

    // --- back-patched framing ---------------------------------------------
    //
    // A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same
    // shape: a slot reserved before the body and filled in once the body's
    // length is known. These are the only writes into an already-written
    // position, and they honour the wire byte order like every other write.

    mutating func patchU32(_ at: Int, _ v: UInt32) {
        let le: [UInt8] = [
            UInt8(v & 0xff), UInt8((v >> 8) & 0xff), UInt8((v >> 16) & 0xff), UInt8((v >> 24) & 0xff),
        ]
        for i in 0..<4 { buf[at + i] = endian == .big ? le[3 - i] : le[i] }
    }

    mutating func patchU16(_ at: Int, _ v: UInt16) {
        let le: [UInt8] = [UInt8(v & 0xff), UInt8((v >> 8) & 0xff)]
        for i in 0..<2 { buf[at + i] = endian == .big ? le[1 - i] : le[i] }
    }

    /// Reserves a 4-aligned DHEADER slot and returns the position the body
    /// starts at, to hand back to `dheaderEnd`. XTypes 1.3 §7.4.3.5.
    public mutating func dheaderBegin() -> Int {
        putU32(0)
        return buf.count
    }

    /// Back-patches the DHEADER with the byte length written since `bodyStart`.
    /// The DHEADER itself is not counted — §7.4.3.5 measures the content that
    /// follows it.
    public mutating func dheaderEnd(_ bodyStart: Int) {
        patchU32(bodyStart - 4, UInt32(buf.count - bodyStart))
    }

    /// Writes the EMHEADER for one member of a `@mutable` aggregate and returns
    /// the body-start position. XTypes 1.3 §7.4.3.4.2:
    ///
    ///     EMHEADER = (must_understand << 31) | (length_code << 28) | member_id
    ///
    /// with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
    /// `emheaderEnd` fills it in; for LC0..LC3 the code alone gives the length
    /// and nothing follows; for LC5..LC7 the body's own leading four octets are
    /// the length, so no separate NEXTINT is written and `emheaderEnd` has
    /// nothing to patch.
    public mutating func emheaderBegin(
        id: UInt32, mustUnderstand: Bool, lengthCode: LengthCode
    ) throws -> Int {
        guard id <= 0x0FFF_FFFF else {
            throw WireError.outOfRange("member id \(id) does not fit the 28 bits of an EMHEADER")
        }
        var header = UInt32(lengthCode.rawValue) << 28 | id
        if mustUnderstand { header |= 1 << 31 }
        putU32(header)
        if lengthCode == .lc4 { putU32(0) }
        return buf.count
    }

    /// Closes the member `emheaderBegin` opened.
    ///
    /// It takes the length code back rather than remembering it, so a caller
    /// that paired an LC4 begin with a differently-coded end gets a refusal
    /// instead of a NEXTINT patched over the first four octets of a member body.
    public mutating func emheaderEnd(_ bodyStart: Int, lengthCode: LengthCode) throws {
        let written = buf.count - bodyStart
        switch lengthCode {
        case .lc4:
            patchU32(bodyStart - 4, UInt32(written))
        case .lc0, .lc1, .lc2, .lc3:
            // 1, 2, 4, 8 — the code *is* the length, so a body of any other size
            // means the plan and the member disagree. Silence here would put a
            // length on the wire that no octet count backs.
            let want = 1 << lengthCode.rawValue
            guard written == want else {
                throw WireError.outOfRange(
                    "LC\(lengthCode.rawValue) frames \(want) octets and the member wrote \(written)")
            }
        case .lc5, .lc6, .lc7:
            guard written >= 4 else {
                throw WireError.outOfRange(
                    "LC\(lengthCode.rawValue) takes its length from the body's first four octets "
                        + "and the member wrote \(written)")
            }
        }
    }

    /// An open PL_CDR1 parameter header, to be handed back to `pidEnd`.
    public struct Pid {
        let at: Int
        let extended: Bool
        let savedOrigin: Int
    }

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
    /// `pidEnd` restores it.
    public mutating func pidBegin(_ pid: UInt16, mustUnderstand: Bool) throws -> Pid {
        guard pid <= pidShortMaxID else {
            throw WireError.outOfRange("member id \(pid) needs the extended parameter header")
        }
        align(4)
        let id = mustUnderstand ? pid | pidFlagMustUnderstand : pid
        putLE(2, [UInt8(id & 0xff), UInt8((id >> 8) & 0xff)])
        let at = buf.count
        buf.append(contentsOf: [0, 0])
        let saved = origin
        origin = buf.count
        return Pid(at: at, extended: false, savedOrigin: saved)
    }

    /// Writes the twelve-octet parameter header XTypes 1.3 §7.4.1.2 requires for
    /// a member id above 0x3F00 or a value longer than 65535 octets:
    ///
    ///     [PID_EXTENDED | mu : u16][8 : u16][eMemberHeader : u32][length : u32]
    ///
    /// The must-understand flag on the marker is not the member's — it is on the
    /// marker under this framing whatever the member says, and the member's own
    /// flag is bit 30 of the eMemberHeader. Reading it off the marker instead
    /// makes every parameter must-understand, which is what Cyclone's wire looks
    /// like if you do.
    public mutating func pidExtendedBegin(_ memberID: UInt32, mustUnderstand: Bool) throws -> Pid {
        guard memberID <= 0x0FFF_FFFF else {
            throw WireError.outOfRange(
                "member id \(memberID) does not fit the 28 bits of an eMemberHeader")
        }
        align(4)
        let marker = pidExtendedID | pidFlagMustUnderstand
        putLE(2, [UInt8(marker & 0xff), UInt8((marker >> 8) & 0xff)])
        putLE(2, [8, 0])
        let header = mustUnderstand ? memberID | emFlagMustUnderstand : memberID
        putLE(4, [
            UInt8(header & 0xff), UInt8((header >> 8) & 0xff),
            UInt8((header >> 16) & 0xff), UInt8((header >> 24) & 0xff),
        ])
        let at = buf.count
        buf.append(contentsOf: [0, 0, 0, 0])
        let saved = origin
        origin = buf.count
        return Pid(at: at, extended: true, savedOrigin: saved)
    }

    /// Closes the parameter `pidBegin` or `pidExtendedBegin` opened.
    ///
    /// The declared length counts the value's octets **and** the padding that
    /// brings the next header back to a four-octet boundary: a reader skips
    /// `length` from the end of the header and has to land on the next
    /// parameter. The padding itself is written after the length is patched, so
    /// the two cannot disagree.
    public mutating func pidEnd(_ header: Pid) throws {
        let width = header.extended ? 4 : 2
        var length = buf.count - header.at - width
        // The parameter's own origin is still in force here, so the round-up is
        // measured from the block that the length describes.
        let past = (buf.count - header.savedOrigin) % 4
        if past != 0 { length += 4 - past }
        if header.extended {
            patchU32(header.at, UInt32(length))
        } else {
            guard length <= 0xFFFF else {
                throw WireError.outOfRange(
                    "a short parameter header describes at most 65535 octets, not \(length)")
            }
            patchU16(header.at, UInt16(length))
        }
        origin = header.savedOrigin
        align(4)
    }

    /// The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END
    /// with a zero length, four-aligned. XTypes 1.3 §7.4.1.2.
    ///
    /// The must-understand flag is set on the sentinel — 0x7F02 on the wire,
    /// which is what Cyclone and RTI write. Fast DDS writes the bare 0x3F02; the
    /// flag is not the sentinel's meaning either way, and this is the majority
    /// form the Rust column records.
    public mutating func pidListEnd() {
        align(4)
        let id = pidListEndID | pidFlagMustUnderstand
        putLE(2, [UInt8(id & 0xff), UInt8((id >> 8) & 0xff)])
        putLE(2, [0, 0])
    }

    public func bytes() -> [UInt8] { buf }
}

/// The packing `putFixed` writes, apart so it can be tested at the edges of its
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
///   - the last nibble is the sign: 0xD negative, 0xC otherwise. A negative zero
///     goes out positive, because there is one zero to write,
///   - alignment is one, and no multi-octet integer is written, so the byte
///     order never enters.
///
/// The octet count is therefore ceil((digits + 1) / 2), whatever the value.
public func fixedOctets(_ value: String, digits: Int, scale: Int) throws -> [UInt8] {
    guard digits >= 1, digits <= 31 else {
        throw WireError.outOfRange("fixed<\(digits),\(scale)>: a digit count is 1..31")
    }
    guard scale >= 0, scale <= digits else {
        throw WireError.outOfRange("fixed<\(digits),\(scale)>: a scale is 0..digits")
    }
    var text = Substring(value)
    var negative = false
    if let first = text.first, first == "+" || first == "-" {
        negative = first == "-"
        text = text.dropFirst()
    }
    var integral = text
    var fraction = Substring("")
    if let dot = text.firstIndex(of: ".") {
        integral = text[text.startIndex..<dot]
        fraction = text[text.index(after: dot)...]
    }
    guard !integral.isEmpty || !fraction.isEmpty else {
        throw WireError.outOfRange("fixed<\(digits),\(scale)>: `\(value)` has no digits")
    }
    for part in [integral, fraction] {
        for ch in part where !ch.isASCII || !ch.isNumber {
            throw WireError.outOfRange("fixed<\(digits),\(scale)>: `\(value)` is not a decimal")
        }
    }
    // Leading zeros carry no value, so they are dropped before the width is
    // checked: `000000.001` is one significant integer digit, not six.
    while integral.first == "0" { integral = integral.dropFirst() }
    guard integral.count <= digits - scale else {
        throw WireError.outOfRange(
            "fixed<\(digits),\(scale)>: `\(value)` needs \(integral.count) integer digits "
                + "and the type has \(digits - scale)")
    }
    // A trailing zero in the fraction is padding either way, so it is dropped
    // before the width is checked and re-added below.
    while fraction.count > scale && fraction.last == "0" { fraction = fraction.dropLast() }
    guard fraction.count <= scale else {
        throw WireError.outOfRange(
            "fixed<\(digits),\(scale)>: `\(value)` needs \(fraction.count) fraction digits "
                + "and the type has \(scale)")
    }
    var nibbles: [UInt8] = []
    if (digits + 1) % 2 == 1 { nibbles.append(0) }
    for _ in 0..<(digits - scale - integral.count) { nibbles.append(0) }
    for ch in integral { nibbles.append(UInt8(ch.wholeNumberValue!)) }
    for ch in fraction { nibbles.append(UInt8(ch.wholeNumberValue!)) }
    for _ in 0..<(scale - fraction.count) { nibbles.append(0) }
    nibbles.append(negative && nibbles.contains(where: { $0 != 0 }) ? 0x0D : 0x0C)
    var out: [UInt8] = []
    var i = 0
    while i + 1 < nibbles.count {
        out.append(nibbles[i] << 4 | nibbles[i + 1])
        i += 2
    }
    return out
}

// --- Reader: a byte cursor ---

public struct Reader {
    let data: [UInt8]
    public var pos: Int = 0
    let endian: Endianness
    let encoding: Encoding
    /// The same cap the writer used, or the fields land elsewhere.
    public var maxAlign: Int { encoding.maxAlign }
    // Mirrors the writer's: a PL_CDR1 parameter's value aligns against its own
    // block.
    var origin: Int = 0

    public init(_ data: [UInt8], _ endian: Endianness, _ encoding: Encoding) {
        self.data = data
        self.endian = endian
        self.encoding = encoding
    }

    mutating func getLE(_ a: Int, _ n: Int) -> [UInt8] {
        let cap = a > maxAlign ? maxAlign : a
        while (pos - origin) % cap != 0 { pos += 1 }
        var out = Array(data[pos..<pos + n])
        pos += n
        if endian == .big { out.reverse() }
        return out
    }
    public mutating func getU32() -> UInt32 {
        let b = getLE(4, 4)
        return UInt32(b[0]) | (UInt32(b[1]) << 8) | (UInt32(b[2]) << 16) | (UInt32(b[3]) << 24)
    }

    // --- primitives beyond getU32 — the byte-exact inverse of the Writer ---

    public mutating func getU8() -> UInt8 {
        let v = data[pos]
        pos += 1
        return v
    }
    public mutating func getU16() -> UInt16 {
        let b = getLE(2, 2)
        return UInt16(b[0]) | (UInt16(b[1]) << 8)
    }
    /// A `long long` at its natural eight-octet alignment, which maxAlign then
    /// caps: eight under XCDR1 and four under XCDR2.
    ///
    /// It passed 4 here while the writer passed 8, which under XCDR1 read the
    /// four octets of padding as the value's low half and returned a different
    /// number with no error anywhere.
    public mutating func getU64() -> UInt64 {
        let b = getLE(8, 8)
        var v: UInt64 = 0
        for i in 0..<8 { v |= UInt64(b[i]) << (8 * UInt64(i)) }
        return v
    }
    public mutating func getF32() -> Float {
        Float(bitPattern: getU32())
    }
    public mutating func getF64() -> Double {
        Double(bitPattern: getU64())
    }
    public mutating func getBool() -> Bool { getU8() != 0 }
    /// The one octet an IDL `char` occupies.
    public mutating func getChar() -> UInt32 { UInt32(getU8()) }
    /// One two-octet `wchar` code unit.
    public mutating func getWchar() -> UInt32 { UInt32(getU16()) }
    /// A `wstring` back into its UTF-16 code units. The length word counts
    /// octets, so the unit count is half of it.
    public mutating func getWstring() -> [UInt16] {
        let octets = Int(getU32())
        var units: [UInt16] = []
        units.reserveCapacity(octets / 2)
        for _ in 0..<(octets / 2) { units.append(getU16()) }
        return units
    }
    /// A `bitset` anchor of the given width back into its holder word.
    public mutating func getBitset(anchor: Int) throws -> UInt64 {
        switch anchor {
        case 1: return UInt64(getU8())
        case 2: return UInt64(getU16())
        case 4: return UInt64(getU32())
        case 8: return getU64()
        default:
            throw WireError.outOfRange(
                "a bitset anchor is one, two, four or eight octets, not \(anchor)")
        }
    }
    /// A `fixed<digits,scale>` back into the decimal it was written from: a
    /// leading `-` when the sign nibble is 0xD, `digits - scale` integer digits,
    /// and a `.` with `scale` fraction digits where the scale is not zero.
    ///
    /// The scale is not on the wire, so it is an argument here for the same
    /// reason it is one in `putFixed`.
    public mutating func getFixed(digits: Int, scale: Int) -> String {
        let octets = (digits + 2) / 2
        var nibbles: [UInt8] = []
        for _ in 0..<octets {
            let b = getU8()
            nibbles.append(b >> 4)
            nibbles.append(b & 0x0F)
        }
        let sign = nibbles[nibbles.count - 1]
        let body = Array(nibbles[(nibbles.count - 1 - digits)..<(nibbles.count - 1)])
        var out = sign == 0x0D ? "-" : ""
        for (i, n) in body.enumerated() {
            if scale != 0 && i == digits - scale { out += "." }
            out += String(n)
        }
        return out
    }

    /// Reads a DHEADER and returns the announced body byte length.
    public mutating func dheaderRead() -> UInt32 { getU32() }

    /// A member header as `emheaderRead` returns it.
    public struct Member {
        public let id: UInt32
        public let mustUnderstand: Bool
        public let lengthCode: LengthCode
        /// The member body's extent in octets, measured from the position
        /// `emheaderRead` left the cursor at.
        public let extent: UInt32
    }

    /// A member header.
    ///
    /// LC0..LC3 take the extent from the code. LC4 consumes the NEXTINT that
    /// follows the header, and the extent is that word. LC5..LC7 do not consume
    /// anything: the next four octets are the member's own leading word and
    /// belong to the body, so they are peeked and the extent is 4 + the word for
    /// LC5, 4 + 4×word for LC6 and 4 + 8×word for LC7 — the last two count
    /// elements, not octets.
    public mutating func emheaderRead() -> Member {
        let header = getU32()
        let id = header & 0x0FFF_FFFF
        let mustUnderstand = header & (1 << 31) != 0
        let lengthCode = LengthCode(rawValue: Int((header >> 28) & 0x7))!
        var extent: UInt32 = 0
        switch lengthCode {
        case .lc0, .lc1, .lc2, .lc3:
            extent = UInt32(1) << UInt32(lengthCode.rawValue)
        case .lc4:
            extent = getU32()
        case .lc5, .lc6, .lc7:
            let at = pos
            let word = getU32()
            pos = at
            switch lengthCode {
            case .lc5: extent = 4 + word
            case .lc6: extent = 4 + 4 * word
            default: extent = 4 + 8 * word
            }
        }
        return Member(id: id, mustUnderstand: mustUnderstand, lengthCode: lengthCode, extent: extent)
    }

    /// A PL_CDR1 parameter header as `pidRead` returns it.
    public struct Parameter {
        public let id: UInt32
        public let mustUnderstand: Bool
        public let length: UInt32
        public let listEnd: Bool
    }

    /// A PL_CDR1 parameter header in either form.
    ///
    /// The extended form is recognised by the marker PID_EXTENDED, and the
    /// must-understand answer then comes from bit 30 of the eMemberHeader and
    /// never from the marker — the marker carries the flag under this framing
    /// whatever the member says.
    ///
    /// The value's alignment origin is moved to the position this call leaves,
    /// and `pidSkip` restores it.
    public mutating func pidRead() -> Parameter {
        let raw = getU16()
        let short = raw & 0x3FFF
        if short == pidListEndID {
            _ = getU16()
            return Parameter(id: 0, mustUnderstand: false, length: 0, listEnd: true)
        }
        if short == pidExtendedID {
            _ = getU16()  // the slength, always 8
            let header = getU32()
            let length = getU32()
            origin = pos
            return Parameter(
                id: header & 0x0FFF_FFFF,
                mustUnderstand: header & emFlagMustUnderstand != 0,
                length: length, listEnd: false)
        }
        let length = UInt32(getU16())
        origin = pos
        return Parameter(
            id: UInt32(short), mustUnderstand: raw & pidFlagMustUnderstand != 0,
            length: length, listEnd: false)
    }

    /// Moves the cursor past a parameter value of `length` octets from
    /// `bodyStart` and restores the stream's alignment origin.
    public mutating func pidSkip(_ bodyStart: Int, _ length: UInt32) {
        pos = bodyStart + Int(length)
        origin = 0
        while pos % 4 != 0 { pos += 1 }
    }
    public mutating func getString() -> String {
        let n = Int(getU32())
        let s = String(decoding: data[pos..<pos + n - 1], as: UTF8.self)
        pos += n
        return s
    }
    public mutating func getSeqU8() -> [UInt8] {
        let n = Int(getU32())
        let b = Array(data[pos..<pos + n])
        pos += n
        return b
    }
}

// --- XRCE WRITE_DATA framing ---

public let xrceSessionNoKey: UInt8 = 0x80
public let xrceStreamBestEffort: UInt8 = 0x01
// E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
// DDS-XRCE 1.0 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would additionally
// require a SampleInfo in the body, which this profile does not carry.
public let xrceWriteFlags: UInt8 = 0x01
/// Wire size of the BaseObjectRequest that precedes the sample (7.7.8).
public let xrceBaseSize = 4
/// OBJK_DATAWRITER -- the ObjectKind of a DataWriter (7.2.2).
public let objkDataWriter: UInt16 = 0x05

/// An ObjectId (7.2.1) from a 12-bit raw id and a 4-bit ObjectKind.
public func xrceObjectId(_ rawId: UInt16, _ kind: UInt16) -> UInt16 {
    ((rawId & 0x0FFF) << 4) | (kind & 0x0F)
}

// The 16-bit submessage_length field cannot describe a sample larger than
// 0xFFFF, so a sample above it has no WRITE_DATA form: refuse rather than
// truncate the length while appending the full payload (which the C SDK's
// zdw_xrce_write_frame rejects).
//
// Optional, and not an empty array. This returned [] and the comment said the
// caller saw the refusal; Client.write never looked, so
// xrceWriteFrame(session:.., stream:.., seq: 1, sample: [UInt8](repeating: 0,
// count: 70000)) had .count == 0 and an empty datagram went out with no error
// anywhere. nil is a refusal a caller has to unwrap before it can spend it.
//
// The submessage body is the BaseObjectRequest (7.7.8: request_id then
// object_id, each two octets most significant first) followed by the sample.
// `objectId` names the target DataWriter -- without it no XRCE agent can route
// the sample. The request_id is the message's own `seq`, which 7.7.8 only
// requires to be unique per stream.
public func xrceWriteFrame(
    session: UInt8, stream: UInt8, seq: UInt16, objectId: UInt16, sample: [UInt8]
) -> [UInt8]? {
    let bodyLen = xrceBaseSize + sample.count
    guard bodyLen <= 0xFFFF else { return nil }
    var out: [UInt8] = [
        session, stream,
        UInt8(seq & 0xff), UInt8((seq >> 8) & 0xff),
        0x07, xrceWriteFlags,
        UInt8(bodyLen & 0xff), UInt8((bodyLen >> 8) & 0xff),
        UInt8((seq >> 8) & 0xff), UInt8(seq & 0xff),
        UInt8((objectId >> 8) & 0xff), UInt8(objectId & 0xff),
    ]
    out.append(contentsOf: sample)
    return out
}

// Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo, len_hi]
// then the sample body. The 16-bit little-endian submessage_length (bytes 6..7)
// bounds the body exactly: the returned slice is frame[8 ..< 8+len], never
// frame[8...] to the end, so trailing padding or an appended submessage is not
// folded into the sample.
//
// Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
// hub->endpoint) — the hub pushes samples with DATA. See DDS-XRCE spec 8.3.5.
// Returns nil (never a crash) for a short header, a wrong submessage id, or a
// declared length that runs past the datagram (truncation / wrong length).
public func xrceReadFrame(_ frame: [UInt8]) -> [UInt8]? {
    guard frame.count >= 8 + xrceBaseSize, frame[4] == 0x07 || frame[4] == 0x09 else { return nil }
    let smLen = Int(frame[6]) | (Int(frame[7]) << 8)
    guard smLen >= xrceBaseSize, 8 + smLen <= frame.count else { return nil }
    return Array(frame[(8 + xrceBaseSize) ..< 8 + smLen])
}

/// `(requestId, objectId)` of a data frame (7.7.8), or nil under the same rules
/// as `xrceReadFrame`.
public func xrceFrameBase(_ frame: [UInt8]) -> (requestId: UInt16, objectId: UInt16)? {
    guard xrceReadFrame(frame) != nil else { return nil }
    return (
        (UInt16(frame[8]) << 8) | UInt16(frame[9]),
        (UInt16(frame[10]) << 8) | UInt16(frame[11])
    )
}

// --- transport ---

public protocol Transport: Sendable {
    func deliver(_ frame: [UInt8])
    func receive() -> [UInt8]?
}

// --- sync client ---

public final class Client {
    let transport: Transport
    let session: UInt8 = xrceSessionNoKey
    let stream: UInt8 = xrceStreamBestEffort
    var seq: UInt16 = 1
    /// ObjectId of the target DataWriter (7.7.8 BaseObjectRequest).
    let objectId: UInt16
    public init(_ transport: Transport, objectId: UInt16) {
        self.transport = transport
        self.objectId = objectId
    }

    // A sample the submessage_length cannot describe stops the program rather
    // than going out as a short datagram. preconditionFailure and not a thrown
    // error: `write` is not `throws` in any of the fifteen SDKs and the size of
    // a sample is a property of the caller's own code, which is what a
    // precondition is for — the same reading as Kotlin's `require`. The
    // sequence is not advanced, because nothing was written.
    public func write(_ sample: [UInt8]) {
        guard
            let frame = xrceWriteFrame(
                session: session, stream: stream, seq: seq, objectId: objectId, sample: sample)
        else {
            preconditionFailure(
                "\(sample.count) octets: the 16-bit XRCE submessage_length describes at most 65535"
            )
        }
        seq = seq &+ 1
        transport.deliver(frame)
    }
    // One non-blocking receive: returns the sample body, or nil.
    public func poll() -> [UInt8]? {
        guard let frame = transport.receive() else { return nil }
        return xrceReadFrame(frame)
    }
}

// --- async reader: an AsyncStream (idiomatic Swift concurrency) ---

public final class AsyncReader: Sendable {
    let transport: Transport
    public init(_ transport: Transport) { self.transport = transport }

    public func stream() -> AsyncStream<[UInt8]> {
        let transport = self.transport
        return AsyncStream { continuation in
            let task = Task {
                while !Task.isCancelled {
                    if let frame = transport.receive() {
                        if let body = xrceReadFrame(frame) { continuation.yield(body) }
                    } else {
                        try? await Task.sleep(nanoseconds: 1_000_000)
                    }
                }
                continuation.finish()
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
}

// In-memory FIFO transport for tests + example.
public final class MemTransport: Transport, @unchecked Sendable {
    private var q: [[UInt8]] = []
    private let lock = NSLock()
    public init() {}
    public func deliver(_ frame: [UInt8]) {
        lock.lock(); q.append(frame); lock.unlock()
    }
    public func receive() -> [UInt8]? {
        lock.lock(); defer { lock.unlock() }
        return q.isEmpty ? nil : q.removeFirst()
    }
}
