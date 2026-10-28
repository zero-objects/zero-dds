// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Putting bytes on the wire. The mirror of `ZdReader` clause for
// clause, and the Swift counterpart of `runtime/d`'s Writer: alignment
// is relative to the start of the stream — XCDR aligns each primitive
// to its own size counted from the encapsulation. XTypes 1.3 §7.4.1.1.

/// Accumulates the bytes of one sample.
public final class ZdWriter {
    private var out: [UInt8] = []
    private let endianness: ZdEndianness
    private var origin = 0
    private let maxAlign: Int
    private let plCdr1: ZdPlCdr1Framing
    /// Whether an LC4 header whose member turns out to be 1, 2, 4 or 8
    /// bytes is rewritten with a short length code (Fast DDS's form).
    private let shortLengthCodes: Bool

    public init(
        _ endianness: ZdEndianness,
        _ maxAlign: Int,
        _ plCdr1: ZdPlCdr1Framing,
        _ shortLengthCodes: Bool
    ) {
        self.endianness = endianness
        self.maxAlign = max(1, maxAlign)
        self.plCdr1 = plCdr1
        self.shortLengthCodes = shortLengthCodes
    }

    /// How many bytes have been written.
    public func position() -> Int {
        return out.count
    }

    /// The bytes written, as a copy.
    public func finish() -> [UInt8] {
        return out
    }

    private func alignTo(_ alignment: Int) {
        let capped = min(alignment, maxAlign)
        if capped <= 1 {
            return
        }
        let offset = out.count - origin
        let padding = (capped - offset % capped) % capped
        for _ in 0..<padding {
            out.append(0)
        }
    }

    private func writeRaw(_ unsigned: UInt64, _ width: Int) {
        if endianness == .little {
            for i in 0..<width {
                out.append(UInt8(truncatingIfNeeded: unsigned >> (8 * UInt64(i))))
            }
        } else {
            for i in stride(from: width - 1, through: 0, by: -1) {
                out.append(UInt8(truncatingIfNeeded: unsigned >> (8 * UInt64(i))))
            }
        }
    }

    private func writeUnsigned(_ unsigned: UInt64, _ width: Int) {
        alignTo(width)
        writeRaw(unsigned, width)
    }

    // -- primitives --------------------------------------------------------

    /// `boolean`: one byte, 0 or 1.
    public func writeBool(_ v: Bool) {
        out.append(v ? 1 : 0)
    }

    /// `octet` and `uint8`.
    public func writeU8(_ v: UInt8) {
        out.append(v)
    }

    /// `int8`.
    public func writeI8(_ v: Int8) {
        out.append(UInt8(bitPattern: v))
    }

    /// `char`: one octet — which is the whole member.
    public func writeChar(_ v: UInt8) {
        out.append(v)
    }

    /// `short`.
    public func writeI16(_ v: Int16) {
        writeUnsigned(UInt64(UInt16(bitPattern: v)), 2)
    }

    /// `unsigned short`.
    public func writeU16(_ v: UInt16) {
        writeUnsigned(UInt64(v), 2)
    }

    /// `long` — 32 bits, which is what an IDL `long` is.
    public func writeI32(_ v: Int32) {
        writeUnsigned(UInt64(UInt32(bitPattern: v)), 4)
    }

    /// `unsigned long`.
    public func writeU32(_ v: UInt32) {
        writeUnsigned(UInt64(v), 4)
    }

    /// `long long`.
    public func writeI64(_ v: Int64) {
        writeUnsigned(UInt64(bitPattern: v), 8)
    }

    /// `unsigned long long`.
    public func writeU64(_ v: UInt64) {
        writeUnsigned(v, 8)
    }

    /// `float`.
    public func writeF32(_ v: Float) {
        writeUnsigned(UInt64(v.bitPattern), 4)
    }

    /// `double`.
    public func writeF64(_ v: Double) {
        writeUnsigned(v.bitPattern, 8)
    }

    /// `long double`: sixteen bytes of IEEE 754 binary128, aligned to
    /// eight (capped by MAXALIGN). The value arrives as decimal text
    /// and is correctly rounded here, once. The two byte orders are the
    /// whole sixteen bytes reversed — CORBA 3.3 part 2 §9.3.1.3.
    public func writeF128(_ v: String) throws {
        let bytes = try zdF128FromDecimal(v, out.count)
        alignTo(8)
        if endianness == .little {
            for i in 0..<16 {
                out.append(bytes[i])
            }
        } else {
            for i in stride(from: 15, through: 0, by: -1) {
                out.append(bytes[i])
            }
        }
    }

    // -- characters and strings --------------------------------------------

    /// `wchar`, at `width` octets and in `framing`'s shape.
    /// "giop-length-prefixed" is CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2
    /// form: the octet count first, then the code unit's octets, all
    /// aligned to one; every order but "stream" means big-endian there.
    public func writeWchar(_ v: UInt16, _ width: Int, _ framing: String, _ order: String) throws {
        if width != 2 && width != 4 {
            try zdFailOutOfRange(out.count, "a wchar width of 2 or 4")
        }
        if framing == "giop-length-prefixed" {
            out.append(UInt8(width))
            let big = order != "stream" || endianness != .little
            if big {
                for i in stride(from: width - 1, through: 0, by: -1) {
                    out.append(UInt8(truncatingIfNeeded: UInt32(v) >> (8 * UInt32(i))))
                }
            } else {
                for i in 0..<width {
                    out.append(UInt8(truncatingIfNeeded: UInt32(v) >> (8 * UInt32(i))))
                }
            }
            return
        }
        if framing != "fixed" {
            try zdFailOutOfRange(out.count, "a wchar framing, not `\(framing)`")
        }
        if width == 4 {
            writeU32(UInt32(v))
            return
        }
        writeU16(v)
    }

    /// A string: a four-byte length including the terminator, the UTF-8
    /// bytes, then a NUL. The length counts bytes. XTypes 1.3 §7.4.1.1.
    public func writeString(_ v: String) {
        let bytes = Array(v.utf8)
        writeU32(UInt32(bytes.count + 1))
        out.append(contentsOf: bytes)
        out.append(0)
    }

    /// A bounded string: the same bytes `writeString` writes. The bound
    /// is a promise about the source and not a difference on the wire,
    /// so this is the only place it can be enforced — refused rather
    /// than cut at a character boundary, which would put a sample on
    /// the wire nobody wrote.
    public func writeStringBounded(_ v: String, _ bound: Int) throws {
        let octets = v.utf8.count
        if octets > bound {
            try zdFailOutOfRange(
                out.count,
                "at most \(bound) octets in a bounded string, and this one has \(octets)")
        }
        writeString(v)
    }

    /// The count check every bounded sequence makes before a byte of it
    /// is written. Table 17 makes `O2.length <= T1.length` the
    /// condition for constructing a `sequence<T, N>`, and the wire
    /// carries no bound to compare against: a sender whose count
    /// already exceeds its own declaration has written a value of a
    /// type nobody declared.
    public func checkSequenceBound(_ actual: UInt32, _ bound: UInt32) throws {
        if actual > bound {
            try zdFailOutOfRange(
                out.count,
                "at most \(bound) elements in a bounded sequence, and this one has \(actual)")
        }
    }

    /// The count check every bounded map makes before a byte of it is
    /// written. Its own message and not `checkSequenceBound`'s: the
    /// unit is an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
    /// map-as-a-sequence-of-pairs equivalence.
    public func checkMapBound(_ actual: UInt32, _ bound: UInt32) throws {
        if actual > bound {
            try zdFailOutOfRange(
                out.count,
                "at most \(bound) entries in a bounded map, and this one has \(actual)")
        }
    }

    /// A wstring, in the form `policy` and `order` name. The value
    /// arrives as a Swift `String` and the conversion to UTF-16 units
    /// happens here, which is what makes §7.4.1.1.2's surrogate rule
    /// this runtime's own reading.
    public func writeWstringUtf8(_ v: String, _ policy: String, _ order: String) throws {
        guard let cell = zdWstringCellOf(policy) else {
            try zdFailOutOfRange(out.count, "a wstring length policy, not `\(policy)`")
        }
        if !zdWstringOrderKnown(order) {
            try zdFailOutOfRange(out.count, "a wstring unit order, not `\(order)`")
        }
        var units = zdUtf16UnitsOf(v)
        if cell.countsCharacters && cell.width == 2 {
            // A character count over two-octet units has no way to
            // carry a code point outside the BMP.
            for (index, unit) in units.enumerated() where unit >= 0xD800 && unit <= 0xDFFF {
                try zdFailOutOfRange(
                    index,
                    "a code point inside the BMP: a character count over two-octet units "
                        + "has no way to carry one outside it")
            }
        }
        if cell.terminated {
            units.append(0)
        }
        // An empty wstring carries no mark.
        if order == "byte-order-mark" && !units.isEmpty {
            units.insert(zdByteOrderMark, at: 0)
        }
        if cell.countsCharacters {
            writeU32(UInt32(units.count))
        } else {
            writeU32(UInt32(units.count * cell.width))
        }
        for unit in units {
            writeWstringUnit(unit, cell.width, order)
        }
    }

    /// A bounded wstring, as `writeStringBounded` is to `writeString`.
    /// The bound counts UTF-16 code units, not characters and not
    /// octets, whatever `policy` puts on the wire.
    public func writeWstringBounded(_ v: String, _ bound: Int, _ policy: String, _ order: String)
        throws
    {
        let units = zdUtf16UnitsOf(v).count
        if units > bound {
            try zdFailOutOfRange(
                out.count,
                "at most \(bound) UTF-16 code units in a bounded wstring, and this one has "
                    + "\(units)")
        }
        try writeWstringUtf8(v, policy, order)
    }

    /// A `wstring` whose characters go out as their own UTF-8 octets.
    ///
    /// `WstringCodeUnits::Utf8` is the measured departure of one
    /// product's generated serialiser: XTypes 1.3 Table 31 gives a
    /// `wstring`'s code unit as `Char16` and §7.4.1.1.2 makes the length
    /// "twice the number of characters", so the clause has no UTF-8 form
    /// at all. The declared length is the UTF-8 octet count plus the
    /// terminator the policy asks for. `wstring-member` is
    /// `04 00 00 00 61 62 63 00` for `"abc"`: three octets and a NUL,
    /// and the four declares both.
    public func writeWstringUtf8Octets(_ v: String, _ policy: String) throws {
        guard let cell = zdWstringCellOf(policy) else {
            try zdFailOutOfRange(out.count, "a wstring length policy, not `\(policy)`")
        }
        let bytes = Array(v.utf8)
        let length = bytes.count + (cell.terminated ? 1 : 0)
        if UInt64(length) > UInt64(UInt32.max) {
            try zdFailOutOfRange(out.count, "a wstring length the UInt32 field can hold")
        }
        writeU32(UInt32(length))
        out.append(contentsOf: bytes)
        if cell.terminated {
            out.append(0)
        }
    }

    /// A bounded UTF-8-octet `wstring`, as `writeWstringBounded` is to
    /// `writeWstringUtf8`: the bound still counts UTF-16 code units,
    /// which is what a `wstring<N>` bound means to every peer, while the
    /// wire carries the UTF-8 octets.
    public func writeWstringUtf8OctetsBounded(_ v: String, _ bound: Int, _ policy: String) throws {
        let units = zdUtf16UnitsOf(v).count
        if units > bound {
            try zdFailOutOfRange(
                out.count,
                "at most \(bound) UTF-16 code units in a bounded wstring, and this one has "
                    + "\(units)")
        }
        try writeWstringUtf8Octets(v, policy)
    }

    private func writeWstringUnit(_ unit: UInt16, _ width: Int, _ order: String) {
        let little = order == "byte-order-mark" || order == "host-unswapped"
        if width == 4 {
            if order == "big-endian-always" {
                writeU32Big(UInt32(unit))
            } else if little {
                writeU32Little(UInt32(unit))
            } else {
                writeU32(UInt32(unit))
            }
            return
        }
        if order == "big-endian-always" {
            writeU16Big(unit)
        } else if little {
            writeU16Little(unit)
        } else {
            writeU16(unit)
        }
    }

    /// A 16-bit value little-endian whatever the stream's order is.
    public func writeU16Little(_ v: UInt16) {
        alignTo(2)
        out.append(UInt8(truncatingIfNeeded: v))
        out.append(UInt8(truncatingIfNeeded: v >> 8))
    }

    /// A 16-bit value big-endian whatever the stream's order is.
    public func writeU16Big(_ v: UInt16) {
        alignTo(2)
        out.append(UInt8(truncatingIfNeeded: v >> 8))
        out.append(UInt8(truncatingIfNeeded: v))
    }

    /// A 32-bit value little-endian whatever the stream's order is.
    public func writeU32Little(_ v: UInt32) {
        alignTo(4)
        for i in 0..<4 {
            out.append(UInt8(truncatingIfNeeded: v >> (8 * UInt32(i))))
        }
    }

    /// A 32-bit value big-endian whatever the stream's order is.
    public func writeU32Big(_ v: UInt32) {
        alignTo(4)
        for i in stride(from: 3, through: 0, by: -1) {
            out.append(UInt8(truncatingIfNeeded: v >> (8 * UInt32(i))))
        }
    }

    // -- fixed -------------------------------------------------------------

    /// `fixed<digits, scale>` as packed BCD, [CDR] §9.3.2.8 — the only
    /// wire definition an IDL fixed has. `count` is "declared",
    /// "value-precision" or "value-string-digits".
    public func writeFixed(_ v: String, _ digits: Int, _ scale: Int, _ count: String) throws {
        let nibbles = try zdFixedNibbles(v, digits, scale, count, out.count)
        var i = 0
        while i < nibbles.count {
            out.append(UInt8((nibbles[i] << 4) | nibbles[i + 1]))
            i += 2
        }
    }

    // -- the bit-bounded kinds ---------------------------------------------

    /// An enumerator, as the integer its bit bound calls for. The
    /// holder is signed — §7.4.3.5.3 rule (5) — and what fits is
    /// everything expressible in the bound's bits: -2^(n-1) ..= 2^n - 1.
    public func writeEnum(_ bitBound: Int, _ value: Int64) throws {
        if bitBound == 0 || bitBound > 32 {
            try zdFailOutOfRange(out.count, "an @bit_bound between 1 and 32, not \(bitBound)")
        }
        if value < -(Int64(1) << (bitBound - 1)) || value > (Int64(1) << bitBound) - 1 {
            try zdFailOutOfRange(out.count, "an enumerator a \(bitBound)-bit holder can hold")
        }
        let pattern = UInt64(bitPattern: value) & ((UInt64(1) << UInt64(bitBound)) - 1)
        if bitBound <= 8 {
            writeU8(UInt8(truncatingIfNeeded: pattern))
        } else if bitBound <= 16 {
            writeU16(UInt16(truncatingIfNeeded: pattern))
        } else {
            writeU32(UInt32(truncatingIfNeeded: pattern))
        }
    }

    /// A bitmask, as the unsigned integer Table 33 gives its bound.
    public func writeBitmask(_ bitBound: Int, _ value: UInt64) throws {
        if bitBound == 0 || bitBound > 64 {
            try zdFailOutOfRange(out.count, "an @bit_bound between 1 and 64, not \(bitBound)")
        }
        if bitBound < 64 && value >= UInt64(1) << UInt64(bitBound) {
            try zdFailOutOfRange(out.count, "a bitmask below 2^\(bitBound)")
        }
        if bitBound <= 8 {
            writeU8(UInt8(truncatingIfNeeded: value))
        } else if bitBound <= 16 {
            writeU16(UInt16(truncatingIfNeeded: value))
        } else if bitBound <= 32 {
            writeU32(UInt32(truncatingIfNeeded: value))
        } else {
            writeU64(value)
        }
    }

    /// One bitfield, shifted into the accumulator a bitset packs into.
    /// A value wider than the field is refused rather than masked.
    public func bitfield(_ value: UInt64, _ width: Int, _ position: Int) throws -> UInt64 {
        if width <= 0 || width > 64 || position < 0 || position + width > 64 {
            try zdFailOutOfRange(out.count, "a bitfield of \(width) bits at \(position), inside 64")
        }
        if width < 64 && value > (UInt64(1) << UInt64(width)) - 1 {
            try zdFailOutOfRange(out.count, "a value inside \(width) bits")
        }
        return value << UInt64(position)
    }

    /// The Cyclone union overlay: the low `octets` of `pattern`,
    /// written as one unsigned integer of that width in the stream's
    /// byte order. A discriminator carrying a label the `union-labels`
    /// dialect axis says selects nothing makes the peer fall through to
    /// `default:` and serialise its one C union storage as that arm —
    /// the selected member's octets, reinterpreted. The pattern is the
    /// UInt64 whose little-endian bytes are the selected member's
    /// little-endian storage octets, zero-extended; the `zdBitPattern*`
    /// functions produce it.
    public func writeUnionOverlay(_ pattern: UInt64, _ octets: Int) throws {
        switch octets {
        case 1:
            writeU8(UInt8(truncatingIfNeeded: pattern))
        case 2:
            writeU16(UInt16(truncatingIfNeeded: pattern))
        case 4:
            writeU32(UInt32(truncatingIfNeeded: pattern))
        case 8:
            writeU64(pattern)
        default:
            try zdFailOutOfRange(out.count, "a union arm of 1, 2, 4 or 8 octets, not \(octets)")
        }
    }

    /// A bitset, as the anchor integer its bitfields pack into; the
    /// anchor's width comes from the layout planner.
    public func writeBitset(_ anchor: Int, _ value: UInt64) throws {
        switch anchor {
        case 1:
            writeU8(UInt8(truncatingIfNeeded: value))
        case 2:
            writeU16(UInt16(truncatingIfNeeded: value))
        case 4:
            writeU32(UInt32(truncatingIfNeeded: value))
        case 8:
            writeU64(value)
        default:
            try zdFailOutOfRange(out.count, "a bitset anchor of 1, 2, 4 or 8 bytes, not \(anchor)")
        }
    }

    // -- packed collection elements ----------------------------------------

    /// Move the alignment origin to the current position and answer the
    /// origin displaced, for `finishPackedElement`.
    ///
    /// `ElementPadding::Packed` is HDDS's measured departure from
    /// §7.4.1, which counts every element's alignment from the
    /// encapsulation: each collection element and each aggregate value
    /// counts its own alignment from its own first octet instead.
    /// `array-1d-struct-final` is
    /// `01000000 0200 03000000 0400 77`, the second `Cell`'s `long` at
    /// offset six where the clause puts it at eight.
    public func beginPackedElement() -> Int {
        let saved = origin
        origin = out.count
        return saved
    }

    /// Restore the origin `beginPackedElement` displaced.
    public func finishPackedElement(_ saved: Int) {
        origin = saved
    }

    // -- framing -----------------------------------------------------------

    /// A DHEADER: a four-byte length placeholder, patched when the
    /// members are done.
    public func writeDheader() -> ZdDHeader {
        alignTo(4)
        let at = out.count
        out.append(contentsOf: [0, 0, 0, 0])
        return ZdDHeader(at)
    }

    /// Patch a DHEADER with the length of what followed it; a header
    /// from `zdNoDheader` is not there and is ignored.
    public func finishDheader(_ header: ZdDHeader) throws {
        if header.at < 0 {
            return
        }
        try patch(header.at, UInt64(out.count - header.at - 4), 4)
    }

    /// An XCDR2 EMHEADER: id, length code and must-understand in one
    /// 32-bit word. §7.4.3.4.2. LC4 reserves a NEXTINT; LC5 to LC7
    /// reuse the member's own leading word.
    public func writeEmheader(_ memberId: Int, _ lengthCode: Int, _ mustUnderstand: Bool)
        throws -> ZdEmHeader
    {
        // §7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and
        // §7.4.3.4.2 leaves them the low 28 bits of the word. Masking a
        // larger one writes a well-formed header for `id & 0x0FFFFFFF`:
        // the peer reads a member the caller did not name, and nothing at
        // either end says so.
        if memberId < 0 || UInt32(truncatingIfNeeded: memberId) > zdEMemberIdMask {
            try zdFailOutOfRange(
                out.count, "a member id inside the 28 bits of an EMHEADER, not \(memberId)")
        }
        alignTo(4)
        var word = (UInt32(lengthCode & 0x7) << 28) | (UInt32(memberId) & zdEMemberIdMask)
        if mustUnderstand {
            word |= 0x8000_0000
        }
        writeU32(word)
        let wordAt = out.count - 4
        if lengthCode == 4 {
            let at = out.count
            out.append(contentsOf: [0, 0, 0, 0])
            return ZdEmHeader(at, wordAt, lengthCode)
        }
        return ZdEmHeader(-1, wordAt, lengthCode)
    }

    /// Patch an EMHEADER's NEXTINT; under `shortLengthCodes` an LC4
    /// header whose member is 1, 2, 4 or 8 bytes loses its NEXTINT and
    /// states the length in the code.
    public func finishEmheader(_ header: ZdEmHeader) throws {
        if header.nextInt < 0 {
            // Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
            // octets, §7.4.3.4.2. A value of another size misdeclares it
            // and the reader's skip lands inside data, so the mismatch is
            // refused here rather than written. Codes 5 to 7 declare
            // nothing of their own: the member's leading length word is
            // the NEXTINT.
            if header.lengthCode >= 0 && header.lengthCode <= 3
                && header.word + 4 <= out.count
            {
                let declared = 1 << header.lengthCode
                let body = out.count - header.word - 4
                if body != declared {
                    try zdFailOutOfRange(
                        out.count,
                        "a member of \(declared) octets under length code "
                            + "\(header.lengthCode), not \(body)")
                }
            }
            return
        }
        let length = out.count - header.nextInt - 4
        if shortLengthCodes && (length == 1 || length == 2 || length == 4 || length == 8) {
            try shortenEmheader(header, length)
            return
        }
        try patch(header.nextInt, UInt64(length), 4)
    }

    private func shortenEmheader(_ header: ZdEmHeader, _ length: Int) throws {
        let code: UInt32
        switch length {
        case 1:
            code = 0
        case 2:
            code = 1
        case 4:
            code = 2
        default:
            code = 3
        }
        out.removeSubrange(header.nextInt..<(header.nextInt + 4))
        let previous = readPatched(header.word)
        try patch(header.word, UInt64((previous & ~(UInt32(0x7) << 28)) | (code << 28)), 4)
    }

    private func readPatched(_ at: Int) -> UInt32 {
        var value: UInt32 = 0
        if endianness == .little {
            for i in stride(from: 3, through: 0, by: -1) {
                value = (value << 8) | UInt32(out[at + i])
            }
        } else {
            for i in 0..<4 {
                value = (value << 8) | UInt32(out[at + i])
            }
        }
        return value
    }

    /// A PL_CDR1 parameter header, short form: [id:u16][length:u16].
    public func writePid(_ pid: Int, _ role: ZdParameterRole, _ mustUnderstand: Bool)
        throws -> ZdPid
    {
        // §7.4.1.2.2 gives the short form the ids 0 to `highestShortId`
        // inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
        // itself. A short header with a reserved id collides with the
        // protocol's own parameters, so such an id takes the extended form
        // or none. This is the write side of the constant the reader has
        // always consulted.
        if pid < 0 || pid > plCdr1.highestShortId {
            try zdFailOutOfRange(
                out.count,
                "a parameter id the short header carries, not \(pid)"
                    + " — the extended header takes it")
        }
        alignTo(4)
        var flagged = pid
        if mustUnderstand && plCdr1.memberMustUnderstand {
            flagged |= zdPidMustUnderstand
        }
        flagged |= try zdPidFlagsAt(plCdr1.pidFlags, "member")
        let bare = flagged & ~(zdPidMustUnderstand | zdPidImplExtension)
        writeU16(UInt16(truncatingIfNeeded: flagged))
        let at = out.count
        out.append(contentsOf: [0, 0])
        return ZdPid(at, false, beginParameterValue(), role, bare, mustUnderstand)
    }

    /// A PL_CDR1 parameter header, extended form:
    /// [PID_EXTENDED][8][id:u32][length:u32]. §7.4.1.2. The member's
    /// must-understand is FLAG_2 of the eMemberHeader.
    public func writePidExtended(_ pid: Int, _ role: ZdParameterRole, _ mustUnderstand: Bool)
        throws -> ZdPid
    {
        // The same 28 bits as an EMHEADER's, here in the eMemberHeader of
        // §7.4.1.2.1. Masking aliases the parameter onto another member
        // exactly as it does there.
        if pid < 0 || UInt32(truncatingIfNeeded: pid) > zdEMemberIdMask {
            try zdFailOutOfRange(
                out.count, "a member id inside the 28 bits of an eMemberHeader, not \(pid)")
        }
        alignTo(4)
        let marker = try zdPidExtended | zdPidFlagsAt(plCdr1.pidFlags, "extended-marker")
        writeU16(UInt16(truncatingIfNeeded: marker))
        writeU16(8)
        var memberHeader = UInt32(truncatingIfNeeded: pid) & zdEMemberIdMask
        if mustUnderstand && plCdr1.memberMustUnderstand {
            memberHeader |= zdEFlagMustUnderstand
        }
        writeU32(memberHeader)
        let at = out.count
        out.append(contentsOf: [0, 0, 0, 0])
        return ZdPid(at, true, beginParameterValue(), role, 0, false)
    }

    private func beginParameterValue() -> Int {
        let saved = origin
        origin = out.count
        return saved
    }

    /// Patch a parameter header with its value's length; the padding to
    /// four goes outside the declared length or inside it, by framing.
    public func finishPid(_ header: ZdPid) throws {
        let width = header.extended ? 4 : 2
        var length = out.count - header.at - width
        if plCdr1.padsDeclaredLength(header.role) {
            let past = (out.count - header.savedOrigin) % 4
            if past != 0 {
                length += 4 - past
            }
        }
        if !header.extended && length > zdPidShortMaxLen {
            try promotePid(header, length)
        } else {
            try patch(header.at, UInt64(length), width)
        }
        origin = header.savedOrigin
        // Under Fast DDS's framing the parameter writes no padding of
        // its own — the next header aligns itself, and a final
        // parameter ends the stream where its value does.
        if plCdr1.padAfterValue {
            alignTo(4)
        }
    }

    private func promotePid(_ header: ZdPid, _ length: Int) throws {
        let start = header.at - 2
        out.insert(contentsOf: [0, 0, 0, 0, 0, 0, 0, 0], at: header.at + 2)
        let marker = try zdPidExtended | zdPidFlagsAt(plCdr1.pidFlags, "extended-marker")
        try patch(start, UInt64(marker), 2)
        try patch(start + 2, 8, 2)
        var memberHeader = UInt32(truncatingIfNeeded: header.pid) & zdEMemberIdMask
        if header.mustUnderstand && plCdr1.memberMustUnderstand {
            memberHeader |= zdEFlagMustUnderstand
        }
        try patch(start + 4, UInt64(memberHeader), 4)
        try patch(start + 8, UInt64(length), 4)
    }

    /// The PID_LIST_END sentinel a PL_CDR1 parameter list ends with;
    /// its flags come from the framing asked at the "sentinel" position.
    public func writeSentinel() throws {
        alignTo(4)
        let sentinel = try zdPidListEnd | zdPidFlagsAt(plCdr1.pidFlags, "sentinel")
        writeU16(UInt16(truncatingIfNeeded: sentinel))
        writeU16(0)
    }

    private func patch(_ at: Int, _ value: UInt64, _ width: Int) throws {
        if width < 8 && value >= UInt64(1) << (UInt64(width) * 8) {
            try zdFailOutOfRange(at, "a \(width)-byte length")
        }
        if endianness == .little {
            for i in 0..<width {
                out[at + i] = UInt8(truncatingIfNeeded: value >> (8 * UInt64(i)))
            }
        } else {
            for i in 0..<width {
                out[at + width - 1 - i] = UInt8(truncatingIfNeeded: value >> (8 * UInt64(i)))
            }
        }
    }
}
